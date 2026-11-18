//! The Maps page (docs/design/studio-maps.md, ADR draft-studio-maps): a map's node graph on `egui::Scene` to
//! wire and tune, the preview of the chosen or pinned node, its numbers. The map, its checks and its working
//! out are `map`'s; this draws and edits them.
use super::look::{self, GAP};
use super::{App, Cmd, FileRow, Page, Thumb, DRAFTS};
use crate::art::{self, Doc, History, Img};
use crate::wall::Wall;
use super::wall as wall_ui;
use crate::map::{self, Data, Graph, Map, Pin, SPECS};
use crate::shell::Flags;
use eframe::egui::{self, emath::TSTransform, Align2, Color32, FontId, Id, Key, Modifiers, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::sync::mpsc::Receiver;

mod make;
mod motion;

pub const DIR: &str = "data/maps";
/// The side of a node's picture as worked out (the preview's stand-in while its full size is on the way).
const THUMB: usize = 64;
const GROUPS: [(&str, Color32); 5] = [
    ("Make", Color32::from_rgb(82, 140, 104)),
    ("Change", Color32::from_rgb(96, 112, 160)),
    ("Combine", Color32::from_rgb(150, 110, 80)),
    ("Turn into", Color32::from_rgb(140, 92, 140)),
    ("Out", Color32::from_rgb(110, 110, 110)),
];

/// A pin's colour is its kind of data (Blender's): grey a number, yellow a colour, purple a direction.
fn kind_colour(k: Data) -> Color32 {
    match k {
        Data::Col => Color32::from_rgb(225, 200, 60),
        Data::Dir => Color32::from_rgb(165, 110, 235),
        _ => Color32::from_gray(165),
    }
}

// a node's box: its band, its picture, then a row each for its words, its numbers and its input pins
const W: f32 = 210.0;
const HEAD: f32 = 26.0;
const PIC: f32 = 72.0;
const ROW_H: f32 = 24.0;
/// A pin is drawn 12 points across and hit 20 across.
const PIN_R: f32 = 6.0;
const REACH: f32 = 10.0;

/// A node's box in the graph, its output pin and its input pins.
pub fn geom(n: &map::Node) -> (Rect, Pos2, Vec<Pos2>) {
    let (top, s) = (Pos2::new(n.at[0], n.at[1]), n.spec());
    let first = s.words.len() + s.nums.len();
    let rows = first + s.pins.len();
    let r = Rect::from_min_size(top, Vec2::new(W, HEAD + PIC + GAP + rows as f32 * ROW_H));
    let ins = (0..s.pins.len()).map(|p| Pos2::new(r.min.x, row_rect(r, first + p).center().y)).collect();
    (r, Pos2::new(r.max.x, top.y + HEAD + PIC * 0.5), ins)
}

/// The part of the graph to show so the nodes in `b` fill about 80 % of an `area` (the Scene fits it in),
/// never zoomed in past 1.5.
fn framed(b: Rect, area: Vec2) -> Rect {
    let z = (area.x * 0.8 / b.width()).min(area.y * 0.8 / b.height()).min(1.5);
    Rect::from_center_size(b.center(), area / z)
}

/// The loop's frame `clock` seconds in.
fn loop_frame(clock: f64, frames: usize, seconds: f32) -> usize {
    (clock / seconds as f64 * frames as f64) as usize % frames
}

/// A node's k-th row under its picture.
fn row_rect(r: Rect, k: usize) -> Rect {
    Rect::from_min_size(Pos2::new(r.min.x + 12.0, r.min.y + HEAD + PIC + GAP * 0.5 + k as f32 * ROW_H + 2.0), Vec2::new(W - 22.0, ROW_H - 4.0))
}

/// A number on a node as Blender draws one: its name left, its value right, over a rail lit up to it; a drag
/// sideways changes it. The new value while it is dragged.
fn num_row(ui: &Ui, r: Rect, id: Id, name: &str, v: f32, (min, max, whole): (f32, f32, bool), legible: bool) -> (egui::Response, Option<f32>) {
    let resp = ui.interact(r, id, Sense::click_and_drag());
    let p = ui.painter();
    p.rect_filled(r, 4.0, Color32::from_gray(28));
    let t = ((v - min) / (max - min)).clamp(0.0, 1.0);
    let lit = if resp.hovered() || resp.dragged() { 0.6 } else { 0.4 };
    p.rect_filled(Rect::from_min_size(r.min, Vec2::new(r.width() * t, r.height())), 4.0, look::ACCENT.gamma_multiply(lit));
    if legible {
        let font = FontId::proportional(look::BODY);
        p.text(r.left_center() + Vec2::new(6.0, 0.0), Align2::LEFT_CENTER, name, font.clone(), Color32::from_gray(225));
        p.text(r.right_center() - Vec2::new(6.0, 0.0), Align2::RIGHT_CENTER, if whole { format!("{v:.0}") } else { format!("{v:.2}") }, font, Color32::WHITE);
    }
    let start = ui.ctx().data_mut(|d| {
        if resp.drag_started() {
            d.insert_temp(id, v);
        }
        d.get_temp::<f32>(id)
    });
    let new = resp.total_drag_delta().zip(start).map(|(d, s)| {
        // a long range (a seed) a step every 4 points, else the rail's width is the whole range
        let per = if whole && max - min > 64.0 { 0.25 } else { (max - min) / r.width() };
        let x = (s + d.x * per).clamp(min, max);
        if whole { x.round() } else { x }
    });
    (resp, new.filter(|x| *x != v))
}

/// A choice as a drop-down; the word picked.
fn word_box(ui: &mut Ui, salt: impl std::hash::Hash + std::fmt::Debug, cur: &str, choices: &[String], width: f32) -> (egui::Response, Option<String>) {
    let mut set = None;
    let r = egui::ComboBox::from_id_salt(salt).width(width).selected_text(if cur.is_empty() { "chosen colours" } else { cur }).show_ui(ui, |ui| {
        for w in choices {
            let r = ui.selectable_label(w == cur, w);
            if ui.is_rect_visible(r.rect) {
                crate::drive::mark(ui.ctx(), format!("choice:{w}"), r.rect);
            }
            if r.clicked() {
                set = Some(w.clone());
            }
        }
    });
    (r.response, set.filter(|w| w != cur))
}

/// A word's choices: its kind's list, or the palette's ramps.
fn choices(words: &[&str], pal: &engine::palette::Palette) -> Vec<String> {
    if words.is_empty() { pal.ramps().into_iter().map(|r| r.0).collect() } else { words.iter().map(|w| w.to_string()).collect() }
}

/// A pin: a node's output (`pin` None; the out has none) or one of its inputs.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Hit {
    pub node: usize,
    pub pin: Option<usize>,
}

/// The pin under a point of the graph, the last drawn node's first.
pub fn pin_at(m: &Map, p: Pos2) -> Option<Hit> {
    m.nodes.iter().enumerate().rev().find_map(|(i, n)| {
        let (_, o, ins) = geom(n);
        if n.spec().kind != "out" && o.distance(p) <= REACH {
            return Some(Hit { node: i, pin: None });
        }
        ins.iter().position(|q| q.distance(p) <= REACH).map(|k| Hit { node: i, pin: Some(k) })
    })
}

/// A wire drawn between two pins either way round, from an output to an input; its undo step's name. A
/// circle or a kind the pin cannot take is refused and the map stays.
pub fn connect(m: &mut Map, a: Hit, b: Hit) -> Result<String, String> {
    let (o, i) = match (a.pin, b.pin) {
        (None, Some(_)) => (a, b),
        (Some(_), None) => (b, a),
        _ => return Err("a wire goes from an output (a node's right) to an input (a node's left)".into()),
    };
    m.wire(o.node, i.node, i.pin.unwrap())?;
    Ok(format!("Wire {} to {}", m.nodes[o.node].id, m.nodes[i.node].id))
}

/// A new node `n` plugged into the pin a wire was let go from: an output into its first pin that takes it,
/// an input from its output.
fn plug(m: &mut Map, from: Hit, n: usize) -> Result<(), String> {
    match from.pin {
        Some(p) => m.wire(n, from.node, p),
        None => (0..m.nodes[n].pins.len()).find_map(|p| m.wire(from.node, n, p).ok()).ok_or("no pin of it takes that".into()),
    }
}

struct MDoc {
    m: Map,
    hist: History<Map>,
    /// The map as saved (its text); none: not on disk yet.
    saved: Option<String>,
}
impl MDoc {
    fn dirty(&self) -> bool {
        self.saved.as_deref() != Some(self.m.write().as_str())
    }
}

enum Drag {
    None,
    /// Nodes moved: where the pointer started and where each started.
    Move(Pos2, Vec<(usize, [f32; 2])>),
    Wire(Hit),
    Box(Pos2),
}

/// The Add a node box: where the node goes (in the graph), where the box stands (on screen), what is typed,
/// and the pin a wire was let go from.
struct Adding {
    at: Pos2,
    screen: Pos2,
    query: String,
    from: Option<Hit>,
    focus: bool,
}

/// The preview's full size worked out on a worker: (node's stamp, frame, size).
type FullKey = (u64, usize, usize);

pub struct Maps {
    docs: BTreeMap<String, MDoc>,
    open: Option<String>,
    graph: Option<Graph>,
    chosen: Vec<usize>,
    pinned: Option<usize>,
    /// The part of the graph in view (the Scene's).
    view: Rect,
    frame_all: bool,
    /// The view framed by itself and the area it was framed for; gone once the view is moved by hand.
    fitted: Option<(Rect, Vec2)>,
    drag: Drag,
    adding: Option<Adding>,
    /// The preview: "map", "tiled", "particles" or "sway".
    mode: &'static str,
    playing: bool,
    /// Seconds played; the input's time last frame.
    clock: f64,
    last_time: Option<f64>,
    player: motion::Player,
    dots: Vec<motion::Dot>,
    speed: f32,
    strength: f32,
    /// The loop's frame shown, as a texture: (its loop, the frame).
    strip_tex: Option<((motion::Key, usize), egui::TextureHandle)>,
    frame: usize,
    merging: Option<String>,
    /// Each node's picture as a texture, by what it shows: (stamp, frame).
    tex: HashMap<usize, ((u64, usize), egui::TextureHandle)>,
    full: Option<(FullKey, egui::TextureHandle)>,
    job: Option<(FullKey, Receiver<Vec<u8>>)>,
    /// The files column's pictures, by the map's text.
    thumbs: RefCell<HashMap<String, (String, Img)>>,
    /// Where the graph was last drawn: its rect on screen and the graph to screen transform.
    area: (Rect, TSTransform),
    /// Use as a block's texture: the block, the face (0 all, 1 top, 2 side, 3 bottom), how many variants, the
    /// ramp a number runs along; and the texture it would write, by what it was made from.
    use_block: String,
    use_face: usize,
    use_variants: usize,
    use_ramp: String,
    use_made: Option<(String, Result<Img, String>)>,
    export_to: String,
    /// New map's list is open: its name box.
    new_list: Option<String>,
    wall: Option<Wall<Map>>,
    wall_tex: HashMap<String, egui::TextureHandle>,
    /// A variation picked on the wall: shown in the graph and preview until Use or Cancel.
    pending: Option<Map>,
    /// A number on a node being typed: its row, the text, and whether it still has to take the keys.
    typing: Option<(Id, String, bool)>,
}

