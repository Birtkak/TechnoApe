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

/// The plants' wind map (ADR 0157; binding 19, the shader's `plant_sway`): a field's frames along the
/// third axis, its push in two signed channels, linear and repeating in all three (`sway_samp`); none, 1 x 1 x 1.
pub(crate) fn sway_view(device: &wgpu::Device, queue: &wgpu::Queue, w: Option<&crate::fx::Wind>) -> wgpu::TextureView {
    use wgpu::util::DeviceExt;
    let (side, frames) = w.map_or((1, 1), |w| (w.field.side as u32, w.field.frames as u32));
    let texels: &[i8] = w.map_or(&[0, 0], |w| &w.texels);
    device
        .create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("sway"),
                size: wgpu::Extent3d { width: side, height: side, depth_or_array_layers: frames },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D3,
                format: wgpu::TextureFormat::Rg8Snorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            bytemuck::cast_slice(texels),
        )
        .create_view(&Default::default())
}

/// The sway's watcher: `[sway]` of the effects file and the map it names, its field worked out on the watcher's
/// thread (again only when the map's text changed); none without `[sway]`, or with its map refused (said on
/// stderr: the plants keep the sines).
/// `Globals::sway` for a wind: 1 / tile, its loops an hour of the sky's clock, on; none, all 0 (the sines).
fn sway_globals(w: Option<&crate::fx::Wind>) -> [f32; 4] {
    w.map_or([0.0; 4], |w| [1.0 / w.tile, (3600.0 / w.field.seconds).round().max(1.0), 1.0, 0.0])
}

fn watch_sway(effects: std::path::PathBuf) -> std::sync::mpsc::Receiver<Option<crate::fx::Wind>> {
    let kept = std::cell::RefCell::new(None);
    watch("sway", crate::fx::maps_dir().to_path_buf(), vec![effects.clone()], move |maps| {
        let pal = std::fs::read_to_string("data/palette.toml").ok().and_then(|t| engine::palette::Palette::parse(&t).ok()).unwrap_or_else(|| crate::fx::PALETTE.clone());
        let got = crate::fx::read_sway(&effects, maps, &pal, &mut kept.borrow_mut()).unwrap_or_else(|e| {
            eprintln!("[technoape] sway: {e}; the plants sway by the sines");
            None
        });
        (got, true)
    })
}

