//! Ground water's table (ADR 0190; the spike `prototypes/ground-water`): the rest state of Dupuit-Darcy flow with
//! the rain's recharge, solved per 8 m map cell with the sea, the lakes and the rivers as fixed heads and the ground
//! as a ceiling. Where the solve presses against the ground the cell seeps: a spring, whose flow is the recharge
//! uphill of it. Steady, unconfined, one head a cell:
//!
//!     div(K b grad h) + R = 0   under the ground;   h = ground, spring = R + div(K b grad h) >= 0 where it would rise over it
//!
//! with b = h - base the saturated thickness. A link's conductance is the harmonic mean of the two cells' K times
//! their mean thickness (the spike's: with it the Dupuit parabola is the discrete model's exact rest).
use super::grid::NONE;

/// A cell with no aquifer under it (tight rock at the ground): no table, no ground water.
pub const DRY: f32 = -1.0e4;
/// A spring giving at least this, m^3/s, starts a visible stream (ADR 0190, "springs start visible streams"): a litre
/// a second, a spring a walker sees run. shortcut: one threshold; by the stream's look once small streams have one.
pub const STREAM: f64 = 1.0e-3;
/// The bake's solve stops when no cell's balance asks for more than this, metres: under half a unit of water
/// (1/4096 m).
pub const TOL: f64 = 1.0e-4;

/// The cells as the solve sees them: ground (the ceiling) and the aquifer's floor, metres; conductivity, m/s (0: no
/// aquifer); recharge, m^3/s a cell; a fixed head, metres (NaN: free); the four neighbours (NONE: none); the cell's
/// width, m.
pub struct Aquifer {
    pub ground: Vec<f64>,
    pub base: Vec<f64>,
    pub k: Vec<f64>,
    pub q: Vec<f64>,
    pub fixed: Vec<f64>,
    pub nb: Vec<[u32; 4]>,
}

impl Aquifer {
    /// The conductance of the link between cells i and j at heads hi, hj, m^2/s: flow i -> j is c (hi - hj).
    pub fn cond(&self, i: usize, j: usize, hi: f64, hj: f64) -> f64 {
        let (ki, kj) = (self.k[i], self.k[j]);
        let kl = if ki > 0.0 && kj > 0.0 { 2.0 * ki * kj / (ki + kj) } else { 0.0 };
        let bi = (hi.min(self.ground[i]) - self.base[i]).max(0.0);
        let bj = (hj.min(self.ground[j]) - self.base[j]).max(0.0);
        kl * 0.5 * (bi + bj)
    }
    fn free(&self, i: usize) -> bool {
        self.fixed[i].is_nan() && self.k[i] > 0.0
    }
    fn links(&self, i: usize) -> impl Iterator<Item = usize> + '_ {
        self.nb[i].iter().filter(|&&j| j != NONE).map(|&j| j as usize)
    }

    /// The rest: Gauss-Seidel with over-relaxation `omega` from the ground, in index order (deterministic), until no
    /// free cell's imbalance asks it to move more than `tol` metres (`residual`), at most `max` sweeps. Heads, sweeps.
    pub fn solve(&self, omega: f64, tol: f64, max: usize) -> (Vec<f64>, usize) {
        let mut h: Vec<f64> = (0..self.k.len()).map(|i| if self.fixed[i].is_nan() { self.ground[i].max(self.base[i]) } else { self.fixed[i] }).collect();
        for sweep in 1..=max {
            let mut moved = 0.0f64;
            for i in 0..h.len() {
                if !self.free(i) {
                    continue;
                }
                // the cell's own balance f(h) = q - sum c(h) (h - hj) and its slope: a link's conductance grows with
                // the cell's thickness (K/2 a metre, between its floor and the ground), so f is quadratic near a dry
                // neighbour and the linear (Picard) target overshoots its root to a 2-cycle; a Newton step does not
                let (mut f, mut df, mut sc, mut sch) = (self.q[i], 0.0, 0.0, 0.0);
                let wet = h[i] > self.base[i] && h[i] < self.ground[i];
                for j in self.links(i) {
                    let c = self.cond(i, j, h[i], h[j]);
                    let (ki, kj) = (self.k[i], self.k[j]);
                    let dc = if wet && ki > 0.0 && kj > 0.0 { ki * kj / (ki + kj) } else { 0.0 };
                    f -= c * (h[i] - h[j]);
                    df -= c + dc * (h[i] - h[j]);
                    sc += c;
                    sch += c * h[j];
                }
                // Newton only while its slope is at least the links' (a cell far under a neighbour flattens it)
                let target = if df <= -sc && df < 0.0 {
                    h[i] - f / df
                } else if sc > 0.0 {
                    (self.q[i] + sch) / sc
                } else {
                    continue;
                }
                .clamp(self.base[i], self.ground[i]);
                let new = (h[i] + omega * (target - h[i])).clamp(self.base[i], self.ground[i]);
                moved = moved.max((new - h[i]).abs());
                h[i] = new;
            }
            // a small move a sweep is no proof under over-relaxation: the balance is
            if moved < tol && sweep % 25 == 0 && self.residual(&h) < tol {
                return (h, sweep);
            }
        }
        (h, max)
    }

    /// The largest imbalance (recharge less Darcy out) of a free cell as the move of its head that would right it,
    /// metres (the imbalance over its links' conductance): a cell held at the ground may only seep (gain), one at its
    /// floor only lose; a cell with no conductance has no balance to right.
    pub fn residual(&self, h: &[f64]) -> f64 {
        (0..h.len())
            .filter(|&i| self.free(i))
            .map(|i| {
                let (mut r, mut sc) = (self.q[i], 0.0);
                for j in self.links(i) {
                    let c = self.cond(i, j, h[i], h[j]);
                    r -= c * (h[i] - h[j]);
                    sc += c;
                }
                let r = if h[i] >= self.ground[i] - 1e-9 { r.min(0.0) } else if h[i] <= self.base[i] { r.max(0.0) } else { r };
                if sc > 0.0 { r.abs() / sc } else { 0.0 }
            })
            .fold(0.0, f64::max)
    }

    /// Each seeping cell's spring, m^3/s (0 elsewhere): its recharge and what flows into it, held at the ground.
    pub fn springs(&self, h: &[f64]) -> Vec<f64> {
        (0..h.len())
            .map(|i| {
                if !self.free(i) || h[i] < self.ground[i] - 1e-6 {
                    return 0.0;
                }
                (self.q[i] + self.links(i).map(|j| self.cond(i, j, h[i], h[j]) * (h[j] - h[i])).sum::<f64>()).max(0.0)
            })
            .collect()
    }
}

