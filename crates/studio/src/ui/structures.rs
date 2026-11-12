//! The Structures page (docs/design/studio-tools.md "Structures and flora", ADR 0052): `data/structures/*.toml`,
//! made by hand in layers or grown from a recipe. The game's renderer stands it on the grass (the view's `place`
//! blocks); below it a slice of one layer to place and clear blocks in; the inspector holds the block list and
//! the tools, or for a grown one the recipe's numbers, the seed and a wall of six seeds. "Freeze" makes a grown
//! one hand work. The file's reading, writing and growing are `engine::structures`'s, tested there.
use super::look::{self, GAP};
use super::wall as wallui;
use super::{App, Cmd, FileRow, Page, Thumb, DRAFTS};
use crate::art::{self, History};
use crate::shell::Flags;
use crate::wall::{Rng, Wall};
use eframe::egui::{self, Color32, Key, Modifiers, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};
use engine::structures::{grow, Crown, Recipe, Structure, CLEAR, KEEP, MAX_SIDE, RANGES};
use glam::Vec3;
use std::collections::BTreeMap;

pub const DIR: &str = "data/structures";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tool {
    Add,
    Keep,
    Clear,
    Box,
    Fill,
    Select,
}
const TOOLS: [(Tool, &str, &str, &str); 6] = [
    (Tool::Add, "Add", "Left click or drag places the chosen block; right click leaves the world's block", "A"),
    (Tool::Keep, "Remove", "Click or drag removes the cell from the structure: the world's block stays", "R"),
    (Tool::Clear, "Air", "Click or drag clears the cell to air: it digs the world's block where the structure stands", "X"),
    (Tool::Box, "Box", "Drag a rectangle on the layer: filled with the chosen block", "B"),
    (Tool::Fill, "Fill", "Click: the joined cells of the same kind on this layer take the chosen block", "G"),
    (Tool::Select, "Select", "Drag a rectangle on the layer to copy (Ctrl+C); Ctrl+V pastes under the pointer, on any layer or structure; right click lets go", "S"),
];

/// A copied rectangle of a layer: `w` cells across, each KEEP, CLEAR or a block's name (so it pastes into a
/// structure with other letters).
#[derive(Clone, PartialEq, Debug)]
struct Piece {
    w: u32,
    cells: Vec<String>,
}

fn copy(s: &Structure, y: u32, a: (u32, u32), b: (u32, u32)) -> Piece {
    let mut cells = vec![];
    for z in a.1.min(b.1)..=a.1.max(b.1) {
        for x in a.0.min(b.0)..=a.0.max(b.0) {
            let l = s.cells[s.at(x, y, z)];
            cells.push(s.key.get(&(l as char).to_string()).cloned().unwrap_or_else(|| (if l == CLEAR { "_" } else { "." }).into()));
        }
    }
    Piece { w: a.0.abs_diff(b.0) + 1, cells }
}

/// The piece with its corner at x, z of layer y; what falls outside the box is dropped.
fn paste(s: &mut Structure, p: &Piece, x: u32, y: u32, z: u32) {
    for (i, c) in p.cells.iter().enumerate() {
        let (px, pz) = (x + i as u32 % p.w, z + i as u32 / p.w);
        if px >= s.size[0] || pz >= s.size[2] {
            continue;
        }
        let l = match c.as_str() {
            "." => Some(KEEP),
            "_" => Some(CLEAR),
            name => s.letter(name),
        };
        if let Some(l) = l {
            let k = s.at(px, y, pz);
            s.cells[k] = l;
        }
    }
}

struct SDoc {
    s: Structure,
    hist: History<Structure>,
    /// The file's text as it is on disk; none for one not saved yet.
    saved: Option<String>,
}
impl SDoc {
    fn dirty(&self) -> bool {
        self.saved.as_deref() != Some(self.s.write().as_str())
    }
}

pub struct Structures {
    docs: BTreeMap<String, SDoc>,
    open: Option<String>,
    /// Files refused on reading, with why.
    refused: BTreeMap<String, String>,
    tool: Tool,
    block: String,
    layer: u32,
    drag: Option<(u32, u32)>,
    /// The selected rectangle (two corners, x and z), what was copied, and the slice cell under the pointer.
    sel: Option<((u32, u32), (u32, u32))>,
    clip: Option<Piece>,
    hover: Option<(u32, u32)>,
    stroke: bool,
    merging: Option<String>,
    wall: Option<Wall<Recipe>>,
    walls: u64,
    /// The view's blocks for the structure as it was when they were made.
    placed: Option<(Structure, String)>,
    /// The file the view was last aimed at.
    aimed: Option<String>,
}

/// A candidate of the wall: another seed, the numbers moved by `spread` (at the default, little more than the seed).
fn vary(r: &Recipe, rng: &mut Rng, spread: f32) -> Recipe {
    let mut v = r.clone();
    v.seed = rng.next() % 1_000_000;
    for (n, lo, hi, _) in RANGES {
        let x = (v.get(n) + rng.signed() * spread * 0.25 * (hi - lo)).clamp(lo, hi);
        // whole like the inspector's sliders, or the first frame shown snaps it into another recipe
        v.set(n, if n == "leaf_density" { (x * 100.0).round() / 100.0 } else { x.round() });
    }
    v
}

