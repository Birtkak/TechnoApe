//! Loose blocks fall and slide as whole blocks (ADR 0079, `docs/design/fine-cells.md` (a)): sand and gravel
//! (`falls` in `data/blocks.toml`). The server's: only what is awake is visited, in a fixed order, so a tick
//! gives the same world on every run and in a replay.
use crate::blocks::{AIR, BLOCKS};
use crate::world::{fold, Block, World};
use glam::IVec3;
use std::collections::BTreeMap;

/// Awake cells visited a tick at most; the rest wait for the next tick, the lowest first. A visit is some
/// 0.5 us (`fall_cost`, play build), up to 3.3 us where every lookup is the generated ground's: at most
/// about 3.5 ms of the 50 ms tick.
pub const BUDGET: usize = 1024;

/// ADR 0101's slides of a pile's top, sloped tops on loose blocks (`mesh`) and glides of moved blocks (client):
/// off for now (ADR 0106, the developer: "just have sand and gravel fall down"); true brings all three back.
pub const SLOPES_AND_SLIDES: bool = false;

/// (body, face, layer, i, j): layer first, so a tick visits bottom up and a stack falls together.
type Key = (u8, u8, i32, i32, i32);

fn key(b: Block) -> Key {
    (b.body, b.face, b.pos.z, b.pos.x, b.pos.y)
}
fn cell(k: Key) -> Block {
    Block { body: k.0, face: k.1, pos: IVec3::new(k.3, k.4, k.2) }
}

/// The cell `di`, `dj` columns over and `dl` layers up from `b`, across a face's edge when it is one.
fn near(world: &World, b: Block, di: i32, dj: i32, dl: i32) -> Block {
    let (f, i, j) = fold(b.face as usize, b.pos.x + di, b.pos.y + dj, world.bodies[b.body as usize].n);
    Block { body: b.body, face: f as u8, pos: IVec3::new(i, j, b.pos.z + dl) }
}

const SIDES: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

/// Cells the fall reaches when it has fallen for `k` ticks of 50 ms from rest: g t^2 / 2 rounded, so the
/// first cell goes once the block's middle is past the edge (0.32 s), at most one a tick.
fn due(k: u32) -> u32 {
    let t = k as f64 / 20.0;
    (4.905 * t * t + 0.5) as u32
}

#[derive(Default, Debug)]
pub struct Falls {
    /// Awake cells, with (ticks it has fallen, cells it has fallen) for a block in flight, (0, 0) otherwise.
    awake: BTreeMap<Key, (u32, u32)>,
    /// Blocks moved and cells visited since the start (counted work, fundamentals).
    pub moves: u64,
    pub visits: u64,
}

impl Falls {
    pub fn awake(&self) -> usize {
        self.awake.len()
    }

    /// Cell `b` alone is looked at next tick (a save's falling blocks on loading).
    pub fn poke(&mut self, b: Block) {
        self.awake.entry(key(b)).or_insert((0, 0));
    }

