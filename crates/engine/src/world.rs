//! Terrain: bodies on the equi-angular cube-sphere grid (ADR 0003), generated from the seed, plus a sparse
//! edit map. A block is (face, column i, column j, layer); layers are geometric shells, so blocks stay cubic.
use crate::blocks::{AIR, BLOCKS};
use crate::liquid::{Col, Ground, Liquids, OPEN, U};
use crate::sky::{Pose, Sky};
use fastnoise_lite::{FastNoiseLite, FractalType, NoiseType};
use glam::{DQuat, DVec3, IVec3};
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};
use std::sync::Arc;

pub const BLOCK: f64 = 0.5;
/// The planet's sea lies this far under its nominal radius (ADR 0020); the continents decide how much of it
/// is sea (ADR 0021).
pub const SEA_DEPTH: f64 = 4.0;
/// The share of the planet's surface the continents leave to the sea (ADR 0021).
pub const OCEAN: f64 = 0.55;
pub const CS: i32 = 32; // cells per chunk edge
pub const P: usize = 34; // padded edge
/// Gravity at a body's surface, m/s^2 (ADR 0007, 0024).
pub const G: f64 = 9.81;
/// No generated surface lies farther than this from a body's radius (m).
pub const AMP: f64 = 80.0;
/// The relief of a body without continents (the moon): +-30 m of hills.
const HILLS: f64 = 30.0;
pub const CRUST: f64 = 64.0;

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

pub struct Body {
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
    /// Layers 0..layers are streamed (terrain plus build headroom).
    pub layers: i32,
    noise: FastNoiseLite,
    /// The sea level as a layer coordinate; generation fills every column lower than it with water.
    pub sea: Option<f64>,
    /// Continents and oceans (ADR 0021): the planet has them, the moon is all hills.
    land: Option<Box<Land>>,
    /// Where players start: the land nearest the +y pole (a direction from the centre).
    pub spawn: DVec3,
    /// What generation lays down: the top, the band under it, the rock below (block ids).
    pub ground: [u8; 3],
    /// The column heights of chunk footprints, shared by every consumer (ADR 0025, E).
    tiles: HeightCache,
}

/// The surface radius of every column of a chunk's footprint with its padding (`generate`'s P x P), a pure
/// function of (face, level, x, y): the chunks stacked over it, its remeshes and its sea share one.
pub struct HeightTile {
    pub h: [f64; P * P],
    /// Of the heights as `f32`, as `generate` compares them.
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
        (h * self.relief).clamp(SEA_DEPTH - AMP + 2.0, AMP - SEA_DEPTH - 2.0)
    }
}

impl Body {
    pub fn new(centre: DVec3, n: i32, seed: i32) -> Self {
        assert!(n > 0 && n % CS == 0, "columns per face edge must be a multiple of {CS}");
        let mut noise = FastNoiseLite::with_seed(seed);
        noise.set_noise_type(Some(NoiseType::OpenSimplex2));
        noise.set_fractal_type(Some(FractalType::FBm));
        noise.set_fractal_octaves(Some(4));
        noise.set_fractal_lacunarity(Some(2.0));
        noise.set_fractal_gain(Some(0.5));
        noise.set_frequency(Some(1.0 / 120.0));
        // 6 n^2 columns cover 4 pi R^2
        let radius = n as f64 * BLOCK / (2.0 * PI / 3.0).sqrt();
        let r0 = radius - CRUST.min(radius * 0.5); // small bodies keep half their radius
        let lnq = (1.0 + BLOCK / radius).ln();
        let layers = (((radius + AMP + BUILD_HEADROOM) / r0).ln() / lnq).ceil() as i32;
        Body { centre, radius, n, root_level: (n / CS).trailing_zeros() as u8, r0, lnq, layers, noise, sea: None, land: None, spawn: DVec3::Y, ground: *GEN, tiles: HeightCache::default() }
    }
    /// The body with continents, oceans and a sea (ADR 0020, 0021), and its spawn on land.
    pub fn with_continents(mut self, seed: i32) -> Self {
        let land = Land::new(seed, self.radius);
        // an eighth of a block under a layer's top: a block at the waterline stands clearly out of the water
        // instead of flickering through it
        self.sea = Some(self.layer_of(land.sea_r).floor() + 0.875);
        self.land = Some(Box::new(land));
        // the land nearest the +y pole, a little above the sea
        let sea_r = self.radius - SEA_DEPTH;
        self.spawn = lattice(4096).filter(|d| d.y > 0.0 && self.height(*d) > sea_r + 2.0).max_by(|a, b| a.y.total_cmp(&b.y)).unwrap_or(DVec3::Y);
        self
    }
    /// Where a player starts: over the spawn, clear of the highest ground within a metre, so a steep slope
    /// never holds it inside a block; it drops onto the ground from there.
    pub fn spawn_point(&self) -> DVec3 {
        self.stand_point(self.spawn)
    }
    /// Feet on the surface over direction d (from the centre): over the highest ground a step around.
    pub fn stand_point(&self, d: DVec3) -> DVec3 {
        let d = d.normalize();
        let a = d.any_orthonormal_vector();
        let b = d.cross(a);
        let mut top: f64 = 0.0;
        for (x, y) in [(0.0, 0.0), (1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0), (0.7, 0.7), (-0.7, 0.7), (0.7, -0.7), (-0.7, -0.7)] {
            top = top.max(self.height((d * self.radius + (a * x + b * y)).normalize()));
        }
        self.centre + d * (top + 0.6)
    }
    /// The radius of the surface over direction d: the ground, or the sea over it.
    pub fn top(&self, d: DVec3) -> f64 {
        let h = self.height(d);
        self.sea.map_or(h, |s| h.max(self.layer_r(s)))
    }
    /// Root chunks per face edge.
    pub fn roots(&self) -> i32 {
        (self.n / CS) >> self.root_level
    }
    /// Radius of layer coordinate l (l = 3.5 is the middle of layer 3).
    pub fn layer_r(&self, l: f64) -> f64 {
        self.r0 * (l * self.lnq).exp()
    }
    /// The lattice, in cells, that a chunk of `level` merges its quads inside and the renderer places its vertices
    /// on (ADR 0025 I2): 8, halved while a lattice cell's sag off the sphere, (n cell)^2 / 8R, would pass half a
    /// pixel (1.1 mrad at 1080p) at the nearest distance that level is drawn from at the lowest detail, 2 chunk
    /// edges. A function of the body and level alone, so a mesh and its draw always agree.
    pub fn lattice(&self, level: u8) -> u32 {
        let cell = self.radius * std::f64::consts::FRAC_PI_2 / self.n as f64 * (1u64 << level) as f64;
        let near = 2.0 * CS as f64 * cell;
        let mut n = 8;
        while n > 1 && (n as f64 * cell).powi(2) / (8.0 * self.radius) > 0.5 * 1.1e-3 * near {
            n /= 2;
        }
        n
    }

