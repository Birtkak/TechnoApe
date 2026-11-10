//! Water (ADR 0020): shallow water on the columns' air runs, with momentum.
//!
//! A column of blocks is split by solid blocks into air runs, its *gaps*; each gap that matters is a node
//! holding water in fixed-point units (`U` per block). Water inside one gap always lies at its bottom, so a
//! node is one number however deep, and the cost of water is its area, not its volume. Neighbouring
//! columns' gaps that overlap are linked; every link carries a velocity (the virtual pipe model), which a
//! difference in water height speeds up and friction slows down. That gives waves, surges and real speeds.
//! Volume moves between nodes in whole units, so the total is exact: nothing appears or vanishes.
//!
//! Only awake nodes are visited. A node sleeps when it and its links have been still for a while; settled
//! water costs nothing, and untouched sea is not stored at all: it is generated from the seed like terrain.
use crate::world::FxMap;

/// Units of water in one full block.
pub const U: i64 = 1 << 12;
/// A gap open to the sky.
pub const OPEN: i32 = i32::MAX;
/// log2 of U.
const UB: u32 = 12;
/// Velocity is stored in 1/2^VF blocks per step.
const VF: u32 = 20;
/// g dt^2 / dx in 1/2^VF blocks per step per block of head, for dt = 1/60 s and 0.5 m blocks.
const CG: i64 = (9.81 / 3600.0 / 0.5 * (1u64 << VF) as f64) as i64;
/// Fastest flow: half a block per step (15 m/s).
const VMAX: i64 = 1 << (VF - 1);
/// The deepest water that still speeds a link up; deeper water flows as if it were this deep. It keeps a
/// step of 1/60 s stable in deep water (waves there run at sqrt(g 8 m) instead of sqrt(g depth)).
const ACAP: i64 = 16 * U;
/// Water shallower than this does not spread over level ground (surface tension; puddles stay puddles).
pub const MIN: i64 = U / 32;
/// Friction: a link loses 1/2^FRICTION of its speed a step, and up to 1/THIN more in water under a block deep.
const FRICTION: u32 = 9;
const THIN: i64 = 128;
/// A link slower than this (0.1 mm/s) stops, and a height difference too small to speed a link past it
/// (0.4 mm) moves nothing and keeps nothing awake. Small, because a slow fill leaves its slope behind: a
/// lake filled from one end ends tilted by up to this per column.
const VSTOP: i64 = 1 << 2;
/// Flow slower than this (6 cm/s) is calm (`settle`).
const VSLOW: i64 = 1 << 11;
/// Every SETTLE_EVERY steps, a body of water whose flow is all slower than VSLOW and whose level varies by
/// less than SETTLE_DH (1.5 cm) is laid flat (`settle`). Bodies over SETTLE_MAX nodes are left to friction.
const SETTLE_EVERY: u64 = 15;
const SETTLE_DH: i64 = U / 32;
/// Stored water next to open sea that is not stored, and this close to sea level, trades with the sea
/// instead of storing it: the edge of the stored region is open water, and a wave smaller than this (1.5 cm)
/// leaves through it.
const SEA_DH: i64 = U / 32;
const SETTLE_MAX: usize = 200_000;

/// Units a step that fall through one block of opening under `h` units of head: sqrt(2 g h) dt / dx.
fn fall_cap(h: i64) -> i64 {
    let m = h.max(U / 8) as f64 / U as f64 * 0.5;
    ((2.0 * 9.81 * m).sqrt() / 60.0 / 0.5 * U as f64) as i64
}

fn moves(dh: i64) -> bool {
    dh * CG >> 12 > VSTOP
}

/// Thin water runs downhill but spreads over level ground only while it is deeper than MIN, and onto dry
/// ground only while it could leave both sides that deep: a puddle's edge stops instead of crawling on.
fn spreads(a: &Node, b: &Node) -> bool {
    b.floor < a.floor || a.water >= if b.water == 0 { 2 * MIN } else { MIN }
}
/// Steps a node stays awake after it last moved.
const QUIET: u16 = 30;
/// A full gap under a ceiling holds more water under pressure: one block of head per 1/K block of excess.
/// Without it a closed tunnel could not pass on the pressure of the lake it leads from.
const K: i64 = 16;

/// A column: body, face and column (i, j), packed.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, serde::Serialize, serde::Deserialize)]
pub struct Col(pub u64);

impl Col {
    pub fn new(body: u8, face: u8, i: i32, j: i32) -> Col {
        Col((body as u64) << 56 | (face as u64) << 48 | (i as u64 & 0xFF_FFFF) << 24 | (j as u64 & 0xFF_FFFF))
    }
    pub fn body(self) -> u8 {
        (self.0 >> 56) as u8
    }
    pub fn face(self) -> u8 {
        (self.0 >> 48) as u8
    }
    pub fn i(self) -> i32 {
        (self.0 >> 24 & 0xFF_FFFF) as i32
    }
    pub fn j(self) -> i32 {
        (self.0 & 0xFF_FFFF) as i32
    }
    /// The tile of TILE x TILE columns that holds this one: what the server sends a client at a time.
    pub fn tile(self) -> Col {
        Col::new(self.body(), self.face(), self.i() / TILE, self.j() / TILE)
    }
}

/// Columns per tile edge (`Col::tile`).
pub const TILE: i32 = 16;

/// What the water needs to know about the solid world.
pub trait Ground {
    /// The air runs of column `c`, bottom-up, as (floor, ceil) layers: air from `floor` to below `ceil`. The
    /// last one is open (`ceil == OPEN`).
    fn gaps(&self, c: Col) -> Vec<(i32, i32)>;
    /// The water generation puts in column `c`: (bottom, top) in units, empty when bottom >= top.
    fn generated(&self, c: Col) -> (i64, i64);
    /// The four neighbouring columns as [-i, +i, -j, +j] in this column's own frame, so slot `k ^ 1` is
    /// straight on from slot `k` (none at the edge of a test box).
    fn neighbours(&self, c: Col) -> [Option<Col>; 4];
}

/// One gap of a column, with its water and its links to the neighbours' gaps.
#[derive(Clone, Debug)]
pub struct Node {
    pub col: Col,
    pub floor: i32,
    pub ceil: i32,
    pub water: i64,
    links: Vec<Link>,
    quiet: u16,
    /// Its neighbour columns are stored (once stored, a column stays stored).
    rim: bool,
    /// The step that last listed it (`step`'s set without a sort).
    seen: u32,
    /// The water the clients were last told about (the wire's dead band).
    sent: i64,
}

