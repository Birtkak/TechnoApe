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
    /// Its lake's columns (i, j), sorted, and its lamps (`World::fluid_lamps`): at most LAKE_LAMPS, each the middle
    /// of a patch of the lake's surface from the body's centre and how many 2 x 2 patches' light it gives (a quarter
    /// of its columns), so the lake is as bright in sum as a lamp a 2 x 2 patch. Worked out once: the lake is a
    /// formula and never moves.
    pub lake_cols: Vec<(i32, i32)>,
    pub lamps: Vec<(DVec3, f32)>,
}

/// The most lamps a crater's lake gives (a 4 x 4 grid of patches over it), so torches beside it keep theirs.
pub const LAKE_LAMPS: i32 = 4;

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
                // the lake's columns: in the crater, the ground under the lake's surface (shortcut: a cave opening a
                // column whose ground stands over the lake is not counted, `lava_col` would give it lava)
                let mut lake_cols = Vec::new();
                let mut lamps = Vec::new();
                if let Some(l) = lake {
                    let r = rc.ceil() as i32 + 1;
                    lake_cols = (i0 - r..=i0 + r)
                        .flat_map(|i| (j0 - r..=j0 + r).map(move |j| (i, j)))
                        .filter(|&(i, j)| (i as f64 + 0.5 - ci).hypot(j as f64 + 0.5 - cj) < rc && self.first_air(self.col_height(face, i, j)) < l)
                        .collect();
                    lake_cols.sort_unstable();
                    let cell = ((2 * r + 1) as f64 / LAKE_LAMPS as f64).ceil() as i32;
                    for (a, b) in (0..LAKE_LAMPS).flat_map(|a| (0..LAKE_LAMPS).map(move |b| (a, b))) {
                        let at: Vec<DVec3> = lake_cols
                            .iter()
                            .filter(|&&(i, j)| (i - (i0 - r)) / cell == a && (j - (j0 - r)) / cell == b)
                            .map(|&(i, j)| self.dir(f, i as f64 + 0.5, j as f64 + 0.5) * self.layer_r(l as f64 - 0.5))
                            .collect();
                        if !at.is_empty() {
                            lamps.push((at.iter().sum::<DVec3>() / at.len() as f64, at.len() as f32 / 4.0));
                        }
                    }
                }
                Crater { v: *v, face, at: (ci, cj), chamber, top, heat, lake, lake_cols, lamps }
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
    /// The heat a fluid at layer z of column (face, i, j) rests at, C (`liquid::Ground::heat`): where generation keeps
    /// lava (`lava_col`), its volcano's chamber heat, which by the formula holds it there liquid (the lake's lava
    /// is its conduit's, turned over); elsewhere the rock's and air's (`heat`), or `liquid::AMBIENT` without a map.
    pub fn rest_heat(&self, face: u8, i: i32, j: i32, z: f64) -> f32 {
        let u = (z * U as f64) as i64;
        let d = |c: &Crater| (i as f64 + 0.5 - c.at.0).hypot(j as f64 + 0.5 - c.at.1);
        if let Some(c) = self.craters().iter().find(|c| c.face == face && c.lake.is_some() && d(c) < c.v.crater.max(c.v.chamber_r) as f64 + 1.0) {
            if self.lava_col(face, i, j).iter().any(|w| (w.0..w.1).contains(&u)) {
                return c.heat;
            }
        }
        self.heat(face, i, j, z).unwrap_or(crate::liquid::AMBIENT)
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

    /// A pit `n` x `n` columns and `deep` layers at the spawn (cold ground, far from the volcano), `units` of fluid
    /// `kind` poured into each column: its columns, row by row, and its floor layer.
    fn pit(w: &mut World, n: i32, deep: i32, units: i64, kind: u8) -> (Vec<Col>, i32) {
        let b = w.bodies[0].clone();
        let (f, i, j) = b.locate(b.spawn);
        let (f, i, j) = (f as u8, i as i32, j as i32);
        let top = b.first_air(b.col_height(f, i, j));
        let z0 = top - deep;
        let cols: Vec<Col> = (0..n).flat_map(|a| (0..n).map(move |e| Col::new(0, f, i + a, j + e))).collect();
        for c in &cols {
            for z in z0..top + 3 {
                w.set_block(Block { body: 0, face: f, pos: IVec3::new(c.i(), c.j(), z) }, AIR);
            }
        }
        let World { bodies, edits, water, sky, time, .. } = w;
        let g = Solid::at(bodies, edits, sky, *time);
        for &c in &cols {
            assert!(water.pour_kind(&g, c, z0, units, kind));
        }
        (cols, z0)
    }

    /// Lava lights what is round it as a lamp block does (lava basics): a lamp at a stored pool's surface beside the
    /// block next to it, in lava's light, and none for water; a live crater's lake, untouched, gives lamps too.
    #[test]
    fn a_lava_pool_lights_the_block_beside_it_and_water_does_not() {
        let rows = crate::fluids::FLUIDS.now();
        for kind in [lava().0, crate::fluids::WATER] {
            let mut w = world(false);
            let (cols, z0) = pit(&mut w, 1, 2, U, kind);
            let beside = w.block_centre(Block { body: 0, face: cols[0].face(), pos: IVec3::new(cols[0].i() + 1, cols[0].j(), z0) });
            let lamps = w.fluid_lamps(&vec![beside; w.bodies.len()], 48.0);
            let lit = lamps.iter().any(|l| (l.1 - beside).length() < 1.5 && l.2 == rows.get(kind).light);
            assert_eq!(lit, kind != crate::fluids::WATER, "kind {kind}: lamps {lamps:?}");
        }
    }

    /// A live crater's lake lights by a few lamps worked out once (the review of lava basics): at most 16, as bright in
    /// sum as a lamp for every 2 x 2 columns of it, and the same lamps once its columns are stored and woken.
    #[test]
    fn a_craters_lake_lights_by_at_most_sixteen_lamps_that_hold_when_it_is_stored() {
        let mut w = world(false);
        let c = live(&w);
        let light = crate::fluids::FLUIDS.now().get(lava().0).light[0];
        let eye = w.bodies[0].centre + c.top;
        let cams = vec![eye; w.bodies.len()];
        let lamps = w.fluid_lamps(&cams, 300.0);
        let sum: f32 = lamps.iter().map(|l| l.2[0] / light).sum();
        let want = c.lake_cols.len() as f32 / 4.0;
        assert!(want > 16.0, "a lake whose lamp a 2 x 2 patch ({want}) would pass the cap");
        assert!(!lamps.is_empty() && lamps.len() <= 16, "the lake's lamps: {}", lamps.len());
        assert!((sum - want).abs() < 1e-3 * want, "as bright as {want} patches: {sum}");
        {
            let World { bodies, edits, water, sky, time, .. } = &mut w;
            let g = Solid::at(bodies, edits, sky, *time);
            let cols: Vec<Col> = c.lake_cols.iter().map(|&(i, j)| Col::new(0, c.face, i, j)).collect();
            cols.iter().for_each(|&k| water.materialize(&g, k));
            cols.iter().for_each(|&k| water.regap(&g, k));
            assert!(water.stored() > 0);
        }
        assert_eq!(w.fluid_lamps(&cams, 300.0), lamps, "storing its columns moves no lamp");
    }

    /// Lava poured into a pit on cold ground freezes to basalt at real rates (lava basics, draft decision): a pit
    /// 3 x 3 and lava two blocks deep in each column. Every unit that froze is booked, so lava's ledger plus what froze
    /// is what was poured.
    ///
    /// The hand calculation for the whole pool (lava 2600 kg/m3, c 1200 J/(kg K), L 400 kJ/kg, k 1.5 W/(m K), poured
    /// at 1150 C, freezing at 1000 C, air and rock at about 15 C, rock effusivity 2200 W s^0.5/(m2 K)):
    /// - to hold: 9 columns of 5200 kg, 150 K of heat (9.4e8 J each) and the freeze (2.1e9 J each): 2.7e10 J;
    /// - the top, 9 m2 through 1 m of its own lava (0.67 m2 K/W) to a surface near 100 C: 1500 W/m2, 13.5 kW;
    /// - the bottom, 9 m2 through the same and the rock's warming (0.25 m2 K/W after a day, 0.55 after five):
    ///   about 1000 W/m2, 9 kW; the 12 outer sides, 24 m2 through half a block (0.33) and the rock: 1500 W/m2, 35 kW;
    /// - about 57 kW while it cools to the freeze point (8.4e9 J, 1.5e5 s), about 50 kW while it freezes (1.9e10 J,
    ///   the depth falling as it goes: 3.7e5 s). So about 5.2e5 s, six days.
    #[test]
    fn a_lava_pool_on_cold_ground_freezes_to_basalt_in_its_time_and_the_ledger_holds() {
        // at the server's step (a second) and a coarser one: the same physics, the same answer
        for dt in [1.0, 10.0] {
            let mut w = world(false);
            let kind = lava().0;
            let (cols, z0) = pit(&mut w, 3, 3, 2 * U, kind);
            let mid = cols[4];
            let ta = w.ground().heat(mid, z0 as f64 + 1.0);
            assert!(ta < 60.0, "cold ground: {ta} C");
            assert_eq!(w.water.temp_at(mid, z0), Some(1150.0), "poured at its row's heat");
            let basalt = crate::blocks::BLOCKS.id("basalt");
            let poured = 9 * 2 * U;
            let ledger = |w: &World| w.water.counted_of(kind) + w.water.froze.get(&kind).copied().unwrap_or(0);
            let mut t = 0.0;
            while w.water.total_of(kind) > 0 && t < 6.0e6 {
                w.cool_fluids(dt, |_| false);
                w.step_water();
                t += dt;
                if (t as i64) % 100_000 == 0 {
                    assert_eq!(ledger(&w), poured, "the ledger at {t} s");
                }
            }
            let hand = 5.2e5;
            eprintln!("dt {dt}: the pool froze through in {t:.0} s ({:.1} days), by hand {hand:.0} s", t / 86400.0);
            assert!(t > hand / 2.0 && t < hand * 2.0, "dt {dt}: froze in {t} s, by hand {hand} s");
            assert_eq!((w.water.total_of(kind), ledger(&w)), (0, poured), "dt {dt}: all of it froze, every unit booked");
            let blocks = cols.iter().flat_map(|c| (z0..z0 + 2).map(move |z| Block { body: 0, face: c.face(), pos: IVec3::new(c.i(), c.j(), z) })).filter(|&b| w.block(b) == basalt).count();
            eprintln!("{blocks} blocks of basalt in the pit, {} units froze", w.water.froze[&kind]);
            assert!((16..=18).contains(&blocks), "dt {dt}: {blocks} blocks of basalt for 18 poured");
        }
    }

    /// Freezing buries nothing: while the cell is taken (a player in it) the lava sets no block there and every unit
    /// stays booked; once it is free the lava freezes through as before, and the edit names the air it filled.
    #[test]
    fn freezing_lava_waits_while_its_cell_is_taken() {
        let mut w = world(false);
        let kind = lava().0;
        let (cols, z0) = pit(&mut w, 1, 2, U, kind);
        let basalt = crate::blocks::BLOCKS.id("basalt");
        let cell = Block { body: 0, face: cols[0].face(), pos: IVec3::new(cols[0].i(), cols[0].j(), z0) };
        let ledger = |w: &World| w.water.counted_of(kind) + w.water.froze.get(&kind).copied().unwrap_or(0);
        for _ in 0..10_000 {
            assert!(w.cool_fluids(60.0, |_| true).is_empty(), "no block while the cell is taken");
            w.step_water();
            assert_eq!(ledger(&w), U, "every unit booked");
        }
        assert_eq!(w.block(cell), AIR, "the taken cell holds no basalt");
        let mut set = Vec::new();
        for _ in 0..10_000 {
            set.extend(w.cool_fluids(60.0, |_| false));
            w.step_water();
        }
        assert_eq!(set, vec![(cell, AIR, basalt)], "free, it froze to a block that filled the air");
        assert_eq!((w.block(cell), w.water.total_of(kind), ledger(&w)), (basalt, 0, U));
    }

    /// The rest state (lava basics): a live crater's lake stored and woken, and a block of lava poured onto it, stepped
    /// a simulated week, freezes nothing: the crater's heat holds it at its chamber's, the formula's own reason.
    #[test]
    fn a_stored_lake_and_a_pour_onto_it_stay_liquid_in_the_craters_heat() {
        let mut w = world(false);
        let (c, kind) = (live(&w), lava().0);
        let (i, j) = (c.at.0.floor() as i32, c.at.1.floor() as i32);
        let r = c.v.crater.ceil() as i32 + 3;
        let cols: Vec<Col> = (i - r..=i + r).flat_map(|a| (j - r..=j + r).map(move |e| Col::new(0, c.face, a, e))).collect();
        let lake = c.lake.unwrap();
        {
            let World { bodies, edits, water, sky, time, .. } = &mut w;
            let g = Solid::at(bodies, edits, sky, *time);
            cols.iter().for_each(|&k| water.materialize(&g, k));
            cols.iter().for_each(|&k| water.regap(&g, k));
            assert!(water.pour_kind(&g, Col::new(0, c.face, i, j), lake - 1, U, kind));
        }
        for _ in 0..7 * 1440 {
            assert!(w.cool_fluids(60.0, |_| false).is_empty(), "nothing freezes");
            for _ in 0..4 {
                w.step_water();
            }
        }
        assert!(w.water.froze.is_empty() && w.water.counted_of(kind) == U, "froze {:?}, counted {}", w.water.froze, w.water.counted_of(kind) / U);
        assert!(w.water.heats().iter().all(|h| h.2.t > lava().1 as f64), "every heat held is over the freeze point");
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
}
