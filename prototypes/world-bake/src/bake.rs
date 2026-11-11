//! Stages 1 to 4 of `docs/design/worldgen.md`: plates, their boundaries and uplift, uplift against
//! stream-power erosion, sea level, the river network and lakes. Deterministic from the seed: every
//! sum and every queue runs in a fixed order; rayon only maps cells to values one by one.
use crate::grid::{Grid, NONE};
use fastnoise_lite::{FastNoiseLite, FractalType, NoiseType};
use glam::DVec3;
use rayon::prelude::*;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::time::Instant;

/// Everything the bake can be tuned by, in one place. The first three are the developer's taste calls
/// (worldgen.md, "Taste calls still open", 1 and 4; the seed is picked from a sheet): change them here or
/// on the command line, nowhere else.
#[derive(Clone, Debug)]
pub struct Params {
    /// TASTE CALL: the highest land over the sea, metres (ADR 0021 today: 50; worldgen proposal: 120).
    pub mountain_m: f32,
    /// TASTE CALL: the deepest sea under it, metres (ADR 0021 today: 25; worldgen proposal: 60).
    pub sea_depth_m: f32,
    /// The share of the surface under the sea (ADR 0021: 55 %).
    pub ocean_share: f32,
    /// Map cell edge at the fine level, metres (worldgen.md: 8 m).
    pub cell_m: f64,
    /// The coarse level's cells are this many fine cells across (8 m * 4 = 32 m, worldgen.md).
    pub coarse: usize,
    pub coarse_iters: usize,
    pub fine_iters: usize,
    /// Stream power: dh/dt = U - K A^m S (n = 1), A in m^2.
    pub k: f32,
    pub m_exp: f32,
    /// Hillslope diffusion, m^2 per step.
    pub kappa: f32,
    /// Peak uplift, metres per step (the time unit is a step; the final heights are rescaled).
    pub uplift: f32,
    /// A cell is a river when it drains this much, m^2.
    pub river_area_m2: f32,
    pub plates: (u32, u32),
    /// The share of crust that is continental (a little over the land share: the lowest of it is shelf).
    pub continental: f32,
    /// Feature widths as shares of the radius: they scale with the planet (ADR 0021: continents do, hills
    /// stay in metres).
    pub collision_w: f64,
    pub arc_w: f64,
    pub rift_w: f64,
}

impl Default for Params {
    fn default() -> Params {
        Params {
            mountain_m: 50.0,
            sea_depth_m: 25.0,
            ocean_share: 0.55,
            cell_m: 8.0,
            coarse: 4,
            coarse_iters: 160,
            fine_iters: 24,
            k: 0.5,
            m_exp: 0.5,
            kappa: 1.5,
            uplift: 1.0,
            river_area_m2: 1.5e5,
            plates: (8, 14),
            continental: 0.48,
            collision_w: 0.05,
            arc_w: 0.035,
            rift_w: 0.012,
        }
    }
}

