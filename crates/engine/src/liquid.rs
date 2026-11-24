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
use crate::interior::{HEAT_PACE, LIGHT};
use crate::world::FxMap;

pub mod phases;

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
/// (A gap without a's fluid is dry ground to it: oil spreads over water as over a floor.)
fn spreads(a: &Node, b: &Node) -> bool {
    b.floor < a.floor || a.water >= if b.water == 0 || b.kind != a.kind { 2 * MIN } else { MIN }
}
/// Layers of fluid a gap may hold (liquids.md "Mixing in the world": the first guess). At the cap a pour of a new
/// fluid is refused, a flow of one passes nothing, and what an edit joins past it is pushed out and booked.
pub const LAYERS: usize = 4;
/// Steps a node stays awake after it last moved.
const QUIET: u16 = 30;
/// A full gap under a ceiling holds more water under pressure: one block of head per 1/K block of excess.
/// Without it a closed tunnel could not pass on the pressure of the lake it leads from.
pub const K: i64 = 16;

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
    /// The fluids other than water generation puts in column `c` (a live volcano's lava, `Body::lava_col`), as a
    /// column is built from them: (bottom, top, units held under pressure, kind), units, bottom-up; each lies in one
    /// of the gaps generation leaves, and every one's head is its body's level, so untouched it lies still.
    fn generated_other(&self, _c: Col) -> Vec<(i64, i64, i64, u8)> {
        Vec::new()
    }
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
    /// The heat a fluid at layer `z` of column `c` rests at, C: what the air and rock round it hold (`Body::heat`),
    /// and in a place generation fills with a fluid other than water, the heat that keeps it so (a live volcano's).
    fn heat(&self, _c: Col, _z: f64) -> f32 {
        AMBIENT
    }
    /// Whether solid cell `z` of column `c` is held at its `heat` (the planet's own ground, not ice): what gives water
    /// beside it held heat (`phases`).
    fn held(&self, c: Col, z: i32) -> bool {
        !self.ice(c, z)
    }
    /// The air at layer `z` of column `c`: its temperature, C, and its pressure, Pa (0: no air). What water's phases
    /// go by (`phases`): the weather's.
    fn air(&self, _c: Col, _z: f64) -> (f64, f64) {
        (AMBIENT as f64, phases::ATM)
    }
    /// Whether cell `z` of column `c` holds water's solid (its row's `freeze` block, ice).
    fn ice(&self, _c: Col, _z: i32) -> bool {
        false
    }
}

/// `Ground::heat` where nothing says otherwise (a test box, the noise land without a map's climate), C.
pub const AMBIENT: f32 = 15.0;

/// A stored node's heat (liquids.md "Phases"; lava basics, draft decision): held only for a fluid with a `heat` row
/// whose node is off its rest (`Ground::heat`); a node without one is at rest. Not on the wire; the save keeps it.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Therm {
    /// Its temperature, C.
    pub t: f64,
    /// Seconds since it came to lie on the rock under it: the rock's own warming slows what it takes.
    pub age: f64,
    /// Units frozen out of it short of a whole one, and whole units frozen short of a block (`cool`).
    pub frac: f64,
    pub solid: i64,
}

/// A node within this of its rest heat, liquid, is at rest (`Liquids::cool`), K.
const REST_K: f64 = 0.5;
/// The rock's thermal effusivity sqrt(k rho c), W s^0.5 / (m^2 K): basalt and granite, k 2.0 W/(m K), rho 2700
/// kg/m3, c 900 J/(kg K) (Turcotte and Schubert, Geodynamics, table 4.1 and 4.2's ranges).
/// shortcut: one rock for every block; from the rock rows once they carry their heat (liquids.md step 5).
/// The real one: `effusivity` is the world's (k times `HEAT_PACE`, its heat capacity over `LIGHT`).
const ROCK_EFFUSIVITY: f64 = 2200.0;
/// The world's rock effusivity, sqrt(HEAT_PACE k rho c / LIGHT).
fn effusivity() -> f64 {
    ROCK_EFFUSIVITY * (HEAT_PACE / LIGHT).sqrt()
}
/// W through one block face from a fluid at t, C, (conductivity `conduct`) into an ice block melting at tf: through
/// half a block of the fluid, at the pace. `thaw` melts by it and `loss` takes it from the fluid, so both sides agree.
fn ice_face(t: f64, tf: f64, conduct: f32) -> f64 {
    (t - tf).max(0.0) / (0.5 / (conduct as f64 * HEAT_PACE))
}
/// Stefan-Boltzmann, W/(m^2 K^4).
const SIGMA: f64 = 5.670374e-8;
/// Free convection over a hot surface in still air, W/(m^2 K) (a horizontal plate's, 5 to 25; the mean of the
/// low end, no wind).
const H_AIR: f64 = 10.0;

/// What a body at `t` C gives a second through `r_in` m^2 K/W of itself (the world's, its conductivity paced) and
/// then from its surface of emissivity `e` to air at `ta` C by radiation and convection, both at `HEAT_PACE`, W/m^2:
/// the surface's heat found where the two flows meet (the real one: every flow carries the same factor).
fn surface(t: f64, ta: f64, r_in: f64, e: f64) -> f64 {
    let k4 = |c: f64| (c + 273.15).powi(4);
    let (mut lo, mut hi) = (t.min(ta), t.max(ta));
    for _ in 0..48 {
        let s = 0.5 * (lo + hi);
        // through the body minus off the surface: falls as the surface warms
        if (t - s) / r_in - HEAT_PACE * (e * SIGMA * (k4(s) - k4(ta)) + H_AIR * (s - ta)) > 0.0 {
            lo = s;
        } else {
            hi = s;
        }
    }
    (t - 0.5 * (lo + hi)) / r_in
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
    /// The fluid it holds (`data/fluids.toml`'s row; 0 water, and 0 when empty).
    pub kind: u8,
    /// Its place in its gap's stack of layers (liquids.md "Mixing in the world"; `restack`): the units of the layers
    /// under it and over it, the weight of those over it in units of its own fluid, the layers the gap holds and which
    /// this is, bottom-up. A gap of one fluid is one node: 0, 0, 0, 1, 0, and every rule is the old one to the bit.
    pub base: i64,
    pub above: i64,
    press: i64,
    pub stack: u8,
    pub layer: u8,
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
    /// Where its fluid's top is, in units.
    pub fn surface(&self) -> i64 {
        self.floor as i64 * U + (self.base + self.water).min(self.cap())
    }
    /// Where its fluid's bottom is, in units (its gap's floor, or the top of the layer under it).
    pub fn bottom(&self) -> i64 {
        self.floor as i64 * U + self.base.min(self.cap())
    }
    /// All its gap holds, units.
    fn fill(&self) -> i64 {
        self.base + self.water + self.above
    }
    /// The height its fluid flows from: its surface pressed by the layers over it, and under a ceiling the pressure
    /// of what the gap holds past it.
    fn head(&self) -> i64 {
        let over = self.fill() - self.cap();
        self.surface() + self.press + if over > 0 { over * K } else { 0 }
    }
}

/// The head `a`'s fluid meets across its link into `b`, if that link carries it (liquids.md "Mixing in the world"):
/// into b's own fluid or a dry gap, as ever; into a gap that holds other fluids and none of a's, through the link to
/// its bottom layer only, at the level a's fluid would lie there by density, pressed by the lighter layers over it.
/// None: another link carries it (a layer of a's fluid there), or the gap holds its most layers (`dens`: each kind's
/// density, `most`: the cap).
fn carry(nodes: &[Node], cols: &FxMap<Col, Vec<u32>>, dens: &[i64], most: usize, a: &Node, b: &Node) -> Option<i64> {
    if b.water == 0 || b.kind == a.kind {
        return Some(b.head());
    }
    if b.layer != 0 {
        return None;
    }
    let gap = || cols[&b.col].iter().map(|&i| &nodes[i as usize]).filter(|n| n.floor == b.floor);
    if gap().count() >= most || gap().any(|n| n.kind == a.kind && n.water > 0) {
        return None;
    }
    let d = |k: u8| dens[(k as usize).min(dens.len() - 1)];
    let (da, mut under, mut press) = (d(a.kind), 0i64, 0i64);
    for n in gap() {
        if d(n.kind) >= da {
            under += n.water;
        } else {
            press += (n.water as i128 * d(n.kind) as i128 / da as i128) as i64;
        }
    }
    Some(b.floor as i64 * U + under.min(b.cap()) + press)
}

/// Each kind's density, g/m3: what layers sort and press by.
fn densities() -> Vec<i64> {
    crate::fluids::FLUIDS.rows.iter().map(|r| (r.density as f64 * 1000.0) as i64).collect()
}

