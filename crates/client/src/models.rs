//! Voxel models in the game (ADR 0047; the format: `engine::models`, docs/design/studio-tools.md "Models").
//! A thread reads and meshes `data/models/` (`models=dir` another folder) and again within a second of a file
//! there changing; the frame thread copies the quads once per change and, each frame, one transform per part
//! of every model drawn: cells whose block names a model, the block in hand, `model=` and `run=models`.
//! Drawn by `vs_model` into the G-buffer with the terrain, lit by `light()` where they stand.
//! States (ADR 0050): a model asked into one (`model=name@state`, `set_state` for a placed cell) plays the
//! state's clip, starts its lasting effects at its points and moves them with the parts, holds its looping sound
//! by the distance to it, gives its light as a lamp, and fires its clip's events once a pass. The particles with
//! a model look (fx.rs) are drawn here too.
use engine::blocks::BLOCKS;
use engine::models::{events_between, mesh, parse_life, Life, Model, Quad, State, VOXEL};
use engine::world::{Block, World};
use glam::{DMat3, DMat4, DVec3, Mat4, Vec3};
use std::sync::mpsc::Receiver;

/// The quads of every model at once, 16 bytes each (1 MB).
pub const MAX_MQUADS: usize = 1 << 16;
/// One transform per part of every model drawn in a frame, 64 bytes each (4 MB). (shortcut: every instance is
/// posed on the frame thread, about 0.3 us each; pose on rayon or the GPU when thousands run at once, ADR 0047)
pub const MAX_PIECES: usize = 1 << 16;
/// The middle of a cell's floor, in its metres: where a model stands and is scaled about.
const FLOOR: Vec3 = Vec3::new(0.5, 0.0, 0.5);
/// Cells with a model block are drawn this near the camera, metres.
const NEAR_M: f64 = 128.0;

/// A part drawn: its transform from its voxels to the camera's frame (3 rows), its first quad, its body.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Piece {
    pub x: [[f32; 4]; 3],
    pub q0: u32,
    pub body: u32,
    pub _p: [u32; 2],
}

/// A model read and meshed off the frame thread: its quads per part, packed as `vs_model` reads them.
#[derive(Clone)]
pub struct Loaded {
    pub name: String,
    pub model: Model,
    /// Its states and its clips' events.
    pub life: Life,
    pub quads: Vec<Vec<[u32; 4]>>,
    /// How large it is drawn, about its cell's floor centre: a creature's `scale` (ADR 0053), else 1.
    pub scale: f32,
    /// Its painted boxes as quads of a pixel (ADR 0114), after its parts' in `quads`: each box's part and its
    /// `Bx::frame`.
    pub boxes: Vec<(usize, Mat4)>,
    /// Where it is at rest in its cell's metres, the lowest corner and the highest (what a pile lays down).
    pub bounds: [Vec3; 2],
    /// Its pictures for the inventory (ADR 0113): a slot's, and the ape's large one (`FIGURE` only).
    pub pictures: [Option<crate::sprite::Sprite>; 2],
}

impl Loaded {
    /// How it lies in a pile (ADR 0114): its scale (the item's size, a model of painted boxes at its real size
    /// unless its file says, a tool `TOOL_LYING` times), its turn (an item standing taller than it is wide laid
    /// down on its face, +z up) and its bounds so turned.
    pub fn lying(&self, item: bool, tool: bool) -> (f32, Mat4, [Vec3; 2]) {
        let s = self.model.item.unwrap_or(if !self.painted_item() { engine::models::ITEM_M } else { 1.0 }) * if tool { TOOL_LYING } else { 1.0 };
        let [lo, hi] = self.bounds;
        let d = hi - lo;
        let lay = if item && d.y > d.x.max(d.z) { Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_2) } else { Mat4::IDENTITY };
        let (a, b) = (lay.transform_point3(lo), lay.transform_point3(hi));
        (s, lay, [a.min(b), a.max(b)])
    }

    /// How it sits in its cell in hand: a painted model (clutter, painted items) by its rest bounds' middle in the
    /// cell's middle (a flint lies low in its cell, under the screen's edge), a flat one tipped 60 degrees to show
    /// the eye its top; a voxel model (a creature, a block's) as before, by its cell (ADR 0114).
    pub fn held(&self) -> Mat4 {
        if !self.painted_item() {
            return Mat4::IDENTITY;
        }
        let [lo, hi] = self.bounds;
        let d = hi - lo;
        // flat: under 0.4 of its width (flint 0.34, the stick 0.15; the loose stone, a lump, 0.49 is not)
        let tip = if d.y < 0.4 * d.x.max(d.z) { Mat4::from_rotation_x(60f32.to_radians()) } else { Mat4::IDENTITY };
        Mat4::from_translation(Vec3::splat(0.5) - tip.transform_point3((lo + hi) * 0.5)) * tip
    }

    /// A painted item (clutter, a hand axe): painted boxes in one rigid part, held, laid and pictured by its
    /// bounds (ADR 0114). A figure of several parts (a creature, the furnace) is so by its cell as a voxel model
    /// is, painted or not (ADR 0116).
    pub fn painted_item(&self) -> bool {
        !self.boxes.is_empty() && self.model.parts.len() == 1
    }

    /// A model meshed: its parts' voxels, then its painted boxes from `paint` (its `.png`; none, no boxes drawn).
    pub fn new(name: String, model: Model, life: Life, paint: Option<&engine::models::Painting>) -> Loaded {
        let mut quads: Vec<Vec<[u32; 4]>> = model.parts.iter().map(|p| mesh(p).iter().map(|q| pack(q, model.rgb[q.colour as usize])).collect()).collect();
        let mut boxes = Vec::new();
        for (i, p) in model.parts.iter().enumerate() {
            for b in p.boxes.iter().filter(|_| paint.is_some()) {
                quads.push(b.pixel_quads(paint.unwrap()).iter().map(|(q, c)| pack(q, *c)).collect());
                boxes.push((i, b.frame()));
            }
        }
        // the corners of every quad's cell at rest
        let rest = model.pose(None, 0.0);
        let mut bounds = [Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)];
        for (k, qs) in quads.iter().enumerate() {
            let m = if k < rest.len() { rest[k] } else { rest[boxes[k - rest.len()].0] * boxes[k - rest.len()].1 };
            for q in qs {
                let at = Vec3::new((q[0] & 63) as f32, (q[0] >> 6 & 63) as f32, (q[0] >> 12 & 63) as f32);
                // (as `vs_model` spans it: one voxel deep, w and h along the next two axes)
                let a = (q[0] >> 18 & 7) as usize / 2;
                let mut ext = Vec3::ONE;
                (ext[(a + 1) % 3], ext[(a + 2) % 3]) = ((q[1] & 63) as f32, (q[1] >> 6 & 63) as f32);
                for c in 0..8 {
                    let p = m.transform_point3(at + ext * Vec3::new((c & 1) as f32, (c >> 1 & 1) as f32, (c >> 2) as f32));
                    bounds = [bounds[0].min(p), bounds[1].max(p)];
                }
            }
        }
        if bounds[0].x > bounds[1].x {
            bounds = [Vec3::ZERO; 2];
        }
        Loaded { name, model, life, quads, scale: 1.0, boxes, bounds, pictures: [None, None] }
    }
}

/// What is shown in hand at the lower right (ADR 0115): the item or block by name (drawn when a model has its
/// name), or with `None` the bare arm of the `FIGURE`; `swing` how far through its swing, 0 at rest, 1 at its
/// peak.
#[derive(Clone, Copy, Debug)]
pub struct Hand<'a> {
    pub item: Option<&'a str>,
    pub swing: f32,
}

/// One swing of the hand (ADR 0115): this long, seconds; as long as a survival dig's hit, so a held dig swings on.
pub const SWING_S: f64 = 0.25;

/// How far through its swing the hand is `age` seconds after a dig, place, strike or pick-up: 0 at rest and at
/// its end, 1 once, early (a quick strike, an eased return).
pub fn swing(age: f64) -> f32 {
    let x = age / SWING_S;
    if !(0.0..1.0).contains(&x) {
        return 0.0;
    }
    (std::f64::consts::PI * (1.0 - (1.0 - x) * (1.0 - x))).sin() as f32
}

/// The bare arm in hand (ADR 0115), in the eye's frame (metres ahead, right, up): where its hand's end is, and
/// the way from there to the shoulder, below and right, out of the view. The developer's call; tuned by eye.
const ARM_HAND: [f64; 3] = [0.55, 0.30, -0.30];
const ARM_WAY: [f64; 3] = [-0.75, 0.5, -0.45];
/// The figure's part shown as the bare arm (ape.toml's `arm_r`), with any part hung from it.
const ARM: &str = "arm_r";
/// The swing at its peak: the hand moved this share of the way to a point `SWING_AIM` times its own distance
/// ahead on the look, and chopped down about itself, radians (turned down about the shoulder, the arm left the
/// view).
const SWING_REACH: f64 = 0.45;
const SWING_AIM: f64 = 1.4;
const SWING_CHOP: f64 = 0.35;

/// The model the inventory shows large, standing, and the pixels of its picture and of a slot's.
pub const FIGURE: &str = "ape";

/// A tool lying in a pile is drawn this many times its item size, so it is found in the grass (the developer's
/// call of 2026-11-14: "make dropped tools larger"); in hand and in slots it keeps its size. A footprint that
/// would then reach past its cell is shrunk to fit it (`Lying::fit`, `piles::lay`).
pub const TOOL_LYING: f32 = 2.0;
pub const FIGURE_PX: u16 = 176;
pub const ICON_PX: u16 = 40;

/// A quad as `vs_model` reads it: corner x | y << 6 | z << 12 | dir << 18; w | h << 6; rgba8.
pub fn pack(q: &Quad, rgb: [u8; 3]) -> [u32; 4] {
    [
        q.at[0] as u32 | (q.at[1] as u32) << 6 | (q.at[2] as u32) << 12 | (q.dir as u32) << 18,
        q.w as u32 | (q.h as u32) << 6,
        u32::from_le_bytes([rgb[0], rgb[1], rgb[2], 255]),
        0,
    ]
}

/// Every model in `dir`, read against `data/palette.toml` and meshed, its states against the effects file
/// `effects`; a refused file is named on stderr and not drawn. (shortcut: a change to the palette alone is seen
/// at the next change in `dir`; watch it too when the Studio edits palettes live)
pub fn load(dir: &std::path::Path, effects: &std::path::Path) -> Vec<Loaded> {
    load_keeping(dir, effects, &crate::audio::rows_file(), &mut Default::default())
}

/// `load` for a running game: a file refused now that loaded before keeps its last good model (still named).
/// The effects and sounds a state may name: those of the files in use (`effects=`, `sound_dir=`) as they are
/// now, else the built-in ones.
fn known(effects: &std::path::Path, sounds: &std::path::Path, palette: &engine::palette::Palette) -> (crate::fx::Set, Vec<String>) {
    let fx = std::fs::read_to_string(effects).ok().and_then(|t| crate::fx::parse_set(&t, palette).ok()).unwrap_or_else(|| crate::fx::SET.clone());
    let sounds = std::fs::read_to_string(sounds).ok().and_then(|t| crate::audio::parse(&t).ok()).or_else(|| crate::audio::parse(crate::audio::ROWS_TOML).ok()).unwrap_or_default().into_iter().map(|r| r.name).collect();
    (fx, sounds)
}

