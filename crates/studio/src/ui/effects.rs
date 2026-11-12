//! The Effects page (docs/design/studio-tools.md "Effects", ADR 0050): a row or group of `data/effects.toml`
//! played by the game's renderer ([`Viewport`]) from the page's own copy of the file, every number a slider,
//! the gradient a bar of stops, the curves small graphs, and a wall of variations drawn as small moving
//! sketches. The editing is `effect`'s, pure and tested; this draws it.
use super::look::{self, c32, GAP};
use super::viewport::DIR;
use super::wall as wallui;
use super::{App, Cmd, FileRow, Page, Thumb, DRAFTS};
use crate::art::{self, History};
use crate::effect::{self, Form, CHOICES, CURVE_MAX, FIELDS, GROUPS, SHAPES};
use crate::rows::{self, Keys};
use crate::shell::Flags;
use crate::wall::{Rng, Wall};
use eframe::egui::{self, Color32, Key, Modifiers, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};
use glam::Vec3;
use std::collections::BTreeMap;

pub const EFFECTS: &str = "data/effects.toml";
/// The rows offered as starts for a new effect (ADR 0050's drafts).
const PRESETS: [&str; 8] = ["flames", "smoke", "sparks", "steam", "dust", "drip", "glint", "leaves"];

/// One row or group open in the Studio: its keys, its undo, its text as saved (none: not in the file yet), and
/// a group's rows switched off (not saved).
struct EDoc {
    k: Keys,
    hist: History<(Keys, Vec<String>)>,
    saved: Option<String>,
    off: Vec<String>,
}
impl EDoc {
    fn dirty(&self) -> bool {
        self.saved.as_deref() != Some(self.k.text().as_str())
    }
    fn group(&self) -> bool {
        self.k.header == "group"
    }
}

fn rgb_of(pal: &engine::palette::Palette, name: &str) -> [u8; 3] {
    pal.colours.iter().find(|c| c.name == name).map_or([255; 3], |c| c.rgb)
}

/// A row's colour at a share of its life: its gradient, else its colour, else the grey of debris.
fn colour_at(t: &toml::Table, pal: &engine::palette::Palette, x: f32) -> Color32 {
    let g = effect::gradient(t);
    let bright = client::fx::curve_at(&effect::curve(t, "bright_curve"), x).min(2.0);
    let c = if !g.is_empty() {
        let grad: Vec<(f32, [f32; 3])> = g.iter().map(|(s, n)| (*s, rgb_of(pal, n).map(|v| v as f32 / 255.0))).collect();
        client::fx::gradient_at(&grad, x)
    } else if let Some(a) = t.get("colour").and_then(|v| v.as_array()) {
        let v: Vec<f32> = a.iter().filter_map(|v| v.as_float().map(|f| f as f32).or(v.as_integer().map(|i| i as f32))).collect();
        [v.first().copied().unwrap_or(1.0), v.get(1).copied().unwrap_or(1.0), v.get(2).copied().unwrap_or(1.0)]
    } else {
        [0.55, 0.5, 0.45]
    };
    let m = |v: f32| ((v * bright).clamp(0.0, 1.0) * 255.0) as u8;
    Color32::from_rgb(m(c[0]), m(c[1]), m(c[2]))
}

