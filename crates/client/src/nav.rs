//! Where the player is on a body, the compass and the waypoints (docs/design/navigation.md, ADR 0030).
//! North is a body's +z, the axis it spins about (`engine::sky`); longitude counts east from a meridian the
//! caller names: the spawn's on the first body, the face turned to it on a moon.
use glam::DVec3;

/// Latitude and longitude, in degrees, of `rel` (from a body's centre); longitude from `zero`'s meridian,
/// east positive, -180 to 180.
pub fn geo(rel: DVec3, zero: DVec3) -> (f64, f64) {
    let lat = (rel.z / rel.length().max(1e-9)).clamp(-1.0, 1.0).asin().to_degrees();
    let lon = (rel.y.atan2(rel.x) - zero.y.atan2(zero.x)).to_degrees();
    (lat, (lon + 540.0).rem_euclid(360.0) - 180.0)
}

/// "12.4 N 48.1 E".
pub fn geo_text((lat, lon): (f64, f64)) -> String {
    format!("{:.1} {} {:.1} {}", lat.abs(), if lat < 0.0 { "S" } else { "N" }, lon.abs(), if lon < 0.0 { "W" } else { "E" })
}

/// North and east along the ground where up is `up`; at a pole, where north has no meaning, the body's +x.
pub fn north_east(up: DVec3) -> (DVec3, DVec3) {
    let n = DVec3::Z - up * up.z;
    let n = if n.length() < 1e-6 { DVec3::X } else { n.normalize() };
    (n, n.cross(up))
}

/// The compass bearing of `dir` where up is `up`: degrees east of north, 0 to 360.
pub fn bearing(up: DVec3, dir: DVec3) -> f64 {
    let (n, e) = north_east(up);
    dir.dot(e).atan2(dir.dot(n)).to_degrees().rem_euclid(360.0)
}

/// A place the player marked: on a body, from its centre, in the body's own frame (it turns with the body);
/// its colour and icon (places in COLOURS and ICONS), and where it shows (bugs/2026-11-11_13-15-26).
#[derive(Clone, Debug, PartialEq)]
pub struct Waypoint {
    pub name: String,
    pub body: usize,
    pub rel: DVec3,
    pub colour: usize,
    pub icon: usize,
    pub view: View,
}

/// Where a waypoint shows: from anywhere, from its own body, or within a radius (km) of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum View {
    Always,
    Body,
    Near(f64),
}

/// The views the waypoint menu steps through.
pub const VIEWS: [View; 9] =
    [View::Body, View::Always, View::Near(0.5), View::Near(1.0), View::Near(2.0), View::Near(5.0), View::Near(10.0), View::Near(25.0), View::Near(100.0)];

impl View {
    pub fn text(self) -> String {
        match self {
            View::Always => "always".into(),
            View::Body => "on its own body".into(),
            View::Near(km) => format!("within {km} km"),
        }
    }
}

impl Waypoint {
    /// A new one, shown on its own body as before, in the colour `colour` (COLOURS by turn).
    pub fn new(name: String, body: usize, rel: DVec3, colour: usize) -> Waypoint {
        Waypoint { name, body, rel, colour: colour % COLOURS.len(), icon: 0, view: View::Body }
    }
    /// It shows to a player in frame `on` (a body's, or `world::INERTIAL`) who is `far` metres from it.
    pub fn shown(&self, on: usize, far: f64) -> bool {
        match self.view {
            View::Always => true,
            View::Body => on == self.body,
            View::Near(km) => far <= km * 1000.0,
        }
    }
}

/// The colours waypoints take in turn, and their names in the menu.
pub const COLOURS: [[f32; 3]; 6] = [[1.0, 0.45, 0.4], [1.0, 0.8, 0.3], [0.5, 0.9, 0.55], [0.44, 0.83, 1.0], [0.75, 0.6, 1.0], [1.0, 0.6, 0.85]];
pub const COLOUR_NAMES: [&str; 6] = ["red", "amber", "green", "sky blue", "violet", "pink"];
/// The icons a waypoint's tag and compass mark can have (`hud::icon` draws them).
pub const ICONS: [&str; 4] = ["diamond", "square", "plus", "ring"];

