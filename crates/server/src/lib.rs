//! The authoritative world (ADR 0006): one `Server` owns the `World`, ticks at `TICK_HZ`, applies the edits
//! players send and tells everyone what changed. Connections arrive on a channel, so the same server runs
//! inside the singleplayer client (an in-memory link) and as the dedicated binary (TCP links).
//!
//! The flight recorder (ADR 0011): with `record`, every join, message and leave goes to a file with its tick,
//! and every second a hash of the state; `replay` runs the file again headless and finds the first checkpoint
//! where the state parts from the recording.
pub mod report;
pub mod save;
pub mod soak;

use engine::net::{local, Link, Player, Recv, ToClient, ToServer, REACH, TICK_HZ};
use engine::blocks::{AIR, BLOCKS};
use engine::edit::{self, Shape};
use engine::events::Event;
use engine::items::{self, Holder, Slot, Stack, ITEMS};
use engine::phys::{self, Input};
use engine::liquid::{Col, TILE, U};
use engine::world::{fnv, Block, FxMap, World};
use engine::sky;
use engine::vitals::{self, Vitals};
use glam::DVec3;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use save::{Save, Saved};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};
mod state;
mod tick;
mod commands;
mod piles;
mod furnaces;
mod water;
mod replay;
pub use replay::*;

/// Hash checkpoints in a recording: one a second.
const CHECK_EVERY: u64 = 20;

/// One record of the flight recorder. Join, Msg and Leave carry the tick that took them in; Check the number
/// of ticks done when the hash was taken.
#[derive(Serialize, Deserialize, Debug)]
pub enum Ev {
    /// `map`: the baked map's hash (ADR 0043), None for the noise land.
    Start { planet_n: i32, moon_n: i32, seed: i32, dt: f64, map: Option<u64> },
    /// Right after `Start` when the server began from a save: the save itself, so a replay starts there too.
    Loaded(Save),
    Join { tick: u64, id: u32 },
    Msg { tick: u64, id: u32, m: ToServer },
    Leave { tick: u64, id: u32 },
    Check { tick: u64, hash: u64 },
    /// Right after `Loaded`: the herds kept beside the save (ADR 0083, `Herds::to_bytes`).
    Herds(Vec<u8>),
}

pub type ServerLink = Link<ToClient, ToServer>;

/// `Steps` a player may have waiting (one second), and steps it may take in one tick after a late batch.
const QUEUE_MAX: usize = 60;
const CREDIT_MAX: i32 = 12;

/// Water steps (1/60 s each, at one node) a tick may spend: about 7 ms. Past it the water slows down instead of
/// the server falling behind (ADR 0020).
const WATER_BUDGET: usize = 60_000;
/// Water tiles go to a client within this distance (m), nearest first, about this many bytes a tick.
/// shortcut: far water is not sent, so a flood out of reach is not seen from orbit; a coarse far copy when
/// views from orbit show water.
const WATER_RANGE: f64 = 128.0;
const WATER_BYTES: usize = 16 * 1024;

/// Cells in one `ToClient::Edits`: about 1 MB, far under the TCP frame limit (16 MB, about 1.7 M cells).
const EDITS_PART: usize = 100_000;

/// Messages taken from one client in one tick; the rest wait for the next, so no client keeps a tick from
/// ending. (A walking client sends a few a tick; a map request or a burst of edits catches up in a few ticks.)
const MSGS_PER_TICK: usize = 64;

/// How far /goto sun stands from the sun's centre, in sun radii: it fills about 23 degrees.
const SUN_VIEW: f64 = 5.0;

/// The longest chat line the server takes, in characters.
const CHAT_MAX: usize = 256;
/// The fastest time warp: the 54-day year in about 6 s.
const MAX_WARP: u64 = 10_000;

/// The cell of WATER_RANGE a water tile's centre (or a client) is in.
fn cell(at: DVec3) -> glam::IVec3 {
    (at / WATER_RANGE).floor().as_ivec3()
}

