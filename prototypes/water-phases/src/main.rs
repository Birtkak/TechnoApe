//! Throwaway spike (docs/design/liquids.md, "Phases", step 5; ADR 0183): water's three phases as one ledger.
//! `cargo run --release` prints every number of the README; `cargo test --release` the tests.
mod phase;

use phase::*;
use std::collections::HashMap;
use std::time::Instant;

fn game(s: f64) -> String {
    format!("{s:>11.0} s = {:>8.2} game days = {:>6.3} game years", s / DAY, s / DAY / YEAR_DAYS)
}

fn main() {
    let c = k();
    println!("Per unit ({:.4} kg): fusion {} mJ, vaporisation {} mJ (L_v fitted {:.0} J/kg), sublimation {} mJ; water {:.0} mJ/K, ice {:.0} mJ/K", MU, c.lfu, c.lvu, c.l_v, c.lsu, c.cwu, c.ciu);
    println!();
    println!("Where water leaves for vapour (boiling over the triple point, sublimation under it):");
    // the air's scale height on the 4 km Terra: R T / (M g) at 15 C under 19.62 m/s2
    let hs: f64 = 8.314 * 288.0 / (0.029 * 19.62);
    for (what, p) in [
        ("1 atm", ATM),
        ("a 1 km peak on Terra (scale height 4.2 km)", ATM * (-1000.0 / hs).exp()),
        ("the 3 km air top on Terra", ATM * (-3000.0 / hs).exp()),
        ("Everest (0.33 atm)", 0.33 * ATM),
        ("Mars-like 700 Pa", 700.0),
        ("Mars 0.006 atm (608 Pa)", 0.006 * ATM),
        ("100 Pa", 100.0),
        ("1 Pa", 1.0),
        ("no air", 0.0),
    ] {
        println!("  {what:<44} {p:>9.1} Pa: {:>8.2} C", t_transition(p));
    }
    println!();

    println!("1. An ice block (1 m3, from 0 C) in +10 C air, h = 5 convective + 5 radiative W/m2/K:");
    let m = melt_block(0.0, 10.0, 5.0, 5.0, 1.0);
    println!("   water {} units, vapour {} (ledger U = {U}); {}", m.water, m.vapour, game(m.seconds));
    println!("   the shrinking cube's law 3 rho L / (5 h dT): {}", game(melt_analytic(10.0, 10.0)));
    let w = melt_block(0.0, 10.0, 20.0, 5.0, 1.0);
    println!("   in a breeze (h_c 20): {}", game(w.seconds));
    let cold = melt_block(-10.0, 10.0, 5.0, 5.0, 1.0);
    println!("   from -10 C: water {} + vapour {}, {}", cold.water, cold.vapour, game(cold.seconds));
    println!();

    let ha = 14.5;
    println!("2. A pond 1 m deep at 0 C under -10 C air, h = 10 + 4.5 W/m2/K, no heat from the ground:");
    let f = freeze_pond(U, 0.0, -10.0, ha, 10.0, false);
    println!("   {:>6} {:>12} {:>12} {:>10} {:>10}", "ice m", "sim s", "Stefan s", "heat W/m2", "game days");
    for &(d, t, q) in &f.marks {
        println!("   {d:>6.2} {t:>12.0} {:>12.0} {q:>10.1} {:>10.1}", stefan(d, -10.0, ha), t / DAY);
    }
    println!("   solid: {}", game(f.solid));
    let night = DAY / 2.0;
    let skim = |dt: f64, ta: f64| {
        let a = 1.0 / (2.0 * K_ICE);
        let b = 1.0 / ha;
        let c = -dt * -ta / (RHO * L_F);
        (-b + (b * b - 4.0 * a * c).sqrt()) / (2.0 * a)
    };
    println!("   ice a game night (900 s) at -10 C: {:.1} mm; a real night (12 h): {:.0} mm", skim(night, -10.0) * 1000.0, skim(43_200.0, -10.0) * 1000.0);
    let winter = YEAR_DAYS / 4.0 * DAY;
    println!("   a game winter ({:.0} game days, {:.2} real days) at -10 C: {:.2} m; a real winter (90 days): {:.2} m", YEAR_DAYS / 4.0, winter / 86_400.0, skim(winter, -10.0), skim(90.0 * 86_400.0, -10.0));
    let fv = freeze_pond(U, 0.0, -10.0, ha, 10.0, true);
    println!("   with the vapour exchange on (air saturated over ice at -10 C): solid in {} ({:+.1} % from sublimation's cooling)", game(fv.solid), (fv.solid / f.solid - 1.0) * 100.0);
    let warm = freeze_pond(U, 4.0, -10.0, ha, 10.0, false);
    println!("   from +4 C: {}", game(warm.solid));
    println!();

    println!("3. A pot of 41 units (10 kg) at 15 C over a 3 kW fire, 0.3 m2 losing heat, 0.05 m2 open:");
    let b = boil_pot(41, 15.0, 15.0, ATM, 3000.0, 0.05);
    println!("   at 1 atm: boils after {}; dry after {}", game(b.to_boil), game(b.dry));
    println!("   plateau {:.3} C, highest {:.3} C, the air cell's vapour +{} units", b.plateau, b.max_t, b.vapour);
    let mb = boil_pot(41, 1.0, -60.0, 700.0, 3000.0, 0.05);
    println!("   at 700 Pa, -60 C air: plateau {:.3} C (the curve's {:.3}), dry after {}, vapour +{}", mb.plateau, t_transition(700.0), game(mb.dry), mb.vapour);
    println!();

    println!("4. Ice without (enough) air, 20 game days:");
    let s = sublimate(-20.0, -60.0, 0.006 * ATM, 2000.0, 20.0 * DAY, 0.05);
    println!("   608 Pa, -60 C, a 2 kW fire beside it: most water ever {}, ice {}, vapour {}, block {:.2} C", s.max_water, s.ice, s.vapour, s.temp);
    let v = sublimate(-10.0, -100.0, 0.0, 500.0, 20.0 * DAY, 0.05);
    println!("   no air, 500 W of sun: most water ever {}, ice {}, vapour {}, block {:.2} C", v.max_water, v.ice, v.vapour, v.temp);
    let v0 = sublimate(-10.0, -100.0, 0.0, 0.0, 20.0 * DAY, 0.05);
    println!("   no air, in shadow: ice {}, vapour {}, block {:.2} C", v0.ice, v0.vapour, v0.temp);
    println!();

    let fw = f_for_sheet(-8.0, ha);
    println!("5. Sea ice: the formula against a stored stretch (20 m deep, no vapour, 25 real years at 1 h steps)");
    println!("   the sea's heat from below that makes the bake's one-block sheet at -8 C the rest state: {fw:.2} W/m2");
    let sea = sea_stretch(12, 20 * U, -0.5, -30.0, fw, ha, 25.0 * 3.2e7, 3600.0);
    println!("   {:>7} {:>9} {:>9} {:>13} {:>13} {:>10}", "air C", "formula", "blocks", "laid, after", "open, after", "bake");
    for &(t, rest, kept, open) in &sea.rows {
        println!("   {t:>7.2} {rest:>9} {:>9.2} {kept:>13} {open:>13} {:>10}", rest as f64 / U as f64, if t < -8.0 { "1 block" } else { "open" });
    }
    // the time a stored patch takes to grow to 90 % of its rest from open water at -8 C
    let mut air = Air::new(-8.0, ATM, ha - 4.5, 4.5);
    air.vapour = false;
    let (mut n, mut src, mut t) = (Node::water(20 * U, 0.0), 0, 0.0);
    let rest = rest_ice(-8.0, fw, ha, 20 * U);
    while n.i * 10 < rest * 9 {
        step_column(&mut n, &mut air, &mut src, fw, 60.0);
        t += 60.0;
    }
    println!("   from open water at -8 C to 90 % of the block: {}", game(t));
    println!();

    bench();
}

