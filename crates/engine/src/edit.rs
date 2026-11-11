//! Edits as shapes (ADR 0008, 0014): a box of cells on one face of one body. A single block, the soak's brush and
//! a dragged line, wall or floor are all boxes. `changes` is the one rule for what a box does, so the server
//! applies it and the client predicts with it.
use crate::blocks::{AIR, BLOCKS};
use crate::world::{Block, World};
use glam::{DVec3, IVec3};
use serde::{Deserialize, Serialize};

/// The longest side of a shape, in blocks: 16 m, the reach.
pub const MAX_SIDE: i32 = 16;
/// The most cells one shape may cover: a full 16 x 16 wall or floor, two blocks thick.
pub const MAX_CELLS: i32 = 512;

/// What part of its box a shape covers (ADR 0017). Cylinders and cones stand on the layers, so they stay
/// upright on the planet.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Kind {
    /// Every cell: a block, the brush, a line, a wall or a floor.
    #[default]
    Box,
    /// The upright cylinder inscribed in the box: towers, pipes, round rooms.
    Cylinder,
    /// The upright cone inscribed in the box, its point at the top layer: roofs and spires.
    Cone,
}

/// A `kind` inside the box from `min` to `max` (inclusive) in (column i, column j, layer).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape {
    pub body: u8,
    pub face: u8,
    pub min: IVec3,
    pub max: IVec3,
    pub kind: Kind,
}

impl Shape {
    pub fn one(b: Block) -> Shape {
        Shape { body: b.body, face: b.face, min: b.pos, max: b.pos, kind: Kind::Box }
    }

    /// The box spanned by two blocks of one face (a drag from `a` to `b`); `None` across faces or bodies.
    pub fn between(a: Block, b: Block) -> Option<Shape> {
        (a.body == b.body && a.face == b.face).then(|| Shape { body: a.body, face: a.face, min: a.pos.min(b.pos), max: a.pos.max(b.pos), kind: Kind::Box })
    }

    pub fn size(&self) -> IVec3 {
        self.max - self.min + 1
    }

    /// True for a box within the size limits; where it lies is the server's check.
    pub fn sane(&self) -> bool {
        let s = self.size();
        s.min_element() >= 1 && s.max_element() <= MAX_SIDE && s.x * s.y * s.z <= MAX_CELLS
    }

    /// True for a sane box whose every cell exists in `world`; reach is the server's to check.
    pub fn fits(&self, world: &World) -> bool {
        let Some(body) = world.bodies.get(self.body as usize) else { return false };
        let inside = |v: IVec3| (0..body.n).contains(&v.x) && (0..body.n).contains(&v.y) && (0..body.layers).contains(&v.z);
        self.face < 6 && self.sane() && inside(self.min) && inside(self.max)
    }

    pub fn cells(&self) -> impl Iterator<Item = Block> + '_ {
        let (lo, hi) = (self.min, self.max);
        (lo.x..=hi.x)
            .flat_map(move |i| (lo.y..=hi.y).flat_map(move |j| (lo.z..=hi.z).map(move |l| IVec3::new(i, j, l))))
            .filter(|p| self.covers(*p))
            .map(|pos| Block { body: self.body, face: self.face, pos })
    }

    /// True when the cell at `p` (inside the box) belongs to the shape: its centre lies within the ellipse
    /// the box's columns inscribe, which for a cone narrows layer by layer towards the top.
    fn covers(&self, p: IVec3) -> bool {
        let s = self.size().as_vec3();
        let c = p.as_vec3() + 0.5 - self.min.as_vec3();
        let (dx, dy) = (c.x / s.x * 2.0 - 1.0, c.y / s.y * 2.0 - 1.0);
        let r = match self.kind {
            Kind::Box => return true,
            Kind::Cylinder => 1.0,
            Kind::Cone => 1.0 - (c.z - 0.5) / s.z,
        };
        dx * dx + dy * dy <= r * r + 1e-4
    }
}

/// What a shape does to the world, as (cell, block before): digging (`m` = air) clears every non-air cell,
/// placing fills every air cell with `m` except those `keep_clear` names (where a player stands).
pub fn changes(world: &World, shape: &Shape, m: u8, keep_clear: impl Fn(Block) -> bool) -> Vec<(Block, u8)> {
    shape
        .cells()
        .filter_map(|b| {
            let old = world.block(b);
            let does = if m == AIR { old != AIR } else { old == AIR && !(BLOCKS.solid(m) && keep_clear(b)) };
            does.then_some((b, old))
        })
        .collect()
}

