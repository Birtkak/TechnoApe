//! The water mesh (ADR 0020): every tile that holds stored water, rebuilt when the server sends it. Each wet
//! column gets a top whose corners are shared with its wet neighbours (a smooth surface, so waves read as
//! waves), and a side where it stands over lower water or a dry floor (a front, a waterfall). Heights move
//! from the last state to the new one over a tick on the GPU, so 20 updates a second look continuous.
use engine::liquid::{Col, OPEN, TILE, U};
use engine::world::{FxMap, World};

/// One quad of the water mesh, as the shader reads it: column (i, j), info = face | body << 3 | kind << 5
/// (0 the top, 1..=4 a side toward -i, +i, -j, +j), the time its move starts, and its heights in layers
/// before and after: a top's corners (0,0) (1,0) (1,1) (0,1); a side's top and bottom at its two ends.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct WQuad {
    pub i: i32,
    pub j: i32,
    pub info: u32,
    pub t0: f32,
    pub prev: [f32; 4],
    pub cur: [f32; 4],
}

/// Seconds a move takes: one server tick.
pub const MOVE_S: f32 = 1.0 / engine::net::TICK_HZ as f32;
/// Neighbours this close (layers) are one surface, and share their corners.
const JOIN: f32 = 0.5;
/// A side is drawn where the water stands this much (layers) over what is next to it.
const SIDE: f32 = 1.0 / 16.0;

#[derive(Default)]
pub struct WaterMesh {
    tiles: FxMap<Col, Vec<WQuad>>,
    /// The open surface (layers) of every column in a wet tile: what it was, what it is, since when; and the
    /// floor it lies on.
    surf: FxMap<Col, (f32, f32, f32, f32)>,
    /// The quads of all tiles, rebuilt when one changed.
    pub all: Vec<WQuad>,
    pub changed: bool,
    /// Ranges of `all` rewritten in place since the last upload (when `changed` is false).
    pub dirty: Vec<std::ops::Range<usize>>,
    /// Each tile's range of `all`.
    at: FxMap<Col, std::ops::Range<usize>>,
    /// The buffer's cap cut `all` once: said once.
    pub cut_told: bool,
    /// The spray's numbers (ADR 0020, its droplets ADR 0031).
    rng: u32,
    /// The world time, in 4 s, the wet tiles were last queued for the tide at, and the queue (`retide`).
    tide_at: i64,
    tide_todo: Vec<Col>,
}

