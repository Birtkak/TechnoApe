//! F10's sights in the sky (ADR 0202): eclipses and Luna's phases found in the world's own orbits.
use crate::nav::elongation;
use glam::DVec3;

/// The next closest line-up from `from` on, before `until`: the moment `off` (how far a body's centre is off a
/// line, f64::MAX on the wrong side) has a minimum under `near`. Searched forward in `step`s, then narrowed.
/// (`closing`: the distance was seen falling, so a search begun just past one line-up finds the next one.)
fn next_lineup(from: f64, until: f64, step: f64, near: f64, off: &dyn Fn(f64) -> f64) -> Option<f64> {
    let (mut t, mut prev, mut closing) = (from, off(from), false);
    while t < until {
        let next = off(t + step);
        closing |= next < prev;
        if closing && prev < near && next >= prev {
            let (mut a, mut b) = ((t - step).max(from), t + step);
            for _ in 0..60 {
                let (x, y) = (a + (b - a) / 3.0, b - (b - a) / 3.0);
                if off(x) < off(y) { b = y } else { a = x }
            }
            return Some((a + b) / 2.0);
        }
        (t, prev) = (t + step, next);
    }
    None
}

/// What F10's sky rows look for (ADR 0202): Sol hidden by a body's moon seen from the body (`Solar`; total,
/// partial or annular by what the discs give), a moon in its body's shadow (`Lunar`), the same line-up seen from
/// the moon (`FromMoon`), and Luna at a phase (`Phase`, an index of `PHASES`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    Solar { body: usize, kind: Cover },
    Lunar { kind: Cover },
    FromMoon,
    Phase(usize),
}

/// How much of a disc is covered: wholly, in part, or a ring of it left (the near disc smaller).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Cover {
    Total,
    Partial,
    Annular,
}

/// Where and when an event is seen: the world time, the body stood on, the direction on it in its own frame, the
/// inertial point to look at, and how much of Sol (an eclipse) or of the moon (a phase: its lit share) shows it.
#[derive(Clone, Copy, Debug)]
pub struct Sight {
    pub t: f64,
    pub body: usize,
    pub dir: DVec3,
    pub look: DVec3,
    pub share: f64,
}

/// The years ahead F10 searches, in the home's years: an event the orbits do not give within it is not offered.
pub const YEARS: f64 = 3.0;