pub fn load_keeping(dir: &std::path::Path, effects: &std::path::Path, sounds: &std::path::Path, kept: &mut std::collections::BTreeMap<String, Loaded>) -> Vec<Loaded> {
    let palette = match std::fs::read_to_string("data/palette.toml").map_err(|e| e.to_string()).and_then(|t| engine::palette::Palette::parse(&t)) {
        Ok(p) => p,
        Err(e) => {
            // the last good models of the files still there, as a refused file keeps its own
            eprintln!("[technoape] models: data/palette.toml: {e}; the models before stay");
            return kept.values().filter(|l| dir.join(format!("{}.toml", l.name)).exists()).cloned().collect();
        }
    };
    let mut files: Vec<_> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "toml")).collect();
    files.sort();
    let (fx, sounds) = known(effects, sounds, &palette);
    let (effect, sound) = (|n: &str| fx.rows_of(n).is_some(), |n: &str| sounds.iter().any(|s| s == n));
    let mut out = Vec::new();
    for f in files {
        let name = f.file_stem().unwrap().to_string_lossy().to_string();
        let shown = f.display().to_string().replace('\\', "/");
        // its painting, for its pictures and (a model of painted boxes, ADR 0081) its boxes as the clutter reads it:
        // a painted model whose painting is missing or refused is refused as a refused file is (its last good stays)
        let png = || std::fs::read(dir.join(format!("{name}.png"))).ok();
        let read = std::fs::read_to_string(&f).map_err(|e| format!("{shown}: {e}")).and_then(|t| parse_life(&shown, &t, &palette, &effect, &sound));
        match read.and_then(|(m, l)| painting_of(&m, png(), &palette).map_err(|e| format!("{}: {e}", shown.replace(".toml", ".png"))).map(|p| (m, l, p))) {
            Ok((model, life, png)) => {
                let mut l = Loaded::new(name, model, life, png.as_ref());
                l.pictures = pictures(&l, png.as_ref());
                kept.insert(l.name.clone(), l.clone());
                out.push(l);
            }
            Err(e) => match kept.get(&name) {
                Some(l) => {
                    eprintln!("[technoape] models: {e}; its last good model stays");
                    out.push(l.clone());
                }
                None => eprintln!("[technoape] models: {e}; not drawn"),
            },
        }
    }
    kept.retain(|n, _| out.iter().any(|l| &l.name == n));
    out
}

/// `creature=name[:speed]`: the row of `data/creatures.toml` (checked against the loaded models) makes its model's
/// clip `walk` at `speed` m/s (the row's walk speed when none; 0 stands idle), in place of one the file keys.
/// (shortcut: a change to data/creatures.toml alone is seen at the next change of the models folder)
fn creature_walk(list: &mut [Loaded], spec: &str, text: &str) {
    let (name, speed) = spec.split_once(':').map_or((spec, None), |(n, s)| (n, s.parse::<f32>().ok()));
    let rows = match engine::creatures::parse("data/creatures.toml", text, &|m| list.iter().find(|l| l.name == m).map(|l| l.model.clone()), &|_| true) {
        Ok(r) => r,
        Err(e) => return eprintln!("[technoape] creatures: {e}"),
    };
    let Some(c) = rows.iter().find(|c| c.name == name) else { return eprintln!("[technoape] creatures: no creature '{name}'") };
    if let Some(l) = list.iter_mut().find(|l| l.name == c.model) {
        walk_into(l, c, speed);
    }
}
/// The creature's walk at `speed` (its row's when none) as its model's clip `walk`, and its scale.
fn walk_into(l: &mut Loaded, c: &engine::creatures::Creature, speed: Option<f32>) {
    let speed = speed.unwrap_or(c.walk_speed);
    let mut clip = engine::creatures::walk(&l.model, c, speed);
    clip.name = "walk".into();
    l.model.clips.retain(|k| k.name != "walk");
    l.model.clips.push(clip);
    // its footfalls play its step sound, as any clip's events do
    l.life.events.retain(|(n, _)| n != "walk");
    l.life.events.push(("walk".into(), engine::creatures::steps(c, speed)));
    l.scale = c.scale;
}

/// An animal to draw (ADR 0083): its model, what it does (`herds::Mind` as a number), its feet and look in
/// body 0's frame, its row's scale.
pub struct Beast {
    pub id: u32,
    pub model: String,
    pub act: u8,
    pub pos: DVec3,
    pub fwd: DVec3,
    pub scale: f32,
}

/// The clip each `herds::Mind` plays, by its number: stand, graze, wander, flee, sleep.
pub const ACT_CLIPS: [&str; 5] = ["a.idle", "a.eat", "a.walk", "a.run", "a.idle"];

/// Every creature row's clips for live animals on its model (ADR 0083): `a.idle`, `a.eat`, `a.walk` at its walk
/// speed and `a.run` at its run speed. A row the models refuse is skipped (its animals stand unanimated).
fn animal_clips(list: &mut [Loaded], text: &str) {
    for c in engine::creatures::rows(text).unwrap_or_default() {
        let Some(l) = list.iter_mut().find(|l| l.name == c.model) else { continue };
        if c.check(Some(&l.model), &|_| true).is_err() {
            continue;
        }
        let mut clips = vec![engine::creatures::idle(&l.model, &c), engine::creatures::act(&l.model, &c, "eat"), engine::creatures::walk(&l.model, &c, c.walk_speed), engine::creatures::walk(&l.model, &c, c.run_speed)];
        for (k, n) in clips.iter_mut().zip(["a.idle", "a.eat", "a.walk", "a.run"]) {
            k.name = n.into();
        }
        l.model.clips.retain(|k| !k.name.starts_with("a."));
        l.model.clips.extend(clips);
    }
}

/// One draw: a part's quads for a run of pieces.
pub struct Draw {
    pub quads: u32,
    pub first: u32,
    pub count: u32,
}

/// A model in a state: which, since when (the frame clock), the clip's time it read last, its lasting effects
/// with their points.
struct Run {
    model: String,
    state: String,
    start: f64,
    prev: f64,
    fx: Vec<(crate::fx::Handle, String)>,
}

pub struct Models {
    rx: Receiver<Vec<Loaded>>,
    /// `creature=` or the last `/spawn`: whose walk the watcher gives its model at each reading.
    creature: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    /// The models as the GPU has them: each with its parts' first quad and count.
    pub list: Vec<(Loaded, Vec<(u32, u32)>)>,
    /// `model=name[:clip|@state]` and run=models' count: what stands ahead of the camera, and where once placed.
    spec: Option<(String, Option<String>, usize)>,
    spec_state: Option<String>,
    /// The Studio's view: the shown model's scale (a creature's, ADR 0053), else its own.
    spec_scale: Option<f32>,
    /// The Studio's view: where the shown model stands from its cell and how it is turned (`show_place`).
    spec_place: Mat4,
    /// The body the bare arm and the held item pass through on their way to the sun and the lamps (ADR 0119):
    /// the camera's own player's place in the proxy list plus one, 0 none.
    pub hand: u32,
    /// The state "" a clip with events plays in without a state: that clip and nothing else.
    clip_state: State,
    /// Placed model cells asked into a state (`set_state`), and the states playing.
    states: std::collections::HashMap<Block, String>,
    runs: std::collections::HashMap<Block, Run>,
    /// The looping sounds held last frame, each sound's reach, and this frame's state lights (body, place, light).
    held: Vec<String>,
    reach: std::collections::HashMap<String, f32>,
    lamps: Vec<(u8, DVec3, [f32; 3])>,
    cells: Vec<Block>,
    pieces: Vec<Piece>,
    pub draws: Vec<Draw>,
    /// This frame's stacks lying in piles as models (`piles::lay`, ADR 0114).
    pub piles: Vec<crate::piles::Lying>,
    /// `take` swapped in new models since the last frame.
    reloaded: bool,
    /// Counts the sets of models taken: what a kept look (`hud::Looks`) was made from.
    pub gen: u64,
}

impl Models {
    pub fn new(a: &crate::Args) -> Models {
        let dir = std::path::PathBuf::from(a.s("models", "data/models"));
        // `creature=name[:speed]` (ADR 0053): that creature's model takes the walk its row makes, as clip `walk`
        let creature = std::sync::Arc::new(std::sync::Mutex::new(a.0.get("creature").cloned()));
        let spawned = creature.clone();
        let kept = std::cell::RefCell::new(Default::default());
        let (effects, sounds) = (std::path::PathBuf::from(a.s("effects", "data/effects.toml")), crate::audio::rows_file());
        // a state is checked against the effects and sounds files: a change to either (or to the creatures) reads
        // the models again, so a model refused for an effect it names loads once the effect is there
        let also = vec![effects.clone(), sounds.clone(), "data/creatures.toml".into()];
        let rx = crate::gpu::watch("models", dir, also, move |d| {
            let mut l = load_keeping(d, &effects, &sounds, &mut kept.borrow_mut());
            // (the spec is taken out first: the lock is never held over the reading, nor a panic here left in it)
            let spec = creature.lock().ok().and_then(|g| g.clone());
            let rows = std::fs::read_to_string("data/creatures.toml").unwrap_or_default();
            if let Some(spec) = &spec {
                creature_walk(&mut l, spec, &rows);
            }
            animal_clips(&mut l, &rows);
            let any = !l.is_empty();
            (l, any)
        });
        let n = if a.s("run", "stand") == "models" { a.get("n", 100usize) } else { a.0.contains_key("model") as usize };
        let spec = a.s("model", "furnace");
        let (spec, state) = spec.split_once('@').map_or((spec.clone(), None), |(n, s)| (n.to_string(), Some(s.to_string())));
        let (name, clip) = spec.split_once(':').map_or((spec.clone(), None), |(n, c)| (n.to_string(), Some(c.to_string())));
        // the reach of the rows in use (`sound_dir=`), else the built-in ones (shortcut: read once at start; a
        // reloaded sounds file's new reach is used from the next run)
        let rows = std::fs::read_to_string(crate::audio::rows_file()).ok().and_then(|t| crate::audio::parse(&t).ok());
        let reach = rows.or_else(|| crate::audio::parse(crate::audio::ROWS_TOML).ok()).unwrap_or_default().into_iter().map(|r| (r.name, r.reach)).collect();
        Models {
            rx,
            creature: spawned,
            list: Vec::new(),
            clip_state: State { clip: clip.clone(), ..State::default() },
            spec: (n > 0).then_some((name, clip, n)),
            spec_state: state,
            spec_scale: None,
            spec_place: Mat4::IDENTITY,
            hand: 0,
            states: Default::default(),
            runs: Default::default(),
            held: Vec::new(),
            reach,
            lamps: Vec::new(),
            cells: Vec::new(),
            pieces: Vec::new(),
            draws: Vec::new(),
            piles: Vec::new(),
            reloaded: false,
            gen: 0,
        }
    }

    /// Puts the model in cell `b` into a state (None: back to `idle`), from the next frame.
    pub fn set_state(&mut self, b: Block, state: Option<&str>) {
        match state {
            Some(s) => self.states.insert(b, s.to_string()),
            None => self.states.remove(&b),
        };
    }

    /// The Studio's view: the shown model drawn at `scale` (set after `show`).
    pub fn show_scale(&mut self, scale: f32) {
        self.spec_scale = Some(scale);
    }

    /// The Studio's view: the shown model stands `at` metres from its cell along the cell's x and z, turned by
    /// `yaw` about its floor centre (set after `show`; the Fauna page's walk in a circle).
    pub fn show_place(&mut self, at: [f32; 2], yaw: f32) {
        self.spec_place = Mat4::from_translation(Vec3::new(at[0], 0.0, at[1]) + FLOOR) * Mat4::from_rotation_y(yaw) * Mat4::from_translation(-FLOOR);
    }

    /// The Studio's view: the shown model's state (None: its clip or `idle`).
    pub fn show_state(&mut self, state: Option<&str>) {
        self.spec_state = state.map(str::to_string);
    }

    /// This frame's lights of the models in a state: (body, place in the world, light).
    pub fn lamps(&self) -> &[(u8, DVec3, [f32; 3])] {
        &self.lamps
    }

