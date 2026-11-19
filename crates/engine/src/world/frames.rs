//! World: its bodies, the ore ledger, frames of reference and gravity.
use super::*;

impl World {
    /// The bodies of `data/bodies.toml` (Terra and Luna), with `planet_n` and `moon_n` columns per face edge.
    pub fn new(planet_n: i32, moon_n: i32, seed: i32) -> Self {
        World::with_map(planet_n, moon_n, seed, None)
    }
    /// The world of a new game: the planet and Luna baked (ADR 0043, 0070), from memory, the disk or baked now.
    pub fn baked(planet_n: i32, moon_n: i32, seed: i32) -> Self {
        World::baked_warp(planet_n, moon_n, seed, 0.0, GENERATION)
    }
    /// `baked` with every body's block grid warped by `warp` (the conformal grid's trial, ADR 0131), the planet
    /// of generation `gen` (`with_grid`).
    pub fn baked_warp(planet_n: i32, moon_n: i32, seed: i32, warp: f64, gen: u8) -> Self {
        World::with_grid(planet_n, moon_n, seed, Some(crate::bake::get(seed, planet_n)), Some(crate::bake::luna::get(seed, moon_n)), warp, gen)
    }
    /// The planet on `map` when one is given (its seed and size must be these), else on the noise land; Luna
    /// on its hills.
    pub fn with_map(planet_n: i32, moon_n: i32, seed: i32, map: Option<Arc<Map>>) -> Self {
        World::with_maps(planet_n, moon_n, seed, map, None)
    }
    /// `with_map`, and Luna on `luna` when one is given (`bake::luna`).
    pub fn with_maps(planet_n: i32, moon_n: i32, seed: i32, map: Option<Arc<Map>>, luna: Option<Arc<Map>>) -> Self {
        World::with_maps_warp(planet_n, moon_n, seed, map, luna, 0.0)
    }
    /// `with_maps` with every body's block grid warped by `warp` (ADR 0131): 0 is today's grid.
    pub fn with_maps_warp(planet_n: i32, moon_n: i32, seed: i32, map: Option<Arc<Map>>, luna: Option<Arc<Map>>, warp: f64) -> Self {
        World::with_grid(planet_n, moon_n, seed, map, luna, warp, 0)
    }
    /// `with_maps_warp`, the planet of generation `gen` (`Body::gen`): from 1 its grid goes down through the
    /// mantle (ADR 0152; generation 0, a save from before, keeps its 192 m floor). Luna and a planet too small for
    /// a deep crust keep theirs.
    pub fn with_grid(planet_n: i32, moon_n: i32, seed: i32, map: Option<Arc<Map>>, luna: Option<Arc<Map>>, warp: f64, gen: u8) -> Self {
        let moon = crate::bodies::moon(&BODIES);
        let list = crate::bodies::sizes(&BODIES, planet_n, moon_n).into_iter().enumerate().map(|(i, n)| (n, if i == 0 { map.clone() } else if Some(i) == moon { luna.clone() } else { None })).collect();
        World::of(&BODIES, list, seed, warp, gen)
    }
    /// The world of `rows` (`data/bodies.toml`'s, or a test's), each body of the columns and map given in row
    /// order (0163-n-bodies). A body's grid lies at its nominal centre: the home's at the origin, the k-th
    /// other's k times MOON_AT out, so a position names its frame by where it is (ADR 0022).
    pub fn of(rows: &[Row], list: Vec<(i32, Option<Arc<Map>>)>, seed: i32, warp: f64, gen: u8) -> Self {
        assert_eq!(rows.len(), list.len(), "a size and a map for every body");
        assert!(rows.len() <= crate::bodies::MAX, "{} bodies; a world holds at most {}", rows.len(), crate::bodies::MAX);
        assert!((0.0..=1.0).contains(&warp), "warp= is 0 to 1");
        let moon = crate::bodies::moon(rows).expect("the home has a moon (bodies::parse)");
        // every body's interior from its row, its pull scaled by the first's (0162-planet-interior)
        let mut first: Option<Interior> = None;
        let mut bodies: Vec<Body> = Vec::with_capacity(rows.len());
        for (i, (row, (n, map))) in rows.iter().zip(list).enumerate() {
            let mut b = Body::new(MOON_AT * i as f64, n, seed + i as i32);
            (b.name, b.air, b.ground, b.warp) = (row.name.clone(), row.air, row.ground, warp);
            b.parent = row.parent.unwrap_or(INERTIAL);
            // a moon `orbit` radii of its parent out (Luna 60, the Moon's), never nearer than orbit_min, once
            // a month by Kepler's law against the home's first moon's for a parent as dense (`sky::period`);
            // one that circles the sun once a year by Kepler's law against the home's distance
            let month = match (row.parent, row.orbit) {
                (Some(p), Some((k, min))) => {
                    // (orbits in parent radii; the home's moon's from its row, which may come later)
                    let q = |r: f64, (k, min): (f64, f64)| (r * k).max(min) / r;
                    let r = bodies[p].radius;
                    let mine = q(r, (k, min));
                    b.orbit = r * mine;
                    let luna = q(bodies[0].radius, rows[moon].orbit.expect("a moon has an orbit (bodies::parse)"));
                    let kepler = if i == moon { crate::sky::MONTH } else { crate::sky::MONTH * (mine / luna).powf(1.5) };
                    b.period = crate::sky::period(kepler);
                    Some(b.period)
                }
                _ => {
                    b.orbit = b.radius * row.sun.expect("a body that circles the sun has its distance (bodies::parse)");
                    b.period = if i == 0 { crate::sky::YEAR } else { crate::sky::period(crate::sky::YEAR * (b.orbit / bodies[0].orbit).powf(1.5)) };
                    None
                }
            };
            let spin = if row.parent.is_none() && !row.home { 0.0 } else { crate::interior::spin(month) };
            b.interior = Interior::new(&row.makeup, b.radius, spin, first.as_ref());
            first.get_or_insert_with(|| b.interior.clone());
            if row.home {
                b.gen = gen;
                if gen >= 1 {
                    b = b.with_mantle();
                }
            }
            bodies.push(match (map, row.bake, row.generator) {
                (Some(m), Some(Bake::Terra), _) => b.with_map(m),
                (Some(m), Some(Bake::Luna), _) => b.with_luna(m),
                (Some(_), None, _) => panic!("body '{}' has no bake, so no map", row.name),
                (None, _, Generator::Continents) => b.with_continents(seed + i as i32),
                _ => b,
            });
        }
        // the sun is the home's `sun` radii away and as big as its first moon lets a total eclipse be perfect
        // (ADR 0041)
        let r0 = bodies[0].radius;
        let (k, min) = rows[moon].orbit.expect("a moon has an orbit (bodies::parse)");
        let sky = Sky::new(r0, bodies[moon].radius, (r0 * k).max(min) / r0, rows[0].sun.expect("the home has the sun's distance"));
        debug_assert_eq!(sky.moon_orbit.to_bits(), bodies[moon].orbit.to_bits());
        World {
            bodies: Arc::new(bodies),
            sky,
            moon,
            time: 0.0,
            edits: FxMap::default(),
            edit_count: 0,
            lights: FxMap::default(),
            lights_rev: 0,
            models: FxMap::default(),
            water: Liquids::default(),
            ore_left: None,
            far: None,
        }
    }
    /// Opens the ore ledger: every body of the planet as generated (`ore::count`), before any edit.
    pub fn open_ledger(&mut self) {
        assert!(self.edits.is_empty(), "the ledger opens on the generated ground");
        self.ore_left = Some(crate::ore::count(&self.bodies[0]).iter().map(|h| h.blocks as i64).collect());
    }
    /// The planet's ore body whose generated ore block b is. shortcut: the planet's only; Luna holds none until
    /// its own minerals (ore-budget.md, "Luna").
    pub fn ore_cell(&self, b: Block) -> Option<u32> {
        let body = &self.bodies[b.body as usize];
        if b.body != 0 || body.ores.is_empty() || body.tile_ores(b.face, b.pos.x.div_euclid(CS), b.pos.y.div_euclid(CS)).is_empty() {
            return None;
        }
        body.ore_at(b.face, b.pos.x, b.pos.y, b.pos.z, body.col_height(b.face, b.pos.x, b.pos.y))
    }
    /// Block b becomes `new`: its body's ore goes down when its own ore leaves the cell, up when it comes back.
    pub(super) fn tally(&mut self, b: Block, new: u16) {
        if self.ore_left.is_none() {
            return;
        }
        let Some(k) = self.ore_cell(b) else { return };
        let ore = self.bodies[0].ore_block(k);
        let was = self.edit(b).unwrap_or(ore) == ore;
        self.ore_left.as_mut().unwrap()[k as usize] += (new == ore) as i64 - was as i64;
    }
    /// The ledger again from the edits alone, for the soak: each body's opening blocks less its cells whose
    /// edit is not its ore. Equal to `ore_left` when no block was lost or made.
    pub fn ore_recount(&self, open: &[i64]) -> Vec<i64> {
        let mut left = open.to_vec();
        for (key, cells) in &self.edits {
            for (pos, &m) in cells {
                let b = Block { body: key.body, face: key.face, pos: *pos };
                if let Some(k) = self.ore_cell(b) {
                    left[k as usize] -= (m != self.bodies[0].ore_block(k)) as i64;
                }
            }
        }
        left
    }
    /// The frame a position (shared coordinates) is in: a body's, near its grid, or the inertial one.
    pub fn frame_at(&self, p: DVec3) -> usize {
        // past the sphere of influence, so a position keeps its frame until `reframe` moves it
        (0..self.bodies.len()).find(|&i| (p - self.bodies[i].centre).length() < 1.5 * self.soi(i)).unwrap_or(INERTIAL)
    }
    /// A body's sphere of influence: within it, a position belongs to the body's frame.
    pub fn soi(&self, i: usize) -> f64 {
        // one that circles the sun as the home's; a moon a quarter of its orbit
        let b = &self.bodies[i];
        if b.parent == INERTIAL { self.sky.moon_orbit * 0.6 } else { b.orbit * 0.25 }
    }
    /// Frame `f`'s pose at time t: its coordinates to the inertial frame. A body is posed on its parent's pose,
    /// down the rows (0163-n-bodies): the home by the sky, a moon on its circle around its parent, one that
    /// circles the sun not home on its ring (`Sky::round_sun`).
    pub fn pose(&self, f: usize, t: f64) -> Pose {
        let Some(b) = self.bodies.get(f) else { return Pose { centre: DVec3::ZERO, rot: DQuat::IDENTITY } };
        match (f, b.parent) {
            (0, _) => self.sky.planet(t),
            (_, INERTIAL) => Sky::round_sun(b.orbit, b.period, t),
            (_, p) => Sky::circle(self.pose(p, t).centre, b.orbit, b.period, t),
        }
    }
    /// Where frame `f`'s pose is anchored in the shared coordinates: its body's grid centre.
    pub fn nominal(&self, f: usize) -> DVec3 {
        if f == INERTIAL {
            INERTIAL_AT
        } else {
            self.bodies[f].centre
        }
    }
    /// A position (shared coordinates) in the inertial frame, now.
    pub fn to_inertial(&self, p: DVec3) -> DVec3 {
        let f = self.frame_at(p);
        self.pose(f, self.time).to_inertial(self.nominal(f), p)
    }
    /// How high point p (shared coordinates) is over the nearest body with air (data/bodies.toml), metres over
    /// its radius, now; f64::MAX with no such body. The air ends `AIR_TOP_M` up.
    pub fn over_air(&self, p: DVec3) -> f64 {
        let x = self.to_inertial(p);
        (0..self.bodies.len()).filter(|&i| self.bodies[i].air).map(|i| (x - self.pose(i, self.time).centre).length() - self.bodies[i].radius).fold(f64::MAX, f64::min)
    }
    /// An inertial position in frame `f`'s shared coordinates, now.
    pub fn from_inertial(&self, f: usize, x: DVec3) -> DVec3 {
        self.pose(f, self.time).from_inertial(self.nominal(f), x)
    }
    /// How frame `from`'s coordinates lie in frame `to`'s, now: x_to = rot * (x - nominal(from)) + off.
    pub fn place(&self, from: usize, to: usize) -> (DQuat, DVec3) {
        if from == to {
            return (DQuat::IDENTITY, self.nominal(from));
        }
        let (a, b) = (self.pose(from, self.time), self.pose(to, self.time));
        (b.rot.inverse() * a.rot, b.from_inertial(self.nominal(to), a.centre))
    }
    /// The inertial velocity of a point at rest at p in frame f.
    fn frame_vel(&self, f: usize, p: DVec3) -> DVec3 {
        let (n, h) = (self.nominal(f), 0.05);
        (self.pose(f, self.time + h).to_inertial(n, p) - self.pose(f, self.time - h).to_inertial(n, p)) / (2.0 * h)
    }
    /// Moves a position and velocity into the frame their real place belongs to (ADR 0022): out of a body's
    /// sphere of influence into the inertial frame, or into one. The velocity keeps the old frame's motion,
    /// so a jump off the moon carries its orbit. Returns the rotation from the old frame to the new one.
    pub fn reframe(&self, pos: &mut DVec3, vel: &mut DVec3) -> Option<DQuat> {
        let f = self.frame_at(*pos);
        let x = self.to_inertial(*pos);
        let t = self.time;
        let inside = |i: usize, k: f64| (x - self.pose(i, t).centre).length() < self.soi(i) * k;
        let g = if f != INERTIAL && inside(f, 1.0) { f } else { (0..self.bodies.len()).find(|&i| inside(i, 0.97)).unwrap_or(INERTIAL) };
        if g == f {
            return None;
        }
        let (pf, pg) = (self.pose(f, t), self.pose(g, t));
        let v = pf.rot * *vel + self.frame_vel(f, *pos);
        let np = self.from_inertial(g, x);
        *vel = pg.rot.inverse() * (v - self.frame_vel(g, np));
        *pos = np;
        Some(pg.rot.inverse() * pf.rot)
    }
    /// An inertial position in the shared coordinates of the frame it lies in (a body's sphere of influence,
    /// or the inertial frame), and that frame.
    pub fn locate(&self, x: DVec3) -> (DVec3, usize) {
        let t = self.time;
        let f = (0..self.bodies.len()).find(|&i| (x - self.pose(i, t).centre).length() < self.soi(i)).unwrap_or(INERTIAL);
        (self.from_inertial(f, x), f)
    }
    /// The height over the nearest body's sea-level sphere, where the bodies really are now.
    pub fn altitude(&self, p: DVec3) -> f64 {
        let x = self.to_inertial(p);
        (0..self.bodies.len())
            .map(|i| (x - self.pose(i, self.time).centre).length() - self.bodies[i].radius)
            .fold(f64::MAX, f64::min)
    }
    /// Gravity at a position (ADR 0024, 0037), m/s^2 in its frame's coordinates: every body pulls towards its
    /// centre where it is now, the square of the distance less with height, so down turns smoothly from one
    /// body to the other and fades out in deep space. At its radius a body pulls with `surface_g`; under it
    /// less, by the radius over its radius (ADR 0152), so nothing at its centre.
    /// Near a moon its own fall around the planet cancels the planet's pull, so only the tide is left (Terra
    /// pulls only G/3600 at Luna, 60 of its radii out, ADR 0041); that fades out by the edge of the moon's
    /// sphere of influence, so gravity stays the same across the hand-over between frames.
    pub fn gravity(&self, p: DVec3) -> DVec3 {
        let f = self.frame_at(p);
        let mut centre = [DVec3::ZERO; crate::bodies::MAX];
        for (i, c) in centre.iter_mut().enumerate().take(self.bodies.len()) {
            *c = if i == f { self.nominal(f) } else { self.from_inertial(f, self.pose(i, self.time).centre) };
        }
        let pull_at = |x: DVec3| -> DVec3 {
            (0..self.bodies.len())
                .map(|i| {
                    let d = centre[i] - x;
                    let r = d.length().max(1e-6);
                    let b = &self.bodies[i].interior;
                    d / r * b.surface_g * b.pull(r / b.radius)
                })
                .sum()
        };
        let g = pull_at(p);
        // (one on rails round the sun falls round nothing that pulls)
        if f == INERTIAL || self.bodies[f].parent == INERTIAL {
            return g;
        }
        let k = 1.0 - smooth01(((p - self.nominal(f)).length() / self.soi(f) - 0.5) / 0.47);
        g - pull_at(self.nominal(f)) * k
    }
    /// Body i's pull at its radius (ADR 0037; `Interior::surface_g`): `interior::G` on the first (Terra) whatever
    /// its size, and on another its mass over the square of its radius; Luna, of Terra's makeup and 3.7 times
    /// smaller, pulls 3.7 times less and a jump there is about 4.6 m.
    pub fn surface_g(&self, i: usize) -> f64 {
        self.bodies[i].interior.surface_g
    }
    /// Body b's magnetic field now, over the home's at the start (draft-magnetic-field): its interior's, the
    /// liquid core and the spin from its row (Terra 1, Luna about 0.012). Every reader asks here, so this is the
    /// one place core mining will lower it, by the liquid metal taken over the liquid metal at the start.
    pub fn field(&self, b: usize) -> f64 {
        self.bodies[b].interior.field
    }
    /// Up at a position: away from the body whose frame it is in; in deep space, away from the nearest body.
    pub fn up_at(&self, p: DVec3) -> DVec3 {
        let f = self.frame_at(p);
        if f == INERTIAL {
            // away from the nearest body where it really is; the inertial frame is not turned
            let x = p - INERTIAL_AT;
            let c = (0..self.bodies.len())
                .map(|i| self.pose(i, self.time).centre)
                .min_by(|a, b| a.distance(x).total_cmp(&b.distance(x)))
                .unwrap_or(DVec3::ZERO);
            return (x - c).normalize_or(DVec3::Y);
        }
        (p - self.nominal(f)).normalize_or(DVec3::Y)
    }
    pub fn nearest(&self, p: DVec3) -> usize {
        let mut best = 0;
        let mut bd = f64::MAX;
        for (i, b) in self.bodies.iter().enumerate() {
            let d = (p - b.centre).length() - b.radius;
            if d < bd {
                bd = d;
                best = i;
            }
        }
        best
    }
}
