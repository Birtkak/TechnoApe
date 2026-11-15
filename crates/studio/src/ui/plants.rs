//! The Plants page (ADR 0091, 0133): the small plants the game draws as crossed painted planes, each a `kind =
//! "plant"` row of `data/clutter.toml` and its paintings `data/plants/<name>.png` (alternates `.1` to `.4`, a look
//! per `looks` key, `.back`, `.far`) at 32 px a metre. The work area paints one of them beside a stand of the
//! plant laid out as the game lays it (`engine::clutter::plant_faces`); the inspector holds its planes.
use super::look::{self, GAP};
use super::{App, Cmd, FileRow, Page, Thumb, DRAFTS};
use crate::art::{self, History, Img, EMPTY};
use crate::shell::Flags;
use eframe::egui::{self, Color32, Key, Modifiers, Rect, Sense, Ui, Vec2};
use engine::palette::Palette;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

pub const FILE: &str = "data/clutter.toml";
pub const DIR: &str = "data/plants";

/// A plant's painting at most this wide and high (`engine::clutter`'s rule: 2 m by 2.5 m at 32 px a metre).
pub const MAX_W: usize = 64;
pub const MAX_H: usize = 80;

/// A new plant's row: grass ground, the warm seasons, two planes.
pub fn new_row(name: &str, planes: u8) -> String {
    format!("[[clutter]]\nname = \"{name}\"\nkind = \"plant\"\nplanes = {planes}\non = [\"grass\", \"meadow grass\"]\nseason = [\"spring\", \"summer\", \"autumn\"]\n")
}

/// A new plant's first painting, 32 px square: a tuft of blades in the palette's greens (its first colour when
/// it has none), to paint over.
pub fn sprout(pal: &Palette) -> Img {
    let col = |n: &str| pal.colours.iter().find(|c| c.name == n).or(pal.colours.first()).map_or([60, 140, 50], |c| c.rgb);
    let greens = [col("grass dark"), col("grass"), col("grass light")];
    let mut img = Img { w: 32, h: 32, px: vec![EMPTY; 32 * 32] };
    for (k, (x0, top, lean)) in [(6, 14, -1), (11, 6, 0), (15, 2, 1), (19, 8, 0), (24, 12, 1), (13, 18, -1)].into_iter().enumerate() {
        for y in top..32 {
            let x = (x0 + lean * (31 - y as i32) / 8).clamp(0, 31) as usize;
            img.px[y * 32 + x] = greens[(k + y / 11) % 3];
        }
    }
    img
}

/// `img` at `w` x `h`, standing where it stood: kept at its bottom middle, new pixels empty.
pub fn resized(img: &Img, w: usize, h: usize) -> Img {
    let (dx, dy) = ((w as i32 - img.w as i32) / 2, h as i32 - img.h as i32);
    let mut out = Img { w, h, px: vec![EMPTY; w * h] };
    for y in 0..h {
        for x in 0..w {
            let (sx, sy) = (x as i32 - dx, y as i32 - dy);
            if (0..img.w as i32).contains(&sx) && (0..img.h as i32).contains(&sy) {
                out.px[y * w + x] = img.px[sy as usize * img.w + sx as usize];
            }
        }
    }
    out
}

/// A painting as the game reads it (`engine::models::painting`: palette colours, alpha 0 or 255) and within a
/// plant's size.
pub fn read(bytes: &[u8], pal: &Palette) -> Result<Img, String> {
    let p = engine::models::painting(bytes, pal)?;
    let img = Img { w: p.w as usize, h: p.h as usize, px: p.px.iter().map(|c| if c[3] == 0 { EMPTY } else { [c[0], c[1], c[2]] }).collect() };
    fits(&img).map(|_| img)
}

fn fits(img: &Img) -> Result<(), String> {
    if img.w == 0 || img.h == 0 || img.w > MAX_W || img.h > MAX_H {
        return Err(format!("{} x {} px, a plant is at most {MAX_W} x {MAX_H} (2 m wide, 2.5 m high at 32 px a metre)", img.w, img.h));
    }
    Ok(())
}