#[derive(Clone, Copy, Debug)]
struct Link {
    to: u32,
    /// The index of the link back, in `to`'s list.
    back: u16,
    /// Which neighbour slot the link leads to.
    dir: u8,
    /// The speed of the flow out through this link, in 1/2^VF blocks per step; never negative (the other
    /// side's link carries the flow the other way).
    v: i32,
    /// The units leaving through this link this step, and what did not make a whole unit yet.
    out: i32,
    frac: i32,
    /// What came in through this link last step, and how fast: water carries its speed on (`step`).
    fin: i32,
    vin: i32,
}

impl Node {
    fn cap(&self) -> i64 {
        if self.ceil == OPEN { i64::MAX } else { (self.ceil - self.floor) as i64 * U }
    }
    /// Where the water's top is, in units.
    pub fn surface(&self) -> i64 {
        self.floor as i64 * U + self.water.min(self.cap())
    }
    /// The height water flows from: the surface, or under a ceiling the pressure of what it holds.
    fn head(&self) -> i64 {
        let over = self.water - self.cap();
        if over > 0 { self.ceil as i64 * U + over * K } else { self.surface() }
    }
}

/// A column's water as the wire and saves carry it: (floor, ceil, water) per gap. Empty: back to generated.
pub type ColWater = (Col, Vec<(i32, i32, i64)>);

#[derive(Default)]
pub struct Liquids {
    nodes: Vec<Node>,
    free: Vec<u32>,
    /// The nodes of every stored column, bottom-up.
    cols: FxMap<Col, Vec<u32>>,
    /// The stored columns of each tile.
    tiles: FxMap<Col, Vec<Col>>,
    /// Nodes stepped next, sorted.
    awake: Vec<u32>,
    /// Columns the clients must hear about.
    dirty: FxMap<Col, ()>,
    /// What generation put into each stored column, and its sum.
    gen: FxMap<Col, i64>,
    gen_total: i64,
    /// Units the open sea took in from stored water (negative: gave out). Calm water at sea level next to
    /// sea that is not stored settles to exactly sea level, the difference goes here, and columns equal to
    /// generation are forgotten again. shortcut: the sea's level does not rise with it; a whole sea holds
    /// millions of blocks, so a bucket is far under a unit of its level, until the water cycle moves sea.
    pub sea_extra: i64,
    /// Work done, for the stats line and the tests.
    pub stepped: u64,
    steps: u64,
    epoch: u32,
}

fn window(a: &Node, b: &Node) -> (i64, i64) {
    let hi = a.ceil.min(b.ceil);
    (a.floor.max(b.floor) as i64 * U, if hi == OPEN { i64::MAX } else { hi as i64 * U })
}

impl Liquids {
    pub fn awake(&self) -> usize {
        self.awake.len()
    }
    pub fn stored(&self) -> usize {
        self.cols.len()
    }
    /// The gaps of column `c` with their water, if it is stored.
    pub fn column(&self, c: Col) -> Option<impl Iterator<Item = &Node>> {
        self.cols.get(&c).map(|ids| ids.iter().map(|&i| &self.nodes[i as usize]))
    }
    /// All water that differs from generation, sorted: for saves, hashes and a joining client.
    pub fn all(&self) -> Vec<ColWater> {
        let mut v: Vec<ColWater> = self.cols.keys().map(|&c| self.col_water(c)).collect();
        v.sort_unstable_by_key(|(c, _)| *c);
        v
    }
    fn col_water(&self, c: Col) -> ColWater {
        (c, self.cols[&c].iter().map(|&i| &self.nodes[i as usize]).map(|n| (n.floor, n.ceil, n.water)).collect())
    }
    /// The columns whose water changed by more than the dead band, or settled, since the last call; each is
    /// sent whole. Settled water is sent exactly, so a client ends equal to the server.
    pub fn take_changes(&mut self) -> Vec<ColWater> {
        const DEAD: i64 = U / 64;
        for &i in &self.awake {
            let n = &self.nodes[i as usize];
            if (n.water - n.sent).abs() >= DEAD {
                self.dirty.insert(n.col, ());
            }
        }
        let mut cols: Vec<Col> = self.dirty.drain().map(|(c, _)| c).collect();
        cols.sort_unstable();
        cols.into_iter()
            .map(|c| match self.cols.get(&c) {
                Some(ids) => {
                    for &i in ids {
                        let n = &mut self.nodes[i as usize];
                        n.sent = n.water;
                    }
                    self.col_water(c)
                }
                None => (c, Vec::new()),
            })
            .collect()
    }
    /// The total water stored, in units. Generation's share of it is `generated_total`.
    pub fn total(&self) -> i64 {
        self.cols.values().flatten().map(|&i| self.nodes[i as usize].water).sum()
    }
    /// What generation put into the stored columns.
    pub fn generated_total(&self) -> i64 {
        self.gen_total
    }
    /// The water that differs from the seed's: changed only by water carried in or out (a bucket, creative,
    /// later rain and steam), never by flowing (ADR 0020).
    pub fn counted(&self) -> i64 {
        self.total() - self.gen_total + self.sea_extra
    }

    /// A client's copy: column `c` becomes what the server sent (no links: a client does not simulate).
    pub fn set_column(&mut self, (c, gaps): &ColWater) {
        self.remove(*c, false);
        if gaps.is_empty() {
            return;
        }
        let ids = gaps.iter().map(|&(floor, ceil, water)| self.alloc(Node { col: *c, floor, ceil, water, links: Vec::new(), quiet: u16::MAX, rim: true, seen: 0, sent: water })).collect();
        self.cols.insert(*c, ids);
        self.tiles.entry(c.tile()).or_default().push(*c);
    }

    /// A tile's stored columns, sorted (empty: all of it is as generation gives it).
    pub fn tile(&self, t: Col) -> Vec<ColWater> {
        let mut v: Vec<ColWater> = self.tiles.get(&t).into_iter().flatten().map(|&c| self.col_water(c)).collect();
        v.sort_unstable_by_key(|w| w.0);
        v
    }
    /// Whether tile `t` holds stored water.
    pub fn wet(&self, t: Col) -> bool {
        self.tiles.contains_key(&t)
    }
    /// The wet tiles among the columns i0..i0+span, j0..j0+span of a face, sorted.
    pub fn wet_tiles(&self, body: u8, face: u8, i0: i32, j0: i32, span: i32) -> Vec<Col> {
        let mut v = Vec::new();
        if self.tiles.is_empty() {
            return v;
        }
        for ti in i0 / TILE..(i0 + span + TILE - 1) / TILE {
            for tj in j0 / TILE..(j0 + span + TILE - 1) / TILE {
                let t = Col::new(body, face, ti, tj);
                if self.tiles.contains_key(&t) {
                    v.push(t);
                }
            }
        }
        v.sort_unstable();
        v
    }
    /// Whether all water stored in tile `t` is asleep.
    pub fn tile_settled(&self, t: Col) -> bool {
        self.tiles.get(&t).into_iter().flatten().flat_map(|c| &self.cols[c]).all(|&i| self.nodes[i as usize].quiet == u16::MAX)
    }
    /// A client's copy: tile `t` becomes what the server sent.
    pub fn set_tile(&mut self, t: Col, cols: &[ColWater]) {
        for c in self.tiles.get(&t).cloned().unwrap_or_default() {
            self.remove(c, false);
        }
        for c in cols {
            self.set_column(c);
        }
    }

