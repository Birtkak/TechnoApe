//! Volcanoes on Terra's arcs (docs/design/volcanoes.md, step 1; draft decision 2026-11-23): a few cones a planet
//! on the volcanic arc's land, raised on the heights with a summit crater, after the erosion and before the
//! climate (so the air and the biomes see them). A map of `super::VERSION` 9 on keeps them; the world puts the
//! chamber, the conduit and the lava lake under each live one (`world::volcano`).
use super::land::{Bake, Rng};
use super::rock::ARC;
use serde::{Deserialize, Serialize};

/// A volcano as the map keeps it, metres (heights over the sea).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Volcano {
    /// The map cell under its summit.
    pub cell: u32,
    /// The cone's foot's radius.
    pub radius: f32,
    /// The crater's rim over the sea.
    pub summit: f32,
    /// The crater's radius and how deep its floor lies under the rim.
    pub crater: f32,
    pub depth: f32,
    /// The chamber's middle over the sea (under it: negative) and its radius; the conduit's radius.
    pub chamber: f32,
    pub chamber_r: f32,
    pub conduit: f32,
    /// A live chamber (held hot from below) or a dead, cold one.
    pub alive: bool,
}

impl Volcano {
    /// Whether its numbers are those of a cone a world can hold (bytes from a server).
    pub fn sane(&self, cells: usize) -> bool {
        let ok = |v: f32, lo: f32, hi: f32| v.is_finite() && (lo..=hi).contains(&v);
        (self.cell as usize) < cells && ok(self.radius, 8.0, 400.0) && ok(self.summit, -100.0, 500.0) && ok(self.crater, 1.0, self.radius) && ok(self.depth, 1.0, 300.0) && ok(self.chamber, -400.0, self.summit) && ok(self.chamber_r, 1.0, 100.0) && ok(self.conduit, 0.5, 20.0)
    }
}

/// Volcanoes on the 4 km planet; another planet gets its share by area (one at least where an arc can hold one).
/// TASTE CALL (default, 2026-11-23): six, two in three of them alive.
const PER_4KM: f64 = 6.0;

/// The cone's height over its foot at `d` metres from its middle: the crater's bowl inside `crater`, its flank
/// falling concave (as a stratovolcano's) to 0 at the foot.
fn rise(d: f32, v: &Volcano, height: f32) -> f32 {
    if d < v.crater {
        let t = d / v.crater;
        return height - v.depth * (1.0 - t * t);
    }
    let t = ((v.radius - d) / (v.radius - v.crater)).clamp(0.0, 1.0);
    // t^1.5 by a square root, the same bits on every platform (no powf)
    height * (t * t * t).sqrt()
}

