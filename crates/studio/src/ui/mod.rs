//! The Studio's frame (docs/design/studio-tools.md "The interface"): a bar of pages, the files column, the work
//! area, the inspector and the status line, with the command box, the key sheet, toasts, the problems list and
//! the watch on `data/`. A page plugs in through [`Page`]; the editing itself is `art`'s, the frame's logic
//! `shell`'s. Pages: `textures`, `palette`, `models`, `sounds`, `effects`; `viewport` is the game's renderer they share.
pub mod look;
mod fauna;
mod models;
mod effects;
mod palette;
mod sounds;
mod structures;
mod textures;
pub mod viewport;
mod wall;

use crate::art::{self, Doc, Img};
use crate::shell::{self, Filter, Flags, Outside, Settings, Watch, FILTERS};
use eframe::egui::{self, Align2, Color32, Id, Key, KeyboardShortcut, ModifierNames, Modifiers, Pos2, Rect, RichText, Sense, Ui, Vec2};
use engine::blocks::Registry;
use engine::palette::Palette;
use look::{GAP, ROW};
use std::collections::BTreeMap;

pub const TEX: &str = "data/textures";
pub const PAL: &str = "data/palette.toml";
pub const DRAFTS: &str = "data/drafts.txt";
pub const BLOCKS: &str = "data/blocks.toml";

/// One thing a page can do: a button, a key, a row in the command box and the key sheet, all from this.
#[derive(Clone)]
pub struct Cmd {
    /// What [`Page::run`] is called with.
    pub id: String,
    pub label: String,
    /// One line: what it does.
    pub help: String,
    pub key: Option<(Modifiers, Key)>,
}

impl Cmd {
    pub fn new(id: impl Into<String>, label: impl Into<String>, help: impl Into<String>) -> Cmd {
        Cmd { id: id.into(), label: label.into(), help: help.into(), key: None }
    }
    pub fn key(mut self, m: Modifiers, k: Key) -> Cmd {
        self.key = Some((m, k));
        self
    }
    pub fn keys(&self) -> String {
        self.key.map_or(String::new(), |(m, k)| KeyboardShortcut::new(m, k).format(&ModifierNames::NAMES, false).replace("Equals", "=").replace("Minus", "-"))
    }
}

/// The small picture of a row in the files column.
pub enum Thumb {
    Img(Img),
    Flat(Color32),
    Ramp(Vec<[u8; 3]>),
}

pub struct FileRow {
    /// What [`Page::open`] is called with.
    pub id: String,
    pub label: String,
    pub thumb: Thumb,
    pub flags: Flags,
}

/// A page of the Studio: it provides its files, its work area, its inspector and its commands; the frame does
/// the rest (search and filters, the command box, keys, the key sheet, history, status line, toasts).
pub trait Page {
    fn name(&self) -> &'static str;
    /// The rows of the files column, in order.
    fn files(&self, app: &App) -> Vec<FileRow>;
    /// The id of the row that is open.
    fn current(&self) -> Option<String>;
    /// "Try in game": the game's arguments only this page knows for its open file (`shell::try_args` has the rest).
    fn try_extra(&self) -> Vec<String> {
        vec![]
    }
    fn open(&mut self, app: &mut App, id: &str);
    /// Everything it can do; a command with a key runs from the keyboard while no text field has the focus.
    fn commands(&self, app: &App) -> Vec<Cmd>;
    fn run(&mut self, app: &mut App, id: &str);
    /// Keys that are not commands (the pointer's), for the key sheet: (keys, what they do).
    fn more_keys(&self) -> Vec<(&'static str, &'static str)> {
        vec![]
    }
    fn work(&mut self, ui: &mut Ui, app: &mut App);
    fn inspector(&mut self, ui: &mut Ui, app: &mut App);
    /// What stays on top of the inspector and does not scroll (the Textures page's block).
    fn inspector_top(&mut self, _ui: &mut Ui, _app: &mut App) {}
    /// The status line's left while the pointer is on no control: what the chosen tool does, and its keys.
    fn help(&self) -> String;
    /// The open file's undo steps by name, oldest first, and its redo steps, next first.
    fn history(&self, _app: &App) -> Option<(Vec<String>, Vec<String>)> {
        None
    }
    /// Undoes or redoes until `n` steps are left to undo.
    fn back_to(&mut self, _app: &mut App, _n: usize) {}
    /// After files changed on disk: keep the page's own state in range.
    fn refresh(&mut self, _app: &App) {}
    /// Whether the files column has a tree of the open file under its rows (the Models page's parts).
    fn has_tree(&self) -> bool {
        false
    }
    fn tree(&mut self, _ui: &mut Ui, _app: &mut App) {}
    /// The page's own files with unsaved work (the textures are the frame's), and saving them all.
    fn unsaved(&self) -> Vec<String> {
        vec![]
    }
    fn save_all(&mut self, _app: &mut App) {}
    /// For `shot=`: wait for what the page shows to be drawn (the game's renderer).
    fn settle(&mut self, _ctx: &egui::Context, _app: &mut App) {}
}

struct Toast {
    text: String,
    bad: bool,
    born: f64,
}

/// What every page shares: the game's data as the Studio holds it, and the frame's messages.
pub struct App {
    pub reg: Registry,
    pub pal: Palette,
    /// Every texture by file name: those on disk and the new ones.
    pub docs: BTreeMap<String, Doc>,
    pub drafts: Vec<String>,
    /// What the file check refuses, one line each.
    pub problems: Vec<String>,
    /// The status line's middle: the pointer's place and colour, set by the page each frame.
    pub pointer: String,
    toasts: Vec<Toast>,
    watch: Watch,
    /// Texture files changed on disk under unsaved work: the developer picks which to keep.
    asks: Vec<String>,
    now: f64,
    /// The game's renderer, shared by the pages (ADR 0049).
    pub view: viewport::Viewport,
    /// A shot: no sound card is opened.
    pub quiet: bool,
}

impl App {
    pub fn toast(&mut self, text: impl Into<String>) {
        self.toasts.push(Toast { text: text.into(), bad: false, born: self.now });
    }
    /// A red toast: it stays until clicked.
    pub fn refuse(&mut self, text: impl Into<String>) {
        self.toasts.push(Toast { text: text.into(), bad: true, born: self.now });
    }
    pub fn is_draft(&self, path: &str) -> bool {
        self.drafts.iter().any(|d| d == path)
    }
    /// The file check of `check=1`, after every save and every outside change.
    pub fn check(&mut self) {
        self.problems = match art::check_data(std::path::Path::new("data")) {
            Ok((_, _, bad)) => bad,
            Err(e) => vec![e],
        };
    }
    pub fn has_problem(&self, path: &str) -> bool {
        self.problems.iter().any(|p| shell::problem_file(p) == Some(path))
    }