/// A small moving picture of a row, side on, in closed form: no state, the same for the same time (the wall's
/// previews and the view's stand-in). shortcut: noise as a wobble and no collisions, good enough to choose by;
/// the game's renderer shows the real thing.
fn sketch(p: &egui::Painter, r: Rect, t: &toml::Table, pal: &engine::palette::Palette, now: f32) {
    let g = |k: &str, d: f32| t.get(k).and_then(|v| v.as_float().map(|f| f as f32).or(v.as_integer().map(|i| i as f32))).unwrap_or(d);
    let pair = |k: &str, d: [f32; 2]| {
        let v: Vec<f32> = t.get(k).and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|v| v.as_float().map(|f| f as f32).or(v.as_integer().map(|i| i as f32))).collect()).unwrap_or_default();
        [v.first().copied().unwrap_or(d[0]), v.get(1).copied().unwrap_or(d[1])]
    };
    let (life, size, speed) = (pair("life", [1.0, 1.0]), pair("size", [0.1, 0.1]), pair("speed", [0.0, 0.0]));
    let (up, grav, drag, rise) = (g("up", 0.0), g("gravity", 1.0), g("drag", 0.0), g("rise", 0.0));
    let noise = t.get("noise").and_then(|v| v.as_table()).map(|z| z.get("strength").and_then(|v| v.as_float().or(v.as_integer().map(|i| i as f64))).unwrap_or(0.0) as f32).unwrap_or(0.0);
    let (rate, count) = (g("rate", 0.0), g("count", 0.0));
    let start = t.get("start").and_then(|v| v.as_str()).unwrap_or("point");
    let wide = match start {
        "box" => pair("box", [0.3, 0.1])[0] * 0.5,
        "ring" => g("radius", 0.3),
        "block" | "faces" => 0.5,
        _ => 0.0,
    };
    let size_c = effect::curve(t, "size_curve");
    let (grow, shrink) = (g("grow", 0.0), g("shrink", 0.0));
    let scale = r.height() / 1.6;
    let ground = r.max.y - r.height() * 0.12;
    p.line_segment([Pos2::new(r.min.x, ground), Pos2::new(r.max.x, ground)], Stroke::new(1.0, Color32::from_gray(60)));
    let n = if rate > 0.0 { (rate * life[1]).ceil().min(90.0) as usize } else { count.min(90.0) as usize };
    let period = if rate > 0.0 { n as f32 / rate.max(0.01) } else { life[1] + 0.6 };
    let accel = rise - 9.8 * grav;
    for j in 0..n {
        let mut rng = Rng::new(j as u64 * 7919 + 13);
        let born = if rate > 0.0 { j as f32 / rate } else { 0.0 };
        let a = (now - born).rem_euclid(period);
        let l = life[0] + (life[1] - life[0]) * rng.f32();
        if a > l {
            continue;
        }
        let ang = std::f32::consts::PI * (0.5 + rng.signed() * 0.5);
        let sp = speed[0] + (speed[1] - speed[0]) * rng.f32();
        let (vx, vy) = (ang.cos() * sp, ang.sin().abs() * sp + up);
        let k = if drag > 0.0 { (1.0 - (-drag * a).exp()) / drag } else { a };
        let x0 = rng.signed() * wide;
        let mut x = x0 + vx * k + noise * 0.04 * (a * 3.0 + j as f32).sin();
        let mut y = vy * k + if drag > 0.0 { accel * (a - k) / drag } else { accel * a * a * 0.5 };
        if y < 0.0 {
            y = 0.0;
            x = x0 + vx * k;
        }
        let share = a / l;
        let mut s = size[0] + (size[1] - size[0]) * rng.f32();
        s *= client::fx::curve_at(&size_c, share);
        if grow > 0.0 && share < grow {
            s *= share / grow;
        }
        if shrink > 0.0 && share > 1.0 - shrink {
            s *= (1.0 - share) / shrink;
        }
        let c = Pos2::new(r.center().x + x * scale, ground - y * scale);
        if r.contains(c) {
            p.rect_filled(Rect::from_center_size(c, Vec2::splat((s * scale).max(1.5))), 0.0, colour_at(t, pal, share));
        }
    }
}

/// The model files of `data/models`, by name (read on open and when files change, never every frame).
fn model_names() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir("data/models").into_iter().flatten().flatten().filter_map(|e| e.file_name().to_str()?.strip_suffix(".toml").map(String::from)).collect();
    v.sort();
    v
}

/// An inspector too narrow for a label beside a slider and its value (a large interface scale): the label goes over.
fn narrow(ui: &Ui) -> bool {
    ui.available_width() < 220.0
}

pub struct Effects {
    docs: BTreeMap<String, EDoc>,
    open: Option<String>,
    disk: String,
    /// What the renderer was last given (the file's text) and how often it was written.
    written: Option<String>,
    gen: u64,
    restarts: u64,
    refused: Option<String>,
    stop: Option<usize>,
    stop_drag: bool,
    swatch: String,
    point: Option<(&'static str, usize)>,
    wall: Option<Wall<Keys>>,
    walls_made: u64,
    merging: Option<String>,
    playing: bool,
    now: f32,
    /// `model_names`, as last read.
    models: Vec<String>,
}

impl Effects {
    pub fn new(_app: &App) -> Effects {
        let mut s = Effects { docs: BTreeMap::new(), open: None, disk: String::new(), written: None, gen: 0, restarts: 0, refused: None, stop: None, stop_drag: false, swatch: String::new(), point: None, wall: None, walls_made: 0, merging: None, playing: true, now: 0.0, models: model_names() };
        s.read_disk();
        s.open = s.docs.keys().find(|n| n.as_str() == "fire").or(s.docs.keys().next()).cloned();
        s
    }

    fn read_disk(&mut self) {
        let text = std::fs::read_to_string(EFFECTS).unwrap_or_default();
        if text == self.disk {
            return;
        }
        for sp in rows::spans(&text).into_iter().filter(|s| s.header == "effect" || s.header == "group") {
            let k = Keys::parse(&text[sp.at.clone()]);
            let saved = Some(k.text());
            match self.docs.get_mut(&sp.name) {
                Some(d) if d.dirty() => d.saved = saved,
                Some(d) => {
                    if d.k != k {
                        d.hist.push((std::mem::replace(&mut d.k, k), d.off.clone()), "Changed on disk");
                    }
                    d.saved = saved;
                }
                None => {
                    self.docs.insert(sp.name.clone(), EDoc { k, hist: History::default(), saved, off: vec![] });
                }
            }
        }
        self.disk = text;
    }

    /// What a `model` or `texture` look may name: the model files, or the blocks the game takes (its own list).
    fn names(&self, look_: &str) -> Vec<String> {
        match look_ {
            "model" => self.models.clone(),
            "texture" => engine::blocks::BLOCKS.placeable().map(|d| d.name.clone()).collect(),
            _ => vec![],
        }
    }

    fn doc(&self) -> Option<&EDoc> {
        self.open.as_ref().and_then(|n| self.docs.get(n))
    }
    fn table(&self) -> toml::Table {
        self.doc().and_then(|d| d.k.table().ok()).unwrap_or_default()
    }