/// Seconds a survival player holds to dig `shape` with bare hands (ADR 0034): its hardest block's hardness,
/// so the hammer digs a shape in its slowest cell's time (the developer, 2026-11-11); 0 when there is nothing
/// to dig. Creative does not wait.
pub fn dig_time(world: &World, shape: &Shape) -> f32 {
    shape.cells().filter_map(|b| BLOCKS.get(world.block(b))).filter(|d| d.solid).map(|d| d.hardness).fold(0.0, f32::max)
}

/// Seconds a half-dug shape keeps its progress after its player looks away or lets go (ADR 0034).
pub const DIG_KEEP_S: f32 = 5.0;
/// Half-dug shapes kept at most.
const KEPT: usize = 8;

/// A survival dig held over time (ADR 0034), as the server counts it and the client shows it: the shape held
/// now, the clock when the hold began and the seconds it had before; shapes left half dug keep their seconds
/// for `DIG_KEEP_S`. The clock is the caller's, in seconds (the server's ticks, the client's frames).
#[derive(Clone, Debug, Default)]
pub struct Hold {
    now: Option<(Shape, f64, f32)>,
    kept: Vec<(Shape, f32, f64)>,
}

impl Hold {
    /// Holds on `at` from clock `t`, or lets go: the shape held before keeps its seconds, a kept one comes
    /// back with its own. True when what is held changed.
    pub fn on(&mut self, at: Option<Shape>, t: f64) -> bool {
        if self.shape() == at {
            return false;
        }
        if let Some((s, t0, base)) = self.now.take() {
            self.kept.push((s, base + (t - t0) as f32, t));
        }
        self.kept.retain(|k| t - k.2 <= DIG_KEEP_S as f64);
        if self.kept.len() > KEPT {
            self.kept.remove(0);
        }
        self.now = at.map(|s| (s, t, self.kept.iter().position(|k| k.0 == s).map_or(0.0, |i| self.kept.remove(i).1)));
        true
    }

    /// The shape held now.
    pub fn shape(&self) -> Option<Shape> {
        self.now.map(|n| n.0)
    }

    /// Seconds held on `s` by clock `t`: 0 unless it is the shape held now.
    pub fn held(&self, s: &Shape, t: f64) -> f32 {
        self.now.filter(|n| n.0 == *s).map_or(0.0, |(_, t0, base)| base + (t - t0) as f32)
    }

    /// The dig went: nothing is held, and nothing kept of that shape.
    pub fn done(&mut self) {
        if let Some((s, ..)) = self.now.take() {
            self.kept.retain(|k| k.0 != s);
        }
    }

    /// The shapes left half dug that still keep their seconds by clock `t`, with them.
    pub fn kept(&self, t: f64) -> impl Iterator<Item = (Shape, f32)> + '_ {
        self.kept.iter().filter(move |k| t - k.2 <= DIG_KEEP_S as f64).map(|k| (k.0, k.1))
    }
}

/// A group of 2 x 2 x 2 blocks around `b`, kept on its face. It was the plain dig and place on 0.5 m
/// blocks (ADR 0008); since ADR 0033 that is one block, and only the soak's bots still use this.
pub fn brush(world: &World, b: Block) -> Shape {
    let body = &world.bodies[b.body as usize];
    let min = b.pos.div_euclid(IVec3::splat(2)) * 2;
    let max = (min + 1).min(IVec3::new(body.n - 1, body.n - 1, body.layers - 1));
    Shape { min: min.max(IVec3::ZERO), max, ..Shape::one(b) }
}

/// What the player builds with (ADR 0017): a kind, the brush or (`single`) one block at each end of a drag,
/// and for cylinders and cones a height in layers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tool {
    pub kind: Kind,
    pub single: bool,
    pub height: i32,
}