    /// The Studio's view (ADR 0049): models come from the returned sender, no longer from the folder's watcher.
    pub fn feed(&mut self) -> std::sync::mpsc::Sender<Vec<Loaded>> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.rx = rx;
        tx
    }

    /// The Studio's view: one model (and its clip) in `cell`, or none.
    pub fn show(&mut self, spec: Option<(String, Option<String>)>, cell: Block) {
        self.spec_scale = None;
        self.spec_place = Mat4::IDENTITY;
        self.clip_state.clip = spec.as_ref().and_then(|s| s.1.clone());
        self.spec = spec.map(|(n, c)| (n, c, 1));
        self.cells = vec![cell];
    }

    /// `/spawn name[:speed]` in a running game: that creature of `data/creatures.toml` walking in place in
    /// `cell`; `/spawn` alone takes it away. A preview for the developer (ADR 0053): this player's eyes only,
    /// nothing lives in the world yet. The answer, or what is wrong, for the chat.
    /// shortcut: the creatures file is read here on the frame thread (small, on a typed command, as the
    /// server's /structure); and a reading of the models already on its way when this runs brings the model
    /// back without the walk until the next save: /spawn again. Both go when creatures are the server's.
    pub fn spawn(&mut self, spec: &str, cell: Block) -> Result<String, String> {
        let text = std::fs::read_to_string("data/creatures.toml").map_err(|e| format!("data/creatures.toml: {e}"))?;
        self.spawn_from(spec, &text, cell)
    }
    pub(crate) fn spawn_from(&mut self, spec: &str, text: &str, cell: Block) -> Result<String, String> {
        let set = |me: &Self, v: Option<String>| {
            if let Ok(mut g) = me.creature.lock() {
                *g = v;
            }
        };
        if spec.is_empty() {
            set(self, None);
            self.show(None, cell);
            return Ok("no creature stands there now".into());
        }
        let (name, speed) = spec.split_once(':').map_or((spec, None), |(n, s)| (n, Some(s)));
        let speed = speed.map(|s| s.parse::<f32>().ok().filter(|v| v.is_finite() && *v >= 0.0).ok_or("usage: /spawn <creature>[:speed]   the speed after ':' is metres a second")).transpose()?;
        let rows = engine::creatures::parse("data/creatures.toml", text, &|m| self.list.iter().find(|(l, _)| l.name == m).map(|(l, _)| l.model.clone()), &|_| true)?;
        let Some(c) = rows.iter().find(|c| c.name == name) else {
            return Err(format!("no creature '{name}'. There are: {}", rows.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", ")));
        };
        let Some((l, _)) = self.list.iter_mut().find(|(l, _)| l.name == c.model) else { return Err(format!("{name}'s model '{}' is not loaded", c.model)) };
        walk_into(l, c, speed);
        set(self, Some(spec.to_string()));
        self.show(Some((c.model.clone(), Some("walk".into()))), cell);
        Ok(format!("{name} walks in place three blocks ahead of you: a preview, for your eyes only. /spawn alone takes it away"))
    }

    /// A model's picture by its name: 0 a slot's, 1 the ape's large one (ADR 0113).
    pub fn picture(&self, name: &str, k: usize) -> Option<&crate::sprite::Sprite> {
        drawn(&self.list, name).and_then(|m| self.list[m].0.pictures[k].as_ref())
    }

    /// The half width in metres of model `name`'s footprint lying in a pile, whichever way it is turned (its
    /// contact shadow's, `piles::shade`); None when no model of that name has anything to draw.
    pub fn lying_half(&self, name: &str, item: bool, tool: bool) -> Option<f32> {
        let (s, _, [a, b]) = self.list[drawn(&self.list, name)?].0.lying(item, tool);
        Some(0.5 * s * (b.x - a.x).hypot(b.z - a.z))
    }

    /// Whether a model of this name is loaded with something to draw (ADR 0114: a painted one whose painting is
    /// missing or refused has none): the one test the hand, a pile and a slot's picture make.
    pub fn has(&self, name: &str) -> bool {
        drawn(&self.list, name).is_some()
    }

    /// Swaps in the watcher's newest models, if it sent any: one copy of their quads.
    pub fn take(&mut self, queue: &wgpu::Queue, buf: &wgpu::Buffer) {
        let Some(set) = self.rx.try_iter().last() else { return };
        let mut quads: Vec<[u32; 4]> = Vec::new();
        self.list.clear();
        for l in set {
            let n: usize = l.quads.iter().map(Vec::len).sum();
            if quads.len() + n > MAX_MQUADS {
                eprintln!("[technoape] models: {} not drawn, more than {MAX_MQUADS} quads in all", l.name);
                continue;
            }
            let parts = l.quads.iter().map(|q| {
                let at = (quads.len() as u32, q.len() as u32);
                quads.extend_from_slice(q);
                at
            });
            let parts = parts.collect();
            self.list.push((l, parts));
        }
        if !quads.is_empty() {
            queue.write_buffer(buf, 0, bytemuck::cast_slice(&quads));
        }
        self.reloaded = true;
        self.gen += 1;
        eprintln!("[technoape] models: {} loaded, {} quads", self.list.len(), quads.len());
    }

    /// This frame's pieces and draws, uploaded. `cams`: per body the camera there and the turn into the
    /// camera's frame (frame.rs); `eye`: the camera's place, look and up in body `cf`'s frame; `hand`: what is
    /// held at the lower right, None when nothing is shown there (ADR 0115).
    #[allow(clippy::too_many_arguments)]
    pub fn frame(&mut self, world: &World, cams: &[(DVec3, DMat3)], cf: usize, eye: (DVec3, DVec3, DVec3), t: f64, hand: Option<Hand>, herd: &[Beast], fx: &mut crate::fx::Fx, audio: &mut crate::audio::Audio) {
        self.pieces.clear();
        self.draws.clear();
        self.lamps.clear();
        // new models: the states' lasting effects start again from the files as they are now
        if std::mem::take(&mut self.reloaded) || self.list.is_empty() {
            for r in self.runs.values_mut() {
                r.fx.drain(..).for_each(|(h, _)| fx.stop(h));
            }
        }
        if self.list.is_empty() {
            // none left (every file refused): nothing plays on unseen
            self.runs.clear();
            self.held.drain(..).for_each(|n| audio.hold(&n, 0.0));
            return;
        }
        // the cells ahead for model= and run=models, once: a widening fan, 1.5 m apart, 3 m out at first
        if let (Some((_, _, n)), true) = (&self.spec, self.cells.is_empty()) {
            let pl = &world.bodies[0];
            let up = (eye.0 - pl.centre).normalize();
            let ahead = (eye.1 - up * eye.1.dot(up)).normalize_or(DVec3::X);
            let side = up.cross(ahead);
            let mut a = 3.0;
            while self.cells.len() < *n {
                let mut b = -(0.6 * a / 1.5f64).floor() * 1.5;
                while b <= 0.6 * a && self.cells.len() < *n {
                    let d = (eye.0 + ahead * a + side * b - pl.centre).normalize();
                    self.cells.push(world.air_over_ground(0, d));
                    b += 1.5;
                }
                a += 1.5;
            }        }
        // the models asked into a state: (cell, model, state, clock)
        let by_name = |n: &str| self.list.iter().position(|(l, _)| l.name == n);
        // (the states borrowed, never a String per cell per frame)
        let mut asked: Vec<(Block, usize, &str, f64)> = Vec::new();
        if let (Some((name, _, _)), Some(st)) = (&self.spec, &self.spec_state) {
            if let Some(m) = by_name(name) {
                asked.extend(self.cells.iter().enumerate().map(|(k, b)| (*b, m, st.as_str(), t + k as f64 * 0.37)));
            }
        }
        // a clip with events and no state (`model=boar:walk` of a creature): its events fire, as the state "" whose
        // clip it is; the clock as without a state, so the drawing is unchanged
        if let (Some((name, Some(clip), _)), None) = (&self.spec, &self.spec_state) {
            if let Some(m) = by_name(name).filter(|m| self.list[*m].0.life.events.iter().any(|(n, e)| n == clip && !e.is_empty())) {
                asked.extend(self.cells.iter().enumerate().map(|(k, b)| (*b, m, "", t + k as f64 * 0.37)));
            }
        }
        for (b, st) in &self.states {
            if let Some(m) = world.models.get(b).and_then(|id| BLOCKS.get(*id)).and_then(|d| d.model.as_deref()).and_then(by_name) {
                asked.push((*b, m, st.as_str(), t));
            }
        }
        // a placed model with a state `placed` plays it while near and asked into no other (a torch's flame, ADR
        // 0117); a flame under water is out
        for (b, id) in &world.models {
            let Some(d) = BLOCKS.get(*id).filter(|_| !self.states.contains_key(b)) else { continue };
            let Some(m) = d.model.as_deref().and_then(by_name).filter(|m| self.list[*m].0.life.states.iter().any(|s| s.name == PLACED)) else { continue };
            let c = world.block_centre(*b);
            if (c - cams[b.body as usize].0).length() < NEAR_M && !(d.flame && world.water_surface(c).is_some()) {
                asked.push((*b, m, PLACED, t));
            }
        }
        // each one's clip and time, for the drawing below
        let mut posed: std::collections::HashMap<Block, (Option<usize>, f64)> = Default::default();
        let mut held: Vec<(String, f32)> = Vec::new();
        for (b, m, st, clock) in asked {
            let l = &self.list[m].0;
            let s = match l.life.states.iter().find(|s| s.name == st) {
                Some(s) => s,
                None if st.is_empty() => &self.clip_state,
                None => continue,
            };
            let (start, prev) = if st.is_empty() { (0.0, clock) } else { (clock, -1e-9) };
            let run = self.runs.entry(b).or_insert_with(|| Run { model: l.name.clone(), state: st.to_string(), start, prev, fx: Vec::new() });
            if run.model != l.name || run.state != st {
                for (h, _) in run.fx.drain(..) {
                    fx.stop(h);
                }
                (run.model, run.state, run.start, run.prev) = (l.name.clone(), st.to_string(), start, prev);
            }
            let ct = clock - run.start;
            let ci = s.clip.as_deref().and_then(|c| l.model.clips.iter().position(|k| k.name == c)).or_else(|| l.model.clips.iter().position(|c| c.name == "idle"));
            let poses = l.model.pose(ci.map(|c| l.model.clips[c].name.as_str()), ct as f32);
            // (the shown model's cells in the view's square frame, as it is drawn)
            let [o, x, y, z] = if self.spec.is_some() && self.cells.contains(&b) { view_axes(world, b) } else { cell_axes(world, b) };
            let lean = lean(world, b);
            let place = |p: Vec3| {
                let p = lean.transform_point3(FLOOR + (p - FLOOR) * l.scale);
                o + x * p.x as f64 + y * p.y as f64 + z * p.z as f64
            };
            let point = |n: Option<&str>| place(n.and_then(|n| l.model.point_at(&poses, n)).unwrap_or(FLOOR));
            let body = &world.bodies[b.body as usize];
            let floor = body.layer_r(b.pos.z as f64);
            let at = |p: DVec3| crate::fx::At::point(b.body, p - body.centre, DVec3::ZERO, floor);
            if run.fx.is_empty() {
                for e in &s.effects {
                    if let Some(h) = fx.start(&e.effect, &at(point(Some(&e.point)))) {
                        run.fx.push((h, e.point.clone()));
                    }
                }
            } else {
                for (h, p) in &run.fx {
                    fx.moved(*h, &at(point(Some(p))));
                }
            }
            if let Some((c, evs)) = ci.and_then(|c| l.life.events.iter().find(|(n, _)| *n == l.model.clips[c].name).map(|(_, e)| (c, e))) {
                for e in events_between(&l.model.clips[c], evs, run.prev, ct) {
                    let p = point(e.point.as_deref());
                    if let Some(snd) = &e.sound {
                        audio.play(snd, Some(p), 1.0, 1.0, false);
                    }
                    if let Some(ef) = &e.effect {
                        fx.play_named(ef, &at(p));
                    }
                }
            }
            run.prev = ct;
            if let Some(snd) = &s.sound {
                // (shortcut: a held loop has no direction, only the nearest one's loudness by its reach; pan it
                // when the mixer can hold a loop at a place)
                let d = (point(None) - eye.0).length() as f32;
                let g = (1.0 - d / self.reach.get(snd).copied().unwrap_or(16.0)).max(0.0).powi(2) * audio.carry;
                match held.iter_mut().find(|h| h.0 == *snd) {
                    Some(h) => h.1 = h.1.max(g),
                    None => held.push((snd.clone(), g)),
                }
            }
            if let Some(li) = &s.light {
                self.lamps.push((b.body, point(Some(&li.point)), li.rgb.map(|v| v * li.strength)));
            }
            posed.insert(b, (ci, ct));
        }
        // states no longer asked for: their effects stop, their particles live out
        self.runs.retain(|b, r| {
            let keep = posed.contains_key(b);
            if !keep {
                r.fx.iter().for_each(|(h, _)| fx.stop(*h));
            }
            keep
        });
        for name in std::mem::take(&mut self.held) {
            if !held.iter().any(|h| h.0 == name) {
                audio.hold(&name, 0.0);
            }
        }
        for (name, g) in &held {
            audio.hold(name, *g);
        }
        self.held = held.into_iter().map(|h| h.0).collect();
        // what to draw: (model, clip, time, its cell's frame or the hand's, in the camera's frame)
        let mut inst: Vec<Vec<(Option<usize>, f64, Mat4, u32, f32)>> = vec![Vec::new(); self.list.len()];
        let by_name = |n: &str| self.list.iter().position(|(l, _)| l.name == n);
        let idle = |m: usize| self.list[m].0.model.clips.iter().position(|c| c.name == "idle");
        if let Some((name, clip, _)) = &self.spec {
            if let Some(m) = by_name(name) {
                let c = clip.as_deref().and_then(|c| self.list[m].0.model.clips.iter().position(|k| k.name == c)).or_else(|| idle(m)).or((!self.list[m].0.model.clips.is_empty()).then_some(0));
                for (k, b) in self.cells.iter().enumerate() {
                    let (c, t) = posed.get(b).copied().unwrap_or((c, t + k as f64 * 0.37));
                    inst[m].push((c, t, frame_of(view_axes(world, *b), cams, *b) * self.spec_place, b.body as u32, self.spec_scale.unwrap_or(self.list[m].0.scale)));
                }
            }
        }
        for (b, id) in &world.models {
            let Some(m) = BLOCKS.get(*id).and_then(|d| d.model.as_deref()).and_then(by_name) else { continue };
            let x = cell_frame(world, cams, *b);
            if (x.w_axis.truncate().length() as f64) < NEAR_M {
                let x = x * lean(world, *b);
                let (c, t) = posed.get(b).copied().unwrap_or((idle(m), t));
                inst[m].push((c, t, x, b.body as u32, 1.0));
            }
        }
        // the animals (ADR 0083): the clip of what each does, each a little out of step with the others; the
        // model's floor centre on its feet, its +z along its look
        for a in herd {
            let Some(m) = by_name(&a.model) else { continue };
            let up = (a.pos - world.bodies[0].centre).normalize_or(DVec3::Z);
            let z = (a.fwd - up * a.fwd.dot(up)).normalize_or(up.any_orthonormal_vector());
            let x = up.cross(z);
            let o = a.pos - (x + z) * 0.5;
            let (cam, turn) = cams[0];
            if (o - cam).length() > NEAR_M {
                continue;
            }
            let clips = &self.list[m].0.model.clips;
            let c = clips.iter().position(|k| k.name == ACT_CLIPS[(a.act as usize).min(4)]).or_else(|| idle(m));
            let f = (DMat4::from_mat3(turn) * DMat4::from_cols(x.extend(0.0), up.extend(0.0), z.extend(0.0), (o - cam).extend(1.0))).as_mat4();
            inst[m].push((c, t + a.id as f64 * 0.37, f, 0, a.scale));
        }
        // the effects' particles with a model look: at `size` times the model, resting on its first part's foot
        for (name, p) in fx.models() {
            let Some(m) = by_name(name) else { continue };
            let foot = self.list[m].0.model.parts.first().map_or(Vec3::ZERO, |q| Vec3::new(q.at[0] as f32 + q.size[0] as f32 * 0.5, q.at[1] as f32, q.at[2] as f32 + q.size[2] as f32 * 0.5) * VOXEL);
            let (cam, turn) = cams[p.body as usize];
            let r = (p.turn * p.size).as_dmat3();
            let x = DMat4::from_mat3(turn) * DMat4::from_cols(r.x_axis.extend(0.0), r.y_axis.extend(0.0), r.z_axis.extend(0.0), (world.bodies[p.body as usize].centre + p.p.as_dvec3() - cam).extend(1.0)) * DMat4::from_translation(-foot.as_dvec3());
            inst[m].push((idle(m), t, x.as_mat4(), p.body as u32, 1.0));
        }
        // in hand (ADR 0115): the item, or with none the figure's bare right arm, both on the one swing
        let mut bare: Option<(usize, usize, Mat4)> = None;
        // the held item's model and place in its instances
        let mut held: Option<(usize, usize)> = None;
        if let Some(h) = hand {
            let (_, fwd, up) = eye;
            let right = fwd.cross(up).normalize();
            let up = right.cross(fwd);
            let eye_m = |v: [f64; 3]| fwd * v[0] + right * v[1] + up * v[2];
            let (hand_at, way) = (eye_m(ARM_HAND), eye_m(ARM_WAY).normalize());
            // the swing: the hand thrust toward the aim (a point ahead on the look) and chopped down about itself
            let sw = h.swing as f64;
            let aim = fwd * ARM_HAND[0] * SWING_AIM;
            let turn = DMat4::from_mat3(cams[cf].1)
                * DMat4::from_translation(hand_at + (aim - hand_at) * SWING_REACH * sw)
                * DMat4::from_axis_angle(right, -SWING_CHOP * sw)
                * DMat4::from_translation(-hand_at);
            match h.item {
                Some(n) => {
                    if let Some(m) = drawn(&self.list, n) {
                        let s = self.list[m].0.model.item.unwrap_or(engine::models::ITEM_M) as f64;
                        let o = fwd * 0.8 + right * 0.42 - up * 0.36 - (right + up - fwd) * (s * 0.5);
                        let x = DMat4::from_cols((right * s).extend(0.0), (up * s).extend(0.0), (-fwd * s).extend(0.0), o.extend(1.0));
                        held = Some((m, inst[m].len()));
                        inst[m].push((idle(m), t, (turn * x).as_mat4() * self.list[m].0.held(), cf as u32, 1.0));
                    }
                }
                None => {
                    let m = drawn(&self.list, FIGURE);
                    if let Some((m, a)) = m.and_then(|m| Some((m, self.list[m].0.model.parts.iter().position(|p| p.name == ARM)?))) {
                        // the arm's voxels to the eye's frame: its length (+y, hand to shoulder) along `way`, its
                        // width level across it; then the model's metres to the arm's voxels at rest
                        let v = engine::models::VOXEL as f64;
                        let ax = up.cross(way).normalize();
                        let az = ax.cross(way);
                        let o = hand_at - (ax * 0.5 + az) * v;
                        let x = DMat4::from_cols((ax * v).extend(0.0), (way * v).extend(0.0), (az * v).extend(0.0), o.extend(1.0));
                        let rest = self.list[m].0.model.pose(None, 0.0)[a].as_dmat4();
                        bare = Some((m, a, (turn * x * rest.inverse()).as_mat4()));
                    }
                }
            }
        }
        // the stacks lying in piles (ADR 0114): at the item's size (a model of painted boxes at its real size unless
        // its file says), an item standing taller than it is wide laid down on its face (+z up), turned about the
        // cell's up, its rest bounds centred on its place and resting on the floor
        for p in &self.piles {
            let Some(m) = drawn(&self.list, &p.name) else { continue };
            let (s, lay, [a, b]) = self.list[m].0.lying(p.item, p.tool);
            let off = Vec3::new(-(a.x + b.x) * 0.5, -a.y, -(a.z + b.z) * 0.5);
            let (cam, turn) = cams[p.body as usize];
            let (up, x) = (p.axes[0].as_dvec3(), p.axes[1].as_dvec3());
            let o = world.bodies[p.body as usize].centre + p.p.as_dvec3() - cam;
            let cell = (DMat4::from_mat3(turn) * DMat4::from_cols(x.extend(0.0), up.extend(0.0), x.cross(up).extend(0.0), o.extend(1.0))).as_mat4();
            inst[m].push((idle(m), t, cell * Mat4::from_rotation_y(p.turn) * Mat4::from_scale(Vec3::splat(s * p.fit)) * Mat4::from_translation(off) * lay, p.body as u32, 1.0));
        }
        for (m, list) in inst.iter().enumerate() {
            let bare = bare.filter(|b| b.0 == m);
            if list.is_empty() && bare.is_none() {
                continue;
            }
            let (l, parts) = &self.list[m];
            // the bare arm: its part and the parts hung from it, at rest
            let arm = |mut i: usize| loop {
                match bare {
                    Some((_, a, _)) if i == a => break true,
                    Some(_) => match l.model.up.get(i).copied().flatten() {
                        Some(k) => i = k,
                        None => break false,
                    },
                    None => break false,
                }
            };
            // (its painted boxes after its parts, as `poses` below: a box is the bare arm's when its part is)
            let rest = match bare {
                Some(_) => {
                    let mut r = l.model.pose(None, 0.0);
                    r.extend(l.boxes.iter().map(|(part, f)| r[*part] * *f).collect::<Vec<_>>());
                    r
                }
                None => Vec::new(),
            };
            let owner = |pi: usize| l.boxes.get(pi.wrapping_sub(l.model.parts.len())).map_or(pi, |b| b.0);
            // a model past the pieces' room is not drawn at all, never in part
            if self.pieces.len() + list.len() * parts.iter().filter(|p| p.1 > 0).count() > MAX_PIECES {
                continue;
            }
            // each one's scale about its cell's floor centre (a creature's in its cells only, never in hand or as a particle)
            let sc = |s: f32| Mat4::from_translation(FLOOR) * Mat4::from_scale(Vec3::splat(s)) * Mat4::from_translation(-FLOOR);
            // (then each painted box's: its part's, then the box's own frame)
            let poses: Vec<Vec<Mat4>> = list
                .iter()
                .map(|(c, t, _, _, _)| {
                    let mut p = l.model.pose(c.map(|c| l.model.clips[c].name.as_str()), *t as f32);
                    p.extend(l.boxes.iter().map(|(part, f)| p[*part] * *f).collect::<Vec<_>>());
                    p
                })
                .collect();
            for (pi, (q0, nq)) in parts.iter().enumerate() {
                if *nq == 0 {
                    continue;
                }
                let first = self.pieces.len() as u32;
                for (k, (_, _, x, body, s)) in list.iter().enumerate() {
                    let r = (*x * sc(*s) * poses[k][pi]).transpose();
                    let hand = if held == Some((m, k)) { self.hand } else { 0 };
                    self.pieces.push(Piece { x: [r.x_axis.to_array(), r.y_axis.to_array(), r.z_axis.to_array()], q0: *q0, body: *body, _p: [hand, 0] });
                }
                if let Some((_, _, x)) = bare.filter(|_| pi < rest.len() && arm(owner(pi))) {
                    let r = (x * rest[pi]).transpose();
                    self.pieces.push(Piece { x: [r.x_axis.to_array(), r.y_axis.to_array(), r.z_axis.to_array()], q0: *q0, body: cf as u32, _p: [self.hand, 0] });
                }
                let count = self.pieces.len() as u32 - first;
                if count > 0 {
                    self.draws.push(Draw { quads: *nq, first, count });
                }
            }
        }
    }

    /// This frame's pieces to the GPU.
    pub fn upload(&self, queue: &wgpu::Queue, buf: &wgpu::Buffer) {
        if !self.pieces.is_empty() {
            queue.write_buffer(buf, 0, bytemuck::cast_slice(&self.pieces));
        }
    }
}

