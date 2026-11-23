//! Effects (ADR 0031, docs/design/effects.md): particles and motion, presentation only. An event names a row of
//! `data/effects.toml` and a place; the pool moves the row's particles in the body's frame under the body's own
//! pull, over the floor of the column each is in (looked up again when it moves into another); one instanced draw puts them into the G-buffer
//! (shader.wgsl `vs_fx`), so the sun, its shadows and the bounce light fall on them as on the terrain. They are
//! not in the traced scene: they cast no shadow. Motion: a `Tween` runs 0 to 1 over a curve, a `Track` maps it
//! onto a value.
use engine::blocks::BLOCKS;
use engine::edit::Predictor;
use engine::events::Event;
use engine::map::{Field, Map};
use engine::palette::Palette;
use engine::world::{Block, World};
use glam::{DVec3, IVec3, Vec3};
use serde::Deserialize;
use std::path::Path;
use std::sync::{Arc, LazyLock};

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
/// how fast it moves (field units a second). With `map` (ADR 0157) the field is that Studio map's
/// (`data/maps/<map>.toml`, a direction): `scale` the metres one tile covers, `speed` times the map's own pace.
#[derive(Deserialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct Noise {
    pub strength: f32,
    #[serde(default = "one")]
    pub scale: f32,
    /// Left out: 0 for the value noise, 1 for a map (`check` puts it).
    #[serde(default = "unset")]
    pub speed: f32,
    #[serde(default)]
    pub map: Option<String>,
}

fn unset() -> f32 {
    f32::NAN
}

/// `[sway]` (ADR 0157): the plants' sway reads `map` (a direction), one tile over `tile` metres of
/// ground; without it the two sines of ADR 0091.
#[derive(Deserialize, Clone, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct Sway {
    pub map: String,
    pub tile: f32,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Ground {
    #[default]
    None,
    Stop,
    Bounce,
    Die,
    /// Bounces, then lies in a cell of the fine lattice (12.5 cm) over its floor, on any chip already there
    /// (ADR 0078).
    Pile,
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

/// The fine cells' edge, m (vision: eight to a block's edge).
pub const FINE: f32 = 0.125;

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
    #[serde(default)]
    sway: Option<Sway>,
}

/// The rows and groups a pool plays, the sway, and the fields of the maps the rows name (`with_fields`: none
/// until a watcher has worked them out); `by_row` each row's field, made by `Fx::swap`.
#[derive(Clone, Debug, Default)]
pub struct Set {
    pub rows: Vec<Row>,
    pub groups: Vec<Group>,
    pub sway: Option<Sway>,
    pub fields: Vec<(String, Arc<Field>)>,
    pub by_row: Vec<Option<Arc<Field>>>,
}

/// The maps compiled in, for a game without `data/maps/` beside it (the friends' zip, the tests): a file there
/// comes first.
const BUILT_IN_MAPS: [(&str, &str); 2] = [("smoke swirl", include_str!("../../../data/maps/smoke swirl.toml")), ("wind gusts", include_str!("../../../data/maps/wind gusts.toml"))];

/// The game's maps' folder (`data/maps/`, the working folder's).
pub fn maps_dir() -> &'static Path {
    Path::new("data/maps")
}

/// The maps compiled in and no folder: what the built-in set (`SET`) is checked against, so a map saved
/// refused in `data/maps/` cannot stop the game starting.
pub fn built_in_maps() -> &'static Path {
    Path::new("")
}

/// Map `name`'s text and the map, from `maps` or else compiled in (only compiled in for `built_in_maps`); a
/// missing or refused map named.
pub fn read_map(maps: &Path, name: &str, palette: &Palette) -> Result<(String, Map), String> {
    let file = maps.join(format!("{name}.toml"));
    let on_disk = (!maps.as_os_str().is_empty()).then(|| std::fs::read_to_string(&file).ok()).flatten();
    let text = on_disk.or_else(|| BUILT_IN_MAPS.iter().find(|m| m.0 == name).map(|m| m.1.to_string())).ok_or(format!("no map '{name}' in {}", maps.display()))?;
    let m = engine::map::parse(&format!("map '{name}'"), &text, palette)?;
    Ok((text, m))
}

/// The fields worked out from map files, by name: the text each was made from and the field.
pub type Kept = std::collections::HashMap<String, (String, Arc<Field>)>;

/// `set` with the fields of the maps its rows name worked out (tens of milliseconds each: a watcher's work, never
/// the frame thread's); a map whose text is as `kept` has it is not worked out again. A map that cannot be read
/// now (changed since the set was checked) gives no field and is named on stderr.
pub fn with_fields(mut set: Set, maps: &Path, palette: &Palette, kept: &mut Kept) -> Set {
    let mut names: Vec<&String> = set.rows.iter().filter_map(|r| r.noise.as_ref()?.map.as_ref()).collect();
    names.sort();
    names.dedup();
    set.fields = names
        .into_iter()
        .filter_map(|n| {
            let (text, m) = read_map(maps, n, palette).map_err(|e| eprintln!("[technoape] effects: {e}")).ok()?;
            if let Some((_, f)) = kept.get(n).filter(|(t, _)| *t == text) {
                return Some((n.clone(), f.clone()));
            }
            let f = Arc::new(Field::new(n, m).map_err(|e| eprintln!("[technoape] effects: {e}")).ok()?);
            kept.insert(n.clone(), (text, f.clone()));
            Some((n.clone(), f))
        })
        .collect();
    set
}

/// The watcher's load: `file` read and checked against the maps in `maps`, its rows' fields worked out.
pub fn load(file: &Path, maps: &Path, palette: &Palette, kept: &mut Kept) -> Result<Set, String> {
    let shown = file.display().to_string().replace('\\', "/");
    let text = std::fs::read_to_string(file).map_err(|_| format!("{shown}: not found"))?;
    let set = parse_set_in(&text, palette, maps).map_err(|e| format!("{shown}: {e}"))?;
    Ok(with_fields(set, maps, palette, kept))
}

/// The plants' wind (`[sway]`): its field, the field as the GPU's texels (two signed bytes a push, made on the
/// watcher's thread so the frame thread only uploads) and the metres one tile of it covers.
#[derive(Clone, Debug)]
pub struct Wind {
    pub field: Arc<Field>,
    pub texels: Arc<Vec<i8>>,
    pub tile: f32,
}

