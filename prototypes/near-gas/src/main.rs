//! Throwaway spike (docs/design/liquids.md, "Gases"; air.md): gas near players, conserving with the coarse air.
//! `cargo run --release` prints every number of the README; `cargo test --release` the tests.
mod gaps;
mod gas;
mod heat;
mod scenes;
#[cfg(test)]
mod tests;

use gas::*;
use heat::*;
use scenes::*;
use std::time::Instant;

const DAY: f64 = 1800.0;

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

/// A patch of heaters (20 kW a block) on stone ground in a box open to the sky: the plume over it is the active set.
fn plume(patch: i32, box_w: i32) -> Sim {
    let mut g = Ground::new([0, 0, 0], [box_w, box_w, 16], 1);
    g.fill([0, 0, 0], [box_w, box_w, 1], STONE);
    let mut s = Sim::new(g, T_AIR);
    let o = (box_w - patch) / 2;
    for j in o..o + patch {
        for i in o..o + patch {
            s.heaters.push(([i, j, 1], 20_000.0));
        }
    }
    s
}

/// ms a step of the couple (heat store and fire) and of the gas, medians of `n` steps.
fn time_steps(s: &mut Sim, n: usize) -> (f64, f64) {
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for _ in 0..n {
        let (c, g) = s.step_timed();
        a.push(c);
        b.push(g);
    }
    (median(a), median(b))
}

fn cost() {
    println!("## Cost a step (20 Hz), single thread");
    println!("| scene | fine cells | heat-store blocks | couple ms | gas ms | ms a second | MB |");
    println!("|---|---|---|---|---|---|---|");
    let row = |name: &str, s: &mut Sim| {
        let (c, g) = time_steps(s, 100);
        let mb = (s.gas.cells.len() * (std::mem::size_of::<Cell>() + 24) + s.blocks.len() * (std::mem::size_of::<Block>() + 24)) as f64 / 1e6;
        println!("| {name} | {} | {} | {c:.3} | {g:.3} | {:.1} | {mb:.2} |", s.gas.cells.len(), s.blocks.len(), (c + g) * 20.0);
    };
    let mut s = room(Hole::Roof);
    s.run(300.0);
    row("room fire, roof hole, at 300 s", &mut s);
    let mut s = plume(11, 24);
    s.run(60.0);
    row("house-sized plume (11 x 11 m of 20 kW)", &mut s);
    let mut s = plume(56, 70);
    s.run(30.0);
    row("forest-front-sized plume (56 x 56 m of 20 kW)", &mut s);
    let mut s = line(Line::Dry);
    s.run(1800.0);
    row("line of wood at 1800 s", &mut s);
    println!("Cell {} B, block {} B (+ about 24 B of hash map each).", std::mem::size_of::<Cell>(), std::mem::size_of::<Block>());
}

fn stores() {
    use gaps::Gaps;
    println!("## Store (ii) against store (i) on the room's 100 kW smoke maker");
    for hole in [Hole::Roof, Hole::Floor] {
        let mut s = room_maker(hole);
        s.run(300.0);
        let (_, g1) = time_steps(&mut s, 100);
        let left1 = 1.0 - s.excess_in(SOOT, ROOM_LO, ROOM_HI) as f64 / s.gas.made[SOOT] as f64;
        let g = room_ground(hole);
        let mut q = Gaps::new(&g, Coarse::standard(128.0 * 128.0 * 30.0, 15.0, T_AIR));
        let t0 = Instant::now();
        for _ in 0..100 {
            q.step();
        }
        let g2 = t0.elapsed().as_secs_f64() * 10.0;
        println!("{hole:?}: (i) {} cells, {g1:.3} ms a step, soot out {left1:.3}; (ii) {} nodes (every gap of the box), {g2:.3} ms a step", s.gas.cells.len(), q.nodes.len());
    }
    for second in [false, true] {
        let mut s = valley();
        s.gas.second = second;
        s.run(600.0);
        let rel = s.gas.made[CO2] as f64;
        let pit = s.excess_in(CO2, PIT_LO, PIT_HI) as f64 / rel;
        let low = s.excess_in(CO2, [11, 0, 0], [32, 3, 4]) as f64 / rel;
        println!("valley, {} order: pit {pit:.3} of the release, the floor's lowest metre and the pit {low:.3}, {} cells made, {} dissolved", if second { "second" } else { "first" }, s.gas.made_cells, s.gas.gone_cells);
    }
    // store (ii) on the valley: the pit's gap reaches the sky
    let g = valley_ground();
    let mut q = Gaps::new(&g, Coarse::standard(128.0 * 128.0 * 30.0, 15.0, T_AIR));
    let at = q.node_at(VENT).unwrap();
    let base: Vec<i64> = q.nodes.iter().map(|n| n.m[CO2]).collect();
    for n in 0..(600.0 / DT) as u64 {
        if n < VENT_STEPS {
            q.nodes[at].m[CO2] += VENT_UNITS;
        }
        q.step();
    }
    let pit: f64 = q.nodes.iter().zip(&base).filter(|(n, _)| (PIT_LO[0]..PIT_HI[0]).contains(&n.i)).map(|(n, b)| (n.m[CO2] - b) as f64 * 3.0 / n.vol()).sum();
    println!("valley, store (ii): pit {:.3} of the release (its gaps run from the pit floor to the sky, mixed)", pit / (VENT_UNITS * VENT_STEPS as i64) as f64);
}

