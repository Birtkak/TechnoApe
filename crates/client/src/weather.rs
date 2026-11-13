//! The planet's air as this client draws it (ADR 0076): the server's columns (`ToClient::Sky`), the step
//! before them, and the cloud map on the GPU blended from one to the other over the grid's step, so clouds
//! move rather than jump every `weather::DT` seconds. Everything here follows from the server's columns and
//! the world's clock, so two clients see the same sky.
use engine::weather::{Cell, DT};
use glam::DVec3;

/// Seconds the clouds' cells take to slide through one drift phase (clouds.wgsl `cl_sample`): long enough
/// that one phase holds for minutes, short enough that a wind change does not shear the pattern far.
pub const DRIFT: f64 = 240.0;
/// The blend between the two steps is uploaded in this many steps a grid step (about 6 a second).
const BLEND_STEPS: f64 = 64.0;

#[derive(Default)]
pub struct Sky {
    pub cur: Vec<Cell>,
    prev: Vec<Cell>,
    /// the world time `cur` landed
    t: f64,
    /// the blend step last uploaded (None: upload again)
    shown: Option<u32>,
}

impl Sky {
    /// The server's message: every column, or the ones that changed.
    pub fn take(&mut self, t: f64, all: Vec<Cell>, changed: Vec<(u32, Cell)>) {
        if !all.is_empty() {
            self.prev = all.clone();
            self.cur = all;
        } else {
            self.prev.clone_from(&self.cur);
        }
        for (k, c) in changed {
            if let Some(x) = self.cur.get_mut(k as usize) {
                *x = c;
            }
        }
        self.t = t;
        self.shown = None;
    }

    /// The map's columns a face edge (0: none yet).
    pub fn m(&self) -> usize {
        ((self.cur.len() / 6) as f64).sqrt().round() as usize
    }

    /// The share of the way from the last step to this one at world time `now`.
    fn blend(&self, now: f64) -> f64 {
        ((now - self.t) / DT as f64).clamp(0.0, 1.0)
    }

    /// The map as the GPU holds it at world time `now` (clouds.wgsl's layout): faces 0 to 5 (cover, rain,
    /// wind east and north biased by a half) blended from the last step, then (snow, temperature, 0, 0).
    pub fn texels(&self, now: f64) -> Vec<u8> {
        let b = (self.blend(now) * BLEND_STEPS).round() / BLEND_STEPS;
        let lerp = |a: u8, c: u8| (a as f64 + (c as f64 - a as f64) * b).round() as u8;
        let mut out = Vec::with_capacity(self.cur.len() * 8);
        for (p, c) in self.prev.iter().zip(&self.cur) {
            out.extend_from_slice(&[lerp(p[0], c[0]), lerp(p[1], c[1]), lerp(p[2] ^ 0x80, c[2] ^ 0x80), lerp(p[3] ^ 0x80, c[3] ^ 0x80)]);
        }
        for c in &self.cur {
            out.extend_from_slice(&[c[4], c[5], 0, 0]);
        }
        out
    }

    /// Writes the map when it changed or the blend moved a step; true when it wrote.
    pub fn upload(&mut self, queue: &wgpu::Queue, tex: &wgpu::Texture, now: f64) -> bool {
        let m = self.m();
        if m == 0 || m > crate::gpu::SKY_TEX as usize || 6 * m * m != self.cur.len() {
            return false;
        }
        let step = (self.blend(now) * BLEND_STEPS).round() as u32;
        if self.shown == Some(step) {
            return false;
        }
        self.shown = Some(step);
        queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &self.texels(now),
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(m as u32 * 4), rows_per_image: Some(m as u32) },
            wgpu::Extent3d { width: m as u32, height: m as u32, depth_or_array_layers: 12 },
        );
        true
    }

    /// `Globals::cloud` at world time `now` for a planet whose sea lies `sea` metres from its centre.
    pub fn cloud(&self, now: f64, sea: f64) -> [f32; 4] {
        let m = self.m();
        if m == 0 || m > crate::gpu::SKY_TEX as usize {
            return [0.0; 4];
        }
        [(now / DRIFT).rem_euclid(1.0) as f32, DRIFT as f32, sea as f32, m as f32]
    }
}

/// Rain heavier than this, mm an hour, is a storm: it lightens (ADR 0076).
pub const STORM: f32 = 8.0;
/// Seconds a slot of lightning lasts: each stormy column may strike once a slot.
const SLOT: f64 = 2.0;
/// The share of slots a stormy column strikes in at `STORM` (up to three times that in heavier rain): under a
/// storm from horizon to horizon a strike every few seconds within 2.5 km.
const STRIKE: f64 = 0.0006;
/// Metres a second sound goes: thunder comes this late from where the flash was.
const SOUND: f64 = 343.0;

/// The body-frame wind vector of an (east, north) wind at unit direction `up` (the planet spins about z, as
/// the air grid's `east_north`).
pub fn wind_at(up: DVec3, w: [f32; 2]) -> DVec3 {
    let east = DVec3::Z.cross(up).try_normalize().unwrap_or(DVec3::X);
    east * w[0] as f64 + up.cross(east) * w[1] as f64
}

