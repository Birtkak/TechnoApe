//! The Effects page's logic without a window (docs/design/studio-tools.md "Effects", ADR 0050): a row of
//! `data/effects.toml` as its keys, its numbers by group with their ranges, the gradient's stops and the
//! curves' points with their edits and ready shapes, and the variations' generator. The game's `fx` reads and
//! checks what this writes.
use crate::rows::{num, Keys};
use crate::wall::Rng;

/// How a number is laid out in a row.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Form {
    One,
    /// `[from, to]`: the first or second.
    Pair(usize),
    /// `noise = { strength, scale, speed }`: one of them.
    Noise(&'static str),
    /// `box = [x, y, z]` or `dir = [x, y, z]`: one of them.
    Three(usize),
}

/// One slider of the inspector: (group, key, which part, label, unit, min, max, what it does).
pub struct Field {
    pub group: &'static str,
    pub key: &'static str,
    pub form: Form,
    pub label: &'static str,
    pub unit: &'static str,
    pub min: f32,
    pub max: f32,
    pub help: &'static str,
}
const fn fd(group: &'static str, key: &'static str, form: Form, label: &'static str, unit: &'static str, min: f32, max: f32, help: &'static str) -> Field {
    Field { group, key, form, label, unit, min, max, help }
}

pub const GROUPS: [&str; 5] = ["Start", "Motion", "Life", "Look", "Light"];

/// Every number of a row the inspector shows, by group, with sensible ranges (the game's own checks still
/// apply on top).
pub const FIELDS: &[Field] = &[
    fd("Start", "count", Form::One, "burst", "", 0.0, 128.0, "particles at once when it starts"),
    fd("Start", "rate", Form::One, "rate", "/s", 0.0, 200.0, "particles a second for as long as it plays"),
    fd("Start", "radius", Form::One, "radius", "m", 0.01, 2.0, "the ring's size (start = ring)"),
    fd("Start", "angle", Form::One, "angle", "°", 0.0, 180.0, "how wide the cone opens (start = cone)"),
    fd("Start", "box", Form::Three(0), "box x", "m", 0.01, 2.0, "the box's width (start = box)"),
    fd("Start", "box", Form::Three(1), "box up", "m", 0.01, 2.0, "the box's height (start = box)"),
    fd("Start", "box", Form::Three(2), "box z", "m", 0.01, 2.0, "the box's depth (start = box)"),
    fd("Start", "dir", Form::Three(0), "dir x", "", -1.0, 1.0, "the cone's axis: sideways (start = cone)"),
    fd("Start", "dir", Form::Three(1), "dir up", "", -1.0, 1.0, "the cone's axis: up (start = cone)"),
    fd("Start", "dir", Form::Three(2), "dir z", "", -1.0, 1.0, "the cone's axis: the other way sideways (start = cone)"),
    fd("Motion", "speed", Form::Pair(0), "speed from", "m/s", 0.0, 10.0, "the slowest a particle flies off"),
    fd("Motion", "speed", Form::Pair(1), "speed to", "m/s", 0.0, 10.0, "the fastest"),
    fd("Motion", "up", Form::One, "up", "m/s", -5.0, 10.0, "more speed upwards at the start"),
    fd("Motion", "gravity", Form::One, "gravity", "×", -1.0, 2.0, "times the planet's pull"),
    fd("Motion", "drag", Form::One, "drag", "/s", 0.0, 10.0, "how fast the air slows it"),
    fd("Motion", "rise", Form::One, "rise", "m/s²", -2.0, 5.0, "a steady lift: a plume"),
    fd("Motion", "spread", Form::One, "spread", "m/s²", 0.0, 2.0, "each particle pushed its own way sideways: a plume widens"),
    fd("Motion", "noise", Form::Noise("strength"), "noise", "m/s²", 0.0, 8.0, "a push through a moving field: flames lick, smoke curls"),
    fd("Motion", "noise", Form::Noise("scale"), "noise grain", "m", 0.05, 4.0, "how large the field's swirls are"),
    fd("Motion", "noise", Form::Noise("speed"), "noise speed", "/s", 0.0, 5.0, "how fast the field moves"),
    fd("Motion", "spin", Form::One, "spin", "rad/s", 0.0, 20.0, "how fast it turns at most"),
    fd("Life", "life", Form::Pair(0), "life from", "s", 0.05, 10.0, "the shortest life"),
    fd("Life", "life", Form::Pair(1), "life to", "s", 0.05, 10.0, "the longest"),
    fd("Life", "grow", Form::One, "grow", "share", 0.0, 1.0, "the first share of the life it grows over"),
    fd("Life", "shrink", Form::One, "shrink", "share", 0.0, 1.0, "the last share of the life it shrinks to nothing over"),
    fd("Look", "size", Form::Pair(0), "size from", "m", 0.005, 2.0, "the smallest"),
    fd("Look", "size", Form::Pair(1), "size to", "m", 0.005, 2.0, "the largest"),
    fd("Look", "colour", Form::Three(0), "red", "", 0.0, 1.0, "the colour's red (a square or a texture without a gradient)"),
    fd("Look", "colour", Form::Three(1), "green", "", 0.0, 1.0, "the colour's green"),
    fd("Look", "colour", Form::Three(2), "blue", "", 0.0, 1.0, "the colour's blue"),
    fd("Light", "glow", Form::One, "glow", "×", 0.0, 8.0, "the light it gives from its colour; a lasting one is a lamp"),
];

