//! The first-person hand's swing (ADR 0115, 0170): `data/hand.toml`'s numbers and the maths the client draws
//! the held item and the bare arm by, here so the Studio can read and measure them too. Presentation only: the
//! server never reads it.
use glam::{DMat4, DVec3};
use serde::{Deserialize, Serialize};

pub const HAND_TOML: &str = include_str!("../../../data/hand.toml");

/// The swing's numbers (the file's comments say what each is).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hand {
    pub seconds: f64,
    pub snap: f64,
    pub reach: f64,
    pub aim: f64,
    pub chop: f64,
    pub arm_hand: [f64; 3],
    pub arm_way: [f64; 3],
}

/// Every scalar's range, for the checks and the Studio's sliders: (name, min, max, unit).
pub const RANGES: [(&str, f32, f32, &str); 5] = [
    ("seconds", 0.05, 2.0, "s"),
    ("snap", 1.0, 6.0, "x"),
    ("reach", 0.0, 1.0, "share"),
    ("aim", 0.5, 4.0, "x"),
    ("chop", 0.0, 1.6, "rad"),
];

/// The numbers in force: the compiled-in file until the client reads `data/hand.toml` again.
pub static HAND: crate::live::Live<Hand> = crate::live::Live::new(|| Hand::parse(HAND_TOML).unwrap_or_else(|e| panic!("{e}")));

impl Hand {
    pub fn get(&self, name: &str) -> f32 {
        (match name {
            "seconds" => self.seconds,
            "snap" => self.snap,
            "reach" => self.reach,
            "aim" => self.aim,
            "chop" => self.chop,
            _ => 0.0,
        }) as f32
    }
    pub fn set(&mut self, name: &str, v: f32) {
        let v = v as f64;
        match name {
            "seconds" => self.seconds = v,
            "snap" => self.snap = v,
            "reach" => self.reach = v,
            "aim" => self.aim = v,
            "chop" => self.chop = v,
            _ => {}
        }
    }

    /// Reads the file's text; a value outside its range is refused by name.
    pub fn parse(text: &str) -> Result<Hand, String> {
        let h: Hand = toml::from_str(text).map_err(|e| format!("data/hand.toml: {e}"))?;
        for (n, lo, hi, _) in RANGES {
            let v = h.get(n);
            if !(lo..=hi).contains(&v) {
                return Err(format!("data/hand.toml: {n} = {v} is outside {lo}..{hi}"));
            }
        }
        Ok(h)
    }

    /// The file's text of these numbers (without its comments).
    pub fn write(&self) -> String {
        toml::to_string(self).expect("the hand always serialises")
    }

    /// How far through its swing the hand is `age` seconds after a dig, place, strike or pick-up: 0 at rest and
    /// at its end, 1 once, early (a quick strike, an eased return).
    pub fn curve(&self, age: f64) -> f32 {
        let x = age / self.seconds;
        if !(0.0..1.0).contains(&x) {
            return 0.0;
        }
        (std::f64::consts::PI * (1.0 - (1.0 - x).powf(self.snap))).sin() as f32
    }

    /// The bare arm's hand end in the eye's frame (`fwd`, `right`, `up` the eye's axes).
    pub fn hand_at(&self, fwd: DVec3, right: DVec3, up: DVec3) -> DVec3 {
        let v = self.arm_hand;
        fwd * v[0] + right * v[1] + up * v[2]
    }

    /// The way from the hand's end to the shoulder in the eye's frame, a unit vector.
    pub fn way(&self, fwd: DVec3, right: DVec3, up: DVec3) -> DVec3 {
        let v = self.arm_way;
        (fwd * v[0] + right * v[1] + up * v[2]).normalize()
    }

