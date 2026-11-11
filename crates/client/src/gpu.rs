//! The GPU's side: the render size, the buffers' layouts, the lamps and the eclipse masks, and `Gpu`, the device with its pipelines (ADR 0025 L: moved out of main.rs).
use super::*;

/// The commit this was built from (build.rs).
pub(crate) const BUILD: &str = env!("TECHNOAPE_BUILD");
/// The render size (ADR 0023): 1920 x 1080 unless play's settings say otherwise; the settings change it.
pub(crate) static SIZE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1920 << 32 | 1080);
pub(crate) fn w() -> u32 {
    (SIZE.load(Ordering::Relaxed) >> 32) as u32
}
pub(crate) fn h() -> u32 {
    SIZE.load(Ordering::Relaxed) as u32
}
/// The depth buffer at the render size.
pub(crate) fn depth_view(device: &wgpu::Device) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width: w(), height: h(), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

/// Where a frame is copied to be read back: filmstrips, shots, the bug report's screenshot.
pub(crate) fn capture_buf(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("capture"),
        size: (row_bytes() * h()) as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// A frame's row in the capture buffer: four bytes a pixel, padded to wgpu's 256.
pub(crate) fn row_bytes() -> u32 {
    (w() * 4).div_ceil(256) * 256
}
pub(crate) const MAX_CHUNKS: usize = 1 << 16;
/// Quads of the water mesh the GPU holds (48 bytes each).
pub(crate) const MAX_WQUADS: usize = 1 << 18;
/// The sea's quads are drawn in SEA_SUB x SEA_SUB pieces (shader.wgsl vs_sea has the same number).
pub(crate) const SEA_SUB: u32 = 8;
pub(crate) const TS_RING: usize = 8;
/// Timestamps a frame writes (ADR 0025): before the ray-tracing builds, after them, after the G-buffer, after
/// the effects (ADR 0031), after `gi_trace`, after `temporal`, after the `atrous` passes, after the screen pass.
pub(crate) const TS_N: usize = 8;
/// The GPU's stages between those timestamps; `gap` is the GPU idle from the last frame's end to this one's start.
pub(crate) const GPU_STAGES: [&str; 9] = ["rt", "gbuffer", "fx", "trace", "temporal", "atrous", "screen", "total", "gap"];
/// The frame thread's stretches, back to back: together they are the frame (ADR 0025).
pub(crate) const CPU_STAGES: [&str; 16] =
    ["pose", "msgs", "edits", "apply", "select", "dispatch", "table", "fx", "acquire", "rt", "encode", "submit", "post", "present", "tail", "between"];
/// The particles the GPU holds (ADR 0031): the largest pool, "many".
pub(crate) const MAX_FX: usize = crate::fx::CAP[2] + crate::fx::CRACKS;
pub(crate) const MAX_BODIES: usize = 4; // the size of `bodies` in shader.wgsl
/// The full moon's light, a quarter of the sun's direct light and bluish: night stays readable (ADR 0022).
pub(crate) const MOON_LIGHT: Vec3 = Vec3::new(0.12, 0.14, 0.18);
/// The lamps the GPU holds: the nearest to the camera within LAMP_REACH metres (ADR 0022; 1024 since ADR 0025, H).
pub(crate) const MAX_LAMPS: usize = 1024;
pub(crate) const LAMP_REACH: f64 = 48.0;
/// The lamp grid (ADR 0025, H): LAMP_G^3 cells of LAMP_CELL metres in the camera's frame, the camera in the
/// middle, each a range into a list of the lamps that reach it: LAMP_R (as in shader.wgsl) or nearer, so a lamp
/// is listed in at most 5^3 cells.
pub(crate) const LAMP_G: usize = 32;
pub(crate) const LAMP_CELL: f32 = 4.0;
pub(crate) const LAMP_R: f32 = 8.0;
pub(crate) const LAMP_GRID_LEN: usize = LAMP_G * LAMP_G * LAMP_G + 1 + MAX_LAMPS * 125;

/// The blocks that give light within LAMP_REACH of the camera, nearest first: (body, middle, light).
/// `cams` is the camera in each body's coordinates.
pub(crate) fn gather_lamps(w: &World, cams: &[DVec3]) -> Vec<(u8, DVec3, [f32; 3])> {
    let mut out: Vec<(f64, u8, DVec3, [f32; 3])> = w
        .lights
        .iter()
        .filter_map(|(b, l)| {
            let x = w.block_centre(*b);
            let d = (x - cams[b.body as usize]).length();
            (d < LAMP_REACH).then_some((d, b.body, x, *l))
        })
        .collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.into_iter().take(MAX_LAMPS).map(|l| (l.1, l.2, l.3)).collect()
}

/// The lamp grid for the lamps at `lamps` (from the camera, in its frame): LAMP_G^3 + 1 offsets into the same
/// list, then each cell's lamps; cell c's are `[out[c], out[c + 1])`.
pub(crate) fn lamp_grid(lamps: &[LampGpu]) -> Vec<u32> {
    let (g, n) = (LAMP_G as i32, LAMP_G * LAMP_G * LAMP_G);
    let o = -(LAMP_G as f32) * LAMP_CELL * 0.5;
    let cells = |p: [f32; 4]| {
        let c = |x: f32| ((x - o) / LAMP_CELL).floor() as i32;
        let r = |a: usize| (c(p[a] - LAMP_R).max(0), c(p[a] + LAMP_R).min(g - 1));
        let (x, y, z) = (r(0), r(1), r(2));
        // how far the lamp is from cell i's span along one axis
        let d = move |a: f32, i: i32| {
            let lo = o + i as f32 * LAMP_CELL;
            (lo - a).max(a - lo - LAMP_CELL).max(0.0)
        };
        (z.0..=z.1)
            .flat_map(move |k| (y.0..=y.1).flat_map(move |j| (x.0..=x.1).map(move |i| (i, j, k))))
            .filter(move |&(i, j, k)| {
                let (dx, dy, dz) = (d(p[0], i), d(p[1], j), d(p[2], k));
                dx * dx + dy * dy + dz * dz < LAMP_R * LAMP_R
            })
            .map(move |(i, j, k)| ((k * g + j) * g + i) as usize)
    };
    let mut out = vec![0u32; n + 1];
    for l in lamps {
        for c in cells(l.pos) {
            out[c + 1] += 1;
        }
    }
    out[0] = n as u32 + 1;
    for c in 0..n {
        out[c + 1] += out[c];
    }
    let mut at = out[..n].to_vec();
    out.resize(out[n] as usize, 0);
    for (li, l) in lamps.iter().enumerate() {
        for c in cells(l.pos) {
            out[at[c] as usize] = li as u32;
            at[c] += 1;
        }
    }
    out
}

#[cfg(test)]
mod eclipse_tests {
    use super::*;
    // a body is in the mask whenever, from some point of another body's surface, its disc and the light's
    // overlap: sampled points against the masks, with the moon passing between the planet and the sun
    #[test]
    fn the_masks_hold_every_body_that_can_cover_a_light() {
        let mut g: Globals = bytemuck::Zeroable::zeroed();
        g.moon_col[3] = 1.0;
        g.sun_pos = [1.0e8, 0.0, 0.0, 7.0e5];
        g.bodies[0].ctr = [0.0, -4000.0, 0.0, 3900.0];
        let mut overlaps_seen = [0, 0];
        for k in 0..400 {
            let a = k as f64 / 400.0 * std::f64::consts::TAU;
            let m = DVec3::new(a.cos(), a.sin(), 0.3 * a.sin()) * 60_000.0 + DVec3::new(0.0, -4000.0, 0.0);
            g.bodies[1].ctr = [m.x as f32, m.y as f32, m.z as f32, 1000.0];
            let masks = eclipse_masks(&g);
            let v = |a: [f32; 4]| (DVec3::new(a[0] as f64, a[1] as f64, a[2] as f64), a[3] as f64);
            let lights = [v(g.sun_pos), v(g.bodies[1].ctr)];
            for (li, &(l, lr)) in lights.iter().enumerate() {
                for i in 0..2 {
                    if li == 1 && i == 1 {
                        continue;
                    }
                    let (c, r) = v(g.bodies[i].ctr);
                    let (cj, rj) = v(g.bodies[1 - i].ctr);
                    // points over the other body's surface, up to AMP above it
                    for s in 0..200 {
                        let (u, w) = (s as f64 * 0.618_034 % 1.0, s as f64 / 200.0);
                        let (th, z) = (u * std::f64::consts::TAU, 2.0 * w - 1.0);
                        let d = DVec3::new((1.0 - z * z).sqrt() * th.cos(), (1.0 - z * z).sqrt() * th.sin(), z);
                        let p = cj + d * (rj + engine::world::AMP * w);
                        let (ld, bv) = (l - p, c - p);
                        let sep = ld.angle_between(bv);
                        if sep < (lr / ld.length()).min(1.0).asin() + (r / bv.length()).min(1.0).asin() {
                            overlaps_seen[li] += 1;
                            assert_eq!(masks[li] >> i & 1, 1, "light {li} body {i} at step {k}");
                        }
                    }
                }
            }
        }
        assert!(overlaps_seen[0] > 0 && overlaps_seen[1] > 0, "the test saw no eclipse: {overlaps_seen:?}");
    }
}

#[cfg(test)]
mod lamp_grid_tests {
    use super::*;
    // every lamp is listed in exactly the cells within its reach, checked against the cells' corners directly
    #[test]
    fn a_cell_lists_the_lamps_that_reach_it() {
        let lamps: Vec<LampGpu> = [[0.3f32, -2.0, 5.0], [-63.0, 40.0, 7.9], [8.0, 8.0, 8.0], [200.0, 0.0, 0.0]]
            .iter()
            .map(|p| LampGpu { pos: [p[0], p[1], p[2], 0.0], col: [1.0; 4] })
            .collect();
        let grid = lamp_grid(&lamps);
        let g = LAMP_G;
        let o = -(g as f32) * LAMP_CELL * 0.5;
        for c in 0..g * g * g {
            let (i, j, k) = (c % g, c / g % g, c / (g * g));
            let lo = [o + i as f32 * LAMP_CELL, o + j as f32 * LAMP_CELL, o + k as f32 * LAMP_CELL];
            let listed = &grid[grid[c] as usize..grid[c + 1] as usize];
            for (li, l) in lamps.iter().enumerate() {
                let d2: f32 = (0..3).map(|a| (lo[a] - l.pos[a]).max(l.pos[a] - lo[a] - LAMP_CELL).max(0.0).powi(2)).sum();
                assert_eq!(listed.contains(&(li as u32)), d2 < LAMP_R * LAMP_R, "cell {c} lamp {li}");
            }
        }
        assert!(grid.len() <= LAMP_GRID_LEN);
    }
}

/// Which bodies can cover the sun's disc (bit i of the first) and the moon's (of the second) from some point of
/// another body's surface this frame (ADR 0025, J): `light()` runs `left_by` only for those, and where none can,
/// `left_by` would return exactly 1. Conservative: from anywhere within `AMP` and a margin of a body's radius,
/// the two directions are at least their angle from its centre less how far that ball turns each.
pub(crate) fn eclipse_masks(g: &Globals) -> [u32; 2] {
    let v = |a: [f32; 4]| (DVec3::new(a[0] as f64, a[1] as f64, a[2] as f64), a[3] as f64);
    let bodies: Vec<(DVec3, f64)> = g.bodies.iter().map(|b| v(b.ctr)).collect();
    let mi = g.moon_col[3] as usize;
    let lights = [v(g.sun_pos), bodies[mi]];
    let mut out = [0u32; 2];
    for (li, &(l, lr)) in lights.iter().enumerate() {
        for (i, &(c, r)) in bodies.iter().enumerate() {
            if r <= 0.0 || (li == 1 && i == mi) {
                continue;
            }
            let can = bodies.iter().enumerate().filter(|&(j, b)| j != i && b.1 > 0.0).any(|(_, &(cj, rj))| {
                let rho = rj + engine::world::AMP + 100.0;
                let (dl, dc) = (l - cj, c - cj);
                let (nl, nc) = (dl.length(), dc.length());
                if nl <= rho + lr || nc <= rho + r {
                    return true;
                }
                let apart = dl.angle_between(dc) - (rho / nl).asin() - (rho / nc).asin();
                apart < (lr / (nl - rho)).min(1.0).asin() + (r / (nc - rho)).min(1.0).asin()
            });
            out[li] |= (can as u32) << i;
        }
    }
    out
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct BodyGpu {
    pub(crate) cam_hi: [f32; 3],
    pub(crate) r0: f32,
    pub(crate) cam_lo: [f32; 3],
    pub(crate) lnq: f32,
    pub(crate) theta: f32,
    pub(crate) half_n: f32,
    pub(crate) _p: [f32; 2],
    /// The body's frame turned into the camera's (ADR 0022): columns.
    pub(crate) rot: [[f32; 4]; 3],
    /// Its centre from the camera in the camera's frame, and its radius (0: no body).
    pub(crate) ctr: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Globals {
    pub(crate) view_proj: [[f32; 4]; 4],
    pub(crate) sun: [f32; 4],
    pub(crate) bodies: [BodyGpu; MAX_BODIES],
    pub(crate) inv_vp: [[f32; 4]; 4],
    pub(crate) sun_pos: [f32; 4],
    pub(crate) sun_col: [f32; 4],
    pub(crate) moon_col: [f32; 4],
    pub(crate) sky: [f32; 4],
    pub(crate) n_lamps: [u32; 4],
    /// The camera's frame in the inertial one (the stars'), as columns; [0][3] the air here, [1][3] the sun's
    /// height over the horizon (sine).
    pub(crate) stars: [[f32; 4]; 3],
    /// x: the settings' graphics rows, a bit each (`App::gfx_bits`).
    pub(crate) opt: [u32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ChunkGpu {
    pub(crate) base: [i32; 3],
    pub(crate) info: u32,
    pub(crate) off: u32,
    pub(crate) _p: [u32; 3],
}

/// The chunk table's entries by mesh offset, kept between frames so a traced instance's index stays valid and
/// the TLAS is rebuilt only when its instances change (ADR 0025, B): the index and the frame last drawn.
#[derive(Default)]
pub(crate) struct ChunkSlots {
    pub(crate) at: std::collections::HashMap<u32, (u32, u64)>,
    pub(crate) free: Vec<u32>,
    pub(crate) next: u32,
}

impl ChunkSlots {
    /// The entry for the mesh at `off`, and whether it is new (its table entry must be written).
    pub(crate) fn get(&mut self, off: u32, frame: u64) -> (u32, bool) {
        if let Some(e) = self.at.get_mut(&off) {
            e.1 = frame;
            return (e.0, false);
        }
        let i = self.free.pop().unwrap_or_else(|| {
            self.next += 1;
            self.next - 1
        });
        self.at.insert(off, (i, frame));
        (i, true)
    }

    /// Entries not drawn this frame go back to the free list.
    pub(crate) fn sweep(&mut self, frame: u64) {
        let free = &mut self.free;
        self.at.retain(|_, e| {
            let keep = e.1 == frame;
            if !keep {
                free.push(e.0);
            }
            keep
        });
    }
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct LampGpu {
    pub(crate) pos: [f32; 4],
    pub(crate) col: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct DrawArgs {
    pub(crate) vertex_count: u32,
    pub(crate) instance_count: u32,
    pub(crate) first_vertex: u32,
    pub(crate) first_instance: u32,
}

pub(crate) struct Gpu {
    pub(crate) window: Arc<Window>,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) config: wgpu::SurfaceConfiguration,
    pub(crate) pipeline: wgpu::RenderPipeline,
    pub(crate) sky_pipeline: wgpu::RenderPipeline,
    /// The overlays (ADR 0017): ghost lines and the 2D layer, with their vertex and rectangle buffers.
    pub(crate) ghost_pipeline: wgpu::RenderPipeline,
    pub(crate) ghost_buf: wgpu::Buffer,
    pub(crate) ui_pipeline: wgpu::RenderPipeline,
    pub(crate) ui_buf: wgpu::Buffer,
    /// The water (ADR 0020): the sea's quads in the chunks, the water mesh from its own buffer.
    pub(crate) sea_pipeline: wgpu::RenderPipeline,
    pub(crate) water_pipeline: wgpu::RenderPipeline,
    pub(crate) wquads: wgpu::Buffer,
    /// The effects (ADR 0031): the particles, drawn into the G-buffer lit per pixel.
    pub(crate) fx_buf: wgpu::Buffer,
    pub(crate) fx_pipeline: wgpu::RenderPipeline,
    pub(crate) bind: wgpu::BindGroup,
    pub(crate) globals: wgpu::Buffer,
    pub(crate) quads: wgpu::Buffer,
    pub(crate) chunks: wgpu::Buffer,
    pub(crate) lamps: wgpu::Buffer,
    pub(crate) lamp_cells: wgpu::Buffer,
    pub(crate) indirect: wgpu::Buffer,
    pub(crate) depth: wgpu::TextureView,
    /// The terrain's shader and the block looks: the bounce light is built again from them on a resize.
    pub(crate) shader: wgpu::ShaderModule,
    pub(crate) looks: wgpu::Buffer,
    pub(crate) ts: Option<(wgpu::QuerySet, wgpu::Buffer, Vec<(wgpu::Buffer, Arc<AtomicBool>, bool)>)>,
    pub(crate) ts_period: f32,
    pub(crate) capture: Option<wgpu::Buffer>,
    pub(crate) adapter_name: String,
    pub(crate) backend: String,
}

impl Gpu {
    /// The GPU, and the traced terrain bound to it. Vulkan only: wgpu's ray queries are there (ADR 0022).
    pub(crate) fn new(window: Arc<Window>, a: &Args, cap: u32) -> (Gpu, rt::Rt, gi::Gi) {
        let mut idesc = wgpu::InstanceDescriptor::new_without_display_handle();
        idesc.backends = wgpu::Backends::VULKAN;
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
        // the light is traced: a GPU without hardware ray queries cannot run the game (ADR 0022)
        assert!(
            af.contains(wgpu::Features::EXPERIMENTAL_RAY_QUERY),
            "TechnoApe needs a GPU with hardware ray tracing (Vulkan ray queries); {} has none",
            info.name
        );
        let want = wgpu::Features::INDIRECT_FIRST_INSTANCE | wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
        let feats = (af & want) | wgpu::Features::EXPERIMENTAL_RAY_QUERY | wgpu::Features::RG11B10UFLOAT_RENDERABLE;
        let mut limits = wgpu::Limits::default().using_minimum_supported_acceleration_structure_values();
        let al = adapter.limits();
        limits.max_storage_buffer_binding_size = al.max_storage_buffer_binding_size;
        limits.max_buffer_size = al.max_buffer_size;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: None,
            required_features: feats,
            required_limits: limits,
            // SAFETY: wgpu gates ray queries as experimental; the client requires them and the gate's views and
            // filmstrips check what they draw
            experimental_features: unsafe { wgpu::ExperimentalFeatures::enabled() },
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
            width: w(),
            height: h(),
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
        let chunks = buf("chunks", (MAX_CHUNKS * std::mem::size_of::<ChunkGpu>()) as u64, U::STORAGE | U::COPY_DST);
        let lamps = buf("lamps", (MAX_LAMPS * std::mem::size_of::<LampGpu>()) as u64, U::STORAGE | U::COPY_DST);
        let lamp_cells = buf("lamp cells", (LAMP_GRID_LEN * 4) as u64, U::STORAGE | U::COPY_DST);
        // the terrain's draws, then the sea's at MAX_CHUNKS * 16
        let indirect = buf("indirect", (MAX_CHUNKS * 32) as u64, U::INDIRECT | U::COPY_DST);
        let wquads = buf("wquads", (MAX_WQUADS * std::mem::size_of::<water::WQuad>()) as u64, U::STORAGE | U::COPY_DST);
        let fx_buf = buf("fx", (MAX_FX * std::mem::size_of::<crate::fx::FxGpu>()) as u64, U::STORAGE | U::COPY_DST);
        // every block's look from the registry (ADR 0013, 0039), shader.wgsl `Look`, 64 bytes each: colour rgb,
        // pattern | side << 8 | metal << 16 | gives light << 24; colour2, size; colour3, amount; contrast
        let looks = buf("looks", 256 * 64, U::STORAGE | U::COPY_DST);
        let look: Vec<[u32; 16]> = (0..=255u8)
            .map(|id| {
                let f = f32::to_bits;
                match engine::blocks::BLOCKS.get(id) {
                    Some(d) => {
                        let ([r, g, b], [r2, g2, b2], [r3, g3, b3]) = (d.colour.map(f), d.colour2.map(f), d.colour3.map(f));
                        let shine = (d.metal * 255.0).round() as u32;
                        let gives = (d.light != [0.0; 3]) as u32;
                        let info = d.pattern as u32 | (d.side as u32) << 8 | shine << 16 | gives << 24;
                        [r, g, b, info, r2, g2, b2, d.size as u32, r3, g3, b3, f(d.amount), f(d.contrast), 0, 0, 0]
                    }
                    None => {
                        let h = f(0.5);
                        [h, h, h, (id as u32) << 8, h, h, h, 0, h, h, h, 0, f(1.0), 0, 0, 0]
                    }
                }
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
                storage(4),
                storage(8),
                storage(9),
                storage(10),
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::AccelerationStructure { vertex_return: false },
                    count: None,
                },
            ],
        });
        let rt = rt::Rt::new(&device, &shader, &globals, &quads, MAX_CHUNKS as u32);
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: quads.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: chunks.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: looks.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: wquads.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 5, resource: rt.tlas.as_binding() },
                wgpu::BindGroupEntry { binding: 8, resource: lamps.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 9, resource: lamp_cells.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 10, resource: fx_buf.as_entire_binding() },
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
                targets: &gi::TARGETS.map(|f| Some(f.into())),
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
        // the effects: into the G-buffer as the terrain, with its depth (ADR 0031)
        let fx_pipeline = {
            let fs = "fs_fx";
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(fs),
                layout: Some(&layout),
                vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_fx"), buffers: &[], compilation_options: Default::default() },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    targets: &gi::TARGETS.map(|f| Some(f.into())),
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
            })
        };
        // water: blended over the terrain, tested against its depth but not writing it, seen from both sides
        let liquid = |vs| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(vs),
                layout: Some(&layout),
                vertex: wgpu::VertexState { module: &shader, entry_point: Some(vs), buffers: &[], compilation_options: Default::default() },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_water"),
                    targets: &[Some(wgpu::ColorTargetState { format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::Greater),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        // the sky: first, behind everything, writing no depth
        let sky_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky"),
            layout: Some(&layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_sky"), buffers: &[], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_sky"),
                targets: &gi::TARGETS.map(|f| Some(f.into())),
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
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
        });
        let sea_pipeline = liquid("vs_sea");
        let water_pipeline = liquid("vs_water");
        let overlay = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("overlay"),
            source: wgpu::ShaderSource::Wgsl((include_str!("overlay.wgsl").to_string() + &hud::font_wgsl()).into()),
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
        let depth = depth_view(&device);
        // a timestamp between passes needs both features; without them no GPU times
        let ts = feats.contains(wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS).then(|| {
            let qs = device.create_query_set(&wgpu::QuerySetDescriptor {
                label: None,
                ty: wgpu::QueryType::Timestamp,
                count: (TS_RING * TS_N) as u32,
            });
            let resolve = buf("ts-resolve", (TS_RING * 256) as u64, U::QUERY_RESOLVE | U::COPY_SRC);
            let ring = (0..TS_RING).map(|_| (buf("ts-read", (TS_N * 8) as u64, U::MAP_READ | U::COPY_DST), Arc::new(AtomicBool::new(false)), false)).collect();
            (qs, resolve, ring)
        });
        // play keeps one for the bug report's screenshot (F9)
        let capture = (a.get("capture_ms", 0) > 0 || a.0.contains_key("shot") || a.s("run", "stand") == "play").then(|| capture_buf(&device));
        let gi = gi::Gi::new(
            &device,
            &shader,
            [
                globals.as_entire_binding(),
                quads.as_entire_binding(),
                chunks.as_entire_binding(),
                looks.as_entire_binding(),
                rt.tlas.as_binding(),
                lamps.as_entire_binding(),
                lamp_cells.as_entire_binding(),
            ],
            format,
            w(),
            h(),
        );
        let gpu = Gpu {
            window,
            sky_pipeline,
            ts_period: queue.get_timestamp_period(),
            device,
            queue,
            surface,
            config,
            pipeline,
            sea_pipeline,
            water_pipeline,
            wquads,
            fx_buf,
            fx_pipeline,
            ghost_pipeline,
            ghost_buf,
            ui_pipeline,
            ui_buf,
            bind,
            globals,
            quads,
            chunks,
            lamps,
            lamp_cells,
            indirect,
            depth,
            shader: shader.clone(),
            looks: looks.clone(),
            ts,
            capture,
            adapter_name: info.name.clone(),
            backend: format!("{:?}", info.backend),
        };
        (gpu, rt, gi)
    }

    pub(crate) fn upload(&self, off: u32, q: &[[u32; 2]]) {
        self.queue.write_buffer(&self.quads, off as u64 * 8, bytemuck::cast_slice(q));
    }
}