/// The lightning of the stormy columns round unit direction `d` (`within` columns of its own each way) in
/// world time [from, to): from the columns' rain, the seed and the clock alone, so every client sees and hears
/// the same strikes. A strike is (world time, body-frame point at the clouds' base over its column).
pub fn strikes(sky: &Sky, seed: i32, d: DVec3, sea: f64, from: f64, to: f64, within: i32) -> Vec<(f64, DVec3)> {
    let m = sky.m();
    if m == 0 || to <= from {
        return Vec::new();
    }
    let g = engine::bake::grid::Grid::bare(m, 1.0);
    let (f, x, y) = g.locate_xy(d);
    let (s0, s1) = ((from / SLOT).floor() as i64, (to / SLOT).floor() as i64);
    let mut out = Vec::new();
    for j in (y as i32 - within).max(0)..(y as i32 + within + 1).min(m as i32) {
        for i in (x as i32 - within).max(0)..(x as i32 + within + 1).min(m as i32) {
            let c = (f * m + j as usize) * m + i as usize;
            let rain = engine::weather::decode(sky.cur[c]).1;
            if rain < STORM {
                continue;
            }
            let p = STRIKE * (rain / STORM).min(3.0) as f64;
            for s in s0..=s1 {
                let mut h = engine::world::fnv(engine::world::FNV0, &seed.to_le_bytes());
                h = engine::world::fnv(h, &(c as u64).to_le_bytes());
                h = engine::world::fnv(h, &s.to_le_bytes());
                let u = |k: u32| ((h >> (k * 16)) & 0xffff) as f64 / 65536.0;
                let t = (s as f64 + u(1)) * SLOT;
                if u(0) >= p || t < from || t >= to {
                    continue;
                }
                out.push((t, g.dir_at(f, i as f64 + u(2), j as f64 + u(3)) * (sea + 300.0)));
            }
        }
    }
    out
}

/// Rain and snow round the camera, lightning and thunder (ADR 0076): drops from the effects pool (ADR 0031)
/// that land on the floor of the column they are over, so a roof or a cave's ceiling stops them; the share of
/// the sky over the camera that is open, which the rain's sound follows; the flash now.
#[derive(Default)]
pub struct Fall {
    acc: f32,
    seed: u32,
    /// the share of the drops near the camera that fall from the open sky to below the head
    pub open: f32,
    /// thunder to play: (world time, metres away)
    thunder: Vec<(f64, f64)>,
    /// world time strikes were looked for up to
    struck: f64,
    /// the last flash's world time and distance
    flash: (f64, f64),
}