    /// The swing `sw` (0 at rest, 1 at its peak) in the eye's frame: the hand thrust toward the aim (a point
    /// ahead on the look) and chopped down about itself.
    pub fn turn(&self, sw: f64, fwd: DVec3, right: DVec3, up: DVec3) -> DMat4 {
        let hand_at = self.hand_at(fwd, right, up);
        let aim = fwd * self.arm_hand[0] * self.aim;
        DMat4::from_translation(hand_at + (aim - hand_at) * self.reach * sw) * DMat4::from_axis_angle(right, -self.chop * sw) * DMat4::from_translation(-hand_at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> Hand {
        Hand::parse(HAND_TOML).unwrap()
    }

    /// The constants the client had before the file (ADR 0115).
    #[test]
    fn the_shipped_file_is_the_old_constants() {
        let h = shipped();
        assert_eq!(h, Hand { seconds: 0.25, snap: 2.0, reach: 0.45, aim: 1.4, chop: 0.35, arm_hand: [0.55, 0.30, -0.30], arm_way: [-0.75, 0.5, -0.45] });
        assert_eq!(HAND.now(), &h);
    }

    #[test]
    fn a_value_out_of_range_is_refused_by_name() {
        let e = Hand::parse(&HAND_TOML.replace("snap = 2.0", "snap = 9")).unwrap_err();
        assert_eq!(e, "data/hand.toml: snap = 9 is outside 1..6");
        let e = Hand::parse(&HAND_TOML.replace("seconds = 0.25", "seconds = 0.01")).unwrap_err();
        assert!(e.contains("seconds = 0.01"), "{e}");
        assert!(Hand::parse(&format!("{HAND_TOML}\nwobble = 1\n")).unwrap_err().contains("wobble"));
    }

    #[test]
    fn write_round_trips() {
        let mut h = shipped();
        assert_eq!(Hand::parse(&h.write()).unwrap(), h);
        h.set("chop", 0.5);
        assert_eq!(h.get("chop"), 0.5);
        assert_eq!(Hand::parse(&h.write()).unwrap(), h);
        assert!(h.write().contains("chop = 0.5\n"), "{}", h.write());
    }

    #[test]
    fn snap_two_is_the_old_curve() {
        let h = shipped();
        let old = |age: f64| {
            let x = age / 0.25;
            if !(0.0..1.0).contains(&x) {
                return 0.0;
            }
            (std::f64::consts::PI * (1.0 - (1.0 - x) * (1.0 - x))).sin() as f32
        };
        for age in [-1.0, 0.0, 0.01, 0.05, 0.073, 0.1, 0.125, 0.2, 0.249, 0.25, 9.0] {
            assert!((h.curve(age) - old(age)).abs() < 1e-6, "at {age}: {} against {}", h.curve(age), old(age));
        }
        assert!(h.curve(0.1) > 0.5, "a swing, not nothing");
    }

    #[test]
    fn a_larger_snap_peaks_earlier() {
        let peak = |h: &Hand| (0..1000).map(|k| h.seconds * k as f64 / 1000.0).max_by(|a, b| h.curve(*a).total_cmp(&h.curve(*b))).unwrap();
        let mut h = shipped();
        let at2 = peak(&h);
        h.snap = 4.0;
        assert!(peak(&h) < at2 - 0.01, "{} then {at2}", peak(&h));
    }

    /// The old matrix, as models.rs built it before the file.
    #[test]
    fn turn_is_the_old_matrix() {
        let h = shipped();
        let (fwd, up) = (DVec3::new(0.3, -0.2, 0.9).normalize(), DVec3::Y);
        let right = fwd.cross(up).normalize();
        let up = right.cross(fwd);
        let hand_at = fwd * 0.55 + right * 0.30 + up * -0.30;
        assert!((h.hand_at(fwd, right, up) - hand_at).length() < 1e-12);
        assert!((h.way(fwd, right, up) - (fwd * -0.75 + right * 0.5 + up * -0.45).normalize()).length() < 1e-12);
        for sw in [0.0, 0.4, 1.0] {
            let aim = fwd * 0.55 * 1.4;
            let old = DMat4::from_translation(hand_at + (aim - hand_at) * 0.45 * sw) * DMat4::from_axis_angle(right, -0.35 * sw) * DMat4::from_translation(-hand_at);
            assert!(h.turn(sw, fwd, right, up).abs_diff_eq(old, 1e-12), "at {sw}");
        }
        assert!(!h.turn(1.0, fwd, right, up).abs_diff_eq(DMat4::IDENTITY, 1e-3), "a swing moves the hand");
    }
}
