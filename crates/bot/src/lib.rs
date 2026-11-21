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
//! Prospecting (ADR 0138): `prospect dx dy dz` strikes the prospecting tool in hand on
//! that block and waits (up to 2 s) for the answer, one JSON line: `{"bot":..,"prospect":[["copper","strong"],..]}`,
//! an empty list when the rock rings empty.
//! Crafting (ADR 0140), as a player does: the grid's cells are slots 37 to 45 (`items::CRAFT_AT`, a
//! 3 x 3 row by row; 37, 38, 40, 41 the inventory's 2 x 2), filled by `move`; `craft [n]` takes the output n times
//! (1 if left out; as many as the cells hold). `use dx dy dz` uses the block there (a workbench opens its 3 x 3),
//! `close` closes the screen (the cells back into the pack). `slots` shows what came of it after a tick.
//! Furnaces (ADR 0141): `use dx dy dz` on a crucible opens its window; `put` and `take` at its cell reach
//! its slots (0 what goes in, 1 the fuel, 2 the mould, 3 what comes out, 4 the second input for an alloy) in place
//! of a pile's; `cast dx dy dz name` chooses its cast's shape by any row of it (`cast dx dy dz` alone none); `furnace` reports the open one: slots, heat, fire and work.
//! Sprint (ADR 0118): `sprint on` makes every walk after it a sprint, `sprint off` a walk again. A walk looks at
//! its target, so in water a sprint swims to it, down or up as well (a dive, a lake's crossing at the surface).
//! Thirst (ADR 0145, docs/design/needs.md): `drink dx dy dz` holds the use button on the water there for 2.5 s (fresh water
//! fills the bar, the sea's does nothing); `report` gives `vitals` as [health, hunger, thirst, breath] (breath:
//! draft breath, full in air). Death's cost (draft): `report` gives `death` and `home`, the centres of its death
//! pile's cell and its sleeping place (null without); `use dx dy dz` on a bed of leaves beside a torch claims it.
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
    /// A prospecting strike on the block at this offset, waiting for its answer.
    Prospect(DVec3),
    /// Take the crafting grid's output n times; use the block at this offset; close the screen (draft ADR
    /// crafting-grid).
    Craft(u16),
    Use(DVec3),
    Close,
    /// Choose the cast of the furnace at this offset (None: none); report the open furnace.
    Cast(DVec3, Option<String>),
    Furnace,
    /// Walks from now on sprint, or not (ADR 0118).
    Sprint(bool),
    /// Drink from the water at this offset, held for `DRINK_S`.
    Drink(DVec3),
    /// Look at the middle of the block at this offset from the feet, until the next walk (the server's
    /// journal reads the look, draft ADR journal-data).
    Look(DVec3),
    Quit,
}

/// A bot's drink is held this long: a little over the two seconds that fill thirst.
const DRINK_S: f64 = 2.5;

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
        "prospect" => Cmd::Prospect(v()?),
        "craft" if w.len() == 1 => Cmd::Craft(1),
        "craft" => Cmd::Craft(num(1)? as u16),
        "use" => Cmd::Use(v()?),
        "close" => Cmd::Close,
        "cast" => Cmd::Cast(v()?, (w.len() > 4).then(|| w[4..].join(" "))),
        "furnace" => Cmd::Furnace,
        "drink" => Cmd::Drink(v()?),
        "look" => Cmd::Look(v()?),
        "sprint" => match w.get(1).copied() {
            Some("on") => Cmd::Sprint(true),
            Some("off") => Cmd::Sprint(false),
            _ => return Err(format!("sprint on or off in '{line}'")),
        },
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
    /// The block grid's warp (ADR 0131's trial): the server's, given by hand (`warp=`), set before the welcome.
    pub warp: f64,
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
    /// When the server last sent anything: one that keeps the link open but ticks no more (stalled, suspended)
    /// would hold a wait forever, so after `SILENT` of nothing the bot takes it for gone.
    heard: Instant,
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
    /// Its walks sprint (`sprint on`).
    sprint: bool,
    /// A prospecting strike's answer awaited until that tick.
    asking: Option<u64>,
    /// The creature kinds by index and the animals the server has this bot see (ADR 0083).
    pub kinds: Vec<String>,
    pub animals: std::collections::BTreeMap<u32, engine::net::Animal>,
    /// The furnace whose window is open, as the server last sent it.
    pub furnace: Option<(engine::world::Block, engine::furnace::Furnace)>,
    /// Health, hunger and thirst as the server last sent them.
    pub vitals: (u8, u8, u8, u8),
    /// Its death mark and sleeping place as the server last sent them (draft death's cost).
    pub marks: (Option<engine::world::Block>, Option<engine::world::Block>),
    /// A drink held until that tick.
    drinking: Option<u64>,
    /// Its journal's entries not unknown, as the server last told them (draft ADR journal-data).
    pub journal: std::collections::BTreeMap<String, engine::journal::State>,
}

/// The world the last bot of this process was welcomed to, by its welcome (seed, bodies, generation, warp), kept
/// fresh: a bot welcomed to the same starts from its bodies (`World::fresh`) instead of building them again.
/// shortcut: holds one world's bodies for the process's life once bots are gone; drop it when a process runs
/// bots on many worlds in turn
#[allow(clippy::type_complexity)]
static SHARED: std::sync::Mutex<Option<((i32, Vec<(i32, Option<u64>)>, u8, u64), World)>> = std::sync::Mutex::new(None);

/// A server silent this long is taken for gone (it sends `You` every tick; a baked world's first map takes
/// some tens of seconds to make).
const SILENT: Duration = Duration::from_secs(120);