    /// Stores columns as a save left them: their water poured into the gaps the terrain has now, asleep.
    pub fn restore(&mut self, g: &impl Ground, cols: &[ColWater]) {
        for (c, gaps) in cols {
            let mut wet = Vec::new();
            for &(floor, ceil, water) in gaps {
                let f = floor as i64 * U;
                let cap = if ceil == OPEN { i64::MAX } else { (ceil - floor) as i64 * U };
                wet.push((f, f + water.min(cap), (water - cap).max(0)));
            }
            self.remove(*c, false);
            self.build(g, *c, &wet, false);
        }
    }

    fn alloc(&mut self, n: Node) -> u32 {
        match self.free.pop() {
            Some(i) => {
                self.nodes[i as usize] = n;
                i
            }
            None => {
                self.nodes.push(n);
                (self.nodes.len() - 1) as u32
            }
        }
    }

    /// Stores column `c` (with the water generation gives it) if it is not stored yet.
    pub fn materialize(&mut self, g: &impl Ground, c: Col) {
        if self.cols.contains_key(&c) {
            return;
        }
        let (lo, hi) = g.generated(c);
        let wet = if lo < hi { vec![(lo, hi, 0)] } else { Vec::new() };
        // asleep: generated water is level; a neighbour's flow wakes it
        self.build(g, c, &wet, false);
    }

    /// Builds column `c`'s nodes from its gaps and pours `wet` into them: intervals of water in units, each
    /// with the units it held under pressure, which stay with the gap that holds the interval's bottom. Water
    /// in what is solid now goes to the gap above it.
    fn build(&mut self, g: &impl Ground, c: Col, wet: &[(i64, i64, i64)], wake: bool) {
        if !self.gen.contains_key(&c) {
            let (lo, hi) = g.generated(c);
            self.gen.insert(c, (hi - lo).max(0));
            self.gen_total += (hi - lo).max(0);
        }
        let gaps = g.gaps(c);
        let mut water = vec![0i64; gaps.len()];
        for &(lo, hi, extra) in wet {
            let k = gaps.iter().rposition(|&(f, _)| f as i64 * U <= lo).unwrap_or(0);
            water[k] += extra;
            for (k, &(f, cl)) in gaps.iter().enumerate() {
                let (gl, gh) = (f as i64 * U, if cl == OPEN { i64::MAX } else { cl as i64 * U });
                water[k] += (hi.min(gh) - lo.max(gl)).max(0);
            }
            // the parts in solid layers (under each gap, down to the gap below): each rises into the gap above it
            let mut below = i64::MIN;
            for (k, &(f, cl)) in gaps.iter().enumerate() {
                let gl = f as i64 * U;
                water[k] += (hi.min(gl) - lo.max(below)).max(0);
                below = if cl == OPEN { i64::MAX } else { cl as i64 * U };
            }
        }
        let ids: Vec<u32> = gaps
            .iter()
            .zip(water)
            .map(|(&(floor, ceil), water)| self.alloc(Node { col: c, floor, ceil, water, links: Vec::new(), quiet: 0, rim: false, seen: 0, sent: water }))
            .collect();
        self.cols.insert(c, ids.clone());
        self.tiles.entry(c.tile()).or_default().push(c);
        for (sa, d) in g.neighbours(c).into_iter().enumerate() {
            let Some(d) = d else { continue };
            if let Some(nb) = self.cols.get(&d).cloned() {
                // the slot c has among d's neighbours (seams turn the frame)
                let sb = g.neighbours(d).iter().position(|&e| e == Some(c)).unwrap_or(sa ^ 1);
                for &a in &ids {
                    for &b in &nb {
                        let (lo, hi) = window(&self.nodes[a as usize], &self.nodes[b as usize]);
                        if lo < hi {
                            self.link(a, b, sa as u8, sb as u8);
                        }
                    }
                }
            }
        }
        for &a in &ids {
            if wake {
                self.wake(a);
            } else {
                self.nodes[a as usize].quiet = u16::MAX;
            }
        }
        self.dirty.insert(c, ());
    }

    fn link(&mut self, a: u32, b: u32, sa: u8, sb: u8) {
        let (la, lb) = (self.nodes[a as usize].links.len() as u16, self.nodes[b as usize].links.len() as u16);
        let l = |to, back, dir| Link { to, back, v: 0, out: 0, frac: 0, dir, fin: 0, vin: 0 };
        self.nodes[a as usize].links.push(l(b, lb, sa));
        self.nodes[b as usize].links.push(l(a, la, sb));
    }

    /// Steps node `a` from the next step on (`step` sorts out repeats).
    fn wake(&mut self, a: u32) {
        self.nodes[a as usize].quiet = 0;
        self.awake.push(a);
    }

    /// The terrain of column `c` changed: its gaps are found again and its water poured into them. A column
    /// not stored yet is stored when it or a neighbour holds water.
    pub fn regap(&mut self, g: &impl Ground, c: Col) {
        let Some(ids) = self.cols.get(&c).cloned() else {
            let (lo, hi) = g.generated(c);
            let wet = lo < hi || g.neighbours(c).iter().flatten().any(|d| match self.cols.get(d) {
                Some(ids) => ids.iter().any(|&i| self.nodes[i as usize].water > 0),
                None => {
                    let (lo, hi) = g.generated(*d);
                    lo < hi
                }
            });
            if wet {
                // generation's water poured into the gaps the edit left
                let wet = if lo < hi { vec![(lo, hi, 0)] } else { Vec::new() };
                self.build(g, c, &wet, true);
                for d in g.neighbours(c).into_iter().flatten() {
                    self.materialize(g, d);
                }
            }
            return;
        };
        let mut wet = Vec::new();
        for &i in &ids {
            let n = &self.nodes[i as usize];
            let f = n.floor as i64 * U;
            wet.push((f, f + n.water.min(n.cap()), (n.water - n.cap()).max(0)));
        }
        self.remove(c, true);
        self.build(g, c, &wet, true);
        for d in g.neighbours(c).into_iter().flatten() {
            self.materialize(g, d);
        }
    }

