//! Far chunks' snapshots of the edits and picks they read (ADR 0109).
use super::*;

impl World {
    /// The shared picks chunk `k`'s padded cells read: its own chunk's, its 26 neighbours' and, at a seam, the
    /// neighbouring face's (ADR 0109). A few dozen lookups; the gathering is `FarSnap::edits`, on the worker.
    pub fn far_snap(&self, k: Key) -> FarSnap {
        let nl = self.bodies[k.body as usize].n >> k.level;
        let mut maps = Vec::new();
        if self.far.as_ref().map_or(self.edits.is_empty(), |f| f.is_empty()) {
            return FarSnap { key: k, nl, maps };
        }
        // without the summary (a server's World) level 0 reads the edits, a copy each; coarser levels none
        let get = |kk: &Key| match &self.far {
            Some(f) => f.get(kk).map(|c| c.picks.clone()),
            None if kk.level == 0 => self.edits.get(kk).map(|m| Arc::new(m.clone())),
            None => None,
        };
        let mut take = |kk: Key| {
            if !maps.iter().any(|(m, _)| *m == kk) {
                if let Some(p) = get(&kk) {
                    maps.push((kk, p));
                }
            }
        };
        for i in 0..27 {
            take(Key { pos: k.pos + IVec3::new(i % 3 - 1, i / 3 % 3 - 1, i / 9 - 1), ..k });
        }
        let c0 = k.pos * CS - 1;
        let pm = P as i32 - 1;
        let edges = [(c0.x < 0, true, 0), (c0.x + pm >= nl, true, pm), (c0.y < 0, false, 0), (c0.y + pm >= nl, false, pm)];
        for (_, along_y, at) in edges.into_iter().filter(|e| e.0) {
            for t in 1..pm {
                let (x, y) = if along_y { (at, t) } else { (t, at) };
                let (f, ci, cj) = fold(k.face as usize, c0.x + x, c0.y + y, nl);
                for z in [c0.z, c0.z + 1, c0.z + pm] {
                    take(Key { face: f as u8, pos: IVec3::new(ci, cj, z).div_euclid(IVec3::splat(CS)), ..k });
                }
            }
        }
        FarSnap { key: k, nl, maps }
    }
    /// Opens the far cells' summary (the client's, ADR 0109), from the edits already made.
    pub fn open_far(&mut self) {
        self.far = Some(FxMap::default());
        let edits: Vec<(Block, u16)> = self.edits.iter().flat_map(|(k, c)| c.iter().map(move |(p, m)| (Block { body: k.body, face: k.face, pos: *p }, *m))).collect();
        for (b, m) in edits {
            self.far_edit(b, None, Some(m));
        }
    }
    /// The levels whose far cells block b's last edit changed, as bits (level 0 always): the chunks
    /// `Streamer::touch_block` meshes again. All when not known (no summary, or the edit was taken back).
    pub fn far_levels(&self, b: Block) -> u32 {
        self.far.as_ref().and_then(|f| f.get(&b.chunk())?.changed.get(&b.pos).copied()).map_or(!0, |m| m | 1)
    }
    /// Block b's edit went from `old` to `new`: the cell that holds it at every level adds it up again, and the
    /// levels whose pick changed are kept for `far_levels` (ADR 0109).
    pub(super) fn far_edit(&mut self, b: Block, old: Option<u16>, new: Option<u16>) {
        let root = self.bodies[b.body as usize].root_level;
        let Some(far) = self.far.as_mut() else { return };
        let mut changed = 0u32;
        for level in (0..=root).filter(|_| old != new) {
            let s = 1i32 << level;
            let cell = b.pos.div_euclid(IVec3::splat(s));
            let k = Key { body: b.body, face: b.face, level, pos: cell.div_euclid(IVec3::splat(CS)) };
            let fc = far.entry(k).or_default();
            let pick = if level == 0 {
                if new.is_none() {
                    fc.changed.remove(&b.pos);
                }
                new
            } else {
                let sum = fc.cells.entry(cell).or_default();
                if b.pos.rem_euclid(IVec3::splat(s)) == IVec3::splat(s / 2) {
                    sum.mid = new.filter(|&m| far_mid(&BLOCKS, m));
                }
                if let Some(m) = old.filter(|&m| far_counts(&BLOCKS, m)) {
                    sum.count(m, -1);
                }
                if let Some(m) = new.filter(|&m| far_counts(&BLOCKS, m)) {
                    sum.count(m, 1);
                }
                let pick = sum.pick(s);
                if sum.mid.is_none() && sum.counts.is_empty() {
                    fc.cells.remove(&cell);
                }
                pick
            };
            if fc.picks.get(&cell).copied() != pick {
                changed |= 1 << level;
                let p = Arc::make_mut(&mut fc.picks);
                match pick {
                    Some(m) => p.insert(cell, m),
                    None => p.remove(&cell),
                };
            }
            if fc.cells.is_empty() && fc.picks.is_empty() {
                far.remove(&k);
            }
        }
        if new.is_some() {
            far.get_mut(&b.chunk()).expect("an edit's level 0 chunk").changed.insert(b.pos, changed);
        }
    }
}

impl FarSnap {
    pub fn is_empty(&self) -> bool {
        self.maps.is_empty()
    }
    /// The edits the chunk's padded cells show, as (padded cell index, material).
    pub fn edits(&self) -> Vec<(u32, u16)> {
        let mut out = Vec::new();
        let k = self.key;
        let c0 = k.pos * CS - 1; // padded cell (0, 0, 0) in cells of this level
        for (kk, m) in self.maps.iter().filter(|(kk, _)| kk.face == k.face) {
            // the own chunk's cells all lie inside; a neighbour's only along the shared side
            let own = kk.pos == k.pos;
            for (c, v) in m.iter() {
                let q = *c - c0;
                if own || (q.cmpge(IVec3::ZERO).all() && q.cmplt(IVec3::splat(P as i32)).all()) {
                    out.push((pidx(q.x as usize, q.y as usize, q.z as usize) as u32, *v));
                }
            }
        }
        // padded strips past a seam read the neighbouring face's cells
        let pm = P as i32 - 1;
        let mut strip = |x: i32, y: i32| {
            let (f, ci, cj) = fold(k.face as usize, c0.x + x, c0.y + y, self.nl);
            for z in 0..P as i32 {
                let c = IVec3::new(ci, cj, c0.z + z);
                let at = c.div_euclid(IVec3::splat(CS));
                if let Some(m) = self.maps.iter().find(|(kk, _)| kk.face == f as u8 && kk.pos == at).and_then(|(_, m)| m.get(&c)) {
                    out.push((pidx(x as usize, y as usize, z as usize) as u32, *m));
                }
            }
        };
        for (x, out_of_face) in [(0, c0.x < 0), (pm, c0.x + pm >= self.nl)] {
            if out_of_face {
                (1..pm).for_each(|y| strip(x, y));
            }
        }
        for (y, out_of_face) in [(0, c0.y < 0), (pm, c0.y + pm >= self.nl)] {
            if out_of_face {
                (1..pm).for_each(|x| strip(x, y));
            }
        }
        out
    }
}
