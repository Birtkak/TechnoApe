//! Water's phases (liquids.md step 5): each rule against its hand calculation, and the one ledger.
use super::*;
use crate::liquid::test_ground::{col, stored, Box3};

/// ADR 0187's pace for the sky of real gravity: the real year over its 334 days of 1800 s (about 52.5).
fn pace() -> f64 {
    REAL_YEAR / (334.0 * crate::sky::DAY)
}

/// What a server does with `cool`'s cells in a box: frozen water's ice set solid (and followed), melted ice back to
/// air with its units poured there.
fn apply(g: &mut Box3, w: &mut Liquids, cells: Vec<(Col, i32, u8)>) -> usize {
    let n = cells.len();
    for (c, z, _) in cells {
        g.set(c.i(), c.j(), z, true);
        g.ice.push((c.i(), c.j(), z));
        w.regap(g, c);
        w.watch_ice(c, z);
    }
    for (c, z, units) in std::mem::take(&mut w.thawed) {
        g.set(c.i(), c.j(), z, false);
        g.ice.retain(|&x| x != (c.i(), c.j(), z));
        w.regap(g, c);
        if units > 0 {
            assert!(w.pour(g, c, z, units));
        }
    }
    n
}

/// Water's ledger: what differs from generation, what boiled off, and what is ice. Only a bucket changes it.
fn ledger(w: &Liquids) -> i64 {
    w.counted() + w.vapour.get(&0).copied().unwrap_or(0) + w.froze.get(&0).copied().unwrap_or(0)
}

/// An ice block (placed, 1 m3) on the ground in still air at +10 C melts to exactly its U units of water.
/// By hand: five faces in the air, h = 10 W/(m2 K) free convection + 4.9 radiation (4 e sigma T^3 at 283 K, e 0.96),
/// a shrinking cube's law t = 3 rho L / (5 h dT) = 3 x 1000 x 334000 / (5 x 14.9 x 10) = 1.34e6 s real, over the pace
/// (52.5): 2.6e4 s, 14 game days. The ice block holds its melt until the last unit, then it is water.
#[test]
fn an_ice_block_in_warm_air_becomes_exactly_its_units_of_water_in_its_time() {
    let mut g = Box3::new(3, 3, 3);
    g.air = (10.0, ATM);
    g.pace = pace();
    g.set(1, 1, 0, true);
    g.ice.push((1, 1, 0));
    let mut w = stored(&g);
    w.watch_ice(col(1, 1), 0);
    let (dt, mut t) = (60.0, 0.0);
    while !g.ice.is_empty() && t < 1e6 {
        let cells = w.cool(&g, dt);
        apply(&mut g, &mut w, cells);
        assert_eq!(w.total(), if g.ice.is_empty() { U } else { 0 }, "no water until it melted through");
        t += dt;
    }
    let ph = Phases::water();
    let hand = 3.0 * 1000.0 * 334_000.0 / (5.0 * ph.h(10.0, ATM) * 10.0) / pace();
    eprintln!("the block melted in {t:.0} s ({:.1} game days), by hand {hand:.0} s", t / crate::sky::DAY);
    assert!(g.ice.is_empty(), "it melted");
    assert!(t > hand / 2.0 && t < hand * 2.0, "melted in {t} s, by hand {hand} s");
    assert_eq!((w.total(), ledger(&w) - U, w.froze[&0]), (U, -U, -U), "exactly its units of water, booked from ice");
}