/// Question 2: where heat is held. 100,000 stored sea columns at rest; 3,000 active cells (2,000 pond columns off
/// their rest, 500 ice blocks, 500 pots); the vapour exchange on (the dearer path, an exp a surface).
fn bench() {
    const STORED: usize = 100_000;
    let ha = 14.5;
    let fw = f_for_sheet(-8.0, ha);
    let mk_airs = || (0..9).map(|a| Air::new(-12.0 + a as f64 * 0.5, ATM, ha - 4.5, 4.5).at_pv(0.8 * p_sat(-12.0, true))).collect::<Vec<_>>();
    let depth = 4 * U;
    let air_of = |c: usize| (c / 316 / 106 * 3 + c % 316 / 106) as u16;

    // (b) a temperature per stored column: every column a node, stepped every step
    let mut airs = mk_airs();
    let mut nodes: Vec<Node> = (0..STORED).map(|c| {
        let r = rest_ice(airs[air_of(c) as usize].t, fw, ha, depth);
        Node { i: r, w: depth - r, ..Node::default() }
    }).collect();
    let air_idx: Vec<u16> = (0..STORED).map(air_of).collect();
    let mut blocks: Vec<Block> = (0..500).map(|_| Block { n: Node::ice(U, -5.0), pool: 0 }).collect();
    let mut pots: Vec<Node> = (0..500).map(|_| Node::water(41, 20.0)).collect();
    let mut src = 0i64;
    let dt = 1.0 / TICK_HZ;
    let mut times = Vec::new();
    for _ in 0..200 {
        let t0 = Instant::now();
        for (n, &a) in nodes.iter_mut().zip(&air_idx) {
            step_column(n, &mut airs[a as usize], &mut src, fw, dt);
        }
        for b in &mut blocks {
            b.step(&mut airs[4], &mut src, 0.0, dt);
        }
        for p in &mut pots {
            step_pot(p, &mut airs[4], &mut src, 3000.0, 0.3, 0.05, dt);
        }
        times.push(t0.elapsed().as_secs_f64() * 1e3);
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let dense = times[times.len() / 2];
    let dense_mem = STORED * (std::mem::size_of::<Node>() + 2) + 500 * std::mem::size_of::<Block>() + 500 * std::mem::size_of::<Node>();

    // (a) a temperature only near a source or a phase boundary: a sparse set; the rest read the air, and once
    // a second every stored column's ice is checked against the formula for its air
    #[derive(Clone)]
    struct Active {
        kind: u8,
        air: u16,
        b: Block,
    }
    let mut airs = mk_airs();
    let ice: Vec<i64> = (0..STORED).map(|c| rest_ice(airs[air_of(c) as usize].t, fw, ha, depth)).collect();
    let mut act: Vec<Active> = Vec::new();
    let mut index: HashMap<u64, u32> = HashMap::new();
    for c in 0..2000 {
        // a pond someone opened: open water at 0 C where the formula has ice
        act.push(Active { kind: 0, air: air_of(c * 50), b: Block { n: Node::water(depth, 0.0), pool: 0 } });
    }
    for _ in 0..500 {
        act.push(Active { kind: 1, air: 4, b: Block { n: Node::ice(U, -5.0), pool: 0 } });
    }
    for _ in 0..500 {
        act.push(Active { kind: 2, air: 4, b: Block { n: Node::water(41, 20.0), pool: 0 } });
    }
    for k in 0..act.len() {
        index.insert(k as u64 * 7919, k as u32);
    }
    let mut times = Vec::new();
    for _ in 0..200 {
        let t0 = Instant::now();
        for a in &mut act {
            let air = &mut airs[a.air as usize];
            match a.kind {
                0 => {
                    step_column(&mut a.b.n, air, &mut src, fw, dt);
                }
                1 => a.b.step(air, &mut src, 0.0, dt),
                _ => step_pot(&mut a.b.n, air, &mut src, 3000.0, 0.3, 0.05, dt),
            }
        }
        times.push(t0.elapsed().as_secs_f64() * 1e3);
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let sparse = times[times.len() / 2];
    // the once-a-second check of the resting columns (the air grid's cells change every 10 s)
    let mut scans = Vec::new();
    let mut woke = 0;
    for _ in 0..50 {
        let t0 = Instant::now();
        let rests: Vec<i64> = airs.iter().map(|a| rest_ice(a.t, fw, ha, depth)).collect();
        woke = ice.iter().zip(&air_idx).filter(|(&i, &a)| (i - rests[a as usize]).abs() > 1).count();
        scans.push(t0.elapsed().as_secs_f64() * 1e3);
    }
    scans.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let scan = scans[scans.len() / 2];
    let sparse_mem = act.capacity() * std::mem::size_of::<Active>() + index.capacity() * (8 + 4 + 1);

    println!("Heat storage, 100,000 stored columns, 3,000 active cells (vapour on), a step of {dt} s:");
    println!("   {:<52} {:>10} {:>12} {:>14} {:>13}", "", "memory", "ms a step", "ms/s at 20 Hz", "ms/s at 1 Hz");
    println!("   {:<52} {:>8.2}MB {:>12.3} {:>14.2} {:>13.3}", "(b) a temperature per stored column", dense_mem as f64 / 1e6, dense, dense * 20.0, dense);
    println!("   {:<52} {:>8.2}MB {:>12.3} {:>14.2} {:>13.3}", "(a) only near sources and phase boundaries", sparse_mem as f64 / 1e6, sparse, sparse * 20.0, sparse);
    println!("   {:<52} {:>10} {:>12.3} {:>14} {:>13.3}", "(a)'s once-a-second check of the 100,000 at rest", "", scan, "", scan);
    println!("   (columns the check would wake: {woke}; each stored column already holds its ice units in either way: {:.2} MB as an i64, 0.2 MB as a u16)", STORED as f64 * 8.0 / 1e6);
    println!("   size of a node {} B, of an active cell {} B", std::mem::size_of::<Node>(), std::mem::size_of::<Active>());
}
