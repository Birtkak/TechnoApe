//! The fast-forward soak (`docs/design/fundamentals.md` section 4, step 1; ADR 0011). Seeded random bots walk,
//! jump, dig, place and teleport, half the teleports onto cube seams and corners, while the server ticks
//! without sleeping and checks after every tick: no walking player inside a block, nothing NaN, nothing under
//! the core, and every bot's copy of the world equals the server's. A failure is shrunk to the fewest bots
//! that still fail it.
use crate::save::Save;
use crate::Server;
use engine::net::{local, Link, Recv, ToClient, ToServer};
use engine::blocks::BLOCKS;
use engine::phys::{self, Input};
use engine::world::World;
use engine::edit::{self, Kind, Predictor, Shape};
use glam::{DVec3, IVec3};
use std::path::Path;
use rayon::prelude::*;
use std::sync::mpsc::channel;

/// Luna's columns per face edge in the soak: at 128 (radius 88 m) its +-30 m hills stay over its core, at 32
/// (radius 22 m) a sixth of its ground lies under the core and walking bots fell in.
pub const MOON_N: i32 = 128;

/// splitmix64: each bot draws from its own stream, so removing one bot leaves the others' choices as they were.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }
    fn f(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn range(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.f()
    }
    fn dir(&mut self) -> DVec3 {
        DVec3::new(self.range(-1.0, 1.0), self.range(-1.0, 1.0), self.range(-1.0, 1.0)).normalize_or(DVec3::X)
    }
}

