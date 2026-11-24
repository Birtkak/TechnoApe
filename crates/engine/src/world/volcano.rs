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
    /// The column just outside its rim's lowest column (i, j): where an eruption's lava is poured (`World::erupt`).
    pub lip: (i32, i32),
}

/// The most lamps a crater's lake gives (a 4 x 4 grid of patches over it), so torches beside it keep theirs.
pub const LAKE_LAMPS: i32 = 4;
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
                let (mut rim, mut low) = (f64::INFINITY, (i0, j0));
                for (i, j) in (i0 - reach..=i0 + reach).flat_map(|i| (j0 - reach..=j0 + reach).map(move |j| (i, j))) {
                    let d = (i as f64 + 0.5 - ci).hypot(j as f64 + 0.5 - cj);
                    if (rc..rc + 1.5).contains(&d) {
                        let h = self.col_height(face, i, j);
                        if h < rim {
                            (rim, low) = (h, (i, j));
                        }
                    }
                }
                // the column on from the lowest rim column, away from the middle
                let out = glam::DVec2::new(low.0 as f64 + 0.5 - ci, low.1 as f64 + 0.5 - cj).normalize_or_zero();
                let lip = (low.0 + out.x.round() as i32, low.1 + out.y.round() as i32);
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
                Crater { v: *v, face, at: (ci, cj), chamber, top, heat, lake, lake_cols, lamps, lip }
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

/// What a chamber did in a step of `World::erupt`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Its pressure passed nine tenths of what its plug bears: the warning.
    Stirs,
    /// Its pressure passed its plug's: the conduit opened.
    Erupts,
    /// The melt over its rest is spent: the conduit closed.
    Ends,
}

/// A live crater's chamber (`World::chamber`), in units of melt (`liquid::U` a cubic metre).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chamber {
    /// The melt over its rest, and whether its conduit is open.
    pub fill: f64,
    pub open: bool,
    /// The melt over its rest its plug bears (`plug / bulk` of its volume), and the plug's strength, MPa.
    pub holds: f64,
    pub plug: f64,
    /// What arrives from below a second, and what pours from the crater a second while it erupts.
    pub supply: f64,
    pub flow: f64,
}

impl Chamber {
    /// Its overpressure, MPa: `bulk` times its fill over its volume.
    pub fn pressure(&self) -> f64 {
        self.plug * self.fill / self.holds
    }
    /// Seconds until it erupts (0 while it does).
    pub fn next(&self) -> f64 {
        if self.open { 0.0 } else { ((self.holds - self.fill) / self.supply).max(0.0) }
    }
}

impl World {
    /// The chamber of the home's crater k (`Body::craters`' order), None for a dead one or a province that never
    /// erupts. A chamber not stepped yet starts filled a share of its plug's by a hash of the seed and its cell, so
    /// a planet's volcanoes do not erupt together.
    pub fn chamber(&self, k: usize) -> Option<Chamber> {
        let b = &self.bodies[0];
        let c = b.craters().get(k).filter(|c| c.lake.is_some())?;
        let e = ROCKS.provinces.get(crate::bake::rock::ARC as usize)?.eruption?;
        let v = 4.0 / 3.0 * std::f64::consts::PI * (c.v.chamber_r as f64).powi(3) * U as f64;
        let holds = v * e.plug as f64 / e.bulk as f64;
        let seed = b.map().map_or(0, |m| m.seed);
        let (fill, open) = self.water.chambers.get(k).copied().unwrap_or_else(|| {
            let h = fnv(fnv(FNV0, &seed.to_le_bytes()), &c.v.cell.to_le_bytes());
            ((h >> 11) as f64 / (1u64 << 53) as f64 * holds, false)
        });
        Some(Chamber { fill, open, holds, plug: e.plug as f64, supply: v * e.supply as f64 / self.sky.year, flow: e.effuse as f64 * U as f64 })
    }

