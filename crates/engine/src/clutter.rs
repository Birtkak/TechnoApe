//! Clutter (docs/design/clutter-and-pickups.md, ADR 0082): the small things on the ground, a pure function of the
//! seed, the column and the season, drawn as still box models in the chunk's mesh (ADR 0068, 0081). Nothing is
//! stored for them; a column with an edit has none. Small plants are rows too (ADR 0091): painted flat planes
//! crossed in the cell, placed the same way, scaled by the Vegetation setting, swayed and tinted by the shader.
use crate::models::Model;
use crate::palette::Palette;
use crate::world::{mix, pidx, unit, Block, Body, HeightTile, Key, World, CS, P};
use glam::IVec3;
use serde::Deserialize;
use std::sync::LazyLock;

/// The seasons a row names, in `sky::season_at`'s order.
pub const SEASONS: [&str; 4] = ["spring", "summer", "autumn", "winter"];
/// A model's alternates: `<name>.1.toml` to `.4`, beside `<name>.toml`.
pub const ALTERNATES: usize = 4;

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Drawn only: no collision, no shadow, thinned by the Clutter setting.
    Decoration,
    /// Taken by hand (7b): the server answers for it.
    Pickup,
    /// crossed painted planes (ADR 0091), thinned by the Vegetation setting
    Plant,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Near {
    /// Within `TREE` columns of a standing trunk.
    Tree,
    /// Not near a trunk.
    Open,
}
/// How near a trunk `near = "tree"` is, in columns.
pub const TREE: i32 = 3;
/// The farthest a row's `water` may reach, columns (a face's own columns; a bank and its gravel bar).
pub const WATER_MAX: i32 = 8;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    clutter: Vec<RawRow>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRow {
    name: String,
    kind: Kind,
    #[serde(default)]
    gives: Option<(String, u16)>,
    on: Vec<String>,
    #[serde(default)]
    near: Option<Near>,
    #[serde(default)]
    season: Vec<String>,
    #[serde(default)]
    regrow: Option<String>,
    #[serde(default)]
    planes: Option<u8>,
    #[serde(default)]
    looks: std::collections::BTreeMap<String, Vec<String>>,
    #[serde(default)]
    wades: bool,
    #[serde(default)]
    water: Option<i32>,
    #[serde(default)]
    over: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub name: String,
    pub kind: Kind,
    /// A pick-up's item by name and how many: a name, not an id, since the items' rows read the minerals' and
    /// rocks', which read the biomes', which read these (`item` resolves it; a test holds every name to a row).
    pub gives: Option<(String, u16)>,
    /// The ground blocks it lies on.
    pub on: Vec<u16>,
    pub near: Option<Near>,
    /// A bit per season it is there (`SEASONS`), all four when the row names none.
    pub seasons: u8,
    /// A pick-up taken comes back the next year (absent: never, a stone).
    pub regrow: bool,
    /// a plant's crossed planes (2 to 4); 0 for a model
    pub planes: u8,
    /// a plant's other paintings `<name>.<look>.png` and the parts of the year (`part`, a bit each) they show in
    pub looks: Vec<(String, u8)>,
    /// a plant that also stands in fresh water up to `WADE` deep (reeds)
    pub wades: bool,
    /// Where it lies, besides its ground (ADR 0097): within this many columns of water (a river, a lake, the
    /// sea), or over one of `over`'s rocks (the rock under the soil band). Neither set: anywhere its ground is.
    pub water: Option<i32>,
    pub over: Vec<u16>,
}

/// Reads and checks the rows: names unique, ground blocks and items that exist, a pick-up gives something and a
/// decoration does not, seasons by name, `regrow` "year" on a pick-up only.
pub fn parse(text: &str) -> Result<Vec<Row>, String> {
    let f: Raw = toml::from_str(text).map_err(|e| e.to_string())?;
    let mut out: Vec<Row> = Vec::new();
    for r in f.clutter {
        let who = format!("clutter '{}'", r.name);
        if out.iter().any(|o| o.name == r.name) {
            return Err(format!("{who}: named twice"));
        }
        let on = r.on.iter().map(|n| crate::blocks::BLOCKS.placeable().find(|d| &d.name == n).map(|d| d.id).ok_or(format!("{who}: no block '{n}'"))).collect::<Result<Vec<_>, _>>()?;
        let gives = match (&r.gives, r.kind) {
            (Some((n, k)), Kind::Pickup) if *k >= 1 => Some((n.clone(), *k)),
            (None, Kind::Decoration | Kind::Plant) => None,
            _ => return Err(format!("{who}: a pick-up gives an item and at least one, a decoration nothing")),
        };
        let mut seasons = if r.season.is_empty() { 15 } else { 0 };
        for s in &r.season {
            seasons |= 1 << SEASONS.iter().position(|n| n == s).ok_or(format!("{who}: no season '{s}'"))?;
        }
        let regrow = match (r.regrow.as_deref(), r.kind) {
            (None, _) => false,
            (Some("year"), Kind::Pickup) => true,
            _ => return Err(format!("{who}: regrow is \"year\", on a pick-up")),
        };
        if r.water.is_some_and(|w| !(1..=WATER_MAX).contains(&w)) {
            return Err(format!("{who}: water is 1 to {WATER_MAX} columns"));
        }
        let over = r.over.iter().map(|n| crate::blocks::BLOCKS.placeable().find(|d| &d.name == n).map(|d| d.id).ok_or(format!("{who}: no rock '{n}'"))).collect::<Result<Vec<_>, _>>()?;
        let planes = match (r.planes, r.kind) {
            (None, Kind::Plant) => 2,
            (Some(n @ 2..=4), Kind::Plant) => n,
            (None, _) => 0,
            _ => return Err(format!("{who}: planes are 2 to 4, on a plant")),
        };
        if !r.looks.is_empty() && r.kind != Kind::Plant {
            return Err(format!("{who}: looks are a plant's"));
        }
        if r.wades && r.kind != Kind::Plant {
            return Err(format!("{who}: wades is a plant's"));
        }
        let mut looks = Vec::new();
        for (n, parts) in &r.looks {
            let mut m = 0u8;
            for p in parts {
                m |= part_bits(p).ok_or(format!("{who}: look '{n}': no part of the year '{p}' (a season, or early or late one)"))?;
            }
            looks.push((n.clone(), m));
        }
        out.push(Row { name: r.name, kind: r.kind, gives, on, near: r.near, seasons, regrow, planes, looks, wades: r.wades, water: r.water, over });
    }
    Ok(out)
}

impl Row {
    /// A pick-up's item id and how many (None for decoration, or an item name no row has).
    pub fn item(&self) -> Option<(u16, u16)> {
        self.gives.as_ref().and_then(|(n, k)| Some((crate::items::ITEMS.named(n)?, *k)))
    }
}

pub static CLUTTER: LazyLock<Vec<Row>> = LazyLock::new(|| parse(include_str!("../../../data/clutter.toml")).unwrap_or_else(|e| panic!("data/clutter.toml: {e}")));

/// What lies on a column: its row and the hash that picks its alternate, turn and place in the cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Thing {
    pub row: usize,
    pub h: u64,
}
impl Thing {
    /// Its alternate of `n`, its turn in sixteenths, its place in the cell (two numbers 0..16).
    pub fn shape(&self, n: usize) -> (usize, u32, u32, u32) {
        let h = self.h;
        (((h >> 16) % n.max(1) as u64) as usize, (h >> 24) as u32 & 15, (h >> 28) as u32 & 15, (h >> 32) as u32 & 15)
    }
}

/// The model that lies on column (face, i, j) of a body with `seed`, of biome `biome` and generated top block
/// `ground`, in `season` (0..4): one roll over the biome's models of that ground and season, pick-ups at their
/// full weight (what the server answers for), decoration at `density` times its own (the Clutter setting: off 0,
/// few 0.25, normal 1, many 4), so a lower setting is a subset of a higher one. `trees`: the trunks near the
/// column. Plants have a roll of their own (`plant`).
#[allow(clippy::too_many_arguments)]
pub fn pick(body: &Body, seed: u64, face: u8, i: i32, j: i32, biome: u8, ground: u16, season: u8, density: f32, trees: &[(i32, i32)]) -> Option<Thing> {
    let h = roll(seed, face, i, j);
    // a sample over an ore body first: it lies on any ground (ADR 0138)
    if let Some(t) = sample(body, seed, face, i, j) {
        return Some(t);
    }
    // a model's place rule (ADR 0097: near water, over a rock) last
    choose(h, &[(Kind::Pickup, 1.0), (Kind::Decoration, density)], i, j, biome, ground, season, trees).filter(|t| wet_or_over(body, face, i, j, &CLUTTER[t.row]))
}

/// Per mineral kind (`ore::MINERALS`), the pick-up row that gives its `sample`: the row of a sample lying on
/// the ground over its bodies (ADR 0138).
pub static SAMPLES: LazyLock<Vec<Option<usize>>> = LazyLock::new(|| {
    let gives = |s: &String| CLUTTER.iter().position(|r| r.kind == Kind::Pickup && r.gives.as_ref().is_some_and(|g| &g.0 == s));
    crate::ore::MINERALS.kinds.iter().map(|k| k.sample.as_ref().map(|s| gives(s).unwrap_or_else(|| panic!("data/clutter.toml: no pick-up gives '{s}', kind '{}''s sample", k.name)))).collect()
});