struct Bot {
    rng: Rng,
    link: Link<ToServer, ToClient>,
    world: World,
    pred: Predictor,
    /// A dig held (ADR 0034): the shape and the tick its `Edit` goes, once its time is up.
    digging: Option<(Shape, u64)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Fail {
    pub tick: u64,
    pub what: String,
}

/// A spot just above the ground of body `bi`; half of them within two columns of a face edge, a quarter of
/// those at a corner, where the grid folds.
fn spot(w: &World, r: &mut Rng) -> DVec3 {
    let bi = if r.f() < 0.1 { 1 } else { 0 };
    let b = &w.bodies[bi];
    let n = b.n as f64;
    let edge = |r: &mut Rng| if r.f() < 0.5 { r.range(0.05, 2.0) } else { n - r.range(0.05, 2.0) };
    let (i, j) = match r.f() {
        x if x < 0.125 => (edge(r), edge(r)),
        x if x < 0.5 => (edge(r), r.range(0.0, n)),
        _ => (r.range(0.0, n), r.range(0.0, n)),
    };
    let (i, j) = if r.f() < 0.5 { (i, j) } else { (j, i) };
    let d = b.dir((r.next() % 6) as usize, i, j);
    b.centre + d * (b.height(d) + 1.0)
}

fn act(b: &mut Bot, s: &Server, me: usize) {
    // a held dig whose time is up is sent (survival takes it, unless the bot moved or the cells changed)
    if let Some((shape, _)) = b.digging.filter(|d| s.tick >= d.1) {
        b.digging = None;
        let me = &s.clients[me].phys;
        let (seq, _) = b.pred.edit(&mut b.world, &shape, 0, |w, c| phys::holds(w, me, c));
        b.link.send(ToServer::Edit { seq, shape, material: 0 });
        return;
    }
    let r = b.rng.f();
    let walk = |rng: &mut Rng| {
        let d = rng.dir();
        // now and then flying, which only creative bots may
        let fly = if rng.f() < 0.1 { rng.range(1.0, 40.0) as f32 } else { 0.0 };
        Input { mv: [rng.range(-1.0, 1.0) as f32, rng.range(-1.0, 1.0) as f32], jump: rng.f() < 0.3, fwd: d.as_vec3().to_array(), pitch: 0.0, turn: rng.range(-0.1, 0.1) as f32, tilt: rng.range(-0.05, 0.05) as f32, roll: rng.range(-0.05, 0.05) as f32, fly, down: rng.f() < 0.3, t: 0.0 }
    };
    if r < 0.05 {
        b.link.send(ToServer::Input(walk(&mut b.rng)));
    } else if r < 0.055 {
        // the teleport is the test's device, not the thing under test: it only lands where there is room
        let p = spot(&s.world, &mut b.rng);
        let fwd = phys::tangent(DVec3::X, phys::up(&s.world, p), DVec3::Z);
        if !phys::blocked(&s.world, p) {
            b.link.send(ToServer::Move { pos: p.to_array(), fwd: fwd.as_vec3().to_array(), pitch: 0.0 });
            b.link.send(ToServer::Input(walk(&mut b.rng)));
        }
    } else if r < 0.085 {
        let feet = s.clients[me].phys.pos;
        let at = feet + b.rng.dir() * b.rng.range(0.0, 3.0);
        let ids: Vec<u8> = BLOCKS.placeable().map(|d| d.id).collect();
        let m = if b.rng.f() < 0.5 { 0 } else { ids[(b.rng.next() % ids.len() as u64) as usize] };
        // a single block, the 1 m brush, a wall or a floor, some of them over the size limit or out of reach
        let mut k = |n: u64| (b.rng.next() % n) as i32;
        let ext = match k(4) {
            0 => IVec3::ZERO,
            1 => IVec3::ONE,
            2 => IVec3::new(k(8), 0, k(3)),
            _ => IVec3::new(k(40), k(40), 0),
        };
        let at = s.world.block_at(at);
        let kind = [Kind::Box, Kind::Cylinder, Kind::Cone][k(3) as usize];
        let shape = Shape { max: at.pos + ext, kind, ..Shape::one(at) };
        // half the digs are held for their time first, now and then let go early; the rest come at once
        if m == 0 && b.rng.f() < 0.5 {
            let t = edit::dig_time(&s.world, &shape) as f64 * if b.rng.f() < 0.8 { 1.05 } else { 0.5 };
            b.link.send(ToServer::Dig(Some(shape)));
            b.digging = Some((shape, s.tick + (t / s.dt).ceil() as u64 + 1));
            return;
        }
        // the bot predicts like a client; its own box is the only one it keeps clear
        let me = &s.clients[me].phys;
        let (seq, _) = b.pred.edit(&mut b.world, &shape, m, |w, c| phys::holds(w, me, c));
        b.link.send(ToServer::Edit { seq, shape, material: m });
    } else if r < 0.087 {
        b.link.send(ToServer::Creative(b.rng.f() < 0.5));
    } else if r < 0.0875 {
        let lines = ["hi", "/time", "/time add 30", "/give stone 3", "/give nothing", "/playmode survival", "/bad", "/time set 99:99"];
        b.link.send(ToServer::Chat(lines[(b.rng.next() % lines.len() as u64) as usize].into()));
    } else if r < 0.1 {
        // the bucket, mostly where it reaches water or air, some of it out of reach or into rock
        let feet = s.clients[me].phys.pos;
        let at = s.world.block_at(feet + b.rng.dir() * b.rng.range(0.0, 5.0));
        b.link.send(ToServer::Bucket { seq: b.pred.seq(), at, fill: b.rng.f() < 0.5 });
    }
}

fn check(s: &Server) -> Result<(), String> {
    // no water appears or vanishes: what differs from the seed is what buckets hold plus what creative made
    if s.water_ledger() != 0 {
        return Err(format!("{} units of water appeared", s.water_ledger()));
    }
    for c in &s.clients {
        let p = &c.phys;
        let id = c.player.id;
        if !(p.pos.is_finite() && p.vel.is_finite() && p.fwd.is_finite() && c.player.pos.iter().all(|x| x.is_finite())) {
            return Err(format!("player {id} not finite: pos {:?} vel {:?}", p.pos, p.vel));
        }
        let b = &s.world.bodies[s.world.nearest(p.pos)];
        if (p.pos - b.centre).length() < b.r0 {
            return Err(format!("player {id} under the core at {:?}", p.pos));
        }
        if c.input.is_some() {
            if phys::inside(&s.world, p) {
                return Err(format!("player {id} inside a block at {:?}", p.pos.to_array()));
            }
        }
    }
    Ok(())
}

/// What the water did in a soak: the most columns stored and awake at once, steps skipped for the budget,
/// bytes of water sent.
#[derive(Debug, PartialEq, Default)]
pub struct WaterStats {
    pub stored: usize,
    pub awake: usize,
    pub skipped: u64,
    pub bytes: u64,
}

/// Runs `ticks` ticks with the bots `which` (numbers into the seed's endless list of bots) on a planet of
/// `planet_n` columns per face edge. `Ok` holds the final state hash and what the water did.
pub fn run(seed: u64, which: &[u32], ticks: u64, planet_n: i32, record: Option<&Path>) -> Result<(u64, WaterStats), Fail> {
    let mut ws = WaterStats::default();
    let (tx, rx) = channel();
    // the planet baked (ADR 0043), as a new game's: the bots share its map, as a local client does
    let mut s = Server::baked(planet_n, MOON_N, seed as i32, rx);
    let map = s.world.bodies[0].map().cloned();
    if let Some(p) = record {
        s.record(p).map_err(|e| Fail { tick: 0, what: format!("cannot record to {}: {e}", p.display()) })?;
    }
    let mut bots: Vec<Bot> = which
        .iter()
        .map(|&k| {
            let (c, sv) = local();
            tx.send(sv).unwrap();
            c.send(ToServer::Hello { name: format!("bot{k}") }); // named, so the save keeps it
            // an empty input makes it a walker, as in the bot crate
            c.send(ToServer::Input(Input::default()));
            Bot { rng: Rng(seed.wrapping_mul(0x2545f4914f6cdd1d) ^ (k as u64 + 1).wrapping_mul(0x9e3779b97f4a7c15)), link: c, world: World::with_map(planet_n, MOON_N, seed as i32, map.clone()), pred: Predictor::default(), digging: None }
        })
        .collect();
    s.tick(); // everyone joins
    for t in 1..ticks {
        for (me, b) in bots.iter_mut().enumerate() {
            act(b, &s, me);
        }
        s.tick();
        // each bot keeps its own world: they take what the server sent side by side
        bots.par_iter_mut().for_each(|b| {
            while let Recv::Msg(m) = b.link.try_recv() {
                match m {
                    ToClient::Edits(v) => {
                        for (blk, m) in v {
                            b.pred.server(&mut b.world, blk, m);
                        }
                    }
                    ToClient::Done { seq, .. } => {
                        b.pred.done(&mut b.world, seq);
                    }
                    ToClient::Water(v) => {
                        for (t, cols) in &v {
                            b.world.water.set_tile(*t, cols);
                        }
                    }
                    // a bot that dies (a survival bot's hard landing, ADR 0040) is back at the spawn at once
                    ToClient::Vitals { dead: Some(_), .. } => b.link.send(ToServer::Respawn),
                    _ => {}
                }
            }
        });
        check(&s).map_err(|what| Fail { tick: t, what })?;
        (ws.stored, ws.awake) = (ws.stored.max(s.world.water.stored()), ws.awake.max(s.world.water.awake()));
        if t % 100 == 0 || t + 1 == ticks {
            let truth = s.world.chunk_hashes();
            for (k, b) in bots.iter().enumerate() {
                let mine = b.world.chunk_hashes();
                if mine != truth {
                    let at = truth.iter().zip(&mine).find(|(a, b)| a != b).map(|(a, _)| a.0).or(truth.get(mine.len()).map(|a| a.0)).or(mine.get(truth.len()).map(|a| a.0));
                    return Err(Fail { tick: t, what: format!("bot {} world differs from the server's at chunk {at:?}", which[k]) });
                }
                // a settled tile the bot has the latest of is the server's, exactly
                let w = &s.world.water;
                let bad = s.water_seen(k).into_iter().find(|&tl| w.tile_settled(tl) && w.tile(tl) != b.world.water.tile(tl));
                if let Some(tl) = bad {
                    return Err(Fail { tick: t, what: format!("bot {} water differs from the server's in tile {tl:?}: {:?} against {:?}", which[k], b.world.water.tile(tl), w.tile(tl)) });
                }
            }
        }
    }
    // a save and a load give back the same state, down to every player's inventory
    let snap = s.snapshot();
    let back = Save::from_bytes(&snap.to_bytes()).map(|v| Server::from_save(v, channel().1));
    match back {
        Ok(b) if b.snapshot() == snap && b.world.hash() == s.world.hash() => {
            (ws.skipped, ws.bytes) = (s.water_skipped, s.water_bytes);
            Ok((s.hash(), ws))
        }
        Ok(b) => {
            let (x, y) = (b.snapshot(), &snap);
            let wd = x.water.iter().zip(&y.water).find(|(a, b)| a != b).map(|(a, b)| format!("{a:?} became {b:?}"));
            let what = format!("a save and a load changed the state: edits {} players {} water {:?} ({} and {} columns) hash {}", x.edits == y.edits, x.players == y.players, wd, x.water.len(), y.water.len(), b.world.hash() == s.world.hash());
            Err(Fail { tick: ticks, what })
        }
        Err(e) => Err(Fail { tick: ticks, what: format!("the save does not load: {e}") }),
    }
}

/// Drops bots one at a time while the run still fails, by tick `fail.tick` at the latest.
pub fn shrink(seed: u64, mut which: Vec<u32>, mut fail: Fail, planet_n: i32) -> (Vec<u32>, Fail) {
    let mut i = 0;
    while i < which.len() && which.len() > 1 {
        let mut fewer = which.clone();
        fewer.remove(i);
        match run(seed, &fewer, fail.tick + 1, planet_n, None) {
            Err(f) => (which, fail) = (fewer, f),
            Ok(_) => i += 1,
        }
    }
    (which, fail)
}

#[cfg(test)]
mod tests {
    use super::*;