/// The death mark's own colour (needs.md: "in the death's own colour"), none of the player's: bone white.
pub const DEATH: [f32; 3] = [0.93, 0.9, 0.82];

/// The death mark's colour index: none of COLOURS'.
pub const DEATH_K: usize = usize::MAX;

/// A waypoint's colour: its place in COLOURS (wrapped), or the death's.
pub fn colour(k: usize) -> [f32; 3] {
    if k == DEATH_K { DEATH } else { COLOURS[k % COLOURS.len()] }
}

/// The game's marks for this player as the server sent them (needs.md "What death costs"): Death at its last
/// death's pile, Home at its sleeping place; seen from anywhere, as the pile may lie on another body.
pub fn game_marks(w: &engine::world::World, death: Option<engine::world::Block>, home: Option<engine::world::Block>) -> Vec<Waypoint> {
    // (the death a plus, Home a square, in ICONS)
    [(death, "Death", DEATH_K, 2), (home, "Home", 2, 1)]
        .into_iter()
        .filter_map(|(b, name, colour, icon)| {
            let b = b?;
            let body = b.body as usize;
            Some(Waypoint { name: name.into(), body, rel: w.block_centre(b) - w.bodies[body].centre, colour, icon, view: View::Always })
        })
        .collect()
}

/// The first "Waypoint N" not taken.
pub fn next_name(list: &[Waypoint]) -> String {
    (1..).map(|n| format!("Waypoint {n}")).find(|n| list.iter().all(|w| &w.name != n)).unwrap()
}

/// The waypoints' file: a line each, `body|x|y|z|colour|icon|view|name` (view: always, body, or a radius in km).
/// The first format's lines, `body|x|y|z|name`, still read. A line that does not read is left out.
pub fn parse(text: &str) -> Vec<Waypoint> {
    let mut out: Vec<Waypoint> = Vec::new();
    for l in text.lines() {
        let f: Vec<&str> = l.splitn(8, '|').collect();
        let num = |k: usize| f.get(k)?.trim().parse::<f64>().ok().filter(|v| v.is_finite());
        let (Some(body), Some(x), Some(y), Some(z)) = (f[0].trim().parse::<usize>().ok(), num(1), num(2), num(3)) else { continue };
        let view = |v: &str| match v {
            "always" => Some(View::Always),
            "body" => Some(View::Body),
            v => v.parse::<f64>().ok().filter(|km| km.is_finite() && *km > 0.0).map(View::Near),
        };
        let small = |k: usize, n: usize| f.get(k)?.parse::<usize>().ok().map(|v| v % n);
        let mut w = Waypoint::new(String::new(), body, DVec3::new(x, y, z), out.len());
        match (f.len(), small(4, COLOURS.len()), small(5, ICONS.len()), f.get(6).and_then(|v| view(v))) {
            (8, Some(c), Some(i), Some(v)) => (w.name, w.colour, w.icon, w.view) = (f[7].to_string(), c, i, v),
            _ => match l.splitn(5, '|').nth(4) {
                Some(name) => w.name = name.to_string(),
                None => continue,
            },
        }
        out.push(w);
    }
    out
}
pub fn text(list: &[Waypoint]) -> String {
    let view = |v: View| match v {
        View::Always => "always".to_string(),
        View::Body => "body".to_string(),
        View::Near(km) => km.to_string(),
    };
    list.iter()
        .map(|w| format!("{}|{}|{}|{}|{}|{}|{}|{}\n", w.body, w.rel.x, w.rel.y, w.rel.z, w.colour, w.icon, view(w.view), w.name.replace(['\n', '\r'], " ")))
        .collect()
}

/// What the ground of column (face, i, j) is called (the compass's label and F3): its biome (Terra's, or Luna's,
/// Mars's and the icy moon's kinds of ground, one table, `biomes::ROWS`), else its rock province; None on a body
/// with no map (a gas giant, the old noise land).
pub fn ground_name(b: &engine::world::Body, face: u8, i: i32, j: i32) -> Option<&'static str> {
    let (ci, cj) = (i as f64 + 0.5, j as f64 + 0.5);
    match b.biome_at(face as usize, ci, cj, b.dir(face as usize, ci, cj)) {
        Some(k) => engine::biomes::ROWS.biomes.get(k as usize).map(|r| r.name.as_str()),
        None => province_name(b, face, i, j),
    }
}

