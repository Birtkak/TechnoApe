//! The smallest stand-alone copy of `liquid.rs`'s link step (ADR 0020) on one gap a column, open to the sky, flat
//! floors: the same fixed point (`U`, `VF`, `CG`), speed-up, friction, thin-sheet drag, `VSTOP`, `QUIET` sleep and
//! `settle` every 15 steps. Left out: ceilings and pressure, falls, carried speed, rivers, the wire.
//! Added: a head offset eta (units << S) per column, so a link speeds up by (surface - eta) differences.
use std::f64::consts::TAU;

pub const U: i64 = 1 << 12;
const UB: u32 = 12;
const VF: u32 = 20;
/// eta's extra fraction bits: the tide's gradient is under a unit a column.
const S: u32 = 8;
const VMAX: i64 = 1 << (VF - 1);
const ACAP: i64 = 16 * U;
const MIN: i64 = U / 32;
const FRICTION: u32 = 9;
const THIN: i64 = 128;
pub const VSTOP: i64 = 1 << 2;
const VSLOW: i64 = 1 << 11;
const SETTLE_EVERY: u64 = 15;
const SETTLE_DH: i64 = U / 32;
const QUIET: u16 = 30;
/// liquid.rs's DEAD: 1/64 block.
pub const DEAD: i64 = U / 64;
const ASLEEP: u16 = u16::MAX;