    fn edit(&mut self, name: &str, f: impl FnOnce(&mut Keys, &mut Vec<String>)) {
        let merge = self.merging.as_deref() == Some(name);
        let Some(d) = self.open.as_ref().and_then(|n| self.docs.get_mut(n)) else { return };
        let before = (d.k.clone(), d.off.clone());
        f(&mut d.k, &mut d.off);
        if (d.k.clone(), d.off.clone()) != before && !merge {
            d.hist.push(before, name);
        }
    }
    fn edit_merged(&mut self, name: &str, f: impl FnOnce(&mut Keys, &mut Vec<String>)) {
        self.edit(name, f);
        self.merging = Some(name.into());
    }

    /// The file as the page has it: the disk's text with every row edited here put in.
    fn text(&self) -> String {
        let mut t = self.disk.clone();
        for (n, d) in self.docs.iter().filter(|(_, d)| d.dirty()) {
            t = rows::put(&t, &d.k.header, n, &d.k.text());
        }
        t
    }

    /// The page's copy of the file to the renderer when it changed and the game takes it; the open row playing.
    fn feed_view(&mut self, app: &mut App) {
        let text = self.text();
        if self.written.as_ref() != Some(&text) {
            self.refused = client::fx::parse_set(&text, &app.pal).err();
            if self.refused.is_none() {
                let _ = std::fs::create_dir_all(DIR);
                if std::fs::write(format!("{DIR}/effects.toml"), &text).is_ok() {
                    self.gen += 1;
                }
            }
            self.written = Some(text);
        }
        app.view.effects(&format!("{DIR}/effects.toml"), self.gen);
        app.view.show_model(None, None, 0.0);
        app.view.place = String::new();
        let shown = self.open.clone().filter(|n| self.docs.get(n).is_some_and(|d| d.saved.is_some() || self.refused.is_none())).filter(|_| self.playing);
        app.view.effect = shown.map(|n| (n, self.restarts));
        app.view.live = self.playing;
    }

    fn save(&mut self, app: &mut App) {
        let Some(name) = self.open.clone() else { return };
        let d = &self.docs[&name];
        let (header, text) = (d.k.header.clone(), d.k.text());
        let pal = app.pal.clone();
        match rows::save(EFFECTS, &header, &name, &text, |t| client::fx::parse_set(t, &pal).map(|_| ())) {
            Ok(new) => {
                self.docs.get_mut(&name).unwrap().saved = Some(text);
                self.disk = new;
                let drafts = art::set_draft(&std::fs::read_to_string(DRAFTS).unwrap_or_default(), &format!("{EFFECTS}#{name}"), false);
                app.write_drafts(drafts);
                app.note(EFFECTS);
                app.toast(format!("Saved {name} in {EFFECTS}"));
                app.check();
            }
            Err(e) => app.refuse(format!("Not saved: {e}")),
        }
    }

    fn new_effect(&mut self, preset: &str) {
        let Some(src) = self.docs.get(preset).map(|d| d.k.clone()) else { return };
        let name = (1..).map(|k| if k == 1 { format!("{preset}.new") } else { format!("{preset}.new{k}") }).find(|n| !self.docs.contains_key(n)).unwrap();
        let mut k = src;
        k.set("name", Some(format!("\"{name}\"")));
        self.docs.insert(name.clone(), EDoc { k, hist: History::default(), saved: None, off: vec![] });
        self.open = Some(name);
        self.wall = None;
    }

