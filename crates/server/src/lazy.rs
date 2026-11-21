//! Server: bodies loaded on approach (step 6b). A body past the home's moon (Mars) is a stub until a player is in
//! its frame, a /goto is bound for it, or a client asks for its map: then its ground is built on a thread of its
//! own (never the tick, never the global pool) and put in at the start of the tick it is done, a tick the
//! recording keeps (`Ev::Body`) so a replay puts it in at the same one. The server never lets one go again.
use super::*;

/// What the server waits on before a body is in: the loads running and what waits for them.
#[derive(Default)]
pub(crate) struct Loads {
    /// Bodies being built, each on its own thread, and the end of the channel it answers on.
    running: Vec<(usize, Receiver<Result<engine::world::Body, String>>)>,
    /// A replay's bodies to put in at the next tick's start (`Ev::Body`): built there, on its own thread.
    pub(crate) replay: Vec<usize>,
    /// Whether loads come from the recording alone (a replay): none is started here.
    pub(crate) replaying: bool,
    /// Bodies whose map this build cannot make: never asked again.
    pub(crate) failed: Vec<usize>,
    /// /goto's bound for a body not in yet: (player id, where to, as typed).
    pub(crate) gotos: Vec<(u32, String)>,
    /// Map downloads asked for a body not in yet: (player id, hash).
    pub(crate) maps: Vec<(u32, u64)>,
}

impl Server {
    /// The tick's first step for lazy bodies: those done go in (logged), those now wanted start, and what waited
    /// for one that is in goes on.
    pub(super) fn bodies_in(&mut self) {
        if self.world.lazy.iter().all(|l| l.as_ref().is_none_or(|l| l.loaded)) && self.loads.gotos.is_empty() && self.loads.maps.is_empty() {
            return;
        }
        let mut done: Vec<(usize, Result<engine::world::Body, String>)> = Vec::new();
        self.loads.running.retain(|(i, rx)| match rx.try_recv() {
            Ok(r) => {
                done.push((*i, r));
                false
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => true,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                done.push((*i, Err("its loader stopped".into())));
                false
            }
        });
        for i in std::mem::take(&mut self.loads.replay) {
            // (a replay waits for it: the recording says it was in by this tick)
            if let Some(load) = self.world.loader(i) {
                done.push((i, std::thread::spawn(load).join().unwrap_or_else(|_| Err("its loader panicked".into()))));
            }
        }
        for (i, r) in done {
            match r {
                Ok(b) => self.put_body(i, b),
                Err(e) => {
                    eprintln!("[technoape-server] {}: {e}", self.world.bodies[i].name);
                    self.loads.failed.push(i);
                }
            }
        }
        // wanted: a player in its frame, a /goto or a map bound for it
        let world = &self.world;
        let want: Vec<usize> = (0..world.bodies.len())
            .filter(|&i| !world.loaded(i))
            .filter(|&i| {
                self.clients.iter().any(|c| world.frame_at(c.phys.pos) == i)
                    || self.loads.gotos.iter().any(|(_, to)| engine::bodies::find(to) == Some(i))
                    || self.loads.maps.iter().any(|&(_, h)| world.map_hash(i) == Some(h))
            })
            .collect();
        for i in want {
            self.want_body(i);
        }
        self.waiting_go_on();
    }

    /// Starts building body i on a thread of its own, unless it is in, building, failed, or this is a replay.
    pub(super) fn want_body(&mut self, i: usize) {
        if self.loads.replaying || self.loads.failed.contains(&i) || self.loads.running.iter().any(|r| r.0 == i) {
            return;
        }
        let Some(load) = self.world.loader(i) else { return };
        let (tx, rx) = channel();
        let name = self.world.bodies[i].name.clone();
        std::thread::Builder::new()
            .name(format!("load {name}"))
            .spawn(move || {
                let t = Instant::now();
                let r = load();
                eprintln!("[technoape-server] {name} loaded in {:.2} s", t.elapsed().as_secs_f64());
                let _ = tx.send(r);
            })
            .expect("a thread to load a body on");
        self.loads.running.push((i, rx));
    }

