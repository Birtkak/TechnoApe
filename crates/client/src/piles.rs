//! The piles on the ground (ADR 0074, 0082, 0114): each stack drawn as the model of its item's or block's name
//! when one is loaded, lying at its place in the cell; else as the small cube of what it looks like.
use crate::app::App;
use crate::fx::{self, FxGpu};
use engine::items::{Slot, ITEMS, ITEM_BASE};
use engine::world::{Block, World};
use glam::{DVec3, Vec3};

/// Piles farther than this are not drawn: a quarter-metre cube is a few pixels there.
const PILE_DRAWN: f64 = 64.0;

/// A block lying in a pile: its edge in cells (about metres).
const BLOCK_M: f32 = 0.2;

/// A stack drawn as a model (`Models::frame`): the model's name, whether an item's (a tool is laid down flat), its
/// place on the floor in its body's frame from the body's centre, the cell's up and ground axes, and its turn
/// about up in radians.
#[derive(Clone, Debug, PartialEq)]
pub struct Lying {
    pub name: String,
    pub item: bool,
    pub body: u8,
    pub p: Vec3,
    pub axes: [Vec3; 3],
    pub turn: f32,
}

/// The piles within `PILE_DRAWN` of `me` as drawn: the stacks whose item or block names a model (`has`) as models,
/// the others as cubes of their look among the particles. Up to four stacks a cell, each at its own corner, turned
/// by the cell's hash, so a pile looks the same every frame and unlike its neighbour.
pub(crate) fn lay(world: &World, piles: &std::collections::HashMap<Block, Vec<Slot>>, me: DVec3, has: &dyn Fn(&str) -> bool) -> (Vec<Lying>, Vec<FxGpu>) {
    let (mut models, mut still) = (Vec::new(), Vec::new());
    for (b, slots) in piles {
        if world.block_centre(*b).distance(me) > PILE_DRAWN {
            continue;
        }
        let at = fx::At::floor_of(world, *b);
        let seed = b.pos.x as u32 ^ (b.pos.y as u32).rotate_left(11) ^ (b.pos.z as u32).rotate_left(22);
        for (k, s) in slots.iter().enumerate().filter_map(|(k, s)| Some((k, (*s)?))) {
            let side = [(-0.15, -0.15), (0.15, 0.1), (-0.1, 0.17), (0.17, -0.15)][k % 4];
            let floor = at.p + at.axes[1] * side.0 + at.axes[2] * side.1;
            let name = ITEMS.name(s.item);
            if has(name) {
                let turn = (seed.wrapping_mul(2654435761) >> 8) as f32 / (1 << 24) as f32 * std::f32::consts::TAU + k as f32 * 1.9;
                models.push(Lying { name: name.to_string(), item: s.item >= ITEM_BASE, body: b.body, p: floor, axes: at.axes, turn });
                continue;
            }
            // the block's own cell look (its painting, as the world draws it) shrunk about the cell's middle, moved
            // in cells (bit 20, as a glide) to its corner and down onto the floor
            let info = ITEMS.look(s.item) as u32 | (b.body as u32) << 8 | (fx::Look::Block as u32) << 10 | 1 << 20;
            let (a, code) = fx::cell_code(*b);
            still.push(FxGpu { p: [side.0, side.1, BLOCK_M / 2.0 - 0.5], size: BLOCK_M, info, angle: 0.0, a, b: code });
        }
    }
    (models, still)
}

impl App {
    /// This frame's piles: the models to `Models::frame`, the cubes laid among the particles.
    pub(crate) fn lay_piles(&mut self) {
        let models = &self.models;
        let (lying, still) = lay(&self.world, &self.piles, self.me.pos, &|n| models.has(n));
        self.models.piles = lying;
        self.fx.lay(still);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::items::Stack;

    fn pile(has: &[&str], me_off: f64) -> (Vec<Lying>, Vec<FxGpu>) {
        let w = World::new(768, 256, 1);
        let b = w.air_over_ground(0, w.bodies[0].spawn);
        let id = |n: &str| ITEMS.named(n).unwrap_or_else(|| panic!("no {n}"));
        let slots = vec![Some(Stack::new(id("hand axe"), 1)), Some(Stack::new(id("granite"), 3)), None, Some(Stack::new(id("flint"), 2))];
        let piles = [(b, slots)].into_iter().collect();
        let me = w.block_centre(b) + (w.block_centre(b) - w.bodies[0].centre).normalize().any_orthonormal_vector() * me_off;
        lay(&w, &piles, me, &|n| has.contains(&n))
    }

    /// ADR 0114: a stack whose item has a loaded model lies as that model, the rest are cubes; the same pile is
    /// turned the same each time; past `PILE_DRAWN` nothing; a model not loaded falls back to the cube.
    #[test]
    fn a_pile_draws_its_models_and_cubes_for_the_rest() {
        let (models, cubes) = pile(&["hand axe", "flint"], 1.0);
        assert_eq!(models.iter().map(|m| (m.name.as_str(), m.item)).collect::<Vec<_>>(), [("hand axe", true), ("flint", true)]);
        assert_eq!(cubes.len(), 1, "granite: a block's look is its cube");
        assert_eq!(cubes[0].info & 255, ITEMS.look(ITEMS.named("granite").unwrap()) as u32 & 255);
        // drawn as the block's own cell look (its painting, as the world has it; ADR 0114 second round), 0.2 m,
        // resting on the cell's floor: `p` its offset in cells (bit 20, as a glide's), its middle 0.1 over the floor
        assert_eq!((cubes[0].info >> 10) & 3, fx::Look::Block as u32, "the block's cell look, not a debris cube");
        assert_eq!(cubes[0].info >> 20 & 1, 1, "p is an offset in cells");
        assert_eq!(cubes[0].size, 0.2);
        assert!((cubes[0].p[2] - (cubes[0].size / 2.0 - 0.5)).abs() < 1e-6, "on the floor: {:?}", cubes[0].p);
        assert!(cubes[0].p[0].abs() < 0.5 - 0.1 && cubes[0].p[1].abs() < 0.5 - 0.1, "inside its cell: {:?}", cubes[0].p);
        assert_ne!(models[0].p, models[1].p, "apart in the cell");
        assert_ne!(models[0].turn, models[1].turn);
        assert_eq!(pile(&["hand axe", "flint"], 1.0).0, models, "the same every frame");
        let (far, far_cubes) = pile(&["hand axe", "flint"], 70.0);
        assert!(far.is_empty() && far_cubes.is_empty(), "past 64 m nothing");
        let (one, cubes) = pile(&["hand axe"], 1.0);
        assert_eq!((one.len(), cubes.len()), (1, 2), "flint without its model is a cube");
    }
}