/// The conformal grid's warp table (ADR 0131, engine `warp.rs`): rg32float, x along b, y along a, read with
/// `textureLoad` (binding 18 of the terrain's group 0; the shader's `warp_at`).
pub(crate) fn warp_view(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::TextureView {
    use wgpu::util::DeviceExt;
    let n = engine::warp::N as u32;
    device
        .create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("warp"),
                size: wgpu::Extent3d { width: n, height: n, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rg32Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            engine::warp::BYTES,
        )
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
/// The most bodies the GPU holds (the size of `bodies` in shader.wgsl, whose body ids are masked by `& 3u`):
/// a stated limit (0163-n-bodies), checked at start by `bodies_fit`.
pub(crate) const MAX_BODIES: usize = engine::bodies::MAX;

/// Whether `n` bodies (`data/bodies.toml`'s rows) fit on the GPU; a world of more is refused at start, never
/// drawn with a body id wrapped onto another body.
pub(crate) fn bodies_fit(n: usize) -> Result<(), String> {
    if n <= MAX_BODIES { Ok(()) } else { Err(format!("data/bodies.toml has {n} bodies; the renderer holds at most {MAX_BODIES} (gpu.rs MAX_BODIES, the `bodies` of shader.wgsl)")) }
}
/// The full moon's light, a quarter of the sun's direct light and bluish: night stays readable (ADR 0022).
pub(crate) const MOON_LIGHT: Vec3 = Vec3::new(0.12, 0.14, 0.18);
/// The lamps the GPU holds: the nearest to the camera within LAMP_REACH metres (ADR 0022; 1024 since ADR 0025, H).
pub(crate) const MAX_LAMPS: usize = 1024;
/// The clouds' map's side in texels (clouds.wgsl CL_TEX): the most air columns a face edge.
pub(crate) const SKY_TEX: u32 = 128;
pub(crate) const LAMP_REACH: f64 = 48.0;
/// The lamp grid (ADR 0025, H): LAMP_G^3 cells of LAMP_CELL metres in the camera's frame, the camera in the
/// middle, each a range into a list of the lamps that reach it: LAMP_R (as in shader.wgsl) or nearer, so a lamp
/// is listed in at most 7^3 cells.
pub(crate) const LAMP_G: usize = 32;
// (8 m cells since the reach doubled, ADR 0072: a lamp still spans at most 7 cells an axis)
pub(crate) const LAMP_CELL: f32 = 8.0;
pub(crate) const LAMP_R: f32 = 24.0;
pub(crate) const LAMP_GRID_LEN: usize = LAMP_G * LAMP_G * LAMP_G + 1 + MAX_LAMPS * 343;

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

/// The world's lamps with the effects' and the models' states' added (ADR 0050), held to the Lamps setting's
/// `cap` in all: when there are more, the nearest to the camera, as `gather_lamps` keeps them.
pub(crate) fn cap_lamps<'a>(all: impl Iterator<Item = &'a (u8, DVec3, [f32; 3])>, cams: &[DVec3], cap: usize) -> Vec<(u8, DVec3, [f32; 3])> {
    let mut out: Vec<_> = all.copied().collect();
    // (sorted only when over: the usual frame keeps the world's own order, already nearest first)
    if out.len() > cap {
        let d = |l: &(u8, DVec3, [f32; 3])| (l.1 - cams[l.0 as usize]).length_squared();
        out.sort_by(|a, b| d(a).total_cmp(&d(b)));
        out.truncate(cap);
    }
    out
}

/// The lamp of the block in hand (bug 2026-11-13_14-31-01): a block that gives light lights the world from the
/// hand, 0.4 m ahead of the eye and 0.3 m under it, as `cap_lamps` takes it (body `cf`, world place, light).
/// A flame (a torch, ADR 0117) under water there is out: none.
pub(crate) fn held_lamp(world: &World, d: Option<&engine::blocks::Def>, cf: u8, eye: DVec3, fwd: DVec3, up: DVec3) -> Option<(u8, DVec3, [f32; 3])> {
    let at = eye + fwd * 0.4 - up * 0.3;
    d.filter(|d| d.light != [0.0; 3] && !(d.flame && world.water_surface(at).is_some())).map(|d| (cf, at, d.light))
}

/// The lamps the other players hold (ADR 0117): from each one's roster entry (`net::Player::hand`, once a
/// second), at its hand as `held_lamp` puts this player's, its eye 1.5 m over its feet, each with its holder's
/// id; `me` is left out.
/// (shortcut: a light moves with the roster's once a second; with the players' own moves when they are sent)
pub(crate) fn others_lamps(world: &World, others: &[engine::net::Player], me: u32) -> Vec<(u32, (u8, DVec3, [f32; 3]))> {
    others
        .iter()
        .filter(|o| o.id != me && o.hand != 0)
        .filter_map(|o| {
            let pos = DVec3::from_array(o.pos);
            let body = world.frame_at(pos);
            let up = (pos - world.bodies.get(body)?.centre).normalize_or(DVec3::Z);
            let fwd = DVec3::from_array(o.fwd.map(f64::from));
            let fwd = (fwd - up * fwd.dot(up)).normalize_or(up.any_orthonormal_vector());
            held_lamp(world, engine::blocks::BLOCKS.get(o.hand), body as u8, pos + up * 1.5, fwd, up).map(|l| (o.id, l))
        })
        .collect()
}

#[cfg(test)]
mod lamp_tests {
    use super::*;

    #[test]
    fn the_lamps_of_effects_and_models_count_against_the_cap() {
        let lamp = |d: f64| (0u8, DVec3::X * d, [1.0f32; 3]);
        let world = [lamp(1.0), lamp(2.0)];
        let extra = [lamp(9.0), lamp(0.5)];
        let kept = cap_lamps(world.iter().chain(&extra), &[DVec3::ZERO], 2);
        let ds: Vec<f64> = kept.iter().map(|l| l.1.x).collect();
        assert_eq!(ds, [0.5, 1.0], "two in all, the nearest");
        assert_eq!(cap_lamps(world.iter(), &[DVec3::ZERO], 16).len(), 2, "under the cap: all");
    }

    /// Bug 2026-11-13_14-31-01: a block that gives light lights the world from the hand, in its light's colour,
    /// a little ahead of the eye; one that does not, nothing.
    #[test]
    fn a_held_lamp_gives_light_from_the_hand() {
        let b = &engine::blocks::BLOCKS;
        let w = World::new(768, 256, 1);
        let up = w.bodies[0].spawn;
        let eye = w.bodies[0].centre + up * (w.bodies[0].height(up) + 1.6);
        let fwd = up.any_orthonormal_vector();
        for name in ["lamp", "torch"] {
            let lamp = b.get(b.id(name)).unwrap();
            let (body, at, light) = held_lamp(&w, Some(lamp), 0, eye, fwd, up).expect("it gives light");
            assert_eq!((body, light), (0, lamp.light), "{name}: by its row");
            assert!((at - eye).length() < 1.0 && (at - eye).dot(fwd) > 0.0, "in hand, ahead of the eye: {at}");
        }
        assert!(held_lamp(&w, b.get(b.id("dirt")), 0, eye, fwd, up).is_none(), "dirt gives none");
        assert!(held_lamp(&w, None, 0, eye, fwd, up).is_none(), "an empty hand none");
    }

    /// ADR 0117: a torch held under water is out; a lamp there stays lit.
    #[test]
    fn a_held_torch_is_out_under_water() {
        let b = &engine::blocks::BLOCKS;
        let w = World::new(768, 256, 1);
        let body = &w.bodies[0];
        let sea = body.layer_r(body.sea.unwrap());
        // (a spiral of directions over the whole planet, the first over deep sea)
        let d = (0..20000).map(|k| { let z = 1.0 - (k as f64 + 0.5) / 10000.0; let a = k as f64 * 2.399963; let r = (1.0 - z * z).sqrt(); DVec3::new(r * a.cos(), r * a.sin(), z) }).find(|d| body.height(*d) < sea - 4.0).expect("a sea");
        let eye = body.centre + d * (sea - 1.0);
        let fwd = d.any_orthonormal_vector();
        assert!(w.water_surface(eye + fwd * 0.4 - d * 0.3).is_some(), "the hand is under water");
        assert!(held_lamp(&w, b.get(b.id("torch")), 0, eye, fwd, d).is_none(), "the torch is out");
        assert!(held_lamp(&w, b.get(b.id("lamp")), 0, eye, fwd, d).is_some(), "the lamp is lit");
    }

    /// ADR 0117: another player's held torch is a lamp in this client's list, at its hand; this player's own
    /// roster entry and an empty or dark hand are none.
    #[test]
    fn another_players_held_torch_is_a_lamp_here() {
        let b = &engine::blocks::BLOCKS;
        let w = World::new(768, 256, 1);
        let up = w.bodies[0].spawn;
        let feet = w.bodies[0].centre + up * w.bodies[0].height(up);
        let p = |id: u32, hand: u16| engine::net::Player { id, name: format!("p{id}"), pos: feet.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0, hand };
        let torch = b.id("torch");
        let got = others_lamps(&w, &[p(1, torch), p(2, torch), p(3, 0), p(4, b.id("dirt"))], 1);
        assert_eq!(got.len(), 1, "player 2's only: {got:?}");
        let (id, (body, at, light)) = got[0];
        assert_eq!(id, 2);
        assert_eq!((body, light), (0, b.get(torch).unwrap().light));
        let h = (at - feet).dot(up);
        assert!((0.9..1.5).contains(&h) && (at - feet).length() < 1.6, "at its hand: {h} m up");
    }
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

/// More bodies than the GPU holds are refused with a message naming the limit (0163-n-bodies); this build's
/// rows fit.
#[test]
fn more_bodies_than_the_gpu_holds_are_refused() {
    assert!(bodies_fit(engine::bodies::BODIES.len()).is_ok());
    assert!(bodies_fit(MAX_BODIES).is_ok());
    let e = bodies_fit(MAX_BODIES + 1).expect_err("five bodies refused");
    assert!(e.contains("5 bodies") && e.contains("at most 4"), "{e}");
}

#[cfg(test)]
mod texture_layer_tests {
    use super::*;
    use engine::textures::{Face, Painted};
    fn face(c: u8, variants: usize) -> Face {
        Face { variants: (0..variants).map(|v| [[c, v as u8, 0, 255]; 256]).collect(), fine: false, mean: [0.0; 3] }
    }
    // a face that falls back to the block's every-face file shares that file's layers
    #[test]
    fn faces_of_one_file_share_their_layers() {
        let all = face(1, 4);
        let blocks: Vec<((u16, usize), Painted)> = (1..=200u16).map(|id| ((id, 0), Painted { faces: [Some(all.clone()), Some(all.clone()), Some(all.clone())] })).collect();
        let (placed, layers, refused) = place(blocks, TEX_LAYERS);
        assert_eq!((placed.len(), layers.len(), refused.len()), (200, 800, 0), "4 layers a block, not 12");
        assert!(placed.iter().all(|(_, _, at)| at[0] == at[1] && at[1] == at[2]));
        // a top of its own takes its own layers; side and bottom still share
        let (placed, layers, _) = place(vec![((1, 0), Painted { faces: [Some(face(2, 1)), Some(all.clone()), Some(all.clone())] })], TEX_LAYERS);
        assert_eq!((placed[0].2, layers.len()), ([0, 1, 1], 5));
        assert_eq!((layers[0][0], layers[1][0], layers[4][0]), ([2, 0, 0, 255], [1, 0, 0, 255], [1, 3, 0, 255]));
        // the cap counts the layers taken, and a block past it is refused whole
        let (placed, layers, refused) = place(vec![((1, 0), Painted { faces: [Some(all.clone()), Some(all.clone()), None] }), ((2, 0), Painted { faces: [Some(face(3, 2)), None, None] })], 5);
        assert_eq!((placed.len(), layers.len(), refused), (1, 4, vec![(2, 0)]));
    }

    // a season's face that is its base's (the file it falls back to) takes the base's layers; its own its own
    #[test]
    fn a_season_shares_its_bases_layers() {
        let all = face(1, 4);
        let base = ((1, 0), Painted { faces: [Some(all.clone()), Some(all.clone()), Some(all.clone())] });
        let autumn = ((1, 2), Painted { faces: [Some(face(2, 1)), Some(all.clone()), Some(all.clone())] });
        let other = ((2, 2), Painted { faces: [Some(all.clone()), None, None] });
        let (placed, layers, _) = place(vec![base, autumn, other], TEX_LAYERS);
        assert_eq!(layers.len(), 4 + 1 + 4, "another block's same paint is its own");
        assert_eq!(placed.iter().map(|p| p.2).collect::<Vec<_>>(), vec![[0, 0, 0], [4, 0, 0], [5, 0, 0]]);
        // the look rows: a season its own paint, a season without its base's
        let t = looks_of(&placed);
        let top = |row: usize| t[row][13] & 0xffff;
        assert_eq!([top(1), top(IDS + 1), top(2 * IDS + 1), top(3 * IDS + 1)].map(|w| w & 4095), [0, 0, 4, 0]);
        assert!(top(1) != 0 && top(IDS + 1) != 0 && top(3 * IDS + 1) != 0, "summer, spring and winter take the base's paint");
        assert_eq!(top(2 * IDS + 2) & 4095, 5, "and another block's season its own");
        assert_eq!(top(2), 0, "with no base paint of its own");
        // bit 8 of size: seasons (a season's paint alone gives a block them); bit 9: a season paints its own
        let size = |row: usize| t[row][7];
        assert_eq!([size(1) >> 8 & 3, size(2) >> 8 & 3, size(3) >> 8 & 3], [3, 3, 0], "{:?}", [size(1), size(2), size(3)]);
    }
}

#[cfg(test)]
mod shown_tests {
    // the hotbar's cube of a block with seasons shows the season where the player stands, as shader.wgsl mixes it
    #[test]
    fn the_hotbar_shows_the_season_where_the_player_stands() {
        let d = engine::blocks::BLOCKS.placeable().find(|d| d.seasons.is_some() && d.tone(2).colour != d.colour).expect("a block with an autumn");
        let close = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5);
        assert!(close(super::shown(d, (1.0, 1.0)).0, d.colour), "summer is its own colour");
        assert!(close(super::shown(d, (2.0, 1.0)).0, d.tone(2).colour), "autumn");
        assert!(close(super::shown(d, (2.0, 0.0)).0, d.colour), "no season near the equator");
        let half = std::array::from_fn(|c| d.colour[c] + ((d.tone(2).colour[c] + d.tone(3).colour[c]) / 2.0 - d.colour[c]) * 0.5);
        assert!(close(super::shown(d, (2.5, 0.5)).0, half), "between autumn and winter, half a season");
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
    /// Its sea level as a layer (negative: none), and its looks' season: the year's phase in quarter turns for
    /// its northern half (`sky::season_at`), negative for a body without seasons.
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
    /// The planet's tide (ADR 0067, `sky::Tide`): Luna's and the sun's directions in its frame, w their pull
    /// in layers (1.5 * TIDE_LUNA and 1.5 * TIDE_SUN over a layer's thickness at the sea); its `base` is in
    /// body 0's sea level.
    pub(crate) tide: [[f32; 4]; 2],
    /// The water (ADR 0071): x its ripples' clock in s (the sky's, so a view is the same every time), y the
    /// fog per metre with the camera under water (0 above), z the light down there (0 night .. 1 noon).
    pub(crate) water: [f32; 4],
    /// The clouds (ADR 0076, clouds.wgsl): x the drift's phase 0 to 1, y its period s, z the planet's sea
    /// radius (0: none), w the map's air columns a face edge (0: no map yet).
    pub(crate) cloud: [f32; 4],
    /// The plants (ADR 0091): xyz the wind at the camera in body 0's frame, m/s (0 in a shot: a still picture
    /// shows them at rest), w how far away they shrink into the ground, m (the Vegetation setting).
    pub(crate) plants: [f32; 4],
    /// The plants' wind map (ADR 0157): x tiles a column (1 / `[sway] tile`), y its loops an hour of
    /// the sky's clock (whole, so the hour's wrap has no jump), z 1 with a map (0: the sines).
    pub(crate) sway: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ChunkGpu {
    pub(crate) base: [i32; 3],
    pub(crate) info: u32,
    pub(crate) off: u32,
    /// Its opaque land quads: a ray's hit in the second geometry (the cutout quads) is that many on (ADR 0069).
    pub(crate) opq: u32,
    /// The climate at its footprint's corners (engine `clutter::chunk_climate`, ADR 0091).
    pub(crate) clim: u32,
    /// Its first still quad (the stills' own BLAS, rt.rs): a ray's hit there is that many on.
    pub(crate) sto: u32,
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
/// An indexed draw's arguments (`multi_draw_indexed_indirect`): the terrain's quads over `quad_index` (ADR 0103).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct DrawIndexed {
    pub(crate) index_count: u32,
    pub(crate) instance_count: u32,
    pub(crate) first_index: u32,
    pub(crate) base_vertex: i32,
    pub(crate) first_instance: u32,
}
/// Quads one indexed draw holds at most: `quad_index`'s length in quads (ADR 0103).
pub(crate) const QUAD_INDEX: u32 = 1 << 16;
/// A quad's two triangles over its four corners (`quad_index`); each triangle's first corner is its provoking
/// vertex, which carries a slope's flat normal (ADR 0101, 0103).
pub(crate) const QUAD_TRIS: [u32; 6] = [0, 1, 2, 3, 0, 2];
/// Mirror of shader.wgsl's `corner_k`: the four corners as `quad_corner`'s k, plain and a slope split along the
/// other diagonal.
#[cfg(test)]
pub(crate) const CORNER_K: [[u32; 4]; 2] = [[0, 1, 2, 5], [3, 4, 5, 0]];
/// The regions of `indirect`, bytes: the sea's draws (ADR 0071), then MAX_CHUNKS indexed draws each for the
/// terrain, the cutout blocks (ADR 0069), glass (ADR 0081), the still models (ADR 0082) and the fluff (ADR 0103).
pub(crate) const IND_SEA: u64 = 0;
pub(crate) const IND_LAND: u64 = MAX_CHUNKS as u64 * 16;
pub(crate) const IND_CUT: u64 = IND_LAND + MAX_CHUNKS as u64 * 20;
pub(crate) const IND_GLASS: u64 = IND_CUT + MAX_CHUNKS as u64 * 20;
pub(crate) const IND_STILL: u64 = IND_GLASS + MAX_CHUNKS as u64 * 20;
pub(crate) const IND_FLUFF: u64 = IND_STILL + MAX_CHUNKS as u64 * 20;

pub(crate) struct Gpu {
    /// The clouds' map (ADR 0076), filled by `sky::Sky::upload`.
    pub(crate) sky_tex: wgpu::Texture,
    pub(crate) sky_view: wgpu::TextureView,
    pub(crate) sky_samp: wgpu::Sampler,
    /// The warp's table (`warp_view`).
    pub(crate) warp_view: wgpu::TextureView,
    /// The plants' wind map (`sway_view`), its sampler, the field it holds and the watcher's newest; `sway` is
    /// `Globals::sway`.
    pub(crate) sway_view: wgpu::TextureView,
    pub(crate) sway_samp: wgpu::Sampler,
    pub(crate) sway_field: Option<Arc<engine::map::Field>>,
    pub(crate) sway_rx: std::sync::mpsc::Receiver<Option<crate::fx::Wind>>,
    pub(crate) sway: [f32; 4],
    pub(crate) window: Arc<Window>,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) config: wgpu::SurfaceConfiguration,
    pub(crate) pipeline: wgpu::RenderPipeline,
    /// The cutout blocks' quads (ADR 0069): the terrain's, through `fs_cut`, which discards an empty texel, so
    /// the opaque terrain keeps early depth.
    /// The cutout blocks in two passes around the opaque terrain (ADR 0090): their depth alone first (`fs_cut_depth`
    /// discards an empty texel and writes nothing else), so the terrain behind a crown is never shaded; then their
    /// G-buffer where the depth is theirs (`Equal`), with no discard, so each pixel is shaded once with early depth.
    pub(crate) cut_depth_pipeline: wgpu::RenderPipeline,
    pub(crate) cut_lit_pipeline: wgpu::RenderPipeline,
    /// The crowns' fluff (ADR 0090, `vs_fluff`): the cutout quads again as larger planes stood out of the crown,
    /// in the same two passes.
    pub(crate) fluff_depth_pipeline: wgpu::RenderPipeline,
    pub(crate) fluff_lit_pipeline: wgpu::RenderPipeline,
    pub(crate) sky_pipeline: wgpu::RenderPipeline,
    /// The overlays (ADR 0017): ghost lines and the 2D layer, with their vertex and rectangle buffers.
    pub(crate) ghost_pipeline: wgpu::RenderPipeline,
    pub(crate) ghost_buf: wgpu::Buffer,
    pub(crate) ui_pipeline: wgpu::RenderPipeline,
    pub(crate) ui_buf: wgpu::Buffer,
    /// The globe map's view, pictures and seen-masks (ADR 0077, `globe::off`).
    pub(crate) globe_buf: wgpu::Buffer,
    /// The water (ADR 0020): the sea's quads in the chunks, the water mesh from its own buffer.
    pub(crate) sea_pipeline: wgpu::RenderPipeline,
    pub(crate) water_pipeline: wgpu::RenderPipeline,
    /// Glass (ADR 0081): the terrain's quads through `fs_glass`, blended over the lit frame as the water.
    pub(crate) glass_pipeline: wgpu::RenderPipeline,
    pub(crate) wquads: wgpu::Buffer,
    /// The effects (ADR 0031): the particles, drawn into the G-buffer lit per pixel.
    pub(crate) fx_buf: wgpu::Buffer,
    pub(crate) fx_pipeline: wgpu::RenderPipeline,
    pub(crate) shade_pipeline: wgpu::RenderPipeline,
    pub mquads: wgpu::Buffer,
    pub mpieces: wgpu::Buffer,
    /// The clutter's still models (ADR 0082): their faces and paintings are bindings 16 and 17, the streamer
    /// lays their quads in level 0's meshes; drawn as the cutout blocks are (ADR 0103): their depth alone before
    /// the terrain (`still_depth_pipeline`), then `still_pipeline` shades each pixel once where that depth is.
    pub still: std::sync::Arc<engine::clutter::Still>,
    pub(crate) still_depth_pipeline: wgpu::RenderPipeline,
    pub(crate) still_pipeline: wgpu::RenderPipeline,
    pub(crate) model_pipeline: wgpu::RenderPipeline,
    pub(crate) bind: wgpu::BindGroup,
    /// `bind`'s layout, for a new TLAS (`rebind`)
    bgl: wgpu::BindGroupLayout,
    pub(crate) globals: wgpu::Buffer,
    pub(crate) quads: wgpu::Buffer,
    pub(crate) chunks: wgpu::Buffer,
    pub(crate) lamps: wgpu::Buffer,
    pub(crate) lamp_cells: wgpu::Buffer,
    /// The still faces and paintings: the bounce pass's rays read them too (ADR 0091).
    pub(crate) sfaces: wgpu::Buffer,
    pub(crate) spaint: wgpu::Buffer,
    pub(crate) indirect: wgpu::Buffer,
    /// Six indices a quad over its four corners, for QUAD_INDEX quads (ADR 0103).
    pub(crate) quad_index: wgpu::Buffer,
    pub(crate) depth: wgpu::TextureView,
    /// The terrain's shader and the block looks: the bounce light is built again from them on a resize.
    pub(crate) shader: wgpu::ShaderModule,
    pub(crate) looks: wgpu::Buffer,
    /// The painted block faces (ADR 0045) and the watcher's newest set of them.
    pub(crate) tex: wgpu::Buffer,
    pub(crate) tex_rx: std::sync::mpsc::Receiver<engine::textures::Set>,
    pub(crate) ts: Option<(wgpu::QuerySet, wgpu::Buffer, Vec<(wgpu::Buffer, Arc<AtomicBool>, bool)>)>,
    pub(crate) ts_period: f32,
    pub(crate) capture: Option<wgpu::Buffer>,
    pub(crate) adapter_name: String,
    pub(crate) backend: String,
}

/// The painted faces' 16 x 16 layers (2 MB); a block past them keeps its pattern.
const TEX_LAYERS: u32 = 2048;

/// What the hotbar and the "looking at" cube show of a painted block: its top's mean and its side's, per look
/// row (`looks_of`: IDS a season).
/// The terrain's bind group: `b` the buffers globals, quads, chunks, looks, water quads, lamps, lamp cells, effects,
/// paintings, model quads, model pieces, globe, still faces, still paintings
#[allow(clippy::too_many_arguments)]
fn main_bind(device: &wgpu::Device, bgl: &wgpu::BindGroupLayout, b: [&wgpu::Buffer; 14], tlas: &wgpu::Tlas, sky: (&wgpu::TextureView, &wgpu::Sampler), warp: &wgpu::TextureView, sway: (&wgpu::TextureView, &wgpu::Sampler)) -> wgpu::BindGroup {
    let [globals, quads, chunks, looks, wquads, lamps, lamp_cells, fx_buf, tex, mquads, mpieces, globe_buf, sfaces, spaint] = b;
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: bgl,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 1, resource: quads.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 2, resource: chunks.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 3, resource: looks.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 4, resource: wquads.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 5, resource: tlas.as_binding() },
            wgpu::BindGroupEntry { binding: 8, resource: lamps.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 9, resource: lamp_cells.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 10, resource: fx_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 11, resource: tex.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 12, resource: mquads.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 13, resource: mpieces.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 20, resource: globe_buf.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 14, resource: wgpu::BindingResource::TextureView(sky.0) },
            wgpu::BindGroupEntry { binding: 15, resource: wgpu::BindingResource::Sampler(sky.1) },
            wgpu::BindGroupEntry { binding: 16, resource: sfaces.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 17, resource: spaint.as_entire_binding() },
            wgpu::BindGroupEntry { binding: 18, resource: wgpu::BindingResource::TextureView(warp) },
            wgpu::BindGroupEntry { binding: 19, resource: wgpu::BindingResource::TextureView(sway.0) },
            wgpu::BindGroupEntry { binding: 21, resource: wgpu::BindingResource::Sampler(sway.1) },
        ],
    })
}

