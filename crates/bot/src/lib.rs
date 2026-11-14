//! A bot: a player on the protocol with no window (ADR 0002, 0006). A script of commands drives it, from
//! `script=` or live from stdin; it answers with one JSON line per `report` or `block`.
//!
//! A bot is a walking player: the server moves it with the shared physics (ADR 0007) and the bot steers.
//! `spawn` makes it a flying one that stays where it put itself, until the next `walk`.
//!
//! Commands (numbers are metres in world axes; `dx dy dz` is relative to the feet):
//!   spawn x y z         teleport the feet      walk dx dy dz       walk there over the ground
//!   wait s              idle s seconds         dig dx dy dz        set that block to air
//!   place dx dy dz m    set that block to m    block dx dy dz      report that block's material
//!   report              report position, players and edits seen    quit
//! Items (ADR 0074; slots are the ape's: 0-8 the hotbar, 9 the worn pack, 10 on its rows):
//!   slots               report every slot held  move a b n         n from slot a to slot b
//!   drop a n            throw n from slot a     take dx dy dz k a n  n from slot k of the pile there to a
//!   put a dx dy dz k n  n from slot a to slot k of the pile there  grab item a n  creative's grid: n into a
//! Tools (ADR 0110): `hold k` takes hotbar slot k in hand (a survival dig uses its tool); `strike dx dy dz` strikes
//! what is in hand on that block (the place button with flint: knapping). `slots` gives a tool's digs left too.
use engine::edit::Shape;
use engine::items::{Holder, Slot, ITEMS};
use engine::net::{Link, Recv, ToClient, ToServer, TICK_HZ};
use engine::phys::{self, Input};
use engine::world::World;
use glam::DVec3;
use std::collections::VecDeque;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

/// A walk ends this close to its target, measured along the ground.
const ARRIVE: f64 = 0.5;

#[derive(Clone, Debug, PartialEq)]
pub enum Cmd {
    Spawn(DVec3),
    Walk(DVec3),
    Wait(f64),
    Dig(DVec3),
    Place(DVec3, u16),
    Block(DVec3),
    Report,
    /// A chat line or a '/' command (ADR 0023): the rest of the line.
    Say(String),
    /// Items (ADR 0074): report the slots; a move within them; a throw; to and from the pile at a cell; from
    /// creative's grid (an item by name).
    Slots,
    Shift(u16, u16, u16),
    Drop(u16, u16),
    Take(DVec3, u16, u16, u16),
    Put(u16, DVec3, u16, u16),
    Grab(u16, u16, u16),
    /// Take the pick-up lying in the cell at this offset from the feet (ADR 0082).
    Pick(DVec3),
    /// Hotbar slot k in hand; strike what is in hand on the block at this offset (ADR 0110).
    Hold(u8),
    Strike(DVec3),
    Quit,
}

/// Parses one command; `None` for an empty line or a comment, an error message for a bad one.
pub fn parse(line: &str) -> Result<Option<Cmd>, String> {
    let w: Vec<&str> = line.split_whitespace().collect();
    if w.is_empty() || w[0].starts_with('#') {
        return Ok(None);
    }
    let num = |i: usize| w.get(i).and_then(|s| s.parse::<f64>().ok()).ok_or(format!("bad number in '{line}'"));
    let v = || Ok::<DVec3, String>(DVec3::new(num(1)?, num(2)?, num(3)?));
    Ok(Some(match w[0] {
        "spawn" => Cmd::Spawn(v()?),
        "walk" => Cmd::Walk(v()?),
        "wait" => Cmd::Wait(num(1)?),
        "dig" => Cmd::Dig(v()?),
        "place" => Cmd::Place(v()?, num(4)? as u16),
        "block" => Cmd::Block(v()?),
        "report" => Cmd::Report,
        "say" => Cmd::Say(w[1..].join(" ")),
        "slots" => Cmd::Slots,
        "move" => Cmd::Shift(num(1)? as u16, num(2)? as u16, num(3)? as u16),
        "drop" => Cmd::Drop(num(1)? as u16, num(2)? as u16),
        "take" => Cmd::Take(v()?, num(4)? as u16, num(5)? as u16, num(6)? as u16),
        "put" => Cmd::Put(num(1)? as u16, DVec3::new(num(2)?, num(3)?, num(4)?), num(5)? as u16, num(6)? as u16),
        "grab" => {
            // the item's name may have spaces: every word but the last two
            let name = w.get(1..w.len().saturating_sub(2)).unwrap_or_default().join(" ");
            let item = ITEMS.named(&name).ok_or(format!("no block or item '{name}' in '{line}'"))?;
            Cmd::Grab(item, num(w.len() - 2)? as u16, num(w.len() - 1)? as u16)
        }
        "pick" => Cmd::Pick(v()?),
        "hold" => Cmd::Hold(num(1)? as u8),
        "strike" => Cmd::Strike(v()?),
        "quit" => Cmd::Quit,
        _ => return Err(format!("unknown command '{}'", w[0])),
    }))
}

