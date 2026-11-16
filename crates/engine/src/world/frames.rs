//! World: its bodies, the ore ledger, frames of reference and gravity.
use super::*;

impl World {
    /// The bodies of `data/bodies.toml` (Terra and Luna), with `planet_n` and `moon_n` columns per face edge.
    pub fn new(planet_n: i32, moon_n: i32, seed: i32) -> Self {
        World::with_map(planet_n, moon_n, seed, None)
    }
    /// The world of a new game: the planet and Luna baked (ADR 0043, 0070), from memory, the disk or baked now.
    pub fn baked(planet_n: i32, moon_n: i32, seed: i32) -> Self {
        World::baked_warp(planet_n, moon_n, seed, 0.0)
    }
    /// `baked` with every body's block grid warped by `warp` (the conformal grid's trial, ADR 0131).
    pub fn baked_warp(planet_n: i32, moon_n: i32, seed: i32, warp: f64) -> Self {
        World::with_maps_warp(planet_n, moon_n, seed, Some(crate::bake::get(seed, planet_n)), Some(crate::bake::luna::get(seed, moon_n)), warp)
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
        assert_eq!(BODIES.len(), 2, "the sky (sky.rs) poses a planet and one moon");
        assert!((0.0..=1.0).contains(&warp), "warp= is 0 to 1");
        let bodies: Vec<Body> = BODIES
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let mut b = Body::new(if i == 0 { DVec3::ZERO } else { MOON_AT }, [planet_n, moon_n][i], seed + i as i32);
                (b.name, b.air, b.ground, b.warp) = (row.name.clone(), row.air, row.ground, warp);
                match (&map, row.generator) {
                    (Some(m), Generator::Continents) if i == 0 => b.with_map(m.clone()),
                    (_, Generator::Continents) => b.with_continents(seed + i as i32),
                    _ => match &luna {
                        Some(l) if i == 1 => b.with_luna(l.clone()),
                        _ => b,
                    },
                }
            })
            .collect();
        // Luna circles Terra `orbit` radii out (60, the Moon's), never nearer than orbit_min; the sun is `sun`
        // radii away and as big as Luna lets a total eclipse be perfect (ADR 0041)
        let (k, min) = BODIES[1].orbit.expect("the second body has an orbit");
        let r0 = bodies[0].radius;
        let sun = BODIES[0].sun.expect("the first body has the sun's distance");
        let sky = Sky::new(r0, bodies[1].radius, (r0 * k).max(min) / r0, sun);
        World {
            bodies: Arc::new(bodies),
            sky,
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
        self.sky.moon_orbit * if i == 0 { 0.6 } else { 0.25 }
    }
    /// Frame `f`'s pose at time t: its coordinates to the inertial frame.
    pub fn pose(&self, f: usize, t: f64) -> Pose {
        match f {
            0 => self.sky.planet(t),
            1 => self.sky.moon(t),
            _ => Pose { centre: DVec3::ZERO, rot: DQuat::IDENTITY },
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
    /// body to the other and fades out in deep space. At its radius a body pulls with `surface_g`; deeper in
    /// a little more, up to 1.25 times at 0.9 of its radius and no more.
    /// Near a moon its own fall around the planet cancels the planet's pull, so only the tide is left (Terra
    /// pulls only G/3600 at Luna, 60 of its radii out, ADR 0041); that fades out by the edge of the moon's
    /// sphere of influence, so gravity stays the same across the hand-over between frames.
    pub fn gravity(&self, p: DVec3) -> DVec3 {
        let f = self.frame_at(p);
        let mut centre = [DVec3::ZERO; 4];
        for (i, c) in centre.iter_mut().enumerate().take(self.bodies.len()) {
            *c = if i == f { self.nominal(f) } else { self.from_inertial(f, self.pose(i, self.time).centre) };
        }
        let pull_at = |x: DVec3| -> DVec3 {
            (0..self.bodies.len())
                .map(|i| {
                    let d = centre[i] - x;
                    let r = d.length().max(1e-6);
                    d / r * self.surface_g(i) * (self.bodies[i].radius / r.max(0.9 * self.bodies[i].radius)).powi(2)
                })
                .sum()
        };
        let g = pull_at(p);
        if f == 0 || f == INERTIAL {
            return g;
        }
        let k = 1.0 - smooth01(((p - self.nominal(f)).length() / self.soi(f) - 0.5) / 0.47);
        g - pull_at(self.nominal(f)) * k
    }
    /// Body i's pull at its radius (ADR 0037): G on the first (Terra) whatever its size, and on another
    /// its radius over the first's (one density, as the developer asked in ADR 0041), so Luna, 3.7 times smaller,
    /// pulls 3.7 times less and a jump there is about 4.6 m.
    pub fn surface_g(&self, i: usize) -> f64 {
        G * self.bodies[i].radius / self.bodies[0].radius
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
