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

/// Whether a joint plane runs between columns `k` and `k + 1` along an axis: about one in `n`.
fn joint(seed: u64, face: u8, axis: u64, k: i32, n: u16) -> bool {
    mix(seed ^ (face as u64) << 40 ^ axis << 36 ^ (k as u32 as u64)) % n as u64 == 0
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
        let n = d.joints;
        f |= if joint(seed, face, 0, i, n) { PI } else { 0 } | if joint(seed, face, 0, i - 1, n) { MI } else { 0 };
        f |= if joint(seed, face, 1, j, n) { PJ } else { 0 } | if joint(seed, face, 1, j - 1, n) { MJ } else { 0 };
    }
    f
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
fn salt(body: &Body) -> u64 {
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
    let here = at(body, block, b.face, b.pos.x, b.pos.y, b.pos.z);
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
            assert_eq!(f & PI, faces(granite, 7, 2, i, 99, 0.0, 0.0, 1.0, 2.0) & PI, "a plane, the same on every j");
        }
        assert_eq!(faces(stone, 7, 2, 0, 0, 0.0, 0.0, 1.0, 2.0), 0, "plain stone has no cracks");
        assert_eq!(faces(BLOCKS.id("granite rubble"), 7, 2, 0, 0, 0.0, 0.0, 1.0, 2.0), 0, "nor rubble");
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
                        if of(&w, b) != 0 {
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
                let here = of(&w, b);
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
        let b = (-4..=4).flat_map(|di| (-4..=4).map(move |dj| (di, dj))).map(|(di, dj)| cell(di, dj, 1)).find(|b| of(&w, *b) != 0).unwrap();
        for c in drop(&w, b, granite) {
            w.set_block(c, AIR);
        }
        assert!(drop(&w, b, granite).iter().all(|c| w.block(*c) == granite));
    }
}
