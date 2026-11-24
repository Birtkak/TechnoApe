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

/// A pond 4 blocks deep under -10 C air grows ice from the top by Stefan's law with the air's resistance, at the
/// pace: t(d) = rho L / dT (d^2 / 2k + d / h) over the pace (k 2.22, h 14.0 at 263 K). By hand: the first block in
/// 1.89e5 s (105 game days), the second in 6.6e5 s. The water under the ice stays liquid, and every unit is booked.
#[test]
fn a_pond_in_freezing_air_grows_ice_from_the_top_at_the_stefan_rate_and_stays_liquid_under_it() {
    let mut g = Box3::new(1, 1, 6);
    g.air = (-10.0, ATM);
    g.pace = pace();
    g.heat = 0.0;
    let mut w = stored(&g);
    assert!(w.pour(&g, col(0, 0), 0, 4 * U));
    let ph = Phases::water();
    let stefan = |d: f64| 1000.0 * ph.lf / ph.mu / 10.0 * (d * d / (2.0 * ph.ki) + d / ph.h(-10.0, ATM)) / pace();
    let (dt, mut t, mut at) = (60.0, 0.0, Vec::new());
    while at.len() < 2 && t < 2e6 {
        let cells = w.cool(&g, dt);
        t += dt;
        if apply(&mut g, &mut w, cells) > 0 {
            at.push(t);
        }
        assert_eq!(ledger(&w), 4 * U, "every unit booked at {t} s");
    }
    for (k, &t) in at.iter().enumerate() {
        let hand = stefan(k as f64 + 1.0);
        eprintln!("ice block {} at {t:.0} s, Stefan {hand:.0} s", k + 1);
        assert!((t / hand - 1.0).abs() < 0.05, "block {}: {t} s against {hand} s", k + 1);
    }
    assert_eq!(g.ice, vec![(0, 0, 3), (0, 0, 2)], "from the top down");
    assert_eq!((w.total(), w.froze[&0]), (2 * U, 2 * U), "two blocks of ice, two of water under it");
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

/// Seconds of a sea in a box: a cool of `dt` and enough water steps that the open sea levels the stored column.
fn run(g: &mut Box3, w: &mut Liquids, secs: f64, dt: f64) {
    let mut t = 0.0;
    while t < secs {
        let cells = w.cool(g, dt);
        apply(g, w, cells);
        for _ in 0..30 {
            w.step(g);
        }
        t += dt;
    }
}

/// A stored patch of polar sea where the bake shows sea ice (the year's mean -10 C, under the bake's -8): it freezes
/// to one block of ice in the cell the bake's sheet fills and holds there (the formula's rest at -10 C is 1.28 m, one
/// block and a part short of the next); the sea under it stays liquid, and every unit is booked.
#[test]
fn a_stored_patch_of_polar_sea_freezes_to_one_block_of_ice_and_holds() {
    let (mut g, mut w) = sea(-10.0);
    let ph = Phases::water();
    let rest = ph.rest_ice(-10.0);
    assert!((1.0..2.0).contains(&rest), "the formula: {rest} m");
    let year = 334.0 * crate::sky::DAY;
    run(&mut g, &mut w, 3.0 * year, 600.0);
    // one block a column, in the bake's cell (a block's edit stores the columns beside it, which this box's sea, with
    // no sheet of its own, freezes in their turn)
    let sheet = |g: &Box3| {
        let mut s = g.ice.clone();
        s.sort_unstable();
        assert!(s.iter().all(|x| x.2 == 4) && s.windows(2).all(|p| (p[0].0, p[0].1) != (p[1].0, p[1].1)), "{s:?}");
        assert!(s.contains(&(2, 2, 4)), "{s:?}");
        s
    };
    let first = sheet(&g);
    let held: Vec<i64> = w.heats().iter().map(|h| h.2.solid).collect();
    run(&mut g, &mut w, 3.0 * year, 600.0);
    let again = sheet(&g);
    assert!(first.iter().all(|x| again.contains(x)), "and it holds");
    // the columns frozen a block for a year or more: what lies under the block is the formula's part
    let part: Vec<i64> = w.heats().iter().filter(|h| first.contains(&(h.0.i(), h.0.j(), 4))).map(|h| h.2.solid).collect();
    eprintln!("{} blocks, then {}; under them {held:?} then {part:?} units of ice short of a block, the formula {:.0}", first.len(), again.len(), (rest - 1.0) * U as f64);
    assert_eq!(part.len(), first.len());
    assert!(part.iter().all(|&p| (p as f64 / U as f64 - (rest - 1.0)).abs() < 0.03), "{part:?} units: the formula's {rest} m");
    assert!(w.total() > 0 && w.temp_at(col(2, 2), 3) == Some(0.0), "liquid under it");
    assert_eq!(ledger(&w), 0, "every unit booked");
}

/// A stored patch of temperate sea a cold spell chilled (a game week at -5 C) settles back liquid when the air warms
/// (+5 C): no ice, and it carries no temperature, so it costs what untouched sea does.
#[test]
fn a_stored_patch_of_temperate_sea_stays_liquid_and_carries_no_temperature_after_it_settles() {
    let (mut g, mut w) = sea(-5.0);
    run(&mut g, &mut w, 7.0 * crate::sky::DAY, 60.0);
    assert!(w.temp_at(col(2, 2), 4).is_some(), "the cold spell held it off its rest");
    g.air.0 = 5.0;
    run(&mut g, &mut w, 14.0 * crate::sky::DAY, 60.0);
    assert!(g.ice.is_empty(), "no ice");
    assert!(w.heats().is_empty(), "no temperature carried: {:?}", w.heats());
    assert_eq!(ledger(&w), 0, "every unit booked");
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