/// The choices a row names by word: (key, its words, what it does).
pub const CHOICES: &[(&str, &str, &[&str], &str)] = &[
    ("Start", "start", &["point", "block", "faces", "ring", "cone", "box"], "where the particles start"),
    ("Life", "ground", &["none", "stop", "bounce", "die"], "what a particle does on the ground"),
    ("Look", "look", &["square", "debris", "block", "texture", "model"], "what a particle is"),
    ("Look", "face", &["camera", "flight", "tumble"], "how it lies: facing you, along its flight, or tumbling"),
];

/// A ready shape of a curve over a particle's life: (name, help, points).
pub const SHAPES: [(&str, &str, &[[f32; 2]]); 5] = [
    ("grow", "starts small, ends full", &[[0.0, 0.3], [1.0, 1.0]]),
    ("shrink", "starts full, ends at nothing", &[[0.0, 1.0], [1.0, 0.0]]),
    ("pulse", "swells in the middle", &[[0.0, 0.5], [0.5, 1.0], [1.0, 0.5]]),
    ("fade in and out", "from nothing, full, to nothing", &[[0.0, 0.0], [0.2, 1.0], [0.8, 1.0], [1.0, 0.0]]),
    ("even", "the same all its life", &[[0.0, 1.0], [1.0, 1.0]]),
];
/// The highest a curve's point goes on the page.
pub const CURVE_MAX: f32 = 3.0;

fn f(v: &toml::Value) -> Option<f32> {
    v.as_float().map(|f| f as f32).or(v.as_integer().map(|i| i as f32))
}

/// A row's number for a field, or its default when the row leaves it out.
pub fn get(t: &toml::Table, fl: &Field) -> Option<f32> {
    let v = t.get(fl.key);
    match fl.form {
        Form::One => v.and_then(f).or(Some(match fl.key {
            "gravity" => 1.0,
            _ => 0.0,
        })),
        Form::Pair(i) => v.and_then(|v| v.as_array()).and_then(|a| a.get(i)).and_then(f),
        Form::Three(i) => v.and_then(|v| v.as_array()).and_then(|a| a.get(i)).and_then(f).or(three(fl.key).map(|d| d[i])),
        Form::Noise(n) => v.and_then(|v| v.as_table()).and_then(|t| t.get(n)).and_then(f),
    }
}

/// What a row without the three numbers means (none: a box is there only when it is its start).
fn three(key: &str) -> Option<[f32; 3]> {
    match key {
        "dir" => Some([0.0, 1.0, 0.0]),
        "colour" => Some([1.0; 3]),
        _ => None,
    }
}

/// Sets a field's number in the row's keys (a noise or box made whole when it was not there).
pub fn set(k: &mut Keys, fl: &Field, v: f32) {
    let t = k.table().unwrap_or_default();
    let v = v.clamp(fl.min, fl.max);
    let text = match fl.form {
        Form::One => num(if fl.key == "count" { v.round() } else { v }),
        Form::Pair(i) => {
            let mut p = [0.0, 0.0];
            for (j, x) in p.iter_mut().enumerate() {
                *x = t.get(fl.key).and_then(|v| v.as_array()).and_then(|a| a.get(j)).and_then(f).unwrap_or(v);
            }
            p[i] = v;
            // from never past to
            if i == 0 {
                p[1] = p[1].max(v);
            } else {
                p[0] = p[0].min(v);
            }
            format!("[{}, {}]", num(p[0]), num(p[1]))
        }
        Form::Three(i) => {
            let mut p = three(fl.key).unwrap_or([0.3, 0.1, 0.3]);
            for (j, x) in p.iter_mut().enumerate() {
                if let Some(o) = t.get(fl.key).and_then(|v| v.as_array()).and_then(|a| a.get(j)).and_then(f) {
                    *x = o;
                }
            }
            p[i] = v;
            // the game refuses a cone's axis of no length: the slider stops short of it
            if fl.key == "dir" && p.iter().map(|x| x * x).sum::<f32>() < 1e-4 {
                return;
            }
            format!("[{}, {}, {}]", num(p[0]), num(p[1]), num(p[2]))
        }
        Form::Noise(n) => {
            let mut z = [("strength", 0.0), ("scale", 1.0), ("speed", 0.0)];
            for (name, x) in z.iter_mut() {
                if let Some(o) = t.get("noise").and_then(|v| v.as_table()).and_then(|t| t.get(*name)).and_then(f) {
                    *x = o;
                }
                if *name == n {
                    *x = v;
                }
            }
            format!("{{ {} }}", z.iter().map(|(a, b)| format!("{a} = {}", num(*b))).collect::<Vec<_>>().join(", "))
        }
    };
    k.set(fl.key, Some(text));
}

