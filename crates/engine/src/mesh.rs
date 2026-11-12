//! Binary greedy mesher: occupancy bitmasks per column, faces culled 34 cells at a time with shifts,
//! then a greedy merge per slice over 32-bit rows (same material only).
//! Quad = 8 bytes: lo = x6 y6 z6 (w-1)5 (h-1)5 dir3, hi = material8 | ao8 << 8: ambient occlusion at each
//! corner, 2 bits (0 open .. 3 a corner shut in), corner (u, v) at bits 2 * (u + 2v) (ADR 0022).
use crate::world::{pidx, CS, P};
use std::sync::LazyLock;

/// Quads of one chunk, grouped by face direction (0..6 = +x,-x,+y,-y,+z,-z), then `cut` quads of cutout blocks
/// (ADR 0069: drawn by the pipeline that discards, their own geometry in the chunk's BLAS), then `water` quads of
/// the generated sea's surface (`world::sea_quads`), drawn by the water pipeline.
pub struct Mesh {
    pub quads: Vec<[u32; 2]>,
    pub counts: [u32; 6],
    pub cut: u32,
    /// Glass quads (ADR 0081), after the cutout ones: drawn blended after the light, the BLAS's second geometry.
    pub glass: u32,
    pub water: u32,
}

/// One bit per block id, set for a block that is not opaque (cutout or glass, `blocks::See`): the mesher's inner
/// loop reads this, not the row. `GLASS`: the same for glass alone (ADR 0081).
static SEE: LazyLock<Box<[u64; 1024]>> = LazyLock::new(|| bits(|id| !crate::blocks::BLOCKS.opaque(id)));
static GLASS: LazyLock<Box<[u64; 1024]>> = LazyLock::new(|| bits(|id| crate::blocks::BLOCKS.glass(id)));
fn bits(f: impl Fn(u16) -> bool) -> Box<[u64; 1024]> {
    let mut t = Box::new([0u64; 1024]);
    for d in crate::blocks::BLOCKS.placeable().filter(|d| f(d.id)) {
        t[d.id as usize >> 6] |= 1 << (d.id & 63);
    }
    t
}
fn is_in(t: &[u64; 1024], c: u16) -> bool {
    t[c as usize >> 6] >> (c & 63) & 1 == 1
}

/// `lattice` > 1 merges faces into quads that stay inside the cells of a lattice that many cells wide, where the
/// renderer places vertices by interpolation (`Body::lattice`, ADR 0025 I2); 1 merges nothing. Every solid block
/// but glass hides the faces behind it: the fast Leaves setting, and every coarse level.
pub fn mesh(cells: &[u16; P * P * P], lattice: usize) -> Mesh {
    mesh_cut(cells, lattice, false)
}