/// The water tiles a client at `me` lacks (it has `have`), within WATER_RANGE, nearest first. Every tile in
/// range lies in the 27 cells around the client's, as a cell is WATER_RANGE wide.
fn wanted(tiles: &FxMap<Col, (u64, DVec3)>, near: &FxMap<glam::IVec3, Vec<Col>>, me: DVec3, have: &FxMap<Col, u64>) -> Vec<(u64, Col)> {
    let c = cell(me);
    let cells = (-1..=1).flat_map(|x| (-1..=1).flat_map(move |y| (-1..=1).map(move |z| c + glam::IVec3::new(x, y, z))));
    let mut want: Vec<(u64, Col)> = cells
        .filter_map(|k| near.get(&k))
        .flatten()
        .map(|t| (t, &tiles[t]))
        .filter(|(t, (v, _))| have.get(t).is_none_or(|s| s < v))
        .map(|(t, (_, at))| ((at.distance(me) * 16.0) as u64, *t))
        .filter(|(d, _)| (*d as f64) < WATER_RANGE * 16.0)
        .collect();
    want.sort_unstable();
    want
}

/// What a save keeps of a connected player.
fn saved(c: &Client) -> Saved {
    Saved { name: c.player.name.clone(), pos: c.phys.pos.to_array(), fwd: c.phys.fwd.as_vec3().to_array(), creative: c.creative, slots: c.slots.clone(), water: c.water, vitals: c.vitals.clone(), marks: c.marks.clone(), journal: engine::journal::JOURNAL.saved(&c.journal, &c.kept) }
}

/// A pile's cell as a key that sorts (by body, face, column, layer: the piles' fixed order, items.md).
type PileKey = (u8, u8, i32, i32, i32);
fn key(b: Block) -> PileKey {
    (b.body, b.face, b.pos.x, b.pos.y, b.pos.z)
}
fn unkey(k: PileKey) -> Block {
    Block { body: k.0, face: k.1, pos: glam::IVec3::new(k.2, k.3, k.4) }
}

/// Things lying in an air cell (ADR 0074): `items::PILE_SLOTS` slots, and the player who threw them with the
/// tick before which it does not pick them up again (so a throw is not taken straight back).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Pile {
    pub slots: Vec<Slot>,
    pub wait: (u32, u64),
}

/// The most layers a laid pile falls to find its floor.
const FALL: i32 = 64;
/// A pile is picked up by a player whose feet are within this of its cell's middle (m), checked every tick.
const PICK: f64 = 1.5;
/// Points a falling rock takes from the head it falls through (ADR 0094).
const ROCK_HIT: u8 = 4;
/// A pile can be opened from this far (m).
const PILE_REACH: f64 = 4.0;
/// A throw lands this far ahead (m), and its thrower cannot take it back for this many ticks (2 s).
const THROW: f64 = 2.0;
const THROW_WAIT: u64 = 40;
/// The most /give hands out at once: 36 stacks.
const GIVE_MAX: u32 = 36 * items::STACK as u32;
/// The journal's `look:` (draft ADR journal-data) is judged this often, in ticks (twice a second), from the
/// player's look as the server holds it, as far as a client aims.
const LOOK_EVERY: u64 = 10;
const LOOK_REACH: f64 = 8.0;

