//! Terrain: bodies on the equi-angular cube-sphere grid (ADR 0003), generated from the seed, plus a sparse
//! edit map. A block is (face, column i, column j, layer); layers are geometric shells, so blocks stay cubic.
use crate::bake::{life, Map};
use crate::biomes::{REACH, ROWS, SPACING, TALLEST};
use crate::blocks::{AIR, BLOCKS};
use crate::rock::{Rock, ROCKS};
use crate::bodies::{Generator, Row, BODIES};
use crate::interior::Interior;
use crate::liquid::{Col, Ground, Liquids, OPEN, TILE, U};
use crate::sky::{Pose, Sky, Tide};
use fastnoise_lite::{FastNoiseLite, FractalType, NoiseType};
use glam::{DQuat, DVec3, IVec3};
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::f64::consts::{FRAC_PI_2, PI};
use std::sync::Arc;
mod body;
mod heights;
mod solid;
pub use solid::*;
mod frames;
pub use frames::lazy_row;
mod edits;
mod far;
mod water;
pub use water::*;
mod generate;
pub use generate::*;
mod volcano;
pub use volcano::{Chamber, Crater, Stage};
mod felt;
pub use felt::{operative, ray_dir, Felt, Rays, FRAME_PROBES, RAYS, RAY_REACH};
mod ground;

pub const BLOCK: f64 = 1.0;
/// The planet's sea lies this far under its nominal radius (ADR 0020); the continents decide how much of it
/// is sea (ADR 0021).
pub const SEA_DEPTH: f64 = 4.0;
/// Beds under water (ADR 0086, `Body::bed`): ground steeper than this (rise over run, across the map's cells)
/// is bare rock under any water: past sediment's angle of repose, about 35 degrees, nothing settles.
pub const BED_STEEP: f64 = 0.7;
/// A river falling this much a metre or more runs fast enough to leave gravel; under BED_SLOW it drops clay,
/// as it does BED_DEEP_RIVER metres down.
pub const BED_RAPID: f64 = 0.01;
pub const BED_SLOW: f64 = 0.002;
pub const BED_DEEP_RIVER: f64 = 2.0;
/// A lake's shallows are sand to this depth, its middle clay; the sea's beach and shelf sand to BED_SHELF, mud on
/// clay further out.
pub const BED_SHALLOW_LAKE: f64 = 1.5;
pub const BED_SHELF: f64 = 8.0;
/// Fresh water shallower than this over land whose band is dirt lies on that soil: a pond's edge, and a stream
/// carrying under BED_STREAM m^2/s a metre of its width.
pub const BED_SOIL: f64 = 0.5;
pub const BED_STREAM: f64 = 0.05;
/// The share of the planet's surface the continents leave to the sea (ADR 0021).
pub const OCEAN: f64 = 0.55;
pub const CS: i32 = 32; // cells per chunk edge
pub const P: usize = 34; // padded edge
/// No generated surface lies farther than this from a body's radius (m), plants included: a baked planet's
/// stretched peaks (350 m on the 4 km planet, ADR 0105; 140 before) and its tallest tree.
pub const AMP: f64 = 400.0;
/// The noise land's bound (ADR 0021), AMP before ADR 0043.
const NOISE_AMP: f64 = 80.0;
/// The deepest a baked sea floor lies under the sea level, metres: the bake's 60 m would reach `r0`.
const FLOOR: f64 = CRUST - SEA_DEPTH - 4.0;
/// The ground of a baked planet is this rough over its map's smooth 8 m cells, metres (ADR 0043).
const DETAIL_M: f64 = 1.0;
/// Biome borders and ground patches wander by about this many columns (ADR 0043).
const JITTER: f64 = 6.0;
/// A ground patch is about this many columns across.
const PATCH: f64 = 3.0;
/// The relief of a body without continents (the moon): +-30 m of hills.
const HILLS: f64 = 30.0;
pub const CRUST: f64 = 64.0;
/// How deep the ground goes (W4, ADR 0063): at least this far under the radius.
pub const DEEP: f64 = 192.0;
/// The mantle's rock is asked for only under this many metres over `DEEP` (`Body::rock`): the crust's floor
/// rises about 40 m under the hottest rift, so above this it is crust everywhere.
pub const MANTLE_LIFT: f64 = 64.0;
/// One atmosphere, MPa, and fresh water's density, kg/m3 (`Body::pressure`).
pub const ATM: f64 = 0.101_325;
pub const WATER_DENSITY: f64 = 1000.0;
/// The width of the slow wave on the mantle's boundaries, metres (`Rock::wobble`).
const WOBBLE_M: f64 = 600.0;

/// The layers under the old 64 m `CRUST` on a body of `n` columns per face edge, so the crust is `DEEP` m: on
/// one big enough to keep half its radius (Terra, and Luna from the 4 km planet up), else none. Every layer
/// above keeps its radius and gains this in index (saves of version 8 and older shift their cells).
pub fn deep_layers(n: i32) -> i32 {
    let radius = n as f64 * BLOCK / (2.0 * PI / 3.0).sqrt();
    if radius < 4.0 * DEEP {
        return 0;
    }
    // layers thin with depth: whole ones down to DEEP
    let old = radius - CRUST;
    (-(1.0 - (DEEP - CRUST) / old).ln() / (1.0 + BLOCK / radius).ln()).ceil() as i32
}