fn colours(s: &Structure, app: &App) -> BTreeMap<u8, Color32> {
    s.key.iter().map(|(l, n)| (l.as_bytes()[0], app.reg.placeable().find(|d| &d.name == n).map_or(Color32::GRAY, |d| Color32::from_rgb((d.colour[0] * 255.0) as u8, (d.colour[1] * 255.0) as u8, (d.colour[2] * 255.0) as u8)))).collect()
}

/// A side view: each column's nearest block from the front, shaded by depth.
fn sketch(p: &egui::Painter, r: Rect, s: &Structure, cols: &BTreeMap<u8, Color32>) {
    let [sx, sy, sz] = s.size;
    let k = (r.width() / sx as f32).min(r.height() / sy as f32);
    let o = Pos2::new(r.center().x - k * sx as f32 / 2.0, r.center().y + k * sy as f32 / 2.0);
    for y in 0..sy {
        for x in 0..sx {
            if let Some((z, c)) = (0..sz).rev().find_map(|z| cols.get(&s.cells[s.at(x, y, z)]).map(|c| (z, *c))) {
                let shade = 0.6 + 0.4 * (z as f32 + 1.0) / sz as f32;
                let c = Color32::from_rgb((c.r() as f32 * shade) as u8, (c.g() as f32 * shade) as u8, (c.b() as f32 * shade) as u8);
                p.rect_filled(Rect::from_min_size(Pos2::new(o.x + x as f32 * k, o.y - (y + 1) as f32 * k), Vec2::splat(k.max(1.0))), 0.0, c);
            }
        }
    }
}

impl Structures {
    pub fn new(app: &App) -> Structures {
        let mut s = Structures { docs: BTreeMap::new(), open: None, refused: BTreeMap::new(), tool: Tool::Add, block: "stone".into(), layer: 0, drag: None, sel: None, clip: None, hover: None, stroke: false, merging: None, wall: None, walls: 0, placed: None, aimed: None };
        s.read_disk(app);
        s.open = s.docs.keys().find(|n| n.as_str() == "oak").or(s.docs.keys().next()).cloned();
        s
    }

    fn read_disk(&mut self, app: &App) {
        self.refused.clear();
        let mut names: Vec<String> = std::fs::read_dir(DIR).into_iter().flatten().flatten().map(|e| e.file_name().to_string_lossy().to_string()).filter_map(|n| n.strip_suffix(".toml").map(str::to_string)).collect();
        names.sort();
        for n in names {
            let path = format!("{DIR}/{n}.toml");
            let text = std::fs::read_to_string(&path).unwrap_or_default().replace("\r\n", "\n");
            match Structure::parse(&path, &text, &app.reg) {
                Ok(s) => match self.docs.get_mut(&n) {
                    Some(d) if d.dirty() => d.saved = Some(text),
                    Some(d) => {
                        if d.s != s {
                            d.hist.push(std::mem::replace(&mut d.s, s), "Changed on disk");
                        }
                        d.saved = Some(text);
                    }
                    None => {
                        self.docs.insert(n, SDoc { s, hist: History::default(), saved: Some(text) });
                    }
                },
                Err(e) => {
                    self.refused.insert(n, e);
                }
            }
        }
    }

    fn doc(&self) -> Option<&SDoc> {
        self.open.as_ref().and_then(|n| self.docs.get(n))
    }

    /// An edit of the open structure, one undo step (a drag or a slider's moves merged into one).
    fn edit(&mut self, name: &str, f: impl FnOnce(&mut Structure)) {
        let merge = self.merging.as_deref() == Some(name);
        let Some(d) = self.open.as_ref().and_then(|n| self.docs.get_mut(n)) else { return };
        let before = d.s.clone();
        f(&mut d.s);
        if d.s != before && !merge {
            d.hist.push(before, name);
        }
    }
    fn edit_merged(&mut self, name: &str, f: impl FnOnce(&mut Structure)) {
        self.edit(name, f);
        self.merging = Some(name.into());
    }
    fn regrow(&mut self, name: &str, r: Recipe) {
        self.edit_merged(name, |s| *s = grow(&r));
    }

    fn save(&mut self, app: &mut App) {
        let Some(name) = self.open.clone() else { return };
        let text = self.docs[&name].s.write();
        let path = format!("{DIR}/{name}.toml");
        // the game's reader takes it before it goes to disk, through a temporary file
        let ok = Structure::parse(&path, &text, &app.reg).and_then(|_| {
            let _ = std::fs::create_dir_all(DIR);
            let tmp = format!("{path}.tmp");
            std::fs::write(&tmp, &text).map_err(|e| format!("{tmp}: {e}"))?;
            std::fs::rename(&tmp, &path).map_err(|e| {
                let _ = std::fs::remove_file(&tmp);
                format!("{path}: {e}")
            })
        });
        match ok {
            Ok(()) => {
                self.docs.get_mut(&name).unwrap().saved = Some(text);
                let drafts = art::set_draft(&std::fs::read_to_string(DRAFTS).unwrap_or_default(), &path, false);
                app.write_drafts(drafts);
                app.note(&path);
                app.toast(format!("Saved {path}"));
                app.check();
            }
            Err(e) => app.refuse(format!("Not saved: {e}")),
        }
    }