    /// `dt` seconds of the home's live chambers (volcanoes.md "Eruptions"; the server's, once a second with its heat
    /// step): each fills at its supply; past its plug its conduit opens and the melt over its rest is poured as
    /// counted lava (`Liquids::erupted`) at its crater's lip, at its flow, until it is spent. A handful of numbers a
    /// step, whatever the world's size.
    /// shortcut: poured on the outer lip of the rim's lowest column, where a risen lake would spill, not by raising
    /// the crater's lake (generation's formula at a fixed level; 1 % of a chamber raises it centimetres), until the
    /// lake's level follows the chamber's pressure.
    /// shortcut: poured whether a player is near or not (the liquid step runs on every stored column of a loaded
    /// body, and the home always is); held while the home is not loaded.
    pub fn erupt(&mut self, dt: f64) -> Vec<(usize, Stage)> {
        let mut out = Vec::new();
        let b = self.bodies[0].clone();
        let lava = lava().0;
        for (k, c) in b.craters().iter().enumerate() {
            let Some(mut ch) = self.chamber(k) else { continue };
            if self.water.chambers.len() <= k {
                let all: Vec<(f64, bool)> = (0..b.craters().len()).map(|k| self.chamber(k).map_or((0.0, false), |c| (c.fill, c.open))).collect();
                self.water.chambers = all;
            }
            let was = ch.fill;
            ch.fill += ch.supply * dt;
            if !ch.open && ch.fill > ch.holds {
                ch.open = true;
                out.push((k, Stage::Erupts));
            } else if !ch.open && was <= 0.9 * ch.holds && ch.fill > 0.9 * ch.holds {
                out.push((k, Stage::Stirs));
            }
            if ch.open && self.loaded(0) {
                let n = (ch.flow * dt).min(ch.fill.floor()) as i64;
                let (i, j) = c.lip;
                let layer = b.first_air(b.col_height(c.face, i, j));
                let World { bodies, edits, water, sky, time, .. } = self;
                if n > 0 && water.pour_kind(&Solid::at(bodies, edits, sky, *time), Col::new(0, c.face, i, j), layer, n, lava) {
                    ch.fill -= n as f64;
                    water.erupted += n;
                }
                if ch.fill < 1.0 {
                    ch.open = false;
                    out.push((k, Stage::Ends));
                }
            }
            self.water.chambers[k] = (ch.fill, ch.open);
        }
        out
    }

