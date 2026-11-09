//! Player movement (ADR 0007): radial gravity, snappy walking, a jump, one-block step-up, and box collision
//! against the blocks. The server runs it per tick from each player's `Input`; the client runs the same code
//! per frame for its own player and corrects to the server's position when they drift apart.
use crate::world::World;
use glam::DVec3;
use serde::{Deserialize, Serialize};

pub const WIDTH: f64 = 0.6;
pub const HEIGHT: f64 = 1.8;
pub const EYE: f64 = 1.6;
pub const WALK: f64 = 4.3;
pub const JUMP: f64 = 4.8; // about 1.2 m high
pub const G: f64 = 9.81;
pub const STEP: f64 = 0.5; // one block
/// The fixed step. The server and every client take the same steps, so the same input gives the same path
/// at any frame rate or tick rate; each side carries the time that did not fill a step over to the next.
pub const DT: f64 = 1.0 / 60.0;
const SUBSTEP: f64 = 0.25;
const CONTACT: f64 = 0.001;

/// What a player wants this tick: `mv` is forward and right in -1..1, `fwd` the look direction.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
pub struct Input {
    pub mv: [f32; 2],
    pub jump: bool,
    pub fwd: [f32; 3],
    pub pitch: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Player {
    /// The feet: bottom centre of the box.
    pub pos: DVec3,
    pub vel: DVec3,
    /// Look direction kept in the tangent plane, carried along as the player walks (no seam has a frame).
    pub fwd: DVec3,
    pub on_ground: bool,
}

impl Player {
    pub fn at(pos: DVec3, fwd: DVec3) -> Player {
        Player { pos, vel: DVec3::ZERO, fwd, on_ground: false }
    }
}

/// Local up at a point: away from the nearest body's centre.
pub fn up(world: &World, p: DVec3) -> DVec3 {
    (p - world.bodies[world.nearest(p)].centre).normalize_or(DVec3::Y)
}

/// `v` without its component along `up`, unit length; `fallback` when nothing is left.
pub fn tangent(v: DVec3, up: DVec3, fallback: DVec3) -> DVec3 {
    let t = v - up * v.dot(up);
    if t.length_squared() < 1e-12 {
        let f = fallback - up * fallback.dot(up);
        return f.normalize_or(up.any_orthonormal_vector());
    }
    t.normalize()
}

/// True when the player's box at `pos` overlaps a solid block. Samples are at most `STEP` apart in height
/// and `WIDTH / 2` across, so no 0.5 m block can sit between them.
pub fn blocked(world: &World, pos: DVec3, right: DVec3, fwd: DVec3, up: DVec3) -> bool {
    let h = WIDTH * 0.5 - CONTACT;
    let layers = (HEIGHT / STEP).ceil() as usize;
    for k in 0..=layers {
        let z = (k as f64 * STEP).clamp(CONTACT, HEIGHT - CONTACT);
        for a in [-h, 0.0, h] {
            for b in [-h, 0.0, h] {
                let p = pos + up * z + right * a + fwd * b;
                if world.block(world.block_at(p)) != 0 {
                    return true;
                }
            }
        }
    }
    false
}

/// Moves `pos` by `d` along one axis, stopping at the first solid block; true when it hit something.
fn slide(world: &World, pos: &mut DVec3, d: DVec3, right: DVec3, fwd: DVec3, up: DVec3) -> bool {
    if d.length_squared() == 0.0 {
        return false;
    }
    if !blocked(world, *pos + d, right, fwd, up) {
        *pos += d;
        return false;
    }
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..8 {
        let mid = 0.5 * (lo + hi);
        if blocked(world, *pos + d * mid, right, fwd, up) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    *pos += d * lo;
    true
}

/// One step of `dt` seconds for one player.
pub fn step(world: &World, p: &mut Player, inp: &Input, dt: f64) {
    let up = up(world, p.pos);
    let look = DVec3::new(inp.fwd[0] as f64, inp.fwd[1] as f64, inp.fwd[2] as f64);
    let fwd = tangent(if look.is_finite() { look } else { p.fwd }, up, p.fwd);
    p.fwd = fwd;
    let right = fwd.cross(up).normalize();

    let mv = DVec3::new(inp.mv[1].clamp(-1.0, 1.0) as f64, 0.0, inp.mv[0].clamp(-1.0, 1.0) as f64);
    let wish = (right * mv.x + fwd * mv.z).clamp_length_max(1.0) * WALK;
    let mut v_up = p.vel.dot(up) - G * dt;
    if p.on_ground && inp.jump {
        v_up = JUMP;
    }
    // snappy: the horizontal velocity is the wish, on the ground and in the air
    p.vel = wish + up * v_up;

    let total = p.vel * dt;
    let n = (total.length() / SUBSTEP).ceil().max(1.0);
    let d = total / n;
    let (dr, df, du) = (right * d.dot(right), fwd * d.dot(fwd), up * d.dot(up));
    let mut on_ground = false;
    for _ in 0..n as usize {
        for h in [dr, df] {
            let was = p.pos;
            if slide(world, &mut p.pos, h, right, fwd, up) && p.on_ground {
                // step-up: one block up, the same move, then back down onto the step
                let mut q = was + up * STEP;
                if !blocked(world, q, right, fwd, up) && !slide(world, &mut q, h, right, fwd, up) {
                    slide(world, &mut q, up * -STEP, right, fwd, up);
                    p.pos = q;
                }
            }
        }
        if slide(world, &mut p.pos, du, right, fwd, up) {
            if d.dot(up) < 0.0 {
                on_ground = true;
            }
            p.vel -= up * p.vel.dot(up);
        }
    }
    p.on_ground = on_ground || (p.vel.dot(up) <= 0.0 && blocked(world, p.pos - up * (2.0 * CONTACT), right, fwd, up));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> World {
        World::new(256, 32, 1337)
    }

    /// Feet position of the surface in the middle of face 2 (+y).
    fn ground(w: &World) -> DVec3 {
        let b = &w.bodies[0];
        let d = b.dir(2, b.n as f64 / 2.0 + 0.25, b.n as f64 / 2.0 + 0.25);
        d * (b.height(d) + 0.6)
    }

    fn run(w: &World, p: &mut Player, inp: &Input, secs: f64) {
        for _ in 0..(secs * 20.0) as usize {
            step(w, p, inp, 0.05);
        }
    }

    #[test]
    fn falls_and_lands_on_the_surface() {
        let w = world();
        let start = ground(&w);
        let up = up(&w, start);
        let mut p = Player::at(start + up * 3.0, DVec3::X);
        run(&w, &mut p, &Input::default(), 3.0);
        assert!(p.on_ground);
        assert!(p.vel.dot(up).abs() < 1e-9);
        let below = w.block(w.block_at(p.pos - up * 0.01));
        let at = w.block(w.block_at(p.pos + up * 0.05));
        assert!(below != 0 && at == 0, "feet rest on a block: below {below}, at {at}");
    }

    #[test]
    fn jumps_about_a_metre_and_lands() {
        let w = world();
        let mut p = Player::at(ground(&w) + up(&w, ground(&w)) * 3.0, DVec3::X);
        run(&w, &mut p, &Input::default(), 3.0);
        let floor = p.pos;
        let up = up(&w, floor);
        step(&w, &mut p, &Input { jump: true, ..Default::default() }, 0.05);
        let mut peak: f64 = 0.0;
        for _ in 0..40 {
            step(&w, &mut p, &Input::default(), 0.05);
            peak = peak.max((p.pos - floor).dot(up));
        }
        assert!(peak > 0.9 && peak < 1.5, "peak {peak}");
        assert!(p.on_ground && (p.pos - floor).length() < 0.01);
    }

    #[test]
    fn walks_over_hills_without_sinking() {
        let w = world();
        let mut p = Player::at(ground(&w) + up(&w, ground(&w)) * 3.0, DVec3::X);
        run(&w, &mut p, &Input::default(), 3.0);
        let start = p.pos;
        let inp = Input { mv: [1.0, 0.0], jump: true, fwd: [1.0, 0.0, 0.0], ..Default::default() };
        for _ in 0..200 {
            step(&w, &mut p, &inp, 0.05);
            let up = up(&w, p.pos);
            assert!(!blocked(&w, p.pos, p.fwd.cross(up), p.fwd, up), "inside a block at {:?}", p.pos);
        }
        let up = up(&w, p.pos);
        let moved = (p.pos - start).reject_from(up).length();
        assert!(moved > 20.0, "walked {moved} m in 10 s");
        // fwd is tangent where the step began; one step on this 88 m test planet tilts up by ~0.002
        assert!((p.fwd.dot(up)).abs() < 1e-2 && (p.fwd.length() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_lone_block_at_head_height_blocks() {
        let mut w = world();
        let mut p = Player::at(ground(&w) + up(&w, ground(&w)) * 3.0, DVec3::X);
        run(&w, &mut p, &Input::default(), 3.0);
        let up = up(&w, p.pos);
        let fwd = tangent(DVec3::X, up, DVec3::X);
        let right = fwd.cross(up).normalize();
        assert!(!blocked(&w, p.pos, right, fwd, up));
        // a block whose centre is 1.25 m up, between the old 0.9 and 1.8 m samples, at an edge midpoint
        let b = w.block_at(p.pos + up * 1.25 + fwd * 0.25);
        w.set_block(b, 1);
        assert!(blocked(&w, p.pos, right, fwd, up));
    }
}