    fn new_structure(&mut self) {
        let name = (1..).map(|k| if k == 1 { "new".to_string() } else { format!("new{k}") }).find(|n| !self.docs.contains_key(n)).unwrap();
        let mut s = Structure::empty([5, 3, 5]);
        s.anchor = [2, 0, 2];
        self.docs.insert(name.clone(), SDoc { s, hist: History::default(), saved: None });
        self.open = Some(name);
        (self.wall, self.layer) = (None, 0);
    }

    /// Puts the block of the tool at a layer cell (a drag's first cell starts the undo step).
    fn paint(&mut self, x: u32, z: u32, right: bool) {
        let (tool, block, y) = (self.tool, self.block.clone(), self.layer);
        let name = TOOLS.iter().find(|t| t.0 == tool).map_or("Edit", |t| t.1);
        let f = |s: &mut Structure| {
            let v = match tool {
                _ if right => KEEP,
                Tool::Keep => KEEP,
                Tool::Clear => CLEAR,
                _ => match s.letter(&block) {
                    Some(l) => l,
                    None => return,
                },
            };
            if tool == Tool::Fill && !right {
                let at = s.at(x, y, z);
                let from = s.cells[at];
                if from == v {
                    return;
                }
                let mut todo = vec![(x, z)];
                while let Some((a, b)) = todo.pop() {
                    let k = s.at(a, y, b);
                    if s.cells[k] != from {
                        continue;
                    }
                    s.cells[k] = v;
                    if a > 0 {
                        todo.push((a - 1, b));
                    }
                    if b > 0 {
                        todo.push((a, b - 1));
                    }
                    if a + 1 < s.size[0] {
                        todo.push((a + 1, b));
                    }
                    if b + 1 < s.size[2] {
                        todo.push((a, b + 1));
                    }
                }
            } else {
                let k = s.at(x, y, z);
                s.cells[k] = v;
            }
        };
        if self.stroke {
            self.edit_merged(name, f);
        } else {
            self.edit(name, f);
            self.stroke = true;
            self.merging = Some(name.into());
        }
    }

    fn fill_box(&mut self, a: (u32, u32), b: (u32, u32)) {
        let (block, y) = (self.block.clone(), self.layer);
        self.edit("Box", |s| {
            let Some(l) = s.letter(&block) else { return };
            for x in a.0.min(b.0)..=a.0.max(b.0) {
                for z in a.1.min(b.1)..=a.1.max(b.1) {
                    let k = s.at(x, y, z);
                    s.cells[k] = l;
                }
            }
        });
    }

    /// A new box size, the cells kept where they still fit.
    fn resize(s: &mut Structure, size: [u32; 3]) {
        let mut n = Structure::empty(size);
        (n.key, n.anchor) = (s.key.clone(), [0, 1, 2].map(|i| s.anchor[i].min(size[i] - 1)));
        for y in 0..size[1].min(s.size[1]) {
            for z in 0..size[2].min(s.size[2]) {
                for x in 0..size[0].min(s.size[0]) {
                    let k = n.at(x, y, z);
                    n.cells[k] = s.cells[s.at(x, y, z)];
                }
            }
        }
        *s = n;
    }

    /// The structure to the renderer as `place` blocks, its anchor in the stand cell.
    fn feed_view(&mut self, app: &mut App) {
        app.view.show_model(None, None, 0.0);
        app.view.live = false;
        let Some(s) = self.doc().map(|d| d.s.clone()) else {
            app.view.place = String::new();
            return;
        };
        if self.placed.as_ref().map(|p| &p.0) != Some(&s) {
            let names: BTreeMap<u8, String> = app.reg.placeable().map(|d| (d.id, d.name.replace(' ', "_"))).collect();
            // the view's frame: x along j, z along i (as `structure=` on a stand run)
            let blocks = s.blocks(&app.reg);
            let mut line: Vec<String> = blocks.iter().map(|(d, id)| format!("{}:{}:{}:{}", d[2], d[0], d[1], names.get(id).map_or("_", |n| n.as_str()))).collect();
            // a clearing of air 4 blocks round it, from its foot up, so the spawn's own trees do not hide it
            let set: std::collections::HashSet<[i32; 3]> = blocks.iter().map(|b| b.0).collect();
            let [ax, ay, az] = s.anchor.map(|v| v as i32);
            let [sx, sy, sz] = s.size.map(|v| v as i32);
            for y in 0..sy - ay + 3 {
                for z in -az - 4..sz - az + 4 {
                    for x in -ax - 4..sx - ax + 4 {
                        if !set.contains(&[x, y, z]) {
                            line.push(format!("{z}:{x}:{y}:_"));
                        }
                    }
                }
            }
            let line = line.join(",");
            self.placed = Some((s, line));
        }
        app.view.place = self.placed.as_ref().unwrap().1.clone();
    }