/// `a; b; c` or one command per line.
pub fn parse_script(s: &str) -> Result<Vec<Cmd>, String> {
    s.split(|c| c == ';' || c == '\n').filter_map(|l| parse(l).transpose()).collect()
}

pub struct Bot {
    pub name: String,
    pub link: Link<ToServer, ToClient>,
    pub world: Option<World>,
    /// The feet: where the server has this bot, or where it put itself with `spawn`.
    pub pos: DVec3,
    pub players: usize,
    pub edits_applied: u64,
    /// Edits the server refused (`Done` with ok false).
    pub refused: u64,
    /// Its slots as the server last sent them (ADR 0074).
    pub slots: Vec<Slot>,
    seq: u32,
    id: u32,
    flying: bool,
    /// The server's ticks seen (one `You` a tick): the bot's clock. Its waits, walks and held digs count these,
    /// not wall time, so a server slowed by a busy PC is waited for (the gate's tests under load).
    ticks: u64,
    /// The walk: where to, and the tick to give up at.
    target: Option<(DVec3, u64)>,
    last_dist: f64,
    notes: Vec<String>,
    /// The live air where it stands, as the server last sent it (W3).
    air: Option<engine::weather::Here>,
    idle_until: Option<u64>,
    start: Instant,
    /// Plays creative (bots start so, ADR 0074); `say /playmode survival` turns it off, and its digs are then held
    /// for their time as a player's are (ADR 0034, 0097).
    creative: bool,
    /// A survival dig held until that tick, of that shape.
    digging: Option<(u64, Shape)>,
    /// The hotbar slot in hand (`hold`).
    hand: u8,
    /// The creature kinds by index and the animals the server has this bot see (ADR 0083).
    pub kinds: Vec<String>,
    pub animals: std::collections::BTreeMap<u32, engine::net::Animal>,
}

impl Bot {
    pub fn new(name: &str, link: Link<ToServer, ToClient>) -> Bot {
        link.send(ToServer::Hello { name: name.to_string() });
        Bot { name: name.into(), link, world: None, pos: DVec3::ZERO, players: 0, edits_applied: 0, refused: 0, slots: Vec::new(), seq: 0, id: 0, flying: false, ticks: 0, target: None, last_dist: 0.0, notes: Vec::new(), air: None, idle_until: None, start: Instant::now(), creative: true, digging: None, hand: 0, kinds: Vec::new(), animals: Default::default() }
    }

    /// The tick `s` seconds of the server's time from now.
    fn after(&self, s: f64) -> u64 {
        self.ticks + (s * TICK_HZ).ceil() as u64
    }

    fn feet(&self) -> DVec3 {
        self.pos
    }

