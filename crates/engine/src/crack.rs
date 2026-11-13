//! Cracks in the rock (mining.md 2, ADR 0094): a rule over (rock, place), computed like the rock itself, the same
//! on the server and on every client. Layered rock (`bedding` in `data/blocks.toml`) breaks along planes under the
//! column's folded top, every `bedding` metres; massive rock (`joints`) along vertical planes between columns, about
//! every `joints` columns, placed by a hash. A survival dig of a block on a crack loosens its neighbours along that
//! crack into rubble (`drop`).
use crate::blocks::BLOCKS;
use crate::world::{fold, Block, Body, World};
use glam::IVec3;

/// The six faces of a cell as bits, in the mesher's order: +i, -i, +j, -j, +l, -l.
pub const PI: u8 = 1;
pub const MI: u8 = 2;
pub const PJ: u8 = 4;
pub const MJ: u8 = 8;
pub const PL: u8 = 16;
pub const ML: u8 = 32;

/// The faces that make a dig a good hit: a plane through the cell (a bedding plane across its span, a joint on its +i
/// or +j side), one side of each plane only, so a dig on a crack is about one in `bedding` metres or two in `joints`
/// (the review: with both sides nearly every dig in shale and basalt was a good hit).
pub const HIT: u8 = PL | PI | PJ;

/// Blocks a good hit drops at most, the dug one with them (mining.md: 3 to 6).
pub const DROP_MAX: usize = 6;
/// Cells a drop's search looks at at most (spike 3: each is a block and a crack lookup of the generated ground).
pub const SEARCH: usize = 32;

fn mix(mut h: u64) -> u64 {
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51afd7ed558ccd);
    h ^= h >> 33;
    h = h.wrapping_mul(0xc4ceb9fe1a85ec53);
    h ^ (h >> 33)
}

/// The columns and metres a joint plane's stretch runs along and up.
const SEG: i32 = 16;
const SEG_M: f32 = 8.0;
const JOG_M: f32 = 3.0;

/// Whether a joint plane runs between columns `k` and `k + 1` along an axis: one in every `n` columns, at a hashed
/// place in each run of `n`, so joints are 1 to 2n - 1 apart and any face 2n columns wide shows one (the overseer: a
/// granite tunnel must always have a joint to read).
fn joint(seed: u64, face: u8, axis: u64, k: i32, n: u16) -> bool {
    let n = n as i32;
    (mix(seed ^ (face as u64) << 40 ^ axis << 36 ^ (k.div_euclid(n) as u32 as u64)) % n as u64) as i32 == k.rem_euclid(n)
}

/// The faces of a cell of `block` that lie on a crack. `top`: the radius of the column's first rock layer (folded,
/// `Rock::top`); `lo`, `hi`: the radii of the cell's bottom and top, `below`: of the bottom of the cell under it.
#[allow(clippy::too_many_arguments)]
pub fn faces(block: u16, seed: u64, face: u8, i: i32, j: i32, top: f32, below: f32, lo: f32, hi: f32) -> u8 {
    let Some(d) = BLOCKS.get(block) else { return 0 };
    let mut f = 0;
    if d.bedding > 0.0 {
        // the plane `top - n * bedding` that a cell spans runs along its top face
        let k = |r: f32| ((top - r) / d.bedding).floor();
        f |= if k(hi) != k(lo) { PL } else { 0 } | if k(lo) != k(below) { ML } else { 0 };
    }
    if d.joints > 0 {
        // a joint runs `SEG` columns along and `SEG_M` metres up, then the next stretch has its own: a few joints
        // that end, not a grid across the whole face (the review)
        let n = d.joints;
        let up = (lo / SEG_M).floor() as i32 as u32 as u64;
        let (si, sj) = (seed ^ mix(up << 32 ^ j.div_euclid(SEG) as u32 as u64), seed ^ mix(1 << 63 ^ up << 32 ^ i.div_euclid(SEG) as u32 as u64));
        // every `JOG_M` metres up a joint steps a column or not, so it climbs crooked
        let band = (lo / JOG_M).floor() as i32 as u32 as u64;
        let (ji, jj) = ((mix(si ^ band) & 1) as i32, (mix(sj ^ band) & 1) as i32);
        f |= if joint(si, face, 0, i - ji, n) { PI } else { 0 } | if joint(si, face, 0, i - 1 - ji, n) { MI } else { 0 };
        f |= if joint(sj, face, 1, j - jj, n) { PJ } else { 0 } | if joint(sj, face, 1, j - 1 - jj, n) { MJ } else { 0 };
    }
    f
}

