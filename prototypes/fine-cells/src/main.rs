//! Spike for docs/design/fine-cells.md. Two questions, measured on one thread:
//! 1. Whole loose blocks (sand, gravel) falling and sliding as blocks: what does one awake block cost a tick?
//! 2. Fine cells (8 x 8 x 8 per block, one byte each) falling and piling as a cellular automaton, kept per
//!    block as a 512-byte payload: what does one awake cell cost, and how long does a burst take to settle?
//! The world is a flat 128 x 128 x 64 grid of blocks (the game's columns are a cube-sphere; the cost of a
//! lookup there is a hash map's, which is what the edits here are too).
use std::collections::{BTreeMap, HashMap};
use std::time::Instant;

type P = (i32, i32, i32); // (layer, i, j): layer first, so a BTreeMap visits bottom up

const AIR: u8 = 0;
const STONE: u8 = 1;
const SAND: u8 = 2;

struct Blocks {
    edits: HashMap<P, u8>,
}
impl Blocks {
    fn generated(p: P) -> u8 {
        if p.0 < 10 {
            STONE
        } else if p.0 < 14 {
            SAND
        } else {
            AIR
        }
    }
    fn get(&self, p: P) -> u8 {
        *self.edits.get(&p).unwrap_or(&Self::generated(p))
    }
}

fn mix(mut x: u64) -> u64 {
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51afd7ed558ccd);
    x ^= x >> 33;
    x
}

const SIDES: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

/// Whole-block falls: the awake set, each with (ticks falling, cells fallen).
fn block_step(w: &mut Blocks, awake: &mut BTreeMap<P, (u16, u16)>, tick: u64, moves: &mut u64) {
    let now: Vec<(P, (u16, u16))> = std::mem::take(awake).into_iter().collect();
    for (p, (k, m)) in now {
        if w.get(p) != SAND {
            continue;
        }
        let below = (p.0 - 1, p.1, p.2);
        if w.get(below) == AIR {
            let due = (4.905 * ((k + 1) as f64 / 20.0).powi(2)) as u16;
            if due > m {
                w.edits.insert(p, AIR);
                w.edits.insert(below, SAND);
                *moves += 1;
                wake(awake, p);
                awake.insert(below, (k + 1, m + 1));
            } else {
                awake.insert(p, (k + 1, m));
            }
            continue;
        }
        let r = (mix(tick ^ ((p.1 as u64) << 20) ^ ((p.2 as u64) << 40)) & 3) as usize;
        for s in 0..4 {
            let (di, dj) = SIDES[(r + s) & 3];
            let side = (p.0, p.1 + di, p.2 + dj);
            if w.get(side) == AIR && w.get((p.0 - 1, side.1, side.2)) == AIR {
                w.edits.insert(p, AIR);
                w.edits.insert(side, SAND);
                *moves += 1;
                wake(awake, p);
                awake.insert(side, (0, 0));
                break;
            }
        }
    }
}

fn wake(awake: &mut BTreeMap<P, (u16, u16)>, p: P) {
    for dl in 0..=1 {
        for (di, dj) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
            awake.entry((p.0 + dl, p.1 + di, p.2 + dj)).or_insert((0, 0));
        }
    }
}

/// Fine cells: blocks that are fine keep 512 bytes; the rest are whole (in `Blocks`).
struct Fine {
    cells: HashMap<P, Box<[u8; 512]>>,
}
fn split(p: P) -> (P, usize) {
    let b = (p.0.div_euclid(8), p.1.div_euclid(8), p.2.div_euclid(8));
    let (l, i, j) = (p.0.rem_euclid(8), p.1.rem_euclid(8), p.2.rem_euclid(8));
    (b, (l * 64 + i * 8 + j) as usize)
}
impl Fine {
    fn get(&self, w: &Blocks, p: P) -> u8 {
        let (b, k) = split(p);
        match self.cells.get(&b) {
            Some(c) => c[k],
            None => w.get(b),
        }
    }
    fn set(&mut self, w: &Blocks, p: P, m: u8) {
        let (b, k) = split(p);
        let c = self.cells.entry(b).or_insert_with(|| Box::new([w.get(b); 512]));
        c[k] = m;
    }
}