    /// Takes everything the server sent; false once the server is gone.
    pub fn receive(&mut self) -> bool {
        loop {
            match self.link.try_recv() {
                Recv::Msg(ToClient::Welcome { id, seed, planet_n, moon_n, blocks, items, map, luna }) => {
                    assert_eq!(blocks, engine::blocks::BLOCKS.hash, "the server's build has other blocks (data/blocks.toml)");
                    assert_eq!(items, engine::items::ITEMS.hash, "the server's build has other items (data/items.toml)");
                    // the baked map (ADR 0043): shared with a server in this process, from the disk, or fetched
                    let map = map.map(|h| engine::net::fetch_map(&mut self.link, seed, planet_n, h).unwrap_or_else(|e| panic!("{e}")));
                    let luna = luna.map(|h| engine::net::fetch_map(&mut self.link, seed, moon_n, h).unwrap_or_else(|e| panic!("{e}")));
                    let w = World::with_maps(planet_n, moon_n, seed, map, luna);
                    self.id = id;
                    // the server spawns players just over the +y pole; an empty input makes this one a
                    // walker, so it lands there
                    self.pos = w.bodies[0].spawn_point();
                    self.world = Some(w);
                    self.link.send(ToServer::Input(Input::default()));
                    // bots build in creative: a script places without digging first
                    self.link.send(ToServer::Creative(true));
                }
                Recv::Msg(ToClient::Edits(v)) => {
                    if let Some(w) = self.world.as_mut() {
                        for (b, m) in v {
                            w.set_block(b, m);
                            self.edits_applied += 1;
                        }
                    }
                }
                // the list comes when it changes; this bot's own place every tick in `You`
                Recv::Msg(ToClient::Players(p)) => self.players = p.len(),
                Recv::Msg(ToClient::You { me, .. }) => {
                    self.ticks += 1;
                    if !self.flying {
                        self.pos = me.pos;
                    }
                }
                Recv::Msg(ToClient::Done { ok, .. }) => self.refused += !ok as u64,
                Recv::Msg(ToClient::Inventory { slots, .. }) => self.slots = slots,
                Recv::Msg(ToClient::Vitals { dead: None, .. } | ToClient::Digging { .. } | ToClient::Map { .. } | ToClient::Piles(_) | ToClient::Taken(_)) => {}
                // a bot that dies respawns at once (ADR 0040); it builds in creative, so only a switch to
                // survival can get it there
                Recv::Msg(ToClient::Vitals { dead: Some(_), .. }) => self.link.send(ToServer::Respawn),
                Recv::Msg(ToClient::Air(h)) => self.air = Some(h),
                Recv::Msg(ToClient::Kinds(k)) => self.kinds = k,
                Recv::Msg(ToClient::Animals { seen, gone }) => {
                    for id in gone {
                        self.animals.remove(&id);
                    }
                    self.animals.extend(seen.into_iter().map(|a| (a.id, a)));
                }
                Recv::Msg(ToClient::Sky { .. }) => {}
                Recv::Msg(ToClient::Chat(t)) => self.notes.push(format!("{{\"bot\":{:?},\"chat\":{t:?}}}", self.name)),
                Recv::Msg(ToClient::Water(v)) => {
                    if let Some(w) = self.world.as_mut() {
                        for (t, cols) in &v {
                            w.water.set_tile(*t, cols);
                        }
                    }
                }
                Recv::Empty => return true,
                Recv::Closed => return false,
            }
        }
    }

    /// One tick: receive, steer the walk towards its target. Returns false when the link closed.
    pub fn tick(&mut self) -> bool {
        if !self.receive() {
            return false;
        }
        let Some(w) = self.world.as_ref() else { return true };
        if let Some((_, shape)) = self.digging.filter(|d| self.ticks >= d.0) {
            self.seq += 1;
            self.link.send(ToServer::Edit { seq: self.seq, shape, material: 0 });
            self.link.send(ToServer::Dig(None));
            self.digging = None;
        }
        if let Some((t, deadline)) = self.target {
            let d = (t - self.pos).reject_from(phys::up(w, self.pos));
            let dist = d.length();
            let late = self.ticks > deadline;
            if dist < ARRIVE || late {
                if late {
                    self.notes.push(format!("{{\"bot\":\"{}\",\"walk\":\"gave up\",\"left\":{:.2}}}", self.name, dist));
                }
                self.target = None;
                self.link.send(ToServer::Input(Input::default()));
            } else {
                // jump when the last tick brought it no closer: a ledge is in the way
                let jump = dist > self.last_dist - 0.02;
                self.link.send(ToServer::Input(Input { mv: [1.0, 0.0], jump, fwd: (d / dist).as_vec3().to_array(), pitch: 0.0, ..Default::default() }));
            }
            self.last_dist = dist;
        }
        true
    }

    /// True while a walk or wait is still going, so the next command has to hold.
    pub fn busy(&self) -> bool {
        self.target.is_some() || self.digging.is_some() || self.idle_until.is_some_and(|u| self.ticks < u)
    }

