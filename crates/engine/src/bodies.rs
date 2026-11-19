//! The bodies as rows of data, from `data/bodies.toml` (fundamentals, section 2: more bodies is more rows).
//! Built into the binary like the blocks, so a client and a server from one build agree.
use crate::bake::Stage;
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

/// The bake that makes a body's map: the stages it runs, in order (0165-bake-stages; `bake::check`), and its
/// body's sun at its highest (`luna::sun_high` of the row: what an airless body's `ice` reads).
/// Terra's (ADR 0043) and Luna's (ADR 0070) lists keep their maps' files, versions and old layouts. A planet's
/// crust (a list starting with `crust` on a body that circles the sun) is the third field's (`bake::mars`, step 5a).
#[derive(Clone, Debug, PartialEq)]
pub struct Bake(pub Vec<Stage>, pub f64, pub Option<Planet>);

/// What a planet's crust stages read of its row (`bake::mars`), and the name its map is kept under.
#[derive(Clone, Debug, PartialEq)]
pub struct Planet {
    pub name: String,
    /// How old its surface is, 0..1: the share of the impacts' power law it has collected (Luna's highlands 1).
    pub age: f64,
}

impl Bake {
    /// This bake's map of a world: from memory, the disk or baked now.
    pub fn get(&self, seed: i32, n: i32) -> std::sync::Arc<crate::bake::Map> {
        if let Some(p) = &self.2 {
            return crate::bake::mars::get(&self.0, seed, n, p);
        }
        match &self.0[..] {
            s if s == Stage::TERRA => crate::bake::get(seed, n),
            s if s == Stage::LUNA => crate::bake::luna::get(seed, n),
            // shortcut: a moon's list other than Luna's is baked at every start and not kept, until a body has one
            s => std::sync::Arc::new(crate::bake::run(s, seed, n, self.1)),
        }
    }
    /// The map of this hash, baked again if it is not kept; None when this build bakes another.
    pub fn for_hash(&self, seed: i32, n: i32, hash: u64) -> Option<std::sync::Arc<crate::bake::Map>> {
        match &self.0[..] {
            _ if self.2.is_some() => Some(self.get(seed, n)).filter(|m| m.hash == hash),
            s if s == Stage::TERRA => crate::bake::for_hash(seed, n, hash),
            s if s == Stage::LUNA => crate::bake::luna::for_hash(seed, n, hash),
            _ => Some(self.get(seed, n)).filter(|m| m.hash == hash),
        }
    }
    /// Where this bake's map of a world is kept (a planet's crust: `mars::file`; another moon's list: a name never
    /// written, `get`).
    pub fn file(&self, seed: i32, n: i32) -> std::path::PathBuf {
        if let Some(p) = &self.2 {
            return crate::bake::mars::file(&self.0, seed, n, p);
        }
        match &self.0[..] {
            s if s == Stage::TERRA => crate::bake::file(seed, n),
            s if s == Stage::LUNA => crate::bake::luna::file(seed, n),
            s => std::path::PathBuf::from(crate::bake::DIR).join(format!("seed-{seed}-planet-{n}-{s:?}.map")),
        }
    }
    /// This bake's map version, for a message.
    pub fn version(&self) -> u32 {
        if self.0[0] == Stage::Plates { crate::bake::VERSION } else if self.2.is_some() { crate::bake::mars::VERSION } else { crate::bake::luna::VERSION }
    }
    /// Whether it bakes on plates (Terra's kind of map, with a sea); else an airless crust's (Luna's).
    pub fn plates(&self) -> bool {
        self.0[0] == Stage::Plates
    }
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
struct Raw {
    name: String,
    parent: String,
    #[serde(default)]
    home: bool,
    stages: Option<Vec<Stage>>,
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
    age: Option<f64>,
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
    /// The row it circles; None: the sun.
    pub parent: Option<usize>,
    /// Whether it is the home world (the first row, and only it): the spawn, the ore and its ledger, the
    /// weather, the tide and the herds are its.
    pub home: bool,
    /// The bake that makes its map; None: generated from the seed alone.
    pub bake: Option<Bake>,
    /// Other words /goto takes for it, lower case.
    pub words: Vec<String>,
    /// Columns per face edge by default (another body's follow from the first's: `columns`).
    pub n: i32,
    pub generator: Generator,
    /// Block ids: the top, the band under it, the rock below.
    pub ground: [u16; 3],
    pub air: bool,
    /// (radii of its parent, at least metres): a moon's; None for a body that circles the sun.
    pub orbit: Option<(f64, f64)>,
    /// The home body's radius over this one's; None for the home body.
    pub size: Option<f64>,
    /// The sun's distance in this body's radii: a body that circles the sun has it.
    pub sun: Option<f64>,
    pub makeup: Makeup,
}

impl Row {
    /// Whether /goto `w` (any case) means this body.
    pub fn named(&self, w: &str) -> bool {
        self.name.eq_ignore_ascii_case(w) || self.words.iter().any(|x| x.eq_ignore_ascii_case(w))
    }
}

/// Reads and checks the rows: at least one, names and words unique, each `parent` the sun or a row above it
/// (so poses are worked out down the rows, never round a loop), exactly one home, the first row, circling the
/// sun and with a moon (its first moon sizes the sun, `sky::Sky::new`), every ground block exists.
pub fn parse(text: &str) -> Result<Vec<Row>, String> {
    let f: File = toml::from_str(text).map_err(|e| e.to_string())?;
    if f.body.is_empty() {
        return Err("no bodies".into());
    }
    let mut seen: Vec<String> = Vec::new();
    let mut rows: Vec<Row> = Vec::new();
    for (i, r) in f.body.into_iter().enumerate() {
        for w in std::iter::once(&r.name).chain(&r.words) {
            let w = w.to_lowercase();
            if w == "sun" || seen.contains(&w) {
                return Err(format!("body '{}': the word '{w}' is taken", r.name));
            }
            seen.push(w);
        }
        let parent = match r.parent.as_str() {
            "sun" => None,
            p => Some(rows.iter().position(|x| x.name == p).ok_or(format!("body '{}': its parent '{p}' is not a row above it", r.name))?),
        };
        if r.home != (i == 0) {
            return Err(format!("body '{}': the first row is the home body, and only it", r.name));
        }
        let n = match (r.home, parent, r.n, r.size, r.sun) {
            (true, None, Some(n), None, Some(sun)) if sun > 1.0 => n,
            (true, ..) => return Err(format!("body '{}': the home body circles the sun, has n and a sun over 1 radius, and no size", r.name)),
            (false, None, None, Some(k), Some(sun)) if k > 0.0 && sun > 1.0 => size_columns(rows[0].n, k),
            (false, None, ..) => return Err(format!("body '{}': a body that circles the sun and is not home has a size over 0 and a sun over 1 radius, and no n", r.name)),
            (false, Some(_), None, Some(k), None) if k >= 1.0 => size_columns(rows[0].n, k),
            _ => return Err(format!("body '{}': a moon has a size of 1 or more, and no n or sun", r.name)),
        };
        if n <= 0 || n % crate::world::CS != 0 {
            return Err(format!("body '{}': n must be a positive multiple of {}", r.name, crate::world::CS));
        }
        let orbit = match (parent, r.orbit, r.orbit_min) {
            (None, None, None) => None,
            (None, _, _) => return Err(format!("body '{}': a body that circles the sun has no orbit (its sun says how far)", r.name)),
            (Some(_), Some(o), m) if o > 1.0 => Some((o, m.unwrap_or(0.0))),
            _ => return Err(format!("body '{}': needs an orbit over 1 radius of its parent", r.name)),
        };
        let mut ground = [0; 3];
        for (g, name) in ground.iter_mut().zip(&r.ground) {
            *g = BLOCKS.placeable().find(|d| &d.name == name).ok_or(format!("body '{}': no block '{name}'", r.name))?.id;
        }
        let makeup = Makeup { kind: r.kind, iron: r.iron, heat: r.heat, density: r.density, weight_g: r.weight_g };
        check(&makeup).map_err(|e| format!("body '{}': {e}", r.name))?;
        if let Some(s) = &r.stages {
            crate::bake::check(s, parent.is_some()).map_err(|e| format!("body '{}': {e}", r.name))?;
        }
        // a planet's crust (`bake::mars`) reads its surface's age; no other bake does
        let planet = r.stages.as_ref().is_some_and(|s| s[0] == Stage::Crust) && parent.is_none();
        let planet = match (planet, r.age.unwrap_or(1.0)) {
            (true, a) if a > 0.0 && a <= 1.0 => Some(Planet { name: r.name.clone(), age: a }),
            (true, a) => return Err(format!("body '{}': age {a} is the share of the craters its surface has collected, over 0 and at most 1", r.name)),
            (false, _) if r.age.is_some() => return Err(format!("body '{}': age is read only by a planet's crust (stages starting with 'crust' on a body that circles the sun)", r.name)),
            _ => None,
        };
        rows.push(Row { name: r.name, parent, home: r.home, bake: r.stages.map(|s| Bake(s, crate::bake::luna::sun_high(r.size, orbit), planet)), words: r.words.iter().map(|w| w.to_lowercase()).collect(), n, generator: r.generator, ground, air: r.air, orbit, size: r.size, sun: r.sun, makeup });
    }
    if moon(&rows).is_none() {
        return Err(format!("body '{}': the home body needs a moon (its first sizes the sun)", rows[0].name));
    }
    Ok(rows)
}

/// The most bodies a world holds: the renderer's four (client `gpu::MAX_BODIES`, the shaders' `& 3u` body ids),
/// which `World::gravity` keeps on the stack too (0163-n-bodies).
pub const MAX: usize = 4;

/// A welcome's body list against this build's rows (the welcome has no version): one of another length is
/// a server of another build, refused as `world::known_generation` refuses a newer world.
pub fn known_bodies(n: usize) -> Result<(), String> {
    if n == BODIES.len() { Ok(()) } else { Err(format!("the server has {n} bodies, this game {}: update the game", BODIES.len())) }
}

/// The home body's first moon: the one the sky's eclipses, tides and moonlight are of (`sky::Sky`).
pub fn moon(rows: &[Row]) -> Option<usize> {
    rows.iter().position(|r| r.parent == Some(0))
}

/// Columns per face edge of every row, in row order, in a world whose home body has `planet_n` and its first
/// moon `moon_n` (planet= and moon=, ADR 0021); any other body is its row's `size` smaller than the home.
pub fn sizes(rows: &[Row], planet_n: i32, moon_n: i32) -> Vec<i32> {
    let m = moon(rows);
    (0..rows.len()).map(|i| if i == 0 { planet_n } else if Some(i) == m { moon_n } else { rows[i].size.map_or(planet_n, |k| size_columns(planet_n, k)) }).collect()
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

/// The home's first moon's columns (`columns`): what moon= means when it is not given.
pub fn moon_columns(first_n: i32) -> i32 {
    columns(moon(&BODIES).expect("the home has a moon (parse)"), first_n)
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
        assert_eq!(BODIES.iter().map(|r| (r.parent, r.home, r.bake.as_ref().map(|b| b.0.clone()))).collect::<Vec<_>>(), [(None, true, Some(Stage::TERRA.to_vec())), (Some(0), false, Some(Stage::LUNA.to_vec()))]);
        assert_eq!((moon(&BODIES), sizes(&BODIES, 1536, 512), moon_columns(6144)), (Some(1), vec![1536, 512], 1664));
        // a welcome of another number of bodies is a server of another build
        assert!(known_bodies(BODIES.len()).is_ok());
        let e = known_bodies(BODIES.len() + 1).unwrap_err();
        assert!(e.contains("the server has 3 bodies, this game 2") && e.contains("update the game"), "{e}");
        assert_eq!((BODIES[0].n, BODIES[1].n), (6144, 1664), "6144 / 3.7 is 1660.5: 52 chunks");
        assert_eq!((find("home"), find("TERRA"), find("luna"), find("sun")), (Some(0), Some(0), Some(1), None));
        assert_eq!((find("planet"), find("moon")), (None, None), "the bodies go by their names (bugs/2026-11-11_13-10-42)");
    }

    #[test]
    fn bad_rows_are_refused() {
        let a = format!("[[body]]\nname = \"A\"\nparent = \"sun\"\nhome = true\nn = 512\ngenerator = \"hills\"\nground = [\"stone\", \"stone\", \"stone\"]\nair = false\nsun = 100.0\n{MAKEUP}");
        let a = a.as_str();
        let row = |name: &str, extra: &str| format!("[[body]]\nname = \"{name}\"\ngenerator = \"hills\"\nground = [\"stone\", \"stone\", \"stone\"]\nair = false\n{MAKEUP}{extra}");
        let b = |extra: &str| format!("{a}{}", row("B", extra));
        let good = b("parent = \"A\"\norbit = 3.0\nsize = 4.0\n");
        let ok = parse(&good).unwrap();
        assert_eq!(ok[1].n, 128, "a quarter of the first body's 512 columns");
        assert_eq!((ok[0].parent, ok[0].home, ok[1].parent, ok[1].home, ok[1].bake.is_some()), (None, true, Some(0), false, false));
        // a moon of the moon, and one that circles the sun
        let more = format!("{good}{}{}", row("C", "parent = \"B\"\norbit = 3.0\nsize = 16.0\n"), row("D", "parent = \"sun\"\nsun = 300.0\nsize = 2.0\nstages = [\"crust\", \"impacts\"]\n"));
        let ok = parse(&more).unwrap();
        assert_eq!((ok[2].parent, ok[2].n, ok[3].parent, ok[3].n, ok[3].bake.as_ref().map(|b| b.0.clone())), (Some(1), 32, None, 256, Some(vec![Stage::Crust, Stage::Impacts])));
        for (bad, why) in [
            (String::new(), "empty"),
            (a.to_string(), "a home without a moon"),
            (b("parent = \"A\"\nsize = 4.0\n"), "no orbit for the moon"),
            (b("parent = \"A\"\norbit = 3.0\n"), "no size for the moon"),
            (b("orbit = 3.0\nsize = 4.0\n"), "no parent"),
            (b("parent = \"Z\"\norbit = 3.0\nsize = 4.0\n"), "a parent of no row"),
            (b("parent = \"B\"\norbit = 3.0\nsize = 4.0\n"), "its own parent"),
            (format!("{}{}", row("C", "parent = \"B\"\norbit = 3.0\nsize = 4.0\n"), &good), "a parent below it"),
            (b("parent = \"A\"\norbit = 3.0\nsize = 4.0\nhome = true\n"), "two homes"),
            (good.replace("home = true\n", ""), "no home"),
            (format!("{good}{}", row("D", "parent = \"sun\"\nsize = 2.0\n")), "circles the sun with no sun"),
            (format!("{good}{}", row("D", "parent = \"sun\"\nsun = 300.0\nsize = 2.0\norbit = 3.0\n")), "circles the sun with an orbit"),
            (b("parent = \"A\"\norbit = 3.0\nsize = 4.0\nn = 128\n"), "both a size and n"),
            (b("parent = \"A\"\norbit = 3.0\nsize = 0.5\n"), "bigger than the first"),
            (good.replace("sun = 100.0\n", ""), "no sun for the first"),
            (good.replacen("sun = 100.0\n", "sun = 100.0\norbit = 3.0\n", 1), "an orbit for the first"),
            (b("parent = \"A\"\norbit = 3.0\nsize = 4.0\nwords = [\"a\"]\n"), "a word taken"),
            (good.replace("512", "500"), "n not a chunk multiple"),
            (good.replacen("\"stone\", \"stone\", \"stone\"", "\"stone\", \"cheese\", \"stone\"", 1), "no such block"),
            (good.replacen("hills", "lava", 1), "no such generator"),
        ] {
            assert!(parse(&bad).is_err(), "{why}");
        }
        // a stage list (0165-bake-stages): an unknown stage, and one without what it needs, refused and saying why
        let stages = |s: &str| parse(&b(&format!("parent = \"A\"\norbit = 3.0\nsize = 4.0\nstages = {s}\n")));
        for (s, says) in [
            ("[\"plates\", \"mars\"]", "unknown variant `mars`"),
            ("[]", "starts with 'plates' or 'crust'"),
            ("[\"erosion\"]", "starts with 'plates' or 'crust'"),
            ("[\"plates\", \"climate\", \"life\"]", "'climate' needs 'erosion' before it"),
            ("[\"plates\", \"erosion\", \"climate\", \"life\", \"ice\"]", "'life' after it"),
            ("[\"plates\", \"erosion\"]", "ends with 'life'"),
            ("[\"crust\", \"volcanism\"]", "'volcanism' needs 'impacts' before it"),
            ("[\"crust\", \"erosion\"]", "not built yet on a crust without plates"),
            ("[\"plates\", \"impacts\", \"erosion\", \"climate\", \"life\"]", "not built yet on plates"),
            ("[\"crust\", \"impacts\", \"impacts\"]", "twice"),
            ("[\"crust\", \"plates\"]", "only the first"),
        ] {
            let e = stages(s).expect_err(s);
            // (an unknown name is the file's reader's error, with its line)
            assert!(e.contains(says) && (s.contains("mars") || e.contains("body 'B'")), "{s}: {e}");
        }
        assert!(stages("[\"crust\", \"impacts\", \"ice\", \"volcanism\"]").is_ok() && stages("[\"plates\", \"erosion\", \"climate\", \"life\"]").is_ok());
        // a planet's crust (step 5a): erosion on it after the impacts, its age read; the crossings still refused
        let planet = |extra: &str| parse(&format!("{good}{}", row("D", &format!("parent = \"sun\"\nsun = 300.0\nsize = 2.0\n{extra}"))));
        for (s, says) in [
            ("stages = [\"crust\", \"erosion\"]\n", "'erosion' needs 'impacts' before it"),
            ("stages = [\"crust\", \"impacts\", \"climate\"]\n", "not built yet on a crust without plates"),
            ("stages = [\"crust\", \"impacts\", \"erosion\", \"life\"]\n", "not built yet on a crust without plates"),
            ("stages = [\"plates\", \"impacts\", \"erosion\", \"climate\", \"life\"]\n", "not built yet on plates"),
            ("stages = [\"crust\", \"impacts\"]\nage = 0.0\n", "age 0"),
            ("stages = [\"crust\", \"impacts\"]\nage = 1.5\n", "age 1.5"),
            ("stages = [\"plates\", \"erosion\", \"climate\", \"life\"]\nage = 1.0\n", "age is read only by a planet's crust"),
            ("age = 1.0\n", "age is read only by a planet's crust"),
        ] {
            let e = planet(s).expect_err(s);
            assert!(e.contains(says) && e.contains("body 'D'"), "{s}: {e}");
        }
        assert!(stages("[\"crust\", \"impacts\", \"age\"]").is_err(), "no stage 'age'");
        let ok = planet("stages = [\"crust\", \"impacts\", \"volcanism\", \"erosion\", \"ice\"]\nage = 0.5\n").unwrap();
        assert_eq!(ok[2].bake.as_ref().unwrap().2, Some(Planet { name: "D".into(), age: 0.5 }));
        assert_eq!(planet("stages = [\"crust\", \"impacts\"]\n").unwrap()[2].bake.as_ref().unwrap().2.as_ref().map(|p| p.age), Some(1.0), "an age of 1 when not given");
        // a moon's crust is Luna's kind: no age, no planet
        assert_eq!(stages("[\"crust\", \"impacts\"]").unwrap()[1].bake.as_ref().unwrap().2, None);
    }

    /// The Mars-like planet's draft row (planet-model.md step 5a, `data/drafts/bodies-mars.toml`): it parses after
    /// bodies.toml's rows, a planet's crust with the developer's stage list, 1.85 times smaller than Terra.
    #[test]
    fn the_mars_draft_row_parses_and_its_list_is_accepted() {
        let rows = parse(&format!("{}{}", include_str!("../../../data/bodies.toml"), include_str!("../../../data/drafts/bodies-mars.toml"))).unwrap();
        assert_eq!(rows[..2], BODIES[..], "Terra and Luna as data/bodies.toml has them");
        let m = &rows[2];
        assert_eq!((m.name.as_str(), m.parent, m.home, m.n, m.orbit), ("Mars", None, false, 3328, None));
        assert_eq!(m.n / crate::world::CS, 104, "13 x 8 chunks, as Luna's 52 are 13 x 4");
        let b = m.bake.as_ref().unwrap();
        assert_eq!(b.0, [Stage::Crust, Stage::Impacts, Stage::Volcanism, Stage::Erosion, Stage::Ice]);
        assert_eq!(b.2, Some(Planet { name: "Mars".into(), age: 1.0 }));
        assert_eq!(b.version(), crate::bake::mars::VERSION);
        assert_eq!((m.makeup.kind, m.makeup.iron, m.makeup.heat, m.makeup.density), (Kind::Rocky, 0.5, 0.15, [2900.0, 3500.0, 6700.0]));
        assert_eq!(m.sun, Some(1.5 * 23500.0 * 1.85), "1.5 AU in its radii");
    }

    const MAKEUP: &str = "kind = \"rocky\"\niron = 0.5\nheat = 0.5\ndensity = [2700.0, 3300.0, 7900.0]\n";

    #[test]
    fn a_makeup_is_whole_and_in_range() {
        let moon = format!("[[body]]\nname = \"B\"\nparent = \"A\"\norbit = 3.0\nsize = 4.0\ngenerator = \"hills\"\nground = [\"stone\", \"stone\", \"stone\"]\nair = false\n{MAKEUP}");
        let row = |makeup: &str| format!("[[body]]\nname = \"A\"\nparent = \"sun\"\nhome = true\nn = 512\ngenerator = \"hills\"\nground = [\"stone\", \"stone\", \"stone\"]\nair = false\nsun = 100.0\n{makeup}{moon}");
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
