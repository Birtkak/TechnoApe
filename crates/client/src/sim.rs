//! `run=sim` (ADR 0025, S): `stand`'s view at the spawn while the in-process server carries players at work, so
//! a benchmark sees the tick, the meshing of their edits and the frame share the cores. `bots=` players (50)
//! walk a few metres, dig or place a block beside them, or wait, at random; the first walks 6 m ahead of the
//! camera and pours water on the ground it crossed twice a second, which runs off downhill. Every later simulation (air, rivers,
//! plants, creatures, networks) adds its load here.
use bot::{Bot, Cmd};
use engine::blocks::BLOCKS;
use engine::net::{self, ToServer, TICK_HZ};
use glam::DVec3;
use server::ServerLink;
use std::sync::mpsc::Sender;
use std::time::Duration;

/// The direction `run=stand` looks along at a spot whose up is `up` (yaw 0).
pub fn ahead(up: DVec3) -> DVec3 {
    let r = if up.y.abs() > 0.9 { DVec3::Z } else { DVec3::Y };
    (r - up * r.dot(up)).normalize()
}

/// Joins `n` bots to the server through `accept` and drives them on their own thread at the tick rate, until
/// the server closes their links.
pub fn start(accept: Sender<ServerLink>, n: usize, seed: u64) {
    std::thread::spawn(move || {
        let mut bots: Vec<Bot> = (0..n)
            .map(|i| {
                let (c, s) = net::local();
                accept.send(s).expect("the local server");
                Bot::new(&format!("sim{i}"), c)
            })
            .collect();
        // xorshift; the choices still differ from run to run, as the bots' own loop runs on the wall clock
        let mut rng = seed.wrapping_mul(0x9e3779b97f4a7c15) | 1;
        let mut f = move || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            (rng >> 11) as f64 / (1u64 << 53) as f64
        };
        let stone = BLOCKS.id("stone");
        let mut tick = 0u64;
        let mut pourer_placed = false;
        loop {
            for (i, b) in bots.iter_mut().enumerate() {
                if !b.tick() {
                    return;
                }
                if b.world.is_none() || b.busy() {
                    continue;
                }
                let up = b.pos.normalize();
                let side = (DVec3::new(f() - 0.5, f() - 0.5, f() - 0.5)).reject_from(up).normalize_or(ahead(up));
                if i == 0 {
                    if !pourer_placed {
                        pourer_placed = true;
                        b.exec(&Cmd::Walk(ahead(up) * 6.0));
                    } else if tick % (TICK_HZ as u64 / 2) == 0 {
                        let at = b.world.as_ref().unwrap().block_at(b.pos - ahead(up) * 1.5 + up * 0.25);
                        b.link.send(ToServer::Bucket { seq: 0, at, fill: false });
                    }
                    continue;
                }
                match f() {
                    x if x < 0.4 => {
                        b.exec(&Cmd::Walk(side * (2.0 + 4.0 * f())));
                    }
                    x if x < 0.9 => {
                        let c = if x < 0.65 { Cmd::Dig(side - up * 0.25) } else { Cmd::Place(side + up * 0.25, stone) };
                        b.exec(&c);
                        b.exec(&Cmd::Wait(0.3 + 0.7 * f()));
                    }
                    _ => {
                        b.exec(&Cmd::Wait(1.0));
                    }
                }
            }
            tick += 1;
            std::thread::sleep(Duration::from_secs_f64(1.0 / TICK_HZ));
        }
    });
}
