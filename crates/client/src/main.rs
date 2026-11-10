//! The TechnoApe client: window, renderer and the benchmark runs (docs/plans/roadmap.md, "The shared benchmark").
mod hud;
mod sys;

use engine::blocks::{AIR, BLOCKS};
use engine::edit::{self, Kind, Predictor, Shape, Tool};
use engine::net::{self, Link, Recv, ToClient, ToServer, TICK_HZ};
use engine::phys::{self, Input};
use engine::stream::{DrawChunk, FrameStats, Opts, Streamer, View};
use engine::world::{Block, World};
use server::Server;
use glam::{DQuat, DVec3, Mat4, Vec3, Vec4};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{CursorGrabMode, Window, WindowId};

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

static LOW_WORKERS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

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
    /// The overlays (ADR 0017): ghost lines and the 2D layer, with their vertex and rectangle buffers.
    ghost_pipeline: wgpu::RenderPipeline,
    ghost_buf: wgpu::Buffer,
    ui_pipeline: wgpu::RenderPipeline,
    ui_buf: wgpu::Buffer,
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
        // every block's look from the registry (ADR 0013): colour rgb, then pattern | side << 8, 16 bytes each
        let looks = buf("looks", 256 * 16, U::STORAGE | U::COPY_DST);
        let look: Vec<[u32; 4]> = (0..=255u8)
            .map(|id| match engine::blocks::BLOCKS.get(id) {
                Some(d) => {
                    let [r, g, b] = d.colour.map(f32::to_bits);
                    [r, g, b, d.pattern as u32 | (d.side as u32) << 8]
                }
                None => [0.5f32.to_bits(), 0.5f32.to_bits(), 0.5f32.to_bits(), (id as u32) << 8],
            })
            .collect();
        queue.write_buffer(&looks, 0, bytemuck::cast_slice(&look));
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
                storage(3),
            ],
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: quads.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: chunks.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: looks.as_entire_binding() },
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
        let overlay = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("overlay"),
            source: wgpu::ShaderSource::Wgsl(include_str!("overlay.wgsl").into()),
        });
        // drawn over everything: no depth test, no depth write, blended
        let over = |label, vs, fs, buffer: wgpu::VertexBufferLayout, topology| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState { module: &overlay, entry_point: Some(vs), buffers: &[Some(buffer)], compilation_options: Default::default() },
                fragment: Some(wgpu::FragmentState {
                    module: &overlay,
                    entry_point: Some(fs),
                    targets: &[Some(wgpu::ColorTargetState { format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState { topology, ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::Always),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let ghost_attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Uint32, 2 => Unorm8x4];
        let ghost_pipeline = over(
            "ghost",
            "ghost_vs",
            "ghost_fs",
            wgpu::VertexBufferLayout { array_stride: 20, step_mode: wgpu::VertexStepMode::Vertex, attributes: &ghost_attrs },
            wgpu::PrimitiveTopology::LineList,
        );
        let ui_attrs = wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4, 2 => Uint32];
        let ui_pipeline = over(
            "ui",
            "ui_vs",
            "ui_fs",
            wgpu::VertexBufferLayout { array_stride: 36, step_mode: wgpu::VertexStepMode::Instance, attributes: &ui_attrs },
            wgpu::PrimitiveTopology::TriangleList,
        );
        let ghost_buf = buf("ghost", (hud::GHOST_MAX * 20) as u64, U::VERTEX | U::COPY_DST);
        let ui_buf = buf("ui", (hud::RECTS_MAX * 36) as u64, U::VERTEX | U::COPY_DST);
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
            ghost_pipeline,
            ghost_buf,
            ui_pipeline,
            ui_buf,
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

/// How far run=play aims a dig or place: half the server's reach, the distance a hand-held tool feels right at.
const AIM_REACH: f64 = 8.0;
const DIGITS: [KeyCode; 9] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
];

/// acts="2:c,3:R,3.5:r": (seconds after load, action), sorted so the soonest is last.
fn parse_acts(s: &str) -> Vec<(f64, String)> {
    let mut v: Vec<(f64, String)> =
        s.split(',').filter_map(|e| e.trim().split_once(':')).filter_map(|(t, a)| Some((t.parse().ok()?, a.to_string()))).collect();
    v.sort_by(|a, b| b.0.total_cmp(&a.0));
    v
}

struct App {
    args: Args,
    gpu: Option<Gpu>,
    world: World,
    link: Link<ToServer, ToClient>,
    my_id: u32,
    /// run=play: the player this client predicts, the keys held, mouse motion since the last frame, the look
    me: phys::Player,
    /// Where `me` was one physics step ago, and the time not yet stepped: the camera is drawn between them.
    prev: DVec3,
    debt: f64,
    drift_max: f64,
    keys: std::collections::HashSet<KeyCode>,
    mouse: (f64, f64),
    pitch: f64,
    input: Input,
    /// Times the server's position replaced the predicted one.
    snaps: u64,
    /// Seconds the last frame spent after present (stats, loading checks), and when it ended: both land in the
    /// next frame's time, outside its timed phases, so a spike reports them.
    tail_prev: f64,
    frame_end: Option<Instant>,
    last_move: f64,
    edits_applied: u64,
    players: usize,
    streamer: Streamer,
    run: String,
    start: Instant,
    loaded_at: Option<Instant>,
    last_frame: Instant,
    ts_slot: usize,
    samples: Samples,
    rng: u64,
    edits_done: u64,
    /// This client's edits in flight (ADR 0015).
    pred: Predictor,
    /// run=play building (ADR 0017): the hammer's tool, the aimed block and the cell before it, the button
    /// held (dig or not) with where it was pressed and where the drag is now, the hotbar slot.
    tool: Tool,
    /// The build hammer is in hand. shortcut: a key takes it up until items exist; the server does not yet
    /// check that a shape comes from a hammer.
    hammer: bool,
    aim: Option<(Block, Block)>,
    held: Option<(bool, Block, Block)>,
    slot: usize,
    creative: bool,
    inv: Vec<(u8, u32)>,
    /// What the overlay buffers hold: the ghost's shape and colour, its vertex count, the rectangle count.
    ghost_of: Option<(Shape, [u8; 4])>,
    ghost_n: u32,
    ui_n: u32,
    /// acts=: scripted input for checks without hands, as (seconds after load, action), soonest last.
    acts: Vec<(f64, String)>,
    title: String,
    last_pos: DVec3,
    sheet: Vec<image::RgbaImage>,
    next_cap: f64,
    done: bool,
    cap_prev: bool,
}

impl App {
    /// One input of run=play's building, from the keyboard and mouse or from acts=: L/R press and l/r release
    /// the dig and place buttons, h takes up or puts away the build hammer, q cycles its box, cylinder and
    /// cone, up/down change a cylinder's or cone's height, c switches creative, 1-9 pick a hotbar slot,
    /// m:dx:dy turns the look.
    fn act(&mut self, a: &str) {
        match a {
            "L" | "R" => {
                let dig = a == "L";
                if let Some(at) = self.aim.map(|(hit, before)| self.anchor(hit, before, dig)) {
                    self.held = Some((dig, at, at));
                }
            }
            "l" | "r" => {
                if let Some((dig, at, end)) = self.held.take().filter(|h| h.0 == (a == "l")) {
                    if let Some(s) = self.shape(dig, at, end) {
                        self.apply(&s, dig);
                    }
                }
            }
            "q" => {
                self.tool.kind = match self.tool.kind {
                    Kind::Box => Kind::Cylinder,
                    Kind::Cylinder => Kind::Cone,
                    Kind::Cone => Kind::Box,
                }
            }
            "c" => self.link.send(ToServer::Creative(!self.creative)),
            "up" | "down" => {
                self.tool.height = (self.tool.height + if a == "up" { 1 } else { -1 }).clamp(1, edit::MAX_SIDE);
            }
            "h" => self.hammer = !self.hammer,
            _ => {
                if let Ok(d) = a.parse::<usize>() {
                    self.slot = d.clamp(1, 9) - 1;
                } else if let Some((x, y)) = a.strip_prefix("m:").and_then(|r| r.split_once(':')) {
                    self.mouse.0 += x.parse().unwrap_or(0.0);
                    self.mouse.1 += y.parse().unwrap_or(0.0);
                } else {
                    eprintln!("[technoape] unknown act {a:?}");
                }
            }
        }
    }

    /// Where a press starts: digging takes the aimed block, placing the cell in front of it.
    fn anchor(&self, hit: Block, before: Block, dig: bool) -> Block {
        if dig { hit } else { before }
    }

    /// What a press at `at` released at `end` does (ADR 0017): bare hands dig and place one block where the
    /// release is; the build hammer places the tool's shape over the drag. shortcut: digging stays one block
    /// until mining tools come.
    fn shape(&self, dig: bool, at: Block, end: Block) -> Option<Shape> {
        if dig || !self.hammer {
            return Some(Shape::one(end));
        }
        self.tool.shape(&self.world, at, end, false)
    }

    /// The hotbar: every placeable block in id order.
    fn slot_block(&self, k: usize) -> Option<u8> {
        BLOCKS.placeable().nth(k).map(|d| d.id)
    }

    fn held_count(&self, m: u8) -> u32 {
        self.inv.iter().find(|(id, _)| *id == m).map_or(0, |(_, n)| *n)
    }

    /// Digs or places a shape: predicted here at once (ADR 0015), sent to the server. A survival place this
    /// player cannot pay for is not sent, so it never flickers in and out.
    fn apply(&mut self, s: &Shape, dig: bool) {
        let m = if dig {
            AIR
        } else {
            match self.slot_block(self.slot) {
                Some(m) => m,
                None => return,
            }
        };
        if !s.fits(&self.world) {
            return;
        }
        if !dig && !self.creative {
            let me = &self.me;
            let need = edit::changes(&self.world, s, m, |c| phys::holds(&self.world, me, c)).len() as u32;
            if need > self.held_count(m) {
                return;
            }
        }
        let me = &self.me;
        let (seq, cells) = self.pred.edit(&mut self.world, s, m, |w, c| phys::holds(w, me, c));
        for c in cells {
            self.streamer.touch_block(&self.world, c);
        }
        self.link.send(ToServer::Edit { seq, shape: *s, material: m });
    }

    /// Each frame of run=play: scripted acts that are due, the aim, the ghost and the 2D layer.
    fn build(&mut self, p: &Pose, t: f64) {
        while self.acts.last().is_some_and(|(at, _)| *at <= t) {
            let (_, a) = self.acts.pop().unwrap();
            self.act(&a);
        }
        self.aim = edit::aim(&self.world, p.pos, p.fwd, AIM_REACH);
        // the ghost: red what a held dig takes, green what a held place fills; when nothing is held, white
        // around the aimed block, or with the hammer where its shape would go
        let ghost = match (self.held, self.aim) {
            (Some((dig, at, _)), Some((hit, before))) => {
                let end = self.anchor(hit, before, dig);
                self.held = Some((dig, at, end));
                self.shape(dig, at, end).map(|s| (s, dig, true))
            }
            (Some((dig, at, end)), None) => self.shape(dig, at, end).map(|s| (s, dig, true)),
            (None, Some((_, before))) if self.hammer => self.shape(false, before, before).map(|s| (s, false, false)),
            (None, Some((hit, _))) => Some((Shape::one(hit), true, false)),
            (None, None) => None,
        };
        let ghost = ghost.filter(|(s, ..)| s.fits(&self.world)).map(|(s, dig, held)| {
            let colour = match (held, dig) {
                (false, _) => [255, 255, 255, 150],
                (true, true) => [255, 90, 60, 230],
                (true, false) => [110, 255, 120, 230],
            };
            (s, colour)
        });
        let gpu = self.gpu.as_ref().unwrap();
        if ghost != self.ghost_of {
            let verts = ghost.map(|(s, c)| hud::ghost(&self.world, &s, c)).unwrap_or_default();
            gpu.queue.write_buffer(&gpu.ghost_buf, 0, bytemuck::cast_slice(&verts));
            (self.ghost_n, self.ghost_of) = (verts.len() as u32, ghost);
        }
        let slots: Vec<hud::Slot> = (0..9)
            .filter_map(|k| self.slot_block(k))
            .map(|m| hud::Slot { colour: BLOCKS.get(m).unwrap().colour, count: (!self.creative).then(|| self.held_count(m)) })
            .collect();
        let height = (self.hammer && self.tool.kind != Kind::Box).then_some(self.tool.height);
        let rects = hud::layer(W as f32, H as f32, &slots, self.slot, self.creative, height);
        let n = rects.len().min(hud::RECTS_MAX);
        gpu.queue.write_buffer(&gpu.ui_buf, 0, bytemuck::cast_slice(&rects[..n]));
        self.ui_n = n as u32;
        // shortcut: the tool and mode as words in the window title, until the HUD has text
        let block = self.slot_block(self.slot).and_then(|m| BLOCKS.get(m)).map_or("-".to_string(), |d| d.name.clone());
        let tool = match (self.hammer, self.tool.kind) {
            (false, _) => "hands".to_string(),
            (true, Kind::Box) => "build hammer: box".to_string(),
            (true, k) => format!("build hammer: {} {} high", format!("{k:?}").to_lowercase(), self.tool.height),
        };
        let title = format!("TechnoApe | {} | {tool} | {block}", if self.creative { "creative" } else { "survival" });
        if title != self.title {
            gpu.window.set_title(&title);
            self.title = title;
        }
    }

    fn frame(&mut self) {
        let a = &self.args;
        // length= scales a run (the benchmark uses 0.5); quick=1 is 0.25, for iterating, never a final number
        let q: f64 = a.get("length", if a.get("quick", 0) == 1 { 0.25 } else { 1.0 });
        let warm: f64 = a.get("warmup", (10.0 * q).max(3.0));
        let speed: f64 = a.get("speed", 60.0);
        let alt: f64 = a.get("alt", 20.0);
        let upload_mb: f64 = a.get("upload_mb", 16.0);
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f64();
        let between = self.frame_end.map_or(0.0, |e| (now - e).as_secs_f64());
        self.last_frame = now;
        let t = self.loaded_at.map(|l| (now - l).as_secs_f64()).unwrap_or(0.0);
        let measuring = self.loaded_at.is_some() && t >= warm;
        let len = if self.run == "play" { a.get("seconds", f64::INFINITY) } else { run_len(&self.run, warm, q) };
        if t >= len && self.loaded_at.is_some() {
            self.finish();
            return;
        }
        let cpu0 = Instant::now();
        let p = if self.run == "play" {
            // the player: mouse turns the look, keys make the input, the shared physics moves the box here in
            // the same fixed steps as on the server, whose position wins when the two drift apart
            let up = phys::up(&self.world, self.me.pos);
            let (mx, my) = std::mem::take(&mut self.mouse);
            let sens = 0.0025;
            self.me.fwd = DQuat::from_axis_angle(up, -mx * sens) * self.me.fwd;
            self.pitch = (self.pitch - my * sens).clamp(-1.55, 1.55);
            let k = |c: KeyCode| self.keys.contains(&c) as i32 as f32;
            let auto = a.get("autowalk", 0) == 1;
            let inp = Input {
                mv: [if auto { 1.0 } else { k(KeyCode::KeyW) - k(KeyCode::KeyS) }, k(KeyCode::KeyD) - k(KeyCode::KeyA)],
                jump: auto || self.keys.contains(&KeyCode::Space),
                fwd: self.me.fwd.as_vec3().to_array(),
                pitch: self.pitch as f32,
            };
            self.debt += dt.min(0.1);
            while self.debt >= phys::DT {
                self.prev = self.me.pos;
                phys::step(&self.world, &mut self.me, &inp, phys::DT);
                self.debt -= phys::DT;
            }
            self.input = inp;
            // the look turns every frame; the position is drawn between the last two physics steps
            let pos = self.prev.lerp(self.me.pos, self.debt / phys::DT);
            let up = phys::up(&self.world, pos);
            let fwd = phys::tangent(self.me.fwd, up, self.me.fwd);
            let look = DQuat::from_axis_angle(fwd.cross(up), self.pitch) * fwd;
            Pose { pos: pos + up * phys::EYE, fwd: look, up }
        } else {
            pose(&self.run, t, &self.world, speed, alt, warm, q, a)
        };
        if self.run == "play" && self.loaded_at.is_some() {
            self.build(&p, t);
        }
        let vel = if dt > 0.0 && self.loaded_at.is_some() { (p.pos - self.last_pos) / dt } else { DVec3::ZERO };
        self.last_pos = p.pos;

        // the server's deltas: edits land in the world and remesh their chunks here, never directly
        loop {
            match self.link.try_recv() {
                Recv::Msg(ToClient::Edits(v)) => {
                    for (b, m) in v {
                        if self.pred.server(&mut self.world, b, m) {
                            self.streamer.touch_block(&self.world, b);
                        }
                        self.edits_applied += 1;
                        if measuring {
                            self.samples.edits += 1;
                        }
                    }
                }
                Recv::Msg(ToClient::Done { seq, .. }) => {
                    for b in self.pred.done(&mut self.world, seq) {
                        self.streamer.touch_block(&self.world, b);
                    }
                }
                Recv::Msg(ToClient::Players(ps)) => {
                    self.players = ps.len();
                    if self.run == "play" {
                        if let Some(sp) = ps.iter().find(|p| p.id == self.my_id).map(|p| DVec3::from_array(p.pos)) {
                            // shortcut: snap when the server disagrees by over a metre; replaying inputs
                            // from the acknowledged tick comes when real latency makes this visible
                            self.drift_max = self.drift_max.max(sp.distance(self.me.pos));
                            if sp.distance(self.me.pos) > 1.0 {
                                (self.me.pos, self.prev) = (sp, sp);
                                self.snaps += 1;
                                self.me.vel = DVec3::ZERO;
                            }
                        }
                    }
                }
                Recv::Msg(ToClient::Inventory { creative, counts }) => (self.creative, self.inv) = (creative, counts),
                Recv::Msg(ToClient::Welcome { .. }) => {}
                Recv::Empty => break,
                Recv::Closed => {
                    eprintln!("[technoape] the server closed the connection");
                    self.finish();
                    return;
                }
            }
        }
        if t - self.last_move >= 1.0 / TICK_HZ {
            if self.run == "play" {
                self.link.send(ToServer::Input(self.input));
            } else {
                // the benchmark runs fly: the camera position is sent as the player's position
                self.link.send(ToServer::Move { pos: p.pos.to_array(), fwd: p.fwd.as_vec3().to_array(), pitch: 0.0 });
            }
            self.last_move = t;
        }

        // edits: 2000 toggles/s within 10 m of the feet, sent to the server; they count when its delta is back
        if self.run == "edit" && self.loaded_at.is_some() {
            let rate: f64 = self.args.get("edit_rate", 2000.0);
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
                let m = if self.world.block(b) == AIR { BLOCKS.id("stone") } else { AIR };
                let me = &self.me;
                let (seq, cells) = self.pred.edit(&mut self.world, &Shape::one(b), m, |w, c| phys::holds(w, me, c));
                for c in cells {
                    self.streamer.touch_block(&self.world, c);
                }
                self.link.send(ToServer::Edit { seq, shape: Shape::one(b), material: m });
                self.edits_done += 1;
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
            if self.ghost_n > 0 {
                rp.set_pipeline(&gpu.ghost_pipeline);
                rp.set_vertex_buffer(0, gpu.ghost_buf.slice(..));
                rp.draw(0..self.ghost_n, 0..1);
            }
            if self.ui_n > 0 {
                rp.set_pipeline(&gpu.ui_pipeline);
                rp.set_vertex_buffer(0, gpu.ui_buf.slice(..));
                rp.draw(0..6, 0..self.ui_n);
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
        let tail0 = Instant::now();
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
                    "{{\"t\":{:.2},\"frame_ms\":{:.2},\"cpu_ms\":{:.2},\"select_ms\":{:.2},\"submit_ms\":{:.2},\"present_ms\":{:.2},\"prev_tail_ms\":{:.2},\"between_ms\":{:.2},\"chunks_applied\":{}}}",
                    t, dt * 1000.0, (cpu_a + cpu_b) * 1000.0, sel * 1000.0, sub * 1000.0, pres * 1000.0, self.tail_prev * 1000.0, between * 1000.0, st.applied
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
        let end = Instant::now();
        self.tail_prev = (end - tail0).as_secs_f64();
        self.frame_end = Some(end);
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
                "\"edits_sent\":{},\"edits_applied\":{},\"players\":{},\"feet\":[{:.2},{:.2},{:.2}],\"snaps\":{},\"drift_max_m\":{:.2},\"frames\":{},\"capture_stalls\":{},\"speed\":{},\"k\":{},\"greedy\":{},\"horizon\":{},\"mdi\":{},\"prefetch_s\":{},",
                "\"threads\":{},\"low_workers\":{},\"alloc_fail\":{},\"backend\":\"{}\",\"adapter\":\"{}\",\"spikes\":[{}]}}"
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
            self.edits_done,
            self.edits_applied,
            self.players,
            self.me.pos.x,
            self.me.pos.y,
            self.me.pos.z,
            self.snaps,
            self.drift_max,
            s.frames,
            s.capture_stalls,
            a.get("speed", 60.0),
            o.k,
            o.greedy as u8,
            o.horizon as u8,
            a.get("mdi", 1),
            o.prefetch_s,
            rayon::current_num_threads(),
            LOW_WORKERS.load(std::sync::atomic::Ordering::Relaxed),
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
        if self.run == "play" && onscreen {
            let _ = window.set_cursor_grab(CursorGrabMode::Confined).or_else(|_| window.set_cursor_grab(CursorGrabMode::Locked));
            window.set_cursor_visible(false);
        }
        let cap: u32 = self.args.get("quad_cap_m", 16u32) * 1_000_000;
        self.gpu = Some(Gpu::new(window, &self.args, cap));
        self.start = Instant::now();
        self.last_frame = Instant::now();
    }
    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, ev: WindowEvent) {
        match ev {
            WindowEvent::CloseRequested => {
                self.finish();
                el.exit();
            }
            WindowEvent::KeyboardInput { event: KeyEvent { physical_key: PhysicalKey::Code(code), state, .. }, .. } => {
                if code == KeyCode::Escape {
                    self.finish();
                    el.exit();
                } else if state == ElementState::Pressed {
                    let key = match code {
                        KeyCode::KeyQ => Some("q".to_string()),
                        KeyCode::KeyH => Some("h".to_string()),
                        KeyCode::KeyC => Some("c".to_string()),
                        _ => (1..=9).find(|d| code == DIGITS[d - 1]).map(|d| d.to_string()),
                    };
                    if let (Some(k), false) = (key, self.keys.contains(&code)) {
                        self.act(&k);
                    }
                    self.keys.insert(code);
                } else {
                    self.keys.remove(&code);
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let k = match button {
                    MouseButton::Left => "L",
                    MouseButton::Right => "R",
                    _ => return,
                };
                self.act(&if state == ElementState::Pressed { k.to_string() } else { k.to_lowercase() });
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y as f64,
                    MouseScrollDelta::PixelDelta(p) => p.y / 40.0,
                };
                if y != 0.0 {
                    self.act(if y > 0.0 { "up" } else { "down" });
                }
            }
            _ => {}
        }
    }
    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, ev: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = ev {
            self.mouse.0 += delta.0;
            self.mouse.1 += delta.1;
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

/// mimalloc: the Windows heap is slow under many threads (speed-lever spike, ADR 0012).
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    let args = Args(
        std::env::args()
            .skip(1)
            .filter_map(|a| a.split_once('=').map(|(k, v)| (k.to_string(), v.to_string())))
            .collect(),
    );
    let threads: usize = args.get("threads", std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8).saturating_sub(2).max(1));
    // workers below normal priority can never preempt the frame thread (speed-lever spike, ADR 0012)
    let low = args.get("low_workers", 1) == 1;
    LOW_WORKERS.store(low, std::sync::atomic::Ordering::Relaxed);
    rayon::ThreadPoolBuilder::new().num_threads(threads).start_handler(move |_| if low { sys::below_normal() }).build_global().unwrap();
    let run = args.s("run", "stand");
    if run == "meshbench" {
        sys::meshbench(&args.0);
        return;
    }
    // the world comes from the server: a remote one (connect=host[:port]) or a local one on its own thread
    // (planet= and moon= are columns per face edge: 3072 is the 1 km planet, 1536 the benchmark's 500 m class)
    let mut local_server = None;
    let link: Link<ToServer, ToClient> = match args.0.get("connect") {
        Some(addr) => {
            let addr = if addr.contains(':') { addr.clone() } else { format!("{addr}:{}", net::DEFAULT_PORT) };
            net::tcp(std::net::TcpStream::connect(&addr).expect("connect to the server")).expect("link")
        }
        None => {
            let (accept_tx, accept_rx) = std::sync::mpsc::channel();
            let (planet, moon, seed) = (args.get("planet", 3072), args.get("moon", 512), args.get("seed", 1337));
            // the flight recorder (ADR 0011): on for play on screen, or wherever record= says
            let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
            let rec = args.0.get("record").cloned().or((args.get("onscreen", 0) == 1).then(|| format!("out/rec/play-{unix}.rec")));
            // saves (ADR 0016): play keeps one per seed and planet under saves/, save= names another, save=0 none
            let save = args.0.get("save").cloned().or((run == "play").then(|| format!("saves/seed-{seed}-planet-{planet}.sav"))).filter(|s| s != "0");
            let mut server = match save.as_deref().map(std::path::Path::new).filter(|p| p.exists()) {
                Some(p) => {
                    // a save this build cannot read stops the game rather than being overwritten
                    let s = server::save::Save::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
                    eprintln!("[technoape] carrying on from {} ({} edited cells)", p.display(), s.edits.len());
                    Server::from_save(s, accept_rx)
                }
                None => Server::new(planet, moon, seed, accept_rx),
            };
            server.autosave = save.map(|p| (p.into(), 30 * TICK_HZ as u64));
            if let Some(r) = rec.filter(|r| r != "0") {
                match server.record(std::path::Path::new(&r)) {
                    Ok(()) => eprintln!("[technoape] recording to {r}"),
                    Err(e) => eprintln!("[technoape] not recording: {e}"),
                }
            }
            let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let st = stop.clone();
            local_server = Some((stop, std::thread::spawn(move || server.run(TICK_HZ, &st))));
            let (c, s) = net::local();
            accept_tx.send(s).unwrap();
            c
        }
    };
    link.send(ToServer::Hello { name: args.s("name", "player") });
    if run != "play" {
        link.send(ToServer::Creative(true)); // the edit benchmark places stone it never dug
    }
    let (my_id, world) = match link.rx.recv_timeout(Duration::from_secs(10)) {
        Ok(ToClient::Welcome { id, seed, planet_n, moon_n, blocks }) => {
            assert_eq!(blocks, engine::blocks::BLOCKS.hash, "the server's build has other blocks (data/blocks.toml)");
            (id, World::new(planet_n, moon_n, seed))
        }
        other => panic!("no welcome from the server: {other:?}"),
    };
    // the server spawns players on the +y pole; the client starts its prediction there too
    let spawn = DVec3::Y * (world.bodies[0].height(DVec3::Y) + 0.6);
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
    let (acts, creative) = (parse_acts(&args.s("acts", "")), run != "play");
    let mut app = App {
        args,
        gpu: None,
        world,
        link,
        my_id,
        me: phys::Player::at(spawn, DVec3::X),
        prev: spawn,
        debt: 0.0,
        drift_max: 0.0,
        keys: Default::default(),
        mouse: (0.0, 0.0),
        pitch: 0.0,
        input: Input::default(),
        snaps: 0,
        tail_prev: 0.0,
        frame_end: None,
        last_move: 0.0,
        edits_applied: 0,
        players: 0,
        streamer,
        run,
        start: Instant::now(),
        loaded_at: None,
        last_frame: Instant::now(),
        ts_slot: 0,
        samples: Samples::default(),
        rng: 0x9E3779B97F4A7C15,
        edits_done: 0,
        pred: Predictor::default(),
        tool: Tool { kind: Kind::Box, single: true, height: 8 },
        hammer: false,
        aim: None,
        held: None,
        slot: 0,
        creative,
        inv: Vec::new(),
        ghost_of: None,
        ghost_n: 0,
        ui_n: 0,
        acts,
        title: String::new(),
        last_pos: DVec3::ZERO,
        sheet: Vec::new(),
        next_cap: 0.0,
        done: false,
        cap_prev: false,
    };
    el.run_app(&mut app).unwrap();
    // the local server saves as it stops
    if let Some((stop, thread)) = local_server {
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = thread.join();
    }
}
