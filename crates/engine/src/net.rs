//! The player protocol (ADR 0006). A client, a bot and the server all speak it; the transport is a pair of
//! channels, fed either directly (the local server in the same process) or by a TCP connection.
use crate::edit::Shape;
use crate::phys::Input;
use crate::world::Block;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};

pub const DEFAULT_PORT: u16 = 7777;
pub const TICK_HZ: f64 = 20.0;
/// How far from the player an edit may be; the server drops anything farther.
pub const REACH: f64 = 16.0;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ToServer {
    Hello { name: String },
    /// Fly or teleport: the position is taken as sent (creative mode; no check yet).
    Move { pos: [f64; 3], fwd: [f32; 3], pitch: f32 },
    /// Walk: the server moves the player with `phys::step` from this input every tick until a `Move`.
    Input(Input),
    /// Walk, predicted (ADR 0019): one input per physics step, numbered from `first`. The server takes each
    /// exactly once, in order, at most as fast as real time, and answers `You` with the last one it took.
    Steps { first: u32, inputs: Vec<Input> },
    /// Dig (`material` 0) or place a shape. The server applies all of it or none and answers `Done` with the
    /// same `seq`, after the `Edits` that carry what it changed.
    Edit { seq: u32, shape: Shape, material: u16 },
    /// Survival or creative (counts nothing). shortcut: anyone may switch; an operator role when servers go public.
    Creative(bool),
    /// The bucket (ADR 0020): `fill` scoops up to a block of water from the gap that holds block `at`, else
    /// the bucket is poured out into it. Answered by `Done` with the same `seq`, like an edit.
    Bucket { seq: u32, at: Block, fill: bool },
    /// A chat line, or a command when it starts with '/' (ADR 0023): the server answers a command to this
    /// player alone and passes any other line to everyone, with the sender's name.
    Chat(String),
    /// A dead player back at the spawn with full health and hunger (ADR 0040); nothing while alive.
    Respawn,
    /// The shape this survival player now holds the dig on, or none: let go or looking elsewhere (ADR 0034).
    /// The server counts the time held on it (`edit::Hold`) and takes the dig's `Edit` only once it has had
    /// its `edit::dig_time`; a shape left keeps its time for `edit::DIG_KEEP_S`.
    Dig(Option<Shape>),
    /// Send the baked map of this hash (ADR 0043; Terra's or Luna's, ADR 0070): `Welcome` named one this
    /// client has neither in memory nor on disk. Answered by `Map` pieces.
    Map(u64),
    /// Moves `n` of the stack in `from` to `to` (ADR 0074): each a holder and its slot (`items::Holder`; creative's
    /// grid takes an item id as its slot, the ground ahead none). The server checks reach, mode and the slots'
    /// rules and applies it whole or not at all (`items::shift`), then answers `Done` with the same `seq` and the
    /// slots as they now are; the client predicts it with the same rule.
    Items { seq: u32, from: (crate::items::Holder, u16), to: (crate::items::Holder, u16), n: u16 },
    /// Takes the pick-up lying in air cell `at` into the slots (ADR 0082): the server checks reach, that one lies
    /// there (`clutter::pickup_at`), was not taken and fits, then answers `Done` with the same `seq`.
    Take { seq: u32, at: crate::world::Block },
    /// The hotbar slot in hand, 0..8 (ADR 0110): the tool a survival dig uses (`items::tool`) and the stone a
    /// `Strike` knaps. Sent on join and on every change of slot; the server takes slot 0 until it comes.
    Hand(u8),
    /// The place button on block `at` with a knapping stone in hand, in survival (ADR 0110): on hard rock
    /// (`items::knaps`) the server counts the strikes, at most one every `items::KNAP_WAIT_S`, and after the item's
    /// `knap` count one of it becomes the item it knaps into. Nothing is answered but the slots.
    /// With a prospecting tool in hand (`items::Tool::prospects`) it is a prospecting strike instead: at most one
    /// every `items::PROSPECT_WAIT_S`, a use of the tool, answered with `ToClient::Prospect`.
    Strike(Block),
    /// `Steps` with each input's sprint bit (ADR 0118), which `Input`'s own bytes leave out so that recordings
    /// from before still read. `ToServer::steps` picks this or `Steps`; `unpacked` makes it a `Steps` again.
    Sprints { first: u32, inputs: Vec<(Input, bool)> },
    /// `Input` with its sprint bit (ADR 0118), for scripts: `ToServer::input` and `unpacked` as above.
    Sprint(Input, bool),
    /// Takes the crafting grid's output up to `n` times (ADR 0140; ADR 0139's make-by-name is gone):
    /// alive, either mode, the grid's cells (`items::CRAFT_AT`) holding the recipe of this name, the 3 x 3 only at a
    /// workbench (`Use`). Each make takes one item from every full cell and puts the output into the slots, all or
    /// nothing (`recipes::make`); it stops when the cells no longer hold it or the output has no room. The name
    /// guards against a grid that changed since the client drew its output. Answered by the slots (`Inventory`).
    Craft { recipe: String, n: u16 },
    /// The place button on block `at` that opens a window (`blocks::Def::window`, ADR 0140): within
    /// `items::BENCH_REACH` and alive, the server opens it for this player (a workbench: the grid's 3 x 3 counts
    /// while it stays within reach). Nothing is answered.
    Use(Block),
    /// The inventory screen closed: the grid's cells go back into the slots (what does not fit at the feet) and a
    /// window opened by `Use` closes.
    Close,
    /// The cast the furnace at `at` works its ingots into (a `cast` row's name), or none (ADR 0141):
    /// alive, within `items::BENCH_REACH`, a furnace there and a cast it reaches. Answered by its `Furnace`.
    Choose { at: Block, cast: Option<String> },
    /// The use button held on the water in cell `at`, or let go (none; ADR 0145): while it is held,
    /// alive, in survival, nothing in hand, within `items::BENCH_REACH` and the water there fresh (`World::salt`,
    /// judged when the cell comes), each of the player's physics steps drinks (`Vitals::drink`). Sent on a
    /// change, as `Dig`; death lets go.
    Drink(Option<Block>),
}

