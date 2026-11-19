//! Luna's bake (worldgen.md, "Luna"; W6, ADR 0070): no air, water or plates. Its parts are stages of the one
//! bake (`super::run`, 0165-bake-stages): `crust`, `impacts`, `ice`, `volcanism`.
//! Old pale crust; a few giant basins, those facing Terra flooded with dark basalt (the maria); craters
//! stamped oldest first, counts by the power law `K`, bowls when small, flat floors, terraces and a peak
//! when large, each with a blanket of debris, older ones worn soft; the youngest bright, with rays. The dust
//! (regolith) is in the kinds: 4 m on the highlands, 1 m on the maria and fresh ground. Lava tubes under the
//! maria with collapsed pits; metal where iron impactors fell, a thorium (KREEP) province by the largest
//! mare. Polar ground the sun never reaches is computed (`SHADOW`); what lies there is the developer's call.
//!
//! What the game keeps is a `bake::Map` of `VERSION` (Terra's maps count from 1, Luna's from 101, so the
//! two never share a file or a hash): per 2 m cell the height and the kind (rows of `data/biomes.toml`
//! after Terra's), and the tubes' balls.
use super::grid::Grid;
use super::land::Rng;
use super::Map;
use fastnoise_lite::{FastNoiseLite, FractalType, NoiseType};
use glam::DVec3;
use rayon::prelude::*;
use std::f64::consts::TAU;
use std::sync::Arc;

/// Changes whenever this bake makes other ground or bytes: 100 + Luna's own version (102: the map gained W4's
/// empty rock field).
pub const VERSION: u32 = 102;

/// The kinds, rows of `data/biomes.toml` after Terra's 15: what the surface is and how deep its dust.
pub const HIGHLAND: u8 = 15;
pub const MARE: u8 = 16;
pub const HIGHLAND_FRESH: u8 = 17;
pub const MARE_FRESH: u8 = 18;
pub const METAL: u8 = 19;
pub const KREEP: u8 = 20;
pub const SHADOW: u8 = 21;
pub const KINDS: [&str; 7] = ["highland", "mare", "fresh highland", "fresh mare", "metal crater", "kreep", "polar shadow"];

/// The rock province under a kind of ground (0165-bake-stages): her crust stage's rock field, read from the
/// kinds the stages laid, so her map's bytes and hash stay as they were. The maria (fresh or not) are basalt over
/// the anorthosite, the thorium province KREEP over it, every other kind (highland, fresh ejecta, a metal crater's
/// floor, the polar shadow) the old anorthosite crust.
pub fn province(kind: u8) -> u8 {
    use super::rock::{LUNA_HIGHLAND, LUNA_KREEP, LUNA_MARE};
    match kind {
        MARE | MARE_FRESH => LUNA_MARE,
        KREEP => LUNA_KREEP,
        _ => LUNA_HIGHLAND,
    }
}

/// Columns per cell: Luna's cells are 2 m (Terra's 8), so a crater 6 m across is three cells.
const CELL: i32 = 2;
/// Metres over the world's sea datum (`world::SEA_DEPTH` under the radius) of the old crust's mean.
const DATUM: f32 = 40.0;
/// The power law: N(>D) = K * area / D^2 craters wider than D, from `D_MIN` to `D_MAX` of the radius.
pub const K: f64 = 0.2;
pub const D_MIN: f64 = 6.0;
const D_MAX: f64 = 0.13;
/// When (0 the oldest crust, 1 now) the maria flooded: craters before it inside a mare are drowned.
const T_MARE: f64 = 0.35;
/// Craters younger than this are bright; younger still and wide enough, rayed.
const FRESH_T: f64 = 0.96;
const RAY_T: f64 = 0.995;
/// How far from a pole the shadow is looked for, radians.
const POLAR: f64 = 0.35;

pub struct Crater {
    pub c: DVec3,
    /// Diameter, m.
    pub d: f64,
    /// When: 0 the oldest, 1 now.
    pub t: f64,
    pub basin: bool,
    pub metal: bool,
    /// Its depth and rim after ageing, m, and the ground at its centre before it fell.
    dep: f64,
    rim: f64,
    h0: f32,
}

/// What the bake measured, for its report and tests.
#[derive(Default)]
pub struct Stats {
    /// Every crater's diameter and whether it can still be seen (no younger, wider crater over its middle,
    /// not drowned by a mare).
    pub craters: Vec<(f64, bool)>,
    /// Polar ground the sun never reaches: cells, m^2, and how far from the pole, radians (each such cell).
    pub shadow: Vec<f64>,
    pub shadow_m2: f64,
    /// Every kind's area, m^2.
    pub area: [f64; 7],
    /// Tubes, their balls, and the pits open to the sky.
    pub tubes: usize,
    pub balls: usize,
    pub pits: usize,
    pub seconds: f64,
}

