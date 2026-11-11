//! The Textures page (docs/design/studio.md "The window's layout", studio-tools.md "Textures: the tools"): a
//! block's faces and variants on a 16 x 16 canvas, the tools, the palette and the tiled preview.
use super::look::{self, c32, dim, draw, GAP};
use super::{App, Cmd, FileRow, Page, Thumb, TEX};
use crate::art::{self, Doc, Img, S};
use crate::shell::Flags;
use eframe::egui::{self, Color32, Key, Modifiers, PointerButton, Pos2, Rect, RichText, Sense, StrokeKind, Ui, Vec2};
use engine::palette::to_hex;

const FACE_NAMES: [&str; 4] = ["All", "Top", "Side", "Bottom"];

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Tool {
    Pencil,
    Fill,
    Picker,
    Line,
}

/// Each tool: its id, name, key, and its help for the status line.
const TOOLS: [(Tool, &str, &str, Key, &str); 4] = [
    (Tool::Pencil, "pencil", "Pencil", Key::B, "Pencil: left paints the first colour, right the second; Alt picks"),
    (Tool::Fill, "fill", "Fill", Key::G, "Fill: fills the touching area of one colour; left the first colour, right the second"),
    (Tool::Picker, "picker", "Picker", Key::I, "Picker: left takes the first colour from the canvas, right the second"),
    (Tool::Line, "line", "Line", Key::L, "Line: drag a straight line; left the first colour, right the second"),
];

struct Stroke {
    button: PointerButton,
    start: (usize, usize),
    last: (usize, usize),
    base: Img,
}

pub struct Textures {
    block: String,
    face: usize,
    variant: usize,
    tool: Tool,
    mirror: bool,
    first: [u8; 3],
    second: [u8; 3],
    zoom: Option<f32>,
    pan: Vec2,
    stroke: Option<Stroke>,
}

impl Textures {
    pub fn new(app: &App) -> Textures {
        let at = |i: usize| app.pal.colours.get(i).or(app.pal.colours.first()).map_or([128; 3], |c| c.rgb);
        let block = app.reg.placeable().next().map_or(String::new(), |d| d.name.clone());
        let mut t = Textures { block: String::new(), face: 0, variant: 0, tool: Tool::Pencil, mirror: false, first: at(0), second: at(1), zoom: None, pan: Vec2::ZERO, stroke: None };
        t.go(app, &block);
        t
    }

    fn file(&self, face: usize) -> String {
        art::file_name(&self.block, face)
    }

    fn go(&mut self, app: &App, block: &str) {
        self.block = block.to_string();
        self.face = (0..4).find(|&f| app.docs.contains_key(&self.file(f))).unwrap_or(0);
        (self.variant, self.zoom, self.pan, self.stroke) = (0, None, Vec2::ZERO, None);
    }

