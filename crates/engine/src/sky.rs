//! The sky's mechanics (ADR 0022): the planet's spin, the moon's orbit and the sun, as pure functions of the
//! world's time in seconds, the same on every machine. Orbits are circles on rails.
//!
//! The inertial frame is centred on the sun. The ecliptic is its xy plane: the planet circles the sun in it
//! once a year, and spins about its body +z, which leans TILT from the ecliptic's +z toward the sun's place at
//! a quarter year (ADR 0055, seasons): world time 0 is the northern spring equinox. The moon circles the planet. A body's pose maps its own frame (its grid at its nominal centre, as
//! `world::Body` has it) to the inertial frame.
use glam::{DMat3, DQuat, DVec3};
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
/// The lean of the planet's spin axis from the ecliptic's pole: 25 degrees (the developer tried it over the design's 20 for stronger seasons, ADR 0060; worldgen.md;
/// an open taste call, ADR 0055). 0 is the sky before the seasons.
pub const TILT: f64 = 25.0 * TAU / 360.0;
/// How much warmer than its yearly mean a place is per unit of sin(latitude) sin(declination): 40 K makes the
/// 45-degree latitudes swing some 10 K either way of the mean over the year at TILT (ADR 0055, open to taste).
pub const SEASON_K: f64 = 40.0;
/// The day's warmest hour after local noon (the ground keeps warming while the sun still gives more than it
/// loses): 2 hours.
const WARMEST: f64 = 2.0 / 24.0 * TAU;
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
    /// The spin axis's lean, radians (`TILT`).
    pub tilt: f64,
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
        Sky { planet_radius, moon_radius, moon_orbit, sun_dist, sun_radius, tilt: TILT }
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
        // the axis leans toward +y, where the sun stands from the planet a quarter year on: the northern summer
        let rot = DQuat::from_rotation_z(spin);
        Pose { centre: -DVec3::new(a.cos(), a.sin(), 0.0) * self.sun_dist, rot: if self.tilt == 0.0 { rot } else { DQuat::from_rotation_x(-self.tilt) * rot } }
    }
    /// The direction to the sun in the planet's body frame (its +z the spin axis, north).
    pub fn sun_dir(&self, t: f64) -> DVec3 {
        let p = self.planet(t);
        p.rot.inverse() * (self.sun(t) - p.centre).normalize()
    }
    /// The sun's declination: its angle north of the planet's equator, radians; +tilt at the northern summer
    /// solstice (a quarter year after world time 0), -tilt at the winter one.
    pub fn declination(&self, t: f64) -> f64 {
        self.sun_dir(t).z.clamp(-1.0, 1.0).asin()
    }
    /// The sun's hour angle at the place `up` (body frame): 0 at its local noon, growing a turn a day, a
    /// quarter turn (PI / 2) at 18:00 local; -PI..PI.
    pub fn hour_angle(&self, t: f64, up: DVec3) -> f64 {
        let s = self.sun_dir(t);
        (s.x * up.y - s.y * up.x).atan2(s.x * up.x + s.y * up.y)
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

/// Luna's pull on the sea, metres: the bulge under Luna stands 1.5 * TIDE_LUNA over the ring 90 degrees off it.
/// With the sun's a third of it (docs/design/rivers-and-tides.md), low to high at springs is 1.5 * (TIDE_LUNA +
/// TIDE_SUN) = 1 m (the developer, 2026-11-12), at neaps half that.
pub const TIDE_LUNA: f64 = 0.5;
pub const TIDE_SUN: f64 = TIDE_LUNA / 3.0;
/// The highest the tide stands over the mean sea anywhere, metres: both bulges at once (2/3 m with the ocean
/// all around), and the most a lopsided ocean's `Tide::base` adds. What the sea's quads are meshed for
/// (`world::sea_quads`), so they are never meshed again for the tide.
pub const TIDE_HIGH: f64 = 1.0;

/// The sea's tide at one moment (ADR 0067): Luna's and the sun's directions in the planet's body frame and the
/// level that keeps the ocean's volume. `at` is a dozen multiplications, so the physics, the water and the
/// shader (`vs_sea`, its port) all ask it per place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tide {
    pub luna: DVec3,
    pub sun: DVec3,
    /// Metres everywhere: minus the bulges' mean over the ocean, so the sea moves water and makes none.
    pub base: f64,
}

