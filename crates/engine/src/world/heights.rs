//! Body: column heights and plants, layers, directions, height tiles and chunk bounds.
use super::*;

impl Body {
    /// The surface radius at face point (i, j) in columns, as level 0 makes it.
    pub fn ground_r(&self, face: usize, i: f64, j: f64) -> f64 {
        self.height_at(face, i, j, self.dir(face, i, j), 0)
    }
    /// The first generated air layer over a surface at radius h: `material`'s own test.
    pub(crate) fn first_air(&self, h: f64) -> i32 {
        let mut z = ((self.layer_of(h) - 0.5).floor() as i32 + 1).max(0);
        while z > 0 && material(h - self.layer_r(z as f64 - 0.5), &self.ground) == AIR {
            z -= 1;
        }
        while material(h - self.layer_r(z as f64 + 0.5), &self.ground) != AIR {
            z += 1;
        }
        z
    }
    /// The plant blocks over level-0 column (i, j) of `face`, (layer, block) by layer, from the candidate spots
    /// (face, cx, cy) `cand` gives (`candidate`, or a cache of it): the one function the mesher, physics and the
    /// water read plants through (ADR 0043). Where two plants meet, the first in spot order wins, wood over
    /// leaves. Near a seam the next face's trees reach over it, in their own face's grid (ADR 0100).
    pub fn plants_with(&self, face: usize, i: i32, j: i32, cand: &mut impl FnMut(usize, i32, i32) -> Option<Foot>) -> Vec<(i32, u16)> {
        let mut out: Vec<(i32, u16, bool)> = Vec::new();
        self.feet_over(face, i, j, cand, |f, p, dx, dz| {
            p.column(f.h, f.v, dx, dz, |z, (m, wood)| match out.iter_mut().find(|o| o.0 == f.z + z) {
                Some(o) => {
                    if wood && !o.2 {
                        *o = (f.z + z, m, wood);
                    }
                }
                None => out.push((f.z + z, m, wood)),
            });
        });
        if let Some((z, m)) = self.ice_sheet(face, i, j) {
            out.push((z, m, false));
        }
        out.sort_unstable_by_key(|o| o.0);
        out.into_iter().map(|o| (o.0, o.1)).collect()
    }
    /// The sheet on the sea or a lake of level-0 column (i, j) (ADR 0107): its water's top layer and its block
    /// where the column's biome is ice and the water stands over the ground, None elsewhere (a river runs open).
    /// The sheet fills the layer the surface lies in, an eighth of a block proud of the sea; its block is the ice
    /// biome's top as on land (snow, patches of bare ice), so the cap reads white from orbit.
    pub(crate) fn ice_sheet(&self, face: usize, i: i32, j: i32) -> Option<(i32, u16)> {
        let b = self.baked.as_ref()?;
        // maps before the caps (and Luna's) keep their ground to the bit: no sheet on an old map's icy lakes
        if !(7..100).contains(&b.map.version) {
            return None;
        }
        let s = b.map.m as f64 / self.n as f64;
        let (ci, cj) = (i as f64 + 0.5, j as f64 + 0.5);
        // most columns: no ice cell within the biome's jitter
        let ice = |di: f64, dj: f64| {
            let (x, y) = self.map_xy(s, ci + di, cj + dj);
            b.map.biome(face, x, y) == crate::bake::life::ICE
        };
        if !(ice(-JITTER, -JITTER) || ice(JITTER, -JITTER) || ice(-JITTER, JITTER) || ice(JITTER, JITTER) || ice(0.0, 0.0)) {
            return None;
        }
        let d = self.dir(face, ci, cj);
        let (x, y) = self.jittered(b, ci, cj, d);
        let (mx, my) = self.map_xy(s, x, y);
        if b.map.biome(face, mx, my) != crate::bake::life::ICE {
            return None;
        }
        let (top, _, river) = self.water_at(face as u8, i, j)?;
        let z = top.floor() as i32;
        if river.is_some() || self.height_at(face, ci, cj, d, 0) >= self.layer_r(z as f64) {
            return None;
        }
        // the patches' roll of `ground_biome_at`
        let cell = ((x / PATCH).floor() as i64 as u64) ^ ((y / PATCH).floor() as i64 as u64).wrapping_mul(0x9e3779b97f4a7c15);
        let roll = unit(mix(b.seed ^ cell ^ (face as u64) << 56));
        Some((z, ROWS.biomes[crate::bake::life::ICE as usize].top(roll, 0.0)))
    }
    /// The highest of `plants_with`'s blocks, `plants_with(..).last()` to the bit (the test keeps that as its
    /// reference): each plant's column is walked down from its top and stops at its first block (ADR 0102).
    pub fn plant_top_with(&self, face: usize, i: i32, j: i32, cand: &mut impl FnMut(usize, i32, i32) -> Option<Foot>) -> Option<(i32, u16)> {
        let mut best: Option<(i32, u16, bool)> = None;
        self.feet_over(face, i, j, cand, |f, p, dx, dz| {
            // (a corner of the square outside the round reach has no block: no crown built for it)
            if (dx * dx + dz * dz) as f32 > (p.radius * p.radius) as f32 + 0.5 {
                return;
            }
            let crown = p.shape_of(f.h, f.v);
            if let Some((z, (m, wood))) = (0..=f.h).rev().find_map(|z| p.cell(&crown, f.h, f.v, dx, dz, z).map(|b| (f.z + z, b))) {
                // the first plant at a layer keeps it unless a later one is wood over its leaves, as in `plants_with`
                if best.is_none_or(|o| z > o.0 || z == o.0 && wood && !o.2) {
                    best = Some((z, m, wood));
                }
            }
        });
        best.map(|o| (o.0, o.1))
    }
    /// Each plant (its foot, row and the column's offset from its trunk) whose square reaches level-0 column
    /// (i, j) of `face`, in spot order: `plants_with`'s walk.
    fn feet_over(&self, face: usize, i: i32, j: i32, cand: &mut impl FnMut(usize, i32, i32) -> Option<Foot>, mut visit: impl FnMut(&Foot, &crate::biomes::Plant, i32, i32)) {
        if self.baked.is_none() {
            return;
        }
        let n = self.n;
        let near = i < REACH || j < REACH || i >= n - REACH || j >= n - REACH;
        // this face's spots and each next face's whose grid, carried over the seam, has this column within reach;
        // by face, so both sides of a seam pick the same plant where two meet
        let grids = (0..6).filter(|&g| g == face || near && AX[g][0].dot(AX[face][0]) == 0).map(|g| if g == face { (g, i, j) } else {
            let (gi, gj) = unfold(g, face, i, j, n);
            (g, gi, gj)
        });
        for (g, i, j) in grids {
            if !((-REACH..n + REACH).contains(&i) && (-REACH..n + REACH).contains(&j)) {
                continue;
            }
            for (cx, cy) in ((j - REACH).div_euclid(SPACING)..=(j + REACH).div_euclid(SPACING)).flat_map(|cy| ((i - REACH).div_euclid(SPACING)..=(i + REACH).div_euclid(SPACING)).map(move |cx| (cx, cy))) {
                let Some(f) = cand(g, cx, cy) else { continue };
                let p = &ROWS.plants[f.plant];
                let (dx, dz) = (i - f.i, j - f.j);
                if dx.abs() > p.radius || dz.abs() > p.radius {
                    continue;
                }
                visit(&f, p, dx, dz);
            }
        }
    }
    /// `plants_with` over level-0 column (i, j): from its cached tile, else computed.
    pub fn col_plants(&self, face: u8, i: i32, j: i32) -> Vec<(i32, u16)> {
        if self.baked.is_none() {
            return Vec::new();
        }
        let (x, y) = (i.div_euclid(CS), j.div_euclid(CS));
        if let Some(e) = self.tiles.map.lock().unwrap().get(&(face, 0, x, y)) {
            let k = ((i - x * CS + 1) as usize + (j - y * CS + 1) as usize * P) as u16;
            let p = &e.0.plants;
            let from = p.partition_point(|q| q.0 < k);
            return p[from..].iter().take_while(|q| q.0 == k).map(|q| (q.1, q.2)).collect();
        }
        let spots = &self.baked.as_ref().unwrap().spots;
        self.plants_with(face as usize, i, j, &mut |g, cx, cy| {
            if let Some(f) = spots.lock().unwrap().get(&(g as u8, cx, cy)) {
                return *f;
            }
            // looked at outside the lock; two threads may both, to the same result
            let f = self.candidate(g, cx, cy);
            let mut m = spots.lock().unwrap();
            if m.len() >= SPOTS {
                m.clear();
            }
            m.insert((g as u8, cx, cy), f);
            f
        })
    }
    /// The ground (`material`) of level-0 column (i, j) of a face: from its cached tile, else computed.
    /// `height`: the column's surface radius when the caller has it (`col_height`'s, to the bit).
    pub fn col_ground(&self, face: u8, i: i32, j: i32, height: Option<f64>) -> [u16; 3] {
        if self.baked.is_none() {
            return self.ground;
        }
        let (x, y) = (i.div_euclid(CS), j.div_euclid(CS));
        if let Some(e) = self.tiles.map.lock().unwrap().get(&(face, 0, x, y)) {
            return e.0.g[(i - x * CS + 1) as usize + (j - y * CS + 1) as usize * P];
        }
        let (ci, cj) = (i as f64 + 0.5, j as f64 + 0.5);
        let d = self.dir(face as usize, ci, cj);
        self.ground_at(face as usize, ci, cj, d, height.unwrap_or_else(|| self.height_at(face as usize, ci, cj, d, 0)))
    }
    /// The body with continents, oceans and a sea (ADR 0020, 0021), and its spawn on land.
    pub fn with_continents(mut self, seed: i32) -> Self {
        let land = Land::new(seed, self.radius);
        // an eighth of a block under a layer's top: a block at the waterline stands clearly out of the water
        // instead of flickering through it
        self.sea = Some(self.layer_of(land.sea_r).floor() + 0.875);
        self.land = Some(Box::new(land));
        self.ocean = self.ocean_moments();
        // the land nearest the +y pole, a little above the sea
        let sea_r = self.radius - SEA_DEPTH;
        self.spawn = lattice(4096).filter(|d| d.y > 0.0 && self.height(*d) > sea_r + 2.0).max_by(|a, b| a.y.total_cmp(&b.y)).unwrap_or(DVec3::Y);
        self
    }
    /// Where a player starts: over the spawn, clear of the highest ground within a metre, so a steep slope
    /// never holds it inside a block; it drops onto the ground from there.
    pub fn spawn_point(&self) -> DVec3 {
        self.stand_point(self.spawn)
    }
    /// Feet on the surface over direction d (from the centre): over the highest ground a step around.
    pub fn stand_point(&self, d: DVec3) -> DVec3 {
        let d = d.normalize();
        let a = d.any_orthonormal_vector();
        let b = d.cross(a);
        let mut top: f64 = 0.0;
        for (x, y) in [(0.0, 0.0), (1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0), (0.7, 0.7), (-0.7, 0.7), (0.7, -0.7), (-0.7, -0.7)] {
            top = top.max(self.height((d * self.radius + (a * x + b * y) * (1.5 * BLOCK)).normalize()));
        }
        // (a block's top stands up to half a block over its column's height)
        self.centre + d * (top + 0.6 + 0.5 * BLOCK)
    }
    /// The tide at body-frame direction d in layers over the mean sea (the sea's layers are a metre thick to
    /// within a part in a thousand).
    pub fn tide_layers(&self, t: &Tide, d: DVec3) -> f64 {
        t.at(d) / (self.layer_r(self.sea.unwrap_or(0.0)) * self.lnq)
    }
    /// The sea's level at the highest tide, a layer coordinate (`sea_quads`): in the layer over the mean's, as the
    /// mean lies an eighth under a layer's top and TIDE_HIGH is under a layer and an eighth.
    pub fn sea_high(&self) -> Option<f64> {
        self.sea.map(|s| s + crate::sky::TIDE_HIGH / (self.layer_r(s) * self.lnq))
    }
    /// The sea's `ocean`: the mean of d d^T over 4096 even directions whose ground is under the sea.
    pub(super) fn ocean_moments(&self) -> glam::DMat3 {
        let r = self.layer_r(self.sea.expect("a sea"));
        let (mut m, mut k) = (glam::DMat3::ZERO, 0usize);
        for d in lattice(4096).filter(|&d| self.height(d) < r) {
            m += glam::DMat3::from_cols(d * d.x, d * d.y, d * d.z);
            k += 1;
        }
        if k == 0 { glam::DMat3::from_diagonal(DVec3::splat(1.0 / 3.0)) } else { m * (1.0 / k as f64) }
    }
    /// The radius of the surface over direction d: the ground, or the sea over it.
    pub fn top(&self, d: DVec3) -> f64 {
        let h = self.height(d);
        self.sea.map_or(h, |s| h.max(self.layer_r(s)))
    }
    /// Root chunks per face edge.
    pub fn roots(&self) -> i32 {
        (self.n / CS) >> self.root_level
    }
    /// Radius of layer coordinate l (l = 3.5 is the middle of layer 3).
    pub fn layer_r(&self, l: f64) -> f64 {
        self.r0 * (l * self.lnq).exp()
    }
    /// The lattice, in cells, that a chunk of `level` merges its quads inside and the renderer places its vertices
    /// on (ADR 0025 I2): 8, halved while a lattice cell's sag off the sphere, (n cell)^2 / 8R, would pass half a
    /// pixel (1.1 mrad at 1080p) at the nearest distance that level is drawn from at the lowest detail, 2 chunk
    /// edges. A function of the body and level alone, so a mesh and its draw always agree.
    pub fn lattice(&self, level: u8) -> u32 {
        let cell = self.radius * std::f64::consts::FRAC_PI_2 / self.n as f64 * (1u64 << level) as f64;
        let near = 2.0 * CS as f64 * cell;
        let mut n = 8;
        while n > 1 && (n as f64 * cell).powi(2) / (8.0 * self.radius) > 0.5 * 1.1e-3 * near {
            n /= 2;
        }
        n
    }

