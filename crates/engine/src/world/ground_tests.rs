//! Ground water in the world (ADR 0190): flooded caves, a dug well filling from its cell, springs' streams, and old
//! worlds left dry.
use super::*;
use crate::liquid::{Ground, K, SUB};
use std::sync::LazyLock;

/// The 1536 planet of seed 5 baked with its table (map version 10), shared by these tests.
static MAP: LazyLock<Arc<Map>> = LazyLock::new(|| crate::bake::shared(5, 1536));

fn world() -> World {
    World::with_grid(1536, 32, 5, Some(MAP.clone()), None, 0.0, GROUND_WATER)
}

/// Every column of face `f` stepping by `step`, with its generated caves as air runs (merged), its ground's first air
/// layer and its table (layers).
fn columns(w: &World, f: u8, step: usize) -> impl Iterator<Item = (Col, Vec<(i32, i32)>, i32, Option<f64>)> + '_ {
    let b = &w.bodies[0];
    (0..b.n).step_by(step).flat_map(move |i| (0..b.n).step_by(step).map(move |j| (i, j))).map(move |(i, j)| {
        let mut runs: Vec<(i32, i32)> = b.col_caves(f, i, j).to_vec();
        runs.sort_unstable();
        let mut m: Vec<(i32, i32)> = Vec::new();
        for (lo, hi) in runs {
            match m.last_mut() {
                Some(l) if lo <= l.1 => l.1 = l.1.max(hi),
                _ => m.push((lo, hi)),
            }
        }
        (Col::new(0, f, i, j), m, b.first_air(b.col_height(f, i, j)), b.table_at(f, i, j))
    })
}

/// A closed cave whose floor lies under the table is generation's water up to the table (full under it, with the
/// pressure share that puts its head at the table), one whose floor lies over it is dry; stored, the flooded one holds
/// what generation gave it, the ledger unmoved, and a player in it is in water to the table.
#[test]
fn a_cave_under_the_table_is_flooded_to_it_and_one_over_it_is_dry() {
    let mut w = world();
    let (mut wet, mut dry) = (None, None);
    for f in 0..6u8 {
        for (c, runs, zs, t) in columns(&w, f, 3) {
            let Some(t) = t else { continue };
            if w.bodies[0].open_water(f, c.i(), c.j()) {
                continue;
            }
            // a closed cave alone in its column, open air between it and the ground
            let [(lo, hi)] = runs[..] else { continue };
            if hi >= zs - 1 {
                continue;
            }
            if (lo as f64) < t - 0.5 && (hi as f64) > t + 0.5 && wet.is_none() {
                wet = Some((c, lo, hi, t));
            }
            if (lo as f64) > t + 1.0 && dry.is_none() {
                dry = Some((c, lo, hi, t));
            }
        }
    }
    let (Some((c, lo, hi, t)), Some((d, dlo, _, dt))) = (wet, dry) else { panic!("a cave through the table {wet:?} and one over it {dry:?}") };
    let g = w.ground();
    let tu = (t * U as f64) as i64;
    println!("a cave {lo}..{hi} with the table at {t:.3}; a dry one from {dlo} over the table at {dt:.3}");
    assert_eq!(g.generated_other(c), vec![(lo as i64 * U, tu, 0, crate::fluids::WATER)], "flooded to the table");
    assert!(g.generated_other(d).is_empty(), "dry over the table");
    assert_eq!(w.water.surface_at(&g, c, lo as f64 + 0.5), Some(tu as f64 / U as f64), "a body in it is in water to the table");
    assert_eq!(w.water.surface_at(&g, d, dlo as f64 + 0.5), None);
    // stored: what generation gave, nothing made or lost
    let before = w.water.counted();
    {
        let World { bodies, edits, water, sky, time, .. } = &mut w;
        water.materialize(&Solid::at(bodies, edits, sky, *time), c);
    }
    let held: Vec<(i32, i32, i64)> = w.water.all().into_iter().find(|x| x.0 == c).unwrap().1;
    assert!(held.contains(&(lo, hi, tu - lo as i64 * U)), "{held:?}");
    assert_eq!((before, w.water.counted()), (0, 0));
}

/// A cave full under the table holds the pressure share that gives its head the table's (`liquid::K` units of head a
/// unit over its roof), so stored it lies still.
#[test]
fn a_cave_full_under_the_table_presses_to_it() {
    let w = world();
    let g = w.ground();
    let found = (0..6u8).flat_map(|f| columns(&w, f, 3).collect::<Vec<_>>()).find(|(c, runs, zs, t)| {
        let [(_, hi)] = runs[..] else { return false };
        t.is_some_and(|t| (hi as f64) < t - 2.0) && hi < zs - 1 && !w.bodies[0].open_water(c.face(), c.i(), c.j())
    });
    let (c, runs, _, t) = found.expect("a cave under the table");
    let (lo, hi) = runs[0];
    let tu = (t.unwrap() * U as f64) as i64;
    assert_eq!(g.generated_other(c), vec![(lo as i64 * U, hi as i64 * U, (tu - hi as i64 * U) / K, crate::fluids::WATER)]);
}