/// A choice set; `start` brings its own numbers and drops the others' (the game takes each only with its start).
/// `look` set, and whatever the game takes with it only: `model` or `texture` names a file (the one it named,
/// else `name`), a gradient and glow only on squares and textures, a face not on debris or a block, a square a
/// colour (white) when it has no gradient.
pub fn choose_look(k: &mut Keys, word: &str, name: Option<&str>) {
    k.set("look", Some(format!("\"{word}\"")));
    for key in ["model", "texture"] {
        let keep = (key == word).then(|| k.get(key).map(String::from).or(name.map(|n| format!("\"{n}\"")))).flatten();
        k.set(key, keep);
    }
    if !matches!(word, "square" | "texture") {
        k.set("gradient", None);
        k.set("glow", None);
    }
    if matches!(word, "debris" | "block") {
        k.set("face", None);
    }
    if word == "square" && k.get("colour").is_none() && k.get("gradient").is_none() {
        k.set("colour", Some("[1, 1, 1]".into()));
    }
}

/// A key that names a file or a row (`model`, `texture`, `then`: the row played where each particle ends), or none.
pub fn set_name(k: &mut Keys, key: &str, name: Option<&str>) {
    k.set(key, name.map(|t| format!("\"{t}\"")));
}

pub fn choose(k: &mut Keys, key: &str, word: &str) {
    k.set(key, Some(format!("\"{word}\"")));
    if key == "start" {
        for (s, extra, default) in [("ring", "radius", "0.3"), ("cone", "angle", "30"), ("box", "box", "[0.3, 0.1, 0.3]")] {
            k.set(extra, (s == word).then(|| k.get(extra).unwrap_or(default).to_string()));
        }
        if word != "cone" {
            k.set("dir", None);
        }
    }
}

/// A gradient: (share of the life, palette name), in order.
pub type Grad = Vec<(f32, String)>;

pub fn gradient(t: &toml::Table) -> Grad {
    t.get("gradient").and_then(|v| v.as_array()).into_iter().flatten().filter_map(|s| {
        let a = s.as_array()?;
        Some((f(a.first()?)?, a.get(1)?.as_str()?.to_string()))
    }).collect()
}
pub fn write_gradient(g: &Grad) -> Option<String> {
    (!g.is_empty()).then(|| format!("[{}]", g.iter().map(|(t, c)| format!("[{}, \"{c}\"]", num(*t))).collect::<Vec<_>>().join(", ")))
}
/// A stop at `t` of colour `c`, in order; its index.
pub fn add_stop(g: &mut Grad, t: f32, c: &str) -> usize {
    let t = t.clamp(0.0, 1.0);
    let i = g.iter().position(|s| s.0 > t).unwrap_or(g.len());
    g.insert(i, (t, c.into()));
    i
}
/// A stop dragged to `t`, kept between its neighbours; its index stays.
pub fn move_stop(g: &mut Grad, i: usize, t: f32) {
    let lo = if i == 0 { 0.0 } else { g[i - 1].0 };
    let hi = g.get(i + 1).map_or(1.0, |s| s.0);
    if let Some(s) = g.get_mut(i) {
        s.0 = t.clamp(lo, hi);
    }
}
/// Removes a stop; the last one stays (a gradient of none is no gradient).
pub fn remove_stop(g: &mut Grad, i: usize) -> bool {
    (g.len() > 1 && i < g.len()).then(|| g.remove(i)).is_some()
}

pub type Curve = Vec<[f32; 2]>;