/// The rock province of column (face, i, j) by its row in data/rocks.toml; None without a map's rock.
pub fn province_name(b: &engine::world::Body, face: u8, i: i32, j: i32) -> Option<&'static str> {
    engine::rock::ROCKS.provinces.get(b.col_rock(face, i, j).province as usize).map(|p| p.name.as_str())
}

/// Luna's elongation at t: how far round her orbit she stands from Sol as seen from Terra, degrees 0 to 360,
/// counted the way she goes (0 new, 90 first quarter, 180 full, 270 last quarter).
pub fn elongation(sky: &engine::sky::Sky, t: f64) -> f64 {
    let p = sky.planet(t).centre;
    let (s, m) = (sky.sun(t) - p, sky.moon(t).centre - p);
    // (her orbit leans 2 degrees off the ecliptic, whose pole is +z: the turn about +z is the one she makes)
    s.cross(m).z.atan2(s.dot(m)).to_degrees().rem_euclid(360.0)
}

/// Luna's eight phases by name, from new on, a 45-degree step of `elongation` each.
pub const PHASES: [&str; 8] = ["new", "waxing crescent", "first quarter", "waxing gibbous", "full", "waning gibbous", "last quarter", "waning crescent"];

/// The phase an elongation (degrees) is in: each name holds the 45 degrees centred on its own.
pub fn phase_name(elong: f64) -> &'static str {
    PHASES[((elong.rem_euclid(360.0) + 22.5) / 45.0) as usize % 8]
}

/// The season at body-frame direction `up` on the home at t, by `sky::season_at`'s quarter (0 the spring there).
pub fn season_name(up: DVec3, t: f64, year: f64) -> &'static str {
    ["spring", "summer", "autumn", "winter"][engine::sky::season_at(up, t, year).0 as usize % 4]
}

/// The local solar time at body-frame direction `up` on the home, hours 0 to 24 (12 when Sol is due south or north).
/// Sol as the world poses the home (ADR 0177); the same angle as `Sky::hour_angle`, since body 0's pose is
/// `Sky::planet` (the review: an earlier note here said that one drifted; it does not).
pub fn local_hour(w: &engine::world::World, t: f64, up: DVec3) -> f64 {
    w.local_hour(0, t, up)
}