impl Tool {
    /// The shape of a press at `a` dragged to `b` (the same block for a click). A box spans both ends; a
    /// cylinder or cone is centred on `a` with the drag as its radius and the tool's height, growing up from
    /// the ground when placing and down into it when digging. `None` across a face edge.
    pub fn shape(&self, world: &World, a: Block, b: Block, dig: bool) -> Option<Shape> {
        let end = |c: Block| if self.single { Shape::one(c) } else { brush(world, c) };
        let (ea, eb) = (end(a), end(b));
        if (ea.body, ea.face) != (eb.body, eb.face) {
            return None;
        }
        let mut s = Shape { min: ea.min.min(eb.min), max: ea.max.max(eb.max), kind: self.kind, ..ea };
        if self.kind != Kind::Box {
            let d = (b.pos - a.pos).abs();
            let r = IVec3::new(d.x.max(d.y), d.x.max(d.y), 0);
            (s.min, s.max) = (ea.min - r, ea.max + r);
            if dig {
                s.min.z = s.max.z - (self.height - 1);
            } else {
                s.max.z = s.min.z + (self.height - 1);
            }
        }
        Some(s)
    }
}

/// The first solid block along the ray from `eye` in direction `dir` within `reach` metres, and the block the
/// ray was in just before it (where a place goes).
pub fn aim(world: &World, eye: DVec3, dir: DVec3, reach: f64) -> Option<(Block, Block)> {
    // shortcut: fixed 5 cm steps instead of an exact grid walk; a corner clipped by under 5 cm can be missed
    let mut prev = world.block_at(eye);
    let mut t = 0.0;
    while t < reach {
        t += 0.05;
        let b = world.block_at(eye + dir * t);
        if b == prev {
            continue;
        }
        if b.pos.z >= 0 && BLOCKS.solid(world.block(b)) {
            return Some((b, prev));
        }
        prev = b;
    }
    None
}

/// The client's side of an edit (ADR 0015): it shows the edit at once and puts back, when the server answers,
/// every cell the server did not confirm. It keeps the server's last word on each cell a pending edit
/// covers; a cell shows the newest pending edit's block until its last pending edit is answered, then the
/// server's. The server sends an edit's `Edits` before its `Done`, so what is put back is the server's state.
#[derive(Default)]
pub struct Predictor {
    next: u32,
    /// Edits sent and not answered yet, oldest first: (seq, cells).
    pending: std::collections::VecDeque<(u32, Vec<Block>)>,
    /// The server's word on every cell a pending edit covers, as `World::edit` gives it.
    truth: crate::world::FxMap<Block, Option<u8>>,
}

impl Predictor {
    /// The last `seq` handed out: a message that is not an edit (a bucket) sends it, so its `Done` confirms
    /// only what was sent before it.
    pub fn seq(&self) -> u32 {
        self.next
    }
    /// Applies the edit to `world` and returns the `seq` to send with it and the cells it changed. A shape that
    /// does not fit the world changes nothing here; the server will refuse it.
    pub fn edit(&mut self, world: &mut World, shape: &Shape, m: u8, keep_clear: impl Fn(&World, Block) -> bool) -> (u32, Vec<Block>) {
        self.next = self.next.wrapping_add(1);
        let w: &World = world;
        let cells: Vec<Block> = if shape.fits(w) && BLOCKS.get(m).is_some() {
            changes(w, shape, m, |b| keep_clear(w, b)).into_iter().map(|(b, _)| b).collect()
        } else {
            Vec::new()
        };
        for &b in &cells {
            self.truth.entry(b).or_insert_with(|| world.edit(b));
            world.set_block(b, m);
        }
        self.pending.push_back((self.next, cells.clone()));
        (self.next, cells)
    }

    /// A cell from the server's `Edits`; true when the world shows it now (no pending edit hides it).
    pub fn server(&mut self, world: &mut World, b: Block, m: u8) -> bool {
        match self.truth.get_mut(&b) {
            Some(t) => {
                *t = Some(m);
                false
            }
            None => {
                world.set_block(b, m);
                true
            }
        }
    }

