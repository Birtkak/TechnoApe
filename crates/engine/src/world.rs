//! Terrain: bodies on the equi-angular cube-sphere grid (ADR 0003), generated from the seed, plus a sparse
//! edit map. A block is (face, column i, column j, layer); layers are geometric shells, so blocks stay cubic.
use crate::blocks::{AIR, BLOCKS};
use crate::liquid::{Col, Ground, Liquids, OPEN, U};
use fastnoise_lite::{FastNoiseLite, FractalType, NoiseType};
use glam::{DVec3, IVec3};
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};
use std::sync::Arc;

pub const BLOCK: f64 = 0.5;
/// The planet's sea lies this far under its nominal radius: about a third of it is sea (ADR 0020).
pub const SEA_DEPTH: f64 = 4.0;
pub const CS: i32 = 32; // cells per chunk edge
pub const P: usize = 34; // padded edge
pub const AMP: f64 = 30.0;
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
        Body { centre, radius, n, root_level: (n / CS).trailing_zeros() as u8, r0, lnq, layers, noise, sea: None }
    }
    /// Root chunks per face edge.
    pub fn roots(&self) -> i32 {
        (self.n / CS) >> self.root_level
    }
    /// Radius of layer coordinate l (l = 3.5 is the middle of layer 3).
    pub fn layer_r(&self, l: f64) -> f64 {
        self.r0 * (l * self.lnq).exp()
    }
    pub fn layer_of(&self, r: f64) -> f64 {
        (r / self.r0).ln() / self.lnq
    }
    /// Unit direction of grid point (i, j) on a face, in columns (i = 3.5 is the middle of column 3).
    pub fn dir(&self, face: usize, i: f64, j: f64) -> DVec3 {
        let th = FRAC_PI_2 / self.n as f64;
        let h = self.n as f64 * 0.5;
        let [n, s, t] = AX[face];
        (n.as_dvec3() + s.as_dvec3() * ((i - h) * th).tan() + t.as_dvec3() * ((j - h) * th).tan()).normalize()
    }
    /// Surface radius along unit direction d.
    pub fn height(&self, d: DVec3) -> f64 {
        let p = d * self.radius;
        self.radius + AMP * self.noise.get_noise_3d(p.x as f32, p.y as f32, p.z as f32) as f64
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
        let h = b.height(b.dir(c.face() as usize, c.i() as f64 + 0.5, c.j() as f64 + 0.5));
        let mut z = ((b.layer_of(h) - 0.5).floor() as i32 + 1).max(0);
        // the same test as `generated_under`, so the two never disagree by a layer
        while z > 0 && material(h - b.layer_r(z as f64 - 0.5)) == AIR {
            z -= 1;
        }
        while material(h - b.layer_r(z as f64 + 0.5)) != AIR {
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

pub fn material(depth: f64) -> u8 {
    if depth < 0.0 {
        AIR
    } else if depth < 0.5 {
        GEN[0]
    } else if depth < 4.0 {
        GEN[1]
    } else {
        GEN[2]
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

pub struct World {
    pub bodies: Arc<Vec<Body>>,
    /// Edits bucketed by level 0 chunk; value = material that replaces the generated one.
    pub edits: FxMap<Key, FxMap<IVec3, u8>>,
    pub edit_count: usize,
    /// Water (ADR 0020): on the server it flows; a client keeps the server's copy.
    pub water: Liquids,
}

impl World {
    /// A planet and its moon 3 km away, with `planet_n` and `moon_n` columns per face edge.
    pub fn new(planet_n: i32, moon_n: i32, seed: i32) -> Self {
        let mut planet = Body::new(DVec3::ZERO, planet_n, seed);
        planet.sea = Some(planet.layer_of(planet.radius - SEA_DEPTH));
        World {
            bodies: Arc::new(vec![planet, Body::new(DVec3::new(3000.0, 0.0, 0.0), moon_n, seed + 1)]),
            edits: FxMap::default(),
            edit_count: 0,
            water: Liquids::default(),
        }
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
        let body = &self.bodies[b.body as usize];
        let p = b.pos.as_dvec3() + 0.5;
        body.height(body.dir(b.face as usize, p.x, p.y))
    }
    /// The generated material of block b under a surface at radius `height` (its `column_height`).
    pub fn generated_under(&self, b: Block, height: f64) -> u8 {
        if b.pos.z < 0 {
            return GEN[2];
        }
        material(height - self.bodies[b.body as usize].layer_r(b.pos.z as f64 + 0.5))
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
/// a `wet` tile (sorted) are left out: the client's water mesh draws all of those.
pub fn sea_quads(body: &Body, key: Key, wet: &[Col]) -> Vec<[u32; 2]> {
    let Some(cz) = sea_cell(body, key) else { return Vec::new() };
    let s = 1i32 << key.level;
    let r = body.layer_r(body.sea.unwrap());
    let base = key.base();
    let n = CS as usize;
    let mut sea = vec![false; n * n];
    for y in 0..n {
        for x in 0..n {
            let (ci, cj) = (base.x + x as i32 * s, base.y + y as i32 * s);
            if wet.binary_search(&Col::new(key.body, key.face, ci, cj).tile()).is_err() {
                // the same sample as `generate`: the middle of the (coarse) column
                sea[y * n + x] = body.height(body.dir(key.face as usize, ci as f64 + 0.5 * s as f64, cj as f64 + 0.5 * s as f64)) < r;
            }
        }
    }
    let z = (cz - key.pos.z * CS + 1) as u32;
    let mut out = Vec::new();
    for y in 0..n {
        let mut x = 0;
        while x < n {
            if !sea[y * n + x] {
                x += 1;
                continue;
            }
            let mut w = 1;
            while x + w < n && sea[y * n + x + w] {
                w += 1;
            }
            let mut h = 1;
            while y + h < n && (x..x + w).all(|i| sea[(y + h) * n + i]) {
                h += 1;
            }
            for v in y..y + h {
                for u in x..x + w {
                    sea[v * n + u] = false;
                }
            }
            out.push([x as u32 | (y as u32) << 6 | z << 12 | ((w - 1) as u32) << 18 | ((h - 1) as u32) << 23 | 4 << 28, 0]);
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
    // one height per column; columns past a seam belong to the neighbouring face
    let nl = body.n >> key.level;
    let mut h = [0f32; P * P];
    let (mut hmin, mut hmax) = (f32::MAX, f32::MIN);
    for y in 0..P {
        for x in 0..P {
            let (f, ci, cj) = fold(key.face as usize, c0.x + x as i32, c0.y + y as i32, nl);
            let v = body.height(body.dir(f, (ci as f64 + 0.5) * s, (cj as f64 + 0.5) * s)) as f32;
            hmin = hmin.min(v);
            hmax = hmax.max(v);
            h[x + y * P] = v;
        }
    }
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
            let mut m = if core || d >= dirt { 3 } else if d >= grass { 2 } else if d >= 0.0 { 1 } else { 0 };
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
    fn border_walls_follow_the_surface() {
        let b = Body::new(DVec3::ZERO, 1536, 1337);
        let mid = (b.n / 2 / CS) as i32;
        // the chunk whose layers hold the surface in the middle of face 2
        let hz = b.layer_of(b.height(b.dir(2, (mid * CS + CS / 2) as f64, (mid * CS + CS / 2) as f64))) as i32;
        let k = Key { body: 0, face: 2, level: 0, pos: IVec3::new(mid, mid, hz.div_euclid(CS)) };
        let Gen::Cells(c) = generate(&b, k, &[]) else { panic!("surface chunk has cells") };
        let m = crate::mesh::mesh(&c, false);
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
        assert!((r - (b.radius - SEA_DEPTH)).abs() < 1e-3, "the surface at {r}"); // a unit is 0.1 mm
        assert_eq!(w.water_surface(p.normalize() * (b.radius + 5.0)), None);
        // the moon has no sea
        assert!(w.bodies[1].sea.is_none());
    }

    /// Buckets poured into the planet's sea: the wave stays local (the edge of what is stored is open
    /// sea), dies down, and nothing stays stored; every unit is counted.
    #[test]
    fn a_bucket_in_the_sea_stays_local() {
        let mut w = World::new(256, 512, 1);
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
