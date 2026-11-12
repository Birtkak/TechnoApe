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
}

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
        match std::fs::read_to_string(&f).map_err(|e| format!("{shown}: {e}")).and_then(|t| parse_life(&shown, &t, &palette, &effect, &sound)) {
            Ok((model, life)) => {
                let quads = model.parts.iter().map(|p| mesh(p).iter().map(|q| pack(q, model.rgb[q.colour as usize])).collect()).collect();
                let l = Loaded { name, model, life, quads, scale: 1.0 };
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
    /// `take` swapped in new models since the last frame.
    reloaded: bool,
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
            if let Some(spec) = &spec {
                creature_walk(&mut l, spec, &std::fs::read_to_string("data/creatures.toml").unwrap_or_default());
            }
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
            states: Default::default(),
            runs: Default::default(),
            held: Vec::new(),
            reach,
            lamps: Vec::new(),
            cells: Vec::new(),
            pieces: Vec::new(),
            draws: Vec::new(),
            reloaded: false,
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

    pub fn has(&self, name: &str) -> bool {
        self.list.iter().any(|(l, _)| l.name == name)
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
        eprintln!("[technoape] models: {} loaded, {} quads", self.list.len(), quads.len());
    }

    /// This frame's pieces and draws, uploaded. `cams`: per body the camera there and the turn into the
    /// camera's frame (frame.rs); `eye`: the camera's place, look and up in body `cf`'s frame; `hand`: the
    /// block in hand by name, drawn at the lower right when a model has its name.
    #[allow(clippy::too_many_arguments)]
    pub fn frame(&mut self, world: &World, cams: &[(DVec3, DMat3)], cf: usize, eye: (DVec3, DVec3, DVec3), t: f64, hand: Option<&str>, fx: &mut crate::fx::Fx, audio: &mut crate::audio::Audio) {
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
            let [o, x, y, z] = cell_axes(world, b);
            let place = |p: Vec3| {
                let p = FLOOR + (p - FLOOR) * l.scale;
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
                    inst[m].push((c, t, cell_frame(world, cams, *b) * self.spec_place, b.body as u32, self.spec_scale.unwrap_or(self.list[m].0.scale)));
                }
            }
        }
        for (b, id) in &world.models {
            let Some(m) = BLOCKS.get(*id).and_then(|d| d.model.as_deref()).and_then(by_name) else { continue };
            let x = cell_frame(world, cams, *b);
            if (x.w_axis.truncate().length() as f64) < NEAR_M {
                let (c, t) = posed.get(b).copied().unwrap_or((idle(m), t));
                inst[m].push((c, t, x, b.body as u32, 1.0));
            }
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
        if let Some(m) = hand.and_then(by_name) {
            let (_, fwd, up) = eye;
            let right = fwd.cross(up).normalize();
            let up = right.cross(fwd);
            let s = 0.35;
            let o = fwd * 0.8 + right * 0.42 - up * 0.36 - (right + up - fwd) * (s * 0.5);
            let x = DMat4::from_cols((right * s).extend(0.0), (up * s).extend(0.0), (-fwd * s).extend(0.0), o.extend(1.0));
            inst[m].push((idle(m), t, (DMat4::from_mat3(cams[cf].1) * x).as_mat4(), cf as u32, 1.0));
        }
        for (m, list) in inst.iter().enumerate() {
            if list.is_empty() {
                continue;
            }
            let (l, parts) = &self.list[m];
            // each one's scale about its cell's floor centre (a creature's in its cells only, never in hand or as a particle)
            let sc = |s: f32| Mat4::from_translation(FLOOR) * Mat4::from_scale(Vec3::splat(s)) * Mat4::from_translation(-FLOOR);
            let poses: Vec<Vec<Mat4>> = list.iter().map(|(c, t, _, _, _)| l.model.pose(c.map(|c| l.model.clips[c].name.as_str()), *t as f32)).collect();
            for (pi, (q0, nq)) in parts.iter().enumerate() {
                if *nq == 0 || self.pieces.len() + list.len() > MAX_PIECES {
                    continue;
                }
                let first = self.pieces.len() as u32;
                for (k, (_, _, x, body, s)) in list.iter().enumerate() {
                    let r = (*x * sc(*s) * poses[k][pi]).transpose();
                    self.pieces.push(Piece { x: [r.x_axis.to_array(), r.y_axis.to_array(), r.z_axis.to_array()], q0: *q0, body: *body, _p: [0; 2] });
                }
                self.draws.push(Draw { quads: *nq, first, count: list.len() as u32 });
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

/// A cell's frame in the camera's (`cell_axes`), the corner at the cell's.
fn cell_frame(world: &World, cams: &[(DVec3, DMat3)], b: Block) -> Mat4 {
    let [o, x, y, z] = cell_axes(world, b);
    let (cam, turn) = cams[b.body as usize];
    (DMat4::from_mat3(turn) * DMat4::from_cols(x.extend(0.0), y.extend(0.0), z.extend(0.0), (o - cam).extend(1.0))).as_mat4()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn furnace(effects: bool) -> Loaded {
        let p = engine::palette::Palette::parse(include_str!("../../../data/palette.toml")).unwrap();
        let mut text = include_str!("../../../data/models/furnace.toml").replace("\r\n", "\n");
        if !effects {
            text = text.replace("effects = [{ effect = \"fire\", point = \"flame\" }, { effect = \"smoke\", point = \"chimney\" }]\n", "");
        }
        let (model, life) = parse_life("furnace.toml", &text, &p, &|_| true, &|_| true).unwrap();
        assert_eq!(life.states.iter().find(|s| s.name == "working").unwrap().effects.is_empty(), !effects);
        Loaded { name: "furnace".into(), model, life, quads: Vec::new(), scale: 1.0 }
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
                m.frame(&w, &cams, 0, eye, t, None, fx, &mut audio);
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
        let p = engine::palette::Palette::parse(include_str!("../../../data/palette.toml")).unwrap();
        let (model, life) = parse_life("boar.toml", include_str!("../../../data/models/boar.toml"), &p, &|_| true, &|_| true).unwrap();
        let mut list = vec![Loaded { name: "boar".into(), quads: vec![Vec::new(); model.parts.len()], model, life, scale: 1.0 }];
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
            m.frame(&w, &cams, 0, eye, t, in_hand.then_some("boar"), &mut fx, &mut audio);
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
        m.list = vec![(Loaded { name: "boar".into(), quads: vec![Vec::new(); model.parts.len()], model, life, scale: 1.0 }, parts)];
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

    /// The Studio's view walks a creature about: the shown model stands `at` metres from its cell along the
    /// cell's x and z, turned by `yaw` about its floor centre, and is otherwise drawn as it was.
    #[test]
    fn the_view_draws_the_shown_model_where_and_turned_as_given() {
        let (at, yaw) = ([2.0, -1.0], 0.7f32);
        let ((was, _), (now, _)) = (boar_drawn(1.0, None, false, None), boar_drawn(1.0, None, false, Some((at, yaw))));
        let w = World::new(768, 256, 1);
        let b = &w.bodies[0];
        let x = cell_frame(&w, &vec![(DVec3::ZERO, DMat3::IDENTITY); w.bodies.len()], w.block_at(b.spawn * (b.height(b.spawn) + 0.1)));
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
}