    /// Drops column `c`'s nodes and their links (the neighbours' links to them too), waking the neighbours
    /// if `wake`: not when `c` is forgotten as generation gives it, which changes nothing they would see.
    fn remove(&mut self, c: Col, wake: bool) {
        let Some(ids) = self.cols.remove(&c) else { return };
        if let Some(t) = self.tiles.get_mut(&c.tile()) {
            t.retain(|&d| d != c);
            if t.is_empty() {
                self.tiles.remove(&c.tile());
            }
        }
        for &a in &ids {
            let links = std::mem::take(&mut self.nodes[a as usize].links);
            for l in links {
                self.unlink(l.to, l.back as usize);
                self.nodes[l.to as usize].rim = false;
                if wake {
                    self.wake(l.to);
                }
            }
            self.nodes[a as usize].quiet = 0;
            self.free.push(a);
        }
        self.awake.retain(|a| !ids.contains(a));
    }

    /// Removes link `k` of node `a` (swap-remove, fixing the moved link's back index).
    fn unlink(&mut self, a: u32, k: usize) {
        let links = &mut self.nodes[a as usize].links;
        links.swap_remove(k);
        if k < links.len() {
            let moved = links[k];
            self.nodes[moved.to as usize].links[moved.back as usize].back = k as u16;
        }
    }

    /// Puts `units` into the gap of column `c` that holds layer `layer` (or the one above it): a bucket
    /// poured, creative water. Returns false when the column has no such gap.
    pub fn pour(&mut self, g: &impl Ground, c: Col, layer: i32, units: i64) -> bool {
        self.materialize(g, c);
        let ids = &self.cols[&c];
        let Some(&a) = ids.iter().find(|&&i| self.nodes[i as usize].ceil > layer) else { return false };
        self.nodes[a as usize].water += units;
        self.touch(g, a);
        true
    }

    /// Takes up to `units` from the gap of column `c` that holds layer `layer`; returns what it took.
    pub fn scoop(&mut self, g: &impl Ground, c: Col, layer: i32, units: i64) -> i64 {
        self.materialize(g, c);
        let ids = &self.cols[&c];
        let Some(&a) = ids.iter().find(|&&i| self.nodes[i as usize].ceil > layer) else { return 0 };
        let n = &mut self.nodes[a as usize];
        let took = units.min(n.water).max(0);
        n.water -= took;
        self.touch(g, a);
        took
    }

    fn touch(&mut self, g: &impl Ground, a: u32) {
        let c = self.nodes[a as usize].col;
        for d in g.neighbours(c).into_iter().flatten() {
            self.materialize(g, d);
        }
        self.wake(a);
        self.dirty.insert(c, ());
    }

    /// The water surface over layer `layer` of column `c`, in layers, if that layer is under water. Columns
    /// not stored are generated water.
    pub fn surface_at(&self, g: &impl Ground, c: Col, layer: f64) -> Option<f64> {
        let top = match self.cols.get(&c) {
            Some(ids) => {
                let n = ids.iter().map(|&i| &self.nodes[i as usize]).find(|n| (n.ceil as f64) > layer && n.floor as f64 <= layer.floor())?;
                n.surface()
            }
            None => {
                let (lo, hi) = g.generated(c);
                if lo >= hi || (layer * U as f64) < lo as f64 {
                    return None;
                }
                hi
            }
        };
        let s = top as f64 / U as f64;
        (layer < s).then_some(s)
    }

