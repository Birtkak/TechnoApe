//! Saves (ADR 0016): only what differs from the seed. That is every edited cell plus every named player's
//! position, mode and inventory; the rest is generated again from the seed. Written whole to a temporary file
//! and renamed over the old save, so a crash mid-write keeps the old save.
use engine::blocks::BLOCKS;
use engine::liquid::ColWater;
use engine::world::Block;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// 2 added water (ADR 0020), 3 the world's time (ADR 0022), 4 is the world of 1 m blocks (ADR 0033). A save
/// of versions 1 to 3 holds a world of 0.5 m blocks, whose cells mean other places now: it is refused.
pub const VERSION: u32 = 4;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Save {
    pub version: u32,
    pub seed: i32,
    pub planet_n: i32,
    pub moon_n: i32,
    /// Every registered block as (id, name) when saved: ids must keep their names (ADR 0013).
    pub blocks: Vec<(u8, String)>,
    /// Every edited cell, sorted, so one state always gives the same file.
    pub edits: Vec<(Block, u8)>,
    /// Sorted by name.
    pub players: Vec<Saved>,
    /// Every stored column's water, sorted, and what creative put in and took out (ADR 0020).
    pub water: Vec<ColWater>,
    pub made: i64,
    pub removed: i64,
    /// What the open sea took in (`Liquids::sea_extra`).
    pub sea_extra: i64,
    /// The world's age in ticks: the sky's clock (ADR 0022).
    pub tick: u64,
}

/// A player the save remembers, found again by name on `Hello`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Saved {
    pub name: String,
    pub pos: [f64; 3],
    pub fwd: [f32; 3],
    pub creative: bool,
    /// Nonzero counts by block id.
    pub inv: Vec<(u8, u32)>,
    /// Units in the bucket.
    pub water: u32,
}

pub fn registry() -> Vec<(u8, String)> {
    BLOCKS.placeable().map(|d| (d.id, d.name.clone())).collect()
}

impl Save {
    pub fn to_bytes(&self) -> Vec<u8> {
        postcard::to_allocvec(self).expect("a save serialises")
    }

    /// Reads a save and checks it is this version and its blocks are this build's blocks.
    pub fn from_bytes(b: &[u8]) -> Result<Save, String> {
        let s: Save = match postcard::from_bytes::<Save>(b) {
            Ok(s) if s.version == VERSION => s,
            _ => return Err(format!("not a save of version {VERSION}: older ones are worlds of 0.5 m blocks (ADR 0033)")),
        };
        for (id, name) in &s.blocks {
            if !BLOCKS.get(*id).is_some_and(|d| &d.name == name || d.was.contains(name)) {
                return Err(format!("block {id} was '{name}' in the save; data/blocks.toml no longer has it so"));
            }
        }
        Ok(s)
    }

    pub fn write(&self, path: &Path) -> std::io::Result<()> {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, self.to_bytes())?;
        std::fs::rename(&tmp, path)
    }

    pub fn read(path: &Path) -> Result<Save, String> {
        Save::from_bytes(&std::fs::read(path).map_err(|e| e.to_string())?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::IVec3;

    fn save(version: u32, blocks: Vec<(u8, String)>) -> Save {
        let b = Block { body: 0, face: 1, pos: IVec3::new(3, 4, 5) };
        Save { version, seed: 9, planet_n: 256, moon_n: 32, blocks, edits: vec![(b, 0)], players: Vec::new(), water: Vec::new(), made: 0, removed: 0, sea_extra: 0, tick: 7 }
    }

    /// A save reads back as written; one from the world of 0.5 m blocks is refused, and says why.
    #[test]
    fn a_save_reads_back_and_an_older_world_is_refused() {
        let s = save(VERSION, registry());
        assert_eq!(Save::from_bytes(&s.to_bytes()).unwrap(), s);
        let old = Save::from_bytes(&save(3, registry()).to_bytes()).unwrap_err();
        assert!(old.contains("0.5 m"), "{old}");
    }

    /// A block renamed in data (moon dust became regolith, W0) keeps its id: a save under the old name loads.
    #[test]
    fn a_save_with_a_renamed_block_loads() {
        let old: Vec<(u8, String)> = registry().into_iter().map(|(id, n)| (id, if n == "regolith" { "moon dust".into() } else { n })).collect();
        assert!(old.iter().any(|b| b.1 == "moon dust"));
        assert!(Save::from_bytes(&save(VERSION, old).to_bytes()).is_ok());
        assert!(Save::from_bytes(&save(VERSION, vec![(7, "cheese".to_string())]).to_bytes()).is_err(), "an unknown name is still refused");
    }
}
