//! The TechnoApe client: window, renderer and the benchmark runs (docs/plans/roadmap.md, "The shared benchmark").
mod sys;

use engine::stream::{DrawChunk, FrameStats, Opts, Streamer, View};
use engine::world::World;
use glam::{DVec3, Mat4, Vec3, Vec4};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

const W: u32 = 1920;
const H: u32 = 1080;
const MAX_CHUNKS: usize = 1 << 16;
const TS_RING: usize = 8;
const MAX_BODIES: usize = 4; // the size of `bodies` in shader.wgsl

struct Args(HashMap<String, String>);
impl Args {
    fn get<T: std::str::FromStr>(&self, k: &str, d: T) -> T {
        self.0.get(k).and_then(|v| v.parse().ok()).unwrap_or(d)
    }
    fn s(&self, k: &str, d: &str) -> String {
        self.0.get(k).cloned().unwrap_or(d.to_string())
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct BodyGpu {
    cam_hi: [f32; 3],
    r0: f32,
    cam_lo: [f32; 3],
    lnq: f32,
    theta: f32,
    half_n: f32,
    _p: [f32; 2],
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    sun: [f32; 4],
    bodies: [BodyGpu; MAX_BODIES],
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ChunkGpu {
    base: [i32; 3],
    info: u32,
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct DrawArgs {
    vertex_count: u32,
    instance_count: u32,
    first_vertex: u32,
    first_instance: u32,
}

/// Camera pose for the run at time t (seconds after load).
struct Pose {
    pos: DVec3,
    fwd: DVec3,
    up: DVec3,
}

fn smooth(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn pose(run: &str, t: f64, w: &World, speed: f64, alt: f64, warm: f64, q: f64, a: &Args) -> Pose {
    let planet = &w.bodies[0];
    let moon = &w.bodies[1];
    let pole = DVec3::new(0.0, planet.height(DVec3::Y) + 1.7, 0.0);
    match run {
        "fly" => {
            let r0 = planet.radius + alt;
            let th = speed * t / r0;
            let dir = DVec3::new(th.sin(), 0.0, th.cos());
            let pos = dir * (planet.height(dir) + alt);
            Pose { pos, fwd: DVec3::new(th.cos(), 0.0, -th.sin()), up: dir }
        }
        "orbit" => {
            let a = pole;
            let b = DVec3::new(0.0, 3500.0, 0.0);
            let c = moon.centre + DVec3::Y * 3500.0;
            let d = moon.centre + DVec3::Y * (moon.height(DVec3::Y) + 1.7);
            let legs = [(a, b, 0), (b, c, 0), (c, d, 1), (d, d, 2), (d, c, 3), (c, b, 3), (b, a, 3)];
            let durs = [10.0, 10.0, 10.0, 5.0, 10.0, 10.0, 10.0].map(|d| d * q);
            let mut tt = (t - warm).max(0.0);
            let mut i = 0;
            while i < legs.len() - 1 && tt > durs[i] {
                tt -= durs[i];
                i += 1;
            }
            if t < warm {
                return Pose { pos: a, fwd: DVec3::Z, up: DVec3::Y };
            }
            let (p0, p1, look) = legs[i];
            let pos = p0.lerp(p1, smooth(tt / durs[i]));
            let near = w.nearest(pos);
            let up = (pos - w.bodies[near].centre).normalize();
            let fwd = match look {
                0 | 1 => (moon.centre - pos).normalize(),
                2 => DVec3::Z,
                _ => (planet.centre - pos).normalize(),
            };
            let fwd = if i == legs.len() - 1 && tt >= durs[i] { DVec3::Z } else { fwd };
            Pose { pos, fwd, up }
        }
        "edit" => Pose { pos: pole, fwd: DVec3::new(0.0, -1.0, 1.0).normalize(), up: DVec3::Y },
        // stand: at=x,y,z picks the spot (a direction from the planet's centre), yaw and pitch turn the head
        _ => {
            let at: Vec<f64> = a.s("at", "0,1,0").split(',').filter_map(|v| v.parse().ok()).collect();
            let up = DVec3::new(at[0], at[1], at[2]).normalize();
            let r = if up.y.abs() > 0.9 { DVec3::Z } else { DVec3::Y };
            let ahead = (r - up * r.dot(up)).normalize();
            let (yaw, pitch) = (a.get("yaw", 0f64).to_radians(), a.get("pitch", 0f64).to_radians());
            let fwd = (ahead * yaw.cos() + up.cross(ahead) * yaw.sin()) * pitch.cos() + up * pitch.sin();
            Pose { pos: up * (planet.height(up) + a.get("eye", 1.7)), fwd, up }
        }
    }
}

fn run_len(run: &str, warm: f64, q: f64) -> f64 {
    warm + q * match run {
        "fly" => 60.0,
        "orbit" => 65.0,
        _ => 30.0,
    }
}

fn frustum_planes(m: Mat4) -> [Vec4; 6] {
    let r = [m.row(0), m.row(1), m.row(2), m.row(3)];
    // reversed-z infinite: near plane is z <= w, no far plane (use a dummy always-true plane)
    let mut p = [r[3] + r[0], r[3] - r[0], r[3] + r[1], r[3] - r[1], r[3] - r[2], Vec4::new(0.0, 0.0, 0.0, 1.0)];
    for q in p.iter_mut() {
        let l = q.truncate().length();
        if l > 0.0 {
            *q /= l;
        }
    }
    p
}

struct Gpu {
    window: Arc<Window>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    bind: wgpu::BindGroup,
    globals: wgpu::Buffer,
    quads: wgpu::Buffer,
    chunks: wgpu::Buffer,
    indirect: wgpu::Buffer,
    depth: wgpu::TextureView,
    ts: Option<(wgpu::QuerySet, wgpu::Buffer, Vec<(wgpu::Buffer, Arc<AtomicBool>, bool)>)>,
    ts_period: f32,
    capture: Option<wgpu::Buffer>,
    adapter_name: String,
    backend: String,
}

impl Gpu {
    fn new(window: Arc<Window>, a: &Args, cap: u32) -> Gpu {
        let backends = match a.s("backend", "vulkan").as_str() {
            "dx12" => wgpu::Backends::DX12,
            _ => wgpu::Backends::VULKAN,
        };
        let mut idesc = wgpu::InstanceDescriptor::new_without_display_handle();
        idesc.backends = backends;
        idesc.flags = wgpu::InstanceFlags::empty();
        let instance = wgpu::Instance::new(idesc);
        let surface = instance.create_surface(window.clone()).unwrap();
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
            apply_limit_buckets: false,
        }))
        .expect("no adapter");
        let info = adapter.get_info();
        eprintln!("[technoape] adapter: {} ({:?}, {:?}, driver {} {})", info.name, info.device_type, info.backend, info.driver, info.driver_info);
        if info.device_type != wgpu::DeviceType::DiscreteGpu {
            eprintln!("[technoape] WARNING: adapter is not a discrete GPU");
        }
        let af = adapter.features();
        let want = wgpu::Features::INDIRECT_FIRST_INSTANCE | wgpu::Features::TIMESTAMP_QUERY;
        let feats = af & want;
        let mut limits = wgpu::Limits::default();
        let al = adapter.limits();
        limits.max_storage_buffer_binding_size = al.max_storage_buffer_binding_size;
        limits.max_buffer_size = al.max_buffer_size;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: None,
            required_features: feats,
            required_limits: limits,
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        }))
        .expect("device");
        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(caps.formats[0]);
        let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Immediate) {
            wgpu::PresentMode::Immediate
        } else {
            wgpu::PresentMode::AutoNoVsync
        };
        eprintln!("[technoape] surface {:?} present {:?} features {:?}", format, present_mode, feats);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            format,
            width: W,
            height: H,
            present_mode,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&device, &config);
        let buf = |label, size: u64, usage| {
            device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false })
        };
        use wgpu::BufferUsages as U;
        let globals = buf("globals", std::mem::size_of::<Globals>() as u64, U::UNIFORM | U::COPY_DST);
        let quads = buf("quads", cap as u64 * 8, U::STORAGE | U::COPY_DST);
        let chunks = buf("chunks", (MAX_CHUNKS * 16) as u64, U::STORAGE | U::COPY_DST);
        let indirect = buf("indirect", (MAX_CHUNKS * 16) as u64, U::INDIRECT | U::COPY_DST);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let storage = |b| wgpu::BindGroupLayoutEntry {
            binding: b,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                storage(1),
                storage(2),
            ],
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: quads.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: chunks.as_entire_binding() },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&bgl)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                targets: &[Some(format.into())],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState { cull_mode: Some(wgpu::Face::Back), ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Greater),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let depth = device
            .create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d { width: W, height: H, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&Default::default());
        let ts = feats.contains(wgpu::Features::TIMESTAMP_QUERY).then(|| {
            let qs = device.create_query_set(&wgpu::QuerySetDescriptor {
                label: None,
                ty: wgpu::QueryType::Timestamp,
                count: (TS_RING * 2) as u32,
            });
            let resolve = buf("ts-resolve", (TS_RING * 256) as u64, U::QUERY_RESOLVE | U::COPY_SRC);
            let ring = (0..TS_RING).map(|_| (buf("ts-read", 16, U::MAP_READ | U::COPY_DST), Arc::new(AtomicBool::new(false)), false)).collect();
            (qs, resolve, ring)
        });
        let capture = (a.get("capture_ms", 0) > 0).then(|| buf("capture", (W * H * 4) as u64, U::MAP_READ | U::COPY_DST));
        Gpu {
            window,
            ts_period: queue.get_timestamp_period(),
            device,
            queue,
            surface,
            config,
            pipeline,
            bind,
            globals,
            quads,
            chunks,
            indirect,
            depth,
            ts,
            capture,
            adapter_name: info.name.clone(),
            backend: format!("{:?}", info.backend),
        }
    }

    fn upload(&self, off: u32, q: &[[u32; 2]]) {
        self.queue.write_buffer(&self.quads, off as u64 * 8, bytemuck::cast_slice(q));
    }
}

