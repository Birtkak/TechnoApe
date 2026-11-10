//! What the overlay pipelines draw (`overlay.wgsl`, ADR 0017): the ghost of a shape as lines, and the 2D
//! layer of crosshair and hotbar as rectangles.
use engine::edit::Shape;
use engine::world::{FxMap, World};
use glam::IVec3;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GhostVert {
    /// Relative to the body's centre.
    pub pos: [f32; 3],
    pub body: u32,
    pub colour: [u8; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Rect {
    /// x0, y0, x1, y1 in clip space, y0 below y1.
    pub xy: [f32; 4],
    pub colour: [f32; 4],
    /// 0 solid, 1..10 the digits 0..9.
    pub glyph: u32,
}

/// The most ghost vertices: every edge of every cell of the largest shape.
pub const GHOST_MAX: usize = engine::edit::MAX_CELLS as usize * 24;
pub const RECTS_MAX: usize = 512;

/// The edges of every face of the shape that has no shape cell beside it: its outline on the curved grid.
pub fn ghost(world: &World, s: &Shape, colour: [u8; 4]) -> Vec<GhostVert> {
    let cells: FxMap<IVec3, ()> = s.cells().map(|b| (b.pos, ())).collect();
    let body = &world.bodies[s.body as usize];
    let corner = |p: IVec3| {
        let v = body.dir(s.face as usize, p.x as f64, p.y as f64) * body.layer_r(p.z as f64);
        GhostVert { pos: v.as_vec3().to_array(), body: s.body as u32, colour }
    };
    let mut out = Vec::new();
    for p in cells.keys() {
        for a in 0..3 {
            for side in [0, 1] {
                let mut e = IVec3::ZERO;
                e[a] = side * 2 - 1;
                if cells.contains_key(&(*p + e)) {
                    continue;
                }
                let (mut u, mut v) = (IVec3::ZERO, IVec3::ZERO);
                u[(a + 1) % 3] = 1;
                v[(a + 2) % 3] = 1;
                let mut o = *p;
                o[a] += side;
                let q = [o, o + u, o + u + v, o + v];
                for k in 0..4 {
                    out.push(corner(q[k]));
                    out.push(corner(q[(k + 1) % 4]));
                }
            }
        }
        if out.len() >= GHOST_MAX {
            out.truncate(GHOST_MAX);
            break;
        }
    }
    out
}

/// One hotbar slot: the block's colour and, in survival, how many are held.
pub struct Slot {
    pub colour: [f32; 3],
    pub count: Option<u32>,
}

/// Crosshair and hotbar for a `w` x `h` pixel screen. `tool_height` shows beside the hotbar for cylinders
/// and cones.
pub fn layer(w: f32, h: f32, slots: &[Slot], selected: usize, creative: bool, tool_height: Option<i32>) -> Vec<Rect> {
    let mut out = Vec::new();
    // pixels from the top-left to clip space
    let mut rect = |x: f32, y: f32, rw: f32, rh: f32, colour: [f32; 4], glyph: u32| {
        let (x0, x1) = (x / w * 2.0 - 1.0, (x + rw) / w * 2.0 - 1.0);
        let (y0, y1) = (1.0 - (y + rh) / h * 2.0, 1.0 - y / h * 2.0);
        out.push(Rect { xy: [x0, y0, x1, y1], colour, glyph });
    };
    let (cx, cy) = (w / 2.0, h / 2.0);
    let white = [1.0, 1.0, 1.0, 0.85];
    rect(cx - 8.0, cy - 1.0, 16.0, 2.0, white, 0);
    rect(cx - 1.0, cy - 8.0, 2.0, 16.0, white, 0);

    let (size, gap) = (44.0, 6.0);
    let n = 9.0;
    let x0 = cx - (n * size + (n - 1.0) * gap) / 2.0;
    let y0 = h - size - 16.0;
    // a strip over the hotbar: gold in creative, grey in survival
    let mode = if creative { [0.95, 0.75, 0.2, 0.9] } else { [0.6, 0.6, 0.6, 0.6] };
    rect(x0, y0 - 10.0, n * size + (n - 1.0) * gap, 4.0, mode, 0);
    let digits = |x_right: f32, y_bottom: f32, value: u32, px: f32, rect: &mut dyn FnMut(f32, f32, f32, f32, [f32; 4], u32)| {
        let s = value.to_string();
        for (k, ch) in s.chars().rev().enumerate() {
            let x = x_right - (k as f32 + 1.0) * px * 4.0;
            let g = ch.to_digit(10).unwrap() + 1;
            rect(x + 1.0, y_bottom - px * 5.0 + 1.0, px * 3.0, px * 5.0, [0.0, 0.0, 0.0, 0.8], g);
            rect(x, y_bottom - px * 5.0, px * 3.0, px * 5.0, [1.0, 1.0, 1.0, 1.0], g);
        }
    };
    for k in 0..9 {
        let x = x0 + k as f32 * (size + gap);
        let frame = if k == selected { [1.0, 1.0, 1.0, 0.95] } else { [0.0, 0.0, 0.0, 0.5] };
        rect(x - 3.0, y0 - 3.0, size + 6.0, size + 6.0, frame, 0);
        match slots.get(k) {
            Some(sl) => {
                let dim = if sl.count == Some(0) { 0.35 } else { 1.0 };
                rect(x, y0, size, size, [sl.colour[0] * dim, sl.colour[1] * dim, sl.colour[2] * dim, 1.0], 0);
                if let Some(c) = sl.count {
                    digits(x + size - 3.0, y0 + size - 3.0, c, 2.0, &mut rect);
                }
            }
            None => rect(x, y0, size, size, [0.1, 0.1, 0.1, 0.4], 0),
        }
    }
    if let Some(hgt) = tool_height {
        let x = x0 - size - gap - 6.0;
        rect(x, y0, size, size, [0.15, 0.2, 0.3, 0.8], 0);
        digits(x + size - 6.0, y0 + size - 8.0, hgt.max(0) as u32, 4.0, &mut rect);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::edit::Kind;
    use engine::world::Block;

    #[test]
    fn a_ghost_outlines_only_the_outside() {
        let w = World::new(256, 32, 1);
        let b = Block { body: 0, face: 4, pos: IVec3::new(10, 10, 20) };
        let one = ghost(&w, &Shape::one(b), [255; 4]);
        assert_eq!(one.len(), 6 * 8, "a cube: six faces of four edges");
        let brush = Shape { max: b.pos + 1, ..Shape::one(b) };
        assert_eq!(ghost(&w, &brush, [255; 4]).len(), 24 * 8, "a 2 x 2 x 2 cube shows its 24 outer faces");
        let big = Shape { max: b.pos + IVec3::new(31, 31, 1), kind: Kind::Box, ..Shape::one(b) };
        assert!(ghost(&w, &big, [255; 4]).len() <= GHOST_MAX);
    }

    #[test]
    fn the_layer_has_a_crosshair_and_nine_slots() {
        let slots = [Slot { colour: [0.5; 3], count: Some(12) }];
        let r = layer(1280.0, 720.0, &slots, 0, false, None);
        assert!(r.len() >= 2 + 1 + 9 * 2 + 4, "{}", r.len());
        assert!(r.iter().all(|r| r.xy[0] < r.xy[2] && r.xy[1] < r.xy[3] && r.xy.iter().all(|v| (-1.0..=1.0).contains(v))));
        assert!(r.iter().any(|r| r.glyph == 2) && r.iter().any(|r| r.glyph == 3), "the count 12 as digits");
    }
}