pub fn curve(t: &toml::Table, key: &str) -> Curve {
    t.get(key).and_then(|v| v.as_array()).into_iter().flatten().filter_map(|p| {
        let a = p.as_array()?;
        Some([f(a.first()?)?, f(a.get(1)?)?])
    }).collect()
}
pub fn write_curve(c: &Curve) -> Option<String> {
    (!c.is_empty()).then(|| format!("[{}]", c.iter().map(|p| format!("[{}, {}]", num(p[0]), num(p[1]))).collect::<Vec<_>>().join(", ")))
}
/// A point at `x` on the curve as it runs there (no bend); its index.
pub fn add_point(c: &mut Curve, x: f32) -> usize {
    let x = x.clamp(0.0, 1.0);
    let y = client::fx::curve_at(c, x);
    let i = c.iter().position(|p| p[0] > x).unwrap_or(c.len());
    c.insert(i, [x, y]);
    i
}
/// A point dragged, kept between its neighbours in x and in 0..CURVE_MAX.
pub fn move_point(c: &mut Curve, i: usize, x: f32, y: f32) {
    let lo = if i == 0 { 0.0 } else { c[i - 1][0] };
    let hi = c.get(i + 1).map_or(1.0, |p| p[0]);
    if let Some(p) = c.get_mut(i) {
        *p = [x.clamp(lo, hi), y.clamp(0.0, CURVE_MAX)];
    }
}
/// Removes a point; two stay.
pub fn remove_point(c: &mut Curve, i: usize) -> bool {
    (c.len() > 2 && i < c.len()).then(|| c.remove(i)).is_some()
}
pub fn shape(name: &str) -> Option<Curve> {
    SHAPES.iter().find(|s| s.0 == name).map(|s| s.2.to_vec())
}

