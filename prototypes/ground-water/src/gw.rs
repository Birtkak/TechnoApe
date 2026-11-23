//! Ground water as counted units in rock pores (Dupuit: one head a column, flow by Darcy between columns,
//! recharge from rain), and the formula it settles to.
//!
//! Units: the engine's `liquid::U` = 4096 units a block (1 m3 of open water). Ground water moves far less than a
//! unit a step, so it is counted in micro-units, `SUB` to a unit: an integer ledger all the same, and it trades
//! whole units with open water (`liquid.rs`) where it meets a gap.

pub const U: i64 = 4096;
pub const SUB: i64 = 1 << 16;
/// Micro-units in a cubic metre of water.
pub const UM: f64 = (U * SUB) as f64;
/// A real year, seconds, and the game's (334 days of 1800 s).
pub const REAL_YEAR: f64 = 365.25 * 86400.0;
pub const GAME_DAY: f64 = 1800.0;
pub const GAME_YEAR: f64 = 334.0 * GAME_DAY;

/// A rectangle of columns, `dx` metres wide, stepped `dt` seconds at a time.
#[derive(Clone)]
pub struct Aquifer {
    pub nx: usize,
    pub ny: usize,
    pub dx: f64,
    pub dt: f64,
    /// The ground's top (seepage over it leaves as a spring) and the aquifer's floor (the top of rock that
    /// holds no water), metres.
    pub ground: Vec<f64>,
    pub base: Vec<f64>,
    /// Hydraulic conductivity, m/s, and specific yield (the share of the rock's volume that drains).
    pub k: Vec<f64>,
    pub sy: Vec<f64>,
    /// A void (a cave or a dug well) in the column, [lo, hi) metres: it holds water at porosity 1.
    pub void: Vec<Option<(f64, f64)>>,
    /// A fixed head: the sea, a river or lake, or the edge of a stored patch reading the formula.
    pub fixed: Vec<Option<f64>>,
    /// Micro-units of rain that reach the table, a step; micro-units a step asked of the void's water (a pump).
    pub recharge: Vec<i64>,
    pub pump: Vec<i64>,
    /// The water held, micro-units.
    pub w: Vec<i64>,
    /// The accounts: rain taken in, net given to the fixed heads, each column's springs, pumped out.
    pub rain_in: i64,
    pub to_fixed: i64,
    pub springs: Vec<i64>,
    pub pumped: i64,
    /// The ledger's opening balance; `check` asserts it every step.
    pub start: i64,
    pub check: bool,
}

/// Recharge `r` m/s on a column of `dx` m, as micro-units a step of `dt` s.
pub fn per_step(r: f64, dt: f64, dx: f64) -> i64 {
    (r * dt * dx * dx * UM).round() as i64
}

/// The steady Dupuit table between two equal fixed heads `l` apart, flat floor: b^2 = b0^2 + (R/K) x (l - x).
#[cfg_attr(not(test), allow(dead_code))]
pub fn dupuit(b0: f64, r: f64, k: f64, x: f64, l: f64) -> f64 {
    (b0 * b0 + r / k * x * (l - x)).sqrt()
}

/// The steady Dupuit mound under uniform recharge, flat floor, centre at 0: b^2 = c - R/(2K) (x^2 + y^2).
#[cfg_attr(not(test), allow(dead_code))]
pub fn mound(c: f64, r: f64, k: f64, x: f64, y: f64) -> f64 {
    (c - r / (2.0 * k) * (x * x + y * y)).sqrt()
}

impl Aquifer {
    pub fn flat(nx: usize, ny: usize, dx: f64, dt: f64, ground: f64, base: f64, k: f64, sy: f64) -> Self {
        let n = nx * ny;
        Aquifer {
            nx,
            ny,
            dx,
            dt,
            ground: vec![ground; n],
            base: vec![base; n],
            k: vec![k; n],
            sy: vec![sy; n],
            void: vec![None; n],
            fixed: vec![None; n],
            recharge: vec![0; n],
            pump: vec![0; n],
            w: vec![0; n],
            rain_in: 0,
            to_fixed: 0,
            springs: vec![0; n],
            pumped: 0,
            start: 0,
            check: true,
        }
    }