/// A pond 4 1/3 blocks deep under -10 C air grows ice from the top by Stefan's law with the air's resistance, at the
/// pace: t(d) = rho L / dT (d^2 / 2k + d / h) over the pace (k 2.22, h 14.0 at 263 K). By hand: the first block in
/// 1.89e5 s (105 game days), the second in 6.6e5 s. The water under the ice stays liquid, and every unit is booked.
#[test]
fn a_pond_in_freezing_air_grows_ice_from_the_top_at_the_stefan_rate_and_stays_liquid_under_it() {
    let mut g = Box3::new(1, 1, 6);
    g.air = (-10.0, ATM);
    g.pace = pace();
    g.heat = 0.0;
    let mut w = stored(&g);
    assert!(w.pour(&g, col(0, 0), 0, 4 * U + U / 3));
    let ph = Phases::water();
    let stefan = |d: f64| 1000.0 * ph.lf / ph.mu / 10.0 * (d * d / (2.0 * ph.ki) + d / ph.h(-10.0, ATM)) / pace();
    let (dt, mut t, mut at) = (60.0, 0.0, Vec::new());
    while at.len() < 2 && t < 2e6 {
        let cells = w.cool(&g, dt);
        t += dt;
        if apply(&mut g, &mut w, cells) > 0 {
            at.push(t);
        }
        assert_eq!(ledger(&w), 4 * U + U / 3, "every unit booked at {t} s");
    }
    for (k, &t) in at.iter().enumerate() {
        let hand = stefan(k as f64 + 1.0);
        eprintln!("ice block {} at {t:.0} s, Stefan {hand:.0} s", k + 1);
        assert!((t / hand - 1.0).abs() < 0.05, "block {}: {t} s against {hand} s", k + 1);
    }
    assert_eq!(g.ice, vec![(0, 0, 4), (0, 0, 3)], "from the top down");
    assert_eq!((w.total(), w.froze[&0]), (2 * U + U / 3, 2 * U), "two blocks of ice, the rest water under it");
    assert_eq!(w.temp_at(col(0, 0), 0), Some(0.0), "the water under the ice is at its freeze point, liquid");
}

/// A pot (an eighth of a block) on rock at 1000 C boils off to `vapour` exactly what it loses, at its boiling point:
/// 100 C at one atmosphere, 1.96 C at Mars-like 700 Pa (the spike's numbers).
#[test]
fn water_on_a_hot_place_boils_off_to_vapour_exactly_and_never_passes_its_boiling_point() {
    for (p, ta, tb) in [(ATM, 20.0, 100.0), (700.0, -20.0, 1.956)] {
        let mut g = Box3::new(1, 1, 3);
        (g.air, g.heat, g.pace) = ((ta, p), 1000.0, pace());
        let mut w = stored(&g);
        assert!(w.pour(&g, col(0, 0), 0, U / 8));
        let (mut t, mut top) = (0.0, f64::MIN);
        while w.total() > 0 && t < 1e7 {
            let cells = w.cool(&g, 10.0);
            assert!(cells.is_empty(), "nothing freezes on a hot place");
            top = top.max(w.temp_at(col(0, 0), 0).unwrap_or(f64::MIN));
            assert_eq!(ledger(&w), U / 8, "every unit booked");
            t += 10.0;
        }
        eprintln!("{p} Pa: dry in {t:.0} s, at most {top:.3} C");
        assert_eq!((w.total(), w.vapour[&0]), (0, U / 8), "{p} Pa: all of it boiled off, to vapour");
        assert!((top - tb).abs() < 0.05, "{p} Pa: boiled at {top} C, the curve's {tb}");
        assert!(!w.boils.is_empty(), "its steam's places");
    }
}

/// Water sealed in a pocket of rock held at 300 C (the geotherm's, a chamber's: it does not cool) takes the rock's heat
/// steadily through the floor and the four walls, and boils dry at that rate, every unit to `vapour`. By hand, the
/// water at 100 C and d m deep (4/3 at first): through half a block of rock (0.5 m / 2.0 W/(m K)) and half its depth
/// of water (k 0.6) from the floor, (300 - 100) / (0.25 + d / 1.2), and from each wall d (300 - 100) / (0.25 + 0.5 /
/// 0.6); t = the integral of rho Lv / P(d) from 0 to 4/3: 3.9e6 s (45 game days).
#[test]
fn water_sealed_in_on_held_rock_boils_dry_at_the_floor_and_walls_steady_rate() {
    let mut g = Box3::new(3, 3, 4);
    g.fill(0..3, 0..3, 0..4, true);
    g.fill(1..2, 1..2, 0..2, false);
    (g.heat, g.air, g.pace) = (300.0, (300.0, ATM), pace());
    let mut w = stored(&g);
    let units = U + U / 3;
    assert!(w.pour(&g, col(1, 1), 0, units));
    let ph = Phases::water();
    let p = |d: f64| 200.0 / (0.25 + d * 0.5 / ph.k) + 4.0 * d * 200.0 / (0.25 + 0.5 / ph.k);
    let (n, d0) = (10_000, units as f64 / U as f64);
    let hand: f64 = (0..n).map(|k| U as f64 * ph.lv / p((k as f64 + 0.5) * d0 / n as f64) * d0 / n as f64).sum();
    let (dt, mut t) = (600.0, 0.0);
    while w.total() > 0 && t < 2.0 * hand {
        assert!(w.cool(&g, dt).is_empty(), "nothing freezes");
        assert_eq!(ledger(&w), units, "every unit booked at {t} s");
        t += dt;
    }
    eprintln!("sealed on 300 C rock: dry in {t:.0} s, by hand {hand:.0} s");
    assert_eq!((w.total(), w.vapour.get(&0).copied()), (0, Some(units)), "all of it boiled off, to vapour");
    assert!((t / hand - 1.0).abs() < 0.1, "dry in {t} s, by hand {hand} s");
}

