//! The Palette page (docs/design/studio.md "The Palette page", studio-tools.md "Palette: ramps"): the ramps as
//! rows and the loose colours last; the inspector names and sets the chosen colour, makes a ramp from a wall of
//! variations or recolours one, and sets the mood of the whole palette with a before and after.
use super::look::{self, c32, draw, GAP};
use super::wall::{self as wall_ui, Act};
use super::{App, Cmd, FileRow, Page, Thumb, PAL, TEX};
use crate::art::{self, History, Img};
use crate::ramp::{self, RampSpec};
use crate::shell::Flags;
use crate::wall::{Rng, Wall};
use eframe::egui::{self, Key, Modifiers, Pos2, Rect, RichText, Sense, StrokeKind, Ui, Vec2};
use engine::palette::{hex, to_hex, Colour, Palette};

#[derive(PartialEq, Clone, Copy)]
enum Mode {
    Colour,
    /// Make a ramp (`ramp` None) or recolour one.
    Ramp,
    Mood,
}

const WALL_N: usize = 9;

/// One candidate of the ramp wall: the spec moved by up to `k` (0..1) on every slider and the base colour.
fn vary(s: &RampSpec, r: &mut Rng, k: f32) -> RampSpec {
    let [h, sat, v] = ramp::hsv(s.base);
    let base = ramp::rgb([h + r.signed() * k * 0.06, (sat + r.signed() * k * 0.25).clamp(0.0, 1.0), (v + r.signed() * k * 0.2).clamp(0.05, 1.0)]);
    RampSpec {
        base,
        steps: s.steps,
        turn: (s.turn + r.signed() * k * 30.0).clamp(0.0, 60.0),
        fade: (s.fade + r.signed() * k * 0.4).clamp(0.0, 1.0),
        reach: (s.reach + r.signed() * k * 0.4).clamp(0.1, 1.0),
    }
}

/// A ramp drawn as a strip of equal cells.
fn strip(p: &egui::Painter, r: Rect, cs: &[[u8; 3]]) {
    let w = r.width() / cs.len().max(1) as f32;
    for (i, c) in cs.iter().enumerate() {
        p.rect_filled(Rect::from_min_size(r.min + Vec2::new(i as f32 * w, 0.0), Vec2::new(w.ceil(), r.height())), 0.0, c32(*c));
    }
}

pub struct PalettePage {
    sel: usize,
    name: String,
    hex: String,
    hsv: [f32; 3],
    moving: Option<usize>,
    /// The colour a slider drag started from, written out when it stops.
    edit_from: Option<[u8; 3]>,
    mode: Mode,
    /// The ramp open in the files column, and the one "Make a ramp" recolours.
    ramp: Option<String>,
    spec: RampSpec,
    new_name: String,
    wall: Option<Wall<RampSpec>>,
    /// Warmer, more saturated, lighter: -1..1 each.
    mood: [f32; 3],
    /// The palette file's text before each edit, and whether it was on the drafts list: the page writes at once,
    /// so undo writes the old bytes back.
    hist: History<(String, bool)>,
    /// The palette file's text as this page last wrote it: an undo over other changes on disk is refused.
    wrote: Option<String>,
    /// The chosen colour's users as last read: (when, its name, the files).
    seen: Option<(f64, String, Vec<String>)>,
}

/// Why an undo or redo from palette `now` back to `back` is refused: the file on disk is not what the page last
/// wrote (it would be lost), or a colour that goes is in use (`uses`: the files that use an rgb).
fn refusal(disk: &str, wrote: Option<&str>, now: &Palette, back: &Palette, uses: impl Fn([u8; 3]) -> Vec<String>) -> Option<String> {
    if wrote.is_some_and(|w| w != disk) {
        return Some(format!("Not undone: {PAL} changed on disk since this page wrote it, and undoing would lose that change. Reload the Studio first."));
    }
    now.colours.iter().filter(|c| !back.colours.iter().any(|b| b.name == c.name)).find_map(|c| {
        let used = uses(c.rgb);
        (!used.is_empty()).then(|| format!("Not undone: it would remove {}, which {} still use. Repaint them first.", c.name, used.join(", ")))
    })
}

/// Removes colour `name` unless a file uses it (`used`: the files that do); the refusal says how many.
fn remove_colour(pal: &mut Palette, name: &str, used: &[String]) -> Result<Colour, String> {
    let i = pal.colours.iter().position(|c| c.name == name).ok_or(format!("No colour named '{name}' in the palette"))?;
    if !used.is_empty() {
        return Err(format!("{name} is not removed: {} file(s) use it ({}). Repaint them first.", used.len(), used.join(", ")));
    }
    Ok(pal.colours.remove(i))
}

/// The model files in `dir` whose `colours` name `name` (a model holds its colours by name).
fn models_using(dir: &std::path::Path, name: &str) -> Vec<String> {
    let want = [format!("\"{name}\""), format!("'{name}'")];
    let mut v: Vec<String> = std::fs::read_dir(dir).into_iter().flatten().flatten().filter_map(|e| {
        let n = e.file_name().into_string().ok().filter(|n| n.ends_with(".toml"))?;
        let text = std::fs::read_to_string(e.path()).ok()?;
        // the `colours = [...]` list, to its closing bracket
        let at = text.lines().position(|l| l.trim_start().strip_prefix("colours").is_some_and(|r| r.trim_start().starts_with('=')))?;
        let list: String = text.lines().skip(at).collect::<Vec<_>>().join("\n");
        let list = &list[..list.find(']').unwrap_or(list.len())];
        want.iter().any(|w| list.contains(w.as_str())).then(|| format!("{}/{n}", super::models::MODELS))
    }).collect();
    v.sort();
    v
}

