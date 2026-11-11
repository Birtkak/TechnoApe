//! Stages 5 to 8 of `docs/design/worldgen.md` on a finished land bake: climate from the air model run
//! to a steady day (`prototypes/air`, ADR 0010), water on land (discharge down the drainage, wetlands),
//! soil, and the biome from a Whittaker table plus soil, height and distance to water.
use super::land::{at, noise, upsample, Bake};
use super::grid::{Grid, NONE};
use rayon::prelude::*;
use std::collections::VecDeque;
use std::time::Instant;

pub const ICE: u8 = 0;
pub const TUNDRA: u8 = 1;
pub const COLD_FOREST: u8 = 2;
pub const TEMPERATE_FOREST: u8 = 3;
pub const GRASSLAND: u8 = 4;
pub const DRY_SCRUB: u8 = 5;
pub const DESERT: u8 = 6;
pub const SAVANNA: u8 = 7;
pub const RAIN_FOREST: u8 = 8;
pub const WETLAND: u8 = 9;
pub const ALPINE: u8 = 10;
pub const BEACH: u8 = 11;
pub const SHELF: u8 = 12;
pub const DEEP: u8 = 13;
pub const REEF: u8 = 14;
pub const LAND_BIOMES: u8 = 12;

/// Names and map colours (the sheets' legend; the look in play is the developer's pick).
pub const BIOMES: [(&str, [u8; 3]); 15] = [
    ("ice", [240, 246, 252]),
    ("tundra", [160, 170, 150]),
    ("cold forest", [40, 92, 70]),
    ("temperate forest", [52, 128, 48]),
    ("grassland", [150, 190, 80]),
    ("dry scrub", [176, 162, 96]),
    ("desert", [232, 206, 140]),
    ("savanna", [204, 186, 70]),
    ("rain forest", [18, 92, 30]),
    ("wetland", [70, 130, 120]),
    ("alpine", [130, 110, 100]),
    ("beach", [250, 236, 180]),
    ("shelf", [50, 120, 180]),
    ("deep sea", [16, 40, 100]),
    ("warm reef", [40, 200, 200]),
];

pub const SAND: u8 = 0;
pub const LOAM: u8 = 1;
pub const CLAY: u8 = 2;
pub const GRAVEL: u8 = 3;
pub const PEAT: u8 = 4;
pub const ROCK: u8 = 5;
pub const SOILS: [&str; 6] = ["sand", "loam", "clay", "gravel", "peat", "rock"];

/// Every threshold of stages 6 to 8 (tuning; the biome list and looks are the developer's).
#[derive(Clone, Debug)]
pub struct LifeParams {
    /// air columns are about this many metres across (air.md: 128)
    pub air_cell_m: f64,
    /// the air model's step for the bake, and when its day counts as repeating
    pub air_dt: f32,
    pub air_days: (usize, usize),
    pub settle_k: f32,
    pub settle_rain: f32,
    /// mm a day that evaporates per C over 0 (what rain must beat to run off or wet the ground)
    pub pet_per_c: f32,
    /// a wetland: ground flatter than this, water at least this many m^3/s through it, a lake within
    /// `wet_lake_m` or a low coast within `wet_coast_m` (a marsh), and rain over this many times the
    /// evaporation
    pub wet_slope: f32,
    pub wet_flow: f32,
    pub wet_humid: f32,
    pub wet_lake_m: f32,
    pub wet_coast_m: f32,
    /// borders blend: the climate is jittered by noise this many metres across, this strong
    pub blend_m: f64,
    pub blend_t: f32,
    pub blend_rain: f32,
    /// biome temperature bounds, C: ice under, tundra under, cold under, warm from
    pub t_ice: f32,
    pub t_tundra: f32,
    pub t_cold: f32,
    pub t_warm: f32,
    /// rain bounds, mm a day: cold (tundra, grassland, forest), temperate (desert, scrub, grass, forest),
    /// warm (desert, scrub, savanna, rain forest)
    pub r_cold: [f32; 2],
    pub r_temp: [f32; 3],
    pub r_warm: [f32; 3],
    /// alpine over this height, metres; beach within this many metres of the sea and under this height
    pub alpine_m: f32,
    pub beach_m: f32,
    pub beach_h: f32,
    /// sea: deeper than this is deep sea; a reef is warmer than this, shallower than that
    pub shelf_m: f32,
    pub reef_t: f32,
    pub reef_m: f32,
}

