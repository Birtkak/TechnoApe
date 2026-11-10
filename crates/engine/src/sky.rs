//! The sky's mechanics (ADR 0022): the planet's spin, the moon's orbit and the sun, as pure functions of the
//! world's time in seconds, the same on every machine. Orbits are circles on rails.
//!
//! The inertial frame is centred on the sun. The ecliptic is its xy plane: the planet circles the sun in it
//! once a year, and spins about its +z, which is the planet's body z; the moon circles the planet. A body's pose maps its own frame (its grid at its nominal centre, as
//! `world::Body` has it) to the inertial frame.
use glam::{DQuat, DVec3};
use std::f64::consts::TAU;

/// The solar day (ADR 0008).
pub const DAY: f64 = 1200.0;
/// From new moon to new moon.
pub const MONTH: f64 = 8.0 * DAY;
/// Once around the sun: the planet's orbit, which carries the sun along the ecliptic. Not a whole number of months, so the new
/// moons meet the eclipse seasons at different places each year.
pub const YEAR: f64 = 27.0 * DAY;
/// The moon's orbit against the ecliptic: steep, so only about a third of new moons eclipse the sun.
pub const INCLINATION: f64 = 40.0 * TAU / 360.0;
/// The planet's orbit around the sun, and the sun's size: 0.53 degrees across, as Earth's.
pub const SUN_DIST: f64 = 1.0e6;
pub const SUN_RADIUS: f64 = 4625.0;
/// Where the planet's spin starts: the spawn (near body +y) has the sun 45 degrees up in the morning.
const SPIN0: f64 = -0.375 * TAU;
/// Where the moon starts on its orbit, from its ascending node: a first quarter in the evening sky.
const MOON0: f64 = 0.25 * TAU;

/// The sky of one world: the sizes it needs from the world's bodies.
#[derive(Clone, Copy, Debug)]
pub struct Sky {
    pub planet_radius: f64,
    pub moon_radius: f64,
    /// The moon's orbit radius: its nominal centre's distance from the planet's (`world::World::new`).
    pub moon_orbit: f64,
}

/// Where a body is and how it is turned at a moment: its frame to the inertial frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub centre: DVec3,
    pub rot: DQuat,
}

impl Pose {
    /// A point in the body's frame (relative to its nominal centre `nominal`) in the inertial frame.
    pub fn to_inertial(&self, nominal: DVec3, p: DVec3) -> DVec3 {
        self.centre + self.rot * (p - nominal)
    }
    pub fn from_inertial(&self, nominal: DVec3, p: DVec3) -> DVec3 {
        nominal + self.rot.inverse() * (p - self.centre)
    }
}

