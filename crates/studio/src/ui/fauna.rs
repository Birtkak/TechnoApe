//! The Fauna page (docs/design/studio-tools.md "Fauna", ADR 0053): the rows of `data/creatures.toml`. The game's
//! renderer shows the creature walking in place with the walk its row makes (`engine::creatures::walk`, sent as the
//! model's clip the way the Models page sends an unsaved model); a speed slider runs it from standing through
//! walking to running. The inspector holds the row: its model, plan and part roles as pick lists, its numbers as
//! sliders in groups, its sounds, and a wall of six gaits. Save rewrites only that row of the file.
use super::look::{self, GAP};
use super::viewport::DIR;
use super::wall as wallui;
use super::{App, Cmd, FileRow, Page, Thumb, DRAFTS};
use crate::art::{self, History};
use crate::shell::Flags;
use crate::wall::{Rng, Wall};
use eframe::egui::{self, Color32, Key, Modifiers, Pos2, Rect, RichText, Stroke, Ui};
use engine::creatures::{self, Creature, Plan, RANGES};
use engine::models::Model;
use glam::Vec3;
use std::collections::BTreeMap;

pub const FILE: &str = "data/creatures.toml";
const MODELS: &str = "data/models";
/// The view's generations of the models folder start here, apart from the Models page's.
const GEN0: u64 = 1 << 40;

struct CDoc {
    c: Creature,
    hist: History<Creature>,
    /// The row as it is in the file; none for one not saved yet.
    saved: Option<Creature>,
}

pub struct Fauna {
    docs: BTreeMap<String, CDoc>,
    open: Option<String>,
    /// Why the file was refused, when it was.
    refused: Option<String>,
    models: BTreeMap<String, Model>,
    sounds: Vec<String>,
    /// m/s; 0 stands (the idle).
    speed: f32,
    merging: Option<String>,
    wall: Option<Wall<Creature>>,
    walls: u64,
    gen: u64,
    written: Option<(Creature, u32)>,
    aimed: Option<String>,
    start: std::time::Instant,
}

/// A candidate of the wall: the gait's numbers moved by `spread`.
fn vary(c: &Creature, rng: &mut Rng, spread: f32) -> Creature {
    let mut v = c.clone();
    for (n, lo, hi, _, group) in RANGES {
        if group == "Gait" || n == "step" {
            let x = (v.get(n) + rng.signed() * spread * 0.5 * (hi - lo)).clamp(lo, hi);
            v.set(n, (x * 100.0).round() / 100.0);
        }
    }
    v
}

/// A side view of the model in its walk at the moment the legs reach furthest: each part's middle slice.
fn sketch(p: &egui::Painter, r: Rect, m: &Model, c: &Creature) {
    let mut m = m.clone();
    let w = creatures::walk(&m, c, c.walk_speed);
    let t = w.length * 0.25;
    m.clips = vec![w];
    let poses = m.pose(Some("walk"), t);
    let mut quads = Vec::new();
    for (k, part) in m.parts.iter().enumerate() {
        let [_, sy, sz] = part.size.map(|v| v as f32);
        let x = part.size[0] as f32 / 2.0;
        let pts: Vec<Vec3> = [[0.0, 0.0], [0.0, sy], [sz, sy], [sz, 0.0]].iter().map(|[z, y]| poses[k].transform_point3(Vec3::new(x, *y, *z))).collect();
        let colour = part.cells.iter().find(|v| **v != 0).map_or(Color32::GRAY, |v| {
            let [r, g, b] = m.rgb[*v as usize - 1];
            Color32::from_rgb(r, g, b)
        });
        quads.push((pts, colour));
    }
    let all: Vec<&Vec3> = quads.iter().flat_map(|q| q.0.iter()).collect();
    let (z0, z1) = all.iter().fold((f32::MAX, f32::MIN), |a, v| (a.0.min(v.z), a.1.max(v.z)));
    let (y0, y1) = all.iter().fold((f32::MAX, f32::MIN), |a, v| (a.0.min(v.y), a.1.max(v.y)));
    let k = (r.width() / (z1 - z0).max(0.1)).min(r.height() / (y1 - y0).max(0.1)) * 0.9;
    let o = Pos2::new(r.center().x - k * (z0 + z1) / 2.0, r.center().y + k * (y0 + y1) / 2.0);
    for (pts, colour) in quads {
        let pts: Vec<Pos2> = pts.iter().map(|v| Pos2::new(o.x + v.z * k, o.y - v.y * k)).collect();
        p.add(egui::Shape::convex_polygon(pts, colour, Stroke::new(1.0, Color32::from_black_alpha(90))));
    }
    p.line_segment([Pos2::new(r.left(), o.y - y0 * k), Pos2::new(r.right(), o.y - y0 * k)], Stroke::new(1.0, Color32::from_gray(90)));
}