impl Default for LifeParams {
    fn default() -> LifeParams {
        LifeParams {
            air_cell_m: 128.0,
            air_dt: 10.0,
            air_days: (15, 45),
            settle_k: 0.005,
            settle_rain: 0.01,
            pet_per_c: 0.02,
            wet_slope: 0.045,
            wet_flow: 0.003,
            wet_humid: 1.0,
            wet_lake_m: 40.0,
            wet_coast_m: 64.0,
            blend_m: 40.0,
            blend_t: 1.2,
            blend_rain: 0.25,
            t_ice: -13.0,
            t_tundra: -7.0,
            t_cold: 5.0,
            t_warm: 15.0,
            r_cold: [0.06, 0.12],
            r_temp: [0.08, 0.16, 0.32],
            r_warm: [0.12, 0.25, 0.7],
            alpine_m: 58.0,
            beach_m: 24.0,
            beach_h: 3.5,
            shelf_m: 22.0,
            reef_t: 21.0,
            reef_m: 12.0,
        }
    }
}

pub struct Life {
    /// mean air temperature at the ground, C; its day swing, K
    pub temp: Vec<f32>,
    pub swing: Vec<f32>,
    /// rain and snow, mm a day
    pub rain: Vec<f32>,
    /// water flowing out of the cell, m^3/s
    pub flow: Vec<f32>,
    pub wet: Vec<bool>,
    pub soil: Vec<u8>,
    pub biome: Vec<u8>,
    /// the air model: days to a repeating day, its last change, columns
    pub air_days: usize,
    pub air_change: (f32, f32),
    pub air_columns: usize,
    pub times: Vec<(&'static str, f64)>,
}

impl Life {
    pub fn hash(&self, mut h: u64) -> u64 {
        use super::land::fnv;
        for v in self.temp.iter().chain(&self.rain).chain(&self.flow) {
            h = fnv(h, &v.to_bits().to_le_bytes());
        }
        h = fnv(h, &self.soil);
        fnv(h, &self.biome)
    }
}

/// Metres per step from cells to the nearest source cell (breadth first over the 8 neighbours).
pub(super) fn distance(g: &Grid, source: impl Fn(usize) -> bool) -> Vec<f32> {
    let n = g.n();
    let mut d = vec![f32::INFINITY; n];
    let mut q = VecDeque::new();
    for c in 0..n {
        if source(c) {
            d[c] = 0.0;
            q.push_back(c as u32);
        }
    }
    while let Some(c) = q.pop_front() {
        for o in g.around(c) {
            let nd = d[c as usize] + g.dist(c, o);
            if nd < d[o as usize] {
                d[o as usize] = nd;
                q.push_back(o);
            }
        }
    }
    d
}

/// The ground's slope over about 24 m, m/m: the heights smoothed twice over the 8 neighbours, then the
/// mean drop to a neighbour. (The steepest drop between 8 m cells is the bake's fine detail: its median
/// on land is 0.47 and almost nothing is flat, measured.)
pub(super) fn slope(b: &Bake) -> Vec<f32> {
    let g = &b.g;
    let blur = |v: &Vec<f32>| -> Vec<f32> {
        (0..g.n() as u32)
            .into_par_iter()
            .map(|c| {
                let (mut s, mut k) = (v[c as usize], 1.0);
                for o in g.around(c) {
                    s += v[o as usize];
                    k += 1.0;
                }
                s / k
            })
            .collect()
    };
    let h0: Vec<f32> = b.h.iter().map(|v| v.max(0.0)).collect();
    let h = blur(&blur(&h0));
    (0..g.n() as u32)
        .into_par_iter()
        .map(|c| {
            let (mut s, mut k) = (0.0, 0.0);
            for o in g.around(c) {
                s += (h[c as usize] - h[o as usize]).abs() / g.dist(c, o);
                k += 1.0;
            }
            s / k
        })
        .collect()
}

pub fn life(b: &Bake, seed: u64, p: &LifeParams) -> Life {
    let mut times = Vec::new();
    let mut t = Instant::now();
    let mut lap = |name: &'static str, times: &mut Vec<(&'static str, f64)>| {
        times.push((name, t.elapsed().as_secs_f64()));
        t = Instant::now();
    };
    let g = &b.g;
    let n = g.n();
    let planet_n = g.m as f64 * 8.0;
    let ma = ((planet_n / p.air_cell_m).round() as usize).max(8);
    let ga = Grid::new(ma, g.r);
    // stage 5: the air columns' mean land height and water share, the model run to a repeating day
    let (mut sum, mut land, mut all) = (vec![0f64; ga.n()], vec![0u32; ga.n()], vec![0u32; ga.n()]);
    let col: Vec<u32> = (0..n as u32).into_par_iter().map(|c| ga.locate(g.d(c))).collect();
    for c in 0..n {
        let a = col[c] as usize;
        all[a] += 1;
        if b.land(c) && !b.lake(c) {
            land[a] += 1;
            sum[a] += b.h[c].max(0.0) as f64;
        }
    }
    let ground: Vec<f32> = (0..ga.n()).map(|a| if land[a] > 0 { (sum[a] / land[a] as f64) as f32 } else { 0.0 }).collect();
    let water: Vec<f32> = (0..ga.n()).map(|a| 1.0 - land[a] as f32 / all[a].max(1) as f32).collect();
    let mut air = super::air::Air::new(ma, g.r, ground, water, super::air::Params::default());
    let m = air.settle(p.air_dt, p.air_days.0, p.air_days.1, p.settle_k, p.settle_rain);
    lap("climate (air model)", &mut times);
    // to the fine cells: smooth, then the lapse by the cell's own height
    let theta = upsample(&ga, g, &m.theta);
    let rain = upsample(&ga, g, &m.rain);
    let swing = upsample(&ga, g, &m.swing);
    let lapse = super::air::Params::default().lapse;
    let temp: Vec<f32> = (0..n).map(|c| theta[c] - lapse * b.h[c].max(0.0)).collect();
    lap("climate to cells", &mut times);