    /// One step of 1/60 s for every awake node.
    pub fn step(&mut self, g: &impl Ground) {
        self.awake.sort_unstable();
        self.awake.dedup();
        // moving water may reach any neighbour, so those are stored before anything moves; open sea next to
        // water at sea level is the exception: that water is held at sea level and trades with the sea
        let mut cols: Vec<Col> = self.awake.iter().filter(|&&a| !self.nodes[a as usize].rim).map(|&a| self.nodes[a as usize].col).collect();
        cols.sort_unstable();
        cols.dedup();
        for c in cols {
            let open = self.cols[&c].iter().copied().find(|&a| self.nodes[a as usize].ceil == OPEN);
            let (mut grow, mut sea) = (false, None);
            for d in g.neighbours(c).into_iter().flatten().filter(|d| !self.cols.contains_key(d)) {
                let (lo, hi) = g.generated(d);
                match open.map(|a| &self.nodes[a as usize]) {
                    Some(n) if lo < hi && n.floor as i64 * U < hi && (n.surface() - hi).abs() < SEA_DH => sea = Some(hi),
                    _ => grow = true,
                }
            }
            if grow || sea.is_none() {
                for d in g.neighbours(c).into_iter().flatten() {
                    self.materialize(g, d);
                }
                for &a in &self.cols[&c] {
                    self.nodes[a as usize].rim = true;
                }
            } else if let (Some(a), Some(hi)) = (open, sea) {
                let n = &mut self.nodes[a as usize];
                let want = hi - n.floor as i64 * U;
                self.sea_extra += n.water - want;
                n.water = want;
            }
        }
        self.awake.sort_unstable();
        self.awake.dedup();
        let awake = std::mem::take(&mut self.awake);
        self.stepped += awake.len() as u64;
        self.epoch = self.epoch.wrapping_add(1);
        let mut outs: Vec<(i32, i32, i32)> = Vec::new();
        // 1: speeds and outflows from the old state
        for &a in &awake {
            let n = &self.nodes[a as usize];
            let (ha, sa, wa) = (n.head(), n.surface(), n.water);
            // what came in last step from each side, and how fast
            let mut came = [(0i64, 0i64); 4];
            for m in &n.links {
                let c = &mut came[m.dir as usize & 3];
                (c.0, c.1) = (c.0 + m.fin as i64, c.1.max(m.vin as i64));
            }
            outs.clear();
            let mut total = 0i64;
            for l in &n.links {
                let b = &self.nodes[l.to as usize];
                let (lo, hi) = window(n, b);
                let area = (sa.min(hi) - lo).clamp(0, ACAP);
                let mut v = l.v as i64;
                if area == 0 || !spreads(n, b) {
                    v = 0;
                } else {
                    let dh = ha - b.head().max(lo);
                    v += dh * CG >> UB;
                    // water that came in from straight behind brings its speed: the share of this node's water
                    // it makes up moves on at the speed it came (a front runs, a jet keeps going)
                    let (fin, vin) = came[(l.dir ^ 1) as usize & 3];
                    if fin > 0 {
                        v += ((vin - v) as i128 * fin.min(wa) as i128 / wa.max(1) as i128) as i64;
                    }
                    // friction, stronger in a thin sheet
                    v -= (v >> FRICTION).max(1);
                    if area < U {
                        v -= v * (U - area) / (THIN * U);
                    }
                    if v < VSTOP {
                        v = 0;
                    }
                    v = v.clamp(0, VMAX);
                }
                let flow = v * area + l.frac as i64;
                outs.push((v as i32, (flow >> VF) as i32, (flow & ((1 << VF) - 1)) as i32));
                total += flow >> VF;
            }
            // never more out than there is
            let scale = total > wa;
            let n = &mut self.nodes[a as usize];
            for (l, &(v, out, frac)) in n.links.iter_mut().zip(&outs) {
                (l.v, l.frac) = (v, if v == 0 { 0 } else { frac });
                l.out = if scale { (out as i64 * wa.max(0) / total) as i32 } else { out };
            }
        }
        // 1b: water falling into a gap (its surface under the opening) goes down that column one block wide,
        // at most as fast as Torricelli's sqrt(2 g h) for the head h over the opening; a hole in a lake's floor
        // drains it through one block, not through every side of the column over the hole
        let mut fall: FxMap<u32, (i64, i64)> = FxMap::default();
        for &a in &awake {
            let n = &self.nodes[a as usize];
            for l in n.links.iter().filter(|l| l.out > 0) {
                let b = &self.nodes[l.to as usize];
                let lo = n.floor.max(b.floor) as i64 * U;
                if b.surface() < lo {
                    let e = fall.entry(l.to).or_insert((0, 0));
                    (e.0, e.1) = (e.0 + l.out as i64, e.1.max(n.head() - lo));
                }
            }
        }
        fall.retain(|_, (sum, h)| *sum > fall_cap(*h));
        if !fall.is_empty() {
            for &a in &awake {
                let fa = self.nodes[a as usize].floor;
                for k in 0..self.nodes[a as usize].links.len() {
                    let l = self.nodes[a as usize].links[k];
                    let b = &self.nodes[l.to as usize];
                    let lo = fa.max(b.floor) as i64 * U;
                    if let Some(&(sum, h)) = fall.get(&l.to) {
                        if l.out > 0 && b.surface() < lo {
                            let cap = fall_cap(h);
                            let m = &mut self.nodes[a as usize].links[k];
                            m.out = (m.out as i64 * cap / sum) as i32;
                            m.v = (m.v as i64 * cap / sum) as i32;
                        }
                    }
                }
            }
        }
        // 2: every awake node and every node it sends to gathers what flows in and out, and notes what came
        // in through each link for the next step's carried speed
        let mut touched: Vec<u32> = Vec::with_capacity(awake.len() * 2);
        for &a in &awake {
            self.nodes[a as usize].seen = self.epoch;
            touched.push(a);
        }
        for &a in &awake {
            for k in 0..self.nodes[a as usize].links.len() {
                let l = self.nodes[a as usize].links[k];
                if l.out > 0 && self.nodes[l.to as usize].seen != self.epoch {
                    self.nodes[l.to as usize].seen = self.epoch;
                    touched.push(l.to);
                }
            }
        }
        let mut moving: Vec<bool> = Vec::with_capacity(touched.len());
        for &a in &touched {
            let mut d = 0i64;
            let mut mv = false;
            for k in 0..self.nodes[a as usize].links.len() {
                let l = self.nodes[a as usize].links[k];
                let m = self.nodes[l.to as usize].links[l.back as usize];
                d += m.out as i64 - l.out as i64;
                mv |= l.v > 0 || l.out > 0 || m.out > 0;
                let n = &mut self.nodes[a as usize].links[k];
                (n.fin, n.vin) = (m.out, m.v);
            }
            self.nodes[a as usize].water += d;
            moving.push(mv);
        }
        // 3: who stays awake: moving nodes, nodes that just received, and the sleeping neighbours a level
        // difference would move
        let mut next = Vec::with_capacity(touched.len());
        for (&a, mv) in touched.iter().zip(moving) {
            let n = &self.nodes[a as usize];
            let mut pull = false;
            for l in &n.links {
                let b = &self.nodes[l.to as usize];
                // an awake neighbour sees to itself; a moving node stays awake anyway
                if mv && b.quiet != u16::MAX {
                    continue;
                }
                let (lo, hi) = window(n, b);
                let push = |x: &Node, y: &Node| (x.surface().min(hi) - lo) > 0 && moves(x.head() - y.head().max(lo)) && spreads(x, y);
                if push(n, b) || push(b, n) {
                    pull = true;
                    if b.quiet == u16::MAX {
                        next.push(l.to);
                    }
                }
            }
            let n = &mut self.nodes[a as usize];
            n.quiet = if mv || pull { 0 } else { n.quiet.saturating_add(1) };
            if n.quiet < QUIET {
                next.push(a);
            } else {
                n.quiet = u16::MAX;
                for l in &mut n.links {
                    (l.v, l.out, l.frac, l.fin, l.vin) = (0, 0, 0, 0, 0);
                }
                // asleep away from what the clients last heard: they get it exactly now
                if n.water != n.sent {
                    let c = n.col;
                    self.dirty.insert(c, ());
                }
            }
        }
        for &a in &next {
            if self.nodes[a as usize].quiet == u16::MAX {
                self.nodes[a as usize].quiet = 0;
            }
        }
        next.sort_unstable();
        next.dedup();
        self.awake = next;
        self.steps += 1;
        if self.steps % SETTLE_EVERY == 0 {
            self.settle(g);
        }
    }

