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
    n: i32,
    generator: Generator,
    ground: [String; 3],
    air: bool,
    orbit: Option<f64>,
    orbit_min: Option<f64>,
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
    /// Columns per face edge by default.
    pub n: i32,
    pub generator: Generator,
    /// Block ids: the top, the band under it, the rock below.
    pub ground: [u8; 3],
    pub air: bool,
    /// (radii of the first body, at least metres); None for the first body.
    pub orbit: Option<(f64, f64)>,
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
        if r.n <= 0 || r.n % crate::world::CS != 0 {
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
        rows.push(Row { name: r.name, words: r.words.iter().map(|w| w.to_lowercase()).collect(), n: r.n, generator: r.generator, ground, air: r.air, orbit });
    }
    Ok(rows)
}

pub static BODIES: LazyLock<Vec<Row>> =
    LazyLock::new(|| parse(include_str!("../../../data/bodies.toml")).unwrap_or_else(|e| panic!("data/bodies.toml: {e}")));

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
        assert_eq!((BODIES[1].generator, BODIES[1].air, BODIES[1].orbit), (Generator::Hills, false, Some((2.83, 3000.0))));
        assert_eq!(BODIES[1].ground[0], BLOCKS.id("regolith"));
        assert_eq!((BODIES[0].n, BODIES[1].n), (6144, 256));
        assert_eq!((find("planet"), find("TERRA"), find("moon"), find("luna"), find("sun")), (Some(0), Some(0), Some(1), Some(1), None));
    }

    #[test]
    fn bad_rows_are_refused() {
        let a = "[[body]]\nname = \"A\"\nn = 512\ngenerator = \"hills\"\nground = [\"stone\", \"stone\", \"stone\"]\nair = false\n";
        assert!(parse(a).is_ok());
        let b = |extra: &str| format!("{a}[[body]]\nname = \"B\"\nn = 512\ngenerator = \"hills\"\nground = [\"stone\", \"stone\", \"stone\"]\nair = false\n{extra}");
        assert!(parse(&b("orbit = 3.0\n")).is_ok());
        for (bad, why) in [
            (String::new(), "empty"),
            (b(""), "no orbit for the second"),
            (format!("{a}orbit = 3.0\n"), "an orbit for the first"),
            (b("orbit = 3.0\nwords = [\"a\"]\n"), "a word taken"),
            (a.replace("512", "500"), "n not a chunk multiple"),
            (a.replace("\"stone\", \"stone\", \"stone\"", "\"stone\", \"cheese\", \"stone\""), "no such block"),
            (a.replace("hills", "lava"), "no such generator"),
        ] {
            assert!(parse(&bad).is_err(), "{why}");
        }
    }
}