impl ToServer {
    /// The predicted steps on the wire: `Sprints` when one of them sprints, else the old `Steps`.
    pub fn steps(first: u32, inputs: Vec<Input>) -> ToServer {
        if inputs.iter().any(|i| i.sprint) {
            ToServer::Sprints { first, inputs: inputs.into_iter().map(|i| (i, i.sprint)).collect() }
        } else {
            ToServer::Steps { first, inputs }
        }
    }
    /// A script's input on the wire: `Sprint` when it sprints, else the old `Input`.
    pub fn input(i: Input) -> ToServer {
        if i.sprint { ToServer::Sprint(i, true) } else { ToServer::Input(i) }
    }
    /// The sprint variants as `Steps` and `Input` with the bit set in each input; anything else as it is.
    pub fn unpacked(self) -> ToServer {
        match self {
            ToServer::Sprints { first, inputs } => ToServer::Steps { first, inputs: inputs.into_iter().map(|(i, sprint)| Input { sprint, ..i }).collect() },
            ToServer::Sprint(i, sprint) => ToServer::Input(Input { sprint, ..i }),
            m => m,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ToClient {
    /// First message: the world is rebuilt from these numbers, then `Edits` brings what changed. `blocks` is
    /// the block registry's hash (`BLOCKS.hash`): a client from a build with other blocks refuses to play.
    /// `bodies`: every body of `data/bodies.toml` in row order (0163-n-bodies) as its columns per face edge
    /// and the hash of the baked map it stands on (ADR 0043, 0070), None for the seed's noise; a client that
    /// lacks a map asks for it (`fetch_map`). `items`: the item
    /// registry's hash (`ITEMS.hash`, ADR 0074), refused the same way as `blocks`. `gen`: the world's generation (`Body::gen`; from 1 the planet's grid
    /// goes down through the mantle, ADR 0152), which the client builds its world of (`World::of`).
    Welcome { id: u32, seed: i32, bodies: Vec<(i32, Option<u64>)>, blocks: u64, items: u64, gen: u8 },
    /// The block and item files' text in force on the server (`live::apply`, ADR 0134): to everyone when the
    /// server reloads them, and before `Welcome` to a client that joins a server whose files are not the build's.
    Defs { blocks: String, items: String },
    /// A piece of the baked map's bytes (`Map::to_bytes`), in order, `pieces` in all.
    Map { piece: u32, pieces: u32, bytes: Vec<u8> },
    Edits(Vec<(Block, u16)>),
    Players(Vec<Player>),
    /// The answer to the `Edit` with this `seq`: applied, or refused whole (reach, size, inventory).
    Done { seq: u32, ok: bool },
    /// This player's whole physics state after the step numbered `step` of its `Steps` (0: none yet), every
    /// tick, and the world's time that poses the sky (ADR 0022). The client replays its later inputs from here
    /// (ADR 0019).
    /// `hunger`: the server's, exactly, after that step (ADR 0118): the client predicts it across its later steps
    /// with the same drain, so it refuses a sprint at the step the server will.
    You { step: u32, me: crate::phys::Player, time: f64, warp: f64, hunger: f64 },
    /// This player's mode, its slots (`items::SLOTS`: the hotbar, the worn pack, its rows, the grid) and the water in
    /// its bucket (units, `liquid::U` a block); sent on join, on change and after every `Items`. shortcut: the
    /// whole list (under 40 slots, a few hundred bytes), not slot deltas, as it changes a few times a second.
    Inventory { creative: bool, slots: Vec<crate::items::Slot>, water: u32 },
    /// Piles that changed, each whole (none: it is gone); every pile on join (ADR 0074). shortcut: every pile
    /// goes to every client; by region, as entities, when littered faces make the list long.
    Piles(Vec<(Block, Vec<crate::items::Slot>)>),
    /// Tiles of water (ADR 0020), each whole: the tile (`Col::tile`) and its stored columns, (floor, ceil,
    /// units) per gap; a column not listed is as generation gives it. A tile is sent when it changed by more
    /// than the dead band, or settled, since this client last had it: nearest first, within reach and a byte
    /// budget per tick.
    Water(Vec<(crate::liquid::Col, Vec<crate::liquid::ColWater>)>),
    /// A line for the chat (ADR 0023): a player's words, an answer to a command, someone joining or leaving.
    Chat(String),
    /// This player's health, hunger, thirst and breath, 0 to `vitals::MAX` (ADR 0040, 0145, draft breath; the
    /// HUD shows breath only under MAX), and how it died while it is dead; sent on join and when what the HUD
    /// shows changes.
    Vitals { health: u8, hunger: u8, thirst: u8, breath: u8, dead: Option<String> },
    /// Another player's dig (ADR 0034): the shape, the share of it done when this was sent and the share it
    /// gains a second (the client runs the cracks on from those), or none when it stopped or finished.
    Digging { id: u32, at: Option<(Shape, f32, f32)> },
    /// The live air where this player stands (W3, `weather::Weather::here`), every `AIR_EVERY` ticks on a
    /// planet whose map keeps a climate.
    Air(crate::weather::Here),
    /// The creature kinds by index (rows of `data/creatures.toml`), once on join (ADR 0083).
    Kinds(Vec<String>),
    /// The animals near this player that are new to it or changed, and those it no longer sees (ADR 0083).
    Animals { seen: Vec<Animal>, gone: Vec<u32> },
    /// Pick-ups taken (ADR 0082): the cell each lay in and the year (`clutter::year`); every one on join.
    /// shortcut: every mark to every client, as the piles.
    Taken(Vec<(crate::world::Block, u32)>),
    /// The planet's air as the client draws it (ADR 0076): every column (`all`, on join) or the columns that
    /// changed (by index), when the grid lands a step at world time `t` or a forced weather changes.
    Sky { t: f64, all: Vec<crate::weather::Cell>, changed: Vec<(u32, crate::weather::Cell)> },
    /// The answer to this player's prospecting `Strike` (ADR 0138): each element
    /// heard and its band (`ore::BANDS`), strongest first (`ore::prospect`); empty: the rock rings empty.
    Prospect(Vec<(String, u8)>),
    /// The furnace this player has open (`Use`, ADR 0141) as it is now: its slots, heat, fire, work and
    /// choice; sent on opening and on every tick it changes while open.
    Furnace { at: Block, f: crate::furnace::Furnace },
    /// Furnaces whose fire was lit (true) or went out, to everyone; every burning one on join: the model's
    /// `working` state (its fire, smoke, sound and light) plays while it burns.
    Fires(Vec<(Block, bool)>),
    /// This player's two marks from the game (needs.md "What death costs"): the cell of the pile its last death
    /// left, until that pile is empty, and its sleeping place, where a respawn wakes it; sent when they change.
    Marks { death: Option<Block>, home: Option<Block> },
    /// What happened this tick that this player can perceive (`events`, ADR 0149), in the tick's
    /// order; sent before the tick's `Edits`, so the client knows which of their cells were digs and falls.
    Events(Vec<crate::events::Event>),
    /// This player's journal entries that changed state (`journal`, draft ADR journal-data), by name; once
    /// it has its name (`Hello`), every entry not unknown, then each change at the end of its tick.
    Journal(Vec<(String, crate::journal::State)>),
}

/// An animal as a client sees it (ADR 0083): `kind` indexes `ToClient::Kinds`, `pos` its feet in body 0's frame,
/// `act` its `herds::Mind` as a number, `call` counts its calls (the voice plays when it changes).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Animal {
    pub id: u32,
    pub kind: u16,
    pub pos: [f64; 3],
    pub fwd: [f32; 3],
    pub act: u8,
    pub call: u8,
}

/// A client is told of the animals this near (m).
pub const SEE: f64 = 80.0;

/// Ticks between the `ToClient::Air` a player is sent (2 s).
pub const AIR_EVERY: u64 = 40;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Player {
    pub id: u32,
    pub name: String,
    /// The feet (bottom centre of the player's box).
    pub pos: [f64; 3],
    pub fwd: [f32; 3],
    pub pitch: f32,
    /// The item in its hand (its selected slot's; 0 none), sent with the roster once a second (ADR 0117): a
    /// client lights the world from another player's held torch or lamp by it.
    pub hand: u16,
}

/// The download's stage name in `bake::Progress`.
pub const DOWNLOAD: &str = "downloading the map";

/// The baked map as `ToClient::Map` pieces of a MiB each.
pub fn map_pieces(map: &crate::bake::Map) -> Vec<ToClient> {
    let bytes = map.to_bytes();
    let pieces = bytes.len().div_ceil(1 << 20) as u32;
    bytes.chunks(1 << 20).enumerate().map(|(k, b)| ToClient::Map { piece: k as u32, pieces, bytes: b.to_vec() }).collect()
}

/// The baked map `Welcome` named (`n` its body's columns): from memory or the disk, else asked of the server
/// and kept. What the server sends meanwhile is put back ahead of the rest, so the client reads it as if nothing
/// happened.
pub fn fetch_map(link: &mut Link<ToServer, ToClient>, seed: i32, n: i32, hash: u64) -> Result<std::sync::Arc<crate::bake::Map>, String> {
    if let Some(m) = crate::bake::find(seed, n, Some(hash)) {
        return Ok(m);
    }
    link.send(ToServer::Map(hash));
    let (mut bytes, mut held) = (Vec::new(), Vec::new());
    loop {
        match link.rx.recv_timeout(std::time::Duration::from_secs(120)) {
            Ok(ToClient::Map { piece, pieces, bytes: b }) => {
                bytes.extend_from_slice(&b);
                crate::bake::report(DOWNLOAD, (piece + 1) as f32 / pieces as f32);
                if piece + 1 >= pieces {
                    break;
                }
            }
            // held no further than the link's inbound bound (`Limits::in_max`), as the link itself
            Ok(_) if held.len() >= Limits::TCP.in_max => return Err(format!("over {} messages came while the map did", Limits::TCP.in_max)),
            Ok(m) => held.push(m),
            Err(e) => return Err(format!("the server's map did not come: {e}")),
        }
    }
    let map = crate::bake::Map::from_bytes(&bytes)?;
    if map.hash != hash {
        return Err("the server's map is not the one it named".into());
    }
    // the link's bound kept: the new channel is bounded too, and the thread that fills it waits while it is
    // full, so the old one fills behind it and a TCP link closes as it would have. No deadlock: only that
    // thread waits, on the reader of `link.rx`, and its send fails once the link is dropped
    let (tx, rx) = std::sync::mpsc::sync_channel(Limits::TCP.in_max);
    let old = std::mem::replace(&mut link.rx, rx);
    std::thread::spawn(move || {
        for m in held {
            if tx.send(m).is_err() {
                return;
            }
        }
        for m in old {
            if tx.send(m).is_err() {
                break;
            }
        }
    });
    Ok(crate::bake::keep(map))
}

/// True when every cell (and edit's block id) a server message names exists in `world`: the client and the bot
/// index by them unchecked, and a bad one would abort them, so a message that fails is dropped whole.
pub fn fits(world: &crate::world::World, m: &ToClient) -> bool {
    let ok = |b: &Block| Shape::one(*b).fits(world);
    match m {
        ToClient::Edits(v) => v.iter().all(|(b, m)| ok(b) && crate::blocks::BLOCKS.get(*m).is_some()),
        ToClient::Taken(v) => v.iter().all(|(b, _)| ok(b)),
        ToClient::Piles(v) => v.iter().all(|(b, _)| ok(b)),
        ToClient::Fires(v) => v.iter().all(|(b, _)| ok(b)),
        ToClient::Marks { death, home } => death.iter().chain(home).all(ok),
        ToClient::Digging { at, .. } => at.as_ref().is_none_or(|(s, _, _)| s.fits(world)),
        ToClient::Furnace { at, .. } => ok(at),
        ToClient::Events(v) => v.iter().all(|e| e.cell().iter().chain(if let crate::events::Event::Fall { from, .. } = e { Some(from) } else { None }).all(ok)),
        _ => true,
    }
}

/// One end of a connection: `O` goes out, `I` comes in.
pub struct Link<O, I> {
    pub tx: Sender<O>,
    pub rx: Receiver<I>,
}

pub enum Recv<I> {
    Msg(I),
    Empty,
    Closed,
}

impl<O, I> Link<O, I> {
    /// Sending to a closed peer is not an error here; the next `try_recv` reports the close.
    pub fn send(&self, m: O) {
        let _ = self.tx.send(m);
    }
    pub fn try_recv(&self) -> Recv<I> {
        match self.rx.try_recv() {
            Ok(m) => Recv::Msg(m),
            Err(TryRecvError::Empty) => Recv::Empty,
            Err(TryRecvError::Disconnected) => Recv::Closed,
        }
    }
}

/// Two ends joined in memory: the local server's connection.
pub fn local<A, B>() -> (Link<A, B>, Link<B, A>) {
    let (a_tx, a_rx) = channel();
    let (b_tx, b_rx) = channel();
    (Link { tx: a_tx, rx: b_rx }, Link { tx: b_tx, rx: a_rx })
}

/// A TCP connection as a link: one thread writes outgoing messages, one reads incoming ones, as frames of a
/// little-endian u32 length and postcard bytes; a frame of length 0 is a ping, never a message. A link silent,
/// flooded or too slow past its `Limits` is closed. Either thread ending closes the link.
pub fn tcp<O, I>(stream: TcpStream) -> std::io::Result<Link<O, I>>
where
    O: Serialize + Send + 'static,
    I: DeserializeOwned + Send + 'static,
{
    tcp_with(stream, Limits::TCP)
}

/// A TCP link's times and queue bounds (tests use small ones).
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// An idle writer sends an empty frame this often, so the other side hears the link is alive.
    pub ping: std::time::Duration,
    /// A link that hears nothing this long (not even a ping) is closed: the peer slept or dropped off
    /// without closing the socket.
    pub silent: std::time::Duration,
    /// Messages read but not yet taken (`Link::rx`); one more closes the link: a flood, or a reader stuck.
    pub in_max: usize,
    /// Bytes waiting to be sent; more closes the link: a peer too slow, or one that never reads.
    pub out_max: usize,
}

impl Limits {
    /// The game's: a ping every 5 s and 30 s of silence; 4096 messages in (a walking client sends a few a tick
    /// and the server takes 64 a tick; the server sends a client 20-40 a second, so a client that stops
    /// reading for about two minutes); 128 MiB out (a join sends both maps, about 23 + 21 MB, and every edit,
    /// about 12 bytes each: at 1 Mbit/s the backlog stays near 60 MB).
    pub const TCP: Limits = Limits { ping: std::time::Duration::from_secs(5), silent: std::time::Duration::from_secs(30), in_max: 4096, out_max: 128 << 20 };
}

/// `tcp` with other `Limits`.
pub fn tcp_with<O, I>(stream: TcpStream, Limits { ping, silent, in_max, out_max }: Limits) -> std::io::Result<Link<O, I>>
where
    O: Serialize + Send + 'static,
    I: DeserializeOwned + Send + 'static,
{
    use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
    stream.set_nodelay(true)?;
    // std's SO_RCVTIMEO / SO_SNDTIMEO: the same on Windows and Linux, unlike keepalive's OS-tuned timings
    stream.set_read_timeout(Some(silent))?;
    stream.set_write_timeout(Some(silent))?;
    let mut w = stream.try_clone()?;
    let cut = stream.try_clone()?;
    let mut r = stream;
    let (out_tx, out_rx) = channel::<O>();
    let (in_tx, in_rx) = std::sync::mpsc::sync_channel::<I>(in_max);
    // the frame maker drops the sender to close the link at once: a shutdown does not wake a blocked read on
    // Windows, so the reader alone would notice only after `silent`
    let in_tx = std::sync::Arc::new(std::sync::Mutex::new(Some(in_tx)));
    let closer = in_tx.clone();
    // one thread makes the frames and counts their bytes, another writes them: the count of bytes waiting is
    // known while the writer is stuck on a slow peer
    let (frame_tx, frame_rx) = channel::<Vec<u8>>();
    let waiting = std::sync::Arc::new(AtomicUsize::new(0));
    let written = waiting.clone();
    std::thread::spawn(move || {
        loop {
            // an empty frame when idle: no message is empty (postcard writes the variant), so it says only "alive"
            let bytes = match out_rx.recv_timeout(ping) {
                Ok(m) => match postcard::to_stdvec(&m) {
                    Ok(b) => b,
                    Err(_) => break,
                },
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Vec::new(),
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            };
            if waiting.fetch_add(bytes.len() + 4, Relaxed) + bytes.len() + 4 > out_max {
                eprintln!("[net] over {} MiB waiting to be sent: the peer is too slow, the link is closed", out_max >> 20);
                let _ = cut.shutdown(std::net::Shutdown::Both);
                closer.lock().unwrap_or_else(|e| e.into_inner()).take();
                break;
            }
            if frame_tx.send(bytes).is_err() {
                break;
            }
        }
    });
    std::thread::spawn(move || {
        for bytes in frame_rx {
            if w.write_all(&(bytes.len() as u32).to_le_bytes()).and_then(|_| w.write_all(&bytes)).is_err() {
                break;
            }
            written.fetch_sub(bytes.len() + 4, Relaxed);
        }
        let _ = w.shutdown(std::net::Shutdown::Both);
    });
    std::thread::spawn(move || {
        let mut len = [0u8; 4];
        let mut buf = Vec::new();
        loop {
            if let Err(e) = r.read_exact(&mut len) {
                if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) {
                    eprintln!("[net] nothing heard for {:.1} s: the link is closed", silent.as_secs_f64());
                }
                break;
            }
            let n = u32::from_le_bytes(len) as usize;
            if n == 0 {
                continue; // a ping
            }
            if n > 1 << 24 {
                break; // nothing legitimate is 16 MB; a bad peer gets dropped
            }
            buf.resize(n, 0);
            if r.read_exact(&mut buf).is_err() {
                break;
            }
            match postcard::from_bytes::<I>(&buf) {
                Ok(m) => match in_tx.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|tx| tx.try_send(m)) {
                    Some(Ok(())) => {}
                    Some(Err(std::sync::mpsc::TrySendError::Full(_))) => {
                        eprintln!("[net] {in_max} messages not taken: a flood, or a reader stuck; the link is closed");
                        break;
                    }
                    Some(Err(std::sync::mpsc::TrySendError::Disconnected(_))) | None => break,
                },
                Err(_) => break,
            }
        }
        // the frame maker holds the sender too: dropping it here is what closes the link
        in_tx.lock().unwrap_or_else(|e| e.into_inner()).take();
        let _ = r.shutdown(std::net::Shutdown::Both);
    });
    Ok(Link { tx: out_tx, rx: in_rx })
}

