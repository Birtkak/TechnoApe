//! The temperature felt at a point (draft 2026-11-24, "Felt temperature"): the operative temperature of ASHRAE 55 and
//! ISO 7726, the air's and the mean radiant temperature's mean weighted by how a person loses heat to each; the mean
//! radiant temperature from the hot surfaces in sight by each one's view factor. And the temperatures the look-at
//! panel shows: a block's and a fluid's at its cell. Read by the client a few times a second, never per block per
//! frame; nothing here changes the simulation.
use super::*;

/// Stefan-Boltzmann's constant, W/m2K4.
pub const SIGMA: f64 = 5.670_374_419e-8;
/// A person's convective coefficient in still air, W/m2K (ASHRAE Handbook, Fundamentals, ch. 9 "Thermal comfort":
/// h_c = 3.1 for a person at rest in air under 0.2 m/s). None on a body without air.
pub const H_C: f64 = 3.1;
/// A clothed person's emissivity (ASHRAE Fundamentals ch. 9: 0.95), in h_r = 4 eps sigma T_m^3.
pub const SKIN_EPS: f64 = 0.95;
/// Stored hot fluid's cells farther than this, m, are not summed (the nearest `MOST`).
pub const REACH: f64 = 24.0;
/// A crater's lake nearer than this, m (to its middle), is summed by samples of its columns; farther, as one patch
/// of its whole area at its middle, out to `FAR`.
pub const NEAR: f64 = 64.0;
/// A lake farther than this, m, is not summed.
pub const FAR: f64 = 512.0;
/// At most this many samples of one lake, and of stored cells, each with its sight line walked.
pub const MOST: usize = 256;
/// The sight line's step, m.
const STEP: f64 = 0.5;

/// The temperature felt at a point, C: the air there, the mean radiant temperature, and the operative temperature
/// of the two (`operative`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Felt {
    pub air: f64,
    pub mrt: f64,
    pub temp: f64,
}

/// A hot surface that radiates: its middle, its outward normal, its area (m2), the temperature its surface radiates
/// at (C) and its emissivity.
#[derive(Clone, Copy, Debug)]
pub struct Glow {
    pub at: DVec3,
    pub up: DVec3,
    pub area: f64,
    pub temp: f64,
    pub emissivity: f64,
}

/// The felt temperature at point p among `glows`, `surr` C the surroundings' (the air's; on a body without air the
/// ground's), `air`: whether there is air to carry heat by convection. The mean radiant temperature
/// T_mrt^4 = T_s^4 + sum F_i eps_i (T_i^4 - T_s^4), F_i a small patch's view factor from a globe (ISO 7726)
/// A cos / (4 pi r^2), the sum of F held at or under 1; the operative temperature (h_c T_air + h_r T_mrt) / (h_c + h_r), h_r = 4 eps sigma T_m^3 at
/// the mean of the two (kelvin), h_c `H_C` with air and 0 without: bounded by the hottest source by itself.
pub fn operative(p: DVec3, surr: f64, air: bool, glows: &[Glow]) -> Felt {
    let k = |c: f64| c + 273.15;
    let ts4 = k(surr).powi(4);
    let f: Vec<f64> = glows
        .iter()
        .map(|g| {
            let d = p - g.at;
            // ISO 7726: the mean radiant temperature is a small sphere's (the globe thermometer, a standing body), so a
            // patch's view factor is the share of the whole sphere's view it fills, A cos / (4 pi r^2); a hot surface
            // filling a hemisphere sums to 0.5. (Closer than half a metre counts as half a metre: the law blows up there.)
            let r2 = d.length_squared().max(0.25);
            g.area * d.normalize_or_zero().dot(g.up).max(0.0) / (4.0 * PI * r2)
        })
        .collect();
    let scale = 1.0 / f.iter().sum::<f64>().max(1.0);
    let mrt4 = ts4 + glows.iter().zip(&f).map(|(g, f)| f * scale * g.emissivity * (k(g.temp).powi(4) - ts4)).sum::<f64>();
    let mrt = mrt4.max(0.0).powf(0.25);
    let hr = 4.0 * SKIN_EPS * SIGMA * ((mrt + k(surr)) / 2.0).powi(3);
    let hc = if air { H_C } else { 0.0 };
    let temp = (hc * k(surr) + hr * mrt) / (hc + hr);
    // (+ 0.0: no "-0.00" in the JSON)
    Felt { air: surr, mrt: mrt - 273.15 + 0.0, temp: temp - 273.15 + 0.0 }
}

