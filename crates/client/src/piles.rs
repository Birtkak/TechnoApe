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
    /// A tool (its item row has `tool`): it lies larger (`models::TOOL_LYING`), in the middle of its quarter.
    pub tool: bool,
    pub body: u8,
    pub p: Vec3,
    pub axes: [Vec3; 3],
    pub turn: f32,
    /// Its scale's share kept (at most 1): shrunk so its footprint stays inside its cell however it is turned.
    pub fit: f32,
}

/// The piles within `PILE_DRAWN` of `me` as drawn: the stacks whose item or block names a model (`footprint`: its
/// lying footprint's half width in metres, by name, whether an item, whether a tool) as models, the others as
/// cubes of their look among the particles, and a contact shadow under each (`shade`). Up to four stacks a cell,
/// each at its own corner, turned by the cell's hash, so a pile looks the same every frame and unlike its
/// neighbour.
pub(crate) fn lay(world: &World, piles: &std::collections::HashMap<Block, Vec<Slot>>, me: DVec3, footprint: &dyn Fn(&str, bool, bool) -> Option<f32>) -> (Vec<Lying>, Vec<FxGpu>, Vec<FxGpu>) {
    let (mut models, mut still, mut shades) = (Vec::new(), Vec::new(), Vec::new());
    for (b, slots) in piles {
        if world.block_centre(*b).distance(me) > PILE_DRAWN {
            continue;
        }
        let at = fx::At::floor_of(world, *b);
        // no shadow over air (dug out from under it), on a see-through floor (glass, leaves) or under water
        let below = world.block(Block { pos: b.pos - glam::IVec3::Z, ..*b });
        let solid = engine::blocks::BLOCKS.opaque(below) && engine::blocks::BLOCKS.solid(below);
        // asked a few centimetres up: the floor itself is the top of the block below, which holds no water
        let dry = world.water_surface(world.bodies[b.body as usize].centre + (at.p + at.axes[0] * 0.05).as_dvec3()).is_none_or(|s| s <= at.p.length() as f64 + 1e-3);
        // each stack's footprint on the floor, in metres from the cell's middle: one shadow round them all
        let mut feet: Vec<((f32, f32), f32)> = Vec::new();
        let seed = b.pos.x as u32 ^ (b.pos.y as u32).rotate_left(11) ^ (fx::gpu_cell(world, *b).pos.z as u32).rotate_left(22);
        for (k, s) in slots.iter().enumerate().filter_map(|(k, s)| Some((k, (*s)?))) {
            let side = [(-0.15, -0.15), (0.15, 0.1), (-0.1, 0.17), (0.17, -0.15)][k % 4];
            let tool = engine::items::tool(Some(s)).is_some();
            // a tool lies twice its size (`TOOL_LYING`): in its quarter's middle
            let side = if tool { (0.25f32.copysign(side.0), 0.25f32.copysign(side.1)) } else { side };
            // in cells, as the cubes are placed: a cell is 0.7 to 1 m across by where it is on its face
            let off = (side.0 * 2.0 * at.half.y, side.1 * 2.0 * at.half.z);
            let floor = at.p + at.axes[1] * off.0 + at.axes[2] * off.1;
            let name = ITEMS.name(s.item);
            if let Some(half) = footprint(name, s.item >= ITEM_BASE, tool) {
                // shrunk so that turned any way its footprint stays inside its cell (a long tool, a narrow cell)
                let fit = ((at.half.y - off.0.abs()).min(at.half.z - off.1.abs()) / half).min(1.0);
                feet.push((off, half * fit));
                let turn = (seed.wrapping_mul(2654435761) >> 8) as f32 / (1 << 24) as f32 * std::f32::consts::TAU + k as f32 * 1.9;
                models.push(Lying { name: name.to_string(), item: s.item >= ITEM_BASE, tool, body: b.body, p: floor, axes: at.axes, turn, fit });
                continue;
            }
            // the block's own cell look (its painting, as the world draws it) shrunk about the cell's middle, moved
            // in cells (bit 19, as a glide) to its corner and down onto the floor
            let info = fx::block_bits(ITEMS.look(s.item)) | (b.body as u32) << 8 | (fx::Look::Block as u32) << 10 | 1 << 19;
            let (a, code) = fx::cell_code(fx::gpu_cell(world, *b));
            still.push(FxGpu { p: [side.0, side.1, BLOCK_M / 2.0 - 0.5], size: BLOCK_M, info, angle: 0.0, a, b: code });
            feet.push((off, BLOCK_M * std::f32::consts::FRAC_1_SQRT_2 * 2.0 * at.half.y.max(at.half.z)));
        }
        if !feet.is_empty() {
            let lo = feet.iter().fold((f32::MAX, f32::MAX), |m, ((x, y), h)| (m.0.min(x - h), m.1.min(y - h)));
            let hi = feet.iter().fold((f32::MIN, f32::MIN), |m, ((x, y), h)| (m.0.max(x + h), m.1.max(y + h)));
            let mid = ((lo.0 + hi.0) / 2.0, (lo.1 + hi.1) / 2.0);
            let half = feet.iter().map(|((x, y), h)| (x - mid.0).hypot(y - mid.1) + h).fold(0.0, f32::max);
            shades.extend(shade(&at, mid, half, solid && dry));
        }
    }
    (models, still, shades)
}