    const N: i32 = 256;

    #[test]
    fn soak_holds_its_invariants() {
        let bots: Vec<u32> = (0..8).collect();
        if let Err(f) = run(7, &bots, 600, N, None) {
            let (keep, f) = shrink(7, bots, f, N);
            panic!("tick {}: {} (bots {keep:?})", f.tick, f.what);
        }
    }

    #[test]
    fn the_same_seed_gives_the_same_state() {
        let bots: Vec<u32> = (0..4).collect();
        assert_eq!(run(3, &bots, 300, N, None), run(3, &bots, 300, N, None));
        assert_ne!(run(3, &bots, 300, N, None), run(4, &bots, 300, N, None));
    }

    #[test]
    fn a_replay_meets_every_checkpoint_of_its_recording() {
        let path = std::env::temp_dir().join(format!("technoape-replay-test-{}.rec", std::process::id()));
        let bots: Vec<u32> = (0..4).collect();
        let end = run(5, &bots, 301, N, Some(&path)).expect("soak");
        let r = crate::replay(&path, Some(301)).expect("replay");
        let _ = std::fs::remove_file(&path);
        assert_eq!(r.diverged, None);
        assert_eq!(r.checks, 15, "a checkpoint per second of 300 ticks");
        assert_eq!(r.server.hash(), end.0, "the replay ends in the soak's state");
    }

    #[test]
    fn a_replay_names_the_checkpoint_where_it_parts_from_the_recording() {
        let path = std::env::temp_dir().join(format!("technoape-diverge-test-{}.rec", std::process::id()));
        run(5, &[0, 1], 101, N, Some(&path)).expect("soak");
        let mut evs = crate::read_recording(&path).expect("read");
        for e in &mut evs {
            if let crate::Ev::Check { tick: 60, hash } = e {
                *hash ^= 1;
            }
        }
        let mut bytes = Vec::new();
        for e in &evs {
            let b = postcard::to_stdvec(e).unwrap();
            bytes.extend((b.len() as u32).to_le_bytes());
            bytes.extend(b);
        }
        std::fs::write(&path, bytes).unwrap();
        let r = crate::replay(&path, None).expect("replay");
        let _ = std::fs::remove_file(&path);
        assert_eq!(r.diverged.map(|d| d.0), Some(60));
        assert_eq!(r.checks, 3);
    }
}