    pub fn layer_of(&self, r: f64) -> f64 {
        (r / self.r0).ln() / self.lnq
    }
    /// Unit direction of grid point (i, j) on a face, in columns (i = 3.5 is the middle of column 3).
    pub fn dir(&self, face: usize, i: f64, j: f64) -> DVec3 {
        self.dir_tan(face, self.col_tan(i), self.col_tan(j))
    }
    /// `dir` from its two tangents: a tile computes them once per row and column, to the same bits.
    fn dir_tan(&self, face: usize, ta: f64, tb: f64) -> DVec3 {
        let [n, s, t] = AX[face];
        (n.as_dvec3() + s.as_dvec3() * ta + t.as_dvec3() * tb).normalize()
    }
    fn col_tan(&self, i: f64) -> f64 {
        let th = FRAC_PI_2 / self.n as f64;
        let h = self.n as f64 * 0.5;
        ((i - h) * th).tan()
    }
    /// The height tile under chunk footprint (face, level, x, y), built here on a miss. Only workers and the
    /// server call it: on the frame thread use `col_height`, which never builds one.
    pub fn tile(&self, face: u8, level: u8, x: i32, y: i32) -> Arc<HeightTile> {
        use std::sync::atomic::Ordering::Relaxed;
        let k = (face, level, x, y);
        let now = self.tiles.tick.fetch_add(1, Relaxed);
        if let Some(e) = self.tiles.map.lock().unwrap().get_mut(&k) {
            e.1 = now;
            return e.0.clone();
        }
        // built outside the lock; two workers may build the same tile, the result is the same
        let tile = Arc::new(self.build_tile(face as usize, level, x, y));
        let mut m = self.tiles.map.lock().unwrap();
        if m.len() >= HEIGHT_TILES {
            let mut ages: Vec<u64> = m.values().map(|e| e.1).collect();
            let mid = ages.len() / 2;
            let cut = *ages.select_nth_unstable(mid).1;
            m.retain(|_, e| e.1 > cut);
        }
        m.insert(k, (tile.clone(), now));
        tile
    }
    fn build_tile(&self, face: usize, level: u8, x: i32, y: i32) -> HeightTile {
        let s = (1i32 << level) as f64;
        let nl = self.n >> level;
        let (c0x, c0y) = (x * CS - 1, y * CS - 1);
        let ta: [f64; P] = std::array::from_fn(|k| self.col_tan(((c0x + k as i32) as f64 + 0.5) * s));
        let tb: [f64; P] = std::array::from_fn(|k| self.col_tan(((c0y + k as i32) as f64 + 0.5) * s));
        let mut t = HeightTile { h: [0.0; P * P], min: f32::MAX, max: f32::MIN };
        for v in 0..P {
            for u in 0..P {
                let (ci, cj) = (c0x + u as i32, c0y + v as i32);
                // columns past a seam belong to the neighbouring face
                let (f, fi, fj) = fold(face, ci, cj, nl);
                let d = if (f, fi, fj) == (face, ci, cj) { self.dir_tan(face, ta[u], tb[v]) } else { self.dir(f, (fi as f64 + 0.5) * s, (fj as f64 + 0.5) * s) };
                let hv = self.height(d);
                t.min = t.min.min(hv as f32);
                t.max = t.max.max(hv as f32);
                t.h[u + v * P] = hv;
            }
        }
        t
    }
    /// The surface radius over level-0 column (i, j) of a face: from its tile when one is cached, else computed,
    /// never building a tile (the frame thread's physics calls it).
    pub fn col_height(&self, face: u8, i: i32, j: i32) -> f64 {
        let (x, y) = (i.div_euclid(CS), j.div_euclid(CS));
        if let Some(e) = self.tiles.map.lock().unwrap().get(&(face, 0, x, y)) {
            return e.0.h[(i - x * CS + 1) as usize + (j - y * CS + 1) as usize * P];
        }
        self.height(self.dir(face as usize, i as f64 + 0.5, j as f64 + 0.5))
    }
    /// Surface radius along unit direction d.
    pub fn height(&self, d: DVec3) -> f64 {
        let p = d * self.radius;
        if let Some(l) = &self.land {
            return l.sea_r + l.over_sea(p);
        }
        self.radius + HILLS * self.noise.get_noise_3d(p.x as f32, p.y as f32, p.z as f32) as f64
    }
    /// Bounding sphere of a chunk, clamped to the streamed layers.
    pub fn bounds(&self, k: Key) -> Bounds {
        let (b, s) = (k.base(), k.span());
        let (i0, i1, j0, j1) = (b.x as f64, (b.x + s) as f64, b.y as f64, (b.y + s) as f64);
        let r_in = self.layer_r(b.z.max(0) as f64);
        let r_out = self.layer_r((b.z + s).min(self.layers) as f64);
        let f = k.face as usize;
        let c = self.dir(f, (i0 + i1) * 0.5, (j0 + j1) * 0.5) * ((r_in + r_out) * 0.5);
        // the farthest points of the wedge from its middle are its corners
        let mut rad2 = 0f64;
        for d in [self.dir(f, i0, j0), self.dir(f, i1, j0), self.dir(f, i0, j1), self.dir(f, i1, j1)] {
            rad2 = rad2.max((d * r_in - c).length_squared()).max((d * r_out - c).length_squared());
        }
        Bounds { c: self.centre + c, rad: rad2.sqrt() }
    }
}

/// The solid world as the water sees it (`liquid::Ground`): terrain from the seed plus the edits.
pub struct Solid<'a> {
    pub bodies: &'a [Body],
    pub edits: &'a FxMap<Key, FxMap<IVec3, u8>>,
}

impl Solid<'_> {
    /// The first generated air layer of column c.
    fn gen_floor(&self, c: Col) -> i32 {
        let b = &self.bodies[c.body() as usize];
        let h = b.col_height(c.face(), c.i(), c.j());
        let mut z = ((b.layer_of(h) - 0.5).floor() as i32 + 1).max(0);
        // the same test as `generated_under`, so the two never disagree by a layer
        while z > 0 && material(h - b.layer_r(z as f64 - 0.5), &b.ground) == AIR {
            z -= 1;
        }
        while material(h - b.layer_r(z as f64 + 0.5), &b.ground) != AIR {
            z += 1;
        }
        z
    }
}

