//! Saves (ADR 0016): only what differs from the seed. That is every edited cell plus every named player's
//! position, mode and inventory; the rest is generated again from the seed. Written whole to a temporary file
//! and renamed over the old save, so a crash mid-write keeps the old save.
use engine::blocks::BLOCKS;
use engine::liquid::ColWater;
use engine::world::Block;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// 2 added water (ADR 0020), 3 the world's time (ADR 0022); a version 1 save still loads, with no water
/// changed and empty buckets, and versions 1 and 2 start the sky's clock at 0. shortcut: a player saved on the
/// moon before version 3 is placed where the moon's grid used to be, in the planet's frame; worlds older than
/// the continents (ADR 0021) are not worth carrying over.
pub const VERSION: u32 = 3;

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

/// Version 2, read to carry an old save over.
#[derive(Deserialize)]
struct SaveV2 {
    version: u32,
    seed: i32,
    planet_n: i32,
    moon_n: i32,
    blocks: Vec<(u8, String)>,
    edits: Vec<(Block, u8)>,
    players: Vec<Saved>,
    water: Vec<ColWater>,
    made: i64,
    removed: i64,
    sea_extra: i64,
}

/// Version 1, read to carry an old save over.
#[derive(Deserialize)]
struct SaveV1 {
    version: u32,
    seed: i32,
    planet_n: i32,
    moon_n: i32,
    blocks: Vec<(u8, String)>,
    edits: Vec<(Block, u8)>,
    players: Vec<SavedV1>,
}

#[derive(Deserialize)]
struct SavedV1 {
    name: String,
    pos: [f64; 3],
    fwd: [f32; 3],
    creative: bool,
    inv: Vec<(u8, u32)>,
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
            _ => match postcard::from_bytes::<SaveV2>(b) {
                Ok(v) if v.version == 2 => Save {
                    version: VERSION,
                    seed: v.seed,
                    planet_n: v.planet_n,
                    moon_n: v.moon_n,
                    blocks: v.blocks,
                    edits: v.edits,
                    players: v.players,
                    water: v.water,
                    made: v.made,
                    removed: v.removed,
                    sea_extra: v.sea_extra,
                    tick: 0,
                },
                _ => match postcard::from_bytes::<SaveV1>(b) {
                Ok(v) if v.version == 1 => Save {
                    version: VERSION,
                    seed: v.seed,
                    planet_n: v.planet_n,
                    moon_n: v.moon_n,
                    blocks: v.blocks,
                    edits: v.edits,
                    players: v.players.into_iter().map(|p| Saved { name: p.name, pos: p.pos, fwd: p.fwd, creative: p.creative, inv: p.inv, water: 0 }).collect(),
                    water: Vec::new(),
                    made: 0,
                    removed: 0,
                    sea_extra: 0,
                    tick: 0,
                },
                _ => return Err(format!("not a save of version 1 to {VERSION}")),
                },
            },
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

    /// A save from before water loads: its edits and players as they were, no water changed, empty buckets.
    #[test]
    fn a_version_1_save_still_loads() {
        let b = Block { body: 0, face: 1, pos: IVec3::new(3, 4, 5) };
        let v1 = (1u32, 9i32, 256i32, 32i32, registry(), vec![(b, 0u8)], vec![("ape".to_string(), [1.0f64, 2.0, 3.0], [1.0f32, 0.0, 0.0], true, vec![(2u8, 7u32)])]);
        let s = Save::from_bytes(&postcard::to_allocvec(&v1).unwrap()).unwrap();
        assert_eq!((s.version, s.seed, s.edits, s.water.len(), s.made), (VERSION, 9, vec![(b, 0)], 0, 0));
        assert_eq!((s.players[0].inv.clone(), s.players[0].water, s.players[0].creative), (vec![(2, 7)], 0, true));
    }

    /// A block renamed in data (moon dust became regolith, W0) keeps its id: a save under the old name loads.
    #[test]
    fn a_save_with_a_renamed_block_loads() {
        let old: Vec<(u8, String)> = registry().into_iter().map(|(id, n)| (id, if n == "regolith" { "moon dust".into() } else { n })).collect();
        assert!(old.iter().any(|b| b.1 == "moon dust"));
        let v1 = (1u32, 9i32, 256i32, 32i32, old, Vec::<(Block, u8)>::new(), Vec::<(String, [f64; 3], [f32; 3], bool, Vec<(u8, u32)>)>::new());
        assert!(Save::from_bytes(&postcard::to_allocvec(&v1).unwrap()).is_ok());
        let bad = (1u32, 9i32, 256i32, 32i32, vec![(7u8, "cheese".to_string())], Vec::<(Block, u8)>::new(), Vec::<(String, [f64; 3], [f32; 3], bool, Vec<(u8, u32)>)>::new());
        assert!(Save::from_bytes(&postcard::to_allocvec(&bad).unwrap()).is_err(), "an unknown name is still refused");
    }
}
