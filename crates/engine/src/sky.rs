//! The sky's mechanics (ADR 0022): the planet's spin, the moon's orbit and the sun, as pure functions of the
//! world's time in seconds, the same on every machine. Orbits are circles on rails.
//!
//! The inertial frame is centred on the sun. The ecliptic is its xy plane: the planet circles the sun in it
//! once a year, and spins about its +z, which is the planet's body z; the moon circles the planet. A body's pose maps its own frame (its grid at its nominal centre, as
//! `world::Body` has it) to the inertial frame.
use glam::{DQuat, DVec3};
use std::f64::consts::TAU;

/// The solar day (ADR 0008): 30 minutes (ADR 0041; was 20).
pub const DAY: f64 = 1800.0;
/// From new moon to new moon.
pub const MONTH: f64 = 16.0 * DAY;
/// Once around the sun: the planet's orbit, which carries the sun along the ecliptic. Not a whole number of months, so the new
/// moons meet the eclipse seasons at different places each year.
pub const YEAR: f64 = 54.0 * DAY;
/// The moon's orbit against the ecliptic: 2 degrees, so about one total eclipse of the sun and one blood moon a
/// year come somewhere (the developer, ADR 0041; 40 degrees made them years apart once Luna moved out to 60 radii).
pub const INCLINATION: f64 = 2.0 * TAU / 360.0;
/// The sky repeats after this: 432 days are 27 months and 8 years, and the spin's sidereal turns are whole too.
/// The poses take the time modulo it, so a world billions of seconds old keeps its angles to the nanoradian.
pub const PERIOD: f64 = 432.0 * DAY;
/// How much smaller the sun looks than the moon from the ground right under the moon: 5 %, as the Moon on a total
/// eclipse is 3 to 6 % bigger than the Sun, so totality lasts about 2 s on the 4 km Terra (ADR 0041; 0.995 gave 0.2 s).
pub const SUN_UNDER_MOON: f64 = 0.95;
/// Where the planet's spin starts: the spawn (near body +y) has the sun 45 degrees up in the morning.
const SPIN0: f64 = -0.375 * TAU;
/// Where the moon starts on its orbit, from its ascending node: a first quarter in the evening sky.
const MOON0: f64 = 0.25 * TAU;

/// The clock at the spawn (ADR 0023): world time 0 is 9 am there and a day is DAY seconds. The day (the
/// first is 1) and the hour, 0 to 24.
pub fn clock(t: f64) -> (u64, f64) {
    let h = 9.0 + 24.0 * t / DAY;
    ((h / 24.0).floor().max(0.0) as u64 + 1, h.rem_euclid(24.0))
}

/// The first world time from `t` on when the spawn's clock shows `hour`.
pub fn next_hour(t: f64, hour: f64) -> f64 {
    t + (hour - clock(t).1).rem_euclid(24.0) / 24.0 * DAY
}