/// The view of a model's slot picture (yaw, pitch), so its painted face shows: a painted item standing taller
/// than wide (the hand axe) from its front (+z, the face the eye sees in hand), a flat one (flint, the stick)
/// from above as it lies, a painted lump (berries, a stone) from three-quarters above; a voxel model as before.
fn picture_view(l: &Loaded) -> (f32, f32) {
    let d = l.bounds[1] - l.bounds[0];
    match () {
        _ if !l.painted_item() => (35.0, 30.0),
        _ if d.y > d.x.max(d.z) => (0.0, 0.0),
        _ if d.y < 0.4 * d.x.max(d.z) => (35.0, 70.0),
        _ => (35.0, 55.0),
    }
}

/// Its pictures, made off the frame thread: a slot's (`picture_view`), and for the `FIGURE` the inventory's large
/// one, nearly level; both from its painting when it has painted boxes.
fn pictures(l: &Loaded, paint: Option<&engine::models::Painting>) -> [Option<crate::sprite::Sprite>; 2] {
    let (yaw, pitch) = picture_view(l);
    let figure = (l.name == FIGURE).then(|| crate::sprite::sprite(&l.model, paint, FIGURE_PX, 22.0, 8.0)).flatten();
    [crate::sprite::sprite(&l.model, paint, ICON_PX, yaw, pitch), figure]
}

/// A model's painting from its file's bytes: none for a model without painted boxes whose painting is missing or
/// unreadable (its picture is then drawn without), refused for a painted one (ADR 0114: else it reloads with no
/// boxes, and a painting caught half-written would blank it).
fn painting_of(m: &Model, bytes: Option<Vec<u8>>, palette: &engine::palette::Palette) -> Result<Option<engine::models::Painting>, String> {
    let boxes = m.parts.iter().any(|p| !p.boxes.is_empty());
    match bytes.ok_or_else(|| "no painting".to_string()).and_then(|b| engine::models::painting(&b, palette)) {
        Ok(p) if boxes => m.check_painting(&p).map(|_| Some(p)),
        Ok(p) => Ok(Some(p)),
        Err(e) if boxes => Err(e),
        Err(_) => Ok(None),
    }
}