impl Gpu {
    /// The bounce's share of the terrain's resources (`Gi::new`), with `tlas`.
    pub(crate) fn shared<'a>(&'a self, tlas: &'a wgpu::Tlas) -> [wgpu::BindingResource<'a>; 13] {
        [
            self.globals.as_entire_binding(),
            self.quads.as_entire_binding(),
            self.chunks.as_entire_binding(),
            self.looks.as_entire_binding(),
            tlas.as_binding(),
            self.lamps.as_entire_binding(),
            self.lamp_cells.as_entire_binding(),
            self.tex.as_entire_binding(),
            wgpu::BindingResource::TextureView(&self.sky_view),
            wgpu::BindingResource::Sampler(&self.sky_samp),
            self.sfaces.as_entire_binding(),
            self.spaint.as_entire_binding(),
            wgpu::BindingResource::TextureView(&self.warp_view),
        ]
    }

    /// The terrain's bind group again with a new TLAS (rt.rs `grew`, ADR 0111).
    pub(crate) fn rebind(&mut self, tlas: &wgpu::Tlas) {
        let b = [&self.globals, &self.quads, &self.chunks, &self.looks, &self.wquads, &self.lamps, &self.lamp_cells, &self.fx_buf, &self.tex, &self.mquads, &self.mpieces, &self.globe_buf, &self.sfaces, &self.spaint];
        self.bind = main_bind(&self.device, &self.bgl, b, tlas, (&self.sky_view, &self.sky_samp), &self.warp_view, (&self.sway_view, &self.sway_samp));
    }
}

