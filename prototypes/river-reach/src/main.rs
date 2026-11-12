//! River reach spike (worldgen-plan W5, docs/design/rivers-and-tides.md "Rivers"): a reach at rest is generated
//! water, a touched reach becomes live water whose open ends book against the reach's water, and the patch is
//! forgotten when it matches generation again. `liquid.rs` is the engine's, with the changes this spike tries.
mod liquid;

use liquid::{Col, Ground, Liquids, DEAD, OPEN, SEA, U};
use std::collections::HashMap;

/// A straight channel along +x: banks at y < 2 and y >= 2 + W, a bed falling `slope` blocks per column, and
/// optionally a drop of `fall` blocks at x = XF (the reach boundary of a waterfall).
struct Channel {
    nx: i32,
    slope: f64,
    depth: f64,
    fall: f64,
    edits: HashMap<(i32, i32, i32), bool>,
}
const W: i32 = 6;
const NY: i32 = W + 4;
const XF: i32 = 300;
const B0: f64 = 40.0;
const RUNOFF: i64 = 4 * U;

impl Channel {
    fn bed(&self, x: i32) -> f64 {
        B0 - self.slope * x as f64 - if x >= XF { self.fall } else { 0.0 }
    }
    fn gen_floor(&self, x: i32, y: i32) -> i32 {
        if (2..2 + W).contains(&y) { self.bed(x).floor() as i32 } else { (B0 + 6.0) as i32 }
    }
    /// The generated level: bed + depth, smooth (a stepped level would stand up to 1/16 off the live water's
    /// smooth slope, and a nudge off a block's top would take from the fall the open end's feed reads).
    fn level(&self, x: i32) -> i64 {
        ((self.bed(x) + self.depth) * U as f64) as i64
    }
    fn solid(&self, x: i32, y: i32, z: i32) -> bool {
        self.edits.get(&(x, y, z)).copied().unwrap_or(z < self.gen_floor(x, y))
    }
}

impl Ground for Channel {
    fn gaps(&self, c: Col) -> Vec<(i32, i32)> {
        let (x, y) = (c.i(), c.j());
        let top = (B0 + 12.0) as i32;
        let mut out = Vec::new();
        let mut start = None;
        for z in 0..top {
            match (self.solid(x, y, z), start) {
                (false, None) => start = Some(z),
                (true, Some(s)) => {
                    out.push((s, z));
                    start = None;
                }
                _ => {}
            }
        }
        out.push((start.unwrap_or(top), OPEN));
        out
    }
    fn generated(&self, c: Col) -> (i64, i64) {
        let (x, y) = (c.i(), c.j());
        let lo = self.gen_floor(x, y) as i64 * U;
        let hi = self.level(x);
        if (2..2 + W).contains(&y) && lo < hi { (lo, hi) } else { (0, 0) }
    }
    fn neighbours(&self, c: Col) -> [Option<Col>; 4] {
        let (x, y) = (c.i(), c.j());
        [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)].map(|(a, b)| ((0..self.nx).contains(&a) && (0..NY).contains(&b)).then(|| Col::new(0, 0, a, b)))
    }
    fn reach(&self, c: Col) -> u32 {
        (c.i() >= XF) as u32
    }
    fn river(&self, c: Col) -> Option<(f64, f64, f64)> {
        let (lo, hi) = self.generated(c);
        // the flow grows from RUNOFF a second at x = 0 to twice that at the far end: each column gathers its share
        let q0 = RUNOFF as f64 / 60.0;
        let q = q0 * (1.0 + c.i() as f64 / self.nx as f64);
        (lo < hi).then(|| (q / W as f64, self.slope * U as f64, q0 / self.nx as f64 / W as f64))
    }
}