/// The state a placed model plays by itself when it has one (ADR 0117: a torch's flame).
pub const PLACED: &str = "placed";

/// A hanging block's model in its cell (ADR 0117): standing on a floor as it is; held from a side, its foot
/// moved to that wall and its top leaned 30 degrees out from it. `from` is `fall::held_from`'s (di, dj, dl); the
/// model's x is the grid's j and its z the grid's i (`cell_axes`).
pub fn hang(from: Option<(i32, i32, i32)>) -> Mat4 {
    match from {
        Some((di, dj, 0)) => {
            let wall = Vec3::new(dj as f32, 0.0, di as f32);
            let mid = Vec3::new(0.5, 0.0, 0.5);
            Mat4::from_translation(mid + wall * 0.36 + Vec3::Y * 0.2) * Mat4::from_axis_angle(Vec3::Y.cross(-wall), 30f32.to_radians()) * Mat4::from_translation(-mid)
        }
        _ => Mat4::IDENTITY,
    }
}

/// The lean of the model placed in cell `b`: `hang` of where it is held from when its block hangs, else none.
fn lean(world: &World, b: Block) -> Mat4 {
    match world.models.get(&b).and_then(|id| BLOCKS.get(*id)) {
        Some(d) if d.hangs => hang(engine::fall::held_from(world, b)),
        _ => Mat4::IDENTITY,
    }
}

/// The place in `list` of the model named `name` that has at least one quad to draw.
fn drawn(list: &[(Loaded, Vec<(u32, u32)>)], name: &str) -> Option<usize> {
    list.iter().position(|(l, _)| l.name == name && l.quads.iter().any(|q| !q.is_empty()))
}

/// A cell's frame in the camera's: the model's x along the grid's j, y up, z along i (a turn of the axes, so
/// nothing mirrors), each as long as the cell is there, the corner at the cell's.
/// A cell's corner and its model axes in its body's frame: x along the grid's j, y up, z along i (a turn of the
/// axes, so nothing mirrors), each as long as the cell is there.
pub(crate) fn cell_axes(world: &World, b: Block) -> [DVec3; 4] {
    let body = &world.bodies[b.body as usize];
    let c = b.pos.as_dvec3() + 0.5;
    let at = |i: f64, j: f64| body.dir(b.face as usize, i, j);
    let r = body.layer_r(c.z);
    let up = at(c.x, c.y);
    let si = (at(c.x + 0.5, c.y) - at(c.x - 0.5, c.y)) * r;
    let sj = (at(c.x, c.y + 0.5) - at(c.x, c.y - 0.5)) * r;
    let su = up * (body.layer_r(c.z + 0.5) - body.layer_r(c.z - 0.5));
    [body.centre + up * r - (si + sj + su) * 0.5, sj, su, si]
}

/// The Studio's view frame of a cell (ADR 0065): `cell_axes` made square, each axis 1 m and at right angles, y up
/// the cell's up and x along its x, the floor's middle where the cell's is. The view's camera and its shown model
/// stand in it, so the Studio's overlay (square metres) lines up with the drawing; a cell's own axes are skewed and
/// up to a tenth longer, and a model drawn in them drifts off the overlay away from the look-at point.
pub(crate) fn view_axes(world: &World, b: Block) -> [DVec3; 4] {
    let [o, cx, cy, cz] = cell_axes(world, b);
    let y = cy.normalize();
    let x = (cx - y * cx.dot(y)).normalize();
    let z = x.cross(y) * x.cross(y).dot(cz).signum();
    [o + (cx + cz) * 0.5 - (x + z) * 0.5, x, y, z]
}

