//! Player movement (ADR 0007, 0024): every body's gravity, the body's own orientation, snappy walking, a jump (a ledge one block high needs it, ADR 0033), and box collision
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
/// Sprint (ADR 0118): the forward share of a walk this many times as fast.
pub const SPRINT: f64 = 1.5;
/// The push of a jump, m/s, the same on every body: 1.25 m high at G stepped at DT (ADR 0037); higher
/// where gravity is weaker.
pub const JUMP: f64 = 6.84;
pub const G: f64 = crate::interior::G;
/// Swimming (ADR 0020): rising speed with jump held, m/s; a body floats with this share of it under water.
pub const SWIM: f64 = 2.0;
const FLOAT: f64 = 0.8;
/// Swimming (ADR 0118): with the eyes under and nothing held the body sinks this fast, m/s; it moves this fast
/// along the ground's way without a sprint; a sprint strokes along the look at SWIM_FAST, once more than
/// STROKE_SUB of the body is in. Under water the velocity eases to what is wanted at WATER_EASE a second.
pub const SINK: f64 = 0.6;
pub const SWIM_SLOW: f64 = WALK * 0.5;
pub const SWIM_FAST: f64 = 4.0;
const STROKE_SUB: f64 = 0.6;
const WATER_EASE: f64 = 4.0;
/// Under means the head this far under the surface, m: a body that dips under while floating comes up again.
const UNDER: f64 = 0.2;
/// Climbing out of the water onto a bank (ADR 0118), m/s: the push that lifts the feet a block and a half.
const CLIMB: f64 = 8.5; // 6.0 at the Earth's pull: the same 1.8 m of lift at Terra's twice (ADR 0132)
/// The fixed step. The server and every client take the same steps, so the same input gives the same path
/// at any frame rate or tick rate; each side carries the time that did not fill a step over to the next.
pub const DT: f64 = 1.0 / 60.0;
const SUBSTEP: f64 = 0.25;
/// Creative flight (ADR 0023): the fastest a flying player may ask for, m/s.
pub const FLY_MAX: f64 = 10_000.0;
const CONTACT: f64 = 0.001;
/// How far a walk lifts over a snag in the floor, metres: far under a block, so a ledge needs a jump.
const SKIM: f64 = 0.02;
/// The body's own orientation (ADR 0024): gravity turns its up towards the pull this fast, rad/s, at full G;
/// with the square of the pull less higher up, so deep space leaves it as it is.
pub const RIGHT: f64 = 3.0;
/// Standing, or flying within NEAR of a body's surface, it is held upright this fast (rad/s): at once while
/// walking, and a smooth turn of a few tenths of a second when it comes in tilted.
pub const HOLD: f64 = 8.0;
pub const NEAR: f64 = 1500.0;
/// Out of a hold the mouse's pitch turns the body itself and Q/E roll it, where the pull is under FREE_G or
/// has not righted the body to within FREE_TILT yet; otherwise it tilts the head over the body's horizon.
pub const FREE_G: f64 = G / 9.81;
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
    /// Sprint held (ADR 0118): walking, the forward share of the move is `SPRINT` times as fast. Not in this
    /// struct's bytes, so recordings from before read: it rides `ToServer::Sprints` and `ToServer::Sprint`.
    #[serde(skip)]
    pub sprint: bool,
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

/// Whether gravity holds the body upright at once: standing, or near a body when flying or when its pull `s` is
/// under FREE_G. A fall near a weak moon is not space: free there, a block's drop down a shaft folded the look
/// into the body and the landing swung it level (bug 2026-11-21_18-02-54). A fall under a stronger pull is
/// righted at RIGHT as before (it is never free once upright), so earlier recordings replay.
fn held(world: &World, p: &Player, flying: bool, s: f64) -> bool {
    p.on_ground || (flying || s < FREE_G) && world.altitude(p.pos) < NEAR
}