impl World {
    /// The temperature felt at world point p (an eye). `live`: the weather's air temperature there (the server's
    /// `Air`), None for the baked climate's with the season and the hour.
    /// The air: `Ground::air` at p's cell (above ground the live air; under it the rock's blended in with depth). On a
    /// body without air the surroundings are the ground's temperature at the surface under p (`rest_heat` at the
    /// column's top cell; `liquid::AMBIENT` without a map) and the felt temperature is the mean radiant one.
    pub fn felt(&self, p: DVec3, live: Option<f64>) -> Felt {
        let (bi, _, g) = self.grid_at(p);
        let b = &self.bodies[bi];
        if b.gas || !self.loaded(bi) {
            let air = live.unwrap_or(crate::liquid::AMBIENT as f64);
            return Felt { air, mrt: air, temp: air };
        }
        // shortcut: the air over a hot surface is the cell's own; the plume rising over lava would raise it here, once
        // the gases step moves heat through the air
        let air = self.air_in(self.block_at(p), g.z, live);
        operative(p, air, b.air, &self.glows(bi, p))
    }

    /// The air in cell b at layer z, C (`felt`'s air without the radiant heat).
    fn air_in(&self, b: Block, z: f64, live: Option<f64>) -> f64 {
        let body = &self.bodies[b.body as usize];
        if !body.air {
            let top = body.first_air(body.col_height(b.face, b.pos.x, b.pos.y)) as f64 - 0.5;
            return body.rest_heat(b.face, b.pos.x, b.pos.y, top) as f64;
        }
        let f = move |_: Col, _: f64| live;
        let mut g = self.ground();
        g.air = Some(Air { sky: &self.sky, t: self.time, temp: Some(&f) });
        g.air(World::col(b), z).0
    }

    /// The hot surfaces p can see on body bi: a live crater's lake within `NEAR` by samples of its columns on a
    /// stride (at most about `MOST`, each for stride^2 columns' area), one farther out to `FAR` as one patch of its
    /// whole area at its middle; stored glowing fluid's top cells within `REACH`, the nearest `MOST`. Each radiates at
    /// its row's `surface` where it has one (lava's crust), else at its own temperature: a lake its chamber's heat, a
    /// stored cell its node's heat where the world keeps it (the server's), else its row's pour heat.
    pub fn glows(&self, bi: usize, p: DVec3) -> Vec<Glow> {
        let body = &self.bodies[bi];
        let rows = crate::fluids::FLUIDS.now();
        let col_w = body.radius * FRAC_PI_2 / body.n as f64;
        let eps = |k: u8| rows.get(k).heat.as_ref().map_or(0.95, |h| h.emissivity as f64);
        let surface = |k: u8, own: f64| rows.get(k).heat.as_ref().filter(|h| h.surface != 0.0).map_or(own, |h| h.surface as f64);
        let mut out = Vec::new();
        let lakes: Vec<&Crater> = body.craters().iter().filter(|c| c.lake.is_some()).collect();
        if let Some(lava) = rows.kind("lava") {
            for c in &lakes {
                let r = body.layer_r(c.lake.expect("filtered") as f64);
                let temp = surface(lava, c.heat as f64);
                let mid = body.dir(c.face as usize, c.at.0, c.at.1);
                let dist = (body.centre + mid * r - p).length();
                if dist >= FAR || c.lake_cols.is_empty() {
                    continue;
                }
                let mut lake = Vec::new();
                if dist >= NEAR {
                    lake.push(Glow { at: body.centre + mid * r, up: mid, area: c.lake_cols.len() as f64 * col_w * col_w, temp, emissivity: eps(lava) });
                } else {
                    let stride = (c.lake_cols.len() as f64 / MOST as f64).sqrt().ceil().max(1.0) as i32;
                    for &(i, j) in c.lake_cols.iter().filter(|q| q.0 % stride == 0 && q.1 % stride == 0) {
                        let up = body.dir(c.face as usize, i as f64 + 0.5, j as f64 + 0.5);
                        lake.push(Glow { at: body.centre + up * r, up, area: (stride as f64 * col_w).powi(2), temp, emissivity: eps(lava) });
                    }
                }
                lake.retain(|g| self.clear(p, g.at + g.up * 0.05));
                out.extend(lake);
            }
        }
        if self.water.stored() > 0 {
            let (f, i, j) = body.locate((p - body.centre).normalize_or(DVec3::Y));
            let span = (REACH / col_w).ceil() as i32 + 1;
            let (i, j) = (i.floor() as i32, j.floor() as i32);
            let mut cells = Vec::new();
            // shortcut: stored fluid is looked for on p's face only, as `fluid_lamps`; a seam's other side once crossed
            for (c, z, kind) in self.water.glowing_in(&self.water.wet_tiles(bi as u8, f as u8, (i - span).max(0), (j - span).max(0), 2 * span + 1)) {
                if lakes.iter().any(|l| l.face == c.face() && l.lake_cols.binary_search(&(c.i(), c.j())).is_ok()) {
                    continue;
                }
                let cell = Block { body: c.body(), face: c.face(), pos: IVec3::new(c.i(), c.j(), z) };
                let up = (self.block_centre(cell) - body.centre).normalize();
                // shortcut: a client holds no node's heat (the server's simulation does): its row's pour heat there
                let own = self.water.temp_at(c, z).unwrap_or_else(|| rows.get(kind).heat.as_ref().map_or(crate::liquid::AMBIENT, |h| h.pour) as f64);
                cells.push(Glow { at: self.block_centre(cell) + up * 0.5, up, area: col_w * col_w, temp: surface(kind, own), emissivity: eps(kind) });
            }
            cells.retain(|g| (g.at - p).length_squared() < REACH * REACH);
            cells.sort_by(|a, b| (a.at - p).length_squared().total_cmp(&(b.at - p).length_squared()));
            cells.truncate(MOST);
            cells.retain(|g| self.clear(p, g.at + g.up * 0.05));
            out.extend(cells);
        }
        // (fire, ADR 0191, not built: a burning cell's faces are added here as glows at the flame's heat)
        out
    }