/// The sample lying on column (face, i, j), if one does: a roll per body whose spread (`OreBody::spread`)
/// covers it, against `1 - q` over `ore::SAMPLE_EVERY_M2`, so densest over the middle and none at the rim.
pub fn sample(body: &Body, seed: u64, face: u8, i: i32, j: i32) -> Option<Thing> {
    let h = roll(seed, face, i, j);
    body.tile_ores(face, i.div_euclid(CS), j.div_euclid(CS)).iter().find_map(|&k| {
        let o = &body.ores[k as usize];
        let row = SAMPLES[o.kind as usize]?;
        let q = o.spread(i, j)?;
        ((unit(mix(h ^ SAMPLE_ROLL ^ k as u64)) as f64) < (1.0 - q) / crate::ore::SAMPLE_EVERY_M2).then_some(Thing { row, h })
    })
}
const SAMPLE_ROLL: u64 = 0x5a3b_1e00_0063;

/// The plant that grows on the column (ADR 0091), as `pick` but by its own roll at `vegetation` times the
/// biome's weights (`VEGETATION`): neither the Clutter setting nor a seasonal model moves a plant, and a lower
/// Vegetation setting is a subset of a higher one. A plant may stand over a model.
#[allow(clippy::too_many_arguments)]
pub fn plant(seed: u64, face: u8, i: i32, j: i32, biome: u8, ground: u16, season: u8, vegetation: f32, trees: &[(i32, i32)]) -> Option<Thing> {
    choose(mix(roll(seed, face, i, j) ^ PLANT_ROLL), &[(Kind::Plant, vegetation)], i, j, biome, ground, season, trees)
}
const PLANT_ROLL: u64 = 0x91a4_7000_0091;

/// One roll `h` over the biome's rows of `kinds` (each at its weight times its density, in that order) that fit
/// the ground and the season, then which row of the kind by a second roll; the `near` rule last.
#[allow(clippy::too_many_arguments)]
fn choose(h: u64, kinds: &[(Kind, f32)], i: i32, j: i32, biome: u8, ground: u16, season: u8, trees: &[(i32, i32)]) -> Option<Thing> {
    let b = crate::biomes::ROWS.biomes.get(biome as usize)?;
    let rows = &*CLUTTER;
    let fits = |r: &Row| r.on.contains(&ground) && r.seasons >> season & 1 == 1;
    let mut u = unit(h);
    let mut found = None;
    for &(k, d) in kinds {
        let w = b.clutter.iter().filter(|c| rows[c.0].kind == k && fits(&rows[c.0])).map(|c| c.1 / 100.0).sum::<f32>();
        if u < w * d {
            found = Some((k, w));
            break;
        }
        u -= w * d;
    }
    let (kind, w) = found?;
    // which of that kind, by a second roll over their weights; four plants in five take their patch's roll
    // (about 12 columns across, ragged at the edges), so a kind grows in drifts of its own
    let r = if kind == Kind::Plant && unit(mix(h ^ 0xd41f7)) < 0.8 {
        let jig = |k: u64| ((mix(h ^ k) & 3) as i32) - 1;
        mix(PLANT_ROLL ^ ((i + jig(0x11)).div_euclid(12) as u32 as u64) << 32 ^ (j + jig(0x22)).div_euclid(12) as u32 as u64)
    } else {
        mix(h ^ 0x5eed)
    };
    let mut v = unit(r) * w;
    let &(row, _) = b.clutter.iter().filter(|c| rows[c.0].kind == kind && fits(&rows[c.0])).find(|c| {
        v -= c.1 / 100.0;
        v < 0.0
    })?;
    let near = || trees.iter().any(|t| (t.0 - i).abs() <= TREE && (t.1 - j).abs() <= TREE);
    match rows[row].near {
        Some(Near::Tree) if !near() => None,
        Some(Near::Open) if near() => None,
        _ => Some(Thing { row, h }),
    }
}

/// Whether row's place rule holds on column (face, i, j) (ADR 0097): near water or over one of its rocks, or it
/// has none. Asked only of the few columns whose roll chose the row.
pub fn wet_or_over(body: &Body, face: u8, i: i32, j: i32, row: &Row) -> bool {
    if row.water.is_none() && row.over.is_empty() {
        return true;
    }
    if let Some(r) = row.water {
        // water standing over the ground (`water_at` names the sea for every dry column too)
        let wet = |i: i32, j: i32| body.water_at(face, i, j).is_some_and(|w| w.0 > body.first_air(body.col_height(face, i, j)) as f64);
        if (-r..=r).flat_map(|a| (-r..=r).map(move |c| (i + a, j + c))).any(|(i, j)| (0..body.n).contains(&i) && (0..body.n).contains(&j) && wet(i, j)) {
            return true;
        }
    }
    // the rock under the soil band, as `World::generated_under` has it there
    let h = body.col_height(face, i, j);
    !row.over.is_empty() && row.over.contains(&crate::rock::ROCKS.at(body.col_rock(face, i, j), (h - 6.0) as f32, 0))
}

fn roll(seed: u64, face: u8, i: i32, j: i32) -> u64 {
    mix(seed ^ 0xc1_0773_5000 ^ (face as u64) << 58 ^ (i as u32 as u64) << 29 ^ j as u32 as u64)
}

/// Whether anything can lie on the column at all: its rolls against every row of the biome, of any ground and
/// season (`density`: the Clutter and the Vegetation settings'). A bound on `pick` and `plant` that costs two
/// hashes, so the mesher asks the ground and the season only of the columns that pass.
pub fn may(seed: u64, face: u8, i: i32, j: i32, biome: u8, density: [f32; 2]) -> bool {
    let Some(b) = crate::biomes::ROWS.biomes.get(biome as usize) else { return false };
    let (mut models, mut plants) = (0.0, 0.0);
    for c in &b.clutter {
        match CLUTTER[c.0].kind {
            Kind::Pickup => models += c.1 / 100.0,
            Kind::Decoration => models += c.1 / 100.0 * density[0],
            Kind::Plant => plants += c.1 / 100.0 * density[1],
        }
    }
    let h = roll(seed, face, i, j);
    unit(h) < models || plants > 0.0 && unit(mix(h ^ PLANT_ROLL)) < plants
}

/// The Vegetation setting's density of plants (few, normal, many; ADR 0091).
pub const VEGETATION: [f32; 3] = [0.4, 1.0, 1.6];

/// The climate a plant's and the grass's tint is made from at column (face, i, j) (ADR 0091): heat and wetness,
/// 0 to 1, from the bake's year (the air at the column's height, -5 to 35 C, and the rain, 0 to 6 mm a day on a
/// root scale). None on a body with no climate.
pub fn climate(body: &Body, face: u8, i: i32, j: i32) -> Option<[f32; 2]> {
    let (theta, _, rain) = body.climate_at(face as usize, i as f64 + 0.5, j as f64 + 0.5)?;
    // the air cools 6.5 K a km over the sea (`climate_at`'s temperature is the sea level's)
    let over = body.col_height(face, i, j) - body.layer_r(body.sea.unwrap_or(0.0));
    let t = theta - 0.0065 * over.max(0.0) as f32;
    Some([((t + 5.0) / 40.0).clamp(0.0, 1.0), (rain.max(0.0) / 6.0).sqrt().clamp(0.0, 1.0)])
}

/// The climate at the corners of chunk `key`'s footprint (ADR 0091), a byte each (heat 4 | wetness 4 << 4; x0y0,
/// x1y0, x0y1, x1y1): the shader's `chunk_climate` mixes them over the chunk, for the grass top and the plants
/// alike, so a field and its ground take one tint and neighbouring chunks share their corners. 0x77 a corner
/// on a body with no climate.
pub fn chunk_climate(body: &Body, key: Key) -> u32 {
    let (b, e) = (key.base(), CS << key.level);
    let q = |v: f32| ((v * 15.0).round() as u32).min(15);
    [(0, 0), (e, 0), (0, e), (e, e)].iter().enumerate().fold(0, |acc, (n, &(x, y))| {
        let c = climate(body, key.face, b.x + x, b.y + y).unwrap_or([0.5, 0.5]);
        acc | (q(c[0]) | q(c[1]) << 4) << (8 * n)
    })
}

/// The tint a green pixel takes, a factor on its painting's colour (ADR 0091): yellow-green where it is dry and
/// warm, deep green where it is wet; for a plant also fresh in spring, yellowing from late summer, dull in
/// winter, by `season` (0 to 1: how much of a season the place has; 0 for the grass block, whose paintings have
/// their seasons). `part` is the part of the year (`part`). The shader's `climate_tint` is this, line for line.
pub fn tint(heat: f32, wet: f32, part: u8, season: f32) -> [f32; 3] {
    const SHIFT: [f32; 8] = [-0.2, -0.1, 0.0, 0.15, 0.35, 0.5, 0.4, 0.3];
    let dry = ((1.0 - wet) * (0.35 + 0.65 * heat) + SHIFT[part as usize & 7] * season).clamp(0.0, 1.0);
    let (wet_c, dry_c) = ([0.62, 0.86, 0.56], [1.4, 0.86, 0.36]);
    let dim = 1.0 - if part >= 6 { 0.18 * season } else { 0.0 };
    // squared: a middling climate stays green, only the dry end turns to straw
    std::array::from_fn(|k| (wet_c[k] + (dry_c[k] - wet_c[k]) * dry * dry) * dim)
}

/// The season (0..4, `SEASONS`) at time `t` over a point of direction `up` from its body's centre: the nearest
/// season's middle (`sky::season_at`).
pub fn season(up: glam::DVec3, t: f64) -> u8 {
    part(up, t) / 2
}

/// Where clutter would lie on level-0 column (i, j) of `tile`'s footprint (tile of face, x, y): the first layer
/// of generated air over it, and its generated top block and biome; None under the sea
/// or in a river or lake. The one ground the mesher and the server ask (ADR 0082).
pub fn ground(body: &Body, tile: &HeightTile, face: u8, i: i32, j: i32) -> Option<(i32, u16, u8)> {
    ground_under(body, tile, face, i, j, 0.0)
}

