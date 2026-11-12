//! Structures (docs/design/studio-tools.md "Structures and flora", ADR 0052): `data/structures/<name>.toml`, a box
//! of whole 1 m world blocks named through a letter key, written as layers like a model's voxels, with an anchor
//! cell; or a `[grow]` recipe of numbers and a seed that grows a tree or a bush into such a box. No GPU: the
//! Studio, the client's stand runs and later the world generator call the same code.
use crate::blocks::{Registry, AIR};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A structure's box is 1..=MAX_SIDE blocks a side (a grown one is cut to it).
pub const MAX_SIDE: u32 = 64;
/// The letter that leaves the world's block as it is.
pub const KEEP: u8 = b'.';
/// The letter that clears the world's block to air.
pub const CLEAR: u8 = b'_';
/// The letters a key may use.
pub const LETTERS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

/// The file as written: either `size`, `anchor`, `layers` and `[key]`, or `[grow]` alone.
#[derive(Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    size: Option<[u32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    anchor: Option<[u32; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    layers: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    key: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    grow: Option<Recipe>,
}

/// The crown's shape: a ball, a cone (a spruce), or a flat lens (an acacia).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Crown {
    #[default]
    Round,
    Cone,
    Flat,
}

/// A tree or a bush in numbers (ADR 0052): the same recipe and seed grow the same blocks on every machine.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub seed: u64,
    /// The blocks it is made of, by their `blocks.toml` names.
    pub wood: String,
    pub leaves: String,
    /// Blocks, 1..=32.
    pub trunk_height: u32,
    /// Blocks a side, 1..=3.
    pub trunk_girth: u32,
    /// 0..=12.
    pub branches: u32,
    /// Degrees from straight up, 0..=90.
    pub branch_angle: f32,
    /// Blocks, 0..=12.
    pub branch_length: u32,
    pub crown: Crown,
    /// The crown's radius in blocks, 0..=12.
    pub crown_size: u32,
    /// The share of the crown's cells that hold leaves, 0..=1.
    pub leaf_density: f32,
}

/// Every number's range, for the checks and the Studio's sliders: (name, min, max, unit).
pub const RANGES: [(&str, f32, f32, &str); 7] = [
    ("trunk_height", 1.0, 32.0, "blocks"),
    ("trunk_girth", 1.0, 3.0, "blocks"),
    ("branches", 0.0, 12.0, ""),
    ("branch_angle", 0.0, 90.0, "degrees"),
    ("branch_length", 0.0, 12.0, "blocks"),
    ("crown_size", 0.0, 12.0, "blocks"),
    ("leaf_density", 0.0, 1.0, ""),
];