/// `mesh`, and with `cutout` (fancy leaves on level 0, ADR 0069) a face is culled only against an opaque
/// neighbour, so the faces behind and between cutout blocks stay, and the cutout blocks' quads go last as `cut`.
/// Glass (ADR 0081) always hides only its own kind and is hidden only by what hides; its quads go after `cut`.
pub fn mesh_cut(cells: &[u16; P * P * P], lattice: usize, cutout: bool) -> Mesh {
    // cols[a][v*P+u] = bits along axis a; u = (a+1)%3, v = (a+2)%3. opq: the same of the cells that hide what is
    // behind them, and gls of glass, made only for a chunk with glass or (fancy) cutout blocks
    let mut cols = [[0u64; P * P]; 3];
    let mut opq_cols: Vec<[u64; P * P]> = Vec::new();
    let mut gls_cols: Vec<[u64; P * P]> = Vec::new();
    let (see_t, glass_t): (&[u64; 1024], &[u64; 1024]) = (&SEE, &GLASS);
    let mut any_see = false;
    for z in 0..P {
        for y in 0..P {
            for x in 0..P {
                let c = cells[pidx(x, y, z)];
                if c != 0 {
                    cols[0][z * P + y] |= 1 << x; // a=x: u=y, v=z
                    cols[1][x * P + z] |= 1 << y; // a=y: u=z, v=x
                    cols[2][y * P + x] |= 1 << z; // a=z: u=x, v=y
                    any_see |= is_in(see_t, c);
                }
            }
        }
    }
    let (mut any_cut, mut any_glass) = (false, false);
    if any_see {
        // a second pass, only in a chunk that holds a block seen through
        opq_cols = vec![[0u64; P * P]; 3];
        gls_cols = vec![[0u64; P * P]; 3];
        for z in 0..P {
            for y in 0..P {
                for x in 0..P {
                    let c = cells[pidx(x, y, z)];
                    if c == 0 {
                        continue;
                    }
                    let glass = is_in(glass_t, c);
                    let cut = !glass && cutout && is_in(see_t, c);
                    if glass {
                        any_glass = true;
                        gls_cols[0][z * P + y] |= 1 << x;
                        gls_cols[1][x * P + z] |= 1 << y;
                        gls_cols[2][y * P + x] |= 1 << z;
                    } else if cut {
                        any_cut = true;
                    } else {
                        opq_cols[0][z * P + y] |= 1 << x;
                        opq_cols[1][x * P + z] |= 1 << y;
                        opq_cols[2][y * P + x] |= 1 << z;
                    }
                }
            }
        }
    }
    let cut_t = see_t;
    let opq: &[[u64; P * P]] = if any_cut || any_glass { &opq_cols } else { &cols };
    let mut groups: [Vec<[u32; 2]>; 8] = Default::default();
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
                    let mut f = if s == 0 { col & !(o >> 1) } else { col & !(o << 1) };
                    if any_glass {
                        // a glass face whose neighbour is the same glass goes (a glass wall has no inner faces)
                        let gl = gls_cols[a][(v + 1) * P + (u + 1)];
                        let mut g = f & gl & if s == 0 { gl >> 1 } else { gl << 1 };
                        while g != 0 {
                            let d = g.trailing_zeros() as usize;
                            g &= g - 1;
                            let (n, k) = (if s == 0 { d + 1 } else { d - 1 }, (u + 1, v + 1));
                            let at = |d: usize| match a {
                                0 => pidx(d, k.0, k.1),
                                1 => pidx(k.1, d, k.0),
                                _ => pidx(k.0, k.1, d),
                            };
                            if cells[at(d)] == cells[at(n)] {
                                f &= !(1 << d);
                            }
                        }
                    }
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
                        let g = if any_glass && is_in(glass_t, m) {
                            7
                        } else if any_cut && is_in(cut_t, m) {
                            6
                        } else {
                            dir
                        };
                        groups[g].push([lo, m as u32 | o << 8]);
                    }
                }
            }
        }
    }
    let mut counts = [0u32; 6];
    let mut quads = Vec::with_capacity(groups.iter().map(|g| g.len()).sum());
    for (i, g) in groups.iter().enumerate() {
        if i < 6 {
            counts[i] = g.len() as u32;
        }
        quads.extend_from_slice(g);
    }
    Mesh { quads, counts, cut: groups[6].len() as u32, glass: groups[7].len() as u32, water: 0 }
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

    /// The slow path, face by face: each solid cell's face whose neighbour does not hide it, as (cell, dir,
    /// block). `hides` is the rule: before ADR 0069 any solid neighbour, with cutout blocks an opaque one.
    fn faces_one_by_one(c: &[u16; P * P * P], hides: impl Fn(u16, u16) -> bool) -> Vec<([usize; 3], u32, u16)> {
        let mut out = Vec::new();
        for z in 1..P - 1 {
            for y in 1..P - 1 {
                for x in 1..P - 1 {
                    let m = c[pidx(x, y, z)];
                    if m == 0 {
                        continue;
                    }
                    for (dir, (dx, dy, dz)) in [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)].into_iter().enumerate() {
                        let n = c[pidx((x as i32 + dx) as usize, (y as i32 + dy) as usize, (z as i32 + dz) as usize)];
                        if !hides(m, n) {
                            out.push(([x - 1, y - 1, z - 1], dir as u32, m));
                        }
                    }
                }
            }
        }
        out.sort();
        out
    }
    /// A quad of one cell (lattice 1 merges nothing) as (the cell it belongs to, dir, block).
    fn face_of(q: &[u32; 2]) -> ([usize; 3], u32, u16) {
        let dir = (q[0] >> 28) & 7;
        let mut p = [(q[0] & 63) as usize, (q[0] >> 6 & 63) as usize, (q[0] >> 12 & 63) as usize];
        if dir % 2 == 0 {
            p[dir as usize / 2] -= 1; // a + face sits on the far side of its cell
        }
        (p, dir, (q[1] & 255) as u16)
    }

    /// ADR 0069: with cutout blocks a face is culled only against an opaque neighbour; without, the mesher is
    /// the old one. Both against the slow path, on a chunk of stone, leaves, shrub and air at random.
    #[test]
    fn faces_are_culled_only_against_opaque_neighbours() {
        let b = &crate::blocks::BLOCKS;
        let (stone, leaves, shrub) = (b.id("stone"), b.id("broadleaf leaves"), b.id("shrub"));
        let mut c = Box::new([0u16; P * P * P]);
        let mut h = 12345u32;
        for v in c.iter_mut() {
            h ^= h << 13;
            h ^= h >> 17;
            h ^= h << 5;
            *v = [0, 0, stone, leaves, leaves, shrub][(h % 6) as usize];
        }
        let fancy = mesh_cut(&c, 1, true);
        let mut got: Vec<_> = fancy.quads.iter().map(face_of).collect();
        got.sort();
        assert_eq!(got, faces_one_by_one(&c, |_, n| b.opaque(n)), "fancy");
        let n_opq = fancy.quads.len() - fancy.cut as usize;
        assert!(fancy.cut > 0 && fancy.quads[n_opq..].iter().all(|q| !b.opaque((q[1] & 255) as u16)), "the cutout quads come last");
        assert!(fancy.quads[..n_opq].iter().all(|q| b.opaque((q[1] & 255) as u16)));
        for fast in [mesh(&c, 1), mesh_cut(&c, 1, false)] {
            let mut got: Vec<_> = fast.quads.iter().map(face_of).collect();
            got.sort();
            assert_eq!(got, faces_one_by_one(&c, |_, n| n != 0), "fast is the old mesher");
            assert_eq!(fast.cut, 0);
        }
        // a chunk with no cutout block meshes the same either way
        let rock: Box<[u16; P * P * P]> = Box::new(c.map(|v| if v == 0 { 0 } else { stone }));
        assert_eq!(mesh_cut(&rock, 8, true).quads, mesh(&rock, 8).quads);
    }

    /// ADR 0081: a glass face is culled against an opaque neighbour or the same glass; nothing is culled against
    /// glass; the glass quads go last, after the cutout ones, fast or fancy.
    #[test]
    fn glass_is_culled_only_against_opaque_or_its_own_kind() {
        let b = &crate::blocks::BLOCKS;
        let (stone, leaves, glass, green) = (b.id("stone"), b.id("broadleaf leaves"), b.id("glass"), b.id("green glass"));
        let mut c = Box::new([0u16; P * P * P]);
        let mut h = 777u32;
        for v in c.iter_mut() {
            h ^= h << 13;
            h ^= h >> 17;
            h ^= h << 5;
            *v = [0, 0, stone, leaves, glass, glass, green][(h % 7) as usize];
        }
        for fancy in [true, false] {
            let m = mesh_cut(&c, 1, fancy);
            let mut got: Vec<_> = m.quads.iter().map(face_of).collect();
            got.sort();
            // what hides a face: an opaque block (fast: leaves too), or the same glass
            let hides = |me: u16, n: u16| n == me && b.glass(me) || if fancy { b.opaque(n) } else { n != 0 && !b.glass(n) };
            assert_eq!(got, faces_one_by_one(&c, hides), "fancy {fancy}");
            let g0 = m.quads.len() - m.glass as usize;
            assert!(m.glass > 0 && m.quads[g0..].iter().all(|q| b.glass((q[1] & 255) as u16)), "the glass quads come last");
            assert!(m.quads[..g0].iter().all(|q| !b.glass((q[1] & 255) as u16)));
            assert_eq!(m.cut > 0, fancy);
        }
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