    /// Runs one command; the JSON line it produces, if any. Quit is the caller's to notice.
    pub fn exec(&mut self, c: &Cmd) -> Option<String> {
        let w = self.world.as_ref().expect("welcomed");
        match c {
            Cmd::Spawn(p) => {
                (self.pos, self.flying, self.target) = (*p, true, None);
                self.link.send(ToServer::Move { pos: p.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0 });
            }
            Cmd::Walk(d) => {
                // three times the time the distance takes, and 2 s: enough for hills, not for a cliff
                let give_up = self.after(d.length() / phys::WALK * 3.0 + 2.0);
                (self.flying, self.last_dist, self.target) = (false, f64::MAX, Some((self.pos + *d, give_up)));
            }
            Cmd::Wait(s) => self.idle_until = Some(self.after(*s)),
            Cmd::Dig(d) if !self.creative => {
                // held for its time, a little over, then dug (the server checks the hold, ADR 0034)
                let shape = Shape::one(w.block_at(self.feet() + *d));
                self.link.send(ToServer::Dig(Some(shape)));
                let tool = engine::items::tool(self.slots.get(self.hand as usize).copied().flatten());
                let need = engine::edit::dig_time_with(w, &shape, tool) as f64;
                self.digging = Some((self.after(need * 1.1 + 0.1), shape));
            }
            Cmd::Dig(d) | Cmd::Place(d, _) => {
                let m = if let Cmd::Place(_, m) = c { *m } else { 0 };
                self.seq += 1;
                self.link.send(ToServer::Edit { seq: self.seq, shape: Shape::one(w.block_at(self.feet() + *d)), material: m });
            }
            Cmd::Hold(k) => {
                self.hand = *k;
                self.link.send(ToServer::Hand(*k));
            }
            Cmd::Strike(d) => self.link.send(ToServer::Strike(w.block_at(self.feet() + *d))),
            Cmd::Pick(d) => {
                self.seq += 1;
                self.link.send(ToServer::Take { seq: self.seq, at: w.block_at(self.feet() + *d) });
            }
            Cmd::Block(d) => {
                let b = w.block_at(self.feet() + *d);
                return Some(format!(
                    "{{\"bot\":\"{}\",\"block\":[{},{},{}],\"face\":{},\"pos\":[{},{},{}],\"material\":{}}}",
                    self.name, d.x, d.y, d.z, b.face, b.pos.x, b.pos.y, b.pos.z, w.block(b)
                ));
            }
            Cmd::Report => {
                return Some(format!(
                    "{{\"bot\":\"{}\",\"t\":{:.2},\"pos\":[{:.2},{:.2},{:.2}],\"players\":{},\"edits_applied\":{},\"refused\":{},\"air\":{},\"animals\":{}}}",
                    self.name,
                    self.start.elapsed().as_secs_f64(),
                    self.pos.x,
                    self.pos.y,
                    self.pos.z,
                    self.players,
                    self.edits_applied,
                    self.refused,
                    self.air.map_or("null".into(), |a| format!("{{\"temp\":{:.2},\"rain_mm_h\":{:.3},\"cover\":{:.2},\"wind\":[{:.2},{:.2}]}}", a.temp, a.rain, a.cover, a.wind[0], a.wind[1])),
                    self.animals_json()
                ));
            }
            Cmd::Say(t) => {
                // the play mode it asks for (the server answers in chat; shortcut: a refusal is not read back)
                match t.split_whitespace().collect::<Vec<_>>()[..] {
                    ["/playmode" | "/gamemode", m, ..] => match m.to_lowercase().as_str() {
                        "creative" | "c" | "1" => self.creative = true,
                        "survival" | "s" | "0" => self.creative = false,
                        _ => {} // the server refuses it and keeps the mode
                    },
                    _ => {}
                }
                self.link.send(ToServer::Chat(t.clone()))
            }
            Cmd::Slots => {
                // a tool's digs left as a fourth number
                let left = |s: Option<engine::items::Stack>| engine::items::left(s).map_or(String::new(), |n| format!(",{n}"));
                let held: Vec<String> = self.slots.iter().enumerate().filter_map(|(k, s)| s.map(|st| format!("[{k},{:?},{}{}]", ITEMS.name(st.item), st.n, left(*s)))).collect();
                return Some(format!("{{\"bot\":\"{}\",\"slots\":[{}]}}", self.name, held.join(",")));
            }
            Cmd::Shift(..) | Cmd::Drop(..) | Cmd::Take(..) | Cmd::Put(..) | Cmd::Grab(..) => {
                let pile = |d: &DVec3| Holder::Pile(w.block_at(self.feet() + *d));
                let (from, to, n) = match c {
                    Cmd::Shift(a, b, n) => ((Holder::Ape, *a), (Holder::Ape, *b), *n),
                    Cmd::Drop(a, n) => ((Holder::Ape, *a), (Holder::Ground, 0), *n),
                    Cmd::Take(d, k, a, n) => ((pile(d), *k), (Holder::Ape, *a), *n),
                    Cmd::Put(a, d, k, n) => ((Holder::Ape, *a), (pile(d), *k), *n),
                    Cmd::Grab(item, a, n) => ((Holder::Grid, *item), (Holder::Ape, *a), *n),
                    _ => unreachable!(),
                };
                self.seq += 1;
                self.link.send(ToServer::Items { seq: self.seq, from, to, n });
            }
            Cmd::Quit => {}
        }
        None
    }