/// Whether the mouse's pitch turns the body itself and Q/E roll it (`Input::tilt`, `Input::roll`): out of a
/// hold where the pull is weak, or has not righted the body yet. Else the pitch is the head's.
pub fn free(world: &World, p: &Player, flying: bool) -> bool {
    let (gu, s) = pull(world, p.pos);
    !held(world, p, flying, s) && (s < FREE_G || p.body_up(gu).angle_between(gu) > FREE_TILT)
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
    // on the warped grid (ADR 0131) a column can be a third of a metre: the box's half width in the columns here
    let (hi, hj) = if b.warp == 0.0 {
        (hw, hw)
    } else {
        let r = b.layer_r(g.z);
        let wide = |di: f64, dj: f64| (b.dir(face, g.x + di, g.y + dj) - b.dir(face, g.x - di, g.y - dj)).length() * r;
        (WIDTH * 0.5 / wide(0.5, 0.0), WIDTH * 0.5 / wide(0.0, 0.5))
    };
    // a cell [k, k + 1) overlaps the open interval (lo, hi) when k < hi and k + 1 > lo
    let span = |lo: f64, hi: f64| (lo.floor() as i32)..(hi.ceil() as i32);
    let (is, js, ls) = (span(g.x - hi, g.x + hi), span(g.y - hj, g.y + hj), span(g.z, g.z + ht));
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
    // a gas body's cloud tops hold the box as a floor (step 6a): shortcut: the dive under them is its own run.
    // Its cells are all air, so only its sphere blocks; a world without gas pays nothing here.
    if world.bodies.iter().any(|g| g.gas && (pos - g.centre).length_squared() < g.radius * g.radius) {
        return true;
    }
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
        world.generated_solid_under(b, h)
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

/// Auto jump (the settings, ADR 0033): a walking player has a ledge one block high in the way, with room
/// on top of it. The client turns this into a jump; the server sees an ordinary jump.
pub fn ledge_ahead(world: &World, p: &Player) -> bool {
    p.on_ground && ledge(world, p, p.vel)
}

/// Under water (ADR 0118): the head UNDER below the surface; a body already sinking stays under a little
/// shallower, one rising leaves a little deeper, so a wobbling water level does not flick it between the two.
fn under(deep: f64, v_up: f64) -> bool {
    deep > HEIGHT + UNDER + 0.1 || deep > HEIGHT + UNDER - 0.1 && v_up <= 0.0
}

/// A bank to climb out on along `dir` (ADR 0118): solid just under the water's surface ahead, and room to stand
/// within 1.5 m over the surface.
fn bank(world: &World, p: &Player, dir: DVec3, up: DVec3, deep: f64) -> bool {
    let ahead = p.pos + dir * (WIDTH * 0.5 + 0.3);
    solid_at(world, ahead + up * (deep - 0.05)) && !blocked(world, ahead + up * (deep + 1.5))
}

fn solid_at(world: &World, x: DVec3) -> bool {
    BLOCKS.get(world.block(world.block_at(x))).is_some_and(|d| d.solid)
}

/// A ledge one block high in the way of moving along `dir`, with room on top of it.
fn ledge(world: &World, p: &Player, dir: DVec3) -> bool {
    let up = up(world, p.pos);
    let dir = dir.reject_from(up);
    if dir.length() < 0.5 {
        return false;
    }
    // across the body's width: a ledge's corner stops the box though the middle of the way is free
    let (dir, side) = (dir.normalize(), dir.cross(up).normalize() * (WIDTH * 0.5));
    let solid = |h: f64| {
        [-1.0, 0.0, 1.0].iter().any(|k| {
            let id = world.block(world.block_at(p.pos + dir * (WIDTH * 0.5 + 0.5) + side * *k + up * h));
            crate::blocks::BLOCKS.get(id).is_some_and(|d| d.solid)
        })
    };
    solid(0.5 * BLOCK) && !solid(1.5 * BLOCK) && !solid(2.5 * BLOCK)
}

/// Whether a player at `pos` (moving `step` metres this step) is held by a body whose ground is not in yet (step
/// 6b, `World::loaded`): within its highest ground plus `HOLD_M`, or a step's travel.
pub fn held_by_a_body(world: &World, pos: DVec3, step: f64) -> bool {
    world.lazy.iter().enumerate().any(|(i, l)| l.as_ref().is_some_and(|l| !l.loaded) && {
        let b = &world.bodies[i];
        (pos - b.centre).length() < b.radius + crate::world::AMP + HOLD_M.max(2.0 * step)
    })
}
/// A player found inside the ground when a body's ground comes in (put there by a teleport while it was not in,
/// step 6b) is set on it, along its radius; its inward speed gone. Others are left as they are.
pub fn onto_ground(world: &World, p: &mut Player) {
    let b = world.frame_at(p.pos);
    if b >= world.bodies.len() || !inside(world, p) {
        return;
    }
    let n = (p.pos - world.bodies[b].centre).normalize_or(DVec3::Y);
    // up a quarter metre at a time, at most to the highest ground's height over the body (`AMP`) and a bit
    for _ in 0..((crate::world::AMP + world.bodies[b].radius) * 4.0) as usize {
        if !inside(world, p) {
            break;
        }
        p.pos += n * 0.25;
    }
    p.vel -= n * p.vel.dot(n).min(0.0);
}

/// How far over a not-yet-loaded body's highest ground a player is held (`held_by_a_body`).
pub const HOLD_M: f64 = 200.0;

/// An input any side may step with: finite numbers only.
pub fn sane(inp: &Input) -> bool {
    inp.fwd.iter().chain(inp.mv.iter()).chain([&inp.pitch, &inp.fly, &inp.turn, &inp.tilt, &inp.roll]).all(|c| c.is_finite()) && inp.t.is_finite()
}

/// One step at the input's own world time (`Input::t`; the world's when 0), the world's clock put back after.
/// True when it sprinted, as `step`.
pub fn step_at(world: &mut World, p: &mut Player, inp: &Input, dt: f64) -> bool {
    let now = world.time;
    if inp.t != 0.0 {
        world.time = inp.t;
    }
    let sprinted = step(world, p, inp, dt);
    world.time = now;
    sprinted
}

/// One step of `dt` seconds for one player. True when it sprinted (ADR 0118): the sprint was applied and the
/// body went further forward than the walk would have taken it, so a sprint into a wall or in the water is
/// none. What a sprint's hunger and its wider view go by.
pub fn step(world: &World, p: &mut Player, inp: &Input, dt: f64) -> bool {
    // a body whose ground is not in yet (step 6b) holds one near it where it is until it is: nothing to stand on or
    // bump into there yet, and neither falling nor flying through it. A pause, not a brake: its speed is kept for
    // when the ground comes, and it may look round meanwhile
    if held_by_a_body(world, p.pos, p.vel.length() * dt) {
        let look = DVec3::new(inp.fwd[0] as f64, inp.fwd[1] as f64, inp.fwd[2] as f64);
        if look.is_finite() && look != DVec3::ZERO {
            p.fwd = tangent(look, p.body_up(pull(world, p.pos).0), p.fwd);
        }
        return false;
    }
    // one found inside a gas body's sphere (a bad save, `at=`, a teleport) would be blocked every way out: put it
    // on the cloud tops along its radius, its inward speed gone
    for g in world.bodies.iter().filter(|b| b.gas) {
        let r = p.pos - g.centre;
        if r.length_squared() < g.radius * g.radius {
            let n = r.normalize_or(DVec3::Y);
            p.pos = g.centre + n * (g.radius + 1e-3);
            p.vel -= n * p.vel.dot(n).min(0.0);
        }
    }
    let start = p.pos;
    let (up, pull) = pull(world, p.pos);
    let down = -up;
    let flying = inp.fly > 0.0;
    // the body's orientation: righted towards the pull as fast as it is strong (held upright standing or flying
    // near a body), then turned, tilted and rolled as the input says
    let look = DVec3::new(inp.fwd[0] as f64, inp.fwd[1] as f64, inp.fwd[2] as f64);
    let (mut bu, mut fwd) = (p.body_up(up), if look.is_finite() && look != DVec3::ZERO { look } else { p.fwd });
    let rate = if held(world, p, flying, pull) { HOLD } else { RIGHT * (pull / G).powi(2) };
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

    // how deep the feet are under the fluid's surface, and how much of the body is under it, 0 to 1
    let fluid = world.fluid_surface(p.pos + up * 0.05);
    let deep = fluid.map_or(0.0, |(r, _)| r - (p.pos - world.bodies[world.nearest(p.pos)].centre).length());
    // the fluid's row (lava basics): a body floats with `float` of it under (Archimedes: its density over the fluid's,
    // FLOAT in water) and moves through it slowed by its viscosity; only water is swum in. Water's ratio is 1.0, so
    // its numbers are the very ones they were
    let rows = crate::fluids::FLUIDS.now();
    let kind = fluid.map_or(crate::fluids::WATER, |f| f.1);
    let float = FLOAT * (rows.rows[0].density as f64 / rows.get(kind).density as f64);
    let visc = rows.get(kind).viscosity as f64;
    let swims = kind == crate::fluids::WATER;
    let sub = (deep / HEIGHT).clamp(0.0, 1.0);
    let mv = DVec3::new(inp.mv[1].clamp(-1.0, 1.0) as f64, 0.0, inp.mv[0].clamp(-1.0, 1.0) as f64);
    // the forward share of the walk a sprint speeds, 0 for none
    let mut sprint_share = 0.0;
    // swimming (ADR 0118): a sprint forward with most of the body in strokes along the look; a body already
    // swimming fast keeps stroking a little shallower, so the switch does not flicker (hysteresis from the speed)
    let swimming = !p.on_ground && p.vel.length() > SWIM_SLOW + 0.5;
    let stroke = !flying && swims && inp.sprint && mv.z > 0.0 && sub > if swimming { STROKE_SUB - 0.1 } else { STROKE_SUB };
    let look = fwd * (inp.pitch as f64).cos() + up * (inp.pitch as f64).sin();
    if flying {
        // the move and up or down at the asked speed, nothing else
        let vert = inp.jump as i32 as f64 - inp.down as i32 as f64;
        p.vel = ((right * mv.x + fwd * mv.z).clamp_length_max(1.0) + up * vert) * (inp.fly as f64).min(FLY_MAX);
    } else {
        let dir = (right * mv.x + fwd * mv.z).clamp_length_max(1.0);
        // sprint (ADR 0118): the forward share of the walk, out of the water (wading to the shins still sprints)
        if inp.sprint && sub < 0.3 && dir.dot(fwd) > 0.0 {
            sprint_share = dir.dot(fwd);
        }
        let dir = dir + fwd * sprint_share * (SPRINT - 1.0);
        // shortcut: a thick fluid slows the walk by 1 + (visc - 1) sub / 16 (lava at the float line: about half again),
        // a taste default; from a drag law on the body once something else wades
        // snow short of a block is no floor: the feet sink through it to the ground and the walk is slower by its depth
        // (ADR 0189, `snow::DRAG`); a full block of it is a block, stood on
        let feet = world.block_at(p.pos + up * 0.05);
        let sunk = world.snow.sunk(World::col(feet), feet.pos.z);
        let wish = dir * WALK * (1.0 - 0.5 * sub) / (1.0 + (visc - 1.0) * sub / 16.0) * (1.0 - crate::snow::DRAG * sunk);
        // climbing OUT: jump held (or a stroke) in real water near the surface, the look not down, against a
        // bank whose top is at or over the surface and within reach; elsewhere a stroke slides along a wall
        // (real water: a third of the body in, or still in with water, not a floor, under the feet; a puddle's
        // jump is the old jump)
        let climb = (sub > 0.3 || sub > 0.0 && !solid_at(world, p.pos - up * 0.6))
            && deep <= HEIGHT + UNDER
            && (inp.jump || stroke)
            && inp.pitch > -0.3
            && dir.length() > 0.1
            && blocked(world, p.pos + up * 0.05 + dir.normalize() * 0.1)
            && bank(world, p, dir.normalize(), up, deep);
        if stroke || swims && under(deep, p.vel.dot(up)) {
            // under water, or stroking: no gravity, no buoyancy; the velocity eases to the stroke along the look,
            // or to the slow move with jump rising, the down key sinking and nothing a slow sink
            let want = if stroke {
                let want = (look * (SWIM_FAST * mv.z) + right * (SWIM_SLOW * mv.x)).clamp_length_max(SWIM_FAST);
                // at the surface the stroke holds the float line: the look's rise is clipped there (no hop out)
                let rise = if climb { f64::INFINITY } else { (deep - FLOAT * HEIGHT) * WATER_EASE };
                want - up * (want.dot(up) - rise).max(0.0)
            } else {
                dir * SWIM_SLOW + up * if inp.jump { SWIM } else if inp.down { -SWIM } else { -SINK }
            };
            p.vel += (want - p.vel) * (WATER_EASE * dt).min(1.0);
            if climb {
                let lift = if ledge(world, p, dir) { CLIMB } else { SWIM };
                p.vel += up * (lift - p.vel.dot(up)).max(0.0);
            }
        } else {
            let mut v_up = p.vel.dot(up) - pull * dt;
            if p.on_ground && inp.jump && sub < 0.3 {
                v_up = JUMP;
            }
            if sub > 0.0 {
                // buoyancy that holds the body FLOAT under, drag, and jump held swims up
                v_up += G / float * sub * dt;
                v_up *= 1.0 - (3.0 * sub * dt).min(1.0);
                // a thick fluid's drag, implicit so it holds at any viscosity (water's 1 divides by 1)
                v_up /= 1.0 + 3.0 * (visc - 1.0) * sub * dt;
                if inp.jump && sub > 0.3 || climb {
                    v_up = v_up.max(SWIM);
                }
                // the down key dives from the surface, as it sinks under it
                if swims && inp.down && sub > 0.3 && !inp.jump {
                    v_up = v_up.min(-SWIM);
                }
                // a ledge in the way climbs out (W5): a river's bed steps a block under shallow water, where
                // swimming up never lifts the feet over the step
                // (and a bank over the surface, climbed to its edge, is jumped onto: ADR 0118)
                if (inp.jump || stroke) && v_up < JUMP && ledge(world, p, wish) {
                    v_up = if climb { CLIMB } else { JUMP };
                }
            }
            // snappy: the horizontal velocity is the wish, on the ground and in the air
            p.vel = wish + up * v_up;
        }
    }

    let total = p.vel * dt;
    // over every body's top layer nothing can be in the way: one move, not the 670 substeps of 10 km/s
    let clear = flying && world.altitude(p.pos) > crate::world::AMP + crate::world::BUILD_HEADROOM + HEIGHT + 1.0 + total.length();
    let n = if clear { 1.0 } else { (total.length() / SUBSTEP).ceil().max(1.0) };
    let d = total / n;
    let (dr, df, du) = (right * d.dot(right), fwd * d.dot(fwd), up * d.dot(up));
    let mut on_ground = false;
    for _ in 0..n as usize {
        for h in [dr, df] {
            let was = p.pos;
            if slide(world, &mut p.pos, h) && p.on_ground && !flying {
                // the floor itself can be in the way by a hair (up is the pull's, which another body tilts
                // off the grid's): the same move from SKIM higher, then back down. A ledge still stops it.
                let mut q = was + up * SKIM;
                if !blocked(world, q) && !slide(world, &mut q, h) {
                    slide(world, &mut q, up * -SKIM);
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
    // a sprint went further forward than the walk's share would in the step, with a twentieth to spare; a
    // stroke moved along the look at all (5 cm/s), not held against a wall
    let sprinted = sprint_share > 0.0 && (p.pos - start).dot(fwd) > WALK * sprint_share * dt * 1.05
        || stroke && (p.pos - start).dot(look) > 0.05 * dt;
    // out of a body's sphere of influence, or into one (ADR 0022): the body turns with the frame
    if let Some(r) = world.reframe(&mut p.pos, &mut p.vel) {
        (p.fwd, p.up) = (r * p.fwd, r * p.up);
    }
    sprinted
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
    fn flight_reaches_ten_km_a_second_and_a_dive_at_it_still_lands() {
        // bug 2026-11-12_20-56-50: "make flying go up to 10.000 m/s"
        let w = world();
        let g = ground(&w);
        let u = up(&w, g);
        let start = g + u * 3000.0;
        let mut p = Player::at(start, tangent(DVec3::X, u, DVec3::Z));
        let inp = Input { fly: 10_000.0, down: true, ..Default::default() };
        step(&w, &mut p, &inp, DT);
        // far over the highest layer the step is one move, and the same move the substeps would make
        assert!(((start - p.pos).length() - 10_000.0 * DT).abs() < 1e-6, "{} m", (start - p.pos).length());
        for _ in 0..60 {
            step(&w, &mut p, &inp, DT);
        }
        assert!(!inside(&w, &p) && p.on_ground, "through the ground: {} m from it", (p.pos - g).length());
        assert!((p.pos - g).length() < 2.0, "{} m from the ground", (p.pos - g).length());
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
        // about G on the ground, the square of the distance less far out
        let g = ground(&w);
        assert!((w.gravity(g).length() - G).abs() < 0.05 * G && w.gravity(g).dot(up(&w, g)) < 0.0);
        let r = w.bodies[0].radius;
        assert!((w.gravity(g.normalize() * r * 2.0).length() - G / 4.0).abs() < 0.01);
    }

    #[test]
    fn a_tilted_body_is_righted_as_it_falls_and_lands_upright() {
        let w = world();
        let g0 = ground(&w);
        let u = up(&w, g0);
        let mut p = Player::at(g0 + u * 300.0, tangent(DVec3::X, u, DVec3::Z));
        p.up = -u; // upside down
        // (from the angle it starts at: the pull 300 m up is a hair off the ground's up)
        let mut last = p.up.angle_between(up(&w, p.pos));
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

    /// Bug report 2026-11-21_18-02-54 ("mining down in a moon with not good gravity every time u fall a block down
    /// ur entire screen flips"): on the giant's icy moon, whose pull is under FREE_G, a player looking down a shaft
    /// drops a block at a time. The client (frame.rs) folds the head's pitch into the body whenever `free` says so,
    /// and the landing rights the body: the look, the body turned by the head's pitch as frame.rs draws it, must
    /// not swing on any step.
    #[test]
    fn looking_down_a_shaft_on_a_weak_moon_the_look_holds_through_each_drop() {
        let mut w = World::with_grid(1536, crate::bodies::moon_columns(1536), 1, None, None, 0.0, 7);
        let bi = crate::bodies::find("callisto").unwrap();
        let land = |w: &World, p: &mut Player| {
            for _ in 0..600 {
                step(w, p, &Input::default(), DT);
                if p.on_ground {
                    return;
                }
            }
            panic!("never landed");
        };
        let top = w.bodies[bi].centre + DVec3::Y * (w.bodies[bi].height(DVec3::Y) + 1.0);
        let u = up(&w, top);
        let mut p = Player::at(top, tangent(DVec3::X, u, DVec3::Z));
        land(&w, &mut p);
        // stood in the middle of a column, so the dug block is the only one under the box
        let b0 = w.block_at(p.pos - u * 0.5);
        p = Player::at(w.block_centre(b0) + u * 0.6, p.fwd);
        land(&w, &mut p);
        let g = pull(&w, p.pos).1;
        assert!(g < FREE_G, "the report's case is a pull under FREE_G: {g}");
        // the client's look as frame.rs draws it: the body's look turned by the head's pitch over its horizon
        let look = |p: &Player, pitch: f64| {
            let up = p.body_up(u);
            DQuat::from_axis_angle(p.fwd.cross(up).normalize(), pitch) * p.fwd
        };
        let (mut pitch, mut tilt) = (-1.5f64, 0.0f64);
        let mut last = look(&p, pitch);
        for k in 0..4 {
            w.set_block(w.block_at(p.pos - up(&w, p.pos) * 0.5), 0);
            for _ in 0..240 {
                if free(&w, &p, false) {
                    tilt += std::mem::take(&mut pitch);
                }
                let inp = Input { pitch: pitch as f32, tilt: std::mem::take(&mut tilt) as f32, ..Default::default() };
                step(&w, &mut p, &inp, DT);
                let now = look(&p, pitch);
                assert!(now.angle_between(last) < 0.05, "drop {k}: the look swung {:.1} degrees in one step", now.angle_between(last).to_degrees());
                last = now;
            }
            assert!(p.on_ground, "drop {k}: landed");
        }
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
        // (the sea's top sits in a layer, up to a block from its nominal depth)
        let under = w.water_surface(p.pos).expect("in the water") - p.pos.length();
        assert!(!p.on_ground && under > 1.0 && under < 1.6, "feet {under} m under the surface");
        let mut q = Player::at(d * (sea - 3.5), DVec3::X);
        step(&w, &mut q, &Input { jump: true, ..Default::default() }, 0.5);
        run(&w, &mut q, &Input { jump: true, ..Default::default() }, 1.0);
        assert!(sea - q.pos.length() < 2.0, "swam up to {} m under", sea - q.pos.length());
    }

    /// A pit 3 x 3 columns and 4 deep at the spawn holding fluid `kind` 3 blocks deep (stored, poured), and its
    /// middle column's direction and fluid surface's radius.
    fn pit(kind: u8) -> (World, DVec3, f64) {
        let mut w = world();
        let b = w.bodies[0].clone();
        let (f, i, j) = b.locate(b.spawn);
        let (f, i, j) = (f as u8, i as i32, j as i32);
        let top = b.first_air(b.col_height(f, i, j));
        let z0 = top - 4;
        for (di, dj, z) in (-1..=1).flat_map(|di| (-1..=1).flat_map(move |dj| (z0..top + 2).map(move |z| (di, dj, z)))) {
            w.set_block(crate::world::Block { body: 0, face: f, pos: glam::IVec3::new(i + di, j + dj, z) }, crate::blocks::AIR);
        }
        {
            let World { bodies, edits, water, sky, time, .. } = &mut w;
            let g = crate::world::Solid::at(bodies, edits, sky, *time);
            for (di, dj) in (-1..=1).flat_map(|di| (-1..=1).map(move |dj| (di, dj))) {
                assert!(water.pour_kind(&g, crate::liquid::Col::new(0, f, i + di, j + dj), z0, 3 * crate::liquid::U, kind));
            }
        }
        let d = b.dir(f as usize, i as f64 + 0.5, j as f64 + 0.5);
        let r = b.layer_r(z0 as f64 + 3.0);
        (w, d, r)
    }

    /// A body floats in a fluid by Archimedes (lava basics, draft decision): it sinks until the share of it under
    /// is its density over the fluid's, FLOAT in water, FLOAT times 1000 / 2600 in lava, so lava carries it mostly
    /// out; and it does not sink to the floor of a pit of lava.
    #[test]
    fn a_body_floats_higher_in_lava_than_in_water_by_the_densities() {
        let rows = crate::fluids::FLUIDS.now();
        let lava = rows.kind("lava").unwrap();
        let under = |kind: u8| {
            let (w, d, r) = pit(kind);
            let mut p = Player::at(w.bodies[0].centre + d * (r - 0.5), DVec3::X);
            run(&w, &mut p, &Input::default(), 15.0);
            assert!(!p.on_ground, "kind {kind}: on the pit's floor");
            r - (p.pos - w.bodies[0].centre).length()
        };
        let (water, molten) = (under(crate::fluids::WATER), under(lava));
        let want = water * rows.rows[0].density as f64 / rows.get(lava).density as f64;
        assert!((water - FLOAT * HEIGHT).abs() < 0.05, "{water} m under in water");
        assert!((molten - want).abs() < 0.05, "{molten} m under in lava, want {want}");
    }

    /// Water carries a body as it did before fluids had densities, to the bit (client prediction and the server
    /// share `step`; recordings replay): a drop, a float, a jump, a swim and a dive in a pit of stored water, hashed.
    #[test]
    fn a_body_in_water_moves_as_before_to_the_bit() {
        let (w, d, r) = pit(crate::fluids::WATER);
        let mut p = Player::at(w.bodies[0].centre + d * (r + 1.0), DVec3::X);
        let mut h = crate::world::FNV0;
        let script = [(Input::default(), 4.0), (Input { jump: true, ..Default::default() }, 1.0), (Input { mv: [0.3, 1.0], ..Default::default() }, 1.0), (Input { down: true, ..Default::default() }, 1.5), (Input { mv: [0.0, 1.0], sprint: true, ..Default::default() }, 1.0)];
        for (inp, secs) in &script {
            for _ in 0..(secs / DT) as usize {
                step(&w, &mut p, inp, DT);
                h = p.pos.to_array().iter().chain(&p.vel.to_array()).fold(h, |h, x| crate::world::fnv(h, &x.to_le_bytes()));
            }
        }
        // the hash of the trace before fluid densities (main at 791a5003)
        assert_eq!(h, 17861800262558682817, "the water's trace");
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

    /// The highest a jump at the client's step lifts the feet, standing at `floor` direction `d` of body `bi`.
    fn peak(w: &World, bi: usize) -> (f64, f64) {
        let b = &w.bodies[bi];
        let d = if bi == 0 { b.spawn } else { DVec3::Y };
        let mut p = Player::at(b.centre + d * (b.height(d) + 3.0), DVec3::X);
        for _ in 0..240 {
            step(w, &mut p, &Input::default(), DT);
        }
        assert!(p.on_ground);
        let (floor, (up, g)) = (p.pos, pull(w, p.pos));
        step(w, &mut p, &Input { jump: true, ..Default::default() }, DT);
        let mut top: f64 = 0.0;
        for _ in 0..600 {
            step(w, &mut p, &Input::default(), DT);
            top = top.max((p.pos - floor).dot(up));
        }
        assert!(p.on_ground);
        (top, g)
    }

    /// Bug 2026-11-11_13-08-58 (ADR 0037): on Terra a jump tops out at 1.25 blocks; the same push on a
    /// smaller body, with less gravity, goes higher.
    #[test]
    fn a_jump_is_a_block_and_a_quarter_on_terra_and_higher_on_luna() {
        let w = World::new(6144, crate::bodies::moon_columns(6144), 1337);
        let (h, g) = peak(&w, 0);
        // at the spawn the ground is a little off sea level, so the pull is a little off G
        let want = 1.25 * G / g;
        assert!((h - want).abs() < 0.015 && (h - 1.25).abs() < 0.05, "{h} m against {want} (pull {g})");
        let (hl, gl) = peak(&w, 1);
        eprintln!("Terra: {h:.3} m at {g:.3} m/s^2; Luna: {hl:.2} m at {gl:.3} m/s^2");
        assert!(gl < G / 3.0 && hl > 4.0 && (hl * gl / (h * g) - 1.0).abs() < 0.03, "Luna: {hl} m at {gl}");
    }

    /// Terra pulls twice as hard as the Earth (the developer, 2026-11-15: the Earth's felt floaty): a jump is
    /// still a block and a quarter, over in 0.71 s instead of 1.0 s.
    #[test]
    fn terra_pulls_twice_the_earths_and_a_jump_is_quicker() {
        assert!((G - 2.0 * 9.81).abs() < 1e-9, "{G}");
        let w = World::new(6144, crate::bodies::moon_columns(6144), 1337);
        let b = &w.bodies[0];
        let mut p = Player::at(b.centre + b.spawn * (b.height(b.spawn) + 3.0), DVec3::X);
        for _ in 0..240 {
            step(&w, &mut p, &Input::default(), DT);
        }
        step(&w, &mut p, &Input { jump: true, ..Default::default() }, DT);
        let mut steps = 1;
        while !p.on_ground && steps < 600 {
            step(&w, &mut p, &Input::default(), DT);
            steps += 1;
        }
        let air = steps as f64 * DT;
        assert!((air - 0.71).abs() < 0.04, "{air} s in the air");
    }

    /// Gravity (ADR 0037, 0041): G at Terra's sea level whatever its size, its radius over
    /// Terra's on another body, weaker deeper in by the radius (ADR 0152), and the square of the distance less out.
    #[test]
    fn gravity_scales_with_the_body_and_falls_towards_the_core() {
        for (n, m) in [(6144, 256), (768, 32)] {
            let w = World::new(n, m, 1);
            let (t, l) = (&w.bodies[0], &w.bodies[1]);
            let at = |b: &crate::world::Body, r: f64| w.gravity(b.centre + DVec3::Y * r).length();
            assert!((at(t, t.radius) - G).abs() < 1e-3, "{}", at(t, t.radius));
            let gl = G * l.radius / t.radius;
            assert!((at(l, l.radius) - gl).abs() < 0.02 * gl, "{} against {gl}", at(l, l.radius));
            for b in [t, l] {
                let s = at(b, b.radius);
                assert!(at(b, b.radius * 0.98) > s && (at(b, b.radius * 0.5) - s * 0.8027).abs() < 0.01 * s, "{} {} {s}", at(b, b.radius * 0.98), at(b, b.radius * 0.5));
            }
            // (out from Luna the planet's pull comes back in, so only Terra shows the plain square law)
            assert!((at(t, t.radius * 2.0) - G / 4.0).abs() < 0.01 * G);
        }
    }

    /// A ledge one block high stops a walk; a jump clears it (ADR 0033: no step-up on 1 m blocks).
    #[test]
    fn a_block_high_ledge_needs_a_jump() {
        let mut w = world();
        let b0 = w.block_at(ground(&w) - up(&w, ground(&w)) * 0.5);
        let at = |dx: i32, dy: i32, dz: i32| Block { pos: b0.pos + glam::IVec3::new(dx, dy, dz), ..b0 };
        // a floor, clear above, and a wall one block high across it three blocks ahead
        for dx in -3..=7 {
            for dy in -3..=3 {
                w.set_block(at(dx, dy, 0), 3);
                for dz in 1..=5 {
                    w.set_block(at(dx, dy, dz), if dz == 1 && dx == 3 { 3 } else { 0 });
                }
            }
        }
        let start = w.block_centre(b0);
        let u = up(&w, start);
        let dir = (w.block_centre(at(1, 0, 0)) - start).reject_from(u).normalize();
        let f = dir.as_vec3().to_array();
        let mut p = Player::at(start + u * 0.6, dir);
        run(&w, &mut p, &Input { fwd: f, ..Default::default() }, 1.0);
        let walk = Input { mv: [1.0, 0.0], fwd: f, ..Default::default() };
        run(&w, &mut p, &walk, 3.0);
        let along = |p: &Player| (p.pos - start).dot(dir);
        assert!(p.on_ground && along(&p) > 1.5 && along(&p) < 2.5, "the ledge stops the walk, at {} m", along(&p));
        // auto jump (the settings): the ledge in the way is seen and jumped, flat ground is not
        let mut q = p.clone();
        assert!(ledge_ahead(&w, &q), "the ledge is seen");
        let mut hops = 0;
        for _ in 0..40 {
            let jump = ledge_ahead(&w, &q);
            hops += (jump && q.on_ground) as i32;
            step(&w, &mut q, &Input { jump, ..walk }, 0.05);
        }
        assert!(along(&q) > 4.0 && hops <= 2, "auto jump clears it: {} m along, {hops} hops", along(&q));
        run(&w, &mut p, &Input { jump: true, ..walk }, 1.0);
        assert!(along(&p) > 4.0, "a jump clears it: {} m along", along(&p));
    }

    /// A level floor never stops a walk, whichever way it goes: the pull's up is a hair off the grid's
    /// (the other body tilts it), so a level move can point into the floor (found in play, ADR 0033).
    #[test]
    fn a_level_floor_is_walked_every_way() {
        let mut w = World::new(6144, 256, 1337);
        let d = w.bodies[0].spawn;
        let b0 = w.block_at(w.bodies[0].centre + d * (w.bodies[0].height(d) - 0.5));
        for dx in -8..=8 {
            for dy in -8..=8 {
                for dz in 0..=5 {
                    w.set_block(Block { pos: b0.pos + glam::IVec3::new(dx, dy, dz), ..b0 }, if dz == 0 { 3 } else { 0 });
                }
            }
        }
        let start = w.block_centre(b0);
        let u = up(&w, start);
        for k in 0..8 {
            let a = k as f64 * std::f64::consts::FRAC_PI_4;
            let dir = tangent(DVec3::X, u, DVec3::Z);
            let dir = dir * a.cos() + dir.cross(u) * a.sin();
            let mut p = Player::at(start + u * 0.6, dir);
            let f = dir.as_vec3().to_array();
            run(&w, &mut p, &Input { fwd: f, ..Default::default() }, 1.0);
            run(&w, &mut p, &Input { mv: [1.0, 0.0], fwd: f, ..Default::default() }, 1.0);
            let moved = (p.pos - start).reject_from(u).length();
            assert!(moved > 3.5, "walked {moved} m in 1 s, way {k}");
        }
    }

    /// Bug report 2026-11-11_12-59-49 ("i get stuck on nothing it seems", in a room dug 6 m down): a room
    /// two blocks high under the ground is walked every way, at the spawn and far from it, where the
    /// grid's up and the pull's lie further apart.
    #[test]
    fn a_dug_room_is_walked_every_way() {
        let mut w = World::new(6144, 256, 1337);
        for d in [w.bodies[0].spawn, (w.bodies[0].spawn + DVec3::new(0.4, 0.1, 0.3)).normalize()] {
            let b0 = w.block_at(w.bodies[0].centre + d * (w.bodies[0].height(d) - 6.5));
            for dx in -5..=5 {
                for dy in -5..=5 {
                    for dz in 0..=3 {
                        w.set_block(Block { pos: b0.pos + glam::IVec3::new(dx, dy, dz), ..b0 }, if dz == 0 || dz == 3 { 3 } else { 0 });
                    }
                }
            }
            let start = w.block_centre(b0);
            let u = up(&w, start);
            for k in 0..8 {
                let a = k as f64 * std::f64::consts::FRAC_PI_4;
                let dir = tangent(DVec3::X, u, DVec3::Z);
                let dir = dir * a.cos() + dir.cross(u) * a.sin();
                let mut p = Player::at(start + u * 0.6, dir);
                let f = dir.as_vec3().to_array();
                run(&w, &mut p, &Input { fwd: f, ..Default::default() }, 1.0);
                run(&w, &mut p, &Input { mv: [1.0, 0.0], fwd: f, ..Default::default() }, 1.0);
                let moved = (p.pos - start).reject_from(u).length();
                assert!(moved > 3.5 && !inside(&w, &p), "walked {moved} m in 1 s, way {k}, at {d:?}");
            }
        }
    }

    /// Sprint (ADR 0118): held, the forward share of the walk is SPRINT times as fast on a level floor; strafe and
    /// backward stay at the walk, and so do swimming and flight.
    #[test]
    fn sprint_is_half_as_fast_again_forward_only() {
        let mut w = World::new(6144, 256, 1337);
        let d = w.bodies[0].spawn;
        let b0 = w.block_at(w.bodies[0].centre + d * (w.bodies[0].height(d) - 0.5));
        for dx in -12..=12 {
            for dy in -12..=12 {
                for dz in 0..=5 {
                    w.set_block(Block { pos: b0.pos + glam::IVec3::new(dx, dy, dz), ..b0 }, if dz == 0 { 3 } else { 0 });
                }
            }
        }
        let start = w.block_centre(b0);
        let u = up(&w, start);
        let dir = tangent(DVec3::X, u, DVec3::Z);
        let f = dir.as_vec3().to_array();
        // the way it went along the look and across it in 60 steps, from standing
        let go = |mv: [f32; 2], sprint: bool| {
            let mut p = Player::at(start + u * 0.6, dir);
            for _ in 0..60 {
                step(&w, &mut p, &Input { fwd: f, ..Default::default() }, DT);
            }
            let s = p.pos;
            for _ in 0..60 {
                step(&w, &mut p, &Input { mv, fwd: f, sprint, ..Default::default() }, DT);
            }
            ((p.pos - s).dot(dir), (p.pos - s).dot(dir.cross(u)))
        };
        let (walk, ran) = (go([1.0, 0.0], false), go([1.0, 0.0], true));
        assert!((walk.0 - WALK).abs() < 0.05 && (ran.0 - WALK * 1.5).abs() < 0.05, "walk {walk:?}, sprint {ran:?}");
        for mv in [[0.0, 1.0], [-1.0, 0.0], [0.0, -1.0]] {
            let (a, b) = (go(mv, false), go(mv, true));
            assert!((a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9, "{mv:?}: {a:?} against {b:?}");
        }
        // a diagonal: its forward share sprints, its strafe does not
        let (a, b) = (go([1.0, 1.0], false), go([1.0, 1.0], true));
        assert!((b.0 - a.0 * 1.5).abs() < 0.05 && (b.1 - a.1).abs() < 1e-4, "{a:?} against {b:?}");
        // flight: the same path with the bit or without
        let fly = |sprint: bool| {
            let mut p = Player::at(start + u * 3.0, dir);
            run(&w, &mut p, &Input { mv: [1.0, 0.0], fwd: f, fly: 20.0, sprint, ..Default::default() }, 1.0);
            p.pos
        };
        assert_eq!(fly(false), fly(true));
    }

    /// A deep spot of the sea on a small planet: the world, the way up there, the sea's surface radius and a
    /// level way ahead.
    fn sea() -> (World, DVec3, f64, DVec3) {
        let w = World::new(1536, 32, 1);
        let b = &w.bodies[0];
        let (i, j) = (0..b.n).step_by(3).flat_map(|i| (0..b.n).step_by(3).map(move |j| (i, j))).find(|&(i, j)| b.height(b.dir(2, i as f64 + 0.5, j as f64 + 0.5)) < b.radius - 12.0).unwrap();
        let d = b.dir(2, i as f64 + 0.5, j as f64 + 0.5);
        let sea = b.radius - crate::world::SEA_DEPTH;
        let r = w.water_surface(d * (sea - 3.0)).expect("the sea");
        let ahead = tangent(DVec3::X, d, DVec3::Z);
        (w, d, r, ahead)
    }

    /// Swimming (ADR 0118): with the head under, the body sinks slowly (SINK), not falling; jump held rises at
    /// SWIM and, let go once the head is out, the body floats with the head out.
    #[test]
    fn under_water_the_body_sinks_slowly_and_jump_rises_to_float() {
        let (w, d, r, ahead) = sea();
        let mut p = Player::at(d * (r - 5.0), ahead);
        run(&w, &mut p, &Input::default(), 2.0);
        let v = p.vel.dot(d);
        assert!((v + SINK).abs() < 0.05, "sinks at {v} m/s");
        // from 5 m under (the feet), jump held: up at SWIM, the time to the surface noted
        let mut q = Player::at(d * (r - 5.0), ahead);
        let mut t = 0.0;
        while q.pos.length() + EYE < r && t < 10.0 {
            step(&w, &mut q, &Input { jump: true, ..Default::default() }, DT);
            t += DT;
        }
        eprintln!("eyes out from 5 m in {t:.2} s");
        assert!(t < 5.0 / SWIM + 0.5, "{t} s to the surface");
        // let go: it floats, the head out, and stays so
        for _ in 0..600 {
            step(&w, &mut q, &Input::default(), DT);
        }
        let feet = r - q.pos.length();
        assert!(feet > 1.0 && feet < EYE, "floats with the feet {feet} m under, the eyes out");
        // and holding jump it bobs at the surface, no higher than the body out of it
        for _ in 0..600 {
            step(&w, &mut q, &Input { jump: true, ..Default::default() }, DT);
            let feet = r - q.pos.length();
            assert!(feet > -0.6 && feet < EYE, "jump held at the surface: feet {feet} m under");
        }
        // floating at the surface, the down key dives (the filmstrip found it held the body up)
        for _ in 0..600 {
            step(&w, &mut q, &Input::default(), DT);
        }
        run(&w, &mut q, &Input { down: true, ..Default::default() }, 2.0);
        let feet = r - q.pos.length();
        eprintln!("down held 2 s from floating: feet {feet:.2} m under");
        assert!(feet > HEIGHT + UNDER + 1.0, "down held at the surface: feet only {feet} m under");
    }

    /// Sprint under water swims along the look, pitch and all, at SWIM_FAST once it has sped up.
    #[test]
    fn a_sprint_swims_along_the_look_at_its_speed() {
        let (w, d, r, ahead) = sea();
        for pitch in [-0.6f32, 0.0, 0.5] {
            let mut p = Player::at(d * (r - 6.0), ahead);
            let inp = Input { mv: [1.0, 0.0], sprint: true, pitch, fwd: ahead.as_vec3().to_array(), ..Default::default() };
            run(&w, &mut p, &Input { fwd: inp.fwd, ..Default::default() }, 0.5);
            for _ in 0..90 {
                step(&w, &mut p, &inp, DT);
            }
            let look = ahead * (pitch as f64).cos() + d * (pitch as f64).sin();
            let v = p.vel;
            assert!((v.length() - SWIM_FAST).abs() < 0.1 && v.normalize().dot(look) > 0.999, "pitch {pitch}: {v:?} against {look:?}");
            // let go, the drag slows it smoothly to the slow sink
            let before = p.vel.length();
            step(&w, &mut p, &Input { fwd: inp.fwd, ..Default::default() }, DT);
            assert!(p.vel.length() < before && p.vel.length() > before * 0.8, "slows smoothly: {before} to {}", p.vel.length());
        }
        // without sprint the head-under move is the slow one
        let mut p = Player::at(d * (r - 6.0), ahead);
        run(&w, &mut p, &Input { mv: [1.0, 0.0], fwd: ahead.as_vec3().to_array(), ..Default::default() }, 2.0);
        let across = p.vel.reject_from(d).length();
        assert!((across - SWIM_SLOW).abs() < 0.05, "{across}");
    }

    /// Builds a stone floor `depth` m under the surface (a block thick, cells -6..=12 by -6..=6 around the spot; the sea over it is as generated)
    /// and, from 4 blocks ahead on, a bank up to the first layer at least `bank` m over the surface; the feet's
    /// start on the floor, the bank's top over the surface and the way to it.
    fn shore(w: &mut World, d: DVec3, r: f64, depth: f64, bank: Option<f64>) -> (DVec3, f64, DVec3) {
        let b0 = w.block_at(d * (r - depth - 0.5));
        let layer_r = |w: &World, l: i32| w.bodies[0].layer_r(l as f64);
        let top = bank.map(|h| (0..).map(|k| b0.pos.z + k).find(|&l| layer_r(w, l) >= r + h).unwrap());
        for dx in -6..=12 {
            for dy in -6..=6 {
                for dz in -1..=8 {
                    let c = Block { pos: b0.pos + glam::IVec3::new(dx, dy, dz), ..b0 };
                    let solid = dz <= 0 || top.is_some_and(|t| dx >= 4 && c.pos.z < t);
                    if solid {
                        w.set_block(c, 3);
                    }
                }
            }
        }
        // the way across the grid's rows towards the bank
        let next = Block { pos: b0.pos + glam::IVec3::X, ..b0 };
        let way = (w.block_centre(next) - w.block_centre(b0)).reject_from(d).normalize();
        (w.block_centre(b0) + d * 0.5, top.map_or(0.0, |t| layer_r(w, t) - r), way)
    }

    /// Snow in eighths (ADR 0189): three eighths are no floor, the feet sink through to the ground and the walk
    /// is slower by `DRAG` x 3/8 (19 %); a full block of snow is stood on.
    #[test]
    fn a_body_sinks_in_three_eighths_of_snow_walks_slower_and_stands_on_a_block_of_it() {
        let mut w = world();
        let g = ground(&w);
        let u = up(&w, g);
        let dir = tangent(DVec3::X, u, DVec3::Z);
        let f = dir.as_vec3().to_array();
        let walk = |w: &World| {
            let mut p = Player::at(g, dir);
            run(w, &mut p, &Input { fwd: f, ..Default::default() }, 1.0);
            let (s, z) = (p.pos, w.block_at(p.pos + u * 0.05).pos.z);
            for _ in 0..20 {
                step(w, &mut p, &Input { mv: [1.0, 0.0], fwd: f, ..Default::default() }, DT);
            }
            (s, z, (p.pos - s).reject_from(u).length(), p.on_ground)
        };
        let (s0, z0, clear, _) = walk(&w);
        let b = w.block_at(s0 + u * 0.05);
        let snow = BLOCKS.id("snow");
        let cols: Vec<_> = (-8..=8).flat_map(|i| (-8..=8).map(move |j| (i, j))).map(|(i, j)| crate::liquid::Col::new(b.body, b.face, b.pos.x + i, b.pos.y + j)).collect();
        // three eighths on the cell the feet stand in, wherever the walk goes
        let seen: Vec<_> = cols.iter().map(|&c| (c, Some(crate::snow::Seen { z: z0, top: 0.375, eighths: 3, block: snow }))).collect();
        w.snow.apply(&seen);
        let (s1, z1, sunk, on) = walk(&w);
        eprintln!("walked {clear:.3} m clear, {sunk:.3} m in three eighths ({:.3})", sunk / clear);
        assert!(on && z1 == z0 && (s1 - s0).dot(u).abs() < 1e-6, "the feet on the ground under the snow");
        assert!((sunk / clear - (1.0 - crate::snow::DRAG * 0.375)).abs() < 0.01, "slowed by the factor: {}", sunk / clear);
        // a full block of snow where the feet stood: stood on, a block higher
        w.snow.apply(&cols.iter().map(|&c| (c, None)).collect::<Vec<_>>());
        w.set_block(b, snow);
        let mut p = Player::at(s0 + u * 1.5, dir);
        run(&w, &mut p, &Input { fwd: f, ..Default::default() }, 2.0);
        assert!(p.on_ground && w.block_at(p.pos + u * 0.05).pos.z == z0 + 1, "on top of the snow block");
    }

    /// Wading (ADR 0118): on the bottom with the feet in and the head out, it is a walk, slower than on land.
    #[test]
    fn wading_in_shallow_water_walks_slower() {
        // (a block deep too: there the body wades on the bottom, it does not sink or float off)
        for depth in [0.8, 1.0] {
            let (mut w, d, r, _) = sea();
            let (feet, _, dir) = shore(&mut w, d, r, depth, None);
            let u = up(&w, feet);
            let f = dir.as_vec3().to_array();
            let mut p = Player::at(feet + u * 0.01, dir);
            run(&w, &mut p, &Input { fwd: f, ..Default::default() }, 1.0);
            assert!(p.on_ground, "{depth} m deep: standing on the bottom");
            let s = p.pos;
            for _ in 0..60 {
                step(&w, &mut p, &Input { mv: [1.0, 0.0], fwd: f, ..Default::default() }, DT);
            }
            let v = (p.pos - s).dot(dir);
            eprintln!("wading {depth} m deep at {v:.2} m/s");
            assert!(p.on_ground && v > 0.6 * WALK && v < 0.9 * WALK, "{depth} m deep: {v} m/s");
        }
    }

    /// Out of the water (ADR 0118): swimming at a bank a block over the surface (its top 0.5 to 1.5 m over it),
    /// jump held, climbs onto it, and so does a sprint's stroke.
    #[test]
    fn swimming_at_a_bank_a_block_high_climbs_out() {
        let (mut w, d, r, _) = sea();
        let (feet, bank, dir) = shore(&mut w, d, r, 6.0, Some(0.5));
        let u = up(&w, feet);
        let f = dir.as_vec3().to_array();
        let mut p = Player::at(feet + u * 4.0, dir);
        // float up to the surface, then at the bank with jump held
        run(&w, &mut p, &Input { jump: true, fwd: f, ..Default::default() }, 3.0);
        for sprint in [false, true] {
            // (the bank is 9 blocks wide: the first steps stood on its top count)
            let mut q = p;
            let mut out = None;
            for k in 0..300 {
                step(&w, &mut q, &Input { mv: [1.0, 0.0], jump: true, sprint, fwd: f, ..Default::default() }, DT);
                let (on, over) = ((q.pos - feet).dot(dir), q.pos.length() - r);
                if q.on_ground && on > 4.0 && (over - bank).abs() < 0.1 {
                    out = Some(k as f64 * DT);
                    break;
                }
            }
            eprintln!("sprint {sprint}: out on a bank {bank:.2} m over the surface after {out:?} s");
            assert!(out.is_some(), "sprint {sprint}: never on the bank {bank} m over the surface, at {:?}", q.pos.length() - r);
        }
    }

    /// The review of ADR 0118's swimming: a stroke ahead and to the side together is no faster than one ahead.
    #[test]
    fn a_diagonal_stroke_is_no_faster() {
        let (w, d, r, ahead) = sea();
        let mut p = Player::at(d * (r - 6.0), ahead);
        let inp = Input { mv: [1.0, 1.0], sprint: true, fwd: ahead.as_vec3().to_array(), ..Default::default() };
        for _ in 0..120 {
            step(&w, &mut p, &inp, DT);
        }
        let v = p.vel.length();
        eprintln!("a diagonal stroke at {v:.2} m/s");
        assert!(v < SWIM_FAST + 0.02, "a diagonal stroke at {v} m/s");
    }

    /// The review of ADR 0118's swimming: a stroke down along a cliff face under water slides down it; the
    /// climb out of the water is no push up there.
    #[test]
    fn a_dive_along_a_cliff_keeps_going_down() {
        let (mut w, d, r, _) = sea();
        let (feet, _, dir) = shore(&mut w, d, r, 7.0, Some(0.5));
        let u = up(&w, feet);
        let f = dir.as_vec3().to_array();
        let mut p = Player::at(feet + u * 3.5 + dir * 3.0, dir);
        let inp = Input { mv: [1.0, 0.0], sprint: true, pitch: -0.6, fwd: f, ..Default::default() };
        let mut last = p.pos.length();
        for k in 0..90 {
            step(&w, &mut p, &inp, DT);
            let h = p.pos.length();
            assert!(h <= last + 1e-9, "step {k}: up {} m along the cliff", h - last);
            last = h;
        }
    }

    /// The review of ADR 0118's swimming: a stroke at a block-high step of the sea floor, deep under, does not
    /// throw the body up.
    #[test]
    fn a_stroke_at_a_seabed_step_does_not_launch() {
        let (mut w, d, r, _) = sea();
        let (feet, _, dir) = shore(&mut w, d, r, 7.0, Some(-6.0));
        let u = up(&w, feet);
        let f = dir.as_vec3().to_array();
        let mut p = Player::at(feet + u * 0.2 + dir * 2.0, dir);
        let inp = Input { mv: [1.0, 0.0], sprint: true, fwd: f, ..Default::default() };
        let mut top = 0f64;
        for _ in 0..120 {
            step(&w, &mut p, &inp, DT);
            top = top.max(p.vel.dot(u));
        }
        assert!(top < 0.5, "thrown up at {top} m/s");
    }

    /// The review of ADR 0118's swimming: in a puddle (under the wading depth) a jump at a ledge is the old
    /// jump, not the climb out of the water.
    #[test]
    fn a_jump_at_a_ledge_in_a_puddle_is_the_old_jump() {
        // on land: a channel two cells long, a block deep, walled round, a ledge a block high at its end, and
        // water poured into it an eighth of a block deep
        let mut w = World::new(1536, 32, 1);
        let d = w.bodies[0].spawn;
        let g = w.block_at(w.bodies[0].centre + d * (w.bodies[0].height(d) - 0.5));
        let at = |dx: i32, dy: i32, dz: i32| Block { pos: g.pos + glam::IVec3::new(dx, dy, dz), ..g };
        for dx in -3..=6 {
            for dy in -3..=3 {
                for dz in 0..=4 {
                    let open = dz >= 2 || dz == 1 && dy == 0 && (dx == 0 || dx == 1);
                    w.set_block(at(dx, dy, dz), if open { 0 } else { 3 });
                }
                w.regap(at(dx, dy, 1));
            }
        }
        let col = |b: Block| crate::liquid::Col::new(b.body, b.face, b.pos.x, b.pos.y);
        {
            let World { bodies, edits, water, sky, time, .. } = &mut w;
            assert!(water.pour(&crate::world::Solid::at(bodies, edits, sky, *time), col(at(0, 0, 1)), at(0, 0, 1).pos.z, crate::liquid::U / 4));
        }
        for _ in 0..120 {
            w.step_water();
        }
        let b0 = at(0, 0, 1);
        let floor = w.bodies[0].layer_r(b0.pos.z as f64);
        let gap = w.water_surface(w.block_centre(b0) - up(&w, w.block_centre(b0)) * 0.49).map_or(0.0, |s| s - floor);
        assert!(gap > 0.02 && gap < 0.3 * HEIGHT, "the puddle is {gap} m deep");
        let next = at(1, 0, 1);
        let dir = (w.block_centre(next) - w.block_centre(b0)).reject_from(d).normalize();
        let u = up(&w, w.block_centre(b0));
        let f = dir.as_vec3().to_array();
        // (against the ledge, so the jump starts in the water)
        let mut p = Player::at(w.block_centre(next) + dir * 0.15 - u * 0.45, dir);
        run(&w, &mut p, &Input { fwd: f, ..Default::default() }, 0.5);
        assert!(p.on_ground, "standing in the puddle");
        let (start, mut top, mut stood) = (p.pos.length(), 0f64, 0f64);
        for _ in 0..90 {
            step(&w, &mut p, &Input { mv: [1.0, 0.0], jump: true, fwd: f, ..Default::default() }, DT);
            top = top.max(p.vel.dot(u));
            if p.on_ground {
                stood = stood.max(p.pos.length() - start);
            }
        }
        eprintln!("a puddle {gap:.2} m deep: up at {top:.2} m/s, stood {stood:.2} m higher");
        // (a centimetre a second over: the puddle's own lift in the step of the jump)
        assert!(top < JUMP + 0.01, "pushed up at {top} m/s, not a jump's {JUMP}");
        assert!(stood > 0.9, "never up on the ledge: {stood} m");
    }

    /// The review of ADR 0118's swimming: a stroke along the surface, looking a little up, holds the float line
    /// at a steady speed and sprints on every step (no flicker between stroking and floating, no hop out).
    #[test]
    fn a_stroke_along_the_surface_is_steady() {
        let (w, d, r, ahead) = sea();
        let f = ahead.as_vec3().to_array();
        let mut p = Player::at(d * (r - FLOAT * HEIGHT), ahead);
        run(&w, &mut p, &Input { fwd: f, ..Default::default() }, 3.0);
        let inp = Input { mv: [1.0, 0.0], sprint: true, pitch: 0.3, fwd: f, ..Default::default() };
        for _ in 0..120 {
            step(&w, &mut p, &inp, DT);
        }
        let mut v0 = None;
        for k in 0..200 {
            let a = p.pos;
            assert!(step(&w, &mut p, &inp, DT), "step {k}: no sprint");
            let v = (p.pos - a).length() / DT;
            let v0 = *v0.get_or_insert(v);
            assert!((v - v0).abs() < 0.05 * v0, "step {k}: {v} m/s against {v0}");
        }
        eprintln!("along the surface at {:.2} m/s, the feet {:.2} m under", v0.unwrap(), r - p.pos.length());
    }

    /// The review of ADR 0118's swimming: a body at rest about the line where floating turns into sinking, under
    /// a water level that wobbles by 3 cm, stays on one side of it.
    #[test]
    fn a_body_at_the_under_line_does_not_flicker() {
        for d0 in [HEIGHT + UNDER - 0.01, HEIGHT + UNDER + 0.01] {
            let mut v = 0.0;
            let first = under(d0, v);
            for k in 0..60 {
                let deep = d0 + if k % 2 == 0 { 0.03 } else { -0.03 };
                let u = under(deep, v);
                assert_eq!(u, first, "at {d0} m, step {k}: under {u}");
                // (under, it sinks; floating, buoyancy lifts it)
                v = if u { -SINK } else { 0.3 };
            }
        }
    }

    /// The review of ADR 0118: a step sprinted only when the sprint took the body further: not into a wall,
    /// not in the sea (its hunger and view go by this).
    #[test]
    fn a_sprint_into_a_wall_is_no_sprint_and_a_stroke_is_one() {
        let mut w = World::new(6144, 256, 1337);
        let d = w.bodies[0].spawn;
        let b0 = w.block_at(w.bodies[0].centre + d * (w.bodies[0].height(d) - 0.5));
        let at = |dx: i32, dz: i32| Block { pos: b0.pos + glam::IVec3::new(dx, 0, dz), ..b0 };
        for dx in -3..=6 {
            for dy in -3..=3 {
                for dz in 0..=4 {
                    w.set_block(Block { pos: b0.pos + glam::IVec3::new(dx, dy, dz), ..b0 }, if dz == 0 || dx == 3 && dz <= 3 { 3 } else { 0 });
                }
            }
        }
        let start = w.block_centre(b0);
        let u = up(&w, start);
        let dir = (w.block_centre(at(1, 0)) - start).reject_from(u).normalize();
        let f = dir.as_vec3().to_array();
        let mut p = Player::at(start + u * 0.6, dir);
        run(&w, &mut p, &Input { fwd: f, ..Default::default() }, 1.0);
        let inp = Input { mv: [1.0, 0.0], fwd: f, sprint: true, ..Default::default() };
        assert!(step(&w, &mut p, &inp, DT), "on the floor it sprints");
        for _ in 0..120 {
            step(&w, &mut p, &inp, DT);
        }
        assert!(!step(&w, &mut p, &inp, DT), "pressed against the wall it does not");
        assert!(!step(&w, &mut p, &Input { sprint: false, ..inp }, DT) && !step(&w, &mut p, &Input { mv: [0.0, 1.0], ..inp }, DT), "nor walking, nor strafing");
        // in the sea
        let w = World::new(1536, 32, 1);
        let b = &w.bodies[0];
        let (i, j) = (0..b.n).step_by(3).flat_map(|i| (0..b.n).step_by(3).map(move |j| (i, j))).find(|&(i, j)| b.height(b.dir(2, i as f64 + 0.5, j as f64 + 0.5)) < b.radius - 8.0).unwrap();
        let d = b.dir(2, i as f64 + 0.5, j as f64 + 0.5);
        let mut p = Player::at(d * (b.radius - crate::world::SEA_DEPTH - 1.0), DVec3::X);
        run(&w, &mut p, &Input::default(), 3.0);
        // floating, a sprint is a stroke (ADR 0118's swimming), which sprints and so drains as one
        assert!(step(&w, &mut p, &Input { mv: [1.0, 0.0], sprint: true, ..Default::default() }, DT), "in the sea a stroke sprints");
        assert!(!step(&w, &mut p, &Input { mv: [1.0, 0.0], ..Default::default() }, DT), "a swim without the sprint does not");
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
        // (every ledge of these steep hills is a jump now, ADR 0033)
        assert!(moved > 10.0, "walked {moved} m in 10 s");
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
