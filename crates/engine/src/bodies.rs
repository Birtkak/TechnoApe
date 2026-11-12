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
        rows.push(Row { name: r.name, words: r.words.iter().map(|w| w.to_lowercase()).collect(), n, generator: r.generator, ground, air: r.air, orbit, size: r.size, sun: r.sun });
    }
    Ok(rows)
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
        let a = "[[body]]\nname = \"A\"\nn = 512\ngenerator = \"hills\"\nground = [\"stone\", \"stone\", \"stone\"]\nair = false\nsun = 100.0\n";
        assert!(parse(a).is_ok());
        let b = |extra: &str| format!("{a}[[body]]\nname = \"B\"\ngenerator = \"hills\"\nground = [\"stone\", \"stone\", \"stone\"]\nair = false\n{extra}");
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
}