/// The reach graph: one number per reach, once a second upstream first.
struct Reach {
    water: i64,
    /// its water at rest, and the local runoff a second that keeps it there
    rest: i64,
    runoff: i64,
    /// seconds of its water it lets out a second (a linear store: out = water / t)
    t: i64,
    down: Option<usize>,
    /// metres: what the generator and a wheel read
    width: f64,
    depth: f64,
}
struct Graph {
    reaches: Vec<Reach>,
    rained: i64,
    to_sea: i64,
    /// the flow each reach let out last second, units
    out: Vec<i64>,
}
impl Graph {
    fn new(rest: i64, runoff: i64, depth: f64) -> Graph {
        // reach 0 flows into reach 1, reach 1 into the sea; each holds `rest` at the flow its runoff gives
        let r = |down, inflow: i64| Reach { water: rest, rest, runoff, t: rest / inflow, down, width: W as f64, depth };
        let reaches = vec![r(Some(1), runoff), r(None, 2 * runoff)];
        Graph { rained: rest * 2, to_sea: 0, out: vec![0; 2], reaches }
    }
    /// One second: runoff in, what the live patches gave or took, then out to the reach below.
    fn tick(&mut self, w: &mut Liquids) {
        let mut inflow = vec![0i64; self.reaches.len()];
        for k in 0..self.reaches.len() {
            let r = &mut self.reaches[k];
            r.water += r.runoff + inflow[k] + w.reach_extra.remove(&(k as u32)).unwrap_or(0);
            self.rained += r.runoff;
            let out = (r.water / r.t).max(0);
            r.water -= out;
            self.out[k] = out;
            match r.down {
                Some(d) => inflow[d] += out,
                None => self.to_sea += out,
            }
        }
    }
    /// What a wheel reads: metres a second, from the reach's flow over its cross-section. Touches no water.
    fn speed(&self, k: usize) -> f64 {
        let r = &self.reaches[k];
        let d = r.depth * r.water as f64 / r.rest as f64;
        self.out[k] as f64 / U as f64 / (r.width * d.max(0.05))
    }
    /// Every unit, wherever it is, less what rain brought and the sea took: constant.
    fn ledger(&self, w: &Liquids) -> i64 {
        self.reaches.iter().map(|r| r.water).sum::<i64>() + w.counted() - self.rained + self.to_sea
    }
}

fn col(x: i32, y: i32) -> Col {
    Col::new(0, 0, x, y)
}

/// The largest |live - generated| over the stored wet columns of the channel, in blocks.
fn deviation(g: &Channel, w: &Liquids) -> f64 {
    let mut worst = 0i64;
    for x in 0..g.nx {
        for y in 2..2 + W {
            if let Some(mut it) = w.column(col(x, y)) {
                let n = it.next().unwrap();
                let (lo, hi) = g.generated(col(x, y));
                if lo < hi {
                    worst = worst.max((n.surface() - hi).abs());
                }
            }
        }
    }
    worst as f64 / U as f64
}

fn run(w: &mut Liquids, g: &Channel, gr: &mut Graph, steps: usize, t: &mut usize, ledger0: i64, mut each: impl FnMut(&Liquids, &Graph, usize)) {
    for _ in 0..steps {
        w.step(g);
        *t += 1;
        if *t % 60 == 0 {
            gr.tick(w);
        }
        assert_eq!(gr.ledger(w), ledger0, "the ledger moved at step {t}");
        each(w, gr, *t);
    }
}

/// Q1 + Q3: a bucket into a reach at rest, for several slopes: how long the patch lives, how big it gets, and
/// how far its live water stands from generation before it is forgotten (forgotten within DEAD by rule).
fn bucket(slope: f64, depth: f64, fall: f64) {
    let g = Channel { nx: 400, slope, depth, fall, edits: HashMap::new() };
    let mut w = Liquids::default();
    let mut gr = Graph::new(400 * U, RUNOFF, depth);
    let l0 = gr.ledger(&w);
    // a fall: buckets on both sides of the lip, so the patch spans it
    let at: Vec<i32> = if fall > 0.0 { vec![XF - 1, XF, XF + 1] } else { vec![200] };
    for &x0 in &at {
        for y in 3..5 {
            w.pour(&g, col(x0, y), (g.level(x0) / U) as i32, U);
            gr.rained += U; // the bucket is carried in
        }
    }
    let (mut t, mut peak, mut gone, mut dev_max) = (0, 0usize, None, 0f64);
    let mut stepped = 0;
    run(&mut w, &g, &mut gr, 60 * 300, &mut t, l0, |w, gr, t| {
        peak = peak.max(w.stored());
        if t > 120 {
            dev_max = dev_max.max(deviation(&g, w));
        }
        if gone.is_none() && w.stored() == 0 {
            gone = Some(t);
            stepped = w.stepped;
        }
    });
    println!(
        "bucket slope {slope:.3} depth {depth} fall {fall}: forgotten after {}, peak {peak} columns stored, {} node-steps, live off generation by up to {dev_max:.4} blocks after 2 s, at the end {} stored {:.4} off",
        gone.map_or("never (300 s)".into(), |t| format!("{:.1} s", t as f64 / 60.0)),
        if gone.is_some() { stepped } else { w.stepped },
        w.stored(),
        deviation(&g, &w)
    );
    if w.stored() > 0 { left(&g, &w); }
}