impl WaterMesh {
    /// Tiles `ts` arrived (already in `world.water`): their surfaces move from what is shown now, and they
    /// and their neighbours are meshed again.
    pub fn update(&mut self, world: &World, ts: &[Col], now: f32) {
        if ts.is_empty() {
            return;
        }
        let g = world.ground();
        let mut again: Vec<Col> = Vec::new();
        for &t in ts {
            let wet = world.water.wet(t);
            for (di, dj) in [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, 1), (-1, 1), (1, -1)] {
                if t.i() + di >= 0 && t.j() + dj >= 0 {
                    again.push(Col::new(t.body(), t.face(), t.i() + di, t.j() + dj));
                }
            }
            for c in tile_cols(t) {
                match (wet, surface(world, &g, c)) {
                    (true, Some((h, f))) => {
                        let shown = self.surf.get(&c).map(|&(p, q, t0, _)| shown(p, q, t0, now));
                        self.surf.insert(c, (shown.unwrap_or(h), h, now, f));
                    }
                    _ => {
                        self.surf.remove(&c);
                    }
                }
            }
        }
        again.sort_unstable();
        again.dedup();
        // a tile with as many quads as before is rewritten in place and only its range uploaded; else all is
        // rebuilt (the reference)
        let mut rebuild = false;
        for t in again {
            if world.water.wet(t) {
                let q = self.mesh(world, t);
                match self.at.get(&t) {
                    Some(r) if r.len() == q.len() => {
                        self.all[r.clone()].copy_from_slice(&q);
                        self.dirty.push(r.clone());
                    }
                    _ => rebuild = true,
                }
                self.tiles.insert(t, q);
            } else {
                rebuild |= self.tiles.remove(&t).is_some();
            }
        }
        if rebuild {
            self.all.clear();
            self.at.clear();
            self.dirty.clear();
            for (t, q) in &self.tiles {
                self.at.insert(*t, self.all.len()..self.all.len() + q.len());
                self.all.extend_from_slice(q);
            }
            self.changed = true;
        }
    }

    /// The tide moves the generated sea (ADR 0064) at most 13 mm in 4 s: every 4 s the wet tiles are meshed again,
    /// so their columns that are not stored follow it (the chunks' sea follows it in the shader, every frame). One
    /// tile a frame, so a large flood never stalls the frame thread.
    pub fn retide(&mut self, world: &World, now: f32) {
        let at = (world.time / 4.0).floor() as i64;
        if std::mem::replace(&mut self.tide_at, at) != at && self.tide_todo.is_empty() {
            self.tide_todo = self.tiles.keys().copied().collect();
        }
        if let Some(t) = self.tide_todo.pop() {
            if self.tiles.contains_key(&t) {
                self.update(world, &[t], now);
            }
        }
    }

    fn mesh(&self, world: &World, t: Col) -> Vec<WQuad> {
        let n = world.bodies[t.body() as usize].n;
        let info = t.face() as u32 | (t.body() as u32) << 3;
        let g = world.ground();
        // shortcut: columns past the face's edge count as dry, so a cube edge has a seam
        let at = |i: i32, j: i32| -> Option<(f32, f32)> {
            if !(0..n).contains(&i) || !(0..n).contains(&j) {
                return None;
            }
            let c = Col::new(t.body(), t.face(), i, j);
            match self.surf.get(&c) {
                Some(&(p, q, _, _)) => Some((p, q)),
                // past the wet tiles: the generated sea, still
                None if !world.water.wet(c.tile()) => surface(world, &g, c).map(|(h, _)| (h, h)),
                None => None,
            }
        };
        let mut out = Vec::new();
        for c in tile_cols(t) {
            let (i, j) = (c.i(), c.j());
            let Some(&(p, q, t0, fl)) = self.surf.get(&c) else {
                // a dry stored column still bounds its neighbours' sides through its floor, below
                continue;
            };
            // a corner is the mean of the columns around it that are one surface with this one
            let corner = |di: i32, dj: i32, now: bool| -> f32 {
                let own = if now { q } else { p };
                let (mut sum, mut k) = (0.0, 0.0);
                for (a, b) in [(di - 1, dj - 1), (di, dj - 1), (di - 1, dj), (di, dj)] {
                    if let Some((pp, qq)) = at(i + a, j + b) {
                        let h = if now { qq } else { pp };
                        if (h - own).abs() < JOIN {
                            sum += h;
                            k += 1.0;
                        }
                    }
                }
                // never under its own floor: thin water by a step would dip under the step's top
                if k > 0.0 { (sum / k).max(fl) } else { own }
            };
            let cs = |now| [corner(0, 0, now), corner(1, 0, now), corner(1, 1, now), corner(0, 1, now)];
            let (cp, cq) = (cs(false), cs(true));
            out.push(WQuad { i, j, info, t0, prev: cp, cur: cq });
            // sides: toward -i, +i, -j, +j, over the two corners of that edge
            for (kind, (di, dj), (a, b)) in [(1u32, (-1, 0), (0, 3)), (2, (1, 0), (1, 2)), (3, (0, -1), (0, 1)), (4, (0, 1), (3, 2))] {
                let below = match at(i + di, j + dj) {
                    Some(h) => Some(h),
                    None => floor(world, Col::new(t.body(), t.face(), i + di, j + dj)).map(|f| (f, f)),
                };
                let Some((bp, bq)) = below else { continue };
                if q - bq > SIDE || p - bp > SIDE {
                    out.push(WQuad { i, j, info: info | kind << 5, t0, prev: [cp[a], cp[b], bp.min(cp[a]), bp.min(cp[b])], cur: [cq[a], cq[b], bq.min(cq[a]), bq.min(cq[b])] });
                }
            }
        }
        out
    }
}

