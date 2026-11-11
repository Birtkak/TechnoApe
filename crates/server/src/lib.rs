//! The authoritative world (ADR 0006): one `Server` owns the `World`, ticks at `TICK_HZ`, applies the edits
//! players send and tells everyone what changed. Connections arrive on a channel, so the same server runs
//! inside the singleplayer client (an in-memory link) and as the dedicated binary (TCP links).
//!
//! The flight recorder (ADR 0011): with `record`, every join, message and leave goes to a file with its tick,
//! and every second a hash of the state; `replay` runs the file again headless and finds the first checkpoint
//! where the state parts from the recording.
pub mod save;
pub mod soak;

use engine::net::{local, Link, Player, Recv, ToClient, ToServer, REACH, TICK_HZ};
use engine::blocks::{AIR, BLOCKS};
use engine::edit::{self, Shape};
use engine::phys::{self, Input};
use engine::liquid::{Col, TILE, U};
use engine::world::{fnv, Block, FxMap, World};
use engine::sky;
use glam::DVec3;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use save::{Save, Saved};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

/// Hash checkpoints in a recording: one a second.
const CHECK_EVERY: u64 = 20;

/// One record of the flight recorder. Join, Msg and Leave carry the tick that took them in; Check the number
/// of ticks done when the hash was taken.
#[derive(Serialize, Deserialize, Debug)]
pub enum Ev {
    Start { planet_n: i32, moon_n: i32, seed: i32, dt: f64 },
    /// Right after `Start` when the server began from a save: the save itself, so a replay starts there too.
    Loaded(Save),
    Join { tick: u64, id: u32 },
    Msg { tick: u64, id: u32, m: ToServer },
    Leave { tick: u64, id: u32 },
    Check { tick: u64, hash: u64 },
}

pub type ServerLink = Link<ToClient, ToServer>;

/// `Steps` a player may have waiting (one second), and steps it may take in one tick after a late batch.
const QUEUE_MAX: usize = 60;
const CREDIT_MAX: u32 = 12;

/// Water steps (1/60 s each, at one node) a tick may spend: about 7 ms. Past it the water slows down instead of
/// the server falling behind (ADR 0020).
const WATER_BUDGET: usize = 60_000;
/// Water tiles go to a client within this distance (m), nearest first, about this many bytes a tick.
/// shortcut: far water is not sent, so a flood out of reach is not seen from orbit; a coarse far copy when
/// views from orbit show water.
const WATER_RANGE: f64 = 128.0;
const WATER_BYTES: usize = 16 * 1024;

/// How far /goto sun stands from the sun's centre, in sun radii: it fills about 23 degrees.
const SUN_VIEW: f64 = 5.0;

/// The longest chat line the server takes, in characters.
const CHAT_MAX: usize = 256;
/// The fastest time warp: the 54-day year in about 6 s.
const MAX_WARP: u64 = 10_000;

/// What a save keeps of a connected player.
fn saved(c: &Client) -> Saved {
    let inv = c.inv.iter().enumerate().filter(|(_, n)| **n > 0).map(|(m, n)| (m as u8, *n)).collect();
    Saved { name: c.player.name.clone(), pos: c.phys.pos.to_array(), fwd: c.phys.fwd.as_vec3().to_array(), creative: c.creative, inv, water: c.water }
}

struct Client {
    link: ServerLink,
    player: Player,
    phys: phys::Player,
    /// Walking input; `None` while the player flies with `Move` or walks by `Steps`.
    input: Option<Input>,
    /// `Steps` not taken yet, by number; the number of the last one taken; how many steps it may take now.
    queue: VecDeque<(u32, Input)>,
    acked: u32,
    credit: u32,
    /// Creative counts nothing; survival digs into `inv` and places from it (ADR 0014).
    creative: bool,
    /// How many of each block kind the player holds, by id.
    inv: Box<[u32; 256]>,
    /// The water in its bucket, in units (`liquid::U` is a full bucket).
    water: u32,
    /// The version of each water tile this client has (`Server::tiles`).
    tiles: FxMap<Col, u64>,
    /// The answers to this tick's edits, sent after its `Edits`.
    done: Vec<(u32, bool)>,
    /// The inventory changed this tick (or the player just joined).
    inv_dirty: bool,
}

pub struct Server {
    pub world: World,
    seed: i32,
    accept: Receiver<ServerLink>,
    clients: Vec<Client>,
    next_id: u32,
    pub tick: u64,
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
    /// Water creative put in and took out, in units: what flowing never changes (`water_ledger`).
    made: i64,
    removed: i64,
    /// Water steps skipped to keep to WATER_BUDGET, and bytes of water sent, since the start.
    pub water_skipped: u64,
    pub water_bytes: u64,
    /// Every water tile that ever changed: the tick it last changed (its version) and its centre.
    tiles: FxMap<Col, (u64, DVec3)>,
    /// When set, `run` adds every tick's length in ms (ADR 0025, S: the benchmark line's tick numbers).
    pub tick_ms: Option<std::sync::Arc<std::sync::Mutex<Vec<f32>>>>,
}

impl Server {
    pub fn new(planet_n: i32, moon_n: i32, seed: i32, accept: Receiver<ServerLink>) -> Server {
        Server {
            world: World::new(planet_n, moon_n, seed),
            seed,
            accept,
            clients: Vec::new(),
            next_id: 1,
            tick: 0,
            tick0: 0,
            warp: 1,
            dt: 1.0 / TICK_HZ,
            debt: 0.0,
            rec: None,
            absent: BTreeMap::new(),
            loaded: None,
            autosave: None,
            made: 0,
            removed: 0,
            water_skipped: 0,
            water_bytes: 0,
            tiles: FxMap::default(),
            tick_ms: None,
        }
    }

    /// A server that carries on from `save` (its seed and planet win over any given elsewhere).
    pub fn from_save(save: Save, accept: Receiver<ServerLink>) -> Server {
        let mut s = Server::new(save.planet_n, save.moon_n, save.seed, accept);
        for (b, m) in &save.edits {
            s.world.set_block(*b, *m);
        }
        let World { bodies, edits, water, .. } = &mut s.world;
        water.restore(&engine::world::Solid { bodies, edits }, &save.water);
        (s.made, s.removed, water.sea_extra) = (save.made, save.removed, save.sea_extra);
        s.tick0 = save.tick;
        let cols: Vec<Col> = save.water.iter().map(|w| w.0).collect();
        s.water_changed(&cols);
        s.absent = save.players.iter().map(|p| (p.name.clone(), p.clone())).collect();
        s.loaded = Some(save);
        s
    }

    /// Everything a save holds, from the current state.
    pub fn snapshot(&self) -> Save {
        let mut edits: Vec<(Block, u8)> =
            self.world.edits.iter().flat_map(|(k, cells)| cells.iter().map(move |(pos, m)| (Block { body: k.body, face: k.face, pos: *pos }, *m))).collect();
        edits.sort_unstable_by_key(|(b, _)| (b.body, b.face, b.pos.x, b.pos.y, b.pos.z));
        let mut players = self.absent.clone();
        for c in self.clients.iter().filter(|c| !c.player.name.is_empty()) {
            players.insert(c.player.name.clone(), saved(c));
        }
        let [p, m] = [self.world.bodies[0].n, self.world.bodies[1].n];
        Save {
            version: save::VERSION,
            seed: self.seed,
            planet_n: p,
            moon_n: m,
            blocks: save::registry(),
            edits,
            players: players.into_values().collect(),
            water: self.world.water.all(),
            made: self.made,
            removed: self.removed,
            sea_extra: self.world.water.sea_extra,
            tick: self.tick0.wrapping_add(self.tick),
        }
    }

    pub fn players(&self) -> usize {
        self.clients.len()
    }

    /// Every player as JSON: id, feet and whether it walks; a replay's dump.
    pub fn positions(&self) -> String {
        let v: Vec<String> = self
            .clients
            .iter()
            .map(|c| format!("{{\"id\":{},\"pos\":[{:.3},{:.3},{:.3}],\"walking\":{}}}", c.player.id, c.phys.pos.x, c.phys.pos.y, c.phys.pos.z, c.input.is_some()))
            .collect();
        format!("[{}]", v.join(","))
    }

    /// Records this session to `path` from the first tick on (the flight recorder).
    pub fn record(&mut self, path: &Path) -> std::io::Result<()> {
        if self.tick != 0 {
            return Err(std::io::Error::other("record before the first tick"));
        }
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        self.rec = Some(BufWriter::new(std::fs::File::create(path)?));
        Ok(())
    }

