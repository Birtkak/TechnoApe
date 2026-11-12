//! The Models page (docs/design/studio-tools.md "The Models page", ADR 0049): `data/models/*.toml` drawn by the
//! game's renderer ([`Viewport`]), edited with voxel tools, parts and pivots, points and a timeline of keys. The
//! editing is `voxel`'s, pure and tested; this maps the pointer and keys to it.
use super::look::{self, c32, GAP};
use super::textures::palette_picker;
use super::viewport::{ModelCam, Viewport, DIR};
use super::{App, Cmd, FileRow, Page, Thumb, DRAFTS};
use crate::art::{self, History};
use crate::shell::Flags;
use crate::voxel::{self, Hit, Motion, Op, Orbit, MOTIONS, VIEWS};
use eframe::egui::{self, Color32, Key, Modifiers, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};
use engine::models::{parse_life, sample, Clip, Ease, Event, Life, Light, Model, State, StateEffect};
use engine::models::VOXEL;
use glam::{IVec3, Mat4, Vec3};
use std::collections::BTreeMap;

pub const MODELS: &str = "data/models";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Tool {
    Add,
    Paint,
    Pick,
    Box,
    Line,
    Fill,
    Select,
    Pivot,
    Point,
}
/// Each tool: its command id, name, key and what it does.
pub(super) const TOOLS: [(Tool, &str, &str, Key, &str); 9] = [
    (Tool::Add, "add", "Add", Key::A, "left click adds a voxel of the first colour on the face under the pointer, right click removes one"),
    (Tool::Paint, "paint", "Paint", Key::P, "left click paints a voxel the first colour, right the second"),
    (Tool::Pick, "pick", "Picker", Key::I, "left click takes a voxel's colour as the first, right as the second"),
    (Tool::Box, "box", "Box", Key::B, "drag a box on the plane of the face first clicked: left adds, right removes"),
    (Tool::Line, "line", "Line", Key::L, "drag a line on the plane of the face first clicked: left adds, right removes"),
    (Tool::Fill, "fill", "Fill", Key::G, "left click recolours the joined run of one colour on that face's layer, right removes it"),
    (Tool::Select, "select", "Select", Key::S, "drag a box of voxels; the arrows and Shift + Page Up/Down move it; drag one of its faces to pull it out"),
    (Tool::Pivot, "pivot", "Pivot", Key::V, "drag the cross: the point the part turns about"),
    (Tool::Point, "point", "Point", Key::O, "click a face to add a point there; drag a point's cross to move it"),
];

/// What each mouse button does with a tool, in the view's bar.
fn buttons(tool: Tool) -> &'static str {
    match tool {
        Tool::Add => "left adds a voxel, right removes one",
        Tool::Paint => "left paints a voxel the first colour, right the second",
        Tool::Pick => "left takes a voxel's colour as the first, right as the second",
        Tool::Box => "left drag adds a box, right drag removes one",
        Tool::Line => "left drag adds a line, right drag removes one",
        Tool::Fill => "left recolours the joined run, right removes it",
        Tool::Select => "left drag selects a box; drag one of its faces to pull it out",
        Tool::Pivot => "left drag moves the yellow cross, the point the part turns about",
        Tool::Point => "left adds a point on a face; drag a point's cross to move it",
    }
}

/// One model file open in the Studio: the model, its undo, its text as on disk, and a count of its edits.
struct MDoc {
    m: Model,
    /// Its states and clip events (ADR 0050), kept through every edit and undo with the model.
    life: Life,
    hist: History<(Model, Life)>,
    saved: Option<String>,
    rev: u64,
    /// The file's text as opened: its comments and line ends are kept when it is saved (`write_keeping`).
    old: String,
}
impl MDoc {
    /// A file's text as the page opens it (its effects and sounds are checked on save, not here).
    fn load(path: &str, text: &str, pal: &engine::palette::Palette) -> Result<MDoc, String> {
        let (m, life) = parse_life(path, text, pal, &|_| true, &|_| true)?;
        let mut d = MDoc { m, life, hist: History::default(), saved: None, rev: 0, old: text.to_string() };
        d.saved = Some(d.text());
        Ok(d)
    }
    /// The file's text as the page saves it.
    fn text(&self) -> String {
        voxel::text_keeping(&self.old, &self.m, &self.life)
    }
    /// An undo, redo or step back on the model and its life together.
    fn step(&mut self, f: impl FnOnce(&mut History<(Model, Life)>, &mut (Model, Life)) -> bool) -> bool {
        let mut now = (self.m.clone(), std::mem::take(&mut self.life));
        let done = f(&mut self.hist, &mut now);
        (self.m, self.life) = now;
        done
    }
    fn dirty(&self) -> bool {
        self.saved.as_deref() != Some(self.text().as_str())
    }
}

/// The unsaved models as files for the recovery set (`recover.rs`): `<name>.toml` and the text a save writes.
fn kept(docs: &BTreeMap<String, MDoc>) -> Vec<(String, Vec<u8>)> {
    docs.iter().filter(|(_, d)| d.dirty()).map(|(n, d)| (format!("{n}.toml"), d.text().into_bytes())).collect()
}

/// Of what a crash left, what is not the file as saved (that much is offered back).
fn left(docs: &BTreeMap<String, MDoc>, files: Vec<(String, Vec<u8>)>) -> Vec<(String, Vec<u8>)> {
    files.into_iter().filter(|(f, b)| docs.get(f.trim_end_matches(".toml")).and_then(|d| d.saved.as_deref()) != Some(String::from_utf8_lossy(b).as_ref())).collect()
}

/// What a crash left put back as unsaved models, one undo step each ("Recovered"); a file the game's reader
/// refuses is given back (file, bytes, why) to be set aside.
fn restore(docs: &mut BTreeMap<String, MDoc>, files: Vec<(String, Vec<u8>)>, pal: &engine::palette::Palette) -> Vec<(String, Vec<u8>, String)> {
    let mut errors = vec![];
    for (f, b) in files {
        let name = f.trim_end_matches(".toml").to_string();
        let new = String::from_utf8(b.clone()).map_err(|e| e.to_string()).and_then(|t| MDoc::load(&format!("{MODELS}/{f}"), &t, pal));
        match (new, docs.get_mut(&name)) {
            (Err(e), _) => errors.push((f, b, e)),
            (Ok(new), Some(d)) => {
                let before = (std::mem::replace(&mut d.m, new.m), std::mem::replace(&mut d.life, new.life));
                d.hist.push(before, "Recovered");
                d.rev += 1;
            }
            (Ok(new), None) => {
                docs.insert(name, MDoc { saved: None, ..new });
            }
        }
    }
    errors
}

/// A drag in the view: where it began, with which button.
#[derive(Clone, Copy)]
struct Drag {
    hit: Hit,
    from: Pos2,
    right: bool,
    /// What it moves: a pivot, a point, a selection's face.
    what: Grab,
    /// Begun on a plane (the layer's, the floor's): its end is where the pointer meets that plane too.
    plane: bool,
}
#[derive(Clone, Copy, PartialEq)]
enum Grab {
    Cells,
    Pivot,
    Point(usize),
    Face,
}

pub struct Models {
    docs: BTreeMap<String, MDoc>,
    open: Option<String>,
    part: usize,
    tool: Tool,
    first: [u8; 3],
    second: [u8; 3],
    mirror: (bool, bool),
    /// Layer mode: the chosen part's row whose plane the clicks land on; the rows above it are dimmed.
    layer: Option<i32>,
    /// The hover came from a plane (the layer's, or the floor's where no voxel is under the pointer).
    on_plane: bool,
    /// Thin dark lines between voxels.
    edges: bool,
    /// What the overlay draws of the voxels, found for its key: the faces the eye sees (in metres, for the
    /// edges) and the dimmed rows above the layer (far first, with their colour); and whether it is the view's now.
    faces: (String, Vec<[Vec3; 4]>, Vec<([Vec3; 4], [u8; 3])>),
    faces_now: bool,
    sel: Option<(IVec3, IVec3)>,
    drag: Option<Drag>,
    hover: Option<Hit>,
    clip: Option<usize>,
    t: f32,
    playing: bool,
    /// The key chosen on the timeline (for its ease).
    key: Option<usize>,
    motion: (Motion, usize, f32),
    orbit: Orbit,
    /// The flat (near orthographic) view.
    flat: bool,
    /// The starter shapes are shown for a new model (Ctrl+N), in place of the view.
    choosing: bool,
    show_block: bool,
    show_player: bool,
    show_grid: bool,
    strength: f32,
    tex_block: String,
    part_name: String,
    /// What the renderer was last given: the model's name, edit count, slice, and how often it was written.
    written: Option<(String, u64, Option<f32>)>,
    gen: u64,
    /// One history step for a whole drag of a value.
    merging: Option<String>,
    /// The model before [`Self::colour`] added a colour to it: the next edit's undo step starts there.
    pre: Option<(Model, Life)>,
    /// The names a state or event may pick: `data/effects.toml`'s rows and groups, `data/sounds.toml`'s rows.
    effects: Vec<String>,
    sounds: Vec<String>,
    /// The state chosen in the inspector, the one the view plays (none: the timeline's clip), and the event
    /// chosen on the timeline; the name boxes being typed in.
    state: Option<usize>,
    playing_state: Option<String>,
    event: Option<usize>,
    names: [String; 3],
    /// The point chosen in the tree, and whether a drag on the timeline moves the chosen event.
    point: Option<usize>,
    moving_event: bool,
    /// When the files on disk were last looked at, and a model changed there under unsaved work (its name).
    looked: f64,
    ask: Option<String>,
}