/// The sky of one world: the sizes it needs from the world's bodies.
#[derive(Clone, Copy, Debug)]
pub struct Sky {
    pub planet_radius: f64,
    pub moon_radius: f64,
    /// The moon's orbit radius: its nominal centre's distance from the planet's (`world::World::new`).
    pub moon_orbit: f64,
    /// The planet's orbit around the sun.
    pub sun_dist: f64,
    /// The sun's radius: from the moon's size, so that a total eclipse is a perfect one (`Sky::new`).
    pub sun_radius: f64,
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
    /// The sky of a planet and its moon, the moon `orbit` and the sun `sun` planet radii away, centre to centre.
    /// The sun's size follows (the developer's rule, ADR 0041): seen from the ground right under the moon at a
    /// new moon, the sun is SUN_UNDER_MOON of the moon's size, so every total eclipse just covers it.
    pub fn new(planet_radius: f64, moon_radius: f64, orbit: f64, sun: f64) -> Sky {
        let (moon_orbit, sun_dist) = (planet_radius * orbit, planet_radius * sun);
        assert!(moon_radius < moon_orbit - planet_radius, "the moon touches or holds the planet's ground");
        let moon_seen = (moon_radius / (moon_orbit - planet_radius)).asin();
        let sun_radius = (sun_dist - planet_radius) * (SUN_UNDER_MOON * moon_seen).sin();
        Sky { planet_radius, moon_radius, moon_orbit, sun_dist, sun_radius }
    }
    /// The sun's centre: the inertial origin.
    pub fn sun(&self, _t: f64) -> DVec3 {
        DVec3::ZERO
    }
    /// The planet: on its circle around the sun once a year, spinning once a sidereal day.
    pub fn planet(&self, t: f64) -> Pose {
        let t = t.rem_euclid(PERIOD);
        let a = TAU * t / YEAR;
        let spin = SPIN0 + TAU * t * (1.0 / DAY + 1.0 / YEAR);
        Pose { centre: -DVec3::new(a.cos(), a.sin(), 0.0) * self.sun_dist, rot: DQuat::from_rotation_z(spin) }
    }
    /// The moon: on its inclined circle once a sidereal month, the same face (its body -x) to the planet.
    pub fn moon(&self, t: f64) -> Pose {
        let t = t.rem_euclid(PERIOD);
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
        let (rs, rm) = ((self.sun_radius / s.length()).asin(), (r / m.length()).min(1.0).asin());
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
        // the standard planet (ADR 0021) and the moon of `World::new`, by data/bodies.toml's rule
        Sky::new(4244.0, 4244.0 / 3.7, 60.0, 23500.0)
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
    fn a_day_is_thirty_minutes_and_starts_in_the_morning() {
        // the developer, 2026-11-11 (ADR 0041): 30 minutes, was 20
        assert_eq!(DAY, 1800.0);
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
            let reach = s.planet_radius + s.moon_radius + mn.length() * (s.sun_radius / s.sun_dist);
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

    /// Bug 2026-11-11_13-17-26 (ADR 0037): a blood moon, the moon in the planet's shadow, is an event: over
    /// the sky's whole period (8 years) it comes about once a year (ADR 0041: Luna's tilt is 2 degrees), each
    /// a few minutes long, as the Moon's are a few hours of a month.
    #[test]
    fn a_blood_moon_is_rare() {
        let s = sky();
        let (mut dark_s, mut runs, mut was) = (0.0, 0, false);
        let mut t = 0.0;
        while t < PERIOD {
            let dark = s.covered(t, s.moon(t).centre, s.planet(t).centre, s.planet_radius) > 0.5;
            (dark_s, runs, was) = (dark_s + 10.0 * dark as i32 as f64, runs + (dark && !was) as i32, dark);
            t += 10.0;
        }
        let years = PERIOD / YEAR;
        eprintln!("{years} years: {runs} blood moons, {dark_s} s in all");
        assert!(runs as f64 >= years / 2.0 && runs as f64 <= years * 3.0, "{runs} in {years} years");
        let each = dark_s / runs as f64;
        assert!(each > 30.0 && each < DAY / 4.0, "{dark_s} s in {runs}");
    }

    #[test]
    fn the_spawn_clock_has_the_sun_overhead_at_noon() {
        assert_eq!(clock(0.0), (1, 9.0));
        assert_eq!(clock(DAY), (2, 9.0));
        let noon = next_hour(0.0, 12.0);
        assert!((noon - DAY / 8.0).abs() < 1e-9 && next_hour(noon, 12.0) == noon);
        assert!((clock(next_hour(noon, 6.5)).1 - 6.5).abs() < 1e-9 && next_hour(noon, 6.5) > noon);
        // the spawn is near the planet's body +y, on the equator: the sun straight up at noon, down at midnight
        let s = sky();
        let height = |t: f64| {
            let p = s.planet(t);
            (p.rot * DVec3::Y).dot((s.sun(t) - p.centre).normalize())
        };
        assert!(height(noon) > 0.99, "{}", height(noon));
        assert!(height(next_hour(0.0, 0.0)) < -0.99);
        assert!(height(next_hour(0.0, 6.0)).abs() < 0.02 && height(next_hour(0.0, 18.0)).abs() < 0.02);
    }

    #[test]
    fn eclipse_shares() {
        let s = sky();
        let t = 0.0;
        // straight behind the moon, as seen from the sun: total; far to the side: none
        let (sn, mn) = (sun(&s, t), moon(&s, t));
        // behind, as far as the ground: the sun just fits behind the moon (ADR 0041), so totality is a few
        // metres wide there, and the partial shadow about the moon's diameter
        let behind = mn + (mn - sn).normalize() * (s.moon_orbit - s.planet_radius);
        assert_eq!(s.eclipse(t, behind + s.planet(t).centre), 1.0);
        assert_eq!(s.eclipse(t, mn + DVec3::Z * 5000.0 - sn.normalize() * 1000.0 + s.planet(t).centre), 0.0);
        // a little off the centre line: partial, and less the farther off
        let side = (mn - sn).normalize().any_orthonormal_vector();
        let at = |x: f64| s.eclipse(t, behind + side * x * s.moon_radius + s.planet(t).centre);
        let (a, b) = (at(0.3), at(1.3));
        assert!(a > b && b > 0.0 && a < 1.0 && at(2.2) == 0.0, "{a} {b}");
    }

    #[test]
    fn the_sky_repeats_and_an_old_world_keeps_its_precision() {
        // the developer's world was 2.4e11 s old (bug 2026-11-10_19-46-22): the same sky as at 1234.5 s, exactly
        let s = sky();
        let (t, old) = (1234.5, 1234.5 + 462_000.0 * PERIOD);
        for (a, b) in [(s.planet(t), s.planet(old)), (s.moon(t), s.moon(old))] {
            assert!(a.centre.distance(b.centre) < 1e-6 && (a.rot - b.rot).length() < 1e-12, "{a:?} {b:?}");
        }
        // and the period is the sky's own: without the wrap it agrees to the float's grain
        let p = |t: f64| (TAU * t / YEAR, TAU * t * (1.0 / DAY + 1.0 / YEAR), TAU * t * (1.0 / MONTH + 1.0 / YEAR));
        for x in [p(PERIOD).0, p(PERIOD).1, p(PERIOD).2] {
            assert!((x / TAU - (x / TAU).round()).abs() < 1e-9, "{}", x / TAU);
        }
    }

    #[test]
    fn the_planet_circles_the_sun_once_a_year() {
        let s = sky();
        for t in [0.0, 777.0, YEAR / 3.0] {
            assert!((s.planet(t).centre.length() - s.sun_dist).abs() < 1e-6);
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
