//! Stages 1 to 4 of `docs/design/worldgen.md`: plates, their boundaries and uplift, uplift against
//! stream-power erosion, sea level, the river network and lakes. Deterministic from the seed: every
//! sum and every queue runs in a fixed order; rayon only maps cells to values one by one.
use super::grid::{Grid, NONE};
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
    /// TASTE CALL, decided 2026-11-11: 120 m (ADR 0021 had 50). The highest land over the sea, metres, before
    /// the peaks are stretched by `peak_extra_m`.
    pub mountain_m: f32,
    /// How much higher the highest land stands after the peaks are stretched (`stretch`, ADR 0105), metres on
    /// the 4 km planet and larger ones; a smaller planet gets its share by size. 0 is the bake before 0105.
    pub peak_extra_m: f32,
    /// TASTE CALL, decided 2026-11-11: 60 m (ADR 0021 had 25). The deepest sea under it, metres.
    pub sea_depth_m: f32,
    /// The share of the surface under the sea (ADR 0021: 55 %).
    pub ocean_share: f32,
    /// Map cell edge at the fine level, metres (worldgen.md: 8 m).
    pub cell_m: f64,
    /// The coarse level's cells are this many fine cells across (8 m * 4 = 32 m, worldgen.md).
    pub coarse: usize,
    pub coarse_iters: usize,
    pub fine_iters: usize,
    /// How long a step is: the solver is implicit, so a longer step is stable and needs fewer steps.
    pub coarse_dt: f32,
    pub fine_dt: f32,
    /// A lake must be this deep somewhere, metres; a shallower one is filled.
    pub min_lake_m: f32,
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
    /// Uplift inside continents, as a share of the peak's: the old shields' patchy rise, and old worn
    /// ranges along ancient sutures. These raise the plains against the ranges at the plate edges.
    pub shield_uplift: f64,
    pub old_range_uplift: f64,
    /// The share of crust that is continental (a little over the land share: the lowest of it is shelf).
    pub continental: f32,
    /// Feature widths as shares of the radius: they scale with the planet (ADR 0021: continents do, hills
    /// stay in metres).
    pub collision_w: f64,
    pub arc_w: f64,
    pub rift_w: f64,
    /// Volcanoes on the arcs (`super::volcano`, map version 9 on); false: the bakes before them.
    pub volcanoes: bool,
    /// The water table and the springs' streams (`super::ground`, map version 10 on); false: the bakes before it.
    pub ground_water: bool,
    /// The climate's year on an air grid twice as coarse as the live one (`life::LifeParams::air_coarse`, map
    /// version 11 on); false: the bakes before it, the year on the live grid's own cells.
    pub coarse_air: bool,
    /// Hill country inside the continents: uplift in ridges `hill_m` metres apart, as a share of the peak's.
    pub hills: f64,
    pub hill_m: f64,
    /// The peaks' stretch (`stretch`): the height it starts from and the metres over which it eases in.
    pub stretch: (f32, f32),
}

impl Params {
    /// The land of the maps before hill country (versions 10 and older): the shields' rise a third of today's, no
    /// hills, the peaks stretched from `STRETCH_FROM` up.
    pub fn before_hills() -> Params {
        Params { shield_uplift: 0.35, hills: 0.0, stretch: (STRETCH_FROM, STRETCH_EASE), ..Params::default() }
    }
}