impl Bot {
    pub fn new(name: &str, link: Link<ToServer, ToClient>) -> Bot {
        link.send(ToServer::Hello { name: name.to_string() });
        Bot { name: name.into(), link, world: None, warp: 0.0, pos: DVec3::ZERO, players: 0, edits_applied: 0, refused: 0, slots: Vec::new(), seq: 0, id: 0, flying: false, ticks: 0, heard: Instant::now(), target: None, last_dist: 0.0, notes: Vec::new(), air: None, idle_until: None, start: Instant::now(), creative: true, digging: None, hand: 0, sprint: false, asking: None, kinds: Vec::new(), animals: Default::default(), furnace: None, vitals: (20, 20, 20, 20), marks: (None, None), drinking: None, journal: Default::default() }
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
            let got = self.link.try_recv();
            if !matches!(got, Recv::Empty | Recv::Closed) {
                self.heard = Instant::now();
            }
            // a cell or block id the world lacks would index past its tables and abort the bot
            if let (Recv::Msg(m), Some(w)) = (&got, &self.world) {
                if !engine::net::fits(w, m) {
                    eprintln!("[technoape-bot] dropped a server message naming a cell or block this world lacks");
                    continue;
                }
            }
            match got {
                Recv::Msg(ToClient::Welcome { id, seed, bodies, blocks, items, gen }) => {
                    let gen = engine::world::known_generation(gen).unwrap_or_else(|e| panic!("{e}"));
                    engine::bodies::known_bodies(bodies.len(), gen).unwrap_or_else(|e| panic!("{e}"));
                    assert_eq!(blocks, engine::blocks::BLOCKS.hash, "the server's build has other blocks (data/blocks.toml)");
                    assert_eq!(items, engine::items::ITEMS.hash, "the server's build has other items (data/items.toml)");
                    // the baked map (ADR 0043): shared with a server in this process, from the disk, or fetched
                    // a body loaded on approach (Mars, step 6b) is a stub until the bot is in its frame (`tick`):
                    // asking its map here made the server load it and each of run=sim's 50 bots build it
                    // shortcut: its map is then made from the disk or baked here, never fetched; fetch it as the
                    // client does once a bot on a remote server goes to a body this PC has no map of
                    // bots of one process welcomed to the same world share its bodies (`SHARED`): run=sim's 50 built
                    // them 50 times, 0.7 s each on their thread, with the frames being timed
                    let key = (seed, bodies.clone(), gen, self.warp.to_bits());
                    let shared = SHARED.lock().unwrap().as_ref().filter(|(k, _)| *k == key).map(|(_, w)| w.fresh());
                    let w = match shared {
                        Some(w) => w,
                        None => {
                            let rows = engine::bodies::of(gen);
                            let list = bodies
                                .iter()
                                .enumerate()
                                .map(|(i, &(n, h))| match h {
                                    Some(h) if engine::world::lazy_row(rows, i) => (n, engine::world::Source::Later(h)),
                                    h => (n, engine::world::Source::Now(h.map(|h| engine::net::fetch_map(&mut self.link, seed, n, h).unwrap_or_else(|e| panic!("{e}"))))),
                                })
                                .collect();
                            let w = World::of_sources(rows, list, seed, self.warp, gen);
                            *SHARED.lock().unwrap() = Some((key, w.fresh()));
                            w
                        }
                    };
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
                Recv::Msg(ToClient::Digging { .. } | ToClient::Map { .. } | ToClient::Piles(_) | ToClient::Taken(_)) => {}
                // a bot that dies respawns at once (ADR 0040); it builds in creative, so only a switch to
                // survival can get it there
                Recv::Msg(ToClient::Vitals { health, hunger, thirst, breath, dead }) => {
                    self.vitals = (health, hunger, thirst, breath);
                    if dead.is_some() {
                        self.link.send(ToServer::Respawn);
                    }
                }
                Recv::Msg(ToClient::Marks { death, home }) => self.marks = (death, home),
                Recv::Msg(ToClient::Journal(v)) => {
                    for (name, s) in v {
                        self.journal.insert(name, s);
                    }
                    self.journal.retain(|_, s| *s != engine::journal::State::Unknown);
                }
                Recv::Msg(ToClient::Air(h)) => self.air = Some(h),
                Recv::Msg(ToClient::Kinds(k)) => self.kinds = k,
                Recv::Msg(ToClient::Defs { blocks, items }) => {
                    if let Err(e) = engine::live::apply(&blocks, &items) {
                        eprintln!("[technoape] data: the server's rows refused, the rows before stay: {e}");
                    }
                    if let Some(w) = self.world.as_mut() {
                        w.refresh_defs();
                    }
                }
                Recv::Msg(ToClient::Animals { seen, gone }) => {
                    for id in gone {
                        self.animals.remove(&id);
                    }
                    self.animals.extend(seen.into_iter().map(|a| (a.id, a)));
                }
                Recv::Msg(ToClient::Sky { .. } | ToClient::Fires(_) | ToClient::Events(_)) => {}
                Recv::Msg(ToClient::Furnace { at, f }) => self.furnace = Some((at, f)),
                Recv::Msg(ToClient::Prospect(v)) => {
                    let heard: Vec<String> = v.iter().map(|(e, b)| format!("[{e:?},{:?}]", engine::ore::BANDS.get(*b as usize).unwrap_or(&"?"))).collect();
                    self.notes.push(format!("{{\"bot\":{:?},\"prospect\":[{}]}}", self.name, heard.join(",")));
                    self.asking = None;
                }
                Recv::Msg(ToClient::Chat(t)) => self.notes.push(format!("{{\"bot\":{:?},\"chat\":{t:?}}}", self.name)),
                Recv::Msg(ToClient::Water(v)) => {
                    if let Some(w) = self.world.as_mut() {
                        for (t, cols) in &v {
                            w.water.set_tile(*t, cols);
                        }
                    }
                }
                Recv::Empty => return self.heard.elapsed() < SILENT,
                Recv::Closed => return false,
            }
        }
    }

