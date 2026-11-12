//! River lines and beds (worldgen-plan W2, ADR 0056): the bake's river cells joined centre to centre into
//! lines, and the bed each line cuts into the ground. A bed only falls from spring to sea: its banks are the
//! lowest ground upstream of each cell, its depth and width grow with the area drained. The beds are dry
//! until the rivers hold water (W5).
use super::grid::{AX, NONE};
use super::Map;
use crate::world::fold;
use glam::DVec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::f64::consts::FRAC_PI_2;

/// A river cell of the bake (worldgen.md stage 4): it drains `area` m^2 into cell `to` (the next river cell,
/// or a cell of the sea).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Reach {
    pub cell: u32,
    pub to: u32,
    pub area: f32,
    /// The mean flow out of the cell, m^3/s (the bake's rain less what evaporates, gathered downstream; since
    /// version 3, 0 before).
    pub flow: f32,
}

/// A map as version 2 wrote it (rivers without their flow, no lakes: a lake's cells were flats at its level).
#[derive(Serialize, Deserialize)]
pub(super) struct V2 {
    pub version: u32,
    pub seed: i32,
    pub n: i32,
    pub m: u32,
    pub h: Vec<f32>,
    pub biome: Vec<u8>,
    pub spawn: [f64; 3],
    pub rivers: Vec<(u32, u32, f32)>,
}

/// A map as version 1 wrote it (no rivers).
#[derive(Deserialize)]
pub(super) struct V1 {
    pub version: u32,
    pub seed: i32,
    pub n: i32,
    pub m: u32,
    pub h: Vec<f32>,
    pub biome: Vec<u8>,
    pub spawn: [f64; 3],
}

/// The drained area of the smallest river, m^2 (`land::Params::river_area_m2`).
const A0: f32 = 1.5e5;
/// How far past its half width a bed's bank cuts the ground, metres, at a slope of 1.
pub const BANK: f32 = 6.0;

/// Half the width of a river that drains `area` m^2: 1 m (a 2 m stream) to 6 m.
pub fn half_width(area: f32) -> f32 {
    (area / A0).powf(0.4).clamp(1.0, 6.0)
}
/// Its depth under the banks: 1 to 3 m.
pub fn depth(area: f32) -> f32 {
    0.6 + 0.4 * half_width(area)
}

/// One line from a river cell's centre to the next cell's: points in metres from the planet's centre, and
/// at each end the bed (metres over the sea), the depth and the half width.
#[derive(Clone, Copy, Debug)]
pub struct Seg {
    pub a: DVec3,
    pub b: DVec3,
    pub bed: [f32; 2],
    pub depth: [f32; 2],
    pub hw: [f32; 2],
    /// The segment downstream (NONE: this one reaches the sea).
    pub next: u32,
    /// Water at rest (W5, ADR 0059): its level at each end, metres over the sea (never falling downstream,
    /// never under a lake or the sea it runs into); its depth over the bed as the rain gives it (0: a dry bed);
    /// its flow, m^3/s; the reach it belongs to (`Lines::reaches`); and whether it lies in a lake.
    pub wl: [f32; 2],
    pub fill: [f32; 2],
    pub q: [f32; 2],
    pub reach: u32,
    pub lake: bool,
}

/// A reach (rivers-and-tides.md): the segments from a spring, a confluence or a lake's outlet to the next, or
/// a lake.
#[derive(Clone, Debug, Default)]
pub struct ReachDef {
    /// Its segments, upstream first (none for a lake).
    pub segs: Vec<u32>,
    /// The reach it flows into (NONE: the sea, or nowhere for a lake no river leaves).
    pub down: u32,
    pub lake: bool,
    /// How long it holds water it takes in, seconds: a river's length over its speed at rest, a lake's
    /// LAKE_HOLD (a linear store, `liquid::Ground::down`).
    pub hold: f32,
}

/// A lake holds what it takes in this long (seconds): far longer than a river, it is wide and still.
const LAKE_HOLD: f32 = 600.0;

/// A river's speed at rest is the Manning formula's with this roughness, clamped (m/s).
const MANNING: f32 = 0.035;
const V_MIN: f32 = 0.3;
const V_MAX: f32 = 2.0;
/// A river's water at rest fills its bed by this share at the median runoff per square metre of catchment,
/// by its runoff's ratio to that to this power, to at most FILL_MAX; under FILL_MIN metres it is a dry bed.
const FILL: f32 = 0.7;
const FILL_POW: f32 = 0.3;
const FILL_MAX: f32 = 0.85;
const FILL_MIN: f32 = 0.15;

impl Seg {
    /// Where along it p lies nearest (0 at a, 1 at b), and how far from it, metres.
    fn nearest(&self, p: DVec3) -> (f32, f32) {
        let ab = self.b - self.a;
        let t = ((p - self.a).dot(ab) / ab.length_squared().max(1e-9)).clamp(0.0, 1.0);
        (t as f32, (p - (self.a + ab * t)).length() as f32)
    }
}

/// The cut a river makes at a point: the ground goes no higher than `cut`, and inside a channel no lower
/// than `floor` (its bed, so the line itself is the bed to the bit).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cut {
    pub cut: f32,
    pub floor: f32,
}

#[derive(Default)]
pub struct Lines {
    pub segs: Vec<Seg>,
    /// The reaches: the rivers' first, then one per lake from `lake_reach`.
    pub reaches: Vec<ReachDef>,
    /// The lake of each lake cell, and each lake's level (metres over the sea).
    pub lake_of: crate::world::FxMap<u32, u32>,
    pub lake_level: Vec<f32>,
    pub lake_reach: u32,
    /// A bit per cell: some segment's cut may reach into it.
    near: Vec<u64>,
    /// The cells where generated water may stand (W5): a wet channel's within its half width, a lake's and the
    /// ring around it. `Map::water` is None quickly elsewhere.
    wet: Vec<u64>,
    /// (cell, segment) for every segment whose cut may reach into a cell, sorted.
    at: Vec<(u32, u32)>,
}

/// Direction of face point (x, y) of a map of m cells per edge (`Grid::dir_at`).
fn dir(m: u32, f: usize, x: f64, y: f64) -> DVec3 {
    let (h, th) = (m as f64 * 0.5, FRAC_PI_2 / m as f64);
    let [n, s, t] = AX[f].map(DVec3::from_array);
    (n + s * ((x - h) * th).tan() + t * ((y - h) * th).tan()).normalize()
}
/// The face of a direction and its cell coordinates there (`Grid::locate_xy`).
fn locate(m: u32, d: DVec3) -> (usize, f64, f64) {
    let a = d.abs();
    let f = if a.x >= a.y && a.x >= a.z { (d.x < 0.0) as usize } else if a.y >= a.z { 2 + (d.y < 0.0) as usize } else { 4 + (d.z < 0.0) as usize };
    let [n, s, t] = AX[f].map(DVec3::from_array);
    let dn = d.dot(n);
    let col = |w: f64| ((w / dn).atan() / FRAC_PI_2 * 2.0 + 1.0) * 0.5 * m as f64;
    (f, col(d.dot(s)), col(d.dot(t)))
}

