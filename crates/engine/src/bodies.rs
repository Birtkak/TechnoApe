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
    /// An icy body's crust (step 6c, `bake::icy`: Luna's stages on ice, its own kinds) in place of a planet's: the
    /// sun's height over its equator that its ice stage reads (`luna::sun_high`), so its map's file names it too.
    pub icy: Option<f64>,
    /// Its shields as the round cones of the maps before version 202 (`bake::mars::CONES`): a world before
    /// generation 15 (`Bake::at`); false in every row.
    pub cones: bool,
}

impl Bake {
    /// This bake as a world of generation `gen` has it: a planet's crust before generation 15 (`world::HILL_COUNTRY`) with
    /// its shields as cones. (Terra's map by generation is `get_at`'s and `file_at`'s.)
    pub fn at(&self, gen: u8) -> Bake {
        Bake(self.0.clone(), self.1, self.2.clone().map(|p| Planet { cones: p.icy.is_none() && gen < crate::world::HILL_COUNTRY, ..p }))
    }
    /// `get` for a world of generation `gen`.
    pub fn get_at(&self, seed: i32, n: i32, gen: u8) -> std::sync::Arc<crate::bake::Map> {
        if self.0[..] == Stage::TERRA && self.2.is_none() { crate::bake::get_at(seed, n, gen) } else { self.at(gen).get(seed, n) }
    }
    /// `file` for a world of generation `gen`.
    pub fn file_at(&self, seed: i32, n: i32, gen: u8) -> std::path::PathBuf {
        if self.0[..] == Stage::TERRA && self.2.is_none() { crate::bake::file_at(seed, n, gen) } else { self.at(gen).file(seed, n) }
    }
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
            // (a planet's crust: as it is baked today, else with the cones of a world before generation 15)
            // (the kept one first, of either version, so a world of the cones bakes no domes on its way to them)
            _ if self.2.is_some() => crate::bake::mars::find(seed, n, hash).or_else(|| Some(self.get(seed, n)).filter(|m| m.hash == hash)).or_else(|| Some(self.at(0).get(seed, n)).filter(|m| m.hash == hash)),
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
        match &self.2 {
            _ if self.0[0] == Stage::Plates => crate::bake::VERSION,
            Some(p) => crate::bake::mars::version(p),
            None => crate::bake::luna::VERSION,
        }
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
    generator: Option<Generator>,
    ground: Option<[String; 3]>,
    air: bool,
    day: Option<f64>,
    colour: Option<[f32; 3]>,
    orbit: Option<f64>,
    orbit_min: Option<f64>,
    size: Option<f64>,
    sun: Option<f64>,
    kind: Kind,
    iron: f64,
    heat: f64,
    density: [f64; 3],
    old: Option<Old>,
    sun_density: Option<f64>,
    age: Option<f64>,
    #[serde(default)]
    since: u8,
}

/// A row's make-up in the worlds before real gravity (`world::REAL`), where it was not its own: its `iron`,
/// `density` and the pull its rock was weighed with.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
struct Old {
    iron: f64,
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
    /// Its make-up in a world before `world::REAL` where its row says another (`makeup_at`).
    pub old: Option<Makeup>,
    /// The sun's mean density, kg/m3 (the home's row; `SUN_DENSITY` if left out): its mass, with the radius the
    /// sky gives it, sets every year by Kepler's law from `world::REAL` on.
    pub sun_density: Option<f64>,
    /// The first world generation it is in (`world::GENERATION`; 0: every world): a world of an older one is
    /// made, saved and sent without it (`of`).
    pub since: u8,
    /// Its sidereal day in the home's days (a body that circles the sun and is not home; None: it does not turn,
    /// 0163-n-bodies): the giant's bands turn with it and its field's spin is it.
    pub day: Option<f64>,
    /// The colour its point of light shines in when its disc is under a pixel (`sky::point_light`).
    pub colour: [f32; 3],
}

