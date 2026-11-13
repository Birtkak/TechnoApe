//! Saves (ADR 0016): only what differs from the seed. That is every edited cell plus every named player's
//! position, mode and inventory; the rest is generated again from the seed. Written whole to a temporary file
//! and renamed over the old save, so a crash mid-write keeps the old save.
use engine::blocks::BLOCKS;
use engine::items::{self, Slot, ITEMS, ITEM_BASE};
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
/// land. A save keeps the version it was read as until it is written again. 8 keeps the live air grid (W3);
/// an older one on a map with a climate starts its air from the map's. A save on a map of version 1 or 2 needs
/// that map's file in `saves/bakes/`: since map version 3 a bake makes another climate (ADR 0060). 9 keeps the
/// river reaches' open water too (W5, ADR 0059); an 8 or older loads with none. 10 is the 192 m crust (ADR
/// 0063): the same layers, some 140 more under them, so a save of 9 or older moves its cells and water up by
/// its bodies' `deep_layers` on reading (9 lies on disk as 10 does).
/// 11 names Luna's baked map (W6, ADR 0070); a 10 or older keeps Luna on its hills, so edits there stay on the
/// ground they were made on. 12 keeps the rain each river reach holds and an air job back but not shown yet
/// (W5, ADR 0059), so a loaded server hands that job's runoff to the reaches on the same tick; an 11 or older
/// loads with none. 13 keeps slots in place of counts (ADR 0074): each player's stacks, the items' names (an
/// item's id is given at load, so a save maps its ids by name) and the piles on the ground; a 12 or older loads
/// its counts into slots in block id order, and what does not fit lies at the player's feet (`Server::from_save`).
/// 14 keeps the pick-ups taken (ADR 0082); a 13 loads with none taken.
pub const VERSION: u32 = 14;

/// The rain each reach holds (`Liquids::rain_held`), an air job back but not shown yet as (ticks till it
/// shows, its view) (`Weather::pending`), and the water's steps (`Liquids::steps`: once a second the reaches
/// pass their water on, at the same step as a server that kept running).
pub type Rain = (Vec<(u32, i64)>, Option<(u64, engine::weather::View)>, u64);

/// The rain brought into the reaches, and what each reach's open water took in (`Liquids::rained`,
/// `Liquids::reach_extra`).
pub type Reaches = (i64, Vec<(u32, i64)>);

/// A save as written; `Save<SavedV4, (), (), (), ()>` is how version 4 lies on disk, `Save<Saved, (), (), (), ()>`
/// version 5, `Save<Saved, Option<u64>, (), (), ()>` 6 and 7 and `Save<Saved, Option<u64>, Option<State>, (), ()>` 8, `Save<.., Reaches, ()>` 9 and 10
/// (postcard keeps no field names, so a player with one more field is another layout; `()` is no bytes).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Save<P = Saved, M = Option<u64>, A = Option<engine::bake::air::State>, R = Reaches, L = Option<u64>, W = Rain, I = Held> {
    pub version: u32,
    pub seed: i32,
    pub planet_n: i32,
    pub moon_n: i32,
    /// Every registered block as (id, name) when saved: ids must keep their names (ADR 0013).
    pub blocks: Vec<(u16, String)>,
    /// Every edited cell, sorted, so one state always gives the same file.
    pub edits: Vec<(Block, u16)>,
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
    /// The live air grid's state (W3, `engine::weather`); None on the noise land or an older map.
    pub air: A,
    /// The river reaches' open water (W5); none before version 9.
    pub reaches: R,
    /// The hash of Luna's baked map (`engine::bake::luna`); None: Luna's hills, as every save before version 10.
    pub luna: L,
    /// The reaches' rain and the air's pending job (W5); none before version 12.
    pub rain: W,
    /// The items' names and the piles (ADR 0074); none before version 13.
    pub held: I,
}

/// What a save of version 13 keeps of items: the (id, name) of every item that is not a block when saved, and
/// every pile on the ground, sorted by cell.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Held {
    pub items: Vec<(u16, String)>,
    pub piles: Vec<(Block, Vec<Slot>)>,
    /// The pick-ups taken (ADR 0082): the cell each lay in and the year, sorted by cell; none before 14.
    pub taken: Vec<(Block, u32)>,
}