/// How deep the fresh water a wading plant stands in may be, in metres (ADR 0091).
pub const WADE: f64 = 0.8;

/// `ground`, also under fresh water less than `depth` deep (never under the sea).
pub fn ground_under(body: &Body, tile: &HeightTile, face: u8, i: i32, j: i32, depth: f64) -> Option<(i32, u16, u8)> {
    let (x, y) = (i.div_euclid(CS), j.div_euclid(CS));
    let k = (i - x * CS + 1) as usize + (j - y * CS + 1) as usize * P;
    if body.baked_seed().is_none() {
        return None;
    }
    let z = body.first_air(tile.h[k]);
    if body.sea.is_some_and(|s| z as f64 <= s) || body.water_at(face, i, j).is_some_and(|w| w.0 - z as f64 >= depth) {
        return None;
    }
    Some((z, tile.g[k][0], tile.biome[k]))
}

/// The parts of the year a look names, a bit each of `part`'s eighths: "summer" both halves, "early summer" or
/// "late summer" one.
pub fn part_bits(name: &str) -> Option<u8> {
    let (half, season) = match name.split_once(' ') {
        Some(("early", s)) => (1, s),
        Some(("late", s)) => (2, s),
        _ => (3, name),
    };
    Some(half << (2 * SEASONS.iter().position(|n| *n == season)?))
}

/// The part of the year (0..8: early spring, late spring, early summer, ..) at time `t` over direction `up`: the
/// season's first or second half around its middle (`sky::season_at`); `season` is this halved.
pub fn part(up: glam::DVec3, t: f64) -> u8 {
    ((crate::sky::season_at(up, t).0 + 0.5) * 2.0).floor().rem_euclid(8.0) as u8
}

/// The year (whole years of `sky::YEAR`) at time `t`: a regrowing pick-up taken in an earlier one is back.
pub fn year(t: f64) -> u32 {
    (t / crate::sky::YEAR).floor().max(0.0) as u32
}

/// When level 0 must lay its clutter again: the part of the year at time `t` (a whole count of eighths, where
/// both hemispheres' parts turn) and its year.
pub fn stamp(t: f64) -> (i64, u32) {
    ((t / crate::sky::YEAR * 8.0).floor() as i64, year(t))
}

/// Whether a thing taken in year `taken` (None: never taken) lies there in year `now`.
pub fn lies(row: &Row, taken: Option<u32>, now: u32) -> bool {
    taken.is_none_or(|y| row.regrow && y != now)
}

/// The pick-up of air cell `b` at time `t`, taken or not: the server's check and the client's aim, which ask
/// the ground, the edits and the season as the mesher does (ADR 0082). None if no pick-up lies there.
pub fn pickup_at(world: &World, b: Block, t: f64) -> Option<Thing> {
    pickup(world, b, t, false)
}

/// `pickup_at`; `built` asks only height tiles already built (the frame thread's aim): None on a miss.
fn pickup(world: &World, b: Block, t: f64, built: bool) -> Option<Thing> {
    let body = world.bodies.get(b.body as usize)?;
    let seed = body.baked_seed()?;
    let (i, j) = (b.pos.x, b.pos.y);
    let (x, y) = (i.div_euclid(CS), j.div_euclid(CS));
    let tile = if built { body.tile_if_built(b.face, 0, x, y)? } else { body.tile(b.face, 0, x, y) };
    let (z, top, biome) = ground(body, &tile, b.face, i, j)?;
    let under = Block { pos: b.pos - IVec3::Z, ..b };
    if z != b.pos.z || world.block(b) != 0 || world.block(under) != top {
        return None;
    }
    // the chunk that lays it (the ground cell's) shows none on a column it holds an edit in
    let key = Key { body: b.body, face: b.face, level: 0, pos: under.pos.div_euclid(IVec3::splat(CS)) };
    let c0 = key.pos * CS - 1;
    let col = (i - c0.x) as usize + (j - c0.y) as usize * P;
    if world.edits_for(key).iter().any(|&(c, _)| c as usize % (P * P) == col) {
        return None;
    }
    let up = body.dir(b.face as usize, i as f64 + 0.5, j as f64 + 0.5);
    pick(body, seed, b.face, i, j, biome, top, season(up, t), 1.0, &tile.trees).filter(|th| CLUTTER[th.row].kind == Kind::Pickup)
}

/// The faces of `th` lying in air cell `b` as they are drawn (its alternate's in `still`), each as its corner and
/// the ends of its two edges in the world: shader_models.wgsl `still_local`'s turn and place in the cell, the cell
/// taken as flat between its corners (well under a millimetre off on a 1 m cell). Kept in step with `still_local`
/// and `quads`' (alternate, turn, du, dv) by hand: the shader cannot run in a test.
pub fn thing_faces(world: &World, still: &Still, b: Block, th: &Thing) -> Vec<[glam::DVec3; 3]> {
    let (Some(body), Some(alts)) = (world.bodies.get(b.body as usize), still.rows.get(th.row)) else { return Vec::new() };
    if alts.is_empty() {
        return Vec::new();
    }
    let (a, turn, du, dv) = th.shape(alts.len());
    let p = b.pos.as_dvec3();
    let corner = |d: glam::DVec3| body.centre + body.dir(b.face as usize, p.x + d.x, p.y + d.y) * body.layer_r(p.z + d.z);
    let o = corner(glam::DVec3::ZERO);
    let (ex, ey, ez) = (corner(glam::DVec3::X) - o, corner(glam::DVec3::Y) - o, corner(glam::DVec3::Z) - o);
    let (s, c) = (turn as f64 * std::f64::consts::TAU / 16.0).sin_cos();
    let off = |k: u32| (k as f64 / 15.0 - 0.5) * 0.5;
    // model x, y (up), z about its middle, turned about y: model y is the cell's up, model z its -j
    let place = |m: glam::Vec3| {
        let d = m.as_dvec3() - glam::DVec3::new(0.5, 0.0, 0.5);
        let (rx, rz) = (d.x * c - d.z * s, d.x * s + d.z * c);
        o + ex * (0.5 + off(du) + rx) + ey * (0.5 + off(dv) - rz) + ez * d.y
    };
    let (first, n) = alts[a];
    still.faces[first as usize..(first + n) as usize]
        .iter()
        .map(|f| {
            let v = |k: usize| glam::Vec3::new(f32::from_bits(f[k]), f32::from_bits(f[k + 1]), f32::from_bits(f[k + 2]));
            [place(v(0)), place(v(0) + v(4)), place(v(0) + v(8))]
        })
        .collect()
}

/// The pick-up the ray from `eye` along unit `dir` meets on its own faces within `reach` metres, before any block
/// `edit::aim` would stop at (ADR 0144): its air cell and itself. `lies` says whether one found
/// still lies there (not taken). A ray through a thing's cell that misses its faces goes on to what is behind.
/// shortcut: only the cells the ray passes are asked, so a face reaching over its cell's side is hit only from
/// within the cell; the models stay inside theirs but for a turned corner.
pub fn aim_thing(world: &World, still: &Still, eye: glam::DVec3, dir: glam::DVec3, reach: f64, t: f64, lies: impl Fn(Block, &Thing) -> bool) -> Option<(Block, Thing)> {
    let hits = |[o, a, c]: &[glam::DVec3; 3]| {
        let (eu, ev) = (*a - *o, *c - *o);
        let den = dir.dot(eu.cross(ev));
        let d = (*o - eye).dot(eu.cross(ev)) / den;
        let q = eye + dir * d - *o;
        let (uu, uv, vv, qu, qv) = (eu.dot(eu), eu.dot(ev), ev.dot(ev), q.dot(eu), q.dot(ev));
        let det = uu * vv - uv * uv;
        let (x, y) = ((qu * vv - qv * uv) / det, (qv * uu - qu * uv) / det);
        den.abs() > 1e-12 && (0.0..=reach).contains(&d) && (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y)
    };
    let mut prev = None;
    let mut s = 0.0;
    while s < reach {
        let b = world.block_at(eye + dir * s);
        s += 0.05;
        if prev == Some(b) {
            continue;
        }
        prev = Some(b);
        if b.pos.z >= 0 && crate::blocks::BLOCKS.get(world.block(b)).is_some_and(|d| d.solid || d.model.is_some()) {
            return None;
        }
        // only tiles already built: a miss is no thing there this frame (it runs on the frame thread)
        if let Some(th) = pickup(world, b, t, true).filter(|th| lies(b, th)) {
            if thing_faces(world, still, b, &th).iter().any(hits) {
                return Some((b, th));
            }
        }
    }
    None
}

/// The still models' faces (ADR 0068, 0081): every face of every clutter row's model and alternates once, at rest
/// in the cell's metres, with the paintings they read; what the GPU holds and the mesher indexes.
#[derive(Default)]
pub struct Still {
    /// Per face, as shader.wgsl `SFace`: corner xyz, its painting's first texel, width edge xyz, the painting's
    /// width, height edge xyz, the rectangle's x | y << 16, w | h << 16, and three words of padding.
    pub faces: Vec<[u32; 16]>,
    /// The paintings' texels, RGBA8, one after another.
    pub paint: Vec<u32>,
    /// Per clutter row, its alternates as (first face, faces).
    pub rows: Vec<Vec<(u32, u32)>>,
    /// per row, a plant's looks: the parts of the year (bits of `part`) and the faces (first, count) they draw
    pub looks: Vec<Vec<(u8, (u32, u32))>>,
    /// per row, a plant's `<name>.back.png` (faces), drawn inside a stand, where its four neighbours grow it too:
    /// a bed darker within than at its edge, from every side (ADR 0091)
    pub back: Vec<Option<(u32, u32)>>,
}