/// The layers of the mantle (ADR 0152, step 1) under the crust's `DEEP` m on a body of `n` columns per face edge
/// that has a deep crust (`deep_layers`), else none: the same grid carried on down to about its core's top, `core`
/// of its radius (`Interior::core_share`; R/sqrt 2 on Terra), a whole number
/// of `MANTLE_STEP` layers, so the coarse levels' chunks and cells stand on the layers they stood on before the
/// mantle and a deep world's surface is drawn as an old one's (1280 on the 4 km planet, 4 m under R/sqrt 2).
pub const MANTLE_STEP: i32 = 256;

/// A new world's generation (`Body::gen`): 1 the mantle and the caves by rock (ADR 0152, 0159), 2 the arcs'
/// basalt flows and lava tubes as one round passage (ADR 0160-generation-number-and-arc-basalt), 3 dry karst in
/// the basins' limestone in place of ADR 0063's worm (0161-karst-caves), 4 Luna's rock field (0165-bake-stages:
/// anorthosite highlands, mare basalt over anorthosite in the maria; Terra as at 3), 5 Mars, the third body
/// (`bodies::Row::since`; Terra and Luna as at 4), 6 the gas giant, the fourth (step 6a; the three as at 5).
pub const GENERATION: u8 = GROUND_WATER;
/// The generation of real gravity (ADR 0181): every body's pull from its own mass (Luna of the Moon's
/// make-up, `bodies::Row::makeup_at`), the periods by Kepler's law from the masses, the sun's mass from its row,
/// tides from the masses and distances, water by its body's pull, and Terra's map baked over that year. A world of
/// an earlier generation keeps all of it as it was (its recordings replay to the bit).
pub const REAL: u8 = 8;
/// The generation of the deep grid on every body (the developer, 2026-11-23: "everything gets a deep grid"): every
/// body with blocks big enough for the deep crust, not the home alone, has its grid carried down to its core's top
/// (`Body::with_mantle`); a liquid core is a metal sea under it, a solid one a closed floor of metal.
pub const DEEP_BODIES: u8 = 9;
/// The generation of Terra's volcanoes (volcanoes.md steps 1 and 2, draft 2026-11-23): her map of version 9, whose
/// arcs hold cones, and under each live one a chamber, a conduit and a crater lake of generated lava, with the
/// heat round them (`volcano`). A world of an earlier generation plays on the map of version 8, as before.
pub const VOLCANOES: u8 = 10;
/// The generation of the geotherm (draft 2026-11-24): the rock's heat under the ground is a sphere's steady
/// conduction from the province's gradient at the top (`Body::geotherm`), not a straight line; it reads in the deep
/// minerals' windows, the mantle's rock and generated lava, so an earlier generation keeps the line.
pub const GEOTHERM: u8 = 11;
/// The generation of ground water (ADR 0190): Terra's map of version 10, whose water table floods the caves under it
/// and whose springs start streams, and the aquifer a well or a cave trades with (`liquid::Liquids::seep`). A world of
/// an earlier generation plays on the map of version 9 and stays dry, as before.
pub const GROUND_WATER: u8 = 12;

/// The home's year in a world of generation `gen` whose home has `planet_n` columns and its moon its row's,
/// seconds: the sky's own (built without a map), which the bake runs its climate over (`bake::Map::year`).
pub fn year_of(planet_n: i32, gen: u8) -> f64 {
    World::with_grid(planet_n, crate::bodies::moon_columns(planet_n), 0, None, None, 0.0, gen).sky.year
}

/// Refuses a world of a generation newer than this build's: it would generate other ground under the edits.
pub fn known_generation(gen: u8) -> Result<u8, String> {
    if gen <= GENERATION { Ok(gen) } else { Err(format!("this world is generation {gen}, this build knows up to {GENERATION}: update the game")) }
}
pub fn mantle_layers(n: i32, core: f64) -> i32 {
    let deep = deep_layers(n);
    if deep == 0 {
        return 0;
    }
    let radius = n as f64 * BLOCK / (2.0 * PI / 3.0).sqrt();
    let lnq = (1.0 + BLOCK / radius).ln();
    let floor = (radius - CRUST) * (-lnq * deep as f64).exp();
    ((floor / (radius * core)).ln() / lnq / MANTLE_STEP as f64).round() as i32 * MANTLE_STEP
}

/// FxHash-style hasher for the maps keyed by chunks and cells: SipHash cost a third of the streamer's selection
/// time, and every block lookup goes through `World::edits`.
#[derive(Default)]
pub struct Fx(u64);
impl Hasher for Fx {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, b: &[u8]) {
        for x in b {
            self.write_u64(*x as u64);
        }
    }
    fn write_u8(&mut self, v: u8) {
        self.write_u64(v as u64)
    }
    fn write_i32(&mut self, v: i32) {
        self.write_u64(v as u32 as u64)
    }
    fn write_u64(&mut self, v: u64) {
        self.0 = (self.0.rotate_left(5) ^ v).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}
pub type FxMap<K, V> = HashMap<K, V, BuildHasherDefault<Fx>>;