/// `Held` as version 13 keeps it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Held13 {
    pub items: Vec<(u16, String)>,
    pub piles: Vec<(Block, Vec<Slot>)>,
}

/// A save of version 12 or older, before slots: what every older layout is read into first.
type Old = Save<SavedV12, Option<u64>, Option<engine::bake::air::State>, Reaches, Option<u64>, Rain, ()>;

/// A player the save remembers, found again by name on `Hello`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Saved {
    pub name: String,
    pub pos: [f64; 3],
    pub fwd: [f32; 3],
    pub creative: bool,
    /// Its slots (`items::APE_SLOTS`; a save of 12 or older gives its counts as a longer list of stacks, which
    /// `Server::from_save` puts into slots).
    pub slots: Vec<Slot>,
    /// Units in the bucket.
    pub water: u32,
    /// Health and hunger (ADR 0040), and how it died if it is dead.
    pub vitals: Vitals,
}

/// A player in a save of version 5 to 12: counts by block id in place of slots.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SavedV12 {
    pub name: String,
    pub pos: [f64; 3],
    pub fwd: [f32; 3],
    pub creative: bool,
    pub inv: Vec<(u16, u32)>,
    pub water: u32,
    pub vitals: Vitals,
}

/// A player in a save of version 4: before health and hunger.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SavedV4 {
    pub name: String,
    pub pos: [f64; 3],
    pub fwd: [f32; 3],
    pub creative: bool,
    pub inv: Vec<(u16, u32)>,
    pub water: u32,
}

pub fn registry() -> Vec<(u16, String)> {
    BLOCKS.placeable().map(|d| (d.id, d.name.clone())).collect()
}

/// A save of 12 or older as this version: each player's counts as stacks, no piles.
fn upgrade(o: Old) -> Save {
    let Save { version, seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map, air, reaches, luna, rain, .. } = o;
    let players = players.into_iter().map(|p| Saved { name: p.name, pos: p.pos, fwd: p.fwd, creative: p.creative, slots: items::from_counts(&p.inv), water: p.water, vitals: p.vitals }).collect();
    Save { version, seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map, air, reaches, luna, rain, held: Held::default() }
}

impl Save {
    pub fn to_bytes(&self) -> Vec<u8> {
        postcard::to_allocvec(self).expect("a save serialises")
    }