/// Places the volcanoes of a bake of `n` columns per face edge and raises their cones in its heights. On the arc's
/// land (the lower flank may reach into the sea), no lake or river cell under the foot (two cells to spare), clear
/// of the face's seams (the world's caves are cut there), spaced eight foot radii apart; which arc cells first is a
/// hash of the seed alone, nothing is placed by the spawn. Exaggerated as the other landforms are (ADR 0105): the
/// foot 4.5 % of the planet's radius (191 m on the 4 km planet; 64 m at least, on a planet of 430 m radius or
/// more), the cone about two thirds as high.
pub fn place(b: &mut Bake, n: u32, seed: u64) -> Vec<Volcano> {
    let (g, cell_m) = (&b.g, b.g.r as f32 * std::f32::consts::FRAC_PI_2 / b.g.m as f32);
    let radius = (0.045 * g.r as f32).clamp(64.0, 200.0);
    if radius > 0.15 * g.r as f32 {
        return Vec::new();
    }
    let height = 0.65 * radius + 10.0;
    // (a crater three cells wide at least, and deep: the map's spline smooths a narrower bowl away)
    let depth = (0.3 * height).max(18.0);
    let (crater, chamber_r, conduit) = ((0.22 * radius).max(3.0 * cell_m), (0.18 * radius).max(8.0), (0.015 * radius).max(2.0));
    // the cell reach of the foot and the rivers' margin, and how many
    let k = (radius / cell_m).ceil() as i64 + 2;
    let want = (PER_4KM * (n as f64 / 6144.0) * (n as f64 / 6144.0)).round().max(1.0) as usize;
    let m = g.m as i64;
    let rivers: std::collections::HashSet<u32> = b.rivers.iter().map(|r| r.cell).collect();
    let foot = |c: u32| {
        let (f, i, j) = g.fij(c);
        let (i, j) = (i as i64, j as i64);
        (-k..=k).flat_map(move |a| (-k..=k).map(move |e| (a, e))).map(move |(a, e)| g.idx(f, (i + a) as usize, (j + e) as usize))
    };
    let mut cands: Vec<(u64, u32)> = (0..g.n() as u32)
        .filter(|&c| {
            let (_, i, j) = g.fij(c);
            let inside = (k..m - k).contains(&(i as i64)) && (k..m - k).contains(&(j as i64));
            inside && b.rock[c as usize] == ARC && b.land(c as usize) && !b.lake(c as usize) && b.h[c as usize] > 5.0
        })
        .map(|c| (Rng::new(seed ^ 0x766f_6c63_616e ^ (c as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)).u64(), c))
        .collect();
    cands.sort_unstable();
    let top = (crate::world::AMP - crate::world::SEA_DEPTH - 2.0 - crate::biomes::TALLEST as f64) as f32 - 10.0;
    let mut out: Vec<Volcano> = Vec::new();
    for (_, c) in cands {
        if out.len() == want {
            break;
        }
        if out.iter().any(|v| g.dist(v.cell, c) < 8.0 * radius) {
            continue;
        }
        let base = b.h[c as usize];
        // an arc is a coast's belt: the cone's lower flank may stand in the sea (an island arc's), its upper part
        // and crater on land; no lake or river under any of it
        let clear = foot(c).all(|o| !b.lake(o as usize) && !rivers.contains(&o) && (b.land(o as usize) || g.dist(c, o) > 0.85 * radius));
        if !clear || base + height > top {
            continue;
        }
        // the deep lies under the foot: the chamber's top 0.7 foot radii under it
        let chamber = base - 0.7 * radius - chamber_r;
        let v = Volcano { cell: c, radius, summit: base + height, crater, depth, chamber, chamber_r, conduit, alive: out.len() % 3 != 2 };
        // the ground under the upper cone levelled to the summit's foot, then the cone on it
        for o in foot(c) {
            let d = g.dist(c, o);
            if d >= radius {
                continue;
            }
            let s = ((radius - d) / (0.5 * radius)).clamp(0.0, 1.0);
            let level = s * s * (3.0 - 2.0 * s);
            let o = o as usize;
            let mut h = b.h[o] + (base - b.h[o]) * level + rise(d, &v, height);
            // (a sea cell stays sea: the flank ends at the shore)
            if b.ocean[o] {
                h = h.min(-1.0);
            }
            let dh = h - b.h[o];
            b.h[o] = h;
            b.w[o] += dh;
            b.lvl[o] += dh;
        }
        out.push(v);
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::{rock, shared, Map};

    /// The first seed whose small planet (768 columns) holds a live volcano: what the world's tests stand on.
    pub(crate) fn live_seed() -> (i32, std::sync::Arc<Map>) {
        (1..=12).map(|s| (s, shared(s, 768))).find(|(_, m)| m.volcanoes.iter().any(|v| v.alive)).expect("a live volcano on a small planet in 12 seeds")
    }

    /// Volcanoes are the seed's (a second bake places the same, to the bit), each on the arc's land with its cone
    /// raised over the foot and a crater under the rim, two in three alive; the bake before them (version 8) has
    /// none.
    #[test]
    fn volcanoes_are_the_seeds_and_stand_on_the_arcs_land() {
        let (seed, m) = live_seed();
        let again = Map::bake(seed, 768);
        assert_eq!((&again.volcanoes, again.hash), (&m.volcanoes, m.hash), "the same volcanoes twice");
        for v in &m.volcanoes {
            let c = v.cell as usize;
            assert_eq!(m.rock[c], rock::ARC, "on the arc: {v:?}");
            assert!(m.h[c] > 0.0 && !m.lakes.iter().any(|l| l.0 == v.cell), "on dry land: {v:?}");
            assert!(m.h[c] < v.summit - v.depth * 0.5, "a crater under the rim: {} m, rim {} m", m.h[c], v.summit);
            assert!(v.chamber + v.chamber_r < m.h[c] - 20.0, "the chamber deep under the crater");
        }
        let old = Map::bake_v8(seed, 768);
        assert!(old.volcanoes.is_empty() && old.version == 8);
        assert_eq!(Map::from_bytes(&m.to_bytes()).unwrap().volcanoes, m.volcanoes, "a map keeps its volcanoes");
    }
}