impl Fall {
    fn rnd(&mut self) -> f64 {
        self.seed = self.seed.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.seed >> 8) as f64 / 16777216.0
    }

    /// This frame's drops or flakes round world point `eye` on the planet (body 0) under the air `here`, at
    /// `level` of the Rain setting (0: none).
    pub fn drops(&mut self, fx: &mut crate::fx::Fx, world: &engine::world::World, eye: DVec3, here: &engine::weather::Here, level: usize, dt: f32) {
        let b = &world.bodies[0];
        let rel = eye - b.centre;
        let up = rel.normalize();
        let snow = here.temp < 0.0;
        // (a drizzle under 0.3 mm an hour falls unseen: the grid rains a little over half the planet)
        let rate = if here.rain < 0.3 { 0.0 } else { [0.0, 350.0, 1100.0][level.min(2)] * (here.rain / 4.0).sqrt().min(1.6) * if snow { 0.6 } else { 1.0 } };
        if rate <= 0.0 || b.over_top(eye) > 60.0 {
            return;
        }
        self.acc = (self.acc + rate * dt).min(200.0);
        let (speed, height, radius) = if snow { (1.6, 6.0, 14.0) } else { (9.0, 11.0, 20.0) };
        let wind = wind_at(up, here.wind);
        let s = up.any_orthonormal_vector();
        let t = up.cross(s);
        let (row, sea_row) = if snow { ("snow", "snow.sea") } else { ("rain", "rain.sea") };
        while self.acc >= 1.0 {
            self.acc -= 1.0;
            let a = self.rnd() * std::f64::consts::TAU;
            // (none lands within 1.5 m: a drop passing the lens is a blot)
            let r = 1.5 + self.rnd().sqrt() * (radius - 1.5);
            let h = height + self.rnd() * 5.0;
            // started upwind of where it lands by as far as the wind carries it in its fall
            let land = rel + (s * a.cos() + t * a.sin()) * r;
            let Some(mut at) = crate::fx::At::falling(world, 0, land + up * h - wind * (h / speed), wind - up * speed) else {
                self.open -= self.open * 0.01;
                continue;
            };
            if r < 8.0 {
                let open = at.floor as f64 <= rel.length() + 2.0;
                self.open += (open as u8 as f32 - self.open) * 0.02;
            }
            // over water it lands on the surface, not the bed
            let name = match world.water_surface(b.centre + land.normalize() * (at.floor as f64 + 0.1)) {
                Some(w) if w > at.floor as f64 => {
                    (at.floor, at.col) = (w as f32, crate::fx::Column::NONE);
                    sea_row
                }
                _ => row,
            };
            fx.play(crate::fx::id(name), &at);
        }
    }

    /// Lightning round world point `eye` at world time `now`: thunder played as its sound arrives, and the
    /// flash's strength now, 0 to 1.
    pub fn storm(&mut self, sky: &Sky, seed: i32, world: &engine::world::World, eye: DVec3, now: f64, audio: &mut crate::audio::Audio) -> f32 {
        let b = &world.bodies[0];
        let rel = eye - b.centre;
        let Some(sea) = b.sea.map(|s| b.layer_r(s)) else { return 0.0 };
        // a clock set back or jumped: look from now on
        if now < self.struck || now - self.struck > 5.0 {
            self.struck = now;
            self.thunder.clear();
        }
        for (t, at) in strikes(sky, seed, rel.normalize(), sea, self.struck, now, 6) {
            let far = (at - rel).length();
            if far < 2500.0 {
                self.flash = (t, far);
                self.thunder.push((t + far / SOUND, far));
            }
        }
        self.struck = now;
        self.thunder.retain(|&(t, far)| {
            if t > now {
                return true;
            }
            // shortcut: heard in the head (a row reaches 200 m at most), louder near; from the strike's way when
            // a row can reach kilometres
            audio.play("thunder", None, (1.6 - far / 1500.0).clamp(0.4, 1.5) as f32, 1.0, false);
            false
        });
        let since = now - self.flash.0;
        if !(0.0..0.7).contains(&since) {
            return 0.0;
        }
        // a flicker: two bright moments, then a fade; weaker far away
        let k = if since < 0.08 || (0.18..0.26).contains(&since) { 1.0 } else { (-(since - 0.26).max(0.0) * 8.0).exp() * 0.5 };
        (k * (1.0 - self.flash.1 / 2500.0).clamp(0.2, 1.0)) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A step's columns are blended in from the last over the grid's step, the winds through their sign, and a
    /// delta changes only the columns it names.
    #[test]
    fn the_map_blends_from_the_last_step_to_this_one() {
        let mut s = Sky::default();
        let n = 6 * 4 * 4;
        s.take(0.0, vec![[0, 0, (-4i8) as u8, 4, 9, 128]; n], Vec::new());
        assert_eq!(s.m(), 4);
        s.take(100.0, Vec::new(), vec![(5, [200, 100, 4, (-4i8) as u8, 9, 128])]);
        let half = s.texels(100.0 + DT as f64 / 2.0);
        assert_eq!(&half[5 * 4..5 * 4 + 4], &[100, 50, 128, 128], "halfway, the winds through calm");
        assert_eq!(&half[0..4], &[0, 0, 124, 132]);
        let done = s.texels(100.0 + DT as f64 * 2.0);
        assert_eq!(&done[5 * 4..5 * 4 + 4], &[200, 100, 132, 124]);
        assert_eq!(&done[n * 4..n * 4 + 2], &[9, 128], "the snow layers follow the faces'");
        assert_eq!(s.cloud(0.0, 1000.0)[3], 4.0);
        assert_eq!(Sky::default().cloud(0.0, 1000.0), [0.0; 4]);
    }

    /// Lightning strikes only stormy columns, the same for every client of a seed, and once each: a window
    /// split in two gives the strikes of the whole.
    #[test]
    fn lightning_is_the_seeds_and_falls_only_in_storms() {
        let mut s = Sky::default();
        let n = 6 * 8 * 8;
        s.take(0.0, vec![engine::weather::encode(1.0, 1.0, [0.0; 2], 0.0, 10.0); n], Vec::new());
        let d = DVec3::new(1.0, 0.1, 0.05).normalize();
        assert!(strikes(&s, 7, d, 1000.0, 0.0, 600.0, 3).is_empty(), "no storm, no lightning");
        s.take(0.0, vec![engine::weather::encode(1.0, 20.0, [0.0; 2], 0.0, 10.0); n], Vec::new());
        let all = strikes(&s, 7, d, 1000.0, 0.0, 600.0, 3);
        assert!((10..100).contains(&all.len()), "{} strikes in 10 minutes over 49 stormy columns", all.len());
        assert_ne!(all, strikes(&s, 8, d, 1000.0, 0.0, 600.0, 3));
        let mut split = strikes(&s, 7, d, 1000.0, 0.0, 250.5, 3);
        split.extend(strikes(&s, 7, d, 1000.0, 250.5, 600.0, 3));
        let key = |v: &mut Vec<(f64, DVec3)>| v.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut whole = all.clone();
        key(&mut split);
        key(&mut whole);
        assert_eq!(split, whole);
        assert!(all.iter().all(|x| (x.1.length() - 1300.0).abs() < 1e-6));
    }
}
