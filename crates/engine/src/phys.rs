//! Player movement (ADR 0007, 0024): every body's gravity, the body's own orientation, snappy walking, a jump, one-block step-up, and box collision
//! against the blocks. The server runs it per tick from each player's `Input`; the client runs the same code
//! per frame for its own player, takes the same numbered steps, and replays from the server's state (ADR 0019).
//!
//! Collision is in grid space: the player's box is `WIDTH` x `WIDTH` x `HEIGHT` in nominal blocks, aligned with
//! the columns and layers of the face under the feet, and it overlaps a cell or it does not. The answer depends
//! on the position alone (not the look, not the way there), so a position the physics accepts stays free.
use crate::blocks::BLOCKS;
use crate::world::{Block, World, BLOCK};
use glam::{DQuat, DVec3};
use serde::{Deserialize, Serialize};

pub const WIDTH: f64 = 0.6;
pub const HEIGHT: f64 = 1.8;
pub const EYE: f64 = 1.6;
pub const WALK: f64 = 4.3;
pub const JUMP: f64 = 4.8; // about 1.2 m high
pub const G: f64 = crate::world::G;
/// Swimming (ADR 0020): rising speed with jump held, m/s; a body floats with this share of it under water.
pub const SWIM: f64 = 2.0;
const FLOAT: f64 = 0.8;
/// The fixed step. The server and every client take the same steps, so the same input gives the same path
/// at any frame rate or tick rate; each side carries the time that did not fill a step over to the next.
pub const DT: f64 = 1.0 / 60.0;
const SUBSTEP: f64 = 0.25;
/// Creative flight (ADR 0023): the fastest a flying player may ask for, m/s.
pub const FLY_MAX: f64 = 1000.0;
const CONTACT: f64 = 0.001;
/// The body's own orientation (ADR 0024): gravity turns its up towards the pull this fast, rad/s, at full G;
/// with the square of the pull less higher up, so deep space leaves it as it is.
pub const RIGHT: f64 = 3.0;
/// Standing, or flying within NEAR of a body's surface, it is held upright this fast (rad/s): at once while
/// walking, and a smooth turn of a few tenths of a second when it comes in tilted.
pub const HOLD: f64 = 8.0;
pub const NEAR: f64 = 1500.0;
/// Out of a hold the mouse's pitch turns the body itself and Q/E roll it, where the pull is under FREE_G or
/// has not righted the body to within FREE_TILT yet; otherwise it tilts the head over the body's horizon.
pub const FREE_G: f64 = 2.0;
pub const FREE_TILT: f64 = 0.1;

