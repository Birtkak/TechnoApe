//! Binary greedy mesher: occupancy bitmasks per column, faces culled 34 cells at a time with shifts,
//! then a greedy merge per slice over 32-bit rows (same material only).
//! Quad = 8 bytes: lo = x6 y6 z6 (w-1)5 (h-1)5 dir3, hi = material8 | ao8 << 8: ambient occlusion at each
//! corner, 2 bits (0 open .. 3 a corner shut in), corner (u, v) at bits 2 * (u + 2v) (ADR 0022).
use crate::world::{pidx, CS, P};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering::Relaxed};

// ---- the see-through spike (prototypes/see-through, ADR 0068): switches set once from a run's arguments ----
/// bit 0: cutout leaves (a face is culled only against an opaque neighbour, near chunks only); bit 1: still box
/// models on grass, bits 2-3 their density (0: 1 per 16 columns, 1: 1 per 4, 2: every column); bits 8+ are
/// the shader's (`opt.x`, shader.wgsl `SPIKE_*`)
pub static SPIKE: AtomicU32 = AtomicU32::new(0);
/// Near meshes made, their mesh time in ns, all their quads, the model quads among them.
pub static STATS: [AtomicU64; 4] = [AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)];
/// The spike's see-through blocks: the four leaves, shrub, reed.
pub fn is_cut(b: u16) -> bool {
    matches!(b, 15..=18 | 20 | 21)
}
fn is_grass(b: u16) -> bool {
    matches!(b, 1 | 23..=29)
}
/// The spike's box models: (stand-in block for a ray's colour, first face in the face table, faces). Must match
/// `MODELS` in the client's spike.rs.
pub const MODEL_FACES: [(u16, u32, u32); 3] = [(3, 0, 18), (16, 18, 4), (12, 22, 36)];

/// Quads of one chunk, grouped by face direction (0..6 = +x,-x,+y,-y,+z,-z), then `water` quads of the
/// generated sea's surface (`world::sea_quads`), drawn by the water pipeline.
pub struct Mesh {
    pub quads: Vec<[u32; 2]>,
    pub counts: [u32; 6],
    pub water: u32,
    /// The still box models' quads (the spike), after the land's and before the water's.
    pub models: u32,
}

/// `lattice` > 1 merges faces into quads that stay inside the cells of a lattice that many cells wide, where the
/// renderer places vertices by interpolation (`Body::lattice`, ADR 0025 I2); 1 merges nothing.
pub fn mesh(cells: &[u16; P * P * P], lattice: usize) -> Mesh {
    mesh_at(cells, lattice, None)
}

