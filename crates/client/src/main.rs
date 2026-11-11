//! The TechnoApe client: window, renderer and the benchmark runs (docs/plans/roadmap.md, "The shared benchmark").
mod gi;
mod hud;
mod rt;
mod sys;
mod water;

use engine::blocks::{AIR, BLOCKS};
use engine::edit::{self, Kind, Predictor, Shape, Tool};
use engine::net::{self, Link, Recv, ToClient, ToServer, TICK_HZ};
use engine::phys::{self, Input};
use engine::stream::{DrawChunk, FrameStats, Opts, Streamer, View};
use engine::world::{Block, World};
use server::Server;
use glam::{DQuat, DVec3, IVec3, Mat4, Vec3, Vec4};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{CursorGrabMode, Window, WindowId};

/// The commit this was built from (build.rs).
const BUILD: &str = env!("TECHNOAPE_BUILD");
/// The render size (ADR 0023): 1920 x 1080 unless play's settings say otherwise; the settings change it.
static SIZE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1920 << 32 | 1080);
fn w() -> u32 {
    (SIZE.load(Ordering::Relaxed) >> 32) as u32
}
fn h() -> u32 {
    SIZE.load(Ordering::Relaxed) as u32
}
/// The depth buffer at the render size.
fn depth_view(device: &wgpu::Device) -> wgpu::TextureView {
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
fn capture_buf(device: &wgpu::Device) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("capture"),
        size: (row_bytes() * h()) as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// A frame's row in the capture buffer: four bytes a pixel, padded to wgpu's 256.
fn row_bytes() -> u32 {
    (w() * 4).div_ceil(256) * 256
}
const MAX_CHUNKS: usize = 1 << 16;
/// Quads of the water mesh the GPU holds (48 bytes each).
const MAX_WQUADS: usize = 1 << 18;
/// The sea's quads are drawn in SEA_SUB x SEA_SUB pieces (shader.wgsl vs_sea has the same number).
const SEA_SUB: u32 = 8;
const TS_RING: usize = 8;
/// Timestamps a frame writes (ADR 0025): before the ray-tracing builds, after them, after the G-buffer, after
/// `gi_trace`, after `temporal`, after the `atrous` passes, after the screen pass.
const TS_N: usize = 7;
/// The GPU's stages between those timestamps; `gap` is the GPU idle from the last frame's end to this one's start.
const GPU_STAGES: [&str; 8] = ["rt", "gbuffer", "trace", "temporal", "atrous", "screen", "total", "gap"];
/// The frame thread's stretches, back to back: together they are the frame (ADR 0025).
const CPU_STAGES: [&str; 15] =
    ["pose", "msgs", "edits", "apply", "select", "dispatch", "table", "acquire", "rt", "encode", "submit", "post", "present", "tail", "between"];
const MAX_BODIES: usize = 4; // the size of `bodies` in shader.wgsl
/// The full moon's light, a quarter of the sun's direct light and bluish: night stays readable (ADR 0022).
const MOON_LIGHT: Vec3 = Vec3::new(0.12, 0.14, 0.18);
/// The lamps the GPU holds: the nearest to the camera within LAMP_REACH metres (ADR 0022; 1024 since ADR 0025, H).
const MAX_LAMPS: usize = 1024;
const LAMP_REACH: f64 = 48.0;
/// The lamp grid (ADR 0025, H): LAMP_G^3 cells of LAMP_CELL metres in the camera's frame, the camera in the
/// middle, each a range into a list of the lamps that reach it: LAMP_R (as in shader.wgsl) or nearer, so a lamp
/// is listed in at most 5^3 cells.
const LAMP_G: usize = 32;
const LAMP_CELL: f32 = 4.0;
const LAMP_R: f32 = 8.0;
const LAMP_GRID_LEN: usize = LAMP_G * LAMP_G * LAMP_G + 1 + MAX_LAMPS * 125;

/// The blocks that give light within LAMP_REACH of the camera, nearest first: (body, middle, light).
/// `cams` is the camera in each body's coordinates.
fn gather_lamps(w: &World, cams: &[DVec3]) -> Vec<(u8, DVec3, [f32; 3])> {
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
fn lamp_grid(lamps: &[LampGpu]) -> Vec<u32> {
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
fn eclipse_masks(g: &Globals) -> [u32; 2] {
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
    /// The body's frame turned into the camera's (ADR 0022): columns.
    rot: [[f32; 4]; 3],
    /// Its centre from the camera in the camera's frame, and its radius (0: no body).
    ctr: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    sun: [f32; 4],
    bodies: [BodyGpu; MAX_BODIES],
    inv_vp: [[f32; 4]; 4],
    sun_pos: [f32; 4],
    sun_col: [f32; 4],
    moon_col: [f32; 4],
    sky: [f32; 4],
    n_lamps: [u32; 4],
    /// The camera's frame in the inertial one (the stars'), as columns; [0][3] the air here, [1][3] the sun's
    /// height over the horizon (sine).
    stars: [[f32; 4]; 3],
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ChunkGpu {
    base: [i32; 3],
    info: u32,
    off: u32,
    _p: [u32; 3],
}

/// The chunk table's entries by mesh offset, kept between frames so a traced instance's index stays valid and
/// the TLAS is rebuilt only when its instances change (ADR 0025, B): the index and the frame last drawn.
#[derive(Default)]
struct ChunkSlots {
    at: std::collections::HashMap<u32, (u32, u64)>,
    free: Vec<u32>,
    next: u32,
}

impl ChunkSlots {
    /// The entry for the mesh at `off`, and whether it is new (its table entry must be written).
    fn get(&mut self, off: u32, frame: u64) -> (u32, bool) {
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
    fn sweep(&mut self, frame: u64) {
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
struct LampGpu {
    pos: [f32; 4],
    col: [f32; 4],
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
    // the spawn: land nearest the +y pole (ADR 0021)
    let pole = planet.spawn * (planet.height(planet.spawn) + 1.7);
    match run {
        "fly" => {
            let r0 = planet.radius + alt;
            let th = speed * t / r0;
            let dir = DVec3::new(th.sin(), 0.0, th.cos());
            let pos = dir * (planet.top(dir) + alt);
            Pose { pos, fwd: DVec3::new(th.cos(), 0.0, -th.sin()), up: dir }
        }
        "orbit" => {
            // the waypoints where the bodies are now, in the inertial frame: the flight between them is there,
            // and each point is drawn in the frame it lies in (ADR 0022)
            let (pp, pm) = (w.pose(0, w.time), w.pose(1, w.time));
            let a = pp.to_inertial(planet.centre, pole);
            // high over both bodies: 3.3 planet radii, as 3.5 km was over the 1 km planet
            let high = planet.radius * 3.3;
            let b = pp.to_inertial(planet.centre, DVec3::new(0.0, high, 0.0));
            // c as high over the moon on the same side, and d landing below it: the run starts when the moon
            // stands 90 degrees from the spawn's zenith (`orbit_start`), so no leg passes through a body
            let up = pp.rot * DVec3::Y;
            let c = pm.centre + up * high;
            let mu = pm.rot.inverse() * up;
            let d = pm.centre + up * (moon.height(mu) + 1.7);
            let legs = [(a, b, 0), (b, c, 0), (c, d, 1), (d, d, 2), (d, c, 3), (c, b, 3), (b, a, 3)];
            let durs = [10.0, 10.0, 10.0, 5.0, 10.0, 10.0, 10.0].map(|d| d * q);
            let mut tt = (t - warm).max(0.0);
            let mut i = 0;
            while i < legs.len() - 1 && tt > durs[i] {
                tt -= durs[i];
                i += 1;
            }
            if t < warm {
                return Pose { pos: pole, fwd: DVec3::Z, up: DVec3::Y };
            }
            let (p0, p1, look) = legs[i];
            let x = p0.lerp(p1, smooth(tt / durs[i]));
            // up from the nearest body, and the look, in the inertial frame
            let near = if (x - pm.centre).length() - moon.radius < x.length() - planet.radius { pm } else { pp };
            let up = (x - near.centre).normalize();
            let fwd = match look {
                0 | 1 => (pm.centre - x).normalize(),
                2 => pm.rot * DVec3::Z,
                _ => (pp.centre - x).normalize(),
            };
            let fwd = if i == legs.len() - 1 && tt >= durs[i] { pp.rot * DVec3::Z } else { fwd };
            let (pos, f) = w.locate(x);
            let back = w.pose(f, w.time).rot.inverse();
            Pose { pos, fwd: back * fwd, up: back * up }
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
            Pose { pos: up * (planet.top(up) + a.get("eye", 1.7)), fwd, up }
        }
    }
}

/// The yaw and pitch (degrees) of run=stand that look along `fwd` from a spot whose up is `up`.
fn stand_angles(up: DVec3, fwd: DVec3) -> (f64, f64) {
    let r = if up.y.abs() > 0.9 { DVec3::Z } else { DVec3::Y };
    let ahead = (r - up * r.dot(up)).normalize();
    let pitch = fwd.dot(up).clamp(-1.0, 1.0).asin();
    (fwd.dot(up.cross(ahead)).atan2(fwd.dot(ahead)).to_degrees(), pitch.to_degrees())
}

/// The world time at which the orbit run starts: the first moment the moon stands 90 degrees from the spawn's
/// zenith, as the route was laid out before the bodies moved (ADR 0022).
fn orbit_start(w: &World) -> f64 {
    let side = |t: f64| (w.pose(0, t).rot * DVec3::Y).dot(w.pose(1, t).centre - w.pose(0, t).centre);
    let mut t = 0.0;
    while t < engine::sky::DAY && side(t).signum() == side(t + 0.5).signum() {
        t += 0.5;
    }
    t
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
    sky_pipeline: wgpu::RenderPipeline,
    /// The overlays (ADR 0017): ghost lines and the 2D layer, with their vertex and rectangle buffers.
    ghost_pipeline: wgpu::RenderPipeline,
    ghost_buf: wgpu::Buffer,
    ui_pipeline: wgpu::RenderPipeline,
    ui_buf: wgpu::Buffer,
    /// The water (ADR 0020): the sea's quads in the chunks, the water mesh from its own buffer.
    sea_pipeline: wgpu::RenderPipeline,
    water_pipeline: wgpu::RenderPipeline,
    wquads: wgpu::Buffer,
    bind: wgpu::BindGroup,
    globals: wgpu::Buffer,
    quads: wgpu::Buffer,
    chunks: wgpu::Buffer,
    lamps: wgpu::Buffer,
    lamp_cells: wgpu::Buffer,
    indirect: wgpu::Buffer,
    depth: wgpu::TextureView,
    /// The terrain's shader and the block looks: the bounce light is built again from them on a resize.
    shader: wgpu::ShaderModule,
    looks: wgpu::Buffer,
    ts: Option<(wgpu::QuerySet, wgpu::Buffer, Vec<(wgpu::Buffer, Arc<AtomicBool>, bool)>)>,
    ts_period: f32,
    capture: Option<wgpu::Buffer>,
    adapter_name: String,
    backend: String,
}

impl Gpu {
    /// The GPU, and the traced terrain bound to it. Vulkan only: wgpu's ray queries are there (ADR 0022).
    fn new(window: Arc<Window>, a: &Args, cap: u32) -> (Gpu, rt::Rt, gi::Gi) {
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
        let feats = (af & want) | wgpu::Features::EXPERIMENTAL_RAY_QUERY;
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
        // every block's look from the registry (ADR 0013): colour rgb, then pattern | side << 8, 16 bytes each
        let looks = buf("looks", 256 * 16, U::STORAGE | U::COPY_DST);
        let look: Vec<[u32; 4]> = (0..=255u8)
            .map(|id| match engine::blocks::BLOCKS.get(id) {
                Some(d) => {
                    let [r, g, b] = d.colour.map(f32::to_bits);
                    let shine = (d.metal * 255.0).round() as u32;
                    let gives = (d.light != [0.0; 3]) as u32;
                    [r, g, b, d.pattern as u32 | (d.side as u32) << 8 | shine << 16 | gives << 24]
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
                storage(4),
                storage(8),
                storage(9),
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
    /// Per stage of `CPU_STAGES` and `GPU_STAGES`, every measured frame.
    cpu_st: Vec<Vec<f32>>,
    gpu_st: Vec<Vec<f32>>,
    /// A frame over budget: its frame number, the CPU part, and the GPU stages once they are read back.
    spikes: Vec<(u64, String, Option<String>)>,
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
/// Seconds a correction takes to ease out (to 1/e).
const CORRECT_S: f64 = 0.1;
/// How much C held magnifies.
const ZOOM: f64 = 3.0;
const AIM_REACH: f64 = 8.0;
/// Two presses of space this close (seconds) take off or land.
const FLY_TAP: f64 = 0.3;
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

/// acts="2:C,2.5:c,3:R,3.5:r": (seconds after load, action), sorted so the soonest is last.
fn parse_acts(s: &str) -> Vec<(f64, String)> {
    let mut v: Vec<(f64, String)> =
        s.split(',').filter_map(|e| e.trim().split_once(':')).filter_map(|(t, a)| Some((t.parse().ok()?, a.to_string()))).collect();
    v.sort_by(|a, b| b.0.total_cmp(&a.0));
    v
}

struct App {
    args: Args,
    gpu: Option<Gpu>,
    /// The traced terrain (ADR 0022), beside the GPU it lives on.
    rt: Option<rt::Rt>,
    slots: ChunkSlots,
    /// The bounce light (ADR 0022).
    gi: Option<gi::Gi>,
    world: World,
    link: Link<ToServer, ToClient>,
    /// run=play: the player this client predicts, the keys held, mouse motion since the last frame, the look
    me: phys::Player,
    /// Where `me` was one physics step ago, and the time not yet stepped: the camera is drawn between them.
    prev: DVec3,
    debt: f64,
    drift_max: f64,
    keys: std::collections::HashSet<KeyCode>,
    mouse: (f64, f64),
    pitch: f64,
    /// The mouse's turn (radians to the left) not stepped yet: the next step takes it, the camera shows it now.
    /// Out in space (`phys::free`) the mouse's pitch tilts the body and Q/E roll it, the same way (ADR 0024).
    turn: f64,
    tilt: f64,
    roll: f64,
    input: Input,
    /// Times the server's state, replayed to now, differed from the prediction by over a millimetre.
    snaps: u64,
    /// run=play (ADR 0019): the steps the server has not answered yet, by number, the next number, and the
    /// first one not sent; `None` until the server's first `You` says where this player is.
    steps: std::collections::VecDeque<(u32, Input)>,
    next_step: u32,
    unsent: u32,
    spawned: bool,
    /// What a correction moved the drawn position by; it eases out over CORRECT_S.
    smooth: DVec3,
    /// Seconds the last frame spent after present (stats, loading checks), and when it ended: both land in the
    /// next frame's time, outside its timed phases, so a spike reports them.
    tail_prev: f64,
    frame_end: Option<Instant>,
    /// When the last steps went to the server: by the wall clock, since the player moves (from its spawn) before
    /// the world has loaded, and a second's steps held back past the server's queue were lost (a correction).
    last_move: Instant,
    edits_applied: u64,
    players: usize,
    streamer: Streamer,
    run: String,
    /// The world time at the run's start: time= seconds, by default 0 (9 am at the spawn), for the orbit run
    /// `orbit_start` (ADR 0022).
    time0: f64,
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
    /// The bucket is in hand (ADR 0020): the dig button scoops, the place button pours, at the cell before
    /// the aimed block (the ray passes through water to what is under it).
    bucket: bool,
    aim: Option<(Block, Block)>,
    held: Option<(bool, Block, Block)>,
    slot: usize,
    creative: bool,
    inv: Vec<(u8, u32)>,
    /// Units of water in the bucket (ADR 0020).
    bucket_water: u32,
    /// The mesh of the tiles that hold stored water, and how many of its quads the GPU has.
    water: water::WaterMesh,
    wquad_n: u32,
    /// The lamps near the camera (`gather_lamps`), and the frame and edit count they were gathered at.
    lamps: Vec<(u8, DVec3, [f32; 3])>,
    lamps_at: (u64, u64),
    /// What the overlay buffers hold: the ghost's shape and colour, its vertex count, the rectangle count.
    ghost_of: Option<(Shape, [u8; 4])>,
    ghost_n: u32,
    ui_n: u32,
    /// acts=: scripted input for checks without hands, as (seconds after load, action), soonest last.
    acts: Vec<(f64, String)>,
    title: String,
    /// shot=: where one settled frame of run=stand goes, and what it measured (ADR 0018).
    shot: Option<String>,
    shot_json: String,
    /// The last frame's camera (position, forward, up): the crack detector casts its rays from it.
    cam: (DVec3, DVec3, DVec3),
    /// The feel report (ADR 0018): edits not on screen yet (frame and time of the click, their cells), then
    /// each one's frames and milliseconds until it was; the last physics positions and the jerk between them.
    clicks: Vec<(u64, Instant, Vec<Block>)>,
    shown: Vec<(u64, f64)>,
    path: Vec<DVec3>,
    jerks: Vec<(f64, f64)>,
    /// When each correction came and how far it moved the player.
    snap_t: Vec<(f64, f64)>,
    last_pos: DVec3,
    sheet: Vec<image::RgbaImage>,
    next_cap: f64,
    done: bool,
    cap_prev: bool,
    /// The chat (ADR 0023): its lines with when they came, and the line being typed (T, or / for a command).
    chat: Vec<(Instant, String)>,
    typing: Option<String>,
    /// The lines this player sent, oldest first, and the one the up arrow shows now.
    sent: Vec<String>,
    recall: Option<usize>,
    /// The command line's completions (Tab, ADR 0023): the one picked, None while the list is hidden.
    pick: Option<usize>,
    /// C held zooms (3x); this eases towards it, 1 is none.
    zoom: f64,
    /// The server's clock (/timewarp): its time and warp at its last `You`, and when that came. The sky's
    /// clock runs on at the warp between them and eases onto it, so a fast warp turns the sky smoothly.
    clock: Option<(f64, f64, Instant)>,
    /// F3: the overlay of numbers; the last frames' times, the last GPU time, RAM and VRAM now and then, and
    /// what the last frame drew.
    f3: bool,
    recent: VecDeque<f32>,
    gpu_last: f64,
    /// ADR 0025: the frame number each timestamp slot was written in, the GPU's end of the last frame read
    /// back (ticks), and the last frame's CPU stretches, which the next frame's interval is made of.
    ts_frames: [u64; TS_RING],
    gpu_end: Option<(u64, u64)>,
    seg_prev: [f64; CPU_STAGES.len()],
    /// The streamer's job time and count when loading finished: the jobs after it are remeshes and new chunks
    /// as the camera moves (ADR 0025, E).
    jobs_at_load: (f64, u64),
    mem: (f64, f64),
    drawn: Drawn,
    /// F9: the frame is grabbed first, then the form holds what is written, and Enter writes it all to bugs/
    /// on a thread, which says how that went.
    bug_grab: bool,
    bug: Option<String>,
    bug_shot: Option<image::RgbaImage>,
    bug_said: (std::sync::mpsc::Sender<String>, std::sync::mpsc::Receiver<String>),
    /// Play's settings (ADR 0023) and where they are kept; the menu (Esc) when open, the mouse pointer, and
    /// the menu's buttons as last drawn (x, y, width, height).
    settings: Settings,
    settings_file: String,
    menu: Option<Menu>,
    pointer: (f32, f32),
    menu_boxes: Vec<[f32; 4]>,
    /// The settings' slider being dragged, and the one whose number is being typed with the text so far.
    drag: Option<usize>,
    edit: Option<(usize, String)>,
    onscreen: bool,
    /// Creative flight (ADR 0023): on or not, its speed (the wheel), and when space was last pressed.
    flying: bool,
    fly_speed: f64,
    space_at: Option<Instant>,
    /// Where this world comes from, for F3 and bug reports: the seed, the server, the save and the recording.
    seed: i32,
    server_at: String,
    save_path: Option<String>,
    rec_path: Option<String>,
    /// The title menu's saves as last listed, and the arguments the game starts again with when it closes
    /// (a world picked, or back to the title).
    worlds: Vec<String>,
    next: Option<Vec<String>>,
}

/// What a start without arguments runs: the title menu over a throwaway world, seen from orbit.
const TITLE_ARGS: [&str; 5] = ["run=play", "onscreen=1", "title=1", "save=0", "record=0"];
/// shortcut: the title lists the newest six saves; a scrolling list when there are more.
const WORLDS_SHOWN: usize = 6;

/// The saves under saves/, newest first.
fn list_worlds() -> Vec<String> {
    let mut v: Vec<(std::time::SystemTime, String)> = std::fs::read_dir("saves")
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "sav"))
        .map(|e| (e.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH), e.path().display().to_string().replace('\\', "/")))
        .collect();
    v.sort_by(|a, b| b.0.cmp(&a.0));
    v.into_iter().take(WORLDS_SHOWN).map(|(_, p)| p).collect()
}

/// The menu's pages (ADR 0023): the title's, and the pause menu's.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Menu {
    Title,
    Worlds,
    Pause,
    Settings,
}

/// Window sizes the settings offer, those that fit the monitor.
const RESOLUTIONS: [(u32, u32); 5] = [(1280, 720), (1600, 900), (1920, 1080), (2560, 1440), (3840, 2160)];
const FOVS: [f64; 6] = [60.0, 70.0, 80.0, 90.0, 100.0, 110.0];
const SENSITIVITIES: [f64; 8] = [0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0];
/// Chunks split when the camera is nearer than this many of their edges (`Opts::k`): more is finer far out.
const DETAILS: [f64; 5] = [3.0, 4.0, 5.0, 6.0, 8.0];

/// Play's settings (ADR 0023), kept as key=value lines in settings.txt; other runs keep the defaults, so
/// benchmarks and views do not change with them.
#[derive(Clone, Debug, PartialEq)]
struct Settings {
    width: u32,
    height: u32,
    /// One of DISPLAYS.
    display: &'static str,
    fov: f64,
    sensitivity: f64,
    detail: f64,
}

/// How the game shows: a window of the chosen size; a borderless window over the whole screen at its size;
/// or the screen switched to the chosen size (exclusive).
const DISPLAYS: [&str; 3] = ["windowed", "windowed fullscreen", "fullscreen"];

impl Default for Settings {
    fn default() -> Settings {
        Settings { width: 1920, height: 1080, display: "windowed", fov: 70.0, sensitivity: 1.0, detail: 4.0 }
    }
}

/// The window's fullscreen state for `s` on monitor `m`, and the size to render at: exclusive fullscreen
/// takes the monitor's fastest mode of the chosen size, or the whole screen borderless when it has none.
fn fullscreen(s: &Settings, m: Option<winit::monitor::MonitorHandle>) -> (Option<winit::window::Fullscreen>, (u32, u32)) {
    use winit::window::Fullscreen;
    let Some(m) = m.filter(|_| s.display != "windowed") else { return (None, (s.width, s.height)) };
    let screen = (m.size().width, m.size().height);
    let mode = m.video_modes().filter(|v| (v.size().width, v.size().height) == (s.width, s.height)).max_by_key(|v| v.refresh_rate_millihertz());
    match mode {
        Some(v) if s.display == "fullscreen" => (Some(Fullscreen::Exclusive(v)), (s.width, s.height)),
        _ => (Some(Fullscreen::Borderless(Some(m))), screen),
    }
}

impl Settings {
    fn parse(text: &str) -> Settings {
        let mut s = Settings::default();
        for (k, v) in text.lines().filter_map(|l| l.split_once('=')).map(|(k, v)| (k.trim(), v.trim())) {
            match k {
                "width" => s.width = v.parse().unwrap_or(s.width).clamp(640, 7680),
                "height" => s.height = v.parse().unwrap_or(s.height).clamp(360, 4320),
                "display" => s.display = DISPLAYS.iter().find(|d| **d == v).copied().unwrap_or(s.display),
                "fov" => s.fov = v.parse().unwrap_or(s.fov).clamp(30.0, 130.0),
                "sensitivity" => s.sensitivity = v.parse().unwrap_or(s.sensitivity).clamp(0.05, 10.0),
                "detail" => s.detail = v.parse().unwrap_or(s.detail).clamp(2.0, 12.0),
                _ => {}
            }
        }
        s
    }
    fn text(&self) -> String {
        format!(
            "width={}\nheight={}\ndisplay={}\nfov={}\nsensitivity={}\ndetail={}\n",
            self.width, self.height, self.display, self.fov, self.sensitivity, self.detail
        )
    }
}

/// How fast Q and E roll the body out in space (ADR 0024), radians a second.
const ROLL: f64 = 1.5;

/// The settings' sliders, rows 2 and 3 (field of view, mouse sensitivity): the track's ends, its step, and the
/// most a typed number may be (the least is the track's).
const SLIDERS: [(f64, f64, f64, f64); 2] = [(30.0, 130.0, 1.0, 130.0), (0.05, 5.0, 0.05, 10.0)];

impl Settings {
    /// Settings row k's slider value, when it is a slider.
    fn slider(&self, k: usize) -> Option<f64> {
        match k {
            2 => Some(self.fov),
            3 => Some(self.sensitivity),
            _ => None,
        }
    }
    /// Sets slider k to v, kept to the slider's range and step.
    fn set_slider(&mut self, k: usize, v: f64) {
        let (lo, _, step, max) = SLIDERS[k - 2];
        let v = ((v.clamp(lo, max) / step).round() * step * 100.0).round() / 100.0;
        if k == 2 { self.fov = v } else { self.sensitivity = v }
    }
    /// Slider k's value as its number shows it.
    fn slider_text(&self, k: usize) -> String {
        if k == 2 { format!("{:.0}", self.fov) } else { format!("{:.2}x", self.sensitivity) }
    }
}

/// The value after `cur` in `list` (before it when `back`), around the end; from the nearest when `cur` is
/// not in it.
fn cycle<T: Copy + PartialEq>(list: &[T], cur: T, back: bool) -> T {
    let i = list.iter().position(|v| *v == cur).unwrap_or(0);
    list[if back { (i + list.len() - 1) % list.len() } else { (i + 1) % list.len() }]
}

#[test]
fn settings_read_back_and_values_cycle() {
    let s = Settings { width: 2560, height: 1440, display: "windowed fullscreen", fov: 90.0, sensitivity: 0.5, detail: 6.0 };
    assert_eq!(Settings::parse(&s.text()), s);
    assert_eq!(Settings::parse("display=sideways").display, "windowed");
    assert_eq!(cycle(&DISPLAYS, "fullscreen", false), "windowed");
    assert_eq!(Settings::parse("fov=nonsense\nwidth=99999\njunk"), Settings { width: 7680, ..Settings::default() });
    assert_eq!(cycle(&FOVS, 110.0, false), 60.0);
    assert_eq!(cycle(&FOVS, 60.0, true), 110.0);
    assert_eq!(cycle(&FOVS, 70.0, false), 80.0);
    let mut s = Settings::default();
    s.set_slider(2, 83.4);
    s.set_slider(3, 0.333);
    assert_eq!((s.fov, s.sensitivity, s.slider_text(3)), (83.0, 0.35, "0.35x".to_string()));
    s.set_slider(2, 500.0);
    s.set_slider(3, 0.0);
    assert_eq!((s.slider(2), s.slider(3), s.slider(4)), (Some(130.0), Some(0.05), None));
}

/// What the last frame drew, for F3 and bug reports.
#[derive(Default)]
struct Drawn {
    chunks: usize,
    quads: u64,
    rt_pending: usize,
    sun_deg: f64,
    moon_lit: f64,
    /// The streamer this frame: chunks missing, holes, requests, jobs in flight.
    streaming: (u32, u32, u32, usize),
}

impl App {
    /// One input of run=play's building, from the keyboard and mouse or from acts=: L/R press and l/r release
    /// the dig and place buttons, h takes up or puts away the build hammer, b the bucket, q cycles its box, cylinder and
    /// cone, up/down change a cylinder's or cone's height, C and c press and let go of the zoom, 1-9 pick a hotbar slot,
    /// m:dx:dy turns the look.
    fn act(&mut self, a: &str) {
        match a {
            "L" | "R" if self.bucket => {
                if let Some((_, before)) = self.aim {
                    self.link.send(ToServer::Bucket { seq: self.pred.seq(), at: before, fill: a == "L" });
                }
            }
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
            "b" => {
                self.tool.kind = match self.tool.kind {
                    Kind::Box => Kind::Cylinder,
                    Kind::Cylinder => Kind::Cone,
                    Kind::Cone => Kind::Box,
                }
            }
            // C held zooms (it is read from the held keys); acts press it with C, let go with c
            "C" => drop(self.keys.insert(KeyCode::KeyC)),
            "c" => drop(self.keys.remove(&KeyCode::KeyC)),
            // the wheel: the flight speed while flying, unless the hammer holds a shape with a height
            "up" | "down" if self.flying && !(self.hammer && self.tool.kind != Kind::Box) => {
                let f = if a == "up" { 1.25 } else { 1.0 / 1.25 };
                self.fly_speed = (self.fly_speed * f).clamp(1.0, phys::FLY_MAX);
            }
            "up" | "down" => {
                self.tool.height = (self.tool.height + if a == "up" { 1 } else { -1 }).clamp(1, edit::MAX_SIDE);
            }
            // space twice within FLY_TAP seconds takes off or lands (creative, ADR 0023)
            "space" => {
                let now = Instant::now();
                if self.space_at.is_some_and(|t| (now - t).as_secs_f64() < FLY_TAP) {
                    self.space_at = None;
                    self.act("fly");
                } else {
                    self.space_at = Some(now);
                }
            }
            "fly" if self.creative => self.flying = !self.flying,
            "fly" => self.note("flying is creative's: /playmode creative".into()),
            "h" => (self.hammer, self.bucket) = (!self.hammer, false),
            "v" => (self.bucket, self.hammer) = (!self.bucket, false),
            // the chat and the bug report (ADR 0023): t opens the chat, / opens it on a command, type:x writes,
            // back takes a character, enter sends or saves, cancel closes; say:x sends a line at once
            "t" | "/" if !self.texting() => {
                self.typing = Some(if a == "/" { "/".into() } else { String::new() });
                // the up arrow first recalls the lines sent before; typing shows the completions
                (self.recall, self.pick) = (None, None);
                (self.held, self.mouse) = (None, (0.0, 0.0));
                self.keys.clear();
            }
            "f3" => self.f3 = !self.f3,
            // . and , step the time warp ten times up or down (the server's /timewarp answers)
            "warp+" | "warp-" => self.link.send(ToServer::Chat(format!("/timewarp {}", if a == "warp+" { "up" } else { "down" }))),
            "f9" if !self.texting() => {
                self.bug_grab = true;
                (self.held, self.mouse) = (None, (0.0, 0.0));
                self.keys.clear();
            }
            "enter" => self.submit(),
            // the menu (ADR 0023): esc opens it, goes back a page or closes it; click:k and rclick:k press button k
            "esc" => self.set_menu(match self.menu {
                None => Some(Menu::Pause),
                Some(Menu::Pause) => None,
                Some(Menu::Settings) => Some(self.home()),
                Some(Menu::Title | Menu::Worlds) => Some(Menu::Title),
            }),
            // point:x:y puts the pointer there (pixels), to see a button lit
            a if a.starts_with("point:") => {
                if let Some((x, y)) = a[6..].split_once(':').and_then(|(x, y)| Some((x.parse().ok()?, y.parse().ok()?))) {
                    self.pointer = (x, y);
                }
            }
            a if a.starts_with("click:") || a.starts_with("rclick:") => {
                if let Some(k) = a.split(':').nth(1).and_then(|k| k.parse().ok()) {
                    self.menu_press(k, a.starts_with('r'));
                }
            }
            // with the completions shown, the arrows move along them and Tab takes one (ADR 0023)
            "earlier" | "later" if self.pick.is_some() && !self.hints().is_empty() => {
                let (n, k) = (self.hints().len(), self.pick.unwrap_or(0));
                self.pick = Some(if a == "earlier" { (k + n - 1) % n } else { (k + 1) % n });
            }
            "tab" => {
                let Some(t) = self.typing.clone() else { return };
                let (at, hints) = engine::net::complete(&t);
                if let Some(h) = hints.get(self.pick.unwrap_or(0).min(hints.len().saturating_sub(1))) {
                    let mut line = format!("{}{h}", &t[..at]);
                    // on to the next word when there is one to pick
                    if !engine::net::complete(&format!("{line} ")).1.is_empty() {
                        line.push(' ');
                    }
                    self.typing = Some(line);
                }
                self.pick = Some(0);
            }
            // the chat's up and down arrows: the lines sent before, newest first, then back to an empty line
            "earlier" | "later" if self.typing.is_some() => {
                let n = self.sent.len();
                let at = match (a, self.recall) {
                    ("earlier", None) => n.checked_sub(1),
                    ("earlier", Some(k)) => Some(k.saturating_sub(1)),
                    (_, Some(k)) if k + 1 < n => Some(k + 1),
                    _ => None,
                };
                if at.is_some() || self.recall.is_some() {
                    self.typing = Some(at.map_or(String::new(), |k| self.sent[k].clone()));
                }
                (self.recall, self.pick) = (at, None);
            }
            "cancel" => (self.typing, self.bug, self.bug_shot, self.edit) = (None, None, None, None),
            // lclick: a left click at the pointer in the menu, let go at once (a slider jumps there)
            "lclick" => {
                self.menu_click(false);
                if self.drag.take().is_some() {
                    self.save_settings();
                }
            }
            "back" => {
                if let Some(t) = self.typing.as_mut().or(self.bug.as_mut()).or(self.edit.as_mut().map(|e| &mut e.1)) {
                    t.pop();
                }
                self.pick = Some(0);
            }
            // a slider's number takes digits and a point only
            a if a.starts_with("type:") && self.edit.is_some() => {
                let t = &mut self.edit.as_mut().unwrap().1;
                let room = 6usize.saturating_sub(t.len());
                t.extend(a[5..].chars().filter(|c| c.is_ascii_digit() || *c == '.').take(room));
            }
            a if a.starts_with("type:") => {
                if let Some(t) = self.typing.as_mut().or(self.bug.as_mut()) {
                    t.push_str(&a[5..]);
                }
                self.pick = Some(0);
            }
            a if a.starts_with("say:") => self.link.send(ToServer::Chat(a[4..].to_string())),
            _ => {
                if let Ok(d) = a.parse::<usize>() {
                    self.slot = d.clamp(1, 9) - 1;
                } else if let Some((x, y)) = a.strip_prefix("m:").and_then(|r| r.split_once(':')) {
                    self.mouse.0 += x.parse().unwrap_or(0.0);
                    self.mouse.1 += y.parse().unwrap_or(0.0);
                } else {
                    self.note(format!("unknown act {a:?}"));
                }
            }
        }
    }

    /// The menu's buttons on its open page.
    fn menu_items(&self) -> Vec<String> {
        let s = &self.settings;
        match self.menu {
            None => Vec::new(),
            Some(Menu::Title) => vec!["Singleplayer".into(), "Settings".into(), "Quit game".into()],
            Some(Menu::Worlds) => {
                let names = self.worlds.iter().map(|p| p.trim_start_matches("saves/").trim_end_matches(".sav").to_string());
                names.chain(["New world".into(), "Back".into()]).collect()
            }
            Some(Menu::Pause) => vec!["Back to game".into(), "Settings".into(), "Main menu".into(), "Quit game".into()],
            Some(Menu::Settings) => vec![
                if s.display == "windowed fullscreen" { format!("Resolution: {}x{} (native)", w(), h()) } else { format!("Resolution: {}x{}", s.width, s.height) },
                format!("Display: {}", s.display),
                "Field of view".into(),
                "Mouse sensitivity".into(),
                format!("Detail distance: {:.0}", s.detail),
                "Done".into(),
            ],
        }
    }

    /// The open page's sliders, row by row (the settings' field of view and mouse sensitivity).
    fn menu_sliders(&self) -> Vec<hud::Slider> {
        (0..self.menu_items().len())
            .map(|k| {
                let v = self.settings.slider(k).filter(|_| self.menu == Some(Menu::Settings))?;
                let (lo, hi, _, _) = SLIDERS[k - 2];
                let typing = self.edit.as_ref().filter(|e| e.0 == k);
                Some((((v - lo) / (hi - lo)) as f32, typing.map_or(self.settings.slider_text(k), |e| e.1.clone()), typing.is_some()))
            })
            .collect()
    }

    /// A left or right click at the pointer in the menu: on a slider's track it drags the slider, on its number
    /// it starts typing one; anywhere else on a button it presses it.
    fn menu_click(&mut self, back: bool) {
        let (x, y) = self.pointer;
        let Some(k) = self.menu_boxes.iter().position(|b| x >= b[0] && x < b[0] + b[2] && y >= b[1] && y < b[1] + b[3]) else { return };
        if self.menu_sliders().get(k).is_some_and(|s| s.is_some()) {
            let (t, n) = hud::slider_parts(self.menu_boxes[k], &self.menu_items()[k]);
            if x >= n[0] - 12.0 {
                self.edit = Some((k, String::new()));
                return;
            }
            if x >= t[0] - 12.0 && !back {
                self.drag = Some(k);
                self.slide();
                return;
            }
        }
        self.menu_press(k, back);
    }

    /// The dragged slider follows the pointer along its track.
    fn slide(&mut self) {
        let Some(k) = self.drag else { return };
        let (t, _) = hud::slider_parts(self.menu_boxes[k], &self.menu_items()[k]);
        let (lo, hi, _, _) = SLIDERS[k - 2];
        let f = ((self.pointer.0 - t[0]) / t[2]).clamp(0.0, 1.0) as f64;
        self.settings.set_slider(k, lo + f * (hi - lo));
    }

    /// Keeps the settings in their file.
    fn save_settings(&mut self) {
        if let Err(e) = std::fs::write(&self.settings_file, self.settings.text()) {
            self.note(format!("the settings could not be saved to {}: {e}", self.settings_file));
        }
    }

    /// Button `k` of the open page pressed (`back`: with the right button, a value goes the other way).
    fn menu_press(&mut self, k: usize, back: bool) {
        match (self.menu, k) {
            (Some(Menu::Title), 0) => self.set_menu(Some(Menu::Worlds)),
            (Some(Menu::Title) | Some(Menu::Pause), 1) => self.set_menu(Some(Menu::Settings)),
            (Some(Menu::Title), 2) | (Some(Menu::Pause), 3) => self.finish(),
            (Some(Menu::Worlds), k) if k < self.worlds.len() => self.start_again(vec![format!("save={}", self.worlds[k])]),
            (Some(Menu::Worlds), k) if k == self.worlds.len() => {
                // a new world: the first free saves/world-N.sav, a seed from the clock; the server makes the file
                let n = (1..).find(|n| !std::path::Path::new(&format!("saves/world-{n}.sav")).exists()).unwrap();
                let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.subsec_nanos() as i32 & 0x7fff_ffff);
                self.start_again(vec![format!("save=saves/world-{n}.sav"), format!("seed={seed}")]);
            }
            (Some(Menu::Worlds), _) => self.set_menu(Some(Menu::Title)),
            (Some(Menu::Pause), 0) => self.set_menu(None),
            (Some(Menu::Pause), 2) => self.start_again(TITLE_ARGS[2..].iter().map(|s| s.to_string()).collect()),
            (Some(Menu::Settings), 0..=4) => {
                let s = &mut self.settings;
                match k {
                    0 if s.display != "windowed fullscreen" => {
                        // a window fits the screen; exclusive fullscreen takes the sizes the screen can switch to
                        let mon = self.gpu.as_ref().and_then(|g| g.window.current_monitor());
                        let exclusive = s.display == "fullscreen";
                        let fits: Vec<(u32, u32)> = RESOLUTIONS
                            .iter()
                            .copied()
                            .filter(|r| match &mon {
                                None => true,
                                Some(m) if exclusive => m.video_modes().any(|v| (v.size().width, v.size().height) == *r),
                                Some(m) => r.0 <= m.size().width && r.1 <= m.size().height,
                            })
                            .collect();
                        (s.width, s.height) = cycle(if fits.is_empty() { &RESOLUTIONS[..1] } else { &fits }, (s.width, s.height), back);
                    }
                    0 => {}
                    1 => s.display = cycle(&DISPLAYS, s.display, back),
                    2 => s.fov = cycle(&FOVS, s.fov, back),
                    3 => s.sensitivity = cycle(&SENSITIVITIES, s.sensitivity, back),
                    _ => s.detail = cycle(&DETAILS, s.detail, back),
                }
                self.streamer.opts.k = self.settings.detail;
                if k <= 1 {
                    self.apply_display();
                }
                self.save_settings();
            }
            (Some(Menu::Settings), 5) => self.set_menu(Some(self.home())),
            _ => {}
        }
    }

    /// The title menu's view: in orbit over the planet's sunlit limb, the planet filling the left half and the
    /// sun to the right, drifting slowly around the line to the sun while the planet turns under it.
    fn title_view(&self, t: f64) -> Pose {
        let w = &self.world;
        let (c, r) = (w.bodies[0].centre, w.bodies[0].radius);
        let s = (w.from_inertial(0, w.sky.sun(w.time)) - c).normalize();
        // just out of the air (it fades by 1.5 km up), the sun 100 degrees from the planet's centre as seen from
        // here, so more than half the disc in view is lit
        let q = DQuat::from_axis_angle(s, t * 0.01) * s.any_orthonormal_vector();
        let (d, b) = (r + 1700.0, 100f64.to_radians());
        let o = -(s * b.cos() + q * b.sin());
        // the limb lies asin(r / d) off the centre's direction; the look a third of the way from it to the sun
        let l = (r / d).asin();
        let a = l + (b - l) * 0.35;
        let e = (s + o * s.dot(o)).normalize(); // the sun's way, square to the centre's
        let fwd = -o * a.cos() + e * a.sin();
        // rolled 35 degrees: the planet low on the left, the sun high on the right
        let up = DQuat::from_axis_angle(fwd, 35f64.to_radians()) * s.cross(fwd).normalize();
        Pose { pos: c + o * d, fwd, up }
    }

    /// The page the settings go back to: the title's in the title menu, else the pause menu.
    fn home(&self) -> Menu {
        if self.args.get("title", 0) == 1 { Menu::Title } else { Menu::Pause }
    }

    /// Closes this world (the local server saves as it stops) and starts the game again with `more` after
    /// play on screen: a fresh process per world, so nothing of the old one carries over.
    fn start_again(&mut self, more: Vec<String>) {
        let mut a: Vec<String> = TITLE_ARGS[..2].iter().map(|s| s.to_string()).collect();
        a[1] = format!("onscreen={}", self.args.get("onscreen", 0));
        // scripted checks keep their length and settings file in the next world too
        a.extend(["seconds", "settings"].iter().filter_map(|k| self.args.0.get(*k).map(|v| format!("{k}={v}"))));
        a.extend(more);
        self.next = Some(a);
        self.finish();
    }

    /// Opens a menu page or closes the menu: the pointer is free while it is open.
    fn set_menu(&mut self, m: Option<Menu>) {
        if m == Some(Menu::Worlds) {
            self.worlds = list_worlds();
        }
        self.menu = m;
        (self.held, self.mouse) = (None, (0.0, 0.0));
        self.keys.clear();
        if self.onscreen {
            let win = &self.gpu.as_ref().unwrap().window;
            if m.is_some() {
                let _ = win.set_cursor_grab(CursorGrabMode::None);
            } else {
                let _ = win.set_cursor_grab(CursorGrabMode::Confined).or_else(|_| win.set_cursor_grab(CursorGrabMode::Locked));
            }
            win.set_cursor_visible(m.is_some());
        }
    }

    /// The window as the settings say: fullscreen at the screen's size, or a window of the chosen size.
    fn apply_display(&mut self) {
        if !self.onscreen {
            self.resize(self.settings.width, self.settings.height);
            return;
        }
        let win = self.gpu.as_ref().unwrap().window.clone();
        let (fs, (nw, nh)) = fullscreen(&self.settings, win.current_monitor());
        let windowed = fs.is_none();
        win.set_fullscreen(fs);
        win.set_decorations(windowed);
        if windowed {
            let _ = win.request_inner_size(winit::dpi::PhysicalSize::new(nw, nh));
        }
        self.resize(nw, nh);
    }

    /// A new render size: the surface, the depth buffer, the capture buffer and the bounce light's targets
    /// are made again at it.
    fn resize(&mut self, nw: u32, nh: u32) {
        if nw == 0 || nh == 0 || (nw, nh) == (w(), h()) {
            return; // minimised, or no change
        }
        SIZE.store((nw as u64) << 32 | nh as u64, Ordering::Relaxed);
        let gpu = self.gpu.as_mut().unwrap();
        (gpu.config.width, gpu.config.height) = (nw, nh);
        gpu.surface.configure(&gpu.device, &gpu.config);
        gpu.depth = depth_view(&gpu.device);
        if gpu.capture.is_some() {
            gpu.capture = Some(capture_buf(&gpu.device));
        }
        let rt = self.rt.as_ref().unwrap();
        let shared = [
            gpu.globals.as_entire_binding(),
            gpu.quads.as_entire_binding(),
            gpu.chunks.as_entire_binding(),
            gpu.looks.as_entire_binding(),
            rt.tlas.as_binding(),
            gpu.lamps.as_entire_binding(),
            gpu.lamp_cells.as_entire_binding(),
        ];
        self.gi = Some(gi::Gi::new(&gpu.device, &gpu.shader, shared, gpu.config.format, nw, nh));
        eprintln!("[technoape] the render size is {nw}x{nh}");
    }

    /// The completions of the command being typed.
    fn hints(&self) -> Vec<String> {
        self.typing.as_deref().map_or(Vec::new(), |t| engine::net::complete(t).1)
    }

    /// The field of view drawn now, in degrees: the setting's, narrowed by the zoom.
    fn fov(&self) -> f64 {
        2.0 * ((self.settings.fov.to_radians() / 2.0).tan() / self.zoom).atan().to_degrees()
    }

    /// A text is being written (the chat or the bug report's form), or F9 waits for its frame: keys and the
    /// mouse go there, not to the player.
    fn texting(&self) -> bool {
        self.typing.is_some() || self.bug.is_some() || self.bug_grab || self.edit.is_some()
    }

    /// A line of this client's own for the chat, and the log.
    fn note(&mut self, line: String) {
        eprintln!("[technoape] {line}");
        self.chat_line(line);
    }

    fn chat_line(&mut self, line: String) {
        self.chat.push((Instant::now(), line));
        if self.chat.len() > 200 {
            self.chat.remove(0);
        }
    }

    /// Enter: the chat's line goes to the server (it runs the commands), the bug report to bugs/.
    fn submit(&mut self) {
        if let Some((k, t)) = self.edit.take() {
            if let Ok(v) = t.parse::<f64>() {
                self.settings.set_slider(k, v);
                self.save_settings();
            }
            return;
        }
        if let Some(line) = self.typing.take() {
            let line = line.trim().to_string();
            self.recall = None;
            if !line.is_empty() && self.sent.last() != Some(&line) {
                self.sent.push(line.clone());
            }
            if line.starts_with('/') {
                self.chat_line(format!("> {line}"));
            }
            // nothing stands under the sun: a creative player flies there
            if line.eq_ignore_ascii_case("/goto sun") && self.creative {
                self.flying = true;
            }
            if !line.is_empty() {
                self.link.send(ToServer::Chat(line));
            }
        } else if let Some(text) = self.bug.take() {
            self.report_bug(text.trim().to_string());
        }
    }

    /// The F3 overlay's lines (ADR 0023), left and right column; a bug report carries them too.
    fn debug_lines(&self) -> (Vec<String>, Vec<String>) {
        let (pos, fwd, _) = self.cam;
        let w = &self.world;
        let space = w.frame_at(pos) == engine::world::INERTIAL;
        let bi = if space { 0 } else { w.frame_at(pos) };
        let b = &w.bodies[bi];
        let rel = pos - b.centre;
        let (r, up) = (rel.length(), rel.normalize());
        let feet = if self.run == "play" { self.me.pos } else { pos - up * 1.7 };
        let blk = w.block_at(feet);
        let x = w.to_inertial(pos);
        let (to_sun, to_planet) = ((x - w.sky.sun(w.time)).length(), (x - w.sky.planet(w.time).centre).length());
        let mut ms: Vec<f32> = self.recent.iter().copied().collect();
        let avg = ms.iter().sum::<f32>() / ms.len().max(1) as f32;
        let p99 = pct(&mut ms, 0.99);
        let (day, h) = engine::sky::clock(w.time);
        let m = (h * 60.0).round() as u32 % (24 * 60);
        let body = ["planet", "moon"].get(bi).copied().unwrap_or("body");
        let sea = b.sea.map_or(String::new(), |s| format!(", {:.1} m over the sea", r - s));
        let look = match self.aim {
            Some((hit, _)) => format!(
                "Looking at: {} (face {}, {} {}, layer {})",
                BLOCKS.get(w.block(hit)).map_or("?", |d| d.name.as_str()),
                hit.face,
                hit.pos.x,
                hit.pos.y,
                hit.pos.z
            ),
            None => "Looking at: nothing in reach".into(),
        };
        let (yaw, pitch) = stand_angles(w.up_at(pos), fwd);
        let tool = if self.bucket { "bucket" } else if self.hammer { "build hammer" } else { "hands" };
        let tool = if self.flying { format!("{tool}, flying {:.0} m/s", self.fly_speed) } else { tool.to_string() };
        let block = self.slot_block(self.slot).and_then(|m| BLOCKS.get(m)).map_or("-", |d| d.name.as_str());
        let d = &self.drawn;
        let gpu = self.gpu.as_ref().unwrap();
        let left = vec![
            format!("TechnoApe {BUILD} ({}), Pillars of Simulation Engine", if cfg!(debug_assertions) { "debug" } else { "release" }),
            format!("{:.0} fps, frame {avg:.2} ms average, {p99:.2} ms p99, GPU {:.2} ms", 1000.0 / avg.max(0.01), self.gpu_last),
            if space {
                format!("In space: {:.1} km from the sun's centre, {:.1} km from the planet's", to_sun / 1000.0, to_planet / 1000.0)
            } else {
                format!("XYZ: {:.2} / {:.2} / {:.2} from the {body}'s centre", rel.x, rel.y, rel.z)
            },
            if space { "Height: no ground under you".into() } else { format!("Height: {r:.1} m from the centre, {:.1} m over the ground{sea}", r - b.top(up)) },
            if space {
                "Feet in block: none".into()
            } else {
                format!("Feet in block: {body} face {} at {} {}, layer {}", blk.face, blk.pos.x, blk.pos.y, blk.pos.z)
            },
            format!("Facing: yaw {yaw:.1}, pitch {pitch:.1}"),
            look,
            format!(
                "Time: day {day}, {:02}:{:02} at the spawn, world time {:.0} s{}",
                m / 60,
                m % 60,
                w.time,
                self.clock.filter(|c| c.1 > 1.0).map_or(String::new(), |c| format!(", warp {}x", c.1))
            ),
            format!("Sun {:.1} degrees over the horizon, the moon {:.0}% lit", d.sun_deg, d.moon_lit * 100.0),
            format!("Mode: {}, {tool}, slot {}: {block}", if self.creative { "creative" } else { "survival" }, self.slot + 1),
            format!("Chunks: {} drawn of {} kept, {} quads, {} traced builds waiting", d.chunks, self.streamer.slot_count(), d.quads, d.rt_pending),
            format!("Streaming: {} missing, {} holes, {} requests, {} jobs running", d.streaming.0, d.streaming.1, d.streaming.2, d.streaming.3),
            format!("Lamps near: {}, water quads: {}, edits applied: {}", self.lamps.len(), self.wquad_n, self.edits_applied),
            format!("Players: {}, corrections: {}", self.players, self.snaps),
            String::new(),
            "F3 hides this. T chats, / runs a command (/help), F9 reports a bug".into(),
        ];
        let right = vec![
            format!("GPU: {}", gpu.adapter_name),
            format!("{} with ray queries, {}x{}", gpu.backend, gpu.config.width, gpu.config.height),
            format!("RAM {:.0} MB, VRAM {:.0} MB", self.mem.0, self.mem.1),
            format!("CPU: {} workers", rayon::current_num_threads()),
            format!("World: seed {}, planet {} and moon {} columns a face", self.seed, w.bodies[0].n, w.bodies.get(1).map_or(0, |b| b.n)),
            format!("Server: {}", self.server_at),
            format!("Save: {}", self.save_path.as_deref().unwrap_or("none")),
            format!("Recording: {}", self.rec_path.as_deref().unwrap_or("none")),
        ];
        (left, right)
    }

    /// F9's report (ADR 0023): bugs/<UTC time>/ gets report.md (the text, a line that stands at the same
    /// spot, F3's lines, the chat, the arguments), screen.png (the frame when F9 was pressed) and world.sav (a
    /// copy of the save). The files are written on a thread.
    fn report_bug(&mut self, text: String) {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let dir = format!("bugs/{}", sys::utc_stamp(now));
        let (left, right) = self.debug_lines();
        let w = &self.world;
        let (pos, fwd, _) = self.cam;
        let bi = w.frame_at(pos);
        let b = &w.bodies[bi.min(w.bodies.len() - 1)];
        let up = (pos - b.centre).normalize();
        let (yaw, pitch) = stand_angles(up, fwd);
        let stand = if bi == 0 {
            format!(
                "technoape.exe run=stand at={:.6},{:.6},{:.6} yaw={yaw:.1} pitch={pitch:.1} eye={:.2} time={:.1} seed={} planet={} capture_ms=2500 sheet=out/bug.png",
                up.x,
                up.y,
                up.z,
                (pos - b.centre).length() - b.top(up),
                w.time,
                self.seed,
                b.n
            )
        } else {
            "(run=stand stands on the planet only)".into()
        };
        let mut args: Vec<String> = self.args.0.iter().map(|(k, v)| format!("{k}={v}")).collect();
        args.sort();
        let chat: Vec<String> =
            self.chat.iter().rev().take(50).rev().map(|(at, l)| format!("- {:.0} s ago: {l}", at.elapsed().as_secs_f64())).collect();
        let inv: Vec<String> = self.inv.iter().map(|(m, n)| format!("{} {n}", BLOCKS.get(*m).map_or("?", |d| d.name.as_str()))).collect();
        let report = format!(
            "# Bug report {}\n\n{}\n\n## To look at it again\n\n- The view, without the session's edits: `{stand}`\n- The world: {}\n- The session: {}\n\n## State (F3)\n\n{}\n\n{}\n\nInventory: {}; bucket {} units\n\n## Chat and messages\n\n{}\n\n## Arguments\n\n`{}`\n",
            sys::utc_stamp(now),
            if text.is_empty() { "(no text)" } else { &text },
            if self.save_path.as_ref().is_some_and(|s| std::path::Path::new(s).exists()) {
                "`world.sav` here is the save as it was (up to 30 s old); play a copy with `technoape.exe run=play onscreen=1 save=<copy>`"
            } else {
                "no save yet (play saves every 30 s), only what the view line shows"
            },
            self.rec_path.as_ref().map_or("not recorded".to_string(), |r| format!("recorded to `{r}`; `technoape-server replay={r}` runs it again")),
            left.iter().filter(|l| !l.is_empty() && !l.starts_with("F3 ")).map(|l| format!("- {l}")).collect::<Vec<_>>().join("\n"),
            right.iter().map(|l| format!("- {l}")).collect::<Vec<_>>().join("\n"),
            if inv.is_empty() { "nothing".into() } else { inv.join(", ") },
            self.bucket_water,
            if chat.is_empty() { "(none)".into() } else { chat.join("\n") },
            args.join(" ")
        );
        let (shot, save, said) = (self.bug_shot.take(), self.save_path.clone(), self.bug_said.0.clone());
        std::thread::spawn(move || {
            let out = (|| -> Result<(), String> {
                let d = std::path::Path::new(&dir);
                std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
                std::fs::write(d.join("report.md"), report).map_err(|e| e.to_string())?;
                if let Some(img) = shot {
                    img.save(d.join("screen.png")).map_err(|e| e.to_string())?;
                }
                if let Some(s) = save.filter(|s| std::path::Path::new(s).exists()) {
                    std::fs::copy(&s, d.join("world.sav")).map_err(|e| e.to_string())?;
                }
                Ok(())
            })();
            let _ = said.send(match out {
                Ok(()) => format!("bug report saved to {dir}/"),
                Err(e) => format!("the bug report could not be saved to {dir}/: {e}"),
            });
        });
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
        for c in &cells {
            self.streamer.touch_block(&self.world, *c);
        }
        if !cells.is_empty() {
            self.clicks.push((self.streamer.frame, Instant::now(), cells));
        }
        self.link.send(ToServer::Edit { seq, shape: *s, material: m });
    }

    /// Each frame of run=play: scripted acts that are due, the aim, the ghost and the 2D layer.
    fn build(&mut self, p: &Pose, t: f64) {
        while self.acts.last().is_some_and(|(at, _)| *at <= t) {
            let (_, a) = self.acts.pop().unwrap();
            self.act(&a);
        }
        while let Ok(l) = self.bug_said.1.try_recv() {
            self.note(l);
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
        // the title menu shows the orbit view and itself only
        let title = self.home() == Menu::Title;
        let mut rects = if title { Vec::new() } else { hud::layer(w() as f32, h() as f32, &slots, self.slot, self.creative, height) };
        if self.world.water_surface(p.pos).is_some() {
            // under water: a blue veil first, under the HUD. shortcut: a veil, not fog, until the sky has fog
            rects.insert(0, hud::Rect { xy: [-1.0, -1.0, 1.0, 1.0], colour: [0.05, 0.25, 0.4, 0.55], glyph: 0 });
        }
        // the text over it (ADR 0023): F3's numbers, the chat, the bug report's form
        let mut paint = hud::Paint::new(w() as f32, h() as f32);
        if self.f3 {
            let (l, r) = self.debug_lines();
            hud::columns(&mut paint, &l, &r);
        }
        let lines: Vec<(f32, &str)> = self.chat.iter().map(|(at, l)| (at.elapsed().as_secs_f32(), l.as_str())).collect();
        let caret = self.start.elapsed().as_millis() / 400 % 2 == 0;
        let (at, hints) = engine::net::complete(self.typing.as_deref().unwrap_or(""));
        if !title {
            hud::chat(&mut paint, &lines, self.typing.as_deref(), caret, at, &hints, self.pick);
        }
        if self.flying {
            let s = format!("Flying {:.0} m/s", self.fly_speed);
            // right of the hotbar (hud::layer: nine 44 px slots 6 px apart), clear of the chat
            paint.text(w() as f32 / 2.0 + 222.0 + 16.0, h() as f32 - 46.0, &s, [1.0, 1.0, 1.0, 0.9]);
        }
        if let Some(m) = self.menu {
            let (title, hint) = match m {
                Menu::Title => ("TechnoApe", "Pillars of Simulation Engine"),
                Menu::Worlds => ("Singleplayer", "A world carries on from its save; a new one gets its own."),
                Menu::Pause => ("Paused", "The world runs on. Esc goes back to the game."),
                Menu::Settings => ("Settings", "Click: the next value. Drag a slider, or click its number."),
            };
            let (items, sliders) = (self.menu_items(), self.menu_sliders());
            let boxes = hud::menu_boxes(h() as f32, &items, &sliders);
            let (x, y) = self.pointer;
            let hover = boxes.iter().position(|b| x >= b[0] && x < b[0] + b[2] && y >= b[1] && y < b[1] + b[3]);
            hud::menu(&mut paint, title, &items, &sliders, hint, hover, caret);
            self.menu_boxes = boxes;
        }
        // the bug report's form over everything, the menus too
        if let Some(b) = &self.bug {
            hud::bug_form(&mut paint, b, caret);
        }
        rects.extend(paint.out);
        let n = rects.len().min(hud::RECTS_MAX);
        gpu.queue.write_buffer(&gpu.ui_buf, 0, bytemuck::cast_slice(&rects[..n]));
        self.ui_n = n as u32;
        // shortcut: the tool and mode as words in the window title, until the HUD has text
        let block = self.slot_block(self.slot).and_then(|m| BLOCKS.get(m)).map_or("-".to_string(), |d| d.name.clone());
        let tool = match (self.hammer, self.tool.kind) {
            _ if self.bucket => format!("bucket {}%", self.bucket_water as i64 * 100 / engine::liquid::U),
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
        // ADR 0025: each stretch of the frame thread, back to back from `now`
        let mut seg = [0f64; CPU_STAGES.len()];
        let mut mark = now;
        macro_rules! lap {
            ($name:literal) => {{
                let n = Instant::now();
                seg[CPU_STAGES.iter().position(|s| *s == $name).unwrap()] += (n - mark).as_secs_f64();
                mark = n;
            }};
        }
        // the sky's clock: the server's in play, else time= seconds into the world plus the run's own
        if self.run != "play" {
            self.world.time = self.time0 + t;
        } else if let Some((st, warp, at)) = self.clock {
            let target = st + (now - at).as_secs_f64() * warp;
            self.world.time += dt.min(0.1) * warp;
            self.world.time += (target - self.world.time) * (1.0 - (-dt / 0.3).exp());
        }
        let p = if self.run == "play" {
            // the player: mouse turns the look, keys make the input, the shared physics moves the box here in
            // the same fixed steps as on the server, whose position wins when the two drift apart
            let (mx, my) = std::mem::take(&mut self.mouse);
            // C held zooms 3x, eased over about 0.1 s; the mouse turns as much slower
            let want = if self.keys.contains(&KeyCode::KeyC) { ZOOM } else { 1.0 };
            self.zoom += (want - self.zoom) * (1.0 - (-dt / 0.04).exp());
            let sens = 0.0025 * self.settings.sensitivity / self.zoom;
            // the look is the player's own and turns in the steps, so no frame change makes it mean another way
            self.turn -= mx * sens;
            // flight is creative's; the server holds to that too
            self.flying &= self.creative;
            if phys::free(&self.world, &self.me, self.flying) {
                // out here the mouse pitches the body itself, the head's tilt folded into it, and Q/E roll it
                self.tilt += std::mem::take(&mut self.pitch) - my * sens;
                let q = |c: KeyCode| self.keys.contains(&c) as i32 as f64;
                self.roll += (q(KeyCode::KeyE) - q(KeyCode::KeyQ)) * ROLL * dt;
            } else {
                self.pitch = (self.pitch - my * sens).clamp(-1.55, 1.55);
            }
            let k = |c: KeyCode| self.keys.contains(&c) as i32 as f32;
            let auto = a.get("autowalk", 0) == 1;
            let mut inp = Input {
                mv: [if auto { 1.0 } else { k(KeyCode::KeyW) - k(KeyCode::KeyS) }, k(KeyCode::KeyD) - k(KeyCode::KeyA)],
                jump: auto || self.keys.contains(&KeyCode::Space),
                pitch: self.pitch as f32,
                fly: if self.flying { self.fly_speed as f32 } else { 0.0 },
                down: self.flying && (self.keys.contains(&KeyCode::ShiftLeft) || self.keys.contains(&KeyCode::ShiftRight)),
                t: self.world.time,
                ..Default::default()
            };
            // nothing moves before the server says where this player starts
            self.debt = if self.spawned { self.debt + dt.min(0.1) } else { 0.0 };
            while self.debt >= phys::DT {
                self.prev = self.me.pos;
                inp.turn = std::mem::take(&mut self.turn) as f32;
                (inp.tilt, inp.roll) = (std::mem::take(&mut self.tilt) as f32, std::mem::take(&mut self.roll) as f32);
                let f0 = self.world.frame_at(self.me.pos);
                phys::step(&self.world, &mut self.me, &inp, phys::DT);
                let f1 = self.world.frame_at(self.me.pos);
                if f0 != f1 {
                    // a step into another frame: the drawn path and what is left of a correction go along, or the
                    // camera is drawn between two places in two frames for a frame
                    let (r, off) = self.world.place(f0, f1);
                    self.prev = r * (self.prev - self.world.nominal(f0)) + off;
                    self.smooth = r * self.smooth;
                    self.path.clear();
                }
                self.steps.push_back((self.next_step, inp));
                self.next_step += 1;
                if self.loaded_at.is_some() {
                    // the third difference of the fixed-step path: jerk in m/s^3
                    self.path.push(self.me.pos);
                    if self.path.len() > 4 {
                        self.path.remove(0);
                    }
                    if let [a, b, c, d] = self.path[..] {
                        self.jerks.push((t, (d - c * 3.0 + b * 3.0 - a).length() / phys::DT.powi(3)));
                    }
                }
                self.debt -= phys::DT;
            }
            self.input = inp;
            // the look turns every frame; the position is drawn between the last two physics steps, plus what
            // is left of the last correction
            self.smooth *= (-dt / CORRECT_S).exp();
            let pos = self.prev.lerp(self.me.pos, self.debt / phys::DT) + self.smooth;
            // the body as last stepped, with the turn, tilt and roll the next step takes shown now; the head's
            // pitch over its horizon
            let up = self.me.body_up(phys::up(&self.world, pos));
            let fwd = DQuat::from_axis_angle(up, self.turn) * phys::tangent(self.me.fwd, up, self.me.fwd);
            let tilt = DQuat::from_axis_angle(fwd.cross(up).normalize(), self.tilt);
            let (up, fwd) = (tilt * up, tilt * fwd);
            let up = DQuat::from_axis_angle(fwd, self.roll) * up;
            let look = DQuat::from_axis_angle(fwd.cross(up), self.pitch) * fwd;
            Pose { pos: pos + up * phys::EYE, fwd: look, up }
        } else {
            pose(&self.run, t, &self.world, speed, alt, warm, q, a)
        };
        if self.run == "play" && self.loaded_at.is_some() {
            self.build(&p, t);
        }
        let p = if self.home() == Menu::Title { self.title_view(t) } else { p };
        let vel = if dt > 0.0 && self.loaded_at.is_some() { (p.pos - self.last_pos) / dt } else { DVec3::ZERO };
        self.last_pos = p.pos;
        lap!("pose");

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
                Recv::Msg(ToClient::Players(ps)) => self.players = ps.len(),
                Recv::Msg(ToClient::You { step, me, time, warp }) if self.run == "play" => {
                    // the first, or a jump (/time set): straight there
                    if self.clock.is_none_or(|(_, w, _)| (time - self.world.time).abs() > 2.0 * w.max(warp) + 1.0) {
                        self.world.time = time;
                    }
                    self.clock = Some((time, warp, Instant::now()));
                    self.reconcile(step, me, t)
                }
                Recv::Msg(ToClient::You { .. }) => {}
                Recv::Msg(ToClient::Inventory { creative, counts, water }) => (self.creative, self.inv, self.bucket_water) = (creative, counts, water),
                Recv::Msg(ToClient::Water(v)) => {
                    let mut ts = Vec::with_capacity(v.len());
                    for (t, cols) in &v {
                        let was = self.world.water.wet(*t);
                        self.world.water.set_tile(*t, cols);
                        if was != self.world.water.wet(*t) {
                            // the chunks' sea leaves this tile out, or covers it again
                            self.streamer.touch_tile(&self.world, *t);
                        }
                        ts.push(*t);
                    }
                    self.water.update(&self.world, &ts, self.start.elapsed().as_secs_f32());
                }
                Recv::Msg(ToClient::Welcome { .. }) => {}
                Recv::Msg(ToClient::Chat(text)) => {
                    for line in text.lines() {
                        eprintln!("[chat] {line}");
                        self.chat_line(line.to_string());
                    }
                }
                Recv::Empty => break,
                Recv::Closed => {
                    eprintln!("[technoape] the server closed the connection");
                    self.finish();
                    return;
                }
            }
        }
        if (now - self.last_move).as_secs_f64() >= 1.0 / TICK_HZ {
            if self.run == "play" {
                let inputs: Vec<Input> = self.steps.iter().filter(|s| s.0 >= self.unsent).map(|s| s.1).collect();
                if !inputs.is_empty() {
                    self.link.send(ToServer::Steps { first: self.unsent, inputs });
                    self.unsent = self.next_step;
                }
            } else {
                // the benchmark runs fly: the camera position is sent as the player's position
                self.link.send(ToServer::Move { pos: p.pos.to_array(), fwd: p.fwd.as_vec3().to_array(), pitch: 0.0 });
            }
            self.last_move = now;
        }
        lap!("msgs");

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
        lap!("edits");

        let gpu = self.gpu.as_ref().unwrap();
        let mut st = FrameStats::default();
        let budget = (upload_mb * 1e6) as usize;
        let mut ups = Vec::new();
        self.streamer.apply(budget, &mut st, |off, q| {
            gpu.upload(off, q);
            ups.push(off);
        });
        let rt = self.rt.as_mut().unwrap();
        // a released mesh's BLAS goes the same frame (ADR 0025, G)
        for off in self.streamer.released.drain(..) {
            rt.forget(off);
        }
        for off in ups {
            rt.forget(off);
            // new quads at an offset: its table entry is for another chunk now
            if let Some((i, _)) = self.slots.at.remove(&off) {
                self.slots.free.push(i);
            }
        }
        if !self.clicks.is_empty() {
            let (world, streamer, frame) = (&self.world, &self.streamer, self.streamer.frame);
            let mut done = Vec::new();
            self.clicks.retain(|(f, at, cells)| {
                let on = cells.iter().all(|b| streamer.shows(world, *b));
                if on {
                    done.push((frame - f, at.elapsed().as_secs_f64() * 1000.0));
                }
                !on
            });
            self.shown.extend(done);
        }
        lap!("apply");

        let aspect = w() as f32 / h() as f32;
        let proj = glam::camera::rh::proj::directx::perspective_infinite_reverse((self.fov() as f32).to_radians(), aspect, 0.1);
        // the local up keeps the horizon level; a fixed axis only when the look is (almost exactly) straight up or
        // down. Swapping to it already at 78 degrees (0.98) rolled the view as the player walked, looking down.
        let up = if p.fwd.cross(p.up).length() < 1e-4 { DVec3::Z } else { p.up };
        self.cam = (p.pos, p.fwd, up);
        let viewm = glam::camera::rh::view::look_to_mat4(Vec3::ZERO, p.fwd.as_vec3(), up.as_vec3());
        let vp = proj * viewm;
        let view = View::new(p.pos, frustum_planes(vp), &self.world);
        let mut draws: Vec<DrawChunk> = Vec::new();
        let mut reqs = Vec::new();
        self.streamer.frame += 1;
        lap!("table");
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
        lap!("select");
        self.streamer.dispatch(&self.world, &mut reqs);
        let sel = ts0.elapsed().as_secs_f64();
        lap!("dispatch");
        if self.streamer.frame % 30 == 0 {
            self.streamer.evict(120);
            let rt = self.rt.as_mut().unwrap();
            for off in self.streamer.released.drain(..) {
                rt.forget(off);
            }
        }

        // the indirect args per frame; the chunk table's entries only for meshes new to it (ADR 0025, B)
        let mut table: Vec<(u32, ChunkGpu)> = Vec::new();
        let mut args: Vec<(f32, DrawArgs)> = Vec::with_capacity(draws.len());
        let mut sea_args: Vec<DrawArgs> = Vec::new();
        let mut nquads = 0u64;
        let mut insts: Vec<rt::Inst> = Vec::with_capacity(draws.len());
        let frame_no = self.streamer.frame;
        for d in draws.iter().take(MAX_CHUNKS) {
            let k = d.key;
            // the lattice its quads were merged in (ADR 0025 I2), as log2 at bit 16
            let lat = if self.streamer.opts.greedy { self.world.bodies[k.body as usize].lattice(k.level) } else { 1 };
            let info = k.face as u32 | (k.level as u32) << 3 | (k.body as u32) << 8 | lat.trailing_zeros() << 16;
            let (ci, new) = self.slots.get(d.off, frame_no);
            assert!((ci as usize) < MAX_CHUNKS, "chunk table full");
            if new {
                table.push((ci, ChunkGpu { base: k.base().into(), info, off: d.off, _p: [0; 3] }));
            }
            let land = d.len - d.water;
            if land > 0 {
                insts.push(rt::Inst { off: d.off, land, base: k.base().into(), info, body: k.body, ci });
            }
            if d.hidden {
                continue;
            }
            args.push((d.dist, DrawArgs { vertex_count: land * 6, instance_count: 1, first_vertex: d.off * 6, first_instance: ci }));
            if d.water > 0 {
                // vs_sea draws each sea quad in SEA_SUB^2 pieces, one instance each: instance = chunk * pieces + piece
                let pieces = SEA_SUB * SEA_SUB;
                sea_args.push(DrawArgs { vertex_count: d.water * 6, instance_count: pieces, first_vertex: (d.off + land) * 6, first_instance: ci * pieces });
            }
            nquads += d.len as u64;
        }
        // near to far, so the depth test drops what is hidden before its fragments trace their rays (ADR 0025, D)
        args.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
        let args: Vec<DrawArgs> = args.into_iter().map(|a| a.1).collect();
        // the water mesh when it changed, the droplets after it every frame
        let n = self.water.all.len().min(MAX_WQUADS);
        if std::mem::take(&mut self.water.changed) {
            gpu.queue.write_buffer(&gpu.wquads, 0, bytemuck::cast_slice(&self.water.all[..n]));
        }
        let drops = self.water.particles(&self.world, dt as f32);
        let d = drops.len().min(MAX_WQUADS - n);
        if d > 0 {
            gpu.queue.write_buffer(&gpu.wquads, (n * std::mem::size_of::<water::WQuad>()) as u64, bytemuck::cast_slice(&drops[..d]));
        }
        self.wquad_n = (n + d) as u32;
        // the sun where the sky has it, seen from the camera's frame (ADR 0022)
        let cf = self.world.frame_at(p.pos);
        let sun = (self.world.from_inertial(cf, self.world.sky.sun(self.world.time)) - p.pos).normalize().as_vec3();
        let clock = self.start.elapsed().as_secs_f32();
        let (sky_m, wt) = (&self.world.sky, self.world.time);
        let sun_at = self.world.from_inertial(cf, sky_m.sun(wt)) - p.pos;
        // the moon's light: sunlight by its phase seen from the planet, less what the planet hides of the sun
        let (sp, mp, pp) = (sky_m.sun(wt), sky_m.moon(wt).centre, sky_m.planet(wt).centre);
        let phase = 0.5 * (1.0 + (sp - mp).normalize().dot((pp - mp).normalize()));
        let moon = MOON_LIGHT * (phase * (1.0 - sky_m.covered(wt, mp, pp, sky_m.planet_radius))) as f32;
        let sun_deg = self.world.up_at(p.pos).dot(sun_at.normalize()).clamp(-1.0, 1.0).asin().to_degrees();
        self.drawn = Drawn {
            chunks: args.len(),
            quads: nquads,
            rt_pending: self.drawn.rt_pending,
            sun_deg,
            moon_lit: phase,
            streaming: (st.missing, st.holes, st.requests, self.streamer.inflight),
        };
        // the sky here: the sun's height over this horizon and what the moon hides of it, and the air thinning
        let day = {
            let h = self.world.up_at(p.pos).dot(sun_at.normalize());
            let x = ((h + 0.2) / 0.35).clamp(0.0, 1.0);
            (x * x * (3.0 - 2.0 * x) * (1.0 - sky_m.eclipse(wt, self.world.to_inertial(p.pos)))) as f32
        };
        // shortcut: a body with a sea has air, the moon none, until bodies carry their atmosphere in data
        let x = self.world.to_inertial(p.pos);
        let over_air = (0..self.world.bodies.len())
            .filter(|&i| self.world.bodies[i].sea.is_some())
            .map(|i| (x - self.world.pose(i, wt).centre).length() - self.world.bodies[i].radius)
            .fold(f64::MAX, f64::min);
        let air = (1.0 - over_air.max(0.0) / 1500.0).clamp(0.0, 1.0) as f32;
        let sky_c = Vec3::new(0.45, 0.65, 0.95) * day * air + Vec3::new(0.01, 0.012, 0.025);
        // the sunlight through the air: the longer its way through it (the lower the sun), the more blue and
        // green it loses; at noon as it was, in space white (shader.wgsl sun_tint is the same)
        let sun_h = self.world.up_at(p.pos).dot(sun_at.normalize()) as f32;
        let tint = {
            let k = Vec3::new(0.02, 0.06, 0.16);
            let mass = (1.0 / (sun_h.max(0.0) + 0.035)).min(28.0);
            Vec3::ONE.lerp(Vec3::new((-k.x * mass).exp(), (-k.y * mass).exp(), (-k.z * mass).exp()) / Vec3::new((-k.x).exp(), (-k.y).exp(), (-k.z).exp()), air)
        };
        let sun_light = Vec3::new(0.65, 0.97 * 0.65, 0.9 * 0.65).lerp(Vec3::splat(0.65), 1.0 - air) * tint;
        let to_stars = glam::DMat3::from_quat(self.world.pose(cf, wt).rot).as_mat3();
        let mut g = Globals {
            view_proj: vp.to_cols_array_2d(),
            sun: [sun.x, sun.y, sun.z, clock],
            bodies: bytemuck::Zeroable::zeroed(),
            inv_vp: vp.inverse().to_cols_array_2d(),
            sun_pos: [sun_at.x as f32, sun_at.y as f32, sun_at.z as f32, engine::sky::SUN_RADIUS as f32],
            sun_col: [sun_light.x, sun_light.y, sun_light.z, 0.0],
            moon_col: [moon.x, moon.y, moon.z, 1.0],
            sky: [sky_c.x, sky_c.y, sky_c.z, day * air],
            n_lamps: [0; 4],
            stars: [
                [to_stars.x_axis.x, to_stars.x_axis.y, to_stars.x_axis.z, air],
                [to_stars.y_axis.x, to_stars.y_axis.y, to_stars.y_axis.z, sun_h],
                [to_stars.z_axis.x, to_stars.z_axis.y, to_stars.z_axis.z, 0.0],
            ],
        };
        // per body: the camera in its coordinates, and its frame in the camera's
        let mut cams = [(DVec3::ZERO, glam::DMat3::IDENTITY); MAX_BODIES];
        for (bi, (b, o)) in self.world.bodies.iter().zip(g.bodies.iter_mut()).enumerate() {
            // the camera in the body's frame, relative to its centre, in two floats per axis: one float is
            // 0.1 mm coarse at 1 km; and the body's frame turned into the camera's
            let (r, off) = self.world.place(cf, bi);
            let c = r * (p.pos - self.world.nominal(cf)) + off - b.centre;
            let m = glam::DMat3::from_quat(self.world.place(bi, cf).0).as_mat3();
            let col = |v: Vec3| [v.x, v.y, v.z, 0.0];
            let hi = c.as_vec3();
            let lo = (c - hi.as_dvec3()).as_vec3();
            *o = BodyGpu {
                cam_hi: hi.into(),
                r0: b.r0 as f32,
                cam_lo: lo.into(),
                lnq: b.lnq as f32,
                theta: (std::f64::consts::FRAC_PI_2 / b.n as f64) as f32,
                half_n: b.n as f32 * 0.5,
                _p: [b.sea.map_or(-1.0, |s| s as f32), 0.0],
                rot: [col(m.x_axis), col(m.y_axis), col(m.z_axis)],
                ctr: { let v = (m.as_dmat3() * -c).as_vec3(); [v.x, v.y, v.z, b.radius as f32] },
            };
            cams[bi] = (c + b.centre, m.as_dmat3());
        }
        // the lamps: gathered again when they change or every 10 frames, placed and gridded every frame
        let ec = self.world.lights_rev;
        if self.lamps_at.1 != ec || self.streamer.frame >= self.lamps_at.0 + 10 {
            let c: Vec<DVec3> = cams.iter().map(|c| c.0).collect();
            self.lamps = gather_lamps(&self.world, &c[..self.world.bodies.len()]);
            self.lamps_at = (self.streamer.frame, ec);
        }
        let lamps: Vec<LampGpu> = self
            .lamps
            .iter()
            .map(|&(bi, x, l)| {
                let (cam, m) = cams[bi as usize];
                let v = (m * (x - cam)).as_vec3();
                LampGpu { pos: [v.x, v.y, v.z, bi as f32], col: [l[0], l[1], l[2], 0.0] }
            })
            .collect();
        g.n_lamps[0] = lamps.len() as u32;
        g.n_lamps[1] = self.gi.as_ref().unwrap().frame() as u32;
        if !lamps.is_empty() {
            gpu.queue.write_buffer(&gpu.lamps, 0, bytemuck::cast_slice(&lamps));
            gpu.queue.write_buffer(&gpu.lamp_cells, 0, bytemuck::cast_slice(&lamp_grid(&lamps)));
        }
        [g.n_lamps[2], g.n_lamps[3]] = eclipse_masks(&g);
        gpu.queue.write_buffer(&gpu.globals, 0, bytemuck::bytes_of(&g));
        self.slots.sweep(frame_no);
        for (ci, c) in &table {
            gpu.queue.write_buffer(&gpu.chunks, (*ci as usize * std::mem::size_of::<ChunkGpu>()) as u64, bytemuck::bytes_of(c));
        }
        if !args.is_empty() {
            gpu.queue.write_buffer(&gpu.indirect, 0, bytemuck::cast_slice(&args));
        }
        if !sea_args.is_empty() {
            gpu.queue.write_buffer(&gpu.indirect, (MAX_CHUNKS * 16) as u64, bytemuck::cast_slice(&sea_args));
        }

        let mut sky = wgpu::Color { r: sky_c.x as f64, g: sky_c.y as f64, b: sky_c.z as f64, a: 1.0 };
        if self.shot.is_some() {
            // the crack detector's colour: no block or sky has it, so every such pixel is a hole or sky
            sky = wgpu::Color { r: 1.0, g: 0.0, b: 1.0, a: 1.0 };
        }
        lap!("table");

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
        lap!("acquire");
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
                    let v: Vec<u64> = bytemuck::cast_slice(&data).to_vec();
                    drop(data);
                    rb.unmap();
                    let ms = |a: u64, b: u64| b.wrapping_sub(a) as f64 * gpu.ts_period as f64 / 1e6;
                    // the old gpu_ms (G-buffer start to screen end) stays comparable; the stages are new
                    let old = ms(v[1], v[6]);
                    let f = self.ts_frames[i];
                    let gap = match self.gpu_end {
                        Some((pf, end)) if pf + 1 == f => ms(end, v[0]),
                        _ => f64::NAN,
                    };
                    self.gpu_end = Some((f, v[6]));
                    let st: Vec<f64> = (0..6).map(|k| ms(v[k], v[k + 1])).chain([ms(v[0], v[6]), gap]).collect();
                    if old < 1000.0 {
                        if measuring {
                            self.samples.gpu_ms.push(old as f32);
                            let g = &mut self.samples.gpu_st;
                            g.resize(GPU_STAGES.len(), Vec::new());
                            for (k, x) in st.iter().enumerate() {
                                if x.is_finite() {
                                    g[k].push(*x as f32);
                                }
                            }
                        }
                        if let Some(s) = self.samples.spikes.iter_mut().find(|s| s.0 == f) {
                            s.2 = Some(GPU_STAGES.iter().zip(&st).map(|(n, x)| format!("\"{n}\":{:.2}", if x.is_finite() { *x } else { -1.0 })).collect::<Vec<_>>().join(","));
                        }
                    }
                    self.gpu_last = old;
                    ts_use = Some(i);
                }
            } else {
                ts_use = Some(i);
            }
        }
        let stamp = |enc: &mut wgpu::CommandEncoder, k: usize| {
            if let (Some(i), Some((qs, _, _))) = (ts_use, &gpu.ts) {
                enc.write_timestamp(qs, (i * TS_N + k) as u32);
            }
        };
        stamp(&mut enc, 0);
        let rt = self.rt.as_mut().unwrap();
        rt.record(&gpu.device, &gpu.queue, &mut enc, &insts);
        let rt_pending = rt.pending;
        self.drawn.rt_pending = rt_pending;
        stamp(&mut enc, 1);
        lap!("rt");
        {
            // the G-buffer: the sky and the terrain; the bounce light and the composite come after
            let gi = self.gi.as_mut().unwrap();
            let [tdirect, talbedo, tgbuf] = gi.targets();
            let target = |view, clear| {
                Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(clear), store: wgpu::StoreOp::Store },
                })
            };
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gbuffer"),
                color_attachments: &[target(tdirect, sky), target(talbedo, wgpu::Color::TRANSPARENT), target(tgbuf, wgpu::Color::TRANSPARENT)],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &gpu.depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_bind_group(0, &gpu.bind, &[]);
            // the crack detector's shots keep the plain clear colour
            if self.shot.is_none() {
                rp.set_pipeline(&gpu.sky_pipeline);
                rp.draw(0..3, 0..1);
            }
            rp.set_pipeline(&gpu.pipeline);
            if self.args.get("mdi", 1) == 1 {
                rp.multi_draw_indirect(&gpu.indirect, 0, args.len() as u32);
            } else {
                for d in &args {
                    rp.draw(d.first_vertex..d.first_vertex + d.vertex_count, d.first_instance..d.first_instance + 1);
                }
            }
            drop(rp);
            stamp(&mut enc, 2);
            gi.record(&gpu.queue, &mut enc, vp, p.pos, cf, self.shot.is_some(), &mut |e, k| stamp(e, 3 + k));
            stamp(&mut enc, 5);
            // the screen: direct and bounced light, then the water and the overlays over it
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("screen"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &fview,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &gpu.depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            gi.composite(&mut rp);
            rp.set_bind_group(0, &gpu.bind, &[]);
            if !sea_args.is_empty() {
                rp.set_pipeline(&gpu.sea_pipeline);
                if self.args.get("mdi", 1) == 1 {
                    rp.multi_draw_indirect(&gpu.indirect, (MAX_CHUNKS * 16) as u64, sea_args.len() as u32);
                } else {
                    for d in &sea_args {
                        rp.draw(d.first_vertex..d.first_vertex + d.vertex_count, d.first_instance..d.first_instance + d.instance_count);
                    }
                }
            }
            if self.wquad_n > 0 {
                rp.set_pipeline(&gpu.water_pipeline);
                rp.draw(0..self.wquad_n * 6, 0..1);
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
        stamp(&mut enc, 6);
        if let (Some(i), Some((qs, resolve, ring))) = (ts_use, &gpu.ts) {
            enc.resolve_query_set(qs, (i * TS_N) as u32..((i + 1) * TS_N) as u32, resolve, (i * 256) as u64);
            enc.copy_buffer_to_buffer(resolve, (i * 256) as u64, &ring[i].0, 0, (TS_N * 8) as u64);
            self.ts_frames[i] = self.streamer.frame;
        }
        // a shot waits a second after loading, so the last meshes are in
        let cap_at = if self.shot.is_some() { 1.0 } else { self.next_cap };
        let film = self.shot.is_some() || self.args.get("capture_ms", 0.0) > 0.0;
        let cap_now = film && gpu.capture.is_some() && self.loaded_at.is_some() && t >= cap_at;
        // F9: this frame, without the form, is the report's screenshot
        let bug_now = self.bug_grab && gpu.capture.is_some();
        if cap_now || bug_now {
            enc.copy_texture_to_buffer(
                frame.texture.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: gpu.capture.as_ref().unwrap(),
                    layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row_bytes()), rows_per_image: Some(h()) },
                },
                wgpu::Extent3d { width: w(), height: h(), depth_or_array_layers: 1 },
            );
        }
        lap!("encode");
        let tsub = Instant::now();
        gpu.queue.submit([enc.finish()]);
        let sub = tsub.elapsed().as_secs_f64();
        lap!("submit");
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
        lap!("post");
        let tpres = Instant::now();
        gpu.queue.present(frame);
        let pres = tpres.elapsed().as_secs_f64();
        lap!("present");
        let tail0 = Instant::now();
        if self.bug_grab {
            let shot = if bug_now { Some(self.read_frame()) } else { None };
            self.bug_shot = shot;
            (self.bug_grab, self.bug) = (false, Some(String::new()));
        }
        if cap_now && self.shot.is_some() {
            self.take_shot();
            self.finish();
        } else if cap_now {
            self.capture_frame(t);
        }

        // a filmstrip grab stalls on a readback: keep that frame and the next out of the numbers
        let skip = cap_now || bug_now || self.cap_prev;
        self.cap_prev = cap_now || bug_now;
        self.recent.push_back((dt * 1000.0) as f32);
        if self.recent.len() > 240 {
            self.recent.pop_front();
        }
        if self.f3 && self.streamer.frame % 30 == 0 {
            self.mem = (sys::ram_mb(), sys::vram_mb());
        }
        if measuring && skip {
            self.samples.capture_stalls += 1;
        }
        if measuring && !skip {
            let s = &mut self.samples;
            // this frame's interval is the last frame's stretches and the wait before this one (ADR 0025)
            let mut fs = self.seg_prev;
            fs[CPU_STAGES.len() - 1] = between;
            s.cpu_st.resize(CPU_STAGES.len(), Vec::new());
            for (v, x) in s.cpu_st.iter_mut().zip(fs) {
                v.push((x * 1000.0) as f32);
            }
            // every frame over the 120 fps budget names where its time went (ADR 0004); the GPU's stages are
            // the last frame's, filled in when they are read back
            if dt * 1000.0 > 8.3 && s.spikes.len() < 50 {
                let stages: Vec<String> = CPU_STAGES.iter().zip(fs).map(|(n, x)| format!("\"{n}\":{:.2}", x * 1000.0)).collect();
                s.spikes.push((
                    self.streamer.frame - 1,
                    format!(
                        "\"t\":{:.2},\"frame_ms\":{:.2},\"cpu_ms\":{:.2},\"select_ms\":{:.2},\"submit_ms\":{:.2},\"present_ms\":{:.2},\"prev_tail_ms\":{:.2},\"between_ms\":{:.2},\"chunks_applied\":{},\"cpu\":{{{}}}",
                        t,
                        dt * 1000.0,
                        (cpu_a + cpu_b) * 1000.0,
                        sel * 1000.0,
                        sub * 1000.0,
                        pres * 1000.0,
                        self.tail_prev * 1000.0,
                        between * 1000.0,
                        st.applied,
                        stages.join(",")
                    ),
                    None,
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
            if (self.streamer.idle() && st.missing == 0 && rt_pending == 0 && self.streamer.frame > 10) || el > 60.0 {
                eprintln!("[technoape] loaded in {:.2} s, {} chunks, {} quads", el, self.streamer.slot_count(), self.streamer.alloc.used);
                self.loaded_at = Some(Instant::now());
                self.jobs_at_load = (self.streamer.job_us_total, self.streamer.jobs_done as u64);
                if self.run == "play" {
                    self.chat_line(format!("TechnoApe {BUILD}: T chats, / runs a command (/help), F3 shows the numbers, F9 reports a bug"));
                }
                self.last_pos = p.pos;
            }
        }
        lap!("tail");
        self.seg_prev = seg;
        let end = mark;
        self.tail_prev = (end - tail0).as_secs_f64();
        self.frame_end = Some(end);
    }

    /// The frame copied to the capture buffer, at full size.
    fn read_frame(&self) -> image::RgbaImage {
        let gpu = self.gpu.as_ref().unwrap();
        let buf = gpu.capture.as_ref().unwrap();
        buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        let data = buf.slice(..).get_mapped_range().unwrap();
        let bgra = matches!(gpu.config.format, wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb);
        let mut img = image::RgbaImage::new(w(), h());
        let row = row_bytes() as usize;
        for (x, y, px) in img.enumerate_pixels_mut() {
            let i = y as usize * row + x as usize * 4;
            let d = &data[i..i + 4];
            *px = image::Rgba(if bgra { [d[2], d[1], d[0], 255] } else { [d[0], d[1], d[2], 255] });
        }
        drop(data);
        buf.unmap();
        img
    }

    fn capture_frame(&mut self, t: f64) {
        let every: f64 = self.args.get("capture_ms", 0.0) / 1000.0;
        self.next_cap = t + every;
        let full = self.read_frame();
        let tw: u32 = self.args.get("capture_w", 384);
        let th = tw * h() / w();
        let img = image::RgbaImage::from_fn(tw, th, |x, y| *full.get_pixel(x * w() / tw, y * h() / th));
        self.sheet.push(img);
    }

    /// shot= (ADR 0018): counts cracks, the clear colour where a ray from the camera must meet a body (it
    /// passes inside the lowest terrain any column can have), then stores the frame at a third of its size
    /// and counts the pixels that differ from baseline= by more than 8 of 255 in a channel.
    fn take_shot(&mut self) {
        let img = self.read_frame();
        let (pos, fwd, up) = self.cam;
        let right = fwd.cross(up).normalize();
        let cu = right.cross(fwd);
        let th = (self.fov().to_radians() / 2.0).tan();
        let aspect = w() as f64 / h() as f64;
        let mut cracks = 0u64;
        let mut at: Vec<String> = Vec::new();
        for (x, y, px) in img.enumerate_pixels() {
            if px.0[..3] != [255, 0, 255] {
                continue;
            }
            let nx = (2.0 * (x as f64 + 0.5) / w() as f64 - 1.0) * th * aspect;
            let ny = (1.0 - 2.0 * (y as f64 + 0.5) / h() as f64) * th;
            let d = (fwd + right * nx + cu * ny).normalize();
            let hits = |c: DVec3, r: f64| {
                let oc = pos - c;
                let (b, cc) = (oc.dot(d), oc.dot(oc) - r * r);
                b * b - cc >= 0.0 && -b + (b * b - cc).sqrt() > 0.0
            };
            if self.world.bodies.iter().any(|b| hits(b.centre, b.radius - engine::world::AMP - 1.0)) {
                cracks += 1;
                if at.len() < 8 {
                    at.push(format!("[{x},{y}]"));
                    // where the ray meets the terrain the generator makes, to find the hole in the world
                    let mut t = 0.0;
                    while t < 5000.0 {
                        let q = pos + d * t;
                        let b = &self.world.bodies[self.world.nearest(q)];
                        let r = (q - b.centre).length();
                        if r < b.height((q - b.centre) / r) {
                            eprintln!("[technoape] crack [{x},{y}] meets the ground {t:.2} m away at {:?}: {:?}", q.to_array(), self.world.block_at(q));
                            break;
                        }
                        t += 0.02;
                    }
                }
            }
        }
        let small = image::imageops::resize(&img, w() / 3, h() / 3, image::imageops::FilterType::Triangle);
        let base = self.args.0.get("baseline").and_then(|b| image::open(b).ok()).map(|i| i.to_rgba8());
        let diff = base.filter(|b| b.dimensions() == small.dimensions()).map(|b| {
            let n = small.pixels().zip(b.pixels()).filter(|(p, q)| p.0.iter().zip(q.0.iter()).any(|(u, v)| u.abs_diff(*v) > 8)).count();
            n as f64 / (small.width() * small.height()) as f64 * 100.0
        });
        let path = self.shot.clone().unwrap();
        if let Some(d) = std::path::Path::new(&path).parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Err(e) = small.save(&path) {
            eprintln!("[technoape] shot {path}: {e}");
        }
        self.shot_json = format!("\"shot\":{{\"file\":{path:?},\"cracks\":{cracks},\"crack_px\":[{}],\"diff_pct\":{}}},", at.join(","), diff.map_or("null".into(), |d| format!("{d:.3}")));
    }

    /// The server's state after step `step` (ADR 0019): the steps it has not taken yet are replayed from it, and
    /// where that lands is where this player is. A difference is a correction, drawn eased out unless it is a
    /// jump of metres (a teleport, a respawn). The first answer places the player.
    fn reconcile(&mut self, step: u32, server: phys::Player, t: f64) {
        if !self.spawned {
            (self.me, self.prev, self.spawned) = (server, server.pos, true);
            self.steps.retain(|s| s.0 > step);
            return;
        }
        self.steps.retain(|s| s.0 > step);
        // each step again at the time it was first taken (the server takes it at that time too)
        let replay = |world: &mut World, steps: &VecDeque<(u32, Input)>| {
            let mut me = server;
            for (_, inp) in steps {
                phys::step_at(world, &mut me, inp, phys::DT);
            }
            me
        };
        let me = replay(&mut self.world, &self.steps);
        let off = self.me.pos - me.pos;
        if off.length() >= 2.0 {
            // a teleport (/goto, a respawn): the look is the server's, in the frame it lands in, level; the
            // steps not answered yet only turn it
            (self.pitch, self.tilt, self.roll) = (0.0, 0.0, 0.0);
        }
        self.drift_max = self.drift_max.max(off.length());
        if off.length() > 0.001 {
            self.snap_t.push((t, off.length()));
            self.snaps += 1;
            self.path.clear(); // a correction is counted as one, not as jerk
            self.smooth = if off.length() < 2.0 { self.smooth + off } else { DVec3::ZERO };
            self.prev -= off;
        }
        self.me = me;
    }

    /// run=play's feel report (fundamentals section 1, ADR 0018) as a JSON field, or nothing in other runs.
    fn feel(&self) -> String {
        if self.run != "play" {
            return String::new();
        }
        let mut mags: Vec<f32> = self.jerks.iter().map(|j| j.1 as f32).collect();
        let mut top = self.jerks.clone();
        top.sort_by(|a, b| b.1.total_cmp(&a.1));
        let top: Vec<String> = top.iter().take(5).map(|(t, j)| format!("{{\"t\":{t:.2},\"jerk\":{j:.0}}}")).collect();
        let frames = self.shown.iter().map(|c| c.0).max().unwrap_or(0);
        let ms = self.shown.iter().map(|c| c.1).fold(0.0, f64::max);
        format!(
            "\"feel\":{{\"corrections\":{},\"corrections_at\":[{}],\"drift_max_m\":{:.3},\"jerk_p50\":{:.0},\"jerk_p99\":{:.0},\"jerk_max\":{:.0},\"jerk_top\":[{}],\"clicks\":{},\"clicks_not_shown\":{},\"click_to_change_frames_max\":{},\"click_to_change_ms_max\":{:.2}}},",
            self.snaps,
            self.snap_t.iter().map(|(t, d)| format!("{{\"t\":{t:.2},\"m\":{d:.2}}}")).collect::<Vec<_>>().join(","),
            self.drift_max,
            pct(&mut mags, 0.5),
            pct(&mut mags, 0.99),
            pct(&mut mags, 1.0),
            top.join(","),
            self.shown.len(),
            self.clicks.len(),
            frames,
            ms
        )
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
        let later = (self.streamer.jobs_done as u64).saturating_sub(self.jobs_at_load.1);
        let job_us_after = (self.streamer.job_us_total - self.jobs_at_load.0) / later.max(1) as f64;
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
            s.spikes.iter().map(|(_, c, g)| format!("{{{c},\"gpu\":{{{}}}}}", g.as_deref().unwrap_or(""))).collect::<Vec<_>>().join(",")
        );
        // ADR 0025: every stage's p50 and p99, the frame thread's and the GPU's
        let mut stages = String::new();
        for (key, names, v) in [("cpu", &CPU_STAGES[..], &mut s.cpu_st), ("gpu", &GPU_STAGES[..], &mut s.gpu_st)] {
            v.resize(names.len(), Vec::new());
            for p in [0.5, 0.99] {
                let f: Vec<String> = names.iter().zip(v.iter_mut()).map(|(n, x)| format!("\"{n}\":{:.3}", pct(x, p))).collect();
                stages += &format!("\"{key}_p{}\":{{{}}},", (p * 100.0) as u32, f.join(","));
            }
        }
        // the shot's and the feel report's fields go in before the spikes
        // what wgpu itself holds at the end, finer than the driver's 256 MB blocks in vram_mb (ADR 0025, G)
        if let Some(r) = gpu.device.generate_allocator_report() {
            stages += &format!("\"gpu_alloc_mb\":{:.0},\"gpu_reserved_mb\":{:.0},", r.total_allocated_bytes as f64 / 1e6, r.total_reserved_bytes as f64 / 1e6);
        }
        stages += &format!("\"job_us_after_load\":{job_us_after:.0},\"jobs_after_load\":{later},\"walks\":{},\"tlas_builds\":{},", self.streamer.walks, self.rt.as_ref().map_or(0, |r| r.tlas_builds));
        let line = line.replacen("\"spikes\":", &format!("{}{}{}\"spikes\":", self.shot_json, self.feel(), stages), 1);
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
        self.onscreen = onscreen;
        // play on screen as the settings say (ADR 0023): a window with a title bar, or fullscreen
        let play = onscreen && self.run == "play";
        let (fs, (fw, fh)) = if play { fullscreen(&self.settings, el.primary_monitor()) } else { (None, (w(), h())) };
        SIZE.store((fw as u64) << 32 | fh as u64, Ordering::Relaxed);
        let attrs = Window::default_attributes()
            .with_fullscreen(fs.clone())
            .with_title("TechnoApe")
            .with_inner_size(winit::dpi::PhysicalSize::new(w(), h()))
            .with_resizable(false)
            .with_decorations(play && fs.is_none())
            .with_position({
                // off-screen runs keep the window far outside every monitor
                let at = if onscreen { 0 } else { -30000 };
                winit::dpi::PhysicalPosition::new(at, at)
            });
        let window = Arc::new(el.create_window(attrs).unwrap());
        if self.run == "play" && onscreen {
            let _ = window.set_cursor_grab(CursorGrabMode::Confined).or_else(|_| window.set_cursor_grab(CursorGrabMode::Locked));
            window.set_cursor_visible(false);
        }
        let cap: u32 = self.args.get("quad_cap_m", 16u32) * 1_000_000;
        let (gpu, rt, gi) = Gpu::new(window, &self.args, cap);
        self.gpu = Some(gpu);
        self.rt = Some(rt);
        self.gi = Some(gi);
        self.start = Instant::now();
        self.last_frame = Instant::now();
        if self.home() == Menu::Title {
            self.set_menu(Some(Menu::Title));
        }
    }
    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, ev: WindowEvent) {
        match ev {
            WindowEvent::CloseRequested => {
                self.finish();
                el.exit();
            }
            WindowEvent::KeyboardInput { event: KeyEvent { physical_key: PhysicalKey::Code(code), state, text, .. }, .. } => {
                if self.texting() {
                    // the chat's bar or the bug report's form takes the keys; Esc closes it
                    if state == ElementState::Pressed {
                        match code {
                            KeyCode::Escape => self.act("cancel"),
                            KeyCode::Enter | KeyCode::NumpadEnter => self.act("enter"),
                            KeyCode::Backspace => self.act("back"),
                            KeyCode::ArrowUp => self.act("earlier"),
                            KeyCode::ArrowDown => self.act("later"),
                            KeyCode::Tab => self.act("tab"),
                            _ => {
                                if let Some(t) = text.filter(|t| !t.chars().any(char::is_control)) {
                                    self.act(&format!("type:{t}"));
                                }
                            }
                        }
                    }
                    return;
                }
                if self.menu.is_some() {
                    // the menus take Esc; F3 and F9 (a bug report) work in them too
                    if state == ElementState::Pressed {
                        match code {
                            KeyCode::Escape => self.act("esc"),
                            KeyCode::F3 => self.act("f3"),
                            KeyCode::F9 => self.act("f9"),
                            _ => {}
                        }
                    }
                    return;
                }
                if code == KeyCode::Escape && self.run != "play" {
                    self.finish();
                    el.exit();
                } else if code == KeyCode::Escape {
                    if state == ElementState::Pressed {
                        self.act("esc");
                    }
                } else if state == ElementState::Pressed {
                    let key = match code {
                        KeyCode::KeyH => Some("h".to_string()),
                        KeyCode::KeyB => Some("b".to_string()),
                        KeyCode::KeyV => Some("v".to_string()),
                        KeyCode::KeyT => Some("t".to_string()),
                        KeyCode::Space => Some("space".to_string()),
                        KeyCode::F3 => Some("f3".to_string()),
                        KeyCode::F9 => Some("f9".to_string()),
                        KeyCode::Period => Some("warp+".to_string()),
                        KeyCode::Comma => Some("warp-".to_string()),
                        _ if text.as_deref() == Some("/") => Some("/".to_string()),
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
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer = (position.x as f32, position.y as f32);
                self.slide();
            }
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. } if self.drag.is_some() => {
                self.drag = None;
                self.save_settings();
            }
            WindowEvent::Resized(size) if self.run == "play" && self.onscreen => self.resize(size.width, size.height),
            WindowEvent::MouseInput { state: ElementState::Pressed, button, .. } if self.menu.is_some() && !self.texting() => {
                self.menu_click(button == MouseButton::Right);
            }
            WindowEvent::MouseInput { .. } | WindowEvent::MouseWheel { .. } if self.texting() || self.menu.is_some() => {}
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
        if let (DeviceEvent::MouseMotion { delta }, false) = (ev, self.texting() || self.menu.is_some()) {
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
    let mut args = Args(
        std::env::args()
            .skip(1)
            .filter_map(|a| a.split_once('=').map(|(k, v)| (k.to_string(), v.to_string())))
            .collect(),
    );
    if args.0.is_empty() {
        // started without arguments (a double click, playtest/): the title menu on screen, in the repository the
        // exe sits in, so saves/ and bugs/ are the usual ones
        args.0.extend(TITLE_ARGS.iter().filter_map(|a| a.split_once('=')).map(|(k, v)| (k.to_string(), v.to_string())));
        let exe = std::env::current_exe().unwrap_or_default();
        if let Some(r) = exe.ancestors().skip(1).find(|d| d.join("Cargo.toml").exists() && d.join("crates").is_dir()) {
            let _ = std::env::set_current_dir(r);
        }
    }
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
    // (planet= and moon= are columns per face edge: 12288 is the 4 km planet, the standard; 24576 the 8 km huge
    // one; 1536 the benchmark's 500 m class)
    let mut local_server = None;
    let (mut save_path, mut rec_path) = (None, None);
    let link: Link<ToServer, ToClient> = match args.0.get("connect") {
        Some(addr) => {
            let addr = if addr.contains(':') { addr.clone() } else { format!("{addr}:{}", net::DEFAULT_PORT) };
            net::tcp(std::net::TcpStream::connect(&addr).expect("connect to the server")).expect("link")
        }
        None => {
            let (accept_tx, accept_rx) = std::sync::mpsc::channel();
            let (planet, moon, seed) = (args.get("planet", 12288), args.get("moon", 512), args.get("seed", 1337));
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
            server.autosave = save.clone().map(|p| (p.into(), 30 * TICK_HZ as u64));
            save_path = save;
            if let Some(r) = rec.filter(|r| r != "0") {
                rec_path = Some(r.clone());
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
    let (_id, mut world, seed) = match link.rx.recv_timeout(Duration::from_secs(10)) {
        Ok(ToClient::Welcome { id, seed, planet_n, moon_n, blocks }) => {
            assert_eq!(blocks, engine::blocks::BLOCKS.hash, "the server's build has other blocks (data/blocks.toml)");
            (id, World::new(planet_n, moon_n, seed), seed)
        }
        other => panic!("no welcome from the server: {other:?}"),
    };
    // the server spawns players on the +y pole; the client starts its prediction there too
    let spawn = world.bodies[0].spawn_point();
    assert!(world.bodies.len() <= MAX_BODIES);
    // place=di:dj:dl:block,... (a name, _ for a space) sets blocks around the stand point, the air block at its
    // feet the origin: lamps or metal in a shot without a session. Only the client's world has them.
    if run != "play" {
        let at: Vec<f64> = args.s("at", "0,1,0").split(',').filter_map(|v| v.parse().ok()).collect();
        let up = DVec3::new(at[0], at[1], at[2]).normalize();
        let feet = world.block_at(up * (world.bodies[0].top(up) + 0.1));
        for spec in args.s("place", "").split(',').filter(|s| !s.is_empty()) {
            let f: Vec<&str> = spec.split(':').collect();
            let d = IVec3::new(f[0].parse().unwrap(), f[1].parse().unwrap(), f[2].parse().unwrap());
            world.set_block(Block { pos: feet.pos + d, ..feet }, BLOCKS.id(&f[3].replace('_', " ")));
        }
    }
    // play's settings (ADR 0023); every other run keeps the defaults
    let settings_file = args.s("settings", "settings.txt");
    let settings = if run == "play" { Settings::parse(&std::fs::read_to_string(&settings_file).unwrap_or_default()) } else { Settings::default() };
    SIZE.store((settings.width as u64) << 32 | settings.height as u64, Ordering::Relaxed);
    let opts = Opts {
        k: args.get("k", settings.detail),
        // merging inside a lattice with interpolated corners (ADR 0025 I2); greedy=0 draws every face
        greedy: args.get("greedy", 1) == 1,
        horizon: args.get("horizon", 1) == 1,
        prefetch_s: args.get("prefetch", 0.5),
        max_inflight: args.get("inflight", threads * 4),
    };
    let cap: u32 = args.get("quad_cap_m", 16u32) * 1_000_000;
    let streamer = Streamer::new(&world, cap, opts);
    let el = EventLoop::new().unwrap();
    el.set_control_flow(ControlFlow::Poll);
    let (acts, creative, shot) = (parse_acts(&args.s("acts", "")), run != "play", args.0.get("shot").cloned());
    let time0 = args.get("time", if run == "orbit" { orbit_start(&world) } else { 0.0 });
    let server_at = args.0.get("connect").map_or("on this machine".to_string(), |a| a.clone());
    let mut app = App {
        time0,
        args,
        gpu: None,
        rt: None,
        slots: ChunkSlots::default(),
        gi: None,
        lamps: Vec::new(),
        lamps_at: (0, u64::MAX),
        world,
        link,
        me: phys::Player::at(spawn, DVec3::X),
        prev: spawn,
        steps: Default::default(),
        next_step: 1,
        unsent: 1,
        spawned: false,
        smooth: DVec3::ZERO,
        debt: 0.0,
        drift_max: 0.0,
        keys: Default::default(),
        mouse: (0.0, 0.0),
        pitch: 0.0,
        turn: 0.0,
        tilt: 0.0,
        roll: 0.0,
        input: Input::default(),
        snaps: 0,
        tail_prev: 0.0,
        frame_end: None,
        last_move: Instant::now(),
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
        bucket: false,
        aim: None,
        held: None,
        slot: 0,
        creative,
        inv: Vec::new(),
        bucket_water: 0,
        water: water::WaterMesh::default(),
        wquad_n: 0,
        ghost_of: None,
        ghost_n: 0,
        ui_n: 0,
        acts,
        title: String::new(),
        shot,
        shot_json: String::new(),
        cam: (DVec3::ZERO, DVec3::X, DVec3::Y),
        clicks: Vec::new(),
        shown: Vec::new(),
        path: Vec::new(),
        jerks: Vec::new(),
        snap_t: Vec::new(),
        last_pos: DVec3::ZERO,
        sheet: Vec::new(),
        next_cap: 0.0,
        done: false,
        cap_prev: false,
        chat: Vec::new(),
        typing: None,
        sent: Vec::new(),
        recall: None,
        pick: None,
        zoom: 1.0,
        clock: None,
        f3: false,
        recent: VecDeque::new(),
        gpu_last: 0.0,
        ts_frames: [0; TS_RING],
        gpu_end: None,
        seg_prev: [0.0; CPU_STAGES.len()],
        jobs_at_load: (0.0, 0),
        mem: (sys::ram_mb(), sys::vram_mb()),
        drawn: Drawn::default(),
        bug_grab: false,
        bug: None,
        bug_shot: None,
        bug_said: std::sync::mpsc::channel(),
        settings,
        settings_file,
        menu: None,
        pointer: (0.0, 0.0),
        menu_boxes: Vec::new(),
        drag: None,
        edit: None,
        onscreen: false,
        flying: false,
        fly_speed: 11.0,
        space_at: None,
        seed,
        server_at,
        save_path,
        rec_path,
        worlds: Vec::new(),
        next: None,
    };
    el.run_app(&mut app).unwrap();
    // the local server saves as it stops
    if let Some((stop, thread)) = local_server {
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = thread.join();
    }
    // a world picked in the menu, or back to the title: the game again, once the old save is written
    if let Some(next) = app.next.take() {
        drop(app);
        let exe = std::env::current_exe().expect("the game's own path");
        if let Err(e) = std::process::Command::new(exe).args(&next).spawn() {
            eprintln!("[technoape] could not start again with {next:?}: {e}");
        }
    }
}
