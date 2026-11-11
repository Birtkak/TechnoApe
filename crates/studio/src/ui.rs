//! The Studio's window (docs/design/studio.md "The window's layout"): the Textures page (files, canvas, tools,
//! palette, preview) and the Palette page. The editing itself is `art`'s; this file is only the interface.
use crate::art::{self, History, Img, S};
use eframe::egui::{self, Color32, Id, Key, Modifiers, PointerButton, Pos2, Rect, RichText, Sense, StrokeKind, Ui, Vec2};
use engine::blocks::Registry;
use engine::palette::{hex, to_hex, Colour, Palette};
use std::collections::BTreeMap;

pub const TEX: &str = "data/textures";
pub const PAL: &str = "data/palette.toml";
pub const DRAFTS: &str = "data/drafts.txt";
const FACE_NAMES: [&str; 4] = ["All", "Top", "Side", "Bottom"];

#[derive(PartialEq, Clone, Copy)]
pub enum Page {
    Textures,
    Palette,
}
#[derive(PartialEq, Clone, Copy)]
enum Tool {
    Pencil,
    Fill,
    Picker,
    Line,
}

struct Doc {
    img: Img,
    hist: History,
    /// The file as it is on disk; none for a file not yet saved.
    saved: Option<Img>,
}
impl Doc {
    fn dirty(&self) -> bool {
        self.saved.as_ref() != Some(&self.img)
    }
}

struct Stroke {
    button: PointerButton,
    start: (usize, usize),
    last: (usize, usize),
    base: Img,
}

pub struct Studio {
    pub page: Page,
    reg: Registry,
    pal: Palette,
    /// Every texture by file name: those on disk and the new ones.
    docs: BTreeMap<String, Doc>,
    drafts: Vec<String>,
    search: String,
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
    status: String,
    // the Palette page
    sel: usize,
    name: String,
    hex: String,
    hsv: [f32; 3],
    moving: Option<usize>,
    /// The colour a slider drag started from, written out when it stops.
    edit_from: Option<[u8; 3]>,
    closing: bool,
    quit: bool,
}

fn c32(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}
fn dim(c: [u8; 3]) -> Color32 {
    let g = (c[0] as u16 + c[1] as u16 + c[2] as u16) / 3;
    let m = |v: u8| ((v as u16 + g * 2) / 3 / 2 + 40) as u8;
    Color32::from_rgb(m(c[0]), m(c[1]), m(c[2]))
}

fn draw(p: &egui::Painter, img: &Img, v: usize, at: Pos2, scale: f32, dimmed: bool) {
    for y in 0..S {
        for x in 0..S {
            let c = img.get(v, x, y);
            let r = Rect::from_min_size(at + Vec2::new(x as f32, y as f32) * scale, Vec2::splat(scale));
            p.rect_filled(r, 0.0, if dimmed { dim(c) } else { c32(c) });
        }
    }
}

impl Studio {
    /// Reads `data/` in the working folder: the blocks, the palette, every texture and the drafts list.
    pub fn load(open: Option<&str>) -> Result<Studio, String> {
        let reg = Registry::parse(&std::fs::read_to_string("data/blocks.toml").map_err(|e| format!("data/blocks.toml: {e}"))?).map_err(|e| format!("data/blocks.toml: {e}"))?;
        let pal = Palette::parse(&std::fs::read_to_string(PAL).map_err(|e| format!("{PAL}: {e}"))?).map_err(|e| format!("{PAL}: {e}"))?;
        let mut docs = BTreeMap::new();
        let mut refused = vec![];
        for n in art::pngs(std::path::Path::new(TEX)) {
            match std::fs::read(format!("{TEX}/{n}")).map_err(|e| e.to_string()).and_then(|b| art::decode(&b)) {
                Ok(img) => {
                    docs.insert(n, Doc { img: img.clone(), hist: History::default(), saved: Some(img) });
                }
                Err(e) => refused.push(format!("{n}: {e}")),
            }
        }
        let block = match open {
            Some(b) => reg.placeable().find(|d| d.name == b).ok_or(format!("no block '{b}' in data/blocks.toml"))?.name.clone(),
            None => reg.placeable().next().ok_or("data/blocks.toml has no blocks")?.name.clone(),
        };
        let at = |i: usize| pal.colours.get(i).or(pal.colours.first()).map_or([128; 3], |c| c.rgb);
        let (first, second) = (at(0), at(1));
        let mut s = Studio {
            page: Page::Textures,
            reg,
            drafts: art::drafts(&std::fs::read_to_string(DRAFTS).unwrap_or_default()),
            docs,
            search: String::new(),
            block,
            face: 0,
            variant: 0,
            tool: Tool::Pencil,
            mirror: false,
            first,
            second,
            zoom: None,
            pan: Vec2::ZERO,
            stroke: None,
            status: if refused.is_empty() { String::new() } else { format!("refused: {}", refused.join("; ")) },
            sel: 0,
            name: String::new(),
            hex: String::new(),
            hsv: [0.0; 3],
            moving: None,
            edit_from: None,
            closing: false,
            quit: false,
            pal,
        };
        s.select(0);
        s.open(&s.block.clone());
        Ok(s)
    }

