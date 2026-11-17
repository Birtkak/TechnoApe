//! The fast-forward soak (`docs/design/fundamentals.md` section 4, step 1; ADR 0011). Seeded random bots walk,
//! jump, dig, place and teleport, half the teleports onto cube seams and corners, while the server ticks
//! without sleeping and checks after every tick: no walking player inside a block (but loose sand or gravel
//! fallen on it, ADR 0079), nothing NaN, nothing under
//! the core, and every bot's copy of the world equals the server's. A failure is shrunk to the fewest bots
//! that still fail it.
use crate::save::Save;
use crate::Server;
use engine::net::{local, Link, Recv, ToClient, ToServer};
use engine::blocks::BLOCKS;
use engine::items::{Holder, ITEMS};
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
    /// A dam it built across a river and the tick it breaks it (W5), and how many it built and broke.
    dam: Option<(Shape, u64)>,
    dams: u32,
    broken: u32,
    heaps: u32,
}

/// A wet point of the baked map's rivers, the way across its channel there and its half width (m), or None
/// when the planet has no river with water.
fn river_point(w: &World, r: &mut Rng) -> Option<(DVec3, DVec3, f64)> {
    let segs = &w.bodies[0].map()?.lines.segs;
    if segs.is_empty() {
        return None;
    }
    for _ in 0..8 {
        let g = &segs[(r.next() % segs.len().max(1) as u64) as usize];
        if g.lake || g.wl[0] <= g.bed[0] {
            continue;
        }
        let p = g.a + (g.b - g.a) * r.f();
        let across = (g.b - g.a).cross(p).normalize_or(DVec3::X);
        return Some((p.normalize(), across, g.hw[0] as f64));
    }
    None
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
        let (me, survival) = (&s.clients[me].phys, !s.clients[me].creative);
        let (seq, _) = b.pred.edit(&mut b.world, &shape, 0, survival, |w, c| phys::holds(w, me, c));
        b.link.send(ToServer::Edit { seq, shape, material: 0 });
        return;
    }
    // a dam whose time is up is dug out again: the pond behind it runs off down the river
    if let Some((shape, _)) = b.dam.filter(|d| s.tick >= d.1) {
        b.dam = None;
        b.broken += 1;
        let (me, survival) = (&s.clients[me].phys, !s.clients[me].creative);
        let (seq, _) = b.pred.edit(&mut b.world, &shape, 0, survival, |w, c| phys::holds(w, me, c));
        b.link.send(ToServer::Edit { seq, shape, material: 0 });
        return;
    }
    let r = b.rng.f();
    let walk = |rng: &mut Rng| {
        let d = rng.dir();
        // now and then flying, which only creative bots may
        let fly = if rng.f() < 0.1 { rng.range(1.0, 40.0) as f32 } else { 0.0 };
        Input { mv: [rng.range(-1.0, 1.0) as f32, rng.range(-1.0, 1.0) as f32], jump: rng.f() < 0.3, fwd: d.as_vec3().to_array(), pitch: 0.0, turn: rng.range(-0.1, 0.1) as f32, tilt: rng.range(-0.05, 0.05) as f32, roll: rng.range(-0.05, 0.05) as f32, fly, down: rng.f() < 0.3, t: 0.0, sprint: rng.f() < 0.3 }
    };
    if r < 0.05 {
        b.link.send(ToServer::input(walk(&mut b.rng)));
    } else if r < 0.055 {
        // the teleport is the test's device, not the thing under test: it only lands where there is room
        let p = spot(&s.world, &mut b.rng);
        let fwd = phys::tangent(DVec3::X, phys::up(&s.world, p), DVec3::Z);
        if !phys::blocked(&s.world, p) {
            b.link.send(ToServer::Move { pos: p.to_array(), fwd: fwd.as_vec3().to_array(), pitch: 0.0 });
            b.link.send(ToServer::input(walk(&mut b.rng)));
        }
    } else if r < 0.085 {
        let feet = s.clients[me].phys.pos;
        let at = feet + b.rng.dir() * b.rng.range(0.0, 3.0);
        let ids: Vec<u16> = BLOCKS.placeable().map(|d| d.id).collect();
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
        let (me, survival) = (&s.clients[me].phys, !s.clients[me].creative);
        let (seq, _) = b.pred.edit(&mut b.world, &shape, m, survival, |w, c| phys::holds(w, me, c));
        b.link.send(ToServer::Edit { seq, shape, material: m });
    } else if r < 0.087 {
        b.link.send(ToServer::Creative(b.rng.f() < 0.5));
    } else if r < 0.0875 {
        let lines = ["hi", "/time", "/time add 30", "/give stone 3", "/give nothing", "/playmode survival", "/bad", "/time set 99:99", "/give basket", "/give dirt 700"];
        b.link.send(ToServer::Chat(lines[(b.rng.next() % lines.len() as u64) as usize].into()));
    } else if r < 0.1 {
        // the bucket, mostly where it reaches water or air, some of it out of reach or into rock
        let feet = s.clients[me].phys.pos;
        let at = s.world.block_at(feet + b.rng.dir() * b.rng.range(0.0, 5.0));
        // or a drink there (needs.md), held on, or let go
        match b.rng.f() {
            f if f < 0.2 => b.link.send(ToServer::Drink(Some(at))),
            f if f < 0.3 => b.link.send(ToServer::Drink(None)),
            _ => b.link.send(ToServer::Bucket { seq: b.pred.seq(), at, fill: b.rng.f() < 0.5 }),
        }
    } else if r < 0.1 + ITEM_SHARE {
        // items (ADR 0074): moves, splits, throws, piles and creative's grid at random, some onto full or wrong
        // slots, some from piles out of reach, some of no item; the items' ledger must hold through all of them
        let pick = |rng: &mut Rng| -> (Holder, u16) {
            match rng.next() % 8 {
                0..=3 => (Holder::Ape, (rng.next() % 40) as u16),
                4 | 5 if !s.piles.is_empty() => (Holder::Pile(crate::unkey(*s.piles.keys().nth((rng.next() % s.piles.len() as u64) as usize).unwrap())), (rng.next() % 5) as u16),
                6 => {
                    let all: Vec<u16> = ITEMS.all().collect();
                    (Holder::Grid, if rng.f() < 0.9 { all[(rng.next() % all.len() as u64) as usize] } else { 0x7fff })
                }
                _ => (Holder::Ground, 0),
            }
        };
        let (from, to) = (pick(&mut b.rng), pick(&mut b.rng));
        let n = (b.rng.next() % 70) as u16;
        b.link.send(ToServer::Items { seq: b.pred.seq(), from, to, n });
    } else if r < 0.102 + ITEM_SHARE && b.dam.is_none() {
        // a dam across a river (W5): from its bank, in creative, a plug of stone over the channel's width,
        // broken again some seconds later; the water's count must hold through both
        let Some((d, across, hw)) = river_point(&s.world, &mut b.rng) else { return };
        let body = &s.world.bodies[0];
        let bank = (d * body.radius + across * (hw + 2.0)).normalize();
        let stand = body.centre + bank * (body.height(bank) + 1.0);
        if phys::blocked(&s.world, stand) {
            return;
        }
        let fwd = phys::tangent(DVec3::X, phys::up(&s.world, stand), DVec3::Z);
        b.link.send(ToServer::Creative(true));
        b.link.send(ToServer::Move { pos: stand.to_array(), fwd: fwd.as_vec3().to_array(), pitch: 0.0 });
        let at = s.world.block_at(body.centre + d * (body.height(d) + 0.5));
        let w = ((2.0 * hw) as i32 + 3).min(12);
        let min = at.pos - IVec3::new(w / 2, w / 2, 0);
        let shape = Shape { min, max: min + IVec3::new(w - 1, w - 1, 2), ..Shape::one(at) };
        let stone = BLOCKS.placeable().find(|d| d.name == "stone").map_or(1, |d| d.id);
        let (seq, _) = b.pred.edit(&mut b.world, &shape, stone, false, |_, _| true);
        b.link.send(ToServer::Edit { seq, shape, material: stone });
        b.dam = Some((shape, s.tick + 120 + b.rng.next() % 240));
        b.dams += 1;
    } else if (0.102 + ITEM_SHARE..0.104 + ITEM_SHARE).contains(&r) {
        // a heap of sand or gravel beside the bot, in creative, and the ground dug out from under it at once:
        // it falls into the hole and slides to its angle of rest (ADR 0079)
        let feet = s.clients[me].phys.pos;
        let fwd = phys::tangent(b.rng.dir(), phys::up(&s.world, feet), DVec3::X);
        let at = s.world.block_at(feet + fwd * 3.0);
        let m = BLOCKS.id(if b.rng.f() < 0.5 { "sand" } else { "gravel" });
        let heap = Shape { min: at.pos - IVec3::new(1, 1, 0), max: at.pos + IVec3::new(1, 1, 2), ..Shape::one(at) };
        let hole = Shape { min: at.pos - IVec3::new(1, 1, 3), max: at.pos + IVec3::new(1, 1, -1), ..Shape::one(at) };
        b.link.send(ToServer::Creative(true));
        let me = &s.clients[me].phys;
        for (shape, m) in [(heap, m), (hole, 0)] {
            // creative from the message just sent, not the server's mode of last tick
            let (seq, _) = b.pred.edit(&mut b.world, &shape, m, false, |w, c| phys::holds(w, me, c));
            b.link.send(ToServer::Edit { seq, shape, material: m });
        }
        b.heaps += 1;
    }
}