    /// Calm water is laid flat and put to sleep as one body. Friction alone leaves a slowly filled lake
    /// tilted and lets the last millimetres creep for minutes; a body whose flow is all slow and whose level
    /// differs by less than SETTLE_DH is given its one exact level instead (every unit kept).
    fn settle(&mut self, g: &impl Ground) {
        let mut seen: FxMap<u32, ()> = FxMap::default();
        let mut forget: Vec<Col> = Vec::new();
        let starts = self.awake.clone();
        for a in starts {
            if seen.contains_key(&a) {
                continue;
            }
            // the body: nodes joined by links that are wet on both sides
            let mut body = vec![a];
            seen.insert(a, ());
            let (mut k, mut calm) = (0, true);
            let (mut lo_h, mut hi_h) = (i64::MAX, i64::MIN);
            let mut sea = None;
            while k < body.len() && calm {
                let n = &self.nodes[body[k] as usize];
                k += 1;
                if n.water == 0 {
                    continue;
                }
                // open sea next to it that is not stored: only a node whose rim was never stored can have it
                if !n.rim && n.ceil == OPEN {
                    for d in g.neighbours(n.col).into_iter().flatten().filter(|d| !self.cols.contains_key(d)) {
                        let (lo, hi) = g.generated(d);
                        if lo < hi && n.surface() > lo.max(n.floor as i64 * U) {
                            sea = Some(hi);
                        }
                    }
                }
                let h = n.head();
                (lo_h, hi_h) = (lo_h.min(h), hi_h.max(h));
                for l in &n.links {
                    calm &= (l.v as i64) < VSLOW;
                    let b = &self.nodes[l.to as usize];
                    let (lo, hi) = window(n, b);
                    if n.surface().min(hi) > lo && b.surface().min(hi) > lo && !seen.contains_key(&l.to) {
                        seen.insert(l.to, ());
                        body.push(l.to);
                    }
                }
                calm &= hi_h - lo_h < SETTLE_DH && body.len() <= SETTLE_MAX;
            }
            if !calm || body.len() < 2 || sea.is_some_and(|l| (lo_h - l).abs().max((hi_h - l).abs()) >= SETTLE_DH) {
                continue;
            }
            body.sort_unstable();
            self.flatten(&body, sea);
            if sea.is_some() {
                // columns back to what generation gives, wet or the dry shore around them, are not stored
                for &a in &body {
                    forget.push(self.nodes[a as usize].col);
                    for l in &self.nodes[a as usize].links {
                        forget.push(self.nodes[l.to as usize].col);
                    }
                }
            }
        }
        let flat: FxMap<u32, ()> = seen;
        self.awake.retain(|a| self.nodes[*a as usize].quiet != u16::MAX || !flat.contains_key(a));
        forget.sort_unstable();
        forget.dedup();
        for c in forget {
            let Some(ids) = self.cols.get(&c) else { continue };
            let (lo, hi) = g.generated(c);
            let plain = match ids[..] {
                [a] => {
                    let n = &self.nodes[a as usize];
                    n.floor as i64 * U == lo && n.water == (hi - lo).max(0) && n.quiet == u16::MAX && g.gaps(c) == [(n.floor, OPEN)]
                }
                _ => false,
            };
            if plain {
                self.remove(c, false);
                self.gen_total -= self.gen.remove(&c).unwrap_or(0);
                self.dirty.insert(c, ());
            }
        }
    }

    /// Gives every node of `body` the water that puts all of them at one level, and puts them to sleep: the
    /// level its water reaches, or `sea` (in units) with the difference given to the open sea.
    fn flatten(&mut self, body: &[u32], sea: Option<i64>) {
        let w = |n: &Node, l: i64| -> i64 {
            let f = n.floor as i64 * U;
            if l <= f {
                0
            } else if n.ceil == OPEN || l <= n.ceil as i64 * U {
                l - f
            } else {
                n.cap() + (l - n.ceil as i64 * U) / K
            }
        };
        let total: i64 = body.iter().map(|&a| self.nodes[a as usize].water).sum();
        // the highest level that does not need more water than there is
        let ns = || body.iter().map(|&a| &self.nodes[a as usize]);
        let mut lo = ns().map(|n| n.floor as i64 * U).min().unwrap();
        let mut hi = ns().map(|n| if n.ceil == OPEN { n.floor } else { n.ceil } as i64 * U).max().unwrap() + total * K + 1;
        if let Some(l) = sea {
            lo = l;
        }
        while sea.is_none() && hi - lo > 1 {
            let mid = lo + (hi - lo) / 2;
            let need: i64 = body.iter().map(|&a| w(&self.nodes[a as usize], mid)).sum();
            if need <= total { lo = mid } else { hi = mid }
        }
        let mut left = total - body.iter().map(|&a| w(&self.nodes[a as usize], lo)).sum::<i64>();
        if sea.is_some() {
            self.sea_extra += left;
            left = 0;
        }
        for &a in body {
            let n = &mut self.nodes[a as usize];
            let (now, up) = (w(n, lo), w(n, lo + 1));
            let extra = (up - now).min(left);
            left -= extra;
            n.water = now + extra;
            n.quiet = u16::MAX;
            for l in &mut n.links {
                (l.v, l.out, l.frac, l.fin, l.vin) = (0, 0, 0, 0, 0);
            }
            if n.water != n.sent {
                let c = n.col;
                self.dirty.insert(c, ());
            }
        }
        debug_assert_eq!(left, 0);
    }
}

/// A box world for tests and the prototype: nx x ny columns of nz layers, walls around it, open above.
pub mod test_ground {
    use super::*;

    /// nx x ny columns of nz layers, walls around it, open above.
    pub struct Box3 {
        pub nx: i32,
        pub ny: i32,
        pub nz: i32,
        pub solid: Vec<bool>,
        /// The sea level and the floors it was generated over (the terrain before any edit).
        pub sea: Option<(i64, Vec<i32>)>,
    }

    impl Box3 {
        pub fn new(nx: i32, ny: i32, nz: i32) -> Box3 {
            Box3 { nx, ny, nz, solid: vec![false; (nx * ny * nz) as usize], sea: None }
        }
        /// Generation fills every column to `level` units over the terrain as it is now.
        pub fn with_sea(&mut self, level: i64) {
            let floors = (0..self.ny).flat_map(|y| (0..self.nx).map(move |x| (x, y))).map(|(x, y)| self.gaps(col(x, y)).last().unwrap().0).collect();
            self.sea = Some((level, floors));
        }
        pub fn set(&mut self, x: i32, y: i32, z: i32, s: bool) {
            let i = (x + y * self.nx + z * self.nx * self.ny) as usize;
            self.solid[i] = s;
        }
        pub fn fill(&mut self, x: std::ops::Range<i32>, y: std::ops::Range<i32>, z: std::ops::Range<i32>, s: bool) {
            for a in x {
                for b in y.clone() {
                    for c in z.clone() {
                        self.set(a, b, c, s);
                    }
                }
            }
        }
        fn is_solid(&self, x: i32, y: i32, z: i32) -> bool {
            z < 0 || (z < self.nz && self.solid[(x + y * self.nx + z * self.nx * self.ny) as usize])
        }
    }

    impl Ground for Box3 {
        fn gaps(&self, c: Col) -> Vec<(i32, i32)> {
            let (x, y) = (c.i(), c.j());
            let mut out = Vec::new();
            let mut start = None;
            for z in 0..=self.nz {
                match (self.is_solid(x, y, z), start) {
                    (false, None) => start = Some(z),
                    (true, Some(s)) => {
                        out.push((s, z));
                        start = None;
                    }
                    _ => {}
                }
            }
            out.push((start.unwrap(), OPEN));
            out
        }
        fn generated(&self, c: Col) -> (i64, i64) {
            match &self.sea {
                Some((s, floors)) => (floors[(c.i() + c.j() * self.nx) as usize] as i64 * U, *s),
                None => (0, 0),
            }
        }
        fn neighbours(&self, c: Col) -> [Option<Col>; 4] {
            let (x, y) = (c.i(), c.j());
            [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]
                .map(|(a, b)| ((0..self.nx).contains(&a) && (0..self.ny).contains(&b)).then(|| Col::new(0, 0, a, b)))
        }
    }

    pub fn col(x: i32, y: i32) -> Col {
        Col::new(0, 0, x, y)
    }