    /// Saves texture files (through a temporary file; a new one never replaces a file on disk) and takes them
    /// off the drafts list.
    pub fn save_textures(&mut self, files: Vec<String>) {
        if files.is_empty() {
            return;
        }
        let _ = std::fs::create_dir_all(TEX);
        let mut drafts = std::fs::read_to_string(DRAFTS).unwrap_or_default();
        for f in files {
            let Some(d) = self.docs.get_mut(&f) else { continue };
            let path = format!("{TEX}/{f}");
            match art::save(std::path::Path::new(&path), &d.img, d.saved.is_none()) {
                Ok(()) => {
                    d.saved = Some(d.img.clone());
                    drafts = art::set_draft(&drafts, &path, false);
                    self.watch.note(std::path::Path::new(&path));
                    self.toast(format!("Saved {f}"));
                }
                Err(e) => self.refuse(format!("Not saved: {e}")),
            }
        }
        self.write_drafts(drafts);
        self.check();
    }
    pub fn write_drafts(&mut self, text: String) {
        if art::drafts(&text) != self.drafts {
            match std::fs::write(DRAFTS, &text) {
                Ok(()) => self.watch.note(std::path::Path::new(DRAFTS)),
                Err(e) => self.refuse(format!("{DRAFTS}: {e}")),
            }
            self.drafts = art::drafts(&text);
        }
    }
    /// Writes the palette; it leaves the drafts list only when a colour itself was changed (a new order, name,
    /// added or removed colour keeps Claude's colours a draft).
    pub fn save_palette(&mut self, colour_changed: bool) {
        match std::fs::write(PAL, self.pal.write()) {
            Ok(()) => {
                self.watch.note(std::path::Path::new(PAL));
                if colour_changed {
                    let text = art::set_draft(&std::fs::read_to_string(DRAFTS).unwrap_or_default(), PAL, false);
                    self.write_drafts(text);
                }
            }
            Err(e) => self.refuse(format!("{PAL} not saved: {e}")),
        }
        self.check();
    }
    pub fn note(&mut self, path: &str) {
        self.watch.note(std::path::Path::new(path));
    }

    /// The watch: whether anything changed on disk.
    fn poll(&mut self) -> bool {
        let changed = self.watch.poll();
        for p in &changed {
            let s = p.to_string_lossy().replace('\\', "/");
            if s == PAL {
                match std::fs::read_to_string(PAL).map_err(|e| e.to_string()).and_then(|t| Palette::parse(&t)) {
                    Ok(p) if p != self.pal => {
                        self.pal = p;
                        self.toast("Reloaded the palette: changed on disk");
                    }
                    _ => {}
                }
            } else if s == DRAFTS {
                self.drafts = art::drafts(&std::fs::read_to_string(DRAFTS).unwrap_or_default());
            } else if s == BLOCKS {
                if let Ok(r) = std::fs::read_to_string(BLOCKS).map_err(|e| e.to_string()).and_then(|t| Registry::parse(&t)) {
                    self.reg = r;
                }
            } else if let Some(name) = s.strip_prefix(&format!("{TEX}/")).filter(|n| n.ends_with(".png")) {
                self.outside_texture(name);
            }
        }
        if !changed.is_empty() {
            self.check();
        }
        !changed.is_empty()
    }
    fn disk(name: &str) -> Option<Img> {
        std::fs::read(format!("{TEX}/{name}")).ok().and_then(|b| art::decode(&b).ok())
    }
    fn outside_texture(&mut self, name: &str) {
        let disk = App::disk(name);
        let (dirty, saved) = self.docs.get(name).map_or((false, None), |d| (d.dirty(), d.saved.clone()));
        match shell::outside(dirty, disk.as_ref(), saved.as_ref()) {
            Outside::Nothing => {}
            Outside::Reload => {
                self.take_disk(name, disk);
                self.toast(format!("Reloaded {name}: changed on disk"));
            }
            Outside::Ask if !self.asks.iter().any(|a| a == name) => self.asks.push(name.into()),
            Outside::Ask => {}
        }
    }
    /// The disk's file wins; what was in the Studio stays one undo step back.
    fn take_disk(&mut self, name: &str, disk: Option<Img>) {
        match (self.docs.get_mut(name), disk) {
            (Some(d), Some(img)) => {
                if d.img != img {
                    d.hist.push(std::mem::replace(&mut d.img, img.clone()), "Changed on disk");
                }
                d.saved = Some(img);
            }
            (Some(_), None) => {
                self.docs.remove(name);
            }
            (None, Some(img)) => {
                self.docs.insert(name.into(), Doc::loaded(img));
            }
            (None, None) => {}
        }
    }
    /// The Studio's work wins: a save then writes over the disk's file (or, if that one is refused, not).
    fn keep_mine(&mut self, name: &str) {
        let disk = App::disk(name);
        if let Some(d) = self.docs.get_mut(name) {
            d.saved = disk;
        }
    }
}

#[derive(PartialEq, Clone, Copy)]
enum Overlay {
    None,
    Commands,
    Keys,
    Problems,
}

pub struct Studio {
    pub app: App,
    pages: Vec<Box<dyn Page>>,
    page: usize,
    pub settings: Settings,
    written: Settings,
    search: String,
    filter: Filter,
    overlay: Overlay,
    query: String,
    picked: usize,
    /// The status line's left, from the control the pointer stood on last frame.
    help: Option<String>,
    styled: bool,
    /// The open file has unsaved work (from the files column, for the inspector's Save).
    open_dirty: bool,
    last_poll: f64,
    closing: bool,
    quit: bool,
    /// Opens the history list next frame (its command).
    history_open: Option<bool>,
    /// The screen's pixels a point, for the scale steps that keep the font sharp; and whether the scale is
    /// still to be put on one (the window's first frame; a shot keeps the scale it is given).
    native: f32,
    snap_scale: bool,
    /// The game "Try in game" started, while the Studio can still talk to it.
    game: Option<std::process::Child>,
    /// Off for a shot: it never writes the developer's studio.txt.
    write_settings: bool,
    /// Unsaved textures kept every few seconds, and what a crash left, still to be offered (`recover.rs`).
    recovery: crate::recover::Recovery,
    recovered: Vec<(String, Img)>,
}

impl Studio {
    pub fn keep_settings(&mut self) {
        self.write_settings = false;
        self.app.quiet = true;
    }

    /// Reads `data/` in the working folder: the blocks, the palette, every texture and the drafts list.
    pub fn load(open: Option<&str>) -> Result<Studio, String> {
        let reg = Registry::parse(&std::fs::read_to_string(BLOCKS).map_err(|e| format!("{BLOCKS}: {e}"))?).map_err(|e| format!("{BLOCKS}: {e}"))?;
        let pal = Palette::parse(&std::fs::read_to_string(PAL).map_err(|e| format!("{PAL}: {e}"))?).map_err(|e| format!("{PAL}: {e}"))?;
        let mut docs = BTreeMap::new();
        let mut refused = vec![];
        for n in art::pngs(std::path::Path::new(TEX)) {
            match std::fs::read(format!("{TEX}/{n}")).map_err(|e| e.to_string()).and_then(|b| art::decode(&b)) {
                Ok(img) => {
                    docs.insert(n, Doc::loaded(img));
                }
                Err(e) => refused.push(format!("{n}: {e}")),
            }
        }
        if let Some(b) = open {
            reg.placeable().find(|d| d.name == b).ok_or(format!("no block '{b}' in {BLOCKS}"))?;
        }
        let settings = Settings::parse(&std::fs::read_to_string(shell::SETTINGS).unwrap_or_default());
        let mut app = App {
            reg,
            pal,
            docs,
            drafts: art::drafts(&std::fs::read_to_string(DRAFTS).unwrap_or_default()),
            problems: vec![],
            pointer: String::new(),
            toasts: vec![],
            watch: Watch::new(&[PAL, DRAFTS, BLOCKS, sounds::SOUNDS, effects::EFFECTS, fauna::FILE], &[TEX, structures::DIR, models::MODELS]),
            asks: vec![],
            now: 0.0,
            view: viewport::Viewport::new(),
            quiet: false,
        };
        for r in refused {
            app.refuse(format!("Refused on load: {r}"));
        }
        app.check();
        let mut pages: Vec<Box<dyn Page>> = vec![Box::new(textures::Textures::new(&app)), Box::new(palette::PalettePage::new(&app)), Box::new(models::Models::new(&app)), Box::new(sounds::Sounds::new(&app)), Box::new(effects::Effects::new(&app)), Box::new(structures::Structures::new(&app))];
        pages.push(Box::new(fauna::Fauna::new(&app)));
        if let Some(b) = open {
            pages[0].open(&mut app, b);
        }
        // what a crash left, but for files that are on disk as kept
        let recovery = crate::recover::Recovery::new(".", "textures", crate::recover::EVERY);
        let recovered = recovery.left().into_iter().filter_map(|(n, b)| Some((n, art::decode(&b).ok()?))).filter(|(n, img)| app.docs.get(n).is_none_or(|d| d.img != *img)).collect();
        Ok(Studio {
            app,
            pages,
            page: 0,
            written: settings.clone(),
            settings,
            search: String::new(),
            filter: Filter::All,
            overlay: Overlay::None,
            query: String::new(),
            picked: 0,
            help: None,
            styled: false,
            open_dirty: false,
            last_poll: 0.0,
            closing: false,
            quit: false,
            write_settings: true,
            history_open: None,
            native: 1.0,
            snap_scale: false,
            game: None,
            recovery,
            recovered,
        })
    }

