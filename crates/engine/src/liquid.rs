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
/// g dt^2 / dx in 1/2^VF blocks per step per block of head, for dt = 1/60 s, at the pull every world before real
/// gravity flowed by (9.81 m/s^2, whatever its body's). Under real gravity each body's own (`Ground::g`, `cg`);
/// this stays what `moves` reckons a head with, so its dead band is a head (1.6 mm) on every body.
const CG: i64 = cg(EARTH_G);
/// The pull water flowed by before real gravity (`world::REAL`), m/s^2.
pub const EARTH_G: f64 = 9.81;
/// `CG` under a pull of `g` m/s^2.
const fn cg(g: f64) -> i64 {
    (g / 3600.0 / crate::world::BLOCK * (1u64 << VF) as f64) as i64
}
/// Fastest flow: half a block per step (30 m/s).
const VMAX: i64 = 1 << (VF - 1);
/// The deepest water that still speeds a link up; deeper water flows as if it were this deep. It keeps a
/// step of 1/60 s stable in deep water (waves there run at sqrt(g 16 m) instead of sqrt(g depth)).
const ACAP: i64 = 16 * U;
/// Water shallower than this does not spread over level ground (surface tension; puddles stay puddles).
pub const MIN: i64 = U / 32;
/// Friction: a link loses 1/2^FRICTION of its speed a step, and up to 1/THIN more in water under a block deep.
const FRICTION: u32 = 9;
const THIN: i64 = 128;
/// A link slower than this (0.2 mm/s) stops, and a height difference too small to speed a link past it
/// (1.6 mm) moves nothing and keeps nothing awake. Small, because a slow fill leaves its slope behind: a
/// lake filled from one end ends tilted by up to this per column.
const VSTOP: i64 = 1 << 2;
/// Flow slower than this (12 cm/s) is calm (`settle`).
const VSLOW: i64 = 1 << 11;
/// Every SETTLE_EVERY steps, a body of water whose flow is all slower than VSLOW and whose level varies by
/// less than SETTLE_DH (3 cm) is laid flat (`settle`). Bodies over SETTLE_MAX nodes are left to friction.
const SETTLE_EVERY: u64 = 15;
const SETTLE_DH: i64 = U / 32;
/// Stored water next to open sea that is not stored, and this close to sea level, trades with the sea
/// instead of storing it: the edge of the stored region is open water, and a wave smaller than this (3 cm)
/// leaves through it.
const SEA_DH: i64 = U / 32;
const SETTLE_MAX: usize = 200_000;

/// Units a step that fall through one block of opening under `h` units of head under a pull of `g`: sqrt(2 g h) dt / dx.
fn fall_cap(h: i64, g: f64) -> i64 {
    let m = h.max(U / 8) as f64 / U as f64 * crate::world::BLOCK;
    ((2.0 * g * m).sqrt() / 60.0 / crate::world::BLOCK * U as f64) as i64
}

fn moves(dh: i64) -> bool {
    dh * CG >> 12 > VSTOP
}
/// `moves` for a fluid of viscosity `visc` over a window `area` deep under its body's `cg`: the speed a head
/// difference gives a link from rest in one step, after the friction the step takes (`Liquids::step`), reaches
/// `VSTOP`. Water keeps `moves`, whose dead band is a head on every body.
fn thick_moves(dh: i64, cg: i64, visc: i64, area: i64) -> bool {
    let mut v = dh * cg >> UB;
    v -= (v * visc >> FRICTION).max(1);
    if area < U {
        v -= v * (U - area) * visc / (THIN * U);
    }
    v >= VSTOP
}

/// Thin water runs downhill but spreads over level ground only while it is deeper than MIN, and onto dry
/// ground only while it could leave both sides that deep: a puddle's edge stops instead of crawling on.
fn spreads(a: &Node, b: &Node) -> bool {
    b.floor < a.floor || a.water >= if b.water == 0 { 2 * MIN } else { MIN }
}
/// Fluid can pass from `a` into `b`: `b` is empty or holds the same fluid.
/// shortcut: two fluids never share a gap, a plain wall stands between them; liquids.md step 4's layers remove it
fn meets(a: &Node, b: &Node) -> bool {
    b.water == 0 || a.kind == b.kind
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
    /// The account the open water of column `c` books against: a reach of a river or a lake, or `SEA`.
    fn reach(&self, _c: Col) -> u32 {
        SEA
    }
    /// A river column's flow, units a step per column of width; its generated surface's fall, units a column;
    /// and the runoff it gathers, units a step (None: still water). Its friction is chosen from the first two
    /// (`drag_for`) so the live water carries the reach's flow at its generated level, and live it is given the
    /// third, as generation's flow grows downstream by it.
    fn river(&self, _c: Col) -> Option<(f64, f64, f64)> {
        None
    }
    /// The account reach `acct`'s water flows on into, and how long the reach holds it, in seconds: what its
    /// open water took in leaves it at that share of a second (`Liquids::step`, once a second).
    fn down(&self, _acct: u32) -> (u32, f64) {
        (SEA, 0.0)
    }
    /// Reach `acct`'s generated (dry-weather) flow, units a second (0: none, as a lake's). Live water upstream
    /// is poured the rain's flow (`reach_flow`) on top, at this flow's share per column.
    fn base(&self, _acct: u32) -> f64 {
        0.0
    }
    /// The tide's moment (ADR 0067): `generated` changes only when this does (the whole second of world time
    /// the sea's tide was taken at; 0 for a sea without a tide).
    fn tide_epoch(&self) -> i64 {
        0
    }
    /// The highest the generated sea can stand in column `c`'s body at any tide, units (MIN: no sea, MAX: unknown): stored
    /// columns floored over it never meet the sea, whatever the tide does.
    fn tide_top(&self, _c: Col) -> i64 {
        i64::MAX
    }
    /// The pull water flows by on body `body`, m/s^2 (ADR 0180: its own from real gravity on; `EARTH_G` before).
    fn g(&self, _body: u8) -> f64 {
        EARTH_G
    }
}

/// `Ground::reach` and `Ground::river` of column c, from `cache` or asked once.
fn river_of(cache: &mut FxMap<Col, (u32, Option<(f64, f64, f64)>)>, g: &impl Ground, c: Col) -> (u32, Option<(f64, f64, f64)>) {
    *cache.entry(c).or_insert_with(|| (g.reach(c), g.river(c)))
}

/// The friction that makes water `depth` units deep on a surface falling `grad` units a column carry `q`
/// units a step per column of width, in steady flow: there each link's speed-up from the slope equals its
/// friction, so v = 65536 grad CG / (U f), and the flow v depth / 2^VF is q. Proportional to the depth, the
/// loss is the same on every column whatever the bed's steps do, so the live surface falls as the generated
/// one does.
pub fn drag_for(grad: f64, depth: f64, q: f64, g: f64) -> i32 {
    (65536.0 * grad * cg(g) as f64 * depth / (U as f64 * q.max(1.0) * (1u64 << VF) as f64)).clamp(65536.0 / 2048.0, 16384.0) as i32
}
/// The speed of steady flow down a fall of `dh` units a column under friction `f` (`drag_for` of the same `g`).
fn steady_speed(dh: i64, f: i32, g: f64) -> i32 {
    if f <= 0 || dh <= 0 {
        return 0;
    }
    (65536 * dh as i128 * cg(g) as i128 / (U as i128 * f as i128)).min(VMAX as i128) as i32
}

/// The open sea's account (`Ground::reach`).
pub const SEA: u32 = u32::MAX;
/// The wire's dead band: water this close to generation is generation (a reach's patch is forgotten).
pub const DEAD: i64 = U / 64;