/// A 3 x 3 box of rock held at `rock` C with a shaft down its middle, open to air at `air` C: its floor and four walls
/// rock, `units` of water poured at its foot.
fn shaft(rock: f32, air: f64, units: i64) -> (Box3, Liquids) {
    let mut g = Box3::new(3, 3, 4);
    g.fill(0..3, 0..3, 0..4, true);
    g.fill(1..2, 1..2, 0..4, false);
    (g.heat, g.air, g.pace) = (rock, (air, ATM), pace());
    let mut w = stored(&g);
    assert!(w.pour(&g, col(1, 1), 0, units));
    (g, w)
}

/// The held rock's conductance to water d m deep in `shaft`'s, W/K: the floor's and the four walls' (half a block of
/// rock, 0.25 m^2 K/W, and half the water's depth or half a block of it).
fn held(ph: &Phases, d: f64) -> f64 {
    1.0 / (0.25 + d * 0.5 / ph.k) + 4.0 * d / (0.25 + 0.5 / ph.k)
}

/// Water on rock held over its boiling point that cannot reach it: in a shaft on 120 C rock under open air at -5 C its
/// steady heat, (G_rock 120 + h (-5)) / (G_rock + h), is about 30 C, so it rests there: no heat entry is ever held
/// (it is not stepped every second for ever), nothing boils, nothing freezes.
#[test]
fn water_on_held_rock_whose_steady_heat_is_under_boiling_rests_with_no_heat() {
    let units = U + U / 3;
    let (g, mut w) = shaft(120.0, -5.0, units);
    let ph = Phases::water();
    let (gr, ga) = (held(&ph, units as f64 / U as f64), ph.h(-5.0, ATM));
    eprintln!("its steady heat {:.1} C", (gr * 120.0 - ga * 5.0) / (gr + ga));
    for k in 0..310 {
        assert!(w.cool(&g, 600.0).is_empty(), "nothing freezes");
        assert!(k < 10 || w.heats().is_empty(), "settled, it holds no heat at step {k}: {:?}", w.heats());
    }
    assert_eq!((w.total(), w.vapour.get(&0).copied().unwrap_or(0)), (units, 0), "nothing boiled");
}