    /// The water column `i` holds with its head at `h` (floored to micro-units; nothing over the ground).
    pub fn storage(&self, i: usize, h: f64) -> i64 {
        let (b, g, s) = (self.base[i], self.ground[i], self.sy[i]);
        let h = h.clamp(b, g);
        let mut m = s * (h - b);
        if let Some((lo, hi)) = self.void[i] {
            let (lo, hi) = (lo.clamp(b, g), hi.clamp(b, g));
            m += (1.0 - s) * (h.clamp(lo, hi) - lo);
        }
        (m * self.dx * self.dx * UM).floor() as i64
    }

    /// The head of column `i` holding `w` micro-units (over the ground only for a moment, before it seeps).
    pub fn head(&self, i: usize, w: i64) -> f64 {
        let (b, g, s) = (self.base[i], self.ground[i], self.sy[i]);
        let mut rem = w as f64 / (self.dx * self.dx * UM);
        let segs = match self.void[i] {
            Some((lo, hi)) => {
                let lo = lo.clamp(b, g);
                let hi = hi.clamp(lo, g);
                [(b, lo, s), (lo, hi, 1.0), (hi, g, s)]
            }
            None => [(b, g, s), (g, g, s), (g, g, s)],
        };
        for (z0, z1, d) in segs {
            let cap = (z1 - z0) * d;
            if rem <= cap {
                return z0 + rem / d;
            }
            rem -= cap;
        }
        g + rem
    }

    /// The conductance of the link between neighbours `i` and `j` at heads `hi`, `hj`, m2/s: the harmonic
    /// mean of their conductivities times the mean saturated thickness (Dupuit). Flow i -> j is c (hi - hj).
    pub fn cond(&self, i: usize, j: usize, hi: f64, hj: f64) -> f64 {
        let (ki, kj) = (self.k[i], self.k[j]);
        let kl = if ki + kj > 0.0 { 2.0 * ki * kj / (ki + kj) } else { 0.0 };
        let bi = (hi.min(self.ground[i]) - self.base[i]).max(0.0);
        let bj = (hj.min(self.ground[j]) - self.base[j]).max(0.0);
        kl * 0.5 * (bi + bj)
    }

    pub fn neighbours(&self, i: usize) -> impl Iterator<Item = usize> {
        let (x, y, nx, ny) = (i % self.nx, i / self.nx, self.nx, self.ny);
        [(x > 0).then(|| i - 1), (x + 1 < nx).then(|| i + 1), (y > 0).then(|| i - nx), (y + 1 < ny).then(|| i + nx)]
            .into_iter()
            .flatten()
    }

    pub fn heads(&self) -> Vec<f64> {
        (0..self.w.len()).map(|i| self.head(i, self.w[i])).collect()
    }

    /// Lays the water at heads `h` and opens the ledger there.
    pub fn set_heads(&mut self, h: &[f64]) {
        for i in 0..h.len() {
            self.w[i] = self.storage(i, self.fixed[i].unwrap_or(h[i]));
        }
        self.open_ledger();
    }

    pub fn open_ledger(&mut self) {
        self.rain_in = 0;
        self.to_fixed = 0;
        self.pumped = 0;
        self.springs.iter_mut().for_each(|s| *s = 0);
        self.start = self.w.iter().sum();
    }

    /// Held + given to the fixed heads + springs + pumped - rain: never changes.
    pub fn balance(&self) -> i64 {
        self.w.iter().sum::<i64>() + self.to_fixed + self.springs.iter().sum::<i64>() + self.pumped - self.rain_in
    }

