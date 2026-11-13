//! Binary greedy mesher: occupancy bitmasks per column, faces culled 34 cells at a time with shifts,
//! then a greedy merge per slice over 32-bit rows (same material only).
//! Quad = 8 bytes: lo = x6 y6 z6 (w-1)5 (h-1)5 dir3, hi = material8 | ao8 << 8: ambient occlusion at each
//! corner, 2 bits (0 open .. 3 a corner shut in), corner (u, v) at bits 2 * (u + 2v) (ADR 0022); a joining
//! block's face (lamps) has join4 << 16: the sides u-, u+, v-, v+ that meet the same block's open face (ADR 0085);
//! a face of cracked rock has crack4 << 20: the sides u-, u+, v-, v+ that run along a crack (ADR 0094).
use crate::world::{pidx, CS, P};
use std::sync::LazyLock;

/// Quads of one chunk, grouped by face direction (0..6 = +x,-x,+y,-y,+z,-z), then `cut` quads of cutout blocks
/// (ADR 0069: drawn by the pipeline that discards, their own geometry in the chunk's BLAS), then `water` quads of
/// the generated sea's surface (`world::sea_quads`), drawn by the water pipeline.
pub struct Mesh {
    pub quads: Vec<[u32; 2]>,
    pub counts: [u32; 6],
    pub cut: u32,
    /// How many of `cut` a ray may pass through an empty texel of (fancy leaves: all; fast: none, ADR 0088).
    pub ray_cut: u32,
    /// How many of `cut` open on air (bit 25), first among them: the client grows its fluff cards on these alone
    /// (ADR 0103), and draws no fluff for the inner faces fancy leaves keep.
    pub fluff: u32,
    /// Glass quads (ADR 0081), after the cutout ones: drawn blended after the light, the BLAS's second geometry.
    pub glass: u32,
    /// Still model quads (ADR 0068, 0082: clutter), after the glass ones and before the water: their own pipeline,
    /// not in the BLAS.
    pub still: u32,
    pub water: u32,
}

/// One bit per block id, set for a block that is not opaque (cutout or glass, `blocks::See`): the mesher's inner
/// loop reads this, not the row. `GLASS`: the same for glass alone (ADR 0081).
static SEE: LazyLock<Box<[u64; 1024]>> = LazyLock::new(|| bits(|id| !crate::blocks::BLOCKS.opaque(id)));
static GLASS: LazyLock<Box<[u64; 1024]>> = LazyLock::new(|| bits(|id| crate::blocks::BLOCKS.glass(id)));
static JOINS: LazyLock<Box<[u64; 1024]>> = LazyLock::new(|| bits(|id| crate::blocks::BLOCKS.joins(id)));
static WOOD: LazyLock<Box<[u64; 1024]>> = LazyLock::new(|| bits(|id| crate::blocks::BLOCKS.wood(id)));
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
/// but glass hides the faces behind it: every coarse level.
pub fn mesh(cells: &[u16; P * P * P], lattice: usize) -> Mesh {
    mesh_cut(cells, lattice, Cut::Solid, true)
}

/// How level 0 meshes cutout blocks (leaves; ADR 0069, 0088).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cut {
    /// As any block: they hide what is behind them, their quads lie with the rest (coarse levels).
    Solid,
    /// Fast leaves: the faces between two cutout blocks go, an opaque block's behind one stays.
    Shell,
    /// Fancy leaves: a face is culled only against an opaque neighbour.
    Full,
}

/// `mesh` with level 0's rule for cutout blocks, `cut`. With `Shell` and `Full` the cutout blocks' quads go
/// last as `cut`, for the pipeline that leaves their empty texels out.
/// Glass (ADR 0081) always hides only its own kind and is hidden only by what hides; its quads go after `cut`.
/// `glass`: the cells may hold glass. Generation makes none, so a `Solid` chunk without edits passes false and
/// its first pass looks at no block's kind, as before glass.
pub fn mesh_cut(cells: &[u16; P * P * P], lattice: usize, cut: Cut, glass: bool) -> Mesh {
    mesh_cracked(cells, lattice, cut, glass, None)
}

