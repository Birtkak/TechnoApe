//! Ore bodies as counted objects (W4 stage 10, ADR 0063; `docs/design/ore-budget.md`). Each body is an
//! ellipsoid in a face's grid, of a kind from `data/minerals.toml`, placed from the map and the seed when a
//! planet loads (the same on the server and every client) where its geology is, and the first of a kind in the
//! ring from the spawn the budget names. Its cells are the solid generated ground inside it: `Body::generated`
//! puts the ore there, the mesher's fill too (`world::generate`), and `count` counts them for the ledger.
use crate::bake::rock::NAMES;
use crate::biomes::ROWS;
use crate::blocks::BLOCKS;
use crate::rock::ROCKS;
use crate::world::{Body, SEA_DEPTH};
use glam::DVec3;
use serde::Deserialize;
use std::sync::LazyLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Shape {
    Lump,
    Vein,
    Seam,
    Layer,
    Blanket,
    Cap,
    Bog,
    Gravel,
}

impl Shape {
    /// Its width over its length, and its half height: metres, or a share of its length when negative.
    fn form(self) -> (f64, f64) {
        match self {
            Shape::Lump => (1.0, -0.75),
            Shape::Vein => (0.0, -0.6),
            Shape::Seam => (0.7, 1.0),
            Shape::Layer => (0.7, 1.5),
            Shape::Blanket => (0.8, 0.5),
            Shape::Cap => (1.0, 1.5),
            Shape::Bog => (0.8, 0.75),
            Shape::Gravel => (0.3, 0.75),
        }
    }
    /// Lies at a depth under the ground of each column (a bed, a blanket), not at a layer.
    pub fn follows_ground(self) -> bool {
        matches!(self, Shape::Blanket | Shape::Cap | Shape::Bog | Shape::Gravel)
    }
    /// Its half axes (length, width, height) for a body of `blocks` cubic metres: 4/3 pi a b h = blocks.
    pub fn axes(self, blocks: f64) -> (f64, f64, f64) {
        let v = blocks / (4.0 / 3.0 * std::f64::consts::PI);
        if self == Shape::Vein {
            // a metre wide each way: two metres thin
            let a = (v / 0.6).sqrt();
            return (a, 1.0, 0.6 * a);
        }
        let (w, h) = self.form();
        let a = if h < 0.0 { (v / (w * -h)).cbrt() } else { (v / (w * h)).sqrt() };
        (a, w * a, if h < 0.0 { -h * a } else { h })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawKind {
    name: String,
    block: String,
    gives: String,
    shape: Shape,
    count: u32,
    blocks: u32,
    depth: [f64; 2],
    provinces: Vec<String>,
    host: Option<String>,
    #[serde(default)]
    biomes: Vec<String>,
    near: Option<[f64; 2]>,
    gravel: Option<String>,
    #[serde(default)]
    gossan: f64,
    #[serde(default)]
    halo: Vec<String>,
    #[serde(default)]
    halo_m: f64,
    sample: Option<String>,
    pressure: Option<[f64; 2]>,
    heat: Option<[f64; 2]>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    kind: Vec<RawKind>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Kind {
    pub name: String,
    pub block: u16,
    /// The element the ledger counts it as; empty for the rusty ground over a body (no ore).
    pub gives: String,
    pub shape: Shape,
    pub count: u32,
    pub blocks: u32,
    pub depth: [f64; 2],
    pub provinces: Vec<u8>,
    pub host: Option<u16>,
    pub biomes: Vec<u8>,
    pub near: Option<[f64; 2]>,
    pub gravel: Option<usize>,
    pub gossan: f64,
    /// The altered rock around it (ADR 0095), outermost grade first; empty for none.
    pub halo: Vec<u16>,
    /// How far out the halo reaches from the ore, metres.
    pub halo_m: f64,
    /// The item that lies loose on the ground over its bodies (a sample, clutter's), by name: the items' rows
    /// read these, so `items::Items::parse` holds it to a row.
    pub sample: Option<String>,
    /// Where it forms (ADR 0158): a body only where `Body::pressure` (MPa) and `Body::heat`
    /// (C) at its middle lie in these [min, max]. A kind with a window forms in the mantle, on a body with one.
    pub pressure: Option<[f64; 2]>,
    pub heat: Option<[f64; 2]>,
}

impl Kind {
    /// It has a window of pressure or heat: it forms deep, its bodies are not cut at the crust's old floor.
    pub fn deep(&self) -> bool {
        self.pressure.is_some() || self.heat.is_some()
    }
}

/// The farthest any halo reaches, metres: `World::set_ores` registers a body this much wider.
pub const HALO_MAX: f64 = 12.0;

#[derive(Debug)]
pub struct Minerals {
    pub kinds: Vec<Kind>,
    /// The kind of rusty ground over a deep body (`gossan`), the last.
    pub gossan: usize,
}

impl Minerals {
    /// Reads and checks the file: every kind names an ore block, provinces, biomes and a gravel kind that exist,
    /// depths and distances in order, a gravel kind is gravel.
    pub fn parse(text: &str) -> Result<Minerals, String> {
        let f: File = toml::from_str(text).map_err(|e| e.to_string())?;
        let block = |owner: &str, n: &str| BLOCKS.placeable().find(|d| d.name == n).filter(|d| d.solid).map(|d| d.id).ok_or(format!("kind '{owner}': no solid block '{n}'"));
        let mut kinds = Vec::new();
        for r in &f.kind {
            let provinces = r.provinces.iter().map(|p| NAMES.iter().position(|n| n == p).map(|i| i as u8).ok_or(format!("kind '{}': no province '{p}'", r.name))).collect::<Result<Vec<u8>, String>>()?;
            let biomes = r.biomes.iter().map(|b| ROWS.biomes.iter().position(|o| &o.name == b).map(|i| i as u8).ok_or(format!("kind '{}': no biome '{b}'", r.name))).collect::<Result<Vec<u8>, String>>()?;
            let gravel = match &r.gravel {
                None => None,
                Some(g) => match f.kind.iter().position(|o| &o.name == g) {
                    Some(k) if f.kind[k].shape == Shape::Gravel => Some(k),
                    _ => return Err(format!("kind '{}': '{g}' is no gravel kind", r.name)),
                },
            };
            let deep = r.pressure.is_some() || r.heat.is_some();
            let window = |w: Option<[f64; 2]>, lo: f64, hi: f64| w.is_none_or(|w| lo <= w[0] && w[0] < w[1] && w[1] <= hi);
            if !window(r.pressure, 0.0, 1000.0) || !window(r.heat, -50.0, 2000.0) || deep && (r.shape.follows_ground() || !r.halo.is_empty() || r.gossan > 0.0 || r.gravel.is_some()) {
                return Err(format!("kind '{}': a window is [min, max], min under max, pressure 0..1000 MPa, heat -50..2000 C, for a body at a depth with no halo, gossan or gravel", r.name));
            }
            // (after the gossan's index: the crust's kinds and the bodies they place are as before the windows)
            if !deep && kinds.iter().any(|k: &Kind| k.deep()) {
                return Err(format!("kind '{}': the kinds with a window come last", r.name));
            }
            // the mantle's kinds lie down to its floor; the crust's to 180 m
            let floor = if deep { 1400.0 } else { 180.0 };
            if !(r.depth[0] >= 0.0 && r.depth[0] <= r.depth[1] && r.depth[1] <= floor) || r.near.is_some_and(|n| !(n[0] >= 0.0 && n[0] < n[1])) || r.blocks == 0 || r.gives.is_empty() {
                return Err(format!("kind '{}': depth 0..{floor} m in order, near in order, blocks over 0, gives an element", r.name));
            }
            if r.host.is_some() && r.shape.follows_ground() {
                return Err(format!("kind '{}': a host rock is for a body at a depth, not one under the ground", r.name));
            }
            let host = r.host.as_ref().map(|h| block(&r.name, h)).transpose()?;
            if !r.halo.is_empty() && (r.shape.follows_ground() || !(r.halo_m > 0.0 && r.halo_m <= HALO_MAX) || r.halo.len() > 3) {
                return Err(format!("kind '{}': a halo is for a body at a depth, one to three grades, halo_m over 0 and at most {HALO_MAX}", r.name));
            }
            let halo = r.halo.iter().map(|h| block(&r.name, h)).collect::<Result<Vec<u16>, String>>()?;
            kinds.push(Kind { name: r.name.clone(), block: block(&r.name, &r.block)?, gives: r.gives.clone(), shape: r.shape, count: r.count, blocks: r.blocks, depth: r.depth, provinces, host, biomes, near: r.near, gravel, gossan: r.gossan, halo, halo_m: r.halo_m, sample: r.sample.clone(), pressure: r.pressure, heat: r.heat });
        }
        if kinds.iter().map(|k| &k.name).collect::<std::collections::BTreeSet<_>>().len() != kinds.len() {
            return Err("two kinds share a name".into());
        }
        // the gossan after the crust's kinds, before the deep ones: its index (in every body of it) is as before
        let gossan = kinds.iter().position(|k| k.deep()).unwrap_or(kinds.len());
        kinds.insert(gossan, Kind { name: "gossan".into(), block: BLOCKS.id("gossan"), gives: String::new(), shape: Shape::Blanket, count: 0, blocks: 1, depth: [0.5, 0.5], provinces: Vec::new(), host: None, biomes: Vec::new(), near: None, gravel: None, gossan: 0.0, halo: Vec::new(), halo_m: 0.0, sample: None, pressure: None, heat: None });
        Ok(Minerals { kinds, gossan })
    }
}

pub static MINERALS: LazyLock<Minerals> = LazyLock::new(|| Minerals::parse(include_str!("../../../data/minerals.toml")).unwrap_or_else(|e| panic!("data/minerals.toml: {e}")));

/// One ore body: an ellipsoid of half axes (a along `angle`, b across, h up) in a face's grid. `at` is (column
/// i, column j, layer), or for a kind that follows the ground (column i, column j, metres under the ground).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OreBody {
    pub kind: u16,
    pub face: u8,
    pub at: DVec3,
    pub a: f64,
    pub b: f64,
    pub h: f64,
    pub angle: f64,
}

impl OreBody {
    /// Where column (i, j) lies across it: 0 at its middle, 1 at its rim; None outside.
    pub fn across(&self, i: i32, j: i32) -> Option<f64> {
        let (dx, dy) = (i as f64 + 0.5 - self.at.x, j as f64 + 0.5 - self.at.y);
        let (c, s) = (self.angle.cos(), self.angle.sin());
        let (u, v) = ((dx * c + dy * s) / self.a, (dy * c - dx * s) / self.b);
        let q = u * u + v * v;
        (q < 1.0).then_some(q)
    }
    /// The layers [lo, hi) it holds in column (i, j) of `body`, whose ground there is at radius `ground`.
    pub fn span(&self, body: &Body, i: i32, j: i32, ground: f64) -> Option<(i32, i32)> {
        let q = self.across(i, j)?;
        let hh = self.h * (1.0 - q).sqrt();
        // the layers whose middles lie inside: z + 0.5 between the two ends
        let kind = &MINERALS.kinds[self.kind as usize];
        let (bot, top) = if kind.shape.follows_ground() {
            (body.layer_of(ground - (self.at.z + hh)), body.layer_of(ground - (self.at.z - hh)))
        } else {
            (self.at.z - hh, self.at.z + hh)
        };
        // no crust's ore under the old worlds' floor (ADR 0152): the mantle holds only the kinds that form there
        let floor = if kind.deep() { 0 } else { body.mantle };
        let (lo, hi) = (((bot - 0.5).ceil() as i32).max(floor), (top - 0.5).floor() as i32 + 1);
        (lo < hi).then_some((lo, hi))
    }
    /// Ingots an ore block gives at column (i, j): 3 in its middle, 2 further out, 1 at its rim (ore-budget.md).
    pub fn grade(&self, i: i32, j: i32) -> u32 {
        match self.across(i, j) {
            Some(q) if q < 0.25 => 3,
            Some(q) if q < 0.6 => 2,
            _ => 1,
        }
    }
    /// Cell (i, j, z) in its own frame: along, across, up, metres from its middle.
    fn local(&self, i: i32, j: i32, z: i32) -> (f64, f64, f64) {
        let (dx, dy) = (i as f64 + 0.5 - self.at.x, j as f64 + 0.5 - self.at.y);
        let (c, s) = (self.angle.cos(), self.angle.sin());
        (dx * c + dy * s, dy * c - dx * s, z as f64 + 0.5 - self.at.z)
    }
    /// Whether a point of its frame (`local`) lies inside it with each half axis grown by `t` metres: a vein's
    /// halo a sheet, a lump's a shell; `t` out along its axes, a little less between them (off a vein's tip).
    fn grown(&self, (u, v, w): (f64, f64, f64), t: f64) -> bool {
        let (u, v, w) = (u / (self.a + t), v / (self.b + t), w / (self.h + t));
        u * u + v * v + w * w < 1.0
    }
    /// Whether cell (i, j, z) lies inside it grown by `t` metres (`grown`), for a body at a depth.
    pub fn within(&self, i: i32, j: i32, z: i32, t: f64) -> bool {
        self.grown(self.local(i, j, z), t)
    }
    /// The halo grade of cell (i, j, z) around it (0 the outermost), if its kind has a halo and the cell lies in
    /// it and not in the body: grade g reaches `halo_m` (1 - g / grades) out, each cell's reach frayed by a hash
    /// of the cell so a grade's edge is ragged (two grades mix over 30 % of the reach where they meet). The
    /// richest grade lies next to the ore.
    /// `mantle`: the body's mantle layers, taken off z for the hash.
    pub fn halo_grade(&self, face: u8, i: i32, j: i32, z: i32, mantle: i32) -> Option<usize> {
        let kind = &MINERALS.kinds[self.kind as usize];
        let n = kind.halo.len();
        if n == 0 {
            return None;
        }
        let p = self.local(i, j, z);
        if !self.grown(p, kind.halo_m * (1.0 + FRAY)) || self.grown(p, 0.0) {
            return None;
        }
        let h = crate::world::mix(0x4a10 ^ (face as u64) << 58 ^ (i as u32 as u64) << 29 ^ (j as u32 as u64) ^ ((z - mantle) as u32 as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15));
        let fray = (crate::world::unit(h) as f64 - 0.5) * 2.0 * FRAY;
        (0..n).rev().find(|&g| self.grown(p, kind.halo_m * (1.0 - g as f64 / n as f64 - fray)))
    }
    /// The layers [lo, hi) its halo can reach in column (i, j): its box widened by the halo's reach, None
    /// where the column lies outside that (the mesher's cut before it asks `halo_grade` per cell).
    pub fn halo_span(&self, i: i32, j: i32) -> Option<(i32, i32)> {
        let r = MINERALS.kinds[self.kind as usize].halo_m * (1.0 + FRAY) + 1.0;
        let (dx, dy) = (i as f64 + 0.5 - self.at.x, j as f64 + 0.5 - self.at.y);
        let (c, s) = (self.angle.cos(), self.angle.sin());
        let (u, v) = (dx * c + dy * s, dy * c - dx * s);
        (u.abs() < self.a + r && v.abs() < self.b + r).then(|| ((self.at.z - self.h - r).floor() as i32, (self.at.z + self.h + r).ceil() as i32 + 1))
    }
    /// Where column (i, j) lies across its samples' spread (its footprint, each half axis at least
    /// `SAMPLE_SPREAD_M`): 0 at its middle, 1 at the rim; None outside.
    pub fn spread(&self, i: i32, j: i32) -> Option<f64> {
        OreBody { a: self.a.max(SAMPLE_SPREAD_M), b: self.b.max(SAMPLE_SPREAD_M), ..*self }.across(i, j)
    }
    /// Metres from cell (i, j, z) of `body` to its surface, 0 inside.
    pub fn distance(&self, body: &Body, i: i32, j: i32, z: i32) -> f64 {
        let (u, v, mut w) = self.local(i, j, z);
        if MINERALS.kinds[self.kind as usize].shape.follows_ground() {
            // its middle is `at.z` metres under the ground of each column
            w = self.at.z - (body.col_height(self.face, i, j) - body.layer_r(z as f64 + 0.5));
        }
        to_surface([u, v, w], [self.a, self.b, self.h])
    }
    /// Its footprint's columns, a column to spare.
    pub fn columns(&self) -> impl Iterator<Item = (i32, i32)> {
        let r = self.a.max(self.b) + 1.0;
        let (i0, i1, j0, j1) = ((self.at.x - r).floor() as i32, (self.at.x + r).ceil() as i32, (self.at.y - r).floor() as i32, (self.at.y + r).ceil() as i32);
        let me = *self;
        (j0..=j1).flat_map(move |j| (i0..=i1).map(move |i| (i, j))).filter(move |&(i, j)| me.across(i, j).is_some())
    }
}

struct Rng(u64);
impl Rng {
    fn f(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// How ragged a halo grade's edge is, a share of `halo_m` either way.
pub const FRAY: f64 = 0.15;

/// Bodies keep this many columns from a face's seam, and from the spawn's own column.
const SEAM: f64 = 6.0;
const SPAWN: f64 = 8.0;

/// Every ore body of a baked planet, in a fixed order: kind by kind as the file lists them, each body's gravel
/// and rusty ground after it. Counts are the file's on the 4 km planet, scaled by area (at least one).
pub fn place(b: &Body, seed: u64) -> Vec<OreBody> {
    let Some(map) = b.map() else { return Vec::new() };
    if map.rock.is_empty() {
        return Vec::new();
    }
    let n = b.n as f64;
    let scale = (n / 6144.0).powi(2);
    let (sf, si, sj) = b.locate(b.spawn);
    // the river cells over the sea, by face, at their columns, with the way they flow
    let m = map.m as usize;
    // (a map cell's middle in the block grid, which the warp moves, ADR 0131)
    let cell_at = |c: u32| {
        let (i, j) = b.unwarp_col(((c as usize % m) as f64 + 0.5) * n / m as f64, (((c as usize / m) % m) as f64 + 0.5) * n / m as f64);
        ((c as usize / (m * m)), i, j)
    };
    let rivers: Vec<(usize, f64, f64, f64)> = map
        .rivers
        .iter()
        .map(|r| {
            let ((f, i, j), (ft, it, jt)) = (cell_at(r.cell), cell_at(r.to));
            (f, i, j, if ft == f { (jt - j).atan2(it - i) } else { 0.0 })
        })
        .collect();
    let mut out: Vec<OreBody> = Vec::new();
    let sea = b.radius - SEA_DEPTH;
    let fits = |out: &[OreBody], k: usize, face: usize, i: f64, j: f64, a: f64| -> Option<f64> {
        let kind = &MINERALS.kinds[k];
        if i < SEAM + a || j < SEAM + a || i > n - SEAM - a || j > n - SEAM - a || (face == sf && (i - si).hypot(j - sj) < SPAWN + a) {
            return None;
        }
        let ground = b.ground_r(face, i, j);
        if ground - sea < 2.0 || out.iter().any(|o| o.face as usize == face && (o.at.x - i).hypot(o.at.y - j) < o.a + a) {
            return None;
        }
        let d = b.dir(face, i, j);
        let rock = b.rock_at(face, i, j, d);
        if !kind.provinces.contains(&rock.province) || (!kind.biomes.is_empty() && !b.biome_at(face, i, j, d).is_some_and(|x| kind.biomes.contains(&x))) {
            return None;
        }
        Some(ground)
    };
    for (k, kind) in MINERALS.kinds.iter().enumerate() {
        // the mantle's kinds on a body with a mantle only
        let want = if kind.count == 0 || kind.deep() && b.mantle == 0 { 0 } else { ((kind.count as f64 * scale).round() as u32).max(1) };
        let mut rng = Rng(seed ^ 0x0e0e ^ (k as u64) << 40);
        let (a, w, h) = kind.shape.axes(kind.blocks as f64);
        let (mut placed, mut tries) = (0, 0);
        while placed < want && tries < want * 600 {
            tries += 1;
            // the first of a kind with a ring from the spawn in the ring, while it has tries
            let ring = kind.near.filter(|_| placed == 0 && tries < 400);
            let (face, i, j, angle) = match (kind.shape, ring) {
                (Shape::Gravel, _) => {
                    let pick: Vec<&(usize, f64, f64, f64)> = match ring {
                        Some([lo, hi]) => rivers.iter().filter(|r| r.0 == sf && (lo..hi).contains(&(r.1 - si).hypot(r.2 - sj))).collect(),
                        None => rivers.iter().collect(),
                    };
                    if pick.is_empty() {
                        tries += 400;
                        continue;
                    }
                    let r = pick[(rng.f() * pick.len() as f64) as usize % pick.len()];
                    (r.0, r.1, r.2, r.3)
                }
                (_, Some([lo, hi])) => {
                    let (dist, t) = (lo + rng.f() * (hi.min(n) - lo), rng.f() * std::f64::consts::TAU);
                    (sf, si + dist * t.cos(), sj + dist * t.sin(), rng.f() * std::f64::consts::PI)
                }
                _ => ((rng.f() * 6.0) as usize % 6, rng.f() * n, rng.f() * n, rng.f() * std::f64::consts::PI),
            };
            let Some(ground) = fits(&out, k, face, i, j, a) else { continue };
            let depth = kind.depth[0] + rng.f() * (kind.depth[1] - kind.depth[0]);
            let z = if kind.shape.follows_ground() { depth } else { b.layer_of(ground - depth) };
            if let Some(host) = kind.host {
                let rock = b.rock_at(face, i, j, b.dir(face, i, j));
                // (the crust's kinds by the crust's layers alone, as before the mantle's rocks)
                let here = if kind.deep() { b.rock(rock, b.layer_r(z) as f32, 0) } else { ROCKS.at(rock, b.layer_r(z) as f32, 0) };
                if here != host {
                    continue;
                }
            }
            // where the field at its middle lies in its window
            let (ci, cj) = (i.floor() as i32, j.floor() as i32);
            let inside = |w: Option<[f64; 2]>, v: Option<f32>| w.is_none_or(|w| v.is_some_and(|v| (w[0]..w[1]).contains(&(v as f64))));
            if kind.deep() && !(inside(kind.pressure, b.pressure(face as u8, ci, cj, z)) && inside(kind.heat, b.heat(face as u8, ci, cj, z))) {
                continue;
            }
            out.push(OreBody { kind: k as u16, face: face as u8, at: DVec3::new(i, j, z), a, b: w, h, angle });
            placed += 1;
            if let Some(g) = kind.gravel {
                // the stream bed below it, where a stream runs within 300 m: a few cells downstream of the nearest
                let near = rivers.iter().filter(|r| r.0 == face).min_by(|x, y| (x.1 - i).hypot(x.2 - j).total_cmp(&(y.1 - i).hypot(y.2 - j)));
                if let Some(&(f, ri, rj, ra)) = near.filter(|r| (r.1 - i).hypot(r.2 - j) < 300.0) {
                    let along = 12.0 + rng.f() * 30.0;
                    let (gi, gj) = (ri + ra.cos() * along, rj + ra.sin() * along);
                    let gk = &MINERALS.kinds[g];
                    let (ga, gw, gh) = gk.shape.axes(gk.blocks as f64);
                    if fits(&out, g, f, gi, gj, ga).is_some() {
                        out.push(OreBody { kind: g as u16, face: f as u8, at: DVec3::new(gi, gj, gk.depth[0]), a: ga, b: gw, h: gh, angle: ra });
                    }
                }
            }
            if kind.gossan > 0.0 {
                let r = kind.gossan * 0.5;
                out.push(OreBody { kind: MINERALS.gossan as u16, face: face as u8, at: DVec3::new(i, j, 0.5), a: r, b: r, h: 0.5, angle: 0.0 });
            }
        }
    }
    out
}

/// One sample lies on this many square metres of ground at the middle of a body's footprint, thinning to none at
/// its rim (ADR 0138; the developer's to tune).
pub const SAMPLE_EVERY_M2: f64 = 40.0;
/// Samples spread at least this far from a body's middle line, metres: a vein two metres thin still shows.
pub const SAMPLE_SPREAD_M: f64 = 6.0;

/// A prospecting strike's bands, weakest first: an element's nearest body within `range` gives the band its
/// distance falls in (`band`).
pub const BANDS: [&str; 4] = ["traces", "some", "strong", "rich"];
/// The most elements an answer names.
pub const PROSPECT_NAMES: usize = 3;

/// The band (index of `BANDS`) of a body `d` metres off for a tool of range `r`: rich under r/8, strong under
/// r/3, some under 2r/3, traces under r; None from r on.
pub fn band(d: f64, r: f64) -> Option<u8> {
    [r / 8.0, r / 3.0, r * 2.0 / 3.0, r].iter().position(|&e| d < e).map(|k| 3 - k as u8)
}
/// A mined body fades: with the share of its ore left under each of these its band drops one more (ADR 0138).
pub const FADE: [f64; 3] = [1.0 / 2.0, 1.0 / 8.0, 1.0 / 32.0];

/// Metres from point `p` to the surface of the ellipsoid of half axes `e` about the origin; 0 inside.
pub fn to_surface(p: [f64; 3], e: [f64; 3]) -> f64 {
    let p = p.map(f64::abs);
    if (0..3).map(|k| (p[k] / e[k]).powi(2)).sum::<f64>() <= 1.0 {
        return 0.0;
    }
    // the nearest point is x_k = e_k^2 p_k / (t + e_k^2) for the t >= 0 where it lies on the surface (the sum
    // falls with t; at t = |e p| it is at most 1): bisected
    let f = |t: f64| (0..3).map(|k| (e[k] * p[k] / (t + e[k] * e[k])).powi(2)).sum::<f64>();
    let (mut lo, mut hi) = (0.0, (0..3).map(|k| (e[k] * p[k]).powi(2)).sum::<f64>().sqrt());
    for _ in 0..64 {
        let mid = 0.5 * (lo + hi);
        if f(mid) > 1.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (0..3).map(|k| (p[k] - e[k] * e[k] * p[k] / (hi + e[k] * e[k])).powi(2)).sum::<f64>().sqrt()
}

/// What a prospecting strike on cell `at` of `w` with a tool of `range` metres hears: per element with ore left
/// in the ground (the ledger's count, when it is open) within range of the cell, its band, strongest first, at
/// most `PROSPECT_NAMES`; empty for none. A body fades by `FADE` with its ledger count over `open`, its blocks as
/// generated (the server's `ore_open`). No direction. shortcut: only the struck face's bodies; one across a
/// face's seam is not heard (bodies keep `SEAM` columns from it).
pub fn prospect(w: &crate::world::World, at: crate::world::Block, range: f64, open: &[i64]) -> Vec<(String, u8)> {
    let Some(b) = w.bodies.get(at.body as usize) else { return Vec::new() };
    let (i, j, z) = (at.pos.x, at.pos.y, at.pos.z);
    let mut near: Vec<(String, u8)> = Vec::new();
    for (k, o) in b.ores.iter().enumerate() {
        let kind = &MINERALS.kinds[o.kind as usize];
        let left = w.ore_left.as_ref().and_then(|l| l.get(k)).filter(|_| at.body == 0);
        let share = left.map_or(1.0, |&n| n as f64 / open.get(k).map_or(n, |&t| t).max(1) as f64);
        if kind.gives.is_empty() || share <= 0.0 || o.face != at.face || (i as f64 + 0.5 - o.at.x).hypot(j as f64 + 0.5 - o.at.y) > o.a.max(o.b) + range {
            continue;
        }
        let fade = FADE.iter().filter(|&&f| share < f).count() as u8;
        let Some(band) = band(o.distance(b, i, j, z), range).and_then(|n| n.checked_sub(fade)) else { continue };
        match near.iter_mut().find(|n| n.0 == kind.gives) {
            Some(n) => n.1 = n.1.max(band),
            None => near.push((kind.gives.clone(), band)),
        }
    }
    near.sort_by(|x, y| y.1.cmp(&x.1).then(x.0.cmp(&y.0)));
    near.truncate(PROSPECT_NAMES);
    near
}

/// What a body holds in the ground: blocks and the ingots they give.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Held {
    pub blocks: u32,
    pub ingots: u32,
}

/// Every body's cells as generated: the solid ground inside it that is its own (an earlier body wins a cell
/// two share). Exact; parallel over the bodies.
pub fn count(b: &Body) -> Vec<Held> {
    use rayon::prelude::*;
    (0..b.ores.len())
        .into_par_iter()
        .map(|k| {
            let o = &b.ores[k];
            let mut held = Held::default();
            for c in cells(b, k) {
                held.blocks += 1;
                held.ingots += o.grade(c.x, c.y);
            }
            held
        })
        .collect()
}

/// Every body's halo as generated (ADR 0095): blocks of each grade, outermost first, that are its own (the
/// cell's highest grade, the first body on a tie). Rock with a look: the ledger never counts it. Parallel.
pub fn count_halo(b: &Body) -> Vec<Vec<u32>> {
    use rayon::prelude::*;
    (0..b.ores.len())
        .into_par_iter()
        .map(|k| {
            let o = &b.ores[k];
            let kind = &MINERALS.kinds[o.kind as usize];
            let mut out = vec![0u32; kind.halo.len()];
            if kind.halo.is_empty() {
                return out;
            }
            let r = (o.a.max(o.b) + kind.halo_m * (1.0 + FRAY) + 1.0).ceil() as i32;
            let (ci, cj) = (o.at.x.floor() as i32, o.at.y.floor() as i32);
            for (i, j) in (cj - r..=cj + r).flat_map(|j| (ci - r..=ci + r).map(move |i| (i, j))) {
                // only the face's own columns: past its seam the cells are the next face's, which has no halo
                if !(0..b.n).contains(&i) || !(0..b.n).contains(&j) {
                    continue;
                }
                let Some((lo, hi)) = o.halo_span(i, j) else { continue };
                let ground = b.col_height(o.face, i, j);
                for z in lo.max(b.mantle)..hi {
                    // the rule of `World::generated_under`: rock under the soil band, no cave, no ore
                    if ground - b.layer_r(z as f64 + 0.5) < 4.0 || b.ore_at(o.face, i, j, z, ground).is_some() || b.col_caves(o.face, i, j).iter().any(|&(l, h)| (l..h).contains(&z)) {
                        continue;
                    }
                    if let Some((w, g)) = b.halo_at(o.face, i, j, z) {
                        out[g] += (w as usize == k) as u32;
                    }
                }
            }
            out
        })
        .collect()
}

/// The cells (i, j, layer) of body `k` as generated.
pub fn cells(b: &Body, k: usize) -> Vec<glam::IVec3> {
    let o = &b.ores[k];
    let mut out = Vec::new();
    for (i, j) in o.columns() {
        let ground = b.col_height(o.face, i, j);
        let Some((lo, hi)) = o.span(b, i, j, ground) else { continue };
        for z in lo..hi {
            if b.ore_at(o.face, i, j, z, ground) == Some(k as u32) {
                out.push(glam::IVec3::new(i, j, z));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An ore body is cut at the old worlds' floor on a body with a mantle as it was at layer 0 before (ADR 0152):
    /// the mantle holds no ore and the ore budget holds.
    #[test]
    fn an_ore_body_stops_at_the_old_floor_over_the_mantle() {
        let b = Body::new(DVec3::ZERO, 6144, 1).with_mantle();
        let k = MINERALS.kinds.iter().position(|k| !k.shape.follows_ground()).unwrap() as u16;
        let o = OreBody { kind: k, face: 2, at: DVec3::new(100.0, 100.0, b.mantle as f64 + 3.0), a: 5.0, b: 5.0, h: 20.0, angle: 0.0 };
        assert_eq!(o.span(&b, 100, 100, b.radius).map(|s| s.0), Some(b.mantle), "cut at the old floor, layer {}", b.mantle);
        assert!(o.halo_span(100, 100).is_none_or(|(lo, _)| lo < b.mantle), "(its halo's box reaches under, the loop cuts it)");
    }

    /// The file reads, every kind's ore is a block, and a body of each shape holds the blocks it names.
    #[test]
    fn the_minerals_read_and_their_shapes_hold_their_blocks() {
        let m = &*MINERALS;
        assert!(m.kinds.len() > 10 && m.kinds[m.gossan].name == "gossan");
        for k in &m.kinds[..m.gossan] {
            let (a, b, h) = k.shape.axes(k.blocks as f64);
            let v = 4.0 / 3.0 * std::f64::consts::PI * a * b * h;
            assert!((v / k.blocks as f64 - 1.0).abs() < 1e-6, "{}: {v} m3 for {}", k.name, k.blocks);
            assert!(a >= b && b >= 1.0 - 1e-9 && h > 0.0, "{}: axes {a} {b} {h}", k.name);
        }
        // a checkout on Windows may give the file CRLF line ends
        let bad = include_str!("../../../data/minerals.toml").replace("\r\n", "\n").replace("provinces = [\"basin\"]\nhost = \"shale\"", "provinces = [\"moon\"]\nhost = \"shale\"");
        assert!(Minerals::parse(&bad).unwrap_err().contains("no province 'moon'"));
        let bad = include_str!("../../../data/minerals.toml").replace("gravel = \"tin gravel\"", "gravel = \"tin vein\"");
        assert!(Minerals::parse(&bad).unwrap_err().contains("no gravel kind"));
        let bad = include_str!("../../../data/minerals.toml").replace("\r\n", "\n").replace("halo_m = 3.0\n", "halo_m = 30.0\n");
        assert!(Minerals::parse(&bad).unwrap_err().contains("a halo is for"));
        let bad = include_str!("../../../data/minerals.toml").replace("\"greisen\"", "\"greisen stone\"");
        assert!(Minerals::parse(&bad).unwrap_err().contains("no solid block 'greisen stone'"));
        // a window out of order or out of range, and a crust's kind after a deep one
        for (from, to, says) in [("heat = [0.0, 110.0]", "heat = [110.0, 0.0]", "a window"), ("pressure = [36.0, 56.0]", "pressure = [-5.0, 56.0]", "a window"), ("heat = [0.0, 110.0]", "heat = [0.0, 110.0]\n\n[[kind]]\nname = \"late\"\nblock = \"coal\"\ngives = \"coal\"\nshape = \"seam\"\ncount = 1\nblocks = 10\ndepth = [5.0, 9.0]\nprovinces = [\"basin\"]", "come last")] {
            let bad = include_str!("../../../data/minerals.toml").replace(from, to);
            assert!(Minerals::parse(&bad).unwrap_err().contains(says), "{from} -> {to}");
        }
        // the gossan's index is the crust's kinds' count, the deep kinds after it
        let m = &*MINERALS;
        assert!(m.kinds[..m.gossan].iter().all(|k| !k.deep()) && m.kinds[m.gossan + 1..].iter().all(Kind::deep) && m.kinds.iter().any(Kind::deep));
    }

    /// The distance to an ellipsoid's surface is the nearest of its points, a sphere's and a thin vein's alike.
    #[test]
    fn the_distance_to_a_body_is_to_its_nearest_point() {
        assert!((to_surface([8.0, 0.0, 0.0], [5.0; 3]) - 3.0).abs() < 1e-6);
        assert_eq!(to_surface([1.0, 2.0, -1.0], [5.0; 3]), 0.0);
        for (p, e) in [([20.0, 3.0, 0.0], [24.0, 1.0, 14.0]), ([-30.0, 2.0, 9.0], [24.0, 1.0, 14.0]), ([3.0, 4.0, 5.0], [8.0, 6.0, 1.5])] {
            let mut best = f64::MAX;
            for a in 0..=600 {
                for b in 0..=600 {
                    let (th, ph) = (a as f64 / 600.0 * std::f64::consts::PI, b as f64 / 600.0 * std::f64::consts::TAU);
                    let q = [e[0] * th.sin() * ph.cos(), e[1] * th.sin() * ph.sin(), e[2] * th.cos()];
                    best = best.min(((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt());
                }
            }
            let d = to_surface(p, e);
            assert!((d - best).abs() < 0.05, "{p:?} to {e:?}: {d}, sampled {best}");
        }
    }

    /// A prospecting strike (ADR 0138) hears each element's nearest body by band:
    /// rich under R/8, strong under R/3, some under 2R/3, traces under R, nothing beyond, the strongest first; a
    /// body the ledger has dug out says nothing.
    #[test]
    fn a_strike_hears_the_bands_by_distance() {
        use crate::world::{Block, World};
        assert_eq!([1.0, 4.0, 8.0, 13.0, 16.0].map(|d| band(d, 16.0)), [Some(3), Some(2), Some(1), Some(0), None]);
        let mut w = World::new(256, 32, 1);
        let b = &w.bodies[0];
        let (f, i, j) = b.locate(b.spawn);
        let (f, i, j) = (f as u8, i as i32, j as i32);
        let z = b.layer_of(b.col_height(f, i, j)) as i32 - 40;
        let kind = |n: &str| MINERALS.kinds.iter().position(|k| k.name == n).unwrap() as u16;
        let lead = OreBody { kind: kind("lead pocket"), face: f, at: DVec3::new(i as f64 + 0.5, j as f64 + 0.5, z as f64 + 0.5), a: 5.0, b: 5.0, h: 5.0, angle: 0.0 };
        // a tin vein standing 10 m beside the lead along i, a metre thin across i
        let tin = OreBody { kind: kind("tin vein"), face: f, at: DVec3::new(i as f64 + 10.5, j as f64 + 0.5, z as f64 + 0.5), a: 10.0, b: 1.0, h: 6.0, angle: std::f64::consts::FRAC_PI_2 };
        std::sync::Arc::get_mut(&mut w.bodies).unwrap()[0].set_ores(vec![lead, tin]);
        let hear = |w: &World, dz: i32| prospect(w, Block { body: 0, face: f, pos: glam::IVec3::new(i, j, z + dz) }, 16.0, &[100, 100]);
        let s = |v: &[(&str, u8)]| v.iter().map(|(n, b)| (n.to_string(), *b)).collect::<Vec<_>>();
        // the tin is 9 m off at the lead's middle (some), farther as the strike rises
        assert_eq!(hear(&w, 0), s(&[("lead", 3), ("tin", 1)]));
        assert_eq!(hear(&w, 6), s(&[("lead", 3), ("tin", 1)]));
        assert_eq!(hear(&w, 9)[0], ("lead".to_string(), 2));
        assert_eq!(hear(&w, 13)[0], ("lead".to_string(), 1));
        assert_eq!(hear(&w, 18)[0], ("lead".to_string(), 0));
        assert_eq!(hear(&w, 25), s(&[]));
        // the lead half left: unchanged; 90 % mined: two bands down at its heart, its rim silent
        w.ore_left = Some(vec![50, 100]);
        assert_eq!(hear(&w, 0), s(&[("lead", 3), ("tin", 1)]));
        w.ore_left = Some(vec![10, 100]);
        assert_eq!(hear(&w, 0), s(&[("lead", 1), ("tin", 1)]));
        assert_eq!(hear(&w, 13), s(&[("tin", 0)]));
        // under 1/32: three down, the heart rings traces; under 1/2 one down
        w.ore_left = Some(vec![3, 49]);
        assert_eq!(hear(&w, 0), s(&[("lead", 0), ("tin", 0)]));
        // the lead dug out: only the tin
        w.ore_left = Some(vec![0, 100]);
        assert_eq!(hear(&w, 0), s(&[("tin", 1)]));
    }
}
