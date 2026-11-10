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
use engine::world::{fnv, Block, World};
use glam::DVec3;
use serde::{Deserialize, Serialize};
use save::{Save, Saved};
use std::collections::{BTreeMap, HashMap};
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

/// What a save keeps of a connected player.
fn saved(c: &Client) -> Saved {
    let inv = c.inv.iter().enumerate().filter(|(_, n)| **n > 0).map(|(m, n)| (m as u8, *n)).collect();
    Saved { name: c.player.name.clone(), pos: c.phys.pos.to_array(), fwd: c.phys.fwd.as_vec3().to_array(), creative: c.creative, inv }
}

struct Client {
    link: ServerLink,
    player: Player,
    phys: phys::Player,
    /// Walking input; `None` while the player flies with `Move`.
    input: Option<Input>,
    /// Creative counts nothing; survival digs into `inv` and places from it (ADR 0014).
    creative: bool,
    /// How many of each block kind the player holds, by id.
    inv: Box<[u32; 256]>,
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
}

impl Server {
    pub fn new(planet_n: i32, moon_n: i32, seed: i32, accept: Receiver<ServerLink>) -> Server {
        Server { world: World::new(planet_n, moon_n, seed), seed, accept, clients: Vec::new(), next_id: 1, tick: 0, dt: 1.0 / TICK_HZ, debt: 0.0, rec: None, absent: BTreeMap::new(), loaded: None, autosave: None }
    }