    /// Takes the void's water of column `i` out (a bucket, a drained cave), booked as pumped.
    pub fn drain(&mut self, i: usize) {
        let lo = self.void[i].expect("a void").0;
        let take = (self.w[i] - self.storage(i, lo)).max(0);
        self.w[i] -= take;
        self.pumped += take;
    }

    /// One explicit step: Darcy on every link (whole micro-units, the same number off one and onto the other),
    /// then rain, the pump, seepage over the ground, the fixed heads.
    pub fn step(&mut self) {
        let n = self.w.len();
        // a fixed head (the sea over its bed) stands where it stands, not where its column's water would
        let h: Vec<f64> = (0..n).map(|i| self.fixed[i].unwrap_or_else(|| self.head(i, self.w[i]))).collect();
        for i in 0..n {
            let (x, y) = (i % self.nx, i / self.nx);
            for j in [(x + 1 < self.nx).then(|| i + 1), (y + 1 < self.ny).then(|| i + self.nx)].into_iter().flatten() {
                if self.fixed[i].is_some() && self.fixed[j].is_some() {
                    continue;
                }
                // f64 then truncated: deterministic on IEEE hardware without fused multiply-add (Rust's default)
                let q = (self.cond(i, j, h[i], h[j]) * (h[i] - h[j]) * self.dt * UM) as i64;
                self.w[i] -= q;
                self.w[j] += q;
            }
        }
        for i in 0..n {
            if let Some(f) = self.fixed[i] {
                let t = self.storage(i, f);
                self.to_fixed += self.w[i] - t;
                self.w[i] = t;
                continue;
            }
            self.w[i] += self.recharge[i];
            self.rain_in += self.recharge[i];
            if self.pump[i] > 0 {
                let lo = self.void[i].map_or(self.base[i], |v| v.0); // a well smaller than the column: from the rock
                let take = self.pump[i].min((self.w[i] - self.storage(i, lo)).max(0));
                self.w[i] -= take;
                self.pumped += take;
            }
            let cap = self.storage(i, self.ground[i]);
            if self.w[i] > cap {
                self.springs[i] += self.w[i] - cap;
                self.w[i] = cap;
            }
        }
        if self.check {
            assert_eq!(self.balance(), self.start, "the ledger");
            assert!(self.w.iter().all(|&w| w >= 0), "no column below empty");
        }
    }
}

/// The formula's field: the steady state of the same equations (every free column's Darcy flows balance its
/// rain; a column whose head would stand over the ground is held at the ground and seeps), by Gauss-Seidel
/// with over-relaxation `omega` from `h0`, until no column's imbalance is over `tol` of the rain (`residual`). Returns heads and sweeps.
pub fn solve(a: &Aquifer, h0: &[f64], omega: f64, tol: f64, max: usize) -> (Vec<f64>, usize) {
    let mut h = h0.to_vec();
    for (i, f) in a.fixed.iter().enumerate() {
        if let Some(f) = f {
            h[i] = *f;
        }
    }
    for sweep in 1..=max {
        let mut moved = 0.0f64;
        for i in 0..h.len() {
            if a.fixed[i].is_some() {
                continue;
            }
            let (mut sc, mut sch) = (0.0, 0.0);
            for j in a.neighbours(i) {
                let c = a.cond(i, j, h[i], h[j]);
                sc += c;
                sch += c * h[j];
            }
            if sc <= 0.0 {
                continue;
            }
            let q = a.recharge[i] as f64 / (a.dt * UM);
            let target = ((q + sch) / sc).clamp(a.base[i], a.ground[i]);
            let new = (h[i] + omega * (target - h[i])).clamp(a.base[i], a.ground[i]);
            moved = moved.max((new - h[i]).abs());
            h[i] = new;
        }
        // a small move a sweep is no proof under over-relaxation: the balance is
        if moved < 1e-3 && sweep % 25 == 0 && residual(a, &h) < tol {
            return (h, sweep);
        }
    }
    (h, max)
}

