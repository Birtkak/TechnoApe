//! Saves (ADR 0016): only what differs from the seed. That is every edited cell plus every named player's
//! position, mode and inventory; the rest is generated again from the seed. Written whole to a temporary file
//! and renamed over the old save, so a crash mid-write keeps the old save.
use engine::blocks::BLOCKS;
use engine::liquid::ColWater;
use engine::vitals::Vitals;
use engine::world::Block;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// 2 added water (ADR 0020), 3 the world's time (ADR 0022), 4 is the world of 1 m blocks (ADR 0033), 5 keeps
/// every player's health and hunger (ADR 0040), 6 was two things on two branches, 7 is both: the shared
/// coordinates of ADR 0041 (Luna's grid and the inertial frame moved out; `Server::from_save` moves the players
/// of a save older than 6) and the name of the baked map it was played on (ADR 0043). A 6 without a map has
/// the coordinates; a 6 that names a map is from before them, and is read as a 5 so its players are moved. A
/// save of versions 1 to 3 holds a world of 0.5 m blocks, whose cells mean other places now: it is refused.
/// One of version 4 loads, its players at full health and hunger; one from before the bake keeps its noise
/// land. 8 keeps the river reaches' open water (W5, ADR 0059); a 7 loads with none. A save keeps the version
/// it was read as until it is written again.
pub const VERSION: u32 = 8;

/// The rain brought into the reaches, and what each reach's open water took in (`Liquids::rained`,
/// `Liquids::reach_extra`).
pub type Reaches = (i64, Vec<(u32, i64)>);

/// A save as written; `Save<SavedV4, (), ()>` is how version 4 lies on disk, `Save<Saved, (), ()>` version 5
/// and `Save<Saved, Option<u64>, ()>` version 7 (postcard keeps no field names, so a player with one more field
/// is another layout; `()` is no bytes).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Save<P = Saved, M = Option<u64>, R = Reaches> {
    pub version: u32,
    pub seed: i32,
    pub planet_n: i32,
    pub moon_n: i32,
    /// Every registered block as (id, name) when saved: ids must keep their names (ADR 0013).
    pub blocks: Vec<(u8, String)>,
    /// Every edited cell, sorted, so one state always gives the same file.
    pub edits: Vec<(Block, u8)>,
    /// Sorted by name.
    pub players: Vec<P>,
    /// Every stored column's water, sorted, and what creative put in and took out (ADR 0020).
    pub water: Vec<ColWater>,
    pub made: i64,
    pub removed: i64,
    /// What the open sea took in (`Liquids::sea_extra`).
    pub sea_extra: i64,
    /// The world's age in ticks: the sky's clock (ADR 0022).
    pub tick: u64,
    /// The hash of the baked map the planet stands on (`engine::bake::Map`); None: the noise land of a save
    /// from before the bake.
    pub map: M,
    pub reaches: R,
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
    /// Health and hunger (ADR 0040), and how it died if it is dead.
    pub vitals: Vitals,
}

/// A player in a save of version 4: before health and hunger.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SavedV4 {
    pub name: String,
    pub pos: [f64; 3],
    pub fwd: [f32; 3],
    pub creative: bool,
    pub inv: Vec<(u8, u32)>,
    pub water: u32,
}

pub fn registry() -> Vec<(u8, String)> {
    BLOCKS.placeable().map(|d| (d.id, d.name.clone())).collect()
}

impl Save {
    pub fn to_bytes(&self) -> Vec<u8> {
        postcard::to_allocvec(self).expect("a save serialises")
    }