fn read_dir(pal: &engine::palette::Palette) -> BTreeMap<String, Map> {
    std::fs::read_dir(DIR)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().strip_suffix(".toml")?.to_string();
            // a file the check refuses is left out here and listed under Problems
            let m = map::parse(&format!("{DIR}/{n}.toml"), &std::fs::read_to_string(e.path()).ok()?, pal).ok()?;
            Some((n, m))
        })
        .collect()
}

impl Maps {
    pub fn new(app: &App) -> Maps {
        let docs: BTreeMap<String, MDoc> = read_dir(&app.pal).into_iter().map(|(n, m)| (n, MDoc { saved: Some(m.write()), m, hist: History::default() })).collect();
        Maps {
            open: docs.keys().next().cloned(),
            docs,
            graph: None,
            chosen: vec![],
            pinned: None,
            view: Rect::ZERO,
            frame_all: true,
            fitted: None,
            drag: Drag::None,
            adding: None,
            mode: "map",
            playing: false,
            clock: 0.0,
            last_time: None,
            player: motion::Player::default(),
            dots: motion::dots(300),
            speed: 0.25,
            strength: 1.0,
            strip_tex: None,
            frame: 0,
            merging: None,
            tex: HashMap::new(),
            full: None,
            job: None,
            thumbs: RefCell::default(),
            area: (Rect::ZERO, TSTransform::IDENTITY),
            use_block: String::new(),
            use_face: 0,
            use_variants: 1,
            use_ramp: String::new(),
            use_made: None,
            export_to: String::new(),
            new_list: None,
            wall: None,
            wall_tex: HashMap::new(),
            pending: None,
            typing: None,
        }
    }

    fn doc(&self) -> Option<&MDoc> {
        self.open.as_ref().and_then(|n| self.docs.get(n))
    }

    /// One named undo step; a drag of a slider or a node is one step however long (`merged`).
    fn edit(&mut self, name: &str, merged: bool, f: impl FnOnce(&mut Map)) {
        let merge = self.merging.as_deref() == Some(name);
        let Some(d) = self.open.as_ref().and_then(|n| self.docs.get_mut(n)) else { return };
        let before = d.m.clone();
        f(&mut d.m);
        if d.m != before && !merge {
            d.hist.push(before, name);
        }
        if merged {
            self.merging = Some(name.into());
        }
    }

    /// The graph as the open map is now; the chosen and pinned nodes kept in range.
    fn sync(&mut self) {
        let Some(m) = self.pending.clone().or_else(|| self.doc().map(|d| d.m.clone())) else { return };
        let n = m.nodes.len();
        match &mut self.graph {
            Some(g) => g.update(&m),
            None => self.graph = Some(Graph::new(m)),
        }
        self.chosen.retain(|&i| i < n);
        self.pinned = self.pinned.filter(|&i| i < n);
        self.frame = self.frame.min(self.graph.as_ref().map_or(1, |g| g.map.frames) - 1);
    }

    /// A node's picture at `THUMB` px as a texture, worked out again only when it changed.
    fn texture(&mut self, ctx: &egui::Context, node: usize) -> egui::TextureId {
        let g = self.graph.as_mut().expect("synced");
        let key = (g.stamp[node], self.frame);
        if let Some((_, t)) = self.tex.get(&node).filter(|(k, _)| *k == key) {
            return t.id();
        }
        let kind = g.kinds[node];
        let img = egui::ColorImage::from_rgba_unmultiplied([THUMB, THUMB], &map::rgba(g.pic(node, self.frame, THUMB), kind));
        match self.tex.get_mut(&node) {
            Some((k, t)) => {
                t.set(img, egui::TextureOptions::NEAREST);
                *k = key;
                t.id()
            }
            None => {
                let t = ctx.load_texture(format!("map node {node}"), img, egui::TextureOptions::NEAREST);
                let id = t.id();
                self.tex.insert(node, (key, t));
                id
            }
        }
    }

    /// The node the preview shows: the pinned one, else the chosen one, else the out.
    fn shown(&self) -> Option<usize> {
        let g = self.graph.as_ref()?;
        self.pinned.or(self.chosen.first().copied()).or(Some(g.out()))
    }