/// A face's flags in the table's word 13: a plant's plane, drawn from both sides, swayed, tinted and faded far
/// away (ADR 0091).
pub const PLANT: u32 = 1;
/// And a shrub's (a plant of four planes or more): wider than tall, lopsided by its hash, seen twice as far as
/// the grass (ADR 0091).
pub const SHRUB: u32 = 2;
/// The meadow ground's painting (`data/plants/meadow ground.png`, three 32 px tiles): never drawn as a face,
/// always face 0, the grass tops take it where the plants have thinned (ADR 0091).
pub const MEADOW: u32 = 4;
/// A plant's painting's pixels a metre: 32, finer than a block's 16 so a blade reads (the developer's call of
/// 2026-11-13; its size is its painting's, a column's painting 32 px wide so neighbours merge).
pub const PLANT_PX: f32 = 32.0;

/// How far a plant's planes lean off upright, each to its own side by turns (ADR 0091): seen from above a tuft
/// then covers ground, so a meadow reads from 20 m up and not only from the side.
pub const PLANT_LEAN: f32 = 0.26;

/// A plant's planes (ADR 0091): `n` planes through the cell's middle, turned by 180/n degrees each and leaning
/// `PLANT_LEAN` off upright to either side by turns, as wide and high (along the plane) as the painting at
/// `PLANT_PX`; each face's corner is its painting's top left, `ev` down.
pub fn plant_faces(n: u8, w: u32, h: u32) -> Vec<([glam::Vec3; 3], [u32; 4])> {
    if n >= 4 {
        // a shrub: three bulging clumps of three planes, of different sizes and turns, set apart in the cell so
        // it is lobed with dents between them, wider than tall together, not one card cross
        let clumps = [(glam::Vec3::new(-0.22, 0.0, -0.08), 0.85, 0.0), (glam::Vec3::new(0.2, 0.0, 0.12), 0.7, 0.35), (glam::Vec3::new(0.02, 0.0, 0.26), 0.55, 0.8)];
        return clumps
            .iter()
            .flat_map(|&(at, size, turn)| {
                plant_cross(3, w, h, size, turn).into_iter().map(move |(v, r)| ([v[0] + at, v[1], v[2]], r))
            })
            .collect();
    }
    plant_cross(n, w, h, 1.0, 0.0)
}

/// `n` leaning planes through the cell's middle at `size` of the painting, the first turned `turn` radians.
fn plant_cross(n: u8, w: u32, h: u32, size: f32, turn: f32) -> Vec<([glam::Vec3; 3], [u32; 4])> {
    let (wm, hm) = (w as f32 / PLANT_PX * size, h as f32 / PLANT_PX * size);
    (0..n)
        .map(|k| {
            let a = std::f32::consts::PI * k as f32 / n as f32 + turn;
            let d = glam::Vec3::new(a.cos(), 0.0, a.sin()) * wm;
            let side = if k % 2 == 0 { 1.0 } else { -1.0 } * PLANT_LEAN.sin();
            let up = glam::Vec3::new(-a.sin() * side, PLANT_LEAN.cos(), a.cos() * side) * hm;
            ([glam::Vec3::new(0.5, 0.0, 0.5) + up - d * 0.5, d, -up], [0, 0, w, h])
        })
        .collect()
}

/// The most faces the table holds: a still quad names its face in 16 bits.
pub const MAX_FACES: usize = 1 << 16;

impl Still {
    /// Each clutter row's model from `dir`: `<name>.toml` and `<name>.png`, and the alternates `<name>.1` to `.4`
    /// beside it. A missing or refused file is named in the list returned, and its row (or that alternate)
    /// draws nothing.
    pub fn load(dir: &std::path::Path, plants: &std::path::Path, palette: &Palette) -> (Still, Vec<String>) {
        let mut s = Still::default();
        let mut bad = Vec::new();
        // face 0: the meadow ground, three 32 px tiles (the shader's `ground_tint` reads it), or a 1 px stand-in
        let meadow = std::fs::read(plants.join("meadow ground.png")).map_err(|e| e.to_string()).and_then(|b| crate::models::painting(&b, palette));
        let meadow = meadow.unwrap_or_else(|e| {
            bad.push(format!("data/plants/meadow ground.png: {e}"));
            crate::models::Painting { w: 1, h: 1, px: vec![[0x4c, 0x94, 0x33, 255]] }
        });
        let face = ([glam::Vec3::ZERO; 3], [0, 0, meadow.w, meadow.h]);
        if let Err(e) = s.push("meadow ground", &meadow, vec![face], MEADOW) {
            bad.push(e);
        }
        // word 14: its mean colour, RGBA8, which far grass fades to instead of point-sampling (no shimmer)
        s.faces[0][14] = mean_colour(&meadow);
        for r in CLUTTER.iter() {
            let mut alts = Vec::new();
            let mut looks = Vec::new();
            if r.kind == Kind::Plant {
                // a plant: its painting, alternates <name>.1.png to .4, and a painting per look
                let one = |base: &str, s: &mut Still| -> Option<Result<(u32, u32), String>> {
                    let png = plants.join(format!("{base}.png"));
                    if base.contains('.') && !png.exists() {
                        return None;
                    }
                    let p = match std::fs::read(&png).map_err(|e| e.to_string()).and_then(|b| crate::models::painting(&b, palette)) {
                        Ok(p) => p,
                        Err(e) => return Some(Err(format!("data/plants/{base}.png: {e}"))),
                    };
                    // what a chunk's bounds and the faces' table can hold (shortcut: the chunk's bounding sphere has no
                    // margin over its top, so a tall plant on a chunk's top layer may be culled a frame early at an edge)
                    if p.w as f32 > 2.0 * PLANT_PX || p.h as f32 > 2.5 * PLANT_PX {
                        return Some(Err(format!("data/plants/{base}.png: {} x {} px, a plant is at most 2 m wide and 2.5 m high ({PLANT_PX} px a metre)", p.w, p.h)));
                    }
                    Some(s.push(&base, &p, plant_faces(r.planes, p.w, p.h), PLANT | if r.planes >= 4 { SHRUB } else { 0 }))
                };
                for a in 0..=ALTERNATES {
                    let base = if a == 0 { r.name.clone() } else { format!("{}.{a}", r.name) };
                    match one(&base, &mut s) {
                        Some(Ok(f)) => alts.push(f),
                        Some(Err(e)) => bad.push(e),
                        None => {}
                    }
                }
                for (n, m) in &r.looks {
                    match one(&format!("{}.{n}", r.name), &mut s) {
                        Some(Ok(f)) => looks.push((*m, f)),
                        Some(Err(e)) => bad.push(e),
                        None => bad.push(format!("data/plants/{}.{n}.png: missing, its row names the look", r.name)),
                    }
                }
                let back = match one(&format!("{}.back", r.name), &mut s) {
                    Some(Ok(f)) => Some(f),
                    Some(Err(e)) => {
                        bad.push(e);
                        None
                    }
                    None => None,
                };
                s.rows.push(alts);
                s.looks.push(looks);
                s.back.push(back);
                continue;
            }
            for a in 0..=ALTERNATES {
                let base = if a == 0 { r.name.clone() } else { format!("{}.{a}", r.name) };
                let (toml, png) = (dir.join(format!("{base}.toml")), dir.join(format!("{base}.png")));
                if a > 0 && !toml.exists() {
                    continue;
                }
                let read = || -> Result<(Model, crate::models::Painting), String> {
                    let text = std::fs::read_to_string(&toml).map_err(|e| e.to_string())?;
                    let m = Model::parse(&base, &text, palette)?;
                    let p = crate::models::painting(&std::fs::read(&png).map_err(|e| format!("{base}.png: {e}"))?, palette).map_err(|e| format!("{base}.png: {e}"))?;
                    m.check_painting(&p)?;
                    Ok((m, p))
                };
                match read().and_then(|(m, p)| s.push(&base, &p, m.rest_faces(), 0)) {
                    Ok(f) => alts.push(f),
                    Err(e) => bad.push(format!("data/models/{base}: {e}")),
                }
            }
            s.rows.push(alts);
            s.looks.push(looks);
            s.back.push(None);
        }
        (s, bad)
    }

    /// Puts painting `p` and its `faces` (rest corner, u and v edges in the cell's metres; painting rectangle)
    /// with `flags` in the tables: (first face, count).
    fn push(&mut self, base: &str, p: &crate::models::Painting, faces: Vec<([glam::Vec3; 3], [u32; 4])>, flags: u32) -> Result<(u32, u32), String> {
        if self.faces.len() + faces.len() > MAX_FACES {
            return Err(format!("{base}: the still faces' table is full ({MAX_FACES})"));
        }
        let base_tx = self.paint.len() as u32;
        self.paint.extend(p.px.iter().map(|c| u32::from_le_bytes(*c)));
        let first = self.faces.len() as u32;
        for ([o, eu, ev], [x, y, w, h]) in faces.iter().copied() {
            let f = |v: f32| v.to_bits();
            self.faces.push([f(o.x), f(o.y), f(o.z), base_tx, f(eu.x), f(eu.y), f(eu.z), p.w, f(ev.x), f(ev.y), f(ev.z), x | y << 16, w | h << 16, flags, 0, 0]);
        }
        Ok((first, faces.len() as u32))
    }
}