impl Models {
    pub fn new(app: &App) -> Models {
        let at = |i: usize| app.pal.colours.get(i).or(app.pal.colours.first()).map_or([128; 3], |c| c.rgb);
        let mut docs = BTreeMap::new();
        let mut names: Vec<String> = std::fs::read_dir(MODELS).into_iter().flatten().flatten().map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.ends_with(".toml")).collect();
        names.sort();
        for n in names {
            let path = format!("{MODELS}/{n}");
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            // a refused file is listed by the file check, not opened
            // (a file read as it is on disk counts as saved when it writes back to other text: comments, spacing)
            if let Ok(d) = MDoc::load(&path, &text, &app.pal) {
                docs.insert(n.trim_end_matches(".toml").to_string(), d);
            }
        }
        let mut s = Models {
            open: docs.keys().next().cloned(),
            docs,
            part: 0,
            tool: Tool::Add,
            first: at(1),
            second: at(0),
            mirror: (false, false),
            layer: None,
            on_plane: false,
            edges: true,
            faces: Default::default(),
            faces_now: false,
            sel: None,
            drag: None,
            hover: None,
            clip: None,
            t: 0.0,
            playing: false,
            key: None,
            motion: (Motion::Swing, 1, 30.0),
            orbit: Orbit::default(),
            flat: false,
            choosing: false,
            show_block: false,
            show_player: false,
            show_grid: true,
            strength: 3.0,
            tex_block: String::new(),
            part_name: String::new(),
            written: None,
            gen: 0,
            merging: None,
            pre: None,
            effects: std::fs::read_to_string(super::effects::EFFECTS).ok().and_then(|t| client::fx::parse_set(&t, &app.pal).ok()).map_or(vec![], |s| s.names().into_iter().map(String::from).collect()),
            sounds: std::fs::read_to_string("data/sounds.toml").ok().and_then(|t| client::audio::parse(&t).ok()).map_or(vec![], |r| r.into_iter().map(|r| r.name).collect()),
            state: None,
            playing_state: None,
            event: None,
            names: Default::default(),
            point: None,
            moving_event: false,
            looked: 0.0,
            ask: None,
        };
        s.opened();
        s
    }

    fn doc(&self) -> Option<&MDoc> {
        self.open.as_ref().and_then(|n| self.docs.get(n))
    }
    fn m(&self) -> Option<&Model> {
        self.doc().map(|d| &d.m)
    }
    fn opened(&mut self) {
        let (parts, clip) = self.m().map_or((0, None), |m| (m.parts.len(), (!m.clips.is_empty()).then_some(0)));
        self.part = self.part.min(parts.saturating_sub(1));
        (self.clip, self.t, self.key, self.sel, self.drag, self.playing) = (clip, 0.0, None, None, None, false);
        let states = self.doc().map_or(0, |d| d.life.states.len());
        (self.state, self.playing_state, self.event) = ((states > 0).then_some(0), None, None);
        self.part_name = self.m().and_then(|m| m.parts.get(self.part)).map_or(String::new(), |p| p.name.clone());
    }
    fn clip_name(&self) -> Option<String> {
        let m = self.m()?;
        m.clips.get(self.clip?).map(|c| c.name.clone())
    }
    fn poses(&self) -> Vec<Mat4> {
        self.m().map_or(vec![], |m| m.pose(self.clip_name().as_deref(), self.t))
    }

    /// One named edit of the open model: a history step when `f` changed it.
    fn edit(&mut self, app: &mut App, name: &str, f: impl FnOnce(&mut Model, &engine::palette::Palette) -> bool) {
        self.edit_life(app, name, |m, _, pal| f(m, pal));
    }
    /// An edit of the model and its life; the life is then kept true to the model (`voxel::fit_life`).
    fn edit_life(&mut self, app: &mut App, name: &str, f: impl FnOnce(&mut Model, &mut Life, &engine::palette::Palette) -> bool) {
        let pal = app.pal.clone();
        let merge = self.merging.as_deref() == Some(name);
        let pre = self.pre.take();
        let Some(d) = self.open.as_ref().and_then(|n| self.docs.get_mut(n)) else { return };
        let before = pre.unwrap_or_else(|| (d.m.clone(), d.life.clone()));
        if f(&mut d.m, &mut d.life, &pal) {
            voxel::fit_life(&d.m, &mut d.life);
            if (&d.m, &d.life) != (&before.0, &before.1) {
                if !merge {
                    d.hist.push(before, name);
                }
                d.rev += 1;
            }
        } else {
            (d.m, d.life) = before;
        }
    }
    fn edit_life_merged(&mut self, app: &mut App, name: &str, f: impl FnOnce(&mut Model, &mut Life, &engine::palette::Palette) -> bool) {
        self.edit_life(app, name, f);
        self.merging = Some(name.into());
    }
    /// A drag's edits are one step: this one, until the pointer is let go.
    fn edit_merged(&mut self, app: &mut App, name: &str, f: impl FnOnce(&mut Model, &engine::palette::Palette) -> bool) {
        self.edit(app, name, f);
        self.merging = Some(name.into());
    }

    fn colour(&mut self, app: &mut App, rgb: [u8; 3]) -> Option<u8> {
        let pal = app.pal.clone();
        let d = self.open.as_ref().and_then(|n| self.docs.get_mut(n))?;
        let before = (d.m.colours.len(), (d.m.clone(), d.life.clone()));
        let k = voxel::colour_index(&mut d.m, &pal, rgb);
        if d.m.colours.len() != before.0 && self.pre.is_none() {
            self.pre = Some(before.1);
        }
        if k.is_none() {
            app.refuse("That colour is not in the palette, or the model holds 62 colours already");
        }
        k
    }

    fn save(&mut self, app: &mut App) {
        let Some(name) = self.open.clone() else { return };
        let Some(d) = self.docs.get_mut(&name) else { return };
        let text = d.text();
        let path = format!("{MODELS}/{name}.toml");
        // the game's own reader first: a model it would refuse is never written (its effects and sounds too)
        let (fx, snd) = (&self.effects, &self.sounds);
        if let Err(e) = parse_life(&path, &text, &app.pal, &|n| fx.is_empty() || fx.iter().any(|x| x == n), &|n| snd.is_empty() || snd.iter().any(|x| x == n)) {
            return app.refuse(format!("Not saved: {e}"));
        }
        let _ = std::fs::create_dir_all(MODELS);
        let tmp = format!("{path}.tmp");
        match std::fs::write(&tmp, &text).and_then(|_| std::fs::rename(&tmp, &path)) {
            Ok(()) => {
                d.saved = Some(text);
                let drafts = art::set_draft(&std::fs::read_to_string(DRAFTS).unwrap_or_default(), &path, false);
                app.write_drafts(drafts);
                app.note(&path);
                app.toast(format!("Saved {name}.toml"));
                app.check();
            }
            Err(e) => {
                let _ = std::fs::remove_file(&tmp);
                app.refuse(format!("Not saved: {path}: {e}"));
            }
        }
    }

    /// A new model named after `base` (a free name: "post", "post 2", ...), made by `make` from that name's
    /// number (a plant's seed: "tuft 3" is seed 3); none made, nothing opens.
    fn new_model(&mut self, base: &str, make: impl FnOnce(u64) -> Option<Model>) {
        let (k, name) = (1u64..).map(|k| (k, if k == 1 { base.to_string() } else { format!("{base} {k}") })).find(|(_, n)| !self.docs.contains_key(n) && !std::path::Path::new(&format!("{MODELS}/{n}.toml")).exists()).unwrap();
        let Some(m) = make(k) else { return };
        self.docs.insert(name.clone(), MDoc { m, life: Life::default(), hist: History::default(), saved: None, rev: 0, old: String::new() });
        self.open = Some(name);
        self.part = 0;
        self.choosing = false;
        self.opened();
    }

    /// In place of the view, for a new model and when there is none: the starter shapes, each a button with its
    /// words, and what the first click does.
    fn starters(&mut self, ui: &mut Ui, app: &mut App) {
        ui.add_space(GAP);
        if self.m().is_none() {
            look::heading(ui, "No model yet: click a shape to start from");
            look::weak(ui, "It opens in the view. There a left click adds a voxel of the chosen colour, a right click removes one, and a right drag turns the view.");
        } else {
            look::heading(ui, "New model: click a shape to start from");
            look::weak(ui, "It opens as a new model beside the others, to recolour and reshape; the open one is kept as it is.");
        }
        ui.add_space(GAP);
        for (id, label, help) in voxel::STARTERS {
            ui.horizontal(|ui| {
                let r = ui.add_sized([130.0, 28.0], egui::Button::new(label));
                crate::drive::mark(ui.ctx(), format!("starter:{id}"), r.rect);
                if look::tip(r, help, "").clicked() {
                    self.run(app, &format!("new:{id}"));
                }
                // (in a row a label does not wrap: a column the row's width left)
                ui.allocate_ui_with_layout(Vec2::new(ui.available_width(), 28.0), egui::Layout::top_down(egui::Align::Min), |ui| ui.add(egui::Label::new(RichText::new(help).small().weak()).wrap()));
            });
        }
        ui.add_space(GAP);
        ui.horizontal_wrapped(|ui| {
            look::weak(ui, "Or grow a small plant:");
            for kind in voxel::PLANTS {
                if look::tip(ui.button(kind), "A small plant grown as a new model, to finish by hand; again grows another", "").clicked() {
                    self.run(app, &format!("plant:{kind}"));
                }
            }
        });
        if self.m().is_some() {
            ui.add_space(GAP);
            if look::tip(ui.button("Cancel"), "Back to the open model", "Esc").clicked() {
                self.choosing = false;
            }
        }
    }

    /// The open model as the renderer reads it (sliced), written when it changed.
    fn feed_view(&mut self, app: &mut App) {
        let Some(name) = self.open.clone() else {
            app.view.show_model(None, None, 0.0);
            return;
        };
        let rev = self.docs[&name].rev;
        let top = self.layer_top();
        let now = Some((name.clone(), rev, top));
        if self.written != now {
            let (m, life) = (&self.docs[&name].m, &self.docs[&name].life);
            let shown = match top {
                Some(s) => voxel::sliced(m, &m.pose(None, 0.0), s),
                None => m.clone(),
            };
            let dir = format!("{DIR}/models");
            let _ = std::fs::create_dir_all(&dir);
            // the unsaved copy with its life, so a state plays before it is saved
            if std::fs::write(format!("{dir}/studio.toml"), voxel::text_life(&shown, life)).is_ok() {
                self.gen += 1;
                self.written = now;
            }
        }
        app.view.models(self.gen);
        app.view.show_model(Some("studio"), self.clip_name().as_deref(), self.t);
        app.view.show_state(self.playing_state.as_deref());
        // the 1 m block for scale: a real block of stone two cells to the -x
        app.view.place = if self.show_block { "0:-2:0:stone".into() } else { String::new() };
    }

    /// The box of a part's cells `lo`..=`hi` in the model's metres, posed as drawn.
    fn cells_box(&self, part: usize, lo: IVec3, hi: IVec3) -> Option<(Vec3, Vec3)> {
        let pose = *self.poses().get(part)?;
        let (lo, hi) = (lo.min(hi).as_vec3(), lo.max(hi).as_vec3() + 1.0);
        let pts = (0..8).map(|k: u8| pose.transform_point3(Vec3::new([lo.x, hi.x][(k & 1) as usize], [lo.y, hi.y][(k >> 1 & 1) as usize], [lo.z, hi.z][(k >> 2 & 1) as usize])));
        Some(pts.fold((Vec3::INFINITY, Vec3::NEG_INFINITY), |(a, b), p| (a.min(p), b.max(p))))
    }
    /// A part's whole box, in metres.
    fn part_box(&self, part: usize) -> Option<(Vec3, Vec3)> {
        let s = self.m()?.parts.get(part)?.size;
        self.cells_box(part, IVec3::ZERO, IVec3::from(s.map(|v| v as i32)) - 1)
    }
    /// What the view turns about: the selection's middle, else the model's.
    fn turn_point(&self) -> Option<Vec3> {
        let b = match self.sel {
            Some((a, b)) => self.cells_box(self.part, a, b),
            None => (0..self.m()?.parts.len()).filter_map(|i| self.part_box(i)).reduce(|(a, b), (c, d)| (a.min(c), b.max(d))),
        }?;
        Some((b.0 + b.1) / 2.0)
    }
    /// F: the selection, else the chosen part, fills the view.
    fn frame(&mut self, app: &App) {
        let b = match self.sel {
            Some((a, b)) => self.cells_box(self.part, a, b),
            None => self.part_box(self.part),
        };
        if let Some((lo, hi)) = b {
            self.orbit.frame(lo, hi, app.view.half_height());
        }
    }

    /// The view cube in the view's top right corner: the axes in colour (x red, y green and up, z blue), a click
    /// on a face looks from there; under it the flat view and framing as buttons.
    fn view_cube(&mut self, ui: &mut Ui, app: &App, view: Rect) {
        let half = 28.0;
        let c = Pos2::new(view.max.x - half * 2.1, view.min.y + half * 2.1);
        let area = Rect::from_center_size(c, Vec2::splat(half * 4.0));
        if !view.contains_rect(area) {
            return;
        }
        let resp = ui.interact(area, egui::Id::unique("model view cube"), Sense::click());
        let p = ui.painter_at(area);
        let (f, r, u) = self.orbit.basis();
        let to2 = |v: Vec3| c + Vec2::new(v.dot(r), -v.dot(u)) * half;
        p.circle_filled(c, half * 1.9, Color32::from_black_alpha(110));
        // the axes from the middle, drawn first: what points away goes behind the cube
        for (a, col, name) in [(Vec3::X, Color32::from_rgb(230, 80, 70), "x"), (Vec3::Y, Color32::from_rgb(110, 210, 90), "y"), (Vec3::Z, Color32::from_rgb(80, 140, 240), "z")] {
            p.line_segment([c, to2(a * 1.7)], Stroke::new(2.0, col));
            p.text(to2(a * 1.85), egui::Align2::CENTER_CENTER, name, egui::FontId::proportional(look::SMALL), col);
        }
        let faces = [(Vec3::Z, 0), (-Vec3::Z, 1), (Vec3::X, 2), (-Vec3::X, 3), (Vec3::Y, 4), (-Vec3::Y, 5)];
        let mut over = None;
        let pointer = resp.hover_pos();
        for (n, k) in faces {
            let facing = n.dot(-f);
            if facing < 0.02 {
                continue;
            }
            let (a, b) = if n.y != 0.0 { (Vec3::X, Vec3::Z) } else { (Vec3::Y, n.cross(Vec3::Y)) };
            let pts: Vec<Pos2> = [n + a + b, n + a - b, n - a - b, n - a + b].iter().map(|v| to2(*v)).collect();
            // inside the face's quad: on the same side of each of its edges
            let side = |i: usize, q: Pos2| (pts[(i + 1) % 4] - pts[i]).rot90().dot(q - pts[i]);
            let inside = pointer.is_some_and(|q| {
                let s: Vec<f32> = (0..4).map(|i| side(i, q)).collect();
                s.iter().all(|v| *v >= 0.0) || s.iter().all(|v| *v <= 0.0)
            });
            if inside {
                over = Some(k);
            }
            let axis: [u8; 3] = if n.x != 0.0 { [150, 70, 64] } else if n.y != 0.0 { [80, 140, 70] } else { [64, 96, 160] };
            let fill = c32(axis.map(|v| (v as f32 * (0.55 + 0.45 * facing) + if inside { 50.0 } else { 0.0 }).min(255.0) as u8));
            p.add(egui::Shape::convex_polygon(pts, fill, Stroke::new(1.0, Color32::from_black_alpha(150))));
            if facing > 0.35 {
                p.text(to2(n), egui::Align2::CENTER_CENTER, VIEWS[k].1, egui::FontId::proportional(look::SMALL * 0.8), Color32::WHITE);
            }
            crate::drive::mark(ui.ctx(), format!("cube:{}", VIEWS[k].0), Rect::from_center_size(to2(n), Vec2::splat(2.0)));
        }
        if let (true, Some(k)) = (resp.clicked(), over) {
            self.orbit.look_from(k);
        }
        let tip = match over {
            Some(k) => format!("Look from the {} (the model faces front, +z, in the hand and in the world)", VIEWS[k].1.to_lowercase()),
            None => "The view cube: click a face to look from that side. The axes: x red, y green and up, z blue".into(),
        };
        look::tip(resp, &tip, "1 3 7, with Shift the far side");
        // under the cube: the flat view and framing, as buttons
        let row = Rect::from_min_max(Pos2::new(view.max.x - 128.0, area.max.y), Pos2::new(view.max.x - 6.0, area.max.y + 22.0));
        if view.contains_rect(row) {
            let mut b = ui.new_child(egui::UiBuilder::new().max_rect(row).layout(egui::Layout::left_to_right(egui::Align::Center)));
            b.spacing_mut().item_spacing = Vec2::splat(4.0);
            if look::tip(look::chip(&mut b, self.flat, "Flat"), "A flat view: no perspective, so rows and sizes line up (again for perspective)", "5").clicked() {
                self.flat = !self.flat;
            }
            if look::tip(b.button(RichText::new("Frame").small()), "Fill the view with the selection, else the chosen part", "F").clicked() {
                self.frame(app);
            }
        }
    }

    /// Layer mode's height in the model's voxels, at rest: the rows above the chosen part's layer row are hidden.
    fn layer_top(&self) -> Option<f32> {
        let l = self.layer?;
        let pose = *self.m()?.pose(None, 0.0).get(self.part)?;
        Some(pose.transform_point3(Vec3::new(0.0, l as f32 + 1.0, 0.0)).y / VOXEL)
    }

    /// Where a left click adds and which voxel a right click removes, for a hover.
    fn aims(&self, h: Hit) -> (Option<IVec3>, Option<IVec3>) {
        let add = if self.on_plane { (!h.solid).then_some(h.cell) } else { Some(h.target()) };
        (add, h.solid.then_some(h.cell))
    }

    /// The faces the overlay draws, found again when the model, the layer, the pose or the eye changed: the ones
    /// the eye sees of the shown voxels (the layer's rows and under), for the edges, and the rows above the layer.
    /// While a button is held (the view turning) they are not found again and not drawn.
    /// shortcut: a face counts as seen by its middle, so one half hidden draws whole; clip per edge if it shows.
    /// shortcut: the layer's rows are cut at rest (`layer_top`) and the faces drawn posed: a clip scrubbed away from
    /// rest moves the plane and the faces off the cut; leave layer mode for that, or cut posed, when it matters.
    fn find_faces(&mut self, view: &Viewport, held: bool) {
        let Some(rev) = self.doc().map(|d| d.rev) else { return };
        let eye = view.cam().eye();
        let key = format!("{:?} {} {:?} {:?} {:?} {} {}", self.open, rev, self.layer_top(), eye, self.clip, self.t, self.edges);
        self.faces_now = self.faces.0 == key;
        if self.faces_now || held {
            return;
        }
        let d = self.doc().unwrap();
        let poses = self.poses();
        let facing = |i: usize, q: &[Vec3; 4], n: IVec3| poses[i].transform_vector3(n.as_vec3()).dot(eye - (q[0] + q[2]) / 2.0) > 0.0;
        let m = match self.layer_top() {
            Some(top) => voxel::sliced(&d.m, &d.m.pose(None, 0.0), top),
            None => d.m.clone(),
        };
        let mut edges = vec![];
        for (i, p) in m.parts.iter().enumerate().filter(|_| self.edges) {
            for (c, k) in voxel::exposed(p) {
                let n = voxel::NORMALS[k];
                let q = voxel::face_quad(c, n).map(|v| poses[i].transform_point3(v));
                // facing the eye, and the first face a ray to its middle meets
                if facing(i, &q, n) && voxel::pick(&m, &poses, None, eye, ((q[0] + q[2]) / 2.0 - eye).normalize()).is_some_and(|h| h.solid && h.part == i && h.cell == c && h.n == n) {
                    edges.push(q);
                }
            }
        }
        let mut dim = vec![];
        if self.layer.is_some() {
            for (i, (a, b)) in d.m.parts.iter().zip(&m.parts).enumerate() {
                let mut above = a.clone();
                for (c, k) in above.cells.iter_mut().zip(&b.cells) {
                    if *k != 0 {
                        *c = 0;
                    }
                }
                for (c, k) in voxel::exposed(&above) {
                    let q = voxel::face_quad(c, voxel::NORMALS[k]).map(|v| poses[i].transform_point3(v));
                    if facing(i, &q, voxel::NORMALS[k]) {
                        let rgb = d.m.rgb.get(voxel::get(&above, c) as usize - 1).copied().unwrap_or([128; 3]);
                        dim.push((((q[0] + q[2]) / 2.0).distance(eye), q, rgb));
                    }
                }
            }
            dim.sort_by(|a, b| b.0.total_cmp(&a.0));
        }
        self.faces = (key, edges, dim.into_iter().map(|(_, q, c)| (q, c)).collect());
        self.faces_now = true;
    }

    /// The bar in the view's top left, always: the tool and what the buttons do, how the view moves, and what
    /// is on (layer, mirror, flat).
    fn view_bar(&self, ui: &Ui, view: Rect) {
        let name = TOOLS.iter().find(|t| t.0 == self.tool).map_or("", |t| t.2);
        let turn = if matches!(self.tool, Tool::Box | Tool::Line) { "Alt + left drag turns" } else { "Right drag (or Alt + left) turns" };
        let mut lines = vec![format!("{name}: {}.", buttons(self.tool)), format!("{turn}, middle drag pans, the wheel zooms.")];
        let mut on = vec![];
        if let Some(l) = self.layer {
            on.push(format!("Layer: row {l}, clicks land on its plane (Page Up/Down move it)"));
        }
        match self.mirror {
            (true, true) => on.push("Mirror x and z".into()),
            (true, false) => on.push("Mirror x".into()),
            (false, true) => on.push("Mirror z".into()),
            _ => {}
        }
        if self.flat {
            on.push("Flat view".into());
        }
        if !on.is_empty() {
            lines.push(on.join(". ") + ".");
        }
        let p = ui.painter_at(view);
        let font = egui::FontId::proportional(look::SMALL);
        let galleys: Vec<_> = lines.into_iter().enumerate().map(|(i, l)| p.layout_no_wrap(l, font.clone(), if i == 0 { Color32::WHITE } else { Color32::from_gray(200) })).collect();
        let w = galleys.iter().map(|g| g.size().x).fold(0.0, f32::max);
        let h: f32 = galleys.iter().map(|g| g.size().y).sum();
        let bg = Rect::from_min_size(view.min + Vec2::splat(6.0), Vec2::new(w + 12.0, h + 8.0));
        p.rect_filled(bg, 4.0, Color32::from_black_alpha(170));
        let mut y = bg.min.y + 4.0;
        for g in galleys {
            let gh = g.size().y;
            p.galley(Pos2::new(bg.min.x + 6.0, y), g, Color32::WHITE);
            y += gh;
        }
        crate::drive::mark(ui.ctx(), "view_bar", bg);
    }

    fn timeline_t(&mut self, t: f32) {
        let len = self.clip.and_then(|c| self.m()?.clips.get(c).map(|c| c.length)).unwrap_or(1.0);
        self.t = t.clamp(0.0, len);
    }

    /// The pointer in the view: what each tool does.
    fn pointer(&mut self, app: &mut App, resp: &egui::Response) {
        let Some(m) = self.m().cloned() else { return };
        if self.drag.is_some() && !resp.ctx.input(|i| i.pointer.any_down()) && !resp.drag_stopped() {
            (self.drag, self.merging) = (None, None);
        }
        let poses = self.poses();
        let part = self.part.min(m.parts.len().saturating_sub(1));
        let pos = resp.hover_pos().or(resp.interact_pointer_pos());
        let ray = pos.and_then(|p| app.view.ray(p));
        if m.parts.is_empty() {
            self.hover = None;
            return;
        }
        // layer mode: the layer's plane; else the voxel under the pointer, else the floor, else where the ray enters the box
        let on = |l: i32| ray.and_then(|(o, d)| voxel::pick_layer(&m.parts[part], poses[part], l, o, d)).map(|h| Hit { part, ..h });
        (self.hover, self.on_plane) = match self.layer {
            Some(l) => (on(l), true),
            None => match (ray.and_then(|(o, d)| voxel::pick(&m, &poses, Some(part), o, d)), on(0)) {
                (Some(h), _) if h.solid => (Some(h), false),
                // (only on the floor's drawn grid: a click far off never grows the part)
                (_, Some(f)) if on_grid(&m.parts[part], f.cell) => (Some(f), true),
                (h, _) if h.is_some_and(|h| h.solid) => (h, false),
                (h, _) => (h, false),
            },
        };
        let pose = poses[part];
        // the status line's middle: the cell under the pointer, the part's box and the part, and the voxel's colour
        app.pointer = match self.hover {
            Some(h) => {
                let (c, p) = (h.cell, &m.parts[part]);
                let what = match voxel::get(p, c) {
                    0 if !voxel::inside(p, c) => "outside the box: an add grows it".to_string(),
                    0 => "empty".to_string(),
                    v => m.colours.get(v as usize - 1).cloned().unwrap_or_default(),
                };
                format!("x {} y {} z {} of {} x {} x {}, part {}: {what}", c.x, c.y, c.z, p.size[0], p.size[1], p.size[2], p.name)
            }
            None => String::new(),
        };
        let screen = |p: Vec3| app.view.to_screen(p);
        let pivot_at = screen(pose.transform_point3(Vec3::from(m.parts[part].pivot)));
        if let Some(q) = pivot_at {
            crate::drive::mark(&resp.ctx, "pivot", Rect::from_center_size(q, Vec2::splat(2.0)));
        }
        let point_at: Vec<Option<Pos2>> = m.points.iter().map(|pt| m.parts.iter().position(|p| p.name == pt.part).and_then(|i| screen(poses[i].transform_point3(Vec3::from(pt.at))))).collect();
        // a press starts a drag (or is a click when let go where it began); Alt + left drag turns the view instead
        let alt = resp.ctx.input(|i| i.modifiers.alt);
        let pressed = (resp.drag_started_by(egui::PointerButton::Primary) && !alt) || resp.drag_started_by(egui::PointerButton::Secondary);
        if pressed && self.drag.is_none() {
            let right = resp.drag_started_by(egui::PointerButton::Secondary);
            let from = resp.interact_pointer_pos().unwrap_or_default();
            let near = |p: Option<Pos2>| p.is_some_and(|p| p.distance(from) < 12.0);
            let what = match self.tool {
                Tool::Pivot if near(pivot_at) => Some(Grab::Pivot),
                Tool::Point => point_at.iter().position(|p| near(*p)).map(Grab::Point),
                Tool::Box | Tool::Line => Some(Grab::Cells),
                Tool::Select if !right => {
                    let on_face = self.sel.zip(self.hover).is_some_and(|((a, b), h)| h.solid && h.cell.cmpge(a.min(b)).all() && h.cell.cmple(a.max(b)).all());
                    Some(if on_face { Grab::Face } else { Grab::Cells })
                }
                _ => None,
            };
            // right drags turn the view, except for the tools that remove with them
            let takes_right = matches!(self.tool, Tool::Box | Tool::Line);
            if let (Some(what), Some(h)) = (what, self.hover.or(Some(Hit { part, cell: IVec3::ZERO, n: IVec3::Y, solid: false, t: 0.0 }))) {
                if !right || takes_right {
                    // (a plane's hit stays on its plane: the drag's cells are on that row)
                    let hit = if right || self.on_plane { Hit { solid: false, ..h } } else { h };
                    if what != Grab::Cells || self.hover.is_some() {
                        self.drag = Some(Drag { hit, from, right, what, plane: self.on_plane });
                    }
                }
            }
        }
        app.view.left_turns = false;
        if let (Some(dr), Some((o, d))) = (self.drag, ray) {
            let now = resp.interact_pointer_pos().unwrap_or(dr.from);
            match dr.what {
                Grab::Pivot | Grab::Point(_) => {
                    // in the plane through it facing the eye, to half voxels
                    let at = match dr.what {
                        Grab::Point(k) => {
                            let i = m.parts.iter().position(|p| p.name == m.points[k].part).unwrap_or(0);
                            (poses[i], Vec3::from(m.points[k].at))
                        }
                        _ => (pose, Vec3::from(m.parts[part].pivot)),
                    };
                    let world = at.0.transform_point3(at.1);
                    let f = (self.orbit.target - self.orbit.eye()).normalize();
                    let t = (world - o).dot(f) / d.dot(f);
                    let local = at.0.inverse().transform_point3(o + d * t);
                    let snap = ((local * 2.0).round() / 2.0).to_array();
                    match dr.what {
                        Grab::Point(k) => self.edit_merged(app, "Move a point", |m, _| {
                            m.points[k].at = snap;
                            true
                        }),
                        _ => self.edit_merged(app, "Move the pivot", |m, _| {
                            m.parts[part].pivot = snap;
                            true
                        }),
                    }
                }
                _ => {}
            }
            if resp.drag_stopped() {
                let end = if dr.plane { voxel::pick_layer(&m.parts[part], pose, dr.hit.cell.y, o, d).map(|h| h.cell) } else { voxel::on_plane(pose, &dr.hit, o, d) };
                let col = if dr.right { None } else { Some(self.first) };
                match (dr.what, self.tool, end) {
                    (Grab::Cells, Tool::Box | Tool::Line, Some(end)) => {
                        let a = dr.hit.target();
                        let cells = if self.tool == Tool::Box { voxel::rect(a, end) } else { voxel::line(a, end) };
                        self.cells(app, if self.tool == Tool::Box { "Box" } else { "Line" }, &cells, col);
                    }
                    (Grab::Cells, Tool::Select, Some(end)) => {
                        let a = if dr.hit.solid { dr.hit.cell } else { dr.hit.target() };
                        let mut end = end;
                        let ax = (0..3).find(|&k| dr.hit.n[k] != 0).unwrap_or(1);
                        end[ax] = a[ax];
                        self.sel = Some((a, end));
                    }
                    (Grab::Face, _, _) => {
                        // how many voxels the drag went along the face's normal, on screen
                        let c = pose.transform_point3(dr.hit.cell.as_vec3() + 0.5);
                        let step = app.view.to_screen(c).zip(app.view.to_screen(pose.transform_point3(dr.hit.cell.as_vec3() + 0.5 + dr.hit.n.as_vec3())));
                        if let (Some((p0, p1)), Some((a, b))) = (step, self.sel) {
                            let dir = p1 - p0;
                            let k = ((now - dr.from).dot(dir) / dir.length_sq().max(1.0)).round() as i32;
                            if k > 0 {
                                let n = dr.hit.n;
                                let mut out = (a, b);
                                self.edit(app, "Extrude", |m, _| {
                                    out = voxel::extrude(m, part, a, b, n, k);
                                    true
                                });
                                self.sel = Some(out);
                            }
                        }
                    }
                    _ => {}
                }
                self.drag = None;
                self.merging = None;
            }
            return;
        }
        let (left, right) = (resp.clicked() && !alt, resp.secondary_clicked());
        if !(left || right) {
            return;
        }
        let Some(h) = self.hover else { return };
        match self.tool {
            Tool::Add | Tool::Box | Tool::Line if left => {
                if let Some(c) = self.aims(h).0 {
                    self.cells(app, "Add", &[c], Some(self.first));
                }
            }
            Tool::Add | Tool::Box | Tool::Line if right && h.solid => self.cells(app, "Remove", &[h.cell], None),
            Tool::Paint if h.solid => {
                let c = if left { self.first } else { self.second };
                if let Some(k) = self.colour(app, c) {
                    let cells = voxel::mirrored(m_size(self, part), &[h.cell], self.mirror.0, self.mirror.1);
                    self.edit(app, "Paint", |m, _| voxel::apply(m, part, &cells, Op::Paint(k)));
                }
            }
            Tool::Pick if h.solid => {
                if let Some(m) = self.m() {
                    let v = voxel::get(&m.parts[part], h.cell) as usize;
                    if let Some(c) = m.rgb.get(v.wrapping_sub(1)).copied() {
                        *(if left { &mut self.first } else { &mut self.second }) = c;
                    }
                }
            }
            Tool::Fill if h.solid => {
                let ax = (0..3).find(|&k| h.n[k] != 0).unwrap_or(1);
                let cells = voxel::fill_plane(&self.m().unwrap().parts[part], h.cell, ax);
                if left {
                    if let Some(k) = self.colour(app, self.first) {
                        let cells = voxel::mirrored(m_size(self, part), &cells, self.mirror.0, self.mirror.1);
                        self.edit(app, "Fill", |m, _| voxel::apply(m, part, &cells, Op::Paint(k)));
                    }
                } else {
                    self.cells(app, "Remove a fill", &cells, None);
                }
            }
            Tool::Point if left => {
                let at = (h.target().as_vec3() + 0.5 - h.n.as_vec3() * 0.5).to_array();
                self.edit(app, "Add a point", |m, _| {
                    voxel::add_point(m, part, at);
                    true
                });
            }
            _ => {}
        }
    }

    /// Adds (a colour) or removes (none) cells of the chosen part, mirrored as set.
    fn cells(&mut self, app: &mut App, name: &str, cells: &[IVec3], colour: Option<[u8; 3]>) {
        let part = self.part;
        let cells = voxel::mirrored(m_size(self, part), cells, self.mirror.0, self.mirror.1);
        let op = match colour {
            Some(c) => match self.colour(app, c) {
                Some(k) => Op::Add(k),
                None => return,
            },
            None => Op::Remove,
        };
        let mut grew = false;
        self.edit(app, name, |m, _| {
            let ok = voxel::apply(m, part, &cells, op);
            grew = ok && cells.iter().any(|c| !voxel::inside(&m.parts[part], *c));
            ok
        });
        if matches!(op, Op::Add(_)) && !grew && cells.iter().any(|c| c.cmplt(IVec3::ZERO).any() || c.cmpge(IVec3::splat(32)).any()) {
            app.toast("A part is at most 32 voxels a side");
        }
    }

    /// The view's guides: the grid, the part's box, the pivot and points, the selection, the player.
    fn overlay(&self, p: &egui::Painter, view: &Viewport) {
        let Some(m) = self.m() else { return };
        let poses = self.poses();
        let line = |a: Vec3, b: Vec3, s: Stroke| {
            if let (Some(a), Some(b)) = (view.to_screen(a), view.to_screen(b)) {
                p.line_segment([a, b], s);
            }
        };
        let cube = |pose: Mat4, lo: Vec3, hi: Vec3, s: Stroke| {
            let c = |x: f32, y: f32, z: f32| pose.transform_point3(Vec3::new(x, y, z));
            for (a, b) in [((0, 0, 0), (1, 0, 0)), ((0, 0, 0), (0, 1, 0)), ((0, 0, 0), (0, 0, 1)), ((1, 1, 1), (0, 1, 1)), ((1, 1, 1), (1, 0, 1)), ((1, 1, 1), (1, 1, 0)), ((1, 0, 0), (1, 1, 0)), ((1, 0, 0), (1, 0, 1)), ((0, 1, 0), (1, 1, 0)), ((0, 1, 0), (0, 1, 1)), ((0, 0, 1), (1, 0, 1)), ((0, 0, 1), (0, 1, 1))] {
                let pick = |k: (i32, i32, i32)| Vec3::new(if k.0 == 0 { lo.x } else { hi.x }, if k.1 == 0 { lo.y } else { hi.y }, if k.2 == 0 { lo.z } else { hi.z });
                let (u, v) = (pick(a), pick(b));
                line(c(u.x, u.y, u.z), c(v.x, v.y, v.z), s);
            }
        };
        if self.show_grid {
            // every 8 voxels, one block: the cell's floor and its neighbours'
            let s = Stroke::new(1.0, Color32::from_white_alpha(40));
            for k in -1..=2 {
                line(Vec3::new(k as f32, 0.0, -1.0), Vec3::new(k as f32, 0.0, 2.0), s);
                line(Vec3::new(-1.0, 0.0, k as f32), Vec3::new(2.0, 0.0, k as f32), s);
            }
        }
        // the floor's edges by name: front where the model faces in the hand and in the world (+z)
        for (at, name) in [(Vec3::new(0.5, 0.0, 1.12), "front"), (Vec3::new(0.5, 0.0, -0.12), "back"), (Vec3::new(1.12, 0.0, 0.5), "right"), (Vec3::new(-0.12, 0.0, 0.5), "left")] {
            if let Some(q) = view.to_screen(at) {
                p.text(q, egui::Align2::CENTER_CENTER, name, egui::FontId::proportional(look::SMALL), Color32::from_white_alpha(if name == "front" { 200 } else { 110 }));
            }
        }
        if self.show_player {
            // a player's outline (0.6 x 1.8 m) beside the cell, for scale
            view.player(p, Vec3::new(1.6, 0.0, 0.2));
        }
        let Some(part) = m.parts.get(self.part) else { return };
        let pose = poses[self.part];
        let size = Vec3::from(part.size.map(|v| v as f32));
        let eye = view.cam().eye();
        let poly = |q: [Vec3; 4], fill: Color32, s: Stroke| {
            let pts: Option<Vec<Pos2>> = q.iter().map(|c| view.to_screen(*c)).collect();
            if let Some(pts) = pts {
                p.add(egui::Shape::convex_polygon(pts, fill, s));
            }
        };
        // the part's floor (in layer mode the layer's plane) in voxel lines, the 1 m lines heavier
        if self.show_grid || self.layer.is_some() {
            let y = self.layer.unwrap_or(0) as f32;
            let s = IVec3::from(part.size.map(|v| v as i32));
            let (lo, hi) = (IVec3::new(-GRID, 0, -GRID), IVec3::new(s.x + GRID, 0, s.z + GRID));
            let tint = if self.layer.is_some() { look::ACCENT } else { Color32::WHITE };
            for ax in [0, 2] {
                let (a0, a1, b0, b1) = if ax == 0 { (lo.x, hi.x, lo.z, hi.z) } else { (lo.z, hi.z, lo.x, hi.x) };
                for k in a0..=a1 {
                    let at = |b: i32| pose.transform_point3(if ax == 0 { Vec3::new(k as f32, y, b as f32) } else { Vec3::new(b as f32, y, k as f32) });
                    let heavy = ((at(b0)[ax] / VOXEL).round() as i32).rem_euclid(8) == 0;
                    let s = if heavy { Stroke::new(1.5, tint.gamma_multiply(0.5)) } else { Stroke::new(1.0, tint.gamma_multiply(0.15)) };
                    line(at(b0), at(b1), s);
                }
            }
        }
        if self.faces_now {
            // the rows above the layer, dimmed: hidden in the view, drawn here faint, the far faces first
            for (q, rgb) in &self.faces.2 {
                poly(*q, Color32::from_rgba_unmultiplied(rgb[0], rgb[1], rgb[2], 60), Stroke::new(1.0, Color32::from_white_alpha(18)));
            }
            // the edges: thin dark lines round every face the eye sees
            for q in &self.faces.1 {
                for i in 0..4 {
                    line(q[i], q[(i + 1) % 4], Stroke::new(1.0, Color32::from_black_alpha(120)));
                }
            }
        }
        cube(pose, Vec3::ZERO, size, Stroke::new(1.0, look::ACCENT.gamma_multiply(0.6)));
        // the mirror's planes, through the part's middle
        for (on, ax) in [(self.mirror.0, 0), (self.mirror.1, 2)] {
            if on {
                let mut q = [Vec3::ZERO, Vec3::new(0.0, size.y, 0.0), Vec3::new(0.0, size.y, 0.0), Vec3::ZERO];
                let other = 2 - ax;
                (q[2][other], q[3][other]) = (size[other], size[other]);
                for c in &mut q {
                    c[ax] = size[ax] / 2.0;
                }
                poly(q.map(|c| pose.transform_point3(c)), look::ACCENT.gamma_multiply(0.12), Stroke::new(1.5, look::ACCENT.gamma_multiply(0.7)));
            }
        }
        if let Some((a, b)) = self.sel {
            let (lo, hi) = (a.min(b).as_vec3(), a.max(b).as_vec3() + 1.0);
            cube(pose, lo, hi, Stroke::new(2.0, look::DRAFT));
        }
        if let Some(h) = self.hover.filter(|_| self.drag.is_none()) {
            let (add, remove) = self.aims(h);
            let twins = |c: IVec3| voxel::mirrored(part.size, &[c], self.mirror.0, self.mirror.1);
            // the face under the pointer, lit: the voxel's, or the plane's cell
            let lit = if self.on_plane { voxel::face_quad(h.cell, IVec3::NEG_Y) } else { voxel::face_quad(h.cell, if h.solid { h.n } else { -h.n }) };
            poly(lit.map(|v| pose.transform_point3(v)), Color32::from_white_alpha(60), Stroke::NONE);
            match self.tool {
                Tool::Add | Tool::Box | Tool::Line => {
                    // a ghost voxel in the chosen colour where a left click adds, half see-through
                    for c in add.map_or(vec![], twins) {
                        let [r, g, b] = self.first;
                        for n in voxel::NORMALS {
                            let q = voxel::face_quad(c, n).map(|v| pose.transform_point3(v));
                            if pose.transform_vector3(n.as_vec3()).dot(eye - (q[0] + q[2]) / 2.0) > 0.0 {
                                poly(q, Color32::from_rgba_unmultiplied(r, g, b, 150), Stroke::NONE);
                            }
                        }
                        // its outline light, so a colour like the ground's still shows
                        cube(pose, c.as_vec3(), c.as_vec3() + 1.0, Stroke::new(1.5, Color32::from_white_alpha(210)));
                    }
                    // the voxel a right click removes, outlined red
                    for c in remove.map_or(vec![], twins) {
                        cube(pose, c.as_vec3(), c.as_vec3() + 1.0, Stroke::new(2.0, Color32::from_rgb(235, 60, 50)));
                    }
                }
                _ if h.solid => cube(pose, h.cell.as_vec3(), h.cell.as_vec3() + 1.0, Stroke::new(1.5, c32(self.first))),
                _ => {}
            }
        }
        let cross = |at: Option<Pos2>, col: Color32, big: f32| {
            if let Some(c) = at {
                p.line_segment([c - Vec2::X * big, c + Vec2::X * big], Stroke::new(2.0, col));
                p.line_segment([c - Vec2::Y * big, c + Vec2::Y * big], Stroke::new(2.0, col));
                p.circle_stroke(c, big * 0.45, Stroke::new(1.5, col));
            }
        };
        cross(view.to_screen(pose.transform_point3(Vec3::from(part.pivot))), Color32::from_rgb(255, 210, 80), 9.0);
        for pt in &m.points {
            if let Some(i) = m.parts.iter().position(|p| p.name == pt.part) {
                let at = view.to_screen(poses[i].transform_point3(Vec3::from(pt.at)));
                cross(at, Color32::from_rgb(240, 120, 220), 6.0);
                if let Some(a) = at {
                    p.text(a + Vec2::new(8.0, -8.0), egui::Align2::LEFT_BOTTOM, &pt.name, egui::FontId::proportional(look::SMALL), Color32::from_rgb(240, 160, 230));
                }
            }
        }
    }

    /// The 2D preview until the game's renderer is up: the model's faces, flat shaded, far ones first.
    fn sketch(&self, p: &egui::Painter, view: &Viewport) {
        let Some(m) = self.m() else { return };
        let poses = self.poses();
        let eye = view.orbit.eye();
        let mut faces = vec![];
        for (i, part) in m.parts.iter().enumerate() {
            for q in engine::models::mesh(part) {
                let a = (q.dir / 2) as usize;
                let (u, v) = ((a + 1) % 3, (a + 2) % 3);
                let mut base = Vec3::new(q.at[0] as f32, q.at[1] as f32, q.at[2] as f32);
                if q.dir % 2 == 0 {
                    base[a] += 1.0;
                }
                let (mut du, mut dv) = (Vec3::ZERO, Vec3::ZERO);
                du[u] = q.w as f32;
                dv[v] = q.h as f32;
                let corners = [base, base + du, base + du + dv, base + dv].map(|c| poses[i].transform_point3(c));
                let mid = (corners[0] + corners[2]) / 2.0;
                let shade = [0.8, 0.7, 1.0, 0.5, 0.9, 0.75][q.dir as usize];
                let rgb = m.rgb[q.colour as usize].map(|c| (c as f32 * shade) as u8);
                faces.push((mid.distance(eye), corners, rgb));
            }
        }
        faces.sort_by(|a, b| b.0.total_cmp(&a.0));
        for (_, c, rgb) in faces {
            let pts: Option<Vec<Pos2>> = c.iter().map(|c| view.to_screen(*c)).collect();
            if let Some(pts) = pts {
                p.add(egui::Shape::convex_polygon(pts, c32(rgb), Stroke::new(0.5, Color32::from_black_alpha(60))));
            }
        }
    }

    /// Under the view: the clip, its length and loop, play, and a row per part with its keys; a click scrubs.
    fn timeline(&mut self, ui: &mut Ui, app: &mut App) {
        let Some(m) = self.m().cloned() else { return };
        ui.horizontal_wrapped(|ui| {
            let cur = self.clip_name().unwrap_or_else(|| "no clip".into());
            let r = egui::ComboBox::from_id_salt("clip").selected_text(cur).width(120.0).show_ui(ui, |ui| {
                for (i, c) in m.clips.iter().enumerate() {
                    if ui.selectable_label(self.clip == Some(i), &c.name).clicked() {
                        (self.clip, self.t, self.key, self.event) = (Some(i), 0.0, None, None);
                    }
                }
            });
            look::tip(r.response, "The clip the timeline shows and plays", "");
            if look::tip(ui.button("New clip"), "A new clip of one second, looping", "").clicked() {
                self.run(app, "clip.new");
            }
            let Some(ci) = self.clip.filter(|c| *c < m.clips.len()) else { return };
            let mut buf = std::mem::take(&mut self.names[2]);
            if let Some(new) = name_box(ui, &mut buf, "clip name", &m.clips[ci].name, "The clip's name; Enter renames it in every state and event list that names it", 70.0) {
                let mut err = None;
                self.edit_life(app, "Rename a clip", |m, life, _| voxel::rename_clip(m, life, ci, &new).map_err(|e| err = Some(e)).is_ok());
                if let Some(e) = err {
                    app.refuse(e);
                }
            }
            self.names[2] = buf;
            let mut len = m.clips[ci].length;
            let r = ui.add(egui::DragValue::new(&mut len).speed(0.02).range(0.1..=60.0).suffix(" s"));
            if look::tip(r, "The clip's length; keys past it move in", "").changed() {
                self.edit_merged(app, "Clip length", |m, _| {
                    let c = &mut m.clips[ci];
                    c.length = len;
                    for k in &mut c.key {
                        k.t = k.t.min(len);
                    }
                    true
                });
            }
            let mut lp = m.clips[ci].looping;
            if look::tip(ui.checkbox(&mut lp, "Loop"), "Play it round and round", "").changed() {
                self.edit(app, "Loop", |m, _| {
                    m.clips[ci].looping = lp;
                    true
                });
            }
            let r = ui.button(if self.playing { "Pause" } else { "Play" });
            if look::tip(r, "Play the clip in the view", "Space").clicked() {
                self.playing = !self.playing;
            }
            look::weak(ui, format!("{:.2} s", self.t));
            let first = self.sounds.first().map(|s| (Some(s.clone()), None)).or(self.effects.first().map(|e| (None, Some(e.clone()))));
            if look::tip(ui.add_enabled(first.is_some(), egui::Button::new("Add event")), "A sound or an effect at the play head (pick which on the event)", "").clicked() {
                let (sound, effect) = first.unwrap();
                let (clip, t) = (m.clips[ci].name.clone(), self.t);
                let mut at = 0;
                self.edit_life(app, "Add an event", |_, life, _| {
                    let ev = voxel::events_mut(life, &clip);
                    ev.push(Event { t, sound, effect, point: None });
                    at = ev.len() - 1;
                    true
                });
                (self.event, self.key) = (Some(at), None);
            }
            let clip_name = m.clips[ci].name.clone();
            self.event_bar(ui, app, &clip_name);
            if let Some(k) = self.key.filter(|k| *k < m.clips[ci].key.len()) {
                ui.separator();
                let key = &m.clips[ci].key[k];
                look::weak(ui, format!("key {} at {:.2}", key.part, key.t));
                let mut ease = key.ease;
                let r = egui::ComboBox::from_id_salt("ease").selected_text(format!("{ease:?}")).width(80.0).show_ui(ui, |ui| {
                    for e in [Ease::Linear, Ease::Smooth, Ease::Step] {
                        ui.selectable_value(&mut ease, e, format!("{e:?}"));
                    }
                });
                look::tip(r.response, "How the motion leaves this key: straight, eased, or held until the next", "");
                if ease != key.ease {
                    self.edit(app, "Ease", |m, _| {
                        m.clips[ci].key[k].ease = ease;
                        true
                    });
                }
                if look::tip(ui.button("Delete key"), "Remove this key", "").clicked() {
                    self.edit(app, "Delete a key", |m, _| {
                        m.clips[ci].key.remove(k);
                        true
                    });
                    self.key = None;
                }
            }
        });
        let Some(ci) = self.clip.filter(|c| *c < m.clips.len()) else {
            look::weak(ui, "No clip: New clip starts one, or a ready-made motion in the inspector fills one");
            return;
        };
        let clip = &m.clips[ci];
        let row = 18.0;
        let name_w = 90.0;
        let h = row * (m.parts.len() as f32 + 2.0) + 4.0;
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h.min(ui.available_height().max(row * 2.0))), Sense::click_and_drag());
        crate::drive::mark(ui.ctx(), "timeline", rect);
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 4.0, ui.visuals().extreme_bg_color);
        let track = Rect::from_min_max(Pos2::new(rect.min.x + name_w, rect.min.y), Pos2::new(rect.max.x - GAP, rect.max.y));
        let x_of = |t: f32| track.min.x + t / clip.length * track.width();
        // the ruler: a tick every tenth of the clip
        for k in 0..=10 {
            let x = x_of(clip.length * k as f32 / 10.0);
            p.line_segment([Pos2::new(x, rect.min.y), Pos2::new(x, rect.min.y + 5.0)], Stroke::new(1.0, Color32::from_gray(90)));
        }
        let mut chosen = None;
        for (i, part) in m.parts.iter().enumerate() {
            let y = rect.min.y + row * (i as f32 + 1.0);
            let r = Rect::from_min_max(Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y + row));
            if i == self.part {
                p.rect_filled(r, 0.0, look::ACCENT.gamma_multiply(0.18));
            }
            p.text(Pos2::new(rect.min.x + 6.0, y + row / 2.0), egui::Align2::LEFT_CENTER, &part.name, egui::FontId::proportional(look::SMALL), Color32::from_gray(200));
            p.line_segment([Pos2::new(track.min.x, y + row / 2.0), Pos2::new(track.max.x, y + row / 2.0)], Stroke::new(1.0, Color32::from_gray(60)));
            for (k, key) in clip.key.iter().enumerate().filter(|(_, k)| k.part == part.name) {
                let c = Pos2::new(x_of(key.t), y + row / 2.0);
                let on = self.key == Some(k);
                let pts = vec![c + Vec2::new(0.0, -5.0), c + Vec2::new(5.0, 0.0), c + Vec2::new(0.0, 5.0), c + Vec2::new(-5.0, 0.0)];
                p.add(egui::Shape::convex_polygon(pts, if on { look::ACCENT } else { Color32::from_rgb(230, 200, 120) }, Stroke::new(1.0, Color32::BLACK)));
                if resp.clicked() && resp.interact_pointer_pos().is_some_and(|q| q.distance(c) < 7.0) {
                    chosen = Some((k, i, key.t));
                }
            }
        }
        // the events row, under the parts: a marker per event at its time
        let evs = self.doc().and_then(|d| d.life.events.iter().find(|(c, _)| *c == clip.name).map(|(_, e)| e.clone())).unwrap_or_default();
        let ey = rect.min.y + row * (m.parts.len() as f32 + 1.0) + row / 2.0;
        p.text(Pos2::new(rect.min.x + 6.0, ey), egui::Align2::LEFT_CENTER, "events", egui::FontId::proportional(look::SMALL), Color32::from_gray(160));
        p.line_segment([Pos2::new(track.min.x, ey), Pos2::new(track.max.x, ey)], Stroke::new(1.0, Color32::from_gray(60)));
        let near_event = |q: Pos2| evs.iter().position(|e| q.distance(Pos2::new(x_of(e.t), ey)) < 8.0);
        for (k, e) in evs.iter().enumerate() {
            let c = Pos2::new(x_of(e.t), ey);
            crate::drive::mark(ui.ctx(), format!("event:{k}"), Rect::from_center_size(c, Vec2::splat(2.0)));
            let col = if self.event == Some(k) { look::ACCENT } else if e.effect.is_some() { Color32::from_rgb(240, 140, 90) } else { Color32::from_rgb(120, 190, 240) };
            p.add(egui::Shape::convex_polygon(vec![c + Vec2::new(0.0, -6.0), c + Vec2::new(5.0, 5.0), c + Vec2::new(-5.0, 5.0)], col, Stroke::new(1.0, Color32::BLACK)));
        }
        if resp.drag_started() {
            // where the press was: a drag starts only once the pointer has left the marker
            if let Some(k) = ui.input(|i| i.pointer.press_origin()).and_then(near_event) {
                (self.event, self.key, self.moving_event) = (Some(k), None, true);
            }
        }
        if self.moving_event {
            if let (Some(q), Some(k)) = (resp.interact_pointer_pos(), self.event) {
                let t = ((q.x - track.min.x) / track.width() * clip.length).clamp(0.0, clip.length);
                let name = clip.name.clone();
                self.edit_life_merged(app, "Move an event", |_, life, _| {
                    voxel::events_mut(life, &name)[k].t = (t * 100.0).round() / 100.0;
                    true
                });
            }
            if resp.drag_stopped() || !resp.dragged() {
                (self.moving_event, self.merging) = (false, None);
            }
            return;
        }
        if resp.clicked() {
            if let Some(k) = resp.interact_pointer_pos().and_then(near_event) {
                (self.event, self.key, self.t) = (Some(k), None, evs[k].t);
                return;
            }
        }
        let x = x_of(self.t);
        p.line_segment([Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)], Stroke::new(2.0, look::BAD));
        if let Some((k, i, t)) = chosen {
            (self.key, self.part, self.t) = (Some(k), i, t);
            self.part_name = m.parts[i].name.clone();
        } else if let Some(q) = resp.interact_pointer_pos().filter(|_| resp.clicked() || resp.dragged()) {
            if q.x >= track.min.x {
                self.timeline_t((q.x - track.min.x) / track.width() * clip.length);
                self.playing = false;
            } else if resp.clicked() {
                let i = ((q.y - rect.min.y) / row - 1.0).floor();
                if i >= 0.0 && (i as usize) < m.parts.len() {
                    self.event = None;
                    self.part = i as usize;
                    self.part_name = m.parts[self.part].name.clone();
                }
            }
        }
        look::tip(resp, "Click or drag to scrub; click a key to choose it (its ease above); a part's name chooses it; the events row: click an event to edit it, drag it to move it", "");
    }

    fn part_inspector(&mut self, ui: &mut Ui, app: &mut App) {
        let Some(m) = self.m().cloned() else { return };
        let Some(part) = m.parts.get(self.part).cloned() else { return };
        let i = self.part;
        look::row(ui, "name", |ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut self.part_name).desired_width(ui.available_width()));
            if look::tip(r.clone(), "The part's name; Enter renames it everywhere it is named", "").lost_focus() && self.part_name != part.name {
                let new = self.part_name.clone();
                let mut err = None;
                self.edit(app, "Rename a part", |m, _| match voxel::rename_part(m, i, &new) {
                    Ok(()) => true,
                    Err(e) => {
                        err = Some(e);
                        false
                    }
                });
                if let Some(e) = err {
                    app.refuse(e);
                    self.part_name = part.name.clone();
                }
            }
        });
        look::row(ui, "parent", |ui| {
            let mut to = m.up.get(i).copied().flatten();
            let r = egui::ComboBox::from_id_salt("parent").selected_text(if part.parent.is_empty() { "none (a root)" } else { &part.parent }).width(ui.available_width() - 8.0).show_ui(ui, |ui| {
                ui.selectable_value(&mut to, None, "none (a root)");
                for (k, q) in m.parts.iter().enumerate().filter(|(k, _)| *k != i) {
                    ui.selectable_value(&mut to, Some(k), &q.name);
                }
            });
            look::tip(r.response, "The part this one hangs on: it moves and turns with it", "");
            if to != m.up.get(i).copied().flatten() {
                let mut err = None;
                self.edit(app, "Re-parent", |m, _| match voxel::reparent(m, i, to) {
                    Ok(()) => true,
                    Err(e) => {
                        err = Some(e);
                        false
                    }
                });
                if let Some(e) = err {
                    app.refuse(e);
                }
            }
        });
        let mut at = part.at;
        let mut pivot = part.pivot;
        egui::Grid::new("part numbers").num_columns(4).spacing(Vec2::new(GAP, 4.0)).show(ui, |ui| {
            look::label(ui, "at");
            let mut ch = false;
            for v in &mut at {
                ch |= ui.add(egui::DragValue::new(v).speed(0.1)).changed();
            }
            if ch {
                self.edit_merged(app, "Move a part", |m, _| {
                    m.parts[i].at = at;
                    true
                });
            }
            ui.end_row();
            look::label(ui, "pivot");
            let mut ch = false;
            for v in &mut pivot {
                ch |= ui.add(egui::DragValue::new(v).speed(0.05).fixed_decimals(1)).changed();
            }
            if ch {
                self.edit_merged(app, "Move the pivot", |m, _| {
                    m.parts[i].pivot = pivot;
                    true
                });
            }
            ui.end_row();
        });
        look::weak(ui, format!("box {} x {} x {} voxels (grows as you add)", part.size[0], part.size[1], part.size[2]));
        // the part's move and turn at the timeline's time: a change sets a key there
        if let Some(ci) = self.clip.filter(|c| *c < m.clips.len()) {
            let (mut mv, mut tn) = sample(&m.clips[ci], &part.name, self.t);
            ui.add_space(GAP * 0.5);
            look::weak(ui, format!("At {:.2} s in {} (a change sets a key):", self.t, m.clips[ci].name));
            egui::Grid::new("part key").num_columns(4).spacing(Vec2::new(4.0, 4.0)).show(ui, |ui| {
                let mut ch = false;
                ui.label(RichText::new("Move").small());
                for k in 0..3 {
                    ch |= ui.add(egui::DragValue::new(&mut mv[k]).speed(0.05).fixed_decimals(2)).changed();
                }
                ui.end_row();
                ui.label(RichText::new("Turn").small());
                for k in 0..3 {
                    ch |= ui.add(egui::DragValue::new(&mut tn[k]).speed(0.5).fixed_decimals(1).suffix("°")).changed();
                }
                ui.end_row();
                if ch {
                    let (t, name) = (self.t, part.name.clone());
                    self.edit_merged(app, "Set a key", |m, _| {
                        voxel::set_key(&mut m.clips[ci], &name, t, mv.to_array(), tn.to_array());
                        true
                    });
                }
            });
        }
        ui.horizontal_wrapped(|ui| {
            if look::tip(ui.button("Add a part"), "A new empty part, a root", "").clicked() {
                self.run(app, "part.add");
            }
            if look::tip(ui.button("Add a child"), "A new empty part hanging on this one", "").clicked() {
                self.run(app, "part.child");
            }
        });
    }

    fn parts_tree(&mut self, ui: &mut Ui, _app: &mut App) {
        ui.label(RichText::new("Parts").strong());
        let Some(m) = self.m().cloned() else {
            look::weak(ui, "no model open");
            return;
        };
        egui::ScrollArea::vertical().id_salt("parts tree").auto_shrink([false, true]).show(ui, |ui| {
            // roots first, each followed by its children, indented
            fn walk(m: &Model, at: Option<usize>, depth: usize, out: &mut Vec<(usize, usize)>) {
                for (i, _) in m.parts.iter().enumerate().filter(|(i, _)| m.up.get(*i).copied().flatten() == at) {
                    out.push((i, depth));
                    walk(m, Some(i), depth + 1, out);
                }
            }
            let mut rows = vec![];
            walk(&m, None, 0, &mut rows);
            for (i, depth) in rows {
                ui.horizontal(|ui| {
                    ui.add_space(depth as f32 * GAP * 1.5);
                    let r = ui.selectable_label(self.part == i, &m.parts[i].name);
                    crate::drive::mark(ui.ctx(), format!("part:{}", m.parts[i].name), r.rect);
                    if look::tip(r, "Choose this part: the tools edit only it", "").clicked() {
                        self.part = i;
                        self.part_name = m.parts[i].name.clone();
                        self.sel = None;
                    }
                });
            }
            if !m.points.is_empty() {
                ui.add_space(GAP * 0.5);
                look::weak(ui, "Points");
                for (k, pt) in m.points.iter().enumerate() {
                    let r = ui.selectable_label(self.point == Some(k), format!("  + {} on {}", pt.name, pt.part));
                    if look::tip(r, "Choose this point: rename or remove it in the inspector", "").clicked() {
                        self.point = Some(k);
                    }
                }
            }
        });
    }
}

