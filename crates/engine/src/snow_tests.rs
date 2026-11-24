//! Snow and ice in eighths: each rule against its hand calculation, and the ledger through fall, melt and sublimation.
use super::*;
use crate::liquid::phases::ATM;
use crate::liquid::test_ground::{col, Box3};

/// A 3 x 3 box with a floor of one block: the snow lies in cell 1.
fn ground(ta: f64, p: f64) -> Box3 {
    let mut g = Box3::new(3, 3, 4);
    g.fill(0..3, 0..3, 0..1, true);
    g.air = (ta, p);
    g
}

/// The one live column, (1, 1).
fn one() -> BTreeSet<Col> {
    [col(1, 1)].into_iter().collect()
}

/// Water: what the snow holds less what it took from the air plus what it sublimated is 0; the air: what it took less
/// what it still owes is what was settled (none here, so 0).
fn books(s: &Snow, vapour: i64) -> (i64, i64) {
    (s.held() - s.lent + vapour, s.lent - s.owing())
}

/// Millimetres of water an hour as the snow's units a second on a square metre.
fn mm_h(r: f64) -> f64 {
    r / 3600.0 * PER_MM
}

/// The weather's snow (`/weather snow`, 3 mm of water an hour, the live grid's heavy snowfall) at -6 C lays by hand
/// 3 x 4.096 = 12.3 units an hour on a square metre: a fifth of an eighth (an eighth is 64 units, 15.6 mm of water, at
/// 125 kg/m^3), the first eighth showing after 5.2 h. The layer holds when it stops (no melt under freezing, no
/// sublimation in air). A column joining starts from the formula's units.
#[test]
fn snow_builds_under_falling_snow_at_the_rate_by_hand_and_holds_when_it_stops() {
    let (g, ph) = (ground(-6.0, ATM), Phases::water());
    let (mut s, mut vap) = (Snow::default(), 0);
    let mut fall = mm_h(3.0);
    let here = |f: f64| move |_: Col, _: i32| Some(Here { cell: 7, formula: 30.4, fall: f });
    s.step(&g, 0.0, &one(), &here(fall), &ph, &mut vap);
    assert_eq!(s.cols[&col(1, 1)].units, 30, "it joins with the formula's whole units");
    let (mut t, mut first) = (0.0, None);
    while t < 3600.0 {
        s.step(&g, 1.0, &one(), &here(fall), &ph, &mut vap);
        t += 1.0;
    }
    let hour = s.cols[&col(1, 1)].units - 30;
    eprintln!("an hour of 3 mm/h: {hour} units, by hand {:.2}", 3.0 * PER_MM);
    assert_eq!(hour, (3.0 * PER_MM) as i64, "an hour's fall by hand");
    while t < 8.0 * 3600.0 {
        s.step(&g, 1.0, &one(), &here(fall), &ph, &mut vap);
        t += 1.0;
        if first.is_none() && s.seen.contains_key(&col(1, 1)) {
            first = Some(t);
        }
    }
    let hand = (EIGHTH - 30) as f64 / mm_h(3.0);
    eprintln!("the first eighth shows at {:.0} s, by hand {hand:.0} s", first.unwrap());
    assert!((first.unwrap() - hand).abs() <= 2.0, "the first eighth at {first:?} s, by hand {hand} s");
    let units = s.cols[&col(1, 1)].units;
    assert_eq!(units, 30 + (8.0 * 3.0 * PER_MM) as i64, "eight hours' fall");
    assert_eq!((s.seen[&col(1, 1)].eighths, s.seen[&col(1, 1)].z), (2, 1), "two eighths after eight hours, on the floor's cell");
    fall = 0.0;
    let kept = s.cols[&col(1, 1)].units;
    for _ in 0..3600 {
        s.step(&g, 1.0, &one(), &here(fall), &ph, &mut vap);
    }
    assert_eq!(s.cols[&col(1, 1)].units, kept, "it holds under freezing air once the snow stops");
    assert_eq!(books(&s, vap), (0, 0));
}

/// An eighth (64 units) in still air at +5 C melts at h dT over the latent heat, on the world's heat: h = (10 + 4 e sigma
/// T^3) x 6 = 87.8 W/(m^2 K), Lf a unit 334 kJ/kg x 0.244 kg / 50 = 1631 J: 0.269 units a second, so the eighth goes in
/// 238 s (real: 300 times, 20 h). Its water soaks in, owed to the air column's soil.
#[test]
fn an_eighth_of_snow_melts_at_plus_five_in_the_time_by_hand() {
    let (g, ph) = (ground(5.0, ATM), Phases::water());
    let (mut s, mut vap) = (Snow::default(), 0);
    let here = |_: Col, _: i32| Some(Here { cell: 7, formula: EIGHTH as f64, fall: 0.0 });
    s.step(&g, 0.0, &one(), &here, &ph, &mut vap);
    assert_eq!(s.seen[&col(1, 1)].eighths, 1);
    let mut t = 0.0;
    while s.cols[&col(1, 1)].units > 0 && t < 1e4 {
        s.step(&g, 1.0, &one(), &here, &ph, &mut vap);
        t += 1.0;
    }
    let hand = EIGHTH as f64 * ph.lf / (ph.h(5.0, ATM) * 5.0);
    eprintln!("an eighth melted at +5 C in {t:.0} s, by hand {hand:.0} s");
    assert!((t - hand).abs() <= 2.0, "{t} s against {hand} s");
    assert!((hand - 238.0).abs() < 2.0, "by hand {hand}");
    assert!(!s.seen.contains_key(&col(1, 1)), "nothing shows");
    assert_eq!((s.owed[&7], vap), ((EIGHTH, -EIGHTH), 0), "taken from the lying snow, soaked into the soil");
    assert_eq!(books(&s, vap), (0, 0));
}