    /// Reads a save and checks it is this version (or 4 or 5, carried over) and its blocks are this build's blocks.
    pub fn from_bytes(b: &[u8]) -> Result<Save, String> {
        let v13 = || postcard::take_from_bytes::<Save<Saved, Option<u64>, Option<engine::bake::air::State>, Reaches, Option<u64>, Rain, Held13>>(b).ok().filter(|(s, rest)| rest.is_empty() && s.version == 13).map(|(s, _)| s);
        let now = postcard::from_bytes::<Save>(b).ok().filter(|s| s.version == VERSION).or_else(|| {
            let Save { version, seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map, air, reaches, luna, rain, held } = v13()?;
            Some(Save { version, seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map, air, reaches, luna, rain, held: Held { items: held.items, piles: held.piles, taken: Vec::new() } })
        });
        let v12 = || postcard::take_from_bytes::<Old>(b).ok().filter(|(s, rest)| rest.is_empty() && s.version == 12).map(|(s, _)| s);
        // nothing left over: W6's pre-merge builds wrote a 10 with `luna` after it, on the old crust (ADR 0070)
        let lunar = || postcard::take_from_bytes::<Save<SavedV12, Option<u64>, Option<engine::bake::air::State>, Reaches, Option<u64>, (), ()>>(b).ok().filter(|(s, rest)| rest.is_empty() && s.version == 11).map(|(s, _)| s);
        let rivered = || postcard::take_from_bytes::<Save<SavedV12, Option<u64>, Option<engine::bake::air::State>, Reaches, (), (), ()>>(b).ok().filter(|(s, rest)| rest.is_empty() && (s.version == 10 || s.version == 9)).map(|(s, _)| s);
        let aired = || postcard::from_bytes::<Save<SavedV12, Option<u64>, Option<engine::bake::air::State>, (), (), (), ()>>(b).ok().filter(|s| s.version == 8);
        let mapped = || postcard::from_bytes::<Save<SavedV12, Option<u64>, (), (), (), (), ()>>(b).ok().filter(|s| s.version == 7 || s.version == 6);
        let bare = || postcard::from_bytes::<Save<SavedV12, (), (), (), (), (), ()>>(b).ok().filter(|s| s.version == 5 || s.version == 6);
        let old: Option<Old> = if now.is_some() {
            None
        } else if let Some(s) = v12() {
            Some(s)
        } else if let Some(s) = lunar() {
            let Save { version, seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map, air, reaches, luna, .. } = s;
            Some(Save { version, seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map, air, reaches, luna, rain: Rain::default(), held: () })
        } else if let Some(s) = rivered() {
            let Save { version, seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map, air, reaches, .. } = s;
            Some(Save { version, seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map, air, reaches, luna: None, rain: Rain::default(), held: () })
        } else if let Some(s) = aired() {
            let Save { version, seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map, air, .. } = s;
            Some(Save { version, seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map, air, reaches: Reaches::default(), luna: None, rain: Rain::default(), held: () })
        } else if let Some(s) = mapped() {
            let Save { version, seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map, .. } = s;
            Some(Save { version: if version == 6 { 5 } else { version }, seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map, air: None, reaches: Reaches::default(), luna: None, rain: Rain::default(), held: () })
        } else if let Some(s) = bare() {
            let Save { version, seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, .. } = s;
            Some(Save { version, seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map: None, air: None, reaches: Reaches::default(), luna: None, rain: Rain::default(), held: () })
        } else {
            match postcard::from_bytes::<Save<SavedV4, (), (), (), (), (), ()>>(b) {
                Ok(s) if s.version == 4 => {
                    let players = s.players.into_iter().map(|p| SavedV12 { name: p.name, pos: p.pos, fwd: p.fwd, creative: p.creative, inv: p.inv, water: p.water, vitals: Vitals::default() }).collect();
                    let Save { seed, planet_n, moon_n, blocks, edits, water, made, removed, sea_extra, tick, .. } = s;
                    Some(Save { version: 4, seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map: None, air: None, reaches: Reaches::default(), luna: None, rain: Rain::default(), held: () })
                }
                _ => return Err(format!("not a save of version 4 to {VERSION}: older ones are worlds of 0.5 m blocks (ADR 0033)")),
            }
        };
        let mut s: Save = match old {
            None => now.expect("read as this version"),
            Some(o) => upgrade(o),
        };
        if s.version < 10 {
            let deep = [engine::world::deep_layers(s.planet_n), engine::world::deep_layers(s.moon_n)];
            for (b, _) in &mut s.edits {
                b.pos.z += deep[b.body as usize];
            }
            for (c, gaps) in &mut s.water {
                let d = deep[c.body() as usize];
                for g in gaps {
                    g.0 += d;
                    if g.1 != engine::liquid::OPEN {
                        g.1 += d;
                    }
                }
            }
        }
        for (id, name) in &s.blocks {
            if !BLOCKS.get(*id).is_some_and(|d| &d.name == name || d.was.contains(name)) {
                return Err(format!("block {id} was '{name}' in the save; data/blocks.toml no longer has it so"));
            }
        }
        // the items' ids by name (ADR 0074): an id of the save's table becomes this build's id of that name
        let ids: Vec<(u16, u16)> = s
            .held
            .items
            .iter()
            .map(|(id, name)| ITEMS.named(name).filter(|&n| n >= ITEM_BASE).map(|n| (*id, n)).ok_or(format!("item {id} was '{name}' in the save; data/items.toml no longer has it")))
            .collect::<Result<_, _>>()?;
        let Save { players, held, .. } = &mut s;
        for st in players.iter_mut().flat_map(|p| p.slots.iter_mut()).chain(held.piles.iter_mut().flat_map(|p| p.1.iter_mut())).flatten() {
            if st.item >= ITEM_BASE {
                st.item = ids.iter().find(|m| m.0 == st.item).ok_or(format!("the save holds item {}, which its table does not name", st.item))?.1;
            }
        }
        // a stack over its item's size (a smaller `stack` in data/items.toml) loads: the server re-packs it
        let known = |v: &[Slot]| v.iter().flatten().all(|s| ITEMS.known(s.item));
        if !players.iter().all(|p| known(&p.slots)) || !held.piles.iter().all(|p| p.1.len() == items::PILE_SLOTS && known(&p.1)) {
            return Err("the save holds a stack of an unknown block".into());
        }
        held.items = ITEMS.table();
        // the cells name only known blocks: an id past the registry would index into the mesh's other bits
        let bad = s.edits.iter().map(|e| e.1).find(|&id| BLOCKS.get(id).is_none());
        if let Some(id) = bad {
            return Err(format!("the save holds block {id}, which data/blocks.toml does not have"));
        }
        Ok(s)
    }