/// A still quad (ADR 0068): lo = the cell x6 y6 z6, its place in the cell u4 v4, dir 7 at bit 28 (no face of a
/// block has it); hi = the face's index in `Still::faces`, its turn in sixteenths at bit 16.
pub fn quad(x: u32, y: u32, z: u32, du: u32, dv: u32, face: u32, turn: u32) -> [u32; 2] {
    [x | y << 6 | z << 12 | du << 18 | dv << 22 | 7 << 28, face | turn << 16]
}

/// The still quads of the clutter on level-0 chunk `key` (its padded `cells`, the `edits` it was generated with):
/// on each column whose generated top lies in it, untouched by an edit, what `pick` puts there, unless it was
/// `taken` (cell, year) and has not come back by the year of the sky's time `t`. Each column takes its own season
/// at `t` (its hemisphere's), as `pickup_at` does.
#[allow(clippy::too_many_arguments)]
pub fn quads(body: &Body, key: Key, cells: &[u16; P * P * P], edits: &[(u32, u16)], still: &Still, t: f64, density: [f32; 2], taken: &[(IVec3, u32)]) -> Vec<[u32; 2]> {
    let mut out = Vec::new();
    let Some(seed) = body.baked_seed() else { return out };
    if key.level != 0 || still.faces.is_empty() {
        return out;
    }
    let tile = body.tile(key.face, 0, key.pos.x, key.pos.y);
    // a chunk no ground top lies in (a cave's, the air's) has none: its cells' z range against the tile's
    let c0z = key.pos.z * CS - 1;
    if body.first_air(tile.max as f64) < c0z + 2 || body.first_air(tile.min as f64) > c0z + CS + 1 {
        return out;
    }
    let year = year(t);
    // ore bodies under the chunk: its columns may hold a sample whatever the biome's roll
    let ores = !body.tile_ores(key.face, key.pos.x, key.pos.y).is_empty();
    let mut edited = [false; P * P];
    for &(c, _) in edits {
        edited[c as usize % (P * P)] = true;
    }
    let c0 = key.pos * CS - 1;
    for v in 1..=CS as usize {
        for u in 1..=CS as usize {
            if edited[u + v * P] {
                continue;
            }
            let (i, j) = (c0.x + u as i32, c0.y + v as i32);
            // the roll first: one hash turns most columns away before the ground's lookups
            if !may(seed, key.face, i, j, tile.biome[u + v * P], density) && (!ores || sample(body, seed, key.face, i, j).is_none()) {
                continue;
            }
            // shallow fresh water too: a wading plant (reeds) stands in it, nothing else does
            let Some((z, top, biome)) = ground_under(body, &tile, key.face, i, j, WADE) else { continue };
            let wet = body.water_at(key.face, i, j).is_some_and(|w| z as f64 <= w.0);
            let zl = z - c0.z;
            // the chunk that holds the ground cell lays it: that chunk is never all air
            if !(2..=CS + 1).contains(&zl) || cells[pidx(u, v, zl as usize)] != 0 || cells[pidx(u, v, zl as usize - 1)] != top {
                continue;
            }
            let p8 = part(body.dir(key.face as usize, i as f64 + 0.5, j as f64 + 0.5), t);
            let model = pick(body, seed, key.face, i, j, biome, top, p8 / 2, density[0], &tile.trees).filter(|_| !wet);
            let grows = plant(seed, key.face, i, j, biome, top, p8 / 2, density[1], &tile.trees).filter(|g| !wet || CLUTTER[g.row].wades);
            for t in [model, grows].into_iter().flatten() {
                let row = &CLUTTER[t.row];
                // (only a pick-up is ever taken: the list is not looked through for the rest)
                if row.kind == Kind::Pickup && !lies(row, taken.iter().find(|x| x.0 == IVec3::new(i, j, z)).map(|x| x.1), year) {
                    continue;
                }
                let alts = &still.rows[t.row];
                if alts.is_empty() {
                    continue;
                }
                let (a, turn, du, dv) = t.shape(alts.len());
                // a neighbour counts if it grows this row and has ground (none too deep: the bed's open-water edge
                // keeps the light painting). shortcut: the neighbours are asked with this column's biome and ground,
                // and past the chunk's tile their ground is not looked up; fine for a look, revisit if it shows
                let has_ground = |ni: i32, nj: i32| ni.div_euclid(CS) != key.pos.x || nj.div_euclid(CS) != key.pos.y || ground_under(body, &tile, key.face, ni, nj, WADE).is_some();
                let inside = || [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().all(|&(di, dj)| has_ground(i + di, j + dj) && plant(seed, key.face, i + di, j + dj, biome, top, p8 / 2, density[1], &tile.trees).is_some_and(|o| o.row == t.row));
                let alt = match still.back[t.row] {
                    Some(b) if row.kind == Kind::Plant && inside() => b,
                    _ => alts[a],
                };
                let (first, n) = still.looks[t.row].iter().find(|l| l.0 >> p8 & 1 == 1).map_or(alt, |l| l.1);
                for f in first..first + n {
                    out.push(quad(u as u32 - 1, v as u32 - 1, zl as u32 - 1, du, dv, f, turn));
                }
            }
        }
    }
    out
}

/// A painting's mean colour over its opaque pixels, packed RGBA8 (r in the low byte).
fn mean_colour(p: &crate::models::Painting) -> u32 {
    let (mut sum, mut n) = ([0u32; 3], 0u32);
    for px in p.px.iter().filter(|px| px[3] >= 128) {
        (0..3).for_each(|k| sum[k] += px[k] as u32);
        n += 1;
    }
    let m = sum.map(|v| v / n.max(1));
    m[0] | m[1] << 8 | m[2] << 16 | 255 << 24
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A shrub (four planes or more) is several bulging clumps, not one card cross (ADR 0091): three crosses of
    /// different sizes, set apart in the cell, wider than tall together.
    #[test]
    fn a_shrub_is_several_clumps() {
        let f = plant_faces(4, 40, 40);
        let feet: Vec<glam::Vec3> = f.iter().map(|(v, _)| v[0] + v[2] + v[1] * 0.5).collect();
        let mut centres: Vec<(i32, i32)> = feet.iter().map(|p| ((p.x * 20.0).round() as i32, (p.z * 20.0).round() as i32)).collect();
        centres.sort();
        centres.dedup();
        assert!(centres.len() >= 3, "three clumps or more: {centres:?}");
        let (lo, hi) = feet.iter().fold((glam::Vec3::splat(9.0), glam::Vec3::splat(-9.0)), |(a, b), p| (a.min(*p), b.max(*p)));
        let top = f.iter().map(|(v, _)| v[0].y).fold(0.0f32, f32::max);
        assert!((hi.x - lo.x).max(hi.z - lo.z) > 0.3, "set apart: {lo:?} {hi:?}");
        assert!(f.iter().map(|(v, _)| v[2].length()).fold(9.0f32, f32::min) < 0.9 * f[0].0[2].length().max(1.25), "of different sizes");
        assert!(top < 1.25, "lower than its painting: {top}");
    }

    #[test]
    fn plant_planes_lean_and_cover_ground() {
        // each plane stands on the cell's middle line and leans to its own side, so from above a tuft covers ground
        let f = plant_faces(3, 32, 32);
        for (k, (v, _)) in f.iter().enumerate() {
            let (top, bottom) = (v[0], v[0] + v[2]);
            assert!(bottom.y.abs() < 1e-5 && (top.y - PLANT_LEAN.cos()).abs() < 1e-4, "{k}: {v:?}");
            let lean = glam::Vec2::new(top.x - bottom.x, top.z - bottom.z).length();
            assert!(lean > 0.2, "{k} leans {lean}");
        }
        let dx = |k: usize| f[k].0[0].x + f[k].0[0].z - (f[k].0[0] + f[k].0[2]).x - (f[k].0[0] + f[k].0[2]).z;
        assert!(dx(0).signum() != dx(1).signum() || dx(1).signum() != dx(2).signum(), "they lean by turns");
    }

    /// A thing on the ground is aimed at by its own faces (ADR 0144): a ray at it hits it before the
    /// ground behind; one through its cell beside it falls to the ground block; past the reach, or taken, it is
    /// not aimed at.
    #[test]
    fn a_thing_is_aimed_at_by_its_own_faces() {
        use crate::bake::Map;
        use glam::DVec3;
        use std::sync::Arc;
        let map = Arc::new(Map::bake(5, 1536));
        let w = World::with_map(1536, 32, 5, Some(map.clone()));
        let pal = Palette::parse(include_str!("../../../data/palette.toml")).unwrap();
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let still = Still::load(&data.join("models"), &data.join("plants"), &pal).0;
        let b = &w.bodies[0];
        let (f, si, sj) = b.locate(b.spawn);
        let (f, si, sj) = (f as u8, si as i32, sj as i32);
        let mut tried = 0;
        for (i, j) in (-60..=60).flat_map(|a| (-60..=60).map(move |c| (si + a, sj + c))) {
            let tile = b.tile(f, 0, i.div_euclid(CS), j.div_euclid(CS));
            let Some((z, _, _)) = ground(b, &tile, f, i, j) else { continue };
            let at = Block { body: 0, face: f, pos: IVec3::new(i, j, z) };
            let Some(th) = pickup_at(&w, at, 0.0) else { continue };
            let under = Block { pos: at.pos - IVec3::Z, ..at };
            let faces = thing_faces(&w, &still, at, &th);
            assert!(!faces.is_empty(), "{}: faces", CLUTTER[th.row].name);
            // read from the packed table as `Still::push` wrote it: each face, in the grid's cells (the shader
            // places in cells, so metres follow the column's size), as long and wide as its words say, and the
            // thing in its cell, on its floor
            let (alt, ..) = th.shape(still.rows[th.row].len());
            let first = still.rows[th.row][alt].0 as usize;
            let grid = |p: DVec3| w.grid_at(p).2 - at.pos.as_dvec3();
            for (k, [o, u, v]) in faces.iter().enumerate() {
                let word = |n: usize| glam::Vec3::new(f32::from_bits(still.faces[first + k][n]), f32::from_bits(still.faces[first + k][n + 1]), f32::from_bits(still.faces[first + k][n + 2])).length() as f64;
                let (g, gu, gv) = (grid(*o), grid(*u), grid(*v));
                assert!(((gu - g).length() - word(4)).abs() < 2e-3 && ((gv - g).length() - word(8)).abs() < 2e-3, "face {k}: its edges' lengths {} {} against {} {}", (gu - g).length(), (gv - g).length(), word(4), word(8));
                assert!(g.min_element() > -0.3 && g.x < 1.3 && g.y < 1.3 && g.z > -0.05, "face {k} lies in its cell, on its floor: {g:?}");
            }
            let mid = faces.iter().flatten().sum::<DVec3>() / (faces.len() * 3) as f64;
            let up = b.dir(f as usize, i as f64 + 0.5, j as f64 + 0.5);
            let eye = w.block_centre(at) + up * 1.8 + up.any_orthonormal_vector() * 0.3;
            // from above the cell: open sky between (no leaves), the block ray meets the ground under
            if crate::edit::aim(&w, eye, (mid - eye).normalize(), 8.0).map(|a| a.0) != Some(under) {
                continue;
            }
            tried += 1;
            let at_it = (mid - eye).normalize();
            // the aim builds no height tile (it runs on the frame thread): in a world that has built none yet it
            // finds nothing, and once the tile is there it finds the thing
            let fresh = World::with_map(1536, 32, 5, Some(map.clone()));
            assert!(aim_thing(&fresh, &still, eye, at_it, 8.0, 0.0, |_, _| true).is_none(), "no tile built: nothing aimed at");
            assert!(fresh.bodies[0].tile_if_built(f, 0, i.div_euclid(CS), j.div_euclid(CS)).is_none(), "the aim built no tile");
            assert_eq!(
aim_thing(&w, &still, eye, at_it, 8.0, 0.0, |_, _| true).map(|a| a.0), Some(at), "{}: aimed at its middle", CLUTTER[th.row].name);
            assert!(aim_thing(&w, &still, eye, at_it, (mid - eye).length() * 0.5, 0.0, |_, _| true).is_none(), "out of reach");
            assert!(aim_thing(&w, &still, eye, at_it, 8.0, 0.0, |_, _| false).is_none(), "taken");
            // the floor's corner farthest from it, inside its cell: the ray falls to the ground block
            let p = at.pos.as_dvec3();
            let floor = [(0.05, 0.05), (0.95, 0.05), (0.05, 0.95), (0.95, 0.95)].map(|(u, v)| b.centre + b.dir(f as usize, p.x + u, p.y + v) * b.layer_r(p.z));
            let far = floor.into_iter().max_by(|a, c| a.distance(mid).total_cmp(&c.distance(mid))).unwrap();
            let beside = (far - eye).normalize();
            assert!(aim_thing(&w, &still, eye, beside, 8.0, 0.0, |_, _| true).is_none(), "{}: beside it", CLUTTER[th.row].name);
            assert_eq!(crate::edit::aim(&w, eye, beside, 8.0).map(|a| a.0), Some(under), "beside it the ground is aimed at");
            if tried >= 8 {
                break;
            }
        }
        assert!(tried >= 3, "{tried} things looked at");
        // the cost, once (the client aims once a frame): from the eye at the spawn, down and ahead
        let (n, t0) = (2000, std::time::Instant::now());
        let e = b.centre + b.spawn * (b.height(b.spawn) + 1.6);
        for k in 0..n {
            let d = (-b.spawn + DVec3::new(0.3, (k % 7) as f64 * 0.1, 0.2)).normalize();
            std::hint::black_box(aim_thing(&w, &still, e, d, 8.0, 0.0, |_, _| true));
        }
        eprintln!("aim_thing: {:.1} us a call", t0.elapsed().as_secs_f64() * 1e6 / n as f64);
    }

    /// Flint (ADR 0097) lies where a new player looks first: near the spawn of every seed, at least `NEAR` within
    /// 60 m, and each one near water (a bank, a beach, a gravel bar) or over limestone or marble.
    #[test]
    fn flint_lies_within_a_short_walk_of_every_spawn() {
        use crate::bake::Map;
        use crate::world::{Block, World};
        use std::sync::Arc;
        const NEAR: usize = 5;
        let flint = CLUTTER.iter().position(|r| r.name == "flint").unwrap();
        for seed in [5, 11, 23] {
            let w = World::with_map(1536, 32, seed, Some(Arc::new(Map::bake(seed, 1536))));
            let b = &w.bodies[0];
            let (f, si, sj) = b.locate(b.spawn);
            let (f, si, sj) = (f as u8, si as i32, sj as i32);
            let mut found = Vec::new();
            for (i, j) in (-60..=60).flat_map(|a| (-60..=60).map(move |c| (si + a, sj + c))).filter(|&(i, j)| ((i - si).pow(2) + (j - sj).pow(2)) <= 3600) {
                let tile = b.tile(f, 0, i.div_euclid(CS), j.div_euclid(CS));
                let Some((z, _, _)) = ground(b, &tile, f, i, j) else { continue };
                if pickup_at(&w, Block { body: 0, face: f, pos: IVec3::new(i, j, z) }, 0.0).is_some_and(|t| t.row == flint) {
                    found.push((i, j));
                }
            }
            eprintln!("seed {seed}: {} flints within 60 m of the spawn", found.len());
            assert!(found.len() >= NEAR, "seed {seed}: {} flints within 60 m of the spawn", found.len());
            for &(i, j) in &found {
                assert!(wet_or_over(b, f, i, j, &CLUTTER[flint]), "seed {seed}: a flint at ({i}, {j}) neither near water nor over its rock");
            }
            // the rule picks places: most dry land of the face is neither by water nor over limestone
            let sea = b.sea.unwrap();
            let land: Vec<_> = (0..b.n).step_by(13).flat_map(|i| (0..b.n).step_by(13).map(move |j| (i, j))).filter(|&(i, j)| b.first_air(b.col_height(f, i, j)) as f64 > sea + 2.0).collect();
            let ok = land.iter().filter(|&&(i, j)| wet_or_over(b, f, i, j, &CLUTTER[flint])).count();
            eprintln!("seed {seed}: {ok} of {} dry columns take flint", land.len());
            assert!(ok * 2 < land.len(), "seed {seed}: {ok} of {} dry columns take flint", land.len());
        }
    }

    /// Ore samples (ADR 0138): every kind's sample has a pick-up row; over the bodies
    /// of the kinds that have one their item lies loose on the dry ground, about `SAMPLE_EVERY_M2` apart at the
    /// middle and thinning to none at the spread's rim, and none lies off every body's spread.
    #[test]
    fn samples_lie_over_the_bodies_and_not_away_from_them() {
        use crate::ore::{MINERALS, SAMPLE_EVERY_M2, SAMPLE_SPREAD_M};
        use crate::world::{Block, World};
        for k in &MINERALS.kinds {
            if let Some(s) = &k.sample {
                assert!(SAMPLES[MINERALS.kinds.iter().position(|o| o.name == k.name).unwrap()].is_some_and(|r| CLUTTER[r].gives.as_ref().is_some_and(|g| &g.0 == s) && CLUTTER[r].kind == Kind::Pickup), "{}: a pick-up row gives '{s}'", k.name);
            }
        }
        let w = World::baked(1536, 32, 7);
        let b = &w.bodies[0];
        let (mut seen, mut want, mut kinds) = (0usize, 0.0f64, std::collections::BTreeSet::new());
        for o in &b.ores {
            let Some(row) = SAMPLES[o.kind as usize] else { continue };
            let r = (o.a.max(o.b).max(SAMPLE_SPREAD_M) + 8.0).ceil() as i32;
            let (ci, cj) = (o.at.x as i32, o.at.y as i32);
            for (i, j) in (ci - r..=ci + r).flat_map(|i| (cj - r..=cj + r).map(move |j| (i, j))).filter(|&(i, j)| (0..b.n).contains(&i) && (0..b.n).contains(&j)) {
                let tile = b.tile(o.face, 0, i.div_euclid(CS), j.div_euclid(CS));
                let Some((z, _, _)) = ground(b, &tile, o.face, i, j) else { continue };
                let here = pickup_at(&w, Block { body: 0, face: o.face, pos: IVec3::new(i, j, z) }, 0.0).is_some_and(|t| t.row == row);
                let others = b.ores.iter().any(|p| SAMPLES[p.kind as usize] == Some(row) && p != o && p.face == o.face && p.spread(i, j).is_some());
                match o.spread(i, j) {
                    Some(q) => {
                        want += (1.0 - q) / SAMPLE_EVERY_M2;
                        seen += here as usize;
                        kinds.insert(o.kind);
                    }
                    None => assert!(!here || others, "{}: a sample at ({i}, {j}) off its body's spread", MINERALS.kinds[o.kind as usize].name),
                }
            }
        }
        eprintln!("{seen} samples over {} kinds' bodies, {want:.1} expected", kinds.len());
        assert!(kinds.len() >= 3 && want > 20.0, "{} kinds, {want} expected", kinds.len());
        assert!((0.7..1.4).contains(&(seen as f64 / want)), "{seen} samples, {want:.1} expected");
    }

    #[test]
    fn the_rows_load_and_the_biomes_name_them() {
        let rows = &*CLUTTER;
        assert!(rows.iter().filter(|r| r.kind == Kind::Decoration).count() >= 3);
        assert!(crate::biomes::ROWS.biomes.iter().filter(|b| !b.clutter.is_empty()).count() >= 4);
        for r in rows.iter().filter(|r| r.kind == Kind::Pickup) {
            assert!(r.item().is_some(), "{}: gives an item of data/items.toml", r.name);
        }
        let bad = |t: &str| parse(t).is_err();
        assert!(bad("[[clutter]]\nname = \"x\"\nkind = \"decoration\"\non = [\"no such block\"]\n"));
        assert!(bad("[[clutter]]\nname = \"x\"\nkind = \"pickup\"\non = [\"grass\"]\n"), "a pick-up gives something");
        assert!(bad("[[clutter]]\nname = \"x\"\nkind = \"decoration\"\non = [\"grass\"]\nseason = [\"monsoon\"]\n"));
        assert!(!bad("[[clutter]]\nname = \"x\"\nkind = \"decoration\"\non = [\"grass\"]\nseason = [\"autumn\"]\n"));
    }

    /// Every row's model and painting load and fit the palette and the face table (ADR 0081, 0082).
    #[test]
    fn every_row_has_a_model_that_loads() {
        let pal = Palette::parse(include_str!("../../../data/palette.toml")).unwrap();
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let (s, bad) = Still::load(&data.join("models"), &data.join("plants"), &pal);
        assert!(bad.is_empty(), "{bad:?}");
        assert_eq!(s.rows.len(), CLUTTER.len());
        assert!(s.rows.iter().all(|a| !a.is_empty()), "every row has a model or painting");
        // the meadow ground the far grass tops take is face 0, three 32 px tiles side by side (ADR 0091)
        assert_eq!(s.faces[0][13], MEADOW, "the meadow ground is face 0");
        assert_eq!(s.faces[0][12], 96 | 32 << 16, "three tiles of 32 x 32");
        let mean = s.faces[0][14].to_le_bytes();
        assert!(mean[3] == 255 && mean[1] > mean[0] && mean[1] > mean[2], "the meadow ground's mean is green: {mean:?}");
        assert!(s.faces.len() < 2000, "{} faces", s.faces.len());
        // in the cell's metres: every corner within a block of the cell (a plant up to 2.5 m high), none of them
        // in pixels or voxels
        for f in &s.faces {
            let o = [0, 1, 2].map(|k| f32::from_bits(f[k]));
            let top = if f[13] & PLANT != 0 { 2.5 } else { 1.5 };
            assert!(o.iter().all(|v| (-0.5..=top).contains(v)), "{o:?}");
            assert!(o[1] >= -0.05, "nothing below the ground: {o:?}");
        }
        for (r, row) in CLUTTER.iter().enumerate() {
            assert_eq!(s.looks[r].len(), row.looks.len(), "{}: a painting per look", row.name);
            let plant = s.faces[s.rows[r][0].0 as usize][13] & PLANT != 0;
            assert_eq!(plant, row.kind == Kind::Plant, "{}", row.name);
            // a shrub (four planes or more) is drawn wide and lopsided and reaches twice as far (ADR 0091)
            let shrub = s.faces[s.rows[r][0].0 as usize][13] & SHRUB != 0;
            assert_eq!(shrub, plant && row.planes >= 4, "{}", row.name);
        }
        // a reed bed has depth: its inside a darker painting, and four outlines so no stand looks stamped
        let reeds = CLUTTER.iter().position(|r| r.name == "reeds").unwrap();
        assert!(s.back[reeds].is_some(), "reeds.back.png");
        assert_eq!(s.rows[reeds].len(), 4, "reeds and .1 to .3");
        assert!(CLUTTER[reeds].wades, "reeds stand in shallow water");
    }

    /// Bug 2026-11-13_14-24-52 (ADR 0099): twigs, stones and grass too small to read from standing height. A
    /// row's longest box edge in metres: a fallen branch 0.6 m or more, a stick 0.75, stones 0.3; short grass
    /// mostly 0.6 m high, with one young outline lower.
    #[test]
    fn clutter_reads_from_standing_height() {
        let pal = Palette::parse(include_str!("../../../data/palette.toml")).unwrap();
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let (s, _) = Still::load(&data.join("models"), &data.join("plants"), &pal);
        let edge = |f: &[u32; 16], k: usize| glam::Vec3::from_array([k, k + 1, k + 2].map(|i| f32::from_bits(f[i]))).length();
        let row = |name: &str| &s.rows[CLUTTER.iter().position(|r| r.name == name).unwrap()];
        let longest = |&(first, n): &(u32, u32)| (first..first + n).map(|i| edge(&s.faces[i as usize], 4).max(edge(&s.faces[i as usize], 8))).fold(0.0, f32::max);
        for (name, least) in [("twigs", 0.6), ("stick", 0.75), ("pebbles", 0.3), ("loose stone", 0.3)] {
            for alt in row(name) {
                assert!(longest(alt) >= least, "{name}: {} m, at least {least}", longest(alt));
            }
        }
        // a plant's planes are as high as its painting along `ev`
        let high: Vec<f32> = row("short grass").iter().map(|&(first, _)| edge(&s.faces[first as usize], 8)).collect();
        assert!(high.iter().filter(|&&h| h >= 0.6).count() >= 3, "short grass mostly tall: {high:?}");
        assert!(high.iter().any(|&h| h < 0.5), "and a young one: {high:?}");
    }

    /// The review of 2026-11-13: the season alone turns at a season's middle, so a year began without a new mesh
    /// and a regrowing pick-up taken then stayed drawn; the stamp turns at both.
    #[test]
    fn the_stamp_turns_with_the_season_and_the_year() {
        let y = crate::sky::YEAR;
        let season_only = |t: f64| (t / y * 4.0).round() as i64;
        assert_eq!(season_only(y - 1.0), season_only(y + 1.0), "the old key missed the year's start");
        assert_ne!(stamp(y - 1.0), stamp(y + 1.0));
        assert_ne!(stamp(y * 0.125 - 1.0), stamp(y * 0.125 + 1.0), "a season's middle");
        assert_ne!(stamp(y * 0.25 - 1.0), stamp(y * 0.25 + 1.0), "a season's halves (ADR 0091's looks)");
        assert_eq!(stamp(y * 0.3), stamp(y * 0.32));
        // the stamp turns wherever any part does, in either hemisphere
        for k in 0..800 {
            let t = y * k as f64 / 400.0;
            for z in [0.9, -0.9] {
                let up = glam::DVec3::new(0.4, 0.0, z).normalize();
                if stamp(t) == stamp(t + y / 400.0) {
                    assert_eq!(part(up, t + 1.0), part(up, t + y / 400.0 - 1.0), "t {t}");
                }
            }
        }
    }

    /// Where each biome with clutter lies on land near the spawn of the 4 km planet of seed 1337: an `at=` for a
    /// filmstrip (cargo test -p engine --lib where_the_biomes_lie -- --ignored --nocapture).
    #[test]
    #[ignore]
    fn where_the_biomes_lie() {
        let w = World::baked(6144, crate::bodies::columns(1, 6144), 1337);
        let b = &w.bodies[0];
        let mut seen = std::collections::BTreeSet::new();
        for k in 0..40000 {
            let (x, z) = ((k % 200) as f64 / 100.0 - 1.0, (k / 200) as f64 / 100.0 - 1.0);
            let d = glam::DVec3::new(x * 0.6, 1.0, z * 0.6).normalize();
            let (f, i, j) = b.locate(d);
            let (i, j) = (i as i32, j as i32);
            let tile = b.tile(f as u8, 0, i.div_euclid(CS), j.div_euclid(CS));
            if let Some((_, _, bi)) = ground(b, &tile, f as u8, i, j) {
                let name = &crate::biomes::ROWS.biomes[bi as usize].name;
                if seen.insert(name.clone()) {
                    println!("{name}: at={:.4},{:.4},{:.4}", d.x, d.y, d.z);
                }
            }
        }
    }

    /// A plant row (ADR 0091): planes 2 to 4 on a plant only, looks by parts of the year.
    #[test]
    fn plant_rows_are_checked() {
        let bad = |t: &str| parse(t).is_err();
        let row = |extra: &str| format!("[[clutter]]\nname = \"x\"\nkind = \"plant\"\non = [\"grass\"]\n{extra}");
        assert!(!bad(&row("")));
        assert_eq!(parse(&row("")).unwrap()[0].planes, 2, "two planes unless it says");
        assert!(!bad(&row("planes = 4\nlooks = { fruit = [\"late summer\"], bloom = [\"spring\"] }\n")));
        assert!(bad(&row("planes = 1\n")) && bad(&row("planes = 5\n")));
        // a plant that stands in shallow fresh water too (reeds), and only a plant
        assert!(!bad(&row("wades = true\n")));
        assert!(bad("[[clutter]]\nname = \"x\"\nkind = \"decoration\"\non = [\"grass\"]\nwades = true\n"), "wading is a plant's");
        assert!(bad(&row("looks = { fruit = [\"midsummer\"] }\n")));
        assert!(bad("[[clutter]]\nname = \"x\"\nkind = \"decoration\"\non = [\"grass\"]\nplanes = 2\n"), "planes are a plant's");
        assert!(bad("[[clutter]]\nname = \"x\"\nkind = \"decoration\"\non = [\"grass\"]\nlooks = { a = [\"spring\"] }\n"));
        let r = &parse(&row("looks = { fruit = [\"late summer\"], bloom = [\"spring\"] }\n")).unwrap()[0];
        assert_eq!(r.looks, vec![("bloom".to_string(), 0b11), ("fruit".to_string(), 0b1000)]);
        assert!(CLUTTER.iter().filter(|r| r.kind == Kind::Plant).count() >= 7, "short and tall grass, fern, reeds, flowers, meadow grass, berry bush");
        let bush = CLUTTER.iter().find(|r| r.name == "berry bush").expect("a berry bush");
        assert!(bush.looks.iter().any(|l| l.1 >> 3 & 1 == 1), "berries in late summer");
        let flowers = CLUTTER.iter().find(|r| r.name == "flowers").unwrap();
        assert_eq!((flowers.kind, flowers.seasons), (Kind::Plant, 0b11), "flowers in spring and summer");
    }

    /// The parts of the year: eighths, early and late halves of `season`'s seasons (ADR 0091).
    #[test]
    fn parts_halve_the_seasons() {
        let up = glam::DVec3::new(0.3, 0.0, 0.9).normalize();
        let y = crate::sky::YEAR;
        let mut seen = [false; 8];
        for k in 0..400 {
            let t = y * k as f64 / 400.0;
            let p = part(up, t);
            seen[p as usize] = true;
            assert_eq!(season(up, t), p / 2);
            let phase = crate::sky::season_at(up, t).0;
            assert_eq!(p % 2 == 1, phase - phase.round() >= 0.0, "t {t}: late is past the middle");
        }
        assert!(seen.iter().all(|&s| s));
        assert_eq!(part_bits("summer"), Some(0b1100));
        assert_eq!(part_bits("late summer"), Some(0b1000));
        assert_eq!(part_bits("early winter"), Some(0b0100_0000));
        assert_eq!(part_bits("late"), None);
    }

    /// Dense meadows (ADR 0091): plants cover most open meadow; the Vegetation setting thins them as a subset
    /// and moves no pick-up and no decoration.
    #[test]
    fn vegetation_scales_plants_only() {
        let rows = &*CLUTTER;
        let bi = crate::biomes::ROWS.biomes.iter().position(|b| b.name == "grassland").unwrap() as u8;
        let meadow = crate::blocks::BLOCKS.id("meadow grass");
        let w = crate::world::World::with_map(256, 32, 1, None);
        let body = &w.bodies[0];
        let n = 200 * 200;
        let mut count = [0usize; 3];
        for i in 0..200 {
            for j in 0..200 {
                let at = |v: f32| plant(9, 1, i, j, bi, meadow, 1, v, &[]);
                let k = VEGETATION.map(at);
                for (c, t) in count.iter_mut().zip(&k) {
                    *c += t.is_some() as usize;
                }
                if let Some(t) = k[0] {
                    assert_eq!(k[2], Some(t), "few is a subset of many");
                    assert_eq!(CLUTTER[t.row].kind, Kind::Plant);
                }
                // the review of 2026-11-13: plants shared the models' roll and hopped with the Clutter setting and
                // the seasons' models; now that roll lays no plant, and `plant` takes no Clutter density
                for d in [0.0, 0.25, 4.0] {
                    if let Some(m) = pick(body, 9, 1, i, j, bi, meadow, 1, d, &[]) {
                        assert_ne!(CLUTTER[m.row].kind, Kind::Plant);
                    }
                }
            }
        }
        // short grass fills what the tall leaves open: no bare ground between tufts (the overseer, 2026-11-13)
        assert!(count[1] * 100 > n * 85, "85 % or more of open meadow has a plant at normal: {count:?} of {n}");
        // the overseer's call of 2026-11-13: kinds grow in drifts of their own, not sprinkled evenly; a uniform
        // pick would make two neighbours alike about (0.45/0.75)^2 + .. = 0.42 of the time
        let (mut same, mut pairs) = (0, 0);
        for i in 0..200 {
            for j in 0..200 {
                if let (Some(a), Some(b)) = (plant(9, 1, i, j, bi, meadow, 1, 1.0, &[]), plant(9, 1, i + 1, j, bi, meadow, 1, 1.0, &[])) {
                    pairs += 1;
                    same += (a.row == b.row) as usize;
                }
            }
        }
        assert!(same * 10 > pairs * 6, "neighbours alike {same} of {pairs}");
        assert!(count[0] < count[1] && count[1] < count[2], "{count:?}");
    }

    /// The tint (ADR 0091): yellow-green where it is dry and warm, deep green where it is wet, yellower in autumn
    /// than in spring, its bits round-trip.
    #[test]
    fn the_tint_follows_the_climate_and_the_season() {
        let (dry, wet) = (tint(0.9, 0.05, 2, 1.0), tint(0.6, 0.95, 2, 1.0));
        assert!(dry[0] > dry[2] * 2.0 && dry[0] > dry[1] * 0.95, "yellow-green: {dry:?}");
        assert!(dry.iter().chain(&wet).all(|&v| v <= 1.4), "it hardly brightens: {dry:?}");
        // a painting's mid green (0.3, 0.6, 0.15) comes out as a dry grass's yellow where it is driest, as the
        // savanna's ground is: red at least four fifths of green
        let g = tint(1.0, 0.0, 2, 1.0);
        assert!(0.3 * g[0] >= 0.8 * 0.6 * g[1], "dry grass yellow: {g:?}");
        // and a temperate meadow (heat and wetness halfway) stays green, red under green
        let m = tint(0.5, 0.5, 2, 1.0);
        assert!(m[0] < m[1], "a temperate meadow green: {m:?}");
        assert!(wet[1] > wet[0] && wet[1] > wet[2] && wet[1] < 1.0, "deep green: {wet:?}");
        assert!(dry[0] / dry[1] > wet[0] / wet[1] + 0.2);
        let (spring, autumn) = (tint(0.5, 0.5, 0, 1.0), tint(0.5, 0.5, 4, 1.0));
        assert!(autumn[0] / autumn[2] > spring[0] / spring[2], "{spring:?} {autumn:?}");
        assert!(tint(0.5, 0.5, 7, 1.0)[1] < tint(0.5, 0.5, 2, 1.0)[1], "dull in winter");
        // the grass block (season 0) and the plants at a season's middle (part 2's shift is 0) are one colour
        assert_eq!(tint(0.7, 0.3, 2, 1.0), tint(0.7, 0.3, 5, 0.0));
        assert_eq!(tint(0.5, 0.5, 4, 0.0), tint(0.5, 0.5, 0, 0.0), "no season where the place has none");
    }

    /// `may` bounds `pick`: whatever `pick` lays, `may` lets through.
    #[test]
    fn may_lets_through_all_that_pick_lays() {
        let bi = crate::biomes::ROWS.biomes.iter().position(|b| b.name == "temperate forest").unwrap() as u8;
        let grass = crate::blocks::BLOCKS.id("grass");
        let w = crate::world::World::with_map(256, 32, 1, None);
        let body = &w.bodies[0];
        let (mut laid, mut asked) = (0, 0);
        for i in 0..300 {
            for j in 0..300 {
                for d in [0.0, 1.0, 4.0] {
                    let m = may(7, 2, i, j, bi, [d, d]);
                    asked += m as usize;
                    if pick(body, 7, 2, i, j, bi, grass, 1, d, &[(150, 150)]).is_some() || plant(7, 2, i, j, bi, grass, 1, d, &[(150, 150)]).is_some() {
                        laid += 1;
                        assert!(m);
                    }
                }
            }
        }
        assert!(laid > 0 && asked < 300 * 300 * 2, "{asked} asked");
    }

    /// A lower Clutter setting is a subset of a higher one, pick-ups do not change with it, and the season and
    /// the ground choose the rows.
    #[test]
    fn density_thins_decoration_only_and_seasons_choose() {
        let rows = &*CLUTTER;
        let bi = crate::biomes::ROWS.biomes.iter().position(|b| b.name == "temperate forest").unwrap() as u8;
        let grass = crate::blocks::BLOCKS.id("grass");
        let w = crate::world::World::with_map(256, 32, 1, None);
        let body = &w.bodies[0];
        let trees = [(100, 100)];
        let (mut few, mut many, mut picks) = (0, 0, 0);
        let mut by_season = [0usize; 4];
        for i in 0..200 {
            for j in 0..200 {
                let f = pick(body, 7, 2, i, j, bi, grass, 2, 0.25, &trees);
                let m = pick(body, 7, 2, i, j, bi, grass, 2, 4.0, &trees);
                if let Some(f) = f {
                    assert_eq!(Some(f), m, "few is a subset of many");
                    few += 1;
                }
                many += m.is_some() as usize;
                if let Some(t) = m.filter(|t| rows[t.row].kind == Kind::Pickup) {
                    picks += 1;
                    assert_eq!(pick(body, 7, 2, i, j, bi, grass, 2, 0.0, &trees), Some(t), "a pick-up stays with clutter off");
                }
                assert!(pick(body, 7, 2, i, j, bi, crate::blocks::BLOCKS.id("stone"), 2, 4.0, &[]).is_none_or(|t| rows[t.row].on.contains(&crate::blocks::BLOCKS.id("stone"))));
                for (s, n) in by_season.iter_mut().enumerate() {
                    if let Some(t) = pick(body, 7, 2, i, j, bi, grass, s as u8, 1.0, &trees).or(plant(7, 2, i, j, bi, grass, s as u8, 1.0, &trees)) {
                        assert!(rows[t.row].seasons >> s & 1 == 1);
                        *n += 1;
                    }
                }
            }
        }
        assert!(few > 0 && many > 3 * few, "few {few}, many {many}");
        assert!(picks > 0);
        assert!(by_season.iter().all(|&n| n > 0), "{by_season:?}");
    }
}