    /// `b` changed: it, the cell over it and the four sides at both layers may now fall or slide, and the cell
    /// under it, which may be a pile's top now (ADR 0101).
    pub fn wake(&mut self, world: &World, b: Block) {
        self.awake.entry(key(near(world, b, 0, 0, -1))).or_insert((0, 0));
        for dl in 0..=1 {
            for (di, dj) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
                self.awake.entry(key(near(world, b, di, dj, dl))).or_insert((0, 0));
            }
        }
    }

    /// One tick: every awake falling block falls a cell when its time says, else, the top of its pile, slides off
    /// a step of two (ADR 0101), else sleeps. A player is no floor: it falls through the player's cells and buries them (the developer,
    /// 2026-11-13; the server takes their breath). Each move is two edits, pushed to `out`.
    pub fn step(&mut self, world: &mut World, tick: u64, out: &mut Vec<(Block, u16)>) {
        self.step_as(world, tick, out, SLOPES_AND_SLIDES)
    }

    /// `step` with the slides on or off (the tests keep ADR 0101's rule running).
    pub(crate) fn step_as(&mut self, world: &mut World, tick: u64, out: &mut Vec<(Block, u16)>, slides: bool) {
        let mut now = std::mem::take(&mut self.awake);
        let mut visits = 0;
        while let Some((k, (t, fell))) = now.pop_first() {
            if visits == BUDGET {
                // the rest wait; one in flight keeps its count over a fresh wake of the same cell
                now.insert(k, (t, fell));
                for (k, v) in now {
                    let e = self.awake.entry(k).or_insert(v);
                    if v.0 > e.0 {
                        *e = v;
                    }
                }
                break;
            }
            visits += 1;
            let p = cell(k);
            let m = world.block(p);
            if !BLOCKS.get(m).is_some_and(|d| d.falls) {
                continue;
            }
            let open = |w: &World, b: Block| !BLOCKS.solid(w.block(b));
            let below = near(world, p, 0, 0, -1);
            let to = if !BLOCKS.solid(world.block(below)) {
                if due(t + 1) <= fell {
                    // in flight, not a cell yet: it waits awake
                    self.awake.insert(k, (t + 1, fell));
                    continue;
                }
                // the block on it comes down with it, at the same count, later this tick (it is higher)
                let over = near(world, p, 0, 0, 1);
                if BLOCKS.get(world.block(over)).is_some_and(|d| d.falls) {
                    now.insert(key(over), (t, fell));
                }
                Some((below, (t + 1, fell + 1)))
            } else if let Some(&s) = self.awake.get(&key(below)).filter(|s| s.0 > 0) {
                // riding on a block in flight that waited this tick (it was visited first): it waits with it
                self.awake.insert(k, s);
                continue;
            } else if !slides || !open(world, near(world, p, 0, 0, 1)) {
                // calmer (ADR 0101): only a pile's top slides; a block with anything on it holds
                continue;
            } else {
                // a hash of the cell and the tick picks the first side: not always the same way
                let r = (crate::world::fnv(crate::world::FNV0, &[&k.2.to_le_bytes()[..], &k.3.to_le_bytes(), &k.4.to_le_bytes(), &tick.to_le_bytes()].concat()) % 4) as usize;
                (0..4).map(|s| SIDES[(r + s) % 4]).map(|(di, dj)| near(world, p, di, dj, 0)).find(|s| open(world, *s) && open(world, near(world, *s, 0, 0, -1))).map(|s| (s, (0, 0)))
            };
            let Some((q, state)) = to else { continue };
            world.set_block(p, AIR);
            world.set_block(q, m);
            out.push((p, AIR));
            out.push((q, m));
            self.moves += 1;
            self.wake(world, p);
            self.awake.insert(key(q), state);
        }
        self.visits += visits as u64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::World;
    use glam::DVec3;

    /// A world with a stone floor 7 x 7 a dozen layers over the ground, in the air: a test's own flat place.
    fn floor() -> (World, Block) {
        let mut w = World::new(512, 32, 5);
        let g = w.air_over_ground(0, DVec3::new(0.3, 0.5, 0.8).normalize());
        let base = Block { pos: g.pos + IVec3::Z * 12, ..g };
        let stone = BLOCKS.id("stone");
        for di in -3..=3 {
            for dj in -3..=3 {
                w.set_block(Block { pos: base.pos + IVec3::new(di, dj, 0), ..base }, stone);
            }
        }
        (w, Block { pos: base.pos + IVec3::Z, ..base })
    }

    fn at(b: Block, di: i32, dj: i32, dl: i32) -> Block {
        Block { pos: b.pos + IVec3::new(di, dj, dl), ..b }
    }

    /// Sand in the world less the generated sand: what the edits made (a fall moves sand, never makes it).
    fn sand_made(w: &World) -> i64 {
        let sand = BLOCKS.id("sand");
        w.edits.iter().flat_map(|(k, c)| c.iter().map(move |(p, m)| (Block { body: k.body, face: k.face, pos: *p }, *m))).map(|(b, m)| (m == sand) as i64 - (w.generated_block(b) == sand) as i64).sum()
    }

    fn settle(f: &mut Falls, w: &mut World, out: &mut Vec<(Block, u16)>) -> u64 {
        settle_as(f, w, out, SLOPES_AND_SLIDES)
    }

    /// ADR 0101's slide tests run with the slides on, whatever the switch says.
    fn settle_as(f: &mut Falls, w: &mut World, out: &mut Vec<(Block, u16)>, slides: bool) -> u64 {
        let mut tick = 0;
        while f.awake() > 0 {
            f.step_as(w, tick, out, slides);
            tick += 1;
            assert!(tick < 2000, "never settled");
        }
        tick
    }

    #[test]
    fn sand_falls_to_the_floor_and_lands_as_one_block() {
        let (mut w, top) = floor();
        let sand = BLOCKS.id("sand");
        let up = at(top, 0, 0, 6);
        w.set_block(up, sand);
        let mut f = Falls::default();
        f.wake(&w, up);
        let mut out = Vec::new();
        let ticks = settle(&mut f, &mut w, &mut out);
        assert_eq!(w.block(top), sand, "it lies on the floor");
        assert_eq!(w.block(up), AIR);
        assert_eq!(f.moves, 6, "a cell a move");
        // 6 m from rest is 1.1 s: 22 ticks, rounded as `due` does; a block never takes two cells a tick
        assert!((18..=26).contains(&ticks), "{ticks} ticks");
        assert_eq!(out.len(), 12, "two edits a move");
    }

    #[test]
    fn a_column_comes_down_together() {
        let (mut w, top) = floor();
        let sand = BLOCKS.id("sand");
        let stone = BLOCKS.id("stone");
        // four sands on a stone pillar; take the pillar's top out from under them
        for l in 0..3 {
            w.set_block(at(top, 0, 0, l), stone);
        }
        // in a shaft, so nothing slides off the stack when it lands
        for l in 0..8 {
            for (si, sj) in SIDES {
                w.set_block(at(top, si, sj, l), stone);
            }
        }
        for l in 3..7 {
            w.set_block(at(top, 0, 0, l), sand);
        }
        let mut f = Falls::default();
        let mut out = Vec::new();
        for l in [2, 1, 0] {
            w.set_block(at(top, 0, 0, l), AIR);
            f.wake(&w, at(top, 0, 0, l));
        }
        let mut tick = 0;
        while f.awake() > 0 {
            f.step(&mut w, tick, &mut out);
            tick += 1;
            // after every tick the four sands are one unbroken stack
            let ls: Vec<i32> = (0..8).filter(|l| w.block(at(top, 0, 0, *l)) == sand).collect();
            assert_eq!(ls.len(), 4, "tick {tick}: {ls:?}");
            assert_eq!(ls[3] - ls[0], 3, "tick {tick}: the stack broke apart: {ls:?}");
        }
        assert_eq!((0..4).map(|l| w.block(at(top, 0, 0, l))).collect::<Vec<_>>(), vec![sand; 4]);
    }

    /// A heap placed in the air falls as one piece: nothing slides off the blocks under it while they wait
    /// to start (a fresh block on one in flight waits with it).
    #[test]
    fn a_heap_in_the_air_comes_down_whole() {
        let (mut w, top) = floor();
        let sand = BLOCKS.id("sand");
        let mut f = Falls::default();
        for l in 8..11 {
            for di in -1..=1 {
                for dj in -1..=1 {
                    w.set_block(at(top, di, dj, l), sand);
                    f.wake(&w, at(top, di, dj, l));
                }
            }
        }
        let mut out = Vec::new();
        for tick in 0..400 {
            f.step(&mut w, tick, &mut out);
            if w.block(at(top, 0, 0, 0)) == sand {
                break; // landed: from here it may spread
            }
            let outside = (-3..=3).flat_map(|di| (-3..=3).map(move |dj| (di, dj))).filter(|&(di, dj): &(i32, i32)| di.abs() > 1 || dj.abs() > 1).any(|(di, dj)| (0..12).any(|l| w.block(at(top, di, dj, l)) == sand));
            assert!(!outside, "tick {tick}: a block slid off the falling heap");
        }
    }

    #[test]
    fn a_heap_slides_to_its_angle_of_rest() {
        let (mut w, top) = floor();
        let sand = BLOCKS.id("sand");
        let mut f = Falls::default();
        let mut out = Vec::new();
        // a tower of five on the middle of the floor
        for l in 0..5 {
            w.set_block(at(top, 0, 0, l), sand);
            f.wake(&w, at(top, 0, 0, l));
        }
        settle_as(&mut f, &mut w, &mut out, true);
        let h = |di: i32, dj: i32| (0..6).filter(|l| w.block(at(top, di, dj, *l)) == sand).count() as i32;
        let n: i32 = (-3..=3).flat_map(|di| (-3..=3).map(move |dj| (di, dj))).map(|(di, dj)| h(di, dj)).sum();
        assert_eq!(n, 5, "no block made or lost");
        assert_eq!(sand_made(&w), 5);
        for di in -2..=2 {
            for dj in -2..=2 {
                for (si, sj) in SIDES {
                    assert!(h(di, dj) - h(di + si, dj + sj) <= 1, "a step of two at {di},{dj}: {} by {}", h(di, dj), h(di + si, dj + sj));
                }
            }
        }
        assert!(h(0, 0) < 5, "the tower spread");
    }

    /// A corridor one block wide along +i from `top`, `len` long, walled with stone up to 8 layers: sand in it can
    /// only go one way, so a slump's end is known.
    fn corridor(w: &mut World, top: Block, len: i32) {
        let stone = BLOCKS.id("stone");
        for l in 0..8 {
            for i in -1..=len {
                w.set_block(at(top, i, 1, l), stone);
                w.set_block(at(top, i, -1, l), stone);
            }
            w.set_block(at(top, -1, 0, l), stone);
            w.set_block(at(top, len, 0, l), stone);
        }
    }

    /// The developer's rule (ADR 0101): a step of one stays put; the top of a pile two over its neighbour slides.
    #[test]
    fn a_step_of_one_stays_and_a_step_of_two_slides() {
        let sand = BLOCKS.id("sand");
        for (h, moves) in [(1, 0), (2, 2)] {
            let (mut w, top) = floor();
            corridor(&mut w, top, 3);
            let mut f = Falls::default();
            for l in 0..h {
                w.set_block(at(top, 0, 0, l), sand);
                f.wake(&w, at(top, 0, 0, l));
            }
            settle_as(&mut f, &mut w, &mut Vec::new(), true);
            // a step of two: the top slides one over (a move) and falls a cell (another)
            assert_eq!(f.moves, moves, "a step of {h}");
            let hs: Vec<usize> = (0..3).map(|i| (0..4).filter(|l| w.block(at(top, i, 0, *l)) == sand).count()).collect();
            assert_eq!(hs, if h == 1 { vec![1, 0, 0] } else { vec![1, 1, 0] }, "a step of {h}");
        }
    }

    /// Calmer (ADR 0101): only a pile's top block slides; one under another block never slides out from under it,
    /// so no hole opens in a pile and a block with anything on it stays.
    #[test]
    fn only_a_piles_top_slides() {
        let (sand, stone) = (BLOCKS.id("sand"), BLOCKS.id("stone"));
        let (mut w, top) = floor();
        corridor(&mut w, top, 4);
        let mut f = Falls::default();
        for l in 0..4 {
            w.set_block(at(top, 0, 0, l), sand);
            f.wake(&w, at(top, 0, 0, l));
        }
        let mut out = Vec::new();
        let mut tick = 0;
        while f.awake() > 0 {
            f.step_as(&mut w, tick, &mut out, true);
            tick += 1;
            // the first column's sand is one unbroken stack on the floor after every tick
            let ls: Vec<i32> = (0..8).filter(|l| w.block(at(top, 0, 0, *l)) == sand).collect();
            assert_eq!(ls, (0..ls.len() as i32).collect::<Vec<_>>(), "tick {tick}: a hole opened under the pile");
        }
        // a stone laid on a tower: the sand under it holds
        let (mut w, top) = floor();
        corridor(&mut w, top, 4);
        let mut f = Falls::default();
        for l in 0..3 {
            w.set_block(at(top, 0, 0, l), sand);
            f.wake(&w, at(top, 0, 0, l));
        }
        w.set_block(at(top, 0, 0, 3), stone);
        settle_as(&mut f, &mut w, &mut Vec::new(), true);
        assert_eq!(f.moves, 0, "sand under a stone slid out");
        // the stone taken: the sand under it is the top now, woken, and slides
        w.set_block(at(top, 0, 0, 3), AIR);
        f.wake(&w, at(top, 0, 0, 3));
        settle_as(&mut f, &mut w, &mut Vec::new(), true);
        assert!(f.moves > 0, "the new top never woke");
    }

    /// A known slump: a tower of four at the end of a corridor peels from the top a block a tick, each new top
    /// sliding before the one ahead of it has landed, and ends flat along the corridor, every block kept.
    #[test]
    fn a_tower_in_a_corridor_slumps_to_steps_of_one() {
        let sand = BLOCKS.id("sand");
        let (mut w, top) = floor();
        corridor(&mut w, top, 4);
        let mut f = Falls::default();
        for l in 0..4 {
            w.set_block(at(top, 0, 0, l), sand);
            f.wake(&w, at(top, 0, 0, l));
        }
        settle_as(&mut f, &mut w, &mut Vec::new(), true);
        let hs: Vec<usize> = (0..4).map(|i| (0..8).filter(|l| w.block(at(top, i, 0, *l)) == sand).count()).collect();
        assert_eq!(hs, vec![1, 1, 1, 1]);
    }

    /// ADR 0106: with the slides off, a sand column over a dug hole falls straight down and the sand around it
    /// stays where it is: no move is sideways.
    #[test]
    fn a_column_over_a_dug_hole_falls_straight_and_its_neighbours_stay() {
        let (mut w, top) = floor();
        let sand = BLOCKS.id("sand");
        for l in 0..3 {
            for di in -1..=1 {
                for dj in -1..=1 {
                    w.set_block(at(top, di, dj, l), sand);
                }
            }
        }
        let hole = at(top, 0, 0, -1);
        w.set_block(hole, AIR);
        let mut f = Falls::default();
        f.wake(&w, hole);
        let mut out = Vec::new();
        settle(&mut f, &mut w, &mut out);
        assert!(f.moves > 0, "the column fell");
        assert!(out.chunks(2).all(|m| m[0].0.pos.truncate() == m[1].0.pos.truncate()), "a move went sideways");
        for (di, dj) in (-1..=1).flat_map(|i| (-1..=1).map(move |j| (i, j))).filter(|&p| p != (0, 0)) {
            assert!((0..3).all(|l| w.block(at(top, di, dj, l)) == sand), "the neighbour {di},{dj} moved");
        }
        assert!((0..3).all(|l| w.block(at(top, 0, 0, l)) == AIR), "the column is gone from over the hole");
        assert_eq!(sand_made(&w), 27, "no block made or lost");
    }

    #[test]
    fn a_tick_visits_at_most_the_budget_and_the_rest_carry_on() {
        let (mut w, top) = floor();
        let sand = BLOCKS.id("sand");
        let mut f = Falls::default();
        for di in -3..=3 {
            for dj in -3..=3 {
                for l in 4..60 {
                    let b = at(top, di, dj, l);
                    w.set_block(b, sand);
                    f.wake(&w, b);
                }
            }
        }
        let mut out = Vec::new();
        f.step(&mut w, 0, &mut out);
        assert_eq!(f.visits, BUDGET as u64);
        assert!(f.awake() > 0);
        settle(&mut f, &mut w, &mut out);
        assert_eq!(sand_made(&w), 49 * 56, "every block is somewhere");
    }

    #[test]
    fn two_runs_of_one_collapse_end_in_the_same_world() {
        let run = || {
            let (mut w, top) = floor();
            let sand = BLOCKS.id("gravel");
            let mut f = Falls::default();
            for l in 0..8 {
                for di in -1..=1 {
                    w.set_block(at(top, di, 0, l), sand);
                    f.wake(&w, at(top, di, 0, l));
                }
            }
            let mut out = Vec::new();
            settle(&mut f, &mut w, &mut out);
            (w.hash(), out.len())
        };
        assert_eq!(run(), run());
    }

    /// The benchmark (ADR 0079): a heap of 2,744 sand blocks dropped and spread, ns a visit; printed for
    /// the ADR (`cargo test -p engine --release fall_cost -- --nocapture`).
    #[test]
    fn fall_cost() {
        let (mut w, top) = floor();
        let sand = BLOCKS.id("sand");
        let mut f = Falls::default();
        for di in -3..=3 {
            for dj in -3..=3 {
                for l in 4..60 {
                    w.set_block(at(top, di, dj, l), sand);
                    f.wake(&w, at(top, di, dj, l));
                }
            }
        }
        let mut out = Vec::new();
        let t = std::time::Instant::now();
        let mut worst = 0f64;
        let mut tick = 0;
        while f.awake() > 0 && tick < 5000 {
            let s = std::time::Instant::now();
            f.step(&mut w, tick, &mut out);
            worst = worst.max(s.elapsed().as_secs_f64());
            tick += 1;
        }
        let s = t.elapsed().as_secs_f64();
        println!("{{\"fall_cost\":{{\"ticks\":{tick},\"moves\":{},\"visits\":{},\"ns_per_visit\":{:.0},\"worst_tick_ms\":{:.3}}}}}", f.moves, f.visits, s * 1e9 / f.visits as f64, worst * 1e3);
        assert!(f.awake() == 0);
    }

    /// Spike 2 of mining.md (ADR 0093): a mine of 5,766 granite rubble blocks resting on a 31 x 31 floor, then a
    /// 9 x 9 hole dug under its middle. Prints the tick's cost with all of it resting, the drop's ns a visit and
    /// worst tick, and the chunks a tick of the drop changes (what a client meshes again)
    /// (`cargo test -p engine --profile play rubble_mine_cost -- --nocapture`).
    #[test]
    fn rubble_mine_cost() {
        let mut w = World::new(512, 32, 5);
        let g = w.air_over_ground(0, DVec3::new(0.3, 0.5, 0.8).normalize());
        let base = Block { pos: g.pos + IVec3::Z * 12, ..g };
        let (stone, rubble) = (BLOCKS.id("stone"), BLOCKS.get(BLOCKS.id("granite")).unwrap().rubble.unwrap());
        let mut f = Falls::default();
        let mut out = Vec::new();
        for di in -15..=15 {
            for dj in -15..=15 {
                w.set_block(at(base, di, dj, 0), stone);
                for l in 1..=6 {
                    w.set_block(at(base, di, dj, l), rubble);
                    f.poke(at(base, di, dj, l));
                }
            }
        }
        settle(&mut f, &mut w, &mut out);
        let resting = w.edits.values().flat_map(|c| c.values()).filter(|&&m| m == rubble).count();
        let t = std::time::Instant::now();
        for tick in 0..10_000 {
            f.step(&mut w, tick, &mut out);
        }
        let rest_ns = t.elapsed().as_secs_f64() * 1e9 / 10_000.0;
        assert!(out.is_empty() || f.awake() == 0);
        let (v0, m0) = (f.visits, f.moves);
        for di in -4..=4 {
            for dj in -4..=4 {
                w.set_block(at(base, di, dj, 0), AIR);
                f.wake(&w, at(base, di, dj, 0));
            }
        }
        let (mut worst, mut chunks_max, mut tick, mut busy) = (0f64, 0usize, 0u64, 0f64);
        while f.awake() > 0 {
            out.clear();
            let s = std::time::Instant::now();
            f.step(&mut w, tick, &mut out);
            let e = s.elapsed().as_secs_f64();
            (worst, busy) = (worst.max(e), busy + e);
            chunks_max = chunks_max.max(out.iter().map(|(b, _)| (b.face, b.pos.div_euclid(IVec3::splat(crate::world::CS)).to_array())).collect::<std::collections::BTreeSet<_>>().len());
            tick += 1;
        }
        let (visits, moves) = (f.visits - v0, f.moves - m0);
        println!("{{\"rubble_mine\":{{\"resting\":{resting},\"rest_tick_ns\":{rest_ns:.0},\"drop_ticks\":{tick},\"moves\":{moves},\"visits\":{visits},\"ns_per_visit\":{:.0},\"worst_tick_ms\":{:.3},\"chunks_a_tick_max\":{chunks_max}}}}}", busy * 1e9 / visits as f64, worst * 1e3);
        assert!(moves > 0 && f.awake() == 0);
    }
}
