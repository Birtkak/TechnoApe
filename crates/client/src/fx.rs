//! Effects (ADR 0031, docs/design/effects.md): particles and motion, presentation only. An event names a row of
//! `data/effects.toml` and a place; the pool moves the row's particles in the body's frame under the body's own
//! pull, over the floor of the column each is in (looked up again when it moves into another); one instanced draw puts them into the G-buffer
//! (shader.wgsl `vs_fx`), so the sun, its shadows and the bounce light fall on them as on the terrain. They are
//! not in the traced scene: they cast no shadow. Motion: a `Tween` runs 0 to 1 over a curve, a `Track` maps it
//! onto a value.
use engine::blocks::BLOCKS;
use engine::edit::Predictor;
use engine::palette::Palette;
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
    /// On a circle of `radius` about the point, flat on the ground (ADR 0050).
    Ring,
    /// At the point, flying out within `angle` degrees of `dir`.
    Cone,
    /// Anywhere in a box of `box` metres about the point.
    Box,
}

/// How a square, a texture or a model lies (ADR 0050).
#[derive(Deserialize, Clone, Copy, PartialEq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Face {
    #[default]
    Camera,
    /// Along its flight, stretched by its speed (rain, sparks).
    Flight,
    /// Turning about an axis of its own (a leaf, a chip).
    Tumble,
}

/// Motion through a moving noise field: `strength` m/s² at most, `scale` the field's grain in metres, `speed`
/// how fast it moves (field units a second).
#[derive(Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct Noise {
    pub strength: f32,
    #[serde(default = "one")]
    pub scale: f32,
    #[serde(default)]
    pub speed: f32,
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
    /// Cracks over a cell being dug (ADR 0034): drawn by `Fx::crack`, not by a row.
    Crack = 3,
    /// A voxel model, `data/models/<model>.toml`, drawn by models.rs (ADR 0050).
    Model = 4,
    /// A square showing the top face of block `texture` (its PNG in data/textures/, else its pattern): drawn
    /// as a square with the texture bit.
    Texture = 5,
}

/// Cracked cells drawn a frame at most: a hammer's dig shows its first ones.
pub const CRACKS: usize = 64;

fn one() -> f32 {
    1.0
}

fn upward() -> [f32; 3] {
    [0.0, 1.0, 0.0]
}

/// One effect, as `data/effects.toml` describes it (its header has the fields' meaning).
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub name: String,
    #[serde(default)]
    pub count: u32,
    /// Particles a second while it plays (a lasting effect, `Fx::start`).
    #[serde(default)]
    pub rate: f32,
    #[serde(default)]
    pub radius: f32,
    #[serde(default)]
    pub angle: f32,
    /// The cone's axis in the event's frame: x, y up, z.
    #[serde(default = "upward")]
    pub dir: [f32; 3],
    #[serde(default, rename = "box")]
    pub size_box: Option<[f32; 3]>,
    /// Colour over life: (share of the life, palette name), in order.
    #[serde(default)]
    pub gradient: Vec<(f32, String)>,
    /// Size and brightness over life: (share of the life, times), in order.
    #[serde(default)]
    pub size_curve: Vec<[f32; 2]>,
    #[serde(default)]
    pub bright_curve: Vec<[f32; 2]>,
    #[serde(default)]
    pub noise: Option<Noise>,
    /// m/s² along the up (a plume's lift) and sideways in a direction of its own (its widening).
    #[serde(default)]
    pub rise: f32,
    #[serde(default)]
    pub spread: f32,
    /// The light it gives: emission times its colour, and a lamp of that strength where a lasting one plays.
    #[serde(default)]
    pub glow: f32,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub texture: Option<String>,
    #[serde(default)]
    pub face: Face,
    /// `gradient` from the palette.
    #[serde(skip)]
    pub grad: Vec<(f32, [f32; 3])>,
    /// `texture` as a block id.
    #[serde(skip)]
    pub tex: u16,
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

/// Several rows played and stopped as one (fire = flames + embers + smoke).
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub name: String,
    pub rows: Vec<String>,
    #[serde(skip)]
    pub ids: Vec<u16>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    effect: Vec<Row>,
    #[serde(default)]
    group: Vec<Group>,
}

/// The rows and groups a pool plays.
#[derive(Clone, Debug, Default)]
pub struct Set {
    pub rows: Vec<Row>,
    pub groups: Vec<Group>,
}

impl Set {
    /// The rows a name plays: a group's, or the row of that name.
    pub fn rows_of(&self, name: &str) -> Option<Vec<u16>> {
        match self.groups.iter().find(|g| g.name == name) {
            Some(g) => Some(g.ids.clone()),
            None => self.rows.iter().position(|r| r.name == name).map(|i| vec![i as u16]),
        }
    }
    /// Every row's name, then every group's.
    pub fn names(&self) -> Vec<&str> {
        self.rows.iter().map(|r| r.name.as_str()).chain(self.groups.iter().map(|g| g.name.as_str())).collect()
    }
}

/// The settings' Particles (few, normal, many): what a row's count is multiplied by, and the pool's cap.
pub const SCALE: [f32; 3] = [0.5, 1.0, 2.0];
pub const CAP: [usize; 3] = [1024, 4096, 16384];

/// The game's palette as built in: gradients name its colours.
pub static PALETTE: LazyLock<Palette> = LazyLock::new(|| Palette::parse(include_str!("../../../data/palette.toml")).expect("data/palette.toml"));

/// Reads and checks the rows: unique names, every `then` a row, lives and sizes positive and in order, shares
/// in 0..1, and no event of any row at any scale over the smallest pool.
pub fn parse(text: &str) -> Result<Vec<Row>, String> {
    parse_set(text, &PALETTE).map(|s| s.rows)
}

/// `parse` with the groups, against a palette.
pub fn parse_set(text: &str, palette: &Palette) -> Result<Set, String> {
    let File { effect: mut rows, group: mut groups } = toml::from_str::<File>(text).map_err(|e| e.to_string())?;
    let names: Vec<String> = rows.iter().map(|r| r.name.clone()).collect();
    for r in rows.iter_mut() {
        if names.iter().filter(|m| **m == r.name).count() > 1 {
            return Err(format!("effect {} twice", r.name));
        }
        check(r, &names, palette)?;
    }
    for g in groups.iter_mut() {
        let n = &g.name;
        if names.contains(n) {
            return Err(format!("group {n}: the name of a row or another group"));
        }
        if g.rows.is_empty() {
            return Err(format!("group {n}: no rows"));
        }
        g.ids = g.rows.iter().map(|m| names.iter().position(|k| k == m).map(|i| i as u16).ok_or(format!("group {n}: no effect {m}"))).collect::<Result<_, _>>()?;
    }
    let gn: Vec<&String> = groups.iter().map(|g| &g.name).collect();
    if let Some(g) = gn.iter().enumerate().find(|(i, g)| gn[..*i].contains(g)) {
        return Err(format!("group {} twice", g.1));
    }
    Ok(Set { rows, groups })
}