impl Lines {
    pub fn new(map: &Map) -> Lines {
        let m = map.m;
        let r = map.radius();
        // a cell's centre is a corner of the columns under it: the line goes through the centre of the column
        // just past it instead, so a reach along a row or a diagonal of columns crosses their centres in order
        // (and the walk's column tops fall with the bed)
        let off = 0.5 + 0.5 * m as f64 / map.n as f64;
        let centre = |c: u32| {
            let (f, i, j) = (c as usize / (m * m) as usize, c % m, (c / m) % m);
            dir(m, f, i as f64 + off, j as f64 + off) * r
        };
        let node: HashMap<u32, usize> = map.rivers.iter().enumerate().map(|(k, v)| (v.cell, k)).collect();
        // the lakes: connected cells of the map's lake list, each its own id
        let level: HashMap<u32, f32> = map.lakes.iter().copied().collect();
        let (mut lake_of, mut lake_level) = (crate::world::FxMap::default(), Vec::new());
        for &(c0, l0) in &map.lakes {
            if lake_of.contains_key(&c0) {
                continue;
            }
            let id = lake_level.len() as u32;
            lake_level.push(l0);
            let mut todo = vec![c0];
            lake_of.insert(c0, id);
            while let Some(c) = todo.pop() {
                let (f, i, j) = (c as usize / (m * m) as usize, (c % m) as i32, ((c / m) % m) as i32);
                for (di, dj) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let (f, i, j) = fold(f, i + di, j + dj, m as i32);
                    let d = (f as u32 * m + j as u32) * m + i as u32;
                    if level.contains_key(&d) && !lake_of.contains_key(&d) {
                        lake_of.insert(d, id);
                        todo.push(d);
                    }
                }
            }
        }
        // a cell's surface: a lake's level, or its ground
        let surf = |c: u32| level.get(&c).copied().unwrap_or(map.h[c as usize]);
        // the banks: the lowest ground at or upstream of each river cell (downstream first, so upstream first
        // reversed: every donor is done before its receiver)
        let mut s: Vec<f32> = map.rivers.iter().map(|v| surf(v.cell)).collect();
        for k in (0..map.rivers.len()).rev() {
            if let Some(&j) = node.get(&map.rivers[k].to) {
                if j < k {
                    s[j] = s[j].min(s[k]);
                }
            }
        }
        let segs: Vec<Seg> = map
            .rivers
            .iter()
            .enumerate()
            .map(|(k, v)| {
                let (d0, w0) = (depth(v.area), half_width(v.area));
                let (bank, area, next) = match node.get(&v.to) {
                    // (a receiver later in the list would be a cycle: end the line there)
                    Some(&j) if j < k => (s[j], map.rivers[j].area, j as u32),
                    _ => (s[k].min(surf(v.to)), v.area, NONE),
                };
                let (d1, w1) = (depth(area), half_width(area));
                let lake = level.contains_key(&v.cell);
                Seg { a: centre(v.cell), b: centre(v.to), bed: [s[k] - d0, bank - d1], depth: [d0, d1], hw: [w0, w1], next, wl: [0.0; 2], fill: [0.0; 2], q: [0.0; 2], reach: NONE, lake }
            })
            .collect();
        let mut segs = segs;
        water(map, &mut segs, &level);
        let (reaches, lake_reach) = reaches(map, &mut segs, &lake_of, lake_level.len());
        // where each segment's cut may reach: the cells around three points along it
        let cell = r * FRAC_PI_2 / m as f64;
        let mut at = Vec::new();
        for (k, g) in segs.iter().enumerate() {
            let reach = (g.hw[0].max(g.hw[1]) + BANK) as f64 + 0.25 * (g.b - g.a).length();
            // equi-angular cells are at least 0.7 of the nominal edge; +1 for where in its cell a point lies
            let kc = (reach / (0.7 * cell)).ceil() as i32 + 1;
            for t in [0.0, 0.5, 1.0] {
                let (f, x, y) = locate(m, (g.a + (g.b - g.a) * t).normalize());
                let (i0, j0) = ((x.floor() as i32).clamp(0, m as i32 - 1), (y.floor() as i32).clamp(0, m as i32 - 1));
                for dj in -kc..=kc {
                    for di in -kc..=kc {
                        let (f, i, j) = fold(f, i0 + di, j0 + dj, m as i32);
                        at.push(((f as u32 * m + j as u32) * m + i as u32, k as u32));
                    }
                }
            }
        }
        at.sort_unstable();
        at.dedup();
        let mut near = vec![0u64; (6 * m as usize * m as usize).div_ceil(64)];
        for &(c, _) in &at {
            near[c as usize / 64] |= 1 << (c % 64);
        }
        let mut wet = vec![0u64; near.len()];
        let mut mark = |f: usize, i: i32, j: i32| {
            let (f, i, j) = fold(f, i, j, m as i32);
            let c = (f as u32 * m + j as u32) * m + i as u32;
            wet[c as usize / 64] |= 1 << (c % 64);
        };
        for g in segs.iter().filter(|g| !g.lake && g.wl.iter().zip(&g.bed).any(|(w, b)| w > b)) {
            // samples a quarter cell apart: a point of the channel lies within its half width and an eighth of
            // a cell of one, and cells are at least 0.7 of the nominal edge
            let kc = ((g.hw[0].max(g.hw[1]) as f64 + 0.15 * cell) / (0.7 * cell)).ceil() as i32;
            let steps = ((g.b - g.a).length() / (0.25 * cell)).ceil().max(1.0) as usize;
            for s in 0..=steps {
                let (f, x, y) = locate(m, (g.a + (g.b - g.a) * (s as f64 / steps as f64)).normalize());
                let (i0, j0) = ((x.floor() as i32).clamp(0, m as i32 - 1), (y.floor() as i32).clamp(0, m as i32 - 1));
                for dj in -kc..=kc {
                    for di in -kc..=kc {
                        mark(f, i0 + di, j0 + dj);
                    }
                }
            }
        }
        for &c in lake_of.keys() {
            let (f, i, j) = (c as usize / (m * m) as usize, (c % m) as i32, ((c / m) % m) as i32);
            for (di, dj) in (-1..=1).flat_map(|a| (-1..=1).map(move |b| (a, b))) {
                mark(f, i + di, j + dj);
            }
        }
        Lines { segs, near, wet, at, reaches, lake_of, lake_level, lake_reach }
    }
    /// The segments whose cut may reach into map cell c.
    pub fn around(&self, c: u32) -> &[(u32, u32)] {
        if self.near.get(c as usize / 64).is_none_or(|w| w & (1 << (c % 64)) == 0) {
            return &[];
        }
        let from = self.at.partition_point(|e| e.0 < c);
        let to = from + self.at[from..].partition_point(|e| e.0 == c);
        &self.at[from..to]
    }
    /// The cut at p (metres from the centre, in map cell c) of the segments `segs`, leaving out rivers
    /// narrower than `col_m` (a far chunk's column would only alias them).
    pub fn cut_of(&self, segs: impl Iterator<Item = u32>, p: DVec3, col_m: f32) -> Option<Cut> {
        let mut cut = f32::INFINITY;
        let (mut best, mut floor) = (f32::INFINITY, f32::NEG_INFINITY);
        for k in segs {
            let g = &self.segs[k as usize];
            if g.lake {
                continue;
            }
            let (t, d) = g.nearest(p);
            let hw = g.hw[0] + (g.hw[1] - g.hw[0]) * t;
            if 2.0 * hw < col_m || d >= hw + BANK {
                continue;
            }
            let bed = g.bed[0] + (g.bed[1] - g.bed[0]) * t;
            let deep = g.depth[0] + (g.depth[1] - g.depth[0]) * t;
            // a flat bottom (wider than a column's half diagonal), a rounded side up to the bank, the bank
            let flat = 0.8 * hw;
            let z = if d <= flat {
                bed
            } else if d < hw {
                bed + deep * ((d - flat) / (hw - flat)).powi(2)
            } else {
                bed + deep + (d - hw)
            };
            cut = cut.min(z);
            if d < best {
                best = d;
                floor = if d < hw { bed } else { f32::NEG_INFINITY };
            }
        }
        cut.is_finite().then_some(Cut { cut, floor })
    }
}