struct Client {
    link: ServerLink,
    player: Player,
    phys: phys::Player,
    /// Walking input; `None` while the player flies with `Move` or walks by `Steps`.
    input: Option<Input>,
    /// `Steps` not taken yet, by number; the number of the last one taken; how many steps it may take now.
    queue: VecDeque<(u32, Input)>,
    acked: u32,
    credit: i32,
    /// Creative counts nothing; survival digs into piles and places from its slots (ADR 0014, 0074).
    creative: bool,
    /// Its slots (`items::SLOTS`): the hotbar, the worn pack, the rows the pack opens, the crafting grid's cells.
    slots: Vec<Slot>,
    /// The workbench it opened (`ToServer::Use`, ADR 0140): its grid's 3 x 3 counts while that is
    /// within `items::BENCH_REACH` (`Server::at_bench`).
    bench: Option<Block>,
    /// The furnace whose window it opened (`ToServer::Use`, ADR 0141), while within `BENCH_REACH`, and
    /// the furnace as it was last sent (`ToClient::Furnace`).
    furnace: Option<Block>,
    sent_furnace: Option<engine::furnace::Furnace>,
    /// The water in its bucket, in units (`liquid::U` is a full bucket).
    water: u32,
    /// The version of each water tile this client has (`Server::tiles`).
    tiles: FxMap<Col, u64>,
    /// The answers to this tick's edits, sent after its `Edits`.
    done: Vec<(u32, bool)>,
    /// The inventory changed this tick (or the player just joined).
    inv_dirty: bool,
    /// Health and hunger (ADR 0040); the motion the last physics step ended with, for the next one's impact;
    /// what the client was last told of them.
    vitals: Vitals,
    was: DVec3,
    /// The breath (draft breath: buried, under water, above the air; not saved: a load starts it anew, ADR 0079).
    breath: vitals::Breath,
    /// Its death mark and sleeping place (draft death's cost), saved; what its client was last told of them.
    marks: save::Marks,
    sent_marks: (Option<Block>, Option<Block>),
    sent: Option<((u8, u8, u8), u8, bool)>,
    /// The water cell the use button is held on (`ToServer::Drink`) and whether it holds fresh water, judged
    /// once when the cell comes (`Server::hold_drink`); not saved, let go on death.
    drink: Option<(Block, bool)>,
    /// The survival dig it holds (ADR 0034), on the server's clock.
    hold: edit::Hold,
    /// The animals as this client was last told of them (ADR 0083).
    animals: FxMap<u32, engine::net::Animal>,
    /// The hotbar slot in hand (`ToServer::Hand`, ADR 0110).
    hand: u8,
    /// The item in hand as the roster last told everyone (ADR 0117): a change sends the roster at once.
    sent_hand: u16,
    /// Knapping (ADR 0110): the item being struck, the strikes that counted, the tick of the last.
    knap: (u16, u8, u64),
    /// The tick of its last prospecting strike that was answered (ADR 0138).
    prospected: Option<u64>,
    /// Its journal (draft ADR journal-data): a state per entry of `JOURNAL`, saved; what its client was last
    /// told of it (None: nothing yet).
    journal: Vec<engine::journal::State>,
    told: Option<Vec<engine::journal::State>>,
    /// Its save's journal entries data/journal.toml does not have (renamed, a typo), saved again untouched.
    kept: Vec<(String, engine::journal::State)>,
}

impl Client {
    /// The tool in hand, in survival (ADR 0110).
    fn tool(&self) -> Option<&'static items::Tool> {
        items::tool(self.slots.get(self.hand as usize).copied().flatten()).filter(|_| !self.creative)
    }
}

/// Allowance on a survival dig's time for the network: the `Dig` and its `Edit` travel the same way, so most
/// of their delays cancel; this covers the rest and a tick's step.
const DIG_LAG_S: f32 = 0.15;
/// And a share of the dig's time: the server's clock drops the time of a late tick, the client's does not.
const DIG_LAG_SHARE: f32 = 0.05;

