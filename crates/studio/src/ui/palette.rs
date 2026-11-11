//! The Palette page (docs/design/studio.md "The Palette page", studio-tools.md "Palette: ramps"): the colours
//! large, in the file's order; the inspector names and sets the chosen one, and a change repaints every texture.
use super::look::{self, c32, GAP};
use super::{App, Cmd, FileRow, Page, Thumb, PAL, TEX};
use crate::art;
use crate::shell::Flags;
use eframe::egui::{self, Color32, Pos2, Rect, RichText, Sense, StrokeKind, Ui, Vec2};
use engine::palette::{hex, to_hex, Colour};

pub struct PalettePage {
    sel: usize,
    name: String,
    hex: String,
    hsv: [f32; 3],
    moving: Option<usize>,
    /// The colour a slider drag started from, written out when it stops.
    edit_from: Option<[u8; 3]>,
}

impl PalettePage {
    pub fn new(app: &App) -> PalettePage {
        let mut p = PalettePage { sel: 0, name: String::new(), hex: String::new(), hsv: [0.0; 3], moving: None, edit_from: None };
        p.select(app, 0);
        p
    }

    fn select(&mut self, app: &App, i: usize) {
        self.sel = i.min(app.pal.colours.len().saturating_sub(1));
        if let Some(c) = app.pal.colours.get(self.sel) {
            self.name = c.name.clone();
            self.hex = to_hex(c.rgb);
            let h = egui::ecolor::Hsva::from_rgb(c.rgb.map(|v| v as f32 / 255.0));
            self.hsv = [h.h, h.s, h.v];
        }
    }

    fn uses(app: &App, rgb: [u8; 3]) -> Vec<String> {
        app.docs.iter().filter(|(_, d)| d.img.uses(rgb) || d.saved.as_ref().is_some_and(|s| s.uses(rgb))).map(|(n, _)| n.clone()).collect()
    }

    /// Changes colour `sel` in the palette and in every texture in memory; `flush` writes it all out.
    fn set_colour(&mut self, app: &mut App, rgb: [u8; 3]) {
        let old = app.pal.colours[self.sel].rgb;
        if old == rgb {
            return;
        }
        if let Err(e) = app.pal.check(&app.pal.colours[self.sel].name, rgb, Some(self.sel)) {
            app.refuse(e);
            return;
        }
        self.edit_from.get_or_insert(old);
        app.pal.colours[self.sel].rgb = rgb;
        self.hex = to_hex(rgb);
        for d in app.docs.values_mut() {
            for img in std::iter::once(&mut d.img).chain(d.saved.as_mut()).chain(d.hist.imgs()) {
                art::repaint(img, old, rgb);
            }
        }
    }

    fn flush(&mut self, app: &mut App) {
        let to = app.pal.colours[self.sel].rgb;
        // a slider let go where it started changed nothing: no file is rewritten and the palette stays a draft
        let Some(from) = self.edit_from.take().filter(|f| *f != to) else { return };
        app.save_palette(true);
        let (done, failed) = art::repaint_dir(std::path::Path::new(TEX), from, to);
        for n in &done {
            app.note(&format!("{TEX}/{n}"));
        }
        app.toast(format!("{} {} to {}: repainted {} file(s)", app.pal.colours[self.sel].name, to_hex(from), to_hex(to), done.len()));
        for (n, why) in failed {
            // that file still holds the old colour on disk: it shows as not saved, so a save writes it
            if let Some(d) = app.docs.get_mut(&n) {
                d.saved = std::fs::read(format!("{TEX}/{n}")).ok().and_then(|b| art::decode(&b).ok());
            }
            app.refuse(format!("Not written: {why}"));
        }
    }