/// The command line's completions (ADR 0023): where the word being typed starts in `line`, and what could
/// stand there. The server's `command` runs the commands; `commands_complete_to_known_words` there checks
/// that this list names only what it knows.
pub fn complete(line: &str) -> (usize, Vec<String>) {
    let Some(rest) = line.strip_prefix('/') else { return (line.len(), Vec::new()) };
    let words: Vec<String> = rest.split(' ').filter(|w| !w.is_empty()).map(str::to_lowercase).collect();
    let typed = if rest.ends_with(' ') || rest.is_empty() { 0 } else { 1 };
    let before = &words[..words.len() - typed];
    // a block's name has spaces: after /give and /place everything typed is one name
    let (at, options): (usize, Vec<String>) = match before.first().map(String::as_str) {
        Some("give" | "place") => {
            // the command is a whole word, so a space follows it
            let at = line.len() - rest[rest.find(' ').unwrap_or(0)..].trim_start().len();
            (at, crate::blocks::BLOCKS.placeable().map(|d| d.name.clone()).collect())
        }
        first => {
            // /goto takes every body's name and words (data/bodies.toml), and the sun
            let names: Vec<String> = crate::bodies::BODIES.iter().flat_map(|r| std::iter::once(r.name.to_lowercase()).chain(r.words.clone())).collect();
            let places: Vec<&str> = names.iter().map(String::as_str).chain(["sun"]).collect();
            let words: &[&str] = match (first, before.get(1).map(String::as_str), before.len()) {
                (None, _, _) => &["time", "playmode", "give", "place", "goto", "timewarp", "heal", "feed", "thirst", "hurt", "help"],
                (Some("timewarp" | "warp"), None, 1) => &["1", "10", "100", "1000", "10000", "up", "down"],
                (Some("time"), None, 1) => &["set", "add"],
                (Some("time"), Some("set"), 2) => &["sunrise", "day", "noon", "sunset", "night", "midnight", "spring", "summer", "autumn", "winter"],
                (Some("playmode" | "gamemode"), None, 1) => &["creative", "survival"],
                (Some("goto"), None, 1) => &places,
                _ => &[],
            };
            (line.len() - rest.rsplit(' ').next().unwrap_or("").len(), words.iter().map(|w| w.to_string()).collect())
        }
    };
    let part = line[at..].to_lowercase();
    // a word typed out in full needs no list
    (at, options.into_iter().filter(|o| o.to_lowercase().starts_with(&part) && o.to_lowercase() != part).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::IVec3;

    /// The sprint bit (ADR 0118) survives the bytes, and an input in the bytes from before reads as not sprinting.
    #[test]
    fn the_sprint_bit_rides_the_wire_and_old_inputs_walk() {
        let walk = Input { mv: [1.0, 0.25], jump: true, fwd: [1.0, 0.0, 0.0], pitch: 0.3, fly: 0.0, t: 12.5, ..Default::default() };
        let inputs = vec![walk, Input { sprint: true, ..walk }, walk];
        let bytes = |m: &ToServer| postcard::to_stdvec(m).unwrap();
        let back = |m: &ToServer| postcard::from_bytes::<ToServer>(&bytes(m)).unwrap().unpacked();
        assert_eq!(back(&ToServer::steps(7, inputs.clone())), ToServer::Steps { first: 7, inputs: inputs.clone() });
        assert!(matches!(ToServer::steps(7, vec![walk]), ToServer::Steps { .. }), "no sprint: the old message");
        assert_eq!(back(&ToServer::input(inputs[1])), ToServer::Input(inputs[1]));
        // an `Input` as it was written before the bit: the same fields in the same order
        let old = (walk.mv, walk.jump, walk.fwd, walk.pitch, walk.turn, walk.tilt, walk.roll, walk.fly, walk.down, walk.t);
        let read: Input = postcard::from_bytes(&postcard::to_stdvec(&old).unwrap()).unwrap();
        assert_eq!((read, read.sprint), (walk, false));
        assert_eq!(postcard::to_stdvec(&inputs[1]).unwrap(), postcard::to_stdvec(&old).unwrap(), "the bit is not in the old bytes");
    }

    #[test]
    fn tcp_round_trip() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        let client: Link<ToServer, ToClient> = tcp(TcpStream::connect(addr).unwrap()).unwrap();
        let server: Link<ToClient, ToServer> = tcp(l.accept().unwrap().0).unwrap();
        let e = ToServer::Edit { seq: 4, shape: Shape::one(Block { body: 0, face: 3, pos: IVec3::new(5, -7, 9) }), material: 2 };
        client.send(ToServer::Hello { name: "bot".into() });
        client.send(e.clone());
        assert_eq!(server.rx.recv().unwrap(), ToServer::Hello { name: "bot".into() });
        assert_eq!(server.rx.recv().unwrap(), e);
        server.send(ToClient::Welcome { id: 1, seed: 7, bodies: vec![(1536, Some(42)), (416, Some(43)), (128, None)], blocks: 9, items: 8, gen: 2 });
        assert_eq!(client.rx.recv().unwrap(), ToClient::Welcome { id: 1, seed: 7, bodies: vec![(1536, Some(42)), (416, Some(43)), (128, None)], blocks: 9, items: 8, gen: 2 });
        drop(server);
        assert!(client.rx.recv().is_err(), "a dropped peer closes the link");
    }

    /// A peer that goes quiet without closing its socket (a laptop asleep) is dropped after the silence; two
    /// idle links keep each other alive with pings that never come out as messages.
    #[test]
    fn a_silent_peer_is_dropped_and_an_idle_one_is_not() {
        use std::time::{Duration, Instant};
        let silent = Duration::from_millis(600);
        let lim = Limits { ping: Duration::from_millis(100), silent, ..Limits::TCP };
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        // a raw socket that never writes: a ghost
        let _ghost = TcpStream::connect(addr).unwrap();
        // (the clock starts before the link does: its silence is counted from its own start)
        let t = Instant::now();
        let server: Link<ToClient, ToServer> = tcp_with(l.accept().unwrap().0, lim).unwrap();
        assert!(matches!(server.rx.recv_timeout(silent * 5), Err(std::sync::mpsc::RecvTimeoutError::Disconnected)), "the ghost's link closes");
        assert!(t.elapsed() >= silent && t.elapsed() < silent * 4, "{:?}", t.elapsed());
        // two of our own ends, both idle for three silences: still open, nothing delivered
        let client: Link<ToServer, ToClient> = tcp_with(TcpStream::connect(addr).unwrap(), lim).unwrap();
        let server: Link<ToClient, ToServer> = tcp_with(l.accept().unwrap().0, lim).unwrap();
        assert!(matches!(server.rx.recv_timeout(silent * 3), Err(std::sync::mpsc::RecvTimeoutError::Timeout)));
        assert!(matches!(client.rx.recv_timeout(Duration::from_millis(10)), Err(std::sync::mpsc::RecvTimeoutError::Timeout)));
        client.send(ToServer::Respawn);
        assert_eq!(server.rx.recv_timeout(silent).unwrap(), ToServer::Respawn);
    }

    /// A flood fills the messages waiting to be taken past `in_max`: the link closes, with what came before it.
    #[test]
    fn a_flood_closes_the_link() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        let client: Link<ToServer, ToClient> = tcp(TcpStream::connect(addr).unwrap()).unwrap();
        let server: Link<ToClient, ToServer> = tcp_with(l.accept().unwrap().0, Limits { in_max: 100, ..Limits::TCP }).unwrap();
        for _ in 0..150 {
            client.send(ToServer::Respawn);
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
        let mut got = 0;
        let end = loop {
            match server.rx.recv_timeout(std::time::Duration::from_secs(1)) {
                Ok(_) => got += 1,
                Err(e) => break e,
            }
        };
        assert_eq!((got, end), (100, std::sync::mpsc::RecvTimeoutError::Disconnected), "the link closed at the 101st, after the 100 before it");
    }

    /// A peer that never reads: the bytes waiting to go out pass `out_max` and the link closes, the sender's
    /// memory bounded.
    #[test]
    fn a_peer_that_never_reads_is_dropped() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        let _deaf = TcpStream::connect(addr).unwrap();
        let server: Link<ToClient, ToServer> = tcp_with(l.accept().unwrap().0, Limits { out_max: 8 << 20, ..Limits::TCP }).unwrap();
        // 64 MiB offered; the socket's buffers take a few MiB, the queue 8
        let piece = ToClient::Map { piece: 0, pieces: 1, bytes: vec![7; 1 << 20] };
        for _ in 0..64 {
            server.send(piece.clone());
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert!(matches!(server.rx.recv_timeout(std::time::Duration::from_secs(2)), Err(std::sync::mpsc::RecvTimeoutError::Disconnected)), "the link is closed");
    }

    /// A server message naming a body, face or cell the world lacks, or a block id the registry lacks, fails
    /// `fits`; one whose cells all exist passes.
    #[test]
    fn a_message_of_cells_the_world_lacks_does_not_fit() {
        let w = crate::world::World::new(1536, 64, 1337);
        let good = Block { body: 0, face: 3, pos: IVec3::new(5, 7, 9) };
        let bad = [Block { body: 9, ..good }, Block { face: 7, ..good }, Block { pos: IVec3::new(-1, 7, 9), ..good }, Block { pos: IVec3::new(5, 7, 1 << 20), ..good }];
        assert!(fits(&w, &ToClient::Edits(vec![(good, 1)])));
        assert!(fits(&w, &ToClient::Chat("hi".into())));
        assert!(!fits(&w, &ToClient::Edits(vec![(good, u16::MAX)])), "an unknown block id");
        for b in bad {
            for m in [
                ToClient::Edits(vec![(good, 1), (b, 1)]),
                ToClient::Taken(vec![(b, 1)]),
                ToClient::Piles(vec![(b, Vec::new())]),
                ToClient::Marks { death: None, home: Some(b) },
                ToClient::Fires(vec![(b, true)]),
                ToClient::Digging { id: 1, at: Some((Shape::one(b), 0.0, 0.0)) },
                ToClient::Events(vec![crate::events::Event::Fall { from: b, to: good, block: 1 }]),
            ] {
                assert!(!fits(&w, &m), "{m:?}");
            }
        }
    }

    /// A client that lacks the map gets it in pieces, and what the server sent meanwhile still comes, in order.
    #[test]
    fn a_missing_map_comes_in_pieces() {
        let cells = 6 * 384 * 384;
        let bytes = crate::bake::Map { version: crate::bake::VERSION, seed: -12345, n: 3072, m: 384, h: (0..cells).map(|c| c as f32).collect(), biome: vec![4; cells], rock: vec![0; cells], ..crate::bake::Map::empty() }.to_bytes();
        let map = crate::bake::Map::from_bytes(&bytes).unwrap();
        // a run before this one kept it: this client must not have it yet
        let _ = std::fs::remove_file(crate::bake::file(-12345, 3072));
        let (mut client, server) = local::<ToServer, ToClient>();
        let hash = map.hash;
        let t = std::thread::spawn(move || {
            assert_eq!(server.rx.recv().unwrap(), ToServer::Map(hash));
            server.send(ToClient::Chat("before".into()));
            let pieces = map_pieces(&map);
            assert!(pieces.len() > 2);
            for p in pieces {
                server.send(p);
            }
            server.send(ToClient::Chat("after".into()));
        });
        let got = fetch_map(&mut client, -12345, 3072, hash).unwrap();
        t.join().unwrap();
        assert_eq!((got.hash, got.h[1000]), (hash, 1000.0));
        assert_eq!(client.rx.recv().unwrap(), ToClient::Chat("before".into()));
        assert_eq!(client.rx.recv().unwrap(), ToClient::Chat("after".into()));
        assert!(crate::bake::find(-12345, 3072, Some(hash)).is_some(), "kept");
    }

    /// What comes during a download is held, but no more than the link's inbound bound: past it the download
    /// fails, as the link would have closed.
    #[test]
    fn a_download_holds_no_more_than_the_inbound_bound() {
        let cells = 6 * 384 * 384;
        let bytes = crate::bake::Map { version: crate::bake::VERSION, seed: -12347, n: 3072, m: 384, h: vec![2.0; cells], biome: vec![4; cells], rock: vec![0; cells], ..crate::bake::Map::empty() }.to_bytes();
        let map = crate::bake::Map::from_bytes(&bytes).unwrap();
        let _ = std::fs::remove_file(crate::bake::file(-12347, 3072));
        let (mut client, server) = local::<ToServer, ToClient>();
        let hash = map.hash;
        for _ in 0..Limits::TCP.in_max + 1 {
            server.send(ToClient::Chat("meanwhile".into()));
        }
        for p in map_pieces(&map) {
            server.send(p);
        }
        assert!(fetch_map(&mut client, -12347, 3072, hash).is_err());
    }

    /// The download reports each piece as it comes (received / total) and ends at 1.
    #[test]
    fn a_map_download_reports_its_pieces() {
        let cells = 6 * 384 * 384;
        let bytes = crate::bake::Map { version: crate::bake::VERSION, seed: -12346, n: 3072, m: 384, h: vec![1.0; cells], biome: vec![4; cells], rock: vec![0; cells], ..crate::bake::Map::empty() }.to_bytes();
        let map = crate::bake::Map::from_bytes(&bytes).unwrap();
        let _ = std::fs::remove_file(crate::bake::file(-12346, 3072));
        let (mut client, server) = local::<ToServer, ToClient>();
        let (hash, p) = (map.hash, std::sync::Arc::new(crate::bake::Progress::default()));
        let seen = p.clone();
        // the server sends every piece whatever it sees (no assert in its thread: a failed one would leave the
        // client waiting out its timeout), and what the client reported after each comes back to be checked here
        let t = std::thread::spawn(move || {
            let asked = server.rx.recv().ok();
            let pieces = map_pieces(&map);
            let n = pieces.len();
            let mut got = Vec::new();
            // one deadline for the whole download, so a wrong report fails in seconds
            let end = std::time::Instant::now() + std::time::Duration::from_secs(5);
            for (k, piece) in pieces.into_iter().enumerate() {
                server.send(piece);
                let want = (k + 1) as f32 / n as f32;
                while seen.get().1 < want && std::time::Instant::now() < end {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                got.push((seen.get(), (DOWNLOAD, want)));
            }
            (asked, got)
        });
        let fetched = crate::bake::reporting(p.clone(), || fetch_map(&mut client, -12346, 3072, hash));
        let (asked, got) = t.join().unwrap();
        assert_eq!(asked, Some(ToServer::Map(hash)));
        assert!(got.len() > 2);
        for (k, (seen, want)) in got.iter().enumerate() {
            assert_eq!(seen, want, "piece {k} of {}", got.len());
        }
        fetched.unwrap();
        assert_eq!(p.get().1, 1.0);
    }

    #[test]
    fn commands_complete() {
        let c = |l: &str| complete(l);
        assert_eq!(c("/"), (1, ["time", "playmode", "give", "place", "goto", "timewarp", "heal", "feed", "thirst", "hurt", "help"].map(String::from).to_vec()));
        assert_eq!(c("/h"), (1, ["heal", "hurt", "help"].map(String::from).to_vec()));
        assert_eq!(c("/g"), (1, vec!["give".to_string(), "goto".to_string()]));
        assert_eq!(c("/time "), (6, vec!["set".to_string(), "add".to_string()]));
        assert_eq!(c("/time set N"), (10, vec!["noon".to_string(), "night".to_string()]));
        assert_eq!(c("/PlayMode c"), (10, vec!["creative".to_string()]));
        assert_eq!(c("/goto luna "), (11, vec![]));
        let s = |v: &[&str]| v.iter().map(|w| w.to_string()).collect::<Vec<_>>();
        assert_eq!(c("/goto "), (6, s(&["terra", "home", "spawn", "luna", "mars", "jupiter", "callisto", "sun"])));
        assert_eq!(c("/goto j"), (6, s(&["jupiter"])));
        assert_eq!(c("/goto c"), (6, s(&["callisto"])));
        assert_eq!(c("/goto m"), (6, s(&["mars"])));
        assert_eq!(c("/goto L"), (6, s(&["luna"])));
        assert_eq!(c("/goto t"), (6, s(&["terra"])));
        assert_eq!(c("hello"), (5, vec![]));
        // a block's name is one word for this, spaces and all
        let (at, v) = c("/give blue l");
        assert_eq!((at, v), (6, vec!["blue lamp".to_string()]));
        assert_eq!(c("/give stone 5").1, Vec::<String>::new());
        assert_eq!(c("/place blue l"), (7, vec!["blue lamp".to_string()]));
        assert_eq!(c("/p"), (1, vec!["playmode".to_string(), "place".to_string()]));
        // the completion put in place completes nothing more
        let (at, v) = c("/time set no");
        assert_eq!(format!("{}{}", &"/time set no"[..at], v[0]), "/time set noon");
    }
}