/// Saves plant `name`: its row into the clutter file `file` (only that row's text changes) and each painting
/// `(<file stem>, picture)` into `dir`, every one first held to the game's own readers (`engine::clutter::parse`
/// and `engine::models::painting`); nothing is written when one is refused.
pub fn save(file: &Path, dir: &Path, name: &str, row: &str, paints: &[(String, &Img)], pal: &Palette) -> Result<(), String> {
    for (stem, img) in paints {
        fits(img).map_err(|e| format!("{stem}.png: {e}"))?;
        read(&art::encode(img), pal).map_err(|e| format!("{stem}.png: {e}"))?;
    }
    // the row held to the game's reader before anything is written; the paintings first, so a row is never
    // on disk without them
    let check = |t: &str| engine::clutter::parse(t).map(|_| ());
    let old = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    check(&crate::rows::put(&old, "clutter", name, row)).map_err(|e| format!("{}: {e}", file.display()))?;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for (stem, img) in paints {
        art::save(&dir.join(format!("{stem}.png")), img, false)?;
    }
    crate::rows::save(&file.to_string_lossy(), "clutter", name, row, check)?;
    Ok(())
}

/// A painting drawn a pixel a square in `rect` (empty pixels a checkerboard, `outline` rectangles x, y, w, h
/// in yellow); while a button is held on a pixel: that pixel and whether it is the left button. A stroke is the
/// frames this returns Some in a row.
pub(super) fn paint_canvas(ui: &mut Ui, rect: Rect, img: &Img, id: &str, outline: &[[u32; 4]], help: &str) -> Option<(usize, usize, bool)> {
    if img.w == 0 || img.h == 0 {
        return None;
    }
    let k = ((rect.width() - 2.0 * GAP) / img.w as f32).min((rect.height() - 2.0 * GAP) / img.h as f32).floor().clamp(1.0, 24.0);
    let at = rect.center() - Vec2::new(img.w as f32, img.h as f32) * k / 2.0;
    let r = ui.interact(rect, egui::Id::unique(id), Sense::click_and_drag());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 4.0, Color32::from_gray(24));
    for y in 0..img.h {
        for x in 0..img.w {
            let c = img.px[y * img.w + x];
            let px = Rect::from_min_size(at + Vec2::new(x as f32, y as f32) * k, Vec2::splat(k));
            p.rect_filled(px, 0.0, if c == EMPTY { Color32::from_gray(if (x + y) % 2 == 0 { 70 } else { 90 }) } else { look::c32(c) });
        }
    }
    for o in outline {
        let q = Rect::from_min_size(at + Vec2::new(o[0] as f32, o[1] as f32) * k, Vec2::new(o[2] as f32, o[3] as f32) * k);
        p.rect_stroke(q, 0.0, egui::Stroke::new(1.5, Color32::from_rgb(240, 210, 80)), egui::StrokeKind::Outside);
    }
    let held = ui.input(|i| (i.pointer.primary_down(), i.pointer.secondary_down()));
    let cell = r.hover_pos().map(|q| ((q - at) / k).floor()).filter(|v| v.x >= 0.0 && v.y >= 0.0 && (v.x as usize) < img.w && (v.y as usize) < img.h);
    let mut hit = None;
    if let Some(v) = cell {
        p.rect_stroke(Rect::from_min_size(at + v * k, Vec2::splat(k)), 0.0, egui::Stroke::new(1.0, Color32::WHITE), egui::StrokeKind::Inside);
        if r.is_pointer_button_down_on() && (held.0 || held.1) {
            hit = Some((v.x as usize, v.y as usize, held.0));
        }
    }
    look::tip(r, help, "");
    hit
}

/// One painting of a plant open in the Studio.
struct PDoc {
    img: Img,
    hist: History<Img>,
    saved: Option<Img>,
    rev: u64,
}

