//! Throwaway spike (docs/design/liquids.md, "The rule"): ground water held by a formula, counted when touched.
//! `cargo run --release` prints the README's numbers; `cargo test --release -- --nocapture` the tests.
mod gw;

use gw::*;
use std::time::Instant;

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

fn main() {
    // 1. Cost of a step of disturbed columns (a heads' bump so every link carries flow; ledger check off)
    println!("Cost of a step (single thread, median of 31):");
    for (nx, ny) in [(100usize, 100usize), (317, 316)] {
        let mut a = Aquifer::flat(nx, ny, 1.0, 10.0, 30.0, 0.0, 1e-5, 0.15);
        a.check = false;
        a.recharge = vec![per_step(0.6 / REAL_YEAR, 10.0, 1.0); nx * ny];
        let h: Vec<f64> = (0..nx * ny).map(|i| 15.0 + ((i % nx) as f64 * 0.3).sin() + ((i / nx) as f64 * 0.2).cos()).collect();
        a.set_heads(&h);
        let mut t = vec![];
        for _ in 0..31 {
            let s = Instant::now();
            a.step();
            t.push(s.elapsed().as_secs_f64() * 1e3);
        }
        let m = median(t);
        println!("  {:>7} columns: {m:.3} ms a step, {:.1} ns a column", nx * ny, m * 1e6 / (nx * ny) as f64);
    }

    // 2. The bake's 8 m field read by 1 m columns: how far the columns' own rest stands from it
    for stripes in [true, false] {
    println!();
    let one_rock = |mut a: Aquifer| {
        if !stripes {
            a.k.iter_mut().for_each(|k| *k = 1e-5);
            a.sy.iter_mut().for_each(|s| *s = 0.1);
        }
        a
    };
    let coarse = one_rock(baked(40, 8.0, 600.0));
    let (fc, sc) = solve(&coarse, &coarse.ground, 1.8, 1e-6, 200_000);
    let fine = one_rock(baked(320, 1.0, 60.0));
    let s = Instant::now();
    let (ff, sf) = solve(&fine, &fine.ground, 1.97, 1e-6, 400_000);
    println!(
        "The 8 m field ({sc} sweeps, residual {:.1e}) against the 1 m columns' own rest ({sf} sweeps, {:.1} s, residual {:.1e}), {}:",
        residual(&coarse, &fc),
        s.elapsed().as_secs_f64(),
        residual(&fine, &ff),
        if stripes { "two rocks in 40 m stripes" } else { "one rock" }
    );
    let at = |x: f64, y: f64| {
        let (i, j) = ((x / 8.0).floor() as usize, (y / 8.0).floor() as usize);
        let (i, j) = (i.min(38), j.min(38));
        let (u, v) = (x / 8.0 - i as f64, y / 8.0 - j as f64);
        let g = |a: usize, b: usize| fc[b * 40 + a];
        (1.0 - u) * (1.0 - v) * g(i, j) + u * (1.0 - v) * g(i + 1, j) + (1.0 - u) * v * g(i, j + 1) + u * v * g(i + 1, j + 1)
    };
    let mut d = vec![];
    for j in 0..=312usize {
        for i in 0..=312usize {
            let k = j * 320 + i;
            if fine.fixed[k].is_some() {
                continue;
            }
            // the engine would clamp the read table to the column's own ground
            let read = at(i as f64, j as f64).min(fine.ground[k]);
            d.push((read - ff[k]).abs());
        }
    }
    let n = d.len();
    let mean = d.iter().sum::<f64>() / n as f64;
    let mut ds = d.clone();
    ds.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let q = |p: f64| ds[((n - 1) as f64 * p) as usize];
    let within = |m: f64| d.iter().filter(|&&e| e * U as f64 <= m).count() as f64 / n as f64 * 100.0;
    println!("  mean {:.1} mm, median {:.1} mm, p99 {:.0} mm, max {:.0} mm", mean * 1e3, q(0.5) * 1e3, q(0.99) * 1e3, q(1.0) * 1e3);
    println!("  columns within 1 unit (0.24 mm) {:.1} %, within 64 units (1/64 block) {:.1} %, within 0.1 m {:.1} %", within(1.0), within(64.0), within(409.6));
    let seep_c = (0..fc.len()).filter(|&i| coarse.fixed[i].is_none() && fc[i] >= coarse.ground[i] - 1e-9).count();
    let seep_f = (0..ff.len()).filter(|&i| fine.fixed[i].is_none() && ff[i] >= fine.ground[i] - 1e-9).count();
    println!("  seeping: {seep_c} of 1600 cells (8 m), {seep_f} of 102,400 columns (1 m)");
    }

    // 3. The bake's cost: one face of the 4 km planet (768 x 768 cells of 8 m), tiled from the same stretch
    println!();
    let face = baked(768, 8.0, 600.0);
    let s = Instant::now();
    let (hf, sw) = solve(&face, &face.ground, 1.95, 1e-6, 200_000);
    let t = s.elapsed().as_secs_f64();
    println!("  (its residual {:.1e})", residual(&face, &hf));
    println!("A face, 768 x 768 cells, Gauss-Seidel at omega 1.95 to a residual of 1e-6 of the rain: {sw} sweeps, {t:.1} s ({:.1} ms a sweep); six faces {:.0} s", t * 1e3 / sw as f64, 6.0 * t);

    // 4. Rates against the calendar: tau = S_y L^2 / (K b), b = 20 m saturated
    println!();
    let scale = REAL_YEAR / GAME_YEAR;
    println!("Relaxation time S_y L^2 / (K b), b = 20 m; the calendar's factor {scale:.2}:");
    println!("  {:<28} {:>8} {:>5} {:>6} {:>12} {:>12} {:>14}", "rock", "K m/s", "S_y", "L m", "real days", "game years", "x52: game days");
    for (name, k, sy) in [
        ("gravel", 1e-3, 0.25),
        ("clean sand", 1e-4, 0.25),
        ("silty sand, sandstone", 1e-5, 0.15),
        ("fractured limestone", 1e-5, 0.05),
        ("basalt (jointed)", 1e-6, 0.05),
        ("fractured granite", 1e-8, 0.01),
        ("shale (an aquitard)", 1e-11, 0.02),
    ] {
        for l in [10.0, 100.0, 1000.0] {
            let tau = sy * l * l / (k * 20.0);
            println!("  {name:<28} {k:>8.0e} {sy:>5} {l:>6} {:>12.2} {:>12.3} {:>14.2}", tau / 86400.0, tau / GAME_YEAR, tau / scale / GAME_DAY);
        }
    }
    let r = 0.6 / REAL_YEAR;
    println!("600 mm a year of recharge: {:.2e} m/s, {:.1} units a column a game day at real rates, {:.0} at x52", r, r * GAME_DAY * U as f64, r * GAME_DAY * U as f64 * scale);
}