/// The sway watcher's load: `[sway]` of `effects` and the map it names in `maps`; none without `[sway]`. `kept`:
/// the last map's text and wind, so an unchanged map is not worked out again.
pub fn read_sway(effects: &Path, maps: &Path, palette: &Palette, kept: &mut Option<(String, Wind)>) -> Result<Option<Wind>, String> {
    let shown = effects.display().to_string().replace('\\', "/");
    let t: toml::Table = std::fs::read_to_string(effects).map_err(|e| format!("{shown}: {e}"))?.parse().map_err(|e| format!("{shown}: {e}"))?;
    let Some(s) = t.get("sway") else { return Ok(None) };
    let s: Sway = s.clone().try_into().map_err(|e| format!("{shown}: sway: {e}"))?;
    if !(s.tile.is_finite() && s.tile > 0.0) {
        return Err(format!("{shown}: sway: tile {} is not metres over 0", s.tile));
    }
    let (text, m) = read_map(maps, &s.map, palette)?;
    if let Some((_, w)) = kept.as_ref().filter(|k| k.0 == text) {
        return Ok(Some(Wind { tile: s.tile, ..w.clone() }));
    }
    let t0 = std::time::Instant::now();
    let field = Arc::new(Field::new(&s.map, m)?);
    let texels = Arc::new(field.push.iter().flat_map(|p| p.map(|x| (x.clamp(-1.0, 1.0) * 127.0).round() as i8)).collect());
    eprintln!("[technoape] sway: map '{}' worked out in {:.0} ms", s.map, t0.elapsed().as_secs_f64() * 1000.0);
    let w = Wind { field, texels, tile: s.tile };
    *kept = Some((text, w.clone()));
    Ok(Some(w))
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

/// `parse` with the groups and the sway, against a palette and the game's maps (`maps_dir`).
pub fn parse_set(text: &str, palette: &Palette) -> Result<Set, String> {
    parse_set_in(text, palette, maps_dir())
}

/// `parse_set` with the maps a row names read from `maps`. The sway's map is not checked here: a refused one
/// leaves the sines (`gpu.rs` says why on stderr), it does not refuse the rows.
pub fn parse_set_in(text: &str, palette: &Palette, maps: &Path) -> Result<Set, String> {
    let File { effect: mut rows, group: mut groups, sway } = toml::from_str::<File>(text).map_err(|e| e.to_string())?;
    if let Some(s) = &sway {
        if !(s.tile.is_finite() && s.tile > 0.0) {
            return Err(format!("sway: tile {} is not metres over 0", s.tile));
        }
    }
    let names: Vec<String> = rows.iter().map(|r| r.name.clone()).collect();
    for r in rows.iter_mut() {
        if names.iter().filter(|m| **m == r.name).count() > 1 {
            return Err(format!("effect {} twice", r.name));
        }
        check(r, &names, palette, maps)?;
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
    Ok(Set { rows, groups, sway, fields: Vec::new(), by_row: Vec::new() })
}

/// One row's checks; every refusal names it and why. `names`: the rows its `then` may name; `maps`: where the
/// map its noise names is.
fn check(r: &mut Row, names: &[String], palette: &Palette, maps: &Path) -> Result<(), String> {
    let n = r.name.clone();
    let no = |why: &str| Err(format!("effect {n}: {why}"));
    if let Some(z) = &mut r.noise {
        if z.speed.is_nan() {
            z.speed = if z.map.is_some() { 1.0 } else { 0.0 };
        }
    }
    // every number finite, named by its field
    let z = r.noise.as_ref().map_or([0.0; 3], |z| [z.strength, z.scale, z.speed]);
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
    if let Some(z) = &r.noise {
        // (a finer grain than 1 cm overflows `noise_at`'s cell numbers far from the body's centre)
        if !(z.strength >= 0.0 && z.scale >= 0.01 && z.speed >= 0.0) {
            return no("noise is { strength 0 or more, scale 0.01 m or more, speed 0 or more }");
        }
        if let Some(m) = &z.map {
            read_map(maps, m, palette).and_then(|(_, map)| Field::check(m, &map)).map_err(|e| format!("effect {n}: noise: {e}"))?;
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
    check(&mut r, &names, &PALETTE, maps_dir())?;
    Ok(r)
}

/// The built-in set: the file as compiled in, checked against the maps compiled in (a map on disk is the
/// watcher's to read: refused, it is named and this set stays).
pub static SET: LazyLock<Set> = LazyLock::new(built_in_set);
fn built_in_set() -> Set {
    parse_set_in(include_str!("../../../data/effects.toml"), &PALETTE, built_in_maps()).expect("data/effects.toml")
}
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

/// A map row's push at p (body frame, m from the centre) and time t, m/s²: the map laid on the plane of p's
/// dominant axis, one tile `scale` m, and its part along up taken out, so it lies along the ground.
pub fn field_push(p: Vec3, t: f32, n: &Noise, f: &Field) -> Vec3 {
    // shortcut: the field jumps where the dominant axis changes (the cube's edges), a chart of the sphere when that is seen
    let a = p.abs();
    let (i, j) = if a.x >= a.y && a.x >= a.z { (1, 2) } else if a.y >= a.z { (2, 0) } else { (0, 1) };
    let m = f.at(p[i] / n.scale, p[j] / n.scale, t * n.speed);
    let mut e = Vec3::ZERO;
    e[i] = m[0];
    e[j] = m[1];
    let up = p.normalize();
    (e - up * e.dot(up)) * n.strength
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
    /// The cell, for a `block` look: drawn exactly where the terrain draws it (as the GPU counts layers, `gpu_cell`).
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
            cell: Some(gpu_cell(world, b)),
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
    /// Glows without a lamp of its own (`unlit`): a placed block's flame, whose block is the light.
    unlit: bool,
    /// Seconds until it stops by itself (`start_for`); infinite until `stop`.
    left: f32,
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

/// A particle's block in its `FxGpu::info`: the id's low 8 bits at 0-7, its high 4 at 20-23 (shader_sky_fx.wgsl
/// `fx_id`; 12 bits, `engine::blocks::IDS`).
pub(crate) const fn block_bits(id: u16) -> u32 {
    (id as u32 & 255) | (id as u32 >> 8) << 20
}
pub(crate) const BLOCK_BITS: u32 = 255 | 15 << 20;
/// A particle's (a model's, a contact shadow's) body in its `info` (shader_sky_fx.wgsl `fx_body`).
pub(crate) const fn body_bits(body: u8) -> u32 {
    (body as u32) << 8
}
/// A particle's look in its `info` (shader_sky_fx.wgsl `fx_look`).
pub(crate) const fn look_bits(look: u32) -> u32 {
    look << 12
}

/// A particle as the shader reads it (`FxP` in shader.wgsl), 32 bytes: where, its edge now, the block
/// (`block_bits`) | body << 8 (4 bits, `body_bits`) | look << 12 (`look_bits`) | face << 16 | texture << 18 | an offset in cells << 19 | glow << 24,
/// its turn, and `a`, `b` as `P` has them.
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
    /// The planet's wind where the camera is, its frame, m/s (ADR 0076): a particle of the planet with drag
    /// slows toward it rather than to rest, so leaves, smoke and snow drift with the weather.
    pub wind: Vec3,
    pool: Vec<P>,
    out: Vec<FxGpu>,
    /// How many of `out`'s last are contact shadows (`piles::shade`, drawn by `vs_shade`).
    pub shades: usize,
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
    /// The fine cells lying chips hold, kept between steps for its room (ADR 0078).
    held: std::collections::HashSet<FineCell>,
    /// A lying chip ended this step: the next looks for chips left over nothing.
    ended: bool,
}

/// A fine cell over a column's floor: (body, the column's frame, (across, along, cells up)).
type FineCell = (u8, [u32; 6], IVec3);

impl Column {
    /// The column's identity for the fine cells: its whole frame, so no two columns share a key.
    fn key(&self) -> [u32; 6] {
        [self.s.x, self.s.y, self.s.z, self.t.x, self.t.y, self.t.z].map(f32::to_bits)
    }
}

impl P {
    /// The fine cell a lying chip is in.
    fn fine(&self) -> FineCell {
        let k = ((self.p.length() - self.floor) / FINE).floor() as i32;
        (self.body, self.col.key(), IVec3::new((self.p.dot(self.col.s) / FINE).floor() as i32, (self.p.dot(self.col.t) / FINE).floor() as i32, k))
    }

    /// A chip come to rest lies in the fine cell under it, on the chips already there (`held`); over one, it
    /// rolls to a side cell lower down, as the loose blocks slide (ADR 0079), within its column.
    fn pile(&mut self, held: &mut std::collections::HashSet<FineCell>) {
        let (s, t, r) = (self.col.s, self.col.t, self.p.length());
        let (body, frame) = (self.body, self.col.key());
        let level = |held: &std::collections::HashSet<FineCell>, a: i32, b: i32| (0..8).find(|k| !held.contains(&(body, frame, IVec3::new(a, b, *k)))).unwrap_or(8);
        let n = [(self.col.w[0] * r / FINE).floor() as i32, (self.col.w[1] * r / FINE).floor() as i32];
        let inside = |a: i32, b: i32| (-n[0]..n[0]).contains(&a) && (-n[1]..n[1]).contains(&b);
        // the cell under it, kept inside its column (a chip at rest a hair past the edge is over this floor)
        let (mut a, mut b) = (((self.p.dot(s) / FINE).floor() as i32).clamp(-n[0], n[0] - 1), ((self.p.dot(t) / FINE).floor() as i32).clamp(-n[1], n[1] - 1));
        let mut k = level(held, a, b);
        for _ in 0..4 {
            let first = (a + b + k).rem_euclid(4) as usize;
            let lower = (0..4).map(|d| [(1, 0), (-1, 0), (0, 1), (0, -1)][(first + d) % 4]).map(|(da, db)| (a + da, b + db)).filter(|&(a, b)| inside(a, b)).map(|(a, b)| (a, b, level(held, a, b))).find(|c| c.2 < k);
            let Some(c) = lower.filter(|_| k > 0) else { break };
            (a, b, k) = c;
        }
        if k == 8 {
            return; // shortcut: a pile a metre high takes no more; it lies where it fell
        }
        held.insert((body, frame, IVec3::new(a, b, k)));
        // the move along s and t that puts the chip over the cell's middle: s and t need not be square
        let (da, db, st) = ((a as f32 + 0.5) * FINE - self.p.dot(s), (b as f32 + 0.5) * FINE - self.p.dot(t), s.dot(t));
        let (ss, tt) = (s.dot(s), t.dot(t));
        let det = ss * tt - st * st;
        let p = self.p + s * ((da * tt - db * st) / det) + t * ((db * ss - da * st) / det);
        self.p = p.normalize() * (self.floor + FINE * (k as f32 + 0.5));
        (self.size, self.spin, self.angle) = (FINE, 0.0, 0.0);
    }
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
            wind: Vec3::ZERO,
            pool: Vec::with_capacity(cap),
            out: Vec::with_capacity(cap + CRACKS),
            shades: 0,
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
            held: Default::default(),
            ended: false,
        }
    }

    /// Reads `file` again within a second of it changing, on a thread of its own; the frame thread swaps the
    /// new rows in at its next `step`, or names the refusal and keeps the old.
    /// The maps its rows name (`data/maps/`) are watched too: a map saved reaches the rows that name it, its field
    /// worked out on the watcher's thread.
    pub fn watch(&mut self, file: std::path::PathBuf) {
        let pal_file = std::path::PathBuf::from("data/palette.toml");
        let kept = std::cell::RefCell::new(Kept::new());
        let there = file.exists();
        self.rx = Some(crate::gpu::watch("effects", maps_dir().to_path_buf(), vec![file.clone(), pal_file.clone()], move |maps| {
            let palette = std::fs::read_to_string(&pal_file).ok().and_then(|t| Palette::parse(&t).ok()).unwrap_or_else(|| PALETTE.clone());
            let set = load(&file, maps, &palette, &mut kept.borrow_mut());
            let found = file.exists();
            (set, found)
        }));
        // its first load waited for here (loading, not the frame loop): a map row pushes from the first frame, so
        // pictures repeat (ADR 0031); a missing file sends none
        if there {
            if let Some(got) = self.rx.as_ref().and_then(|rx| rx.recv().ok()) {
                self.took(got);
            }
        }
    }

    /// The watcher's load swapped in, or its refusal named (the rows before stay).
    fn took(&mut self, got: Result<Set, String>) {
        match got.and_then(|set| self.swap(set)) {
            Ok(()) => eprintln!("[technoape] effects: {} rows, {} groups read", self.set.rows.len(), self.set.groups.len()),
            Err(e) => eprintln!("[technoape] effects: {e}; the rows before stay"),
        }
    }

    /// A new set of rows: the living particles and lasting effects keep theirs by name (those whose row is gone
    /// go). Refused when it lacks a row the code plays (`id`'s).
    pub fn swap(&mut self, mut set: Set) -> Result<(), String> {
        set.by_row = set.rows.iter().map(|r| r.noise.as_ref()?.map.as_ref().and_then(|m| set.fields.iter().find(|f| f.0 == *m)).map(|f| f.1.clone())).collect();
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
            self.emitters.push(Emitter { handle: h, row: r, at: *at, due: 0.0, unlit: false, left: f32::INFINITY });
        }
        Some(Handle(h))
    }

    /// `start` for `secs` seconds, then it stops by itself (a reaction's steam).
    pub fn start_for(&mut self, name: &str, at: &At, secs: f32) {
        if let Some(h) = self.start(name, at) {
            self.emitters.iter_mut().filter(|e| e.handle == h.0).for_each(|e| e.left = secs);
        }
    }

    /// A lasting effect's new place (it follows a moving part); its particles already out fly on.
    pub fn moved(&mut self, h: Handle, at: &At) {
        self.emitters.iter_mut().filter(|e| e.handle == h.0).for_each(|e| e.at = *at);
    }

    /// A lasting effect's glow gives no lamp from now on, its particles as bright as before (ADR 0117: a placed
    /// torch's flame, whose block already is the light).
    pub fn unlit(&mut self, h: Handle) {
        self.emitters.iter_mut().filter(|e| e.handle == h.0).for_each(|e| e.unlit = true);
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
    /// Particles the pool holds at most, and the Particles setting's scale.
    pub fn cap(&self) -> (usize, f32) {
        (self.cap, self.scale)
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
        // the Earth's pull, not `phys::G`: the effects were tuned by eye before Terra got heavier (2026-11-15), and a
        // row's `gravity` is a share of this
        let g0 = 9.81f32;
        // the file changed: its rows in, or why not
        if let Some(got) = self.rx.as_ref().and_then(|rx| rx.try_iter().last()) {
            self.took(got);
        }
        self.events = 0;
        self.out.clear();
        self.shades = 0;
        self.mout.clear();
        self.cracks = 0;
        self.clock += dt;
        self.emit(world, dt);
        let rows = &self.set.rows;
        let clock = self.clock;
        // each row's map field (`swap` made the list; a row naming a map not worked out pushes nothing)
        let fields = &self.set.by_row;
        // the fine cells the lying chips hold, when a chip may come to lie this step (ADR 0078): one pass
        let piling = |q: &P| rows[q.row as usize].ground == Ground::Pile;
        let ended = std::mem::take(&mut self.ended);
        let mut held = std::mem::take(&mut self.held);
        held.clear();
        if ended || self.pool.iter().any(|q| !q.rest && piling(q)) {
            held.extend(self.pool.iter().filter(|q| q.rest && piling(q)).map(|q| q.fine()));
        }
        // a lying chip ended last step: a chip piled over a cell left empty falls again, and so on up (bottom
        // first), so none hangs on nothing; only the chips `pile` put on the lattice (FINE) are piled
        if ended {
            let mut lying: Vec<(i32, usize)> = (0..self.pool.len()).filter(|&i| self.pool[i].rest && self.pool[i].size == FINE && piling(&self.pool[i])).map(|i| (self.pool[i].fine().2.z, i)).filter(|c| c.0 > 0).collect();
            lying.sort_unstable();
            for (_, i) in lying {
                let f = self.pool[i].fine();
                if !held.contains(&(f.0, f.1, f.2 - IVec3::Z)) {
                    held.remove(&f);
                    self.pool[i].rest = false;
                }
            }
        }
        let mut i = 0;
        while i < self.pool.len() {
            let q = &mut self.pool[i];
            let r = &rows[q.row as usize];
            q.age += dt;
            if !q.rest && q.age < q.life {
                let up = q.p.normalize();
                let air = if q.body == 0 { self.wind } else { Vec3::ZERO };
                q.v += (up * (-g0 * r.gravity) + (air - q.v) * r.drag) * dt;
                // (the rows before ADR 0050 push nothing: their motion stays exactly as it was)
                if let Some(f) = fields.get(q.row as usize).and_then(|f| f.as_deref()) {
                    q.v += (q.push + field_push(q.p, clock, r.noise.as_ref().unwrap(), f)) * dt;
                } else if let Some(n) = r.noise.as_ref().filter(|n| n.map.is_none()) {
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
                        Ground::Bounce | Ground::Pile => {
                            // a third of the fall back up, half of the slide kept; slow, it lies down
                            q.v = (q.v - up * down) * 0.5 - up * down * 0.3;
                            q.spin *= 0.5;
                            if q.v.length() < 0.4 {
                                (q.v, q.rest) = (Vec3::ZERO, true);
                                if r.ground == Ground::Pile && q.col.w[0] > 0.0 {
                                    q.pile(&mut held);
                                }
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
                self.ended |= q.rest && r.ground == Ground::Pile;
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
            let mut info = block_bits(q.block) | body_bits(q.body) | look_bits(gl);
            if gl == 1 {
                // the new looks' bits (ADR 0050): face << 16, a texture's block in the block's place | 1 << 18,
                // glow * 32 << 24; along the flight `a` is its direction and `angle` its streak, a 30th of a second
                info |= (r.face as u32) << 16 | (((r.glow * 32.0).round() as u32).min(255)) << 24;
                if r.look == Look::Texture {
                    info = (info & !BLOCK_BITS) | block_bits(r.tex) | 1 << 18;
                }
                if r.face == Face::Flight {
                    a = pack_dir(q.v);
                    angle = q.v.length() / 30.0;
                }
            }
            self.out.push(FxGpu { p: q.p.to_array(), size, info, angle, a, b });
            i += 1;
        }
        self.held = held;
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
        emitters.retain(|e| e.left > 0.0);
        for e in emitters.iter_mut() {
            e.left -= dt;
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
            if r.glow > 0.0 && !e.unlit {
                let c = if r.grad.is_empty() { r.colour.unwrap_or([1.0; 3]) } else { r.grad.iter().fold([0.0; 3], |s, g| [0, 1, 2].map(|k| s[k] + g.1[k] / r.grad.len() as f32)) };
                let mut c = c.map(|v| v * r.glow * 0.25);
                if let Some(z) = &r.noise {
                    let f = 0.8 + 0.2 * noise_at(Vec3::new(e.handle as f32 * 7.3, 0.0, 0.0), self.clock * 4.0, &Noise { strength: 1.0, scale: 1.0, speed: z.speed.max(1.0), map: None }).x;
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
        let (a, code) = cell_code(gpu_cell(world, b));
        self.out.push(FxGpu { p: [0.0; 3], size: 1.0, info: block_bits(world.block(b)) | body_bits(b.body) | look_bits(Look::Crack as u32), angle: k.clamp(0.0, 1.0), a, b: code });
    }

    /// Debris no step moves, drawn after this step's particles until the next step: the piles on the ground
    /// (ADR 0074).
    pub fn lay(&mut self, still: impl IntoIterator<Item = FxGpu>) {
        self.out.extend(still);
    }

    /// Blocks gliding into their cells (ADR 0101), before every particle: a full buffer is cut at its end, and a
    /// glide cut off would leave a hole where its cell is meshed as air.
    pub fn lay_first(&mut self, glides: impl IntoIterator<Item = FxGpu>) {
        self.out.splice(0..0, glides);
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
pub(crate) fn pack_dir(v: Vec3) -> u32 {
    let d = v.normalize_or(Vec3::Y).to_array().map(|x| ((x * 0.5 + 0.5) * 1023.0).round() as u32);
    d[0] | d[1] << 10 | d[2] << 20
}

/// Cell b as the GPU counts its layers (from the floor of the worlds before the mantle, `frame::gpu_r0`).
pub(crate) fn gpu_cell(world: &World, b: Block) -> Block {
    Block { pos: b.pos - IVec3::Z * world.bodies[b.body as usize].mantle, ..b }
}

/// A cell as the shader's `block` and `crack` looks find it on the grid: column and face.
pub(crate) fn cell_code(c: Block) -> (u32, u32) {
    (c.pos.x as u32 & 0x7fff | (c.pos.y as u32 & 0x7fff) << 15, c.pos.z as u32 & 0xffff | (c.face as u32) << 16)
}

/// Seconds a loose block takes to glide into the cell it moved to (ADR 0101).
pub const GLIDE_S: f64 = 0.2;

/// A loose block gliding into the cell the server moved it to (ADR 0101): drawn as that cell's block, offset
/// toward the cell it left, while the terrain meshes the cell as air (`Streamer::gliding`).
#[derive(Clone, Copy, Debug)]
pub struct Glide {
    pub to: Block,
    pub block: u16,
    /// The offset in cells from `to` at `t0`.
    from: Vec3,
    t0: f64,
}

impl Glide {
    pub fn new(from: Block, to: Block, block: u16, now: f64) -> Glide {
        Glide { to, block, from: (from.pos - to.pos).as_vec3(), t0: now }
    }
    /// The offset in cells from `to` at time `now`, eased in and out.
    pub fn offset(&self, now: f64) -> Vec3 {
        let t = ((now - self.t0) / GLIDE_S).clamp(0.0, 1.0) as f32;
        self.from * (1.0 - t * t * (3.0 - 2.0 * t))
    }
    /// Moved on again from `to` (a fall a cell a tick): from where it is drawn now toward the next cell, at most a
    /// cell behind it, so a long fall never trails (a restarted ease alone settles some six cells back).
    pub fn chain(&mut self, to: Block, now: f64) {
        self.from = (self.offset(now) + (self.to.pos - to.pos).as_vec3()).clamp_length_max(1.0);
        (self.to, self.t0) = (to, now);
    }
    pub fn ended(&self, now: f64) -> bool {
        now >= self.t0 + GLIDE_S
    }
    /// As the GPU draws it: a whole-cell look of `to`, bit 19 saying `p` is the offset in cells (`vs_fx`).
    pub fn gpu(&self, now: f64, mantle: i32) -> FxGpu {
        let (a, b) = cell_code(Block { pos: self.to.pos - IVec3::Z * mantle, ..self.to });
        FxGpu { p: self.offset(now).to_array(), size: 1.0, info: block_bits(self.block) | body_bits(self.to.body) | look_bits(Look::Block as u32) | 1 << 19, angle: 0.0, a, b }
    }
}

/// A cell the client's world changed, with what it was: a solid block gone plays the break, one come the place.
pub fn changed(fx: &mut Fx, world: &World, b: Block, was: u16, now: u16) {
    if !fx.on || was == now {
        return;
    }
    let play = fx.events < EVENTS;
    fx.events += 1;
    if engine::edit::breaks(was, now) {
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

/// What the server said happened to the cells of the edits that follow (`ToClient::Events`, sent before a
/// tick's `Edits`; ADR 0149): a cell an `Edit` event names plays its break or place and is heard
/// (`server_edit`), the cell a `Fall` came to glides in from the one it left, and a cell no event names (the
/// cell a fall left, a joining client's edits) changes without a sound or an effect. An event pairs with the
/// edit that makes its cell what it says (`now`, a fall's block): a stack's lower block leaving a cell is not
/// the upper one arriving in it, later in the same tick.
#[derive(Default)]
pub struct Pending(std::collections::HashMap<Block, Vec<(u16, Then)>>);

/// What a server edit of a cell shows (`Pending`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Then {
    Edit,
    Glide(Block),
}

impl Pending {
    /// Events (a message of them; a tick's may come in several, all before its edits). The server sends every
    /// `Edit` and `Fall` with the edit it pairs with, so none is left over.
    pub fn take(&mut self, events: &[Event]) {
        for e in events {
            match *e {
                Event::Edit { at, now, .. } => self.0.entry(at).or_default().push((now, Then::Edit)),
                Event::Fall { from, to, block } => self.0.entry(to).or_default().push((block, Then::Glide(from))),
                _ => {}
            }
        }
    }
    /// The first of the events for cell `b` that made it `m`, for the server's edit of it to `m`.
    pub fn cell(&mut self, b: Block, m: u16) -> Option<Then> {
        let v = self.0.get_mut(&b)?;
        let t = v.remove(v.iter().position(|e| e.0 == m)?).1;
        if v.is_empty() {
            self.0.remove(&b);
        }
        Some(t)
    }
    /// Events waiting for their edits (none once a tick's edits are in).
    pub fn len(&self) -> usize {
        self.0.values().map(Vec::len).sum()
    }
}

/// The tools that wore out in player `me`'s own hand among a tick's events (ADR 0110): each plays its snap.
pub fn worn(events: &[Event], me: u32) -> Vec<u16> {
    events.iter().filter_map(|e| if let Event::Worn { item, by } = *e { (by == me).then_some(item) } else { None }).collect()
}

/// The server's edit of cell `b` to `m` as the tick's events said (`Pending`): one an `Edit` event named plays
/// and, if `hear` may, reads the cell for its sound (`server_edit`); any other changes the world silently.
/// Whether it shows, what the cell was (when read) and what the events said.
pub fn server_cell(fx: &mut Fx, pred: &mut Predictor, world: &mut World, pending: &mut Pending, b: Block, m: u16, hear: bool) -> (bool, Option<u16>, Option<Then>) {
    let then = pending.cell(b, m);
    let (shows, was) = if then == Some(Then::Edit) { server_edit(fx, pred, world, b, m, hear) } else { (pred.server(world, b, m), None) };
    (shows, was, then)
}

/// What one server `Edits` message did, for the frame to finish (`server_edits`): the cells that show a
/// change (to remesh), the glides to start (from, to, block; ADR 0101), the cells to hear (cell, was, now; at
/// most three a message), and the slides among the glides (one slump's hiss, its loudness and where).
#[derive(Default, Debug, PartialEq)]
pub struct Applied {
    pub shown: Vec<Block>,
    pub glides: Vec<(Block, Block, u16)>,
    pub heard: Vec<(Block, u16, u16)>,
    pub slid: u32,
    pub slid_at: Option<Block>,
}

/// A server `Edits` message applied as the tick's events said (`server_cell` a cell, in order); `hear`: sounds
/// may play (the world is loaded).
pub fn server_edits(fx: &mut Fx, pred: &mut Predictor, world: &mut World, pending: &mut Pending, v: &[(Block, u16)], hear: bool) -> Applied {
    let mut a = Applied::default();
    for &(b, m) in v {
        let ear = hear && a.heard.len() < 3;
        let (shows, was, then) = server_cell(fx, pred, world, pending, b, m, ear);
        if let Some(Then::Glide(from)) = then {
            a.glides.push((from, b, m));
            // a slide, not a fall: a block dropping down a shaft is no slump
            if b.pos.z == from.pos.z {
                a.slid += 1;
                a.slid_at.get_or_insert(b);
            }
        }
        if shows {
            a.shown.push(b);
            if let Some(was) = was.filter(|_| ear) {
                a.heard.push((b, was, m));
            }
        }
    }
    a
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

    /// A cell look (a crack, a gliding block, a pile, a break's cube) on a world with a mantle (ADR 0152) is drawn
    /// where the terrain draws its cell: the GPU counts layers from the old floor, its `r0` there.
    #[test]
    fn a_cell_look_on_a_deep_world_is_drawn_at_its_cell() {
        let w = World::with_grid(1536, 32, 1, None, None, 0.0, 1);
        let b = &w.bodies[0];
        assert!(b.mantle > 0);
        let c = Block { body: 0, face: 2, pos: IVec3::new(5, 7, b.layers - 20) };
        let drawn = |code: u32| crate::frame::gpu_r0(b) * (b.lnq * ((code & 0xffff) as f64 + 0.5)).exp();
        let want = b.layer_r(c.pos.z as f64 + 0.5);
        for (what, code) in [("an effect's", cell_code(At::cell(&w, c, 0).cell.unwrap()).1), ("a crack's or a pile's", cell_code(gpu_cell(&w, c)).1), ("a glide's", Glide::new(c, c, 8, 1.0).gpu(1.0, b.mantle).b)] {
            assert!((drawn(code) - want).abs() < 1e-6, "{what} cell is drawn {} m from its own", drawn(code) - want);
        }
    }

    fn world() -> World {
        World::new(768, 256, 1)
    }

    /// ADR 0101: a loose block's move glides from the cell it left to its new one over `GLIDE_S`, eased, drawn as
    /// its new cell's block (bit 19: `p` is the offset in cells); a move on from its target carries on from where
    /// it is drawn, no jump back.
    #[test]
    fn a_moved_block_glides_into_its_cell_and_a_second_move_carries_on() {
        let c = |x: i32, z: i32| Block { body: 0, face: 2, pos: IVec3::new(x, 7, z) };
        let mut g = Glide::new(c(5, 10), c(6, 10), 8, 1.0);
        assert_eq!(g.offset(1.0), Vec3::new(-1.0, 0.0, 0.0), "it starts where it was");
        let mid = g.offset(1.0 + GLIDE_S / 2.0);
        assert!((mid.x + 0.5).abs() < 1e-5, "half way at half the time: {mid}");
        assert_eq!(g.offset(1.0 + GLIDE_S), Vec3::ZERO);
        let d = g.gpu(1.0 + GLIDE_S / 2.0, 0);
        assert_eq!((d.info & 255, d.info >> 12 & 3, d.info >> 19 & 1), (8, Look::Block as u32, 1));
        assert_eq!((d.a, d.b), cell_code(c(6, 10)), "drawn as its new cell");
        // it falls on from its new cell a quarter of the way in: from about where it is drawn, toward the next
        let at = g.offset(1.05);
        g.chain(c(6, 9), 1.05);
        assert_eq!(g.to, c(6, 9));
        assert!((g.offset(1.05) - (at + Vec3::Z)).length() < 0.4, "a small jump at most: {} against {}", g.offset(1.05), at + Vec3::Z);
        assert!(!g.ended(1.05 + GLIDE_S * 0.9) && g.ended(1.05 + GLIDE_S));
        // a long fall, a cell a tick: never drawn more than a cell behind the cell it is in (the review of 0101)
        for k in 1..30 {
            g.chain(c(6, 9 - k), 1.05 + 0.05 * k as f64);
            assert!(g.offset(1.05 + 0.05 * k as f64).length() <= 1.0 + 1e-5, "tick {k}: {} cells behind", g.offset(1.05 + 0.05 * k as f64).length());
        }
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
            for k in 0..260 {
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

    /// A dug block's chips lie on the fine lattice (ADR 0078): each in a cell of 12.5 cm over the floor, none
    /// in another's cell, piled up where they fall together.
    #[test]
    fn chips_pile_on_the_fine_lattice() {
        let mut w = world();
        let c = surface(&w);
        w.set_block(c, AIR);
        let at = At::cell(&w, c, BLOCKS.id("stone"));
        let mut fx = Fx::new(true, 4096, 1.0, 5);
        for _ in 0..4 {
            fx.play(id("break"), &at);
        }
        for _ in 0..100 {
            fx.step(&w, 1.0 / 60.0);
        }
        let lying: Vec<&P> = fx.pool.iter().filter(|p| p.rest).collect();
        assert!(lying.len() >= 36, "{} of 48 lie", lying.len());
        for p in &lying {
            let over = (p.p.length() - p.floor) / FINE;
            assert!((over - over.floor() - 0.5).abs() < 0.01, "a chip {over} cells over its floor: not in a cell");
        }
        for (k, a) in lying.iter().enumerate() {
            for b in &lying[k + 1..] {
                assert!(a.p.distance(b.p) > 0.1, "two chips in one cell: {} apart", a.p.distance(b.p));
            }
        }
        assert!(lying.iter().any(|p| p.p.length() - p.floor > FINE), "nothing piled on another");
    }

    /// A chip whose support fades first (lives differ) falls on to what is under it: none hangs in the air.
    #[test]
    fn a_piled_chip_falls_when_the_one_under_it_is_gone() {
        let mut w = world();
        let c = surface(&w);
        w.set_block(c, AIR);
        let at = At::cell(&w, c, BLOCKS.id("stone"));
        let mut fx = Fx::new(true, 4096, 1.0, 5);
        for _ in 0..4 {
            fx.play(id("break"), &at);
        }
        for _ in 0..100 {
            fx.step(&w, 1.0 / 60.0);
        }
        assert!(fx.pool.iter().any(|p| p.rest && p.fine().2.z > 0), "a pile to test");
        // the bottom layer goes
        let mut kept = 0;
        for p in fx.pool.iter_mut().filter(|p| p.rest) {
            if p.fine().2.z == 0 {
                p.age = p.life;
            } else {
                kept += 1;
            }
        }
        for _ in 0..40 {
            fx.step(&w, 1.0 / 60.0);
        }
        let lying: Vec<FineCell> = fx.pool.iter().filter(|p| p.rest).map(|p| p.fine()).collect();
        assert!(lying.len() >= kept, "the chips that fell came to lie again: {} of {kept}", lying.len());
        for f in &lying {
            assert!(f.2.z == 0 || lying.contains(&(f.0, f.1, f.2 - IVec3::Z)), "a chip hangs {} cells over its floor on nothing", f.2.z);
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
        assert_eq!((d[0].angle, d[0].info >> 12 & 3, d[0].info & BLOCK_BITS), (0.5, Look::Crack as u32, block_bits(w.block(c))));
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
                    // under its floor (a chip piled on another lies over it: ADR 0078; none hangs, its own test)
                    if p.rest {
                        assert!(r - p.floor < 1.0, "seed {seed}: debris at rest {} over its floor", r - p.floor);
                        let under = w.block_at(w.bodies[0].centre + (p.p * ((p.floor - 0.05) / r)).as_dvec3());
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
        let (seq, cells) = pred.edit(&mut w, &shape, AIR, false, |_, _| false);
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

    /// The event list (ADR 0149): another player's dig plays its break and is heard because the tick's
    /// events said it was a dig, and only then; a fall's two cells and an edit no event names change silently.
    #[test]
    fn a_server_edit_plays_and_is_heard_only_as_the_events_say() {
        let mut w = world();
        let mut pred = Predictor::default();
        let mut fx = Fx::new(true, 4096, 1.0, 1);
        let mut pending = Pending::default();
        let a = surface(&w);
        let cell = |k: i32| {
            let b = Block { pos: a.pos + IVec3::new(3 * k, 0, 0), ..a };
            Block { pos: IVec3::new(b.pos.x, b.pos.y, surface_z(&w, b)), ..b }
        };
        let (dug, quiet, from) = (cell(1), cell(2), cell(3));
        let to = Block { pos: from.pos + IVec3::Z, ..from };
        let was = w.block(dug);
        pending.take(&[Event::Edit { at: dug, was, now: AIR, by: Some(2) }, Event::Fall { from, to, block: was }]);
        // the dig: its break, and the cell read for its sound
        assert_eq!(server_cell(&mut fx, &mut pred, &mut w, &mut pending, dug, AIR, true), (true, Some(was), Some(Then::Edit)));
        assert_eq!(fx.played, 1);
        // no event for it: the world changes, nothing plays, nothing is read
        assert_eq!(server_cell(&mut fx, &mut pred, &mut w, &mut pending, quiet, AIR, true), (true, None, None));
        // the fall: the cell left is silent, the one come to glides in from it
        assert_eq!(server_cell(&mut fx, &mut pred, &mut w, &mut pending, from, AIR, true).2, None);
        assert_eq!(server_cell(&mut fx, &mut pred, &mut w, &mut pending, to, was, true).2, Some(Then::Glide(from)));
        assert_eq!(fx.played, 1, "one break in all");
        assert_eq!(pending.len(), 0, "every event met its edit");
        // a cell placed and dug in one tick: an event each, each met by the edit that made the cell what it says
        pending.take(&[Event::Edit { at: dug, was: AIR, now: was, by: None }, Event::Edit { at: dug, was, now: AIR, by: None }]);
        assert_eq!((pending.cell(dug, was), pending.cell(dug, AIR), pending.cell(dug, AIR)), (Some(Then::Edit), Some(Then::Edit), None));
        // a tool worn out snaps for the player whose hand it was in, not for another
        let ev = [Event::Worn { item: 40, by: 2 }, Event::Worn { item: 41, by: 3 }, Event::Died { by: 2 }];
        assert_eq!((worn(&ev, 2), worn(&ev, 9)), (vec![40], vec![]));
    }

    /// The review of the event list: a falling stack moves its lowest block first and the one above into the cell
    /// it left, later in the same tick (`Falls::step`). Each block arriving glides in from where it was, the cell
    /// left first plays no glide of air, and a slide into a cell a falling block left is one slide.
    #[test]
    fn a_falling_stack_glides_each_block_in_from_where_it_was() {
        let sand = BLOCKS.id("sand");
        let gravel = BLOCKS.id("gravel");
        let w0 = world();
        let g = surface(&w0);
        let up = |k: i32| Block { pos: g.pos + IVec3::Z * k, ..g };
        let side = |k: i32| Block { pos: g.pos + IVec3::new(1, 0, k), ..g };
        // (what the cells hold before; the server's events and edits of one tick; the glides wanted; the slides)
        type Case = (Vec<(Block, u16)>, Vec<Event>, Vec<(Block, u16)>, Vec<(Block, Block, u16)>, u32);
        let fall = |from: Block, to: Block, block: u16| Event::Fall { from, to, block };
        let cases: Vec<(&str, Case)> = vec![
            // two sand over a cell of air: the lower drops into it, the upper into the lower's cell
            ("two", (vec![(up(1), AIR), (up(2), sand), (up(3), sand)], vec![fall(up(2), up(1), sand), fall(up(3), up(2), sand)], vec![(up(2), AIR), (up(1), sand), (up(3), AIR), (up(2), sand)], vec![(up(2), up(1), sand), (up(3), up(2), sand)], 0)),
            // three, the top one gravel
            ("three", (vec![(up(1), AIR), (up(2), sand), (up(3), sand), (up(4), gravel)], vec![fall(up(2), up(1), sand), fall(up(3), up(2), sand), fall(up(4), up(3), gravel)], vec![(up(2), AIR), (up(1), sand), (up(3), AIR), (up(2), sand), (up(4), AIR), (up(3), gravel)], vec![(up(2), up(1), sand), (up(3), up(2), sand), (up(4), up(3), gravel)], 0)),
            // a block drops out of a cell and gravel beside it slides into that cell
            ("slide", (vec![(up(1), AIR), (up(2), sand), (side(2), gravel)], vec![fall(up(2), up(1), sand), fall(side(2), up(2), gravel)], vec![(up(2), AIR), (up(1), sand), (side(2), AIR), (up(2), gravel)], vec![(up(2), up(1), sand), (side(2), up(2), gravel)], 1)),
        ];
        for (name, (before, events, edits, glides, slid)) in cases {
            let mut w = world();
            for &(b, m) in &before {
                w.set_block(b, m);
            }
            let (mut fx, mut pred, mut pending) = (Fx::new(true, 4096, 1.0, 1), Predictor::default(), Pending::default());
            // (in two messages, as a tick's events over `EDITS_PART` come: they pair all the same)
            pending.take(&events[..1]);
            pending.take(&events[1..]);
            let a = server_edits(&mut fx, &mut pred, &mut w, &mut pending, &edits, true);
            assert_eq!(a.glides, glides, "{name}");
            assert_eq!(pending.len(), 0, "{name}: every event met its edit");
            assert_eq!((a.slid, a.slid_at.is_some()), (slid, slid > 0), "{name}");
            assert_eq!((fx.played, a.heard.len()), (0, 0), "{name}: a move breaks nothing and is not heard");
            for &(b, m) in &edits {
                assert_eq!(w.block(b), edits.iter().rev().find(|e| e.0 == b).unwrap().1, "{name}: {b:?} {m}");
            }
        }
    }

    /// ADR 0093: a survival dig of rock leaves its rubble, solid to solid; it breaks (one effect), it does not place.
    #[test]
    fn rock_dug_into_its_rubble_plays_a_break() {
        let w = world();
        let mut fx = Fx::new(true, 4096, 1.0, 1);
        let (granite, rubble) = (BLOCKS.id("granite"), BLOCKS.id("granite rubble"));
        changed(&mut fx, &w, surface(&w), granite, rubble);
        assert!(fx.played == 1 && fx.live() > 0, "the break and its chips");
        changed(&mut fx, &w, surface(&w), BLOCKS.id("basalt"), rubble);
        assert_eq!(fx.played, 1, "another rock's rubble is no dig");
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

    /// The drawn particles with their info in the layout before sixteen bodies (step 6b: body at 8-9, look at
    /// 10-11), so the locks below hold the bytes they always did: the body's two new bits and the look's move are
    /// all that changed.
    fn before_16(d: &[FxGpu]) -> Vec<FxGpu> {
        d.iter().map(|g| {
            let (body, look) = ((g.info >> 8) & 15, (g.info >> 12) & 3);
            assert!(body < 4 && (g.info >> 14) & 3 == 0);
            FxGpu { info: (g.info & !0xff00) | body << 8 | look << 10, ..*g }
        }).collect()
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
                h = engine::world::fnv(h, bytemuck::cast_slice(&before_16(fx.drawn())));
            }
            got.push((name, h));
        }
        println!("{got:?}");
        // break's chips pile since ADR 0078
        let want: [u64; 6] = [17591566305369780750, 12728753821768735219, 10756546070781930058, 5885070126393282328, 9628921545010191082, 7602677355907020114];
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
        let n = Noise { strength: 1.0, scale: 0.5, speed: 1.0, map: None };
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

    /// A row without `map` moves through the value noise exactly as before the game read maps: the drawn bytes of
    /// a fire's flames and smoke over two seconds, hashed, as the code before it made them.
    #[test]
    fn a_noise_row_without_a_map_moves_exactly_as_before() {
        let w = world();
        let at = At::point(0, DVec3::new(0.0, 2000.0, 0.0), DVec3::ZERO, 0.0);
        let rows = "[[effect]]\nname='t flames'\nrate=45\nstart='box'\nbox=[0.4,0.05,0.4]\nspeed=[0.0,0.15]\nup=0.4\ngravity=0.0\nrise=1.2\ndrag=1.5\nlife=[0.45,0.8]\nsize=[0.1,0.2]\nlook='square'\ncolour=[1,0.5,0]\nnoise={ strength=2.5, scale=0.3, speed=1.5 }\nspin=2.0\n\n[[effect]]\nname='t smoke'\nrate=6\nstart='box'\nbox=[0.3,0.1,0.3]\nspeed=[0.05,0.2]\nup=0.6\ngravity=0.0\nrise=0.25\nspread=0.08\ndrag=0.5\nlife=[2.5,4.0]\nsize=[0.1,0.18]\nlook='square'\ncolour=[0.3,0.3,0.3]\nnoise={ strength=0.8, scale=0.8 }\n";
        let mut got = Vec::new();
        for name in ["t flames", "t smoke"] {
            let mut fx = pool_with(rows, 1.0);
            fx.start(name, &at).unwrap();
            let mut h = engine::world::FNV0;
            for _ in 0..120 {
                fx.step(&w, 1.0 / 60.0);
                h = engine::world::fnv(h, bytemuck::cast_slice(&before_16(fx.drawn())));
            }
            got.push(h);
        }
        println!("{got:?}");
        assert_eq!(got, [9210340065840107319u64, 3809945637352739919]);
    }

    /// A folder of its own with `t push.toml`: a slope of a gradient across, so a tile's right half pushes along
    /// +x everywhere (its left half along -x).
    fn maps_dir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("technoape-maps-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(d.join("maps")).unwrap();
        std::fs::write(d.join("maps/t push.toml"), PUSH_MAP.replace("STRENGTH", "10")).unwrap();
        d
    }
    const PUSH_MAP: &str = "size = 16\nframes = 1\n\n[[node]]\nid = \"g\"\nkind = \"gradient\"\nshape = \"across\"\n\n[[node]]\nid = \"s\"\nkind = \"slope\"\nstrength = STRENGTH\nin = \"g\"\n\n[[node]]\nid = \"out\"\nkind = \"out\"\nin = \"s\"\n";
    const PUSHED: &str = "[[effect]]\nname='t pushed'\ncount=1\nlife=[5,5]\nsize=[0.1,0.1]\nlook='square'\ncolour=[1,1,1]\ngravity=0\nnoise={ map='t push', strength=2, scale=1000 }\n";

    /// The CPU cost a particle a step of a map row's push against the value noise's, and of working out the two
    /// shipped fields. `cargo test -p client --profile play --lib noise_cost -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn noise_cost() {
        for name in ["smoke swirl", "wind gusts"] {
            let t = std::time::Instant::now();
            let f = Field::new(name, read_map(super::maps_dir(), name, &PALETTE).unwrap().1).unwrap();
            println!("{name}: worked out in {:.0} ms", t.elapsed().as_secs_f64() * 1000.0);
            std::hint::black_box(f);
        }
        let f = Field::new("smoke swirl", read_map(super::maps_dir(), "smoke swirl", &PALETTE).unwrap().1).unwrap();
        let (old, map) = (Noise { strength: 0.8, scale: 0.8, speed: 0.4, map: None }, Noise { strength: 0.8, scale: 4.0, speed: 1.0, map: Some("smoke swirl".into()) });
        let ps: Vec<Vec3> = (0..4096).map(|k| Vec3::new(37.0 + (k % 64) as f32 * 0.07, 4200.0 + (k / 64) as f32 * 0.05, -812.0 + k as f32 * 0.003)).collect();
        for (label, run) in [("noise_at", &(|p: Vec3, t: f32| noise_at(p, t, &old) * old.strength) as &dyn Fn(Vec3, f32) -> Vec3), ("field_push", &|p, t| field_push(p, t, &map, &f))] {
            let mut best = f64::MAX;
            for rep in 0..20 {
                let t0 = std::time::Instant::now();
                let mut s = Vec3::ZERO;
                for p in &ps {
                    s += run(*p, rep as f32 * 0.016);
                }
                std::hint::black_box(s);
                best = best.min(t0.elapsed().as_secs_f64());
            }
            println!("{label}: {:.1} ns a particle (best of 20 over 4096)", best * 1e9 / ps.len() as f64);
        }
    }

    #[test]
    fn a_row_naming_a_missing_map_is_refused_naming_both() {
        let d = maps_dir("missing");
        let e = parse_set_in(&PUSHED.replace("'t push'", "'no such map'"), &PALETTE, &d.join("maps")).unwrap_err();
        assert!(e.contains("effect t pushed") && e.contains("no such map"), "{e}");
        // a number map is refused as no direction, named
        std::fs::write(d.join("maps/t grey.toml"), "[[node]]\nid = \"g\"\nkind = \"constant\"\n\n[[node]]\nid = \"out\"\nkind = \"out\"\nin = \"g\"\n").unwrap();
        let e = parse_set_in(&PUSHED.replace("'t push'", "'t grey'"), &PALETTE, &d.join("maps")).unwrap_err();
        assert!(e.contains("effect t pushed") && e.contains("'t grey'") && e.contains("Slope or Curl"), "{e}");
        assert!(parse_set_in(PUSHED, &PALETTE, &d.join("maps")).is_ok());
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_particle_in_a_field_that_pushes_one_way_gains_speed_that_way_and_none_up() {
        let d = maps_dir("push");
        let (w, maps) = (world(), d.join("maps"));
        let set = with_fields(parse_set_in(PUSHED, &PALETTE, &maps).unwrap(), &maps, &PALETTE, &mut Default::default());
        assert_eq!(set.fields.len(), 1);
        let mut fx = Fx::new(true, 64, 1.0, 1);
        let mut all = SET.clone();
        all.rows.extend(set.rows);
        all.fields = set.fields;
        fx.swap(all).unwrap();
        // z the dominant axis: the map's u along x, v along y; u = 750 / 1000 lies in the half that pushes +x
        let p = DVec3::new(750.0, 250.0, 5000.0);
        fx.start("t pushed", &At::point(0, p, DVec3::ZERO, 0.0)).unwrap();
        for _ in 0..30 {
            fx.step(&w, 1.0 / 60.0);
        }
        let q = &fx.pool[0];
        let up = q.p.normalize();
        let along = (Vec3::X - up * up.x).normalize();
        assert!(q.v.dot(along) > 0.5 * 2.0 * 0.5 * 0.9, "pushed along +x on the ground: {:?}", q.v);
        assert!(q.v.dot(up).abs() < 1e-4 * q.v.length(), "no push along up: {}", q.v.dot(up));
        let _ = std::fs::remove_dir_all(d);
    }

    /// A map saved refused on disk (here a number map as `smoke swirl`) refuses the file's rows read against that
    /// folder, but the built-in set is checked against the compiled-in maps only: the game still starts.
    #[test]
    fn a_refused_map_on_disk_leaves_the_built_in_set_building() {
        let d = maps_dir("refused");
        let shipped = include_str!("../../../data/maps/smoke swirl.toml");
        std::fs::write(d.join("maps/smoke swirl.toml"), shipped.replace("in = \"curl\"", "in = \"soften\"")).unwrap();
        let e = parse_set_in(include_str!("../../../data/effects.toml"), &PALETTE, &d.join("maps")).unwrap_err();
        assert!(e.contains("effect smoke") && e.contains("smoke swirl"), "{e}");
        let set = parse_set_in(include_str!("../../../data/effects.toml"), &PALETTE, built_in_maps()).unwrap();
        assert!(set.rows.iter().any(|r| r.noise.as_ref().is_some_and(|n| n.map.as_deref() == Some("smoke swirl"))));
        built_in_set();
        let _ = std::fs::remove_dir_all(d);
    }

    /// The fields are in from the first frame: `watch` waits for the watcher's first load (part of loading, not
    /// the frame loop), so a map row pushes from its first step and pictures repeat (ADR 0031).
    #[test]
    fn the_fields_are_in_before_the_first_step() {
        let d = maps_dir("first");
        let file = d.join("effects.toml");
        std::fs::write(&file, include_str!("../../../data/effects.toml")).unwrap();
        let mut fx = Fx::new(true, 64, 1.0, 1);
        fx.watch(file);
        let smoke = fx.set.rows.iter().position(|r| r.name == "smoke").unwrap();
        assert!(fx.set.by_row.get(smoke).is_some_and(|f| f.is_some()), "the smoke's field is in once watch returns");
        let _ = std::fs::remove_dir_all(d);
    }

    /// `[sway] tile` of 0 or under (inf texture coordinates in the shader) is refused by the sway's watcher, named.
    #[test]
    fn the_sways_watcher_refuses_a_tile_not_over_zero() {
        let d = maps_dir("tile");
        let file = d.join("effects.toml");
        let mut kept = None;
        for tile in ["0.0", "-4.0"] {
            std::fs::write(&file, format!("[sway]\nmap = 't push'\ntile = {tile}\n")).unwrap();
            let e = read_sway(&file, &d.join("maps"), &PALETTE, &mut kept).map(|w| w.is_some()).unwrap_err();
            assert!(e.contains("tile"), "{e}");
        }
        std::fs::write(&file, "[sway]\nmap = 't push'\ntile = 48.0\n").unwrap();
        let w = read_sway(&file, &d.join("maps"), &PALETTE, &mut kept).unwrap().unwrap();
        assert_eq!(w.texels.len(), w.field.push.len() * 2);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn the_watchers_load_takes_a_changed_map_file_and_sway() {
        let d = maps_dir("load");
        let file = d.join("effects.toml");
        std::fs::write(&file, format!("[sway]\nmap = 't push'\ntile = 48.0\n\n{PUSHED}")).unwrap();
        let mut kept = Default::default();
        let a = load(&file, &d.join("maps"), &PALETTE, &mut kept).unwrap();
        assert_eq!(a.sway, Some(Sway { map: "t push".into(), tile: 48.0 }));
        let b = load(&file, &d.join("maps"), &PALETTE, &mut kept).unwrap();
        assert!(Arc::ptr_eq(&a.fields[0].1, &b.fields[0].1), "an unchanged map is not worked out again");
        std::fs::write(d.join("maps/t push.toml"), PUSH_MAP.replace("STRENGTH", "1")).unwrap();
        let c = load(&file, &d.join("maps"), &PALETTE, &mut kept).unwrap();
        let (fa, fc) = (&a.fields[0].1, &c.fields[0].1);
        assert!(fc.push[12][0] < fa.push[12][0] * 0.5, "the changed map's weaker push: {:?} {:?}", fc.push[12], fa.push[12]);
        let _ = std::fs::remove_dir_all(d);
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

    /// A boil's steam (liquids.md step 4) is a lasting row (a rate, no count): the server's Boil event plays it for a
    /// while, it is seen, and it stops by itself.
    #[test]
    fn a_boils_steam_is_seen_and_stops_by_itself() {
        let w = world();
        let mut fx = Fx::new(true, 4096, 1.0, 3);
        let at = At::point(0, DVec3::new(0.0, 2000.0, 0.0), DVec3::ZERO, 0.0);
        fx.start_for("steam", &at, 1.5);
        fx.step(&w, 0.5);
        assert!(fx.live() > 0, "steam out");
        for _ in 0..10 {
            fx.step(&w, 1.0);
        }
        assert_eq!(fx.live(), 0, "it stopped by itself");
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