    // stage 6: rain less evaporation runs down the receivers (an order upstream first)
    let pet = |c: usize| p.pet_per_c * temp[c].max(0.0);
    let mut indeg = vec![0u32; n];
    for c in 0..n {
        let r = b.recv[c];
        if r != NONE {
            indeg[r as usize] += 1;
        }
    }
    let mut q: Vec<u32> = (0..n as u32).filter(|&c| indeg[c as usize] == 0).collect();
    let mut flow = vec![0f64; n];
    let mut k = 0;
    while k < q.len() {
        let c = q[k] as usize;
        k += 1;
        if b.land(c) {
            // mm a day over the cell's area to m^3/s
            flow[c] += ((rain[c] - pet(c)).max(0.1 * rain[c]) as f64) * 1e-3 * g.area(c as u32) as f64 / 86400.0 * (86400.0 / super::air::DAY);
        }
        let r = b.recv[c];
        if r != NONE {
            flow[r as usize] += flow[c];
            indeg[r as usize] -= 1;
            if indeg[r as usize] == 0 {
                q.push(r);
            }
        }
    }
    let flow: Vec<f32> = flow.iter().map(|&v| v as f32).collect();
    let sl = slope(b);
    let lake: Vec<bool> = (0..n).map(|c| b.lake(c)).collect();
    let to_lake = distance(g, |c| lake[c]);
    let to_sea = distance(g, |c| b.ocean[c]);
    let by_lake: Vec<bool> = (0..n).map(|c| to_lake[c] <= p.wet_lake_m || (to_sea[c] <= p.wet_coast_m && b.h[c] < 2.0 * p.beach_h)).collect();
    let wet: Vec<bool> = (0..n)
        .into_par_iter()
        .map(|c| {
            b.land(c) && !lake[c] && temp[c] > p.t_tundra && sl[c] < p.wet_slope && (flow[c] > p.wet_flow || by_lake[c]) && rain[c] > p.wet_humid * pet(c)
        })
        .collect();
    if std::env::var("LIFEDBG").is_ok() {
        let l: Vec<usize> = (0..n).filter(|&c| b.land(c) && !lake[c]).collect();
        let share = |f: &dyn Fn(usize) -> bool| l.iter().filter(|&&c| f(c)).count() as f64 / l.len() as f64;
        eprintln!("land: warm {:.3} flat {:.3} flow {:.3} by lake {:.3} humid {:.3} flat+flow {:.3} flat+humid {:.3}",
            share(&|c| temp[c] > p.t_tundra), share(&|c| sl[c] < p.wet_slope), share(&|c| flow[c] > p.wet_flow), share(&|c| by_lake[c]),
            share(&|c| rain[c] > p.wet_humid * pet(c)), share(&|c| sl[c] < p.wet_slope && (flow[c] > p.wet_flow || by_lake[c])), share(&|c| sl[c] < p.wet_slope && rain[c] > p.wet_humid * pet(c)));
        let mut sv: Vec<f32> = l.iter().map(|&c| sl[c]).collect();
        sv.sort_by(f32::total_cmp);
        let mut fv: Vec<f32> = l.iter().map(|&c| flow[c]).collect();
        fv.sort_by(f32::total_cmp);
        let q = |v: &Vec<f32>, f: f64| v[((v.len() - 1) as f64 * f) as usize];
        for tb in (-20..30).step_by(3) {
            let mut r: Vec<f32> = l.iter().filter(|&&c| (tb as f32..tb as f32 + 3.0).contains(&temp[c])).map(|&c| rain[c]).collect();
            if r.is_empty() { continue; }
            r.sort_by(f32::total_cmp);
            let q = |f: f64| r[((r.len() - 1) as f64 * f) as usize];
            eprintln!("T {tb:>3}..{:>3}: land {:.3} rain p10 {:.3} p50 {:.3} p90 {:.3}", tb + 3, r.len() as f64 / l.len() as f64, q(0.1), q(0.5), q(0.9));
        }
        let wetc: Vec<usize> = l.iter().copied().filter(|&c| temp[c] > p.t_tundra && sl[c] < p.wet_slope && (flow[c] > p.wet_flow || by_lake[c])).collect();
        eprintln!("wet but for humidity: {:.3}, of which humid {:.3}", wetc.len() as f64 / l.len() as f64, wetc.iter().filter(|&&c| rain[c] > p.wet_humid * pet(c)).count() as f64 / wetc.len().max(1) as f64);
        eprintln!("slope p10 {:.3} p50 {:.3} p90 {:.3}; flow p50 {:.4} p90 {:.4} p99 {:.3}", q(&sv, 0.1), q(&sv, 0.5), q(&sv, 0.9), q(&fv, 0.5), q(&fv, 0.9), q(&fv, 0.99));
    }
    lap("water on land", &mut times);