/// One plant: its row's text as the page writes it and as the file has it, and its paintings by file stem.
struct Plant {
    row: String,
    saved_row: Option<String>,
    paints: BTreeMap<String, PDoc>,
}
impl Plant {
    fn planes(&self) -> u8 {
        crate::rows::Keys::parse(&self.row).get("planes").and_then(|v| v.trim().parse().ok()).unwrap_or(2)
    }
    fn dirty(&self) -> bool {
        self.saved_row.as_deref() != Some(&self.row) || self.paints.values().any(|d| d.saved.as_ref() != Some(&d.img))
    }
}

pub struct Plants {
    plants: BTreeMap<String, Plant>,
    open: Option<String>,
    /// The open painting's file stem (`fern`, `fern.1`, `fern.back`).
    file: String,
    first: [u8; 3],
    second: [u8; 3],
    erase: bool,
    stroking: bool,
    yaw: f32,
    tex: HashMap<String, (u64, egui::TextureHandle)>,
    new_name: String,
    refused: Vec<String>,
}

/// A painting's small picture for the files column: fitted into 16 x 16 on the column's dark, standing on its
/// bottom edge.
fn thumb(img: &Img) -> Img {
    let k = (img.w.max(img.h) as f32 / 16.0).max(1.0);
    let mut t = Img { w: 16, h: 16, px: vec![[26, 28, 32]; 256] };
    for y in 0..16 {
        for x in 0..16 {
            let (sx, sy) = ((x as f32 * k) as usize, img.h as f32 - (16 - y) as f32 * k);
            if sx < img.w && sy >= 0.0 {
                let c = img.px[sy as usize * img.w + sx];
                if c != EMPTY {
                    t.px[y * 16 + x] = c;
                }
            }
        }
    }
    t
}

impl Plants {
    pub fn new(app: &App) -> Plants {
        let text = std::fs::read_to_string(FILE).unwrap_or_default();
        let mut plants = BTreeMap::new();
        let mut refused = vec![];
        let rows = engine::clutter::parse(&text).unwrap_or_else(|e| {
            refused.push(format!("{FILE}: {e}"));
            vec![]
        });
        let on_disk = art::pngs(Path::new(DIR));
        for s in crate::rows::spans(&text).into_iter().filter(|s| s.header == "clutter") {
            let Some(r) = rows.iter().find(|r| r.name == s.name && r.kind == engine::clutter::Kind::Plant) else { continue };
            let mut paints = BTreeMap::new();
            let looks: Vec<&str> = r.looks.iter().map(|(n, _)| n.as_str()).collect();
            for f in on_disk.iter().filter_map(|f| f.strip_suffix(".png")) {
                let tail = f.strip_prefix(r.name.as_str()).and_then(|t| if t.is_empty() { Some("") } else { t.strip_prefix('.') });
                let Some(tail) = tail else { continue };
                if !(tail.is_empty() || ["1", "2", "3", "4", "back", "far"].contains(&tail) || looks.contains(&tail)) {
                    continue;
                }
                match std::fs::read(format!("{DIR}/{f}.png")).map_err(|e| e.to_string()).and_then(|b| read(&b, &app.pal)) {
                    Ok(img) => {
                        paints.insert(f.to_string(), PDoc { img: img.clone(), hist: History::default(), saved: Some(img), rev: 0 });
                    }
                    Err(e) => refused.push(format!("{DIR}/{f}.png: {e}")),
                }
            }
            let row = text[s.at].to_string();
            plants.insert(r.name.clone(), Plant { saved_row: Some(row.clone()), row, paints });
        }
        let at = |n: &str| app.pal.colours.iter().find(|c| c.name == n).or(app.pal.colours.first()).map_or([80, 150, 60], |c| c.rgb);
        let open = plants.keys().next().cloned();
        Plants { file: open.clone().unwrap_or_default(), open, plants, first: at("grass"), second: at("grass dark"), erase: false, stroking: false, yaw: 0.6, tex: HashMap::new(), new_name: String::new(), refused }
    }

