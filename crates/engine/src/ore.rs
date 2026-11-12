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
    fn axes(self, blocks: f64) -> (f64, f64, f64) {
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
}

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
            if !(r.depth[0] >= 0.0 && r.depth[0] <= r.depth[1] && r.depth[1] <= 180.0) || r.near.is_some_and(|n| !(n[0] >= 0.0 && n[0] < n[1])) || r.blocks == 0 || r.gives.is_empty() {
                return Err(format!("kind '{}': depth 0..180 m in order, near in order, blocks over 0, gives an element", r.name));
            }
            if r.host.is_some() && r.shape.follows_ground() {
                return Err(format!("kind '{}': a host rock is for a body at a depth, not one under the ground", r.name));
            }
            let host = r.host.as_ref().map(|h| block(&r.name, h)).transpose()?;
            kinds.push(Kind { name: r.name.clone(), block: block(&r.name, &r.block)?, gives: r.gives.clone(), shape: r.shape, count: r.count, blocks: r.blocks, depth: r.depth, provinces, host, biomes, near: r.near, gravel, gossan: r.gossan });
        }
        if kinds.iter().map(|k| &k.name).collect::<std::collections::BTreeSet<_>>().len() != kinds.len() {
            return Err("two kinds share a name".into());
        }
        let gossan = kinds.len();
        kinds.push(Kind { name: "gossan".into(), block: BLOCKS.id("gossan"), gives: String::new(), shape: Shape::Blanket, count: 0, blocks: 1, depth: [0.5, 0.5], provinces: Vec::new(), host: None, biomes: Vec::new(), near: None, gravel: None, gossan: 0.0 });
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
        let (bot, top) = if MINERALS.kinds[self.kind as usize].shape.follows_ground() {
            (body.layer_of(ground - (self.at.z + hh)), body.layer_of(ground - (self.at.z - hh)))
        } else {
            (self.at.z - hh, self.at.z + hh)
        };
        let (lo, hi) = (((bot - 0.5).ceil() as i32).max(0), (top - 0.5).floor() as i32 + 1);
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
    let cell_at = |c: u32| ((c as usize / (m * m)), ((c as usize % m) as f64 + 0.5) * n / m as f64, (((c as usize / m) % m) as f64 + 0.5) * n / m as f64);
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
        let want = if kind.count == 0 { 0 } else { ((kind.count as f64 * scale).round() as u32).max(1) };
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
                if ROCKS.at(rock, b.layer_r(z) as f32, 0) != host {
                    continue;
                }
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
        let bad = include_str!("../../../data/minerals.toml").replace("provinces = [\"basin\"]\nhost = \"shale\"", "provinces = [\"moon\"]\nhost = \"shale\"");
        assert!(Minerals::parse(&bad).unwrap_err().contains("no province 'moon'"));
        let bad = include_str!("../../../data/minerals.toml").replace("gravel = \"tin gravel\"", "gravel = \"tin vein\"");
        assert!(Minerals::parse(&bad).unwrap_err().contains("no gravel kind"));
    }
}