/// `mesh_cut`, and `cracks` (per cell its joint faces and the bedding line across each side face, `stream::cracks`,
/// padded as `cells`) draws each crack on the faces beside it: those faces never merge. Bits 20-31 of a quad are the
/// crack's on rock and the twigs' (ADR 0090) on a cutout block: rock that cracks is never cutout.
pub fn mesh_cracked(cells: &[u16; P * P * P], lattice: usize, cut: Cut, glass: bool, cracks: Option<&[[u8; 5]; P * P * P]>) -> Mesh {
    let cutout = cut != Cut::Solid;
    // cols[a][v*P+u] = bits along axis a; u = (a+1)%3, v = (a+2)%3. opq: the same of the cells that hide what is
    // behind them, and gls of glass, made only for a chunk with glass or (fancy) cutout blocks
    let mut cols = [[0u64; P * P]; 3];
    let mut opq_cols: Vec<[u64; P * P]> = Vec::new();
    let mut gls_cols: Vec<[u64; P * P]> = Vec::new();
    let (see_t, glass_t): (&[u64; 1024], &[u64; 1024]) = (&SEE, &GLASS);
    let (mut any_see, mut glass_seen) = (false, false);
    for z in 0..P {
        for y in 0..P {
            for x in 0..P {
                let c = cells[pidx(x, y, z)];
                if c != 0 {
                    cols[0][z * P + y] |= 1 << x; // a=x: u=y, v=z
                    cols[1][x * P + z] |= 1 << y; // a=y: u=z, v=x
                    cols[2][y * P + x] |= 1 << z; // a=z: u=x, v=y
                    if (cutout || glass) && is_in(see_t, c) {
                        any_see = true;
                        glass_seen |= glass && is_in(glass_t, c);
                    }
                }
            }
        }
    }
    let (mut any_cut, mut any_glass) = (false, false);
    if glass_seen || cutout && any_see {
        // a second pass, only in a chunk with glass or cutout blocks
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
    let root = if any_cut { roots(cells, cut_t) } else { Vec::new() };
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
                    if any_cut && cut == Cut::Shell {
                        // fast leaves: a cutout block's face against a cutout block goes (a crown has no inner
                        // faces), an opaque block's behind one stays, seen through its holes (ADR 0088)
                        let ct = col & !o & !gls_cols[a][(v + 1) * P + (u + 1)];
                        f &= !(ct & if s == 0 { ct >> 1 } else { ct << 1 });
                    }
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
            // the face's crack edges: its cell's crack faces across the plane's two axes, u-, u+, v-, v+
            let (au, av) = ((a + 1) % 3, (a + 2) % 3);
            // and, on a side face, the bedding line across it (bits 24-31, `crack::bed`)
            let edges = |u: usize, v: usize, d: usize| -> u32 {
                let Some(k) = cracks else { return 0 };
                let (u, v, d) = (u + 1, v + 1, d + 1);
                let k = &k[match a {
                    0 => pidx(d, u, v),
                    1 => pidx(v, d, u),
                    _ => pidx(u, v, d),
                }];
                let f = k[0] as u32;
                let bed = if dir < 4 { k[1 + dir] as u32 } else { 0 };
                (f >> (2 * au + 1) & 1) | (f >> (2 * au) & 1) << 1 | (f >> (2 * av + 1) & 1) << 2 | (f >> (2 * av) & 1) << 3 | bed << 4
            };
            let joins_t: &[u64; 1024] = &JOINS;
            // the culling rule above, for one cell: it hides a face behind it (glass and fancy cutout blocks do not)
            let hides = |c: u16| c != 0 && !(any_glass && is_in(glass_t, c)) && !(any_cut && is_in(cut_t, c));
            for d in 0..CS as usize {
                let rows = &mut planes[d];
                for v in 0..CS as usize {
                    while rows[v] != 0 {
                        let u = rows[v].trailing_zeros() as usize;
                        let m = mat(u, v, d);
                        let o = ao(u, v, d);
                        // a joining block's face never merges: each knows its own sides
                        let jm = is_in(joins_t, m);
                        let ck = edges(u, v, d);
                        // a cutout block's face never merges either: each carries its own way to wood and its
                        // own fluff (ADR 0090)
                        let merge = lattice > 1 && o == 0 && !jm && ck == 0 && !(any_cut && is_in(cut_t, m));
                        // a merged quad has one occlusion per corner: only open faces merge
                        let open = |u: usize, v: usize| ao(u, v, d) == 0 && edges(u, v, d) == 0;
                        let mut w = 1;
                        if merge {
                            // never across a lattice line
                            while u + w < 32 && (u + w) % lattice != 0 && rows[v] >> (u + w) & 1 == 1 && mat(u + w, v, d) == m && open(u + w, v) {
                                w += 1;
                            }
                        }
                        let mask = (((1u64 << w) - 1) << u) as u32;
                        let mut h = 1;
                        if merge {
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
                        let mut j = if jm { join_sides(cells, a, s, u, v, d, m, &hides) } else { 0 };
                        if g == 6 {
                            j |= twig_bits(root[match a { 0 => pidx(d + 1, u + 1, v + 1), 1 => pidx(v + 1, d + 1, u + 1), _ => pidx(u + 1, v + 1, d + 1) }], a) << 4;
                            // bit 25: it opens on air (the client's fluff grows there only)
                            j |= ((!occ(u + 1, v + 1, if s == 0 { d + 2 } else { d })) as u32) << 9;
                        }
                        groups[g].push([lo, m as u32 | o << 8 | j << 16 | ck << 20]);
                    }
                }
            }
        }
    }
    // the cutout faces that open on air first: the client's fluff passes draw those alone (ADR 0103)
    groups[6].sort_by_key(|q| q[1] >> 25 & 1 == 0);
    let n_fluff = groups[6].iter().take_while(|q| q[1] >> 25 & 1 == 1).count() as u32;
    let mut counts = [0u32; 6];
    let mut quads = Vec::with_capacity(groups.iter().map(|g| g.len()).sum());
    for (i, g) in groups.iter().enumerate() {
        if i < 6 {
            counts[i] = g.len() as u32;
        }
        quads.extend_from_slice(g);
    }
    let n_cut = groups[6].len() as u32;
    Mesh { quads, counts, cut: n_cut, ray_cut: if cut == Cut::Full { n_cut } else { 0 }, fluff: n_fluff, glass: groups[7].len() as u32, still: 0, water: 0 }
}