    fn doc(&self) -> Option<&PDoc> {
        self.plants.get(self.open.as_ref()?)?.paints.get(&self.file)
    }
    fn doc_mut(&mut self) -> Option<&mut PDoc> {
        self.plants.get_mut(self.open.as_ref()?)?.paints.get_mut(&self.file)
    }
    /// One edit of the open painting; `merge` keeps a stroke's pixels one undo step.
    fn edit(&mut self, name: &str, merge: bool, f: impl FnOnce(&mut Img)) {
        let Some(d) = self.doc_mut() else { return };
        let before = d.img.clone();
        f(&mut d.img);
        if d.img != before {
            if !merge {
                d.hist.push(before, name);
            }
            d.rev += 1;
        }
    }
    fn set_planes(&mut self, n: u8) {
        let Some(p) = self.open.as_ref().and_then(|o| self.plants.get_mut(o)) else { return };
        let mut k = crate::rows::Keys::parse(&p.row);
        k.set_after("planes", Some(n.to_string()), &["kind"]);
        p.row = k.text();
    }

    fn save(&mut self, app: &mut App) {
        let Some(name) = self.open.clone() else { return };
        let Some(p) = self.plants.get_mut(&name) else { return };
        let dirty: Vec<(String, &Img)> = p.paints.iter().filter(|(_, d)| d.saved.as_ref() != Some(&d.img)).map(|(f, d)| (f.clone(), &d.img)).collect();
        let files: Vec<String> = dirty.iter().map(|(f, _)| format!("{DIR}/{f}.png")).collect();
        match save(Path::new(FILE), Path::new(DIR), &name, &p.row, &dirty, &app.pal) {
            Ok(()) => {
                p.saved_row = Some(p.row.clone());
                for d in p.paints.values_mut() {
                    d.saved = Some(d.img.clone());
                }
                let mut drafts = std::fs::read_to_string(DRAFTS).unwrap_or_default();
                for f in &files {
                    drafts = art::set_draft(&drafts, f, false);
                    app.note(f);
                }
                app.write_drafts(drafts);
                app.note(FILE);
                app.toast(format!("Saved {name}: its row and {} painting(s). The game reads them when it starts; a new or changed row after a rebuild (clutter.toml is built in)", files.len()));
            }
            Err(e) => app.refuse(format!("Not saved: {e}")),
        }
    }

    fn new_plant(&mut self, app: &mut App) {
        let name = self.new_name.trim().to_string();
        // a name of no row in the file as it is now and of no painting on disk, so nothing is written over
        let taken = crate::rows::spans(&std::fs::read_to_string(FILE).unwrap_or_default()).iter().any(|s| s.header == "clutter" && s.name == name) || Path::new(&format!("{DIR}/{name}.png")).exists();
        if name.is_empty() || name.contains(['.', '"', '/', '\\']) || self.plants.contains_key(&name) || taken {
            return app.refuse(format!("\"{name}\": a plant needs a new name without . \" / or \\"));
        }
        let img = sprout(&app.pal);
        let paints = BTreeMap::from([(name.clone(), PDoc { img, hist: History::default(), saved: None, rev: 0 })]);
        self.plants.insert(name.clone(), Plant { row: new_row(&name, 2), saved_row: None, paints });
        (self.open, self.file) = (Some(name.clone()), name);
        self.new_name.clear();
    }

    /// The next free alternate `.1` to `.4`, a copy of the open painting.
    fn add_alternate(&mut self, app: &mut App) {
        let Some(name) = self.open.clone() else { return };
        let Some(p) = self.plants.get_mut(&name) else { return };
        let Some(stem) = (1..=engine::clutter::ALTERNATES).map(|k| format!("{name}.{k}")).find(|s| !p.paints.contains_key(s)) else {
            return app.refuse(format!("{name} has all {} alternates", engine::clutter::ALTERNATES));
        };
        let Some(img) = p.paints.get(&self.file).map(|d| d.img.clone()) else { return };
        p.paints.insert(stem.clone(), PDoc { img, hist: History::default(), saved: None, rev: 0 });
        self.file = stem;
    }

