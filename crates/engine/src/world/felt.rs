//! The temperature felt at a point (draft 2026-11-24, "Felt temperature"): the operative temperature of ASHRAE 55 and
//! ISO 7726, the air's and the mean radiant temperature's mean weighted by how a person loses heat to each; the mean
//! radiant temperature from what a fixed set of rays from the eye meets. And the temperatures the look-at panel shows:
//! a block's and a fluid's at its cell. Read by the client a few rays a frame, never in one frame; nothing here changes
//! the simulation.
use super::*;

/// Stefan-Boltzmann's constant, W/m2K4.
pub const SIGMA: f64 = 5.670_374_419e-8;
/// A person's convective coefficient in still air, W/m2K (ASHRAE Handbook, Fundamentals, ch. 9 "Thermal comfort":
/// h_c = 3.1 for a person at rest in air under 0.2 m/s). None on a body without air.
pub const H_C: f64 = 3.1;
/// A clothed person's emissivity (ASHRAE Fundamentals ch. 9: 0.95), in h_r = 4 eps sigma T_m^3.
pub const SKIN_EPS: f64 = 0.95;
/// The rays from the eye: directions evenly over the sphere, each 1/`RAYS` of a globe's view.
pub const RAYS: usize = 384;
/// How far a ray looks, m.
/// shortcut: a hot surface farther than this is not seen: a crater's lake beyond 48 m adds about 2 C (the lake rule
/// before rays read 17 C at 60 m from a lake's edge in air at 15 C); a longer reach or a coarser far march when a
/// glowing sight farther off matters.
pub const RAY_REACH: f64 = 48.0;
/// The cells one ray looks at, at most (48 m across the grid's three axes takes up to 83).
pub const RAY_CELLS: u64 = 96;
/// The cells the client's rays look at in one frame, at most (`World::march`): a full set takes a few dozen frames.
pub const FRAME_PROBES: u64 = 400;

/// The temperature felt at a point, C: the air there, the mean radiant temperature, and the operative temperature
/// of the two (`operative`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Felt {
    pub air: f64,
    pub mrt: f64,
    pub temp: f64,
}

/// A set of rays marched from an eye, whole or begun (`World::march`): how many, the sum of the emissivities of the
/// hot surfaces they met and of each emissivity times its temperature^4 (kelvin), and the cells looked at.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rays {
    pub done: usize,
    pub eps: f64,
    pub eps_t4: f64,
    pub probes: u64,
}

impl Rays {
    /// The mean radiant temperature, C, of a whole set in surroundings at `surr` C (ISO 7726, a globe's): each ray is
    /// 1/N of the sphere's view, so T_mrt^4 = T_s^4 + (1/N) sum over hits of eps (T_hit^4 - T_s^4). A ray that meets
    /// a solid block or nothing sees the surroundings. View factors, what hides what, a hemisphere's 0.5 and a sum
    /// at or under 1 all follow from the count.
    pub fn mrt(&self, surr: f64) -> f64 {
        let ts4 = (surr + 273.15).powi(4);
        let n = RAYS as f64;
        (ts4 * (1.0 - self.eps / n) + self.eps_t4 / n).max(0.0).powf(0.25) - 273.15
    }
}

/// Ray k's direction: a Fibonacci lattice over the sphere, fixed in the world's frame (a still eye's value is
/// still).
pub fn ray_dir(k: usize) -> DVec3 {
    let z = 1.0 - (2.0 * k as f64 + 1.0) / RAYS as f64;
    let r = (1.0 - z * z).max(0.0).sqrt();
    let a = k as f64 * PI * (3.0 - 5f64.sqrt());
    DVec3::new(r * a.cos(), r * a.sin(), z)
}

