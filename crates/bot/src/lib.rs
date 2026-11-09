//! A bot: a player on the protocol with no window (ADR 0002, 0006). A script of commands drives it, from
//! `script=` or live from stdin; it answers with one JSON line per `report` or `block`.
//!
//! Commands (numbers are metres in world axes; `dx dy dz` is relative to the feet):
//!   spawn x y z         teleport               walk dx dy dz       move there at `speed`
//!   wait s              idle s seconds         dig dx dy dz        set that block to air
//!   place dx dy dz m    set that block to m    block dx dy dz      report that block's material
//!   report              report position, players and edits seen    quit
use engine::net::{Link, Recv, ToClient, ToServer, TICK_HZ};
use engine::world::World;
use glam::DVec3;
use std::collections::VecDeque;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

pub const EYE: f64 = 1.7;

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
    pub pos: DVec3,
    pub players: usize,
    pub edits_applied: u64,
    pub speed: f64,
    target: Option<DVec3>,
    idle_until: Option<Instant>,
    start: Instant,
}

impl Bot {
    pub fn new(name: &str, link: Link<ToServer, ToClient>) -> Bot {
        link.send(ToServer::Hello { name: name.to_string() });
        Bot { name: name.into(), link, world: None, pos: DVec3::ZERO, players: 0, edits_applied: 0, speed: 4.0, target: None, idle_until: None, start: Instant::now() }
    }

    fn up(&self) -> DVec3 {
        let w = self.world.as_ref().expect("welcomed");
        (self.pos - w.bodies[w.nearest(self.pos)].centre).normalize_or(DVec3::Y)
    }

    fn feet(&self) -> DVec3 {
        self.pos - self.up() * EYE
    }

    /// Takes everything the server sent; false once the server is gone.
    pub fn receive(&mut self) -> bool {
        loop {
            match self.link.try_recv() {
                Recv::Msg(ToClient::Welcome { seed, planet_n, moon_n, .. }) => {
                    let w = World::new(planet_n, moon_n, seed);
                    // default spawn: eye height over the +y pole of the planet, like the client's benchmark
                    self.pos = DVec3::new(0.0, w.bodies[0].height(DVec3::Y) + EYE, 0.0);
                    self.world = Some(w);
                }
                Recv::Msg(ToClient::Edits(v)) => {
                    if let Some(w) = self.world.as_mut() {
                        for (b, m) in v {
                            w.set_block(b, m);
                            self.edits_applied += 1;
                        }
                    }
                }
                Recv::Msg(ToClient::Players(p)) => self.players = p.len(),
                Recv::Empty => return true,
                Recv::Closed => return false,
            }
        }
    }

    /// One tick: receive, move towards the target, report the position. Returns false when the link closed.
    pub fn tick(&mut self, dt: f64) -> bool {
        if !self.receive() {
            return false;
        }
        if self.world.is_none() {
            return true;
        }
        if let Some(t) = self.target {
            let d = t - self.pos;
            let step = self.speed * dt;
            if d.length() <= step {
                self.pos = t;
                self.target = None;
            } else {
                self.pos += d.normalize() * step;
            }
        }
        self.link.send(ToServer::Move { pos: self.pos.to_array(), yaw: 0.0, pitch: 0.0 });
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
            Cmd::Spawn(p) => self.pos = *p,
            Cmd::Walk(d) => self.target = Some(self.pos + *d),
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
        let mut last = Instant::now();
        loop {
            let now = Instant::now();
            if !self.tick((now - last).as_secs_f64()) {
                return;
            }
            last = now;
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
        // both spawn over the +y pole; the watcher walks 2 m aside, the digger makes a 2-deep hole next to
        // its feet, and the watcher reads those blocks from the deltas it received (offsets aim at the
        // middle of a column: the pole itself is a column corner, and the watcher's up tilts 1.4 degrees)
        let w = std::thread::spawn(move || {
            let mut watcher = watcher;
            let mut out = Vec::new();
            watcher.run(parse_script("walk 2 0 0; wait 0.5; block -1.75 -0.5 0.25; block -1.75 -1.0 0.25; report").unwrap(), None, |l| out.push(l));
            out
        });
        let mut dout = Vec::new();
        digger.run(parse_script("wait 0.2; dig 0.25 -0.5 0.25; dig 0.25 -1.0 0.25; wait 0.2; block 0.25 -0.5 0.25; report; wait 1").unwrap(), None, |l| dout.push(l));
        let wout = w.join().unwrap();
        assert!(dout[0].contains("\"material\":0"), "{dout:?}");
        assert!(wout[0].contains("\"material\":0") && wout[1].contains("\"material\":0"), "{wout:?}");
        assert!(wout[2].contains("\"players\":2") && wout[2].contains("\"edits_applied\":2"), "{wout:?}");
    }
}
