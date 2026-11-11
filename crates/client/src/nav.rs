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

/// The next moment from `from` on when Luna's shadow falls on Terra with its middle on the ground (`lunar`
/// false), or Luna is wholly in Terra's shadow (true): the closest line-up of the three, searched forward in
/// the sky's own functions over one whole cycle of it (bugs/2026-11-11_13-12-42). None when there is none.
pub fn next_eclipse(sky: &engine::sky::Sky, from: f64, lunar: bool) -> Option<f64> {
    // how far Luna's centre is off the line from Sol through Terra, on the near side (the far side for lunar)
    let off = |t: f64| {
        let p = sky.planet(t).centre;
        let s = (sky.sun(t) - p).normalize();
        let m = sky.moon(t).centre - p;
        let along = m.dot(s);
        if (along > 0.0) != lunar { (m - s * along).length() } else { f64::MAX }
    };
    let near = if lunar { sky.planet_radius - 2.0 * sky.moon_radius } else { 0.8 * sky.planet_radius };
    // a line-up lasts over a thousand seconds (Luna moves a few metres a second), so a minute's step finds it
    const STEP: f64 = 60.0;
    // (`closing`: Luna was seen coming nearer the line, so a search begun just past one eclipse's middle
    // finds the next one)
    let (mut t, mut prev, mut closing) = (from, off(from), false);
    while t < from + engine::sky::PERIOD + 4000.0 {
        let next = off(t + STEP);
        closing |= next < prev;
        if closing && prev < near && next >= prev {
            // the closest moment lies within a step either side: narrow it down
            let (mut a, mut b) = ((t - STEP).max(from), t + STEP);
            for _ in 0..60 {
                let (x, y) = (a + (b - a) / 3.0, b - (b - a) / 3.0);
                if off(x) < off(y) { b = y } else { a = x }
            }
            return Some((a + b) / 2.0);
        }
        (t, prev) = (t + STEP, next);
    }
    None
}

/// Where the line from Sol through Luna's centre meets a sphere of radius `r` round Terra at time `t`: from
/// Terra's centre, in the inertial frame's directions. None when it misses.
pub fn shadow_at(sky: &engine::sky::Sky, t: f64, r: f64) -> Option<DVec3> {
    let p = sky.planet(t).centre;
    let m = sky.moon(t).centre - p;
    let d = (m - (sky.sun(t) - p)).normalize();
    let b = m.dot(d);
    let disc = b * b - (m.length_squared() - r * r);
    (disc > 0.0).then(|| m + d * (-b - disc.sqrt()))
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn the_next_eclipses_are_found_in_the_sky_itself() {
        // bugs/2026-11-11_13-12-42: searched forward in the sky's own functions, so new orbits stay right
        let w = engine::world::World::new(256, 32, 1);
        let s = w.sky;
        let t0 = 5000.0;
        let started = std::time::Instant::now();
        let solar = next_eclipse(&s, t0, false).expect("a solar eclipse within the sky's cycle");
        let lunar = next_eclipse(&s, t0, true).expect("a lunar eclipse within the sky's cycle");
        eprintln!("searched both in {:.1} ms", started.elapsed().as_secs_f64() * 1000.0);
        assert!(solar > t0 && lunar > t0);
        // where the shadow's middle meets the ground, the sun is hidden; Luna's centre sees none of it
        let p = s.planet(solar).centre;
        let hit = shadow_at(&s, solar, s.planet_radius).expect("the shadow's middle on Terra");
        assert!(s.eclipse(solar, p + hit) > 0.999, "{}", s.eclipse(solar, p + hit));
        assert!((hit.length() - s.planet_radius).abs() < 1e-6);
        assert_eq!(s.covered(lunar, s.moon(lunar).centre, s.planet(lunar).centre, s.planet_radius), 1.0);
        // (the review) asked again just past an eclipse's middle, the answer is the one after it, not this one
        for (found, lunar) in [(solar, false), (lunar, true)] {
            let after = next_eclipse(&s, found + 90.0, lunar).expect("another");
            assert!(after > found + 2000.0, "{after} after {found}");
        }
        // nothing earlier: a minute before, the sun was not yet hidden there
        assert!(s.eclipse(solar - 60.0, p + hit) < 0.999);
    }
}
