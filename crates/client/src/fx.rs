//! Effects (ADR 0031, docs/design/effects.md): particles and motion, presentation only. An event names a row of
//! `data/effects.toml` and a place; the pool moves the row's particles in the body's frame under the body's own
//! pull, over the floor of the column each is in (looked up again when it moves into another); one instanced draw puts them into the G-buffer
//! (shader.wgsl `vs_fx`), so the sun, its shadows and the bounce light fall on them as on the terrain. They are
//! not in the traced scene: they cast no shadow. Motion: a `Tween` runs 0 to 1 over a curve, a `Track` maps it
//! onto a value.
use engine::blocks::BLOCKS;
use engine::edit::Predictor;
use engine::world::{Block, World};
use glam::{DVec3, IVec3, Vec3};
use serde::Deserialize;
use std::sync::LazyLock;

#[derive(Deserialize, Clone, Copy, PartialEq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Start {
    #[default]
    Point,
    Block,
    Faces,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Ground {
    #[default]
    None,
    Stop,
    Bounce,
    Die,
}

/// The numbers are the shader's (`vs_fx`).
#[derive(Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Look {
    Debris = 0,
    Square = 1,
    Block = 2,
}

fn one() -> f32 {
    1.0
}

/// One effect, as `data/effects.toml` describes it (its header has the fields' meaning).
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub name: String,
    pub count: u32,
    #[serde(default)]
    pub start: Start,
    #[serde(default)]
    pub speed: [f32; 2],
    #[serde(default)]
    pub up: f32,
    #[serde(default = "one")]
    pub gravity: f32,
    #[serde(default)]
    pub drag: f32,
    #[serde(default)]
    pub ground: Ground,
    pub life: [f32; 2],
    pub size: [f32; 2],
    #[serde(default)]
    pub grow: f32,
    #[serde(default)]
    pub shrink: f32,
    pub look: Look,
    #[serde(default)]
    pub colour: Option<[f32; 3]>,
    #[serde(default)]
    pub spin: f32,
    #[serde(default)]
    pub then: Option<String>,
    /// `then` as a row index.
    #[serde(skip)]
    pub then_i: Option<u16>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    effect: Vec<Row>,
}

/// The settings' Particles (few, normal, many): what a row's count is multiplied by, and the pool's cap.
pub const SCALE: [f32; 3] = [0.5, 1.0, 2.0];
pub const CAP: [usize; 3] = [1024, 4096, 16384];

/// Reads and checks the rows: unique names, every `then` a row, lives and sizes positive and in order, shares
/// in 0..1, and no event of any row at any scale over the smallest pool.
pub fn parse(text: &str) -> Result<Vec<Row>, String> {
    let mut rows = toml::from_str::<File>(text).map_err(|e| e.to_string())?.effect;
    let names: Vec<String> = rows.iter().map(|r| r.name.clone()).collect();
    for r in rows.iter_mut() {
        let n = &r.name;
        if names.iter().filter(|m| *m == n).count() > 1 {
            return Err(format!("effect {n} twice"));
        }
        if !(r.life[0] > 0.0 && r.life[0] <= r.life[1]) || !(r.size[0] > 0.0) || r.speed[0] > r.speed[1] || r.speed[0] < 0.0 {
            return Err(format!("effect {n}: life and size must be above 0, every pair from low to high"));
        }
        if !(0.0..=1.0).contains(&r.grow) || !(0.0..=1.0).contains(&r.shrink) || r.grow + r.shrink > 1.0 {
            return Err(format!("effect {n}: grow and shrink are shares of the life, together at most 1"));
        }
        if (r.count as f32 * SCALE[2]) as usize > CAP[0] / 4 {
            return Err(format!("effect {n}: {} particles an event at most", CAP[0] / 4));
        }
        if r.look == Look::Square && r.colour.is_none() {
            return Err(format!("effect {n}: a square needs a colour"));
        }
        r.then_i = match &r.then {
            Some(t) => Some(names.iter().position(|m| m == t).ok_or(format!("effect {n}: no effect {t}"))? as u16),
            None => None,
        };
    }
    Ok(rows)
}

pub static ROWS: LazyLock<Vec<Row>> = LazyLock::new(|| parse(include_str!("../../../data/effects.toml")).expect("data/effects.toml"));

/// A row's index by name; a name not in the file is a bug in the code that plays it.
pub fn id(name: &str) -> u16 {
    ROWS.iter().position(|r| r.name == name).unwrap_or_else(|| panic!("no effect {name} in data/effects.toml")) as u16
}

/// Where an event happened, in its body's frame from the body's centre: a point, the cell's frame (up and the
/// two along the ground, unit) and its half sizes along them, a velocity the particles start with, the floor's
/// radius under it, and the block it shows.
#[derive(Clone, Copy, Debug)]
pub struct At {
    pub body: u8,
    pub p: Vec3,
    pub axes: [Vec3; 3],
    pub half: Vec3,
    pub v: Vec3,
    pub floor: f32,
    /// The column the floor is under; `Column::NONE` keeps the floor wherever the particle goes (the water's
    /// spray, whose floor is the water's).
    pub col: Column,
    pub block: u8,
    /// The cell, for a `block` look: drawn exactly where the terrain draws it.
    pub cell: Option<Block>,
}

