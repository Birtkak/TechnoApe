//! The Sounds page (docs/design/studio-tools.md "Sounds", ADR 0048): a row of `data/sounds.toml` made from
//! bases and a chain of steps, rendered by the game's own `audio::render_row` on a worker thread and played
//! through the game's mixer. The editing is `sound`'s, pure and tested; this draws it and hears it.
use super::look::{self, GAP};
use super::wall as wallui;
use super::{App, Cmd, FileRow, Page, Thumb, DRAFTS};
use crate::art::{self, History};
use crate::import::{self, Credit, Make};
use crate::rows;
use crate::shell::Flags;
use crate::sound::{self, Card, Layer, Sound, STARTS};
use crate::wall::Wall;
use client::audio::{self, chain, chain::STEPS, Audio, Bank, Bus, Row, RATE};
use eframe::egui::{self, Color32, Key, Modifiers, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::mpsc::{channel, Receiver, Sender};

pub const SOUNDS: &str = "data/sounds.toml";
/// The recordings' folder: `<row>.wav` and its takes, `base/` the bases (ADR 0032, 0048).
pub const REC: &str = "data/sounds";

/// A file being imported (ADR 0066): pending, shown and heard, until Use or Cancel.
struct Import {
    file: String,
    /// The whole file, one channel at 48 kHz.
    src: Vec<f32>,
    info: String,
    m: Make,
    /// The row's peak, for Level.
    peak: f32,
    level: bool,
    /// `src` made by `m`: what Use writes.
    out: Vec<f32>,
    credit: Credit,
    base: String,
    /// The handle being dragged: 0 the start, 1 the end.
    grab: Option<usize>,
}
impl Import {
    fn remake(&mut self) {
        self.m.level = self.level.then_some(self.peak);
        self.out = import::make(&self.src, &self.m);
    }
}

/// A Use: the row it was made on, its step's name in that row's undo, and each file it wrote as it was before
/// and after (none: no file), so undo and redo put the files back.
struct Wrote {
    row: String,
    name: String,
    /// The WAV: its path, its bytes before (none: no file) and after.
    wav: (String, Option<Vec<u8>>, Vec<u8>),
    /// Its credits row: the file's name there, the row before (none: no row) and after. Only this row moves on
    /// undo; the rest of the credits file is left as it is then.
    credit: (String, Option<String>, String),
    done: bool,
}

/// One row open in the Studio: the sound, its undo, and its text as saved (none: not in the file yet).
struct SDoc {
    s: Sound,
    hist: History<Sound>,
    saved: Option<String>,
}
impl SDoc {
    fn dirty(&self) -> bool {
        self.saved.as_deref() != Some(self.s.write().as_str())
    }
}

/// What the worker is asked to render; of each kind only the newest is done.
enum Job {
    /// The file's rows (the bases come from them).
    Rows(Vec<Row>),
    Take(u64, Row, Option<usize>),
    Saved(u64, Row),
    Wall(u64, Vec<Row>),
    Family(u64, Vec<Row>),
    Preview(u64, Row),
}
enum Done {
    Take(u64, Result<Vec<Vec<f32>>, String>, f32),
    Saved(u64, Option<Vec<f32>>),
    Wall(u64, Vec<Option<Vec<f32>>>),
    Family(u64, Vec<(String, Vec<f32>)>),
    Preview(u64, Option<Vec<f32>>),
}

/// The bank holds the rows `need` are made from (and theirs), built from `all`; only what is missing is made.
fn ensure(bank: &mut Option<Bank>, all: &[Row], need: &[&Row], force: bool) {
    let mut names: BTreeSet<String> = bank.as_ref().map_or(BTreeSet::new(), |b| b.rows.iter().map(|r| r.name.clone()).collect());
    let before = names.len();
    let mut stack: Vec<String> = need.iter().flat_map(|r| r.from.iter().map(|b| b.base.clone())).collect();
    while let Some(n) = stack.pop() {
        if let Some(r) = all.iter().find(|r| r.name == n) {
            if names.insert(n) {
                stack.extend(r.from.iter().map(|b| b.base.clone()));
            }
        }
    }
    if force || bank.is_none() || names.len() != before {
        let rows: Vec<Row> = all.iter().filter(|r| names.contains(&r.name)).cloned().collect();
        *bank = Some(Bank::build(rows, Some(std::path::Path::new("data/sounds")), bank.as_ref()).0);
    }
}

fn worker(rx: Receiver<Job>, tx: Sender<Done>) {
    let (mut all, mut bank) = (Vec::<Row>::new(), None::<Bank>);
    while let Ok(j) = rx.recv() {
        let (mut rows, mut take, mut saved, mut wall, mut fam, mut prev) = (None, None, None, None, None, None);
        for j in std::iter::once(j).chain(rx.try_iter()) {
            match j {
                Job::Rows(r) => rows = Some(r),
                Job::Take(g, r, s) => take = Some((g, r, s)),
                Job::Saved(g, r) => saved = Some((g, r)),
                Job::Wall(g, r) => wall = Some((g, r)),
                Job::Family(g, r) => fam = Some((g, r)),
                Job::Preview(g, r) => prev = Some((g, r)),
            }
        }
        if let Some(r) = rows {
            all = r;
            if bank.is_some() {
                ensure(&mut bank, &all, &[], true);
            }
        }
        let mut render = |r: &Row, skip: Option<usize>| {
            ensure(&mut bank, &all, &[r], false);
            audio::render_row(bank.as_ref().unwrap(), r, skip)
        };
        let done = (
            take.map(|(g, r, s)| {
                let t0 = std::time::Instant::now();
                let out = render(&r, s);
                Done::Take(g, out, t0.elapsed().as_secs_f32() * 1000.0)
            }),
            saved.map(|(g, r)| Done::Saved(g, render(&r, None).ok().and_then(|t| t.into_iter().next()))),
            prev.map(|(g, r)| Done::Preview(g, render(&r, None).ok().and_then(|t| t.into_iter().next()))),
            wall.map(|(g, rs)| Done::Wall(g, rs.iter().map(|r| render(r, None).ok().and_then(|t| t.into_iter().next())).collect())),
            fam.map(|(g, rs)| Done::Family(g, rs.iter().map(|r| (r.name.clone(), render(r, None).ok().and_then(|t| t.into_iter().next()).unwrap_or_default())).collect())),
        );
        for d in [done.0, done.1, done.2, done.3, done.4].into_iter().flatten() {
            if tx.send(d).is_err() {
                return;
            }
        }
    }
}

/// Sent and received counts of one kind of job.
#[derive(Default, Clone, Copy)]
struct Gen {
    sent: u64,
    got: u64,
}
impl Gen {
    fn next(&mut self) -> u64 {
        self.sent += 1;
        self.sent
    }
    fn busy(&self) -> bool {
        self.got < self.sent
    }
}

/// A row with one take, as the wall and the family strip render it.
fn one_take(text: &str) -> Option<Row> {
    let mut r = audio::parse(text).ok()?.pop()?;
    r.variants = 1;
    Some(r)
}

/// The wave: a column per pixel from its lowest to its highest sample, its loudness as a line over it.
fn draw_wave(p: &egui::Painter, r: Rect, x: &[f32], norm: bool, col: Color32) {
    let pk = sound::peaks(x, r.width().max(1.0) as usize);
    let scale = if norm { 1.0 / pk.iter().fold(0.05f32, |m, v| m.max(-v[0]).max(v[1])) } else { 1.0 };
    let (mid, h) = (r.center().y, r.height() * 0.48 * scale);
    p.line_segment([Pos2::new(r.min.x, mid), Pos2::new(r.max.x, mid)], Stroke::new(1.0, Color32::from_gray(55)));
    let mut env = vec![];
    for (i, v) in pk.iter().enumerate() {
        let x = r.min.x + i as f32 + 0.5;
        p.line_segment([Pos2::new(x, mid - v[1] * h), Pos2::new(x, mid - v[0] * h + 0.5)], Stroke::new(1.0, col));
        env.push(Pos2::new(x, mid - (v[2] * 1.414 * h).min(r.height() * 0.5)));
    }
    if env.len() > 1 {
        p.add(egui::Shape::line(env, Stroke::new(1.5, look::ACCENT)));
    }
}

/// The cave of In place: a stone room's ring (the game's own cave reverb is not built yet, ADR 0048).
fn cave(x: &[f32]) -> Vec<f32> {
    chain::apply(x.to_vec(), &chain::Step { kind: sound::kind("reverb").unwrap(), n: [0.75, 0.35, 0.4] })
}

pub struct Sounds {
    docs: BTreeMap<String, SDoc>,
    open: Option<String>,
    tx: Sender<Job>,
    rx: Receiver<Done>,
    take_gen: Gen,
    saved_gen: Gen,
    wall_gen: Gen,
    fam_gen: Gen,
    prev_gen: Gen,
    /// The open row as the game reads it, and what was last sent to render.
    row: Option<Row>,
    sent: Option<(String, String)>,
    takes: Vec<Vec<f32>>,
    take: usize,
    err: Option<String>,
    ms: f32,
    saved_take: Option<Vec<f32>>,
    fam: Vec<(String, Vec<f32>)>,
    audition: bool,
    distance: f32,
    in_cave: bool,
    audio: Option<Audio>,
    /// Sounds due to play: (when, samples).
    queue: VecDeque<(f64, Vec<f32>)>,
    /// What plays now: its start and length in seconds; a loop row goes round until stopped.
    head: Option<(f64, f64)>,
    going_round: bool,
    play_new: bool,
    wall: Option<Wall<Vec<Card>>>,
    wall_waves: Vec<Option<Vec<f32>>>,
    wall_for: Option<(u64, f32, Vec<Vec<Card>>)>,
    walls_made: u64,
    hover_base: Option<String>,
    merging: Option<String>,
    drag: Option<usize>,
    name_edit: String,
    disk: String,
    now: f64,
    imports: u32,
    import: Option<Import>,
    import_path: String,
    wrote: Vec<Wrote>,
    /// The bases under `data/sounds/base/`, and the open row's recordings (what the game plays instead of its chain).
    file_bases: Vec<String>,
    recorded: Vec<String>,
}

impl Sounds {
    pub fn new(_app: &App) -> Sounds {
        let (tx, jobs) = channel();
        let (done, rx) = channel();
        std::thread::Builder::new().name("studio sounds".into()).spawn(move || worker(jobs, done)).expect("a thread for the sounds");
        let mut s = Sounds {
            docs: BTreeMap::new(),
            open: None,
            tx,
            rx,
            take_gen: Gen::default(),
            saved_gen: Gen::default(),
            wall_gen: Gen::default(),
            fam_gen: Gen::default(),
            prev_gen: Gen::default(),
            row: None,
            sent: None,
            takes: vec![],
            take: 0,
            err: None,
            ms: 0.0,
            saved_take: None,
            fam: vec![],
            audition: true,
            distance: 0.0,
            in_cave: false,
            audio: None,
            queue: VecDeque::new(),
            head: None,
            going_round: false,
            play_new: false,
            wall: None,
            wall_waves: vec![],
            wall_for: None,
            walls_made: 0,
            hover_base: None,
            merging: None,
            drag: None,
            name_edit: String::new(),
            disk: String::new(),
            now: 0.0,
            imports: 0,
            import: None,
            import_path: String::new(),
            wrote: vec![],
            file_bases: vec![],
            recorded: vec![],
        };
        s.read_disk();
        s.open = s.docs.keys().find(|n| n.as_str() == "furnace.clank").or(s.docs.keys().next()).cloned();
        s.opened();
        s
    }

    /// Reads the file: rows with no unsaved work take the disk's text; the worker gets the rows.
    fn read_disk(&mut self) {
        let text = std::fs::read_to_string(SOUNDS).unwrap_or_default();
        if text == self.disk {
            return;
        }
        for sp in rows::spans(&text).into_iter().filter(|s| s.header == "sound") {
            let Ok(s) = Sound::parse(&text[sp.at.clone()]) else { continue };
            let saved = Some(s.write());
            match self.docs.get_mut(&sp.name) {
                Some(d) if d.dirty() => d.saved = saved,
                Some(d) => {
                    if d.s != s {
                        d.hist.push(std::mem::replace(&mut d.s, s), "Changed on disk");
                    }
                    d.saved = saved;
                }
                None => {
                    self.docs.insert(sp.name.clone(), SDoc { s, hist: History::default(), saved });
                }
            }
        }
        if let Ok(rows) = audio::parse(&text) {
            let _ = self.tx.send(Job::Rows(rows));
        }
        self.disk = text;
        self.sent = None;
        self.read_recordings();
    }

    /// The recordings on disk: the bases, and the open row's takes.
    fn read_recordings(&mut self) {
        let mut b: Vec<String> = std::fs::read_dir(format!("{REC}/base")).into_iter().flatten().flatten().filter_map(|e| e.file_name().to_str()?.strip_suffix(".wav").map(String::from)).collect();
        b.sort();
        self.file_bases = b;
        self.recorded = self.open.as_deref().map_or(vec![], |r| import::recordings(std::path::Path::new(REC), r));
    }

    /// A file to import: read, made 48 kHz, and shown as pending (ADR 0066).
    fn begin_import(&mut self, app: &mut App, path: &str) {
        let path = path.trim().trim_matches('"');
        if path.is_empty() {
            app.refuse("Give the sound file's path, or drop the file on the page");
            return;
        }
        // shortcut: decoded on the frame thread (a few seconds of sound in well under a second); a worker when long files are common
        match import::load(std::path::Path::new(path)) {
            Ok((src, d)) => {
                let peak = match (self.takes.first(), self.row.as_ref()) {
                    (Some(t), Some(r)) if r.gain > 0.0 => (t.iter().fold(0.0f32, |m, s| m.max(s.abs())) / r.gain).clamp(0.05, 1.0),
                    _ => 1.0,
                };
                let info = format!("{} Hz, {} channel{}, {:.0} ms", d.rate, d.channels, if d.channels == 1 { "" } else { "s" }, d.samples.len() as f32 / d.rate as f32 * 1000.0);
                let mut im = Import { file: path.to_string(), src, info, m: Make::default(), peak, level: true, out: vec![], credit: Credit::default(), base: import::base_name(path), grab: None };
                im.remake();
                self.stop();
                self.import = Some(im);
            }
            Err(e) => app.refuse(format!("Not imported: {e}")),
        }
    }

    /// Use: the import written as this row's sound (or its next take) or as a base, its credits row with it;
    /// one undo step on the open row.
    fn use_import(&mut self, app: &mut App, as_base: bool) {
        let (Some(im), Some(row)) = (self.import.as_ref(), self.open.clone()) else { return };
        if let Some(why) = im.credit.refused() {
            app.refuse(format!("Not used: {why}"));
            return;
        }
        let rel = if as_base {
            let b = im.base.trim();
            if b.is_empty() || b.contains(['/', '\\', '"']) || self.docs.contains_key(b) || self.file_bases.iter().any(|f| f == b) {
                app.refuse(format!("'{b}' is taken (a row or a base) or not a name"));
                return;
            }
            format!("base/{b}.wav")
        } else {
            match import::take_name(std::path::Path::new(REC), &row) {
                Some(n) => n,
                None => {
                    app.refuse(format!("{row} has its {} takes: none added", import::TAKES + 1));
                    return;
                }
            }
        };
        if im.out.is_empty() {
            app.refuse("Not used: nothing is left between the handles");
            return;
        }
        let path = format!("{REC}/{rel}");
        let old = std::fs::read_to_string(import::CREDITS).unwrap_or_default();
        let credit = (rel.clone(), import::credit_row(&old, &rel), Credit { file: rel.clone(), ..im.credit.clone() }.row());
        self.imports += 1;
        let name = format!("Import #{} {path}", self.imports);
        let mut w = Wrote { row: row.clone(), name: name.clone(), wav: (path.clone(), std::fs::read(&path).ok(), import::wav_bytes(&im.out)), credit, done: false };
        if let Err(e) = Self::apply(&mut w, true) {
            app.refuse(format!("Not used: {e}"));
            return;
        }
        let d = self.docs.get_mut(&row).unwrap();
        d.hist.push(d.s.clone(), &name);
        // a redo of something undone before is gone with its step
        self.wrote.retain(|o| o.done || o.row != row);
        self.wrote.push(w);
        self.import = None;
        self.after_files(app);
        app.toast(if as_base { format!("Kept {path}: a row made from '{}' plays it", rel.trim_start_matches("base/").trim_end_matches(".wav")) } else { format!("Used {path}: the game plays it for {row} within a second") });
    }

    /// Writes a Use's files as after it (`on`) or as before it.
    /// The WAV is touched only while it is what the step left (a file edited since is never removed or
    /// replaced); the credits row is set or taken out in the file as it is now.
    fn apply(w: &mut Wrote, on: bool) -> Result<(), String> {
        let (path, before, after) = &w.wav;
        let (want, put) = if on { (before.as_deref(), Some(after.as_slice())) } else { (Some(after.as_slice()), before.as_deref()) };
        if std::fs::read(path).ok().as_deref() != want {
            return Err(format!("{path} changed since: left as it is"));
        }
        let text = std::fs::read_to_string(import::CREDITS).unwrap_or_default();
        let (file, row_before, row_after) = &w.credit;
        let credits = match (on, row_before) {
            (true, _) => import::put_row(&text, file, row_after),
            (false, Some(r)) => import::put_row(&text, file, r),
            (false, None) => import::drop_credit(&text, file),
        };
        import::put_file(path, put)?;
        if let Err(e) = import::put_file(import::CREDITS, Some(credits.as_bytes())) {
            // the WAV back as it was, so the pair stays together
            let _ = import::put_file(path, want);
            return Err(e);
        }
        w.done = on;
        Ok(())
    }

    /// After an undo or redo: each Use on the open row is on disk exactly while its step is in the row's undo.
    fn follow_files(&mut self, app: &mut App) {
        let (Some(row), Some(d)) = (self.open.clone(), self.doc()) else { return };
        let steps: Vec<String> = d.hist.undo.iter().map(|s| s.name.clone()).collect();
        let mut changed = false;
        // undone newest first, redone oldest first
        for i in (0..self.wrote.len()).rev() {
            let w = &mut self.wrote[i];
            if w.row == row && w.done && !steps.contains(&w.name) {
                changed = true;
                if let Err(e) = Self::apply(w, false) {
                    app.refuse(format!("Not undone: {e}"));
                }
            }
        }
        for w in self.wrote.iter_mut().filter(|w| w.row == row && !w.done && steps.contains(&w.name)) {
            changed = true;
            if let Err(e) = Self::apply(w, true) {
                app.refuse(format!("Not redone: {e}"));
            }
        }
        if changed {
            self.after_files(app);
        }
    }

    /// Recordings changed: the worker's bank made again (a new base), the lists and the check.
    fn after_files(&mut self, app: &mut App) {
        if let Ok(rows) = audio::parse(&self.disk) {
            let _ = self.tx.send(Job::Rows(rows));
        }
        self.read_recordings();
        app.check();
    }

    /// The import panel: the file's wave with the cut's handles, its making, its credits, and Use or Cancel.
    fn import_panel(&mut self, ui: &mut Ui, app: &mut App) {
        let row = self.open.clone();
        let is_loop = self.doc().is_some_and(|d| d.s.is_loop());
        let next_take = row.as_deref().and_then(|r| import::take_name(std::path::Path::new(REC), r));
        let now = self.now;
        let head = self.head;
        let Some(im) = self.import.as_mut() else { return };
        let mut act = None;
        ui.add_space(GAP * 0.5);
        ui.label(RichText::new(format!("Import into {}: {}", row.as_deref().unwrap_or("(no sound open)"), im.file)).strong());
        look::weak(ui, format!("{}. A recording from a file: cut it, then use it as this sound, or keep it as a base that rows make many sounds from (their chain: pitch, spread, trim).", im.info));
        ui.add_space(GAP * 0.5);
        let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 150.0), Sense::click_and_drag());
        crate::drive::mark(ui.ctx(), "import_wave", r);
        let p = ui.painter_at(r);
        p.rect_filled(r, 6.0, ui.visuals().extreme_bg_color);
        let inner = r.shrink(GAP);
        draw_wave(&p, inner, &im.src, false, Color32::from_gray(150));
        let x_of = |k: f32| inner.min.x + k.clamp(0.0, 1.0) * inner.width();
        let (a, b) = (im.m.start.min(im.m.end), im.m.start.max(im.m.end));
        let dim = Color32::from_black_alpha(150);
        p.rect_filled(Rect::from_min_max(inner.min, Pos2::new(x_of(a), inner.max.y)), 0.0, dim);
        p.rect_filled(Rect::from_min_max(Pos2::new(x_of(b), inner.min.y), inner.max), 0.0, dim);
        // what Trim silence keeps inside the cut
        if im.m.trim && !im.src.is_empty() {
            let (i0, i1) = (import::share(im.src.len(), a), import::share(im.src.len(), b));
            let (t0, t1) = import::sound_bounds(&im.src[i0..i1.max(i0)]);
            let n = im.src.len() as f32;
            let s = Stroke::new(1.0, look::GOOD);
            for t in [i0 + t0, i0 + t1] {
                let x = x_of(t as f32 / n);
                p.line_segment([Pos2::new(x, inner.min.y), Pos2::new(x, inner.max.y)], s);
            }
        }
        for (k, v) in [im.m.start, im.m.end].into_iter().enumerate() {
            let x = x_of(v);
            p.line_segment([Pos2::new(x, r.min.y), Pos2::new(x, r.max.y)], Stroke::new(2.0, look::ACCENT));
            p.rect_filled(Rect::from_center_size(Pos2::new(x, r.min.y + 9.0), Vec2::new(10.0, 14.0)), 2.0, look::ACCENT);
            p.text(Pos2::new(x + if k == 0 { 7.0 } else { -7.0 }, r.max.y - 4.0), if k == 0 { egui::Align2::LEFT_BOTTOM } else { egui::Align2::RIGHT_BOTTOM }, if k == 0 { "start" } else { "end" }, egui::FontId::proportional(look::SMALL), Color32::from_gray(200));
        }
        if let Some((t0, l)) = head {
            let k = ((now - t0) / l).clamp(0.0, 1.0) as f32;
            let px = x_of(a) + k * (x_of(b) - x_of(a));
            p.line_segment([Pos2::new(px, r.min.y), Pos2::new(px, r.max.y)], Stroke::new(2.0, look::BAD));
        }
        let at = |pos: Pos2| ((pos.x - inner.min.x) / inner.width()).clamp(0.0, 1.0);
        if resp.drag_started() {
            if let Some(pos) = resp.interact_pointer_pos() {
                im.grab = Some(if (pos.x - x_of(im.m.start)).abs() <= (pos.x - x_of(im.m.end)).abs() { 0 } else { 1 });
            }
        }
        if let (Some(g), Some(pos)) = (im.grab, resp.interact_pointer_pos().filter(|_| resp.dragged())) {
            *(if g == 0 { &mut im.m.start } else { &mut im.m.end }) = at(pos);
        }
        if resp.drag_stopped() {
            im.grab = None;
            im.remake();
        }
        if look::tip(resp, "The whole file; drag a blue handle to cut the start or the end (the dark part goes). Green: what Trim silence keeps. A click plays the result.", "").clicked() {
            act = Some("import.play");
        }
        ui.add_space(GAP * 0.5);
        ui.horizontal_wrapped(|ui| {
            if look::tip(ui.button(if head.is_some() { "Stop" } else { "Play" }), "Hear what Use will write", "").clicked() {
                act = Some("import.play");
            }
            let mut ch = look::tip(ui.checkbox(&mut im.m.trim, "Trim silence"), "Cut the quiet before and after the sound (under -40 dB of its peak)", "").changed();
            ch |= look::tip(ui.checkbox(&mut im.level, "Level"), &format!("As loud as the row's own sound (peak {:.0}% of full)", im.peak * 100.0), "").changed();
            if ch {
                im.remake();
            }
            look::weak(ui, format!("{} ms fades at both ends, against clicks", import::FADE_MS));
            ui.label(RichText::new(format!("{:.0} ms", im.out.len() as f32 / import::RATE as f32 * 1000.0)).strong());
        });
        let secs = im.out.len() as f32 / import::RATE as f32;
        if !is_loop && secs > import::LONG_S {
            ui.label(RichText::new(format!("{secs:.1} s is long for a one-shot (over {} s): cut it, or make the row a loop", import::LONG_S)).small().color(look::DRAFT));
        }
        ui.add_space(GAP);
        ui.label(RichText::new("Where it came from").strong());
        egui::Grid::new("import credits").num_columns(2).spacing(Vec2::new(GAP, 4.0)).show(ui, |ui| {
            ui.label("source");
            let r = ui.add(egui::TextEdit::singleline(&mut im.credit.source).hint_text("https://… the page you downloaded it from").desired_width(320.0));
            crate::drive::mark(ui.ctx(), "import_source", r.rect);
            look::tip(r, "Kept in data/sounds/credits.toml: a credits screen needs it before any public release", "");
            ui.end_row();
            ui.label("licence");
            ui.horizontal(|ui| {
                for l in ["own", "CC0", "CC BY", "other"] {
                    let help = match l {
                        "own" => "Made or recorded by you",
                        "CC0" => "Free for anything, no credit needed",
                        "CC BY" => "Free with credit: the author and title go on the credits screen",
                        _ => "Any other licence: the check refuses it, so it cannot be kept",
                    };
                    let r = look::tip(look::chip(ui, im.credit.licence == l, l), help, "");
                    crate::drive::mark(ui.ctx(), format!("licence:{}", l.replace(' ', "_")), r.rect);
                    if r.clicked() {
                        im.credit.licence = l.into();
                    }
                }
            });
            ui.end_row();
            for (k, v) in [("author", &mut im.credit.author), ("title", &mut im.credit.title)] {
                ui.label(k);
                look::tip(ui.add(egui::TextEdit::singleline(v).hint_text("for CC BY: shown on the credits screen").desired_width(320.0)), "Who made it and what they called it", "");
                ui.end_row();
            }
        });
        if im.credit.licence == "other" {
            ui.label(RichText::new("The check refuses licences other than own, CC0 and CC BY: a file under one cannot be kept.").small().color(look::BAD));
        }
        ui.add_space(GAP);
        let why = im.credit.refused();
        ui.horizontal_wrapped(|ui| {
            let target = match (&row, &next_take) {
                (Some(_), Some(n)) => format!("Writes {REC}/{n}"),
                (Some(r), None) => format!("{r} has all its takes"),
                (None, _) => "Open a sound first".into(),
            };
            let r = ui.add_enabled(why.is_none() && next_take.is_some(), egui::Button::new(RichText::new("Use as this sound").strong()));
            crate::drive::mark(ui.ctx(), "import_use", r.rect);
            if look::tip(r, &format!("{target}: the game plays the recording for this row instead of its chain (a second one is another take)"), "").clicked() {
                act = Some("import.use");
            }
            ui.separator();
            let r = ui.add(egui::TextEdit::singleline(&mut im.base).desired_width(130.0));
            look::tip(r, "The base's name: a row made `from` it plays it through its chain", "");
            let r = ui.add_enabled(why.is_none() && row.is_some(), egui::Button::new("Keep as a base"));
            crate::drive::mark(ui.ctx(), "import_base", r.rect);
            if look::tip(r, &format!("Writes {REC}/base/{}.wav: then New from… or a row's bases make many sounds out of it", im.base.trim()), "").clicked() {
                act = Some("import.base");
            }
            ui.separator();
            let r = ui.button("Cancel");
            crate::drive::mark(ui.ctx(), "import_cancel", r.rect);
            if look::tip(r, "Forget this import: nothing is written", "Esc").clicked() {
                act = Some("import.cancel");
            }
        });
        if let Some(w) = why {
            look::weak(ui, format!("To keep it: {w}."));
        }
        if let Some(a) = act {
            self.run(app, a);
        }
    }

    fn doc(&self) -> Option<&SDoc> {
        self.open.as_ref().and_then(|n| self.docs.get(n))
    }

    /// After a row is opened: its saved take, its family and its first render.
    fn opened(&mut self) {
        (self.take, self.wall, self.wall_waves, self.wall_for, self.going_round, self.sent) = (0, None, vec![], None, false, None);
        self.name_edit = self.open.clone().unwrap_or_default();
        self.saved_take = None;
        self.recorded = self.open.as_deref().map_or(vec![], |r| import::recordings(std::path::Path::new(REC), r));
        let Some(d) = self.doc() else { return };
        let (saved, name) = (d.saved.as_deref().and_then(one_take), d.s.name());
        if let Some(r) = saved {
            let g = self.saved_gen.next();
            let _ = self.tx.send(Job::Saved(g, r));
        }
        let (fam, _, rank) = sound::family(&name);
        let members: Vec<Row> = if fam.is_empty() || rank == 1 {
            vec![]
        } else {
            let names: Vec<String> = self.docs.keys().filter(|n| sound::family(n).0 == fam && sound::family(n).2 == rank).cloned().collect();
            sound::ordered(&names).into_iter().filter_map(|(n, _)| one_take(&self.docs[&n].s.write())).collect()
        };
        self.fam.clear();
        if members.len() > 1 {
            let g = self.fam_gen.next();
            let _ = self.tx.send(Job::Family(g, members));
        }
    }

    fn edit(&mut self, name: &str, f: impl FnOnce(&mut Sound) -> bool) {
        let merge = self.merging.as_deref() == Some(name);
        let Some(d) = self.open.as_ref().and_then(|n| self.docs.get_mut(n)) else { return };
        let before = d.s.clone();
        if f(&mut d.s) && d.s != before && !merge {
            d.hist.push(before, name);
        }
    }
    fn edit_merged(&mut self, name: &str, f: impl FnOnce(&mut Sound) -> bool) {
        self.edit(name, f);
        self.merging = Some(name.into());
    }

    /// The worker's answers.
    fn poll(&mut self, app: &App) {
        while let Ok(d) = self.rx.try_recv() {
            match d {
                Done::Take(g, out, ms) => {
                    self.take_gen.got = self.take_gen.got.max(g);
                    if g == self.take_gen.sent {
                        self.ms = ms;
                        match out {
                            Ok(t) => {
                                (self.takes, self.err) = (t, None);
                                self.take = self.take.min(self.takes.len().saturating_sub(1));
                                // audition: every change plays at once (a loop going round takes it at its next lap)
                                if (self.audition && !self.going_round) || std::mem::take(&mut self.play_new) {
                                    self.play(app);
                                }
                            }
                            Err(e) => self.err = Some(e),
                        }
                    }
                }
                Done::Saved(g, t) => {
                    self.saved_gen.got = self.saved_gen.got.max(g);
                    if g == self.saved_gen.sent {
                        self.saved_take = t;
                    }
                }
                Done::Wall(g, w) => {
                    self.wall_gen.got = self.wall_gen.got.max(g);
                    if g == self.wall_gen.sent {
                        self.wall_waves = w;
                    }
                }
                Done::Family(g, f) => {
                    self.fam_gen.got = self.fam_gen.got.max(g);
                    if g == self.fam_gen.sent {
                        self.fam = f;
                    }
                }
                Done::Preview(g, t) => {
                    self.prev_gen.got = self.prev_gen.got.max(g);
                    if let Some(t) = t.filter(|_| g == self.prev_gen.sent) {
                        self.out(app, &t);
                    }
                }
            }
        }
    }

    /// The open row's text sent to render when it changed, as the game reads it: switched-off cards written
    /// off, left out by `audio::parse` and drawing nothing.
    fn feed(&mut self) {
        let Some(d) = self.doc() else { return };
        let skip = None;
        let text = d.s.write();
        let key = (d.s.name(), text.clone());
        if self.sent.as_ref() == Some(&key) {
            return;
        }
        self.sent = Some(key);
        match audio::parse(&text).map(|mut v| v.pop()) {
            Ok(Some(r)) => {
                self.row = Some(r.clone());
                self.err = None;
                let g = self.take_gen.next();
                let _ = self.tx.send(Job::Take(g, r, skip));
            }
            Ok(None) => {}
            Err(e) => self.err = Some(e),
        }
    }

    /// Samples to the sound card (opened on the first play; never in a shot), as In place sets it.
    fn out(&mut self, app: &App, x: &[f32]) {
        self.head = Some((self.now, x.len() as f64 / RATE as f64));
        if app.quiet {
            return;
        }
        let y = if self.in_cave { cave(x) } else { x.to_vec() };
        let (bus, reach) = self.row.as_ref().map_or((Bus::World, 24.0), |r| (r.bus, r.reach));
        let a = self.audio.get_or_insert_with(|| {
            let mut a = Audio::new();
            a.open();
            a
        });
        a.audition(&y, bus, self.distance, reach, if self.in_cave { 0.3 } else { 0.0 });
    }
    fn play(&mut self, app: &App) {
        if let Some(t) = self.takes.get(self.take).cloned() {
            self.queue.clear();
            self.going_round = self.doc().is_some_and(|d| d.s.is_loop());
            self.out(app, &t);
        } else {
            self.play_new = true;
        }
    }
    fn stop(&mut self) {
        (self.going_round, self.head) = (false, None);
        self.queue.clear();
    }
    /// Plays one after the other, a little apart.
    fn in_turn(&mut self, list: Vec<Vec<f32>>) {
        self.stop();
        let mut at = self.now;
        for x in list {
            let len = x.len() as f64 / RATE as f64;
            self.queue.push_back((at, x));
            at += len + 0.35;
        }
    }
    fn tick(&mut self, app: &App, ctx: &egui::Context) {
        while self.queue.front().is_some_and(|q| q.0 <= self.now) {
            let (_, x) = self.queue.pop_front().unwrap();
            self.out(app, &x);
        }
        if let Some((t0, len)) = self.head {
            if self.now >= t0 + len {
                self.head = None;
                if self.going_round {
                    if let Some(t) = self.takes.get(self.take).cloned() {
                        self.out(app, &t);
                    }
                }
            }
        }
        if self.head.is_some() || !self.queue.is_empty() || self.take_gen.busy() || self.wall_gen.busy() || self.fam_gen.busy() || self.saved_gen.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }

    fn save(&mut self, app: &mut App) {
        let Some(name) = self.open.clone() else { return };
        let text = self.docs[&name].s.write();
        match rows::save(SOUNDS, "sound", &name, &text, |t| audio::parse(t).map(|_| ())) {
            Ok(new) => {
                let d = self.docs.get_mut(&name).unwrap();
                d.saved = Some(text);
                self.disk = new.clone();
                if let Ok(rows) = audio::parse(&new) {
                    let _ = self.tx.send(Job::Rows(rows));
                }
                let drafts = art::set_draft(&std::fs::read_to_string(DRAFTS).unwrap_or_default(), &format!("{SOUNDS}#{name}"), false);
                app.write_drafts(drafts);
                app.note(SOUNDS);
                app.toast(format!("Saved {name} in {SOUNDS}"));
                app.check();
                self.opened();
            }
            Err(e) => app.refuse(format!("Not saved: {e}")),
        }
    }

    fn new_row(&mut self, base: Option<&str>) {
        let src = self.doc().map(|d| d.s.clone());
        let stem = match (base, &src) {
            (None, Some(s)) => format!("{}.copy", s.name()),
            _ => "new.sound".into(),
        };
        let name = (1..).map(|k| if k == 1 { stem.clone() } else { format!("{stem}.{k}") }).find(|n| !self.docs.contains_key(n)).unwrap();
        let s = match (base, src) {
            (None, Some(mut s)) => {
                s.keys.set("name", Some(format!("\"{name}\"")));
                s
            }
            (b, _) => {
                let b = b.unwrap_or("base.thud");
                Sound::parse(&format!("[[sound]]\nname = \"{name}\"\nbus = \"world\"\nvariants = 4\nfrom = [{{ base = \"{b}\" }}]\n")).unwrap()
            }
        };
        self.docs.insert(name.clone(), SDoc { s, hist: History::default(), saved: None });
        self.open = Some(name);
        self.opened();
    }

    fn mutate(&mut self) {
        let Some(chain) = self.doc().map(|d| d.s.chain.clone()).filter(|c| !c.is_empty()) else { return };
        self.walls_made += 1;
        self.wall = Some(Wall::new(chain, 6, 0.4, 0x50D5 + self.walls_made, &sound::mutate));
    }
    /// The wall's pictures rendered when its items changed.
    fn feed_wall(&mut self) {
        let (Some(w), Some(d)) = (&self.wall, self.doc()) else { return };
        let key = (w.seed, w.spread, w.items.clone());
        if self.wall_for.as_ref() == Some(&key) {
            return;
        }
        let rows: Vec<Row> = w
            .items
            .iter()
            .map(|chain| {
                let mut s = d.s.clone();
                s.chain = chain.clone();
                one_take(&s.write()).unwrap_or_else(|| one_take(&d.s.write_with(|_, _| true)).unwrap_or_else(|| audio::parse(&d.s.write()).unwrap().remove(0)))
            })
            .collect();
        self.wall_for = Some(key);
        self.wall_waves.clear();
        let g = self.wall_gen.next();
        let _ = self.tx.send(Job::Wall(g, rows));
    }

    fn bases(&self) -> Vec<String> {
        let mut v: Vec<String> = self.docs.keys().filter(|n| n.starts_with("base.")).cloned().collect();
        v.extend(self.file_bases.iter().cloned());
        v.extend(self.docs.keys().filter(|n| !n.starts_with("base.") && Some(*n) != self.open.as_ref()).cloned());
        v
    }

    fn header(&mut self, ui: &mut Ui, app: &mut App) {
        let Some(d) = self.doc() else { return };
        let new = d.saved.is_none();
        let name = d.s.name();
        ui.horizontal_wrapped(|ui| {
            if new {
                let r = ui.add(egui::TextEdit::singleline(&mut self.name_edit).desired_width(160.0));
                if look::tip(r.clone(), "The new row's name; Enter renames it (a row in the file keeps its name: the game plays it by name)", "").lost_focus() && self.name_edit != name {
                    let to = self.name_edit.trim().to_string();
                    if self.wrote.iter().any(|w| w.row == name) {
                        app.refuse(format!("{name} has an import under its name: it keeps the name"));
                        self.name_edit = name.clone();
                    } else if to.is_empty() || self.docs.contains_key(&to) || to.contains('"') {
                        app.refuse(format!("'{to}' is taken or not a name"));
                        self.name_edit = name.clone();
                    } else {
                        let mut d = self.docs.remove(&name).unwrap();
                        d.s.keys.set("name", Some(format!("\"{to}\"")));
                        self.docs.insert(to.clone(), d);
                        self.open = Some(to);
                        self.sent = None;
                    }
                }
            } else {
                ui.label(RichText::new(&name).strong());
            }
            let cmds = self.commands(app);
            for (id, label) in [("play", if self.going_round || self.head.is_some() { "Stop" } else { "Play" }), ("compare", "Compare"), ("all", "Every take")] {
                let c = cmds.iter().find(|c| c.id == id).unwrap();
                if look::tip(ui.button(label), &c.help, &c.keys()).clicked() {
                    self.run(app, id);
                }
            }
            look::tip(ui.toggle_value(&mut self.audition, "Audition"), "Every change plays at once (rendered beside the interface; the newest change wins)", "");
            let state = if self.take_gen.busy() { "rendering…".to_string() } else { format!("rendered in {:.0} ms", self.ms) };
            look::weak(ui, state);
        });
        if let Some(e) = &self.err {
            ui.label(RichText::new(format!("The game refuses this: {e}")).small().color(look::BAD));
        }
        if !self.recorded.is_empty() {
            look::weak(ui, format!("The game plays the recording{} {} for this row, not the chain below (Ctrl+Z after an import takes it back).", if self.recorded.len() == 1 { "" } else { "s" }, self.recorded.join(", ")));
        }
    }

    /// The family's rows side by side: a click opens one.
    fn family_strip(&mut self, ui: &mut Ui, app: &mut App) {
        if self.fam.len() < 2 {
            return;
        }
        let mut go = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            let w = ((ui.available_width() - GAP * (self.fam.len() as f32 - 1.0)) / self.fam.len() as f32).clamp(80.0, 200.0);
            for (n, x) in &self.fam {
                let (r, resp) = ui.allocate_exact_size(Vec2::new(w, 46.0), Sense::click());
                let p = ui.painter_at(r);
                let on = self.open.as_deref() == Some(n.as_str());
                p.rect_filled(r, 4.0, ui.visuals().extreme_bg_color);
                draw_wave(&p, r.shrink2(Vec2::new(4.0, 12.0)).translate(Vec2::new(0.0, 6.0)), x, true, Color32::from_gray(150));
                p.text(r.min + Vec2::new(6.0, 3.0), egui::Align2::LEFT_TOP, sound::family(n).1, egui::FontId::proportional(look::SMALL), Color32::from_gray(210));
                if on {
                    p.rect_stroke(r, 4.0, Stroke::new(2.0, look::ACCENT), egui::StrokeKind::Inside);
                }
                if look::tip(resp, &format!("{n}: a click opens it (its family side by side)"), "").clicked() && !on {
                    go = Some(n.clone());
                }
            }
        });
        if let Some(n) = go {
            self.open(app, &n);
        }
    }

    fn wave_area(&mut self, ui: &mut Ui, h: f32) {
        let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::click());
        let p = ui.painter_at(r);
        p.rect_filled(r, 6.0, ui.visuals().extreme_bg_color);
        let inner = r.shrink(GAP);
        if let Some(x) = self.takes.get(self.take) {
            draw_wave(&p, inner, x, false, Color32::from_gray(170));
            let len = x.len() as f32 / RATE;
            p.text(inner.right_top(), egui::Align2::RIGHT_TOP, format!("{len:.2} s"), egui::FontId::proportional(look::SMALL), Color32::from_gray(140));
            if let Some((t0, l)) = self.head {
                let k = ((self.now - t0) / l).clamp(0.0, 1.0) as f32;
                let px = inner.min.x + k * inner.width();
                p.line_segment([Pos2::new(px, r.min.y), Pos2::new(px, r.max.y)], Stroke::new(2.0, look::BAD));
            }
        } else {
            p.text(inner.center(), egui::Align2::CENTER_CENTER, if self.take_gen.busy() { "rendering…" } else { "no take" }, egui::FontId::proportional(look::BODY), Color32::from_gray(120));
        }
        if look::tip(resp, "The take's samples, its loudness as the blue line; a click plays it", "Space").clicked() {
            self.play_new = false;
            self.queue.clear();
            if let Some(t) = self.takes.get(self.take).cloned() {
                self.head = None;
                self.going_round = self.doc().is_some_and(|d| d.s.is_loop());
                self.queue.push_back((self.now, t));
            }
        }
    }

    fn takes_row(&mut self, ui: &mut Ui) {
        let n = self.takes.len();
        let Some(variants) = self.doc().map(|d| d.s.variants()) else { return };
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(4.0);
            ui.label(RichText::new("Takes").small());
            for k in 0..n {
                if look::tip(look::chip(ui, self.take == k, &format!("{}", k + 1)), "Show and play this take: each draws its own spread", "").clicked() {
                    self.take = k;
                }
            }
            ui.add_space(GAP);
            let mut v = variants;
            let r = ui.add(egui::DragValue::new(&mut v).range(1..=8).speed(0.05).prefix("made: "));
            if look::tip(r, "How many takes the game makes (1 to 8): no take plays twice running", "").changed() {
                self.edit_merged("Takes", |s| {
                    s.keys.set("variants", (v != 1).then(|| v.to_string()));
                    true
                });
            }
        });
    }

    fn layers(&mut self, ui: &mut Ui, app: &mut App) {
        let Some(d) = self.doc() else { return };
        look::heading(ui, "Made from");
        if d.s.from.is_empty() {
            look::weak(ui, format!("{} code layers (waves and noise), kept as written; the chain works on them too", d.s.keys.get("layer").map_or(0, |l| l.matches("wave").count())));
            return;
        }
        let from = d.s.from.clone();
        let bases = self.bases();
        let mut hover = None;
        for (i, b) in from.iter().enumerate() {
            ui.horizontal(|ui| {
                let mut pick = b.base.clone();
                let r = egui::ComboBox::from_id_salt(("base", i)).selected_text(&b.base).width(150.0).height(320.0).show_ui(ui, |ui| {
                    for n in &bases {
                        let r = ui.selectable_value(&mut pick, n.clone(), n);
                        if r.hovered() {
                            hover = Some(n.clone());
                        }
                    }
                });
                look::tip(r.response, "The base: a row or a recording in data/sounds/base/; hover one in the list to hear it", "");
                if pick != b.base {
                    self.edit("Choose a base", |s| {
                        s.from[i].base = pick;
                        true
                    });
                }
                let (mut g, mut dl) = (b.gain, b.delay);
                ui.spacing_mut().slider_width = 90.0;
                let r = ui.add(egui::Slider::new(&mut g, 0.0..=2.0).fixed_decimals(2).prefix("gain "));
                if look::tip(r.clone(), "How loud this base is in the mix", "").changed() {
                    self.edit_merged(&format!("Base {} gain", i + 1), |s| {
                        s.from[i].gain = g;
                        true
                    });
                }
                let r = ui.add(egui::Slider::new(&mut dl, 0.0..=2.0).fixed_decimals(3).suffix(" s").prefix("delay "));
                if look::tip(r, "When it starts after the first", "").changed() {
                    self.edit_merged(&format!("Base {} delay", i + 1), |s| {
                        s.from[i].delay = dl;
                        true
                    });
                }
                if look::tip(ui.small_button("x"), "Remove this base", "").clicked() && from.len() > 1 {
                    self.edit("Remove a base", |s| {
                        s.from.remove(i);
                        true
                    });
                }
            });
        }
        if look::tip(ui.small_button("+ base"), "Add another base to the mix", "").clicked() {
            self.edit("Add a base", |s| {
                s.from.push(Layer { base: "base.thud".into(), gain: 1.0, delay: 0.0 });
                true
            });
        }
        // a base under the pointer in the list: heard once
        if hover != self.hover_base {
            self.hover_base = hover.clone();
            if let Some(r) = hover.and_then(|n| self.docs.get(&n)).and_then(|d| one_take(&d.s.write())) {
                let g = self.prev_gen.next();
                let _ = self.tx.send(Job::Preview(g, r));
            }
        }
        let _ = app;
    }

    fn chain_cards(&mut self, ui: &mut Ui) {
        let Some(d) = self.doc() else { return };
        let chain = d.s.chain.clone();
        let s0 = d.s.clone();
        ui.horizontal(|ui| {
            look::heading(ui, "Chain");
            look::weak(ui, "each step changes what comes out of the one above");
        });
        let mut rects = vec![];
        for (i, c) in chain.iter().enumerate() {
            let (name, nums) = STEPS[c.kind];
            let fill = if c.on { Color32::from_rgb(40, 43, 49) } else { Color32::from_rgb(33, 34, 37) };
            let fr = egui::Frame::new().fill(fill).stroke(Stroke::new(1.0, Color32::from_rgb(58, 61, 68))).corner_radius(6.0).inner_margin(egui::Margin::symmetric(8, 6)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let (hr, h) = ui.allocate_exact_size(Vec2::new(14.0, 18.0), Sense::drag());
                    crate::drive::mark(ui.ctx(), format!("handle:{i}"), hr);
                    for k in 0..3 {
                        let y = hr.min.y + 5.0 + k as f32 * 4.0;
                        ui.painter().line_segment([Pos2::new(hr.min.x + 2.0, y), Pos2::new(hr.max.x - 2.0, y)], Stroke::new(1.5, Color32::from_gray(if h.hovered() { 200 } else { 120 })));
                    }
                    if look::tip(h.clone(), "Drag to move this step up or down the chain", "").drag_started() {
                        self.drag = Some(i);
                    }
                    let mut on = c.on;
                    let r = ui.checkbox(&mut on, RichText::new(name).strong());
                    crate::drive::mark(ui.ctx(), format!("on:{i}"), r.rect);
                    if look::tip(r, "On or off: an off step is heard without, and not saved", "").changed() {
                        self.edit(if on { "Switch a step on" } else { "Switch a step off" }, |s| s.toggle(i));
                    }
                    ui.add(egui::Label::new(RichText::new(sound::step_help(name)).small().weak()).truncate());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if look::tip(ui.small_button("x"), "Remove this step", "").clicked() {
                            self.edit(&format!("Remove {name}"), |s| s.remove(i));
                        }
                    });
                });
                for (k, num) in nums.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.add_sized(Vec2::new(68.0, 18.0), egui::Label::new(RichText::new(num.name).small()));
                        let u = sound::unit(name, num.name);
                        let pct = u == "%";
                        let log = matches!(num.name, "hz" | "times" | "seconds" | "rate" | "q");
                        let mut v = c.n[k];
                        ui.spacing_mut().slider_width = (ui.available_width() - 320.0).max(60.0);
                        let s = egui::Slider::new(&mut v, num.min..=num.max).logarithmic(log).custom_formatter(move |x, _| if pct { format!("{:.0} %", x * 100.0) } else if x.abs() >= 100.0 { format!("{x:.0} {u}") } else { format!("{x:.2} {u}") });
                        let r = ui.add(s);
                        if look::tip(r.clone(), &format!("{name}: {} from {} to {}", num.name, num.min, num.max), "").changed() {
                            self.edit_merged(&format!("{name} {}", num.name), |s| s.set(i, k, v));
                        }
                        let mut sp = s0.spread_of(c.kind, k);
                        ui.spacing_mut().slider_width = 44.0;
                        let r = ui.add(egui::Slider::new(&mut sp, 0.0..=sound::spread_max(c.kind, k)).show_value(false));
                        let shown = if pct { format!("±{:.0} %", sp * 100.0) } else { format!("±{} {u}", rows::num(sp)) };
                        look::weak(ui, if sp > 0.0 { shown.clone() } else { "same".into() });
                        if look::tip(r, &format!("Spread: how far each take strays (now {shown}), for every {name} step"), "").changed() {
                            self.edit_merged(&format!("{name} spread"), |s| s.set_spread(c.kind, k, sp));
                        }
                    });
                }
            });
            rects.push(fr.response.rect);
        }
        // a card dragged by its handle: a line where it goes, moved there when let go
        if let Some(i) = self.drag {
            let y = ui.input(|inp| inp.pointer.hover_pos()).map_or(0.0, |p| p.y);
            let to = rects.iter().enumerate().filter(|(k, r)| *k != i && r.center().y < y).count();
            let line_y = if to == 0 { rects.first().map_or(0.0, |r| r.min.y - 3.0) } else { rects.iter().enumerate().filter(|(k, _)| *k != i).nth(to - 1).map_or(0.0, |(_, r)| r.max.y + 3.0) };
            if let Some(r) = rects.first() {
                ui.painter().line_segment([Pos2::new(r.min.x, line_y), Pos2::new(r.max.x, line_y)], Stroke::new(2.0, look::ACCENT));
            }
            if !ui.input(|inp| inp.pointer.any_down()) {
                self.drag = None;
                self.edit("Move a step", |s| s.move_card(i, to));
            }
        }
        if chain.is_empty() {
            look::weak(ui, "No steps yet: add one, or pick a start in the inspector");
        }
        let menu = ui.menu_button("+ step", |ui| {
            for (name, _) in STEPS {
                let r = ui.add(egui::Button::new(RichText::new(format!("{name}  ")).strong()).right_text(RichText::new(sound::step_help(name)).small().weak()));
                crate::drive::mark(ui.ctx(), format!("step:{name}"), r.rect);
                if r.clicked() {
                    self.edit(&format!("Add {name}"), |s| {
                        s.add(sound::kind(name).unwrap());
                        true
                    });
                    ui.close();
                }
            }
        })
        .response;
        crate::drive::mark(ui.ctx(), "add_step", menu.rect);
        menu.on_hover_text("Add a step at the end of the chain: every step with what it does");
    }
}