    /// A server that carries on from `save` (its seed and planet win over any given elsewhere).
    pub fn from_save(save: Save, accept: Receiver<ServerLink>) -> Server {
        let mut s = Server::new(save.planet_n, save.moon_n, save.seed, accept);
        for (b, m) in &save.edits {
            s.world.set_block(*b, *m);
        }
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
        Save { version: save::VERSION, seed: self.seed, planet_n: p, moon_n: m, blocks: save::registry(), edits, players: players.into_values().collect() }
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

    /// The state as one number: the world's edits, every player's physics and input, the time carried over.
    pub fn hash(&self) -> u64 {
        let mut h = fnv(fnv(self.world.hash(), &self.tick.to_le_bytes()), &self.debt.to_bits().to_le_bytes());
        for p in self.absent.values() {
            h = fnv(h, &postcard::to_allocvec(p).unwrap());
        }
        for c in &self.clients {
            let p = &c.phys;
            h = fnv(h, &c.player.id.to_le_bytes());
            for v in [p.pos, p.vel, p.fwd] {
                for x in v.to_array() {
                    h = fnv(h, &x.to_bits().to_le_bytes());
                }
            }
            h = fnv(h, &[p.on_ground as u8, c.input.is_some() as u8, c.creative as u8]);
            for n in c.inv.iter() {
                h = fnv(h, &n.to_le_bytes());
            }
            for x in c.player.pos {
                h = fnv(h, &x.to_bits().to_le_bytes());
            }
            if let Some(i) = c.input {
                for x in [i.mv[0], i.mv[1], i.fwd[0], i.fwd[1], i.fwd[2], i.pitch] {
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
            self.tick();
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
                creative: false,
                inv: Box::new([0; 256]),
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
                            (c.player.pos, c.player.fwd, c.creative, c.inv_dirty) = (sv.pos, sv.fwd, sv.creative, true);
                            c.inv.fill(0);
                            for (m, n) in sv.inv {
                                c.inv[m as usize] = n;
                            }
                        }
                    }
                    Recv::Msg(ToServer::Move { pos, fwd, pitch }) => {
                        if pos.iter().all(|c| c.is_finite()) && fwd.iter().all(|c| c.is_finite()) {
                            let c = &mut self.clients[i];
                            let p = DVec3::from_array(pos);
                            c.phys = phys::Player::at(p, phys::tangent(DVec3::from_array(fwd.map(f64::from)), phys::up(&self.world, p), DVec3::X));
                            c.input = None;
                            (c.player.pos, c.player.fwd, c.player.pitch) = (pos, fwd, pitch);
                        }
                    }
                    Recv::Msg(ToServer::Input(inp)) => {
                        if inp.fwd.iter().all(|c| c.is_finite()) && inp.mv.iter().all(|c| c.is_finite()) {
                            self.clients[i].input = Some(inp);
                        }
                    }
                    Recv::Msg(ToServer::Edit { seq, shape, material }) => {
                        let ok = self.edit(i, &shape, material, &mut changed);
                        self.clients[i].done.push((seq, ok));
                    }
                    Recv::Msg(ToServer::Creative(on)) => {
                        let c = &mut self.clients[i];
                        (c.creative, c.inv_dirty) = (on, true);
                    }
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
                }
            }
        }
        self.debt += self.dt;
        let steps = ((self.debt + 1e-9) / phys::DT) as u32;
        self.debt -= steps as f64 * phys::DT;
        for c in &mut self.clients {
            if let Some(inp) = c.input {
                for _ in 0..steps {
                    phys::step(&self.world, &mut c.phys, &inp, phys::DT);
                }
                c.player.pos = c.phys.pos.to_array();
                c.player.fwd = c.phys.fwd.as_vec3().to_array();
                c.player.pitch = inp.pitch;
            }
        }
        if !changed.is_empty() {
            for c in &self.clients {
                c.link.send(ToClient::Edits(changed.clone()));
            }
        }
        for c in &mut self.clients {
            for (seq, ok) in c.done.drain(..) {
                c.link.send(ToClient::Done { seq, ok });
            }
            if std::mem::take(&mut c.inv_dirty) {
                let counts = c.inv.iter().enumerate().filter(|(_, n)| **n > 0).map(|(m, n)| (m as u8, *n)).collect();
                c.link.send(ToClient::Inventory { creative: c.creative, counts });
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

    /// Where a new player's feet go: on the surface at the planet's +y pole, like the client's benchmark.
    fn spawn(&self) -> DVec3 {
        let b = &self.world.bodies[0];
        b.centre + DVec3::Y * (b.height(DVec3::Y) + 0.6)
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
        let pole = s.world.bodies[0].centre + DVec3::Z * (s.world.bodies[0].radius + 2.0);
        let under = s.world.block_at(pole - DVec3::Z * 2.5);
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

    #[test]
    fn bad_edits_are_dropped() {
        let (mut s, links) = server_with(1);
        drain(&links[0]);
        let pole = s.world.bodies[0].centre + DVec3::Z * (s.world.bodies[0].radius + 2.0);
        let under = s.world.block_at(pole - DVec3::Z * 2.5);
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
            assert!(got.contains(&ToClient::Done { seq, ok: false }), "edit {seq} refused");
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
        assert!(got.contains(&ToClient::Inventory { creative: false, counts: vec![] }), "sent on join");
        assert!(got.contains(&ToClient::Inventory { creative: false, counts: vec![(stone, 8)] }));
        // eight dirt are not held, so none of them goes in; the eight stone are
        l.send(ToServer::Edit { seq: 2, shape: brush, material: BLOCKS.id("dirt") });
        l.send(ToServer::Edit { seq: 3, shape: brush, material: stone });
        s.tick();
        let got = drain(l);
        assert!(got.contains(&ToClient::Done { seq: 2, ok: false }) && got.contains(&ToClient::Done { seq: 3, ok: true }));
        assert!(got.contains(&ToClient::Inventory { creative: false, counts: vec![] }));
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
        assert!(got.contains(&ToClient::Inventory { creative: false, counts: vec![(BLOCKS.id("stone"), 1)] }), "{got:?}");
        assert!(DVec3::from_array(back.clients[0].player.pos).distance(ground + DVec3::Z) < 0.01);
        assert!(Save::from_bytes(b"not a save").is_err());
    }

    #[test]
    fn walking_input_moves_the_player_on_the_ground() {
        let (mut s, links) = server_with(1);
        drain(&links[0]);
        let start = DVec3::from_array(s.clients[0].player.pos);
        links[0].send(ToServer::Input(Input { mv: [1.0, 0.0], jump: true, fwd: [1.0, 0.0, 0.0], pitch: 0.0 }));
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
    fn a_closed_link_removes_the_player() {
        let (mut s, mut links) = server_with(2);
        drop(links.remove(0));
        s.tick();
        assert_eq!(s.players(), 1);
    }
}