/// `mesh`, and for a near chunk (`near`: its seed for the models' places) the spike's switches.
pub fn mesh_at(cells: &[u16; P * P * P], lattice: usize, near: Option<u32>) -> Mesh {
    let t = std::time::Instant::now();
    let spike = SPIKE.load(Relaxed);
    let cut = near.is_some() && spike & 1 != 0;
    // cols[a][v*P+u] = bits along axis a; u = (a+1)%3, v = (a+2)%3. opq: the same of opaque cells only
    let mut cols = [[0u64; P * P]; 3];
    // only a cutout mesh pays for the second set (27 KB to zero)
    let mut opq_cols = if cut { vec![[0u64; P * P]; 3] } else { Vec::new() };
    for z in 0..P {
        for y in 0..P {
            for x in 0..P {
                let c = cells[pidx(x, y, z)];
                if c != 0 {
                    cols[0][z * P + y] |= 1 << x; // a=x: u=y, v=z
                    cols[1][x * P + z] |= 1 << y; // a=y: u=z, v=x
                    cols[2][y * P + x] |= 1 << z; // a=z: u=x, v=y
                    if cut && !is_cut(c) {
                        opq_cols[0][z * P + y] |= 1 << x;
                        opq_cols[1][x * P + z] |= 1 << y;
                        opq_cols[2][y * P + x] |= 1 << z;
                    }
                }
            }
        }
    }
    let opq: &[[u64; P * P]] = if cut { &opq_cols } else { &cols };
    let mut groups: [Vec<[u32; 2]>; 6] = Default::default();
    let mut planes = vec![[0u32; 32]; 32]; // planes[depth][v] = bits over u
    for a in 0..3 {
        for s in 0..2 {
            let dir = a * 2 + s;
            for p in planes.iter_mut() {
                *p = [0; 32];
            }
            for v in 0..CS as usize {
                for u in 0..CS as usize {
                    let col = cols[a][(v + 1) * P + (u + 1)];
                    let o = opq[a][(v + 1) * P + (u + 1)];
                    let f = if s == 0 { col & !(o >> 1) } else { col & !(o << 1) };
                    let mut bits = (f >> 1) & 0xFFFF_FFFF;
                    while bits != 0 {
                        let d = bits.trailing_zeros() as usize;
                        bits &= bits - 1;
                        planes[d][v] |= 1 << u;
                    }
                }
            }
            let mat = |u: usize, v: usize, d: usize| -> u16 {
                let (u, v, d) = (u + 1, v + 1, d + 1);
                match a {
                    0 => cells[pidx(d, u, v)],
                    1 => cells[pidx(v, d, u)],
                    _ => cells[pidx(u, v, d)],
                }
            };
            // the cells in front of the face touching each corner: two sides and the diagonal, as voxel games
            // have always done it; the padding holds every one
            let occ = |u: usize, v: usize, d: usize| -> bool {
                let i = match a {
                    0 => pidx(d, u, v),
                    1 => pidx(v, d, u),
                    _ => pidx(u, v, d),
                };
                cells[i] != 0
            };
            let ao = |u: usize, v: usize, d: usize| -> u32 {
                let fd = if s == 0 { d + 2 } else { d };
                let mut bits = 0;
                for cv in 0..2 {
                    for cu in 0..2 {
                        let (pu, pv) = (u + 2 * cu, v + 2 * cv);
                        let (s1, s2, c) = (occ(pu, v + 1, fd), occ(u + 1, pv, fd), occ(pu, pv, fd));
                        let o = if s1 && s2 { 3 } else { s1 as u32 + s2 as u32 + c as u32 };
                        bits |= o << (2 * (cu + 2 * cv));
                    }
                }
                bits
            };
            let out = &mut groups[dir];
            for d in 0..CS as usize {
                let rows = &mut planes[d];
                for v in 0..CS as usize {
                    while rows[v] != 0 {
                        let u = rows[v].trailing_zeros() as usize;
                        let m = mat(u, v, d);
                        let o = ao(u, v, d);
                        // a merged quad has one occlusion per corner: only open faces merge
                        let open = |u: usize, v: usize| ao(u, v, d) == 0;
                        let mut w = 1;
                        if lattice > 1 && o == 0 {
                            // never across a lattice line
                            while u + w < 32 && (u + w) % lattice != 0 && rows[v] >> (u + w) & 1 == 1 && mat(u + w, v, d) == m && open(u + w, v) {
                                w += 1;
                            }
                        }
                        let mask = (((1u64 << w) - 1) << u) as u32;
                        let mut h = 1;
                        if lattice > 1 && o == 0 {
                            while v + h < 32 && (v + h) % lattice != 0 && rows[v + h] & mask == mask && (u..u + w).all(|i| mat(i, v + h, d) == m && open(i, v + h)) {
                                h += 1;
                            }
                        }
                        for r in &mut rows[v..v + h] {
                            *r &= !mask;
                        }
                        // position of the quad's min corner in cells; + faces sit on the far side of the cell
                        let dd = d + if s == 0 { 1 } else { 0 };
                        let (x, y, z) = match a {
                            0 => (dd, u, v),
                            1 => (v, dd, u),
                            _ => (u, v, dd),
                        };
                        let lo = x as u32 | (y as u32) << 6 | (z as u32) << 12 | ((w - 1) as u32) << 18
                            | ((h - 1) as u32) << 23 | (dir as u32) << 28;
                        out.push([lo, m as u32 | o << 8]);
                    }
                }
            }
        }
    }
    let mut counts = [0u32; 6];
    let mut quads = Vec::with_capacity(groups.iter().map(|g| g.len()).sum());
    for (i, g) in groups.iter().enumerate() {
        counts[i] = g.len() as u32;
        quads.extend_from_slice(g);
    }
    let mut models = 0;
    if let (Some(seed), true) = (near, spike & 2 != 0) {
        let n0 = quads.len();
        clutter(cells, seed, (spike >> 2) & 3, &mut quads);
        models = (quads.len() - n0) as u32;
    }
    if near.is_some() {
        for (s, v) in STATS.iter().zip([1, t.elapsed().as_nanos() as u64, quads.len() as u64, models as u64]) {
            s.fetch_add(v, Relaxed);
        }
    }
    Mesh { quads, counts, water: 0, models }
}