/// The operative temperature, C, in air at `surr` C with mean radiant temperature `mrt` C, `air`: whether there is
/// air to carry heat by convection: (h_c T_air + h_r T_mrt) / (h_c + h_r), h_r = 4 eps sigma T_m^3 at the mean of the
/// two (kelvin), h_c `H_C` with air and 0 without (then it is the mean radiant temperature).
pub fn operative(surr: f64, air: bool, mrt: f64) -> Felt {
    let (ts, tm) = (surr + 273.15, mrt + 273.15);
    let hr = 4.0 * SKIN_EPS * SIGMA * ((tm + ts) / 2.0).powi(3);
    let hc = if air { H_C } else { 0.0 };
    let temp = (hc * ts + hr * tm) / (hc + hr);
    // (+ 0.0: no "-0.00" in the JSON)
    Felt { air: surr, mrt: mrt + 0.0, temp: temp - 273.15 + 0.0 }
}

impl World {
    /// The temperature felt at world point p (an eye), its rays all marched at once (`felt_of`): for a run's last
    /// line, a bot's report and tests; the client marches them a few a frame. `live`: the weather's air temperature
    /// there (the server's `Air`), None for the baked climate's with the season and the hour.
    pub fn felt(&self, p: DVec3, live: Option<f64>) -> Felt {
        let mut r = Rays::default();
        self.march(p, &mut r, u64::MAX);
        self.felt_of(p, live, &r)
    }

    /// The felt temperature at p from a set of rays marched from about there (none met: the air's). In a glowing
    /// fluid (lava) it is the fluid's own temperature, not its crust's. Else the air: `Ground::air` at p's cell (above
    /// ground the live air; under it the rock's blended in with depth); on a body without air the surroundings are the
    /// ground's temperature at the surface under p (`rest_heat` at the column's top cell; `liquid::AMBIENT` without a
    /// map) and the felt temperature is the mean radiant one.
    pub fn felt_of(&self, p: DVec3, live: Option<f64>, r: &Rays) -> Felt {
        let (bi, _, g) = self.grid_at(p);
        let b = &self.bodies[bi];
        if b.gas || !self.loaded(bi) {
            let air = live.unwrap_or(crate::liquid::AMBIENT as f64);
            return Felt { air, mrt: air, temp: air };
        }
        let cell = self.block_at(p);
        if let Some(k) = self.fluid_at(p).filter(|&k| crate::fluids::FLUIDS.now().get(k).glows()) {
            let t = self.fluid_temp(cell, k, live);
            return Felt { air: t, mrt: t, temp: t };
        }
        // shortcut: the air over a hot surface is the cell's own; the plume rising over lava would raise it here, once
        // the gases step moves heat through the air
        let air = self.air_in(cell, g.z, live);
        operative(air, b.air, r.mrt(air))
    }

    /// Marches set r's next rays from p, whole rays only, while the cells looked at in this call stay within
    /// `budget` (`FRAME_PROBES` a frame for the client, `u64::MAX` for all at once).
    pub fn march(&self, p: DVec3, r: &mut Rays, budget: u64) {
        let bi = self.nearest(p);
        if self.bodies[bi].gas || !self.loaded(bi) {
            r.done = RAYS;
            return;
        }
        let g = self.ground();
        let rows = crate::fluids::FLUIDS.now();
        let start = r.probes;
        while r.done < RAYS && (r.probes - start).saturating_add(RAY_CELLS) <= budget {
            if let Some((t, eps)) = self.ray(&g, rows, p, ray_dir(r.done), &mut r.probes) {
                r.eps += eps;
                r.eps_t4 += eps * (t + 273.15).powi(4);
            }
            r.done += 1;
        }
    }