    /// The picture a face shows: its own file, else the block's every-face file (dimmed, `false`).
    fn shown<'a>(&self, app: &'a App) -> Option<(&'a Img, bool)> {
        match app.docs.get(&self.file(self.face)) {
            Some(d) => Some((&d.img, true)),
            None => app.docs.get(&self.file(0)).map(|d| (&d.img, false)),
        }
    }
    fn block_colour(&self, app: &App) -> [u8; 3] {
        app.reg.placeable().find(|d| d.name == self.block).map_or([0.5; 3], |d| d.colour).map(|v| (v * 255.0).round() as u8)
    }

    /// The face's own file, made from what it shows (or the palette colour nearest the block's) when it has none.
    fn doc<'a>(&self, app: &'a mut App) -> &'a mut Doc {
        let key = self.file(self.face);
        if !app.docs.contains_key(&key) {
            let img = match self.shown(app) {
                Some((img, _)) => img.clone(),
                None => {
                    let c = self.block_colour(app);
                    let d = |p: &engine::palette::Colour| p.rgb.iter().zip(c).map(|(a, b)| (*a as i32 - b as i32).pow(2)).sum::<i32>();
                    Img::filled(1, app.pal.colours.iter().min_by_key(|p| d(p)).map_or(c, |p| p.rgb))
                }
            };
            app.docs.insert(key.clone(), Doc { img, hist: Default::default(), saved: None });
        }
        app.docs.get_mut(&key).unwrap()
    }

    fn v(&self, app: &App) -> usize {
        self.shown(app).map_or(0, |(img, _)| self.variant.min(img.variants() - 1))
    }

    fn colour_name(app: &App, c: [u8; 3]) -> String {
        app.pal.find(c).map_or(format!("{} (not in the palette)", to_hex(c)), |i| app.pal.colours[i].name.clone())
    }

    fn tabs(&mut self, ui: &mut Ui, app: &mut App) {
        ui.horizontal(|ui| {
            for f in 0..4 {
                let own = app.docs.contains_key(&self.file(f));
                let t = if own { RichText::new(FACE_NAMES[f]) } else { RichText::new(FACE_NAMES[f]).weak() };
                let help = match f {
                    0 => "Every face: <block>.png".to_string(),
                    _ => format!("Only the {} face: <block>_{}.png{}", FACE_NAMES[f].to_lowercase(), art::FACES[f], if own { "" } else { "; dimmed: it falls back to All" }),
                };
                if look::tip(ui.selectable_label(self.face == f, t), &help, "").clicked() {
                    (self.face, self.variant, self.stroke) = (f, 0, None);
                }
            }
            ui.separator();
            let n = self.shown(app).map_or(1, |(img, _)| img.variants());
            for v in 0..n {
                let r = ui.selectable_label(self.variant == v, format!("{}", v + 1));
                if look::tip(r, &format!("Variant {}: the game picks one per block by its place", v + 1), &format!("{}", v + 1)).clicked() {
                    self.variant = v;
                }
            }
            if n < 4 && look::tip(ui.button("+"), "A new variant, copied from this one", "").clicked() {
                self.run(app, "variant.add");
            }
            ui.separator();
            let note = match self.shown(app) {
                Some((_, true)) => self.file(self.face),
                Some((_, false)) => format!("no {} yet: shows {}", self.file(self.face), self.file(0)),
                None => format!("no {} yet: the block keeps its pattern", self.file(self.face)),
            };
            ui.add(egui::Label::new(RichText::new(note).small().weak()).truncate());
        });
    }

    fn paint(&mut self, ui: &Ui, app: &mut App, hover: Option<(usize, usize)>, clamped: Option<(usize, usize)>) {
        let (alt, pressed) = ui.input(|i| (i.modifiers.alt, [PointerButton::Primary, PointerButton::Secondary].map(|b| i.pointer.button_pressed(b))));
        let tool = if alt { Tool::Picker } else { self.tool };
        let (v, mirror) = (self.v(app), self.mirror);
        if let Some(px) = hover.filter(|_| self.stroke.is_none()) {
            for (k, button) in [PointerButton::Primary, PointerButton::Secondary].into_iter().enumerate() {
                if !pressed[k] {
                    continue;
                }
                let c = if k == 0 { self.first } else { self.second };
                match tool {
                    Tool::Picker => {
                        let got = self.shown(app).map(|(img, _)| img.get(v.min(img.variants() - 1), px.0, px.1));
                        if let Some(got) = got.filter(|g| app.pal.find(*g).is_some()) {
                            *(if k == 0 { &mut self.first } else { &mut self.second }) = got;
                        }
                    }
                    Tool::Fill => {
                        let d = self.doc(app);
                        d.hist.push(d.img.clone(), "Fill");
                        art::fill(&mut d.img, v, px.0, px.1, c, mirror);
                    }
                    Tool::Pencil | Tool::Line => {
                        let d = self.doc(app);
                        let base = d.img.clone();
                        d.hist.push(base.clone(), if tool == Tool::Line { "Line" } else { "Pencil" });
                        art::pencil(&mut d.img, v, px.0, px.1, c, mirror);
                        self.stroke = Some(Stroke { button, start: px, last: px, base });
                    }
                }
            }
        }
        let Some(st) = &self.stroke else { return };
        let (button, start, last) = (st.button, st.start, st.last);
        if !ui.input(|i| i.pointer.button_down(button)) {
            self.stroke = None;
            return;
        }
        let Some(to) = clamped else { return };
        let c = if button == PointerButton::Primary { self.first } else { self.second };
        let base = (tool == Tool::Line).then(|| st.base.clone());
        let d = self.doc(app);
        match base {
            Some(base) => {
                d.img = base;
                art::line(&mut d.img, v, start, to, c, mirror);
            }
            None => art::line(&mut d.img, v, last, to, c, mirror),
        }
        self.stroke.as_mut().unwrap().last = to;
    }

    fn palette(&mut self, ui: &mut Ui, app: &App) {
        ui.horizontal(|ui| {
            for (k, c) in [(0, self.first), (1, self.second)] {
                let (r, resp) = ui.allocate_exact_size(Vec2::new(36.0, 36.0), Sense::hover());
                ui.painter().rect_filled(r, 4.0, c32(c));
                ui.painter().rect_stroke(r, 4.0, egui::Stroke::new(1.0, Color32::from_gray(90)), StrokeKind::Inside);
                look::tip(resp, if k == 0 { "First colour: the left button paints it" } else { "Second colour: the right button paints it" }, "");
            }
            ui.vertical(|ui| {
                ui.label(RichText::new(Self::colour_name(app, self.first)).small());
                ui.label(RichText::new(format!("right: {}", Self::colour_name(app, self.second))).small().weak());
            });
        });
        ui.add_space(GAP * 0.5);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(4.0);
            for c in &app.pal.colours {
                let (r, resp) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
                let p = ui.painter();
                p.rect_filled(r, 3.0, c32(c.rgb));
                if c.rgb == self.second {
                    p.rect_stroke(r.shrink(4.0), 1.0, egui::Stroke::new(2.0, Color32::from_gray(20)), StrokeKind::Inside);
                    p.rect_stroke(r.shrink(5.0), 1.0, egui::Stroke::new(1.0, Color32::WHITE), StrokeKind::Inside);
                }
                if c.rgb == self.first {
                    p.rect_stroke(r, 3.0, egui::Stroke::new(2.0, look::ACCENT), StrokeKind::Outside);
                }
                let resp = look::tip(resp, &format!("{}: left click makes it the first colour, right click the second", c.name), "");
                if resp.clicked() {
                    self.first = c.rgb;
                }
                if resp.secondary_clicked() {
                    self.second = c.rgb;
                }
            }
        });
    }

    fn preview(&self, ui: &mut Ui, app: &App) {
        let Some((img, _)) = self.shown(app) else {
            look::weak(ui, "nothing painted yet");
            return;
        };
        let scale = (ui.available_width() / (3 * S) as f32).floor().clamp(1.0, 6.0);
        let (r, _) = ui.allocate_exact_size(Vec2::splat(3.0 * S as f32 * scale), Sense::hover());
        for j in 0..3 {
            for i in 0..3 {
                let at = r.min + Vec2::new(i as f32, j as f32) * S as f32 * scale;
                draw(ui.painter(), img, art::variant_at(i, j, img.variants()), at, scale, false);
            }
        }
    }
}

