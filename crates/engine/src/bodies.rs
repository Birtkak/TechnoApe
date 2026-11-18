//! The bodies as rows of data, from `data/bodies.toml` (fundamentals, section 2: more bodies is more rows).
//! Built into the binary like the blocks, so a client and a server from one build agree.
use crate::blocks::BLOCKS;
use serde::Deserialize;
use std::sync::LazyLock;

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Generator {
    /// Continents, oceans and a sea (ADR 0021).
    Continents,
    /// Noise hills, no sea.
    Hills,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
struct Raw {
    name: String,
    #[serde(default)]
    words: Vec<String>,
    n: Option<i32>,
    generator: Generator,
    ground: [String; 3],
    air: bool,
    orbit: Option<f64>,
    orbit_min: Option<f64>,
    size: Option<f64>,
    sun: Option<f64>,
    kind: Kind,
    iron: f64,
    heat: f64,
    density: [f64; 3],
    weight_g: Option<f64>,
}

/// What a body is (planet-model.md): only rocky bodies are built.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Rocky,
    Icy,
    Gas,
}

/// What a body is made of: its interior (`interior::Interior`) is derived from this and its radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Makeup {
    pub kind: Kind,
    /// The metal core's radius over the body's, 0..1.
    pub iron: f64,
    /// How hot the inside still is, 0 dead (the core all solid) to 1 young (all liquid).
    pub heat: f64,
    /// The crust's rock, the mantle's and the core's metal, kg/m3, lightest on top.
    pub density: [f64; 3],
    /// The pull the weight of its rock is reckoned with, m/s^2, where it is not its own (Luna: Terra's, today's
    /// pressure until her deep is generated as hers).
    pub weight_g: Option<f64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    body: Vec<Raw>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub name: String,
    /// Other words /goto takes for it, lower case.
    pub words: Vec<String>,
    /// Columns per face edge by default (another body's follow from the first's: `columns`).
    pub n: i32,
    pub generator: Generator,
    /// Block ids: the top, the band under it, the rock below.
    pub ground: [u16; 3],
    pub air: bool,
    /// (radii of the first body, at least metres); None for the first body.
    pub orbit: Option<(f64, f64)>,
    /// The first body's radius over this one's; None for the first body.
    pub size: Option<f64>,
    /// The sun's distance in this body's radii: the first body's only.
    pub sun: Option<f64>,
    pub makeup: Makeup,
}

impl Row {
    /// Whether /goto `w` (any case) means this body.
    pub fn named(&self, w: &str) -> bool {
        self.name.eq_ignore_ascii_case(w) || self.words.iter().any(|x| x.eq_ignore_ascii_case(w))
    }
}

/// Reads and checks the rows: at least one, names and words unique, the first circles the sun and the others
/// the first, every ground block exists.
pub fn parse(text: &str) -> Result<Vec<Row>, String> {
    let f: File = toml::from_str(text).map_err(|e| e.to_string())?;
    if f.body.is_empty() {
        return Err("no bodies".into());
    }
    let mut seen: Vec<String> = Vec::new();
    let mut rows = Vec::new();
    for (i, r) in f.body.into_iter().enumerate() {
        for w in std::iter::once(&r.name).chain(&r.words) {
            let w = w.to_lowercase();
            if w == "sun" || seen.contains(&w) {
                return Err(format!("body '{}': the word '{w}' is taken", r.name));
            }
            seen.push(w);
        }
        let n = match (i, r.n, r.size, r.sun) {
            (0, Some(n), None, Some(sun)) if sun > 1.0 => n,
            (0, ..) => return Err(format!("body '{}': the first body has n and a sun over 1 radius, and no size", r.name)),
            (_, None, Some(k), None) if k >= 1.0 => size_columns(rows.first().map_or(0, |f: &Row| f.n), k),
            _ => return Err(format!("body '{}': a body after the first has a size of 1 or more, and no n or sun", r.name)),
        };
        if n <= 0 || n % crate::world::CS != 0 {
            return Err(format!("body '{}': n must be a positive multiple of {}", r.name, crate::world::CS));
        }
        let orbit = match (i, r.orbit, r.orbit_min) {
            (0, None, None) => None,
            (0, _, _) => return Err(format!("body '{}': the first body circles the sun, it has no orbit", r.name)),
            (_, Some(o), m) if o > 1.0 => Some((o, m.unwrap_or(0.0))),
            _ => return Err(format!("body '{}': needs an orbit over 1 radius of the first body", r.name)),
        };
        let mut ground = [0; 3];
        for (g, name) in ground.iter_mut().zip(&r.ground) {
            *g = BLOCKS.placeable().find(|d| &d.name == name).ok_or(format!("body '{}': no block '{name}'", r.name))?.id;
        }
        let makeup = Makeup { kind: r.kind, iron: r.iron, heat: r.heat, density: r.density, weight_g: r.weight_g };
        check(&makeup).map_err(|e| format!("body '{}': {e}", r.name))?;
        rows.push(Row { name: r.name, words: r.words.iter().map(|w| w.to_lowercase()).collect(), n, generator: r.generator, ground, air: r.air, orbit, size: r.size, sun: r.sun, makeup });
    }
    Ok(rows)
}