/// FNV-1a: stable across runs, builds and compiler versions, so a hash can be stored next to a replay (ADR 0011).
pub fn fnv(h: u64, bytes: &[u8]) -> u64 {
    bytes.iter().fold(h, |h, &b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}
pub const FNV0: u64 = 0xcbf29ce484222325;

fn fnv_ivec(h: u64, p: IVec3) -> u64 {
    [p.x, p.y, p.z].iter().fold(h, |h, c| fnv(h, &c.to_le_bytes()))
}
/// Builds may rise this far above the highest terrain and still be streamed.
pub const BUILD_HEADROOM: f64 = 16.0;
/// Border walls reach this many cells under the surface (see `generate`).
pub const SKIRT: usize = 2;

const fn v(x: i32, y: i32, z: i32) -> IVec3 {
    IVec3::new(x, y, z)
}
/// [face][n, s, t]: the face normal and its two in-face axes, s x t = n. Columns i run along s, j along t.
pub const AX: [[IVec3; 3]; 6] = [
    [v(1, 0, 0), v(0, 1, 0), v(0, 0, 1)],
    [v(-1, 0, 0), v(0, 0, 1), v(0, 1, 0)],
    [v(0, 1, 0), v(0, 0, 1), v(1, 0, 0)],
    [v(0, -1, 0), v(1, 0, 0), v(0, 0, 1)],
    [v(0, 0, 1), v(1, 0, 0), v(0, 1, 0)],
    [v(0, 0, -1), v(0, 1, 0), v(1, 0, 0)],
];

/// Column (i, j) of a face with `n` columns per edge, folded onto the neighbouring face when it lies outside.
/// Only one of i, j may be outside: the diagonal across a cube corner does not exist (it gets some valid cell).
pub fn fold(face: usize, i: i32, j: i32, n: i32) -> (usize, i32, i32) {
    if (0..n).contains(&i) && (0..n).contains(&j) {
        return (face, i, j);
    }
    let [nn, s, t] = AX[face];
    // cell centre as an integer point on the cube of half-size n; the part past the edge bends down the side
    let (a, b) = (2 * i + 1 - n, 2 * j + 1 - n);
    let p = if a.abs() > n {
        s * a.signum() * n + nn * (2 * n - a.abs()) + t * b.clamp(1 - n, n - 1)
    } else {
        t * b.signum() * n + nn * (2 * n - b.abs()) + s * a
    };
    let f = (0..6).find(|f| AX[*f][0].dot(p) == n).unwrap();
    (f, (AX[f][1].dot(p) + n - 1) / 2, (AX[f][2].dot(p) + n - 1) / 2)
}

/// Column (i, j) of `face` in the grid of `g`, a face next to it, carried on over their seam: `fold`'s inverse,
/// past one edge of `g` (ADR 0100).
pub fn unfold(g: usize, face: usize, i: i32, j: i32, n: i32) -> (i32, i32) {
    let [nf, s, t] = AX[face];
    let p = nf * n + s * (2 * i + 1 - n) + t * (2 * j + 1 - n);
    let k = AX[g][0].dot(p);
    // the part of the side face below g's edge bends up into g's plane, past the edge
    let q = p + (AX[g][0] + nf) * (n - k);
    ((AX[g][1].dot(q) + n - 1).div_euclid(2), (AX[g][2].dot(q) + n - 1).div_euclid(2))
}

/// A ball of air in the ground, in a face's grid: `at` is (column i, column j, layer), `r` in blocks. A kind's
/// (`data/caves.toml`) may stretch it: `h` layers up and down instead of `r` (a fissure's tall slot), and cut
/// flat `floor` layers under its middle (a tube's floor); a plain ball has both `r` and `kind` `Cave::BALL`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cave {
    pub face: u8,
    pub at: DVec3,
    pub r: f64,
    pub h: f64,
    pub floor: f64,
    pub kind: u8,
}

impl Cave {
    /// ADR 0063's caves and Luna's: no row of `data/caves.toml`.
    pub const BALL: u8 = u8::MAX;
    pub fn ball(face: u8, at: DVec3, r: f64) -> Cave {
        Cave { face, at, r, h: r, floor: r, kind: Cave::BALL }
    }
    /// The layers [lo, hi) of column (i, j) it hollows, at layer 0 and up: the cells whose middle lies inside it.
    pub fn span(&self, i: i32, j: i32) -> Option<(i32, i32)> {
        let d2 = (i as f64 + 0.5 - self.at.x).powi(2) + (j as f64 + 0.5 - self.at.y).powi(2);
        if d2 >= self.r * self.r {
            return None;
        }
        // (h / r is 1 on a ball, to the bit)
        let hh = (self.r * self.r - d2).sqrt() * (self.h / self.r);
        let (lo, hi) = (((self.at.z - hh - 0.5).ceil() as i32).max(0), (self.at.z + hh - 0.5).floor() as i32 + 1);
        let lo = lo.max((self.at.z - self.floor - 0.5).ceil() as i32);
        (lo < hi).then_some((lo, hi))
    }
}

/// One footprint's cave spans by column (`Body::cave_cols`): the columns any cave hollows, sorted, each with
/// the start of its spans in `spans` (the next entry's start ends them; the last entry is an end mark), its
/// spans the union of every cave's there, sorted and disjoint: a tube's column holds some 190 balls' spans.
#[derive(Default)]
pub(crate) struct CaveCols {
    cols: Vec<(u16, u32)>,
    spans: Vec<(i32, i32)>,
}

impl CaveCols {
    /// Column k's spans (k = di + dj CS inside the footprint).
    fn get(&self, k: u16) -> &[(i32, i32)] {
        match self.cols[..self.cols.len() - 1].binary_search_by_key(&k, |c| c.0) {
            Ok(n) => &self.spans[self.cols[n].1 as usize..self.cols[n + 1].1 as usize],
            Err(_) => &[],
        }
    }
}

/// The light a layer-0 cell open down to the metal sea gives (draft the metal sea): a warm lamp's, as strong as the lamp block (25 open cells at 3.0 burnt a 5 x 5 shaft white).
pub const CORE_LIGHT: [f32; 3] = [1.0, 0.66, 0.26];
/// A solid core's metal: layer 0 of a deep body without a metal sea (`Body::rock`).
static CORE_METAL: std::sync::LazyLock<u16> = std::sync::LazyLock::new(|| BLOCKS.id("iron-nickel"));

