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