    /// Every column of the box stored.
    pub fn stored(g: &Box3) -> Liquids {
        let mut w = Liquids::default();
        for y in 0..g.ny {
            for x in 0..g.nx {
                w.materialize(g, col(x, y));
            }
        }
        w
    }

    /// The water top of the open gap of column (x, y), in blocks.
    pub fn level(w: &Liquids, x: i32, y: i32) -> f64 {
        w.column(col(x, y)).unwrap().last().unwrap().surface() as f64 / U as f64
    }
}

#[cfg(test)]
mod tests {
    use super::test_ground::*;
    use super::*;

    #[test]
    fn col_packs_and_unpacks() {
        let c = Col::new(1, 5, 12287, 3);
        assert_eq!((c.body(), c.face(), c.i(), c.j()), (1, 5, 12287, 3));
    }

    #[test]
    fn still_water_stays_still_and_sleeps() {
        let mut g = Box3::new(8, 8, 8);
        g.fill(0..8, 0..8, 0..2, true);
        let mut w = stored(&g);
        for y in 0..8 {
            for x in 0..8 {
                w.pour(&g, col(x, y), 2, 3 * U);
            }
        }
        let t = w.total();
        for _ in 0..200 {
            w.step(&g);
        }
        assert_eq!(w.total(), t);
        assert_eq!(w.awake(), 0, "level water falls asleep");
        assert!((0..8).all(|x| level(&w, x, 3) == 5.0));
    }

    /// A dam breaks in a 60 m channel: 2 m of water rushes onto a dry floor. Ritter's solution gives a front
    /// at 2 sqrt(g h) = 8.9 m/s; a grid model is slower at the very tip. Then it all settles level and asleep
    /// within 150 s (a real one sloshes for a minute or two).
    #[test]
    fn a_dam_break_runs_at_real_speed_and_settles() {
        let mut g = Box3::new(120, 1, 8);
        g.fill(0..120, 0..1, 0..1, true);
        let mut w = stored(&g);
        for x in 0..40 {
            w.pour(&g, col(x, 0), 1, 4 * U);
        }
        let t = w.total();
        let front = |w: &Liquids| (0..120).rev().find(|&x| w.column(col(x, 0)).unwrap().last().unwrap().water >= U / 16).unwrap();
        for _ in 0..60 {
            w.step(&g);
        }
        // metres the front ran in one second
        let ran = (front(&w) - 39) as f64 * 0.5;
        assert!((4.0..10.0).contains(&ran), "the front ran {ran} m in 1 s");
        // the bore runs to the far wall, comes back and dies down
        for _ in 0..60 * 150 {
            w.step(&g);
        }
        assert_eq!(w.total(), t, "not a unit appears or vanishes");
        assert_eq!(w.awake(), 0, "settles and sleeps");
        let (lo, hi) = (0..120).map(|x| level(&w, x, 0)).fold((f64::MAX, f64::MIN), |(a, b), l| (a.min(l), b.max(l)));
        assert!(hi - lo < 0.02, "levels {lo}..{hi}");
    }

    /// Two shafts joined by a closed tunnel at the bottom: water poured into one rises in the other to the
    /// same height (the tunnel passes the pressure on).
    #[test]
    fn a_u_tube_evens_out() {
        let mut g = Box3::new(12, 1, 24);
        g.fill(0..12, 0..1, 0..24, true);
        g.fill(0..1, 0..1, 1..24, false); // left shaft
        g.fill(11..12, 0..1, 1..24, false); // right shaft
        g.fill(0..12, 0..1, 1..3, false); // the tunnel, two blocks high, under 21 blocks of rock
        let mut w = stored(&g);
        w.pour(&g, col(0, 0), 1, 40 * U);
        let t = w.total();
        for _ in 0..60 * 120 {
            w.step(&g);
        }
        assert_eq!(w.total(), t);
        let (l, r) = (level(&w, 0, 0), level(&w, 11, 0));
        assert!((l - r).abs() < 0.25, "left {l} right {r}");
        assert!(l > 3.5, "the water stands in the shafts, {l}");
    }

    /// Water over a ledge falls to the floor below and none stays on the ledge but a film.
    #[test]
    fn water_runs_off_a_ledge() {
        let mut g = Box3::new(20, 1, 12);
        g.fill(0..20, 0..1, 0..1, true);
        g.fill(0..6, 0..1, 1..8, true); // a ledge 7 blocks up
        let mut w = stored(&g);
        for x in 0..6 {
            w.pour(&g, col(x, 0), 8, 2 * U);
        }
        let t = w.total();
        for _ in 0..60 * 30 {
            w.step(&g);
        }
        assert_eq!(w.total(), t);
        let up: i64 = (0..6).map(|x| w.column(col(x, 0)).unwrap().last().unwrap().water).sum();
        assert!(up <= 6 * 2 * MIN, "{up} units left on the ledge: more than a film");
    }

    /// A drop of water on a wide floor spreads into a puddle and stops: it does not crawl on as a film.
    #[test]
    fn a_puddle_stops_spreading() {
        let mut g = Box3::new(64, 64, 4);
        g.fill(0..64, 0..64, 0..1, true);
        let mut w = stored(&g);
        w.pour(&g, col(32, 32), 1, 8 * U);
        for _ in 0..60 * 30 {
            w.step(&g);
        }
        assert_eq!(w.total(), 8 * U);
        assert_eq!(w.awake(), 0);
        let wet = (0..64).flat_map(|y| (0..64).map(move |x| (x, y))).filter(|&(x, y)| w.column(col(x, y)).unwrap().last().unwrap().water > 0).count();
        assert!((8..=(8 * U / MIN) as usize).contains(&wet), "{wet} wet columns");
        assert!(wet < 64 * 64 / 2, "{wet} wet columns: the puddle crawled over the floor");
    }

    /// A block placed in a pool pushes its water up; a floor dug out lets the water down into the hole.
    #[test]
    fn edits_move_water_but_keep_every_unit() {
        let mut g = Box3::new(6, 6, 10);
        g.fill(0..6, 0..6, 0..3, true);
        let mut w = stored(&g);
        for y in 0..6 {
            for x in 0..6 {
                w.pour(&g, col(x, y), 3, 2 * U);
            }
        }
        let t = w.total();
        g.set(2, 2, 3, true);
        w.regap(&g, col(2, 2));
        assert_eq!(w.total(), t);
        let n: Vec<_> = w.column(col(2, 2)).unwrap().map(|n| (n.floor, n.water)).collect();
        assert_eq!(n, vec![(4, 2 * U)], "the water moved up over the new block");
        g.set(4, 4, 2, false);
        g.set(4, 4, 1, false);
        w.regap(&g, col(4, 4));
        assert_eq!(w.total(), t);
        for _ in 0..600 {
            w.step(&g);
        }
        assert_eq!(w.total(), t);
        assert_eq!(w.column(col(4, 4)).unwrap().last().unwrap().floor, 1);
        // a closed hollow in the ground fills only if something opens it
        g.fill(1..2, 1..2, 0..2, false);
        w.regap(&g, col(1, 1));
        let gaps: Vec<_> = w.column(col(1, 1)).unwrap().map(|n| (n.floor, n.ceil, n.water)).collect();
        assert_eq!(gaps[0], (0, 2, 0));
    }

