//! Clutter (docs/design/clutter-and-pickups.md, ADR 0082): the small things on the ground, a pure function of the
//! seed, the column and the season, drawn as still box models in the chunk's mesh (ADR 0068, 0081). Nothing is
//! stored for them; a column with an edit has none.
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
            (None, Kind::Decoration) => None,
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
        out.push(Row { name: r.name, kind: r.kind, gives, on, near: r.near, seasons, regrow });
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
/// (what the server answers for), decoration at `density` times its own (the Clutter setting: off 0, few 0.25,
/// normal 1, many 4), so a lower setting is a subset of a higher one. `trees`: the trunks near the column.
#[allow(clippy::too_many_arguments)]
pub fn pick(seed: u64, face: u8, i: i32, j: i32, biome: u8, ground: u16, season: u8, density: f32, trees: &[(i32, i32)]) -> Option<Thing> {
    let b = crate::biomes::ROWS.biomes.get(biome as usize)?;
    if b.clutter.is_empty() {
        return None;
    }
    let rows = &*CLUTTER;
    let fits = |r: &Row| r.on.contains(&ground) && r.seasons >> season & 1 == 1;
    let weight = |k: Kind| b.clutter.iter().filter(|c| rows[c.0].kind == k && fits(&rows[c.0])).map(|c| c.1 / 100.0).sum::<f32>();
    let (wp, wd) = (weight(Kind::Pickup), weight(Kind::Decoration));
    let h = roll(seed, face, i, j);
    let u = unit(h);
    let (kind, w) = if u < wp {
        (Kind::Pickup, wp)
    } else if u < wp + wd * density {
        (Kind::Decoration, wd)
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
pub fn may(seed: u64, face: u8, i: i32, j: i32, biome: u8, density: f32) -> bool {
    let Some(b) = crate::biomes::ROWS.biomes.get(biome as usize) else { return false };
    let (mut wp, mut wd) = (0.0, 0.0);
    for c in &b.clutter {
        if CLUTTER[c.0].kind == Kind::Pickup { wp += c.1 / 100.0 } else { wd += c.1 / 100.0 }
    }
    unit(roll(seed, face, i, j)) < wp + wd * density
}

/// The season (0..4, `SEASONS`) at time `t` over a point of direction `up` from its body's centre: the nearest
/// season's middle (`sky::season_at`).
pub fn season(up: glam::DVec3, t: f64) -> u8 {
    (crate::sky::season_at(up, t).0.round() as i64).rem_euclid(4) as u8
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

/// The year (whole years of `sky::YEAR`) at time `t`: a regrowing pick-up taken in an earlier one is back.
pub fn year(t: f64) -> u32 {
    (t / crate::sky::YEAR).floor().max(0.0) as u32
}

/// When level 0 must lay its clutter again: the season nearest time `t` (a whole count of them) and its year.
pub fn stamp(t: f64) -> (i64, u32) {
    ((t / crate::sky::YEAR * 4.0).round() as i64, year(t))
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
    pick(seed, b.face, i, j, biome, top, season(up, t), 1.0, &tile.trees).filter(|th| CLUTTER[th.row].kind == Kind::Pickup)
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
}

/// The most faces the table holds: a still quad names its face in 16 bits.
pub const MAX_FACES: usize = 1 << 16;

impl Still {
    /// Each clutter row's model from `dir`: `<name>.toml` and `<name>.png`, and the alternates `<name>.1` to `.4`
    /// beside it. A missing or refused file is named in the list returned, and its row (or that alternate)
    /// draws nothing.
    pub fn load(dir: &std::path::Path, palette: &Palette) -> (Still, Vec<String>) {
        let mut s = Still::default();
        let mut bad = Vec::new();
        for r in CLUTTER.iter() {
            let mut alts = Vec::new();
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
                match read() {
                    Ok((m, p)) => {
                        let faces = m.rest_faces();
                        if s.faces.len() + faces.len() > MAX_FACES {
                            bad.push(format!("{base}: the still faces' table is full ({MAX_FACES})"));
                            continue;
                        }
                        let base_tx = s.paint.len() as u32;
                        s.paint.extend(p.px.iter().map(|c| u32::from_le_bytes(*c)));
                        alts.push((s.faces.len() as u32, faces.len() as u32));
                        for ([o, eu, ev], [x, y, w, h]) in faces {
                            let f = |v: f32| v.to_bits();
                            s.faces.push([f(o.x), f(o.y), f(o.z), base_tx, f(eu.x), f(eu.y), f(eu.z), p.w, f(ev.x), f(ev.y), f(ev.z), x | y << 16, w | h << 16, 0, 0, 0]);
                        }
                    }
                    Err(e) => bad.push(format!("data/models/{base}: {e}")),
                }
            }
            s.rows.push(alts);
        }
        (s, bad)
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
pub fn quads(body: &Body, key: Key, cells: &[u16; P * P * P], edits: &[(u32, u16)], still: &Still, t: f64, density: f32, taken: &[(IVec3, u32)]) -> Vec<[u32; 2]> {
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
            let sn = season(body.dir(key.face as usize, i as f64 + 0.5, j as f64 + 0.5), t);
            let Some(t) = pick(seed, key.face, i, j, biome, top, sn, density, &tile.trees) else { continue };
            if !lies(&CLUTTER[t.row], taken.iter().find(|x| x.0 == IVec3::new(i, j, z)).map(|x| x.1), year) {
                continue;
            }
            let alts = &still.rows[t.row];
            if alts.is_empty() {
                continue;
            }
            let (a, turn, du, dv) = t.shape(alts.len());
            let (first, n) = alts[a];
            for f in first..first + n {
                out.push(quad(u as u32 - 1, v as u32 - 1, zl as u32 - 1, du, dv, f, turn));
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
        assert!(rows.iter().filter(|r| r.kind == Kind::Decoration).count() >= 5);
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
        let (s, bad) = Still::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/models"), &pal);
        assert!(bad.is_empty(), "{bad:?}");
        assert_eq!(s.rows.len(), CLUTTER.len());
        assert!(s.rows.iter().all(|a| !a.is_empty()), "every row has a model");
        assert!(s.faces.len() < 2000, "{} faces", s.faces.len());
        // in the cell's metres: every corner within a block of the cell, none of them in pixels or voxels
        for f in &s.faces {
            let o = [0, 1, 2].map(|k| f32::from_bits(f[k]));
            assert!(o.iter().all(|v| (-0.5..=1.5).contains(v)), "{o:?}");
            assert!(o[1] >= -0.05, "nothing below the ground: {o:?}");
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
        assert_eq!(stamp(y * 0.3), stamp(y * 0.32));
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

    /// `may` bounds `pick`: whatever `pick` lays, `may` lets through.
    #[test]
    fn may_lets_through_all_that_pick_lays() {
        let bi = crate::biomes::ROWS.biomes.iter().position(|b| b.name == "temperate forest").unwrap() as u8;
        let grass = crate::blocks::BLOCKS.id("grass");
        let (mut laid, mut asked) = (0, 0);
        for i in 0..300 {
            for j in 0..300 {
                for d in [0.0, 1.0, 4.0] {
                    let m = may(7, 2, i, j, bi, d);
                    asked += m as usize;
                    if pick(7, 2, i, j, bi, grass, 1, d, &[(150, 150)]).is_some() {
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
                let f = pick(7, 2, i, j, bi, grass, 2, 0.25, &trees);
                let m = pick(7, 2, i, j, bi, grass, 2, 4.0, &trees);
                if let Some(f) = f {
                    assert_eq!(Some(f), m, "few is a subset of many");
                    few += 1;
                }
                many += m.is_some() as usize;
                if let Some(t) = m.filter(|t| rows[t.row].kind == Kind::Pickup) {
                    picks += 1;
                    assert_eq!(pick(7, 2, i, j, bi, grass, 2, 0.0, &trees), Some(t), "a pick-up stays with clutter off");
                }
                assert!(pick(7, 2, i, j, bi, crate::blocks::BLOCKS.id("stone"), 2, 4.0, &[]).is_none_or(|t| rows[t.row].on.contains(&crate::blocks::BLOCKS.id("stone"))));
                for (s, n) in by_season.iter_mut().enumerate() {
                    if let Some(t) = pick(7, 2, i, j, bi, grass, s as u8, 1.0, &trees) {
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