fn fine_step(w: &Blocks, f: &mut Fine, awake: &mut Vec<P>, next: &mut Vec<P>, tick: u64) -> u64 {
    awake.sort_unstable();
    awake.dedup();
    let mut moves = 0;
    for &p in awake.iter() {
        let m = f.get(w, p);
        if m == AIR {
            continue;
        }
        let below = (p.0 - 1, p.1, p.2);
        let mut to = None;
        if f.get(w, below) == AIR {
            to = Some(below);
        } else {
            let r = (mix(tick ^ ((p.1 as u64) << 20) ^ ((p.2 as u64) << 40)) & 3) as usize;
            for s in 0..4 {
                let (di, dj) = SIDES[(r + s) & 3];
                let d = (p.0 - 1, p.1 + di, p.2 + dj);
                if f.get(w, d) == AIR && f.get(w, (p.0, d.1, d.2)) == AIR {
                    to = Some(d);
                    break;
                }
            }
        }
        if let Some(d) = to {
            f.set(w, p, AIR);
            f.set(w, d, m);
            moves += 1;
            next.push(d);
            for dl in 0..=1 {
                for (di, dj) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
                    next.push((p.0 + dl, p.1 + di, p.2 + dj));
                }
            }
        }
    }
    awake.clear();
    std::mem::swap(awake, next);
    moves
}

fn main() {
    // 1. whole blocks: dig out N columns' bottom stone under the 4 m of sand, let everything settle
    for &holes in &[1usize, 64, 1024] {
        let mut w = Blocks { edits: HashMap::new() };
        let mut awake = BTreeMap::new();
        for h in 0..holes {
            let (i, j) = ((h % 32) as i32 * 4, (h / 32) as i32 * 4);
            for l in 6..10 {
                w.edits.insert((l, i, j), AIR); // a shaft under the sand
            }
            wake(&mut awake, (9, i, j));
        }
        let (mut moves, mut visits, mut ticks) = (0u64, 0u64, 0u64);
        let t = Instant::now();
        let mut peak = 0;
        while !awake.is_empty() && ticks < 2000 {
            visits += awake.len() as u64;
            peak = peak.max(awake.len());
            block_step(&mut w, &mut awake, ticks, &mut moves);
            ticks += 1;
        }
        let s = t.elapsed().as_secs_f64();
        println!(
            "{{\"case\":\"blocks\",\"holes\":{holes},\"ticks\":{ticks},\"moves\":{moves},\"visits\":{visits},\"peak_awake\":{peak},\"ns_per_visit\":{:.1},\"worst_tick_ms_est\":{:.3}}}",
            s * 1e9 / visits as f64,
            s * 1e3 / ticks as f64 * (peak as f64 / (visits as f64 / ticks as f64))
        );
    }
    // 2. fine cells: N blocks of sand burst 2 m over a floor of stone, each into its 512 cells, settle
    for &bursts in &[1usize, 8, 64] {
        let w = Blocks { edits: HashMap::new() };
        let mut f = Fine { cells: HashMap::new() };
        let mut awake = Vec::new();
        for b in 0..bursts {
            let (bi, bj) = ((b % 8) as i32 * 3, (b / 8) as i32 * 3);
            for l in 0..8 {
                for i in 0..8 {
                    for j in 0..8 {
                        let p = ((16) * 8 + l, bi * 8 + i, bj * 8 + j);
                        f.set(&w, p, SAND);
                        awake.push(p);
                    }
                }
            }
        }
        let mut next = Vec::new();
        let (mut moves, mut visits, mut ticks, mut peak) = (0u64, 0u64, 0u64, 0usize);
        let t = Instant::now();
        while !awake.is_empty() && ticks < 5000 {
            visits += awake.len() as u64;
            peak = peak.max(awake.len());
            moves += fine_step(&w, &mut f, &mut awake, &mut next, ticks);
            ticks += 1;
        }
        let s = t.elapsed().as_secs_f64();
        // the settled pile: how many fine blocks hold sand, and how many are full (could be whole again)
        let held = f.cells.values().filter(|c| c.iter().any(|&m| m == SAND)).count();
        let full = f.cells.values().filter(|c| c.iter().all(|&m| m == SAND)).count();
        let sand: usize = f.cells.values().map(|c| c.iter().filter(|&&m| m == SAND).count()).sum();
        println!(
            "{{\"case\":\"fine\",\"bursts\":{bursts},\"ticks\":{ticks},\"moves\":{moves},\"visits\":{visits},\"peak_awake\":{peak},\"ns_per_visit\":{:.1},\"fine_blocks\":{held},\"full_again\":{full},\"cells_kept\":{sand},\"cells_in\":{}}}",
            s * 1e9 / visits as f64,
            bursts * 512
        );
        assert_eq!(sand, bursts * 512, "the ledger: no cell lost or made");
    }
}