/// A painted block's faces' means by look row (`looks_of`'s rows: season * IDS + id), empty until the textures load.
pub(crate) static SHOWN: std::sync::RwLock<Vec<Option<[[f32; 3]; 2]>>> = std::sync::RwLock::new(Vec::new());
const IDS: usize = engine::blocks::IDS;

/// A painted block's top and side (their first variants) as `hud::cube` draws them, by block id.
/// Counts the textures' reloads: what a kept look (`hud::Looks`) was made from.
pub(crate) static PAINT_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static PAINTING: std::sync::RwLock<Vec<(u16, [Option<engine::textures::Pixels>; 2])>> = std::sync::RwLock::new(Vec::new());

/// The painting of block `d`'s top and side for `hud::cube` (bug 2026-11-13_14-31-01: in hand as placed).
/// shortcut: summer's paint only, a season's own paint shows in the world but not on the cubes; upgrade when
/// seasons get their own paintings.
pub(crate) fn painting(d: &engine::blocks::Def) -> [Option<engine::textures::Pixels>; 2] {
    PAINTING.read().unwrap().iter().find(|(id, _)| *id == d.id).map_or([None; 2], |p| p.1)
}

/// A fine face's 32 x 32 variant from its quarters (top left, top right, bottom left, bottom right) at 16 x 16:
/// each pixel the mean of the 2 x 2 there, there when at least two of them are (a cutout's holes stay).
pub(crate) fn halve(q: &[engine::textures::Pixels]) -> engine::textures::Pixels {
    let mut out = [[0u8; 4]; 256];
    for (k, o) in out.iter_mut().enumerate() {
        let (x, y) = (k % 16, k / 16);
        let quarter = &q[(y / 8) * 2 + x / 8];
        let (mut c, mut n) = ([0u32; 3], 0u32);
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let p = quarter[((y % 8) * 2 + dy) * 16 + (x % 8) * 2 + dx];
            if p[3] > 0 {
                (0..3).for_each(|m| c[m] += p[m] as u32);
                n += 1;
            }
        }
        if n >= 2 {
            *o = [(c[0] / n) as u8, (c[1] / n) as u8, (c[2] / n) as u8, 255];
        }
    }
    out
}

#[test]
fn a_fine_face_halved_keeps_its_quarters_in_place_and_its_holes() {
    let mut q = [[[10, 20, 30, 255]; 256]; 4];
    q[1] = [[200, 100, 50, 255]; 256];
    q[2][0] = [0; 4];
    q[2][1] = [0; 4];
    q[2][16] = [0; 4];
    let h = halve(&q);
    assert_eq!(h[0], [10, 20, 30, 255]);
    assert_eq!(h[15], [200, 100, 50, 255], "the top right quarter at the top right");
    assert_eq!(h[8 * 16], [0; 4], "three of four empty: a hole");
    // two of four there: a pixel, the mean of the two
    let mut q = [[[0u8; 4]; 256]; 4];
    q[3][0] = [100, 0, 0, 255];
    q[3][17] = [200, 50, 0, 255];
    assert_eq!(halve(&q)[8 * 16 + 8], [150, 25, 0, 255], "two of four: there");
    q[3][17] = [0; 4];
    assert_eq!(halve(&q)[8 * 16 + 8], [0; 4], "one of four: a hole");
}

/// A block as the hotbar's cubes draw it in the season `sn` where the player stands (`season_here`): top
/// colour, side colour, pattern; a painted block in its faces' means with plain noise (ADR 0045), any other in
/// its colours (a grass block's sides are its `side` block's), each mixed by the season as shader.wgsl
/// `look_in` mixes its rows (no stagger).
pub(crate) fn shown(d: &engine::blocks::Def, sn: (f64, f64)) -> ([f32; 3], [f32; 3], u8) {
    let painted = SHOWN.read().unwrap();
    let id = d.id as usize;
    let row = |r: usize| match painted.get(r * IDS + id).copied().flatten() {
        Some(c) => c,
        None => [d.tone(r).colour, engine::blocks::BLOCKS.get(d.side).map_or(d.colour, |s| s.tone(r).colour)],
    };
    // a season's paint alone gives a block seasons (`looks_of`); the summer cube keeps an unpainted block's pattern
    let pattern = if painted.get(id).is_some_and(|p| p.is_some()) { 0 } else { d.pattern as u8 };
    let own = row(0);
    let seasons = d.seasons.is_some() || (1..4).any(|r| painted.get(r * IDS + id).is_some_and(|p| p.is_some()));
    let [top, side] = match (seasons, season_rows(sn)) {
        (true, Some((x, y, f, s))) => {
            let (x, y) = (row(x), row(y));
            std::array::from_fn(|k| std::array::from_fn(|c| own[k][c] + ((x[k][c] + (y[k][c] - x[k][c]) * f) - own[k][c]) * s))
        }
        _ => own,
    };
    (top, side, pattern)
}