    fn log(&mut self, ev: Ev) {
        let Some(w) = self.rec.as_mut() else { return };
        let b = postcard::to_stdvec(&ev).expect("events serialize");
        if let Err(e) = w.write_all(&(b.len() as u32).to_le_bytes()).and_then(|_| w.write_all(&b)) {
            eprintln!("[technoape-server] recording stopped: {e}");
            self.rec = None;
        }
    }

    /// Conservation (ADR 0020): water that differs from generation plus water in buckets, which only creative
    /// changes. Zero when no unit appeared or vanished.
    pub fn water_ledger(&self) -> i64 {
        let held: i64 = self.clients.iter().map(|c| c.water as i64).chain(self.absent.values().map(|p| p.water as i64)).sum();
        self.world.water.counted() + held - (self.made - self.removed)
    }

    /// The state as one number: the world's edits and water, every player's physics and input, the time
    /// carried over.
    pub fn hash(&self) -> u64 {
        let mut h = fnv(fnv(self.world.hash(), &self.tick.to_le_bytes()), &self.debt.to_bits().to_le_bytes());
        h = fnv(h, &postcard::to_allocvec(&(self.world.water.all(), self.made, self.removed, self.world.water.sea_extra)).unwrap());
        for p in self.absent.values() {
            h = fnv(h, &postcard::to_allocvec(p).unwrap());
        }
        for c in &self.clients {
            let p = &c.phys;
            h = fnv(h, &c.player.id.to_le_bytes());
            for v in [p.pos, p.vel, p.fwd, p.up] {
                for x in v.to_array() {
                    h = fnv(h, &x.to_bits().to_le_bytes());
                }
            }
            h = fnv(h, &[p.on_ground as u8, c.input.is_some() as u8, c.creative as u8]);
            h = fnv(h, &c.water.to_le_bytes());
            for n in c.inv.iter() {
                h = fnv(h, &n.to_le_bytes());
            }
            for x in c.player.pos {
                h = fnv(h, &x.to_bits().to_le_bytes());
            }
            if let Some(i) = c.input {
                for x in [i.mv[0], i.mv[1], i.fwd[0], i.fwd[1], i.fwd[2], i.pitch, i.turn, i.tilt, i.roll] {
                    h = fnv(h, &x.to_bits().to_le_bytes());
                }
                h = fnv(h, &[i.jump as u8]);
            }
        }
        h
    }

    /// Ticks forever at `hz`, sleeping between ticks; the thread or process ends it.
    /// Ticks at `hz` until `stop` is set, then saves (if `autosave` says where) and returns.
    pub fn run(mut self, hz: f64, stop: &std::sync::atomic::AtomicBool) {
        self.dt = 1.0 / hz;
        let step = Duration::from_secs_f64(1.0 / hz);
        let mut next = Instant::now();
        while !stop.load(std::sync::atomic::Ordering::Relaxed) {
            let t = Instant::now();
            self.tick();
            if let Some(v) = &self.tick_ms {
                v.lock().unwrap().push(t.elapsed().as_secs_f32() * 1000.0);
            }
            next += step;
            match next.checked_duration_since(Instant::now()) {
                Some(d) => std::thread::sleep(d),
                None => next = Instant::now(), // a late tick does not try to catch up
            }
        }
        self.save_now();
    }

    /// Saves where `autosave` says, on this thread: for shutting down.
    pub fn save_now(&self) {
        if let Some((path, _)) = &self.autosave {
            match self.snapshot().write(path) {
                Ok(()) => eprintln!("[technoape-server] saved to {}", path.display()),
                Err(e) => eprintln!("[technoape-server] save failed: {e}"),
            }
        }
    }

    pub fn tick(&mut self) {
        // the sky's clock: the world's age, which a save carries on (ADR 0022)
        self.tick0 = self.tick0.wrapping_add(self.warp - 1);
        self.world.time = self.tick0.wrapping_add(self.tick) as f64 / TICK_HZ;
        if self.tick == 0 {
            let [b0, b1] = [&self.world.bodies[0], &self.world.bodies[1]];
            let start = Ev::Start { planet_n: b0.n, moon_n: b1.n, seed: self.seed, dt: self.dt };
            self.log(start);
            if let Some(save) = self.loaded.take() {
                self.log(Ev::Loaded(save));
            }
        }
        while let Ok(link) = self.accept.try_recv() {
            let id = self.next_id;
            self.next_id += 1;
            self.log(Ev::Join { tick: self.tick, id });
            let [p, m] = [self.world.bodies[0].n, self.world.bodies[1].n];
            link.send(ToClient::Welcome { id, seed: self.seed, planet_n: p, moon_n: m, blocks: BLOCKS.hash });
            let all: Vec<(Block, u8)> = self
                .world
                .edits
                .iter()
                .flat_map(|(k, cells)| cells.iter().map(move |(pos, m)| (Block { body: k.body, face: k.face, pos: *pos }, *m)))
                .collect();
            if !all.is_empty() {
                link.send(ToClient::Edits(all));
            }
            let spawn = self.spawn();
            self.clients.push(Client {
                link,
                player: Player { id, name: String::new(), pos: spawn.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 },
                phys: phys::Player::at(spawn, DVec3::X),
                input: None,
                queue: VecDeque::new(),
                acked: 0,
                credit: 0,
                creative: false,
                inv: Box::new([0; 256]),
                water: 0,
                tiles: FxMap::default(),
                done: Vec::new(),
                inv_dirty: true,
            });
        }
        let mut changed: Vec<(Block, u8)> = Vec::new();
        let mut i = 0;
        while i < self.clients.len() {
            let mut open = true;
            loop {
                let r = self.clients[i].link.try_recv();
                let (tick, id) = (self.tick, self.clients[i].player.id);
                match &r {
                    Recv::Msg(m) => self.log(Ev::Msg { tick, id, m: m.clone() }),
                    Recv::Closed => self.log(Ev::Leave { tick, id }),
                    Recv::Empty => {}
                }
                match r {
                    Recv::Msg(ToServer::Hello { name }) => {
                        let c = &mut self.clients[i];
                        c.player.name = name.chars().take(32).collect();
                        // a player the save remembers carries on where it left, with what it held
                        if let Some(sv) = self.absent.remove(&c.player.name) {
                            let p = DVec3::from_array(sv.pos);
                            c.phys = phys::Player::at(p, phys::tangent(DVec3::from_array(sv.fwd.map(f64::from)), phys::up(&self.world, p), DVec3::X));
                            (c.player.pos, c.player.fwd, c.creative, c.inv_dirty, c.water) = (sv.pos, sv.fwd, sv.creative, true, sv.water);
                            c.inv.fill(0);
                            for (m, n) in sv.inv {
                                c.inv[m as usize] = n;
                            }
                        }
                        let line = format!("{} joined", self.clients[i].player.name);
                        self.say(line);
                    }
                    Recv::Msg(ToServer::Move { pos, fwd, pitch }) => {
                        if pos.iter().all(|c| c.is_finite()) && fwd.iter().all(|c| c.is_finite()) {
                            self.put(i, DVec3::from_array(pos), DVec3::from_array(fwd.map(f64::from)));
                            self.clients[i].player.pitch = pitch;
                        }
                    }
                    Recv::Msg(ToServer::Input(inp)) => {
                        if phys::sane(&inp) {
                            self.clients[i].input = Some(inp);
                        }
                    }
                    Recv::Msg(ToServer::Steps { first, inputs }) => {
                        let c = &mut self.clients[i];
                        c.input = None;
                        for (k, inp) in inputs.into_iter().enumerate() {
                            // in order and new only; a client more than a second ahead loses the rest
                            let seq = first.wrapping_add(k as u32);
                            if seq > c.queue.back().map_or(c.acked, |q| q.0) && c.queue.len() < QUEUE_MAX && phys::sane(&inp) {
                                c.queue.push_back((seq, inp));
                            }
                        }
                    }
                    Recv::Msg(ToServer::Edit { seq, shape, material }) => {
                        let ok = self.edit(i, &shape, material, &mut changed);
                        self.clients[i].done.push((seq, ok));
                    }
                    Recv::Msg(ToServer::Bucket { seq, at, fill }) => {
                        let ok = self.bucket(i, at, fill);
                        self.clients[i].done.push((seq, ok));
                    }
                    Recv::Msg(ToServer::Creative(on)) => {
                        let c = &mut self.clients[i];
                        (c.creative, c.inv_dirty) = (on, true);
                    }
                    Recv::Msg(ToServer::Chat(text)) => self.chat(i, &text, &mut changed),
                    Recv::Empty => break,
                    Recv::Closed => {
                        open = false;
                        break;
                    }
                }
            }
            if open {
                i += 1;
            } else {
                let c = self.clients.swap_remove(i);
                if !c.player.name.is_empty() {
                    self.absent.insert(c.player.name.clone(), saved(&c));
                    self.say(format!("{} left", c.player.name));
                }
            }
        }
        self.debt += self.dt;
        let steps = ((self.debt + 1e-9) / phys::DT) as u32;
        self.debt -= steps as f64 * phys::DT;
        for c in &mut self.clients {
            // flight is creative's (ADR 0023)
            let creative = c.creative;
            let walk = move |inp: Input| if creative { inp } else { Input { fly: 0.0, ..inp } };
            if let Some(inp) = c.input.map(walk) {
                for _ in 0..steps {
                    phys::step(&self.world, &mut c.phys, &inp, phys::DT);
                }
                c.player.pos = c.phys.pos.to_array();
                c.player.fwd = c.phys.fwd.as_vec3().to_array();
                c.player.pitch = inp.pitch;
            } else {
                // each step once, at most as fast as real time; a late batch catches up within CREDIT_MAX
                c.credit = (c.credit + steps).min(CREDIT_MAX);
                while c.credit > 0 {
                    let Some((seq, inp)) = c.queue.pop_front() else { break };
                    let mut inp = walk(inp);
                    // at the client's time when it is near this clock (it predicted the step then), else at this one
                    let near = 2.0 * self.warp as f64; // two seconds of real time
                    if (inp.t - self.world.time).abs() > near {
                        inp.t = 0.0;
                    }
                    phys::step_at(&mut self.world, &mut c.phys, &inp, phys::DT);
                    (c.acked, c.credit, c.player.pitch) = (seq, c.credit - 1, inp.pitch);
                }
                c.player.pos = c.phys.pos.to_array();
                c.player.fwd = c.phys.fwd.as_vec3().to_array();
            }
            c.link.send(ToClient::You { step: c.acked, me: c.phys, time: self.world.time, warp: self.warp as f64 });
        }
        // water finds the new gaps of every column an edit changed, then flows: as many steps as the physics
        // took, fewer when that would spend more than WATER_BUDGET
        let mut cols: Vec<Block> = changed.iter().map(|(b, _)| Block { pos: glam::IVec3::new(b.pos.x, b.pos.y, 0), ..*b }).collect();
        cols.sort_unstable_by_key(|b| (b.body, b.face, b.pos.x, b.pos.y));
        cols.dedup();
        for b in cols {
            self.world.regap(b);
        }
        let can = (WATER_BUDGET / self.world.water.awake().max(1)) as u32;
        let wsteps = if can == 0 && self.tick % 4 == 0 { 1 } else { steps.min(can) };
        self.water_skipped += (steps - wsteps) as u64;
        for _ in 0..wsteps {
            self.world.step_water();
        }
        if !changed.is_empty() {
            for c in &self.clients {
                c.link.send(ToClient::Edits(changed.clone()));
            }
        }
        let cols: Vec<Col> = self.world.water.take_changes().into_iter().map(|w| w.0).collect();
        self.water_changed(&cols);
        self.send_water();
        for c in &mut self.clients {
            for (seq, ok) in c.done.drain(..) {
                c.link.send(ToClient::Done { seq, ok });
            }
            if std::mem::take(&mut c.inv_dirty) {
                let counts = c.inv.iter().enumerate().filter(|(_, n)| **n > 0).map(|(m, n)| (m as u8, *n)).collect();
                c.link.send(ToClient::Inventory { creative: c.creative, counts, water: c.water });
            }
        }
        let players: Vec<Player> = self.clients.iter().map(|c| c.player.clone()).collect();
        for c in &self.clients {
            c.link.send(ToClient::Players(players.clone()));
        }
        self.tick += 1;
        if let Some((path, every)) = &self.autosave {
            if self.tick % every == 0 {
                // the snapshot is taken here, between ticks; serialising and writing are a thread's
                let (s, path) = (self.snapshot(), path.clone());
                std::thread::spawn(move || {
                    if let Err(e) = s.write(&path) {
                        eprintln!("[technoape-server] save failed: {e}");
                    }
                });
            }
        }
        if self.rec.is_some() {
            if self.tick % CHECK_EVERY == 0 {
                let ev = Ev::Check { tick: self.tick, hash: self.hash() };
                self.log(ev);
            }
            // every tick reaches the file, so a crash (panic = abort) loses at most the tick it happened in
            if let Err(e) = self.rec.as_mut().unwrap().flush() {
                eprintln!("[technoape-server] recording stopped: {e}");
                self.rec = None;
            }
        }
    }