    // stage 7: soil
    let to_land = distance(g, |c| b.land(c));
    let soil: Vec<u8> = (0..n)
        .into_par_iter()
        .map(|c| {
            if !b.land(c) {
                return if -b.h[c] < p.shelf_m && to_land[c] < 400.0 { SAND } else { CLAY };
            }
            let arid = rain[c] < 0.5 * pet(c) || rain[c] < 0.1;
            if sl[c] > 0.6 {
                ROCK
            } else if sl[c] > 0.35 || (b.h[c] > p.alpine_m && sl[c] > 0.2) {
                GRAVEL
            } else if to_sea[c] <= p.beach_m && b.h[c] < p.beach_h {
                SAND
            } else if wet[c] {
                if temp[c] < 8.0 { PEAT } else { CLAY }
            } else if lake[c] || (flow[c] > 5.0 * p.wet_flow && sl[c] < 0.02) {
                CLAY
            } else if arid {
                SAND
            } else {
                LOAM
            }
        })
        .collect();
    if std::env::var("LIFEDBG").is_ok() {
        let l: Vec<usize> = (0..n).filter(|&c| b.land(c) && !lake[c]).collect();
        let k: Vec<String> = (0..6).map(|s| format!("{} {:.3}", SOILS[s], l.iter().filter(|&&c| soil[c] == s as u8).count() as f64 / l.len() as f64)).collect();
        eprintln!("soil on land: {}", k.join(", "));
    }
    lap("soil", &mut times);