/// Each padded cell's way to wood through cutout blocks (ADR 0090): the world direction 0..6 (+x, -x, +y, -y,
/// +z, -z) of the neighbour that leads to it, bits 0-2 (7: none within `TWIG_REACH` steps), and the steps less
/// one, bits 3-4. A cutout block on an opaque block that is not wood roots there, below it (-z), one step: a
/// shrub's twigs grow from the ground. Only in a chunk with cutout blocks, so a short walk over its cells.
const TWIG_REACH: u8 = 3;
fn roots(cells: &[u16; P * P * P], cut_t: &[u64; 1024]) -> Vec<u8> {
    let (wood_t, glass_t): (&[u64; 1024], &[u64; 1024]) = (&WOOD, &GLASS);
    let step = [1isize, -1, P as isize, -(P as isize), (P * P) as isize, -((P * P) as isize)];
    // the neighbour of cell i (at x, y, z) the way k, inside the padded grid
    let next = |i: usize, k: usize| -> Option<usize> {
        let (x, y, z) = (i % P, i / P % P, i / (P * P));
        let c = [x, y, z][k / 2];
        (if k % 2 == 0 { c + 1 < P } else { c > 0 }).then(|| (i as isize + step[k]) as usize)
    };
    let mut root = vec![7u8; P * P * P];
    let mut ring = Vec::new();
    let is_cut = |c: u16| c != 0 && is_in(cut_t, c) && !is_in(glass_t, c);
    // the padding too: a crown across a chunk's edge roots through its neighbour's cells there.
    // shortcut: wood more than the padding away is not seen, so a bare crown's blocks at a chunk edge may lose
    // their twigs; mesh with a halo of TWIG_REACH cells if that shows
    for i in 0..P * P * P {
        if !is_cut(cells[i]) {
            continue;
        }
        let k = (0..6).find(|&k| next(i, k).is_some_and(|j| is_in(wood_t, cells[j]))).or_else(|| {
            let under = cells[next(i, 5)?];
            (under != 0 && !is_cut(under) && !is_in(glass_t, under)).then_some(5)
        });
        if let Some(k) = k {
            root[i] = k as u8;
            ring.push(i);
        }
    }
    // outward a ring at a time; a cell takes the first way it is reached by
    for n in 1..TWIG_REACH {
        let mut out = Vec::new();
        for &i in &ring {
            for k in 0..6 {
                let Some(j) = next(i, k) else { continue };
                if root[j] == 7 && is_cut(cells[j]) {
                    root[j] = (k ^ 1) as u8 | n << 3; // back toward i
                    out.push(j);
                }
            }
        }
        ring = out;
    }
    root
}