/// The real Sun's mean density, kg/m3: the sun's when the home's row names none.
pub const SUN_DENSITY: f64 = 1408.0;

impl Row {
    /// Its make-up in a world of generation `gen`: its `old` one before real gravity, else its row's.
    pub fn makeup_at(&self, gen: u8) -> &Makeup {
        match &self.old {
            Some(o) if gen < crate::world::REAL => o,
            _ => &self.makeup,
        }
    }
    /// A gas body (step 6a): no blocks, so no grid, map, chunks, bake or save data; drawn by the sky's shader.
    pub fn gas(&self) -> bool {
        self.makeup.kind == Kind::Gas
    }
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
    if f.body.len() > MAX {
        return Err(format!("{} bodies; a world holds at most {MAX}", f.body.len()));
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
        // a generation's bodies are the rows down to its last (`of`): the home in every world, a row never older
        // than the one above it
        if r.home && r.since > 0 {
            return Err(format!("body '{}': the home's since is 0, not {}", r.name, r.since));
        }
        if let Some(p) = rows.last().filter(|p| p.since > r.since) {
            return Err(format!("body '{}': since {} is under the row above's {}: new rows go at the end", r.name, r.since, p.since));
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
        // a baked moon of a moon is not built: its sun's height (`luna::sun_high`) would take its orbit in its parent's
        // radii for the home's
        if let Some(p) = parent.filter(|&p| rows[p].parent.is_some() && r.stages.is_some()) {
            return Err(format!("body '{}': a baked moon of a moon ('{}') is not built yet", r.name, rows[p].name));
        }
        // its ground clear of its parent's (radii in the home's: a size is the home's radius over the body's), with
        // room for both grounds' relief: `AMP` over the home's radius, the same share of each body's (a bake
        // stretches its relief with its size)
        if let (Some(p), Some((o, _)), Some(k)) = (parent, orbit, r.size) {
            let share = crate::world::AMP / (rows[0].n as f64 * crate::world::BLOCK / (2.0 * std::f64::consts::PI / 3.0).sqrt());
            let (rp, rm) = (1.0 / rows[p].size.unwrap_or(1.0), 1.0 / k);
            if (o - 1.0) * rp - rm <= share * (rp + rm) {
                return Err(format!("body '{}': its ground would be inside its parent '{}' at an orbit of {o} of its radii", r.name, rows[p].name));
            }
        }
        let makeup = Makeup { kind: r.kind, iron: r.iron, heat: r.heat, density: r.density, weight_g: None };
        check(&makeup).map_err(|e| format!("body '{}': {e}", r.name))?;
        let old = r.old.as_ref().map(|o| Makeup { iron: o.iron, density: o.density, weight_g: o.weight_g, ..makeup });
        if let Some(o) = &old {
            check(o).map_err(|e| format!("body '{}': its old make-up: {e}", r.name))?;
        }
        match r.sun_density {
            Some(_) if !r.home => return Err(format!("body '{}': only the home's row names the sun's density", r.name)),
            Some(d) if !(d > 0.0 && d.is_finite()) => return Err(format!("body '{}': sun_density {d} is kg/m3, over 0", r.name)),
            _ => {}
        }
        // gas has no blocks (step 6a): no ground, generator or bake; a rocky body has its ground
        let gas = r.kind == Kind::Gas;
        let mut ground = [crate::blocks::AIR; 3];
        match (gas, &r.ground, r.generator) {
            (true, None, None) if r.stages.is_none() => {}
            (true, None, None) => return Err(format!("body '{}': a gas body has no bake (no stages)", r.name)),
            (true, ..) => return Err(format!("body '{}': a gas body has no ground (no ground or generator)", r.name)),
            (false, Some(names), Some(_)) => {
                for (g, name) in ground.iter_mut().zip(names) {
                    *g = BLOCKS.placeable().find(|d| &d.name == name).ok_or(format!("body '{}': no block '{name}'", r.name))?.id;
                }
            }
            (false, ..) => return Err(format!("body '{}': a rocky body has its ground and generator", r.name)),
        }
        match r.day {
            Some(d) if !(d > 0.0 && d.is_finite()) => return Err(format!("body '{}': day {d} is its day in the home's days, over 0", r.name)),
            Some(_) if r.home || parent.is_some() => return Err(format!("body '{}': only a body that circles the sun and is not home has a day of its own", r.name)),
            _ => {}
        }
        if let Some(c) = r.colour.filter(|c| !c.iter().all(|v| (0.0..=1.0).contains(v))) {
            return Err(format!("body '{}': colour {c:?} is red, green and blue, each from 0 to 1", r.name));
        }
        if let Some(s) = &r.stages {
            crate::bake::check(s, parent.is_some()).map_err(|e| format!("body '{}': {e}", r.name))?;
        }
        // an icy body's crust (`bake::icy`, step 6c): Luna's stages on ice, with no lava to flood it
        let icy = r.kind == Kind::Icy && r.stages.is_some();
        match &r.stages {
            Some(s) if icy && s[0] != Stage::Crust => return Err(format!("body '{}': an icy body bakes on a 'crust', not on plates", r.name)),
            Some(s) if icy && s.contains(&Stage::Volcanism) => return Err(format!("body '{}': stage 'volcanism' is not built yet on an icy crust", r.name)),
            _ => {}
        }
        // a planet's crust (`bake::mars`) reads its surface's age; no other bake does
        let planet = r.stages.as_ref().is_some_and(|s| s[0] == Stage::Crust) && parent.is_none() && !icy;
        let planet = match (planet, r.age.unwrap_or(1.0)) {
            (true, a) if a > 0.0 && a <= 1.0 => Some(Planet { name: r.name.clone(), age: a, icy: None, cones: false }),
            (true, a) => return Err(format!("body '{}': age {a} is the share of the craters its surface has collected, over 0 and at most 1", r.name)),
            (false, _) if r.age.is_some() => return Err(format!("body '{}': age is read only by a planet's crust (stages starting with 'crust' on a body that circles the sun)", r.name)),
            // kept by its row's name and hash as a planet's map is (`mars::file`), baked on ice
            _ if icy => Some(Planet { name: r.name.clone(), age: 1.0, icy: Some(0.0), cones: false }),
            _ => None,
        };
        // the sun at its highest over its equator (`luna::sun_high`): a moon of a body that is not the home sees the
        // home's moon's sun disc smaller by its system's distance from the sun over the home's
        let high = match parent.filter(|&t| t != 0) {
            Some(t) => {
                let far = rows[0].sun.unwrap_or(1.0) / (rows[t].sun.unwrap_or(1.0) / rows[t].size.unwrap_or(1.0));
                crate::sky::INCLINATION + (crate::bake::luna::sun_high(None, None) - crate::sky::INCLINATION) * far
            }
            None => crate::bake::luna::sun_high(r.size, orbit),
        };
        let planet = planet.map(|p| Planet { icy: p.icy.map(|_| high), ..p });
        rows.push(Row { name: r.name, parent, home: r.home, bake: r.stages.map(|s| Bake(s, high, planet)), words: r.words.iter().map(|w| w.to_lowercase()).collect(), n, generator: r.generator.unwrap_or(Generator::Hills), ground, air: r.air, orbit, size: r.size, sun: r.sun, makeup, old, sun_density: r.sun_density, since: r.since, day: r.day, colour: r.colour.unwrap_or([0.7; 3]) });
    }
    if moon(&rows).is_none() {
        return Err(format!("body '{}': the home body needs a moon (its first sizes the sun)", rows[0].name));
    }
    Ok(rows)
}

/// The most bodies a world holds (step 6b; was 4, 0163-n-bodies): the renderer's (client `gpu::MAX_BODIES`, WGSL
/// `MAX_BODIES`, a body's 4 bits in a particle's and a water quad's word, 8 in a chunk's), which `World::gravity`
/// keeps on the stack too.
pub const MAX: usize = 16;

/// A welcome's body list against this build's rows for the world's generation (the welcome has no version):
/// one of another length is a server of another build, refused as `world::known_generation` refuses a newer world.
pub fn known_bodies(n: usize, gen: u8) -> Result<(), String> {
    let mine = of(gen).len();
    if n == mine { Ok(()) } else { Err(format!("the server has {n} bodies, this game {mine}: update the game")) }
}

/// The rows of a world of generation `gen`: those whose `since` it has reached (they are the first rows, `parse`).
pub fn of(gen: u8) -> &'static [Row] {
    &BODIES[..BODIES.iter().take_while(|r| r.since <= gen).count()]
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

/// A makeup the interior can be derived from: a rocky or gas body, a core inside it, heat 0..1, densities
/// 500..30000 kg/m3 growing going down, a positive pull. On a gas body (step 6a) `iron` is the conducting fluid's
/// top over the radius, `heat` leaves its solid core 1 - heat of that, and `density` is the gas's, the gas's and
/// the fluid's (the core reckoned at the fluid's): its mass follows from them as a rocky body's does.
fn check(m: &Makeup) -> Result<(), String> {
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
    fn the_data_file_has_terra_luna_and_mars() {
        let names: Vec<&str> = BODIES.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["Terra", "Luna", "Mars", "Jupiter", "Callisto"]);
        // a world of generation 4 or before has the first two, from 5 Mars too, from 6 the giant, from 7 its icy moon
        // (ADR 0160's rule: old worlds as they were)
        assert_eq!([0, 4, 5, 6, 7].map(|g| of(g).len()), [2, 2, 3, 4, 5]);
        assert_eq!(BODIES.iter().map(|r| r.since).collect::<Vec<_>>(), [0, 0, 5, 6, 7]);
        assert!(known_bodies(2, 4).is_ok() && known_bodies(3, 5).is_ok() && known_bodies(4, 6).is_ok() && known_bodies(5, 7).is_ok());
        assert!(known_bodies(3, 6).unwrap_err().contains("the server has 3 bodies, this game 4"));
        let e = known_bodies(2, 5).unwrap_err();
        assert!(e.contains("the server has 2 bodies, this game 3") && e.contains("update the game"), "{e}");
        assert_eq!(find("mars"), Some(2));
        let two = of(4);
        assert_eq!((two[0].generator, two[0].air, two[0].orbit), (Generator::Continents, true, None));
        assert_eq!((two[1].generator, two[1].air, two[1].orbit), (Generator::Hills, false, Some((60.0, 0.0))));
        assert_eq!(two[1].ground[0], BLOCKS.id("regolith"));
        assert_eq!(two.iter().map(|r| (r.parent, r.home, r.bake.as_ref().map(|b| b.0.clone()))).collect::<Vec<_>>(), [(None, true, Some(Stage::TERRA.to_vec())), (Some(0), false, Some(Stage::LUNA.to_vec()))]);
        assert_eq!((moon(two), sizes(two, 1536, 512), moon_columns(6144)), (Some(1), vec![1536, 512], 1664));
        assert_eq!(sizes(&BODIES, 6144, 1664), [6144, 1664, 3328, 67584, 1536]);
        assert_eq!((two[0].n, two[1].n), (6144, 1664), "6144 / 3.7 is 1660.5: 52 chunks");
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
        // `since` (generation 5): the home's is 0, and rows never go back in it; refused, saying so
        let later = |s: &str| s.replacen("sun = 100.0\n", "sun = 100.0\nsince = 5\n", 1);
        for (bad, says) in [
            (later(&good), "the home's since is 0, not 5"),
            (later(&b("parent = \"A\"\norbit = 3.0\nsize = 4.0\nsince = 5\n")), "the home's since is 0, not 5"),
            (format!("{}{}", b("parent = \"A\"\norbit = 3.0\nsize = 4.0\nsince = 5\n"), row("C", "parent = \"A\"\norbit = 5.0\nsize = 4.0\n")), "since 0 is under the row above's 5"),
        ] {
            let e = parse(&bad).unwrap_err();
            assert!(e.contains(says), "{says}: {e}");
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
        assert_eq!(ok[2].bake.as_ref().unwrap().2, Some(Planet { name: "D".into(), age: 0.5, icy: None, cones: false }));
        assert_eq!(planet("stages = [\"crust\", \"impacts\"]\n").unwrap()[2].bake.as_ref().unwrap().2.as_ref().map(|p| p.age), Some(1.0), "an age of 1 when not given");
        // a moon's crust is Luna's kind: no age, no planet
        assert_eq!(stages("[\"crust\", \"impacts\"]").unwrap()[1].bake.as_ref().unwrap().2, None);
    }

    /// Mars's row (planet-model.md step 5): a planet's crust with the developer's stage list, 1.85 times smaller
    /// than Terra, in the worlds of generation 5 on.
    #[test]
    fn the_mars_row_parses_and_its_list_is_accepted() {
        let m = &BODIES[2];
        assert_eq!(m.since, 5);
        assert_eq!((m.name.as_str(), m.parent, m.home, m.n, m.orbit), ("Mars", None, false, 3328, None));
        assert_eq!(m.n / crate::world::CS, 104, "13 x 8 chunks, as Luna's 52 are 13 x 4");
        let b = m.bake.as_ref().unwrap();
        assert_eq!(b.0, [Stage::Crust, Stage::Impacts, Stage::Volcanism, Stage::Erosion, Stage::Ice]);
        assert_eq!(b.2, Some(Planet { name: "Mars".into(), age: 1.0, icy: None, cones: false }));
        assert_eq!(b.version(), crate::bake::mars::VERSION);
        assert_eq!((m.makeup.kind, m.makeup.iron, m.makeup.heat, m.makeup.density), (Kind::Rocky, 0.5, 1.0, [2900.0, 3500.0, 6700.0]));
        assert_eq!(m.sun, Some(1.5 * 23500.0 * 1.85), "1.5 AU in its radii");
    }

    /// The gas giant (planet-model.md "Gas planets", step 6a): 11 times Terra's radius, 5 times her distance from the
    /// sun, a fast spin, in the worlds of generation 6 on; no bake, no ground, no grid of its own (`Row::gas`).
    #[test]
    fn the_giant_row_is_a_gas_body() {
        let j = &BODIES[3];
        assert_eq!((j.name.as_str(), j.parent, j.home, j.since, j.bake.is_none()), ("Jupiter", None, false, 6, true));
        assert_eq!(j.makeup.kind, Kind::Gas);
        assert!(j.gas() && !BODIES[..3].iter().any(Row::gas) && !BODIES[4].gas());
        assert_eq!(j.n, 6144 * 11, "11 Terra radii");
        assert_eq!(j.sun, Some(5.0 * 23500.0 / 11.0), "5 AU in its radii");
        assert_eq!(j.day, Some(0.4), "a day of 0.4 Terra's (the real one's 10 hours)");
        assert_eq!(j.ground, [crate::blocks::AIR; 3], "no ground: gas has no blocks");
        assert_eq!(find("jupiter"), Some(3));
        // the colours its point of light shines in (and the others')
        assert!(BODIES.iter().all(|r| r.colour.iter().all(|c| (0.0..=1.0).contains(c))));
        assert!(BODIES[2].colour[0] > BODIES[2].colour[2] && BODIES[0].colour[2] > BODIES[0].colour[0], "red Mars, blue Terra");
    }

    /// The giant's icy moon (planet-model.md step 6c): the first body whose parent is neither the sun nor the home and
    /// the first of kind icy, in the worlds of generation 7 on; its map named by its row and hash as Mars's is, baked
    /// on ice (`bake::icy`), under a sun five times farther than the home's moon's.
    #[test]
    fn the_icy_moon_row_parses_and_is_the_giants() {
        let c = &BODIES[4];
        assert_eq!((c.name.as_str(), c.parent, c.home, c.since, c.makeup.kind), ("Callisto", Some(3), false, 7, Kind::Icy));
        assert_eq!((c.n, c.n / crate::world::CS, c.orbit, c.size), (1536, 48, Some((8.0, 0.0)), Some(4.0)));
        assert_eq!([4, 5, 6, 7].map(|g| of(g).len()), [2, 3, 4, 5], "a world of generation 6 has four bodies, of 7 five");
        assert!(known_bodies(5, 7).is_ok() && known_bodies(4, 7).is_err());
        assert_eq!(find("callisto"), Some(4));
        assert_eq!(moon(&BODIES), Some(1), "the home's moon is still Luna");
        assert_eq!(sizes(&BODIES, 6144, 1664), [6144, 1664, 3328, 67584, 1536]);
        let b = c.bake.as_ref().unwrap();
        assert_eq!(b.0, [Stage::Crust, Stage::Impacts, Stage::Ice]);
        assert_eq!(b.2, Some(Planet { name: "Callisto".into(), age: 1.0, icy: Some(b.1), cones: false }));
        assert_eq!(b.version(), crate::bake::icy::VERSION);
        assert!(b.file(1, 1536).to_str().unwrap().contains("-callisto-"), "named by its row: {:?}", b.file(1, 1536));
        // the sun at its highest over its equator: the orbit's tilt and the sun's disc, five times smaller than from
        // the home's moon (the giant's 5 AU), not the disc of a sun at Luna's distance
        let luna = BODIES[1].bake.as_ref().unwrap().1;
        let disc = luna - crate::sky::INCLINATION;
        assert!((b.1 - crate::sky::INCLINATION - disc / 5.0).abs() < 1e-9, "{} against {}", b.1, crate::sky::INCLINATION + disc / 5.0);
    }

    /// A moon of a parent that is not the home (step 6c): accepted with Luna's family of stages; refused, saying so,
    /// when its parent is a later row or itself, when it would lie inside its parent, or when an icy crust lists
    /// volcanism.
    #[test]
    fn a_moon_of_another_parent_is_accepted_and_bad_ones_refused() {
        let rocky = "generator = \"hills\"\nground = [\"stone\", \"stone\", \"stone\"]\n";
        let home = format!("[[body]]\nname = \"A\"\nparent = \"sun\"\nhome = true\nn = 512\n{rocky}air = false\nsun = 100.0\n{MAKEUP}[[body]]\nname = \"B\"\nparent = \"A\"\norbit = 3.0\nsize = 4.0\n{rocky}air = false\n{MAKEUP}");
        let giant = format!("{home}[[body]]\nname = \"G\"\nparent = \"sun\"\nsun = 300.0\nsize = 0.5\nair = false\nkind = \"gas\"\niron = 0.8\nheat = 0.9\ndensity = [500.0, 500.0, 1700.0]\n");
        let icy = "kind = \"icy\"\niron = 0.5\nheat = 0.0\ndensity = [950.0, 1700.0, 3000.0]\n";
        let moon = |name: &str, extra: &str| format!("[[body]]\nname = \"{name}\"\n{rocky}air = false\n{icy}{extra}");
        let ok = parse(&format!("{giant}{}", moon("C", "parent = \"G\"\norbit = 10.0\nsize = 8.0\nstages = [\"crust\", \"impacts\", \"ice\"]\n"))).unwrap();
        assert_eq!((ok[3].parent, ok[3].makeup.kind, ok[3].n), (Some(2), Kind::Icy, 64));
        assert_eq!(ok[3].bake.as_ref().unwrap().2.as_ref().map(|p| (&p.name[..], p.age)), Some(("C", 1.0)));
        // its map's file names every input its bake reads: the sun's height over it comes from the giant's sun and
        // size, so another giant is another file, and the same rows the same one
        let file = |g: &str| parse(&format!("{}{}", g, moon("C", "parent = \"G\"\norbit = 10.0\nsize = 8.0\nstages = [\"crust\", \"impacts\", \"ice\"]\n"))).unwrap()[3].bake.as_ref().unwrap().file(1, 64);
        assert_eq!(file(&giant), file(&giant));
        assert_ne!(file(&giant), file(&giant.replace("sun = 300.0", "sun = 600.0")), "the giant farther from the sun");
        assert_ne!(file(&giant), file(&giant.replace("size = 0.5", "size = 0.25")), "a larger giant");
        for (bad, says) in [
            (format!("{home}{}{giant_row}", moon("C", "parent = \"G\"\norbit = 10.0\nsize = 8.0\n"), giant_row = &giant[home.len()..]), "its parent 'G' is not a row above it"),
            (format!("{giant}{}", moon("C", "parent = \"C\"\norbit = 10.0\nsize = 8.0\n")), "its parent 'C' is not a row above it"),
            // the giant is 2 home radii, the moon an eighth of one: 1.05 of the giant's radii is 0.1 over its cloud tops
            (format!("{giant}{}", moon("C", "parent = \"G\"\norbit = 1.05\nsize = 8.0\n")), "inside its parent"),
            // grounds that just touch (1.0625 of the giant's radii) overlap by their relief
            (format!("{giant}{}", moon("C", "parent = \"G\"\norbit = 1.0625\nsize = 8.0\n")), "inside its parent"),
            (format!("{giant}{}", moon("C", "parent = \"G\"\norbit = 2.0\nsize = 8.0\n")), "inside its parent"),
            // a baked moon of a moon: its sun's height would be worked out from the wrong orbit
            (format!("{giant}{}", moon("C", "parent = \"B\"\norbit = 20.0\nsize = 32.0\nstages = [\"crust\", \"impacts\", \"ice\"]\n")), "a baked moon of a moon"),
            (format!("{giant}{}", moon("C", "parent = \"G\"\norbit = 10.0\nsize = 8.0\nstages = [\"crust\", \"impacts\", \"ice\", \"volcanism\"]\n")), "'volcanism' is not built yet on an icy crust"),
            (format!("{giant}{}", moon("C", "parent = \"G\"\norbit = 10.0\nsize = 8.0\nstages = [\"crust\", \"impacts\"]\nage = 0.5\n")), "age is read only by a planet's crust"),
        ] {
            let e = parse(&bad).expect_err(says);
            assert!(e.contains(says) && e.contains("body 'C'"), "{says}: {e}");
        }
    }

    #[test]
    fn a_gas_row_is_refused_with_what_it_cannot_have() {
        let rocky = "generator = \"hills\"\nground = [\"stone\", \"stone\", \"stone\"]\n";
        let home = format!("[[body]]\nname = \"A\"\nparent = \"sun\"\nhome = true\nn = 512\n{rocky}air = false\nsun = 100.0\n{MAKEUP}[[body]]\nname = \"B\"\nparent = \"A\"\norbit = 3.0\nsize = 4.0\n{rocky}air = false\n{MAKEUP}");
        let gas = |extra: &str| format!("{home}[[body]]\nname = \"G\"\nparent = \"sun\"\nsun = 300.0\nsize = 0.5\nair = false\nkind = \"gas\"\niron = 0.8\nheat = 0.9\ndensity = [500.0, 500.0, 1700.0]\n{extra}");
        let ok = parse(&gas("day = 0.4\n")).unwrap();
        assert!(ok[2].gas() && ok[2].bake.is_none() && ok[2].n == 1024);
        for (extra, says) in [
            ("stages = [\"crust\", \"impacts\"]\n", "a gas body has no bake"),
            ("ground = [\"stone\", \"stone\", \"stone\"]\n", "a gas body has no ground"),
            ("generator = \"hills\"\n", "a gas body has no ground"),
            ("day = 0.0\n", "day 0"),
            ("day = nan\n", "day NaN"),
            ("day = inf\n", "day inf"),
            ("colour = [-0.1, 0.5, 0.5]\n", "colour"),
            ("colour = [1.5, 0.5, 0.5]\n", "colour"),
            ("colour = [0.5, nan, 0.5]\n", "colour"),
        ] {
            let e = parse(&gas(extra)).expect_err(extra);
            assert!(e.contains(says) && e.contains("body 'G'"), "{extra}: {e}");
        }
        // a rocky body still needs its ground
        let e = parse(&home.replacen("ground = [\"stone\", \"stone\", \"stone\"]\n", "", 1)).unwrap_err();
        assert!(e.contains("ground"), "{e}");
    }

    const MAKEUP: &str = "kind = \"rocky\"\niron = 0.5\nheat = 0.5\ndensity = [2700.0, 3300.0, 7900.0]\n";

    /// Room for many bodies (step 6b): a world of `MAX` (16) rows builds, every body in its own frame; a 17th row
    /// is refused by the file's reader and by `World::of`, each with a message naming the limit.
    #[test]
    fn sixteen_bodies_build_and_a_seventeenth_is_refused() {
        let file = |k: usize| {
            let mut s = format!("[[body]]\nname = \"A\"\nparent = \"sun\"\nhome = true\nn = 512\ngenerator = \"hills\"\nground = [\"stone\", \"stone\", \"stone\"]\nair = false\nsun = 100.0\n{MAKEUP}");
            s += &format!("[[body]]\nname = \"B\"\nparent = \"A\"\norbit = 3.0\nsize = 4.0\ngenerator = \"hills\"\nground = [\"stone\", \"stone\", \"stone\"]\nair = false\n{MAKEUP}");
            for i in 2..k {
                s += &format!("[[body]]\nname = \"P{i}\"\nparent = \"sun\"\nsun = {}\nsize = 4.0\ngenerator = \"hills\"\nground = [\"stone\", \"stone\", \"stone\"]\nair = false\n{MAKEUP}", 400.0 + 100.0 * i as f64);
            }
            s
        };
        let rows = parse(&file(MAX)).unwrap();
        assert_eq!(MAX, 16);
        let w = crate::world::World::of(&rows, rows.iter().map(|r| (r.n, None)).collect(), 1, 0.0, crate::world::GENERATION);
        assert_eq!(w.bodies.len(), 16);
        // each body's grid is its own frame, and the last one's gravity is its own
        for i in 0..16 {
            assert_eq!(w.frame_at(w.bodies[i].centre + glam::DVec3::Y * w.bodies[i].radius), i, "body {i}");
        }
        let top = w.bodies[15].centre + glam::DVec3::Z * w.bodies[15].radius;
        assert!(w.gravity(top).normalize().dot(-glam::DVec3::Z) > 0.99);
        let e = parse(&file(MAX + 1)).unwrap_err();
        assert!(e.contains("17 bodies") && e.contains("at most 16"), "{e}");
        let mut more = rows.clone();
        more.push(Row { name: "P16".into(), ..rows[15].clone() });
        let e = std::panic::catch_unwind(|| crate::world::World::of(&more, more.iter().map(|r| (r.n, None)).collect(), 1, 0.0, crate::world::GENERATION)).map(|_| ()).unwrap_err();
        let e = e.downcast_ref::<String>().cloned().unwrap_or_default();
        assert!(e.contains("17 bodies") && e.contains("at most 16"), "{e}");
    }

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
            (with("rocky", "cheese"), "no such kind", "kind"),
        ] {
            let e = parse(&bad).expect_err(why);
            assert!(e.contains(says), "{why}: {e}");
        }
    }
}