/// Within the floor grid the view draws under a part: its box and 4 cells round it.
fn on_grid(p: &engine::models::Part, c: IVec3) -> bool {
    let s = IVec3::from(p.size.map(|v| v as i32));
    c.x >= -GRID && c.z >= -GRID && c.x < s.x + GRID && c.z < s.z + GRID
}
const GRID: i32 = 4;

fn m_size(s: &Models, part: usize) -> [u32; 3] {
    s.m().and_then(|m| m.parts.get(part)).map_or([1; 3], |p| p.size)
}

impl Page for Models {
    fn name(&self) -> &'static str {
        "Models"
    }

    fn files(&self, app: &App) -> Vec<FileRow> {
        self.docs
            .iter()
            .map(|(n, d)| {
                let path = format!("{MODELS}/{n}.toml");
                FileRow { id: n.clone(), label: n.clone(), thumb: Thumb::Img(voxel::thumb(&d.m)), flags: Flags { made: d.saved.is_some(), dirty: d.dirty(), draft: app.is_draft(&path), problem: app.has_problem(&path) } }
            })
            .collect()
    }

    fn current(&self) -> Option<String> {
        self.open.clone()
    }

    fn open(&mut self, _app: &mut App, id: &str) {
        if self.docs.contains_key(id) {
            self.open = Some(id.to_string());
            self.part = 0;
            self.opened();
        }
    }

    fn commands(&self, _app: &App) -> Vec<Cmd> {
        let mut v: Vec<Cmd> = TOOLS.iter().map(|(_, id, name, k, help)| Cmd::new(format!("tool.{id}"), format!("Tool: {name}"), *help).key(Modifiers::NONE, *k)).collect();
        v.push(Cmd::new("mirror.x", "Mirror in x on or off", "every edit also on the far side of the part's middle in x").key(Modifiers::NONE, Key::X));
        v.push(Cmd::new("mirror.z", "Mirror in z on or off", "every edit also on the far side of the part's middle in z").key(Modifiers::NONE, Key::Z));
        v.push(Cmd::new("undo", "Undo", "undo the last change to this model").key(Modifiers::COMMAND, Key::Z));
        v.push(Cmd::new("redo", "Redo", "redo what was undone").key(Modifiers::COMMAND, Key::Y));
        v.push(Cmd::new("save", "Save the model", "write it (checked by the game's reader first) and take it off the drafts list").key(Modifiers::COMMAND, Key::S));
        v.push(Cmd::new("new", "New model", "choose a shape to start from: a block, a slab, a post, a tool, a plant cross or a creature").key(Modifiers::COMMAND, Key::N));
        for (id, label, help) in voxel::STARTERS {
            v.push(Cmd::new(format!("new:{id}"), format!("New model: {label}"), help));
        }
        for kind in voxel::PLANTS {
            v.push(Cmd::new(format!("plant:{kind}"), format!("New {kind}"), "a small plant grown as a new model, to finish by hand; again grows another"));
        }
        v.push(Cmd::new("play", "Play or pause", "the clip in the view").key(Modifiers::NONE, Key::Space));
        v.push(Cmd::new("shade", "Shade by light", "every voxel along its ramp by how open it is: open sides lighter, crevices and undersides darker"));
        v.push(Cmd::new("clip.new", "New clip", "a new clip of one second, looping"));
        v.push(Cmd::new("part.add", "Add a part", "a new empty part, a root"));
        v.push(Cmd::new("part.child", "Add a child part", "a new empty part hanging on the chosen one"));
        v.push(Cmd::new("motion", "Ready-made motion", "fill the clip's keys of the chosen part with the motion set in the inspector"));
        for (id, k, what) in [("left", Key::ArrowLeft, "-x"), ("right", Key::ArrowRight, "+x"), ("up", Key::ArrowUp, "-z"), ("down", Key::ArrowDown, "+z"), ("raise", Key::PageUp, "+y"), ("lower", Key::PageDown, "-y")] {
            // (Page Up/Down alone move the layer)
            let mods = if matches!(id, "raise" | "lower") { Modifiers::SHIFT } else { Modifiers::NONE };
            v.push(Cmd::new(format!("sel.{id}"), format!("Move the selection {what}"), "one voxel").key(mods, k));
        }
        v.push(Cmd::new("layer.on", "Layer mode on or off", "clicks land on one row's plane, the rows above are dimmed: build floor by floor, or work inside"));
        v.push(Cmd::new("layer.up", "Layer up", "layer mode's plane a row up (turns layer mode on)").key(Modifiers::NONE, Key::PageUp));
        v.push(Cmd::new("layer.down", "Layer down", "layer mode's plane a row down (turns layer mode on)").key(Modifiers::NONE, Key::PageDown));
        v.push(Cmd::new("edges", "Edges on or off", "thin dark lines between voxels, so a face of one colour still reads as voxels"));
        v.push(Cmd::new("sel.none", "Select nothing", "drop the selection").key(Modifiers::NONE, Key::Escape));
        // (the first key that matches runs, and a plain key matches with Shift held too: the Shift views first)
        for (k, (id, name, ..)) in VIEWS.iter().enumerate().rev() {
            let key = [Key::Num1, Key::Num3, Key::Num7][k / 2];
            let mods = if k % 2 == 0 { Modifiers::NONE } else { Modifiers::SHIFT };
            v.push(Cmd::new(format!("view:{id}"), format!("View from the {}", name.to_lowercase()), "look at the model from that side (the view cube's face)").key(mods, key));
        }
        v.push(Cmd::new("view.flat", "Flat view on or off", "no perspective, so rows and sizes line up").key(Modifiers::NONE, Key::Num5));
        v.push(Cmd::new("view.frame", "Frame", "fill the view with the selection, else the chosen part").key(Modifiers::NONE, Key::F));
        v
    }

    fn run(&mut self, app: &mut App, id: &str) {
        if let Some(t) = id.strip_prefix("tool.").and_then(|t| TOOLS.iter().find(|x| x.1 == t)) {
            self.tool = t.0;
            return;
        }
        // with an argument (the command box and shot=): part:<name>, clip:<name>, time:<s>, motion:<kind>,
        // fromtex:<block>, layer:<row or off>, sel:x;y;z;x;y;z, scene:<grass|cave|none>, hour:<h>, cam:yaw;pitch;dist
        if let Some((cmd, arg)) = id.split_once(':') {
            let m = self.m().cloned();
            match cmd {
                "part" => {
                    if let Some(i) = m.and_then(|m| m.parts.iter().position(|p| p.name == arg)) {
                        self.part = i;
                        self.part_name = arg.into();
                    }
                }
                "clip" => {
                    if let Some(i) = m.and_then(|m| m.clips.iter().position(|c| c.name == arg)) {
                        (self.clip, self.t) = (Some(i), 0.0);
                    }
                }
                "time" => self.timeline_t(arg.parse().unwrap_or(0.0)),
                // state:<name> chooses a state, playstate:<name> plays it in the view, event:<k> and point:<k> choose
                "state" | "playstate" => {
                    if let Some(i) = self.doc().and_then(|d| d.life.states.iter().position(|s| s.name == arg)) {
                        self.state = Some(i);
                        if cmd == "playstate" {
                            self.playing_state = Some(arg.into());
                        }
                    }
                }
                "event" => self.event = arg.parse().ok(),
                "point" => self.point = arg.parse().ok(),
                "layer" | "slice" => self.layer = arg.parse().ok(),
                "motion" => {
                    if let Some(k) = MOTIONS.iter().find(|x| format!("{:?}", x.0).eq_ignore_ascii_case(arg)) {
                        self.motion = (k.0, self.motion.1, k.3);
                        self.run(app, "motion");
                    }
                }
                "fromtex" => {
                    self.tex_block = arg.into();
                    let faces: Vec<Option<art::Img>> = (1..4).map(|f| app.docs.get(&art::file_name(arg, f)).or(app.docs.get(&art::file_name(arg, 0))).map(|d| d.img.clone())).collect();
                    let mut made = None;
                    self.edit(app, "From a texture", |m, pal| {
                        made = voxel::from_texture(m, pal, [faces[0].as_ref(), faces[1].as_ref(), faces[2].as_ref()]);
                        made.is_some()
                    });
                    match made {
                        Some(i) => {
                            self.part = i;
                            self.part_name = self.m().map_or(String::new(), |m| m.parts[i].name.clone());
                        }
                        None => app.refuse(format!("{arg} has no painted faces to start from")),
                    }
                }
                "sel" => {
                    let n: Vec<i32> = arg.split(';').filter_map(|x| x.parse().ok()).collect();
                    if let [a, b, c, d, e, f] = n[..] {
                        self.sel = Some((IVec3::new(a, b, c), IVec3::new(d, e, f)));
                    }
                }
                "plant" if voxel::PLANTS.contains(&arg) => self.new_model(arg, |k| Some(voxel::plant(&app.pal, arg, k))),
                "new" => self.new_model(arg, |_| voxel::starter(&app.pal, arg)),
                "view" => {
                    if let Some(k) = VIEWS.iter().position(|v| v.0 == arg) {
                        self.orbit.look_from(k);
                    }
                }
                "scene" => app.view.scene = super::viewport::Scene::named(arg),
                "hour" => app.view.hour = arg.parse().unwrap_or(app.view.hour),
                "cam" => {
                    let n: Vec<f32> = arg.split(';').filter_map(|x| x.parse().ok()).collect();
                    if let [y, p, d] = n[..] {
                        (self.orbit.yaw, self.orbit.pitch, self.orbit.dist) = (y.to_radians(), p.to_radians(), d);
                    }
                }
                "show" => match arg {
                    "block" => self.show_block = !self.show_block,
                    "player" => self.show_player = !self.show_player,
                    _ => self.show_grid = !self.show_grid,
                },
                _ => {}
            }
            return;
        }
        let part = self.part;
        match id {
            "mirror.x" => self.mirror.0 = !self.mirror.0,
            "mirror.z" => self.mirror.1 = !self.mirror.1,
            "play" => self.playing = !self.playing && self.clip.is_some(),
            "save" => self.save(app),
            "new" => self.choosing = true,
            "undo" | "redo" => {
                if let Some(d) = self.open.as_ref().and_then(|n| self.docs.get_mut(n)) {
                    let done = d.step(|h, now| if id == "undo" { h.undo(now) } else { h.redo(now) });
                    if done {
                        d.rev += 1;
                    }
                }
                self.opened_keep();
            }
            "shade" => {
                let k = self.strength;
                let mut n = 0;
                self.edit(app, "Shade by light", |m, pal| {
                    n = voxel::shade_by_light(m, pal, Some(part), k);
                    n > 0
                });
                app.toast(format!("Shade by light: {n} voxels stepped along their ramps"));
            }
            "clip.new" => {
                let mut at = None;
                self.edit(app, "New clip", |m, _| {
                    let name = (1..).map(|k| if k == 1 { "idle".to_string() } else { format!("clip {k}") }).find(|n| !m.clips.iter().any(|c| &c.name == n)).unwrap();
                    m.clips.push(Clip { name, length: 1.0, looping: true, key: vec![] });
                    at = Some(m.clips.len() - 1);
                    true
                });
                if at.is_some() {
                    (self.clip, self.t) = (at, 0.0);
                }
            }
            "part.add" | "part.child" => {
                let parent = (id == "part.child").then_some(part);
                let mut made = 0;
                self.edit(app, "Add a part", |m, _| {
                    made = voxel::add_part(m, parent);
                    true
                });
                self.part = made;
                self.part_name = self.m().map_or(String::new(), |m| m.parts[made].name.clone());
            }
            "motion" => {
                if self.clip.is_none() {
                    self.run(app, "clip.new");
                }
                let (Some(ci), Some(name)) = (self.clip, self.m().and_then(|m| m.parts.get(part)).map(|p| p.name.clone())) else { return };
                let (kind, axis, amount) = self.motion;
                self.edit(app, "Ready-made motion", |m, _| {
                    voxel::motion(&mut m.clips[ci], &name, kind, axis, amount);
                    true
                });
            }
            "sel.none" => (self.sel, self.choosing) = (None, false),
            "view.flat" => self.flat = !self.flat,
            "layer.on" => self.layer = if self.layer.is_some() { None } else { Some(0) },
            "layer.up" | "layer.down" => {
                let l = self.layer.map_or(0, |l| l + if id == "layer.up" { 1 } else { -1 });
                // (not below row 0: an add there would grow the box down and leave the plane a row under it)
                self.layer = Some(l.clamp(0, m_size(self, part)[1] as i32));
            }
            "edges" => self.edges = !self.edges,
            "view.frame" => self.frame(app),
            _ => {
                if let (Some(dir), Some((a, b))) = (id.strip_prefix("sel."), self.sel) {
                    let by = match dir {
                        "left" => IVec3::NEG_X,
                        "right" => IVec3::X,
                        "up" => IVec3::NEG_Z,
                        "down" => IVec3::Z,
                        "raise" => IVec3::Y,
                        _ => IVec3::NEG_Y,
                    };
                    self.edit(app, "Move the selection", |m, _| voxel::move_sel(m, part, a, b, by));
                    self.sel = Some((a + by, b + by));
                }
            }
        }
    }

    fn more_keys(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("Right drag", "turn the view about the selection, else the model's middle (Box and Line: removes)"),
            ("Alt + left drag", "turn the view, for a hand without a middle or right button"),
            ("Middle drag", "pan the view"),
            ("Wheel", "zoom toward the pointer"),
            ("Left click", "the tool's first action (add, paint, pick)"),
            ("Right click", "the tool's second action (remove, second colour)"),
        ]
    }

    fn work(&mut self, ui: &mut Ui, app: &mut App) {
        if !ui.input(|i| i.pointer.any_down()) && self.drag.is_none() {
            self.merging = None;
        }
        if self.playing {
            let dt = ui.input(|i| i.stable_dt).min(0.1);
            if let Some(c) = self.clip.and_then(|c| self.m()?.clips.get(c).cloned()) {
                self.t = if c.looping { (self.t + dt).rem_euclid(c.length) } else { (self.t + dt).min(c.length) };
            }
            ui.ctx().request_repaint();
        }
        app.view.live = self.playing;
        let now = ui.input(|i| i.time);
        if now - self.looked >= 1.0 {
            self.looked = now;
            self.outside(app);
        }
        if let Some(name) = self.ask.clone() {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(look::DRAFT, format!("{name}.toml changed on disk while it has unsaved work here."));
                if look::tip(ui.button("Reload"), "Take the disk's model; yours stays one undo step back", "").clicked() {
                    self.take_disk(app, &name);
                    self.ask = None;
                }
                if look::tip(ui.button("Keep mine"), "Keep the Studio's model; the next save writes it over the disk's", "").clicked() {
                    if let (Some(d), Ok(text)) = (self.docs.get_mut(&name), std::fs::read_to_string(format!("{MODELS}/{name}.toml"))) {
                        d.saved = MDoc::load(&name, &text, &app.pal).ok().map(|f| f.text());
                    }
                    self.ask = None;
                }
            });
        }
        if self.choosing || self.m().is_none() {
            self.starters(ui, app);
            return;
        }
        app.view.controls(ui);
        self.feed_view(app);
        // (no clip chosen: the timeline is its one line of buttons, and the view has the rest)
        let timeline_h = if self.clip.is_none() { 62.0 } else { 62.0 + 18.0 * (self.m().map_or(1, |m| m.parts.len()) as f32 + 2.0) + GAP };
        // the view over the timeline, the bar between them dragged; until then the timeline has what its rows need
        let total = ui.available_height();
        let view_share = ((total - timeline_h - GAP) / total.max(1.0)).clamp(0.1, 0.9);
        let view_h = look::split(ui, "view", view_share, total) - GAP;
        app.view.orbit = self.orbit;
        app.view.model = Some(ModelCam { flat: self.flat, pivot: self.turn_point(), right_turns: !matches!(self.tool, Tool::Box | Tool::Line) });
        let resp = {
            let me = &*self;
            app.view.ui(ui, view_h, |p, v, _| me.sketch(p, v))
        };
        self.orbit = app.view.orbit;
        self.pointer(app, &resp);
        let painter = ui.painter_at(resp.rect);
        self.find_faces(&app.view, ui.input(|i| i.pointer.any_down()));
        self.overlay(&painter, &app.view);
        self.view_cube(ui, app, resp.rect);
        self.view_bar(ui, resp.rect);
        let tool = TOOLS.iter().find(|t| t.0 == self.tool).unwrap();
        look::tip(resp, &format!("{}: {}", tool.2, tool.4), tool.3.name());
        look::split_bar(ui, "view", view_share, total);
        self.timeline(ui, app);
    }

    fn inspector(&mut self, ui: &mut Ui, app: &mut App) {
        if let Some(mut sec) = look::section(ui, "Tools") {
            let ui = &mut *sec;
            let w = ((ui.available_width() - 8.0) / 3.0).floor() - 1.0;
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(4.0);
                for (t, _, name, k, help) in TOOLS {
                    if look::tip(look::tool(ui, self.tool == t, name, k.name(), w), help, k.name()).clicked() {
                        self.tool = t;
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(4.0);
                look::tip(look::toggle(ui, &mut self.mirror.0, "Mirror x X"), "Every edit also on the far side of the part's middle in x", "X");
                look::tip(look::toggle(ui, &mut self.mirror.1, "Mirror z Z"), "Every edit also on the far side of the part's middle in z", "Z");
            });
        }
        if let Some(mut sec) = look::section(ui, "Colours") {
            let ui = &mut *sec;
            palette_picker(ui, "models", &app.pal, &mut self.first, &mut self.second);
            if let Some(n) = self.m().map(|m| m.colours.len()) {
                let max = super::grow::MODEL_MAX;
                let t = RichText::new(format!("This model: {n} of {max} colours")).small();
                let r = ui.label(if n + 6 >= max { t.color(look::DRAFT) } else { t.weak() });
                look::tip(r, &format!("A model's voxel rows have one letter per colour, so one model holds at most {max}; the palette itself has no limit"), "");
            }
        }
        if let Some(mut sec) = look::section(ui, "Model") {
            let ui = &mut *sec;
            ui.horizontal(|ui| {
                if look::tip(ui.button("New model"), "Start a new model from a shape: a block, a slab, a post, a tool, a plant cross or a creature", "Ctrl+N").clicked() {
                    self.run(app, "new");
                }
                ui.menu_button("Plant…", |ui| {
                    for kind in voxel::PLANTS {
                        if ui.button(kind).clicked() {
                            self.run(app, &format!("plant:{kind}"));
                            ui.close();
                        }
                    }
                })
                .response
                .on_hover_text("A small plant grown as a new model, to finish by hand; again grows another");
            });
            ui.horizontal(|ui| {
                if look::tip(ui.button("Shade by light"), "Every voxel of the part along its ramp by how open it is: open tops lighter, crevices and undersides darker", "").clicked() {
                    self.run(app, "shade");
                }
                ui.spacing_mut().slider_width = (ui.available_width() - 60.0).max(40.0);
                look::tip(ui.add(egui::Slider::new(&mut self.strength, 1.0..=6.0).fixed_decimals(1)), "How strongly: more steps along the ramp", "");
            });
            let painted: Vec<String> = app.reg.placeable().map(|d| d.name.clone()).filter(|n| (0..4).any(|f| app.docs.contains_key(&art::file_name(n, f)))).collect();
            ui.horizontal(|ui| {
                let w = (ui.available_width() - 70.0).max(60.0);
                let r = egui::ComboBox::from_id_salt("from texture").selected_text(if self.tex_block.is_empty() { "a painted block" } else { &self.tex_block }).width(w).show_ui(ui, |ui| {
                    if painted.is_empty() {
                        look::weak(ui, "no block is painted yet");
                    }
                    for n in &painted {
                        ui.selectable_value(&mut self.tex_block, n.clone(), n);
                    }
                });
                look::tip(r.response, "A painted block to start a part from", "");
                if look::tip(ui.add_enabled(!self.tex_block.is_empty(), egui::Button::new("Carve")), "From a texture: the block's faces become a new 8-voxel cube part to carve", "").clicked() {
                    let b = self.tex_block.clone();
                    self.run(app, &format!("fromtex:{b}"));
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(4.0);
                if look::tip(look::chip(ui, self.layer.is_some(), "Layer"), "Layer mode: clicks land on one row's plane and the rows above are dimmed: build floor by floor, or work inside", "").clicked() {
                    self.run(app, "layer.on");
                }
                if let Some(l) = self.layer {
                    if look::tip(ui.small_button("Down"), "The layer a row down", "PgDn").clicked() {
                        self.run(app, "layer.down");
                    }
                    look::weak(ui, format!("row {l}"));
                    if look::tip(ui.small_button("Up"), "The layer a row up", "PgUp").clicked() {
                        self.run(app, "layer.up");
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(4.0);
                look::weak(ui, "Look from");
                for (k, (id, name, ..)) in VIEWS.iter().enumerate() {
                    let key = format!("{}{}", if k % 2 == 1 { "Shift+" } else { "" }, ["1", "3", "7"][k / 2]);
                    if look::tip(ui.small_button(*name), &format!("Look at the model from the {} (the view cube's face)", name.to_lowercase()), &key).clicked() {
                        self.run(app, &format!("view:{id}"));
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                look::tip(look::toggle(ui, &mut self.show_block, "1 m block"), "A real stone block two cells away, for scale", "");
                look::tip(look::toggle(ui, &mut self.show_player, "Player"), "A player's outline (0.6 x 1.8 m) beside the cell", "");
                look::tip(look::toggle(ui, &mut self.show_grid, "Grid"), "The floor's voxel lines under the part, the 1 m lines heavier", "");
                look::tip(look::toggle(ui, &mut self.edges, "Edges"), "Thin dark lines between voxels, so a face of one colour still reads as voxels", "");
            });
        }
        if let Some(mut sec) = look::section(ui, "Part") {
            let ui = &mut *sec;
            self.part_inspector(ui, app);
        }
        if let Some(mut sec) = look::section(ui, "Point") {
            let ui = &mut *sec;
            self.point_inspector(ui, app);
        }
        if let Some(mut sec) = look::section(ui, "States") {
            let ui = &mut *sec;
            self.states_inspector(ui, app);
        }
        if let Some(mut sec) = look::section(ui, "Ready-made motion") {
            let ui = &mut *sec;
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(4.0);
                for (k, name, help, amount, _) in MOTIONS {
                    if look::tip(look::chip(ui, self.motion.0 == k, name), help, "").clicked() {
                        self.motion = (k, self.motion.1, amount);
                    }
                }
            });
            ui.horizontal(|ui| {
                for (a, n) in ["x", "y", "z"].iter().enumerate() {
                    if look::tip(look::chip(ui, self.motion.1 == a, n), "The axis it turns about or moves along", "").clicked() {
                        self.motion.1 = a;
                    }
                }
                let unit = MOTIONS.iter().find(|m| m.0 == self.motion.0).map_or("", |m| m.4);
                look::tip(ui.add(egui::DragValue::new(&mut self.motion.2).speed(0.5).suffix(format!(" {unit}"))), "How far", "");
                if look::tip(ui.button("Fill"), "Fill the clip's keys of the chosen part with this motion", "").clicked() {
                    self.run(app, "motion");
                }
            });
        }
        if let Some((a, b)) = self.sel {
            if let Some(mut sec) = look::section(ui, "Selection") {
                let ui = &mut *sec;
                let (lo, hi) = (a.min(b), a.max(b));
                look::weak(ui, format!("{} x {} x {} voxels; the arrows and Shift + Page Up/Down move it, a drag on one of its faces pulls it out", hi.x - lo.x + 1, hi.y - lo.y + 1, hi.z - lo.z + 1));
            }
        }
    }

    fn help(&self) -> String {
        let t = TOOLS.iter().find(|t| t.0 == self.tool).unwrap();
        format!("{}: {}   [{}]", t.2, t.4, t.3.name())
    }

    fn history(&self, _app: &App) -> Option<(Vec<String>, Vec<String>)> {
        let d = self.doc()?;
        Some((d.hist.undo.iter().map(|s| s.name.clone()).collect(), d.hist.redo.iter().rev().map(|s| s.name.clone()).collect()))
    }

    fn back_to(&mut self, _app: &mut App, n: usize) {
        if let Some(d) = self.open.as_ref().and_then(|k| self.docs.get_mut(k)) {
            d.step(|h, now| {
                h.back_to(now, n);
                true
            });
            d.rev += 1;
        }
        self.opened_keep();
    }

    fn has_tree(&self) -> bool {
        true
    }
    fn tree(&mut self, ui: &mut Ui, app: &mut App) {
        self.parts_tree(ui, app);
    }

    fn unsaved(&self) -> Vec<String> {
        self.docs.iter().filter(|(_, d)| d.dirty()).map(|(n, _)| format!("{n}.toml")).collect()
    }

    fn save_all(&mut self, app: &mut App) {
        let keep = self.open.clone();
        for n in self.docs.iter().filter(|(_, d)| d.dirty()).map(|(n, _)| n.clone()).collect::<Vec<_>>() {
            self.open = Some(n);
            self.save(app);
        }
        self.open = keep;
    }

    fn kept(&self) -> Option<Vec<(String, Vec<u8>)>> {
        Some(kept(&self.docs))
    }
    fn unsaved_of(&self, files: Vec<(String, Vec<u8>)>) -> Vec<(String, Vec<u8>)> {
        left(&self.docs, files)
    }
    fn restore(&mut self, app: &mut App, files: Vec<(String, Vec<u8>)>) -> Vec<(String, Vec<u8>, String)> {
        let refused = restore(&mut self.docs, files, &app.pal);
        if self.open.is_none() {
            self.open = self.docs.keys().next().cloned();
        }
        self.opened_keep();
        refused
    }

    fn settle(&mut self, ctx: &egui::Context, app: &mut App) {
        app.view.settle(ctx, std::time::Duration::from_secs(90));
    }
}

impl Models {
    /// After an undo: the chosen part and clip kept in range.
    fn opened_keep(&mut self) {
        let Some(m) = self.m() else { return };
        let (np, nc) = (m.parts.len(), m.clips.len());
        self.part = self.part.min(np.saturating_sub(1));
        if self.clip.is_some_and(|c| c >= nc) {
            self.clip = (nc > 0).then_some(0);
        }
        self.part_name = self.m().and_then(|m| m.parts.get(self.part)).map_or(String::new(), |p| p.name.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::models::parse_life;

    const FURNACE: &str = include_str!("../../../../data/models/furnace.toml");
    fn pal() -> engine::palette::Palette {
        engine::palette::Palette::parse(include_str!("../../../../data/palette.toml")).unwrap()
    }

    #[test]
    fn saving_keeps_the_states_and_events() {
        let (_, life) = parse_life("furnace", FURNACE, &pal(), &|_| true, &|_| true).unwrap();
        assert!(!life.states.is_empty() && !life.events.is_empty());
        let d = MDoc::load("furnace", FURNACE, &pal()).unwrap();
        let (_, back) = parse_life("furnace", &d.text(), &pal(), &|_| true, &|_| true).unwrap();
        assert_eq!(back, life, "the page's save dropped the model's life");
    }

    #[test]
    fn an_unsaved_model_is_kept_offered_back_after_a_crash_and_cleared_on_save() {
        use crate::recover::Recovery;
        use std::time::{Duration, Instant};
        let root = std::env::temp_dir().join(format!("technoape-models-recover-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let open = || BTreeMap::from([("furnace".to_string(), MDoc::load("furnace", FURNACE, &pal()).unwrap())]);
        let mut docs = open();
        assert!(kept(&docs).is_empty(), "nothing unsaved yet");
        // an edit, and a new model never saved
        let d = docs.get_mut("furnace").unwrap();
        d.m.parts[0].pivot = [3.0, 0.0, 4.0];
        let edited = d.text();
        assert!(d.dirty());
        let m = d.m.clone();
        docs.insert("post".into(), MDoc { m, life: Life::default(), hist: History::default(), saved: None, rev: 0, old: String::new() });
        let t0 = Instant::now();
        let mut r = Recovery::new(&root, "models", Duration::from_secs(3));
        assert!(r.keep(t0, || kept(&docs)));
        // a crash: the next start opens the files as saved and finds the set
        let mut again = open();
        let back = left(&again, Recovery::new(&root, "models", Duration::from_secs(3)).left());
        let names: Vec<&str> = back.iter().map(|f| f.0.as_str()).collect();
        assert_eq!(names, ["furnace.toml", "post.toml"], "both offered back");
        assert!(restore(&mut again, back, &pal()).is_empty(), "no errors");
        assert_eq!(again["furnace"].text(), edited, "the edit is back");
        assert!(again["furnace"].dirty() && again["furnace"].hist.undo.len() == 1, "as unsaved work, one undo step");
        assert!(again["post"].saved.is_none(), "a new model stays new");
        // what is the file as saved is not offered
        assert!(left(&open(), vec![("furnace.toml".into(), open()["furnace"].text().into_bytes())]).is_empty(), "the file as saved is not offered");
        // saved (as `save` marks it): nothing kept, the set is cleared
        for d in again.values_mut() {
            d.saved = Some(d.text());
        }
        assert!(r.keep(t0 + Duration::from_secs(4), || kept(&again)));
        assert!(Recovery::new(&root, "models", Duration::from_secs(3)).left().is_empty(), "cleared on save");
        let _ = std::fs::remove_dir_all(&root);
    }
}

/// A pick list: the current name (or `none`'s words); a click on another gives Some(the new one; None for `none`).
fn pick(ui: &mut Ui, id: impl std::hash::Hash + std::fmt::Debug, cur: Option<&str>, opts: &[String], none: Option<&str>, w: f32) -> Option<Option<String>> {
    let mut out = None;
    egui::ComboBox::from_id_salt(id).selected_text(cur.or(none).unwrap_or("")).width(w).show_ui(ui, |ui| {
        if let Some(n) = none {
            if ui.selectable_label(cur.is_none(), n).clicked() {
                out = Some(None);
            }
        }
        for o in opts {
            if ui.selectable_label(cur == Some(o.as_str()), o).clicked() {
                out = Some(Some(o.clone()));
            }
        }
    });
    out
}

/// A name box that shows `name` until it is typed in; Some(the typed name) when Enter or a click away ends it.
fn name_box(ui: &mut Ui, buf: &mut String, id: &str, name: &str, help: &str, w: f32) -> Option<String> {
    let id = ui.make_persistent_id(id);
    if !ui.memory(|m| m.has_focus(id)) {
        *buf = name.to_string();
    }
    let r = ui.add(egui::TextEdit::singleline(buf).id(id).desired_width(w));
    (look::tip(r, help, "").lost_focus() && buf.trim() != name).then(|| buf.trim().to_string())
}

impl Models {
    /// The chosen point: its name (renamed everywhere a state or event names it) and its removal.
    fn point_inspector(&mut self, ui: &mut Ui, app: &mut App) {
        let Some(m) = self.m().cloned() else { return };
        let Some(i) = self.point.filter(|i| *i < m.points.len()) else {
            look::weak(ui, "Choose a point in the tree (the Point tool O adds one)");
            return;
        };
        let pt = &m.points[i];
        ui.horizontal(|ui| {
            ui.label(RichText::new("Name").small());
            let mut buf = std::mem::take(&mut self.names[1]);
            if let Some(new) = name_box(ui, &mut buf, "point name", &pt.name, "The point's name; Enter renames it in every state and event that names it", (ui.available_width() - 70.0).clamp(60.0, 160.0)) {
                let mut err = None;
                self.edit_life(app, "Rename a point", |m, life, _| voxel::rename_point(m, life, i, &new).map_err(|e| err = Some(e)).is_ok());
                if let Some(e) = err {
                    app.refuse(e);
                }
            }
            self.names[1] = buf;
            if look::tip(ui.button("Remove"), "Remove the point: the state effects, lights and event places that name it are cleared", "").clicked() {
                self.edit_life(app, "Remove a point", |m, _, _| {
                    m.points.remove(i);
                    true
                });
                self.point = None;
            }
        });
        look::weak(ui, format!("on {} at {:?}", pt.part, pt.at));
    }

    /// The model's states (ADR 0050): the list, and the chosen one's clip, effects, sound and light; Play state
    /// shows it in the view, in the game's light.
    fn states_inspector(&mut self, ui: &mut Ui, app: &mut App) {
        let Some(d) = self.doc() else { return };
        let (m, life) = (d.m.clone(), d.life.clone());
        let points: Vec<String> = m.points.iter().map(|p| p.name.clone()).collect();
        let clips: Vec<String> = m.clips.iter().map(|c| c.name.clone()).collect();
        let effects = self.effects.clone();
        let sounds = self.sounds.clone();
        ui.horizontal_wrapped(|ui| {
            for (i, s) in life.states.iter().enumerate() {
                if look::tip(ui.selectable_label(self.state == Some(i), &s.name), "Choose this state", "").clicked() {
                    self.state = Some(i);
                }
            }
            if look::tip(ui.button("Add"), "A new state, playing the timeline's clip", "").clicked() {
                let clip = self.clip_name();
                let mut at = 0;
                self.edit_life(app, "Add a state", |_, life, _| {
                    let name = (1..).map(|k| if k == 1 { "working".to_string() } else { format!("state{k}") }).find(|n| !life.states.iter().any(|s| &s.name == n)).unwrap();
                    life.states.push(State { name, clip, ..Default::default() });
                    at = life.states.len() - 1;
                    true
                });
                self.state = Some(at);
            }
        });
        let Some(si) = self.state.filter(|i| *i < life.states.len()) else {
            look::weak(ui, "No state: Add makes one (a clip, effects at points, a looping sound, a light)");
            return;
        };
        let st = life.states[si].clone();
        let w = (ui.available_width() - 60.0).max(80.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Name").small());
            let mut buf = std::mem::take(&mut self.names[0]);
            if let Some(new) = name_box(ui, &mut buf, "state name", &st.name, "The state's name, one word (the game asks for it by name); Enter renames it", (ui.available_width() - 70.0).clamp(60.0, 160.0)) {
                if new.is_empty() || new.contains(char::is_whitespace) || life.states.iter().any(|s| s.name == new) {
                    app.refuse(format!("'{new}' is empty, has a space, or is another state's name"));
                } else {
                    self.edit_life(app, "Rename a state", |_, life, _| {
                        life.states[si].name = new.clone();
                        true
                    });
                    if self.playing_state.as_deref() == Some(st.name.as_str()) {
                        self.playing_state = Some(new);
                    }
                }
            }
            self.names[0] = buf;
            if look::tip(ui.button("Remove"), "Remove this state", "").clicked() {
                self.edit_life(app, "Remove a state", |_, life, _| {
                    life.states.remove(si);
                    true
                });
                self.state = si.checked_sub(1).or((life.states.len() > 1).then_some(0));
                self.playing_state = None;
            }
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("Clip").small());
            if let Some(c) = pick(ui, "state clip", st.clip.as_deref(), &clips, Some("none (idle)"), w) {
                self.edit_life(app, "A state's clip", |_, life, _| {
                    life.states[si].clip = c;
                    true
                });
            }
        });
        look::weak(ui, "Effects, lasting at points:");
        for (k, e) in st.effects.iter().enumerate() {
            ui.horizontal(|ui| {
                let half = ((ui.available_width() - 40.0) / 2.0).max(60.0);
                if let Some(Some(n)) = pick(ui, ("state fx", k), Some(&e.effect), &effects, None, half) {
                    self.edit_life(app, "A state's effect", |_, life, _| {
                        life.states[si].effects[k].effect = n;
                        true
                    });
                }
                if let Some(Some(p)) = pick(ui, ("state fx point", k), Some(&e.point), &points, None, half) {
                    self.edit_life(app, "An effect's point", |_, life, _| {
                        life.states[si].effects[k].point = p;
                        true
                    });
                }
                if look::tip(ui.small_button("x"), "Remove this effect", "").clicked() {
                    self.edit_life(app, "Remove an effect", |_, life, _| {
                        life.states[si].effects.remove(k);
                        true
                    });
                }
            });
        }
        if look::tip(ui.add_enabled(!points.is_empty() && !effects.is_empty(), egui::Button::new("Add an effect")), "An effect lasting at a point while the state holds (the model needs a point)", "").clicked() {
            let (effect, point) = (effects[0].clone(), points[0].clone());
            self.edit_life(app, "Add an effect", |_, life, _| {
                life.states[si].effects.push(StateEffect { effect, point });
                true
            });
        }
        ui.horizontal(|ui| {
            ui.label(RichText::new("Sound").small());
            if let Some(s) = pick(ui, "state sound", st.sound.as_deref(), &sounds, Some("none"), w) {
                self.edit_life(app, "A state's sound", |_, life, _| {
                    life.states[si].sound = s;
                    true
                });
            }
        });
        let mut lit = st.light.is_some();
        if look::tip(ui.add_enabled(!points.is_empty(), egui::Checkbox::new(&mut lit, "Light")), "A light at a point while the state holds", "").changed() {
            let colour = app.pal.colours.iter().find(|c| c.name == "lamp").or(app.pal.colours.first()).map_or(String::new(), |c| c.name.clone());
            let light = lit.then(|| Light { point: points[0].clone(), colour, strength: 1.0, rgb: [1.0; 3] });
            self.edit_life(app, "A state's light", |_, life, _| {
                life.states[si].light = light;
                true
            });
        }
        if let Some(l) = &st.light {
            let colours: Vec<String> = app.pal.colours.iter().map(|c| c.name.clone()).collect();
            ui.horizontal(|ui| {
                let half = ((ui.available_width() - 30.0) / 2.0).max(60.0);
                if let Some(Some(p)) = pick(ui, "light point", Some(&l.point), &points, None, half) {
                    self.edit_life(app, "A light's point", |_, life, _| {
                        life.states[si].light.as_mut().unwrap().point = p;
                        true
                    });
                }
                if let Some(Some(c)) = pick(ui, "light colour", Some(&l.colour), &colours, None, half) {
                    let rgb = app.pal.colours.iter().find(|k| k.name == c).map_or([1.0; 3], |k| k.rgb.map(|v| v as f32 / 255.0));
                    self.edit_life(app, "A light's colour", |_, life, _| {
                        let l = life.states[si].light.as_mut().unwrap();
                        (l.colour, l.rgb) = (c, rgb);
                        true
                    });
                }
            });
            let mut k = l.strength;
            ui.horizontal(|ui| {
                ui.spacing_mut().slider_width = (ui.available_width() - 60.0).max(40.0);
                if look::tip(ui.add(egui::Slider::new(&mut k, 0.1..=3.0).fixed_decimals(2)), "The light's strength", "").changed() {
                    self.edit_life_merged(app, "A light's strength", |_, life, _| {
                        life.states[si].light.as_mut().unwrap().strength = k;
                        true
                    });
                }
            });
        }
        let on = self.playing_state.as_deref() == Some(st.name.as_str());
        let r = ui.add(egui::Button::new(if on { "Stop state" } else { "Play state" }).selected(on));
        if look::tip(r, "Show the model in this state in the view, by the game's renderer: its clip, effects, light and events (unsaved edits too; after editing a playing state, stop and play it again)", "").clicked() {
            self.playing_state = (!on).then(|| st.name.clone());
        }
    }

    /// The timeline's chosen event: its sound, its effect, its point, and its removal.
    fn event_bar(&mut self, ui: &mut Ui, app: &mut App, clip: &str) {
        let Some(d) = self.doc() else { return };
        let points: Vec<String> = d.m.points.iter().map(|p| p.name.clone()).collect();
        let evs = d.life.events.iter().find(|(c, _)| c == clip).map_or(vec![], |(_, e)| e.clone());
        let Some(k) = self.event.filter(|k| *k < evs.len()) else { return };
        let e = evs[k].clone();
        let (effects, sounds) = (self.effects.clone(), self.sounds.clone());
        ui.separator();
        look::weak(ui, format!("event at {:.2} s", e.t));
        let mut change: Option<(&str, Event)> = None;
        if let Some(v) = pick(ui, "event sound", e.sound.as_deref(), &sounds, Some("no sound"), 110.0) {
            change = Some(("An event's sound", Event { sound: v, ..e.clone() }));
        }
        if let Some(v) = pick(ui, "event effect", e.effect.as_deref(), &effects, Some("no effect"), 90.0) {
            change = Some(("An event's effect", Event { effect: v, ..e.clone() }));
        }
        if let Some(v) = pick(ui, "event point", e.point.as_deref(), &points, Some("at the foot"), 90.0) {
            change = Some(("An event's point", Event { point: v, ..e.clone() }));
        }
        let clip = clip.to_string();
        if let Some((name, new)) = change {
            self.edit_life(app, name, |_, life, _| {
                voxel::events_mut(life, &clip)[k] = new;
                true
            });
        }
        if look::tip(ui.button("Delete event"), "Remove this event (one left with no sound and no effect goes too)", "").clicked() {
            self.edit_life(app, "Delete an event", |_, life, _| {
                voxel::events_mut(life, &clip).remove(k);
                true
            });
            self.event = None;
        }
    }
}

impl Models {
    /// The model files changed by another program: an unedited one is reloaded, an edited one asks (the
    /// Textures page's rule, `shell::outside`).
    fn outside(&mut self, app: &mut App) {
        for name in self.docs.keys().cloned().collect::<Vec<_>>() {
            let path = format!("{MODELS}/{name}.toml");
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            // (a file the game would refuse is left to the file check)
            let Ok(disk) = MDoc::load(&path, &text, &app.pal).map(|d| d.text()) else { continue };
            let d = &self.docs[&name];
            match crate::shell::outside(d.dirty(), Some(&disk), d.saved.as_ref()) {
                crate::shell::Outside::Nothing => {}
                crate::shell::Outside::Reload => {
                    self.take_disk(app, &name);
                    app.toast(format!("Reloaded {name}.toml: changed on disk"));
                }
                crate::shell::Outside::Ask => self.ask = Some(name),
            }
        }
    }

    /// The disk's model in place of the Studio's, one undo step on.
    fn take_disk(&mut self, app: &App, name: &str) {
        let path = format!("{MODELS}/{name}.toml");
        let Some(fresh) = std::fs::read_to_string(&path).ok().and_then(|t| MDoc::load(&path, &t, &app.pal).ok()) else { return };
        let Some(d) = self.docs.get_mut(name) else { return };
        d.hist.push((d.m.clone(), d.life.clone()), "Reload from disk");
        (d.m, d.life, d.saved) = (fresh.m, fresh.life, fresh.saved);
        d.rev += 1;
        if self.open.as_deref() == Some(name) {
            self.opened_keep();
        }
    }
}