impl Recipe {
    pub fn get(&self, name: &str) -> f32 {
        match name {
            "trunk_height" => self.trunk_height as f32,
            "trunk_girth" => self.trunk_girth as f32,
            "branches" => self.branches as f32,
            "branch_angle" => self.branch_angle,
            "branch_length" => self.branch_length as f32,
            "crown_size" => self.crown_size as f32,
            "leaf_density" => self.leaf_density,
            _ => 0.0,
        }
    }
    /// Sets a number, rounded to a whole one where the field is whole.
    pub fn set(&mut self, name: &str, v: f32) {
        let u = v.round().max(0.0) as u32;
        match name {
            "trunk_height" => self.trunk_height = u,
            "trunk_girth" => self.trunk_girth = u,
            "branches" => self.branches = u,
            "branch_angle" => self.branch_angle = v,
            "branch_length" => self.branch_length = u,
            "crown_size" => self.crown_size = u,
            "leaf_density" => self.leaf_density = v,
            _ => {}
        }
    }
    fn check(&self, reg: &Registry) -> Result<(), String> {
        for (n, lo, hi, _) in RANGES {
            let v = self.get(n);
            if !(lo..=hi).contains(&v) {
                return Err(format!("grow: {n} = {v} is outside {lo}..{hi}"));
            }
        }
        for n in [&self.wood, &self.leaves] {
            block_id(reg, n).ok_or(format!("grow: no block '{n}' in data/blocks.toml"))?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Structure {
    /// x, y (up), z in blocks.
    pub size: [u32; 3],
    /// The cell set into the place it stands on (a tree's trunk foot: the first air over the ground).
    pub anchor: [u32; 3],
    /// Letter to block name.
    pub key: BTreeMap<String, String>,
    /// A letter per cell (KEEP, CLEAR or a key's), at x + size.x * (z + size.z * y).
    pub cells: Vec<u8>,
    /// The recipe a grown one comes from (it is written instead of the layers); none for one made by hand.
    pub grow: Option<Recipe>,
}

fn block_id(reg: &Registry, name: &str) -> Option<u16> {
    reg.placeable().find(|d| d.name == name).map(|d| d.id)
}

impl Structure {
    /// An empty box of KEEP cells.
    pub fn empty(size: [u32; 3]) -> Structure {
        Structure { size, anchor: [0; 3], key: BTreeMap::new(), cells: vec![KEEP; (size[0] * size[1] * size[2]) as usize], grow: None }
    }

    pub fn at(&self, x: u32, y: u32, z: u32) -> usize {
        (x + self.size[0] * (z + self.size[2] * y)) as usize
    }

    /// Reads and checks a structure; every refusal names `file` and the cell or field.
    pub fn parse(file: &str, text: &str, reg: &Registry) -> Result<Structure, String> {
        let at = |e: String| format!("{file}: {e}");
        let f: File = toml::from_str(text).map_err(|e| at(e.to_string()))?;
        if let Some(r) = f.grow {
            if f.size.is_some() || f.anchor.is_some() || f.layers.is_some() || f.key.is_some() {
                return Err(at("a grown structure has [grow] alone (freeze it to write layers)".into()));
            }
            r.check(reg).map_err(at)?;
            return Ok(grow(&r));
        }
        let size = f.size.ok_or(at("no size".into()))?;
        if size.iter().any(|v| !(1..=MAX_SIDE).contains(v)) {
            return Err(at(format!("size {size:?} outside 1..{MAX_SIDE}")));
        }
        let anchor = f.anchor.unwrap_or([0; 3]);
        if (0..3).any(|i| anchor[i] >= size[i]) {
            return Err(at(format!("anchor {anchor:?} is outside the box {size:?}")));
        }
        let key = f.key.unwrap_or_default();
        for (l, n) in &key {
            if l.len() != 1 || !LETTERS.contains(&l.as_bytes()[0]) {
                return Err(at(format!("key '{l}' is not one letter or digit")));
            }
            block_id(reg, n).ok_or(at(format!("key '{l}': no block '{n}' in data/blocks.toml")))?;
        }
        let cells = decode(size, f.layers.as_deref().unwrap_or(""), &key).map_err(at)?;
        Ok(Structure { size, anchor, key, cells, grow: None })
    }

    /// The file's text: `[grow]` alone for a grown one, else the layers (parse it again to check it).
    pub fn write(&self) -> String {
        let f = match &self.grow {
            Some(r) => File { grow: Some(r.clone()), ..File::default() },
            None => File { size: Some(self.size), anchor: Some(self.anchor), layers: Some(encode(self.size, &self.cells)), key: Some(self.key.clone()), grow: None },
        };
        toml::to_string(&f).expect("a structure always serialises")
    }

    /// A grown one as plain layers for hand work (the same blocks; the recipe is dropped).
    pub fn freeze(&mut self) {
        self.grow = None;
    }

    /// The letter of a block, added to the key when it has none yet; None when the key is full.
    pub fn letter(&mut self, name: &str) -> Option<u8> {
        if let Some((l, _)) = self.key.iter().find(|(_, n)| *n == name) {
            return Some(l.as_bytes()[0]);
        }
        let l = *LETTERS.iter().find(|l| !self.key.contains_key(&(**l as char).to_string()))?;
        self.key.insert((l as char).to_string(), name.to_string());
        Some(l)
    }

    /// The cells it sets, as offsets from the anchor (x, y up, z) and the block id (AIR for a cleared one).
    pub fn blocks(&self, reg: &Registry) -> Vec<([i32; 3], u16)> {
        let ids: BTreeMap<u8, u16> = self.key.iter().filter_map(|(l, n)| Some((l.as_bytes()[0], block_id(reg, n)?))).collect();
        let mut v = vec![];
        for y in 0..self.size[1] {
            for z in 0..self.size[2] {
                for x in 0..self.size[0] {
                    let id = match self.cells[self.at(x, y, z)] {
                        KEEP => continue,
                        CLEAR => AIR,
                        l => match ids.get(&l) {
                            Some(id) => *id,
                            None => continue,
                        },
                    };
                    let d = [x as i32 - self.anchor[0] as i32, y as i32 - self.anchor[1] as i32, z as i32 - self.anchor[2] as i32];
                    v.push((d, id));
                }
            }
        }
        v
    }
}

/// The layers: from the bottom up, a blank line between, one row per z, one letter per x.
fn decode(size: [u32; 3], text: &str, key: &BTreeMap<String, String>) -> Result<Vec<u8>, String> {
    let [sx, sy, sz] = size.map(|v| v as usize);
    let mut layers: Vec<Vec<&str>> = vec![Vec::new()];
    for line in text.lines().map(str::trim_end) {
        if line.is_empty() {
            if !layers.last().unwrap().is_empty() {
                layers.push(Vec::new());
            }
        } else {
            layers.last_mut().unwrap().push(line);
        }
    }
    if layers.last().is_some_and(|l| l.is_empty()) {
        layers.pop();
    }
    if layers.len() != sy {
        return Err(format!("{} layers, the box is {sy} high", layers.len()));
    }
    let mut cells = Vec::with_capacity(sx * sy * sz);
    for (y, rows) in layers.iter().enumerate() {
        if rows.len() != sz {
            return Err(format!("layer {} has {} rows, the box is {sz} deep", y + 1, rows.len()));
        }
        for (z, row) in rows.iter().enumerate() {
            if row.len() != sx {
                return Err(format!("layer {} row {} is {} long, the box is {sx} wide", y + 1, z + 1, row.len()));
            }
            for (x, ch) in row.bytes().enumerate() {
                if ch != KEEP && ch != CLEAR && !key.contains_key(&(ch as char).to_string()) {
                    return Err(format!("layer {} row {} column {}: '{}' is not in the key", y + 1, z + 1, x + 1, ch as char));
                }
                cells.push(ch);
            }
        }
    }
    Ok(cells)
}

/// `decode`'s inverse.
fn encode(size: [u32; 3], cells: &[u8]) -> String {
    let [sx, sy, sz] = size.map(|v| v as usize);
    let layers: Vec<String> = (0..sy).map(|y| (0..sz).map(|z| String::from_utf8_lossy(&cells[sx * (z + sz * y)..sx * (z + sz * y) + sx]).into_owned() + "\n").collect()).collect();
    layers.join("\n")
}

/// splitmix64: the same numbers on every machine.
fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}
/// 0..1 from a seed and a cell: a leaf's chance does not hang on the order cells are visited in.
fn chance(seed: u64, p: [i32; 3]) -> f32 {
    let k = (p[0] as u64 & 0xfffff) | (p[1] as u64 & 0xfffff) << 20 | (p[2] as u64 & 0xfffff) << 40;
    (mix(seed ^ mix(k)) >> 40) as f32 / (1u64 << 24) as f32
}

/// Grows a recipe into a structure (its `grow` is the recipe): the trunk, the branches with a tuft of leaves
/// each, the crown about the trunk's top. Cut to MAX_SIDE a side about the trunk's foot.
pub fn grow(r: &Recipe) -> Structure {
    let mut cells: BTreeMap<[i32; 3], u8> = BTreeMap::new();
    let (w, l) = (b'w', b'l');
    let g = r.trunk_girth.max(1) as i32;
    let h = r.trunk_height as i32;
    for y in 0..h {
        for x in 0..g {
            for z in 0..g {
                cells.insert([x, y, z], w);
            }
        }
    }
    let c = g as f32 / 2.0;
    let leaf_ball = |cells: &mut BTreeMap<[i32; 3], u8>, centre: [f32; 3], rx: f32, ry: f32, cone: bool| {
        let (rx, ry) = (rx.max(0.5), ry.max(0.5));
        for y in (centre[1] - ry).floor() as i32..=(centre[1] + ry).ceil() as i32 {
            for x in (centre[0] - rx).floor() as i32..=(centre[0] + rx).ceil() as i32 {
                for z in (centre[2] - rx).floor() as i32..=(centre[2] + rx).ceil() as i32 {
                    let d = [x as f32 + 0.5 - centre[0], y as f32 + 0.5 - centre[1], z as f32 + 0.5 - centre[2]];
                    let inside = if cone {
                        // from the foot (centre - ry) to the tip (centre + ry), narrowing
                        let k = 1.0 - (d[1] + ry) / (2.0 * ry);
                        (0.0..=1.0).contains(&k) && (d[0] * d[0] + d[2] * d[2]).sqrt() <= rx * k + 0.3
                    } else {
                        (d[0] / rx).powi(2) + (d[1] / ry).powi(2) + (d[2] / rx).powi(2) <= 1.0
                    };
                    if inside && chance(r.seed, [x, y, z]) < r.leaf_density {
                        cells.entry([x, y, z]).or_insert(l);
                    }
                }
            }
        }
    };
    // branches: up the trunk's upper half, spread round by the golden angle
    let mut rng = mix(r.seed);
    let mut next = || {
        rng = mix(rng);
        (rng >> 40) as f32 / (1u64 << 24) as f32
    };
    let mut tufts = vec![];
    for b in 0..r.branches {
        let y0 = h as f32 * (0.45 + 0.5 * next());
        let az = b as f32 * 2.399_963 + next() * 0.6;
        let el = (r.branch_angle + (next() - 0.5) * 20.0).clamp(0.0, 90.0).to_radians();
        let dir = [el.sin() * az.cos(), el.cos(), el.sin() * az.sin()];
        let len = r.branch_length as f32;
        let mut t = 0.0;
        while t <= len {
            let p = [c + dir[0] * t, y0 + dir[1] * t, c + dir[2] * t];
            cells.insert(p.map(|v| v.floor() as i32), w);
            t += 0.5;
        }
        tufts.push([c + dir[0] * len, y0 + dir[1] * len, c + dir[2] * len]);
    }
    let rs = r.crown_size as f32;
    if r.crown_size > 0 {
        for p in tufts {
            leaf_ball(&mut cells, p, (rs * 0.5).max(1.0), (rs * 0.4).max(1.0), false);
        }
        let top = h as f32;
        match r.crown {
            Crown::Round => leaf_ball(&mut cells, [c, top, c], rs, rs, false),
            Crown::Flat => leaf_ball(&mut cells, [c, top, c], rs, rs * 0.45, false),
            Crown::Cone => leaf_ball(&mut cells, [c, top + rs * 0.5, c], rs, rs * 1.5, true),
        }
    }
    // the box: the cells' bounds, cut to MAX_SIDE about the trunk's foot
    let half = MAX_SIDE as i32 / 2;
    let lo = |i: usize| cells.keys().map(|p| p[i]).min().unwrap_or(0).max(if i == 1 { 0 } else { -half });
    let hi = |i: usize| cells.keys().map(|p| p[i]).max().unwrap_or(0).min(if i == 1 { MAX_SIDE as i32 - 1 } else { half - 1 });
    let (min, max) = ([lo(0), lo(1), lo(2)], [hi(0), hi(1), hi(2)]);
    let size = [0, 1, 2].map(|i| (max[i] - min[i] + 1) as u32);
    let mut s = Structure::empty(size);
    s.key.insert("w".into(), r.wood.clone());
    s.key.insert("l".into(), r.leaves.clone());
    s.anchor = [(-min[0]) as u32, 0, (-min[2]) as u32];
    for (p, v) in cells {
        if (0..3).all(|i| (min[i]..=max[i]).contains(&p[i])) {
            let k = s.at((p[0] - min[0]) as u32, (p[1] - min[1]) as u32, (p[2] - min[2]) as u32);
            s.cells[k] = v;
        }
    }
    s.grow = Some(r.clone());
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::BLOCKS;

    const HUT: &str = "size = [3, 2, 2]
anchor = [1, 0, 0]
layers = \"\"\"
sss
s_s

.s.
...
\"\"\"

[key]
s = \"stone\"
";

    fn oak() -> Recipe {
        Recipe { seed: 7, wood: "log".into(), leaves: "broadleaf leaves".into(), trunk_height: 6, trunk_girth: 1, branches: 4, branch_angle: 50.0, branch_length: 3, crown: Crown::Round, crown_size: 3, leaf_density: 0.8 }
    }

    #[test]
    fn a_structure_reads_writes_byte_exact_and_places_from_its_anchor() {
        let s = Structure::parse("hut.toml", HUT, &BLOCKS).unwrap();
        assert_eq!((s.size, s.anchor), ([3, 2, 2], [1, 0, 0]));
        assert_eq!(s.write(), HUT);
        assert_eq!(Structure::parse("again", &s.write(), &BLOCKS).unwrap(), s);
        let b = s.blocks(&BLOCKS);
        // 5 stone and one cleared cell in the bottom layer, one stone on top; '.' sets nothing
        assert_eq!(b.len(), 7);
        let stone = BLOCKS.id("stone");
        assert!(b.contains(&([-1, 0, 0], stone)) && b.contains(&([0, 0, 1], AIR)) && b.contains(&([0, 1, 0], stone)));
    }

    #[test]
    fn bad_structures_are_refused_by_file_and_cell() {
        for (bad, why) in [
            (HUT.replace("\"stone\"", "\"gold\""), "key 's': no block 'gold'"),
            (HUT.replace("s_s\n", "sxs\n"), "hut.toml: layer 1 row 2 column 2: 'x' is not in the key"),
            (HUT.replace("[3, 2, 2]", "[65, 2, 2]"), "outside 1..64"),
            (HUT.replace("[1, 0, 0]", "[3, 0, 0]"), "anchor [3, 0, 0] is outside"),
            (HUT.replace("sss\n", "ssss\n"), "layer 1 row 1 is 4 long"),
            (HUT.replace("\n.s.\n...\n", ""), "1 layers, the box is 2 high"),
            (format!("{HUT}\n{}", grow(&oak()).write()), "[grow] alone"),
        ] {
            let e = Structure::parse("hut.toml", &bad, &BLOCKS).unwrap_err();
            assert!(e.starts_with("hut.toml") && e.contains(why), "{why}: got {e}");
        }
    }

    #[test]
    fn a_grown_tree_is_the_same_from_the_same_seed_and_stays_in_its_box() {
        let a = grow(&oak());
        assert_eq!(a, grow(&oak()));
        let b = grow(&Recipe { seed: 8, ..oak() });
        assert_ne!(a.cells, b.cells);
        // the trunk's foot is the anchor and it is wood
        assert_eq!(a.cells[a.at(a.anchor[0], 0, a.anchor[2])], b'w');
        assert!(a.cells.contains(&b'l'));
        // the largest numbers are cut to the cap
        let big = Recipe { trunk_height: 32, trunk_girth: 3, branches: 12, branch_angle: 90.0, branch_length: 12, crown: Crown::Cone, crown_size: 12, leaf_density: 1.0, ..oak() };
        for seed in 0..4 {
            let s = grow(&Recipe { seed, ..big.clone() });
            assert!(s.size.iter().all(|v| (1..=MAX_SIDE).contains(v)), "{:?}", s.size);
            assert_eq!(s.cells.len(), (s.size[0] * s.size[1] * s.size[2]) as usize);
        }
    }

    #[test]
    fn a_grown_file_round_trips_and_freezes_into_the_same_blocks() {
        let s = grow(&oak());
        let text = s.write();
        assert!(text.starts_with("[grow]") && !text.contains("layers"), "{text}");
        assert_eq!(Structure::parse("oak.toml", &text, &BLOCKS).unwrap(), s);
        let mut f = s.clone();
        f.freeze();
        let back = Structure::parse("oak.toml", &f.write(), &BLOCKS).unwrap();
        assert_eq!((back.cells, back.grow), (s.cells.clone(), None));
        let e = Structure::parse("oak.toml", &text.replace("trunk_height = 6", "trunk_height = 40"), &BLOCKS).unwrap_err();
        assert!(e.contains("oak.toml: grow: trunk_height = 40 is outside 1..32"), "{e}");
    }

    #[test]
    fn the_drafts_read_and_write_back_byte_exact() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/structures");
        let mut n = 0;
        for e in std::fs::read_dir(&dir).unwrap() {
            let p = e.unwrap().path();
            let text = std::fs::read_to_string(&p).unwrap().replace("\r\n", "\n");
            let s = Structure::parse(&p.display().to_string(), &text, &BLOCKS).unwrap();
            assert_eq!(s.write(), text, "{}", p.display());
            n += 1;
        }
        assert!(n >= 3);
    }
}
