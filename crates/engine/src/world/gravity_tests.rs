//! Real gravity (generation 8, ADR 0180): the periods by Kepler's law from the masses, the sun's mass from its row,
//! tides from the masses and distances. Every number here is derived; `docs/design/real-gravity-changes.md` lists
//! them against the generations before.
use super::*;
use std::f64::consts::TAU;

fn real() -> World {
    World::with_grid(6144, 1664, 1, None, None, 0.0, REAL)
}

/// Kepler's period of body i round its parent (the sun for a planet), seconds: two bodies, the one G.
fn kepler(w: &World, i: usize) -> f64 {
    let b = &w.bodies[i];
    let k = w.bodies[0].interior.k;
    let parent = if b.parent == INERTIAL { w.sun_mass() } else { w.bodies[b.parent].interior.mass };
    TAU * (b.orbit.powi(3) / (k * (parent + b.interior.mass))).sqrt()
}

/// Every orbit's period follows from the masses and distances, within 1 % of Kepler's law (the rounding to a whole
/// fraction of the sky's repeat, `Sky::repeat`); the sun's mass is its row's density in the radius the sky gives it;
/// the year comes out near 300 days. A world of generation 7 keeps its periods set by hand.
#[test]
fn every_period_follows_from_the_masses() {
    let w = real();
    let rho = crate::bodies::BODIES[0].sun_density.unwrap();
    assert_eq!(w.sun_mass(), rho * 4.0 / 3.0 * std::f64::consts::PI * w.sky.sun_radius.powi(3));
    for i in 0..w.bodies.len() {
        let (got, want) = (w.bodies[i].period, kepler(&w, i));
        assert!((got / want - 1.0).abs() < 0.01, "{}: {got} s, Kepler {want} s", w.bodies[i].name);
        let k = w.sky.repeat / got;
        assert_eq!(k, k.round(), "{}: a whole fraction of the repeat", w.bodies[i].name);
        println!("{}: period {got:.0} s ({:.2} days), Kepler {want:.0} s", w.bodies[i].name, got / crate::sky::DAY);
    }
    assert_eq!((w.sky.year, w.sky.month), (w.bodies[0].period, w.bodies[1].period));
    let days = w.sky.year / crate::sky::DAY;
    assert!((250.0..400.0).contains(&days), "a year of {days} days");
    // the sky still repeats, and a day is still 30 minutes
    for t in [0.0, 1234.5, 3.0e6] {
        for i in 0..w.bodies.len() {
            assert!(w.pose(i, t + w.sky.repeat).centre.distance(w.pose(i, t).centre) < 1e-3, "body {i} repeats");
        }
    }
    let old = World::with_grid(6144, 1664, 1, None, None, 0.0, 7);
    assert_eq!((old.sky.year, old.bodies[1].period, old.sky.repeat), (crate::sky::YEAR, crate::sky::MONTH, crate::sky::PERIOD));
}

/// A moon keeps its face to its parent (it turns once a sidereal period, not against the home's year), and its
/// place goes round once a period of its own.
#[test]
fn moons_are_locked_to_their_own_parent() {
    let w = real();
    for i in [1, 4] {
        let p = w.bodies[i].parent;
        for t in [0.0, 777.0, 0.3 * w.bodies[i].period, 5.0e5] {
            let (m, c) = (w.pose(i, t), w.pose(p, t).centre);
            let to_parent = (c - m.centre).normalize();
            assert!((m.rot * -DVec3::X).dot(to_parent) > 1.0 - 1e-9, "{} faces its parent at {t}", w.bodies[i].name);
            let back = w.pose(i, t + w.bodies[i].period);
            assert!((back.centre - w.pose(p, t + w.bodies[i].period).centre).distance(m.centre - c) < 1e-3, "{} round once a period", w.bodies[i].name);
        }
    }
}