/// splitmix64
pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    pub fn u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    pub fn f(&mut self) -> f64 {
        (self.u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    pub fn range(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.f()
    }
    pub fn dir(&mut self) -> DVec3 {
        loop {
            let v = DVec3::new(self.range(-1., 1.), self.range(-1., 1.), self.range(-1., 1.));
            if (0.01..1.0).contains(&v.length_squared()) {
                return v.normalize();
            }
        }
    }
}

fn noise(seed: i32, octaves: i32) -> FastNoiseLite {
    let mut n = FastNoiseLite::with_seed(seed);
    n.set_noise_type(Some(NoiseType::OpenSimplex2));
    n.set_fractal_type(Some(FractalType::FBm));
    n.set_fractal_octaves(Some(octaves));
    n.set_frequency(Some(1.0));
    n
}
fn at(n: &FastNoiseLite, d: DVec3, freq: f64) -> f32 {
    let p = d * freq;
    n.get_noise_3d(p.x as f32, p.y as f32, p.z as f32)
}

/// f32 to a u32 that sorts the same way (negatives too)
fn key(v: f32) -> u32 {
    let b = v.to_bits();
    if b >> 31 == 1 {
        !b
    } else {
        b | 0x8000_0000
    }
}

/// Multi-source Dijkstra over the cell graph: per cell the cost to its nearest source and that source's
/// index. `step(from, to, metres, source)` is the cost of a step; costs past `limit` are not followed.
fn dijkstra(g: &Grid, sources: &[(u32, f64)], limit: f64, step: impl Fn(u32, u32, f64, u32) -> f64) -> (Vec<f32>, Vec<u32>) {
    let mut cost = vec![f64::INFINITY; g.n()];
    let mut src = vec![NONE; g.n()];
    let mut q = BinaryHeap::new();
    for (s, &(c, c0)) in sources.iter().enumerate() {
        if c0 < cost[c as usize] {
            cost[c as usize] = c0;
            src[c as usize] = s as u32;
            q.push(Reverse((c0.to_bits(), c)));
        }
    }
    while let Some(Reverse((k, c))) = q.pop() {
        let cc = f64::from_bits(k);
        if cc > cost[c as usize] {
            continue;
        }
        for o in g.around(c) {
            let nc = cc + step(c, o, g.dist(c, o) as f64, src[c as usize]);
            if nc < cost[o as usize] && nc <= limit {
                cost[o as usize] = nc;
                src[o as usize] = src[c as usize];
                q.push(Reverse((nc.to_bits(), o)));
            }
        }
    }
    (cost.iter().map(|&c| c as f32).collect(), src)
}

pub struct Plate {
    pub pole: DVec3,
    pub spin: f64,
}

/// What stages 1 and 2 leave on the coarse grid.
pub struct Tectonics {
    pub plate: Vec<u8>,
    pub continental: Vec<bool>,
    /// 0 none, 1 convergent, 2 divergent, 3 transform
    pub boundary: Vec<u8>,
    pub uplift: Vec<f32>,
    /// starting height, model metres
    pub h0: Vec<f32>,
}

/// Stage 1: plates by Voronoi growth from random seeds, each growing at its own pace through a noisy
/// cost, so their borders wander; each plate turns about its own Euler pole; continental crust grown
/// inside the plates until it covers `continental` of the surface.
pub fn plates(g: &Grid, seed: u64, p: &Params) -> (Vec<u8>, Vec<bool>, Vec<Plate>) {
    let mut rng = Rng::new(seed);
    let k = rng.range(p.plates.0 as f64, p.plates.1 as f64 + 1.0).floor() as usize;
    let seeds: Vec<(u32, f64)> = (0..k).map(|_| (g.locate(rng.dir()), 0.0)).collect();
    let pace: Vec<f64> = (0..k).map(|_| rng.range(0.6, 1.4)).collect();
    let warp = noise(seed as i32 ^ 0x51ab, 3);
    let (_, src) = dijkstra(g, &seeds, f64::INFINITY, |_, o, d, s| d * pace[s as usize] * (1.0 + 0.7 * at(&warp, g.d(o), 3.0) as f64));
    let plate: Vec<u8> = src.iter().map(|&s| s as u8).collect();
    let plates: Vec<Plate> = (0..k).map(|_| Plate { pole: rng.dir(), spin: rng.range(0.5, 1.5) * if rng.f() < 0.5 { -1.0 } else { 1.0 } }).collect();

    // continents: from each plate's seed in a random order, a noisy blob of a random share of the plate
    let total: f64 = (0..g.n() as u32).map(|c| g.area(c) as f64).sum();
    let mut area = vec![0.0; k];
    for c in 0..g.n() as u32 {
        area[plate[c as usize] as usize] += g.area(c) as f64;
    }
    let mut cont = vec![false; g.n()];
    let mut have = 0.0;
    let mut order: Vec<usize> = (0..k).collect();
    for i in (1..k).rev() {
        order.swap(i, (rng.u64() % (i as u64 + 1)) as usize);
    }
    let blob = noise(seed as i32 ^ 0x7c3d, 5);
    for &pl in &order {
        if have >= p.continental as f64 * total {
            break;
        }
        let want = (rng.range(0.35, 0.95) * area[pl]).min(p.continental as f64 * total - have);
        let (cost, _) = dijkstra(g, &[seeds[pl]], f64::INFINITY, |_, o, d, _| {
            if plate[o as usize] as usize != pl {
                f64::INFINITY
            } else {
                // strongly uneven going: coasts with bays and peninsulas, not discs
                d * (2.2 * at(&blob, g.d(o), 2.5) as f64).exp()
            }
        });
        let mut cells: Vec<u32> = (0..g.n() as u32).filter(|&c| cost[c as usize].is_finite()).collect();
        cells.sort_by(|a, b| cost[*a as usize].total_cmp(&cost[*b as usize]).then(a.cmp(b)));
        let mut got = 0.0;
        for c in cells {
            if got >= want {
                break;
            }
            cont[c as usize] = true;
            got += g.area(c) as f64;
        }
        have += got;
    }
    (plate, cont, plates)
}

/// Stage 2: at each boundary the plates' relative motion says what happens (after Cortial et al. 2019,
/// without stepping time): collision belts where continents meet, a volcanic arc on the overriding side
/// and a trench on the sinking one where an ocean dives, island arcs where oceans meet, ridges and rifts
/// where plates part, little at transforms. The uplift falls off with distance from the boundary.
pub fn tectonics(g: &Grid, seed: u64, p: &Params) -> Tectonics {
    let (plate, cont, plates) = plates(g, seed, p);
    let n = g.n();
    let vel = |c: u32| {
        let pl = &plates[plate[c as usize] as usize];
        pl.pole.cross(g.d(c)) * pl.spin
    };
    let mut boundary = vec![0u8; n];
    // per boundary cell: (amplitude, width m, offset m)
    let mut src: Vec<(u32, f64)> = Vec::new();
    let mut what: Vec<(f64, f64, f64)> = Vec::new();
    let mut ridges: Vec<(u32, f64)> = Vec::new();
    let r = g.r;
    for c in 0..n as u32 {
        let pc = plate[c as usize];
        let mut best: Option<(f64, f64, u32)> = None; // (|v_rel|, convergence, neighbour)
        for o in g.around(c) {
            if plate[o as usize] == pc {
                continue;
            }
            let (dc, dn) = (g.d(c), g.d(o));
            let t = ((dn - dc) - dc * (dn - dc).dot(dc)).normalize();
            let rel = vel(c) - vel(o);
            let conv = rel.dot(t);
            if best.is_none_or(|b| rel.length() > b.0) {
                best = Some((rel.length(), conv, o));
            }
        }
        let Some((speed, conv, o)) = best else { continue };
        let (cc, co) = (cont[c as usize], cont[o as usize]);
        let kind = if conv > 0.35 * speed { 1 } else if conv < -0.35 * speed { 2 } else { 3 };
        boundary[c as usize] = kind;
        let s = conv.abs().min(2.0);
        let w = match kind {
            1 => match (cc, co) {
                (true, true) => (1.0 * s, p.collision_w * r, 0.0),
                (true, false) => (0.8 * s, p.arc_w * r, 0.6 * p.arc_w * r),
                (false, true) => (-0.6 * s, 0.4 * p.arc_w * r, 0.0),
                // oceans meet: the plate with the higher index rides over and grows an island arc
                (false, false) if pc > plate[o as usize] => (0.6 * s, 0.6 * p.arc_w * r, 0.4 * p.arc_w * r),
                (false, false) => (-0.5 * s, 0.4 * p.arc_w * r, 0.0),
            },
            2 => {
                ridges.push((c, 0.0));
                if cc { (-0.35 * s, p.rift_w * r, 0.0) } else { (0.0, 1.0, 0.0) }
            }
            _ => (0.12 * s, 0.3 * p.arc_w * r, 0.0),
        };
        src.push((c, 0.0));
        what.push(w);
    }
    let reach = 3.0 * p.collision_w * r;
    let (dist, near) = dijkstra(g, &src, reach, |_, _, d, _| d);
    let base = noise(seed as i32 ^ 0x1f2e, 4);
    let spots: Vec<(DVec3, f64)> = {
        let mut rng = Rng::new(seed ^ 0xabcdef);
        (0..rng.range(3.0, 7.0) as usize).map(|_| (rng.dir(), rng.range(0.4, 0.9))).collect()
    };
    let (age, _) = dijkstra(g, &ridges, f64::INFINITY, |_, _, d, _| d);
    // how far inside its continent a cell lies: margins slope down into shelves
    let edge: Vec<(u32, f64)> = (0..n as u32).filter(|&c| cont[c as usize] && g.around(c).any(|o| !cont[o as usize])).map(|c| (c, 0.0)).collect();
    let (inland, _) = dijkstra(g, &edge, f64::INFINITY, |_, o, d, _| if cont[o as usize] { d } else { f64::INFINITY });
    let old = noise(seed as i32 ^ 0x0e1d, 3);
    let mut uplift = vec![0f32; n];
    let mut h0 = vec![0f32; n];
    uplift.par_iter_mut().zip(h0.par_iter_mut()).enumerate().for_each(|(c, (u, h))| {
        let d = g.d(c as u32);
        let mut v = 0.0;
        if near[c] != NONE {
            let (a, w, off) = what[near[c] as usize];
            v += a * (-((dist[c] as f64 - off) / w).powi(2)).exp();
        }
        let nz = at(&base, d, 6.0) as f64;
        if cont[c] {
            // old shields far inside: a slow, patchy rise that makes hills, not ranges
            v += 0.05 * (1.0 + nz);
            // old worn ranges along ancient sutures: the ridges of a low noise
            v += 0.35 * (1.0 - 4.0 * at(&old, d, 2.2).abs() as f64).max(0.0);
        }
        for (s, a) in &spots {
            v += a * (-(d.dot(*s).clamp(-1., 1.).acos() * r / (p.arc_w * r * 0.5)).powi(2)).exp();
        }
        *u = (v * p.uplift as f64) as f32;
        // continental crust stands high, oceanic deepens away from the ridges (cooling with age)
        *h = if cont[c] {
            (-8.0 + 14.0 * (inland[c] as f64 / (0.06 * r)).min(1.0) + 5.0 * nz) as f32
        } else {
            let a = (age[c] as f64 / (0.5 * r)).min(1.0);
            (-(20.0 + 40.0 * a.sqrt()) + 4.0 * nz) as f32
        };
    });
    Tectonics { plate, continental: cont, boundary, uplift, h0 }
}

/// The sea level: the height under which `share` of the surface lies (by cell count; cells are within
/// 20 % of each other's area and the share is checked by area in the report).
fn quantile(h: &[f32], share: f32) -> f32 {
    let mut v = h.to_vec();
    let k = ((v.len() as f32 * share) as usize).min(v.len() - 1);
    *v.select_nth_unstable_by(k, f32::total_cmp).1
}

/// The drainage of a height field: Priority-Flood (Barnes et al. 2014, with epsilon) from the coast fills
/// every depression to its spill point, so each land cell has a strictly lower neighbour; the steepest of
/// those is its receiver (D8, O'Callaghan and Mark 1984). `order` lists the land cells from the coast up,
/// so a cell's receiver comes before it (the stack order of Braun and Willett 2013).
pub struct Drainage {
    pub ocean: Vec<bool>,
    /// the filled surface: the ground, or the lake's level over it
    pub w: Vec<f32>,
    pub recv: Vec<u32>,
    pub order: Vec<u32>,
    pub area: Vec<f32>,
}

/// A lake: the filled surface stands this much over the ground (model metres).
pub const LAKE: f32 = 0.05;
const EPS: f32 = 1e-4;

/// The ocean: the regions under the sea level big enough to be a sea (0.2 % of the surface). A smaller
/// hollow under the sea level inland is a basin: it fills to a lake at its spill point.
pub fn oceans(g: &Grid, h: &[f32], sea: f32) -> Vec<bool> {
    let n = g.n();
    let mut ocean = vec![false; n];
    let mut seen = vec![false; n];
    for c0 in 0..n as u32 {
        if seen[c0 as usize] || h[c0 as usize] >= sea {
            continue;
        }
        seen[c0 as usize] = true;
        let (mut stack, mut cells) = (vec![c0], Vec::new());
        while let Some(c) = stack.pop() {
            cells.push(c);
            for o in g.around(c) {
                if !seen[o as usize] && h[o as usize] < sea {
                    seen[o as usize] = true;
                    stack.push(o);
                }
            }
        }
        if cells.len() * 500 >= n {
            for c in cells {
                ocean[c as usize] = true;
            }
        }
    }
    ocean
}

pub fn drain(g: &Grid, h: &[f32], sea: f32) -> Drainage {
    let n = g.n();
    let ocean = oceans(g, h, sea);
    let mut w: Vec<f32> = h.iter().zip(&ocean).map(|(&v, &o)| if o { sea } else { v }).collect();
    let mut done = ocean.clone();
    let mut q = BinaryHeap::new();
    for c in 0..n as u32 {
        if !done[c as usize] && g.around(c).any(|o| ocean[o as usize]) {
            done[c as usize] = true;
            q.push(Reverse((key(w[c as usize]), c)));
        }
    }
    let mut order = Vec::with_capacity(n / 2);
    while let Some(Reverse((_, c))) = q.pop() {
        order.push(c);
        let wc = w[c as usize];
        for o in g.around(c) {
            if !done[o as usize] {
                done[o as usize] = true;
                w[o as usize] = h[o as usize].max(wc + EPS);
                q.push(Reverse((key(w[o as usize]), o)));
            }
        }
    }
    let recv: Vec<u32> = (0..n as u32)
        .into_par_iter()
        .map(|c| {
            if ocean[c as usize] {
                return NONE;
            }
            let wc = w[c as usize];
            let mut best = (0.0f32, NONE);
            for o in g.around(c) {
                // the sea's surface, a hair under the coast, so a coast cell at the waterline still drains
                let wo = if ocean[o as usize] { sea - EPS } else { w[o as usize] };
                let s = (wc - wo) / g.dist(c, o);
                if s > best.0 || (s == best.0 && s > 0.0 && o < best.1) {
                    best = (s, o);
                }
            }
            best.1
        })
        .collect();
    let mut area: Vec<f32> = (0..n as u32).into_par_iter().map(|c| g.area(c)).collect();
    for &c in order.iter().rev() {
        let r = recv[c as usize];
        if r != NONE && !ocean[r as usize] {
            area[r as usize] += area[c as usize];
        }
    }
    Drainage { w, recv, order, area, ocean }
}

/// Stage 3 at one level: uplift against stream-power erosion, solved implicitly along the receivers
/// (Braun and Willett 2013; the scheme of Cordonnier et al. 2016), with hillslope diffusion, the sea level
/// cut at the ocean share every step. Returns the largest change of the last step (metres).
pub fn erode(g: &Grid, h: &mut [f32], u: &[f32], p: &Params, iters: usize) -> f32 {
    let mut last = 0.0;
    let n = g.n();
    for _ in 0..iters {
        let sea = quantile(h, p.ocean_share);
        let d = drain(g, h, sea);
        let before = h.to_vec();
        for &c in &d.order {
            let c = c as usize;
            if d.w[c] - h[c] > LAKE {
                // a lake floor: it fills, nothing cuts it
                h[c] += u[c].max(0.0);
                continue;
            }
            let r = d.recv[c] as usize;
            let base = if d.ocean[r] { sea } else if d.w[r] - h[r] > LAKE { d.w[r] } else { h[r] };
            let f = p.k * d.area[c].powf(p.m_exp) / g.dist(c as u32, r as u32);
            h[c] = ((h[c] + u[c]) + f * base) / (1.0 + f);
        }
        // the sea floor rises with the arcs and sinks into the trenches, within bounds
        for c in 0..n {
            if d.ocean[c] {
                h[c] = (h[c] + u[c] * 0.5).max(-120.0);
            }
        }
        // hillslope diffusion on land; the sea's neighbours count at the waterline, so coasts stay
        let snap = h.to_vec();
        h.par_iter_mut().enumerate().for_each(|(c, hv)| {
            if d.ocean[c] {
                return;
            }
            let (nb, k) = g.neigh(c as u32);
            let (mut s, mut wsum) = (0.0, 0.0);
            for &o in &nb[..k] {
                let l2 = g.dist(c as u32, o).powi(2);
                s += (if d.ocean[o as usize] { sea.max(snap[o as usize]) } else { snap[o as usize] } - snap[c]) / l2;
                wsum += 1.0 / l2;
            }
            let a = (p.kappa * wsum).min(0.5) / wsum.max(1e-12);
            *hv = snap[c] + a * s;
        });
        last = (0..n).filter(|&c| !d.ocean[c]).map(|c| (h[c] - before[c]).abs()).fold(0.0, f32::max);
    }
    last
}

/// The coarse field at each fine cell: a tent filter over the coarse cell the fine cell lies in and its
/// neighbours, by true distance, so it is as smooth across a seam as inside a face. (Bilinear inside each
/// face, clamped at its border, made the seams steps 3 to 4 times steeper than anywhere else: measured.)
fn upsample(gc: &Grid, gf: &Grid, v: &[f32]) -> Vec<f32> {
    let s = (gc.r * std::f64::consts::FRAC_PI_2 / gc.m as f64) as f32 * 1.3;
    (0..gf.n() as u32)
        .into_par_iter()
        .map(|c| {
            let d = gf.dir[c as usize];
            let k = gc.locate(gf.d(c));
            let (mut sum, mut wsum) = (0.0f32, 0.0f32);
            for o in std::iter::once(k).chain(gc.around(k)) {
                let e = gc.dir[o as usize];
                let dist = ((d[0] - e[0]).powi(2) + (d[1] - e[1]).powi(2) + (d[2] - e[2]).powi(2)).sqrt() * gc.r as f32;
                let w = (1.0 - dist / s).max(0.0);
                sum += w * v[o as usize];
                wsum += w;
            }
            sum / wsum
        })
        .collect()
}

pub struct River {
    pub cell: u32,
    pub to: u32,
    /// drained area, m^2
    pub area: f32,
}
pub struct Lake {
    pub cells: u32,
    /// level over the sea, metres
    pub level: f32,
}

pub struct Bake {
    pub g: Grid,
    /// the sea's cells (an inland hollow under the sea level is land: a lake)
    pub ocean: Vec<bool>,
    /// metres over the sea level (negative: under it)
    pub h: Vec<f32>,
    /// the filled surface: lake levels, metres over the sea
    pub w: Vec<f32>,
    pub recv: Vec<u32>,
    pub area: Vec<f32>,
    pub plate: Vec<u8>,
    pub boundary: Vec<u8>,
    pub continental: Vec<bool>,
    pub rivers: Vec<River>,
    pub lakes: Vec<Lake>,
    /// seconds per stage
    pub times: Vec<(&'static str, f64)>,
    pub converged: (f32, f32),
    /// metres per model metre: (land, sea)
    pub scale: (f32, f32),
}

impl Bake {
    pub fn land(&self, c: usize) -> bool {
        !self.ocean[c]
    }
    pub fn lake(&self, c: usize) -> bool {
        self.land(c) && self.w[c] - self.h[c] > LAKE * self.land_scale()
    }
    /// metres per model metre on land, kept for the lake test
    pub fn land_scale(&self) -> f32 {
        self.scale.0
    }
}

pub fn bake(planet_n: u32, seed: u64, p: &Params) -> Bake {
    let mut times = Vec::new();
    let mut t = Instant::now();
    let mut lap = |name: &'static str, times: &mut Vec<(&'static str, f64)>| {
        times.push((name, t.elapsed().as_secs_f64()));
        t = Instant::now();
    };
    // the engine's radius for N columns of 0.5 m (ADR 0003)
    let r = planet_n as f64 * 0.5 / (2.0 * std::f64::consts::PI / 3.0).sqrt();
    let mf = ((planet_n as f64 * 0.5 / p.cell_m).round() as usize).max(p.coarse * 8);
    let mf = mf - mf % p.coarse;
    let gc = Grid::new(mf / p.coarse, r);
    let gf = Grid::new(mf, r);
    lap("grid", &mut times);
    let tc = tectonics(&gc, seed, p);
    lap("plates and boundaries", &mut times);
    let mut hc = tc.h0.clone();
    let conv_c = erode(&gc, &mut hc, &tc.uplift, p, p.coarse_iters);
    lap("erosion, coarse", &mut times);
    if std::env::var("SEAMS").is_ok() {
        eprintln!("seams h0 {:?} uplift {:?} coarse {:?}", seam_steps(&gc, &tc.h0), seam_steps(&gc, &tc.uplift), seam_steps(&gc, &hc));
    }
    let mut h = upsample(&gc, &gf, &hc);
    if std::env::var("SEAMS").is_ok() {
        eprintln!("seams upsampled {:?}", seam_steps(&gf, &h));
    }
    let u = upsample(&gc, &gf, &tc.uplift);
    let detail = noise(seed as i32 ^ 0x3c3c, 3);
    h.par_iter_mut().enumerate().for_each(|(c, v)| *v += 0.4 * at(&detail, gf.d(c as u32), r / 12.0));
    let to_fine = |v: &Vec<u8>| -> Vec<u8> {
        (0..gf.n() as u32).map(|c| v[gc.locate(gf.d(c)) as usize]).collect()
    };
    let (plate, boundary) = (to_fine(&tc.plate), to_fine(&tc.boundary));
    let continental: Vec<bool> = (0..gf.n() as u32).map(|c| tc.continental[gc.locate(gf.d(c)) as usize]).collect();
    lap("refine", &mut times);
    let conv_f = erode(&gf, &mut h, &u, p, p.fine_iters);
    lap("erosion, fine", &mut times);
    if std::env::var("SEAMS").is_ok() {
        eprintln!("seams fine {:?}", seam_steps(&gf, &h));
    }

    // stage 4: the sea at the ocean share, then the relief scaled to the taste call
    // the sea level that puts `ocean_share` of the surface under the ocean (basins inland are not ocean)
    let share = |sea: f32| {
        let o = oceans(&gf, &h, sea);
        (0..gf.n()).filter(|&c| o[c]).map(|c| gf.area(c as u32) as f64).sum::<f64>() / (0..gf.n()).map(|c| gf.area(c as u32) as f64).sum::<f64>()
    };
    let (mut lo, mut hi) = (quantile(&h, p.ocean_share * 0.9), quantile(&h, (p.ocean_share * 1.2).min(0.95)));
    for _ in 0..10 {
        let mid = 0.5 * (lo + hi);
        if share(mid) < p.ocean_share as f64 { lo = mid } else { hi = mid }
    }
    let sea = 0.5 * (lo + hi);
    let d = drain(&gf, &h, sea);
    let top = h.iter().fold(0f32, |a, &v| a.max(v - sea)).max(1e-3);
    let deep = h.iter().fold(0f32, |a, &v| a.max(sea - v)).max(1e-3);
    let scale = (p.mountain_m / top, p.sea_depth_m / deep);
    let sc = |v: f32| if v >= sea { (v - sea) * scale.0 } else { (v - sea) * scale.1 };
    let hm: Vec<f32> = h.iter().map(|&v| sc(v)).collect();
    let wm: Vec<f32> = d.w.iter().map(|&v| sc(v)).collect();
    lap("sea and coast", &mut times);

    let rivers: Vec<River> = d
        .order
        .iter()
        .filter(|&&c| d.area[c as usize] >= p.river_area_m2)
        .map(|&c| River { cell: c, to: d.recv[c as usize], area: d.area[c as usize] })
        .collect();
    // lakes: the connected cells where the filled surface stands over the ground
    let lake_at = |c: usize| !d.ocean[c] && wm[c] - hm[c] > LAKE * scale.0;
    let mut seen = vec![false; gf.n()];
    let mut lakes = Vec::new();
    for &c0 in &d.order {
        if seen[c0 as usize] || !lake_at(c0 as usize) {
            continue;
        }
        let (mut stack, mut cells) = (vec![c0], 0u32);
        seen[c0 as usize] = true;
        while let Some(c) = stack.pop() {
            cells += 1;
            for o in gf.around(c) {
                if !seen[o as usize] && lake_at(o as usize) {
                    seen[o as usize] = true;
                    stack.push(o);
                }
            }
        }
        lakes.push(Lake { cells, level: wm[c0 as usize] });
    }
    lap("rivers and lakes", &mut times);
    let (recv, area, ocean) = (d.recv, d.area, d.ocean);
    Bake { g: gf, h: hm, w: wm, recv, area, ocean, plate, boundary, continental, rivers, lakes, times, converged: (conv_c, conv_f), scale }
}

/// Mean height step between neighbours (inside a face, across a seam): a seam that shows has a larger one.
pub fn seam_steps(g: &Grid, h: &[f32]) -> (f64, f64) {
    let (mut a, mut na, mut b, mut nb) = (0.0, 0, 0.0, 0);
    for c in 0..g.n() as u32 {
        let f = g.fij(c).0;
        for o in g.around(c) {
            let d = (h[c as usize] - h[o as usize]).abs() as f64 / g.dist(c, o) as f64;
            if g.fij(o).0 == f { a += d; na += 1 } else { b += d; nb += 1 }
        }
    }
    (a / na as f64, b / nb as f64)
}

pub fn fnv(h: u64, b: &[u8]) -> u64 {
    b.iter().fold(h, |h, &x| (h ^ x as u64).wrapping_mul(0x100_0000_01b3))
}

impl Bake {
    /// One number for the whole result: heights, plates, rivers and lakes.
    pub fn hash(&self) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325;
        for v in &self.h {
            h = fnv(h, &v.to_bits().to_le_bytes());
        }
        h = fnv(h, &self.plate);
        for r in &self.rivers {
            h = fnv(h, &r.cell.to_le_bytes());
            h = fnv(h, &r.to.to_le_bytes());
        }
        for l in &self.lakes {
            h = fnv(h, &l.cells.to_le_bytes());
            h = fnv(h, &l.level.to_bits().to_le_bytes());
        }
        h
    }
}