/// The ground's texels a metre: a contact shadow steps in them, as the shadows do (ADR 0108).
const TEXELS: f32 = 32.0;

/// A contact shadow under a pile whose stacks lie within `half` metres of `off` (metres along the floor's axes
/// from the middle of the floor `at`; ADR 0114 second round): a square `vs_shade` draws flat on the floor, `size`
/// its half width, darkening the albedo under it in rings of whole texels (`fs_shade`). One a pile, as shades
/// that overlapped would multiply darker. Its middle on the ground's texel grid, its half width the footprint's
/// and a little more in whole texels, held inside its cell (so it never hangs over a step); None where the floor
/// is not solid, is see-through or under water (`open` false).
pub(crate) fn shade(at: &fx::At, off: (f32, f32), half: f32, open: bool) -> Option<FxGpu> {
    let snap = |v: f32| (v * TEXELS).round() / TEXELS;
    let (hy, hz) = (at.half.y, at.half.z);
    let (x, y) = (snap(off.0.clamp(-0.9 * hy, 0.9 * hy)), snap(off.1.clamp(-0.9 * hz, 0.9 * hz)));
    let r = ((half * 1.15 * TEXELS).ceil() + 1.0) / TEXELS;
    let r = r.min(((hy - x.abs()).min(hz - y.abs()) * TEXELS).floor() / TEXELS);
    let p = at.p + at.axes[1] * x + at.axes[2] * y;
    (open && r > 0.0).then(|| FxGpu { p: p.to_array(), size: r, info: (at.body as u32) << 8, angle: 0.0, a: fx::pack_dir(at.axes[1]), b: 0 })
}

impl App {
    /// This frame's piles: the models to `Models::frame`, the cubes laid among the particles.
    pub(crate) fn lay_piles(&mut self) {
        let models = &self.models;
        let (lying, still, shades) = lay(&self.world, &self.piles, self.me.pos, &|n, item, tool| models.lying_half(n, item, tool));
        self.models.piles = lying;
        self.fx.lay(still);
        // last in the buffer: `vs_shade` draws them after the particles (frame.rs)
        self.fx.shades = shades.len();
        self.fx.lay(shades);
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
        let (m, c, _) = lay(&w, &piles, me, &|n, _, _| has.contains(&n).then_some(0.1));
        (m, c)
    }

    /// ADR 0114 second round: a contact shadow sits on the texel grid, a little larger than the footprint in
    /// whole texels, inside its cell, and is not made on a see-through floor or under water.
    #[test]
    fn a_contact_shadow_is_snapped_sized_and_kept_in_its_cell() {
        let w = World::new(768, 256, 1);
        let at = fx::At::floor_of(&w, w.air_over_ground(0, w.bodies[0].spawn));
        for (off, half) in [((0.0, 0.0), 0.1), ((-0.153, 0.171), 0.12), ((0.25, -0.25), 0.24), ((0.4, 0.4), 0.3)] {
            let s = shade(&at, off, half, true).expect("a shadow on open ground");
            let d = Vec3::from(s.p) - at.p;
            let (x, y) = (d.dot(at.axes[1]), d.dot(at.axes[2]));
            assert!(d.dot(at.axes[0]).abs() < 1e-4, "on the floor: {d}");
            for v in [x, y, s.size] {
                assert!((v * TEXELS - (v * TEXELS).round()).abs() < 1e-3, "{v} on the texel grid");
            }
            assert!(x.abs() + s.size <= at.half.y + 1e-4 && y.abs() + s.size <= at.half.z + 1e-4, "inside its cell: {x} {y} {}", s.size);
            if off.0.abs() + half * 1.1 < 0.45 {
                assert!(s.size >= half * 1.1, "{} round a footprint of {half}", s.size);
            }
            assert_eq!(s.info >> 8 & 3, at.body as u32);
        }
        assert_eq!(shade(&at, (0.0, 0.0), 0.1, false), None, "none on glass or in water");
    }