pub struct Body {
    /// Its row's name in `data/bodies.toml`: Terra, Luna.
    pub name: String,
    /// Whether it has air (the sky's colour, ADR 0022).
    pub air: bool,
    /// A gas body (its row's `kind`, step 6a): no blocks. Its grid's numbers are only its radius's; every cell
    /// is air and takes no edit (`World::block`, `set_block`), its surface is its cloud tops (`height`), nothing
    /// streams it, and the cloud tops hold a player (phys `blocked`).
    pub gas: bool,
    /// An icy body (its row's `kind`): its core is rock, not metal (`rock`'s floor).
    pub icy: bool,
    /// The body it circles (its row's `parent`, 0163-n-bodies); INERTIAL: the sun.
    pub parent: usize,
    /// How far out it circles its parent, centre to centre, metres (the sun's distance for one that circles the
    /// sun); and once in how many seconds (`sky::period`). The home's are the sky's.
    pub orbit: f64,
    pub period: f64,
    /// Its world's year (the home's period, `sky::Sky::year`), seconds: what its seasons and its plants' year
    /// turn by (`clutter::part`), so the mesher that has only the body has it.
    pub year: f64,
    /// The highest its sea's tide stands over the mean, metres (`sky::Sky::tide_high`): its sea's quads are meshed
    /// for it (`sea_high`).
    pub tide_high: f64,
    /// Its turn in seconds, a whole fraction of `sky::PERIOD`, for one that circles the sun and is not home, from
    /// its row's `day` (0: it does not turn; the giant's bands turn in the client, so its pose does not).
    pub day: f64,
    pub centre: DVec3,
    /// Nominal surface radius: the mean block there is BLOCK wide.
    pub radius: f64,
    /// Columns per face edge: 32 * m * 2^root_level with a small m, so the chunk quadtree tiles the face.
    pub n: i32,
    pub root_level: u8,
    /// Radius of the bottom of layer 0. Nothing below it is stored, meshed or dug.
    pub r0: f64,
    /// Each layer is exp(lnq) times thicker than the one below.
    pub lnq: f64,
    /// The block grid's warp towards the conformal cube-sphere (a trial, ADR 0131; `warp.rs`): 0 today's
    /// equi-angular grid, to the bit; 1 square block tops everywhere. Set before the body is given its map.
    pub warp: f64,
    /// Layers 0..layers are streamed (terrain plus build headroom).
    pub layers: i32,
    /// The mantle's layers under the crust (ADR 0152; `with_mantle`): a new world's planet has `mantle_layers`,
    /// a body of a save from before and every other body none. Layer z here is layer z - mantle of the body
    /// without it, at the same radius.
    pub mantle: i32,
    /// The world's generation (ADR 0160-generation-number-and-arc-basalt): which generation rules its ground
    /// was made by, kept in its save and sent to every client. 0 the worlds before the mantle, 1 the mantle's
    /// (ADR 0154), `GENERATION` a new world's. A rule new in a generation asks `gen >=` it. Every body has its world's (before 4, the planet's alone was set).
    pub gen: u8,
    noise: FastNoiseLite,
    /// The sea level as a layer coordinate; generation fills every column lower than it with water.
    pub sea: Option<f64>,
    /// The liquid metal sea's level under the mantle as a layer coordinate (draft the metal sea): one level for the
    /// whole body, generated and stored nowhere, like `sea`; a body with a mantle has it at the bottom of layer 0.
    pub core: Option<f64>,
    /// What lies under its ground, from its row (0162-planet-interior): zones, mass, gravity, the weight of rock.
    pub interior: Interior,
    /// Continents and oceans (ADR 0021): the planet has them, the moon is all hills.
    land: Option<Box<Land>>,
    /// Where players start: the land nearest the +y pole (a direction from the centre).
    pub spawn: DVec3,
    /// The second moment of the sea's area: the mean of d d^T over its directions (ADR 0067; a third of the
    /// identity with no sea), which keeps the tide's volume (`Sky::tide`).
    pub ocean: glam::DMat3,
    /// What generation lays down: the top, the band under it, the rock below (block ids).
    pub ground: [u16; 3],
    /// Generated hollows in the ground (ADR 0062, 0063), in the order they were made: every reader asks
    /// `col_caves`, none looks here itself. Set with `set_caves`.
    pub caves: Vec<Cave>,
    /// The caves by (face, level-0 chunk x, y) of every footprint a ball reaches, with a column to spare.
    cave_tiles: FxMap<(u8, i32, i32), Vec<u32>>,
    /// Every column's cave spans, merged (`col_caves`), by the same footprints: made once in `set_caves`.
    cave_cols: FxMap<(u8, i32, i32), CaveCols>,
    /// Ore bodies (ADR 0063), in the order they were made: their index is their id in the ledger. Every reader
    /// asks `ore_at` or `col_ores`. Set with `set_ores`.
    pub ores: Vec<crate::ore::OreBody>,
    ore_tiles: FxMap<(u8, i32, i32), Vec<u32>>,
    /// Each body's layers at most, [lo, hi]: a chunk outside them keeps its shortcuts (`generate`).
    ore_z: Vec<(f64, f64)>,
    /// The column heights of chunk footprints, shared by every consumer (ADR 0025, E).
    tiles: HeightCache,
    /// A baked planet's map and the noise over it (ADR 0043); None: the noise land of ADR 0021.
    baked: Option<Box<Baked>>,
}