/// Each segment's water at rest: its depth from the runoff of its catchment, its flow, and its level, from the
/// sea up (a river never stands under the lake or the sea it runs into, nor falls downstream).
fn water(map: &Map, segs: &mut [Seg], level: &HashMap<u32, f32>) {
    let node: HashMap<u32, usize> = map.rivers.iter().enumerate().map(|(k, v)| (v.cell, k)).collect();
    let mut per: Vec<f32> = map.rivers.iter().filter(|r| r.flow > 0.0).map(|r| r.flow / r.area).collect();
    per.sort_by(f32::total_cmp);
    let mid = per.get(per.len() / 2).copied().unwrap_or(0.0);
    let fill = |r: &Reach, depth: f32| {
        let f = if mid > 0.0 { depth * (FILL * (r.flow / r.area / mid).powf(FILL_POW)).min(FILL_MAX) } else { 0.0 };
        if f < FILL_MIN { 0.0 } else { f }
    };
    for k in 0..segs.len() {
        let v = &map.rivers[k];
        let end = node.get(&v.to).map_or(v, |&j| &map.rivers[j]);
        let g = &mut segs[k];
        g.fill = [fill(v, g.depth[0]), fill(end, g.depth[1])];
        let len = (g.b - g.a).length().max(1e-3) as f32;
        let slope = ((g.bed[0] - g.bed[1]) / len).max(0.0);
        let q = |f: f32, hw: f32| (f.powf(2.0 / 3.0) * slope.sqrt() / MANNING).clamp(V_MIN, V_MAX) * 2.0 * hw * f;
        g.q = [q(g.fill[0], g.hw[0]), q(g.fill[1], g.hw[1])];
    }
    // the list is downstream first: each segment's next has its level already
    for k in 0..segs.len() {
        let v = &map.rivers[k];
        if segs[k].lake {
            let l = level[&v.cell];
            segs[k].wl = [l, l];
            continue;
        }
        let below = match segs[k].next {
            NONE => level.get(&v.to).copied().unwrap_or(0.0),
            j => segs[j as usize].wl[0],
        };
        let g = &mut segs[k];
        g.wl[1] = (g.bed[1] + g.fill[1]).max(below);
        g.wl[0] = (g.bed[0] + g.fill[0]).max(g.wl[1]);
    }
}

/// The reach graph: a river reach begins at a spring, a confluence or a lake's outlet; each lake is one reach,
/// flowing out where a segment of its own leaves it.
fn reaches(map: &Map, segs: &mut [Seg], lake_of: &crate::world::FxMap<u32, u32>, lakes: usize) -> (Vec<ReachDef>, u32) {
    let mut fed = vec![0u32; segs.len()];
    let mut from_lake = vec![false; segs.len()];
    for g in segs.iter() {
        if g.next != NONE {
            fed[g.next as usize] += 1;
            from_lake[g.next as usize] |= g.lake;
        }
    }
    let mut out: Vec<ReachDef> = Vec::new();
    let starts: Vec<usize> = (0..segs.len()).filter(|&k| !segs[k].lake && (fed[k] != 1 || from_lake[k])).collect();
    for k0 in starts {
        let id = out.len() as u32;
        let mut r = ReachDef { segs: Vec::new(), down: NONE, lake: false, hold: 0.0 };
        let mut k = k0;
        loop {
            segs[k].reach = id;
            r.segs.push(k as u32);
            let n = segs[k].next;
            if n == NONE || segs[n as usize].lake || fed[n as usize] != 1 {
                break;
            }
            k = n as usize;
        }
        out.push(r);
    }
    let lake_reach = out.len() as u32;
    out.extend((0..lakes).map(|_| ReachDef { segs: Vec::new(), down: NONE, lake: true, hold: 0.0 }));
    for k in 0..segs.len() {
        if segs[k].lake {
            let id = lake_reach + lake_of[&map.rivers[k].cell];
            segs[k].reach = id;
            out[id as usize].segs.push(k as u32);
        }
    }
    for r in out.iter_mut() {
        r.hold = if r.lake {
            LAKE_HOLD
        } else {
            let t: f32 = r.segs.iter().map(|&k| {
                let g = &segs[k as usize];
                let v = if g.fill[0] > 0.0 { g.q[0] / (2.0 * g.hw[0] * g.fill[0]) } else { V_MIN };
                (g.b - g.a).length() as f32 / v.max(V_MIN)
            }).sum();
            t.max(1.0)
        };
    }
    for r in 0..out.len() {
        let last = *out[r].segs.last().unwrap_or(&NONE);
        out[r].down = if out[r].lake {
            // the first of its segments whose next is not its own
            out[r].segs.iter().map(|&k| segs[k as usize].next).find(|&n| n != NONE && segs[n as usize].reach != r as u32).map_or(NONE, |n| segs[n as usize].reach)
        } else {
            match segs[last as usize].next {
                NONE => NONE,
                n => segs[n as usize].reach,
            }
        };
    }
    (out, lake_reach)
}

