//! The Models page (docs/design/studio-tools.md "The Models page", ADR 0049): `data/models/*.toml` drawn by the
//! game's renderer ([`Viewport`]), figures of painted boxes (ADR 0116): painted on the model or on its painting,
//! with parts and pivots, points, states and a timeline of keys. The editing is `voxel`'s, pure and tested; this
//! maps the pointer and keys to it.
use super::look::{self, c32, GAP};
use super::textures::palette_picker;
use super::viewport::{ModelCam, Viewport, DIR};
use super::{App, Cmd, FileRow, Page, Thumb, DRAFTS};
use crate::art::{self, History, Img};
use crate::shell::Flags;
use crate::voxel::{self, Motion, Orbit, PaintHit, MOTIONS, VIEWS};
use eframe::egui::{self, Color32, Key, Modifiers, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};
use engine::models::{parse_life, sample, Clip, Ease, Event, Life, Light, Model, State, StateEffect};
use glam::{Mat4, Vec3};
use std::collections::BTreeMap;

pub const MODELS: &str = "data/models";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Tool {
    Paint,
    Pick,
    Pivot,
    Point,
}
/// Each tool: its command id, name, key and what it does.
pub(super) const TOOLS: [(Tool, &str, &str, Key, &str); 4] = [
    (Tool::Paint, "paint", "Paint", Key::P, "paint on the model in the view, or on the painting beside it: left the first colour, right the second (the Eraser empties pixels)"),
    (Tool::Pick, "pick", "Picker", Key::I, "left click takes the colour under the pointer as the first, right as the second"),
    (Tool::Pivot, "pivot", "Pivot", Key::V, "drag the yellow cross: the point the part turns about"),
    (Tool::Point, "point", "Point", Key::O, "click the model to add a point there (where a state's effects and light sit); drag a point's cross to move it"),
];

/// What each mouse button does with a tool, in the view's bar.
fn buttons(tool: Tool) -> &'static str {
    match tool {
        Tool::Paint => "left paints the first colour, right the second",
        Tool::Pick => "left takes the colour as the first, right as the second",
        Tool::Pivot => "left drag moves the yellow cross, the point the part turns about",
        Tool::Point => "left click adds a point on the model; drag a point's cross to move it",
    }
}

/// One model file open in the Studio: the model, its undo, its text as on disk, and a count of its edits.
struct MDoc {
    m: Model,
    /// Its states and clip events (ADR 0050), kept through every edit and undo with the model.
    life: Life,
    /// One undo for the model, its life and its painting (a stroke a step).
    hist: History<(Model, Life, Img)>,
    saved: Option<String>,
    rev: u64,
    /// The file's text as opened: its comments and line ends are kept when it is saved (`write_keeping`).
    old: String,
    /// Its painting `<name>.png` (ADR 0081, 0116), 0 x 0 for none; as on disk.
    paint: Img,
    paint_saved: Option<Img>,
    /// Why its `.png` on disk was refused: it is never written over.
    paint_refused: Option<String>,
}
impl MDoc {
    /// A file's text as the page opens it (its effects and sounds are checked on save, not here).
    fn load(path: &str, text: &str, pal: &engine::palette::Palette) -> Result<MDoc, String> {
        let (m, life) = parse_life(path, text, pal, &|_| true, &|_| true)?;
        let read = path.strip_suffix(".toml").and_then(|p| std::fs::read(format!("{p}.png")).ok()).map(|b| engine::models::painting(&b, pal));
        let paint_refused = read.as_ref().and_then(|r| r.as_ref().err().cloned());
        let paint_saved = read.and_then(|r| r.ok()).map(|p| Img { w: p.w as usize, h: p.h as usize, px: p.px.iter().map(|c| if c[3] == 0 { art::EMPTY } else { [c[0], c[1], c[2]] }).collect() });
        let paint = paint_saved.clone().unwrap_or(NO_PAINT);
        let mut d = MDoc { m, life, hist: History::default(), saved: None, rev: 0, old: text.to_string(), paint, paint_saved, paint_refused };
        d.saved = Some(d.text());
        Ok(d)
    }
    /// The file's text as the page saves it.
    fn text(&self) -> String {
        voxel::text_keeping(&self.old, &self.m, &self.life)
    }
    /// An undo, redo or step back on the model, its life and its painting together.
    fn step(&mut self, f: impl FnOnce(&mut History<(Model, Life, Img)>, &mut (Model, Life, Img)) -> bool) -> bool {
        let mut now = (self.m.clone(), std::mem::take(&mut self.life), std::mem::replace(&mut self.paint, NO_PAINT));
        let done = f(&mut self.hist, &mut now);
        (self.m, self.life, self.paint) = now;
        done
    }
    fn dirty(&self) -> bool {
        self.saved.as_deref() != Some(self.text().as_str()) || self.paint_dirty()
    }
    fn paint_dirty(&self) -> bool {
        self.paint.w > 0 && self.paint_saved.as_ref() != Some(&self.paint)
    }
}