    fn texture(&mut self, ctx: &egui::Context, stem: &str) -> Option<egui::TextureId> {
        let d = self.plants.get(self.open.as_ref()?)?.paints.get(stem)?;
        if self.tex.get(stem).is_none_or(|(rev, _)| *rev != d.rev) {
            let px: Vec<u8> = d.img.px.iter().flat_map(|c| if *c == EMPTY { [0; 4] } else { [c[0], c[1], c[2], 255] }).collect();
            let t = ctx.load_texture(format!("plant {stem}"), egui::ColorImage::from_rgba_unmultiplied([d.img.w, d.img.h], &px), egui::TextureOptions::NEAREST);
            self.tex.insert(stem.to_string(), (d.rev, t));
        }
        self.tex.get(stem).map(|(_, t)| t.id())
    }

    /// The painting, a pixel a square; left paints the first colour, right the second (the eraser: empties).
    fn canvas(&mut self, ui: &mut Ui, rect: Rect) {
        let Some(img) = self.doc().map(|d| d.img.clone()) else { return };
        let hit = paint_canvas(ui, rect, &img, "plant canvas", &[], "Left paints the first colour, right the second; with the eraser on, both empty a pixel. 32 px are 1 m; the bottom row stands on the ground");
        if let Some((x, y, left)) = hit {
            let c = if self.erase { EMPTY } else if left { self.first } else { self.second };
            let merge = self.stroking;
            self.edit(if self.erase { "Erase" } else { "Paint" }, merge, |img| img.px[y * img.w + x] = c);
        }
        self.stroking = hit.is_some();
    }

    /// A stand of 3 x 3 of the plant on its ground, each cell's planes as the game lays them (`plant_faces`):
    /// the middle one wears the open painting, the others its alternates in turn. Drag turns it.
    // shortcut: drawn back to front by each plane's middle, without the climate's tint, the sway or the light;
    // the game view cannot stand a plant yet (it grows them by biome), add that when one is asked for
    fn stand(&mut self, ui: &mut Ui, rect: Rect) {
        let Some(name) = self.open.clone() else { return };
        let Some(planes) = self.plants.get(&name).map(|p| p.planes()) else { return };
        let r = ui.interact(rect, egui::Id::unique("plant stand"), Sense::drag());
        self.yaw += r.drag_delta().x * 0.01;
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 4.0, Color32::from_rgb(142, 184, 222));
        let (cy, sy, cp, sp) = (self.yaw.cos(), self.yaw.sin(), 0.42f32.cos(), 0.42f32.sin());
        let k = rect.width().min(rect.height() * 1.4) / 4.0;
        let centre = rect.center() + Vec2::new(0.0, k * 0.55);
        let proj = |v: glam::Vec3| {
            let (x, z) = (v.x * cy - v.z * sy, v.x * sy + v.z * cy);
            (centre + Vec2::new(x * k, -(v.y * cp - z * sp) * k), v.y * sp + z * cp)
        };
        let ground = [(-1.5, -1.5), (1.5, -1.5), (1.5, 1.5), (-1.5, 1.5)].map(|(x, z)| proj(glam::Vec3::new(x, 0.0, z)).0);
        p.add(egui::Shape::convex_polygon(ground.to_vec(), Color32::from_rgb(86, 120, 54), egui::Stroke::NONE));
        let mut alts: Vec<String> = std::iter::once(name.clone()).chain((1..=engine::clutter::ALTERNATES).map(|a| format!("{name}.{a}"))).collect();
        alts.retain(|s| self.plants[&name].paints.contains_key(s));
        let mut faces = vec![];
        for (n, (i, j)) in (-1..=1).flat_map(|i| (-1..=1).map(move |j| (i, j))).enumerate() {
            let stem = if (i, j) == (0, 0) || alts.is_empty() { self.file.clone() } else { alts[n % alts.len()].clone() };
            let Some(img) = self.plants[&name].paints.get(&stem).map(|d| (d.img.w, d.img.h)) else { continue };
            let Some(tex) = self.texture(ui.ctx(), &stem) else { continue };
            let off = glam::Vec3::new(i as f32 - 0.5, 0.0, j as f32 - 0.5);
            for ([o, eu, ev], _) in engine::clutter::plant_faces(planes, img.0 as u32, img.1 as u32) {
                let o = o + off;
                let depth = proj(o + (eu + ev) * 0.5).1;
                faces.push((depth, tex, [o, o + eu, o + eu + ev, o + ev]));
            }
        }
        faces.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, tex, q) in faces {
            let mut m = egui::Mesh::with_texture(tex);
            for (v, uv) in q.iter().zip([[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]) {
                m.vertices.push(egui::epaint::Vertex { pos: proj(*v).0, uv: egui::pos2(uv[0], uv[1]), color: Color32::WHITE });
            }
            m.indices.extend([0, 1, 2, 0, 2, 3]);
            p.add(egui::Shape::mesh(m));
        }
        look::tip(r, "The plant three by three as the game lays its planes (a 1 m cell each); drag turns it. The game adds the climate's tint, the wind and the light", "");
    }
}