/// The tide follows from the masses and distances (`sky::tide_rise`): Luna's and the sun's rise on Terra are their
/// masses over hers by the cube of her radius over their distance, millimetres; every other raiser's is under a
/// micrometre, so the sea sums those two; the sea's quads are meshed for the derived range, not a metre. Before
/// real gravity the hand-set metre at springs.
#[test]
fn tides_follow_from_the_masses() {
    let w = real();
    let (t, l) = (&w.bodies[0], &w.bodies[1]);
    let rise = |m: f64, d: f64| crate::sky::tide_rise(m, t.interior.mass, t.radius, d);
    assert_eq!((w.sky.tide_moon, w.sky.tide_sun), (rise(l.interior.mass, w.sky.moon_orbit), rise(w.sun_mass(), w.sky.sun_dist)));
    assert!(w.sky.tide_moon > 1e-4 && w.sky.tide_moon < 1e-3 && w.sky.tide_sun < w.sky.tide_moon, "{} {}", w.sky.tide_moon, w.sky.tide_sun);
    for j in 2..w.bodies.len() {
        let b = &w.bodies[j];
        // at its closest: a planet's orbit less Terra's, a moon of another its parent's
        let a = if b.parent == INERTIAL { b.orbit } else { w.bodies[b.parent].orbit };
        assert!(rise(b.interior.mass, (a - t.orbit).abs()) < 1e-6, "{}'s tide on Terra", b.name);
    }
    assert_eq!(t.tide_high, 1.5 * (w.sky.tide_moon + w.sky.tide_sun));
    let tide = w.tide();
    assert_eq!((tide.h_luna, tide.h_sun), (w.sky.tide_moon, w.sky.tide_sun));
    let old = World::with_grid(6144, 1664, 1, None, None, 0.0, 7);
    assert_eq!((old.sky.tide_moon, old.sky.tide_sun, old.bodies[0].tide_high), (crate::sky::TIDE_LUNA, crate::sky::TIDE_SUN, crate::sky::TIDE_HIGH));
    println!("Terra's tide: Luna's range {:.3} mm, the sun's {:.3} mm, springs {:.3} mm", 1.5e3 * w.sky.tide_moon, 1.5e3 * w.sky.tide_sun, 1e3 * t.tide_high);
}