    pub fn show(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        if self.page == Page::Textures && !ctx.egui_wants_keyboard_input() {
            self.keys(&ctx);
        }
        egui::Panel::top("pages").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.page, Page::Textures, "Textures");
                ui.selectable_value(&mut self.page, Page::Palette, "Palette");
                ui.separator();
                ui.label(RichText::new(&self.status).weak());
            });
        });
        match self.page {
            Page::Textures => {
                egui::Panel::left("files").exact_size(190.0).resizable(false).show(ui, |ui| self.files(ui));
                egui::Panel::right("tools").exact_size(250.0).resizable(false).show(ui, |ui| self.tools(ui));
                egui::CentralPanel::default().show(ui, |ui| self.canvas(ui));
            }
            Page::Palette => {
                egui::Panel::right("colour").exact_size(280.0).resizable(false).show(ui, |ui| self.colour(ui));
                egui::CentralPanel::default().show(ui, |ui| self.swatches(ui));
            }
        }
        self.close_check(&ctx);
    }

    // ---- the files and their faces

    fn file(&self, face: usize) -> String {
        art::file_name(&self.block, face)
    }
    fn is_draft(&self, file: &str) -> bool {
        self.drafts.iter().any(|d| *d == format!("{TEX}/{file}"))
    }

    /// The picture a face shows: its own file, else the block's every-face file (dimmed, `false`).
    fn shown(&self) -> Option<(&Img, bool)> {
        match self.docs.get(&self.file(self.face)) {
            Some(d) => Some((&d.img, true)),
            None => self.docs.get(&self.file(0)).map(|d| (&d.img, false)),
        }
    }

    fn open(&mut self, block: &str) {
        self.block = block.to_string();
        self.face = (0..4).find(|&f| self.docs.contains_key(&self.file(f))).unwrap_or(0);
        (self.variant, self.zoom, self.pan, self.stroke) = (0, None, Vec2::ZERO, None);
    }

    /// The face's own file, made from what it shows (or the palette colour nearest the block's) when it has none.
    fn doc(&mut self) -> &mut Doc {
        let key = self.file(self.face);
        if !self.docs.contains_key(&key) {
            let img = match self.shown() {
                Some((img, _)) => img.clone(),
                None => {
                    let c = self.reg.placeable().find(|d| d.name == self.block).map_or([0.5; 3], |d| d.colour).map(|v| (v * 255.0).round() as u8);
                    let d = |p: &Colour| p.rgb.iter().zip(c).map(|(a, b)| (*a as i32 - b as i32).pow(2)).sum::<i32>();
                    Img::filled(1, self.pal.colours.iter().min_by_key(|p| d(p)).map_or(c, |p| p.rgb))
                }
            };
            self.docs.insert(key.clone(), Doc { img, hist: History::default(), saved: None });
        }
        self.docs.get_mut(&key).unwrap()
    }

    fn save_files(&mut self, files: Vec<String>) {
        let _ = std::fs::create_dir_all(TEX);
        let mut drafts = std::fs::read_to_string(DRAFTS).unwrap_or_default();
        for f in files {
            let d = self.docs.get_mut(&f).unwrap();
            match art::save(std::path::Path::new(&format!("{TEX}/{f}")), &d.img, d.saved.is_none()) {
                Ok(()) => {
                    d.saved = Some(d.img.clone());
                    drafts = art::set_draft(&drafts, &format!("{TEX}/{f}"), false);
                    self.status = format!("saved {TEX}/{f}");
                }
                Err(e) => self.status = format!("{TEX}/{f} not saved: {e}"),
            }
        }
        self.write_drafts(drafts);
    }
    fn write_drafts(&mut self, text: String) {
        if art::drafts(&text) != self.drafts {
            if let Err(e) = std::fs::write(DRAFTS, &text) {
                self.status = format!("{DRAFTS}: {e}");
            }
            self.drafts = art::drafts(&text);
        }
    }
    fn save_block(&mut self) {
        let files = (0..4).map(|f| self.file(f)).filter(|f| self.docs.get(f).is_some_and(Doc::dirty)).collect();
        self.save_files(files);
    }

    fn keys(&mut self, ctx: &egui::Context) {
        let (mut undo, mut redo, mut save) = (false, false, false);
        ctx.input_mut(|i| {
            undo = i.consume_key(Modifiers::COMMAND, Key::Z);
            redo = i.consume_key(Modifiers::COMMAND, Key::Y);
            save = i.consume_key(Modifiers::COMMAND, Key::S);
            for (k, t) in [(Key::B, Tool::Pencil), (Key::G, Tool::Fill), (Key::I, Tool::Picker), (Key::L, Tool::Line)] {
                if i.consume_key(Modifiers::NONE, k) {
                    self.tool = t;
                }
            }
            if i.consume_key(Modifiers::NONE, Key::M) {
                self.mirror = !self.mirror;
            }
            for (n, k) in [Key::Num1, Key::Num2, Key::Num3, Key::Num4].into_iter().enumerate() {
                if i.consume_key(Modifiers::NONE, k) {
                    self.pick_variant(n);
                }
            }
        });
        if (undo || redo) && self.stroke.is_none() {
            if let Some(d) = self.docs.get_mut(&self.file(self.face)) {
                let done = if undo { d.hist.undo(&mut d.img) } else { d.hist.redo(&mut d.img) };
                self.variant = self.variant.min(d.img.variants() - 1);
                if !done {
                    self.status = format!("nothing to {}", if undo { "undo" } else { "redo" });
                }
            }
        }
        if save {
            self.save_block();
        }
    }

    fn pick_variant(&mut self, v: usize) {
        if self.shown().is_some_and(|(img, _)| v < img.variants()) {
            self.variant = v;
        }
    }

    fn files(&mut self, ui: &mut Ui) {
        ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("search"));
        ui.add_space(4.0);
        let q = self.search.to_lowercase();
        let names: Vec<(String, [f32; 3])> = self.reg.placeable().filter(|d| d.name.contains(&q)).map(|d| (d.name.clone(), d.colour)).collect();
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (name, colour) in names {
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 36.0), Sense::click());
                let p = ui.painter();
                if name == self.block {
                    p.rect_filled(rect, 3.0, ui.visuals().selection.bg_fill);
                } else if resp.hovered() {
                    p.rect_filled(rect, 3.0, ui.visuals().widgets.hovered.weak_bg_fill);
                }
                let files: Vec<String> = (0..4).map(|f| art::file_name(&name, f)).collect();
                let thumb = Rect::from_min_size(rect.min + Vec2::new(2.0, 2.0), Vec2::splat(32.0));
                match files.iter().find_map(|f| self.docs.get(f)) {
                    Some(d) => draw(p, &d.img, 0, thumb.min, 2.0, false),
                    None => {
                        p.rect_filled(thumb, 0.0, dim(colour.map(|v| (v * 255.0).round() as u8)));
                    }
                }
                let dirty = files.iter().any(|f| self.docs.get(f).is_some_and(Doc::dirty));
                let text = if dirty { format!("{name}  \u{25cf}") } else { name.clone() };
                p.text(Pos2::new(thumb.max.x + 8.0, rect.center().y), egui::Align2::LEFT_CENTER, text, egui::FontId::proportional(14.0), ui.visuals().text_color());
                if files.iter().any(|f| self.is_draft(f)) {
                    p.text(Pos2::new(rect.max.x - 4.0, rect.center().y), egui::Align2::RIGHT_CENTER, "draft", egui::FontId::proportional(11.0), Color32::from_rgb(230, 170, 60));
                }
                if resp.clicked() {
                    self.open(&name);
                }
            }
        });
    }

    // ---- the canvas

    fn canvas(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            for f in 0..4 {
                let own = self.docs.contains_key(&self.file(f));
                let t = if own { RichText::new(FACE_NAMES[f]) } else { RichText::new(FACE_NAMES[f]).weak() };
                if ui.selectable_label(self.face == f, t).clicked() {
                    (self.face, self.variant, self.stroke) = (f, 0, None);
                }
            }
            ui.separator();
            let note = match self.shown() {
                Some((_, true)) => format!("{}{}", self.file(self.face), if self.docs[&self.file(self.face)].dirty() { "  (not saved)" } else { "" }),
                Some((_, false)) => format!("no {} yet: shows {}", self.file(self.face), self.file(0)),
                None => format!("no {} yet: the block keeps its pattern", self.file(self.face)),
            };
            ui.label(RichText::new(note).weak());
        });
        let n = self.shown().map_or(1, |(img, _)| img.variants());
        ui.horizontal(|ui| {
            for v in 0..n {
                if ui.selectable_label(self.variant == v, format!("{}", v + 1)).clicked() {
                    self.variant = v;
                }
            }
            if n < 4 && ui.button("+").on_hover_text("a new variant, copied from this one").clicked() {
                let v = self.variant;
                let d = self.doc();
                if let Some(wider) = art::add_variant(&d.img, v) {
                    d.hist.push(std::mem::replace(&mut d.img, wider));
                    self.variant = n;
                }
            }
        });
        let avail = ui.available_size() - Vec2::new(0.0, 24.0);
        let (rect, resp) = ui.allocate_exact_size(avail.max(Vec2::splat(16.0)), Sense::click_and_drag());
        let fit = (rect.width().min(rect.height()) / S as f32).floor().max(1.0);
        let scale = self.zoom.unwrap_or(fit);
        let at = (rect.center() - Vec2::splat(S as f32 * scale / 2.0) + self.pan).round();
        let p = ui.painter_at(rect);
        let face = Rect::from_min_size(at, Vec2::splat(S as f32 * scale));
        match self.shown() {
            Some((img, own)) => draw(&p, img, self.variant.min(img.variants() - 1), at, scale, !own),
            None => {
                let c = self.reg.placeable().find(|d| d.name == self.block).map_or([0.5; 3], |d| d.colour).map(|v| (v * 255.0).round() as u8);
                p.rect_filled(face, 0.0, dim(c));
                p.text(face.center(), egui::Align2::CENTER_CENTER, "unpainted: paint to start", egui::FontId::proportional(16.0), Color32::from_gray(230));
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

        // zoom, pan
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
        if !self.pal.colours.is_empty() {
            self.paint(ui, hover, clamped);
        }
        let info = match (hover, self.shown()) {
            (Some((x, y)), Some((img, _))) => {
                let c = img.get(self.variant.min(img.variants() - 1), x, y);
                let name = self.pal.find(c).map_or(format!("{} (not in the palette)", to_hex(c)), |i| self.pal.colours[i].name.clone());
                format!("pixel {x}, {y}   {name}")
            }
            (Some((x, y)), None) => format!("pixel {x}, {y}"),
            _ => String::new(),
        };
        ui.label(info);
    }

    fn paint(&mut self, ui: &Ui, hover: Option<(usize, usize)>, clamped: Option<(usize, usize)>) {
        let (alt, pressed) = ui.input(|i| (i.modifiers.alt, [PointerButton::Primary, PointerButton::Secondary].map(|b| i.pointer.button_pressed(b))));
        let tool = if alt { Tool::Picker } else { self.tool };
        let (v, mirror) = (self.shown().map_or(0, |(img, _)| self.variant.min(img.variants() - 1)), self.mirror);
        if let Some(px) = hover.filter(|_| self.stroke.is_none()) {
            for (k, button) in [PointerButton::Primary, PointerButton::Secondary].into_iter().enumerate() {
                if !pressed[k] {
                    continue;
                }
                let c = if k == 0 { self.first } else { self.second };
                match tool {
                    Tool::Picker => {
                        let got = self.shown().map(|(img, _)| img.get(v.min(img.variants() - 1), px.0, px.1));
                        if let Some(got) = got.filter(|g| self.pal.find(*g).is_some()) {
                            *(if k == 0 { &mut self.first } else { &mut self.second }) = got;
                        }
                    }
                    Tool::Fill => {
                        let d = self.doc();
                        d.hist.push(d.img.clone());
                        art::fill(&mut d.img, v, px.0, px.1, c, mirror);
                    }
                    Tool::Pencil | Tool::Line => {
                        let d = self.doc();
                        let base = d.img.clone();
                        d.hist.push(base.clone());
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
        let d = self.doc();
        match base {
            Some(base) => {
                d.img = base;
                art::line(&mut d.img, v, start, to, c, mirror);
            }
            None => art::line(&mut d.img, v, last, to, c, mirror),
        }
        self.stroke.as_mut().unwrap().last = to;
    }

    // ---- tools, palette, preview

    fn tools(&mut self, ui: &mut Ui) {
        ui.heading("Tools");
        ui.horizontal_wrapped(|ui| {
            for (t, label) in [(Tool::Pencil, "Pencil  B"), (Tool::Fill, "Fill  G"), (Tool::Picker, "Picker  I"), (Tool::Line, "Line  L")] {
                ui.selectable_value(&mut self.tool, t, label);
            }
            ui.toggle_value(&mut self.mirror, "Mirror  M");
        });
        ui.horizontal(|ui| {
            if ui.button("Undo  Ctrl+Z").clicked() {
                if let Some(d) = self.docs.get_mut(&self.file(self.face)) {
                    d.hist.undo(&mut d.img);
                    self.variant = self.variant.min(d.img.variants() - 1);
                }
            }
            if ui.button("Redo  Ctrl+Y").clicked() {
                if let Some(d) = self.docs.get_mut(&self.file(self.face)) {
                    d.hist.redo(&mut d.img);
                }
            }
        });
        if ui.button("Save  Ctrl+S").clicked() {
            self.save_block();
        }
        ui.separator();
        ui.heading("Palette");
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(3.0);
            for c in &self.pal.colours {
                let (r, resp) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());
                let p = ui.painter();
                p.rect_filled(r, 2.0, c32(c.rgb));
                if c.rgb == self.second {
                    p.rect_stroke(r.shrink(3.0), 0.0, egui::Stroke::new(2.0, Color32::from_rgb(240, 200, 60)), StrokeKind::Inside);
                }
                if c.rgb == self.first {
                    p.rect_stroke(r, 2.0, egui::Stroke::new(2.0, Color32::WHITE), StrokeKind::Outside);
                    p.rect_stroke(r, 2.0, egui::Stroke::new(1.0, Color32::BLACK), StrokeKind::Inside);
                }
                let resp = resp.on_hover_text(&c.name);
                if resp.clicked() {
                    self.first = c.rgb;
                }
                if resp.secondary_clicked() {
                    self.second = c.rgb;
                }
            }
        });
        let name = |rgb| self.pal.find(rgb).map_or("-".into(), |i| self.pal.colours[i].name.clone());
        ui.label(format!("{}   (right: {})", name(self.first), name(self.second)));
        ui.separator();
        ui.heading("Preview");
        if let Some((img, _)) = self.shown() {
            let scale = (ui.available_width() / (3 * S) as f32).floor().clamp(1.0, 6.0);
            let (r, _) = ui.allocate_exact_size(Vec2::splat(3.0 * S as f32 * scale), Sense::hover());
            for j in 0..3 {
                for i in 0..3 {
                    let at = r.min + Vec2::new(i as f32, j as f32) * S as f32 * scale;
                    draw(ui.painter(), img, art::variant_at(i, j, img.variants()), at, scale, false);
                }
            }
        } else {
            ui.label(RichText::new("nothing painted yet").weak());
        }
    }

    // ---- the Palette page

    fn select(&mut self, i: usize) {
        self.sel = i.min(self.pal.colours.len().saturating_sub(1));
        if let Some(c) = self.pal.colours.get(self.sel) {
            self.name = c.name.clone();
            self.hex = to_hex(c.rgb);
            let h = egui::ecolor::Hsva::from_rgb(c.rgb.map(|v| v as f32 / 255.0));
            self.hsv = [h.h, h.s, h.v];
        }
    }

    fn uses(&self, rgb: [u8; 3]) -> Vec<&str> {
        self.docs.iter().filter(|(_, d)| d.img.uses(rgb) || d.saved.as_ref().is_some_and(|s| s.uses(rgb))).map(|(n, _)| n.as_str()).collect()
    }

    /// Writes the palette; it leaves the drafts list only when a colour itself was changed (a new order, name,
    /// added or removed colour keeps Claude's colours a draft).
    fn save_palette(&mut self, colour_changed: bool) {
        match std::fs::write(PAL, self.pal.write()) {
            Ok(()) if colour_changed => {
                let text = art::set_draft(&std::fs::read_to_string(DRAFTS).unwrap_or_default(), PAL, false);
                self.write_drafts(text);
            }
            Ok(()) => {}
            Err(e) => self.status = format!("{PAL} not saved: {e}"),
        }
    }

    /// Changes colour `sel` in the palette and in every texture in memory; `flush` writes it all out.
    fn set_colour(&mut self, rgb: [u8; 3]) {
        let old = self.pal.colours[self.sel].rgb;
        if old == rgb {
            return;
        }
        if let Err(e) = self.pal.check(&self.pal.colours[self.sel].name, rgb, Some(self.sel)) {
            self.status = e;
            return;
        }
        self.edit_from.get_or_insert(old);
        self.pal.colours[self.sel].rgb = rgb;
        self.hex = to_hex(rgb);
        for d in self.docs.values_mut() {
            for img in std::iter::once(&mut d.img).chain(d.saved.as_mut()).chain(d.hist.undo.iter_mut()).chain(d.hist.redo.iter_mut()) {
                art::repaint(img, old, rgb);
            }
        }
        for c in [&mut self.first, &mut self.second] {
            if *c == old {
                *c = rgb;
            }
        }
    }

    fn flush(&mut self) {
        let to = self.pal.colours[self.sel].rgb;
        // a slider let go where it started changed nothing: no file is rewritten and the palette stays a draft
        let Some(from) = self.edit_from.take().filter(|f| *f != to) else { return };
        self.save_palette(true);
        let (done, failed) = art::repaint_dir(std::path::Path::new(TEX), from, to);
        self.status = format!("{} {} -> {}: repainted {} file(s)", self.pal.colours[self.sel].name, to_hex(from), to_hex(to), done.len());
        for (n, why) in failed {
            // that file still holds the old colour on disk: it shows as not saved, so a save writes it
            if let Some(d) = self.docs.get_mut(&n) {
                d.saved = std::fs::read(format!("{TEX}/{n}")).ok().and_then(|b| art::decode(&b).ok());
            }
            self.status += &format!("; not written: {why}");
        }
    }

    fn swatches(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("A click selects; drag a colour to move it.").weak());
        ui.add_space(6.0);
        let (mut rects, mut clicked) = (vec![], None);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(8.0);
            for (i, c) in self.pal.colours.iter().enumerate() {
                let (r, resp) = ui.allocate_exact_size(Vec2::new(72.0, 90.0), Sense::click_and_drag());
                let sw = Rect::from_min_size(r.min, Vec2::splat(72.0));
                let p = ui.painter();
                p.rect_filled(sw, 4.0, c32(c.rgb));
                if i == self.sel {
                    p.rect_stroke(sw, 4.0, egui::Stroke::new(3.0, Color32::WHITE), StrokeKind::Outside);
                }
                p.text(Pos2::new(r.center().x, r.max.y - 7.0), egui::Align2::CENTER_CENTER, &c.name, egui::FontId::proportional(12.0), ui.visuals().text_color());
                if resp.clicked() {
                    clicked = Some(i);
                }
                if resp.drag_started() {
                    self.moving = Some(i);
                }
                rects.push(r);
            }
        });
        if let Some(i) = clicked {
            self.select(i);
        }
        let Some(from) = self.moving else { return };
        let pos = ui.input(|i| i.pointer.interact_pos());
        let to = pos.and_then(|p| rects.iter().position(|r| r.contains(p)));
        if let Some(t) = to {
            ui.painter().rect_stroke(rects[t].expand(4.0), 4.0, egui::Stroke::new(2.0, Color32::from_rgb(240, 200, 60)), StrokeKind::Outside);
        }
        if ui.input(|i| i.pointer.any_released()) {
            self.moving = None;
            if let Some(t) = to.filter(|t| *t != from) {
                let c = self.pal.colours.remove(from);
                self.pal.colours.insert(t, c);
                self.select(t);
                self.save_palette(false);
            }
        }
    }

    fn colour(&mut self, ui: &mut Ui) {
        ui.heading("Colour");
        if self.pal.colours.is_empty() {
            ui.label("The palette is empty.");
        } else {
            let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 48.0), Sense::hover());
            ui.painter().rect_filled(r, 4.0, c32(self.pal.colours[self.sel].rgb));
            ui.label("name");
            if ui.text_edit_singleline(&mut self.name).lost_focus() && self.name != self.pal.colours[self.sel].name {
                match self.pal.check(&self.name, self.pal.colours[self.sel].rgb, Some(self.sel)) {
                    Ok(()) => {
                        self.pal.colours[self.sel].name = self.name.clone();
                        self.save_palette(false);
                    }
                    Err(e) => {
                        self.status = e;
                        self.name = self.pal.colours[self.sel].name.clone();
                    }
                }
            }
            ui.label("hex");
            if ui.text_edit_singleline(&mut self.hex).lost_focus() {
                match hex(self.hex.trim()) {
                    Some(rgb) => {
                        self.set_colour(rgb);
                        self.flush();
                    }
                    None => self.status = format!("'{}' is not #rrggbb", self.hex),
                }
                self.select(self.sel);
            }
            ui.spacing_mut().slider_width = 120.0;
            for (k, label) in ["hue", "saturation", "value"].into_iter().enumerate() {
                let r = ui.add(egui::Slider::new(&mut self.hsv[k], 0.0..=1.0).text(label));
                if r.changed() {
                    let rgb = egui::ecolor::Hsva::new(self.hsv[0], self.hsv[1], self.hsv[2], 1.0).to_rgb().map(|v| (v * 255.0).round().clamp(0.0, 255.0) as u8);
                    self.set_colour(rgb);
                }
                if r.drag_stopped() || (r.changed() && !r.dragged()) {
                    self.flush();
                }
            }
            let uses: Vec<String> = self.uses(self.pal.colours[self.sel].rgb).into_iter().map(String::from).collect();
            ui.label(format!("used by {} file(s)", uses.len()));
            for u in &uses {
                ui.label(RichText::new(u).weak());
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Add").clicked() {
                    self.add();
                }
                if ui.add_enabled(uses.is_empty(), egui::Button::new("Remove")).on_disabled_hover_text("in use").clicked() {
                    self.pal.colours.remove(self.sel);
                    self.select(self.sel);
                    // never paint with a colour that left the palette
                    let fallback = self.pal.colours.first().map_or([128; 3], |c| c.rgb);
                    for c in [&mut self.first, &mut self.second] {
                        if self.pal.find(*c).is_none() {
                            *c = fallback;
                        }
                    }
                    self.save_palette(false);
                }
            });
        }
        if self.pal.colours.is_empty() && ui.button("Add").clicked() {
            self.add();
        }
    }

    fn add(&mut self) {
        let name = (1..).map(|k| format!("new {k}")).find(|n| self.pal.colours.iter().all(|c| c.name != *n)).unwrap();
        let mut rgb = self.pal.colours.get(self.sel).map_or([128; 3], |c| c.rgb);
        while self.pal.find(rgb).is_some() {
            rgb[2] = rgb[2].wrapping_add(1);
            if rgb[2] == 0 {
                rgb[1] = rgb[1].wrapping_add(1);
            }
        }
        self.pal.colours.push(Colour { name, rgb });
        self.select(self.pal.colours.len() - 1);
        self.save_palette(false);
    }

    // ---- closing

    fn close_check(&mut self, ctx: &egui::Context) {
        let unsaved: Vec<String> = self.docs.iter().filter(|(_, d)| d.dirty()).map(|(n, _)| n.clone()).collect();
        if ctx.input(|i| i.viewport().close_requested()) && !self.quit && !unsaved.is_empty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.closing = true;
        }
        if !self.closing {
            return;
        }
        egui::Modal::new(Id::unique("close")).show(ctx, |ui| {
            ui.heading("Unsaved work");
            ui.label(format!("Not saved: {}", unsaved.join(", ")));
            ui.horizontal(|ui| {
                if ui.button("Save all and close").clicked() {
                    self.save_files(unsaved.clone());
                    self.quit = self.docs.values().all(|d| !d.dirty());
                }
                if ui.button("Close without saving").clicked() {
                    self.quit = true;
                }
                if ui.button("Cancel").clicked() {
                    self.closing = false;
                }
            });
        });
        if self.quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}