/// Open water in a shaft on rock held at 400 C under 20 C air boils at the rate its steady heat T* gives,
/// (G_rock(d) + h) (T* - 100) with h at the real rate (a source drives it), down to the depth where T* is its boiling
/// point (with `REST_K`): there it rests and drops its heat. By hand: from 4/3 to 1 block in the integral of
/// rho Lv / ((G_rock + h)(T* - 100)) over d, and the rest at G_rock(d) (400 - 100.5) = h (100.5 - 20), about 0.83 m.
#[test]
fn open_water_on_held_rock_boils_at_its_steady_heats_rate_down_to_where_it_rests() {
    let units = U + U / 3;
    let (g, mut w) = shaft(400.0, 20.0, units);
    let ph = Phases::water();
    let ga = ph.h(20.0, ATM);
    let net = |d: f64| held(&ph, d) * (400.0 - 100.0) - ga * (100.0 - 20.0);
    let (n, d0) = (10_000, units as f64 / U as f64);
    // (it comes at the air's 20 C and warms first: rho c d / G ln((T* - 20) / (T* - 100)), G = G_rock + h)
    let (gs, ts) = (held(&ph, d0) + ga, (held(&ph, d0) * 400.0 + ga * 20.0) / (held(&ph, d0) + ga));
    let warm = d0 * U as f64 * ph.c / gs * ((ts - 20.0) / (ts - 100.0)).ln();
    let hand: f64 = warm + (0..n).map(|k| U as f64 * ph.lv / net(1.0 + (k as f64 + 0.5) * (d0 - 1.0) / n as f64) * (d0 - 1.0) / n as f64).sum::<f64>();
    let rest = (0..100_000).map(|k| k as f64 * 1e-5 * d0).find(|&d| held(&ph, d) * (400.0 - 100.5) >= ga * (100.5 - 20.0)).unwrap();
    let (dt, mut t, mut at) = (600.0, 0.0, None);
    while (w.total() == units || !w.heats().is_empty()) && t < 1e8 {
        assert!(w.cool(&g, dt).is_empty(), "nothing freezes");
        assert_eq!(ledger(&w), units, "every unit booked at {t} s");
        t += dt;
        if at.is_none() && w.total() <= U {
            at = Some(t);
        }
    }
    let at = at.expect("it boiled down to a block");
    let left = w.total() as f64 / U as f64;
    eprintln!("to a block in {at:.0} s, by hand {hand:.0} s; at rest at {left:.3} m after {t:.0} s, by hand {rest:.3} m");
    assert!((at / hand - 1.0).abs() < 0.1, "to a block in {at} s, by hand {hand} s");
    assert!(w.heats().is_empty() && (left - rest).abs() < 0.02, "at rest {left} m deep, by hand {rest} m, heats {:?}", w.heats());
    for _ in 0..300 {
        w.cool(&g, dt);
        assert!(w.heats().is_empty(), "at rest it stays so");
    }
}

/// The boiling point by the place's pressure (the spike's table): 100 C at one atmosphere, 93.75 on a 1 km peak of
/// Terra at 19.62 m/s^2 (79,9 kPa), 1.96 at 700 Pa; under the triple point (Mars's 608 Pa) ice goes straight to gas
/// under its freeze point; in no air at absolute zero.
#[test]
fn water_boils_colder_on_a_peak_and_at_mars_like_pressure() {
    let ph = Phases::water();
    let peak = pressure(1000.0, 14.0, 19.62);
    for (p, want) in [(ATM, 100.0), (peak, 93.75), (49_673.0, 81.87), (33_437.0, 72.55), (700.0, 1.96), (608.0, -0.07), (0.0, -273.15)] {
        let t = ph.boils_at(p);
        eprintln!("{p:.0} Pa: {t:.2} C");
        assert!((t - want).abs() < 0.1, "{p} Pa: {t} C, the spike's {want}");
    }
    assert!((peak - 79_895.0).abs() < 300.0, "a 1 km peak: {peak} Pa");
}

/// With no air an ice block sublimates: its units go to `vapour` and no water is ever made.
#[test]
fn with_no_air_ice_goes_to_vapour_without_making_water() {
    let mut g = Box3::new(3, 3, 3);
    (g.air, g.pace) = ((15.0, 0.0), pace());
    g.set(1, 1, 0, true);
    g.ice.push((1, 1, 0));
    let mut w = stored(&g);
    w.watch_ice(col(1, 1), 0);
    let mut t = 0.0;
    while !g.ice.is_empty() && t < 1e7 {
        let cells = w.cool(&g, 60.0);
        apply(&mut g, &mut w, cells);
        assert_eq!(w.total(), 0, "no water at {t} s");
        t += 60.0;
    }
    eprintln!("sublimated in {t:.0} s ({:.1} game days)", t / crate::sky::DAY);
    assert!(g.ice.is_empty(), "it sublimated");
    assert_eq!((w.total(), w.vapour[&0], ledger(&w)), (0, U, 0), "all of it to vapour, booked from ice");
}