#[derive(Default)]
struct Samples {
    frame_ms: Vec<f32>,
    cpu_ms: Vec<f32>,
    gpu_ms: Vec<f32>,
    draws: u64,
    quads: u64,
    frames: u64,
    missing_max: u32,
    holes_max: u32,
    applied: u64,
    applied_nonempty: u64,
    ram_max: f64,
    vram_max: f64,
    edits: u64,
    sel_ms: Vec<f32>,
    acq_ms: Vec<f32>,
    submit_ms: Vec<f32>,
    spikes: Vec<String>,
    capture_stalls: u32,
}

fn pct(v: &mut Vec<f32>, p: f64) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[((v.len() - 1) as f64 * p).round() as usize]
}

struct App {
    args: Args,
    gpu: Option<Gpu>,
    world: World,
    streamer: Streamer,
    run: String,
    start: Instant,
    loaded_at: Option<Instant>,
    last_frame: Instant,
    ts_slot: usize,
    samples: Samples,
    rng: u64,
    edits_done: u64,
    last_pos: DVec3,
    sheet: Vec<image::RgbaImage>,
    next_cap: f64,
    done: bool,
    cap_prev: bool,
}

impl App {
    fn frame(&mut self) {
        let a = &self.args;
        // quick=1: quarter-length runs and a 3 s warm-up, for iterating; never a final number
        let q: f64 = if a.get("quick", 0) == 1 { 0.25 } else { 1.0 };
        let warm: f64 = a.get("warmup", if q < 1.0 { 3.0 } else { 10.0 });
        let speed: f64 = a.get("speed", 60.0);
        let alt: f64 = a.get("alt", 20.0);
        let upload_mb: f64 = a.get("upload_mb", 16.0);
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f64();
        self.last_frame = now;
        let t = self.loaded_at.map(|l| (now - l).as_secs_f64()).unwrap_or(0.0);
        let measuring = self.loaded_at.is_some() && t >= warm;
        if t >= run_len(&self.run, warm, q) && self.loaded_at.is_some() {
            self.finish();
            return;
        }
        let cpu0 = Instant::now();
        let p = pose(&self.run, t, &self.world, speed, alt, warm, q, a);
        let vel = if dt > 0.0 && self.loaded_at.is_some() { (p.pos - self.last_pos) / dt } else { DVec3::ZERO };
        self.last_pos = p.pos;

        // edits: 2000 toggles/s within 10 m of the feet
        if self.run == "edit" && self.loaded_at.is_some() {
            let rate: f64 = a.get("edit_rate", 2000.0);
            let target = (t * rate) as u64;
            let feet = p.pos - p.up * 1.7;
            while self.edits_done < target {
                let mut r = || {
                    self.rng ^= self.rng << 13;
                    self.rng ^= self.rng >> 7;
                    self.rng ^= self.rng << 17;
                    (self.rng >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
                };
                let o = loop {
                    let o = DVec3::new(r(), r(), r());
                    if o.length_squared() <= 1.0 {
                        break o * 10.0;
                    }
                };
                let b = self.world.block_at(feet + o);
                if b.pos.z < 0 {
                    continue; // below layer 0 nothing is stored
                }
                let m = if self.world.block(b) == 0 { 3 } else { 0 };
                self.world.set_block(b, m);
                self.streamer.touch_block(&self.world, b);
                self.edits_done += 1;
                if measuring {
                    self.samples.edits += 1;
                }
            }
        }

        let gpu = self.gpu.as_ref().unwrap();
        let mut st = FrameStats::default();
        let budget = (upload_mb * 1e6) as usize;
        self.streamer.apply(budget, &mut st, |off, q| gpu.upload(off, q));

        let aspect = W as f32 / H as f32;
        let proj = glam::camera::rh::proj::directx::perspective_infinite_reverse(70f32.to_radians(), aspect, 0.1);
        let up = if p.fwd.dot(p.up).abs() > 0.98 { DVec3::Z } else { p.up };
        let viewm = glam::camera::rh::view::look_to_mat4(Vec3::ZERO, p.fwd.as_vec3(), up.as_vec3());
        let vp = proj * viewm;
        let view = View::new(p.pos, frustum_planes(vp), &self.world);
        let mut draws: Vec<DrawChunk> = Vec::new();
        let mut reqs = Vec::new();
        self.streamer.frame += 1;
        let ts0 = Instant::now();
        self.streamer.select(&self.world, &view, true, 1.0, &mut draws, &mut reqs, &mut st);
        let pf = self.streamer.opts.prefetch_s;
        // the look-ahead only requests chunks, so every `prefetch_every`th frame is enough
        let every: u64 = self.args.get("prefetch_every", 4);
        if pf > 0.0 && vel.length() > 1.0 {
            // between refreshes the last look-ahead requests stay in the queue, or the workers fill up with
            // low-priority hidden chunks and the look-ahead arrives late
            let mut ahead_reqs = std::mem::take(&mut self.streamer.ahead_reqs);
            if self.streamer.frame % every == 0 {
                ahead_reqs.clear();
                let ahead = View::new(p.pos + vel * pf, view.planes, &self.world);
                let mut sink = Vec::new();
                let mut st2 = FrameStats::default();
                self.streamer.select(&self.world, &ahead, false, 1.5, &mut sink, &mut ahead_reqs, &mut st2);
            }
            reqs.extend_from_slice(&ahead_reqs);
            self.streamer.ahead_reqs = ahead_reqs;
        }
        self.streamer.dispatch(&self.world, &mut reqs);
        let sel = ts0.elapsed().as_secs_f64();
        if self.streamer.frame % 30 == 0 {
            self.streamer.evict(120);
        }

        // build per-frame chunk table and indirect args
        let mut table: Vec<ChunkGpu> = Vec::with_capacity(draws.len());
        let mut args: Vec<DrawArgs> = Vec::with_capacity(draws.len());
        let mut nquads = 0u64;
        for (ci, d) in draws.iter().enumerate().take(MAX_CHUNKS) {
            let k = d.key;
            table.push(ChunkGpu { base: k.base().into(), info: k.face as u32 | (k.level as u32) << 3 | (k.body as u32) << 8 });
            args.push(DrawArgs { vertex_count: d.len * 6, instance_count: 1, first_vertex: d.off * 6, first_instance: ci as u32 });
            nquads += d.len as u64;
        }
        let sun = Vec3::new(0.4, 0.8, 0.45).normalize();
        let mut g = Globals { view_proj: vp.to_cols_array_2d(), sun: [sun.x, sun.y, sun.z, 0.0], bodies: bytemuck::Zeroable::zeroed() };
        for (b, o) in self.world.bodies.iter().zip(g.bodies.iter_mut()) {
            // the camera relative to the body in two floats per axis: one float is 0.1 mm coarse at 1 km
            let c = p.pos - b.centre;
            let hi = c.as_vec3();
            let lo = (c - hi.as_dvec3()).as_vec3();
            *o = BodyGpu {
                cam_hi: hi.into(),
                r0: b.r0 as f32,
                cam_lo: lo.into(),
                lnq: b.lnq as f32,
                theta: (std::f64::consts::FRAC_PI_2 / b.n as f64) as f32,
                half_n: b.n as f32 * 0.5,
                _p: [0.0; 2],
            };
        }
        gpu.queue.write_buffer(&gpu.globals, 0, bytemuck::bytes_of(&g));
        if !table.is_empty() {
            gpu.queue.write_buffer(&gpu.chunks, 0, bytemuck::cast_slice(&table));
            gpu.queue.write_buffer(&gpu.indirect, 0, bytemuck::cast_slice(&args));
        }

        // sky darkens with altitude
        let near = self.world.nearest(p.pos);
        let b = &self.world.bodies[near];
        let altm = ((p.pos - b.centre).length() - b.radius).max(0.0);
        let k = (1.0 - altm / 1500.0).clamp(0.0, 1.0);
        let sky = wgpu::Color { r: 0.45 * k + 0.01, g: 0.65 * k + 0.01, b: 0.95 * k + 0.03, a: 1.0 };

        let frame = match gpu.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            other => {
                eprintln!("[technoape] surface: {:?}", std::mem::discriminant(&other));
                gpu.surface.configure(&gpu.device, &gpu.config);
                return;
            }
        };
        let cpu_a = cpu0.elapsed().as_secs_f64();
        let acq = cpu_a;
        let cpu1 = Instant::now();
        let fview = frame.texture.create_view(&Default::default());
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        // timestamp slot: read a finished one, then reuse
        let mut ts_use = None;
        if let Some((_, _, ring)) = &gpu.ts {
            let i = self.ts_slot % TS_RING;
            let (rb, flag, pending) = &ring[i];
            if *pending {
                if flag.load(Ordering::Acquire) {
                    let data = rb.slice(..).get_mapped_range().unwrap();
                    let v: &[u64] = bytemuck::cast_slice(&data);
                    let ms = (v[1].wrapping_sub(v[0])) as f64 * gpu.ts_period as f64 / 1e6;
                    drop(data);
                    rb.unmap();
                    if measuring && ms < 1000.0 {
                        self.samples.gpu_ms.push(ms as f32);
                    }
                    ts_use = Some(i);
                }
            } else {
                ts_use = Some(i);
            }
        }
        {
            let tsw = ts_use.map(|i| wgpu::RenderPassTimestampWrites {
                query_set: &gpu.ts.as_ref().unwrap().0,
                beginning_of_pass_write_index: Some((i * 2) as u32),
                end_of_pass_write_index: Some((i * 2 + 1) as u32),
            });
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &fview,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(sky), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &gpu.depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
                timestamp_writes: tsw,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_pipeline(&gpu.pipeline);
            rp.set_bind_group(0, &gpu.bind, &[]);
            if self.args.get("mdi", 1) == 1 {
                rp.multi_draw_indirect(&gpu.indirect, 0, args.len() as u32);
            } else {
                for d in &args {
                    rp.draw(d.first_vertex..d.first_vertex + d.vertex_count, d.first_instance..d.first_instance + 1);
                }
            }
        }
        if let (Some(i), Some((qs, resolve, ring))) = (ts_use, &gpu.ts) {
            enc.resolve_query_set(qs, (i * 2) as u32..(i * 2 + 2) as u32, resolve, (i * 256) as u64);
            enc.copy_buffer_to_buffer(resolve, (i * 256) as u64, &ring[i].0, 0, 16);
        }
        let cap_now = gpu.capture.is_some() && self.loaded_at.is_some() && t >= self.next_cap;
        if cap_now {
            enc.copy_texture_to_buffer(
                frame.texture.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: gpu.capture.as_ref().unwrap(),
                    layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(W * 4), rows_per_image: Some(H) },
                },
                wgpu::Extent3d { width: W, height: H, depth_or_array_layers: 1 },
            );
        }
        let tsub = Instant::now();
        gpu.queue.submit([enc.finish()]);
        let sub = tsub.elapsed().as_secs_f64();
        if let (Some(i), Some((_, _, ring))) = (ts_use, &gpu.ts) {
            let f = ring[i].1.clone();
            f.store(false, Ordering::Release);
            ring[i].0.slice(..).map_async(wgpu::MapMode::Read, move |r| {
                if r.is_ok() {
                    f.store(true, Ordering::Release)
                }
            });
        }
        if let (Some(i), Some((_, _, ring))) = (ts_use, &mut self.gpu.as_mut().unwrap().ts) {
            ring[i].2 = true;
        }
        self.ts_slot += 1;
        let gpu = self.gpu.as_ref().unwrap();
        let _ = gpu.device.poll(wgpu::PollType::Poll);
        let cpu_b = cpu1.elapsed().as_secs_f64();
        gpu.window.pre_present_notify();
        let tpres = Instant::now();
        gpu.queue.present(frame);
        let pres = tpres.elapsed().as_secs_f64();
        if cap_now {
            self.capture_frame(t);
        }

        // a filmstrip grab stalls on a readback: keep that frame and the next out of the numbers
        let skip = cap_now || self.cap_prev;
        self.cap_prev = cap_now;
        if measuring && skip {
            self.samples.capture_stalls += 1;
        }
        if measuring && !skip {
            let s = &mut self.samples;
            // every frame over the 120 fps budget names where its time went (ADR 0004)
            if dt * 1000.0 > 8.3 && s.spikes.len() < 50 {
                s.spikes.push(format!(
                    "{{\"t\":{:.2},\"frame_ms\":{:.2},\"cpu_ms\":{:.2},\"select_ms\":{:.2},\"submit_ms\":{:.2},\"present_ms\":{:.2},\"chunks_applied\":{}}}",
                    t, dt * 1000.0, (cpu_a + cpu_b) * 1000.0, sel * 1000.0, sub * 1000.0, pres * 1000.0, st.applied
                ));
            }
            s.frame_ms.push((dt * 1000.0) as f32);
            s.cpu_ms.push(((cpu_a + cpu_b) * 1000.0) as f32);
            s.sel_ms.push((sel * 1000.0) as f32);
            s.submit_ms.push((sub * 1000.0) as f32);
            s.acq_ms.push((acq * 1000.0) as f32);
            s.draws += args.len() as u64;
            s.quads += nquads;
            s.frames += 1;
            s.missing_max = s.missing_max.max(st.missing);
            s.holes_max = s.holes_max.max(st.holes);
            s.applied += st.applied as u64;
            s.applied_nonempty += st.applied_nonempty as u64;
            if s.frames % 60 == 1 {
                s.ram_max = s.ram_max.max(sys::ram_mb());
                s.vram_max = s.vram_max.max(sys::vram_mb());
            }
        }
        if self.loaded_at.is_none() {
            let el = self.start.elapsed().as_secs_f64();
            if (self.streamer.idle() && st.missing == 0 && self.streamer.frame > 10) || el > 60.0 {
                eprintln!("[technoape] loaded in {:.2} s, {} chunks, {} quads", el, self.streamer.slot_count(), self.streamer.alloc.used);
                self.loaded_at = Some(Instant::now());
                self.last_pos = p.pos;
            }
        }
    }

    fn capture_frame(&mut self, t: f64) {
        let every: f64 = self.args.get("capture_ms", 0.0) / 1000.0;
        self.next_cap = t + every;
        let gpu = self.gpu.as_ref().unwrap();
        let buf = gpu.capture.as_ref().unwrap();
        buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        let data = buf.slice(..).get_mapped_range().unwrap();
        let bgra = matches!(gpu.config.format, wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb);
        let tw: u32 = self.args.get("capture_w", 384);
        let th = tw * H / W;
        let mut img = image::RgbaImage::new(tw, th);
        for y in 0..th {
            for x in 0..tw {
                let sx = x * W / tw;
                let sy = y * H / th;
                let i = ((sy * W + sx) * 4) as usize;
                let (r, g, b) = if bgra { (data[i + 2], data[i + 1], data[i]) } else { (data[i], data[i + 1], data[i + 2]) };
                img.put_pixel(x, y, image::Rgba([r, g, b, 255]));
            }
        }
        drop(data);
        buf.unmap();
        self.sheet.push(img);
    }

    fn finish(&mut self) {
        if self.done {
            return;
        }
        self.done = true;
        let gpu = self.gpu.as_ref().unwrap();
        let s = &mut self.samples;
        let secs: f64 = s.frame_ms.iter().map(|v| *v as f64).sum::<f64>() / 1000.0;
        let o = &self.streamer.opts;
        let a = &self.args;
        let job_us = self.streamer.job_us_total / self.streamer.jobs_done.max(1) as f64;
        let (p99, max) = (pct(&mut s.frame_ms, 0.99), pct(&mut s.frame_ms, 1.0));
        // ADR 0004; an unknown RAM or VRAM reading fails
        let budget_ok = p99 <= 8.3 && max <= 16.7 && s.ram_max > 0.0 && s.ram_max <= 8192.0 && s.vram_max > 0.0 && s.vram_max <= 6144.0;
        let line = format!(
            concat!(
                "{{\"run\":\"{}\",\"label\":\"{}\",\"quick\":{},\"planet_n\":{},\"planet_radius_m\":{:.0},\"budget_ok\":{},",
                "\"fps_avg\":{:.1},\"frame_p50_ms\":{:.3},\"frame_p99_ms\":{:.3},\"frame_max_ms\":{:.3},",
                "\"cpu_ms_p50\":{:.3},\"cpu_ms_p99\":{:.3},\"gpu_ms_p50\":{:.3},\"gpu_ms_p99\":{:.3},",
                "\"ram_mb\":{:.0},\"vram_mb\":{:.0},\"chunks_meshed_per_s\":{:.1},\"chunks_nonempty_per_s\":{:.1},",
                "\"missing_chunks_max\":{},\"holes_max\":{},\"draws_avg\":{:.0},\"quads_avg\":{:.0},\"quad_buffer_mb\":{:.1},",
                "\"select_ms_p50\":{:.3},\"submit_ms_p50\":{:.3},\"pre_acquire_ms_p50\":{:.3},\"edits_per_s\":{:.0},\"job_us_avg\":{:.0},",
                "\"frames\":{},\"capture_stalls\":{},\"speed\":{},\"k\":{},\"greedy\":{},\"horizon\":{},\"mdi\":{},\"prefetch_s\":{},",
                "\"threads\":{},\"alloc_fail\":{},\"backend\":\"{}\",\"adapter\":\"{}\",\"spikes\":[{}]}}"
            ),
            self.run,
            a.s("label", ""),
            a.get("quick", 0),
            self.world.bodies[0].n,
            self.world.bodies[0].radius,
            budget_ok,
            s.frames as f64 / secs.max(1e-9),
            pct(&mut s.frame_ms, 0.5),
            p99,
            max,
            pct(&mut s.cpu_ms, 0.5),
            pct(&mut s.cpu_ms, 0.99),
            pct(&mut s.gpu_ms, 0.5),
            pct(&mut s.gpu_ms, 0.99),
            s.ram_max,
            s.vram_max,
            s.applied as f64 / secs.max(1e-9),
            s.applied_nonempty as f64 / secs.max(1e-9),
            s.missing_max,
            s.holes_max,
            s.draws as f64 / s.frames.max(1) as f64,
            s.quads as f64 / s.frames.max(1) as f64,
            self.streamer.alloc.used as f64 * 8.0 / 1e6,
            pct(&mut s.sel_ms, 0.5),
            pct(&mut s.submit_ms, 0.5),
            pct(&mut s.acq_ms, 0.5),
            s.edits as f64 / secs.max(1e-9),
            job_us,
            s.frames,
            s.capture_stalls,
            a.get("speed", 60.0),
            o.k,
            o.greedy as u8,
            o.horizon as u8,
            a.get("mdi", 1),
            o.prefetch_s,
            rayon::current_num_threads(),
            self.streamer.alloc_fail,
            gpu.backend,
            gpu.adapter_name,
            s.spikes.join(",")
        );
        println!("{}", line);
        if let Some(path) = a.0.get("out") {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
            writeln!(f, "{}", line).unwrap();
        }
        if !self.sheet.is_empty() {
            let cols = 6.min(self.sheet.len() as u32);
            let rows = (self.sheet.len() as u32 + cols - 1) / cols;
            let (tw, th) = (self.sheet[0].width(), self.sheet[0].height());
            let mut big = image::RgbaImage::new(tw * cols, th * rows);
            for (i, im) in self.sheet.iter().enumerate() {
                image::imageops::replace(&mut big, im, ((i as u32 % cols) * tw) as i64, ((i as u32 / cols) * th) as i64);
            }
            let path = a.s("sheet", &format!("{}_strip.png", self.run));
            big.save(&path).unwrap();
            eprintln!("[technoape] filmstrip {} ({} frames)", path, self.sheet.len());
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        let onscreen = self.args.get("onscreen", 0) == 1;
        let attrs = Window::default_attributes()
            .with_title("TechnoApe")
            .with_inner_size(winit::dpi::PhysicalSize::new(W, H))
            .with_resizable(false)
            .with_decorations(false)
            .with_position(winit::dpi::PhysicalPosition::new(if onscreen { 0 } else { -30000 }, -30000));
        let window = Arc::new(el.create_window(attrs).unwrap());
        let cap: u32 = self.args.get("quad_cap_m", 16u32) * 1_000_000;
        self.gpu = Some(Gpu::new(window, &self.args, cap));
        self.start = Instant::now();
        self.last_frame = Instant::now();
    }
    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, ev: WindowEvent) {
        if let WindowEvent::CloseRequested = ev {
            el.exit();
        }
    }
    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        if self.gpu.is_none() {
            return;
        }
        if self.done {
            el.exit();
            return;
        }
        self.frame();
    }
}

