//! A planet's crust without plates (planet-model.md step 5a, 0167-mars-bake): the bake of a body that circles
//! the sun and lists `crust` first (`bodies::Planet`), the Mars-like planet's. Real-world logic, the relief
//! exaggerated for a planet of 2.3 km as Terra's is (ADR 0043, 0060), each landform something to find:
//! - `crust`: the dichotomy: low smooth northern plains (`SHARE` of the surface), high old southern highlands and
//!   a scarp between them;
//! - `impacts`: two giant basins in the highlands, then Luna's craters (`luna::K`'s power law times the row's `age`,
//!   her crater shapes), each kept only where the ground it fell on is older than it: the plains were laid at
//!   `T_PLAINS`, so their older craters lie buried; with `volcanism` listed, those after `T_VOLC` fall after it;
//! - `volcanism`: one plate over a hot spot, dead now: a bulge on the dichotomy's edge, lava plains over it, three
//!   shields in a row on it and one far larger past its edge, each with a summit caldera; lava tubes marked down
//!   their flanks (as Luna's map marks hers, for the cave run); the great canyon the crust tore as the bulge rose
//!   (along the dichotomy, away from the giant); then the young craters;
//! - `erosion`, long ago and dry now: chaos at the canyon's end and the outflow channels it burst into the plains;
//!   the drainage of Terra's bake (`land::drain`) to the old shore of the northern plains, whose valley networks are
//!   carved into the highlands (their beds kept as the map's dry `rivers`), the lake beds they filled in crater
//!   floors and the basins, and deltas at their mouths;
//! - `ice`: layered polar caps with spiral troughs, frost on the floors of high-latitude craters.
//!
//! What the game will keep is a `bake::Map` of `VERSION` (Terra's maps count from 1, Luna's from 101, a planet's
//! crust's from 201): per 4 m cell the height and the kind (`KINDS`, after Luna's), the valleys' beds and the tubes'
//! balls, kept under `file`'s name. Nothing reads where a player lands: the map's `spawn` is picked after the last
//! stage.
use super::grid::Grid;
use super::land::{self, Rng};
use super::luna::{self, Crater, Land};
use super::{Map, Reach, Stage};
use crate::bodies::Planet;
use fastnoise_lite::FastNoiseLite;
use glam::DVec3;
use rayon::prelude::*;
use std::collections::HashMap;
use std::f64::consts::{PI, TAU};
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex, Weak};
use std::time::Instant;

/// Changes whenever this bake makes other ground or bytes: 200 + its own version.
pub const VERSION: u32 = 201;

/// The kinds, after Luna's (rows of `data/biomes.toml` to come, with the run that puts the planet in the world).
pub const HIGHLAND: u8 = 22;
pub const PLAIN: u8 = 23;
pub const LAVA: u8 = 24;
pub const SHIELD: u8 = 25;
pub const CANYON: u8 = 26;
pub const CHAOS: u8 = 27;
pub const CHANNEL: u8 = 28;
pub const SEDIMENT: u8 = 29;
pub const DELTA: u8 = 30;
pub const CAP: u8 = 31;
pub const FROST: u8 = 32;
pub const KINDS: [&str; 11] = ["cratered highland", "dust plain", "lava plain", "basalt shield", "canyon floor", "chaos", "outflow channel", "sediment basin", "delta", "polar ice", "frost"];

/// Columns per cell: 4 m cells (Terra's 8, Luna's 2).
const CELL: i32 = 4;
/// Metres over the world's sea datum: the highlands' mean and the plains'.
const HIGH: f32 = 40.0;
const LOW: f32 = -25.0;
/// The plains' share of the surface (the real northern lowlands about a third), and the scarp's half width, rad.
const SHARE: f64 = 0.38;
const SCARP: f64 = 0.035;
/// Craters from `D_MIN` m (smaller ones the dust filled) to `D_MAX` of the radius.
const D_MIN: f64 = 12.0;
const D_MAX: f64 = 0.1;
/// When (0 the oldest crust, 1 now) the plains were laid, and the lava over the bulge.
const T_PLAINS: f32 = 0.8;
const T_VOLC: f32 = 0.9;
/// The northern plains' old shore, m: where the valleys' water met standing water.
const SHORE: f32 = -12.0;
/// The giant's summit at most, m (the world's columns end at `world::AMP` less a tree: 378).
const TOP: f32 = 360.0;
/// A giant basin's depth at most, m: 0.045 of its width reaches it near n 3328; on larger planets the basins
/// stay this deep, so their floors (with craters in them) stay over the world's floor (-56).
const BASIN_DEP: f64 = 66.0;
/// The drained area a valley starts at, m^2.
const A_VALLEY: f32 = 6e4;
/// The bulge's radius, rad, and height, m.
const RB: f64 = 0.6;
const HB: f64 = 70.0;

/// A shield volcano as the volcanism stage built it.
#[derive(Clone, Copy, Debug)]
pub struct Shield {
    pub c: DVec3,
    /// Its radius, m.
    pub radius: f64,
    /// The ground round its foot, its caldera's rim and how deep the caldera is, m.
    pub base: f32,
    pub summit: f32,
    pub caldera: f32,
    pub giant: bool,
}