    fn numbers(&mut self, ui: &mut Ui, group: &str) {
        let t = self.table();
        for (_, key, words, help) in CHOICES.iter().filter(|c| c.0 == group) {
            let cur = t.get(*key).and_then(|v| v.as_str()).unwrap_or(words[0]).to_string();
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(4.0);
                ui.add_sized(Vec2::new(64.0, 18.0), egui::Label::new(RichText::new(*key).small()));
                for w in *words {
                    let r = look::tip(look::chip(ui, cur == *w, w), help, "");
                    crate::drive::mark(ui.ctx(), format!("choice:{key}:{w}"), r.rect);
                    if r.clicked() && cur != *w {
                        if *key == "look" {
                            // a model or a texture look names a file that exists, the first unless the row named one
                            let first = self.names(w).into_iter().next();
                            if matches!(*w, "model" | "texture") && first.is_none() {
                                continue;
                            }
                            self.edit(&format!("{key}: {w}"), |k, _| effect::choose_look(k, w, first.as_deref()));
                        } else {
                            self.edit(&format!("{key}: {w}"), |k, _| effect::choose(k, key, w));
                        }
                    }
                }
            });
        }
        let look_ = t.get("look").and_then(|v| v.as_str()).unwrap_or("square").to_string();
        match group {
            "Look" if matches!(look_.as_str(), "model" | "texture") => self.pick(ui, &look_, self.names(&look_), false, "The file it shows: a model of data/models, or the block whose top face it shows"),
            "Life" => {
                let me = self.open.clone().unwrap_or_default();
                let rows = self.docs.iter().filter(|(n, d)| !d.group() && d.saved.is_some() && **n != me).map(|(n, _)| n.clone()).collect();
                self.pick(ui, "then", rows, true, "The row played where each particle ends (a row of the file), or none");
            }
            _ => {}
        }
        let start = t.get("start").and_then(|v| v.as_str()).unwrap_or("point").to_string();
        for fl in FIELDS.iter().filter(|f| f.group == group) {
            let own = match fl.key {
                "radius" => Some("ring"),
                "angle" | "dir" => Some("cone"),
                "box" => Some("box"),
                _ => None,
            };
            if own.is_some_and(|s| s != start) {
                continue;
            }
            // one colour: a square or a texture without a gradient
            if fl.key == "colour" && (t.contains_key("gradient") || !matches!(look_.as_str(), "square" | "texture")) {
                continue;
            }
            let Some(mut v) = effect::get(&t, fl).or(match fl.form {
                Form::Noise(n) => Some(if n == "scale" { 1.0 } else { 0.0 }),
                Form::Pair(_) => Some(fl.min),
                _ => None,
            }) else { continue };
            if narrow(ui) {
                ui.label(RichText::new(fl.label).small());
            }
            ui.horizontal(|ui| {
                if !narrow(ui) {
                    ui.add_sized(Vec2::new(64.0, 18.0), egui::Label::new(RichText::new(fl.label).small()).truncate());
                }
                ui.spacing_mut().slider_width = (ui.available_width() - 100.0).max(40.0);
                let log = fl.min > 0.0 && fl.max / fl.min >= 50.0;
                let u = fl.unit;
                let r = ui.add(egui::Slider::new(&mut v, fl.min..=fl.max).logarithmic(log).custom_formatter(move |x, _| if x.abs() >= 100.0 { format!("{x:.0} {u}") } else { format!("{x:.2} {u}") }));
                crate::drive::mark(ui.ctx(), format!("field:{}", fl.label), r.rect);
                if look::tip(r, fl.help, "").changed() {
                    self.edit_merged(fl.label, |k, _| effect::set(k, fl, v));
                }
            });
        }
    }

    /// A key naming a file or a row, chosen from those that exist; `none` offers none.
    fn pick(&mut self, ui: &mut Ui, key: &str, names: Vec<String>, none: bool, help: &str) {
        let cur = self.table().get(key).and_then(|v| v.as_str()).map(String::from);
        let mut pick = cur.clone();
        if narrow(ui) {
            ui.label(RichText::new(key).small());
        }
        ui.horizontal(|ui| {
            if !narrow(ui) {
                ui.add_sized(Vec2::new(64.0, 18.0), egui::Label::new(RichText::new(key).small()));
            }
            let r = egui::ComboBox::from_id_salt(("effect name", key.to_string())).selected_text(cur.clone().unwrap_or("none".into())).width((ui.available_width() - 16.0).max(60.0)).height(480.0).show_ui(ui, |ui| {
                if none {
                    let r = ui.selectable_value(&mut pick, None, "none");
                    if ui.is_rect_visible(r.rect) {
                        crate::drive::mark(ui.ctx(), format!("pick:{key}:none"), r.rect);
                    }
                }
                for n in &names {
                    let r = ui.selectable_value(&mut pick, Some(n.clone()), n);
                    if ui.is_rect_visible(r.rect) {
                        crate::drive::mark(ui.ctx(), format!("pick:{key}:{n}"), r.rect);
                    }
                }
            });
            crate::drive::mark(ui.ctx(), format!("pick:{key}"), r.response.rect);
            look::tip(r.response, help, "");
        });
        if pick != cur {
            self.edit(&format!("{key}: {}", pick.as_deref().unwrap_or("none")), |k, _| effect::set_name(k, key, pick.as_deref()));
        }
    }

    /// The gradient: a bar of the colour over the life with its stops; drag a stop, click the bar to add one,
    /// right click a stop to remove it, a swatch colours the chosen stop.
    fn gradient(&mut self, ui: &mut Ui, app: &mut App) {
        let t = self.table();
        let look_ = t.get("look").and_then(|v| v.as_str()).unwrap_or("square");
        if !matches!(look_, "square" | "texture") {
            look::weak(ui, "a gradient colours squares and textures");
            return;
        }
        let mut g = effect::gradient(&t);
        if g.is_empty() {
            if look::tip(ui.button("Colour over life"), "Give it a gradient of palette colours over its life, in place of one colour", "").clicked() {
                self.edit("Add a gradient", |k, _| {
                    k.set("gradient", Some("[[0, \"lamp light\"], [1, \"stone dark\"]]".into()));
                    k.set("colour", None);
                });
            }
            return;
        }
        let (bar, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.0), Sense::click_and_drag());
        crate::drive::mark(ui.ctx(), "gradient", bar);
        let p = ui.painter_at(bar.expand(2.0));
        let strip = Rect::from_min_max(bar.min, Pos2::new(bar.max.x, bar.min.y + 20.0));
        let n = 64;
        for i in 0..n {
            let x = i as f32 / n as f32;
            let rr = Rect::from_min_max(Pos2::new(strip.min.x + x * strip.width(), strip.min.y), Pos2::new(strip.min.x + (x + 1.0 / n as f32) * strip.width() + 1.0, strip.max.y));
            p.rect_filled(rr, 0.0, colour_at(&t, &app.pal, x + 0.5 / n as f32));
        }
        p.rect_stroke(strip, 3.0, Stroke::new(1.0, Color32::from_gray(80)), egui::StrokeKind::Outside);
        let x_of = |s: f32| strip.min.x + s * strip.width();
        let near = |g: &effect::Grad, pos: Pos2| g.iter().enumerate().map(|(i, s)| (i, (x_of(s.0) - pos.x).abs())).filter(|(_, d)| *d < 8.0).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(i, _)| i);
        for (i, (s, c)) in g.iter().enumerate() {
            let x = x_of(*s);
            crate::drive::mark(ui.ctx(), format!("stop:{i}"), Rect::from_center_size(Pos2::new(x, bar.max.y - 6.0), Vec2::splat(2.0)));
            let on = self.stop == Some(i);
            let tri = vec![Pos2::new(x, strip.max.y), Pos2::new(x + 6.0, bar.max.y - 2.0), Pos2::new(x - 6.0, bar.max.y - 2.0)];
            p.add(egui::Shape::convex_polygon(tri, c32(rgb_of(&app.pal, c)), Stroke::new(if on { 2.0 } else { 1.0 }, if on { look::ACCENT } else { Color32::from_gray(200) })));
        }
        // (the press's place, not the pointer's when the drag is reported: a fast drag is already past the stop)
        if resp.drag_started() {
            if let Some(pos) = ui.input(|i| i.pointer.press_origin()) {
                self.stop = near(&g, pos);
                self.stop_drag = self.stop.is_some();
            }
        }
        if let (true, Some(i), Some(pos)) = (self.stop_drag && resp.dragged(), self.stop, resp.interact_pointer_pos()) {
            effect::move_stop(&mut g, i, (pos.x - strip.min.x) / strip.width());
            let text = effect::write_gradient(&g);
            self.edit_merged("Move a stop", |k, _| k.set("gradient", text));
        }
        if resp.drag_stopped() {
            self.stop_drag = false;
        }
        if resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                match near(&g, pos) {
                    Some(i) => self.stop = Some(i),
                    None => {
                        let at = (pos.x - strip.min.x) / strip.width();
                        let c = self.stop.and_then(|i| g.get(i)).map_or(g[0].1.clone(), |s| s.1.clone());
                        self.stop = Some(effect::add_stop(&mut g, at, &c));
                        let text = effect::write_gradient(&g);
                        self.edit("Add a stop", |k, _| k.set("gradient", text));
                    }
                }
            }
        }
        if resp.secondary_clicked() {
            if let Some(i) = resp.interact_pointer_pos().and_then(|q| near(&g, q)) {
                if effect::remove_stop(&mut g, i) {
                    self.stop = None;
                    let text = effect::write_gradient(&g);
                    self.edit("Remove a stop", |k, _| k.set("gradient", text));
                }
            }
        }
        look::tip(resp, "Colour over the life: drag a stop, click the bar to add one, right click a stop to remove it", "");
        let one = look::tip(look::chip(ui, false, "one colour"), "One colour all its life, the first stop's, in place of the gradient", "");
        crate::drive::mark(ui.ctx(), "btn:one_colour", one.rect);
        if one.clicked() {
            let c = rgb_of(&app.pal, &g[0].1).map(|v| rows::num(v as f32 / 255.0));
            let text = format!("[{}, {}, {}]", c[0], c[1], c[2]);
            self.edit("One colour", |k, _| {
                k.set("gradient", None);
                k.set("colour", Some(text));
            });
            return;
        }
        let chosen = self.stop.filter(|i| *i < g.len());
        match chosen {
            Some(i) => look::weak(ui, format!("stop {} at {:.0} %: {}; a swatch below colours it", i + 1, g[i].0 * 100.0, g[i].1)),
            None => look::weak(ui, "click a stop, then a swatch to colour it"),
        };
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(2.0);
            for c in &app.pal.colours {
                let (r, resp) = ui.allocate_exact_size(Vec2::splat(16.0), Sense::click());
                ui.painter().rect_filled(r, 2.0, c32(c.rgb));
                if chosen.is_some_and(|i| g[i].1 == c.name) {
                    ui.painter().rect_stroke(r, 2.0, Stroke::new(2.0, look::ACCENT), egui::StrokeKind::Outside);
                }
                if look::tip(resp, &c.name, "").clicked() {
                    if let Some(i) = chosen {
                        g[i].1 = c.name.clone();
                        let text = effect::write_gradient(&g);
                        self.edit("Colour a stop", |k, _| k.set("gradient", text));
                    }
                    self.swatch = c.name.clone();
                }
            }
        });
    }

    /// A curve over the life as a small graph: drag a point, click to add one, right click to remove; ready shapes.
    fn curve(&mut self, ui: &mut Ui, key: &'static str, title: &str) {
        let t = self.table();
        let mut c = effect::curve(&t, key);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(4.0);
            ui.label(RichText::new(title).small());
            for (name, help, _) in SHAPES {
                if look::tip(look::chip(ui, c == effect::shape(name).unwrap(), name), help, "").clicked() {
                    let text = effect::write_curve(&effect::shape(name).unwrap());
                    self.edit(&format!("{title}: {name}"), |k, _| k.set(key, text));
                }
            }
            if !c.is_empty() && look::tip(look::chip(ui, false, "none"), "No curve: the same all its life", "").clicked() {
                self.edit(&format!("{title}: none"), |k, _| k.set(key, None));
            }
        });
        let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 64.0), Sense::click_and_drag());
        crate::drive::mark(ui.ctx(), format!("curve:{key}"), r);
        let p = ui.painter_at(r.expand(4.0));
        p.rect_filled(r, 4.0, ui.visuals().extreme_bg_color);
        let to = |q: [f32; 2]| Pos2::new(r.min.x + q[0] * r.width(), r.max.y - q[1] / CURVE_MAX * r.height());
        let from = |q: Pos2| [((q.x - r.min.x) / r.width()).clamp(0.0, 1.0), ((r.max.y - q.y) / r.height() * CURVE_MAX).clamp(0.0, CURVE_MAX)];
        let one = to([0.0, 1.0]).y;
        p.line_segment([Pos2::new(r.min.x, one), Pos2::new(r.max.x, one)], Stroke::new(1.0, Color32::from_gray(60)));
        let shown = if c.is_empty() { vec![[0.0, 1.0], [1.0, 1.0]] } else { c.clone() };
        let mut line = vec![to([0.0, shown[0][1]])];
        line.extend(shown.iter().map(|q| to(*q)));
        line.push(to([1.0, shown.last().unwrap()[1]]));
        p.add(egui::Shape::line(line, Stroke::new(1.5, if c.is_empty() { Color32::from_gray(90) } else { look::ACCENT })));
        for (i, q) in c.iter().enumerate() {
            crate::drive::mark(ui.ctx(), format!("point:{key}:{i}"), Rect::from_center_size(to(*q), Vec2::splat(2.0)));
            let on = self.point == Some((key, i));
            p.circle_filled(to(*q), if on { 5.0 } else { 4.0 }, if on { look::ACCENT } else { Color32::from_gray(220) });
        }
        let near = |c: &effect::Curve, pos: Pos2| c.iter().position(|q| to(*q).distance(pos) < 9.0);
        if resp.drag_started() {
            self.point = ui.input(|i| i.pointer.press_origin()).and_then(|q| near(&c, q)).map(|i| (key, i));
        }
        if let (Some((k0, i)), Some(pos)) = (self.point, resp.interact_pointer_pos().filter(|_| resp.dragged())) {
            if k0 == key {
                let q = from(pos);
                effect::move_point(&mut c, i, q[0], q[1]);
                let text = effect::write_curve(&c);
                self.edit_merged(&format!("{title}: move a point"), |k, _| k.set(key, text));
            }
        }
        if resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos().filter(|q| near(&c, *q).is_none()) {
                if c.is_empty() {
                    c = vec![[0.0, 1.0], [1.0, 1.0]];
                }
                let i = effect::add_point(&mut c, from(pos)[0]);
                effect::move_point(&mut c, i, from(pos)[0], from(pos)[1]);
                let text = effect::write_curve(&c);
                self.edit(&format!("{title}: add a point"), |k, _| k.set(key, text));
            }
        }
        if resp.secondary_clicked() {
            if let Some(i) = resp.interact_pointer_pos().and_then(|q| near(&c, q)) {
                if effect::remove_point(&mut c, i) {
                    let text = effect::write_curve(&c);
                    self.edit(&format!("{title}: remove a point"), |k, _| k.set(key, text));
                }
            }
        }
        look::tip(resp, &format!("{title} over the life (the line is 1): drag a point, click to add one, right click to remove"), "");
    }

    /// The noise field as small moving strokes, as the game pushes particles through it.
    fn noise_picture(&self, ui: &mut Ui) {
        let t = self.table();
        let Some(z) = t.get("noise").and_then(|v| v.as_table()) else {
            look::weak(ui, "no noise: the noise slider gives it some");
            return;
        };
        let n = |k: &str, d: f32| z.get(k).and_then(|v| v.as_float().map(|f| f as f32).or(v.as_integer().map(|i| i as f32))).unwrap_or(d);
        let field = client::fx::Noise { strength: n("strength", 0.0), scale: n("scale", 1.0).max(0.01), speed: n("speed", 0.0) };
        let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 70.0), Sense::hover());
        let p = ui.painter_at(r);
        p.rect_filled(r, 4.0, ui.visuals().extreme_bg_color);
        let step = 10.0;
        let metres = 1.5 / r.width();
        let len = (field.strength / 8.0).sqrt() * 9.0 + 1.0;
        let mut y = r.min.y + step / 2.0;
        while y < r.max.y {
            let mut x = r.min.x + step / 2.0;
            while x < r.max.x {
                let q = Vec3::new((x - r.min.x) * metres, (r.max.y - y) * metres, 0.0);
                let v = client::fx::noise_at(q, self.now, &field);
                let d = Vec2::new(v.x, -v.y);
                let a = Pos2::new(x, y);
                p.line_segment([a - d * len * 0.5, a + d * len * 0.5], Stroke::new(1.2, Color32::from_gray(90 + (v.z.abs() * 120.0) as u8)));
                x += step;
            }
            y += step;
        }
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
    }

    fn group_list(&mut self, ui: &mut Ui, app: &mut App) {
        let Some(d) = self.doc() else { return };
        let t = d.k.table().unwrap_or_default();
        let on: Vec<String> = t.get("rows").and_then(|v| v.as_array()).into_iter().flatten().filter_map(|v| v.as_str().map(String::from)).collect();
        let mut all = on.clone();
        all.extend(d.off.iter().cloned());
        let mut go = None;
        for n in all {
            ui.horizontal(|ui| {
                let mut is_on = on.contains(&n);
                if look::tip(ui.checkbox(&mut is_on, ""), "On or off in the group (an off row is not saved in it)", "").changed() {
                    let name = n.clone();
                    self.edit(if is_on { "Row on" } else { "Row off" }, |k, off| {
                        let t = k.table().unwrap_or_default();
                        let mut rows: Vec<String> = t.get("rows").and_then(|v| v.as_array()).into_iter().flatten().filter_map(|v| v.as_str().map(String::from)).collect();
                        if is_on {
                            off.retain(|o| *o != name);
                            rows.push(name);
                        } else if rows.len() > 1 {
                            rows.retain(|r| *r != name);
                            off.push(name);
                        }
                        k.set("rows", Some(format!("[{}]", rows.iter().map(|r| format!("\"{r}\"")).collect::<Vec<_>>().join(", "))));
                    });
                }
                if look::tip(ui.selectable_label(false, &n), "Open this row", "").clicked() {
                    go = Some(n.clone());
                }
            });
        }
        if let Some(n) = go {
            self.open(app, &n);
        }
    }
}

