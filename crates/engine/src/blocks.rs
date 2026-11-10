//! The block registry: every block's id, name and look, from `data/blocks.toml` (ADR 0013). The file is built
//! into the binary, so a client and a server from one build agree; `Welcome` carries `BLOCKS.hash` so builds
//! that do not agree refuse each other.
use serde::Deserialize;
use std::sync::LazyLock;

pub const AIR: u8 = 0;

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
/// The numbers are the shader's (`shader.wgsl`, `Look.info`).
pub enum Pattern {
    #[default]
    Plain = 0,
    Grass = 1,
    Stone = 2,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
struct Raw {
    id: u8,
    name: String,
    #[serde(default = "yes")]
    solid: bool,
    #[serde(default = "grey")]
    colour: [f32; 3],
    #[serde(default)]
    pattern: Pattern,
    side: Option<String>,
}
fn yes() -> bool {
    true
}
fn grey() -> [f32; 3] {
    [0.5, 0.5, 0.5]
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    block: Vec<Raw>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Def {
    pub id: u8,
    pub name: String,
    pub solid: bool,
    pub colour: [f32; 3],
    pub pattern: Pattern,
    /// The block whose look a grass block's sides take.
    pub side: u8,
}

pub struct Registry {
    defs: Vec<Option<Def>>,
    pub hash: u64,
}

impl Registry {
    /// Reads and checks a registry: 0 is a non-solid "air", ids and names are unique, every `side` exists,
    /// colours lie in 0..1.
    pub fn parse(text: &str) -> Result<Registry, String> {
        let f: File = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut defs: Vec<Option<Def>> = vec![None; 256];
        for r in &f.block {
            if defs[r.id as usize].is_some() {
                return Err(format!("block id {} is used twice", r.id));
            }
            if f.block.iter().filter(|o| o.name == r.name).count() > 1 {
                return Err(format!("block name '{}' is used twice", r.name));
            }
            if r.colour.iter().any(|c| !(0.0..=1.0).contains(c)) {
                return Err(format!("block '{}': colour outside 0..1", r.name));
            }
            let side = match &r.side {
                None => r.id,
                Some(s) => f.block.iter().find(|o| &o.name == s).ok_or(format!("block '{}': no block '{s}' for its side", r.name))?.id,
            };
            defs[r.id as usize] = Some(Def { id: r.id, name: r.name.clone(), solid: r.solid, colour: r.colour, pattern: r.pattern, side });
        }
        match &defs[0] {
            Some(d) if d.name == "air" && !d.solid => {}
            _ => return Err("block 0 must be a non-solid 'air'".into()),
        }
        Ok(Registry { defs, hash: crate::world::fnv(crate::world::FNV0, text.as_bytes()) })
    }

    pub fn get(&self, id: u8) -> Option<&Def> {
        self.defs[id as usize].as_ref()
    }

    /// The id of a block the code itself names (generation); a missing one is a broken data file.
    pub fn id(&self, name: &str) -> u8 {
        self.defs.iter().flatten().find(|d| d.name == name).unwrap_or_else(|| panic!("data/blocks.toml has no block '{name}'")).id
    }

    pub fn solid(&self, id: u8) -> bool {
        self.defs[id as usize].as_ref().is_some_and(|d| d.solid)
    }

    /// Every block but air, in id order.
    pub fn placeable(&self) -> impl Iterator<Item = &Def> {
        self.defs.iter().flatten().filter(|d| d.id != AIR)
    }
}

pub static BLOCKS: LazyLock<Registry> =
    LazyLock::new(|| Registry::parse(include_str!("../../../data/blocks.toml")).unwrap_or_else(|e| panic!("data/blocks.toml: {e}")));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_data_file_loads_and_names_what_generation_needs() {
        for n in ["air", "grass", "dirt", "stone"] {
            BLOCKS.id(n);
        }
        assert!(!BLOCKS.solid(AIR) && BLOCKS.solid(BLOCKS.id("stone")));
        assert_eq!(BLOCKS.get(BLOCKS.id("grass")).unwrap().side, BLOCKS.id("dirt"));
    }

    #[test]
    fn bad_files_are_refused() {
        let air = "[[block]]\nid = 0\nname = \"air\"\nsolid = false\n";
        assert!(Registry::parse(air).is_ok());
        for (bad, why) in [
            ("[[block]]\nid = 0\nname = \"rock\"\n", "0 is not air"),
            (&format!("{air}[[block]]\nid = 0\nname = \"x\"\n"), "id twice"),
            (&format!("{air}[[block]]\nid = 1\nname = \"air\"\n"), "name twice"),
            (&format!("{air}[[block]]\nid = 1\nname = \"x\"\nside = \"y\"\n"), "unknown side"),
            (&format!("{air}[[block]]\nid = 1\nname = \"x\"\ncolour = [2.0, 0, 0]\n"), "colour"),
            (&format!("{air}[[block]]\nid = 1\nname = \"x\"\nhardness = 3\n"), "unknown field"),
        ] {
            assert!(Registry::parse(bad).is_err(), "{why}");
        }
    }
}