/// The column a well can be dug in: land with no cave and no open water, the table 3 to 12 m under the ground, the
/// aquifer's floor well under it.
fn well_site(w: &World) -> (Col, i32, f64) {
    let g = w.ground();
    for f in 0..6u8 {
        for (c, runs, zs, t) in columns(w, f, 7) {
            let Some(t) = t else { continue };
            let Some(a) = g.aquifer(c) else { continue };
            if runs.is_empty() && (3.0..12.0).contains(&(zs as f64 - t)) && a.base < t - 10.0 && a.k >= 1e-6 {
                return (c, zs, t);
            }
        }
    }
    panic!("no well site");
}

/// Seconds of the server's water: `seep` and the heat (once a second, `cool_fluids`) and 60 steps.
fn second(w: &mut World) {
    w.cool_fluids(1.0, |_| false);
    for _ in 0..60 {
        w.step_water();
    }
}

/// The water in the well (the gaps of column c from layer `floor` up), units.
fn in_well(w: &World, c: Col) -> i64 {
    w.water.column(c).map_or(0, |ns| ns.map(|n| n.water).sum())
}

/// A shaft dug 3 m under the table fills from its cell at the well's rate (Peaceman's index, the rule's, at the
/// calendar's pace) to the table and no higher; the stored water and the aquifer's own ledger balance every second,
/// and after two and a half blocks are bailed out it fills again, the bucket's water counted out.
#[test]
fn a_hole_dug_under_the_table_fills_to_it_at_the_rule_and_the_ledger_holds() {
    let mut w = world();
    let (c, zs, t) = well_site(&w);
    let bottom = (t - 3.0).floor() as i32;
    for z in bottom..zs {
        let b = Block { body: 0, face: c.face(), pos: IVec3::new(c.i(), c.j(), z) };
        w.set_block(b, AIR);
        w.regap(b);
    }
    assert!(w.water.column(c).is_some(), "the dug column is stored, dry, to fill");
    assert_eq!(in_well(&w, c), 0);
    // the first second by hand: Q = 2 pi K b / ln(0.198 dx / 0.198) (h - floor), at the pace, whole units
    let a = {
        let g = w.ground();
        let mut g = g;
        g.air = Some(Air { sky: &w.sky, t: w.time, temp: None });
        (g.aquifer(c).unwrap(), g.calendar())
    };
    let (aq, pace) = a;
    // the calendar's pace: the real year over the game's
    assert!((pace - 365.25 * 86_400.0 / w.sky.year).abs() < 1e-9 && pace > 1.0, "ground water's pace {pace}, the year {} s", w.sky.year);
    let wi = 2.0 * std::f64::consts::PI * aq.k * (aq.rest - aq.base) / (aq.dx).ln();
    let first = (wi * (aq.table - bottom as f64) * pace * (U * SUB) as f64) as i64 / SUB;
    second(&mut w);
    let got = in_well(&w, c);
    println!("well at ({}, {}): table {t:.3}, floor {bottom}, K {:e}, b {:.1} m, pace {pace:.2}: {got} units in the first second, {first} by hand", c.i(), c.j(), aq.k, aq.rest - aq.base);
    assert!((got - first).abs() <= 1 && got > 0, "{got} against {first}");
    let top = |w: &World| bottom as f64 + in_well(w, c) as f64 / U as f64;
    let mut secs = 1;
    while (t - top(&w)) * 64.0 > 1.0 {
        second(&mut w);
        secs += 1;
        assert!(top(&w) <= t + 1.0 / U as f64, "over the table: {} against {t}", top(&w));
        assert_eq!((w.water.counted(), w.water.ground_ledger()), (0, 0), "the ledger at {secs} s");
        assert!(secs < 20_000, "never filled: {} against {t}", top(&w));
    }
    for _ in 0..200 {
        second(&mut w);
        assert!(top(&w) <= t + 1.0 / U as f64, "over the table: {} against {t}", top(&w));
    }
    println!("filled to {:.4} (table {t:.4}) in {secs} s; aquifer cells touched {}", top(&w), w.water.aquifer.len());
    assert!((top(&w) - t).abs() * 64.0 <= 1.0);
    assert_eq!((w.water.counted(), w.water.ground_ledger()), (0, 0));
    // bailed: two and a half blocks out by the bucket, counted out; it fills again
    let bail = 2 * U + U / 2;
    let mut took = 0;
    while took < bail {
        let z = (top(&w) - 1e-6).floor() as i32;
        let World { bodies, edits, water, sky, time, .. } = &mut w;
        took += water.scoop(&Solid::at(bodies, edits, sky, *time), c, z, (bail - took).min(U));
    }
    assert_eq!((took, w.water.counted()), (bail, -bail), "the bucket's");
    for _ in 0..secs + 200 {
        second(&mut w);
        assert_eq!((w.water.counted(), w.water.ground_ledger()), (-bail, 0));
    }
    assert!((top(&w) - t).abs() * 64.0 <= 1.0, "filled again: {} against {t}", top(&w));
    // and left at rest the touched cells go back to the table's rest, to be dropped
    for _ in 0..20_000 {
        if w.water.aquifer.is_empty() {
            break;
        }
        second(&mut w);
    }
    assert!(w.water.aquifer.is_empty(), "cells still off rest: {:?}", w.water.aquifer.keys().collect::<Vec<_>>());
    assert_eq!((w.water.counted(), w.water.ground_ledger()), (-bail, 0));
}