    /// Puts player `i` at `p` looking along `look` (in the tangent plane there), at rest.
    fn put(&mut self, i: usize, p: DVec3, look: DVec3) {
        let c = &mut self.clients[i];
        c.phys = phys::Player::at(p, phys::tangent(look, phys::up(&self.world, p), DVec3::X));
        (c.input, c.credit) = (None, 0);
        c.queue.clear();
        (c.player.pos, c.player.fwd) = (p.to_array(), c.phys.fwd.as_vec3().to_array());
    }

    /// Where /goto takes a player (ADR 0023): Terra's spawn, another body (Luna) where the sun is 45 degrees up,
    /// or a spot in space SUN_VIEW sun radii from the sun's centre with the sun on its horizon; and the way to look.
    /// A body is named by its row in `data/bodies.toml` (Terra or planet, Luna or moon).
    fn goto(&self, to: &str) -> Option<(DVec3, DVec3)> {
        let w = &self.world;
        match to {
            "sun" => {
                let (sun, planet) = (w.sky.sun(w.time), w.sky.planet(w.time).centre);
                // beside the sun as seen from the planet: up there is away from the planet, the sun level
                let side = (planet - sun).normalize().cross(DVec3::Z).normalize();
                let x = sun + side * sky::SUN_RADIUS * SUN_VIEW;
                let (pos, f) = w.locate(x);
                Some((pos, w.pose(f, w.time).rot.inverse() * (sun - x)))
            }
            _ => match engine::bodies::find(to).filter(|&i| i < w.bodies.len())? {
                0 => Some((w.bodies[0].spawn_point(), w.bodies[0].spawn.any_orthonormal_vector())),
                // the day side, the sun 45 degrees up: an airless night is black
                i => {
                    let p = w.pose(i, w.time);
                    let s = (p.rot.inverse() * (w.sky.sun(w.time) - p.centre)).normalize();
                    let d = (s + s.any_orthonormal_vector()).normalize();
                    Some((w.bodies[i].stand_point(d), s))
                }
            },
        }
    }

    /// One line to everyone's chat.
    fn say(&self, line: String) {
        for c in &self.clients {
            c.link.send(ToClient::Chat(line.clone()));
        }
    }

    /// A chat line from client `i` (ADR 0023): a '/' command runs here and answers it alone; any other line
    /// goes to everyone with its sender's name.
    fn chat(&mut self, i: usize, text: &str, changed: &mut Vec<(Block, u8)>) {
        let text: String = text.chars().filter(|c| !c.is_control()).take(CHAT_MAX).collect();
        let text = text.trim();
        if let Some(cmd) = text.strip_prefix('/') {
            let answer = self.command(i, cmd, changed);
            self.clients[i].link.send(ToClient::Chat(answer));
        } else if !text.is_empty() {
            let name = &self.clients[i].player.name;
            self.say(format!("<{}> {text}", if name.is_empty() { "?" } else { name }));
        }
    }