/// Units a step a layer of density `heavy` sinks through one of `light` and viscosity `visc` under it, under a pull of
/// `g`: a Rayleigh-Taylor overturn's fingers fall at about sqrt(A g l) (A the Atwood number, l their size, a block),
/// slowed by the lighter fluid's friction. shortcut: one speed whatever the depths, fingers a block wide; a two-phase
/// rate (drop size, Stokes) if a game rule ever hangs on the time.
fn sink_rate(light: f64, heavy: f64, visc: i64, g: f64) -> i64 {
    let a = (heavy - light) / (heavy + light);
    ((a * g * crate::world::BLOCK).sqrt() / visc as f64 / 60.0 / crate::world::BLOCK * U as f64).max(1.0) as i64
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

/// A column's water as the wire and saves carry it: (floor, ceil, water) per gap, and a gap of several layers once a
/// layer, bottom-up (liquids.md step 4: a gap of one fluid is the one entry it always was). Empty: back to generated.
pub type ColWater = (Col, Vec<(i32, i32, i64)>);

/// A column's layers, (floor, ceil, (kind, units)) bottom-up, as intervals of fluid to pour (`Liquids::build`):
/// (bottom, top, units held under pressure, kind) in units, each layer over those before it in its gap.
fn intervals(layers: impl Iterator<Item = (i32, i32, (u8, i64))>) -> Vec<(i64, i64, i64, u8)> {
    let (mut wet, mut at, mut base) = (Vec::new(), None, 0i64);
    for (floor, ceil, (kind, water)) in layers {
        if at != Some(floor) {
            (at, base) = (Some(floor), 0);
        }
        let f = floor as i64 * U;
        let cap = if ceil == OPEN { i64::MAX } else { (ceil - floor) as i64 * U };
        let (lo, hi) = (f + base.min(cap), f + (base + water).min(cap));
        wet.push((lo, hi, water - (hi - lo), kind));
        base += water;
    }
    wet
}

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
    /// What generation put into the stored columns of each fluid other than water (`Ground::generated_other`), by
    /// kind: never forgotten, a column of it stays stored.
    gen_other: std::collections::BTreeMap<u8, i64>,
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
    /// The heat of the nodes off their rest (`Therm`), by node: empty in a world of water only.
    heat: FxMap<u32, Therm>,
    /// Units of each fluid that froze (`cool`): into its `freeze` block, a block for every U, or, a sheet thinner
    /// than a block frozen through, into nothing. `counted_of(kind)` plus this is what was carried in.
    pub froze: std::collections::BTreeMap<u8, i64>,
    /// Units of each fluid boiled off by a reaction (`react`): steam the air grid will take in (liquids.md step 6).
    /// `counted_of(kind)` plus this and `froze` is what was carried in.
    pub vapour: std::collections::BTreeMap<u8, i64>,
    /// Where a reaction boiled fluid off since the caller last took them, (column, layer, kind): the steam's places.
    pub boils: Vec<(Col, i32, u8)>,
    /// Blocks frozen (booked in `froze`) whose cell was taken, (column, layer, kind): rock waiting for its cell, set
    /// by the next `cool`'s caller; never liquid again, so it holds no heat to give.
    pub waiting: Vec<(Col, i32, u8)>,
    /// Frozen rock short of a block whose node emptied with no neighbour of its fluid to take it, by (column, fluid):
    /// (the floor it froze at, its units), booked in `froze`, under a block each; it joins the column's next freeze
    /// and is set as a block once it makes one. Saved (a fifth tail).
    /// shortcut: under a block of frozen rock is held unseen until layers in eighths (ADR 0189) can show it
    pub crust: std::collections::BTreeMap<(Col, u8), (i32, i64)>,
    /// J a hot fluid gave ice blocks through their faces (`ice_face`), summed: the lava's side of the flow `thaw`
    /// takes (the energy-balance test).
    pub ice_given: f64,
    /// Layers a gap may hold (`LAYERS`; a test sets fewer, `set_layers`; 0: `LAYERS`).
    most: u8,
    /// Ice blocks whose heat is followed (`phases`; placed, frozen, or touched by an edit), by (column, layer): the units
    /// of the block melted or sublimated so far, held in it until the whole block goes. shortcut: not saved (a reload
    /// starts each block's melt again, its units kept; the server watches every ice edit again on load).
    ice: FxMap<(Col, i32), f64>,
    /// Ice blocks gone since the caller last took them, (column, layer, units): the cell is air again and `units` of
    /// water are poured there (0: it sublimated, its units are in `vapour`).
    pub thawed: Vec<(Col, i32, i64)>,
    /// The chamber of each of the home's craters (`Body::craters`' order; `World::erupt`): the units of melt over its
    /// rest, and whether its conduit is open (erupting). Empty until the first step of a world with a live one.
    pub chambers: Vec<(f64, bool)>,
    /// Units of lava the chambers poured (`World::erupt`): carried in from below, so `counted_of(lava)` plus what of
    /// it `froze` and boiled to `vapour` is this plus what buckets brought.
    pub erupted: i64,
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
        let ids = gaps.iter().map(|&(floor, ceil, w)| (floor, ceil, unpack(w))).map(|(floor, ceil, (kind, water))| self.alloc(Node { col: *c, floor, ceil, water, links: Vec::new(), quiet: u16::MAX, kind, base: 0, above: 0, press: 0, stack: 1, layer: 0, rim: true, seen: 0, sent: water, drag: 0, grad: 0, lat: 0, lacc: 0, top: 0 })).collect();
        self.cols.insert(*c, ids);
        self.tiles.entry(c.tile()).or_default().push(*c);
        if self.other {
            self.restack(*c);
        }
    }

    /// Each gap of column c's layers, bottom-up as they lie in its list: what lies under and over each and how hard
    /// the layers over it press (by density, in its own fluid's units of head).
    fn restack(&mut self, c: Col) {
        let Some(ids) = self.cols.get(&c) else { return };
        let dens = densities();
        let d = |k: u8| dens[(k as usize).min(dens.len() - 1)];
        let mut k = 0;
        while k < ids.len() {
            let floor = self.nodes[ids[k] as usize].floor;
            let e = k + ids[k..].iter().take_while(|&&i| self.nodes[i as usize].floor == floor).count();
            let total: i64 = ids[k..e].iter().map(|&i| self.nodes[i as usize].water).sum();
            let mut base = 0;
            for x in k..e {
                let da = d(self.nodes[ids[x] as usize].kind);
                let press = ids[x + 1..e].iter().map(|&i| &self.nodes[i as usize]).map(|o| (o.water as i128 * d(o.kind) as i128 / da as i128) as i64).sum();
                let n = &mut self.nodes[ids[x] as usize];
                (n.base, n.above, n.press, n.stack, n.layer) = (base, total - base - n.water, press, (e - k) as u8, (x - k) as u8);
                base += n.water;
            }
            k = e;
        }
    }

    /// The nodes of column c's gap on floor `floor`, bottom-up.
    fn gap(&self, c: Col, floor: i32) -> Vec<u32> {
        self.cols.get(&c).into_iter().flatten().copied().filter(|&i| self.nodes[i as usize].floor == floor).collect()
    }

    /// A new layer of fluid `kind` in column c's gap on floor `floor`, empty, `at` from the bottom of the gap's
    /// layers: linked to every neighbour's gap it opens onto, awake. (`restack` after the units are in.)
    fn add_layer(&mut self, g: &impl Ground, c: Col, floor: i32, kind: u8, at: usize) -> u32 {
        let ids = self.cols[&c].clone();
        let first = ids.iter().position(|&i| self.nodes[i as usize].floor == floor).expect("the gap is stored");
        let s = &self.nodes[ids[first] as usize];
        let n = Node { col: c, floor, ceil: s.ceil, water: 0, kind, base: 0, above: 0, press: 0, stack: 1, layer: 0, links: Vec::new(), quiet: 0, rim: s.rim, seen: 0, sent: 0, drag: s.drag, grad: s.grad, lat: 0, lacc: 0, top: s.top };
        let a = self.alloc(n);
        let width = ids[first..].iter().take_while(|&&i| self.nodes[i as usize].floor == floor).count();
        self.cols.get_mut(&c).unwrap().insert(first + at.min(width), a);
        for (sa, d) in g.neighbours(c).into_iter().enumerate() {
            let Some(d) = d else { continue };
            let Some(nb) = self.cols.get(&d).cloned() else { continue };
            let sb = g.neighbours(d).iter().position(|&e| e == Some(c)).unwrap_or(sa ^ 1);
            for b in nb {
                let (lo, hi) = window(&self.nodes[a as usize], &self.nodes[b as usize]);
                if lo < hi {
                    self.link(a, b, sa as u8, sb as u8, (0, 0));
                }
            }
        }
        self.other = true;
        self.wake(a);
        self.dirty.insert(c, ());
        a
    }

    /// Drops empty layer a from its gap (it holds others): its links go, its heat's crust to a neighbour of its fluid.
    /// Its index is freed by the caller once the awake list no longer holds it.
    fn drop_layer(&mut self, a: u32) {
        let c = self.nodes[a as usize].col;
        if let Some(h) = self.heat.remove(&a) {
            self.hand_crust(a, self.nodes[a as usize].kind, &h);
        }
        for l in std::mem::take(&mut self.nodes[a as usize].links) {
            self.unlink(l.to, l.back as usize);
        }
        self.cols.get_mut(&c).unwrap().retain(|&i| i != a);
        self.dirty.insert(c, ());
    }

    /// A node of fluid with heat that froze or flowed out through hands the crust it froze short of a block to a
    /// neighbour of the same fluid (what froze in a pool makes its blocks wherever the rest flowed), else its column
    /// keeps it (`crust`): a sheet's rock joins the next freeze there and sets a block once it makes a whole one.
    fn hand_crust(&mut self, a: u32, kind: u8, h: &Therm) {
        if h.solid <= 0 {
            return;
        }
        let b = self.nodes[a as usize].links.iter().map(|l| l.to).find(|&b| self.nodes[b as usize].kind == kind && self.nodes[b as usize].water > 0);
        if let Some(b) = b {
            let t = self.heat.get(&b).map_or(h.t, |x| x.t);
            let e = self.heat.entry(b).or_insert(Therm { t, age: h.age, frac: 0.0, solid: 0 });
            e.solid += h.solid;
            return;
        }
        let (col, floor) = (self.nodes[a as usize].col, self.nodes[a as usize].floor);
        let e = self.crust.entry((col, kind)).or_insert((floor, 0));
        e.0 = e.0.min(floor);
        e.1 += h.solid;
        // a whole block: set at the first free cell at or over its floor (the caller's, as `waiting` is)
        while e.1 >= U {
            e.1 -= U;
            self.waiting.push((col, e.0, kind));
        }
        if e.1 == 0 {
            self.crust.remove(&(col, kind));
        }
    }

    /// `units` of fluid `kind` at `t` C come into node a: a fluid with heat mixes its heat by units (water carries none).
    fn warm(&mut self, g: &impl Ground, a: u32, kind: u8, units: i64, t: f64) {
        if kind == crate::fluids::WATER || crate::fluids::FLUIDS.now().get(kind).heat.is_none() {
            return;
        }
        let was = self.nodes[a as usize].water;
        let t0 = if was > 0 { self.temp_of(g, a) } else { 0.0 };
        let e = self.heat.entry(a).or_insert(Therm { t: 0.0, age: 0.0, frac: 0.0, solid: 0 });
        e.t = (t0 * was as f64 + t * units as f64) / (was + units).max(1) as f64;
    }

    /// The fluid that flowed this step into gaps holding other fluids (`foreign`: receiver, kind, units, sender) joins
    /// its own layer there, or lies as a new one by density, or past the cap is pushed out and booked; then every gap
    /// of these columns and of the touched layered ones loses its empty layers, joins neighbouring layers of one fluid,
    /// lets a denser layer over a lighter one sink through it a step (`sink_rate`), and is restacked.
    fn layers_after(&mut self, g: &impl Ground, foreign: Vec<(u32, u8, i64, u32)>, touched: &[u32]) {
        let rows = crate::fluids::FLUIDS.now();
        let most = self.most();
        let mut cols: Vec<Col> = touched.iter().map(|&a| &self.nodes[a as usize]).filter(|n| n.stack > 1).map(|n| n.col).collect();
        for (a, kind, units, from) in foreign {
            let (c, floor) = (self.nodes[a as usize].col, self.nodes[a as usize].floor);
            cols.push(c);
            let t = self.temp_of(g, from);
            let gap = self.gap(c, floor);
            let to = match gap.iter().copied().find(|&i| self.nodes[i as usize].kind == kind && self.nodes[i as usize].water > 0) {
                Some(i) => Some(i),
                None if gap.len() < most => {
                    // under the first lighter layer
                    let d = rows.get(kind).density;
                    let at = gap.iter().position(|&i| self.nodes[i as usize].water > 0 && rows.get(self.nodes[i as usize].kind).density < d).unwrap_or(gap.len());
                    Some(self.add_layer(g, c, floor, kind, at))
                }
                None => None,
            };
            match to {
                Some(i) => {
                    self.warm(g, i, kind, units, t);
                    self.nodes[i as usize].water += units;
                    self.wake(i);
                }
                None => self.spill(g, c, kind, units),
            }
        }
        cols.sort_unstable();
        cols.dedup();
        let mut dropped = Vec::new();
        for c in cols {
            let floors: Vec<i32> = {
                let mut f: Vec<i32> = self.cols[&c].iter().map(|&i| self.nodes[i as usize].floor).collect();
                f.dedup();
                f
            };
            for floor in floors {
                self.sink(g, c, floor);
                let gap = self.gap(c, floor);
                if gap.len() < 2 {
                    continue;
                }
                // empty layers go (a dry gap keeps one node), and two layers of one fluid that came to touch are one
                let mut keep: Vec<u32> = Vec::new();
                for (x, &i) in gap.iter().enumerate() {
                    let n = &self.nodes[i as usize];
                    let joins = keep.last().copied().filter(|&p| n.water > 0 && self.nodes[p as usize].kind == n.kind);
                    if let Some(p) = joins {
                        let (k, w, t) = (n.kind, n.water, self.temp_of(g, i));
                        self.warm(g, p, k, w, t);
                        self.nodes[p as usize].water += w;
                        self.nodes[i as usize].water = 0;
                        // (water's `frac` is progress, not matter, `phases`: the kept node's is enough)
                        if let Some(h) = self.heat.remove(&i) {
                            if let Some(e) = self.heat.get_mut(&p) {
                                (e.frac, e.solid) = (e.frac + h.frac, e.solid + h.solid);
                            }
                        }
                        self.wake(p);
                    } else if n.water > 0 || (keep.is_empty() && x == gap.len() - 1) {
                        if n.water == 0 {
                            self.nodes[i as usize].kind = 0;
                        }
                        keep.push(i);
                        continue;
                    }
                    self.drop_layer(i);
                    dropped.push(i);
                }
            }
            self.restack(c);
        }
        if !dropped.is_empty() {
            dropped.sort_unstable();
            self.awake.retain(|a| dropped.binary_search(a).is_err());
            self.free.extend(dropped);
        }
    }

    /// The first denser layer over a lighter one in column c's gap on floor `floor` sinks through it by a step's
    /// worth (`sink_rate`): into the layer of its fluid under the lighter one, or a new one there; at the cap into
    /// its fluid's layer further down, else the stack stays as it is. An overturning gap stays awake.
    fn sink(&mut self, g: &impl Ground, c: Col, floor: i32) {
        let gap = self.gap(c, floor);
        let rows = crate::fluids::FLUIDS.now();
        let dens = |i: u32| rows.get(self.nodes[i as usize].kind).density as f64;
        let Some(x) = (0..gap.len().saturating_sub(1)).find(|&x| self.nodes[gap[x] as usize].water > 0 && self.nodes[gap[x + 1] as usize].water > 0 && dens(gap[x + 1]) > dens(gap[x])) else { return };
        let (light, heavy) = (gap[x], gap[x + 1]);
        let kind = self.nodes[heavy as usize].kind;
        let visc = if self.viscosity.is_empty() { rows.get(self.nodes[light as usize].kind).viscosity as i64 } else { self.viscosity[(self.nodes[light as usize].kind as usize).min(self.viscosity.len() - 1)] };
        let s = sink_rate(dens(light), dens(heavy), visc, g.g(c.body())).min(self.nodes[heavy as usize].water);
        let to = match x.checked_sub(1).map(|y| gap[y]).filter(|&y| self.nodes[y as usize].kind == kind) {
            Some(y) => y,
            None if gap.len() < self.most() => self.add_layer(g, c, floor, kind, x),
            // no room for a new layer: through the lighter ones to its own fluid's layer further down
            None => match gap[..x].iter().rev().copied().find(|&y| self.nodes[y as usize].kind == kind && self.nodes[y as usize].water > 0) {
                Some(y) => y,
                None => return,
            },
        };
        let t = self.temp_of(g, heavy);
        self.warm(g, to, kind, s, t);
        self.nodes[to as usize].water += s;
        self.nodes[heavy as usize].water -= s;
        for i in self.gap(c, floor) {
            self.wake(i);
        }
        self.dirty.insert(c, ());
    }

    /// `units` of fluid `kind` with no room in column c's gap: water to the column's open water's account (as a block
    /// set into it, ADR 0071), another fluid to `spilt`. Counted either way.
    fn spill(&mut self, g: &impl Ground, c: Col, kind: u8, units: i64) {
        if kind == crate::fluids::WATER {
            self.book(g.reach(c), units);
        } else {
            *self.spilt.entry(kind).or_default() += units;
        }
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
            let wet = intervals(gaps.iter().map(|&(floor, ceil, w)| (floor, ceil, unpack(w))));
            self.other |= wet.iter().any(|w| w.3 != 0);
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
        let mut wet = if lo < hi { vec![(lo, hi, 0, 0)] } else { Vec::new() };
        wet.extend(g.generated_other(c));
        // asleep: generated water is level; a neighbour's flow wakes it
        self.build(g, c, &wet, false);
    }

    /// Builds column `c`'s nodes from its gaps and pours `wet` into them: intervals of water in units, each
    /// with the units it held under pressure, which stay with the gap that holds the interval's bottom. Water
    /// in what is solid now goes to the gap above it.
    /// Each interval is of one fluid (its fourth number) and lies on what came before it in its gap, a layer of its
    /// own unless it is the fluid of the layer it lies on (where an edit joins two gaps, the upper's fluid lies on the
    /// lower's, and a denser one sinks from there); past the cap of layers it joins its own fluid's layer there, else
    /// is pushed out and booked (`spill`).
    fn build(&mut self, g: &impl Ground, c: Col, wet: &[(i64, i64, i64, u8)], wake: bool) {
        if !self.gen.contains_key(&c) {
            let (lo, hi) = g.generated(c);
            self.gen.insert(c, (hi - lo).max(0));
            self.gen_total += (hi - lo).max(0);
            for (lo, hi, extra, kind) in g.generated_other(c) {
                *self.gen_other.entry(kind).or_default() += hi - lo + extra;
            }
        }
        self.other |= wet.iter().any(|w| w.3 != 0);
        let gaps = g.gaps(c);
        let mut stacks: Vec<Vec<(u8, i64)>> = vec![Vec::new(); gaps.len()];
        for &(lo, hi, extra, kind) in wet {
            let k = gaps.iter().rposition(|&(f, _)| f as i64 * U <= lo).unwrap_or(0);
            self.put(g, c, &mut stacks[k], extra, kind);
            for (k, &(f, cl)) in gaps.iter().enumerate() {
                let (gl, gh) = (f as i64 * U, if cl == OPEN { i64::MAX } else { cl as i64 * U });
                self.put(g, c, &mut stacks[k], (hi.min(gh) - lo.max(gl)).max(0), kind);
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
                self.put(g, c, &mut stacks[k], part - sunk, kind);
                below = if cl == OPEN { i64::MAX } else { cl as i64 * U };
            }
        }
        let mut ids: Vec<u32> = Vec::with_capacity(gaps.len());
        for (&(floor, ceil), stack) in gaps.iter().zip(stacks) {
            let (lo, hi) = g.generated(c);
            let (drag, grad, lat) = match g.river(c) {
                Some((q, grad, lat)) if ceil == OPEN && lo < hi && floor as i64 * U == lo => (drag_for(grad, (hi - lo) as f64, q, g.g(c.body())), grad as i32, (lat * 65536.0) as u32),
                _ => (0, 0, 0),
            };
            let layers = if stack.is_empty() { vec![(0, 0)] } else { stack };
            for (kind, water) in layers {
                ids.push(self.alloc(Node { col: c, floor, ceil, water, kind, base: 0, above: 0, press: 0, stack: 1, layer: 0, links: Vec::new(), quiet: 0, rim: false, seen: 0, sent: water, drag, grad, lat, lacc: 0, top: hi }));
            }
        }
        let layered = ids.len() > gaps.len();
        self.cols.insert(c, ids.clone());
        self.tiles.entry(c.tile()).or_default().push(c);
        if layered {
            self.restack(c);
        }
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

    /// `units` of fluid `kind` onto a gap being built that holds the layers `stack` (`build`): on the top layer if it
    /// is of that fluid, else a layer of its own; at the cap into its fluid's layer there, else the lightest of the
    /// gap's fluids and it is pushed out (`spill`), as a denser fluid displaces a lighter one.
    fn put(&mut self, g: &impl Ground, c: Col, stack: &mut Vec<(u8, i64)>, units: i64, kind: u8) {
        if units <= 0 {
            return;
        }
        let most = self.most();
        match stack.last().map(|l| l.0) {
            Some(top) if top == kind => stack.last_mut().unwrap().1 += units,
            _ if stack.len() < most => stack.push((kind, units)),
            _ => {
                if let Some(l) = stack.iter_mut().find(|l| l.0 == kind) {
                    l.1 += units;
                    return;
                }
                let rows = crate::fluids::FLUIDS.now();
                let light = (0..stack.len()).min_by(|&a, &b| rows.get(stack[a].0).density.total_cmp(&rows.get(stack[b].0).density)).unwrap();
                if rows.get(stack[light].0).density < rows.get(kind).density {
                    let (k, n) = stack.remove(light);
                    self.spill(g, c, k, n);
                    stack.push((kind, units));
                } else {
                    self.spill(g, c, kind, units);
                }
            }
        }
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
            let other = g.generated_other(c);
            let wet = lo < hi || !other.is_empty() || g.neighbours(c).iter().flatten().any(|d| match self.cols.get(d) {
                Some(ids) => ids.iter().any(|&i| self.nodes[i as usize].water > 0),
                None => {
                    let (lo, hi) = g.generated(*d);
                    lo < hi || !g.generated_other(*d).is_empty()
                }
            });
            if wet {
                // generation's water (and lava) poured into the gaps the edit left
                let mut wet = if lo < hi { vec![(lo, hi, 0, 0)] } else { Vec::new() };
                wet.extend(other);
                self.build(g, c, &wet, true);
                for d in g.neighbours(c).into_iter().flatten() {
                    self.materialize(g, d);
                }
            }
            return;
        };
        let wet = intervals(ids.iter().map(|&i| &self.nodes[i as usize]).map(|n| (n.floor, n.ceil, (n.kind, n.water))));
        let kept = self.take_heat(g, &ids);
        self.remove(c, true);
        self.build(g, c, &wet, true);
        self.give_heat(c, kept);
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
            self.heat.remove(&a);
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
    /// `pour` of fluid `kind`: onto the gap's top layer, a layer of its own on it when that is another fluid (a
    /// denser one then sinks through, `sink`); refused when the gap holds its most layers.
    pub fn pour_kind(&mut self, g: &impl Ground, c: Col, layer: i32, units: i64, kind: u8) -> bool {
        self.materialize(g, c);
        let ids = &self.cols[&c];
        let Some(&first) = ids.iter().find(|&&i| self.nodes[i as usize].ceil > layer) else { return false };
        let floor = self.nodes[first as usize].floor;
        let gap = self.gap(c, floor);
        let top = *gap.last().unwrap();
        let n = &self.nodes[top as usize];
        let a = if n.water == 0 || n.kind == kind {
            top
        } else if gap.len() < self.most() {
            self.add_layer(g, c, floor, kind, gap.len())
        } else {
            return false;
        };
        // a fluid with heat comes at its row's pour heat, mixed with what the layer held
        if let Some(h) = crate::fluids::FLUIDS.now().get(kind).heat.as_ref() {
            self.warm(g, a, kind, units, h.pour as f64);
        }
        let n = &mut self.nodes[a as usize];
        n.kind = kind;
        self.other |= kind != 0;
        n.water += units;
        self.touch(g, a);
        if self.other {
            self.restack(c);
        }
        true
    }
    /// The stored nodes of a fluid that gives light in tiles `tiles` (`wet_tiles`), as (column, its surface's top
    /// layer, kind), sorted by column: what is round a camera, not the world.
    pub fn glowing_in(&self, tiles: &[Col]) -> Vec<(Col, i32, u8)> {
        if !self.other {
            return Vec::new();
        }
        let rows = crate::fluids::FLUIDS.now();
        let mut v: Vec<(Col, i32, u8)> = tiles
            .iter()
            .flat_map(|t| self.tiles.get(t).into_iter().flatten())
            .flat_map(|c| &self.cols[c])
            .map(|&i| &self.nodes[i as usize])
            .filter(|n| n.water > 0 && rows.get(n.kind).glows())
            .map(|n| (n.col, ((n.surface() - 1).div_euclid(U)) as i32, n.kind))
            .collect();
        v.sort_unstable_by_key(|x| (x.0, x.1));
        v
    }
    /// The block of fluid `kind` frozen for column c's cell z could not be set (the cell was taken): it stays frozen
    /// and booked, rock `waiting` to be set the next time (`cool`), whatever became of the node it froze from. Put
    /// back as liquid it would carry heat it gave away already (a reaction's skin boiled water with it).
    pub fn unfreeze(&mut self, c: Col, z: i32, kind: u8) {
        self.waiting.push((c, z, kind));
    }
    /// The heat node a holds, C: its own, or at rest what its place gives.
    fn temp_of(&self, g: &impl Ground, a: u32) -> f64 {
        self.heat.get(&a).map_or_else(|| self.rest_of(g, a), |h| h.t)
    }
    /// `Ground::heat` at the middle of node a's fluid.
    fn rest_of(&self, g: &impl Ground, a: u32) -> f64 {
        let n = &self.nodes[a as usize];
        g.heat(n.col, n.floor as f64 + n.water.min(n.cap()) as f64 / U as f64 * 0.5) as f64
    }
    /// The heat of the node holding layer `layer` of column `c`, if it is held off its rest (tests, F3).
    pub fn temp_at(&self, c: Col, layer: i32) -> Option<f64> {
        let ids = self.cols.get(&c)?;
        let &a = ids.iter().find(|&&i| self.nodes[i as usize].ceil > layer)?;
        self.gap(c, self.nodes[a as usize].floor).iter().find_map(|i| self.heat.get(i)).map(|h| h.t)
    }
    /// Every heat held, as (column, the node's floor, heat), sorted by those and its fluid: for saves.
    pub fn heats(&self) -> Vec<(Col, i32, Therm)> {
        self.held_heats().into_iter().map(|(c, f, _, h)| (c, f, h)).collect()
    }
    /// The fluid of each of `heats`, in its order (a save's: layers of one gap share a floor).
    pub fn heat_kinds(&self) -> Vec<u8> {
        self.held_heats().into_iter().map(|x| x.2).collect()
    }
    fn held_heats(&self) -> Vec<(Col, i32, u8, Therm)> {
        let mut v: Vec<(Col, i32, u8, Therm)> = self.heat.iter().map(|(&a, &h)| (self.nodes[a as usize].col, self.nodes[a as usize].floor, self.nodes[a as usize].kind, h)).collect();
        v.sort_unstable_by_key(|x| (x.0, x.1, x.2));
        v
    }
    /// A save's heats back on the nodes `restore` built: each on its column's node of its fluid (`kinds`, by `heats`'
    /// order; a save from before them names none: a fluid other than water) and that floor, else on the column's first
    /// node of that fluid that has none yet (the ground under it changed since), else dropped. Then every stored node of such a fluid still without one, outside generation's own (which rests at
    /// its place's heat) and where its place would not hold it molten (it rested there, `cool` dropped its heat), is
    /// given its row's pour heat: a save from before heat, or one whose heat was dropped, must not leave lava liquid
    /// for ever.
    pub fn restore_heat(&mut self, g: &impl Ground, heats: &[(Col, i32, Therm)], kinds: &[u8]) {
        let rows = crate::fluids::FLUIDS.now();
        let hot = |n: &Node| n.water > 0 && n.kind != crate::fluids::WATER && rows.get(n.kind).heat.is_some();
        for (k, &(c, floor, h)) in heats.iter().enumerate() {
            let kind = kinds.get(k).copied();
            let held = |n: &Node| n.water > 0 && rows.get(n.kind).heat.is_some() && kind.map_or(n.kind != crate::fluids::WATER, |k| n.kind == k);
            let Some(ids) = self.cols.get(&c) else { continue };
            let own = ids.iter().copied().find(|&i| held(&self.nodes[i as usize]) && self.nodes[i as usize].floor == floor && !self.heat.contains_key(&i));
            if let Some(a) = own.or_else(|| ids.iter().copied().find(|&i| held(&self.nodes[i as usize]) && !self.heat.contains_key(&i))) {
                self.heat.insert(a, h);
            }
        }
        if !self.other {
            return;
        }
        let mut cols: Vec<Col> = self.cols.keys().copied().collect();
        cols.sort_unstable();
        for c in cols {
            let gen = g.generated_other(c);
            for &a in &self.cols[&c] {
                let n = &self.nodes[a as usize];
                let (lo, hi) = (n.floor as i64 * U, n.surface());
                if !hot(n) || self.heat.contains_key(&a) || gen.iter().any(|w| w.0 < hi && lo < w.1.max(w.0 + 1)) {
                    continue;
                }
                // where its place holds it molten it rests there with none (`cool` dropped it): no heat from nothing
                if rows.get(n.kind).freeze.as_ref().is_some_and(|f| self.rest_of(g, a) > f.at as f64) {
                    continue;
                }
                let t = rows.get(n.kind).heat.as_ref().map_or(0.0, |h| h.pour as f64);
                self.heat.insert(a, Therm { t, age: 0.0, frac: 0.0, solid: 0 });
            }
        }
    }
    /// The heats of column c's nodes, by fluid, before its nodes are built again (`give_heat`): a node with none
    /// carries its rest's, which is its old place's.
    fn take_heat(&self, g: &impl Ground, ids: &[u32]) -> Vec<(u8, i64, Therm)> {
        if !self.other && self.heat.is_empty() {
            return Vec::new();
        }
        let rows = crate::fluids::FLUIDS.now();
        // (water carries one only while it is held off its rest, `phases`)
        ids.iter()
            .map(|&a| (a, &self.nodes[a as usize]))
            .filter(|&(a, n)| n.water > 0 && (n.kind != crate::fluids::WATER || self.heat.contains_key(&a)) && rows.get(n.kind).heat.is_some())
            .map(|(a, n)| (n.kind, n.water, self.heat.get(&a).copied().unwrap_or(Therm { t: self.rest_of(g, a), age: 0.0, frac: 0.0, solid: 0 })))
            .collect()
    }
    /// What `take_heat` took, on column c's new nodes: each of a fluid takes the fluid's mean heat by units and its
    /// oldest age; the first takes what froze short of a block. shortcut: one heat per fluid a column across an
    /// edit, where its gaps were split; per gap if a split ever holds two heats far apart.
    fn give_heat(&mut self, c: Col, kept: Vec<(u8, i64, Therm)>) {
        let mut kinds: Vec<u8> = kept.iter().map(|k| k.0).collect();
        kinds.sort_unstable();
        kinds.dedup();
        for k in kinds {
            let of = kept.iter().filter(|x| x.0 == k);
            let w: i64 = of.clone().map(|x| x.1).sum();
            let t = of.clone().map(|x| x.2.t * x.1 as f64).sum::<f64>() / w.max(1) as f64;
            let age = of.clone().map(|x| x.2.age).fold(0.0, f64::max);
            let (mut frac, mut solid) = (of.clone().map(|x| x.2.frac).sum::<f64>(), of.map(|x| x.2.solid).sum::<i64>());
            for &a in self.cols.get(&c).into_iter().flatten() {
                let n = &self.nodes[a as usize];
                if n.kind == k && n.water > 0 {
                    self.heat.insert(a, Therm { t, age, frac, solid });
                    (frac, solid) = (0.0, 0);
                }
            }
        }
    }
    /// Node heats mixed by what flows this step (`step`, before the units move): a node's new heat is its own
    /// that stays and what flows in, by units. Nodes at rest that send are given their rest's heat to carry.
    fn mix_heat(&mut self, g: &impl Ground, touched: &[u32]) {
        let rows = crate::fluids::FLUIDS.now();
        let hot = |n: &Node| n.kind != crate::fluids::WATER && rows.get(n.kind).heat.is_some();
        let mut rest = Vec::new();
        for &a in touched {
            let n = &self.nodes[a as usize];
            if !hot(n) {
                continue;
            }
            // (only its own fluid mixes in: another's goes to its own layer, `layers_after`)
            for l in &n.links {
                if self.nodes[l.to as usize].links[l.back as usize].out > 0 && self.nodes[l.to as usize].kind == n.kind && !self.heat.contains_key(&l.to) {
                    rest.push(l.to);
                }
            }
        }
        rest.sort_unstable();
        rest.dedup();
        for a in rest {
            let t = self.rest_of(g, a);
            self.heat.insert(a, Therm { t, age: 0.0, frac: 0.0, solid: 0 });
        }
        let mut mixed = Vec::new();
        for &a in touched {
            let n = &self.nodes[a as usize];
            if !hot(n) {
                continue;
            }
            let (mut inn, mut sum, mut gone) = (0i64, 0.0, 0i64);
            for l in &n.links {
                gone += l.out as i64;
                let m = self.nodes[l.to as usize].links[l.back as usize];
                if m.out > 0 && self.nodes[l.to as usize].kind == n.kind {
                    inn += m.out as i64;
                    sum += m.out as f64 * self.heat[&l.to].t;
                }
            }
            if inn == 0 {
                continue;
            }
            let own = (n.water - gone).max(0);
            let had = self.heat.get(&a).copied().filter(|_| own > 0);
            let t0 = if own > 0 { self.temp_of(g, a) } else { 0.0 };
            let t = (t0 * own as f64 + sum) / (own + inn) as f64;
            mixed.push((a, Therm { t, ..had.unwrap_or(Therm { t, age: 0.0, frac: 0.0, solid: 0 }) }));
        }
        for (a, h) in mixed {
            self.heat.insert(a, h);
        }
    }

    /// One heat step of `dt` seconds for every node held off its rest (lava basics, draft decision; 1 Hz on the
    /// server): it gives heat to the air over its open surface (radiation and convection) and its open sides, and to
    /// the rock under it and beside it (conduction through half its own depth, then into rock warming since it came,
    /// a semi-infinite solid's), all to its place's rest heat (`Ground::heat`). Past its freeze point what it loses
    /// more freezes its units at its row's latent heat; each whole block of them is a cell of its `freeze` block at
    /// the node's floor, returned (column, layer, kind) for the caller to set; a sheet frozen through short of a
    /// block is nothing. Every unit frozen is booked to `froze`. A node back at rest, liquid, is forgotten.
    /// Fluids that react meet first (`react`). Heat runs on the world's two constants (`interior::HEAT_PACE`, `LIGHT`).
    pub fn cool(&mut self, g: &impl Ground, dt: f64) -> Vec<(Col, i32, u8)> {
        let mut out = std::mem::take(&mut self.waiting);
        out.extend(self.react(g));
        let rows = crate::fluids::FLUIDS.now();
        // water's phases (liquids.md step 5): stored water off its rest is given a heat, ice blocks followed melt
        let ph = phases::Phases::of(rows.get(crate::fluids::WATER));
        if let Some(ph) = &ph {
            self.wake_water(g, ph);
            self.thaw(g, dt, ph);
        }
        if self.heat.is_empty() {
            return out;
        }
        let mut ids: Vec<u32> = self.heat.keys().copied().collect();
        ids.sort_unstable();
        for a in ids {
            let n = &self.nodes[a as usize];
            if let (Some(ph), true) = (&ph, n.kind == crate::fluids::WATER && n.water > 0) {
                let h = self.heat[&a];
                match self.water_heat(g, a, h, dt, ph, &mut out) {
                    Some(h) => self.heat.insert(a, h),
                    None => self.heat.remove(&a),
                };
                continue;
            }
            let row = rows.get(n.kind);
            let Some(hr) = row.heat.as_ref().filter(|_| n.water > 0 && n.kind != crate::fluids::WATER) else {
                self.heat.remove(&a);
                continue;
            };
            let tf = row.freeze.as_ref().map_or(f64::NEG_INFINITY, |f| f.at as f64);
            let ta = self.rest_of(g, a);
            let mut h = self.heat[&a];
            if (h.t - ta).abs() < REST_K && ta > tf && h.solid == 0 {
                self.heat.remove(&a);
                continue;
            }
            let rho = row.density as f64;
            // what it stores is a `LIGHT`th of the real; its loss's flows are paced in `loss`, and slowed by its row's
            // own factor (lava's 0.5: the developer's taste call, 2026-11-24, so a flow runs further before it crusts)
            let cap = n.water as f64 / U as f64 * rho * hr.capacity as f64 / LIGHT;
            // what goes into an ice block is at the plain pace, the very number `thaw` melts it by
            let (lost, to_ice) = self.loss(g, a, &h, ta, hr);
            let q = (lost * hr.pace as f64 + to_ice) * dt;
            self.ice_given += to_ice * dt;
            // a coarse step never carries it past the place's temperature, either way; it freezes only from heat taken
            // below its freeze point, so a place over that point never makes rock
            if q <= 0.0 {
                h.t += (-q / cap).min((ta - h.t).max(0.0));
            } else {
                let sensible = ((h.t - ta.max(tf)).max(0.0) * cap).min(q);
                h.t -= sensible / cap;
                if ta < tf {
                    h.frac += (q - sensible) / (hr.latent as f64 / LIGHT * rho / U as f64);
                }
            }
            let take = (h.frac.floor() as i64).min(n.water);
            let (col, floor, kind) = (n.col, n.floor, n.kind);
            if take > 0 {
                h.frac -= take as f64;
                // a sheet's crust held in this column freezes on with it
                h.solid += take + self.crust.remove(&(col, kind)).map_or(0, |x| x.1);
                *self.froze.entry(kind).or_default() += take;
                let n = &mut self.nodes[a as usize];
                n.water -= take;
                if n.water == 0 && n.stack == 1 {
                    n.kind = 0;
                }
                if n.quiet == u16::MAX {
                    self.wake(a);
                }
                self.dirty.insert(col, ());
                if self.nodes[a as usize].stack > 1 {
                    self.restack(col);
                }
            }
            // every whole block a step (a coarse step freezes more than one)
            while h.solid >= U && row.freeze.is_some() {
                out.push((col, floor, kind));
                h.solid -= U;
            }
            if self.nodes[a as usize].water == 0 {
                // frozen through: its crust short of a block goes as any emptied node's does (`hand_crust`)
                self.heat.remove(&a);
                self.hand_crust(a, kind, &h);
                // a layer under or over others goes, as every emptied layer does (`layers_after`)
                if self.nodes[a as usize].stack > 1 {
                    self.drop_layer(a);
                    self.awake.retain(|&i| i != a);
                    self.free.push(a);
                    self.restack(col);
                }
            } else {
                h.age += dt;
                self.heat.insert(a, h);
            }
        }
        out
    }
    /// Fluids meeting fluids their rows react with (liquids.md "Mixing in the world"; once a second, before `cool`'s
    /// heat step): a node of a fluid whose row `reacts` with another, touching a node of that one (a layer on or under
    /// it in its gap, or a neighbour's beside it, the two fluids side by side), freezes its skin at the contact at
    /// once, a cell of itself (U units, or what it holds), into its `freeze` block. The heat that skin gives, cooling
    /// from the node's own heat (`temp_of`) to the other's boiling point and freezing (its row's capacity and latent
    /// heat), and the reaction's own `heat`, boils the other off: from its place's heat to its boiling point, and its
    /// latent heat of vaporisation (its row's `heat`). Where the other cannot take a whole skin's heat the skin is what
    /// it can quench; a film too thin to quench a unit takes its heat from the node, which is colder for it. Frozen
    /// units go to `froze` (a block a U at the node's floor, returned as `cool`'s), boiled ones to `vapour`, the
    /// contact's cell to `boils`.
    /// shortcut: the skin's heat all goes at once (a block carries no heat), and a node reacts with each node it
    /// touches once a second whatever the contact's area; by area once blocks carry heat (liquids.md step 5).
    fn react(&mut self, g: &impl Ground) -> Vec<(Col, i32, u8)> {
        let mut out = Vec::new();
        if !self.other {
            return out;
        }
        let rows = crate::fluids::FLUIDS.now();
        let reacts = |k: u8, with: u8| rows.get(k).reacts.iter().find(|r| rows.kind(&r.with) == Some(with));
        // the pairs (reacting, other): what every stored node of a fluid that reacts touches, held hot or at rest in a
        // hot place, awake or asleep. shortcut: every stored node is looked at once a second; a list of the contacts
        // kept by the step if worlds come to hold a great deal of stored fluid other than water
        let mut pairs: Vec<(u32, u32)> = Vec::new();
        for &a in self.cols.values().flatten() {
            let n = &self.nodes[a as usize];
            if n.water == 0 || rows.get(n.kind).reacts.is_empty() {
                continue;
            }
            for b in self.touching(a) {
                if reacts(n.kind, self.nodes[b as usize].kind).is_some() {
                    pairs.push((a, b));
                }
            }
        }
        pairs.sort_unstable();
        pairs.dedup();
        let mut cols = Vec::new();
        for (r, w) in pairs {
            let (rn, wn) = (&self.nodes[r as usize], &self.nodes[w as usize]);
            let (Some(rx), true) = (reacts(rn.kind, wn.kind), rn.water > 0 && wn.water > 0) else { continue };
            let (rr, wr) = (rows.get(rn.kind), rows.get(wn.kind));
            let (Some(hr), Some(hw)) = (rr.heat.as_ref(), wr.heat.as_ref().filter(|h| h.vaporise > 0.0)) else { continue };
            let tb = wr.boil.as_ref().map_or(100.0, |b| b.at as f64);
            let t = self.temp_of(g, r);
            if t <= tb {
                continue;
            }
            let tw = self.rest_of(g, w).min(tb);
            // J a unit of the skin gives, and a unit of the other takes to boil off, real: only their ratio is used, in
            // which `LIGHT` cancels (the skin's heat is the water's boil, both sides in one step, at no rate)
            let e_r = rr.density as f64 / U as f64 * (hr.capacity as f64 * (t - tb) + hr.latent as f64) + rx.heat as f64 * 1000.0;
            let e_w = wr.density as f64 / U as f64 * (hw.capacity as f64 * (tb - tw) + hw.vaporise as f64);
            let mut skin = rn.water.min(U);
            if skin as f64 * e_r > wn.water as f64 * e_w {
                skin = (wn.water as f64 * e_w / e_r) as i64;
            }
            let boil = if skin > 0 { ((skin as f64 * e_r / e_w) as i64).min(wn.water) } else { wn.water };
            let (rc, rf, rk, rw, wk) = (rn.col, rn.floor, rn.kind, rn.water, wn.kind);
            let z = ((rn.surface().min(wn.surface()) - 1).div_euclid(U)) as i32;
            self.boils.push((wn.col, z, wk));
            cols.extend([rc, wn.col]);
            let n = &mut self.nodes[w as usize];
            n.water -= boil;
            if n.water == 0 && n.stack == 1 {
                n.kind = 0;
            }
            *self.vapour.entry(wk).or_default() += boil;
            let h = self.heat.entry(r).or_insert(Therm { t, age: 0.0, frac: 0.0, solid: 0 });
            if skin == 0 {
                h.t -= boil as f64 * e_w / (rw as f64 / U as f64 * rr.density as f64 * hr.capacity as f64);
            }
            h.solid += skin;
            while h.solid >= U && rr.freeze.is_some() {
                out.push((rc, rf, rk));
                h.solid -= U;
            }
            *self.froze.entry(rk).or_default() += skin;
            let n = &mut self.nodes[r as usize];
            n.water -= skin;
            if n.water == 0 {
                if n.stack == 1 {
                    n.kind = 0;
                }
                let h = self.heat.remove(&r).unwrap();
                self.hand_crust(r, rk, &h);
            }
            self.wake(r);
            self.wake(w);
        }
        cols.sort_unstable();
        cols.dedup();
        for c in cols {
            self.dirty.insert(c, ());
            self.restack(c);
        }
        out
    }
    /// The wet nodes node a's fluid touches: the layers on and under it in its gap, and the neighbours' fluids beside
    /// its own.
    fn touching(&self, a: u32) -> Vec<u32> {
        let n = &self.nodes[a as usize];
        let mut v: Vec<u32> = self.gap(n.col, n.floor).into_iter().filter(|&i| (self.nodes[i as usize].layer as i32 - n.layer as i32).abs() == 1).collect();
        for l in &n.links {
            let b = &self.nodes[l.to as usize];
            let (lo, hi) = window(n, b);
            if b.water > 0 && n.bottom().max(b.bottom()).max(lo) < n.surface().min(b.surface()).min(hi) {
                v.push(l.to);
            }
        }
        v.retain(|&i| self.nodes[i as usize].water > 0);
        v
    }
    /// The heat node a gives away a second at heat `h`, its place's rest heat `ta`, W (negative: it takes in): every
    /// flow the world's (`HEAT_PACE` on its conductivity, the air's terms and the rock's effusivity, the rock storing
    /// a `LIGHT`th).
    fn loss(&self, g: &impl Ground, a: u32, h: &Therm, ta: f64, hr: &crate::fluids::Heat) -> (f64, f64) {
        let n = &self.nodes[a as usize];
        let k = hr.conduct as f64 * HEAT_PACE;
        let e = hr.emissivity as f64;
        let deep = (n.water.min(n.cap()) as f64 / U as f64).max(1.0 / 64.0);
        // the watched ice blocks beside its cells, as `thaw` finds them (a cell's middle in this fluid): each face
        // gives that block `ice_face`, the one number both sides use; it is not rock
        let (mut ice_side, mut ice_floor, mut to_ice) = (0i64, false, 0.0);
        if !self.ice.is_empty() {
            let tf = crate::fluids::FLUIDS.now().get(crate::fluids::WATER).freeze.as_ref().map_or(0.0, |f| f.at as f64);
            let iced = |c: Col, z: i64| self.ice.contains_key(&(c, z as i32)) && g.ice(c, z as i32);
            let mut y = n.bottom().div_euclid(U);
            while y * U + U / 2 < n.surface() {
                if y * U + U / 2 >= n.bottom() {
                    let sides = g.neighbours(n.col).into_iter().flatten().filter(|&d| iced(d, y)).count() as i64;
                    ice_side += sides * (((y + 1) * U).min(n.surface()) - (y * U).max(n.bottom())).max(0);
                    to_ice += sides as f64 * ice_face(h.t, tf, hr.conduct);
                    for z in [y - 1, y + 1].into_iter().filter(|&z| iced(n.col, z)) {
                        ice_floor |= z == y - 1 && n.base == 0 && y == n.floor as i64;
                        to_ice += ice_face(h.t, tf, hr.conduct);
                    }
                }
                y += 1;
            }
        }
        // into rock: through half its own depth (or half a block, sideways), then the rock's own warming
        let rock = |r_in: f64| (h.t - ta) / (r_in + (std::f64::consts::PI * h.age.max(1.0)).sqrt() / effusivity());
        // (a layer on another lies on it, not on rock; one under another has no open surface: what it gives the fluid
        // over it is a reaction's, `react`. shortcut: no heat passes between layers that do not react)
        let mut p = if n.base > 0 || ice_floor { 0.0 } else { rock(deep * 0.5 / k) };
        if n.above == 0 && n.fill() < n.cap() {
            p += surface(h.t, ta, deep * 0.5 / k, e);
        }
        // its four sides (a block wide each): fluid beside it takes nothing here (the same fluid is one body; another's
        // exchange is `react`'s alone), air over a neighbour's surface is an open side, ice is `to_ice`, the rest rock
        let (lo, hi) = (n.bottom(), n.surface());
        let (mut wet, mut open) = (0i64, 0i64);
        for l in &n.links {
            let b = &self.nodes[l.to as usize];
            let (wl, wh) = window(n, b);
            let (wl, wh) = (wl.max(lo), wh.min(hi));
            if wh <= wl {
                continue;
            }
            let cover = if b.water > 0 { (b.surface().min(wh) - wl).max(0) } else { 0 };
            wet += cover;
            open += wh - wl - cover;
        }
        let side = (4 * (hi - lo) - wet - open - ice_side).max(0);
        p += side as f64 / U as f64 * rock(0.5 / k);
        (p + open as f64 / U as f64 * surface(h.t, ta, 0.5 / k, e), to_ice)
    }
    /// `scoop` of fluid `kind`.
    pub fn scoop_kind(&mut self, g: &impl Ground, c: Col, layer: i32, units: i64, kind: u8) -> i64 {
        self.materialize(g, c);
        let ids = &self.cols[&c];
        let Some(&first) = ids.iter().find(|&&i| self.nodes[i as usize].ceil > layer) else { return 0 };
        // from the gap's top layer (an empty one under layers goes with the next step, `layers_after`)
        let a = *self.gap(c, self.nodes[first as usize].floor).last().unwrap();
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
        if self.other {
            self.restack(c);
        }
        took
    }
    /// The units of fluid `kind` that differ from the seed's (`counted` for water).
    pub fn counted_of(&self, kind: u8) -> i64 {
        // (only water trades with the sea and rivers, or rains; lava is generated in volcanoes, `gen_other`)
        if kind == crate::fluids::WATER {
            return self.counted();
        }
        self.total_of(kind) + self.spilt.get(&kind).copied().unwrap_or(0) - self.gen_other.get(&kind).copied().unwrap_or(0)
    }
    /// The fluid in layer `layer` of column `c`, if any (stored, else as generation gives it): what a player's
    /// feet stand in.
    pub fn kind_at(&self, g: &impl Ground, c: Col, layer: f64) -> Option<u8> {
        let u = layer * U as f64;
        if self.cols.contains_key(&c) {
            return self.in_gap(c, layer).find(|n| n.water > 0 && u >= n.bottom() as f64 && u < n.surface() as f64).map(|n| n.kind);
        }
        let (lo, hi) = g.generated(c);
        if lo < hi && (lo as f64..hi as f64).contains(&u) {
            return Some(crate::fluids::WATER);
        }
        g.generated_other(c).into_iter().find(|w| (w.0 as f64..w.1 as f64).contains(&u)).map(|w| w.3)
    }
    /// The layers of column c's gap that holds layer `layer`, bottom-up (none if c is not stored or it is solid there).
    fn in_gap(&self, c: Col, layer: f64) -> impl Iterator<Item = &Node> {
        let ids = self.cols.get(&c).map_or(&[][..], |v| &v[..]);
        let floor = ids.iter().map(|&i| &self.nodes[i as usize]).find(|n| (n.ceil as f64) > layer && n.floor as f64 <= layer.floor()).map(|n| n.floor);
        ids.iter().map(|&i| &self.nodes[i as usize]).filter(move |n| Some(n.floor) == floor)
    }
    /// Fluid `kind` flows with viscosity `v` instead of its row's (tests).
    pub fn set_viscosity(&mut self, kind: u8, v: i64) {
        if self.viscosity.is_empty() {
            self.viscosity = crate::fluids::FLUIDS.rows.iter().map(|r| r.viscosity as i64).collect();
        }
        self.viscosity[kind as usize] = v;
    }

    /// A gap holds at most `n` layers (tests: the cap reached with few fluids).
    pub fn set_layers(&mut self, n: u8) {
        self.most = n;
    }
    fn most(&self) -> usize {
        if self.most == 0 { LAYERS } else { self.most as usize }
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
            Some(_) => {
                // (water's alone: what is drunk, swum in and puts out a flame; `fluid_surface_at` holds a body up in any):
                // the top of the gap's highest water layer (a dry gap's is its floor)
                self.in_gap(c, layer).filter(|n| n.kind == crate::fluids::WATER).last()?.surface()
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

    /// `surface_at` of any fluid, with its kind: stored, or as generation gives it (water, then a volcano's lava).
    pub fn fluid_surface_at(&self, g: &impl Ground, c: Col, layer: f64) -> Option<(f64, u8)> {
        let u = layer * U as f64;
        let (top, kind) = match self.cols.get(&c) {
            // the layer at `layer`, else (over them all) the top one, as before layers
            Some(_) => {
                let n = self.in_gap(c, layer).filter(|n| n.water > 0).find(|n| u < n.surface() as f64).or_else(|| self.in_gap(c, layer).filter(|n| n.water > 0).last())?;
                (n.surface(), n.kind)
            }
            None => {
                let (lo, hi) = g.generated(c);
                if lo < hi && u >= lo as f64 {
                    (hi, crate::fluids::WATER)
                } else {
                    g.generated_other(c).into_iter().find(|w| (w.0 as f64..w.1 as f64).contains(&u)).map(|w| (w.1, w.3))?
                }
            }
        };
        let s = top as f64 / U as f64;
        (layer < s).then_some((s, kind))
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
        // layers by density (`carry`): only once a fluid other than water was stored
        let dens = if self.other { densities() } else { Vec::new() };
        let (cols, most) = (&self.cols, self.most());
        let flows = |&a: &u32| {
            let n = &nodes[a as usize];
            let cg = cgs[n.col.body() as usize];
            let visc = rows[(n.kind as usize).min(rows.len() - 1)]; // an unknown kind (a newer save): the last row, as `Fluids::get`
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
                // (a layer flows through its own depth in the window)
                let area = (sa.min(hi) - lo.max(n.bottom())).clamp(0, ACAP);
                let mut v = l.v as i64;
                let hb = if area > 0 && spreads(n, b) { carry(nodes, cols, &dens, most, n, b) } else { None };
                if let Some(hb) = hb {
                    let mut dh = ha - hb.max(lo);
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
                } else {
                    v = 0;
                }
                let mut flow = v * area + l.frac as i64;
                // a closed gap's head rises K times as fast as the water it takes in: unbounded, a full gap under
                // a block set into the sea took in a wave, shot it out again at K times the height, and swung
                // on, pumping water out over the block and leaving the gap half empty (ADR 0071). Into or out of
                // a closed gap, a link passes at most what would level the two heads, shared over its links: half
                // the difference between two open surfaces, a (K + 1)-th of it against a full gap's pressure.
                if let Some(hb) = hb.filter(|_| v > 0 && (n.ceil != OPEN || b.ceil != OPEN)) {
                    let dh = (ha - hb.max(lo)).max(0);
                    let mut most = i64::MAX;
                    if b.ceil != OPEN {
                        let room = (b.cap() - b.fill()).max(0);
                        let level = if 2 * room >= dh { dh / 2 } else { room + (dh - 2 * room) / (K + 1) };
                        most = most.min(level / b.links.len().max(1) as i64);
                    }
                    if n.ceil != OPEN {
                        let over = (n.fill() - n.cap()).max(0);
                        most = most.min((over + (sa - hb.max(lo)).max(0) / 2) / n.links.len().max(1) as i64);
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
        // a dry layer takes the kind of its first link that brings any; what another fluid brings a layer goes on to
        // its own layer in that gap, or a new one, after the step (`foreign`: receiver, kind, units, sender)
        let mut foreign: Vec<(u32, u8, i64, u32)> = Vec::new();
        let mut divert: FxMap<u32, i64> = FxMap::default();
        if self.other {
            for &a in &touched {
                let wet = self.nodes[a as usize].water != 0;
                let mut kind = wet.then_some(self.nodes[a as usize].kind);
                for k in 0..self.nodes[a as usize].links.len() {
                    let l = self.nodes[a as usize].links[k];
                    let (from, m) = (self.nodes[l.to as usize].kind, self.nodes[l.to as usize].links[l.back as usize]);
                    if m.out <= 0 {
                        continue;
                    }
                    match kind {
                        None => kind = Some(from),
                        Some(have) if have != from => {
                            foreign.push((a, from, m.out as i64, l.to));
                            *divert.entry(a).or_default() += m.out as i64;
                        }
                        _ => {}
                    }
                }
                if let Some(k) = kind.filter(|_| !wet) {
                    self.nodes[a as usize].kind = k;
                }
            }
            // heat rides with what flows (water carries none)
            self.mix_heat(g, &touched);
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
            if !divert.is_empty() {
                d -= divert.get(&a).copied().unwrap_or(0);
            }
            let n = &mut self.nodes[a as usize];
            n.water += d;
            // (an emptied layer of a stack keeps its fluid until `layers_after` drops it)
            if n.water == 0 && n.stack == 1 {
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
                    let Some(hy) = (if spreads(x, y) { carry(&self.nodes, &self.cols, &dens, most, x, y) } else { None }) else { return false };
                    let (area, dh) = (x.surface().min(hi) - lo.max(x.bottom()), x.head() - hy.max(lo));
                    let visc = rows[(x.kind as usize).min(rows.len() - 1)];
                    // a thick fluid: only while the speed a step gives it outlasts its friction (else it never sleeps)
                    let moving = if visc == 1 { moves(dh) } else { thick_moves(dh, cgs[x.col.body() as usize], visc, area) };
                    area > 0 && moving
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
        if self.other {
            self.layers_after(g, foreign, &touched);
        }
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
                // (a stack of layers is never laid at the sea's level: the sea is water alone)
                calm &= n.stack == 1 || n.ceil == OPEN;
                if !n.rim && n.ceil == OPEN && n.kind == crate::fluids::WATER && n.stack == 1 {
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
            if self.other {
                let mut cols: Vec<Col> = body.iter().filter(|&&a| self.nodes[a as usize].stack > 1).map(|&a| self.nodes[a as usize].col).collect();
                cols.dedup();
                cols.into_iter().for_each(|c| self.restack(c));
            }
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
                    // (only water alone trades with open water; another fluid, or layers, store the column)
                    Some(n) if lo < hi && n.floor as i64 * U < hi && n.kind == crate::fluids::WATER && n.stack == 1 => {
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
                    Some(n) if n.water == 0 && n.stack == 1 && lo < hi && n.floor as i64 * U >= hi => {}
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
            // (a column generation fills with another fluid is never given back: it would come back full)
            if n.ceil != OPEN || n.kind != crate::fluids::WATER || self.heat.contains_key(&a) || (n.water - want).abs() >= DEAD || (want == 0 && lo < hi) || g.gaps(c) != [(n.floor, OPEN)] || !g.generated_other(c).is_empty() {
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
    /// level its water reaches, or `sea` (in units) with the difference given to the open sea. A layer's level is its
    /// head (its surface pressed by the layers over it), over the layers under it.
    fn flatten(&mut self, body: &[u32], sea: Option<(i64, u32)>) {
        let w = |n: &Node, l: i64| -> i64 {
            let f = n.floor as i64 * U + n.base + n.press;
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
        let mut hi = ns().map(|n| if n.ceil == OPEN { n.floor } else { n.ceil } as i64 * U + n.base + n.press).max().unwrap() + total * K + 1;
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
        /// `Ground::heat`, everywhere, C.
        pub heat: f32,
        /// `Ground::air` (C, Pa), everywhere.
        pub air: (f64, f64),
        /// The cells holding ice (`Ground::ice`), solid too.
        pub ice: Vec<(i32, i32, i32)>,
    }

    impl Box3 {
        pub fn new(nx: i32, ny: i32, nz: i32) -> Box3 {
            Box3 { nx, ny, nz, solid: vec![false; (nx * ny * nz) as usize], sea: None, epoch: 0, g: EARTH_G, heat: AMBIENT, air: (AMBIENT as f64, phases::ATM), ice: Vec::new() }
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
        fn heat(&self, _c: Col, _z: f64) -> f32 {
            self.heat
        }
        fn air(&self, _c: Col, _z: f64) -> (f64, f64) {
            self.air
        }
        fn ice(&self, c: Col, z: i32) -> bool {
            self.ice.contains(&(c.i(), c.j(), z))
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

    /// Water alone steps to the bit as it did before fluids lay in layers (liquids.md step 4): the `deterministic`
    /// run's every column and gap after every step, hashed, against the number the build before layers made: the
    /// same trace run on commit cddfd4f2's engine (the parent of layers) printed this hash and ledger, seen equal.
    #[test]
    fn water_alone_steps_to_the_bit_as_before_layers() {
        let mut g = Box3::new(30, 30, 6);
        g.fill(0..30, 0..30, 0..1, true);
        g.fill(10..12, 0..20, 1..4, true);
        let mut w = stored(&g);
        let mut h: u64 = 0xcbf29ce484222325;
        let mut eat = |v: i64| {
            for b in v.to_le_bytes() {
                h = (h ^ b as u64).wrapping_mul(0x100000001b3);
            }
        };
        for k in 0..340 {
            if k < 40 {
                w.pour(&g, col(k % 30, (k * 7) % 30), 1, U + k as i64);
            }
            w.step(&g);
            for (c, gaps) in w.all() {
                eat(c.0 as i64);
                gaps.iter().for_each(|&(f, t, u)| [f as i64, t as i64, u].into_iter().for_each(&mut eat));
            }
        }
        assert_eq!((h, w.counted()), (12127160028250922625, 164620), "the trace's hash and the ledger");
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

    /// Water and lava poured side by side meet and lie in layers, the lava under: each fluid's ledger is exact at
    /// every one of 1,000 steps (no heat step runs, so they do not react).
    #[test]
    fn water_and_lava_side_by_side_layer_and_each_ledger_is_exact() {
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
        // and a pour onto the other fluid lies on it
        assert!(w.pour_kind(&g, col(12, 1), 1, U, crate::fluids::WATER), "water is poured onto lava");
        wet += U;
        for _ in 0..1000 {
            w.step(&g);
            assert_eq!((w.counted_of(crate::fluids::WATER), w.counted_of(LAVA)), (wet, hot));
        }
        // where both lie in a gap, the lava is under
        for (x, y) in (0..20).flat_map(|x| (0..3).map(move |y| (x, y))) {
            let l = layers(&w, x, y);
            assert!(l.windows(2).all(|p| p[0].0 == LAVA && p[1].0 == WATER), "({x}, {y}): {l:?}");
        }
        assert!((0..3).any(|y| layers(&w, 15, y).len() == 2), "the water ran out over the lava");
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

    /// A save from a build with more fluid rows: an unknown kind flows as the last row, it does not panic.
    #[test]
    fn a_kind_past_the_last_row_steps_as_the_last_row() {
        let mut g = Box3::new(6, 1, 6);
        g.fill(0..6, 0..1, 0..1, true);
        let mut w = stored(&g);
        w.pour_kind(&g, col(1, 0), 1, 2 * U, LAVA);
        let all: Vec<ColWater> = w.all().into_iter().map(|(c, gaps)| (c, gaps.into_iter().map(|(f, t, v)| (f, t, pack(9, unpack(v).1))).collect())).collect();
        let mut back = Liquids::default();
        back.restore(&g, &all);
        back.pour_kind(&g, col(1, 0), 1, U, 9); // wakes it
        for _ in 0..30 {
            back.step(&g);
        }
        assert_eq!(back.counted_of(9), 3 * U);
    }

    fn oil() -> u8 {
        crate::fluids::FLUIDS.kind("oil").expect("an oil row")
    }
    const WATER: u8 = crate::fluids::WATER;
    /// The wet layers of column (x, y)'s open gap, bottom-up, as (kind, units).
    fn layers(w: &Liquids, x: i32, y: i32) -> Vec<(u8, i64)> {
        w.column(col(x, y)).unwrap().filter(|n| n.ceil == OPEN && n.water > 0).map(|n| (n.kind, n.water)).collect()
    }
    /// A 12 x 12 pond two blocks deep, settled, with two blocks of oil poured on its middle.
    fn oiled_pond() -> (Box3, Liquids) {
        let mut g = Box3::new(12, 12, 8);
        g.fill(0..12, 0..12, 0..1, true);
        let mut w = stored(&g);
        for y in 0..12 {
            for x in 0..12 {
                w.pour(&g, col(x, y), 1, 2 * U);
            }
        }
        for _ in 0..120 {
            w.step(&g);
        }
        assert!(w.pour_kind(&g, col(6, 6), 1, 2 * U, oil()), "oil is poured onto water");
        (g, w)
    }

    /// Oil poured on a pond floats (liquids.md "Mixing in the world"): it spreads as a layer on top of the water and
    /// stays there, and every unit of both is kept at every step.
    #[test]
    fn oil_poured_on_a_pond_floats_on_top_and_both_ledgers_are_exact() {
        let oil = oil();
        let (g, mut w) = oiled_pond();
        for s in 0..60 * 60 {
            w.step(&g);
            assert_eq!((w.counted_of(WATER), w.counted_of(oil)), (144 * 2 * U, 2 * U), "step {s}");
        }
        let mut slick = 0;
        for y in 0..12 {
            for x in 0..12 {
                let l = layers(&w, x, y);
                if let Some(i) = l.iter().position(|&(k, _)| k == oil) {
                    slick += 1;
                    assert!(i == l.len() - 1 && l[0].0 == WATER && l.len() == 2, "({x}, {y}): oil on top of water, {l:?}");
                }
            }
        }
        assert!(slick > 8, "the oil spread as a layer: {slick} columns");
    }

    /// Water poured onto oil lands on top and sinks under it at a rate, not at once: a Rayleigh-Taylor overturn's
    /// fall, sqrt(A g l) / 4 (A = 150 / 1850, l a block, oil 4 times water's friction) = 0.22 m/s, 15 units a step:
    /// a block of water through in 270 steps (4.5 s) by hand. Then the stack lies still, water under oil.
    #[test]
    fn water_poured_on_oil_sinks_under_it_at_a_rate() {
        let oil = oil();
        let mut g = Box3::new(1, 1, 8);
        g.fill(0..1, 0..1, 0..1, true);
        let mut w = stored(&g);
        assert!(w.pour_kind(&g, col(0, 0), 1, 2 * U, oil));
        assert!(w.pour_kind(&g, col(0, 0), 1, U, WATER), "water is poured onto oil");
        assert_eq!(layers(&w, 0, 0), vec![(oil, 2 * U), (WATER, U)], "it lands on top");
        let mut t = 0;
        while layers(&w, 0, 0) != vec![(WATER, U), (oil, 2 * U)] {
            w.step(&g);
            t += 1;
            assert_eq!((w.counted_of(WATER), w.counted_of(oil)), (U, 2 * U), "step {t}");
            assert!(t < 60 * 60, "sank in an hour: {:?}", layers(&w, 0, 0));
        }
        eprintln!("a block of water sank through two of oil in {t} steps (by hand 270)");
        assert!((135..540).contains(&t), "{t} steps, by hand 270");
        for _ in 0..600 {
            w.step(&g);
        }
        assert_eq!((layers(&w, 0, 0), w.awake()), (vec![(WATER, U), (oil, 2 * U)], 0), "then it lies still");
    }

    /// A dam of oil breaks over a channel of still water: the oil runs out on top of the water, the water under it
    /// gives way, and neither fluid loses a unit.
    #[test]
    fn a_dam_break_of_oil_over_water_spreads_on_top() {
        let oil = oil();
        let mut g = Box3::new(60, 1, 8);
        g.fill(0..60, 0..1, 0..1, true);
        let mut w = stored(&g);
        for x in 0..60 {
            w.pour(&g, col(x, 0), 1, 2 * U);
        }
        for x in 0..10 {
            assert!(w.pour_kind(&g, col(x, 0), 1, 2 * U, oil));
        }
        for s in 0..60 * 30 {
            w.step(&g);
            assert_eq!((w.counted_of(WATER), w.counted_of(oil)), (60 * 2 * U, 10 * 2 * U), "step {s}");
        }
        let front = (0..60).rev().find(|&x| layers(&w, x, 0).iter().any(|l| l.0 == oil)).unwrap();
        eprintln!("the oil's front ran to column {front} in 30 s");
        assert!(front >= 20, "the oil ran out over the water: its front at {front}");
        for x in 0..60 {
            let l = layers(&w, x, 0);
            assert!(l[0].0 == WATER && l.iter().skip(1).all(|k| k.0 == oil) && l.len() <= 2, "column {x}: oil over water, {l:?}");
        }
    }

    /// Two fluids in one gap go through a save and to a client's copy as they are: each gap's layers bottom-up.
    #[test]
    fn layers_survive_a_save_and_reach_a_clients_copy() {
        let oil = oil();
        let (g, mut w) = oiled_pond();
        for _ in 0..600 {
            w.step(&g);
        }
        let all = w.all();
        assert!(all.iter().any(|(_, gaps)| gaps.len() > 1), "a column of two layers");
        let mut back = Liquids::default();
        back.restore(&g, &all);
        assert_eq!((back.all(), back.counted_of(oil), back.counted_of(WATER)), (all.clone(), 2 * U, 288 * U));
        w.take_changes();
        let mut client = Liquids::default();
        for t in w.wet_tiles(0, 0, 0, 0, 16) {
            client.set_tile(t, &w.tile_sent(t));
        }
        assert_eq!(client.all(), all);
        let tops = |w: &Liquids| (0..12).flat_map(|y| (0..12).map(move |x| (x, y))).map(|(x, y)| level(w, x, y)).collect::<Vec<_>>();
        assert_eq!(tops(&client), tops(&w), "the client sees the stack's tops where the server has them");
    }

    /// A stack at rest does not move (liquids.md, the rule): oil lying evenly on water in every column, stored as a save
    /// leaves it and woken, steps a minute and is what it was, asleep.
    #[test]
    fn a_stored_stack_at_rest_does_not_move() {
        let oil = oil();
        let mut g = Box3::new(8, 8, 8);
        g.fill(0..8, 0..8, 0..1, true);
        let mut cols: Vec<ColWater> = (0..8).flat_map(|y| (0..8).map(move |x| (col(x, y), vec![(1, OPEN, pack(WATER, 2 * U)), (1, OPEN, pack(oil, U / 2))]))).collect();
        cols.sort_unstable_by_key(|c| c.0);
        let mut w = Liquids::default();
        w.restore(&g, &cols);
        assert_eq!(w.all(), cols);
        for &(c, _) in &cols {
            w.pour_kind(&g, c, 1, 0, oil);
        }
        for _ in 0..60 * 60 {
            w.step(&g);
        }
        assert_eq!((w.all(), w.awake()), (cols, 0));
    }

    /// The cap of layers a gap holds (4; here 1, to reach it with these fluids): a pour of a new fluid onto a full gap
    /// is refused, a flow into one passes nothing, and an edit that joins two gaps of different fluids pushes the
    /// lighter out to `spilt` (water to its account): every ledger stays exact.
    #[test]
    fn the_layer_cap_keeps_every_ledger_exact() {
        let oil = oil();
        let mut g = Box3::new(6, 1, 8);
        g.fill(0..6, 0..1, 0..1, true);
        g.fill(3..4, 0..1, 0..8, true); // a wall
        let mut w = stored(&g);
        w.set_layers(1);
        for x in 0..3 {
            assert!(w.pour_kind(&g, col(x, 0), 1, 2 * U, WATER));
        }
        assert!(w.pour_kind(&g, col(4, 0), 1, 3 * U, oil) && w.pour_kind(&g, col(5, 0), 1, 3 * U, oil));
        assert!(!w.pour_kind(&g, col(1, 0), 1, U, oil), "a full gap takes no new fluid");
        // the wall falls: oil meets water, and only one of them may lie in a gap
        g.fill(3..4, 0..1, 1..8, false);
        w.regap(&g, col(3, 0));
        for s in 0..60 * 60 {
            w.step(&g);
            assert_eq!((w.counted_of(WATER), w.counted_of(oil)), (6 * U, 6 * U), "step {s}");
        }
        assert!((0..6).all(|x| layers(&w, x, 0).len() <= 1), "one layer a gap");
        // a block between a gap of oil and one of water is dug: the lighter of the joined gap is pushed out, booked
        let mut g = Box3::new(1, 1, 8);
        g.fill(0..1, 0..1, 0..1, true);
        g.set(0, 0, 3, true);
        let mut w = stored(&g);
        w.set_layers(1);
        assert!(w.pour_kind(&g, col(0, 0), 1, U, oil) && w.pour_kind(&g, col(0, 0), 4, U, WATER));
        g.set(0, 0, 3, false);
        w.regap(&g, col(0, 0));
        assert_eq!((w.counted_of(WATER), w.counted_of(oil), w.spilt.get(&oil).copied()), (U, U, Some(U)), "oil pushed out, booked");
        assert_eq!(layers(&w, 0, 0), vec![(WATER, U)]);
    }

    /// Lava under water (liquids.md "Mixing in the world"): one heat step freezes the lava's skin at the contact, a
    /// block of it, to basalt at its floor, and boils off the water that skin's heat can: by hand, the skin gives
    /// 2600 kg x (1200 J/(kg K) x (1150 - 100) K + 400 kJ/kg) = 4.316e9 J as it cools to the boil and freezes; water at
    /// 15 C takes 1000 kg/m3 x (4186 x 85 + 2.257e6) J/kg = 2.6128e9 J/m3 to boil off: 1.6519 blocks, 6766 units.
    /// The water lost is the vapour booked, the lava frozen is `froze`, and the energies balance to a unit's rounding.
    #[test]
    fn lava_under_water_freezes_its_skin_to_basalt_and_boils_the_water_by_its_heat() {
        let mut g = Box3::new(1, 1, 8);
        g.fill(0..1, 0..1, 0..1, true);
        let mut w = stored(&g);
        assert!(w.pour_kind(&g, col(0, 0), 1, 2 * U, LAVA));
        assert!(w.pour_kind(&g, col(0, 0), 1, 3 * U, WATER), "water is poured onto lava");
        assert_eq!(layers(&w, 0, 0), vec![(LAVA, 2 * U), (WATER, 3 * U)]);
        let out = w.cool(&g, 1.0);
        let (froze, steam) = (w.froze.get(&LAVA).copied().unwrap_or(0), w.vapour.get(&WATER).copied().unwrap_or(0));
        eprintln!("froze {froze} units of lava, boiled {steam} of water (by hand {U} and 6766)");
        assert_eq!(out, vec![(col(0, 0), 1, LAVA)], "a block of basalt at the lava's floor");
        assert_eq!(froze, U, "the skin, a block");
        let (e_l, e_w) = (2600.0 * (1200.0 * 1050.0 + 4.0e5) / U as f64, 1000.0 * (4186.0 * 85.0 + 2.257e6) / U as f64);
        assert!((steam as f64 * e_w - froze as f64 * e_l).abs() <= e_w, "energy: {} J boiled, {} J given", steam as f64 * e_w, froze as f64 * e_l);
        assert_eq!((steam, w.total_of(WATER), w.counted_of(WATER) + steam), (6766, 3 * U - 6766, 3 * U), "the water lost is the vapour");
        assert_eq!((w.total_of(LAVA), w.counted_of(LAVA) + froze), (U, 2 * U), "the lava left and what froze");
        assert!(!w.boils.is_empty(), "the steam's place, for its effect");
    }

    /// Lava's heat step never passes its place's heat (the review of the two constants, 2026-11-24): a thin sheet on
    /// ground at 1100 C, over its freeze point, stepped at a warp's coarse steps stays liquid between that heat and its
    /// pour heat and makes no rock.
    #[test]
    fn a_thin_lava_sheet_on_hot_ground_rests_liquid_at_any_step() {
        for dt in [100.0, 10_000.0] {
            let mut g = Box3::new(1, 1, 3);
            g.heat = 1100.0;
            let mut w = stored(&g);
            assert!(w.pour_kind(&g, col(0, 0), 0, U / 16, LAVA));
            for k in 0..50 {
                assert!(w.cool(&g, dt).is_empty(), "dt {dt}: no rock at step {k}");
                if let Some(t) = w.temp_at(col(0, 0), 0) {
                    assert!((1100.0..=1150.0).contains(&t), "dt {dt}: {t} C at step {k}");
                }
            }
            assert_eq!((w.froze.get(&LAVA).copied().unwrap_or(0), w.total_of(LAVA)), (0, U / 16), "dt {dt}: all of it liquid");
        }
    }

    /// The heat node a of `w` gives a second to air and rock (`loss`), at its own heat.
    fn lava_loss(w: &Liquids, g: &Box3, a: u32) -> f64 {
        let hr = crate::fluids::FLUIDS.now().get(LAVA).heat.clone().unwrap();
        w.loss(g, a, &w.heat[&a], w.rest_of(g, a), &hr).0
    }

    /// A face lava shares with another fluid gives nothing to the air (the review): that exchange is `react`'s alone.
    /// Lava beside water loses what lava beside lava (no flow through that face) does.
    #[test]
    fn lava_beside_water_loses_nothing_through_their_face() {
        let loss = |other: u8| {
            let g = Box3::new(2, 1, 3);
            let mut w = stored(&g);
            assert!(w.pour_kind(&g, col(0, 0), 0, U / 2, LAVA));
            assert!(w.pour_kind(&g, col(1, 0), 0, U, other));
            let a = *w.cols[&col(0, 0)].iter().find(|&&i| w.nodes[i as usize].kind == LAVA).unwrap();
            lava_loss(&w, &g, a)
        };
        let (by_water, by_lava) = (loss(WATER), loss(LAVA));
        assert!((by_water / by_lava - 1.0).abs() < 1e-9, "beside water {by_water} W, beside lava {by_lava} W");
    }

    /// One flow at a face of lava on ice (the review): what lava gives an ice block through their face is what the block
    /// takes, within 1 %, over a run.
    #[test]
    fn lava_beside_an_ice_block_gives_it_exactly_the_heat_it_takes() {
        let mut g = Box3::new(2, 1, 2);
        g.fill(0..2, 0..1, 1..2, true);
        g.set(1, 0, 0, true);
        g.ice.push((1, 0, 0));
        let mut w = stored(&g);
        w.watch_ice(col(1, 0), 0);
        assert!(w.pour_kind(&g, col(0, 0), 0, U, LAVA));
        for _ in 0..30 {
            w.cool(&g, 1.0);
        }
        let took = w.ice_melted(col(1, 0), 0).unwrap() * phases::Phases::water().lf;
        eprintln!("the lava gave {:.0} J through the face, the ice took {took:.0} J", w.ice_given);
        assert!(took > 0.0 && (w.ice_given / took - 1.0).abs() < 0.01, "gave {} J, took {took} J", w.ice_given);
    }

    /// Lava flowing into a pool beside it: where it meets the water its skin freezes and the water boils off, and
    /// every unit of both is booked (lava's frozen in `froze`, water's boiled in `vapour`).
    #[test]
    fn lava_flowing_into_a_pool_makes_rock_and_steam_and_the_ledgers_hold() {
        let mut g = Box3::new(12, 1, 8);
        g.fill(0..12, 0..1, 0..1, true);
        let mut w = stored(&g);
        for x in 4..12 {
            assert!(w.pour_kind(&g, col(x, 0), 1, 2 * U, WATER));
        }
        for x in 0..3 {
            assert!(w.pour_kind(&g, col(x, 0), 1, 3 * U, LAVA));
        }
        let mut blocks = Vec::new();
        for _ in 0..60 {
            for _ in 0..60 {
                w.step(&g);
            }
            blocks.extend(w.cool(&g, 1.0));
            let (froze, steam) = (w.froze.get(&LAVA).copied().unwrap_or(0), w.vapour.get(&WATER).copied().unwrap_or(0));
            assert_eq!((w.counted_of(WATER) + steam, w.counted_of(LAVA) + froze), (16 * U, 9 * U));
        }
        eprintln!("{} blocks of basalt, {:?} froze, {:?} boiled", blocks.len(), w.froze, w.vapour);
        assert!(!blocks.is_empty() && w.vapour.get(&WATER).is_some_and(|&v| v > 0), "rock and steam");
    }

    /// A block of lava frozen by water whose cell is taken waits as rock: it boils nothing more than its skin's heat
    /// did (a block put back as lava at its freeze point would boil the water again every second, from nothing).
    #[test]
    fn a_frozen_block_waiting_for_its_cell_boils_nothing_more() {
        let mut g = Box3::new(1, 1, 8);
        g.fill(0..1, 0..1, 0..1, true);
        let mut w = stored(&g);
        assert!(w.pour_kind(&g, col(0, 0), 1, U, LAVA) && w.pour_kind(&g, col(0, 0), 1, 5 * U, WATER));
        let mut steam = Vec::new();
        for s in 0..10 {
            for (c, z, k) in w.cool(&g, 1.0) {
                w.unfreeze(c, z, k); // a player stands in the cell
            }
            w.step(&g);
            steam.push(w.vapour.get(&WATER).copied().unwrap_or(0));
            assert_eq!(w.counted_of(LAVA) + w.froze.get(&LAVA).copied().unwrap_or(0), U, "lava's ledger, second {s}");
            assert_eq!(w.counted_of(WATER) + steam[s], 5 * U, "water's, second {s}");
        }
        assert!(steam[0] > 0 && steam.iter().all(|&s| s == steam[0]), "boiled once, by the skin: {steam:?}");
    }

    /// Two blocks frozen for one cell in a second: the first is set, the cell's gap moves up, and the second, its cell
    /// taken and its gap gone, still comes back the next second to be set (it was dropped, its units booked frozen).
    #[test]
    fn a_second_block_for_a_cell_whose_gap_is_gone_is_not_lost() {
        let mut g = Box3::new(1, 1, 8);
        g.fill(0..1, 0..1, 0..1, true);
        let mut w = stored(&g);
        assert!(w.pour_kind(&g, col(0, 0), 1, 2 * U, LAVA) && w.pour_kind(&g, col(0, 0), 1, 5 * U, WATER));
        let out = w.cool(&g, 1.0);
        assert_eq!(out, vec![(col(0, 0), 1, LAVA)]);
        g.set(0, 0, 1, true);
        w.regap(&g, col(0, 0));
        w.unfreeze(col(0, 0), 1, LAVA); // a second block for the same cell, now basalt
        assert!(w.cool(&g, 1.0).contains(&(col(0, 0), 1, LAVA)), "it waits, to be set");
    }

    /// A layer frozen through under another is emptied as every layer is: its node goes and no dry node of water's
    /// kind is left under the oil for the oil to flow into as water.
    #[test]
    fn a_layer_frozen_through_under_oil_leaves_no_water_behind() {
        let oil = oil();
        let mut g = Box3::new(3, 1, 8);
        g.fill(0..3, 0..1, 0..1, true);
        let mut w = stored(&g);
        assert!(w.pour_kind(&g, col(0, 0), 1, U / 8, LAVA));
        for x in 0..3 {
            assert!(w.pour_kind(&g, col(x, 0), 1, U, oil));
        }
        let all_oil = |w: &Liquids, when: &str| {
            for x in 0..3 {
                let kinds: Vec<(u8, i64)> = w.column(col(x, 0)).unwrap().map(|n| (n.kind, n.water)).collect();
                assert!(kinds.iter().all(|k| k.0 == oil), "{when}, column {x}: {kinds:?}");
            }
        };
        for t in 0..100_000 {
            w.cool(&g, 60.0);
            if w.total_of(LAVA) == 0 {
                break;
            }
            assert!(t < 99_999, "the lava froze through");
            for _ in 0..4 {
                w.step(&g);
            }
        }
        all_oil(&w, "as it froze through");
        for _ in 0..600 {
            w.step(&g);
        }
        all_oil(&w, "after");
        assert_eq!((w.total_of(WATER), w.counted_of(oil)), (0, 3 * U));
    }

    /// Lava at rest in a hot place (no heat of its own held, not awake) touching water at rest reacts all the same:
    /// under it in its gap, and beside it across a link.
    #[test]
    fn lava_and_water_at_rest_in_a_hot_place_still_react() {
        for beside in [false, true] {
            let mut g = Box3::new(2, 1, 8);
            g.fill(0..2, 0..1, 0..1, true);
            g.heat = 1150.0;
            let mut w = stored(&g);
            assert!(w.pour_kind(&g, col(0, 0), 1, U, LAVA) && w.pour_kind(&g, col(beside as i32, 0), 1, U, WATER));
            for a in std::mem::take(&mut w.awake) {
                w.nodes[a as usize].quiet = u16::MAX;
            }
            w.heat.clear();
            w.cool(&g, 1.0);
            assert!(w.vapour.get(&WATER).is_some_and(|&v| v > 0), "beside {beside}: they met and the water boiled");
        }
    }

    /// Lava poured on a full stack (lava, water, oil) sinks through to its own layer at the bottom, with no room for
    /// a fifth layer on the way.
    #[test]
    fn a_denser_layer_sinks_to_its_own_at_the_cap() {
        let oil = oil();
        let mut g = Box3::new(1, 1, 8);
        g.fill(0..1, 0..1, 0..1, true);
        let mut w = stored(&g);
        for (k, u) in [(LAVA, U), (WATER, U), (oil, U), (LAVA, U / 2)] {
            assert!(w.pour_kind(&g, col(0, 0), 1, u, k));
        }
        assert_eq!(layers(&w, 0, 0).len(), 4, "the cap");
        for _ in 0..3600 {
            w.step(&g);
        }
        assert_eq!(layers(&w, 0, 0), vec![(LAVA, 3 * U / 2), (WATER, U), (oil, U)]);
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