fn hash3(x: u32, y: u32, z: u32, seed: u32) -> u32 {
    let mut h = x.wrapping_mul(73856093) ^ y.wrapping_mul(19349663) ^ z.wrapping_mul(83492791) ^ seed.wrapping_mul(2654435761);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    h ^ (h >> 16)
}

/// The spike's still box models: on the air cell over a grass top, by the cell's hash one in `1 << (4 - 2 * d)`
/// columns, a model, its turn and its place in the cell. A model quad: lo = x6 y6 z6 (the air cell) | turn4 << 18
/// | jitter 3 + 3 << 22 | 1 << 31; hi = stand-in block | face << 16 (shader.wgsl `model_corner`).
fn clutter(cells: &[u16; P * P * P], seed: u32, d: u32, out: &mut Vec<[u32; 2]>) {
    let every = 1u32 << (4 - 2 * d.min(2));
    for y in 1..P - 1 {
        for x in 1..P - 1 {
            // from the padding's layer 0: grass there tops this chunk's layer 0
            for z in 0..P - 2 {
                let c = cells[pidx(x, y, z)];
                if is_grass(c) && cells[pidx(x, y, z + 1)] == 0 {
                    let h = hash3(x as u32, y as u32, z as u32, seed);
                    if h % every == 0 {
                        let k = (h >> 8) % 10;
                        let (mat, first, n) = MODEL_FACES[if k < 4 { 0 } else if k < 8 { 1 } else { 2 }];
                        let lo = (x - 1) as u32 | ((y - 1) as u32) << 6 | (z as u32) << 12 | ((h >> 12) & 15) << 18 | ((h >> 16) & 63) << 22 | 1 << 31;
                        for f in first..first + n {
                            out.push([lo, mat as u32 | f << 16]);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn single_cell_has_six_faces_and_slab_merges() {
        let mut c = Box::new([0u16; P * P * P]);
        c[pidx(5, 5, 5)] = 3;
        let m = mesh(&c, 8);
        assert_eq!(m.counts, [1; 6]);
        let mut c = Box::new([0u16; P * P * P]);
        for z in 1..33 {
            for x in 1..33 {
                c[pidx(x, 1, z)] = 1;
            }
        }
        let m = mesh(&c, 8);
        assert_eq!(m.counts[2], 16); // the top merged into 8x8 quads, one per lattice cell
        let m = mesh(&c, 1);
        assert_eq!(m.counts[2], 1024);
    }

    #[test]
    fn a_floor_by_a_wall_is_occluded_at_the_wall() {
        // a floor at y = 1, a wall along x = 6 on top of it: the floor cell at x = 5 has its +x corners shut
        let mut c = Box::new([0u16; P * P * P]);
        for z in 1..33 {
            for x in 1..33 {
                c[pidx(x, 1, z)] = 1;
            }
            c[pidx(6, 2, z)] = 1;
        }
        let m = mesh(&c, 1);
        let top = |x: u32, z: u32| m.quads.iter().find(|q| (q[0] >> 28) & 7 == 2 && q[0] & 63 == x && (q[0] >> 12) & 63 == z).unwrap()[1];
        // a=y: u = z, v = x; corners with v = 1 (x + 1) touch the wall
        let ao = top(4, 10) >> 8;
        assert_eq!(ao & 0b11, 0);
        assert_eq!((ao >> 4) & 0b11, 2); // the wall beside and diagonal
        assert_eq!(top(1, 10) >> 8, 0, "open floor");
    }
}
