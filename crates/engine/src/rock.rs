//! The rock under the soil (W4, ADR 0063): each province's layers from `data/rocks.toml`, and the one rule
//! that says which rock lies at a radius in a column, which the mesher's tile and `World::generated_under`
//! both ask (`Rocks::at`). The provinces themselves are the bake's (`bake::rock`).
use crate::blocks::BLOCKS;
use serde::Deserialize;
use std::sync::LazyLock;

/// A column's rock: its province and the radius the first layer's top lies at there, folded; on a body with a
/// mantle also the year's mean air over it (C) and how far its mantle's boundaries wave there (m), for
/// `Body::rock` (0 on one without); where a lava flow of its province lies over it (`Flow`, generation 2 on),
/// the radius of the flow's foot, else 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rock {
    pub province: u8,
    pub top: f32,
    pub air: f32,
    pub wobble: f32,
    pub flow: f32,
}

impl Rock {
    /// No province (a planet without a map, or a map from before the rock): the biome's rock.
    pub const NONE: Rock = Rock { province: u8::MAX, top: 0.0, air: 0.0, wobble: 0.0, flow: 0.0 };
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFlow {
    rock: String,
    share: f32,
    thick: f32,
    width: f32,
}

/// Lava flows over a province's top layers (`data/rocks.toml`, a world of generation 2 on): `rock` from the
/// ground down `thick` metres over about `share` of its ground, in fields some `width` metres across.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flow {
    pub rock: u16,
    pub share: f32,
    pub thick: f32,
    pub width: f32,
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
    flow: Option<RawFlow>,
    body: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDeep {
    block: String,
    pressure: f32,
    heat: f32,
    slope: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMantle {
    wobble: f32,
    rock: Vec<RawDeep>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    datum: f32,
    mantle: RawMantle,
    province: Vec<RawProvince>,
}

/// A mantle rock (`data/rocks.toml`): it takes over, going down, where the column's pressure reaches `pressure`
/// MPa at `heat` C, `slope` MPa more a degree warmer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Deep {
    pub block: u16,
    pub pressure: f32,
    pub heat: f32,
    pub slope: f32,
}

impl Deep {
    /// The pressure it takes over at in rock of `heat` C, MPa.
    pub fn at(&self, heat: f32) -> f32 {
        self.pressure + self.slope * (heat - self.heat)
    }
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
    pub flow: Option<Flow>,
    /// The row of `data/bodies.toml` whose ground it is (`body`, the home's when not given; draft-bake-stages).
    pub body: usize,
}

pub struct Rocks {
    /// Metres over the sea the first layer's top lies, before folding.
    pub datum: f32,
    /// The mantle's rocks top down, the first the crust's floor (`Body::rock`), in every province.
    pub mantle: Vec<Deep>,
    /// How far the mantle's boundaries wave, metres.
    pub wobble: f32,
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
            let flow = match &p.flow {
                None => None,
                Some(f) => {
                    let d = BLOCKS.placeable().find(|d| d.name == f.rock && d.solid).ok_or(format!("province '{}': the flow's rock: no solid block '{}'", p.name, f.rock))?;
                    if !(0.0..=1.0).contains(&f.share) || !(5.0..=100.0).contains(&f.thick) || !(50.0..=5000.0).contains(&f.width) {
                        return Err(format!("province '{}': a flow's share 0..1, thick 5..100 m, width 50..5000 m", p.name));
                    }
                    Some(Flow { rock: d.id, share: f.share, thick: f.thick, width: f.width })
                }
            };
            let body = match &p.body {
                None => 0,
                Some(b) => crate::bodies::BODIES.iter().position(|r| &r.name == b).ok_or(format!("province '{}': no body '{b}' in data/bodies.toml", p.name))?,
            };
            // a province's id (its place in NAMES) says whose map can hold it (`Map::rock`): Terra's to the ocean
            // floor, the home's moon's after; its `body` must agree, so neither's provinces lie on the other
            let id = provinces.len() as u8;
            let owner = if id <= crate::bake::rock::OCEAN { 0 } else { crate::bodies::moon(&crate::bodies::BODIES).expect("the home has a moon (bodies::parse)") };
            if body != owner {
                return Err(format!("province '{}' is {}'s (its place in the list): its body is '{}'", p.name, crate::bodies::BODIES[owner].name, crate::bodies::BODIES[owner].name));
            }
            provinces.push(Province { name: p.name.clone(), layers, fold: p.fold, heat: p.heat, caves: p.caves, flow, body });
        }
        let m = &f.mantle;
        let mut mantle: Vec<Deep> = Vec::new();
        for r in &m.rock {
            let block = BLOCKS.placeable().find(|d| d.name == r.block && d.solid).ok_or(format!("the mantle: no solid block '{}'", r.block))?.id;
            // a boundary moves less with heat than the pressure grows going down (0.05 MPa a metre in the crust,
            // at most 1 C a metre warmer), so a column crosses it once
            if !(r.pressure > 0.0 && r.pressure <= 1000.0) || !(-0.04..=0.04).contains(&r.slope) || !(-50.0..=2000.0).contains(&r.heat) || mantle.last().is_some_and(|l| l.pressure >= r.pressure) {
                return Err(format!("the mantle: '{}' takes over at {} MPa; pressures 0..1000 MPa rising row by row, slope -0.04..0.04 MPa a degree, heat -50..2000 C", r.block, r.pressure));
            }
            mantle.push(Deep { block, pressure: r.pressure, heat: r.heat, slope: r.slope });
        }
        if mantle.is_empty() || !(0.0..=20.0).contains(&m.wobble) {
            return Err("the mantle: at least one rock, wobble 0..20 m".into());
        }
        Ok(Rocks { datum: f.datum, mantle, wobble: m.wobble, provinces })
    }

    /// The mantle rock that has taken over at pressure `p` MPa in rock of `heat` C, the deepest first; None over
    /// the first (the crust).
    pub fn deep(&self, p: f32, heat: f32) -> Option<u16> {
        self.mantle.iter().rev().find(|d| p >= d.at(heat)).map(|d| d.block)
    }

    /// The rock at radius `r` in a column of rock `rock`; `fallback` (the biome's rock) when it has no province.
    pub fn at(&self, rock: Rock, r: f32, fallback: u16) -> u16 {
        let Some(p) = self.provinces.get(rock.province as usize) else { return fallback };
        if rock.flow > 0.0 && r >= rock.flow {
            if let Some(f) = &p.flow {
                return f.rock;
            }
        }
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
        let basin = Rock { province: crate::bake::rock::BASIN, top: 100.0, ..Rock::NONE };
        let id = |n: &str| BLOCKS.id(n);
        assert_eq!(r.at(basin, 150.0, 0), id("sandstone"), "over the top: the first layer goes on up");
        assert_eq!(r.at(basin, 76.0, 0), id("sandstone"));
        assert_eq!(r.at(basin, 74.0, 0), id("shale"));
        assert_eq!(r.at(basin, 50.0, 0), id("limestone"));
        assert_eq!(r.at(basin, -500.0, 0), id("granite"), "the last layer to the bottom");
        assert_eq!(r.at(Rock::NONE, 50.0, 7), 7, "no province: the biome's rock");
        assert!(r.provinces.iter().all(|p| p.layers.iter().all(|l| BLOCKS.solid(l.0))));
    }

    /// Luna's rows (draft-bake-stages): hers by the `body` key, Terra's the home's; her highland anorthosite to the
    /// bottom, her maria and her thorium province over anorthosite, every rock one a player can dig (a pick).
    #[test]
    fn lunas_provinces_are_hers_and_lie_on_anorthosite() {
        use crate::bake::rock::{LUNA_HIGHLAND, LUNA_KREEP, LUNA_MARE, OCEAN};
        let r = &*ROCKS;
        let luna = crate::bodies::find("luna").unwrap();
        for (k, p) in r.provinces.iter().enumerate() {
            assert_eq!(p.body, if k as u8 <= OCEAN { 0 } else { luna }, "{}", p.name);
        }
        let id = |n: &str| BLOCKS.id(n);
        let layers = |p: u8| r.provinces[p as usize].layers.iter().map(|l| l.0).collect::<Vec<_>>();
        assert_eq!(layers(LUNA_HIGHLAND), [id("anorthosite")]);
        assert_eq!(layers(LUNA_MARE), [id("mare basalt"), id("anorthosite")]);
        assert_eq!(layers(LUNA_KREEP), [id("kreep"), id("anorthosite")]);
        for p in [LUNA_HIGHLAND, LUNA_MARE, LUNA_KREEP] {
            let p = &r.provinces[p as usize];
            assert!(p.caves == Caves::None && p.flow.is_none(), "{}: caves and flows on Luna are the next step's", p.name);
            assert!(p.layers.iter().all(|l| BLOCKS.get(l.0).is_some_and(|d| d.tool.as_deref() == Some("pick"))), "{}", p.name);
        }
    }

    #[test]
    fn a_broken_file_is_refused_and_says_why() {
        let good = include_str!("../../../data/rocks.toml");
        for (from, to, says) in [("\"granite\", 0.0", "\"granit\", 0.0", "no block 'granit'"), ("[\"gneiss\", 0.0]", "[\"gneiss\", 5.0]", "the last 0"), ("name = \"rift\"", "name = \"rifts\"", "in that order"), ("block = \"peridotite\"", "block = \"water\"", "the mantle"), ("rock = \"basalt\", share", "rock = \"basalts\", share", "the flow's rock: no solid block 'basalts'"), ("share = 0.5,", "share = 1.4,", "a flow's share"), ("body = \"Luna\"", "body = \"Lune\"", "no body 'Lune'"), ("name = \"rift\"", "name = \"rift\"\nbody = \"Luna\"", "province 'rift' is Terra's")] {
            let e = Rocks::parse(&good.replacen(from, to, 1)).err().unwrap_or_else(|| panic!("{from} -> {to} loads"));
            assert!(e.contains(says), "{e}");
        }
    }

    /// The mantle's rows (ADR 0158): a boundary takes over at its pressure at its heat and
    /// moves by its slope with heat; a row out of order, or one whose slope would let a column cross it twice,
    /// is refused.
    #[test]
    fn the_mantle_rows_take_over_by_pressure_and_heat_and_bad_ones_are_refused() {
        let r = &*ROCKS;
        let id = |n: &str| BLOCKS.id(n);
        assert_eq!(r.mantle.iter().map(|d| d.block).collect::<Vec<_>>(), [id("peridotite"), id("ringwoodite"), id("bridgmanite")]);
        let ring = r.mantle[1];
        assert_eq!(r.deep(ring.pressure - 0.01, ring.heat), Some(id("peridotite")));
        assert_eq!(r.deep(ring.pressure, ring.heat), Some(id("ringwoodite")));
        // 100 C hotter: its slope's 3 MPa deeper
        assert_eq!(r.deep(ring.pressure + 0.99 * 100.0 * ring.slope, ring.heat + 100.0), Some(id("peridotite")));
        assert_eq!(r.deep(ring.pressure + 1.01 * 100.0 * ring.slope, ring.heat + 100.0), Some(id("ringwoodite")));
        assert_eq!(r.deep(1.0, 20.0), None, "the crust");
        assert_eq!(r.deep(500.0, 100.0), Some(id("bridgmanite")));
        let good = include_str!("../../../data/rocks.toml").replace("\r\n", "\n");
        for (from, to) in [("pressure = 33.0\n", "pressure = 3.0\n"), ("slope = 0.03\n", "slope = 0.5\n"), ("pressure = 56.7\n", "pressure = -1.0\n")] {
            let e = Rocks::parse(&good.replacen(from, to, 1)).err().unwrap_or_else(|| panic!("{from} -> {to} loads"));
            assert!(e.contains("the mantle"), "{e}");
        }
    }
}