/// The first time from `from` on when the world's own orbits give `e`, and where on which body it is seen
/// (ADR 0202): nothing is posed, the poses are `World::pose` at the found time. None when they do not give it
/// within `YEARS`.
pub fn find(w: &engine::world::World, from: f64, e: Event) -> Option<Sight> {
    let sky = &w.sky;
    let until = from + YEARS * sky.year;
    let rad = |k: usize| w.bodies[k].radius;
    let at = |k: usize, t: f64| w.pose(k, t).centre;
    // the place on body k in direction x (inertial) at t, and its direction in the body's frame
    let place = |k: usize, t: f64, x: DVec3| {
        let d = w.pose(k, t).rot.inverse() * x.normalize();
        (at(k, t) + x.normalize() * rad(k), d)
    };
    // how far moon m's centre is off the line from Sol through body k, on Sol's side (`behind`: the far side)
    let off = |k: usize, m: usize, behind: bool, t: f64| {
        let p = at(k, t);
        let s = (sky.sun(t) - p).normalize();
        let c = at(m, t) - p;
        let along = c.dot(s);
        if (along > 0.0) != behind { (c - s * along).length() } else { f64::MAX }
    };
    // a step under a tenth of the moon's radius at its speed round its parent, so no line-up is jumped
    let step_of = |m: usize| (w.bodies[m].period * rad(m) / (std::f64::consts::TAU * w.bodies[m].orbit) * 0.1).clamp(1.0, 60.0);
    match e {
        Event::Solar { body: k, kind } => {
            let m = (0..w.bodies.len()).find(|&m| w.bodies[m].parent == k && m != k)?;
            // the penumbra's reach round the axis at the body: the moon's radius and the sun's angle over its orbit
            let reach = rad(k) + 2.0 * (rad(m) + w.bodies[m].orbit * sky.sun_radius / (sky.sun(from) - at(k, from)).length());
            let mut t = from;
            while let Some(found) = next_lineup(t, until, step_of(m), reach, &|t| off(k, m, false, t)) {
                t = found + 2.0 * step_of(m);
                let (s, c) = (sky.sun(found), at(m, found));
                // where the line from Sol through the moon's centre meets the body, if it does
                let p = at(k, found);
                let dir = (c - s).normalize();
                let (q, r) = (c - p, rad(k));
                let b = q.dot(dir);
                let disc = b * b - (q.length_squared() - r * r);
                let sight = if disc > 0.0 {
                    let x = q + dir * (-b - disc.sqrt());
                    let (spot, d) = place(k, found, x);
                    let cover = sky.covered(found, spot, c, rad(m));
                    // concentric discs: whole when the moon's is the larger, else a ring
                    let ring = (rad(m) / (c - spot).length()).asin() < (sky.sun_radius / (s - spot).length()).asin();
                    (if ring { Cover::Annular } else { Cover::Total }, Sight { t: found, body: k, dir: d, look: s, share: cover })
                } else {
                    // the axis misses: the most of Sol hidden anywhere Sol stands 5 degrees up
                    let best = sphere(4000).map(|u| place(k, found, u)).filter(|(spot, _)| altitude(*spot - p, s - *spot) > 5.0).map(|(spot, d)| (sky.covered(found, spot, c, rad(m)), d)).max_by(|a, b| a.0.total_cmp(&b.0));
                    match best {
                        Some((cover, d)) if cover > 0.05 => (Cover::Partial, Sight { t: found, body: k, dir: d, look: s, share: cover }),
                        _ => continue,
                    }
                };
                if sight.0 == kind {
                    return Some(sight.1);
                }
            }
            None
        }
        Event::Lunar { .. } | Event::FromMoon => {
            let m = w.moon;
            let mut t = from;
            while let Some(found) = next_lineup(t, until, step_of(m), rad(0) + rad(m), &|t| off(0, m, true, t)) {
                t = found + 2.0 * step_of(m);
                let (p, c) = (at(0, found), at(m, found));
                // Luna's centre and four points of her limb across the shadow: how many lie in the umbra
                let s = (sky.sun(found) - c).normalize();
                let (e1, e2) = (s.any_orthonormal_vector(), s.cross(s.any_orthonormal_vector()));
                let dark = [DVec3::ZERO, e1, -e1, e2, -e2].iter().filter(|v| sky.covered(found, c + **v * rad(m) * 0.98, p, rad(0)) >= 0.999).count();
                let got = match dark {
                    5 => Cover::Total,
                    0 => continue,
                    _ => Cover::Partial,
                };
                match e {
                    Event::FromMoon if got == Cover::Total => {
                        let (spot, d) = place(m, found, p - c);
                        return Some(Sight { t: found, body: m, dir: d, look: p, share: sky.covered(found, spot, p, rad(0)) });
                    }
                    Event::Lunar { kind } if got == kind => {
                        // seen from under her on Terra: her at the zenith, the night side
                        let (_, d) = place(0, found, c - p);
                        return Some(Sight { t: found, body: 0, dir: d, look: c, share: dark as f64 / 5.0 });
                    }
                    _ => {}
                }
            }
            None
        }
        Event::Phase(k) => {
            let want = k as f64 * 45.0;
            // the elongation's distance past `want`, -180 to 180: it crosses 0 upward once a month
            let past = |t: f64| (elongation(sky, t) - want + 540.0).rem_euclid(360.0) - 180.0;
            let step = 60.0;
            let mut t = from;
            while t < until {
                if past(t) < 0.0 && past(t + step) >= 0.0 {
                    let (mut a, mut b) = (t, t + step);
                    for _ in 0..50 {
                        let mid = (a + b) / 2.0;
                        if past(mid) < 0.0 { a = mid } else { b = mid }
                    }
                    let found = (a + b) / 2.0;
                    let (p, c, s) = (at(0, found), at(w.moon, found), sky.sun(found));
                    t += step;
                    // a phase is a look at her lit face: a month whose full or new is an eclipse is passed over
                    if sky.covered(found, c, p, rad(0)) > 0.0 || sky.covered(found, p, c, rad(w.moon)) > 0.0 {
                        continue;
                    }
                    // the darkest sky on Terra that has her 20 degrees up or more
                    let (_, d) = sphere(4000).map(|u| place(0, found, u)).filter(|(spot, _)| altitude(*spot - p, c - *spot) >= 20.0).min_by(|x, y| altitude(x.0 - p, s - x.0).total_cmp(&altitude(y.0 - p, s - y.0)))?;
                    // her lit share: (1 + cos of the angle at her between Sol and Terra) / 2
                    let lit = (1.0 + (s - c).normalize().dot((p - c).normalize())) / 2.0;
                    return Some(Sight { t: found, body: 0, dir: d, look: c, share: lit });
                }
                t += step;
            }
            None
        }
    }
}