impl WaterMesh {
    /// The spray over every side that drops more than half a block (a fall or a front): droplets of the
    /// effects' `spray` row (ADR 0031), which splash where they land.
    pub fn spray(&mut self, world: &World, dt: f32, fx: &mut crate::fx::Fx) {
        let mut r = self.rng.max(1);
        let mut rnd = move || {
            r ^= r << 13;
            r ^= r >> 17;
            r ^= r << 5;
            (r as f32 / u32::MAX as f32, r)
        };
        let spray = crate::fx::id("spray");
        for q in &self.all {
            let kind = q.info >> 5 & 7;
            let fall = (q.cur[0] + q.cur[1] - q.cur[2] - q.cur[3]) * 0.5;
            if !(1..=4).contains(&kind) || fall < 1.0 || rnd().0 > dt * 12.0 * fall.min(8.0) {
                continue;
            }
            let (face, bi) = ((q.info & 7) as usize, (q.info >> 3 & 3) as usize);
            let b = &world.bodies[bi];
            let (out_u, out_v, edge_u, edge_v) = match kind {
                1 => (-1.0, 0.0, 0.0, 1.0),
                2 => (1.0, 0.0, 0.0, 1.0),
                3 => (0.0, -1.0, 1.0, 0.0),
                _ => (0.0, 1.0, 1.0, 0.0),
            };
            let along = rnd().0 as f64;
            let (u, v) = (q.i as f64 + 0.5 + out_u * 0.5 + edge_u * (along - 0.5), q.j as f64 + 0.5 + out_v * 0.5 + edge_v * (along - 0.5));
            let up = b.dir(face, u, v);
            let out = (b.dir(face, u + out_u * 0.01, v + out_v * 0.01) - up).normalize();
            let p = up * b.layer_r(q.cur[0] as f64 - 0.05);
            let vel = out * (0.6 + rnd().0 as f64) + up * (rnd().0 as f64 * 0.25);
            fx.play(spray, &crate::fx::At::point(bi as u8, p, vel, b.layer_r(q.cur[2].min(q.cur[3]) as f64)));
        }
        self.rng = rnd().1;
    }
}

fn shown(p: f32, q: f32, t0: f32, now: f32) -> f32 {
    p + (q - p) * ((now - t0) / MOVE_S).clamp(0.0, 1.0)
}

fn tile_cols(t: Col) -> impl Iterator<Item = Col> {
    (0..TILE * TILE).map(move |k| Col::new(t.body(), t.face(), t.i() * TILE + k % TILE, t.j() * TILE + k / TILE))
}

/// The open water surface of column c in layers, and its floor: stored water, else the generated sea.
fn surface(world: &World, g: &impl engine::liquid::Ground, c: Col) -> Option<(f32, f32)> {
    match world.water.column(c) {
        Some(mut ns) => ns.find(|n| n.ceil == OPEN && n.water > 0).map(|n| ((n.floor as i64 * U + n.water) as f32 / U as f32, n.floor as f32)),
        None => {
            let (lo, hi) = g.generated(c);
            (lo < hi).then_some((hi as f32 / U as f32, lo as f32 / U as f32))
        }
    }
}