    fn add(&mut self, app: &mut App) {
        let name = (1..).map(|k| format!("new {k}")).find(|n| app.pal.colours.iter().all(|c| c.name != *n)).unwrap();
        let mut rgb = app.pal.colours.get(self.sel).map_or([128; 3], |c| c.rgb);
        while app.pal.find(rgb).is_some() {
            rgb[2] = rgb[2].wrapping_add(1);
            if rgb[2] == 0 {
                rgb[1] = rgb[1].wrapping_add(1);
            }
        }
        app.pal.colours.push(Colour { name, rgb });
        self.select(app, app.pal.colours.len() - 1);
        app.save_palette(false);
    }

    fn remove(&mut self, app: &mut App) {
        let Some(c) = app.pal.colours.get(self.sel) else { return };
        if !Self::uses(app, c.rgb).is_empty() {
            return app.refuse(format!("{} is in use: repaint those files first", c.name));
        }
        let c = app.pal.colours.remove(self.sel);
        self.select(app, self.sel);
        app.toast(format!("Removed {}", c.name));
        app.save_palette(false);
    }
}

impl Page for PalettePage {
    fn name(&self) -> &'static str {
        "Palette"
    }
    fn files(&self, app: &App) -> Vec<FileRow> {
        let draft = app.is_draft(PAL);
        app.pal.colours.iter().map(|c| FileRow { id: c.name.clone(), label: c.name.clone(), thumb: Thumb::Flat(c32(c.rgb)), flags: Flags { made: true, draft, dirty: false, problem: app.has_problem(PAL) } }).collect()
    }
    fn current(&self) -> Option<String> {
        None
    }
    fn open(&mut self, app: &mut App, id: &str) {
        if let Some(i) = app.pal.colours.iter().position(|c| c.name == id) {
            self.select(app, i);
        }
    }
    fn commands(&self, _app: &App) -> Vec<Cmd> {
        vec![Cmd::new("colour.add", "Add a colour", "a new colour beside the chosen one"), Cmd::new("colour.remove", "Remove the colour", "only while no texture uses it")]
    }
    fn run(&mut self, app: &mut App, id: &str) {
        match id {
            "colour.add" => self.add(app),
            "colour.remove" => self.remove(app),
            _ => {}
        }
    }
    fn more_keys(&self) -> Vec<(&'static str, &'static str)> {
        vec![("Click", "choose a colour"), ("Drag a colour", "move it in the order")]
    }
    fn help(&self) -> String {
        "Palette: a click chooses a colour, a drag moves it; the inspector names and changes it, and every texture follows".into()
    }
    fn refresh(&mut self, app: &App) {
        if self.edit_from.is_none() {
            self.select(app, self.sel);
        }
    }

    fn work(&mut self, ui: &mut Ui, app: &mut App) {
        look::weak(ui, "A click chooses a colour; drag one to move it.");
        ui.add_space(GAP);
        let (mut rects, mut clicked) = (vec![], None);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(GAP);
                for (i, c) in app.pal.colours.iter().enumerate() {
                    let (r, resp) = ui.allocate_exact_size(Vec2::new(80.0, 98.0), Sense::click_and_drag());
                    let sw = Rect::from_min_size(r.min, Vec2::splat(80.0));
                    let p = ui.painter();
                    p.rect_filled(sw, 6.0, c32(c.rgb));
                    if i == self.sel {
                        p.rect_stroke(sw, 6.0, egui::Stroke::new(3.0, look::ACCENT), StrokeKind::Outside);
                    }
                    let g = p.layout_no_wrap(c.name.clone(), egui::FontId::proportional(look::SMALL), ui.visuals().text_color());
                    p.with_clip_rect(r).galley(Pos2::new(r.center().x - g.size().x.min(r.width()) / 2.0, r.max.y - g.size().y), g, Color32::WHITE);
                    let resp = look::tip(resp, &format!("{} {}: a click chooses it, a drag moves it", c.name, to_hex(c.rgb)), "");
                    if resp.clicked() {
                        clicked = Some(i);
                    }
                    if resp.drag_started() {
                        self.moving = Some(i);
                    }
                    rects.push(r);
                }
            });
        });
        if let Some(i) = clicked {
            self.select(app, i);
        }
        let Some(from) = self.moving else { return };
        let pos = ui.input(|i| i.pointer.interact_pos());
        let to = pos.and_then(|p| rects.iter().position(|r| r.contains(p)));
        if let Some(t) = to {
            ui.painter().rect_stroke(rects[t].expand(4.0), 6.0, egui::Stroke::new(2.0, look::ACCENT), StrokeKind::Outside);
        }
        if ui.input(|i| i.pointer.any_released()) {
            self.moving = None;
            if let Some(t) = to.filter(|t| *t != from) {
                let c = app.pal.colours.remove(from);
                app.pal.colours.insert(t, c);
                self.select(app, t);
                app.save_palette(false);
            }
        }
    }

    fn inspector(&mut self, ui: &mut Ui, app: &mut App) {
        look::section(ui, "Colour");
        if app.pal.colours.is_empty() {
            ui.label("The palette is empty.");
            if ui.button("Add").clicked() {
                self.add(app);
            }
            return;
        }
        let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 48.0), Sense::hover());
        ui.painter().rect_filled(r, 6.0, c32(app.pal.colours[self.sel].rgb));
        egui::Grid::new("colour fields").num_columns(2).spacing(Vec2::new(GAP, GAP * 0.75)).show(ui, |ui| {
            ui.label("Name");
            let r = look::tip(ui.add(egui::TextEdit::singleline(&mut self.name).desired_width(f32::INFINITY)), "The colour's name: lower case, unique; Enter keeps it", "");
            if r.lost_focus() && self.name != app.pal.colours[self.sel].name {
                match app.pal.check(&self.name, app.pal.colours[self.sel].rgb, Some(self.sel)) {
                    Ok(()) => {
                        app.pal.colours[self.sel].name = self.name.clone();
                        app.save_palette(false);
                    }
                    Err(e) => {
                        app.refuse(e);
                        self.name = app.pal.colours[self.sel].name.clone();
                    }
                }
            }
            ui.end_row();
            ui.label("Hex");
            let r = look::tip(ui.add(egui::TextEdit::singleline(&mut self.hex).desired_width(f32::INFINITY)), "The colour as #rrggbb; Enter sets it and repaints every texture that uses it", "");
            if r.lost_focus() {
                match hex(self.hex.trim()) {
                    Some(rgb) => {
                        self.set_colour(app, rgb);
                        self.flush(app);
                    }
                    None => app.refuse(format!("'{}' is not #rrggbb", self.hex)),
                }
                self.select(app, self.sel);
            }
            ui.end_row();
        });
        ui.add_space(GAP * 0.5);
        ui.spacing_mut().slider_width = (ui.available_width() - 90.0).max(80.0);
        for (k, label) in ["hue", "saturation", "value"].into_iter().enumerate() {
            let r = ui.add(egui::Slider::new(&mut self.hsv[k], 0.0..=1.0).text(label).show_value(false));
            let r = look::tip(r, &format!("The colour's {label}; every texture using it follows as it moves"), "");
            if r.changed() {
                let rgb = egui::ecolor::Hsva::new(self.hsv[0], self.hsv[1], self.hsv[2], 1.0).to_rgb().map(|v| (v * 255.0).round().clamp(0.0, 255.0) as u8);
                self.set_colour(app, rgb);
            }
            if r.drag_stopped() || (r.changed() && !r.dragged()) {
                self.flush(app);
            }
        }
        let uses = Self::uses(app, app.pal.colours[self.sel].rgb);
        ui.add_space(GAP * 0.5);
        ui.label(RichText::new(format!("Used by {} file(s)", uses.len())).small());
        for u in &uses {
            look::weak(ui, u);
        }
        ui.add_space(GAP);
        ui.horizontal(|ui| {
            if look::tip(ui.button("Add"), "A new colour beside the chosen one", "").clicked() {
                self.add(app);
            }
            let r = ui.add_enabled(uses.is_empty(), egui::Button::new("Remove"));
            if look::tip(r, "Remove the colour; greyed while a texture uses it", "").clicked() {
                self.remove(app);
            }
        });
    }
}