    /// Tops the live chamber of the home's nearest to world point p up to its plug's strength, so it erupts by the
    /// rule at the next step (a command's, `/erupt`): its crater's index, None without a live one.
    pub fn force_eruption(&mut self, p: DVec3) -> Option<usize> {
        let b = self.bodies[0].clone();
        let k = (0..b.craters().len()).filter(|&k| self.chamber(k).is_some()).min_by(|&x, &y| {
            let d = |k: usize| (b.centre + b.craters()[k].top - p).length();
            d(x).total_cmp(&d(y))
        })?;
        self.erupt(0.0);
        let ch = self.chamber(k)?;
        self.water.chambers[k].0 = ch.fill.max(ch.holds);
        Some(k)
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
        // this lake's own (another crater's lamps lie in reach too): what lies within its lamps' farthest from the eye
        let far = c.lamps.iter().map(|l| (w.bodies[0].centre + l.0 - eye).length()).fold(0.0, f64::max) + 2.0;
        let near = |w: &World| -> Vec<(u8, DVec3, [f32; 3])> {
            let mut v: Vec<_> = w.fluid_lamps(&cams, 300.0).into_iter().filter(|l| (l.1 - eye).length() < far).collect();
            v.sort_by(|a, b| a.1.to_array().partial_cmp(&b.1.to_array()).unwrap());
            v
        };
        let lamps = near(&w);
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
        assert_eq!(near(&w), lamps, "storing its columns moves no lamp");
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

    /// Water's phases through the world's edits (liquids.md step 5): a pond 2 1/3 blocks deep under -30 C air freezes,
    /// but while its cell is taken (a player in it) no ice is set there and the freeze holds at the one block booked,
    /// waiting (decision D), every unit booked; free, the ice is set where the water's top was, once, then the next block
    /// under it, the third of a block short of one stays water; in +20 C air it all melts back to exactly its water.
    #[test]
    fn freezing_water_never_buries_a_player_and_its_ice_melts_back_in_warm_air() {
        let mut w = world(false);
        let water = crate::fluids::WATER;
        let units = 2 * U + U / 3;
        let (cols, z0) = pit(&mut w, 1, 3, units, water);
        let ice = crate::blocks::BLOCKS.id("ice");
        let cell = |z| Block { body: 0, face: cols[0].face(), pos: IVec3::new(cols[0].i(), cols[0].j(), z) };
        let ledger = |w: &World| w.water.counted() + w.water.vapour.get(&water).copied().unwrap_or(0) + w.water.froze.get(&water).copied().unwrap_or(0);
        let (cold, warm) = (|_: Col, _: f64| Some(-30.0), |_: Col, _: f64| Some(20.0));
        for _ in 0..1000 {
            assert!(w.cool_fluids_in(600.0, |_| true, Some(&cold)).is_empty(), "no ice while the cell is taken");
            w.step_water();
            assert_eq!(ledger(&w), units, "every unit booked");
            assert!(w.water.waiting.len() <= 1, "one block waits: {:?}", w.water.waiting);
        }
        assert_eq!((w.block(cell(z0 + 2)), w.water.total(), w.water.waiting.len()), (AIR, units - U, 1), "one block waiting for the cell, the rest water");
        let mut set = Vec::new();
        for _ in 0..1000 {
            set.extend(w.cool_fluids_in(600.0, |_| false, Some(&cold)));
            w.step_water();
        }
        assert_eq!(set, vec![(cell(z0 + 2), AIR, ice), (cell(z0 + 1), AIR, ice)], "free, the waiting block once, then the next under it");
        assert_eq!((w.water.total(), ledger(&w)), (U / 3, units), "short of a block, water");
        let mut t = 0.0;
        while (w.block(cell(z0 + 2)) == ice || w.block(cell(z0 + 1)) == ice) && t < 1e7 {
            set.extend(w.cool_fluids_in(600.0, |_| false, Some(&warm)));
            w.step_water();
            t += 600.0;
        }
        eprintln!("melted back in {t:.0} s");
        assert_eq!(set.last().map(|s| s.2), Some(AIR), "the ice melted");
        assert_eq!((w.water.total(), ledger(&w)), (units, units), "back to exactly its water");
    }

    /// Ice melting where its water cannot be poured (decision F: the gap holds its most layers, here one of oil) stays
    /// ice and tries again each second; once the water has a place the block is its water.
    #[test]
    fn ice_whose_water_cannot_be_poured_stays_ice_until_it_can() {
        let mut w = world(false);
        let water = crate::fluids::WATER;
        let oil = crate::fluids::FLUIDS.now().kind("oil").unwrap();
        let (cols, z0) = pit(&mut w, 1, 3, U, oil);
        let ice = crate::blocks::BLOCKS.id("ice");
        let cell = Block { body: 0, face: cols[0].face(), pos: IVec3::new(cols[0].i(), cols[0].j(), z0 + 2) };
        w.set_block(cell, ice);
        w.regap(cell);
        w.watch_ice_near(cell);
        w.water.set_layers(1);
        let ledger = |w: &World| w.water.counted_of(water) + w.water.vapour.get(&water).copied().unwrap_or(0) + w.water.froze.get(&water).copied().unwrap_or(0);
        let warm = |_: Col, _: f64| Some(20.0);
        for _ in 0..200 {
            w.cool_fluids_in(600.0, |_| false, Some(&warm));
            w.step_water();
            assert_eq!(ledger(&w), 0, "every unit booked");
        }
        assert_eq!((w.block(cell), w.water.total_of(water)), (ice, 0), "kept as ice while its water has no place");
        w.water.set_layers(0);
        let mut t = 0.0;
        while w.block(cell) == ice && t < 1e6 {
            w.cool_fluids_in(600.0, |_| false, Some(&warm));
            w.step_water();
            t += 600.0;
        }
        assert_eq!((w.block(cell), w.water.total_of(water), ledger(&w)), (AIR, U, 0), "then its water");
    }

    /// Lava at a coarse step (decision C: the server's step under time warp) sets every whole block it froze, more than
    /// one a step if it must: 3 U poured are 3 blocks of basalt.
    #[test]
    fn lava_at_a_coarse_step_sets_every_block_it_froze() {
        for dt in [1e4, 1e5, 1e6] {
            let mut w = world(false);
            let kind = lava().0;
            let (cols, z0) = pit(&mut w, 1, 4, 3 * U, kind);
            let basalt = crate::blocks::BLOCKS.id("basalt");
            let mut t = 0.0;
            while w.water.total_of(kind) > 0 && t < 1e9 {
                w.cool_fluids(dt, |_| false);
                w.step_water();
                t += dt;
            }
            let blocks = (z0..z0 + 4).filter(|&z| w.block(Block { body: 0, face: cols[0].face(), pos: IVec3::new(cols[0].i(), cols[0].j(), z) }) == basalt).count() as i64;
            assert_eq!((w.water.total_of(kind), blocks * U, w.water.froze.get(&kind).copied()), (0, 3 * U, Some(3 * U)), "dt {dt}");
        }
    }

    /// An old world on the baked climate: a block placed in the sea and dug (its columns stored, regapped) and stepped
    /// with the heat step a while is just as it is stepped without it: no heat held, nothing frozen (decision A).
    #[test]
    fn stored_sea_on_the_baked_climate_steps_as_it_did_without_the_heat_step() {
        let after = |heat: bool| {
            let mut w = world(false);
            let b = w.bodies[0].clone();
            let (i, j) = (0..b.n).step_by(2).flat_map(|i| (0..b.n).step_by(2).map(move |j| (i, j))).find(|&(i, j)| b.height(b.dir(2, i as f64 + 0.5, j as f64 + 0.5)) < b.radius - 7.0).unwrap();
            let c = Col::new(0, 2, i, j);
            let (_, hi) = w.ground().generated(c);
            let at = Block { body: 0, face: 2, pos: IVec3::new(i, j, (hi / U) as i32) };
            w.set_block(at, crate::blocks::BLOCKS.id("stone"));
            w.regap(at);
            assert!(w.water.stored() > 0);
            for k in 0..600 {
                if k == 300 {
                    w.put_edit(at, None);
                    w.regap(at);
                }
                if heat {
                    assert!(w.cool_fluids(1.0, |_| false).is_empty(), "nothing freezes or melts");
                }
                for _ in 0..60 {
                    w.step_water();
                }
            }
            (w.water.all(), w.water.heats().len(), w.water.froze.clone(), w.water.vapour.clone())
        };
        let (with, without) = (after(true), after(false));
        assert_eq!(with.1, 0, "no heat held");
        assert_eq!(with, without);
    }

    /// Two blocks frozen for one cell that was taken (a reaction's two skins in one second): both are set the next
    /// time, the second on the first, in the water over it; none is lost and nothing comes back as lava.
    #[test]
    fn frozen_blocks_for_one_cell_stack_upward_and_none_is_lost() {
        let mut w = world(false);
        let kind = lava().0;
        let (cols, z0) = pit(&mut w, 1, 4, U, crate::fluids::WATER);
        let basalt = crate::blocks::BLOCKS.id("basalt");
        *w.water.froze.entry(kind).or_default() += 2 * U;
        w.water.unfreeze(cols[0], z0, kind);
        w.water.unfreeze(cols[0], z0, kind);
        let cell = |z| Block { body: 0, face: cols[0].face(), pos: IVec3::new(cols[0].i(), cols[0].j(), z) };
        let set = w.cool_fluids(1.0, |_| false);
        assert_eq!(set, vec![(cell(z0), AIR, basalt), (cell(z0 + 1), AIR, basalt)], "both set, the second on the first");
        assert_eq!((w.water.counted_of(kind), w.water.froze[&kind], w.water.total_of(crate::fluids::WATER)), (0, 2 * U, U));
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

    /// The live crater's index.
    fn live_k(w: &World) -> usize {
        w.bodies[0].craters().iter().position(|c| c.lake.is_some()).expect("a live volcano")
    }
    /// Lava's ledger: what differs from generation, what of it froze and boiled off.
    fn lava_ledger(w: &World) -> i64 {
        let k = lava().0;
        w.water.counted_of(k) + w.water.froze.get(&k).copied().unwrap_or(0) + w.water.vapour.get(&k).copied().unwrap_or(0)
    }
    /// Crater k's chamber `holds - left` units short of erupting.
    fn short_of(w: &mut World, k: usize, left: f64) -> Chamber {
        w.erupt(0.0);
        let ch = w.chamber(k).unwrap();
        w.water.chambers[k].0 = ch.holds - left;
        w.chamber(k).unwrap()
    }

    /// A live chamber fills at its supply and erupts the step its fill passes what its plug bears, not before
    /// (volcanoes.md "Eruptions"): an hour a step from where the seed starts it, its fill growing by exactly its
    /// supply, nothing poured, until it passes; the warning nine tenths of the way. Its plug and supply give one
    /// eruption in three game years.
    #[test]
    fn a_chamber_fills_at_its_supply_and_erupts_when_it_passes_its_plug_not_before() {
        let mut w = world(false);
        let k = live_k(&w);
        let c0 = w.chamber(k).expect("a live chamber");
        let years = c0.holds / c0.supply / w.sky.year;
        eprintln!("chamber: holds {:.1} m3 over its rest, {:.3} m3 a day, starts at {:.1} m3, one eruption in {years:.2} years", c0.holds / U as f64, c0.supply * 1800.0 / U as f64, c0.fill / U as f64);
        assert!((years - 3.0).abs() < 0.01, "an eruption every {years} years");
        assert!(c0.fill >= 0.0 && c0.fill < c0.holds && !c0.open, "{c0:?}");
        let dt = 3600.0;
        let (mut stirred, mut n) = (false, 0);
        loop {
            let before = w.chamber(k).unwrap();
            let st = w.erupt(dt);
            let now = w.chamber(k).unwrap();
            if before.fill + before.supply * dt > before.holds {
                assert!(st.contains(&(k, Stage::Erupts)) && w.water.erupted > 0, "erupted the step it passed: {st:?}");
                break;
            }
            assert!(!now.open && w.water.erupted == 0 && !st.contains(&(k, Stage::Erupts)), "nothing before it passed: {st:?} {now:?}");
            assert!((now.fill - before.fill - before.supply * dt).abs() < 1e-6, "filled at its supply");
            stirred |= st.contains(&(k, Stage::Stirs));
            n += 1;
            assert!(n < 30_000, "never erupted");
        }
        assert!(stirred || c0.fill > 0.9 * c0.holds, "the warning came first");
        eprintln!("erupted after {n} hours");
    }

    /// An eruption pours exactly the chamber's melt over its rest, every unit booked as erupted (lava's ledger stays
    /// whole through the pour, the flow down the cone, freezing to basalt), and it ends when that melt is spent.
    #[test]
    fn an_eruption_pours_exactly_its_excess_and_lavas_ledger_holds_to_basalt() {
        let mut w = world(false);
        let k = live_k(&w);
        let supply = w.chamber(k).unwrap().supply;
        let f0 = short_of(&mut w, k, 0.5 * supply).fill;
        let mut stages = Vec::new();
        let mut supplied = 0.0;
        for _ in 0..600 {
            supplied += w.chamber(k).unwrap().supply;
            stages.extend(w.erupt(1.0));
            w.cool_fluids(1.0, |_| false);
            for _ in 0..60 {
                w.step_water();
            }
            assert_eq!(lava_ledger(&w), w.water.erupted, "every unit poured is booked");
        }
        let end = w.chamber(k).unwrap();
        assert_eq!(stages, vec![(k, Stage::Erupts), (k, Stage::Ends)]);
        // (spent to under a unit when it closed, then filling again at its supply)
        assert!(!end.open && end.fill < 1.0 + 600.0 * supply, "spent: {end:?}");
        assert!((w.water.erupted as f64 - (f0 + supplied - end.fill)).abs() < 1e-6, "poured {} of {}", w.water.erupted, f0 + supplied - end.fill);
        eprintln!("poured {:.1} m3", w.water.erupted as f64 / U as f64);
        let mut basalt = 0;
        for _ in 0..3 * 1440 {
            basalt += w.cool_fluids(60.0, |_| false).iter().filter(|e| e.2 == *BASALT).count();
            for _ in 0..4 {
                w.step_water();
            }
            assert_eq!(lava_ledger(&w), w.water.erupted, "booked as it flows and freezes");
        }
        let lava_left = w.water.counted_of(lava().0);
        eprintln!("{basalt} blocks of basalt, {:.1} m3 still lava, {:.1} m3 froze", lava_left as f64 / U as f64, w.water.froze.get(&lava().0).copied().unwrap_or(0) as f64 / U as f64);
        assert!(basalt > 0, "the flow froze to basalt");
    }

    /// It ends because the melt is spent, not on a timer: twice the melt over its rest pours twice as long.
    #[test]
    fn an_eruption_ends_when_its_excess_is_spent() {
        let last = |excess: f64| {
            let mut w = world(false);
            let k = live_k(&w);
            let ch = short_of(&mut w, k, 0.0);
            w.water.chambers[k].0 = ch.holds * excess;
            let mut s = 0;
            while s < 10_000 {
                s += 1;
                if w.erupt(1.0).contains(&(k, Stage::Ends)) {
                    break;
                }
            }
            (s, w.water.erupted)
        };
        let (a, b) = (last(1.0), last(2.0));
        eprintln!("{a:?} {b:?}");
        assert!(a.0 > 3 && (b.0 as f64 / a.0 as f64 - 2.0).abs() < 0.1, "seconds poured: {} and {}", a.0, b.0);
        assert!((b.1 as f64 / a.1 as f64 - 2.0).abs() < 0.01, "units poured: {} and {}", a.1, b.1);
    }

    /// Two worlds of one seed erupt the same, to the step: the same stages at the same steps and the same lava where
    /// it ran.
    #[test]
    fn two_worlds_of_one_seed_erupt_the_same_to_the_step() {
        let run = || {
            let mut w = world(false);
            let k = live_k(&w);
            let supply = w.chamber(k).unwrap().supply;
            short_of(&mut w, k, 3.5 * supply);
            let mut log = Vec::new();
            for s in 0..120 {
                log.extend(w.erupt(1.0).into_iter().map(|e| (s, e)));
                w.cool_fluids(1.0, |_| false);
                for _ in 0..60 {
                    w.step_water();
                }
            }
            (log, w.water.all(), w.water.chambers.iter().map(|c| (c.0.to_bits(), c.1)).collect::<Vec<_>>())
        };
        let (a, b) = (run(), run());
        assert!(a.0.len() == 2 && a.0[0].0 == 3, "erupted at its fourth second: {:?}", a.0);
        assert_eq!(a, b);
    }

    /// A dead volcano never erupts, forced or not; a world without a live one (no map, or the map's volcanoes dead)
    /// keeps no chamber and its water steps as it did.
    #[test]
    fn a_dead_volcano_never_erupts_and_a_world_without_a_live_one_steps_as_before() {
        let mut w = world(true);
        assert!((0..w.bodies[0].craters().len()).all(|k| w.chamber(k).is_none()));
        assert_eq!(w.force_eruption(w.bodies[0].centre), None, "nothing to force");
        for _ in 0..10_000 {
            assert!(w.erupt(1.0e4).is_empty());
        }
        assert!(w.water.chambers.is_empty() && w.water.erupted == 0 && w.water.stored() == 0);
        let mut old = World::new(256, 32, 5);
        let before = old.water.all();
        for _ in 0..100 {
            assert!(old.erupt(1.0e4).is_empty());
            old.step_water();
        }
        assert!(old.water.chambers.is_empty() && old.water.erupted == 0 && old.water.all() == before);
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