    /// Body i's ground goes in: the world's, the ore ledger's opening, the recording's.
    fn put_body(&mut self, i: usize, b: engine::world::Body) {
        let at = self.world.ore_base(i);
        self.ore_open.splice(at..at, engine::ore::count(&b).into_iter().map(|h| h.blocks as i64));
        self.world.put(i, b);
        // one teleported under its ground while it was not in stands on it now (held ones above go on as they were)
        for c in self.clients.iter_mut().filter(|c| self.world.frame_at(c.phys.pos) == i) {
            phys::onto_ground(&self.world, &mut c.phys);
            c.player.pos = c.phys.pos.to_array();
        }
        self.log(Ev::Body { tick: self.tick, body: i as u8 });
    }

    /// The /goto's and map downloads that waited for a body now in.
    fn waiting_go_on(&mut self) {
        let world = &self.world;
        let ready = |to: &str| engine::bodies::find(to).is_none_or(|i| world.loaded(i));
        let (go, wait): (Vec<_>, Vec<_>) = std::mem::take(&mut self.loads.gotos).into_iter().partition(|(_, to)| ready(to));
        self.loads.gotos = wait;
        for (id, to) in go {
            let Some(i) = self.clients.iter().position(|c| c.player.id == id) else { continue };
            if let Some((p, look)) = self.goto(&to) {
                self.put(i, p, look);
                let name = engine::bodies::find(&to).map_or("the sun", |b| self.world.bodies[b].name.as_str()).to_string();
                self.clients[i].link.send(ToClient::Chat(format!("gone to {name}")));
            }
        }
        let maps = std::mem::take(&mut self.loads.maps);
        for (id, h) in maps {
            // (one for a body that cannot be loaded is dropped: it would wait for ever)
            if self.loads.failed.iter().any(|&b| self.world.map_hash(b) == Some(h)) {
                continue;
            }
            if !self.world.bodies.iter().any(|b| b.map().is_some_and(|m| m.hash == h)) {
                self.loads.maps.push((id, h));
                continue;
            }
            if let Some(i) = self.clients.iter().position(|c| c.player.id == id) {
                self.send_map(i, h);
            }
        }
    }

    /// Every lazy body in now, on this thread: a replay of a recording from before bodies came in late, which
    /// had them all from the start.
    pub fn load_all(&mut self) {
        for i in 0..self.world.bodies.len() {
            if let Some(load) = self.world.loader(i) {
                match load() {
                    Ok(b) => {
                        let at = self.world.ore_base(i);
                        self.ore_open.splice(at..at, engine::ore::count(&b).into_iter().map(|h| h.blocks as i64));
                        self.world.put(i, b);
                    }
                    Err(e) => panic!("{e}"),
                }
            }
        }
    }

    /// Sends client i the baked map of hash h, when a body stands on it.
    pub(super) fn send_map(&mut self, i: usize, h: u64) {
        if let Some(m) = self.world.bodies.iter().filter_map(|b| b.map()).find(|m| m.hash == h) {
            if !self.clients[i].maps.contains(&h) {
                self.clients[i].maps.push(h);
            }
            for p in self.map_pieces.entry(h).or_insert_with(|| engine::net::map_pieces(m)) {
                self.clients[i].link.send(p.clone());
            }
        }
    }

    /// Whether message m of client i is refused because the body it is on has no ground yet: an edit, dig,
    /// bucket, pick-up, strike, use, drink or items to or from the ground there. A refused one with a `seq` is
    /// answered `Done` false; the client is told why once a tick (`told`).
    pub(super) fn refused_on_a_stub(&mut self, i: usize, m: &ToServer, told: &mut bool) -> bool {
        let world_side = |h: &(Holder, u16)| !matches!(h.0, Holder::Ape | Holder::Grid);
        let seq = match m {
            ToServer::Edit { seq, .. } | ToServer::Bucket { seq, .. } | ToServer::Take { seq, .. } => Some(*seq),
            ToServer::Items { seq, from, to, .. } if world_side(from) || world_side(to) => Some(*seq),
            ToServer::Dig(Some(_)) | ToServer::Strike(_) | ToServer::Use(_) | ToServer::Choose { .. } | ToServer::Drink(Some(_)) => None,
            _ => return false,
        };
        let b = self.world.frame_at(self.clients[i].phys.pos);
        if b >= self.world.bodies.len() || self.world.loaded(b) {
            return false;
        }
        let c = &mut self.clients[i];
        if let Some(seq) = seq {
            c.done.push((seq, false));
            c.inv_dirty = true;
        }
        if !std::mem::replace(told, true) {
            c.link.send(ToClient::Chat(format!("{}'s ground is not in yet: nothing there can be changed", self.world.bodies[b].name)));
        }
        true
    }
}