    /// The server's `Done` for `seq` (and so for every older edit): returns the cells put back.
    pub fn done(&mut self, world: &mut World, seq: u32) -> Vec<Block> {
        let mut back = Vec::new();
        while self.pending.front().is_some_and(|(s, _)| s.wrapping_sub(seq) as i32 <= 0) {
            let (_, cells) = self.pending.pop_front().unwrap();
            for b in cells {
                // shortcut: a linear scan per cell, fine for the few edits in flight; a count per cell if drags pile up
                if self.pending.iter().any(|(_, c)| c.contains(&b)) {
                    continue; // a newer edit still shows here
                }
                // a confirmed cell already shows the server's block: nothing to put back or remesh
                if let Some(t) = self.truth.remove(&b).filter(|t| *t != world.edit(b)) {
                    world.put_edit(b, t);
                    back.push(b);
                }
            }
        }
        back
    }

    pub fn pending(&self) -> usize {
        self.pending.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hold_counts_its_time_and_keeps_a_left_shape_five_seconds() {
        let b = |i| Shape::one(Block { body: 0, face: 2, pos: IVec3::new(i, 0, 0) });
        let mut h = Hold::default();
        assert!(h.on(Some(b(1)), 10.0) && !h.on(Some(b(1)), 10.5), "only a change counts");
        assert_eq!(h.held(&b(1), 11.0), 1.0);
        // elsewhere for 2 s: the first keeps its second, shown while kept; back, it goes on from it
        h.on(Some(b(2)), 11.0);
        assert_eq!((h.held(&b(1), 12.0), h.held(&b(2), 12.0)), (0.0, 1.0));
        assert_eq!(h.kept(12.0).collect::<Vec<_>>(), vec![(b(1), 1.0)]);
        h.on(Some(b(1)), 13.0);
        assert_eq!(h.held(&b(1), 13.5), 1.5);
        // let go for longer than DIG_KEEP_S: it starts over
        h.on(None, 14.0);
        assert_eq!(h.kept(14.0 + DIG_KEEP_S as f64 + 0.1).count(), 0);
        h.on(Some(b(1)), 14.0 + DIG_KEEP_S as f64 + 0.1);
        assert_eq!(h.held(&b(1), 14.0 + DIG_KEEP_S as f64 + 0.1), 0.0);
        // a dig that went keeps nothing
        h.done();
        assert_eq!((h.shape(), h.kept(20.0).count()), (None, 0));
    }

    #[test]
    fn a_dig_takes_its_slowest_cell_s_time() {
        let mut w = World::new(256, 32, 1);
        let b = &w.bodies[0];
        let top = w.block_at(b.centre + DVec3::Z * (b.height(DVec3::Z) - 0.5));
        let hard = |n: &str| BLOCKS.get(BLOCKS.id(n)).unwrap().hardness;
        w.set_block(top, BLOCKS.id("dirt"));
        let below = Block { pos: top.pos - IVec3::Z, ..top };
        w.set_block(below, BLOCKS.id("stone"));
        assert_eq!(dig_time(&w, &Shape::one(top)), hard("dirt"));
        // the hammer's shape over dirt and stone goes in the stone's time, not the sum (the developer, 2026-11-11)
        assert_eq!(dig_time(&w, &Shape { min: below.pos, ..Shape::one(top) }), hard("stone"));
        let air = Block { pos: top.pos + IVec3::Z * 3, ..top };
        assert_eq!(dig_time(&w, &Shape::one(air)), 0.0);
    }

    #[test]
    fn a_box_covers_its_cells_and_the_limits_hold() {
        let b = |i, j, l| Block { body: 0, face: 2, pos: IVec3::new(i, j, l) };
        let s = Shape::between(b(5, 3, 9), b(4, 4, 8)).unwrap();
        assert_eq!(s.cells().count(), 8);
        assert!(s.cells().all(|c| c.pos.cmpge(IVec3::new(4, 3, 8)).all() && c.pos.cmple(IVec3::new(5, 4, 9)).all()));
        assert!(s.sane() && Shape::one(b(1, 1, 1)).sane());
        assert!(Shape::between(b(0, 0, 0), Block { face: 3, ..b(0, 0, 0) }).is_none());
        assert!(!Shape::between(b(0, 0, 0), b(32, 0, 0)).unwrap().sane(), "33 long");
        assert!(!Shape::between(b(0, 0, 0), b(31, 31, 2)).unwrap().sane(), "too many cells");
        assert!(!Shape { min: IVec3::ONE, max: IVec3::ZERO, ..s }.sane(), "inside out");
    }

    #[test]
    fn cylinders_are_round_and_cones_narrow_to_the_top() {
        let o = Block { body: 0, face: 2, pos: IVec3::ZERO };
        let cyl = Shape { max: IVec3::new(9, 9, 3), kind: Kind::Cylinder, ..Shape::one(o) };
        let layer = |s: &Shape, l: i32| s.cells().filter(|c| c.pos.z == l).count();
        // a 10 x 10 disc: about pi * 25 = 79 cells, the corners left out, the edge middles kept
        assert!((70..=90).contains(&layer(&cyl, 0)), "{}", layer(&cyl, 0));
        assert!((0..4).all(|l| layer(&cyl, l) == layer(&cyl, 0)));
        let has = |s: &Shape, p: IVec3| s.cells().any(|c| c.pos == p);
        assert!(!has(&cyl, IVec3::ZERO) && has(&cyl, IVec3::new(0, 4, 0)) && has(&cyl, IVec3::new(9, 5, 2)));
        let cone = Shape { max: IVec3::new(8, 8, 4), kind: Kind::Cone, ..cyl };
        let sizes: Vec<usize> = (0..5).map(|l| layer(&cone, l)).collect();
        assert!(sizes.windows(2).all(|w| w[0] > w[1]) && sizes[4] >= 1, "{sizes:?}");
        assert_eq!(Shape::one(o).cells().count(), 1);
        assert_eq!(Shape { kind: Kind::Cylinder, ..Shape::one(o) }.cells().count(), 1, "a 1-wide cylinder is its column");
    }

    #[test]
    fn digging_takes_what_is_there_and_placing_fills_only_air() {
        let mut w = World::new(256, 32, 1);
        let top = surface(&w);
        let s = Shape::between(top, Block { pos: top.pos + IVec3::new(1, 1, 1), ..top }).unwrap();
        let dug = changes(&w, &s, AIR, |_| false);
        assert!(dug.iter().all(|(_, old)| *old != AIR) && !dug.is_empty());
        for (b, _) in &dug {
            w.set_block(*b, AIR);
        }
        let stone = BLOCKS.id("stone");
        assert_eq!(changes(&w, &s, stone, |_| false).len(), 8, "all eight are air now");
        assert_eq!(changes(&w, &s, stone, |b| b == top).len(), 7, "a player keeps a cell clear");
    }

    /// The server's side for the predictor tests: applies an edit and returns the `Edits` it would send.
    fn serve(w: &mut World, s: &Shape, m: u8, keep_clear: impl Fn(Block) -> bool) -> Vec<(Block, u8)> {
        let c: Vec<(Block, u8)> = changes(w, s, m, keep_clear).into_iter().map(|(b, _)| (b, m)).collect();
        for (b, m) in &c {
            w.set_block(*b, *m);
        }
        c
    }

    #[test]
    fn a_prediction_ends_as_the_server_decided() {
        let fresh = World::new(256, 32, 1);
        let top = surface(&fresh);
        let brush = Shape { max: top.pos + 1, ..Shape::one(top) };
        let stone = BLOCKS.id("stone");
        let (mut srv, mut cli, mut p) = (World::new(256, 32, 1), World::new(256, 32, 1), Predictor::default());

        // refused: nothing comes back, everything is put back, down to no edit being recorded
        let (s1, cells) = p.edit(&mut cli, &brush, AIR, |_, _| false);
        assert!(!cells.is_empty() && cli.chunk_hashes() != fresh.chunk_hashes());
        p.done(&mut cli, s1);
        assert_eq!((cli.chunk_hashes(), cli.edit_count, p.pending()), (fresh.chunk_hashes(), 0, 0));

        // accepted, but the server kept one cell open for a player the client did not know about
        let (s2, _) = p.edit(&mut cli, &brush, AIR, |_, _| false);
        for (b, m) in serve(&mut srv, &brush, AIR, |_| false) {
            p.server(&mut cli, b, m);
        }
        p.done(&mut cli, s2);
        let (s3, _) = p.edit(&mut cli, &brush, stone, |_, _| false);
        for (b, m) in serve(&mut srv, &brush, stone, |b| b == top) {
            p.server(&mut cli, b, m);
        }
        p.done(&mut cli, s3);
        assert_eq!(cli.block(top), AIR);
        assert_eq!(cli.chunk_hashes(), srv.chunk_hashes());

        // two edits in flight over the same cells; the first is refused, another player's edit lands on one
        // cell in between, the second is accepted
        let line = Shape { max: top.pos + IVec3::new(3, 0, 0), ..Shape::one(top) };
        let (s4, _) = p.edit(&mut cli, &brush, stone, |_, _| false);
        let (s5, _) = p.edit(&mut cli, &line, AIR, |_, _| false);
        let other = Block { pos: top.pos + IVec3::new(0, 1, 1), ..top };
        for (b, m) in serve(&mut srv, &Shape::one(other), BLOCKS.id("dirt"), |_| false) {
            p.server(&mut cli, b, m);
        }
        p.done(&mut cli, s4);
        assert_eq!(cli.block(top), AIR, "the newer edit still shows");
        for (b, m) in serve(&mut srv, &line, AIR, |_| false) {
            p.server(&mut cli, b, m);
        }
        p.done(&mut cli, s5);
        assert_eq!((cli.chunk_hashes(), cli.edit_count, p.pending()), (srv.chunk_hashes(), srv.edit_count, 0));
    }

    #[test]
    fn the_brush_is_the_1m_cell_and_tools_shape_drags() {
        let w = World::new(256, 32, 1);
        let at = |i, j, l| Block { body: 0, face: 4, pos: IVec3::new(i, j, l) };
        let b = brush(&w, at(5, 6, 9));
        assert_eq!((b.min, b.max), (IVec3::new(4, 6, 8), IVec3::new(5, 7, 9)));
        assert_eq!(brush(&w, at(4, 7, 8)), b, "every block of a 1 m cell gives that cell");
        let wall = Tool { kind: Kind::Box, single: false, height: 8 }.shape(&w, at(5, 6, 9), at(14, 6, 13), false).unwrap();
        assert_eq!((wall.min, wall.max), (IVec3::new(4, 6, 8), IVec3::new(15, 7, 13)));
        let one = Tool { kind: Kind::Box, single: true, height: 8 };
        assert_eq!(one.shape(&w, at(5, 6, 9), at(5, 6, 9), true).unwrap(), Shape::one(at(5, 6, 9)));
        let tower = Tool { kind: Kind::Cylinder, single: false, height: 8 };
        // centred on the press, the drag is the radius whichever way it goes
        let up = tower.shape(&w, at(20, 20, 10), at(24, 21, 10), false).unwrap();
        assert_eq!((up.min, up.max, up.kind), (IVec3::new(16, 16, 10), IVec3::new(25, 25, 17), Kind::Cylinder));
        let down = tower.shape(&w, at(20, 20, 10), at(20, 16, 10), true).unwrap();
        assert_eq!((down.min, down.max), (IVec3::new(16, 16, 4), IVec3::new(25, 25, 11)), "a dug shaft goes down");
        let thin = Tool { single: true, ..tower }.shape(&w, at(20, 20, 10), at(17, 20, 10), false).unwrap();
        assert_eq!((thin.min, thin.max), (IVec3::new(17, 17, 10), IVec3::new(23, 23, 17)), "radius 3 around one block");
        assert!(tower.shape(&w, at(20, 20, 10), Block { face: 3, ..at(20, 20, 10) }, false).is_none());
    }

    #[test]
    fn aim_finds_the_ground_and_the_cell_before_it() {
        let w = World::new(256, 32, 1);
        let ground = w.bodies[0].centre + glam::DVec3::Z * w.bodies[0].height(glam::DVec3::Z);
        let (hit, before) = aim(&w, ground + glam::DVec3::Z * 1.6, -glam::DVec3::Z, 8.0).unwrap();
        assert!(BLOCKS.solid(w.block(hit)) && !BLOCKS.solid(w.block(before)));
        assert_eq!((hit.pos.x, hit.pos.y, hit.pos.z + 1), (before.pos.x, before.pos.y, before.pos.z));
        assert!(aim(&w, ground + glam::DVec3::Z * 1.6, glam::DVec3::Z, 8.0).is_none(), "nothing in the sky");
    }

    /// The block just under the surface at the planet's +z pole.
    fn surface(w: &World) -> Block {
        let b = &w.bodies[0];
        w.block_at(b.centre + glam::DVec3::Z * (b.height(glam::DVec3::Z) - 0.25))
    }
}
