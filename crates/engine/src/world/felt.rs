//! The temperature felt at a point (draft 2026-11-24, "Felt temperature"): the air there, plus the radiant heat of
//! hot fluid in sight within `REACH`, by the view factor of a small patch. And the temperatures the look-at panel
//! shows: a block's and a fluid's at its cell. Read by the client a few times a second, never per block per frame;
//! nothing here changes the simulation.
use super::*;

/// Stefan-Boltzmann's constant, W/m2K4.
pub const SIGMA: f64 = 5.670_374_419e-8;
/// The film coefficient that turns irradiance into a felt rise, W/m2K: a clothed person in still air loses heat
/// at about h_c 3.1 (convection) plus h_r 4.7 (linearised radiation), so h = h_c + h_r ~ 8 (ASHRAE Handbook,
/// Fundamentals, ch. 9 "Thermal comfort", the operative temperature's combined coefficient).
pub const FILM: f64 = 8.0;
/// Sources farther than this, m, are not summed.
pub const REACH: f64 = 24.0;
/// At most this many sources are summed (the nearest), each with its sight line walked.
pub const MOST: usize = 256;
/// The sight line's step, m.
const STEP: f64 = 0.5;

/// The temperature felt at a point, C: the air there, the rise its radiant sources give, and their sum, held under
/// the hottest source in sight (a body is never warmed past what warms it).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Felt {
    pub air: f64,
    pub radiant: f64,
    pub temp: f64,
}

/// A hot surface that radiates: its middle, its outward normal, its area (m2), its temperature (C) and emissivity.
#[derive(Clone, Copy, Debug)]
pub struct Glow {
    pub at: DVec3,
    pub up: DVec3,
    pub area: f64,
    pub temp: f64,
    pub emissivity: f64,
}

/// The felt rise at point p in air at `air` C from `glows`, C: each a small patch's irradiance
/// sigma eps (Ts^4 - Ta^4) A cos / (pi r^2), the sum over `FILM`. A source colder than the air adds nothing.
pub fn radiant(p: DVec3, air: f64, glows: &[Glow]) -> f64 {
    let k4 = |c: f64| (c + 273.15).powi(4);
    let q: f64 = glows
        .iter()
        .map(|g| {
            let d = p - g.at;
            // (a patch closer than half a metre counts as at half a metre: the small-patch law blows up there)
            let r2 = d.length_squared().max(0.25);
            let cos = d.normalize_or_zero().dot(g.up).max(0.0);
            SIGMA * g.emissivity * (k4(g.temp) - k4(air)).max(0.0) * g.area * cos / (PI * r2)
        })
        .sum();
    // (+ 0.0: no "-0.00" in the JSON)
    q / FILM + 0.0
}