/// A cell's frame in the camera's (`cell_axes`), the corner at the cell's.
fn cell_frame(world: &World, cams: &[(DVec3, DMat3)], b: Block) -> Mat4 {
    frame_of(cell_axes(world, b), cams, b)
}
/// Axes (`cell_axes`, `view_axes`) as a frame in the camera's.
fn frame_of([o, x, y, z]: [DVec3; 4], cams: &[(DVec3, DMat3)], b: Block) -> Mat4 {
    let (cam, turn) = cams[b.body as usize];
    (DMat4::from_mat3(turn) * DMat4::from_cols(x.extend(0.0), y.extend(0.0), z.extend(0.0), (o - cam).extend(1.0))).as_mat4()
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec4;

    fn furnace(effects: bool) -> Loaded {
        let p = engine::palette::Palette::parse(include_str!("../../../data/palette.toml")).unwrap();
        let mut text = include_str!("../../../data/models/furnace.toml").replace("\r\n", "\n");
        if !effects {
            text = text.replace("effects = [{ effect = \"fire\", point = \"flame\" }, { effect = \"smoke\", point = \"chimney\" }]\n", "");
        }
        let (model, life) = parse_life("furnace.toml", &text, &p, &|_| true, &|_| true).unwrap();
        assert_eq!(life.states.iter().find(|s| s.name == "working").unwrap().effects.is_empty(), !effects);
        Loaded { name: "furnace".into(), model, life, quads: Vec::new(), scale: 1.0, boxes: vec![], bounds: [Vec3::ZERO; 2], pictures: Default::default() }
    }

    /// The furnace working on the planet for a second, then `change` done to the models, then 30 s more: the
    /// particles alive at the end.
    fn alive_after(change: impl Fn(&mut Models)) -> usize {
        let w = World::new(768, 256, 1);
        let b = &w.bodies[0];
        let cell = w.block_at(b.spawn * (b.height(b.spawn) + 0.1));
        let mut m = Models::new(&crate::Args([("models".to_string(), "no/such/folder".to_string())].into_iter().collect()));
        m.list = vec![(furnace(true), Vec::new())];
        m.show(Some(("furnace".into(), None)), cell);
        m.show_state(Some("working"));
        let (mut fx, mut audio) = (crate::fx::Fx::new(true, 4096, 1.0, 1), crate::audio::Audio::new());
        let cams = vec![(DVec3::ZERO, DMat3::IDENTITY); w.bodies.len()];
        let eye = (b.centre + b.spawn * (b.height(b.spawn) + 2.0), DVec3::X, b.spawn);
        let mut t = 0.0;
        let mut run = |m: &mut Models, fx: &mut crate::fx::Fx, secs: f64| {
            while t < secs {
                m.frame(&w, &cams, 0, eye, t, None, &[], fx, &mut audio);
                fx.step(&w, 0.1);
                t += 0.1;
            }
        };
        run(&mut m, &mut fx, 1.0);
        assert!(fx.live() > 0, "the working furnace burns");
        change(&mut m);
        run(&mut m, &mut fx, 31.0);
        fx.live()
    }

    #[test]
    fn a_bad_palette_keeps_the_models_before() {
        // (the tests run in crates/client, where data/palette.toml is not: the palette read fails)
        assert!(!std::path::Path::new("data/palette.toml").exists());
        let d = std::env::temp_dir().join(format!("technoape-models-keep-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("furnace.toml"), "x").unwrap();
        let mut kept = std::collections::BTreeMap::new();
        kept.insert("furnace".to_string(), furnace(false));
        kept.insert("gone".to_string(), Loaded { name: "gone".into(), ..furnace(false) });
        let names: Vec<String> = load_keeping(&d, "data/effects.toml".as_ref(), "data/sounds.toml".as_ref(), &mut kept).into_iter().map(|l| l.name).collect();
        let _ = std::fs::remove_dir_all(&d);
        assert_eq!(names, ["furnace"], "the last good model of each file still there");
    }

    /// The boar of `data/` walking as `creature=boar` with its row's scale times `scale`, `model=boar:walk`
    /// on the planet for 2 s: the first piece's transform and how often its step sound played.
    fn boar_walking(scale: f32) -> (Piece, u32) {
        let (p, n) = boar_drawn(scale, None, false, None);
        (p[0], n)
    }

    /// As `boar_walking`, the Studio's view scale `view` given after `show`, and the boar in hand too: the first
    /// part's pieces (in its cell, then in hand) and the step sounds played.
    fn boar_drawn(scale: f32, view: Option<f32>, in_hand: bool, place: Option<([f32; 2], f32)>) -> (Vec<Piece>, u32) {
        // (meshed, with its real bounds: what the hand reads)
        let mut list = vec![shipped("boar")];
        let text = include_str!("../../../data/creatures.toml").replace("\r\n", "\n");
        // the boar's row alone: the rows after it name models this test does not load
        let row = &text[text.find("[[creature]]\nname = \"boar\"").unwrap()..];
        let row = &row[..row[1..].find("[[creature]]").map_or(row.len(), |i| i + 1)];
        creature_walk(&mut list, "boar", &row.replace("scale = 1.0", &format!("scale = {scale:?}")));
        let w = World::new(768, 256, 1);
        let b = &w.bodies[0];
        let cell = w.block_at(b.spawn * (b.height(b.spawn) + 0.1));
        let mut m = Models::new(&crate::Args([("models".to_string(), "no/such/folder".to_string())].into_iter().collect()));
        let parts = (0..list[0].model.parts.len()).map(|_| (0, 1)).collect();
        m.list = vec![(list.remove(0), parts)];
        m.show(Some(("boar".into(), Some("walk".into()))), cell);
        if let Some(s) = view {
            m.show_scale(s);
        }
        if let Some((at, yaw)) = place {
            m.show_place(at, yaw);
        }
        let (mut fx, mut audio) = (crate::fx::Fx::new(true, 64, 1.0, 1), crate::audio::Audio::new());
        let eye = (b.centre + b.spawn * (b.height(b.spawn) + 2.0), DVec3::X, b.spawn);
        audio.ear[0] = eye.0;
        let cams = vec![(DVec3::ZERO, DMat3::IDENTITY); w.bodies.len()];
        let mut t = 0.0;
        while t < 2.0 {
            m.frame(&w, &cams, 0, eye, t, in_hand.then_some(Hand { item: Some("boar"), swing: 0.0 }), &[], &mut fx, &mut audio);
            t += 1.0 / 60.0;
        }
        let n = m.draws[0].count as usize;
        (m.pieces[..n].to_vec(), audio.played.get("step.dirt").copied().unwrap_or(0))
    }

    /// `/spawn boar:2` in a running game: the loaded boar takes the walk its row makes and stands in the cell;
    /// what is no creature or no speed is refused by name and changes nothing; `/spawn` alone takes it away.
    #[test]
    fn spawn_stands_a_creature_of_the_rows_walking_and_refuses_by_name() {
        let p = engine::palette::Palette::parse(include_str!("../../../data/palette.toml")).unwrap();
        let (model, life) = parse_life("boar.toml", include_str!("../../../data/models/boar.toml"), &p, &|_| true, &|_| true).unwrap();
        let text = include_str!("../../../data/creatures.toml").replace("\r\n", "\n");
        let row = &text[text.find("[[creature]]\nname = \"boar\"").unwrap()..];
        let row = &row[..row[1..].find("[[creature]]").map_or(row.len(), |i| i + 1)];
        let w = World::new(768, 256, 1);
        let b = &w.bodies[0];
        let cell = w.block_at(b.spawn * (b.height(b.spawn) + 0.1));
        let mut m = Models::new(&crate::Args([("models".to_string(), "no/such/folder".to_string())].into_iter().collect()));
        let parts = (0..model.parts.len()).map(|_| (0, 1)).collect();
        m.list = vec![(Loaded { name: "boar".into(), quads: vec![Vec::new(); model.parts.len()], model, life, scale: 1.0, boxes: vec![], bounds: [Vec3::ZERO; 2], pictures: Default::default() }, parts)];
        for (bad, why) in [("ape", "no creature 'ape'"), ("boar:fast", "metres a second")] {
            let e = m.spawn_from(bad, row, cell).unwrap_err();
            assert!(e.contains(why), "{bad}: {e}");
            assert!(m.spec.is_none() && !m.list[0].0.model.clips.iter().any(|c| c.name == "walk"), "{bad} changed something");
        }
        let said = m.spawn_from("boar:2", row, cell).unwrap();
        assert!(said.starts_with("boar walks"), "{said}");
        assert_eq!(m.spec.as_ref().map(|s| (s.0.as_str(), s.1.as_deref())), Some(("boar", Some("walk"))));
        assert!(m.list[0].0.model.clips.iter().any(|c| c.name == "walk") && m.cells == vec![cell]);
        // the spec the watcher reads at its next reading of the models (that reading is not run here)
        assert_eq!(m.creature.lock().unwrap().as_deref(), Some("boar:2"));
        m.spawn_from("", row, cell).unwrap();
        assert!(m.spec.is_none() && m.creature.lock().unwrap().is_none());
    }

    fn axis_len(p: &Piece, r: usize) -> f32 {
        Vec3::new(p.x[0][r], p.x[1][r], p.x[2][r]).length()
    }

    /// A creature's scale is its cells' only: the same model in hand is drawn as any block in hand.
    #[test]
    fn a_creatures_scale_never_reaches_the_model_in_hand() {
        let ((one, _), (two, _)) = (boar_drawn(1.0, None, true, None), boar_drawn(2.0, None, true, None));
        assert_eq!((one.len(), two.len()), (2, 2), "in its cell and in hand");
        for r in 0..3 {
            assert!((axis_len(&two[0], r) / axis_len(&one[0], r) - 2.0).abs() < 1e-3, "the cell's axis {r}");
            assert!((axis_len(&two[1], r) / axis_len(&one[1], r) - 1.0).abs() < 1e-3, "in hand, axis {r}: {} then {}", axis_len(&one[1], r), axis_len(&two[1], r));
        }
    }

    /// The boar's first part drawn once in hand, its file's `item` set to `item`; and, with `pile`, lying in a pile.
    fn in_hand(item: Option<f32>, pile: bool) -> Vec<Piece> {
        let mut l = shipped("boar");
        l.model.item = item;
        let w = World::new(768, 256, 1);
        let b = &w.bodies[0];
        let mut m = Models::new(&crate::Args([("models".to_string(), "no/such/folder".to_string())].into_iter().collect()));
        let parts = (0..l.model.parts.len()).map(|_| (0, 1)).collect();
        m.list = vec![(l, parts)];
        if pile {
            let at = crate::fx::At::floor_of(&w, w.air_over_ground(0, b.spawn));
            m.piles = vec![crate::piles::Lying { name: "boar".into(), item: true, tool: false, body: 0, p: at.p, axes: at.axes, turn: 0.3, fit: 1.0 }];
        }
        let (mut fx, mut audio) = (crate::fx::Fx::new(true, 64, 1.0, 1), crate::audio::Audio::new());
        let eye = (b.centre + b.spawn * (b.height(b.spawn) + 2.0), DVec3::X, b.spawn);
        m.frame(&w, &vec![(DVec3::ZERO, DMat3::IDENTITY); w.bodies.len()], 0, eye, 0.0, Some(Hand { item: Some("boar"), swing: 0.0 }), &[], &mut fx, &mut audio);
        m.pieces[..m.draws[0].count as usize].to_vec()
    }

    /// ADR 0114: a model of painted boxes (flint, clutter) has a quad a pixel for an instance, on all six faces of
    /// its box, and is drawn in hand (ADR 0110 said so; before this nothing was drawn).
    #[test]
    fn a_painted_box_model_is_drawn_in_hand() {
        let pal = engine::palette::Palette::parse(include_str!("../../../data/palette.toml")).unwrap();
        let (model, life) = parse_life("flint.toml", include_str!("../../../data/models/flint.toml"), &pal, &|_| true, &|_| true).unwrap();
        let paint = engine::models::painting(include_bytes!("../../../data/models/flint.png"), &pal).unwrap();
        // what the file says, so the test follows a repainted flint: its parts, its one box, that box's middle
        let (parts, bx) = (model.parts.len(), model.parts[0].boxes[0].clone());
        let size = Vec3::from(bx.size.map(|v| v.max(1) as f32));
        let mid = bx.frame().transform_point3(size * 0.5);
        let l = Loaded::new("flint".into(), model, life, Some(&paint));
        assert_eq!((l.quads.len(), l.boxes.len()), (parts + 1, 1), "its part (one empty voxel), then its box");
        let dirs: std::collections::BTreeSet<u32> = l.quads[parts].iter().map(|q| q[0] >> 18 & 7).collect();
        assert_eq!(dirs.len(), 6, "six faces");
        let centre = (l.bounds[0] + l.bounds[1]) * 0.5;
        assert!((centre - mid * engine::models::VOXEL).length() < 0.02, "its bounds about its box: {:?} against {}", l.bounds, mid * engine::models::VOXEL);
        let w = World::new(768, 256, 1);
        let b = &w.bodies[0];
        let mut m = Models::new(&crate::Args([("models".to_string(), "no/such/folder".to_string())].into_iter().collect()));
        let parts = l.quads.iter().map(|q| (0, q.len() as u32)).collect();
        m.list = vec![(l, parts)];
        let (mut fx, mut audio) = (crate::fx::Fx::new(true, 64, 1.0, 1), crate::audio::Audio::new());
        let eye = (b.centre + b.spawn * (b.height(b.spawn) + 2.0), DVec3::X, b.spawn);
        m.frame(&w, &vec![(DVec3::ZERO, DMat3::IDENTITY); w.bodies.len()], 0, eye, 0.0, Some(Hand { item: Some("flint"), swing: 0.0 }), &[], &mut fx, &mut audio);
        assert_eq!(m.draws.iter().map(|d| (d.count, d.quads > 0)).collect::<Vec<_>>(), [(1, true)], "the box drawn once, in hand");
        // its middle where the hand holds things (not low in its cell, under the screen's edge), its painted top
        // turned to the eye as a flat thing is held up to be seen
        let p = m.pieces[m.draws[0].first as usize];
        let at = |v: Vec4| Vec3::new(Vec4::from(p.x[0]).dot(v), Vec4::from(p.x[1]).dot(v), Vec4::from(p.x[2]).dot(v));
        let (fwd, up) = (eye.1, eye.2);
        let right = fwd.cross(up).normalize();
        let hold = (fwd * 0.8 + right * 0.42 - right.cross(fwd) * 0.36).as_vec3();
        let mid = size.extend(2.0) * 0.5;
        assert!((at(mid) - hold).length() < 0.03, "{} against {hold}", at(mid));
        assert!(at(Vec4::Y).normalize().dot(-fwd.as_vec3()) > 0.6, "top to the eye: {}", at(Vec4::Y));
    }

    /// Review of ADR 0114: a painted model's painting missing, unreadable or too small for its boxes refuses the
    /// model (a reload keeps its last good one); a voxel model reads without one.
    #[test]
    fn a_painted_model_without_a_good_painting_is_refused() {
        let pal = engine::palette::Palette::parse(include_str!("../../../data/palette.toml")).unwrap();
        let flint = Model::parse("flint.toml", include_str!("../../../data/models/flint.toml"), &pal).unwrap();
        let png = include_bytes!("../../../data/models/flint.png").to_vec();
        assert!(painting_of(&flint, Some(png.clone()), &pal).unwrap().is_some());
        assert!(painting_of(&flint, None, &pal).is_err(), "missing");
        assert!(painting_of(&flint, Some(png[..png.len() / 2].to_vec()), &pal).is_err(), "half written");
        let small = engine::models::painting(include_bytes!("../../../data/models/pebbles.png"), &pal).unwrap();
        let mut stone = Model::parse("loose stone.toml", include_str!("../../../data/models/loose stone.toml"), &pal).unwrap();
        stone.parts[0].boxes[0].uv = [small.w, small.h];
        let mut bytes = Vec::new();
        bytes.extend_from_slice(include_bytes!("../../../data/models/pebbles.png"));
        assert!(painting_of(&stone, Some(bytes), &pal).is_err(), "its boxes outside the painting");
        let voxels = Model::parse("cube.toml", "colours = [\"copper\"]
[[part]]
name = \"a\"
box = [1, 1, 1]
voxels = \"a\"
", &pal).unwrap();
        assert!(painting_of(&voxels, None, &pal).unwrap().is_none(), "a voxel model needs none");
    }

    /// Review of ADR 0114: a model that loads but has nothing to draw (a painted one without its painting) is no
    /// model to the hand, a pile or a slot: the item is its cube again.
    #[test]
    fn a_model_with_nothing_to_draw_is_its_cube() {
        let pal = engine::palette::Palette::parse(include_str!("../../../data/palette.toml")).unwrap();
        let (model, life) = parse_life("flint.toml", include_str!("../../../data/models/flint.toml"), &pal, &|_| true, &|_| true).unwrap();
        let l = Loaded::new("flint".into(), model, life, None);
        let w = World::new(768, 256, 1);
        let b = &w.bodies[0];
        let mut m = Models::new(&crate::Args([("models".to_string(), "no/such/folder".to_string())].into_iter().collect()));
        let parts = l.quads.iter().map(|q| (0, q.len() as u32)).collect();
        m.list = vec![(l, parts)];
        assert!(!m.has("flint"), "nothing to draw");
        assert!(m.picture("flint", 0).is_none(), "no picture in a slot either");
        let cell = w.air_over_ground(0, b.spawn);
        let piles = [(cell, vec![Some(engine::items::Stack::new(engine::items::ITEMS.named("flint").unwrap(), 1))])].into_iter().collect();
        let (lying, cubes, _) = crate::piles::lay(&w, &piles, w.block_centre(cell), &|n, i, t| m.lying_half(n, i, t));
        assert_eq!((lying.len(), cubes.len()), (0, 1), "its cube in the pile");
        let (mut fx, mut audio) = (crate::fx::Fx::new(true, 64, 1.0, 1), crate::audio::Audio::new());
        let eye = (b.centre + b.spawn * (b.height(b.spawn) + 2.0), DVec3::X, b.spawn);
        m.frame(&w, &vec![(DVec3::ZERO, DMat3::IDENTITY); w.bodies.len()], 0, eye, 0.0, Some(Hand { item: Some("flint"), swing: 0.0 }), &[], &mut fx, &mut audio);
        assert!(m.draws.is_empty(), "nothing in hand");
    }

    /// Review of ADR 0114: a voxel model in hand (a block's, a creature's) is held as before: its cell's corner where
    /// it was, not moved to its middle nor tipped; only flat painted items are.
    #[test]
    fn a_voxel_model_in_hand_is_held_as_before() {
        let p = engine::palette::Palette::parse(include_str!("../../../data/palette.toml")).unwrap();
        let _ = p;
        let l = shipped("boar");
        assert!(l.bounds[1].x > l.bounds[0].x, "real bounds");
        let first = l.quads.iter().position(|q| !q.is_empty()).unwrap();
        let w = World::new(768, 256, 1);
        let b = &w.bodies[0];
        let mut m = Models::new(&crate::Args([("models".to_string(), "no/such/folder".to_string())].into_iter().collect()));
        let parts = l.quads.iter().map(|q| (0, q.len() as u32)).collect();
        m.list = vec![(l, parts)];
        let (mut fx, mut audio) = (crate::fx::Fx::new(true, 64, 1.0, 1), crate::audio::Audio::new());
        let eye = (b.centre + b.spawn * (b.height(b.spawn) + 2.0), DVec3::X, b.spawn);
        m.frame(&w, &vec![(DVec3::ZERO, DMat3::IDENTITY); w.bodies.len()], 0, eye, 0.0, Some(Hand { item: Some("boar"), swing: 0.0 }), &[], &mut fx, &mut audio);
        // main's hold: the cell's corner so that its middle is at the hand's point, its axes 0.35 m
        let (fwd, up) = (eye.1, eye.2);
        let right = fwd.cross(up).normalize();
        let up = right.cross(fwd);
        let s = 0.35;
        let o = fwd * 0.8 + right * 0.42 - up * 0.36 - (right + up - fwd) * (s * 0.5);
        let x = DMat4::from_cols((right * s).extend(0.0), (up * s).extend(0.0), (-fwd * s).extend(0.0), o.extend(1.0)).as_mat4();
        let l = &m.list[0].0;
        let idle = l.model.clips.iter().find(|c| c.name == "idle").map(|c| c.name.as_str());
        let mut pose = l.model.pose(idle, 0.0);
        pose.extend(l.boxes.iter().map(|(part, f)| pose[*part] * *f).collect::<Vec<_>>());
        let want = (x * pose[first]).transpose();
        let got = m.pieces[m.draws[0].first as usize];
        for (r, row) in [want.x_axis, want.y_axis, want.z_axis].iter().enumerate() {
            assert!((Vec4::from(got.x[r]) - *row).length() < 1e-4, "row {r}: {:?} against {row}", got.x[r]);
        }
    }

    /// Review of ADR 0114: of the models that can be in hand (an item or block of their name), only the flat ones
    /// (flint, the stick) are tipped; the voxel ones (creatures, the furnace) are held as on main.
    #[test]
    fn only_flat_painted_models_are_tipped_in_hand() {
        let pal = engine::palette::Palette::parse(include_str!("../../../data/palette.toml")).unwrap();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/models");
        let mut tipped = vec![];
        for name in ["berries", "flint", "hand axe", "loose stone", "mushroom", "stick", "boar", "furnace", "ape", "deer"] {
            let text = std::fs::read_to_string(dir.join(format!("{name}.toml"))).unwrap();
            let (model, life) = parse_life(&format!("{name}.toml"), &text, &pal, &|_| true, &|_| true).unwrap();
            let png = painting_of(&model, std::fs::read(dir.join(format!("{name}.png"))).ok(), &pal).unwrap();
            let l = Loaded::new(name.into(), model, life, png.as_ref());
            let h = l.held();
            if !l.painted_item() {
                assert_eq!(h, Mat4::IDENTITY, "{name}: a voxel model or a painted figure is held by its cell as on main");
            } else if h.y_axis.y < 0.99 {
                tipped.push(name);
            }
        }
        assert_eq!(tipped, ["flint", "stick"]);
    }

    /// ADR 0114: a model's `item` is the size of its cell in hand, in place of the fixed 0.35 m; a pile lays it at
    /// the same size.
    #[test]
    fn a_models_item_size_is_its_size_in_hand_and_in_a_pile() {
        let (plain, small) = (in_hand(None, false), in_hand(Some(0.2), false));
        for r in 0..3 {
            assert!((axis_len(&small[0], r) / axis_len(&plain[0], r) - 0.2 / 0.35).abs() < 1e-3, "axis {r}: {} then {}", axis_len(&plain[0], r), axis_len(&small[0], r));
        }
        let lying = in_hand(Some(0.2), true);
        assert_eq!(lying.len(), 2, "in hand and in the pile");
        for r in 0..3 {
            assert!((axis_len(&lying[1], r) / axis_len(&small[0], r) - 1.0).abs() < 1e-3, "lying, axis {r}");
        }
    }

    /// ADR 0114 second round: the hand axe lying in a pile rests on the ground's top (its lowest quad corner at the
    /// floor, not in it); a tool lies at `TOOL_LYING` times its `item` size (the developer's call: about 42 cm),
    /// anything else at its item size.
    #[test]
    fn a_lying_axe_rests_on_the_floor_at_its_size() {
        for (tool, fit, k) in [(true, 1.0, TOOL_LYING), (false, 1.0, 1.0), (true, 0.5, TOOL_LYING * 0.5)] {
            let (low, span, long) = lying_axe(tool, fit);
            assert!(low.abs() < 0.005, "tool {tool}: its lowest corner {low} m from the floor");
            assert!((span - long * k).abs() < 0.03 * k, "tool {tool}: {span} m long on the ground, its item size {long}");
        }
    }

    /// The hand axe lying alone (as a tool or not): its lowest corner over the floor, its length on the ground,
    /// its item size's length.
    fn lying_axe(tool: bool, fit: f32) -> (f32, f32, f32) {
        let pal = engine::palette::Palette::parse(include_str!("../../../data/palette.toml")).unwrap();
        let (model, life) = parse_life("hand axe.toml", include_str!("../../../data/models/hand axe.toml"), &pal, &|_| true, &|_| true).unwrap();
        let paint = engine::models::painting(include_bytes!("../../../data/models/hand axe.png"), &pal).unwrap();
        let l = Loaded::new("hand axe".into(), model, life, Some(&paint));
        let long = (l.bounds[1] - l.bounds[0]).max_element() * l.model.item.unwrap();
        let w = World::new(768, 256, 1);
        let b = &w.bodies[0];
        let mut m = Models::new(&crate::Args([("models".to_string(), "no/such/folder".to_string())].into_iter().collect()));
        let mut at = 0;
        let parts: Vec<(u32, u32)> = l.quads.iter().map(|q| { at += q.len() as u32; (at - q.len() as u32, q.len() as u32) }).collect();
        let quads: Vec<[u32; 4]> = l.quads.concat();
        m.list = vec![(l, parts.clone())];
        let floor = crate::fx::At::floor_of(&w, w.air_over_ground(0, b.spawn));
        m.piles = vec![crate::piles::Lying { name: "hand axe".into(), item: true, tool, body: 0, p: floor.p, axes: floor.axes, turn: 0.7, fit }];
        let (mut fx, mut audio) = (crate::fx::Fx::new(true, 64, 1.0, 1), crate::audio::Audio::new());
        let eye = (b.centre + b.spawn * (b.height(b.spawn) + 2.0), DVec3::X, b.spawn);
        m.frame(&w, &vec![(DVec3::ZERO, DMat3::IDENTITY); w.bodies.len()], 0, eye, 0.0, None, &[], &mut fx, &mut audio);
        let (up, base) = (floor.axes[0], b.centre.as_vec3() + floor.p);
        let (mut low, mut pts) = (f32::MAX, vec![]);
        for pc in &m.pieces {
            let (q0, n) = *parts.iter().find(|p| p.0 == pc.q0 && p.1 > 0).unwrap();
            for q in &quads[q0 as usize..(q0 + n) as usize] {
                let dir = q[0] >> 18 & 7;
                let a = (dir / 2) as usize;
                let mut c = Vec3::new((q[0] & 63) as f32, (q[0] >> 6 & 63) as f32, (q[0] >> 12 & 63) as f32);
                c[a] += if dir & 1 == 0 { 1.0 } else { 0.0 };
                let (e1, e2) = ((a + 1) % 3, (a + 2) % 3);
                for (u, v) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
                    let mut p = c;
                    p[e1] += u * (q[1] & 63) as f32;
                    p[e2] += v * (q[1] >> 6 & 63) as f32;
                    let v4 = p.extend(1.0);
                    let r = Vec3::new(Vec4::from(pc.x[0]).dot(v4), Vec4::from(pc.x[1]).dot(v4), Vec4::from(pc.x[2]).dot(v4)) - base;
                    low = low.min(r.dot(up));
                    pts.push(r - up * r.dot(up));
                }
            }
        }
        let span = pts.iter().flat_map(|a| pts.iter().map(move |b| (*a - *b).length())).fold(0.0, f32::max);
        (low, span, long)
    }

    /// The Studio's overlay draws the model in square metres about the eye it sends: the renderer's frame for
    /// the camera and the shown model must be square too, or the two drift apart away from the look-at point
    /// (a cell of the cube-sphere is skewed and not quite 1 m; the bug: edges and box a voxel off a long model).
    #[test]
    fn the_view_frame_is_square_metres() {
        for planet in [768, 6144] {
            let w = World::new(planet, 256, 1);
            let b = &w.bodies[0];
            for d in [DVec3::Y, b.spawn, DVec3::new(0.3, 1.0, 0.7).normalize()] {
                let cell = w.air_over_ground(0, d);
                let [_, x, y, z] = view_axes(&w, cell);
                let [_, cx, cy, cz] = cell_axes(&w, cell);
                for (n, a) in [("x", x), ("y", y), ("z", z)] {
                    assert!((a.length() - 1.0).abs() < 1e-9, "{planet} {d}: axis {n} is {} m (cell: {} {} {})", a.length(), cx.length(), cy.length(), cz.length());
                }
                assert!(x.dot(y).abs() < 1e-9 && y.dot(z).abs() < 1e-9 && z.dot(x).abs() < 1e-9, "{planet} {d}: not at right angles");
                // the same way round as the cell's, up its up
                assert!(y.dot(cy.normalize()) > 1.0 - 1e-9 && x.cross(y).dot(z) * cx.cross(cy).dot(cz) > 0.0);
            }
        }
    }

    /// The Studio's view walks a creature about: the shown model stands `at` metres from its cell along the
    /// cell's x and z, turned by `yaw` about its floor centre, and is otherwise drawn as it was.
    #[test]
    fn the_view_draws_the_shown_model_where_and_turned_as_given() {
        let (at, yaw) = ([2.0, -1.0], 0.7f32);
        let ((was, _), (now, _)) = (boar_drawn(1.0, None, false, None), boar_drawn(1.0, None, false, Some((at, yaw))));
        let w = World::new(768, 256, 1);
        let b = &w.bodies[0];
        let x = frame_of(view_axes(&w, w.block_at(b.spawn * (b.height(b.spawn) + 0.1))), &vec![(DVec3::ZERO, DMat3::IDENTITY); w.bodies.len()], w.block_at(b.spawn * (b.height(b.spawn) + 0.1)));
        let mat = |p: &Piece| Mat4::from_cols_array_2d(&[p.x[0], p.x[1], p.x[2], [0.0, 0.0, 0.0, 1.0]]).transpose();
        let want = x * Mat4::from_translation(Vec3::new(at[0], 0.0, at[1]) + FLOOR) * Mat4::from_rotation_y(yaw) * Mat4::from_translation(-FLOOR) * x.inverse() * mat(&was[0]);
        let got = mat(&now[0]);
        assert!(got.abs_diff_eq(want, 1e-2), "got {got:?}
want {want:?}");
        assert!(!got.abs_diff_eq(mat(&was[0]), 0.5), "it moved");
    }

    /// The Studio's view draws the shown model at the scale it was given (the Fauna page: the row's `scale`).
    #[test]
    fn the_view_draws_the_shown_model_at_its_given_scale() {
        let ((one, _), (three, _)) = (boar_drawn(1.0, None, false, None), boar_drawn(1.0, Some(3.0), false, None));
        for r in 0..3 {
            assert!((axis_len(&three[0], r) / axis_len(&one[0], r) - 3.0).abs() < 1e-3, "axis {r}: {} then {}", axis_len(&one[0], r), axis_len(&three[0], r));
        }
    }

    #[test]
    fn a_creature_is_drawn_at_its_scale_and_steps_on_its_footfalls() {
        let ((one, steps), (two, _)) = (boar_walking(1.0), boar_walking(2.0));
        for r in 0..3 {
            let len = |p: &Piece| Vec3::new(p.x[0][r], p.x[1][r], p.x[2][r]).length();
            assert!((len(&two) / len(&one) - 2.0).abs() < 1e-3, "axis {r}: {} then {}", len(&one), len(&two));
        }
        // 2 s of a 0.58 s cycle, two footfalls a cycle
        let cycle = 2.0 * 0.35 / 1.2;
        assert_eq!(steps, (0..20).filter(|k| (0.25 + 0.5 * *k as f64) * cycle < 2.0).count() as u32, "the boar's footfalls, a quarter and three quarters into each cycle");
    }

    #[test]
    fn states_are_checked_against_the_effects_and_sounds_in_use() {
        let d = std::env::temp_dir().join(format!("technoape-models-known-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = engine::palette::Palette::parse(include_str!("../../../data/palette.toml")).unwrap();
        std::fs::write(d.join("effects.toml"), "[[effect]]\nname='only here'\ncount=1\nlife=[1,1]\nsize=[0.1,0.1]\nlook='debris'\n").unwrap();
        std::fs::write(d.join("sounds.toml"), "[[sound]]\nname = 'only.here'\nbus = 'world'\nlayer = [{ wave = 'sine', freq = [440, 440], decay = 0.1 }]\n").unwrap();
        let (fx, sounds) = known(&d.join("effects.toml"), &d.join("sounds.toml"), &p);
        let _ = std::fs::remove_dir_all(&d);
        assert!(fx.rows_of("only here").is_some() && fx.rows_of("fire").is_none(), "the effects file in use");
        assert_eq!(sounds, ["only.here"], "the sounds file in use");
    }

    #[test]
    fn a_reload_or_a_refused_model_stops_the_old_states_effects() {
        // the file read again without the state's effects: they stop, their particles live out
        assert_eq!(alive_after(|m| (m.list, m.reloaded) = (vec![(furnace(false), Vec::new())], true)), 0);
        // every model refused (none left): nothing burns on unseen
        assert_eq!(alive_after(|m| m.list.clear()), 0);
        // and with nothing changed it burns on
        assert!(alive_after(|_| ()) > 0);
    }

    /// The ape loaded, each part's quads at its index times 1000 (so a piece names its part), and `hand` drawn:
    /// the pieces and the eye's look, up and right.
    fn ape_hand(hand: Option<Hand>) -> (Models, Loaded, [DVec3; 3]) {
        let pal = engine::palette::Palette::parse(include_str!("../../../data/palette.toml")).unwrap();
        let load = || {
            let (model, life) = parse_life("ape.toml", include_str!("../../../data/models/ape.toml"), &pal, &|_| true, &|_| true).unwrap();
            let paint = painting_of(&model, Some(include_bytes!("../../../data/models/ape.png").to_vec()), &pal).unwrap();
            Loaded::new(FIGURE.into(), model, life, paint.as_ref())
        };
        let l = load();
        let w = World::new(768, 256, 1);
        let b = &w.bodies[0];
        let mut m = Models::new(&crate::Args([("models".to_string(), "no/such/folder".to_string())].into_iter().collect()));
        let parts = l.quads.iter().enumerate().map(|(i, q)| (i as u32 * 1000, q.len() as u32)).collect();
        m.list = vec![(l, parts)];
        let (mut fx, mut audio) = (crate::fx::Fx::new(true, 64, 1.0, 1), crate::audio::Audio::new());
        let up = b.spawn;
        let fwd = up.any_orthonormal_vector();
        let eye = (b.centre + up * (b.height(up) + 2.0), fwd, up);
        m.frame(&w, &vec![(DVec3::ZERO, DMat3::IDENTITY); w.bodies.len()], 0, eye, 0.0, hand, &[], &mut fx, &mut audio);
        (m, load(), [fwd, up, fwd.cross(up)])
    }

    /// A piece's transform in the arm's own voxels: a box's piece (the painted ape, ADR 0116) is its part's pose
    /// times its box's frame, so that frame is taken off again. The first piece is the arm's first box.
    fn arm_voxels(m: &Models, k: usize) -> Mat4 {
        let p = &m.pieces[k];
        let l = &m.list[0].0;
        let part = l.model.parts.len();
        let b = (p.q0 / 1000) as usize - part;
        Mat4::from_cols(Vec4::from(p.x[0]), Vec4::from(p.x[1]), Vec4::from(p.x[2]), Vec4::W).transpose() * l.boxes[b].1.inverse()
    }

    /// The hand's end of the arm (its first layer's middle, ape.toml's arm_r box: the hand at y = 0) in the
    /// camera's frame.
    fn hand_end(m: &Models) -> DVec3 {
        arm_voxels(m, 0).transform_point3(Vec3::new(0.5, 0.0, 1.0)).as_dvec3()
    }

    /// ADR 0115: with nothing in hand the ape's own right arm is drawn, once, and nothing else of the ape; low at
    /// the right, ahead of the eye. ADR 0116: the painted ape's arm is its boxes (the forearm, palm, fingers and
    /// thumb), every one of them and nothing else.
    #[test]
    fn an_empty_hand_draws_the_figures_arm_only() {
        let (m, l, [fwd, up, right]) = ape_hand(Some(Hand { item: None, swing: 0.0 }));
        let arm = l.model.parts.iter().position(|p| p.name == "arm_r").unwrap();
        let want: Vec<u32> = l.boxes.iter().enumerate().filter(|(_, b)| b.0 == arm).map(|(k, _)| (l.model.parts.len() + k) as u32 * 1000).collect();
        assert!(want.len() >= 3, "the arm is boxes: {want:?}");
        assert_eq!(m.pieces.iter().map(|p| p.q0).collect::<Vec<_>>(), want, "the right arm's boxes, once each");
        let h = hand_end(&m);
        assert!(h.dot(fwd) > 0.2 && h.dot(right) > 0.0 && h.dot(up) < 0.0, "ahead, right and low: {h}");
        // at the ape's own size: its 5 voxels, 0.625 m from hand to shoulder, every box on the one arm
        for k in 0..m.pieces.len() {
            let r = arm_voxels(&m, k);
            let len = (r.transform_point3(Vec3::new(0.5, 5.0, 1.0)).as_dvec3() - h).length();
            assert!((len - 0.625).abs() < 1e-3, "box {k}: the arm {len} m long");
        }
    }

    /// ADR 0115: nothing shown in hand (a hidden HUD, the death screen) draws no arm.
    #[test]
    fn no_hand_draws_no_arm() {
        assert!(ape_hand(None).0.pieces.is_empty());
    }

    /// ADR 0115: the swing strikes the hand in toward the aim at its peak, never out of the view (the first
    /// pose turned it down and the arm left the screen for a third of the swing), and puts it back at its end.
    #[test]
    fn the_swing_strikes_toward_the_aim_and_back() {
        let at = |s: f32| hand_end(&ape_hand(Some(Hand { item: None, swing: s })).0);
        let (_, _, [fwd, up, _]) = ape_hand(None);
        let off = |p: DVec3| p.angle_between(fwd);
        let (rest, peak) = (at(0.0), at(1.0));
        assert!(off(peak) < off(rest) - 0.1, "nearer the aim at the peak: {rest} then {peak}");
        // the view's half height is 35 degrees: the hand stays inside it all the way
        for k in 0..=10 {
            let p = at(k as f32 / 10.0);
            assert!(p.dot(up) / p.dot(fwd) > -32f64.to_radians().tan(), "in view at {k}/10: {p}");
        }
        assert!((at(swing(SWING_S)) - rest).length() < 1e-6, "back at the end");
    }

    /// ADR 0115: a swing is 0 at rest and at its end and peaks once; a new event (its age 0 again) restarts it.
    #[test]
    fn a_swing_peaks_once_and_ends_at_rest() {
        assert_eq!((swing(-1.0), swing(0.0), swing(SWING_S), swing(9.0)), (0.0, 0.0, 0.0, 0.0));
        let s: Vec<f32> = (0..=100).map(|k| swing(SWING_S * k as f64 / 100.0)).collect();
        let top = s.iter().cloned().fold(0.0, f32::max);
        assert!(top > 0.99, "it reaches its peak: {top}");
        let turns = s.windows(3).filter(|w| (w[1] - w[0]) * (w[2] - w[1]) < 0.0).count();
        assert_eq!(turns, 1, "up once, then down");
        // the hand is half way through when the next dig comes: its age is 0 again, the swing starts over
        assert!(swing(SWING_S * 0.5) > 0.0 && swing(0.0) == 0.0);
    }

    /// ADR 0117: a torch on a floor stands as its model; one held from a wall has its foot at that wall and its
    /// top out from it, from the blocks alone (`fall::held_from`, which the server's rule reads too).
    #[test]
    fn a_wall_torch_leans_out_from_its_wall() {
        assert_eq!(hang(Some((0, 0, -1))), Mat4::IDENTITY);
        assert_eq!(hang(None), Mat4::IDENTITY);
        for (di, dj) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let m = hang(Some((di, dj, 0)));
            let wall = Vec3::new(dj as f32, 0.0, di as f32);
            let (foot, top) = (m.transform_point3(Vec3::new(0.5, 0.0, 0.5)), m.transform_point3(Vec3::new(0.5, 0.65, 0.5)));
            assert!((foot - Vec3::new(0.5, 0.0, 0.5)).dot(wall) > 0.3, "{wall}: its foot by the wall: {foot}");
            assert!((top - foot).dot(wall) < -0.2 && (top - foot).y > 0.5, "{wall}: its top out from it: {top}");
            assert!((0..3).all(|a| (-0.01..1.01).contains(&foot[a]) && (-0.01..1.01).contains(&top[a])), "{wall}: inside its cell: {foot} {top}");
        }
        // the client's drawing reads the placed cell's own world: a wall to -i
        let mut w = World::new(768, 256, 1);
        let b = &w.bodies[0];
        let cell = w.air_over_ground(0, b.spawn);
        let up = Block { pos: cell.pos + glam::IVec3::new(0, 0, 3), ..cell };
        for (di, dj, dl) in [(0, 0, -1), (1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0)] {
            w.set_block(Block { pos: up.pos + glam::IVec3::new(di, dj, dl), ..up }, engine::blocks::AIR);
        }
        w.set_block(Block { pos: up.pos - glam::IVec3::X, ..up }, BLOCKS.id("stone"));
        w.set_block(up, BLOCKS.id("torch"));
        assert_eq!(lean(&w, up), hang(Some((-1, 0, 0))));
    }

    /// A model of `data/models` loaded as `load` loads it: its painting, its boxes, its pictures.
    fn shipped(name: &str) -> Loaded {
        let pal = engine::palette::Palette::parse(include_str!("../../../data/palette.toml")).unwrap();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/models");
        let text = std::fs::read_to_string(dir.join(format!("{name}.toml"))).unwrap();
        let (model, life) = parse_life(&format!("{name}.toml"), &text, &pal, &|_| true, &|_| true).unwrap();
        let png = painting_of(&model, std::fs::read(dir.join(format!("{name}.png"))).ok(), &pal).unwrap();
        let mut l = Loaded::new(name.into(), model, life, png.as_ref());
        l.pictures = pictures(&l, png.as_ref());
        l
    }

    /// ADR 0116: a painted box on a part the walk moves (the deer's front leg) is drawn by that part's pose at
    /// each time, not by its rest: what stays fixed between two times is only the cell's frame.
    #[test]
    fn a_painted_box_follows_its_part_through_a_walk() {
        let mut list = vec![shipped("deer")];
        let text = include_str!("../../../data/creatures.toml").replace("\r\n", "\n");
        let row = &text[text.find("[[creature]]\nname = \"deer\"").unwrap()..];
        let row = &row[..row[1..].find("[[creature]]").map_or(row.len(), |i| i + 1)];
        creature_walk(&mut list, "deer", row);
        let l = list.remove(0);
        let (n, leg) = (l.model.parts.len(), l.model.parts.iter().position(|p| p.name == "leg_fl").unwrap());
        let b = l.boxes.iter().position(|x| x.0 == leg).unwrap();
        let frame = l.boxes[b].1;
        let w = World::new(768, 256, 1);
        let body = &w.bodies[0];
        let cell = w.block_at(body.spawn * (body.height(body.spawn) + 0.1));
        let mut m = Models::new(&crate::Args([("models".to_string(), "no/such/folder".to_string())].into_iter().collect()));
        let parts = l.quads.iter().enumerate().map(|(i, q)| (i as u32 * 1000, q.len() as u32)).collect();
        m.list = vec![(l, parts)];
        m.show(Some(("deer".into(), Some("walk".into()))), cell);
        let (mut fx, mut audio) = (crate::fx::Fx::new(true, 64, 1.0, 1), crate::audio::Audio::new());
        let eye = (body.centre + body.spawn * (body.height(body.spawn) + 2.0), DVec3::X, body.spawn);
        let cams = vec![(DVec3::ZERO, DMat3::IDENTITY); w.bodies.len()];
        let mut at = |t: f64| {
            m.frame(&w, &cams, 0, eye, t, None, &[], &mut fx, &mut audio);
            let p = m.pieces.iter().find(|p| p.q0 == (n + b) as u32 * 1000).expect("the leg's box drawn");
            let got = Mat4::from_cols(Vec4::from(p.x[0]), Vec4::from(p.x[1]), Vec4::from(p.x[2]), Vec4::W).transpose();
            let pose = m.list[0].0.model.pose(Some("walk"), t as f32)[leg];
            (got, got * frame.inverse() * pose.inverse())
        };
        let ((a, cell_a), (c, cell_c)) = (at(0.1), at(0.35));
        assert!(!a.abs_diff_eq(c, 1e-3), "the leg's box moved between the two times");
        assert!(cell_a.abs_diff_eq(cell_c, 1e-3), "and moved as its leg did:\n{cell_a}\n{cell_c}");
    }

    /// ADR 0116: the inventory's large ape is drawn from its painting (it was made without one, and the painted
    /// ape's voxels are empty).
    #[test]
    fn the_inventorys_figure_is_its_painting() {
        let l = shipped(FIGURE);
        let fig = l.pictures[1].as_ref().expect("the figure's picture");
        let filled: usize = fig.runs.iter().map(|r| r.2 as usize).sum();
        assert!(filled > (FIGURE_PX as usize).pow(2) / 8, "it fills its square: {filled} px");
        assert!(l.pictures[0].is_some(), "and a slot's");
    }

    /// ADR 0116: every model of `data/models` together stays under `MAX_MQUADS` with 30 % to spare.
    #[test]
    fn every_shipped_model_fits_the_quads_with_room_to_spare() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/models");
        let mut total = 0;
        for f in std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "toml")) {
            let l = shipped(&f.file_stem().unwrap().to_string_lossy());
            assert!(l.quads.iter().any(|q| !q.is_empty()), "{}: nothing to draw", l.name);
            total += l.quads.iter().map(Vec::len).sum::<usize>();
        }
        assert!(total * 10 <= MAX_MQUADS * 7, "{total} quads of {MAX_MQUADS}");
    }
}
