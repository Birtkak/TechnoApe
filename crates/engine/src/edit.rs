//! Edits as shapes (ADR 0008, 0014): a box of cells on one face of one body. The 1 m brush, a single block and
//! a dragged line, wall or floor are all boxes. `changes` is the one rule for what a box does, so the server
//! applies it and the client predicts with it.
use crate::blocks::{AIR, BLOCKS};
use crate::world::{Block, World};
use glam::IVec3;
use serde::{Deserialize, Serialize};

/// The longest side of a shape, in blocks: 16 m, the reach.
pub const MAX_SIDE: i32 = 32;
/// The most cells one shape may cover: a full 32 x 32 wall or floor, two blocks thick.
pub const MAX_CELLS: i32 = 2048;

/// Every cell from `min` to `max` (inclusive) in (column i, column j, layer).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape {
    pub body: u8,
    pub face: u8,
    pub min: IVec3,
    pub max: IVec3,
}

impl Shape {
    pub fn one(b: Block) -> Shape {
        Shape { body: b.body, face: b.face, min: b.pos, max: b.pos }
    }

    /// The box spanned by two blocks of one face (a drag from `a` to `b`); `None` across faces or bodies.
    pub fn between(a: Block, b: Block) -> Option<Shape> {
        (a.body == b.body && a.face == b.face).then(|| Shape { body: a.body, face: a.face, min: a.pos.min(b.pos), max: a.pos.max(b.pos) })
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
        (lo.x..=hi.x).flat_map(move |i| {
            (lo.y..=hi.y).flat_map(move |j| (lo.z..=hi.z).map(move |l| Block { body: self.body, face: self.face, pos: IVec3::new(i, j, l) }))
        })
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

    /// The block just under the surface at the planet's +z pole.
    fn surface(w: &World) -> Block {
        let b = &w.bodies[0];
        w.block_at(b.centre + glam::DVec3::Z * (b.height(glam::DVec3::Z) - 0.25))
    }
}
