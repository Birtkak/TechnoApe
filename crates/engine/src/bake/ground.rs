//! Ground water's table (ADR 0190; the spike `prototypes/ground-water`): the rest state of Dupuit-Darcy flow with
//! the rain's recharge, solved on `EDGE` cells a face edge with the sea, the lakes and the rivers as fixed heads and the ground
//! as a ceiling. Where the solve presses against the ground the cell seeps: a spring, whose flow is the recharge
//! uphill of it. Steady, unconfined, one head a cell:
//!
//!     div(K b grad h) + R = 0   under the ground;   h = ground, spring = R + div(K b grad h) >= 0 where it would rise over it
//!
//! with b = h - base the saturated thickness. A link's conductance is the harmonic mean of the two cells' K times
//! their mean thickness over the higher of their two floors (with equal floors the spike's, whose rest is the Dupuit
//! parabola exactly; over the higher floor, a cell's outflow only grows with its head, so each cell's balance has one
//! root: with each cell's own floor two cells on a step between floors can hold two, and the sweep never settles).
use super::grid::NONE;

/// A cell with no aquifer under it (tight rock at the ground): no table, no ground water.
pub const DRY: f32 = -1.0e4;
/// A spring giving at least this, m^3/s, starts a visible stream (ADR 0190, "springs start visible streams"): a litre
/// a second, a spring a walker sees run. shortcut: one threshold; by the stream's look once small streams have one.
pub const STREAM: f64 = 1.0e-3;
/// The bake's solve stops when no cell's balance asks for more than this, metres (a millimetre: over-relaxed sweeps
/// on the planets settle to a fifth of it, then a few cells by a seeping edge swing by 0.1 mm for good).
pub const TOL: f64 = 1.0e-3;
/// The most sweeps the bake's solve takes (the planets settle in about 3,000); a field not settled by then is kept
/// as it stands, deterministic all the same.
pub const SWEEPS: usize = 20_000;

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
        let fl = self.base[i].max(self.base[j]);
        let bi = (hi.min(self.ground[i]) - fl).max(0.0);
        let bj = (hj.min(self.ground[j]) - fl).max(0.0);
        self.kl(i, j) * 0.5 * (bi + bj)
    }
    /// The link's K: the harmonic mean of its cells' (0 where either has no aquifer).
    fn kl(&self, i: usize, j: usize) -> f64 {
        let (ki, kj) = (self.k[i], self.k[j]);
        if ki > 0.0 && kj > 0.0 { 2.0 * ki * kj / (ki + kj) } else { 0.0 }
    }
    fn free(&self, i: usize) -> bool {
        self.fixed[i].is_nan() && self.k[i] > 0.0
    }
    fn links(&self, i: usize) -> impl Iterator<Item = usize> + '_ {
        self.nb[i].iter().filter(|&&j| j != NONE).map(|&j| j as usize)
    }

    /// The rest: Gauss-Seidel with over-relaxation `omega` from the ground, in index order (deterministic), each cell
    /// moved toward the head that rights its own balance (`root`), until no free cell's imbalance asks it to move more
    /// than `tol` metres (`residual`), at most `max` sweeps. Heads, sweeps. Every 100 sweeps the balance is weighed and
    /// when it has not shrunk the over-relaxation is halved toward 1: over-relaxed, a seeping edge on a planet can
    /// swing for good (metres after 20,000 sweeps on the 512 and 1536 planets), while plain Gauss-Seidel on this balance (each
    /// cell's outflow grows with its head and falls with its neighbours') only closes in on the rest.
    pub fn solve(&self, omega: f64, tol: f64, max: usize) -> (Vec<f64>, usize) {
        let mut h: Vec<f64> = (0..self.k.len()).map(|i| if self.fixed[i].is_nan() { self.ground[i].max(self.base[i]) } else { self.fixed[i] }).collect();
        let (mut omega, mut last) = (omega, f64::INFINITY);
        for sweep in 1..=max {
            if sweep % 100 == 0 {
                let r = self.residual(&h);
                if r >= last {
                    omega = 1.0 + 0.5 * (omega - 1.0);
                }
                last = r;
            }
            let mut moved = 0.0f64;
            for i in 0..h.len() {
                if !self.free(i) {
                    continue;
                }
                let target = self.root(i, &h);
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

    /// Cell i's own balance f(x) = q - sum c(x) (x - hj) at head x with its neighbours' heads held, and its slope.
    fn balance(&self, i: usize, h: &[f64], x: f64) -> (f64, f64) {
        let (mut f, mut df) = (self.q[i], 0.0);
        for j in self.links(i) {
            let fl = self.base[i].max(self.base[j]);
            let c = self.cond(i, j, x, h[j]);
            let dc = if x > fl && x < self.ground[i] { 0.5 * self.kl(i, j) } else { 0.0 };
            f -= c * (x - h[j]);
            df -= c + dc * (x - h[j]);
        }
        (f, df)
    }

    /// The head that rights cell i's balance with its neighbours' held: f falls as the head rises (over the link's
    /// floor, a link's outflow only grows with it), so its one root lies in [floor, ground], or the cell seeps at the
    /// ground or lies dry on its floor. Newton inside a bisection bracket: a single Newton step overshoots where f is
    /// flat (a cell far under a neighbour) or quadratic (by a dry one), and the sweep then swings in a checkerboard.
    fn root(&self, i: usize, h: &[f64]) -> f64 {
        let (mut lo, mut hi) = (self.base[i], self.ground[i]);
        if self.balance(i, h, hi).0 >= 0.0 {
            return hi;
        }
        if self.balance(i, h, lo).0 <= 0.0 {
            return lo;
        }
        let mut x = h[i].clamp(lo, hi);
        for _ in 0..40 {
            let (f, df) = self.balance(i, h, x);
            if f > 0.0 { lo = x } else { hi = x }
            let n = if df < 0.0 { x - f / df } else { f64::NAN };
            let next = if n > lo && n < hi { n } else { 0.5 * (lo + hi) };
            if (next - x).abs() < 1e-9 || hi - lo < 1e-9 {
                return next;
            }
            x = next;
        }
        x
    }

    /// The largest move a free cell's own balance asks of its head with its neighbours' held (`root`), metres: a cell
    /// held at the ground that gains, or one on its floor that loses, asks none. (The imbalance over the links'
    /// conductance asked metres of a cell lying dry on its floor, where no water flows at all.)
    pub fn residual(&self, h: &[f64]) -> f64 {
        (0..h.len()).filter(|&i| self.free(i)).map(|i| (self.root(i, h) - h[i]).abs()).fold(0.0, f64::max)
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

/// The table's own grid, cells a face edge on every planet (ADR 0190 as built). The solve's sweeps go as the square of
/// the cells along an edge and each sweep as their number, so its time goes as the edge's fourth power: 96 an edge
/// (the small planet at 8 m) settles in 4.3 s, 768 (the 4 km planet at 8 m) ran 3 hours unsettled. At 160 it is about
/// half a minute on every planet. The table is a smooth surface between drains hundreds of metres apart (cells of
/// 38 m on the 4 km planet, 5 m on the small one, 77 m on the huge one); where it meets the ground (springs, flooded
/// caves, a dug hole) is read from the bilinear head against each column's own ground, not from these cells.
pub const EDGE: usize = 160;

/// The table of a bake on plates once its `life` has run, metres over the sea a map cell (`DRY` on tight rock), and
/// each map cell's spring, m^3/s. `n`: the planet's columns per face edge; `pet_per_c` the life's evaporation a
/// degree. Solved on `EDGE` cells a face edge: each the mean of the map cells it covers (ground, the floor and K of
/// those with an aquifer, their recharge), held at the lowest drain among them (sea, lake, river); read back at each
/// map cell's middle bilinearly, under its own ground. A solve cell's spring goes to the lowest map cell under it
/// where the table so read meets the ground.
pub fn table(b: &super::land::Bake, l: &super::life::Life, n: u32, pet_per_c: f32) -> (Vec<f32>, Vec<f64>) {
    let t = std::time::Instant::now();
    let (g, rocks) = (&b.g, &*crate::rock::ROCKS);
    let (m, cells, e) = (g.m, g.n(), EDGE);
    let bottom = -(crate::world::DEEP - crate::world::SEA_DEPTH) as f32;
    let mut river = vec![false; cells];
    for r in &b.rivers {
        river[r.cell as usize] = true;
    }
    // each map cell: its aquifer (K, floor), its drain's head (NaN: none), its recharge, m/s
    let aq: Vec<Option<(f64, f64)>> = (0..cells).map(|c| b.rock.get(c).and_then(|&p| rocks.aquifer(p, b.h[c], bottom)).map(|(k, _, f)| (k as f64, f as f64))).collect();
    let drain: Vec<f64> = (0..cells)
        .map(|c| if b.ocean[c] { 0.0 } else if b.lake(c) { b.lvl[c] as f64 } else if river[c] { b.h[c] as f64 } else { f64::NAN })
        .collect();
    let span = |a: usize| a * m / e..((a + 1) * m).div_ceil(e).clamp(a * m / e + 1, m);
    let under = |c: usize| {
        let (f, x, y) = (c / (e * e), c % e, (c / e) % e);
        span(y).flat_map(move |j| span(x).map(move |i| (f * m + j) * m + i))
    };
    let k = e * e * 6;
    let dx = n as f64 / e as f64;
    let mut a = Aquifer { ground: vec![0.0; k], base: vec![0.0; k], k: vec![0.0; k], q: vec![0.0; k], fixed: vec![f64::NAN; k], nb: links(e) };
    for c in 0..k {
        let (mut ground, mut cnt, mut kk, mut base, mut wet, mut rain, mut dry, mut low) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, f64::INFINITY);
        for i in under(c) {
            (ground, cnt) = (ground + b.h[i] as f64, cnt + 1.0);
            if let Some((ki, fi)) = aq[i] {
                (kk, base, wet) = (kk + ki, base + fi, wet + 1.0);
            }
            if drain[i].is_nan() {
                rain += recharge(l.rain[i], pet_per_c * l.temp[i].max(0.0), rocks.recharge);
                dry += 1.0;
            } else {
                low = low.min(drain[i]);
            }
        }
        a.ground[c] = ground / cnt;
        if wet > 0.0 {
            (a.k[c], a.base[c]) = (kk / wet, (base / wet).min(a.ground[c]));
        } else {
            a.base[c] = a.ground[c];
        }
        if low.is_finite() {
            a.fixed[c] = low;
        } else {
            a.q[c] = rain / dry * dx * dx;
        }
    }
    let (h, sweeps) = a.solve(1.9, TOL, SWEEPS);
    let solved = a.springs(&h);
    // back on the map's cells: bilinear between the four solve cells round its middle (clamped to the face), the
    // nearest one's where one of them is dry
    let wet = |f: usize, x: i64, y: i64| {
        let c = (f * e + y.clamp(0, e as i64 - 1) as usize) * e + x.clamp(0, e as i64 - 1) as usize;
        (a.k[c] > 0.0).then_some(h[c])
    };
    let table: Vec<f32> = (0..cells)
        .map(|c| {
            if aq[c].is_none() {
                return DRY;
            }
            if !drain[c].is_nan() {
                return drain[c] as f32;
            }
            let (f, i, j) = (c / (m * m), c % m, (c / m) % m);
            let (u, v) = ((i as f64 + 0.5) * e as f64 / m as f64 - 0.5, (j as f64 + 0.5) * e as f64 / m as f64 - 0.5);
            let (x0, y0) = (u.floor() as i64, v.floor() as i64);
            let (fx, fy) = (u - x0 as f64, v - y0 as f64);
            let hs = match [wet(f, x0, y0), wet(f, x0 + 1, y0), wet(f, x0, y0 + 1), wet(f, x0 + 1, y0 + 1)] {
                [Some(a0), Some(a1), Some(a2), Some(a3)] => (a0 * (1.0 - fx) + a1 * fx) * (1.0 - fy) + (a2 * (1.0 - fx) + a3 * fx) * fy,
                _ => match wet(f, (u + 0.5).floor() as i64, (v + 0.5).floor() as i64) {
                    Some(t) => t,
                    None => return DRY,
                },
            };
            hs.min(b.h[c] as f64) as f32
        })
        .collect();
    let mut springs = vec![0.0; cells];
    for c in 0..k {
        if solved[c] > 0.0 {
            let seep = under(c).filter(|&i| drain[i].is_nan() && table[i] != DRY && table[i] >= b.h[i] - 1e-3).min_by(|&x, &y| b.h[x].total_cmp(&b.h[y]).then(x.cmp(&y)));
            if let Some(i) = seep {
                springs[i] += solved[c];
            }
        }
    }
    eprintln!("[bake] ground water: {sweeps} sweeps on {e} cells a face edge, residual {:.1e} m, {} seeping cells: {:.1} s", a.residual(&h), solved.iter().filter(|&&s| s > 0.0).count(), t.elapsed().as_secs_f64());
    if let Ok(p) = std::env::var("GW_DUMP") {
        std::fs::write(p, table.iter().flat_map(|x| (*x as f64).to_le_bytes()).collect::<Vec<u8>>()).unwrap();
    }
    (table, springs)
}

/// Each cell's four neighbours on a grid of `m` cells a face edge, folded over the seams.
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