impl Fauna {
    pub fn new(app: &App) -> Fauna {
        let mut f = Fauna { docs: BTreeMap::new(), open: None, refused: None, models: BTreeMap::new(), sounds: Vec::new(), speed: 1.0, merging: None, wall: None, walls: 0, gen: GEN0, written: None, aimed: None, start: std::time::Instant::now() };
        f.read_disk(app);
        f.open = f.docs.keys().find(|n| n.as_str() == "boar").or(f.docs.keys().next()).cloned();
        f.speed = f.doc().map_or(1.0, |d| d.c.walk_speed);
        f
    }

    fn read_disk(&mut self, app: &App) {
        self.models = std::fs::read_dir(MODELS)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let n = e.file_name().to_string_lossy().strip_suffix(".toml")?.to_string();
                let t = std::fs::read_to_string(e.path()).ok()?;
                Some((n.clone(), Model::parse(&n, &t, &app.pal).ok()?))
            })
            .collect();
        let sounds = std::fs::read_to_string(super::sounds::SOUNDS).unwrap_or_default();
        self.sounds = crate::rows::spans(&sounds).into_iter().filter(|s| s.header == "sound").map(|s| s.name).collect();
        let text = std::fs::read_to_string(FILE).unwrap_or_default();
        match self.parse(&text) {
            Ok(rows) => {
                self.refused = None;
                for c in rows {
                    match self.docs.get_mut(&c.name) {
                        Some(d) if d.saved.as_ref() != Some(&d.c) => d.saved = Some(c),
                        Some(d) => {
                            if d.c != c {
                                d.hist.push(std::mem::replace(&mut d.c, c.clone()), "Changed on disk");
                            }
                            d.saved = Some(c);
                        }
                        None => {
                            self.docs.insert(c.name.clone(), CDoc { c: c.clone(), hist: History::default(), saved: Some(c) });
                        }
                    }
                }
            }
            Err(e) => self.refused = Some(e),
        }
    }

    fn parse(&self, text: &str) -> Result<Vec<Creature>, String> {
        creatures::parse(FILE, &text.replace("\r\n", "\n"), &|n| self.models.get(n).cloned(), &|s| self.sounds.iter().any(|k| k == s))
    }

    fn doc(&self) -> Option<&CDoc> {
        self.open.as_ref().and_then(|n| self.docs.get(n))
    }

    /// An edit of the open row, one undo step (a slider's moves merged into one).
    fn edit(&mut self, name: &str, f: impl FnOnce(&mut Creature)) {
        let merge = self.merging.as_deref() == Some(name);
        let Some(d) = self.open.as_ref().and_then(|n| self.docs.get_mut(n)) else { return };
        let before = d.c.clone();
        f(&mut d.c);
        if d.c != before && !merge {
            d.hist.push(before, name);
        }
        self.merging = Some(name.into());
    }

    fn save(&mut self, app: &mut App) {
        let Some(name) = self.open.clone() else { return };
        let c = self.docs[&name].c.clone();
        let row = creatures::write_row(&c);
        let check = |t: &str| self.parse(t).map(|_| ());
        match crate::rows::save(FILE, "creature", &name, &row, check) {
            Ok(_) => {
                self.docs.get_mut(&name).unwrap().saved = Some(c);
                let drafts = art::set_draft(&std::fs::read_to_string(DRAFTS).unwrap_or_default(), &format!("{FILE}#{name}"), false);
                app.write_drafts(drafts);
                app.note(FILE);
                app.toast(format!("Saved {name} in {FILE}"));
                app.check();
            }
            Err(e) => app.refuse(format!("Not saved: {e}")),
        }
    }

    /// The open creature's model with its walk (or idle) at the slider's speed, written where the view reads.
    fn feed_view(&mut self, app: &mut App) -> Option<String> {
        let d = self.doc()?;
        let m = self.models.get(&d.c.model)?;
        let c = d.c.clone();
        let (text, clip) = shown(m, &c, self.speed);
        let now = Some((c.clone(), self.speed.to_bits()));
        if self.written != now {
            let dir = format!("{DIR}/models");
            let _ = std::fs::create_dir_all(&dir);
            if std::fs::write(format!("{dir}/creature.toml"), text).is_ok() {
                self.gen += 1;
                self.written = now;
            }
        }
        app.view.models(self.gen);
        let t = self.start.elapsed().as_secs_f32();
        app.view.show_model_at(Some("creature"), Some(&clip.name), t, c.scale);
        app.view.show_state(None);
        app.view.live = true;
        Some(format!("{}: one cycle {:.2} s", clip.name, clip.length))
    }

    fn aim(&self, app: &mut App) {
        app.view.orbit.target = Vec3::new(0.5, 0.4, 0.5);
        app.view.orbit.dist = 3.0;
        app.view.orbit.pitch = 0.2;
    }

    /// A pick list of the model's parts ("none" first) for one role; true when it changed.
    fn part_pick(ui: &mut Ui, id: &str, label: &str, parts: &[String], cur: &mut String) -> bool {
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label(RichText::new(label).small());
            let shown = if cur.is_empty() { "none".to_string() } else { cur.clone() };
            egui::ComboBox::from_id_salt(id).selected_text(shown).width((ui.available_width() - 8.0).max(60.0)).show_ui(ui, |ui| {
                for p in std::iter::once(String::new()).chain(parts.iter().cloned()) {
                    let text = if p.is_empty() { "none" } else { p.as_str() };
                    if ui.selectable_label(*cur == p, text).clicked() && *cur != p {
                        *cur = p;
                        changed = true;
                    }
                }
            });
        });
        changed
    }
}