/// A sea in a box: the bed at z < 3, the generated sea up to 4 7/8 (the cell the bake's sheet takes is 4), its
/// middle column stored and the ring round it the open sea's.
fn sea(air: f64) -> (Box3, Liquids) {
    let mut g = Box3::new(5, 5, 9);
    g.fill(0..5, 0..5, 0..3, true);
    g.with_sea(4 * U + 7 * U / 8);
    (g.air, g.pace) = ((air, ATM), pace());
    let mut w = Liquids::default();
    w.materialize(&g, col(2, 2));
    assert!(w.pour(&g, col(2, 2), 4, 0));
    (g, w)
}

/// `secs` seconds of a box: a cool of `dt`, its cells applied, and `steps` water steps after each. The blocks set.
fn run(g: &mut Box3, w: &mut Liquids, secs: f64, dt: f64, steps: usize) -> usize {
    let (mut t, mut set) = (0.0, 0);
    while t < secs {
        let cells = w.cool(g, dt);
        set += apply(g, w, cells);
        for _ in 0..steps {
            w.step(g);
        }
        t += dt;
    }
    set
}

/// Units booked as ice.
fn frozen(w: &Liquids) -> i64 {
    w.froze.get(&0).copied().unwrap_or(0)
}

/// A box of one column, `deep` layers, rock at 0 C and air at `air` C (`p` Pa), `units` of water poured in.
fn pond(deep: i32, air: f64, p: f64, units: i64) -> (Box3, Liquids) {
    let mut g = Box3::new(1, 1, deep);
    (g.air, g.pace, g.heat) = ((air, p), pace(), 0.0);
    let mut w = stored(&g);
    assert!(w.pour(&g, col(0, 0), 0, units));
    (g, w)
}

/// Stored sea is left to the formula in this step (the review's decision A): a stored patch of polar sea under
/// -20 C air for a game week takes no heat, sets no ice and stores no column more.
#[test]
fn stored_sea_under_freezing_air_takes_no_heat_sets_no_ice_and_stores_no_more_columns() {
    let (mut g, mut w) = sea(-20.0);
    let n = w.stored();
    let set = run(&mut g, &mut w, 7.0 * crate::sky::DAY, 60.0, 30);
    assert_eq!((set, w.heats().len(), frozen(&w)), (0, 0, 0));
    assert!(w.stored() <= n, "{} columns stored, {n} at first", w.stored());
}

/// Less than a block of water never freezes (decision B): a puddle of 2000 units at -30 C for 23 game days holds
/// all its units liquid and nothing is booked as ice.
#[test]
fn a_puddle_short_of_a_block_never_freezes() {
    let (mut g, mut w) = pond(3, -30.0, ATM, 2000);
    let set = run(&mut g, &mut w, 2e6, 600.0, 0);
    assert_eq!((set, w.total(), frozen(&w)), (0, 2000, 0));
}

/// A pond a block and a half deep at -30 C ends as one ice block in its top cell over half a block of water.
#[test]
fn a_pond_of_a_block_and_a_half_ends_as_one_ice_block_over_half_a_block_of_water() {
    let (mut g, mut w) = pond(4, -30.0, ATM, 3 * U / 2);
    run(&mut g, &mut w, 3e6, 600.0, 0);
    assert_eq!((g.ice.clone(), w.total(), frozen(&w)), (vec![(0, 0, 1)], U / 2, U));
}

/// A pond freezing (short of its first block) drained from below, its water flowing into a pocket beside and under
/// it: no unit is lost and nothing is booked as ice without a block.
#[test]
fn a_freezing_pond_drained_from_below_loses_no_water() {
    let mut g = Box3::new(2, 1, 8);
    g.fill(0..1, 0..1, 0..4, true);
    g.fill(1..2, 0..1, 4..8, true);
    (g.air, g.pace, g.heat) = ((-30.0, ATM), pace(), 0.0);
    let mut w = stored(&g);
    assert!(w.pour(&g, col(0, 0), 4, 3 * U / 2));
    run(&mut g, &mut w, 1800.0, 60.0, 0);
    assert!(!w.heats().is_empty(), "freezing");
    g.set(1, 0, 4, false);
    w.regap(&g, col(1, 0));
    run(&mut g, &mut w, 3600.0, 60.0, 60);
    assert_eq!(frozen(&w), g.ice.len() as i64 * U, "ice booked only where a block is");
    assert_eq!(w.total() + frozen(&w), 3 * U / 2, "every unit liquid or a block of ice");
}

