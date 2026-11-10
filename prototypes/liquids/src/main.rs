//! Water spike (ADR 0020): measures the engine's column water against a voxel cellular automaton on the same
//! scenes. Prints one JSON line per measurement. `cargo run --release` from this folder.
use engine::liquid::test_ground::{col, level, stored, Box3};
use engine::liquid::{Liquids, U};
use std::time::Instant;

fn json(name: &str, fields: &[(&str, f64)]) {
    let f: Vec<String> = fields.iter().map(|(k, v)| format!("\"{k}\":{v:.4}")).collect();
    println!("{{\"scene\":\"{name}\",{}}}", f.join(","));
}

/// Steps until nothing is awake (or `max`); returns seconds.
fn settle(w: &mut Liquids, g: &Box3, max: usize) -> f64 {
    for k in 0..max {
        if w.awake() == 0 {
            return k as f64 / 60.0;
        }
        w.step(g);
    }
    f64::NAN
}

/// A dam breaks in a 60 m channel: 2 m of water behind it. Ritter: front at 2 sqrt(g h) = 8.86 m/s.
fn dam_columns() {
    let mut g = Box3::new(120, 1, 8);
    g.fill(0..120, 0..1, 0..1, true);
    let mut w = stored(&g);
    for x in 0..40 {
        w.pour(&g, col(x, 0), 1, 4 * U);
    }
    let t = w.total();
    let front = |w: &Liquids| (0..120).rev().find(|&x| w.column(col(x, 0)).unwrap().last().unwrap().water >= U / 16).unwrap();
    let mut at = Vec::new();
    for k in 1..=120 {
        w.step(&g);
        if k % 30 == 0 {
            at.push((front(&w) - 39) as f64 * 0.5);
        }
    }
    let s = settle(&mut w, &g, 60 * 300) + 2.0;
    json("dam_columns", &[("front_0.5s_m", at[0]), ("front_1s_m", at[1]), ("front_2s_m", at[3]), ("ritter_1s_m", 8.86), ("asleep_s", s), ("units_lost", (t - w.total()) as f64)]);
}

/// The same dam break in a voxel automaton: every 0.5 m cell holds a fill 0..1 (plus a little compression);
/// each step water falls, then shares sideways, then pressure pushes up (the "compressible" CA).
struct Cells {
    nx: usize,
    nz: usize,
    m: Vec<f32>,
    solid: Vec<bool>,
}

impl Cells {
    fn i(&self, x: usize, z: usize) -> usize {
        x + z * self.nx
    }
    fn step(&mut self) -> usize {
        const MAXM: f32 = 1.0;
        const COMP: f32 = 0.02;
        let stable = |total: f32| if total <= 1.0 { 1.0 } else if total < 2.0 * MAXM + COMP { (MAXM * MAXM + total * COMP) / (MAXM + COMP) } else { (total + COMP) / 2.0 };
        let mut n = self.m.clone();
        let mut visited = 0;
        for z in 0..self.nz {
            for x in 0..self.nx {
                let i = self.i(x, z);
                if self.solid[i] || self.m[i] <= 0.0001 {
                    continue;
                }
                visited += 1;
                let mut rem = self.m[i];
                // down
                if z > 0 && !self.solid[self.i(x, z - 1)] {
                    let j = self.i(x, z - 1);
                    let f = (stable(rem + self.m[j]) - self.m[j]).clamp(0.0, rem.min(1.0));
                    n[i] -= f;
                    n[j] += f;
                    rem -= f;
                }
                // sideways
                for dx in [-1i64, 1] {
                    let xx = x as i64 + dx;
                    if rem <= 0.0 || xx < 0 || xx >= self.nx as i64 {
                        continue;
                    }
                    let j = self.i(xx as usize, z);
                    if self.solid[j] {
                        continue;
                    }
                    let f = ((self.m[i] - self.m[j]) / 4.0).clamp(0.0, rem);
                    n[i] -= f;
                    n[j] += f;
                    rem -= f;
                }
                // up
                if rem > 0.0 && z + 1 < self.nz && !self.solid[self.i(x, z + 1)] {
                    let j = self.i(x, z + 1);
                    let f = (rem - stable(rem + self.m[j])).clamp(0.0, rem.min(1.0));
                    n[i] -= f;
                    n[j] += f;
                }
            }
        }
        self.m = n;
        visited
    }
}

fn dam_cells() {
    let (nx, nz) = (120, 8);
    let mut c = Cells { nx, nz, m: vec![0.0; nx * nz], solid: vec![false; nx * nz] };
    for x in 0..nx {
        let i = c.i(x, 0);
        c.solid[i] = true;
    }
    for x in 0..40 {
        for z in 1..5 {
            let i = c.i(x, z);
            c.m[i] = 1.0;
        }
    }
    let total: f32 = c.m.iter().sum();
    let front = |c: &Cells| (0..nx).rev().find(|&x| c.m[c.i(x, 1)] >= 1.0 / 16.0).unwrap();
    let mut at = Vec::new();
    for k in 1..=120 {
        c.step();
        if k % 30 == 0 {
            at.push((front(&c) as f64 - 39.0) * 0.5);
        }
    }
    let after: f32 = c.m.iter().sum();
    json("dam_cells", &[("front_0.5s_m", at[0]), ("front_1s_m", at[1]), ("front_2s_m", at[3]), ("ritter_1s_m", 8.86), ("volume_drift", (after - total) as f64)]);
}