struct Baked {
    map: Arc<Map>,
    /// `candidate` by (face, cx, cy), for lookups without a cached tile (the server's physics builds none);
    /// past `SPOTS` it is emptied.
    spots: std::sync::Mutex<FxMap<(u8, i32, i32), Option<Foot>>>,
    /// `river_quads`' level grid by chunk column (face, level, x, y): generated water, which edits never change,
    /// so a chunk meshed again after an edit looks nothing up; past `LEVEL_GRIDS` it is emptied.
    levels: std::sync::Mutex<FxMap<(u8, u8, i32, i32), Arc<Vec<Option<f64>>>>>,
    detail: FastNoiseLite,
    jitter: FastNoiseLite,
    /// The rock layers' folds (`rock_at`), frequency 1: sampled at metres over a province's wavelength.
    fold: FastNoiseLite,
    /// Each province's lava flow's cut on its noise (`Body::flow`); infinite without one.
    flow_cut: Vec<f32>,
    /// Its map's volcanoes in the grid (`Body::craters`; a map of version 9 on).
    craters: Vec<Crater>,
    seed: u64,
}

/// No plant block stands higher than this over the ground of its own column, metres: the tallest plant on
/// ground up to 8 m higher a column out, which the map's 8 m spline never makes (the test checks the margin).
/// shortcut: a margin, not a bound; a cliff steeper than 8 under a crown would lose its leaves to physics.
pub const PLANT_TOP: f64 = (TALLEST + 2 + 8 * REACH) as f64;

/// Plant spots kept per body for lookups without a tile: about 2 MB.
const SPOTS: usize = 65536;
/// Level grids kept per body: 18 KB each, 9 MB at most (only chunks near a river or a lake get one).
const LEVEL_GRIDS: usize = 512;

/// A plant's place (ADR 0043): its kind (`biomes::ROWS.plants`), height in blocks, trunk column and the layer
/// of its foot, the first air over the ground there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Foot {
    pub plant: usize,
    pub h: i32,
    /// The tree's own hash: where its limbs go, which leaves its crown lets fall (ADR 0092).
    pub v: u32,
    pub i: i32,
    pub j: i32,
    pub z: i32,
}

pub(crate) fn mix(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}

/// A number in 0..1 from a hash.
pub(crate) fn unit(h: u64) -> f32 {
    (h >> 40) as f32 / (1u64 << 24) as f32
}

/// The surface radius of every column of a chunk's footprint with its padding (`generate`'s P x P), a pure
/// function of (face, level, x, y): the chunks stacked over it, its remeshes and its sea share one.
pub struct HeightTile {
    pub h: [f64; P * P],
    /// Each column's ground (`material`): the top, the band, the rock.
    pub g: [[u16; 3]; P * P],
    /// Each column's rock under the band (`rock::Rocks::at`).
    pub rock: [Rock; P * P],
    /// Plant blocks over the ground (level 0 of a baked planet only): (column, layer, block), by column then
    /// layer. They stand where the generated ground is air; an edit wins over them.
    pub plants: Vec<(u16, i32, u16)>,
    /// Each column's biome (0 on a planet with no map), and the trunks of the plants within `REACH` of the
    /// footprint (level 0 of a baked planet): what clutter asks (ADR 0082).
    pub biome: [u8; P * P],
    pub trees: Vec<(i32, i32)>,
    /// Of the heights as `f32`, for `generate`'s early-outs.
    pub min: f32,
    pub max: f32,
}

/// Tiles by (face, level, x, y) with the tick each was last used; past `HEIGHT_TILES` the older half goes.
/// The seam for world generation: per-column layers computed once, then sampled by every consumer.
#[derive(Default)]
struct HeightCache {
    map: std::sync::Mutex<FxMap<(u8, u8, i32, i32), (Arc<HeightTile>, u64)>>,
    tick: std::sync::atomic::AtomicU64,
}

/// About 9 KB each: 37 MB at most, more than the streamer's footprints at default detail.
const HEIGHT_TILES: usize = 4096;

/// Continents, oceans and islands (ADR 0021). Heights are metres over the sea level. The continents' sizes
/// scale with the planet (3 to 5 of them on any size); hills, mountains, islands and depths are in metres.
struct Land {
    warp: FastNoiseLite,
    cont: FastNoiseLite,
    hills: FastNoiseLite,
    ridge: FastNoiseLite,
    isle: FastNoiseLite,
    /// The continent value at the coast: OCEAN of the surface lies under it.
    cut: f32,
    /// The sea level's radius.
    sea_r: f64,
    /// Heights are in metres from 1 km radius up; a smaller (test) planet is flatter in proportion.
    relief: f64,
}

fn noise(seed: i32, freq: f64, octaves: i32) -> FastNoiseLite {
    let mut n = FastNoiseLite::with_seed(seed);
    n.set_noise_type(Some(NoiseType::OpenSimplex2));
    n.set_fractal_type(Some(FractalType::FBm));
    n.set_fractal_octaves(Some(octaves));
    n.set_frequency(Some(freq as f32));
    n
}

/// The detail of ground `a` metres rough (`bake::rough`) at detail noise value `n` (ADR 00XX): rounded
/// swells on gentle ground, turning into sharp-crested crags (the noise folded at 0) where it is rougher
/// than a metre.
fn detail(a: f64, n: f64) -> f64 {
    let c = smooth01((a - 1.0) / 1.5);
    a * ((1.0 - c) * n + c * 1.6 * (0.25 - n.abs()))
}