/// A variation of a row: each number the row has moved by up to `spread` of a fair share of its range.
pub fn vary(k: &Keys, r: &mut Rng, spread: f32) -> Keys {
    let mut out = k.clone();
    let t = k.table().unwrap_or_default();
    for fl in FIELDS.iter().filter(|fl| t.contains_key(fl.key) && fl.key != "count") {
        let Some(v) = get(&out.table().unwrap_or_default(), fl) else { continue };
        let x = r.signed() * spread;
        set(&mut out, fl, if v > 0.0 { v * 2f32.powf(x) } else { v + x * (fl.max - fl.min) * 0.1 });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLAMES: &str = "[[effect]]\nname = \"flames\"\nrate = 45\nstart = \"box\"\nbox = [0.4, 0.05, 0.4]\nspeed = [0, 0.15]\nlife = [0.45, 0.8]\nsize = [0.14, 0.24]\nsize_curve = [[0, 0.6], [0.25, 1], [1, 0.15]]\nlook = \"square\"\ngradient = [[0, \"lamp light\"], [0.6, \"copper\"], [1, \"copper dark\"]]\nglow = 3\nnoise = { strength = 2.5, scale = 0.3, speed = 1.5 }\n";

    fn field(key: &str, form: Form) -> &'static Field {
        FIELDS.iter().find(|f| f.key == key && f.form == form).unwrap()
    }

    #[test]
    fn gradient_stops_add_move_remove_and_write() {
        let k = Keys::parse(FLAMES);
        let mut g = gradient(&k.table().unwrap());
        assert_eq!(g.len(), 3);
        assert_eq!(write_gradient(&g).as_deref(), k.get("gradient"), "read and written back the same");
        assert_eq!(add_stop(&mut g, 0.3, "lamp"), 1);
        assert_eq!(g[1], (0.3, "lamp".into()));
        move_stop(&mut g, 1, 0.9);
        assert_eq!(g[1].0, 0.6, "kept before the next stop");
        move_stop(&mut g, 1, -1.0);
        assert_eq!(g[1].0, 0.0);
        g[1].1 = "stone".into();
        assert!(remove_stop(&mut g, 0));
        assert_eq!(g[0], (0.0, "stone".into()));
        while remove_stop(&mut g, 0) {}
        assert_eq!(g.len(), 1, "the last stop stays");
        assert_eq!(add_stop(&mut g, 5.0, "lamp"), 1, "kept in 0..1, at the end");
        assert_eq!(g[1].0, 1.0);
    }

    #[test]
    fn curve_points_and_the_ready_shapes() {
        let k = Keys::parse(FLAMES);
        let mut c = curve(&k.table().unwrap(), "size_curve");
        assert_eq!(write_curve(&c).as_deref(), k.get("size_curve"));
        let i = add_point(&mut c, 0.5);
        assert_eq!(i, 2);
        assert!((c[2][1] - client::fx::curve_at(&curve(&k.table().unwrap(), "size_curve"), 0.5)).abs() < 1e-6, "a new point does not bend it");
        move_point(&mut c, 2, 0.0, 9.0);
        assert_eq!(c[2], [0.25, CURVE_MAX], "between its neighbours, under the top");
        assert!(remove_point(&mut c, 2));
        assert!(remove_point(&mut c, 1));
        assert!(!remove_point(&mut c, 0), "two stay");
        for (name, help, pts) in SHAPES {
            let s = shape(name).unwrap();
            assert!(!help.is_empty() && s.len() >= 2 && s.len() == pts.len());
            assert!(s.windows(2).all(|w| w[0][0] <= w[1][0]) && s[0][0] == 0.0 && s.last().unwrap()[0] == 1.0, "{name} runs the whole life in order");
        }
        assert_eq!(client::fx::curve_at(&shape("shrink").unwrap(), 1.0), 0.0);
        assert_eq!(client::fx::curve_at(&shape("pulse").unwrap(), 0.5), 1.0);
    }

    #[test]
    fn numbers_set_in_their_form_and_the_game_takes_the_row() {
        let set_ = client::fx::parse_set(&std::fs::read_to_string("../../data/effects.toml").unwrap(), &client::fx::PALETTE).unwrap();
        let mut k = Keys::parse(FLAMES);
        set(&mut k, field("speed", Form::Pair(0)), 0.5);
        assert_eq!(k.get("speed"), Some("[0.5, 0.5]"), "from never past to");
        set(&mut k, field("noise", Form::Noise("scale")), 0.6);
        assert_eq!(k.get("noise"), Some("{ strength = 2.5, scale = 0.6, speed = 1.5 }"));
        set(&mut k, field("rate", Form::One), 999.0);
        assert_eq!(k.get("rate"), Some("200"), "kept in range");
        set(&mut k, field("rate", Form::One), 45.0);
        set(&mut k, field("box", Form::Three(1)), 0.2);
        assert_eq!(k.get("box"), Some("[0.4, 0.2, 0.4]"));
        client::fx::parse_row(&k.text(), &set_).unwrap();
        choose(&mut k, "start", "ring");
        assert_eq!((k.get("box"), k.get("radius")), (None, Some("0.3")), "a start brings its numbers and drops the others'");
        client::fx::parse_row(&k.text(), &set_).unwrap();
        // a cone's dir, never of no length
        choose(&mut k, "start", "cone");
        set(&mut k, field("dir", Form::Three(0)), 0.5);
        assert_eq!(k.get("dir"), Some("[0.5, 1, 0]"), "dir starts upwards");
        set(&mut k, field("dir", Form::Three(0)), 0.0);
        set(&mut k, field("dir", Form::Three(1)), 0.0);
        assert_eq!(k.get("dir"), Some("[0, 1, 0]"), "a dir of no length is not taken");
        client::fx::parse_row(&k.text(), &set_).unwrap();
        // every look leaves a row the game takes: what only squares and textures take goes, a model or a block is named
        choose_look(&mut k, "debris", None);
        assert!(k.get("gradient").is_none() && k.get("glow").is_none(), "{}", k.text());
        client::fx::parse_row(&k.text(), &set_).unwrap();
        choose_look(&mut k, "square", None);
        set(&mut k, field("colour", Form::Three(0)), 0.2);
        assert_eq!(k.get("colour"), Some("[0.2, 1, 1]"), "a square without a gradient is white to start");
        client::fx::parse_row(&k.text(), &set_).unwrap();
        choose_look(&mut k, "model", Some("furnace"));
        assert_eq!(k.get("model"), Some("\"furnace\""));
        client::fx::parse_row(&k.text(), &set_).unwrap();
        choose_look(&mut k, "texture", Some("dirt"));
        assert_eq!((k.get("model"), k.get("texture")), (None, Some("\"dirt\"")));
        client::fx::parse_row(&k.text(), &set_).unwrap();
        choose_look(&mut k, "texture", None);
        assert_eq!(k.get("texture"), Some("\"dirt\""), "a look chosen again keeps its name");
        set_name(&mut k, "then", Some("smoke"));
        assert_eq!(client::fx::parse_row(&k.text(), &set_).unwrap().then.as_deref(), Some("smoke"));
        choose(&mut k, "start", "ring");
        let mut r = Rng::new(3);
        let a = vary(&k, &mut r, 1.0);
        assert_eq!(a, vary(&k, &mut Rng::new(3), 1.0), "the same for a seed");
        assert_ne!(a, k);
        let t = a.table().unwrap();
        for fl in FIELDS.iter().filter(|fl| t.contains_key(fl.key)) {
            let v = get(&t, fl).unwrap();
            assert!((fl.min..=fl.max).contains(&v), "{} {v}", fl.label);
        }
    }
}
