//! Binary greedy mesher: occupancy bitmasks per column, faces culled 34 cells at a time with shifts,
//! then a greedy merge per slice over 32-bit rows (same material only).
//! Quad = 8 bytes: lo = x6 y6 z6 (w-1)5 (h-1)5 dir3, hi = material.
use crate::world::{pidx, CS, P};

/// Quads of one chunk, grouped by face direction (0..6 = +x,-x,+y,-y,+z,-z).
pub struct Mesh {
    pub quads: Vec<[u32; 2]>,
    pub counts: [u32; 6],
}

pub fn mesh(cells: &[u8; P * P * P], greedy: bool) -> Mesh {
    // cols[a][v*P+u] = bits along axis a; u = (a+1)%3, v = (a+2)%3.
    let mut cols = [[0u64; P * P]; 3];
    for z in 0..P {
        for y in 0..P {
            for x in 0..P {
                if cells[pidx(x, y, z)] != 0 {
                    cols[0][z * P + y] |= 1 << x; // a=x: u=y, v=z
                    cols[1][x * P + z] |= 1 << y; // a=y: u=z, v=x
                    cols[2][y * P + x] |= 1 << z; // a=z: u=x, v=y
                }
            }
        }
    }
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
                    let f = if s == 0 { col & !(col >> 1) } else { col & !(col << 1) };
                    let mut bits = (f >> 1) & 0xFFFF_FFFF;
                    while bits != 0 {
                        let d = bits.trailing_zeros() as usize;
                        bits &= bits - 1;
                        planes[d][v] |= 1 << u;
                    }
                }
            }
            let mat = |u: usize, v: usize, d: usize| -> u8 {
                let (u, v, d) = (u + 1, v + 1, d + 1);
                match a {
                    0 => cells[pidx(d, u, v)],
                    1 => cells[pidx(v, d, u)],
                    _ => cells[pidx(u, v, d)],
                }
            };
            let out = &mut groups[dir];
            for d in 0..CS as usize {
                let rows = &mut planes[d];
                for v in 0..CS as usize {
                    while rows[v] != 0 {
                        let u = rows[v].trailing_zeros() as usize;
                        let m = mat(u, v, d);
                        let mut w = 1;
                        if greedy {
                            while u + w < 32 && rows[v] >> (u + w) & 1 == 1 && mat(u + w, v, d) == m {
                                w += 1;
                            }
                        }
                        let mask = (((1u64 << w) - 1) << u) as u32;
                        let mut h = 1;
                        if greedy {
                            while v + h < 32 && rows[v + h] & mask == mask && (u..u + w).all(|i| mat(i, v + h, d) == m) {
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
                        out.push([lo, m as u32]);
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
    Mesh { quads, counts }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn single_cell_has_six_faces_and_slab_merges() {
        let mut c = Box::new([0u8; P * P * P]);
        c[pidx(5, 5, 5)] = 3;
        let m = mesh(&c, true);
        assert_eq!(m.counts, [1; 6]);
        let mut c = Box::new([0u8; P * P * P]);
        for z in 1..33 {
            for x in 1..33 {
                c[pidx(x, 1, z)] = 1;
            }
        }
        let m = mesh(&c, true);
        assert_eq!(m.counts[2], 1); // whole top merged into one 32x32 quad
        let m = mesh(&c, false);
        assert_eq!(m.counts[2], 1024);
    }
}