/// The names of a plan's leg slots.
/// The model file the view reads for a creature at `speed`: its model with the walk (or idle) clip, and that clip.
fn shown(m: &Model, c: &Creature, speed: f32) -> (String, engine::models::Clip) {
    let clip = creatures::walk(m, c, speed);
    let mut shown = m.clone();
    shown.clips.retain(|k| k.name != clip.name);
    shown.clips.push(clip.clone());
    // its footfalls as the clip's events, so they sound in the view as in the game (`creature=`)
    let life = engine::models::Life { events: vec![(clip.name.clone(), creatures::steps(c, speed))], ..Default::default() };
    (engine::models::write_life(&shown, &life), clip)
}

fn leg_names(plan: Plan) -> &'static [&'static str] {
    match plan {
        Plan::Quadruped => &["front left leg", "front right leg", "back left leg", "back right leg"],
        Plan::Fish => &[],
        _ => &["left leg", "right leg"],
    }
}

impl Page for Fauna {
    fn name(&self) -> &'static str {
        "Fauna"
    }

    fn files(&self, app: &App) -> Vec<FileRow> {
        let mut v: Vec<FileRow> = self
            .docs
            .iter()
            .map(|(n, d)| {
                let main = self.models.get(&d.c.model).and_then(|m| m.rgb.first()).map_or(Color32::GRAY, |[r, g, b]| Color32::from_rgb(*r, *g, *b));
                let path = format!("{FILE}#{n}");
                let dirty = d.saved.as_ref() != Some(&d.c);
                FileRow { id: n.clone(), label: format!("{n} ({})", d.c.plan.name()), thumb: Thumb::Flat(main), flags: Flags { made: d.saved.is_some(), draft: app.is_draft(&path), dirty, problem: app.has_problem(FILE) } }
            })
            .collect();
        if self.refused.is_some() {
            v.push(FileRow { id: "(refused)".into(), label: "creatures.toml (refused)".into(), thumb: Thumb::Flat(look::BAD), flags: Flags { made: true, draft: false, dirty: false, problem: true } });
        }
        v
    }

    fn current(&self) -> Option<String> {
        self.open.clone()
    }

    fn open(&mut self, app: &mut App, id: &str) {
        if self.docs.contains_key(id) {
            self.open = Some(id.into());
            self.wall = None;
            self.speed = self.docs[id].c.walk_speed;
        } else if let Some(e) = &self.refused {
            app.refuse(e.clone());
        }
    }

    fn commands(&self, _app: &App) -> Vec<Cmd> {
        vec![
            Cmd::new("save", "Save the creature", "rewrite its row of data/creatures.toml (checked by the game's reader) and take it off the drafts list").key(Modifiers::COMMAND, Key::S),
            Cmd::new("undo", "Undo", "undo the last change to this creature").key(Modifiers::COMMAND, Key::Z),
            Cmd::new("redo", "Redo", "redo what was undone").key(Modifiers::COMMAND, Key::Y),
            Cmd::new("stand", "Stand", "the speed to 0: the idle").key(Modifiers::NONE, Key::Num0),
            Cmd::new("walk", "Walk", "the speed to its walking speed").key(Modifiers::NONE, Key::Num1),
            Cmd::new("run", "Run", "the speed to its running speed").key(Modifiers::NONE, Key::Num2),
            Cmd::new("vary", "Variations", "a wall of six gaits of this creature").key(Modifiers::NONE, Key::V),
        ]
    }

    fn run(&mut self, app: &mut App, id: &str) {
        // with an argument (the command box and shot=): speed:<m/s>, take:<wall item>, hour:<h>
        if let Some((cmd, arg)) = id.split_once(':') {
            match cmd {
                "speed" => self.speed = arg.parse().unwrap_or(self.speed),
                "take" => {
                    if let Some(c) = self.wall.as_ref().and_then(|w| w.items.get(arg.parse::<usize>().unwrap_or(99)).cloned()) {
                        self.merging = None;
                        self.edit("Take a variation", |x| *x = c);
                    }
                }
                "hour" => app.view.hour = arg.parse().unwrap_or(app.view.hour),
                _ => {}
            }
            return;
        }
        let row = self.doc().map(|d| d.c.clone());
        match id {
            "save" => self.save(app),
            "undo" | "redo" => {
                if let Some(d) = self.open.as_ref().and_then(|n| self.docs.get_mut(n)) {
                    let _ = if id == "undo" { d.hist.undo(&mut d.c) } else { d.hist.redo(&mut d.c) };
                }
            }
            "stand" => self.speed = 0.0,
            "walk" => self.speed = row.map_or(1.0, |c| c.walk_speed),
            "run" => self.speed = row.map_or(3.0, |c| c.run_speed),
            "vary" => {
                if let Some(c) = row {
                    self.walls += 1;
                    self.wall = Some(Wall::new(c, 6, 0.4, 0xfa + self.walls, &vary));
                }
            }
            _ => {}
        }
    }

    fn more_keys(&self) -> Vec<(&'static str, &'static str)> {
        vec![("Right drag", "turn the view"), ("Wheel", "zoom the view")]
    }

    fn work(&mut self, ui: &mut Ui, app: &mut App) {
        if !ui.input(|i| i.pointer.any_down()) {
            self.merging = None;
        }
        app.view.controls(ui);
        let Some(name) = self.open.clone() else {
            look::weak(ui, "No creature in data/creatures.toml");
            return;
        };
        if self.aimed != self.open {
            self.aim(app);
            self.aimed = self.open.clone();
        }
        app.view.place = String::new();
        let info = self.feed_view(app);
        let c = self.docs[&name].c.clone();
        ui.horizontal(|ui| {
            ui.label(RichText::new(&name).strong());
            look::weak(ui, format!("{} on model {}; {}; walking in place", c.plan.name(), c.model, info.unwrap_or_else(|| "no model to show".into())));
        });
        ui.horizontal(|ui| {
            ui.label(RichText::new("Speed").small());
            ui.spacing_mut().slider_width = (ui.available_width() - 260.0).clamp(80.0, 500.0);
            let r = ui.add(egui::Slider::new(&mut self.speed, 0.0..=c.run_speed).fixed_decimals(2).suffix(" m/s"));
            look::tip(r, "From standing (0, the idle) through walking to running; the cycle follows so the feet keep pace", "0 1 2");
            let what = if self.speed <= 0.0 {
                "stand"
            } else if self.speed < (c.walk_speed + c.run_speed) / 2.0 {
                "walk"
            } else {
                "run"
            };
            look::weak(ui, what);
        });
        let total = ui.available_height();
        let m = self.models.get(&c.model).cloned();
        app.view.left_turns = true;
        app.view.ui(ui, total, |p, _, r| {
            if let Some(m) = &m {
                sketch(p, r.shrink(GAP), m, &c);
            }
        });
    }

    fn inspector(&mut self, ui: &mut Ui, app: &mut App) {
        let cmds = self.commands(app);
        let button = |ui: &mut Ui, me: &mut Self, app: &mut App, id: &str, label: &str| {
            let c = cmds.iter().find(|c| c.id == id).unwrap();
            if look::tip(ui.button(label), &c.help, &c.keys()).clicked() {
                me.run(app, id);
            }
        };
        look::section(ui, "Creature");
        ui.horizontal_wrapped(|ui| {
            for (id, label) in [("undo", "Undo"), ("redo", "Redo"), ("save", "Save")] {
                button(ui, self, app, id, label);
            }
        });
        let Some(d) = self.doc() else { return };
        let c = d.c.clone();
        let mut n = c.clone();
        let parts: Vec<String> = self.models.get(&c.model).map(|m| m.parts.iter().map(|p| p.name.clone()).collect()).unwrap_or_default();
        if let Err(e) = c.check(self.models.get(&c.model), &|s| self.sounds.iter().any(|k| k == s)) {
            ui.colored_label(look::BAD, RichText::new(e).small());
        }
        egui::ScrollArea::vertical().id_salt("fauna").max_height((ui.available_height() - if self.wall.is_some() { 330.0 } else { 4.0 }).max(120.0)).show(ui, |ui| {
            ui.separator();
            look::section(ui, "Body");
            ui.horizontal(|ui| {
                ui.label(RichText::new("model").small());
                egui::ComboBox::from_id_salt("model").selected_text(&n.model).width((ui.available_width() - 8.0).max(60.0)).show_ui(ui, |ui| {
                    for k in self.models.keys() {
                        if ui.selectable_label(*k == n.model, k).clicked() {
                            n.model = k.clone();
                        }
                    }
                });
            });
            ui.label(RichText::new("plan").small());
            ui.horizontal_wrapped(|ui| {
                for (p, label) in Plan::ALL {
                    if look::tip(look::chip(ui, n.plan == p, label), "The body plan: how the legs move against each other", "").clicked() {
                        n.plan = p;
                    }
                }
            });
            if n.plan != c.plan {
                n.legs.resize(n.plan.legs(), String::new());
            }
            for (i, label) in leg_names(n.plan).iter().enumerate() {
                if let Some(slot) = n.legs.get_mut(i) {
                    Self::part_pick(ui, &format!("leg{i}"), label, &parts, slot);
                }
            }
            for (what, list) in [("arm", &mut n.arms), ("wing", &mut n.wings)] {
                let mut two = [list.first().cloned().unwrap_or_default(), list.get(1).cloned().unwrap_or_default()];
                let mut ch = false;
                for (i, side) in ["left", "right"].iter().enumerate() {
                    ch |= Self::part_pick(ui, &format!("{what}{i}"), &format!("{side} {what}"), &parts, &mut two[i]);
                }
                if ch {
                    *list = if two.iter().all(|s| s.is_empty()) { vec![] } else { two.to_vec() };
                }
            }
            Self::part_pick(ui, "head", "head", &parts, &mut n.head);
            Self::part_pick(ui, "tail", "tail", &parts, &mut n.tail);
            for group in ["Body", "Gait"] {
                if group == "Gait" {
                    ui.separator();
                    look::section(ui, "Gait");
                }
                for (k, lo, hi, unit, g) in RANGES {
                    if g != group {
                        continue;
                    }
                    ui.label(RichText::new(k.replace('_', " ")).small());
                    let mut v = n.get(k);
                    ui.spacing_mut().slider_width = (ui.available_width() - 110.0).max(40.0);
                    let r = ui.add(egui::Slider::new(&mut v, lo..=hi).fixed_decimals(2).suffix(format!(" {unit}")));
                    crate::drive::mark(ui.ctx(), format!("slider:{k}"), r.rect);
                    if r.changed() {
                        n.set(k, v);
                    }
                }
            }
            ui.separator();
            look::section(ui, "Sounds");
            for (label, field) in [("step", 0), ("voice", 1)] {
                let cur = if field == 0 { &mut n.step_sound } else { &mut n.voice };
                ui.horizontal(|ui| {
                    ui.label(RichText::new(label).small());
                    let shown = if cur.is_empty() { "none".to_string() } else { cur.clone() };
                    egui::ComboBox::from_id_salt(label).selected_text(shown).width((ui.available_width() - 8.0).max(60.0)).show_ui(ui, |ui| {
                        for s in std::iter::once(String::new()).chain(self.sounds.iter().cloned()) {
                            let text = if s.is_empty() { "none" } else { s.as_str() };
                            if ui.selectable_label(*cur == s, text).clicked() {
                                *cur = s;
                            }
                        }
                    });
                });
            }
            ui.separator();
            look::section(ui, "Variations");
            if self.wall.is_none() {
                let c = cmds.iter().find(|c| c.id == "vary").unwrap();
                if look::tip(ui.button("Six gaits"), &c.help, &c.keys()).clicked() {
                    self.wall = Some(Wall::new(n.clone(), 6, 0.4, 0xfa + self.walls, &vary));
                    self.walls += 1;
                }
            }
        });
        if n != c {
            self.edit("Row", |x| *x = n);
        }
        let Some(m) = self.models.get(&c.model).cloned() else { return };
        let chosen = self.wall.as_ref().and_then(|w| w.items.iter().position(|i| *i == c));
        let mut act = wallui::Act::None;
        if let Some(w) = &mut self.wall {
            let k = if ui.available_width() >= 360.0 { 3.0 } else { 2.0 };
            let cell = egui::Vec2::new(((ui.available_width() - (k - 1.0) * GAP) / k).floor().max(60.0), 56.0);
            act = wallui::show(ui, w, chosen, cell, |p, rect, t| sketch(p, rect, &m, t));
        }
        let gen: crate::wall::Gen<Creature> = &vary;
        match act {
            wallui::Act::Take(i) => self.run(app, &format!("take:{i}")),
            wallui::Act::More(i) => {
                if let Some(w) = &mut self.wall {
                    w.more_like(i, gen);
                }
            }
            wallui::Act::Back => {
                if let Some(w) = &mut self.wall {
                    w.back();
                }
            }
            wallui::Act::Shuffle => {
                if let Some(w) = &mut self.wall {
                    w.shuffle(gen);
                }
            }
            wallui::Act::Spread => {
                if let Some(w) = &mut self.wall {
                    w.fill(6, gen);
                }
            }
            _ => {}
        }
    }

    fn help(&self) -> String {
        "The creature walks in place with the walk its row makes; 0 stands, 1 walks, 2 runs; V a wall of six gaits; Ctrl+S saves its row".into()
    }

    fn history(&self, _app: &App) -> Option<(Vec<String>, Vec<String>)> {
        let d = self.doc()?;
        Some((d.hist.undo.iter().map(|s| s.name.clone()).collect(), d.hist.redo.iter().rev().map(|s| s.name.clone()).collect()))
    }

    fn back_to(&mut self, _app: &mut App, n: usize) {
        if let Some(d) = self.open.as_ref().and_then(|k| self.docs.get_mut(k)) {
            d.hist.back_to(&mut d.c, n);
        }
    }

    fn refresh(&mut self, app: &App) {
        self.read_disk(app);
    }

    fn unsaved(&self) -> Vec<String> {
        self.docs.iter().filter(|(_, d)| d.saved.as_ref() != Some(&d.c)).map(|(n, _)| format!("{FILE}#{n}")).collect()
    }

    fn save_all(&mut self, app: &mut App) {
        let keep = self.open.clone();
        for n in self.docs.iter().filter(|(_, d)| d.saved.as_ref() != Some(&d.c)).map(|(n, _)| n.clone()).collect::<Vec<_>>() {
            self.open = Some(n);
            self.save(app);
        }
        self.open = keep;
    }

    fn settle(&mut self, ctx: &egui::Context, app: &mut App) {
        app.view.settle(ctx, std::time::Duration::from_secs(90));
    }
}