    /// Metres point `p` is over the sea level (none on a body without a sea).
    pub fn over_sea(&self, p: DVec3) -> Option<f64> {
        Some((p - self.centre).length() - self.layer_r(self.sea?))
    }
    pub fn layer_of(&self, r: f64) -> f64 {
        (r / self.r0).ln() / self.lnq
    }
    /// Unit direction of grid point (i, j) on a face, in columns (i = 3.5 is the middle of column 3).
    pub fn dir(&self, face: usize, i: f64, j: f64) -> DVec3 {
        if self.warp == 0.0 {
            return self.dir_tan(face, self.col_tan(i), self.col_tan(j));
        }
        let (i, j) = self.warp_col(i, j);
        self.dir_tan(face, self.col_tan(i), self.col_tan(j))
    }
    /// `dir` from its two tangents: a tile computes them once per row and column, to the same bits.
    fn dir_tan(&self, face: usize, ta: f64, tb: f64) -> DVec3 {
        let [n, s, t] = AX[face];
        (n.as_dvec3() + s.as_dvec3() * ta + t.as_dvec3() * tb).normalize()
    }
    fn col_tan(&self, i: f64) -> f64 {
        let th = FRAC_PI_2 / self.n as f64;
        let h = self.n as f64 * 0.5;
        ((i - h) * th).tan()
    }
    /// `tile` if it is built already, else None: the frame thread's lookups, which never build one (principle 8).
    pub fn tile_if_built(&self, face: u8, level: u8, x: i32, y: i32) -> Option<Arc<HeightTile>> {
        self.tiles.map.lock().unwrap().get(&(face, level, x, y)).map(|e| e.0.clone())
    }
    /// The height tile under chunk footprint (face, level, x, y), built here on a miss. Only workers and the
    /// server call it: on the frame thread use `col_height`, which never builds one.
    pub fn tile(&self, face: u8, level: u8, x: i32, y: i32) -> Arc<HeightTile> {
        use std::sync::atomic::Ordering::Relaxed;
        let k = (face, level, x, y);
        let now = self.tiles.tick.fetch_add(1, Relaxed);
        if let Some(e) = self.tiles.map.lock().unwrap().get_mut(&k) {
            e.1 = now;
            return e.0.clone();
        }
        // built outside the lock; two workers may build the same tile, the result is the same
        let tile = Arc::new(self.build_tile(face as usize, level, x, y));
        let mut m = self.tiles.map.lock().unwrap();
        let mut gone = Vec::new();
        if m.len() >= HEIGHT_TILES {
            // the cut is chosen, and the old tiles freed, outside the lock: the frame thread's physics takes it
            // per column
            let mut ages: Vec<u64> = m.values().map(|e| e.1).collect();
            drop(m);
            let mid = ages.len() / 2;
            let cut = *ages.select_nth_unstable(mid).1;
            m = self.tiles.map.lock().unwrap();
            gone.extend(m.extract_if(|_, e| e.1 <= cut));
        }
        m.insert(k, (tile.clone(), now));
        drop(m);
        drop(gone);
        tile
    }
    pub(super) fn build_tile(&self, face: usize, level: u8, x: i32, y: i32) -> HeightTile {
        let s = (1i32 << level) as f64;
        let nl = self.n >> level;
        let (c0x, c0y) = (x * CS - 1, y * CS - 1);
        let ta: [f64; P] = std::array::from_fn(|k| self.col_tan(((c0x + k as i32) as f64 + 0.5) * s));
        let tb: [f64; P] = std::array::from_fn(|k| self.col_tan(((c0y + k as i32) as f64 + 0.5) * s));
        let mut t = HeightTile { h: [0.0; P * P], g: [self.ground; P * P], rock: [Rock::NONE; P * P], plants: Vec::new(), biome: [0; P * P], trees: Vec::new(), min: f32::MAX, max: f32::MIN };
        // the candidate spots the footprint's plants come from, each looked at once: this face's within reach of the
        // footprint in a grid (a hash map a lookup cost more than the far canopy's whole sample, ADR 0102), the rest
        // (over a seam) in a map; the far tiles ask only for trees
        let ask: fn(&Body, usize, i32, i32) -> Option<Foot> = if level == 0 { Body::candidate } else { Body::tree_candidate };
        let (sx, sy) = (((c0x << level) - REACH).div_euclid(SPACING), ((c0y << level) - REACH).div_euclid(SPACING));
        let sw = (((c0x + P as i32) << level) + REACH).div_euclid(SPACING) - sx + 1;
        let mut grid: Vec<Option<Option<Foot>>> = vec![None; (sw * sw) as usize];
        let mut other: FxMap<(usize, i32, i32), Option<Foot>> = FxMap::default();
        let mut spots = |g: usize, cx: i32, cy: i32| -> Option<Foot> {
            let (a, b) = (cx - sx, cy - sy);
            if g == face && (0..sw).contains(&a) && (0..sw).contains(&b) {
                *grid[(a + b * sw) as usize].get_or_insert_with(|| ask(self, g, cx, cy))
            } else {
                *other.entry((g, cx, cy)).or_insert_with(|| ask(self, g, cx, cy))
            }
        };
        for v in 0..P {
            for u in 0..P {
                let (ci, cj) = (c0x + u as i32, c0y + v as i32);
                // columns past a seam belong to the neighbouring face
                let (f, fi, fj) = fold(face, ci, cj, nl);
                // (the warped grid is not separable: its directions a point each)
                let d = if (f, fi, fj) == (face, ci, cj) && self.warp == 0.0 { self.dir_tan(face, ta[u], tb[v]) } else { self.dir(f, (fi as f64 + 0.5) * s, (fj as f64 + 0.5) * s) };
                let (pi, pj) = ((fi as f64 + 0.5) * s, (fj as f64 + 0.5) * s);
                let hv = self.height_at(f, pi, pj, d, level);
                t.min = t.min.min(hv as f32);
                t.max = t.max.max(hv as f32);
                t.h[u + v * P] = hv;
                if self.baked.is_some() {
                    (t.g[u + v * P], t.biome[u + v * P]) = self.ground_biome_at(f, pi, pj, d, hv);
                    t.rock[u + v * P] = self.rock_at(f, pi, pj, d);
                    if level == 0 {
                        for (z, m) in self.plants_with(f, fi, fj, &mut spots) {
                            t.max = t.max.max(self.layer_r(z as f64 + 1.0) as f32);
                            t.plants.push(((u + v * P) as u16, z, m));
                        }
                    } else {
                        // far chunks: the trees' canopy is the ground, as tall as the highest real tree over the
                        // coarse column's middle (level 1) or its four quarter points, with that tree's top block,
                        // so a forest reads as one (ADR 0100)
                        let q = 1 << level;
                        let at: &[(i32, i32)] = if level == 1 { &[(1, 1)] } else { &[(q / 4, q / 4), (3 * q / 4, q / 4), (q / 4, 3 * q / 4), (3 * q / 4, 3 * q / 4)] };
                        let tops = at.iter().filter_map(|&(a, c)| self.plant_top_with(f, (fi << level) + a, (fj << level) + c, &mut spots));
                        if let Some((z, m)) = tops.max_by_key(|o| o.0) {
                            let top = self.layer_r(z as f64 + 1.0);
                            // not where the coarse column's middle is under water: a lakeside crown would hide the lake
                            let wet = self.water_at(f as u8, (fi << level) + q / 2, (fj << level) + q / 2).is_some_and(|w| w.0 > self.layer_of(hv));
                            if top > hv && !wet {
                                (t.h[u + v * P], t.g[u + v * P], t.rock[u + v * P]) = (top, [m; 3], Rock::NONE);
                                t.max = t.max.max(top as f32);
                            }
                        }
                        // the ice on a polar sea or lake is the far ground, as the canopy is (ADR 0107)
                        if let Some((z, m)) = self.ice_sheet(f, (fi << level) + q / 2, (fj << level) + q / 2) {
                            // at least up to the middle of the coarse cell the sheet lies in, or that cell stays air
                            // and the far sheet sinks under the sea's surface
                            let mid = (z.div_euclid(q) as f64 + 0.5) * q as f64 + 0.01;
                            let top = self.layer_r((z as f64 + 1.0).max(mid));
                            if top > t.h[u + v * P] {
                                (t.h[u + v * P], t.g[u + v * P], t.rock[u + v * P]) = (top, [m; 3], Rock::NONE);
                                t.max = t.max.max(top as f32);
                            }
                        }
                    }
                }
            }
        }
        // the trunks among the spots looked at (this face's only: a seam's columns ask their own face)
        if level == 0 {
            let mine = grid.iter().flatten().chain(other.iter().filter(|(k, _)| k.0 == face).map(|(_, f)| f));
            t.trees = mine.flatten().filter(|f| ROWS.plants[f.plant].trunk != 0).map(|f| (f.i, f.j)).collect();
        }
        t.trees.sort_unstable();
        t
    }
    /// The surface radius over level-0 column (i, j) of a face: from its tile when one is cached, else computed,
    /// never building a tile (the frame thread's physics calls it).
    pub fn col_height(&self, face: u8, i: i32, j: i32) -> f64 {
        if self.gas {
            return self.radius;
        }
        let (x, y) = (i.div_euclid(CS), j.div_euclid(CS));
        if let Some(e) = self.tiles.map.lock().unwrap().get(&(face, 0, x, y)) {
            return e.0.h[(i - x * CS + 1) as usize + (j - y * CS + 1) as usize * P];
        }
        let (ci, cj) = (i as f64 + 0.5, j as f64 + 0.5);
        self.height_at(face as usize, ci, cj, self.dir(face as usize, ci, cj), 0)
    }
    /// Metres from `p` down to the highest place a block can be (the top of the build headroom): above
    /// zero there is nothing but air around, whatever was built or however deep the sea is under it.
    pub fn over_top(&self, p: DVec3) -> f64 {
        (p - self.centre).length() - self.r0 * (self.layers as f64 * self.lnq).exp()
    }
    /// Surface radius along unit direction d.
    pub fn height(&self, d: DVec3) -> f64 {
        // a gas body's surface is its cloud tops (step 6a)
        if self.gas {
            return self.radius;
        }
        if self.baked.is_some() {
            let (f, i, j) = self.locate(d);
            return self.height_at(f, i, j, d, 0);
        }
        let p = d * self.radius;
        if let Some(l) = &self.land {
            return l.sea_r + l.over_sea(p);
        }
        self.radius + HILLS * self.noise.get_noise_3d(p.x as f32, p.y as f32, p.z as f32) as f64
    }
    /// Bounding sphere of a chunk, clamped to the streamed layers.
    pub fn bounds(&self, k: Key) -> Bounds {
        let (b, s) = (k.base(), k.span());
        let (i0, i1, j0, j1) = (b.x as f64, (b.x + s) as f64, b.y as f64, (b.y + s) as f64);
        let r_in = self.layer_r(b.z.max(0) as f64);
        let r_out = self.layer_r((b.z + s).min(self.layers) as f64);
        let f = k.face as usize;
        let c = self.dir(f, (i0 + i1) * 0.5, (j0 + j1) * 0.5) * ((r_in + r_out) * 0.5);
        // the farthest points of the wedge from its middle are its corners
        let mut rad2 = 0f64;
        for d in [self.dir(f, i0, j0), self.dir(f, i1, j0), self.dir(f, i0, j1), self.dir(f, i1, j1)] {
            rad2 = rad2.max((d * r_in - c).length_squared()).max((d * r_out - c).length_squared());
        }
        Bounds { c: self.centre + c, rad: rad2.sqrt() }
    }
}