    /// One tick: receive, steer the walk towards its target. Returns false when the link closed.
    pub fn tick(&mut self) -> bool {
        if !self.receive() {
            return false;
        }
        // in the frame of a body loaded on approach: its ground in first (some tenths of a second, this thread)
        if let Some(w) = self.world.as_mut() {
            let i = w.frame_at(self.pos);
            if let Some(load) = w.loader(i) {
                w.put(i, load().unwrap_or_else(|e| panic!("{e}")));
            }
        }
        let Some(w) = self.world.as_ref() else { return true };
        if let Some((_, shape)) = self.digging.filter(|d| self.ticks >= d.0) {
            self.seq += 1;
            self.link.send(ToServer::Edit { seq: self.seq, shape, material: 0 });
            self.link.send(ToServer::Dig(None));
            self.digging = None;
        }
        if self.drinking.is_some_and(|u| self.ticks >= u) {
            self.link.send(ToServer::Drink(None));
            self.drinking = None;
        }
        if let Some((t, deadline)) = self.target {
            let up = phys::up(w, self.pos);
            let d = (t - self.pos).reject_from(up);
            let dist = d.length();
            // the look at the target (a swim's stroke follows it; a walk ignores it)
            let pitch = (t - self.pos).dot(up).atan2(dist).clamp(-1.5, 1.5) as f32;
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
                self.link.send(ToServer::input(Input { mv: [1.0, 0.0], jump, fwd: (d / dist).as_vec3().to_array(), pitch, sprint: self.sprint, ..Default::default() }));
            }
            self.last_dist = dist;
        }
        true
    }

    /// True while a walk or wait is still going, so the next command has to hold.
    pub fn busy(&self) -> bool {
        self.target.is_some() || self.digging.is_some() || self.drinking.is_some() || self.asking.is_some_and(|u| self.ticks < u) || self.idle_until.is_some_and(|u| self.ticks < u)
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
            Cmd::Prospect(d) => {
                self.link.send(ToServer::Strike(w.block_at(self.feet() + *d)));
                self.asking = Some(self.after(2.0));
            }
            Cmd::Sprint(on) => self.sprint = *on,
            Cmd::Drink(d) => {
                self.link.send(ToServer::Drink(Some(w.block_at(self.feet() + *d))));
                self.drinking = Some(self.after(DRINK_S));
            }
            // the recipe the cells hold as this bot has them (the 3 x 3: the server judges whether a bench is open)
            Cmd::Craft(n) => {
                if let Some(r) = engine::recipes::RECIPES.matched(&self.slots, true) {
                    self.link.send(ToServer::Craft { recipe: r.name.clone(), n: *n });
                }
            }
            Cmd::Use(d) => self.link.send(ToServer::Use(w.block_at(self.feet() + *d))),
            Cmd::Close => self.link.send(ToServer::Close),
            Cmd::Cast(d, cast) => self.link.send(ToServer::Choose { at: w.block_at(self.feet() + *d), cast: cast.clone() }),
            Cmd::Furnace => {
                let Some((_, f)) = &self.furnace else { return Some(format!("{{\"bot\":{:?},\"furnace\":null}}", self.name)) };
                let slots: Vec<String> = f.slots.iter().enumerate().filter_map(|(k, s)| s.map(|st| format!("[{k},{:?},{}]", ITEMS.name(st.item), st.n))).collect();
                return Some(format!("{{\"bot\":{:?},\"furnace\":{{\"slots\":[{}],\"heat\":{:.1},\"burning\":{},\"work\":{:.2},\"choice\":{:?}}}}}", self.name, slots.join(","), f.heat, f.burning(), f.work, f.choice));
            }
            Cmd::Look(d) => {
                let up = phys::up(w, self.pos);
                let to = w.block_centre(w.block_at(self.feet() + *d)) - (self.pos + up * phys::EYE);
                let flat = to.reject_from(up);
                let pitch = to.dot(up).atan2(flat.length()).clamp(-1.55, 1.55) as f32;
                self.link.send(ToServer::Input(Input { fwd: flat.normalize_or(DVec3::X).as_vec3().to_array(), pitch, ..Default::default() }));
            }
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
                    "{{\"bot\":\"{}\",\"t\":{:.2},\"pos\":[{:.2},{:.2},{:.2}],\"players\":{},\"edits_applied\":{},\"refused\":{},\"vitals\":[{},{},{},{}],\"death\":{},\"home\":{},\"air\":{},\"animals\":{},\"journal\":{{{}}}}}",
                    self.name,
                    self.start.elapsed().as_secs_f64(),
                    self.pos.x,
                    self.pos.y,
                    self.pos.z,
                    self.players,
                    self.edits_applied,
                    self.refused,
                    self.vitals.0,
                    self.vitals.1,
                    self.vitals.2,
                    self.vitals.3,
                    self.mark_json(self.marks.0),
                    self.mark_json(self.marks.1),
                    self.air.map_or("null".into(), |a| format!("{{\"temp\":{:.2},\"rain_mm_h\":{:.3},\"cover\":{:.2},\"wind\":[{:.2},{:.2}]}}", a.temp, a.rain, a.cover, a.wind[0], a.wind[1])),
                    self.animals_json(),
                    self.journal.iter().map(|(n, s)| format!("{n:?}:{:?}", s.name())).collect::<Vec<_>>().join(",")
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
                // a furnace's cell reaches its slots, any other a pile's
                let pile = |d: &DVec3| {
                    let b = w.block_at(self.feet() + *d);
                    if engine::recipes::RECIPES.furnace(w.block(b)).is_some() { Holder::Furnace(b) } else { Holder::Pile(b) }
                };
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

    /// A mark's cell centre as [x,y,z], or null.
    fn mark_json(&self, m: Option<engine::world::Block>) -> String {
        m.zip(self.world.as_ref()).map_or("null".into(), |(b, w)| {
            let c = w.block_centre(b);
            format!("[{:.2},{:.2},{:.2}]", c.x, c.y, c.z)
        })
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

    /// A body loaded on approach (Mars, step 6b) is a stub in a bot's world too: its map is not asked at the
    /// welcome (that made the server load Mars and every bot of `run=sim` build it, while the frames were timed)
    #[test]
    fn a_bot_leaves_a_body_loaded_on_approach_for_later() {
        let gen = engine::world::GENERATION;
        let rows = engine::bodies::of(gen);
        let lazy: Vec<usize> = (0..rows.len()).filter(|&i| engine::world::lazy_row(rows, i)).collect();
        assert!(!lazy.is_empty(), "the newest generation has a body loaded on approach");
        let bodies = engine::bodies::sizes(rows, 256, 32).into_iter().enumerate().map(|(i, n)| (n, lazy.contains(&i).then_some(0x5eed_u64))).collect();
        let (c, s) = local::<ToServer, ToClient>();
        let mut bot = Bot::new("lazy", c);
        s.send(ToClient::Welcome { id: 1, seed: 3, bodies, blocks: engine::blocks::BLOCKS.hash, items: engine::items::ITEMS.hash, gen });
        // (a server gone: a map asked of it would never come)
        drop(s);
        bot.receive();
        let w = bot.world.as_ref().expect("welcomed");
        assert!(lazy.iter().all(|&i| !w.loaded(i)), "a body loaded on approach waits");
    }

    /// Bots in one process welcomed to the same world build its bodies once (run=sim's 50 built them 50 times,
    /// 0.7 s each, while the frames were timed); each still has its own edits
    #[test]
    fn bots_of_one_world_share_its_bodies_not_their_edits() {
        let gen = engine::world::GENERATION;
        let bodies: Vec<(i32, Option<u64>)> = engine::bodies::sizes(engine::bodies::of(gen), 256, 32).into_iter().map(|n| (n, None)).collect();
        let welcomed = |name: &str| {
            let (c, s) = local::<ToServer, ToClient>();
            let mut bot = Bot::new(name, c);
            s.send(ToClient::Welcome { id: 1, seed: 11, bodies: bodies.clone(), blocks: engine::blocks::BLOCKS.hash, items: engine::items::ITEMS.hash, gen });
            drop(s);
            bot.receive();
            bot
        };
        let (mut a, b) = (welcomed("a"), welcomed("b"));
        let (wa, wb) = (a.world.as_mut().unwrap(), b.world.as_ref().unwrap());
        assert!(std::sync::Arc::ptr_eq(&wa.bodies, &wb.bodies), "the bodies are built once");
        let at = wa.block_at(wa.bodies[0].spawn_point());
        wa.set_block(at, engine::blocks::BLOCKS.id("stone"));
        assert_eq!((wa.edit_count, b.world.as_ref().unwrap().edit_count), (1, 0), "an edit is its own bot's");
    }

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
        assert_eq!((parse("sprint on"), parse("sprint off")), (Ok(Some(Cmd::Sprint(true))), Ok(Some(Cmd::Sprint(false)))));
        assert!(parse("sprint").is_err());
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

    /// Thirst through the protocol (ADR 0145): a survival bot made thirsty drinks from a pond poured
    /// into a pit beside the spawn and its report shows the bar full again.
    #[test]
    fn a_thirsty_bot_drinks_from_a_pond_and_reports_its_thirst() {
        let h = World::new(256, 32, 1).bodies[0].height(DVec3::Y);
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let mut s = Server::new(256, 32, 1, rx);
            // a pit 3 x 3 and two deep, its middle 2.5 m along +x from the pole, filled by hand
            let w = &mut s.world;
            let mid = w.air_over_ground(0, (DVec3::Y * h + DVec3::X * 2.5).normalize());
            for (di, dj, dz) in (-1..=1).flat_map(|i| (-1..=1).flat_map(move |j| [(i, j, 1), (i, j, 2)])) {
                let c = engine::world::Block { pos: mid.pos + glam::IVec3::new(di, dj, -dz), ..mid };
                w.set_block(c, 0);
                w.regap(c);
            }
            for (di, dj) in (-1..=1).flat_map(|i| (-1..=1).map(move |j| (i, j))) {
                let c = engine::world::Block { pos: mid.pos + glam::IVec3::new(di, dj, -2), ..mid };
                let World { bodies, edits, water, sky, time, .. } = &mut s.world;
                water.pour(&engine::world::Solid::at(bodies, edits, sky, *time), World::col(c), c.pos.z, 2 * engine::liquid::U);
            }
            s.run(TICK_HZ, &std::sync::atomic::AtomicBool::new(false))
        });
        let (c, sv) = local();
        tx.send(sv).unwrap();
        let mut bot = Bot::new("drinker", c);
        let mut out = Vec::new();
        let script = format!("spawn 0 {h} 0; say /playmode survival; walk 0 0 0; wait 1; say /thirst 2; wait 0.3; report; drink 2.5 -1.5 0; wait 0.2; report");
        bot.run(parse_script(&script).unwrap(), None, |l| out.push(l));
        let reports: Vec<&String> = out.iter().filter(|l| l.contains("\"vitals\"")).collect();
        assert_eq!(reports.len(), 2, "{out:?}");
        assert!(reports[0].contains("\"vitals\":[20,20,2,20]"), "thirsty first: {}", reports[0]);
        assert!(reports[1].contains("\"vitals\":[20,20,20,20]"), "full after the drink: {}", reports[1]);
    }

    /// Breath through the protocol (draft breath): a survival bot on the sea bed loses its breath in 30 s and
    /// then health; walked out onto the land its breath is back in 2 s.
    #[test]
    fn a_bot_under_water_loses_its_breath_then_health_and_air_brings_it_back() {
        let w = World::new(256, 32, 1);
        let b = &w.bodies[0];
        let sea = b.layer_r(b.sea.unwrap());
        let dirs = || (0..20000).map(|k| { let z = 1.0 - (k as f64 + 0.5) / 10000.0; let a = k as f64 * 2.399963; let r = (1.0 - z * z).sqrt(); DVec3::new(r * a.cos(), r * a.sin(), z) });
        let d = dirs().find(|d| b.height(*d) < sea - 4.0).expect("a sea");
        let land = dirs().find(|d| b.height(*d) > sea + 6.0).expect("land");
        let (deep, dry) = (b.centre + d * (b.height(d) + 0.1), b.centre + land * (b.height(land) + 1.0));
        let (tx, rx) = channel();
        std::thread::spawn(move || Server::new(256, 32, 1, rx).run(TICK_HZ, &std::sync::atomic::AtomicBool::new(false)));
        let (c, sv) = local();
        tx.send(sv).unwrap();
        let mut bot = Bot::new("diver", c);
        let mut out = Vec::new();
        let script = format!(
            "say /playmode survival; spawn {} {} {}; walk 0 0 0; wait 33; report; spawn {} {} {}; walk 0 0 0; wait 2.5; report",
            deep.x, deep.y, deep.z, dry.x, dry.y, dry.z
        );
        bot.run(parse_script(&script).unwrap(), None, |l| out.push(l));
        let vitals = |l: &str| -> Vec<u8> { l.split("\"vitals\":[").nth(1).unwrap().split(']').next().unwrap().split(',').map(|x| x.parse().unwrap()).collect() };
        let r: Vec<Vec<u8>> = out.iter().filter(|l| l.contains("\"vitals\"")).map(|l| vitals(l)).collect();
        assert_eq!(r.len(), 2, "{out:?}");
        assert!(r[0][3] == 0 && r[0][0] < 20, "under: no breath, health going: {:?}", r[0]);
        // (a point may still go in the tick between the report and the teleport)
        assert!(r[1][3] == 20 && r[1][0] + 1 >= r[0][0], "out: the breath back, no more health gone: {:?} {:?}", r[0], r[1]);
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

    /// The journal by script (draft ADR journal-data): a survival bot given a flint reports flint known and the
    /// hand axe it points at noticed; one that looks at clay a metre beside it reports clay noticed, and not
    /// before it looked.
    #[test]
    fn a_survival_bot_learns_flint_by_having_it_and_notices_clay_by_looking() {
        let (tx, rx) = channel();
        std::thread::spawn(move || Server::new(256, 32, 1, rx).run(TICK_HZ, &std::sync::atomic::AtomicBool::new(false)));
        let (c, s) = local();
        tx.send(s).unwrap();
        let mut bot = Bot::new("learner", c);
        let w = World::new(256, 32, 1);
        let h = w.bodies[0].height(DVec3::Y);
        let (d, at) = ("1.0 -0.5 0.25", w.block_at(DVec3::new(1.25, h - 0.5, 0.5)));
        let mut out = Vec::new();
        let script = format!("wait 0.5; say /playmode survival; spawn 0.25 {h} 0.25; wait 0.5; report; say /give flint 1; say /place clay {} {} {} {}; wait 1; report; look {d}; wait 1; report", at.face, at.pos.x, at.pos.y, at.pos.z);
        bot.run(parse_script(&script).unwrap(), None, |l| out.push(l));
        let reports: Vec<&String> = out.iter().filter(|l| l.contains("\"journal\"")).collect();
        assert!(reports[0].contains("\"journal\":{}"), "{out:?}");
        assert!(reports[1].contains("\"journal\":{\"flint\":\"known\",\"hand-axe\":\"noticed\"}"), "{out:?}");
        assert!(reports[2].contains("\"journal\":{\"clay\":\"noticed\",\"flint\":\"known\",\"hand-axe\":\"noticed\"}"), "{out:?}");
    }

    /// Crafting by script, end to end (ADR 0140): a survival bot moves four logs into the 2 x 2 and
    /// takes a workbench, places it, uses it, lays a copper pick head over a stick in the far column (the bench's
    /// alone), takes a pick and digs copper with it (grade 2): one use less.
    #[test]
    fn a_survival_bot_makes_a_workbench_then_a_copper_pick_at_it_and_digs_copper() {
        let (tx, rx) = channel();
        std::thread::spawn(move || Server::new(256, 32, 1, rx).run(TICK_HZ, &std::sync::atomic::AtomicBool::new(false)));
        let (c, s) = local();
        tx.send(s).unwrap();
        let mut bot = Bot::new("smith", c);
        let w = World::new(256, 32, 1);
        let h = w.bodies[0].height(DVec3::Y);
        let (d, at) = ("1.0 -0.5 0.25", w.block_at(DVec3::new(1.25, h - 0.5, 0.5)));
        let (bench, wb) = ("-1.5 0.5 0.25", engine::blocks::BLOCKS.id("workbench"));
        let g = engine::items::CRAFT_AT;
        let script = format!(
            "wait 0.5; say /playmode survival; spawn 0.25 {h} 0.25; wait 0.5; say /place copper {} {} {} {}; say /give oak_log 4; wait 0.5; \
             move 0 {} 1; move 0 {} 1; move 0 {} 1; move 0 {} 1; wait 0.5; craft; wait 0.5; slots; \
             place {bench} {wb}; wait 0.5; block {bench}; use {bench}; say /give copper_pick_head 1; say /give stick 1; wait 0.5; \
             move 0 {} 1; move 1 {} 1; wait 0.5; craft; wait 0.5; slots; hold 0; dig {d}; wait 0.5; slots; block {d}",
            at.face, at.pos.x, at.pos.y, at.pos.z, g, g + 1, g + 3, g + 4, g + 2, g + 5
        );
        let mut out = Vec::new();
        bot.run(parse_script(&script).unwrap(), None, |l| out.push(l));
        let slots: Vec<&String> = out.iter().filter(|l| l.contains("\"slots\"")).collect();
        let blocks: Vec<&String> = out.iter().filter(|l| l.contains("\"material\"")).collect();
        let uses = ITEMS.get(ITEMS.named("copper pick").unwrap()).unwrap().tool.as_ref().unwrap().uses;
        assert!(slots[0].contains("[0,\"workbench\",1]") && !slots[0].contains("log"), "{out:?}");
        assert!(blocks[0].contains(&format!("\"material\":{wb}")), "placed: {out:?}");
        assert!(slots[1].contains(&format!("[0,\"copper pick\",1,{uses}]")) && !slots[1].contains("head") && !slots[1].contains("stick") && !slots[1].contains("workbench"), "{out:?}");
        assert!(slots[2].contains(&format!("[0,\"copper pick\",1,{}]", uses - 1)), "{out:?} ({} refused)", bot.refused);
        assert!(blocks[1].contains("\"material\":0"), "{out:?}");
        assert!(parse("craft 3").unwrap() == Some(Cmd::Craft(3)) && parse("craft").unwrap() == Some(Cmd::Craft(1)) && parse("close").unwrap() == Some(Cmd::Close));
        assert!(parse("craft copper pick").is_err(), "crafting by name is gone");
    }

    /// The copper age end to end (ADR 0141): a survival bot given logs, clay, copper gravel and a stick
    /// makes a workbench, at it a crucible and by hand a clay mould; fed gravel and logs the crucible melts two
    /// ingots, which cast with the mould into a pick head; the head on the stick is a copper pick, which digs.
    #[test]
    fn a_survival_bot_melts_gravel_casts_a_head_and_digs_with_the_pick() {
        let (tx, rx) = channel();
        std::thread::spawn(move || Server::new(256, 32, 1, rx).run(TICK_HZ, &std::sync::atomic::AtomicBool::new(false)));
        let (c, s) = local();
        tx.send(s).unwrap();
        let mut bot = Bot::new("founder", c);
        let w = World::new(256, 32, 1);
        let h = w.bodies[0].height(DVec3::Y);
        let (d, at) = ("1.0 -0.5 0.25", w.block_at(DVec3::new(1.25, h - 0.5, 0.5)));
        let (bench, wb) = ("-1.5 0.5 0.25", engine::blocks::BLOCKS.id("workbench"));
        let (cru, cb) = ("0.25 0.5 -1.5", engine::blocks::BLOCKS.id("crucible"));
        let g = engine::items::CRAFT_AT;
        let ring: String = [0, 1, 2, 3, 5, 6, 7, 8].iter().map(|k| format!("move 1 {} 1; ", g + k)).collect();
        let script = format!(
            "wait 0.5; say /playmode survival; spawn 0.25 {h} 0.25; wait 0.5; say /place copper {} {} {} {}; \
             say /give oak_log 6; say /give clay 10; say /give copper_gravel 4; say /give stick 1; wait 0.5; \
             move 0 {g} 1; move 0 {} 1; move 0 {} 1; move 0 {} 1; wait 0.5; craft; wait 0.5; place {bench} {wb}; wait 0.5; use {bench}; wait 0.3; \
             {ring}wait 0.5; craft; wait 0.5; place {cru} {cb}; wait 0.5; block {cru}; \
             move 1 {g} 1; move 1 {} 1; wait 0.5; craft; wait 0.5; slots; \
             use {cru}; wait 0.3; put 2 {cru} 0 4; put 0 {cru} 1 2; put 1 {cru} 2 1; cast {cru} cast pick head; wait 28; furnace; \
             take {cru} 3 5 2; wait 0.3; put 5 {cru} 0 2; wait 6; furnace; take {cru} 3 6 1; wait 0.3; \
             move 6 {g} 1; move 3 {} 1; wait 0.5; craft; wait 0.5; slots; hold 0; dig {d}; wait 0.5; slots; block {d}",
            at.face, at.pos.x, at.pos.y, at.pos.z, g + 1, g + 3, g + 4, g + 1, g + 3
        );
        let mut out = Vec::new();
        bot.run(parse_script(&script).unwrap(), None, |l| out.push(l));
        let slots: Vec<&String> = out.iter().filter(|l| l.contains("\"slots\"") && !l.contains("furnace")).collect();
        let furnace: Vec<&String> = out.iter().filter(|l| l.contains("\"furnace\"")).collect();
        let blocks: Vec<&String> = out.iter().filter(|l| l.contains("\"material\"")).collect();
        let uses = ITEMS.get(ITEMS.named("copper pick").unwrap()).unwrap().tool.as_ref().unwrap().uses;
        assert!(blocks[0].contains(&format!("\"material\":{cb}")), "the crucible placed: {out:?}");
        assert!(slots[0].contains("\"clay mould\",1") && !slots[0].contains("\"clay\""), "{out:?}");
        assert!(furnace[0].contains("[3,\"copper ingot\",2]") && !furnace[0].contains("gravel") && furnace[0].contains("\"burning\":true"), "{out:?}");
        assert!(furnace[1].contains("[3,\"copper pick head\",1]") && furnace[1].contains("[2,\"clay mould\",1]"), "{out:?}");
        assert!(slots[1].contains(&format!("\"copper pick\",1,{uses}]")) && !slots[1].contains("head"), "{out:?}");
        assert!(slots[2].contains(&format!("\"copper pick\",1,{}]", uses - 1)), "{out:?} ({} refused)", bot.refused);
        assert!(blocks[1].contains("\"material\":0"), "{out:?}");
        assert!(parse("cast 1 0 0 cast pick head").unwrap() == Some(Cmd::Cast(DVec3::new(1.0, 0.0, 0.0), Some("cast pick head".into()))));
        assert!(parse("cast 1 0 0").unwrap() == Some(Cmd::Cast(DVec3::X, None)));
    }

    /// Draft ADR tin-and-bronze: a bot puts copper ingots into a crucible's second input (slot 4) and tin into
    /// the first, and the furnace it reports has melted them into bronze.
    #[test]
    fn a_survival_bot_alloys_bronze_through_the_second_input() {
        let (tx, rx) = channel();
        std::thread::spawn(move || Server::new(256, 32, 1, rx).run(TICK_HZ, &std::sync::atomic::AtomicBool::new(false)));
        let (c, s) = local();
        tx.send(s).unwrap();
        let mut bot = Bot::new("alloyer", c);
        let w = World::new(256, 32, 1);
        let h = w.bodies[0].height(DVec3::Y);
        let (d, at) = ("1.0 -0.5 0.25", w.block_at(DVec3::new(1.25, h - 0.5, 0.5)));
        let script = format!(
            "wait 0.5; say /playmode survival; spawn 0.25 {h} 0.25; wait 0.5; say /place crucible {} {} {} {}; \
             say /give copper_ingot 3; say /give tin_ingot 1; say /give oak_log 1; wait 0.5; \
             use {d}; wait 0.3; put 0 {d} 4 3; put 1 {d} 0 1; put 2 {d} 1 1; wait 26; furnace",
            at.face, at.pos.x, at.pos.y, at.pos.z
        );
        let mut out = Vec::new();
        bot.run(parse_script(&script).unwrap(), None, |l| out.push(l));
        let f = out.iter().find(|l| l.contains("\"furnace\"")).expect("a furnace line");
        assert!(f.contains("[3,\"bronze ingot\",4]") && !f.contains("tin") && !f.contains("copper"), "{out:?}");
    }

    /// Prospecting by script (ADR 0138): a survival bot given a hand axe strikes granite
    /// beside its feet and prints the answer as one JSON line (no ore on the noise land: the rock rings empty); a
    /// strike sooner than a second after is not answered, and each answered one costs the axe a use.
    #[test]
    fn a_survival_bot_prospects_and_prints_the_answer() {
        let (tx, rx) = channel();
        std::thread::spawn(move || Server::new(256, 32, 1, rx).run(TICK_HZ, &std::sync::atomic::AtomicBool::new(false)));
        let (c, s) = local();
        tx.send(s).unwrap();
        let mut bot = Bot::new("prospector", c);
        let w = World::new(256, 32, 1);
        let h = w.bodies[0].height(DVec3::Y);
        let (d, at) = ("1.0 -0.5 0.25", w.block_at(DVec3::new(1.25, h - 0.5, 0.5)));
        let mut out = Vec::new();
        let script = format!("wait 0.5; say /playmode survival; spawn 0.25 {h} 0.25; wait 0.5; say /place granite {} {} {} {}; say /give hand_axe 1; hold 0; wait 0.5; prospect {d}; strike {d}; wait 1.1; prospect {d}; slots", at.face, at.pos.x, at.pos.y, at.pos.z);
        bot.run(parse_script(&script).unwrap(), None, |l| out.push(l));
        let heard: Vec<&String> = out.iter().filter(|l| l.contains("\"prospect\"")).collect();
        assert_eq!(heard.len(), 2, "{out:?}");
        assert!(heard.iter().all(|l| l.contains("\"prospect\":[]")), "{out:?}");
        let uses = ITEMS.get(ITEMS.named("hand axe").unwrap()).unwrap().tool.as_ref().unwrap().uses;
        assert!(out.last().unwrap().contains(&format!("[0,\"hand axe\",1,{}]", uses - 2)), "{out:?}");
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

    /// Sprint by script (ADR 0118): over the same 60 server ticks from the same spawn, a survival bot told to
    /// sprint covers half as much ground again as one that walks (ticks, not seconds: the review of ADR 0118).
    #[test]
    fn a_sprinting_bot_covers_half_as_much_again_in_the_same_ticks() {
        let covered = |sprint: bool| {
            let (tx, rx) = channel();
            let mut s = Server::new(256, 32, 1, rx);
            let (c, sv) = local();
            tx.send(sv).unwrap();
            let mut b = Bot::new("runner", c);
            let tick = |b: &mut Bot, s: &mut Server, n: usize| {
                for _ in 0..n {
                    b.tick();
                    s.tick();
                }
            };
            tick(&mut b, &mut s, 20);
            b.exec(&Cmd::Say("/playmode survival".into()));
            b.exec(&Cmd::Sprint(sprint));
            tick(&mut b, &mut s, 40);
            let from = b.pos;
            b.exec(&Cmd::Walk(DVec3::X * 30.0));
            tick(&mut b, &mut s, 60);
            (b.pos - from).reject_from(phys::up(b.world.as_ref().unwrap(), b.pos)).length()
        };
        let (walk, sprint) = (covered(false), covered(true));
        eprintln!("60 ticks: walked {walk:.2} m, sprinted {sprint:.2} m");
        assert!((sprint / walk - 1.5).abs() < 0.1, "walked {walk:.2} m, sprinted {sprint:.2} m");
    }

    /// Swimming by script (ADR 0118): a sprinting bot set down in the sea swims 15 m across it at the stroke's
    /// speed and arrives.
    #[test]
    fn a_sprinting_bot_swims_across_the_sea() {
        let w = World::new(256, 32, 1);
        let b = &w.bodies[0];
        let sea = b.radius - engine::world::SEA_DEPTH;
        // a way 15 m long over sea at least 3 m deep
        let (d, way) = (0..4000)
            .map(|k| {
                let z = 1.0 - 2.0 * (k as f64 + 0.5) / 4000.0;
                let t = k as f64 * 2.4;
                DVec3::new((1.0 - z * z).sqrt() * t.cos(), z, (1.0 - z * z).sqrt() * t.sin())
            })
            .find_map(|d| {
                let a = phys::tangent(DVec3::X, d, DVec3::Z);
                (0..=18).all(|k| b.height((d * sea + a * k as f64).normalize()) < sea - 3.0).then_some((d, a))
            })
            .expect("open sea");
        let (at, go) = (b.centre + d * (sea - 1.0), way * 15.0);
        // the server stepped by hand, a tick for each of the bot's (ticks, not seconds: the review of ADR 0118)
        let (tx, rx) = channel();
        let mut s = Server::new(256, 32, 1, rx);
        let (c, sv) = local();
        tx.send(sv).unwrap();
        let mut bot = Bot::new("swimmer", c);
        let tick = |b: &mut Bot, s: &mut Server| {
            b.tick();
            s.tick();
        };
        for _ in 0..20 {
            tick(&mut bot, &mut s);
        }
        bot.exec(&Cmd::Spawn(at));
        bot.exec(&Cmd::Sprint(true));
        for _ in 0..20 {
            tick(&mut bot, &mut s);
        }
        let from = bot.pos;
        bot.exec(&Cmd::Walk(go));
        let mut n = 0;
        while bot.target.is_some() && n < 1000 {
            tick(&mut bot, &mut s);
            n += 1;
        }
        let secs = n as f64 / TICK_HZ;
        let up = phys::up(bot.world.as_ref().unwrap(), bot.pos);
        let crossed = (bot.pos - from).reject_from(up).length();
        eprintln!("swam {crossed:.2} m in {n} ticks ({secs:.2} s)");
        assert!(bot.notes.iter().all(|l| !l.contains("gave up")) && crossed > 14.0, "{crossed} m in {n} ticks: {:?}", bot.notes);
        assert!(secs < 15.0 / phys::SWIM_SLOW, "faster than the slow swim: {secs} s");
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

    /// Death's cost through the protocol (draft): a bot claims a bed of leaves beside a torch, dies with what it
    /// carries, and its report shows the death mark where it died and its pos on the bed it woke at.
    #[test]
    fn a_bot_that_dies_reports_its_death_mark_and_wakes_on_its_bed() {
        let h = World::new(256, 32, 1).bodies[0].height(DVec3::Y);
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let mut s = Server::new(256, 32, 1, rx);
            let w = &mut s.world;
            let bed = w.air_over_ground(0, (DVec3::Y * h + DVec3::X * 4.5).normalize());
            for z in 1..4 {
                w.set_block(engine::world::Block { pos: bed.pos + glam::IVec3::Z * z, ..bed }, 0);
            }
            w.set_block(bed, engine::blocks::BLOCKS.id("oak leaves"));
            w.set_block(engine::world::Block { pos: bed.pos + glam::IVec3::new(1, 0, 0), ..bed }, engine::blocks::BLOCKS.id("torch"));
            s.run(TICK_HZ, &std::sync::atomic::AtomicBool::new(false))
        });
        let (c, sv) = local();
        tx.send(sv).unwrap();
        let mut bot = Bot::new("sleeper", c);
        let mut out = Vec::new();
        let script = format!("spawn 0 {h} 0; say /playmode survival; walk 0 0 0; wait 0.5; use 4.5 0.5 0; say /give oak_log 5; wait 0.5; say /hurt 40; wait 1; report");
        bot.run(parse_script(&script).unwrap(), None, |l| out.push(l));
        let r = out.iter().find(|l| l.contains("\"death\"")).unwrap_or_else(|| panic!("{out:?}"));
        let num = |key: &str| -> Vec<f64> { r.split(&format!("\"{key}\":[")).nth(1).unwrap_or_else(|| panic!("{key} in {r}")).split(']').next().unwrap().split(',').map(|x| x.parse().unwrap()).collect() };
        let (pos, death, home) = (num("pos"), num("death"), num("home"));
        let at = |v: &[f64]| DVec3::new(v[0], v[1], v[2]);
        assert!(at(&death).distance(DVec3::new(0.0, h, 0.0)) < 2.0, "the death mark where it died: {r}");
        assert!(at(&pos).distance(at(&home)) < 2.0, "woke on its bed: {r}");
        assert!(at(&home).distance(DVec3::new(4.5, h, 0.0)) < 2.0, "the bed claimed: {r}");
    }
}