impl World {
    /// The temperature felt at world point p (an eye). `live`: the weather's air temperature there (the server's
    /// `Air`), None for the baked climate's with the season and the hour.
    /// The air: `Ground::air` at p's cell (above ground the live air; under it whatever the air rule gives there).
    /// On a body without air (Luna) there is no air: the ground's temperature at the surface under p (`rest_heat` at
    /// the column's top cell, the map's surface mean; `liquid::AMBIENT` without a map), radiant heat on top.
    pub fn felt(&self, p: DVec3, live: Option<f64>) -> Felt {
        let (bi, _, g) = self.grid_at(p);
        let b = &self.bodies[bi];
        if b.gas || !self.loaded(bi) {
            let air = live.unwrap_or(crate::liquid::AMBIENT as f64);
            return Felt { air, radiant: 0.0, temp: air };
        }
        let cell = self.block_at(p);
        let air = self.air_in(cell, g.z, live);
        let glows = self.glows(bi, p);
        let radiant = radiant(p, air, &glows);
        let hottest = glows.iter().map(|g| g.temp).fold(air, f64::max);
        Felt { air, radiant, temp: (air + radiant).min(hottest) }
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

    /// The hot surfaces within `REACH` of p on body bi that p can see, the nearest `MOST`: a live crater's lake (a
    /// patch a column at its chamber's heat) and stored glowing fluid's top cells (its node's heat where the world
    /// keeps it, the server's; else its row's pour heat).
    pub fn glows(&self, bi: usize, p: DVec3) -> Vec<Glow> {
        let body = &self.bodies[bi];
        let rows = crate::fluids::FLUIDS.now();
        let col_w = body.radius * FRAC_PI_2 / body.n as f64;
        let eps = |k: u8| rows.get(k).heat.as_ref().map_or(0.95, |h| h.emissivity as f64);
        let mut out = Vec::new();
        let lakes: Vec<&Crater> = body.craters().iter().filter(|c| c.lake.is_some()).collect();
        if let Some(lava) = rows.kind("lava") {
            for c in &lakes {
                // the whole lake out of reach: none of its columns looked at
                if (body.centre + c.top - p).length() > REACH + c.v.crater as f64 * col_w + 8.0 {
                    continue;
                }
                let r = body.layer_r(c.lake.expect("filtered") as f64);
                for &(i, j) in &c.lake_cols {
                    let up = body.dir(c.face as usize, i as f64 + 0.5, j as f64 + 0.5);
                    out.push(Glow { at: body.centre + up * r, up, area: col_w * col_w, temp: c.heat as f64, emissivity: eps(lava) });
                }
            }
        }
        if self.water.stored() > 0 {
            let (f, i, j) = body.locate((p - body.centre).normalize_or(DVec3::Y));
            let span = (REACH / col_w).ceil() as i32 + 1;
            let (i, j) = (i.floor() as i32, j.floor() as i32);
            // shortcut: stored fluid is looked for on p's face only, as `fluid_lamps`; a seam's other side once crossed
            for (c, z, kind) in self.water.glowing_in(&self.water.wet_tiles(bi as u8, f as u8, (i - span).max(0), (j - span).max(0), 2 * span + 1)) {
                if lakes.iter().any(|l| l.face == c.face() && l.lake_cols.binary_search(&(c.i(), c.j())).is_ok()) {
                    continue;
                }
                let cell = Block { body: c.body(), face: c.face(), pos: IVec3::new(c.i(), c.j(), z) };
                let up = (self.block_centre(cell) - body.centre).normalize();
                // shortcut: a client holds no node's heat (the server's simulation does): its row's pour heat there
                let temp = self.water.temp_at(c, z).unwrap_or_else(|| rows.get(kind).heat.as_ref().map_or(crate::liquid::AMBIENT, |h| h.pour) as f64);
                out.push(Glow { at: self.block_centre(cell) + up * 0.5, up, area: col_w * col_w, temp, emissivity: eps(kind) });
            }
        }
        // (fire, ADR 0191, not built: a burning cell's faces are added here as glows at the flame's heat)
        out.retain(|g| (g.at - p).length_squared() < REACH * REACH);
        out.sort_by(|a, b| (a.at - p).length_squared().total_cmp(&(b.at - p).length_squared()));
        out.truncate(MOST);
        out.retain(|g| self.clear(p, g.at + g.up * 0.05));
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
    /// it, 64 cells at most.
    pub fn fluid_depth(&self, b: Block, kind: u8) -> f64 {
        let body = &self.bodies[b.body as usize];
        let c = World::col(b);
        let g = self.ground();
        let Some((top, _)) = self.water.fluid_surface_at(&g, c, b.pos.z as f64 + 0.5) else { return 0.0 };
        let mut z = b.pos.z;
        while z > 0 && b.pos.z - z < 64 && self.water.kind_at(&g, c, z as f64 - 0.5) == Some(kind) {
            z -= 1;
        }
        body.layer_r(top) - body.layer_r(z as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn patch(at: DVec3) -> Glow {
        Glow { at, up: DVec3::Z, area: 1.0, temp: 1150.0, emissivity: 0.95 }
    }

    /// The law (by hand: sigma 0.95 (1423.15^4 - 288.15^4) / (pi r^2) / 8): a square metre of lava at 1150 C
    /// straight under the eye warms it 87.77 C at 10 m and 21.94 C at 20 m in air at 15 C; one beside it (cos 0)
    /// adds nothing.
    #[test]
    fn a_patch_warms_by_the_view_factor_law() {
        let at = |r: f64| radiant(DVec3::new(0.0, 0.0, r), 15.0, &[patch(DVec3::ZERO)]);
        assert!((at(10.0) - 87.774).abs() < 0.01, "{}", at(10.0));
        assert!((at(20.0) - 21.944).abs() < 0.01, "{}", at(20.0));
        assert_eq!(radiant(DVec3::new(5.0, 0.0, 0.0), 15.0, &[patch(DVec3::ZERO)]), 0.0);
    }

    /// In the open air far from any source the felt temperature is the air's: the live air given, and without it
    /// the air rule's own.
    #[test]
    fn open_air_far_from_any_source_feels_as_the_air() {
        let w = World::new(768, crate::bodies::moon_columns(768), 1);
        let b = &w.bodies[0];
        let eye = b.centre + b.spawn * (b.height(b.spawn) + 1.6);
        let f = w.felt(eye, Some(12.5));
        assert_eq!((f.temp, f.radiant), (12.5, 0.0), "{f:?}");
        let f = w.felt(eye, None);
        assert_eq!(f.temp, w.air_in(w.block_at(eye), w.grid_at(eye).2.z, None));
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
        assert_eq!(w.felt(eye, Some(30.0)).temp, want);
    }

    fn volcano_world() -> World {
        let (seed, map) = crate::bake::volcano::tests::live_seed();
        World::with_grid(768, crate::bodies::moon_columns(768), seed, Some(map), None, 0.0, GENERATION)
    }

    /// Beside a live crater's lake it is dangerously hot and it falls with distance: an eye 1.6 m over the ground
    /// 2, 10 and 30 columns out from the lake's edge along its face's i.
    #[test]
    fn a_lava_lakes_edge_is_dangerously_hot_and_cools_with_distance() {
        let w = volcano_world();
        let b = &w.bodies[0];
        let c = b.craters().iter().find(|c| c.lake.is_some()).expect("a lake").clone();
        let (_, ej) = (c.at.0 as i32, c.at.1 as i32);
        let edge = c.lake_cols.iter().filter(|q| q.1 == ej).map(|q| q.0).max().expect("a lake row through its middle");
        let at = |d: i32| {
            let up = b.dir(c.face as usize, (edge + d) as f64 + 0.5, ej as f64 + 0.5);
            let top = b.first_air(b.col_height(c.face, edge + d, ej)) as f64;
            w.felt(b.centre + up * (b.layer_r(top) + 1.6), Some(15.0))
        };
        let f: Vec<(i32, f64, f64)> = (2..=16).chain([30]).map(|d| (d, at(d).temp, at(d).radiant)).collect();
        eprintln!("felt by the lake (columns out, C, rise): {f:?}");
        let of = |d: i32| f.iter().find(|x| x.0 == d).unwrap();
        // at the edge the law's rise passes the lake's own heat: held there; it falls going out over the rim
        assert!(of(2).1 > 1000.0 && of(2).2 > of(10).2 && of(10).2 > 0.0, "{f:?}");
        // behind the rim (the crater's wall in the way) and past the reach nothing is summed
        assert_eq!(of(30).1, 15.0, "{f:?}");
    }

    /// A pit of lava at the spawn's cold ground: 3 m from it over open ground it reads hot; a stone wall between
    /// stops its heat and the air's temperature is felt.
    #[test]
    fn a_stone_block_between_stops_the_lavas_heat() {
        let mut w = volcano_world();
        let lava = crate::fluids::FLUIDS.now().kind("lava").unwrap();
        let b = w.bodies[0].clone();
        let (f, i, j) = b.locate(b.spawn);
        let (f, i, j) = (f as u8, i as i32, j as i32);
        let top = b.first_air(b.col_height(f, i, j));
        let cell = |di: i32, z: i32| Block { body: 0, face: f, pos: IVec3::new(i + di, j, z) };
        // flat open ground round it: air from the top layer up, the pit's column one deeper
        for di in -1..=4 {
            for z in top - 1..top + 4 {
                w.set_block(cell(di, z), if z < top { BLOCKS.id("stone") } else { AIR });
            }
        }
        w.set_block(cell(0, top - 1), AIR);
        {
            let World { bodies, edits, water, sky, time, .. } = &mut w;
            let g = Solid::at(bodies, edits, sky, *time);
            water.regap(&g, World::col(cell(0, 0)));
            assert!(water.pour_kind(&g, World::col(cell(0, 0)), top - 1, U, lava));
        }
        let eye = w.block_centre(cell(3, top)) + (w.block_centre(cell(3, top)) - b.centre).normalize() * 0.6;
        let open = w.felt(eye, Some(15.0));
        assert!(open.temp > 30.0, "3 m from lava in the open: {open:?}");
        for z in top..top + 3 {
            w.set_block(cell(1, z), BLOCKS.id("stone"));
        }
        let walled = w.felt(eye, Some(15.0));
        assert_eq!(walled.temp, 15.0, "behind stone: {walled:?}");
    }
}