    pub fn write(&self, path: &Path) -> std::io::Result<()> {
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        let tmp = path.with_extension("tmp");
        // on the disk before the rename, so a crash leaves the old save or the new one, never half of one
        let mut f = std::fs::File::create(&tmp)?;
        std::io::Write::write_all(&mut f, &self.to_bytes())?;
        f.sync_all()?;
        drop(f);
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

    fn save(version: u32, blocks: Vec<(u16, String)>) -> Save {
        let b = Block { body: 0, face: 1, pos: IVec3::new(3, 4, 5) };
        Save { version, seed: 9, planet_n: 256, moon_n: 32, blocks, edits: vec![(b, 0)], players: Vec::new(), water: Vec::new(), made: 0, removed: 0, sea_extra: 0, tick: 7, map: Some(0x5eed), air: None, reaches: Reaches::default(), luna: None, rain: Rain::default(), held: Held { items: if version >= 13 { ITEMS.table() } else { Vec::new() }, piles: Vec::new(), taken: Vec::new() } }
    }

    /// The same save in the layout of versions 6 and 7: no air.
    fn without_air(s: &Save) -> Vec<u8> {
        let Save { version, seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map, .. } = s.clone();
        postcard::to_allocvec(&Save::<Saved, Option<u64>, (), (), (), (), ()> { version, seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map, air: (), reaches: (), luna: (), rain: (), held: () }).unwrap()
    }

    /// A save of version 7 (before the live air) loads with its map and no air; this version keeps the air's
    /// state, and one with a state reads back with it.
    #[test]
    fn a_save_from_before_the_air_loads_and_this_one_keeps_it() {
        let s = save(VERSION, registry());
        let v7 = Save::from_bytes(&without_air(&Save { version: 7, ..s.clone() })).expect("version 7 loads");
        assert_eq!((v7.version, v7.map, v7.air.is_none()), (7, Some(0x5eed), true));
        let a = engine::bake::air::State { t: 12.5, theta: vec![1.0; 8], vap: vec![2; 8], cloud: vec![0; 8], dirt: vec![0; 8], wind: vec![0.5; 4], tg: vec![3.0; 2], soil: vec![4; 2], snow: vec![0; 2], dirt_down: vec![0; 2], sea: 99 };
        let aired = Save { air: Some(a), ..s };
        assert_eq!(Save::from_bytes(&aired.to_bytes()).unwrap(), aired);
    }

    /// A save written while block ids were 8 bits (ADR 0062) loads unchanged: postcard writes a u16 under 128
    /// as the same one byte, and no build had a block id over 29 then.
    #[test]
    fn a_save_with_8_bit_block_ids_loads() {
        let b = Block { body: 0, face: 1, pos: IVec3::new(3, 4, 5) };
        // (the blocks of that time: Luna's from 200 came later, and a u8 over 127 is not a u16's byte)
        let then: Vec<(u16, String)> = registry().into_iter().filter(|b| b.0 < 128).collect();
        let blocks: Vec<(u8, String)> = then.iter().map(|(id, n)| (*id as u8, n.clone())).collect();
        let stone = BLOCKS.id("stone");
        let player = ("ape".to_string(), [1.0f64, 2.0, 3.0], [1.0f32, 0.0, 0.0], false, vec![(stone as u8, 5u32)], 7u32, Vitals::default());
        let old = (9u32, 9i32, 256i32, 32i32, blocks, vec![(b, stone as u8)], vec![player], Vec::<ColWater>::new(), 0i64, 0i64, 0i64, 7u64, Some(0x5eedu64), None::<engine::bake::air::State>, Reaches::default());
        let s = Save::from_bytes(&postcard::to_allocvec(&old).unwrap()).expect("an 8-bit save loads");
        assert_eq!((s.edits.clone(), s.players[0].slots.clone()), (vec![(b, stone)], vec![Some(items::Stack::new(stone, 5))]));
        assert_eq!(s.blocks, then);
        let mut bad = Save { version: VERSION, ..s.clone() };
        bad.players[0].slots.push(Some(items::Stack::new(300, 1)));
        assert!(Save::from_bytes(&bad.to_bytes()).unwrap_err().contains("unknown block"));
    }

    /// A save from before the 192 m crust (ADR 0063) moves its cells and water up by the deep layers of
    /// each body; one of this version reads as written.
    #[test]
    fn a_save_from_before_the_deep_ground_moves_up() {
        let (n, m) = (1536, 32);
        assert_eq!((engine::world::deep_layers(n), engine::world::deep_layers(m)), (146, 0));
        let at = |body: u8, z: i32| Block { body, face: 1, pos: IVec3::new(3, 4, z) };
        let col = engine::liquid::Col::new(0, 1, 3, 4);
        let mut s = Save { planet_n: n, moon_n: m, edits: vec![(at(0, 70), 0), (at(1, 70), 0)], water: vec![(col, vec![(60, 62, 5), (70, engine::liquid::OPEN, 0)])], ..save(9, registry()) };
        let mut bytes = s.to_bytes();
        let n = bytes.len();
        assert_eq!(bytes.split_off(n - 7), vec![0; 7], "a 9 has no `luna` (a None's one byte), `rain` (three: none, none, 0) nor `held` (three empty lists)");
        let old = Save::from_bytes(&bytes).unwrap();
        assert_eq!(old.edits, vec![(at(0, 216), 0), (at(1, 70), 0)]);
        assert_eq!(old.water, vec![(col, vec![(206, 208, 5), (216, engine::liquid::OPEN, 0)])]);
        (s.version, s.held.items) = (VERSION, ITEMS.table());
        assert_eq!(Save::from_bytes(&s.to_bytes()).unwrap(), s);
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
        let old = Save::<SavedV4, (), (), (), (), (), ()> {
            version: 4, seed: s.seed, planet_n: s.planet_n, moon_n: s.moon_n, blocks: s.blocks.clone(), edits: s.edits.clone(), players: vec![p4],
            water: Vec::new(), made: 0, removed: 0, sea_extra: 0, tick: 7, map: (), air: (), reaches: (), luna: (), rain: (), held: (),
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
        let v5 = Save::<Saved, (), (), (), (), (), ()> { version: 5, seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map: (), air: (), reaches: (), luna: (), rain: (), held: () };
        let back = Save::from_bytes(&postcard::to_allocvec(&v5).unwrap()).expect("version 5 loads");
        assert_eq!((back.map, back.edits.clone()), (None, s.edits.clone()));
        assert_eq!(Save::from_bytes(&s.to_bytes()).unwrap().map, Some(0x5eed));
    }

    /// Version 6 was two layouts on two branches: without a map it has ADR 0041's coordinates and stays a 6; with
    /// a map it is from before them and reads as a 5, so `Server::from_save` moves its players.
    #[test]
    fn the_two_saves_of_version_6_are_told_apart() {
        let s = save(6, registry());
        let baked = Save::from_bytes(&without_air(&s)).expect("a 6 with a map loads");
        assert_eq!((baked.version, baked.map), (5, Some(0x5eed)));
        let Save { seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, .. } = s;
        let bare = Save::<Saved, (), (), (), (), (), ()> { version: 6, seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map: (), air: (), reaches: (), luna: (), rain: (), held: () };
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
        let v7 = Save::from_bytes(&without_air(&Save { version: 7, ..s })).expect("version 7 loads");
        assert_eq!((v7.version, v7.map, v7.reaches), (7, Some(0x5eed), Reaches::default()));
    }

    /// A save of version 8 (W3's live air, ADR 0060, before the reaches) loads with its air and no reaches.
    #[test]
    fn a_version_8_loads_with_its_air() {
        let Save { seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map, .. } = save(VERSION, registry());
        let a = engine::bake::air::State { t: 12.5, theta: vec![1.0; 8], vap: vec![2; 8], cloud: vec![0; 8], dirt: vec![0; 8], wind: vec![0.5; 4], tg: vec![3.0; 2], soil: vec![4; 2], snow: vec![0; 2], dirt_down: vec![0; 2], sea: 99 };
        let v8 = Save::<Saved, Option<u64>, Option<engine::bake::air::State>, (), (), (), ()> { version: 8, seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map, air: Some(a.clone()), reaches: (), luna: (), rain: (), held: () };
        let back = Save::from_bytes(&postcard::to_allocvec(&v8).unwrap()).expect("version 8 loads");
        assert_eq!((back.version, back.map, back.air, back.reaches), (8, Some(0x5eed), Some(a), Reaches::default()));
    }

    /// A save of version 9 or 10 (before Luna's bake, ADR 0070) loads with Luna on its hills; this version names
    /// Luna's map and reads back with it.
    #[test]
    fn a_version_9_or_10_keeps_luna_on_its_hills() {
        for version in [9, 10] {
            let Save { seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map, .. } = save(VERSION, registry());
            let reaches: Reaches = (5, vec![(2, 3)]);
            let old = Save::<Saved, Option<u64>, Option<engine::bake::air::State>, Reaches, (), (), ()> { version, seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map, air: None, reaches: reaches.clone(), luna: (), rain: (), held: () };
            let back = Save::from_bytes(&postcard::to_allocvec(&old).unwrap()).unwrap_or_else(|e| panic!("version {version} loads: {e}"));
            assert_eq!((back.version, back.map, back.reaches, back.luna), (version, Some(0x5eed), reaches, None));
        }
        let now = Save { luna: Some(0x1a2a), ..save(VERSION, registry()) };
        assert_eq!(Save::from_bytes(&now.to_bytes()).unwrap().luna, Some(0x1a2a));
        // W6's builds before the merge wrote version 10 with `luna` on the old crust: refused, not read as W4's 10
        let branch = Save { version: 10, luna: Some(0x1a2a), ..save(VERSION, registry()) };
        assert!(Save::from_bytes(&branch.to_bytes()).is_err());
    }

    /// 12 keeps the reaches' rain, the air's job back but not shown yet and the water's steps (W5); an 11
    /// loads with none of them.
    #[test]
    fn the_rain_is_kept_and_a_version_11_loads_without_it() {
        let mut s = save(VERSION, registry());
        s.rain = (vec![(3, 40), (9, -2)], None, 1234);
        assert_eq!(Save::from_bytes(&s.to_bytes()).unwrap().rain, s.rain);
        let Save { seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map, air, reaches, luna, .. } = s;
        let v11 = Save::<Saved, Option<u64>, Option<engine::bake::air::State>, Reaches, Option<u64>, (), ()> { version: 11, seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map, air, reaches, luna, rain: (), held: () };
        let back = Save::from_bytes(&postcard::to_allocvec(&v11).unwrap()).expect("version 11 loads");
        assert_eq!((back.version, back.map, back.rain), (11, Some(0x5eed), Rain::default()));
    }

    /// A block renamed in data (moon dust became regolith, W0) keeps its id: a save under the old name loads.
    #[test]
    fn a_save_with_a_renamed_block_loads() {
        let old: Vec<(u16, String)> = registry().into_iter().map(|(id, n)| (id, if n == "regolith" { "moon dust".into() } else { n })).collect();
        assert!(old.iter().any(|b| b.1 == "moon dust"));
        assert!(Save::from_bytes(&save(VERSION, old).to_bytes()).is_ok());
        assert!(Save::from_bytes(&save(VERSION, vec![(7, "cheese".to_string())]).to_bytes()).is_err(), "an unknown name is still refused");
    }

    /// 13 keeps slots, the items' names and the piles (ADR 0074): an item's id is mapped by its name, so a save
    /// whose items had other ids loads; an unknown name is refused; a 12 loads its counts as stacks.
    #[test]
    fn slots_and_piles_are_kept_and_items_are_found_by_name() {
        use items::Stack;
        let (stone, basket) = (BLOCKS.id("snow"), ITEMS.named("basket").unwrap());
        let p = |slots: Vec<Slot>| Saved { name: "ape".into(), pos: [1.0, 2.0, 3.0], fwd: [1.0, 0.0, 0.0], creative: false, slots, water: 0, vitals: Vitals::default() };
        let mut slots = items::ape();
        (slots[0], slots[1]) = (Some(Stack::new(stone, 5)), Some(Stack::new(basket + 7, 1)));
        let cell = Block { body: 0, face: 2, pos: IVec3::new(1, 2, 3) };
        let held = Held { items: vec![(basket + 7, "basket".into())], piles: vec![(cell, vec![Some(Stack::new(basket + 7, 1)), None, None, None])], taken: vec![(cell, 3)] };
        let s = Save { players: vec![p(slots)], held, ..save(VERSION, registry()) };
        let back = Save::from_bytes(&s.to_bytes()).expect("loads");
        assert_eq!((back.players[0].slots[1], back.held.piles[0].1[0]), (Some(Stack::new(basket, 1)), Some(Stack::new(basket, 1))));
        assert_eq!(back.held.items, ITEMS.table());
        let gone = Save { held: Held { items: vec![(basket + 7, "sledge".into())], ..s.held.clone() }, ..s.clone() };
        assert!(Save::from_bytes(&gone.to_bytes()).unwrap_err().contains("sledge"));
        // a 12: counts
        let Save { seed, planet_n, moon_n, blocks, edits, water, made, removed, sea_extra, tick, map, air, reaches, luna, rain, .. } = s;
        let old = SavedV12 { name: "ape".into(), pos: [1.0, 2.0, 3.0], fwd: [1.0, 0.0, 0.0], creative: false, inv: vec![(stone, 70)], water: 0, vitals: Vitals::default() };
        let v12: Old = Save { version: 12, seed, planet_n, moon_n, blocks, edits, players: vec![old], water, made, removed, sea_extra, tick, map, air, reaches, luna, rain, held: () };
        let back = Save::from_bytes(&postcard::to_allocvec(&v12).unwrap()).expect("version 12 loads");
        assert_eq!((back.version, back.players[0].slots.clone()), (12, vec![Some(Stack::new(stone, 64)), Some(Stack::new(stone, 6))]));
    }

    /// 14 keeps the pick-ups taken (ADR 0082); a 13 loads with none taken.
    #[test]
    fn the_pick_ups_taken_are_kept_and_a_13_has_none() {
        let cell = Block { body: 0, face: 2, pos: IVec3::new(1, 2, 3) };
        let mut s = save(VERSION, registry());
        s.held.taken = vec![(cell, 3)];
        assert_eq!(Save::from_bytes(&s.to_bytes()).expect("loads").held.taken, vec![(cell, 3)]);
        let Save { seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map, air, reaches, luna, rain, held, .. } = s;
        let v13 = Save { version: 13, seed, planet_n, moon_n, blocks, edits, players, water, made, removed, sea_extra, tick, map, air, reaches, luna, rain, held: Held13 { items: held.items, piles: held.piles } };
        let back = Save::from_bytes(&postcard::to_allocvec(&v13).unwrap()).expect("version 13 loads");
        assert_eq!((back.version, back.held.taken), (13, Vec::new()));
    }
}
