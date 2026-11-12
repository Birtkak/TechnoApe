//! The Textures page (docs/design/studio.md "The window's layout", studio-tools.md "Textures: the tools"): a
//! block's faces and variants on a 16 x 16 canvas, the tools, the palette, the starts (pattern, picture,
//! another block), the variations wall for variants and the previews. The editing is `art`'s and `tools`'.
use super::look::{self, c32, dim, draw, GAP};
use super::wall::{self as wall_ui, Act};
use super::{App, Cmd, FileRow, Page, Thumb, TEX};
use crate::art::{self, Doc, Img, S};
use crate::pattern;
use crate::shell::Flags;
use crate::tools::{self, Clip, Sel};
use crate::wall::{Rng, Wall};
use eframe::egui::{self, Color32, Key, Modifiers, PointerButton, Pos2, Rect, RichText, Sense, StrokeKind, Ui, Vec2};
use engine::blocks::See;
use engine::palette::{to_hex, Palette};

const FACE_NAMES: [&str; 4] = ["All", "Top", "Side", "Bottom"];
/// The seasons' tabs, in `engine::blocks::SEASONS`' order (docs/design/seasonal-looks.md); summer's is the base.
const SEASON_NAMES: [&str; 4] = ["Spring", "Summer", "Autumn", "Winter"];

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Tool {
    Pencil,
    Fill,
    Picker,
    Line,
    Shade,
    Scatter,
    Spray,
    Dither,
    Gradient,
    Select,
    Replace,
    Eraser,
}

/// Each tool: its id, name, key, and its help for the status line and tooltip.
pub(super) const TOOLS: [(Tool, &str, &str, Key, &str); 12] = [
    (Tool::Pencil, "pencil", "Pencil", Key::B, "Pencil: left paints the first colour, right the second; Alt picks"),
    (Tool::Fill, "fill", "Fill", Key::G, "Fill: fills the touching area of one colour; left the first colour, right the second"),
    (Tool::Picker, "picker", "Picker", Key::I, "Picker: left takes the first colour from the canvas, right the second"),
    (Tool::Line, "line", "Line", Key::L, "Line: drag a straight line; left the first colour, right the second"),
    (Tool::Shade, "shade", "Shade", Key::U, "Shade: left makes a pixel one step darker on its ramp, right one lighter; a drag shades each pixel once"),
    (Tool::Scatter, "scatter", "Scatter", Key::N, "Scatter: paints the colour and its ramp neighbours at random, by the amount; grain in palette colours"),
    (Tool::Spray, "spray", "Spray", Key::A, "Spray: paints by chance within a radius; left the first colour, right the second"),
    (Tool::Dither, "dither", "Dither", Key::D, "Dither fill: the touching area becomes a checker of the first and second colour"),
    (Tool::Gradient, "gradient", "Gradient", Key::E, "Gradient: drag a line; the face runs from the first to the second colour along their ramp, dithered"),
    (Tool::Select, "select", "Select", Key::S, "Select: drag a rectangle; drag inside it to move it; flip, turn, copy and paste in the inspector"),
    (Tool::Replace, "replace", "Replace", Key::R, "Replace colour: a click turns every pixel of that colour in the file into the first colour (right: the second)"),
    (Tool::Eraser, "eraser", "Eraser", Key::X, "Eraser: left or right makes pixels empty, holes the faces behind show through; any paint fills them again"),
];
/// The Eraser's tip while the block is opaque: why it is off.
const ERASER_OFF: &str = "Eraser: only for a cutout block (choose Cutout under Kind); an opaque block has no empty pixels";
/// The kinds of block (`see` in data/blocks.toml, ADR 0069): (id, name, what it is for).
const KINDS: [(See, &str, &str); 2] = [
    (See::Opaque, "Opaque", "Opaque: hides what is behind it, for ground, stone, wood."),
    (See::Cutout, "Cutout", "Cutout: pixels are there or empty, for leaves, plants, grates. The faces behind it are drawn."),
];

/// Block `block`'s row of data/blocks.toml (`text`) with `see = "cutout"` after its name, or without it for
/// opaque; None when the file has no such row.
pub fn kind_row(text: &str, block: &str, see: See) -> Option<String> {
    let span = crate::rows::spans(text).into_iter().find(|s| s.header == "block" && s.name == block)?;
    let mut k = crate::rows::Keys::parse(&text[span.at]);
    let v = match see {
        See::Opaque => None,
        See::Cutout => Some("\"cutout\"".to_string()),
        See::Glass => Some("\"glass\"".to_string()),
    };
    k.set_after("see", v, &["name"]);
    Some(k.text())
}

struct Stroke {
    button: PointerButton,
    tool: Tool,
    start: (i32, i32),
    last: (i32, i32),
    /// The picture when the stroke began: a line, gradient or move draws from it again each frame.
    base: Img,
    /// Pixels shaded or scattered this stroke: each only once.
    touched: Vec<(usize, usize)>,
    /// A selection being moved: where it was and what it holds.
    lifted: Option<(Sel, Clip)>,
}

pub struct Textures {
    block: String,
    /// The season painted (`engine::blocks::SEASONS`): 1, summer, is the base files.
    season: usize,
    /// Which seasons the block's row has a look for (`has_look`), by the registry and block it was read for.
    looks: std::cell::RefCell<Option<((u64, String), [bool; 4])>>,
    face: usize,
    variant: usize,
    tool: Tool,
    mirror: bool,
    tiled: bool,
    seams: bool,
    first: [u8; 3],
    second: [u8; 3],
    zoom: Option<f32>,
    pan: Vec2,
    stroke: Option<Stroke>,
    rng: Rng,
    amount: f32,
    radius: i32,
    density: f32,
    sel: Option<Sel>,
    clip: Option<Clip>,
    picture: String,
    from_block: String,
    swap: (String, String),
    /// The fixed preview on top of the inspector: 0 the block, 1 tiled 3 x 3, 2 tiled 5 x 5, 3 unfolded.
    preview: usize,
    /// "Generate variants" (usability round step 3): open, its wall (None while variant 1 is flat: nothing to
    /// vary), and how many variants in all.
    gen: bool,
    wall: Option<Wall<Img>>,
    count: usize,
    /// A variant tab's x asked first: the variant holds paint.
    ask_remove: Option<usize>,
    /// The lit cube: its camera, and a hash of the textures last written for the renderer.
    orbit: crate::voxel::Orbit,
    tex_gen: u64,
    /// When the renderer's textures were last written, and how long that took (ms).
    tex_at: std::time::Instant,
    tex_ms: f32,
    /// "Start from: Pattern" (usability round step 2): the panel's recipe, the well a swatch click sets.
    pat: Option<PatPanel>,
    /// Each file's last recipe in this session, for the panel to open with.
    recipes: std::collections::HashMap<String, (pattern::Recipe, bool)>,
    /// A change shown in the canvas and every preview before Use: (file, the picture it would hold).
    pending: Option<(String, Img)>,
    /// Kind changes of a block whose open face has no file of its own to hold the step: (block, the kind it
    /// was set to), undone and redone in order.
    kinds: (Vec<(String, See)>, Vec<(String, See)>),
}

struct PatPanel {
    recipe: pattern::Recipe,
    /// The colour well a click on a palette swatch sets next.
    well: Option<usize>,
    /// Fill every variant, each from its own seed (else the open one).
    every: bool,
}

/// The fixed preview's chips: (id, name, help).
const PREVIEWS: [(&str, &str, &str); 4] = [
    ("block", "Block", "The block in the game's renderer, on a floor of its own kind: drag turns, the wheel zooms, a double click puts the camera back"),
    ("3", "3 x 3", "The face tiled 3 x 3, variants mixed as the game mixes them"),
    ("5", "5 x 5", "The face tiled 5 x 5: repeats show sooner"),
    ("unfold", "Unfolded", "Top, the four sides and the bottom unfolded, as the block wears them"),
];
/// The fixed preview's picture height (points): the box with its chips is about 220.
const BOX: f32 = 190.0;
/// While a stroke is held the block follows it at most this often (ms): the writes cost the frame little
/// (measured: ADR 0064), the renderer's reload sets the pace.
const FOLLOW_MS: u128 = 120;
fn home() -> crate::voxel::Orbit {
    crate::voxel::Orbit { target: glam::Vec3::new(0.5, 0.3, 0.5), yaw: 0.7, pitch: 0.55, dist: 3.4 }
}

impl Textures {
    pub fn new(app: &App) -> Textures {
        let at = |i: usize| app.pal.colours.get(i).or(app.pal.colours.first()).map_or([128; 3], |c| c.rgb);
        let block = app.reg.placeable().next().map_or(String::new(), |d| d.name.clone());
        let mut t = Textures {
            block: String::new(),
            season: 1,
            looks: Default::default(),
            face: 0,
            variant: 0,
            tool: Tool::Pencil,
            mirror: false,
            tiled: false,
            seams: false,
            first: at(0),
            second: at(1),
            zoom: None,
            pan: Vec2::ZERO,
            stroke: None,
            rng: Rng::new(1),
            amount: 0.5,
            radius: 2,
            density: 0.35,
            sel: None,
            clip: None,
            picture: String::new(),
            from_block: String::new(),
            swap: (String::new(), String::new()),
            preview: 0,
            gen: false,
            wall: None,
            count: 3,
            ask_remove: None,
            orbit: home(),
            tex_gen: 0,
            tex_at: std::time::Instant::now(),
            tex_ms: 0.0,
            pat: None,
            recipes: Default::default(),
            pending: None,
            kinds: Default::default(),
        };
        t.go(app, &block);
        t
    }

    fn file(&self, face: usize) -> String {
        art::season_file(&self.block, face, self.season)
    }

    fn go(&mut self, app: &App, block: &str) {
        self.block = block.to_string();
        self.face = (0..4).find(|&f| app.docs.contains_key(&self.file(f))).unwrap_or(0);
        (self.variant, self.zoom, self.pan, self.stroke, self.sel, self.wall) = (0, None, Vec2::ZERO, None, None, None);
        (self.pat, self.pending, self.gen, self.ask_remove) = (None, None, false, None);
    }

    /// Another face or season: what was open or pending for the old file goes (the review of ADR 0064).
    fn leave(&mut self, face: usize, season: usize) {
        (self.face, self.season, self.variant, self.stroke, self.sel) = (face, season, 0, None, None);
        (self.pat, self.pending, self.gen, self.wall, self.ask_remove) = (None, None, false, None, None);
    }