/// The bedding line across a side face of a cell of `block`: `ta`, `tb` the folded top (`Rock::top`) at the face's two
/// vertical edges (its low and high end along the face), `lo`, `hi` the cell's bottom and top radii. 0 when no plane
/// crosses the face; else `(h0 + 8) | (dh + 4) << 5`: the texel row (of 16, from the bottom) the plane lies in at the low
/// end, and the rows it climbs to the high end. The plane is the one nearest the face's middle, so the faces beside it
/// along a wall, which share its edges' tops, carry on the same line through the fold (the shader draws it).
pub fn bed(block: u16, ta: f32, tb: f32, lo: f32, hi: f32) -> u8 {
    let Some(s) = BLOCKS.get(block).map(|d| d.bedding).filter(|&s| s > 0.0) else { return 0 };
    let n = ((0.5 * (ta + tb) - 0.5 * (lo + hi)) / s).round();
    let row = |t: f32| ((t - n * s - lo) / (hi - lo) * 16.0).floor();
    let (a, b) = (row(ta), row(tb));
    if (a < 0.0 && b < 0.0) || (a > 15.0 && b > 15.0) {
        return 0;
    }
    // shortcut: a fold steeper than 4 rows (1/4 m) a metre is cut to that and its line breaks at the face edge
    let h0 = a.clamp(-7.0, 23.0);
    let dh = (b - h0).clamp(-4.0, 3.0);
    (h0 as i32 + 8) as u8 | ((dh as i32 + 4) as u8) << 5
}

/// `faces` of cell (i, j, l) of a body holding `block` there.
pub fn at(body: &Body, block: u16, face: u8, i: i32, j: i32, l: i32) -> u8 {
    let Some(d) = BLOCKS.get(block) else { return 0 };
    if d.bedding == 0.0 && d.joints == 0 {
        return 0;
    }
    let r = |l: i32| body.layer_r(l as f64) as f32;
    faces(block, salt(body), face, i, j, body.col_rock(face, i, j).top, r(l - 1), r(l), r(l + 1))
}

/// `faces` of block `b` of the world, whatever holds it (a placed rock cracks where the rule says, as generated).
pub fn of(world: &World, b: Block) -> u8 {
    at(&world.bodies[b.body as usize], world.block(b), b.face, b.pos.x, b.pos.y, b.pos.z)
}

/// What the hashed joints mix in for a body. shortcut: its size and centre, not the world's seed (`Body` keeps none):
/// every world of a planet size has the same joints; add the seed to `Body` when anyone notices
pub(crate) fn salt(body: &Body) -> u64 {
    body.n as u64 ^ body.centre.x.to_bits() ^ body.centre.z.to_bits().rotate_left(17)
}

/// The step from a cell through face bit `f`.
fn step(f: u8) -> IVec3 {
    [IVec3::X, -IVec3::X, IVec3::Y, -IVec3::Y, IVec3::Z, -IVec3::Z][f.trailing_zeros() as usize]
}