    /// Untouched sea is generated and still; a stored sea column next to it agrees and nothing moves.
    #[test]
    fn generated_sea_is_level() {
        let mut g = Box3::new(10, 10, 10);
        g.fill(0..10, 0..10, 0..2, true);
        g.fill(0..5, 0..10, 2..4, true);
        g.with_sea(5 * U);
        let mut w = Liquids::default();
        w.regap(&g, col(7, 7));
        for _ in 0..120 {
            w.step(&g);
        }
        assert_eq!(w.awake(), 0);
        assert_eq!(w.counted(), 0);
        assert_eq!(w.surface_at(&g, col(2, 2), 4.5), Some(5.0));
        assert_eq!(w.surface_at(&g, col(2, 2), 5.5), None);
    }

    /// A basin filled slowly from one end ends level: the slope a slow fill needs does not stay behind.
    #[test]
    fn a_slow_fill_ends_level() {
        let mut g = Box3::new(80, 3, 8);
        g.fill(0..80, 0..3, 0..1, true);
        let mut w = stored(&g);
        for _ in 0..60 * 20 {
            w.pour(&g, col(0, 1), 1, U / 8);
            w.step(&g);
        }
        for _ in 0..60 * 60 {
            w.step(&g);
        }
        let (lo, hi) = (0..80).map(|x| level(&w, x, 1)).fold((f64::MAX, f64::MIN), |(a, b), l| (a.min(l), b.max(l)));
        assert!(hi - lo < 0.01, "levels {lo}..{hi}");
        assert_eq!(w.awake(), 0);
    }

    /// A tank 4 m deep empties through a hole in its floor about as fast as Torricelli says
    /// (v = sqrt(2 g h)), slowing as it empties.
    #[test]
    fn a_tank_drains_through_its_floor() {
        let mut g = Box3::new(9, 9, 30);
        g.fill(0..9, 0..9, 0..20, true);
        g.fill(2..7, 2..7, 12..20, false); // the tank: 5 x 5 columns, 8 blocks deep
        g.fill(0..9, 0..9, 0..11, false); // a big room under it
        g.fill(0..9, 0..9, 0..1, true);
        let mut w = stored(&g);
        for y in 2..7 {
            for x in 2..7 {
                w.pour(&g, col(x, y), 12, 8 * U);
            }
        }
        g.set(4, 4, 11, false); // the hole: one block
        w.regap(&g, col(4, 4));
        let t = w.total();
        let tank = |w: &Liquids| -> i64 {
            (2..7).flat_map(|y| (2..7).map(move |x| (x, y))).map(|(x, y)| w.column(col(x, y)).unwrap().filter(|n| n.floor >= 11).map(|n| n.water).sum::<i64>()).sum()
        };
        let start = tank(&w);
        for _ in 0..60 {
            w.step(&g);
        }
        // a 0.5 m hole under 4 m of water passes about 0.25 m^2 x 8.9 m/s; in blocks per second:
        let rate = (start - tank(&w)) as f64 / U as f64;
        assert!((10.0..80.0).contains(&rate), "{rate} blocks in the first second");
        for _ in 0..60 * 30 {
            w.step(&g);
        }
        assert_eq!(w.total(), t);
        assert!(tank(&w) < 25 * MIN, "{} units left in the tank", tank(&w));
    }

    /// A channel dug from the sea fills to sea level; the sea far away stays generated and untouched.
    #[test]
    fn the_sea_fills_a_dug_channel() {
        // a sea of 600 columns: what the channel takes from it lowers it by less than SETTLE_DH, so it is
        // laid back to sea level and forgotten
        let mut g = Box3::new(40, 30, 12);
        g.fill(0..40, 0..30, 0..2, true);
        g.fill(20..40, 0..30, 2..8, true); // land up to layer 8, sea over the rest at layer 6
        g.with_sea(6 * U);
        let mut w = Liquids::default();
        // dig a channel 3 deep into the land, from the shore inwards
        for x in 20..30 {
            g.fill(x..x + 1, 2..3, 5..8, false);
            w.regap(&g, col(x, 2));
        }
        for _ in 0..60 * 60 {
            w.step(&g);
        }
        assert_eq!(w.awake(), 0);
        // level with the sea
        for x in 20..30 {
            let l = level(&w, x, 2);
            assert!((l - 6.0).abs() < 0.001, "column {x} at {l}");
        }
        // the water in the channel came out of the sea, which is counted
        assert_eq!(w.counted(), 0);
        assert!(w.sea_extra < 0, "the open sea gave {} units", -w.sea_extra);
        assert!(w.stored() < 100, "{} columns still stored", w.stored());
    }

    /// Buckets poured into the open sea spread, calm down, and are given to the sea: nothing stays stored, and
    /// the water is still counted.
    #[test]
    fn a_splash_in_the_sea_is_forgotten_when_calm() {
        let mut g = Box3::new(80, 80, 30);
        g.fill(0..80, 0..80, 0..4, true);
        g.with_sea(20 * U);
        let mut w = Liquids::default();
        w.pour(&g, col(40, 40), 21, 4 * U);
        let mut peak = 0;
        for _ in 0..60 * 60 {
            w.step(&g);
            peak = peak.max(w.stored());
        }
        assert_eq!((w.awake(), w.stored()), (0, 0), "after a peak of {peak} stored columns");
        assert_eq!((w.counted(), w.sea_extra), (4 * U, 4 * U));
        let gone = w.take_changes();
        assert!(gone.iter().all(|(_, gaps)| gaps.is_empty()) && !gone.is_empty(), "clients hear the columns are generated again");
    }

    /// The same pours give the same water, bit for bit.
    #[test]
    fn deterministic() {
        let run = || {
            let mut g = Box3::new(30, 30, 6);
            g.fill(0..30, 0..30, 0..1, true);
            g.fill(10..12, 0..20, 1..4, true);
            let mut w = stored(&g);
            for k in 0..40 {
                w.pour(&g, col(k % 30, (k * 7) % 30), 1, U + k as i64);
                w.step(&g);
            }
            for _ in 0..300 {
                w.step(&g);
            }
            w.all()
        };
        assert_eq!(run(), run());
    }
}
