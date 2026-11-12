//! Caves by rock (W4, ADR 0063; `docs/design/worldgen.md`, "Caves follow rock"): water-cut systems that
//! wander along the water table through limestone and come out where a valley cuts down to it, and lava tubes
//! a few metres under rift basalt. Made from the map and the seed when a planet loads, on the server and every
//! client alike, as balls of air (`world::Cave`) that the readers ask through `Body::col_caves`.
use crate::rock::{Caves, ROCKS};
use crate::world::{Body, Cave, SEA_DEPTH};
use glam::DVec3;

/// A candidate start every this many columns, in each direction: some three limestone systems a square
/// kilometre of basin.
const SPACING: i32 = 384;
/// Caves keep this far from a face's seam (a ball is cut there, `col_caves`) and from the spawn, columns.
const SEAM: f64 = 12.0;
const SPAWN: f64 = 40.0;
/// A ball whose ground is lower than this over the sea is not made: no cave opens under the sea (the
/// generated sea starts at the generated floor, `Solid::generated`).
const DRY: f64 = 3.0;

struct Rng(u64);
impl Rng {
    fn f(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Every cave of a baked planet, in a fixed order.
pub fn make(b: &Body, seed: u64) -> Vec<Cave> {
    let Some(map) = b.map() else { return Vec::new() };
    if map.rock.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let (sf, si, sj) = b.locate(b.spawn);
    let n = b.n;
    for face in 0..6 {
        for cy in 0..n / SPACING {
            for cx in 0..n / SPACING {
                let mut rng = Rng(seed ^ 0xca5e ^ (face as u64) << 52 ^ (cx as u64) << 26 ^ cy as u64);
                let (i, j) = ((cx * SPACING) as f64 + rng.f() * SPACING as f64, (cy * SPACING) as f64 + rng.f() * SPACING as f64);
                if face == sf && (i - si).hypot(j - sj) < SPAWN + 60.0 {
                    continue;
                }
                let d = b.dir(face, i, j);
                let rock = b.rock_at(face, i, j, d);
                let Some(p) = ROCKS.provinces.get(rock.province as usize) else { continue };
                let worm = match p.caves {
                    Caves::Limestone if rng.f() < 0.5 => Worm::Limestone,
                    Caves::Tubes if rng.f() < 0.7 => Worm::Tube,
                    _ => continue,
                };
                let a = rng.f() * std::f64::consts::TAU;
                let len = match worm {
                    Worm::Limestone => 50 + (rng.f() * 110.0) as usize,
                    Worm::Tube => 30 + (rng.f() * 80.0) as usize,
                };
                walk(b, &mut rng, worm, face, i, j, a, len, 2, &mut out);
            }
        }
    }
    out
}

#[derive(Clone, Copy, PartialEq)]
enum Worm {
    Limestone,
    Tube,
}

/// The water table under face point (i, j), metres over the sea: the ground's height over the sea, smoothed
/// over 40 m and subdued to six tenths of it, and at least 3 m under the ground there.
pub fn water_table(b: &Body, face: usize, i: f64, j: f64) -> f64 {
    let over = |i: f64, j: f64| b.ground_r(face, i, j) - (b.radius - SEA_DEPTH);
    let smooth = [(0.0, 0.0), (40.0, 0.0), (-40.0, 0.0), (0.0, 40.0), (0.0, -40.0)].iter().map(|(a, c)| over(i + a, j + c)).sum::<f64>() / 5.0;
    (0.6 * smooth.max(0.0)).min(over(i, j) - 3.0)
}

/// One cave's balls from (i, j): a limestone system holds to the water table and ends where the limestone
/// does, a tube runs a few metres under the ground downhill; either ends near a seam, the spawn or the sea.
#[allow(clippy::too_many_arguments)]
fn walk(b: &Body, rng: &mut Rng, worm: Worm, face: usize, mut i: f64, mut j: f64, mut a: f64, len: usize, branches: u32, out: &mut Vec<Cave>) {
    let (n, sea) = (b.n as f64, b.radius - SEA_DEPTH);
    let (sf, si, sj) = b.locate(b.spawn);
    let tube_depth = 3.0 + rng.f() * 3.0;
    let mut z = None::<f64>;
    for _ in 0..len {
        let r = match worm {
            Worm::Limestone if rng.f() < 0.06 => 3.5 + rng.f() * 2.0,
            Worm::Limestone => 1.6 + rng.f() * 1.8,
            Worm::Tube => 1.8 + rng.f() * 1.0,
        };
        if i < SEAM + r || j < SEAM + r || i > n - SEAM - r || j > n - SEAM - r || (face == sf && (i - si).hypot(j - sj) < SPAWN) {
            return;
        }
        let ground = b.ground_r(face, i, j);
        if ground - sea < DRY + r {
            return;
        }
        let want = match worm {
            Worm::Limestone => b.layer_of(sea + water_table(b, face, i, j)),
            // a skylight now and then: the tube's roof breaks the ground
            Worm::Tube if rng.f() < 0.03 => b.layer_of(ground - r * 0.5),
            Worm::Tube => b.layer_of(ground - tube_depth - r),
        };
        let zz = z.map_or(want, |z| z + (want - z).clamp(-0.5, 0.5));
        z = Some(zz);
        // still in the rock this cave is cut in
        let rock = b.rock_at(face, i, j, b.dir(face, i, j));
        let here = ROCKS.at(rock, b.layer_r(zz) as f32, 0);
        let fits = match worm {
            Worm::Limestone => here == crate::blocks::BLOCKS.id("limestone"),
            Worm::Tube => here == crate::blocks::BLOCKS.id("basalt"),
        };
        if !fits || zz - r < 1.0 {
            return;
        }
        out.push(Cave { face: face as u8, at: DVec3::new(i, j, zz), r });
        if branches > 0 && rng.f() < 0.025 {
            let side = if rng.f() < 0.5 { 1.0 } else { -1.0 };
            let (turn, len) = (side * (0.8 + rng.f() * 0.8), 20 + (rng.f() * 40.0) as usize);
            walk(b, rng, worm, face, i, j, a + turn, len, branches - 1, out);
        }
        a += match worm {
            Worm::Limestone => (rng.f() - 0.5) * 0.7,
            // downhill, as lava ran
            Worm::Tube => {
                let slope = |da: f64| b.ground_r(face, i + 6.0 * (a + da).cos(), j + 6.0 * (a + da).sin());
                if slope(0.4) < slope(-0.4) { 0.15 } else { -0.15 }
            }
        };
        let step = 0.8 * r;
        (i, j) = (i + a.cos() * step, j + a.sin() * step);
    }
}
