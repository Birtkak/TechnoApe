//! World: its bodies, the ore ledger, frames of reference and gravity.
use super::*;

impl World {
    /// The bodies of `data/bodies.toml` in a world of generation 0 (Terra and Luna), with `planet_n` and `moon_n`
    /// columns per face edge.
    pub fn new(planet_n: i32, moon_n: i32, seed: i32) -> Self {
        World::with_map(planet_n, moon_n, seed, None)
    }
    /// The world of a new game: the planet and Luna baked (ADR 0043, 0070), from memory, the disk or baked now.
    pub fn baked(planet_n: i32, moon_n: i32, seed: i32) -> Self {
        World::baked_warp(planet_n, moon_n, seed, 0.0, GENERATION)
    }
    /// `baked`, the planet of generation `gen` (`with_grid`), every body's block grid warped by `warp` or by the
    /// generation's (`grid_warp`), the more of the two.
    pub fn baked_warp(planet_n: i32, moon_n: i32, seed: i32, warp: f64, gen: u8) -> Self {
        World::with_grid(planet_n, moon_n, seed, Some(crate::bake::get_at(seed, planet_n, gen)), Some(crate::bake::luna::get(seed, moon_n)), warp, gen)
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
    /// mantle (ADR 0152; generation 0, a save from before, keeps its 192 m floor); every other body with blocks from
    /// `DEEP_BODIES`, before it its old floor. A body too small for a deep crust keeps its floor.
    /// The bodies are the rows of generation `gen` (`bodies::of`: Mars from 5); one after the home's moon is on
    /// its own bake when the home is on a map (a baked world), else on the seed's noise.
    pub fn with_grid(planet_n: i32, moon_n: i32, seed: i32, map: Option<Arc<Map>>, luna: Option<Arc<Map>>, warp: f64, gen: u8) -> Self {
        World::with_grid_lazy(planet_n, moon_n, seed, map, luna, warp, gen, false)
    }
    /// `with_grid`; with `lazy` every body `lazy_row` names stands on its map later (`Source::Later`: its map is
    /// got once for its hash and let go).
    #[allow(clippy::too_many_arguments)]
    pub fn with_grid_lazy(planet_n: i32, moon_n: i32, seed: i32, map: Option<Arc<Map>>, luna: Option<Arc<Map>>, warp: f64, gen: u8, lazy: bool) -> Self {
        let rows = crate::bodies::of(gen);
        let moon = crate::bodies::moon(rows);
        let list = crate::bodies::sizes(rows, planet_n, moon_n)
            .into_iter()
            .enumerate()
            .map(|(i, n)| (n, if i == 0 { map.clone() } else if Some(i) == moon { luna.clone() } else { map.as_ref().and_then(|_| Some(rows[i].bake.as_ref()?.get(seed, n))) }))
            .enumerate()
            .map(|(i, (n, m))| match m {
                Some(m) if lazy && lazy_row(rows, i) => (n, Source::Later(m.hash)),
                m => (n, Source::Now(m)),
            })
            .collect();
        World::of_sources(rows, list, seed, warp, gen)
    }
    /// The world of `rows` (`data/bodies.toml`'s, or a test's), each body of the columns and map given in row
    /// order (0163-n-bodies). A body's grid lies at its nominal centre: the home's at the origin, the k-th
    /// other's k times MOON_AT out, so a position names its frame by where it is (ADR 0022).
    pub fn of(rows: &[Row], list: Vec<(i32, Option<Arc<Map>>)>, seed: i32, warp: f64, gen: u8) -> Self {
        World::of_sources(rows, list.into_iter().map(|(n, m)| (n, Source::Now(m))).collect(), seed, warp, gen)
    }
    /// `of`, a body given as `Source::Later` a stub until its ground is loaded (step 6b: `loader`, `put`).
    pub fn of_sources(rows: &[Row], list: Vec<(i32, Source)>, seed: i32, warp: f64, gen: u8) -> Self {
        assert_eq!(rows.len(), list.len(), "a size and a map for every body");
        assert!(rows.len() <= crate::bodies::MAX, "{} bodies; a world holds at most {}", rows.len(), crate::bodies::MAX);
        assert!((0.0..=1.0).contains(&warp), "warp= is 0 to 1");
        // the grid follows from the generation (CONFORMAL); `warp` only warps a world of an earlier one (a test's)
        let warp = warp.max(grid_warp(gen));
        let moon = crate::bodies::moon(rows).expect("the home has a moon (bodies::parse)");
        // every body's interior from its row, its pull scaled by the first's (0162-planet-interior)
        let mut first: Option<Interior> = None;
        // the sun is the home's `sun` radii away and as big as its first moon lets a total eclipse be perfect
        // (ADR 0041); under real gravity (ADR 0180) it has its row's density, and every period is Kepler's from the
        // masses and distances with the one G, a whole fraction of the sky's repeat (`sky::period_in`)
        let r0 = Body::radius_of(list[0].0);
        let (k, min) = rows[moon].orbit.expect("a moon has an orbit (bodies::parse)");
        let mut sky = Sky::new(r0, Body::radius_of(list[moon].0), (r0 * k).max(min) / r0, rows[0].sun.expect("the home has the sun's distance"));
        let real = gen >= REAL;
        let big_g = crate::interior::constant(rows[0].makeup_at(gen), r0);
        let kepler = |a: f64, mass: f64| crate::sky::period_in(std::f64::consts::TAU * (a.powi(3) / (big_g * mass)).sqrt(), crate::sky::REPEAT);
        if real {
            sky.sun_mass = rows[0].sun_density.unwrap_or(crate::bodies::SUN_DENSITY) * 4.0 / 3.0 * std::f64::consts::PI * sky.sun_radius.powi(3);
        }
        let repeat = if real { crate::sky::REPEAT } else { crate::sky::PERIOD };
        // the home's tide from its moon's mass and the sun's, at their distances (`sky::tide_rise`); every other
        // raiser's is under a micrometre (gravity_tests)
        let (home_mass, moon_mass) = (Interior::new(rows[0].makeup_at(gen), r0, 0.0, None).mass, Interior::new(rows[moon].makeup_at(gen), sky.moon_radius, 0.0, None).mass);
        let tides = (crate::sky::tide_rise(moon_mass, home_mass, r0, sky.moon_orbit), crate::sky::tide_rise(sky.sun_mass, home_mass, r0, sky.sun_dist));
        let tide_high = if real { 1.5 * (tides.0 + tides.1) } else { crate::sky::TIDE_HIGH };
        let mut bodies: Vec<Arc<Body>> = Vec::with_capacity(rows.len());
        let mut lazy = vec![None; rows.len()];
        for (i, (row, (n, map))) in rows.iter().zip(list).enumerate() {
            let mut b = Body::new(if row.gas() { GAS_AT } else { MOON_AT } * i as f64, n, seed + i as i32);
            (b.name, b.air, b.ground, b.warp, b.gas, b.icy) = (row.name.clone(), row.air, row.ground, warp, row.gas(), row.makeup.kind == crate::bodies::Kind::Icy);
            b.parent = row.parent.unwrap_or(INERTIAL);
            let own = Interior::new(row.makeup_at(gen), b.radius, 0.0, first.as_ref()).mass;
            // a moon `orbit` radii of its parent out (Luna 60, the Moon's), never nearer than orbit_min, once
            // a month by Kepler's law against the home's first moon's, by the root of the home's mean density over
            // its parent's (1 for the home's moons, to the bit; the giant's moon 2.1 times slower; `sky::period`);
            // one that circles the sun once a year by Kepler's law against the home's distance
            let month = match (row.parent, row.orbit) {
                (Some(p), Some((k, min))) => {
                    // (orbits in parent radii; the home's moon's from its row, which may come later)
                    let q = |r: f64, (k, min): (f64, f64)| (r * k).max(min) / r;
                    let r = bodies[p].radius;
                    let mine = q(r, (k, min));
                    b.orbit = r * mine;
                    let luna = q(bodies[0].radius, rows[moon].orbit.expect("a moon has an orbit (bodies::parse)"));
                    let dens = |b: &Body| b.interior.mass / b.radius.powi(3);
                    b.period = if real {
                        // (real gravity: sidereal, the two masses about each other)
                        kepler(b.orbit, bodies[p].interior.mass + own)
                    } else {
                        crate::sky::period(if i == moon { crate::sky::MONTH } else { crate::sky::MONTH * (mine / luna).powf(1.5) * (dens(&bodies[0]) / dens(&bodies[p])).sqrt() })
                    };
                    Some(b.period)
                }
                _ => {
                    b.orbit = b.radius * row.sun.expect("a body that circles the sun has its distance (bodies::parse)");
                    b.period = if real {
                        kepler(b.orbit, sky.sun_mass + own)
                    } else if i == 0 {
                        crate::sky::YEAR
                    } else {
                        crate::sky::period(crate::sky::YEAR * (b.orbit / bodies[0].orbit).powf(1.5))
                    };
                    None
                }
            };
            // (one that circles the sun and is not home turns only with a `day`: its pose does not, 0163's ring)
            let spin = if row.parent.is_none() && !row.home { row.day.map_or(0.0, |d| 1.0 / (d * crate::sky::DAY)) } else { crate::interior::spin(month, if real { f64::INFINITY } else { crate::sky::YEAR }, if i == 0 { b.period } else { bodies[0].period }) };
            // its pose turns too (F9 "mars does not rotate"); shortcut: the giant's bands take their own angle in the
            // client (frame.rs `far_lights`), so its pose stays still until that angle is read from here
            if row.parent.is_none() && !row.home && !row.gas() {
                b.day = row.day.map_or(0.0, |d| crate::sky::period_in(d * crate::sky::DAY, repeat));
            }
            b.year = if i == 0 { b.period } else { bodies[0].period };
            b.tide_high = tide_high;
            b.interior = Interior::new(row.makeup_at(gen), b.radius, spin, first.as_ref());
            first.get_or_insert_with(|| b.interior.clone());
            // every body is of the world's generation (from 4 Luna's rock field asks it); the home's grid goes down
            // through the mantle from 1, every body's with blocks from DEEP_BODIES (one too small for the deep crust
            // gets none, as a small home)
            b.gen = gen;
            if row.home && gen >= 1 || gen >= DEEP_BODIES && !row.gas() {
                b = b.with_mantle();
            }
            bodies.push(match map {
                Source::Now(map) => Arc::new(b.on(row, map, seed + i as i32)),
                Source::Later(hash) => {
                    assert!(row.bake.is_some(), "body '{}' has no bake, so no map", row.name);
                    let b = Arc::new(b);
                    lazy[i] = Some(Lazy { hash, seed, loaded: false, stub: b.clone() });
                    b
                }
            });
        }
        let sky = if real {
            sky.real(bodies[0].period, bodies[moon].period, tides.0, tides.1)
        } else {
            // what Kepler's law needs for the hand-set year (a number for the record; nothing reads it there)
            Sky { sun_mass: (std::f64::consts::TAU / sky.year).powi(2) * bodies[0].orbit.powi(3) / big_g - bodies[0].interior.mass, ..sky }
        };
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
            snow: Default::default(),
            ore_left: None,
            far: None,
            lazy,
        }
    }
    /// A new world on this one's bodies, shared, as `of_sources` made it: no edits, lights, models, water or
    /// ledger, at time 0. What another player in this process starts from without building the bodies again
    /// (run=sim's bots).
    pub fn fresh(&self) -> World {
        World {
            bodies: self.bodies.clone(),
            sky: self.sky,
            moon: self.moon,
            time: 0.0,
            edits: FxMap::default(),
            edit_count: 0,
            lights: FxMap::default(),
            lights_rev: 0,
            models: FxMap::default(),
            water: Liquids::default(),
            snow: Default::default(),
            ore_left: None,
            far: None,
            lazy: self.lazy.clone(),
        }
    }
    /// Body `i` to change, in a test or as it is made: shared with no one yet.
    pub fn body_mut(&mut self, i: usize) -> &mut Body {
        Arc::get_mut(&mut Arc::get_mut(&mut self.bodies).expect("the bodies are not shared yet")[i]).expect("the body is not shared yet")
    }
    /// Whether body `i`'s ground is in: every body but one loaded on approach that is not (step 6b).
    pub fn loaded(&self, i: usize) -> bool {
        self.lazy.get(i).and_then(Option::as_ref).is_none_or(|l| l.loaded)
    }
    /// The hash of the map body `i` stands on, None for the noise; one not loaded yet says the hash it will be on.
    pub fn map_hash(&self, i: usize) -> Option<u64> {
        match self.lazy.get(i).and_then(Option::as_ref) {
            Some(l) => Some(l.hash),
            None => self.bodies[i].map().map(|m| m.hash),
        }
    }
    /// What builds body `i`'s ground off the frame thread and the tick (step 6b), None when it is in or never
    /// lazy: its map of the hash (from memory, the disk, or baked again) and everything made from it, `World::of`'s
    /// own steps on the same stub, so it comes out bit for bit as at the start. The closure answers Err when this
    /// build bakes another map than the hash's; the body then stays a stub.
    pub fn loader(&self, i: usize) -> Option<impl FnOnce() -> Result<Body, String> + Send + 'static> {
        let l = self.lazy.get(i).and_then(Option::as_ref).filter(|l| !l.loaded)?;
        let (row, seed, n, hash) = (crate::bodies::of(self.bodies[i].gen)[i].clone(), l.seed, self.bodies[i].n, l.hash);
        let build = self.builder(i)?;
        Some(move || {
            let bake = row.bake.as_ref().expect("a lazy body has a bake");
            let map = bake.for_hash(seed, n, hash).ok_or_else(|| format!("this world was played on another map of {} of seed {seed} (map version {}): it stays out of reach", row.name, bake.version()))?;
            Ok(build(map))
        })
    }
    /// `loader`'s second half: body `i` built on `map`.
    pub fn builder(&self, i: usize) -> Option<impl FnOnce(Arc<Map>) -> Body + Send + 'static> {
        let l = self.lazy.get(i).and_then(Option::as_ref).filter(|l| !l.loaded)?;
        let (row, seed, stub) = (crate::bodies::of(self.bodies[i].gen)[i].clone(), l.seed + i as i32, l.stub.stub(l.seed + i as i32));
        Some(move |map| stub.on(&row, Some(map), seed))
    }
    /// Body `i`'s ground, built by its `loader`, goes in (or one another world of this process has, shared).
    pub fn put(&mut self, i: usize, body: impl Into<Arc<Body>>) {
        let body = body.into();
        let l = self.lazy[i].as_mut().expect("a lazy body");
        assert!(body.map().is_some_and(|m| m.hash == l.hash), "the body of its hash");
        l.loaded = true;
        let at = self.ore_base(i);
        Arc::make_mut(&mut self.bodies)[i] = body;
        // the ledger (the server's) takes the body's ore bodies in as opened, less its cells edited before (a
        // save's): as `open_ledger` and `tally` would have had them with the body in from the start
        if self.ore_left.is_some() {
            let mut add: Vec<i64> = crate::ore::count(&self.bodies[i]).into_iter().map(|h| h.blocks as i64).collect();
            for (key, cells) in self.edits.iter().filter(|(k, _)| k.body as usize == i) {
                for (pos, &m) in cells {
                    if let Some(k) = self.ore_cell(Block { body: key.body, face: key.face, pos: *pos }) {
                        add[k as usize] -= (m != self.bodies[i].ore_block(k)) as i64;
                    }
                }
            }
            self.ore_left.as_mut().unwrap().splice(at..at, add);
        }
    }
    /// Body `i`'s ground goes out again (the client, when it has left it far behind): back to its stub, its map
    /// and tables freed once no job holds them.
    pub fn unload(&mut self, i: usize) {
        assert!(self.ore_left.is_none(), "a world with a ledger keeps what it loaded");
        let Some(l) = self.lazy[i].as_mut().filter(|l| l.loaded) else { return };
        l.loaded = false;
        Arc::make_mut(&mut self.bodies)[i] = l.stub.clone();
    }
    /// Opens the ore ledger: every ore body of every body (world body by world body, each's in its order) as
    /// generated (`ore::count`), before any edit.
    pub fn open_ledger(&mut self) {
        assert!(self.edits.is_empty(), "the ledger opens on the generated ground");
        self.ore_left = Some(self.bodies.iter().flat_map(|b| crate::ore::count(b).into_iter().map(|h| h.blocks as i64)).collect());
    }
    /// Where world body `body`'s ore bodies start in the ledger (`ore_left`, `ore_recount`): the bodies'
    /// before it.
    pub fn ore_base(&self, body: usize) -> usize {
        self.bodies[..body.min(self.bodies.len())].iter().map(|b| b.ores.len()).sum()
    }
    /// The ore body of its own body whose generated ore block b is (an index into that body's `ores`).
    pub fn ore_cell(&self, b: Block) -> Option<u32> {
        let body = &self.bodies[b.body as usize];
        if body.ores.is_empty() || body.tile_ores(b.face, b.pos.x.div_euclid(CS), b.pos.y.div_euclid(CS)).is_empty() {
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
        let ore = self.bodies[b.body as usize].ore_block(k);
        let was = self.edit(b).unwrap_or(ore) == ore;
        let at = self.ore_base(b.body as usize) + k as usize;
        self.ore_left.as_mut().unwrap()[at] += (new == ore) as i64 - was as i64;
    }
    /// The ledger again from the edits alone, for the soak: each body's opening blocks less its cells whose
    /// edit is not its ore. Equal to `ore_left` when no block was lost or made.
    pub fn ore_recount(&self, open: &[i64]) -> Vec<i64> {
        let mut left = open.to_vec();
        for (key, cells) in &self.edits {
            for (pos, &m) in cells {
                let b = Block { body: key.body, face: key.face, pos: *pos };
                if let Some(k) = self.ore_cell(b) {
                    left[self.ore_base(b.body as usize) + k as usize] -= (m != self.bodies[b.body as usize].ore_block(k)) as i64;
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
        // a moon of the home a quarter of its orbit; a moon of another body by Laplace's rule against its parent, at
        // most that (the giant's icy moon's 16 km, inside the giant's); one that circles the sun the home's, or by
        // Laplace's rule from its mass and orbit against the home's where that is larger (the giant's 7.5 Mm;
        // Mars's would be 0.65 of the home's)
        let b = &self.bodies[i];
        if b.parent == 0 {
            return b.orbit * 0.25;
        }
        if b.parent != INERTIAL {
            return (b.orbit * (b.interior.mass / self.bodies[b.parent].interior.mass).powf(0.4)).min(b.orbit * 0.25);
        }
        let home = self.sky.moon_orbit * 0.6;
        if i == 0 {
            return home;
        }
        let h = &self.bodies[0];
        home.max(home * b.orbit / h.orbit * (b.interior.mass / h.interior.mass).powf(0.4))
    }
    /// Frame `f`'s pose at time t: its coordinates to the inertial frame. A body is posed on its parent's pose,
    /// down the rows (0163-n-bodies): the home by the sky, a moon on its circle around its parent, one that
    /// circles the sun not home on its ring (`Sky::round_sun`).
    pub fn pose(&self, f: usize, t: f64) -> Pose {
        let Some(b) = self.bodies.get(f) else { return Pose { centre: DVec3::ZERO, rot: DQuat::IDENTITY } };
        match (f, b.parent) {
            (0, _) => self.sky.planet(t),
            (_, INERTIAL) => self.sky.round_sun(b.orbit, b.period, b.day, t),
            (_, p) => self.sky.circle(self.pose(p, t).centre, b.orbit, b.period, t),
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
        // the innermost sphere that holds it: a moon's comes after its parent's in the rows (a moon of the giant lies
        // in the giant's), and the frame it is in keeps it to its whole sphere, another takes it inside 0.97 of its
        let g = (0..self.bodies.len()).rev().find(|&i| inside(i, if i == f { 1.0 } else { 0.97 })).unwrap_or(INERTIAL);
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
        // (the innermost: a moon's sphere is inside its parent's and after it in the rows)
        let f = (0..self.bodies.len()).rev().find(|&i| (x - self.pose(i, t).centre).length() < self.soi(i)).unwrap_or(INERTIAL);
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
    /// The sun's mass, kg (`Sky::sun_mass`).
    pub fn sun_mass(&self) -> f64 {
        self.sky.sun_mass
    }
    pub fn surface_g(&self, i: usize) -> f64 {
        self.bodies[i].interior.surface_g
    }
    /// Body b's magnetic field now, over the home's at the start (0164-magnetic-field): its interior's, the
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

impl Body {
    /// The body on its row's ground: the map given (a plates bake's or an airless crust's), else its seed's noise
    /// (the continents where the row says so). `World::of`'s last step, and a lazy body's `loader`'s.
    pub(crate) fn on(self, row: &Row, map: Option<Arc<Map>>, seed: i32) -> Body {
        match (map, &row.bake, row.generator) {
            (Some(m), Some(k), _) if k.plates() => self.with_map(m),
            (Some(m), Some(_), _) => self.with_luna(m),
            (Some(_), None, _) => panic!("body '{}' has no bake, so no map", row.name),
            (None, _, Generator::Continents) => self.with_continents(seed),
            _ => self,
        }
    }
}

/// Whether row i's body is loaded on approach where its world is (step 6b): one past the home's first moon with a
/// bake (Mars). The home and its moon are always in (Luna is some 5 pixels across from home: always streamed); a
/// gas body has no ground to load.
pub fn lazy_row(rows: &[Row], i: usize) -> bool {
    crate::bodies::moon(rows).is_some_and(|m| i > m) && !rows[i].gas() && rows[i].bake.is_some()
}
