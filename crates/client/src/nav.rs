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

/// A place the player marked: on a body, from its centre, in the body's own frame (it turns with the body).
#[derive(Clone, Debug, PartialEq)]
pub struct Waypoint {
    pub name: String,
    pub body: usize,
    pub rel: DVec3,
}

/// The colours waypoints take in turn.
pub const COLOURS: [[f32; 3]; 6] = [[1.0, 0.45, 0.4], [1.0, 0.8, 0.3], [0.5, 0.9, 0.55], [0.44, 0.83, 1.0], [0.75, 0.6, 1.0], [1.0, 0.6, 0.85]];

/// The first "Waypoint N" not taken.
pub fn next_name(list: &[Waypoint]) -> String {
    (1..).map(|n| format!("Waypoint {n}")).find(|n| list.iter().all(|w| &w.name != n)).unwrap()
}

/// The waypoints' file: a line each, `body|x|y|z|name`. A line that does not read is left out.
pub fn parse(text: &str) -> Vec<Waypoint> {
    text.lines()
        .filter_map(|l| {
            let mut f = l.splitn(5, '|');
            let body = f.next()?.trim().parse().ok()?;
            let mut n = || f.next()?.trim().parse::<f64>().ok().filter(|v| v.is_finite());
            let rel = DVec3::new(n()?, n()?, n()?);
            Some(Waypoint { name: f.next()?.to_string(), body, rel })
        })
        .collect()
}
pub fn text(list: &[Waypoint]) -> String {
    list.iter().map(|w| format!("{}|{}|{}|{}|{}\n", w.body, w.rel.x, w.rel.y, w.rel.z, w.name.replace(['\n', '\r'], " "))).collect()
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
        let list = vec![
            Waypoint { name: "Waypoint 1".into(), body: 0, rel: DVec3::new(1.5, -4000.25, 3.0) },
            Waypoint { name: "Home | the cave".into(), body: 1, rel: DVec3::new(0.0, 1061.0, 0.0) },
            Waypoint { name: "Waypoint 3".into(), body: 0, rel: DVec3::ZERO },
        ];
        assert_eq!(parse(&text(&list)), list);
        assert_eq!(next_name(&list), "Waypoint 2");
        assert_eq!(next_name(&[]), "Waypoint 1");
        // a line that does not read is left out, the others stay
        assert_eq!(parse("junk\n0|1|2|NaN|x\n0|1|2|3|ok\n"), vec![Waypoint { name: "ok".into(), body: 0, rel: DVec3::new(1.0, 2.0, 3.0) }]);
    }
}