impl PalettePage {
    pub fn new(app: &App) -> PalettePage {
        let mut p = PalettePage {
            sel: 0,
            name: String::new(),
            hex: String::new(),
            hsv: [0.0; 3],
            moving: None,
            edit_from: None,
            mode: Mode::Colour,
            ramp: None,
            spec: RampSpec::new([128; 3]),
            new_name: String::new(),
            wall: None,
            mood: [0.0; 3],
            hist: History::default(),
            wrote: None,
            seen: None,
        };
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
            self.ramp = c.ramp.clone();
        }
    }

    /// Writes the palette; the file as it was is one undo step back.
    fn save(&mut self, app: &mut App, colour_changed: bool, name: &str) {
        let before = std::fs::read_to_string(PAL).unwrap_or_default();
        let draft = app.drafts.iter().any(|d| d == PAL);
        app.save_palette(colour_changed);
        let after = std::fs::read_to_string(PAL).ok();
        if after.as_deref() != Some(before.as_str()) {
            self.hist.push((before, draft), name);
            self.wrote = after;
        }
    }

    /// Undo or redo: the file's old text back on disk, and every texture repainted from the colours it had.
    fn step(&mut self, app: &mut App, undo: bool) -> bool {
        if self.edit_from.is_some() {
            return false;
        }
        let disk = std::fs::read_to_string(PAL).unwrap_or_default();
        let mut now = (disk.clone(), app.drafts.iter().any(|d| d == PAL));
        if !(if undo { self.hist.undo(&mut now) } else { self.hist.redo(&mut now) }) {
            return false;
        }
        let (text, draft) = now.clone();
        let why = match Palette::parse(&text) {
            Ok(pal) => match refusal(&disk, self.wrote.as_deref(), &app.pal, &pal, |rgb| Self::uses(app, rgb)) {
                None => std::fs::write(PAL, &text).map(|_| pal).map_err(|e| format!("Palette not written to {PAL}: {e}")),
                Some(why) => Err(why),
            },
            Err(e) => Err(format!("Not changed: {PAL} would not read afterwards: {e}")),
        };
        let pal = match why {
            Ok(p) => p,
            Err(why) => {
                // the step stays where it was
                let _ = if undo { self.hist.redo(&mut now) } else { self.hist.undo(&mut now) };
                app.refuse(why);
                return false;
            }
        };
        app.note(PAL);
        self.wrote = Some(text);
        // back on the drafts list (or off it) as it was at that step
        if draft != app.drafts.iter().any(|d| d == PAL) {
            let list = art::set_draft(&std::fs::read_to_string(super::DRAFTS).unwrap_or_default(), PAL, draft);
            app.write_drafts(list);
        }
        // one edit changes one thing: a colour that kept its name and changed its rgb is a repaint
        let pairs: Vec<([u8; 3], [u8; 3])> = app.pal.colours.iter().filter_map(|c| pal.colours.iter().find(|n| n.name == c.name && n.rgb != c.rgb).map(|n| (c.rgb, n.rgb))).collect();
        app.pal = pal;
        Self::repaint_all(app, &pairs);
        app.check();
        self.select(app, self.sel);
        true
    }

    /// `uses` of the chosen colour for the inspector, drawn every frame: the model files are read again at most
    /// once a second (and at once for another colour).
    fn uses_shown(&mut self, app: &App) -> Vec<String> {
        let c = &app.pal.colours[self.sel];
        if self.seen.as_ref().is_none_or(|(t, n, _)| n != &c.name || app.now - t >= 1.0) {
            self.seen = Some((app.now, c.name.clone(), Self::uses(app, c.rgb)));
        }
        self.seen.as_ref().unwrap().2.clone()
    }

    /// The texture files and saved model files that use a colour.
    fn uses(app: &App, rgb: [u8; 3]) -> Vec<String> {
        let mut v: Vec<String> = app.docs.iter().filter(|(_, d)| d.img.uses(rgb) || d.saved.as_ref().is_some_and(|s| s.uses(rgb))).map(|(n, _)| n.clone()).collect();
        // shortcut: models as saved on disk; an unsaved model's new colour is not seen (its save then refuses, nothing lost)
        if let Some(c) = app.pal.colours.iter().find(|c| c.rgb == rgb) {
            v.extend(models_using(std::path::Path::new(super::models::MODELS), &c.name));
        }
        v
    }

    /// Repaints every texture, in memory and on disk, by (old, new) colour pairs at once.
    fn repaint_all(app: &mut App, pairs: &[([u8; 3], [u8; 3])]) {
        if pairs.is_empty() {
            return;
        }
        for d in app.docs.values_mut() {
            for img in std::iter::once(&mut d.img).chain(d.saved.as_mut()).chain(d.hist.imgs()) {
                art::remap(img, pairs);
            }
        }
        let (done, failed) = art::remap_dir(std::path::Path::new(TEX), pairs);
        for n in &done {
            app.note(&art::tex_path(std::path::Path::new(TEX), n).to_string_lossy());
        }
        if !done.is_empty() {
            app.toast(format!("Repainted {} texture file(s)", done.len()));
        }
        for (n, why) in failed {
            // that file still holds the old colours on disk: it shows as not saved, so a save writes it
            if let Some(d) = app.docs.get_mut(&n) {
                d.saved = std::fs::read(art::tex_path(std::path::Path::new(TEX), &n)).ok().and_then(|b| art::decode(&b).ok());
            }
            app.refuse(format!("Texture not repainted on disk: {why}. It shows as unsaved; Save writes it."));
        }
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
        self.save(app, true, "Colour");
        let (done, failed) = art::repaint_dir(std::path::Path::new(TEX), from, to);
        for n in &done {
            app.note(&art::tex_path(std::path::Path::new(TEX), n).to_string_lossy());
        }
        app.toast(format!("{} {} to {}: repainted {} file(s)", app.pal.colours[self.sel].name, to_hex(from), to_hex(to), done.len()));
        for (n, why) in failed {
            if let Some(d) = app.docs.get_mut(&n) {
                d.saved = std::fs::read(art::tex_path(std::path::Path::new(TEX), &n)).ok().and_then(|b| art::decode(&b).ok());
            }
            app.refuse(format!("Texture not repainted on disk: {why}. It shows as unsaved; Save writes it."));
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
        self.add_named(app, &name, rgb, "Add");
    }

    /// A new colour at the end of the palette (the "+" where the swatches are: `add:<name>=#rrggbb`), one undo
    /// step named `step`.
    fn add_named(&mut self, app: &mut App, name: &str, rgb: [u8; 3], step: &str) {
        if let Err(e) = app.pal.check(name, rgb, None) {
            return app.refuse(e);
        }
        app.pal.colours.push(Colour::new(name, rgb));
        self.select(app, app.pal.colours.len() - 1);
        app.toast(format!("Added {name} to the palette"));
        self.save(app, false, step);
    }

    fn remove(&mut self, app: &mut App, step: &str) {
        let Some(c) = app.pal.colours.get(self.sel).cloned() else { return };
        let used = Self::uses(app, c.rgb);
        match remove_colour(&mut app.pal, &c.name, &used) {
            Ok(_) => {
                self.select(app, self.sel);
                app.toast(format!("Removed {}", c.name));
                self.save(app, false, step);
            }
            Err(e) => app.refuse(e),
        }
    }

    /// "Add shades" of colour `i` (`ramp::shades`).
    fn add_shades(&mut self, app: &mut App, i: usize) {
        match ramp::shades(&app.pal, i) {
            Ok((pal, added)) => {
                app.pal = pal;
                let at = app.pal.colours.iter().position(|c| c.name == added[0]).unwrap_or(0);
                self.select(app, at);
                app.toast(format!("Added the shades {}", added.join(", ")));
                self.save(app, false, &format!("Add shades of {}", app.pal.colours.get(i).map_or("", |c| c.name.as_str())));
            }
            Err(e) => app.refuse(e),
        }
    }

    /// Opens "Make a ramp": around the chosen colour for a new one, or around the open ramp's middle to recolour it.
    fn start_ramp(&mut self, app: &App, recolour: bool) {
        let run = self.ramp.as_deref().map(|r| app.pal.ramp(r)).unwrap_or_default();
        if recolour && !run.is_empty() {
            self.spec = RampSpec { steps: run.len(), ..RampSpec::new(app.pal.colours[run[run.len() / 2]].rgb) };
        } else {
            self.ramp = None;
            self.spec = RampSpec::new(app.pal.colours.get(self.sel).map_or([128; 3], |c| c.rgb));
            self.new_name = (1..).map(|k| if k == 1 { "new ramp".to_string() } else { format!("new ramp {k}") }).find(|n| app.pal.ramp(n).is_empty()).unwrap();
        }
        self.wall = Some(Wall::new(self.spec, WALL_N, 0.5, 1, &vary));
        self.mode = Mode::Ramp;
    }

    fn apply_ramp(&mut self, app: &mut App) {
        let colours = ramp::make(&self.spec);
        match self.ramp.clone() {
            Some(r) => match ramp::set(&mut app.pal, &r, &colours) {
                Ok(pairs) => {
                    self.save(app, !pairs.is_empty(), "Recolour the ramp");
                    Self::repaint_all(app, &pairs);
                    app.toast(format!("Recoloured the ramp {r}"));
                    self.select(app, self.sel);
                }
                Err(e) => app.refuse(e),
            },
            None => match ramp::add(&mut app.pal, &self.new_name, &colours) {
                Ok(()) => {
                    self.save(app, false, "Add the ramp");
                    app.toast(format!("Added the ramp {}", self.new_name.trim()));
                    self.select(app, app.pal.colours.len() - colours.len() / 2 - 1);
                    self.mode = Mode::Colour;
                }
                Err(e) => app.refuse(e),
            },
        }
    }

    fn apply_mood(&mut self, app: &mut App) {
        let after = ramp::mood(&app.pal, self.mood[0], self.mood[1], self.mood[2]);
        let pairs: Vec<([u8; 3], [u8; 3])> = app.pal.colours.iter().map(|c| c.rgb).zip(after.iter().copied()).filter(|(a, b)| a != b).collect();
        if pairs.is_empty() {
            return app.toast("The mood changes nothing");
        }
        for (c, a) in app.pal.colours.iter_mut().zip(&after) {
            c.rgb = *a;
        }
        self.save(app, true, "Mood");
        Self::repaint_all(app, &pairs);
        app.toast(format!("Mood applied: {} colour(s) changed", pairs.len()));
        self.mood = [0.0; 3];
        self.select(app, self.sel);
    }

    /// The colours drawn as rows: one per ramp, the loose colours last. The rects for dragging, by colour index.
    fn rows(&mut self, ui: &mut Ui, app: &App) -> (Vec<(usize, Rect)>, Option<usize>) {
        let (mut rects, mut clicked) = (vec![], None);
        let mut groups: Vec<(String, Vec<usize>)> = app.pal.ramps();
        let loose: Vec<usize> = (0..app.pal.colours.len()).filter(|&i| app.pal.colours[i].ramp.is_none()).collect();
        if !loose.is_empty() {
            groups.push((String::new(), loose));
        }
        let size = 64.0;
        for (name, idx) in groups {
            // a ramp wider than the column (a large interface scale) wraps instead of running under the inspector
            ui.horizontal_wrapped(|ui| {
                let (r, _) = ui.allocate_exact_size(Vec2::new(110.0, size), Sense::hover());
                let on = self.ramp.as_deref() == Some(name.as_str()) && !name.is_empty();
                let label = if name.is_empty() { "loose colours" } else { name.as_str() };
                let col = if on { look::ACCENT } else { ui.visuals().text_color() };
                let g = ui.painter().layout(label.to_string(), egui::FontId::proportional(look::body()), col, r.width() - 4.0);
                ui.painter().galley(Pos2::new(r.min.x, r.center().y - g.size().y / 2.0), g, col);
                ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                for i in idx {
                    let c = &app.pal.colours[i];
                    // (wide enough for a colour's name under it)
                    let (r, resp) = ui.allocate_exact_size(Vec2::new(size + 24.0, size + 16.0), Sense::click_and_drag());
                    crate::drive::mark(ui.ctx(), format!("colour:{}", c.name), r);
                    let sw = Rect::from_min_size(r.min, Vec2::new(r.width(), size));
                    let p = ui.painter();
                    p.rect_filled(sw, 4.0, c32(c.rgb));
                    if i == self.sel {
                        p.rect_stroke(sw, 4.0, egui::Stroke::new(3.0, look::ACCENT), StrokeKind::Outside);
                    }
                    // a name longer than its swatch ends in "…" (the whole of it in the tip), never cut mid-letter
                    let mut job = egui::text::LayoutJob::simple_singleline(c.name.clone(), egui::FontId::proportional(look::small()), look::TEXT2);
                    job.wrap = egui::text::TextWrapping::truncate_at_width(r.width() - 2.0);
                    let g = p.layout_job(job);
                    let x = r.center().x - g.size().x / 2.0;
                    p.galley(Pos2::new(x, r.max.y - g.size().y), g, look::TEXT2);
                    let resp = look::tip(resp, &format!("{} {}: a click chooses it, a drag moves it", c.name, to_hex(c.rgb)), "");
                    if resp.clicked() {
                        clicked = Some(i);
                    }
                    if resp.drag_started() {
                        self.moving = Some(i);
                    }
                    rects.push((i, r));
                }
            });
            ui.add_space(GAP * 0.5);
        }
        (rects, clicked)
    }

    fn ramp_work(&mut self, ui: &mut Ui, app: &App) {
        let now = ramp::make(&self.spec);
        match &self.ramp {
            Some(r) => {
                ui.label(RichText::new(format!("Recolour the ramp {r}")).strong());
                let before: Vec<[u8; 3]> = app.pal.ramp(r).iter().map(|&i| app.pal.colours[i].rgb).collect();
                for (label, cs) in [("before", before), ("after", now.clone())] {
                    ui.horizontal(|ui| {
                        ui.add_sized(Vec2::new(60.0, 40.0), egui::Label::new(RichText::new(label).small().weak()));
                        let (rect, _) = ui.allocate_exact_size(Vec2::new((ui.available_width()).min(560.0), 40.0), Sense::hover());
                        strip(ui.painter(), rect, &cs);
                    });
                }
            }
            None => {
                ui.label(RichText::new("Make a ramp").strong());
                let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().min(560.0), 48.0), Sense::hover());
                strip(ui.painter(), rect, &now);
            }
        }
        look::weak(ui, "Dark to light; the base sits in the middle. Click a candidate to take it, then set the sliders.");
        ui.add_space(GAP);
        let Some(w) = self.wall.as_mut() else { return };
        let chosen = w.items.iter().position(|s| *s == self.spec);
        let act = wall_ui::show(ui, w, chosen, Vec2::new(170.0, 52.0), |p, r, s| strip(p, r, &ramp::make(s)));
        match act {
            Act::Take(i) => self.spec = w.items[i],
            Act::More(i) => w.more_like(i, &vary),
            Act::Back => {
                w.back();
            }
            Act::Shuffle => w.shuffle(&vary),
            Act::Spread => w.fill(WALL_N, &vary),
            _ => {}
        }
    }

    fn mood_work(&mut self, ui: &mut Ui, app: &App) {
        let after = ramp::mood(&app.pal, self.mood[0], self.mood[1], self.mood[2]);
        let pairs: Vec<([u8; 3], [u8; 3])> = app.pal.colours.iter().map(|c| c.rgb).zip(after.iter().copied()).collect();
        ui.label(RichText::new("Mood of the palette").strong());
        look::weak(ui, "Before and after; \"Apply\" rewrites the palette once and repaints every texture.");
        ui.add_space(GAP);
        let before: Vec<[u8; 3]> = app.pal.colours.iter().map(|c| c.rgb).collect();
        for (label, cs) in [("before", &before), ("after", &after)] {
            ui.horizontal(|ui| {
                ui.add_sized(Vec2::new(60.0, 32.0), egui::Label::new(RichText::new(label).small().weak()));
                let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().min(720.0), 32.0), Sense::hover());
                strip(ui.painter(), rect, cs);
            });
        }
        ui.add_space(GAP);
        let painted: Vec<(&String, &Img)> = app.docs.iter().map(|(n, d)| (n, &d.img)).collect();
        if painted.is_empty() {
            look::weak(ui, "No painted blocks yet: their before and after shows here once a texture exists.");
            return;
        }
        ui.label(RichText::new("Every painted block").small());
        ui.horizontal_wrapped(|ui| {
            for (n, img) in painted {
                let mut a = img.clone();
                art::remap(&mut a, &pairs);
                ui.vertical(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::new(16.0 * 4.0 * 2.0 + 4.0, 64.0), Sense::hover());
                    draw(ui.painter(), img, 0, r.min, 4.0, false);
                    draw(ui.painter(), &a, 0, r.min + Vec2::new(68.0, 0.0), 4.0, false);
                    look::weak(ui, n.as_str());
                });
            }
        });
    }

    fn colour_inspector(&mut self, ui: &mut Ui, app: &mut App) {
        if let Some(mut sec) = look::section(ui, "Colour") {
            let ui = &mut *sec;
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
                look::label(ui, "name");
                let r = look::tip(ui.add(egui::TextEdit::singleline(&mut self.name).desired_width(f32::INFINITY)), "The colour's name: lower case, unique; Enter keeps it", "");
                if r.lost_focus() && self.name != app.pal.colours[self.sel].name {
                    match app.pal.check(&self.name, app.pal.colours[self.sel].rgb, Some(self.sel)) {
                        Ok(()) => {
                            app.pal.colours[self.sel].name = self.name.clone();
                            self.save(app, false, "Rename");
                        }
                        Err(e) => {
                            app.refuse(e);
                            self.name = app.pal.colours[self.sel].name.clone();
                        }
                    }
                }
                ui.end_row();
                look::label(ui, "hex");
                let r = look::tip(ui.add(egui::TextEdit::singleline(&mut self.hex).desired_width(f32::INFINITY)), "The colour as #rrggbb; Enter sets it and repaints every texture that uses it", "");
                if r.lost_focus() {
                    match hex(self.hex.trim()) {
                        Some(rgb) => {
                            self.set_colour(app, rgb);
                            self.flush(app);
                        }
                        None => app.refuse(format!("'{}' is not a colour. Type it as #rrggbb, six hex digits.", self.hex)),
                    }
                    self.select(app, self.sel);
                }
                ui.end_row();
                look::label(ui, "ramp");
                let cur = app.pal.colours[self.sel].ramp.clone();
                let mut pick = cur.clone();
                let r = egui::ComboBox::from_id_salt("ramp of colour").selected_text(cur.clone().unwrap_or("none".into())).width(ui.available_width()).show_ui(ui, |ui| {
                    ui.selectable_value(&mut pick, None, "none");
                    for (n, _) in app.pal.ramps() {
                        ui.selectable_value(&mut pick, Some(n.clone()), n);
                    }
                });
                look::tip(r.response, "The ramp it belongs to; a ramp runs dark to light in the palette's order", "");
                if pick != cur {
                    app.pal.colours[self.sel].ramp = pick.clone();
                    self.ramp = pick;
                    self.save(app, false, "Ramp");
                }
                ui.end_row();
            });
            ui.add_space(GAP * 0.5);
            for (k, label) in ["hue", "saturation", "value"].into_iter().enumerate() {
                let r = look::row(ui, label, |ui| {
                    ui.spacing_mut().slider_width = (ui.available_width() - 8.0).max(40.0);
                    ui.add(egui::Slider::new(&mut self.hsv[k], 0.0..=1.0).show_value(false))
                });
                crate::drive::mark(ui.ctx(), format!("slider:{label}"), r.rect);
                let r = look::tip(r, &format!("The colour's {label}; every texture using it follows as it moves"), "");
                if r.changed() {
                    let rgb = egui::ecolor::Hsva::new(self.hsv[0], self.hsv[1], self.hsv[2], 1.0).to_rgb().map(|v| (v * 255.0).round().clamp(0.0, 255.0) as u8);
                    self.set_colour(app, rgb);
                }
                if r.drag_stopped() || (r.changed() && !r.dragged()) {
                    self.flush(app);
                }
            }
            let uses = self.uses_shown(app);
            ui.add_space(GAP * 0.5);
            ui.label(RichText::new(format!("Used by {} file(s)", uses.len())).small());
            for u in &uses {
                look::weak(ui, u);
            }
            ui.add_space(GAP);
            ui.horizontal_wrapped(|ui| {
                if look::tip(ui.button("Add"), "A new colour beside the chosen one", "").clicked() {
                    self.add(app);
                }
                let r = ui.add_enabled(uses.is_empty(), egui::Button::new("Remove"));
                if look::tip(r, "Remove the colour; greyed while a texture or a model uses it", "").clicked() {
                    self.remove(app, "Remove");
                }
            });
        }
    }

    fn ramp_inspector(&mut self, ui: &mut Ui, app: &mut App) {
        let recolour = self.ramp.is_some();
        if let Some(mut sec) = look::section(ui, if recolour { "Recolour the ramp" } else { "Make a ramp" }) {
            let ui = &mut *sec;
            ui.horizontal(|ui| {
                let (r, resp) = ui.allocate_exact_size(Vec2::splat(36.0), Sense::hover());
                ui.painter().rect_filled(r, 4.0, c32(self.spec.base));
                look::tip(resp, "The base colour: the ramp's middle", "");
                ui.vertical(|ui| {
                    ui.label(RichText::new("base").small());
                    look::weak(ui, to_hex(self.spec.base));
                });
            });
            ui.spacing_mut().slider_width = (ui.available_width() - 165.0).max(60.0); // room for the value box and the label
            let s = &mut self.spec;
            let mut steps = s.steps as u32;
            let r = ui.add_enabled(!recolour, egui::Slider::new(&mut steps, 3..=9).text("steps"));
            look::tip(r, "How many colours, 3 to 9 (a ramp being recoloured keeps its own)", "");
            s.steps = steps as usize;
            look::tip(ui.add(egui::Slider::new(&mut s.turn, 0.0..=60.0).text("hue turn").suffix("°")), "How far the hue turns towards cool in the dark and warm in the light", "");
            look::tip(ui.add(egui::Slider::new(&mut s.fade, 0.0..=1.0).text("saturation fall")), "How much the colour greys towards both ends", "");
            look::tip(ui.add(egui::Slider::new(&mut s.reach, 0.1..=1.0).text("reach")), "How far the ends go towards black and white", "");
            ui.add_space(GAP);
            if recolour {
                let r = ui.button("Recolour the ramp");
                if look::tip(r, "Write these colours into the ramp; every texture using it follows", "").clicked() {
                    self.apply_ramp(app);
                }
            } else {
                ui.horizontal(|ui| {
                    ui.label("Name");
                    look::tip(ui.add(egui::TextEdit::singleline(&mut self.new_name).desired_width(f32::INFINITY)), "The new ramp's name; its colours are named after it", "");
                });
                let r = ui.button("Add the ramp");
                if look::tip(r, "Add these colours to the palette as a new ramp", "").clicked() {
                    self.apply_ramp(app);
                }
            }
            if look::tip(ui.button("Close"), "Back to the chosen colour", "").clicked() {
                self.mode = Mode::Colour;
            }
        }
    }

    fn mood_inspector(&mut self, ui: &mut Ui, app: &mut App) {
        if let Some(mut sec) = look::section(ui, "Mood of the palette") {
            let ui = &mut *sec;
            ui.spacing_mut().slider_width = (ui.available_width() - 165.0).max(60.0); // room for the value box and the label
            for (k, (label, help)) in [("warmer", "Turn every hue towards warm (right) or cool (left)"), ("saturated", "More (right) or less (left) colourful"), ("lighter", "Lighter (right) or darker (left)")].into_iter().enumerate() {
                look::tip(ui.add(egui::Slider::new(&mut self.mood[k], -1.0..=1.0).text(label).show_value(false)), help, "");
            }
            ui.add_space(GAP);
            ui.horizontal_wrapped(|ui| {
                if look::tip(ui.button("Apply"), "Rewrite the palette once and repaint every texture", "").clicked() {
                    self.apply_mood(app);
                }
                if look::tip(ui.button("Reset"), "The sliders back to the middle", "").clicked() {
                    self.mood = [0.0; 3];
                }
                if look::tip(ui.button("Close"), "Back to the chosen colour", "").clicked() {
                    self.mode = Mode::Colour;
                }
            });
        }
    }
}