impl Ground for Solid<'_> {
    fn gaps(&self, c: Col) -> Vec<(i32, i32)> {
        let zs = self.gen_floor(c);
        let b = &self.bodies[c.body() as usize];
        let (i, j) = (c.i(), c.j());
        let mut edits: Vec<(i32, bool)> = Vec::new();
        for zc in 0..=(b.layers + CS) / CS {
            let k = Key { body: c.body(), face: c.face(), level: 0, pos: IVec3::new(i.div_euclid(CS), j.div_euclid(CS), zc) };
            if let Some(cells) = self.edits.get(&k) {
                for z in zc * CS..(zc + 1) * CS {
                    if let Some(m) = cells.get(&IVec3::new(i, j, z)) {
                        edits.push((z, BLOCKS.solid(*m)));
                    }
                }
            }
        }
        let lo = edits.first().map_or(zs, |e| e.0.min(zs)).max(0);
        let hi = edits.last().map_or(zs, |e| (e.0 + 1).max(zs));
        let solid = |z: i32| edits.iter().find(|e| e.0 == z).map_or(z < zs, |e| e.1);
        let mut out = Vec::new();
        let mut start = None;
        for z in lo..hi {
            match (solid(z), start) {
                (false, None) => start = Some(z),
                (true, Some(s)) => {
                    out.push((s, z));
                    start = None;
                }
                _ => {}
            }
        }
        out.push((start.unwrap_or(hi), OPEN));
        out
    }
    fn generated(&self, c: Col) -> (i64, i64) {
        let Some(sea) = self.bodies[c.body() as usize].sea else { return (0, 0) };
        (self.gen_floor(c) as i64 * U, (sea * U as f64) as i64)
    }
    fn neighbours(&self, c: Col) -> [Option<Col>; 4] {
        let n = self.bodies[c.body() as usize].n;
        let (i, j) = (c.i(), c.j());
        [(i - 1, j), (i + 1, j), (i, j - 1), (i, j + 1)].map(|(a, b)| {
            let (f, a, b) = fold(c.face() as usize, a, b, n);
            Some(Col::new(c.body(), f as u8, a, b))
        })
    }
}

#[derive(Clone, Copy)]
pub struct Bounds {
    pub c: DVec3,
    pub rad: f64,
}

/// The blocks generation names, looked up once in the registry: grass, dirt, stone.
static GEN: std::sync::LazyLock<[u8; 3]> =
    std::sync::LazyLock::new(|| [BLOCKS.id("grass"), BLOCKS.id("dirt"), BLOCKS.id("stone")]);