/// The two nearest seasons' look rows (0 summer, 1 spring, 2 autumn, 3 winter), how far from the first to the
/// second and the share of a season, for the season (phase, strength) of `engine::sky::season_at`; None for
/// none (shader.wgsl `season_rows`, without a stagger).
fn season_rows((phase, strength): (f64, f64)) -> Option<(usize, usize, f32, f32)> {
    let q = phase.rem_euclid(4.0);
    let i = (q as usize).min(3);
    (strength > 0.0).then(|| (engine::blocks::ROW[i], engine::blocks::ROW[(i + 1) & 3], (q - i as f64) as f32, strength as f32))
}

/// The season where the player at `pos` stands (`engine::sky::season_at`): on the first body by its latitude
/// and the year, none elsewhere, as frame.rs and shader.wgsl `season` have it.
pub(crate) fn season_here(w: &engine::world::World, pos: glam::DVec3) -> (f64, f64) {
    if w.frame_at(pos) != 0 {
        return (0.0, 0.0);
    }
    engine::sky::season_at(w.up_at(pos), w.time)
}

/// Where block `id`'s fluff and light go in the looks buffer, in bytes, one per season (a look row is 20 words; they
/// are its 17th and 18th), and the words.
fn fluff_words(id: u16, f: engine::blocks::Fluff) -> [(u64, [u32; 2]); 4] {
    std::array::from_fn(|season| (((season as u64 * IDS as u64 + id as u64) * 20 + 16) * 4, [f.packed(), f.light.to_bits()]))
}

/// Every block's look from the registry (ADR 0013, 0039, 0045), shader.wgsl `Look`, 80 bytes each: colour rgb,
/// pattern | metal << 16 | gives light << 24 | cutout << 25 | glass << 26 | joins << 27; colour2, size; colour3, amount; contrast; the painted
/// faces (16 bits a face: layer | variants - 1 << 12 | fine << 14 (32 x 32, four layers a variant, ADR 0090) | painted << 15; top and side in `tex`, the bottom in
/// `tex2`); the one colour of a painted block (`mean`, unorm 8-bit, painted << 24); a cutout block's fluff (`Fluff::packed`, ADR 0090) and its share of light through (f32); the block its
/// sides take (`side`), a word free. A row per id and season: 4 * IDS.
fn looks_of(painted: &[((u16, usize), engine::textures::Painted, [u32; 3])]) -> Vec<[u32; 20]> {
    looks_in(&engine::blocks::BLOCKS, painted)
}
fn looks_in(reg: &engine::blocks::Registry, painted: &[((u16, usize), engine::textures::Painted, [u32; 3])]) -> Vec<[u32; 20]> {
    (0..4 * IDS)
            .map(|row| {
                // four rows a block (ADR 0061): summer (the row's own look), spring, autumn, winter
                let (id, season) = ((row % IDS) as u16, row / IDS);
                let f = f32::to_bits;
                match reg.get(id) {
                    Some(d) => {
                        let tone = d.tone(season);
                        let ([r, g, b], [r2, g2, b2], [r3, g3, b3]) = (tone.colour.map(f), tone.colour2.map(f), tone.colour3.map(f));
                        // size, and for a block with seasons bit 8 (a season's paint alone gives it them), bit 9
                        // when a season paints its own, and its stagger from bit 16
                        let paints = painted.iter().any(|((b, q), _, _)| *b == id && *q != 0);
                        let size = d.size as u32 | ((d.seasons.is_some() || paints) as u32) << 8 | (paints as u32) << 9 | engine::sky::stagger_units(d.stagger) << 16;
                        let shine = (d.metal * 255.0).round() as u32;
                        let gives = (d.light != [0.0; 3]) as u32;
                        let info = d.pattern as u32 | shine << 16 | gives << 24 | ((d.see == engine::blocks::See::Cutout) as u32) << 25 | ((d.see == engine::blocks::See::Glass) as u32) << 26 | (d.joins as u32) << 27;
                        let (mut tex, mut mean) = (0u64, 0);
                        // a season's own paint, else the base's (a season row without one is its pattern only when
                        // the base has none)
                        let own = painted.iter().find(|(k, _, _)| *k == (id, season));
                        if let Some((_, p, at)) = own.or_else(|| painted.iter().find(|(k, _, _)| *k == (id, 0))) {
                            for (k, face) in p.faces.iter().enumerate() {
                                if let Some(face) = face {
                                    tex |= ((at[k] | (face.variants.len() as u32 / (if face.fine { 4 } else { 1 }) - 1) << 12 | (face.fine as u32) << 14 | 1 << 15) as u64) << (16 * k);
                                }
                            }
                            let [mr, mg, mb] = p.colour().unwrap_or(d.colour).map(|c| (c * 255.0).round() as u32);
                            mean = mr | mg << 8 | mb << 16 | 1 << 24;
                        }
                        // a cutout block's fluff (ADR 0090, `Fluff::packed`), the same in every season
                        // and the share of the sun a face of it lets through to a shadow ray
                        let (fluff, through) = d.fluff.map_or((0, 0.0), |f| (f.packed(), f.light));
                        [r, g, b, info, r2, g2, b2, size, r3, g3, b3, f(tone.amount), f(tone.contrast), tex as u32, mean, (tex >> 32) as u32, fluff, f(through), d.side as u32, 0]
                    }
                    None => {
                        let h = f(0.5);
                        [h, h, h, 0, h, h, h, 0, h, h, h, 0, f(1.0), 0, 0, 0, 0, 0, id as u32, 0]
                    }
                }
            })
        .collect()
}

/// Where the painted faces go among `cap` layers: per block and season (its look row's, `looks_of`; the base
/// first) its first layer for top, side and bottom, the layers in order, and the ones that did not fit (they
/// keep their patterns, a season the base's paint).
type Placed = ((u16, usize), engine::textures::Painted, [u32; 3]);
fn place(rows: Vec<((u16, usize), engine::textures::Painted)>, cap: u32) -> (Vec<Placed>, Vec<engine::textures::Pixels>, Vec<(u16, usize)>) {
    let (mut placed, mut layers, mut refused): (Vec<Placed>, Vec<_>, Vec<_>) = (vec![], vec![], vec![]);
    for (key, p) in rows {
        // a face the same as an earlier one of its row (a `<block>.png` they fall back to), or as one of its
        // block's placed rows (a season's face the base's), takes its layers
        let first = |k: usize| (0..k).find(|&e| p.faces[e].is_some() && p.faces[e] == p.faces[k]).map(|e| (None, e));
        let placed_as = |k: usize| placed.iter().filter(|(o, _, _): &&Placed| o.0 == key.0).find_map(|(_, q, at)| (0..3).find(|&e| q.faces[e].is_some() && q.faces[e] == p.faces[k]).map(|e| at[e]));
        let shared = |k: usize| first(k).or_else(|| placed_as(k).map(|l| (Some(l), k)));
        let n: u32 = (0..3).filter(|&k| shared(k).is_none()).filter_map(|k| p.faces[k].as_ref()).map(|f| f.variants.len() as u32).sum();
        if layers.len() as u32 + n > cap {
            refused.push(key);
            continue;
        }
        let mut at = [0; 3];
        for k in 0..3 {
            match (shared(k), &p.faces[k]) {
                (Some((Some(l), _)), _) => at[k] = l,
                (Some((None, e)), _) => at[k] = at[e],
                (None, Some(f)) => {
                    at[k] = layers.len() as u32;
                    layers.extend_from_slice(&f.variants);
                }
                (None, None) => {}
            }
        }
        placed.push((key, p, at));
    }
    (placed, layers, refused)
}

/// A texture set's rows for `place`: every block's base paint (look row 0), then its seasons'.
fn rows_of(set: engine::textures::Set) -> Vec<((u16, usize), engine::textures::Painted)> {
    set.blocks.into_iter().map(|(id, p)| ((id, 0), p)).chain(set.seasons.into_iter().map(|(id, q, p)| ((id, q), p))).collect()
}