/// Generated water over a point (W5, ADR 0059): a river's or a lake's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Water {
    /// Its level, metres over the sea.
    pub level: f32,
    /// Its reach (`Lines::reaches`).
    pub reach: u32,
    /// A river's flow per metre of its width, m^2/s; its surface's fall, m a metre; and how much its flow per
    /// metre of width grows a metre downstream (the runoff it gathers), m/s. All 0 on a lake.
    pub q: f32,
    pub fall: f32,
    pub gain: f32,
}

impl Map {
    /// The rivers' and lakes' water over unit direction d, at face point (x, y) in cells: a river's inside its
    /// channel (where its level stands over its bed), a lake's over its cells and the cells next to them. The
    /// higher of the two where both are; None on a map of version 2 or before (dry beds, lakes as flats).
    /// Whether a river line or a lake lies over or next to map cells x0..=x1, y0..=y1 of a face (folded past
    /// its edges): a chunk with none skips `water`'s lookups.
    pub fn wet_near(&self, face: usize, x0: i32, y0: i32, x1: i32, y1: i32) -> bool {
        if self.version < 3 {
            return false;
        }
        let m = self.m as i32;
        (y0..=y1).any(|y| {
            (x0..=x1).any(|x| {
                let (f, i, j) = fold(face, x, y, m);
                let c = (f as u32 * self.m + j as u32) * self.m + i as u32;
                !self.lines.around(c).is_empty() || self.lines.lake_of.contains_key(&c)
            })
        })
    }
        pub fn water(&self, face: usize, x: f64, y: f64, d: DVec3) -> Option<Water> {
        let m = self.m as i32;
        let (f, i, j) = fold(face, (x.floor() as i32).clamp(0, m - 1), (y.floor() as i32).clamp(0, m - 1), m);
        let c = (f as u32 * self.m + j as u32) * self.m + i as u32;
        if self.lines.wet.get(c as usize / 64).is_none_or(|w| w & (1 << (c % 64)) == 0) {
            return None;
        }
        self.water_among(face, x, y, d, self.lines.around(c).iter().map(|e| e.1))
    }
    /// `water`, with the river segments to try given.
    fn water_among(&self, face: usize, x: f64, y: f64, d: DVec3, segs: impl Iterator<Item = u32>) -> Option<Water> {
        if self.version < 3 {
            return None;
        }
        let m = self.m as i32;
        let mut best: Option<Water> = None;
        let mut take = |w: Water| {
            if best.is_none_or(|b| w.level > b.level) {
                best = Some(w);
            }
        };
        // a lake: the cells whose centres are around the point
        if !self.lines.lake_of.is_empty() {
            let (i0, j0) = ((x - 0.5).floor() as i32, (y - 0.5).floor() as i32);
            for (di, dj) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let (f, i, j) = fold(face, i0 + di, j0 + dj, m);
                if let Some(&l) = self.lines.lake_of.get(&((f as u32 * self.m + j as u32) * self.m + i as u32)) {
                    take(Water { level: self.lines.lake_level[l as usize], reach: self.lines.lake_reach + l, q: 0.0, fall: 0.0, gain: 0.0 });
                }
            }
        }
        // a river: the nearest line whose channel holds the point
        let p = d * self.radius();
        let mut near: Option<(f32, Water)> = None;
        for k in segs {
            let g = &self.lines.segs[k as usize];
            if g.lake {
                continue;
            }
            let (t, dist) = g.nearest(p);
            let at = |v: [f32; 2]| v[0] + (v[1] - v[0]) * t;
            let (hw, wl) = (at(g.hw), at(g.wl));
            if dist >= hw || wl <= at(g.bed) || near.is_some_and(|n| n.0 <= dist) {
                continue;
            }
            let len = (g.b - g.a).length().max(1e-3) as f32;
            let w = 2.0 * hw;
            near = Some((dist, Water { level: wl, reach: g.reach, q: at(g.q) / w, fall: (g.wl[0] - g.wl[1]) / len, gain: (g.q[1] - g.q[0]) / len / w }));
        }
        if let Some((_, w)) = near {
            take(w);
        }
        best
    }
    /// The planet's radius in metres (ADR 0003: n columns of 1 m per face edge).
    pub fn radius(&self) -> f64 {
        self.n as f64 / (2.0 * std::f64::consts::PI / 3.0).sqrt()
    }
    /// The rivers' cut at unit direction d, which lies at face point (x, y) in cells, for columns `col_m`
    /// metres wide; None: no river near.
    pub fn river(&self, face: usize, x: f64, y: f64, d: DVec3, col_m: f32) -> Option<Cut> {
        let m = self.m as i32;
        let (f, i, j) = fold(face, (x.floor() as i32).clamp(0, m - 1), (y.floor() as i32).clamp(0, m - 1), m);
        let segs = self.lines.around((f as u32 * self.m + j as u32) * self.m + i as u32);
        if segs.is_empty() {
            return None;
        }
        self.lines.cut_of(segs.iter().map(|e| e.1), d * self.radius(), col_m)
    }
    /// The river lines from each spring (a river cell nothing drains into) to the sea, as segment indices.
    pub fn springs(&self) -> Vec<u32> {
        let mut fed = vec![false; self.lines.segs.len()];
        for g in &self.lines.segs {
            if g.next != NONE {
                fed[g.next as usize] = true;
            }
        }
        (0..fed.len() as u32).filter(|&k| !fed[k as usize]).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::World;
    use std::sync::{Arc, LazyLock};

    /// The 1 km planet (Luna's size from W6) with smaller rivers counted, as the prototype's test has them.
    static MAP: LazyLock<Arc<Map>> = LazyLock::new(|| Arc::new(Map::bake_with(2, 1536, &crate::bake::land::Params { river_area_m2: 2e4, ..Default::default() })));

    /// Two diagonal reaches that cross (a 2 x 2 of cells draining crosswise) would give two beds where they
    /// meet. Steepest descent cannot make one (each diagonal needs the other's end lower); a flat's flood
    /// order could.
    fn crossings(map: &Map) -> usize {
        let m = map.m;
        let cells: HashMap<u32, u32> = map.rivers.iter().map(|r| (r.cell, r.to)).collect();
        map.rivers
            .iter()
            .filter(|r| {
                let (i, j, ti, tj) = (r.cell % m, (r.cell / m) % m, r.to % m, (r.to / m) % m);
                r.to / (m * m) == r.cell / (m * m) && ti.abs_diff(i) == 1 && tj.abs_diff(j) == 1 && {
                    let (a, b) = (r.cell - i + ti, r.cell - j * m + tj * m);
                    cells.get(&a) == Some(&b) || cells.get(&b) == Some(&a)
                }
            })
            .count()
    }

    #[test]
    fn the_lines_run_downstream_and_the_index_finds_every_cut() {
        let map = &*MAP;
        let segs = &map.lines.segs;
        assert!(map.springs().len() >= 10, "{} springs, {} segments", map.springs().len(), segs.len());
        for (k, g) in segs.iter().enumerate() {
            assert!(g.bed[1] <= g.bed[0], "segment {k} climbs: {:?}", g.bed);
            if g.next != NONE {
                assert!((g.next as usize) < k, "downstream first");
                assert_eq!(g.bed[1], segs[g.next as usize].bed[0], "segment {k} meets the next at its bed");
            }
        }
        assert_eq!(crossings(map), 0, "crossing reaches");
        // the index against every segment, at points scattered around the lines (the slow path is the reference)
        let mut rng = crate::bake::land::Rng::new(9);
        let (mut cut, mut tried) = (0, 0);
        for g in segs.iter().step_by(3) {
            for _ in 0..8 {
                let p = (g.a + (g.b - g.a) * rng.f() + rng.dir() * rng.range(0.0, 16.0)).normalize();
                let (f, x, y) = locate(map.m, p);
                let fast = map.river(f, x, y, p, 1.0);
                let slow = map.lines.cut_of(0..segs.len() as u32, p * map.radius(), 1.0);
                assert_eq!(fast, slow, "at {p}");
                cut += fast.is_some() as usize;
                tried += 1;
            }
        }
        assert!(cut > tried / 4, "{cut} of {tried} points cut");
        // the rivers survive the round trip, and a map of version 1 still reads (with none)
        let back = Map::from_bytes(&map.to_bytes()).unwrap();
        assert_eq!((back.rivers.len(), back.lines.segs.len()), (map.rivers.len(), segs.len()));
        let v1 = postcard::to_stdvec(&(1u32, map.seed, map.n, map.m, &map.h, &map.biome, map.spawn)).unwrap();
        let old = Map::from_bytes(&v1).unwrap();
        assert_eq!((old.version, old.rivers.len()), (1, 0));
    }

    /// The walk test (worldgen-plan W2): along every river from its spring, the ground of the game's own
    /// height function never climbs, nor the top block of the columns the river crosses, and the river ends
    /// in the sea.
    #[test]
    fn every_river_walks_downhill_from_spring_to_sea() {
        let map = MAP.clone();
        let w = World::with_map(map.n, 32, map.seed, Some(map.clone()));
        let b = &w.bodies[0];
        let sea = b.layer_r(b.sea.unwrap());
        let (mut walked, mut climbs, mut worst, mut steps_up) = (0.0, 0, 0f64, 0);
        for s in map.springs() {
            let (mut k, mut last, mut top) = (s, f64::INFINITY, i32::MAX);
            loop {
                let g = &map.lines.segs[k as usize];
                if g.lake {
                    // through a lake the river is the lake (its bed is the lake's): walk on from its outlet
                    (last, top) = (f64::INFINITY, i32::MAX);
                    if g.next == NONE {
                        break;
                    }
                    k = g.next;
                    continue;
                }
                let steps = ((g.b - g.a).length() / 0.25).ceil() as usize;
                for q in 0..steps {
                    let d = (g.a + (g.b - g.a) * (q as f64 / steps as f64)).normalize();
                    let h = b.height(d);
                    let (f, i, j) = b.locate(d);
                    let z = b.layer_of(b.col_height(f as u8, i.floor() as i32, j.floor() as i32)).floor() as i32;
                    steps_up += (z > top) as usize;
                    top = top.min(z);
                    if h > last + 1e-6 {
                        climbs += 1;
                        worst = worst.max(h - last);
                    }
                    last = last.min(h);
                    walked += 0.25;
                }
                if g.next == NONE {
                    assert!(b.height(g.b.normalize()) < sea, "the river from spring {s} ends over the sea");
                    break;
                }
                k = g.next;
            }
        }
        assert!(climbs == 0 && steps_up == 0, "{climbs} climbs in {walked:.0} m of rivers, the worst {worst:.3} m; {steps_up} blocks up");
    }

    /// Rivers and lakes hold generated water (W5, ADR 0059): a point just under a wet river line's level, or
    /// under a lake's, is under water, at that level; a lake has a bed under its level; a dry bed holds none.
    #[test]
    fn rivers_and_lakes_hold_generated_water() {
        let map = MAP.clone();
        assert!(!map.lakes.is_empty(), "the test planet has lakes");
        let w = World::with_map(map.n, 32, map.seed, Some(map.clone()));
        let b = &w.bodies[0];
        let base = b.radius - crate::world::SEA_DEPTH;
        let (mut wet, mut dry, mut tried) = (0, 0, 0);
        for g in map.lines.segs.iter().step_by(7).filter(|g| !g.lake) {
            let d = ((g.a + g.b) * 0.5).normalize();
            let level = base + 0.5 * (g.wl[0] + g.wl[1]) as f64;
            let ground = b.height(d);
            tried += 1;
            match w.water_surface(d * (level - 0.05)) {
                Some(r) if g.fill[0].min(g.fill[1]) > 0.0 && ground < level - 0.3 => {
                    assert!((r - level.max(b.layer_r(b.sea.unwrap()))).abs() < 0.05, "the river's level {level:.3}, the water's {r:.3}");
                    wet += 1;
                }
                None if g.fill[0].max(g.fill[1]) == 0.0 => dry += 1,
                _ => {}
            }
        }
        assert!(wet > tried / 2, "{wet} of {tried} river points under water ({dry} dry beds)");
        let (mut lakes, mut deep) = (0, 0);
        for &(c, l) in map.lakes.iter().step_by(5) {
            let (f, i, j) = ((c / (map.m * map.m)) as usize, c % map.m, (c / map.m) % map.m);
            let s = b.n as f64 / map.m as f64;
            let d = b.dir(f, (i as f64 + 0.5) * s, (j as f64 + 0.5) * s);
            let level = base + l as f64;
            if b.height(d) < level - 0.5 {
                deep += 1;
                // (a river running in may stand a little higher where its channel reaches)
                let r = w.water_surface(d * (level - 0.25)).expect("under a lake");
                assert!(r > level - 0.05 && r < level + 1.0, "the lake's level {level:.3}, the water's {r:.3}");
            }
            lakes += 1;
        }
        assert!(deep > lakes / 2, "{deep} of {lakes} lake cells have a bed under the water");
        // every reach holds its water a while and runs on down to the sea (or ends in a lake no river leaves)
        let rs = &map.lines.reaches;
        for (k, r) in rs.iter().enumerate() {
            assert!(r.hold > 0.0 && r.hold.is_finite(), "reach {k} holds {}", r.hold);
            let (mut d, mut n) = (k as u32, 0);
            while d != NONE {
                d = rs[d as usize].down;
                n += 1;
                assert!(n <= rs.len(), "reach {k} runs in a loop");
            }
        }
    }

    /// Rivers and lakes are drawn (W5, ADR 0059): the chunk that holds a river's or a lake's level gets a
    /// water quad over each wet column at its generated level (kind 5, its level in 1/64 layer over the
    /// chunk's base).
    #[test]
    fn rivers_and_lakes_get_surface_quads_at_their_level() {
        use crate::world::{generate, river_quads, Key, CS};
        use glam::IVec3;
        let map = MAP.clone();
        let w = World::with_map(map.n, 32, map.seed, Some(map.clone()));
        let b = &w.bodies[0];
        let (mut hit, mut tried) = (0, 0);
        let pts = map.lines.segs.iter().filter(|g| !g.lake && g.fill[0] > 0.3).step_by(5).map(|g| g.a.normalize());
        for d in pts.take(60) {
            let blk = w.block_at(d * b.radius);
            let Some((top, acct, _)) = b.water_at(blk.face, blk.pos.x, blk.pos.y) else { continue };
            if acct == crate::liquid::SEA {
                continue;
            }
            tried += 1;
            let key = Key { body: 0, face: blk.face, level: 0, pos: IVec3::new(blk.pos.x.div_euclid(CS), blk.pos.y.div_euclid(CS), (top.floor() as i32).div_euclid(CS)) };
            let qs = river_quads(b, key, &[], &generate(b, key, &[]));
            let (x, y) = (blk.pos.x - key.base().x, blk.pos.y - key.base().y);
            let q = qs.iter().find(|q| {
                let (qx, qy, qw, qh) = ((q[0] & 63) as i32, (q[0] >> 6 & 63) as i32, (q[0] >> 18 & 31) as i32 + 1, (q[0] >> 23 & 31) as i32 + 1);
                (qx..qx + qw).contains(&x) && (qy..qy + qh).contains(&y)
            });
            if let Some(q) = q {
                assert_eq!(q[0] >> 28, 5, "a river quad");
                let level = key.base().z as f64 + (q[1] & 0x3ffff) as f64 / 64.0;
                assert!((level - top).abs() < 0.05, "drawn at {level:.3}, the water's top {top:.3}");
                hit += 1;
            }
        }
        assert!(tried > 10 && hit > tried * 2 / 3, "{hit} of {tried} river columns drawn");
    }

    /// A walker follows a river from its spring to the sea (W5, ADR 0059): the longest river wet all the way,
    /// walked and swum by the player's own physics, is in the water nearly all the way and reaches the sea.
    #[test]
    fn a_walker_follows_a_river_from_spring_to_sea_in_water() {
        use crate::liquid::Ground;
        use crate::phys::{self, Input, Player};
        let map = MAP.clone();
        let w = World::with_map(map.n, 32, map.seed, Some(map.clone()));
        let segs = &map.lines.segs;
        let mut fed = vec![false; segs.len()];
        for g in segs {
            if g.next != NONE {
                fed[g.next as usize] = true;
            }
        }
        // each spring's course down to the sea, if it is wet and out of lakes all the way
        let course = |k0: usize| {
            let mut v = vec![k0];
            while segs[*v.last().unwrap()].next != NONE {
                v.push(segs[*v.last().unwrap()].next as usize);
            }
            v.iter().all(|&k| !segs[k].lake && segs[k].fill[0].min(segs[k].fill[1]) > 0.0).then_some(v)
        };
        let path = (0..segs.len()).filter(|&k| !fed[k]).filter_map(course).max_by_key(|v| v.len()).expect("a river wet from spring to sea");
        let ends: Vec<DVec3> = std::iter::once(segs[path[0]].a).chain(path.iter().map(|&k| segs[k].b)).map(|p| p.normalize()).collect();
        // a point every metre or so along the line, the walker heading for each in turn
        let pts: Vec<DVec3> = ends.windows(2).flat_map(|q| {
            let n = ((q[1] - q[0]).length() * map.radius()).ceil().max(1.0) as usize;
            (0..n).map(move |k| q[0].lerp(q[1], k as f64 / n as f64).normalize())
        }).chain(ends.last().copied()).collect();
        let b = &w.bodies[0];
        let mut p = Player::at(b.centre + pts[0] * (b.height(pts[0]) + 0.5), DVec3::X);
        let (mut next, mut wet, mut ticks) = (1, 0, 0);
        // a walker that stops making way sidesteps for a while, one way then the other (a bank's corner at a
        // bend catches its box, as it would a player's)
        let (mut was, mut side, mut stalls) = (p.pos, 0, 0.0f32);
        let length: f64 = pts.windows(2).map(|q| (q[1] - q[0]).length() * b.radius).sum();
        while next < pts.len() && ticks < (length * 60.0) as usize + 3600 {
            let to = b.centre + pts[next] * b.radius;
            let up = phys::up(&w, p.pos);
            let ahead = (to - p.pos) - up * (to - p.pos).dot(up);
            if ahead.length() < 1.5 {
                next += 1;
                continue;
            }
            if ticks % 60 == 0 {
                if (p.pos - was).length() < 1.0 && side == 0 {
                    stalls += 1.0;
                    side = 40;
                }
                was = p.pos;
            }
            let strafe = if side > 0 { side -= 1; if stalls % 2.0 == 1.0 { 1.0 } else { -1.0 } } else { 0.0 };
            let inp = Input { mv: [if side > 0 { 0.3 } else { 1.0 }, strafe], jump: true, fwd: ahead.normalize().as_vec3().to_array(), ..Default::default() };
            phys::step(&w, &mut p, &inp, 1.0 / 60.0);
            // in the water: the column it is over holds water (it hops along with jump held)
            let (lo, hi) = w.ground().generated(World::col(w.block_at(p.pos)));
            wet += (lo < hi) as usize;
            ticks += 1;
        }
        assert_eq!(next, pts.len(), "reached point {next} of {} after {:.0} s ({length:.0} m)", pts.len(), ticks as f64 / 60.0);
        assert!(wet * 10 >= ticks * 9, "in the water {wet} of {ticks} ticks along {length:.0} m");
    }

    /// No plant grows in a river or a lake (W5): every plant's foot by a river stands over its water.
    #[test]
    fn no_plant_stands_in_a_river() {
        use crate::biomes::SPACING;
        let map = MAP.clone();
        let w = World::with_map(map.n, 32, map.seed, Some(map.clone()));
        let b = &w.bodies[0];
        let (mut feet, mut near) = (0, 0);
        for g in map.lines.segs.iter().filter(|g| !g.lake && g.fill[0] > 0.3).step_by(3) {
            let blk = w.block_at(g.a.normalize() * b.radius);
            let (cx, cy) = (blk.pos.x / SPACING, blk.pos.y / SPACING);
            for (dx, dy) in (-2..=2).flat_map(|x| (-2..=2).map(move |y| (x, y))) {
                let Some(f) = b.candidate(blk.face as usize, cx + dx, cy + dy) else { continue };
                feet += 1;
                if let Some((top, acct, _)) = b.water_at(blk.face, f.i, f.j) {
                    near += (acct != crate::liquid::SEA) as usize;
                    assert!(f.z as f64 > top, "a plant at layer {} in water up to {top:.2}", f.z);
                }
            }
        }
        assert!(feet > 50, "{feet} plants by the rivers ({near} by their water)");
    }

    /// Stand points for a filmstrip (W5): a river's bank, a lake's shore and the steepest rapids of the
    /// default planet. `cargo test --release -p engine --lib -- --ignored --nocapture stand_points`
    #[test]
    #[ignore]
    fn stand_points() {
        let map = crate::bake::get(1337, 6144);
        let segs = &map.lines.segs;
        let wet = |g: &&Seg| !g.lake && g.fill[0] > 0.4 && g.wl[0] > 0.5;
        let bank = |g: &Seg| {
            let p = (g.a + g.b) * 0.5;
            let across = (g.b - g.a).cross(p).normalize();
            let at = (p + across * (g.hw[0] as f64 + 4.0)).normalize();
            println!("  at={:.6},{:.6},{:.6} hw {:.1} fill {:.2} fall {:.3} level {:.1}", at.x, at.y, at.z, g.hw[0], g.fill[0], (g.wl[0] - g.wl[1]) / (g.b - g.a).length() as f32, g.wl[0]);
        };
        println!("wide rivers:");
        let mut v: Vec<&Seg> = segs.iter().filter(wet).collect();
        v.sort_by(|a, b| b.hw[0].total_cmp(&a.hw[0]));
        v.iter().step_by(v.len() / 6 + 1).take(6).for_each(|g| bank(g));
        println!("rapids:");
        v.sort_by(|a, b| ((b.wl[0] - b.wl[1]) / (b.b - b.a).length() as f32).total_cmp(&((a.wl[0] - a.wl[1]) / (a.b - a.a).length() as f32)));
        v.iter().take(4).for_each(|g| bank(g));
        println!("lakes (a shore segment):");
        segs.iter().filter(|g| g.lake && g.next != NONE && !segs[g.next as usize].lake).take(4).for_each(|g| { let d = g.b.normalize(); println!("  at={:.6},{:.6},{:.6} level {:.1}", d.x, d.y, d.z, g.wl[0]); });
        // the cost of a chunk job's river quads, on chunks of level 0 to 3 that hold a river's level
        use crate::world::{generate, river_quads, sea_quads, Key, CS};
        let w = World::with_map(6144, 32, 1337, Some(map.clone()));
        let b = &w.bodies[0];
        let mut keys = Vec::new();
        for g in v.iter().step_by(v.len() / 200 + 1) {
            let blk = w.block_at(g.a.normalize() * (b.radius - crate::world::SEA_DEPTH + g.wl[0] as f64));
            for level in 0..4u8 {
                let s = CS << level;
                keys.push(Key { body: 0, face: blk.face, level, pos: glam::IVec3::new(blk.pos.x.div_euclid(s), blk.pos.y.div_euclid(s), blk.pos.z.div_euclid(s)) });
            }
        }
        let gens: Vec<_> = keys.iter().map(|&k| generate(b, k, &[])).collect();
        for level in 0..4u8 {
            let t = std::time::Instant::now();
            let mut c = 0;
            for (&k, g) in keys.iter().zip(&gens).filter(|(k, _)| k.level == level) {
                std::hint::black_box(river_quads(b, k, &[], g));
                c += 1;
            }
            println!("  level {level}: {:.1} us a chunk", t.elapsed().as_secs_f64() / c as f64 * 1e6);
        }
        let t = std::time::Instant::now();
        let n: usize = keys.iter().zip(&gens).map(|(&k, g)| river_quads(b, k, &[], g).len()).sum();
        let tr = t.elapsed().as_secs_f64() / keys.len() as f64 * 1e6;
        let t = std::time::Instant::now();
        let ns: usize = keys.iter().zip(&gens).map(|(&k, g)| sea_quads(b, k, &[], g).len()).sum();
        let ts = t.elapsed().as_secs_f64() / keys.len() as f64 * 1e6;
        let t = std::time::Instant::now();
        for (&k, _) in keys.iter().zip(&gens) {
            std::hint::black_box(generate(b, k, &[]));
        }
        let tg = t.elapsed().as_secs_f64() / keys.len() as f64 * 1e6;
        println!("river_quads {tr:.1} us a chunk ({n} quads over {} chunks); sea_quads {ts:.1} us ({ns}); generate {tg:.1} us", keys.len());
        // a dry chunk far over the ground: the early reject
        let high: Vec<Key> = keys.iter().map(|k| Key { pos: k.pos + glam::IVec3::Z * 4, ..*k }).collect();
        let t = std::time::Instant::now();
        for &k in &high {
            std::hint::black_box(river_quads(b, k, &[], &crate::world::Gen::Empty));
        }
        println!("river_quads over the ground {:.2} us a chunk", t.elapsed().as_secs_f64() / high.len() as f64 * 1e6);
        // the reach tick: every reach holding water
        let mut liq = crate::liquid::Liquids::default();
        for r in 0..map.lines.reaches.len() as u32 {
            liq.rain(r, 4096 * 50);
        }
        let g = w.ground();
        let t = std::time::Instant::now();
        for _ in 0..60 {
            liq.step(&g);
        }
        println!("a second of steps with {} reaches holding water: {:.1} us", map.lines.reaches.len(), t.elapsed().as_secs_f64() * 1e6);
    }

    /// The detail follows the slope (ADR 0058): on the game's own heights, steep ground is several times
    /// rougher than a plain, and the sea floor by the coast stays calm under its water. Measured as the RMS
    /// of the ground over the map's spline, away from the river beds.
    #[test]
    fn steep_ground_is_rougher_than_flat() {
        let map = MAP.clone();
        let w = World::with_map(map.n, 32, map.seed, Some(map.clone()));
        let b = &w.bodies[0];
        let s = map.m as f64 / b.n as f64;
        let cell = b.n as f64 / map.m as f64;
        let (mut flat, mut steep, mut coast) = ([0.0f64; 2], [0.0f64; 2], [0.0f64; 2]);
        for face in 0..6 {
            for j in (0..b.n).step_by(5) {
                for i in (0..b.n).step_by(5) {
                    let d = b.dir(face, i as f64 + 0.5, j as f64 + 0.5);
                    let (f, pi, pj) = b.locate(d);
                    let (x, y) = (pi * s, pj * s);
                    let base = map.height(f, x, y, 0) as f64;
                    if map.river(f, x, y, d, 1.0).is_some() || !(-40.0..100.0).contains(&base) {
                        continue;
                    }
                    let slope = |x: f64, y: f64| ((map.height(f, x + 0.5, y, 0) - map.height(f, x - 0.5, y, 0)) as f64).hypot((map.height(f, x, y + 0.5, 0) - map.height(f, x, y - 0.5, 0)) as f64) / cell;
                    let g = slope(x, y);
                    // a plain: flat a cell around too (a hilltop's flat spot is rough from its sides), and not
                    // a lake's flat (calm by design, it would flatter the ratio)
                    let plain = (0.005..0.1).contains(&g) && [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)].iter().all(|(a, c)| slope(x + a, y + c) < 0.1);
                    let dev = b.height(d) - (b.radius - crate::world::SEA_DEPTH + base);
                    let bin = if base > 3.0 && plain { &mut flat } else if g > 0.5 { &mut steep } else if (-8.0..-1.0).contains(&base) { &mut coast } else { continue };
                    bin[0] += dev * dev;
                    bin[1] += 1.0;
                }
            }
        }
        let rms = |a: [f64; 2]| (a[0] / a[1]).sqrt();
        let (f, st, c) = (rms(flat), rms(steep), rms(coast));
        println!("rms: plain {f:.3} m ({} cols), steep {st:.3} m ({}), coast floor {c:.3} m ({}); steep / plain {:.1}", flat[1], steep[1], coast[1], st / f);
        assert!(flat[1] > 100.0 && steep[1] > 100.0 && coast[1] > 100.0, "too few columns to measure");
        assert!(st > 4.0 * f, "steep {st:.3} m is not 4 x plain {f:.3} m");
        assert!(c < 0.15, "the coast's sea floor is {c:.3} m rough");
    }

    /// The bed: on its line the ground is the bed, the flat bottom is wider than a column, and a bank's cut
    /// ends where its reach does.
    #[test]
    fn a_bed_is_flat_across_and_its_cut_ends_at_the_bank() {
        let map = &*MAP;
        let g = map.lines.segs.iter().find(|g| g.next != NONE && g.hw[0] >= 1.0).unwrap();
        let mid = (g.a + g.b) * 0.5;
        let side = (g.b - g.a).cross(mid).normalize();
        let at = |d: f32| {
            let p = (mid + side * d as f64).normalize();
            let (f, x, y) = locate(map.m, p);
            map.river(f, x, y, p, 1.0)
        };
        let c = at(0.0).unwrap();
        assert_eq!(c.floor, c.cut, "the line is the bed");
        let hw = 0.5 * (g.hw[0] + g.hw[1]);
        assert!(at(0.75).unwrap().cut <= c.cut + 1e-3, "flat past a column's half diagonal");
        assert!(at(hw + 1.0).unwrap().cut > c.cut, "the bank rises");
    }

    /// The cost on the default 4 km planet (release: `cargo test --release -p engine rivers_cost -- --ignored
    /// --nocapture`), and a spot to look along a wide river from; keeps the map in the game's saves/bakes.
    #[test]
    #[ignore]
    fn rivers_cost() {
        let t = std::time::Instant::now();
        let map = Map::bake(1337, 6144);
        let bake = t.elapsed().as_secs_f64();
        let t = std::time::Instant::now();
        let lines = Lines::new(&map);
        let new = t.elapsed().as_secs_f64();
        let mb = (lines.at.len() * 8 + lines.near.len() * 8 + lines.segs.len() * std::mem::size_of::<Seg>()) as f64 / 1e6;
        println!("bake {bake:.1} s, lines {:.3} s: {} rivers, {} springs, {} index pairs, {mb:.1} MB, {} crossings", new, map.rivers.len(), map.springs().len(), lines.at.len(), crossings(&map));
        let mut rng = crate::bake::land::Rng::new(3);
        let pts: Vec<DVec3> = (0..1_000_000).map(|_| rng.dir()).collect();
        let near: Vec<DVec3> = (0..1_000_000).map(|k| { let g = &lines.segs[k % lines.segs.len()]; (g.a + (g.b - g.a) * rng.f() + rng.dir() * 6.0).normalize() }).collect();
        let w = World::with_map(map.n, 32, map.seed, Some(Arc::new(Map::from_bytes(&map.to_bytes()).unwrap())));
        let mut dry = Map::from_bytes(&map.to_bytes()).unwrap();
        dry.lines = Lines::default();
        let w0 = World::with_map(map.n, 32, map.seed, Some(Arc::new(dry)));
        for (name, ps) in [("anywhere", &pts), ("near rivers", &near)] {
            let mut ns = [0.0; 2];
            for _ in 0..2 {
                for (k, w) in [&w0, &w].into_iter().enumerate() {
                    let t = std::time::Instant::now();
                    let s: f64 = ps.iter().map(|&p| w.bodies[0].height(p)).sum();
                    ns[k] = t.elapsed().as_secs_f64() * 1e3;
                    assert!(s > 0.0);
                }
            }
            println!("{name}: height {:.0} ns a lookup without rivers, {:.0} ns with", ns[0], ns[1]);
        }
        // the real consumer: level-0 height tiles (a chunk's footprint of columns), 3000 of them over a face
        let mut ns = [0.0; 2];
        for (k, w) in [&w0, &w].into_iter().enumerate() {
            let t = std::time::Instant::now();
            for q in 0..3000 {
                let tile = w.bodies[0].tile(2, 0, 20 + q % 60, 20 + q / 60);
                assert!(tile.h[0] > 0.0);
            }
            ns[k] = t.elapsed().as_secs_f64() * 1e6 / 3000.0;
        }
        println!("level-0 tile: {:.1} us without rivers, {:.1} us with", ns[0], ns[1]);
        // a spot: the widest river on land, from a node 30 m up its reach, looking 40 m downstream
        let segs = &lines.segs;
        let k = (0..segs.len()).filter(|&k| segs[k].next != NONE && segs[k].bed[0] > 4.0).max_by(|&a, &c| segs[a].hw[0].total_cmp(&segs[c].hw[0])).unwrap();
        let up = segs[k].a.normalize();
        let mut q = k;
        for _ in 0..4 { if segs[q as usize].next != NONE { q = segs[q as usize].next as usize; } }
        let ahead_ref = if up.y.abs() > 0.9 { DVec3::Z } else { DVec3::Y };
        let ahead = (ahead_ref - up * ahead_ref.dot(up)).normalize();
        let to = segs[q].b - segs[k].a;
        let yaw = to.dot(up.cross(ahead)).atan2(to.dot(ahead)).to_degrees();
        println!("spot: at={:.6},{:.6},{:.6} yaw={yaw:.1} hw {:.1} m bed {:.1} m", up.x, up.y, up.z, segs[k].hw[0], segs[k].bed[0]);
        let dir = std::path::Path::new("../../saves/bakes");
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(crate::bake::file(1337, 6144).file_name().unwrap()), map.to_bytes()).unwrap();
    }
}