/// The generated block `depth` metres under the surface, from a body's `ground`.
pub fn material(depth: f64, ground: &[u8; 3]) -> u8 {
    if depth < 0.0 {
        AIR
    } else if depth < 0.5 {
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
/// where it is (ADR 0022). The moon really orbits much nearer: `sky::Sky::moon_orbit`.
pub const MOON_AT: DVec3 = DVec3::new(1.0e6, 0.0, 0.0);
/// The inertial frame's origin (the sun's centre) in the shared coordinates.
pub const INERTIAL_AT: DVec3 = DVec3::new(0.0, 0.0, -1.0e6);
/// The inertial frame's index; the bodies' frames are their indices.
pub const INERTIAL: usize = usize::MAX;

pub struct World {
    pub bodies: Arc<Vec<Body>>,
    /// The sky's mechanics, and the world's time in seconds that poses them (ADR 0022).
    pub sky: Sky,
    pub time: f64,
    /// Edits bucketed by level 0 chunk; value = material that replaces the generated one.
    pub edits: FxMap<Key, FxMap<IVec3, u8>>,
    pub edit_count: usize,
    /// The edits that give light, and a number that changes with them: the client's lamps come from here, so
    /// nothing scans the edits (ADR 0025, H).
    pub lights: FxMap<Block, [f32; 3]>,
    pub lights_rev: u64,
    /// Water (ADR 0020): on the server it flows; a client keeps the server's copy.
    pub water: Liquids,
}

impl World {
    /// A planet and its moon 3 km away, with `planet_n` and `moon_n` columns per face edge.
    pub fn new(planet_n: i32, moon_n: i32, seed: i32) -> Self {
        let planet = Body::new(DVec3::ZERO, planet_n, seed).with_continents(seed);
        let mut moon = Body::new(MOON_AT, moon_n, seed + 1);
        moon.ground = [BLOCKS.id("moon dust"), BLOCKS.id("moon dust"), GEN[2]];
        // the moon orbits 2.83 planet radii out, as from the 1 km planet; never nearer than 3 km (small test planets)
        let sky = Sky { planet_radius: planet.radius, moon_radius: moon.radius, moon_orbit: (planet.radius * 2.83).max(3000.0) };
        World {
            bodies: Arc::new(vec![planet, moon]),
            sky,
            time: 0.0,
            edits: FxMap::default(),
            edit_count: 0,
            lights: FxMap::default(),
            lights_rev: 0,
            water: Liquids::default(),
        }
    }
    /// The frame a position (shared coordinates) is in: a body's, near its grid, or the inertial one.
    pub fn frame_at(&self, p: DVec3) -> usize {
        // past the sphere of influence, so a position keeps its frame until `reframe` moves it
        (0..self.bodies.len()).find(|&i| (p - self.bodies[i].centre).length() < 1.5 * self.soi(i)).unwrap_or(INERTIAL)
    }
    /// A body's sphere of influence: within it, a position belongs to the body's frame.
    pub fn soi(&self, i: usize) -> f64 {
        self.sky.moon_orbit * if i == 0 { 0.6 } else { 0.25 }
    }
    /// Frame `f`'s pose at time t: its coordinates to the inertial frame.
    pub fn pose(&self, f: usize, t: f64) -> Pose {
        match f {
            0 => self.sky.planet(t),
            1 => self.sky.moon(t),
            _ => Pose { centre: DVec3::ZERO, rot: DQuat::IDENTITY },
        }
    }
    /// Where frame `f`'s pose is anchored in the shared coordinates: its body's grid centre.
    pub fn nominal(&self, f: usize) -> DVec3 {
        if f == INERTIAL {
            INERTIAL_AT
        } else {
            self.bodies[f].centre
        }
    }
    /// A position (shared coordinates) in the inertial frame, now.
    pub fn to_inertial(&self, p: DVec3) -> DVec3 {
        let f = self.frame_at(p);
        self.pose(f, self.time).to_inertial(self.nominal(f), p)
    }
    /// An inertial position in frame `f`'s shared coordinates, now.
    pub fn from_inertial(&self, f: usize, x: DVec3) -> DVec3 {
        self.pose(f, self.time).from_inertial(self.nominal(f), x)
    }
    /// How frame `from`'s coordinates lie in frame `to`'s, now: x_to = rot * (x - nominal(from)) + off.
    pub fn place(&self, from: usize, to: usize) -> (DQuat, DVec3) {
        if from == to {
            return (DQuat::IDENTITY, self.nominal(from));
        }
        let (a, b) = (self.pose(from, self.time), self.pose(to, self.time));
        (b.rot.inverse() * a.rot, b.from_inertial(self.nominal(to), a.centre))
    }
    /// The inertial velocity of a point at rest at p in frame f.
    fn frame_vel(&self, f: usize, p: DVec3) -> DVec3 {
        let (n, h) = (self.nominal(f), 0.05);
        (self.pose(f, self.time + h).to_inertial(n, p) - self.pose(f, self.time - h).to_inertial(n, p)) / (2.0 * h)
    }
    /// Moves a position and velocity into the frame their real place belongs to (ADR 0022): out of a body's
    /// sphere of influence into the inertial frame, or into one. The velocity keeps the old frame's motion,
    /// so a jump off the moon carries its orbit. Returns the rotation from the old frame to the new one.
    pub fn reframe(&self, pos: &mut DVec3, vel: &mut DVec3) -> Option<DQuat> {
        let f = self.frame_at(*pos);
        let x = self.to_inertial(*pos);
        let t = self.time;
        let inside = |i: usize, k: f64| (x - self.pose(i, t).centre).length() < self.soi(i) * k;
        let g = if f != INERTIAL && inside(f, 1.0) { f } else { (0..self.bodies.len()).find(|&i| inside(i, 0.97)).unwrap_or(INERTIAL) };
        if g == f {
            return None;
        }
        let (pf, pg) = (self.pose(f, t), self.pose(g, t));
        let v = pf.rot * *vel + self.frame_vel(f, *pos);
        let np = self.from_inertial(g, x);
        *vel = pg.rot.inverse() * (v - self.frame_vel(g, np));
        *pos = np;
        Some(pg.rot.inverse() * pf.rot)
    }
    /// An inertial position in the shared coordinates of the frame it lies in (a body's sphere of influence,
    /// or the inertial frame), and that frame.
    pub fn locate(&self, x: DVec3) -> (DVec3, usize) {
        let t = self.time;
        let f = (0..self.bodies.len()).find(|&i| (x - self.pose(i, t).centre).length() < self.soi(i)).unwrap_or(INERTIAL);
        (self.from_inertial(f, x), f)
    }
    /// The height over the nearest body's sea-level sphere, where the bodies really are now.
    pub fn altitude(&self, p: DVec3) -> f64 {
        let x = self.to_inertial(p);
        (0..self.bodies.len())
            .map(|i| (x - self.pose(i, self.time).centre).length() - self.bodies[i].radius)
            .fold(f64::MAX, f64::min)
    }
    /// Gravity at a position (ADR 0024), m/s^2 in its frame's coordinates: every body pulls towards its centre
    /// where it is now, with G over all its terrain (within AMP of its radius) and the square of the distance
    /// less past that, so down turns smoothly from one body to the other and fades out in deep space.
    pub fn gravity(&self, p: DVec3) -> DVec3 {
        let f = self.frame_at(p);
        (0..self.bodies.len())
            .map(|i| {
                let c = if i == f { self.nominal(f) } else { self.from_inertial(f, self.pose(i, self.time).centre) };
                let d = c - p;
                let r = d.length().max(1e-6);
                d / r * G * ((self.bodies[i].radius + AMP) / r).min(1.0).powi(2)
            })
            .sum()
    }
    /// Up at a position: away from the body whose frame it is in; in deep space, away from the nearest body.
    pub fn up_at(&self, p: DVec3) -> DVec3 {
        let f = self.frame_at(p);
        if f == INERTIAL {
            // away from the nearest body where it really is; the inertial frame is not turned
            let x = p - INERTIAL_AT;
            let c = (0..self.bodies.len())
                .map(|i| self.pose(i, self.time).centre)
                .min_by(|a, b| a.distance(x).total_cmp(&b.distance(x)))
                .unwrap_or(DVec3::ZERO);
            return (x - c).normalize_or(DVec3::Y);
        }
        (p - self.nominal(f)).normalize_or(DVec3::Y)
    }
    pub fn nearest(&self, p: DVec3) -> usize {
        let mut best = 0;
        let mut bd = f64::MAX;
        for (i, b) in self.bodies.iter().enumerate() {
            let d = (p - b.centre).length() - b.radius;
            if d < bd {
                bd = d;
                best = i;
            }
        }
        best
    }
    /// The block of the nearest body that holds world point p.
    pub fn block_at(&self, p: DVec3) -> Block {
        let (bi, face, g) = self.grid_at(p);
        let n = self.bodies[bi].n;
        let col = |c: f64| (c.floor() as i32).clamp(0, n - 1);
        Block { body: bi as u8, face: face as u8, pos: IVec3::new(col(g.x), col(g.y), g.z.floor() as i32) }
    }
    /// Where world point p lies in the grid of the nearest body: (body, face, (column i, column j, layer)) with
    /// fractions, so column 3.5 is the middle of column 3. The columns are not clamped to the face.
    pub fn grid_at(&self, p: DVec3) -> (usize, usize, DVec3) {
        let bi = self.nearest(p);
        let b = &self.bodies[bi];
        let v = p - b.centre;
        let a = v.abs();
        let face = if a.x >= a.y && a.x >= a.z {
            (v.x < 0.0) as usize
        } else if a.y >= a.z {
            2 + (v.y < 0.0) as usize
        } else {
            4 + (v.z < 0.0) as usize
        };
        let [n, s, t] = AX[face].map(|x| x.as_dvec3());
        let d = v.dot(n);
        let col = |w: f64| ((w / d).atan() / FRAC_PI_4 + 1.0) * 0.5 * b.n as f64;
        (bi, face, DVec3::new(col(v.dot(s)), col(v.dot(t)), b.layer_of(v.length())))
    }
    /// World position of the middle of a block.
    pub fn block_centre(&self, b: Block) -> DVec3 {
        let body = &self.bodies[b.body as usize];
        let p = b.pos.as_dvec3() + 0.5;
        body.centre + body.dir(b.face as usize, p.x, p.y) * body.layer_r(p.z)
    }
    pub fn generated_block(&self, b: Block) -> u8 {
        self.generated_under(b, self.column_height(b))
    }
    /// The terrain's surface radius over block b's column: the costly part of `generated_block`, the same for
    /// every layer of a column.
    pub fn column_height(&self, b: Block) -> f64 {
        self.bodies[b.body as usize].col_height(b.face, b.pos.x, b.pos.y)
    }
    /// The generated material of block b under a surface at radius `height` (its `column_height`).
    pub fn generated_under(&self, b: Block, height: f64) -> u8 {
        let body = &self.bodies[b.body as usize];
        if b.pos.z < 0 {
            return body.ground[2];
        }
        material(height - body.layer_r(b.pos.z as f64 + 0.5), &body.ground)
    }
    /// The column that holds block b.
    pub fn col(b: Block) -> Col {
        Col::new(b.body, b.face, b.pos.x, b.pos.y)
    }
    /// The solid world as the water sees it.
    pub fn ground(&self) -> Solid<'_> {
        Solid { bodies: &self.bodies, edits: &self.edits }
    }
    /// Steps the water once (1/60 s).
    pub fn step_water(&mut self) {
        let World { bodies, edits, water, .. } = self;
        water.step(&Solid { bodies, edits });
    }
    /// Block b changed: its column's water finds its new gaps.
    pub fn regap(&mut self, b: Block) {
        let World { bodies, edits, water, .. } = self;
        water.regap(&Solid { bodies, edits }, World::col(b));
    }
    /// The radius of the water surface over world point p, if p is under water.
    pub fn water_surface(&self, p: DVec3) -> Option<f64> {
        let (bi, _, g) = self.grid_at(p);
        let b = self.block_at(p);
        let l = self.water.surface_at(&self.ground(), World::col(b), g.z)?;
        Some(self.bodies[bi].layer_r(l))
    }
    pub fn edit(&self, b: Block) -> Option<u8> {
        self.edits.get(&b.chunk()).and_then(|m| m.get(&b.pos)).copied()
    }
    pub fn block(&self, b: Block) -> u8 {
        self.edit(b).unwrap_or_else(|| self.generated_block(b))
    }
    pub fn set_block(&mut self, b: Block, m: u8) {
        if self.edits.entry(b.chunk()).or_default().insert(b.pos, m).is_none() {
            self.edit_count += 1;
        }
        let light = BLOCKS.get(m).map_or([0.0; 3], |d| d.light);
        let changed = if light != [0.0; 3] { self.lights.insert(b, light) != Some(light) } else { self.lights.remove(&b).is_some() };
        self.lights_rev += changed as u64;
    }
    /// Puts back what `edit` returned: an edit, or (None) the generated block with no edit recorded.
    pub fn put_edit(&mut self, b: Block, e: Option<u8>) {
        match e {
            Some(m) => self.set_block(b, m),
            None => {
                let k = b.chunk();
                if let Some(cells) = self.edits.get_mut(&k) {
                    if cells.remove(&b.pos).is_some() {
                        self.edit_count -= 1;
                        self.lights_rev += self.lights.remove(&b).is_some() as u64;
                    }
                    if cells.is_empty() {
                        self.edits.remove(&k);
                    }
                }
            }
        }
    }
    /// One hash per level 0 chunk that holds edits, in chunk order: two worlds from the same seed agree where
    /// these agree, and the first difference names the chunk where they part.
    pub fn chunk_hashes(&self) -> Vec<(Key, u64)> {
        let mut v: Vec<(Key, u64)> = self
            .edits
            .iter()
            .map(|(k, cells)| {
                let mut c: Vec<(&IVec3, &u8)> = cells.iter().collect();
                c.sort_unstable_by_key(|(p, _)| (p.x, p.y, p.z));
                (*k, c.iter().fold(FNV0, |h, (p, m)| fnv(fnv_ivec(h, **p), &[**m])))
            })
            .collect();
        v.sort_unstable_by_key(|(k, _)| (k.body, k.face, k.level, k.pos.x, k.pos.y, k.pos.z));
        v
    }
    /// The whole world's state as one number.
    pub fn hash(&self) -> u64 {
        self.chunk_hashes().iter().fold(FNV0, |h, (k, c)| fnv(fnv_ivec(fnv(h, &[k.body, k.face, k.level]), k.pos), &c.to_le_bytes()))
    }
    /// The edits a chunk's padded cells sample, as (padded cell index, material): a job's snapshot.
    pub fn edits_for(&self, k: Key) -> Vec<(u32, u8)> {
        let mut out = Vec::new();
        if self.edits.is_empty() {
            return out;
        }
        // coarse cells point-sample the block in their middle
        let s = 1i32 << k.level;
        let half = s / 2;
        let c0 = k.pos * CS - 1; // padded cell (0, 0, 0) in cells of this level
        let lo = c0 * s;
        let hi = lo + IVec3::splat(P as i32 * s);
        let mut take = |m: &FxMap<IVec3, u8>| {
            for (b, v) in m {
                let q = *b - lo - half;
                if q.x.rem_euclid(s) | q.y.rem_euclid(s) | q.z.rem_euclid(s) != 0 {
                    continue;
                }
                let q = q.div_euclid(IVec3::splat(s));
                if q.cmpge(IVec3::ZERO).all() && q.cmplt(IVec3::splat(P as i32)).all() {
                    out.push((pidx(q.x as usize, q.y as usize, q.z as usize) as u32, *v));
                }
            }
        };
        let clo = lo.div_euclid(IVec3::splat(CS));
        let chi = (hi - 1).div_euclid(IVec3::splat(CS));
        let span = (chi - clo + 1).as_i64vec3();
        if (span.x * span.y * span.z) as usize > self.edits.len() {
            for (c, m) in &self.edits {
                if c.body == k.body && c.face == k.face && c.pos.cmpge(clo).all() && c.pos.cmple(chi).all() {
                    take(m);
                }
            }
        } else {
            for z in clo.z..=chi.z {
                for y in clo.y..=chi.y {
                    for x in clo.x..=chi.x {
                        if let Some(m) = self.edits.get(&Key { level: 0, pos: IVec3::new(x, y, z), ..k }) {
                            take(m);
                        }
                    }
                }
            }
        }
        // padded strips past a seam sample the neighbouring face
        let nl = self.bodies[k.body as usize].n >> k.level;
        let pm = P as i32 - 1;
        let mut strip = |x: i32, y: i32| {
            let (f, ci, cj) = fold(k.face as usize, c0.x + x, c0.y + y, nl);
            for z in 0..P as i32 {
                let b = Block { body: k.body, face: f as u8, pos: IVec3::new(ci, cj, c0.z + z) * s + half };
                if let Some(m) = self.edit(b) {
                    out.push((pidx(x as usize, y as usize, z as usize) as u32, m));
                }
            }
        };
        for (x, out_of_face) in [(0, c0.x < 0), (pm, c0.x + pm >= nl)] {
            if out_of_face {
                (1..pm).for_each(|y| strip(x, y));
            }
        }
        for (y, out_of_face) in [(0, c0.y < 0), (pm, c0.y + pm >= nl)] {
            if out_of_face {
                (1..pm).for_each(|x| strip(x, y));
            }
        }
        out
    }
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
    Cells(Box<[u8; P * P * P]>),
}

pub fn pidx(x: usize, y: usize, z: usize) -> usize {
    x + y * P + z * P * P
}

/// Generate the padded cell grid of a chunk: x, y, z = column i, column j, layer.
/// The cell (at chunk `key`'s level) that holds the sea's surface, if it is in this chunk.
pub fn sea_cell(body: &Body, key: Key) -> Option<i32> {
    let cz = (body.sea? / (1i32 << key.level) as f64).floor() as i32;
    (cz.div_euclid(CS) == key.pos.z).then_some(cz)
}

/// The generated sea's surface in chunk `key` (ADR 0020): quads in the mesher's format, top faces of the cell
/// that holds the sea level, merged into rectangles; the water shader lays them at the exact level. Columns in
/// a `wet` tile (sorted) are left out: the client's water mesh draws all of those. A quad's material is 1
/// where the (coarse) cell at the sea level is solid: far away a cell is up to 128 m tall, and the shader lays
/// that water on the cell's top instead of inside it.
pub fn sea_quads(body: &Body, key: Key, wet: &[Col], gen: &Gen) -> Vec<[u32; 2]> {
    let Some(cz) = sea_cell(body, key) else { return Vec::new() };
    let s = 1i32 << key.level;
    let r = body.layer_r(body.sea.unwrap());
    let base = key.base();
    let n = CS as usize;
    let zl = (cz - key.pos.z * CS) as usize;
    // 0 no sea, 1 sea over air, 2 sea in a solid cell
    let mut sea = vec![0u8; n * n];
    let heights = body.tile(key.face, key.level, key.pos.x, key.pos.y);
    for y in 0..n {
        for x in 0..n {
            let (ci, cj) = (base.x + x as i32 * s, base.y + y as i32 * s);
            // the same sample as `generate`: the middle of the (coarse) column
            if wet.binary_search(&Col::new(key.body, key.face, ci, cj).tile()).is_err() && heights.h[x + 1 + (y + 1) * P] < r {
                let solid = match gen {
                    Gen::Cells(c) => c[pidx(x + 1, y + 1, zl + 1)] != 0,
                    Gen::Solid => true,
                    Gen::Empty => false,
                };
                // a block that holds the sea level stands out of it; only a coarse cell stands for shallows
                if !(solid && key.level == 0) {
                    sea[y * n + x] = 1 + solid as u8;
                }
            }
        }
    }
    let z = (zl + 1) as u32;
    // a quad is a flat chord under the sphere: one wider than the terrain's quads under it would sag below
    // them (120 m for a whole far chunk). Merged quads stay within about 90 m, a sag of 0.25 m.
    let cap = (((2.0 * body.radius).sqrt() / BLOCK) as usize >> key.level).clamp(1, n);
    let mut out = Vec::new();
    for y in 0..n {
        let mut x = 0;
        while x < n {
            let m = sea[y * n + x];
            if m == 0 {
                x += 1;
                continue;
            }
            let mut w = 1;
            while x + w < n && w < cap && sea[y * n + x + w] == m {
                w += 1;
            }
            let mut h = 1;
            while y + h < n && h < cap && (x..x + w).all(|i| sea[(y + h) * n + i] == m) {
                h += 1;
            }
            for v in y..y + h {
                for u in x..x + w {
                    sea[v * n + u] = 0;
                }
            }
            out.push([x as u32 | (y as u32) << 6 | z << 12 | ((w - 1) as u32) << 18 | ((h - 1) as u32) << 23 | 4 << 28, (m - 1) as u32]);
            x += w;
        }
    }
    out
}

pub fn generate(body: &Body, key: Key, edits: &[(u32, u8)]) -> Gen {
    let s = (1i32 << key.level) as f64;
    let c0 = key.pos * CS - 1; // padded cell (0, 0, 0) in cells of this level
    let r_lo = body.layer_r(c0.z as f64 * s);
    // the top padding's skirt looks SKIRT cells higher, past the chunk: generated terrain is air there when
    // the column is lower than this
    let z_air = c0.z + (P - 1 + SKIRT) as i32;
    let r_air = body.layer_r((z_air as f64 + 0.5) * s) as f32;
    if edits.is_empty() {
        // whole chunk outside the terrain's height range: no noise needed
        if r_lo > body.radius + AMP {
            return Gen::Empty;
        }
        if (r_air as f64) < body.radius - AMP {
            return Gen::Solid;
        }
    }
    // one height per column, from the footprint's tile (columns past a seam are the neighbouring face's)
    let tile = body.tile(key.face, key.level, key.pos.x, key.pos.y);
    let h: [f32; P * P] = std::array::from_fn(|i| tile.h[i] as f32);
    let (hmin, hmax) = (tile.min, tile.max);
    if edits.is_empty() {
        if r_lo as f32 > hmax {
            return Gen::Empty;
        }
        if r_air < hmin {
            return Gen::Solid;
        }
    }
    let mut cells = Box::new([0u8; P * P * P]);
    // coarse cells keep the surface band one cell thick, so the far planet stays green
    let cf = (s * BLOCK) as f32;
    let grass = cf.max(0.5);
    let dirt = (2.0 * cf).max(4.0);
    let mut any_air = false;
    let mut any_solid = false;
    for z in 0..P {
        let l = (c0.z as f64 + z as f64 + 0.5) * s;
        let r = body.layer_r(l) as f32;
        let core = l < 0.0;
        for (i, hv) in h.iter().enumerate() {
            let d = hv - r;
            let mut m = if core || d >= dirt { body.ground[2] } else if d >= grass { body.ground[1] } else if d >= 0.0 { body.ground[0] } else { 0 };
            if z == P - 1 && z_air >= 0 && *hv < r_air {
                // the skirt rule for the top padding, which has no cells above it here (ADR 0018). shortcut:
                // from the generated height alone, edits over the chunk do not count; a dug hole there could
                // still open a pinhole against a coarser chunk above
                m = 0;
            }
            any_air |= m == 0;
            any_solid |= m != 0;
            cells[z * P * P + i] = m;
        }
    }
    for (i, m) in edits {
        cells[*i as usize] = *m;
        any_air |= *m == 0;
        any_solid |= *m != 0;
    }
    if !any_solid {
        return Gen::Empty;
    }
    if !any_air {
        return Gen::Solid;
    }
    // skirts: a neighbour at another detail level draws a different surface, so faces culled against the
    // padding would leave holes and slivers at the chunk border. Padding cells within SKIRT cells under
    // air become air, and the chunk draws its own border walls that deep (hidden inside a same-level
    // neighbour, a tidy step against a coarser or finer one). Bottom-up, so the lookahead sees originals.
    for z in 0..P - 1 {
        for y in 0..P {
            for x in 0..P {
                let pad = x == 0 || y == 0 || z == 0 || x == P - 1 || y == P - 1;
                if pad && cells[pidx(x, y, z)] != 0 && (z + 1..=(z + SKIRT).min(P - 1)).any(|a| cells[pidx(x, y, a)] == 0) {
                    cells[pidx(x, y, z)] = 0;
                }
            }
        }
    }
    Gen::Cells(cells)
}

#[cfg(test)]
mod tests {
    #[test]
    fn height_tiles_match_the_direct_heights_to_the_bit() {
        // ADR 0025, E: the cache must not move a height by one bit, padding past a seam included
        let b = Body::new(DVec3::ZERO, 1536, 7).with_continents(7);
        let last = b.n / CS - 1;
        for (face, level, x, y) in [(2u8, 0u8, 5, 9), (0, 0, 0, 3), (4, 2, last >> 2, 0), (1, 1, 2, last >> 1)] {
            let s = (1i32 << level) as f64;
            let t = b.tile(face, level, x, y);
            for v in 0..P {
                for u in 0..P {
                    let (f, ci, cj) = fold(face as usize, x * CS - 1 + u as i32, y * CS - 1 + v as i32, b.n >> level);
                    let want = b.height(b.dir(f, (ci as f64 + 0.5) * s, (cj as f64 + 0.5) * s));
                    assert_eq!(t.h[u + v * P].to_bits(), want.to_bits(), "face {face} level {level} at {u},{v}");
                }
            }
        }
        // a level-0 column from a cached tile or computed: the same bits
        let (i, j) = (5 * CS + 7, 9 * CS + 30);
        assert_eq!(b.col_height(2, i, j).to_bits(), b.height(b.dir(2, i as f64 + 0.5, j as f64 + 0.5)).to_bits());
        assert_eq!(b.col_height(3, i, j).to_bits(), b.height(b.dir(3, i as f64 + 0.5, j as f64 + 0.5)).to_bits());
    }

    #[test]
    fn border_walls_follow_the_surface() {
        let b = Body::new(DVec3::ZERO, 1536, 1337);
        let mid = (b.n / 2 / CS) as i32;
        // the chunk whose layers hold the surface in the middle of face 2
        let hz = b.layer_of(b.height(b.dir(2, (mid * CS + CS / 2) as f64, (mid * CS + CS / 2) as f64))) as i32;
        let k = Key { body: 0, face: 2, level: 0, pos: IVec3::new(mid, mid, hz.div_euclid(CS)) };
        let Gen::Cells(c) = generate(&b, k, &[]) else { panic!("surface chunk has cells") };
        let m = crate::mesh::mesh(&c, 1);
        // every -x face sits at cell x = 0 of a column whose surface is inside the chunk; with +-30 m
        // terrain most of the 32 border columns qualify, and without skirts there would be none
        let west = m.quads.iter().filter(|q| (q[0] >> 28) & 7 == 1 && q[0] & 63 == 0).count();
        assert!(west >= 24, "{west} border faces on the -x side");
    }

    use super::*;

    #[test]
    fn fold_steps_over_a_seam_and_back() {
        let n = 64;
        for face in 0..6 {
            for k in 0..n {
                for (i, j) in [(-1, k), (n, k), (k, -1), (k, n)] {
                    let (f, fi, fj) = fold(face, i, j, n);
                    assert_ne!(f, face);
                    assert!((0..n).contains(&fi) && (0..n).contains(&fj));
                    // the folded cell touches an edge of its face, and one step over that edge is the cell we left
                    let back = [(-1, fj), (n, fj), (fi, -1), (fi, n)].into_iter().filter(|(a, b)| (a - fi).abs() + (b - fj).abs() == 1);
                    let home = (face, i.clamp(0, n - 1), j.clamp(0, n - 1));
                    assert!(back.map(|(a, b)| fold(f, a, b, n)).any(|c| c == home), "face {face} ({i}, {j})");
                }
            }
        }
    }

    #[test]
    fn folded_columns_are_neighbours_in_space() {
        let b = Body::new(DVec3::ZERO, 64, 1);
        let width = b.radius * FRAC_PI_2 / b.n as f64; // the widest column
        for face in 0..6 {
            for k in 0..b.n {
                for (i, j) in [(-1, k), (b.n, k), (k, -1), (k, b.n)] {
                    let (f, fi, fj) = fold(face, i, j, b.n);
                    let inside = b.dir(face, i.clamp(0, b.n - 1) as f64 + 0.5, j.clamp(0, b.n - 1) as f64 + 0.5);
                    let d = (b.dir(f, fi as f64 + 0.5, fj as f64 + 0.5) - inside).length() * b.radius;
                    assert!(d < width * 1.01, "face {face} ({i}, {j}) is {d} m from its neighbour");
                }
            }
        }
    }

    #[test]
    fn block_at_finds_the_block_of_its_centre() {
        let w = World::new(96, 32, 1337);
        for body in 0..2u8 {
            let n = w.bodies[body as usize].n;
            for face in 0..6u8 {
                for (i, j, l) in [(0, 0, 0), (n - 1, 0, 40), (n / 2, n / 3, 17), (5, n - 1, 99), (n - 1, n - 1, 3)] {
                    let b = Block { body, face, pos: IVec3::new(i, j, l) };
                    assert_eq!(w.block_at(w.block_centre(b)), b);
                }
            }
        }
    }

    #[test]
    fn blocks_are_cubic_and_half_a_metre_at_the_surface() {
        let b = Body::new(DVec3::ZERO, 3072, 1);
        let l = b.layer_of(b.radius);
        let thick = b.layer_r(l + 1.0) - b.layer_r(l);
        assert!((thick - BLOCK).abs() < 1e-3, "layer {thick} m");
        let mid = b.n as f64 * 0.5;
        let wide = (b.dir(2, mid + 1.0, mid) - b.dir(2, mid, mid)).length() * b.radius;
        assert!(wide > 0.5 && wide < 0.56, "widest column {wide} m");
        assert_eq!((b.root_level, b.roots()), (5, 3));
    }

    /// The padded strip of a chunk at a seam must equal the border cells of the chunk on the other face,
    /// or faces would be missing or doubled along the seam.
    #[test]
    fn padding_across_a_seam_matches_the_neighbouring_chunk() {
        let mut w = World::new(256, 32, 1337);
        let body = &w.bodies[0];
        let top = body.layer_of(body.radius) as i32 / CS;
        // an edit right at the seam, on the neighbour's side
        let (f, fi, fj) = fold(0, -1, 40, 256);
        let dug = Block { body: 0, face: f as u8, pos: IVec3::new(fi, fj, top * CS + 5) };
        let m = if w.block(dug) == 0 { 3 } else { 0 };
        w.set_block(dug, m);
        let body = &w.bodies[0];
        let cells = |k: Key| match generate(body, k, &w.edits_for(k)) {
            Gen::Cells(c) => c,
            Gen::Empty => Box::new([0; P * P * P]),
            Gen::Solid => Box::new([3; P * P * P]),
        };
        let mut checked = 0;
        for ck in top - 1..=top + 1 {
            let k = Key { body: 0, face: 0, level: 0, pos: IVec3::new(0, 1, ck) };
            let mine = cells(k);
            for y in 1..=CS {
                let (f, fi, fj) = fold(0, -1, CS + y - 1, 256);
                let nk = Key { face: f as u8, pos: IVec3::new(fi.div_euclid(CS), fj.div_euclid(CS), ck), ..k };
                let theirs = cells(nk);
                for z in 0..P {
                    let (lx, ly) = (fi.rem_euclid(CS) as usize + 1, fj.rem_euclid(CS) as usize + 1);
                    // the padding is the neighbour's border with the skirt rule applied: air within SKIRT
                    // cells under air (the neighbour's own border cells are never padding, so they keep it)
                    let mut want = theirs[pidx(lx, ly, z)];
                    if z < P - 1 && (z + 1..=(z + SKIRT).min(P - 1)).any(|a| theirs[pidx(lx, ly, a)] == 0) {
                        want = 0;
                    }
                    assert_eq!(mine[pidx(0, y as usize, z)], want, "y {y} z {z}");
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 3 * 32 * 34);
        let k = Key { body: 0, face: 0, level: 0, pos: IVec3::new(0, 1, top) };
        // the edit past the seam shows in the padding (a placed block under air is skirted away, so dig)
        if m == 0 {
            assert_eq!(cells(k)[pidx(0, 40 - CS as usize + 1, 6)], 0, "the edit past the seam shows in the padding");
        }
    }

    #[test]
    fn bounds_hold_the_chunk() {
        let b = Body::new(DVec3::new(10.0, 0.0, 0.0), 96, 1);
        for k in [Key { body: 0, face: 3, level: 0, pos: IVec3::new(2, 0, 3) }, Key { body: 0, face: 5, level: 1, pos: IVec3::new(0, 0, 0) }] {
            let bd = b.bounds(k);
            let (base, s) = (k.base().as_dvec3(), k.span() as f64);
            for i in 0..=4 {
                for j in 0..=4 {
                    for l in 0..=4 {
                        let g = base + DVec3::new(i as f64, j as f64, l as f64) * s / 4.0;
                        let p = b.centre + b.dir(k.face as usize, g.x, g.y) * b.layer_r(g.z.min(b.layers as f64));
                        assert!((p - bd.c).length() <= bd.rad + 1e-9);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod water_tests {
    use super::*;
    use crate::liquid::Ground;

    /// A column's gaps follow the terrain and its edits: generated ground is solid up to the surface, a dug
    /// hollow under it is a closed gap, a placed block splits the open air.
    #[test]
    fn gaps_follow_terrain_and_edits() {
        let mut w = World::new(1536, 512, 1337);
        let b0 = Block { body: 0, face: 2, pos: IVec3::new(700, 700, 0) };
        let c = World::col(b0);
        let zs = w.ground().gen_floor(c);
        assert_eq!(w.generated_block(Block { pos: IVec3::new(700, 700, zs - 1), ..b0 }) != AIR, true);
        assert_eq!(w.generated_block(Block { pos: IVec3::new(700, 700, zs), ..b0 }), AIR);
        assert_eq!(w.ground().gaps(c), vec![(zs, OPEN)]);
        for z in [zs - 6, zs - 5] {
            w.set_block(Block { pos: IVec3::new(700, 700, z), ..b0 }, AIR);
        }
        w.set_block(Block { pos: IVec3::new(700, 700, zs + 3), ..b0 }, BLOCKS.id("stone"));
        assert_eq!(w.ground().gaps(c), vec![(zs - 6, zs - 4), (zs, zs + 3), (zs + 4, OPEN)]);
    }

    /// The sea: a low column is water up to sea level from generation, and a point in it is under water.
    #[test]
    fn the_sea_is_generated_where_the_ground_is_low() {
        let w = World::new(1536, 512, 1337);
        let b = &w.bodies[0];
        let sea = b.sea.unwrap();
        // find a low column on face 2
        let (i, j) = (0..b.n).step_by(7).flat_map(|i| (0..b.n).step_by(7).map(move |j| (i, j))).find(|&(i, j)| b.height(b.dir(2, i as f64 + 0.5, j as f64 + 0.5)) < b.radius - 10.0).unwrap();
        let c = Col::new(0, 2, i, j);
        let (lo, hi) = w.ground().generated(c);
        assert!(lo < hi && hi == (sea * U as f64) as i64, "{lo} {hi} {}", sea * U as f64);
        let p = b.dir(2, i as f64 + 0.5, j as f64 + 0.5) * (b.layer_r(lo as f64 / U as f64) + 1.0);
        let r = w.water_surface(p).expect("under the sea");
        assert!((r - b.layer_r(sea)).abs() < 1e-3, "the surface at {r}"); // a unit is 0.1 mm
        assert!((r - (b.radius - SEA_DEPTH)).abs() < BLOCK, "the sea {r} within a block of its depth");
        assert_eq!(w.water_surface(p.normalize() * (b.radius + 5.0)), None);
        // the moon has no sea
        assert!(w.bodies[1].sea.is_none());
    }

    /// The standard planet is about OCEAN sea, the spawn is on land, and its ground stays within AMP.
    #[test]
    fn continents_leave_the_chosen_share_to_the_sea() {
        for seed in [1, 1337] {
            let w = World::new(12288, 512, seed);
            let b = &w.bodies[0];
            let sea = b.layer_r(b.sea.unwrap());
            let hs: Vec<f64> = lattice(20000).map(|d| b.height(d)).collect();
            let wet = hs.iter().filter(|h| **h < sea).count() as f64 / hs.len() as f64;
            assert!((wet - OCEAN).abs() < 0.06, "seed {seed}: {wet:.3} of the surface is sea");
            assert!(hs.iter().all(|h| (h - b.radius).abs() < AMP));
            let top = hs.iter().cloned().fold(f64::MIN, f64::max) - sea;
            assert!(top > 25.0, "seed {seed}: the highest ground is {top:.1} m over the sea");
            assert!(b.height(b.spawn) > sea + 2.0);
        }
    }

    /// Buckets poured into the planet's sea: the wave stays local (the edge of what is stored is open
    /// sea), dies down, and nothing stays stored; every unit is counted.
    #[test]
    fn a_bucket_in_the_sea_stays_local() {
        let mut w = World::new(1536, 512, 1);
        let b = &w.bodies[0];
        let (i, j) = (0..b.n).step_by(3).flat_map(|i| (0..b.n).step_by(3).map(move |j| (i, j))).find(|&(i, j)| b.height(b.dir(2, i as f64 + 0.5, j as f64 + 0.5)) < b.radius - 7.0).unwrap();
        let c = Col::new(0, 2, i, j);
        let (_, hi) = w.ground().generated(c);
        let World { bodies, edits, water, .. } = &mut w;
        assert!(water.pour(&Solid { bodies, edits }, c, (hi / U) as i32, 4 * U));
        let mut peak = 0;
        for _ in 0..60 * 30 {
            w.step_water();
            peak = peak.max(w.water.stored());
        }
        assert!(peak < 400, "{peak} columns stored");
        assert_eq!((w.water.stored(), w.water.counted()), (0, 4 * U));
    }
}

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
        assert!((pv.length() - std::f64::consts::TAU * crate::sky::SUN_DIST / crate::sky::YEAR).abs() < 0.01);
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