    // stage 8: the biome, the climate jittered so borders interfinger over tens of metres
    let nt = noise(seed as i32 ^ 0x51ed, 2);
    let nr = noise(seed as i32 ^ 0x7a11, 2);
    let f = g.r / p.blend_m;
    let biome: Vec<u8> = (0..n)
        .into_par_iter()
        .map(|c| {
            let d = g.d(c as u32);
            let tt = temp[c] + p.blend_t * at(&nt, d, f);
            if b.ocean[c] {
                let depth = -b.h[c];
                return if depth > p.shelf_m {
                    DEEP
                } else if tt > p.reef_t && depth < p.reef_m && soil[c] == SAND {
                    REEF
                } else {
                    SHELF
                };
            }
            let rr = rain[c] * (p.blend_rain * at(&nr, d, f)).exp();
            let h = b.h[c];
            if tt < p.t_ice {
                ICE
            } else if h > p.alpine_m {
                ALPINE
            } else if tt < p.t_tundra {
                TUNDRA
            } else if soil[c] == SAND && to_sea[c] <= p.beach_m && h < p.beach_h {
                BEACH
            } else if wet[c] {
                WETLAND
            } else if tt < p.t_cold {
                // cold and dry: tundra under 0 C, a cold desert over it (Whittaker)
                if rr < p.r_cold[0] { if tt < 0.0 { TUNDRA } else { DESERT } } else if rr < p.r_cold[1] { GRASSLAND } else { COLD_FOREST }
            } else if tt < p.t_warm {
                let r = p.r_temp;
                if rr < r[0] { DESERT } else if rr < r[1] { DRY_SCRUB } else if rr < r[2] { GRASSLAND } else { TEMPERATE_FOREST }
            } else {
                let r = p.r_warm;
                if rr < r[0] { DESERT } else if rr < r[1] { DRY_SCRUB } else if rr < r[2] { SAVANNA } else { RAIN_FOREST }
            }
        })
        .collect();
    lap("biomes", &mut times);
    Life { temp, swing, rain, flow, wet, soil, biome, air_days: m.days, air_change: m.last_change, air_columns: ga.n(), times }
}

/// Each biome's share: of the land (lakes left out) for the land biomes, of the sea for the sea's.
pub fn shares(b: &Bake, l: &Life) -> [f64; 15] {
    let mut a = [0f64; 15];
    let (mut land, mut sea) = (0f64, 0f64);
    for c in 0..b.g.n() {
        if b.lake(c) {
            continue;
        }
        let ar = b.g.area(c as u32) as f64;
        a[l.biome[c] as usize] += ar;
        if b.ocean[c] { sea += ar } else { land += ar }
    }
    for (k, v) in a.iter_mut().enumerate() {
        *v /= if k < LAND_BIOMES as usize { land } else { sea };
    }
    a
}