/// The share of a bot's ticks that move items (ADR 0074).
const ITEM_SHARE: f64 = 0.01;

fn check(s: &Server) -> Result<(), String> {
    // no item appears or vanishes: what every player and pile holds is what digs, places, the grid and /give made
    if let Some(&(k, held, made)) = s.item_ledger().first() {
        return Err(format!("{}: {held} held, {made} made", engine::items::ITEMS.name(k)));
    }
    // no animal is left inside the ground or off to infinity (ADR 0083); rubble and sand that fell on one bury it
    // as they bury a player (ADR 0079, 0093)
    let fell = |b| s.world.edit(b).map_or(!BLOCKS.solid(s.world.block(b)), |m| !BLOCKS.solid(m) || BLOCKS.get(m).is_some_and(|d| d.falls));
    if let Some(a) = s.herds.all().find(|a| !a.p.pos.is_finite() || (engine::phys::inside(&s.world, &a.p) && !engine::phys::cells(&s.world, a.p.pos).all(fell))) {
        return Err(format!("animal {} at {:?} is inside the ground", a.id, a.p.pos));
    }
    // no water appears or vanishes: what differs from the seed is what buckets hold plus what creative made
    if s.water_ledger() != 0 {
        return Err(format!("{} units of water appeared", s.water_ledger()));
    }
    // nor does any of the air's: what it handed the rivers as rain is in the water's ledger (W5)
    if let Some(x) = s.air_ledger().filter(|&x| x != 0) {
        return Err(format!("{x} units of the air's water appeared"));
    }
    // no ore block appears or vanishes unseen: each body's ledger is what a recount from the edits says
    if let Some(&(k, ledger, recount)) = s.ore_ledger().first() {
        return Err(format!("ore body {k}: the ledger holds {ledger} blocks, the edits say {recount}"));
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
        // no need outside 0..20 (needs.md)
        let max = engine::vitals::MAX as f64;
        if !((0.0..=max).contains(&c.vitals.hunger) && (0.0..=max).contains(&c.vitals.thirst)) {
            return Err(format!("player {id}'s needs out of range: {:?}", c.vitals));
        }
        if c.input.is_some() {
            // (sand and gravel fall on players and bury them: ADR 0079)
            // (only sand that came there by an edit, a fall: generated dunes still count as blocks)
            let loose = |b| s.world.edit(b).map_or(!BLOCKS.solid(s.world.block(b)), |m| !BLOCKS.solid(m) || BLOCKS.get(m).is_some_and(|d| d.falls));
            if phys::inside(&s.world, p) && !phys::cells(&s.world, p.pos).all(loose) {
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
    /// Dams bots built across rivers, and broke again (W5, ADR 0059).
    pub dams: u32,
    pub broken: u32,
    /// Heaps of sand bots dug out from under (ADR 0079), and the blocks that fell or slid.
    pub heaps: u32,
    pub falls: u64,
}

/// Runs `ticks` ticks with the bots `which` (numbers into the seed's endless list of bots) on a planet of
/// `planet_n` columns per face edge. `Ok` holds the final state hash and what the water did.
pub fn run(seed: u64, which: &[u32], ticks: u64, planet_n: i32, record: Option<&Path>) -> Result<(u64, WaterStats), Fail> {
    run_warp(seed, which, ticks, planet_n, record, 1)
}

/// `run` with the clock `warp` times faster (/timewarp): the tide (ADR 0067) runs through many cycles in a short
/// soak. The first bot of `which` asks for it in chat as it joins, so the recording carries it and a replay, or a
/// repro with `only=` any subset, warps the same.
pub fn run_warp(seed: u64, which: &[u32], ticks: u64, planet_n: i32, record: Option<&Path>, warp: u64) -> Result<(u64, WaterStats), Fail> {
    let mut ws = WaterStats::default();
    let (tx, rx) = channel();
    // the planet baked (ADR 0043), as a new game's: the bots share its map, as a local client does
    let mut s = Server::baked(planet_n, MOON_N, seed as i32, rx);
    let (map, luna) = (s.world.bodies[0].map().cloned(), s.world.bodies[1].map().cloned());
    if let Some(p) = record {
        s.record(p).map_err(|e| Fail { tick: 0, what: format!("cannot record to {}: {e}", p.display()) })?;
    }
    let mut bots: Vec<Bot> = which
        .iter()
        .enumerate()
        .map(|(i, &k)| {
            let (c, sv) = local();
            tx.send(sv).unwrap();
            c.send(ToServer::Hello { name: format!("bot{k}") }); // named, so the save keeps it
            if i == 0 && warp > 1 {
                c.send(ToServer::Chat(format!("/timewarp {warp}")));
            }
            // an empty input makes it a walker, as in the bot crate
            c.send(ToServer::Input(Input::default()));
            Bot { rng: Rng(seed.wrapping_mul(0x2545f4914f6cdd1d) ^ (k as u64 + 1).wrapping_mul(0x9e3779b97f4a7c15)), link: c, world: World::with_maps(planet_n, MOON_N, seed as i32, map.clone(), luna.clone()), pred: Predictor::default(), digging: None, dam: None, dams: 0, broken: 0, heaps: 0 }
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
            (ws.dams, ws.broken) = (bots.iter().map(|b| b.dams).sum(), bots.iter().map(|b| b.broken).sum());
            (ws.heaps, ws.falls) = (bots.iter().map(|b| b.heaps).sum(), s.falls.moves);
            eprintln!("[soak] animals: {} met in {} sites, {} awake at the end", s.herds.all().count(), s.herds.sites.len(), s.herds.awake);
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
pub fn shrink(seed: u64, mut which: Vec<u32>, mut fail: Fail, planet_n: i32, warp: u64) -> (Vec<u32>, Fail) {
    let mut i = 0;
    while i < which.len() && which.len() > 1 {
        let mut fewer = which.clone();
        fewer.remove(i);
        match run_warp(seed, &fewer, fail.tick + 1, planet_n, None, warp) {
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
        match run(7, &bots, 600, N, None) {
            Err(f) => {
                let (keep, f) = shrink(7, bots, f, N, 1);
                panic!("tick {}: {} (bots {keep:?})", f.tick, f.what);
            }
            Ok(_) => {}
        }
    }

    /// Bots dam the rivers and break the dams again (W5, ADR 0059) on the smallest planet with wet rivers:
    /// the water's count holds every tick, and the same seed gives the same state twice.
    #[test]
    fn dams_built_and_broken_keep_the_count_and_the_state() {
        let bots: Vec<u32> = (0..4).collect();
        let a = run(7, &bots, 900, 1536, None).unwrap_or_else(|f| panic!("tick {}: {}", f.tick, f.what));
        assert!(a.1.dams > 2 && a.1.broken > 1, "{:?}", a.1);
        assert_eq!(run(7, &bots, 900, 1536, None).map(|r| r.0), Ok(a.0));
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

    /// A soak under time warp (`warp=`): the clock runs that many times as far, the invariants hold, and the
    /// warp is in the recording, so its replay ends in the soak's state with no argument of its own; also when
    /// the run is a subset of bots without bot 0 (a repro's `only=`).
    #[test]
    fn a_warped_soak_runs_the_clock_faster_and_its_replay_follows() {
        let path = |w| std::env::temp_dir().join(format!("technoape-warp-test-{w}-{}.rec", std::process::id()));
        let mut time = Vec::new();
        for (w, bots) in [(1, vec![0, 1, 2, 3]), (30, vec![0, 1, 2, 3]), (30, vec![2, 3])] {
            let end = run_warp(5, &bots, 301, N, Some(&path(w)), w).unwrap_or_else(|f| panic!("warp {w}, tick {}: {}", f.tick, f.what));
            let r = crate::replay(&path(w), Some(301)).expect("replay");
            let _ = std::fs::remove_file(path(w));
            assert_eq!((r.diverged, r.server.hash()), (None, end.0), "warp {w}, bots {bots:?}: the replay ends in the soak's state");
            time.push(r.server.world.time);
        }
        for k in [time[1] / time[0], time[2] / time[0]] {
            assert!((29.0..=30.0).contains(&k), "the clock ran {k} times as far: {time:?}");
        }
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