/// A tank 4 m deep, 2.5 x 2.5 m, over a room, with a 0.5 m hole in its floor: Torricelli says
/// 0.25 m^2 x sqrt(2 g h) = 2.2 m^3/s = 17.7 blocks a second at first.
fn tank() {
    let mut g = Box3::new(9, 9, 30);
    g.fill(0..9, 0..9, 0..20, true);
    g.fill(2..7, 2..7, 12..20, false);
    g.fill(0..9, 0..9, 0..11, false);
    g.fill(0..9, 0..9, 0..1, true);
    let mut w = stored(&g);
    for y in 2..7 {
        for x in 2..7 {
            w.pour(&g, col(x, y), 12, 8 * U);
        }
    }
    g.set(4, 4, 11, false);
    w.regap(&g, col(4, 4));
    let tank = |w: &Liquids| -> f64 {
        (2..7).flat_map(|y| (2..7).map(move |x| (x, y))).map(|(x, y)| w.column(col(x, y)).unwrap().filter(|n| n.floor >= 11).map(|n| n.water).sum::<i64>()).sum::<i64>() as f64 / U as f64
    };
    let start = tank(&w);
    for _ in 0..60 {
        w.step(&g);
    }
    let first = start - tank(&w);
    let mut empty = f64::NAN;
    for k in 0..60 * 60 {
        w.step(&g);
        if tank(&w) < 1.0 && empty.is_nan() {
            empty = 1.0 + k as f64 / 60.0;
        }
    }
    // Torricelli for a tank of area A over a hole a: t = (A / a) sqrt(2 H / g)
    json("tank", &[("blocks_first_s", first), ("torricelli_first_s", 17.7), ("empty_s", empty), ("torricelli_empty_s", 25.0 * (2.0 * 4.0 / 9.81f64).sqrt())]);
}

/// Cost: a 128 x 128 basin with 2 m of water and a 10 m wide, 3 m high hump of water dropped in the middle
/// (a big surge); ns per awake node-step, and how long until it is all asleep.
fn surge() {
    let n = 128;
    let mut g = Box3::new(n, n, 16);
    g.fill(0..n, 0..n, 0..1, true);
    let mut w = stored(&g);
    for y in 0..n {
        for x in 0..n {
            let r2 = (x - n / 2).pow(2) + (y - n / 2).pow(2);
            w.pour(&g, col(x, y), 1, 4 * U + if r2 < 100 { 6 * U } else { 0 });
        }
    }
    let t = w.total();
    let (s0, t0) = (w.stepped, Instant::now());
    let mut steps = 0;
    let mut peak = 0;
    while w.awake() > 0 && steps < 60 * 600 {
        w.step(&g);
        peak = peak.max(w.awake());
        steps += 1;
    }
    let dt = t0.elapsed().as_secs_f64();
    let node_steps = (w.stepped - s0) as f64;
    let flat = (0..n).map(|x| level(&w, x, n / 2)).fold((f64::MAX, f64::MIN), |(a, b), l| (a.min(l), b.max(l)));
    json("surge_128", &[
        ("ns_per_node_step", dt * 1e9 / node_steps),
        ("asleep_s", steps as f64 / 60.0),
        ("peak_awake", peak as f64),
        ("ms_per_step_at_peak", dt * 1e3 / steps as f64 * peak as f64 / (node_steps / steps as f64)),
        ("level_spread_blocks", flat.1 - flat.0),
        ("units_lost", (t - w.total()) as f64),
    ]);
}

/// Cost of the automaton for the same surge, in 3D cells (one slice is enough for the per-cell number).
fn surge_cells() {
    let (nx, nz) = (128 * 128 / 16, 16);
    let mut c = Cells { nx, nz, m: vec![0.0; nx * nz], solid: vec![false; nx * nz] };
    for x in 0..nx {
        let i = c.i(x, 0);
        c.solid[i] = true;
        for z in 1..5 {
            let i = c.i(x, z);
            c.m[i] = 1.0;
        }
    }
    for x in nx / 2 - 10..nx / 2 + 10 {
        for z in 5..11 {
            let i = c.i(x, z);
            c.m[i] = 1.0;
        }
    }
    let t0 = Instant::now();
    let mut visited = 0;
    for _ in 0..600 {
        visited += c.step();
    }
    let dt = t0.elapsed().as_secs_f64();
    json("surge_cells", &[("ns_per_wet_cell_step", dt * 1e9 / visited as f64), ("wet_cells_per_column", visited as f64 / 600.0 / nx as f64)]);
}

/// A splash in a generated sea: how far the stored region spreads and how long until it sleeps.
fn splash() {
    let n = 400;
    let mut g = Box3::new(n, n, 24);
    g.fill(0..n, 0..n, 0..4, true);
    g.with_sea(20 * U);
    let mut w = Liquids::default();
    w.pour(&g, col(n / 2, n / 2), 21, 4 * U);
    let t0 = Instant::now();
    let s = settle(&mut w, &g, 60 * 600);
    json("sea_splash", &[("stored_columns", w.stored() as f64), ("asleep_s", s), ("wall_ms", t0.elapsed().as_secs_f64() * 1e3), ("bucket_units_kept", (w.total() - w.generated_total()) as f64 / U as f64)]);
}

fn main() {
    dam_columns();
    dam_cells();
    tank();
    surge();
    surge_cells();
    splash();
}
