//! Terrain: bodies on the equi-angular cube-sphere grid (ADR 0003), generated from the seed, plus a sparse
//! edit map. A block is (face, column i, column j, layer); layers are geometric shells, so blocks stay cubic.
use fastnoise_lite::{FastNoiseLite, FractalType, NoiseType};
use glam::{DVec3, IVec3};
use std::collections::HashMap;
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};
use std::sync::Arc;

pub const BLOCK: f64 = 0.5;
pub const CS: i32 = 32; // cells per chunk edge
pub const P: usize = 34; // padded edge
pub const AMP: f64 = 30.0;
pub const CRUST: f64 = 64.0;

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
        Body { centre, radius, n, root_level: (n / CS).trailing_zeros() as u8, r0, lnq, layers, noise }
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

#[derive(Clone, Copy)]
pub struct Bounds {
    pub c: DVec3,
    pub rad: f64,
}

pub fn material(depth: f64) -> u8 {
    if depth < 0.0 {
        0
    } else if depth < 0.5 {
        1
    } else if depth < 4.0 {
        2
    } else {
        3
    }
}

/// One block of a body: pos = (column i, column j, layer).
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
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
    pub edits: HashMap<Key, HashMap<IVec3, u8>>,
    pub edit_count: usize,
}

impl World {
    /// A planet and its moon 3 km away, with `planet_n` and `moon_n` columns per face edge.
    pub fn new(planet_n: i32, moon_n: i32, seed: i32) -> Self {
        World {
            bodies: Arc::new(vec![Body::new(DVec3::ZERO, planet_n, seed), Body::new(DVec3::new(3000.0, 0.0, 0.0), moon_n, seed + 1)]),
            edits: HashMap::new(),
            edit_count: 0,
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
        if b.pos.z < 0 {
            return 3;
        }
        let body = &self.bodies[b.body as usize];
        let p = b.pos.as_dvec3() + 0.5;
        material(body.height(body.dir(b.face as usize, p.x, p.y)) - body.layer_r(p.z))
    }
    fn edit(&self, b: Block) -> Option<u8> {
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
        let mut take = |m: &HashMap<IVec3, u8>| {
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
pub fn generate(body: &Body, key: Key, edits: &[(u32, u8)]) -> Gen {
    let s = (1i32 << key.level) as f64;
    let c0 = key.pos * CS - 1; // padded cell (0, 0, 0) in cells of this level
    let r_lo = body.layer_r(c0.z as f64 * s);
    let r_hi = body.layer_r((c0.z + P as i32) as f64 * s);
    if edits.is_empty() {
        // whole chunk outside the terrain's height range: no noise needed
        if r_lo > body.radius + AMP {
            return Gen::Empty;
        }
        if r_hi < body.radius - AMP {
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
        if (r_hi as f32) < hmin {
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
            let m = if core || d >= dirt { 3 } else if d >= grass { 2 } else if d >= 0.0 { 1 } else { 0 };
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