/// The watcher (ADR 0045, 0047): a thread named `name` that calls `load` on `dir` at the start and again within
/// a second of a file there (or one of `also`) changing, appearing or going, and sends what it made, so reading
/// and decoding never run on the frame thread. `load` says whether it found anything: the first look at an empty
/// folder sends nothing.
pub(crate) fn watch<T: Send + 'static>(name: &str, dir: std::path::PathBuf, also: Vec<std::path::PathBuf>, load: impl Fn(&std::path::Path) -> (T, bool) + Send + 'static) -> std::sync::mpsc::Receiver<T> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name(name.into())
        .spawn(move || {
            let mut seen = None;
            loop {
                // one folder down too: textures' hd/ (ADR 0090), whose files change without the folder's time
                let mut now: Vec<_> = std::fs::read_dir(&dir)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .flat_map(|e| {
                        let sub = e.file_type().is_ok_and(|t| t.is_dir());
                        std::iter::once(e.path()).chain(sub.then(|| std::fs::read_dir(e.path()).into_iter().flatten().flatten().map(|f| f.path())).into_iter().flatten()).collect::<Vec<_>>()
                    })
                    .filter_map(|p| { let m = p.metadata().ok()?; Some((p.into_os_string(), m.modified().ok()?, m.len())) })
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .collect();
                now.extend(also.iter().filter_map(|f| Some((f.as_os_str().to_owned(), f.metadata().ok()?.modified().ok()?, f.metadata().ok()?.len()))));
                // and the block and item rows (ADR 0134): what is made here is made over them (a block's looks,
                // a row's model), so it is made again when they are reloaded
                now.push((std::ffi::OsString::from("live rows"), std::time::UNIX_EPOCH, engine::live::rev()));
                if seen.as_ref() != Some(&now) {
                    let (set, any) = load(&dir);
                    if (seen.is_some() || any) && tx.send(set).is_err() {
                        return;
                    }
                    seen = Some(now);
                }
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        })
        .expect("a watcher thread");
    rx
}

