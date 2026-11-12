//! Creatures (docs/design/studio-tools.md "Fauna", ADR 0053): `data/creatures.toml`, one `[[creature]]` row per
//! kind: its model, its body plan, its speeds, which of the model's parts are its legs, arms, head, tail and
//! wings, its sounds and its gait numbers. `walk` and `idle` make ordinary model clips from those, so a walk is
//! never keyed by hand. No GPU and no world: the Studio, the client's stand runs and later the server's creatures
//! call the same code. Nothing spawns creatures yet.
use crate::models::{Clip, Ease, Event, Key, Model};
use serde::{Deserialize, Serialize};

/// The body plans: how the legs (and wings) move against each other.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Plan {
    /// Two legs in opposite phase, the arms against the legs.
    #[default]
    Biped,
    /// Four legs (front left, front right, back left, back right) in diagonal pairs.
    Quadruped,
    /// Two legs as a biped's, two wings beating with the step.
    Bird,
    /// No legs: the tail and the head carry the motion.
    Fish,
}

impl Plan {
    pub const ALL: [(Plan, &'static str); 4] = [(Plan::Biped, "biped"), (Plan::Quadruped, "quadruped"), (Plan::Bird, "bird"), (Plan::Fish, "fish")];
    /// How many legs the plan takes.
    pub fn legs(self) -> usize {
        match self {
            Plan::Biped | Plan::Bird => 2,
            Plan::Quadruped => 4,
            Plan::Fish => 0,
        }
    }
    pub fn name(self) -> &'static str {
        Plan::ALL.iter().find(|p| p.0 == self).unwrap().1
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Creature {
    pub name: String,
    /// A model of `data/models/`, by its file's name.
    pub model: String,
    pub plan: Plan,
    /// How large the model is drawn, about its cell's floor centre.
    pub scale: f32,
    /// Metres a second.
    pub walk_speed: f32,
    pub run_speed: f32,
    /// Metres a foot moves in one step.
    pub step: f32,
    /// Part names: a biped's left then right; a quadruped's front left, front right, back left, back right.
    #[serde(default)]
    pub legs: Vec<String>,
    /// Left then right, swinging against the legs; none or two.
    #[serde(default)]
    pub arms: Vec<String>,
    /// "" for none.
    #[serde(default)]
    pub head: String,
    #[serde(default)]
    pub tail: String,
    /// Left then right; none or two.
    #[serde(default)]
    pub wings: Vec<String>,
    /// Rows of `data/sounds.toml`; "" for none.
    #[serde(default)]
    pub step_sound: String,
    #[serde(default)]
    pub voice: String,
    /// Degrees a leg swings each way from hanging straight, at walking speed.
    pub swing: f32,
    /// Voxels the body dips at the end of each stride (twice a cycle).
    pub bob: f32,
    /// Degrees the head turns each way.
    pub sway: f32,
    /// Degrees the tail turns each way.
    pub wag: f32,
}

/// Every number's range, for the checks and the Studio's sliders: (name, min, max, unit, group).
pub const RANGES: [(&str, f32, f32, &str, &str); 8] = [
    ("scale", 0.25, 4.0, "x", "Body"),
    ("walk_speed", 0.1, 10.0, "m/s", "Body"),
    ("run_speed", 0.1, 25.0, "m/s", "Body"),
    ("step", 0.05, 3.0, "m", "Body"),
    ("swing", 0.0, 90.0, "deg", "Gait"),
    ("bob", 0.0, 4.0, "voxels", "Gait"),
    ("sway", 0.0, 45.0, "deg", "Gait"),
    ("wag", 0.0, 90.0, "deg", "Gait"),
];

/// Keys a cycle of the walk has (and one more, the first again, so it loops without a jump).
const KEYS: usize = 16;

impl Creature {
    pub fn get(&self, name: &str) -> f32 {
        match name {
            "scale" => self.scale,
            "walk_speed" => self.walk_speed,
            "run_speed" => self.run_speed,
            "step" => self.step,
            "swing" => self.swing,
            "bob" => self.bob,
            "sway" => self.sway,
            "wag" => self.wag,
            _ => 0.0,
        }
    }
    pub fn set(&mut self, name: &str, v: f32) {
        match name {
            "scale" => self.scale = v,
            "walk_speed" => self.walk_speed = v,
            "run_speed" => self.run_speed = v,
            "step" => self.step = v,
            "swing" => self.swing = v,
            "bob" => self.bob = v,
            "sway" => self.sway = v,
            "wag" => self.wag = v,
            _ => {}
        }
    }

    /// The row checked against its model and the sounds that exist; the refusal says why.
    pub fn check(&self, model: Option<&Model>, sound: &dyn Fn(&str) -> bool) -> Result<(), String> {
        for (n, lo, hi, _, _) in RANGES {
            let v = self.get(n);
            if !(lo..=hi).contains(&v) {
                return Err(format!("{n} = {v} is outside {lo}..{hi}"));
            }
        }
        if self.run_speed < self.walk_speed {
            return Err(format!("run_speed {} is below walk_speed {}", self.run_speed, self.walk_speed));
        }
        let m = model.ok_or(format!("no model '{}' in data/models", self.model))?;
        if self.legs.len() != self.plan.legs() {
            return Err(format!("a {} has {} legs, the row names {}", self.plan.name(), self.plan.legs(), self.legs.len()));
        }
        for (what, list) in [("arms", &self.arms), ("wings", &self.wings)] {
            if !matches!(list.len(), 0 | 2) {
                return Err(format!("{what}: none or two, the row names {}", list.len()));
            }
        }
        let parts = self.legs.iter().chain(&self.arms).chain(&self.wings).chain([&self.head, &self.tail]).filter(|p| !p.is_empty());
        for p in parts {
            if !m.parts.iter().any(|q| &q.name == p) {
                return Err(format!("model '{}' has no part '{p}'", self.model));
            }
        }
        for s in [&self.step_sound, &self.voice] {
            if !s.is_empty() && !sound(s) {
                return Err(format!("no sound '{s}' in data/sounds.toml"));
            }
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    creature: Vec<Creature>,
}

/// Reads and checks `data/creatures.toml`: `model` gives a model by name, `sound` says which sounds exist. A
/// refusal names `file`, the row and why.
pub fn parse(file: &str, text: &str, model: &dyn Fn(&str) -> Option<Model>, sound: &dyn Fn(&str) -> bool) -> Result<Vec<Creature>, String> {
    let f: File = toml::from_str(text).map_err(|e| format!("{file}: {e}"))?;
    for (i, c) in f.creature.iter().enumerate() {
        if f.creature[..i].iter().any(|d| d.name == c.name) {
            return Err(format!("{file}: creature '{}' is named twice", c.name));
        }
        c.check(model(&c.model).as_ref(), sound).map_err(|e| format!("{file}: creature '{}': {e}", c.name))?;
    }
    Ok(f.creature)
}

/// The file's text of these rows (parse it again to check it).
pub fn write(rows: &[Creature]) -> String {
    let text = toml::to_string(&File { creature: rows.to_vec() }).expect("creatures always serialise");
    // each number as its f32 prints (1.2, not 1.2000000476837158), so a saved row reads as it was typed
    let line = |l: &str| match l.split_once(" = ").and_then(|(k, v)| Some((k, v.parse::<f32>().ok()?))) {
        Some((k, v)) => format!("{k} = {v:?}\n"),
        None => format!("{l}\n"),
    };
    text.lines().map(line).collect()
}

/// One row as `[[creature]]` text, for rewriting just that row of the file.
pub fn write_row(c: &Creature) -> String {
    write(std::slice::from_ref(c))
}

/// The first root part: the body that bobs.
fn body(m: &Model) -> Option<&str> {
    m.parts.iter().find(|p| p.parent.is_empty()).map(|p| p.name.as_str())
}

/// The walk at `speed` m/s as an ordinary looping clip named "walk": one cycle is two steps, so it lasts
/// 2 x step / speed and the feet keep pace with the ground. A speed under 1 cm/s (or not a number), or a step
/// not over 0, gives the idle; a speed over run_speed's top (25 m/s) walks at that. The row need not be checked
/// (the Studio walks the row being edited): legs not matching the plan do not swing.
pub fn walk(m: &Model, c: &Creature, speed: f32) -> Clip {
    let Some(length) = cycle(c, speed) else { return idle(m, c) };
    let speed = speed.min(25.0);
    // a faster gait swings wider, a slower one narrower (at walking speed: the row's swing)
    let amp = c.swing * (speed / c.walk_speed).sqrt().clamp(0.3, 1.6);
    // (part, phase in cycles, turn axis 0..3, amplitude, sign)
    let mut swings: Vec<(&str, f32, usize, f32)> = Vec::new();
    let legs: Vec<&str> = c.legs.iter().map(String::as_str).collect();
    match c.plan {
        _ if legs.len() != c.plan.legs() => {}
        Plan::Biped | Plan::Bird => {
            swings.extend([(legs[0], 0.0, 0, amp), (legs[1], 0.5, 0, amp)]);
        }
        Plan::Quadruped => {
            // the diagonal pairs: front left with back right, front right with back left
            swings.extend([(legs[0], 0.0, 0, amp), (legs[3], 0.0, 0, amp), (legs[1], 0.5, 0, amp), (legs[2], 0.5, 0, amp)]);
        }
        Plan::Fish => {}
    }
    if c.arms.len() == 2 {
        swings.extend([(c.arms[0].as_str(), 0.5, 0, amp * 0.8), (c.arms[1].as_str(), 0.0, 0, amp * 0.8)]);
    }
    if c.wings.len() == 2 {
        // the wings beat about the body's length, mirrored, twice a cycle
        // (at two beats a cycle, half a beat is a quarter cycle: 0.5 would put both wings in step)
        swings.extend([(c.wings[0].as_str(), 0.0, 2, amp * 0.5), (c.wings[1].as_str(), 0.25, 2, amp * 0.5)]);
    }
    let mut key = Vec::new();
    for i in 0..=KEYS {
        let f = i as f32 / KEYS as f32;
        let t = f * length;
        let wave = |phase: f32, beats: f32| ((f + phase) * beats * std::f32::consts::TAU).sin();
        let turn = |part: &str, axis: usize, deg: f32, key: &mut Vec<Key>| {
            let mut r = [0.0; 3];
            r[axis] = deg;
            key.push(Key { part: part.into(), t, shift: [0.0; 3], turn: r, ease: Ease::Linear });
        };
        for &(p, phase, axis, a) in &swings {
            let beats = if axis == 2 { 2.0 } else { 1.0 };
            turn(p, axis, a * wave(phase, beats), &mut key);
        }
        if !c.head.is_empty() {
            turn(&c.head, 1, c.sway * wave(0.25, 1.0), &mut key);
        }
        if !c.tail.is_empty() {
            turn(&c.tail, 1, c.wag * wave(0.75, 1.0), &mut key);
        }
        // the body lowest where the legs reach furthest (a quarter and three quarters in), so the feet stay down
        if let Some(b) = body(m).filter(|b| !swings.iter().any(|s| s.0 == *b)) {
            let dip = -c.bob * 0.5 * (1.0 - (f * 2.0 * std::f32::consts::TAU).cos());
            key.push(Key { part: b.into(), t, shift: [0.0, dip, 0.0], turn: [0.0; 3], ease: Ease::Linear });
        }
    }
    snap(&mut key, length);
    Clip { name: "walk".into(), length, looping: true, key }
}

/// The walk's footfalls at `speed` (as `walk` makes it) as clip events playing the row's step sound: one a step,
/// where the legs reach furthest and the body is lowest (a quarter and three quarters into the cycle; a
/// quadruped's diagonal pair lands as one). None for the idle, a plan without legs or no step sound.
pub fn steps(c: &Creature, speed: f32) -> Vec<Event> {
    match cycle(c, speed) {
        Some(length) if !c.step_sound.is_empty() && c.plan.legs() > 0 && c.legs.len() == c.plan.legs() => [0.25, 0.75].map(|f| Event { t: f * length, sound: Some(c.step_sound.clone()), ..Event::default() }).into(),
        _ => Vec::new(),
    }
}

/// A walk cycle's length at `speed`, None for the idle (see `walk`).
fn cycle(c: &Creature, speed: f32) -> Option<f32> {
    (speed >= 0.01 && c.step > 0.0).then(|| 2.0 * c.step / speed.min(25.0))
}

/// Standing: the body breathes, the head looks about and the tail moves a little, over 3 s; the legs rest.
pub fn idle(m: &Model, c: &Creature) -> Clip {
    let length = 3.0;
    let mut key = Vec::new();
    for i in 0..=KEYS {
        let f = i as f32 / KEYS as f32;
        let t = f * length;
        let wave = |phase: f32| ((f + phase) * std::f32::consts::TAU).sin();
        if !c.head.is_empty() {
            key.push(Key { part: c.head.clone(), t, shift: [0.0; 3], turn: [0.0, c.sway * 0.4 * wave(0.0), 0.0], ease: Ease::Linear });
        }
        if !c.tail.is_empty() {
            key.push(Key { part: c.tail.clone(), t, shift: [0.0; 3], turn: [0.0, c.wag * 0.3 * wave(0.5), 0.0], ease: Ease::Linear });
        }
        if let Some(b) = body(m).filter(|b| !c.legs.iter().any(|l| l == b)) {
            let breathe = c.bob * 0.15 * (1.0 - (f * std::f32::consts::TAU).cos());
            key.push(Key { part: b.into(), t, shift: [0.0, breathe, 0.0], turn: [0.0; 3], ease: Ease::Linear });
        }
    }
    snap(&mut key, length);
    Clip { name: "idle".into(), length, looping: true, key }
}

/// The last keys' times exactly the length (f32 sums can land a hair past it, which the model's check refuses).
fn snap(key: &mut [Key], length: f32) {
    for k in key {
        k.t = k.t.min(length);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::sample;
    use crate::palette::Palette;

    fn palette() -> Palette {
        Palette::parse("[[colour]]\nname = \"stone\"\nrgb = \"#808080\"\n").unwrap()
    }

    fn part(name: &str, parent: &str) -> String {
        format!("[[part]]\nname = \"{name}\"\nparent = \"{parent}\"\nat = [0, 0, 0]\npivot = [0, 1, 0]\nbox = [1, 1, 1]\nvoxels = \"a\"\n\n")
    }

    fn model(parts: &[(&str, &str)]) -> Model {
        let text = format!("colours = [\"stone\"]\n\n{}", parts.iter().map(|(n, p)| part(n, p)).collect::<String>());
        Model::parse("test.toml", &text, &palette()).unwrap()
    }

    fn boar() -> Model {
        model(&[("body", ""), ("head", "body"), ("fl", "body"), ("fr", "body"), ("bl", "body"), ("br", "body"), ("tail", "body")])
    }

    fn row(plan: &str, legs: &str, extra: &str) -> String {
        format!(
            "[[creature]]\nname = \"boar\"\nmodel = \"boar\"\nplan = \"{plan}\"\nscale = 1.0\nwalk_speed = 1.2\nrun_speed = 4.0\nstep = 0.4\nlegs = [{legs}]\n{extra}step_sound = \"step.dirt\"\nvoice = \"\"\nswing = 30.0\nbob = 0.5\nsway = 10.0\nwag = 20.0\n"
        )
    }
    const FOUR: &str = "\"fl\", \"fr\", \"bl\", \"br\"";

    fn read(text: &str) -> Result<Vec<Creature>, String> {
        parse("data/creatures.toml", text, &|n| (n == "boar").then(boar), &|s| s == "step.dirt")
    }

    #[test]
    fn a_row_reads_and_writes_byte_exact() {
        let rows = read(&row("quadruped", FOUR, "head = \"head\"\ntail = \"tail\"\n")).unwrap();
        assert_eq!(rows[0].plan, Plan::Quadruped);
        assert_eq!(rows[0].legs.len(), 4);
        let w = write(&rows);
        // numbers as a person writes them, not an f32 widened to f64's digits
        assert!(w.contains("walk_speed = 1.2\n") && w.contains("bob = 0.5\n"), "{w}");
        assert_eq!(read(&w).unwrap(), rows);
        assert_eq!(write(&read(&w).unwrap()), w);
    }

    #[test]
    fn refusals_name_the_row_and_why() {
        let e = read(&row("quadruped", FOUR, "").replace("model = \"boar\"", "model = \"pig\"")).unwrap_err();
        assert!(e.contains("creature 'boar'") && e.contains("no model 'pig'"), "{e}");
        let e = read(&row("quadruped", "\"fl\", \"fr\", \"bl\", \"hoof\"", "")).unwrap_err();
        assert!(e.contains("no part 'hoof'"), "{e}");
        let e = read(&row("quadruped", FOUR, "").replace("swing = 30.0", "swing = 200.0")).unwrap_err();
        assert!(e.contains("swing = 200 is outside 0..90"), "{e}");
        let e = read(&row("biped", FOUR, "")).unwrap_err();
        assert!(e.contains("a biped has 2 legs"), "{e}");
        let e = read(&row("quadruped", FOUR, "").replace("step.dirt", "step.moon")).unwrap_err();
        assert!(e.contains("no sound 'step.moon'"), "{e}");
        let e = read(&row("quadruped", FOUR, "").replace("run_speed = 4.0", "run_speed = 1.0")).unwrap_err();
        assert!(e.contains("below walk_speed"), "{e}");
    }

    fn quad() -> (Model, Creature) {
        (boar(), read(&row("quadruped", FOUR, "head = \"head\"\ntail = \"tail\"\n")).unwrap().remove(0))
    }

    #[test]
    fn the_cycle_keeps_pace_with_the_ground() {
        let (m, c) = quad();
        for speed in [0.5, 1.2, 4.0] {
            let w = walk(&m, &c, speed);
            assert!((w.length - 2.0 * c.step / speed).abs() < 1e-6, "{} at {speed}", w.length);
            assert!(w.looping);
        }
    }

    #[test]
    fn diagonal_pairs_swing_together_and_against_the_others() {
        let (m, c) = quad();
        let w = walk(&m, &c, 1.2);
        let mut moved = 0.0f32;
        for i in 0..40 {
            let t = i as f32 / 40.0 * w.length;
            let x = |p: &str| sample(&w, p, t).1.x;
            assert!((x("fl") - x("br")).abs() < 1e-4, "front left and back right together at {t}");
            assert!((x("fl") + x("fr")).abs() < 1e-4, "front left against front right at {t}");
            assert!((x("bl") + x("br")).abs() < 1e-4, "back left against back right at {t}");
            moved = moved.max(x("fl").abs());
        }
        assert!(moved > 20.0, "the legs swing ({moved} deg)");
    }

    #[test]
    fn a_bipeds_legs_are_opposite_and_its_arms_against_them() {
        let m = model(&[("body", ""), ("head", "body"), ("arm_l", "body"), ("arm_r", "body"), ("leg_l", "body"), ("leg_r", "body")]);
        let text = row("biped", "\"leg_l\", \"leg_r\"", "arms = [\"arm_l\", \"arm_r\"]\nhead = \"head\"\n").replace("model = \"boar\"", "model = \"ape\"");
        let c = parse("c.toml", &text, &|n| (n == "ape").then(|| m.clone()), &|_| true).unwrap().remove(0);
        let w = walk(&m, &c, 1.0);
        for i in 0..20 {
            let t = i as f32 / 20.0 * w.length;
            let x = |p: &str| sample(&w, p, t).1.x;
            assert!((x("leg_l") + x("leg_r")).abs() < 1e-4);
            assert!(x("leg_l") * x("arm_l") <= 1e-6, "the left arm swings against the left leg");
        }
        assert!(sample(&w, "leg_l", w.length / 4.0).1.x.abs() > 15.0, "the legs swing");
    }

    #[test]
    fn the_clip_loops_without_a_jump_and_the_model_takes_it() {
        let (mut m, c) = quad();
        for clip in [walk(&m, &c, 1.7), idle(&m, &c)] {
            for p in ["body", "head", "fl", "fr", "bl", "br", "tail"] {
                let (a, b) = (sample(&clip, p, 0.0), sample(&clip, p, clip.length));
                assert!((a.0 - b.0).length() < 1e-3 && (a.1 - b.1).length() < 1e-3, "{} of {p}: {a:?} then {b:?}", clip.name);
            }
            m.clips.push(clip);
        }
        // the model's own reader takes the clips (every key on a part, inside the length)
        let p = palette();
        let again = Model::parse("t.toml", &m.write(), &p).unwrap();
        assert_eq!(again.clips.len(), 2);
        // the body dips, never rises, in the walk
        let w = &again.clips[0];
        let low = (0..32).map(|i| sample(w, "body", i as f32 / 32.0 * w.length).0.y).fold(0.0f32, f32::min);
        assert!(low < -0.4 && (0..32).all(|i| sample(w, "body", i as f32 / 32.0 * w.length).0.y <= 1e-6));
    }

    #[test]
    fn a_birds_wings_beat_mirrored() {
        let m = model(&[("body", ""), ("leg_l", "body"), ("leg_r", "body"), ("wing_l", "body"), ("wing_r", "body")]);
        let text = row("bird", "\"leg_l\", \"leg_r\"", "wings = [\"wing_l\", \"wing_r\"]\n").replace("model = \"boar\"", "model = \"bird\"");
        let c = parse("c.toml", &text, &|n| (n == "bird").then(|| m.clone()), &|_| true).unwrap().remove(0);
        let w = walk(&m, &c, 1.0);
        let mut most = 0.0f32;
        for i in 0..20 {
            let t = i as f32 / 20.0 * w.length;
            let z = |p: &str| sample(&w, p, t).1.z;
            // the same turn about the body's length on both would roll the bird, not flap it
            assert!((z("wing_l") + z("wing_r")).abs() < 1e-4, "at {t}: {} and {}", z("wing_l"), z("wing_r"));
            most = most.max(z("wing_l").abs());
        }
        assert!(most > 5.0, "the wings beat ({most} deg)");
    }

    #[test]
    fn a_missing_optional_part_is_fine_and_the_walk_is_the_same_each_time() {
        let (m, mut c) = quad();
        (c.head, c.tail) = (String::new(), String::new());
        c.check(Some(&m), &|_| true).unwrap();
        let w = walk(&m, &c, 1.0);
        assert!(w.key.iter().all(|k| k.part != "head" && k.part != "tail"));
        assert_eq!(w, walk(&m, &c, 1.0));
        assert_eq!(walk(&m, &c, 0.0).name, "idle");
    }

    #[test]
    fn an_unchecked_row_or_a_wild_speed_never_panics_or_gives_nan() {
        // the Studio walks the row being edited: the plan switched to four legs while it still names two
        let (m, mut c) = quad();
        c.legs.truncate(2);
        let w = walk(&m, &c, 1.0);
        assert!(w.length.is_finite() && w.length > 0.0);
        // `creature=boar:inf`, `:nan`, a hair over 0, or a step edited to 0: a clip of finite, positive length
        let (m, mut c) = quad();
        for speed in [f32::INFINITY, f32::NAN, 1e-45, 1e9] {
            let w = walk(&m, &c, speed);
            assert!(w.length.is_finite() && w.length > 0.0 && w.key.iter().all(|k| k.t.is_finite()), "{speed}: {}", w.length);
        }
        c.step = 0.0;
        let w = walk(&m, &c, 1.0);
        assert!(w.length.is_finite() && w.length > 0.0, "step 0: {}", w.length);
    }

    #[test]
    fn the_footfalls_fall_where_the_feet_are_down() {
        let m2 = model(&[("body", ""), ("leg_l", "body"), ("leg_r", "body")]);
        let text = row("biped", "\"leg_l\", \"leg_r\"", "").replace("model = \"boar\"", "model = \"ape\"");
        let biped = (m2.clone(), parse("c.toml", &text, &|n| (n == "ape").then(|| m2.clone()), &|_| true).unwrap().remove(0));
        for (m, c) in [quad(), biped] {
            let w = walk(&m, &c, 1.0);
            let ev = steps(&c, 1.0);
            assert_eq!(ev.len(), 2, "{:?}: one a step, two a cycle", c.plan);
            let reach = |p: &str| (0..64).map(|i| sample(&w, p, i as f32 / 64.0 * w.length).1.x.abs()).fold(0.0f32, f32::max);
            for e in &ev {
                assert_eq!(e.sound.as_deref(), Some("step.dirt"));
                assert!((0.0..w.length).contains(&e.t));
                assert!((sample(&w, "body", e.t).0.y + c.bob).abs() < 1e-3, "{:?}: the body lowest at {}", c.plan, e.t);
                for l in &c.legs {
                    assert!((sample(&w, l, e.t).1.x.abs() - reach(l)).abs() < 1e-2, "{:?}: {l} at full reach at {}", c.plan, e.t);
                }
            }
        }
        let (_, mut c) = quad();
        assert!(steps(&c, 0.0).is_empty(), "standing: no steps");
        c.step_sound.clear();
        assert!(steps(&c, 1.0).is_empty(), "no step sound: no events");
    }

    #[test]
    fn the_shipped_creatures_read() {
        let p = Palette::parse(include_str!("../../../data/palette.toml")).unwrap();
        let model = |n: &str| std::fs::read_to_string(format!("../../data/models/{n}.toml")).ok().and_then(|t| Model::parse(n, &t, &p).ok());
        let rows = parse("data/creatures.toml", include_str!("../../../data/creatures.toml"), &model, &|_| true).unwrap();
        assert!(rows.iter().any(|c| c.plan == Plan::Quadruped) && rows.iter().any(|c| c.plan == Plan::Biped));
    }
}