/// A column of the grid as a particle tests it: unit directions along its two ground axes, square to its
/// middle, and its half widths as their dot products with a direction at its edge. A direction d is in it
/// while |d.s| and |d.t| are under those.
#[derive(Clone, Copy, Debug)]
pub struct Column {
    pub s: Vec3,
    pub t: Vec3,
    pub w: [f32; 2],
    /// No solid block in the 8 looked at: the floor is only where the lookup stopped, looked up again there.
    pub deep: bool,
}

/// The same column whatever its lookup found under it: a block resting over a void is still that column.
impl PartialEq for Column {
    fn eq(&self, o: &Column) -> bool {
        (self.s, self.t, self.w) == (o.s, o.t, o.w)
    }
}

impl Column {
    pub const NONE: Column = Column { s: Vec3::ZERO, t: Vec3::ZERO, w: [0.0; 2], deep: false };
    /// Whether point p, `r` from the centre along the column, is over it.
    fn holds(&self, p: Vec3, r: f32) -> bool {
        p.dot(self.s).abs() <= self.w[0] * r && p.dot(self.t).abs() <= self.w[1] * r
    }
}

/// The floor of cell b's column: the top of the first solid block from b down, 8 at most (its radius); and
/// the column.
pub fn ground(world: &World, b: Block) -> (f32, Column) {
    let body = &world.bodies[b.body as usize];
    // the column's terrain height once, not once a block (`World::block` would)
    let h = world.column_height(b);
    let solid = (0..=8)
        .map(|k| Block { pos: b.pos - IVec3::new(0, 0, k), ..b })
        .find(|c| BLOCKS.solid(world.edit(*c).unwrap_or_else(|| world.generated_under(*c, h))));
    let floor = solid.map_or(body.layer_r(b.pos.z as f64 - 8.0), |c| body.layer_r(c.pos.z as f64 + 1.0));
    let c = b.pos.as_dvec3() + 0.5;
    let at = |i: f64, j: f64| body.dir(b.face as usize, i, j);
    let up = at(c.x, c.y);
    let flat = |v: DVec3| (v - up * v.dot(up)).normalize();
    let (s, t) = (flat(at(c.x + 0.5, c.y) - at(c.x - 0.5, c.y)), flat(at(c.x, c.y + 0.5) - at(c.x, c.y - 0.5)));
    let w = [at(c.x + 0.5, c.y).dot(s), at(c.x, c.y + 0.5).dot(t)].map(|x| x as f32);
    (floor as f32, Column { s: s.as_vec3(), t: t.as_vec3(), w, deep: solid.is_none() })
}

impl At {
    /// A cell: its middle, its frame and size from the grid, and the floor under it (`ground`: a break's cell
    /// is air by now).
    pub fn cell(world: &World, b: Block, block: u8) -> At {
        let body = &world.bodies[b.body as usize];
        let c = b.pos.as_dvec3() + 0.5;
        let r = body.layer_r(c.z);
        let at = |i: f64, j: f64| body.dir(b.face as usize, i, j);
        let up = at(c.x, c.y);
        let s = at(c.x + 0.5, c.y) - at(c.x - 0.5, c.y);
        let t = at(c.x, c.y + 0.5) - at(c.x, c.y - 0.5);
        let h = (body.layer_r(c.z + 0.5) - body.layer_r(c.z - 0.5)) * 0.5;
        let (floor, col) = ground(world, b);
        At {
            body: b.body,
            p: (up * r).as_vec3(),
            axes: [up.as_vec3(), s.normalize().as_vec3(), t.normalize().as_vec3()],
            half: Vec3::new(h as f32, (s.length() * r * 0.5) as f32, (t.length() * r * 0.5) as f32),
            v: Vec3::ZERO,
            floor,
            col,
            block,
            cell: Some(b),
        }
    }
    /// A point of body `body` (from its centre, in its frame) moving at v, over a floor at radius `floor`.
    pub fn point(body: u8, p: DVec3, v: DVec3, floor: f64) -> At {
        let up = p.normalize().as_vec3();
        let s = up.any_orthonormal_vector();
        At { body, p: p.as_vec3(), axes: [up, s, up.cross(s)], half: Vec3::ZERO, v: v.as_vec3(), floor: floor as f32, col: Column::NONE, block: 0, cell: None }
    }
}

/// One particle: in its body's frame from the centre (f32 is 0.24 mm coarse at 2 km, finer than any of them).
#[derive(Clone, Copy, Debug, PartialEq)]
struct P {
    p: Vec3,
    v: Vec3,
    age: f32,
    life: f32,
    size: f32,
    angle: f32,
    spin: f32,
    floor: f32,
    col: Column,
    row: u16,
    body: u8,
    block: u8,
    rest: bool,
    /// For the shader: the debris' texel seed, or the cell's columns i | j << 15 for a `block` look.
    a: u32,
    /// A square's colour (rgba8), or the cell's layer | face << 16.
    b: u32,
    tag: u32,
}

/// A particle as the shader reads it (`FxP` in shader.wgsl), 32 bytes: where, its edge now, the block | body
/// << 8 | look << 10, its turn, and `a`, `b` as `P` has them.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Debug, PartialEq)]
pub struct FxGpu {
    pub p: [f32; 3],
    pub size: f32,
    pub info: u32,
    pub angle: f32,
    pub a: u32,
    pub b: u32,
}

