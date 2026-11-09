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
    Place(DVec3, u8),
    Block(DVec3),
    Report,
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
        "place" => Cmd::Place(v()?, num(4)? as u8),
        "block" => Cmd::Block(v()?),
        "report" => Cmd::Report,
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
    id: u32,
    flying: bool,
    /// The walk: where to, and when to give up.
    target: Option<(DVec3, Instant)>,
    last_dist: f64,
    notes: Vec<String>,
    idle_until: Option<Instant>,
    start: Instant,
}

impl Bot {
    pub fn new(name: &str, link: Link<ToServer, ToClient>) -> Bot {
        link.send(ToServer::Hello { name: name.to_string() });
        Bot { name: name.into(), link, world: None, pos: DVec3::ZERO, players: 0, edits_applied: 0, id: 0, flying: false, target: None, last_dist: 0.0, notes: Vec::new(), idle_until: None, start: Instant::now() }
    }

    fn feet(&self) -> DVec3 {
        self.pos
    }

    /// Takes everything the server sent; false once the server is gone.
    pub fn receive(&mut self) -> bool {
        loop {
            match self.link.try_recv() {
                Recv::Msg(ToClient::Welcome { id, seed, planet_n, moon_n }) => {
                    let w = World::new(planet_n, moon_n, seed);
                    self.id = id;
                    // the server spawns players just over the +y pole; an empty input makes this one a
                    // walker, so it lands there
                    self.pos = DVec3::Y * (w.bodies[0].height(DVec3::Y) + 0.6);
                    self.world = Some(w);
                    self.link.send(ToServer::Input(Input::default()));
                }
                Recv::Msg(ToClient::Edits(v)) => {
                    if let Some(w) = self.world.as_mut() {
                        for (b, m) in v {
                            w.set_block(b, m);
                            self.edits_applied += 1;
                        }
                    }
                }
                Recv::Msg(ToClient::Players(p)) => {
                    self.players = p.len();
                    if let Some(me) = p.iter().find(|p| p.id == self.id && !self.flying) {
                        self.pos = DVec3::from_array(me.pos);
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
        if let Some((t, deadline)) = self.target {
            let d = (t - self.pos).reject_from(phys::up(w, self.pos));
            let dist = d.length();
            let late = Instant::now() > deadline;
            if dist < ARRIVE || late {
                if late {
                    self.notes.push(format!("{{\"bot\":\"{}\",\"walk\":\"gave up\",\"left\":{:.2}}}", self.name, dist));
                }
                self.target = None;
                self.link.send(ToServer::Input(Input::default()));
            } else {
                // jump when the last tick brought it no closer: something higher than the step-up is in the way
                let jump = dist > self.last_dist - 0.02;
                self.link.send(ToServer::Input(Input { mv: [1.0, 0.0], jump, fwd: (d / dist).as_vec3().to_array(), pitch: 0.0 }));
            }
            self.last_dist = dist;
        }
        true
    }

    /// True while a walk or wait is still going, so the next command has to hold.
    pub fn busy(&self) -> bool {
        self.target.is_some() || self.idle_until.is_some_and(|u| Instant::now() < u)
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
                let give_up = Instant::now() + Duration::from_secs_f64(d.length() / phys::WALK * 3.0 + 2.0);
                (self.flying, self.last_dist, self.target) = (false, f64::MAX, Some((self.pos + *d, give_up)));
            }
            Cmd::Wait(s) => self.idle_until = Some(Instant::now() + Duration::from_secs_f64(*s)),
            Cmd::Dig(d) => self.link.send(ToServer::Edit { block: w.block_at(self.feet() + *d), material: 0 }),
            Cmd::Place(d, m) => self.link.send(ToServer::Edit { block: w.block_at(self.feet() + *d), material: *m }),
            Cmd::Block(d) => {
                let b = w.block_at(self.feet() + *d);
                return Some(format!(
                    "{{\"bot\":\"{}\",\"block\":[{},{},{}],\"face\":{},\"pos\":[{},{},{}],\"material\":{}}}",
                    self.name, d.x, d.y, d.z, b.face, b.pos.x, b.pos.y, b.pos.z, w.block(b)
                ));
            }
            Cmd::Report => {
                return Some(format!(
                    "{{\"bot\":\"{}\",\"t\":{:.2},\"pos\":[{:.2},{:.2},{:.2}],\"players\":{},\"edits_applied\":{}}}",
                    self.name, self.start.elapsed().as_secs_f64(), self.pos.x, self.pos.y, self.pos.z, self.players, self.edits_applied
                ));
            }
            Cmd::Quit => {}
        }
        None
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

    #[test]
    fn parse_script_and_errors() {
        let s = parse_script("spawn 1 2 3; wait 0.5\n# a comment\ndig 0 -1 0; place 1 0 0 3; block 1 0 0; report; quit").unwrap();
        assert_eq!(s.len(), 7);
        assert_eq!(s[3], Cmd::Place(DVec3::new(1.0, 0.0, 0.0), 3));
        assert!(parse("fly 1 2 3").is_err());
        assert!(parse("walk 1 x 3").is_err());
    }

    #[test]
    fn one_bot_digs_and_the_other_sees_the_hole() {
        let (tx, rx) = channel();
        std::thread::spawn(move || Server::new(256, 32, 1, rx).run(200.0));
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
            watcher.run(parse_script(&format!("spawn 2 {h} 0; wait 0.5; block -1.75 -0.5 0.25; block -1.75 -1.0 0.25; report")).unwrap(), None, |l| out.push(l));
            out
        });
        let mut dout = Vec::new();
        digger.run(parse_script(&format!("spawn 0 {h} 0; wait 0.2; dig 0.25 -0.5 0.25; dig 0.25 -1.0 0.25; wait 0.2; block 0.25 -0.5 0.25; report; wait 1")).unwrap(), None, |l| dout.push(l));
        let wout = w.join().unwrap();
        assert!(dout[0].contains("\"material\":0"), "{dout:?}");
        assert!(wout[0].contains("\"material\":0") && wout[1].contains("\"material\":0"), "{wout:?}");
        assert!(wout[2].contains("\"players\":2") && wout[2].contains("\"edits_applied\":2"), "{wout:?}");
    }

    #[test]
    fn a_bot_lands_and_walks_ten_metres_over_the_ground() {
        let (tx, rx) = channel();
        std::thread::spawn(move || Server::new(256, 32, 1, rx).run(200.0));
        let (c, s) = local();
        tx.send(s).unwrap();
        let mut b = Bot::new("walker", c);
        let mut out = Vec::new();
        b.run(parse_script("wait 1").unwrap(), None, |l| out.push(l));
        let start = b.pos;
        b.run(parse_script("walk 10 0 0; wait 0.5").unwrap(), None, |l| out.push(l));
        let moved = b.pos - start;
        assert!(out.is_empty(), "{out:?}");
        assert!((moved.x - 10.0).abs() < 1.0 && moved.z.abs() < 1.0, "moved {moved:?}");
        let w = b.world.as_ref().unwrap();
        let up = phys::up(w, b.pos);
        let fwd = phys::tangent(DVec3::X, up, DVec3::Z);
        let right = fwd.cross(up);
        assert!(!phys::blocked(w, b.pos, right, fwd, up) && phys::blocked(w, b.pos - up * 0.05, right, fwd, up), "feet on the ground at {:?}", b.pos);
    }
}
