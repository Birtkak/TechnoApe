//! Terra's biomes and still plants as rows of data, from `data/biomes.toml` (ADR 0042, 0043): each biome's
//! ground (top, band, rock, patches, snow on the heights) and the plants that stand on it, generated as
//! plain blocks. The rows follow the bake's biomes (`bake::life::BIOMES`) in order.
use crate::bake::life;
use crate::blocks::BLOCKS;
use serde::Deserialize;
use std::sync::LazyLock;

/// No crown is wider than this many blocks from its trunk: a column looks this far for the plants over it,
/// and a plant stands this far inside its face (so none crosses a seam).
pub const REACH: i32 = 3;
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
    pub trunk: u8,
    pub leaves: u8,
    pub height: [i32; 2],
    pub radius: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Biome {
    pub name: String,
    /// Block ids: the top, the band under it, the rock below.
    pub ground: [u8; 3],
    /// (block, the share of the surface it covers), in order.
    pub patches: Vec<(u8, f32)>,
    pub snow_over: Option<f32>,
    /// (index into `Rows::plants`, its share of the candidate spots), in order.
    pub plants: Vec<(usize, f32)>,
}

pub struct Rows {
    pub plants: Vec<Plant>,
    pub biomes: Vec<Biome>,
}

impl Biome {
    /// The surface block, from `roll` (uniform in 0..1, the same over a patch) and the height over the sea.
    pub fn top(&self, roll: f32, over_sea: f32) -> u8 {
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
    /// The block of a plant `h` blocks tall at (dx, dz) columns from its trunk and `z` layers over its foot (0 the
    /// first layer of air), and whether it is wood (wood wins over leaves where two plants meet).
    pub fn block(&self, h: i32, dx: i32, dz: i32, z: i32) -> Option<(u8, bool)> {
        let d2 = (dx * dx + dz * dz) as f32;
        let centre = dx == 0 && dz == 0;
        let r = self.radius as f32;
        if z < 0 || z > h || d2 > r * r + 0.5 {
            return None;
        }
        let wood = |top: i32| (centre && z < top && self.trunk != 0).then_some((self.trunk, true));
        let leaf = |yes: bool| (yes && self.leaves != 0).then_some((self.leaves, false));
        match self.shape {
            Shape::Column => wood(h),
            Shape::Bush => leaf(centre && z < h),
            Shape::Cone => wood(h - 1).or_else(|| {
                let w = r * (h - z) as f32 / (h - 2).max(1) as f32;
                leaf(z >= 2 && d2 <= w * w + 0.3)
            }),
            Shape::Round => {
                let (zc, rv) = (h as f32 - 0.8 * r, (0.8 * r).max(1.0));
                wood(zc as i32 + 1).or_else(|| leaf(d2 / (r * r).max(1.0) + ((z as f32 - zc) / rv).powi(2) <= 1.0))
            }
            Shape::Flat => wood(h).or_else(|| {
                let w = if z == h { r } else if z == h - 1 { r - 1.0 } else { -1.0 };
                leaf(w >= 0.0 && d2 <= w * w + 0.5)
            }),
        }
    }
}

/// Reads and checks the rows: plant names unique, heights 1..=TALLEST, radii 0..=REACH, every block exists,
/// each shape has the blocks it needs; one biome per bake biome with its name, shares in 0..1 adding up to at
/// most 1, every plant named exists.
pub fn parse(text: &str) -> Result<Rows, String> {
    let f: File = toml::from_str(text).map_err(|e| e.to_string())?;
    let block = |owner: &str, name: &Option<String>| -> Result<u8, String> {
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
        plants.push(Plant { name: p.name, shape: p.shape, trunk, leaves, height: p.height, radius: p.radius });
    }
    if f.biome.len() != life::BIOMES.len() {
        return Err(format!("{} biomes, the bake makes {}", f.biome.len(), life::BIOMES.len()));
    }
    let mut biomes = Vec::new();
    for (b, want) in f.biome.into_iter().zip(life::BIOMES) {
        let who = format!("biome '{}'", b.name);
        if b.name != want.0 {
            return Err(format!("{who}: the bake's biome here is '{}'", want.0));
        }
        let ground = [block(&who, &Some(b.top))?, block(&who, &Some(b.band))?, block(&who, &Some(b.rock))?];
        let shares = |v: &[(String, f32)]| v.iter().all(|s| (0.0..=1.0).contains(&s.1)) && v.iter().map(|s| s.1).sum::<f32>() <= 1.0;
        if !shares(&b.patches) || !shares(&b.plants) {
            return Err(format!("{who}: shares lie in 0..1 and add up to at most 1"));
        }
        let patches = b.patches.iter().map(|(n, s)| Ok((block(&who, &Some(n.clone()))?, *s))).collect::<Result<_, String>>()?;
        let plants = b.plants.iter().map(|(n, s)| Ok((plants.iter().position(|p| &p.name == n).ok_or(format!("{who}: no plant '{n}'"))?, *s))).collect::<Result<_, String>>()?;
        biomes.push(Biome { name: b.name, ground, patches, snow_over: b.snow_over, plants });
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
        assert_eq!(r.biomes.len(), 15);
        let b = |i: u8| &r.biomes[i as usize];
        assert_eq!(b(life::DESERT).ground, [BLOCKS.id("sand"), BLOCKS.id("sand"), BLOCKS.id("stone")]);
        assert_eq!(b(life::TEMPERATE_FOREST).ground[0], BLOCKS.id("grass"));
        assert!(b(life::ICE).plants.is_empty() && b(life::ALPINE).plants.is_empty(), "no plants on ice or the heights");
        assert_eq!(b(life::ALPINE).top(0.9, 120.0), BLOCKS.id("snow"), "snow on the peaks");
        assert_eq!(b(life::ALPINE).top(0.1, 60.0), BLOCKS.id("stone"), "a stone patch");
        assert_eq!(b(life::ALPINE).top(0.9, 60.0), BLOCKS.id("gravel"));
        let spruce = r.plants.iter().position(|p| p.name == "spruce").unwrap();
        assert_eq!(b(life::COLD_FOREST).plant(0.1), Some(spruce));
        assert_eq!(b(life::COLD_FOREST).plant(0.99), None);
    }

    #[test]
    fn plants_keep_to_their_reach_and_stand_on_a_trunk() {
        for p in &ROWS.plants {
            for h in p.height[0]..=p.height[1] {
                let mut any = false;
                for dx in -REACH - 1..=REACH + 1 {
                    for dz in -REACH - 1..=REACH + 1 {
                        for z in -1..=TALLEST + 1 {
                            if let Some((b, _)) = p.block(h, dx, dz, z) {
                                any = true;
                                assert!(dx.abs() <= p.radius && dz.abs() <= p.radius && (0..=h).contains(&z), "{} at {dx} {dz} {z}", p.name);
                                assert!(BLOCKS.solid(b));
                            }
                        }
                    }
                }
                assert!(any, "{} of {h}", p.name);
                assert!(p.block(h, 0, 0, 0).is_some(), "{} stands on its foot", p.name);
            }
        }
    }

    #[test]
    fn bad_rows_are_refused() {
        let good = include_str!("../../../data/biomes.toml");
        assert!(parse(good).is_ok());
        for (from, to, why) in [
            ("height = [8, 12]", "height = [8, 40]", "too tall"),
            ("radius = 3\n\n[[plant]]\nname = \"broadleaf\"", "radius = 5\n\n[[plant]]\nname = \"broadleaf\"", "too wide"),
            ("leaves = \"spruce leaves\"", "leaves = \"cheese\"", "no such block"),
            ("[\"spruce\", 0.5]", "[\"spruce\", 0.99]", "shares over 1"),
            ("[\"spruce\", 0.5]", "[\"oak\", 0.5]", "no such plant"),
            ("name = \"tundra\"", "name = \"steppe\"", "not the bake's biome"),
            ("name = \"cactus\"\nshape = \"column\"\ntrunk = \"cactus\"", "name = \"cactus\"\nshape = \"column\"", "a column with no trunk"),
        ] {
            assert!(good.contains(from), "{why}: the test's text is gone");
            assert!(parse(&good.replacen(from, to, 1)).is_err(), "{why}");
        }
    }
}