impl Page for Textures {
    fn name(&self) -> &'static str {
        "Textures"
    }

    fn files(&self, app: &App) -> Vec<FileRow> {
        app.reg
            .placeable()
            .map(|d| {
                let files: Vec<String> = (0..4).map(|f| art::file_name(&d.name, f)).collect();
                let docs: Vec<&Doc> = files.iter().filter_map(|f| app.docs.get(f)).collect();
                let thumb = match docs.first() {
                    Some(doc) => Thumb::Img(doc.img.clone()),
                    None => Thumb::Flat(dim(d.colour.map(|v| (v * 255.0).round() as u8))),
                };
                let flags = Flags {
                    made: docs.iter().any(|d| d.saved.is_some()),
                    draft: files.iter().any(|f| app.is_draft(&format!("{TEX}/{f}"))),
                    dirty: docs.iter().any(|d| d.dirty()),
                    problem: files.iter().any(|f| app.has_problem(&format!("{TEX}/{f}"))),
                };
                FileRow { id: d.name.clone(), label: d.name.clone(), thumb, flags }
            })
            .collect()
    }
    fn current(&self) -> Option<String> {
        Some(self.block.clone())
    }
    fn open(&mut self, app: &mut App, id: &str) {
        if app.reg.placeable().any(|d| d.name == id) {
            self.go(app, id);
        }
    }

    fn commands(&self, _app: &App) -> Vec<Cmd> {
        let mut v: Vec<Cmd> = TOOLS.iter().map(|(_, id, name, k, help)| Cmd::new(format!("tool.{id}"), format!("Tool: {name}"), *help).key(Modifiers::NONE, *k)).collect();
        v.push(Cmd::new("mirror", "Mirror on or off", "paint both halves at once, mirrored across the middle").key(Modifiers::NONE, Key::M));
        for (n, k) in [Key::Num1, Key::Num2, Key::Num3, Key::Num4].into_iter().enumerate() {
            v.push(Cmd::new(format!("variant.{}", n + 1), format!("Variant {}", n + 1), "show and paint this variant").key(Modifiers::NONE, k));
        }
        v.push(Cmd::new("variant.add", "Add a variant", "a new variant (up to 4), copied from this one"));
        for (f, name) in FACE_NAMES.iter().enumerate() {
            v.push(Cmd::new(format!("face.{f}"), format!("Face: {name}"), "paint this face's own file"));
        }
        v.push(Cmd::new("undo", "Undo", "undo the last change to this file").key(Modifiers::COMMAND, Key::Z));
        v.push(Cmd::new("redo", "Redo", "redo what was undone").key(Modifiers::COMMAND, Key::Y));
        v.push(Cmd::new("save", "Save the block", "write the block's unsaved files and take them off the drafts list").key(Modifiers::COMMAND, Key::S));
        v.push(Cmd::new("zoom.fit", "Zoom to fit", "the canvas as large as fits, centred"));
        v
    }

    fn run(&mut self, app: &mut App, id: &str) {
        if let Some(t) = id.strip_prefix("tool.").and_then(|t| TOOLS.iter().find(|x| x.1 == t)) {
            self.tool = t.0;
            return;
        }
        if let Some(n) = id.strip_prefix("variant.").and_then(|n| n.parse::<usize>().ok()) {
            if self.shown(app).is_some_and(|(img, _)| n >= 1 && n <= img.variants()) {
                self.variant = n - 1;
            }
            return;
        }
        if let Some(f) = id.strip_prefix("face.").and_then(|n| n.parse::<usize>().ok()).filter(|f| *f < 4) {
            (self.face, self.variant, self.stroke) = (f, 0, None);
            return;
        }
        match id {
            "mirror" => self.mirror = !self.mirror,
            "variant.add" => {
                let v = self.v(app);
                let n = self.shown(app).map_or(1, |(img, _)| img.variants());
                let d = self.doc(app);
                if let Some(wider) = art::add_variant(&d.img, v) {
                    d.hist.push(std::mem::replace(&mut d.img, wider), "Add a variant");
                    self.variant = n;
                } else {
                    app.refuse("A texture holds at most 4 variants (64 px wide)");
                }
            }
            "undo" | "redo" if self.stroke.is_none() => {
                if let Some(d) = app.docs.get_mut(&self.file(self.face)) {
                    let done = if id == "undo" { d.hist.undo(&mut d.img) } else { d.hist.redo(&mut d.img) };
                    self.variant = self.variant.min(d.img.variants() - 1);
                    if !done {
                        app.toast(format!("Nothing to {id}"));
                    }
                }
            }
            "save" => {
                let files: Vec<String> = (0..4).map(|f| self.file(f)).filter(|f| app.docs.get(f).is_some_and(Doc::dirty)).collect();
                if files.is_empty() {
                    app.toast(format!("{}: nothing to save", self.block));
                }
                app.save_textures(files);
            }
            "zoom.fit" => (self.zoom, self.pan) = (None, Vec2::ZERO),
            _ => {}
        }
    }

    fn more_keys(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("Left / right", "paint the first / second colour"),
            ("Alt held", "the picker, whatever tool is chosen"),
            ("Wheel", "zoom the canvas"),
            ("Middle drag", "move the canvas"),
            ("Right click a swatch", "make it the second colour"),
        ]
    }

    fn help(&self) -> String {
        let t = TOOLS.iter().find(|t| t.0 == self.tool).unwrap();
        format!("{}   [{}]{}", t.4, t.3.name(), if self.mirror { "   · mirror on [M]" } else { "" })
    }

    fn history(&self, app: &App) -> Option<(Vec<String>, Vec<String>)> {
        let d = app.docs.get(&self.file(self.face));
        Some(d.map_or((vec![], vec![]), |d| (d.hist.undo.iter().map(|s| s.name.clone()).collect(), d.hist.redo.iter().rev().map(|s| s.name.clone()).collect())))
    }
    fn back_to(&mut self, app: &mut App, n: usize) {
        if let Some(d) = app.docs.get_mut(&self.file(self.face)) {
            d.hist.back_to(&mut d.img, n);
            self.variant = self.variant.min(d.img.variants() - 1);
        }
    }
    fn refresh(&mut self, app: &App) {
        self.variant = self.v(app);
        if self.pal_missing(app, self.first) {
            self.first = app.pal.colours.first().map_or([128; 3], |c| c.rgb);
        }
        if self.pal_missing(app, self.second) {
            self.second = app.pal.colours.get(1).or(app.pal.colours.first()).map_or([128; 3], |c| c.rgb);
        }
    }

    fn work(&mut self, ui: &mut Ui, app: &mut App) {
        self.refresh(app);
        self.tabs(ui, app);
        ui.add_space(GAP * 0.5);
        let avail = ui.available_size();
        let (rect, resp) = ui.allocate_exact_size(avail.max(Vec2::splat(16.0)), Sense::click_and_drag());
        let fit = ((rect.width().min(rect.height()) - 2.0 * GAP) / S as f32).floor().max(1.0);
        let scale = self.zoom.unwrap_or(fit);
        let at = (rect.center() - Vec2::splat(S as f32 * scale / 2.0) + self.pan).round();
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 6.0, ui.visuals().extreme_bg_color);
        let face = Rect::from_min_size(at, Vec2::splat(S as f32 * scale));
        match self.shown(app) {
            Some((img, own)) => draw(&p, img, self.v(app), at, scale, !own),
            None => {
                p.rect_filled(face, 0.0, dim(self.block_colour(app)));
                p.text(face.center(), egui::Align2::CENTER_CENTER, "unpainted: paint to start", egui::FontId::proportional(look::BODY), Color32::from_gray(230));
            }
        }
        if scale >= 8.0 {
            let grid = egui::Stroke::new(1.0, Color32::from_black_alpha(60));
            for i in 1..S {
                let o = i as f32 * scale;
                p.line_segment([Pos2::new(face.min.x + o, face.min.y), Pos2::new(face.min.x + o, face.max.y)], grid);
                p.line_segment([Pos2::new(face.min.x, face.min.y + o), Pos2::new(face.max.x, face.min.y + o)], grid);
            }
        }
        p.rect_stroke(face, 0.0, egui::Stroke::new(1.0, Color32::from_gray(90)), StrokeKind::Outside);
        if resp.dragged_by(PointerButton::Middle) {
            self.pan += resp.drag_delta();
        }
        if resp.hovered() {
            let wheel: f32 = ui.input(|i| i.events.iter().map(|e| if let egui::Event::MouseWheel { delta, .. } = e { delta.y } else { 0.0 }).sum());
            if wheel != 0.0 {
                self.zoom = Some((scale + wheel.signum()).clamp(1.0, 64.0));
            }
        }
        // the pixel under the pointer, clamped while a stroke runs
        let pos = ui.input(|i| i.pointer.hover_pos());
        let cell = |q: Pos2| ((q - at) / scale).floor();
        let hover = pos.map(cell).filter(|c| (0.0..S as f32).contains(&c.x) && (0.0..S as f32).contains(&c.y) && rect.contains(pos.unwrap())).map(|c| (c.x as usize, c.y as usize));
        let clamped = pos.map(cell).map(|c| (c.x.clamp(0.0, 15.0) as usize, c.y.clamp(0.0, 15.0) as usize));
        if !app.pal.colours.is_empty() {
            self.paint(ui, app, hover, clamped);
        }
        app.pointer = match (hover, self.shown(app)) {
            (Some((x, y)), Some((img, _))) => format!("{x}, {y}   {}", Self::colour_name(app, img.get(self.v(app), x, y))),
            (Some((x, y)), None) => format!("{x}, {y}"),
            _ => format!("{}  ×{}", self.file(self.face), scale),
        };
    }

    fn inspector(&mut self, ui: &mut Ui, app: &mut App) {
        look::section(ui, "Tools");
        let cmds = self.commands(app);
        ui.horizontal_wrapped(|ui| {
            for (t, id, name, k, help) in TOOLS {
                let r = ui.selectable_label(self.tool == t, format!("{name}  {}", k.name()));
                if look::tip(r, help, k.name()).clicked() {
                    self.tool = t;
                }
                let _ = id;
            }
            let r = ui.toggle_value(&mut self.mirror, "Mirror  M");
            look::tip(r, "Paint both halves at once, mirrored across the middle", "M");
        });
        ui.horizontal(|ui| {
            for id in ["undo", "redo", "save"] {
                let c = cmds.iter().find(|c| c.id == id).unwrap();
                if look::tip(ui.button(&c.label), &c.help, &c.keys()).clicked() {
                    self.run(app, id);
                }
            }
        });
        ui.separator();
        look::section(ui, "Colours");
        self.palette(ui, app);
        ui.separator();
        look::section(ui, "Preview");
        self.preview(ui, app);
    }
}

impl Textures {
    fn pal_missing(&self, app: &App, c: [u8; 3]) -> bool {
        app.pal.find(c).is_none()
    }
}