/// What a player wants this tick: `mv` is forward and right in -1..1, `pitch` the head's look over the body's
/// horizon. The body's orientation is the player's own (`Player::fwd`, `Player::up`): `turn` turns it about its
/// up, radians to the left; `tilt` about its right, radians up, and `roll` about its look, radians to the right
/// (a client sends those two only where `free` says the body turns itself).
/// A non-zero `fwd` instead sets it outright, in the coordinates of the frame the player is in: for scripts
/// (bots, the soak). A client uses `turn` only, as a direction would mean another one once a step changes frames.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Default)]
pub struct Input {
    pub mv: [f32; 2],
    pub jump: bool,
    pub fwd: [f32; 3],
    pub pitch: f32,
    #[serde(default)]
    pub turn: f32,
    #[serde(default)]
    pub tilt: f32,
    #[serde(default)]
    pub roll: f32,
    /// Creative flight (ADR 0023): the speed in m/s, 0 walking. No gravity; jump rises, `down` sinks, and
    /// the box still collides. The server takes it from creative players only.
    #[serde(default)]
    pub fly: f32,
    #[serde(default)]
    pub down: bool,
    /// The world time the client stepped this at, 0 for none. Out of a body's frame where the player goes
    /// depends on where the bodies are, so the server steps it at the same time (`step_at`), or a client
    /// whose clock is a little off is corrected all the way.
    #[serde(default)]
    pub t: f64,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Player {
    /// The feet: bottom centre of the box.
    pub pos: DVec3,
    pub vel: DVec3,
    /// The body's look, square to its up, carried along as the player moves (no seam has a frame).
    pub fwd: DVec3,
    pub on_ground: bool,
    /// The body's own up (ADR 0024), which gravity turns towards its pull; zero until the first step stands
    /// the player upright.
    #[serde(default)]
    pub up: DVec3,
}

impl Player {
    pub fn at(pos: DVec3, fwd: DVec3) -> Player {
        Player { pos, vel: DVec3::ZERO, fwd, on_ground: false, up: DVec3::ZERO }
    }
    /// The body's up, or `gravity_up` before the first step has set it.
    pub fn body_up(&self, gravity_up: DVec3) -> DVec3 {
        if self.up.length_squared() > 0.5 { self.up } else { gravity_up }
    }
}

/// Up at a point and the pull's strength (m/s^2): against all the bodies' gravity added up (ADR 0024).
pub fn pull(world: &World, p: DVec3) -> (DVec3, f64) {
    let g = world.gravity(p);
    let s = g.length();
    (if s > 1e-12 { -g / s } else { world.up_at(p) }, s)
}

/// Local up at a point: against the pull.
pub fn up(world: &World, p: DVec3) -> DVec3 {
    pull(world, p).0
}

/// Whether gravity holds the body upright at once: standing, or flying near a body.
fn held(world: &World, p: &Player, flying: bool) -> bool {
    p.on_ground || flying && world.altitude(p.pos) < NEAR
}

/// Whether the mouse's pitch turns the body itself and Q/E roll it (`Input::tilt`, `Input::roll`): out of a
/// hold where the pull is weak, or has not righted the body yet. Else the pitch is the head's.
pub fn free(world: &World, p: &Player, flying: bool) -> bool {
    let (gu, s) = pull(world, p.pos);
    !held(world, p, flying) && (s < FREE_G || p.body_up(gu).angle_between(gu) > FREE_TILT)
}

/// `v` turned about `axis` by `a` radians.
fn turned(axis: DVec3, a: f64, v: DVec3) -> DVec3 {
    DQuat::from_axis_angle(axis, a) * v
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

/// The cells the player's box overlaps with its feet at `pos`. Cells past the face's edge are found through
/// space (the cell centre on the extended face, then the block there), which also holds across a cube corner.
pub fn cells(world: &World, pos: DVec3) -> impl Iterator<Item = Block> + '_ {
    let (bi, face, g) = world.grid_at(pos);
    let b = &world.bodies[bi];
    let (hw, ht) = (WIDTH * 0.5 / BLOCK, HEIGHT / BLOCK);
    // a cell [k, k + 1) overlaps the open interval (lo, hi) when k < hi and k + 1 > lo
    let span = |lo: f64, hi: f64| (lo.floor() as i32)..(hi.ceil() as i32);
    let (is, js, ls) = (span(g.x - hw, g.x + hw), span(g.y - hw, g.y + hw), span(g.z, g.z + ht));
    is.flat_map(move |i| {
        let ls = ls.clone();
        js.clone().flat_map(move |j| {
            ls.clone().map(move |l| {
                if (0..b.n).contains(&i) && (0..b.n).contains(&j) {
                    Block { body: bi as u8, face: face as u8, pos: glam::IVec3::new(i, j, l) }
                } else {
                    world.block_at(b.centre + b.dir(face, i as f64 + 0.5, j as f64 + 0.5) * b.layer_r(l as f64 + 0.5))
                }
            })
        })
    })
}

/// True when the player's box with its feet at `pos` overlaps a solid block.
pub fn blocked(world: &World, pos: DVec3) -> bool {
    // `cells` walks the layers of one column after another: one terrain lookup per column, not per cell
    let mut col: Option<((u8, u8, i32, i32), f64)> = None;
    cells(world, pos).any(|b| {
        if let Some(m) = world.edit(b) {
            return BLOCKS.solid(m);
        }
        let key = (b.body, b.face, b.pos.x, b.pos.y);
        let h = match col {
            Some((k, h)) if k == key => h,
            _ => world.column_height(b),
        };
        col = Some((key, h));
        BLOCKS.solid(world.generated_under(b, h))
    })
}

/// True when the player is inside a block: what the physics never lets happen by itself.
pub fn inside(world: &World, p: &Player) -> bool {
    blocked(world, p.pos)
}

/// True when block `b` holds part of the player's box: a solid block there would trap the player.
pub fn holds(world: &World, p: &Player, b: Block) -> bool {
    cells(world, p.pos).any(|c| c == b)
}

/// Moves `pos` by `d`, stopping at the first solid block; true when it hit something.
fn slide(world: &World, pos: &mut DVec3, d: DVec3) -> bool {
    if d.length_squared() == 0.0 {
        return false;
    }
    if !blocked(world, *pos + d) {
        *pos += d;
        return false;
    }
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..8 {
        let mid = 0.5 * (lo + hi);
        if blocked(world, *pos + d * mid) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    *pos += d * lo;
    true
}

/// One layer's thickness at `pos`, a little over: the step-up climbs exactly one block, wherever it is.
fn one_layer(world: &World, pos: DVec3) -> f64 {
    let b = &world.bodies[world.nearest(pos)];
    (pos - b.centre).length() * (b.lnq.exp() - 1.0) * 1.01
}

/// An input any side may step with: finite numbers only.
pub fn sane(inp: &Input) -> bool {
    inp.fwd.iter().chain(inp.mv.iter()).chain([&inp.pitch, &inp.fly, &inp.turn, &inp.tilt, &inp.roll]).all(|c| c.is_finite()) && inp.t.is_finite()
}

/// One step at the input's own world time (`Input::t`; the world's when 0), the world's clock put back after.
pub fn step_at(world: &mut World, p: &mut Player, inp: &Input, dt: f64) {
    let now = world.time;
    if inp.t != 0.0 {
        world.time = inp.t;
    }
    step(world, p, inp, dt);
    world.time = now;
}

/// One step of `dt` seconds for one player.
pub fn step(world: &World, p: &mut Player, inp: &Input, dt: f64) {
    let (up, pull) = pull(world, p.pos);
    let down = -up;
    let flying = inp.fly > 0.0;
    // the body's orientation: righted towards the pull as fast as it is strong (held upright standing or flying
    // near a body), then turned, tilted and rolled as the input says
    let look = DVec3::new(inp.fwd[0] as f64, inp.fwd[1] as f64, inp.fwd[2] as f64);
    let (mut bu, mut fwd) = (p.body_up(up), if look.is_finite() && look != DVec3::ZERO { look } else { p.fwd });
    let rate = if held(world, p, flying) { HOLD } else { RIGHT * (pull / G).powi(2) };
    let a = bu.angle_between(up);
    if a <= rate * dt {
        fwd = turned(bu.cross(up).normalize_or(fwd), a, fwd);
        bu = up;
    } else if rate > 0.0 {
        let axis = bu.cross(up).normalize_or(fwd.cross(bu).normalize_or(bu.any_orthonormal_vector()));
        (bu, fwd) = (turned(axis, rate * dt, bu), turned(axis, rate * dt, fwd));
    }
    fwd = tangent(fwd, bu, p.fwd);
    if inp.turn != 0.0 {
        fwd = turned(bu, inp.turn as f64, fwd);
    }
    if inp.tilt != 0.0 {
        let r = fwd.cross(bu).normalize();
        (bu, fwd) = (turned(r, inp.tilt as f64, bu), turned(r, inp.tilt as f64, fwd));
    }
    if inp.roll != 0.0 {
        bu = turned(fwd, inp.roll as f64, bu);
    }
    bu = bu.normalize();
    fwd = tangent(fwd, bu, p.fwd);
    (p.up, p.fwd) = (bu, fwd);
    // flight goes along the body's own axes; walking along the ground's, the body's look laid level on it
    let (fwd, right, up) = if flying {
        (fwd, fwd.cross(bu).normalize(), bu)
    } else {
        // looking straight up or down, the body's up shows the way it faces
        let f = tangent(fwd, up, -bu * fwd.dot(up).signum());
        (f, f.cross(up).normalize(), up)
    };

    // how much of the body is under water, 0 to 1
    let sub = match world.water_surface(p.pos + up * 0.05) {
        Some(r) => ((r - (p.pos - world.bodies[world.nearest(p.pos)].centre).length()) / HEIGHT).clamp(0.0, 1.0),
        None => 0.0,
    };
    let mv = DVec3::new(inp.mv[1].clamp(-1.0, 1.0) as f64, 0.0, inp.mv[0].clamp(-1.0, 1.0) as f64);
    if flying {
        // the move and up or down at the asked speed, nothing else
        let vert = inp.jump as i32 as f64 - inp.down as i32 as f64;
        p.vel = ((right * mv.x + fwd * mv.z).clamp_length_max(1.0) + up * vert) * (inp.fly as f64).min(FLY_MAX);
    } else {
        let wish = (right * mv.x + fwd * mv.z).clamp_length_max(1.0) * WALK * (1.0 - 0.5 * sub);
        let mut v_up = p.vel.dot(up) - pull * dt;
        if p.on_ground && inp.jump && sub < 0.3 {
            v_up = JUMP;
        }
        if sub > 0.0 {
            // buoyancy that holds the body FLOAT under, drag, and jump held swims up
            v_up += G / FLOAT * sub * dt;
            v_up *= 1.0 - (3.0 * sub * dt).min(1.0);
            if inp.jump && sub > 0.3 {
                v_up = v_up.max(SWIM);
            }
        }
        // snappy: the horizontal velocity is the wish, on the ground and in the air
        p.vel = wish + up * v_up;
    }

    let total = p.vel * dt;
    let n = (total.length() / SUBSTEP).ceil().max(1.0);
    let d = total / n;
    let (dr, df, du) = (right * d.dot(right), fwd * d.dot(fwd), up * d.dot(up));
    let rise = one_layer(world, p.pos);
    let mut on_ground = false;
    for _ in 0..n as usize {
        for h in [dr, df] {
            let was = p.pos;
            if slide(world, &mut p.pos, h) && p.on_ground && !flying {
                // step-up: one block up, the same move, then back down onto the step
                let mut q = was + up * rise;
                if !blocked(world, q) && !slide(world, &mut q, h) {
                    slide(world, &mut q, up * -rise);
                    p.pos = q;
                }
            }
        }
        if slide(world, &mut p.pos, du) {
            if d.dot(down) > 0.0 {
                on_ground = true;
            }
            p.vel -= up * p.vel.dot(up);
        }
    }
    p.on_ground = on_ground || (p.vel.dot(down) >= 0.0 && blocked(world, p.pos + down * (2.0 * CONTACT)));
    // out of a body's sphere of influence, or into one (ADR 0022): the body turns with the frame
    if let Some(r) = world.reframe(&mut p.pos, &mut p.vel) {
        (p.fwd, p.up) = (r * p.fwd, r * p.up);
    }
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
        // on land: the spawn (the middle of face 2 may be sea)
        let d = b.spawn;
        d * (b.height(d) + 0.6)
    }

    fn run(w: &World, p: &mut Player, inp: &Input, secs: f64) {
        for _ in 0..(secs * 20.0) as usize {
            step(w, p, inp, 0.05);
        }
    }

    #[test]
    fn flight_holds_still_rises_moves_at_its_speed_and_collides() {
        let w = world();
        let start = ground(&w) + up(&w, ground(&w)) * 20.0;
        let u = up(&w, start);
        let fwd = tangent(DVec3::X, u, DVec3::Z);
        let mut p = Player::at(start, fwd);
        let f = fwd.as_vec3().to_array();
        let hover = Input { fwd: f, fly: 30.0, ..Default::default() };
        run(&w, &mut p, &hover, 2.0);
        assert!(p.pos.distance(start) < 1e-9, "no gravity: {}", p.pos.distance(start));
        run(&w, &mut p, &Input { jump: true, ..hover }, 1.0);
        assert!(((p.pos - start).dot(u) - 30.0).abs() < 0.01, "rises at its speed");
        let at = p.pos;
        run(&w, &mut p, &Input { mv: [1.0, 0.0], ..hover }, 1.0);
        assert!((p.pos.distance(at) - 30.0).abs() < 0.5, "{}", p.pos.distance(at));
        // down into the ground: stops on it, outside the blocks
        run(&w, &mut p, &Input { down: true, ..hover }, 4.0);
        assert!(!inside(&w, &p) && p.on_ground, "landed");
        let fast = Input { fly: 1e9, ..hover };
        assert!(sane(&fast) && !sane(&Input { fly: f32::NAN, ..hover }));
    }

    #[test]
    fn the_look_stays_level_and_the_flight_straight_out_of_the_planets_frame() {
        // bug 2026-11-10_19-46-22: flying out at 1000 m/s, a frame's later steps looked the old frame's way and
        // the player fell back in and out of the planet's sphere of influence. A client frame takes a few steps
        // with one input; its turn goes with the first. The clock as the developer's save had it.
        let mut w = world();
        w.time = 239084231911.3;
        // 1 km up the 88 m test planet: a step bends up by under a degree, as at 7 km over the real one
        let start = ground(&w) + up(&w, ground(&w)) * 1000.0;
        let mut p = Player::at(start, tangent(DVec3::X, up(&w, start), DVec3::Z));
        let inp = Input { mv: [1.0, 0.0], jump: true, fly: 1000.0, ..Default::default() };
        let from_planet = |w: &World, p: &Player| (w.to_inertial(p.pos) - w.pose(0, w.time).centre).length();
        let (mut last, mut frames, mut left) = (from_planet(&w, &p), 0, false);
        while frames < 400 {
            for k in 0..3 {
                let inp = Input { turn: if k == 0 { 0.01 } else { 0.0 }, ..inp };
                let (was, was_up) = (p.fwd, p.body_up(up(&w, p.pos)));
                let f = w.frame_at(p.pos);
                step(&w, &mut p, &inp, DT);
                w.time += DT;
                let r = from_planet(&w, &p);
                assert!(r > last, "frame {frames} step {k}: back towards the planet, {last:.1} -> {r:.1} m");
                // held upright near the planet; past NEAR the body keeps its own way (ADR 0024)
                if w.altitude(p.pos) < NEAR - 50.0 {
                    assert!(p.fwd.dot(up(&w, p.pos)).abs() < 0.02, "the look leaves the horizon: {}", p.fwd.dot(up(&w, p.pos)));
                }
                if w.frame_at(p.pos) == f {
                    assert!(was.angle_between(p.fwd) < 0.03, "the look jumps {:.1} degrees", was.angle_between(p.fwd).to_degrees());
                    assert!(was_up.angle_between(p.up) < 0.03, "the body's up jumps {:.1} degrees", was_up.angle_between(p.up).to_degrees());
                }
                left |= w.frame_at(p.pos) == crate::world::INERTIAL;
                last = r;
            }
            frames += 1;
        }
        assert!(left, "out of the planet's frame at {last:.0} m");
    }

    #[test]
    fn gravity_is_the_same_from_either_frame_at_the_hand_over() {
        // bugs 2026-11-10_19-46-22, 20-44-48: down flipped where the frame changed; now every body pulls, in
        // whichever frame the place is written
        let mut w = world();
        w.time = 239084231911.3;
        let (c, soi) = (w.pose(0, w.time).centre, w.soi(0));
        for k in 0..50 {
            let dir = DVec3::new(1.0, 0.3 + k as f64 * 0.05, -0.2).normalize();
            let x = c + dir * soi * (0.9 + k as f64 * 0.01);
            let near = w.pose(0, w.time).rot * w.gravity(w.from_inertial(0, x));
            let far = w.gravity(w.from_inertial(crate::world::INERTIAL, x));
            assert!((near - far).length() < 1e-9 * far.length().max(1e-6), "{near} against {far}");
        }
        // G over the ground, the square of the distance less far out
        let g = ground(&w);
        assert!((w.gravity(g).length() - G).abs() < 0.01 && w.gravity(g).dot(up(&w, g)) < 0.0);
        let r = w.bodies[0].radius + crate::world::AMP;
        assert!((w.gravity(g.normalize() * r * 2.0).length() - G / 4.0).abs() < 0.01);
    }

    #[test]
    fn a_tilted_body_is_righted_as_it_falls_and_lands_upright() {
        let w = world();
        let g0 = ground(&w);
        let u = up(&w, g0);
        let mut p = Player::at(g0 + u * 300.0, tangent(DVec3::X, u, DVec3::Z));
        p.up = -u; // upside down
        let mut last = std::f64::consts::PI;
        for _ in 0..60 * 60 {
            step(&w, &mut p, &Input::default(), DT);
            let a = p.up.angle_between(up(&w, p.pos));
            assert!(a <= last + 1e-6 && last - a <= HOLD * DT + 1e-6, "turned {:.3} rad in a step", last - a);
            last = a;
            if p.on_ground {
                break;
            }
        }
        assert!(p.on_ground, "landed");
        step(&w, &mut p, &Input::default(), DT);
        assert!(p.up.angle_between(up(&w, p.pos)) < 1e-9 && p.fwd.dot(p.up).abs() < 1e-9, "upright on the ground");
        assert!(!free(&w, &p, false));
    }

    #[test]
    fn deep_space_lets_the_body_loop_and_roll() {
        let mut w = world();
        w.time = 239084231911.3;
        let x = w.pose(0, w.time).centre + DVec3::new(0.3, 1.0, 0.2).normalize() * w.soi(0) * 3.0;
        let (pos, f) = w.locate(x);
        assert_eq!(f, crate::world::INERTIAL);
        let u = up(&w, pos);
        let mut p = Player::at(pos, tangent(DVec3::X, u, DVec3::Z));
        p.up = u;
        assert!(free(&w, &p, false) && free(&w, &p, true), "free out here");
        let (f0, u0) = (p.fwd, p.up);
        let tilt = Input { tilt: std::f32::consts::FRAC_PI_2, fly: 1.0, ..Default::default() };
        step(&w, &mut p, &tilt, DT);
        assert!(p.fwd.angle_between(u0) < 1e-3, "a quarter turn up looks where up was");
        for _ in 0..3 {
            step(&w, &mut p, &tilt, DT);
        }
        assert!(p.fwd.angle_between(f0) < 1e-3 && p.up.angle_between(u0) < 1e-3, "a whole loop over the top");
        step(&w, &mut p, &Input { roll: std::f32::consts::PI, fly: 1.0, ..Default::default() }, DT);
        assert!(p.up.angle_between(-u0) < 1e-3 && p.fwd.angle_between(f0) < 1e-3, "rolled upside down");
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

    /// Dropped into the sea, a player floats with the head out, and swims up with jump held.
    #[test]
    fn floats_in_the_sea_and_swims_up() {
        let w = World::new(1536, 32, 1);
        let b = &w.bodies[0];
        let (i, j) = (0..b.n).step_by(3).flat_map(|i| (0..b.n).step_by(3).map(move |j| (i, j))).find(|&(i, j)| b.height(b.dir(2, i as f64 + 0.5, j as f64 + 0.5)) < b.radius - 8.0).unwrap();
        let d = b.dir(2, i as f64 + 0.5, j as f64 + 0.5);
        let sea = b.radius - crate::world::SEA_DEPTH;
        let mut p = Player::at(d * (sea - 1.0), DVec3::X);
        run(&w, &mut p, &Input::default(), 10.0);
        let under = sea - p.pos.length();
        assert!(!p.on_ground && under > 1.0 && under < 1.6, "feet {under} m under the surface");
        let mut q = Player::at(d * (sea - 3.5), DVec3::X);
        step(&w, &mut q, &Input { jump: true, ..Default::default() }, 0.5);
        run(&w, &mut q, &Input { jump: true, ..Default::default() }, 1.0);
        assert!(sea - q.pos.length() < 2.0, "swam up to {} m under", sea - q.pos.length());
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
            assert!(!inside(&w, &p), "inside a block at {:?}", p.pos);
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
        assert!(!blocked(&w, p.pos));
        // a block whose centre is 1.25 m up, between the old 0.9 and 1.8 m samples, at an edge midpoint
        let b = w.block_at(p.pos + up * 1.25 + fwd * 0.25);
        w.set_block(b, 1);
        assert!(blocked(&w, p.pos));
    }
}