/// The floor of a stored dry column's open air: what water next to it falls down to.
fn floor(world: &World, c: Col) -> Option<f32> {
    world.water.column(c)?.find(|n| n.ceil == OPEN).map(|n| n.floor as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Water poured on the planet's land: the tile is meshed with a top per wet column and sides at its edge,
    /// and the move starts from what was shown.
    #[test]
    fn a_wet_tile_is_meshed_with_tops_and_sides() {
        use engine::liquid::Ground;
        let mut w = World::new(256, 512, 1);
        // a tile all on land, and a column in the middle of it
        let n = w.bodies[0].n / TILE;
        let (c, floor) = {
            let g = w.ground();
            let dry = |t: Col| tile_cols(t).all(|c| { let (lo, hi) = g.generated(c); lo >= hi });
            let t = (0..n * n).map(|k| Col::new(0, 2, k % n, k / n)).find(|&t| dry(t)).expect("a dry tile");
            let c = Col::new(0, 2, t.i() * TILE + 8, t.j() * TILE + 8);
            (c, g.gaps(c).last().unwrap().0)
        };
        w.water.set_column(&(c, vec![(floor, OPEN, U / 2)]));
        let mut m = WaterMesh::default();
        m.update(&w, &[c.tile()], 1.0);
        let tops = m.all.iter().filter(|q| q.info >> 5 == 0).count();
        let sides = m.all.iter().filter(|q| q.info >> 5 != 0).count();
        assert_eq!(tops, 1);
        assert_eq!(sides, 0, "unstored dry land around it is higher, or it would have flowed there");
        let top = m.all[0];
        assert_eq!(top.cur, [floor as f32 + 0.5; 4]);
        // it drains to a quarter: the move starts at the half that was shown
        w.water.set_column(&(c, vec![(floor, OPEN, U / 4)]));
        m.update(&w, &[c.tile()], 1.01);
        let top = m.all[0];
        assert_eq!((top.prev[0], top.cur[0], top.t0), (floor as f32 + 0.5, floor as f32 + 0.25, 1.01));
        // a stored dry neighbour lower down gets a side over it
        let d = Col::new(0, 2, c.i() + 1, c.j());
        w.water.set_column(&(d, vec![(floor - 2, OPEN, 0)]));
        m.update(&w, &[c.tile()], 2.0);
        let side = m.all.iter().find(|q| q.info >> 5 == 2).unwrap_or_else(|| panic!("a side toward +i: {:?}", m.all.iter().map(|q| (q.i, q.j, q.info, q.cur)).collect::<Vec<_>>()));
        assert_eq!(side.cur[2], (floor - 2) as f32);
        // dry again: nothing is drawn
        w.water.set_tile(c.tile(), &[]);
        m.update(&w, &[c.tile()], 3.0);
        assert!(m.all.is_empty());
    }

    /// A re-meshed tile with as many quads as before is rewritten in place, its range alone to upload; any
    /// sequence of updates leaves `all` holding the quads of the full rebuild (the reference: every tile's).
    #[test]
    fn a_remeshed_tile_of_the_same_size_is_rewritten_in_place() {
        use engine::liquid::Ground;
        let mut w = World::new(256, 512, 1);
        let n = w.bodies[0].n / TILE;
        let cols: Vec<(Col, i32)> = {
            let g = w.ground();
            let dry = |t: Col| tile_cols(t).all(|c| { let (lo, hi) = g.generated(c); lo >= hi });
            let ts: Vec<Col> = (0..n * n).map(|k| Col::new(0, 2, k % n, k / n)).filter(|&t| dry(t)).step_by(3).take(3).collect();
            ts.iter().map(|t| Col::new(0, 2, t.i() * TILE + 8, t.j() * TILE + 8)).map(|c| (c, g.gaps(c).last().unwrap().0)).collect()
        };
        assert_eq!(cols.len(), 3);
        let bytes = |v: &mut dyn Iterator<Item = &WQuad>| {
            let mut v: Vec<Vec<u8>> = v.map(|q| bytemuck::bytes_of(q).to_vec()).collect();
            v.sort();
            v
        };
        let check = |m: &WaterMesh, step: &str| assert_eq!(bytes(&mut m.all.iter()), bytes(&mut m.tiles.values().flatten()), "{step}");
        let mut m = WaterMesh::default();
        for (c, f) in &cols {
            w.water.set_column(&(*c, vec![(*f, OPEN, U / 2)]));
        }
        m.update(&w, &cols.iter().map(|c| c.0.tile()).collect::<Vec<_>>(), 1.0);
        check(&m, "three new tiles");
        assert!(m.changed);
        // the same quads, other heights: in place
        let same = |m: &mut WaterMesh, w: &mut World, k: usize, water: i64, now: f32| {
            let (c, f) = cols[k];
            w.water.set_column(&(c, vec![(f, OPEN, water)]));
            m.changed = false;
            m.dirty.clear();
            m.update(w, &[c.tile()], now);
            assert!(!m.changed && !m.dirty.is_empty(), "tile {k}: rebuilt");
            let want = f as f32 + water as f32 / U as f32;
            let got: Vec<f32> = m.dirty.iter().flat_map(|r| m.all[r.clone()].iter().map(|q| q.cur[0])).collect();
            // (a wet neighbour of its tile is re-meshed too)
            assert!(got.iter().any(|h| (h - want).abs() < 1e-3), "tile {k}: another range, tops at {got:?}, not {want}");
        };
        same(&mut m, &mut w, 1, U / 4, 1.1);
        check(&m, "tile 1 drained");
        // a stored dry neighbour lower down adds a side: rebuilt
        let (c0, f0) = cols[0];
        w.water.set_column(&(Col::new(0, 2, c0.i() + 1, c0.j()), vec![(f0 - 2, OPEN, 0)]));
        m.update(&w, &[c0.tile()], 1.2);
        assert!(m.changed);
        check(&m, "a side added");
        same(&mut m, &mut w, 2, U / 3, 1.3);
        check(&m, "tile 2 drained");
        same(&mut m, &mut w, 1, U / 8, 1.4);
        check(&m, "tile 1 again");
        w.water.set_tile(c0.tile(), &[]);
        m.update(&w, &[c0.tile()], 1.5);
        check(&m, "tile 0 dry");
        assert_eq!(m.all.len(), 2);
    }
}
