//! World: blocks, generated ground, water steps, edits and hashes.
use super::*;

impl World {
    /// The block of the nearest body that holds world point p.
    pub fn block_at(&self, p: DVec3) -> Block {
        let (bi, face, g) = self.grid_at(p);
        let n = self.bodies[bi].n;
        let col = |c: f64| (c.floor() as i32).clamp(0, n - 1);
        Block { body: bi as u8, face: face as u8, pos: IVec3::new(col(g.x), col(g.y), g.z.floor() as i32) }
    }
    /// Where world point p lies in the grid of the nearest body: (body, face, (column i, column j, layer)) with
    /// fractions, so column 3.5 is the middle of column 3. The columns are not clamped to the face.
    pub fn grid_at(&self, p: DVec3) -> (usize, usize, DVec3) {
        let bi = self.nearest(p);
        let b = &self.bodies[bi];
        let v = p - b.centre;
        let a = v.abs();
        let face = if a.x >= a.y && a.x >= a.z {
            (v.x < 0.0) as usize
        } else if a.y >= a.z {
            2 + (v.y < 0.0) as usize
        } else {
            4 + (v.z < 0.0) as usize
        };
        // `Body::locate`'s columns (the same bits as this worked out for itself before the warp, ADR 0131)
        let (_, i, j) = b.locate(v);
        (bi, face, DVec3::new(i, j, b.layer_of(v.length())))
    }
    /// World position of the middle of a block.
    pub fn block_centre(&self, b: Block) -> DVec3 {
        let body = &self.bodies[b.body as usize];
        let p = b.pos.as_dvec3() + 0.5;
        body.centre + body.dir(b.face as usize, p.x, p.y) * body.layer_r(p.z)
    }
    pub fn generated_block(&self, b: Block) -> u16 {
        self.generated_under(b, self.column_height(b))
    }
    /// The terrain's surface radius over block b's column: the costly part of `generated_block`, the same for
    /// every layer of a column.
    /// The first air cell over the ground (edits and plants counted) along direction d from body `bi`'s centre:
    /// where a model, an effect or a structure stands. The height along d can fall inside the top block, or over
    /// it (the column's own height is at its centre): walked up to air, then down onto the ground, 16 at most.
    pub fn air_over_ground(&self, bi: usize, d: DVec3) -> Block {
        let pl = &self.bodies[bi];
        let mut c = self.block_at(pl.centre + d * (pl.height(d) + 0.1));
        let at = |c: Block, dz: i32| self.block(Block { pos: c.pos + IVec3::Z * dz, ..c });
        for _ in 0..16 {
            if at(c, 0) == AIR {
                break;
            }
            c.pos.z += 1;
        }
        for _ in 0..16 {
            if at(c, -1) != AIR || c.pos.z <= 0 {
                break;
            }
            c.pos.z -= 1;
        }
        c
    }
    pub fn column_height(&self, b: Block) -> f64 {
        self.bodies[b.body as usize].col_height(b.face, b.pos.x, b.pos.y)
    }
    /// The generated material of block b under a surface at radius `height` (its `column_height`).
    pub fn generated_under(&self, b: Block, height: f64) -> u16 {
        let body = &self.bodies[b.body as usize];
        let depth = height - body.layer_r(b.pos.z as f64 + 0.5);
        if depth < 0.0 {
            // over the ground: a plant's block, or air (a crown can hang over lower ground: `PLANT_TOP`)
            if body.baked.is_none() || -depth > PLANT_TOP {
                return AIR;
            }
            return body.col_plants(b.face, b.pos.x, b.pos.y).iter().find(|p| p.0 == b.pos.z).map_or(AIR, |p| p.1);
        }
        if body.col_caves(b.face, b.pos.x, b.pos.y).iter().any(|&(lo, hi)| (lo..hi).contains(&b.pos.z)) {
            return AIR;
        }
        if let Some(k) = body.ore_at(b.face, b.pos.x, b.pos.y, b.pos.z, height) {
            return body.ore_block(k);
        }
        let ground = body.col_ground(b.face, b.pos.x, b.pos.y, Some(height));
        if b.pos.z < 0 {
            return ground[2];
        }
        if depth >= 4.0 {
            // under the band: the rock, by the rule the mesher's fill asks too, altered round a body (ADR 0095)
            if let Some((k, g)) = body.halo_at(b.face, b.pos.x, b.pos.y, b.pos.z) {
                return body.halo_block(k, g);
            }
            let r = body.layer_r(b.pos.z as f64 + 0.5) as f32;
            return ROCKS.at(body.col_rock(b.face, b.pos.x, b.pos.y), r, ground[2]);
        }
        material(depth, &ground)
    }
    /// Whether `generated_under` is solid, without asking rock or ore under the soil band (always solid): the
    /// physics' question, per cell.
    pub fn generated_solid_under(&self, b: Block, height: f64) -> bool {
        let body = &self.bodies[b.body as usize];
        let solid = if height - body.layer_r(b.pos.z as f64 + 0.5) < 4.0 {
            BLOCKS.solid(self.generated_under(b, height))
        } else {
            !body.col_caves(b.face, b.pos.x, b.pos.y).iter().any(|&(lo, hi)| (lo..hi).contains(&b.pos.z))
        };
        debug_assert_eq!(solid, BLOCKS.solid(self.generated_under(b, height)), "{b:?}");
        solid
    }
    /// The column that holds block b.
    pub fn col(b: Block) -> Col {
        Col::new(b.body, b.face, b.pos.x, b.pos.y)
    }
    /// The solid world as the water sees it.
    pub fn ground(&self) -> Solid<'_> {
        Solid::at(&self.bodies, &self.edits, &self.sky, self.time)
    }
    /// The planet's tide now, to the frame (`Solid` takes it at the whole second): what the sea's shader is given.
    pub fn tide(&self) -> Tide {
        self.sky.tide(self.time, &self.bodies[0].ocean)
    }
    /// Steps the water once (1/60 s).
    pub fn step_water(&mut self) {
        let World { bodies, edits, water, sky, time, .. } = self;
        water.step(&Solid::at(bodies, edits, sky, *time));
    }
    /// Block b changed: its column's water finds its new gaps.
    pub fn regap(&mut self, b: Block) {
        let World { bodies, edits, water, sky, time, .. } = self;
        water.regap(&Solid::at(bodies, edits, sky, *time), World::col(b));
    }
    /// The radius of the water surface over world point p, if p is under water.
    pub fn water_surface(&self, p: DVec3) -> Option<f64> {
        let (bi, _, g) = self.grid_at(p);
        let b = self.block_at(p);
        let l = self.water.surface_at(&self.ground(), World::col(b), g.z)?;
        Some(self.bodies[bi].layer_r(l))
    }
    /// Whether the water in cell b is the sea's, salt (docs/design/needs.md): the generated sea is; the bake's
    /// rivers and lakes and water poured on land are fresh.
    /// shortcut: told from generation, not carried by the water, so sea water led or poured inland is fresh;
    /// a bit on the water's columns once water is carried far (pipes, the pot) or mixes
    pub fn salt(&self, b: Block) -> bool {
        let (g, c) = (self.ground(), World::col(b));
        let (lo, hi) = g.generated(c);
        lo < hi && g.reach(c) == crate::liquid::SEA
    }
    pub fn edit(&self, b: Block) -> Option<u16> {
        self.edits.get(&b.chunk()).and_then(|m| m.get(&b.pos)).copied()
    }
    pub fn block(&self, b: Block) -> u16 {
        self.edit(b).unwrap_or_else(|| self.generated_block(b))
    }
    pub fn set_block(&mut self, b: Block, m: u16) {
        self.tally(b, m);
        let old = self.edits.entry(b.chunk()).or_default().insert(b.pos, m);
        if old.is_none() {
            self.edit_count += 1;
        }
        self.far_edit(b, old, Some(m));
        let light = BLOCKS.get(m).map_or([0.0; 3], |d| d.light);
        let changed = if light != [0.0; 3] { self.lights.insert(b, light) != Some(light) } else { self.lights.remove(&b).is_some() };
        self.lights_rev += changed as u64;
        if BLOCKS.get(m).is_some_and(|d| d.model.is_some()) {
            self.models.insert(b, m);
        } else {
            self.models.remove(&b);
        }
    }
    /// The block rows changed under a running world (`live::apply`, ADR 0134): what `set_block` derived from
    /// them per placed block, its light and whether it is drawn as a model, is derived again.
    /// shortcut: every edit is walked on the caller's thread (about a millisecond per 50,000 edits); on a
    /// worker when a reload must not cost a frame.
    pub fn refresh_defs(&mut self) {
        let (mut lights, mut models) = (FxMap::default(), FxMap::default());
        for (k, cells) in &self.edits {
            for (p, &m) in cells {
                let Some(d) = BLOCKS.get(m) else { continue };
                let b = Block { body: k.body, face: k.face, pos: *p };
                if d.light != [0.0; 3] {
                    lights.insert(b, d.light);
                }
                if d.model.is_some() {
                    models.insert(b, m);
                }
            }
        }
        if lights != self.lights {
            self.lights = lights;
            self.lights_rev += 1;
        }
        self.models = models;
    }
    /// Puts back what `edit` returned: an edit, or (None) the generated block with no edit recorded.
    pub fn put_edit(&mut self, b: Block, e: Option<u16>) {
        match e {
            Some(m) => self.set_block(b, m),
            None => {
                if self.edit(b).is_some() {
                    let g = self.generated_block(b);
                    self.tally(b, g);
                }
                let k = b.chunk();
                if let Some(cells) = self.edits.get_mut(&k) {
                    let old = cells.remove(&b.pos);
                    if cells.is_empty() {
                        self.edits.remove(&k);
                    }
                    if old.is_some() {
                        self.edit_count -= 1;
                        self.lights_rev += self.lights.remove(&b).is_some() as u64;
                        self.models.remove(&b);
                        self.far_edit(b, old, None);
                    }
                }
            }
        }
    }
    /// One hash per level 0 chunk that holds edits, in chunk order: two worlds from the same seed agree where
    /// these agree, and the first difference names the chunk where they part.
    pub fn chunk_hashes(&self) -> Vec<(Key, u64)> {
        let mut v: Vec<(Key, u64)> = self
            .edits
            .iter()
            .map(|(k, cells)| {
                let mut c: Vec<(&IVec3, &u16)> = cells.iter().collect();
                c.sort_unstable_by_key(|(p, _)| (p.x, p.y, p.z));
                // an id under 256 hashes as its one byte, so recordings made with 8-bit ids still replay
                (*k, c.iter().fold(FNV0, |h, (p, m)| fnv(fnv_ivec(h, **p), &m.to_le_bytes()[..1 + (**m > 255) as usize])))
            })
            .collect();
        v.sort_unstable_by_key(|(k, _)| (k.body, k.face, k.level, k.pos.x, k.pos.y, k.pos.z));
        v
    }
    /// The whole world's state as one number.
    pub fn hash(&self) -> u64 {
        self.chunk_hashes().iter().fold(FNV0, |h, (k, c)| fnv(fnv_ivec(fnv(h, &[k.body, k.face, k.level]), k.pos), &c.to_le_bytes()))
    }
    /// The edits a chunk's padded cells show, as (padded cell index, material): at level 0 the edits, coarser
    /// each cell's pick (ADR 0109). Mesh jobs take `far_snap` and gather on the worker.
    pub fn edits_for(&self, k: Key) -> Vec<(u32, u16)> {
        self.far_snap(k).edits()
    }
}