    /// Every entry of the command box: (id, label, where it comes from, keys).
    fn entries(&self) -> Vec<(String, String, String, String)> {
        let mut v = vec![];
        for c in self.pages[self.page].commands(&self.app) {
            v.push((c.id.clone(), c.label.clone(), c.help.clone(), c.keys()));
        }
        for c in Self::frame_commands() {
            v.push((c.id.clone(), c.label.clone(), c.help.clone(), c.keys()));
        }
        for p in &self.pages {
            for f in p.files(&self.app) {
                v.push((format!("open:{}:{}", p.name(), f.id), format!("Open {}", f.label), p.name().into(), String::new()));
            }
        }
        v
    }
    fn frame_commands() -> Vec<Cmd> {
        vec![
            Cmd::new("frame:commands", "Command box", "find any file, page, tool or action and run it").key(Modifiers::COMMAND, Key::P),
            Cmd::new("frame:keys", "Key sheet", "every key of this page").key(Modifiers::NONE, Key::F1),
            Cmd::new("frame:problems", "Problems", "what the file check refuses; a click opens the file"),
            Cmd::new("frame:history", "History", "the open file's undo steps by name; a click goes back to one"),
            Cmd::new("frame:larger", "Interface larger", "the interface scale up to the next size the font's pixels are sharp at").key(Modifiers::COMMAND, Key::Equals),
            Cmd::new("frame:smaller", "Interface smaller", "the interface scale down a step").key(Modifiers::COMMAND, Key::Minus),
            Cmd::new("frame:scale100", "Interface as new", "the interface scale nearest 100 % that the font's pixels are sharp at").key(Modifiers::COMMAND, Key::Num0),
            Cmd::new("frame:try", "Try in game", "save the open file and start the game on it: a creative world that keeps no save, the thing standing ahead; again while that game runs stands the open structure or creature ahead of you there").key(Modifiers::NONE, Key::F5),
            Cmd::new("frame:files", "Files column", "hide or show the files column on this page").key(Modifiers::NONE, Key::F2),
            Cmd::new("frame:inspector", "Inspector", "hide or show the inspector on this page").key(Modifiers::NONE, Key::F3),
            Cmd::new("frame:focus", "Work area alone", "hide both side panels, or bring them back").key(Modifiers::NONE, Key::F4),
            Cmd::new("frame:swap", "Swap the sides", "the inspector on the left and the files on the right, or back"),
            Cmd::new("frame:reset", "Layout as new", "on every page: both panels shown, every section open and in its own order, every divider where it started"),
        ]
        .into_iter()
        .chain(look::PAGES.iter().zip([Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7]).map(|((name, _), k)| Cmd::new(format!("page:{name}"), format!("{name} page"), format!("go to the {name} page")).key(Modifiers::COMMAND, k)))
        .collect()
    }

    /// The Studio as it was left: each page's file open again when it is still there, and the page shown.
    /// The window calls it once at its start; a shot does not (it shows what its arguments say).
    pub fn resume(&mut self) {
        self.snap_scale = true;
        for i in 0..self.pages.len() {
            let Some(id) = self.settings.open.get(self.pages[i].name()).cloned() else { continue };
            if self.pages[i].current().as_deref() != Some(&id) && self.pages[i].files(&self.app).iter().any(|f| f.id == id) {
                self.pages[i].open(&mut self.app, &id);
            }
        }
        if let Some(i) = self.pages.iter().position(|p| p.name() == self.settings.page) {
            self.page = i;
        }
    }

    /// Starts the game beside the Studio on the open file, saved first (the game reads what is on disk, and
    /// from then on shows each save within a second). The Studio does not wait for it.
    fn try_in_game(&mut self) {
        if self.open_dirty {
            let i = self.page;
            self.pages[i].run(&mut self.app, "save");
        }
        let page = &self.pages[self.page];
        let (line, args) = (shell::game_line(page.name(), page.current().as_deref()), shell::try_args(page.name(), page.current().as_deref(), &page.try_extra()));
        // the game it started is still up: say it there, in place of a second game
        if let Some(g) = self.game.as_mut().and_then(|g| matches!(g.try_wait(), Ok(None)).then_some(g)) {
            use std::io::Write;
            match (line, g.stdin.as_mut()) {
                (Some(l), Some(i)) => match writeln!(i, "{l}").and_then(|_| i.flush()) {
                    Ok(()) => self.app.toast(format!("Sent to the game: {l}")),
                    Err(e) => self.app.refuse(format!("Try in game: the running game took no line ({e})")),
                },
                _ => self.app.toast("The game is running: it shows a saved file within a second"),
            }
            return;
        }
        match viewport::Viewport::exe().ok_or("no game exe found".to_string()).and_then(|exe| std::process::Command::new(exe).args(&args).stdin(std::process::Stdio::piped()).spawn().map_err(|e| e.to_string())) {
            Ok(g) => {
                self.game = Some(g);
                self.app.toast(format!("Started the game: {}", args[4..args.len() - 1].join(" ")));
            }
            Err(e) => self.app.refuse(format!("Try in game: {e}")),
        }
    }

    /// A side panel's name in `studio.txt`: this page's own ("Models/inspector").
    fn panel(&self, what: &str) -> String {
        format!("{}/{what}", self.pages[self.page].name())
    }