impl Tide {
    /// No tide: the sea at its mean everywhere.
    pub const NONE: Tide = Tide { luna: DVec3::ZERO, sun: DVec3::ZERO, base: 0.0 };
    /// The sea's height over its mean, metres, at body-frame direction `up` (a unit vector): the classic bulge
    /// (3 cos^2 a - 1) / 2 under and opposite Luna and the sun; its -1/2 is in `base`, with the ocean's.
    pub fn at(&self, up: DVec3) -> f64 {
        let (l, s) = (up.dot(self.luna), up.dot(self.sun));
        1.5 * (TIDE_LUNA * l * l + TIDE_SUN * s * s) + self.base
    }
}

impl Sky {
    /// The direction to the moon in the planet's body frame.
    pub fn moon_dir(&self, t: f64) -> DVec3 {
        let p = self.planet(t);
        p.rot.inverse() * (self.moon(t).centre - p.centre).normalize()
    }
    /// The tide at world time t over an ocean whose area's second moment is `ocean` (the mean of d d^T over
    /// the sea's directions d, trace 1; a third of the identity for a sea all around): its mean over the ocean is
    /// then exactly 0, so the rise under Luna is water from the low ring, not water made.
    pub fn tide(&self, t: f64, ocean: &DMat3) -> Tide {
        let (luna, sun) = (self.moon_dir(t), self.sun_dir(t));
        let base = -1.5 * (TIDE_LUNA * luna.dot(*ocean * luna) + TIDE_SUN * sun.dot(*ocean * sun));
        Tide { luna, sun, base }
    }
}

/// How long the sun's centre stays up in a day at latitude `lat` when its declination is `decl` (radians):
/// seconds, 0 to DAY; DAY / 2 on the equator or at an equinox.
pub fn day_length(lat: f64, decl: f64) -> f64 {
    (-lat.tan() * decl.tan()).clamp(-1.0, 1.0).acos() / std::f64::consts::PI * DAY
}

/// The season names `/time set` takes: 0 the northern spring equinox, 1 its summer solstice, 2 autumn, 3 winter.
pub fn season(name: &str) -> Option<u32> {
    ["spring", "summer", "autumn", "winter"].iter().position(|&n| n == name).map(|q| q as u32)
}

/// The first world time from `t` on, whole days later so the clock shows the same hour, nearest the start of
/// season `q` (`season`): within half a day of it, as a year is not a whole number of quarter days.
pub fn next_season(t: f64, q: u32) -> f64 {
    let ahead = (q as f64 / 4.0 * YEAR - t).rem_euclid(YEAR);
    let days = (ahead / DAY).round();
    // a round up to a whole year: the season's start is under half a day behind, so stay
    t + if days * DAY >= YEAR { 0.0 } else { days * DAY }
}

/// The season a block's look takes (docs/design/seasonal-looks.md) where the planet's body-frame up is `up`, at
/// world time t: the year's phase in quarter turns, 0..4 (0 spring, 1 summer, 2 autumn, 3 winter; the south
/// half a year on), and how much of a season the place has, 0 near the equator to 1 from the mid latitudes on
/// (a smooth step from 10 to 35 degrees, taken on the latitude's sine). shader.wgsl `season` is its port.
/// shortcut: latitude stands in for the climate and the sun for the temperature that trails it; upgrade to the
/// baked climate's summer-to-winter swing and a fixed lag when a warm coast turning with a cold peak looks wrong.
pub fn season_at(up: DVec3, t: f64) -> (f64, f64) {
    let s = ((up.z.abs() - SEASON_FROM) / (SEASON_FULL - SEASON_FROM)).clamp(0.0, 1.0);
    ((t / YEAR * 4.0 + if up.z < 0.0 { 2.0 } else { 0.0 }).rem_euclid(4.0), s * s * (3.0 - 2.0 * s))
}
/// The sines of the latitudes where a look's seasons begin (10 degrees) and are whole (35).
const SEASON_FROM: f64 = 0.1736;
const SEASON_FULL: f64 = 0.5736;
/// A stagger of `days` (a block row's, `blocks::Def::stagger`) in the look table's unit: 1/1024 quarter turn.
pub fn stagger_units(days: f32) -> u32 {
    (days as f64 * DAY / (YEAR / 4.0) * 1024.0).round() as u32
}

