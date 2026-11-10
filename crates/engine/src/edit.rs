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

    /// The block just under the surface at the planet's +z pole.
    fn surface(w: &World) -> Block {
        let b = &w.bodies[0];
        w.block_at(b.centre + glam::DVec3::Z * (b.height(glam::DVec3::Z) - 0.25))
    }
}