/// g dt^2 / dx in 1/2^VF blocks a step per block of head, dt 1/60 s (liquid.rs's `CG` with g a parameter).
pub fn cg(g: f64) -> i64 {
    (g / 3600.0 * (1u64 << VF) as f64) as i64
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Settle {
    /// none: links alone
    Off,
    /// liquid.rs today: a calm body laid flat (to one sea neighbour's level if it touches the sea)
    Flat,
    /// a calm body laid to its equilibrium: surface - eta uniform (to the sea's mean + eta if it touches it)
    Tide,
}

/// A tide raiser: its rise h (m) and its direction in the body frame turning `rate` rad/s in the equator plane.
#[derive(Clone, Copy)]
pub struct Raiser {
    pub h: f64,
    pub rate: f64,
    pub phase: f64,
}

pub struct Sim {
    pub w: usize,
    pub h: usize,
    pub floor: Vec<i32>,
    pub water: Vec<i64>,
    pub solid: Vec<bool>,
    pub sea: Vec<bool>,
    /// outward speed per neighbour [-x, +x, -y, +y], 1/2^VF blocks a step, and the unit fractions
    pub v: Vec<[i32; 4]>,
    frac: Vec<[i32; 4]>,
    out: Vec<[i32; 4]>,
    quiet: Vec<u16>,
    awake: Vec<u32>,
    /// body-frame up of each column, and eta now and at the column's last lay (units << S)
    up: Vec<[f64; 3]>,
    pub eta: Vec<i64>,
    laid: Vec<i64>,
    pub raisers: Vec<Raiser>,
    /// each raiser's (h, cos, sin) at this step: worked out once a step, not once a column
    dirs: Vec<(f64, f64, f64)>,
    any_sea: bool,
    /// the mean sea's surface, units
    pub mean: i64,
    pub term: bool,
    pub settle: Settle,
    /// re-lay sleeping bodies whose equilibrium moved DEAD or more, once a second (the sleep rule tried)
    pub relay: bool,
    /// the re-lay's dead band, units
    pub band: i64,
    pub cg: i64,
    pub sea_extra: i64,
    pub removed: i64,
    pub steps: u64,
    pub stepped: u64,
    pub relaid: u64,
    pub wakes: u64,
    t: f64,
}

impl Sim {
    /// A w x h grid on a sphere of `radius` m, its x along the equator from longitude `lon0`, water `depth`
    /// blocks over floor 0 everywhere, at the mean.
    pub fn new(w: usize, h: usize, radius: f64, lon0: f64, depth: i64, raisers: Vec<Raiser>) -> Sim {
        let n = w * h;
        let up = (0..n).map(|i| {
            let (x, y) = ((i % w) as f64, (i / w) as f64);
            let (lon, lat) = (lon0 + x / radius, y / radius);
            [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
        }).collect();
        let mut s = Sim {
            w, h, floor: vec![0; n], water: vec![depth * U; n], solid: vec![false; n], sea: vec![false; n],
            v: vec![[0; 4]; n], frac: vec![[0; 4]; n], out: vec![[0; 4]; n], quiet: vec![ASLEEP; n], awake: Vec::new(),
            up, eta: vec![0; n], laid: vec![0; n], raisers, dirs: Vec::new(), any_sea: false, mean: depth * U, term: true, settle: Settle::Tide, relay: true, band: DEAD,
            cg: cg(9.81), sea_extra: 0, removed: 0, steps: 0, stepped: 0, relaid: 0, wakes: 0, t: 0.0,
        };
        s.retide_all();
        s
    }
    fn aim(&mut self) {
        self.dirs = self.raisers.iter().map(|r| {
            let a = r.phase + r.rate * self.t;
            (r.h, a.cos(), a.sin())
        }).collect();
    }
    fn eta_of(&self, i: usize) -> i64 {
        let u = self.up[i];
        let mut e = 0.0;
        for &(h, c0, s0) in &self.dirs {
            let c = u[0] * c0 + u[1] * s0;
            e += h * (1.5 * c * c - 0.5);
        }
        (e * U as f64 * (1u64 << S) as f64).round() as i64
    }
    fn retide_all(&mut self) {
        self.aim();
        for i in 0..self.eta.len() {
            self.eta[i] = self.eta_of(i);
        }
    }
    pub fn time(&self) -> f64 {
        self.t
    }
    pub fn awake_count(&self) -> usize {
        self.awake.len()
    }
    pub fn surface(&self, i: usize) -> i64 {
        self.floor[i] as i64 * U + self.water[i]
    }
    /// the open sea's level in column i: the mean plus the tide there
    pub fn sea_level(&self, i: usize) -> i64 {
        self.mean + (self.eta[i] >> S)
    }
    /// every column's water, the sea's too
    pub fn all_units(&self) -> i64 {
        (0..self.water.len()).filter(|&i| !self.solid[i]).map(|i| self.water[i]).sum()
    }
    pub fn stored_units(&self) -> i64 {
        (0..self.water.len()).filter(|&i| !self.sea[i] && !self.solid[i]).map(|i| self.water[i]).sum()
    }
    fn nb(&self, i: usize, d: usize) -> Option<usize> {
        let (x, y) = (i % self.w, i / self.w);
        let j = match d {
            0 if x > 0 => i - 1,
            1 if x + 1 < self.w => i + 1,
            2 if y > 0 => i - self.w,
            3 if y + 1 < self.h => i + self.w,
            _ => return None,
        };
        (!self.solid[j]).then_some(j)
    }
    /// every stored column at its equilibrium now (the rest state), and the sea at its level
    pub fn lay_all(&mut self) {
        self.any_sea = self.sea.iter().any(|&s| s);
        for i in 0..self.water.len() {
            if !self.solid[i] {
                self.water[i] = self.sea_level(i) - self.floor[i] as i64 * U;
                self.laid[i] = self.eta[i];
            }
        }
    }
    pub fn wake(&mut self, i: usize) {
        if self.quiet[i] == ASLEEP {
            self.quiet[i] = 0;
            self.awake.push(i as u32);
        }
    }
    pub fn wake_all(&mut self) {
        for i in 0..self.water.len() {
            if !self.solid[i] {
                self.wake(i);
            }
        }
    }
    /// a block set into column i: its water leaves the count (booked to `removed`)
    pub fn set_solid(&mut self, i: usize) {
        self.removed += self.water[i];
        self.water[i] = 0;
        self.solid[i] = true;
        for d in 0..4 {
            if let Some(j) = self.nb(i, d) {
                self.wake(j);
            }
        }
    }
    /// (surface - eta) of a over b's, units << S, the link's head difference (b's floored at the window)
    fn dh(&self, a: usize, b: usize) -> i64 {
        let lo = self.floor[a].max(self.floor[b]) as i64 * U;
        let d = (self.surface(a) - self.surface(b).max(lo)) << S;
        if self.term { d - (self.eta[a] - self.eta[b]) } else { d }
    }
    fn moves(&self, dh: i64) -> bool {
        dh * self.cg >> (UB + S) > VSTOP
    }
    fn spreads(&self, a: usize, b: usize) -> bool {
        self.floor[b] < self.floor[a] || self.water[a] >= if self.water[b] == 0 { 2 * MIN } else { MIN }
    }

    pub fn step(&mut self) {
        self.t += 1.0 / 60.0;
        self.steps += 1;
        // the tide's moment for the columns stepped (the term's cost: one eta a node a step)
        let awake = std::mem::take(&mut self.awake);
        if self.term || self.any_sea {
            self.aim();
            for &a in &awake {
                self.eta[a as usize] = self.eta_of(a as usize);
            }
        }
        // the sea held at its level, the difference to its account
        for &a in &awake {
            let a = a as usize;
            if self.sea[a] {
                let want = self.sea_level(a) - self.floor[a] as i64 * U;
                self.sea_extra -= want - self.water[a];
                self.water[a] = want;
            }
        }
        self.stepped += awake.len() as u64;
        // 1: speeds and outflows
        for &a in &awake {
            let a = a as usize;
            let (sa, wa) = (self.surface(a), self.water[a]);
            let mut total = 0i64;
            let mut res = [(0i32, 0i32, 0i32); 4];
            for d in 0..4 {
                let Some(b) = self.nb(a, d) else { continue };
                let lo = self.floor[a].max(self.floor[b]) as i64 * U;
                let area = (sa - lo).clamp(0, ACAP);
                let mut v = self.v[a][d] as i64;
                if area == 0 || !self.spreads(a, b) {
                    v = 0;
                } else {
                    v += self.dh(a, b) * self.cg >> (UB + S);
                    v -= (v >> FRICTION).max(1);
                    if area < U {
                        v -= v * (U - area) / (THIN * U);
                    }
                    if v < VSTOP {
                        v = 0;
                    }
                    v = v.clamp(0, VMAX);
                }
                let flow = v * area + self.frac[a][d] as i64;
                res[d] = (v as i32, (flow >> VF) as i32, (flow & ((1 << VF) - 1)) as i32);
                total += flow >> VF;
            }
            if total > wa {
                for r in &mut res {
                    r.1 = (r.1 as i64 * wa.max(0) / total) as i32;
                }
            }
            for d in 0..4 {
                self.v[a][d] = res[d].0;
                self.frac[a][d] = if res[d].0 == 0 { 0 } else { res[d].2 };
                self.out[a][d] = res[d].1;
            }
        }
        // 2: move the water; whoever received is touched
        let mut touched = awake.clone();
        for &a in &awake {
            let a = a as usize;
            for d in 0..4 {
                let o = self.out[a][d] as i64;
                if o > 0 {
                    let b = self.nb(a, d).unwrap();
                    self.water[a] -= o;
                    self.water[b] += o;
                    if self.quiet[b] == ASLEEP {
                        self.quiet[b] = 0;
                        touched.push(b as u32);
                    }
                }
            }
        }
        // 3: who stays awake (moving, or a neighbour's head difference would move it)
        let mut next = Vec::with_capacity(touched.len());
        for &a in &touched {
            let a = a as usize;
            let mut mv = false;
            let mut pull = false;
            for d in 0..4 {
                let Some(b) = self.nb(a, d) else { continue };
                mv |= self.v[a][d] > 0 || self.out[a][d] > 0 || self.out[b][d ^ 1] > 0;
                let push = |x: usize, y: usize| self.surface(x) > self.floor[x].max(self.floor[y]) as i64 * U && self.moves(self.dh(x, y)) && self.spreads(x, y);
                if push(a, b) || push(b, a) {
                    pull = true;
                    if self.quiet[b] == ASLEEP {
                        next.push(b as u32);
                    }
                }
            }
            self.quiet[a] = if mv || pull { 0 } else { self.quiet[a].saturating_add(1) };
            // the sea's columns are the formula's edge: kept awake (a mouth is a few columns)
            if self.quiet[a] < QUIET || self.sea[a] {
                next.push(a as u32);
            } else {
                self.quiet[a] = ASLEEP;
                self.v[a] = [0; 4];
                self.frac[a] = [0; 4];
                self.out[a] = [0; 4];
            }
        }
        for &a in &next {
            if self.quiet[a as usize] == ASLEEP {
                self.quiet[a as usize] = 0;
            }
        }
        next.sort_unstable();
        next.dedup();
        self.awake = next;
        if self.steps % SETTLE_EVERY == 0 && self.settle != Settle::Off {
            self.settle_awake();
        }
        if self.steps % 60 == 0 {
            self.second();
        }
    }

    /// The bodies (stored, wet, linked) holding any of `starts`, by flood fill.
    fn bodies(&self, starts: &[u32], all: bool) -> Vec<Vec<usize>> {
        let mut seen = vec![false; self.water.len()];
        let mut out = Vec::new();
        for &s in starts {
            let s = s as usize;
            if seen[s] || self.sea[s] || self.solid[s] || (!all && self.quiet[s] == ASLEEP) {
                continue;
            }
            seen[s] = true;
            let mut body = vec![s];
            let mut k = 0;
            while k < body.len() {
                let a = body[k];
                k += 1;
                for d in 0..4 {
                    if let Some(b) = self.nb(a, d) {
                        if !seen[b] && !self.sea[b] && self.water[a] > 0 && self.water[b] > 0 {
                            seen[b] = true;
                            body.push(b);
                        }
                    }
                }
            }
            out.push(body);
        }
        out
    }
    /// sea columns beside a body
    fn sea_beside(&self, body: &[usize]) -> Option<usize> {
        body.iter().find_map(|&a| (0..4).filter_map(|d| self.nb(a, d)).find(|&b| self.sea[b]))
    }
    /// liquid.rs's `settle`: a calm body laid flat, or (Settle::Tide) to its equilibrium
    fn settle_awake(&mut self) {
        let starts = self.awake.clone();
        for body in self.bodies(&starts, false) {
            if body.len() < 2 {
                continue;
            }
            let head = |s: &Sim, a: usize| if s.settle == Settle::Tide { s.surface(a) - (s.eta[a] >> S) } else { s.surface(a) };
            let calm = body.iter().all(|&a| self.v[a].iter().all(|&v| (v as i64) < VSLOW));
            let (lo, hi) = body.iter().fold((i64::MAX, i64::MIN), |(l, h), &a| (l.min(head(self, a)), h.max(head(self, a))));
            if !calm || hi - lo >= SETTLE_DH {
                continue;
            }
            self.lay(&body);
            for &a in &body {
                self.quiet[a] = ASLEEP;
                self.v[a] = [0; 4];
                self.frac[a] = [0; 4];
            }
        }
        self.awake.retain(|&a| self.quiet[a as usize] != ASLEEP);
    }
    /// Lay a body: against the sea to the sea's level (flat: one sea neighbour's; tide: each column's own), else
    /// conserving its units exactly, flat or surface - eta uniform.
    fn lay(&mut self, body: &[usize]) {
        let tide = self.settle == Settle::Tide;
        let e = |s: &Sim, a: usize| if tide { s.eta[a] >> S } else { 0 };
        if let Some(sea) = self.sea_beside(body) {
            let flat = self.sea_level(sea);
            for &a in body {
                let want = if tide { self.sea_level(a) } else { flat } - self.floor[a] as i64 * U;
                self.sea_extra -= want - self.water[a];
                self.water[a] = want;
                self.laid[a] = self.eta[a];
            }
            return;
        }
        // level c: sum(c + e_a - floor_a U) = units; the remainder spread a unit a column, so the sum is exact
        let units: i64 = body.iter().map(|&a| self.water[a]).sum();
        let base: i64 = body.iter().map(|&a| e(self, a) - self.floor[a] as i64 * U).sum();
        let n = body.len() as i64;
        let c = (units - base).div_euclid(n);
        let mut rem = (units - base).rem_euclid(n);
        for &a in body {
            let extra = if rem > 0 { rem -= 1; 1 } else { 0 };
            self.water[a] = (c + e(self, a) - self.floor[a] as i64 * U + extra).max(0);
            self.laid[a] = self.eta[a];
        }
        debug_assert_eq!(body.iter().map(|&a| self.water[a]).sum::<i64>(), units);
    }
    /// Once a second (liquid.rs's `retide` moment): the eta of every stored column is taken, and a sleeping body
    /// whose equilibrium moved DEAD or more since it was laid is re-laid in place, without waking (`relay`); else,
    /// as today, sleeping water beside the sea that is DEAD off the sea's level is woken.
    fn second(&mut self) {
        self.retide_all();
        let sleeping: Vec<u32> = (0..self.water.len() as u32).filter(|&i| self.quiet[i as usize] == ASLEEP && !self.solid[i as usize]).collect();
        if self.relay && self.settle == Settle::Tide {
            for body in self.bodies(&sleeping, true) {
                if body.iter().any(|&a| self.quiet[a] != ASLEEP) {
                    continue;
                }
                let sea = self.sea_beside(&body).is_some();
                // a closed body cares only for the change's shape, not its mean
                let mean = if sea { 0 } else { body.iter().map(|&a| self.eta[a] - self.laid[a]).sum::<i64>() / body.len() as i64 };
                if body.iter().any(|&a| ((self.eta[a] - self.laid[a] - mean) >> S).abs() >= self.band) {
                    self.lay(&body);
                    self.relaid += body.len() as u64;
                }
            }
        } else {
            for i in sleeping {
                let a = i as usize;
                if self.sea[a] {
                    continue;
                }
                let off = (0..4).filter_map(|d| self.nb(a, d)).any(|b| self.sea[b] && (self.sea_level(b) - self.surface(a)).abs() >= DEAD);
                if off {
                    self.wake(a);
                    self.wakes += 1;
                }
            }
        }
        for i in 0..self.water.len() {
            if self.sea[i] && !self.solid[i] {
                self.wake(i);
            }
        }
    }
}

/// Terra's radius on the 4 km planet (6144 columns a face edge), m: Body::new's n / sqrt(2 pi / 3).
pub const R: f64 = 6144.0 / 1.447_202_472_141_282_4; // sqrt(2 pi / 3)
/// The lunar day the sky's numbers give (1 / (1/DAY - 1/MONTH)), s: two highs in it.
pub const LUNAR_DAY: f64 = 1920.0;

/// A raiser for a spring range `range` m (1.5 h), turning once a lunar day.
pub fn luna(range: f64) -> Raiser {
    Raiser { h: range / 1.5, rate: TAU / LUNAR_DAY, phase: 0.0 }
}

pub fn bench() {
    use std::time::Instant;
    // 1: a step's cost, 100,000 stored columns all awake (kept awake), with and without the term
    for term in [false, true, false, true, false, true] {
        let mut s = Sim::new(500, 200, R, 0.3, 2, vec![luna(1.0), Raiser { h: 1.0 / 1.5 / 3.0, rate: TAU / 1800.0, phase: 1.0 }]);
        (s.term, s.settle, s.relay) = (term, Settle::Off, false);
        s.lay_all();
        // a wave so every link carries flow
        for i in 0..s.water.len() {
            s.water[i] += ((i % 7) as i64 - 3) * U / 64;
        }
        s.wake_all();
        for _ in 0..60 {
            s.step();
        }
        let (n0, t0) = (s.stepped, Instant::now());
        for _ in 0..600 {
            s.wake_all();
            s.step();
        }
        let dt = t0.elapsed().as_secs_f64();
        let nodes = (s.stepped - n0) as f64;
        println!("{{\"bench\":\"step\",\"term\":{term},\"awake\":{:.0},\"ms_per_step\":{:.3},\"ns_per_node\":{:.2}}}", nodes / 600.0, dt / 600.0 * 1e3, dt / nodes * 1e9);
    }
    // 2: who is awake through a tidal cycle on a 100,000-column basin beside the sea (500 x 200, the sea along x = 0)
    for (label, range, term, settle, relay, band) in [
        ("today: no term, flat settle, edge wake", 1.0, false, Settle::Flat, false, DEAD),
        ("term, tide settle, edge wake", 1.0, true, Settle::Tide, false, DEAD),
        ("term, tide settle, re-lay asleep, band 1/64", 1.0, true, Settle::Tide, true, DEAD),
        ("term, tide settle, re-lay asleep, band 1/256", 1.0, true, Settle::Tide, true, DEAD / 4),
        ("term, tide settle, re-lay asleep, band 1/1024", 1.0, true, Settle::Tide, true, DEAD / 16),
        ("derived Terra (6.3 mm springs), band 1/256", 0.0063, true, Settle::Tide, true, DEAD / 4),
    ] {
        let mut s = Sim::new(501, 200, R, 0.3, 2, vec![luna(range)]);
        (s.term, s.settle, s.relay, s.band) = (term, settle, relay, band);
        for y in 0..200 {
            s.sea[y * 501] = true;
        }
        s.lay_all();
        let t0 = Instant::now();
        let steps = (LUNAR_DAY / 2.0 * 60.0) as u64;
        let mut peak = 0;
        let n0 = s.stepped;
        for _ in 0..steps {
            s.step();
            peak = peak.max(s.awake_count());
        }
        let dt = t0.elapsed().as_secs_f64();
        let worst = (0..s.water.len()).filter(|&i| !s.sea[i]).map(|i| (s.surface(i) - s.sea_level(i)).abs()).max().unwrap();
        println!("{{\"bench\":\"cycle\",\"case\":\"{label}\",\"mean_awake\":{:.0},\"peak_awake\":{peak},\"relaid_cols_per_s\":{:.0},\"wakes\":{},\"ms_per_step\":{:.4},\"worst_off_sea_mm\":{:.2}}}", (s.stepped - n0) as f64 / steps as f64, s.relaid as f64 / (LUNAR_DAY / 2.0), s.wakes, dt / steps as f64 * 1e3, worst as f64 / U as f64 * 1e3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MM: f64 = 1e3 / U as f64;
    /// the wire's dead band in mm (15.6): water this close to generation is generation
    const DEAD_MM: f64 = DEAD as f64 * MM;
    /// the modes each test is run in: (name, term, settle)
    const TODAY: (bool, Settle) = (false, Settle::Flat);
    const PROPOSED: (bool, Settle) = (true, Settle::Tide);

    /// A 200-column channel, the sea held at mean + eta in column 0; two tidal cycles; the worst |stored - sea's
    /// level there| after the first minute, mm.
    fn harbour((term, settle): (bool, Settle)) -> f64 {
        let mut s = Sim::new(201, 1, R, 0.3, 2, vec![luna(1.0)]);
        (s.term, s.settle, s.band) = (term, settle, DEAD / 4);
        s.sea[0] = true;
        s.lay_all();
        s.wake_all();
        let mut worst = 0i64;
        while s.time() < LUNAR_DAY {
            s.step();
            if s.time() > 60.0 {
                worst = worst.max((1..201).map(|i| (s.surface(i) - s.sea_level(i)).abs()).max().unwrap());
            }
        }
        worst as f64 * MM
    }

    #[test]
    fn a_stored_harbour_stays_level_with_the_sea() {
        let (today, proposed) = (harbour(TODAY), harbour(PROPOSED));
        let links = harbour((true, Settle::Flat));
        println!("harbour: today {today:.2} mm, term + flat settle {links:.2} mm, term + tide settle {proposed:.2} mm off the sea");
        assert!(today > DEAD_MM, "today's harbour is flat, so off the sea's slope: {today}");
        assert!(proposed < DEAD_MM, "{proposed} mm");
    }

    /// A closed 200-column basin: its end-to-end tilt against the potential's, each second, and its units.
    fn basin((term, settle): (bool, Settle)) -> (f64, f64, i64) {
        let mut s = Sim::new(200, 1, R, 0.3, 2, vec![luna(1.0)]);
        (s.term, s.settle, s.band) = (term, settle, DEAD / 4);
        s.lay_all();
        s.wake_all();
        let units = s.stored_units();
        let (mut worst, mut most) = (0i64, 0i64);
        let mut drift = 0i64;
        while s.time() < LUNAR_DAY {
            s.step();
            drift = drift.max((s.stored_units() - units).abs());
            if s.steps % 60 == 0 && s.time() > 60.0 {
                let want = (s.eta[199] - s.eta[0]) >> S;
                let got = s.surface(199) - s.surface(0);
                worst = worst.max((got - want).abs());
                most = most.max(want.abs());
            }
        }
        (worst as f64 * MM, most as f64 * MM, drift)
    }

    #[test]
    fn a_closed_basin_tilts_with_the_tide_and_keeps_its_units() {
        let (today, most, d0) = basin(TODAY);
        let (links, _, d1) = basin((true, Settle::Off));
        let (proposed, _, d2) = basin(PROPOSED);
        println!("basin: the potential tilts it up to {most:.1} mm end to end; off that: today {today:.2} mm, term on links alone {links:.2} mm, term + tide settle {proposed:.2} mm; units drift {d0} {d1} {d2}");
        assert_eq!((d0, d1, d2), (0, 0, 0), "units exact");
        assert!(today > DEAD_MM && proposed < DEAD_MM, "{today} {proposed}");
    }

    /// A 200 x 5 band behind a harbour mouth (the sea at x = 0); at a quarter cycle a block is set in (100, 2); the
    /// worst |level - sea's level| of its four neighbours after that, mm.
    fn block((term, settle): (bool, Settle)) -> (f64, i64) {
        let (w, h) = (201, 5);
        let mut s = Sim::new(w, h, R, 0.3, 2, vec![luna(1.0)]);
        (s.term, s.settle, s.band) = (term, settle, DEAD / 4);
        for y in 0..h {
            s.sea[y * w] = true;
        }
        s.lay_all();
        s.wake_all();
        let b = 2 * w + 100;
        let nbs = [b - 1, b + 1, b - w, b + w];
        let mut worst = 0i64;
        let start = s.all_units() + s.sea_extra;
        while s.time() < LUNAR_DAY {
            s.step();
            if s.steps == (LUNAR_DAY / 4.0 * 60.0) as u64 {
                s.set_solid(b);
            }
            if s.solid[b] {
                worst = worst.max(nbs.iter().map(|&i| (s.surface(i) - s.sea_level(i)).abs()).max().unwrap());
            }
        }
        // the ledger: stored + what the sea took + what the block displaced
        let ledger = s.all_units() + s.sea_extra + s.removed - start;
        (worst as f64 * MM, ledger)
    }

    #[test]
    fn a_block_in_the_band_leaves_the_sea_whole() {
        let (today, l0) = block(TODAY);
        let (proposed, l1) = block(PROPOSED);
        println!("block: its neighbours off the sea: today {today:.2} mm, proposed {proposed:.2} mm; ledgers {l0} {l1}");
        assert_eq!((l0, l1), (0, 0));
        assert!(today > DEAD_MM && proposed < DEAD_MM, "{today} {proposed}");
    }

    /// The rest-state rule: a 200-column stretch laid at the formula's level, stepped 60 s: the fastest link and
    /// the most any level moved off the formula, mm.
    fn rest((term, settle): (bool, Settle), range: f64) -> (i64, f64) {
        let mut s = Sim::new(200, 1, R, 0.3, 2, vec![luna(range)]);
        (s.term, s.settle, s.band) = (term, settle, DEAD / 4);
        // the tide frozen (rate 0) at its steepest over the stretch: the rule is about the state, not its motion
        s.raisers[0].rate = 0.0;
        s.raisers[0].phase = 0.3 + 100.0 / R - std::f64::consts::FRAC_PI_4;
        s.retide_all();
        s.lay_all();
        s.wake_all();
        let (mut vmax, mut off) = (0i64, 0i64);
        for _ in 0..3600 {
            s.step();
            vmax = vmax.max(s.v.iter().flatten().map(|&v| v as i64).max().unwrap());
            off = off.max((0..200).map(|i| (s.surface(i) - s.sea_level(i)).abs()).max().unwrap());
        }
        (vmax, off as f64 * MM)
    }

    #[test]
    fn untouched_water_laid_at_the_formula_does_not_move() {
        for range in [1.0, 10.0] {
            let (v0, o0) = rest(TODAY, range);
            let (v1, o1) = rest(PROPOSED, range);
            println!("rest, {range} m range: today vmax {v0} off {o0:.2} mm; proposed vmax {v1} off {o1:.2} mm (VSTOP {VSTOP})");
            // at 1 m the slope a column (under a unit) is inside VSTOP's dead band: today's water holds it too
            if range > 5.0 {
                assert!(v0 >= VSTOP && o0 > 10.0, "today's links level it");
            }
            assert!(v1 < VSTOP && o1 < 1.0);
        }
    }
}

#[cfg(test)]
mod probe {
    use super::*;
    #[test]
    #[ignore]
    fn links_alone() {
        let mut s = Sim::new(200, 1, R, 0.3, 2, vec![luna(1.0)]);
        (s.term, s.settle) = (true, Settle::Off);
        s.lay_all();
        s.wake_all();
        while s.time() < LUNAR_DAY {
            s.step();
            if s.steps % 1800 == 0 {
                let want = (s.eta[199] - s.eta[0]) >> S;
                let got = s.surface(199) - s.surface(0);
                let vmax = s.v.iter().flatten().max().unwrap();
                println!("t {:.0} want {want} got {got} awake {} vmax {vmax}", s.time(), s.awake_count());
            }
        }
    }
}