/// A spring is a cell where the table meets the ground; where its flow is a litre a second or more a stream starts
/// there (a reach the map without ground water does not have), running down to a river, a lake or the sea with water
/// in it.
#[test]
fn a_spring_where_the_table_meets_the_ground_starts_a_stream() {
    let (new, old) = (&*MAP, crate::bake::Map::bake_v9(5, 1536));
    let seeping = (0..new.h.len()).filter(|&c| new.table[c] != crate::bake::ground::DRY && new.table[c] >= new.h[c] - 1e-3 && new.h[c] > 0.0).count();
    let was: std::collections::HashSet<u32> = old.rivers.iter().map(|r| r.cell).collect();
    let streams: Vec<&crate::bake::rivers::Reach> = new.rivers.iter().filter(|r| !was.contains(&r.cell)).collect();
    // a stream's head: no new reach drains into it
    let heads: Vec<&&crate::bake::rivers::Reach> = streams.iter().filter(|r| !streams.iter().any(|o| o.to == r.cell)).collect();
    println!("{seeping} seeping cells, {} streams of {} cells", heads.len(), streams.len());
    assert!(!heads.is_empty(), "a spring starts a stream");
    for h in &heads {
        let c = h.cell as usize;
        assert!(new.table[c] >= new.h[c] - 1e-3, "a stream starts where the table meets the ground: cell {c} table {} ground {}", new.table[c], new.h[c]);
        assert!(h.flow > 0.0);
    }
    // the reaches before are kept, each as it was
    assert!(old.rivers.iter().all(|r| new.rivers.iter().any(|n| n.cell == r.cell && n.to == r.to)));
    // and a stream holds water in the world
    let w = world();
    let b = &w.bodies[0];
    let m = new.m as usize;
    let h = heads[0].cell as usize;
    let (f, ci, cj) = (h / (m * m), h % m, (h / m) % m);
    let s = b.n as usize / m;
    let wet = (0..s * s).any(|k| b.water_at(f as u8, (ci * s + k % s) as i32, (cj * s + k / s) as i32).is_some_and(|x| x.1 != crate::liquid::SEA));
    assert!(wet, "the stream's first cell holds water");
}

/// A world of generation 11 plays on the map of version 9 and is dry as it was: no table, no cave water, no aquifer.
#[test]
fn a_generation_11_world_has_no_ground_water() {
    let old = Arc::new(crate::bake::Map::bake_v9(5, 1536));
    assert!(old.table.is_empty() && old.version == 9);
    let w = World::with_grid(1536, 32, 5, Some(old), None, 0.0, GEOTHERM);
    let g = w.ground();
    assert!(!g.ground_water());
    for f in 0..6u8 {
        for (c, runs, _, t) in columns(&w, f, 11) {
            assert!(t.is_none() && g.aquifer(c).is_none());
            if !runs.is_empty() {
                assert!(g.generated_other(c).iter().all(|x| x.3 != crate::fluids::WATER));
            }
        }
    }
}

/// Two bakes of one seed give one table, and two worlds on them the same cave water.
#[test]
fn two_worlds_of_one_seed_agree() {
    let (a, b) = (Arc::new(crate::bake::Map::bake(5, 768)), crate::bake::shared(5, 768));
    assert_eq!((a.hash, a.version), (b.hash, crate::bake::VERSION));
    assert!(a.table == b.table && !a.table.is_empty());
    let [wa, wb] = [a, b].map(|m| World::with_grid(768, 32, 5, Some(m), None, 0.0, GROUND_WATER));
    let (ga, gb) = (wa.ground(), wb.ground());
    let mut wet = 0;
    for f in 0..6u8 {
        for (c, runs, _, _) in columns(&wa, f, 2) {
            if !runs.is_empty() {
                let x = ga.generated_other(c);
                wet += !x.is_empty() as usize;
                assert_eq!(x, gb.generated_other(c));
            }
        }
    }
    println!("{wet} cave columns with water, the same in both");
}