impl Page for PalettePage {
    fn name(&self) -> &'static str {
        "Palette"
    }
    fn files(&self, app: &App) -> Vec<FileRow> {
        let flags = Flags { made: true, draft: app.is_draft(PAL), dirty: false, problem: app.has_problem(PAL) };
        let mut rows: Vec<FileRow> = app.pal.ramps().into_iter().map(|(n, idx)| FileRow { id: format!("ramp:{n}"), label: n, thumb: Thumb::Ramp(idx.iter().map(|&i| app.pal.colours[i].rgb).collect()), flags }).collect();
        let loose: Vec<[u8; 3]> = app.pal.colours.iter().filter(|c| c.ramp.is_none()).map(|c| c.rgb).collect();
        if !loose.is_empty() {
            rows.push(FileRow { id: "loose".into(), label: "loose colours".into(), thumb: Thumb::Ramp(loose), flags });
        }
        rows
    }
    fn current(&self) -> Option<String> {
        Some(self.ramp.as_ref().map_or("loose".into(), |r| format!("ramp:{r}")))
    }
    fn open(&mut self, app: &mut App, id: &str) {
        let first = match id.strip_prefix("ramp:") {
            Some(r) => app.pal.ramp(r).first().copied(),
            None => app.pal.colours.iter().position(|c| c.ramp.is_none()).or_else(|| app.pal.colours.iter().position(|c| c.name == id)),
        };
        if let Some(i) = first {
            self.select(app, i);
            if self.mode == Mode::Ramp {
                self.start_ramp(app, self.ramp.is_some());
            }
        }
    }
    fn commands(&self, _app: &App) -> Vec<Cmd> {
        vec![
            Cmd::new("colour.add", "Add a colour", "a new colour beside the chosen one"),
            Cmd::new("colour.remove", "Remove the colour", "only while no texture uses it"),
            Cmd::new("ramp.make", "Make a ramp", "a new ramp from the chosen colour, picked from a wall of variations"),
            Cmd::new("ramp.recolour", "Recolour this ramp", "the ramp's sliders and a wall of variations; every texture follows"),
            Cmd::new("ramp.apply", "Add or recolour the ramp", "write the ramp being made into the palette"),
            Cmd::new("mood", "Mood of the palette", "warmer, more saturated, lighter: the whole palette, with a before and after"),
            Cmd::new("mood.apply", "Apply the mood", "rewrite the palette once and repaint every texture"),
            Cmd::new("undo", "Undo", "the palette file as it was before the last change, every texture repainted with it").key(Modifiers::COMMAND, Key::Z),
            Cmd::new("redo", "Redo", "redo what was undone").key(Modifiers::COMMAND, Key::Y),
        ]
    }
    fn run(&mut self, app: &mut App, id: &str) {
        match id {
            "colour.add" => self.add(app),
            "colour.remove" => self.remove(app, "Remove"),
            "ramp.make" => self.start_ramp(app, false),
            "ramp.recolour" if self.ramp.is_some() => self.start_ramp(app, true),
            "ramp.recolour" => app.toast("Choose a colour of a ramp first"),
            "ramp.apply" if self.mode == Mode::Ramp => self.apply_ramp(app),
            "mood" => self.mode = Mode::Mood,
            "mood.apply" => self.apply_mood(app),
            "undo" | "redo" => {
                self.step(app, id == "undo");
            }
            // what the swatches on other pages send (`grow.rs`): by name, never by place; their Undo undoes
            // only the step they made, not a later one
            _ if id.starts_with("undo:") => match self.hist.undo.last().map(|s| s.name.clone()) {
                Some(n) if n == id[5..] => {
                    self.step(app, true);
                }
                n => app.refuse(format!("Not undone: the palette's last change is {}", n.map_or("none".into(), |n| format!("'{n}' (see the Palette page's history)")))),
            },
            _ if id.starts_with("add:") => match id[4..].rsplit_once('=').map(|(n, h)| (n, hex(h))) {
                Some((n, Some(rgb))) => self.add_named(app, n, rgb, &format!("Add {n}")),
                _ => app.refuse(format!("Not a colour: {id}. Use #rrggbb.")),
            },
            _ if id.starts_with("remove:") || id.starts_with("shades:") => {
                let name = &id[id.find(':').unwrap() + 1..];
                let Some(i) = app.pal.colours.iter().position(|c| c.name == name) else { return app.refuse(format!("No colour named '{name}' in the palette")) };
                self.select(app, i);
                if id.starts_with("remove:") {
                    self.remove(app, &format!("Remove {name}"));
                } else {
                    self.add_shades(app, i);
                }
            }
            _ => {
                // `mood=w,s,l` sets the sliders (for shots)
                if let Some(v) = id.strip_prefix("mood=") {
                    for (k, x) in v.split(';').filter_map(|x| x.parse::<f32>().ok()).take(3).enumerate() {
                        self.mood[k] = x.clamp(-1.0, 1.0);
                    }
                    self.mode = Mode::Mood;
                }
            }
        }
    }
    fn more_keys(&self) -> Vec<(&'static str, &'static str)> {
        vec![("Click", "choose a colour"), ("Drag a colour", "move it in the order; onto another ramp, it joins it")]
    }
    fn help(&self) -> String {
        match self.mode {
            Mode::Colour => "Palette: a click chooses a colour, a drag moves it; the inspector names and changes it, and every texture follows".into(),
            Mode::Ramp => "Make a ramp: click a candidate to take it, \"more like this\" for a wall around it; the sliders fine-tune".into(),
            Mode::Mood => "Mood: the sliders move every colour; \"Apply\" rewrites the palette and repaints every texture".into(),
        }
    }
    fn history(&self, _app: &App) -> Option<(Vec<String>, Vec<String>)> {
        Some((self.hist.undo.iter().map(|s| s.name.clone()).collect(), self.hist.redo.iter().rev().map(|s| s.name.clone()).collect()))
    }
    fn back_to(&mut self, app: &mut App, n: usize) {
        while self.hist.undo.len() > n && self.step(app, true) {}
        while self.hist.undo.len() < n && self.step(app, false) {}
    }
    fn refresh(&mut self, app: &App) {
        if self.edit_from.is_none() {
            self.select(app, self.sel);
        }
    }

    fn work(&mut self, ui: &mut Ui, app: &mut App) {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            match self.mode {
                Mode::Ramp => return self.ramp_work(ui, app),
                Mode::Mood => return self.mood_work(ui, app),
                Mode::Colour => {}
            }
            look::weak(ui, "Each ramp is a row from dark to light; a click chooses a colour, a drag moves it.");
            ui.add_space(GAP);
            let (rects, clicked) = self.rows(ui, app);
            if let Some(i) = clicked {
                self.select(app, i);
            }
            let Some(from) = self.moving else { return };
            let pos = ui.input(|i| i.pointer.interact_pos());
            let to = pos.and_then(|p| rects.iter().find(|(_, r)| r.contains(p)).map(|(i, _)| *i));
            if let Some(t) = to {
                let r = rects.iter().find(|(i, _)| *i == t).unwrap().1;
                ui.painter().rect_stroke(r.expand(3.0), 4.0, egui::Stroke::new(2.0, look::ACCENT), StrokeKind::Outside);
            }
            if ui.input(|i| i.pointer.any_released()) {
                self.moving = None;
                if let Some(t) = to.filter(|t| *t != from) {
                    // a colour dropped on another ramp's colour joins that ramp at its place
                    let ramp = app.pal.colours[t].ramp.clone();
                    let mut c = app.pal.colours.remove(from);
                    c.ramp = ramp;
                    app.pal.colours.insert(t, c);
                    self.select(app, t);
                    self.save(app, false, "Move a colour");
                }
            }
        });
    }

    fn inspector(&mut self, ui: &mut Ui, app: &mut App) {
        match self.mode {
            Mode::Colour => self.colour_inspector(ui, app),
            Mode::Ramp => self.ramp_inspector(ui, app),
            Mode::Mood => self.mood_inspector(ui, app),
        }
        if let Some(mut sec) = look::section(ui, "Ramps and mood") {
            let ui = &mut *sec;
            ui.horizontal_wrapped(|ui| {
                if self.mode != Mode::Ramp && look::tip(ui.button("Make a ramp"), "A new ramp from the chosen colour, from a wall of variations", "").clicked() {
                    self.start_ramp(app, false);
                }
                if self.ramp.is_some() && self.mode == Mode::Colour && look::tip(ui.button("Recolour this ramp"), "The ramp's sliders and a wall of variations; every texture follows", "").clicked() {
                    self.start_ramp(app, true);
                }
                if self.mode != Mode::Mood && look::tip(ui.button("Mood"), "Warmer, more saturated, lighter: the whole palette at once", "").clicked() {
                    self.mode = Mode::Mood;
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_undo_over_a_change_on_disk_or_of_a_colour_in_use_is_refused() {
        let now = Palette::parse("[[colour]]\nname = \"a\"\nrgb = \"#102030\"\n\n[[colour]]\nname = \"new 1\"\nrgb = \"#405060\"\n").unwrap();
        let back = Palette::parse("[[colour]]\nname = \"a\"\nrgb = \"#102030\"\n").unwrap();
        let nobody = |_: [u8; 3]| Vec::new();
        assert_eq!(refusal("x", Some("x"), &now, &back, nobody), None);
        let why = refusal("x, and another program's change", Some("x"), &now, &back, nobody).expect("a change on disk would be lost");
        assert!(why.contains("changed on disk"), "{why}");
        let why = refusal("x", Some("x"), &now, &back, |rgb| if rgb == [0x40, 0x50, 0x60] { vec!["dirt.png".into()] } else { vec![] }).expect("new 1 is in use");
        assert!(why.contains("new 1") && why.contains("dirt.png"), "{why}");
        // the other way (a colour comes back) takes nothing away
        assert_eq!(refusal("x", Some("x"), &back, &now, |_| vec!["dirt.png".into()]), None);
    }

    #[test]
    fn an_unused_colour_is_removed_and_a_used_one_refused_with_its_count() {
        let mut pal = Palette::parse("[[colour]]\nname = \"a\"\nrgb = \"#102030\"\n\n[[colour]]\nname = \"b\"\nrgb = \"#405060\"\n").unwrap();
        let why = remove_colour(&mut pal, "a", &["dirt.png".into(), "models/post.toml".into()]).unwrap_err();
        assert!(why.contains("2 file(s)") && why.contains("dirt.png"), "{why}");
        assert_eq!(pal.colours.len(), 2, "nothing removed");
        assert_eq!(remove_colour(&mut pal, "b", &[]).unwrap().name, "b");
        assert_eq!(pal.colours.len(), 1);
        assert!(remove_colour(&mut pal, "b", &[]).is_err(), "gone already");
        // a model names its colours: removing one it names would break it
        let dir = std::env::temp_dir().join(format!("technoape-palette-uses-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("post.toml"), "# a post of stone\ncolours = [\"stone\", \"stone dark\"]\n[[part]]\nname = \"a\"\n").unwrap();
        std::fs::write(dir.join("other.txt"), "colours = [\"stone\"]").unwrap();
        std::fs::write(dir.join("tuft.toml"), "colours_note = [\"moss\"]\ncolours = ['moss']\n").unwrap();
        assert_eq!(models_using(&dir, "moss"), ["data/models/tuft.toml"], "single quotes, and only the key colours");
        assert_eq!(models_using(&dir, "stone dark"), ["data/models/post.toml"]);
        assert_eq!(models_using(&dir, "stone"), ["data/models/post.toml"]);
        assert!(models_using(&dir, "a").is_empty(), "a part's name is no colour");
        assert!(models_using(&dir, "dark").is_empty(), "whole names only");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