/// One gap of a column, with its water and its links to the neighbours' gaps.
#[derive(Clone, Debug)]
pub struct Node {
    pub col: Col,
    pub floor: i32,
    pub ceil: i32,
    pub water: i64,
    /// The fluid it holds (`data/fluids.toml`'s row; 0 water, and 0 when empty): one kind a gap.
    pub kind: u8,
    links: Vec<Link>,
    quiet: u16,
    /// Its neighbour columns are stored (once stored, a column stays stored).
    rim: bool,
    /// The step that last listed it (`step`'s set without a sort).
    seen: u32,
    /// The water the clients were last told about (the wire's dead band).
    sent: i64,
    /// A river column's friction (`drag_for`; 0: still water's) and its surface's fall a column, units.
    drag: i32,
    grad: i32,
    /// Runoff it gathers, in 1/65536 units a step, and what did not make a whole unit yet.
    lat: u32,
    lacc: u32,
    /// Its generated surface, units (a river column's).
    top: i64,
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

/// A gap's units and its fluid as the wire and saves carry them (`ColWater`).
/// The kind rides in the top byte (kept under 128, so the number stays positive): water is kind 0, so water's
/// columns are the very numbers they were before fluid rows, and every older save and recording reads as water.
pub fn pack(kind: u8, units: i64) -> i64 {
    units | (kind as i64) << 56
}
pub fn unpack(v: i64) -> (u8, i64) {
    ((v >> 56) as u8, v & ((1 << 56) - 1))
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
    /// Nodes fallen asleep since the last `forget_matching`: a body that slept before it matched generation
    /// is tried once more.
    slept: Vec<u32>,
    /// Columns the clients must hear about.
    dirty: FxMap<Col, ()>,
    /// What generation put into each stored column, and its sum.
    gen: FxMap<Col, i64>,
    gen_total: i64,
    /// `Ground::generated` of the columns next to awake ones, kept: it samples the terrain noise, and a shore
    /// that trades with the sea asks for the same columns every step. shortcut: never pruned (24 bytes a shore
    /// column); prune it with `forget` if a long-lived server's shores ever make it large.
    gen_at: FxMap<Col, (i64, i64)>,
    /// `Ground::reach` and `Ground::river` of the same columns, kept the same way (`open_ends` asks every step).
    river_at: FxMap<Col, (u32, Option<(f64, f64, f64)>)>,
    /// Units the open sea took in from stored water (negative: gave out). Calm water at sea level next to
    /// sea that is not stored settles to exactly sea level, the difference goes here, and columns equal to
    /// generation are forgotten again. shortcut: the sea's level does not rise with it; a whole sea holds
    /// millions of blocks, so a bucket is far under a unit of its level, until the water cycle moves sea.
    pub sea_extra: i64,
    /// The same for the open water of each river reach and lake (`Ground::reach`): what it took in from
    /// stored water. The reach graph takes it into the reach's water.
    pub reach_extra: std::collections::BTreeMap<u32, i64>,
    /// Water rain brought into the reaches (`rain`): counted, as water carried in.
    pub rained: i64,
    /// The rain each reach holds (W5, ADR 0059): kept apart from `reach_extra`, which also carries what the
    /// reach lends its live water and gets back, so the rain's flow (`reach_flow`) never feeds on itself. Live
    /// water upstream is poured it on top of generation's dry-weather base; the rest runs down to the sea.
    pub rain_held: std::collections::BTreeMap<u32, i64>,
    /// Work done, for the stats line and the tests.
    pub stepped: u64,
    /// Steps taken: `settle` and `run_reaches` run on its multiples, so a save keeps it (W5).
    pub steps: u64,
    epoch: u32,
    /// A fluid other than water was ever stored: only then does a step sort out two kinds flowing into one gap.
    other: bool,
    /// Viscosities set by a test (`set_viscosity`), by kind; empty: the rows'.
    viscosity: Vec<i64>,
    /// Units of a fluid other than water an edit pushed out of a gap a denser one kept (`build`), by kind: counted.
    /// shortcut: not saved; with water and lava only, lava is never pushed out (it is the denser)
    pub spilt: std::collections::BTreeMap<u8, i64>,
    /// `Ground::tide_epoch` at the last step: when it moves, `retide`.
    tide_epoch: i64,
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
        (c, self.cols[&c].iter().map(|&i| &self.nodes[i as usize]).map(|n| (n.floor, n.ceil, pack(n.kind, n.water))).collect())
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
        self.total_of(crate::fluids::WATER)
    }
    /// The total of fluid `kind` stored, in units.
    pub fn total_of(&self, kind: u8) -> i64 {
        self.cols.values().flatten().map(|&i| &self.nodes[i as usize]).filter(|n| n.kind == kind).map(|n| n.water).sum()
    }
    /// What generation put into the stored columns.
    pub fn generated_total(&self) -> i64 {
        self.gen_total
    }
    /// The water that differs from the seed's: changed only by water carried in or out (a bucket, creative,
    /// later rain and steam), never by flowing (ADR 0020).
    pub fn counted(&self) -> i64 {
        self.total() - self.gen_total + self.sea_extra + self.reach_extra.values().sum::<i64>() + self.rain_held.values().sum::<i64>() - self.rained
    }
    /// Rain (or a spring) brings `units` into reach `acct` (`rain_held`; W5: the air's runoff, `Weather::tick`):
    /// it raises the reach's live water and runs on down to the sea.
    pub fn rain(&mut self, acct: u32, units: i64) {
        if acct == SEA {
            self.sea_extra += units;
        } else if units != 0 {
            *self.rain_held.entry(acct).or_default() += units;
        }
        self.rained += units;
    }
    fn book(&mut self, acct: u32, units: i64) {
        if acct == SEA {
            self.sea_extra += units;
        } else if units != 0 {
            *self.reach_extra.entry(acct).or_default() += units;
        }
    }

    /// A client's copy: column `c` becomes what the server sent (no links: a client does not simulate).
    pub fn set_column(&mut self, (c, gaps): &ColWater) {
        self.remove(*c, false);
        if gaps.is_empty() {
            return;
        }
        self.other |= gaps.iter().any(|g| unpack(g.2).0 != 0);
        let ids = gaps.iter().map(|&(floor, ceil, w)| (floor, ceil, unpack(w))).map(|(floor, ceil, (kind, water))| self.alloc(Node { col: *c, floor, ceil, water, links: Vec::new(), quiet: u16::MAX, kind, rim: true, seen: 0, sent: water, drag: 0, grad: 0, lat: 0, lacc: 0, top: 0 })).collect();
        self.cols.insert(*c, ids);
        self.tiles.entry(c.tile()).or_default().push(*c);
    }