impl Default for Params {
    fn default() -> Params {
        Params {
            mountain_m: 120.0,
            peak_extra_m: 230.0,
            sea_depth_m: 60.0,
            ocean_share: 0.55,
            cell_m: 8.0,
            coarse: 4,
            coarse_iters: 320,
            fine_iters: 48,
            coarse_dt: 1.0,
            fine_dt: 1.0,
            min_lake_m: 1.0,
            k: 0.5,
            m_exp: 0.5,
            kappa: 1.5,
            uplift: 1.0,
            river_area_m2: 1.5e5,
            plates: (8, 14),
            shield_uplift: 1.0,
            old_range_uplift: 0.8,
            continental: 0.48,
            collision_w: 0.05,
            arc_w: 0.035,
            rift_w: 0.012,
            volcanoes: true,
            ground_water: true,
            coarse_air: true,
            hills: 2.0,
            hill_m: 700.0,
            stretch: (10.0, 90.0),
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

pub fn noise(seed: i32, octaves: i32) -> FastNoiseLite {
    let mut n = FastNoiseLite::with_seed(seed);
    n.set_noise_type(Some(NoiseType::OpenSimplex2));
    n.set_fractal_type(Some(FractalType::FBm));
    n.set_fractal_octaves(Some(octaves));
    n.set_frequency(Some(1.0));
    n
}
pub fn at(n: &FastNoiseLite, d: DVec3, freq: f64) -> f32 {
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
pub(super) fn dijkstra(g: &Grid, sources: &[(u32, f64)], limit: f64, step: impl Fn(u32, u32, f64, u32) -> f64) -> (Vec<f32>, Vec<u32>) {
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
    let hill = noise(seed as i32 ^ 0x4b1d, 4);
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
            v += p.shield_uplift * (1.0 + nz);
            // old worn ranges along ancient sutures: the ridges of a low noise
            v += p.old_range_uplift * (1.0 - 4.0 * at(&old, d, 2.2).abs() as f64).max(0.0);
            if p.hills > 0.0 {
                // hill country: ridges a few hundred metres apart (in metres on every planet), stronger in some
                // regions than others
                let ridge = (1.0 - 2.0 * at(&hill, d, r / p.hill_m).abs() as f64).max(0.0);
                v += p.hills * ridge * ridge * (0.7 + 0.6 * at(&base, d, 2.0) as f64).clamp(0.2, 1.3);
            }
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
    /// the filled surface: the ground, or the lake's level over it, sloped by the epsilon so water runs
    pub w: Vec<f32>,
    /// the spill level without the epsilon: where it stands over the ground there is a lake
    pub lvl: Vec<f32>,
    pub recv: Vec<u32>,
    pub order: Vec<u32>,
    pub area: Vec<f32>,
    /// the land cells by basin (`basins`)
    pub cells: Vec<u32>,
    pub start: Vec<usize>,
}

impl Drainage {
    /// Water stands over this cell: a lake, not a plain the flood's epsilon slopes.
    pub fn lake(&self, c: usize, h: f32) -> bool {
        !self.ocean[c] && self.lvl[c] - h > LAKE
    }
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

/// A monotone priority queue (radix heap, Ahuja et al. 1990): pops the exact minimum as long as nothing
/// smaller than the last popped key is pushed, which Priority-Flood guarantees. O(1) amortised per key
/// against the binary heap's O(log n); the same keys come out in the same order.
pub struct Radix {
    last: u64,
    b: Vec<Vec<u64>>,
}
impl Radix {
    pub fn new() -> Radix {
        Radix { last: 0, b: vec![Vec::new(); 65] }
    }
    fn slot(&self, x: u64) -> usize {
        if x == self.last { 0 } else { 64 - (x ^ self.last).leading_zeros() as usize }
    }
    pub fn push(&mut self, x: u64) {
        debug_assert!(x >= self.last);
        let i = self.slot(x);
        self.b[i].push(x);
    }
    pub fn pop(&mut self) -> Option<u64> {
        if self.b[0].is_empty() {
            let i = (1..65).find(|&i| !self.b[i].is_empty())?;
            let v = std::mem::take(&mut self.b[i]);
            self.last = *v.iter().min().unwrap();
            for x in &v {
                let j = self.slot(*x);
                self.b[j].push(*x);
            }
            self.b[i] = v;
            self.b[i].clear();
        }
        self.b[0].pop()
    }
}


/// A monotone priority queue for big floods: keys go into buckets by their top bits, and only the current
/// bucket is a binary heap. Pops the exact minimum while nothing smaller than the last pop is pushed, as
/// Priority-Flood guarantees: the same keys in the same order as `Radix`, with fewer moves per key.
pub struct Buckets {
    cur: usize,
    heap: BinaryHeap<Reverse<u64>>,
    b: Vec<Vec<u64>>,
}
impl Buckets {
    /// 2^20 buckets: a key's top 20 bits, so a bucket spans 2^-11 of a height's magnitude
    const SHIFT: u32 = 44;
    pub fn new() -> Buckets {
        Buckets { cur: 0, heap: BinaryHeap::new(), b: vec![Vec::new(); 1 << (64 - Self::SHIFT)] }
    }
    pub fn push(&mut self, x: u64) {
        let i = (x >> Self::SHIFT) as usize;
        if i <= self.cur && !self.heap.is_empty() {
            debug_assert!(i == self.cur);
            self.heap.push(Reverse(x));
        } else {
            self.b[i].push(x);
            self.cur = self.cur.min(i);
        }
    }
    pub fn pop(&mut self) -> Option<u64> {
        loop {
            if let Some(Reverse(x)) = self.heap.pop() {
                return Some(x);
            }
            while self.cur < self.b.len() && self.b[self.cur].is_empty() {
                self.cur += 1;
            }
            if self.cur == self.b.len() {
                return None;
            }
            let v = std::mem::take(&mut self.b[self.cur]);
            self.heap = BinaryHeap::from(v.into_iter().map(Reverse).collect::<Vec<_>>());
        }
    }
}

pub trait Queue {
    fn push(&mut self, x: u64);
    fn pop(&mut self) -> Option<u64>;
}
impl Queue for Radix {
    fn push(&mut self, x: u64) {
        Radix::push(self, x)
    }
    fn pop(&mut self) -> Option<u64> {
        Radix::pop(self)
    }
}
impl Queue for Buckets {
    fn push(&mut self, x: u64) {
        Buckets::push(self, x)
    }
    fn pop(&mut self) -> Option<u64> {
        Buckets::pop(self)
    }
}

/// Priority-Flood of one land mass from its coast: fills `w` (with the epsilon) and `lvl` (without),
/// returns the cells from the coast up.
/// SAFETY: the caller hands each mass to one thread; its cells and their land neighbours are its own.
unsafe fn flood<Q: Queue>(mut q: Q, coast: &[u32], g: &Grid, h: &[f32], ws: Shared<f32>, ls: Shared<f32>, ds: Shared<bool>) -> Vec<u32> {
    unsafe {
        for &c in coast {
            ds.set(c as usize, true);
            q.push((key(ws.get(c as usize)) as u64) << 32 | c as u64);
        }
        let mut order = Vec::new();
        while let Some(x) = q.pop() {
            let c = x as u32;
            order.push(c);
            let (wc, lc) = (ws.get(c as usize), ls.get(c as usize));
            for o in g.around(c) {
                if !ds.get(o as usize) {
                    ds.set(o as usize, true);
                    let ho = h[o as usize];
                    ws.set(o as usize, ho.max(wc + EPS));
                    ls.set(o as usize, ho.max(lc));
                    q.push((key(ws.get(o as usize)) as u64) << 32 | o as u64);
                }
            }
        }
        order
    }
}

/// Writes from parallel land masses or basins into one slice: sound because they share no cell.
#[derive(Clone, Copy)]
struct Shared<T>(*mut T);
unsafe impl<T> Send for Shared<T> {}
unsafe impl<T> Sync for Shared<T> {}
impl<T: Copy> Shared<T> {
    unsafe fn get(&self, i: usize) -> T {
        *self.0.add(i)
    }
    unsafe fn set(&self, i: usize, v: T) {
        *self.0.add(i) = v
    }
}

/// The coast cells of each land mass (land cells connected through land), the masses in the order of
/// their first cell, each one's coast in cell order. No water crosses from one mass to another, so each
/// drains on its own thread.
fn masses(g: &Grid, ocean: &[bool], coast: &[u32]) -> Vec<Vec<u32>> {
    let n = g.n();
    let mut label = vec![NONE; n];
    let mut k = 0u32;
    let mut stack = Vec::new();
    for c0 in 0..n as u32 {
        if ocean[c0 as usize] || label[c0 as usize] != NONE {
            continue;
        }
        label[c0 as usize] = k;
        stack.push(c0);
        while let Some(c) = stack.pop() {
            for o in g.around(c) {
                if !ocean[o as usize] && label[o as usize] == NONE {
                    label[o as usize] = k;
                    stack.push(o);
                }
            }
        }
        k += 1;
    }
    let mut seeds = vec![Vec::new(); k as usize];
    for &c in coast {
        seeds[label[c as usize] as usize].push(c);
    }
    seeds
}

pub fn drain(g: &Grid, h: &[f32], sea: f32) -> Drainage {
    let n = g.n();
    let ocean = oceans(g, h, sea);
    let mut w: Vec<f32> = h.iter().zip(&ocean).map(|(&v, &o)| if o { sea } else { v }).collect();
    let mut lvl = w.clone();
    let mut done = ocean.clone();
    let coast: Vec<u32> = (0..n as u32).into_par_iter().filter(|&c| !ocean[c as usize] && g.around(c).any(|o| ocean[o as usize])).collect();
    let seeds = masses(g, &ocean, &coast);
    let (ws, ls, ds) = (Shared(w.as_mut_ptr()), Shared(lvl.as_mut_ptr()), Shared(done.as_mut_ptr()));
    let floods: Vec<Vec<u32>> = seeds
        .par_iter()
        .map(|coast| {
            // SAFETY: a mass's cells and their land neighbours belong to it alone; the sea is only read
            // a long coast means a big mass: buckets beat the radix heap there, a small one needs no 2^20 buckets
            unsafe {
                if coast.len() > 20_000 {
                    flood(Buckets::new(), coast, g, h, ws, ls, ds)
                } else {
                    flood(Radix::new(), coast, g, h, ws, ls, ds)
                }
            }
        })
        .collect();
    // the receivers: the steepest way down the filled surface
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
    // the basins: grouped by the cell that drains into the sea, each in flood order
    let (mut root, mut index) = (vec![NONE; n], vec![NONE; n]);
    let (roots, idx) = (Shared(root.as_mut_ptr()), Shared(index.as_mut_ptr()));
    let parts: Vec<(Vec<u32>, Vec<u32>, Vec<usize>)> = floods
        .into_par_iter()
        .map(|order| {
            // SAFETY: as the flood: a basin lies inside one mass
            unsafe {
                let mut count: Vec<usize> = Vec::new();
                for &c in &order {
                    let r = recv[c as usize];
                    let rt = if ocean[r as usize] { c } else { roots.get(r as usize) };
                    roots.set(c as usize, rt);
                    if rt == c {
                        idx.set(c as usize, count.len() as u32);
                        count.push(0);
                    }
                    count[idx.get(rt as usize) as usize] += 1;
                }
                let mut at: Vec<usize> = count.iter().scan(0, |a, &k| { let s = *a; *a += k; Some(s) }).collect();
                let mut cells = vec![0u32; order.len()];
                for &c in &order {
                    let k = idx.get(roots.get(c as usize) as usize) as usize;
                    cells[at[k]] = c;
                    at[k] += 1;
                }
                (order, cells, count)
            }
        })
        .collect();
    let land: usize = parts.iter().map(|p| p.0.len()).sum();
    let (mut order, mut cells, mut start) = (Vec::with_capacity(land), Vec::with_capacity(land), vec![0usize]);
    for (o, c, k) in parts {
        order.extend(o);
        cells.extend(c);
        for k in k {
            start.push(start.last().unwrap() + k);
        }
    }
    let mut area: Vec<f32> = (0..n as u32).into_par_iter().map(|c| g.area(c)).collect();
    let a = Shared(area.as_mut_ptr());
    (0..start.len() - 1).into_par_iter().for_each(|k| {
        for &c in cells[start[k]..start[k + 1]].iter().rev() {
            let r = recv[c as usize];
            if !ocean[r as usize] {
                // SAFETY: c and r are in basin k, which no other thread touches
                unsafe { a.set(r as usize, a.get(r as usize) + a.get(c as usize)) };
            }
        }
    });
    Drainage { w, lvl, recv, order, area, ocean, cells, start }
}

/// Stage 3 at one level: uplift against stream-power erosion, solved implicitly along the receivers
/// (Braun and Willett 2013; the scheme of Cordonnier et al. 2016), with hillslope diffusion, the sea level
/// cut at the ocean share every step. Returns the largest change of the last step (metres).
pub fn erode(g: &Grid, h: &mut [f32], u: &[f32], p: &Params, iters: usize, dt: f32) -> f32 {
    let mut last = 0.0;
    let n = g.n();
    for k in 0..iters {
        super::within(k as f32 / iters as f32);
        let sea = quantile(h, p.ocean_share);
        let d = drain(g, h, sea);
        let before = h.to_vec();
        let hs = Shared(h.as_mut_ptr());
        (0..d.start.len() - 1).into_par_iter().for_each(|k| {
            for &c in &d.cells[d.start[k]..d.start[k + 1]] {
                let c = c as usize;
                // SAFETY: c and its receiver are in basin k, which no other thread touches
                let hc = unsafe { hs.get(c) };
                if d.lake(c, hc) {
                    // a lake floor: it fills, nothing cuts it
                    unsafe { hs.set(c, hc + dt * u[c].max(0.0)) };
                    continue;
                }
                let r = d.recv[c] as usize;
                let base = if d.ocean[r] { sea } else { let hr = unsafe { hs.get(r) }; if d.lake(r, hr) { d.lvl[r] } else { hr } };
                let f = dt * p.k * d.area[c].powf(p.m_exp) / g.dist(c as u32, r as u32);
                unsafe { hs.set(c, ((hc + dt * u[c]) + f * base) / (1.0 + f)) };
            }
        });
        // the sea floor rises with the arcs and sinks into the trenches, within bounds
        for c in 0..n {
            if d.ocean[c] {
                h[c] = (h[c] + dt * u[c] * 0.5).max(-120.0);
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
            let a = (dt * p.kappa * wsum).min(0.5) / wsum.max(1e-12);
            *hv = snap[c] + a * s;
        });
        last = (0..n).filter(|&c| !d.ocean[c]).map(|c| (h[c] - before[c]).abs()).fold(0.0, f32::max);
    }
    last
}

/// The coarse field at each fine cell: a tent filter over the coarse cell the fine cell lies in and its
/// neighbours, by true distance, so it is as smooth across a seam as inside a face. (Bilinear inside each
/// face, clamped at its border, made the seams steps 3 to 4 times steeper than anywhere else: measured.)
pub fn upsample(gc: &Grid, gf: &Grid, v: &[f32]) -> Vec<f32> {
    (0..gf.n() as u32).into_par_iter().map(|c| upsample_at(gc, gf.dir[c as usize], gc.locate(gf.d(c)), v)).collect()
}

/// `upsample` at one unit direction `d` that lies in coarse cell `k` (the map's climate at a point).
pub fn upsample_at(gc: &Grid, d: [f32; 3], k: u32, v: &[f32]) -> f32 {
    let s = (gc.r * std::f64::consts::FRAC_PI_2 / gc.m as f64) as f32 * 1.3;
    let (mut sum, mut wsum) = (0.0f32, 0.0f32);
    for o in std::iter::once(k).chain(gc.around(k)) {
        let e = gc.dir[o as usize];
        let dist = ((d[0] - e[0]).powi(2) + (d[1] - e[1]).powi(2) + (d[2] - e[2]).powi(2)).sqrt() * gc.r as f32;
        let w = (1.0 - dist / s).max(0.0);
        sum += w * v[o as usize];
        wsum += w;
    }
    sum / wsum
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
    /// the deepest water, metres (before the peaks' stretch, ADR 0105)
    pub depth: f32,
}

pub struct Bake {
    pub g: Grid,
    /// the sea's cells (an inland hollow under the sea level is land: a lake)
    pub ocean: Vec<bool>,
    /// metres over the sea level (negative: under it)
    pub h: Vec<f32>,
    /// the filled surface: lake levels, metres over the sea
    pub w: Vec<f32>,
    /// the lake level without the flood's epsilon (`Drainage::lvl`), metres over the sea
    pub lvl: Vec<f32>,
    /// the lakes' cells (`lake`)
    lake: Vec<bool>,
    pub recv: Vec<u32>,
    pub area: Vec<f32>,
    pub plate: Vec<u8>,
    pub boundary: Vec<u8>,
    pub continental: Vec<bool>,
    /// Each cell's rock province (`super::rock`, stage 9).
    pub rock: Vec<u8>,
    pub rivers: Vec<River>,
    pub lakes: Vec<Lake>,
    /// seconds per stage
    pub times: Vec<(&'static str, f64)>,
    pub converged: (f32, f32),
    /// metres per model metre: (land, sea); on land under `STRETCH_FROM` (ADR 0105)
    pub scale: (f32, f32),
    /// stages 5 to 8 (`life.rs`), once run
    pub life: Option<super::life::Life>,
}

impl Bake {
    pub fn land(&self, c: usize) -> bool {
        !self.ocean[c]
    }
    /// Under a lake: where the filled surface stood over the ground by `LAKE` before the stretch.
    pub fn lake(&self, c: usize) -> bool {
        self.lake[c]
    }
}

/// The `plates` and `erosion` stages as one call (stages 1 to 4 of worldgen.md).
pub fn bake(planet_n: u32, seed: u64, p: &Params) -> Bake {
    erosion(crust(planet_n, seed, p), planet_n, seed, p)
}

/// The crust by plates (the bake's `plates` stage, 0165-bake-stages): the grids, the plates and their
/// boundaries' uplift.
pub struct Crust {
    gc: Grid,
    gf: Grid,
    tc: Tectonics,
    r: f64,
    times: Vec<(&'static str, f64)>,
    t: Instant,
}

/// Records stage `name`'s seconds since `t` and reports it (`super::lap`).
fn lap_at(name: &'static str, times: &mut Vec<(&'static str, f64)>, t: &mut Instant) {
    times.push((name, t.elapsed().as_secs_f64()));
    *t = Instant::now();
    super::lap(name);
}

/// The `plates` stage: stages 1 and 2 of worldgen.md.
pub fn crust(planet_n: u32, seed: u64, p: &Params) -> Crust {
    let (mut times, mut t) = (Vec::new(), Instant::now());
    // the engine's radius for N columns of 1 m blocks (ADR 0003, 0033)
    let r = planet_n as f64 / (2.0 * std::f64::consts::PI / 3.0).sqrt();
    let mf = ((planet_n as f64 / p.cell_m).round() as usize).max(p.coarse * 8);
    let mf = mf - mf % p.coarse;
    let gc = Grid::new(mf / p.coarse, r);
    let gf = Grid::new(mf, r);
    lap_at("grid", &mut times, &mut t);
    let tc = tectonics(&gc, seed, p);
    lap_at("plates and boundaries", &mut times, &mut t);
    Crust { gc, gf, tc, r, times, t }
}

/// The `erosion` stage on a crust by plates: erosion coarse and fine (the provinces laid as the grid is
/// refined), the sea and the coast, the rivers and lakes (stages 3 and 4).
pub fn erosion(c: Crust, planet_n: u32, seed: u64, p: &Params) -> Bake {
    let Crust { gc, gf, tc, r, mut times, mut t } = c;
    let mut lap = |name: &'static str, times: &mut Vec<(&'static str, f64)>| lap_at(name, times, &mut t);
    let mut hc = tc.h0.clone();
    let conv_c = erode(&gc, &mut hc, &tc.uplift, p, p.coarse_iters, p.coarse_dt);
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
    let rock = to_fine(&super::rock::provinces(&gc, &tc, seed, p));
    let continental: Vec<bool> = (0..gf.n() as u32).map(|c| tc.continental[gc.locate(gf.d(c)) as usize]).collect();
    lap("refine", &mut times);
    let conv_f = erode(&gf, &mut h, &u, p, p.fine_iters, p.fine_dt);
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
    let mut hm: Vec<f32> = h.iter().map(|&v| sc(v)).collect();
    let mut wm: Vec<f32> = d.w.iter().map(|&v| sc(v)).collect();
    let mut lm: Vec<f32> = d.lvl.iter().map(|&v| sc(v)).collect();
    lap("sea and coast", &mut times);

    let rivers: Vec<River> = d
        .order
        .iter()
        .filter(|&&c| d.area[c as usize] >= p.river_area_m2)
        .map(|&c| River { cell: c, to: d.recv[c as usize], area: d.area[c as usize] })
        .collect();
    // lakes: the connected cells where the filled surface stands over the ground
    let lake_at = |c: usize| !d.ocean[c] && lm[c] - hm[c] > LAKE * scale.0;
    let mut seen = vec![false; gf.n()];
    let mut found: Vec<Vec<u32>> = Vec::new();
    for &c0 in &d.order {
        if seen[c0 as usize] || !lake_at(c0 as usize) {
            continue;
        }
        let (mut stack, mut cells) = (vec![c0], Vec::new());
        seen[c0 as usize] = true;
        while let Some(c) = stack.pop() {
            cells.push(c);
            for o in gf.around(c) {
                if !seen[o as usize] && lake_at(o as usize) {
                    seen[o as usize] = true;
                    stack.push(o);
                }
            }
        }
        found.push(cells);
    }
    // a lake shallower than `min_lake_m` everywhere is a puddle or a lagoon the rising sea left: its silt
    // fills it, flat ground at the water's level
    let mut lakes = Vec::new();
    for cells in found {
        let depth = cells.iter().map(|&c| lm[c as usize] - hm[c as usize]).fold(0f32, f32::max);
        if depth < p.min_lake_m {
            cells.iter().for_each(|&c| hm[c as usize] = lm[c as usize]);
        } else {
            lakes.push(Lake { cells: cells.len() as u32, level: lm[cells[0] as usize], depth });
        }
    }
    // the lakes are found before the peaks are stretched (ADR 0105), so they are the unstretched bake's
    let lake = (0..gf.n()).map(|c| !d.ocean[c] && lm[c] - hm[c] > LAKE * scale.0).collect();
    let extra = p.peak_extra_m * (planet_n as f32 / 6144.0).min(1.0);
    hm.iter_mut().chain(wm.iter_mut()).chain(lm.iter_mut()).for_each(|v| *v = stretch(*v, p.mountain_m, extra, p.stretch));
    lakes.iter_mut().for_each(|l| l.level = stretch(l.level, p.mountain_m, extra, p.stretch));
    lap("rivers and lakes", &mut times);
    let (recv, area, ocean) = (d.recv, d.area, d.ocean);
    Bake { g: gf, h: hm, w: wm, lvl: lm, recv, area, ocean, lake, plate, boundary, continental, rock, rivers, lakes, times, converged: (conv_c, conv_f), scale, life: None }
}

/// Land under this height, metres over the sea, is never stretched (`stretch`).
pub const STRETCH_FROM: f32 = 45.0;
/// Over this many metres above `STRETCH_FROM` the stretch eases from none to its full factor.
pub const STRETCH_EASE: f32 = 20.0;

/// The peaks stretched (ADR 0105): height `v` over the sea on land whose highest point is `top`, raised so
/// that `top` stands `extra` higher. Under `STRETCH_FROM` it is `v` to the bit; above, the relief over it is
/// scaled by a factor that eases in from 1, so the map's slope is continuous and the order of heights (every
/// downhill, so every river and lake) is kept.
pub fn stretch(v: f32, top: f32, extra: f32, (from, ease): (f32, f32)) -> f32 {
    if v <= from || extra <= 0.0 {
        return v;
    }
    debug_assert!(top >= from + ease, "the stretch eases in under the highest land");
    let x = v - from;
    let e = (x / ease).min(1.0);
    from + x * (1.0 + extra / (top - from) * e * e * (3.0 - 2.0 * e))
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
        self.life.as_ref().map_or(h, |l| l.hash(h))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The land's relief by numbers, to try values with (RELIEF_N, RELIEF_SEED; HILLS, HILL_M, SHIELD, OLD, KAPPA,
    /// K, MOUNTAIN, EXTRA override the parameters): the heights' quantiles, the relief inside 130 m, the slope.
    #[test]
    #[ignore]
    fn relief_report() {
        let env = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<f64>().ok());
        let mut p = Params { volcanoes: false, ..Params::default() };
        p.hills = env("HILLS").unwrap_or(p.hills);
        p.hill_m = env("HILL_M").unwrap_or(p.hill_m);
        p.shield_uplift = env("SHIELD").unwrap_or(p.shield_uplift);
        p.old_range_uplift = env("OLD").unwrap_or(p.old_range_uplift);
        p.kappa = env("KAPPA").map_or(p.kappa, |v| v as f32);
        p.k = env("K").map_or(p.k, |v| v as f32);
        p.mountain_m = env("MOUNTAIN").map_or(p.mountain_m, |v| v as f32);
        p.peak_extra_m = env("EXTRA").map_or(p.peak_extra_m, |v| v as f32);
        p.stretch = (env("FROM").map_or(p.stretch.0, |v| v as f32), env("EASE").map_or(p.stretch.1, |v| v as f32));
        let (n, seed) = (env("RELIEF_N").unwrap_or(3072.0) as u32, env("RELIEF_SEED").unwrap_or(1337.0) as u64);
        let b = bake(n, seed, &p);
        let g = &b.g;
        let land: Vec<usize> = (0..g.n()).filter(|&c| b.land(c) && !b.lake(c)).collect();
        let q = |v: &mut Vec<f32>| {
            v.sort_by(f32::total_cmp);
            [0.1, 0.25, 0.5, 0.75, 0.9, 0.99, 1.0].map(|s: f32| v[((v.len() - 1) as f32 * s) as usize].round())
        };
        let mut hs: Vec<f32> = land.iter().map(|&c| b.h[c]).collect();
        // the relief inside 8 cells (64 m) of every 53rd land cell, and its steepest drop to a neighbour
        let (mut rel, mut slope) = (Vec::new(), Vec::new());
        for &c in land.iter().step_by(53) {
            let (mut lo, mut hi) = (b.h[c], b.h[c]);
            let mut seen = std::collections::HashSet::from([c as u32]);
            let mut ring = vec![c as u32];
            for _ in 0..8 {
                let mut next = Vec::new();
                for &a in &ring {
                    for o in g.around(a) {
                        if seen.insert(o) {
                            let h = b.h[o as usize].max(0.0);
                            (lo, hi) = (lo.min(h), hi.max(h));
                            next.push(o);
                        }
                    }
                }
                ring = next;
            }
            rel.push(hi - lo);
            slope.push(g.around(c as u32).map(|o| (b.h[c] - b.h[o as usize].max(0.0)) / g.dist(c as u32, o)).fold(0f32, f32::max) * 100.0);
        }
        println!("RELIEF hills {} hill_m {} shield {} old {} kappa {} k {}: lakes {} rivers {}", p.hills, p.hill_m, p.shield_uplift, p.old_range_uplift, p.kappa, p.k, b.lakes.len(), b.rivers.len());
        println!("RELIEF  height m    (10 25 50 75 90 99 100 %): {:?}", q(&mut hs));
        println!("RELIEF  relief in 130 m                      : {:?}", q(&mut rel));
        println!("RELIEF  slope %                              : {:?}", q(&mut slope));
        // RELIEF_PNG: the face with the most land, its heights shaded from the north-west (sea blue, land by height)
        if let Ok(path) = std::env::var("RELIEF_PNG") {
            let m = g.m;
            let f = (0..6).max_by_key(|&f| (0..m * m).filter(|&k| b.land(g.idx(f, k % m, k / m) as usize)).count()).unwrap();
            let mut img = super::super::pictures::Img { w: m, h: m, px: vec![[0; 3]; m * m] };
            for (j, i) in (0..m).flat_map(|j| (0..m).map(move |i| (j, i))) {
                let h = |i: usize, j: usize| b.h[g.idx(f, i.min(m - 1), j.min(m - 1)) as usize];
                let v = h(i, j);
                let shade = (0.75 + (h(i + 1, j + 1) - v) * 0.06).clamp(0.25, 1.25);
                let t = (v / 150.0).clamp(0.0, 1.0);
                let c = if v <= 0.0 { [40.0, 70.0, 130.0] } else { [90.0 + 130.0 * t, 150.0 - 20.0 * t, 70.0 + 110.0 * t] };
                img.px[j * m + i] = c.map(|x| (x * if v <= 0.0 { 1.0 } else { shade }).clamp(0.0, 255.0) as u8);
            }
            img.save(&path).unwrap();
        }
    }

    /// The fast drainage (land masses and basins in parallel, radix heap and buckets) against the slow path
    /// it replaced, kept here as the reference: one binary-heap Priority-Flood and one serial sweep of the
    /// area. The same water level and the same area to the bit.
    /// The peaks' stretch (ADR 0105): the lowland to the bit, every height's order, the top `extra` higher.
    #[test]
    fn the_stretch_keeps_the_lowland_and_the_order_and_lifts_the_top() {
        let (top, extra) = (120.0, 230.0);
        // the stretch of the maps before hill country, and today's (from lower, eased in over more)
        for (from, ease) in [(STRETCH_FROM, STRETCH_EASE), Params::default().stretch] {
            let by = (from, ease);
            for v in [-60.0, 0.0, from * 0.3, from] {
                assert_eq!(stretch(v, top, extra, by).to_bits(), (v as f32).to_bits(), "{v} is lowland");
            }
            assert_eq!(stretch(top, top, extra, by), top + extra, "the highest");
            let mut last = from;
            for k in 1..=((top - from) * 100.0) as usize {
                let v = from + k as f32 * 0.01;
                let s = stretch(v, top, extra, by);
                assert!(s > last, "{v}: {s} not over {last}");
                assert!(k > 1 || s - last < 0.0101, "no step where the stretch begins");
                last = s;
            }
            assert_eq!(stretch(80.0, top, 0.0, by), 80.0, "no extra, no stretch");
        }
    }

    /// Hill country (ADR 0207): the land inside the continents stands higher than on the maps before it, the top
    /// and the sea's share the same.
    #[test]
    fn hill_country_raises_the_land_and_keeps_the_top_and_the_sea() {
        let quart = |b: &Bake| {
            let mut v: Vec<f32> = (0..b.g.n()).filter(|&c| b.land(c) && !b.lake(c)).map(|c| b.h[c]).collect();
            v.sort_by(f32::total_cmp);
            (v[v.len() / 2], v[v.len() * 9 / 10], v[v.len() - 1], v.len() as f32 / b.g.n() as f32)
        };
        let p = |p: Params| Params { volcanoes: false, ..p };
        let (new, old) = (quart(&bake(3072, 5, &p(Params::default()))), quart(&bake(3072, 5, &p(Params::before_hills()))));
        assert!(new.0 > 1.2 * old.0 && new.1 > 1.3 * old.1, "half and a tenth of the land stand over {:?}, before {:?}", new, old);
        assert!((new.2 - old.2).abs() < 0.01, "the highest land {} against {}", new.2, old.2);
        assert!((new.3 - old.3).abs() < 0.02, "the land's share {} against {}", new.3, old.3);
    }

    /// A plain wider than LAKE / EPS cells is no lake: the flood's epsilon must not add up to one there (it
    /// did, and the erosion then left such plains alone as lake floors). A pit is still a lake.
    #[test]
    fn a_wide_plain_is_no_lake_and_a_pit_is() {
        let g = Grid::new(512, 1000.0);
        let pit = |d: [f32; 3]| d[0] > 0.995;
        let h: Vec<f32> = (0..g.n())
            .map(|c| {
                let d = g.dir[c];
                if d[0] < -0.9 { -10.0 } else if pit(d) { 0.0 } else { 1.0 }
            })
            .collect();
        let d = drain(&g, &h, -5.0);
        let plain = (0..g.n()).filter(|&c| h[c] == 1.0 && d.lake(c, h[c])).count();
        let pits = (0..g.n()).filter(|&c| pit(g.dir[c])).count();
        let wet = (0..g.n()).filter(|&c| pit(g.dir[c]) && d.lake(c, h[c])).count();
        assert!(pits > 10 && wet == pits, "the pit: {wet} of {pits} cells are lake");
        assert_eq!(plain, 0, "plain cells taken for lake");
    }

    /// The buckets and the radix heap pop what a binary heap pops, for keys pushed as a flood pushes them:
    /// never under the last pop, often in the current bucket, sometimes far above it.
    #[test]
    fn the_monotone_queues_pop_in_heap_order() {
        let mut rng = Rng::new(3);
        let (mut b, mut r, mut heap) = (Buckets::new(), Radix::new(), BinaryHeap::new());
        let mut c = 0u64;
        let mut push = |x: u64, b: &mut Buckets, r: &mut Radix, heap: &mut BinaryHeap<Reverse<u64>>| {
            c += 1;
            let x = x << 32 | c;
            b.push(x);
            r.push(x);
            heap.push(Reverse(x));
        };
        for _ in 0..1000 {
            push(key(rng.f() as f32 * 50.0) as u64, &mut b, &mut r, &mut heap);
        }
        let mut popped = 0;
        while let Some(Reverse(x)) = heap.pop() {
            assert_eq!((b.pop(), r.pop()), (Some(x), Some(x)), "pop {popped}");
            popped += 1;
            let v = f32::from_bits((x >> 32) as u32 & 0x7fff_ffff); // key() of a positive height sets the top bit
            if popped < 200_000 {
                for k in 0..3 {
                    let up = [1e-4, 0.01 * rng.f() as f32, 5.0 * rng.f() as f32][k];
                    push(key(v + up) as u64, &mut b, &mut r, &mut heap);
                }
            }
        }
        assert_eq!((b.pop(), r.pop()), (None, None));
        assert!(popped > 200_000);
    }

    #[test]
    fn the_fast_drainage_matches_the_slow_path() {
        let g = Grid::new(48, 1000.0);
        let n = g.n();
        let h: Vec<f32> = (0..n)
            .map(|c| {
                let d = g.dir[c];
                let x = (c as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 40;
                40.0 * (3.0 * d[0]).sin() * (2.0 * d[1]).cos() + 25.0 * d[2] + (x % 1000) as f32 * 0.004
            })
            .collect();
        let sea = quantile(&h, 0.5);
        let d = drain(&g, &h, sea);

        let ocean = oceans(&g, &h, sea);
        let mut w: Vec<f32> = h.iter().zip(&ocean).map(|(&v, &o)| if o { sea } else { v }).collect();
        let mut done = ocean.clone();
        let mut q = BinaryHeap::new();
        for c in 0..n as u32 {
            if !done[c as usize] && g.around(c).any(|o| ocean[o as usize]) {
                done[c as usize] = true;
                q.push(Reverse((key(w[c as usize]), c)));
            }
        }
        let mut order = Vec::new();
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
        let mut area: Vec<f32> = (0..n as u32).map(|c| g.area(c)).collect();
        for &c in order.iter().rev() {
            let r = d.recv[c as usize];
            if r != NONE && !ocean[r as usize] {
                area[r as usize] += area[c as usize];
            }
        }
        assert!(order.len() > n / 4, "too little land to test: {}", order.len());
        assert!(d.start.len() > 2, "one basin only");
        // the order goes mass by mass now: the same cells, each after its receiver
        let mut a = d.order.clone();
        a.sort_unstable();
        let mut b = order.clone();
        b.sort_unstable();
        assert_eq!(a, b, "flooded cells");
        let mut at = vec![usize::MAX; n];
        d.order.iter().enumerate().for_each(|(i, &c)| at[c as usize] = i);
        assert!(d.order.iter().all(|&c| { let r = d.recv[c as usize] as usize; d.ocean[r] || at[r] < at[c as usize] }), "a receiver after its cell");
        assert!(d.w.iter().zip(&w).all(|(a, b)| a.to_bits() == b.to_bits()), "water level");
        assert!(d.area.iter().zip(&area).all(|(a, b)| a.to_bits() == b.to_bits()), "area");
    }
}
