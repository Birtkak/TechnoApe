//! The Maps page (docs/design/studio-maps.md, ADR draft-studio-maps): a map's node graph on `egui::Scene` to
//! wire and tune, the preview of the chosen or pinned node, its numbers. The map, its checks and its working
//! out are `map`'s; this draws and edits them.
use super::look::{self, GAP};
use super::{App, Cmd, FileRow, Page, Thumb, DRAFTS};
use crate::art::{self, History, Img};
use crate::map::{self, Data, Graph, Map, Pin, SPECS};
use crate::shell::Flags;
use eframe::egui::{self, emath::TSTransform, Align2, Color32, FontId, Id, Key, Modifiers, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::sync::mpsc::Receiver;

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

// a node's box: its band, its picture, a row a pin
const W: f32 = 184.0;
const HEAD: f32 = 24.0;
const PIC: f32 = 72.0;
const PIN_ROW: f32 = 26.0;
/// A pin is drawn 12 points across and hit 20 across.
const PIN_R: f32 = 6.0;
const REACH: f32 = 10.0;

/// A node's box in the graph, its output pin and its input pins.
pub fn geom(n: &map::Node) -> (Rect, Pos2, Vec<Pos2>) {
    let top = Pos2::new(n.at[0], n.at[1]);
    let pins = n.spec().pins.len();
    let r = Rect::from_min_size(top, Vec2::new(W, HEAD + PIC + GAP + pins as f32 * PIN_ROW + GAP * 0.5));
    let ins = (0..pins).map(|p| Pos2::new(r.min.x, top.y + HEAD + PIC + GAP + (p as f32 + 0.5) * PIN_ROW)).collect();
    (r, Pos2::new(r.max.x, top.y + HEAD + PIC * 0.5), ins)
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
    drag: Drag,
    adding: Option<Adding>,
    tiled: bool,
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
            drag: Drag::None,
            adding: None,
            tiled: false,
            frame: 0,
            merging: None,
            tex: HashMap::new(),
            full: None,
            job: None,
            thumbs: RefCell::default(),
            area: (Rect::ZERO, TSTransform::IDENTITY),
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
        let Some(m) = self.doc().map(|d| d.m.clone()) else { return };
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
        let name = (2..).map(|k| format!("{from} {k}")).find(|n| !self.docs.contains_key(n) && !std::path::Path::new(&format!("{DIR}/{n}.toml")).exists()).unwrap();
        self.docs.insert(name.clone(), MDoc { m, hist: History::default(), saved: None });
        self.open_map(&name);
    }

    fn open_map(&mut self, name: &str) {
        self.open = Some(name.into());
        (self.graph, self.chosen, self.pinned, self.frame_all, self.adding) = (None, vec![], None, true, None);
        self.tex.clear();
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
    fn graph_ui(&mut self, ui: &mut Ui) -> TSTransform {
        let ctx = ui.ctx().clone();
        let tr = ctx.layer_transform_to_global(ui.layer_id()).unwrap_or_default();
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
            let mut job = egui::text::LayoutJob::simple_singleline(n.id.clone(), font.clone(), Color32::WHITE);
            if n.id != s.kind {
                job.append(s.kind, 6.0, egui::TextFormat::simple(font.clone(), Color32::from_gray(200)));
            }
            job.wrap = egui::text::TextWrapping::truncate_at_width(W - 36.0);
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
            // the input rows: a pin, its name, and its slider while nothing is plugged in
            for (k, (pin, at)) in n.pins.iter().zip(ins).enumerate() {
                let (name, pk, _) = s.pins[k];
                let row = Rect::from_min_max(Pos2::new(r.min.x + 14.0, at.y - PIN_ROW / 2.0 + 2.0), Pos2::new(r.max.x - GAP, at.y + PIN_ROW / 2.0 - 2.0));
                p.text(Pos2::new(row.min.x, at.y), Align2::LEFT_CENTER, name, font.clone(), Color32::from_gray(200));
                if let (Pin::Value(v), false) = (pin, pk == Data::Dir) {
                    let mut v = *v;
                    let sr = Rect::from_min_max(Pos2::new(row.min.x + 44.0, row.min.y), row.max);
                    let resp = ui.scope_builder(egui::UiBuilder::new().max_rect(sr), |ui| {
                        ui.spacing_mut().slider_width = (sr.width() - 48.0).max(20.0);
                        ui.spacing_mut().interact_size.x = 40.0;
                        ui.add(egui::Slider::new(&mut v, 0.0..=1.0).max_decimals(2))
                    });
                    mark(format!("slider:{}:{name}", n.id), sr);
                    if look::tip(resp.inner, &format!("{name} while nothing is plugged in; plug a wire in and the wire gives it"), "").changed() {
                        self.edit(&format!("Move {name}"), true, |m| {
                            m.set_num(i, name, v);
                        });
                    }
                }
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
                let slider = egui::Slider::new(&mut v, sl.min..=sl.max).max_decimals(2);
                let r = ui.add(if sl.whole { slider.integer() } else { slider });
                crate::drive::mark(ui.ctx(), format!("num:{}", sl.name), r.rect);
                if look::tip(r, &format!("{}: {} to {}", sl.name, sl.min, sl.max), "").changed() {
                    self.edit(&format!("Move {}", sl.name), true, |m| {
                        m.set_num(i, sl.name, v);
                    });
                }
                // (it does not fit beside the slider: it goes under it)
                if n.nums[k] != sl.def && look::tip(look::chip(ui, false, "reset"), &format!("{} back to {} as new", sl.name, sl.def), "").clicked() {
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
                    let r = egui::ComboBox::from_id_salt(("map word", *name)).selected_text(if cur.is_empty() { "chosen colours" } else { &cur }).show_ui(ui, |ui| {
                        for (ramp, _) in app.pal.ramps() {
                            let r = ui.selectable_label(ramp == cur, &ramp);
                            if ui.is_rect_visible(r.rect) {
                                crate::drive::mark(ui.ctx(), format!("word:{name}:{ramp}"), r.rect);
                            }
                            if r.clicked() {
                                set = Some(ramp);
                            }
                        }
                    });
                    crate::drive::mark(ui.ctx(), format!("word:{name}"), r.response.rect);
                    look::tip(r.response, "The palette ramp the number runs along, dark to light", "");
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
                        let mut g = Graph::new(d.m.clone());
                        let o = g.out();
                        let kind = g.kinds[o];
                        let px = map::rgba(g.pic(o, 0, 16), kind);
                        let img = Img { w: 16, h: 16, px: px.chunks(4).map(|c| [c[0], c[1], c[2]]).collect() };
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
            Cmd::new("save", "Save the map", "write data/maps/<name>.toml (read back as the check reads it) and take it off the drafts list").key(Modifiers::COMMAND, Key::S),
            Cmd::new("undo", "Undo", "undo the last change to this map").key(Modifiers::COMMAND, Key::Z),
            Cmd::new("redo", "Redo", "redo what was undone").key(Modifiers::COMMAND, Key::Y),
        ];
        for n in self.docs.keys() {
            v.push(Cmd::new(format!("new:{n}"), format!("New map from {n}"), "a copy of it under a new name, not saved yet"));
        }
        v
    }

    fn run(&mut self, app: &mut App, id: &str) {
        if let Some(from) = id.strip_prefix("new:") {
            return self.new_from(from);
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
            "view:map" => self.tiled = false,
            "view:tiled" => self.tiled = true,
            "save" => self.save(app),
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
        let Some(name) = self.open.clone() else {
            look::weak(ui, "Choose a map on the left to open it");
            return;
        };
        self.sync();
        ui.horizontal(|ui| {
            ui.label(RichText::new(&name).strong());
            ui.add(egui::Label::new(RichText::new("right click or Space adds a node; drag a pin to wire it; F frames everything").small().weak()).truncate());
        });
        // the work area scrolls: the graph takes what is in view of it, not its whole width
        let outer = Rect::from_min_max(ui.cursor().min, ui.clip_rect().max);
        crate::drive::mark(ui.ctx(), "graph", outer);
        ui.painter().rect_filled(outer, 4.0, ui.visuals().extreme_bg_color);
        if self.frame_all || self.view == Rect::ZERO {
            let m = &self.doc().unwrap().m;
            self.view = m.nodes.iter().map(|n| geom(n).0).reduce(|a, b| a.union(b)).unwrap_or(Rect::from_min_size(Pos2::ZERO, Vec2::splat(200.0))).expand(60.0);
            self.frame_all = false;
        }
        let mut view = self.view;
        let scene = egui::Scene::new().zoom_range(0.2..=2.0).max_inner_size(Vec2::splat(1e5)).drag_pan_buttons(egui::DragPanButtons::MIDDLE).scroll_zooms(true);
        let r = ui.scope_builder(egui::UiBuilder::new().max_rect(outer), |ui| scene.show(ui, &mut view, |ui| self.graph_ui(ui))).inner;
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
        let (id, frames) = {
            let g = self.graph.as_ref().unwrap();
            (g.map.nodes[node].id.clone(), g.map.frames)
        };
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for (on, label, cmd, help) in [(!self.tiled, "Map", "view:map", "The picture as it is"), (self.tiled, "Tiled", "view:tiled", "The picture 3 x 3, so a seam would show")] {
                let r = look::chip(ui, on, label);
                crate::drive::mark(ui.ctx(), format!("chip:{label}"), r.rect);
                if look::tip(r, help, "").clicked() {
                    self.tiled = cmd == "view:tiled";
                }
            }
            look::weak(ui, if self.pinned == Some(node) { format!("{id}, pinned") } else { id });
        });
        if frames > 1 {
            let mut f = self.frame;
            look::row(ui, "frame", |ui| {
                let r = ui.add(egui::Slider::new(&mut f, 0..=frames - 1));
                crate::drive::mark(ui.ctx(), "frame", r.rect);
                look::tip(r, "Which frame of the loop the pictures show", "");
            });
            self.frame = f;
        }
        let side = ui.available_width().min(260.0);
        let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), side), Sense::hover());
        let r = Rect::from_center_size(r.center(), Vec2::splat(side));
        crate::drive::mark(ui.ctx(), "preview", r);
        let tex = self.preview_texture(ui.ctx(), node);
        let n = if self.tiled { 3 } else { 1 };
        let cell = side / n as f32;
        for k in 0..n * n {
            ui.painter().image(tex, Rect::from_min_size(r.min + Vec2::new((k % n) as f32, (k / n) as f32) * cell, Vec2::splat(cell)), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        }
        look::tip(resp, "The chosen node's picture, or the pinned one's; a number grey, a colour as it is, a direction in red and green", "");
        ui.add_space(GAP * 0.5);
    }

    fn inspector(&mut self, ui: &mut Ui, app: &mut App) {
        if let Some(mut sec) = look::section(ui, "File") {
            let ui = &mut *sec;
            let names: Vec<String> = self.docs.keys().cloned().collect();
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