/// Recharge, m/s (real time), of rain `rain` mm a day less evaporation `pet` mm a day: `share` of what runs off or
/// soaks in (`life`'s rule, at least a tenth of the rain).
pub fn recharge(rain: f32, pet: f32, share: f32) -> f64 {
    share as f64 * (rain - pet).max(0.1 * rain) as f64 * 1e-3 / 86400.0
}

/// The table of a bake on plates once its `life` has run, metres over the sea a cell (`DRY` on tight rock), and each
/// cell's spring, m^3/s. `n`: the planet's columns per face edge; `pet_per_c` the life's evaporation a degree.
pub fn table(b: &super::land::Bake, l: &super::life::Life, n: u32, pet_per_c: f32) -> (Vec<f32>, Vec<f64>) {
    let t = std::time::Instant::now();
    let g = &b.g;
    let (m, cells) = (g.m as i32, g.n());
    let dx = n as f64 / g.m as f64;
    let rocks = &*crate::rock::ROCKS;
    let bottom = -(crate::world::DEEP - crate::world::SEA_DEPTH) as f32;
    let river: Vec<bool> = {
        let mut v = vec![false; cells];
        for r in &b.rivers {
            v[r.cell as usize] = true;
        }
        v
    };
    let mut a = Aquifer { ground: vec![0.0; cells], base: vec![0.0; cells], k: vec![0.0; cells], q: vec![0.0; cells], fixed: vec![f64::NAN; cells], nb: vec![[NONE; 4]; cells] };
    for c in 0..cells {
        let ground = b.h[c];
        a.ground[c] = ground as f64;
        if let Some((k, _, floor)) = b.rock.get(c).and_then(|&p| rocks.aquifer(p, ground, bottom)) {
            (a.k[c], a.base[c]) = (k as f64, floor as f64);
        } else {
            a.base[c] = ground as f64;
        }
        if b.ocean[c] {
            a.fixed[c] = 0.0;
        } else if b.lake(c) {
            a.fixed[c] = b.lvl[c] as f64;
        } else if river[c] {
            a.fixed[c] = ground as f64;
        } else {
            a.q[c] = recharge(l.rain[c], pet_per_c * l.temp[c].max(0.0), rocks.recharge) * dx * dx;
        }
    }
    a.nb = links(g.m);
    let tol: f64 = std::env::var("GW_TOL").ok().and_then(|s| s.parse().ok()).unwrap_or(TOL);
    let (h, sweeps) = a.solve(1.95, tol, 200_000);
    let springs = a.springs(&h);
    let table = (0..cells).map(|c| if a.k[c] > 0.0 { h[c] as f32 } else { DRY }).collect();
    if let Ok(p) = std::env::var("GW_DUMP") {
        std::fs::write(p, h.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>()).unwrap();
    }
    eprintln!("[bake] ground water: {sweeps} sweeps, residual {:.1e} m, {} seeping cells: {:.1} s", a.residual(&h), springs.iter().filter(|&&s| s > 0.0).count(), t.elapsed().as_secs_f64());
    (table, springs)
}