/// The pool: fixed when made, never grown, the oldest dropped when it is full. `on` false (benchmark and view
/// runs without `fx=1`) plays nothing, so their pictures repeat.
pub struct Fx {
    pub on: bool,
    pool: Vec<P>,
    out: Vec<FxGpu>,
    /// Plays `then` rows ask for, where a particle died; kept to reuse its room.
    later: Vec<(u16, At)>,
    cap: usize,
    scale: f32,
    rng: u32,
    next_tag: u32,
    /// Bursts started, by every event: the tests count them.
    pub played: u64,
    /// Cell events since the last step: past `EVENTS` a frame the rest play nothing (a hammer's hundreds).
    events: u32,
    /// Where a full pool looks next for the particle to replace.
    cursor: usize,
    /// The Particles setting's choice the pool follows, or none (benchmarks, fx_cap=).
    pub setting: Option<usize>,
}

/// The cell events a frame that play an effect.
pub const EVENTS: u32 = 32;

impl Fx {
    pub fn new(on: bool, cap: usize, scale: f32, seed: u32) -> Fx {
        let cap = cap.max(1);
        Fx { on, pool: Vec::with_capacity(cap), out: Vec::with_capacity(cap), later: Vec::with_capacity(256), cap, scale, rng: seed.max(1), next_tag: 1, played: 0, events: 0, cursor: 0, setting: None }
    }

    /// A new cap and scale (the Particles setting changed): the pool shrinks at once or grows its room once.
    pub fn set(&mut self, cap: usize, scale: f32) {
        self.cap = cap.max(1);
        self.scale = scale;
        self.pool.truncate(self.cap);
        self.pool.reserve_exact(self.cap - self.pool.len());
        self.out.reserve_exact(self.cap.saturating_sub(self.out.len()));
    }

    /// Follows the Particles setting's choice k, when `setting` says this run does (run=play without fx_cap=).
    pub fn follow(&mut self, k: usize) {
        if self.setting.is_some_and(|s| s != k) {
            self.set(CAP[k], SCALE[k]);
            self.setting = Some(k);
        }
    }

    pub fn live(&self) -> usize {
        self.pool.len()
    }

    fn rnd(&mut self) -> f32 {
        let mut r = self.rng;
        r ^= r << 13;
        r ^= r >> 17;
        r ^= r << 5;
        self.rng = r;
        (r >> 8) as f32 / (1u32 << 24) as f32
    }

    /// A random unit vector.
    fn dir(&mut self) -> Vec3 {
        loop {
            let v = Vec3::new(self.rnd(), self.rnd(), self.rnd()) * 2.0 - 1.0;
            let l = v.length_squared();
            if l > 1e-4 && l <= 1.0 {
                return v / l.sqrt();
            }
        }
    }

    /// Plays row `row` at `at`; returns a tag that `end` takes to stop its particles early.
    pub fn play(&mut self, row: u16, at: &At) -> u32 {
        if !self.on {
            return 0;
        }
        let r = &ROWS[row as usize];
        // a row of one stays one: a second grow or drop would lie on the first
        let n = if r.count == 1 { 1 } else { ((r.count as f32 * self.scale).round() as usize).max(1) };
        let tag = self.next_tag;
        self.next_tag = self.next_tag.wrapping_add(1).max(1);
        self.played += 1;
        let [up, s, t] = at.axes;
        for _ in 0..n {
            let (u, v, w) = (self.rnd() * 2.0 - 1.0, self.rnd() * 2.0 - 1.0, self.rnd() * 2.0 - 1.0);
            let off = match r.start {
                Start::Point => Vec3::ZERO,
                Start::Block => up * u * at.half.x + s * v * at.half.y + t * w * at.half.z,
                Start::Faces => {
                    // on one of the six faces: one coordinate pushed to its side
                    let k = (self.rnd() * 6.0) as u32;
                    let side = if k & 1 == 0 { 1.0 } else { -1.0 };
                    let c = match k / 2 {
                        0 => Vec3::new(side, v, w),
                        1 => Vec3::new(u, side, w),
                        _ => Vec3::new(u, v, side),
                    };
                    up * c.x * at.half.x + s * c.y * at.half.y + t * c.z * at.half.z
                }
            };
            let speed = r.speed[0] + (r.speed[1] - r.speed[0]) * self.rnd();
            let d = self.dir();
            let life = r.life[0] + (r.life[1] - r.life[0]) * self.rnd();
            let size = if r.grow > 0.0 { r.size[0] } else { r.size[0] + (r.size[1] - r.size[0]) * self.rnd() };
            let spin = r.spin * (self.rnd() * 2.0 - 1.0);
            let seed = (self.rnd() * 16777216.0) as u32;
            let (a, b) = match (r.look, at.cell) {
                (Look::Block, Some(c)) => (c.pos.x as u32 & 0x7fff | (c.pos.y as u32 & 0x7fff) << 15, c.pos.z as u32 & 0xffff | (c.face as u32) << 16),
                (Look::Square, _) => {
                    let c = r.colour.unwrap_or([1.0; 3]).map(|x| (x.clamp(0.0, 1.0) * 255.0).round() as u32);
                    (seed, c[0] | c[1] << 8 | c[2] << 16 | 255 << 24)
                }
                _ => (seed, 0),
            };
            let mut x = at.p + off;
            // no part of it starts under the floor it lands on
            let half = if r.look == Look::Block { 0.0 } else { size * 0.5 };
            if r.ground != Ground::None && x.length() < at.floor + half {
                x = x.normalize() * (at.floor + half);
            }
            let p = P {
                p: x,
                v: at.v + d * speed + up * r.up,
                age: 0.0,
                life,
                size,
                angle: 0.0,
                spin,
                floor: at.floor,
                col: at.col,
                row,
                body: at.body,
                block: at.block,
                rest: false,
                a,
                b,
                tag,
            };
            if self.pool.len() < self.cap {
                self.pool.push(p);
            } else {
                // full: of the next 32 from the cursor, the one nearest its end goes; the cursor moves on, so a
                // burst into a full pool costs 32 looks a particle, not the pool's size (shortcut: not the
                // farthest from the camera; fine while bursts are local, add the distance when weather fills it)
                let n = self.pool.len();
                let look = n.min(32);
                let end = |q: &P| q.age / q.life;
                let k = (0..look).map(|j| (self.cursor + j) % n).max_by(|&x, &y| end(&self.pool[x]).total_cmp(&end(&self.pool[y]))).unwrap();
                self.cursor = (self.cursor + look) % n;
                self.pool[k] = p;
            }
        }
        tag
    }