/// What Sol does today at body-frame direction `up` when it does not cross the horizon there (`decl` its
/// declination, radians): ", polar night" or ", polar day", else nothing. Past the polar circle in its winter Sol
/// stays down at every local hour: F3's 15:57 with Sol 25 degrees under the horizon (2026-11-24) was the pole at
/// the solstice, where Sol stands the tilt's 25 degrees down all day, and the hour was right.
/// shortcut: Sol's centre on the plain horizon (no disc, no height of the eye).
pub fn polar(up: DVec3, decl: f64) -> &'static str {
    let lat = up.z.clamp(-1.0, 1.0).asin();
    if lat.abs() + decl.abs() <= std::f64::consts::FRAC_PI_2 {
        ""
    } else if lat * decl > 0.0 {
        ", polar day"
    } else {
        ", polar night"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sol's height over the horizon at body-frame direction `up` on the home at t, degrees, as the frame takes it
    /// for F3's Sol line (frame.rs `sun_deg`): from the poses.
    fn sol_deg(w: &engine::world::World, t: f64, up: DVec3) -> f64 {
        let p = w.pose(0, t);
        up.dot((p.rot.inverse() * (w.sky.sun(t) - p.centre)).normalize()).clamp(-1.0, 1.0).asin().to_degrees()
    }

    /// F3's local time against where Sol stands (the review of 2026-11-24: 15:57 local with Sol 25 degrees down):
    /// in the old sky and under real gravity, at every latitude, longitude and season tried, Sol is highest at
    /// local noon, its height at every minute of the day is the one the hour, the latitude and the declination
    /// give (sin h = sin lat sin dec + cos lat cos dec cos H), and at the equinox on the equator it is on the
    /// horizon at 6 and 18. So the hour is right wherever Sol's height is; the pair seen is the polar night.
    #[test]
    fn the_local_time_is_tied_to_where_sol_stands() {
        use std::f64::consts::TAU;
        let real = engine::world::World::with_grid(256, engine::bodies::moon_columns(256), 1, None, None, 0.0, engine::world::GENERATION);
        for w in [engine::world::World::new(256, 32, 1), real] {
            let year = w.sky.year;
            for (lat, lon) in [(-60.0f64, 10.0f64), (-28.0, 356.5), (0.0, 100.0), (45.0, 250.0), (70.0, 181.0)] {
                let (la, lo) = (lat.to_radians(), lon.to_radians());
                let up = DVec3::new(la.cos() * lo.cos(), la.cos() * lo.sin(), la.sin());
                for t0 in [0.0, 0.25 * year, 0.5 * year, 0.7 * year] {
                    let day: Vec<(f64, f64, f64)> = (0..1440).map(|k| t0 + k as f64 * engine::sky::DAY / 1440.0).map(|t| (t, sol_deg(&w, t, up), local_hour(&w, t, up))).collect();
                    let top = day.iter().max_by(|a, b| a.1.total_cmp(&b.1)).unwrap();
                    assert!((top.2 - 12.0).abs() < 0.1, "lat {lat} lon {lon} t {t0}: Sol highest ({:.1} deg) at {:.2} h local", top.1, top.2);
                    for &(t, deg, h) in &day {
                        let (dec, ha) = (w.sky.declination(t), (h - 12.0) / 24.0 * TAU);
                        let want = (la.sin() * dec.sin() + la.cos() * dec.cos() * ha.cos()).asin().to_degrees();
                        assert!((deg - want).abs() < 0.01, "lat {lat} lon {lon} t {t}: {h:.2} h local, Sol {deg:.2} deg, by the hour {want:.2}");
                    }
                    if lat == 0.0 && t0 == 0.0 {
                        for hour in [6.0, 18.0] {
                            let at = day.iter().min_by(|a, b| (a.2 - hour).abs().total_cmp(&(b.2 - hour).abs())).unwrap();
                            assert!(at.1.abs() < 1.5, "the equinox on the equator at {hour} h: Sol {:.2} deg", at.1);
                        }
                    }
                }
            }
            // the pair F3 showed: a step from the south pole in its midwinter Sol is the tilt under the horizon at
            // every hour of the day, and the hour there is whatever the longitude of that step is
            let t = 0.25 * year;
            let down = engine::sky::TILT.to_degrees();
            for lo in [0.0f64, 1.0, 2.5, 4.0] {
                let up = DVec3::new(1e-6 * lo.cos(), 1e-6 * lo.sin(), -1.0).normalize();
                assert!((sol_deg(&w, t, up) + down).abs() < 0.2, "Sol at the pole in midwinter: {:.2} deg", sol_deg(&w, t, up));
                assert_eq!(polar(up, w.sky.declination(t)), ", polar night");
                assert_eq!(polar(-up, w.sky.declination(t)), ", polar day");
            }
            let hours: Vec<f64> = [0.0f64, 1.0, 2.5, 4.0].iter().map(|lo| local_hour(&w, t, DVec3::new(1e-6 * lo.cos(), 1e-6 * lo.sin(), -1.0).normalize())).collect();
            assert!(hours.iter().any(|h| (h - hours[0]).abs() > 3.0), "the hours round the pole: {hours:?}");
            // inside the polar circles Sol rises and sets every day; past them only round the equinoxes
            assert_eq!((polar(DVec3::new(0.0, 1.0, 1.0).normalize(), w.sky.declination(t)), polar(DVec3::new(0.0, 1.0, -1.0).normalize(), w.sky.declination(t))), ("", ""));
            assert_eq!(polar(DVec3::new(0.0, 0.1, -1.0).normalize(), w.sky.declination(0.0)), "");
        }
    }

    #[test]
    fn luna_s_phases_follow_her_elongation() {
        assert_eq!([0.0, 44.0, 70.0, 90.0, 180.0, 270.0, 300.0, 359.0].map(phase_name), ["new", "waxing crescent", "first quarter", "first quarter", "full", "last quarter", "waning crescent", "new"]);
        // in the sky she goes round once a month, the way her elongation is counted: forward a day, it grows
        let w = engine::world::World::new(256, 32, 1);
        let s = w.sky;
        let (a, b) = (elongation(&s, 1000.0), elongation(&s, 1000.0 + engine::sky::DAY));
        assert!((b - a).rem_euclid(360.0) > 5.0 && (b - a).rem_euclid(360.0) < 60.0, "{a} then {b}");
        // the spawn's clock and the local solar time agree within the spawn's longitude's offset: an hour later is an hour later
        let up = DVec3::Y;
        let (h0, h1) = (local_hour(&w, 0.0, up), local_hour(&w, engine::sky::DAY / 24.0, up));
        assert!(((h1 - h0).rem_euclid(24.0) - 1.0).abs() < 0.05, "{h0} {h1}");
        assert_eq!(season_name(DVec3::Z, 1.0, s.year), "spring");
    }

    #[test]
    fn local_noon_is_where_sol_stands_overhead_in_the_world_s_own_orbits() {
        // local noon under Sol and midnight opposite it, by the poses (F3's 15:57 with Sol 25 degrees down, 2026-11-24,
        // was not a drift of the hour: this held before the rewrite too, which changed nothing it computes)
        let w = engine::world::World::new(256, 32, 1);
        for t in [0.0, 5000.0, 40190.0, 123456.0] {
            let p = w.pose(0, t);
            let s = (p.rot.inverse() * (w.sky.sun(t) - p.centre)).normalize();
            let h = local_hour(&w, t, s);
            assert!((h - 12.0).abs() < 0.1, "under Sol at {t}: {h}");
            let h = local_hour(&w, t, -s);
            assert!(h < 0.1 || h > 23.9, "opposite Sol at {t}: {h}");
        }
    }

    #[test]
    fn the_ground_is_named_by_its_biome_and_a_map_less_body_by_nothing() {
        let plain = engine::world::World::new(256, 32, 1);
        assert_eq!(ground_name(&plain.bodies[0], 0, 10, 10), None);
        let w = engine::world::World::with_map(512, 32, 5, Some(std::sync::Arc::new(engine::bake::Map::bake(5, 512))));
        let b = &w.bodies[0];
        let names: std::collections::HashSet<&str> = (0..6u8).flat_map(|f| (0..8).map(move |k| (f, 20 + k * 60))).filter_map(|(f, k)| ground_name(b, f, k, k)).collect();
        assert!(names.len() >= 2 && names.iter().all(|n| engine::bake::life::BIOMES.iter().any(|b| b.0 == *n)), "{names:?}");
        assert!(province_name(b, 0, 100, 100).is_some());
    }

    #[test]
    fn latitude_longitude_and_bearings_follow_the_spin_axis() {
        // the meridian through +y (the spawn's pole of the grid): on the equator, longitude 0
        let zero = DVec3::Y;
        let near = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9;
        assert!(near(geo(DVec3::Y * 4000.0, zero), (0.0, 0.0)));
        assert_eq!(geo(DVec3::Z, zero).0, 90.0);
        assert_eq!(geo(-DVec3::Z, zero).0, -90.0);
        // the body turns about +z from x towards y, so east of +y is towards -x
        assert!(near(geo(-DVec3::X, zero), (0.0, 90.0)));
        assert!(near(geo(DVec3::X, zero), (0.0, -90.0)));
        assert!(near(geo(DVec3::new(0.0, -1.0, 1.0), zero), (45.0, 180.0)) || near(geo(DVec3::new(0.0, -1.0, 1.0), zero), (45.0, -180.0)));
        assert_eq!(geo_text((-12.44, 48.06)), "12.4 S 48.1 E");
        assert_eq!(geo_text((0.0, -179.96)), "0.0 N 180.0 W");
        // standing on +y: north is +z, east is -x
        let up = DVec3::Y;
        assert!((bearing(up, DVec3::Z) - 0.0).abs() < 1e-9);
        assert!((bearing(up, -DVec3::X) - 90.0).abs() < 1e-9);
        assert!((bearing(up, -DVec3::Z) - 180.0).abs() < 1e-9);
        assert!((bearing(up, DVec3::X) - 270.0).abs() < 1e-9);
        // looking down a little does not change the bearing; walking east raises the longitude
        assert!((bearing(up, (DVec3::Z - DVec3::Y * 0.5).normalize()) - 0.0).abs() < 1e-9);
        let (_, e) = north_east(up);
        assert!(geo(up + e * 0.01, zero).1 > 0.0);
        // at the pole north is still a direction, not a NaN
        assert!(bearing(DVec3::Z, DVec3::X).is_finite());
    }

    #[test]
    fn the_games_marks_are_death_in_its_own_colour_and_home_where_the_server_puts_them() {
        let w = engine::world::World::new(256, 32, 1);
        let (pile, bed) = (w.air_over_ground(0, DVec3::Y), w.air_over_ground(1, DVec3::X));
        let m = game_marks(&w, Some(pile), Some(bed));
        assert_eq!(m.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Death", "Home"]);
        assert_eq!((m[0].body, m[1].body), (0, 1));
        assert!((m[0].rel + w.bodies[0].centre).distance(w.block_centre(pile)) < 1e-9);
        assert_eq!((colour(m[0].colour), m[0].view), (DEATH, View::Always));
        assert!(COLOURS.iter().all(|c| *c != DEATH), "death has its own colour");
        assert!(game_marks(&w, None, None).is_empty());
    }

    #[test]
    fn waypoints_read_back_and_take_the_first_free_name() {
        let at = |name: &str, body, rel| Waypoint::new(name.into(), body, rel, 0);
        let list = vec![
            at("Waypoint 1", 0, DVec3::new(1.5, -4000.25, 3.0)),
            Waypoint { colour: 4, icon: 2, view: View::Always, ..at("Home | the cave", 1, DVec3::new(0.0, 1061.0, 0.0)) },
            Waypoint { view: View::Near(2.5), ..at("Waypoint 3", 0, DVec3::ZERO) },
        ];
        assert_eq!(parse(&text(&list)), list);
        assert_eq!(next_name(&list), "Waypoint 2");
        assert_eq!(next_name(&[]), "Waypoint 1");
        // a line that does not read is left out, the others stay; a line of the first format reads, its colour
        // by its place as before, shown on its own body
        let old = parse("junk\n0|1|2|NaN|x\n0|1|2|3|ok\n1|0|0|5|a | b\n");
        assert_eq!(old, vec![at("ok", 0, DVec3::new(1.0, 2.0, 3.0)), Waypoint { colour: 1, ..at("a | b", 1, DVec3::new(0.0, 0.0, 5.0)) }]);
    }

    #[test]
    fn a_waypoint_shows_where_its_view_says() {
        // bugs/2026-11-11_13-15-26: always, on its own body only, or within a radius
        let w = |view| Waypoint { view, ..Waypoint::new("x".into(), 1, DVec3::ZERO, 0) };
        assert!(w(View::Always).shown(0, 1e6) && w(View::Always).shown(usize::MAX, 1e9));
        assert!(w(View::Body).shown(1, 1e6) && !w(View::Body).shown(0, 10.0));
        assert!(w(View::Near(5.0)).shown(0, 4999.0) && !w(View::Near(5.0)).shown(1, 5001.0));
        // the menu steps through every view and back around
        let mut v = View::Body;
        for _ in 0..VIEWS.len() {
            v = crate::menu::cycle(&VIEWS, v, false);
        }
        assert_eq!(v, View::Body);
        assert_eq!((View::Near(25.0).text(), View::Body.text()), ("within 25 km".to_string(), "on its own body".to_string()));
    }
}