fn smooth01(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// Directions spread evenly over the sphere (a Fibonacci lattice).
fn lattice(n: usize) -> impl Iterator<Item = DVec3> {
    let ga = PI * (3.0 - 5f64.sqrt());
    (0..n).map(move |k| {
        let y = 1.0 - 2.0 * (k as f64 + 0.5) / n as f64;
        let r = (1.0 - y * y).sqrt();
        DVec3::new(r * (ga * k as f64).cos(), y, r * (ga * k as f64).sin())
    })
}

impl Land {
    fn new(seed: i32, radius: f64) -> Land {
        let mut l = Land {
            warp: noise(seed ^ 0x51, 1.6 / radius, 2),
            cont: noise(seed ^ 0x52, 0.75 / radius, 3),
            hills: noise(seed ^ 0x53, 1.0 / 140.0, 4),
            ridge: noise(seed ^ 0x54, 1.0 / 650.0, 4),
            isle: noise(seed ^ 0x55, 1.0 / 260.0, 3),
            cut: 0.0,
            sea_r: radius - SEA_DEPTH,
            relief: (radius / 1000.0).min(1.0),
        };
        let mut v: Vec<f32> = lattice(4096).map(|d| l.continent(d * radius)).collect();
        v.sort_by(f32::total_cmp);
        l.cut = v[(OCEAN * v.len() as f64) as usize];
        l
    }
    /// The continent value at p (m from the centre), its coasts warped so they wander.
    fn continent(&self, p: DVec3) -> f32 {
        let (x, y, z) = (p.x as f32, p.y as f32, p.z as f32);
        let k = 0.22 * p.length() as f32;
        let w = |o: f32| self.warp.get_noise_3d(x + o, y - o, z + 2.0 * o) * k;
        self.cont.get_noise_3d(x + w(0.0), y + w(311.0), z + w(-517.0))
    }
    /// Metres over the sea level at p.
    fn over_sea(&self, p: DVec3) -> f64 {
        let (x, y, z) = (p.x as f32, p.y as f32, p.z as f32);
        let e = (self.continent(p) - self.cut) as f64;
        let hills = self.hills.get_noise_3d(x, y, z) as f64;
        let h = if e >= 0.0 {
            // plains rising inland, hills, and mountain ranges well inside the continents
            let inland = smooth01(e / 0.15);
            let mut h = 14.0 * inland + hills * (1.0 + 9.0 * smooth01(e / 0.06));
            if e > 0.06 {
                let r = 1.0 - (self.ridge.get_noise_3d(x, y, z) as f64).abs();
                h += 45.0 * r * r * r * smooth01((e - 0.06) / 0.15);
            }
            h
        } else {
            // a shelf off the coast, then the deep; islands rise from it
            let mut h = -23.0 * smooth01(-e / 0.1) + 1.5 * hills;
            if e > -0.25 {
                let i = self.isle.get_noise_3d(x, y, z) as f64 - 0.3;
                if i > 0.0 {
                    h += i * 110.0 * (1.0 - 0.5 * smooth01(-e / 0.25));
                }
            }
            h
        };
        // the bound before the bake raised AMP: old worlds keep their heights to the bit
        (h * self.relief).clamp(SEA_DEPTH - NOISE_AMP + 2.0, NOISE_AMP - SEA_DEPTH - 2.0)
    }
}

#[derive(Clone, Copy)]
pub struct Bounds {
    pub c: DVec3,
    pub rad: f64,
}

/// The blocks generation names, looked up once in the registry: grass, dirt, stone.
/// The beds' blocks (ADR 0086, `Body::bed`), looked up once: sand, clay, mud, gravel, dirt.
static BED: std::sync::LazyLock<[u16; 5]> =
    std::sync::LazyLock::new(|| ["sand", "clay", "mud", "gravel", "dirt"].map(|n| BLOCKS.id(n)));
static GEN: std::sync::LazyLock<[u16; 3]> =
    std::sync::LazyLock::new(|| [BLOCKS.id("grass"), BLOCKS.id("dirt"), BLOCKS.id("stone")]);

/// The generated block `depth` metres under the surface, from a body's `ground`.
pub fn material(depth: f64, ground: &[u16; 3]) -> u16 {
    if depth < 0.0 {
        AIR
    } else if depth < BLOCK {
        // the top block, wherever its middle lies under the surface: the mesh draws it so too (`generate`)
        ground[0]
    } else if depth < 4.0 {
        ground[1]
    } else {
        ground[2]
    }
}

/// One block of a body: pos = (column i, column j, layer).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, serde::Serialize, serde::Deserialize)]
pub struct Block {
    pub body: u8,
    pub face: u8,
    pub pos: IVec3,
}

impl Block {
    fn chunk(&self) -> Key {
        Key { body: self.body, face: self.face, level: 0, pos: self.pos.div_euclid(IVec3::splat(CS)) }
    }
}

/// The moon's grid lies here in the shared coordinates, far from the planet's, so a position names its frame by
/// where it is (ADR 0022). The moon really orbits elsewhere: `sky::Sky::moon_orbit`. 10,000 km: past the
/// planet's frame on the largest planet (1.5 x 0.6 of a 60 radii orbit, 920 km at 24576 columns; ADR 0041,
/// it was 1,000 km before Luna moved out to 60 radii).
pub const MOON_AT: DVec3 = DVec3::new(1.0e7, 0.0, 0.0);
/// A gas body's grid lies its index times this out instead (step 6a): its sphere of influence from its mass
/// (`World::soi`, 7.5 Mm for the giant) would reach the grids MOON_AT apart.
pub const GAS_AT: DVec3 = DVec3::new(1.0e8, 0.0, 0.0);
/// Where MOON_AT was before ADR 0041: a save's players there are moved (server `from_save`).
pub const OLD_MOON_AT: DVec3 = DVec3::new(1.0e6, 0.0, 0.0);
/// The inertial frame's origin (the sun's centre) in the shared coordinates: far enough that the sun's
/// surroundings (/goto sun, some million km out at 23,500 planet radii) never fall in a body's frame.
pub const INERTIAL_AT: DVec3 = DVec3::new(0.0, 0.0, -1.0e10);
/// The inertial frame's index; the bodies' frames are their indices.
pub const INERTIAL: usize = usize::MAX;
/// Where the air ends, metres over a body with air (needs.md "Breath": 1,500 m, where the sky thins to none):
/// above it the ape cannot breathe (`World::over_air`).
pub const AIR_TOP_M: f64 = 1500.0;

pub struct World {
    pub bodies: Arc<Vec<Arc<Body>>>,
    /// The sky's mechanics, and the world's time in seconds that poses them (ADR 0022).
    pub sky: Sky,
    /// The home's first moon (`bodies::moon`): the body the sky's eclipses, tides and moonlight are of.
    pub moon: usize,
    pub time: f64,
    /// Edits bucketed by level 0 chunk; value = material that replaces the generated one.
    pub edits: FxMap<Key, FxMap<IVec3, u16>>,
    pub edit_count: usize,
    /// The edits that give light, and a number that changes with them: the client's lamps come from here, so
    /// nothing scans the edits (ADR 0025, H).
    pub lights: FxMap<Block, [f32; 3]>,
    pub lights_rev: u64,
    /// The cells whose block names a model (ADR 0047), with the block: drawn as models, not cubes.
    pub models: FxMap<Block, u16>,
    /// Water (ADR 0020): on the server it flows; a client keeps the server's copy.
    pub water: Liquids,
    /// Snow, and ice and rock short of a block, in eighths (ADR 0189): the server's live columns; what every
    /// column shows, a client's copy from the wire.
    pub snow: crate::snow::Snow,
    /// The ore left in each of the planet's bodies (ADR 0063), blocks, as edits change it: the server's, opened
    /// with `open_ledger` before any edit; None where nothing counts it (a client).
    pub ore_left: Option<Vec<i64>>,
    /// Per chunk of every level, the block each edited cell shows (ADR 0109), kept up to date per edit: what a
    /// mesh job reads, so the frame thread only hands it the shared maps. The client's, opened with `open_far`;
    /// None where nothing meshes far chunks (the server), and then level 0 reads the edits.
    pub far: Option<FxMap<Key, FarChunk>>,
    /// Per body, Some for one whose ground comes in on approach (step 6b, `World::of_sources`): its map's hash
    /// and whether it is in. One not in is a stub, the body before its map (no heights, caves or ores).
    pub lazy: Vec<Option<Lazy>>,
}

/// A body loaded on approach (step 6b): the hash of the map it stands on, the world's seed, and whether its
/// ground is in (`World::loader`, `put`, `unload`).
#[derive(Clone)]
pub struct Lazy {
    pub hash: u64,
    pub seed: i32,
    pub loaded: bool,
    /// The body before its map, what it is while not loaded and what its loader builds on.
    stub: Arc<Body>,
}

/// What a body of `World::of_sources` stands on: a map now (None: its seed's noise), or the map of this hash
/// later, when something nears it (step 6b).
pub enum Source {
    Now(Option<Arc<Map>>),
    Later(u64),
}

/// Whether a placed block counts towards a far cell (ADR 0109): opaque and not a model.
fn far_counts(reg: &crate::blocks::Registry, m: u16) -> bool {
    reg.opaque(m) && reg.get(m).is_none_or(|d| d.model.is_none())
}
/// Whether an edit at a far cell's middle shows there, as before ADR 0109: a dig or any block but a model.
fn far_mid(reg: &crate::blocks::Registry, m: u16) -> bool {
    m == AIR || reg.get(m).is_none_or(|d| d.model.is_none())
}

/// The block each edited cell of one chunk shows, shared with the mesh jobs that read it (copied on write).
pub type Picks = Arc<FxMap<IVec3, u16>>;

/// What the edits in one coarse cell come to (ADR 0109): the edit at its middle, and how many of each counting
/// block it holds.
#[derive(Default)]
struct CellSum {
    mid: Option<u16>,
    counts: Vec<(u16, u32)>,
}

impl CellSum {
    /// The middle's edit; else, once the cell holds s counting blocks (s its edge), the most common, a tie to
    /// the lower id; else none (the generated cell).
    fn pick(&self, s: i32) -> Option<u16> {
        if self.mid.is_some() {
            return self.mid;
        }
        if self.counts.iter().map(|c| c.1).sum::<u32>() < s as u32 {
            return None;
        }
        self.counts.iter().copied().reduce(|a, b| if b.1 > a.1 || (b.1 == a.1 && b.0 < a.0) { b } else { a }).map(|c| c.0)
    }
    fn count(&mut self, m: u16, d: i32) {
        match self.counts.iter().position(|c| c.0 == m) {
            Some(i) => {
                self.counts[i].1 = (self.counts[i].1 as i32 + d) as u32;
                if self.counts[i].1 == 0 {
                    self.counts.swap_remove(i);
                }
            }
            None => {
                debug_assert!(d > 0, "block {m} taken from a cell that counts none");
                self.counts.push((m, d as u32))
            }
        }
    }
}

/// One chunk's far cells: their sums (levels over 0) and the block each shows; at level 0, per edited block,
/// the levels its last edit changed (`World::far_levels`).
#[derive(Default)]
pub struct FarChunk {
    cells: FxMap<IVec3, CellSum>,
    pub picks: Picks,
    changed: FxMap<IVec3, u32>,
}

/// What a mesh job needs of the edits (ADR 0109): the picks of the chunks its padded cells lie in, taken on the
/// frame thread in a few lookups, read on the worker.
pub struct FarSnap {
    key: Key,
    nl: i32,
    maps: Vec<(Key, Picks)>,
}

/// A chunk of the LOD octree in grid space: level L has cells of 2^L columns and layers, 32 cells per edge.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Key {
    pub body: u8,
    pub face: u8,
    pub level: u8,
    pub pos: IVec3,
}