/// With no air a pit of 3 U in one step boils what its heat and its freeze's latent heat pay for, sets every whole
/// block of the rest as ice, and boils what is short of a block: ice and vapour are its 3 U to the unit.
#[test]
fn with_no_air_a_pit_of_three_blocks_ends_as_ice_blocks_and_vapour_to_the_unit() {
    let (mut g, mut w) = pond(5, -20.0, 0.0, 3 * U);
    let cells = w.cool(&g, 1.0);
    let n = apply(&mut g, &mut w, cells) as i64;
    let vap = w.vapour.get(&0).copied().unwrap_or(0);
    eprintln!("{n} blocks of ice, {vap} units of vapour");
    assert_eq!((w.total(), n * U + vap, frozen(&w), n), (0, 3 * U, n * U, 2));
}

/// A pond freezing under open air, roofed with stone, no longer touches the air: it freezes no further and is let
/// go (decision D).
#[test]
fn a_pond_roofed_with_stone_while_it_freezes_stops_freezing() {
    let (mut g, mut w) = pond(4, -30.0, ATM, 3 * U / 2);
    run(&mut g, &mut w, 1800.0, 60.0, 0);
    assert!(!w.heats().is_empty(), "freezing");
    g.set(0, 0, 2, true);
    w.regap(&g, col(0, 0));
    let set = run(&mut g, &mut w, 3e6, 600.0, 0);
    assert_eq!((set, w.total(), frozen(&w), w.heats().len()), (0, 3 * U / 2, 0, 0));
}

/// Water is never hotter than what heats it (decision E): a puddle half frozen short of a block, in +10 C air at a
/// step of 10,000 s (the server's step at the most time warp), melts and boils nothing.
#[test]
fn a_melt_in_warm_air_at_a_coarse_step_boils_nothing() {
    let (g, mut w) = pond(3, 10.0, ATM, 100);
    let a = w.cols[&col(0, 0)][0];
    w.heat.insert(a, Therm { t: 0.0, age: 0.0, frac: 50.0, solid: 0 });
    for _ in 0..10 {
        w.cool(&g, 10_000.0);
    }
    assert_eq!((w.total(), w.vapour.get(&0).copied().unwrap_or(0), w.heats().len()), (100, 0, 0));
}

/// Water under an ice lid placed on it at -3 C gains nothing from the lid (decision E): a pond and a stored patch
/// of sea for a game year never pass 4 C and boil nothing.
#[test]
fn water_under_a_placed_ice_lid_at_minus_three_never_warms() {
    let (mut pond, mut pw) = pond(4, -3.0, ATM, 3 * U / 2);
    let (mut sea, mut sw) = sea(-3.0);
    for (g, w, c, z, steps) in [(&mut pond, &mut pw, (0, 0), 2, 0), (&mut sea, &mut sw, (2, 2), 5, 30)] {
        g.set(c.0, c.1, z, true);
        g.ice.push((c.0, c.1, z));
        w.regap(g, col(c.0, c.1));
        w.watch_ice(col(c.0, c.1), z);
        let (mut t, mut top) = (0.0, f64::MIN);
        while t < 334.0 * crate::sky::DAY {
            let cells = w.cool(g, 600.0);
            apply(g, w, cells);
            for _ in 0..steps {
                w.step(g);
            }
            top = top.max(w.temp_at(col(c.0, c.1), 0).unwrap_or(f64::MIN).max(w.temp_at(col(c.0, c.1), 4).unwrap_or(f64::MIN)));
            t += 600.0;
        }
        eprintln!("under the lid at most {top:.2} C");
        assert!(top <= 4.0, "at most {top} C");
        assert_eq!(w.vapour.get(&0).copied().unwrap_or(0), 0, "nothing boiled");
    }
}