    /// A cell gone under particles that lie on it: those in its column over its top fall on to the floor
    /// under it (`at`'s).
    pub fn fall_through(&mut self, at: &At) {
        let top = at.p.length() + at.half.x;
        for q in self.pool.iter_mut().filter(|q| q.body == at.body && q.floor > at.floor + 1e-3 && q.floor <= top + 1e-3) {
            if q.col == at.col {
                (q.floor, q.col, q.rest) = (at.floor, at.col, false);
            }
        }
    }

    /// Ends the particles a play's tag names (the grown block once its mesh shows).
    pub fn end(&mut self, tag: u32) {
        self.pool.retain(|p| p.tag != tag);
    }

    /// Moves every particle on by dt: the body's pull towards its centre, drag, the floor of its column; the
    /// dead go, the living are packed for the GPU (`drawn`) in the same pass.
    pub fn step(&mut self, world: &World, dt: f32) {
        let g0 = engine::phys::G as f32;
        let rows = &*ROWS;
        self.events = 0;
        self.out.clear();
        let mut i = 0;
        while i < self.pool.len() {
            let q = &mut self.pool[i];
            let r = &rows[q.row as usize];
            q.age += dt;
            if !q.rest && q.age < q.life {
                let up = q.p.normalize();
                q.v += (up * (-g0 * r.gravity) - q.v * r.drag) * dt;
                let was = q.p;
                q.p += q.v * dt;
                q.angle += q.spin * dt;
                // out of its column: the floor of the one it is over now (a few block lookups, once a crossing),
                // or, into a block's side, back off it
                if q.col.w[0] > 0.0 && !q.col.holds(q.p, q.p.dot(up)) {
                    let b = world.block_at(world.bodies[q.body as usize].centre + q.p.as_dvec3());
                    let b = Block { body: q.body, ..b };
                    let (floor, col) = ground(world, b);
                    // the floor at the cell's own top: the cell is solid
                    if floor >= world.bodies[q.body as usize].layer_r(b.pos.z as f64 + 1.0) as f32 {
                        q.p = was;
                        q.v = up * q.v.dot(up) - (q.v - up * q.v.dot(up)) * 0.3;
                    } else {
                        (q.floor, q.col) = (floor, col);
                    }
                }
                let half = if r.look == Look::Block { 0.0 } else { q.size * 0.5 };
                // its height along the old up: a frame's step sideways changes the radius by under a micrometre
                let mut over = q.p.dot(up) - half - q.floor;
                // down to where a deep column's lookup stopped: look on from there
                if over < 0.0 && q.col.deep {
                    let b = world.block_at(world.bodies[q.body as usize].centre + q.p.as_dvec3());
                    (q.floor, q.col) = ground(world, Block { body: q.body, ..b });
                    over = q.p.dot(up) - half - q.floor;
                }
                let down = q.v.dot(up);
                if r.ground != Ground::None && over < 0.0 && down < 0.0 {
                    q.p -= up * over;
                    match r.ground {
                        Ground::Stop => (q.v, q.rest) = (Vec3::ZERO, true),
                        Ground::Bounce => {
                            // a third of the fall back up, half of the slide kept; slow, it lies down
                            q.v = (q.v - up * down) * 0.5 - up * down * 0.3;
                            q.spin *= 0.5;
                            if q.v.length() < 0.4 {
                                (q.v, q.rest) = (Vec3::ZERO, true);
                            }
                        }
                        _ => {
                            q.age = q.life;
                            if let Some(t) = r.then_i {
                                let at = At::point(q.body, q.p.as_dvec3(), DVec3::ZERO, q.floor as f64);
                                if self.later.len() < self.later.capacity() {
                                    self.later.push((t, at));
                                }
                            }
                        }
                    }
                }
            }
            if q.age >= q.life {
                self.pool.swap_remove(i);
                continue;
            }
            let x = q.age / q.life;
            let mut size = q.size;
            if r.grow > 0.0 {
                size = r.size[0] + (r.size[1] - r.size[0]) * (x / r.grow).min(1.0);
            }
            if r.shrink > 0.0 && x > 1.0 - r.shrink {
                size *= (1.0 - x) / r.shrink;
            }
            self.out.push(FxGpu { p: q.p.to_array(), size, info: q.block as u32 | (q.body as u32) << 8 | (r.look as u32) << 10, angle: q.angle, a: q.a, b: q.b });
            i += 1;
        }
        while let Some((t, at)) = self.later.pop() {
            self.play(t, &at);
        }
    }