const NO_PAINT: Img = Img { w: 0, h: 0, px: vec![] };

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
                let before = (std::mem::replace(&mut d.m, new.m), std::mem::replace(&mut d.life, new.life), d.paint.clone());
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

/// A drag in the view: what it moves (a pivot, a point).
#[derive(Clone, Copy)]
struct Drag {
    what: Grab,
}
#[derive(Clone, Copy, PartialEq)]
enum Grab {
    Pivot,
    Point(usize),
}

pub struct Models {
    docs: BTreeMap<String, MDoc>,
    open: Option<String>,
    part: usize,
    tool: Tool,
    first: [u8; 3],
    second: [u8; 3],
    drag: Option<Drag>,
    /// The painted pixel under the pointer in the view.
    hover: Option<PaintHit>,
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
    part_name: String,
    /// What the renderer was last given: the model's name, edit count, and when its painting was written.
    written: Option<(String, u64, Option<std::time::SystemTime>)>,
    gen: u64,
    /// One history step for a whole drag of a value.
    merging: Option<String>,
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
    /// The Paint tool's eraser, and a stroke under way (one undo step).
    erase: bool,
    stroking: bool,
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
            tool: Tool::Paint,
            first: at(1),
            second: at(0),
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
            part_name: String::new(),
            written: None,
            gen: 0,
            merging: None,
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
            erase: false,
            stroking: false,
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
        (self.clip, self.t, self.key, self.drag, self.playing) = (clip, 0.0, None, None, false);
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
        let Some(d) = self.open.as_ref().and_then(|n| self.docs.get_mut(n)) else { return };
        let before = (d.m.clone(), d.life.clone(), d.paint.clone());
        if f(&mut d.m, &mut d.life, &pal) {
            voxel::fit_life(&d.m, &mut d.life);
            if (&d.m, &d.life) != (&before.0, &before.1) {
                if !merge {
                    d.hist.push(before, name);
                }
                d.rev += 1;
            }
        } else {
            (d.m, d.life, _) = before;
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

    /// One edit of the open model's painting; `merge` keeps a stroke one undo step.
    /// Whether it changed the painting.
    fn edit_paint(&mut self, name: &str, merge: bool, f: impl FnOnce(&mut Img)) -> bool {
        let Some(d) = self.open.as_ref().and_then(|n| self.docs.get_mut(n)) else { return false };
        let before = d.paint.clone();
        f(&mut d.paint);
        if d.paint == before {
            return false;
        }
        if !merge {
            d.hist.push((d.m.clone(), d.life.clone(), before), name);
        }
        d.rev += 1;
        true
    }
    /// A change of boxes that also grows the painting (`voxel::add_box`, `voxel::voxels_to_boxes`): one step for
    /// both, or a refusal and neither.
    fn box_edit(&mut self, app: &mut App, name: &str, f: impl FnOnce(&mut Model, &mut Img) -> Result<(), String>) {
        let Some(d) = self.doc() else { return };
        let (mut m, mut paint) = (d.m.clone(), d.paint.clone());
        match f(&mut m, &mut paint) {
            Ok(()) => self.edit(app, name, |mm, _| {
                *mm = m;
                true
            }),
            Err(e) => return app.refuse(format!("{name}: not done. {e}")),
        }
        if let Some(d) = self.open.as_ref().and_then(|n| self.docs.get_mut(n)) {
            if d.paint != paint {
                // (the model's step above holds the painting as it was: one undo takes both back)
                if d.hist.undo.last().is_none_or(|s| s.name != name) {
                    d.hist.push((d.m.clone(), d.life.clone(), d.paint.clone()), name);
                }
                d.paint = paint;
                d.rev += 1;
            }
        }
    }

    /// One pixel painted (left the first colour, right the second, the eraser empties it); a stroke is one step.
    fn dab(&mut self, x: usize, y: usize, left: bool) {
        let c = if self.erase { art::EMPTY } else if left { self.first } else { self.second };
        let merge = self.stroking;
        // (a stroke merges only once its first change made it a step: one begun on a pixel already that colour does
        // not fold into the step before it)
        self.stroking |= self.edit_paint(if self.erase { "Erase" } else { "Paint" }, merge, |img| {
            if x < img.w && y < img.h {
                img.px[y * img.w + x] = c;
            }
        });
    }

    /// The model's painting, flat, filling `rect`: the open part's box nets outlined, and the pixel under the
    /// pointer in the view lit; the tool works here as on the model.
    fn painting(&mut self, ui: &mut Ui, rect: Rect) {
        let Some(d) = self.doc() else { return };
        let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(egui::Align::Min)));
        let ui = &mut ui;
        if d.paint.w == 0 {
            look::weak(ui, "No painting yet: Add a box (inspector, Part) starts one.");
            return;
        }
        let img = d.paint.clone();
        let mut outline: Vec<[u32; 4]> = d.m.parts.get(self.part).map_or(vec![], |p| p.boxes.iter().flat_map(|b| b.rects()).collect());
        if let Some(h) = self.hover {
            outline.push([h.x as u32, h.y as u32, 1, 1]);
        }
        look::weak(ui, format!("Painting, {} x {} px. Yellow: the open part's boxes.", img.w, img.h));
        let r = ui.available_rect_before_wrap();
        let hit = super::plants::paint_canvas(ui, r, &img, "model painting", &outline, "The model's painting: the tool works here as on the model");
        crate::drive::mark(ui.ctx(), "painting", r);
        if let Some((x, y, left)) = hit {
            match self.tool {
                Tool::Pick => {
                    if let Some(c) = Some(img.px[y * img.w + x]).filter(|c| *c != art::EMPTY) {
                        *(if left { &mut self.first } else { &mut self.second }) = c;
                    }
                }
                Tool::Paint => self.dab(x, y, left),
                _ => {}
            }
        }
    }

    fn save(&mut self, app: &mut App) {
        let Some(name) = self.open.clone() else { return };
        let Some(d) = self.docs.get_mut(&name) else { return };
        let text = d.text();
        let path = format!("{MODELS}/{name}.toml");
        // the game's own reader first: a model it would refuse is never written (its effects and sounds too)
        let (fx, snd) = (&self.effects, &self.sounds);
        let m = match parse_life(&path, &text, &app.pal, &|n| fx.is_empty() || fx.iter().any(|x| x == n), &|n| snd.is_empty() || snd.iter().any(|x| x == n)) {
            Ok((m, _)) => m,
            Err(e) => return app.refuse(format!("Not saved: {e}")),
        };
        // its painting too, held to the game's reader and to every box's net, before anything is written
        let png = format!("{MODELS}/{name}.png");
        if let Some(e) = d.paint_refused.as_ref().filter(|_| d.paint_dirty()) {
            return app.refuse(format!("Not saved: {png} is on disk but refused ({e}); fix or remove it first"));
        }
        if d.paint_dirty() {
            let read = engine::models::painting(&art::encode(&d.paint), &app.pal).and_then(|p| m.check_painting(&p));
            if let Err(e) = read.and_then(|_| art::save(std::path::Path::new(&png), &d.paint, false)) {
                return app.refuse(format!("Not saved: {png}: {e}"));
            }
            d.paint_saved = Some(d.paint.clone());
            app.note(&png);
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

    /// A new figure named after `base` (a free name: "post", "post 2", ...) from starter shape `kind`: its parts
    /// made painted boxes in their colours, to paint (ADR 0116). None made, nothing opens.
    fn new_model(&mut self, app: &mut App, base: &str, kind: &str) {
        let name = (1u64..).map(|k| if k == 1 { base.to_string() } else { format!("{base} {k}") }).find(|n| !self.docs.contains_key(n) && !std::path::Path::new(&format!("{MODELS}/{n}.toml")).exists()).unwrap();
        let Some(mut m) = voxel::starter(&app.pal, kind) else { return app.refuse(format!("No new model: there is no starter shape '{kind}'")) };
        let mut paint = NO_PAINT;
        if let Err(e) = voxel::voxels_to_boxes(&mut m, &mut paint) {
            return app.refuse(format!("No new model: the {kind} could not be made of painted boxes ({e})"));
        }
        self.docs.insert(name.clone(), MDoc { m, life: Life::default(), hist: History::default(), saved: None, rev: 0, old: String::new(), paint, paint_saved: None, paint_refused: None });
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
            look::weak(ui, "It opens as painted boxes, one a part. Paint on it in the view or on its painting beside it; a right drag turns the view.");
        } else {
            look::heading(ui, "New model: click a shape to start from");
            look::weak(ui, "It opens as a new model of painted boxes beside the others, to paint and shape; the open one is kept as it is.");
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
        if self.m().is_some() {
            ui.add_space(GAP);
            if look::tip(ui.button("Cancel"), "Back to the open model", "Esc").clicked() {
                self.choosing = false;
            }
        }
    }

    /// The open model as the renderer reads it, written when it changed.
    fn feed_view(&mut self, app: &mut App) {
        let Some(name) = self.open.clone() else {
            app.view.show_model(None, None, 0.0);
            return;
        };
        let rev = self.docs[&name].rev;
        // (and its painting's time: a repainted .png is handed over again with the model unchanged)
        let painted = std::fs::metadata(format!("{MODELS}/{name}.png")).and_then(|m| m.modified()).ok();
        let now = Some((name.clone(), rev, painted));
        if self.written != now {
            let (shown, life) = (&self.docs[&name].m, &self.docs[&name].life);
            let dir = format!("{DIR}/models");
            let _ = std::fs::create_dir_all(&dir);
            // the unsaved copy with its life, so a state plays before it is saved
            // and its painting, so its painted boxes are drawn too (ADR 0114), written whole under another name and
            // renamed, so the renderer never reads half of it; none, none left from another model
            let (tmp, png) = (format!("{dir}/studio.png.part"), format!("{dir}/studio.png"));
            let paint = &self.docs[&name].paint;
            if paint.w == 0 || std::fs::write(&tmp, art::encode(paint)).and_then(|_| std::fs::rename(&tmp, &png)).is_err() {
                let _ = std::fs::remove_file(&png);
            }
            if std::fs::write(format!("{dir}/studio.toml"), voxel::text_life(shown, life)).is_ok() {
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

    /// A part's painted boxes' corners in the model's metres, posed as drawn (a part of none: its voxel box's).
    fn part_corners(&self, part: usize) -> Vec<Vec3> {
        let (Some(m), Some(pose)) = (self.m(), self.poses().get(part).copied()) else { return vec![] };
        let faces: Vec<_> = m.box_faces().into_iter().filter(|f| f.part == part).collect();
        if faces.is_empty() {
            let s = m.parts.get(part).map_or(Vec3::ONE, |p| Vec3::from(p.size.map(|v| v as f32)));
            return (0..8).map(|k: u8| pose.transform_point3(Vec3::new([0.0, s.x][(k & 1) as usize], [0.0, s.y][(k >> 1 & 1) as usize], [0.0, s.z][(k >> 2 & 1) as usize]))).collect();
        }
        faces.iter().flat_map(|f| [f.o, f.o + f.eu, f.o + f.ev, f.o + f.eu + f.ev]).map(|p| pose.transform_point3(p)).collect()
    }
    /// A part's bounds, in metres.
    fn part_box(&self, part: usize) -> Option<(Vec3, Vec3)> {
        let c = self.part_corners(part);
        (!c.is_empty()).then(|| c.iter().fold((Vec3::INFINITY, Vec3::NEG_INFINITY), |(a, b), p| (a.min(*p), b.max(*p))))
    }
    /// What the view turns about: the model's middle.
    fn turn_point(&self) -> Option<Vec3> {
        let b = (0..self.m()?.parts.len()).filter_map(|i| self.part_box(i)).reduce(|(a, b), (c, d)| (a.min(c), b.max(d)))?;
        Some((b.0 + b.1) / 2.0)
    }
    /// F: the chosen part fills the view.
    fn frame(&mut self, app: &App) {
        if let Some((lo, hi)) = self.part_box(self.part) {
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
            if look::tip(b.button(RichText::new("Frame").small()), "Fill the view with the chosen part", "F").clicked() {
                self.frame(app);
            }
        }
    }

    /// The bar in the view's top left, always: the tool and what the buttons do, and how the view moves.
    fn view_bar(&self, ui: &Ui, view: Rect) {
        let name = TOOLS.iter().find(|t| t.0 == self.tool).map_or("", |t| t.2);
        let tool = if self.erase && self.tool == Tool::Paint { "both buttons erase".to_string() } else { buttons(self.tool).to_string() };
        let mut lines = vec![format!("{name}: {tool}."), "Right drag turns, middle drag pans, wheel zooms.".to_string()];
        if self.flat {
            lines.push("Flat view.".into());
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
        let Some(d) = self.doc() else { return };
        let (m, paint) = (d.m.clone(), d.paint.clone());
        if self.drag.is_some() && !resp.ctx.input(|i| i.pointer.any_down()) && !resp.drag_stopped() {
            (self.drag, self.merging) = (None, None);
        }
        if m.parts.is_empty() {
            self.hover = None;
            return;
        }
        let poses = self.poses();
        let part = self.part.min(m.parts.len() - 1);
        let pos = resp.hover_pos().or(resp.interact_pointer_pos());
        let ray = pos.and_then(|p| app.view.ray(p));
        self.hover = ray.and_then(|(o, d)| voxel::pick_paint(&m, &poses, &paint, o, d));
        // the status line's middle: the part and the painting's pixel under the pointer, and its colour
        app.pointer = match self.hover {
            Some(h) => {
                let c = paint.px[h.y * paint.w + h.x];
                let colour = if c == art::EMPTY { "empty".to_string() } else { app.pal.colours.iter().find(|p| p.rgb == c).map_or(format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]), |p| p.name.clone()) };
                format!("part {}, painting x {} y {}: {colour}", m.parts[h.part].name, h.x, h.y)
            }
            None => String::new(),
        };
        let screen = |p: Vec3| app.view.to_screen(p);
        let pose = poses[part];
        let pivot_at = screen(pose.transform_point3(Vec3::from(m.parts[part].pivot)));
        if let Some(q) = pivot_at {
            crate::drive::mark(&resp.ctx, "pivot", Rect::from_center_size(q, Vec2::splat(2.0)));
        }
        let point_at: Vec<Option<Pos2>> = m.points.iter().map(|pt| m.parts.iter().position(|p| p.name == pt.part).and_then(|i| screen(poses[i].transform_point3(Vec3::from(pt.at))))).collect();
        let alt = resp.ctx.input(|i| i.modifiers.alt);
        // Paint: a left press paints, and a drag goes on painting (one step); right clicks paint the second colour
        if self.tool == Tool::Paint && !alt {
            let held = resp.ctx.input(|i| i.pointer.primary_down()) && (resp.is_pointer_button_down_on() || resp.dragged_by(egui::PointerButton::Primary));
            if let Some(h) = self.hover.filter(|_| held) {
                self.dab(h.x, h.y, true);
            } else if let Some(h) = self.hover.filter(|_| resp.secondary_clicked()) {
                self.stroking = false;
                self.dab(h.x, h.y, false);
            }
            app.view.left_turns = false;
            return;
        }
        // a press of the pivot's or a point's cross starts a drag of it; Alt + left drag turns the view instead
        if resp.drag_started_by(egui::PointerButton::Primary) && !alt && self.drag.is_none() {
            let from = resp.interact_pointer_pos().unwrap_or_default();
            let near = |p: Option<Pos2>| p.is_some_and(|p| p.distance(from) < 12.0);
            let what = match self.tool {
                Tool::Pivot if near(pivot_at) => Some(Grab::Pivot),
                Tool::Point => point_at.iter().position(|p| near(*p)).map(Grab::Point),
                _ => None,
            };
            self.drag = what.map(|what| Drag { what });
        }
        app.view.left_turns = false;
        if let (Some(dr), Some((o, d))) = (self.drag, ray) {
            // in the plane through it facing the eye, to half voxels
            let at = match dr.what {
                Grab::Point(k) => {
                    let i = m.parts.iter().position(|p| p.name == m.points[k].part).unwrap_or(0);
                    (poses[i], Vec3::from(m.points[k].at))
                }
                Grab::Pivot => (pose, Vec3::from(m.parts[part].pivot)),
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
                Grab::Pivot => self.edit_merged(app, "Move the pivot", |m, _| {
                    m.parts[part].pivot = snap;
                    true
                }),
            }
            if resp.drag_stopped() {
                (self.drag, self.merging) = (None, None);
            }
            return;
        }
        let (left, right) = (resp.clicked() && !alt, resp.secondary_clicked());
        let Some(h) = self.hover.filter(|_| left || right) else { return };
        match self.tool {
            Tool::Pick => {
                let c = paint.px[h.y * paint.w + h.x];
                if c != art::EMPTY {
                    *(if left { &mut self.first } else { &mut self.second }) = c;
                }
            }
            Tool::Point if left => {
                // on the face clicked, in its part's voxels, to half voxels
                let local = poses[h.part].inverse().transform_point3(h.at);
                let at = ((local * 2.0).round() / 2.0).to_array();
                let mut made = 0;
                self.edit(app, "Add a point", |m, _| {
                    made = voxel::add_point(m, h.part, at);
                    true
                });
                self.point = Some(made);
            }
            _ => {}
        }
    }

    /// The view's guides: the floor's grid, the chosen part's boxes, the pixel under the pointer, the pivot and
    /// points, the player.
    fn overlay(&self, p: &egui::Painter, view: &Viewport) {
        let Some(m) = self.m() else { return };
        let poses = self.poses();
        let line = |a: Vec3, b: Vec3, s: Stroke| {
            if let (Some(a), Some(b)) = (view.to_screen(a), view.to_screen(b)) {
                p.line_segment([a, b], s);
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
        // the chosen part's boxes, outlined
        let s = Stroke::new(1.0, look::ACCENT.gamma_multiply(0.6));
        for f in m.box_faces().into_iter().filter(|f| f.part == self.part) {
            let q = [f.o, f.o + f.eu, f.o + f.eu + f.ev, f.o + f.ev].map(|v| pose.transform_point3(v));
            for i in 0..4 {
                line(q[i], q[(i + 1) % 4], s);
            }
        }
        // the pixel under the pointer, outlined in the colour it would take
        if let Some(h) = self.hover.filter(|_| self.drag.is_none()) {
            if let Some(f) = m.box_faces().get(h.face).filter(|f| h.x as u32 >= f.rect[0] && h.y as u32 >= f.rect[1]) {
                let (u, v) = ((h.x as u32 - f.rect[0]) as f32, (h.y as u32 - f.rect[1]) as f32);
                let (eu, ev) = (f.eu / f.rect[2] as f32, f.ev / f.rect[3] as f32);
                let o = f.o + eu * u + ev * v;
                let q = [o, o + eu, o + eu + ev, o + ev].map(|c| poses[f.part].transform_point3(c));
                let col = match self.tool {
                    Tool::Paint if self.erase => Color32::from_rgb(235, 60, 50),
                    Tool::Paint => c32(self.first),
                    _ => Color32::WHITE,
                };
                for i in 0..4 {
                    line(q[i], q[(i + 1) % 4], Stroke::new(2.0, col));
                }
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

    /// The 2D preview until the game's renderer is up: the boxes' faces, each in its painting's middle colour,
    /// flat shaded, far ones first.
    fn sketch(&self, p: &egui::Painter, view: &Viewport) {
        let Some(d) = self.doc() else { return };
        let poses = self.poses();
        let eye = view.orbit.eye();
        let mut faces = vec![];
        for f in d.m.box_faces() {
            let [x, y, w, h] = f.rect.map(|v| v as usize);
            let Some(&rgb) = d.paint.px.get((y + h / 2) * d.paint.w + x + w / 2).filter(|c| x + w / 2 < d.paint.w && **c != art::EMPTY) else { continue };
            let corners = [f.o, f.o + f.eu, f.o + f.eu + f.ev, f.o + f.ev].map(|c| poses[f.part].transform_point3(c));
            let n = (corners[1] - corners[0]).cross(corners[3] - corners[0]).normalize_or_zero();
            let shade = 0.6 + 0.4 * n.y.abs().max(n.z.abs() * 0.8);
            faces.push((((corners[0] + corners[2]) / 2.0).distance(eye), corners, rgb.map(|c| (c as f32 * shade) as u8)));
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
        // its painted boxes (ADR 0081, 0116): corner, size and turn in pixels (4 a voxel), where the net lies
        let (mut change, mut remove) = (None, None);
        for (j, b) in part.boxes.iter().enumerate() {
            let mut nb = b.clone();
            egui::Grid::new(("part box", j)).num_columns(4).spacing(Vec2::new(4.0, 2.0)).show(ui, |ui| {
                look::label(ui, &format!("box {} at", j + 1));
                for v in &mut nb.at {
                    look::tip(ui.add(egui::DragValue::new(v).speed(0.25)), "Its corner in the part, pixels (4 a voxel)", "");
                }
                ui.end_row();
                look::label(ui, "size");
                for v in &mut nb.size {
                    look::tip(ui.add(egui::DragValue::new(v).range(0..=voxel::BOX_SIDE)), "Pixels a side, at most 63; 0 makes it a plane", "");
                }
                ui.end_row();
                look::label(ui, "turn");
                for v in &mut nb.turn {
                    ui.add(egui::DragValue::new(v).speed(1.0).suffix("°"));
                }
                ui.end_row();
                look::label(ui, "net at");
                for v in &mut nb.uv {
                    look::tip(ui.add(egui::DragValue::new(v).range(0..=engine::models::MAX_PAINT)), "Where its net starts in the painting", "");
                }
                if look::tip(ui.small_button("Remove"), "Remove this box (its pixels stay in the painting)", "").clicked() {
                    remove = Some(j);
                }
                ui.end_row();
            });
            if nb != *b {
                change = Some((j, nb));
            }
        }
        if let Some((j, nb)) = change {
            self.edit_merged(app, "Change a box", |m, _| {
                m.parts[i].boxes[j] = nb;
                true
            });
        }
        if let Some(j) = remove {
            self.edit(app, "Remove a box", |m, _| {
                m.parts[i].boxes.remove(j);
                true
            });
        }
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
            if look::tip(ui.button("Add a box"), "A painted box of 8 px a side on this part, its net added below the painting in the first colour", "").clicked() {
                self.run(app, "box.add");
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

impl Page for Models {
    fn name(&self) -> &'static str {
        "Models"
    }

    fn files(&self, app: &App) -> Vec<FileRow> {
        self.docs
            .iter()
            .map(|(n, d)| {
                let path = format!("{MODELS}/{n}.toml");
                FileRow { id: n.clone(), label: n.clone(), thumb: Thumb::Img(voxel::thumb(&d.m, (d.paint.w > 0).then_some(&d.paint))), flags: Flags { made: d.saved.is_some(), dirty: d.dirty(), draft: app.is_draft(&path), problem: app.has_problem(&path) } }
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
        v.push(Cmd::new("erase", "Eraser on or off", "the Paint tool empties pixels instead").key(Modifiers::NONE, Key::E));
        v.push(Cmd::new("undo", "Undo", "undo the last change to this model").key(Modifiers::COMMAND, Key::Z));
        v.push(Cmd::new("redo", "Redo", "redo what was undone").key(Modifiers::COMMAND, Key::Y));
        v.push(Cmd::new("save", "Save the model", "write it (checked by the game's reader first) and take it off the drafts list").key(Modifiers::COMMAND, Key::S));
        v.push(Cmd::new("new", "New model", "choose a shape to start from: a block, a slab, a post, a tool or a creature").key(Modifiers::COMMAND, Key::N));
        for (id, label, help) in voxel::STARTERS {
            v.push(Cmd::new(format!("new:{id}"), format!("New model: {label}"), help));
        }
        v.push(Cmd::new("box.add", "Add a box", "a painted box of 8 px a side on the open part, its net below the painting"));
        v.push(Cmd::new("play", "Play or pause", "the clip in the view").key(Modifiers::NONE, Key::Space));
        v.push(Cmd::new("clip.new", "New clip", "a new clip of one second, looping"));
        v.push(Cmd::new("part.add", "Add a part", "a new empty part, a root"));
        v.push(Cmd::new("part.child", "Add a child part", "a new empty part hanging on the chosen one"));
        v.push(Cmd::new("motion", "Ready-made motion", "fill the clip's keys of the chosen part with the motion set in the inspector"));
        v.push(Cmd::new("cancel", "Cancel", "back to the open model from the new model's shapes").key(Modifiers::NONE, Key::Escape));
        // (the first key that matches runs, and a plain key matches with Shift held too: the Shift views first)
        for (k, (id, name, ..)) in VIEWS.iter().enumerate().rev() {
            let key = [Key::Num1, Key::Num3, Key::Num7][k / 2];
            let mods = if k % 2 == 0 { Modifiers::NONE } else { Modifiers::SHIFT };
            v.push(Cmd::new(format!("view:{id}"), format!("View from the {}", name.to_lowercase()), "look at the model from that side (the view cube's face)").key(mods, key));
        }
        v.push(Cmd::new("view.flat", "Flat view on or off", "no perspective, so rows and sizes line up").key(Modifiers::NONE, Key::Num5));
        v.push(Cmd::new("view.frame", "Frame", "fill the view with the chosen part").key(Modifiers::NONE, Key::F));
        v
    }

    fn run(&mut self, app: &mut App, id: &str) {
        if let Some(t) = id.strip_prefix("tool.").and_then(|t| TOOLS.iter().find(|x| x.1 == t)) {
            self.tool = t.0;
            return;
        }
        // with an argument (the command box and shot=): part:<name>, clip:<name>, time:<s>, motion:<kind>,
        // new:<shape>, scene:<grass|cave|none>, hour:<h>, cam:yaw;pitch;dist, show:<block|player|grid>
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
                "motion" => {
                    if let Some(k) = MOTIONS.iter().find(|x| format!("{:?}", x.0).eq_ignore_ascii_case(arg)) {
                        self.motion = (k.0, self.motion.1, k.3);
                        self.run(app, "motion");
                    }
                }
                "new" => self.new_model(app, arg, arg),
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
            "erase" => self.erase = !self.erase,
            "play" => self.playing = !self.playing && self.clip.is_some(),
            "save" => self.save(app),
            "box.add" => {
                let (part, c) = (self.part, self.first);
                self.box_edit(app, "Add a box", |m, p| voxel::add_box(m, p, part, [8, 8, 8], c));
            }
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
            "cancel" => self.choosing = false,
            "view.flat" => self.flat = !self.flat,
            "view.frame" => self.frame(app),
            _ => {}
        }
    }

    fn more_keys(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("Right drag", "turn the view about the model's middle"),
            ("Alt + left drag", "turn the view, for a hand without a middle or right button"),
            ("Middle drag", "pan the view"),
            ("Wheel", "zoom toward the pointer"),
            ("Left click or drag", "the tool's first action (paint, pick, add a point)"),
            ("Right click", "the tool's second action (the second colour)"),
        ]
    }

    fn work(&mut self, ui: &mut Ui, app: &mut App) {
        if !ui.input(|i| i.pointer.any_down()) && self.drag.is_none() {
            self.merging = None;
            self.stroking = false;
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
        app.view.model = Some(ModelCam { flat: self.flat, pivot: self.turn_point(), right_turns: true });
        // the view on the left, the model's painting on the right: paint on either
        let (full, top) = (ui.available_width(), ui.cursor().min);
        let paint_w = if self.doc().is_some_and(|d| d.paint.w > 0) { (full * 0.5).floor() } else { 0.0 };
        let resp = {
            let me = &*self;
            ui.allocate_ui(Vec2::new(full - paint_w, view_h), |ui| app.view.ui(ui, view_h, |p, v, _| me.sketch(p, v))).inner
        };
        if paint_w > 0.0 {
            self.painting(ui, Rect::from_min_size(Pos2::new(top.x + full - paint_w + GAP, top.y), Vec2::new(paint_w - GAP, view_h)));
        }
        self.orbit = app.view.orbit;
        self.pointer(app, &resp);
        let painter = ui.painter_at(resp.rect);
        self.overlay(&painter, &app.view);
        self.view_cube(ui, app, resp.rect);
        self.view_bar(ui, resp.rect);
        let tool = TOOLS.iter().find(|t| t.0 == self.tool).unwrap();
        look::tip(resp, &format!("{}: {}", tool.2, tool.4), tool.3.name());
        look::split_bar(ui, "view", view_share, total);
        self.timeline(ui, app);
    }

    fn inspector(&mut self, ui: &mut Ui, app: &mut App) {
        if let Some(mut sec) = look::section(ui, "Paint") {
            let ui = &mut *sec;
            let w = ((ui.available_width() - 12.0) / 4.0).floor() - 1.0;
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(4.0);
                for (t, _, name, k, help) in TOOLS {
                    if look::tip(look::tool(ui, self.tool == t, name, k.name(), w), help, k.name()).clicked() {
                        self.tool = t;
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                look::tip(look::toggle(ui, &mut self.erase, "Eraser E"), "Paint empties pixels instead: a see-through hole in the box", "E");
                look::weak(ui, "Ctrl+Z undoes a stroke");
            });
            // (the palette scrolls in a box of its own, so the states below it stay in reach)
            egui::ScrollArea::vertical().id_salt("models palette").max_height(240.0).show(ui, |ui| {
                palette_picker(ui, "models", &app.pal, &mut self.first, &mut self.second);
            });
        }
        if let Some(mut sec) = look::section(ui, "States") {
            let ui = &mut *sec;
            self.states_inspector(ui, app);
        }
        if let Some(mut sec) = look::section(ui, "Part") {
            let ui = &mut *sec;
            self.part_inspector(ui, app);
        }
        if let Some(mut sec) = look::section(ui, "Point") {
            let ui = &mut *sec;
            self.point_inspector(ui, app);
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
        if let Some(mut sec) = look::section(ui, "Model") {
            let ui = &mut *sec;
            if look::tip(ui.button("New model"), "Start a new figure of painted boxes from a shape: a block, a slab, a post, a tool or a creature", "Ctrl+N").clicked() {
                self.run(app, "new");
            }
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
                look::tip(look::toggle(ui, &mut self.show_grid, "Grid"), "The floor's 1 m lines", "");
            });
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
        docs.insert("post".into(), MDoc { m, life: Life::default(), hist: History::default(), saved: None, rev: 0, old: String::new(), paint: NO_PAINT, paint_saved: None, paint_refused: None });
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
        d.hist.push((d.m.clone(), d.life.clone(), d.paint.clone()), "Reload from disk");
        (d.m, d.life, d.saved, d.paint, d.paint_saved, d.paint_refused) = (fresh.m, fresh.life, fresh.saved, fresh.paint, fresh.paint_saved, fresh.paint_refused);
        d.rev += 1;
        if self.open.as_deref() == Some(name) {
            self.opened_keep();
        }
    }
}
