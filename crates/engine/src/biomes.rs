//! Terra's biomes and still plants as rows of data, from `data/biomes.toml` (ADR 0042, 0043): each biome's
//! ground (top, band, rock, patches, snow on the heights) and the plants that stand on it, generated as
//! plain blocks. The rows follow the bake's biomes (`bake::life::BIOMES`) in order.
use crate::bake::life;
use crate::blocks::BLOCKS;
use serde::Deserialize;
use std::sync::LazyLock;

/// No crown is wider than this many blocks from its trunk: a column looks this far for the plants over it,
/// and a plant stands this far inside its face (so none crosses a seam).
pub const REACH: i32 = 6;
/// One candidate spot for a plant every `SPACING` x `SPACING` columns, jittered inside them.
pub const SPACING: i32 = 4;
/// The tallest plant, blocks.
pub const TALLEST: i32 = 16;

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Shape {
    Cone,
    Round,
    Flat,
    Droop,
    Palm,
    Column,
    Bush,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPlant {
    name: String,
    shape: Shape,
    #[serde(default)]
    trunk: Option<String>,
    #[serde(default)]
    leaves: Option<String>,
    height: [i32; 2],
    radius: i32,
    #[serde(default)]
    fork: Option<f32>,
    #[serde(default)]
    limbs: Option<u8>,
    #[serde(default)]
    bare: Option<f32>,
    #[serde(default)]
    depth: Option<f32>,
    #[serde(default)]
    thick: bool,
    #[serde(default)]
    warm: Option<f32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBiome {
    name: String,
    top: String,
    band: String,
    rock: String,
    #[serde(default)]
    patches: Vec<(String, f32)>,
    #[serde(default)]
    snow_over: Option<f32>,
    #[serde(default)]
    plants: Vec<(String, f32)>,
    #[serde(default)]
    clutter: Vec<(String, f32)>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    plant: Vec<RawPlant>,
    biome: Vec<RawBiome>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Plant {
    pub name: String,
    pub shape: Shape,
    /// Block ids (0 where the shape has none).
    pub trunk: u16,
    pub leaves: u16,
    pub height: [i32; 2],
    pub radius: i32,
    /// Round, flat and droop: the share of its height where the trunk forks, and into how many limbs.
    pub fork: f32,
    pub limbs: u8,
    /// Cone: the share of its height that is bare trunk under the lowest tier.
    pub bare: f32,
    /// Round, flat and droop: how thick the crown is each way from its middle, blocks.
    pub depth: f32,
    /// Round, flat and droop: a large one stands on a trunk of 2 x 2.
    pub thick: bool,
    /// It grows only where the year's mean air is at least this, C.
    pub warm: Option<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Biome {
    pub name: String,
    /// Block ids: the top, the band under it, the rock below.
    pub ground: [u16; 3],
    /// (block, the share of the surface it covers), in order.
    pub patches: Vec<(u16, f32)>,
    pub snow_over: Option<f32>,
    /// (index into `Rows::plants`, its share of the candidate spots), in order.
    pub plants: Vec<(usize, f32)>,
    /// (index into `clutter::CLUTTER`, how many lie on 100 columns of its ground), in order (ADR 0082).
    pub clutter: Vec<(usize, f32)>,
}

pub struct Rows {
    pub plants: Vec<Plant>,
    pub biomes: Vec<Biome>,
}

impl Biome {
    /// The surface block, from `roll` (uniform in 0..1, the same over a patch) and the height over the sea.
    pub fn top(&self, roll: f32, over_sea: f32) -> u16 {
        if self.snow_over.is_some_and(|s| over_sea >= s) {
            return BLOCKS.id("snow");
        }
        let mut acc = 0.0;
        for &(b, share) in &self.patches {
            acc += share;
            if roll < acc {
                return b;
            }
        }
        self.ground[0]
    }
    /// The plant at a candidate spot, from `roll` (uniform in 0..1).
    pub fn plant(&self, roll: f32) -> Option<usize> {
        let mut acc = 0.0;
        for &(p, share) in &self.plants {
            acc += share;
            if roll < acc {
                return Some(p);
            }
        }
        None
    }
}

impl Plant {
    /// Small, medium or large (0, 1, 2): where height `h` falls in the row's range (ADR 0092).
    pub fn size(&self, h: i32) -> i32 {
        ((h - self.height[0]) * 3 / (self.height[1] - self.height[0] + 1)).clamp(0, 2)
    }
    /// The crown's radius at height `h`: 2, 1 or 0 blocks narrower than `radius` for a small, medium or large
    /// tree, at least 1.
    pub fn crown(&self, h: i32) -> i32 {
        (self.radius + self.size(h) - 2).max(1).min(self.radius.max(1))
    }

    /// The block of a plant `h` blocks tall at (dx, dz) columns from its trunk and `z` layers over its foot (0 the
    /// first layer of air), and whether it is wood (wood wins over leaves where two plants meet). `v` is the
    /// tree's own hash (`world::Foot::v`): where its limbs go and which leaves its crown lets fall (ADR 0092).
    pub fn block(&self, h: i32, v: u32, dx: i32, dz: i32, z: i32) -> Option<(u16, bool)> {
        let r = self.radius;
        if z < 0 || z > h || dx.abs() > r || dz.abs() > r || (dx * dx + dz * dz) as f32 > (r * r) as f32 + 0.5 {
            return None;
        }
        let centre = dx == 0 && dz == 0;
        let wood = |yes: bool| (yes && self.trunk != 0).then_some((self.trunk, true));
        let leaf = |yes: bool| (yes && self.leaves != 0).then_some((self.leaves, false));
        match self.shape {
            Shape::Column if self.leaves == 0 || r == 0 => wood(centre && z < h),
            Shape::Column => self.stalks(h, v, dx, dz, z),
            // low and wide: one layer, the middle and most of its four sides, a corner only beside a side
            Shape::Bush => {
                let side = |x: i32, y: i32| (x == 0) != (y == 0) && hash(v, x, y, 6) % 4 != 0;
                let corner = dx != 0 && dz != 0 && hash(v, dx, dz, 6) % 2 == 0 && (side(dx, 0) || side(0, dz));
                leaf(z == 0 && (centre || side(dx, dz) || corner))
            }
            Shape::Cone => self.tiers(h, v, dx, dz, z),
            Shape::Palm => {
                if centre {
                    return wood(z < h).or_else(|| leaf(z == h));
                }
                // four fronds along the axes that bow down at their tips, and short ones between them in most
                // of the four quarters, by the hash
                let (ax, az, cr) = (dx.abs(), dz.abs(), self.crown(h));
                let (d, axis) = (ax.max(az), ax == 0 || az == 0);
                let quarter = (dx > 0) as i32 * 2 + (dz > 0) as i32;
                let frond = if axis {
                    (z == h && d <= cr - 1) || (z == h - 1 && d >= cr - 1 && d <= cr)
                } else {
                    z == h && d <= cr - 1 && (ax == az || ax == az + 1) && hash(v, quarter, 0, 7) % 3 != 0
                };
                leaf(frond)
            }
            Shape::Round | Shape::Flat | Shape::Droop => self.crowned(h, v, dx, dz, z),
        }
    }

    /// A tree that forks (round, flat, droop): a trunk (2 x 2 on a large `thick` one) to `fork` of its height
    /// that goes on up the middle; `limbs` limbs that spread from there out to two thirds of the crown's radius,
    /// under its leaves where they show, each then (on a crown of three or more blocks) parting into three
    /// branches that reach its rim inside the leaves. The leaves are a lens `depth` thick each way, wider than
    /// tall (flat: a pad), its outer leaves falling by the hash but never one whose way in is open. A willow
    /// (droop) is an umbrella whose rim sags, a withe hanging from each limb's middle branch and strands of leaves one
    /// block wide, of uneven lengths, hanging beside the withes from its rim to a block or two over the ground.
    fn crowned(&self, h: i32, v: u32, dx: i32, dz: i32, z: i32) -> Option<(u16, bool)> {
        // a crown is wider than tall at every size: a smaller one is thinner too
        let cr = self.crown(h) as f32;
        let rim = cr + 0.5;
        let cv = (0.7 * rim).max(1.5).min(self.depth);
        let d2 = (dx * dx + dz * dz) as f32;
        // nothing of the tree lies outside its rim: the limbs end inside it (cheap first, for the chunk's sake)
        if d2 > rim * rim {
            return None;
        }
        let droop = self.shape == Shape::Droop;
        let bend = if droop { 1.2 } else { 0.0 };
        // the lens's middle over a column d2 (squared) out from the trunk
        let mid = |d2: f32| h as f32 - cv - bend * d2 / (rim * rim);
        let layer = |z: f32| (z.round() as i32).clamp(1, h - 1);
        // where the limbs part into branches: just under the lens
        let zb = layer(mid(0.0) - cv);
        let fork = ((self.fork * h as f32).round() as i32).min(zb - 1).max(1);
        let large = self.size(h) == 2 && self.thick;
        // the trunk, and its leader up into the lens to two under its top
        if (dx, dz) == (0, 0) && z <= layer(mid(0.0)).max(h - 2) {
            return Some((self.trunk, true));
        }
        if large && (0..=1).contains(&dx) && (0..=1).contains(&dz) && z < fork {
            return Some((self.trunk, true));
        }
        // the limbs and branches as segments, and where each ends: directions from a table of sixteen, not sin
        // and cos, whose last bit a Linux server and a Windows client may round apart
        let (n, turn) = (self.limbs as i32, (hash(v, 0, 0, 2) % 16) as i32);
        let at = |d: i32, r: f32, z: i32| [(r * COS16[d.rem_euclid(16) as usize]).round(), (r * COS16[(d + 12).rem_euclid(16) as usize]).round(), z as f32];
        let reach = (cr - 1.0).max(0.0);
        let end_z = layer(mid(reach * reach));
        // on the stack: limbs are at most 6 (`parse`), so 24 segments; a willow's withes hang from each limb's
        // middle branch, six at most
        let (mut segs, mut ns, mut ends, mut ne) = ([([0f32; 3], [0f32; 3]); 24], 0, [[0f32; 3]; 6], 0);
        for k in 0..n {
            let d = turn + k * 16 / n + (hash(v, k, 0, 3) % 3) as i32 - 1;
            let base = [0.0, 0.0, fork as f32];
            if cr >= 3.0 {
                let m = at(d, 0.65 * cr, zb.min(end_z));
                (segs[ns], ns) = ((base, m), ns + 1);
                for j in [-2, 0, 2] {
                    let e = at(d + j, reach, end_z);
                    (segs[ns], ns) = ((m, e), ns + 1);
                    if j == 0 {
                        (ends[ne], ne) = (e, ne + 1);
                    }
                }
            } else {
                let e = at(d, reach, end_z);
                (segs[ns], ns, ends[ne], ne) = ((base, e), ns + 1, e, ne + 1);
            }
        }
        let (segs, ends) = (&segs[..ns], &ends[..ne]);
        let c = [dx as f32, dz as f32, z as f32];
        if segs.iter().any(|&(a, b)| on(a, b, c)) || (droop && ends.iter().any(|e| (e[0], e[1]) == (c[0], c[1]) && z >= self.withe(v, e) && (z as f32) < e[2])) {
            return Some((self.trunk, true));
        }
        // how deep in the lens a cell is (0 its middle, 1 its skin), and whether it holds a leaf
        let depth = |x: i32, y: i32, zz: i32| {
            let q = (x * x + y * y) as f32;
            q / (rim * rim) + ((zz as f32 - mid(q)) / cv).powi(2)
        };
        let lens = |x: i32, y: i32, zz: i32| {
            let e = depth(x, y, zz);
            if e > 1.0 {
                return false;
            }
            if e < 0.6 {
                return true;
            }
            let (ix, iy) = if x.abs() >= y.abs() { (x - x.signum(), y) } else { (x, y - y.signum()) };
            let iz = zz - (zz as f32 - mid((x * x + y * y) as f32)).signum() as i32;
            // a small crown keeps its skin whole: ragged, it would be a few leaves
            cr < 4.0 || (hash(v, x, y, zz) % 256 >= 80 && (depth(ix, iy, zz) < 0.6 || depth(x, y, iz) < 0.6))
        };
        if lens(dx, dz, z) {
            return Some((self.leaves, false));
        }
        // a willow's strand: on every other column each way of its rim beside a withe (a block's gap between
        // strands seen from any side), most of them, from under the
        // umbrella down to a block or two over the ground, one in four stopping half way
        if droop && d2 >= (cr - 1.0) * (cr - 1.0) && dx.rem_euclid(2) == 0 && dz.rem_euclid(2) == 0 {
            let k = hash(v, dx, dz, 1);
            let withe = ends.iter().any(|e| (e[0] - c[0]).abs() <= 2.0 && (e[1] - c[1]).abs() <= 2.0);
            let low = if k % 4 == 0 { h / 2 } else { 1 + (k >> 2) as i32 % 2 };
            if withe && k % 5 != 0 && z >= low && (z as f32) < mid(d2) {
                return Some((self.leaves, false));
            }
        }
        None
    }

    /// The lowest layer of a willow's withe from limb end `e`: one or two over the ground.
    fn withe(&self, v: u32, e: &[f32; 3]) -> i32 {
        1 + (hash(v, e[0] as i32, e[1] as i32, 4) % 2) as i32
    }

    /// A layered conifer (cone): a trunk to its top, bare under `bare` of its height, then tiers from the top
    /// down, wider as they go, a gap layer under each. A tier's lower layer carries its limbs along three or
    /// four of the axes, by the hash; leaves clump within a block of a limb or the trunk.
    fn tiers(&self, h: i32, v: u32, dx: i32, dz: i32, z: i32) -> Option<(u16, bool)> {
        let leaf = Some((self.leaves, false));
        if dx == 0 && dz == 0 {
            return if z < h { Some((self.trunk, true)) } else { leaf };
        }
        let base = ((self.bare * h as f32).round() as i32).max(2);
        let t = h - z;
        if z < base || t % 3 == 2 {
            return None;
        }
        let tier = t / 3;
        let cr = self.crown(h) as f32;
        let mut w = (cr * (t + 1) as f32 / (h - base + 1).max(1) as f32 + 0.7).min(cr);
        if t % 3 == 0 {
            w -= 1.0;
        }
        let d2 = (dx * dx + dz * dz) as f32;
        if d2 > w * w + 0.3 {
            return None;
        }
        let k = hash(v, tier, 0, 4);
        let skip = if k % 4 == 0 { 4 } else { (k >> 2) % 4 };
        let mut near = d2 <= 2.0;
        for (a, (ax, az)) in [(1, 0), (0, 1), (-1, 0), (0, -1)].into_iter().enumerate() {
            if a as u32 == skip {
                continue;
            }
            let (along, across) = (dx * ax + dz * az, (dx * az - dz * ax).abs());
            if along >= 1 && across == 0 && t % 3 == 1 && (along as f32) <= w - 1.0 {
                return Some((self.trunk, true));
            }
            near |= along >= 1 && across <= 1;
        }
        if near { leaf } else { None }
    }

    /// A clump of stalks (bamboo): the middle one and about a third of those around it, each 0..3 shorter than
    /// `h` and topped with a leaf, with leaves beside it on its upper three layers.
    fn stalks(&self, h: i32, v: u32, dx: i32, dz: i32, z: i32) -> Option<(u16, bool)> {
        let r = self.radius - 1;
        let stalk = |x: i32, y: i32| -> Option<i32> {
            let here = (x, y) == (0, 0) || (x.abs() <= r && y.abs() <= r && hash(v, x, y, 5) % 3 == 0);
            here.then(|| (h - (hash(v, x, y, 6) % 4) as i32).max(1))
        };
        if let Some(top) = stalk(dx, dz) {
            return if z < top { Some((self.trunk, true)) } else if z == top { Some((self.leaves, false)) } else { None };
        }
        let beside = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|(a, b)| stalk(dx + a, dz + b).is_some_and(|top| z >= top - 3 && z < top));
        (beside && hash(v, dx, dz, z) % 2 == 0).then_some((self.leaves, false))
    }
}

/// cos(d 22.5 degrees) for d in 0..16, to four places (sin is cos four steps on): the limbs' directions.
const COS16: [f32; 16] = [1.0, 0.9239, 0.7071, 0.3827, 0.0, -0.3827, -0.7071, -0.9239, -1.0, -0.9239, -0.7071, -0.3827, 0.0, 0.3827, 0.7071, 0.9239];

/// Whether the segment from `a` to `b` passes through cell `c` (its middle). Plain arithmetic only, the same on
/// every platform.
fn on(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> bool {
    // the segment's span inside the cell's cube, a hair larger so a segment through an edge or a corner takes
    // every cell there: the cells it passes are joined face to face (a limb steps out, then up)
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for i in 0..3 {
        let (min, max, d) = (c[i] - 0.501, c[i] + 0.501, b[i] - a[i]);
        if d == 0.0 {
            if a[i] < min || a[i] > max {
                return false;
            }
        } else {
            let (t0, t1) = ((min - a[i]) / d, (max - a[i]) / d);
            lo = lo.max(t0.min(t1));
            hi = hi.min(t0.max(t1));
        }
    }
    lo <= hi
}

/// A hash of a tree's own hash and a small cell, 24 bits.
fn hash(v: u32, a: i32, b: i32, c: i32) -> u32 {
    (crate::world::mix(((v as u64) << 32) | ((a & 0x3ff) as u64) << 20 | ((b & 0x3ff) as u64) << 10 | (c & 0x3ff) as u64) >> 40) as u32
}

/// Reads and checks the rows: plant names unique, heights 1..=TALLEST, radii 0..=REACH, every block exists,
/// each shape has the blocks it needs; one biome per bake biome with its name, shares in 0..1 adding up to at
/// most 1, every plant named exists.
pub fn parse(text: &str) -> Result<Rows, String> {
    let f: File = toml::from_str(text).map_err(|e| e.to_string())?;
    let block = |owner: &str, name: &Option<String>| -> Result<u16, String> {
        name.as_ref().map_or(Ok(0), |n| BLOCKS.placeable().find(|d| &d.name == n).map(|d| d.id).ok_or(format!("{owner}: no block '{n}'")))
    };
    let mut plants: Vec<Plant> = Vec::new();
    for p in f.plant {
        let who = format!("plant '{}'", p.name);
        if plants.iter().any(|q| q.name == p.name) {
            return Err(format!("{who}: named twice"));
        }
        if !(1 <= p.height[0] && p.height[0] <= p.height[1] && p.height[1] <= TALLEST) || !(0..=REACH).contains(&p.radius) {
            return Err(format!("{who}: height 1..={TALLEST}, radius 0..={REACH}"));
        }
        let (trunk, leaves) = (block(&who, &p.trunk)?, block(&who, &p.leaves)?);
        let needs = match p.shape {
            Shape::Column => trunk != 0,
            Shape::Bush => leaves != 0,
            _ => trunk != 0 && leaves != 0,
        };
        if !needs {
            return Err(format!("{who}: a {:?} needs its trunk and leaves", p.shape));
        }
        let (fork, limbs, bare, depth) = (p.fork.unwrap_or(0.5), p.limbs.unwrap_or(3), p.bare.unwrap_or(0.2), p.depth.unwrap_or(1.5));
        if !(0.1..=0.9).contains(&fork) || !(1..=6).contains(&limbs) || !(0.0..=0.8).contains(&bare) || !(0.5..=4.0).contains(&depth) || p.warm.is_some_and(|t| !(-30.0..=40.0).contains(&t)) {
            return Err(format!("{who}: fork 0.1..0.9, limbs 1..6, bare 0..0.8, depth 0.5..4, warm -30..40 C"));
        }
        let forks = matches!(p.shape, Shape::Round | Shape::Flat | Shape::Droop);
        if (!forks && (p.fork.is_some() || p.limbs.is_some() || p.depth.is_some() || p.thick)) || (p.shape != Shape::Cone && p.bare.is_some()) {
            return Err(format!("{who}: fork, limbs, depth and thick are for round, flat and droop, bare for cone"));
        }
        // a forked tree needs room under its crown (its layers clamp to 1..h-1), and a 2 x 2 trunk a radius of 2
        if forks && (p.height[0] < 3 || (p.thick && p.radius < 2)) {
            return Err(format!("{who}: a forked tree is 3 or more tall, and a thick one has a radius of 2 or more"));
        }
        // three sizes need three heights; a bush or a stalk has none
        let sized = p.radius > 0 && p.shape != Shape::Bush;
        if sized && p.height[1] - p.height[0] < 2 {
            return Err(format!("{who}: a tree's height spans at least three values, one for each size"));
        }
        plants.push(Plant { name: p.name, shape: p.shape, trunk, leaves, height: p.height, radius: p.radius, fork, limbs, bare, depth, thick: p.thick, warm: p.warm });
    }
    // Terra's biomes, then Luna's kinds of ground (`bake::luna`, ADR 0070)
    let names: Vec<&str> = life::BIOMES.iter().map(|b| b.0).chain(crate::bake::luna::KINDS).collect();
    if f.biome.len() != names.len() {
        return Err(format!("{} biomes, the bakes make {}", f.biome.len(), names.len()));
    }
    let mut biomes = Vec::new();
    for (b, want) in f.biome.into_iter().zip(names) {
        let who = format!("biome '{}'", b.name);
        if b.name != want {
            return Err(format!("{who}: the bake's biome here is '{want}'"));
        }
        let ground = [block(&who, &Some(b.top))?, block(&who, &Some(b.band))?, block(&who, &Some(b.rock))?];
        let shares = |v: &[(String, f32)]| v.iter().all(|s| (0.0..=1.0).contains(&s.1)) && v.iter().map(|s| s.1).sum::<f32>() <= 1.0;
        if !shares(&b.patches) || !shares(&b.plants) {
            return Err(format!("{who}: shares lie in 0..1 and add up to at most 1"));
        }
        let patches = b.patches.iter().map(|(n, s)| Ok((block(&who, &Some(n.clone()))?, *s))).collect::<Result<_, String>>()?;
        let plants = b.plants.iter().map(|(n, s)| Ok((plants.iter().position(|p| &p.name == n).ok_or(format!("{who}: no plant '{n}'"))?, *s))).collect::<Result<_, String>>()?;
        let rows = &*crate::clutter::CLUTTER;
        if b.clutter.iter().any(|c| !(0.0..=100.0).contains(&c.1)) || b.clutter.iter().map(|c| c.1).sum::<f32>() > 100.0 {
            return Err(format!("{who}: clutter lies on 0..100 of 100 columns, adding up to at most 100"));
        }
        let clutter = b.clutter.iter().map(|(n, k)| Ok((rows.iter().position(|r| &r.name == n).ok_or(format!("{who}: no clutter '{n}' in data/clutter.toml"))?, *k))).collect::<Result<_, String>>()?;
        biomes.push(Biome { name: b.name, ground, patches, snow_over: b.snow_over, plants, clutter });
    }
    Ok(Rows { plants, biomes })
}

pub static ROWS: LazyLock<Rows> = LazyLock::new(|| parse(include_str!("../../../data/biomes.toml")).unwrap_or_else(|e| panic!("data/biomes.toml: {e}")));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_data_file_has_every_biome_of_the_bake() {
        let r = &*ROWS;
        assert_eq!(r.biomes.len(), 15 + crate::bake::luna::KINDS.len(), "Terra's 15, then Luna's kinds");
        for (k, name) in crate::bake::luna::KINDS.iter().enumerate() {
            assert_eq!(&r.biomes[crate::bake::luna::HIGHLAND as usize + k].name, name);
        }
        // Luna's ice (the developer, ADR 0070): under 1 m of dust where the sun never reaches
        let shadow = &r.biomes[crate::bake::luna::SHADOW as usize].ground;
        assert_eq!(shadow[..2], [BLOCKS.id("regolith"), BLOCKS.id("ice")]);
        let b = |i: u8| &r.biomes[i as usize];
        assert_eq!(b(life::DESERT).ground, [BLOCKS.id("sand"), BLOCKS.id("sand"), BLOCKS.id("stone")]);
        assert_eq!(b(life::TEMPERATE_FOREST).ground[0], BLOCKS.id("grass"));
        assert!(b(life::ICE).plants.is_empty() && b(life::ALPINE).plants.is_empty(), "no plants on ice or the heights");
        assert_eq!(b(life::ALPINE).top(0.9, 120.0), BLOCKS.id("snow"), "snow on the peaks");
        assert_eq!(b(life::ALPINE).top(0.1, 60.0), BLOCKS.id("stone"), "a stone patch");
        assert_eq!(b(life::ALPINE).top(0.9, 60.0), BLOCKS.id("gravel"));
        let spruce = r.plants.iter().position(|p| p.name == "spruce").unwrap();
        assert_eq!(b(life::COLD_FOREST).plant(0.01), Some(spruce));
        assert_eq!(b(life::COLD_FOREST).plant(0.99), None);
    }

    /// Some trees' own hashes (`Foot::v`), spread over the bits.
    fn hashes() -> impl Iterator<Item = u32> {
        (0..256u64).map(|k| crate::world::mix(k * 0x9e37_79b9 + 1) as u32)
    }
    /// Every block of one tree: (dx, dz, z) -> (block, wood).
    fn tree(p: &Plant, h: i32, v: u32) -> std::collections::BTreeMap<(i32, i32, i32), (u16, bool)> {
        let mut out = std::collections::BTreeMap::new();
        for dx in -REACH - 1..=REACH + 1 {
            for dz in -REACH - 1..=REACH + 1 {
                for z in -1..=TALLEST + 1 {
                    if let Some(b) = p.block(h, v, dx, dz, z) {
                        out.insert((dx, dz, z), b);
                    }
                }
            }
        }
        out
    }

    /// ADR 0092: a tree is a pure function of its height and hash, inside its footprint (its radius and its
    /// height), on its foot; trees of one species differ by their hash.
    #[test]
    fn plants_keep_to_their_reach_and_stand_on_a_trunk() {
        for p in &ROWS.plants {
            let mut shapes = std::collections::BTreeSet::new();
            for h in p.height[0]..=p.height[1] {
                for v in hashes() {
                    let t = tree(p, h, v);
                    assert_eq!(t, tree(p, h, v), "{} is the same each time", p.name);
                    assert!(!t.is_empty(), "{} of {h}", p.name);
                    for (&(dx, dz, z), &(b, wood)) in &t {
                        assert!(dx.abs() <= p.radius && dz.abs() <= p.radius && (0..=h).contains(&z), "{} at {dx} {dz} {z}", p.name);
                        assert!(BLOCKS.solid(b));
                        assert_eq!(b, if wood { p.trunk } else { p.leaves }, "{}", p.name);
                    }
                    assert!(p.block(h, v, 0, 0, 0).is_some(), "{} stands on its foot", p.name);
                    shapes.insert(format!("{t:?}"));
                }
            }
            if p.radius > 0 && p.shape != Shape::Bush {
                assert!(shapes.len() > (p.height[1] - p.height[0] + 1) as usize, "{}: trees of one height differ by their hash", p.name);
            }
        }
    }

    /// The limbs' table is the rounded circle it stands for.
    #[test]
    fn the_limb_table_is_a_circle() {
        for (d, c) in COS16.iter().enumerate() {
            assert!((*c as f64 - (d as f64 * std::f64::consts::TAU / 16.0).cos()).abs() < 1e-4, "{d}");
        }
    }

    /// ADR 0092 (the overseer's winter stills: branch blocks that met only at corners floated off the trunk): a
    /// bare tree is one skeleton, every block of wood joined face to face to wood on the ground.
    #[test]
    fn the_wood_is_one_skeleton() {
        for p in ROWS.plants.iter().filter(|p| p.trunk != 0) {
            for h in p.height[0]..=p.height[1] {
                for v in hashes() {
                    let t = tree(p, h, v);
                    let wood: std::collections::BTreeSet<(i32, i32, i32)> = t.iter().filter(|b| b.1 .1).map(|b| *b.0).collect();
                    let mut seen: std::collections::BTreeSet<(i32, i32, i32)> = wood.iter().filter(|k| k.2 == 0).copied().collect();
                    let mut todo: Vec<(i32, i32, i32)> = seen.iter().copied().collect();
                    while let Some(k) = todo.pop() {
                        for n in [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)].map(|d| (k.0 + d.0, k.1 + d.1, k.2 + d.2)) {
                            if wood.contains(&n) && seen.insert(n) {
                                todo.push(n);
                            }
                        }
                    }
                    let loose: Vec<_> = wood.difference(&seen).collect();
                    assert!(loose.is_empty(), "{} of {h} ({v:x}): wood joined to nothing by a face at {loose:?}", p.name);
                }
            }
        }
    }

    /// ADR 0092: no leaf hangs alone: each has a leaf or wood of its tree beside it (one of six faces), or the
    /// ground under it (a shrub).
    #[test]
    fn every_leaf_touches_a_leaf_or_a_branch() {
        for p in &ROWS.plants {
            for h in p.height[0]..=p.height[1] {
                for v in hashes() {
                    let t = tree(p, h, v);
                    for (&(x, y, z), &(_, wood)) in &t {
                        let touches = [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)].iter().any(|d| t.contains_key(&(x + d.0, y + d.1, z + d.2)));
                        assert!(wood || touches || z == 0, "{} of {h} ({v:x}): a lone leaf at {x} {y} {z}", p.name);
                    }
                }
            }
        }
    }

    /// ADR 0092 (the developer: winter twigs "should look like they are coming from a log"): every leaf of a
    /// tree that loses its leaves lies within two blocks of its wood (on each axis), so a bare crown is a
    /// skeleton of trunk, limbs and branches, with no twig far from one.
    #[test]
    fn every_leaf_of_a_bare_crown_is_near_its_wood() {
        for n in ["broadleaf", "oak", "birch", "willow"] {
            let p = ROWS.plants.iter().find(|p| p.name == n).unwrap();
            assert!(BLOCKS.get(p.leaves).is_some_and(|d| d.seasons.is_some()), "{n} loses its leaves");
            for h in p.height[0]..=p.height[1] {
                for v in hashes() {
                    let t = tree(p, h, v);
                    let wood: Vec<(i32, i32, i32)> = t.iter().filter(|b| b.1 .1).map(|b| *b.0).collect();
                    for (&(x, y, z), _) in t.iter().filter(|b| !b.1 .1) {
                        let near = wood.iter().any(|w| (w.0 - x).abs() <= 2 && (w.1 - y).abs() <= 2 && (w.2 - z).abs() <= 2);
                        assert!(near, "{n} of {h} ({v:x}): the leaf at {x} {y} {z} is over two blocks from wood");
                    }
                }
            }
        }
    }

    /// ADR 0092 (the overseer's look at the first shots: a thin crown on a thick trunk reads as a post): a forked
    /// tree's crown is at least three times as wide as its trunk at every size, seven blocks or more on a large
    /// one, and over a block thick (but the savanna's flat top); a trunk is 2 x 2 only on a large tree.
    #[test]
    fn crowns_are_wide_for_their_trunks() {
        for p in ROWS.plants.iter().filter(|p| matches!(p.shape, Shape::Round | Shape::Flat | Shape::Droop)) {
            for h in p.height[0]..=p.height[1] {
                for v in hashes().take(64) {
                    let t = tree(p, h, v);
                    let trunk = t.iter().filter(|(k, b)| b.1 && k.2 == 0).count();
                    let large = p.size(h) == 2;
                    assert!(trunk == 1 || (trunk == 4 && large), "{} of {h}: a trunk of {trunk} blocks", p.name);
                    let leaves: Vec<(i32, i32, i32)> = t.iter().filter(|b| !b.1 .1).map(|b| *b.0).collect();
                    let span = |f: fn(&(i32, i32, i32)) -> i32| leaves.iter().map(f).max().unwrap() - leaves.iter().map(f).min().unwrap() + 1;
                    let wide = span(|k| k.0).min(span(|k| k.1));
                    let width = if trunk == 4 { 2 } else { 1 };
                    assert!(wide >= 3 * width && (!large || wide >= 7), "{} of {h} ({v:x}): a crown {wide} wide on a trunk {width} wide", p.name);
                    // not a sheet: most leaves have the crown (leaf or wood) over or under them
                    let thick = leaves.iter().filter(|k| t.contains_key(&(k.0, k.1, k.2 + 1)) || t.contains_key(&(k.0, k.1, k.2 - 1))).count();
                    assert!(p.shape == Shape::Flat || thick * 3 >= leaves.len() * 2, "{} of {h}: a crown over a block thick ({thick} of {} leaves)", p.name, leaves.len());
                    // a dome, not a ball on a pole: no taller than it is wide (a willow's strands aside)
                    let tall = span(|k| k.2);
                    assert!(p.shape == Shape::Droop || tall <= wide, "{} of {h} ({v:x}): a crown {tall} tall and {wide} wide", p.name);
                }
            }
        }
    }

    /// ADR 0092: trees fork into branches inside and under the crown, conifers stand in tiers with gaps, and a
    /// species comes in three sizes whose crowns widen with them.
    #[test]
    fn trees_branch_in_tiers_and_sizes() {
        let named = |n: &str| ROWS.plants.iter().find(|p| p.name == n).unwrap_or_else(|| panic!("no plant {n}"));
        for n in ["broadleaf", "oak", "birch", "jungle tree", "acacia", "willow", "spruce", "pine"] {
            let p = named(n);
            let branched = hashes().filter(|&v| tree(p, p.height[1], v).iter().any(|(&(x, y, _), &(_, wood))| wood && (x, y) != (0, 0))).count();
            assert!(branched * 4 >= hashes().count() * 3, "{n}: most large ones branch ({branched})");
            let wide = |h: i32| hashes().map(|v| tree(p, h, v).keys().map(|k| k.0.abs().max(k.1.abs())).max().unwrap()).max().unwrap();
            assert!(wide(p.height[0]) < wide(p.height[1]), "{n}: a small crown is narrower than a large one");
            assert_eq!(p.size(p.height[0]), 0);
            assert_eq!(p.size(p.height[1]), 2);
        }
        for n in ["spruce", "pine"] {
            let p = named(n);
            for v in hashes() {
                let t = tree(p, p.height[1], v);
                let leafy = |z: i32| t.iter().any(|(k, b)| k.2 == z && !b.1);
                let layers: Vec<i32> = (0..=p.height[1]).filter(|&z| leafy(z)).collect();
                let gaps = layers.windows(2).filter(|w| w[1] - w[0] > 1).count();
                assert!(gaps >= 2, "{n} ({v:x}): tiers with gaps, leaves on {layers:?}");
                assert!(layers[0] as f32 >= p.bare * p.height[1] as f32 - 1.0, "{n}: bare under {}", layers[0]);
            }
        }
        // a willow's rim hangs low, a palm's fronds spread from its top, bamboo stands in a clump of stalks
        let w = named("willow");
        let bare = hashes().find(|&v| !tree(w, w.height[1], v).iter().any(|(k, b)| !b.1 && k.2 <= w.height[1] / 2 && (k.0, k.1) != (0, 0)));
        assert!(bare.is_none(), "a willow hangs ({bare:x?})");
        let palm = named("palm");
        let t = tree(palm, palm.height[1], 1);
        assert!(t.iter().filter(|(k, b)| !b.1 && k.2 >= palm.height[1] - 1).count() >= 8 && t.iter().all(|(k, b)| b.1 || k.2 >= palm.height[1] - 1), "fronds only at the top");
        // a bush is low and wide: one layer, two or three blocks across (the overseer: not a pillar)
        let shrub = named("shrub");
        for v in hashes() {
            let t = tree(shrub, shrub.height[1], v);
            assert!(t.keys().all(|k| k.2 == 0) && t.len() >= 2 && t.keys().any(|k| k.0 != 0 || k.1 != 0), "a low, wide bush ({v:x}): {:?}", t.keys());
        }
        let bamboo = named("bamboo");
        assert!(hashes().any(|v| tree(bamboo, bamboo.height[1], v).iter().filter(|(k, b)| b.1 && k.2 == 0).count() >= 3), "a clump of stalks");
    }

    /// ADR 0092: the new species, each with its own log and leaves, in botanically sensible biomes.
    #[test]
    fn the_species_grow_in_their_biomes() {
        let r = &*ROWS;
        let grows = |biome: u8, plant: &str| r.biomes[biome as usize].plants.iter().any(|&(i, s)| r.plants[i].name == plant && s > 0.0);
        for (plant, log, leaves) in [("birch", "birch log", "birch leaves"), ("oak", "oak log", "oak leaves"), ("pine", "pine log", "pine leaves"), ("willow", "willow log", "willow leaves"), ("palm", "palm log", "palm leaves"), ("bamboo", "bamboo", "bamboo leaves")] {
            let p = r.plants.iter().find(|p| p.name == plant).unwrap_or_else(|| panic!("no plant {plant}"));
            assert_eq!((p.trunk, p.leaves), (BLOCKS.id(log), BLOCKS.id(leaves)), "{plant}");
        }
        for (biome, plant) in [(life::COLD_FOREST, "birch"), (life::COLD_FOREST, "pine"), (life::COLD_FOREST, "spruce"), (life::TEMPERATE_FOREST, "oak"), (life::TEMPERATE_FOREST, "willow"), (life::WETLAND, "oak"), (life::WETLAND, "willow"), (life::BEACH, "palm"), (life::RAIN_FOREST, "palm"), (life::RAIN_FOREST, "bamboo"), (life::RAIN_FOREST, "jungle tree"), (life::SAVANNA, "acacia"), (life::TEMPERATE_FOREST, "broadleaf")] {
            assert!(grows(biome, plant), "{plant} in {}", r.biomes[biome as usize].name);
        }
        let palm = r.plants.iter().find(|p| p.name == "palm").unwrap();
        assert!(palm.warm.is_some_and(|t| t >= 15.0), "palms only where it is warm");
        assert!(!grows(life::COLD_FOREST, "palm") && !grows(life::RAIN_FOREST, "pine") && !grows(life::DESERT, "oak"));
    }

    #[test]
    fn bad_rows_are_refused() {
        // A checkout on Windows may give the file CRLF line ends; the rows below are written with LF.
        let good = &include_str!("../../../data/biomes.toml").replace("\r\n", "\n");
        assert!(parse(good).is_ok());
        for (from, to, why) in [
            ("height = [7, 14]", "height = [7, 40]", "too tall"),
            ("radius = 3\nbare = 0.15", "radius = 9\nbare = 0.15", "too wide"),
            ("leaves = \"spruce leaves\"", "leaves = \"cheese\"", "no such block"),
            ("[\"spruce\", 0.32]", "[\"spruce\", 0.99]", "shares over 1"),
            ("[\"spruce\", 0.32]", "[\"baobab\", 0.2]", "no such plant"),
            ("fork = 0.35", "fork = 1.5", "a fork over the top"),
            ("warm = 15.0", "warm = nan", "warm not a number"),
            ("bare = 0.55", "bare = 0.55\nfork = 0.5", "a fork on a cone"),
            ("height = [5, 10]\nradius = 6", "height = [8, 9]\nradius = 6", "too few heights for three sizes"),
            ("limbs = 4\ndepth = 2.5", "limbs = 4\ndepth = 9.0", "a crown too deep"),
            ("leaves = \"willow leaves\"
height = [6, 10]", "leaves = \"willow leaves\"
height = [1, 10]", "a forked tree too short to fork"),
            ("leaves = \"willow leaves\"
height = [6, 10]
radius = 5", "leaves = \"willow leaves\"
height = [6, 10]
radius = 1", "a 2 x 2 trunk wider than its radius"),
            ("bare = 0.55", "bare = 0.55\nthick = true", "a thick cone"),
            ("limbs = 4\ndepth = 2.5", "limbs = 9\ndepth = 2.5", "too many limbs"),
            ("name = \"tundra\"", "name = \"steppe\"", "not the bake's biome"),
            ("name = \"cactus\"\nshape = \"column\"\ntrunk = \"cactus\"", "name = \"cactus\"\nshape = \"column\"", "a column with no trunk"),
        ] {
            assert!(good.contains(from), "{why}: the test's text is gone");
            assert!(parse(&good.replacen(from, to, 1)).is_err(), "{why}");
        }
    }
}