    #[cfg(test)]
    fn resting(&self, world: &World) -> Vec<Block> {
        self.pool.iter().filter(|p| p.rest).map(|p| world.block_at(world.bodies[p.body as usize].centre + p.p.as_dvec3())).collect()
    }

    /// The particles for the GPU as the last step left them, in a buffer kept from frame to frame.
    pub fn drawn(&self) -> &[FxGpu] {
        &self.out
    }
}

/// A cell the client's world changed, with what it was: a solid block gone plays the break, one come the place.
pub fn changed(fx: &mut Fx, world: &World, b: Block, was: u8, now: u8) {
    if !fx.on || was == now {
        return;
    }
    let play = fx.events < EVENTS;
    fx.events += 1;
    if BLOCKS.solid(was) && !BLOCKS.solid(now) {
        // past the budget it plays nothing but still drops what lies on the cell (one floor lookup, and only
        // while anything is alive)
        if play || !fx.pool.is_empty() {
            let at = At::cell(world, b, was);
            fx.fall_through(&at);
            if play {
                fx.play(id("break"), &at);
            }
        }
    } else if play && BLOCKS.solid(now) && !BLOCKS.solid(was) {
        fx.play(id("place"), &At::cell(world, b, now));
    }
}

/// A cell from the server's `Edits` through the prediction (`Predictor::server`): the effect plays when the
/// world shows the change, so a cell this client already changed itself plays nothing twice. Whether it
/// shows, and what the cell was when that was read: with effects on, or `hear` (its sound, frame.rs); a
/// block read costs the terrain's noise, so a batch of hundreds reads only what it needs (ADR 0032).
pub fn server_edit(fx: &mut Fx, pred: &mut Predictor, world: &mut World, b: Block, m: u8, hear: bool) -> (bool, Option<u8>) {
    let was = (fx.on || hear).then(|| world.block(b));
    let shows = pred.server(world, b, m);
    if let (true, Some(was)) = (shows, was) {
        changed(fx, world, b, was, m);
    }
    (shows, was)
}

// ---- motion ----

// the design's curves (docs/design/effects.md, Motion); the hand uses Pulse, content the others
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Curve {
    Linear,
    In,
    Out,
    InOut,
    /// Up and back: 0, 1 half way, 0.
    Pulse,
    /// Past 1 and settling on it.
    Spring,
    /// Shaking about 0, dying out.
    Shake,
}

/// A start time, a length and a curve; `repeat` starts it again at its end (a walk's bob).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tween {
    pub start: f64,
    pub len: f64,
    pub curve: Curve,
    pub repeat: bool,
}

impl Tween {
    pub fn new(start: f64, len: f64, curve: Curve) -> Tween {
        Tween { start, len, curve, repeat: false }
    }
    /// Where it is at `now` (in the start's units: seconds, or metres walked), 0 before and after for the
    /// curves that end at 0, else held at its end.
    pub fn at(&self, now: f64) -> f32 {
        let mut x = ((now - self.start) / self.len.max(1e-9)) as f32;
        if self.repeat {
            x = x.rem_euclid(1.0);
        }
        let x = x.clamp(0.0, 1.0);
        use std::f32::consts::PI;
        match self.curve {
            Curve::Linear => x,
            Curve::In => x * x,
            Curve::Out => 1.0 - (1.0 - x) * (1.0 - x),
            Curve::InOut => x * x * (3.0 - 2.0 * x),
            Curve::Pulse => (x * PI).sin(),
            Curve::Spring => 1.0 - (-6.0 * x).exp() * (x * 3.0 * PI).cos() * (1.0 - x),
            Curve::Shake => (x * 6.0 * PI).sin() * (1.0 - x),
        }
    }
    #[allow(dead_code)]
    pub fn done(&self, now: f64) -> bool {
        !self.repeat && now >= self.start + self.len
    }
}

/// A value a track moves.
pub trait Mix: Copy {
    fn mix(self, to: Self, x: f32) -> Self;
}
impl Mix for f32 {
    fn mix(self, to: f32, x: f32) -> f32 {
        self + (to - self) * x
    }
}
impl<const N: usize> Mix for [f32; N] {
    fn mix(self, to: Self, x: f32) -> Self {
        std::array::from_fn(|i| self[i].mix(to[i], x))
    }
}
impl Mix for Vec3 {
    fn mix(self, to: Vec3, x: f32) -> Vec3 {
        self.lerp(to, x)
    }
}

/// A tween mapped onto a value: a move, a turn, a scale, a fade, a colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Track<T: Mix> {
    pub tween: Tween,
    pub from: T,
    pub to: T,
}