fn watch_textures(dir: std::path::PathBuf) -> std::sync::mpsc::Receiver<engine::textures::Set> {
    // each file's last good paint, kept on while a save of it is bad
    let kept = std::cell::RefCell::new(engine::textures::Kept::new());
    watch("textures", dir, Vec::new(), move |d| {
        let set = engine::textures::load_keeping(d, &engine::blocks::BLOCKS, &mut kept.borrow_mut());
        for why in &set.refused {
            eprintln!("[technoape] textures: {why}");
        }
        let any = !set.blocks.is_empty() || !set.seasons.is_empty();
        (set, any)
    })
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
        // the models' two buffers (ADR 0047) take the main bind group past the default 8 storage buffers a stage
        limits.max_storage_buffers_per_shader_stage = al.max_storage_buffers_per_shader_stage;
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
        // the draws, in the regions IND_SEA .. IND_FLUFF
        let indirect = buf("indirect", IND_FLUFF + MAX_CHUNKS as u64 * 20, U::INDIRECT | U::COPY_DST);
        // the terrain's quads are drawn indexed (ADR 0103): quad q's corners are vertices 4q .. 4q + 3 (shader.wgsl
        // `corner_k`), its two triangles QUAD_TRIS, so each corner is shaded once, not once a triangle
        let quad_index = wgpu::util::DeviceExt::create_buffer_init(&device, &wgpu::util::BufferInitDescriptor {
            label: Some("quad index"),
            contents: bytemuck::cast_slice(&(0..QUAD_INDEX).flat_map(|q| QUAD_TRIS.map(|k| q * 4 + k)).collect::<Vec<u32>>()),
            usage: U::INDEX,
        });
        let wquads = buf("wquads", (MAX_WQUADS * std::mem::size_of::<water::WQuad>()) as u64, U::STORAGE | U::COPY_DST);
        let fx_buf = buf("fx", (MAX_FX * std::mem::size_of::<crate::fx::FxGpu>()) as u64, U::STORAGE | U::COPY_DST);
        // every block's look from the registry (ADR 0013, 0039), shader.wgsl `Look`, 80 bytes each: colour rgb,
        // pattern | metal << 16 | gives light << 24 ...; colour2, size; colour3, amount; contrast (`looks_of`)
        let looks = buf("looks", (4 * IDS * 80) as u64, U::STORAGE | U::COPY_DST);
        queue.write_buffer(&looks, 0, bytemuck::cast_slice(&looks_of(&[])));
        // the painted faces (ADR 0045): 16 x 16 layers, filled by `take_textures` from the watcher's thread
        let tex = buf("block textures", TEX_LAYERS as u64 * 1024, U::STORAGE | U::COPY_DST);
        // the models (ADR 0047): their quads, and one transform per part drawn
        let mquads = buf("model quads", (crate::models::MAX_MQUADS * 16) as u64, U::STORAGE | U::COPY_DST);
        // the globe map (ADR 0077): filled by `App::globe_frame` once the worker's pictures are made
        let globe_buf = buf("globe", (crate::globe::LEN * 4) as u64, U::STORAGE | U::COPY_DST);
        let mpieces = buf("model pieces", (crate::models::MAX_PIECES * std::mem::size_of::<crate::models::Piece>()) as u64, U::STORAGE | U::COPY_DST);
        // the clutter's models, read once at the start (shortcut: not live as the models' are; a restart reads them)
        let palette = std::fs::read_to_string("data/palette.toml").ok().and_then(|t| engine::palette::Palette::parse(&t).ok()).unwrap_or_else(|| crate::fx::PALETTE.clone());
        let (still, bad) = engine::clutter::Still::load(std::path::Path::new(&a.get("models", String::from("data/models"))), std::path::Path::new(&a.get("plants", String::from("data/plants"))), &palette);
        for b in bad {
            eprintln!("clutter: {b}");
        }
        let sfaces = buf("still faces", (still.faces.len().max(1) * 64) as u64, U::STORAGE | U::COPY_DST);
        queue.write_buffer(&sfaces, 0, bytemuck::cast_slice(&still.faces));
        let spaint = buf("still paintings", (still.paint.len().max(1) * 4) as u64, U::STORAGE | U::COPY_DST);
        queue.write_buffer(&spaint, 0, bytemuck::cast_slice(&still.paint));
        let still = std::sync::Arc::new(still);
        // the clouds' map (ADR 0076, clouds.wgsl): a texel per air column, CL_TEX a side, twelve layers
        let sky_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("sky"),
            size: wgpu::Extent3d { width: SKY_TEX, height: SKY_TEX, depth_or_array_layers: 12 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let sky_view = sky_tex.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2Array), ..Default::default() });
        let sky_samp = device.create_sampler(&wgpu::SamplerDescriptor { mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, ..Default::default() });
        let warp_view = warp_view(&device, &queue);
        // the plants' wind map (ADR 0157): the watcher's first look waited for here, at loading, so
        // the first frame already sways by it and pictures repeat (ADR 0031); it always sends one
        let sway_rx = watch_sway(std::path::PathBuf::from(a.get("effects", String::from("data/effects.toml"))));
        let sway_wind = sway_rx.recv().ok().flatten();
        let sway_view = sway_view(&device, &queue, sway_wind.as_ref());
        let repeat = wgpu::AddressMode::Repeat;
        let sway_samp = device.create_sampler(&wgpu::SamplerDescriptor { address_mode_u: repeat, address_mode_v: repeat, address_mode_w: repeat, mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, ..Default::default() });
        let tex_rx = watch_textures(std::path::PathBuf::from(a.get("textures", String::from("data/textures"))));
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(concat!(
                include_str!("shader_vertex.wgsl"),
                include_str!("shader_light.wgsl"),
                include_str!("shader.wgsl"),
                include_str!("shader_water.wgsl"),
                include_str!("shader_sky_fx.wgsl"),
                include_str!("shader_models.wgsl"),
                include_str!("clouds.wgsl")
            ).into()),
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
                storage(11),
                storage(12),
                storage(13),
                storage(20),
                wgpu::BindGroupLayoutEntry {
                    binding: 14,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2Array, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { binding: 15, visibility: wgpu::ShaderStages::VERTEX_FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
                storage(16),
                storage(17),
                wgpu::BindGroupLayoutEntry {
                    binding: 18,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: false }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 19,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D3, multisampled: false },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry { binding: 21, visibility: wgpu::ShaderStages::VERTEX, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
            ],
        });
        let rt = rt::Rt::new(&device, &shader, &globals, &quads, &sfaces, &warp_view, MAX_CHUNKS as u32);
        let bind = main_bind(&device, &bgl, [&globals, &quads, &chunks, &looks, &wquads, &lamps, &lamp_cells, &fx_buf, &tex, &mquads, &mpieces, &globe_buf, &sfaces, &spaint], &rt.tlas, (&sky_view, &sky_samp), &warp_view, (&sway_view, &sway_samp));
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
        let gbuffer_pass = |vs: &'static str, fs: &'static str, cull, colour: bool, compare| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(fs),
                layout: Some(&layout),
                vertex: wgpu::VertexState { module: &shader, entry_point: Some(vs), buffers: &[], compilation_options: Default::default() },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    targets: &gi::TARGETS.map(|f| Some(wgpu::ColorTargetState { format: f, blend: None, write_mask: if colour { wgpu::ColorWrites::ALL } else { wgpu::ColorWrites::empty() } })),
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState { cull_mode: cull, ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(compare == wgpu::CompareFunction::Greater),
                    depth_compare: Some(compare),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let gbuffer_pipeline = |vs, fs, cull| gbuffer_pass(vs, fs, cull, true, wgpu::CompareFunction::Greater);
        let fx_pipeline = gbuffer_pipeline("vs_fx", "fs_fx", Some(wgpu::Face::Back));
        // the contact shadows under lying things (ADR 0114): multiplied into the albedo's colour only, where the
        // floor is the nearest thing (depth tested, not written); the light, the normal and the depth untouched
        let multiply = wgpu::BlendComponent { src_factor: wgpu::BlendFactor::Dst, dst_factor: wgpu::BlendFactor::Zero, operation: wgpu::BlendOperation::Add };
        let shade_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("fs_shade"),
            layout: Some(&layout),
            vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_shade"), buffers: &[], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_shade"),
                targets: &[
                    Some(wgpu::ColorTargetState { format: gi::TARGETS[0], blend: None, write_mask: wgpu::ColorWrites::empty() }),
                    Some(wgpu::ColorTargetState { format: gi::TARGETS[1], blend: Some(wgpu::BlendState { color: multiply, alpha: wgpu::BlendComponent::REPLACE }), write_mask: wgpu::ColorWrites::COLOR }),
                    Some(wgpu::ColorTargetState { format: gi::TARGETS[2], blend: None, write_mask: wgpu::ColorWrites::empty() }),
                ],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
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
        });
        let cut_depth_pipeline = gbuffer_pass("vs_cut", "fs_cut_depth", Some(wgpu::Face::Back), false, wgpu::CompareFunction::Greater);
        let cut_lit_pipeline = gbuffer_pass("vs_cut", "fs_cut_lit", Some(wgpu::Face::Back), true, wgpu::CompareFunction::Equal);
        let fluff_depth_pipeline = gbuffer_pass("vs_fluff", "fs_fluff_depth", Some(wgpu::Face::Back), false, wgpu::CompareFunction::Greater);
        let fluff_lit_pipeline = gbuffer_pass("vs_fluff", "fs_fluff_lit", Some(wgpu::Face::Back), true, wgpu::CompareFunction::Equal);
        // still models: either way round (`fs_still` drops the face turned away); depth first, then lit where it is
        let still_depth_pipeline = gbuffer_pass("vs_still", "fs_still_depth", None, false, wgpu::CompareFunction::Greater);
        let still_pipeline = gbuffer_pass("vs_still", "fs_still", None, true, wgpu::CompareFunction::Equal);
        // the models (ADR 0047): into the G-buffer with the terrain
        let model_pipeline = gbuffer_pipeline("vs_model", "fs_model", Some(wgpu::Face::Back));
        // water: blended over the terrain, tested against its depth but not writing it, seen from both sides
        let liquid = |vs, fs, cull| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(vs),
                layout: Some(&layout),
                vertex: wgpu::VertexState { module: &shader, entry_point: Some(vs), buffers: &[], compilation_options: Default::default() },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    targets: &[Some(wgpu::ColorTargetState { format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState { cull_mode: cull, ..Default::default() },
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
        let sea_pipeline = liquid("vs_sea", "fs_water", None);
        let water_pipeline = liquid("vs_water", "fs_water", None);
        // a glass wall shows its near face only: one pane's tint, not two
        let glass_pipeline = liquid("vs", "fs_glass", Some(wgpu::Face::Back));
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
        let capture = (a.get("capture_ms", 0) > 0 || a.0.contains_key("shot") || ["play", "view"].contains(&a.s("run", "stand").as_str())).then(|| capture_buf(&device));
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
                tex.as_entire_binding(),
                wgpu::BindingResource::TextureView(&sky_view),
                wgpu::BindingResource::Sampler(&sky_samp),
                sfaces.as_entire_binding(),
                spaint.as_entire_binding(),
                wgpu::BindingResource::TextureView(&warp_view),
            ],
            (&sky_view, &sky_samp),
            format,
            w(),
            h(),
        );
        let gpu = Gpu {
            warp_view,
            sky_tex,
            sky_view,
            sky_samp,
            window,
            sky_pipeline,
            ts_period: queue.get_timestamp_period(),
            device,
            queue,
            surface,
            config,
            pipeline,
            cut_depth_pipeline,
            cut_lit_pipeline,
            fluff_depth_pipeline,
            fluff_lit_pipeline,
            sea_pipeline,
            water_pipeline,
            glass_pipeline,
            wquads,
            fx_buf,
            fx_pipeline,
            shade_pipeline,
            mquads,
            mpieces,
            bgl,
            still,
            still_depth_pipeline,
            still_pipeline,
            model_pipeline,
            ghost_pipeline,
            ghost_buf,
            ui_pipeline,
            ui_buf,
            globe_buf,
            bind,
            globals,
            quads,
            chunks,
            lamps,
            lamp_cells,
            sfaces,
            spaint,
            indirect,
            quad_index,
            depth,
            shader: shader.clone(),
            looks: looks.clone(),
            tex,
            tex_rx,
            sway_view,
            sway_samp,
            sway: sway_globals(sway_wind.as_ref()),
            sway_field: sway_wind.map(|w| w.field),
            sway_rx,
            ts,
            capture,
            adapter_name: info.name.clone(),
            backend: format!("{:?}", info.backend),
        };
        (gpu, rt, gi)
    }

    /// Swaps in the sway watcher's newest field, if it sent one: true when the wind map is new and the terrain's
    /// bind group must be made again (`rebind`). Under the frame's budget: one texture upload of the field's size
    /// (1 MB for `wind gusts`, its texels made on the watcher), only when its map changed.
    pub(crate) fn take_sway(&mut self) -> bool {
        let Some(got) = self.sway_rx.try_iter().last() else { return false };
        let new = match (&got, &self.sway_field) {
            (Some(w), Some(was)) => !Arc::ptr_eq(&w.field, was),
            (None, None) => false,
            _ => true,
        };
        self.sway = sway_globals(got.as_ref());
        if new {
            self.sway_view = sway_view(&self.device, &self.queue, got.as_ref());
            self.sway_field = got.map(|w| w.field);
        }
        new
    }

    /// Swaps in the watcher's newest textures, if it sent any: the layers, the looks, the hotbar's colours.
    pub(crate) fn take_textures(&self) {
        let Some(set) = self.tex_rx.try_iter().last() else { return };
        let (placed, layers, refused) = place(rows_of(set), TEX_LAYERS);
        for (id, season) in refused {
            let name = &engine::blocks::BLOCKS.get(id).unwrap().name;
            // a season's paint refused leaves that season the base's paint
            let season = engine::blocks::ROW.iter().position(|r| *r == season).filter(|_| season != 0).map_or(String::new(), |k| format!(" in {}", engine::blocks::SEASONS[k]));
            eprintln!("[technoape] textures: {name}{season} refused, more than {TEX_LAYERS} faces and variants in all");
        }
        if !layers.is_empty() {
            self.queue.write_buffer(&self.tex, 0, bytemuck::cast_slice(&layers));
        }
        let mut shown = vec![None; 4 * IDS];
        // per look row as `looks_of` takes them: a season's own paint, else the base's
        for r in 0..4 {
            for ((id, season), p, _) in placed.iter().filter(|((_, q), _, _)| *q == r || *q == 0) {
                let d = engine::blocks::BLOCKS.get(*id).unwrap();
                let at = r * IDS + *id as usize;
                if p.colour().is_some() && (*season == r || shown[at].is_none()) {
                    let side = engine::blocks::BLOCKS.get(d.side).map_or(d.colour, |s| s.tone(r).colour);
                    shown[at] = Some([p.faces[0].as_ref().map_or(d.tone(r).colour, |f| f.mean), p.faces[1].as_ref().map_or(side, |f| f.mean)]);
                }
            }
        }
        // (tools/reload_check.py reads this line)
        let blocks = placed.iter().filter(|((_, q), _, _)| *q == 0).count();
        eprintln!("[technoape] textures: {blocks} blocks painted, {} season looks, {} layers", placed.len() - blocks, layers.len());
        self.queue.write_buffer(&self.looks, 0, bytemuck::cast_slice(&looks_of(&placed)));
        *SHOWN.write().unwrap() = shown;
        // a fine face (ADR 0090): its first 32 x 32 variant (four quarters) halved to 16 x 16 (ADR 0113)
        let first = |f: &Option<engine::textures::Face>| f.as_ref().and_then(|f| match (f.fine, f.variants.len()) { (true, 4..) => Some(halve(&f.variants[..4])), (true, _) => None, _ => f.variants.first().copied() });
        *PAINTING.write().unwrap() = placed.iter().filter(|((_, q), _, _)| *q == 0).map(|((id, _), p, _)| (*id, [first(&p.faces[0]), first(&p.faces[1])])).collect();
        PAINT_GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// A cutout block's fluff drawn so while a Studio slider is dragged (the view's `fluff`): its four look rows'
    /// fluff and light words written over. Nothing keeps it: the next `looks_of` (a texture or row reload, as the
    /// let-go slider's save is) writes the row's own numbers back.
    pub(crate) fn set_fluff(&self, id: u16, f: engine::blocks::Fluff) {
        for (at, words) in fluff_words(id, f) {
            self.queue.write_buffer(&self.looks, at, bytemuck::cast_slice(&words));
        }
    }

    pub(crate) fn upload(&self, off: u32, q: &[[u32; 2]]) {
        self.queue.write_buffer(&self.quads, off as u64 * 8, bytemuck::cast_slice(q));
    }
}

