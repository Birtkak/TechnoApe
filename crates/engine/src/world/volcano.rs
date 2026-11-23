//! Terra's volcanoes in the world (docs/design/volcanoes.md steps 1 and 2; draft decision 2026-11-23): the bake's
//! cones (`bake::volcano`) put in the block grid, the heat round a live one's chamber and conduit, and the lava
//! generation keeps there. One temperature field: the far rock's (`Body::heat` as before) plus
//! `(T_chamber - T_far) * R / r` round the chamber and the same round the conduit; where it is over lava's freeze
//! point inside those hollows the cell holds lava at rest, a formula as the untouched sea is.
use super::*;
use crate::bake::volcano::Volcano;

/// A volcano placed in its body's block grid.
#[derive(Clone, Debug)]
pub struct Crater {
    pub v: Volcano,
    pub face: u8,
    /// The middle of its crater, in columns of its face.
    pub at: (f64, f64),
    /// The chamber's middle and the conduit's top, from the body's centre, m.
    pub chamber: DVec3,
    pub top: DVec3,
    /// Its chamber's heat, C (its province's `chamber`; 0: a dead volcano, cold).
    pub heat: f32,
    /// The layer its lava stands level under (the lake's surface): a live volcano whose crater holds a lake.
    pub lake: Option<i32>,
}

/// A live volcano's cone is bare this many crater radii out from its middle: no plant on its upper cone.
// shortcut: a look by distance, standing in for heat, gas and fresh rock killing plants; a rule of the heat
// field once it takes the ground's surface as a boundary and plants read temperature
pub const BARE: f64 = 2.0;
/// The bare cone's ground.
pub(super) static BASALT: std::sync::LazyLock<u16> = std::sync::LazyLock::new(|| crate::blocks::BLOCKS.id("basalt"));

/// Lava's row of `data/fluids.toml` and its freeze point, C.
fn lava() -> (u8, f32) {
    let rows = crate::fluids::FLUIDS.now();
    let k = rows.kind("lava").expect("data/fluids.toml has lava");
    (k, rows.get(k).freeze.as_ref().map_or(f32::INFINITY, |f| f.at))
}