    /// Runs a command by id: the frame's, a page's (`page:<name>`), a file's (`open:<page>:<id>`) or the open
    /// page's own. The command box, the keys and `shot= cmd=` all come here.
    pub fn run_command(&mut self, id: &str) {
        match id {
            "frame:commands" => {
                self.overlay = Overlay::Commands;
                (self.query, self.picked) = (String::new(), 0);
            }
            "frame:keys" => self.overlay = Overlay::Keys,
            "frame:problems" => self.overlay = Overlay::Problems,
            "frame:history" => self.history_open = Some(true),
            "frame:larger" => self.settings.scale = shell::sharp_scale(self.settings.scale, self.native, 1),
            "frame:smaller" => self.settings.scale = shell::sharp_scale(self.settings.scale, self.native, -1),
            "frame:scale100" => self.settings.scale = shell::sharp_scale(1.0, self.native, 0),
            "frame:files" | "frame:inspector" => {
                let key = self.panel(&id[6..]);
                if !self.settings.hidden.remove(&key) {
                    self.settings.hidden.insert(key);
                }
            }
            "frame:focus" => {
                let keys = [self.panel("files"), self.panel("inspector")];
                let hide = !keys.iter().all(|k| self.settings.hidden.contains(k));
                for k in keys {
                    if hide {
                        self.settings.hidden.insert(k);
                    } else {
                        self.settings.hidden.remove(&k);
                    }
                }
            }
            "frame:try" => self.try_in_game(),
            "frame:swap" => self.settings.swap ^= true,
            "frame:reset" => self.settings = Settings { files_w: self.settings.files_w, inspector_w: self.settings.inspector_w, scale: self.settings.scale, page: self.settings.page.clone(), open: self.settings.open.clone(), ..Settings::default() },
            _ => {
                if let Some(h) = id.strip_prefix("hour:").and_then(|h| h.parse::<f32>().ok()) {
                    // the game view's Time slider (for `shot= cmd=hour:18`)
                    self.app.view.hour = h.rem_euclid(24.0);
                } else if let Some(k) = id.strip_prefix("season:").and_then(|q| engine::blocks::SEASONS.iter().position(|n| *n == q)) {
                    // the game view's Year slider at that season's start, and the Textures page on its tab
                    // (for `shot= season=autumn`)
                    self.app.view.day = viewport::season_day(k);
                    for p in self.pages.iter_mut().filter(|p| p.name() == "Textures") {
                        p.run(&mut self.app, id);
                    }
                } else if let Some(name) = id.strip_prefix("page:") {
                    if let Some(i) = self.pages.iter().position(|p| p.name().eq_ignore_ascii_case(name)) {
                        self.page = i;
                    }
                } else if let Some((page, file)) = id.strip_prefix("open:").and_then(|r| r.split_once(':')) {
                    if let Some(i) = self.pages.iter().position(|p| p.name() == page) {
                        self.page = i;
                        self.pages[i].open(&mut self.app, file);
                    }
                } else {
                    self.pages[self.page].run(&mut self.app, id);
                }
            }
        }
    }
    /// For `shot=`: the open page's game view drawn and settled.
    pub fn settle(&mut self, ctx: &egui::Context) {
        self.pages[self.page].settle(ctx, &mut self.app);
    }
    /// The renderer's numbers, for the shot's JSON line.
    pub fn view_report(&self) -> String {
        self.app.view.report()
    }

    /// For `shot= input=`: what a script can check without reading the picture.
    pub fn drive_report(&self) -> String {
        let p = &self.pages[self.page];
        let mut unsaved: Vec<String> = self.app.docs.iter().filter(|(_, d)| d.dirty()).map(|(n, _)| n.clone()).collect();
        unsaved.extend(self.pages.iter().flat_map(|p| p.unsaved()));
        let toasts: Vec<String> = self.app.toasts.iter().map(|t| t.text.clone()).collect();
        let (undo, redo) = p.history(&self.app).unwrap_or_default();
        let overlay = match self.overlay {
            Overlay::None => "none",
            Overlay::Commands => "commands",
            Overlay::Keys => "keys",
            Overlay::Problems => "problems",
        };
        let js = |v: &[String]| format!("[{}]", v.iter().map(|s| crate::js(s)).collect::<Vec<_>>().join(","));
        format!(
            "{{\"page\":{},\"open\":{},\"unsaved\":{},\"toasts\":{},\"undo\":{},\"redo\":{},\"overlay\":\"{overlay}\",\"closing\":{},\"search\":{},\"query\":{},\"scale\":{}}}",
            crate::js(p.name()),
            p.current().map_or("null".into(), |c| crate::js(&c)),
            js(&unsaved),
            js(&toasts),
            js(&undo),
            js(&redo),
            self.closing,
            crate::js(&self.search),
            crate::js(&self.query),
            self.settings.scale
        )
    }

    /// Opens the command box with some text typed (for `shot=`).
    pub fn type_query(&mut self, q: &str) {
        self.run_command("frame:commands");
        self.query = q.into();
    }

    /// A click on a problem: open the file it names.
    fn open_problem(&mut self, line: &str) {
        let Some(file) = shell::problem_file(line) else { return self.app.toast("That problem names no file") };
        if file == PAL {
            self.run_command("page:Palette");
        } else if let Some(m) = file.strip_prefix("data/models/").and_then(|f| f.strip_suffix(".toml")) {
            self.run_command(&format!("open:Models:{m}"));
        } else if let Some(block) = art::block_of(file, self.app.reg.placeable().map(|d| d.name.as_str())) {
            self.run_command(&format!("open:Textures:{block}"));
        } else {
            self.app.toast(format!("{file} names no block: rename or remove it in the folder"));
        }
        self.overlay = Overlay::None;
    }

    pub fn show(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        if !self.styled {
            look::fonts(&ctx);
            look::apply(&ctx);
            self.styled = true;
        }
        self.native = ctx.native_pixels_per_point().unwrap_or(1.0);
        if std::mem::take(&mut self.snap_scale) {
            self.settings.scale = shell::sharp_scale(self.settings.scale, self.native, 0);
        }
        if (ctx.zoom_factor() - self.settings.scale).abs() > 1e-3 {
            ctx.set_zoom_factor(self.settings.scale);
        }
        crate::drive::clear(&ctx);
        self.app.now = ctx.input(|i| i.time);
        // the watch: a cheap look at the modified times about once a second
        if self.app.now - self.last_poll >= 1.0 {
            self.last_poll = self.app.now;
            if self.app.poll() {
                for p in &mut self.pages {
                    p.refresh(&self.app);
                }
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(1000));
        self.keys(&ctx);
        look::set_layout(&ctx, look::Layout { page: self.pages[self.page].name().into(), folded: self.settings.folded.clone(), splits: self.settings.splits.clone(), order: self.settings.order.clone() });

        egui::Panel::top("pages").show(ui, |ui| self.top_bar(ui));
        egui::Panel::bottom("status").exact_size(30.0).show(ui, |ui| self.status_line(ui));
        // at a large scale on a small window the side panels give way to the work
        let wide = ctx.content_rect().width();
        // (the centre keeps at least half the window; what does not fit a column scrolls, never runs under another)
        // a wide window lets a panel be dragged as wide as leaves the work 520 points
        let room = wide - 520.0;
        let swap = self.settings.swap;
        // each page has its own two panels: shown or not, and as wide as it was left
        let (fk, ik) = (self.panel("files"), self.panel("inspector"));
        let (fw, iw) = (self.settings.widths.get(&fk).copied().unwrap_or(self.settings.files_w), self.settings.widths.get(&ik).copied().unwrap_or(self.settings.inspector_w));
        let (hide_f, hide_i) = (self.settings.hidden.contains(&fk), self.settings.hidden.contains(&ik));
        let side = |left: bool, id: &str| if left { egui::Panel::left(egui::Id::unique(id)) } else { egui::Panel::right(egui::Id::unique(id)) };
        let files = (!hide_f).then(|| side(!swap, &fk).resizable(true).default_size(fw).size_range(Settings::FILES.0.min(wide * 0.18)..=Settings::FILES.1.min((wide * 0.2).max(room * 0.4))).show(ui, |ui| self.files(ui)));
        let insp_max = (wide * 0.3).max(room - if hide_f { 0.0 } else { fw });
        let insp = (!hide_i).then(|| side(swap, &ik).resizable(true).default_size(iw).size_range(Settings::INSPECTOR.0.min(wide * 0.26)..=Settings::INSPECTOR.1.min(insp_max)).show(ui, |ui| self.inspector(ui)));
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::both().id_salt("work").auto_shrink([false, false]).show(ui, |ui| self.pages[self.page].work(ui, &mut self.app));
        });
        // only a drag of an edge sets the widths kept, not a panel squeezed by a large scale
        if ctx.input(|i| i.pointer.any_down()) {
            // (a width is kept once it differs from what the page would have anyway)
            for (key, was, panel) in [(fk, fw, files), (ik, iw, insp)] {
                if let Some(w) = panel.map(|p| p.response.rect.width().round()).filter(|w| (w - was).abs() >= 1.0) {
                    self.settings.widths.insert(key, w);
                }
            }
        }
        // what was folded or dragged this frame
        let l = look::layout(&ctx);
        if l.folded != self.settings.folded || l.splits != self.settings.splits || l.order != self.settings.order {
            (self.settings.folded, self.settings.splits, self.settings.order) = (l.folded, l.splits, l.order);
        }