/// A cell's way to wood (`roots`) as its face of axis `a` carries it: the side of the face it lies on, u-, u+,
/// v-, v+ (1..4), 5 behind or in front of the face, 0 none (as every quad of a coarse level); the steps less one above, two bits (bits 20-24 of the
/// quad's high word).
fn twig_bits(r: u8, a: usize) -> u32 {
    let k = (r & 7) as usize;
    if k == 7 {
        return 0;
    }
    // wood above (+z, k 4) folds the painting like wood behind: hinged there the twigs would hang down as streamers
    let side = match (k / 2 + 3 - a) % 3 {
        _ if k == 4 => 5,
        0 => 5,
        // an odd world direction is the negative one: the u- or v- side
        1 => 1 + (!k as u32 & 1),
        _ => 3 + (!k as u32 & 1),
    };
    side | ((r >> 3) as u32) << 3
}

/// The sides of joining block `m`'s face at (u, v, d) of axis `a`, side `s` that meet the same block whose face
/// there is drawn too (`hides`: the mesher's culling rule): u-, u+, v-, v+ as bits 0..3 (ADR 0085). Out of the mesher's loop: few blocks join.
#[cold]
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn join_sides(cells: &[u16; P * P * P], a: usize, s: usize, u: usize, v: usize, d: usize, m: u16, hides: &dyn Fn(u16) -> bool) -> u32 {
    // padded coordinates: the face's cell is (u + 1, v + 1, d + 1), the cell in front of it fd
    let fd = if s == 0 { d + 2 } else { d };
    let cell = |u: usize, v: usize, d: usize| match a {
        0 => cells[pidx(d, u, v)],
        1 => cells[pidx(v, d, u)],
        _ => cells[pidx(u, v, d)],
    };
    [(u, v + 1), (u + 2, v + 1), (u + 1, v), (u + 1, v + 2)].iter().enumerate()
        .map(|(i, &(nu, nv))| ((cell(nu, nv, d + 1) == m && !hides(cell(nu, nv, fd))) as u32) << i)
        .sum()
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

    /// ADR 0094: a crack along the top of one cell of a row draws on its two side faces' upper edges (u+ of a y face:
    /// u is z there) and stops them merging; its own top lies on the plane and draws nothing; the rest merge as before.
    #[test]
    fn a_crack_marks_the_edges_of_the_faces_beside_it_and_stops_their_merge() {
        let mut c = [0u16; P * P * P];
        let mut k = [[0u8; 5]; P * P * P];
        for x in 1..6 {
            c[pidx(x, 1, 1)] = 3;
        }
        k[pidx(3, 1, 1)][0] = crate::crack::PL;
        let plain = mesh_cut(&c, 8, Cut::Solid, false);
        let m = mesh_cracked(&c, 8, Cut::Solid, false, Some(&k));
        let cracked: Vec<[u32; 2]> = m.quads.iter().copied().filter(|q| q[1] >> 20 != 0).collect();
        assert_eq!(cracked.len(), 2, "{cracked:?}");
        for q in &cracked {
            assert_eq!((q[1] >> 20, q[0] >> 18 & 1023), (2, 0), "u+ only, one cell");
            assert!([2, 3].contains(&(q[0] >> 28 & 7)), "the y faces");
        }
        assert_eq!(m.quads.len(), plain.quads.len() + 4, "each y face split in three");
        assert_eq!(mesh_cracked(&c, 8, Cut::Solid, false, None).quads, plain.quads);
        // a bedding line on the cell's +j face rides on bits 24-31 of that face alone, and stops its merge too
        let mut k = [[0u8; 5]; P * P * P];
        k[pidx(3, 1, 1)][1 + 2] = 0xa5;
        let m = mesh_cracked(&c, 8, Cut::Solid, false, Some(&k));
        let lined: Vec<[u32; 2]> = m.quads.iter().copied().filter(|q| q[1] >> 20 != 0).collect();
        assert_eq!(lined.len(), 1, "{lined:?}");
        assert_eq!((lined[0][1] >> 24, lined[0][1] >> 20 & 15, lined[0][0] >> 28 & 7, lined[0][0] >> 18 & 1023), (0xa5, 0, 2, 0));
    }

    /// ADR 0090: a cutout face carries where its block's way to wood leads (bits 20-22: 1..4 u-, u+, v-, v+ in the
    /// face's plane, 5 behind the face, 0 none within three blocks) and how far the wood is (bits 23-24, one less
    /// than the steps), so a bare crown's twigs grow from the log; a leaf on the ground roots there.
    #[test]
    fn leaf_faces_know_the_way_to_the_wood() {
        let b = &crate::blocks::BLOCKS;
        let (log, leaves, stone) = (b.id("log"), b.id("broadleaf leaves"), b.id("stone"));
        let mut c = Box::new([0u16; P * P * P]);
        for z in 10..13 {
            c[pidx(10, 10, z)] = log;
        }
        for x in 11..15 {
            c[pidx(x, 10, 11)] = leaves; // a row out from the log along +x: 1, 2, 3 steps, then none
        }
        c[pidx(10, 11, 11)] = leaves; // beside the log on +y
        c[pidx(20, 20, 20)] = leaves; // alone in the air
        c[pidx(10, 10, 9)] = leaves; // under the log
        c[pidx(5, 5, 5)] = stone;
        c[pidx(5, 5, 6)] = leaves; // a shrub on the ground
        c[pidx(0, 28, 11)] = log; // in the padding: the neighbour chunk's log and leaf
        c[pidx(0, 29, 11)] = leaves;
        c[pidx(1, 29, 11)] = leaves;
        for (cut, lattice) in [(Cut::Shell, 1), (Cut::Full, 1), (Cut::Shell, 8), (Cut::Full, 8)] {
            let m = mesh_cut(&c, lattice, cut, false);
            // a cutout face never merges: each is one cell's (level 0 meshes on a lattice of up to 8)
            assert!(m.quads[m.quads.len() - m.cut as usize..].iter().all(|q| (q[0] >> 18) & 1023 == 0), "{cut:?} {lattice}: merged");
            let twig = |at: [usize; 3], dir: u32| {
                let q = m.quads.iter().find(|q| { let (p, d, _) = face_of(q); p.map(|v| v + 1) == at && d == dir }).unwrap_or_else(|| panic!("no face {dir} at {at:?}"));
                ((q[1] >> 20) & 7, (q[1] >> 23) & 3)
            };
            // +y face (a = y: u = z, v = x): the log at -x is v-; +z face (u = x, v = y): u-
            assert_eq!(twig([11, 10, 11], 2), (3, 0), "{cut:?}");
            assert_eq!(twig([11, 10, 11], 4), (1, 0));
            assert_eq!(twig([12, 10, 11], 4), (1, 1), "two steps on, the same way");
            assert_eq!(twig([13, 10, 11], 5), (1, 2));
            assert_eq!(twig([14, 10, 11], 4).0, 0, "four steps: none");
            assert_eq!(twig([14, 10, 11], 0).0, 0);
            assert_eq!(twig([10, 11, 11], 2), (5, 0), "the log behind the face");
            assert_eq!(twig([10, 11, 11], 4), (3, 0), "+z face of the leaf on +y: the log at -y is v-");
            assert_eq!(twig([20, 20, 20], 4).0, 0, "no wood, no ground");
            // wood above: the twigs fold from the face's middle (reach up and out), never hang from the top edge
            assert_eq!(twig([10, 10, 9], 0), (5, 0), "wood above a side face folds");
            assert_eq!(twig([10, 10, 9], 3), (5, 0));
            assert_eq!(twig([5, 5, 6], 4), (5, 0), "a shrub roots in the ground under it");
            assert_eq!(twig([5, 5, 6], 0), (3, 0), "+x face (u = y, v = z): the ground at -z is v-");
            assert_eq!(twig([1, 29, 11], 4), (1, 1), "rooted through the next chunk's leaf, in the padding");
            // bit 25: the face opens on air, where the client grows its fluff (fancy leaves keep inner faces)
            let open = |at: [usize; 3], dir: u32| m.quads.iter().find(|q| { let (p, d, _) = face_of(q); p.map(|v| v + 1) == at && d == dir }).map(|q| q[1] >> 25 & 1);
            assert_eq!(open([11, 10, 11], 2), Some(1));
            assert_eq!(open([11, 10, 11], 0), if cut == Cut::Full { Some(0) } else { None }, "against a leaf");
        }
    }

    /// ADR 0085: a joining block's face carries which of its four neighbours in the face's plane are the same
    /// block with that face open too (bits 16-19: u-, u+, v-, v+), so the shader draws the frame on the outer
    /// edge only; a joining block never merges, and other blocks carry none.
    #[test]
    fn lamps_know_their_neighbours_in_the_wall() {
        let b = &crate::blocks::BLOCKS;
        let (lamp, stone) = (b.id("lamp"), b.id("stone"));
        let mut c = Box::new([0u16; P * P * P]);
        // a row of three lamps along x at y = 5, z = 5, stone beside the last; a stone in front (+z) of the first
        for x in 5..8 {
            c[pidx(x, 5, 5)] = lamp;
        }
        c[pidx(8, 5, 5)] = stone;
        c[pidx(5, 5, 6)] = stone;
        let m = mesh(&c, 8);
        let mask = |cell: [usize; 3], dir: u32| {
            let q = m.quads.iter().find(|q| face_of(q).0 == cell && face_of(q).1 == dir).expect("face");
            q[1] >> 16
        };
        // +y faces (a = y: u = z, v = x): the middle lamp joins both ways along x (v), the first only to +x
        assert_eq!(mask([4, 4, 4], 2), 8);
        assert_eq!(mask([5, 4, 4], 2), 4 | 8);
        assert_eq!(mask([6, 4, 4], 2), 4); // stone beyond is not a lamp
        // +z faces (a = z: u = x, v = y): the first lamp's is covered, so the second does not join to it
        assert_eq!(mask([5, 4, 4], 4), 2);
        // stone carries no mask
        assert!(m.quads.iter().filter(|q| face_of(q).2 == stone).all(|q| q[1] >> 16 == 0));

        // a column of three lamps along y, their -x faces (a = x: u = y, v = z; front at x - 1): a glass pane in
        // front of the middle one leaves its face drawn, so it still joins both ways (as fancy leaves would)
        let glass = b.id("glass");
        let mut c = Box::new([0u16; P * P * P]);
        for y in 10..13 {
            c[pidx(10, y, 10)] = lamp;
        }
        c[pidx(9, 11, 10)] = glass;
        let m = mesh_cut(&c, 8, Cut::Solid, true);
        let mask = |cell: [usize; 3], dir: u32| {
            let q = m.quads.iter().find(|q| face_of(q).0 == cell && face_of(q).1 == dir).expect("face");
            q[1] >> 16
        };
        assert_eq!(mask([9, 9, 9], 1), 2);
        assert_eq!(mask([9, 10, 9], 1), 1 | 2);
        assert_eq!(mask([9, 11, 9], 1), 1);
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
        let fancy = mesh_cut(&c, 1, Cut::Full, true);
        let mut got: Vec<_> = fancy.quads.iter().map(face_of).collect();
        got.sort();
        assert_eq!(got, faces_one_by_one(&c, |_, n| b.opaque(n)), "fancy");
        let n_opq = fancy.quads.len() - fancy.cut as usize;
        assert_eq!(fancy.ray_cut, fancy.cut, "fancy: a ray passes their holes");
        assert!(fancy.cut > 0 && fancy.quads[n_opq..].iter().all(|q| !b.opaque((q[1] & 255) as u16)), "the cutout quads come last");
        assert!(fancy.quads[..n_opq].iter().all(|q| b.opaque((q[1] & 255) as u16)));
        // those that open on air first, `fluff` of them: the client's fluff passes draw only these (ADR 0103)
        let (cuts, n) = (&fancy.quads[n_opq..], fancy.fluff as usize);
        let open = |q: &[u32; 2]| q[1] >> 25 & 1 == 1;
        assert!(n > 0 && n < cuts.len(), "a crown has open and inner faces: {n} of {}", cuts.len());
        assert!(cuts[..n].iter().all(open) && !cuts[n..].iter().any(open));
        let solid = mesh(&c, 1);
        let mut got: Vec<_> = solid.quads.iter().map(face_of).collect();
        got.sort();
        assert_eq!(got, faces_one_by_one(&c, |_, n| n != 0), "a coarse level is the old mesher");
        assert_eq!(solid.cut, 0);
        {
            let fast = mesh_cut(&c, 1, Cut::Shell, true);
            let mut got: Vec<_> = fast.quads.iter().map(face_of).collect();
            got.sort();
            // fast: a cutout block hides its own kind's faces (no inner faces in a crown), never an opaque
            // block's: the ground under a shrub shows through its holes
            assert_eq!(got, faces_one_by_one(&c, |me, n| b.opaque(n) || n != 0 && !b.opaque(me)), "fast");
            // its cutout blocks' quads go last too: the client draws their holes open (ADR 0088)
            let n_opq = fast.quads.len() - fast.cut as usize;
            assert_eq!(fast.ray_cut, 0, "fast: whole blocks to a ray");
            assert!(fast.cut > 0 && fast.quads[n_opq..].iter().all(|q| !b.opaque((q[1] & 255) as u16)), "fast: the cutout quads come last");
            assert!(fast.quads[..n_opq].iter().all(|q| b.opaque((q[1] & 255) as u16)));
        }
        // a chunk with no cutout block meshes the same either way
        let rock: Box<[u16; P * P * P]> = Box::new(c.map(|v| if v == 0 { 0 } else { stone }));
        assert_eq!(mesh_cut(&rock, 8, Cut::Full, true).quads, mesh(&rock, 8).quads);
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
            let m = mesh_cut(&c, 1, if fancy { Cut::Full } else { Cut::Shell }, true);
            let mut got: Vec<_> = m.quads.iter().map(face_of).collect();
            got.sort();
            // what hides a face: an opaque block, the same glass, or (fast) a cutout block a cutout block's
            let cut = |c: u16| c != 0 && !b.opaque(c) && !b.glass(c);
            let hides = |me: u16, n: u16| n == me && b.glass(me) || b.opaque(n) || !fancy && cut(me) && cut(n);
            assert_eq!(got, faces_one_by_one(&c, hides), "fancy {fancy}");
            let g0 = m.quads.len() - m.glass as usize;
            assert!(m.glass > 0 && m.quads[g0..].iter().all(|q| b.glass((q[1] & 255) as u16)), "the glass quads come last");
            assert!(m.quads[..g0].iter().all(|q| !b.glass((q[1] & 255) as u16)));
            assert!(m.cut > 0, "the cutout quads are grouped fast or fancy");
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