/// The air's temperature at the ground, C: the place's yearly mean `mean` and its day's swing `swing` (K,
/// warmest to coldest) from the baked climate, with the season (latitude `lat` and the sun's declination
/// `decl`, radians) and the hour (the sun's hour angle `hour`, `Sky::hour_angle`) on top.
/// shortcut: no seasonal lag (the warmest day is the solstice, not a month after) and no live air (W3's air
/// grid in play will replace the season and day terms with its own surface layer).
pub fn temperature(mean: f64, swing: f64, lat: f64, decl: f64, hour: f64) -> f64 {
    mean + SEASON_K * lat.sin() * decl.sin() + swing / 2.0 * (hour - WARMEST).cos()
}

/// A biome's yearly mean temperature, C, and its day's swing, K: the middle of the band the bake chose it by
/// (`bake::life::LifeParams`' bounds) and a swing by how dry and how wet it is, in the bake's order ICE .. REEF.
/// shortcut: the baked map keeps the biome, not the climate it came from; when `bake::Map` keeps `life::Life`'s
/// `temp` and `swing` per cell (a bake format change), the F3 overlay reads those instead.
pub const BIOME_CLIMATE: [(f64, f64); 15] = [
    (-20.0, 6.0), (-10.0, 8.0), (0.0, 8.0), (10.0, 8.0), (12.0, 10.0), (18.0, 14.0), (24.0, 18.0), (24.0, 12.0),
    (26.0, 6.0), (12.0, 6.0), (-3.0, 10.0), (18.0, 6.0), (15.0, 2.0), (8.0, 1.0), (26.0, 2.0),
];

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
        // a day later the same, but for the season's drift (the declination moves up to 2.3 degrees a day)
        assert!((elevation(&s, DAY, DVec3::Y) - e0).abs() < 2.5);
        let flat = Sky { tilt: 0.0, ..s };
        assert!((elevation(&flat, DAY, DVec3::Y) - elevation(&flat, 0.0, DVec3::Y)).abs() < 0.01);
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

    /// The sun's height at a place's local noon, `lat` degrees north, near world time `t` (searched over a
    /// day), and the declination then (it moves some 2 degrees a day near an equinox).
    fn noon_height(s: &Sky, t: f64, lat: f64) -> (f64, f64) {
        let up = DVec3::new(0.0, lat.to_radians().cos(), lat.to_radians().sin());
        let at = |k: i32| t - DAY / 2.0 + k as f64;
        let k = (0..1800).max_by(|&a, &b| elevation(s, at(a), up).total_cmp(&elevation(s, at(b), up))).unwrap();
        (elevation(s, at(k), up), s.declination(at(k)).to_degrees())
    }

    #[test]
    fn the_noon_sun_follows_latitude_and_season() {
        // ADR (seasons): the spin axis tilted TILT, t = 0 the northern spring equinox
        let s = sky();
        let e = s.tilt.to_degrees();
        assert!((e - 25.0).abs() < 1e-9, "the developer's tilt");
        for (t, decl) in [(0.0, 0.0), (YEAR / 4.0, e), (YEAR / 2.0, 0.0), (0.75 * YEAR, -e)] {
            assert!((s.declination(t).to_degrees() - decl).abs() < 0.05, "{t}: {}", s.declination(t).to_degrees());
            for lat in [0.0, 30.0, 60.0, -45.0] {
                let (got, d) = noon_height(&s, t, lat);
                assert!((d - decl).abs() < 2.5, "{d} at {t}");
                let want = 90.0 - (lat - d).abs();
                assert!((got - want).abs() < 0.2, "lat {lat} at {t}: {got} against {want}");
            }
        }
        // past the polar circle the summer sun never sets and the winter sun never rises
        let pole = DVec3::new(0.0, 0.1, 1.0).normalize();
        assert!((0..18).all(|k| elevation(&s, YEAR / 4.0 + k as f64 * 100.0, pole) > 0.0));
        assert!((0..18).all(|k| elevation(&s, 0.75 * YEAR + k as f64 * 100.0, pole) < 0.0));
    }

    #[test]
    fn the_day_is_longer_in_summer() {
        let s = sky();
        // counted: seconds of a day the sun's centre is up, against the formula
        let up_for = |t0: f64, lat: f64| {
            let up = DVec3::new(0.0, lat.to_radians().cos(), lat.to_radians().sin());
            (0..1800).filter(|&k| elevation(&s, t0 + k as f64, up) > 0.0).count() as f64
        };
        for t in [0.0, YEAR / 4.0, YEAR / 2.0, 0.75 * YEAR, YEAR / 9.0] {
            // (not near the poles: there the day's own drift of the declination moves sunset by minutes)
            for lat in [0.0, 40.0, -55.0, 62.0] {
                let (got, want) = (up_for(t - DAY / 2.0, lat), day_length(lat.to_radians(), s.declination(t)));
                assert!((got - want).abs() < 12.0, "lat {lat} at {t}: {got} against {want}");
            }
        }
        assert_eq!(day_length(0.5, 0.0), DAY / 2.0);
        // 46 degrees north: 15 of 24 hours in summer, 9 in winter
        assert!(day_length(0.8, s.tilt) > 0.6 * DAY && day_length(0.8, -s.tilt) < 0.4 * DAY);
        assert_eq!(day_length(1.5, s.tilt), DAY);
        assert_eq!(day_length(-1.5, s.tilt), 0.0);
    }

    #[test]
    fn a_looks_season_follows_the_year_and_the_hemisphere() {
        let at = |lat: f64| DVec3::new(lat.to_radians().cos(), 0.0, lat.to_radians().sin());
        for t in [0.0, 777.0, YEAR / 3.0, 5.6 * YEAR] {
            assert_eq!(season_at(at(0.0), t).1, 0.0, "the equator has no seasons");
            assert_eq!(season_at(at(-8.0), t).1, 0.0);
            assert_eq!((season_at(at(40.0), t).1, season_at(at(-80.0), t).1), (1.0, 1.0), "the mid latitudes on have them whole");
            let (n, s) = (season_at(at(50.0), t).0, season_at(at(-50.0), t).0);
            assert!((0.0..4.0).contains(&n) && ((s - n).rem_euclid(4.0) - 2.0).abs() < 1e-9, "the south is half a year on: {n} {s}");
        }
        let mid = season_at(at(22.0), 0.0).1;
        assert!(mid > 0.2 && mid < 0.8, "between, a part: {mid}");
        // `/time set <season>` lands on that season's pure look: within half a day of its start
        for q in 0..4 {
            let p = season_at(at(50.0), next_season(100.0, q)).0;
            let off = (p - q as f64 + 2.0).rem_euclid(4.0) - 2.0;
            assert!(off.abs() <= 0.5 * DAY / (YEAR / 4.0) + 1e-9, "season {q}: phase {p}");
        }
    }

    #[test]
    fn tilt_zero_is_the_old_sky() {
        // the sky before the tilt: the spin about the ecliptic's +z, as ADR 0022 had it
        let s = Sky { tilt: 0.0, ..sky() };
        for t in [0.0, 225.0, 1234.5, YEAR / 3.0, 0.7 * PERIOD] {
            let u = t.rem_euclid(PERIOD);
            let old = DQuat::from_rotation_z(SPIN0 + TAU * u * (1.0 / DAY + 1.0 / YEAR));
            assert_eq!(s.planet(t).rot, old);
            assert_eq!(s.declination(t), 0.0);
        }
    }

    #[test]
    fn the_tilt_moves_no_body_so_eclipses_stay_perfect() {
        // ADR 0041: the tilt turns only the planet about its centre; the sun, the planet's and the moon's
        // centres and the moon's turn are the same at any season, so every eclipse is too
        let (a, b) = (Sky { tilt: 0.0, ..sky() }, sky());
        let mut t = 0.0;
        while t < PERIOD {
            assert_eq!((a.planet(t).centre, a.moon(t)), (b.planet(t).centre, b.moon(t)));
            t += 997.0;
        }
    }

    #[test]
    fn a_season_is_reached_at_the_same_hour() {
        // (also just before and after the year's turn, and exactly on a season)
        for (t, q) in (0..4).map(|q| (5000.0, q)).chain([(YEAR - 0.3 * DAY, 0), (0.4 * DAY, 0), (YEAR / 2.0, 2), (YEAR / 4.0, 3)]) {
            let to = next_season(t, q);
            assert!(to >= t && to < t + YEAR, "{q}: {to}");
            assert!((clock(to).1 - clock(t).1).abs() < 1e-6, "the hour stays");
            let off = (to / YEAR - q as f64 / 4.0).rem_euclid(1.0);
            assert!(off.min(1.0 - off) * YEAR <= DAY / 2.0 + 1e-6, "{q}: {off}");
        }
        assert_eq!(season("summer"), Some(1));
        assert_eq!(season("noon"), None);
    }

    #[test]
    fn temperature_by_season_and_hour() {
        let s = sky();
        let (mean, swing) = BIOME_CLIMATE[crate::bake::life::TEMPERATE_FOREST as usize];
        assert_eq!((mean, swing), (10.0, 8.0));
        assert!(BIOME_CLIMATE[crate::bake::life::ICE as usize].0 < BIOME_CLIMATE[crate::bake::life::DESERT as usize].0);
        let lat = 45f64.to_radians();
        let at = |t: f64, lat: f64, h: f64| temperature(mean, swing, lat, s.declination(t), h);
        // the year: warm in the northern summer, cold in its winter, the other way in the south; the equator
        // and the equinoxes keep the mean
        assert!(at(YEAR / 4.0, lat, 0.0) > at(0.75 * YEAR, lat, 0.0) + 15.0);
        assert!(at(YEAR / 4.0, -lat, 0.0) < at(0.75 * YEAR, -lat, 0.0) - 15.0);
        assert!((at(YEAR / 4.0, 0.0, 0.0) - at(0.75 * YEAR, 0.0, 0.0)).abs() < 1e-9);
        // the day: warmest after noon, coldest before dawn, `swing` apart
        let day: Vec<f64> = (0..240).map(|k| at(0.0, lat, (k as f64 / 240.0 - 0.5) * TAU)).collect();
        let (hi, lo) = (day.iter().copied().fold(f64::MIN, f64::max), day.iter().copied().fold(f64::MAX, f64::min));
        assert!((hi - lo - swing).abs() < 0.01 && ((hi + lo) / 2.0 - mean).abs() < 0.01);
        let warmest = day.iter().position(|&v| v == hi).unwrap() as f64 / 240.0 * 24.0 - 12.0;
        assert!((1.0..=3.0).contains(&warmest), "{warmest} h after noon");
        // tilt 0: no seasons
        let flat = Sky { tilt: 0.0, ..sky() };
        assert_eq!(temperature(mean, swing, lat, flat.declination(YEAR / 4.0), 0.0), temperature(mean, swing, lat, 0.0, 0.0));
        // the hour angle: 0 at local noon, a quarter turn at 18:00 local
        let up = DVec3::new(0.0, lat.cos(), lat.sin());
        let noon = next_hour(0.0, 12.0);
        assert!(s.hour_angle(noon, up).abs() < 0.05, "{}", s.hour_angle(noon, up));
        assert!((s.hour_angle(next_hour(0.0, 18.0), up) - TAU / 4.0).abs() < 0.05);
    }

    /// A sea all around, for the tide tests.
    const ROUND: DMat3 = DMat3::from_diagonal(DVec3::splat(1.0 / 3.0));

    /// The world times nearest a new moon and the first quarter after it, from t0: Luna nearest the sun, then
    /// nearest a right angle to it.
    fn phases(s: &Sky, t0: f64) -> (f64, f64) {
        let ts = (0..4000).map(|k| t0 + MONTH * k as f64 / 4000.0);
        let cos = |t: f64| s.moon_dir(t).dot(s.sun_dir(t));
        let new = ts.clone().max_by(|&a, &b| cos(a).total_cmp(&cos(b))).unwrap();
        let quarter = ts.filter(|&t| t > new && t < new + MONTH / 2.0).min_by(|&a, &b| cos(a).abs().total_cmp(&cos(b).abs())).unwrap();
        (new, quarter)
    }

    /// The tide's lowest and highest at `up` over a lunar day from t, and how many highs it has.
    fn tide_day(s: &Sky, t: f64, up: DVec3) -> (f64, f64, usize) {
        let lunar_day = 1.0 / (1.0 / DAY - 1.0 / MONTH);
        let h: Vec<f64> = (0..=960).map(|k| s.tide(t + lunar_day * k as f64 / 960.0, &ROUND).at(up)).collect();
        let highs = (1..h.len() - 1).filter(|&k| h[k] > h[k - 1] && h[k] >= h[k + 1]).count();
        (h.iter().copied().fold(f64::MAX, f64::min), h.iter().copied().fold(f64::MIN, f64::max), highs)
    }

    #[test]
    fn the_tide_is_a_metre_at_springs_half_at_neaps_and_twice_a_lunar_day() {
        let s = sky();
        let (new, quarter) = phases(&s, 3.0 * DAY);
        // on the equator: Luna's declination is at most the tilt and its inclination, so near the full bulge
        let up = DVec3::X;
        let (lo, hi, highs) = tide_day(&s, new - 0.5 * DAY, up);
        assert!((0.8..=1.0).contains(&(hi - lo)), "springs: {lo:.3} to {hi:.3} m");
        assert_eq!(highs, 2, "two high tides a lunar day");
        let (nlo, nhi, _) = tide_day(&s, quarter - 0.5 * DAY, up);
        assert!((0.4..=0.6).contains(&(nhi - nlo)), "neaps: {nlo:.3} to {nhi:.3} m");
        // never over TIDE_HIGH, anywhere, at springs
        for k in 0..200 {
            let t = new + k as f64 * 37.0;
            for d in [DVec3::X, DVec3::Y, DVec3::Z, DVec3::new(1.0, 1.0, 0.3).normalize()] {
                assert!(s.tide(t, &ROUND).at(d) <= TIDE_HIGH);
            }
        }
        // a pure function of the time: the same bits again, and the same a sky's period later
        assert_eq!(s.tide(new, &ROUND), s.tide(new, &ROUND));
        assert!((s.tide(new, &ROUND).at(up) - s.tide(new + PERIOD, &ROUND).at(up)).abs() < 1e-9);
    }

    #[test]
    fn the_tide_moves_water_and_makes_none() {
        let s = sky();
        // a Fibonacci sphere: even directions
        let dirs: Vec<DVec3> = (0..20000)
            .map(|k| {
                let z = 1.0 - (k as f64 + 0.5) / 10000.0;
                let a = k as f64 * 2.399963229728653;
                let r = (1.0 - z * z).sqrt();
                DVec3::new(r * a.cos(), r * a.sin(), z)
            })
            .collect();
        // and a lopsided ocean on about two thirds of it
        let ocean: Vec<DVec3> = dirs.iter().copied().filter(|d| d.z < 0.3 || d.x > 0.8).collect();
        let m = ocean.iter().fold(DMat3::ZERO, |m, d| m + DMat3::from_cols(*d * d.x, *d * d.y, *d * d.z)) * (1.0 / ocean.len() as f64);
        for k in 0..50 {
            let t = k as f64 * 431.0;
            let round = dirs.iter().map(|&d| s.tide(t, &ROUND).at(d)).sum::<f64>() / dirs.len() as f64;
            let own = ocean.iter().map(|&d| s.tide(t, &m).at(d)).sum::<f64>() / ocean.len() as f64;
            assert!(round.abs() < 1e-4 && own.abs() < 1e-9, "mean level {round} and {own} m at {t}");
            // and the bulges still move: the range at springs is not taken out
            let h: Vec<f64> = ocean.iter().map(|&d| s.tide(t, &m).at(d)).collect();
            let range = h.iter().copied().fold(f64::MIN, f64::max) - h.iter().copied().fold(f64::MAX, f64::min);
            assert!(range > 0.4, "{range} m across the ocean");
            assert!(h.iter().all(|&v| v <= TIDE_HIGH));
        }
    }
}