        self.overlays(&ctx);
        self.toasts(&ctx);
        self.asks(&ctx);
        self.close_check(&ctx);
        self.recover(&ctx);
        self.help = look::take_help(&ctx);
        // where the developer is, for the next start
        let (name, cur) = (self.pages[self.page].name(), self.pages[self.page].current());
        if self.settings.page != name {
            self.settings.page = name.into();
        }
        if let Some(c) = cur.filter(|c| self.settings.open.get(name) != Some(c)) {
            self.settings.open.insert(name.into(), c);
        }
        // studio.txt, once a drag of a panel's edge is let go
        if self.write_settings && self.settings != self.written && !ctx.input(|i| i.pointer.any_down()) {
            if std::fs::write(shell::SETTINGS, self.settings.write()).is_ok() {
                self.written = self.settings.clone();
            }
        }
    }

    fn keys(&mut self, ctx: &egui::Context) {
        let typing = ctx.egui_wants_keyboard_input();
        let mut run = vec![];
        let mut cmds = Self::frame_commands();
        if !typing && self.overlay == Overlay::None && self.app.asks.is_empty() && !self.closing {
            cmds.extend(self.pages[self.page].commands(&self.app));
        }
        ctx.input_mut(|i| {
            for c in &cmds {
                let Some((m, k)) = c.key else { continue };
                // the frame's own keys work while typing in a box too, a page's not
                if (!typing || c.id.starts_with("frame:")) && i.consume_shortcut(&KeyboardShortcut::new(m, k)) {
                    run.push(c.id.clone());
                }
            }
        });
        for id in run {
            if id == "frame:commands" && self.overlay == Overlay::Commands {
                self.overlay = Overlay::None;
            } else {
                self.run_command(&id);
            }
        }
    }

    fn top_bar(&mut self, ui: &mut Ui) {
        // the buttons on the right first, then the title and the page tabs in what is left, wrapping to a second
        // row in a narrow window
        let narrow = ui.available_width() < 1100.0;
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let pct = format!("{:.0} %", self.settings.scale * 100.0);
                let r = ui.menu_button(pct, |ui| {
                    // (the sizes the font's pixels are sharp at, smallest first)
                    let mut sizes = vec![shell::sharp_scale(Settings::SCALE.0, self.native, 0)];
                    while sizes.last().is_some_and(|s| shell::sharp_scale(*s, self.native, 1) > s + 1e-3) {
                        sizes.push(shell::sharp_scale(sizes[sizes.len() - 1], self.native, 1));
                    }
                    for s in sizes {
                        if ui.selectable_label((self.settings.scale - s).abs() < 0.01, format!("{:.0} %", s * 100.0)).clicked() {
                            self.settings.scale = s;
                            ui.close();
                        }
                    }
                });
                crate::drive::mark(ui.ctx(), "scale", r.response.rect);
                look::tip(r.response, "Interface scale, kept in studio.txt", "Ctrl+= and Ctrl+-");
                let r = ui.button("Keys");
                crate::drive::mark(ui.ctx(), "keys", r.rect);
                if look::tip(r, "Every key of this page", "F1").clicked() {
                    self.overlay = Overlay::Keys;
                }
                let r = ui.button("Try in game");
                crate::drive::mark(ui.ctx(), "try", r.rect);
                if look::tip(r, "Save the open file and start the game on it: a creative world that keeps no save, the thing standing ahead. Later saves show there within a second", "F5").clicked() {
                    self.run_command("frame:try");
                }
                // the layout: what is shown and where
                let r = ui.menu_button("View", |ui| {
                    let s = &self.settings;
                    let mut run = None;
                    look::weak(ui, format!("on the {} page", self.pages[self.page].name()));
                    for (id, on, label) in [("frame:files", !s.hidden.contains(&self.panel("files")), "Files column   F2"), ("frame:inspector", !s.hidden.contains(&self.panel("inspector")), "Inspector   F3")] {
                        if ui.selectable_label(on, label).clicked() {
                            run = Some(id);
                        }
                    }
                    if ui.button("Work area alone   F4").clicked() {
                        run = Some("frame:focus");
                    }
                    ui.separator();
                    look::weak(ui, "everywhere");
                    for (id, on, label) in [("frame:swap", s.swap, "Inspector on the left")] {
                        if ui.selectable_label(on, label).clicked() {
                            run = Some(id);
                        }
                    }
                    ui.separator();
                    if ui.button("Layout as new").clicked() {
                        run = Some("frame:reset");
                    }
                    if let Some(id) = run {
                        self.run_command(id);
                        ui.close();
                    }
                });
                crate::drive::mark(ui.ctx(), "view", r.response.rect);
                look::tip(r.response, "Hide or show the side panels, swap their sides; sections fold by a click on their title, dividers drag", "F2, F3, F4");
                let r = ui.button("Find…");
                crate::drive::mark(ui.ctx(), "find", r.rect);
                if look::tip(r, "Find any file, page, tool or action and run it", "Ctrl+P").clicked() {
                    self.run_command("frame:commands");
                }
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center).with_main_wrap(true), |ui| {
                    ui.add_space(GAP * 0.5);
                    if !narrow {
                        ui.label(RichText::new("TechnoApe Studio").strong().color(Color32::from_gray(170)));
                        ui.add_space(GAP * 2.0);
                    }
                    for i in 0..self.pages.len() {
                        let name = self.pages[i].name();
                        // a tab wears its page's colour: a line under it, and its fill when open
                        let (on, c) = (self.page == i, look::page_colour(name));
                        let text = RichText::new(name).strong();
                        let r = ui.add(egui::Button::new(if on { text.color(Color32::WHITE) } else { text }).fill(if on { c.gamma_multiply(0.35) } else { Color32::TRANSPARENT }).min_size(Vec2::new(if narrow { 0.0 } else { 90.0 }, 26.0)));
                        let line = Rect::from_min_max(Pos2::new(r.rect.min.x + 4.0, r.rect.max.y - 2.0), Pos2::new(r.rect.max.x - 4.0, r.rect.max.y));
                        ui.painter().rect_filled(line, 1.0, if on { c } else { c.gamma_multiply(0.45) });
                        crate::drive::mark(ui.ctx(), format!("tab:{name}"), r.rect);
                        if look::tip(r, &format!("The {name} page"), &format!("Ctrl+{}", i + 1)).clicked() {
                            self.page = i;
                        }
                    }
                });
            });
        });
    }

    fn status_line(&mut self, ui: &mut Ui) {
        let page = &self.pages[self.page];
        let help = self.help.clone().unwrap_or_else(|| page.help());
        let unsaved = self.app.docs.values().filter(|d| d.dirty()).count() + self.pages.iter().map(|p| p.unsaved().len()).sum::<usize>();
        ui.horizontal_centered(|ui| {
            let full = ui.available_width();
            ui.allocate_ui(Vec2::new(full * 0.5, 20.0), |ui| {
                ui.add(egui::Label::new(RichText::new(help).small()).truncate());
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let n = self.app.problems.len();
                let badge = if n == 0 { RichText::new("no problems").small().color(look::GOOD) } else { RichText::new(format!("{n} problem{}", if n == 1 { "" } else { "s" })).small().color(look::BAD) };
                let r = ui.add(egui::Button::new(badge).frame(n > 0));
                if look::tip(r, "The file check (check=1) after every save and outside change; a click lists what it refuses", "").clicked() {
                    self.overlay = if self.overlay == Overlay::Problems { Overlay::None } else { Overlay::Problems };
                }
                ui.separator();
                let saved = if unsaved == 0 { RichText::new("all saved").small().weak() } else { RichText::new(format!("{unsaved} unsaved")).small().color(look::DRAFT) };
                look::tip(ui.label(saved), "Files with work not saved yet; Ctrl+S saves the open block", "Ctrl+S");
                ui.separator();
                ui.add(egui::Label::new(RichText::new(&self.app.pointer).small().monospace()).truncate());
            });
        });
    }

    fn files(&mut self, ui: &mut Ui) {
        ui.add_space(GAP);
        let r = ui.add(egui::TextEdit::singleline(&mut self.search).hint_text("Search").desired_width(f32::INFINITY));
        crate::drive::mark(ui.ctx(), "search", r.rect);
        look::tip(r, "Search the files of this page by name", "");
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
            for (f, name, help) in FILTERS {
                let r = look::chip(ui, self.filter == f, name);
                if look::tip(r, help, "").clicked() {
                    self.filter = f;
                }
            }
        });
        ui.add_space(GAP * 0.5);
        let page = &self.pages[self.page];
        let rows = page.files(&self.app);
        let drafts = rows.iter().filter(|r| r.flags.draft).count();
        let cur = page.current();
        self.open_dirty = rows.iter().any(|r| r.flags.dirty && cur.as_deref() == Some(r.id.as_str()));
        let q = self.search.to_lowercase();
        let shown: Vec<&FileRow> = rows.iter().filter(|r| r.label.to_lowercase().contains(&q) && self.filter.keeps(r.flags)).collect();
        let mut open = None;
        egui::Panel::bottom("files foot").show_separator_line(true).show(ui, |ui| {
            ui.add_space(GAP * 0.5);
            let text = match drafts {
                0 => RichText::new("no drafts left").small().weak(),
                n => RichText::new(format!("{n} still Claude's draft{}", if n == 1 { "" } else { "s" })).small().color(look::DRAFT),
            };
            look::tip(ui.label(text), "Drafts are Claude's; saving your own over one takes it off data/drafts.txt", "");
        });
        if self.pages[self.page].has_tree() {
            let (pages, app, i) = (&mut self.pages, &mut self.app, self.page);
            let h = ui.available_height();
            let r = egui::Panel::bottom("page tree").resizable(true).default_size(240.0f32.min(h * 0.45)).size_range(60.0f32.min(h * 0.2)..=(h * 0.7).max(60.0)).show(ui, |ui| {
                // (never taller than the panel: a scroll area's least height would draw over the files)
                egui::ScrollArea::both().id_salt("tree").min_scrolled_height(0.0).auto_shrink([false, false]).show(ui, |ui| {
                    ui.add_space(GAP * 0.5);
                    pages[i].tree(ui, app);
                });
            });
            crate::drive::mark(ui.ctx(), "tree", r.response.rect);
        }
        let list = egui::ScrollArea::vertical().min_scrolled_height(0.0).auto_shrink([false, false]).show(ui, |ui| {
            if shown.is_empty() {
                look::weak(ui, "nothing matches");
            }
            let mut heading = None;
            for row in shown {
                let (group, name, tag) = look::split_label(&row.label);
                // rows of one family ("dirt: dig") stand under its name; what has none comes under "other" after them
                if group != heading && (group.is_some() || heading.is_some()) {
                    ui.add_space(GAP * 0.5);
                    look::weak(ui, group.unwrap_or("other").to_uppercase());
                }
                heading = group;
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), ROW), Sense::click());
                crate::drive::mark(ui.ctx(), format!("file:{}", row.id), rect);
                let p = ui.painter();
                let on = cur.as_deref() == Some(row.id.as_str());
                if on {
                    p.rect_filled(rect, 4.0, ui.visuals().selection.bg_fill);
                } else if resp.hovered() {
                    p.rect_filled(rect, 4.0, ui.visuals().widgets.hovered.weak_bg_fill);
                }
                let thumb = Rect::from_min_size(rect.min + Vec2::new(4.0, 3.0), Vec2::splat(28.0));
                match &row.thumb {
                    Thumb::Img(img) => look::draw(p, img, 0, thumb.min, 28.0 / 16.0, false),
                    Thumb::Flat(c) => {
                        p.rect_filled(thumb, 2.0, *c);
                    }
                    Thumb::Ramp(cs) => {
                        let w = thumb.width() / cs.len().max(1) as f32;
                        for (i, c) in cs.iter().enumerate() {
                            p.rect_filled(Rect::from_min_size(thumb.min + Vec2::new(i as f32 * w, 0.0), Vec2::new(w, thumb.height())), 0.0, look::c32(*c));
                        }
                    }
                }
                let mut right = rect.max.x - 6.0;
                let small = egui::FontId::proportional(look::SMALL);
                if row.flags.draft {
                    let g = p.layout_no_wrap("draft".into(), small.clone(), look::DRAFT);
                    let w = g.size().x;
                    let pill = Rect::from_center_size(Pos2::new(right - w / 2.0 - 4.0, rect.center().y), Vec2::new(w + 8.0, 16.0));
                    p.rect_stroke(pill, 8.0, egui::Stroke::new(1.0, look::DRAFT.gamma_multiply(0.7)), egui::StrokeKind::Inside);
                    p.galley(pill.center() - g.size() / 2.0, g, look::DRAFT);
                    right = pill.min.x - 4.0;
                }
                if row.flags.problem {
                    p.circle_filled(Pos2::new(right - 4.0, rect.center().y), 4.0, look::BAD);
                    right -= 16.0;
                }
                let x = thumb.max.x + GAP;
                let text_col = if row.flags.made || on { ui.visuals().strong_text_color() } else { ui.visuals().text_color() };
                // a name too long for its row ends in "…", not under the badge
                let mut job = egui::text::LayoutJob::simple_singleline(name.into(), egui::FontId::proportional(look::BODY), text_col);
                if let Some(tag) = tag {
                    job.append(tag, 6.0, egui::TextFormat::simple(small.clone(), ui.visuals().weak_text_color()));
                }
                job.wrap = egui::text::TextWrapping::truncate_at_width((right - 2.0 - x).max(0.0));
                let g = p.layout_job(job);
                let end = x + g.size().x;
                p.galley(Pos2::new(x, rect.center().y - g.size().y / 2.0), g, text_col);
                if row.flags.dirty {
                    p.circle_filled(Pos2::new((end + 9.0).min(right - 6.0), rect.center().y), 3.5, look::DRAFT);
                }
                let what = if row.flags.dirty { "not saved" } else if row.flags.made { "painted" } else { "not painted yet" };
                if look::tip(resp, &format!("{}: {what}; a click opens it", row.label), "").clicked() {
                    open = Some(row.id.clone());
                }
            }
        });
        crate::drive::mark(ui.ctx(), "files", list.inner_rect);
        if let Some(id) = open {
            self.pages[self.page].open(&mut self.app, &id);
        }
    }

    fn inspector(&mut self, ui: &mut Ui) {
        // the open file's own three, in one place on every page; Save is lit while there is work to save
        let hist = self.pages[self.page].history(&self.app);
        let cmds = self.pages[self.page].commands(&self.app);
        ui.add_space(GAP * 0.75);
        ui.horizontal(|ui| {
            for (id, label) in [("undo", "Undo"), ("redo", "Redo"), ("save", "Save")] {
                let Some(c) = cmds.iter().find(|c| c.id == id) else { continue };
                let live = match (id, &hist) {
                    ("undo", Some((u, _))) => !u.is_empty(),
                    ("redo", Some((_, r))) => !r.is_empty(),
                    _ => true,
                };
                let b = if id == "save" && self.open_dirty { egui::Button::new(RichText::new(label).color(Color32::WHITE)).fill(look::GOOD.gamma_multiply(0.45)) } else { egui::Button::new(label) };
                let r = ui.add_enabled(live, b);
                crate::drive::mark(ui.ctx(), format!("btn:{id}"), r.rect);
                if look::tip(r, &c.help, &c.keys()).clicked() {
                    self.pages[self.page].run(&mut self.app, id);
                }
            }
            // whose inspector this is: the open file, in the page's colour
            let open = self.pages[self.page].current();
            if let Some(open) = self.pages[self.page].files(&self.app).into_iter().find(|f| Some(&f.id) == open.as_ref()).map(|f| look::split_label(&f.label).1.to_string()) {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(egui::Label::new(RichText::new(open).strong().color(look::page_colour(self.pages[self.page].name()))).truncate());
                });
            }
        });
        ui.add_space(GAP * 0.25);
        ui.separator();
        self.pages[self.page].inspector_top(ui, &mut self.app);
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            self.pages[self.page].inspector(ui, &mut self.app);
            ui.add_space(GAP);
            egui::CollapsingHeader::new(RichText::new("History").strong()).id_salt("history").default_open(false).open(self.history_open.take()).show(ui, |ui| {
                let Some((undo, redo)) = hist else {
                    look::weak(ui, "this page saves each change at once");
                    return;
                };
                let mut go = None;
                if ui.selectable_label(undo.is_empty(), RichText::new("as opened").small()).clicked() {
                    go = Some(0);
                }
                for (i, n) in undo.iter().enumerate() {
                    let r = ui.selectable_label(i + 1 == undo.len(), RichText::new(format!("{}. {n}", i + 1)).small());
                    if look::tip(r, "Go back to just after this step", "Ctrl+Z steps one").clicked() {
                        go = Some(i + 1);
                    }
                }
                for (i, n) in redo.iter().enumerate() {
                    let r = ui.selectable_label(false, RichText::new(format!("{}. {n}", undo.len() + i + 1)).small().weak());
                    if look::tip(r, "Undone; a click redoes up to it", "Ctrl+Y steps one").clicked() {
                        go = Some(undo.len() + i + 1);
                    }
                }
                if let Some(n) = go {
                    self.pages[self.page].back_to(&mut self.app, n);
                }
            });
        });
    }

    fn overlays(&mut self, ctx: &egui::Context) {
        match self.overlay {
            Overlay::None => {}
            Overlay::Commands => self.command_box(ctx),
            Overlay::Keys => self.key_sheet(ctx),
            Overlay::Problems => self.problems(ctx),
        }
    }

    fn command_box(&mut self, ctx: &egui::Context) {
        let mut found: Vec<(i32, (String, String, String, String))> = self.entries().into_iter().filter_map(|e| shell::score(&self.query, &e.1).map(|s| (s, e))).collect();
        found.sort_by_key(|(s, _)| -s);
        found.truncate(12);
        self.picked = self.picked.min(found.len().saturating_sub(1));
        let (mut run, mut close) = (None, false);
        let w = (ctx.content_rect().width() * 0.5).clamp(360.0, 620.0);
        let area = egui::Modal::default_area(Id::unique("command box")).anchor(Align2::CENTER_TOP, Vec2::new(0.0, 80.0));
        let m = egui::Modal::new(Id::unique("command box")).area(area).show(ctx, |ui| {
            ui.set_width(w);
            let r = ui.add(egui::TextEdit::singleline(&mut self.query).hint_text("type a file, page, tool or action").desired_width(f32::INFINITY));
            r.request_focus();
            if r.changed() {
                self.picked = 0;
            }
            ui.input_mut(|i| {
                if i.consume_key(Modifiers::NONE, Key::ArrowDown) {
                    self.picked = (self.picked + 1).min(found.len().saturating_sub(1));
                }
                if i.consume_key(Modifiers::NONE, Key::ArrowUp) {
                    self.picked = self.picked.saturating_sub(1);
                }
                if i.consume_key(Modifiers::NONE, Key::Enter) {
                    run = found.get(self.picked).map(|f| f.1 .0.clone());
                }
            });
            ui.add_space(GAP * 0.5);
            if found.is_empty() {
                look::weak(ui, "nothing found");
            }
            for (k, (_, (id, label, from, keys))) in found.iter().enumerate() {
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::click());
                let p = ui.painter();
                if k == self.picked {
                    p.rect_filled(rect, 4.0, ui.visuals().selection.bg_fill);
                } else if resp.hovered() {
                    p.rect_filled(rect, 4.0, ui.visuals().widgets.hovered.weak_bg_fill);
                }
                let y = rect.center().y;
                let kg = p.layout_no_wrap(keys.clone(), egui::FontId::monospace(look::SMALL), Color32::from_gray(170));
                let kx = rect.max.x - GAP - kg.size().x;
                p.galley(Pos2::new(kx, y - kg.size().y / 2.0), kg, Color32::from_gray(170));
                let lg = p.layout_no_wrap(label.clone(), egui::FontId::proportional(look::BODY), ui.visuals().strong_text_color());
                let lw = lg.size().x;
                p.with_clip_rect(rect.with_max_x(kx - GAP)).galley(Pos2::new(rect.min.x + GAP, y - lg.size().y / 2.0), lg, Color32::WHITE);
                let fx = rect.min.x + GAP * 2.0 + lw;
                if fx < kx - GAP * 2.0 {
                    p.with_clip_rect(rect.with_max_x(kx - GAP)).text(Pos2::new(fx, y), Align2::LEFT_CENTER, from, egui::FontId::proportional(look::SMALL), Color32::from_gray(130));
                }
                if resp.clicked() {
                    run = Some(id.clone());
                }
            }
            ui.add_space(GAP * 0.5);
            look::weak(ui, "Up and Down choose   Enter runs   Esc closes");
        });
        if m.should_close() {
            close = true;
        }
        if let Some(id) = run {
            self.overlay = Overlay::None;
            self.run_command(&id);
        } else if close {
            self.overlay = Overlay::None;
        }
    }

    fn key_sheet(&mut self, ctx: &egui::Context) {
        let page = &self.pages[self.page];
        let cmds: Vec<Cmd> = page.commands(&self.app).into_iter().filter(|c| c.key.is_some()).collect();
        let more = page.more_keys();
        let frame: Vec<Cmd> = Self::frame_commands().into_iter().filter(|c| c.key.is_some()).collect();
        let name = page.name();
        let help = page.help();
        let m = egui::Modal::new(Id::unique("key sheet")).show(ctx, |ui| {
            ui.set_width(640.0_f32.min(ctx.content_rect().width() - 80.0));
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("Keys: {name}")).strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| look::weak(ui, "Esc or a click outside closes"));
            });
            ui.separator();
            // the page's own help first: what it is for and how to start
            ui.add(egui::Label::new(RichText::new(help).color(look::ACCENT)).wrap());
            ui.add_space(GAP * 0.5);
            let rows = |ui: &mut Ui, id: &str, list: Vec<(String, String)>| {
                egui::Grid::new(id).num_columns(2).spacing(Vec2::new(GAP * 2.0, 4.0)).show(ui, |ui| {
                    for (k, what) in list {
                        ui.label(RichText::new(k).monospace().color(look::ACCENT));
                        ui.label(RichText::new(what).small());
                        ui.end_row();
                    }
                });
            };
            ui.columns(2, |c| {
                look::band(&mut c[0], name, Sense::hover());
                rows(&mut c[0], "keys page", cmds.iter().map(|k| (k.keys(), k.label.clone())).collect());
                look::band(&mut c[1], "The pointer", Sense::hover());
                rows(&mut c[1], "keys more", more.iter().map(|(k, w)| (k.to_string(), w.to_string())).collect());
                look::band(&mut c[1], "Everywhere", Sense::hover());
                rows(&mut c[1], "keys frame", frame.iter().map(|k| (k.keys(), k.label.clone())).collect());
            });
        });
        if m.should_close() {
            self.overlay = Overlay::None;
        }
    }

    fn problems(&mut self, ctx: &egui::Context) {
        let mut open = None;
        let r = egui::Area::new(Id::unique("problems")).order(egui::Order::Foreground).anchor(Align2::RIGHT_BOTTOM, Vec2::new(-GAP, -38.0)).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_max_width(520.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Problems").strong());
                    look::weak(ui, "the file check, as check=1 runs it");
                });
                ui.separator();
                if self.app.problems.is_empty() {
                    ui.label(RichText::new("Every file passes.").color(look::GOOD));
                }
                egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                    for p in &self.app.problems {
                        let r = ui.add(egui::Button::new(RichText::new(p).small()).frame(false).wrap());
                        if look::tip(r, "Open the file it names", "").clicked() {
                            open = Some(p.clone());
                        }
                    }
                });
            });
        });
        if let Some(p) = open {
            self.open_problem(&p);
        } else if r.response.clicked_elsewhere() || ctx.input(|i| i.key_pressed(Key::Escape)) {
            // the badge's own click toggles it; any other click outside closes it
            if !ctx.input(|i| i.pointer.interact_pos()).is_some_and(|p| p.y > ctx.content_rect().max.y - 30.0) {
                self.overlay = Overlay::None;
            }
        }
    }

    fn toasts(&mut self, ctx: &egui::Context) {
        let now = self.app.now;
        self.app.toasts.retain(|t| t.bad || now - t.born < 4.0);
        if self.app.toasts.is_empty() {
            return;
        }
        let mut gone = None;
        let shown = self.app.toasts.len().saturating_sub(5);
        egui::Area::new(Id::unique("toasts")).order(egui::Order::Tooltip).anchor(Align2::RIGHT_BOTTOM, Vec2::new(-GAP * 1.5, -40.0)).interactable(true).show(ctx, |ui| {
            ui.set_max_width(380.0);
            for (i, t) in self.app.toasts.iter().enumerate().skip(shown) {
                let (fill, edge) = if t.bad { (Color32::from_rgb(70, 30, 30), look::BAD) } else { (Color32::from_rgb(44, 47, 53), Color32::from_gray(70)) };
                let r = egui::Frame::new().fill(fill).stroke(egui::Stroke::new(1.0, edge)).corner_radius(6.0).inner_margin(egui::Margin::symmetric(12, 8)).show(ui, |ui| {
                    ui.set_min_width(220.0);
                    ui.add(egui::Label::new(RichText::new(&t.text).small().color(Color32::from_gray(230))).wrap());
                    if t.bad {
                        look::weak(ui, "click to dismiss");
                    }
                });
                if t.bad && r.response.interact(Sense::click()).clicked() {
                    gone = Some(i);
                }
            }
        });
        if let Some(i) = gone {
            self.app.toasts.remove(i);
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }

    fn asks(&mut self, ctx: &egui::Context) {
        let Some(name) = self.app.asks.first().cloned() else { return };
        let mut done = None;
        egui::Modal::new(Id::unique("outside change")).show(ctx, |ui| {
            ui.set_width(460.0);
            ui.label(RichText::new("Changed by another program").strong());
            ui.add_space(GAP * 0.5);
            ui.label(format!("{TEX}/{name} changed on disk while it has unsaved work here. Which one stays?"));
            ui.add_space(GAP);
            ui.horizontal(|ui| {
                if look::tip(ui.button("Keep mine"), "Keep the Studio's picture; the next save writes it over the disk's", "").clicked() {
                    done = Some(true);
                }
                if look::tip(ui.button("Take the disk's"), "Load the file from disk; your picture stays one undo step back", "").clicked() {
                    done = Some(false);
                }
            });
        });
        if let Some(mine) = done {
            self.app.asks.remove(0);
            if mine {
                self.app.keep_mine(&name);
            } else {
                self.app.take_disk(&name, App::disk(&name));
            }
            for p in &mut self.pages {
                p.refresh(&self.app);
            }
        }
    }

    /// After a crash: offer the kept textures back. Else every few seconds: keep the unsaved ones (not a shot).
    fn recover(&mut self, ctx: &egui::Context) {
        if !self.recovered.is_empty() {
            let names: Vec<String> = self.recovered.iter().map(|f| f.0.clone()).collect();
            match crate::recover::offer(ctx, "textures", &names) {
                Some(true) => {
                    for (n, img) in std::mem::take(&mut self.recovered) {
                        let d = self.app.docs.entry(n).or_insert_with(|| Doc { img: img.clone(), hist: Default::default(), saved: None });
                        if d.img != img {
                            d.hist.push(std::mem::replace(&mut d.img, img), "Recovered");
                        }
                    }
                    self.app.toast(format!("Restored as unsaved work: {}", names.join(", ")));
                    for p in &mut self.pages {
                        p.refresh(&self.app);
                    }
                }
                Some(false) => {
                    self.recovered.clear();
                    self.recovery.clear();
                }
                None => {}
            }
        } else if self.write_settings {
            let docs = &self.app.docs;
            self.recovery.keep(std::time::Instant::now(), || docs.iter().filter(|(_, d)| d.dirty()).map(|(n, d)| (n.clone(), art::encode(&d.img))).collect());
        }
    }

    fn close_check(&mut self, ctx: &egui::Context) {
        let mut unsaved: Vec<String> = self.app.docs.iter().filter(|(_, d)| d.dirty()).map(|(n, _)| n.clone()).collect();
        unsaved.extend(self.pages.iter().flat_map(|p| p.unsaved()));
        // a clean close keeps nothing to offer next time
        if self.write_settings && self.recovered.is_empty() && (self.quit || (unsaved.is_empty() && ctx.input(|i| i.viewport().close_requested()))) {
            self.recovery.clear();
        }
        if ctx.input(|i| i.viewport().close_requested()) && !self.quit && !unsaved.is_empty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.closing = true;
        }
        if !self.closing {
            return;
        }
        egui::Modal::new(Id::unique("close")).show(ctx, |ui| {
            ui.label(RichText::new("Unsaved work").strong());
            ui.label(format!("Not saved: {}", unsaved.join(", ")));
            ui.add_space(GAP);
            ui.horizontal(|ui| {
                if ui.button("Save all and close").clicked() {
                    self.app.save_textures(unsaved.clone());
                    for p in &mut self.pages {
                        p.save_all(&mut self.app);
                    }
                    self.quit = self.app.docs.values().all(|d| !d.dirty()) && self.pages.iter().all(|p| p.unsaved().is_empty());
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