/// Q2 + Q3: a dam built across the reach, then broken; the ledger holds every step, and how long the wave is live.
fn dam(slope: f64) {
    let depth = 1.5;
    let mut g = Channel { nx: 400, slope, depth, fall: 0.0, edits: HashMap::new() };
    let mut w = Liquids::default();
    let mut gr = Graph::new(400 * U, RUNOFF, depth);
    let l0 = gr.ledger(&w);
    let xd = 250;
    let top = (g.level(xd) / U) as i32 + 2;
    let cols: Vec<(i32, i32)> = (2..2 + W).map(|y| (xd, y)).collect();
    for &(x, y) in &cols {
        for z in g.gen_floor(x, y)..top {
            g.edits.insert((x, y, z), true);
        }
        w.regap(&g, col(x, y));
    }
    let mut t = 0;
    let mut peak = 0;
    run(&mut w, &g, &mut gr, 60 * 120, &mut t, l0, |w, _, _| peak = peak.max(w.stored()));
    let (up, down) = (w.column(col(xd - 1, 4)).map(|mut i| i.next().unwrap().surface()), w.column(col(xd + 1, 4)).map(|mut i| i.next().unwrap().surface()));
    let pond = up.unwrap_or(g.level(xd - 1)) as f64 / U as f64 - g.level(xd - 1) as f64 / U as f64;
    let below = down.unwrap_or(g.level(xd + 1)) as f64 / U as f64 - g.level(xd + 1) as f64 / U as f64;
    println!(
        "dam slope {slope}: after 120 s {} columns stored (peak {peak}), {} awake; pond {pond:+.2} blocks over generation, below the dam {below:+.2}; reach 1 got {:.1} blocks/s (rest {:.1})",
        w.stored(),
        w.awake(),
        gr.out[0] as f64 / U as f64,
        gr.reaches[0].runoff as f64 / U as f64
    );
    profile(&g, &w, 10);
    for &(x, y) in &cols {
        for z in g.gen_floor(x, y)..top {
            g.edits.remove(&(x, y, z));
        }
        w.regap(&g, col(x, y));
    }
    let t0 = t;
    let (mut gone, mut peak, mut flood) = (None, 0, 0f64);
    let s0 = w.stepped;
    run(&mut w, &g, &mut gr, 60 * 600, &mut t, l0, |w, gr, t| {
        peak = peak.max(w.stored());
        flood = flood.max(gr.out[0] as f64 / U as f64);
        if gone.is_none() && w.stored() == 0 {
            gone = Some(t);
        }
        if std::env::args().count() > 2 && (t - t0) % 1800 == 0 {
            println!("   {} s: {} stored {} awake, fed {:.1} held {:.1} blocks", (t - t0) / 60, w.stored(), w.awake(), w.fed as f64 / U as f64, w.held as f64 / U as f64);
            profile(&g, w, 10);
            for y in 2..2 + W {
                if let Some(x) = (0..g.nx).find(|&x| w.column(col(x, y)).is_some()) {
                    println!("     row {y}: first stored x {x} gen {} {} | {}", g.generated(col(x, y)).0, g.generated(col(x, y)).1, w.debug_col(col(x, y)));
                }
            }
        }
    });
    if std::env::args().count() > 1 {
        for _ in 0..0 {
            let f0 = w.fed;
            w.step(&g);
            println!("    step: fed {} | 124 {} | 125 {} | 126 {}", w.fed - f0, w.debug_col(col(124, 4)), w.debug_col(col(125, 4)), w.debug_col(col(126, 4)));
        }
    }
    profile(&g, &w, 10);
    println!(
        "  broken: live for {}, peak {peak} columns, {} node-steps; the reach below received up to {flood:.1} blocks/s; ledger held every step",
        gone.map_or(format!("over 600 s ({} stored, {:.4} off)", w.stored(), deviation(&g, &w)), |e| format!("{:.1} s", (e - t0) as f64 / 60.0)),
        w.stepped - s0
    );
}