impl Body {
    /// Its map's volcanoes in its grid (`with_map`, once the map is in: the lake is levelled on the ground as
    /// generated). A live one's lake stands a third of the crater's depth over its floor, and two metres under the
    /// lowest ground round the crater's rim, so it never spills; a whole layer, so its level is the same everywhere
    /// it is read (a closed gap's pressure is a whole number of units).
    pub(super) fn craters_of(&self, map: &Map) -> Vec<Crater> {
        let (m, k) = (map.m as usize, self.n as f64 / map.m as f64);
        let hot = ROCKS.provinces.get(crate::bake::rock::ARC as usize).and_then(|p| p.chamber).unwrap_or(0.0);
        map.volcanoes
            .iter()
            .map(|v| {
                let c = v.cell as usize;
                let (f, x, y) = (c / (m * m), c % m, (c / m) % m);
                let (ci, cj) = self.unwarp_col((x as f64 + 0.5) * k, (y as f64 + 0.5) * k);
                let face = f as u8;
                let (i0, j0) = (ci.floor() as i32, cj.floor() as i32);
                let floor = self.col_height(face, i0, j0);
                let heat = if v.alive { hot } else { 0.0 };
                let rc = v.crater as f64;
                let reach = rc.ceil() as i32 + 3;
                let mut rim = f64::INFINITY;
                for (i, j) in (i0 - reach..=i0 + reach).flat_map(|i| (j0 - reach..=j0 + reach).map(move |j| (i, j))) {
                    let d = (i as f64 + 0.5 - ci).hypot(j as f64 + 0.5 - cj);
                    if (rc..rc + 1.5).contains(&d) {
                        rim = rim.min(self.col_height(face, i, j));
                    }
                }
                let level = (floor + v.depth as f64 / 3.0).min(rim - 2.0);
                let lake = (self.layer_of(level).floor() as i32).max(0);
                // shortcut: a crater too shallow for its lake has no lava at all (the bake's crater is deep enough on
                // every map seen), until the conduit's own top is drawn
                let lake = (heat > 0.0 && lake > self.first_air(floor)).then_some(lake);
                let up = self.dir(f, ci, cj);
                let chamber = up * (self.radius - SEA_DEPTH + v.chamber as f64);
                let top = up * self.layer_r(lake.unwrap_or_else(|| self.first_air(floor)) as f64);
                Crater { v: *v, face, at: (ci, cj), chamber, top, heat, lake }
            })
            .collect()
    }
    /// A live volcano's hollows, as caves (`set_caves` carves them): its chamber, a ball, and its conduit, a ball a
    /// layer from the chamber's middle up to the lake's surface (so every column within its radius is hollow all the
    /// way). None for a dead one: its chamber is cold rock.
    pub(super) fn crater_caves(&self, c: &Crater) -> Vec<Cave> {
        let Some(lake) = c.lake else { return Vec::new() };
        let mid = self.layer_of(c.chamber.length());
        let mut out = vec![Cave::ball(c.face, DVec3::new(c.at.0, c.at.1, mid), c.v.chamber_r as f64)];
        out.extend((mid.floor() as i32..lake).map(|z| Cave::ball(c.face, DVec3::new(c.at.0, c.at.1, z as f64 + 0.5), c.v.conduit as f64)));
        out
    }
    /// Its volcanoes in its grid (none before generation 10's map).
    pub fn craters(&self) -> &[Crater] {
        self.baked.as_ref().map_or(&[], |b| &b.craters)
    }
    /// The rock's heat at point p (from the body's centre) whose far heat is `far`, C: the warmest of every live
    /// volcano's conduction, `(T_chamber - far) * R / r` from its chamber and the same from its conduit (a segment
    /// from the chamber's middle to its top, R its radius), all of `T_chamber` inside them.
    pub(super) fn volcano_heat(&self, p: DVec3, far: f32) -> f32 {
        let mut t = far;
        for c in self.craters().iter().filter(|c| c.heat > 0.0) {
            let ball = c.v.chamber_r as f64 / (p - c.chamber).length();
            let ax = c.top - c.chamber;
            let s = ((p - c.chamber).dot(ax) / ax.length_squared().max(1e-9)).clamp(0.0, 1.0);
            let pipe = c.v.conduit as f64 / (p - (c.chamber + ax * s)).length();
            let share = ball.max(pipe).min(1.0) as f32;
            t = t.max(far + (c.heat - far) * share);
        }
        t
    }
    /// Whether column (face, i, j) is on a live volcano's bare upper cone (`BARE`).
    pub(super) fn bare_cone(&self, face: u8, i: i32, j: i32) -> bool {
        self.craters().iter().any(|c| c.heat > 0.0 && c.face == face && (i as f64 + 0.5 - c.at.0).hypot(j as f64 + 0.5 - c.at.1) < BARE * c.v.crater as f64)
    }
    /// The water tiles (`liquid::Col::tile`) of body `body` (this one) that hold a live crater's lake: what a client
    /// draws as generated lava, as it draws stored fluid.
    pub fn lava_tiles(&self, body: u8) -> Vec<Col> {
        let mut out: Vec<Col> = Vec::new();
        for c in self.craters().iter().filter(|c| c.lake.is_some()) {
            let r = c.v.crater as f64 + 1.0;
            let span = |v: f64| ((v - r).floor() as i32).div_euclid(TILE)..=((v + r).floor() as i32).div_euclid(TILE);
            out.extend(span(c.at.0).flat_map(|x| span(c.at.1).map(move |y| Col::new(body, c.face, x, y))));
        }
        out.sort_unstable();
        out.dedup();
        out
    }
    /// Whether column (face, i, j) lies within `reach` columns of a volcano's crater (no plant grows there).
    pub(super) fn in_crater(&self, face: u8, i: i32, j: i32, reach: i32) -> bool {
        self.craters().iter().any(|c| c.face == face && (i as f64 + 0.5 - c.at.0).hypot(j as f64 + 0.5 - c.at.1) < c.v.crater as f64 + reach as f64 + 1.0)
    }
    /// The lava generation keeps in column (face, i, j), as `liquid::Ground::generated_other` gives it: in a live
    /// volcano's chamber and conduit (the column's hollows, which in a volcano's foot are its own) and its crater's
    /// lake, where the heat there is over lava's freeze point. A hollow closed under the ground is full and holds the
    /// lake's head under pressure (`liquid::K`); the one open to the sky holds lava up to the lake's level. So all of
    /// it stands at one head: untouched, nothing moves.
    pub fn lava_col(&self, face: u8, i: i32, j: i32) -> Vec<(i64, i64, i64, u8)> {
        let d = |c: &Crater| (i as f64 + 0.5 - c.at.0).hypot(j as f64 + 0.5 - c.at.1);
        let Some(c) = self.craters().iter().find(|c| c.face == face && c.lake.is_some() && d(c) < c.v.crater.max(c.v.chamber_r) as f64 + 1.0) else { return Vec::new() };
        let (kind, freeze) = lava();
        let molten = |z: f64| self.heat(face, i, j, z).is_some_and(|t| t > freeze);
        let lake = c.lake.expect("found with one") as i64 * U;
        let zs = self.first_air(self.col_height(face, i, j));
        let mut open = if d(c) < c.v.crater as f64 { zs } else { i32::MAX };
        let mut out = Vec::new();
        for &(lo, hi) in self.col_caves(face, i, j) {
            if hi >= zs {
                open = open.min(lo);
            } else if molten((lo + hi) as f64 * 0.5) {
                out.push((lo as i64 * U, hi as i64 * U, (lake - hi as i64 * U).max(0) / crate::liquid::K, kind));
            }
        }
        // the lake's heat: its conduit's top under it
        let (ci, cj) = (c.at.0.floor() as i32, c.at.1.floor() as i32);
        if open < i32::MAX && lake > open as i64 * U && self.heat(face, ci, cj, c.lake.unwrap() as f64 - 0.5).is_some_and(|t| t > freeze) {
            out.push((open as i64 * U, lake, 0, kind));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A world of generation 10 on the first small planet with a live volcano; `dead`: the same map with it dead.
    fn world(dead: bool) -> World {
        let (seed, map) = crate::bake::volcano::tests::live_seed();
        let map = if dead {
            let mut m = Map::from_bytes(&map.to_bytes()).unwrap();
            m.volcanoes.iter_mut().for_each(|v| v.alive = false);
            Arc::new(m)
        } else {
            map
        };
        World::with_grid(768, crate::bodies::moon_columns(768), seed, Some(map), None, 0.0, VOLCANOES)
    }
    fn live(w: &World) -> Crater {
        w.bodies[0].craters().iter().find(|c| c.heat > 0.0).expect("a live volcano").clone()
    }

    /// The heat (volcanoes.md, the rule): all of the chamber's heat in it, over lava's freeze point, and round it the
    /// far heat plus `(T_chamber - T_far) * R / r`: twice as far, half the excess. A dead volcano adds nothing.
    #[test]
    fn the_heat_of_a_live_chamber_is_over_lavas_freeze_and_falls_as_r_over_r() {
        let w = world(false);
        let (b, c, freeze) = (&w.bodies[0], live(&w), lava().1);
        let (i, j) = (c.at.0.floor() as i32, c.at.1.floor() as i32);
        let mid = b.layer_of(c.chamber.length());
        let t = b.heat(c.face, i, j, mid).unwrap();
        assert!(t > freeze && (t - c.heat).abs() < 1e-3, "{t} C in the chamber, {} C its row's, lava freezes at {freeze} C", c.heat);
        let side = (b.dir(c.face as usize, c.at.0 + 1.0, c.at.1) - b.dir(c.face as usize, c.at.0, c.at.1)).normalize();
        let excess = |k: f64| b.volcano_heat(c.chamber + side * (k * c.v.chamber_r as f64), 20.0) - 20.0;
        let (two, four) = (excess(2.0), excess(4.0));
        assert!((two - (c.heat - 20.0) / 2.0).abs() < 1.0 && (two / four - 2.0).abs() < 0.01, "excess {two} C at 2 R, {four} C at 4 R");
        let dead = world(true);
        let t = dead.bodies[0].heat(c.face, i, j, mid).unwrap();
        assert!(t < 200.0, "a dead chamber is the far rock's heat: {t} C");
    }

    /// Lava where the heat keeps it (volcanoes.md, the inside): a live crater's conduit open to its lake, level at
    /// the lake's surface; a column over the chamber off the conduit, the chamber full under the lake's head and the
    /// lake over it; a player's feet there are in lava. A dead crater holds none, and has no hollows.
    #[test]
    fn a_live_crater_holds_lava_and_a_dead_one_none() {
        let w = world(false);
        let (b, c, kind) = (&w.bodies[0], live(&w), lava().0);
        let (i, j) = (c.at.0.floor() as i32, c.at.1.floor() as i32);
        let lake = c.lake.expect("a live crater's lake");
        let zs = b.first_air(b.col_height(c.face, i, j));
        let col = b.lava_col(c.face, i, j);
        assert!(col.len() == 1 && col[0].1 == lake as i64 * U && col[0].2 == 0 && col[0].3 == kind, "the conduit open to the lake: {col:?}");
        assert!(col[0].0 < (zs as i64 - 20) * U && zs < lake, "the conduit reaches down from under the lake: {col:?}, ground {zs}, lake {lake}");
        let off = i + c.v.conduit.ceil() as i32 + 2;
        let col = b.lava_col(c.face, off, j);
        assert!(col.len() == 2 && col[0].2 == (lake as i64 * U - col[0].1) / crate::liquid::K && col[1].1 == lake as i64 * U, "the chamber under pressure, the lake over it: {col:?}");
        let feet = b.centre + b.dir(c.face as usize, i as f64 + 0.5, j as f64 + 0.5) * b.layer_r(lake as f64 - 0.5);
        assert!(w.lava_at(feet) && !w.lava_at(b.centre + (feet - b.centre) * (1.0 + 4.0 / b.radius)), "in the lake, not over it");
        let dead = world(true);
        let (d, dc) = (&dead.bodies[0], dead.bodies[0].craters()[0].clone());
        assert!(dc.lake.is_none() && d.lava_col(c.face, i, j).is_empty() && d.col_caves(c.face, i, j).is_empty(), "a dead crater: no lava, no conduit");
    }

    /// The rest state (volcanoes.md: a formula like the untouched sea): the lake, its conduit and chamber stored and
    /// woken are already at one head, so stepped nothing moves; a cell dug under the lake takes lava by the ordinary
    /// flow, and the lava's ledger stays exact (what is stored is what generation gave).
    #[test]
    fn a_stored_patch_of_the_lava_lake_stepped_does_not_move() {
        let mut w = world(false);
        let (c, kind) = (live(&w), lava().0);
        let (i, j) = (c.at.0.floor() as i32, c.at.1.floor() as i32);
        let r = c.v.crater.ceil() as i32 + 3;
        let cols: Vec<Col> = (i - r..=i + r).flat_map(|a| (j - r..=j + r).map(move |e| Col::new(0, c.face, a, e))).collect();
        {
            let World { bodies, edits, water, sky, time, .. } = &mut w;
            let g = Solid::at(bodies, edits, sky, *time);
            cols.iter().for_each(|&k| water.materialize(&g, k));
            // every node awake: a step looks at all of them
            cols.iter().for_each(|&k| water.regap(&g, k));
        }
        let held = |w: &World| cols.iter().map(|&k| w.water.column(k).map(|n| n.map(|n| (n.floor, n.ceil, n.water, n.kind)).collect::<Vec<_>>())).collect::<Vec<_>>();
        let before = held(&w);
        let stored = w.water.total_of(kind);
        assert!(stored > 1000 * U && w.water.counted_of(kind) == 0, "{} blocks of lava stored, {} counted", stored / U, w.water.counted_of(kind));
        for _ in 0..600 {
            w.step_water();
        }
        assert!(held(&w) == before, "the stored lake moved");
        // a cell dug in the crater's floor, under the lake: lava runs in, and none is made or lost
        let (a, e) = (i + c.v.conduit.ceil() as i32 + 3, j);
        let floor = w.bodies[0].first_air(w.bodies[0].col_height(c.face, a, e)) - 1;
        let cell = Block { body: 0, face: c.face, pos: IVec3::new(a, e, floor) };
        w.set_block(cell, AIR);
        w.regap(cell);
        for _ in 0..600 {
            w.step_water();
        }
        assert_eq!(w.water.kind_at(&w.ground(), Col::new(0, c.face, a, e), floor as f64 + 0.5), Some(kind), "the dug cell holds lava");
        assert_eq!((w.water.counted_of(kind), w.water.counted()), (0, 0), "the ledgers stay exact");
    }

    /// The upper cone of a live volcano is bare: no plant within `BARE` crater radii of its middle, beyond the
    /// crater's own rim. A dead volcano's cone stays green.
    #[test]
    fn a_live_volcanos_upper_cone_is_bare() {
        // (the candidate spots over the cone, and those whose plant is rooted on the upper cone outside the crater)
        let hot_plants = |w: &World| {
            let (b, c, sp) = (&w.bodies[0], w.bodies[0].craters()[0].clone(), crate::biomes::SPACING);
            let (ci, cj, r) = (c.at.0.floor() as i32, c.at.1.floor() as i32, c.v.radius as i32);
            let (mut spots, mut plants) = (0, 0);
            for cx in (ci - r).div_euclid(sp)..=(ci + r).div_euclid(sp) {
                for cy in (cj - r).div_euclid(sp)..=(cj + r).div_euclid(sp) {
                    spots += 1;
                    plants += b.candidate(c.face as usize, cx, cy).is_some_and(|f| (f.i as f64 + 0.5 - c.at.0).hypot(f.j as f64 + 0.5 - c.at.1) < BARE * c.v.crater as f64) as usize;
                }
            }
            (spots, plants)
        };
        let (n, plants) = hot_plants(&world(false));
        assert_eq!(plants, 0, "plants rooted on the upper cone, of {n} spots over the cone");
        assert!(hot_plants(&world(true)).1 > 0, "a dead cone keeps its plants");
        // and its ground is the bare rock, not soil
        let top = |w: &World| {
            let (b, c) = (&w.bodies[0], w.bodies[0].craters()[0].clone());
            b.col_ground(c.face, (c.at.0 + c.v.crater as f64 + 10.0) as i32, c.at.1 as i32, None)
        };
        let basalt = crate::blocks::BLOCKS.id("basalt");
        assert_eq!(top(&world(false)), [basalt; 3], "the live cone's ground");
        assert_ne!(top(&world(true))[0], basalt, "a dead cone keeps its soil");
    }
}