impl Sky {
    /// The sun's centre: the inertial origin.
    pub fn sun(&self, _t: f64) -> DVec3 {
        DVec3::ZERO
    }
    /// The planet: on its circle around the sun once a year, spinning once a sidereal day.
    pub fn planet(&self, t: f64) -> Pose {
        let a = TAU * t / YEAR;
        let spin = SPIN0 + TAU * t * (1.0 / DAY + 1.0 / YEAR);
        Pose { centre: -DVec3::new(a.cos(), a.sin(), 0.0) * SUN_DIST, rot: DQuat::from_rotation_z(spin) }
    }
    /// The moon: on its inclined circle once a sidereal month, the same face (its body -x) to the planet.
    pub fn moon(&self, t: f64) -> Pose {
        let u = MOON0 + TAU * t * (1.0 / MONTH + 1.0 / YEAR);
        let (s, c, si, ci) = (u.sin(), u.cos(), INCLINATION.sin(), INCLINATION.cos());
        let e1 = DVec3::new(c, s * ci, s * si);
        let e2 = DVec3::new(-s, c * ci, c * si);
        let rot = DQuat::from_mat3(&glam::DMat3::from_cols(e1, e2, e1.cross(e2)));
        Pose { centre: self.planet(t).centre + e1 * self.moon_orbit, rot }
    }
    /// The share of the sun's disc the moon hides, seen from inertial point p: 0 none, 1 total.
    pub fn eclipse(&self, t: f64, p: DVec3) -> f64 {
        self.covered(t, p, self.moon(t).centre, self.moon_radius)
    }
    /// The share of the sun's disc a sphere at `c` of radius `r` hides, seen from inertial point p: the moon
    /// over the planet (`eclipse`), or the planet over the moon (a lunar eclipse, which dims the moonlight).
    pub fn covered(&self, t: f64, p: DVec3, c: DVec3, r: f64) -> f64 {
        let (s, m) = (self.sun(t) - p, c - p);
        let (rs, rm) = ((SUN_RADIUS / s.length()).asin(), (r / m.length()).min(1.0).asin());
        let d = s.cross(m).length().atan2(s.dot(m));
        if d >= rs + rm {
            return 0.0;
        }
        if d <= rm - rs {
            return 1.0;
        }
        if d <= rs - rm {
            return (rm / rs).powi(2);
        }
        // two discs in the small-angle plane: the lens they share
        let lens = |a: f64, b: f64| a * a * ((d * d + a * a - b * b) / (2.0 * d * a)).clamp(-1.0, 1.0).acos();
        let k = ((-d + rs + rm) * (d + rs - rm) * (d - rs + rm) * (d + rs + rm)).max(0.0).sqrt() * 0.5;
        ((lens(rs, rm) + lens(rm, rs) - k) / (std::f64::consts::PI * rs * rs)).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sky() -> Sky {
        // the standard planet (ADR 0021) and the moon of `World::new`
        Sky { planet_radius: 4244.0, moon_radius: 177.0, moon_orbit: 4244.0 * 2.83 }
    }

    /// The sun and the moon from the planet's centre.
    fn sun(s: &Sky, t: f64) -> DVec3 {
        s.sun(t) - s.planet(t).centre
    }
    fn moon(s: &Sky, t: f64) -> DVec3 {
        s.moon(t).centre - s.planet(t).centre
    }

    /// The sun's height over a point fixed on the planet (body frame direction `up`).
    fn elevation(s: &Sky, t: f64, up: DVec3) -> f64 {
        let u = s.planet(t).rot * up;
        (u.dot(sun(&s, t).normalize())).asin().to_degrees()
    }

    #[test]
    fn a_day_is_twenty_minutes_and_starts_in_the_morning() {
        let s = sky();
        let e0 = elevation(&s, 0.0, DVec3::Y);
        assert!((e0 - 45.0).abs() < 0.5, "{e0}");
        assert!(elevation(&s, 1.0, DVec3::Y) > e0, "the sun rises");
        assert!((elevation(&s, DAY, DVec3::Y) - e0).abs() < 0.01);
        assert!(elevation(&s, DAY / 2.0, DVec3::Y) < -40.0, "night half a day later");
    }

    #[test]
    fn the_moon_keeps_its_face_and_its_orbit() {
        let s = sky();
        for t in [0.0, 1234.0, 5000.0] {
            let (m, c) = (s.moon(t), moon(&s, t));
            // its body -x looks at the planet
            assert!((m.rot * -DVec3::X).dot(-c.normalize()) > 0.999_999);
            assert!((c.length() - s.moon_orbit).abs() < 1e-6);
        }
    }

    #[test]
    fn eclipses_come_sometimes_and_some_are_total() {
        let s = sky();
        // over 10 years (30 new moons), each new moon: does the moon's penumbra touch the planet?
        let (mut solar, mut lunar, mut total, mut moons) = (0, 0, 0, 0);
        let mut t = 0.0;
        while t < 10.0 * YEAR {
            // the moment of new moon: the least sun-moon angle within this month
            let best = |from: f64, sign: f64| {
                let mut b = (f64::MAX, from);
                let mut x = from;
                while x < from + MONTH {
                    let a = (sun(&s, x).normalize() * sign).angle_between(moon(&s, x).normalize());
                    if a < b.0 {
                        b = (a, x);
                    }
                    x += 20.0;
                }
                b
            };
            let (_, tn) = best(t, 1.0);
            let (sn, mn) = (sun(&s, tn), moon(&s, tn));
            // the moon's distance from the sun-planet line, against the penumbra's reach
            let off = (mn - sn.normalize() * mn.dot(sn.normalize())).length();
            let reach = s.planet_radius + s.moon_radius + mn.length() * (SUN_RADIUS / SUN_DIST);
            if off < reach {
                solar += 1;
                // the centre of the shadow on the ground, if it falls on the planet: total there?
                let d = (mn - sn).normalize();
                let b = mn.dot(d);
                let disc = b * b - (mn.length_squared() - s.planet_radius.powi(2));
                if disc > 0.0 {
                    let p = mn + d * (-b - disc.sqrt());
                    if s.eclipse(tn, p + s.planet(tn).centre) > 0.999 {
                        total += 1;
                    }
                }
            }
            let (_, tf) = best(t, -1.0);
            let (sf, mf) = (sun(&s, tf), moon(&s, tf));
            if (mf - sf.normalize() * mf.dot(sf.normalize())).length() < s.planet_radius + s.moon_radius {
                lunar += 1;
            }
            moons += 1;
            t += MONTH;
        }
        eprintln!("{moons} new moons: {solar} solar eclipses ({total} total somewhere), {lunar} lunar");
        assert!((moons / 5..=moons * 3 / 5).contains(&solar), "{solar} of {moons}");
        assert!((moons / 5..=moons * 3 / 5).contains(&lunar), "{lunar} of {moons}");
        assert!(total > 0, "no total eclipse in 10 years");
    }

    #[test]
    fn eclipse_shares() {
        let s = sky();
        let t = 0.0;
        // straight behind the moon, as seen from the sun: total; far to the side: none
        let (sn, mn) = (sun(&s, t), moon(&s, t));
        // 4 km behind, as far as the ground: totality to 158 m off the centre line, partial to 196 m
        let behind = mn + (mn - sn).normalize() * 4000.0;
        assert_eq!(s.eclipse(t, behind + s.planet(t).centre), 1.0);
        assert_eq!(s.eclipse(t, mn + DVec3::Z * 5000.0 - sn.normalize() * 1000.0 + s.planet(t).centre), 0.0);
        // a little off the centre line: partial, and less the farther off
        let side = (mn - sn).normalize().any_orthonormal_vector();
        let a = s.eclipse(t, behind + side * 170.0 + s.planet(t).centre);
        let b = s.eclipse(t, behind + side * 185.0 + s.planet(t).centre);
        assert!(a > b && b > 0.0 && a < 1.0, "{a} {b}");
    }

    #[test]
    fn the_planet_circles_the_sun_once_a_year() {
        let s = Sky { planet_radius: 4244.0, moon_radius: 177.0, moon_orbit: 4244.0 * 2.83 };
        for t in [0.0, 777.0, YEAR / 3.0] {
            assert!((s.planet(t).centre.length() - SUN_DIST).abs() < 1e-6);
        }
        assert!(s.planet(YEAR).centre.distance(s.planet(0.0).centre) < 1e-6);
        assert!(s.planet(YEAR / 2.0).centre.distance(-s.planet(0.0).centre) < 1e-6);
    }

    #[test]
    fn the_planet_hides_the_sun_from_the_moon_behind_it() {
        let s = sky();
        let t = 300.0;
        let (sun, p) = (s.sun(t), s.planet(t).centre);
        let away = (p - sun).normalize();
        assert_eq!(s.covered(t, p + away * s.moon_orbit, p, s.planet_radius), 1.0);
        let side = away.cross(DVec3::Z).normalize();
        assert_eq!(s.covered(t, p + side * s.moon_orbit, p, s.planet_radius), 0.0);
    }
}