/// What the bake measured, for its report, its pictures and its tests.
#[derive(Default)]
pub struct Stats {
    /// The plains' centre, and their share of the surface.
    pub north: DVec3,
    pub lowland: f64,
    /// Every crater: its centre, diameter (m) and whether it can still be seen.
    pub craters: Vec<(DVec3, f64, bool)>,
    pub basins: Vec<(DVec3, f64)>,
    pub bulge: DVec3,
    pub shields: Vec<Shield>,
    /// The canyon's axis, from the bulge, and its length, m.
    pub canyon: Vec<DVec3>,
    pub canyon_m: f64,
    /// The outflow channels' axes, from the chaos.
    pub channels: Vec<Vec<DVec3>>,
    /// Valley cells, and their mouths: into the plains, a basin, a lake bed, other ground (younger lava).
    pub valleys: usize,
    pub mouths: [usize; 4],
    pub deltas: usize,
    /// Every kind's area, m^2, and its mean height, m.
    pub area: [f64; 11],
    pub mean_h: [f64; 11],
    /// Tubes, their balls, and the pits open to the sky.
    pub tubes: usize,
    pub balls: usize,
    pub pits: usize,
    pub stages: Vec<(&'static str, f64)>,
    pub seconds: f64,
}

fn smooth(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn angle(a: DVec3, b: DVec3) -> f64 {
    a.dot(b).clamp(-1.0, 1.0).acos()
}
/// The point `s` radians from `c` toward the tangent direction `u`.
fn toward(c: DVec3, u: DVec3, s: f64) -> DVec3 {
    let u = (u - c * u.dot(c)).normalize();
    (c * s.cos() + u * s.sin()).normalize()
}
/// The local east and north at `c` (+z the pole).
fn east(c: DVec3) -> DVec3 {
    DVec3::Z.cross(c).normalize_or(DVec3::X)
}
fn northward(c: DVec3) -> DVec3 {
    c.cross(east(c))
}
fn at(n: &FastNoiseLite, p: DVec3) -> f32 {
    n.get_noise_3d(p.x as f32, p.y as f32, p.z as f32)
}

/// A bake of a planet's crust under way: its ground and kinds, when each cell's surface was last made, its craters
/// and features.
pub struct Work {
    seed: i32,
    n: i32,
    r: f64,
    rng: Rng,
    land: Land,
    /// When each cell's surface was last made, 0 the oldest crust .. 1 now: a crater older than it is buried.
    age: Vec<f32>,
    /// The crust's lowland.
    low: Vec<bool>,
    north: DVec3,
    theta0: f64,
    craters: Vec<Crater>,
    young: Vec<Crater>,
    canyon: Vec<DVec3>,
    tubes: Vec<(u32, f32, f32)>,
    rivers: Vec<Reach>,
    stats: Stats,
    t0: Instant,
}

impl Work {
    /// Every cell within `a` radians of `c`, with its angle.
    fn each(land: &Land, c: DVec3, a: f64, mut f: impl FnMut(usize, f64)) {
        let r = land.r;
        land.cap(c, 2.0 * (0.5 * a.min(PI)).sin() * r * 1.0001, |cell, dist| f(cell, 2.0 * (dist / (2.0 * r)).min(1.0).asin()));
    }
}

/// The bake of a list (`super::check`ed for a planet) on a planet's crust, the stages in its order, and what it
/// measured; `age` is the row's (`bodies::Planet`).
pub fn run(stages: &[Stage], seed: i32, n: i32, age: f64) -> (Map, Stats) {
    use Stage::*;
    super::check(stages, false).unwrap_or_else(|e| panic!("a bake of {stages:?}: {e}"));
    assert_eq!(stages[0], Crust, "a planet's crust");
    // the title menu's bar ("Making the universe"): each stage as it starts
    let say = |k: usize| {
        let what = match stages[k] {
            Impacts => "the planet's craters",
            Volcanism => "the planet's volcanoes",
            Erosion => "the planet's dry valleys",
            Ice => "the planet's ice",
            _ => "the planet's crust",
        };
        super::report(what, k as f32 / stages.len() as f32);
    };
    say(0);
    let mut lap = Instant::now();
    let mut w = crust(seed, n);
    w.stats.stages.push(("crust", lap.elapsed().as_secs_f64()));
    for (k, &st) in stages.iter().enumerate().skip(1) {
        say(k);
        lap = Instant::now();
        let name = match st {
            Impacts => (impacts(&mut w, stages.contains(&Volcanism), age), "impacts").1,
            Volcanism => (volcanism(&mut w), "volcanism").1,
            Erosion => (erosion(&mut w), "erosion").1,
            Ice => (ice(&mut w), "ice").1,
            Plates | Crust | Climate | Life => unreachable!("checked: one crust, nothing of plates or air on it"),
        };
        w.stats.stages.push((name, lap.elapsed().as_secs_f64()));
    }
    w.finish()
}

/// The `crust` stage: the dichotomy. The plains are a cap round a centre near the north pole (the seed tilts it),
/// lobed; the highlands rough and old, the plains smooth and laid at `T_PLAINS`.
pub fn crust(seed: i32, n: i32) -> Work {
    let t0 = Instant::now();
    let m = (n / CELL).max(4) as usize;
    let r = n as f64 / (2.0 * PI / 3.0).sqrt();
    let mut rng = Rng::new(seed as u32 as u64 ^ 0x4d61_7273);
    let north = (DVec3::Z + rng.dir() * 0.45).normalize();
    let theta0 = (1.0 - 2.0 * SHARE).acos();
    let (lobes, rough, region) = (luna::noise(seed, 0x31, 1.0 / 900.0, 3), luna::noise(seed, 0x32, 1.0 / 180.0, 4), luna::noise(seed, 0x33, 1.0 / 1200.0, 2));
    let g = Grid::new(m, r);
    let cells: Vec<(f32, bool)> = (0..g.n() as u32)
        .into_par_iter()
        .map(|c| {
            let (d, p) = (g.d(c), g.d(c) * r);
            let b = theta0 + 0.25 * at(&lobes, p) as f64;
            let t = smooth(b - SCARP, b + SCARP, angle(d, north)) as f32;
            // the highlands rise away from the plains, as the real ones slope north: their water ran to the plains
            let hi = HIGH - 12.0 + 24.0 * smooth(theta0, PI, angle(d, north)) as f32 + 9.0 * at(&rough, p) + 5.0 * at(&region, p);
            let lo = LOW + 1.5 * at(&rough, p);
            (lo + (hi - lo) * t, t < 0.5)
        })
        .collect();
    let low: Vec<bool> = cells.iter().map(|c| c.1).collect();
    let land = Land { h: cells.iter().map(|c| c.0).collect(), kind: low.iter().map(|&l| if l { PLAIN } else { HIGHLAND }).collect(), g, r };
    let age = low.iter().map(|&l| if l { T_PLAINS } else { 0.0 }).collect();
    let stats = Stats { north, ..Stats::default() };
    Work { seed, n, r, rng, land, age, low, north, theta0, craters: Vec::new(), young: Vec::new(), canyon: Vec::new(), tubes: Vec::new(), rivers: Vec::new(), stats, t0 }
}

/// A giant basin: a deep flat floor, steep walls and a broad low rim, older than everything on it.
fn basin(land: &mut Land, c: DVec3, d: f64) {
    // deep for its width on a small planet, but never deeper than the world's columns hold under the highlands
    let (rb, dep, rim) = (0.5 * d / land.r, (0.045 * d).min(BASIN_DEP), 10.0);
    let h0 = land.h[land.g.locate(c) as usize] as f64;
    let mut h = std::mem::take(&mut land.h);
    Work::each(land, c, 1.5 * rb, |cell, a| {
        let x = a / rb;
        let p = if x < 1.0 { -dep * (1.0 - smooth(0.55, 1.0, x)) + rim * smooth(0.8, 1.0, x) } else { rim * (1.0 - smooth(1.0, 1.5, x)) };
        let w = smooth(1.15, 0.85, x);
        h[cell] = (w * (h0 + p) + (1.0 - w) * (h[cell] as f64 + p)) as f32;
    });
    land.h = h;
}

/// The `impacts` stage: two giant basins on the highlands, then Luna's power law of craters, each kept where the
/// ground it fell on is older than it, stamped oldest first; with `volcanism` listed those after `T_VOLC` wait for it.
pub fn impacts(w: &mut Work, volcanism: bool, age: f64) {
    let Work { r, rng, land, age: made, north, theta0, craters, young, stats, .. } = w;
    let r = *r;
    for k in 0..2 {
        let c = loop {
            let c = rng.dir();
            if angle(c, *north) > *theta0 + 0.45 && stats.basins.iter().all(|b| angle(c, b.0) > 1.2) {
                break c;
            }
        };
        let d = r * if k == 0 { rng.range(0.6, 0.72) } else { rng.range(0.38, 0.48) };
        basin(land, c, d);
        stats.basins.push((c, d));
    }
    let count = (luna::K * age * 4.0 * PI * r * r / (D_MIN * D_MIN)) as usize;
    let d_max = D_MAX * r;
    let mut all = Vec::new();
    for _ in 0..count {
        let d = D_MIN / rng.f().max(1e-12).sqrt();
        let (c, t) = (rng.dir(), rng.f());
        if d <= d_max && t as f32 >= made[land.g.locate(c) as usize] {
            all.push(Crater::new(c, d, t, false, false));
        }
    }
    all.sort_by(|a, b| a.t.total_cmp(&b.t));
    let split = if volcanism { all.partition_point(|c| (c.t as f32) < T_VOLC) } else { all.len() };
    *young = all.split_off(split);
    for cr in &mut all {
        land.stamp(cr, &[], false);
    }
    *craters = all;
}

/// The `volcanism` stage: the bulge, its lava plains, the shields and their calderas and tubes, the canyon, then the
/// craters younger than the lava.
pub fn volcanism(w: &mut Work) {
    let Work { seed, r, rng, land, age, north, theta0, craters, young, canyon, tubes, stats, .. } = w;
    let (seed, r, north) = (*seed, *r, *north);
    let (e1, e2) = (north.any_orthonormal_vector(), north.cross(north.any_orthonormal_vector()));
    // the bulge: on the dichotomy's edge, the highland side, the azimuth nearest the equator far from the basins
    let a = *theta0 + 0.15;
    let phi0 = rng.range(0.0, TAU);
    let score = |p: DVec3| p.z.abs() + if stats.basins.iter().all(|b| angle(p, b.0) > 1.3) { 0.0 } else { 1.0 };
    let c = (0..64)
        .map(|k| {
            let phi = phi0 + k as f64 / 64.0 * TAU;
            (north * a.cos() + (e1 * phi.cos() + e2 * phi.sin()) * a.sin()).normalize()
        })
        .min_by(|p, q| score(*p).total_cmp(&score(*q)))
        .unwrap();
    stats.bulge = c;
    let (wrinkle, lobe) = (luna::noise(seed, 0x41, 1.0 / 40.0, 2), luna::noise(seed, 0x42, 1.0 / 500.0, 2));
    let dome = |d: DVec3| {
        let x = angle(d, c) / RB;
        if x < 1.0 { HB * (1.0 - x * x).powi(2) } else { 0.0 }
    };
    // the bulge rises and lava floods its low ground: craters older than it drown, high rims stand out
    let (mut h, mut kind) = (std::mem::take(&mut land.h), std::mem::take(&mut land.kind));
    Work::each(land, c, 1.35 * 1.3 * RB, |cell, ang| {
        let p = land.g.d(cell as u32) * r;
        let up = dome(p.normalize());
        let mut v = h[cell] as f64 + up;
        let xl = ang / (1.35 * RB * (1.0 + 0.2 * at(&lobe, p) as f64));
        if xl < 1.0 {
            let level = up + 44.0 + (-20.0 - 44.0) * smooth(0.6, 1.0, xl);
            if v < level {
                v = level + 0.6 * at(&wrinkle, p) as f64;
                kind[cell] = LAVA;
                age[cell] = T_VOLC;
            }
        }
        h[cell] = v as f32;
    });
    (land.h, land.kind) = (h, kind);

    // the shields: three in a row across the bulge (north-east to south-west), and the giant past its north-west edge
    let (ea, no) = (east(c), northward(c));
    let turn = rng.range(-0.3, 0.3);
    let ne = (no + ea).normalize() * turn.cos() + (no - ea).normalize() * turn.sin();
    let mut plan: Vec<(DVec3, f64, Option<f32>)> = [-0.24, 0.0, 0.24].iter().map(|&s| (toward(c, ne, s + rng.range(-0.03, 0.03)), rng.range(0.13, 0.16), None)).collect();
    let tw = rng.range(-0.25, 0.25);
    let nw = (no - ea).normalize() * tw.cos() + (no + ea).normalize() * tw.sin();
    plan.push((toward(c, nw, RB * rng.range(0.9, 1.05)), 0.3, Some(TOP - rng.range(0.0, 12.0) as f32)));
    for (sc, rad, top) in plan {
        let giant = top.is_some();
        let (e3, n3) = (east(sc), northward(sc));
        let base = (0..16).map(|k| land.h[land.g.locate(toward(sc, e3 * (k as f64 / 16.0 * TAU).cos() + n3 * (k as f64 / 16.0 * TAU).sin(), rad)) as usize]).sum::<f32>() / 16.0;
        let summit = top.unwrap_or(base + rng.range(150.0, 190.0) as f32) as f64;
        let (rc, scarp): (f64, f64) = if giant { (0.13, 35.0) } else { (0.1, 0.0) };
        let hh = (summit - base as f64 - scarp) / (1.0 - rc).powf(1.15);
        let cd = 0.07 * hh;
        // the giant's caldera is two, nested
        let inner = toward(sc, e3 * rng.range(-1.0, 1.0) + n3 * rng.range(-1.0, 1.0), 0.05 * rad);
        let (mut h, mut kind) = (std::mem::take(&mut land.h), std::mem::take(&mut land.kind));
        Work::each(land, sc, rad, |cell, ang| {
            let x = ang / rad;
            if x >= 1.0 {
                return;
            }
            let wall = scarp * (1.0 - smooth(0.9, 0.95, x));
            let mut v = base as f64 + hh * (1.0 - x).powf(1.15) + wall;
            if x < rc {
                v = summit - cd * (1.0 - smooth(0.7 * rc, rc, x));
            }
            if giant {
                let x2 = angle(land.g.d(cell as u32), inner) / (0.06 * rad);
                v = v.min(summit - 1.6 * cd * (1.0 - smooth(0.6, 1.0, x2)));
            }
            if v >= h[cell] as f64 - 1.0 && x < 0.97 {
                kind[cell] = SHIELD;
                age[cell] = T_VOLC;
            }
            h[cell] = h[cell].max(v as f32);
        });
        (land.h, land.kind) = (h, kind);
        stats.shields.push(Shield { c: sc, radius: rad * r, base, summit: summit as f32, caldera: cd as f32, giant });
    }

    // lava tubes down the flanks: balls of air under a roof, pits where it fell in (cut by the cave run)
    for sh in stats.shields.clone() {
        let rad = sh.radius / r;
        for _ in 0..if sh.giant { 5 } else { 2 } {
            let w0 = rng.range(0.0, TAU);
            let out = east(sh.c) * w0.cos() + northward(sh.c) * w0.sin();
            let mut d = toward(sh.c, out, rad * rng.range(0.18, 0.3));
            let mut head = out;
            let rt = rng.range(4.0, 6.5);
            let steps = (0.7 * sh.radius / (1.2 * rt)) as u64;
            let pits = [rng.u64() % steps.max(1), rng.u64() % steps.max(1)];
            let mut any = false;
            for s in 0..steps {
                let cell = land.g.locate(d);
                let (_, i, j) = land.g.fij(cell);
                let edge = (rt / CELL as f64) as usize + 2;
                if land.kind[cell as usize] != SHIELD || i.min(j) < edge || i.max(j) + edge >= land.g.m || angle(d, sh.c) > 0.92 * rad {
                    break;
                }
                let pit = pits.contains(&s);
                let roof = if pit { -1.5 } else { rng.range(5.0, 9.0) };
                tubes.push((cell, (rt + roof) as f32, rt as f32));
                stats.pits += pit as usize;
                any = true;
                // down the flank, wandering
                let radial = (d - sh.c) - d * (d - sh.c).dot(d);
                head = (head - d * head.dot(d)).normalize() * 0.8 + radial.normalize_or(head) * 0.2;
                head = (head - d * head.dot(d)).normalize();
                let tn = rng.range(-0.25, 0.25);
                head = (head * tn.cos() + d.cross(head) * tn.sin()).normalize();
                d = (d + head * (1.2 * rt / r)).normalize();
            }
            stats.tubes += any as usize;
        }
    }
    stats.balls = tubes.len();

    // the canyon: the crust torn as the bulge rose, from its flank along the dichotomy's edge, away from the giant
    let ac = *theta0 + 0.24;
    let az = |d: DVec3| {
        let q = d - north * d.dot(north);
        q.dot(e2).atan2(q.dot(e1))
    };
    let giant = stats.shields.last().unwrap().c;
    let on = |phi: f64, da: f64| (north * (ac + da).cos() + (e1 * phi.cos() + e2 * phi.sin()) * (ac + da).sin()).normalize();
    let sgn = if angle(on(az(c) + 0.3, 0.0), giant) > angle(on(az(c) - 0.3, 0.0), giant) { 1.0 } else { -1.0 };
    let len = rng.range(0.9, 1.05);
    let meander = luna::noise(seed, 0x43, 1.0 / 400.0, 2);
    let steps = (len * r / 2.0) as usize;
    let axis: Vec<DVec3> = (0..=steps)
        .map(|k| {
            let arc = 0.3 + len * k as f64 / steps as f64;
            let p = on(az(c) + sgn * arc / ac.sin(), 0.0);
            on(az(c) + sgn * arc / ac.sin(), 0.04 * at(&meander, p * r) as f64)
        })
        .collect();
    // each cell's nearest point of the troughs, as a share of their half width there, and how far along
    let mut near: HashMap<usize, (f32, f32)> = HashMap::new();
    let mut put = |land: &Land, p: DVec3, s: f64, wm: f64| {
        Work::each(land, p, wm / r, |cell, ang| {
            let q = (ang * r / wm) as f32;
            let e = near.entry(cell).or_insert((f32::INFINITY, 0.0));
            if q < e.0 {
                *e = (q, s as f32);
            }
        })
    };
    for (k, &p) in axis.iter().enumerate() {
        let s = k as f64 / steps as f64;
        let wm = 35.0 + 65.0 * (PI * s).sin().powf(0.8);
        put(land, p, s, wm);
        // a second trough beside the first along its middle, on the highland side
        if (0.35..0.75).contains(&s) {
            put(land, toward(p, -north, 0.045 + 0.02 * (PI * (s - 0.35) / 0.4).sin()), s, 0.55 * wm);
        }
    }
    for (&cell, &(q, s)) in &near {
        if q < 1.0 {
            let floor = 45.0 + (-8.0 - 45.0) * s;
            let v = floor + (land.h[cell] - floor) * smooth(0.55, 1.0, q as f64) as f32;
            land.h[cell] = land.h[cell].min(v);
            if q < 0.75 {
                land.kind[cell] = CANYON;
                age[cell] = T_VOLC;
            }
        }
    }
    stats.canyon_m = len * r;
    stats.canyon = axis.clone();
    *canyon = axis;

    // the craters since
    for cr in young.iter_mut() {
        land.stamp(cr, &[], false);
    }
    craters.append(young);
}

/// The `erosion` stage, long ago: the chaos and outflow channels, the valley networks to the plains' old shore,
/// their lake beds and deltas; dry now, the beds kept.
pub fn erosion(w: &mut Work) {
    let Work { seed, r, rng, land, age, low, north, canyon, rivers, stats, .. } = w;
    let (seed, r, north) = (*seed, *r, *north);
    let n = land.g.n();
    // 1. chaos at the canyon's end, where the ground water burst out, and the channels it cut into the plains
    if let Some(&e) = canyon.last() {
        let knobs = luna::noise(seed, 0x51, 1.0 / 30.0, 2);
        let down = north - e * north.dot(e);
        let side = down.cross(e).normalize();
        let mut near: HashMap<usize, (f32, f32)> = HashMap::new();
        for k in 0..3 {
            let cc = toward(toward(e, side, [0.0, 0.08, -0.08][k]), down, 0.02 * k as f64);
            let (mut h, mut kind) = (std::mem::take(&mut land.h), std::mem::take(&mut land.kind));
            Work::each(land, cc, 0.065, |cell, ang| {
                let x = ang / 0.065;
                let floor = -10.0 + 14.0 * at(&knobs, land.g.d(cell as u32) * r).max(0.0);
                let v = floor + (h[cell] - floor) * smooth(0.7, 1.0, x) as f32;
                if v < h[cell] {
                    h[cell] = v;
                    if x < 0.85 {
                        kind[cell] = CHAOS;
                        age[cell] = T_VOLC;
                    }
                }
            });
            (land.h, land.kind) = (h, kind);
            // the channel: toward the plains, wandering, until well out on them
            let (mut d, mut head) = (cc, (down - cc * down.dot(cc)).normalize());
            head = (head * 1.0 + cc.cross(head) * rng.range(-0.35, 0.35)).normalize();
            let mut path = vec![d];
            let mut out = 0;
            while out < 30 && path.len() < 4000 {
                let tl = north - d * north.dot(d);
                head = (head - d * head.dot(d)).normalize() * 0.97 + tl.normalize_or(head) * 0.03;
                head = (head - d * head.dot(d)).normalize();
                let tn = rng.range(-0.08, 0.08);
                head = (head * tn.cos() + d.cross(head) * tn.sin()).normalize();
                d = (d + head * (2.0 / r)).normalize();
                path.push(d);
                let cell = land.g.locate(d) as usize;
                if low[cell] && land.h[cell] < SHORE {
                    out += 1;
                }
            }
            let wm = rng.range(20.0, 30.0);
            for (i, &p) in path.iter().enumerate() {
                let s = i as f32 / (path.len() - 1) as f32;
                Work::each(land, p, wm / r, |cell, ang| {
                    let q = (ang * r / wm) as f32;
                    let e = near.entry(cell).or_insert((f32::INFINITY, 0.0));
                    if q < e.0 {
                        *e = (q, s);
                    }
                });
            }
            stats.channels.push(path);
        }
        for (&cell, &(q, s)) in &near {
            if q < 1.0 {
                let floor = -10.0 + (LOW - 4.0 + 10.0) * s;
                land.h[cell] = land.h[cell].min(floor + (land.h[cell] - floor) * smooth(0.5, 1.0, q as f64) as f32);
                if q < 0.8 {
                    land.kind[cell] = CHANNEL;
                    age[cell] = T_VOLC;
                }
            }
        }
    }

    // 2. the drainage to the old shore (Terra's: Priority-Flood and D8) on the ground as the water saw it: the
    // regional slope, 8 cells wide, so the valleys run through the craters (and breach their rims) as the real ones
    // did, and only the large hollows held lakes
    let q = (land.g.m / 8).max(4);
    let gc = Grid::new(q, r);
    let mut mean = vec![(0f64, 0u32); gc.n()];
    for c in 0..n {
        let e = &mut mean[gc.locate(land.g.d(c as u32)) as usize];
        *e = (e.0 + land.h[c] as f64, e.1 + 1);
    }
    let coarse: Vec<f32> = mean.iter().map(|e| (e.0 / e.1.max(1) as f64) as f32).collect();
    let ground = land::upsample(&gc, &land.g, &coarse);
    let dr = land::drain(&land.g, &ground, SHORE);
    // a lake: water stood two metres or more over the ground the water saw
    let lake = |c: usize| !dr.ocean[c] && dr.lvl[c] - ground[c] > 2.0;
    let valley: Vec<bool> = (0..n).into_par_iter().map(|c| !dr.ocean[c] && !lake(c) && dr.area[c] >= A_VALLEY && !matches!(land.kind[c], LAVA | SHIELD)).collect();
    // the mouths: a valley cell whose water leaves the network
    let mut beds: Vec<usize> = Vec::new();
    let mut fans: Vec<(usize, f32)> = Vec::new();
    for &c in &dr.order {
        let c = c as usize;
        if !valley[c] {
            continue;
        }
        stats.valleys += 1;
        let to = dr.recv[c] as usize;
        if valley[to] {
            continue;
        }
        let sink = if dr.ocean[to] { if low[to] { 0 } else { 1 } } else if lake(to) { 2 } else { 3 };
        stats.mouths[sink] += 1;
        if sink == 2 {
            beds.push(to);
        }
        if sink < 3 && dr.area[c] >= 4.0 * A_VALLEY {
            fans.push((c, dr.area[c]));
        }
    }
    // the lake beds the valleys fed: the lake's cells, a flat of sediment three metres under its spill
    let mut seen = vec![false; n];
    while let Some(c0) = beds.pop() {
        if seen[c0] {
            continue;
        }
        seen[c0] = true;
        let mut todo = vec![c0];
        while let Some(c) = todo.pop() {
            if matches!(land.kind[c], CANYON | CHAOS | CHANNEL) {
                continue;
            }
            land.h[c] = land.h[c].max(dr.lvl[c] - 3.0);
            land.kind[c] = SEDIMENT;
            for o in land.g.around(c as u32) {
                let o = o as usize;
                if !seen[o] && lake(o) && !valley[o] {
                    seen[o] = true;
                    todo.push(o);
                }
            }
        }
    }
    // the basins' floors under the shore: old sediment too
    for c in 0..n {
        if dr.ocean[c] && !low[c] && land.kind[c] == HIGHLAND {
            land.kind[c] = SEDIMENT;
        }
    }
    // 3. the valleys carved, deeper and wider downstream, the bed falling all the way: from the springs down, a bed
    // never higher than the one above it, so a rim in the way is cut through
    let depth = |a: f32| (1.2 * (a / A_VALLEY).powf(0.3)).min(7.0);
    let mut bed: Vec<f32> = (0..n).map(|c| if valley[c] { land.h[c] - depth(dr.area[c]) } else { f32::INFINITY }).collect();
    for &c in dr.order.iter().rev() {
        let (c, to) = (c as usize, dr.recv[c as usize] as usize);
        if valley[c] && valley[to] {
            bed[to] = bed[to].min(bed[c] - 0.02);
        }
    }
    for c in 0..n {
        if valley[c] && dr.area[c] > 4.0 * A_VALLEY {
            for o in land.g.around(c as u32) {
                let o = o as usize;
                if !valley[o] && !dr.ocean[o] {
                    if land.h[o] > bed[c] + 0.5 {
                        land.h[o] = (land.h[o] - 0.5 * depth(dr.area[c])).max(bed[c] + 0.5);
                    }
                }
            }
        }
    }
    for c in 0..n {
        if valley[c] {
            land.h[c] = bed[c];
        }
    }
    // 4. the deltas: fans where the big valleys met standing water
    for &(c, area) in &fans {
        let hm = land.h[c];
        let rho = (25.0 + 20.0 * (area / (4.0 * A_VALLEY)).powf(0.35)).min(90.0) as f64;
        let (mut h, mut kind) = (std::mem::take(&mut land.h), std::mem::take(&mut land.kind));
        Work::each(land, land.g.d(c as u32), rho / r, |cell, ang| {
            if dr.ocean[cell] || kind[cell] == SEDIMENT {
                let v = hm - 0.15 * (ang * r) as f32;
                if v > h[cell] {
                    h[cell] = v;
                    kind[cell] = DELTA;
                }
            }
        });
        (land.h, land.kind) = (h, kind);
        stats.deltas += 1;
    }
    *rivers = dr.order.iter().filter(|&&c| valley[c as usize]).map(|&c| Reach { cell: c, to: dr.recv[c as usize], area: dr.area[c as usize], flow: 0.0 }).collect();
}

/// The `ice` stage on a planet with air: layered caps over both poles, cut by spiral troughs, and frost on the floors
/// of the craters at high latitudes.
pub fn ice(w: &mut Work) {
    let Work { seed, r, land, age, craters, rivers, .. } = w;
    let (seed, r) = (*seed, *r);
    let lobe = luna::noise(seed, 0x61, 1.0 / 300.0, 2);
    for (pole, rc, hc) in [(DVec3::Z, 0.2, 45.0), (DVec3::NEG_Z, 0.15, 32.0)] {
        let (e1, e2) = (DVec3::X, DVec3::Y * pole.z);
        let (mut h, mut kind) = (std::mem::take(&mut land.h), std::mem::take(&mut land.kind));
        Work::each(land, pole, rc * 1.2, |cell, ang| {
            let d = land.g.d(cell as u32);
            let x = ang / (rc * (1.0 + 0.15 * at(&lobe, d * r) as f64));
            if x >= 1.0 {
                return;
            }
            let mut t = (1.0 - x * x).sqrt() * hc;
            // the layers, as terraces on its slopes
            t += ((t / 3.0).round() * 3.0 - t) * 0.5;
            // the spiral troughs the wind cut
            let u = (d.dot(e2).atan2(d.dot(e1)) / TAU * 3.0 + x * 1.5).rem_euclid(1.0);
            if (0.25..0.95).contains(&x) && u < 0.08 {
                t -= 6.0 * (1.0 - (u / 0.04 - 1.0).abs());
            }
            h[cell] += t.max(1.0) as f32;
            kind[cell] = CAP;
            age[cell] = 1.0;
        });
        (land.h, land.kind) = (h, kind);
    }
    let mut kind = std::mem::take(&mut land.kind);
    for cr in craters.iter().filter(|c| c.c.z.abs() > 0.87 && c.d >= 30.0) {
        Work::each(land, cr.c, 0.3 * cr.d / r, |cell, _| {
            if kind[cell] != CAP {
                kind[cell] = FROST;
            }
        });
    }
    land.kind = kind;
    // the valleys the caps buried are beds no more
    rivers.retain(|v| land.kind[v.cell as usize] != CAP);
}

impl Work {
    /// The map of what the stages made, and what they measured.
    pub fn finish(self) -> (Map, Stats) {
        let Work { seed, n, land, age, low, craters, tubes, rivers, mut stats, t0, .. } = self;
        // which craters can still be seen: youngest first, a crater hides under a younger, wider one over its middle
        // and under ground made after it
        let mut cover = vec![0f32; land.g.n()];
        let mut seen = vec![false; craters.len()];
        let mut order: Vec<usize> = (0..craters.len()).collect();
        order.sort_by(|&a, &b| craters[b].t.total_cmp(&craters[a].t));
        for k in order {
            let cr = &craters[k];
            let cc = land.g.locate(cr.c) as usize;
            seen[k] = (cover[cc] as f64) < cr.d && cr.t as f32 >= age[cc];
            Work::each(&land, cr.c, 0.4 * cr.d / land.r, |cell, _| cover[cell] = cover[cell].max(cr.d as f32));
        }
        stats.craters = craters.iter().zip(&seen).map(|(c, &s)| (c.c, c.d, s)).collect();
        let (mut all, mut lowland) = (0.0, 0.0);
        for c in 0..land.g.n() {
            let (a, k) = (land.g.area(c as u32) as f64, (land.kind[c] - HIGHLAND) as usize);
            stats.area[k] += a;
            stats.mean_h[k] += a * land.h[c] as f64;
            all += a;
            lowland += a * low[c] as u8 as f64;
        }
        for k in 0..KINDS.len() {
            stats.mean_h[k] /= stats.area[k].max(1e-9);
        }
        stats.lowland = lowland / all;
        let spawn = luna::spawn(&land, CELL);
        let mut map = Map { version: VERSION, seed, n, m: land.g.m as u32, h: land.h, biome: land.kind, spawn, rivers, tubes, ..Map::empty() };
        map.finish(&map.to_bytes());
        stats.seconds = t0.elapsed().as_secs_f64();
        eprintln!("[bake] a planet's crust of seed {seed} planet {n}: {:.1} s", stats.seconds);
        (map, stats)
    }
}

/// Where the map of a planet's crust is kept: its row's name, a hash of what its stages read (the list and the
/// age) and the version, so another list, row or bake code is another file and an old one is never read as new.
/// An icy body's map (`bake::icy`) is kept by the same rule under its own version.
pub fn file(stages: &[Stage], seed: i32, n: i32, p: &Planet) -> PathBuf {
    // every input the bake reads: an icy body's ice also reads the sun's height (a planet's tag as before)
    let key = match p.icy {
        Some(high) => format!("{stages:?} {:?} {:016x}", p.age, high.to_bits()),
        None => format!("{stages:?} {:?}", p.age),
    };
    let tag = crate::world::fnv(crate::world::FNV0, key.as_bytes()) as u32;
    let name: String = p.name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    PathBuf::from(super::DIR).join(format!("seed-{seed}-planet-{n}-{name}-{tag:08x}-v{}.map", version(p)))
}

/// The map version a named bake makes: a planet's crust's, or an icy body's (`bake::icy`).
pub fn version(p: &Planet) -> u32 {
    if p.icy.is_some() { super::icy::VERSION } else { VERSION }
}

/// Maps of planets' crusts in memory, by file.
static KEPT: LazyLock<Mutex<HashMap<PathBuf, Weak<Map>>>> = LazyLock::new(Mutex::default);

/// The planet's map of (seed, n): from memory, from its file, else baked now and kept.
pub fn get(stages: &[Stage], seed: i32, n: i32, p: &Planet) -> Arc<Map> {
    let path = file(stages, seed, n, p);
    if let Some(m) = KEPT.lock().unwrap().get(&path).and_then(Weak::upgrade) {
        return m;
    }
    match std::fs::read(&path).ok().and_then(|b| Map::from_bytes(&b).ok()).filter(|m| m.version == version(p) && m.seed == seed && m.n == n) {
        Some(m) => remember(path, m),
        None => match p.icy {
            Some(high) => keep(stages, seed, n, p, super::icy::run(stages, seed, n, high).0),
            None => keep(stages, seed, n, p, run(stages, seed, n, p.age).0),
        },
    }
}

/// Keeps a planet's map in memory and in its file.
pub fn keep(stages: &[Stage], seed: i32, n: i32, p: &Planet, map: Map) -> Arc<Map> {
    let path = file(stages, seed, n, p);
    super::write(&path, &map);
    remember(path, map)
}

/// The kept planet's map (memory or its file, never baked here) of (seed, n) with this hash, of any row that
/// bakes a planet's crust: what `bake::find` gives a client before it asks the server for a map.
pub fn find(seed: i32, n: i32, hash: u64) -> Option<Arc<Map>> {
    planets().find_map(|(s, p)| {
        let path = file(s, seed, n, p);
        let kept = KEPT.lock().unwrap().get(&path).and_then(Weak::upgrade);
        match kept {
            Some(m) => Some(m),
            None => Some(remember(path.clone(), std::fs::read(&path).ok().and_then(|b| Map::from_bytes(&b).ok()).filter(|m| m.version == version(p) && m.seed == seed && m.n == n)?)),
        }
        .filter(|m| m.hash == hash)
    })
}

/// Keeps a planet's (or an icy body's) map that came from a server (`bake::keep`) under its row's name.
/// shortcut: the first row whose bake makes the map's version (one planet's crust, Mars, and one icy body); a second
/// of either needs the map to name its row.
pub fn keep_any(map: Map) -> Arc<Map> {
    let (s, p) = planets().find(|(_, p)| version(p) == map.version).expect("a planet's map, so a row that bakes one");
    keep(s, map.seed, map.n, p, map)
}

/// The rows' stage lists and rows that bake a planet's crust.
fn planets() -> impl Iterator<Item = (&'static [Stage], &'static Planet)> {
    crate::bodies::BODIES.iter().filter_map(|r| r.bake.as_ref().and_then(|b| Some((&b.0[..], b.2.as_ref()?))))
}

fn remember(path: PathBuf, map: Map) -> Arc<Map> {
    let m = Arc::new(map);
    KEPT.lock().unwrap().insert(path, Arc::downgrade(&m));
    m
}

impl Stats {
    /// One JSON line of the numbers: the kinds' shares and heights, craters by ground, the shields, canyon,
    /// channels, valleys and tubes, and the stages' seconds.
    pub fn json(&self, map: &Map) -> String {
        let all: f64 = self.area.iter().sum();
        let kinds: Vec<String> = KINDS.iter().enumerate().map(|(k, n)| format!("\"{n}\": [{:.4}, {:.1}]", self.area[k] / all, self.mean_h[k])).collect();
        let dens = density(self, map, 24.0);
        let dens: Vec<String> = KINDS.iter().zip(dens).filter(|d| d.1 > 0.0).map(|(n, d)| format!("\"{n}\": {d:.1}")).collect();
        let shields: Vec<String> = self.shields.iter().map(|s| format!("{{\"radius\": {:.0}, \"base\": {:.0}, \"summit\": {:.0}, \"caldera\": {:.1}, \"giant\": {}}}", s.radius, s.base, s.summit, s.caldera, s.giant)).collect();
        let stages: Vec<String> = self.stages.iter().map(|(n, s)| format!("\"{n}\": {s:.2}")).collect();
        let chan: Vec<String> = self.channels.iter().map(|c| format!("{:.0}", 2.0 * (c.len() - 1) as f64)).collect();
        format!(
            "{{\"seed\": {}, \"n\": {}, \"cells\": {}, \"seconds\": {:.2}, \"stages\": {{{}}}, \"bytes\": {}, \"hash\": \"{:016x}\", \"lowland\": {:.3}, \"kinds_share_mean_h\": {{{}}}, \"craters\": {}, \"seen\": {}, \"seen_per_km2_over_24m\": {{{}}}, \"basins_m\": {:?}, \"shields\": [{}], \"canyon_m\": {:.0}, \"channels_m\": [{}], \"valley_cells\": {}, \"mouths_plain_basin_lake_other\": {:?}, \"deltas\": {}, \"tubes\": {}, \"balls\": {}, \"pits\": {}, \"min_h\": {:.1}, \"max_h\": {:.1}}}",
            map.seed, map.n, map.h.len(), self.seconds, stages.join(", "), map.to_bytes().len(), map.hash, self.lowland, kinds.join(", "), self.craters.len(), self.craters.iter().filter(|c| c.2).count(), dens.join(", "),
            self.basins.iter().map(|b| b.1.round()).collect::<Vec<_>>(), shields.join(", "), self.canyon_m, chan.join(", "), self.valleys, self.mouths, self.deltas, self.tubes, self.balls, self.pits,
            map.h.iter().fold(f32::MAX, |a, &b| a.min(b)), map.h.iter().fold(f32::MIN, |a, &b| a.max(b))
        )
    }
}

/// Craters seen at least `d` m wide per km^2, by the kind of ground their middle lies on.
pub fn density(s: &Stats, map: &Map, d: f64) -> [f64; 11] {
    let g = Grid::bare(map.m as usize, 1.0);
    let mut k = [0f64; 11];
    for c in s.craters.iter().filter(|c| c.2 && c.1 >= d) {
        let (f, x, y) = g.locate_xy(c.0);
        k[(map.biome(f, x, y) - HIGHLAND) as usize] += 1.0;
    }
    std::array::from_fn(|i| if s.area[i] > 0.0 { k[i] / (s.area[i] / 1e6) } else { 0.0 })
}

pub use super::pictures::mars as pictures;

#[cfg(test)]
mod tests {
    use super::*;

    const MARS: [Stage; 5] = [Stage::Crust, Stage::Impacts, Stage::Volcanism, Stage::Erosion, Stage::Ice];

    /// One bake of (seed, n) for the tests that only read it.
    fn shared(seed: i32, n: i32) -> Arc<(Map, Stats)> {
        static MAPS: LazyLock<Mutex<HashMap<(i32, i32), Arc<std::sync::OnceLock<Arc<(Map, Stats)>>>>>> = LazyLock::new(Mutex::default);
        let cell = MAPS.lock().unwrap().entry((seed, n)).or_default().clone();
        cell.get_or_init(|| Arc::new(run(&MARS, seed, n, 1.0))).clone()
    }

    /// The heights stay within the world's columns at every size of planet: what scales with the body (the basins,
    /// the craters) is held to the world's range, not left to grow with n.
    #[test]
    fn heights_stay_in_the_worlds_columns_at_every_size() {
        for n in [832, 6144] {
            for seed in [1, 2] {
                let m = &shared(seed, n).0;
                let (lo, hi) = m.h.iter().fold((f32::MAX, f32::MIN), |(a, b), &h| (a.min(h), b.max(h)));
                assert!(lo >= -56.0 && hi < 378.0, "seed {seed} n {n}: heights {lo:.1} to {hi:.1}");
            }
        }
    }

    fn planet() -> Planet {
        Planet { name: "Mars".into(), age: 1.0, icy: None }
    }

    /// The bake is the same twice, reads back from its bytes, and is kept under its own name: read back from that
    /// file (a valid map put there is what comes back, so it was read, not baked), baked again when the file is no
    /// map; another list or age is another file, and Terra's and Luna's names are not touched.
    #[test]
    fn a_planets_bake_is_the_same_twice_and_round_trips_through_its_file() {
        let (seed, n) = (-4242, 832);
        let (a, b) = (run(&MARS, seed, n, 1.0).0, run(&MARS, seed, n, 1.0).0);
        assert_eq!(a.hash, b.hash, "deterministic");
        let back = Map::from_bytes(&a.to_bytes()).unwrap();
        assert_eq!((back.hash, back.version, &back.tubes, back.rivers.len(), &back.biome), (a.hash, VERSION, &a.tubes, a.rivers.len(), &a.biome));
        assert!(!a.tubes.is_empty() && !a.rivers.is_empty());
        let mut odd = Map::from_bytes(&a.to_bytes()).unwrap();
        odd.biome[0] = HIGHLAND - 1;
        assert!(Map::from_bytes(&odd.to_bytes()).is_err(), "a kind outside a planet's crust's is refused");
        let path = file(&MARS, seed, n, &planet());
        assert_eq!(path.file_name().unwrap().to_str().unwrap(), format!("seed--4242-planet-832-mars-{:08x}-v201.map", crate::world::fnv(crate::world::FNV0, b"[Crust, Impacts, Volcanism, Erosion, Ice] 1.0") as u32));
        assert_ne!(path, file(&MARS[..4], seed, n, &planet()), "another list, another file");
        assert_ne!(path, file(&MARS, seed, n, &Planet { age: 0.5, ..planet() }), "another age, another file");
        assert!(![super::super::file(seed, n), luna::file(seed, n)].contains(&path));
        let _ = std::fs::remove_file(&path);
        let got = get(&MARS, seed, n, &planet());
        assert_eq!(got.hash, a.hash, "baked and kept");
        assert!(path.exists());
        drop(got);
        // a valid map of this version in the file: it is what comes back
        let other = Map { spawn: [1.0, 0.0, 0.0], ..Map::from_bytes(&a.to_bytes()).unwrap() };
        let other = Map::from_bytes(&other.to_bytes()).unwrap();
        assert_ne!(other.hash, a.hash);
        std::fs::write(&path, other.to_bytes()).unwrap();
        assert_eq!(get(&MARS, seed, n, &planet()).hash, other.hash, "read back from its file, not baked");
        // and no map there: baked again
        std::fs::write(&path, b"no map").unwrap();
        assert_eq!(get(&MARS, seed, n, &planet()).hash, a.hash, "baked again");
        let _ = std::fs::remove_file(&path);
    }

    /// The map has what the row calls for, on two seeds (the seed moves the detail, the row makes the planet):
    /// the dichotomy, craters by age, the shields and their calderas, the canyon, the chaos and channels, valleys
    /// that run downhill into the plains and basins, caps at both poles, tubes on the shields; and nothing placed
    /// for where a player lands.
    #[test]
    fn the_map_has_what_the_row_calls_for() {
        let mut bulges = Vec::new();
        for seed in [1, 2] {
            let st = shared(seed, 1664);
            let (m, s) = (&st.0, &st.1);
            let why = |w: &str| format!("seed {seed}: {w}");
            let k = |kind: u8| (kind - HIGHLAND) as usize;
            let all: f64 = s.area.iter().sum();
            for (i, a) in s.area.iter().enumerate() {
                assert!(*a > 0.0, "{}", why(&format!("no {}", KINDS[i])));
            }
            assert!(m.h.iter().all(|h| (-56.0..378.0).contains(h)), "{}", why("within the world's columns"));
            // the dichotomy: low plains about a third, the highlands far over them
            assert!((0.3..0.46).contains(&s.lowland), "{}", why(&format!("lowland {:.3}", s.lowland)));
            assert!(s.mean_h[k(HIGHLAND)] - s.mean_h[k(PLAIN)] > 50.0, "{}", why(&format!("highland {:.0} m, plains {:.0} m", s.mean_h[k(HIGHLAND)], s.mean_h[k(PLAIN)])));
            assert!(s.area[k(HIGHLAND)] / all > 0.3, "{}", why("highland share"));
            // craters by age: the old highlands many times the plains and the volcanic ground
            let dens = density(s, m, 24.0);
            assert!(dens[k(HIGHLAND)] > 3.0 * dens[k(PLAIN)] && dens[k(HIGHLAND)] > 3.0 * dens[k(LAVA)], "{}", why(&format!("craters per km2 {dens:?}")));
            assert_eq!(s.basins.len(), 2);
            // the shields: three and a giant, each with a caldera in its summit, the giant far higher
            assert_eq!((s.shields.len(), s.shields.iter().filter(|v| v.giant).count()), (4, 1));
            let g = Grid::new(m.m as usize, m.radius());
            for v in &s.shields {
                let (mut top, mut mid) = (f32::MIN, m.h[g.locate(v.c) as usize]);
                for c in 0..g.n() as u32 {
                    let a = angle(g.d(c), v.c) * m.radius();
                    if a < 0.3 * v.radius {
                        top = top.max(m.h[c as usize]);
                    }
                    if a < 0.04 * v.radius {
                        mid = mid.min(m.h[c as usize]);
                    }
                }
                assert!(top - mid > 0.5 * v.caldera && v.caldera > 5.0, "{}", why(&format!("caldera {:.1} m deep, rim {top:.0}, floor {mid:.0}", v.caldera)));
                assert_eq!(m.biome[g.locate(v.c) as usize], SHIELD);
            }
            let giant = s.shields.iter().find(|v| v.giant).unwrap();
            assert!(giant.summit - s.mean_h[k(HIGHLAND)] as f32 > 250.0, "{}", why(&format!("the giant's summit {:.0} m", giant.summit)));
            for v in s.shields.iter().filter(|v| !v.giant) {
                assert!(giant.summit - giant.base > 1.5 * (v.summit - v.base) && giant.radius > 1.5 * v.radius, "{}", why("the giant far larger"));
            }
            // the canyon, a stated length, and the channels out of the chaos onto the plains
            assert!((0.85..1.1).contains(&(s.canyon_m / m.radius())), "{}", why(&format!("canyon {:.0} m", s.canyon_m)));
            let floor = s.canyon.iter().filter(|&&p| m.biome[g.locate(p) as usize] == CANYON).count();
            assert!(floor > s.canyon.len() * 8 / 10, "{}", why(&format!("canyon floor at {floor} of {} axis points", s.canyon.len())));
            let onto = s.channels.iter().filter(|c| m.h[g.locate(*c.last().unwrap()) as usize] < SHORE).count();
            assert!(s.channels.len() == 3 && onto == 3, "{}", why(&format!("{onto} of {} channels onto the plains", s.channels.len())));
            // the valleys: down the whole way, and into the plains or a basin or lake bed
            let reach: HashMap<u32, f32> = m.rivers.iter().map(|r| (r.cell, m.h[r.cell as usize])).collect();
            let up = m.rivers.iter().filter(|r| reach.get(&r.to).is_some_and(|&h| h >= m.h[r.cell as usize])).count();
            assert_eq!(up, 0, "{}", why("a valley running uphill"));
            let into: usize = s.mouths[..3].iter().sum();
            assert!(m.rivers.len() > 500 && into * 10 >= (into + s.mouths[3]) * 8, "{}", why(&format!("{} valley cells, mouths {:?}", m.rivers.len(), s.mouths)));
            assert!(s.deltas > 0, "{}", why("no delta"));
            // the caps at both poles
            for z in [1.0, -1.0] {
                let near: Vec<u32> = (0..g.n() as u32).filter(|&c| g.d(c).z * z > 0.995).collect();
                let ice = near.iter().filter(|&&c| m.biome[c as usize] == CAP).count();
                assert!(ice * 100 >= near.len() * 95, "{}", why(&format!("cap at z {z}: {ice} of {}", near.len())));
            }
            // the tubes on the shields
            assert!(s.tubes >= 6 && s.pits >= 6, "{}", why(&format!("{} tubes, {} pits", s.tubes, s.pits)));
            assert!(m.tubes.iter().all(|t| m.biome[t.0 as usize] == SHIELD), "{}", why("a tube off the shields"));
            bulges.push((s.bulge, giant.c, DVec3::from_array(m.spawn)));
        }
        // the seed moves the planet's features, and with them nothing follows the landing point
        let ((b1, g1, s1), (b2, g2, s2)) = (bulges[0], bulges[1]);
        assert!(angle(b1, b2) > 0.2, "the bulge moves with the seed");
        assert!((angle(g1, s1) - angle(g2, s2)).abs() > 0.05, "the giant is not placed by the landing point");
    }

    /// Mars's map bytes as `bakemap=` made them before Mars was a body (seed 1 at n 832 and its full 3328).
    #[test]
    fn the_map_is_as_it_was_baked() {
        for (n, want) in [(832, 0x552a1c34492c457fu64), (3328, 0x8703842819e918b5)] {
            let m = &shared(1, n).0;
            assert_eq!(m.hash, want, "n {n}: {:016x}", m.hash);
        }
    }

    /// A world of generation 5 with Mars on its map at n 832 (Terra on the noise land, Luna small).
    fn world() -> crate::world::World {
        let rows = crate::bodies::of(5);
        let list = vec![(1536, None), (32, None), (832, Some(Arc::new(Map::from_bytes(&shared(1, 832).0.to_bytes()).unwrap())))];
        crate::world::World::of(rows, list, 1, 0.0, 5)
    }

    /// Every one of Mars's kinds has its own ground from `data/biomes.toml` (planet-model.md step 5b): its top,
    /// the band under it and its rock below, generated so in its columns; no Terra or Luna biome, no water, plant,
    /// cave or ore on it.
    #[test]
    fn each_kind_of_mars_generates_its_ground_and_nothing_of_terra_or_luna() {
        use crate::biomes::ROWS;
        use crate::world::Block;
        let w = world();
        let b = &w.bodies[2];
        assert_eq!((b.name.as_str(), b.map().map(|m| m.version), b.sea, b.caves.len(), b.ores.len()), ("Mars", Some(VERSION), None, 0, 0));
        let (mut seen, mut tops) = ([0usize; 11], [false; 11]);
        let n = 832;
        for c in (0..6 * n * n).step_by(53) {
            let (f, i, j) = ((c / (n * n)) as u8, (c % n) as i32, ((c / n) % n) as i32);
            let (ci, cj) = (i as f64 + 0.5, j as f64 + 0.5);
            let kind = b.biome_at(f as usize, ci, cj, b.dir(f as usize, ci, cj)).unwrap();
            assert!((HIGHLAND..=FROST).contains(&kind), "column {c}: kind {kind}");
            seen[(kind - HIGHLAND) as usize] += 1;
            assert!(b.water_at(f, i, j).is_none() && b.col_plants(f, i, j).is_empty(), "column {c}: water or a plant");
            let ground = ROWS.biomes[kind as usize].ground;
            let top = w.column_height(Block { body: 2, face: f, pos: glam::IVec3::new(i, j, 0) });
            // every layer from 12 m down to over the ground is its kind's stack (`world::material` of the depth of
            // the layer's middle; on a small body's thicker layers the top block's metre can fall between two)
            let z0 = b.layer_of(top).floor() as i32;
            for z in b.layer_of(top - 12.0).floor() as i32..=z0 + 1 {
                let want = crate::world::material(top - b.layer_r(z as f64 + 0.5), &ground);
                assert_eq!(w.generated_block(Block { body: 2, face: f, pos: glam::IVec3::new(i, j, z) }), want, "column {c} ({}), layer {z} of {z0}", KINDS[(kind - HIGHLAND) as usize]);
                tops[(kind - HIGHLAND) as usize] |= want == ground[0];
            }
        }
        assert!(seen.iter().all(|&k| k > 0), "every kind seen: {seen:?}");
        assert!(tops.iter().all(|&t| t), "every kind's top block generated somewhere: {tops:?}");
    }

    /// Mars pulls with its own interior: not Terra's nor Luna's, and with no field to speak of (no aurora).
    #[test]
    fn mars_pulls_as_its_own_and_has_no_field() {
        let w = world();
        let (t, l, m) = (w.surface_g(0), w.surface_g(1), w.surface_g(2));
        assert!(m < t && m > 0.2 * t && m != l, "Terra {t}, Luna {l}, Mars {m}");
        let p = w.bodies[2].centre + DVec3::Y * (w.bodies[2].radius + 1.0);
        assert!((w.gravity(p).length() / m - 1.0).abs() < 0.01, "{} against {m}", w.gravity(p).length());
        assert!(w.field(2) < 0.02, "field {}", w.field(2));
        println!("Mars: g {m:.3} m/s2 (Terra {t:.3}, Luna {l:.3}), field {:.4}", w.field(2));
    }

    /// Places to stand on seed 1's full-size Mars, for pictures (`run=stand body=2 at=`): `cargo test -p engine --lib
    /// mars_places -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn mars_places() {
        let st = shared(1, 3328);
        let (m, s) = (&st.0, &st.1);
        let g = Grid::new(m.m as usize, m.radius());
        let p = |w: &str, d: DVec3| println!("PLACE {w} at={:.5},{:.5},{:.5}", d.x, d.y, d.z);
        let giant = s.shields.iter().find(|v| v.giant).unwrap();
        p("giant", giant.c);
        // halfway down the giant's flank, toward the bulge
        let side = (s.bulge - giant.c * giant.c.dot(s.bulge)).normalize();
        let a = 0.5 * giant.radius / m.radius();
        p("flank", (giant.c * a.cos() + side * a.sin()).normalize());
        let mid = s.canyon[s.canyon.len() / 2];
        p("canyon", mid);
        // the rim: the first highland or lava cell going out from the axis
        let along = (s.canyon[s.canyon.len() / 2 + 1] - mid).normalize();
        let across = mid.cross(along).normalize();
        for k in 1..400 {
            let d = (mid + across * (k as f64 * 2.0 / m.radius())).normalize();
            if m.biome[g.locate(d) as usize] != CANYON {
                p("rim", (mid + across * ((k + 6) as f64 * 2.0 / m.radius())).normalize());
                println!("PLACE rim_look {:.5},{:.5},{:.5}", -across.x, -across.y, -across.z);
                break;
            }
        }
        p("north cap", DVec3::new(0.05, 0.0, 1.0).normalize());
        for (w, kind) in [("plain", PLAIN), ("sediment", SEDIMENT), ("delta", DELTA), ("lava", LAVA)] {
            let c = (0..g.n() as u32).filter(|&c| m.biome[c as usize] == kind && g.d(c).z.abs() < 0.5).nth(2000).or_else(|| (0..g.n() as u32).find(|&c| m.biome[c as usize] == kind));
            if let Some(c) = c {
                p(w, g.d(c));
            }
        }
        if let Some(c) = s.craters.iter().filter(|c| c.2 && (80.0..160.0).contains(&c.1) && m.biome[g.locate(c.0) as usize] == HIGHLAND).nth(5) {
            p(&format!("crater d {:.0}", c.1), c.0);
        }
    }

    /// Mars's generated blocks at generation 5, locked (as `luna_blocks_hash` locks Luna's).
    #[test]
    fn mars_generates_as_locked() {
        assert_eq!(format!("{:016x}", blocks_hash(&world())), "3b6b6a314af6caa5");
    }

    /// Step 6b: Mars loaded on approach is a stub until it is built (no map, heights, caves or ores), and then
    /// generates bit for bit as one built at the start: the lock above, its ores and ledger, its spawn; out again and
    /// in again the same.
    #[test]
    fn mars_loaded_late_is_mars_from_the_start() {
        use crate::world::Source;
        let rows = crate::bodies::of(5);
        let map = || Arc::new(Map::from_bytes(&shared(1, 832).0.to_bytes()).unwrap());
        let list = vec![(1536, Source::Now(None)), (32, Source::Now(None)), (832, Source::Later(map().hash))];
        let mut w = crate::world::World::of_sources(rows, list, 1, 0.0, 5);
        let eager = world();
        w.open_ledger();
        let (stub, all) = (w.ore_left.clone().unwrap(), { let mut e = world(); e.open_ledger(); e.ore_left.unwrap() });
        assert!(!w.loaded(2) && w.loaded(0) && w.bodies[2].map().is_none() && w.bodies[2].ores.is_empty(), "a stub");
        assert_eq!(w.map_hash(2), eager.map_hash(2), "the stub says its map");
        for _ in 0..2 {
            // the real path: the loader finds the map of the hash (`for_hash`: kept, on disk or baked) on its own
            let b = w.loader(2).expect("a stub has a loader")().expect("this build makes this map");
            w.put(2, b);
            assert!(w.loaded(2) && w.builder(2).is_none());
            assert_eq!(blocks_hash(&w), blocks_hash(&eager), "generated as at the start");
            let (a, e) = (&w.bodies[2], &eager.bodies[2]);
            assert_eq!((a.ores.len(), a.caves.len(), a.spawn, a.sea, a.layers), (e.ores.len(), e.caves.len(), e.spawn, e.sea, e.layers));
            assert_eq!(w.ore_left.as_ref().unwrap(), &all, "the ledger as opened with Mars in (stub's {} bodies)", stub.len());
            // (the ledger's world keeps what it loaded: out again only on a client's)
            w.ore_left = None;
            w.unload(2);
            assert!(!w.loaded(2) && w.bodies[2].map().is_none());
            w.ore_left = Some(stub.clone());
        }
    }

    /// Step 6b: from Terra, Mars (a stub) is neither streamed nor asked for: no slot, no map, no tables. A camera
    /// near enough asks for it; once its ground is put in, its chunks stream until it is drawn with no hole (the
    /// sky's point held till then); the camera back on Terra drops every chunk of it and their room.
    #[test]
    fn mars_streams_only_once_it_is_near() {
        use crate::stream::{FrameStats, Opts, Streamer, View};
        use crate::world::Source;
        let rows = crate::bodies::of(5);
        let map = Arc::new(Map::from_bytes(&shared(1, 832).0.to_bytes()).unwrap());
        let list = vec![(1536, Source::Now(None)), (32, Source::Now(None)), (832, Source::Later(map.hash))];
        let mut w = crate::world::World::of_sources(rows, list, 1, 0.0, 5);
        let opts = Opts { k: 4.0, greedy: false, cutout: false, horizon: true, prefetch_s: 0.0, max_inflight: 64 };
        let mut s = Streamer::new(&w, 1 << 22, opts);
        // (a coarse pixel, so the distances below stay in Mars's frame: 1.5 pixels is some 15 km out)
        let px = 0.05;
        let planes = [glam::Vec4::new(0.0, 0.0, 0.0, 1.0); 6];
        let frames = |s: &mut Streamer, w: &crate::world::World, cam: DVec3, n: usize| {
            let view = View::new(cam, planes, w);
            let mut want = Vec::new();
            for _ in 0..n {
                let (mut out, mut reqs, mut st) = (Vec::new(), Vec::new(), FrameStats::default());
                s.apply(usize::MAX, &mut st, |_, _| {});
                s.frame += 1;
                want = s.seen(w, &view, px);
                s.select(w, &view, true, 1.0, &mut out, &mut reqs, &mut st);
                s.dispatch(w, &mut reqs);
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            want
        };
        let terra = w.bodies[0].spawn_point() + w.bodies[0].spawn * 2.0;
        assert_eq!(frames(&mut s, &w, terra, 50), Vec::<usize>::new(), "Mars is not asked for from Terra");
        assert!(s.slots_of(0) > 0 && s.slots_of(2) == 0 && !s.ready(2), "Terra streams, Mars does not");
        // 1.5 pixels across: asked for, not streamed before it is in
        let (mc, mr) = (w.bodies[2].centre, w.bodies[2].radius);
        let near = mc + DVec3::Y * (2.0 * mr / (1.5 * px));
        assert_eq!(frames(&mut s, &w, near, 3), vec![2], "near enough, Mars is asked for");
        assert_eq!(s.slots_of(2), 0);
        let b = w.builder(2).unwrap()(map.clone());
        w.put(2, b);
        let (mut n, t0) = (0, std::time::Instant::now());
        while !s.ready(2) {
            assert!(t0.elapsed().as_secs() < 600, "Mars never drawn whole (its jobs wait behind the bakes of the tests beside it: some minutes)");
            frames(&mut s, &w, near, 1);
            n += 1;
        }
        let held = s.slots_of(2);
        assert!(held > 0, "its chunks are in");
        // between the two thresholds it stays (0.75 px), under OUT_PX it goes
        let mid = mc + DVec3::Y * (2.0 * mr / (0.75 * px));
        assert!(frames(&mut s, &w, mid, 5).is_empty() && s.slots_of(2) > 0, "held between the thresholds");
        let released = s.released.len();
        frames(&mut s, &w, terra, 5);
        assert!(s.slots_of(2) == 0 && !s.ready(2), "dropped back on Terra: {} slots", s.slots_of(2));
        assert!(s.released.len() > released, "its room given back");
        println!("Mars: {held} slots after {n} frames, all gone on Terra");
    }

    /// Step 6b: a player near Mars before its ground is in is held where it is (not dropped through it): a pause,
    /// not a brake. It keeps the speed it had and may look round; once Mars is in it goes on as a player that was
    /// never held would have from there (no free stop, no fall from rest); one put below the ground (a teleport)
    /// is set on it.
    #[test]
    fn a_player_near_mars_is_held_until_its_ground_is_in() {
        use crate::phys::{self, Input, Player};
        use crate::world::Source;
        let rows = crate::bodies::of(5);
        let map = Arc::new(Map::from_bytes(&shared(1, 832).0.to_bytes()).unwrap());
        let list = vec![(1536, Source::Now(None)), (32, Source::Now(None)), (832, Source::Later(map.hash))];
        let mut w = crate::world::World::of_sources(rows, list, 1, 0.0, 5);
        let m = &w.bodies[2];
        let start = m.centre + DVec3::Y * (m.top(DVec3::Y) + 30.0);
        let below = m.centre + DVec3::Y * (m.top(DVec3::Y) - 5.0);
        let mut p = Player::at(start, DVec3::X);
        let mut deep = Player::at(below, DVec3::X);
        p.vel = DVec3::Y * -20.0;
        let look = Input { fwd: [0.0, 0.0, 1.0], ..Input::default() };
        for _ in 0..60 {
            phys::step(&w, &mut p, &look, phys::DT);
            phys::step(&w, &mut deep, &Input::default(), phys::DT);
        }
        assert!(p.pos == start && p.vel == DVec3::Y * -20.0, "paused: {} m off, {:?} m/s", p.pos.distance(start), p.vel);
        assert!(p.fwd.dot(DVec3::Z) > 0.99, "it looked round while held: {:?}", p.fwd);
        let b = w.builder(2).unwrap()(map);
        w.put(2, b);
        phys::onto_ground(&w, &mut deep);
        assert!(!phys::inside(&w, &deep) && (deep.pos - w.bodies[2].centre).length() >= w.bodies[2].top(DVec3::Y), "set on the ground");
        p.fwd = DVec3::X;
        for _ in 0..600 {
            phys::step(&w, &mut p, &Input::default(), phys::DT);
        }
        // the same fall on a Mars built at the start, step for step
        let eager = world();
        let mut q = Player::at(start, DVec3::X);
        q.vel = DVec3::Y * -20.0;
        for _ in 0..600 {
            phys::step(&eager, &mut q, &Input::default(), phys::DT);
        }
        let m = &w.bodies[2];
        let over = (p.pos - m.centre).length() - m.top((p.pos - m.centre).normalize());
        println!("landed {over:.2} m from the top, on ground {}, inside {}", p.on_ground, phys::inside(&w, &p));
        assert!(p.on_ground && p.pos == q.pos && p.vel == q.vel, "as on Mars from the start: {} m apart", p.pos.distance(q.pos));
    }

    fn blocks_hash(w: &crate::world::World) -> u64 {
        use crate::world::Block;
        let b = &w.bodies[2];
        let n = 832;
        let mut h = crate::world::FNV0;
        for c in (0..6 * n * n).step_by(997) {
            let (f, i, j) = ((c / (n * n)) as u8, (c % n) as i32, ((c / n) % n) as i32);
            let top = w.column_height(Block { body: 2, face: f, pos: glam::IVec3::new(i, j, 0) });
            for z in b.layer_of(top - 70.0).floor() as i32..=b.layer_of(top + 1.0).ceil() as i32 {
                h = crate::world::fnv(h, &w.generated_block(Block { body: 2, face: f, pos: glam::IVec3::new(i, j, z) }).to_le_bytes());
            }
        }
        h
    }
}