fn main() {
    let args = Args(
        std::env::args()
            .skip(1)
            .filter_map(|a| a.split_once('=').map(|(k, v)| (k.to_string(), v.to_string())))
            .collect(),
    );
    let threads: usize = args.get("threads", std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8).saturating_sub(2).max(1));
    rayon::ThreadPoolBuilder::new().num_threads(threads).build_global().unwrap();
    let run = args.s("run", "stand");
    if run == "meshbench" {
        sys::meshbench(&args.0);
        return;
    }
    // columns per face edge: 3072 is the 1 km planet of the first playable, 1536 the benchmark's 500 m class
    let world = World::new(args.get("planet", 3072), args.get("moon", 512));
    assert!(world.bodies.len() <= MAX_BODIES);
    let opts = Opts {
        k: args.get("k", 4.0),
        // shortcut: no greedy merging, a merged quad is a chord across the curved grid and leaves cracks;
        // merge inside a lattice with interpolated corners when quad counts matter
        greedy: args.get("greedy", 0) == 1,
        horizon: args.get("horizon", 1) == 1,
        prefetch_s: args.get("prefetch", 0.5),
        max_inflight: args.get("inflight", threads * 4),
    };
    let cap: u32 = args.get("quad_cap_m", 16u32) * 1_000_000;
    let streamer = Streamer::new(&world, cap, opts);
    let el = EventLoop::new().unwrap();
    el.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        args,
        gpu: None,
        world,
        streamer,
        run,
        start: Instant::now(),
        loaded_at: None,
        last_frame: Instant::now(),
        ts_slot: 0,
        samples: Samples::default(),
        rng: 0x9E3779B97F4A7C15,
        edits_done: 0,
        last_pos: DVec3::ZERO,
        sheet: Vec::new(),
        next_cap: 0.0,
        done: false,
        cap_prev: false,
    };
    el.run_app(&mut app).unwrap();
}