    /// Whether the straight line from a to b passes no solid, opaque block (water, glass, leaves and plants let it
    /// through): a walk in `STEP` steps, each cell once.
    /// shortcut: a wall blocks all radiant heat and passes none by conduction, until blocks' rows carry conductivity,
    /// heat capacity and emissivity; and a step of half a metre can slip past a block's corner.
    pub fn clear(&self, a: DVec3, b: DVec3) -> bool {
        let len = (b - a).length();
        let dir = (b - a) / len.max(1e-9);
        let mut last = None;
        let mut t = 0.0;
        while t < len {
            let cell = self.block_at(a + dir * t);
            if last != Some(cell) {
                #[cfg(test)]
                LOOKUPS.with(|n| n.set(n.get() + 1));
                let m = self.block(cell);
                if BLOCKS.solid(m) && BLOCKS.opaque(m) {
                    return false;
                }
                last = Some(cell);
            }
            t += STEP;
        }
        true
    }

    /// The fluid at world point p, if p is in one (stored, or as generation gives it).
    pub fn fluid_at(&self, p: DVec3) -> Option<u8> {
        let (_, _, g) = self.grid_at(p);
        self.water.kind_at(&self.ground(), World::col(self.block_at(p)), g.z)
    }

    /// Whether cell b lies in the ground as generated under its top cell: the top cell, in the sun and the wind,
    /// reads the air over it, not the rock's year-round mean.
    fn in_ground(&self, b: Block) -> bool {
        let body = &self.bodies[b.body as usize];
        b.pos.z < body.first_air(body.col_height(b.face, b.pos.x, b.pos.y)) - 1
    }

    /// The temperature of the block in cell b, C (the look-at panel): in the ground the rock's (`Body::rest_heat`:
    /// the geotherm, a volcano's heat), over it the air's there; a fluid's frozen block (ice) at most its freeze point.
    pub fn block_temp(&self, b: Block, live: Option<f64>) -> f64 {
        let body = &self.bodies[b.body as usize];
        let t = if self.in_ground(b) { body.rest_heat(b.face, b.pos.x, b.pos.y, b.pos.z as f64 + 0.5) as f64 } else { self.air_in(b, b.pos.z as f64 + 0.5, live) };
        let m = self.block(b);
        let freeze = crate::fluids::FLUIDS.now().rows.iter().filter_map(|r| r.freeze.as_ref()).find(|f| BLOCKS.id(&f.into) == m).map(|f| f.at as f64);
        freeze.map_or(t, |f| t.min(f))
    }