/// What a survival dig of `b` (holding `block`, not yet changed) loosens with it: none when `b` is on no crack (a bad
/// hit: the one block); else up to 2..5 more of the same rock, the cells that share `b`'s first crack plane, found
/// breadth first in a fixed order within `SEARCH` cells (a good hit drops 3 to 6 in all, by a hash of the cell).
pub fn drop(world: &World, b: Block, block: u16) -> Vec<Block> {
    let body = &world.bodies[b.body as usize];
    let here = at(body, block, b.face, b.pos.x, b.pos.y, b.pos.z) & HIT;
    if here == 0 || BLOCKS.get(block).is_none_or(|d| d.rubble.is_none()) {
        return Vec::new();
    }
    let plane = 1 << here.trailing_zeros();
    let want = 2 + (mix(salt(body) ^ (b.face as u64) << 56 ^ (b.pos.x as u32 as u64) << 28 ^ b.pos.y as u32 as u64 ^ (b.pos.z as u64) << 44) % 4) as usize;
    // the crack's own plane: its neighbours lie across the other two axes
    let along: Vec<IVec3> = [PI, PJ, PL].into_iter().filter(|&a| a != plane && a << 1 != plane).flat_map(|a| [step(a), -step(a)]).collect();
    let (mut out, mut seen, mut queue) = (Vec::new(), vec![b], std::collections::VecDeque::from([b]));
    while let Some(c) = queue.pop_front() {
        for &s in &along {
            if out.len() >= want || seen.len() >= SEARCH {
                return out;
            }
            let (f, i, j) = fold(c.face as usize, c.pos.x + s.x, c.pos.y + s.y, body.n);
            let n = Block { body: c.body, face: f as u8, pos: IVec3::new(i, j, c.pos.z + s.z) };
            if seen.contains(&n) {
                continue;
            }
            seen.push(n);
            if world.block(n) == block && of(world, n) & plane != 0 {
                out.push(n);
                queue.push_back(n);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::AIR;
    use glam::DVec3;

    #[test]
    fn bedding_runs_under_the_folded_top_and_joints_between_columns() {
        let (lime, granite, stone) = (BLOCKS.id("limestone"), BLOCKS.id("granite"), BLOCKS.id("stone"));
        let s = BLOCKS.get(lime).unwrap().bedding;
        assert!(s > 2.0, "limestone is bedded, over 2 m apart");
        // a plane at top - 2 s: the cell from top - 2 s - 0.4 to + 0.6 holds it on its top face, the one over it on its foot
        let top = 100.0;
        let p = top - 2.0 * s;
        assert_eq!(faces(lime, 1, 0, 0, 0, top, p - 1.4, p - 0.4, p + 0.6), PL);
        assert_eq!(faces(lime, 1, 0, 0, 0, top, p - 0.4, p + 0.6, p + 1.6), ML);
        // the plane follows the top: a column whose top is 0.5 m lower has it 0.5 m lower
        assert_eq!(faces(lime, 1, 0, 0, 0, top - 0.5, p - 1.9, p - 0.9, p + 0.1), PL);
        // joints: between i and i + 1 on one column is between them on its neighbour too, about one in `joints`
        let n = BLOCKS.get(granite).unwrap().joints;
        assert!(n >= 3);
        let cuts = (0..1000).filter(|&i| faces(granite, 7, 2, i, 5, 0.0, 0.0, 1.0, 2.0) & PI != 0).count();
        assert!((1000 / n as usize / 2..=1000 * 2 / n as usize).contains(&cuts), "{cuts} joints in 1000 columns, one in {n}");
        for i in 0..200 {
            let f = faces(granite, 7, 2, i, 5, 0.0, 0.0, 1.0, 2.0);
            assert_eq!(f & PI != 0, faces(granite, 7, 2, i + 1, 5, 0.0, 0.0, 1.0, 2.0) & MI != 0, "both sides of the plane");
            assert_eq!(f & PI, faces(granite, 7, 2, i, 15, 0.0, 0.0, 1.0, 2.0) & PI, "a plane along its stretch of j");
            assert_eq!(f & PI, faces(granite, 7, 2, i, 5, 0.0, 0.0, 2.0, 3.0) & PI, "and up its band");
        }
        // any 2n columns hold a joint: a face of 8 blocks in granite (n = 4) always shows one
        for i in 0..200 {
            assert!((i..i + 2 * n as i32).any(|i| faces(granite, 7, 2, i, 5, 0.0, 0.0, 1.0, 2.0) & PI != 0), "a joint in {i}..+{}", 2 * n);
        }
        let other = (0..200).filter(|&i| faces(granite, 7, 2, i, 5, 0.0, 0.0, 1.0, 2.0) & PI != faces(granite, 7, 2, i, 99, 0.0, 0.0, 1.0, 2.0) & PI).count();
        assert!(other > 10, "another stretch has its own joints: {other} of 200 differ");
        assert_eq!(faces(stone, 7, 2, 0, 0, 0.0, 0.0, 1.0, 2.0), 0, "plain stone has no cracks");
        assert_eq!(faces(BLOCKS.id("granite rubble"), 7, 2, 0, 0, 0.0, 0.0, 1.0, 2.0), 0, "nor rubble");
    }

    /// The overseer's look (ADR 0094): a bedding line runs on through the faces beside it and climbs with the fold.
    #[test]
    fn a_bedding_line_runs_on_across_faces_and_follows_the_fold() {
        let lime = BLOCKS.id("limestone");
        let s = BLOCKS.get(lime).unwrap().bedding;
        let line = |b: u8| (b as i32 & 31) - 8;
        let climb = |b: u8| (b as i32 >> 5) - 4;
        // level layers: a plane 0.25 m over the cell's foot is row 4 at both ends
        let top = 10.0 * s + 0.25;
        let b = bed(lime, top, top, 0.0, 1.0);
        assert_eq!((line(b), climb(b)), (4, 0), "{b}");
        // a fold rising 1/8 m a metre: the line climbs 2 rows across a face, and the next face starts where it ended
        let (t0, t1, t2) = (top, top + 0.125, top + 0.25);
        let (b0, b1) = (bed(lime, t0, t1, 0.0, 1.0), bed(lime, t1, t2, 0.0, 1.0));
        assert_eq!((line(b0), climb(b0)), (4, 2));
        assert_eq!(line(b1), line(b0) + climb(b0), "one line, not bricks");
        // no plane in the cell: nothing; and rock with no bedding draws none
        assert_eq!(bed(lime, top + s / 2.0, top + s / 2.0, 0.0, 1.0), 0);
        assert_eq!(bed(BLOCKS.id("granite"), top, top, 0.0, 1.0), 0);
    }

    /// Spike 3 of mining.md (ADR 0094): the drop's search over generated rock on the 4 km planet, where every
    /// lookup is the map's (cargo test -p engine --profile play --lib drop_cost -- --ignored --nocapture).
    #[test]
    #[ignore]
    fn drop_cost() {
        let w = World::baked(6144, crate::bodies::columns(1, 6144), 1337);
        let body = &w.bodies[0];
        let mut on = Vec::new();
        'find: for face in 0..6u8 {
            for i in (0..body.n).step_by(97) {
                for j in (0..body.n).step_by(89) {
                    let top = body.layer_of(body.col_height(face, i, j)) as i32;
                    for z in (top - 60..top - 6).step_by(3) {
                        let b = Block { body: 0, face, pos: IVec3::new(i, j, z) };
                        let m = w.block(b);
                        if of(&w, b) & HIT != 0 {
                            on.push((b, m));
                            if on.len() == 3000 {
                                break 'find;
                            }
                        }
                    }
                }
            }
        }
        let t = std::time::Instant::now();
        let (mut worst, mut dropped) = (0f64, 0usize);
        for &(b, m) in &on {
            let s = std::time::Instant::now();
            dropped += drop(&w, b, m).len();
            worst = worst.max(s.elapsed().as_secs_f64());
        }
        let mean = t.elapsed().as_secs_f64() / on.len() as f64;
        let s = std::time::Instant::now();
        for &(b, _) in &on {
            std::hint::black_box(of(&w, b));
        }
        let of_us = s.elapsed().as_secs_f64() * 1e6 / on.len() as f64;
        println!("{{\"drop_cost\":{{\"digs\":{},\"dropped_mean\":{:.2},\"mean_us\":{:.1},\"worst_us\":{:.1},\"crack_lookup_us\":{of_us:.2},\"search_cap\":{SEARCH}}}}}", on.len(), dropped as f64 / on.len() as f64, mean * 1e6, worst * 1e6);
        assert!(on.len() > 100);
    }

    /// A wall of granite: a dig on a joint takes 2..5 more of the granite on that joint, never another block, never
    /// off the plane; a dig off every crack takes none.
    #[test]
    fn a_dig_on_a_crack_drops_its_neighbours_along_it() {
        let mut w = World::new(512, 32, 5);
        let g = w.air_over_ground(0, DVec3::new(0.3, 0.5, 0.8).normalize());
        let granite = BLOCKS.id("granite");
        let base = Block { pos: g.pos + IVec3::Z * 4, ..g };
        for di in -8..=8 {
            for dj in -8..=8 {
                for dl in 0..4 {
                    w.set_block(Block { pos: base.pos + IVec3::new(di, dj, dl), ..base }, granite);
                }
            }
        }
        let cell = |di: i32, dj: i32, dl: i32| Block { pos: base.pos + IVec3::new(di, dj, dl), ..base };
        let (mut good, mut bad) = (0, 0);
        for di in -4..=4 {
            for dj in -4..=4 {
                let b = cell(di, dj, 1);
                let here = of(&w, b) & HIT;
                let got = drop(&w, b, granite);
                if here == 0 {
                    assert!(got.is_empty());
                    bad += 1;
                    continue;
                }
                good += 1;
                assert!((2..=5).contains(&got.len()), "{} more", got.len());
                let plane = 1 << here.trailing_zeros();
                for c in &got {
                    assert!(w.block(*c) == granite && of(&w, *c) & plane != 0 && *c != b, "on the plane");
                    let off = c.pos - b.pos;
                    let axis = [PI, PJ, PL].iter().position(|&a| a == plane || a << 1 == plane).unwrap();
                    assert_eq!(off[axis], 0, "the same side of the plane");
                }
            }
        }
        assert!(good > 0 && bad > 0, "{good} on a crack, {bad} off");
        // another rock in the way stops it
        let b = (-4..=4).flat_map(|di| (-4..=4).map(move |dj| (di, dj))).map(|(di, dj)| cell(di, dj, 1)).find(|b| of(&w, *b) & HIT != 0).unwrap();
        for c in drop(&w, b, granite) {
            w.set_block(c, AIR);
        }
        assert!(drop(&w, b, granite).iter().all(|c| w.block(*c) == granite));
    }
}