/// One row's checks; every refusal names it and why. `names`: the rows its `then` may name.
fn check(r: &mut Row, names: &[String], palette: &Palette) -> Result<(), String> {
    let n = r.name.clone();
    let no = |why: &str| Err(format!("effect {n}: {why}"));
    // every number finite, named by its field
    let z = r.noise.map_or([0.0; 3], |z| [z.strength, z.scale, z.speed]);
    let curves = |c: &[[f32; 2]]| c.iter().flatten().copied().collect::<Vec<f32>>();
    let nums: [(&str, Vec<f32>); 22] = [
        ("rate", vec![r.rate]),
        ("radius", vec![r.radius]),
        ("angle", vec![r.angle]),
        ("dir", r.dir.to_vec()),
        ("box", r.size_box.map_or(vec![], |b| b.to_vec())),
        ("gradient", r.gradient.iter().map(|s| s.0).collect()),
        ("size_curve", curves(&r.size_curve)),
        ("bright_curve", curves(&r.bright_curve)),
        ("noise", z.to_vec()),
        ("rise", vec![r.rise]),
        ("spread", vec![r.spread]),
        ("glow", vec![r.glow]),
        ("speed", r.speed.to_vec()),
        ("up", vec![r.up]),
        ("gravity", vec![r.gravity]),
        ("drag", vec![r.drag]),
        ("life", r.life.to_vec()),
        ("size", r.size.to_vec()),
        ("grow", vec![r.grow]),
        ("shrink", vec![r.shrink]),
        ("colour", r.colour.map_or(vec![], |c| c.to_vec())),
        ("spin", vec![r.spin]),
    ];
    if let Some((field, _)) = nums.iter().find(|(_, v)| v.iter().any(|x| !x.is_finite())) {
        return Err(format!("effect {n}: {field} is not a finite number"));
    }
    if r.size[0] > r.size[1] {
        return no("size is [low, high], from low to high");
    }
    if !(r.life[0] > 0.0 && r.life[0] <= r.life[1]) || !(r.size[0] > 0.0) || r.speed[0] > r.speed[1] || r.speed[0] < 0.0 {
        return no("life and size must be above 0, every pair from low to high");
    }
    if !(0.0..=1.0).contains(&r.grow) || !(0.0..=1.0).contains(&r.shrink) || r.grow + r.shrink > 1.0 {
        return no("grow and shrink are shares of the life, together at most 1");
    }
    if (r.count as f32 * SCALE[2]) as usize > CAP[0] / 4 {
        return Err(format!("effect {n}: {} particles an event at most", CAP[0] / 4));
    }
    if r.count == 0 && r.rate <= 0.0 {
        return no("a count, a rate or both");
    }
    if !(r.rate >= 0.0) || r.rate * r.life[1] * SCALE[2] > (CAP[0] / 4) as f32 {
        return Err(format!("effect {n}: a rate times the longest life at most {} alive", CAP[0] / 4));
    }
    if r.look == Look::Crack {
        return no("cracks are drawn by the dig, not by a row");
    }
    let coloured = matches!(r.look, Look::Square | Look::Texture);
    if r.look == Look::Square && r.colour.is_none() && r.gradient.is_empty() {
        return no("a square needs a colour or a gradient");
    }
    if !r.gradient.is_empty() && !coloured {
        return no("a gradient colours squares and textures only");
    }
    if r.glow != 0.0 && (!coloured || !(0.0..=8.0).contains(&r.glow)) {
        return no("glow is 0..8, on squares and textures");
    }
    if r.face != Face::Camera && matches!(r.look, Look::Debris | Look::Block) {
        return no("debris always tumbles and a block lies in its cell: no face");
    }
    // each start's own numbers, and only with it
    let fits = |s: Start, given: bool| given == (r.start == s);
    if !fits(Start::Ring, r.radius != 0.0) || !(r.radius >= 0.0) {
        return no("radius is over 0, for start = \"ring\" only");
    }
    if !fits(Start::Cone, r.angle != 0.0) || !(0.0..=180.0).contains(&r.angle) || r.dir != upward() && r.start != Start::Cone || Vec3::from(r.dir).length() < 1e-3 {
        return no("angle is 0..180 degrees about a dir, for start = \"cone\" only");
    }
    if !fits(Start::Box, r.size_box.is_some()) || r.size_box.is_some_and(|b| b.iter().any(|v| !(*v > 0.0))) {
        return no("box is three sizes over 0, for start = \"box\" only");
    }
    let curve = |c: &[[f32; 2]]| c.windows(2).all(|w| w[0][0] <= w[1][0]) && c.iter().all(|p| (0.0..=1.0).contains(&p[0]) && p[1] >= 0.0);
    if !curve(&r.size_curve) || !curve(&r.bright_curve) {
        return no("a curve's points are [share of the life 0..1, in order; value 0 or more]");
    }
    if !r.gradient.windows(2).all(|w| w[0].0 <= w[1].0) || r.gradient.iter().any(|s| !(0.0..=1.0).contains(&s.0)) {
        return no("a gradient's stops are [share of the life 0..1, in order; palette name]");
    }
    r.grad = r
        .gradient
        .iter()
        .map(|(t, c)| palette.colours.iter().find(|p| &p.name == c).map(|p| (*t, p.rgb.map(|v| v as f32 / 255.0))).ok_or(format!("effect {n}: colour '{c}' is not in the palette")))
        .collect::<Result<_, _>>()?;
    if let Some(z) = r.noise {
        // (a finer grain than 1 cm overflows `noise_at`'s cell numbers far from the body's centre)
        if !(z.strength >= 0.0 && z.scale >= 0.01 && z.speed >= 0.0) {
            return no("noise is { strength 0 or more, scale 0.01 m or more, speed 0 or more }");
        }
    }
    if (r.look == Look::Model) != r.model.is_some() {
        return no("look = \"model\" names a model, and only it");
    }
    if (r.look == Look::Texture) != r.texture.is_some() {
        return no("look = \"texture\" names a block, and only it");
    }
    if let Some(t) = &r.texture {
        r.tex = BLOCKS.placeable().find(|d| &d.name == t).map(|d| d.id).ok_or(format!("effect {n}: no block '{t}' for its texture"))?;
    }
    r.then_i = match &r.then {
        Some(t) => Some(names.iter().position(|m| m == t).ok_or(format!("effect {n}: no effect {t}"))? as u16),
        None => None,
    };
    Ok(())
}

/// One row from text (with or without its `[[effect]]` line) as the Studio edits it, checked; its `then` may
/// name a row of `set` or itself. `Fx::put` plays it.
pub fn parse_row(text: &str, set: &Set) -> Result<Row, String> {
    let t = if text.contains("[[effect]]") { text.to_string() } else { format!("[[effect]]\n{text}") };
    let mut f = toml::from_str::<File>(&t).map_err(|e| e.to_string())?;
    if f.effect.len() != 1 || !f.group.is_empty() {
        return Err("one [[effect]] row and no group".into());
    }
    let mut r = f.effect.pop().unwrap();
    let mut names: Vec<String> = set.rows.iter().map(|r| r.name.clone()).collect();
    names.push(r.name.clone());
    check(&mut r, &names, &PALETTE)?;
    Ok(r)
}

/// The built-in set: the file as compiled in.
pub static SET: LazyLock<Set> = LazyLock::new(|| parse_set(include_str!("../../../data/effects.toml"), &PALETTE).expect("data/effects.toml"));
/// Its rows: what `id` names (the code's own effects).
pub static ROWS: LazyLock<Vec<Row>> = LazyLock::new(|| SET.rows.clone());