    fn aim(&self, app: &mut App) {
        if let Some(s) = self.doc().map(|d| &d.s) {
            let c = Vec3::new(s.size[0] as f32 / 2.0 - s.anchor[0] as f32, s.size[1] as f32 * 0.45, s.size[2] as f32 / 2.0 - s.anchor[2] as f32);
            app.view.orbit.target = c;
            app.view.orbit.dist = s.size.iter().copied().max().unwrap_or(4) as f32 * 1.5 + 3.0;
            app.view.orbit.pitch = 0.25;
        }
    }

    /// The layer's cells: x across, z down; the chosen tool on a click or a drag.
    fn slice(&mut self, ui: &mut Ui, app: &mut App, h: f32) {
        let Some(s) = self.doc().map(|d| d.s.clone()) else { return };
        let grown = s.grow.is_some();
        self.layer = self.layer.min(s.size[1] - 1);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Layer").small());
            let mut l = self.layer + 1;
            ui.spacing_mut().slider_width = (ui.available_width() - 260.0).clamp(80.0, 400.0);
            let r = ui.add(egui::Slider::new(&mut l, 1..=s.size[1]).suffix(format!(" of {}", s.size[1])));
            look::tip(r, "The layer the slice shows, from the bottom; Page Up and Page Down step it", "PgUp PgDn");
            self.layer = l - 1;
            if grown {
                look::weak(ui, "grown: Freeze it to edit by hand");
            }
        });
        let cols = colours(&s, app);
        let k = ((ui.available_width() - 2.0) / s.size[0] as f32).min((h - 30.0) / s.size[2] as f32).clamp(4.0, 40.0);
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(k * s.size[0] as f32, k * s.size[2] as f32), Sense::click_and_drag());
        crate::drive::mark(ui.ctx(), "slice", rect);
        let p = ui.painter_at(rect);
        let y = self.layer;
        for z in 0..s.size[2] {
            for x in 0..s.size[0] {
                let r = Rect::from_min_size(rect.min + Vec2::new(x as f32 * k, z as f32 * k), Vec2::splat(k)).shrink(0.5);
                match s.cells[s.at(x, y, z)] {
                    KEEP => {
                        let below = (y > 0).then(|| cols.get(&s.cells[s.at(x, y - 1, z)])).flatten();
                        p.rect_filled(r, 0.0, below.map_or(Color32::from_gray(46), |c| c.gamma_multiply(0.25)));
                    }
                    CLEAR => {
                        p.rect_filled(r, 0.0, Color32::from_gray(22));
                        p.line_segment([r.left_top(), r.right_bottom()], Stroke::new(1.0, Color32::from_gray(90)));
                    }
                    l => {
                        p.rect_filled(r, 0.0, cols.get(&l).copied().unwrap_or(Color32::GRAY));
                    }
                }
                if [x, y, z] == s.anchor {
                    p.circle_stroke(r.center(), k * 0.3, Stroke::new(2.0, look::ACCENT));
                }
            }
        }
        let cell = |pos: Pos2| {
            let v = (pos - rect.min) / k;
            (v.x >= 0.0 && v.y >= 0.0 && (v.x as u32) < s.size[0] && (v.y as u32) < s.size[2]).then(|| (v.x as u32, v.y as u32))
        };
        self.hover = resp.hover_pos().and_then(cell);
        // the selection, held to the box (a smaller structure may be open than it was made on)
        self.sel = self.sel.filter(|(a, b)| a.0.max(b.0) < s.size[0] && a.1.max(b.1) < s.size[2]);
        let frame = |a: (u32, u32), b: (u32, u32)| Rect::from_two_pos(rect.min + Vec2::new(a.0.min(b.0) as f32, a.1.min(b.1) as f32) * k, rect.min + Vec2::new(a.0.max(b.0) as f32 + 1.0, a.1.max(b.1) as f32 + 1.0) * k);
        if let Some((a, b)) = self.sel {
            p.rect_stroke(frame(a, b), 0.0, Stroke::new(2.0, Color32::WHITE), egui::StrokeKind::Inside);
        }
        if let Some(c) = resp.hover_pos().and_then(cell) {
            let l = s.cells[s.at(c.0, y, c.1)];
            let what = match l {
                KEEP => "the world's block".to_string(),
                CLEAR => "air".into(),
                l => s.key.get(&(l as char).to_string()).cloned().unwrap_or_default(),
            };
            app.pointer = format!("x {} z {} layer {}: {what}", c.0 + 1, c.1 + 1, y + 1);
        }
        if grown {
            return;
        }
        let right = resp.dragged_by(egui::PointerButton::Secondary) || resp.secondary_clicked();
        let pos = ui.input(|i| i.pointer.interact_pos());
        // a drag starts once the pointer has moved: its first cell is where the button went down
        let origin = ui.input(|i| i.pointer.press_origin()).and_then(cell);
        // the box's far corner held to the slice, so a drag let go outside it still fills
        let held = |pos: Pos2| {
            let v = (pos - rect.min) / k;
            ((v.x.max(0.0) as u32).min(s.size[0] - 1), (v.y.max(0.0) as u32).min(s.size[2] - 1))
        };
        let select = self.tool == Tool::Select;
        if select && right {
            self.sel = None;
            return;
        }
        if (self.tool == Tool::Box || select) && !right {
            let mut done = None;
            if resp.drag_started() {
                self.drag = origin;
            }
            if let (Some(a), Some(b)) = (self.drag, pos.map(held)) {
                p.rect_stroke(frame(a, b), 0.0, Stroke::new(2.0, look::ACCENT), egui::StrokeKind::Inside);
                if resp.drag_stopped() {
                    done = Some((a, b));
                }
            }
            if resp.drag_stopped() {
                self.drag = None;
            }
            if resp.clicked() {
                done = pos.and_then(cell).map(|c| (c, c));
            }
            match done {
                Some((a, b)) if select => self.sel = Some((a, b)),
                Some((a, b)) => self.fill_box(a, b),
                None => {}
            }
            return;
        }
        if let Some(c) = origin.filter(|_| resp.drag_started()) {
            self.paint(c.0, c.1, right);
        }
        if resp.clicked() || resp.secondary_clicked() || resp.dragged() {
            if let Some(c) = pos.and_then(cell) {
                self.paint(c.0, c.1, right);
            }
        }
    }
}