    /// The preview's picture: at the map's size from the worker once it is done, the node's small picture
    /// till then; the window never waits for it.
    fn preview_texture(&mut self, ctx: &egui::Context, node: usize) -> egui::TextureId {
        let (key, m) = {
            let g = self.graph.as_ref().expect("synced");
            ((g.stamp[node], self.frame, g.map.size), g.map.clone())
        };
        self.poll(ctx);
        if let Some((k, t)) = &self.full {
            if *k == key {
                return t.id();
            }
        }
        if self.job.is_none() {
            let (tx, rx) = std::sync::mpsc::channel();
            let (frame, size) = (self.frame, m.size);
            std::thread::spawn(move || {
                let mut g = Graph::new(m);
                let kind = g.kinds[node];
                let _ = tx.send(map::rgba(g.pic(node, frame, size), kind));
            });
            self.job = Some((key, rx));
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
        self.texture(ctx, node)
    }

    /// The worker's picture, once it is there.
    fn poll(&mut self, ctx: &egui::Context) {
        let Some((key, rx)) = &self.job else { return };
        match rx.try_recv() {
            Ok(px) => {
                let img = egui::ColorImage::from_rgba_unmultiplied([key.2, key.2], &px);
                let key = *key;
                match &mut self.full {
                    Some((k, t)) => {
                        t.set(img, egui::TextureOptions::NEAREST);
                        *k = key;
                    }
                    None => self.full = Some((key, ctx.load_texture("map preview", img, egui::TextureOptions::NEAREST))),
                }
                self.job = None;
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => self.job = None,
            Err(_) => {}
        }
    }

    fn save(&mut self, app: &mut App) {
        let Some(name) = self.open.clone() else { return };
        let text = self.docs[&name].m.write();
        let path = format!("{DIR}/{name}.toml");
        // read back as the check reads it, before it is written
        if let Err(e) = map::parse(&path, &text, &app.pal) {
            return app.refuse(format!("Not saved: {e}"));
        }
        let _ = std::fs::create_dir_all(DIR);
        match std::fs::write(&path, &text) {
            Ok(()) => {
                self.docs.get_mut(&name).unwrap().saved = Some(text);
                app.write_drafts(art::set_draft(&std::fs::read_to_string(DRAFTS).unwrap_or_default(), &path, false));
                app.note(&path);
                app.toast(format!("Saved {path}"));
                app.check();
            }
            Err(e) => app.refuse(format!("Not saved: {path}: {e}")),
        }
    }

    /// A copy of a map under a new name, open and unsaved.
    fn new_from(&mut self, from: &str) {
        let Some(m) = self.docs.get(from).map(|d| d.m.clone()) else { return };
        let name = self.free_name(&format!("{from} 2"));
        self.docs.insert(name.clone(), MDoc { m, hist: History::default(), saved: None });
        self.open_map(&name);
    }

    fn taken(&self, name: &str) -> bool {
        self.docs.contains_key(name) || std::path::Path::new(&format!("{DIR}/{name}.toml")).exists()
    }

    /// `base`, or `base 2`, `base 3`... the first no map has.
    fn free_name(&self, base: &str) -> String {
        std::iter::once(base.to_string()).chain((2..).map(|k| format!("{} {k}", base.trim_end_matches(" 2")))).find(|n| !self.taken(n)).unwrap()
    }

    /// A new map from a starter under the name typed (else the starter's), open and unsaved. Not a draft: the
    /// developer made it.
    fn make_from(&mut self, app: &mut App, starter: &str) {
        let typed = self.new_list.as_deref().unwrap_or("").trim().to_string();
        let name = if typed.is_empty() { self.free_name(starter) } else { typed };
        if self.taken(&name) {
            return app.refuse(format!("A map named '{name}' is there already: type another name"));
        }
        if name.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|']) {
            return app.refuse(format!("'{name}' cannot be a file name"));
        }
        match make::starter(starter, &app.pal) {
            Ok(m) => {
                self.docs.insert(name.clone(), MDoc { m, hist: History::default(), saved: None });
                self.open_map(&name);
                app.toast(format!("New map {name} from {starter} (unsaved)"));
            }
            Err(e) => app.refuse(e),
        }
    }

    fn open_map(&mut self, name: &str) {
        self.open = Some(name.into());
        (self.graph, self.chosen, self.pinned, self.frame_all, self.adding) = (None, vec![], None, true, None);
        (self.new_list, self.wall, self.pending, self.typing, self.use_made) = (None, None, None, None, None);
        self.export_to = format!("exports/{name}.png");
        self.tex.clear();
        self.wall_tex.clear();
    }

    /// New map's list: the starters with their pictures and what each is for, and the name box.
    fn new_list_ui(&mut self, ui: &mut Ui, app: &mut App) {
        let mut name = self.new_list.clone().unwrap_or_default();
        look::heading(ui, "New map");
        look::weak(ui, if self.open.is_none() { "No map yet: a click on a starter makes your first one" } else { "A click on a starter makes the new map, open and unsaved" });
        let mut close = false;
        ui.horizontal(|ui| {
            ui.label("name");
            let r = ui.add(egui::TextEdit::singleline(&mut name).hint_text("the starter's name").desired_width(220.0));
            crate::drive::mark(ui.ctx(), "new:name", r.rect);
            look::tip(r, "The new map's name (its file data/maps/<name>.toml); left empty, the starter's name", "");
            if self.open.is_some() && look::tip(ui.button("Cancel"), "Back to the open map", "Escape").clicked() {
                close = true;
            }
        });
        ui.add_space(GAP);
        let mut pick = None;
        for (st, what, _) in make::STARTERS {
            let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width().min(560.0), 60.0), Sense::click());
            crate::drive::mark(ui.ctx(), format!("starter:{}", st.replace(' ', "_")), r);
            let img = {
                let mut thumbs = self.thumbs.borrow_mut();
                let key = format!("starter {st}");
                if !thumbs.contains_key(&key) {
                    if let Ok(m) = make::starter(st, &app.pal) {
                        thumbs.insert(key.clone(), (String::new(), make::thumb(&m, 16)));
                    }
                }
                thumbs.get(&key).map(|t| t.1.clone())
            };
            let p = ui.painter();
            if resp.hovered() {
                p.rect_filled(r, 6.0, ui.visuals().widgets.hovered.weak_bg_fill);
            }
            if let Some(img) = img {
                look::draw(p, &img, 0, r.min + Vec2::new(6.0, 6.0), 3.0, false);
            }
            p.text(r.min + Vec2::new(64.0, 18.0), Align2::LEFT_CENTER, st, FontId::proportional(look::BODY), Color32::WHITE);
            p.text(r.min + Vec2::new(64.0, 40.0), Align2::LEFT_CENTER, what, FontId::proportional(look::SMALL), ui.visuals().weak_text_color());
            if look::tip(resp, &format!("A new map from {st}: {what}"), "").clicked() {
                pick = Some(st);
            }
        }
        if close || ui.input(|i| i.key_pressed(Key::Escape)) && self.open.is_some() {
            self.new_list = None;
            return;
        }
        self.new_list = Some(name);
        if let Some(st) = pick {
            self.make_from(app, st);
        }
    }

    /// The block's texture Use would write, worked out again only when what it is made from changed.
    fn use_result(&mut self, app: &App) -> Option<(String, Result<Img, String>)> {
        let g = self.graph.as_ref()?;
        if self.use_block.is_empty() {
            return None;
        }
        let file = art::file_name(&self.use_block, self.use_face);
        // the block's size: this face's painting, else any of the block's, else a plain 16
        let side = app.docs.get(&file).or_else(|| (0..4).find_map(|f| app.docs.get(&art::file_name(&self.use_block, f)))).map_or(art::S, |d| d.img.h);
        let key = format!("{}|{file}|{}|{}|{side}", g.map.write(), self.use_variants, self.use_ramp);
        if self.use_made.as_ref().is_none_or(|u| u.0 != key) {
            let ramp: Vec<[u8; 3]> = app.pal.ramp(&self.use_ramp).iter().map(|&i| app.pal.colours[i].rgb).collect();
            self.use_made = Some((key, make::texture(&g.map, side, self.use_variants, &ramp, &app.pal)));
        }
        self.use_made.as_ref().map(|u| (file, u.1.clone()))
    }

    /// The block's own ramp: the palette ramp of the colour nearest the block's.
    fn own_ramp(app: &App, block: &str) -> String {
        let c = app.reg.placeable().find(|d| d.name == block).map_or([0.5; 3], |d| d.colour);
        app.pal.find(crate::tools::snap(&app.pal, c)).and_then(|i| app.pal.colours[i].ramp.clone()).unwrap_or_default()
    }

    fn set_block(&mut self, app: &App, block: &str) {
        self.use_block = block.to_string();
        self.use_ramp = Self::own_ramp(app, block);
    }

    /// Use: the texture written through the Textures page's own save, one undo step there, checked.
    fn use_texture(&mut self, app: &mut App) {
        let Some((file, made)) = self.use_result(app) else { return app.refuse("Choose a block first") };
        let img = match made {
            Ok(img) => img,
            Err(e) => return app.refuse(format!("Not used: {e}")),
        };
        let step = format!("Map {}", self.open.clone().unwrap_or_default());
        match app.docs.get_mut(&file) {
            Some(d) => {
                let before = std::mem::replace(&mut d.img, img);
                if d.img != before {
                    d.hist.push(before, &step);
                }
            }
            None => {
                app.docs.insert(file.clone(), Doc { img, hist: History::default(), saved: None });
            }
        }
        app.save_textures(vec![file]);
    }

    fn use_panel(&mut self, ui: &mut Ui, app: &mut App) {
        let Some(kind) = self.graph.as_ref().map(|g| g.kinds[g.out()]) else { return };
        let blocks: Vec<String> = app.reg.placeable().map(|d| d.name.clone()).collect();
        look::row(ui, "block", |ui| {
            let (r, b) = word_box(ui, "maps use block", if self.use_block.is_empty() { "choose a block" } else { &self.use_block }, &blocks, 150.0);
            crate::drive::mark(ui.ctx(), "use:block", r.rect);
            look::tip(r, "The block whose texture the map becomes", "");
            if let Some(b) = b {
                self.set_block(app, &b);
            }
        });
        look::row(ui, "face", |ui| {
            for (f, label) in ["all", "top", "side", "bottom"].iter().enumerate() {
                let r = look::chip(ui, self.use_face == f, label);
                crate::drive::mark(ui.ctx(), format!("use.face:{f}"), r.rect);
                if look::tip(r, "Which face's file: all is the block's one picture", "").clicked() {
                    self.use_face = f;
                }
            }
        });
        look::row(ui, "variants", |ui| {
            look::tip(ui.add(egui::Slider::new(&mut self.use_variants, 1..=4)), "How many variants: each the same graph with its seeds moved on", "");
        });
        if kind == Data::Num {
            let ramps = choices(&[], &app.pal);
            look::row(ui, "ramp", |ui| {
                let (r, w) = word_box(ui, "maps use ramp", &self.use_ramp, &ramps, 150.0);
                look::tip(r, "The palette ramp the number runs along, dark to light (the block's own to start with)", "");
                if let Some(w) = w {
                    self.use_ramp = w;
                }
            });
        }
        let Some((file, made)) = self.use_result(app) else {
            look::weak(ui, "Choose a block: the map shows beside its texture now, nothing is written before Use");
            return;
        };
        match &made {
            Err(e) => {
                ui.label(RichText::new(e).color(look::BAD));
            }
            Ok(img) => {
                let now = app.docs.get(&file).map(|d| d.img.clone());
                let k = (110.0 / (3 * img.h) as f32).floor().max(1.0);
                let cell = 3.0 * img.h as f32 * k;
                let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), cell + 22.0), Sense::hover());
                let p = ui.painter();
                for (x, label, pic) in [(r.min.x, "now", now.as_ref()), (r.min.x + cell + GAP * 2.0, "after Use", Some(img))] {
                    p.text(Pos2::new(x, r.min.y + 8.0), Align2::LEFT_CENTER, label, FontId::proportional(look::SMALL), ui.visuals().weak_text_color());
                    let at = Pos2::new(x, r.min.y + 20.0);
                    match pic {
                        Some(pic) => {
                            for j in 0..3 {
                                for i in 0..3 {
                                    look::draw(p, pic, art::variant_at([i, j, 0], pic.variants()), at + Vec2::new(i as f32, j as f32) * pic.h as f32 * k, k, false);
                                }
                            }
                        }
                        None => {
                            p.text(at + Vec2::splat(cell / 2.0), Align2::CENTER_CENTER, "none yet", FontId::proportional(look::SMALL), ui.visuals().weak_text_color());
                        }
                    }
                }
                crate::drive::mark(ui.ctx(), "use:shown", r);
            }
        }
        let r = ui.add_enabled(made.is_ok(), egui::Button::new(RichText::new("Use").color(Color32::WHITE)).fill(look::GOOD.gamma_multiply(0.45)));
        crate::drive::mark(ui.ctx(), "btn:use", r.rect);
        if look::tip(r, &format!("Write {file}: one undo step on the Textures page, saved and checked"), "").clicked() {
            self.use_texture(app);
        }
    }

    fn export(&mut self, app: &mut App) {
        let Some(m) = self.doc().map(|d| d.m.clone()) else { return };
        let path = self.export_to.trim().to_string();
        match map::export(&m, &path) {
            Ok((w, h)) => app.toast(format!("Wrote {path}: {w} x {h}")),
            Err(e) => app.refuse(format!("Not written: {e}")),
        }
    }

    fn export_panel(&mut self, ui: &mut Ui, app: &mut App) {
        let r = ui.add(egui::TextEdit::singleline(&mut self.export_to).desired_width(f32::INFINITY));
        crate::drive::mark(ui.ctx(), "export:path", r.rect);
        look::tip(r, "Where the PNG goes, from the repository's folder", "");
        let there = std::path::Path::new(self.export_to.trim()).exists();
        let r = ui.button(if there { "Replace" } else { "Export" });
        crate::drive::mark(ui.ctx(), "btn:export", r.rect);
        if look::tip(r, "A PNG at the map's size: a number grey, a colour as it is, a direction in red and green (middle grey no push); an animated map as one strip of its frames", "").clicked() {
            self.export(app);
        }
    }

    /// The variations wall: the open graph under other seeds.
    fn open_wall(&mut self) {
        let Some(m) = self.doc().map(|d| d.m.clone()) else { return };
        (self.wall, self.pending) = (Some(Wall::new(m, 10, 0.5, 7, &make::variation)), None);
        self.wall_tex.clear();
    }

    fn wall_close(&mut self, keep: bool) {
        if let Some(p) = self.pending.take().filter(|_| keep) {
            self.edit("Variation", false, |m| *m = p);
        }
        self.wall = None;
        self.wall_tex.clear();
    }

    fn wall_ui(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            let r = ui.add_enabled(self.pending.is_some(), egui::Button::new(RichText::new("Use").color(Color32::WHITE)).fill(look::GOOD.gamma_multiply(0.45)));
            crate::drive::mark(ui.ctx(), "btn:wall.use", r.rect);
            if look::tip(r, "The picked seeds into the map: one undo step", "").clicked() {
                self.wall_close(true);
            }
            let r = ui.button("Cancel");
            crate::drive::mark(ui.ctx(), "btn:wall.cancel", r.rect);
            if look::tip(r, "Back to the graph as it was", "Escape").clicked() {
                self.wall_close(false);
            }
            look::weak(ui, if self.pending.is_some() { "Picked: the preview shows it" } else { "A click picks one; the preview shows it" });
        });
        if self.wall.is_none() || ui.input(|i| i.key_pressed(Key::Escape)) {
            return self.wall_close(false);
        }
        let ctx = ui.ctx().clone();
        let w = self.wall.as_mut().unwrap();
        let ids: Vec<egui::TextureId> = w
            .items
            .iter()
            .map(|m| {
                let key = m.write();
                self.wall_tex
                    .entry(key)
                    .or_insert_with(|| {
                        let (kind, p) = make::picture(m, THUMB);
                        ctx.load_texture("map variation", egui::ColorImage::from_rgba_unmultiplied([THUMB, THUMB], &map::rgba(&p, kind)), egui::TextureOptions::NEAREST)
                    })
                    .id()
            })
            .collect();
        let chosen = self.pending.as_ref().and_then(|p| w.items.iter().position(|i| i == p));
        let full = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        match wall_ui::show_with(ui, w, chosen, Vec2::splat(140.0), false, |p, r, i, _| {
            p.image(ids[i], r, full, Color32::WHITE);
        }) {
            wall_ui::Act::Take(i) => self.pending = (i > 0).then(|| w.items[i].clone()),
            wall_ui::Act::More(i) => w.more_like(i, &make::variation),
            wall_ui::Act::Back => {
                w.back();
            }
            wall_ui::Act::Shuffle => w.shuffle(&make::variation),
            wall_ui::Act::Spread => w.fill(10, &make::variation),
            _ => {}
        }
    }

    /// A number row on a node; a double click types into it: Enter takes it (held to its range), Escape or a
    /// click away leaves it. The row's response (none while typed into) and the new value, typed or not.
    fn num_field(&mut self, ui: &mut Ui, rr: Rect, id: Id, name: &str, v: f32, (min, max, whole): (f32, f32, bool), legible: bool) -> (Option<egui::Response>, Option<(f32, bool)>) {
        if let Some((_, text, fresh)) = self.typing.as_mut().filter(|t| t.0 == id) {
            let r = ui.put(rr, egui::TextEdit::singleline(text).font(FontId::proportional(look::BODY)));
            if std::mem::take(fresh) {
                r.request_focus();
            }
            let (enter, esc) = ui.input(|i| (i.key_pressed(Key::Enter), i.key_pressed(Key::Escape)));
            let typed = text.trim().parse::<f32>().ok().filter(|x| x.is_finite());
            if enter || esc || r.lost_focus() {
                self.typing = None;
            }
            let x = typed.filter(|_| enter).map(|x| x.clamp(min, max)).map(|x| if whole { x.round() } else { x });
            return (None, x.map(|x| (x, true)));
        }
        let (resp, new) = num_row(ui, rr, id, name, v, (min, max, whole), legible);
        if resp.double_clicked() {
            self.typing = Some((id, if whole { format!("{v:.0}") } else { format!("{v:.2}") }, true));
        }
        (Some(resp), new.map(|x| (x, false)))
    }

    fn add_node(&mut self, app: &App, kind: &str, at: Pos2, from: Option<Hit>) {
        let pal = app.pal.clone();
        let mut made = None;
        self.edit(&format!("Add {kind}"), false, |m| {
            let n = m.add(kind, [0.0, 0.0]);
            // centred where it was asked for
            let size = geom(&m.nodes[n]).0.size();
            m.nodes[n].at = [at.x - size.x / 2.0, at.y - size.y / 2.0];
            // a Colours node is saved with a ramp: the palette's first to start with
            if kind == "colours" {
                if let Some(r) = pal.ramps().first() {
                    let _ = m.set_word(n, "ramp", &r.0, &pal);
                }
            }
            if let Some(f) = from {
                let _ = plug(m, f, n);
            }
            made = Some(n);
        });
        self.chosen = made.into_iter().collect();
    }

    /// The graph: wires under the nodes, each node its band, picture, pins and the sliders of its unplugged
    /// pins; what the pointer does with them.
    fn graph_ui(&mut self, ui: &mut Ui, pal: &engine::palette::Palette) -> TSTransform {
        let ctx = ui.ctx().clone();
        let tr = ctx.layer_transform_to_global(ui.layer_id()).unwrap_or_default();
        // rows' text and drop-downs hide where they could not be read
        let legible = tr.scaling >= 0.45;
        // a faint dot grid that moves with the graph, so a pan or a zoom is seen
        let seen = ui.clip_rect();
        let mut step = 32.0;
        while seen.area() / (step * step) > 6000.0 {
            step *= 2.0;
        }
        for gx in (seen.min.x / step).floor() as i32..=(seen.max.x / step).ceil() as i32 {
            for gy in (seen.min.y / step).floor() as i32..=(seen.max.y / step).ceil() as i32 {
                ui.painter().circle_filled(Pos2::new(gx as f32 * step, gy as f32 * step), 1.2 / tr.scaling.max(0.5), Color32::from_gray(52));
            }
        }
        let mark = |name: String, r: Rect| crate::drive::mark(&ctx, name, tr * r);
        let pointer = ctx.input(|i| i.pointer.latest_pos()).map(|p| tr.inverse() * p);
        let m = self.doc().expect("open").m.clone();
        let kinds = self.graph.as_ref().expect("synced").kinds.clone();
        let p = ui.painter().clone();
        let wire = |a: Pos2, b: Pos2, c: Color32| {
            let dx = ((b.x - a.x).abs() * 0.5).max(40.0);
            p.add(egui::epaint::CubicBezierShape::from_points_stroke([a, a + Vec2::new(dx, 0.0), b - Vec2::new(dx, 0.0), b], false, Color32::TRANSPARENT, Stroke::new(2.5, c)));
        };
        let geoms: Vec<_> = m.nodes.iter().map(geom).collect();
        for (i, n) in m.nodes.iter().enumerate() {
            for (k, pin) in n.pins.iter().enumerate() {
                if let Pin::Wire(j) = pin {
                    wire(geoms[*j].1, geoms[i].2[k], kind_colour(kinds[*j]));
                }
            }
        }
        let font = FontId::proportional(look::BODY);
        for (i, n) in m.nodes.iter().enumerate() {
            let s = n.spec();
            let (r, out, ins) = &geoms[i];
            let body = ui.interact(*r, ui.scope_id().with(("node", i)), Sense::click_and_drag());
            mark(format!("node:{}", n.id), *r);
            let on = self.chosen.contains(&i);
            p.rect_filled(*r, 5.0, Color32::from_rgb(44, 46, 52));
            let band = Rect::from_min_size(r.min, Vec2::new(W, HEAD));
            let gc = GROUPS.iter().find(|g| g.0 == s.group).map_or(Color32::GRAY, |g| g.1);
            p.rect_filled(band, egui::CornerRadius { nw: 5, ne: 5, sw: 0, se: 0 }, gc.gamma_multiply(0.75));
            // its name strong, its kind weak after it
            let mut job = egui::text::LayoutJob::simple_singleline(n.id.clone(), font.clone(), Color32::WHITE);
            if n.id != s.kind {
                job.append(s.kind, 8.0, egui::TextFormat::simple(font.clone(), Color32::from_gray(170)));
            }
            job.wrap = egui::text::TextWrapping::truncate_at_width(W - 34.0);
            let g = p.layout_job(job);
            p.galley(band.left_center() + Vec2::new(GAP, -g.size().y / 2.0), g, Color32::WHITE);
            p.rect_stroke(*r, 5.0, if on { Stroke::new(2.0, look::ACCENT) } else { Stroke::new(1.0, Color32::from_gray(70)) }, egui::StrokeKind::Outside);
            let pic = Rect::from_min_size(Pos2::new(r.center().x - PIC / 2.0, r.min.y + HEAD + GAP * 0.5), Vec2::splat(PIC - GAP * 0.5));
            let tex = self.texture(&ctx, i);
            p.image(tex, pic, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            // the pin button: this node stays in the preview
            let pb = Rect::from_center_size(band.right_center() - Vec2::new(14.0, 0.0), Vec2::splat(18.0));
            let pr = ui.interact(pb, ui.scope_id().with(("pinbtn", i)), Sense::click());
            mark(format!("pinbtn:{}", n.id), pb);
            let pinned = self.pinned == Some(i);
            let pc = if pinned { Color32::WHITE } else if pr.hovered() { Color32::from_gray(220) } else { Color32::from_gray(150) };
            p.circle(pb.center() - Vec2::new(0.0, 3.0), 4.0, if pinned { pc } else { Color32::TRANSPARENT }, Stroke::new(1.5, pc));
            p.line_segment([pb.center() + Vec2::new(0.0, 1.0), pb.center() + Vec2::new(0.0, 8.0)], Stroke::new(1.5, pc));
            if pinned {
                p.rect_stroke(band, egui::CornerRadius { nw: 5, ne: 5, sw: 0, se: 0 }, Stroke::new(1.5, Color32::WHITE), egui::StrokeKind::Inside);
            }
            if look::tip(pr, if pinned { "Pinned: the preview shows this node whatever is chosen; a click lets it go" } else { "Pin: the preview keeps showing this node while you tune others" }, "P").clicked() {
                self.pinned = if pinned { None } else { Some(i) };
            }
            // a press on a row chooses its node too
            let mut pressed = false;
            // the rows, as on Blender's nodes: its choices as drop-downs, its numbers as sliders, then its input
            // pins, each with its slider while nothing is plugged in
            for (k, (name, words)) in s.words.iter().enumerate() {
                let rr = row_rect(*r, k);
                mark(format!("row:{}:{name}", n.id), rr);
                if !legible {
                    continue;
                }
                let (resp, set) = ui.scope_builder(egui::UiBuilder::new().max_rect(rr), |ui| word_box(ui, ("map node word", i, k), &n.words[k], &choices(words, pal), rr.width() - 8.0)).inner;
                pressed |= look::tip(resp, &format!("{name}: what this node does with it"), "").is_pointer_button_down_on();
                if let Some(w) = set {
                    self.edit(&format!("{name} {w}"), false, |m| {
                        let _ = m.set_word(i, name, &w, pal);
                    });
                }
            }
            for (k, sl) in s.nums.iter().enumerate() {
                let rr = row_rect(*r, s.words.len() + k);
                mark(format!("row:{}:{}", n.id, sl.name), rr);
                let (resp, new) = self.num_field(ui, rr, ui.scope_id().with(("num", i, k)), sl.name, n.nums[k], (sl.min, sl.max, sl.whole), legible);
                match resp {
                    Some(resp) => pressed |= look::tip(resp, &format!("{}: {} to {}; drag sideways to change it, a double click types it", sl.name, sl.min, sl.max), "").is_pointer_button_down_on(),
                    None => mark("typing".into(), rr),
                }
                if let Some((v, typed)) = new {
                    self.edit(&format!("{} {}", if typed { "Type" } else { "Move" }, sl.name), !typed, |m| {
                        m.set_num(i, sl.name, v);
                    });
                }
            }
            for (k, pin) in n.pins.iter().enumerate() {
                let (name, pk, _) = s.pins[k];
                let rr = row_rect(*r, s.words.len() + s.nums.len() + k);
                mark(format!("row:{}:{name}", n.id), rr);
                match pin {
                    Pin::Value(v) if pk != Data::Dir => {
                        let (resp, new) = self.num_field(ui, rr, ui.scope_id().with(("pinv", i, k)), name, *v, (0.0, 1.0, false), legible);
                        match resp {
                            Some(resp) => pressed |= look::tip(resp, &format!("{name} while nothing is plugged in (a double click types it); plug a wire in and the wire gives it"), "").is_pointer_button_down_on(),
                            None => mark("typing".into(), rr),
                        }
                        if let Some((v, typed)) = new {
                            self.edit(&format!("{} {name}", if typed { "Type" } else { "Move" }), !typed, |m| {
                                m.set_num(i, name, v);
                            });
                        }
                    }
                    _ if legible => {
                        p.text(rr.left_center() + Vec2::new(6.0, 0.0), Align2::LEFT_CENTER, name, font.clone(), Color32::from_gray(200));
                    }
                    _ => {}
                }
            }
            if pressed && !on {
                self.chosen = vec![i];
            }
            // the pins over everything of the node, so they are what a press near them takes
            let pin_kind = |k: Option<usize>| match k {
                None => kinds[i],
                Some(k) if s.pins[k].1 == Data::Any => kinds[i],
                Some(k) => s.pins[k].1,
            };
            let outs: Vec<(Option<usize>, Pos2)> = (s.kind != "out").then_some((None, *out)).into_iter().chain(ins.iter().enumerate().map(|(k, q)| (Some(k), *q))).collect();
            for (k, q) in outs {
                let pr = ui.interact(Rect::from_center_size(q, Vec2::splat(REACH * 2.0)), ui.scope_id().with(("pin", i, k)), Sense::drag());
                mark(k.map_or(format!("out:{}", n.id), |k| format!("pin:{}:{}", n.id, s.pins[k].0)), Rect::from_center_size(q, Vec2::splat(REACH * 2.0)));
                p.circle(q, PIN_R, kind_colour(pin_kind(k)), Stroke::new(if pr.hovered() { 2.0 } else { 1.0 }, Color32::from_gray(20)));
                if pr.drag_started() {
                    self.drag = Drag::Wire(Hit { node: i, pin: k });
                }
                let what = match k {
                    None => format!("Gives {}: drag to an input to wire it, let go on empty graph to add a node that takes it", pin_kind(None).name()),
                    Some(_) => format!("Takes {}: drag to an output to wire it; drag a plugged one off to unplug it", pin_kind(k).name()),
                };
                look::tip(pr, &what, "");
            }
            // choose and move
            if body.drag_started() {
                if !on {
                    self.chosen = vec![i];
                }
                let from = self.chosen.iter().map(|&c| (c, m.nodes[c].at)).collect();
                self.drag = Drag::Move(pointer.unwrap_or_default(), from);
            }
            if body.clicked() {
                self.chosen = vec![i];
            }
            look::tip(body, &format!("{} ({}): a click chooses it, a drag moves it", n.id, s.group), "");
        }
        // what a drag is doing
        let released = ctx.input(|i| i.pointer.primary_released());
        match (&self.drag, pointer) {
            (Drag::Move(start, from), Some(now)) => {
                let (d, from) = (now - *start, from.clone());
                let name = format!("Drag {}", m.nodes[from[0].0].id);
                self.edit(&name, true, |m| {
                    for (i, at) in &from {
                        m.nodes[*i].at = [at[0] + d.x, at[1] + d.y];
                    }
                });
            }
            (Drag::Wire(from), Some(now)) => {
                let from = *from;
                let start = if from.pin.is_none() { geoms[from.node].1 } else { geoms[from.node].2[from.pin.unwrap()] };
                let to = pin_at(&m, now).filter(|h| h.node != from.node);
                let tried = to.map(|t| connect(&mut m.clone(), from, t));
                let c = match &tried {
                    Some(Err(e)) => {
                        look::say(&ctx, &format!("This wire cannot be made: {e}"));
                        look::BAD
                    }
                    _ => kind_colour(kinds[from.node]),
                };
                let (a, b) = if from.pin.is_none() { (start, now) } else { (now, start) };
                wire(a, b, c);
                if released {
                    match (to, tried) {
                        (Some(t), Some(Ok(name))) => self.edit(&name, false, |m| {
                            let _ = connect(m, from, t);
                        }),
                        (Some(_), _) => {}
                        (None, _) if from.pin.is_some() && matches!(m.nodes[from.node].pins[from.pin.unwrap()], Pin::Wire(_)) => {
                            let name = format!("Unplug {} of {}", m.nodes[from.node].spec().pins[from.pin.unwrap()].0, m.nodes[from.node].id);
                            self.edit(&name, false, |m| m.unplug(from.node, from.pin.unwrap()));
                        }
                        (None, _) => self.adding = Some(Adding { at: now, screen: tr * now, query: String::new(), from: Some(from), focus: true }),
                    }
                }
            }
            (Drag::Box(start), Some(now)) => {
                let b = Rect::from_two_pos(*start, now);
                p.rect(b, 2.0, look::ACCENT.gamma_multiply(0.12), Stroke::new(1.0, look::ACCENT), egui::StrokeKind::Inside);
                if released {
                    self.chosen = geoms.iter().enumerate().filter(|(_, g)| g.0.intersects(b)).map(|(i, _)| i).collect();
                }
            }
            _ => {}
        }
        if released {
            self.drag = Drag::None;
        }
        tr
    }

    /// The Add a node box: a search over the kinds by group; opened from a wire, only what fits it.
    fn add_box(&mut self, ctx: &egui::Context, app: &mut App) {
        let Some(m) = self.doc().map(|d| d.m.clone()) else { return };
        let area = self.area.0;
        let Some(a) = &mut self.adding else { return };
        let (from, at) = (a.from, a.at);
        let (mut pick, mut close) = (None, ctx.input(|i| i.key_pressed(Key::Escape)));
        let r = egui::Area::new(Id::unique("maps add")).order(egui::Order::Foreground).fixed_pos(a.screen).constrain_to(area).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_width(230.0);
                look::weak(ui, if from.is_some() { "Add a node that fits this wire" } else { "Add a node" });
                let r = ui.add(egui::TextEdit::singleline(&mut a.query).hint_text("type to find").desired_width(f32::INFINITY));
                crate::drive::mark(ui.ctx(), "add:search", r.rect);
                if std::mem::take(&mut a.focus) {
                    r.request_focus();
                }
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                let q = a.query.to_lowercase();
                let fits = |kind: &str| {
                    let mut c = m.clone();
                    let n = c.add(kind, [0.0, 0.0]);
                    from.is_none_or(|f| plug(&mut c, f, n).is_ok())
                };
                let mut first = None;
                egui::ScrollArea::vertical().max_height((area.height() - 110.0).clamp(120.0, 380.0)).show(ui, |ui| {
                    for (g, gc) in GROUPS {
                        let kinds: Vec<&str> = SPECS.iter().filter(|s| s.group == g && s.kind != "out" && s.kind.contains(&q) && fits(s.kind)).map(|s| s.kind).collect();
                        if kinds.is_empty() {
                            continue;
                        }
                        ui.label(RichText::new(g).small().color(gc.gamma_multiply(1.4)));
                        for k in kinds {
                            first = first.or(Some(k));
                            let r = ui.add(egui::Button::new(k).min_size(Vec2::new(ui.available_width(), 0.0)));
                            if ui.is_rect_visible(r.rect) {
                                crate::drive::mark(ui.ctx(), format!("add:{k}"), r.rect);
                            }
                            if look::tip(r, &format!("Add a {k} node ({g}) here"), "").clicked() {
                                pick = Some(k);
                            }
                        }
                    }
                    if first.is_none() {
                        look::weak(ui, "nothing fits");
                    }
                });
                if enter {
                    pick = pick.or(first);
                }
            });
        });
        if ctx.input(|i| i.pointer.any_pressed()) && !ctx.input(|i| i.pointer.interact_pos()).is_some_and(|p| r.response.rect.contains(p)) {
            close = true;
        }
        if let Some(k) = pick {
            self.add_node(app, k, at, from);
            close = true;
        }
        if close {
            self.adding = None;
        }
    }

    /// Play or pause; paused, the loop stands on the frame it was on.
    fn play(&mut self) {
        // (before the first frame there is no graph yet: it plays from the start)
        let (frames, seconds) = self.graph.as_ref().map_or((1, 1.0), |g| (g.map.frames, g.map.seconds));
        if self.playing {
            self.frame = loop_frame(self.clock, frames, seconds);
        } else {
            self.clock = (self.frame as f64 + 1e-3) * seconds as f64 / frames as f64;
        }
        self.playing = !self.playing;
    }

    /// The preview's mode; Particles and Sway start playing, since they are to be seen moving.
    fn set_mode(&mut self, mode: &'static str) {
        self.mode = mode;
        if matches!(mode, "particles" | "sway") && !self.playing {
            self.play();
        }
    }

    fn open_add(&mut self, at_screen: Option<Pos2>) {
        let (rect, tr) = self.area;
        let screen = at_screen.filter(|p| rect.contains(*p)).unwrap_or(rect.center());
        self.adding = Some(Adding { at: tr.inverse() * screen, screen, query: String::new(), from: None, focus: true });
    }

    /// The chosen node's numbers and words.
    fn node_numbers(&mut self, ui: &mut Ui, app: &mut App) {
        let [i] = self.chosen[..] else {
            look::weak(ui, if self.chosen.is_empty() { "Choose a node in the graph to set its numbers" } else { "Several nodes chosen: Delete removes them, Ctrl+D copies them" });
            return;
        };
        let n = self.doc().unwrap().m.nodes[i].clone();
        let s = n.spec();
        look::weak(ui, format!("{} ({}, {})", n.id, s.kind, s.group));
        for (k, sl) in s.nums.iter().enumerate() {
            let mut v = n.nums[k];
            look::row(ui, sl.name, |ui| {
                // room for the reset at the row's end
                ui.spacing_mut().interact_size.x = 56.0;
                ui.spacing_mut().slider_width = (ui.available_width() - 56.0 - 24.0 - 12.0).max(24.0);
                let slider = egui::Slider::new(&mut v, sl.min..=sl.max).max_decimals(2);
                let r = ui.add(if sl.whole { slider.integer() } else { slider });
                crate::drive::mark(ui.ctx(), format!("num:{}", sl.name), r.rect);
                if look::tip(r, &format!("{}: {} to {}", sl.name, sl.min, sl.max), "").changed() {
                    self.edit(&format!("Move {}", sl.name), true, |m| {
                        m.set_num(i, sl.name, v);
                    });
                }
                let r = ui.add_enabled(n.nums[k] != sl.def, egui::Button::new("").min_size(Vec2::splat(22.0)));
                // a turning-back arrow, drawn: the game's font has no such glyph
                let (c, st) = (r.rect.center(), ui.visuals().widgets.style(&r).fg_stroke);
                let at = |a: f32, rad: f32| c + Vec2::angled(a) * rad;
                let arc: Vec<Pos2> = (0..=12).map(|j| at(-2.4 + j as f32 * 0.38, 5.5)).collect();
                let tip = arc[0];
                ui.painter().add(egui::Shape::line(arc, st));
                ui.painter().line_segment([tip, tip + Vec2::new(3.5, 0.0)], st);
                ui.painter().line_segment([tip, tip + Vec2::new(0.0, 3.5)], st);
                crate::drive::mark(ui.ctx(), format!("reset:{}", sl.name), r.rect);
                if look::tip(r, &format!("{} back to {} as new", sl.name, sl.def), "").clicked() {
                    self.edit(&format!("Reset {}", sl.name), false, |m| {
                        m.set_num(i, sl.name, sl.def);
                    });
                }
            });
        }
        for (k, (name, words)) in s.words.iter().enumerate() {
            let cur = n.words[k].clone();
            let mut set = None;
            look::row(ui, name, |ui| {
                if words.is_empty() {
                    let (r, w) = word_box(ui, ("map word", *name), &cur, &choices(words, &app.pal), 160.0);
                    set = w;
                    crate::drive::mark(ui.ctx(), format!("word:{name}"), r.rect);
                    look::tip(r, "The palette ramp the number runs along, dark to light", "");
                } else {
                    for w in *words {
                        let r = look::chip(ui, cur == *w, w);
                        crate::drive::mark(ui.ctx(), format!("word:{name}:{w}"), r.rect);
                        if look::tip(r, &format!("{name}: {w}"), "").clicked() {
                            set = Some(w.to_string());
                        }
                    }
                }
            });
            if let Some(w) = set.filter(|w| *w != cur) {
                let pal = app.pal.clone();
                self.edit(&format!("{} {w}", name), false, |m| {
                    let _ = m.set_word(i, name, &w, &pal);
                });
            }
        }
        if s.nums.is_empty() && s.words.is_empty() {
            look::weak(ui, "nothing to set: its pins are on the node");
        }
    }

    fn map_numbers(&mut self, ui: &mut Ui) {
        let m = self.doc().unwrap().m.clone();
        look::row(ui, "size", |ui| {
            for s in map::SIZES {
                let r = look::chip(ui, m.size == s, &s.to_string());
                crate::drive::mark(ui.ctx(), format!("size:{s}"), r.rect);
                if look::tip(r, "The picture's side in pixels, as written and exported", "").clicked() && m.size != s {
                    self.edit(&format!("Size {s}"), false, |m| m.size = s);
                }
            }
        });
        let (mut f, mut sec) = (m.frames, m.seconds);
        look::row(ui, "frames", |ui| {
            let r = ui.add(egui::Slider::new(&mut f, 1..=64));
            crate::drive::mark(ui.ctx(), "map:frames", r.rect);
            if look::tip(r, "Frames in the loop; 1 is a still picture", "").changed() {
                self.edit("Move frames", true, |m| m.frames = f);
            }
        });
        look::row(ui, "seconds", |ui| {
            let r = ui.add(egui::Slider::new(&mut sec, 0.1..=60.0).logarithmic(true).max_decimals(1).suffix(" s"));
            if look::tip(r, "How long the loop takes", "").changed() {
                self.edit("Move seconds", true, |m| m.seconds = sec);
            }
        });
    }
}