    /// ADR 0114 review: through `lay` itself, a pile on solid opaque dry ground gets one contact shadow for all
    /// its stacks (overlapping shades would multiply darker), none over air (dug out from under it), glass,
    /// leaves or under water.
    #[test]
    fn a_pile_shades_only_solid_dry_opaque_ground_once() {
        use engine::blocks::{AIR, BLOCKS};
        use engine::liquid::{Ground, OPEN, U};
        let id = |n: &str| ITEMS.named(n).unwrap();
        let slots = vec![Some(Stack::new(id("hand axe"), 1)), Some(Stack::new(id("granite"), 3)), Some(Stack::new(id("flint"), 2)), Some(Stack::new(id("granite"), 1))];
        let shades = |under: Option<u16>, wet: bool| {
            let mut w = World::new(768, 256, 1);
            let b = w.air_over_ground(0, w.bodies[0].spawn);
            if let Some(m) = under {
                w.set_block(Block { pos: b.pos - glam::IVec3::Z, ..b }, m);
            }
            if wet {
                let floor = w.ground().gaps(World::col(b)).last().unwrap().0;
                w.water.set_column(&(World::col(b), vec![(floor, OPEN, U)]));
            }
            let piles = [(b, slots.clone())].into_iter().collect();
            lay(&w, &piles, w.block_centre(b), &|n, _, _| ["hand axe", "flint"].contains(&n).then_some(0.2)).2
        };
        for (what, under, wet) in [("air", Some(AIR), false), ("glass", Some(BLOCKS.id("glass")), false), ("leaves", Some(BLOCKS.id("broadleaf leaves")), false), ("water", None, true)] {
            assert!(shades(under, wet).is_empty(), "none over {what}");
        }
        assert_eq!(shades(None, false).len(), 1, "one shade for the pile's four stacks");
    }

    /// ADR 0114 review: a thing whose footprint would reach past its cell (a made-up long tool) is shrunk to fit
    /// it, turned any way, and its shadow follows the shrunk footprint.
    #[test]
    fn a_long_tool_shrinks_to_fit_its_cell() {
        let w = World::new(768, 256, 1);
        let b = w.air_over_ground(0, w.bodies[0].spawn);
        let at = fx::At::floor_of(&w, b);
        let slots = vec![Some(Stack::new(ITEMS.named("hand axe").unwrap(), 1))];
        let piles = [(b, slots)].into_iter().collect();
        let (long, footprint) = (0.8, |_: &str, _, _| Some(0.8));
        let (m, _, s) = lay(&w, &piles, w.block_centre(b), &footprint);
        let d = m[0].p - at.p;
        let (x, y) = (d.dot(at.axes[1]).abs(), d.dot(at.axes[2]).abs());
        let half = long * m[0].fit;
        assert!(m[0].fit < 1.0 && x + half <= at.half.y + 1e-4 && y + half <= at.half.z + 1e-4, "inside its cell: fit {} at {x} {y}", m[0].fit);
        assert!(s[0].size <= ((half * 1.15 * TEXELS).ceil() + 1.0) / TEXELS + 1e-4, "its shadow the shrunk footprint's: {}", s[0].size);
    }

    /// ADR 0114: a stack whose item has a loaded model lies as that model, the rest are cubes; the same pile is
    /// turned the same each time; past `PILE_DRAWN` nothing; a model not loaded falls back to the cube.
    #[test]
    fn a_pile_draws_its_models_and_cubes_for_the_rest() {
        let (models, cubes) = pile(&["hand axe", "flint"], 1.0);
        assert_eq!(models.iter().map(|m| (m.name.as_str(), m.item, m.tool)).collect::<Vec<_>>(), [("hand axe", true, true), ("flint", true, false)]);
        assert_eq!(cubes.len(), 1, "granite: a block's look is its cube");
        assert_eq!(cubes[0].info & fx::BLOCK_BITS, fx::block_bits(ITEMS.look(ITEMS.named("granite").unwrap())));
        // drawn as the block's own cell look (its painting, as the world has it; ADR 0114 second round), 0.2 m,
        // resting on the cell's floor: `p` its offset in cells (bit 19, as a glide's), its middle 0.1 over the floor
        assert_eq!((cubes[0].info >> 10) & 3, fx::Look::Block as u32, "the block's cell look, not a debris cube");
        assert_eq!(cubes[0].info >> 19 & 1, 1, "p is an offset in cells");
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