/// A makeup the interior can be derived from: a rocky body, a core inside it, heat 0..1, densities 500..30000
/// kg/m3 growing going down, a positive pull.
fn check(m: &Makeup) -> Result<(), String> {
    if m.kind != Kind::Rocky {
        return Err(format!("kind '{:?}' is not built yet (rocky only)", m.kind).to_lowercase());
    }
    if !(m.iron > 0.0 && m.iron < 1.0) {
        return Err(format!("iron {} is the core's radius over the body's, over 0 and under 1", m.iron));
    }
    if !(0.0..=1.0).contains(&m.heat) {
        return Err(format!("heat {} is 0 (dead) to 1 (young)", m.heat));
    }
    let [c, mantle, core] = m.density;
    if !m.density.iter().all(|d| (500.0..=30000.0).contains(d)) || !(c <= mantle && mantle <= core) {
        return Err(format!("density {:?}: crust, mantle and core, 500..30000 kg/m3, none lighter than the one over it", m.density));
    }
    if m.weight_g.is_some_and(|g| !(g > 0.0 && g < 1000.0)) {
        return Err("weight_g is a pull over 0 m/s^2".into());
    }
    Ok(())
}

pub static BODIES: LazyLock<Vec<Row>> =
    LazyLock::new(|| parse(include_str!("../../../data/bodies.toml")).unwrap_or_else(|e| panic!("data/bodies.toml: {e}")));

/// Columns per face edge of a body `size` times smaller than one of `first_n`: the nearest multiple of a chunk
/// (on the offered planets 13 chunks times a power of two, which the chunk tree tiles), at least one chunk.
fn size_columns(first_n: i32, size: f64) -> i32 {
    let cs = crate::world::CS;
    ((first_n as f64 / size / cs as f64).round() as i32).max(1) * cs
}

/// Body `i`'s columns per face edge in a world whose first body has `first_n`: the client, the server and a
/// new world's page all ask this, so a moon is always its row's `size` times smaller (ADR 0041). A save
/// keeps the columns it was made with.
pub fn columns(i: usize, first_n: i32) -> i32 {
    BODIES[i].size.map_or(first_n, |k| size_columns(first_n, k))
}