/// How far heads `h` are from steady: the largest imbalance (rain in less Darcy out) of a free column that does
/// not seep, as a share of the largest column's rain.
pub fn residual(a: &Aquifer, h: &[f64]) -> f64 {
    let qmax = a.recharge.iter().copied().max().unwrap_or(0).max(1) as f64 / (a.dt * UM);
    (0..h.len())
        .filter(|&i| a.fixed[i].is_none() && h[i] < a.ground[i] - 1e-9 && h[i] > a.base[i])
        .map(|i| {
            let out: f64 = a.neighbours(i).map(|j| a.cond(i, j, h[i], h[j]) * (h[i] - h[j])).sum();
            (a.recharge[i] as f64 / (a.dt * UM) - out).abs() / qmax
        })
        .fold(0.0, f64::max)
}

/// The largest head difference between `a` and `b` over the free columns, in units of open water (1/4096 m).
pub fn dev_units(aq: &Aquifer, a: &[f64], b: &[f64]) -> f64 {
    (0..a.len()).filter(|&i| aq.fixed[i].is_none()).map(|i| (a[i] - b[i]).abs()).fold(0.0, f64::max) * U as f64
}

/// A bake-like stretch 320 m square sampled every `dx` m (`n` = 320 / dx): the sea on its west edge, a river
/// down a valley at y = 160 m, two rocks in 40 m stripes, rain wetter to the east (316 to 631 mm a year), a hill
/// whose slopes the table cuts (springs). The same ground at any resolution, so the bake's 8 m field can be held
/// against the 1 m columns' own rest.
pub fn baked(n: usize, dx: f64, dt: f64) -> Aquifer {
    let mut a = Aquifer::flat(n, n, dx, dt, 0.0, -40.0, 1e-5, 0.1);
    for i in 0..n * n {
        // repeats every 320 m, so a whole face can be laid of it
        let (x, y) = (((i % n) as f64 * dx) % 320.0, ((i / n) as f64 * dx) % 320.0);
        let hill = 45.0 * (-((x - 200.0).powi(2) + (y - 230.0).powi(2)) / 90.0f64.powi(2)).exp();
        a.ground[i] = 1.0 + 0.12 * x + 0.08 * (y - 160.0).abs() + hill + 2.0 * (x / 37.0).sin() * (y / 23.0).cos();
        if ((y / 40.0) as usize) % 2 == 1 {
            a.k[i] = 1e-6; // a tighter rock in stripes
            a.sy[i] = 0.05;
        }
        a.recharge[i] = per_step(1e-8 * (1.0 + x / 320.0), dt, dx);
        if x == 0.0 {
            a.ground[i] = -5.0;
            a.fixed[i] = Some(0.0); // the sea
        } else if (y - 160.0).abs() < dx * 0.5 {
            a.fixed[i] = Some(a.ground[i] - 0.5); // the river's surface
        }
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A slice 101 columns long between two rivers whose heads stand 10 m over the floor; 600 mm of rain a
    /// year reaches the table; fine sand (K 1e-5 m/s, yield 0.15).
    fn slice() -> (Aquifer, Vec<f64>) {
        let (n, dx, dt, k) = (101, 1.0, 400.0, 1e-5);
        let mut a = Aquifer::flat(n, 1, dx, dt, 30.0, 0.0, k, 0.15);
        let r = per_step(0.6 / REAL_YEAR, dt, dx);
        a.recharge = vec![r; n];
        a.fixed[0] = Some(10.0);
        a.fixed[n - 1] = Some(10.0);
        let re = r as f64 / (dt * dx * dx * UM);
        let f = (0..n).map(|i| dupuit(10.0, re, k, i as f64 * dx, (n - 1) as f64 * dx)).collect();
        (a, f)
    }

    #[test]
    fn the_slice_formula_is_its_rest() {
        let (mut a, f) = slice();
        a.set_heads(&f);
        for _ in 0..20_000 {
            a.step();
        }
        let d = dev_units(&a, &a.heads(), &f);
        println!("slice at the formula, 20,000 steps: {d:.4} units off");
        assert!(d < 1.0, "{d}");
    }

    #[test]
    fn the_slice_settles_to_its_formula() {
        for start in [10.0, 25.0] {
            let (mut a, f) = slice();
            a.set_heads(&vec![start; a.nx]);
            for _ in 0..200_000 {
                a.step();
            }
            let d = dev_units(&a, &a.heads(), &f);
            println!("slice from {start} m: {d:.4} units off the formula");
            assert!(d < 1.0, "{d}");
        }
    }

    /// A 41 x 41 patch whose edge reads the mound's formula (as a stored patch's edge reads unstored ground).
    fn patch() -> (Aquifer, Vec<f64>) {
        let (n, dx, dt, k) = (41usize, 1.0, 10.0, 1e-4);
        let mut a = Aquifer::flat(n, n, dx, dt, 40.0, 0.0, k, 0.2);
        let r = per_step(0.9 / REAL_YEAR, dt, dx);
        a.recharge = vec![r; n * n];
        let re = r as f64 / (dt * dx * dx * UM);
        let c = 20.0f64.powi(2);
        let f: Vec<f64> = (0..n * n).map(|i| mound(c, re, k, (i % n) as f64 - 20.0, (i / n) as f64 - 20.0)).collect();
        for i in 0..n * n {
            let (x, y) = (i % n, i / n);
            if x == 0 || y == 0 || x == n - 1 || y == n - 1 {
                a.fixed[i] = Some(f[i]);
            }
        }
        (a, f)
    }

    #[test]
    fn the_patch_formula_is_its_rest_and_it_settles_there() {
        let (mut a, f) = patch();
        a.set_heads(&f);
        for _ in 0..20_000 {
            a.step();
        }
        let d = dev_units(&a, &a.heads(), &f);
        println!("patch at the formula, 20,000 steps: {d:.4} units off");
        assert!(d < 1.0, "{d}");
        let (mut a, f) = patch();
        a.set_heads(&vec![19.0; f.len()]);
        for _ in 0..60_000 {
            a.step();
        }
        let d = dev_units(&a, &a.heads(), &f);
        println!("patch from 19 m: {d:.4} units off the formula");
        assert!(d < 1.0, "{d}");
    }

    fn baked() -> Aquifer {
        super::baked(40, 8.0, 600.0)
    }

    #[test]
    fn the_baked_field_is_the_rest_and_springs_feed_the_heads() {
        let mut a = baked();
        let (f, sweeps) = solve(&a, &a.ground.clone(), 1.8, 1e-6, 200_000);
        let seeps = (0..f.len()).filter(|&i| a.fixed[i].is_none() && f[i] >= a.ground[i] - 1e-9).count();
        println!("baked field: {sweeps} sweeps, {seeps} seeping cells");
        assert!(seeps > 0, "the hill must cut the table somewhere");
        a.set_heads(&f);
        for _ in 0..20_000 {
            a.step();
        }
        let d = dev_units(&a, &a.heads(), &f);
        println!("baked field at the formula, 20,000 steps of 600 s: {d:.4} units off");
        assert!(d < 1.0, "{d}");
        // at rest, everything rained in leaves by the springs and the sea and river
        a.open_ledger();
        for _ in 0..1000 {
            a.step();
        }
        let out = a.to_fixed + a.springs.iter().sum::<i64>();
        println!("at rest over 1000 steps: rain {} out {} (springs {})", a.rain_in, out, a.springs.iter().sum::<i64>());
        assert!(((out - a.rain_in) as f64).abs() < 1e-4 * a.rain_in as f64);
        // from the sea's level everywhere it rises to the same field
        let mut b = baked();
        b.set_heads(&vec![0.0; f.len()]);
        for _ in 0..400_000 {
            b.step();
        }
        let d = dev_units(&b, &b.heads(), &f);
        println!("baked from sea level: {d:.4} units off the formula");
        assert!(d < 1.0, "{d}");
    }

    /// A flat table 20 m over the floor, held at the edges of a 61 x 61 patch, no rain.
    fn level(k: f64) -> Aquifer {
        let n = 61;
        let mut a = Aquifer::flat(n, n, 1.0, 10.0, 30.0, 0.0, k, 0.2);
        for i in 0..n * n {
            let (x, y) = (i % n, i / n);
            if x == 0 || y == 0 || x == n - 1 || y == n - 1 {
                a.fixed[i] = Some(20.0);
            }
        }
        a.set_heads(&vec![20.0; n * n]);
        a
    }
    const MID: usize = 30 * 61 + 30;

    /// Steps until column `i`'s head is within `units` of 20 m; the seconds it took.
    fn recover(a: &mut Aquifer, i: usize, units: f64) -> f64 {
        let mut s = 0;
        while (a.head(i, a.w[i]) - 20.0).abs() * U as f64 > units {
            a.step();
            s += 1;
            assert!(s < 200_000, "never recovered");
        }
        s as f64 * a.dt
    }

    #[test]
    fn a_cave_below_the_table_fills_to_it_and_one_above_stays_dry() {
        let mut a = level(1e-5);
        a.void[MID] = Some((17.0, 23.0)); // cut through the table
        let before = a.w[MID];
        a.drain(MID); // drained dry
        assert!(a.w[MID] < before);
        let t = recover(&mut a, MID, 1.0);
        let lvl = a.head(MID, a.w[MID]);
        println!("cave 17..23 m drained: back to the table ({lvl:.5} m) in {t:.0} s");
        assert!((lvl - 20.0).abs() * (U as f64) <= 1.0);
        // one above the table: the cut changes nothing, it never holds water
        let mut b = level(1e-5);
        b.void[MID] = Some((22.0, 26.0));
        for _ in 0..10_000 {
            b.step();
        }
        assert!(b.w[MID] <= b.storage(MID, 22.0), "dry");
        assert!((b.head(MID, b.w[MID]) - 20.0).abs() * (U as f64) <= 1.0);
    }

    #[test]
    fn a_well_fills_at_a_rate_set_by_the_rock() {
        let mut ts = vec![];
        for k in [1e-5, 1e-4] {
            let mut a = level(k);
            a.void[MID] = Some((15.0, 30.0)); // dug from the ground 5 m under the table
            a.drain(MID);
            let mut s = 0;
            while a.head(MID, a.w[MID]) < 15.0 + 0.9 * 5.0 {
                a.step();
                s += 1;
                assert!(s < 2_000_000, "never refilled");
            }
            ts.push(s as f64 * a.dt);
        }
        println!("well 90 % refilled: K 1e-5 in {:.0} s, K 1e-4 in {:.0} s, ratio {:.2}", ts[0], ts[1], ts[0] / ts[1]);
        assert!((8.0..12.0).contains(&(ts[0] / ts[1])));
    }

    #[test]
    fn pumping_draws_a_cone_and_a_dry_well_recovers() {
        let mut a = level(1e-4);
        a.void[MID] = Some((15.0, 30.0));
        let t = 1e-4 * 20.0;
        let q = 2e-3; // m3/s
        a.pump[MID] = (q * a.dt * UM) as i64;
        for _ in 0..60_000 {
            a.step();
        }
        let h = a.heads();
        let s = |r: usize| 20.0 - h[MID + r];
        let cone: Vec<f64> = (1..=25).map(s).collect();
        assert!(cone.windows(2).all(|p| p[0] > p[1]), "drawdown falls off with distance");
        let thiem = q / (2.0 * std::f64::consts::PI * t) * (8.0f64 / 2.0).ln();
        println!("cone: s(1) {:.3} s(2) {:.3} s(8) {:.3} m; s(2)-s(8) {:.4} against Thiem {:.4}", s(1), s(2), s(8), s(2) - s(8), thiem);
        assert!(((s(2) - s(8)) / thiem - 1.0).abs() < 0.1);
        // pumped harder than the rock gives: the well runs dry, the pump gets only the inflow
        a.pump[MID] *= 20;
        for _ in 0..20_000 {
            a.step();
        }
        let before = a.pumped;
        a.step();
        assert!(a.head(MID, a.w[MID]) < 15.0 + 1.0 / U as f64, "dry");
        assert!(a.pumped - before < a.pump[MID], "the pump gets less than it asks");
        a.pump[MID] = 0;
        let t = recover(&mut a, MID, 1.0);
        println!("dry well back within a unit of the table in {t:.0} s");
    }

    /// Held per 8 m cell, a 1 m well is a sub-cell feature: its level is the cell's head less Peaceman's
    /// well term Q / (2 pi T) ln(0.198 dx / r_w), r_w = 0.198 m being what a 1 m column is as a well.
    #[test]
    fn a_well_in_an_8_m_cell_stands_where_the_1_m_column_does() {
        let (q, k) = (2e-3, 1e-4);
        let steady = |n: usize, dx: f64, dt: f64, void: bool| {
            let mut a = Aquifer::flat(n, n, dx, dt, 30.0, 0.0, k, 0.2);
            for i in 0..n * n {
                let (x, y) = (i % n, i / n);
                if x == 0 || y == 0 || x == n - 1 || y == n - 1 {
                    a.fixed[i] = Some(20.0);
                }
            }
            a.set_heads(&vec![20.0; n * n]);
            let mid = n / 2 * n + n / 2;
            if void {
                a.void[mid] = Some((15.0, 30.0));
            }
            a.pump[mid] = (q * dt * UM) as i64;
            for _ in 0..200_000 {
                a.step();
            }
            a.head(mid, a.w[mid])
        };
        let col = steady(65, 1.0, 10.0, true);
        let cell = steady(9, 8.0, 200.0, false);
        let t = k * (cell - 0.0);
        let well = cell - q / (2.0 * std::f64::consts::PI * t) * (0.198f64 * 8.0 / 0.198).ln();
        println!("well drawdown: 1 m columns {:.4} m; 8 m cells + Peaceman {:.4} m (the cell's own {:.4})", 20.0 - col, 20.0 - well, 20.0 - cell);
        assert!(((20.0 - well) / (20.0 - col) - 1.0).abs() < 0.05);
    }

    #[test]
    fn a_spring_where_the_table_cuts_a_hillside_gives_the_recharge_uphill() {
        // a divide at x = 0, ground falling to a cut at the last column, 1 m of wet rain a year
        let (n, dt, k) = (60usize, 500.0, 1e-5);
        let mut a = Aquifer::flat(n, 1, 1.0, dt, 0.0, 0.0, k, 0.2);
        for i in 0..n {
            a.ground[i] = 4.0 + 0.2 * (n - 1 - i) as f64;
        }
        a.recharge = vec![per_step(1.0 / REAL_YEAR, dt, 1.0); n];
        a.set_heads(&vec![4.0; n]);
        for _ in 0..150_000 {
            a.step();
        }
        a.open_ledger();
        for _ in 0..10_000 {
            a.step();
        }
        let sp = a.springs[n - 1];
        println!("spring: {} micro-units against {} rained in over the window", sp, a.rain_in);
        assert!(((sp - a.rain_in) as f64).abs() < 1e-4 * a.rain_in as f64);
        assert!(a.springs[..n - 1].iter().all(|&s| s == 0), "no seep uphill");
    }
}
