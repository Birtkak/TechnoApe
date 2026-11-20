//! Body: construction, the baked map's lookups, ground, plants' feet, caves and ores.
use super::*;

impl Body {
    pub fn new(centre: DVec3, n: i32, seed: i32) -> Self {
        assert!(n > 0 && n % CS == 0, "columns per face edge must be a multiple of {CS}");
        let mut noise = FastNoiseLite::with_seed(seed);
        noise.set_noise_type(Some(NoiseType::OpenSimplex2));
        noise.set_fractal_type(Some(FractalType::FBm));
        noise.set_fractal_octaves(Some(4));
        noise.set_fractal_lacunarity(Some(2.0));
        noise.set_fractal_gain(Some(0.5));
        noise.set_frequency(Some(1.0 / 120.0));
        // 6 n^2 columns cover 4 pi R^2
        let radius = n as f64 * BLOCK / (2.0 * PI / 3.0).sqrt();
        let lnq = (1.0 + BLOCK / radius).ln();
        // small bodies keep half their radius; the deep ground lies under the old floor, layer for layer
        let r0 = (radius - CRUST.min(radius * 0.5)) * (-lnq * deep_layers(n) as f64).exp();
        let layers = (((radius + AMP + BUILD_HEADROOM) / r0).ln() / lnq).ceil() as i32;
        Body { name: String::new(), air: false, gas: false, parent: INERTIAL, orbit: 0.0, period: 0.0, centre, radius, n, root_level: (n / CS).trailing_zeros() as u8, r0, lnq, warp: 0.0, layers, mantle: 0, gen: 0, noise, sea: None, core: None, interior: Interior::new(&BODIES[0].makeup, radius, crate::interior::spin(None), None), land: None, spawn: DVec3::Y, ocean: glam::DMat3::from_diagonal(DVec3::splat(1.0 / 3.0)), ground: *GEN, caves: Vec::new(), cave_tiles: FxMap::default(), cave_cols: FxMap::default(), ores: Vec::new(), ore_tiles: FxMap::default(), ore_z: Vec::new(), tiles: HeightCache::default(), baked: None }
    }
    /// The body on the deep grid (ADR 0152, step 1): the same grid carried on down `mantle_layers` more, layer 0
    /// at about R/sqrt 2; every layer above keeps its radius and gains that in index. Set before its map.
    pub fn with_mantle(mut self) -> Self {
        assert!(self.baked.is_none() && self.caves.is_empty() && self.ores.is_empty(), "the mantle goes in before the map");
        let k = mantle_layers(self.n, self.interior.core_share());
        self.r0 *= (-self.lnq * k as f64).exp();
        (self.layers, self.mantle) = (self.layers + k, k);
        // the liquid outer core under it (draft the metal sea): one level, at the floor until metal is taken
        self.core = (k > 0 && self.interior.liquid > 0.0).then_some(0.0);
        self
    }
    /// The rock at radius r of a column of rock `rock`, `fallback` the biome's (ADR 0063): the one rule the
    /// mesher's fill and `generated_under` ask. On a body with a mantle, under about 192 m the mantle's rock
    /// that has formed there (ADR 0158): by the pressure and heat of the column at sea level
    /// (isostasy: the same at a radius under hill and sea), the province's heat under the column's air, each
    /// boundary moved by the column's wobble.
    pub fn rock(&self, rock: Rock, r: f32, fallback: u16) -> u16 {
        if self.mantle > 0 && (r as f64) < self.interior.moho() + MANTLE_LIFT {
            let sea = self.radius - SEA_DEPTH;
            let heat = rock.air + ROCKS.provinces.get(rock.province as usize).map_or(0.0, |p| p.heat) * (sea - r as f64).max(0.0) as f32;
            let p = ATM + self.rock_weight(sea, r as f64 - rock.wobble as f64);
            if let Some(m) = ROCKS.deep(p as f32, heat) {
                return m;
            }
        }
        ROCKS.at(rock, r, fallback)
    }
    /// The weight of the rock between radius r and the ground at radius `ground` over it, MPa (`Interior::rock_weight`).
    pub fn rock_weight(&self, ground: f64, r: f64) -> f64 {
        self.interior.rock_weight(ground, r)
    }
    /// The pressure at layer z of column (face, i, j), MPa (ADR 0158): the weight of what lies
    /// above as generated (its rock, the water over its ground) and 1 atmosphere at the top. None without a
    /// map, as `heat`. Generated only: a dig does not change it.
    pub fn pressure(&self, face: u8, i: i32, j: i32, z: f64) -> Option<f32> {
        self.baked.as_ref()?;
        let (ground, r) = (self.col_height(face, i, j), self.layer_r(z));
        let top = self.water_at(face, i, j).map_or(ground, |w| self.layer_r(w.0).max(ground));
        let water = WATER_DENSITY * self.interior.weight_g * 1e-6 * (top - r.max(ground)).max(0.0);
        Some((ATM + water + self.rock_weight(ground, r)) as f32)
    }
    /// The body on a baked map (ADR 0043): its heights, biomes, ground and plants, and its spawn.
    pub fn with_map(mut self, map: Arc<Map>) -> Self {
        assert_eq!(map.n, self.n, "a map of another planet size");
        let mut detail = FastNoiseLite::with_seed(map.seed ^ 0x6e7a);
        detail.set_noise_type(Some(NoiseType::OpenSimplex2));
        detail.set_fractal_type(Some(FractalType::FBm));
        detail.set_fractal_octaves(Some(2));
        detail.set_frequency(Some(1.0 / 12.0));
        let mut jitter = FastNoiseLite::with_seed(map.seed ^ 0x1f7e);
        jitter.set_noise_type(Some(NoiseType::OpenSimplex2));
        jitter.set_frequency(Some(1.0 / 24.0));
        let mut fold = FastNoiseLite::with_seed(map.seed ^ 0x0f01);
        fold.set_noise_type(Some(NoiseType::OpenSimplex2));
        fold.set_fractal_type(Some(FractalType::FBm));
        fold.set_fractal_octaves(Some(2));
        fold.set_frequency(Some(1.0));
        let spawn = DVec3::from_array(map.spawn);
        // each flow's cut: the noise's value over which its share of 4096 directions spread over the sphere lie
        let flow_cut = ROCKS
            .provinces
            .iter()
            .map(|p| {
                let Some(f) = p.flow else { return f32::INFINITY };
                let k = self.radius / f.width as f64;
                let mut v: Vec<f32> = (0..4096).map(|n| flow_noise_of(&fold, fibonacci(n, 4096) * k)).collect();
                v.sort_by(f32::total_cmp);
                v[((1.0 - f.share as f64) * 4095.0).round() as usize]
            })
            .collect();
        self.baked = Some(Box::new(Baked { seed: map.seed as u32 as u64, map, detail, jitter, fold, flow_cut, spots: Default::default(), levels: Default::default() }));
        self.sea = Some(self.layer_of(self.radius - SEA_DEPTH).floor() + 0.875);
        self.ocean = self.ocean_moments();
        // the scored spawn, moved to the nearest column with no plant within a step
        let (f, x, y) = self.locate(spawn);
        let (i0, j0) = (x as i32, y as i32);
        let clear = |i: i32, j: i32| (-1..=1).all(|a| (-1..=1).all(|b| self.col_plants(f as u8, i + a, j + b).is_empty()));
        let at = (0..=12).flat_map(|r| (-r..=r).flat_map(move |a| (-r..=r).map(move |b| (a, b))).filter(move |(a, b): &(i32, i32)| a.abs().max(b.abs()) == r)).find(|(a, b)| clear(i0 + a, j0 + b));
        let (a, b) = at.unwrap_or((0, 0));
        self.spawn = self.dir(f, (i0 + a) as f64 + 0.5, (j0 + b) as f64 + 0.5);
        // an airless body has no sea: its caves and ores ask no water and no shore (the ocean above keeps its old sea)
        if self.map().is_some_and(|m| m.crust()) {
            self.sea = None;
        }
        let caves = crate::caves::make(&self, self.baked.as_ref().unwrap().seed);
        self.set_caves(caves);
        let ores = crate::ore::place(&self, self.baked.as_ref().unwrap().seed);
        self.set_ores(ores);
        // plants were looked at for the spawn before the caves were in
        self.baked.as_ref().unwrap().spots.lock().unwrap().clear();
        self
    }
    /// Luna on its bake (ADR 0070): the map's heights and kinds as a baked planet's, no sea, and its lava
    /// tubes as caves. A planet's crust (Mars, `bake::mars`) the same, without caves: its marked tubes wait for
    /// its rock provinces.
    pub fn with_luna(self, map: Arc<Map>) -> Self {
        if map.version == crate::bake::mars::VERSION {
            return self.with_map(map);
        }
        let mut b = self.with_map(map.clone());
        debug_assert!(b.sea.is_none(), "with_map lays no sea on a Luna map");
        let (m, k) = (map.m as usize, b.n as f64 / map.m as f64);
        let tubes = map
            .tubes
            .iter()
            .map(|&(c, depth, r)| {
                let c = c as usize;
                let (f, i, j) = (c / (m * m), c % m, (c / m) % m);
                let ground = b.radius - SEA_DEPTH + map.h[c] as f64;
                // (the map's cell in the block grid, which the warp moves, ADR 0131)
                let (ci, cj) = b.unwarp_col((i as f64 + 0.5) * k, (j as f64 + 0.5) * k);
                Cave::ball(f as u8, DVec3::new(ci, cj, b.layer_of(ground - depth as f64)), r as f64)
            })
            .collect();
        // before generation 4 her baked tubes are all her caves (no rock field, none of `make`'s); from 4 her tubes
        // are data/caves.toml's cut in her mare basalt (`make`), and the baked ones are not used
        if !b.rock_field() {
            b.set_caves(tubes);
        }
        b
    }
    /// The map it was baked from, if it was.
    pub fn map(&self) -> Option<&Arc<Map>> {
        self.baked.as_ref().map(|b| &b.map)
    }
    /// Whether `rock_at` answers a province here: a map with a rock layer, or Luna's from generation 4
    /// (0165-bake-stages). Without one no ore body or cave by rock is made.
    pub fn rock_field(&self) -> bool {
        self.map().is_some_and(|m| !m.rock.is_empty() || m.version == crate::bake::luna::VERSION && self.gen >= 4)
    }
    /// The row of `data/bodies.toml` whose provinces its rock field names (`Map::rock` hands them out by the
    /// map: the home's moon's on a Luna map, the home's on any other): a mineral or cave kind is made only on
    /// the body its provinces are of.
    pub fn rock_body(&self) -> usize {
        match self.map() {
            Some(m) if m.version == crate::bake::luna::VERSION => crate::bodies::moon(&BODIES).unwrap_or(0),
            // shortcut: the one row that bakes a planet's crust; a second such row needs its index in the body
            Some(m) if m.version == crate::bake::mars::VERSION => BODIES.iter().position(|r| r.bake.as_ref().is_some_and(|b| b.2.is_some())).unwrap_or(0),
            _ => 0,
        }
    }
    /// The face of direction d and its point there in columns (i = 3.5 is the middle of column 3).
    pub fn locate(&self, d: DVec3) -> (usize, f64, f64) {
        let a = d.abs();
        let f = if a.x >= a.y && a.x >= a.z { (d.x < 0.0) as usize } else if a.y >= a.z { 2 + (d.y < 0.0) as usize } else { 4 + (d.z < 0.0) as usize };
        let [nn, s, t] = AX[f].map(|v| v.as_dvec3());
        let dn = d.dot(nn);
        let col = |w: f64| ((w / dn).atan() / FRAC_PI_2 * 2.0 + 1.0) * 0.5 * self.n as f64;
        if self.warp == 0.0 {
            return (f, col(d.dot(s)), col(d.dot(t)));
        }
        let (i, j) = self.unwarp_col(col(d.dot(s)), col(d.dot(t)));
        (f, i, j)
    }
    /// Face point (i, j) of the block grid in columns as the equi-angular grid's columns: where `dir` lays it, and
    /// where the baked map (which is equi-angular by direction) is read for it. Itself with no warp.
    pub fn warp_col(&self, i: f64, j: f64) -> (f64, f64) {
        if self.warp == 0.0 {
            return (i, j);
        }
        let h = self.n as f64 * 0.5;
        let (v, _) = crate::warp::at((i - h) / h, (j - h) / h);
        (i + self.warp * h * v[0], j + self.warp * h * v[1])
    }
    /// `warp_col`'s inverse: the block grid's point at the equi-angular grid's point (i, j).
    pub fn unwarp_col(&self, i: f64, j: f64) -> (f64, f64) {
        if self.warp == 0.0 {
            return (i, j);
        }
        let h = self.n as f64 * 0.5;
        let (a, b) = crate::warp::unwarp((i - h) / h, (j - h) / h, self.warp);
        (h + a * h, h + b * h)
    }
    /// Block-grid face point (i, j) as a point of the baked map's cells (`s` its cells a column).
    pub(crate) fn map_xy(&self, s: f64, i: f64, j: f64) -> (f64, f64) {
        let (x, y) = self.warp_col(i, j);
        (x * s, y * s)
    }
    /// The water generation puts over column (i, j) of a face (W5, ADR 0059): its top as a layer coordinate,
    /// the account it books against (`liquid::Ground::reach`), and for a river its flow per column of width
    /// (units a step), its surface's fall a column (units) and the runoff a column gathers (units a step). The
    /// sea's where no river or lake is, or where theirs would stand under it.
    pub fn water_at(&self, face: u8, i: i32, j: i32) -> Option<(f64, u32, Option<(f64, f64, f64)>)> {
        self.water_tide(face, i, j, None)
    }
    /// `water_at` with the sea at `tide` (ADR 0067) instead of its mean: the tidal band goes wet and dry, and a
    /// river's last columns under the high sea are the sea's.
    pub fn water_tide(&self, face: u8, i: i32, j: i32, tide: Option<&Tide>) -> Option<(f64, u32, Option<(f64, f64, f64)>)> {
        let (ci, cj) = (i as f64 + 0.5, j as f64 + 0.5);
        let d = tide.map(|_| self.dir(face as usize, ci, cj));
        let sea = self.sea.map(|s| (s + tide.zip(d).map_or(0.0, |(t, d)| self.tide_layers(t, d)), crate::liquid::SEA, None));
        let Some(b) = &self.baked else { return sea };
        let s = b.map.m as f64 / self.n as f64;
        let (mx, my) = self.map_xy(s, ci, cj);
        if !b.map.maybe_wet(face as usize, mx, my) {
            return sea;
        }
        let Some(w) = b.map.water(face as usize, mx, my, d.unwrap_or_else(|| self.dir(face as usize, ci, cj))) else { return sea };
        let top = self.layer_of(self.radius - SEA_DEPTH + w.level as f64);
        if sea.is_some_and(|s| s.0 >= top) {
            return sea;
        }
        // generation carries the dry-weather base of the flow; the rain brings the rest (W5, ADR 0059)
        let (u, q) = (U as f64, U as f64 / 60.0 * crate::weather::BASE);
        let river = (w.q > 0.0).then(|| (w.q as f64 * q, w.fall as f64 * u, w.gain as f64 * q));
        Some((top, w.reach, river))
    }
    /// The surface radius at face point (i, j) in columns, d its direction, for chunks of `level`: from the
    /// map's mip whose cells are about as wide as the columns, plus the detail; or the noise land.
    pub(crate) fn height_at(&self, face: usize, i: f64, j: f64, d: DVec3, level: u8) -> f64 {
        let Some(b) = &self.baked else { return self.height(d) };
        let s = b.map.m as f64 / self.n as f64;
        let k = (level as i32 - (1.0 / s).log2().round() as i32).max(0) as usize;
        let p = d * self.radius;
        // (below, i and j only say where the map is read)
        let (i, j) = self.warp_col(i, j);
        let noise = || b.detail.get_noise_3d(p.x as f32, p.y as f32, p.z as f32) as f64;
        let mut h = if b.map.version == 1 {
            // a map of version 1: the uniform detail of ADR 0043, to the bit
            b.map.height(face, i * s, j * s, k) as f64 + DETAIL_M * noise()
        } else if k.min(b.map.mips.len()) > 0 {
            // far columns wider than a cell: the mip alone (worldgen.md, "far chunks")
            b.map.height(face, i * s, j * s, k) as f64
        } else {
            debug_assert!(!b.map.rough.is_empty(), "a map of version {} without its roughness", b.map.version);
            let (h, a) = b.map.height_rough(face, i * s, j * s);
            h as f64 + detail(a as f64, noise())
        };
        // the river beds (ADR 0056), after the detail so a bed's line is the bed to the bit
        if let Some(c) = b.map.river(face, i * s, j * s, d, (1u32 << level) as f32) {
            h = h.min(c.cut as f64).max(c.floor as f64);
        }
        // maps from before ADR 0105's peaks keep the clamp of AMP 140, so their worlds keep their ground to the bit
        let amp = if b.map.version == crate::bake::VERSION || b.map.version == 6 || b.map.version == crate::bake::mars::VERSION { AMP } else { 140.0 };
        self.radius - SEA_DEPTH + h.clamp(-FLOOR, amp - SEA_DEPTH - 2.0 - TALLEST as f64)
    }
    /// Face point (i, j) moved by the jitter noise at d, in columns: biome borders and patches wander.
    pub(super) fn jittered(&self, b: &Baked, i: f64, j: f64, d: DVec3) -> (f64, f64) {
        let p = d * self.radius;
        let (x, y, z) = (p.x as f32, p.y as f32, p.z as f32);
        (i + JITTER * b.jitter.get_noise_3d(x, y, z) as f64, j + JITTER * b.jitter.get_noise_3d(x + 517.0, y - 311.0, z + 97.0) as f64)
    }
    /// The seed of its map, None on a planet with no map.
    pub fn baked_seed(&self) -> Option<u64> {
        self.baked.as_ref().map(|b| b.seed)
    }
    /// The biome at face point (i, j) in columns (its direction d), None on a planet with no map.
    pub fn biome_at(&self, face: usize, i: f64, j: f64, d: DVec3) -> Option<u8> {
        let b = self.baked.as_ref()?;
        let (x, y) = self.jittered(b, i, j, d);
        let s = b.map.m as f64 / self.n as f64;
        let (x, y) = self.map_xy(s, x, y);
        Some(b.map.biome(face, x, y))
    }
    /// The baked year's climate at face point (i, j) in columns (`bake::Map::climate_at`): potential
    /// temperature C, day swing K, rain mm a day; None without a map that keeps one.
    pub fn climate_at(&self, face: usize, i: f64, j: f64) -> Option<(f32, f32, f32)> {
        let b = self.baked.as_ref()?;
        let s = b.map.m as f64 / self.n as f64;
        let (x, y) = self.map_xy(s, i, j);
        b.map.climate_at(face, x, y)
    }
    /// The baked year's mean air at the ground of column (face, i, j), C: what the bake chose the biome by
    /// (`bake::Map::temp`). None without a map that keeps a climate.
    pub fn year_air(&self, face: usize, i: i32, j: i32) -> Option<f32> {
        let b = self.baked.as_ref()?;
        let s = b.map.m as f64 / self.n as f64;
        let (x, y) = self.map_xy(s, i as f64 + 0.5, j as f64 + 0.5);
        b.map.temp(face, x, y)
    }
    /// The rock's temperature at layer z of column (face, i, j), C (ADR 0063): the year's mean air at the
    /// ground, and the province's `heat` a metre deeper under it. None without a map's climate and rock.
    pub fn heat(&self, face: u8, i: i32, j: i32, z: f64) -> Option<f32> {
        let b = self.baked.as_ref()?;
        let s = b.map.m as f64 / self.n as f64;
        let (x, y) = self.map_xy(s, i as f64 + 0.5, j as f64 + 0.5);
        let air = b.map.temp(face as usize, x, y)?;
        let p = ROCKS.provinces.get(self.col_rock(face, i, j).province as usize)?;
        Some(air + p.heat * (self.col_height(face, i, j) - self.layer_r(z)).max(0.0) as f32)
    }
    /// The rock at face point (i, j) in columns, d its direction (ADR 0063): the province of the map cell, its
    /// border wandering as the biomes' do, and its layers' top, folded. `Rock::NONE` without a map's rock, and on
    /// Luna before generation 4 (her kinds' rocks, 0165-bake-stages).
    pub fn rock_at(&self, face: usize, i: f64, j: f64, d: DVec3) -> Rock {
        let Some(b) = &self.baked else { return Rock::NONE };
        if self.gen < 4 && b.map.version == crate::bake::luna::VERSION {
            return Rock::NONE;
        }
        let (x, y) = self.jittered(b, i, j, d);
        let s = b.map.m as f64 / self.n as f64;
        let (mx, my) = self.map_xy(s, x, y);
        let Some(p) = b.map.rock(face, mx, my) else { return Rock::NONE };
        let [amp, width] = ROCKS.provinces[p as usize].fold;
        let q = d * (self.radius / width as f64);
        let wave = b.fold.get_noise_3d(q.x as f32, q.y as f32, q.z as f32);
        let top = (self.radius - SEA_DEPTH + ROCKS.datum as f64 + (amp * wave) as f64) as f32;
        let flow = self.flow(b, face, p, s, i, j, d);
        if self.mantle == 0 {
            return Rock { province: p, top, flow, ..Rock::NONE };
        }
        // the mantle's: the year's air over the column's middle, as `heat` reads it, and a slow wave of its own
        let (ax, ay) = self.map_xy(s, i, j);
        let q = d * (self.radius / WOBBLE_M);
        let wobble = ROCKS.wobble * b.fold.get_noise_3d(q.x as f32 + 911.0, q.y as f32 - 377.0, q.z as f32 + 143.0);
        Rock { province: p, top, air: b.map.temp(face, ax, ay).unwrap_or(0.0), wobble, flow }
    }
    /// Where province p's lava flow (`rock::Flow`) lies over face point (i, j), the radius of its foot: its
    /// `thick` metres under the map's ground there; 0 where none lies, in a province without one, or in a world
    /// before generation 2. Its fields are where a slow noise of the seed is over the cut that leaves `share`.
    #[allow(clippy::too_many_arguments)]
    fn flow(&self, b: &Baked, face: usize, p: u8, s: f64, i: f64, j: f64, d: DVec3) -> f32 {
        let Some(f) = ROCKS.provinces[p as usize].flow.filter(|_| self.gen >= 2) else { return 0.0 };
        if flow_noise(b, d * (self.radius / f.width as f64)) < b.flow_cut[p as usize] {
            return 0.0;
        }
        let (mx, my) = self.map_xy(s, i, j);
        (self.radius - SEA_DEPTH + b.map.height(face, mx, my, 0) as f64 - f.thick as f64) as f32
    }
    /// The rock of level-0 column (i, j): from its tile when one is cached, else computed (as `col_height`).
    pub fn col_rock(&self, face: u8, i: i32, j: i32) -> Rock {
        if self.baked.is_none() {
            return Rock::NONE;
        }
        let (x, y) = (i.div_euclid(CS), j.div_euclid(CS));
        if let Some(e) = self.tiles.map.lock().unwrap().get(&(face, 0, x, y)) {
            return e.0.rock[(i - x * CS + 1) as usize + (j - y * CS + 1) as usize * P];
        }
        let (ci, cj) = (i as f64 + 0.5, j as f64 + 0.5);
        self.rock_at(face as usize, ci, cj, self.dir(face as usize, ci, cj))
    }
    /// The ground (`material`) at face point (i, j) in columns, d its direction, `h` its surface radius.
    pub(super) fn ground_at(&self, face: usize, i: f64, j: f64, d: DVec3, h: f64) -> [u16; 3] {
        self.ground_biome_at(face, i, j, d, h).0
    }
    /// `ground_at` and the biome it is of (0 on a planet with no map).
    pub(crate) fn ground_biome_at(&self, face: usize, i: f64, j: f64, d: DVec3, h: f64) -> ([u16; 3], u8) {
        let Some(b) = &self.baked else { return (self.ground, 0) };
        let (x, y) = self.jittered(b, i, j, d);
        let s = b.map.m as f64 / self.n as f64;
        let (mx, my) = self.map_xy(s, x, y);
        let biome = b.map.biome(face, mx, my);
        let row = &ROWS.biomes[biome as usize];
        let cell = ((x / PATCH).floor() as i64 as u64) ^ ((y / PATCH).floor() as i64 as u64).wrapping_mul(0x9e3779b97f4a7c15);
        let roll = unit(mix(b.seed ^ cell ^ (face as u64) << 56));
        if let Some(bed) = self.bed(b, face, i, j, d, h, biome, row.ground) {
            return ([bed[0], bed[1], row.ground[2]], biome);
        }
        ([row.top(roll, (h - (self.radius - SEA_DEPTH)) as f32), row.ground[1], row.ground[2]], biome)
    }
    /// The bed under standing or running water (ADR 0086), top and band, where the column's top block lies
    /// under the water; None on dry ground (the biome's, banks too). By what the bake knows: the slope of the
    /// map's ground, the water's depth, a river's fall. Steep ground is bare rock (the province's); a river
    /// lays gravel where it runs fast, sand where it is gentle, clay where it is slow or deep (and at its
    /// mouth, where it is flat); a lake sand in its shallows, clay in the middle; the sea sand on the beach and
    /// the shelf, mud over clay further out. A warm reef keeps its row (coral). Where the land's band is dirt, a
    /// pond's thin edge and a small gentle stream lie on that soil, flooded: dirt (never grass).
    #[allow(clippy::too_many_arguments)]
    fn bed(&self, b: &Baked, face: usize, i: f64, j: f64, d: DVec3, h: f64, biome: u8, ground: [u16; 3]) -> Option<[u16; 2]> {
        let [sand, clay, mud, gravel, dirt] = *BED;
        let soil = ground[1] == dirt;
        let (top, reach, river) = self.water_at(face as u8, i.floor() as i32, j.floor() as i32)?;
        let depth = self.layer_r(top) - h;
        if depth <= 0.0 {
            return None;
        }
        let sea = river.is_none() && reach == crate::liquid::SEA;
        if sea && biome == life::REEF {
            return None;
        }
        // the slope of the map's cell: the ground's lie, not a column's step
        let s = b.map.m as f64 / self.n as f64;
        let (mx, my) = self.map_xy(s, i, j);
        let slope = b.map.slope(face, mx, my) as f64;
        let bed = if slope > BED_STEEP || (!sea && river.is_none() && biome == life::ALPINE) {
            // the biome's rock where there is no province, as `generated_under` has it under the band
            let rock = ROCKS.at(self.rock_at(face, i, j, d), h as f32, ground[2]);
            return Some([rock, rock]);
        } else if let Some((q, fall, _)) = river {
            // the flow per metre of width as the bake has it (`water_tide` scaled it into units)
            let q = q / (U as f64 / 60.0 * crate::weather::BASE);
            match fall / U as f64 {
                f if f >= BED_RAPID => gravel,
                _ if q < BED_STREAM && depth < BED_SOIL && soil => dirt,
                f if f < BED_SLOW || depth >= BED_DEEP_RIVER => clay,
                _ => sand,
            }
        } else if sea {
            if depth >= BED_SHELF {
                return Some([mud, clay]);
            }
            sand
        } else if depth < BED_SOIL && soil {
            dirt
        } else if depth < BED_SHALLOW_LAKE {
            sand
        } else {
            clay
        };
        Some([bed; 2])
    }
    /// The plant at candidate spot (cx, cy) of a face (one each `SPACING` x `SPACING` columns), if one grows
    /// there: on a baked planet, over the sea, its foot inside the face (its crown may reach over the seam, ADR 0100).
    pub fn candidate(&self, face: usize, cx: i32, cy: i32) -> Option<Foot> {
        self.candidate_of(face, cx, cy, false)
    }
    /// `candidate` of the trees alone (`candidate(..).filter(trunk)`, to the bit): a spot that grows no tree is
    /// turned down before its ground is looked at (the far tiles' canopy, ADR 0100, 0102).
    pub fn tree_candidate(&self, face: usize, cx: i32, cy: i32) -> Option<Foot> {
        self.candidate_of(face, cx, cy, true)
    }
    fn candidate_of(&self, face: usize, cx: i32, cy: i32, trees: bool) -> Option<Foot> {
        let b = self.baked.as_ref()?;
        let r = mix(b.seed.wrapping_mul(0x2545f4914f6cdd1d) ^ (face as u64) << 58 ^ (cx as u32 as u64) << 29 ^ cy as u32 as u64);
        let (i, j) = (cx * SPACING + (r % SPACING as u64) as i32, cy * SPACING + ((r >> 8) % SPACING as u64) as i32);
        if i < 0 || j < 0 || i >= self.n || j >= self.n {
            return None;
        }
        let roll = unit(mix(r ^ 0x5eed));
        // nor a tree in a clearing: one patch of four by four spots in eight, by the hash, so sun reaches a
        // forest's floor (ADR 0092)
        let clearing = mix(b.seed.wrapping_mul(0x9e3779b97f4a7c15) ^ (face as u64) << 58 ^ (cx.div_euclid(4) as u32 as u64) << 29 ^ cy.div_euclid(4) as u32 as u64) % 8 == 0;
        // trees alone: a clearing's, or a roll that picks a tree in no biome, is none before the biome is asked
        if trees && (clearing || !ROWS.may_tree(roll)) {
            return None;
        }
        let d = self.dir(face, i as f64 + 0.5, j as f64 + 0.5);
        let row = &ROWS.biomes[self.biome_at(face, i as f64 + 0.5, j as f64 + 0.5, d)? as usize];
        let plant = row.plant(roll)?;
        let tree = ROWS.plants[plant].trunk != 0;
        if trees && !tree || clearing && tree {
            return None;
        }
        let h = self.height_at(face, i as f64 + 0.5, j as f64 + 0.5, d, 0);
        let z = self.first_air(h);
        if z as f64 <= self.sea? {
            return None;
        }
        // nor in a river or a lake (W5)
        if self.water_at(face as u8, i, j).is_some_and(|w| z as f64 <= w.0) {
            return None;
        }
        // nor over a cave's open mouth (a skylight, a slot), on a planet with the mantle (generation 1 came in on
        // the mantle flag; a rule new since asks `self.gen`, ADR 0160-generation-number-and-arc-basalt)
        if self.mantle > 0 && self.col_caves(face as u8, i, j).iter().any(|&(lo, hi)| (lo..hi).contains(&(z - 1))) {
            return None;
        }
        // nor, from generation 2, a plant whose crown hangs over a cave's open mouth: a skylight is seen from the
        // sky (ADR 0160-generation-number-and-arc-basalt). Columns without a cave cost a tile lookup.
        let crown = ROWS.plants[plant].radius;
        if self.gen >= 2 && self.over_cave(face, i, j, crown) {
            return None;
        }
        // a palm only where the year is warm (ADR 0092)
        if ROWS.plants[plant].warm.is_some_and(|t| self.year_air(face, i, j).is_none_or(|a| a < t)) {
            return None;
        }
        // nor a tree whose crown would hang over a river: a river stays open to swim and wade along (the
        // crown's rim and half way in, sixteen columns)
        let p = &ROWS.plants[plant];
        if p.trunk != 0 && p.radius >= 2 {
            let over = |di: i32, dj: i32| {
                let (g, a, c) = fold(face, i + di, j + dj, self.n);
                self.water_at(g as u8, a, c).is_some_and(|w| w.2.is_some())
            };
            let (r, s) = (p.radius, (p.radius * 7 + 5) / 10);
            for (di, dj) in [(r, 0), (-r, 0), (0, r), (0, -r), (s, s), (s, -s), (-s, s), (-s, -s)] {
                if over(di, dj) || over(di / 2, dj / 2) {
                    return None;
                }
            }
        }
        let [lo, hi] = ROWS.plants[plant].height;
        Some(Foot { plant, h: lo + ((r >> 16) % (hi - lo + 1) as u64) as i32, v: mix(r ^ 0x7ee5) as u32, i, j, z })
    }
    /// The layers [lo, hi) of column (face, i, j) a cave hollows, at layer 0 and up: under the ground they are
    /// air (`World::generated_under`, `generate` at level 0, `Solid::gaps`), over it they change nothing.
    /// shortcut: a cave's ball lies in its face's grid, so one across a seam is cut there; fine while caves
    /// stay clear of the seams (step 2 of W4 places them).
    /// The spans are each cave's merged into sorted, disjoint ones: every reader asks only which layers are hollow.
    pub fn col_caves(&self, face: u8, i: i32, j: i32) -> &[(i32, i32)] {
        let (x, y) = (i.div_euclid(CS), j.div_euclid(CS));
        self.cave_cols.get(&(face, x, y)).map_or(&[], |t| t.get(((i - x * CS) + (j - y * CS) * CS) as u16))
    }
    /// `col_caves` as it was before the merged table: each cave's span in the cave order (the test's reference).
    #[cfg(test)]
    pub(crate) fn col_caves_slow(&self, face: u8, i: i32, j: i32) -> Vec<(i32, i32)> {
        self.tile_caves(face, i.div_euclid(CS), j.div_euclid(CS)).filter_map(|c| c.span(i, j)).collect()
    }
    /// Whether a cave's open mouth lies under the square of `r` columns round (i, j) of `face` (a crown's,
    /// ADR 0160-generation-number-and-arc-basalt): a column of it whose top generated block is hollow. Inside the
    /// face only the hollow columns of the square's footprints are looked at; one over a seam asks each column.
    pub(crate) fn over_cave(&self, face: usize, i: i32, j: i32, r: i32) -> bool {
        let open = |g: usize, a: i32, c: i32, caves: &[(i32, i32)]| {
            let (ci, cj) = (a as f64 + 0.5, c as f64 + 0.5);
            let top = self.first_air(self.height_at(g, ci, cj, self.dir(g, ci, cj), 0)) - 1;
            caves.iter().any(|&(lo, hi)| (lo..hi).contains(&top))
        };
        if !(i - r >= 0 && j - r >= 0 && i + r < self.n && j + r < self.n) {
            return (-r..=r).flat_map(|a| (-r..=r).map(move |c| (a, c))).any(|(di, dj)| {
                let (g, a, c) = fold(face, i + di, j + dj, self.n);
                let caves = self.col_caves(g as u8, a, c);
                !caves.is_empty() && open(g, a, c, caves)
            });
        }
        let tiles = |v: i32| (v - r).div_euclid(CS)..=(v + r).div_euclid(CS);
        tiles(i).any(|x| {
            tiles(j).any(|y| {
                let Some(t) = self.cave_cols.get(&(face as u8, x, y)) else { return false };
                t.cols.windows(2).any(|w| {
                    let (a, c) = (x * CS + (w[0].0 as i32 % CS), y * CS + (w[0].0 as i32 / CS));
                    (a - i).abs() <= r && (c - j).abs() <= r && open(face, a, c, &t.spans[w[0].1 as usize..w[1].1 as usize])
                })
            })
        })
    }
    /// `over_cave` as `candidate` asked it before: every column of the square (the test's reference).
    #[cfg(test)]
    pub(crate) fn over_cave_slow(&self, face: usize, i: i32, j: i32, r: i32) -> bool {
        let inside = i - r >= 0 && j - r >= 0 && i + r < self.n && j + r < self.n;
        let tiles = |v: i32| (v - r).div_euclid(CS)..=(v + r).div_euclid(CS);
        if inside && tiles(i).all(|x| tiles(j).all(|y| self.tile_caves(face as u8, x, y).next().is_none())) {
            return false;
        }
        for (di, dj) in (-r..=r).flat_map(|a| (-r..=r).map(move |c| (a, c))) {
            let (g, a, c) = fold(face, i + di, j + dj, self.n);
            let caves = self.col_caves_slow(g as u8, a, c);
            if caves.is_empty() {
                continue;
            }
            let (ci, cj) = (a as f64 + 0.5, c as f64 + 0.5);
            let top = self.first_air(self.height_at(g, ci, cj, self.dir(g, ci, cj), 0)) - 1;
            if caves.iter().any(|&(lo, hi)| (lo..hi).contains(&top)) {
                return true;
            }
        }
        false
    }
    /// Asserts `over_cave` is `over_cave_slow` round every cave (every square radius up to 7, at its middle and
    /// its rim), at the seams and over the faces; the squares over a mouth found.
    #[cfg(test)]
    pub(crate) fn check_over_cave(&self) -> usize {
        let mut at: Vec<(usize, i32, i32)> = Vec::new();
        for (k, c) in self.caves.iter().enumerate().filter(|(k, _)| k % 3 == 0) {
            let (ci, cj, e) = (c.at.x.floor() as i32, c.at.y.floor() as i32, c.r.ceil() as i32 + 2);
            at.extend([(0, 0), (e, 0), (-e, 1), (1, e), (-e, -e), (e + 3, -2)].map(|(a, b)| (c.face as usize, ci + a + k as i32 % 2, cj + b)));
        }
        for f in 0..6 {
            for k in (0..self.n).step_by(11) {
                at.extend([(f, 0, k), (f, 3, k), (f, self.n - 2, k), (f, k, 1), (f, k, self.n - 5), (f, k, (k * 7) % self.n)]);
            }
        }
        let mut found = 0;
        for (n, &(f, i, j)) in at.iter().enumerate() {
            for r in [n as i32 % 8, (n as i32 + 3) % 8] {
                let slow = self.over_cave_slow(f, i, j, r);
                found += slow as usize;
                assert_eq!(self.over_cave(f, i, j, r), slow, "face {f} ({i}, {j}) radius {r}");
            }
        }
        found
    }
    /// Asserts `col_caves` is `col_caves_slow`'s union on the columns round every cave (a ring past its reach),
    /// both sides of every seam on a band, and a spread over each face; the columns with a cave checked.
    #[cfg(test)]
    pub(crate) fn check_col_caves(&self) -> usize {
        let union = |mut v: Vec<(i32, i32)>| {
            v.sort_unstable();
            let mut out: Vec<(i32, i32)> = Vec::new();
            for (lo, hi) in v {
                match out.last_mut() {
                    Some(l) if lo <= l.1 => l.1 = l.1.max(hi),
                    _ => out.push((lo, hi)),
                }
            }
            out
        };
        let mut cols: Vec<(u8, i32, i32)> = Vec::new();
        for c in &self.caves {
            let r = c.r.ceil() as i32 + 3;
            let (ci, cj) = (c.at.x.floor() as i32, c.at.y.floor() as i32);
            cols.extend((ci - r..=ci + r).flat_map(|i| (cj - r..=cj + r).map(move |j| (c.face, i, j))));
        }
        for f in 0..6u8 {
            for k in (0..self.n).step_by(7) {
                cols.extend([(f, -1, k), (f, 0, k), (f, self.n - 1, k), (f, self.n, k), (f, k, -1), (f, k, self.n)]);
                cols.extend((0..self.n).step_by(13).map(|j| (f, k, j)));
            }
        }
        cols.sort_unstable();
        cols.dedup();
        let mut hollow = 0;
        for (f, i, j) in cols {
            let slow = self.col_caves_slow(f, i, j);
            hollow += !slow.is_empty() as usize;
            assert_eq!(self.col_caves(f, i, j), union(slow), "column ({f}, {i}, {j})");
        }
        hollow
    }
    /// The caves that reach level-0 footprint (x, y) of a face.
    pub(super) fn tile_caves(&self, face: u8, x: i32, y: i32) -> impl Iterator<Item = &Cave> {
        self.cave_tiles.get(&(face, x, y)).into_iter().flatten().map(|&k| &self.caves[k as usize])
    }
    /// Puts these caves in the ground, in place of any before.
    pub fn set_caves(&mut self, caves: Vec<Cave>) {
        self.cave_tiles.clear();
        for (k, c) in caves.iter().enumerate() {
            let span = |v: f64| ((v - c.r - 2.0).floor() as i32).div_euclid(CS)..=((v + c.r + 2.0).floor() as i32).div_euclid(CS);
            for x in span(c.at.x) {
                for y in span(c.at.y) {
                    self.cave_tiles.entry((c.face, x, y)).or_default().push(k as u32);
                }
            }
        }
        // each cave's spans into its columns' footprints (a span needs |i + 0.5 - x| < r: inside `cells`' square,
        // and inside the footprints `cave_tiles` gives it), then each column's merged
        let mut cols: FxMap<(u8, i32, i32), Vec<(u16, i32, i32)>> = FxMap::default();
        for c in &caves {
            let r = c.r.ceil() as i32 + 1;
            let (ci, cj) = (c.at.x.floor() as i32, c.at.y.floor() as i32);
            for (i, j) in (ci - r..=ci + r).flat_map(|i| (cj - r..=cj + r).map(move |j| (i, j))) {
                if let Some((lo, hi)) = c.span(i, j) {
                    let (x, y) = (i.div_euclid(CS), j.div_euclid(CS));
                    cols.entry((c.face, x, y)).or_default().push((((i - x * CS) + (j - y * CS) * CS) as u16, lo, hi));
                }
            }
        }
        self.cave_cols = cols
            .into_iter()
            .map(|(k, mut v)| {
                v.sort_unstable();
                let mut t = CaveCols::default();
                for (col, lo, hi) in v {
                    match t.spans.last_mut() {
                        Some(last) if t.cols.last().is_some_and(|c| c.0 == col) && lo <= last.1 => last.1 = last.1.max(hi),
                        _ => {
                            if t.cols.last().is_none_or(|c| c.0 != col) {
                                t.cols.push((col, t.spans.len() as u32));
                            }
                            t.spans.push((lo, hi));
                        }
                    }
                }
                t.cols.push((u16::MAX, t.spans.len() as u32));
                (k, t)
            })
            .collect();
        self.caves = caves;
    }
    /// Puts these ore bodies in the ground, in place of any before.
    pub fn set_ores(&mut self, ores: Vec<crate::ore::OreBody>) {
        self.ore_tiles.clear();
        // a body's halo reaches past it (ADR 0095)
        let reach = |o: &crate::ore::OreBody| crate::ore::MINERALS.kinds[o.kind as usize].halo_m * (1.0 + crate::ore::FRAY);
        for (k, o) in ores.iter().enumerate() {
            // and its samples' spread on the ground over it
            let r = o.a.max(o.b).max(crate::ore::SAMPLE_SPREAD_M) + 2.0 + reach(o);
            let span = |v: f64| ((v - r).floor() as i32).div_euclid(CS)..=((v + r).floor() as i32).div_euclid(CS);
            for x in span(o.at.x) {
                for y in span(o.at.y) {
                    self.ore_tiles.entry((o.face, x, y)).or_default().push(k as u32);
                }
            }
        }
        self.ore_z = ores
            .iter()
            .map(|o| if crate::ore::MINERALS.kinds[o.kind as usize].shape.follows_ground() { (self.layer_of((self.radius - AMP - o.at.z - o.h).max(self.r0)) - 1.0, self.layer_of(self.radius + AMP) + 1.0) } else { (o.at.z - o.h - 1.0 - reach(o), o.at.z + o.h + 1.0 + reach(o)) })
            .collect();
        self.ores = ores;
    }
    /// The ore bodies that reach level-0 footprint (x, y) of a face, by index.
    pub(crate) fn tile_ores(&self, face: u8, x: i32, y: i32) -> &[u32] {
        self.ore_tiles.get(&(face, x, y)).map_or(&[], |v| v.as_slice())
    }
    /// The ore spans [lo, hi) of column (face, i, j), whose ground is at radius `ground`, and their bodies,
    /// first body first. A cell in one is ore only where it is ground: not over the ground, not a cave.
    pub fn col_ores(&self, face: u8, i: i32, j: i32, ground: f64) -> Vec<(i32, i32, u32)> {
        let mut out = Vec::new();
        for &k in self.tile_ores(face, i.div_euclid(CS), j.div_euclid(CS)) {
            if let Some((lo, hi)) = self.ores[k as usize].span(self, i, j, ground) {
                out.push((lo, hi, k));
            }
        }
        out
    }
    /// The ore body whose ore generated layer z of column (face, i, j) is, under ground at radius `ground`:
    /// the one rule `World::generated_under`, `generate` and the ledger's count share.
    pub fn ore_at(&self, face: u8, i: i32, j: i32, z: i32, ground: f64) -> Option<u32> {
        if z < 0 || self.tile_ores(face, i.div_euclid(CS), j.div_euclid(CS)).is_empty() || ground < self.layer_r(z as f64 + 0.5) {
            return None;
        }
        let k = self.col_ores(face, i, j, ground).into_iter().find(|&(lo, hi, _)| (lo..hi).contains(&z))?.2;
        (!self.col_caves(face, i, j).iter().any(|&(lo, hi)| (lo..hi).contains(&z))).then_some(k)
    }
    /// The halo of altered rock that layer z of column (face, i, j) is (ADR 0095), as (body, grade): the
    /// highest grade any body's halo gives the cell, the first body on a tie. The one rule `generated_under`,
    /// `generate` and the report share; they ask it only of rock under the soil band that is no ore or cave.
    pub fn halo_at(&self, face: u8, i: i32, j: i32, z: i32) -> Option<(u32, usize)> {
        let mut best: Option<(u32, usize)> = None;
        for &k in self.tile_ores(face, i.div_euclid(CS), j.div_euclid(CS)) {
            // (its fraying hashed by the layer of the grid without a mantle: the same halo on either)
            if let Some(g) = self.ores[k as usize].halo_grade(face, i, j, z, self.mantle) {
                if best.is_none_or(|(_, b)| g > b) {
                    best = Some((k, g));
                }
            }
        }
        best
    }
    /// The block of grade g of body k's halo.
    pub fn halo_block(&self, k: u32, g: usize) -> u16 {
        crate::ore::MINERALS.kinds[self.ores[k as usize].kind as usize].halo[g]
    }
    /// The ore block of body k.
    pub fn ore_block(&self, k: u32) -> u16 {
        crate::ore::MINERALS.kinds[self.ores[k as usize].kind as usize].block
    }
}

/// The lava flows' noise at q (`Body::flow`): the folds' noise, moved off them.
fn flow_noise(b: &Baked, q: DVec3) -> f32 {
    flow_noise_of(&b.fold, q)
}

fn flow_noise_of(fold: &FastNoiseLite, q: DVec3) -> f32 {
    fold.get_noise_3d(q.x as f32 - 263.0, q.y as f32 + 721.0, q.z as f32 - 449.0)
}

/// Direction k of n spread evenly over the sphere (a Fibonacci lattice).
fn fibonacci(k: usize, n: usize) -> DVec3 {
    let z = 1.0 - (2.0 * k as f64 + 1.0) / n as f64;
    let a = k as f64 * PI * (3.0 - 5f64.sqrt());
    let s = (1.0 - z * z).sqrt();
    DVec3::new(s * a.cos(), s * a.sin(), z)
}