/// How far over the ground (or the sea; a gas body's cloud tops, none) F10 stands a player: over the trees.
pub const OVER: f64 = 40.0;
/// The eye's height over the feet.
const EYE: f64 = 1.7;

/// Where F10 stands the player for sight `s`: `OVER` metres over the ground in its direction, and for an eclipse of
/// the sun on the shadow's axis at the eye's own height, not on the sphere of `radius` (the review: with Sol 45
/// degrees up that was some 70 m off the axis, outside a 50 m umbra). The feet (body frame), the eye (inertial)
/// and the share seen from the eye: of Sol hidden for an eclipse of the sun, else the sight's own.
pub fn stand(w: &engine::world::World, s: &Sight) -> (DVec3, DVec3, f64) {
    let b = &w.bodies[s.body];
    let pose = w.pose(s.body, s.t);
    let eye_r = |d: DVec3| {
        let r = (b.stand_point(d) - b.centre).length();
        if b.gas { r } else { r.max(b.top(d) + 0.6) + OVER }
    };
    let solar = s.look == w.sky.sun(s.t);
    let moon = (0..w.bodies.len()).find(|&m| w.bodies[m].parent == s.body && m != s.body).filter(|_| solar);
    // where the line from Sol through the moon's centre meets the sphere of radius r, in the body's frame
    let axis = |m: usize, r: f64| {
        let c = w.pose(m, s.t).centre;
        let (dir, q) = ((c - w.sky.sun(s.t)).normalize(), c - pose.centre);
        let k = q.dot(dir);
        let disc = k * k - (q.length_squared() - r * r);
        (disc > 0.0).then(|| (pose.rot.inverse() * (q + dir * (-k - disc.sqrt()))).normalize())
    };
    let mut d = s.dir;
    // a total or a ring is found on the axis (`find`); the ground under the axis at height is not the ground
    // under the old spot, so a few rounds settle it
    if let Some(m) = moon.filter(|&m| axis(m, b.radius).is_some()) {
        for _ in 0..3 {
            d = axis(m, eye_r(d) + EYE).unwrap_or(d);
        }
    }
    let feet = b.centre + d * eye_r(d);
    let eye = pose.to_inertial(b.centre, feet + d * EYE);
    let share = moon.map_or(s.share, |m| w.sky.covered(s.t, eye, w.pose(m, s.t).centre, w.bodies[m].radius));
    (feet, eye, share)
}

/// How many degrees `to` stands over the horizon of a place whose up is `up`.
pub fn altitude(up: DVec3, to: DVec3) -> f64 {
    up.normalize().dot(to.normalize()).clamp(-1.0, 1.0).asin().to_degrees()
}