/// Each cell's four neighbours on the map's grid of `m` cells a face edge, folded over the seams.
fn links(m: usize) -> Vec<[u32; 4]> {
    let m = m as i32;
    (0..6 * m * m)
        .map(|c| {
            let (f, i, j) = ((c / (m * m)) as usize, c % m, (c / m) % m);
            [(i - 1, j), (i + 1, j), (i, j - 1), (i, j + 1)].map(|(x, y)| {
                let (f, x, y) = crate::world::fold(f, x, y, m);
                ((f as i32 * m + y) * m + x) as u32
            })
        })
        .collect()
}

/// The streams the springs start (ADR 0190): from each spring giving `STREAM` or more, in index order, its cells
/// down the drainage (`recv`) until a river, a lake, the sea or a stream already laid; downstream first, as the
/// bake's rivers are, to go after them.
pub fn streams(b: &super::land::Bake, springs: &[f64]) -> Vec<super::land::River> {
    let mut wet = vec![false; b.h.len()];
    for r in &b.rivers {
        wet[r.cell as usize] = true;
    }
    let mut out = Vec::new();
    for c in 0..springs.len() {
        if springs[c] < STREAM || wet[c] || b.ocean[c] || b.lake(c) {
            continue;
        }
        let mut chain = Vec::new();
        let mut at = c as u32;
        while at != NONE && !wet[at as usize] && !b.ocean[at as usize] && !b.lake(at as usize) {
            wet[at as usize] = true;
            chain.push(at);
            at = b.recv[at as usize];
        }
        // a chain that runs off the drainage (no receiver) ends at its last cell's: it is not laid
        if at == NONE {
            continue;
        }
        out.extend(chain.iter().rev().map(|&k| super::land::River { cell: k, to: b.recv[k as usize], area: b.area[k as usize] }));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A strip of `n` cells `dx` wide between two streams at its ends whose surface stands `b0` over a flat floor at
    /// 0, the ground far over it, rock of `k`, recharge `r` m/s.
    fn strip(n: usize, dx: f64, b0: f64, k: f64, r: f64) -> Aquifer {
        let mut a = Aquifer { ground: vec![1000.0; n], base: vec![0.0; n], k: vec![k; n], q: vec![r * dx * dx; n], fixed: vec![f64::NAN; n], nb: vec![[NONE; 4]; n] };
        for i in 0..n {
            a.nb[i] = [if i > 0 { i as u32 - 1 } else { NONE }, if i + 1 < n { i as u32 + 1 } else { NONE }, NONE, NONE];
        }
        a.fixed[0] = b0;
        a.fixed[n - 1] = b0;
        a
    }
    /// Dupuit's parabola between two equal heads l apart on a flat floor: b^2 = b0^2 + (R/K) x (l - x).
    fn dupuit(b0: f64, r: f64, k: f64, x: f64, l: f64) -> f64 {
        (b0 * b0 + r / k * x * (l - x)).sqrt()
    }

    /// A ridge between two streams 800 m apart, 10 m of water over a flat floor at each, 600 mm a year of recharge
    /// in fine sand: the table is Dupuit's parabola (within a unit of water, 1/4096 m, everywhere).
    #[test]
    fn a_ridge_between_two_streams_stands_on_dupuits_parabola() {
        let (n, dx, k, r) = (101, 8.0, 1e-5, 0.6 / (365.25 * 86400.0));
        let a = strip(n, dx, 10.0, k, r);
        let (h, sweeps) = a.solve(1.9, 1e-9, 200_000);
        let l = (n - 1) as f64 * dx;
        let off = (0..n).map(|i| (h[i] - dupuit(10.0, r, k, i as f64 * dx, l)).abs()).fold(0.0, f64::max);
        println!("ridge: {sweeps} sweeps, the middle {:.4} m (Dupuit {:.4}), {:.3} units off at most", h[n / 2], dupuit(10.0, r, k, l / 2.0, l), off * 4096.0);
        assert!(off * 4096.0 < 1.0, "{off} m off the parabola");
        assert!(h[n / 2] > 10.0 + 1.0, "a mound between the streams");
    }

    /// A dry climate's table is deeper than a wet one's by the rule: the mound over the streams goes as R / K
    /// (b^2 - b0^2 = R/K x (l - x)), so a quarter of the recharge gives a quarter of b^2 - b0^2 in the middle, and
    /// the table stands that much deeper under the same ground.
    #[test]
    fn a_dry_climates_table_is_deeper_than_a_wet_ones_by_the_rule() {
        let (n, dx, k) = (101, 8.0, 1e-5);
        // rain less evaporation 3 mm a day in the wet one, a quarter of it in the dry one (the share of rocks.toml)
        let share = crate::rock::ROCKS.recharge;
        let (wet, dry) = (recharge(4.0, 1.0, share), recharge(1.75, 1.0, share));
        assert!((wet / dry - 4.0).abs() < 1e-9, "{wet} {dry}");
        let mid = |r: f64| strip(n, dx, 10.0, k, r).solve(1.9, 1e-9, 200_000).0[n / 2];
        let (hw, hd) = (mid(wet), mid(dry));
        let ratio = (hw * hw - 100.0) / (hd * hd - 100.0);
        println!("the middle's table: wet {hw:.3} m, dry {hd:.3} m over the floor; b^2 - b0^2 in the ratio {ratio:.4}");
        assert!(hd < hw && (ratio - 4.0).abs() < 1e-3, "{hw} {hd} {ratio}");
    }

    /// Every link the solve sees runs both ways, over the face seams too (a one-way link makes the system
    /// unsymmetric, and over-relaxation then diverges).
    #[test]
    fn the_cells_links_run_both_ways() {
        let m = 96;
        let nb = links(m);
        let mut bad = Vec::new();
        for c in 0..nb.len() {
            for &j in &nb[c] {
                if !nb[j as usize].contains(&(c as u32)) {
                    bad.push((c, j));
                }
            }
        }
        assert!(bad.is_empty(), "{} one-way links, first {:?}", bad.len(), &bad[..bad.len().min(6)]);
    }

    /// A slope from a divide down to a cut where the ground meets the table: the cut seeps (a spring) and gives exactly
    /// what rained in uphill of it; nothing seeps uphill.
    #[test]
    fn a_spring_gives_the_recharge_uphill_of_it() {
        let (n, dx, k, r) = (40, 8.0, 1e-5, 1.0 / (365.25 * 86400.0));
        let mut a = strip(n, dx, 0.0, k, r);
        a.fixed[0] = f64::NAN;
        a.fixed[n - 1] = f64::NAN;
        a.nb[n - 1][1] = NONE;
        for i in 0..n {
            a.ground[i] = 4.0 + 1.6 * (n - 1 - i) as f64;
        }
        let (h, _) = a.solve(1.9, 1e-9, 400_000);
        let s = a.springs(&h);
        let rain: f64 = a.q.iter().sum();
        println!("spring {:.6e} m3/s against {rain:.6e} rained in; seeping {:?}", s[n - 1], (0..n).filter(|&i| s[i] > 0.0).collect::<Vec<_>>());
        assert!((s.iter().sum::<f64>() / rain - 1.0).abs() < 1e-6);
        assert!(s[..n - 3].iter().all(|&s| s == 0.0) && s[n - 1] > 0.0, "the spring at the foot of the slope only");
    }
}