fn smooth(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Crater {
    pub(super) fn new(c: DVec3, d: f64, t: f64, basin: bool, metal: bool) -> Crater {
        // shallower and lower rimmed as they age (worn by the debris of later ones)
        let fresh = 0.35 + 0.65 * t;
        // a basin is shallow for its width but far deeper than a crater (the real ones: a few km in hundreds)
        let dep = if basin { 0.025 * d } else { (0.15 * d).min(3.0 * (d / 20.0).powf(0.45)) * fresh };
        Crater { c, d, t, basin, metal, dep, rim: if basin { 0.15 } else { 0.25 } * dep, h0: 0.0 }
    }
    /// How far out it reaches, m: its debris to 3 radii (a basin's to 1.5), a rayed one's rays to 8.
    fn reach(&self) -> f64 {
        let k = if self.basin { 1.5 } else if self.rayed() { 8.0 } else { 3.0 };
        0.5 * self.d * k
    }
    fn rayed(&self) -> bool {
        self.t > RAY_T && self.d >= 30.0
    }
    /// The ground's change at x radii from its middle, m: inside, over `h0`; outside, added.
    fn profile(&self, x: f64) -> f64 {
        let (d, dep, rim) = (self.d, self.dep, self.rim);
        if x >= 1.0 {
            let fade = if self.basin { 1.0 - smooth(1.2, 1.5, x) } else { 1.0 - smooth(2.0, 3.0, x) };
            return rim * x.powi(-3) * fade;
        }
        // large ones have a flat floor
        let f = 0.45 * smooth(30.0, 80.0, d);
        let u = ((x - f) / (1.0 - f)).max(0.0);
        let mut p = -dep + (dep + rim) * u * u;
        let terr = smooth(120.0, 250.0, d) * (0.35 + 0.65 * self.t);
        if terr > 0.0 && u > 0.25 {
            let step = (dep + rim) / 5.0;
            p += ((p / step).round() * step - p) * 0.6 * terr;
        }
        p + 0.5 * dep * (1.0 - x / 0.18).max(0.0).powi(2) * smooth(100.0, 200.0, d)
    }
}

/// The ground under way: heights and kinds on a grid (a planet's crust, `mars`, stamps its craters with it too).
pub(super) struct Land {
    pub(super) g: Grid,
    pub(super) r: f64,
    pub(super) h: Vec<f32>,
    pub(super) kind: Vec<u8>,
}

impl Land {
    /// Every cell within `reach` m of `c` (chord), with its distance: per face the box round the cap's samples.
    pub(super) fn cap(&self, c: DVec3, reach: f64, mut f: impl FnMut(usize, f64)) {
        let m = self.g.m;
        let a = reach / self.r;
        let (e1, e2) = (c.any_orthonormal_vector(), c.cross(c.any_orthonormal_vector()));
        let mut boxes = [(f64::MAX, f64::MAX, f64::MIN, f64::MIN); 6];
        let mut put = |d: DVec3| {
            let (fc, x, y) = self.g.locate_xy(d);
            let b = &mut boxes[fc];
            *b = (b.0.min(x), b.1.min(y), b.2.max(x), b.3.max(y));
        };
        put(c);
        for ring in [0.5, 1.0] {
            for k in 0..24 {
                let w = k as f64 / 24.0 * TAU;
                put((c * (a * ring).cos() + (e1 * w.cos() + e2 * w.sin()) * (a * ring).sin()).normalize());
            }
        }
        for (fc, b) in boxes.iter().enumerate() {
            if b.0 > b.2 {
                continue;
            }
            let lo = |v: f64| ((v - 2.0).floor().max(0.0) as usize).min(m - 1);
            let hi = |v: f64| ((v + 2.0).ceil().max(0.0) as usize).min(m - 1);
            let (mut i0, mut j0, mut i1, mut j1) = (lo(b.0), lo(b.1), hi(b.2), hi(b.3));
            // the samples may miss a corner of the cap on this face: grow each side while its edge is inside
            let inside = |i: usize, j: usize| (self.g.d(self.g.idx(fc, i, j)) - c).length() * self.r <= reach;
            loop {
                let (a, b2, c2, d2) = (i0, j0, i1, j1);
                if i0 > 0 && (j0..=j1).any(|j| inside(i0, j)) {
                    i0 = i0.saturating_sub(4);
                }
                if i1 + 1 < m && (j0..=j1).any(|j| inside(i1, j)) {
                    i1 = (i1 + 4).min(m - 1);
                }
                if j0 > 0 && (i0..=i1).any(|i| inside(i, j0)) {
                    j0 = j0.saturating_sub(4);
                }
                if j1 + 1 < m && (i0..=i1).any(|i| inside(i, j1)) {
                    j1 = (j1 + 4).min(m - 1);
                }
                if (a, b2, c2, d2) == (i0, j0, i1, j1) {
                    break;
                }
            }
            for j in j0..=j1 {
                for i in i0..=i1 {
                    let cell = self.g.idx(fc, i, j) as usize;
                    let dist = (self.g.d(cell as u32) - c).length() * self.r;
                    if dist <= reach {
                        f(cell, dist);
                    }
                }
            }
        }
    }
    /// A crater's ground, and with `kinds` Luna's kinds (fresh, metal, rays) round it.
    pub(super) fn stamp(&mut self, cr: &mut Crater, rays: &[(f64, f64)], kinds: bool) {
        cr.h0 = self.h[self.g.locate(cr.c) as usize];
        let (r_c, h0) = (0.5 * cr.d, cr.h0 as f64);
        let (e1, e2) = (cr.c.any_orthonormal_vector(), cr.c.cross(cr.c.any_orthonormal_vector()));
        let (mut h, mut kind) = (std::mem::take(&mut self.h), std::mem::take(&mut self.kind));
        self.cap(cr.c, cr.reach(), |cell, dist| {
            let x = dist / r_c;
            let p = cr.profile(x);
            let w = smooth(1.15, 0.85, x);
            let old = h[cell] as f64;
            h[cell] = (w * (h0 + p) + (1.0 - w) * (old + p)) as f32;
            if !kinds {
                return;
            }
            let k = &mut kind[cell];
            let mare = matches!(*k, MARE | MARE_FRESH);
            if cr.metal && x < 0.55 {
                *k = METAL;
            } else if cr.t > FRESH_T && x < 1.3 {
                *k = if mare { MARE_FRESH } else if *k == METAL { METAL } else { HIGHLAND_FRESH };
            } else if x < 1.0 {
                *k = if mare { MARE } else { HIGHLAND };
            } else if cr.rayed() && x > 1.3 {
                let v = self.g.d(cell as u32) - cr.c;
                let phi = v.dot(e2).atan2(v.dot(e1));
                let on = rays.iter().any(|&(a, len)| {
                    let da = (phi - a + TAU * 1.5).rem_euclid(TAU) - TAU * 0.5;
                    da.abs() < 0.04 + 0.2 / x && x < 2.0 + 6.0 * len
                });
                if on && *k != METAL {
                    *k = if mare { MARE_FRESH } else { HIGHLAND_FRESH };
                }
            }
        });
        (self.h, self.kind) = (h, kind);
    }
    /// A basin facing Terra fills with basalt nearly to its rim, as the real maria did.
    fn flood(&mut self, b: &Crater, wrinkle: &FastNoiseLite) {
        let level = b.h0 as f64 + 0.5 * b.rim;
        let (mut h, mut kind) = (std::mem::take(&mut self.h), std::mem::take(&mut self.kind));
        let r = self.r;
        self.cap(b.c, 0.65 * b.d, |cell, _| {
            if (h[cell] as f64) < level {
                let p = self.g.d(cell as u32) * r;
                h[cell] = (level + 0.4 * wrinkle.get_noise_3d(p.x as f32, p.y as f32, p.z as f32) as f64) as f32;
                kind[cell] = MARE;
            }
        });
        (self.h, self.kind) = (h, kind);
    }
}

/// Bakes Luna of `n` columns per face edge from `seed`: her stages through the one pipeline (`super::run`).
pub fn bake(seed: i32, n: i32) -> Map {
    bake_stats(seed, n).0
}

/// `bake` and what it measured: Luna's stages under the sun of the home's moon (`bodies::moon`), the body
/// this map is (`World::with_grid`).
pub fn bake_stats(seed: i32, n: i32) -> (Map, Stats) {
    let luna = &crate::bodies::BODIES[crate::bodies::moon(&crate::bodies::BODIES).expect("the home has a moon (bodies::parse)")];
    run(&super::Stage::LUNA, seed, n, sun_high(luna.size, luna.orbit))
}

/// The sun's highest over the equator of a body `size` times smaller than the home and `orbit` radii of its
/// parent out, radians: its orbit's tilt from the ecliptic, and the disc's edge (as wide as Luna seen from
/// Terra's ground, a little less: sky.rs `SUN_UNDER_MOON`). Without a size or an orbit, Luna's.
pub fn sun_high(size: Option<f64>, orbit: Option<(f64, f64)>) -> f64 {
    let (size, orbit) = (size.unwrap_or(3.7), orbit.map_or(60.0, |o| o.0));
    crate::sky::INCLINATION + crate::sky::SUN_UNDER_MOON * (1.0 / (size * (orbit - 1.0))).asin()
}

/// The bake of a list (`super::check`ed) on a crust without plates, the stages in its order, and what it
/// measured; `high` is `sun_high` of the body baked.
pub fn run(stages: &[super::Stage], seed: i32, n: i32, high: f64) -> (Map, Stats) {
    use super::Stage::*;
    super::check(stages, true).unwrap_or_else(|e| panic!("a bake of {stages:?}: {e}"));
    let mut w = crust(seed, n);
    for &st in &stages[1..] {
        match st {
            Impacts => impacts(&mut w, stages.contains(&Volcanism)),
            Ice => ice(&mut w, high),
            Volcanism => volcanism(&mut w),
            Plates | Crust | Erosion | Climate | Life => unreachable!("checked: one crust, nothing of plates on it"),
        }
    }
    w.finish()
}

/// A bake of an airless body under way (0165-bake-stages): its ground and kinds, its craters, its tubes.
pub struct Work {
    seed: i32,
    n: i32,
    r: f64,
    rng: Rng,
    land: Land,
    craters: Vec<Crater>,
    tubes: Vec<(u32, f32, f32)>,
    stats: Stats,
    t0: std::time::Instant,
}

pub(super) fn noise(seed: i32, s: i32, freq: f32, oct: i32) -> FastNoiseLite {
    let mut f = FastNoiseLite::with_seed(seed ^ s);
    f.set_noise_type(Some(NoiseType::OpenSimplex2));
    f.set_fractal_type(Some(FractalType::FBm));
    f.set_fractal_octaves(Some(oct));
    f.set_frequency(Some(freq));
    f
}

/// The `crust` stage: the old crust, rough from the start, all highland.
pub fn crust(seed: i32, n: i32) -> Work {
    let t0 = std::time::Instant::now();
    let m = (n / CELL).max(4) as usize;
    let r = n as f64 / (2.0 * std::f64::consts::PI / 3.0).sqrt();
    let rng = Rng::new(seed as u32 as u64 ^ 0x4c75_6e61);
    // 1. the old crust: rough from the start
    let crust = noise(seed, 0x11, 1.0 / 160.0, 4);
    let g = Grid::new(m, r);
    let h: Vec<f32> = (0..g.n() as u32)
        .into_par_iter()
        .map(|c| {
            let p = g.d(c) * r;
            DATUM + 8.0 * crust.get_noise_3d(p.x as f32, p.y as f32, p.z as f32)
        })
        .collect();
    let land = Land { kind: vec![HIGHLAND; g.n()], g, r, h };
    Work { seed, n, r, rng, land, craters: Vec::new(), tubes: Vec::new(), stats: Stats::default(), t0 }
}

/// The `impacts` stage: the giant basins and the craters by the power law, stamped oldest first; with
/// `maria` (the `volcanism` stage listed) the basins facing Terra flood with basalt at `T_MARE`, between the
/// craters before and after it, as the real maria did. Then which craters can still be seen.
pub fn impacts(w: &mut Work, maria: bool) {
    let (r, rng, land) = (w.r, &mut w.rng, &mut w.land);
    let area = 4.0 * std::f64::consts::PI * r * r;
    // 2. the basins: four wide ones on the side that faces Terra (body -x, sky.rs `moon`), two anywhere
    let mut craters = Vec::new();
    for k in 0..6 {
        let (c, d) = if k < 4 { ((DVec3::NEG_X + rng.dir() * 0.7).normalize(), rng.range(0.5, 0.9)) } else { (rng.dir(), rng.range(0.4, 0.7)) };
        craters.push(Crater::new(c, r * d, rng.range(0.0, 0.15), true, false));
    }
    // 3. the craters by the power law: D = D_MIN / sqrt(u) has N(>D) = N * (D_MIN / D)^2
    let d_max = D_MAX * r;
    let count = (K * area / (D_MIN * D_MIN)) as usize;
    for _ in 0..count {
        let d = D_MIN / rng.f().max(1e-12).sqrt();
        let (c, t, metal) = (rng.dir(), rng.f(), rng.f() < 0.01);
        if d <= d_max {
            craters.push(Crater::new(c, d, t, false, metal && d >= 15.0));
        }
    }
    craters.sort_by(|a, b| a.t.total_cmp(&b.t));
    let wrinkle = noise(w.seed, 0x22, 1.0 / 40.0, 2);
    let mut flooded = false;
    for k in 0..craters.len() {
        if maria && !flooded && craters[k].t >= T_MARE {
            flooded = true;
            for b in craters[..k].iter().filter(|b| b.basin && b.c.dot(DVec3::NEG_X) > 0.5) {
                land.flood(b, &wrinkle);
            }
        }
        let rays: Vec<(f64, f64)> = if craters[k].rayed() { (0..12).map(|_| (rng.range(0.0, TAU), rng.f())).collect() } else { Vec::new() };
        let mut cr = std::mem::replace(&mut craters[k], Crater::new(DVec3::X, 1.0, 0.0, false, false));
        land.stamp(&mut cr, &rays, true);
        craters[k] = cr;
    }

    // a moon too small for any crater after the maria still floods them
    if maria && !flooded {
        for b in craters.iter().filter(|b| b.basin && b.c.dot(DVec3::NEG_X) > 0.5) {
            land.flood(b, &wrinkle);
        }
    }

    // which craters can still be seen: youngest first, a crater hides under a younger, wider one over its middle
    let mut cover = vec![0f32; land.g.n()];
    let mut seen = vec![false; craters.len()];
    for k in (0..craters.len()).rev() {
        let cr = &craters[k];
        let cc = land.g.locate(cr.c) as usize;
        let drowned = cr.t < T_MARE && !cr.basin && matches!(land.kind[cc], MARE | MARE_FRESH);
        seen[k] = (cover[cc] as f64) < cr.d && !drowned;
        land.cap(cr.c, 0.4 * cr.d, |cell, _| cover[cell] = cover[cell].max(cr.d as f32));
    }
    drop(cover);
    w.stats.craters = craters.iter().zip(&seen).filter(|(c, _)| !c.basin).map(|(c, &s)| (c.d, s)).collect();

    w.craters = craters;
}

/// The `ice` stage on an airless body: the polar ground no sun reaches (on 8 m cells), a kind of its own.
pub fn ice(w: &mut Work, high: f64) {
    let land = &mut w.land;
    // the polar shadow, on 8 m cells: no sun from any side at its highest (the orbit's tilt and the sun's disc)
    let shadow = polar_shadow(land, high);
    let q = land.g.m / 4;
    for &(c, _) in &shadow {
        let (f, i, j) = (c / (q * q), c % q, (c / q) % q);
        for b in 0..4 {
            for a in 0..4 {
                let cell = land.g.idx(f, 4 * i + a, 4 * j + b) as usize;
                if land.kind[cell] != METAL {
                    land.kind[cell] = SHADOW;
                }
            }
        }
    }
    w.stats.shadow = shadow.iter().map(|s| s.1).collect();
}

/// The `volcanism` stage after the impacts: the thorium (KREEP) province round the largest mare, and the lava
/// tubes under the maria with pits where the roof fell in.
pub fn volcanism(w: &mut Work) {
    let Work { r, rng, land, craters, tubes, stats, .. } = w;
    let r = *r;
    // 7. thorium: a ring sector of highland round the largest mare
    if let Some(b) = craters.iter().filter(|b| b.basin && b.c.dot(DVec3::NEG_X) > 0.5).max_by(|a, b| a.d.total_cmp(&b.d)) {
        let (e1, e2) = (b.c.any_orthonormal_vector(), b.c.cross(b.c.any_orthonormal_vector()));
        let (a0, rb) = (rng.range(0.0, TAU), 0.5 * b.d);
        let mut kind = std::mem::take(&mut land.kind);
        land.cap(b.c, 1.6 * rb, |cell, dist| {
            let v = land.g.d(cell as u32) - b.c;
            let da = ((v.dot(e2).atan2(v.dot(e1)) - a0 + TAU * 1.5).rem_euclid(TAU) - TAU * 0.5).abs();
            if dist > rb && da < 1.0 && kind[cell] == HIGHLAND {
                kind[cell] = KREEP;
            }
        });
        land.kind = kind;
    }

    // 6. the lava tubes under the maria, with pits where the roof fell in
    for b in craters.iter().filter(|b| b.basin && b.c.dot(DVec3::NEG_X) > 0.5) {
        for _ in 0..1 + rng.u64() % 2 {
            let (e1, e2) = (b.c.any_orthonormal_vector(), b.c.cross(b.c.any_orthonormal_vector()));
            let w = rng.range(0.0, TAU);
            let mut d = (b.c + (e1 * w.cos() + e2 * w.sin()) * (rng.range(0.1, 0.35) * b.d / r)).normalize();
            let mut head = e1 * rng.range(-1.0, 1.0) + e2 * rng.range(-1.0, 1.0);
            let rt = rng.range(3.5, 5.5);
            let steps = 30 + rng.u64() % 40;
            let pits = [rng.u64() % steps, rng.u64() % steps];
            let mut any = false;
            for s in 0..steps {
                let cell = land.g.locate(d);
                // in the mare, and off the face seams, where a cave is cut (`world::Body::col_caves`)
                let (_, i, j) = land.g.fij(cell);
                let edge = (rt / CELL as f64) as usize + 2;
                if !matches!(land.kind[cell as usize], MARE | MARE_FRESH) || i.min(j) < edge || i.max(j) + edge >= land.g.m {
                    break;
                }
                let pit = pits.contains(&s);
                // the roof under the spline's ground; the drawn ground's detail may take a metre or two off it
                let roof = if pit { -1.5 } else { rng.range(5.0, 9.0) };
                tubes.push((cell, (rt + roof) as f32, rt as f32));
                stats.pits += pit as usize;
                any = true;
                head = (head - d * head.dot(d)).normalize();
                let turn = rng.range(-0.25, 0.25);
                head = (head * turn.cos() + d.cross(head) * turn.sin()).normalize();
                d = (d + head * (1.2 * rt / r)).normalize();
            }
            stats.tubes += any as usize;
        }
    }
    stats.balls = tubes.len();
}

impl Work {
    /// The map of what the stages made, and what they measured.
    pub fn finish(self) -> (Map, Stats) {
        let Work { seed, n, land, tubes, mut stats, t0, .. } = self;
        for c in 0..land.g.n() as u32 {
            stats.area[(land.kind[c as usize] - HIGHLAND) as usize] += land.g.area(c) as f64;
        }
        stats.shadow_m2 = stats.area[(SHADOW - HIGHLAND) as usize];
        let spawn = spawn(&land, CELL);
        let mut map = Map { version: VERSION, seed, n, m: land.g.m as u32, h: land.h, biome: land.kind, spawn, tubes, ..Map::empty() };
        map.finish(&map.to_bytes());
        stats.seconds = t0.elapsed().as_secs_f64();
        eprintln!("[bake] Luna of seed {seed} planet {n}: {:.1} s", stats.seconds);
        (map, stats)
    }
}

/// The 8 m cells (index on a grid of m/4) near the poles that no sun reaches, the sun at most `high` over the
/// equator (`sun_high`), and their distance from the pole.
fn polar_shadow(land: &Land, high: f64) -> Vec<(usize, f64)> {
    let (m, q) = (land.g.m, land.g.m / 4);
    if q < 2 {
        return Vec::new();
    }
    let coarse = Grid::bare(q, land.r);
    let h: Vec<f32> = (0..6 * q * q)
        .map(|c| {
            let (f, i, j) = (c / (q * q), c % q, (c / q) % q);
            let mut s = 0.0;
            for b in 0..4 {
                for a in 0..4 {
                    s += land.h[(f * m + 4 * j + b) * m + 4 * i + a];
                }
            }
            s / 16.0
        })
        .collect();
    let rad = |h: f32| land.r - crate::world::SEA_DEPTH + h as f64;
    let ground = |d: DVec3| {
        let (f, x, y) = coarse.locate_xy(d);
        let k = |v: f64| (v.floor().max(0.0) as usize).min(q - 1);
        rad(h[(f * q + k(y)) * q + k(x)])
    };
    (0..6 * q * q)
        .into_par_iter()
        .filter_map(|c| {
            let (f, i, j) = (c / (q * q), c % q, (c / q) % q);
            let d = coarse.dir_at(f, i as f64 + 0.5, j as f64 + 0.5);
            let from_pole = d.z.abs().min(1.0).acos();
            if from_pole > POLAR {
                return None;
            }
            let p = d * (rad(h[c]) + 1.0);
            let dz = high * d.z.signum();
            let lit = (0..32).any(|k| {
                let w = k as f64 / 32.0 * TAU;
                let s = DVec3::new(dz.cos() * w.cos(), dz.cos() * w.sin(), dz.sin());
                if s.dot(d) <= 0.0 {
                    return false;
                }
                let mut t = 4.0;
                while t < 700.0 {
                    let x = p + s * t;
                    if ground(x.normalize()) > x.length() {
                        return false;
                    }
                    t *= 1.15;
                }
                true
            });
            (!lit).then_some((c, from_pole))
        })
        .collect()
}

/// Where a player lands on Luna by default (and on a planet's crust, `mars`; its cells `cell` columns wide):
/// gentle ground nearest the +y pole, as on Terra.
pub(super) fn spawn(land: &Land, cell: i32) -> [f64; 3] {
    let m = land.g.m;
    let best = (0..land.g.n() as u32)
        .filter(|&c| {
            let (f, i, j) = land.g.fij(c);
            let at = |i: usize, j: usize| land.h[(f * m + j) * m + i];
            i > 0 && j > 0 && i + 1 < m && j + 1 < m && land.g.d(c).y > 0.9 && {
                let s = (at(i + 1, j) - at(i - 1, j)).hypot(at(i, j + 1) - at(i, j - 1)) / (2.0 * cell as f32);
                s < 0.15
            }
        })
        .max_by(|&a, &b| land.g.d(a).y.total_cmp(&land.g.d(b).y));
    best.map_or([0.0, 1.0, 0.0], |c| land.g.d(c).to_array())
}

/// Where Luna's map of (seed, n) is kept.
pub fn file(seed: i32, n: i32) -> std::path::PathBuf {
    super::file_v(seed, n, VERSION)
}

/// Luna's map of (seed, n): from memory, from disk, else baked now and kept.
pub fn get(seed: i32, n: i32) -> Arc<Map> {
    super::find_in(seed, n, None, &[VERSION]).unwrap_or_else(|| super::keep(bake(seed, n)))
}

/// The map a save names by `hash`: kept, or baked now; None if this bake makes another.
pub fn for_hash(seed: i32, n: i32, hash: u64) -> Option<Arc<Map>> {
    super::find_in(seed, n, Some(hash), &[VERSION]).or_else(|| Some(super::keep(bake(seed, n))).filter(|m| m.hash == hash))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Luna a quarter the standard one's columns (416: 290 m in radius), quick to bake.
    fn small() -> (Map, Stats) {
        bake_stats(7, 416)
    }

    /// A row's own stage list, in another order than Luna's, bakes (review of 0165-bake-stages: the ice stage
    /// looked Luna up by her exact list and panicked), and its ice is that body's: a moon this near and large
    /// has the sun high enough over its poles to leave no shadow; one at Luna's orbit and size has some.
    #[test]
    fn the_ice_stage_bakes_the_body_its_row_names() {
        let moon = |size: f64, orbit: f64| {
            let t = format!("[[body]]\nname = \"A\"\nparent = \"sun\"\nhome = true\nn = 512\ngenerator = \"hills\"\nground = [\"stone\", \"stone\", \"stone\"]\nair = false\nsun = 100.0\n{M}[[body]]\nname = \"B\"\nparent = \"A\"\norbit = {orbit:?}\nsize = {size:?}\ngenerator = \"hills\"\nground = [\"stone\", \"stone\", \"stone\"]\nair = false\nstages = [\"crust\", \"impacts\", \"volcanism\", \"ice\"]\n{M}", M = "kind = \"rocky\"\niron = 0.5\nheat = 0.5\ndensity = [2700.0, 3300.0, 7900.0]\n");
            let rows = crate::bodies::parse(&t).unwrap();
            rows[1].bake.as_ref().unwrap().get(7, 1664).biome.iter().filter(|&&k| k == SHADOW).count()
        };
        assert!(moon(3.7, 60.0) > 0, "Luna's orbit and size: shadow at the poles");
        assert_eq!(moon(1.0, 2.0), 0, "a large near moon: the sun high over its poles, no shadow");
    }

    #[test]
    fn craters_follow_the_power_law_and_the_small_ones_saturate() {
        let (_, s) = small();
        let n = |d: f64, seen: bool| s.craters.iter().filter(|c| c.0 >= d && (c.1 || !seen)).count() as f64;
        // made: N(>D) = N(>D_MIN) (D_MIN / D)^2, less those past D_MAX (the law cut there)
        let cut = |d: f64| d.powi(-2) - (D_MAX * 416.0 / (2.0 * std::f64::consts::PI / 3.0).sqrt()).powi(-2);
        for d in [12.0, 24.0] {
            let want = n(D_MIN, false) * cut(d) / cut(D_MIN);
            assert!((n(d, false) / want - 1.0).abs() < 0.15, "N(>{d}) {} against {want:.0}", n(d, false));
        }
        // seen: the small ones hide under younger ones and maria, so the seen slope is shallower than made's
        let slope = |seen: bool| (n(24.0, seen) / n(6.0, seen)).ln() / 4f64.ln();
        assert!(slope(true) > slope(false) && slope(true) < -1.0, "slopes made {:.2}, seen {:.2}", slope(false), slope(true));
        assert!(n(6.0, true) < n(6.0, false), "some are gone");
    }

    #[test]
    fn maria_face_terra_and_every_kind_is_there() {
        let (m, s) = small();
        let g = Grid::bare(m.m as usize, 1.0);
        let mare = |side: f64| {
            let c: Vec<usize> = (0..m.biome.len()).filter(|&c| g.dir_at(c / (m.m * m.m) as usize, (c % m.m as usize) as f64 + 0.5, ((c / m.m as usize) % m.m as usize) as f64 + 0.5).x * side > 0.3).collect();
            c.iter().filter(|&&c| matches!(m.biome[c], MARE | MARE_FRESH)).count() as f64 / c.len() as f64
        };
        assert!(mare(-1.0) > 0.15 && mare(1.0) < 0.05, "maria on Terra's side {:.2}, the far side {:.2}", mare(-1.0), mare(1.0));
        for (k, a) in s.area.iter().enumerate() {
            if KINDS[k] != "polar shadow" {
                assert!(*a > 0.0, "no {}", KINDS[k]);
            }
        }
        assert!(s.tubes >= 2 && s.pits >= 2, "{} tubes, {} pits", s.tubes, s.pits);
        assert!(m.h.iter().all(|h| (-20.0..100.0).contains(h)), "within the world's clamp");
    }

    #[test]
    fn a_bake_is_the_same_twice_and_reads_back_without_touching_terras_versions() {
        let (a, b) = (bake(3, 128), bake(3, 128));
        assert_eq!(a.hash, b.hash);
        let back = Map::from_bytes(&a.to_bytes()).unwrap();
        assert_eq!((back.hash, back.version, &back.tubes, &back.biome), (a.hash, VERSION, &a.tubes, &a.biome));
        assert_ne!(VERSION, super::super::VERSION);
        // a Terra map's bytes hold no tubes field
        let t = Map { version: super::super::VERSION, n: 64, m: 8, h: vec![1.0; 384], biome: vec![4; 384], ..Map::empty() };
        assert_eq!(t.to_bytes(), postcard::to_stdvec(&t).unwrap());
    }

    /// A deep bowl at the north pole lies in shadow the whole year, flat ground there does not, and a moon too
    /// small for any crater still floods its maria.
    #[test]
    fn a_polar_bowl_is_in_shadow_and_flat_ground_is_not() {
        let (m, r) = (256, 400.0);
        let g = Grid::new(m, r);
        let flat = Land { h: vec![DATUM; g.n()], kind: vec![HIGHLAND; g.n()], g, r };
        assert!(polar_shadow(&flat, sun_high(None, None)).is_empty(), "nothing hides flat ground at the pole");
        let mut bowl = Land { h: flat.h.clone(), kind: flat.kind.clone(), g: Grid::new(m, r), r };
        let mut cr = Crater::new(DVec3::Z, 80.0, 1.0, false, false);
        bowl.stamp(&mut cr, &[], true);
        let s = polar_shadow(&bowl, sun_high(None, None));
        assert!(!s.is_empty() && s.iter().all(|c| c.1 < 0.1), "{} cells in shadow, all in the bowl", s.len());
        let (tiny, st) = bake_stats(1, 64);
        assert!(st.area[(MARE - HIGHLAND) as usize] > 0.0 && tiny.biome.contains(&MARE), "a tiny Luna's maria");
    }

    /// Luna's body stands on the bake: no sea, its tubes hollow the ground under a roof, the highland's dust
    /// lies on anorthosite and a mare's on basalt.
    #[test]
    fn luna_stands_on_its_bake() {
        use crate::blocks::{AIR, BLOCKS};
        use crate::world::{Block, World};
        let map = Arc::new(bake(7, 416));
        let w = World::with_maps(256, 416, 7, None, Some(map.clone()));
        let b = &w.bodies[1];
        assert!(b.sea.is_none() && b.map().is_some());
        assert_eq!(b.caves.len(), map.tubes.len());
        let at = |face: u8, i: i32, j: i32, z: i32| w.generated_block(Block { body: 1, face, pos: glam::IVec3::new(i, j, z) });
        // a tube's ball under its roof (not a pit): air in the middle, rock over it
        let (k, cave) = map.tubes.iter().zip(&b.caves).find(|(t, _)| t.1 > t.2 + 3.0).expect("a roofed ball");
        let (i, j, z) = (cave.at.x as i32, cave.at.y as i32, cave.at.z as i32);
        assert_eq!(at(cave.face, i, j, z), AIR, "inside the tube of {k:?}");
        assert_ne!(at(cave.face, i, j, z + cave.r.ceil() as i32 + 1), AIR, "its roof");
        // the dust and the rock under it, by kind
        let m = map.m as usize;
        for (kind, dust, rock) in [(HIGHLAND, "regolith", "anorthosite"), (MARE, "mare regolith", "mare basalt")] {
            // a cell whose 9 x 9 cells round it are all this kind: the ground's jitter (6 columns) stays inside
            let c = (0..map.biome.len())
                .find(|&c| {
                    let (f, i, j) = (c / (m * m), (c % m) as i64, ((c / m) % m) as i64);
                    (-4..=4).all(|b: i64| (-4..=4).all(|a: i64| (0..m as i64).contains(&(i + a)) && (0..m as i64).contains(&(j + b)) && map.biome[(f * m + (j + b) as usize) * m + (i + a) as usize] == kind))
                })
                .unwrap();
            let (f, i, j) = ((c / (m * m)) as u8, (2 * (c % m) + 1) as i32, (2 * ((c / m) % m) + 1) as i32);
            let top = w.column_height(Block { body: 1, face: f, pos: glam::IVec3::new(i, j, 0) });
            let layer = |depth: f64| b.layer_of(top - depth).floor() as i32;
            assert_eq!(at(f, i, j, layer(0.5)), BLOCKS.id(dust), "{dust} on top");
            assert_eq!(at(f, i, j, layer(8.0)), BLOCKS.id(rock), "{rock} under it");
        }
    }

    /// Luna's generated blocks of generation `gen` on the small Luna: an FNV over every 97th column of each face,
    /// 70 m down from its ground to a metre over it.
    /// `col_caves` from the merged table answers what each cave's span did, round her lava tubes (generation 4)
    /// and her baked ones (generation 3).
    #[test]
    fn luna_col_caves_merged_is_the_union() {
        let map = Arc::new(bake(1, 1664));
        for gen in [3, 4] {
            let w = crate::world::World::with_grid(256, 1664, 1, None, Some(map.clone()), 0.0, gen);
            let n = w.bodies[1].check_col_caves();
            assert!(n > 1000, "generation {gen}: {n} hollow columns checked");
            let found = w.bodies[1].check_over_cave();
            assert!(found > 10, "generation {gen}: {found} squares over a mouth");
            // (her tube walk's faces side by side are the walk in order)
            let (b, seed) = (&w.bodies[1], w.bodies[1].baked_seed().unwrap());
            assert!(crate::caves::systems(b, seed) == crate::caves::systems_of_slow(b, seed, &crate::caves::CAVES.kinds));
        }
    }

    fn luna_blocks_hash(gen: u8) -> u64 {
        use crate::world::{Block, World};
        let map = Arc::new(bake(7, 416));
        let w = World::with_grid(256, 416, 7, None, Some(map), 0.0, gen);
        let b = &w.bodies[1];
        let mut h = crate::world::FNV0;
        for c in (0..6 * 416 * 416).step_by(97) {
            let (f, i, j) = ((c / (416 * 416)) as u8, (c % 416) as i32, ((c / 416) % 416) as i32);
            let top = w.column_height(Block { body: 1, face: f, pos: glam::IVec3::new(i, j, 0) });
            for z in b.layer_of(top - 70.0).floor() as i32..=b.layer_of(top + 1.0).ceil() as i32 {
                h = crate::world::fnv(h, &w.generated_block(Block { body: 1, face: f, pos: glam::IVec3::new(i, j, z) }).to_le_bytes());
            }
        }
        h
    }

    /// A lock taken on the code before Luna's rock field (0165-bake-stages): her ground of generations 0 to 3,
    /// block for block, as it was.
    #[test]
    fn luna_generates_as_before_up_to_generation_3() {
        for gen in [0, 3] {
            assert_eq!(format!("{:016x}", luna_blocks_hash(gen)), "a22a376de6be21ba", "generation {gen}");
        }
    }

    /// Generation 4 (0165-bake-stages): Luna's rock field. Her provinces are hers alone, the maria's share of her
    /// columns is her map's, `Body::rock` (`Rocks::at`) answers what is generated there, and under the dust the
    /// highlands are anorthosite and the maria basalt over anorthosite; her ground differs from generation 3's.
    #[test]
    fn a_generation_4_luna_has_anorthosite_highlands_and_basalt_maria() {
        use crate::bake::rock::{LUNA_HIGHLAND, LUNA_KREEP, LUNA_MARE};
        use crate::blocks::BLOCKS;
        use crate::rock::ROCKS;
        use crate::world::{Block, World, GENERATION};
        assert!(GENERATION >= 4);
        let map = Arc::new(bake(7, 416));
        let w = World::with_grid(256, 416, 7, None, Some(map.clone()), 0.0, 4);
        let b = &w.bodies[1];
        let luna = crate::bodies::find("luna").unwrap();
        let id = |n: &str| BLOCKS.id(n);
        let (mut cols, mut mare, mut seen) = (0, 0, [0usize; 3]);
        for c in (0..6 * 416 * 416).step_by(31) {
            let (f, i, j) = ((c / (416 * 416)) as u8, (c % 416) as i32, ((c / 416) % 416) as i32);
            let rock = b.col_rock(f, i, j);
            let p = rock.province;
            assert!([LUNA_HIGHLAND, LUNA_MARE, LUNA_KREEP].contains(&p) && ROCKS.provinces[p as usize].body == luna, "column {c}: province {p}");
            let (ci, cj) = (i as f64 + 0.5, j as f64 + 0.5);
            let kind = b.biome_at(f as usize, ci, cj, b.dir(f as usize, ci, cj)).unwrap();
            assert_eq!(p, province(kind), "column {c}: the province is its kind's");
            cols += 1;
            mare += (p == LUNA_MARE) as usize;
            seen[(p - LUNA_HIGHLAND) as usize] += 1;
            // what is generated under the dust is the rock field's, but where a tube hollows it
            let top = w.column_height(Block { body: 1, face: f, pos: glam::IVec3::new(i, j, 0) });
            let caves = b.col_caves(f, i, j);
            for depth in [6.0, 20.0, 45.0, 70.0] {
                let z = b.layer_of(top - depth).floor() as i32;
                // (or an ore body of hers takes it, 0166-luna-ore-and-caves)
                if caves.iter().any(|&(lo, hi)| (lo..hi).contains(&z)) || b.ore_at(f, i, j, z, top).is_some() {
                    continue;
                }
                let got = w.generated_block(Block { body: 1, face: f, pos: glam::IVec3::new(i, j, z) });
                let band = crate::biomes::ROWS.biomes[kind as usize].ground[1];
                if got != band {
                    assert_eq!(got, b.rock(rock, b.layer_r(z as f64 + 0.5) as f32, u16::MAX), "column {c}, {depth} m down");
                }
                if depth == 70.0 {
                    assert_eq!(got, id("anorthosite"), "column {c}: anorthosite 70 m down under every kind");
                }
                if depth == 6.0 && p == LUNA_MARE && got != band {
                    assert_eq!(got, id("mare basalt"), "column {c}: a mare's basalt under its dust");
                }
            }
        }
        // the maria's share of the columns against the map's cells (by area)
        let g = Grid::new(map.m as usize, 1.0);
        let all: f64 = (0..g.n() as u32).map(|c| g.area(c) as f64).sum();
        let want = (0..g.n() as u32).filter(|&c| matches!(map.biome[c as usize], MARE | MARE_FRESH)).map(|c| g.area(c) as f64).sum::<f64>() / all;
        let got = mare as f64 / cols as f64;
        assert!((got - want).abs() < 0.02 && want > 0.05, "maria {got:.3} of the columns, {want:.3} of the map");
        assert!(seen.iter().all(|&k| k > 0), "every province seen: {seen:?}");
        println!("highland {:.3}, mare {:.3}, kreep {:.3} of {cols} columns; the map's maria {want:.3}", seen[0] as f64 / cols as f64, seen[1] as f64 / cols as f64, seen[2] as f64 / cols as f64);
        let h = format!("{:016x}", luna_blocks_hash(4));
        assert_ne!(h, "a22a376de6be21ba", "generation 4's ground is not 3's");
        // and a lock on it, as generation 3's above
        assert_eq!(h, "b0f00158257f21d2");
        // generation 5 (Mars) leaves her as 4 made her
        assert_eq!(format!("{:016x}", luna_blocks_hash(5)), h);
    }

    /// Generation 4's Luna on her rock field (0166-luna-ore-and-caves), the standard Luna: each of her kinds of
    /// mineral has bodies, only in its provinces, host rock, biome and depth window, and none of Terra's is on her;
    /// none of them, and none of her caves, is placed for where her spawn is; her caves are `data/caves.toml`'s
    /// kinds cut in her rock under the dust band, with no water table; the ledger opens with her bodies after the
    /// planet's and a dig on her books against her own.
    #[test]
    fn a_generation_4_luna_has_ore_and_caves_by_her_rock() {
        use crate::ore::{count, MINERALS};
        use crate::world::{Block, World};
        let map = Arc::new(bake(1, 1664));
        let mut w = World::with_grid(256, 1664, 1, None, Some(map.clone()), 0.0, 4);
        let luna = crate::bodies::moon(&crate::bodies::BODIES).unwrap();
        let b = &w.bodies[luna];
        assert!(b.sea.is_none() && b.rock_field() && b.rock_body() == luna);
        let held = count(b);
        let mine: Vec<usize> = (0..MINERALS.kinds.len()).filter(|&k| MINERALS.kinds[k].body == luna).collect();
        assert!(!mine.is_empty() && mine.len() <= 3, "at most three kinds of Luna's: {mine:?}");
        for &k in &mine {
            let kind = &MINERALS.kinds[k];
            let of: Vec<usize> = (0..b.ores.len()).filter(|&o| b.ores[o].kind as usize == k).collect();
            assert!(!of.is_empty(), "{}: no body", kind.name);
            let blocks: u32 = of.iter().map(|&o| held[o].blocks).sum();
            assert!(blocks > 0, "{}: no block", kind.name);
            for &o in &of {
                let d = &b.ores[o];
                let (f, i, j) = (d.face as usize, d.at.x, d.at.y);
                let dir = b.dir(f, i, j);
                let rock = b.rock_at(f, i, j, dir);
                assert!(kind.provinces.contains(&rock.province), "{}: in province {}", kind.name, rock.province);
                let depth = b.ground_r(f, i, j) - b.layer_r(d.at.z);
                assert!(depth >= kind.depth[0] - 0.5 && depth <= kind.depth[1] + 0.5, "{}: {depth:.1} m down", kind.name);
                if let Some(h) = kind.host {
                    assert_eq!(b.rock(rock, b.layer_r(d.at.z) as f32, 0), h, "{}: in its host", kind.name);
                }
                if !kind.biomes.is_empty() {
                    assert!(kind.biomes.contains(&b.biome_at(f, i, j, dir).unwrap()), "{}: under its kind of ground", kind.name);
                }
            }
            println!("{}: {} bodies, {blocks} blocks", kind.name, of.len());
        }
        assert!(b.ores.iter().all(|o| MINERALS.kinds[o.kind as usize].body == luna), "none of Terra's kinds on Luna");
        // her caves: the kinds of data/caves.toml on her rock, under the dust band, no water anywhere
        let sys = crate::caves::systems(b, b.baked_seed().unwrap());
        assert!(!sys.is_empty(), "lava tubes in her maria");
        let basalt = crate::blocks::BLOCKS.id("mare basalt");
        for s in &sys {
            assert_eq!(crate::caves::CAVES.kinds[s.kind as usize].body, luna);
            // the passage's balls (the shafts of its skylights come after them)
            for c in &s.balls[..s.axis.len()] {
                for (i, j, z) in crate::caves::cells(b, c) {
                    let (ground, r) = (b.col_height(c.face, i, j), b.layer_r(z as f64 + 0.5));
                    assert!(ground - r >= 4.0, "a tube cell {:.1} m under the ground, in the band", ground - r);
                    assert_eq!(b.rock(b.col_rock(c.face, i, j), r as f32, 0), basalt, "a tube cut in her basalt");
                }
            }
            assert!(!s.openings.is_empty(), "a tube has a skylight");
        }
        // a skylight is open from the sky to the tube's floor
        // (over the heap of the roof that fell, at most 0.45 of the tube's height on its floor)
        let (mut skylights, mut open) = (0, 0);
        for s in &sys {
            for o in &s.openings {
                let p = s.axis[(o.0 + o.1) / 2];
                let (f, i, j) = (s.face, p.x.floor() as i32, p.y.floor() as i32);
                let top = w.column_height(Block { body: luna as u8, face: f, pos: glam::IVec3::new(i, j, 0) });
                let (lo, hi) = (b.layer_of(p.z + 3.5).floor() as i32, b.layer_of(top).floor() as i32);
                skylights += 1;
                open += (lo..hi).all(|z| w.generated_block(Block { body: luna as u8, face: f, pos: glam::IVec3::new(i, j, z) }) == crate::blocks::AIR) as usize;
            }
        }
        assert!(open * 10 >= skylights * 9, "{open} of {skylights} skylights open from the sky into the tube");
        assert_eq!(crate::caves::water_table(b, 2, 100.5, 100.5), f64::NEG_INFINITY, "no water table on Luna");
        // nothing moves with her spawn
        let (ores, caves) = (b.ores.clone(), b.caves.clone());
        let seed = b.baked_seed().unwrap();
        let spawn = &mut Arc::get_mut(&mut w.bodies).unwrap()[luna].spawn;
        *spawn = -*spawn;
        let b = &w.bodies[luna];
        assert_eq!(crate::ore::place(b, seed), ores, "her ores with her spawn moved");
        assert_eq!(crate::caves::make(b, seed), caves, "her caves with her spawn moved");
        // the ledger: the planet's bodies (none on this unbaked planet), then hers; a dig on her books against her
        w.open_ledger();
        let open = w.ore_left.clone().unwrap();
        let base = w.ore_base(luna);
        assert_eq!(open.len(), base + w.bodies[luna].ores.len());
        assert_eq!(open[base..].to_vec(), count(&w.bodies[luna]).iter().map(|h| h.blocks as i64).collect::<Vec<_>>());
        let cell = crate::ore::cells(&w.bodies[luna], 0)[0];
        let blk = Block { body: luna as u8, face: w.bodies[luna].ores[0].face, pos: cell };
        assert_eq!(w.ore_cell(blk), Some(0));
        w.set_block(blk, crate::blocks::AIR);
        let left = w.ore_left.clone().unwrap();
        assert_eq!(left[base], open[base] - 1, "the dig books against Luna's body");
        assert_eq!(left.iter().sum::<i64>(), open.iter().sum::<i64>() - 1, "and nothing else");
        assert_eq!(w.ore_recount(&open), left, "the recount from the edits agrees");
        // generation 3: none of it, her baked tubes as before
        let w3 = World::with_grid(256, 1664, 1, None, Some(map.clone()), 0.0, 3);
        assert!(w3.bodies[luna].ores.is_empty() && w3.bodies[luna].caves.len() == map.tubes.len(), "generation 3's Luna as before");
    }

    /// What generation 4's ore and caves cost the standard Luna's load (seed 1), against generation 3's on the
    /// same code; run by hand: `cargo test -p engine --release luna_load_cost -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn luna_load_cost() {
        use crate::world::World;
        let map = Arc::new(bake(1, 1664));
        for gen in [3, 4, 3, 4] {
            let t = std::time::Instant::now();
            let mut w = World::with_grid(256, 1664, 1, None, Some(map.clone()), 0.0, gen);
            let built = t.elapsed().as_secs_f64();
            w.open_ledger();
            eprintln!("LUNA_LOAD gen {gen}: world {built:.3} s, ledger {:.3} s more, {} caves, {} ore bodies", t.elapsed().as_secs_f64() - built, w.bodies[1].caves.len(), w.bodies[1].ores.len());
        }
    }

    /// Stand lines inside the standard Luna's longest lava tube (seed 1, generation 4), feet a metre over its floor,
    /// looking along it: `cargo test -p engine luna_tube_stands -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn luna_tube_stands() {
        use crate::world::World;
        let w = World::with_grid(256, 1664, 1, None, Some(Arc::new(bake(1, 1664))), 0.0, 4);
        let b = &w.bodies[1];
        let sys = crate::caves::systems(b, b.baked_seed().unwrap());
        // a stand line beside the first body of each of her minerals, a gallery dug toward its middle, lamps in it
        for k in (0..crate::ore::MINERALS.kinds.len()).filter(|&k| crate::ore::MINERALS.kinds[k].body == 1) {
            let Some(o) = b.ores.iter().find(|o| o.kind as usize == k) else { continue };
            let f = o.face as usize;
            let (c, sn) = (o.angle.cos(), o.angle.sin());
            let back = o.a + 3.0;
            let (si, sj) = (o.at.x - c * back, o.at.y - sn * back);
            let up = b.dir(f, si, sj);
            let r = if up.y.abs() > 0.9 { DVec3::Z } else { DVec3::Y };
            let ahead = (r - up * r.dot(up)).normalize();
            let to = b.dir(f, o.at.x, o.at.y) - up;
            let yaw = to.dot(up.cross(ahead)).atan2(to.dot(ahead)).to_degrees();
            let feet = o.at.z.floor() - 1.0;
            let depth = b.ground_r(f, si, sj) - b.layer_r(feet);
            let mut place = Vec::new();
            for step in 0..(back as i32 - 1) {
                let (di, dj) = ((c * step as f64).round() as i32, (sn * step as f64).round() as i32);
                for (wi, wj) in [(0, 0), (-sn.round() as i32, c.round() as i32), (sn.round() as i32, -c.round() as i32)] {
                    for dz in 0..3 {
                        place.push(format!("{}:{}:{dz}:{}", di + wi, dj + wj, if step % 4 == 2 && dz == 2 && (wi, wj) != (0, 0) { "lamp" } else { "air" }));
                    }
                }
            }
            let mut dug = std::collections::BTreeMap::new();
            for p in &place {
                let d: Vec<i32> = p.split(':').take(3).map(|v| v.parse().unwrap()).collect();
                let at = glam::IVec3::new(si.floor() as i32 + d[0], sj.floor() as i32 + d[1], feet as i32 + d[2]);
                *dug.entry(w.generated_block(crate::world::Block { body: 1, face: f as u8, pos: at })).or_insert(0) += 1;
            }
            println!("DUG {dug:?}");
            println!("ORE {} at={:.6},{:.6},{:.6} depth={depth:.1} yaw={yaw:.0} pitch=-5 place={}", crate::ore::MINERALS.kinds[k].name, up.x, up.y, up.z, place.join(","));
        }
        let s = sys.iter().max_by_key(|s| s.axis.len()).unwrap();
        for k in [s.axis.len() / 3, s.axis.len() / 2] {
            let (q, at) = (s.axis[k], s.axis[k + 12]);
            let f = s.face as usize;
            let up = b.dir(f, q.x, q.y);
            let r = if up.y.abs() > 0.9 { DVec3::Z } else { DVec3::Y };
            let ahead = (r - up * r.dot(up)).normalize();
            let to = b.dir(f, at.x, at.y) - up;
            let yaw = to.dot(up.cross(ahead)).atan2(to.dot(ahead)).to_degrees();
            let floor = s.balls[k].at.z - s.balls[k].floor;
            let (ci, cj) = (q.x.floor() as i32, q.y.floor() as i32);
            let col: Vec<u16> = (floor as i32 - 1..floor as i32 + 8).map(|z| w.generated_block(crate::world::Block { body: 1, face: s.face, pos: glam::IVec3::new(ci, cj, z) })).collect();
            println!("COLUMN from layer {}: {col:?}, caves {:?}", floor as i32 - 1, b.col_caves(s.face, ci, cj));
            println!("STAND at={:.6},{:.6},{:.6} depth={:.1} yaw={yaw:.0} pitch=0 (tube of {} m, point {k})", up.x, up.y, up.z, b.ground_r(f, q.x, q.y) - b.layer_r(floor + 1.0), s.axis.len());
        }
    }

    /// The report of the standard Luna (1664 columns, seed 5): its bake time, crater counts by size, the kinds'
    /// areas, the shadow, and `out/luna/map.png` (longitude across, latitude down, Terra's side in the middle).
    /// `cargo test -p engine --release luna_report -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn luna_report() {
        let n = std::env::var("LUNA_N").ok().and_then(|v| v.parse().ok()).unwrap_or(1664);
        let (m, s) = bake_stats(5, n);
        let made = |d: f64| s.craters.iter().filter(|c| c.0 >= d).count();
        let seen = |d: f64| s.craters.iter().filter(|c| c.0 >= d && c.1).count();
        let bins: Vec<String> = [6.0, 12.0, 24.0, 48.0, 96.0].iter().map(|&d| format!("[{d}, {}, {}]", made(d), seen(d))).collect();
        let slope = |f: &dyn Fn(f64) -> usize| (f(48.0) as f64 / f(6.0) as f64).ln() / 8f64.ln();
        let kinds: Vec<String> = KINDS.iter().zip(s.area).map(|(k, a)| format!("\"{k}\": {:.3}", a / 1e6)).collect();
        let pole = s.shadow.iter().fold(0f64, |a, &b| a.max(b)).to_degrees();
        println!(
            "{{\"seconds\": {:.2}, \"bytes\": {}, \"craters_made_seen_over_d\": [{}], \"slope_made\": {:.2}, \"slope_seen\": {:.2}, \"km2\": {{{}}}, \"shadow_m2\": {:.0}, \"shadow_cells_8m\": {}, \"shadow_farthest_deg\": {pole:.1}, \"tubes\": {}, \"balls\": {}, \"pits\": {}}}",
            s.seconds, m.to_bytes().len(), bins.join(", "), slope(&made), slope(&seen), kinds.join(", "), s.shadow_m2, s.shadow.len(), s.tubes, s.balls, s.pits
        );
        // the sheet: shaded relief tinted by kind
        let (w, hh) = (1440usize, 720usize);
        let g = Grid::bare(m.m as usize, 1.0);
        let mut px = vec![0u8; w * hh * 3];
        for y in 0..hh {
            for x in 0..w {
                let (lon, lat) = ((x as f64 + 0.5) / w as f64 * TAU - std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2 - (y as f64 + 0.5) / hh as f64 * std::f64::consts::PI);
                // the middle of the sheet is body -x, Terra's side
                let d = DVec3::new(-lat.cos() * lon.sin(), lat.cos() * lon.cos(), lat.sin());
                let at = |d: DVec3| {
                    let (f, i, j) = g.locate_xy(d.normalize());
                    m.height(f, i, j, 0)
                };
                let e = 1.0 / m.m as f64;
                let east = DVec3::Z.cross(d).normalize_or(DVec3::Y);
                let shade = (0.5 + 0.12 * (at(d - east * e) - at(d + east * e)) as f64).clamp(0.0, 1.0);
                let (f, i, j) = g.locate_xy(d);
                let k = m.biome(f, i, j);
                let tint = match k {
                    MARE => [0.45, 0.45, 0.5],
                    MARE_FRESH => [0.7, 0.7, 0.75],
                    HIGHLAND_FRESH => [1.0, 1.0, 1.0],
                    METAL => [1.0, 0.5, 0.2],
                    KREEP => [0.9, 0.75, 0.5],
                    SHADOW => [0.3, 0.5, 1.0],
                    _ => [0.85, 0.85, 0.83],
                };
                for c in 0..3 {
                    px[(y * w + x) * 3 + c] = (255.0 * tint[c] * (0.35 + 0.65 * shade)) as u8;
                }
            }
        }
        std::fs::create_dir_all("../../out/luna").unwrap();
        let file = std::fs::File::create("../../out/luna/map.png").unwrap();
        let mut e = png::Encoder::new(std::io::BufWriter::new(file), w as u32, hh as u32);
        e.set_color(png::ColorType::Rgb);
        e.write_header().unwrap().write_image_data(&px).unwrap();
    }
}