/// n directions spread evenly over the sphere (a Fibonacci lattice).
fn sphere(n: usize) -> impl Iterator<Item = DVec3> {
    let golden = std::f64::consts::PI * (3.0 - 5f64.sqrt());
    (0..n).map(move |k| {
        let z = 1.0 - (k as f64 + 0.5) / n as f64 * 2.0;
        let r = (1.0 - z * z).sqrt();
        let a = golden * k as f64;
        DVec3::new(r * a.cos(), r * a.sin(), z)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nav::{phase_name, PHASES};

    /// A new world's orbits (generation `GENERATION`, real gravity) on the 4 km planet, without its maps: the poses
    /// are the same, the ground is not asked.
    fn world() -> engine::world::World {
        engine::world::World::with_grid(6144, engine::bodies::moon_columns(6144), 1, None, None, 0.0, engine::world::GENERATION)
    }

    #[test]
    fn every_eclipse_found_is_one_by_the_orbits_own_geometry() {
        let w = world();
        let sky = &w.sky;
        let started = std::time::Instant::now();
        let from = 5000.0;
        let spot = |s: &Sight| w.pose(s.body, s.t).to_inertial(w.bodies[s.body].centre, w.bodies[s.body].centre + s.dir * w.bodies[s.body].radius);
        let moon = |t: f64| w.pose(w.moon, t).centre;
        let (r, m) = (w.bodies[0].radius, &w.bodies[w.moon]);
        // seen on Terra where the line from Sol through Luna meets the ground: wholly hidden, and not ten minutes before
        let total = find(&w, from, Event::Solar { body: 0, kind: Cover::Total }).expect("a total eclipse of the sun");
        assert!(total.t > from && sky.covered(total.t, spot(&total), moon(total.t), m.radius) >= 0.999);
        assert!(sky.covered(total.t - 600.0, spot(&total), moon(total.t - 600.0), m.radius) < 0.5);
        // asked again just past it, the answer is the next one, not this one
        let next = find(&w, total.t + 90.0, Event::Solar { body: 0, kind: Cover::Total }).expect("another");
        assert!(next.t > total.t + 2000.0, "{} after {}", next.t, total.t);
        // partial: some of Sol hidden at the place, not all
        let part = find(&w, from, Event::Solar { body: 0, kind: Cover::Partial }).expect("a partial eclipse of the sun");
        let c = sky.covered(part.t, spot(&part), moon(part.t), m.radius);
        assert!(c > 0.05 && c < 0.999 && (c - part.share).abs() < 1e-9, "{c}");
        // no ring of Sol round Luna from anywhere on Terra: from the farthest ground that sees her she still looks
        // larger than Sol does from the nearest (ADR 0041's sizes), so the orbits never give an annular eclipse
        let smallest_luna = (m.radius / (m.orbit * m.orbit - r * r).sqrt()).asin();
        let largest_sol = (sky.sun_radius / (sky.sun_dist - r)).asin();
        assert!(smallest_luna > largest_sol, "{smallest_luna} {largest_sol}");
        // Luna in Terra's shadow: wholly, at her centre and four points of her limb; in part, some of them
        // (the review) and from where F10 stands the eye, 40 m over the ground: still wholly hidden, Sol well up
        let (_, eye, share) = stand(&w, &total);
        let up = eye - w.pose(0, total.t).centre;
        assert!(sky.covered(total.t, eye, moon(total.t), m.radius) >= 0.999 && share >= 0.999, "{share} from the eye");
        assert!(altitude(up, total.look - eye) > 5.0);
        // on the shadow's axis itself, not beside it (the umbra here is some 50 m across)
        let axis = (moon(total.t) - total.look).normalize();
        let off = (eye - moon(total.t)).reject_from_normalized(axis).length();
        assert!(off < 1.0, "the eye {off:.1} m off the axis");
        let lunar = find(&w, from, Event::Lunar { kind: Cover::Total }).expect("a total eclipse of Luna");
        assert!(sky.covered(lunar.t, moon(lunar.t), w.pose(0, lunar.t).centre, r) >= 0.999 && lunar.share == 1.0);
        let lpart = find(&w, from, Event::Lunar { kind: Cover::Partial }).expect("a partial eclipse of Luna");
        assert!(lpart.share > 0.0 && lpart.share < 1.0);
        // seen from Luna's face to Terra in a total one: Terra hides all of Sol
        let fl = find(&w, from, Event::FromMoon).expect("Terra over Sol from Luna");
        assert_eq!(fl.body, w.moon);
        assert!(sky.covered(fl.t, spot(&fl), w.pose(0, fl.t).centre, r) >= 0.999);
        // the giant's moon over Sol from the giant's cloud tops: whole, or a ring if the moon looks the smaller
        let giant = (0..w.bodies.len()).find(|&k| w.bodies[k].gas).expect("a gas giant");
        let gm = (0..w.bodies.len()).find(|&k| w.bodies[k].parent == giant).expect("the giant's moon");
        let (kind, g) = [Cover::Total, Cover::Annular].into_iter().find_map(|kind| find(&w, from, Event::Solar { body: giant, kind }).map(|s| (kind, s))).expect("an eclipse at the giant by its moon");
        let cover = sky.covered(g.t, spot(&g), w.pose(gm, g.t).centre, w.bodies[gm].radius);
        assert!(cover > 0.3 && (cover - g.share).abs() < 1e-9, "{kind:?} {cover}");
        eprintln!("searched in {:.0} ms: the giant's is {kind:?}, {cover:.3} of Sol hidden; times {} {} {} {} {} {}", started.elapsed().as_secs_f64() * 1000.0, total.t, part.t, lunar.t, lpart.t, fl.t, g.t);
    }

    #[test]
    fn every_phase_found_has_its_elongation_and_luna_up() {
        let w = world();
        for (k, name) in PHASES.iter().enumerate() {
            let s = find(&w, 5000.0, Event::Phase(k)).unwrap_or_else(|| panic!("no {name}"));
            assert_eq!(phase_name(elongation(&w.sky, s.t)), *name);
            // (the review) by the poses, not by `elongation`: the angle at Terra between Sol and her is the phase's,
            // within her orbit's lean, on the side she waxes on; and her lit share is the one that angle gives
            let p = w.pose(0, s.t);
            let (to_sol, to_her) = ((w.sky.sun(s.t) - p.centre).normalize(), (w.pose(w.moon, s.t).centre - p.centre).normalize());
            let apart = to_sol.dot(to_her).clamp(-1.0, 1.0).acos().to_degrees();
            let want = (k as f64 * 45.0).min(360.0 - k as f64 * 45.0);
            assert!((apart - want).abs() < 2.5, "{name}: {apart} degrees apart");
            if k % 4 != 0 {
                assert_eq!(to_sol.cross(to_her).z > 0.0, k < 4, "{name}: on the side she waxes on");
            }
            let lit = (1.0 - want.to_radians().cos()) / 2.0;
            assert!((s.share - lit).abs() < 0.03, "{name}: {} lit, {lit} by the angle", s.share);
            // the place has her 20 degrees up or more
            let at = p.centre + p.rot * s.dir * w.bodies[0].radius;
            assert!(altitude(p.rot * s.dir, s.look - at) >= 19.9, "{name}");
            // (a shot, 2026-11-24: the first full Luna found was the total eclipse's) never one in an eclipse
            let (t, c) = (s.t, w.pose(w.moon, s.t).centre);
            assert!(w.sky.covered(t, c, p.centre, w.bodies[0].radius) == 0.0 && w.sky.covered(t, p.centre, c, w.bodies[w.moon].radius) == 0.0, "{name} at {t}");
        }
    }
}