/// The numbers of `docs/design/real-gravity-changes.md`, generation 7 against 8 on the 4 km planet
/// (`cargo test -p engine --lib the_numbers -- --nocapture`). Eclipses: a new (full) moon whose centre passes
/// within the sum of the discs' and the parallax's reach of the sun's (the shadow's) is one somewhere on Terra.
#[test]
fn the_numbers() {
    let day = crate::sky::DAY;
    for gen in [7, REAL] {
        let w = World::with_grid(6144, 1664, 1, None, None, 0.0, gen);
        let (t, s) = (&w.bodies[0], &w.sky);
        let month = 1.0 / (1.0 / s.month - 1.0 / s.year);
        // (before 8 Body::period of a moon is its month against the home's year)
        let synodic = if gen >= REAL { month } else { s.month };
        println!("generation {gen}: G {:.4e}, sun radius {:.0} m mass {:.4e} kg ({:.0} kg/m3, {:.3e} Terras), {:.3} deg from Terra; year {:.2} days, season {:.2} days; Luna's new-moon month {:.2} days, {:.2} new moons a year; tide Luna {:.4} m sun {:.4} m high {:.4} m",
            t.interior.k, s.sun_radius, s.sun_mass, s.sun_mass / (4.0 / 3.0 * std::f64::consts::PI * s.sun_radius.powi(3)), s.sun_mass / t.interior.mass, 2.0 * (s.sun_radius / s.sun_dist).asin().to_degrees(), s.year / day, s.year / 4.0 / day, synodic / day, s.year / synodic, 1.5 * s.tide_moon, 1.5 * s.tide_sun, t.tide_high);
        for (i, b) in w.bodies.iter().enumerate() {
            let size = if i == 0 { 0.0 } else { 2.0 * (b.radius / ((w.pose(i, 0.0).centre - w.pose(0, 0.0).centre).length())).asin().to_degrees() };
            println!("  {}: radius {:.0} m, mass {:.4e} ({:.4} Terras), mean {:.0} kg/m3, pull {:.3} m/s2 (weight {:.3}), orbit {:.0} m, period {:.0} s ({:.3} days), day {:.0} s, seen from Terra {:.4} deg at t 0, field {:.4}",
                b.name, b.radius, b.interior.mass, b.interior.mass / t.interior.mass, b.interior.mass / (4.0 / 3.0 * std::f64::consts::PI * b.radius.powi(3)), b.interior.surface_g, b.interior.weight_g, b.orbit, b.period, b.period / day, b.day, size, w.field(i));
        }
        // eclipses over 40 years: at each new and full moon, the angle between the sun and Luna (or Terra's shadow)
        let (mut solar, mut lunar) = (0, 0);
        let (reach_s, reach_l) = (((t.radius + s.moon_radius) / s.moon_orbit).asin() + (s.sun_radius / s.sun_dist).asin(), ((t.radius + s.moon_radius) / s.moon_orbit).asin());
        let years = 40.0;
        // Luna's angle from the sun seen from Terra's centre, and from the anti-sun
        let el = |x: f64| {
            let (p, m) = (w.sky.planet(x).centre, w.sky.moon(x).centre);
            ((m - p).normalize().dot((-p).normalize())).clamp(-1.0, 1.0).acos()
        };
        let least = |f: &dyn Fn(f64) -> f64, mut a: f64, mut b: f64| {
            for _ in 0..50 {
                let (m1, m2) = (a + (b - a) / 3.0, b - (b - a) / 3.0);
                if f(m1) > f(m2) { a = m1 } else { b = m2 }
            }
            f(a)
        };
        let dt = synodic / 400.0;
        let (sun_off, shadow_off) = (|x: f64| el(x), |x: f64| std::f64::consts::PI - el(x));
        let mut x = dt;
        while x < years * s.year {
            for (f, reach, count) in [(&sun_off as &dyn Fn(f64) -> f64, reach_s, &mut solar), (&shadow_off, reach_l, &mut lunar)] {
                if f(x) < f(x - dt) && f(x) <= f(x + dt) {
                    *count += (least(f, x - dt, x + dt) < reach) as u32;
                }
            }
            x += dt;
        }
        println!("  eclipses a year somewhere on Terra: of the sun {:.2}, of Luna {:.2}", solar as f64 / years, lunar as f64 / years);
    }
}

/// The seasons, the plants' year and Terra's baked climate follow the world's year: a quarter of real gravity's
/// 334 days is summer; a map of version 8 is baked over that year (its climate another than the 54-day year's of
/// version 7, its ground the same), and a world before real gravity stands on a version 7 map.
#[test]
fn seasons_and_the_bake_follow_the_year() {
    let w = real();
    let y = w.sky.year;
    assert!(w.bodies.iter().all(|b| b.year == y), "every body has its world's year");
    let north = DVec3::new(0.3, 0.0, 0.9).normalize();
    assert!((crate::sky::season_at(north, 0.25 * y, y).0 - 1.0).abs() < 1e-9, "summer a quarter year on");
    assert_eq!((crate::clutter::year(1.5 * y, y), crate::clutter::year(1.5 * crate::sky::YEAR, y)), (1, 0));
    assert!(crate::sky::next_season(0.0, 2, y) > 0.49 * y);
    let (new, old) = (crate::bake::Map::bake_v8(5, 256), crate::bake::Map::bake_v7(5, 256));
    assert_eq!((new.version, new.year(), old.version, old.year()), (8, crate::world::year_of(256, REAL), 7, crate::sky::YEAR));
    assert_eq!(new.h, old.h, "the same ground");
    assert_ne!(new.climate, old.climate, "a year 6 times longer bakes another climate");
    println!("year {} days at planet 256; the 4 km planet's {} days", new.year() / crate::sky::DAY, crate::world::year_of(6144, REAL) / crate::sky::DAY);
}