    /// The temperature of fluid `kind` in cell b, C (the look-at panel): its node's heat where the world keeps it
    /// (the server's); water by the place (the rock's in the ground, else the air's), not under its freeze point; a
    /// generated fluid by `rest_heat` (a crater's lake its chamber's heat); other stored fluid its row's pour heat.
    pub fn fluid_temp(&self, b: Block, kind: u8, live: Option<f64>) -> f64 {
        let c = World::col(b);
        if let Some(t) = self.water.temp_at(c, b.pos.z) {
            return t;
        }
        let row = crate::fluids::FLUIDS.now().get(kind).clone();
        let body = &self.bodies[b.body as usize];
        if kind == crate::fluids::WATER {
            let t = if self.in_ground(b) { body.rest_heat(b.face, b.pos.x, b.pos.y, b.pos.z as f64 + 0.5) as f64 } else { self.air_in(b, b.pos.z as f64 + 0.5, live) };
            return row.freeze.as_ref().map_or(t, |f| t.max(f.at as f64));
        }
        if self.water.column(c).is_none() {
            return body.rest_heat(b.face, b.pos.x, b.pos.y, b.pos.z as f64 + 0.5) as f64;
        }
        // shortcut: a client holds no node's heat (the server's simulation does): the row's pour heat
        row.heat.as_ref().map_or(crate::liquid::AMBIENT, |h| h.pour) as f64
    }

    /// How deep fluid `kind` is under its surface over cell b, m: down from the surface to the first cell without
    /// it, 64 cells at most. None for a generated fluid other than water (a crater's lake runs on down its conduit).
    pub fn fluid_depth(&self, b: Block, kind: u8) -> Option<f64> {
        let body = &self.bodies[b.body as usize];
        let c = World::col(b);
        if kind != crate::fluids::WATER && self.water.column(c).is_none() {
            return None;
        }
        let g = self.ground();
        let Some((top, _)) = self.water.fluid_surface_at(&g, c, b.pos.z as f64 + 0.5) else { return Some(0.0) };
        let mut z = b.pos.z;
        while z > 0 && b.pos.z - z < 64 && self.water.kind_at(&g, c, z as f64 - 0.5) == Some(kind) {
            z -= 1;
        }
        Some(body.layer_r(top) - body.layer_r(z as f64))
    }
}