fn fire() {
    println!("## Fire, against the game clock (a game day is {DAY} s)");
    let mut s = room(Hole::None);
    let mut died = None;
    let mut peak: f64 = 0.0;
    for n in 0..(1800.0 / DT) as u64 {
        s.step();
        let i = s.gas.cell_at([6, 6, 6]).unwrap() as usize;
        peak = peak.max(s.gas.pressure(i));
        if died.is_none() && !s.block_at(ROOM_FIRE).unwrap().burning {
            died = Some(n as f64 * DT);
        }
    }
    println!("sealed room 6 x 6 x 3: the fire died at {:.0} s ({:.2} game days) after {:.2} kg; the room's pressure peaked at {:.0} Pa", died.unwrap_or(-1.0), died.unwrap_or(0.0) / DAY, s.burnt as f64 / MU_PER_KG as f64, peak);
    for hole in [Hole::Roof, Hole::Floor] {
        let mut s = room(hole);
        s.run(1800.0);
        let b = s.block_at(ROOM_FIRE).unwrap();
        let left = 1.0 - s.excess_in(SOOT, ROOM_LO, ROOM_HI) as f64 / s.gas.made[SOOT] as f64;
        println!("room, {hole:?} hole, 1800 s: burnt {:.2} kg, burning {}, soot out {left:.3}", s.burnt as f64 / MU_PER_KG as f64, b.burning);
    }
    let w = &MATS[WOOD as usize];
    let burn_out = w.fuel / (w.burn * 4.0);
    println!("a block of wood with 4 open faces burns out in {:.0} s ({:.1} game days) at {:.0} kW", burn_out, burn_out / DAY, w.burn * 4.0 * 15e3);
    for kind in [Line::Dry, Line::Wet, Line::Wind] {
        let mut s = line(kind);
        s.run(4.0 * 3600.0);
        let v: Vec<String> = LINE.map(|i| s.ignited([i, 2, 1]).map_or("-".into(), |t| format!("{t:.0}"))).collect();
        println!("line {kind:?}, 4 h: caught at {}", v.join(" "));
    }
    let mut s = pot();
    let mut boil = None;
    while s.block_at(POT_AT).unwrap().water > 0 {
        s.step();
        if boil.is_none() && s.boiled > 0 {
            boil = Some(s.t);
        }
    }
    println!("pot of 10 kg on 3 kW: first unit boiled at {:.0} s ({:.2} game days), dry at {:.0} s ({:.2} game days)", boil.unwrap(), boil.unwrap() / DAY, s.t, s.t / DAY);
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("wet") {
        let mut s = line(Line::Wet);
        for m in 0..12 {
            s.run(600.0);
            let b = s.block_at([4, 2, 1]).unwrap();
            println!("{} min: next block skin {:.1} K, core {:.1} K, skin water {} of {}, boiled {}", (m + 1) * 10, b.t_skin(), b.t_core(), b.water, b.water0, s.boiled);
        }
        return;
    }
    cost();
    stores();
    fire();
}