impl Page for Plants {
    fn name(&self) -> &'static str {
        "Plants"
    }
    fn files(&self, app: &App) -> Vec<FileRow> {
        let mut v: Vec<FileRow> = self
            .plants
            .iter()
            .map(|(n, p)| {
                let path = format!("{DIR}/{n}.png");
                let img = p.paints.get(n).or(p.paints.values().next()).map_or(Img { w: 16, h: 16, px: vec![[26, 28, 32]; 256] }, |d| thumb(&d.img));
                FileRow { id: n.clone(), label: n.clone(), thumb: Thumb::Img(img), flags: Flags { made: p.saved_row.is_some(), dirty: p.dirty(), draft: app.is_draft(&path), problem: app.has_problem(&path) } }
            })
            .collect();
        v.extend(self.refused.iter().map(|e| FileRow { id: format!("(refused) {e}"), label: e.clone(), thumb: Thumb::Flat(look::BAD), flags: Flags { made: true, draft: false, dirty: false, problem: true } }));
        v
    }
    fn current(&self) -> Option<String> {
        self.open.clone()
    }
    fn open(&mut self, app: &mut App, id: &str) {
        if self.plants.contains_key(id) {
            (self.open, self.file) = (Some(id.into()), id.into());
            if !self.plants[id].paints.contains_key(id) {
                self.file = self.plants[id].paints.keys().next().cloned().unwrap_or_default();
            }
        } else if let Some(e) = id.strip_prefix("(refused) ") {
            app.refuse(e.to_string());
        }
    }
    fn commands(&self, _app: &App) -> Vec<Cmd> {
        vec![
            Cmd::new("save", "Save the plant", "write its row of data/clutter.toml and its paintings in data/plants, each checked by the game's reader").key(Modifiers::COMMAND, Key::S),
            Cmd::new("undo", "Undo", "undo the last change to this painting").key(Modifiers::COMMAND, Key::Z),
            Cmd::new("redo", "Redo", "redo what was undone").key(Modifiers::COMMAND, Key::Y),
            Cmd::new("erase", "Eraser", "both buttons empty a pixel, or paint again").key(Modifiers::NONE, Key::E),
            Cmd::new("alt", "Add an alternate", "the next of .1 to .4, a copy of the open painting: the game picks one per column"),
            Cmd::new("new", "New plant", "a new plant from the name in the inspector: a tuft to paint over and its row"),
        ]
    }
    fn run(&mut self, app: &mut App, id: &str) {
        match id {
            "save" => self.save(app),
            "undo" => {
                if let Some(d) = self.doc_mut() {
                    if d.hist.undo(&mut d.img) {
                        d.rev += 1;
                    }
                }
            }
            "redo" => {
                if let Some(d) = self.doc_mut() {
                    if d.hist.redo(&mut d.img) {
                        d.rev += 1;
                    }
                }
            }
            "erase" => self.erase = !self.erase,
            "alt" => self.add_alternate(app),
            "new" => self.new_plant(app),
            _ => {
                if let Some(n) = id.strip_prefix("planes:").and_then(|n| n.parse().ok()) {
                    self.set_planes(n);
                } else if let Some(f) = id.strip_prefix("file:") {
                    self.file = f.to_string();
                } else if let Some(n) = id.strip_prefix("new:") {
                    self.new_name = n.to_string();
                    self.new_plant(app);
                }
            }
        }
    }
    fn more_keys(&self) -> Vec<(&'static str, &'static str)> {
        vec![("Left / right on the painting", "paint the first / second colour"), ("Drag on the stand", "turn it")]
    }
    fn work(&mut self, ui: &mut Ui, app: &mut App) {
        let Some(name) = self.open.clone() else {
            look::heading(ui, "No plant yet: name one in the inspector and press New plant");
            return;
        };
        ui.horizontal_wrapped(|ui| {
            let stems: Vec<String> = self.plants[&name].paints.keys().cloned().collect();
            for s in stems {
                let label = s.strip_prefix(name.as_str()).and_then(|t| t.strip_prefix('.')).unwrap_or("base").to_string();
                if look::tip(look::chip(ui, self.file == s, &label), &format!("{DIR}/{s}.png"), "").clicked() {
                    self.file = s;
                }
            }
            if look::tip(ui.small_button("+ alternate"), "The next of .1 to .4, a copy of this painting: the game picks one per column", "").clicked() {
                self.add_alternate(app);
            }
        });
        if let Some(d) = self.doc() {
            look::weak(ui, format!("{}.png: {} x {} px, {:.2} x {:.2} m", self.file, d.img.w, d.img.h, d.img.w as f32 / 32.0, d.img.h as f32 / 32.0));
        }
        let rect = ui.available_rect_before_wrap();
        let half = rect.width() * 0.5;
        self.canvas(ui, Rect::from_min_size(rect.min, Vec2::new(half - GAP * 0.5, rect.height())));
        self.stand(ui, Rect::from_min_max(rect.min + Vec2::new(half + GAP * 0.5, 0.0), rect.max));
        ui.allocate_rect(rect, Sense::hover());
    }
    fn inspector(&mut self, ui: &mut Ui, app: &mut App) {
        if let Some(mut sec) = look::section(ui, "Colours") {
            let ui = &mut *sec;
            look::tip(look::toggle(ui, &mut self.erase, "Eraser E"), "Both buttons empty a pixel: the plant's outline", "E");
            super::textures::palette_picker(ui, "plants", &app.pal, &mut self.first, &mut self.second);
        }
        if let Some(mut sec) = look::section(ui, "Plant") {
            let ui = &mut *sec;
            let planes = self.open.as_ref().and_then(|o| self.plants.get(o)).map(|p| p.planes());
            if let Some(n) = planes {
                ui.horizontal(|ui| {
                    look::label(ui, "planes");
                    for (k, help) in [(2, "two crossed planes: grass, flowers"), (3, "three: a fuller tuft, a fern"), (4, "a shrub: three lobed clumps of three planes")] {
                        if look::tip(look::chip(ui, n == k, &k.to_string()), help, "").clicked() {
                            self.set_planes(k);
                        }
                    }
                });
            }
            if let Some((w, h)) = self.doc().map(|d| (d.img.w, d.img.h)) {
                let (mut nw, mut nh) = (w, h);
                ui.horizontal(|ui| {
                    look::label(ui, "size px");
                    look::tip(ui.add(egui::DragValue::new(&mut nw).range(1..=MAX_W)), "Wide, in pixels (32 a metre); it grows or shrinks about its middle", "");
                    look::tip(ui.add(egui::DragValue::new(&mut nh).range(1..=MAX_H)), "High, in pixels; it grows or shrinks at the top, its foot stays on the ground", "");
                });
                if (nw, nh) != (w, h) {
                    self.edit("Resize", false, |img| *img = resized(img, nw, nh));
                }
            }
            look::weak(ui, "The ground it grows on, its seasons and where (a biome's undergrowth in data/biomes.toml) stay in the files for now.");
        }
        if let Some(mut sec) = look::section(ui, "New plant") {
            let ui = &mut *sec;
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.new_name).hint_text("a name").desired_width(ui.available_width() - 90.0));
                if look::tip(ui.button("New plant"), "A tuft to paint over and its row (two planes on grass, spring to autumn)", "").clicked() {
                    self.new_plant(app);
                }
            });
        }
    }
    fn help(&self) -> String {
        "Plants: left paints the first colour, right the second, E the eraser; drag the stand to turn it".into()
    }
    fn history(&self, _app: &App) -> Option<(Vec<String>, Vec<String>)> {
        self.doc().map(|d| (d.hist.undo.iter().map(|s| s.name.clone()).collect(), d.hist.redo.iter().rev().map(|s| s.name.clone()).collect()))
    }
    fn back_to(&mut self, _app: &mut App, n: usize) {
        if let Some(d) = self.doc_mut() {
            d.hist.back_to(&mut d.img, n);
            d.rev += 1;
        }
    }
    fn unsaved(&self) -> Vec<String> {
        self.plants.iter().filter(|(_, p)| p.dirty()).map(|(n, _)| format!("{n} (plant)")).collect()
    }
    fn save_all(&mut self, app: &mut App) {
        let open = self.open.clone();
        for n in self.plants.iter().filter(|(_, p)| p.dirty()).map(|(n, _)| n.clone()).collect::<Vec<_>>() {
            self.open = Some(n);
            self.save(app);
        }
        self.open = open;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pal() -> Palette {
        Palette::parse(include_str!("../../../../data/palette.toml")).unwrap()
    }

    /// A plant from nothing: its row and paintings are written where the game reads them, and read back by the
    /// game's own readers; the stand's planes are as large as the painting.
    #[test]
    fn a_new_plant_saves_as_the_game_reads_it() {
        let (pal, d) = (pal(), std::env::temp_dir().join(format!("studio-plants-{}", std::process::id())));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("plants")).unwrap();
        let file = d.join("clutter.toml");
        std::fs::write(&file, include_str!("../../../../data/clutter.toml")).unwrap();
        let img = sprout(&pal);
        let alt = resized(&img, 40, 48);
        assert_eq!((alt.w, alt.h, alt.px[47 * 40 + 4 + 11]), (40, 48, img.px[31 * 32 + 11]), "kept at its foot, in the middle");
        save(&file, &d.join("plants"), "clover", &new_row("clover", 3), &[("clover".into(), &img), ("clover.1".into(), &alt)], &pal).unwrap();
        let rows = engine::clutter::parse(&std::fs::read_to_string(&file).unwrap()).unwrap();
        let r = rows.iter().find(|r| r.name == "clover").expect("its row");
        assert_eq!((r.kind, r.planes), (engine::clutter::Kind::Plant, 3));
        assert!(rows.iter().any(|r| r.name == "berry bush"), "the other rows kept");
        let back = read(&std::fs::read(d.join("plants/clover.1.png")).unwrap(), &pal).unwrap();
        assert_eq!(back, alt, "the painting as painted");
        // a refused one writes nothing: too wide, or a colour off the palette
        let wide = resized(&img, MAX_W + 2, 32);
        assert!(save(&file, &d.join("plants"), "wide", &new_row("wide", 2), &[("wide".into(), &wide)], &pal).unwrap_err().contains("at most"));
        assert!(!d.join("plants/wide.png").exists() && !std::fs::read_to_string(&file).unwrap().contains("\"wide\""));
        let mut off = img.clone();
        off.px[0] = [1, 2, 3];
        assert!(save(&file, &d.join("plants"), "off", &new_row("off", 2), &[("off".into(), &off)], &pal).is_err());
        let faces = engine::clutter::plant_faces(3, img.w as u32, img.h as u32);
        assert_eq!(faces.len(), 3);
        assert!((faces[0].0[1].length() - 1.0).abs() < 1e-4, "32 px wide is 1 m: {:?}", faces[0].0[1]);
        let _ = std::fs::remove_dir_all(&d);
    }
}