impl Page for Maps {
    fn name(&self) -> &'static str {
        "Maps"
    }

    fn files(&self, app: &App) -> Vec<FileRow> {
        let mut thumbs = self.thumbs.borrow_mut();
        self.docs
            .iter()
            .map(|(n, d)| {
                let text = d.m.write();
                let img = match thumbs.get(n).filter(|t| t.0 == text) {
                    Some(t) => t.1.clone(),
                    None => {
                        let img = make::thumb(&d.m, 16);
                        thumbs.insert(n.clone(), (text.clone(), img.clone()));
                        img
                    }
                };
                let path = format!("{DIR}/{n}.toml");
                FileRow { id: n.clone(), label: n.clone(), thumb: Thumb::Img(img), flags: Flags { made: d.saved.is_some(), draft: app.is_draft(&path), dirty: d.saved.as_deref() != Some(text.as_str()), problem: app.has_problem(&path) } }
            })
            .collect()
    }

    fn current(&self) -> Option<String> {
        self.open.clone()
    }

    fn open(&mut self, _app: &mut App, id: &str) {
        if self.docs.contains_key(id) {
            self.open_map(id);
        }
    }

    fn commands(&self, _app: &App) -> Vec<Cmd> {
        let mut v = vec![
            Cmd::new("add", "Add a node", "a search box of the kinds of node, by group; the node goes where the pointer is").key(Modifiers::NONE, Key::Space),
            Cmd::new("delete", "Delete the chosen nodes", "what they fed is unplugged; the out stays").key(Modifiers::NONE, Key::Delete),
            Cmd::new("copy", "Copy the chosen nodes", "copies a little aside, wired as the originals").key(Modifiers::COMMAND, Key::D),
            Cmd::new("frame", "Frame everything", "the whole graph in view").key(Modifiers::NONE, Key::F),
            Cmd::new("pin", "Pin the chosen node", "the preview keeps showing it while you tune others; again lets it go").key(Modifiers::NONE, Key::P),
            Cmd::new("view:map", "Preview the map", "the chosen or pinned node's picture"),
            Cmd::new("view:tiled", "Preview tiled", "the picture 3 x 3, so a seam would show"),
            Cmd::new("view:particles", "Preview particles", "dots drift in the node's push (a number curled, a colour by its brightness): does it swirl, stream or pile up"),
            Cmd::new("view:sway", "Preview sway", "a row of grass blades bent by the node as it scrolls past"),
            Cmd::new("play", "Play or pause", "plays the loop at frames / seconds; paused, it stands on a frame").key(Modifiers::NONE, Key::K),
            Cmd::new("save", "Save the map", "write data/maps/<name>.toml (read back as the check reads it) and take it off the drafts list").key(Modifiers::COMMAND, Key::S),
            Cmd::new("undo", "Undo", "undo the last change to this map").key(Modifiers::COMMAND, Key::Z),
            Cmd::new("redo", "Redo", "redo what was undone").key(Modifiers::COMMAND, Key::Y),
            Cmd::new("new", "New map", "the starters, each with its picture and what it is for, and a name").key(Modifiers::COMMAND, Key::N),
            Cmd::new("variations", "Variations", "the graph under other seeds; a click shows one, Use keeps it").key(Modifiers::NONE, Key::M),
            Cmd::new("use", "Use as the block's texture", "write the map as the chosen block's face (Use section): one undo step on the Textures page, saved and checked"),
            Cmd::new("export", "Export the map", "a PNG at the map's size where the Export section says; an animated map as a strip of frames"),
            Cmd::new("wall.use", "Use the variation", "the picked seeds into the map, one undo step"),
            Cmd::new("wall.cancel", "Close the variations", "back to the graph as it was"),
        ];
        for (st, what, _) in make::STARTERS {
            v.push(Cmd::new(format!("starter:{st}"), format!("New map: {st}"), what));
        }
        for n in self.docs.keys() {
            v.push(Cmd::new(format!("new:{n}"), format!("New map from {n}"), "a copy of it under a new name, not saved yet"));
        }
        v
    }

    fn run(&mut self, app: &mut App, id: &str) {
        if let Some(from) = id.strip_prefix("new:") {
            return self.new_from(from);
        }
        if let Some(st) = id.strip_prefix("starter:") {
            return self.make_from(app, st);
        }
        // the Use panel's choices, for scripts: use.block:<block>, use.face:<0..3>, use.variants:<1..4>, use.ramp:<ramp>
        if let Some(b) = id.strip_prefix("use.block:") {
            return self.set_block(app, b);
        }
        if let Some(f) = id.strip_prefix("use.face:").and_then(|f| f.parse::<usize>().ok()) {
            self.use_face = f.min(3);
            return;
        }
        if let Some(n) = id.strip_prefix("use.variants:").and_then(|f| f.parse::<usize>().ok()) {
            self.use_variants = n.clamp(1, 4);
            return;
        }
        if let Some(r) = id.strip_prefix("use.ramp:") {
            self.use_ramp = r.to_string();
            return;
        }
        if id == "new" {
            self.new_list = Some(String::new());
            return;
        }
        if self.open.is_none() {
            return;
        }
        match id {
            "add" => self.open_add(None),
            "delete" => {
                let chosen = std::mem::take(&mut self.chosen);
                if !chosen.is_empty() {
                    let names: Vec<String> = chosen.iter().map(|&i| self.doc().unwrap().m.nodes[i].id.clone()).collect();
                    self.edit(&format!("Delete {}", names.join(", ")), false, |m| m.remove(&chosen));
                    self.pinned = None;
                }
            }
            "copy" => {
                let chosen = self.chosen.clone();
                let mut made = vec![];
                self.edit("Copy nodes", false, |m| made = m.copy(&chosen));
                self.chosen = made;
            }
            "frame" => self.frame_all = true,
            "pin" => self.pinned = if self.pinned.is_some() { None } else { self.chosen.first().copied() },
            "view:map" => self.mode = "map",
            "view:tiled" => self.mode = "tiled",
            "view:particles" => self.set_mode("particles"),
            "view:sway" => self.set_mode("sway"),
            "play" => self.play(),
            "save" => self.save(app),
            "variations" => self.open_wall(),
            "use" => {
                self.sync();
                self.use_texture(app);
            }
            "export" => self.export(app),
            "wall.use" => self.wall_close(true),
            "wall.cancel" => self.wall_close(false),
            "undo" | "redo" => {
                if let Some(d) = self.open.as_ref().and_then(|n| self.docs.get_mut(n)) {
                    let mut now = d.m.clone();
                    if if id == "undo" { d.hist.undo(&mut now) } else { d.hist.redo(&mut now) } {
                        d.m = now;
                    }
                }
            }
            _ => {}
        }
    }

    fn more_keys(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("Middle drag", "pans the graph"),
            ("Wheel", "zooms the graph to the pointer"),
            ("Right click", "Add a node, where the pointer is"),
            ("Drag a pin", "wires it; let go on empty graph to add a node that fits"),
            ("Drag on empty graph", "chooses the nodes in the box"),
        ]
    }

    fn work(&mut self, ui: &mut Ui, app: &mut App) {
        if !ui.input(|i| i.pointer.any_down()) {
            self.merging = None;
        }
        if self.new_list.is_some() || self.open.is_none() {
            return self.new_list_ui(ui, app);
        }
        let Some(name) = self.open.clone() else { return };
        self.sync();
        ui.label(RichText::new(&name).strong());
        if self.wall.is_some() {
            return self.wall_ui(ui);
        }
        // the work area scrolls: the graph takes what is in view of it, not its whole width
        let outer = Rect::from_min_max(ui.cursor().min, ui.clip_rect().max);
        crate::drive::mark(ui.ctx(), "graph", outer);
        ui.painter().rect_filled(outer, 4.0, ui.visuals().extreme_bg_color);
        // framed on open and on F, and again while the area settles or is resized, until the view is moved
        let refit = self.fitted.is_some_and(|(v, s)| v == self.view && s != outer.size());
        let auto = std::mem::take(&mut self.frame_all) || self.view == Rect::ZERO || refit;
        if auto {
            let m = &self.doc().unwrap().m;
            self.view = framed(m.nodes.iter().map(|n| geom(n).0).reduce(|a, b| a.union(b)).unwrap_or(Rect::from_min_size(Pos2::ZERO, Vec2::splat(200.0))), outer.size());
        }
        let mut view = self.view;
        let scene = egui::Scene::new().zoom_range(0.2..=2.0).max_inner_size(Vec2::splat(1e5)).drag_pan_buttons(egui::DragPanButtons::MIDDLE).scroll_zooms(true);
        let r = ui.scope_builder(egui::UiBuilder::new().max_rect(outer), |ui| scene.show(ui, &mut view, |ui| self.graph_ui(ui, &app.pal))).inner;
        self.fitted = if auto { Some((view, outer.size())) } else { self.fitted.filter(|f| f.0 == view) };
        self.view = view;
        self.area = (outer, r.inner);
        // the empty graph: a drag draws a box, a click chooses nothing, a right click adds a node
        let bg = r.response;
        let at = ui.ctx().input(|i| i.pointer.interact_pos());
        if bg.drag_started_by(egui::PointerButton::Primary) {
            if let Some(p) = at {
                self.drag = Drag::Box(r.inner.inverse() * p);
            }
        }
        if bg.clicked() {
            self.chosen.clear();
        }
        if bg.secondary_clicked() {
            self.open_add(at);
        }
        look::tip(bg, "The graph: a drag draws a box to choose nodes, right click adds a node, middle drag pans, the wheel zooms", "Space, F");
        self.add_box(&ui.ctx().clone(), app);
    }

    fn inspector_top(&mut self, ui: &mut Ui, _app: &mut App) {
        if self.open.is_none() {
            return;
        }
        self.sync();
        let Some(node) = self.shown() else { return };
        let (id, m, stamp) = {
            let g = self.graph.as_ref().unwrap();
            (g.map.nodes[node].id.clone(), g.map.clone(), g.stamp[node])
        };
        let frames = m.frames;
        // the clock runs while playing
        let now = ui.input(|i| i.time);
        let dt = self.last_time.map_or(0.0, |t| (now - t).clamp(0.0, 0.1));
        self.last_time = Some(now);
        if self.playing {
            self.clock += dt;
            ui.ctx().request_repaint();
        }
        let shown = if self.playing { loop_frame(self.clock, frames, m.seconds) } else { self.frame };
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for (mode, label, help) in [
                ("map", "Map", "The picture as it is"),
                ("tiled", "Tiled", "The picture 3 x 3, so a seam would show"),
                ("particles", "Particles", "Dots drift in the node's push (a number curled, a colour by its brightness): does it swirl, stream or pile up"),
                ("sway", "Sway", "A row of grass blades bent by the node as it scrolls past"),
            ] {
                let r = look::chip(ui, self.mode == mode, label);
                crate::drive::mark(ui.ctx(), format!("chip:{label}"), r.rect);
                if look::tip(r, help, "").clicked() {
                    self.set_mode(mode);
                }
            }
            look::weak(ui, if self.pinned == Some(node) { format!("{id}, pinned") } else { id });
        });
        let moving = matches!(self.mode, "particles" | "sway");
        if frames > 1 || moving {
            ui.horizontal(|ui| {
                let r = ui.add(egui::Button::new(if self.playing { "Pause" } else { "Play" }).min_size(Vec2::new(64.0, 0.0)));
                crate::drive::mark(ui.ctx(), "play", r.rect);
                if look::tip(r, "Plays the loop at frames / seconds; paused, it stands on a frame", "K").clicked() {
                    self.play();
                }
                if frames > 1 {
                    let mut f = shown;
                    ui.spacing_mut().slider_width = (ui.available_width() - look::VALUE - 8.0).max(40.0);
                    ui.spacing_mut().interact_size.x = look::VALUE;
                    let r = ui.add(egui::Slider::new(&mut f, 0..=frames - 1));
                    crate::drive::mark(ui.ctx(), "frame", r.rect);
                    if look::tip(r, "Which frame of the loop the pictures show; moving it pauses", "").changed() {
                        (self.frame, self.playing) = (f, false);
                    }
                }
            });
        }
        if self.mode == "particles" {
            look::row(ui, "speed", |ui| look::tip(ui.add(egui::Slider::new(&mut self.speed, 0.02..=1.0).max_decimals(2)), "How fast the dots go: picture sides a second at the strongest push", ""));
        }
        if self.mode == "sway" {
            look::row(ui, "strength", |ui| look::tip(ui.add(egui::Slider::new(&mut self.strength, 0.0..=3.0).max_decimals(2)), "How far the map bends the blades", ""));
        }
        let side = ui.available_width().min(260.0);
        let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), side), Sense::hover());
        let r = Rect::from_center_size(r.center(), Vec2::splat(side));
        crate::drive::mark(ui.ctx(), "preview", r);
        // the loop, worked out once on a worker while it plays or moves something, kept till the graph changes
        let key = (stamp, m.size.min(256));
        if self.playing || moving {
            self.player.get(key, &m, node);
        }
        let strip = self.player.strip.as_ref().filter(|s| (self.playing || moving) && s.frames.len() == frames);
        let tex = match strip {
            Some(st) => {
                let tk = (st.key, shown);
                if self.strip_tex.as_ref().is_none_or(|t| t.0 != tk) {
                    let img = egui::ColorImage::from_rgba_unmultiplied([st.side, st.side], &map::rgba(&st.frames[shown], st.kind));
                    match &mut self.strip_tex {
                        Some((k, t)) => {
                            t.set(img, egui::TextureOptions::NEAREST_REPEAT);
                            *k = tk;
                        }
                        None => self.strip_tex = Some((tk, ui.ctx().load_texture("map loop", img, egui::TextureOptions::NEAREST_REPEAT))),
                    }
                }
                self.strip_tex.as_ref().unwrap().1.id()
            }
            None => self.preview_texture(ui.ctx(), node),
        };
        let full = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        let p = ui.painter_at(r);
        match (self.mode, self.player.strip.as_ref().filter(|_| moving)) {
            ("particles", Some(st)) => {
                p.image(tex, r, full, Color32::from_gray(80));
                let f = shown.min(st.push.len() - 1);
                if self.playing {
                    motion::drift(&mut self.dots, &st.push[f], st.side, self.speed, dt as f32);
                }
                let at = |q: [f32; 2]| r.min + Vec2::new(q[0], q[1]) * side;
                for d in &self.dots {
                    let pts: Vec<[f32; 2]> = d.trail.iter().copied().chain([d.at]).collect();
                    for (k, w) in pts.windows(2).enumerate() {
                        // a step across an edge is the wrap, not a line
                        if (w[0][0] - w[1][0]).abs() < 0.3 && (w[0][1] - w[1][1]).abs() < 0.3 {
                            let a = (k + 1) as f32 / pts.len() as f32;
                            p.line_segment([at(w[0]), at(w[1])], Stroke::new(1.2, Color32::from_white_alpha((a * 200.0) as u8)));
                        }
                    }
                    p.circle_filled(at(d.at), 1.6, Color32::WHITE);
                }
            }
            ("sway", Some(st)) => {
                let f = shown.min(st.frames.len() - 1);
                p.rect_filled(r, 0.0, Color32::from_rgb(28, 34, 44));
                let ground = r.max.y - side * 0.2;
                // the map's middle row under the ground, scrolling as the blades read it
                let off = (self.clock * 0.1) as f32;
                p.image(tex, Rect::from_min_max(Pos2::new(r.min.x, ground + 4.0), r.max), Rect::from_min_max(Pos2::new(off, 0.4), Pos2::new(off + 1.0, 0.6)), Color32::WHITE);
                p.line_segment([Pos2::new(r.min.x, ground), Pos2::new(r.max.x, ground)], Stroke::new(2.0, Color32::from_rgb(90, 70, 50)));
                let blade = |u: f32, h: f32, w: f32, c: Color32, leaves: bool| {
                    let b = motion::bend(&st.frames[f], st.side, st.kind, u + off, self.strength);
                    let pt = |t: f32| Pos2::new(r.min.x + u * side + b * h * 0.6 * t * t, ground - h * t * (1.0 - 0.15 * (b * t).powi(2)).max(0.4));
                    for k in 0..6 {
                        let (t0, t1) = (k as f32 / 6.0, (k + 1) as f32 / 6.0);
                        p.line_segment([pt(t0), pt(t1)], Stroke::new(w * (1.0 - t0 * 0.8), c));
                        if leaves && k % 2 == 1 {
                            let s = if k % 4 == 1 { 1.0 } else { -1.0 };
                            p.line_segment([pt(t1), pt(t1) + Vec2::new(s * h * 0.12 + b * h * 0.08, -h * 0.06)], Stroke::new(w * 0.8, c));
                        }
                    }
                };
                for k in 0..26 {
                    let g = (k * 37 % 11) as f32 / 11.0;
                    blade((k as f32 + 0.5) / 26.0, side * (0.26 + 0.12 * g), 3.0, Color32::from_rgb(70 + (g * 50.0) as u8, 160 + (g * 40.0) as u8, 70), false);
                }
                blade(0.62, side * 0.6, 4.0, Color32::from_rgb(60, 130, 70), true);
            }
            ("particles" | "sway", None) => {
                p.image(tex, r, full, Color32::from_gray(80));
                p.text(r.center(), Align2::CENTER_CENTER, "working out the loop…", FontId::proportional(look::BODY), Color32::WHITE);
            }
            _ => {
                let n = if self.mode == "tiled" { 3 } else { 1 };
                let cell = side / n as f32;
                for k in 0..n * n {
                    p.image(tex, Rect::from_min_size(r.min + Vec2::new((k % n) as f32, (k / n) as f32) * cell, Vec2::splat(cell)), full, Color32::WHITE);
                }
            }
        }
        look::tip(resp, "The chosen node's picture, or the pinned one's; a number grey, a colour as it is, a direction in red and green", "");
        ui.add_space(GAP * 0.5);
    }

    fn inspector(&mut self, ui: &mut Ui, app: &mut App) {
        if let Some(mut sec) = look::section(ui, "File") {
            let ui = &mut *sec;
            let names: Vec<String> = self.docs.keys().cloned().collect();
            let r = ui.button("New map…");
            crate::drive::mark(ui.ctx(), "btn:new", r.rect);
            if look::tip(r, "A new map from a starter", "Ctrl+N").clicked() {
                self.new_list = Some(String::new());
            }
            let r = ui.menu_button("New from…", |ui| {
                for n in names {
                    if ui.button(&n).clicked() {
                        self.new_from(&n);
                        ui.close();
                    }
                }
            });
            look::tip(r.response, "A copy of a map under a new name, to change without touching it", "");
        }
        if self.open.is_none() {
            return;
        }
        if let Some(mut sec) = look::section(ui, "Node") {
            self.node_numbers(&mut sec, app);
        }
        if let Some(mut sec) = look::section(ui, "Map") {
            self.map_numbers(&mut sec);
        }
        if let Some(mut sec) = look::section(ui, "Use") {
            self.use_panel(&mut sec, app);
        }
        if let Some(mut sec) = look::section(ui, "Export") {
            self.export_panel(&mut sec, app);
        }
    }

    fn help(&self) -> String {
        "Right click or Space adds a node; drag from a pin to wire it; Delete removes, Ctrl+D copies, F frames everything; middle drag pans, the wheel zooms".into()
    }

    fn history(&self, _app: &App) -> Option<(Vec<String>, Vec<String>)> {
        let d = self.doc()?;
        Some((d.hist.undo.iter().map(|s| s.name.clone()).collect(), d.hist.redo.iter().rev().map(|s| s.name.clone()).collect()))
    }

    fn back_to(&mut self, _app: &mut App, n: usize) {
        if let Some(d) = self.open.as_ref().and_then(|k| self.docs.get_mut(k)) {
            let mut now = d.m.clone();
            d.hist.back_to(&mut now, n);
            d.m = now;
        }
    }

    fn refresh(&mut self, app: &App) {
        for (n, m) in read_dir(&app.pal) {
            let saved = Some(m.write());
            match self.docs.get_mut(&n) {
                Some(d) if d.dirty() => d.saved = saved,
                Some(d) => {
                    if d.m != m {
                        d.hist.push(std::mem::replace(&mut d.m, m), "Changed on disk");
                    }
                    d.saved = saved;
                }
                None => {
                    self.docs.insert(n, MDoc { m, hist: History::default(), saved });
                }
            }
        }
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

    fn kept(&self) -> Option<Vec<(String, Vec<u8>)>> {
        Some(self.docs.iter().filter(|(_, d)| d.dirty()).map(|(n, d)| (format!("{n}.toml"), d.m.write().into_bytes())).collect())
    }

    fn unsaved_of(&self, files: Vec<(String, Vec<u8>)>) -> Vec<(String, Vec<u8>)> {
        files.into_iter().filter(|(f, b)| self.docs.get(f.trim_end_matches(".toml")).and_then(|d| d.saved.as_deref()) != Some(String::from_utf8_lossy(b).as_ref())).collect()
    }

    fn restore(&mut self, app: &mut App, files: Vec<(String, Vec<u8>)>) -> Vec<(String, Vec<u8>, String)> {
        let mut refused = vec![];
        for (f, b) in files {
            let name = f.trim_end_matches(".toml").to_string();
            match String::from_utf8(b.clone()).map_err(|e| e.to_string()).and_then(|t| map::parse(&format!("{DIR}/{f}"), &t, &app.pal)) {
                Err(e) => refused.push((f, b, e)),
                Ok(m) => match self.docs.get_mut(&name) {
                    Some(d) => d.hist.push(std::mem::replace(&mut d.m, m), "Recovered"),
                    None => {
                        self.docs.insert(name, MDoc { m, hist: History::default(), saved: None });
                    }
                },
            }
        }
        if self.open.is_none() {
            self.open = self.docs.keys().next().cloned();
        }
        refused
    }

    /// For `shot=`: the preview's worker done.
    fn settle(&mut self, ctx: &egui::Context, _app: &mut App) {
        let t0 = std::time::Instant::now();
        self.player.wait();
        while self.job.is_some() && t0.elapsed() < std::time::Duration::from_secs(30) {
            std::thread::sleep(std::time::Duration::from_millis(5));
            self.poll(ctx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clouds() -> Map {
        let pal = engine::palette::Palette::parse(&std::fs::read_to_string("../../data/palette.toml").unwrap()).unwrap();
        map::parse("clouds", &std::fs::read_to_string("../../data/maps/clouds.toml").unwrap(), &pal).unwrap()
    }

    #[test]
    fn a_pin_is_hit_near_it_and_the_out_has_no_output() {
        let m = clouds();
        let (noise, levels, out) = (m.find("noise").unwrap(), m.find("levels").unwrap(), m.find("out").unwrap());
        let (r, o, _) = geom(&m.nodes[noise]);
        assert_eq!(pin_at(&m, o + Vec2::new(4.0, -5.0)), Some(Hit { node: noise, pin: None }));
        assert_eq!(pin_at(&m, geom(&m.nodes[levels]).2[0] + Vec2::new(-6.0, 6.0)), Some(Hit { node: levels, pin: Some(0) }));
        assert_eq!(pin_at(&m, r.center()), None, "the middle of a node is no pin");
        assert_eq!(pin_at(&m, geom(&m.nodes[out]).1), None, "the out gives nothing");
        assert!(REACH * 2.0 >= 12.0 && PIN_R * 2.0 >= 12.0, "pins 12 points across at least");
    }

    #[test]
    fn a_graph_opens_framed() {
        let area = Vec2::new(800.0, 600.0);
        // a wide strip fills 80 % of the width
        let v = framed(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 200.0)), area);
        assert!((area.x / v.width() * 1000.0 - 640.0).abs() < 0.5, "{v:?}");
        // a lone node is not blown up past 1.5
        let v = framed(Rect::from_min_size(Pos2::ZERO, Vec2::new(100.0, 100.0)), area);
        assert!((area.x / v.width() - 1.5).abs() < 1e-4 && v.center() == Pos2::new(50.0, 50.0), "{v:?}");
    }

    #[test]
    fn a_wire_through_the_page_refuses_a_circle_and_a_kind() {
        let mut m = clouds();
        let inv = m.add("invert", [0.0, 300.0]);
        let levels = m.find("levels").unwrap();
        let name = connect(&mut m, Hit { node: inv, pin: Some(0) }, Hit { node: levels, pin: None }).unwrap();
        assert_eq!(name, "Wire levels to invert", "drawn from the input back to the output");
        let before = m.clone();
        let e = connect(&mut m, Hit { node: inv, pin: None }, Hit { node: levels, pin: Some(0) }).unwrap_err();
        assert!(e.contains("circle"), "{e}");
        assert_eq!(m, before);
        let len = m.add("length", [0.0, 0.0]);
        assert!(connect(&mut m, Hit { node: inv, pin: None }, Hit { node: len, pin: Some(0) }).unwrap_err().contains("takes a direction"));
        assert!(connect(&mut m, Hit { node: inv, pin: None }, Hit { node: levels, pin: None }).is_err(), "two outputs");
        // a wire let go on empty graph: the new node plugs into its first pin that takes it
        let slope = m.add("slope", [0.0, 0.0]);
        plug(&mut m, Hit { node: inv, pin: None }, slope).unwrap();
        assert_eq!(m.nodes[slope].pins[0], Pin::Wire(inv));
        let blend = m.add("blend", [0.0, 0.0]);
        let noise = m.find("noise").unwrap();
        plug(&mut m, Hit { node: blend, pin: Some(2) }, noise).unwrap();
        assert!(plug(&mut m, Hit { node: len, pin: Some(0) }, inv).is_err(), "invert gives no direction");
    }
}