impl Page for Sounds {
    fn name(&self) -> &'static str {
        "Sounds"
    }

    fn files(&self, app: &App) -> Vec<FileRow> {
        let names: Vec<String> = self.docs.keys().cloned().collect();
        sound::ordered(&names)
            .into_iter()
            .map(|(n, label)| {
                let d = &self.docs[&n];
                let colour = match sound::family(&n) {
                    (_, p, 0) if p == "dig" => Color32::from_rgb(150, 110, 70),
                    (_, p, 0) if p == "place" => Color32::from_rgb(100, 140, 90),
                    (_, _, 0) => Color32::from_rgb(90, 120, 160),
                    (_, _, 1) => Color32::from_gray(110),
                    _ if d.s.is_loop() => Color32::from_rgb(130, 100, 160),
                    _ => Color32::from_rgb(120, 120, 90),
                };
                FileRow { id: n.clone(), label, thumb: Thumb::Flat(colour), flags: Flags { made: d.saved.is_some(), draft: app.is_draft(&format!("{SOUNDS}#{n}")), dirty: d.dirty(), problem: false } }
            })
            .collect()
    }

    fn current(&self) -> Option<String> {
        self.open.clone()
    }

    fn open(&mut self, _app: &mut App, id: &str) {
        if self.docs.contains_key(id) && self.open.as_deref() != Some(id) {
            self.stop();
            self.open = Some(id.into());
            self.opened();
        }
    }

    fn commands(&self, _app: &App) -> Vec<Cmd> {
        let mut v = vec![
            Cmd::new("play", "Play or stop", "the take shown; a loop goes round until stopped").key(Modifiers::NONE, Key::Space),
            Cmd::new("compare", "Compare", "the saved sound, then the edited one").key(Modifiers::NONE, Key::C),
            Cmd::new("all", "Every take", "each take in turn").key(Modifiers::NONE, Key::T),
            Cmd::new("audition", "Audition on or off", "every change plays at once").key(Modifiers::NONE, Key::A),
            Cmd::new("mutate", "Mutate", "a wall of six variations of the chain's numbers").key(Modifiers::NONE, Key::M),
            Cmd::new("save", "Save the sound", "rewrite only this row of data/sounds.toml (checked by the game's reader) and take it off the drafts list").key(Modifiers::COMMAND, Key::S),
            Cmd::new("undo", "Undo", "undo the last change to this sound").key(Modifiers::COMMAND, Key::Z),
            Cmd::new("redo", "Redo", "redo what was undone").key(Modifiers::COMMAND, Key::Y),
            Cmd::new("new", "New sound from a base", "a new row made from base.thud: choose its bases and chain").key(Modifiers::COMMAND, Key::N),
            Cmd::new("copy", "Copy this sound", "a new row with this one's bases and chain"),
            Cmd::new("cave", "In a cave or the open", "hear it as in a stone room, or in the open"),
            Cmd::new("import", "Import a sound file", "WAV, OGG, MP3 or FLAC: drop it on the page, or give its path under Import (right)"),
            Cmd::new("import.use", "Use the import as this sound", "write it as this row's recording (or its next take), with its credits row"),
            Cmd::new("import.base", "Keep the import as a base", "write it under data/sounds/base/ for rows to be made from"),
            Cmd::new("import.cancel", "Cancel the import", "nothing is written"),
        ];
        for (name, help, _) in STARTS {
            v.push(Cmd::new(format!("start:{name}"), format!("Start: {name}"), help));
        }
        for (name, _) in STEPS {
            v.push(Cmd::new(format!("add:{name}"), format!("Add step: {name}"), sound::step_help(name)));
        }
        v
    }

    fn run(&mut self, app: &mut App, id: &str) {
        // with an argument (the command box and shot=): add:<step>, start:<name>, take:<wall item>,
        // distance:<m>, toggle:<card>, move:<from>;<to>, new:<base>, base:<card>;<spread>
        if let Some((cmd, arg)) = id.split_once(':') {
            match cmd {
                "add" => {
                    if let Some(k) = sound::kind(arg) {
                        self.edit(&format!("Add {arg}"), |s| {
                            s.add(k);
                            true
                        });
                    }
                }
                "start" => {
                    if let Some(cards) = STARTS.iter().find(|s| s.0 == arg).and_then(|s| sound::start_cards(s.2).ok()) {
                        self.edit(&format!("Start: {arg}"), |s| {
                            s.chain.extend(cards);
                            true
                        });
                    }
                }
                "take" => {
                    if let Some(c) = self.wall.as_ref().and_then(|w| w.items.get(arg.parse::<usize>().unwrap_or(99)).cloned()) {
                        self.edit("Take a variation", |s| {
                            s.chain = c;
                            true
                        });
                    }
                }
                "distance" => self.distance = arg.parse().unwrap_or(0.0),
                "toggle" => {
                    let i = arg.parse().unwrap_or(usize::MAX);
                    self.edit("Switch a step", |s| s.toggle(i));
                }
                "move" => {
                    if let Some((a, b)) = arg.split_once(';').and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?))) {
                        self.edit("Move a step", |s| s.move_card(a, b));
                    }
                }
                "new" => self.new_row(Some(arg)),
                "import" => self.begin_import(app, arg),
                _ => {}
            }
            return;
        }
        match id {
            "play" => {
                if self.going_round || self.head.is_some() {
                    self.stop();
                } else {
                    self.play(app);
                }
            }
            "compare" => {
                let list: Vec<Vec<f32>> = [self.saved_take.clone(), self.takes.first().cloned()].into_iter().flatten().collect();
                self.in_turn(list);
                if self.saved_take.is_none() {
                    app.toast("Not saved yet: only the edited sound plays");
                }
            }
            "all" => self.in_turn(self.takes.clone()),
            "audition" => self.audition = !self.audition,
            "mutate" => self.mutate(),
            "save" => self.save(app),
            "undo" | "redo" => {
                if let Some(d) = self.open.as_ref().and_then(|n| self.docs.get_mut(n)) {
                    if id == "undo" {
                        d.hist.undo(&mut d.s);
                    } else {
                        d.hist.redo(&mut d.s);
                    }
                }
                self.follow_files(app);
            }
            "import" => {
                let p = self.import_path.clone();
                self.begin_import(app, &p);
            }
            "import.use" => self.use_import(app, false),
            "import.base" => self.use_import(app, true),
            "import.cancel" => {
                self.import = None;
                self.stop();
            }
            "import.play" => {
                if self.head.is_some() {
                    self.stop();
                } else if let Some(x) = self.import.as_ref().map(|i| i.out.clone()) {
                    self.going_round = false;
                    self.out(app, &x);
                }
            }
            "new" => self.new_row(Some("base.thud")),
            "copy" => self.new_row(None),
            "cave" => self.in_cave = !self.in_cave,
            _ => {}
        }
    }

    fn more_keys(&self) -> Vec<(&'static str, &'static str)> {
        vec![("Click the wave", "play the take"), ("Drag a card's handle", "move the step in the chain"), ("Hover a base in its list", "hear it")]
    }

    fn work(&mut self, ui: &mut Ui, app: &mut App) {
        self.now = ui.input(|i| i.time);
        if !ui.input(|i| i.pointer.any_down()) {
            self.merging = None;
        }
        self.poll(app);
        self.feed();
        self.feed_wall();
        self.tick(app, ui.ctx());
        let dropped: Vec<std::path::PathBuf> = ui.ctx().input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        if let Some(f) = dropped.first() {
            self.begin_import(app, &f.to_string_lossy());
        }
        if self.import.is_some() {
            if ui.input(|i| i.key_pressed(Key::Escape)) {
                self.run(app, "import.cancel");
            }
            egui::ScrollArea::vertical().id_salt("import").auto_shrink([false, false]).show(ui, |ui| self.import_panel(ui, app));
            return;
        }
        if self.doc().is_none() {
            ui.add_space(GAP * 2.0);
            ui.label(RichText::new("No sound yet").strong());
            look::weak(ui, "First click: New from… (right) makes a sound from a base. Or drop a sound file here (WAV, OGG, MP3, FLAC) to start from a recording.");
            return;
        }
        ui.add_space(GAP * 0.5);
        self.header(ui, app);
        self.family_strip(ui, app);
        // the wave over the chain, the bar between them dragged
        let total = ui.available_height().max(1.0);
        let share = (total * 0.28).clamp(90.0, 200.0) / total;
        self.wave_area(ui, look::split(ui, "wave", share, total));
        self.takes_row(ui);
        look::split_bar(ui, "wave", share, total);
        egui::ScrollArea::vertical().id_salt("sound work").auto_shrink([false, false]).show(ui, |ui| {
            self.layers(ui, app);
            ui.add_space(GAP * 0.5);
            self.chain_cards(ui);
        });
    }

    fn inspector(&mut self, ui: &mut Ui, app: &mut App) {
        if let Some(mut sec) = look::section(ui, "Import") {
            let ui = &mut *sec;
            look::weak(ui, "A sound from a file (WAV, OGG, MP3, FLAC): drop it on the page, or give its path");
            ui.horizontal(|ui| {
                let r = ui.add(egui::TextEdit::singleline(&mut self.import_path).hint_text("C:\\…\\sound.ogg").desired_width((ui.available_width() - 70.0).max(80.0)));
                crate::drive::mark(ui.ctx(), "import_path", r.rect);
                let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                let b = ui.button("Import");
                crate::drive::mark(ui.ctx(), "import_go", b.rect);
                if look::tip(b, "Read the file and show it, to cut, hear and credit before anything is written", "").clicked() || enter {
                    self.run(app, "import");
                }
            });
        }
        if let Some(mut sec) = look::section(ui, "Sound") {
            let ui = &mut *sec;
            let cmds = self.commands(app);
            ui.horizontal_wrapped(|ui| {
                for (id, label) in [("copy", "Copy")] {
                    let c = cmds.iter().find(|c| c.id == id).unwrap();
                    if look::tip(ui.button(label), &c.help, &c.keys()).clicked() {
                        self.run(app, id);
                    }
                }
                let bases = self.bases();
                ui.menu_button("New from…", |ui| {
                    for b in bases.iter().filter(|b| b.starts_with("base.")) {
                        if ui.button(b).clicked() {
                            self.new_row(Some(b));
                            ui.close();
                        }
                    }
                })
                .response
                .on_hover_text("A new row made from a base: then its chain");
            });
        }
        if let Some(s) = self.doc().map(|d| d.s.clone()) {
            if let Some(mut sec) = look::section(ui, "Row") {
                let ui = &mut *sec;
                look::row(ui, "bus", |ui| {
                    ui.spacing_mut().button_padding.x = 6.0; // four chips on one line
                    for b in crate::sound::BUSES {
                        if look::tip(look::chip(ui, s.bus() == b, b), "Whose volume slider it follows in the game's Sound settings", "").clicked() {
                            self.edit("Bus", |s| s.set_bus(b));
                        }
                    }
                });
                for (i, (key, _, min, max, help)) in crate::sound::SCALARS.iter().enumerate() {
                    look::row(ui, key, |ui| {
                        let mut v = s.scalar(i);
                        let r = ui.add(egui::Slider::new(&mut v, *min..=*max).fixed_decimals(if *max > 10.0 { 0 } else { 2 }).suffix(if *key == "reach" { " m" } else { "" }));
                        if look::tip(r, help, "").changed() {
                            self.edit_merged(key, |s| s.set_scalar(i, v));
                        }
                    });
                }
            }
        }
        if let Some(mut sec) = look::section(ui, "In place") {
            let ui = &mut *sec;
            look::row(ui, "distance", |ui| {
                let reach = self.row.as_ref().map_or(24.0, |r| r.reach);
                look::tip(ui.add(egui::Slider::new(&mut self.distance, 0.0..=reach).fixed_decimals(1).suffix(" m")), "Heard as the game hears it this far ahead (0: in the head); gone at the row's reach", "");
            });
            look::row(ui, "room", |ui| {
                if look::tip(look::chip(ui, !self.in_cave, "Open"), "In the open: as it is", "").clicked() {
                    self.in_cave = false;
                }
                if look::tip(look::chip(ui, self.in_cave, "Cave"), "In a stone room: a long ring, a little duller", "").clicked() {
                    self.in_cave = true;
                }
            });
        }
        if let Some(mut sec) = look::section(ui, "Starts") {
            let ui = &mut *sec;
            look::weak(ui, "a ready chain, added to the stack");
            let mut pick = None;
            egui::Grid::new("starts").num_columns(2).spacing(Vec2::new(4.0, 4.0)).show(ui, |ui| {
                for (k, (name, help, _)) in STARTS.iter().enumerate() {
                    let w = (ui.available_width() / if k % 2 == 0 { 2.0 } else { 1.0 } - 4.0).max(60.0);
                    if look::tip(ui.add_sized(Vec2::new(w.min(130.0), 24.0), egui::Button::new(RichText::new(*name).small())), help, "").clicked() {
                        pick = Some(*name);
                    }
                    if k % 2 == 1 {
                        ui.end_row();
                    }
                }
            });
            if let Some(n) = pick {
                self.run(app, &format!("start:{n}"));
            }
        }
        if let Some(mut sec) = look::section(ui, "Mutate") {
            let ui = &mut *sec;
            ui.horizontal(|ui| {
                let r = ui.add_enabled(self.doc().is_some_and(|d| !d.s.chain.is_empty()), egui::Button::new("Mutate"));
                if look::tip(r, "Six variations of the chain's numbers; a click takes one", "M").clicked() {
                    self.mutate();
                }
                if self.wall.is_some() && look::tip(ui.small_button("close"), "Put the wall away", "").clicked() {
                    self.wall = None;
                }
            });
            let mut act = wallui::Act::None;
            let chosen = self.doc().and_then(|d| self.wall.as_ref()?.items.iter().position(|c| *c == d.s.chain));
            let waves = self.wall_waves.clone();
            if let Some(w) = &mut self.wall {
                let cell = Vec2::new(((ui.available_width() - 3.0 * GAP) / 3.0).floor().max(60.0), 44.0);
                act = wallui::show_with(ui, w, chosen, cell, true, |p, r, i, _| match waves.get(i).and_then(|x| x.as_ref()) {
                    Some(x) => draw_wave(p, r, x, true, Color32::from_gray(170)),
                    None => {
                        p.text(r.center(), egui::Align2::CENTER_CENTER, "…", egui::FontId::proportional(look::SMALL), Color32::from_gray(120));
                    }
                });
            }
            let gen: crate::wall::Gen<Vec<Card>> = &sound::mutate;
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
                wallui::Act::Play(i) => {
                    if let Some(x) = self.wall_waves.get(i).cloned().flatten() {
                        self.in_turn(vec![x]);
                    }
                }
                wallui::Act::None => {}
            }
        }
    }

    fn help(&self) -> String {
        "Space plays, C compares saved and edited, T every take, M mutates; drag a card's handle to move a step".into()
    }

    fn history(&self, _app: &App) -> Option<(Vec<String>, Vec<String>)> {
        let d = self.doc()?;
        Some((d.hist.undo.iter().map(|s| s.name.clone()).collect(), d.hist.redo.iter().rev().map(|s| s.name.clone()).collect()))
    }

    fn back_to(&mut self, app: &mut App, n: usize) {
        if let Some(d) = self.open.as_ref().and_then(|k| self.docs.get_mut(k)) {
            d.hist.back_to(&mut d.s, n);
        }
        self.follow_files(app);
    }

    fn refresh(&mut self, _app: &App) {
        self.read_disk();
    }

    fn unsaved(&self) -> Vec<String> {
        self.docs.iter().filter(|(_, d)| d.dirty()).map(|(n, _)| format!("{SOUNDS}#{n}")).collect()
    }

    fn save_all(&mut self, app: &mut App) {
        let keep = self.open.clone();
        for n in self.docs.iter().filter(|(_, d)| d.dirty()).map(|(n, _)| n.clone()).collect::<Vec<_>>() {
            self.open = Some(n);
            self.save(app);
        }
        self.open = keep;
        self.opened();
    }

    /// For `shot=`: every render asked for done.
    fn settle(&mut self, _ctx: &egui::Context, app: &mut App) {
        let t0 = std::time::Instant::now();
        while t0.elapsed() < std::time::Duration::from_secs(60) && (self.take_gen.busy() || self.wall_gen.busy() || self.fam_gen.busy() || self.saved_gen.busy()) {
            std::thread::sleep(std::time::Duration::from_millis(10));
            self.poll(app);
        }
    }
}