/// Q4: a wheel reading the reach's speed wakes nothing.
fn wheel() {
    let g = Channel { nx: 400, slope: 0.02, depth: 1.5, fall: 0.0, edits: HashMap::new() };
    let mut w = Liquids::default();
    let mut gr = Graph::new(400 * U, RUNOFF, 1.5);
    gr.tick(&mut w);
    let t = std::time::Instant::now();
    let mut s = 0.0;
    for k in 0..1_000_000 {
        s += gr.speed(k & 1);
    }
    let ns = t.elapsed().as_secs_f64() * 1e9 / 1e6;
    for _ in 0..600 {
        w.step(&g);
    }
    println!("wheel: speed {:.2} m/s ({ns:.1} ns a read); after 10 s of reads and steps {} columns stored, {} node-steps", s / 1e6, w.stored(), w.stepped);
    assert_eq!((w.stored(), w.stepped), (0, 0));
}

/// The reach graph's cost: a few thousand reaches a second.
fn graph_cost() {
    let n = 4000;
    let mut reaches: Vec<Reach> = (0..n).map(|k| Reach { water: 400 * U, rest: 400 * U, runoff: 4 * U, t: 100, down: if k + 1 < n { Some(k + 1) } else { None }, width: 4.0, depth: 1.0 }).collect();
    reaches.reverse(); // upstream first as the tick walks them: 0 drains into 1 ...
    for (k, r) in reaches.iter_mut().enumerate() {
        r.down = if k + 1 < n { Some(k + 1) } else { None };
    }
    let mut gr = Graph { out: vec![0; n], reaches, rained: 0, to_sea: 0 };
    let mut w = Liquids::default();
    let t = std::time::Instant::now();
    for _ in 0..1000 {
        gr.tick(&mut w);
    }
    println!("graph: {n} reaches, {:.1} us a tick", t.elapsed().as_secs_f64() * 1e6 / 1000.0);
}

fn left(g: &Channel, w: &Liquids) {
    for x in 0..g.nx {
        for y in 0..NY {
            if w.column(col(x, y)).is_some() {
                println!("    left: {x},{y} gen {:?} gaps {:?} {}", g.generated(col(x, y)), g.gaps(col(x, y)), w.debug_col(col(x, y)));
            }
        }
    }
}

/// A pillar standing in the river (a bridge's): the water must flow round it for good. What does it keep live?
fn pillar() {
    let depth = 1.5;
    let mut g = Channel { nx: 400, slope: 0.02, depth, fall: 0.0, edits: HashMap::new() };
    let mut w = Liquids::default();
    let mut gr = Graph::new(400 * U, RUNOFF, depth);
    let l0 = gr.ledger(&w);
    for z in g.gen_floor(200, 4)..(g.level(200) / U) as i32 + 2 {
        g.edits.insert((200, 4, z), true);
    }
    w.regap(&g, col(200, 4));
    let mut t = 0;
    run(&mut w, &g, &mut gr, 60 * 120, &mut t, l0, |_, _, _| {});
    let s0 = w.stepped;
    run(&mut w, &g, &mut gr, 60 * 60, &mut t, l0, |_, _, _| {});
    println!("pillar: after 3 min {} columns stored, {} awake, {:.0} node-steps a second, {:.4} blocks off generation at worst", w.stored(), w.awake(), (w.stepped - s0) as f64 / 60.0, deviation(&g, &w));
}

fn main() {
    let _ = (SEA, DEAD);
    if std::env::args().count() > 1 { dam(0.02); return; }
    pillar();
    wheel();
    graph_cost();
    for s in [0.005, 0.02, 0.05, 0.1] {
        bucket(s, 1.5, 0.0);
    }
    bucket(0.02, 1.0, 0.0);
    bucket(0.02, 1.5, 3.0);
    for s in [0.02] {
        dam(s);
    }
}

/// The live surface against generation along the channel's middle row, every `every` columns.
#[allow(dead_code)]
fn profile(g: &Channel, w: &Liquids, every: i32) {
    let mut s = String::new();
    for x in (0..g.nx).step_by(every as usize) {
        let gen = g.level(x) as f64 / U as f64;
        match w.column(col(x, 4)) {
            Some(mut it) => {
                let n = it.next().unwrap();
                s += &format!(" {x}:{:+.2}", n.surface() as f64 / U as f64 - gen);
            }
            None => s += &format!(" {x}:."),
        }
    }
    println!("   profile{s}");
}