/// The body /goto `w` means, by name or word.
pub fn find(w: &str) -> Option<usize> {
    BODIES.iter().position(|r| r.named(w))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_data_file_has_terra_and_luna() {
        let names: Vec<&str> = BODIES.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["Terra", "Luna"]);
        assert_eq!((BODIES[0].generator, BODIES[0].air, BODIES[0].orbit), (Generator::Continents, true, None));
        assert_eq!((BODIES[1].generator, BODIES[1].air, BODIES[1].orbit), (Generator::Hills, false, Some((60.0, 0.0))));
        assert_eq!(BODIES[1].ground[0], BLOCKS.id("regolith"));
        assert_eq!((BODIES[0].n, BODIES[1].n), (6144, 1664), "6144 / 3.7 is 1660.5: 52 chunks");
        assert_eq!((find("home"), find("TERRA"), find("luna"), find("sun")), (Some(0), Some(0), Some(1), None));
        assert_eq!((find("planet"), find("moon")), (None, None), "the bodies go by their names (bugs/2026-11-11_13-10-42)");
    }

    #[test]
    fn bad_rows_are_refused() {
        let a = format!("[[body]]\nname = \"A\"\nn = 512\ngenerator = \"hills\"\nground = [\"stone\", \"stone\", \"stone\"]\nair = false\nsun = 100.0\n{MAKEUP}");
        let a = a.as_str();
        assert!(parse(a).is_ok());
        let b = |extra: &str| format!("{a}[[body]]\nname = \"B\"\ngenerator = \"hills\"\nground = [\"stone\", \"stone\", \"stone\"]\nair = false\n{MAKEUP}{extra}");
        let ok = parse(&b("orbit = 3.0\nsize = 4.0\n")).unwrap();
        assert_eq!(ok[1].n, 128, "a quarter of the first body's 512 columns");
        for (bad, why) in [
            (String::new(), "empty"),
            (b("size = 4.0\n"), "no orbit for the second"),
            (b("orbit = 3.0\n"), "no size for the second"),
            (b("orbit = 3.0\nsize = 4.0\nn = 128\n"), "both a size and n"),
            (b("orbit = 3.0\nsize = 0.5\n"), "bigger than the first"),
            (a.replace("sun = 100.0\n", ""), "no sun for the first"),
            (format!("{a}orbit = 3.0\n"), "an orbit for the first"),
            (b("orbit = 3.0\nsize = 4.0\nwords = [\"a\"]\n"), "a word taken"),
            (a.replace("512", "500"), "n not a chunk multiple"),
            (a.replace("\"stone\", \"stone\", \"stone\"", "\"stone\", \"cheese\", \"stone\""), "no such block"),
            (a.replace("hills", "lava"), "no such generator"),
        ] {
            assert!(parse(&bad).is_err(), "{why}");
        }
    }

    const MAKEUP: &str = "kind = \"rocky\"\niron = 0.5\nheat = 0.5\ndensity = [2700.0, 3300.0, 7900.0]\n";

    #[test]
    fn a_makeup_is_whole_and_in_range() {
        let row = |makeup: &str| format!("[[body]]\nname = \"A\"\nn = 512\ngenerator = \"hills\"\nground = [\"stone\", \"stone\", \"stone\"]\nair = false\nsun = 100.0\n{makeup}");
        let ok = parse(&row(MAKEUP)).unwrap();
        assert_eq!(ok[0].makeup, Makeup { kind: Kind::Rocky, iron: 0.5, heat: 0.5, density: [2700.0, 3300.0, 7900.0], weight_g: None });
        let with = |from: &str, to: &str| row(&MAKEUP.replace(from, to));
        for (bad, why, says) in [
            (with("iron = 0.5\n", ""), "no iron", "iron"),
            (with("heat = 0.5\n", ""), "no heat", "heat"),
            (with("kind = \"rocky\"\n", ""), "no kind", "kind"),
            (with("density = [2700.0, 3300.0, 7900.0]\n", ""), "no density", "density"),
            (with("iron = 0.5", "iron = 1.0"), "a core as big as the body", "iron"),
            (with("iron = 0.5", "iron = 0.0"), "no core", "iron"),
            (with("heat = 0.5", "heat = 1.5"), "hotter than young", "heat"),
            (with("heat = 0.5", "heat = -0.1"), "colder than dead", "heat"),
            (with("3300.0", "2000.0"), "a mantle lighter than its crust", "density"),
            (with("7900.0", "90000.0"), "a core of neutron star", "density"),
            (with("2700.0, ", ""), "two densities", "density"),
            (format!("{}weight_g = -1.0\n", row(MAKEUP)), "a pull upward", "weight_g"),
            (with("rocky", "icy"), "icy is not built", "not built yet"),
            (with("rocky", "gas"), "gas is not built", "not built yet"),
            (with("rocky", "cheese"), "no such kind", "kind"),
        ] {
            let e = parse(&bad).expect_err(why);
            assert!(e.contains(says), "{why}: {e}");
        }
    }
}