    /// The commands (ADR 0023), and the answer for the player who gave one. shortcut: anyone may run every
    /// command; an operator role when servers go public.
    fn command(&mut self, i: usize, cmd: &str, changed: &mut Vec<(Block, u8)>) -> String {
        let w: Vec<&str> = cmd.split_whitespace().collect();
        let mode = |creative: bool| if creative { "creative" } else { "survival" };
        let block_named = |name: &str| BLOCKS.placeable().find(|d| d.name.eq_ignore_ascii_case(name));
        let blocks = || BLOCKS.placeable().map(|d| d.name.as_str()).collect::<Vec<&str>>().join(", ");
        match w.first().map(|s| s.to_lowercase()).as_deref() {
            Some("help") | None => [
                "Commands:",
                "  /time                the time of day at the spawn",
                "  /time set <when>     sunrise day noon sunset night",
                "                       midnight, 14:30, or world seconds",
                "  /time add <seconds>  move the clock on",
                "  /playmode <mode>     creative or survival",
                "  /give <block> [n]    n blocks, 64 if left out",
                "  /place <block>       one under your feet; or at",
                "    [face i j layer]   that block of the body you are on (F3)",
                "  /goto <where>        terra (home: the spawn), luna, or sun",
                "  /timewarp <n>        the clock n times faster (1 to 10000,",
                "                       up, down); . and , step it too",
                "  /help                this list",
                "Creative: double space flies, the wheel sets the speed.",
            ]
            .join("\n"),
            Some("time") => {
                let now = self.tick0.wrapping_add(self.tick) as f64 / TICK_HZ;
                let named = |v: &str| match v {
                    "sunrise" => Some(6.0),
                    "day" => Some(9.0),
                    "noon" => Some(12.0),
                    "sunset" => Some(18.0),
                    "night" => Some(21.0),
                    "midnight" => Some(0.0),
                    _ => {
                        let (h, m) = v.split_once(':')?;
                        let (h, m) = (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?);
                        (h < 24 && m < 60).then(|| h as f64 + m as f64 / 60.0)
                    }
                };
                let secs = |v: &str| v.parse::<f64>().ok().filter(|s| s.is_finite());
                let to = match (w.get(1).copied(), w.get(2).copied()) {
                    (None | Some("query"), _) => None,
                    (Some("set"), Some(v)) => match (named(v), secs(v)) {
                        (Some(h), _) => Some(sky::next_hour(now, h)),
                        (None, Some(s)) if s >= 0.0 => Some(s),
                        _ => return format!("not a time: {v}\n  try noon, sunset, 14:30 or a number of seconds"),
                    },
                    (Some("add"), Some(v)) => match secs(v).map(|s| now + s) {
                        Some(t) if t >= 0.0 => Some(t),
                        _ => return format!("not a number of seconds: {v}"),
                    },
                    _ => return "usage:\n  /time\n  /time set <sunrise|day|noon|sunset|night|midnight|hh:mm|seconds>\n  /time add <seconds>".into(),
                };
                let t = match to {
                    Some(t) => {
                        // the next tick shows it; a time before this session's start wraps the tick counter
                        let ticks = (t.min(1e12) * TICK_HZ).round() as u64; // past 1e12 s the clock steps coarser than 0.1 ms
                        self.tick0 = ticks.wrapping_sub(self.tick);
                        self.world.time = ticks as f64 / TICK_HZ;
                        self.world.time
                    }
                    None => now,
                };
                let (day, h) = sky::clock(t);
                let what = if to.is_some() { "time set to" } else { "it is" };
                let m = (h * 60.0).round() as u32 % (24 * 60);
                format!("{what} day {day}, {:02}:{:02} at the spawn (world time {t:.0} s)", m / 60, m % 60)
            }
            Some("goto") => {
                let to = w.get(1).map(|s| s.to_lowercase()).unwrap_or_default();
                if to == "sun" && !self.clients[i].creative {
                    return "the sun is creative's: there is nothing to stand on\n  /playmode creative, then /goto sun".into();
                }
                match self.goto(&to) {
                    Some((p, look)) => {
                        self.put(i, p, look);
                        format!("gone to {}", engine::bodies::find(&to).map_or("the sun", |b| self.world.bodies[b].name.as_str()))
                    }
                    None => "usage: /goto terra|luna|sun\n  terra (or home) is the spawn".into(),
                }
            }
            Some("timewarp") | Some("warp") => {
                let now = self.warp;
                let to = match w.get(1).map(|s| s.to_lowercase()).as_deref() {
                    None => return format!("the time warp is {now}x
usage: /timewarp <1 to {MAX_WARP}|up|down>"),
                    Some("up") => (now * 10).min(MAX_WARP),
                    Some("down") => (now / 10).max(1),
                    Some(v) => match v.trim_end_matches('x').parse::<u64>() {
                        Ok(n) if (1..=MAX_WARP).contains(&n) => n,
                        _ => return format!("not a time warp: {v}
  1 (real time) to {MAX_WARP}"),
                    },
                };
                self.warp = to;
                let (day, year) = (sky::DAY / to as f64, sky::YEAR / to as f64);
                format!("time warp {to}x: a day in {day:.0} s, a year in {year:.0} s")
            }
            Some("playmode") | Some("gamemode") => {
                let c = &mut self.clients[i];
                let on = match w.get(1).map(|s| s.to_lowercase()).as_deref() {
                    None => return format!("you play in {} mode\nusage: /playmode creative|survival", mode(c.creative)),
                    Some("creative" | "c" | "1") => true,
                    Some("survival" | "s" | "0") => false,
                    Some(m) => return format!("no play mode '{m}': creative or survival"),
                };
                (c.creative, c.inv_dirty) = (on, true);
                format!("play mode: {}", mode(on))
            }
            Some("give") => {
                // the count is a last word that is a number; the block name is the words before it
                let (words, n) = match w.last().and_then(|s| s.parse::<u32>().ok()) {
                    Some(n) if w.len() > 2 => (&w[1..w.len() - 1], n),
                    _ => (&w[1..], 64),
                };
                let name = words.join(" ");
                let Some(d) = block_named(&name) else {
                    let what = if name.is_empty() { String::new() } else { format!("no block '{name}'\n") };
                    return format!("{what}usage: /give <block> [count]\nblocks: {}", blocks());
                };
                let c = &mut self.clients[i];
                let held = &mut c.inv[d.id as usize];
                *held = held.saturating_add(n);
                c.inv_dirty = true;
                format!("gave {n} {}: {} held", d.name, *held)
            }
            Some("place") => {
                // bugs/2026-11-11_13-03-12: the last four words, when numbers, are F3's "Feet in" (face, i, j,
                // layer) on the body the player is on; without them the block under the feet. Free, like /give.
                // shortcut: anyone may place any block anywhere on their body, as anyone may run every command;
                // it becomes the owner's and creative's when the server has roles
                let nums: Vec<i32> = w.iter().skip(1).rev().take(4).map_while(|s| s.parse().ok()).collect();
                let (words, at) = if nums.len() == 4 { (&w[1..w.len() - 4], Some(nums)) } else { (&w[1..], None) };
                let name = words.join(" ");
                let Some(d) = block_named(&name) else {
                    let what = if name.is_empty() { String::new() } else { format!("no block '{name}'\n") };
                    return format!("{what}usage: /place <block> [face i j layer]\nblocks: {}", blocks());
                };
                let world = &self.world;
                let feet = self.clients[i].phys.pos;
                let body = world.frame_at(feet);
                if body == engine::world::INERTIAL {
                    return "there is no body under you out here: /place works on Terra and Luna".into();
                }
                let b = match at {
                    // (a face that is no face, a negative one too, fails `fits` below)
                    Some(n) => Block { body: body as u8, face: u8::try_from(n[3]).unwrap_or(u8::MAX), pos: glam::IVec3::new(n[2], n[1], n[0]) },
                    None => world.block_at(feet - phys::up(world, feet) * 0.5),
                };
                if !Shape::one(b).fits(world) {
                    return format!("no such block on {}: face 0 to 5, then i j layer as F3 shows them", world.bodies[body].name);
                }
                if BLOCKS.solid(d.id) && self.clients.iter().any(|c| phys::holds(world, &c.phys, b)) {
                    return "a player stands there: a solid block would trap them".into();
                }
                self.world.set_block(b, d.id);
                changed.push((b, d.id));
                format!("placed {} at face {} {} {}, layer {}", d.name, b.face, b.pos.x, b.pos.y, b.pos.z)
            }
            Some(other) => format!("unknown command /{other}\n  /help lists the commands"),
        }
    }

    /// Where a new player's feet go: on the surface at the planet's +y pole, like the client's benchmark.
    fn spawn(&self) -> DVec3 {
        let b = &self.world.bodies[0];
        b.spawn_point()
    }

    /// Client `i` digs or places `shape`: all of it or none. False when the shape fails `valid_shape` or a
    /// survival player cannot pay for every block it places.
    fn edit(&mut self, i: usize, shape: &Shape, m: u8, changed: &mut Vec<(Block, u8)>) -> bool {
        if !self.valid_shape(shape, m, DVec3::from_array(self.clients[i].player.pos)) {
            return false;
        }
        // a solid block never goes where it would trap a player
        let (world, clients) = (&self.world, &self.clients);
        let cells = edit::changes(world, shape, m, |b| clients.iter().any(|c| phys::holds(world, &c.phys, b)));
        let c = &mut self.clients[i];
        if !c.creative && !cells.is_empty() {
            if m == AIR {
                for (_, old) in &cells {
                    c.inv[*old as usize] = c.inv[*old as usize].saturating_add(1);
                }
            } else {
                let Some(left) = c.inv[m as usize].checked_sub(cells.len() as u32) else { return false };
                c.inv[m as usize] = left;
            }
            c.inv_dirty = true;
        }
        for (b, _) in cells {
            self.world.set_block(b, m);
            changed.push((b, m));
        }
        true
    }

    /// The water tiles client `i` has the latest version of.
    pub fn water_seen(&self, i: usize) -> Vec<Col> {
        let mut v: Vec<Col> = self.clients[i].tiles.iter().filter(|(t, v)| self.tiles[*t].0 == **v).map(|(t, _)| *t).collect();
        v.sort_unstable();
        v
    }

    /// Columns whose water changed: their tiles get this tick as version.
    fn water_changed(&mut self, cols: &[Col]) {
        for c in cols {
            let t = c.tile();
            let b = &self.world.bodies[t.body() as usize];
            let mid = (TILE as f64) * 0.5;
            let at = b.centre + b.dir(t.face() as usize, (t.i() * TILE) as f64 + mid, (t.j() * TILE) as f64 + mid) * b.radius;
            self.tiles.entry(t).or_insert((0, at)).0 = self.tick + 1;
        }
    }

    /// Sends every client the water tiles it lacks: within WATER_RANGE, nearest first, until WATER_BYTES.
    /// The rest waits for the next tick, so a big flood reaches a client over a few ticks, near water first.
    fn send_water(&mut self) {
        // each tile built and measured once a tick, however many clients get it
        let mut built: FxMap<Col, (Vec<engine::liquid::ColWater>, usize)> = FxMap::default();
        // what each client lacks, nearest first: a pass over every tile per client, so side by side when there
        // are many (in a game's own server the meshing fills rayon's pool, and one client needs no help)
        let seen: Vec<(DVec3, &FxMap<Col, u64>)> = self.clients.iter().map(|c| (DVec3::from_array(c.player.pos), &c.tiles)).collect();
        let lacks = |(me, have): &(DVec3, &FxMap<Col, u64>)| {
            let mut want: Vec<(u64, Col)> = self
                .tiles
                .iter()
                .filter(|(t, (v, _))| have.get(t).is_none_or(|s| s < v))
                .map(|(t, (_, at))| ((at.distance(*me) * 16.0) as u64, *t))
                .filter(|(d, _)| (*d as f64) < WATER_RANGE * 16.0)
                .collect();
            want.sort_unstable();
            want
        };
        let wants: Vec<Vec<(u64, Col)>> = if seen.len() >= 8 { seen.par_iter().map(lacks).collect() } else { seen.iter().map(lacks).collect() };
        for (c, want) in self.clients.iter_mut().zip(wants) {
            if want.is_empty() {
                continue;
            }
            let (mut msg, mut bytes) = (Vec::new(), 0);
            for (_, t) in want {
                let (cols, n) = built.entry(t).or_insert_with(|| {
                    let cols = self.world.water.tile_sent(t);
                    let n = postcard::to_allocvec(&(t, &cols)).map_or(0, |b| b.len());
                    (cols, n)
                });
                bytes += *n;
                c.tiles.insert(t, self.tiles[&t].0);
                msg.push((t, cols.clone()));
                if bytes >= WATER_BYTES {
                    break;
                }
            }
            self.water_bytes += bytes as u64;
            c.link.send(ToClient::Water(msg));
        }
    }

    /// Client `i`'s bucket at block `at`: `fill` scoops up to a full bucket from the water there, else the
    /// bucket is poured out there (into the gap that holds `at`, which must be air). Creative scoops and pours
    /// a full bucket every time, holds nothing, and the ledger counts it.
    fn bucket(&mut self, i: usize, at: Block, fill: bool) -> bool {
        let body = self.world.bodies.get(at.body as usize);
        let inside = body.is_some_and(|b| at.face < 6 && (0..b.n).contains(&at.pos.x) && (0..b.n).contains(&at.pos.y) && (0..b.layers).contains(&at.pos.z));
        let feet = DVec3::from_array(self.clients[i].player.pos);
        if !inside || self.world.block_centre(at).distance(feet) > REACH || BLOCKS.solid(self.world.block(at)) {
            return false;
        }
        let col = World::col(at);
        let c = &mut self.clients[i];
        let World { bodies, edits, water, .. } = &mut self.world;
        let g = engine::world::Solid { bodies, edits };
        let ok = match (fill, c.creative) {
            (true, false) => {
                let took = water.scoop(&g, col, at.pos.z, U - c.water as i64);
                c.water += took as u32;
                took > 0
            }
            (true, true) => {
                self.removed += water.scoop(&g, col, at.pos.z, U);
                true
            }
            (false, false) => c.water > 0 && water.pour(&g, col, at.pos.z, std::mem::take(&mut c.water) as i64),
            (false, true) => {
                let ok = water.pour(&g, col, at.pos.z, U);
                self.made += if ok { U } else { 0 };
                ok
            }
        };
        c.inv_dirty |= ok && !c.creative;
        ok
    }

    /// The trust boundary: a box within the size limits on a face that exists, a known block, every cell
    /// within the player's reach.
    fn valid_shape(&self, s: &Shape, m: u8, at: DVec3) -> bool {
        s.fits(&self.world)
            && BLOCKS.get(m).is_some()
            && s.cells().all(|b| self.world.block_centre(b).distance(at) <= REACH)
    }
}

/// What a replay found.
pub struct Replayed {
    pub server: Server,
    pub checks: u64,
    /// The first checkpoint that disagreed: (tick, recorded hash, replayed hash).
    pub diverged: Option<(u64, u64, u64)>,
}

/// Reads a recording.
pub fn read_recording(path: &Path) -> std::io::Result<Vec<Ev>> {
    let bytes = std::fs::read(path)?;
    let (mut evs, mut at) = (Vec::new(), 0);
    // a crash can cut the last record short; everything before it stands
    while at + 4 <= bytes.len() {
        let n = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        let Some(b) = bytes.get(at + 4..at + 4 + n) else { break };
        evs.push(postcard::from_bytes(b).map_err(std::io::Error::other)?);
        at += 4 + n;
    }
    Ok(evs)
}

/// Runs a recording again headless, to its end or to tick `until`, stopping at the first checkpoint that
/// disagrees.
pub fn replay(path: &Path, until: Option<u64>) -> std::io::Result<Replayed> {
    let evs = read_recording(path)?;
    let Some(Ev::Start { planet_n, moon_n, seed, dt }) = evs.first() else {
        return Err(std::io::Error::other("not a recording: no Start"));
    };
    let (tx, rx) = channel();
    let (mut s, rest) = match evs.get(1) {
        // (a recording carries no version of its own; one that starts from an older save is from before
        // 1 m blocks, ADR 0033. shortcut: one that starts from the seed alone is not told apart)
        Some(Ev::Loaded(save)) if save.version != save::VERSION => return Err(std::io::Error::other("a recording of an older world (0.5 m blocks, ADR 0033)")),
        Some(Ev::Loaded(save)) => (Server::from_save(save.clone(), rx), &evs[2..]),
        _ => (Server::new(*planet_n, *moon_n, *seed, rx), &evs[1..]),
    };
    s.dt = *dt;
    let mut links: HashMap<u32, Link<ToServer, ToClient>> = HashMap::new();
    let until = until.unwrap_or(u64::MAX);
    let mut r = Replayed { server: s, checks: 0, diverged: None };
    let run_to = |s: &mut Server, links: &HashMap<u32, Link<ToServer, ToClient>>, t: u64| {
        while s.tick < t.min(until) {
            s.tick();
            for l in links.values() {
                while let Recv::Msg(_) = l.try_recv() {}
            }
        }
    };
    for ev in rest {
        let t = match ev {
            Ev::Join { tick, .. } | Ev::Msg { tick, .. } | Ev::Leave { tick, .. } | Ev::Check { tick, .. } => *tick,
            Ev::Start { .. } | Ev::Loaded(_) => return Err(std::io::Error::other("a Start or Loaded past the start")),
        };
        run_to(&mut r.server, &links, t);
        if r.server.tick >= until {
            break;
        }
        match ev {
            Ev::Join { id, .. } => {
                let (c, sv) = local();
                tx.send(sv).unwrap();
                links.insert(*id, c);
            }
            Ev::Msg { id, m, .. } => {
                if let Some(l) = links.get(id) {
                    l.send(m.clone());
                }
            }
            Ev::Leave { id, .. } => {
                links.remove(id);
            }
            Ev::Check { tick, hash } => {
                r.checks += 1;
                let got = r.server.hash();
                if got != *hash {
                    r.diverged = Some((*tick, *hash, got));
                    break;
                }
            }
            Ev::Start { .. } | Ev::Loaded(_) => unreachable!(),
        }
    }
    if until != u64::MAX && r.diverged.is_none() {
        run_to(&mut r.server, &links, until);
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::net::local;
    use std::sync::mpsc::channel;

    fn server_with(n: usize) -> (Server, Vec<Link<ToServer, ToClient>>) {
        let (tx, rx) = channel();
        let mut s = Server::new(256, 32, 1, rx);
        let links = (0..n)
            .map(|_| {
                let (c, sv) = local();
                tx.send(sv).unwrap();
                c
            })
            .collect();
        s.tick();
        (s, links)
    }

    fn drain(l: &Link<ToServer, ToClient>) -> Vec<ToClient> {
        let mut v = Vec::new();
        while let Recv::Msg(m) = l.try_recv() {
            v.push(m);
        }
        v
    }

    #[test]
    fn edit_reaches_the_other_player_and_a_late_joiner() {
        let (mut s, links) = server_with(2);
        for l in &links {
            assert!(matches!(drain(l)[0], ToClient::Welcome { seed: 1, planet_n: 256, .. }));
        }
        // stand on the +z pole and toggle the block under the feet
        let (pole, under) = on_land(&s);
        let stone = BLOCKS.id("stone");
        let m = if s.world.block(under) == stone { AIR } else { stone };
        links[0].send(ToServer::Move { pos: pole.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
        links[0].send(ToServer::Creative(true));
        links[0].send(ToServer::Edit { seq: 1, shape: Shape::one(under), material: m });
        s.tick();
        assert!(drain(&links[0]).contains(&ToClient::Done { seq: 1, ok: true }));
        assert_eq!(s.world.block(under), m);
        assert!(drain(&links[1]).contains(&ToClient::Edits(vec![(under, m)])));
        // a third player joining later gets the edit in the initial sync
        let (tx, rx) = channel();
        let mut s2 = Server { accept: rx, ..s };
        let (c, sv) = local();
        tx.send(sv).unwrap();
        s2.tick();
        assert!(drain(&c).contains(&ToClient::Edits(vec![(under, m)])));
    }

    /// A spot on dry land away from the spawn (out of a new player's reach), 2 m over the middle of a column,
    /// and the air cell on the ground under it (placing fills air only).
    fn on_land(s: &Server) -> (DVec3, Block) {
        let b = &s.world.bodies[0];
        let sea = b.layer_r(b.sea.unwrap());
        let d = (10..400).map(|t| (b.spawn + DVec3::X * 0.02 * t as f64).normalize()).find(|d| b.height(*d) > sea + 1.0).unwrap();
        let k = s.world.block_at(b.centre + d * b.radius);
        let d = b.dir(k.face as usize, k.pos.x as f64 + 0.5, k.pos.y as f64 + 0.5);
        let h = b.height(d);
        (b.centre + d * (h + 2.0), s.world.block_at(b.centre + d * (h + 0.6)))
    }

    #[test]
    fn bad_edits_are_dropped() {
        let (mut s, links) = server_with(1);
        drain(&links[0]);
        let (pole, under) = on_land(&s);
        let before = s.world.block(under);
        let one = Shape::one(under);
        links[0].send(ToServer::Edit { seq: 1, shape: one, material: 0 }); // player still at the origin: out of reach
        links[0].send(ToServer::Move { pos: pole.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
        links[0].send(ToServer::Edit { seq: 2, shape: one, material: 200 }); // unknown material
        links[0].send(ToServer::Edit { seq: 3, shape: Shape { body: 9, ..one }, material: 0 }); // no such body
        let far = Shape { max: one.min + glam::IVec3::new(0, 0, 40), ..one };
        links[0].send(ToServer::Edit { seq: 4, shape: far, material: 0 }); // too long, and reaching the sky
        let wide = Shape { min: one.min - glam::IVec3::new(31, 31, 0), ..one };
        links[0].send(ToServer::Edit { seq: 5, shape: wide, material: 0 }); // in size, but 22 m away at one corner
        links[0].send(ToServer::Edit { seq: 6, shape: one, material: BLOCKS.id("stone") }); // survival, nothing held
        s.tick();
        assert_eq!(s.world.block(under), before);
        assert_eq!(s.world.edit_count, 0);
        let got = drain(&links[0]);
        assert!(!got.iter().any(|m| matches!(m, ToClient::Edits(_))));
        for seq in 1..=6 {
            assert!(got.contains(&ToClient::Done { seq, ok: false }), "edit {seq} refused: {:?}", got.iter().filter(|m| matches!(m, ToClient::Done { .. })).collect::<Vec<_>>());
        }
    }

    #[test]
    fn survival_digs_into_the_inventory_and_pays_for_what_it_places() {
        let (mut s, links) = server_with(1);
        let l = &links[0];
        let ground = s.world.bodies[0].centre + DVec3::Z * s.world.bodies[0].height(DVec3::Z);
        let pole = ground + DVec3::Z;
        let deep = s.world.block_at(ground - DVec3::Z * 6.0); // below the dirt: stone
        let stone = BLOCKS.id("stone");
        assert_eq!(s.world.block(deep), stone);
        let brush = Shape { max: deep.pos + 1, ..Shape::one(deep) };
        l.send(ToServer::Move { pos: pole.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
        l.send(ToServer::Edit { seq: 1, shape: brush, material: AIR });
        s.tick();
        assert!(brush.cells().all(|b| s.world.block(b) == AIR));
        let got = drain(l);
        assert!(got.contains(&ToClient::Inventory { creative: false, counts: vec![], water: 0 }), "sent on join");
        assert!(got.contains(&ToClient::Inventory { creative: false, counts: vec![(stone, 8)], water: 0 }));
        // eight dirt are not held, so none of them goes in; the eight stone are
        l.send(ToServer::Edit { seq: 2, shape: brush, material: BLOCKS.id("dirt") });
        l.send(ToServer::Edit { seq: 3, shape: brush, material: stone });
        s.tick();
        let got = drain(l);
        assert!(got.contains(&ToClient::Done { seq: 2, ok: false }) && got.contains(&ToClient::Done { seq: 3, ok: true }));
        assert!(got.contains(&ToClient::Inventory { creative: false, counts: vec![], water: 0 }));
        assert!(brush.cells().all(|b| s.world.block(b) == stone));
        assert_eq!(s.world.edit_count, 8, "the world is as generated again, but the cells stay recorded");
    }

    #[test]
    fn a_saved_player_comes_back_with_what_it_held() {
        let (mut s, links) = server_with(1);
        let l = &links[0];
        let ground = s.world.bodies[0].centre + DVec3::Z * s.world.bodies[0].height(DVec3::Z);
        let deep = s.world.block_at(ground - DVec3::Z * 6.0);
        l.send(ToServer::Hello { name: "ape".into() });
        l.send(ToServer::Move { pos: (ground + DVec3::Z).to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
        l.send(ToServer::Edit { seq: 1, shape: Shape::one(deep), material: AIR });
        s.tick();
        drop(links); // the player leaves; the server keeps it by name
        s.tick();
        assert_eq!(s.players(), 0);
        let path = std::env::temp_dir().join(format!("technoape-test-{}.sav", std::process::id()));
        s.snapshot().write(&path).unwrap();
        let (tx, rx) = channel();
        let mut back = Server::from_save(Save::read(&path).unwrap(), rx);
        std::fs::remove_file(&path).unwrap();
        assert_eq!((back.world.hash(), back.snapshot()), (s.world.hash(), s.snapshot()));
        let rec = path.with_extension("rec");
        back.record(&rec).unwrap();
        let (c, sv) = local();
        tx.send(sv).unwrap();
        c.send(ToServer::Hello { name: "ape".into() });
        back.tick();
        let got = drain(&c);
        // a recording of a server that began from a save replays from that save
        for _ in 0..40 {
            back.tick();
        }
        let r = replay(&rec, None).unwrap();
        std::fs::remove_file(&rec).unwrap();
        assert_eq!((r.diverged, r.checks, r.server.world.hash()), (None, 2, back.world.hash()));
        assert!(got.contains(&ToClient::Inventory { creative: false, counts: vec![(BLOCKS.id("stone"), 1)], water: 0 }), "{got:?}");
        assert!(DVec3::from_array(back.clients[0].player.pos).distance(ground + DVec3::Z) < 0.01);
        assert!(Save::from_bytes(b"not a save").is_err());
    }

    /// A bucket scoops a block of sea and pours it back; creative water is counted as made. Through all of it,
    /// a save and a load, not a unit appears or vanishes (the ledger stays 0).
    #[test]
    fn the_bucket_carries_water_and_none_is_lost() {
        let (mut s, links) = server_with(1);
        let l = &links[0];
        let b = s.world.bodies.clone();
        let b = &b[0];
        let sea = b.radius - engine::world::SEA_DEPTH;
        // a spot over 2 m of sea
        let d = (0..2000)
            .map(|k| {
                let z = 1.0 - 2.0 * (k as f64 + 0.5) / 2000.0;
                let t = k as f64 * 2.4;
                DVec3::new((1.0 - z * z).sqrt() * t.cos(), z, (1.0 - z * z).sqrt() * t.sin())
            })
            .find(|&d| b.height(d) < sea - 2.0)
            .expect("the planet has sea");
        let at = s.world.block_at(b.centre + d * (sea - 0.25));
        l.send(ToServer::Hello { name: "ape".into() });
        l.send(ToServer::Move { pos: (b.centre + d * (sea + 1.0)).to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
        l.send(ToServer::Bucket { seq: 1, at, fill: true });
        s.tick();
        let got = drain(l);
        assert!(got.contains(&ToClient::Done { seq: 1, ok: true }), "{got:?}");
        assert!(got.iter().any(|m| matches!(m, ToClient::Inventory { water, .. } if *water == U as u32)));
        assert!(got.iter().any(|m| matches!(m, ToClient::Water(v) if !v.is_empty())), "the scoop is sent");
        assert_eq!(s.water_ledger(), 0);
        // a second scoop fills nothing: the bucket is full
        l.send(ToServer::Bucket { seq: 2, at, fill: true });
        s.tick();
        assert!(drain(l).contains(&ToClient::Done { seq: 2, ok: false }));
        for _ in 0..100 {
            s.tick();
            assert_eq!(s.water_ledger(), 0);
        }
        l.send(ToServer::Bucket { seq: 3, at, fill: false });
        l.send(ToServer::Creative(true));
        l.send(ToServer::Bucket { seq: 4, at, fill: false });
        s.tick();
        let got = drain(l);
        assert!(got.contains(&ToClient::Done { seq: 3, ok: true }) && got.contains(&ToClient::Done { seq: 4, ok: true }));
        assert_eq!((s.clients[0].water, s.made), (0, U));
        for _ in 0..200 {
            s.tick();
            assert_eq!(s.water_ledger(), 0);
        }
        // far out of reach: refused
        l.send(ToServer::Bucket { seq: 5, at: s.world.block_at(b.centre - d * sea), fill: true });
        s.tick();
        assert!(drain(l).contains(&ToClient::Done { seq: 5, ok: false }));
        let back = Server::from_save(Save::from_bytes(&s.snapshot().to_bytes()).unwrap(), channel().1);
        assert_eq!((back.hash() == s.hash(), back.water_ledger()), (false, 0), "players differ, water does not");
        assert_eq!(back.world.water.all(), s.world.water.all());
        assert!(s.world.water.stored() > 0);
    }

    #[test]
    fn walking_input_moves_the_player_on_the_ground() {
        let (mut s, links) = server_with(1);
        drain(&links[0]);
        let start = DVec3::from_array(s.clients[0].player.pos);
        links[0].send(ToServer::Input(Input { mv: [1.0, 0.0], jump: true, fwd: [1.0, 0.0, 0.0], pitch: 0.0, ..Default::default() }));
        for _ in 0..60 {
            s.tick(); // 3 s at the default 20 Hz
        }
        let p = &s.clients[0].phys;
        let up = phys::up(&s.world, p.pos);
        // the spawn faces a steep slope that takes a jump per wall; walking speed is phys's test
        assert!((p.pos - start).reject_from(up).length() > 3.0, "moved {:?}", p.pos - start);
        assert!(!phys::inside(&s.world, p));
        let seen = drain(&links[0]).into_iter().filter_map(|m| if let ToClient::Players(v) = m { Some(v) } else { None }).last().unwrap();
        assert_eq!(seen[0].pos, p.pos.to_array(), "clients get the server's position");
    }

    #[test]
    fn steps_are_taken_once_in_order_and_replay_to_the_same_place() {
        let (mut s, links) = server_with(1);
        drain(&links[0]);
        let l = &links[0];
        let start = s.clients[0].phys;
        // each step at its own time near the server's clock, as a client sends it: the moon's pull moves with
        // the clock (ADR 0024)
        let t0 = s.world.time + 0.5;
        let inp = |k: u32| Input { mv: [1.0, (k % 7) as f32 / 7.0 - 0.5], jump: k % 40 == 0, fwd: [1.0, 0.0, 0.2], pitch: 0.0, t: t0 + k as f64 * phys::DT, ..Default::default() };
        // 3 s of steps in batches of three, one batch a tick, with a duplicate and an old one thrown in
        let mut me = start;
        for t in 0..60u32 {
            l.send(ToServer::Steps { first: t * 3 + 1, inputs: (t * 3 + 1..t * 3 + 4).map(inp).collect() });
            if t == 30 {
                l.send(ToServer::Steps { first: 2, inputs: vec![inp(2)] });
            }
            for k in t * 3 + 1..t * 3 + 4 {
                phys::step_at(&mut s.world, &mut me, &inp(k), phys::DT);
            }
            s.tick();
        }
        let you = drain(l).into_iter().filter_map(|m| if let ToClient::You { step, me, .. } = m { Some((step, me)) } else { None }).last().unwrap();
        assert_eq!(you, (180, me), "the server took every step once and lands where the client predicted");
        // a client a second ahead is held to real time: sixty steps waiting at most, and in one tick the three
        // of this tick plus the three it had left over
        l.send(ToServer::Steps { first: 181, inputs: (181..400).map(inp).collect() });
        s.tick();
        assert_eq!((s.clients[0].acked, s.clients[0].queue.len()), (186, 54));
        assert!((start.pos - me.pos).length() > 3.0);
    }

    #[test]
    fn a_client_whose_clock_is_off_lands_where_it_predicted_in_space() {
        // out of a body's frame, up and the way ahead follow where the bodies are: the server must step at
        // the client's time, or every flight there is corrected (the camera glitch of bugs/2026-11-10_19-03-47)
        let (mut s, links) = server_with(1);
        let l = &links[0];
        l.send(ToServer::Creative(true));
        s.tick();
        let w = &s.world;
        let (pc, mc) = (w.sky.planet(w.time).centre, w.sky.moon(w.time).centre);
        let (pos, f) = w.locate(pc + (pc - mc).normalize() * 2500.0);
        assert_eq!(f, engine::world::INERTIAL);
        l.send(ToServer::Move { pos: pos.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
        s.tick();
        drain(l);
        let mut me = s.clients[0].phys;
        let mut cw = World::new(256, 32, 1);
        for t in 0..40u32 {
            cw.time = s.world.time - 0.04; // the client's clock a little behind the server's
            let inp = Input { mv: [1.0, 0.3], fwd: [0.0, 1.0, 0.0], fly: 1000.0, jump: t % 2 == 0, t: cw.time, ..Default::default() };
            for _ in 0..3 {
                phys::step(&cw, &mut me, &inp, phys::DT);
            }
            l.send(ToServer::Steps { first: t * 3 + 1, inputs: vec![inp; 3] });
            s.tick();
        }
        let (step, server) = drain(l).into_iter().filter_map(|m| if let ToClient::You { step, me, .. } = m { Some((step, me)) } else { None }).last().unwrap();
        assert_eq!(step, 120);
        assert!((server.pos - me.pos).length() < 1e-9, "the server is {} m off", (server.pos - me.pos).length());
        assert!((me.pos - pos).length() > 100.0, "it flew");
    }

    #[test]
    fn chat_reaches_everyone_and_commands_answer_their_sender() {
        let (mut s, links) = server_with(2);
        links[0].send(ToServer::Hello { name: "ape".into() });
        links[1].send(ToServer::Hello { name: "bot".into() });
        s.tick();
        let chat = |l: &Link<ToServer, ToClient>| -> Vec<String> {
            drain(l).into_iter().filter_map(|m| if let ToClient::Chat(t) = m { Some(t) } else { None }).collect()
        };
        assert!(chat(&links[1]).contains(&"ape joined".to_string()));
        chat(&links[0]);
        links[0].send(ToServer::Chat("hello there".into()));
        links[0].send(ToServer::Chat("/time set noon".into()));
        links[0].send(ToServer::Chat("/give Blue Lamp 5".into()));
        links[0].send(ToServer::Chat("/give stone".into()));
        links[0].send(ToServer::Chat("/playmode creative".into()));
        links[0].send(ToServer::Chat("/nope".into()));
        s.tick();
        let (a, b) = (chat(&links[0]), chat(&links[1]));
        assert_eq!(b, vec!["<ape> hello there".to_string()], "commands answer only their sender");
        assert_eq!(a.len(), 6, "{a:?}");
        assert!(a[1].starts_with("time set to day 1, 12:00"), "{}", a[1]);
        assert_eq!((a[2].as_str(), a[3].as_str()), ("gave 5 blue lamp: 5 held", "gave 64 stone: 64 held"));
        assert!(a[5].starts_with("unknown command"));
        s.tick();
        assert!((s.world.time - sky::DAY / 8.0).abs() < 0.2, "{}", s.world.time);
        let c = &s.clients.iter().find(|c| c.player.name == "ape").unwrap();
        assert!(c.creative && c.inv[BLOCKS.id("blue lamp") as usize] == 5 && c.inv[BLOCKS.id("stone") as usize] == 64);
        // time runs on from where it was set, and survives a save
        assert!(s.snapshot().tick as f64 / TICK_HZ > sky::DAY / 8.0);
        links[0].send(ToServer::Chat("/time set 10".into()));
        links[0].send(ToServer::Chat("/time".into()));
        s.tick();
        let a = chat(&links[0]);
        assert!(a.iter().any(|l| l.starts_with("it is day 1, 09:12")), "{a:?}");
        s.tick();
        assert!((s.world.time - 10.0).abs() < 0.2, "a time before the session's start: {}", s.world.time);
    }

    #[test]
    fn timewarp_runs_the_clock_faster() {
        let (mut s, links) = server_with(1);
        s.tick();
        links[0].send(ToServer::Chat("/timewarp 100".into()));
        s.tick();
        let t0 = s.world.time;
        for _ in 0..10 {
            s.tick();
        }
        assert!((s.world.time - t0 - 10.0 * 100.0 / TICK_HZ).abs() < 1e-6, "{}", s.world.time - t0);
        let you = drain(&links[0]).into_iter().filter_map(|m| if let ToClient::You { warp, .. } = m { Some(warp) } else { None }).last();
        assert_eq!(you, Some(100.0));
        links[0].send(ToServer::Chat("/timewarp up".into()));
        links[0].send(ToServer::Chat("/timewarp 0".into()));
        s.tick();
        assert_eq!(s.warp, 1000, "0 is refused, up is ten times");
    }

    #[test]
    fn commands_complete_to_known_words() {
        // every line the completion list builds (engine::net::complete) is one the commands know
        let mut lines = vec!["/".to_string()];
        let mut full = Vec::new();
        while let Some(l) = lines.pop() {
            let (at, opts) = engine::net::complete(&l);
            if opts.is_empty() {
                full.push(l.trim_end().to_string());
            }
            lines.extend(opts.iter().map(|o| format!("{}{o} ", &l[..at])));
        }
        assert!(full.len() > 15, "{full:?}");
        let (mut s, links) = server_with(1);
        s.tick();
        drain(&links[0]);
        for l in &full {
            links[0].send(ToServer::Chat(l.clone()));
        }
        s.tick();
        let answers: Vec<String> = drain(&links[0]).into_iter().filter_map(|m| if let ToClient::Chat(t) = m { Some(t) } else { None }).collect();
        assert_eq!(answers.len(), full.len(), "{answers:?}");
        for (l, a) in full.iter().zip(&answers) {
            assert!(!["unknown command", "no play mode", "no block", "not a"].iter().any(|bad| a.starts_with(bad)), "{l}: {a}");
        }
    }

    #[test]
    fn goto_takes_a_player_to_the_bodies_and_the_sun() {
        let (mut s, links) = server_with(1);
        let me = |s: &Server| s.clients[0].phys.pos;
        for (to, body) in [("Luna", 1), ("home", 0), ("luna", 1), ("terra", 0)] {
            links[0].send(ToServer::Chat(format!("/goto {to}")));
            s.tick();
            let b = &s.world.bodies[body];
            let r = (me(&s) - b.centre).length();
            assert!((r - b.top((me(&s) - b.centre) / r)).abs() < 3.0, "{to}: {r}");
            assert!(!phys::inside(&s.world, &s.clients[0].phys));
        }
        links[0].send(ToServer::Chat("/goto sun".into()));
        s.tick();
        assert!(me(&s).distance(s.world.bodies[0].spawn_point()) < 3.0, "survival stays");
        links[0].send(ToServer::Chat("/playmode creative".into()));
        links[0].send(ToServer::Chat("/goto sun".into()));
        s.tick();
        let x = s.world.to_inertial(me(&s));
        let d = (x - s.world.sky.sun(s.world.time)).length() / sky::SUN_RADIUS;
        assert!((d - SUN_VIEW).abs() < 0.1, "{d} sun radii away");
        let got: Vec<ToClient> = drain(&links[0]);
        for line in ["gone to the sun", "gone to Luna", "gone to Terra"] {
            assert!(got.iter().any(|m| matches!(m, ToClient::Chat(t) if t == line)), "{line}");
        }
    }

    #[test]
    fn goto_knows_the_bodies_by_name_only() {
        // bugs/2026-11-11_13-10-42: planet and moon are gone, Terra and Luna are the names
        let (mut s, links) = server_with(1);
        s.tick();
        drain(&links[0]);
        for to in ["planet", "moon"] {
            links[0].send(ToServer::Chat(format!("/goto {to}")));
        }
        links[0].send(ToServer::Chat("/help".into()));
        s.tick();
        let a: Vec<String> = drain(&links[0]).into_iter().filter_map(|m| if let ToClient::Chat(t) = m { Some(t) } else { None }).collect();
        assert!(a[0].starts_with("usage: /goto") && a[1].starts_with("usage: /goto"), "{a:?}");
        assert!(!a.iter().any(|l| l.contains("planet") || l.contains("moon")), "{a:?}");
    }

    #[test]
    fn place_puts_a_block_under_the_player_or_where_it_is_told() {
        // bugs/2026-11-11_13-03-12
        let (mut s, links) = server_with(2);
        s.tick();
        drain(&links[0]);
        drain(&links[1]);
        let feet = s.clients[0].phys.pos;
        let under = s.world.block_at(feet - phys::up(&s.world, feet) * 0.5);
        let there = Block { pos: under.pos + glam::IVec3::new(3, 0, 4), ..under };
        let (p, q) = (under.pos, there.pos);
        links[0].send(ToServer::Chat("/place blue lamp".into()));
        links[0].send(ToServer::Chat(format!("/place copper {} {} {} {}", there.face, q.x, q.y, q.z)));
        links[0].send(ToServer::Chat("/place nothing".into()));
        links[0].send(ToServer::Chat(format!("/place stone {} {} {} {}", under.face, p.x, p.y, p.z + 1)));
        // (the review) a face that is none, a negative one too, and a place off the grid are refused
        for bad in ["-1 5 5 0", "6 5 5 0", "0 5 5 2000000000", "0 -5 5 0"] {
            links[0].send(ToServer::Chat(format!("/place stone {bad}")));
        }
        s.tick();
        assert_eq!((s.world.block(under), s.world.block(there)), (BLOCKS.id("blue lamp"), BLOCKS.id("copper")));
        let a: Vec<String> = drain(&links[0]).into_iter().filter_map(|m| if let ToClient::Chat(t) = m { Some(t) } else { None }).collect();
        assert_eq!(a.len(), 8, "{a:?}");
        assert!(a[4..].iter().all(|t| t.starts_with("no such block on")), "{a:?}");
        assert!(a[0].starts_with("placed blue lamp at face") && a[1].starts_with("placed copper"), "{a:?}");
        assert!(a[2].starts_with("no block 'nothing'"), "{a:?}");
        assert!(a[3].starts_with("a player stands there"), "a solid block never traps a player: {a:?}");
        let edits: usize = drain(&links[1]).into_iter().map(|m| if let ToClient::Edits(e) = m { e.len() } else { 0 }).sum();
        assert_eq!(edits, 2, "everyone sees the blocks");
    }

    #[test]
    fn a_closed_link_removes_the_player() {
        let (mut s, mut links) = server_with(2);
        drop(links.remove(0));
        s.tick();
        assert_eq!(s.players(), 1);
    }
}