    /// Tile `t` as the clients are told it: each column's water as `take_changes` last listed it. The current
    /// water would leak a neighbour's change within the dead band, and when that change undid itself while
    /// the column slept, nothing would ever send the client the undo.
    pub fn tile_sent(&self, t: Col) -> Vec<ColWater> {
        let mut v: Vec<ColWater> = self.tiles.get(&t).into_iter().flatten().map(|&c| (c, self.cols[&c].iter().map(|&i| &self.nodes[i as usize]).map(|n| (n.floor, n.ceil, pack(n.kind, n.sent))).collect())).collect();
        v.sort_unstable_by_key(|w| w.0);
        v
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
        // the save was taken after its second's `retide`: a loaded world steps as the running one did
        self.tide_epoch = g.tide_epoch();
        for (c, gaps) in cols {
            let mut wet = Vec::new();
            for &(floor, ceil, w) in gaps {
                let (kind, water) = unpack(w);
                let f = floor as i64 * U;
                let cap = if ceil == OPEN { i64::MAX } else { (ceil - floor) as i64 * U };
                wet.push((f, f + water.min(cap), (water - cap).max(0), kind));
                self.other |= kind != 0;
            }
            self.remove(*c, false);
            self.build(g, *c, &wet, false);
        }
        // a river's water was moving when it was saved: it runs on (the sea's and a pond's sleep)
        for (c, _) in cols {
            for a in self.cols.get(c).cloned().unwrap_or_default() {
                if self.nodes[a as usize].drag > 0 && self.nodes[a as usize].water > 0 {
                    self.wake(a);
                }
            }
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
        let wet = if lo < hi { vec![(lo, hi, 0, 0)] } else { Vec::new() };
        // asleep: generated water is level; a neighbour's flow wakes it
        self.build(g, c, &wet, false);
    }

    /// Builds column `c`'s nodes from its gaps and pours `wet` into them: intervals of water in units, each
    /// with the units it held under pressure, which stay with the gap that holds the interval's bottom. Water
    /// in what is solid now goes to the gap above it.
    /// Each interval is of one fluid (its fourth number): a gap holds one, so where an edit joins two, the denser
    /// keeps the gap and the other is pushed out: water to its account (as a block set into it, ADR 0071), another
    /// fluid to `spilt`.
    fn build(&mut self, g: &impl Ground, c: Col, wet: &[(i64, i64, i64, u8)], wake: bool) {
        if !self.gen.contains_key(&c) {
            let (lo, hi) = g.generated(c);
            self.gen.insert(c, (hi - lo).max(0));
            self.gen_total += (hi - lo).max(0);
        }
        let gaps = g.gaps(c);
        let mut water = vec![0i64; gaps.len()];
        let mut kinds = vec![0u8; gaps.len()];
        for &(lo, hi, extra, kind) in wet {
            let k = gaps.iter().rposition(|&(f, _)| f as i64 * U <= lo).unwrap_or(0);
            self.put(g, c, (&mut water[k], &mut kinds[k]), extra, kind);
            for (k, &(f, cl)) in gaps.iter().enumerate() {
                let (gl, gh) = (f as i64 * U, if cl == OPEN { i64::MAX } else { cl as i64 * U });
                self.put(g, c, (&mut water[k], &mut kinds[k]), (hi.min(gh) - lo.max(gl)).max(0), kind);
            }
            // the parts in solid layers (under each gap, down to the gap below): each rises into the gap above it,
            // unless that gap's floor stands over generation's still water here (the sea's, a lake's): a block set
            // into its surface pushed it out, and on top of the block it stood as a puddle over the sea for good
            // (ADR 0071); it joins the water's account instead, as a splash does when it is forgotten. A river's
            // runs off the block on its own (a dam and a pillar's tests hold it to that)
            let (glo, ghi) = g.generated(c);
            let mut below = i64::MIN;
            for (k, &(f, cl)) in gaps.iter().enumerate() {
                let gl = f as i64 * U;
                let part = (hi.min(gl) - lo.max(below)).max(0);
                // (only what lay under generation's level: water standing over it, poured or carried, still rises)
                let sunk = if kind == crate::fluids::WATER && glo < ghi && gl >= ghi && g.river(c).is_none() { (hi.min(gl).min(ghi) - lo.max(below)).max(0) } else { 0 };
                self.book(g.reach(c), sunk);
                self.put(g, c, (&mut water[k], &mut kinds[k]), part - sunk, kind);
                below = if cl == OPEN { i64::MAX } else { cl as i64 * U };
            }
        }
        let ids: Vec<u32> = gaps
            .iter()
            .zip(water)
            .zip(kinds)
            .map(|((&(floor, ceil), water), kind)| {
                let kind = if water > 0 { kind } else { 0 };
                let (lo, hi) = g.generated(c);
                let (drag, grad, lat) = match g.river(c) {
                    Some((q, grad, lat)) if ceil == OPEN && lo < hi && floor as i64 * U == lo => (drag_for(grad, (hi - lo) as f64, q, g.g(c.body())), grad as i32, (lat * 65536.0) as u32),
                    _ => (0, 0, 0),
                };
                self.alloc(Node { col: c, floor, ceil, water, kind, links: Vec::new(), quiet: 0, rim: false, seen: 0, sent: water, drag, grad, lat, lacc: 0, top: hi })
            })
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
                            // two river columns: the link starts at the speed generation's flow has, so a patch
                            // carries the reach's flow from its first step (from rest its upstream end would
                            // fill up while the water speeds up, and the patch would grow up the whole river)
                            let (na, nb) = (&self.nodes[a as usize], &self.nodes[b as usize]);
                            // across a generated fall (a reach's lip) the water speeds up on the reach's own fall,
                            // not the drop's: the lip keeps its depth, and passes the reach's flow
                            let (mut v, mut fall) = ((0, 0), 0u8);
                            if na.drag > 0 && nb.drag > 0 {
                                let (ga, gb) = (g.generated(c).1, g.generated(d).1);
                                let cap = |n: &Node| (U / 2).max(4 * n.grad as i64);
                                if (ga - gb).abs() > cap(na).min(cap(nb)) {
                                    fall = 4;
                                }
                                let dh = |dh: i64, n: &Node| if fall > 0 { dh.min(n.grad as i64) } else { dh };
                                let pull = g.g(c.body());
                                v = (steady_speed(dh(ga - gb, na), na.drag, pull), steady_speed(dh(gb - ga, nb), nb.drag, pull));
                            }
                            self.link(a, b, sa as u8 | fall, sb as u8 | fall, v);
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

    /// `units` of fluid `kind` into a gap being built that holds `gap` (units, kind): another fluid there, the
    /// lighter of the two is pushed out (`build`).
    fn put(&mut self, g: &impl Ground, c: Col, gap: (&mut i64, &mut u8), units: i64, kind: u8) {
        let (water, have) = gap;
        if units <= 0 {
            return;
        }
        if *water > 0 && *have != kind {
            let rows = crate::fluids::FLUIDS.now();
            let (out, n) = if rows.get(kind).density > rows.get(*have).density { (std::mem::replace(have, kind), std::mem::replace(water, units)) } else { (kind, units) };
            if out == crate::fluids::WATER {
                self.book(g.reach(c), n);
            } else {
                *self.spilt.entry(out).or_default() += n;
            }
            return;
        }
        *have = kind;
        *water += units;
    }

    fn link(&mut self, a: u32, b: u32, sa: u8, sb: u8, v: (i32, i32)) {
        let (la, lb) = (self.nodes[a as usize].links.len() as u16, self.nodes[b as usize].links.len() as u16);
        let l = |to, back, dir, v| Link { to, back, v, out: 0, frac: 0, dir, fin: 0, vin: 0 };
        self.nodes[a as usize].links.push(l(b, lb, sa, v.0));
        self.nodes[b as usize].links.push(l(a, la, sb, v.1));
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
                let wet = if lo < hi { vec![(lo, hi, 0, 0)] } else { Vec::new() };
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
            wet.push((f, f + n.water.min(n.cap()), (n.water - n.cap()).max(0), n.kind));
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
        self.pour_kind(g, c, layer, units, crate::fluids::WATER)
    }
    /// `pour` of fluid `kind`.
    pub fn pour_kind(&mut self, g: &impl Ground, c: Col, layer: i32, units: i64, kind: u8) -> bool {
        self.materialize(g, c);
        let ids = &self.cols[&c];
        let Some(&a) = ids.iter().find(|&&i| self.nodes[i as usize].ceil > layer) else { return false };
        let n = &mut self.nodes[a as usize];
        if n.water > 0 && n.kind != kind {
            return false;
        }
        n.kind = kind;
        self.other |= kind != 0;
        n.water += units;
        self.touch(g, a);
        true
    }
    /// `scoop` of fluid `kind`.
    pub fn scoop_kind(&mut self, g: &impl Ground, c: Col, layer: i32, units: i64, kind: u8) -> i64 {
        self.materialize(g, c);
        let ids = &self.cols[&c];
        let Some(&a) = ids.iter().find(|&&i| self.nodes[i as usize].ceil > layer) else { return 0 };
        let n = &mut self.nodes[a as usize];
        if n.kind != kind {
            return 0;
        }
        let took = units.min(n.water).max(0);
        n.water -= took;
        if n.water == 0 {
            n.kind = 0;
        }
        self.touch(g, a);
        took
    }
    /// The units of fluid `kind` that differ from the seed's (`counted` for water).
    pub fn counted_of(&self, kind: u8) -> i64 {
        // (only water is generated, trades with the sea and rivers, or rains)
        if kind == crate::fluids::WATER {
            return self.counted();
        }
        self.total_of(kind) + self.spilt.get(&kind).copied().unwrap_or(0)
    }
    /// Fluid `kind` flows with viscosity `v` instead of its row's (tests).
    pub fn set_viscosity(&mut self, kind: u8, v: i64) {
        if self.viscosity.is_empty() {
            self.viscosity = crate::fluids::FLUIDS.rows.iter().map(|r| r.viscosity as i64).collect();
        }
        self.viscosity[kind as usize] = v;
    }

    /// Takes up to `units` from the gap of column `c` that holds layer `layer`; returns what it took.
    pub fn scoop(&mut self, g: &impl Ground, c: Col, layer: i32, units: i64) -> i64 {
        self.scoop_kind(g, c, layer, units, crate::fluids::WATER)
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
                // shortcut: only water holds up, is drunk or swum in; lava neither holds up nor hurts yet (step 4)
                if n.kind != crate::fluids::WATER {
                    return None;
                }
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
        if g.tide_epoch() != self.tide_epoch {
            self.tide_epoch = g.tide_epoch();
            self.retide(g);
        }
        self.awake.sort_unstable();
        self.awake.dedup();
        self.open_ends(g, true);
        self.awake.sort_unstable();
        self.awake.dedup();
        // the runoff a live river column gathers, from its reach's account
        for k in 0..self.awake.len() {
            let n = &mut self.nodes[self.awake[k] as usize];
            if n.lat > 0 && n.kind == crate::fluids::WATER {
                n.lacc += n.lat;
                let add = (n.lacc >> 16) as i64;
                n.lacc &= 0xFFFF;
                if add > 0 {
                    n.water += add;
                    let c = n.col;
                    let acct = river_of(&mut self.river_at, g, c).0;
                    self.book(acct, -add);
                }
            }
        }
        let awake = std::mem::take(&mut self.awake);
        self.stepped += awake.len() as u64;
        self.epoch = self.epoch.wrapping_add(1);
        // 1: speeds and outflows from the old state: each node reads only its own links and its neighbours'
        // water, so the nodes are worked out in parallel and written back in order
        let nodes = &self.nodes;
        // each body's pull, as its speed-up a step (`cg`)
        let pulls: [f64; crate::bodies::MAX] = std::array::from_fn(|b| g.g(b as u8));
        let cgs = pulls.map(cg);
        // each fluid's friction a step, in water's (`data/fluids.toml` viscosity)
        let rows: Vec<i64> = if self.viscosity.is_empty() { crate::fluids::FLUIDS.rows.iter().map(|r| r.viscosity as i64).collect() } else { self.viscosity.clone() };
        let flows = |&a: &u32| {
            let n = &nodes[a as usize];
            let cg = cgs[n.col.body() as usize];
            let visc = rows[n.kind as usize];
            let (ha, sa, wa) = (n.head(), n.surface(), n.water);
            // what came in last step from each side, and how fast
            let mut came = [(0i64, 0i64); 4];
            for m in &n.links {
                let c = &mut came[m.dir as usize & 3];
                (c.0, c.1) = (c.0 + m.fin as i64, c.1.max(m.vin as i64));
            }
            let mut outs: Vec<(i32, i32, i32)> = Vec::with_capacity(n.links.len());
            let mut total = 0i64;
            for l in &n.links {
                let b = &nodes[l.to as usize];
                let (lo, hi) = window(n, b);
                let area = (sa.min(hi) - lo).clamp(0, ACAP);
                let mut v = l.v as i64;
                if area == 0 || !spreads(n, b) || !meets(n, b) {
                    v = 0;
                } else {
                    let mut dh = ha - b.head().max(lo);
                    if l.dir & 4 != 0 {
                        // over a fall the reach's surface runs on as if there were none: the lip passes the
                        // reach's flow when it stands at its generated level, more when higher
                        dh = dh.min(n.grad as i64 + ha - n.top);
                    }
                    v += dh * cg >> UB;
                    // water that came in from straight behind brings its speed: the share of this node's water
                    // it makes up moves on at the speed it came (a front runs, a jet keeps going)
                    let (fin, vin) = came[(l.dir ^ 1) as usize & 3];
                    if fin > 0 && n.drag == 0 {
                        v += ((vin - v) as i128 * fin.min(wa) as i128 / wa.max(1) as i128) as i64;
                    }
                    // friction, stronger in a thin sheet
                    if n.drag > 0 {
                        v -= (v * n.drag as i64 >> 16).max(1);
                    } else {
                        // a thick fluid loses `visc` times water's (water's 1 is the old rule to the bit)
                        v -= (v * visc >> FRICTION).max(1);
                        if area < U {
                            v -= v * (U - area) * visc / (THIN * U);
                        }
                    }
                    if v < VSTOP {
                        v = 0;
                    }
                    v = v.clamp(0, VMAX);
                }
                let mut flow = v * area + l.frac as i64;
                // a closed gap's head rises K times as fast as the water it takes in: unbounded, a full gap under
                // a block set into the sea took in a wave, shot it out again at K times the height, and swung
                // on, pumping water out over the block and leaving the gap half empty (ADR 0071). Into or out of
                // a closed gap, a link passes at most what would level the two heads, shared over its links: half
                // the difference between two open surfaces, a (K + 1)-th of it against a full gap's pressure.
                if v > 0 && (n.ceil != OPEN || b.ceil != OPEN) {
                    let dh = (ha - b.head().max(lo)).max(0);
                    let mut most = i64::MAX;
                    if b.ceil != OPEN {
                        let room = (b.cap() - b.water).max(0);
                        let level = if 2 * room >= dh { dh / 2 } else { room + (dh - 2 * room) / (K + 1) };
                        most = most.min(level / b.links.len().max(1) as i64);
                    }
                    if n.ceil != OPEN {
                        let over = (wa - n.cap()).max(0);
                        most = most.min((over + (sa - b.head().max(lo)).max(0) / 2) / n.links.len().max(1) as i64);
                    }
                    flow = flow.min(most.max(0) << VF);
                }
                outs.push((v as i32, (flow >> VF) as i32, (flow & ((1 << VF) - 1)) as i32));
                total += flow >> VF;
            }
            // never more out than there is
            if total > wa {
                for o in &mut outs {
                    o.1 = (o.1 as i64 * wa.max(0) / total) as i32;
                }
            }
            outs
        };
        // on the caller's thread: side by side on rayon's global pool, a server tick waited behind the client's chunk
        // jobs and cave walks there; alone a step of 54,000 awake nodes took 12 ms so, 8 ms side by side
        let outs: Vec<(i32, i32, i32)> = awake.iter().flat_map(flows).collect();
        let mut outs = outs.into_iter();
        for &a in &awake {
            for (l, (v, out, frac)) in self.nodes[a as usize].links.iter_mut().zip(&mut outs) {
                (l.v, l.frac, l.out) = (v, if v == 0 { 0 } else { frac }, out);
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
        let nodes = &self.nodes;
        fall.retain(|&to, (sum, h)| *sum > fall_cap(*h, pulls[nodes[to as usize].col.body() as usize]));
        if !fall.is_empty() {
            for &a in &awake {
                let fa = self.nodes[a as usize].floor;
                for k in 0..self.nodes[a as usize].links.len() {
                    let l = self.nodes[a as usize].links[k];
                    let b = &self.nodes[l.to as usize];
                    let lo = fa.max(b.floor) as i64 * U;
                    if let Some(&(sum, h)) = fall.get(&l.to) {
                        if l.out > 0 && b.surface() < lo {
                            let cap = fall_cap(h, pulls[b.col.body() as usize]);
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
        // a gap holds one fluid: a dry one takes the kind of its first link that brings any, and what another kind
        // would bring it this step stays where it was (shortcut: a plain wall between fluids, as `meets`)
        if self.other {
            for &a in &touched {
                if self.nodes[a as usize].water != 0 {
                    continue;
                }
                let mut kind = None;
                for k in 0..self.nodes[a as usize].links.len() {
                    let l = self.nodes[a as usize].links[k];
                    let (from, m) = (self.nodes[l.to as usize].kind, self.nodes[l.to as usize].links[l.back as usize]);
                    if m.out <= 0 {
                        continue;
                    }
                    match kind {
                        None => kind = Some(from),
                        Some(first) if first != from => {
                            let m = &mut self.nodes[l.to as usize].links[l.back as usize];
                            (m.out, m.v, m.frac) = (0, 0, 0);
                        }
                        _ => {}
                    }
                }
                if let Some(k) = kind {
                    self.nodes[a as usize].kind = k;
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
            let n = &mut self.nodes[a as usize];
            n.water += d;
            if n.water == 0 {
                n.kind = 0;
            }
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
                let push = |x: &Node, y: &Node| {
                    let (area, dh) = (x.surface().min(hi) - lo, x.head() - y.head().max(lo));
                    let visc = rows[x.kind as usize];
                    // a thick fluid: only while the speed a step gives it outlasts its friction (else it never sleeps)
                    let moving = if visc == 1 { moves(dh) } else { thick_moves(dh, cgs[x.col.body() as usize], visc, area) };
                    area > 0 && moving && spreads(x, y) && meets(x, y)
                };
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
                self.slept.push(a);
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
        self.open_ends(g, false);
        self.steps += 1;
        if self.steps % SETTLE_EVERY == 0 {
            self.settle(g);
            self.forget_matching(g);
        }
        if self.steps % 60 == 0 {
            self.run_reaches(g);
        }
    }

    /// The tide moved (ADR 0067; once a second): generation's levels are asked again, and stored water beside
    /// open sea whose level moved by the dead band or more wakes, so its open end lets the sea in or out (the
    /// difference booked to `sea_extra` as ever). Sleeping water away from the sea costs one comparison.
    fn retide(&mut self, g: &impl Ground) {
        self.gen_at.clear();
        self.river_at.clear();
        let mut wake = Vec::new();
        for (&c, ids) in &self.cols {
            let Some(&a) = ids.iter().find(|&&a| self.nodes[a as usize].ceil == OPEN) else { continue };
            let n = &self.nodes[a as usize];
            if n.rim || n.quiet != u16::MAX || n.floor as i64 * U >= g.tide_top(c) {
                continue;
            }
            // the level `open_ends` holds this column at: its own generated one where generation wets it (a river's
            // mouth stands over the sea), else the open water's beside it
            let own = g.generated(c);
            for d in g.neighbours(c).into_iter().flatten().filter(|d| !self.cols.contains_key(d)) {
                let (lo, hi) = g.generated(d);
                let t = if own.0 < own.1 { own.1 } else { hi };
                if lo < hi && t > n.floor as i64 * U && (t - n.surface()).abs() >= DEAD && g.reach(d) == SEA {
                    wake.push(a);
                    break;
                }
            }
        }
        for a in wake {
            self.wake(a);
        }
    }

    /// Once a second, each reach's open water passes on what it took in (or gave out) to the reach below: a
    /// linear store, out = extra / hold, at least a unit a second so it empties. Highest account first, so the
    /// bake's reaches (numbered downstream first) mostly pass water down in one tick; any order keeps the count.
    fn run_reaches(&mut self, g: &impl Ground) {
        let accts: Vec<u32> = self.reach_extra.keys().rev().copied().collect();
        for a in accts {
            let x = self.reach_extra[&a];
            if x == 0 {
                self.reach_extra.remove(&a);
                continue;
            }
            let (down, hold) = g.down(a);
            let out = if hold <= 1.0 { x } else { x.signum() * ((x.abs() as f64 / hold) as i64).max(1) };
            *self.reach_extra.get_mut(&a).unwrap() -= out;
            self.book(down, out);
        }
        // the rain the reaches hold, by the same linear store, into the rain of the reach below
        let accts: Vec<u32> = self.rain_held.keys().rev().copied().collect();
        for a in accts {
            let x = self.rain_held[&a];
            if x == 0 {
                self.rain_held.remove(&a);
                continue;
            }
            let (down, hold) = g.down(a);
            let out = if hold <= 1.0 { x } else { x.signum() * ((x.abs() as f64 / hold) as i64).max(1) };
            *self.rain_held.get_mut(&a).unwrap() -= out;
            if down == SEA {
                self.sea_extra += out;
            } else {
                *self.rain_held.entry(down).or_default() += out;
            }
        }
    }

    /// The flow reach `acct` carries over its generated (dry-weather) one, units a second: the rain it holds
    /// over the time it holds it (a wheel's read: one lookup).
    pub fn reach_flow(&self, g: &impl Ground, acct: u32) -> f64 {
        let x = self.rain_held.get(&acct).copied().unwrap_or(0).max(0) as f64;
        x / g.down(acct).1.max(1.0)
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
                // (only water: another fluid is never laid at the sea's level nor given to it)
                if !n.rim && n.ceil == OPEN && n.kind == crate::fluids::WATER {
                    for d in g.neighbours(n.col).into_iter().flatten().filter(|d| !self.cols.contains_key(d)) {
                        let (lo, hi) = g.generated(d);
                        if lo < hi && n.surface() > lo.max(n.floor as i64 * U) {
                            sea = Some((hi, d));
                        }
                    }
                }
                let h = n.head();
                (lo_h, hi_h) = (lo_h.min(h), hi_h.max(h));
                for l in &n.links {
                    calm &= (l.v as i64) < VSLOW;
                    let b = &self.nodes[l.to as usize];
                    let (lo, hi) = window(n, b);
                    if n.surface().min(hi) > lo && b.surface().min(hi) > lo && b.kind == n.kind && !seen.contains_key(&l.to) {
                        seen.insert(l.to, ());
                        body.push(l.to);
                    }
                }
                calm &= hi_h - lo_h < SETTLE_DH && body.len() <= SETTLE_MAX;
            }
            if !calm || body.len() < 2 || sea.is_some_and(|(l, _)| (lo_h - l).abs().max((hi_h - l).abs()) >= SETTLE_DH) {
                continue;
            }
            body.sort_unstable();
            let sea = sea.map(|(l, d)| (l, river_of(&mut self.river_at, g, d).0));
            self.flatten(&body, sea);
            if sea.is_some() {
                // columns back to what generation gives, wet or the dry shore around them, are not stored
                for &a in &body {
                    let c = self.nodes[a as usize].col;
                    forget.push(c);
                    forget.extend(g.neighbours(c).into_iter().flatten());
                }
            }
        }
        let flat: FxMap<u32, ()> = seen;
        self.awake.retain(|a| self.nodes[*a as usize].quiet != u16::MAX || !flat.contains_key(a));
        self.forget(g, forget);
    }

    /// The edge of the stored water against open water that is not stored (the sea, a lake, a river reach).
    /// At the start of a step (`start`): moving water may reach any neighbour, so those are stored before
    /// anything moves, and an open river upstream pours in its flow. At its end: an open end that holds a level
    /// is put back to it, the difference to its account.
    fn open_ends(&mut self, g: &impl Ground, start: bool) {
        // moving water may reach any neighbour, so those are stored before anything moves; open sea next to
        // water at sea level is the exception: that water is held at sea level and trades with the sea
        let mut cols: Vec<Col> = self.awake.iter().filter(|&&a| !self.nodes[a as usize].rim).map(|&a| self.nodes[a as usize].col).collect();
        cols.sort_unstable();
        cols.dedup();
        for c in cols {
            let open = self.cols[&c].iter().copied().find(|&a| self.nodes[a as usize].ceil == OPEN);
            let own = *self.gen_at.entry(c).or_insert_with(|| g.generated(c));
            let mut sea = None;
            let mut grow: Vec<Col> = Vec::new();
            let mut feed: Vec<(i64, u32)> = Vec::new();
            let mut rainfed: Vec<(i64, u32)> = Vec::new();
            for d in g.neighbours(c).into_iter().flatten().filter(|d| !self.cols.contains_key(d)) {
                let (lo, hi) = *self.gen_at.entry(d).or_insert_with(|| g.generated(d));
                match open.map(|a| &self.nodes[a as usize]) {
                    // (only water trades with open water; another fluid stores the column, and meets it as a wall)
                    Some(n) if lo < hi && n.floor as i64 * U < hi && n.kind == crate::fluids::WATER => {
                        // held at its own generated level (a river slopes), or the open water's where generation
                        // left this column dry (dug); open water downstream takes whatever arrives and gives back
                        // what is missing, open water level or upstream only while the surface is near it
                        let t = if own.0 < own.1 { own.1 } else { hi };
                        if hi > t {
                            // upstream: the open river pours in its steady flow whatever this column holds, until
                            // the water here stands over its level (a dam's pond: the patch grows upstream)
                            if n.surface() > t + SEA_DH {
                                grow.push(d);
                            } else if let (acct, Some((q, grad, _))) = river_of(&mut self.river_at, g, d) {
                                // its share of the reach's flow: all of it straight downstream, none across; the
                                // dry-weather base from the reach's account, and the rain's flow on top from its rain
                                let share = q * ((hi - t) as f64 / grad.max(1.0)).min(1.0);
                                feed.push((share.round() as i64, acct));
                                let base = g.base(acct);
                                if base > 0.0 {
                                    // up, so light rain on a small river is poured too (capped by what the reach holds)
                                    rainfed.push(((share * self.reach_flow(g, acct) / base).ceil() as i64, acct));
                                }
                            }
                        } else if hi < t || (n.surface() - t).abs() < SEA_DH {
                            sea = Some((t, river_of(&mut self.river_at, g, d).0));
                        } else {
                            grow.push(d);
                        }
                    }
                    // a dry column over the open water beside it: nothing can pass between them
                    Some(n) if n.water == 0 && lo < hi && n.floor as i64 * U >= hi => {}
                    _ => grow.push(d),
                }
            }
            if !start {
                // after the flow: the open ends that hold a level put it back exactly (what is sent and saved)
                if let (Some(a), Some((hi, acct))) = (open, sea) {
                    let n = &mut self.nodes[a as usize];
                    let want = hi - n.floor as i64 * U;
                    let d = n.water - want;
                    n.water = want;
                    self.book(acct, d);
                    let n = &self.nodes[a as usize];
                    // the open node may sleep while another node of its column moves: no step will send this
                    if n.quiet == u16::MAX && n.water != n.sent {
                        self.dirty.insert(c, ());
                    }
                }
                continue;
            }
            for d in grow {
                self.materialize(g, d);
            }
            if let Some(a) = open {
                for (q, acct) in feed {
                    self.nodes[a as usize].water += q;
                    self.book(acct, -q);
                }
                for (q, acct) in rainfed {
                    let held = self.rain_held.get_mut(&acct).map_or(0, |x| *x);
                    let q = q.min(held).max(0);
                    if q > 0 {
                        self.nodes[a as usize].water += q;
                        *self.rain_held.get_mut(&acct).unwrap() -= q;
                    }
                }
            }
            if sea.is_none() && self.cols[&c].len() > 0 && g.neighbours(c).into_iter().flatten().all(|d| self.cols.contains_key(&d)) {
                for &a in &self.cols[&c] {
                    self.nodes[a as usize].rim = true;
                }
            }
        }
    }

    /// A body of water whose every column is within the dead band of what generation gives it is forgotten,
    /// moving or not: a river's patch flows on in its open water once it matches it. The difference goes to
    /// each column's account (`Ground::reach`), so the count holds.
    fn forget_matching(&mut self, g: &impl Ground) {
        let mut seen: FxMap<u32, ()> = FxMap::default();
        let mut starts = self.awake.clone();
        starts.append(&mut self.slept);
        let mut gone: Vec<Col> = Vec::new();
        // columns whose ground was changed under generated water (a pit dug in a river's bed): generation
        // cannot describe them, so they stay stored, asleep, once the water around them is forgotten
        let mut keep: Vec<u32> = Vec::new();
        for a in starts {
            // a slept node may have been removed since (its index free or reused)
            let live = self.cols.get(&self.nodes[a as usize].col).is_some_and(|ids| ids.contains(&a));
            if !live || seen.contains_key(&a) || self.nodes[a as usize].water == 0 {
                continue;
            }
            let mut body = vec![a];
            seen.insert(a, ());
            let (mut k, mut ok, mut open, mut kept) = (0, true, false, Vec::new());
            // the whole body, even once it fails: a part left unseen would pass as a body of its own
            while k < body.len() {
                let n = &self.nodes[body[k] as usize];
                k += 1;
                let c = n.col;
                let (lo, hi) = *self.gen_at.entry(c).or_insert_with(|| g.generated(c));
                let n = &self.nodes[body[k - 1] as usize];
                let dug = lo < hi && n.floor as i64 * U < lo;
                let want = if lo < hi && (n.floor as i64 * U == lo || dug) { hi - n.floor as i64 * U } else { 0 };
                ok &= self.cols[&c].len() == 1 && n.ceil == OPEN && n.kind == crate::fluids::WATER && (n.water - want).abs() < DEAD;
                if dug {
                    kept.push(body[k - 1]);
                }
                open |= !n.rim;
                for l in &n.links {
                    let b = &self.nodes[l.to as usize];
                    if b.water > 0 && b.kind == n.kind && !seen.contains_key(&l.to) {
                        seen.insert(l.to, ());
                        body.push(l.to);
                    }
                }
            }
            if !ok || !open {
                continue;
            }
            keep.extend(kept);
            for &a in &body {
                let c = self.nodes[a as usize].col;
                gone.push(c);
                // its dry shore too: the banks stored around it, linked or not
                gone.extend(g.neighbours(c).into_iter().flatten());
            }
        }
        self.forget(g, gone);
        for a in keep {
            let n = &mut self.nodes[a as usize];
            n.quiet = u16::MAX;
            for l in &mut n.links {
                (l.v, l.out, l.frac, l.fin, l.vin) = (0, 0, 0, 0, 0);
            }
            if n.water != n.sent {
                let c = n.col;
                self.dirty.insert(c, ());
            }
        }
        let quiet: Vec<u32> = self.awake.iter().copied().filter(|&a| self.nodes[a as usize].quiet == u16::MAX).collect();
        if !quiet.is_empty() {
            self.awake.retain(|a| !quiet.contains(a));
        }
    }

    /// Columns back to what generation gives (within the dead band) are not stored: the difference goes to
    /// each one's account. A dry one forgotten offers its neighbours too, so a patch's whole dry shore goes.
    fn forget(&mut self, g: &impl Ground, mut todo: Vec<Col>) {
        let mut tried: FxMap<Col, ()> = FxMap::default();
        while let Some(c) = todo.pop() {
            if tried.insert(c, ()).is_some() {
                continue;
            }
            let Some(ids) = self.cols.get(&c) else { continue };
            let [a] = ids[..] else { continue };
            let n = &self.nodes[a as usize];
            let (lo, hi) = g.generated(c);
            let want = if lo < hi && n.floor as i64 * U == lo { hi - lo } else { 0 };
            if n.ceil != OPEN || n.kind != crate::fluids::WATER || (n.water - want).abs() >= DEAD || (want == 0 && lo < hi) || g.gaps(c) != [(n.floor, OPEN)] {
                continue;
            }
            let w = n.water;
            self.remove(c, false);
            let rec = self.gen.remove(&c).unwrap_or(0);
            self.gen_total -= rec;
            self.book(g.reach(c), w - rec);
            self.dirty.insert(c, ());
            if want == 0 {
                todo.extend(g.neighbours(c).into_iter().flatten().filter(|d| !tried.contains_key(d)));
            }
        }
    }

    /// Gives every node of `body` the water that puts all of them at one level, and puts them to sleep: the
    /// level its water reaches, or `sea` (in units) with the difference given to the open sea.
    fn flatten(&mut self, body: &[u32], sea: Option<(i64, u32)>) {
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
        if let Some((l, _)) = sea {
            lo = l;
        }
        while sea.is_none() && hi - lo > 1 {
            let mid = lo + (hi - lo) / 2;
            let need: i64 = body.iter().map(|&a| w(&self.nodes[a as usize], mid)).sum();
            if need <= total { lo = mid } else { hi = mid }
        }
        let mut left = total - body.iter().map(|&a| w(&self.nodes[a as usize], lo)).sum::<i64>();
        if let Some((_, acct)) = sea {
            self.book(acct, left);
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
        /// `Ground::tide_epoch`: a test that moves the sea's level moves this with it.
        pub epoch: i64,
        /// `Ground::g`, every column's.
        pub g: f64,
    }

    impl Box3 {
        pub fn new(nx: i32, ny: i32, nz: i32) -> Box3 {
            Box3 { nx, ny, nz, solid: vec![false; (nx * ny * nz) as usize], sea: None, epoch: 0, g: EARTH_G }
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
        fn g(&self, _body: u8) -> f64 {
            self.g
        }
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
        fn tide_epoch(&self) -> i64 {
            self.epoch
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

    /// A river in a straight channel along +x (`prototypes/river-reach`): banks at y < 2 and y >= 2 + W, a bed
    /// falling `slope` blocks a column in whole-block steps under a smooth surface `depth` over it, a drop of
    /// `fall` blocks at x = nx * 3 / 4 (a waterfall: the reach below is account 1), and a flow that grows from
    /// `Q0` units a second at x = 0 to twice that at the far end. `edits` are solid (true) or air cells.
    pub struct Channel {
        pub nx: i32,
        pub slope: f64,
        pub depth: f64,
        pub fall: f64,
        pub edits: FxMap<(i32, i32, i32), bool>,
    }
    pub const W: i32 = 6;
    pub const B0: f64 = 40.0;
    pub const Q0: f64 = 4.0 * U as f64;

    impl Channel {
        pub fn new(nx: i32, slope: f64, depth: f64, fall: f64) -> Channel {
            Channel { nx, slope, depth, fall, edits: FxMap::default() }
        }
        fn xf(&self) -> i32 {
            self.nx * 3 / 4
        }
        pub fn bed(&self, x: i32) -> f64 {
            B0 - self.slope * x as f64 - if x >= self.xf() { self.fall } else { 0.0 }
        }
        pub fn gen_floor(&self, x: i32, y: i32) -> i32 {
            if (2..2 + W).contains(&y) { self.bed(x).floor() as i32 } else { (B0 + 6.0) as i32 }
        }
        /// The generated level, units: smooth, as a river's is.
        pub fn level(&self, x: i32) -> i64 {
            ((self.bed(x) + self.depth) * U as f64) as i64
        }
        fn solid(&self, x: i32, y: i32, z: i32) -> bool {
            self.edits.get(&(x, y, z)).copied().unwrap_or(z < self.gen_floor(x, y))
        }
        /// Cells x, y, z0..z1 made solid (or air), the water told.
        pub fn set(&mut self, w: &mut Liquids, x: i32, y: i32, z: std::ops::Range<i32>, solid: bool) {
            for z in z {
                self.edits.insert((x, y, z), solid);
            }
            w.regap(self, col(x, y));
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
            [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)].map(|(a, b)| ((0..self.nx).contains(&a) && (0..W + 4).contains(&b)).then(|| Col::new(0, 0, a, b)))
        }
        fn reach(&self, c: Col) -> u32 {
            (c.i() >= self.xf()) as u32
        }
        fn down(&self, acct: u32) -> (u32, f64) {
            if acct == 0 { (1, 10.0) } else { (SEA, 5.0) }
        }
        fn base(&self, _acct: u32) -> f64 {
            Q0
        }
        fn river(&self, c: Col) -> Option<(f64, f64, f64)> {
            let (lo, hi) = self.generated(c);
            let q = Q0 / 60.0 * (1.0 + c.i() as f64 / self.nx as f64);
            (lo < hi).then(|| (q / W as f64, self.slope * U as f64, Q0 / 60.0 / self.nx as f64 / W as f64))
        }
    }

    /// The largest |live - generated| over the stored river columns, in blocks.
    pub fn off(g: &Channel, w: &Liquids) -> f64 {
        let mut worst = 0i64;
        for (c, gaps) in w.all() {
            let (lo, hi) = g.generated(c);
            if lo < hi && gaps.len() == 1 && gaps[0].0 as i64 * U == lo {
                worst = worst.max((lo + gaps[0].2 - hi).abs());
            }
        }
        worst as f64 / U as f64
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

    /// A dam breaks in a 120 m channel: 4 m of water rushes onto a dry floor. Ritter's solution gives a front
    /// at 2 sqrt(g h) = 12.5 m/s; a grid model is slower at the very tip. Then it all settles level and asleep
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
        let ran = (front(&w) - 39) as f64 * crate::world::BLOCK;
        assert!((6.0..14.0).contains(&ran), "the front ran {ran} m in 1 s");
        // the bore runs to the far wall, comes back and dies down
        for _ in 0..60 * 150 {
            w.step(&g);
        }
        assert_eq!(w.total(), t, "not a unit appears or vanishes");
        assert_eq!(w.awake(), 0, "settles and sleeps");
        let (lo, hi) = (0..120).map(|x| level(&w, x, 0)).fold((f64::MAX, f64::MIN), |(a, b), l| (a.min(l), b.max(l)));
        assert!(hi - lo < 0.02, "levels {lo}..{hi}");
    }

    /// Water by its body's pull (`Ground::g`): the same dam break under Terra's 19.62 and Callisto's 1.85 m/s^2
    /// runs its front 10 m out in times whose ratio is the root of the pulls' (3.26), as Ritter's 2 sqrt(g h)
    /// (3.45 measured). Further out the fixed friction a step costs the slower front more: 3.97 at 20 m.
    #[test]
    fn a_dam_break_runs_slower_under_a_weaker_pull() {
        let reach = |pull: f64| {
            let mut g = Box3::new(120, 1, 8);
            g.g = pull;
            g.fill(0..120, 0..1, 0..1, true);
            let mut w = stored(&g);
            for x in 0..40 {
                w.pour(&g, col(x, 0), 1, 4 * U);
            }
            (1..).find(|_| {
                w.step(&g);
                w.column(col(49, 0)).unwrap().last().unwrap().water >= U / 16
            })
            .unwrap() as f64
        };
        let (fast, slow) = (reach(19.62), reach(1.85));
        let want = (19.62f64 / 1.85).sqrt();
        assert!((slow / fast / want - 1.0).abs() < 0.15, "10 m in {fast} steps under 19.62, {slow} under 1.85: {} (Ritter {want})", slow / fast);
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
        // (80 m of basin: a wave crosses it in about 20 s)
        for _ in 0..60 * 180 {
            w.step(&g);
        }
        let (lo, hi) = (0..80).map(|x| level(&w, x, 1)).fold((f64::MAX, f64::MIN), |(a, b), l| (a.min(l), b.max(l)));
        assert!(hi - lo < 0.01, "levels {lo}..{hi}");
        assert_eq!(w.awake(), 0);
    }

    /// A tank 8 m deep empties through a hole in its floor about as fast as Torricelli says
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
        // a 1 m hole under 8 m of water passes about 1 m^2 x 12.5 m/s; in blocks per second:
        let rate = (start - tank(&w)) as f64 / U as f64;
        assert!((10.0..80.0).contains(&rate), "{rate} blocks in the first second");
        // (Torricelli empties these 200 m^3 in about 32 s)
        for _ in 0..60 * 60 {
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

    /// The tide (ADR 0067): a harbour dug behind the shore, open to the sea through a channel 2 wide, fills as
    /// the sea rises and empties as it falls, a metre either way, every unit counted every step.
    #[test]
    fn a_dug_harbour_follows_the_tide_in_and_out() {
        let mut g = Box3::new(40, 30, 12);
        g.fill(0..40, 0..30, 0..2, true);
        g.fill(20..40, 0..30, 2..8, true); // land up to layer 8, sea over the rest at layer 6
        g.with_sea(6 * U);
        let mut w = Liquids::default();
        let mut dug = Vec::new();
        for x in 20..36 {
            for y in 10..12 {
                dug.push((x, y));
            }
        }
        for x in 26..36 {
            for y in 6..16 {
                dug.push((x, y));
            }
        }
        dug.sort_unstable();
        dug.dedup();
        for &(x, y) in &dug {
            g.fill(x..x + 1, y..y + 1, 4..8, false);
            w.regap(&g, col(x, y));
        }
        // the sea a metre from low to high and back over 480 s, stepped once a second as `world::Solid` steps it
        let sea = |s: i64| 6 * U + (U as f64 / 2.0 * (std::f64::consts::TAU * s as f64 / 480.0).sin()) as i64;
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for s in 0..960 {
            g.sea.as_mut().unwrap().0 = sea(s);
            g.epoch = s;
            for k in 0..60 {
                w.step(&g);
                assert_eq!(w.counted(), 0, "the count at {s} s, step {k}");
            }
            if s >= 480 {
                let l = level(&w, 33, 13);
                (lo, hi) = (lo.min(l), hi.max(l));
            }
        }
        assert!(hi > 6.45 && lo < 5.55, "the harbour from {lo:.3} to {hi:.3}");
        // and it ends where the sea is
        let end = sea(959) as f64 / U as f64;
        assert!((level(&w, 33, 13) - end).abs() < 0.03, "{} against the sea's {end}", level(&w, 33, 13));
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
    /// Steps until the river's live patch is forgotten (or `cap` steps), checking the count every step: what was
    /// poured, and nothing else, whatever went to the reaches' accounts.
    fn until_forgotten(w: &mut Liquids, g: &Channel, cap: usize, poured: i64) -> Option<usize> {
        for t in 1..=cap {
            w.step(g);
            assert_eq!(w.counted(), poured, "the count at step {t}");
            if w.stored() == 0 {
                return Some(t);
            }
        }
        None
    }

    /// A bucket into a river at rest (W5, ADR 0059): the touched columns are live, the open river upstream pours
    /// in its flow, the river downstream takes what arrives, and the patch is forgotten again once it is within
    /// the dead band of generation, moving or not; the bucket's water went to the reach's account.
    #[test]
    fn a_bucket_in_a_river_is_forgotten_and_counted() {
        for slope in [0.005, 0.05, 0.1] {
            let g = Channel::new(160, slope, 1.5, 0.0);
            let mut w = Liquids::default();
            for y in 3..5 {
                assert!(w.pour(&g, col(80, y), (g.level(80) / U) as i32, U));
            }
            let t = until_forgotten(&mut w, &g, 60 * 30, 2 * U).unwrap_or_else(|| panic!("slope {slope}: {} columns still stored, {:.4} off", w.stored(), off(&g, &w)));
            assert!(t < 60 * 10, "slope {slope}: live {:.1} s", t as f64 / 60.0);
            assert_eq!(w.reach_extra.values().sum::<i64>() + w.sea_extra, 2 * U, "the bucket went to the reach, and on to the sea");
        }
    }

    /// The same across a waterfall (a reach boundary with a drop of 3 blocks): the lip passes the reach's flow at
    /// its generated depth, so a patch spanning the fall matches generation and goes too.
    #[test]
    fn a_touched_waterfall_is_forgotten() {
        let g = Channel::new(160, 0.02, 1.5, 3.0);
        let mut w = Liquids::default();
        for x in [119, 120, 121] {
            for y in 3..5 {
                w.pour(&g, col(x, y), (g.level(x) / U) as i32, U);
            }
        }
        let t = until_forgotten(&mut w, &g, 60 * 60, 6 * U).unwrap_or_else(|| panic!("{} columns still stored, {:.4} off", w.stored(), off(&g, &w)));
        assert!(t < 60 * 20, "live {:.1} s", t as f64 / 60.0);
    }

    /// A dam across the river: no rule of its own. The pond behind it rises over generation and backs up the
    /// river; the count holds every step; broken, its wave runs down and the patch is forgotten.
    #[test]
    fn a_dam_backs_the_river_up_and_its_wave_goes_when_broken() {
        let mut g = Channel::new(200, 0.02, 1.5, 0.0);
        let mut w = Liquids::default();
        let (xd, top) = (100, (g.level(100) / U) as i32 + 1);
        for y in 2..2 + W {
            let z = g.gen_floor(xd, y)..top;
            g.set(&mut w, xd, y, z, true);
        }
        for t in 0..60 * 60 {
            w.step(&g);
            assert_eq!(w.counted(), 0, "the count at step {t}");
        }
        let pond = level(&w, xd - 1, 4) - g.level(xd - 1) as f64 / U as f64;
        assert!(pond > 0.4, "the pond stands {pond:.2} blocks over the river");
        assert!(w.column(col(xd - 25, 4)).is_some(), "and backs it up");
        for y in 2..2 + W {
            let z = g.gen_floor(xd, y)..top;
            for z in z {
                g.edits.remove(&(xd, y, z));
            }
            w.regap(&g, col(xd, y));
        }
        let t = until_forgotten(&mut w, &g, 60 * 400, 0).unwrap_or_else(|| panic!("{} columns still stored, {:.4} off", w.stored(), off(&g, &w)));
        println!("the dam's wave was live {:.1} s", t as f64 / 60.0);
    }

    /// Rain raises a river's live water (W5, ADR 0059): the open river pours in its dry-weather base plus the
    /// flow of the rain its reach holds, which it uses up, so a dam's pond fills faster after rain.
    #[test]
    fn rain_fills_a_dams_pond_faster() {
        let pond = |rain: i64| {
            let mut g = Channel::new(200, 0.02, 1.5, 0.0);
            let mut w = Liquids::default();
            let (xd, top) = (100, (g.level(100) / U) as i32 + 3);
            for y in 2..2 + W {
                let z = g.gen_floor(xd, y)..top;
                g.set(&mut w, xd, y, z, true);
            }
            w.rain(0, rain);
            // 4 s: before a heavy rain's backwater reaches the test channel's upstream edge, where no river pours
            for t in 0..60 * 4 {
                w.step(&g);
                assert_eq!(w.counted(), 0, "the count at step {t}");
            }
            (w.total() - w.generated_total()) as f64 / U as f64
        };
        let (dry, wet) = (pond(0), pond((Q0 * 40.0) as i64));
        assert!(wet > dry * 1.3, "the pond holds {wet:.1} blocks after rain, {dry:.1} without");
    }

    /// And a dry spell lowers it to the base: a patch a bucket woke stands over generation while its reach
    /// holds rain, and once the rain has run off it settles to generation and is forgotten.
    #[test]
    fn rain_raises_a_live_river_and_a_dry_spell_lowers_it_to_the_base() {
        let run = |rain: i64| {
            let g = Channel::new(160, 0.02, 1.5, 0.0);
            let mut w = Liquids::default();
            for y in 3..5 {
                w.pour(&g, col(60, y), (g.level(60) / U) as i32, U);
            }
            w.rain(0, rain);
            let mut high = 0.0f64;
            for t in 0..60 * 3 {
                w.step(&g);
                assert_eq!(w.counted(), 2 * U, "the count at step {t}");
                // the water standing over generation, block-seconds: the bucket's, and what the rain raised
                high += (w.total() - w.generated_total()) as f64 / U as f64 / 60.0;
            }
            let gone = until_forgotten(&mut w, &g, 60 * 600, 2 * U);
            // the rain left runs on down its reaches to the sea, waking nothing
            for _ in 0..60 * 200 {
                w.step(&g);
            }
            assert_eq!((w.stored(), w.counted()), (0, 2 * U));
            (high, gone, w.rain_held.values().sum::<i64>())
        };
        // rain that triples the reach's flow at first (its hold is 10 s): a heavier one spreads the patch to the
        // test box's ends, where no river takes the water on
        let (dry, wet) = (run(0), run((Q0 * 20.0) as i64));
        assert!(wet.0 > dry.0 * 1.5, "the river stood {:.1} block-seconds over generation in rain, {:.1} without", wet.0, dry.0);
        assert!(wet.1.is_some(), "after the rain the patch settles to generation and is forgotten");
        assert_eq!(wet.2, 0, "the rain ran off");
    }

    /// What a reach's open water took in runs down its reaches to the sea, each holding it for its own time
    /// (a linear store); rain booked to a reach goes the same way. The count holds every step.
    #[test]
    fn a_reach_passes_what_it_took_in_down_to_the_sea() {
        let g = Channel::new(160, 0.02, 1.5, 3.0);
        let mut w = Liquids::default();
        for y in 3..5 {
            w.pour(&g, col(60, y), (g.level(60) / U) as i32, U);
        }
        w.rain(0, U);
        for t in 0..60 * 120 {
            w.step(&g);
            assert_eq!(w.counted(), 2 * U, "the count at step {t}");
        }
        assert_eq!((w.stored(), w.rained), (0, U));
        assert!(w.reach_extra.values().all(|&x| x == 0), "{:?}", w.reach_extra);
        assert_eq!(w.sea_extra, 3 * U, "the bucket and the rain reached the sea");
    }

    /// A river patch saved while it moved loads live (review of W5): it runs on and is forgotten, instead of
    /// standing frozen in the river.
    #[test]
    fn a_restored_river_patch_runs_on() {
        let g = Channel::new(160, 0.02, 1.5, 0.0);
        let mut w = Liquids::default();
        for y in 3..5 {
            w.pour(&g, col(80, y), (g.level(80) / U) as i32, U);
        }
        for _ in 0..30 {
            w.step(&g);
        }
        let mut back = Liquids::default();
        back.restore(&g, &w.all());
        assert!(back.stored() > 0);
        let poured = back.counted();
        assert!(until_forgotten(&mut back, &g, 60 * 30, poured).is_some(), "{} columns still stored", back.stored());
    }

        /// A pit dug in the river's bed (review of W5): the river around it is forgotten again and the pit, which
    /// generation cannot describe, stays stored but asleep at the river's level; nothing keeps working.
    #[test]
    fn a_pit_dug_in_a_river_bed_falls_asleep() {
        let mut g = Channel::new(160, 0.02, 1.5, 0.0);
        let mut w = Liquids::default();
        let z = g.gen_floor(80, 4);
        g.set(&mut w, 80, 4, z - 2..z, false);
        for t in 0..60 * 60 {
            w.step(&g);
            assert_eq!(w.counted(), 0, "the count at step {t}");
        }
        let s0 = w.stepped;
        for _ in 0..60 * 10 {
            w.step(&g);
        }
        assert_eq!((w.stored(), w.awake(), w.stepped - s0), (1, 0, 0));
        assert!((level(&w, 80, 4) - g.level(80) as f64 / U as f64).abs() < 0.02, "the pit stands at the river's level");
    }

        /// A pillar standing in the river (a bridge's) keeps nothing live but its own column, which sleeps.
    #[test]
    fn a_pillar_in_a_river_keeps_nothing_awake() {
        let mut g = Channel::new(160, 0.02, 1.5, 0.0);
        let mut w = Liquids::default();
        let z = g.gen_floor(80, 4)..(g.level(80) / U) as i32 + 2;
        g.set(&mut w, 80, 4, z, true);
        for _ in 0..60 * 30 {
            w.step(&g);
        }
        let s0 = w.stepped;
        for _ in 0..60 * 10 {
            w.step(&g);
        }
        assert_eq!((w.stored(), w.awake(), w.stepped - s0), (1, 0, 0));
    }

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

    const LAVA: u8 = 1;

    /// Steps until a dam break of 4 m of fluid `kind` (viscosity `visc`, None: its row's) has run its front 10 m
    /// out, and until 10 blocks of it have passed the dam.
    fn dam_break(kind: u8, visc: Option<i64>) -> (f64, f64) {
        let run = |done: &dyn Fn(&Liquids) -> bool| {
            let mut g = Box3::new(120, 1, 8);
            g.fill(0..120, 0..1, 0..1, true);
            let mut w = stored(&g);
            if let Some(v) = visc {
                w.set_viscosity(kind, v);
            }
            for x in 0..40 {
                w.pour_kind(&g, col(x, 0), 1, 4 * U, kind);
            }
            (1..60 * 600)
                .find(|_| {
                    w.step(&g);
                    done(&w)
                })
                .expect("it gets there") as f64
        };
        let top = |w: &Liquids, x: i32| w.column(col(x, 0)).unwrap().last().unwrap().water;
        (run(&|w| top(w, 49) >= U / 16), run(&|w| (40..120).map(|x| top(w, x)).sum::<i64>() >= 10 * U))
    }

    /// Lava flows by its row (viscosity 64: its link loses 64 times water's friction a step). That thick, its dam
    /// break is overdamped (friction leads, inertia hardly counts), and the rule predicts the time its flow takes
    /// grows linearly with the viscosity: half as thick, half as long (measured 2.08 for the 10 blocks past the dam).
    /// (Past about 128 the stop speed `VSTOP` holds the slowest links still and it grows faster.) Against water,
    /// whose front runs at Ritter's inertial speed, its front is 12 times slower (848 steps for 10 m against 69).
    #[test]
    fn a_dam_break_of_lava_runs_slower_by_its_viscosity() {
        let (water, _) = dam_break(crate::fluids::WATER, None);
        let (lava, lava_flow) = dam_break(LAVA, None);
        let (_, half_flow) = dam_break(LAVA, Some(32));
        println!("{{\"water_front\":{water},\"lava_front\":{lava},\"lava_flow\":{lava_flow},\"visc32_flow\":{half_flow}}}");
        assert!(lava > 8.0 * water, "lava's front {lava} steps against water's {water}: visibly slower");
        assert!((lava_flow / half_flow / 2.0 - 1.0).abs() < 0.1, "viscosity 64 against 32: {} (2 overdamped)", lava_flow / half_flow);
    }

    /// Water and lava poured side by side meet and never mix: each fluid's ledger is exact after 1,000 steps,
    /// and every gap holds one kind.
    #[test]
    fn water_and_lava_side_by_side_do_not_mix_and_each_ledger_is_exact() {
        let mut g = Box3::new(20, 3, 6);
        g.fill(0..20, 0..3, 0..1, true);
        let mut w = stored(&g);
        let (mut wet, mut hot) = (0, 0);
        for y in 0..3 {
            for x in 0..10 {
                assert!(w.pour_kind(&g, col(x, y), 1, 3 * U, crate::fluids::WATER));
                wet += 3 * U;
            }
            for x in 10..20 {
                assert!(w.pour_kind(&g, col(x, y), 1, 2 * U, LAVA));
                hot += 2 * U;
            }
        }
        // and a pour onto the other fluid is refused
        assert!(!w.pour_kind(&g, col(12, 1), 1, U, crate::fluids::WATER), "water is not poured into lava");
        for _ in 0..1000 {
            w.step(&g);
            assert_eq!((w.counted_of(crate::fluids::WATER), w.counted_of(LAVA)), (wet, hot));
        }
        // the water ran no further than the wall it met, and the lava kept its side
        assert!((0..3).all(|y| w.column(col(9, y)).unwrap().last().unwrap().kind == crate::fluids::WATER));
        assert!((0..3).all(|y| w.column(col(10, y)).unwrap().last().unwrap().kind == LAVA));
    }

    /// Two fluids flowing into one empty gap in the same step: it takes one, the other stays where it was.
    #[test]
    fn two_fluids_into_one_dry_gap_keep_their_ledgers() {
        let mut g = Box3::new(3, 1, 6);
        g.fill(0..3, 0..1, 0..1, true);
        let mut w = stored(&g);
        w.pour_kind(&g, col(0, 0), 1, 3 * U, crate::fluids::WATER);
        w.pour_kind(&g, col(2, 0), 1, 3 * U, LAVA);
        for _ in 0..1000 {
            w.step(&g);
            assert_eq!((w.counted_of(crate::fluids::WATER), w.counted_of(LAVA)), (3 * U, 3 * U));
        }
    }

    /// The kind rides in a gap's units on the wire and in saves: water's bytes are what they were, lava's come back.
    #[test]
    fn lava_round_trips_through_its_columns_and_water_packs_as_before() {
        assert_eq!(pack(crate::fluids::WATER, 12345), 12345);
        assert_eq!(unpack(pack(LAVA, 12345)), (LAVA, 12345));
        let mut g = Box3::new(6, 1, 6);
        g.fill(0..6, 0..1, 0..1, true);
        let mut w = stored(&g);
        w.pour_kind(&g, col(1, 0), 1, 2 * U, LAVA);
        w.pour_kind(&g, col(5, 0), 1, 2 * U, crate::fluids::WATER);
        for _ in 0..30 {
            w.step(&g);
        }
        let all = w.all();
        let mut back = Liquids::default();
        back.restore(&g, &all);
        assert_eq!((back.all(), back.counted_of(LAVA)), (all.clone(), w.counted_of(LAVA)));
        // a client's copy from the tiles it is sent
        let mut client = Liquids::default();
        for t in w.wet_tiles(0, 0, 0, 0, 16) {
            client.set_tile(t, &w.tile_sent(t));
        }
        assert!(client.column(col(1, 0)).unwrap().any(|n| n.kind == LAVA));
    }

    /// The step's cost with 10,000 stored columns of water, all awake (a 100 x 100 basin whose half is 2 m
    /// higher): `cargo test -p engine --release step_cost -- --ignored --nocapture` prints ms a step.
    #[test]
    #[ignore]
    fn step_cost() {
        let mut g = Box3::new(100, 100, 8);
        g.fill(0..100, 0..100, 0..1, true);
        let mut w = stored(&g);
        for y in 0..100 {
            for x in 0..100 {
                w.pour(&g, col(x, y), 1, if x < 50 { 4 * U } else { 2 * U });
            }
        }
        for _ in 0..10 {
            w.step(&g);
        }
        let (t, n) = (std::time::Instant::now(), 300);
        for _ in 0..n {
            w.step(&g);
        }
        let ms = t.elapsed().as_secs_f64() * 1000.0 / n as f64;
        println!("{{\"liquid_step_ms\":{ms:.3},\"stored\":{},\"awake\":{}}}", w.stored(), w.awake());
    }
}