/// A row's index by name; a name not in the file is a bug in the code that plays it.
pub fn id(name: &str) -> u16 {
    ROWS.iter().position(|r| r.name == name).unwrap_or_else(|| panic!("no effect {name} in data/effects.toml")) as u16
}

/// A curve of (x, value) points at x: straight between them, held past the ends; no points is 1.
pub fn curve_at(c: &[[f32; 2]], x: f32) -> f32 {
    let Some(k) = c.iter().position(|p| p[0] > x) else { return c.last().map_or(1.0, |p| p[1]) };
    if k == 0 {
        return c[0][1];
    }
    let (a, b) = (c[k - 1], c[k]);
    a[1] + (b[1] - a[1]) * (x - a[0]) / (b[0] - a[0])
}

/// A gradient's colour at x, as `curve_at` runs; no stops is white.
pub fn gradient_at(g: &[(f32, [f32; 3])], x: f32) -> [f32; 3] {
    let Some(k) = g.iter().position(|p| p.0 > x) else { return g.last().map_or([1.0; 3], |p| p.1) };
    if k == 0 {
        return g[0].1;
    }
    let (a, b) = (g[k - 1], g[k]);
    a.1.mix(b.1, (x - a.0) / (b.0 - a.0))
}

/// The noise field at p (metres) and time t: three channels of smooth value noise, each -1..1, continuous in
/// both and the same for the same inputs. The field slides along the diagonal at `speed` grains a second.
pub fn noise_at(p: Vec3, t: f32, n: &Noise) -> Vec3 {
    let q = p / n.scale + Vec3::splat(t * n.speed);
    let f = q.floor();
    let i = f.as_ivec3();
    let w = q - f;
    let w = w * w * (Vec3::splat(3.0) - 2.0 * w);
    let corner = |dx: i32, dy: i32, dz: i32| {
        let mut h = (i.x + dx) as u32 ^ ((i.y + dy) as u32).wrapping_mul(0x9e37_79b1) ^ ((i.z + dz) as u32).wrapping_mul(0x85eb_ca77);
        h ^= h >> 15;
        h = h.wrapping_mul(0x2c1b_3c6d);
        h ^= h >> 12;
        h = h.wrapping_mul(0x297a_2d39);
        h ^= h >> 15;
        // three channels from one hash, 10 bits each
        Vec3::new((h & 1023) as f32, (h >> 10 & 1023) as f32, (h >> 20 & 1023) as f32) / 511.5 - 1.0
    };
    let x0 = corner(0, 0, 0).lerp(corner(1, 0, 0), w.x);
    let x1 = corner(0, 1, 0).lerp(corner(1, 1, 0), w.x);
    let x2 = corner(0, 0, 1).lerp(corner(1, 0, 1), w.x);
    let x3 = corner(0, 1, 1).lerp(corner(1, 1, 1), w.x);
    x0.lerp(x1, w.y).lerp(x2.lerp(x3, w.y), w.z)
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
    pub block: u16,
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
    pub fn cell(world: &World, b: Block, block: u16) -> At {
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
    /// The middle of a cell's floor, as `cell` has the cell otherwise: where a fire or a fountain stands.
    pub fn floor_of(world: &World, b: Block) -> At {
        let mut at = At::cell(world, b, 0);
        at.p -= at.axes[0] * at.half.x;
        at
    }
    /// A point of body `body` (from its centre, in its frame) moving at v over the floor of the column it is in,
    /// looked up again as it crosses into others (a raindrop, ADR 0076); None inside a solid block.
    pub fn falling(world: &World, body: u8, p: DVec3, v: DVec3) -> Option<At> {
        let b = Block { body, ..world.block_at(world.bodies[body as usize].centre + p) };
        let (floor, col) = ground(world, b);
        (floor <= world.bodies[body as usize].layer_r(b.pos.z as f64) as f32).then(|| At { col, ..At::point(body, p, v, floor as f64) })
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
    block: u16,
    rest: bool,
    /// For the shader: the debris' texel seed, or the cell's columns i | j << 15 for a `block` look.
    a: u32,
    /// A square's colour (rgba8), or the cell's layer | face << 16.
    b: u32,
    tag: u32,
    /// Its own steady push, m/s²: `rise` up and `spread` sideways.
    push: Vec3,
}

/// A lasting effect playing (`Fx::start`): stopped, moved and restarted by its handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Handle(pub u32);

struct Emitter {
    handle: u32,
    row: u16,
    at: At,
    /// Particles owed by the rate, not yet whole.
    due: f64,
}

/// A particle with a model look, for models.rs to draw: its row, body, place (body frame, from the centre), its
/// turn (columns: the model's x, y, z in the body's frame) and scale.
#[derive(Clone, Copy, Debug)]
pub struct FxModel {
    pub row: u16,
    pub body: u8,
    pub p: Vec3,
    pub turn: glam::Mat3,
    pub size: f32,
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
    /// Cracks drawn since the last step.
    cracks: usize,
    /// The rows and groups it plays: the built-in set until a reload of the file (`watch`) or the Studio's
    /// `put` changes them.
    pub set: Set,
    /// `id`'s row numbers (the built-in set's) to the set's; `NO` where a row is gone.
    remap: Vec<u16>,
    emitters: Vec<Emitter>,
    /// This step's particles with a model look, and the lamps of the lasting effects that glow (body, world
    /// place, light).
    mout: Vec<FxModel>,
    lamps: Vec<(u8, DVec3, [f32; 3])>,
    /// Seconds stepped: the noise field's clock.
    clock: f32,
    rx: Option<std::sync::mpsc::Receiver<Result<Set, String>>>,
    /// `fxplay=` placed its effects.
    pub placed: bool,
    /// What `show` plays for the Studio.
    shown: Option<Handle>,
}

/// The cell events a frame that play an effect.
pub const EVENTS: u32 = 32;
const NO: u16 = u16::MAX;

impl Fx {
    pub fn new(on: bool, cap: usize, scale: f32, seed: u32) -> Fx {
        let cap = cap.max(1);
        let set = SET.clone();
        let remap = (0..set.rows.len() as u16).collect();
        Fx {
            on,
            pool: Vec::with_capacity(cap),
            out: Vec::with_capacity(cap + CRACKS),
            later: Vec::with_capacity(256),
            cap,
            scale,
            rng: seed.max(1),
            next_tag: 1,
            played: 0,
            events: 0,
            cursor: 0,
            setting: None,
            cracks: 0,
            set,
            remap,
            emitters: Vec::new(),
            mout: Vec::new(),
            lamps: Vec::new(),
            clock: 0.0,
            rx: None,
            placed: false,
            shown: None,
        }
    }

    /// Reads `file` again within a second of it changing, on a thread of its own; the frame thread swaps the
    /// new rows in at its next `step`, or names the refusal and keeps the old.
    pub fn watch(&mut self, file: std::path::PathBuf) {
        let dir = file.parent().map_or(std::path::PathBuf::from("."), |d| d.to_path_buf());
        let shown = file.display().to_string().replace('\\', "/");
        self.rx = Some(crate::gpu::watch("effects", dir, Vec::new(), move |_| {
            let Ok(text) = std::fs::read_to_string(&file) else { return (Err(format!("{shown}: not found")), false) };
            let palette = std::fs::read_to_string("data/palette.toml").ok().and_then(|t| Palette::parse(&t).ok()).unwrap_or_else(|| PALETTE.clone());
            (parse_set(&text, &palette).map_err(|e| format!("{shown}: {e}")), true)
        }));
    }

    /// A new set of rows: the living particles and lasting effects keep theirs by name (those whose row is gone
    /// go). Refused when it lacks a row the code plays (`id`'s).
    pub fn swap(&mut self, set: Set) -> Result<(), String> {
        let remap: Vec<u16> = ROWS.iter().map(|r| set.rows.iter().position(|m| m.name == r.name).map_or(NO, |i| i as u16)).collect();
        if let Some(k) = remap.iter().position(|&i| i == NO) {
            return Err(format!("effect {} is played by the game itself: keep it", ROWS[k].name));
        }
        let by_name: Vec<u16> = self.set.rows.iter().map(|r| set.rows.iter().position(|m| m.name == r.name).map_or(NO, |i| i as u16)).collect();
        self.pool.retain_mut(|p| {
            p.row = by_name[p.row as usize];
            p.row != NO
        });
        self.emitters.retain_mut(|e| {
            e.row = by_name[e.row as usize];
            e.row != NO
        });
        self.later.clear();
        (self.set, self.remap) = (set, remap);
        Ok(())
    }

    /// One row put into the set as the Studio edits it (by name: replaced, or added): every `then` and group
    /// keeps naming the same rows.
    pub fn put(&mut self, row: Row) {
        let mut set = self.set.clone();
        match set.rows.iter().position(|r| r.name == row.name) {
            Some(i) => set.rows[i] = row,
            None => set.rows.push(row),
        }
        let names: Vec<String> = set.rows.iter().map(|r| r.name.clone()).collect();
        for r in set.rows.iter_mut() {
            r.then_i = r.then.as_ref().and_then(|t| names.iter().position(|m| m == t)).map(|i| i as u16);
        }
        let _ = self.swap(set);
    }

    /// The rows' names, then the groups'.
    pub fn names(&self) -> Vec<&str> {
        self.set.names()
    }

    /// The row of a particle or lasting effect by its number in `set`.
    pub fn row(&self, i: u16) -> &Row {
        &self.set.rows[i as usize]
    }

    /// Plays a row or group once where `at` is: each row's `count`. False when no row or group has that name.
    pub fn play_named(&mut self, name: &str, at: &At) -> bool {
        let Some(rows) = self.set.rows_of(name) else { return false };
        for r in rows {
            let tag = self.tag();
            self.burst(r, at, tag);
        }
        true
    }

    /// Starts a row or group lasting at `at`: each row's `count` at once, then its `rate` a second until
    /// `stop`. None when effects are off or no row or group has that name.
    pub fn start(&mut self, name: &str, at: &At) -> Option<Handle> {
        let rows = self.set.rows_of(name).filter(|_| self.on)?;
        let h = self.tag();
        for r in rows {
            self.burst(r, at, h);
            // a count-only row too, so `restart` knows it; `emit` passes it by
            self.emitters.push(Emitter { handle: h, row: r, at: *at, due: 0.0 });
        }
        Some(Handle(h))
    }

    /// A lasting effect's new place (it follows a moving part); its particles already out fly on.
    pub fn moved(&mut self, h: Handle, at: &At) {
        self.emitters.iter_mut().filter(|e| e.handle == h.0).for_each(|e| e.at = *at);
    }

    /// Stops a lasting effect: no more particles; those out live their lives.
    pub fn stop(&mut self, h: Handle) {
        self.emitters.retain(|e| e.handle != h.0);
    }

    /// Plays a lasting effect again from its start: its particles gone, its counts burst again.
    pub fn restart(&mut self, h: Handle) {
        self.end(h.0);
        let es: Vec<(u16, At)> = self.emitters.iter_mut().filter(|e| e.handle == h.0).map(|e| {
            e.due = 0.0;
            (e.row, e.at)
        }).collect();
        for (r, at) in es {
            self.burst(r, &at, h.0);
        }
    }

    /// The Studio's view: `name` lasting on the floor of `cell` in place of what it showed (None: nothing);
    /// `restart` with the handle plays it again.
    pub fn show(&mut self, name: Option<&str>, world: &World, cell: Block) -> Option<Handle> {
        if let Some(h) = self.shown.take() {
            self.stop(h);
            self.end(h.0);
        }
        self.shown = name.and_then(|n| self.start(n, &At::floor_of(world, cell)));
        self.shown
    }

    /// `fxplay=name[:n]`, once the world is loaded: n of a row or group lasting on the ground in a widening
    /// fan ahead of `eye` (position, look), 1.5 m apart from 3 m out, as `model=` stands its models.
    pub fn scene(&mut self, world: &World, eye: (DVec3, DVec3), spec: &str) {
        if self.placed || spec.is_empty() {
            return;
        }
        self.placed = true;
        let (name, n) = spec.split_once(':').map_or((spec, 1), |(a, b)| (a, b.parse().unwrap_or(1)));
        let pl = &world.bodies[0];
        let up = (eye.0 - pl.centre).normalize();
        let ahead = (eye.1 - up * eye.1.dot(up)).normalize_or(DVec3::X);
        let side = up.cross(ahead);
        let (mut a, mut k) = (3.0, 0);
        while k < n {
            let mut b = -(0.6 * a / 1.5f64).floor() * 1.5;
            while b <= 0.6 * a && k < n {
                let d = (eye.0 + ahead * a + side * b - pl.centre).normalize();
                let c = world.air_over_ground(0, d);
                if self.start(name, &At::floor_of(world, c)).is_none() {
                    eprintln!("[technoape] fxplay: no effect {name}");
                    return;
                }
                (b, k) = (b + 1.5, k + 1);
            }
            a += 1.5;
        }
    }

    /// This step's particles with a model look, and their rows' model names.
    pub fn models(&self) -> impl Iterator<Item = (&str, &FxModel)> {
        self.mout.iter().map(|m| (self.set.rows[m.row as usize].model.as_deref().unwrap_or(""), m))
    }

    /// The lamps of the lasting effects that glow, this step: (body, place in the world, light).
    pub fn lamps(&self) -> &[(u8, DVec3, [f32; 3])] {
        &self.lamps
    }

    fn tag(&mut self) -> u32 {
        let tag = self.next_tag;
        self.next_tag = self.next_tag.wrapping_add(1).max(1);
        tag
    }

    /// A new cap and scale (the Particles setting changed): the pool shrinks at once or grows its room once.
    pub fn set(&mut self, cap: usize, scale: f32) {
        self.cap = cap.max(1);
        self.scale = scale;
        self.pool.truncate(self.cap);
        self.pool.reserve_exact(self.cap - self.pool.len());
        self.out.reserve_exact((self.cap + CRACKS).saturating_sub(self.out.len()));
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
        let tag = self.tag();
        self.burst(self.remap[row as usize], at, tag);
        tag
    }

    /// Row `row` of the set's `count` at `at`, its particles tagged `tag`.
    fn burst(&mut self, row: u16, at: &At, tag: u32) {
        let count = self.set.rows[row as usize].count;
        if !self.on || count == 0 {
            return;
        }
        // a row of one stays one: a second grow or drop would lie on the first
        let n = if count == 1 { 1 } else { ((count as f32 * self.scale).round() as usize).max(1) };
        self.played += 1;
        let set = std::mem::take(&mut self.set);
        for _ in 0..n {
            self.spawn(&set.rows[row as usize], row, at, tag);
        }
        self.set = set;
    }

    /// One particle of row `r` (number `row`) at `at`. The random numbers are drawn in the order the rows
    /// before ADR 0050 drew them; the new starts and pushes draw more only when a row asks for them.
    fn spawn(&mut self, r: &Row, row: u16, at: &At, tag: u32) {
        use std::f32::consts::PI;
        let [up, s, t] = at.axes;
        let (u, v, w) = (self.rnd() * 2.0 - 1.0, self.rnd() * 2.0 - 1.0, self.rnd() * 2.0 - 1.0);
        let off = match r.start {
            Start::Point | Start::Cone => Vec3::ZERO,
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
            Start::Ring => {
                let a = (u + 1.0) * PI;
                (s * a.cos() + t * a.sin()) * r.radius
            }
            Start::Box => {
                let b = r.size_box.unwrap_or([0.0; 3]);
                (s * u * b[0] + up * v * b[1] + t * w * b[2]) * 0.5
            }
        };
        let speed = r.speed[0] + (r.speed[1] - r.speed[0]) * self.rnd();
        let d = if r.start == Start::Cone {
            // uniform over the cap within `angle` of the axis
            let axis = (s * r.dir[0] + up * r.dir[1] + t * r.dir[2]).normalize();
            let c = 1.0 - (v * 0.5 + 0.5) * (1.0 - r.angle.to_radians().cos());
            let (b0, b1) = axis.any_orthonormal_pair();
            let a = (w + 1.0) * PI;
            axis * c + (b0 * a.cos() + b1 * a.sin()) * (1.0 - c * c).max(0.0).sqrt()
        } else {
            self.dir()
        };
        let life = r.life[0] + (r.life[1] - r.life[0]) * self.rnd();
        let size = if r.grow > 0.0 { r.size[0] } else { r.size[0] + (r.size[1] - r.size[0]) * self.rnd() };
        let spin = r.spin * (self.rnd() * 2.0 - 1.0);
        let seed = (self.rnd() * 16777216.0) as u32;
        let (a, b) = match (r.look, at.cell) {
            (Look::Block, Some(c)) => cell_code(c),
            (Look::Square | Look::Texture, _) => {
                let c = r.colour.unwrap_or([1.0; 3]).map(|x| (x.clamp(0.0, 1.0) * 255.0).round() as u32);
                (seed, c[0] | c[1] << 8 | c[2] << 16 | 255 << 24)
            }
            _ => (seed, 0),
        };
        let mut push = up * r.rise;
        if r.spread != 0.0 {
            let a = self.rnd() * 2.0 * PI;
            push += (s * a.cos() + t * a.sin()) * r.spread;
        }
        let mut x = at.p + off;
        // no part of it starts under the floor it lands on
        let half = if matches!(r.look, Look::Block | Look::Model) { 0.0 } else { size * 0.5 };
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
            push,
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
        // the file changed: its rows in, or why not
        if let Some(got) = self.rx.as_ref().and_then(|rx| rx.try_iter().last()) {
            match got.and_then(|set| self.swap(set)) {
                Ok(()) => eprintln!("[technoape] effects: {} rows, {} groups read", self.set.rows.len(), self.set.groups.len()),
                Err(e) => eprintln!("[technoape] effects: {e}; the rows before stay"),
            }
        }
        self.events = 0;
        self.out.clear();
        self.mout.clear();
        self.cracks = 0;
        self.clock += dt;
        self.emit(world, dt);
        let rows = &self.set.rows;
        let clock = self.clock;
        let mut i = 0;
        while i < self.pool.len() {
            let q = &mut self.pool[i];
            let r = &rows[q.row as usize];
            q.age += dt;
            if !q.rest && q.age < q.life {
                let up = q.p.normalize();
                q.v += (up * (-g0 * r.gravity) - q.v * r.drag) * dt;
                // (the rows before ADR 0050 push nothing: their motion stays exactly as it was)
                if let Some(n) = &r.noise {
                    q.v += (q.push + noise_at(q.p, clock, n) * n.strength) * dt;
                } else if q.push != Vec3::ZERO {
                    q.v += q.push * dt;
                }
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
                let half = if matches!(r.look, Look::Block | Look::Model) { 0.0 } else { q.size * 0.5 };
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
            if !r.size_curve.is_empty() {
                size *= curve_at(&r.size_curve, x);
            }
            if r.look == Look::Model {
                // upright about its up, along its flight, or turning about the axis its seed picks
                let up = q.p.normalize();
                let (s, t) = up.any_orthonormal_pair();
                // (t, up, s), not (s, up, t): `any_orthonormal_pair` gives s x t = up, so this is a turn, not a mirror
                let upright = glam::Mat3::from_cols(t, up, s);
                let turn = match r.face {
                    Face::Flight if q.v.length_squared() > 1e-6 => glam::Mat3::from_quat(glam::Quat::from_rotation_arc(Vec3::Y, q.v.normalize())),
                    Face::Tumble => upright * glam::Mat3::from_axis_angle(seed_axis(q.a), q.angle),
                    _ => upright,
                };
                self.mout.push(FxModel { row: q.row, body: q.body, p: q.p, turn, size });
                i += 1;
                continue;
            }
            let mut b = q.b;
            if !r.grad.is_empty() || !r.bright_curve.is_empty() {
                let c = if r.grad.is_empty() { r.colour.unwrap_or([1.0; 3]) } else { gradient_at(&r.grad, x) };
                let k = curve_at(&r.bright_curve, x);
                let c = c.map(|v| ((v * k).clamp(0.0, 1.0) * 255.0).round() as u32);
                b = c[0] | c[1] << 8 | c[2] << 16 | 255 << 24;
            }
            let (gl, mut a, mut angle) = (if r.look == Look::Texture { Look::Square } else { r.look } as u32, q.a, q.angle);
            let mut info = q.block as u32 | (q.body as u32) << 8 | gl << 10;
            if gl == 1 {
                // the new looks' bits (ADR 0050): face << 16, a texture's block in the block's place | 1 << 18,
                // glow * 32 << 24; along the flight `a` is its direction and `angle` its streak, a 30th of a second
                info |= (r.face as u32) << 16 | (((r.glow * 32.0).round() as u32).min(255)) << 24;
                if r.look == Look::Texture {
                    info = (info & !255) | r.tex as u32 | 1 << 18;
                }
                if r.face == Face::Flight {
                    a = pack_dir(q.v);
                    angle = q.v.length() / 30.0;
                }
            }
            self.out.push(FxGpu { p: q.p.to_array(), size, info, angle, a, b });
            i += 1;
        }
        while let Some((t, at)) = self.later.pop() {
            let tag = self.tag();
            self.burst(t, &at, tag);
        }
    }

    /// The lasting effects' particles this step, by their rates, and the lamps of those that glow (one a
    /// handle, at its place, its rows' glows added, flickering by the noise of the clock).
    fn emit(&mut self, world: &World, dt: f32) {
        self.lamps.clear();
        let mut last = None;
        if self.emitters.is_empty() {
            return;
        }
        let set = std::mem::take(&mut self.set);
        let mut emitters = std::mem::take(&mut self.emitters);
        for e in emitters.iter_mut() {
            let r = &set.rows[e.row as usize];
            if r.rate <= 0.0 {
                continue;
            }
            e.due += (r.rate * self.scale * dt) as f64;
            // (a hair over: twelve sixtieths of a second at 5 a second is one, not 0.9999)
            let n = (e.due + 1e-6).floor();
            e.due -= n;
            for _ in 0..n as usize {
                self.spawn(r, e.row, &e.at, e.handle);
            }
            if r.glow > 0.0 {
                let c = if r.grad.is_empty() { r.colour.unwrap_or([1.0; 3]) } else { r.grad.iter().fold([0.0; 3], |s, g| [0, 1, 2].map(|k| s[k] + g.1[k] / r.grad.len() as f32)) };
                let mut c = c.map(|v| v * r.glow * 0.25);
                if let Some(z) = &r.noise {
                    let f = 0.8 + 0.2 * noise_at(Vec3::new(e.handle as f32 * 7.3, 0.0, 0.0), self.clock * 4.0, &Noise { strength: 1.0, scale: 1.0, speed: z.speed.max(1.0) }).x;
                    c = c.map(|v| v * f);
                }
                match self.lamps.last_mut() {
                    // a group's rows add up to one lamp
                    Some(l) if last == Some(e.handle) => l.2 = [l.2[0] + c[0], l.2[1] + c[1], l.2[2] + c[2]],
                    _ => {
                        self.lamps.push((e.at.body, world.bodies[e.at.body as usize].centre + e.at.p.as_dvec3(), c));
                        last = Some(e.handle);
                    }
                }
            }
        }
        self.emitters = emitters;
        self.set = set;
    }

    #[cfg(test)]
    fn resting(&self, world: &World) -> Vec<Block> {
        self.pool.iter().filter(|p| p.rest).map(|p| world.block_at(world.bodies[p.body as usize].centre + p.p.as_dvec3())).collect()
    }

    /// Cracks over cell b this frame, `k` of the way (0..1) to its dig's end (ADR 0034): drawn over the real
    /// block in the block's own colours, not kept (the next `step` clears them), `CRACKS` a frame at most.
    pub fn crack(&mut self, world: &World, b: Block, k: f32) {
        if !self.on || self.cracks >= CRACKS {
            return;
        }
        self.cracks += 1;
        let (a, code) = cell_code(b);
        self.out.push(FxGpu { p: [0.0; 3], size: 1.0, info: world.block(b) as u32 | (b.body as u32) << 8 | (Look::Crack as u32) << 10, angle: k.clamp(0.0, 1.0), a, b: code });
    }

    /// The particles for the GPU as the last step left them, in a buffer kept from frame to frame.
    pub fn drawn(&self) -> &[FxGpu] {
        &self.out
    }
}

/// A unit axis from a particle's seed, as the shader's debris turns about.
fn seed_axis(a: u32) -> Vec3 {
    let h = a.wrapping_mul(747796405).wrapping_add(2891336453);
 (Vec3::new((h & 255) as f32, (h >> 8 & 255) as f32, (h >> 16 & 255) as f32) - Vec3::splat(127.25)).normalize_or(Vec3::Y)
}

/// A direction as three 10-bit numbers for the shader (`unpack_dir` in shader.wgsl).
fn pack_dir(v: Vec3) -> u32 {
    let d = v.normalize_or(Vec3::Y).to_array().map(|x| ((x * 0.5 + 0.5) * 1023.0).round() as u32);
    d[0] | d[1] << 10 | d[2] << 20
}

/// A cell as the shader's `block` and `crack` looks find it on the grid: column and face.
fn cell_code(c: Block) -> (u32, u32) {
    (c.pos.x as u32 & 0x7fff | (c.pos.y as u32 & 0x7fff) << 15, c.pos.z as u32 & 0xffff | (c.face as u32) << 16)
}

/// A cell the client's world changed, with what it was: a solid block gone plays the break, one come the place.
pub fn changed(fx: &mut Fx, world: &World, b: Block, was: u16, now: u16) {
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
pub fn server_edit(fx: &mut Fx, pred: &mut Predictor, world: &mut World, b: Block, m: u16, hear: bool) -> (bool, Option<u16>) {
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
    fn cracks_draw_over_a_cell_for_one_frame_and_never_grow_the_buffer() {
        let w = world();
        let c = surface(&w);
        let mut fx = Fx::new(true, 16, 1.0, 1);
        let room = fx.out.capacity();
        fx.step(&w, 0.01);
        fx.crack(&w, c, 0.5);
        let d = fx.drawn();
        assert_eq!(d.len(), 1);
        assert_eq!((d[0].angle, d[0].info >> 10 & 3, d[0].info & 255), (0.5, Look::Crack as u32, w.block(c) as u32));
        // drawn this frame only: the caller says again each frame how far the dig is
        fx.step(&w, 0.01);
        assert!(fx.drawn().is_empty());
        for _ in 0..CRACKS + 5 {
            fx.crack(&w, c, 1.0);
        }
        assert_eq!(fx.drawn().len(), CRACKS);
        assert_eq!(fx.out.capacity(), room);
        // effects off: none
        let mut off = Fx::new(false, 16, 1.0, 1);
        off.crack(&w, c, 0.5);
        assert!(off.drawn().is_empty());
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

    /// ADR 0050: the rows before it draw exactly as they did. The numbers are the drawn particles' bytes over
    /// two seconds, hashed, as the code before ADR 0050 made them.
    #[test]
    fn the_old_rows_draw_exactly_as_before() {
        let mut w = world();
        let c = surface(&w);
        w.set_block(c, AIR);
        let at = At::cell(&w, c, BLOCKS.id("stone"));
        let mut got = Vec::new();
        for name in ["break", "place", "grow", "spray", "splash", "chip"] {
            let mut fx = Fx::new(true, 4096, 1.0, 11);
            fx.play(id(name), &at);
            let mut h = engine::world::FNV0;
            for _ in 0..120 {
                fx.step(&w, 1.0 / 60.0);
                h = engine::world::fnv(h, bytemuck::cast_slice(fx.drawn()));
            }
            got.push((name, h));
        }
        println!("{got:?}");
        let want: [u64; 6] = [16124491450335446172, 12728753821768735219, 10756546070781930058, 5885070126393282328, 9628921545010191082, 7602677355907020114];
        assert_eq!(got.iter().map(|g| g.1).collect::<Vec<_>>(), want);
    }

    #[test]
    fn curves_and_gradients_run_between_their_points_and_hold_past_them() {
        let c = [[0.0, 1.0], [0.5, 2.0], [1.0, 0.0]];
        assert_eq!((curve_at(&c, 0.25), curve_at(&c, 0.75), curve_at(&c, -1.0), curve_at(&c, 2.0)), (1.5, 1.0, 1.0, 0.0));
        assert_eq!(curve_at(&[], 0.3), 1.0);
        assert_eq!(curve_at(&[[0.4, 3.0]], 0.0), 3.0);
        let g = [(0.0, [1.0, 1.0, 1.0]), (0.5, [1.0, 0.0, 0.0]), (1.0, [0.0, 0.0, 0.0])];
        assert_eq!(gradient_at(&g, 0.25), [1.0, 0.5, 0.5]);
        assert_eq!(gradient_at(&g, 0.75), [0.5, 0.0, 0.0]);
        assert_eq!(gradient_at(&g, 1.5), [0.0; 3]);
        // a row's gradient by palette names
        let set = parse_set("[[effect]]\nname='a'\ncount=1\nlife=[1,1]\nsize=[0.1,0.1]\nlook='square'\ngradient=[[0.0,'lamp light'],[1.0,'stone dark']]", &PALETTE).unwrap();
        assert_eq!(set.rows[0].grad[1].1, [0x46 as f32 / 255.0, 0x46 as f32 / 255.0, 0x49 as f32 / 255.0]);
    }

    #[test]
    fn the_noise_field_is_the_same_for_the_same_place_and_time_and_continuous() {
        let n = Noise { strength: 1.0, scale: 0.5, speed: 1.0 };
        let p = Vec3::new(3.1, 1200.7, -40.2);
        assert_eq!(noise_at(p, 2.0, &n), noise_at(p, 2.0, &n));
        let mut most = 0.0f32;
        let mut spread = 0.0f32;
        for k in 0..2000 {
            let q = p + Vec3::new(k as f32 * 0.0137, k as f32 * 0.0071, 0.0);
            let a = noise_at(q, 2.0, &n);
            assert!(a.abs().max_element() <= 1.0);
            // a millimetre or a millisecond on moves it a little only
            most = most.max((noise_at(q + Vec3::X * 0.001, 2.0, &n) - a).length()).max((noise_at(q, 2.001, &n) - a).length());
            spread = spread.max((a - noise_at(p, 2.0, &n)).length());
        }
        assert!(most < 0.02, "a jump of {most}");
        assert!(spread > 0.5, "the field barely changes: {spread}");
        // it moves with time
        assert_ne!(noise_at(p, 2.0, &n), noise_at(p, 3.0, &n));
    }

    const LASTING: &str = "[[effect]]\nname='t flames'\nrate=10\nlife=[5,5]\nsize=[0.1,0.1]\nlook='square'\ncolour=[1,0.5,0]\ngravity=0\nglow=2\n\n[[effect]]\nname='t smoke'\ncount=2\nrate=4\nlife=[5,5]\nsize=[0.1,0.1]\nlook='square'\ncolour=[0.3,0.3,0.3]\ngravity=0\nrise=0.5\nspread=0.2\n\n[[group]]\nname='t fire'\nrows=['t flames','t smoke']\n";

    fn pool_with(text: &str, scale: f32) -> Fx {
        let mut fx = Fx::new(true, 4096, scale, 3);
        let mut set = SET.clone();
        let extra = parse_set(text, &PALETTE).unwrap();
        let k = set.rows.len() as u16;
        set.rows.extend(extra.rows);
        set.groups.extend(extra.groups.into_iter().map(|mut g| {
            g.ids.iter_mut().for_each(|i| *i += k);
            g
        }));
        fx.swap(set).unwrap();
        fx
    }

    #[test]
    fn a_rate_makes_its_count_over_time_until_stopped_and_a_group_plays_its_rows_as_one() {
        let w = world();
        let at = At::point(0, DVec3::new(0.0, 2000.0, 0.0), DVec3::ZERO, 0.0);
        for scale in [1.0, 2.0] {
            let mut fx = pool_with(LASTING, scale);
            let h = fx.start("t flames", &at).unwrap();
            for _ in 0..120 {
                fx.step(&w, 1.0 / 60.0);
            }
            assert_eq!(fx.live(), (20.0 * scale) as usize, "10 a second for 2 s at scale {scale}");
            fx.stop(h);
            for _ in 0..60 {
                fx.step(&w, 1.0 / 60.0);
            }
            assert_eq!(fx.live(), (20.0 * scale) as usize, "stopped: none more, those out live on");
        }
        // the group: both rows, smoke's count at once, then 10 + 4 a second; one lamp for the glowing flames
        let mut fx = pool_with(LASTING, 1.0);
        assert_eq!(fx.set.rows_of("t fire").unwrap().len(), 2);
        let h = fx.start("t fire", &at).unwrap();
        assert_eq!(fx.live(), 2);
        for _ in 0..60 {
            fx.step(&w, 1.0 / 60.0);
        }
        assert_eq!(fx.live(), 2 + 14);
        assert_eq!(fx.lamps().len(), 1);
        // it follows its place, restarts from nothing, and stops as one
        let far = At::point(0, DVec3::new(0.0, 2000.0, 50.0), DVec3::ZERO, 0.0);
        fx.moved(h, &far);
        fx.step(&w, 0.5);
        assert!(fx.pool.iter().filter(|p| p.p.z > 49.0).count() >= 7);
        fx.restart(h);
        assert_eq!(fx.live(), 2);
        fx.stop(h);
        fx.step(&w, 1.0);
        assert_eq!((fx.live(), fx.lamps().len()), (2, 0));
        assert!(fx.start("no such", &at).is_none());
        // a count-only row: a restart bursts it again, and it adds no more over time
        let mut fx = pool_with(&LASTING.replace("count=2\nrate=4", "count=3"), 1.0);
        let h = fx.start("t smoke", &at).unwrap();
        fx.step(&w, 1.0);
        assert_eq!(fx.live(), 3);
        fx.restart(h);
        assert_eq!(fx.live(), 3, "restarted: its count again");
        fx.step(&w, 1.0);
        assert_eq!(fx.live(), 3);
        // effects off: nothing starts
        assert!(Fx::new(false, 16, 1.0, 1).start("break", &at).is_none());
    }

    #[test]
    fn each_start_keeps_to_its_shape() {
        let rows = "[[effect]]\nname='ring'\ncount=100\nstart='ring'\nradius=0.5\nlife=[1,1]\nsize=[0.1,0.1]\nlook='debris'\n\n\
                    [[effect]]\nname='box'\ncount=100\nstart='box'\nbox=[2.0,0.5,1.0]\nlife=[1,1]\nsize=[0.1,0.1]\nlook='debris'\n\n\
                    [[effect]]\nname='cone'\ncount=100\nstart='cone'\nangle=20\ndir=[1,0,0]\nspeed=[1,1]\nlife=[1,1]\nsize=[0.1,0.1]\nlook='debris'\n";
        let mut fx = pool_with(rows, 1.0);
        let at = At::point(0, DVec3::new(0.0, 2000.0, 0.0), DVec3::ZERO, 0.0);
        let [up, s, t] = at.axes;
        let mut last = 0;
        for (name, ok) in [
            ("ring", &(|o: Vec3, _: Vec3| (o.length() - 0.5).abs() < 1e-3 && o.dot(up).abs() < 1e-3) as &dyn Fn(Vec3, Vec3) -> bool),
            ("box", &|o: Vec3, _| o.dot(s).abs() <= 1.0 + 1e-3 && o.dot(up).abs() <= 0.25 + 1e-3 && o.dot(t).abs() <= 0.5 + 1e-3),
            ("cone", &|o: Vec3, v: Vec3| o.length() < 1e-3 && v.normalize().dot(s) >= 20f32.to_radians().cos() - 1e-4),
        ] {
            fx.play_named(name, &at);
            let new = &fx.pool[last..];
            assert_eq!(new.len(), 100);
            assert!(new.iter().all(|p| ok(p.p - at.p, p.v)), "{name} out of its shape");
            // and fills it: not all in one place
            assert!(new.iter().any(|p| (p.p - new[0].p).length() > 0.1 || (p.v - new[0].v).length() > 0.1), "{name} all alike");
            last = fx.pool.len();
        }
    }

    #[test]
    fn new_fields_are_refused_by_row_and_reason() {
        let base = "[[effect]]\nname='a'\ncount=1\nlife=[1,1]\nsize=[0.1,0.1]\n";
        for (extra, why) in [
            ("look='debris'\nradius=1", "radius"),
            ("look='debris'\nstart='ring'", "radius"),
            ("look='debris'\nstart='box'", "box"),
            ("look='debris'\nangle=30", "angle"),
            ("look='debris'\nstart='cone'\nangle=200", "angle"),
            ("look='square'\ngradient=[[0.0,'mauve']]", "colour 'mauve' is not in the palette"),
            ("look='square'\ngradient=[[0.5,'stone'],[0.2,'stone']]", "in order"),
            ("look='debris'\ngradient=[[0.0,'stone']]", "squares and textures"),
            ("look='square'\ncolour=[1,1,1]\nsize_curve=[[1.5,1]]", "curve"),
            ("look='square'\ncolour=[1,1,1]\nnoise={strength=1,scale=0}", "noise"),
            ("look='square'\ncolour=[1,1,1]\nnoise={strength=1,wobble=2}", "unknown field"),
            ("look='debris'\nglow=1", "glow"),
            ("look='debris'\nface='flight'", "no face"),
            ("look='model'", "names a model"),
            ("look='texture'\ntexture='cheese'", "no block 'cheese'"),
            ("look='square'\ncolour=[1,1,1]\nrate=1000", "rate"),
            ("look='debris'\nspin=nan", "spin"),
            ("look='debris'\ngravity=inf", "gravity"),
            ("look='square'\ncolour=[1,nan,1]", "colour"),
            ("look='square'\ncolour=[1,1,1]\nnoise={strength=1,scale=0.001}", "0.01"),
        ] {
            let text = format!("{base}{extra}");
            let e = parse(&text).map(|_| ()).expect_err(why);
            assert!(e.contains("effect a") || e.contains("unknown field"), "{why}: {e}");
            assert!(e.contains(why), "{why}: {e}");
        }
        let e = parse("[[effect]]\nname='a'\ncount=1\nlife=[1,1]\nsize=[0.2,0.1]\nlook='debris'").unwrap_err();
        assert!(e.contains("effect a") && e.contains("size"), "a pair from high to low: {e}");
        let e = parse("[[effect]]\nname='a'\nlife=[1,1]\nsize=[0.1,0.1]\nlook='debris'").unwrap_err();
        assert!(e.contains("a count, a rate"), "{e}");
        let e = parse_set(&format!("{base}look='debris'\n[[group]]\nname='g'\nrows=['a','b']"), &PALETTE).unwrap_err();
        assert_eq!(e, "group g: no effect b");
        let e = parse_set(&format!("{base}look='debris'\n[[group]]\nname='a'\nrows=['a']"), &PALETTE).unwrap_err();
        assert!(e.contains("group a: the name of a row"), "{e}");
        // the Studio's one row: checked the same, its `then` may name the set's rows
        assert!(parse_row("name='x'\ncount=1\nlife=[1,1]\nsize=[0.1,0.1]\nlook='debris'\nthen='splash'", &SET).is_ok());
        assert!(parse_row("name='x'\ncount=1\nlife=[1,1]\nsize=[0.1,0.1]\nlook='debris'\nthen='nope'", &SET).unwrap_err().contains("no effect nope"));
    }

    #[test]
    fn the_drafts_are_there_and_a_reload_keeps_what_plays_by_name() {
        for name in ["fire", "smoke", "sparks", "steam", "dust", "drip", "glint", "leaves"] {
            assert!(SET.rows_of(name).is_some(), "draft {name}");
            assert!(include_str!("../../../data/drafts.txt").contains(&format!("data/effects.toml#{name}")), "{name} not in drafts.txt");
        }
        for r in SET.rows.iter().filter(|r| r.look == Look::Model) {
            let m = r.model.as_ref().unwrap();
            assert!(std::path::Path::new(&format!("../../data/models/{m}.toml")).exists(), "effect {}: no model {m}", r.name);
        }
        let w = world();
        let mut fx = Fx::new(true, 4096, 1.0, 3);
        let at = At::point(0, DVec3::new(0.0, 2000.0, 0.0), DVec3::ZERO, 0.0);
        let h = fx.start("fire", &at).unwrap();
        fx.step(&w, 0.5);
        let live = fx.live();
        assert!(live > 0);
        // the same rows in another order: everything keeps playing
        let mut set = SET.clone();
        set.rows.reverse();
        let names: Vec<String> = set.rows.iter().map(|r| r.name.clone()).collect();
        for g in set.groups.iter_mut() {
            g.ids = g.rows.iter().map(|m| names.iter().position(|k| k == m).unwrap() as u16).collect();
        }
        for r in set.rows.iter_mut() {
            r.then_i = r.then.as_ref().map(|t| names.iter().position(|m| m == t).unwrap() as u16);
        }
        fx.swap(set).unwrap();
        assert_eq!(fx.live(), live);
        assert!(fx.pool.iter().all(|p| fx.set.rows[p.row as usize].name != "break"));
        fx.step(&w, 0.5);
        assert!(fx.live() > live, "the lasting fire burns on");
        assert_eq!(fx.play(id("break"), &at) > 0, true);
        assert!(fx.pool.iter().any(|p| fx.set.rows[p.row as usize].name == "break"), "id's numbers follow the new order");
        fx.stop(h);
        // a set without a row the code plays is refused
        let mut set = SET.clone();
        set.rows.retain(|r| r.name != "chip");
        assert!(fx.swap(set).unwrap_err().contains("chip"));
    }

    #[test]
    fn a_model_particle_is_turned_never_mirrored() {
        // a mirrored turn draws the model inside out under back-face culling
        let rows = "[[effect]]\nname='m up'\ncount=1\nlife=[5,5]\nsize=[1,1]\nlook='model'\nmodel='leaf'\ngravity=0\n\n\
                    [[effect]]\nname='m tumble'\ncount=1\nlife=[5,5]\nsize=[1,1]\nlook='model'\nmodel='leaf'\nface='tumble'\nspin=3\ngravity=0\n";
        let mut fx = pool_with(rows, 1.0);
        let at = At::point(0, DVec3::new(300.0, 2000.0, -700.0), DVec3::ZERO, 0.0);
        fx.play_named("m up", &at);
        fx.play_named("m tumble", &at);
        fx.step(&world(), 0.1);
        let dets: Vec<f32> = fx.models().map(|(_, m)| m.turn.determinant()).collect();
        assert_eq!(dets.len(), 2);
        assert!(dets.iter().all(|d| (d - 1.0).abs() < 1e-4), "a turn, not a mirror: {dets:?}");
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