impl Page for Structures {
    fn name(&self) -> &'static str {
        "Structures"
    }

    fn files(&self, app: &App) -> Vec<FileRow> {
        let mut v: Vec<FileRow> = self
            .docs
            .iter()
            .map(|(n, d)| {
                let cols = colours(&d.s, app);
                let main = d.s.key.keys().max_by_key(|l| d.s.cells.iter().filter(|c| **c == l.as_bytes()[0]).count()).and_then(|l| cols.get(&l.as_bytes()[0])).copied().unwrap_or(Color32::GRAY);
                let label = if d.s.grow.is_some() { format!("{n} (grown)") } else { n.clone() };
                let path = format!("{DIR}/{n}.toml");
                FileRow { id: n.clone(), label, thumb: Thumb::Flat(main), flags: Flags { made: d.saved.is_some(), draft: app.is_draft(&path), dirty: d.dirty(), problem: app.has_problem(&path) } }
            })
            .collect();
        for (n, _) in self.refused.iter().filter(|(n, _)| !self.docs.contains_key(*n)) {
            v.push(FileRow { id: n.clone(), label: format!("{n} (refused)"), thumb: Thumb::Flat(look::BAD), flags: Flags { made: true, draft: false, dirty: false, problem: true } });
        }
        v
    }

    fn current(&self) -> Option<String> {
        self.open.clone()
    }

    fn open(&mut self, app: &mut App, id: &str) {
        if self.docs.contains_key(id) {
            self.open = Some(id.into());
            (self.wall, self.drag, self.sel, self.layer) = (None, None, None, 0);
        } else if let Some(e) = self.refused.get(id) {
            app.refuse(e.clone());
        }
    }

    fn commands(&self, _app: &App) -> Vec<Cmd> {
        let mut v = vec![
            Cmd::new("save", "Save the structure", "write data/structures/<name>.toml (checked by the game's reader) and take it off the drafts list").key(Modifiers::COMMAND, Key::S),
            Cmd::new("undo", "Undo", "undo the last change to this structure").key(Modifiers::COMMAND, Key::Z),
            Cmd::new("redo", "Redo", "redo what was undone").key(Modifiers::COMMAND, Key::Y),
            Cmd::new("new", "New structure", "an empty box of 5 by 3 by 5 blocks to build by hand"),
            Cmd::new("mirror", "Mirror", "the structure mirrored left to right").key(Modifiers::NONE, Key::M),
            Cmd::new("copyup", "Copy the layer up", "this layer's cells into the layer above").key(Modifiers::NONE, Key::C),
            Cmd::new("copy", "Copy the selection", "the selected rectangle of this layer, to paste on any layer or structure").key(Modifiers::COMMAND, Key::C),
            Cmd::new("paste", "Paste", "what was copied, its corner under the pointer (else at the selection's corner) on this layer").key(Modifiers::COMMAND, Key::V),
            Cmd::new("up", "Layer up", "the slice one layer higher").key(Modifiers::NONE, Key::PageUp),
            Cmd::new("down", "Layer down", "the slice one layer lower").key(Modifiers::NONE, Key::PageDown),
            Cmd::new("freeze", "Freeze", "a grown structure as plain layers, to edit by hand (the recipe is dropped)"),
            Cmd::new("another", "Another seed", "the recipe grown from another seed").key(Modifiers::NONE, Key::N),
            Cmd::new("vary", "Variations", "a wall of six seeds of this recipe").key(Modifiers::NONE, Key::V),
        ];
        for (t, name, help, key) in TOOLS {
            let k = match t {
                Tool::Add => Key::A,
                Tool::Keep => Key::R,
                Tool::Clear => Key::X,
                Tool::Box => Key::B,
                Tool::Fill => Key::G,
                Tool::Select => Key::S,
            };
            let _ = key;
            v.push(Cmd::new(format!("tool:{name}"), format!("{name} tool"), help).key(Modifiers::NONE, k));
        }
        v
    }

    fn run(&mut self, app: &mut App, id: &str) {
        // with an argument (the command box and shot=): tool:<name>, block:<name>, layer:<n>, take:<wall item>, hour:<h>
        if let Some((cmd, arg)) = id.split_once(':') {
            match cmd {
                "tool" => self.tool = TOOLS.iter().find(|t| t.1.eq_ignore_ascii_case(arg)).map_or(self.tool, |t| t.0),
                "block" => self.block = arg.replace('_', " "),
                "layer" => self.layer = arg.parse::<u32>().unwrap_or(1).saturating_sub(1),
                "take" => {
                    if let Some(r) = self.wall.as_ref().and_then(|w| w.items.get(arg.parse::<usize>().unwrap_or(99)).cloned()) {
                        self.edit("Take a variation", |s| *s = grow(&r));
                    }
                }
                "hour" => app.view.hour = arg.parse().unwrap_or(app.view.hour),
                _ => {}
            }
            return;
        }
        let grown = self.doc().and_then(|d| d.s.grow.clone());
        match id {
            "save" => self.save(app),
            "new" => self.new_structure(),
            "undo" | "redo" => {
                if let Some(d) = self.open.as_ref().and_then(|n| self.docs.get_mut(n)) {
                    if if id == "undo" { d.hist.undo(&mut d.s) } else { d.hist.redo(&mut d.s) } {}
                }
            }
            // the selection and the layer are held to the box here too: a command can come before the slice is drawn
            "copy" => match (self.sel, self.doc()) {
                (Some((a, b)), Some(d)) if a.0.max(b.0) < d.s.size[0] && a.1.max(b.1) < d.s.size[2] => {
                    let p = copy(&d.s, self.layer.min(d.s.size[1] - 1), a, b);
                    app.toast(format!("Copied {} x {} cells: Ctrl+V pastes under the pointer", p.w, p.cells.len() as u32 / p.w));
                    self.clip = Some(p);
                }
                _ => app.toast("Nothing selected: drag a rectangle with Select (S)".to_string()),
            },
            "paste" if grown.is_none() => match self.clip.clone() {
                Some(p) => {
                    let (at, y) = (self.hover.or(self.sel.map(|(a, b)| (a.0.min(b.0), a.1.min(b.1)))).unwrap_or((0, 0)), self.layer);
                    self.edit("Paste", |s| paste(s, &p, at.0, y.min(s.size[1] - 1), at.1));
                }
                None => app.toast("Nothing copied yet".to_string()),
            },
            "up" => self.layer += 1,
            "down" => self.layer = self.layer.saturating_sub(1),
            "mirror" if grown.is_none() => self.edit("Mirror", |s| {
                let old = s.clone();
                for y in 0..s.size[1] {
                    for z in 0..s.size[2] {
                        for x in 0..s.size[0] {
                            let k = s.at(x, y, z);
                            s.cells[k] = old.cells[old.at(s.size[0] - 1 - x, y, z)];
                        }
                    }
                }
                s.anchor[0] = s.size[0] - 1 - s.anchor[0];
            }),
            "copyup" if grown.is_none() => {
                let y = self.layer;
                self.edit("Copy the layer up", |s| {
                    if y + 1 < s.size[1] {
                        for z in 0..s.size[2] {
                            for x in 0..s.size[0] {
                                let k = s.at(x, y + 1, z);
                                s.cells[k] = s.cells[s.at(x, y, z)];
                            }
                        }
                    }
                });
                self.layer += 1;
            }
            "freeze" => self.edit("Freeze", |s| s.freeze()),
            "another" => {
                if let Some(r) = grown {
                    let seed = Rng::new(r.seed ^ 0x5eed).next() % 1_000_000;
                    self.edit("Another seed", |s| *s = grow(&Recipe { seed, ..r }));
                }
            }
            "vary" => {
                if let Some(r) = grown {
                    self.walls += 1;
                    self.wall = Some(Wall::new(r, 6, 0.05, 0x57 + self.walls, &vary));
                }
            }
            _ => {}
        }
    }

    fn more_keys(&self) -> Vec<(&'static str, &'static str)> {
        vec![("Left click, drag", "the tool on the slice"), ("Right click", "leave the world's block"), ("Right drag", "turn the view"), ("Wheel", "zoom the view")]
    }

    fn work(&mut self, ui: &mut Ui, app: &mut App) {
        if !ui.input(|i| i.pointer.any_down()) {
            (self.merging, self.stroke) = (None, false);
        }
        app.view.land_only = true;
        app.view.controls(ui);
        let Some(name) = self.open.clone() else {
            look::weak(ui, "No structure: New makes one");
            return;
        };
        if self.aimed != self.open {
            self.aim(app);
            self.aimed = self.open.clone();
        }
        self.feed_view(app);
        let s = self.docs[&name].s.clone();
        ui.horizontal(|ui| {
            ui.label(RichText::new(&name).strong());
            look::weak(ui, format!("{} x {} x {} blocks, {} set", s.size[0], s.size[1], s.size[2], s.blocks(&app.reg).len()));
        });
        let total = ui.available_height();
        let cols = colours(&s, app);
        app.view.left_turns = false;
        // the view over the slice, the bar between them dragged
        app.view.ui(ui, look::split(ui, "view", 0.55, total), |p, _, r| sketch(p, r.shrink(GAP), &s, &cols));
        look::split_bar(ui, "view", 0.55, total);
        let h = ui.available_height();
        self.slice(ui, app, h);
    }

    fn inspector(&mut self, ui: &mut Ui, app: &mut App) {
        let cmds = self.commands(app);
        let button = |ui: &mut Ui, me: &mut Self, app: &mut App, id: &str, label: &str| {
            let c = cmds.iter().find(|c| c.id == id).unwrap();
            let r = ui.button(label);
            crate::drive::mark(ui.ctx(), format!("btn:{id}"), r.rect);
            if look::tip(r, &c.help, &c.keys()).clicked() {
                me.run(app, id);
            }
        };
        if let Some(mut sec) = look::section(ui, "Structure") {
            let ui = &mut *sec;
            ui.horizontal_wrapped(|ui| {
                for (id, label) in [("new", "New")] {
                    button(ui, self, app, id, label);
                }
            });
        }
        let Some(d) = self.doc() else { return };
        let s = d.s.clone();
        let Some(r) = s.grow.clone() else {
            if let Some(mut sec) = look::section(ui, "Tools") {
                let ui = &mut *sec;
                ui.horizontal_wrapped(|ui| {
                    for (t, name, help, key) in TOOLS {
                        if look::tip(look::chip(ui, self.tool == t, name), help, key).clicked() {
                            self.tool = t;
                        }
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    button(ui, self, app, "mirror", "Mirror");
                    button(ui, self, app, "copyup", "Copy layer up");
                    button(ui, self, app, "copy", "Copy");
                    button(ui, self, app, "paste", "Paste");
                });
            }
            if let Some(mut sec) = look::section(ui, "Box") {
                let ui = &mut *sec;
                let mut size = s.size;
                let mut anchor = s.anchor;
                let (mut ch_size, mut ch_anchor) = (false, false);
                look::row(ui, "size", |ui| {
                    for v in &mut size {
                        ui.spacing_mut().interact_size.x = 40.0;
                        ch_size |= ui.add(egui::DragValue::new(v).range(1..=MAX_SIDE).speed(0.1)).on_hover_text("The box in blocks: across (x), up (layers), deep (z)").changed();
                    }
                });
                look::row(ui, "anchor", |ui| {
                    for (i, v) in anchor.iter_mut().enumerate() {
                        ui.spacing_mut().interact_size.x = 40.0;
                        ch_anchor |= ui.add(egui::DragValue::new(v).range(0..=s.size[i] - 1).speed(0.1)).on_hover_text("The cell set into the place it stands on (the circle on the slice)").changed();
                    }
                });
                if ch_size {
                    self.edit_merged("Size", |s| Self::resize(s, size));
                }
                if ch_anchor {
                    self.edit_merged("Anchor", |s| s.anchor = anchor);
                }
            }
            if let Some(mut sec) = look::section(ui, "Blocks") {
                let ui = &mut *sec;
                egui::ScrollArea::vertical().id_salt("blocks").max_height(ui.available_height().max(120.0)).show(ui, |ui| {
                    for b in app.reg.placeable() {
                        let c = Color32::from_rgb((b.colour[0] * 255.0) as u8, (b.colour[1] * 255.0) as u8, (b.colour[2] * 255.0) as u8);
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                            ui.painter().rect_filled(r, 2.0, c);
                            let used = s.key.values().any(|n| n == &b.name);
                            let text = if used { RichText::new(&b.name).strong() } else { RichText::new(&b.name) };
                            if ui.selectable_label(self.block == b.name, text).on_hover_text("The block the tools place").clicked() {
                                self.block = b.name.clone();
                            }
                        });
                    }
                });
            }
                return;
        };
        // a grown one: the recipe
        if let Some(mut sec) = look::section(ui, "Recipe") {
            let ui = &mut *sec;
            ui.horizontal_wrapped(|ui| {
                button(ui, self, app, "freeze", "Freeze");
            });
            let mut nr = r.clone();
            look::row(ui, "seed", |ui| {
                ui.add(egui::DragValue::new(&mut nr.seed).speed(1.0)).on_hover_text("The seed: the same seed grows the same blocks");
                let c = cmds.iter().find(|c| c.id == "another").unwrap();
                let rb = ui.button("Another");
                crate::drive::mark(ui.ctx(), "another", rb.rect);
                if look::tip(rb, &c.help, &c.keys()).clicked() {
                    self.run(app, "another");
                }
            });
            for (n, lo, hi, unit) in RANGES {
                let mut v = r.get(n);
                let whole = n != "leaf_density";
                let mut sl = egui::Slider::new(&mut v, lo..=hi);
                sl = if whole { sl.step_by(1.0).fixed_decimals(0) } else { sl.fixed_decimals(2) };
                if !unit.is_empty() {
                    sl = sl.suffix(format!(" {}", if unit == "degrees" { "deg" } else { unit }));
                }
                let rr = look::row(ui, &n.replace('_', " "), |ui| ui.add(sl));
                crate::drive::mark(ui.ctx(), format!("recipe:{n}"), rr.rect);
                if rr.changed() {
                    nr.set(n, v);
                }
            }
            look::row(ui, "crown", |ui| {
                for (c, label) in [(Crown::Round, "round"), (Crown::Cone, "cone"), (Crown::Flat, "flat")] {
                    if look::tip(look::chip(ui, nr.crown == c, label), "The crown's shape", "").clicked() {
                        nr.crown = c;
                    }
                }
            });
            for (what, field) in [("wood", 0), ("leaves", 1)] {
                let cur = if field == 0 { nr.wood.clone() } else { nr.leaves.clone() };
                look::row(ui, what, |ui| egui::ComboBox::from_id_salt(what).selected_text(&cur).width((ui.available_width() - 8.0).max(60.0)).show_ui(ui, |ui| {
                    for b in app.reg.placeable() {
                        if ui.selectable_label(b.name == cur, &b.name).clicked() {
                            if field == 0 {
                                nr.wood = b.name.clone();
                            } else {
                                nr.leaves = b.name.clone();
                            }
                        }
                    }
                }));
            }
            if nr != r {
                self.regrow("Recipe", nr);
            }
        }
        if self.wall.is_none() {
            if let Some(mut sec) = look::section(ui, "Variations") {
                let ui = &mut *sec;
                if look::tip(ui.button("Six seeds"), "A wall of six seeds of this recipe; a click takes one", "V").clicked() {
                    self.run(app, "vary");
                }
            }
            return;
        }
        let chosen = self.wall.as_ref().and_then(|w| w.items.iter().position(|i| *i == r));
        let mut act = wallui::Act::None;
        let reg_cols = |t: &Recipe| {
            let g = grow(t);
            let c = colours(&g, app);
            (g, c)
        };
        if let Some(w) = &mut self.wall {
            // three across when the inspector is wide enough for the wall's labels, else two
            let n = if ui.available_width() >= 360.0 { 3.0 } else { 2.0 };
            let cell = Vec2::new(((ui.available_width() - (n - 1.0) * GAP) / n).floor().max(60.0), 90.0);
            act = wallui::show(ui, w, chosen, cell, |p, rect, t| {
                let (g, c) = reg_cols(t);
                sketch(p, rect, &g, &c);
            });
        }
        let gen: crate::wall::Gen<Recipe> = &vary;
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
        match self.doc().map(|d| d.s.grow.is_some()) {
            Some(true) => "Grown: the recipe's numbers and the seed grow it; N another seed, V a wall of six; Freeze to edit by hand".into(),
            _ => "A adds, R removes (the world stays), X clears to air, B a box, G fills; right click removes; PgUp PgDn the layer, M mirrors, C copies the layer up".into(),
        }
    }

    fn history(&self, _app: &App) -> Option<(Vec<String>, Vec<String>)> {
        let d = self.doc()?;
        Some((d.hist.undo.iter().map(|s| s.name.clone()).collect(), d.hist.redo.iter().rev().map(|s| s.name.clone()).collect()))
    }

    fn back_to(&mut self, _app: &mut App, n: usize) {
        if let Some(d) = self.open.as_ref().and_then(|k| self.docs.get_mut(k)) {
            d.hist.back_to(&mut d.s, n);
        }
    }

    fn refresh(&mut self, app: &App) {
        self.read_disk(app);
    }

    fn unsaved(&self) -> Vec<String> {
        self.docs.iter().filter(|(_, d)| d.dirty()).map(|(n, _)| format!("{DIR}/{n}.toml")).collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_copied_rectangle_pastes_into_another_structure_by_block_name_and_stops_at_the_box() {
        let mut a = Structure::empty([4, 2, 3]);
        let (st, lg) = (a.letter("stone").unwrap(), a.letter("log").unwrap());
        for (x, z, v) in [(1, 0, st), (2, 0, lg), (1, 1, CLEAR)] {
            let k = a.at(x, 1, z);
            a.cells[k] = v;
        }
        let p = copy(&a, 1, (2, 1), (1, 0));
        assert_eq!(p, Piece { w: 2, cells: ["stone", "log", "_", "."].map(String::from).to_vec() }, "either corner first");
        // another structure, where log already has the first letter
        let mut b = Structure::empty([3, 1, 3]);
        let lg_b = b.letter("log").unwrap();
        let k = b.at(2, 0, 2);
        b.cells[k] = lg_b;
        paste(&mut b, &p, 2, 0, 1);
        let st_b = b.letter("stone").unwrap();
        assert_ne!(st_b, lg_b);
        assert_eq!(b.cells[b.at(2, 0, 1)], st_b);
        assert_eq!(b.cells[b.at(2, 0, 2)], CLEAR, "a copied cell replaces what was there, air too");
        assert_eq!(b.cells.iter().filter(|c| **c != KEEP).count(), 2, "the column past the box is dropped");
    }
}