impl Page for Effects {
    fn name(&self) -> &'static str {
        "Effects"
    }

    fn files(&self, app: &App) -> Vec<FileRow> {
        let mut v: Vec<(bool, &String, &EDoc)> = self.docs.iter().map(|(n, d)| (!d.group(), n, d)).collect();
        v.sort_by_key(|x| (x.0, x.1.clone()));
        v.into_iter()
            .map(|(_, n, d)| {
                let t = d.k.table().unwrap_or_default();
                let colour = if d.group() { Color32::from_gray(150) } else { colour_at(&t, &app.pal, 0.3) };
                let label = if d.group() { format!("{n} (group)") } else { n.clone() };
                FileRow { id: n.clone(), label, thumb: Thumb::Flat(colour), flags: Flags { made: d.saved.is_some(), draft: app.is_draft(&format!("{EFFECTS}#{n}")), dirty: d.dirty(), problem: false } }
            })
            .collect()
    }

    fn current(&self) -> Option<String> {
        self.open.clone()
    }

    fn open(&mut self, _app: &mut App, id: &str) {
        if self.docs.contains_key(id) {
            self.open = Some(id.into());
            (self.wall, self.stop, self.point) = (None, None, None);
            self.restarts += 1;
        }
    }

    fn commands(&self, _app: &App) -> Vec<Cmd> {
        let mut v = vec![
            Cmd::new("restart", "Play it again", "the effect from its start in the view").key(Modifiers::NONE, Key::R),
            Cmd::new("play", "Play or stop", "the effect in the view").key(Modifiers::NONE, Key::Space),
            Cmd::new("vary", "Variations", "a wall of variations of this row's numbers").key(Modifiers::NONE, Key::M),
            Cmd::new("save", "Save the effect", "rewrite only this row of data/effects.toml (checked by the game's reader) and take it off the drafts list").key(Modifiers::COMMAND, Key::S),
            Cmd::new("undo", "Undo", "undo the last change to this effect").key(Modifiers::COMMAND, Key::Z),
            Cmd::new("redo", "Redo", "redo what was undone").key(Modifiers::COMMAND, Key::Y),
        ];
        for p in PRESETS {
            v.push(Cmd::new(format!("new:{p}"), format!("New effect from {p}"), "a new row starting from this preset"));
        }
        v
    }

    fn run(&mut self, app: &mut App, id: &str) {
        // with an argument (the command box and shot=): new:<preset>, take:<wall item>, hour:<h>, scene:<grass|cave>, stop:<i>
        if let Some((cmd, arg)) = id.split_once(':') {
            match cmd {
                "new" => self.new_effect(arg),
                "take" => {
                    if let Some(k) = self.wall.as_ref().and_then(|w| w.items.get(arg.parse::<usize>().unwrap_or(99)).cloned()) {
                        self.edit("Take a variation", |d, _| *d = k);
                    }
                }
                "hour" => app.view.hour = arg.parse().unwrap_or(app.view.hour),
                "scene" => app.view.scene = if arg == "cave" { super::viewport::Scene::Cave } else { super::viewport::Scene::Grass },
                "stop" => self.stop = arg.parse().ok(),
                _ => {}
            }
            return;
        }
        match id {
            "restart" => {
                self.restarts += 1;
                self.playing = true;
            }
            "play" => self.playing = !self.playing,
            "vary" => {
                if let Some(k) = self.doc().filter(|d| !d.group()).map(|d| d.k.clone()) {
                    self.walls_made += 1;
                    self.wall = Some(Wall::new(k, 6, 0.4, 0xEF + self.walls_made, &effect::vary));
                }
            }
            "save" => self.save(app),
            "undo" | "redo" => {
                if let Some(d) = self.open.as_ref().and_then(|n| self.docs.get_mut(n)) {
                    let mut now = (d.k.clone(), d.off.clone());
                    if if id == "undo" { d.hist.undo(&mut now) } else { d.hist.redo(&mut now) } {
                        (d.k, d.off) = now;
                    }
                }
            }
            _ => {}
        }
    }

    fn work(&mut self, ui: &mut Ui, app: &mut App) {
        self.now = ui.input(|i| i.time) as f32;
        if !ui.input(|i| i.pointer.any_down()) {
            self.merging = None;
        }
        app.view.controls(ui);
        let Some(name) = self.open.clone() else {
            look::weak(ui, "No effect");
            return;
        };
        self.feed_view(app);
        ui.horizontal(|ui| {
            ui.label(RichText::new(&name).strong());
            let cmds = self.commands(app);
            for (id, label) in [("restart", "Play again"), ("play", if self.playing { "Stop" } else { "Play" })] {
                let c = cmds.iter().find(|c| c.id == id).unwrap();
                if look::tip(ui.button(label), &c.help, &c.keys()).clicked() {
                    self.run(app, id);
                }
            }
            if let Some(e) = &self.refused {
                ui.add(egui::Label::new(RichText::new(format!("The game refuses this: {e}")).small().color(look::BAD)).truncate());
            }
        });
        let t = self.table();
        let pal = app.pal.clone();
        let now = self.now;
        let h = ui.available_height();
        app.view.ui(ui, h, |p, _, r| sketch(p, r.shrink(GAP), &t, &pal, now));
        if self.playing {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
        }
    }

    fn inspector(&mut self, ui: &mut Ui, app: &mut App) {
        let cmds = self.commands(app);
        look::section(ui, "Effect");
        ui.horizontal_wrapped(|ui| {
            for (id, label) in [("undo", "Undo"), ("redo", "Redo"), ("save", "Save")] {
                let c = cmds.iter().find(|c| c.id == id).unwrap();
                if look::tip(ui.button(label), &c.help, &c.keys()).clicked() {
                    self.run(app, id);
                }
            }
            ui.menu_button("New from…", |ui| {
                let have: Vec<&str> = PRESETS.iter().copied().filter(|p| self.docs.contains_key(*p)).collect();
                for p in have {
                    if ui.button(p).clicked() {
                        self.new_effect(p);
                        ui.close();
                    }
                }
            })
            .response
            .on_hover_text("A new effect starting from a preset: fire, smoke, sparks…");
        });
        let Some(d) = self.doc() else { return };
        if d.group() {
            ui.separator();
            look::section(ui, "Rows played together");
            self.group_list(ui, app);
            return;
        }
        let name = self.open.clone().unwrap_or_default();
        let in_groups: Vec<String> = self.docs.iter().filter(|(_, g)| g.group() && g.k.get("rows").is_some_and(|r| r.contains(&format!("\"{name}\"")))).map(|(n, _)| n.clone()).collect();
        if !in_groups.is_empty() {
            look::weak(ui, format!("played in {}", in_groups.join(", ")));
        }
        for g in GROUPS {
            ui.separator();
            look::section(ui, g);
            self.numbers(ui, g);
            match g {
                "Motion" => self.noise_picture(ui),
                "Life" => self.curve(ui, "size_curve", "size"),
                "Look" => self.gradient(ui, app),
                "Light" => self.curve(ui, "bright_curve", "brightness"),
                _ => {}
            }
        }
        ui.separator();
        look::section(ui, "Variations");
        if self.wall.is_none() {
            if look::tip(ui.button("Make variations"), "Six variations of this row's numbers, each moving; a click takes one", "M").clicked() {
                self.run(app, "vary");
            }
            return;
        }
        let chosen = self.doc().and_then(|d| self.wall.as_ref()?.items.iter().position(|k| *k == d.k));
        let (pal, now) = (app.pal.clone(), self.now);
        let mut act = wallui::Act::None;
        if let Some(w) = &mut self.wall {
            let cell = Vec2::new(((ui.available_width() - 3.0 * GAP) / 3.0).floor().max(60.0), 70.0);
            act = wallui::show(ui, w, chosen, cell, |p, r, k| sketch(p, r, &k.table().unwrap_or_default(), &pal, now));
        }
        let gen: crate::wall::Gen<Keys> = &effect::vary;
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
        "R plays it again, Space stops it, M makes variations; the gradient: drag a stop, click the bar to add, right click to remove".into()
    }

    fn history(&self, _app: &App) -> Option<(Vec<String>, Vec<String>)> {
        let d = self.doc()?;
        Some((d.hist.undo.iter().map(|s| s.name.clone()).collect(), d.hist.redo.iter().rev().map(|s| s.name.clone()).collect()))
    }

    fn back_to(&mut self, _app: &mut App, n: usize) {
        if let Some(d) = self.open.as_ref().and_then(|k| self.docs.get_mut(k)) {
            let mut now = (d.k.clone(), d.off.clone());
            d.hist.back_to(&mut now, n);
            (d.k, d.off) = now;
        }
    }

    fn refresh(&mut self, _app: &App) {
        self.read_disk();
        self.models = model_names();
    }

    fn unsaved(&self) -> Vec<String> {
        self.docs.iter().filter(|(_, d)| d.dirty()).map(|(n, _)| format!("{EFFECTS}#{n}")).collect()
    }

    fn save_all(&mut self, app: &mut App) {
        let keep = self.open.clone();
        for n in self.docs.iter().filter(|(_, d)| d.dirty()).map(|(n, _)| n.clone()).collect::<Vec<_>>() {
            self.open = Some(n);
            self.save(app);
        }
        self.open = keep;
    }

    fn settle(&mut self, ctx: &egui::Context, app: &mut App) {
        app.view.settle(ctx, std::time::Duration::from_secs(90));
    }
}