#[cfg(test)]
thread_local! {
    /// Block lookups made by sight lines (`clear`), counted for a test of what one `felt` costs.
    pub(super) static LOOKUPS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn patch(at: DVec3, temp: f64) -> Glow {
        Glow { at, up: DVec3::Z, area: 1.0, temp, emissivity: 0.95 }
    }
    fn near(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    /// The operative temperature by hand (ASHRAE 55): a square metre at 375 C straight under the eye, eps 0.95, in air
    /// at 15 C: F = 0.95 / (4 pi r^2) (a globe's view of it); T_mrt^4 = 288.15^4 + F (648.15^4 - 288.15^4); h_r = 4
    /// 0.95 sigma T_m^3 at the mean of T_mrt and the air's; felt = (3.1 T_air + h_r T_mrt) / (3.1 + h_r): 15.833 C at
    /// 10 m (T_mrt 16.330) and 15.209 C at 20 m (T_mrt 15.334). One beside it (cos 0) adds nothing.
    #[test]
    fn a_patch_warms_by_the_operative_temperature() {
        let at = |r: f64| operative(DVec3::new(0.0, 0.0, r), 15.0, true, &[patch(DVec3::ZERO, 375.0)]);
        assert!(near(at(10.0).temp, 15.833, 0.005) && near(at(10.0).mrt, 16.330, 0.005), "{:?}", at(10.0));
        assert!(near(at(20.0).temp, 15.209, 0.005) && near(at(20.0).mrt, 15.334, 0.005), "{:?}", at(20.0));
        assert!(near(operative(DVec3::new(5.0, 0.0, 0.0), 15.0, true, &[patch(DVec3::ZERO, 375.0)]).temp, 15.0, 1e-9));
    }

    /// However close and however wide the source, the felt temperature never passes the hottest source's: half a
    /// metre over a floor of 41 x 41 square metres at 375 C, with air and without.
    #[test]
    fn felt_never_passes_the_hottest_source() {
        let floor: Vec<Glow> = (-20..=20).flat_map(|x| (-20..=20).map(move |y| patch(DVec3::new(x as f64, y as f64, 0.0), 375.0))).collect();
        for air in [true, false] {
            for h in [0.1, 0.5, 2.0] {
                let f = operative(DVec3::new(0.0, 0.0, h), 15.0, air, &floor);
                assert!(f.temp <= 375.0 && f.mrt <= 375.0 && f.temp > 15.0, "air {air} at {h} m: {f:?}");
            }
        }
    }

    /// In the open air far from any source the felt temperature is the air's: the live air given, and without it
    /// the air rule's own.
    #[test]
    fn open_air_far_from_any_source_feels_as_the_air() {
        let w = World::new(768, crate::bodies::moon_columns(768), 1);
        let b = &w.bodies[0];
        let eye = b.centre + b.spawn * (b.height(b.spawn) + 1.6);
        let f = w.felt(eye, Some(12.5));
        assert!(near(f.temp, 12.5, 1e-9) && near(f.mrt, 12.5, 1e-9), "{f:?}");
        let f = w.felt(eye, None);
        assert!(near(f.temp, w.air_in(w.block_at(eye), w.grid_at(eye).2.z, None), 1e-9));
    }

    /// On a body without air the felt temperature is its ground's at the surface under the point, whatever the
    /// weather says.
    #[test]
    fn on_a_body_without_air_it_is_the_grounds() {
        let w = World::new(768, crate::bodies::moon_columns(768), 1);
        let b = &w.bodies[1];
        assert!(!b.air);
        let eye = b.centre + b.spawn * (b.height(b.spawn) + 1.6);
        let c = w.block_at(eye);
        let want = b.rest_heat(c.face, c.pos.x, c.pos.y, b.first_air(b.col_height(c.face, c.pos.x, c.pos.y)) as f64 - 0.5) as f64;
        assert!(near(w.felt(eye, Some(30.0)).temp, want, 1e-9));
    }

    fn volcano_world() -> World {
        let (seed, map) = crate::bake::volcano::tests::live_seed();
        World::with_grid(768, crate::bodies::moon_columns(768), seed, Some(map), None, 0.0, GENERATION)
    }

    /// From a live crater's lake outward along its face's i, the eye 1.6 m over the ground or, where the rim would
    /// hide the lake, raised until its edge is in sight: hot at the edge, falling with distance, never over the
    /// crust's 375 C; and just behind the rim at the ground, the air's. One `felt` at the edge walks its sight lines
    /// in under 50,000 block lookups.
    #[test]
    fn a_lava_lakes_felt_falls_with_distance_under_its_crust() {
        let w = volcano_world();
        let b = &w.bodies[0];
        let c = b.craters().iter().find(|c| c.lake.is_some()).expect("a lake").clone();
        let ej = c.at.1 as i32;
        let edge = c.lake_cols.iter().filter(|q| q.1 == ej).map(|q| q.0).max().expect("a lake row through its middle");
        let lake_r = b.layer_r(c.lake.unwrap() as f64);
        let edge_at = b.centre + b.dir(c.face as usize, edge as f64 + 0.5, ej as f64 + 0.5) * lake_r;
        let column = |d: i32| {
            let up = b.dir(c.face as usize, (edge + d) as f64 + 0.5, ej as f64 + 0.5);
            (up, b.layer_r(b.first_air(b.col_height(c.face, edge + d, ej)) as f64))
        };
        let in_sight = |d: i32| {
            let (up, ground) = column(d);
            let mut h = 1.6;
            while !w.clear(b.centre + up * (ground + h), edge_at + up * 0.05) {
                h += 1.0;
            }
            (h, w.felt(b.centre + up * (ground + h), Some(15.0)))
        };
        let f: Vec<(i32, f64, Felt)> = [2, 5, 10, 20, 30, 60].into_iter().map(|d| {
            let (h, f) = in_sight(d);
            (d, h, f)
        }).collect();
        eprintln!("felt by the lake in sight (m out, eye over the ground, felt): {f:?}");
        assert!(f.iter().all(|x| x.2.temp <= 375.0 && x.2.mrt <= 375.0), "{f:?}");
        assert!(f.windows(2).all(|p| p[0].2.temp > p[1].2.temp), "falls with distance: {f:?}");
        assert!(f[0].2.temp > 60.0 && f[5].2.temp > 15.0, "{f:?}");
        // just behind the rim, at the ground: the first column out whose ground eye cannot see the edge
        let behind = (1..40).find(|&d| {
            let (up, ground) = column(d);
            !w.clear(b.centre + up * (ground + 1.6), edge_at + up * 0.05)
        });
        if let Some(d) = behind {
            let (up, ground) = column(d);
            eprintln!("just behind the rim, {d} m out: {:?}", w.felt(b.centre + up * (ground + 1.6), Some(15.0)));
        }
        // what one felt at the edge costs in block lookups
        let (up, ground) = column(2);
        LOOKUPS.with(|n| n.set(0));
        w.felt(b.centre + up * (ground + 1.6), Some(15.0));
        let n = LOOKUPS.with(|n| n.get());
        eprintln!("block lookups of one felt at the edge: {n}");
        assert!(n < 50_000, "{n}");
    }

    /// No cliff at the bounds: over the lake's middle, straight up, the felt temperature just inside and just outside
    /// `NEAR` (samples against the lake as one patch) differs by under 2 C; over one stored lava cell, just inside and
    /// outside `REACH`, by under 1 C.
    #[test]
    fn felt_is_continuous_across_the_reaches() {
        let mut w = volcano_world();
        let b = w.bodies[0].clone();
        let c = b.craters().iter().find(|c| c.lake.is_some()).expect("a lake").clone();
        let mid = b.dir(c.face as usize, c.at.0, c.at.1);
        let lake = b.centre + mid * b.layer_r(c.lake.unwrap() as f64);
        let (i, o) = (w.felt(lake + mid * (NEAR - 0.1), Some(15.0)), w.felt(lake + mid * (NEAR + 0.1), Some(15.0)));
        eprintln!("over the lake at {NEAR} m: {i:?} {o:?}");
        assert!(near(i.temp, o.temp, 2.0) && i.temp > 15.1, "{i:?} {o:?}");
        let (cell, _) = lava_pit(&mut w);
        let top = w.block_centre(cell) + (w.block_centre(cell) - b.centre).normalize() * 0.5;
        let up = (top - b.centre).normalize();
        let (i, o) = (w.felt(top + up * (REACH - 0.1), Some(15.0)), w.felt(top + up * (REACH + 0.1), Some(15.0)));
        eprintln!("over a lava cell at {REACH} m: {i:?} {o:?}");
        assert!(near(i.temp, o.temp, 1.0) && i.temp > 15.0, "{i:?} {o:?}");
    }

    /// One stored lava cell in a pit at the spawn's flat open ground (stone round it): the cell and the pit's top layer.
    fn lava_pit(w: &mut World) -> (Block, i32) {
        lava_stream(w, 1)
    }

    /// A stream of stored lava one cell wide and `len` long along j, sunk in flat stone at the spawn with open air over
    /// it: its first cell and the ground's top layer.
    fn lava_stream(w: &mut World, len: i32) -> (Block, i32) {
        let lava = crate::fluids::FLUIDS.now().kind("lava").unwrap();
        let b = w.bodies[0].clone();
        let (f, i, j) = b.locate(b.spawn);
        let (f, i, j) = (f as u8, i as i32, j as i32);
        let top = b.first_air(b.col_height(f, i, j));
        let cell = |di: i32, dj: i32, z: i32| Block { body: 0, face: f, pos: IVec3::new(i + di, j + dj, z) };
        for di in -1..=4 {
            for dj in -1..=len {
                for z in top - 1..top + 4 {
                    w.set_block(cell(di, dj, z), if z < top { BLOCKS.id("stone") } else { AIR });
                }
            }
        }
        for dj in 0..len {
            w.set_block(cell(0, dj, top - 1), AIR);
            let World { bodies, edits, water, sky, time, .. } = &mut *w;
            let g = Solid::at(bodies, edits, sky, *time);
            water.regap(&g, World::col(cell(0, dj, 0)));
            assert!(water.pour_kind(&g, World::col(cell(0, dj, 0)), top - 1, U, lava));
        }
        (cell(0, 0, top - 1), top)
    }

    /// Radiant heat adds up over the hot surface in view. 2 columns beside a lava stream 1 wide and 10 long (this small
    /// planet's columns are 1.085 m, 1.178 m2 a cell), the eye 1.6 m over the ground: by hand, F = sum 1.178 x 1.6 /
    /// (4 pi r^3) over its ten cells (r 2.66 to 4.82 m) = 0.042, T_mrt^4 = 288.15^4 + 0.042 x 0.95 x (648.15^4 -
    /// 288.15^4) so T_mrt 69 C, felt 52 C. 3 m over the middle of a crater's lake the lake fills much of the half of a
    /// globe's view below (F 0.36 of at most 0.5, read back from its T_mrt 231 C): felt 191 C, the still air's h_c
    /// pulling it under the mean radiant temperature, and both far over the stream's.
    #[test]
    fn radiant_heat_adds_up_over_the_hot_surface_in_view() {
        let mut w = volcano_world();
        let b = w.bodies[0].clone();
        let c = b.craters().iter().find(|c| c.lake.is_some()).expect("a lake").clone();
        let mid = b.dir(c.face as usize, c.at.0, c.at.1);
        let lake = w.felt(b.centre + mid * (b.layer_r(c.lake.unwrap() as f64) + 3.0), Some(15.0));
        let (first, top) = lava_stream(&mut w, 10);
        let beside = Block { pos: IVec3::new(first.pos.x + 2, first.pos.y + 4, top), ..first };
        let up = (w.block_centre(beside) - b.centre).normalize();
        let eye = w.block_centre(beside) - up * 0.5 + up * 1.6;
        let stream = w.felt(eye, Some(15.0));
        eprintln!("2 m beside a 1 x 10 lava stream: {stream:?}; 3 m over a crater's lake: {lake:?}");
        assert!(near(stream.temp, 52.0, 3.0), "the stream, by hand 52 C: {stream:?}");
        assert!(near(lake.temp, 191.0, 5.0) && lake.mrt > 3.0 * stream.mrt, "the lake, 191 C at F 0.36: {lake:?}");
    }

    /// 3 m from a lava pit over open ground it reads warm; a stone wall between stops its heat and the air's is felt.
    #[test]
    fn a_stone_block_between_stops_the_lavas_heat() {
        let mut w = volcano_world();
        let (cell, top) = lava_pit(&mut w);
        let b = w.bodies[0].clone();
        let at = |di: i32, z: i32| Block { pos: IVec3::new(cell.pos.x + di, cell.pos.y, z), ..cell };
        let eye = w.block_centre(at(3, top)) + (w.block_centre(at(3, top)) - b.centre).normalize() * 0.6;
        let open = w.felt(eye, Some(15.0));
        assert!(open.temp > 17.0, "3 m from lava in the open: {open:?}");
        for z in top..top + 3 {
            w.set_block(at(1, z), BLOCKS.id("stone"));
        }
        let walled = w.felt(eye, Some(15.0));
        assert!(near(walled.temp, 15.0, 1e-9), "behind stone: {walled:?}");
    }

    /// Down a shaft under the 4 km planet's spawn the air takes the rock's heat (ADR 0197's geotherm, the air rule's
    /// blend): the felt temperature 100 m and 600 m down, deeper warmer.
    #[test]
    fn the_felt_temperature_warms_down_a_shaft() {
        let w = World::with_grid(6144, 32, 1, Some(crate::bake::shared(1, 6144)), None, 0.0, GENERATION);
        let b = &w.bodies[0];
        let d = b.spawn;
        let ground = b.height(d);
        let f: Vec<(f64, Felt)> = [0.0, 100.0, 600.0].into_iter().map(|down| (down, w.felt(b.centre + d * (ground + 1.6 - down), None))).collect();
        eprintln!("felt down the shaft (m down, felt): {f:?}");
        assert!(f[2].1.temp > f[1].1.temp, "{f:?}");
    }
}