    /// What one ray from p along dir meets within `RAY_REACH`: the temperature (C) and emissivity of the first
    /// glowing fluid's surface, or None (a solid, opaque block first, or nothing). Cell to cell: the grid is taken as
    /// straight over a centimetre to find the nearest wall the ray crosses, so each cell is looked at once. Water,
    /// glass, leaves and plants let it through.
    /// shortcut: a wall blocks all radiant heat and passes none by conduction, until blocks' rows carry conductivity,
    /// heat capacity and emissivity.
    fn ray(&self, g: &Solid<'_>, rows: &crate::fluids::Fluids, p: DVec3, dir: DVec3, probes: &mut u64) -> Option<(f64, f64)> {
        let mut cell = self.block_at(p);
        let (mut t, mut looked, mut steps) = (0.0, 0, 0);
        while t < RAY_REACH && looked < RAY_CELLS && steps < 4 * RAY_CELLS {
            steps += 1;
            let (_, f0, g0) = self.grid_at(p + dir * t);
            let (_, f1, g1) = self.grid_at(p + dir * (t + 0.01));
            let wall = if f0 == f1 {
                let d = (g1 - g0) / 0.01;
                let exit = |g: f64, d: f64| if d > 1e-9 { (g.floor() + 1.0 - g) / d } else if d < -1e-9 { (g - g.floor()) / -d } else { f64::INFINITY };
                exit(g0.x, d.x).min(exit(g0.y, d.y)).min(exit(g0.z, d.z)).clamp(0.0, 2.0)
            } else {
                0.0
            };
            t += wall + 1e-3;
            let next = self.block_at(p + dir * t);
            if next == cell {
                continue;
            }
            cell = next;
            looked += 1;
            *probes += 1;
            let m = self.block(cell);
            if BLOCKS.solid(m) {
                if BLOCKS.opaque(m) {
                    return None;
                }
                continue;
            }
            // (any fluid in the cell: the bottom of it, so a cell part full counts)
            let Some(k) = self.water.kind_at(g, World::col(cell), cell.pos.z as f64 + 0.05) else { continue };
            let row = rows.get(k);
            if !row.glows() {
                continue;
            }
            let heat = row.heat.as_ref();
            let temp = heat.filter(|h| h.surface != 0.0).map_or_else(|| self.fluid_temp(cell, k, None), |h| h.surface as f64);
            return Some((temp, heat.map_or(0.95, |h| h.emissivity as f64)));
        }
        None
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

    /// Whether rain or snow falling at world point p boils off (ADR 0205, hot ground): p is in a
    /// glowing fluid (lava, stored or generated) or over lava lying on the open ground of its column, so none lands
    /// on lava, or anywhere over a live crater's lake, however high, so none falls through the hot air over it.
    /// The lake by the lava that is there now, at its
    /// surface's level in p's column (none where it was built over or is gone), and by p's distance from the
    /// crater's middle (so over a face's seam too).
    /// shortcut: lava alone; rock near a chamber reads over 100 C at its top cell while the air on it is cool, so
    /// the ground's own heat is not asked until the heat field takes the surface as a boundary (`volcano::BARE`).
    pub fn boils_rain(&self, p: DVec3) -> bool {
        let bi = self.block_at(p).body as usize;
        let b = &self.bodies[bi];
        let d = (p - b.centre).normalize();
        let glows = |q: DVec3| self.fluid_at(q).is_some_and(|k| crate::fluids::FLUIDS.now().get(k).glows());
        // a live crater's lake: by the lava there now at its surface's level, whatever lies under a roof built on it
        let near = |k: &&Crater| k.lake.is_some() && (d - k.top.normalize()).length() * k.top.length() < k.v.crater as f64 + 1.0;
        if let Some(k) = b.craters().iter().find(near) {
            let at = b.centre + d * b.layer_r(k.lake.expect("found with one") as f64 - 0.5);
            return glows(p) || !BLOCKS.solid(self.block(self.block_at(at))) && glows(at);
        }
        // elsewhere: in lava, or over lava lying on the ground of p's column (a drop made high over it is not yet
        // in the lava it will land in). shortcut: the ground as `air_over_ground` finds it from the terrain up, so
        // lava under a roof someone built keeps the rain off the roof; from the sky down when roofs over lava are built
        glows(p) || glows(b.centre + d * (b.layer_r(self.air_over_ground(bi, d).pos.z as f64) + 0.02))
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
mod tests {
    use super::*;

    /// The rays see a hot disk by its solid angle (the patch law, ISO 7726's globe: F = Omega / 4 pi, for a disk of
    /// radius R at h, (1 - h / sqrt(h^2 + R^2)) / 2): the share of the 384 directions that meet it, facing any way, is
    /// within 5 rays of that (the lattice is even to about 1 % of the view: 135 rays where 131.3 are owed is the worst
    /// here, a disk round the X axis); a whole half space is 192 within 3. (A square metre at 10 m is 1 / (4 pi 100)
    /// of the view, a third of one ray: one patch is not a test of rays, a disk is.)
    #[test]
    fn the_rays_see_a_disk_by_its_solid_angle() {
        let n = RAYS as f64;
        for up in [DVec3::X, DVec3::Y, DVec3::Z, DVec3::ONE.normalize(), DVec3::new(-0.3, 0.8, 0.52).normalize()] {
            let half = (0..RAYS).filter(|&k| ray_dir(k).dot(up) < 0.0).count();
            assert!(half.abs_diff(RAYS / 2) <= 3, "{up}: {half}");
            for (h, r) in [(1.0, 1.0), (1.0, 3.0), (2.0, 10.0), (3.0, 1.5), (0.5, 50.0)] {
                let hits = (0..RAYS)
                    .filter(|&k| {
                        let d = ray_dir(k);
                        let c = -d.dot(up);
                        c > 0.0 && (d * (h / c) + up * h).length() <= r
                    })
                    .count() as f64;
                let want = (1.0 - h / (h * h + r * r).sqrt()) / 2.0;
                assert!((hits / n - want).abs() <= 5.0 / n,"{up}, h {h} r {r}: {hits} rays, {} by hand", want * n);
            }
        }
    }

    /// The mean radiant and operative temperatures by hand (ASHRAE 55): half the rays meeting lava's crust at 375 C,
    /// eps 0.95, in air at 15 C: T_mrt^4 = 288.15^4 + 0.5 x 0.95 x (648.15^4 - 288.15^4), 270.649 C; h_r = 4 x 0.95 sigma
    /// T_m^3 at the mean of T_mrt and the air's, felt (3.1 T_air + h_r T_mrt) / (3.1 + h_r) = 228.062 C. All of them:
    /// 367.070 and 322.796 C, under the crust's 375; without air felt is the mean radiant. None: the air's.
    #[test]
    fn the_mean_radiant_and_operative_temperatures_by_hand() {
        let set = |hits: usize| Rays { done: RAYS, eps: 0.95 * hits as f64, eps_t4: 0.95 * hits as f64 * 648.15f64.powi(4), probes: 0 };
        let (half, all) = (set(RAYS / 2), set(RAYS));
        assert!(near(half.mrt(15.0), 270.649, 0.005) && near(operative(15.0, true, half.mrt(15.0)).temp, 228.062, 0.005), "{}", half.mrt(15.0));
        assert!(near(all.mrt(15.0), 367.070, 0.005) && near(operative(15.0, true, all.mrt(15.0)).temp, 322.796, 0.005));
        assert!(near(operative(15.0, false, all.mrt(15.0)).temp, 367.070, 0.005));
        assert!(all.mrt(15.0) < 375.0);
        assert!(near(set(0).mrt(15.0), 15.0, 1e-9) && near(operative(15.0, true, 15.0).temp, 15.0, 1e-9));
    }

    /// The client's rays (`march`, `FRAME_PROBES` a frame) at a lava lake's edge and in the open look at no more
    /// cells in a frame than the cap, finish a set in under 100 frames, and give what one march of all of them gives.
    #[test]
    fn the_client_marches_a_set_in_frames_under_the_cap() {
        let w = volcano_world();
        let b = &w.bodies[0];
        let c = b.craters().iter().find(|c| c.lake.is_some()).expect("a lake").clone();
        let ej = c.at.1 as i32;
        let edge = c.lake_cols.iter().filter(|q| q.1 == ej).map(|q| q.0).max().unwrap() + 1;
        let up = b.dir(c.face as usize, edge as f64 + 0.5, ej as f64 + 0.5);
        let shore = b.centre + up * (b.layer_r(b.first_air(b.col_height(c.face, edge, ej)) as f64) + 1.6);
        let open = b.centre + b.spawn * (b.height(b.spawn) + 1.6);
        for p in [shore, open] {
            let mut r = Rays::default();
            let (mut frames, mut most) = (0, 0);
            while r.done < RAYS {
                let before = r.probes;
                w.march(p, &mut r, FRAME_PROBES);
                most = most.max(r.probes - before);
                frames += 1;
            }
            let mut all = Rays::default();
            w.march(p, &mut all, u64::MAX);
            eprintln!("a set of rays: {frames} frames, at most {most} cells a frame, {} in all, {} of 384 met lava", r.probes, r.eps / 0.95);
            assert!(most <= FRAME_PROBES && frames < 100, "{frames} frames, {most} a frame");
            assert_eq!(r, all);
        }
    }

    fn near(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
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

    /// Ground made for lava round the spawn of w's home: `pad` columns about the rectangle of offsets `is` x `js`
    /// from the spawn's column cleared to stone three cells deep under the spawn column's top layer and open air
    /// eight layers over it, then stored lava poured one cell deep in the rectangle. Returns the spawn's column and
    /// that top layer.
    fn lava(w: &mut World, is: std::ops::Range<i32>, js: std::ops::Range<i32>, pad: i32) -> (Block, i32) {
        let kind = crate::fluids::FLUIDS.now().kind("lava").unwrap();
        let b = w.bodies[0].clone();
        let (f, i, j) = b.locate(b.spawn);
        let (f, i, j) = (f as u8, i as i32, j as i32);
        let top = b.first_air(b.col_height(f, i, j));
        let cell = |di: i32, dj: i32, z: i32| Block { body: 0, face: f, pos: IVec3::new(i + di, j + dj, z) };
        for di in is.start - pad..is.end + pad {
            for dj in js.start - pad..js.end + pad {
                for z in top - 3..top + 8 {
                    let lake = is.contains(&di) && js.contains(&dj) && z == top - 1;
                    w.set_block(cell(di, dj, z), if z < top && !lake { BLOCKS.id("stone") } else { AIR });
                }
            }
        }
        let World { bodies, edits, water, sky, time, .. } = w;
        let g = Solid::at(bodies, edits, sky, *time);
        for di in is.clone() {
            for dj in js.clone() {
                let c = World::col(cell(di, dj, 0));
                water.regap(&g, c);
                assert!(water.pour_kind(&g, c, top - 1, U, kind));
            }
        }
        (cell(0, 0, top), top)
    }

    /// The eye `h` m over the ground (the bottom of layer `top`) at the column `di`, `dj` from `at`'s.
    fn eye(w: &World, at: Block, di: i32, dj: i32, h: f64) -> DVec3 {
        let c = Block { pos: IVec3::new(at.pos.x + di, at.pos.y + dj, at.pos.z), ..at };
        let up = (w.block_centre(c) - w.bodies[0].centre).normalize();
        w.block_centre(c) - up * 0.5 + up * h
    }

    fn lava_world() -> World {
        World::new(768, crate::bodies::moon_columns(768), 1)
    }

    /// Over a floor of lava filling the whole lower half of the view (104 x 104 m, the eye half a metre over its
    /// middle) the mean radiant temperature is the hemisphere's by hand, F 0.5 at the crust's 375 C, eps 0.95:
    /// T_mrt^4 = 288.15^4 + 0.5 x 0.95 x (648.15^4 - 288.15^4), 270.6 C; within a few percent (the planet's curve
    /// takes the floor's far edge under the rays nearest the horizon) and never over it.
    #[test]
    fn a_lava_floor_under_the_whole_view_gives_the_hemispheres_heat() {
        let mut w = lava_world();
        let (at, _) = lava(&mut w, -52..52, -52..52, 2);
        let f = w.felt(eye(&w, at, 0, 0, 0.5), Some(15.0));
        eprintln!("half a metre over a 104 m floor of lava: {f:?} (the hemisphere by hand: 270.6 C)");
        assert!(f.mrt <= 270.65 && f.mrt > 0.95 * 270.65, "{f:?}");
    }

    /// Along the shore of a floor of lava wider than 100 m, the eye 1.6 m over the stone one column out, ten points a
    /// metre apart read hot (not the air's) and within 10 % of each other.
    #[test]
    fn the_shore_of_a_wide_lava_floor_is_hot_and_even() {
        let mut w = lava_world();
        let (at, _) = lava(&mut w, -52..52, -52..52, 2);
        let f: Vec<f64> = (-5..5).map(|dj| w.felt(eye(&w, at, 52, dj, 1.6), Some(15.0)).temp).collect();
        let (lo, hi) = (f.iter().cloned().fold(f64::MAX, f64::min), f.iter().cloned().fold(f64::MIN, f64::max));
        let mean = f.iter().sum::<f64>() / f.len() as f64;
        eprintln!("the shore of a 104 m floor of lava, 1.6 m up, ten points a metre apart: {f:.1?}");
        assert!(lo > 60.0, "hot at the shore: {f:?}");
        assert!(hi - lo < 0.1 * mean, "even along the shore: {f:?}");
    }

    /// An eye in lava feels the lava's own temperature (its pour heat on a client), not its crust's or the air's.
    #[test]
    fn an_eye_in_lava_feels_the_lava() {
        let mut w = lava_world();
        let (at, top) = lava(&mut w, -1..2, -1..2, 2);
        let pour = crate::fluids::FLUIDS.now().get(crate::fluids::FLUIDS.now().kind("lava").unwrap()).heat.as_ref().unwrap().pour as f64;
        let inside = w.block_centre(Block { pos: IVec3::new(at.pos.x, at.pos.y, top - 1), ..at });
        let f = w.felt(inside, Some(15.0));
        assert!(near(f.temp, pour, 1e-9) && near(f.mrt, pour, 1e-9), "{f:?}");
    }

    /// Radiant heat adds up over the hot surface in view. 2 columns beside a lava stream 1 wide and 10 long (this small
    /// planet's columns are about 1.085 m, 1.178 m2 a cell), the eye 1.6 m over the ground: by hand, F = sum 1.178 x 1.6
    /// / (4 pi r^3) over its ten cells (r 2.66 to 4.82 m) = 0.042, T_mrt^4 = 288.15^4 + 0.042 x 0.95 x (648.15^4 -
    /// 288.15^4) so T_mrt 69 C, felt 52 C; by rays 0.042 is 16 of 384, a ray each 2 to 3 C, so within 10 C at the
    /// stream's side, and the eye one column either way along it about the same.
    #[test]
    fn radiant_heat_adds_up_over_the_hot_surface_in_view() {
        let mut w = lava_world();
        let (at, _) = lava(&mut w, 0..1, 0..10, 4);
        let f: Vec<Felt> = (3..7).map(|dj| w.felt(eye(&w, at, 2, dj, 1.6), Some(15.0))).collect();
        eprintln!("2 columns beside a 1 x 10 lava stream, the eye along it at 3..7: {f:?}");
        assert!(f.iter().all(|f| near(f.temp, 52.0, 10.0)), "the stream, by hand 52 C: {f:?}");
    }

    /// A lava pool 4 x 6 seen from 2 columns off reads warm; a stone wall 4 high between stops its heat and the air's
    /// is felt.
    #[test]
    fn a_stone_block_between_stops_the_lavas_heat() {
        let mut w = lava_world();
        let (at, top) = lava(&mut w, -4..0, -3..3, 4);
        let p = eye(&w, at, 2, 0, 1.6);
        let open = w.felt(p, Some(15.0));
        assert!(open.temp > 20.0, "2 columns from lava in the open: {open:?}");
        for dj in -6..6 {
            for z in top..top + 4 {
                w.set_block(Block { pos: IVec3::new(at.pos.x + 1, at.pos.y + dj, z), ..at }, BLOCKS.id("stone"));
            }
        }
        let walled = w.felt(p, Some(15.0));
        eprintln!("by a lava pool: open {open:?}, walled {walled:?}");
        assert!(near(walled.temp, 15.0, 1e-9), "behind stone: {walled:?}");
    }

    /// Down a shaft dug under the small planet's spawn the air takes the rock's heat by the air rule's blend, by hand:
    /// T = T_open + (T_rock - T_open)(1 - exp(-depth / 30 m)), the rock's `Body::heat` at the eye's layer, the open
    /// air 30 C (live); nothing hot in sight, so felt is that air. 10 m and 40 m down.
    #[test]
    fn the_felt_temperature_takes_the_rocks_heat_down_a_shaft() {
        let mut w = volcano_world();
        let b = w.bodies[0].clone();
        let (f, i, j) = b.locate(b.spawn);
        let (f, i, j) = (f as u8, i as i32, j as i32);
        let top = b.first_air(b.col_height(f, i, j));
        for z in top - 45..top {
            w.set_block(Block { body: 0, face: f, pos: IVec3::new(i, j, z) }, AIR);
        }
        let felt: Vec<(f64, f64, f64)> = [10.0, 40.0]
            .into_iter()
            .map(|down| {
                let p = b.centre + b.dir(f as usize, i as f64 + 0.5, j as f64 + 0.5) * (b.col_height(f, i, j) - down);
                let z = w.grid_at(p).2.z;
                let rock = b.heat(f, i, j, z).expect("a baked map's rock heat") as f64;
                let hand = 30.0 + (rock - 30.0) * (1.0 - (-down / 30.0f64).exp());
                (down, hand, w.felt(p, Some(30.0)).temp)
            })
            .collect();
        eprintln!("down a dug shaft (m down, by hand, felt): {felt:?}");
        assert!(felt.iter().all(|x| near(x.1, x.2, 0.05)), "{felt:?}");
        assert!((felt[1].2 - 30.0).abs() > 1.0, "40 m down is not the open air's: {felt:?}");
    }

    /// From a live crater's lake outward along its face's i, the eye 1.6 m over the ground or, where the rim would
    /// hide the lake, raised until its edge is in sight (and one column either way along the shore): hot at the edge,
    /// falling with distance, never over the crust's 375 C; just behind the rim at the ground; 3 m over its middle.
    /// From 30 m out the eye is raised 54 m to see past the rim, over `RAY_REACH` from the lake: the air's.
    #[test]
    fn a_lava_lakes_felt_falls_with_distance_under_its_crust() {
        // (on the land it was written for: on hill country seed 2's lake sits in a crater so deep that 20 m out the
        // eye must rise 47 m to see its rim, and the felt is the air's from there on)
        let (seed, map) = crate::bake::volcano::tests::live_seed_v11();
        let w = World::with_grid(768, crate::bodies::moon_columns(768), seed, Some(map), None, 0.0, HILL_COUNTRY - 1);
        let b = &w.bodies[0];
        let c = b.craters().iter().find(|c| c.lake.is_some()).expect("a lake").clone();
        let ej = c.at.1 as i32;
        let edge = c.lake_cols.iter().filter(|q| q.1 == ej).map(|q| q.0).max().expect("a lake row through its middle");
        let lake_r = b.layer_r(c.lake.unwrap() as f64);
        let edge_at = b.centre + b.dir(c.face as usize, edge as f64 + 0.5, ej as f64 + 0.5) * lake_r;
        let column = |d: i32, dj: i32| {
            let up = b.dir(c.face as usize, (edge + d) as f64 + 0.5, (ej + dj) as f64 + 0.5);
            (up, b.layer_r(b.first_air(b.col_height(c.face, edge + d, ej + dj)) as f64))
        };
        let open = |a: DVec3, b: DVec3| {
            let n = ((b - a).length() / 0.25) as i32;
            (1..n).all(|k| {
                let m = w.block(w.block_at(a + (b - a) * (k as f64 / n as f64)));
                !(BLOCKS.solid(m) && BLOCKS.opaque(m))
            })
        };
        let in_sight = |d: i32, dj: i32| {
            let (up, ground) = column(d, dj);
            let mut h = 1.6;
            while !open(b.centre + up * (ground + h), edge_at + up * 0.05) {
                h += 1.0;
            }
            (h, w.felt(b.centre + up * (ground + h), Some(15.0)).temp)
        };
        let f: Vec<(i32, f64, f64, f64, f64)> = [2, 5, 10, 20, 30]
            .into_iter()
            .map(|d| {
                let (h, t) = in_sight(d, 0);
                let side: Vec<f64> = [-1, 1].iter().map(|&dj| in_sight(d, dj).1).collect();
                (d, h, t, side[0].min(side[1]).min(t), side[0].max(side[1]).max(t))
            })
            .collect();
        eprintln!("felt by the lake in sight (m out, eye over the ground, felt, the least and most a column either way): {f:.1?}");
        assert!(f.iter().all(|x| x.4 <= 375.0), "{f:?}");
        assert!(f.windows(2).all(|p| p[0].2 > p[1].2), "falls with distance: {f:?}");
        assert!(f[0].2 > 40.0 && f[3].2 > 15.0 && near(f[4].2, 15.0, 1e-9), "{f:?}");
        if let Some(d) = (1..40).find(|&d| {
            let (up, ground) = column(d, 0);
            !open(b.centre + up * (ground + 1.6), edge_at + up * 0.05)
        }) {
            let (up, ground) = column(d, 0);
            eprintln!("just behind the rim, {d} m out: {:?}", w.felt(b.centre + up * (ground + 1.6), Some(15.0)));
        }
        let mid = b.dir(c.face as usize, c.at.0, c.at.1);
        let over = w.felt(b.centre + mid * (lake_r + 3.0), Some(15.0));
        eprintln!("3 m over the lake's middle: {over:?}");
        assert!(over.temp > 150.0 && over.mrt <= 375.0, "{over:?}");
    }

    /// Bug 2026-11-24_18-46-31 (rain inside the volcano, falling onto 1150 C lava): no drop lands on lava, a crater's
    /// lake or lava poured anywhere; beside it and on the plain land it rains as ever.
    #[test]
    fn rain_boils_off_over_lava_and_falls_elsewhere() {
        let w = volcano_world();
        let b = &w.bodies[0];
        let c = b.craters().iter().find(|c| c.lake.is_some()).expect("a live crater with a lake");
        let (i, j) = c.lake_cols[c.lake_cols.len() / 2];
        let d = b.dir(c.face as usize, i as f64 + 0.5, j as f64 + 0.5);
        let bed = b.layer_r(b.first_air(b.col_height(c.face, i, j)) as f64);
        assert!(w.lava_at(b.centre + d * (bed + 0.1)), "the lake over its bed");
        assert!(w.boils_rain(b.centre + d * (bed + 0.1)), "onto the lake's bed, through the lava");
        assert!(w.boils_rain(b.centre + d * (b.layer_r(c.lake.unwrap() as f64) + 30.0)), "in the air 30 m over the lake");
        // the review: by the lava that is there now, not the lake as baked: a column of the lake built over with
        // stone down through its surface has rain again
        let mut roofed = volcano_world();
        for z in c.lake.unwrap() - 3..c.lake.unwrap() + 1 {
            let cell = Block { body: 0, face: c.face, pos: IVec3::new(i, j, z) };
            roofed.set_block(cell, BLOCKS.id("stone"));
            roofed.regap(cell);
        }
        assert!(!roofed.boils_rain(b.centre + d * (b.layer_r(c.lake.unwrap() as f64) + 30.0)), "over a lake column filled with stone");
        let land = b.centre + b.spawn * (b.layer_r(b.first_air(b.height(b.spawn)) as f64) + 0.1);
        assert!(!w.boils_rain(land), "on the land at the spawn");
        // lava poured on cold ground: none onto it, rain beside it
        let mut w = lava_world();
        let (at, top) = lava(&mut w, 0..3, 0..3, 2);
        assert!(w.boils_rain(eye(&w, Block { pos: IVec3::new(at.pos.x, at.pos.y, top - 1), ..at }, 1, 1, 0.1)), "onto poured lava");
        assert!(w.boils_rain(eye(&w, at, 1, 1, 14.0)), "14 m over poured lava, where a drop is made");
        assert!(!w.boils_rain(eye(&w, at, 30, 30, 0.1)) && !w.boils_rain(eye(&w, at, 30, 30, 14.0)), "30 columns from it");
    }
}