impl Key {
    /// Columns and layers along one edge.
    pub fn span(&self) -> i32 {
        CS << self.level
    }
    /// Minimum corner in columns and layers.
    pub fn base(&self) -> IVec3 {
        self.pos * self.span()
    }
    /// Nominal edge in metres.
    pub fn edge(&self) -> f64 {
        BLOCK * self.span() as f64
    }
    pub fn child(&self, i: i32) -> Key {
        Key { level: self.level - 1, pos: self.pos * 2 + IVec3::new(i & 1, (i >> 1) & 1, (i >> 2) & 1), ..*self }
    }
}

pub enum Gen {
    Empty,
    Solid,
    Cells(Box<[u16; P * P * P]>),
}

pub fn pidx(x: usize, y: usize, z: usize) -> usize {
    x + y * P + z * P * P
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod water_tests;

#[cfg(test)]
mod frame_tests {
    use super::*;

    /// Positions go to the inertial frame and back exactly; `place` agrees; leaving a body keeps its motion.
    #[test]
    fn frames_round_trip_and_carry_their_motion() {
        let mut w = World::new(12288, 512, 1337);
        w.time = 777.0;
        let (p0, m0) = (w.bodies[0].centre, w.bodies[1].centre);
        let near_moon = m0 + DVec3::new(0.0, 250.0, 40.0);
        for p in [DVec3::new(10.0, 4300.0, -20.0), near_moon] {
            let f = w.frame_at(p);
            assert!((w.from_inertial(f, w.to_inertial(p)) - p).length() < 1e-6);
        }
        assert_eq!((w.frame_at(p0 + DVec3::Y * 4300.0), w.frame_at(near_moon)), (0, 1));
        // the moon's grid placed in the planet's frame is where the inertial frame says
        let (rot, off) = w.place(1, 0);
        assert!(((rot * (near_moon - m0) + off) - w.from_inertial(0, w.to_inertial(near_moon))).length() < 1e-6);

        // at rest on the planet past its sphere of influence: into the inertial frame, moving with the spin
        let up = DVec3::Y * (w.soi(0) * 1.01);
        let (mut pos, mut vel) = (up, DVec3::ZERO);
        let x = w.to_inertial(pos);
        assert!(w.reframe(&mut pos, &mut vel).is_some());
        assert_eq!(w.frame_at(pos), INERTIAL);
        assert!((w.to_inertial(pos) - x).length() < 1e-6, "the place does not move");
        let spin = std::f64::consts::TAU * (1.0 / crate::sky::DAY + 1.0 / crate::sky::YEAR) * up.length();
        // the planet's own motion around the sun, taken out to see the spin
        let pv = w.sky.planet(w.time + 0.5).centre - w.sky.planet(w.time - 0.5).centre;
        assert!((pv.length() - std::f64::consts::TAU * w.sky.sun_dist / crate::sky::YEAR).abs() < 0.01);
        assert!(((vel - pv).length() - spin).abs() < 0.01 * spin, "{} against {spin}", (vel - pv).length());
        // and back into the planet's frame at rest
        let back = DVec3::Y * (w.soi(0) * 0.9);
        let (mut q, mut v2) = (w.from_inertial(INERTIAL, w.to_inertial(back)), vel);
        assert!(w.reframe(&mut q, &mut v2).is_some());
        assert_eq!(w.frame_at(q), 0);
        // lower down the spin is slower: what is left is that difference
        let left = spin * (1.0 - 0.9 / 1.01);
        assert!((v2.length() - left).abs() < 0.02 * spin, "{} against {left}", v2.length());

        // at rest on the moon past its sphere of influence: the moon's orbital speed carries on
        let (mut pos, mut vel) = (m0 + DVec3::Z * (w.soi(1) * 1.01), DVec3::ZERO);
        assert!(w.reframe(&mut pos, &mut vel).is_some());
        let orbit = std::f64::consts::TAU * w.sky.moon_orbit * (1.0 / crate::sky::MONTH + 1.0 / crate::sky::YEAR);
        assert!((vel - pv).length() > 0.8 * orbit, "{} against the orbit's {orbit}", (vel - pv).length());
    }
}

#[cfg(test)]
mod lattice_tests {
    use super::*;
    // 8 cells near the ground, never growing with the level, a power of two that divides a chunk
    #[test]
    fn the_lattice_shrinks_with_the_level() {
        for n in [1536, 12288, 24576] {
            let b = Body::new(DVec3::ZERO, n, 1);
            assert_eq!(b.lattice(0), 8, "planet {n}");
            let l: Vec<u32> = (0..=12).map(|lv| b.lattice(lv)).collect();
            assert!(l.windows(2).all(|w| w[1] <= w[0]) && l.iter().all(|x| x.is_power_of_two() && CS as u32 % x == 0), "{l:?}");
            assert!(l[12] < 8, "planet {n}: {l:?}");
        }
    }
}

#[cfg(test)]
mod baked_tests;

#[cfg(test)]
mod record_tests;

#[cfg(test)]
mod gravity_tests;

#[cfg(test)]
mod ground_tests;