/// A save's heats come back on their own fluid (decision G): lava's and water's in layers of one gap, saved and
/// restored, each on its own node, whichever was poured first.
#[test]
fn saved_heats_come_back_on_their_own_fluid() {
    for first in [1u8, 0] {
        let g = Box3::new(1, 1, 4);
        let mut w = stored(&g);
        for k in [first, 1 - first] {
            assert!(w.pour_kind(&g, col(0, 0), 0, U / 2, k));
        }
        let a = w.cols[&col(0, 0)].iter().copied().find(|&i| w.nodes[i as usize].kind == 0).unwrap();
        w.heat.insert(a, Therm { t: 3.0, age: 0.0, frac: 100.0, solid: 0 });
        let by_kind = |w: &Liquids| {
            let mut v: Vec<(u8, f64, f64)> = w.cols[&col(0, 0)].iter().filter_map(|&i| w.heat.get(&i).map(|h| (w.nodes[i as usize].kind, h.t, h.frac))).collect();
            v.sort_by(|x, y| x.partial_cmp(y).unwrap());
            v
        };
        for rev in [false, true] {
            let (mut heats, mut kinds) = (w.heats(), w.heat_kinds());
            if rev {
                heats.reverse();
                kinds.reverse();
            }
            let mut b = Liquids::default();
            b.restore(&g, &w.all());
            b.restore_heat(&g, &heats, &kinds);
            assert_eq!(by_kind(&b), by_kind(&w), "{} poured first, the list reversed {rev}", if first == 1 { "lava" } else { "water" });
        }
    }
}

/// Lava that came to rest where its place holds it molten (its heat entry dropped, as `cool` drops it) is restored
/// from a save without a heat: it gets none, so it is not molten at its pour heat again (heat from nothing). Lava
/// without a heat at a cold place (a save from before heat) still gets its pour heat.
#[test]
fn resting_lava_at_a_hot_place_comes_back_without_new_heat() {
    let lava = crate::fluids::FLUIDS.now().kind("lava").unwrap();
    for (hot, want) in [(1100.0f32, None), (AMBIENT, Some(1150.0))] {
        let mut g = Box3::new(1, 1, 4);
        g.heat = hot;
        let mut w = Liquids::default();
        assert!(w.pour_kind(&g, col(0, 0), 0, U, lava));
        w.heat.clear();
        let mut b = Liquids::default();
        b.restore(&g, &w.all());
        b.restore_heat(&g, &w.heats(), &w.heat_kinds());
        assert_eq!(b.temp_at(col(0, 0), 0), want, "ground at {hot} C");
    }
}

/// A water layer boiled dry under oil on hot rock goes as every emptied layer does (decision H): no dry node of
/// water's kind is left under the oil, and every unit is in `vapour`.
#[test]
fn water_under_oil_boiled_dry_leaves_no_dry_water_layer() {
    let oil = 2u8;
    let mut g = Box3::new(1, 1, 4);
    (g.heat, g.pace) = (1000.0, pace());
    let mut w = stored(&g);
    assert!(w.pour(&g, col(0, 0), 0, U / 8));
    w.cool(&g, 1.0);
    assert!(!w.heats().is_empty(), "on the hot rock it holds a heat");
    assert!(w.pour_kind(&g, col(0, 0), 0, U, oil));
    // looked at as it boils dry, before a step tidies the gap
    for _ in 0..100_000 {
        w.cool(&g, 10.0);
        if w.total_of(0) == 0 {
            break;
        }
        for _ in 0..4 {
            w.step(&g);
        }
    }
    let kinds: Vec<(u8, i64)> = w.column(col(0, 0)).unwrap().map(|n| (n.kind, n.water)).collect();
    assert!(kinds.iter().all(|k| k.0 == oil), "{kinds:?}");
    assert_eq!((w.total_of(0), w.vapour.get(&0).copied()), (0, Some(U / 8)));
}

/// Water alone, where nothing freezes or boils, is given no heat: its maths and its save are what they were.
#[test]
fn water_at_rest_in_mild_air_carries_nothing() {
    let mut g = Box3::new(3, 3, 3);
    g.pace = pace();
    let mut w = stored(&g);
    assert!(w.pour(&g, col(1, 1), 0, U));
    for _ in 0..100 {
        assert!(w.cool(&g, 60.0).is_empty());
    }
    assert!(w.heats().is_empty() && w.froze.is_empty() && w.vapour.is_empty());
}