    /// The animals it sees (ADR 0083): how many, and the nearest five with kind, distance and what they do.
    pub fn animals_json(&self) -> String {
        const ACTS: [&str; 5] = ["stand", "graze", "wander", "flee", "sleep"];
        let mut near: Vec<(f64, &engine::net::Animal)> = self.animals.values().map(|a| ((DVec3::from_array(a.pos) - self.pos).length(), a)).collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        let list: Vec<String> = near.iter().take(5).map(|(d, a)| {
            let kind = self.kinds.get(a.kind as usize).map_or("?", |k| k.as_str());
            format!("{{\"id\":{},\"kind\":{kind:?},\"d\":{d:.1},\"act\":{:?}}}", a.id, ACTS.get(a.act as usize).unwrap_or(&"?"))
        }).collect();
        format!("{{\"seen\":{},\"nearest\":[{}]}}", self.animals.len(), list.join(","))
    }

    /// Runs the script at TICK_HZ, then commands from `live` as they come (none: stop after the script),
    /// calling `out` for every JSON line. Ends on `quit`, a closed link, or the script's end without `live`.
    pub fn run(&mut self, script: Vec<Cmd>, live: Option<Receiver<Cmd>>, mut out: impl FnMut(String)) {
        let mut queue: VecDeque<Cmd> = script.into();
        let step = Duration::from_secs_f64(1.0 / TICK_HZ);
        loop {
            if !self.tick() {
                return;
            }
            self.notes.drain(..).for_each(&mut out);
            if let Some(rx) = &live {
                while let Ok(c) = rx.try_recv() {
                    queue.push_back(c);
                }
            }
            while self.world.is_some() && !self.busy() {
                let Some(c) = queue.pop_front() else {
                    if live.is_none() {
                        return;
                    }
                    break;
                };
                if c == Cmd::Quit {
                    return;
                }
                if let Some(line) = self.exec(&c) {
                    out(line);
                }
            }
            std::thread::sleep(step);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::net::local;
    use server::Server;
    use std::sync::mpsc::channel;

    // These servers tick at the game's own rate (they ran at 200 Hz): a bot's clock is the server's ticks, 20 a
    // second, so a script's `wait 0.5` is ten ticks, however long a busy PC takes over them.

    #[test]
    fn parse_script_and_errors() {
        let s = parse_script("spawn 1 2 3; wait 0.5\n# a comment\ndig 0 -1 0; place 1 0 0 3; block 1 0 0; report; quit").unwrap();
        assert_eq!(s.len(), 7);
        assert_eq!(s[3], Cmd::Place(DVec3::new(1.0, 0.0, 0.0), 3));
        let items = parse_script("slots; move 0 4 3; drop 1 2; take 0 -0.5 1 2 5 6; put 5 0 -0.5 1 0 6; grab blue lamp 2 64").unwrap();
        assert_eq!(items[3], Cmd::Take(DVec3::new(0.0, -0.5, 1.0), 2, 5, 6));
        assert_eq!(items[5], Cmd::Grab(engine::blocks::BLOCKS.id("blue lamp"), 2, 64));
        assert!(parse("grab nothing 1 2").is_err());
        assert!(parse("fly 1 2 3").is_err());
        assert!(parse("walk 1 x 3").is_err());
    }

    #[test]
    fn one_bot_digs_and_the_other_sees_the_hole() {
        let (tx, rx) = channel();
        std::thread::spawn(move || Server::new(256, 32, 1, rx).run(TICK_HZ, &std::sync::atomic::AtomicBool::new(false)));
        let mut bots: Vec<Bot> = ["digger", "watcher"]
            .iter()
            .map(|n| {
                let (c, s) = local();
                tx.send(s).unwrap();
                Bot::new(n, c)
            })
            .collect();
        let watcher = bots.pop().unwrap();
        let mut digger = bots.pop().unwrap();
        // the digger stands at the +y pole and the watcher 2 m aside; the digger makes a 2-deep hole next to
        // its feet, and the watcher reads those blocks from the deltas it received (offsets aim at the
        // middle of a column: the pole itself is a column corner)
        let h = World::new(256, 32, 1).bodies[0].height(DVec3::Y);
        let w = std::thread::spawn(move || {
            let mut watcher = watcher;
            let mut out = Vec::new();
            watcher.run(parse_script(&format!("spawn 2 {h} 0; wait 0.5; block -1.75 -0.5 0.25; block -1.75 -1.5 0.25; report")).unwrap(), None, |l| out.push(l));
            out
        });
        let mut dout = Vec::new();
        digger.run(parse_script(&format!("spawn 0 {h} 0; wait 0.2; dig 0.25 -0.5 0.25; dig 0.25 -1.5 0.25; wait 0.2; block 0.25 -0.5 0.25; report; wait 1")).unwrap(), None, |l| dout.push(l));
        let (wchat, wout): (Vec<String>, Vec<String>) = w.join().unwrap().into_iter().partition(|l| l.contains("\"chat\""));
        let dout: Vec<String> = dout.into_iter().filter(|l| !l.contains("\"chat\"")).collect();
        assert!(wchat.iter().any(|l| l.contains("digger joined")), "{wchat:?}");
        assert!(dout[0].contains("\"material\":0"), "{dout:?}");
        assert!(wout[0].contains("\"material\":0") && wout[1].contains("\"material\":0"), "{wout:?}");
        assert!(wout[2].contains("\"players\":2") && wout[2].contains("\"edits_applied\":2"), "{wout:?}");
    }

    /// Flint by script (ADR 0097): a survival bot digs a generated gravel cell that holds a flint, holding the dig
    /// for its time as a player does, walks onto the pile, and has the gravel and the flint in its slots.
    #[test]
    fn a_survival_bot_digs_flint_out_of_gravel() {
        let w = World::baked(1536, 32, 7);
        let b = &w.bodies[0];
        let (gravel, flint) = (engine::blocks::BLOCKS.id("gravel"), engine::items::ITEMS.named("flint").unwrap());
        let sea = b.sea.unwrap();
        let cell = (0..6u8)
            .flat_map(|f| (40..b.n - 40).step_by(3).flat_map(move |i| (40..b.n - 40).step_by(3).map(move |j| (f, i, j))))
            .find_map(|(f, i, j)| {
                let top = w.air_over_ground(0, b.dir(f as usize, i as f64 + 0.5, j as f64 + 0.5));
                let c = engine::world::Block { pos: top.pos - glam::IVec3::Z, ..top };
                (c.pos.z as f64 > sea && w.block(c) == gravel && engine::edit::held(&w, c, gravel) == Some(flint)).then_some(c)
            })
            .expect("a gravel cell with flint");
        let up = b.dir(cell.face as usize, cell.pos.x as f64 + 0.5, cell.pos.y as f64 + 0.5);
        let centre = w.block_centre(cell);
        let feet = centre + up * 2.0;
        let d = centre - feet;
        let (tx, rx) = channel();
        std::thread::spawn(move || Server::baked(1536, 32, 7, rx).run(TICK_HZ, &std::sync::atomic::AtomicBool::new(false)));
        let (c, s) = local();
        tx.send(s).unwrap();
        let mut bot = Bot::new("knapper", c);
        let mut out = Vec::new();
        let script = format!("wait 0.5; say /playmode survival; spawn {} {} {}; wait 0.5; dig {} {} {}; wait 0.5; walk {} {} {}; wait 3; slots", feet.x, feet.y, feet.z, d.x, d.y, d.z, d.x, d.y, d.z);
        bot.run(parse_script(&script).unwrap(), None, |l| out.push(l));
        let line = out.iter().find(|l| l.contains("\"slots\"")).expect("a slots line");
        assert!(line.contains("\"flint\",1") && line.contains("\"gravel\",1"), "{line} ({} refused)", bot.refused);
    }

    /// Knapping and the first tool by script (ADR 0110): a survival bot strikes flint four times on granite under
    /// its feet, holds the hand axe it became, digs the granite with it in the axe's time, and has one use less.
    #[test]
    fn a_survival_bot_knaps_a_hand_axe_and_digs_with_it() {
        let (tx, rx) = channel();
        std::thread::spawn(move || Server::new(256, 32, 1, rx).run(TICK_HZ, &std::sync::atomic::AtomicBool::new(false)));
        let (c, s) = local();
        tx.send(s).unwrap();
        let mut bot = Bot::new("knapper", c);
        let w = World::new(256, 32, 1);
        let h = w.bodies[0].height(DVec3::Y);
        // granite in the ground a metre beside the feet (/place by face, i, j, layer: not under the bot)
        let (d, at) = ("1.0 -0.5 0.25", w.block_at(DVec3::new(1.25, h - 0.5, 0.5)));
        let mut out = Vec::new();
        let strikes = format!("strike {d}; wait 0.5; ").repeat(4);
        let script = format!("wait 0.5; say /playmode survival; spawn 0.25 {h} 0.25; wait 0.5; say /place granite {} {} {} {}; say /give flint 2; hold 0; wait 0.5; slots; {strikes}slots; hold 1; dig {d}; wait 0.5; slots; block {d}", at.face, at.pos.x, at.pos.y, at.pos.z);
        bot.run(parse_script(&script).unwrap(), None, |l| out.push(l));
        let slots: Vec<&String> = out.iter().filter(|l| l.contains("\"slots\"")).collect();
        let uses = ITEMS.get(ITEMS.named("hand axe").unwrap()).unwrap().tool.as_ref().unwrap().uses;
        assert!(slots[0].contains("[0,\"flint\",2]"), "{out:?}");
        assert!(slots[1].contains(&format!("[0,\"flint\",1]")) && slots[1].contains(&format!("[1,\"hand axe\",1,{uses}]")), "{out:?}");
        assert!(slots[2].contains(&format!("[1,\"hand axe\",1,{}]", uses - 1)), "{out:?} ({} refused)", bot.refused);
        let rubble = engine::blocks::BLOCKS.id("granite rubble");
        assert!(out.last().unwrap().contains(&format!("\"material\":{rubble}")), "{out:?}");
    }

    /// A bot drives the items (ADR 0074): given stone, it moves some along its slots, throws some, and
    /// reports what it holds.
    #[test]
    fn a_bot_moves_throws_and_reports_its_slots() {
        let (tx, rx) = channel();
        std::thread::spawn(move || Server::new(256, 32, 1, rx).run(TICK_HZ, &std::sync::atomic::AtomicBool::new(false)));
        let (c, s) = local();
        tx.send(s).unwrap();
        let mut b = Bot::new("hauler", c);
        let mut out = Vec::new();
        b.run(parse_script("wait 0.5; say /give stone 10; wait 0.3; move 0 4 3; drop 0 2; grab blue lamp 2 64; wait 0.3; slots").unwrap(), None, |l| out.push(l));
        let line = out.iter().find(|l| l.contains("\"slots\"")).expect("a slots line");
        assert!(line.contains("[0,\"stone\",5]") && line.contains("[4,\"stone\",3]") && line.contains("[2,\"blue lamp\",64]"), "{line}");
    }

    #[test]
    fn a_bot_lands_and_walks_ten_metres_over_the_ground() {
        let (tx, rx) = channel();
        std::thread::spawn(move || Server::new(256, 32, 1, rx).run(TICK_HZ, &std::sync::atomic::AtomicBool::new(false)));
        let (c, s) = local();
        tx.send(s).unwrap();
        let mut b = Bot::new("walker", c);
        let mut out = Vec::new();
        b.run(parse_script("wait 1").unwrap(), None, |l| out.push(l));
        let start = b.pos;
        b.run(parse_script("walk 10 0 0; wait 1.5").unwrap(), None, |l| out.push(l));
        let moved = b.pos - start;
        assert!(out.iter().all(|l| l.contains("\"chat\"")), "{out:?}");
        assert!((moved.x - 10.0).abs() < 1.0 && moved.z.abs() < 1.0, "moved {moved:?}");
        let w = b.world.as_ref().unwrap();
        let up = phys::up(w, b.pos);
        assert!(!phys::blocked(w, b.pos) && phys::blocked(w, b.pos - up * 0.05), "feet on the ground at {:?}", b.pos);
    }
}
