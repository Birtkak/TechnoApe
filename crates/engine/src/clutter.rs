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
        let planes = match (r.planes, r.kind) {
            (None, Kind::Plant) => 2,
            (Some(n @ 2..=4), Kind::Plant) => n,
            (None, _) => 0,
            _ => return Err(format!("{who}: planes are 2 to 4, on a plant")),
        };
        if !r.looks.is_empty() && r.kind != Kind::Plant {
            return Err(format!("{who}: looks are a plant's"));
        }
        let mut looks = Vec::new();
        for (n, parts) in &r.looks {
            let mut m = 0u8;
            for p in parts {
                m |= part_bits(p).ok_or(format!("{who}: look '{n}': no part of the year '{p}' (a season, or early or late one)"))?;
            }
            looks.push((n.clone(), m));
        }
        out.push(Row { name: r.name, kind: r.kind, gives, on, near: r.near, seasons, regrow, planes, looks });
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

/// What lies on column (face, i, j) of a body with `seed`, of biome `biome` and generated top block `ground`, in
/// `season` (0..4): one roll over the biome's rows of that ground and season, pick-ups at their full weight
/// (what the server answers for), then decoration at `density[0]` times its own (the Clutter setting: off 0,
/// few 0.25, normal 1, many 4), then plants at `density[1]` times theirs (the Vegetation setting, `VEGETATION`),
/// so a lower setting is a subset of a higher one. `trees`: the trunks near the column.
#[allow(clippy::too_many_arguments)]
pub fn pick(seed: u64, face: u8, i: i32, j: i32, biome: u8, ground: u16, season: u8, density: [f32; 2], trees: &[(i32, i32)]) -> Option<Thing> {
    let b = crate::biomes::ROWS.biomes.get(biome as usize)?;
    if b.clutter.is_empty() {
        return None;
    }
    let rows = &*CLUTTER;
    let fits = |r: &Row| r.on.contains(&ground) && r.seasons >> season & 1 == 1;
    let weight = |k: Kind| b.clutter.iter().filter(|c| rows[c.0].kind == k && fits(&rows[c.0])).map(|c| c.1 / 100.0).sum::<f32>();
    let (wp, wd, wl) = (weight(Kind::Pickup), weight(Kind::Decoration), weight(Kind::Plant));
    let h = roll(seed, face, i, j);
    let u = unit(h);
    let (kind, w) = if u < wp {
        (Kind::Pickup, wp)
    } else if u < wp + wd * density[0] {
        (Kind::Decoration, wd)
    } else if u < wp + wd * density[0] + wl * density[1] {
        (Kind::Plant, wl)
    } else {
        return None;
    };
    // which of that kind, by a second roll over their weights
    let mut v = unit(mix(h ^ 0x5eed)) * w;
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

fn roll(seed: u64, face: u8, i: i32, j: i32) -> u64 {
    mix(seed ^ 0xc1_0773_5000 ^ (face as u64) << 58 ^ (i as u32 as u64) << 29 ^ j as u32 as u64)
}

/// Whether anything can lie on the column at all: its roll against every row of the biome, of any ground and
/// season. A bound on `pick` that costs one hash, so the mesher asks the ground and the season only of the few
/// columns that pass (most do not).
pub fn may(seed: u64, face: u8, i: i32, j: i32, biome: u8, density: [f32; 2]) -> bool {
    let Some(b) = crate::biomes::ROWS.biomes.get(biome as usize) else { return false };
    let mut w = 0.0;
    for c in &b.clutter {
        w += c.1 / 100.0 * match CLUTTER[c.0].kind {
            Kind::Pickup => 1.0,
            Kind::Decoration => density[0],
            Kind::Plant => density[1],
        };
    }
    unit(roll(seed, face, i, j)) < w
}

/// The Vegetation setting's density of plants (few, normal, many; ADR 0091).
pub const VEGETATION: [f32; 3] = [0.4, 1.0, 1.6];

/// The climate a plant's tint is made from at layer `z` of column (face, i, j) (ADR 0091): heat and wetness,
/// 0 to 1, from the bake's year (the air's temperature at the ground, -5 to 35 C, and the rain, 0 to 6 mm a day
/// on a root scale). None on a body with no climate.
pub fn climate(body: &Body, face: u8, i: i32, j: i32, z: i32) -> Option<[f32; 2]> {
    let (theta, _, rain) = body.climate_at(face as usize, i as f64 + 0.5, j as f64 + 0.5)?;
    // a layer is a metre (ADR 0033); the air cools 6.5 K a km over the sea
    let t = theta - 0.0065 * (z as f64 - body.sea.unwrap_or(0.0)).max(0.0) as f32;
    Some([((t + 5.0) / 40.0).clamp(0.0, 1.0), (rain.max(0.0) / 6.0).sqrt().clamp(0.0, 1.0)])
}

/// The tint a plant's green takes, a factor on its painting's colour (ADR 0091): yellow-green where it is dry
/// and warm, deep green where it is wet, fresh in spring, yellowing from late summer, dull in winter. `part` is
/// the part of the year (`part`). The shader's `climate_tint` is this, line for line; leaves and the grass block
/// can take it the same way.
pub fn tint(heat: f32, wet: f32, part: u8) -> [f32; 3] {
    const SHIFT: [f32; 8] = [-0.2, -0.1, 0.0, 0.15, 0.35, 0.5, 0.4, 0.3];
    let dry = ((1.0 - wet) * (0.35 + 0.65 * heat) + SHIFT[part as usize & 7]).clamp(0.0, 1.0);
    let (wet_c, dry_c) = ([0.62, 0.86, 0.56], [1.12, 1.0, 0.42]);
    let dim = if part >= 6 { 0.82 } else { 1.0 };
    std::array::from_fn(|k| (wet_c[k] + (dry_c[k] - wet_c[k]) * dry) * dim)
}

/// The season (0..4, `SEASONS`) at time `t` over a point of direction `up` from its body's centre: the nearest
/// season's middle (`sky::season_at`).
pub fn season(up: glam::DVec3, t: f64) -> u8 {
    part(up, t) / 2
}

/// Where clutter would lie on level-0 column (i, j) of `tile`'s footprint (tile of face, x, y): the first layer
/// of generated air over it, and its generated top block and biome; None at a face's last columns, under the sea
/// or in a river or lake. The one ground the mesher and the server ask (ADR 0082).
pub fn ground(body: &Body, tile: &HeightTile, face: u8, i: i32, j: i32) -> Option<(i32, u16, u8)> {
    let (x, y) = (i.div_euclid(CS), j.div_euclid(CS));
    let k = (i - x * CS + 1) as usize + (j - y * CS + 1) as usize * P;
    let r = crate::biomes::REACH;
    if body.baked_seed().is_none() || i < r || j < r || i >= body.n - r || j >= body.n - r {
        return None;
    }
    let z = body.first_air(tile.h[k]);
    if body.sea.is_some_and(|s| z as f64 <= s) || body.water_at(face, i, j).is_some_and(|w| z as f64 <= w.0) {
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
    let body = world.bodies.get(b.body as usize)?;
    let seed = body.baked_seed()?;
    let (i, j) = (b.pos.x, b.pos.y);
    let tile = body.tile(b.face, 0, i.div_euclid(CS), j.div_euclid(CS));
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
    pick(seed, b.face, i, j, biome, top, season(up, t), [1.0, 1.0], &tile.trees).filter(|th| CLUTTER[th.row].kind == Kind::Pickup)
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
}

/// A face's flags in the table's word 13: a plant's plane, drawn from both sides, swayed, tinted and faded far
/// away (ADR 0091).
pub const PLANT: u32 = 1;
/// A plant's painting's pixels a metre (ADR 0081's 32 px/m): its size is its painting's.
pub const PLANT_PX: f32 = 32.0;

/// A plant's planes (ADR 0091): `n` upright planes through the cell's middle, turned by 180/n degrees each, as
/// wide and high as the painting at `PLANT_PX`; each face's corner is its painting's top left, `ev` down.
pub fn plant_faces(n: u8, w: u32, h: u32) -> Vec<([glam::Vec3; 3], [u32; 4])> {
    let (wm, hm) = (w as f32 / PLANT_PX, h as f32 / PLANT_PX);
    (0..n)
        .map(|k| {
            let a = std::f32::consts::PI * k as f32 / n as f32;
            let d = glam::Vec3::new(a.cos(), 0.0, a.sin()) * wm;
            ([glam::Vec3::new(0.5, hm, 0.5) - d * 0.5, d, glam::Vec3::new(0.0, -hm, 0.0)], [0, 0, w, h])
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
                    Some(s.push(&base, &p, plant_faces(r.planes, p.w, p.h), PLANT))
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
                s.rows.push(alts);
                s.looks.push(looks);
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
/// block has it); hi = the face's index in `Still::faces`, its turn in sixteenths at bit 16, and (ADR 0091) the
/// tint's `TINT` bits at bit 20: heat 4, wetness 4, the part of the year 3.
pub fn quad(x: u32, y: u32, z: u32, du: u32, dv: u32, face: u32, turn: u32, tint: u32) -> [u32; 2] {
    [x | y << 6 | z << 12 | du << 18 | dv << 22 | 7 << 28, face | turn << 16 | tint << 20]
}

/// A tint's 11 bits (ADR 0091; the shader's `climate_tint` reads them): heat and wetness in sixteenths, then the
/// part of the year. `d` (0..1) dithers the rounding, so a slope of climate shows no steps.
pub fn tint_bits(c: [f32; 2], part: u8, d: f32) -> u32 {
    let q = |v: f32| ((v * 15.0 + d).floor() as u32).min(15);
    q(c[0]) | q(c[1]) << 4 | (part as u32 & 7) << 8
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
    let mut edited = [false; P * P];
    for &(c, _) in edits {
        edited[c as usize % (P * P)] = true;
    }
    let c0 = key.pos * CS - 1;
    // the climate at the footprint's corners, asked once a chunk and only if a plant lies in it; between, linear
    let mut corners: Option<[[f32; 2]; 4]> = None;
    for v in 1..=CS as usize {
        for u in 1..=CS as usize {
            if edited[u + v * P] {
                continue;
            }
            let (i, j) = (c0.x + u as i32, c0.y + v as i32);
            // the roll first: one hash turns most columns away before the ground's lookups
            if !may(seed, key.face, i, j, tile.biome[u + v * P], density) {
                continue;
            }
            let Some((z, top, biome)) = ground(body, &tile, key.face, i, j) else { continue };
            let zl = z - c0.z;
            // the chunk that holds the ground cell lays it: that chunk is never all air
            if !(2..=CS + 1).contains(&zl) || cells[pidx(u, v, zl as usize)] != 0 || cells[pidx(u, v, zl as usize - 1)] != top {
                continue;
            }
            let p8 = part(body.dir(key.face as usize, i as f64 + 0.5, j as f64 + 0.5), t);
            let Some(t) = pick(seed, key.face, i, j, biome, top, p8 / 2, density, &tile.trees) else { continue };
            if !lies(&CLUTTER[t.row], taken.iter().find(|x| x.0 == IVec3::new(i, j, z)).map(|x| x.1), year) {
                continue;
            }
            let row = &CLUTTER[t.row];
            let alts = &still.rows[t.row];
            if alts.is_empty() {
                continue;
            }
            let (a, turn, du, dv) = t.shape(alts.len());
            let (first, n) = still.looks[t.row].iter().find(|l| l.0 >> p8 & 1 == 1).map_or(alts[a], |l| l.1);
            let tint = if row.kind == Kind::Plant {
                let k = corners.get_or_insert_with(|| {
                    let at = |x: i32, y: i32| climate(body, key.face, x, y, z).unwrap_or([0.5, 0.5]);
                    [at(c0.x + 1, c0.y + 1), at(c0.x + CS, c0.y + 1), at(c0.x + 1, c0.y + CS), at(c0.x + CS, c0.y + CS)]
                });
                let (fu, fv) = ((u - 1) as f32 / (CS - 1) as f32, (v - 1) as f32 / (CS - 1) as f32);
                let c = std::array::from_fn(|n| (k[0][n] * (1.0 - fu) + k[1][n] * fu) * (1.0 - fv) + (k[2][n] * (1.0 - fu) + k[3][n] * fu) * fv);
                tint_bits(c, p8, unit(mix(t.h ^ 0x7177)))
            } else {
                0
            };
            for f in first..first + n {
                out.push(quad(u as u32 - 1, v as u32 - 1, zl as u32 - 1, du, dv, f, turn, tint));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

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
        }
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
        let n = 200 * 200;
        let mut count = [0usize; 3];
        for i in 0..200 {
            for j in 0..200 {
                let at = |v: f32| pick(9, 1, i, j, bi, meadow, 1, [1.0, v], &[]);
                let k = VEGETATION.map(at);
                for (c, t) in count.iter_mut().zip(&k) {
                    *c += t.is_some_and(|t| rows[t.row].kind == Kind::Plant) as usize;
                }
                if let Some(t) = k[0] {
                    assert_eq!(k[2], Some(t), "few is a subset of many");
                }
                for t in k.iter().flatten().filter(|t| rows[t.row].kind != Kind::Plant) {
                    assert_eq!(pick(9, 1, i, j, bi, meadow, 1, [1.0, 0.0], &[]), Some(*t), "plants move nothing else");
                }
            }
        }
        assert!(count[1] * 2 > n, "most open meadow has a plant at normal: {count:?} of {n}");
        assert!(count[0] < count[1] && count[1] < count[2], "{count:?}");
    }

    /// The tint (ADR 0091): yellow-green where it is dry and warm, deep green where it is wet, yellower in autumn
    /// than in spring, its bits round-trip.
    #[test]
    fn the_tint_follows_the_climate_and_the_season() {
        let (dry, wet) = (tint(0.9, 0.05, 2), tint(0.6, 0.95, 2));
        assert!(dry[0] > dry[2] * 2.0 && dry[0] > dry[1] * 0.95, "yellow-green: {dry:?}");
        assert!(dry.iter().chain(&wet).all(|&v| v <= 1.12), "it hardly brightens: {dry:?}");
        assert!(wet[1] > wet[0] && wet[1] > wet[2] && wet[1] < 1.0, "deep green: {wet:?}");
        assert!(dry[0] / dry[1] > wet[0] / wet[1] + 0.2);
        let (spring, autumn) = (tint(0.5, 0.5, 0), tint(0.5, 0.5, 4));
        assert!(autumn[0] / autumn[2] > spring[0] / spring[2], "{spring:?} {autumn:?}");
        assert!(tint(0.5, 0.5, 7)[1] < tint(0.5, 0.5, 2)[1], "dull in winter");
        let b = tint_bits([1.0, 0.0], 5, 0.99);
        assert_eq!((b & 15, b >> 4 & 15, b >> 8), (15, 0, 5));
        assert_eq!(tint_bits([0.5, 0.5], 0, 0.0) & 255, 7 | 7 << 4);
        assert!(b < 1 << 11, "11 bits over the turn's 20");
    }

    /// `may` bounds `pick`: whatever `pick` lays, `may` lets through.
    #[test]
    fn may_lets_through_all_that_pick_lays() {
        let bi = crate::biomes::ROWS.biomes.iter().position(|b| b.name == "temperate forest").unwrap() as u8;
        let grass = crate::blocks::BLOCKS.id("grass");
        let (mut laid, mut asked) = (0, 0);
        for i in 0..300 {
            for j in 0..300 {
                for d in [0.0, 1.0, 4.0] {
                    let m = may(7, 2, i, j, bi, [d, d]);
                    asked += m as usize;
                    if pick(7, 2, i, j, bi, grass, 1, [d, d], &[(150, 150)]).is_some() {
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
        let trees = [(100, 100)];
        let (mut few, mut many, mut picks) = (0, 0, 0);
        let mut by_season = [0usize; 4];
        for i in 0..200 {
            for j in 0..200 {
                let f = pick(7, 2, i, j, bi, grass, 2, [0.25, 0.0], &trees);
                let m = pick(7, 2, i, j, bi, grass, 2, [4.0, 0.0], &trees);
                if let Some(f) = f {
                    assert_eq!(Some(f), m, "few is a subset of many");
                    few += 1;
                }
                many += m.is_some() as usize;
                if let Some(t) = m.filter(|t| rows[t.row].kind == Kind::Pickup) {
                    picks += 1;
                    assert_eq!(pick(7, 2, i, j, bi, grass, 2, [0.0, 0.0], &trees), Some(t), "a pick-up stays with clutter off");
                }
                assert!(pick(7, 2, i, j, bi, crate::blocks::BLOCKS.id("stone"), 2, [4.0, 1.0], &[]).is_none_or(|t| rows[t.row].on.contains(&crate::blocks::BLOCKS.id("stone"))));
                for (s, n) in by_season.iter_mut().enumerate() {
                    if let Some(t) = pick(7, 2, i, j, bi, grass, s as u8, [1.0, 1.0], &trees) {
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