pub struct Server {
    pub world: World,
    seed: i32,
    accept: Receiver<ServerLink>,
    clients: Vec<Client>,
    next_id: u32,
    pub tick: u64,
    /// The folder of data files a command reads (`/structure`): `data` beside the server.
    pub data: String,
    /// The world's age when this server started (from its save): the sky's clock runs on from it.
    tick0: u64,
    /// The time warp (/timewarp): world ticks per tick, 1 is real time. A restart is back at 1.
    /// shortcut: only the clock runs faster, not the physics, so a player leaving a body's frame under warp
    /// keeps its real-time velocity; physics steps under warp when ships fly orbits.
    warp: u64,
    /// Seconds per tick, for the physics; `run` sets it from its rate.
    pub dt: f64,
    /// Time the physics has not stepped through yet (less than one `phys::DT`).
    debt: f64,
    rec: Option<BufWriter<std::fs::File>>,
    /// Players who left (or a save remembers), by name, waiting for a `Hello` with that name.
    absent: BTreeMap<String, Saved>,
    /// The save this server began from, until the recorder has logged it.
    loaded: Option<Save>,
    /// Where and how often (in ticks) to save; the write runs on its own thread.
    pub autosave: Option<(std::path::PathBuf, u64)>,
    /// The autosave thread still writing, if any: the next save waits for it, as both write one temp file.
    saving: Option<std::thread::JoinHandle<()>>,
    /// Water creative put in and took out, in units: what flowing never changes (`water_ledger`).
    made: i64,
    removed: i64,
    /// Water steps skipped to keep to WATER_BUDGET, and bytes of water sent, since the start.
    pub water_skipped: u64,
    pub water_bytes: u64,
    /// Every water tile that ever changed: the tick it last changed (its version) and its centre.
    tiles: FxMap<Col, (u64, DVec3)>,
    /// The same tiles by the cell of WATER_RANGE their centre is in: a client looks only at the 27 cells
    /// around its own, not at every tile that ever changed. shortcut: both only grow (a few dozen bytes a
    /// tile); forget tiles back at their generated water when a long-running server's memory shows them.
    near: FxMap<glam::IVec3, Vec<Col>>,
    /// When set, `run` adds every tick's length in ms (ADR 0025, S: the benchmark line's tick numbers).
    pub tick_ms: Option<std::sync::Arc<std::sync::Mutex<Vec<f32>>>>,
    /// The baked maps as `ToClient::Map` pieces by hash, each made on its first request (18 MB for the 4 km
    /// planet, 21 for its Luna).
    map_pieces: HashMap<u64, Vec<ToClient>>,
    /// The live air grid (W3), on a planet whose map keeps a climate.
    pub weather: Option<engine::weather::Weather>,
    /// The air's ledger plus the rain it handed the rivers when this server started (`air_ledger`).
    air0: i128,
    /// Each ore body's blocks as generated: the ledger's opening (`ore_ledger`).
    pub ore_open: Vec<i64>,
    /// The animals (ADR 0083): what the seed makes near players and the kept delta of met sites.
    pub herds: engine::herds::Herds,
    /// The data files as last looked at (`watch_defs`): each one's time and size.
    defs_seen: Option<[Option<(std::time::SystemTime, u64)>; 4]>,
    /// Whether `data/` is watched (`watch_defs`): the game's servers; never a soak, a replay or a test, which
    /// run on the build's rows whatever lies beside them.
    pub live_data: bool,
    /// The herds read beside a save (`read_herds`), until the recorder has logged them.
    herds_loaded: Option<Vec<u8>>,
    /// Loose blocks awake to fall or slide (ADR 0079).
    pub falls: engine::fall::Falls,
    /// The air as clients have been sent it (ADR 0076), the view's hash it was made from (None: make it
    /// again), and the weather `/weather` forces.
    sky: Vec<engine::weather::Cell>,
    sky_at: Option<u64>,
    force: Option<engine::weather::Force>,
    /// Every pile on the ground, by cell (ADR 0074), and the cells whose pile changed this tick.
    piles: BTreeMap<PileKey, Pile>,
    piles_changed: Vec<Block>,
    /// Of each item, what digs, places, creative's grid and /give made since the start (or what was held at
    /// the load): the items' ledger (`item_ledger`).
    item_made: BTreeMap<u16, i64>,
    /// The pick-ups taken (ADR 0082): the cell each lay in and the year, and those taken this tick.
    taken: BTreeMap<PileKey, u32>,
    taken_changed: Vec<(Block, u32)>,
    /// Every furnace that holds something, burns, is warm or has a cast chosen, by its block (ADR 0141);
    /// one missing is a fresh one. And the fires lit or gone out this tick, told to everyone.
    furnaces: BTreeMap<PileKey, engine::furnace::Furnace>,
    fires_changed: Vec<(Block, bool)>,
    /// What happened this tick (`engine::events`, ADR 0149), in the order the tick's systems pushed it;
    /// sent to the clients that perceive it before the tick's edits, then cleared.
    events: Vec<engine::events::Event>,
}

#[cfg(test)]
mod tests;
