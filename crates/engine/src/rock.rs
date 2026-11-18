//! The rock under the soil (W4, ADR 0063): each province's layers from `data/rocks.toml`, and the one rule
//! that says which rock lies at a radius in a column, which the mesher's tile and `World::generated_under`
//! both ask (`Rocks::at`). The provinces themselves are the bake's (`bake::rock`).
use crate::blocks::BLOCKS;
use serde::Deserialize;
use std::sync::LazyLock;

/// A column's rock: its province and the radius the first layer's top lies at there, folded.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rock {
    pub province: u8,
    pub top: f32,
}

impl Rock {
    /// No province (a planet without a map, or a map from before the rock): the biome's rock.
    pub const NONE: Rock = Rock { province: u8::MAX, top: 0.0 };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Caves {
    None,
    Limestone,
    Tubes,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProvince {
    name: String,
    layers: Vec<(String, f32)>,
    fold: [f32; 2],
    heat: f32,
    caves: Caves,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    datum: f32,
    mantle: String,
    province: Vec<RawProvince>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Province {
    pub name: String,
    /// Top down: (block, metres thick); the last is 0, to the bottom.
    pub layers: Vec<(u16, f32)>,
    /// How far the layers wave (m) and the waves' width (m).
    pub fold: [f32; 2],
    /// Degrees C warmer a metre deeper.
    pub heat: f32,
    pub caves: Caves,
}

pub struct Rocks {
    /// Metres over the sea the first layer's top lies, before folding.
    pub datum: f32,
    /// The rock under the crust of a body with a mantle (ADR 0152; `Body::rock`), in every province.
    pub mantle: u16,
    pub provinces: Vec<Province>,
}

impl Rocks {
    /// Reads and checks the file: the provinces are the bake's, by name and in its order; every layer names a
    /// solid block, has a thickness over 0 but the last, which has 0; fold and heat are in range.
    pub fn parse(text: &str) -> Result<Rocks, String> {
        let f: File = toml::from_str(text).map_err(|e| e.to_string())?;
        let names: Vec<&str> = f.province.iter().map(|p| p.name.as_str()).collect();
        if names != crate::bake::rock::NAMES {
            return Err(format!("the provinces must be {:?} in that order, not {names:?}", crate::bake::rock::NAMES));
        }
        let mut provinces = Vec::new();
        for p in &f.province {
            let mut layers = Vec::new();
            for (k, (name, m)) in p.layers.iter().enumerate() {
                let d = BLOCKS.placeable().find(|d| &d.name == name).ok_or(format!("province '{}': no block '{name}'", p.name))?;
                if !d.solid {
                    return Err(format!("province '{}': '{name}' is not solid", p.name));
                }
                let last = k + 1 == p.layers.len();
                if last != (*m == 0.0) || !(0.0..=200.0).contains(m) {
                    return Err(format!("province '{}': layer '{name}' is {m} m; every layer but the last is 0..200 m, the last 0 (to the bottom)", p.name));
                }
                layers.push((d.id, *m));
            }
            if layers.is_empty() {
                return Err(format!("province '{}' has no layers", p.name));
            }
            if !(0.0..=50.0).contains(&p.fold[0]) || !(20.0..=5000.0).contains(&p.fold[1]) || !(0.0..=1.0).contains(&p.heat) {
                return Err(format!("province '{}': fold 0..50 m over 20..5000 m, heat 0..1 C a metre", p.name));
            }
            provinces.push(Province { name: p.name.clone(), layers, fold: p.fold, heat: p.heat, caves: p.caves });
        }
        let mantle = BLOCKS.placeable().find(|d| d.name == f.mantle && d.solid).ok_or(format!("the mantle: no solid block '{}'", f.mantle))?.id;
        Ok(Rocks { datum: f.datum, mantle, provinces })
    }

    /// The rock at radius `r` in a column of rock `rock`; `fallback` (the biome's rock) when it has no province.
    pub fn at(&self, rock: Rock, r: f32, fallback: u16) -> u16 {
        let Some(p) = self.provinces.get(rock.province as usize) else { return fallback };
        let mut foot = rock.top;
        for &(b, m) in &p.layers {
            foot -= m;
            if m == 0.0 || r >= foot {
                return b;
            }
        }
        unreachable!("the last layer goes to the bottom")
    }

    /// The layer a rock block is in a province, top down: which layer a find came from.
    pub fn layer_of(&self, province: u8, block: u16) -> Option<usize> {
        self.provinces.get(province as usize)?.layers.iter().position(|l| l.0 == block)
    }
}

pub static ROCKS: LazyLock<Rocks> = LazyLock::new(|| Rocks::parse(include_str!("../../../data/rocks.toml")).unwrap_or_else(|e| panic!("data/rocks.toml: {e}")));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_data_file_loads_and_layers_go_top_down() {
        let r = &*ROCKS;
        let basin = Rock { province: crate::bake::rock::BASIN, top: 100.0 };
        let id = |n: &str| BLOCKS.id(n);
        assert_eq!(r.at(basin, 150.0, 0), id("sandstone"), "over the top: the first layer goes on up");
        assert_eq!(r.at(basin, 76.0, 0), id("sandstone"));
        assert_eq!(r.at(basin, 74.0, 0), id("shale"));
        assert_eq!(r.at(basin, 50.0, 0), id("limestone"));
        assert_eq!(r.at(basin, -500.0, 0), id("granite"), "the last layer to the bottom");
        assert_eq!(r.at(Rock::NONE, 50.0, 7), 7, "no province: the biome's rock");
        assert!(r.provinces.iter().all(|p| p.layers.iter().all(|l| BLOCKS.solid(l.0))));
    }

    #[test]
    fn a_broken_file_is_refused_and_says_why() {
        let good = include_str!("../../../data/rocks.toml");
        for (from, to, says) in [("\"granite\", 0.0", "\"granit\", 0.0", "no block 'granit'"), ("[\"gneiss\", 0.0]", "[\"gneiss\", 5.0]", "the last 0"), ("name = \"rift\"", "name = \"rifts\"", "in that order"), ("mantle = \"peridotite\"", "mantle = \"water\"", "the mantle")] {
            let e = Rocks::parse(&good.replacen(from, to, 1)).err().unwrap_or_else(|| panic!("{from} -> {to} loads"));
            assert!(e.contains(says), "{e}");
        }
    }
}