impl<T: Mix> Track<T> {
    pub fn at(&self, now: f64) -> T {
        self.from.mix(self.to, self.tween.at(now))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::blocks::AIR;

    fn world() -> World {
        World::new(768, 256, 1)
    }

    /// A solid cell on the planet's surface at the spawn and the radius of its top.
    fn surface(w: &World) -> Block {
        let b = &w.bodies[0];
        let mut c = w.block_at(b.spawn * (b.height(b.spawn) + 1.0));
        while !BLOCKS.solid(w.block(c)) {
            c.pos.z -= 1;
        }
        while BLOCKS.solid(w.block(Block { pos: c.pos + IVec3::Z, ..c })) {
            c.pos.z += 1;
        }
        assert!(BLOCKS.solid(w.block(c)));
        c
    }

    #[test]
    fn the_rows_read_and_check() {
        assert!(ROWS.len() >= 5);
        for name in ["break", "place", "grow", "spray", "splash"] {
            id(name);
        }
        assert_eq!(ROWS[id("spray") as usize].then_i, Some(id("splash")));
        let bad = |t: &str| parse(t).is_err();
        assert!(bad("[[effect]]\nname='a'\ncount=1\nlife=[1,1]\nsize=[0.1,0.1]\nlook='debris'\nthen='b'"));
        assert!(bad("[[effect]]\nname='a'\ncount=1\nlife=[0,1]\nsize=[0.1,0.1]\nlook='debris'"));
        assert!(bad("[[effect]]\nname='a'\ncount=1000\nlife=[1,1]\nsize=[0.1,0.1]\nlook='debris'"));
        assert!(bad("[[effect]]\nname='a'\ncount=1\nlife=[1,1]\nsize=[0.1,0.1]\nlook='square'"));
        assert!(bad("[[effect]]\nname='a'\ncount=1\nlife=[1,1]\nsize=[0.1,0.1]\nlook='debris'\nwind=2"));
        assert!(!bad("[[effect]]\nname='a'\ncount=1\nlife=[1,1]\nsize=[0.1,0.1]\nlook='debris'"));
    }

    #[test]
    fn a_burst_is_the_same_for_the_same_seed_lands_and_is_gone() {
        let mut w = world();
        let c = surface(&w);
        w.set_block(c, AIR);
        let at = At::cell(&w, c, BLOCKS.id("stone"));
        let run = |seed| {
            let mut fx = Fx::new(true, 4096, 1.0, seed);
            fx.play(id("break"), &at);
            fx.step(&w, 0.0);
            let first: Vec<FxGpu> = fx.drawn().to_vec();
            let mut lowest = f32::MAX;
            let mut rest = 0;
            for k in 0..200 {
                fx.step(&w, 1.0 / 60.0);
                if k == 90 {
                    rest = fx.pool.iter().filter(|p| p.rest).count();
                }
                lowest = fx.pool.iter().map(|p| p.p.length() - p.size * 0.5).fold(lowest, f32::min);
            }
            (first, rest, lowest, fx.live())
        };
        let (a, rest, lowest, live) = run(7);
        assert_eq!(a.len(), 12);
        assert_eq!(a, run(7).0, "the same seed, the same burst");
        assert_ne!(a, run(8).0);
        // after 1.5 s the debris lies on the floor (the top of the block under the dug one, or the ground it
        // was thrown on to), never under it
        assert!(rest >= 10, "{rest} of 12 at rest");
        assert!(lowest >= at.floor - 1e-3, "{lowest} under the floor {}", at.floor);
        // after its longest life nothing is left: no leak
        assert_eq!(live, 0);
        // whatever the seed, nothing comes to rest inside a block (thrown on to the ground around the hole,
        // or against its walls)
        for seed in 1..30 {
            let mut fx = Fx::new(true, 4096, 1.0, seed);
            fx.play(id("break"), &at);
            for _ in 0..80 {
                fx.step(&w, 1.0 / 60.0);
            }
            for b in fx.resting(&w) {
                assert!(!BLOCKS.solid(w.block(b)), "seed {seed}: at rest inside a block at {b:?}");
            }
        }
    }

    #[test]
    fn a_spray_drop_dies_on_the_floor_and_splashes_there() {
        let w = world();
        let c = surface(&w);
        let r = w.bodies[0].layer_r(c.pos.z as f64 + 1.0);
        let up = w.block_centre(c).normalize();
        let mut fx = Fx::new(true, 4096, 1.0, 3);
        fx.play(id("spray"), &At::point(0, up * (r + 2.0), DVec3::ZERO, r));
        // 2 m fall: 0.64 s
        for _ in 0..42 {
            fx.step(&w, 1.0 / 60.0);
        }
        assert_eq!(fx.live(), 3, "three splash drops");
        assert_eq!(fx.played, 2);
    }

    #[test]
    fn a_full_pool_drops_the_oldest_and_never_grows() {
        let w = world();
        let mut fx = Fx::new(true, 16, 1.0, 1);
        let at = At::point(0, DVec3::new(0.0, 2000.0, 0.0), DVec3::ZERO, 0.0);
        for _ in 0..5 {
            fx.play(id("break"), &at);
            fx.step(&w, 0.01);
        }
        assert_eq!(fx.live(), 16);
        assert_eq!(fx.pool.capacity(), 16);
        // the newest burst is all there
        assert_eq!(fx.pool.iter().filter(|p| p.age < 0.015).count(), 12);
        // the Particles setting changed in the menu: the pool follows at once
        fx.set(8, 1.0);
        assert_eq!(fx.live(), 8);
        fx.play(id("break"), &at);
        assert_eq!(fx.live(), 8);
        // a row of one stays one at "many" (the grow, a drop: two would lie on each other)
        let mut many = Fx::new(true, 64, 2.0, 1);
        many.play(id("grow"), &at);
        assert_eq!(many.live(), 1);
        // fx_cap=0 is a pool of one, not a panic
        let mut none = Fx::new(true, 0, 1.0, 1);
        none.play(id("break"), &at);
        none.play(id("break"), &at);
        assert_eq!(none.live(), 1);
    }

    #[test]
    fn debris_falls_on_when_a_block_over_a_void_breaks_under_it() {
        let mut w = world();
        let mut fx = Fx::new(true, 4096, 1.0, 5);
        let c = surface(&w);
        let (top, under) = (c, Block { pos: c.pos - IVec3::Z, ..c });
        let was = w.block(top);
        w.set_block(top, AIR);
        changed(&mut fx, &w, top, was, AIR);
        for _ in 0..60 {
            fx.step(&w, 1.0 / 60.0);
        }
        // the ground under the block it lies on is gone 10 deep (a bridge, a shaft's lid), then the block goes
        for k in 1..=10 {
            w.set_block(Block { pos: under.pos - IVec3::new(0, 0, k), ..under }, AIR);
        }
        let was = w.block(under);
        w.set_block(under, AIR);
        changed(&mut fx, &w, under, was, AIR);
        let col = At::cell(&w, under, 0).col;
        let over: Vec<&P> = fx.pool.iter().filter(|p| p.tag == 1 && col.holds(p.p, p.p.length())).collect();
        assert!(!over.is_empty());
        assert!(over.iter().all(|p| !p.rest), "debris over the broken block still lies on it");
    }

    #[test]
    fn debris_thrown_into_a_deep_pit_lands_on_its_bottom() {
        let mut w = world();
        let c = surface(&w);
        // a pit 14 blocks (7 m) deep beside the dug cell: deeper than a column's lookup of 8
        let pit = |k: i32| Block { pos: c.pos + IVec3::new(1, 0, -k), ..c };
        for k in 0..14 {
            w.set_block(pit(k), AIR);
        }
        let bottom = w.bodies[0].layer_r((c.pos.z - 13) as f64) as f32;
        let mut fell = 0;
        for seed in 1..40 {
            let mut fx = Fx::new(true, 4096, 1.0, seed);
            let was = w.block(c);
            w.set_block(c, AIR);
            changed(&mut fx, &w, c, was, AIR);
            for _ in 0..120 {
                fx.step(&w, 1.0 / 60.0);
                for p in &fx.pool {
                    let r = p.p.length();
                    if r < bottom + 0.5 {
                        fell += 1;
                    }
                    if p.rest {
                        let under = w.block_at(w.bodies[0].centre + (p.p * ((r - p.size * 0.5 - 0.05) / r)).as_dvec3());
                        assert!(BLOCKS.solid(w.block(Block { body: 0, ..under })), "seed {seed}: debris at rest in the air, {r} against the pit's bottom {bottom}");
                    }
                }
            }
            w.set_block(c, was);
        }
        assert!(fell > 0, "no debris reached the pit's bottom; the test does not test it");
    }

    #[test]
    fn exactly_one_break_per_solid_to_air_delta_also_another_players() {
        let mut w = world();
        let mut pred = Predictor::default();
        let mut fx = Fx::new(true, 4096, 1.0, 1);
        let mine = surface(&w);
        let theirs = Block { pos: mine.pos + IVec3::new(3, 0, 0), ..mine };
        let theirs = Block { pos: IVec3::new(theirs.pos.x, theirs.pos.y, surface_z(&w, theirs)), ..theirs };
        // this client digs: the prediction changes the cell, the effect plays once
        let shape = engine::edit::Shape::one(mine);
        let was = w.block(mine);
        let (seq, cells) = pred.edit(&mut w, &shape, AIR, |_, _| false);
        assert_eq!(cells, vec![mine]);
        changed(&mut fx, &w, mine, was, AIR);
        assert_eq!(fx.played, 1);
        // the server's delta for it, and its Done: nothing more
        assert!(!server_edit(&mut fx, &mut pred, &mut w, mine, AIR, false).0);
        pred.done(&mut w, seq);
        assert_eq!(fx.played, 1);
        // another player's dig arrives as a delta: one break
        let their_was = w.block(theirs);
        assert_eq!(server_edit(&mut fx, &mut pred, &mut w, theirs, AIR, false), (true, Some(their_was)));
        assert_eq!(fx.played, 2);
        assert_eq!(fx.live(), 24);
        // the same delta again changes nothing and plays nothing
        server_edit(&mut fx, &mut pred, &mut w, theirs, AIR, false);
        assert_eq!(fx.played, 2);
        // effects off (benchmarks, views): nothing
        let mut off = Fx::new(false, 4096, 1.0, 1);
        let deeper = Block { pos: theirs.pos - IVec3::Z, ..theirs };
        assert_eq!(server_edit(&mut off, &mut pred, &mut w, deeper, AIR, false), (true, None));
        assert_eq!((off.played, off.live()), (0, 0));
        // heard with effects off: the old block is read for its sound, still no effect
        let deepest = Block { pos: deeper.pos - IVec3::Z, ..deeper };
        let was = w.block(deepest);
        assert_eq!(server_edit(&mut off, &mut pred, &mut w, deepest, AIR, true), (true, Some(was)));
        assert_eq!(off.played, 0);
    }

    #[test]
    fn debris_falls_on_when_the_block_under_it_breaks_too() {
        let mut w = world();
        let mut fx = Fx::new(true, 4096, 1.0, 5);
        let c = surface(&w);
        let (top, under) = (c, Block { pos: c.pos - IVec3::Z, ..c });
        // the top breaks, its debris lies on the one under it; then that one breaks
        let was = w.block(top);
        w.set_block(top, AIR);
        changed(&mut fx, &w, top, was, AIR);
        for _ in 0..60 {
            fx.step(&w, 1.0 / 60.0);
        }
        let r = w.bodies[0].layer_r(under.pos.z as f64) as f32;
        assert!(fx.pool.iter().any(|p| p.rest && p.p.length() > r + 0.4), "some lie on the block under it");
        // a frame whose event budget is spent (a hammer's dig) still lets it fall
        for k in 0..EVENTS as i32 {
            changed(&mut fx, &w, Block { pos: c.pos + IVec3::new(40, k, 20), ..c }, was, AIR);
        }
        let was = w.block(under);
        w.set_block(under, AIR);
        changed(&mut fx, &w, under, was, AIR);
        for _ in 0..30 {
            fx.step(&w, 1.0 / 60.0);
        }
        // the first burst's debris over the hole falls on to the new floor (a block of 1 m takes it longer than 0.5 s to
        // settle); nothing lies inside a block
        let col = At::cell(&w, under, 0).col;
        let over: Vec<&P> = fx.pool.iter().filter(|p| p.tag == 1 && col.holds(p.p, p.p.length())).collect();
        assert!(!over.is_empty());
        for p in over {
            assert!((p.floor - r).abs() < 1e-3 && p.p.length() - p.size * 0.5 < r + 0.6, "{} over {r}, floor {}", p.p.length(), p.floor);
        }
        for b in fx.resting(&w) {
            assert!(!BLOCKS.solid(w.block(b)), "debris at rest inside a block at {b:?}");
        }
    }

    fn surface_z(w: &World, b: Block) -> i32 {
        let mut c = b;
        while BLOCKS.solid(w.block(Block { pos: c.pos + IVec3::Z, ..c })) {
            c.pos.z += 1;
        }
        while !BLOCKS.solid(w.block(c)) {
            c.pos.z -= 1;
        }
        c.pos.z
    }

    /// The frame thread's cost of `step` with a full pool of breaks, a frame at 120 fps: the budget is 0.05 ms
    /// for 4,096 (docs/design/effects.md). `cargo test --release -p client step_cost -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn step_cost() {
        let w = world();
        let c = surface(&w);
        let at = At::cell(&w, Block { pos: c.pos + IVec3::Z, ..c }, BLOCKS.id("grass"));
        for n in [4096, 16384] {
            let mut fx = Fx::new(true, n, 1.0, 1);
            let mut times = Vec::new();
            for f in 0..2000 {
                while fx.live() + 12 <= n {
                    fx.play(id("break"), &at);
                }
                let t = std::time::Instant::now();
                fx.step(&w, 1.0 / 120.0);
                if f >= 200 {
                    times.push(t.elapsed().as_secs_f64() * 1000.0);
                }
            }
            times.sort_by(f64::total_cmp);
            let rest = fx.pool.iter().filter(|p| p.rest).count();
            println!("step {n}: p50 {:.4} ms, p99 {:.4} ms ({rest} at rest)", times[times.len() / 2], times[times.len() * 99 / 100]);
        }
    }

    #[test]
    fn tweens_run_their_curves_and_tracks_map_them() {
        let t = Tween::new(1.0, 0.5, Curve::Pulse);
        assert!(t.at(0.0).abs() < 1e-6 && t.at(2.0).abs() < 1e-6);
        assert!((t.at(1.25) - 1.0).abs() < 1e-6);
        assert!(t.done(1.5) && !t.done(1.4));
        let lin = Tween::new(0.0, 2.0, Curve::Linear);
        assert_eq!((lin.at(1.0), lin.at(5.0)), (0.5, 1.0));
        assert!(Tween::new(0.0, 1.0, Curve::Spring).at(1.0) > 0.99);
        assert!(Tween::new(0.0, 1.0, Curve::Shake).at(1.0).abs() < 1e-5);
        let bob = Tween { repeat: true, ..Tween::new(0.0, 0.8, Curve::Pulse) };
        assert!((bob.at(0.4) - bob.at(1.2)).abs() < 1e-5 && !bob.done(100.0));
        let tr = Track { tween: lin, from: [0.0, 10.0], to: [4.0, 20.0] };
        assert_eq!(tr.at(1.0), [2.0, 15.0]);
    }
}