#[cfg(test)]
mod watch_tests {
    /// A change to one of `also` (the effects or sounds file a model's states are checked against) reads the
    /// folder again, its files untouched.
    #[test]
    fn a_change_to_a_file_also_watched_reads_the_folder_again() {
        let d = std::env::temp_dir().join(format!("technoape-watch-also-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("models")).unwrap();
        std::fs::write(d.join("models/m.toml"), "x").unwrap();
        std::fs::write(d.join("effects.toml"), "before").unwrap();
        let fx = d.join("effects.toml");
        let rx = super::watch("watch test", d.join("models"), vec![fx.clone()], move |_| (std::fs::read_to_string(&fx).unwrap_or_default(), true));
        assert_eq!(rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap(), "before");
        std::fs::write(d.join("effects.toml"), "after, longer").unwrap();
        let again = rx.recv_timeout(std::time::Duration::from_secs(5));
        let _ = std::fs::remove_dir_all(&d);
        assert_eq!(again.as_deref(), Ok("after, longer"), "the folder was not read again");
    }
}

#[cfg(test)]
mod look_tests {
    use super::IDS;
    use engine::blocks::BLOCKS;

    /// Ids past 255 (12 bits): block 300's look rows are its own in every season, at season * IDS + 300, its side
    /// block (4095, near the top) in its own word and a joining block flagged; the shader reads them with the
    /// same layout the engine packs (`quad_id`, `id_bits`, `IDS` in shader_vertex.wgsl as in engine mesh.rs and
    /// blocks.rs; fx.rs `block_bits` as shader_sky_fx.wgsl `fx_id` reads it).
    #[test]
    fn a_block_past_255_has_its_own_look_rows_and_the_shader_reads_ids_as_the_engine_packs_them() {
        let reg = engine::blocks::Registry::parse("[[block]]
id = 0
name = \"air\"
solid = false
            [[block]]
id = 300
name = \"high\"
hardness = 1
joins = true
side = \"top\"
colour = [0.1, 0.2, 0.3]
[block.winter]
colour = [0.9, 0.8, 0.7]
            [[block]]
id = 4095
name = \"top\"
hardness = 1
colour = [0.5, 0.5, 0.5]
").unwrap();
        let t = super::looks_in(&reg, &[]);
        assert_eq!(t.len(), 4 * IDS);
        let high = reg.get(300).unwrap();
        for s in 0..4 {
            let row = t[s * IDS + 300];
            assert_eq!([row[0], row[1], row[2]], high.tone(s).colour.map(f32::to_bits), "season {s}");
            assert_eq!((row[18], row[3] >> 27 & 1, row[7] >> 8 & 1), (4095, 1, 1), "season {s}: its side, joins, seasons");
        }
        assert_eq!(t[3 * IDS + 300][0], 0.9f32.to_bits(), "winter's own colour");
        assert_eq!((t[4095][0], t[4095][3] >> 27 & 1, t[44][18]), (0.5f32.to_bits(), 0, 44), "the top id; a row with none is its own side");
        // the shader's helpers are the engine's, letter for letter
        let src = include_str!("shader_vertex.wgsl");
        assert!(src.contains(&format!("const IDS: u32 = {IDS}u;")), "shader_vertex.wgsl IDS");
        assert!(src.contains("fn quad_id(mat: u32) -> u32 { return (mat & 255u) | ((mat >> 8u) & 0xf00u); }"));
        assert!(src.contains("fn id_bits(id: u32) -> u32 { return (id & 255u) | ((id >> 8u) << 16u); }"));
        assert!(src.contains("var<storage, read> looks: array<Look, 16384>;") && 4 * IDS == 16384);
        assert!(include_str!("shader_sky_fx.wgsl").contains("fn fx_id(info: u32) -> u32 { return (info & 255u) | ((info >> 12u) & 0xf00u); }"));
        // and what those lines compute, on the CPU: every id through a quad's word and a particle's
        let wgsl_quad_id = |mat: u32| (mat & 255) | ((mat >> 8) & 0xf00);
        let wgsl_fx_id = |info: u32| (info & 255) | ((info >> 12) & 0xf00);
        for id in 0..IDS as u16 {
            let hi = engine::mesh::id_bits(id);
            assert_eq!((wgsl_quad_id(hi | 0xfff0_ff00), (id as u32 & 255) | (id as u32 >> 8) << 16), (id as u32, hi), "{id}");
            assert_eq!(wgsl_fx_id(crate::fx::block_bits(id) | !crate::fx::BLOCK_BITS), id as u32, "{id}: a particle's");
        }
    }

    /// The Studio's fluff sliders (the view's `fluff`): the words written over the looks buffer land on the block's
    /// fluff and light in every season and nowhere else; with the row's own numbers they change nothing.
    #[test]
    fn a_dragged_fluff_lands_on_the_blocks_look_in_every_season() {
        let reed = BLOCKS.id("reed");
        let own = BLOCKS.get(reed).unwrap().fluff.unwrap();
        let t = super::looks_of(&[]);
        let write = |f| {
            let mut words: Vec<u32> = t.iter().flatten().copied().collect();
            for (at, w) in super::fluff_words(reed, f) {
                words[at as usize / 4..at as usize / 4 + 2].copy_from_slice(&w);
            }
            words
        };
        let flat: Vec<u32> = t.iter().flatten().copied().collect();
        assert!(write(own) == flat, "the row's own fluff is what looks_of wrote");
        let f = engine::blocks::Fluff { size: 2.5, push: 0.5, droop: 0.25, long: 2.0, dense: 0.5, light: 0.25 };
        let w = write(f);
        let changed: Vec<usize> = (0..w.len()).filter(|i| w[*i] != flat[*i]).collect();
        let expect: Vec<usize> = (0..4).flat_map(|s| [(s * IDS + reed as usize) * 20 + 16, (s * IDS + reed as usize) * 20 + 17]).collect();
        assert_eq!(changed, expect, "reed's fluff and light words in the four seasons");
    }

    /// ADR 0061: four rows a block, the row's own (summer) first, then spring, autumn, winter; a block without
    /// seasons is the same row four times with nothing over its size's 8 bits, so the shader leaves it alone.
    #[test]
    fn the_look_table_has_a_row_a_season() {
        let t = super::looks_of(&[]);
        assert_eq!(t.len(), 4 * IDS);
        for d in BLOCKS.placeable() {
            let id = d.id as usize;
            for s in 0..4 {
                let (row, tone) = (t[s * IDS + id], d.tone(s));
                assert_eq!([row[0], row[1], row[2]], tone.colour.map(f32::to_bits), "{} season {s}", d.name);
                assert_eq!((row[7] & 255, row[7] >> 8 & 1), (d.size as u32, d.seasons.is_some() as u32), "{}", d.name);
                if d.seasons.is_none() {
                    assert_eq!((row, row[7] >> 8), (t[id], 0), "{} has one look", d.name);
                }
            }
        }
        let leaves = BLOCKS.get(BLOCKS.id("broadleaf leaves")).unwrap();
        assert_eq!(t[leaves.id as usize][7] >> 16, engine::sky::stagger_units(3.0), "3 days of 13.5 a season");
        assert_ne!(t[2 * IDS + leaves.id as usize][..3], t[leaves.id as usize][..3], "autumn is not summer");
    }
}