/// The ledger through fall, melt and sublimation, with amounts that are no whole units: a column joins with 100.7
/// units of the formula's, 0.37 units a second fall at -3 C for 1000 s, the air turns +2 C, then the column is set on
/// an airless place (under the triple point it sublimates, to vapour). At every step what the snow holds is what it
/// took less what soaked in or went to vapour, and the air is owed exactly that.
#[test]
fn the_ledger_balances_through_fall_melt_and_sublimation() {
    let ph = Phases::water();
    let (mut s, mut vap) = (Snow::default(), 0i64);
    let mut g = ground(-3.0, ATM);
    let here = |f: f64| move |_: Col, _: i32| Some(Here { cell: 3, formula: 100.7, fall: f });
    for k in 0..4000 {
        let f = if k < 1000 { 0.37 } else { 0.0 };
        if k == 1000 {
            g.air = (2.0, ATM);
        }
        if k == 1300 {
            g.air = (-20.0, 0.0);
        }
        s.step(&g, 1.0, &one(), &here(f), &ph, &mut vap);
        assert_eq!(books(&s, vap), (0, 0), "step {k}");
        if k == 999 {
            assert_eq!(s.cols[&col(1, 1)].units, 100 + 369, "the fall's 370 units, the last short of a whole one");
        }
    }
    let (taken, soaked) = s.owed[&3];
    eprintln!("took {taken}, soaked {soaked}, sublimated {vap}, holds {}", s.held());
    assert!(soaked < 0 && vap > 0, "it melted and it sublimated");
    assert_eq!(taken + soaked - vap, s.held(), "every unit");
}

/// A column leaving the live set gives back what it holds to the air column's lying snow, and one joining again takes
/// the formula's: no unit made or lost, whatever fell or melted while it was live.
#[test]
fn a_column_leaving_and_rejoining_the_live_set_keeps_the_water() {
    let (g, ph) = (ground(-8.0, ATM), Phases::water());
    let (mut s, mut vap) = (Snow::default(), 0);
    let here = |_: Col, _: i32| Some(Here { cell: 5, formula: 70.0, fall: 0.05 });
    for _ in 0..600 {
        s.step(&g, 1.0, &one(), &here, &ph, &mut vap);
    }
    let held = s.held();
    assert_eq!(held, 70 + 30);
    s.step(&g, 1.0, &BTreeSet::new(), &here, &ph, &mut vap);
    assert!(s.cols.is_empty() && s.seen.is_empty(), "it left");
    assert_eq!((s.lent, s.owed[&5]), (0, (0, 0)), "all it took given back to the lying snow");
    s.step(&g, 0.0, &one(), &here, &ph, &mut vap);
    assert_eq!((s.cols[&col(1, 1)].units, s.lent, s.owed[&5]), (70, 70, (70, 0)), "joined again from the formula");
    assert_eq!(books(&s, vap), (0, 0));
}

/// Eight eighths are a block of snow, set (packed, its units still the snow's); melting down to it takes it back.
#[test]
fn eight_eighths_of_snow_are_a_block_and_melt_back_into_eighths() {
    let ph = Phases::water();
    let mut g = ground(-8.0, ATM);
    let (mut s, mut vap) = (Snow::default(), 0);
    let here = |f: f64| move |_: Col, _: i32| Some(Here { cell: 1, formula: 500.0, fall: f });
    s.step(&g, 0.0, &one(), &here(1.0), &ph, &mut vap);
    let edits = s.step(&g, 20.0, &one(), &here(1.0), &ph, &mut vap);
    let snow = BLOCKS.id("snow");
    assert_eq!(edits, vec![(col(1, 1), 1, snow)], "the block set in the floor's cell");
    g.set(1, 1, 1, true);
    s.touch(col(1, 1));
    assert_eq!((s.cols[&col(1, 1)].floor, s.cols[&col(1, 1)].units, s.held()), (2, 8, 520));
    g.air = (5.0, ATM);
    let mut back = Vec::new();
    for _ in 0..200 {
        back.extend(s.step(&g, 1.0, &one(), &here(0.0), &ph, &mut vap));
    }
    assert_eq!(back, vec![(col(1, 1), 1, crate::blocks::AIR)], "melted down to it, the block taken back");
    assert!(s.packed.is_empty() && s.cols[&col(1, 1)].floor == 1);
    assert_eq!(books(&s, vap), (0, 0));
}