    /// The picture a face shows in the season painted: its own file, else (dimmed, `false`) the season's
    /// every-face file, the base face's, the base every-face file.
    /// A pending change (a pattern, a set of variants) shows in its place until Use or Cancel.
    fn shown<'a>(&'a self, app: &'a App) -> Option<(&'a Img, bool)> {
        match &self.pending {
            Some((f, img)) if *f == self.file(self.face) => Some((img, true)),
            _ => self.real(app),
        }
    }

    /// `shown` without the pending change: what the files hold.
    fn real<'a>(&self, app: &'a App) -> Option<(&'a Img, bool)> {
        if self.season == 1 {
            return Self::face_img(app, &self.block, self.face);
        }
        let get = |f: usize, q: usize| app.docs.get(&art::season_file(&self.block, f, q)).map(|d| &d.img);
        match get(self.face, self.season) {
            Some(img) => Some((img, true)),
            None => get(0, self.season).or(get(self.face, 1)).or(get(0, 1)).map(|i| (i, false)),
        }
    }

    /// Whether the block has season `q` of its own: a look in data/blocks.toml or a painted file.
    fn has_season(&self, app: &App, q: usize) -> bool {
        q == 1 || self.has_look(app, q) || (0..4).any(|f| app.docs.contains_key(&art::season_file(&self.block, f, q)))
    }

    /// Whether the block's row in data/blocks.toml has a `[block.<season>]` for season `q` (the registry folds
    /// a missing one into the base look): read again only when the registry or the block changed.
    fn has_look(&self, app: &App, q: usize) -> bool {
        let mut seen = self.looks.borrow_mut();
        if seen.as_ref().is_none_or(|(k, _)| *k != (app.reg.hash, self.block.clone())) {
            let text = std::fs::read_to_string(super::BLOCKS).unwrap_or_default();
            let has = std::array::from_fn(|q| q != 1 && crate::rows::put_table(&text, "block", &self.block, engine::blocks::SEASONS[q], None) != text);
            *seen = Some(((app.reg.hash, self.block.clone()), has));
        }
        seen.as_ref().is_some_and(|(_, has)| has[q])
    }

    /// "Add season": the base's painted files copied into the season's (unsaved, to paint over) and, when the
    /// row has no look for it, a `[block.<season>]` with the base colour in data/blocks.toml.
    fn add_season(&mut self, app: &mut App) {
        let q = self.season;
        if q == 1 {
            return app.toast("Summer is the base look: pick Spring, Autumn or Winter");
        }
        let mut copied = 0;
        for f in 0..4 {
            let to = art::season_file(&self.block, f, q);
            if let (Some(base), false) = (app.docs.get(&art::season_file(&self.block, f, 1)), app.docs.contains_key(&to)) {
                let img = base.img.clone();
                app.docs.insert(to, Doc { img, hist: Default::default(), saved: None });
                copied += 1;
            }
        }
        let def = app.reg.placeable().find(|d| d.name == self.block).cloned();
        if let Some(d) = def.filter(|_| !self.has_look(app, q)) {
            let c = d.colour.map(crate::rows::num).join(", ");
            match Self::save_season_look(&self.block, q, Some(&format!("colour = [{c}]\n"))) {
                Ok(reg) => app.reg = reg,
                Err(e) => return app.refuse(e),
            }
        }
        let name = engine::blocks::SEASONS[q];
        app.toast(format!("{}: {name} added from the base ({copied} painted files to save, its look in {})", self.block, super::BLOCKS));
    }

    /// "Remove": the season's painted files gone (from the disk too) and its look dropped from data/blocks.toml.
    fn remove_season(&mut self, app: &mut App) {
        let q = self.season;
        if q == 1 {
            return;
        }
        for f in 0..4 {
            let n = art::season_file(&self.block, f, q);
            app.docs.remove(&n);
            let path = format!("{TEX}/{n}");
            if std::path::Path::new(&path).exists() {
                if let Err(e) = std::fs::remove_file(&path) {
                    app.refuse(format!("{path}: {e}"));
                }
            }
        }
        match Self::save_season_look(&self.block, q, None) {
            Ok(reg) => app.reg = reg,
            Err(e) => return app.refuse(e),
        }
        app.toast(format!("{}: {} removed, the base look again", self.block, engine::blocks::SEASONS[q]));
    }

    /// Writes (or drops) a block's `[block.<season>]` in data/blocks.toml, when the game's reader takes it; the
    /// registry it now reads.
    fn save_season_look(block: &str, q: usize, body: Option<&str>) -> Result<engine::blocks::Registry, String> {
        let path = super::BLOCKS;
        let old = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        let new = crate::rows::put_table(&old, "block", block, engine::blocks::SEASONS[q], body);
        let reg = engine::blocks::Registry::parse(&new).map_err(|e| format!("{path}: {e}"))?;
        let tmp = format!("{path}.tmp");
        std::fs::write(&tmp, &new).and_then(|_| std::fs::rename(&tmp, path)).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("{path}: {e}")
        })?;
        Ok(reg)
    }
    fn face_img<'a>(app: &'a App, block: &str, face: usize) -> Option<(&'a Img, bool)> {
        match app.docs.get(&art::file_name(block, face)) {
            Some(d) => Some((&d.img, true)),
            None => app.docs.get(&art::file_name(block, 0)).map(|d| (&d.img, false)),
        }
    }
    fn block_colour(&self, app: &App) -> [u8; 3] {
        app.reg.placeable().find(|d| d.name == self.block).map_or([0.5; 3], |d| d.tone(engine::blocks::ROW[self.season]).colour).map(|v| (v * 255.0).round() as u8)
    }

    /// The face's own file, made from what it shows (or the palette colour nearest the block's) when it has none.
    fn doc<'a>(&self, app: &'a mut App) -> &'a mut Doc {
        let key = self.file(self.face);
        if !app.docs.contains_key(&key) {
            let img = match self.real(app) {
                Some((img, _)) => img.clone(),
                None => Img::filled(1, tools::snap(&app.pal, self.block_colour(app).map(|v| v as f32 / 255.0))),
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

    /// One named edit of the open face: `f` on the picture and the variant, a history step if it changed it.
    fn edit(&mut self, app: &mut App, name: &str, f: impl FnOnce(&mut Img, usize, &Palette)) {
        let v = self.v(app);
        let pal = app.pal.clone();
        let d = self.doc(app);
        let before = d.img.clone();
        f(&mut d.img, v, &pal);
        if d.img != before {
            d.hist.push(before, name);
        }
    }

    /// A new picture for the open variant (pattern, picture).
    fn put_variant(&mut self, app: &mut App, name: &str, face: Img) {
        self.edit(app, name, |img, v, _| tools::paste(img, v, &tools::copy(&face, 0, Sel { x: 0, y: 0, w: S, h: S }), 0, 0));
        app.toast(format!("{name}: {}", self.file(self.face)));
    }

    /// The block's row in the season painted.
    fn def(&self, app: &App) -> Option<engine::blocks::Def> {
        let mut def = app.reg.placeable().find(|d| d.name == self.block).cloned()?;
        let l = def.tone(engine::blocks::ROW[self.season]);
        (def.colour, def.colour2, def.colour3, def.amount, def.contrast) = (l.colour, l.colour2, l.colour3, l.amount, l.contrast);
        Some(def)
    }

    /// The block's kind: opaque or cutout (`see` in its row).
    fn kind(&self, app: &App) -> See {
        app.reg.placeable().find(|d| d.name == self.block).map_or(See::Opaque, |d| d.see)
    }

    /// Writes the block's kind to its row in data/blocks.toml when the game's reader takes it (no undo step).
    fn write_kind(&self, app: &mut App, see: See) -> bool {
        let path = super::BLOCKS;
        let block = self.block.clone();
        let res = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}")).and_then(|old| {
            let row = kind_row(&old, &block, see).ok_or(format!("{path}: no row for {block}"))?;
            crate::rows::save(path, "block", &block, &row, |t| engine::blocks::Registry::parse(t).map(|_| ()))
        });
        match res {
            Ok(new) => {
                app.reg = crate::rows::blocks(&new).0;
                true
            }
            Err(e) => {
                app.refuse(e);
                false
            }
        }
    }

    /// "Kind": the block made opaque or cutout, one undo step (in the open face's history, else the page's). To
    /// opaque, the open face's empty pixels are filled with its mean colour in that step; empty pixels in the
    /// block's other files refuse it, so none are lost unseen.
    fn set_kind(&mut self, app: &mut App, see: See) {
        if self.kind(app) == see {
            return;
        }
        let open = self.file(self.face);
        if see == See::Opaque && !self.may_be_opaque(app) {
            return;
        }
        if !self.write_kind(app, see) {
            return;
        }
        let name = format!("Kind: {}", if see == See::Cutout { "cutout" } else { "opaque" });
        let pal = app.pal.clone();
        match app.docs.get_mut(&open) {
            Some(d) => {
                let before = d.img.clone();
                let filled = see == See::Opaque && art::fill_holes(&mut d.img, &pal);
                d.hist.push(before, &name);
                if filled {
                    app.toast(format!("{}: opaque; the empty pixels of {open} filled with its mean colour (Undo takes it back)", self.block));
                }
            }
            None => {
                self.kinds.0.push((self.block.clone(), see));
                self.kinds.1.clear();
            }
        }
        if see == See::Opaque && self.tool == Tool::Eraser {
            self.tool = Tool::Pencil;
        }
    }

    /// Whether the block may become opaque: no file but the open face's has empty pixels (refused by name if not,
    /// so an undo or a click never leaves a file the game would refuse).
    fn may_be_opaque(&self, app: &mut App) -> bool {
        let open = self.file(self.face);
        let others: Vec<String> = (0..4).flat_map(|q| (0..4).map(move |f| (q, f))).map(|(q, f)| art::season_file(&self.block, f, q)).filter(|n| *n != open && app.docs.get(n).is_some_and(|d| d.img.holes())).collect();
        if !others.is_empty() {
            app.refuse(format!("Opaque: {} {} empty pixels; fill them first (Fill on an empty pixel), only the open face's are filled for you", others.join(", "), if others.len() == 1 { "has" } else { "have" }));
        }
        others.is_empty()
    }

    /// One undo (or redo) of the open face, a kind step writing the kind back; whether there was one.
    fn step(&mut self, app: &mut App, undo: bool) -> bool {
        let other = |s: See| if s == See::Cutout { See::Opaque } else { See::Cutout };
        let Some(d) = app.docs.get(&self.file(self.face)) else {
            // no file of its own: the page's kind steps
            let from = if undo { &self.kinds.0 } else { &self.kinds.1 };
            let Some(&(_, see)) = from.iter().rev().find(|(b, _)| *b == self.block) else { return false };
            let to_see = if undo { other(see) } else { see };
            if to_see == See::Opaque && !self.may_be_opaque(app) {
                return true;
            }
            let (from, to) = if undo { (&mut self.kinds.0, &mut self.kinds.1) } else { (&mut self.kinds.1, &mut self.kinds.0) };
            let Some((b, see)) = from.pop_if(|(b, _)| *b == self.block) else { return false };
            to.push((b, see));
            self.write_kind(app, to_see);
            return true;
        };
        let name = if undo { d.hist.undo.last() } else { d.hist.redo.last() }.map(|s| s.name.clone());
        // a kind step to opaque is refused while another file has empty pixels
        if name.as_deref().and_then(|n| n.strip_prefix("Kind: ")).is_some_and(|k| (k == "cutout") == undo) && !self.may_be_opaque(app) {
            return true;
        }
        let Some(d) = app.docs.get_mut(&self.file(self.face)) else { return false };
        let done = if undo { d.hist.undo(&mut d.img) } else { d.hist.redo(&mut d.img) };
        self.variant = self.variant.min(d.img.variants() - 1);
        if let Some(k) = name.as_deref().and_then(|n| n.strip_prefix("Kind: ")) {
            let see = if k == "cutout" { See::Cutout } else { See::Opaque };
            self.write_kind(app, if undo { other(see) } else { see });
        }
        done
    }

    /// Opens the pattern panel with the file's last recipe this session, else the block's own row.
    fn open_pattern(&mut self, app: &App, every: bool) {
        let key = self.file(self.face);
        let (recipe, was) = match (self.recipes.get(&key), self.def(app)) {
            (Some(r), _) => r.clone(),
            (None, Some(d)) => (pattern::Recipe::of(&d, &app.pal), false),
            (None, None) => return,
        };
        (self.gen, self.wall, self.stroke) = (false, None, None);
        self.pat = Some(PatPanel { recipe, well: None, every: every || was });
    }

    /// The pending picture from the panel's recipe, made again each frame (live while a slider moves).
    fn pat_pending(&mut self, app: &App) {
        let (Some(pp), Some(d)) = (&self.pat, self.def(app)) else { return };
        let base = self.real(app).map(|(i, _)| i.clone()).unwrap_or_else(|| Img::filled(1, tools::snap(&app.pal, self.block_colour(app).map(|v| v as f32 / 255.0))));
        let v = self.variant.min(base.variants() - 1);
        let img = pp.recipe.fill(&base, v, pp.every, &app.reg, &d, self.face, &app.pal);
        self.pending = Some((self.file(self.face), img));
    }

    /// Use: the pending pattern becomes the file, one undo step; the recipe is kept for the file.
    fn pat_use(&mut self, app: &mut App) {
        self.pat_pending(app);
        let Some(pp) = self.pat.take() else { return };
        self.recipes.insert(self.file(self.face), (pp.recipe, pp.every));
        if let Some((f, img)) = self.pending.take() {
            self.edit(app, "Start from pattern", |me, _, _| *me = img);
            app.toast(format!("Pattern used: {f} (unsaved; Undo takes it back)"));
        }
    }

    fn pat_cancel(&mut self) {
        if let Some(pp) = self.pat.take() {
            self.recipes.insert(self.file(self.face), (pp.recipe, pp.every));
        }
        self.pending = None;
    }

    /// The pattern panel in the middle column: the twelve patterns, three colour wells, the values the chosen
    /// one reads, Another, this or every variant, Use and Cancel.
    fn pattern_panel(&mut self, ui: &mut Ui, app: &mut App) {
        let Some(def) = self.def(app) else { return };
        let (face, pal) = (self.face, app.pal.clone());
        let n = self.real(app).map_or(1, |(img, _)| img.variants());
        let Some(pp) = self.pat.as_mut() else { return };
        let mut act = "";
        ui.label(RichText::new("Start from a pattern").strong());
        look::weak(ui, "A pattern draws the face from three colours and a few numbers. Pick one, tune it, then paint over it.");
        ui.add_space(GAP);
        // a fixed grid: a wrapping row cannot know a column's width before it is drawn
        let cols = ((ui.available_width() + 6.0) / 70.0).floor().max(1.0) as usize;
        for line in pattern::NAMES.chunks(cols) {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(6.0);
                for &(p, name) in line {
                    // the chosen one with its values, the others at their own defaults; all with the colours chosen
                    let mut k = pp.recipe.clone();
                    if p != k.pattern {
                        k.set_pattern(p);
                    }
                    let img = k.draw(&app.reg, &def, face, &pal, k.seed);
                    ui.vertical(|ui| {
                        ui.set_width(64.0);
                        ui.spacing_mut().item_spacing = Vec2::splat(2.0);
                        let (r, resp) = ui.allocate_exact_size(Vec2::splat(56.0), Sense::click());
                        crate::drive::mark(ui.ctx(), format!("pat:{name}"), r);
                        draw(ui.painter(), &img, 0, r.min + Vec2::splat(4.0), 3.0, false);
                        let on = p == pp.recipe.pattern;
                        if on || resp.hovered() {
                            ui.painter().rect_stroke(r, 4.0, egui::Stroke::new(if on { 2.0 } else { 1.0 }, if on { look::ACCENT } else { Color32::from_gray(120) }), StrokeKind::Inside);
                        }
                        if look::tip(resp, &format!("{name}: a click picks it"), "").clicked() {
                            pp.recipe.set_pattern(p);
                        }
                        ui.label(if on { RichText::new(name).small().strong() } else { RichText::new(name).small() });
                    });
                }
            });
        }
        ui.add_space(GAP);
        ui.label(RichText::new("Colours").strong());
        let cs = pp.recipe.colours(&pal);
        ui.horizontal(|ui| {
            for (k, name) in ["Base", "Second", "Third"].into_iter().enumerate() {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(2.0);
                    let (r, resp) = ui.allocate_exact_size(Vec2::new(64.0, 36.0), Sense::click());
                    crate::drive::mark(ui.ctx(), format!("well:{k}"), r);
                    ui.painter().rect_filled(r, 4.0, c32(cs[k]));
                    let armed = pp.well == Some(k);
                    ui.painter().rect_stroke(r, 4.0, egui::Stroke::new(if armed { 3.0 } else { 1.0 }, if armed { look::ACCENT } else { Color32::from_gray(90) }), StrokeKind::Inside);
                    let auto = pp.recipe.colours[k].is_none();
                    let help = match k {
                        0 => "The base colour: click here, then a colour under Colours on the right".to_string(),
                        _ => format!("The {} colour (lines, mortar, spots): auto is one step {} than the base on its ramp; click here, then a colour under Colours on the right", name.to_lowercase(), if k == 1 { "darker" } else { "lighter" }),
                    };
                    if look::tip(resp, &help, "").clicked() {
                        pp.well = if armed { None } else { Some(k) };
                    }
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(if auto { format!("{name}: auto") } else { name.to_string() }).small());
                        if k > 0 && !auto && look::tip(ui.small_button("make auto"), "Back to one step darker or lighter than the base", "").clicked() {
                            pp.recipe.colours[k] = None;
                        }
                    });
                    ui.label(RichText::new(colour_name(&pal, cs[k])).small().weak());
                });
            }
        });
        if let Some(k) = pp.well {
            ui.label(RichText::new(format!("Now click a colour for the {} well, here or under Colours on the right", ["base", "second", "third"][k])).color(look::ACCENT));
            let (mut a, mut b) = (cs[k], cs[k]);
            if let Some(c) = swatches(ui, "well", &pal, &mut a, &mut b) {
                pp.recipe.colours[k] = Some(c);
                pp.well = None;
            }
        }
        ui.add_space(GAP);
        ui.label(RichText::new("Values").strong());
        let (rs, ra) = pattern::Recipe::reads(pp.recipe.pattern);
        let (ds, da) = pp.recipe.pattern.defaults().unwrap_or((1, 0.5));
        ui.spacing_mut().slider_width = (ui.available_width() - 190.0).clamp(80.0, 260.0);
        let sw = ui.spacing().slider_width;
        // one value's row: its slider with the number, its tip, and a reset (true when clicked)
        let row = |ui: &mut Ui, id: &str, help: &str, s: egui::Slider| -> bool {
            ui.horizontal(|ui| {
                let r = ui.add(s);
                crate::drive::mark(ui.ctx(), format!("slider:{id}"), Rect::from_min_size(r.rect.min, Vec2::new(sw, r.rect.height())));
                look::tip(r, help, "");
                look::tip(ui.small_button("reset"), "Back to the pattern's own value", "").clicked()
            })
            .inner
        };
        if rs && row(ui, "size", "How large the parts are, in pixels: bricks' height, tiles' and planks' width, the rings' step", egui::Slider::new(&mut pp.recipe.size, 1..=8).text("size")) {
            pp.recipe.size = ds;
        }
        if ra && row(ui, "amount", "How much of the second and third colour: how many spots, threads, cracked boards", egui::Slider::new(&mut pp.recipe.amount, 0.0..=1.0).text("amount")) {
            pp.recipe.amount = da;
        }
        if row(ui, "contrast", "How strongly each pixel's shade varies: 0 flat, 1 as the game draws blocks", egui::Slider::new(&mut pp.recipe.contrast, 0.0..=2.0).text("contrast")) {
            pp.recipe.contrast = 1.0;
        }
        if !rs {
            look::weak(ui, "this pattern takes no size or amount");
        }
        ui.add_space(GAP);
        ui.horizontal_wrapped(|ui| {
            let r = ui.button("Another");
            crate::drive::mark(ui.ctx(), "btn:pat.another", r.rect);
            if look::tip(r, "A new draw of the same values, where the pattern has chance in it (a new seed)", "").clicked() {
                pp.recipe.seed += 1;
            }
            ui.separator();
            let r = ui.radio_value(&mut pp.every, false, "This variant");
            look::tip(r, "Fill only the variant open on the canvas", "");
            let r = ui.radio_value(&mut pp.every, true, if n == 1 { "Every variant (makes 4)" } else { "Every variant" });
            crate::drive::mark(ui.ctx(), "pat:every", r.rect);
            look::tip(r, "Fill every variant, each from its own seed: one pattern, a varied field; a file of one variant gets four", "");
        });
        ui.add_space(GAP);
        ui.horizontal(|ui| {
            let r = ui.add(egui::Button::new(RichText::new("Use").color(Color32::WHITE)).fill(look::GOOD.gamma_multiply(0.45)));
            crate::drive::mark(ui.ctx(), "btn:pat.use", r.rect);
            if look::tip(r, "Put the pattern into the file: one undo step, saved with Save", "").clicked() {
                act = "use";
            }
            let r = ui.button("Cancel");
            crate::drive::mark(ui.ctx(), "btn:pat.cancel", r.rect);
            if look::tip(r, "Close the panel; the file stays as it was", "").clicked() {
                act = "cancel";
            }
        });
        match act {
            "use" => self.pat_use(app),
            "cancel" => self.pat_cancel(),
            _ => {}
        }
    }

    fn start_picture(&mut self, app: &mut App, path: &str) {
        match std::fs::read(path).map_err(|e| e.to_string()).and_then(|b| tools::decode_picture(&b)) {
            Ok((w, h, px)) => {
                let face = tools::from_picture(w, h, &px, &app.pal);
                self.put_variant(app, "Start from a picture", face);
            }
            Err(e) => app.refuse(format!("{path}: {e}")),
        }
    }

    fn copy_block(&mut self, app: &mut App, block: &str) {
        let Some((img, _)) = Self::face_img(app, block, self.face) else {
            return app.refuse(format!("{block} has no painted texture to copy"));
        };
        let img = img.clone();
        self.edit(app, &format!("Copy from {block}"), |me, _, _| *me = img);
        self.variant = 0;
    }

    fn swap_ramp(&mut self, app: &mut App, from: &str, to: &str) {
        let colours = |r: &str| app.pal.ramp(r).iter().map(|&i| app.pal.colours[i].rgb).collect::<Vec<_>>();
        let (a, b) = (colours(from), colours(to));
        let mut err = None;
        self.edit(app, "Swap ramp", |img, _, _| {
            if let Err(e) = art::swap_ramp(img, &a, &b) {
                err = Some(e);
            }
        });
        match err {
            Some(e) => app.refuse(format!("Swap ramp {from} to {to}: {e}")),
            None => app.toast(format!("Swapped the ramp {from} for {to}")),
        }
    }

    fn sel_op(&mut self, app: &mut App, op: &str) {
        let Some(s) = self.sel else { return app.toast("Select a rectangle first (S)") };
        let second = self.second;
        if op == "copy" {
            let v = self.v(app);
            if let Some((img, _)) = self.shown(app) {
                self.clip = Some(tools::copy(img, v, s));
                app.toast(format!("Copied {} x {} pixels", s.w, s.h));
            }
            return;
        }
        let mut ns = s;
        self.edit(app, if op == "turn" { "Turn" } else { "Flip" }, |img, v, _| {
            let c = tools::copy(img, v, s);
            let c = match op {
                "flipx" => c.flip_x(),
                "flipy" => c.flip_y(),
                _ => c.turn(),
            };
            tools::cut(img, v, s, second);
            tools::paste(img, v, &c, s.x as i32, s.y as i32);
            ns = Sel { w: c.w, h: c.h, ..s };
        });
        self.sel = Some(Sel::corners((ns.x, ns.y), (ns.x + ns.w - 1, ns.y + ns.h - 1)));
    }

    fn paste(&mut self, app: &mut App) {
        let Some(c) = self.clip.clone() else { return app.toast("Nothing copied yet (Ctrl+C copies the selection)") };
        let (x, y) = self.sel.map_or((0, 0), |s| (s.x, s.y));
        self.edit(app, "Paste", |img, v, _| tools::paste(img, v, &c, x as i32, y as i32));
        self.sel = Some(Sel::corners((x, y), ((x + c.w - 1).min(S - 1), (y + c.h - 1).min(S - 1))));
        self.tool = Tool::Select;
    }

    /// "Generate variants": the wall made around the file as it is; none while variant 1 is one flat colour.
    fn open_wall(&mut self, app: &mut App) {
        (self.pat, self.pending, self.stroke, self.gen, self.wall) = (None, None, None, true, None);
        let Some((img, _)) = self.real(app) else { return };
        if art::flat(img, 0) {
            return;
        }
        let around = img.clone();
        let (n, pal) = (self.count, app.pal.clone());
        self.wall = Some(Wall::new(around, 9, 0.35, 7, &|a: &Img, r: &mut Rng, k: f32| tools::make_variants(a, n, k, &pal, r)));
    }

    fn wall_close(&mut self) {
        (self.gen, self.wall, self.pending) = (false, None, None);
    }

    /// Use this: the pending set becomes the file, one undo step.
    fn wall_use(&mut self, app: &mut App) {
        if let Some((f, img)) = self.pending.take().filter(|(f, _)| *f == self.file(self.face)) {
            let n = img.variants();
            self.edit(app, "Generate variants", |me, _, _| *me = img);
            app.toast(format!("{f}: {n} variants (unsaved; Undo takes them back)"));
        }
        self.wall_close();
    }

    fn remove_variant(&mut self, app: &mut App, v: usize) {
        self.ask_remove = None;
        self.edit(app, &format!("Remove variant {}", v + 1), |img, _, _| {
            if let Some(less) = art::remove_variant(img, v) {
                *img = less;
            }
        });
        self.variant = self.variant.min(self.real(app).map_or(1, |(i, _)| i.variants()) - 1);
    }

    /// The "Generate variants" panel: each set as a 4 x 4 patch of ground with its variants small under it; a
    /// click makes it pending, Use this keeps it. A flat variant 1 points to the pattern panel instead.
    fn wall_panel(&mut self, ui: &mut Ui, app: &mut App) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Generate variants").strong());
            let r = ui.button("Close");
            crate::drive::mark(ui.ctx(), "btn:wall.close", r.rect);
            if look::tip(r, "Back to the canvas; a set not used is dropped", "").clicked() {
                self.wall_close();
            }
        });
        let mut n = self.count;
        ui.spacing_mut().slider_width = 100.0;
        let r = ui.add(egui::Slider::new(&mut n, 2..=4).text("variants in all"));
        look::tip(r, "How many variants each set has, your variant 1 among them", "");
        if n != self.count {
            self.count = n;
            self.open_wall(app);
        }
        look::weak(ui, "Each set keeps your variant 1 and changes a share of its pixels for the others.");
        ui.add_space(GAP);
        if !self.gen {
            return;
        }
        if self.wall.is_none() {
            ui.label(RichText::new("Variant 1 is one flat colour: there is nothing to vary.").strong());
            look::weak(ui, "Start from a pattern instead: it can fill every variant, each from its own seed, for a varied field at once. Or paint variant 1 first.");
            ui.add_space(GAP * 0.5);
            let r = ui.button("Start from a pattern");
            crate::drive::mark(ui.ctx(), "btn:flat.pattern", r.rect);
            if look::tip(r, "The pattern panel, set to fill every variant", "").clicked() {
                self.open_pattern(app, true);
            }
            return;
        }
        let pending = self.pending.as_ref().filter(|(f, _)| *f == self.file(self.face)).map(|p| p.1.clone());
        ui.horizontal(|ui| {
            let r = ui.add_enabled(pending.is_some(), egui::Button::new(RichText::new("Use this").color(Color32::WHITE)).fill(look::GOOD.gamma_multiply(0.45)));
            crate::drive::mark(ui.ctx(), "btn:wall.use", r.rect);
            if look::tip(r, "Put the picked set into the file: one undo step, saved with Save", "").clicked() {
                self.wall_use(app);
            }
            let r = ui.add_enabled(pending.is_some(), egui::Button::new("Cancel"));
            if look::tip(r, "Drop the picked set; the file stays as it was", "").clicked() {
                self.pending = None;
            }
        });
        // its own line: a long label in a row widens the panel past its column
        look::weak(ui, if pending.is_some() { "Shown on the canvas and the block: Use this keeps it." } else { "A click on a set shows it on the canvas and the block." });
        ui.add_space(GAP * 0.5);
        let (n, pal, file) = (self.count, app.pal.clone(), self.file(self.face));
        let gen = |a: &Img, r: &mut Rng, k: f32| tools::make_variants(a, n, k, &pal, r);
        let Some(w) = self.wall.as_mut() else { return };
        let chosen = pending.as_ref().and_then(|p| w.items.iter().position(|i| i == p));
        // a set is ground: 4 x 4 blocks mixed as the game mixes them, 2.5 px a texel, its variants small under it
        let k = 2.5;
        let patch = 4.0 * S as f32 * k;
        let cell = Vec2::new(patch + 12.0, patch + 12.0 + 4.0 + 2.0 * S as f32);
        let act = wall_ui::show(ui, w, chosen, cell, |p, r, img| {
            for j in 0..4 {
                for i in 0..4 {
                    draw(p, img, art::variant_at([i, j, 0], img.variants()), r.min + Vec2::new(i as f32, j as f32) * S as f32 * k, k, false);
                }
            }
            for v in 0..img.variants() {
                draw(p, img, v, r.min + Vec2::new(v as f32 * (2.0 * S as f32 + 6.0), patch + 4.0), 2.0, false);
            }
        });
        match act {
            Act::Take(i) => self.pending = Some((file, w.items[i].clone())),
            Act::More(i) => w.more_like(i, &gen),
            Act::Back => {
                w.back();
            }
            Act::Shuffle => w.shuffle(&gen),
            Act::Spread => w.fill(9, &gen),
            Act::None | Act::Play(_) => {}
        }
    }

    fn tabs(&mut self, ui: &mut Ui, app: &mut App) {
        ui.horizontal_wrapped(|ui| {
            for q in 0..4 {
                let own = self.has_season(app, q);
                let t = if own { RichText::new(SEASON_NAMES[q]) } else { RichText::new(SEASON_NAMES[q]).weak() };
                let help = match q {
                    1 => "Summer: the base look and files, <block>.png".to_string(),
                    _ => format!("The block in {}: <block>@{}.png{}", engine::blocks::SEASONS[q], engine::blocks::SEASONS[q], if own { "" } else { "; dimmed: it shows the base until added" }),
                };
                let r = ui.selectable_label(self.season == q, t);
                crate::drive::mark(ui.ctx(), &format!("season:{q}"), r.rect);
                if look::tip(r, &help, "").clicked() {
                    self.run(app, &format!("season:{}", engine::blocks::SEASONS[q]));
                }
            }
            if self.season != 1 {
                let own = self.has_season(app, self.season);
                if !own && look::tip(ui.button("Add season"), "Copy the base into this season: its painted files and its look in data/blocks.toml", "").clicked() {
                    self.add_season(app);
                }
                if own && look::tip(ui.button("Remove"), "Drop this season: its painted files (from the disk too) and its look; the base shows again", "").clicked() {
                    self.remove_season(app);
                }
            }
            ui.end_row();
            for f in 0..4 {
                let own = app.docs.contains_key(&self.file(f));
                let t = if own { RichText::new(FACE_NAMES[f]) } else { RichText::new(FACE_NAMES[f]).weak() };
                let help = match f {
                    0 => "Every face: <block>.png".to_string(),
                    _ => format!("Only the {} face: <block>_{}.png{}", FACE_NAMES[f].to_lowercase(), art::FACES[f], if own { "" } else { "; dimmed: it falls back to All" }),
                };
                let r = ui.selectable_label(self.face == f, t);
                crate::drive::mark(ui.ctx(), format!("facetab:{f}"), r.rect);
                if look::tip(r, &help, "").clicked() {
                    self.leave(f, self.season);
                }
            }
            ui.separator();
            let n = self.shown(app).map_or(1, |(img, _)| img.variants());
            const WHY: &str = "The game gives each block one of these by its place, so a field does not repeat.";
            look::tip(ui.label("Variant"), WHY, "");
            for v in 0..n {
                let r = ui.selectable_label(self.variant == v, format!("{}", v + 1));
                crate::drive::mark(ui.ctx(), format!("variant:{}", v + 1), r.rect);
                if look::tip(r, &format!("Variant {}. {WHY}", v + 1), &format!("{}", v + 1)).clicked() {
                    self.variant = v;
                }
                if n > 1 && self.real(app).is_some_and(|(_, own)| own) {
                    let r = ui.add(egui::Button::new(RichText::new("x").small()).frame(false));
                    crate::drive::mark(ui.ctx(), format!("variant.remove:{}", v + 1), r.rect);
                    if look::tip(r, &format!("Remove variant {} (asks first when it holds paint; Undo brings it back)", v + 1), "").clicked() {
                        self.run(app, &format!("variant.remove={}", v + 1));
                    }
                }
            }
            if n < 4 && look::tip(ui.button("+"), "A new variant, copied from the open one", "").clicked() {
                self.run(app, "variant.add");
            }
            if let Some(v) = self.ask_remove {
                ui.label(RichText::new(format!("Remove variant {}? It holds paint.", v + 1)).color(look::DRAFT));
                let r = ui.button("Remove");
                crate::drive::mark(ui.ctx(), "btn:variant.remove", r.rect);
                if look::tip(r, "Remove it; Undo brings it back", "").clicked() {
                    self.remove_variant(app, v);
                }
                if look::tip(ui.button("Keep"), "Keep the variant", "").clicked() {
                    self.ask_remove = None;
                }
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

    /// The pointer on the canvas starts and carries strokes. `cell`: the canvas pixel under the pointer in face
    /// coordinates (outside 0..16 on the ghosted neighbours), None when off the canvas.
    fn paint(&mut self, ui: &Ui, app: &mut App, cell: Option<(i32, i32)>) {
        let (alt, pressed) = ui.input(|i| (i.modifiers.alt, [PointerButton::Primary, PointerButton::Secondary].map(|b| i.pointer.button_pressed(b))));
        let tool = if alt { Tool::Picker } else { self.tool };
        let (v, mirror, tiled) = (self.v(app), self.mirror, self.tiled);
        let on_face = |c: (i32, i32)| (0..S as i32).contains(&c.0) && (0..S as i32).contains(&c.1);
        if let Some(px) = cell.filter(|c| self.stroke.is_none() && (tiled || on_face(*c))) {
            for (k, button) in [PointerButton::Primary, PointerButton::Secondary].into_iter().enumerate() {
                if !pressed[k] {
                    continue;
                }
                let (c, other) = if k == 0 { (self.first, self.second) } else { (self.second, self.first) };
                let (x, y) = (px.0.rem_euclid(S as i32) as usize, px.1.rem_euclid(S as i32) as usize);
                let mut lifted = None;
                match tool {
                    Tool::Picker => {
                        let got = self.shown(app).map(|(img, _)| img.get(v.min(img.variants() - 1), x, y));
                        if let Some(got) = got.filter(|g| app.pal.find(*g).is_some()) {
                            *(if k == 0 { &mut self.first } else { &mut self.second }) = got;
                        }
                        continue;
                    }
                    Tool::Fill => {
                        self.edit(app, "Fill", |img, v, _| if tiled { tools::fill_wrap(img, v, x, y, c) } else { art::fill(img, v, x, y, c, mirror) });
                        continue;
                    }
                    Tool::Dither => {
                        self.edit(app, "Dither fill", |img, v, _| tools::dither(img, v, x, y, c, other, tiled));
                        continue;
                    }
                    Tool::Replace => {
                        let from = self.shown(app).map_or(c, |(img, _)| img.get(v, x, y));
                        self.edit(app, "Replace colour", |img, _, _| {
                            art::repaint(img, from, c);
                        });
                        continue;
                    }
                    Tool::Gradient if app.pal.between(self.first, self.second).is_none() => {
                        app.refuse("Gradient: the first and second colour must be on one ramp");
                        continue;
                    }
                    Tool::Select => match self.sel.filter(|s| s.contains((x, y))) {
                        Some(s) => {
                            let d = self.doc(app);
                            lifted = Some((s, tools::copy(&d.img, v, s)));
                        }
                        None => self.sel = Some(Sel::corners((x, y), (x, y))),
                    },
                    _ => {}
                }
                let name = TOOLS.iter().find(|t| t.0 == tool).unwrap().2;
                let d = self.doc(app);
                let base = d.img.clone();
                // a selection drawn changes nothing; a move does
                if tool != Tool::Select || lifted.is_some() {
                    d.hist.push(base.clone(), if tool == Tool::Select { "Move" } else { name });
                }
                self.stroke = Some(Stroke { button, tool, start: px, last: px, base, touched: vec![], lifted });
                self.carry(app, px);
            }
        }
        let Some(st) = &self.stroke else { return };
        if !ui.input(|i| i.pointer.button_down(st.button)) {
            // a stroke that changed nothing leaves no step behind
            let tool = st.tool;
            if let Some(d) = app.docs.get_mut(&self.file(self.face)) {
                if tool != Tool::Select && d.hist.undo.last().is_some_and(|s| s.img == d.img) {
                    d.hist.undo.pop();
                }
            }
            self.stroke = None;
            return;
        }
        if let Some(to) = cell {
            let to = if tiled { (to.0.clamp(-16, 31), to.1.clamp(-16, 31)) } else { (to.0.clamp(0, 15), to.1.clamp(0, 15)) };
            if to != st.last {
                self.carry(app, to);
            }
        }
    }

    /// Carries a stroke on to canvas pixel `to`.
    fn carry(&mut self, app: &mut App, to: (i32, i32)) {
        let v = self.v(app);
        let Some(mut st) = self.stroke.take() else { return };
        let (mirror, first, second) = (self.mirror, self.first, self.second);
        let c = if st.button == PointerButton::Primary { first } else { second };
        let pal = app.pal.clone();
        let wrapped = |c: (i32, i32)| (c.0.rem_euclid(S as i32) as usize, c.1.rem_euclid(S as i32) as usize);
        let path = tools::line_points(st.last, to);
        let (amount, radius, density) = (self.amount, self.radius, self.density);
        let mut sel = self.sel;
        let rng = &mut self.rng;
        let key = art::file_name(&self.block, self.face);
        let Some(d) = app.docs.get_mut(&key) else { return };
        match st.tool {
            Tool::Pencil => tools::line_wrap(&mut d.img, v, st.last, to, c, mirror),
            Tool::Line => {
                d.img = st.base.clone();
                tools::line_wrap(&mut d.img, v, st.start, to, c, mirror);
            }
            Tool::Shade | Tool::Scatter => {
                for p in path.into_iter().map(wrapped) {
                    if st.touched.contains(&p) {
                        continue;
                    }
                    st.touched.push(p);
                    if st.tool == Tool::Shade {
                        tools::shade(&mut d.img, v, p.0, p.1, &pal, if st.button == PointerButton::Primary { -1 } else { 1 }, mirror);
                    } else {
                        art::pencil(&mut d.img, v, p.0, p.1, tools::scatter_colour(&pal, c, amount, rng), mirror);
                    }
                }
            }
            Tool::Spray => tools::spray(&mut d.img, v, to, radius, density, c, mirror, rng),
            Tool::Eraser => tools::line_wrap(&mut d.img, v, st.last, to, art::EMPTY, mirror),
            Tool::Gradient => {
                d.img = st.base.clone();
                let (a, b) = if st.button == PointerButton::Primary { (first, second) } else { (second, first) };
                if let Some(ramp) = pal.between(a, b) {
                    let mid = |p: (i32, i32)| (p.0 as f32 + 0.5, p.1 as f32 + 0.5);
                    tools::gradient(&mut d.img, v, mid(st.start), mid(to), &ramp);
                }
            }
            Tool::Select => match &st.lifted {
                Some((s, clip)) => {
                    d.img = st.base.clone();
                    let (dx, dy) = (to.0 - st.start.0, to.1 - st.start.1);
                    tools::cut(&mut d.img, v, *s, second);
                    tools::paste(&mut d.img, v, clip, s.x as i32 + dx, s.y as i32 + dy);
                    let (nx, ny) = ((s.x as i32 + dx).clamp(0, (S - s.w) as i32) as usize, (s.y as i32 + dy).clamp(0, (S - s.h) as i32) as usize);
                    sel = Some(Sel { x: nx, y: ny, ..*s });
                }
                None => sel = Some(Sel::corners(wrapped(st.start), wrapped(to))),
            },
            _ => {}
        }
        self.sel = sel;
        st.last = to;
        self.stroke = Some(st);
    }

    fn palette(&mut self, ui: &mut Ui, app: &App) {
        let before = self.first;
        let clicked = palette_picker(ui, "textures", &app.pal, &mut self.first, &mut self.second);
        // a well of the pattern panel waiting for a colour takes the click instead of the first colour
        if let (Some(c), Some(pp)) = (clicked, self.pat.as_mut()) {
            if let Some(k) = pp.well.take() {
                pp.recipe.colours[k] = Some(c);
                self.first = before;
            }
        }
    }

    /// The fixed box on top of the inspector: the chips, then the block or a flat preview, never scrolled away.
    fn preview_box(&mut self, ui: &mut Ui, app: &mut App) {
        ui.horizontal(|ui| {
            for (k, (id, name, help)) in PREVIEWS.iter().enumerate() {
                let r = look::chip(ui, self.preview == k, name);
                crate::drive::mark(ui.ctx(), format!("preview:{id}"), r.rect);
                if look::tip(r, help, "").clicked() {
                    self.preview = k;
                }
            }
        });
        if self.preview == 0 {
            self.lit_cube(ui, app);
        } else {
            self.flat_preview(ui, app);
        }
        ui.add_space(GAP * 0.25);
        ui.separator();
    }

    /// The tiled or unfolded face in the box, centred.
    fn flat_preview(&mut self, ui: &mut Ui, app: &App) {
        let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), BOX), Sense::hover());
        crate::drive::mark(ui.ctx(), "preview", r);
        let p = ui.painter_at(r);
        p.rect_filled(r, 4.0, ui.visuals().extreme_bg_color);
        let note = |p: &egui::Painter, t: &str| {
            p.text(r.center(), egui::Align2::CENTER_CENTER, t, egui::FontId::proportional(look::SMALL), Color32::from_gray(140));
        };
        let fit = |w: f32, h: f32| ((r.width() - GAP) / w).min((r.height() - GAP) / h).floor().clamp(1.0, 6.0);
        if self.preview == 3 {
            let pend = |f: usize| self.pending.as_ref().filter(|p| p.0 == art::file_name(&self.block, f)).map(|p| (&p.1, true));
            let faces = [1, 2, 3].map(|f| pend(f).or_else(|| Self::face_img(app, &self.block, f).filter(|(_, own)| *own)).or_else(|| pend(0)).or_else(|| Self::face_img(app, &self.block, f)));
            if faces.iter().all(Option::is_none) {
                return note(&p, "nothing painted yet");
            }
            let scale = fit(4.0 * S as f32, 3.0 * S as f32);
            let t = S as f32 * scale;
            let at0 = (r.center() - Vec2::new(2.0 * t, 1.5 * t)).round();
            let flat = dim(self.block_colour(app));
            for (f, i, j) in [(0, 1, 0), (1, 0, 1), (1, 1, 1), (1, 2, 1), (1, 3, 1), (2, 1, 2)] {
                let at = at0 + Vec2::new(i as f32, j as f32) * t;
                match faces[f] {
                    Some((img, _)) => draw(&p, img, 0, at, scale, false),
                    None => {
                        p.rect_filled(Rect::from_min_size(at, Vec2::splat(t)), 0.0, flat);
                    }
                }
            }
            return;
        }
        let Some((img, _)) = self.shown(app) else {
            return note(&p, "nothing painted yet");
        };
        let n = if self.preview == 2 { 5 } else { 3 };
        // whole pixels per texel: a fractional scale leaves hairline gaps between the tiles
        let scale = fit((n * S) as f32, (n * S) as f32);
        let at0 = (r.center() - Vec2::splat(n as f32 * S as f32 * scale / 2.0)).round();
        for j in 0..n {
            for i in 0..n {
                let at = at0 + Vec2::new(i as f32, j as f32) * S as f32 * scale;
                draw(&p, img, art::variant_at([i as i32, j as i32, 0], img.variants()), at, scale, false);
            }
        }
    }

    /// The block in the game's renderer (ADR 0049): on the ground among its own kind, lit by the sun (or a lamp in
    /// the cave), with the textures as they are in the Studio, saved or not.
    fn lit_cube(&mut self, ui: &mut Ui, app: &mut App) {
        use std::hash::{Hash, Hasher};
        let pend = self.pending.as_ref();
        let files: Vec<(&String, &Img)> = app.docs.iter().filter(|(n, _)| pend.is_none_or(|p| p.0 != **n)).map(|(n, d)| (n, &d.img)).chain(pend.map(|p| (&p.0, &p.1))).collect();
        let mut h = std::hash::DefaultHasher::new();
        for (n, img) in &files {
            (n, img.w, &img.px).hash(&mut h);
        }
        let gen = h.finish();
        let dir = format!("{}/textures", super::viewport::DIR);
        // while a stroke is held: at most every FOLLOW_MS and only the open file; else every file anew
        let held = ui.input(|i| i.pointer.any_down());
        if gen != self.tex_gen && (!held || self.tex_at.elapsed().as_millis() >= FOLLOW_MS) {
            let t0 = std::time::Instant::now();
            if held {
                let n = self.file(self.face);
                if let Some((_, img)) = files.iter().find(|(f, _)| **f == n) {
                    let _ = std::fs::create_dir_all(&dir);
                    let _ = std::fs::write(format!("{dir}/{n}"), art::encode(img));
                }
            } else {
                let _ = std::fs::remove_dir_all(&dir);
                let _ = std::fs::create_dir_all(&dir);
                for (n, img) in &files {
                    let _ = std::fs::write(format!("{dir}/{n}"), art::encode(img));
                }
            }
            self.tex_ms = t0.elapsed().as_secs_f32() * 1000.0;
            (self.tex_gen, self.tex_at) = (gen, std::time::Instant::now());
        }
        app.view.textures(&dir, self.tex_gen);
        app.view.show_model(None, None, 0.0);
        let b = self.block.replace(' ', "_");
        let mut place = vec![format!("0:0:0:{b}")];
        for di in -1..=1 {
            for dj in -1..=1 {
                place.push(format!("{di}:{dj}:-1:{b}"));
            }
        }
        app.view.place = place.join(",");
        app.view.orbit = self.orbit;
        app.view.left_turns = true;
        app.view.live = false;
        let r = app.view.ui(ui, BOX, |p, _, r| {
            p.text(r.center(), egui::Align2::CENTER_CENTER, "the lit block comes with the game's renderer", egui::FontId::proportional(look::SMALL), Color32::from_gray(140));
        });
        let help = format!("The block in the game's renderer: drag turns, the wheel zooms, the middle drag pans, a double click puts the camera back (textures written in {:.1} ms)", self.tex_ms);
        let r = look::tip(r, &help, "");
        self.orbit = if r.double_clicked() { home() } else { app.view.orbit };
        app.view.land_only = true;
    }

    fn settle_view(&mut self, ctx: &egui::Context, app: &mut App) {
        app.view.settle(ctx, std::time::Duration::from_secs(90));
    }

    fn tool_options(&mut self, ui: &mut Ui, app: &mut App) {
        ui.spacing_mut().slider_width = (ui.available_width() - 165.0).max(60.0); // room for the value box and the label
        match self.tool {
            Tool::Scatter => {
                look::tip(ui.add(egui::Slider::new(&mut self.amount, 0.0..=1.0).text("amount")), "How many pixels take a ramp neighbour instead of the colour", "");
            }
            Tool::Spray => {
                look::tip(ui.add(egui::Slider::new(&mut self.radius, 1..=6).text("radius")), "The spray's radius in pixels", "");
                look::tip(ui.add(egui::Slider::new(&mut self.density, 0.05..=1.0).text("density")), "The chance each pixel in the radius is painted", "");
            }
            Tool::Gradient => match app.pal.between(self.first, self.second) {
                Some(r) => {
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::hover());
                    let w = rect.width() / r.len() as f32;
                    for (i, c) in r.iter().enumerate() {
                        ui.painter().rect_filled(Rect::from_min_size(rect.min + Vec2::new(i as f32 * w, 0.0), Vec2::new(w.ceil(), 18.0)), 0.0, c32(*c));
                    }
                    look::weak(ui, "the run from the first to the second colour");
                }
                None => {
                    look::weak(ui, "choose a first and second colour on one ramp");
                }
            },
            Tool::Select => {
                ui.horizontal_wrapped(|ui| {
                    for (id, label, help, key) in [
                        ("sel.flipx", "Flip x", "Mirror the selection left to right", ""),
                        ("sel.flipy", "Flip y", "Mirror the selection top to bottom", ""),
                        ("sel.turn", "Turn", "Turn the selection a quarter clockwise", ""),
                        ("sel.copy", "Copy", "Copy the selection, to paste on any face, variant or block", "Ctrl+C"),
                        ("sel.paste", "Paste", "Paste what was copied at the selection's corner", "Ctrl+V"),
                    ] {
                        if look::tip(ui.button(label), help, key).clicked() {
                            self.run(app, id);
                        }
                    }
                });
                look::weak(ui, match self.sel {
                    Some(s) => format!("{} x {} at {}, {}; a move leaves the second colour behind; the arrows shift the face", s.w, s.h, s.x, s.y),
                    None => "drag on the canvas to select; the arrows shift the whole face".into(),
                });
            }
            Tool::Replace => {
                look::weak(ui, "every pixel of the clicked colour, in every variant of the file");
            }
            Tool::Eraser => {
                look::weak(ui, "empty pixels show as a checkerboard; in the game the faces behind show through");
            }
            _ => {}
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
                // every face in every season, the base's first (its thumbnail)
                let files: Vec<String> = [1, 0, 2, 3].into_iter().flat_map(|q| (0..4).map(move |f| art::season_file(&d.name, f, q))).collect();
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
        v.push(Cmd::new("kind.opaque", "Kind: opaque", "the block hides what is behind it; the open face's empty pixels are filled with its mean colour"));
        v.push(Cmd::new("kind.cutout", "Kind: cutout", "the block's pixels are there or empty (the Eraser); the faces behind it are drawn"));
        v.push(Cmd::new("mirror", "Mirror on or off", "paint both halves at once, mirrored across the middle").key(Modifiers::NONE, Key::M));
        v.push(Cmd::new("tiled", "Tiled painting on or off", "the neighbours ghosted round the face; strokes wrap over the edges").key(Modifiers::NONE, Key::T));
        v.push(Cmd::new("seams", "Seam view on or off", "mark edge pixels that clash with the far edge they meet when tiled").key(Modifiers::SHIFT, Key::T));
        for (n, k) in [Key::Num1, Key::Num2, Key::Num3, Key::Num4].into_iter().enumerate() {
            v.push(Cmd::new(format!("variant.{}", n + 1), format!("Variant {}", n + 1), "show and paint this variant").key(Modifiers::NONE, k));
        }
        v.push(Cmd::new("variant.add", "Add a variant", "a new variant (up to 4), copied from this one"));
        for (q, name) in SEASON_NAMES.iter().enumerate() {
            v.push(Cmd::new(format!("season:{}", engine::blocks::SEASONS[q]), format!("Season: {name}"), "paint the block's look in this season (summer is the base)"));
        }
        v.push(Cmd::new("season.add", "Add the season", "copy the base into this season: its painted files and its look in data/blocks.toml"));
        v.push(Cmd::new("season.remove", "Remove the season", "drop this season's painted files and look; the base shows again"));
        v.push(Cmd::new("variants", "Generate variants", "sets of variants made from variant 1, each as a patch of ground; a click shows one, Use this keeps it"));
        v.push(Cmd::new("wall.use", "Use this set of variants", "the pending set into the file, one undo step"));
        v.push(Cmd::new("wall.cancel", "Cancel the set", "the file stays as it was"));
        for (f, name) in FACE_NAMES.iter().enumerate() {
            v.push(Cmd::new(format!("face.{f}"), format!("Face: {name}"), "paint this face's own file"));
        }
        for (id, k) in [("left", Key::ArrowLeft), ("right", Key::ArrowRight), ("up", Key::ArrowUp), ("down", Key::ArrowDown)] {
            v.push(Cmd::new(format!("shift.{id}"), format!("Shift the face {id}"), "move every pixel one step, wrapping over the edge").key(Modifiers::NONE, k));
        }
        v.push(Cmd::new("sel.copy", "Copy the selection", "to paste on any face, variant or block").key(Modifiers::COMMAND, Key::C));
        v.push(Cmd::new("sel.paste", "Paste", "what was copied, at the selection's corner").key(Modifiers::COMMAND, Key::V));
        v.push(Cmd::new("sel.flipx", "Flip the selection left to right", "mirror it in place"));
        v.push(Cmd::new("sel.flipy", "Flip the selection top to bottom", "mirror it in place"));
        v.push(Cmd::new("sel.turn", "Turn the selection", "a quarter clockwise"));
        v.push(Cmd::new("sel.all", "Select all", "the whole face").key(Modifiers::COMMAND, Key::A));
        v.push(Cmd::new("start.pattern", "Start from a pattern", "the pattern panel: pick a pattern, its colours and values, see it live, then Use"));
        v.push(Cmd::new("pat.use", "Use the pattern", "the pending pattern into the file, one undo step"));
        v.push(Cmd::new("pat.cancel", "Cancel the pattern", "close the pattern panel; the file stays as it was"));
        v.push(Cmd::new("pat.another", "Another draw of the pattern", "a new seed for the same values"));
        v.push(Cmd::new("undo", "Undo", "undo the last change to this file").key(Modifiers::COMMAND, Key::Z));
        v.push(Cmd::new("redo", "Redo", "redo what was undone").key(Modifiers::COMMAND, Key::Y));
        v.push(Cmd::new("save", "Save the block", "write the block's unsaved files and take them off the drafts list").key(Modifiers::COMMAND, Key::S));
        v.push(Cmd::new("zoom.fit", "Zoom to fit", "the canvas as large as fits, centred"));
        v.push(Cmd::new("preview.block", "Preview the block", "the block in the game's renderer, on top of the inspector"));
        v.push(Cmd::new("preview.3", "Preview 3 x 3", "the face tiled, variants mixed as the game mixes them"));
        v.push(Cmd::new("preview.5", "Preview 5 x 5", "a larger tiled preview"));
        v.push(Cmd::new("preview.unfold", "Preview unfolded", "top, sides and bottom unfolded"));
        v
    }

    fn run(&mut self, app: &mut App, id: &str) {
        if let Some(t) = id.strip_prefix("tool.").and_then(|t| TOOLS.iter().find(|x| x.1 == t)) {
            if t.0 == Tool::Eraser && self.kind(app) != See::Cutout {
                return app.refuse(ERASER_OFF);
            }
            self.tool = t.0;
            return;
        }
        if let Some(n) = id.strip_prefix("variant.").and_then(|n| n.parse::<usize>().ok()) {
            if self.shown(app).is_some_and(|(img, _)| n >= 1 && n <= img.variants()) {
                self.variant = n - 1;
            }
            return;
        }
        if let Some(q) = id.strip_prefix("season:").and_then(|q| engine::blocks::SEASONS.iter().position(|n| *n == q)) {
            // the season painted, and the game view's year at its start so the lit cube shows it
            self.leave(self.face, q);
            app.view.day = super::viewport::season_day(q);
            return;
        }
        if let Some(f) = id.strip_prefix("face.").and_then(|n| n.parse::<usize>().ok()).filter(|f| *f < 4) {
            self.leave(f, self.season);
            return;
        }
        // with an argument (shots, scripts): `picture=<path>`, `copy=<block>`, `swap=<from>;<to>`,
        // `colours=<first>;<second>`, `sel=x;y;w;h`, `take=<n>` (an item of the variants wall)
        if let Some((k, arg)) = id.split_once('=') {
            match k {
                "picture" => self.start_picture(app, arg),
                "copy" => self.copy_block(app, arg),
                "swap" => {
                    if let Some((a, b)) = arg.split_once(';') {
                        self.swap_ramp(app, a, b);
                    }
                }
                "colours" => {
                    let find = |n: &str| app.pal.colours.iter().find(|c| c.name == n).map(|c| c.rgb);
                    if let Some((a, b)) = arg.split_once(';') {
                        self.first = find(a).unwrap_or(self.first);
                        self.second = find(b).unwrap_or(self.second);
                    }
                }
                "sel" => {
                    let n: Vec<usize> = arg.split(';').filter_map(|x| x.parse().ok()).collect();
                    if let [x, y, w, h] = n[..] {
                        self.sel = Some(Sel::corners((x, y), (x + w.max(1) - 1, y + h.max(1) - 1)));
                    }
                }
                "pattern" => {
                    if self.pat.is_none() {
                        self.open_pattern(app, false);
                    }
                    if let (Some(pp), Some((p, _))) = (self.pat.as_mut(), pattern::NAMES.iter().find(|n| n.1 == arg)) {
                        pp.recipe.set_pattern(*p);
                    }
                }
                "take" => {
                    if let Some(item) = arg.parse::<usize>().ok().and_then(|i| self.wall.as_ref()?.items.get(i).cloned()) {
                        self.pending = Some((self.file(self.face), item));
                    }
                }
                "variant.remove" => {
                    let v = arg.parse::<usize>().unwrap_or(0).saturating_sub(1);
                    match self.real(app) {
                        Some((img, true)) if v < img.variants() && img.variants() > 1 => {
                            if art::flat(img, v) {
                                self.remove_variant(app, v);
                            } else {
                                self.ask_remove = Some(v);
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
            return;
        }
        match id {
            "mirror" => self.mirror = !self.mirror,
            "tiled" => (self.tiled, self.zoom, self.pan) = (!self.tiled, None, Vec2::ZERO),
            "seams" => self.seams = !self.seams,
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
            "variants" => self.open_wall(app),
            "wall.use" => self.wall_use(app),
            "wall.cancel" => self.wall_close(),
            "season.add" => self.add_season(app),
            "season.remove" => self.remove_season(app),
            "shift.left" | "shift.right" | "shift.up" | "shift.down" => {
                let (dx, dy) = match id {
                    "shift.left" => (-1, 0),
                    "shift.right" => (1, 0),
                    "shift.up" => (0, -1),
                    _ => (0, 1),
                };
                self.edit(app, "Shift", |img, v, _| tools::shift(img, v, dx, dy));
            }
            "sel.copy" => self.sel_op(app, "copy"),
            "sel.paste" => self.paste(app),
            "sel.flipx" => self.sel_op(app, "flipx"),
            "sel.flipy" => self.sel_op(app, "flipy"),
            "sel.turn" => self.sel_op(app, "turn"),
            "sel.all" => {
                self.sel = Some(Sel { x: 0, y: 0, w: S, h: S });
                self.tool = Tool::Select;
            }
            "start.pattern" => self.open_pattern(app, false),
            "pat.use" => self.pat_use(app),
            "pat.cancel" => self.pat_cancel(),
            "pat.another" => {
                if let Some(pp) = self.pat.as_mut() {
                    pp.recipe.seed += 1;
                }
            }
            "undo" | "redo" if self.stroke.is_none() => {
                if !self.step(app, id == "undo") && app.docs.contains_key(&self.file(self.face)) {
                    app.toast(format!("Nothing to {id}"));
                }
            }
            "kind.opaque" => self.set_kind(app, See::Opaque),
            "kind.cutout" => self.set_kind(app, See::Cutout),
            "save" => {
                let files: Vec<String> = (0..4).flat_map(|q| (0..4).map(move |f| (q, f))).map(|(q, f)| art::season_file(&self.block, f, q)).filter(|f| app.docs.get(f).is_some_and(Doc::dirty)).collect();
                if files.is_empty() {
                    app.toast(format!("{}: nothing to save", self.block));
                }
                app.save_textures(files);
            }
            "zoom.fit" => (self.zoom, self.pan) = (None, Vec2::ZERO),
            "preview.block" => self.preview = 0,
            "preview.3" => self.preview = 1,
            "preview.5" => self.preview = 2,
            "preview.unfold" => self.preview = 3,
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
            ("Drop a PNG", "start this variant from the picture"),
        ]
    }

    fn help(&self) -> String {
        let t = TOOLS.iter().find(|t| t.0 == self.tool).unwrap();
        let mut s = format!("{}   [{}]", t.4, t.3.name());
        for (on, what) in [(self.mirror, "mirror [M]"), (self.tiled, "tiled [T]"), (self.seams, "seams [Shift+T]")] {
            if on {
                s += &format!("   · {what}");
            }
        }
        s
    }

    fn history(&self, app: &App) -> Option<(Vec<String>, Vec<String>)> {
        let d = app.docs.get(&self.file(self.face));
        Some(d.map_or((vec![], vec![]), |d| (d.hist.undo.iter().map(|s| s.name.clone()).collect(), d.hist.redo.iter().rev().map(|s| s.name.clone()).collect())))
    }
    fn back_to(&mut self, app: &mut App, n: usize) {
        // step by step, so a kind step on the way writes its kind back
        let f = self.file(self.face);
        let len = |app: &App| app.docs.get(&f).map(|d| d.hist.undo.len());
        while len(app).is_some_and(|l| l > n) && self.step(app, true) {}
        while len(app).is_some_and(|l| l < n) && self.step(app, false) {}
    }
    fn refresh(&mut self, app: &App) {
        self.variant = self.v(app);
        if self.tool == Tool::Eraser && self.kind(app) != See::Cutout {
            self.tool = Tool::Pencil;
        }
        if app.pal.find(self.first).is_none() {
            self.first = app.pal.colours.first().map_or([128; 3], |c| c.rgb);
        }
        if app.pal.find(self.second).is_none() {
            self.second = app.pal.colours.get(1).or(app.pal.colours.first()).map_or([128; 3], |c| c.rgb);
        }
    }

    fn settle(&mut self, ctx: &egui::Context, app: &mut App) {
        self.settle_view(ctx, app);
    }

    fn work(&mut self, ui: &mut Ui, app: &mut App) {
        self.refresh(app);
        self.tabs(ui, app);
        ui.add_space(GAP * 0.5);
        // a PNG dropped on the window starts this variant from it
        let dropped: Vec<std::path::PathBuf> = ui.ctx().input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        if let Some(p) = dropped.first() {
            self.start_picture(app, &p.to_string_lossy());
        }
        // the panel's own room, never what a wide row of tabs pushed it to
        let mut room = ui.available_rect_before_wrap().intersect(ui.clip_rect());
        if self.pat.is_some() || self.gen {
            // the panel on the left, the canvas beside it showing what is pending
            let left = Rect::from_min_size(room.min, Vec2::new((room.width() * 0.55).clamp(360.0_f32.min(room.width()), 700.0), room.height()));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(left).layout(egui::Layout::top_down(egui::Align::Min)));
            egui::ScrollArea::vertical().id_salt("start panel").auto_shrink([false, false]).show(&mut child, |ui| if self.pat.is_some() { self.pattern_panel(ui, app) } else { self.wall_panel(ui, app) });
            room.min.x = left.max.x + GAP;
        }
        if self.pat.is_some() {
            self.pat_pending(app);
        }
        let rect = Rect::from_min_size(room.min, room.size().max(Vec2::splat(16.0)));
        let resp = ui.allocate_rect(rect, Sense::click_and_drag());
        crate::drive::mark(ui.ctx(), "canvas", rect);
        let span = if self.tiled { 2.0 } else { 1.0 };
        let fit = ((rect.width().min(rect.height()) - 2.0 * GAP) / (S as f32 * span)).floor().max(1.0);
        let scale = self.zoom.unwrap_or(fit);
        let at = (rect.center() - Vec2::splat(S as f32 * scale / 2.0) + self.pan).round();
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 6.0, ui.visuals().extreme_bg_color);
        let face = Rect::from_min_size(at, Vec2::splat(S as f32 * scale));
        crate::drive::mark(ui.ctx(), "face", face);
        let v = self.v(app);
        match self.shown(app) {
            Some((img, own)) => {
                if self.tiled {
                    for j in -1..=1 {
                        for i in -1..=1 {
                            if (i, j) != (0, 0) {
                                draw(&p, img, v, at + Vec2::new(i as f32, j as f32) * S as f32 * scale, scale, true);
                            }
                        }
                    }
                }
                draw(&p, img, v, at, scale, !own);
            }
            None => {
                p.rect_filled(face, 0.0, dim(self.block_colour(app)));
                p.text(face.center(), egui::Align2::CENTER_CENTER, "unpainted: paint to start, or Start from: Pattern", egui::FontId::proportional(look::BODY), Color32::from_gray(230));
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
        p.rect_stroke(face, 0.0, egui::Stroke::new(1.0, Color32::from_gray(if self.tiled { 200 } else { 90 })), StrokeKind::Outside);
        let px_rect = |x: usize, y: usize| Rect::from_min_size(at + Vec2::new(x as f32, y as f32) * scale, Vec2::splat(scale));
        if self.seams {
            if let Some((img, _)) = self.shown(app) {
                for (x, y) in tools::seams(img, v, &app.pal) {
                    p.rect_stroke(px_rect(x, y).shrink(1.0), 0.0, egui::Stroke::new(2.0, look::BAD), StrokeKind::Inside);
                }
            }
        }
        if let Some(s) = self.sel {
            let r = Rect::from_min_size(at + Vec2::new(s.x as f32, s.y as f32) * scale, Vec2::new(s.w as f32, s.h as f32) * scale);
            p.rect_stroke(r, 0.0, egui::Stroke::new(3.0, Color32::BLACK), StrokeKind::Middle);
            p.rect_stroke(r, 0.0, egui::Stroke::new(1.5, look::ACCENT), StrokeKind::Middle);
        }
        if let Some(st) = self.stroke.as_ref().filter(|s| s.tool == Tool::Gradient) {
            let c = |q: (i32, i32)| at + Vec2::new(q.0 as f32 + 0.5, q.1 as f32 + 0.5) * scale;
            p.line_segment([c(st.start), c(st.last)], egui::Stroke::new(2.0, Color32::WHITE));
        }
        if resp.dragged_by(PointerButton::Middle) {
            self.pan += resp.drag_delta();
        }
        if resp.hovered() {
            let wheel: f32 = ui.input(|i| i.events.iter().map(|e| if let egui::Event::MouseWheel { delta, .. } = e { delta.y } else { 0.0 }).sum());
            if wheel != 0.0 {
                self.zoom = Some((scale + wheel.signum()).clamp(1.0, 64.0));
            }
        }
        let pos = ui.input(|i| i.pointer.hover_pos()).filter(|q| rect.contains(*q) || self.stroke.is_some());
        let cell = pos.map(|q| ((q - at) / scale).floor()).map(|c| (c.x as i32, c.y as i32));
        if let Some((f, _)) = self.pending.as_ref().filter(|(f, _)| *f == self.file(self.face)) {
            // nothing is painted under a pending change: it is shown, not yet the file's
            let g = p.layout_no_wrap(format!("Pending: {f} as it would be. Use or Cancel"), egui::FontId::proportional(look::BODY), Color32::WHITE);
            let at = rect.left_top() + Vec2::splat(GAP);
            p.rect_filled(Rect::from_min_size(at, g.size()).expand(4.0), 4.0, look::DRAFT.gamma_multiply(0.7));
            p.galley(at, g, Color32::WHITE);
        } else if !app.pal.colours.is_empty() {
            self.paint(ui, app, cell);
        }
        let on = cell.filter(|c| (self.tiled && (-16..32).contains(&c.0) && (-16..32).contains(&c.1)) || ((0..16).contains(&c.0) && (0..16).contains(&c.1)));
        app.pointer = match (on, self.shown(app)) {
            (Some((x, y)), Some((img, _))) => {
                let (ux, uy) = (x.rem_euclid(16) as usize, y.rem_euclid(16) as usize);
                let v = self.v(app);
                format!("{ux}, {uy}   {}", if img.empty_at(v, ux, uy) { "empty".into() } else { Self::colour_name(app, img.get(v, ux, uy)) })
            }
            (Some((x, y)), None) => format!("{x}, {y}"),
            _ => format!("{}  ×{}", self.file(self.face), scale),
        };
    }

    fn inspector(&mut self, ui: &mut Ui, app: &mut App) {
        if let Some(mut sec) = look::section(ui, "Kind") {
            let ui = &mut *sec;
            let (kind, mut pick) = (self.kind(app), None);
            ui.horizontal(|ui| {
                for (see, name, what) in KINDS {
                    let r = look::chip(ui, kind == see, name);
                    crate::drive::mark(ui.ctx(), format!("kind:{}", name.to_lowercase()), r.rect);
                    let help = if see == See::Cutout { format!("{what} Writes see = \"cutout\" to its row in data/blocks.toml; one undo step. The game and this view take a new kind after a rebuild (blocks.toml is built in)") } else { format!("{what} Drops see from its row in data/blocks.toml; the open face's empty pixels are filled with its mean colour; one undo step. The game and this view take a new kind after a rebuild (blocks.toml is built in)") };
                    if look::tip(r, &help, "").clicked() {
                        pick = Some(see);
                    }
                }
            });
            // the chosen kind's line; the other's is its chip's tip
            look::weak(ui, KINDS.iter().find(|k| k.0 == kind).map_or("", |k| k.2));
            if let Some(see) = pick {
                self.set_kind(app, see);
            }
        }
        let kind = self.kind(app);
        if let Some(mut sec) = look::section(ui, "Tools") {
            let ui = &mut *sec;
            // three columns of tools
            let w = ((ui.available_width() - 8.0) / 3.0).floor() - 1.0;
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(4.0);
                for (t, _, name, k, help) in TOOLS {
                    if t == Tool::Eraser && kind != See::Cutout {
                        // offered only to a cutout block: shown off, its tip says why
                        let r = ui.add_enabled_ui(false, |ui| look::tool(ui, false, name, k.name(), w)).inner;
                        r.on_disabled_hover_text(ERASER_OFF);
                        continue;
                    }
                    let r = look::tool(ui, self.tool == t, name, k.name(), w);
                    crate::drive::mark(ui.ctx(), format!("tool:{}", name.to_lowercase()), r.rect);
                    if look::tip(r, help, k.name()).clicked() {
                        self.tool = t;
                    }
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::splat(4.0);
                look::tip(look::toggle(ui, &mut self.mirror, "Mirror M"), "Paint both halves at once, mirrored across the middle", "M");
                if look::tip(look::toggle(ui, &mut self.tiled, "Tiled T"), "The neighbours ghosted round the face; strokes wrap over the edges", "T").changed() {
                    (self.zoom, self.pan) = (None, Vec2::ZERO);
                }
                look::tip(look::toggle(ui, &mut self.seams, "Seams"), "Mark edge pixels that clash with the far edge they meet when tiled", "Shift+T");
            });
            self.tool_options(ui, app);
        }
        if let Some(mut sec) = look::section(ui, "Start from") {
            let ui = &mut *sec;
            ui.horizontal_wrapped(|ui| {
                let r = ui.button("Pattern");
                crate::drive::mark(ui.ctx(), "btn:pattern", r.rect);
                if look::tip(r, "The pattern panel: pick one of the game's twelve patterns, its colours and values, see it live, then Use", "").clicked() {
                    self.open_pattern(app, false);
                }
                let r = ui.button("Generate variants");
                crate::drive::mark(ui.ctx(), "btn:variants", r.rect);
                if look::tip(r, "Sets of variants made from your variant 1, each shown as a patch of ground; a click shows one, Use this keeps it", "").clicked() {
                    self.open_wall(app);
                }
            });
            ui.horizontal(|ui| {
                let w = (ui.available_width() - 64.0).max(60.0);
                let r = ui.add(egui::TextEdit::singleline(&mut self.picture).hint_text("a PNG's path, or drop one").desired_width(w));
                look::tip(r, "Any PNG cut to 16 x 16 and snapped to the palette, as a rough to paint over", "");
                if look::tip(ui.button("Open"), "Start this variant from the picture", "").clicked() {
                    let p = self.picture.trim().trim_matches('"').to_string();
                    self.start_picture(app, &p);
                }
            });
            let painted: Vec<String> = app.reg.placeable().map(|d| d.name.clone()).filter(|n| *n != self.block && (0..4).any(|f| app.docs.contains_key(&art::file_name(n, f)))).collect();
            ui.horizontal(|ui| {
                let w = (ui.available_width() - 64.0).max(60.0);
                let r = egui::ComboBox::from_id_salt("copy from block").selected_text(if self.from_block.is_empty() { "another block" } else { &self.from_block }).width(w).show_ui(ui, |ui| {
                    if painted.is_empty() {
                        look::weak(ui, "no other block is painted yet");
                    }
                    for n in &painted {
                        ui.selectable_value(&mut self.from_block, n.clone(), n);
                    }
                });
                look::tip(r.response, "A painted block to start from", "");
                if look::tip(ui.add_enabled(!self.from_block.is_empty(), egui::Button::new("Copy")), "Start this face from that block's texture", "").clicked() {
                    let b = self.from_block.clone();
                    self.copy_block(app, &b);
                }
            });
            let ramps = app.pal.ramps();
            if ramps.len() > 1 {
                ui.horizontal(|ui| {
                    let w = ((ui.available_width() - 70.0) / 2.0).max(50.0);
                    for k in 0..2 {
                        let cur = if k == 0 { self.swap.0.clone() } else { self.swap.1.clone() };
                        let len = (k == 1).then(|| app.pal.ramp(&self.swap.0).len());
                        let mut pick = cur.clone();
                        let r = egui::ComboBox::from_id_salt(("swap", k)).selected_text(if cur.is_empty() { if k == 0 { "ramp" } else { "to ramp" } } else { &cur }).width(w).show_ui(ui, |ui| {
                            for (n, v) in &ramps {
                                if len.is_none_or(|l| l == v.len()) {
                                    ui.selectable_value(&mut pick, n.clone(), n);
                                }
                            }
                        });
                        look::tip(r.response, if k == 0 { "Swap ramp: the ramp the file uses now" } else { "The ramp of the same length to repaint it with" }, "");
                        *(if k == 0 { &mut self.swap.0 } else { &mut self.swap.1 }) = pick;
                    }
                    let ok = !self.swap.0.is_empty() && !self.swap.1.is_empty() && self.swap.0 != self.swap.1;
                    if look::tip(ui.add_enabled(ok, egui::Button::new("Swap")), "Repaint this file from one ramp to another of the same length", "").clicked() {
                        let (a, b) = self.swap.clone();
                        self.swap_ramp(app, &a, &b);
                    }
                });
            }
        }
        if let Some(mut sec) = look::section(ui, "Colours") {
            let ui = &mut *sec;
            self.palette(ui, app);
        }
        if self.preview == 0 {
            if let Some(mut sec) = look::section(ui, "Scene") {
                let ui = &mut *sec;
                look::weak(ui, "where the block above stands: the ground, the time of day, the season");
                app.view.controls(ui);
            }
        }
    }

    fn inspector_top(&mut self, ui: &mut Ui, app: &mut App) {
        self.preview_box(ui, app);
    }
}

pub fn colour_name(pal: &Palette, c: [u8; 3]) -> String {
    pal.find(c).map_or(format!("{} (not in the palette)", to_hex(c)), |i| pal.colours[i].name.clone())
}

/// The two colours and the palette's ramps as rows (the loose colours last): left click takes the first colour,
/// right click the second. Every page that paints with the palette shows this one.
/// The colour a left click took, if one did.
/// `key` names the page, for its own search and "+" (`grow.rs`).
pub fn palette_picker(ui: &mut Ui, key: &str, pal: &Palette, first: &mut [u8; 3], second: &mut [u8; 3]) -> Option<[u8; 3]> {
    ui.horizontal(|ui| {
        for (k, c) in [(0, *first), (1, *second)] {
            let (r, resp) = ui.allocate_exact_size(Vec2::new(36.0, 36.0), Sense::hover());
            ui.painter().rect_filled(r, 4.0, c32(c));
            ui.painter().rect_stroke(r, 4.0, egui::Stroke::new(1.0, Color32::from_gray(90)), StrokeKind::Inside);
            look::tip(resp, if k == 0 { "First colour: the left button paints it" } else { "Second colour: the right button paints it" }, "");
        }
        ui.vertical(|ui| {
            ui.label(RichText::new(colour_name(pal, *first)).small());
            ui.label(RichText::new(format!("right: {}", colour_name(pal, *second))).small().weak());
        });
    });
    ui.add_space(GAP * 0.5);
    swatches(ui, key, pal, first, second)
}

/// The palette's swatches alone, the ramps as rows, the loose colours last: left click takes the first colour
/// (and is returned), right click the second. `key` names the place for the search and the "+" (`grow.rs`).
pub fn swatches(ui: &mut Ui, key: &str, pal: &Palette, first: &mut [u8; 3], second: &mut [u8; 3]) -> Option<[u8; 3]> {
    let mut clicked = None;
    let q = super::grow::search(ui, key, pal);
    // the ramps as rows, the loose colours last; under a search only the colours it finds
    let mut rows: Vec<Vec<usize>> = pal.ramps().into_iter().map(|(_, v)| v).collect();
    let loose: Vec<usize> = (0..pal.colours.len()).filter(|&i| pal.colours[i].ramp.is_none()).collect();
    if !loose.is_empty() {
        rows.push(loose);
    }
    let rows: Vec<Vec<usize>> = rows.into_iter().map(|r| r.into_iter().filter(|&i| super::grow::shown(pal, i, &q)).collect::<Vec<_>>()).filter(|r| !r.is_empty()).collect();
    if rows.is_empty() {
        look::weak(ui, format!("No colour is called '{q}'"));
    }
    let size = 24.0;
    ui.spacing_mut().item_spacing = Vec2::splat(3.0);
    // two ramps to a line when they fit
    let per = ((ui.available_width() + 3.0) / (size + 3.0)).floor().max(1.0) as usize;
    let mut line: Vec<Option<usize>> = vec![];
    let mut lines = vec![];
    for row in rows {
        if !line.is_empty() && line.len() + 1 + row.len() > per {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(None);
        }
        line.extend(row.into_iter().map(Some));
    }
    lines.push(line);
    for line in lines {
        ui.horizontal(|ui| {
            for i in line {
                let (r, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
                let Some(i) = i else { continue };
                let c = &pal.colours[i];
                let p = ui.painter();
                p.rect_filled(r, 3.0, c32(c.rgb));
                if c.rgb == *second {
                    p.rect_stroke(r.shrink(5.0), 1.0, egui::Stroke::new(2.0, Color32::from_gray(20)), StrokeKind::Inside);
                    p.rect_stroke(r.shrink(6.0), 1.0, egui::Stroke::new(1.0, Color32::WHITE), StrokeKind::Inside);
                }
                if c.rgb == *first {
                    p.rect_stroke(r, 3.0, egui::Stroke::new(2.0, look::ACCENT), StrokeKind::Outside);
                }
                crate::drive::mark(ui.ctx(), format!("swatch:{}", c.name), r);
                let resp = look::tip(resp, &format!("{}: left click makes it the first colour, right click the second", c.name), "");
                if resp.clicked() {
                    *first = c.rgb;
                    clicked = Some(c.rgb);
                }
                if resp.secondary_clicked() {
                    *second = c.rgb;
                }
            }
        });
    }
    // the palette grows here: a new colour is taken as a click would
    super::grow::grow(ui, key, pal, first).or(clicked)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLOCKS: &str = "# a comment\r\n[[block]]\r\nid = 0\r\nname = \"air\"\r\nsolid = false\r\n\r\n[[block]]\r\nid = 1\r\nname = \"leaf\"\r\nhardness = 1\r\n# kept\r\n\r\n[block.autumn]\r\ncolour = [0.5, 0.2, 0.1]\r\n";

    #[test]
    fn the_kind_is_written_to_the_blocks_row_and_back() {
        let row = kind_row(BLOCKS, "leaf", See::Cutout).unwrap();
        let cut = crate::rows::put(BLOCKS, "block", "leaf", &row);
        assert!(cut.contains("name = \"leaf\"\r\nsee = \"cutout\"\r\nhardness = 1"), "after the name: {cut:?}");
        let reg = engine::blocks::Registry::parse(&cut).unwrap();
        assert_eq!(reg.get(reg.id("leaf")).unwrap().see, See::Cutout);
        assert!(cut.contains("[block.autumn]") && cut.starts_with("# a comment"), "the rest stays");
        let back = crate::rows::put(&cut, "block", "leaf", &kind_row(&cut, "leaf", See::Opaque).unwrap());
        assert_eq!(back, BLOCKS, "opaque drops the line, every other byte as it was");
        assert!(kind_row(BLOCKS, "sand", See::Cutout).is_none());
    }
}
