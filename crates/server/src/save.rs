//! Saves (ADR 0016): only what differs from the seed. That is every edited cell plus every named player's
//! position, mode and inventory; the rest is generated again from the seed. Written whole to a temporary file
//! and renamed over the old save, so a crash mid-write keeps the old save.
use engine::blocks::BLOCKS;
use engine::world::Block;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const VERSION: u32 = 1;

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
        let s: Save = postcard::from_bytes(b).map_err(|e| format!("not a save of version {VERSION}: {e}"))?;
        if s.version != VERSION {
            return Err(format!("save version {}, this build reads {VERSION}", s.version));
        }
        for (id, name) in &s.blocks {
            if BLOCKS.get(*id).map(|d| &d.name) != Some(name) {
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