#[cfg(test)]
mod tests {
    use engine::creatures;

    /// The walk the view is given carries the row's footfalls as its events, so its steps sound as in the game.
    #[test]
    fn the_shown_walk_steps_on_its_footfalls() {
        let data = std::path::Path::new("../../data");
        let pal = engine::palette::Palette::parse(&std::fs::read_to_string(data.join("palette.toml")).unwrap()).unwrap();
        let model = |n: &str| std::fs::read_to_string(data.join(format!("models/{n}.toml"))).ok().and_then(|t| engine::models::Model::parse(n, &t, &pal).ok());
        let rows = creatures::parse("c", &std::fs::read_to_string(data.join("creatures.toml")).unwrap().replace("\r\n", "\n"), &model, &|_| true).unwrap();
        let boar = rows.iter().find(|c| c.name == "boar").unwrap();
        let (text, clip) = super::shown(&model(&boar.model).unwrap(), boar, boar.walk_speed);
        let (_, life) = engine::models::parse_life("creature.toml", &text, &pal, &|_| true, &|_| true).unwrap();
        let want = creatures::steps(boar, boar.walk_speed);
        assert!(!want.is_empty());
        assert_eq!(life.events.iter().find(|(n, _)| *n == clip.name).map(|e| &e.1), Some(&want), "{text}");
    }

    #[test]
    fn the_shipped_file_checks_and_saving_an_unchanged_row_keeps_every_byte() {
        let data = std::path::Path::new("../../data");
        let pal = engine::palette::Palette::parse(&std::fs::read_to_string(data.join("palette.toml")).unwrap()).unwrap();
        assert_eq!(crate::art::check_creatures(data, &pal), None);
        let text = std::fs::read_to_string(data.join("creatures.toml")).unwrap();
        let model = |n: &str| std::fs::read_to_string(data.join(format!("models/{n}.toml"))).ok().and_then(|t| engine::models::Model::parse(n, &t, &pal).ok());
        for c in creatures::parse("c", &text.replace("\r\n", "\n"), &model, &|_| true).unwrap() {
            assert_eq!(crate::rows::put(&text, "creature", &c.name, &creatures::write_row(&c)), text, "{}", c.name);
        }
    }
}
