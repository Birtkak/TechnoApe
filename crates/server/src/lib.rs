//! The authoritative world (ADR 0006): one `Server` owns the `World`, ticks at `TICK_HZ`, applies the edits
//! players send and tells everyone what changed. Connections arrive on a channel, so the same server runs
//! inside the singleplayer client (an in-memory link) and as the dedicated binary (TCP links).
use engine::net::{Link, Player, Recv, ToClient, ToServer, MATERIALS, REACH};
use engine::world::{Block, World};
use glam::DVec3;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

pub type ServerLink = Link<ToClient, ToServer>;

struct Client {
    link: ServerLink,
    player: Player,
}

pub struct Server {
    pub world: World,
    seed: i32,
    accept: Receiver<ServerLink>,
    clients: Vec<Client>,
    next_id: u32,
    pub tick: u64,
}

impl Server {
    pub fn new(planet_n: i32, moon_n: i32, seed: i32, accept: Receiver<ServerLink>) -> Server {
        Server { world: World::new(planet_n, moon_n, seed), seed, accept, clients: Vec::new(), next_id: 1, tick: 0 }
    }

    pub fn players(&self) -> usize {
        self.clients.len()
    }

    /// Ticks forever at `hz`, sleeping between ticks; the thread or process ends it.
    pub fn run(mut self, hz: f64) {
        let step = Duration::from_secs_f64(1.0 / hz);
        let mut next = Instant::now();
        loop {
            self.tick();
            next += step;
            match next.checked_duration_since(Instant::now()) {
                Some(d) => std::thread::sleep(d),
                None => next = Instant::now(), // a late tick does not try to catch up
            }
        }
    }

    pub fn tick(&mut self) {
        while let Ok(link) = self.accept.try_recv() {
            let id = self.next_id;
            self.next_id += 1;
            let [p, m] = [self.world.bodies[0].n, self.world.bodies[1].n];
            link.send(ToClient::Welcome { id, seed: self.seed, planet_n: p, moon_n: m });
            let all: Vec<(Block, u8)> = self
                .world
                .edits
                .iter()
                .flat_map(|(k, cells)| cells.iter().map(move |(pos, m)| (Block { body: k.body, face: k.face, pos: *pos }, *m)))
                .collect();
            if !all.is_empty() {
                link.send(ToClient::Edits(all));
            }
            self.clients.push(Client { link, player: Player { id, name: String::new(), pos: [0.0; 3], yaw: 0.0, pitch: 0.0 } });
        }
        let mut changed: Vec<(Block, u8)> = Vec::new();
        let mut i = 0;
        while i < self.clients.len() {
            let mut open = true;
            loop {
                match self.clients[i].link.try_recv() {
                    Recv::Msg(ToServer::Hello { name }) => self.clients[i].player.name = name.chars().take(32).collect(),
                    Recv::Msg(ToServer::Move { pos, yaw, pitch }) => {
                        if pos.iter().all(|c| c.is_finite()) {
                            let p = &mut self.clients[i].player;
                            (p.pos, p.yaw, p.pitch) = (pos, yaw, pitch);
                        }
                    }
                    Recv::Msg(ToServer::Edit { block, material }) => {
                        let at = DVec3::from_array(self.clients[i].player.pos);
                        if self.valid_edit(block, material, at) && self.world.block(block) != material {
                            self.world.set_block(block, material);
                            changed.push((block, material));
                        }
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
                self.clients.swap_remove(i);
            }
        }
        if !changed.is_empty() {
            for c in &self.clients {
                c.link.send(ToClient::Edits(changed.clone()));
            }
        }
        let players: Vec<Player> = self.clients.iter().map(|c| c.player.clone()).collect();
        for c in &self.clients {
            c.link.send(ToClient::Players(players.clone()));
        }
        self.tick += 1;
    }

    /// The trust boundary: a block that exists, a known material, within the player's reach.
    fn valid_edit(&self, b: Block, m: u8, at: DVec3) -> bool {
        let Some(body) = self.world.bodies.get(b.body as usize) else { return false };
        b.face < 6
            && m < MATERIALS
            && (0..body.n).contains(&b.pos.x)
            && (0..body.n).contains(&b.pos.y)
            && (0..body.layers).contains(&b.pos.z)
            && self.world.block_centre(b).distance(at) <= REACH
    }
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
        let m = if s.world.block(under) == 3 { 0 } else { 3 };
        links[0].send(ToServer::Move { pos: pole.to_array(), yaw: 0.0, pitch: 0.0 });
        links[0].send(ToServer::Edit { block: under, material: m });
        s.tick();
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
        links[0].send(ToServer::Edit { block: under, material: 0 }); // player still at the origin: out of reach
        links[0].send(ToServer::Move { pos: pole.to_array(), yaw: 0.0, pitch: 0.0 });
        links[0].send(ToServer::Edit { block: under, material: 200 }); // unknown material
        links[0].send(ToServer::Edit { block: Block { body: 9, ..under }, material: 0 }); // no such body
        s.tick();
        assert_eq!(s.world.block(under), before);
        assert_eq!(s.world.edit_count, 0);
        assert!(!drain(&links[0]).iter().any(|m| matches!(m, ToClient::Edits(_))));
    }

    #[test]
    fn a_closed_link_removes_the_player() {
        let (mut s, mut links) = server_with(2);
        drop(links.remove(0));
        s.tick();
        assert_eq!(s.players(), 1);
    }
}