    /// Reads a save and checks it is this version (or 4 or 5, carried over) and its blocks are this build's blocks.
    pub fn from_bytes(b: &[u8]) -> Result<Save, String> {
        let now = postcard::from_bytes::<Save>(b).ok().filter(|s| s.version == VERSION);
        let mapped = || postcard::from_bytes::<Save<Saved, Option<u64>, ()>>(b).ok().filter(|s| s.version == 7 || s.version == 6);
        let bare = || postcard::from_bytes::<Save<Saved, (), ()>>(b).ok().filter(|s| s.version == 5 || s.version == 6);
        let s: Save = if let Some(s) = now {
            s
        } else if let Some(s) = mapped() {
            let Save { version, seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map, .. } = s;
            let version = if version == 6 { 5 } else { version };
            Save { version, seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map, reaches: Reaches::default() }
        } else if let Some(s) = bare() {
            let Save { version, seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, .. } = s;
            Save { version, seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map: None, reaches: Reaches::default() }
        } else {
            match postcard::from_bytes::<Save<SavedV4, (), ()>>(b) {
                Ok(s) if s.version == 4 => {
                    let players = s.players.into_iter().map(|p| Saved { name: p.name, pos: p.pos, fwd: p.fwd, creative: p.creative, inv: p.inv, water: p.water, vitals: Vitals::default() }).collect();
                    let Save { seed, planet_n, moon_n, blocks, edits, water, made, removed, sea_extra, tick, .. } = s;
                    Save { version: 4, seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map: None, reaches: Reaches::default() }
                }
                _ => return Err(format!("not a save of version 4 to {VERSION}: older ones are worlds of 0.5 m blocks (ADR 0033)")),
            }
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
        Save { version, seed: 9, planet_n: 256, moon_n: 32, blocks, edits: vec![(b, 0)], players: Vec::new(), water: Vec::new(), made: 0, removed: 0, sea_extra: 0, tick: 7, map: Some(0x5eed), reaches: Reaches::default() }
    }

    /// A save reads back as written; one from the world of 0.5 m blocks is refused, and says why.
    #[test]
    fn a_save_reads_back_and_an_older_world_is_refused() {
        let s = save(VERSION, registry());
        assert_eq!(Save::from_bytes(&s.to_bytes()).unwrap(), s);
        let old = Save::from_bytes(&save(3, registry()).to_bytes()).unwrap_err();
        assert!(old.contains("0.5 m"), "{old}");
    }

    /// A save of version 4 (before health and hunger) loads, its players at full health and hunger; a player
    /// that is hurt, hungry or dead comes back so from a save of this version.
    #[test]
    fn a_save_from_before_vitals_loads_at_full_health() {
        let p4 = SavedV4 { name: "ape".into(), pos: [1.0, 2.0, 3.0], fwd: [1.0, 0.0, 0.0], creative: false, inv: vec![(2, 5)], water: 7 };
        let s = save(VERSION, registry());
        let old = Save::<SavedV4, (), ()> {
            version: 4, seed: s.seed, planet_n: s.planet_n, moon_n: s.moon_n, blocks: s.blocks.clone(), edits: s.edits.clone(), players: vec![p4],
            water: Vec::new(), made: 0, removed: 0, sea_extra: 0, tick: 7, map: (), reaches: (),
        };
        let back = Save::from_bytes(&postcard::to_allocvec(&old).unwrap()).expect("version 4 loads");
        assert_eq!((back.version, back.map), (4, None), "an old save keeps its version and its noise land");
        assert_eq!((back.players[0].name.as_str(), back.players[0].water, &back.players[0].vitals), ("ape", 7, &Vitals::default()));
        assert_eq!(back.edits, s.edits);
        let hurt = Vitals { health: 3, hunger: 0.25, clock: 17, dead: Some("ape fell 30 m".into()) };
        let mut now = back.clone();
        (now.version, now.players[0].vitals) = (VERSION, hurt.clone());
        assert_eq!(Save::from_bytes(&now.to_bytes()).unwrap().players[0].vitals, hurt);
    }

    /// A save of version 5 (before the bake) loads on its noise land; this version keeps its map's hash.
    #[test]
    fn a_save_from_before_the_bake_keeps_its_noise_land() {
        let s = save(VERSION, registry());
        let Save { seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, .. } = s.clone();
        let v5 = Save::<Saved, (), ()> { version: 5, seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map: (), reaches: () };
        let back = Save::from_bytes(&postcard::to_allocvec(&v5).unwrap()).expect("version 5 loads");
        assert_eq!((back.map, back.edits.clone()), (None, s.edits.clone()));
        assert_eq!(Save::from_bytes(&s.to_bytes()).unwrap().map, Some(0x5eed));
    }

    /// Version 6 was two layouts on two branches: without a map it has ADR 0041's coordinates and stays a 6; with
    /// a map it is from before them and reads as a 5, so `Server::from_save` moves its players.
    #[test]
    fn the_two_saves_of_version_6_are_told_apart() {
        let s = save(6, registry());
        let baked = Save::from_bytes(&s.to_bytes()).expect("a 6 with a map loads");
        assert_eq!((baked.version, baked.map), (5, Some(0x5eed)));
        let Save { seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, .. } = s;
        let bare = Save::<Saved, (), ()> { version: 6, seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map: (), reaches: () };
        let moved = Save::from_bytes(&postcard::to_allocvec(&bare).unwrap()).expect("a 6 without a map loads");
        assert_eq!((moved.version, moved.map), (6, None));
    }

    /// The reaches' open water (W5, ADR 0059) is kept: what each took in and the rain that came, so the count
    /// holds across a reload; a save of version 7, from before them, loads with none.
    #[test]
    fn the_reaches_water_is_kept_and_a_version_7_still_loads() {
        let mut s = save(VERSION, registry());
        s.reaches = (5, vec![(3, -40), (17, 12)]);
        assert_eq!(Save::from_bytes(&s.to_bytes()).unwrap().reaches, s.reaches);
        let Save { seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map, .. } = s;
        let v7 = Save::<Saved, Option<u64>, ()> { version: 7, seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map, reaches: () };
        let back = Save::from_bytes(&postcard::to_allocvec(&v7).unwrap()).expect("version 7 loads");
        assert_eq!((back.version, back.map, back.reaches), (7, Some(0x5eed), Reaches::default()));
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
