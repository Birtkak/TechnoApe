//! The frame's logic without a window (docs/design/studio-tools.md "The interface"): `studio.txt`, the command
//! box's matching, the files column's filters, the watch on `data/` and what an outside change does.
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const SETTINGS: &str = "studio.txt";

/// What `studio.txt` keeps: the panel widths (points), the interface scale and the layout as the developer
/// left it.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub files_w: f32,
    pub inspector_w: f32,
    pub scale: f32,
    /// The inspector on the left and the files on the right.
    pub swap: bool,
    /// Side panels hidden on a page, as "Page/files" and "Page/inspector": each page has its own.
    pub hidden: BTreeSet<String>,
    /// A page's own panel widths as dragged, by the same names; `files_w` and `inspector_w` until then.
    pub widths: BTreeMap<String, f32>,
    /// A page's inspector sections in the developer's order (those not named follow in the page's own).
    pub order: BTreeMap<String, Vec<String>>,
    /// Inspector sections folded shut, as "Page/Section".
    pub folded: BTreeSet<String>,
    /// Dragged dividers of a page's work area, as "Page/name": the upper part's share of the height.
    pub splits: BTreeMap<String, f32>,
    /// Where the developer was: the page shown, and each page's open file (the Studio starts there again).
    pub page: String,
    pub open: BTreeMap<String, String>,
    /// The interface in the game's pixel font (`font=game`), else the clean sans.
    pub game_font: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { files_w: 230.0, inspector_w: 320.0, scale: 1.0, swap: false, hidden: BTreeSet::new(), widths: BTreeMap::new(), order: BTreeMap::new(), folded: BTreeSet::new(), splits: BTreeMap::new(), page: String::new(), open: BTreeMap::new(), game_font: false }
    }
}

impl Settings {
    pub const FILES: (f32, f32) = (150.0, 800.0);
    pub const INSPECTOR: (f32, f32) = (220.0, 1200.0);
    pub const SCALE: (f32, f32) = (0.6, 2.0);
    pub const SPLIT: (f32, f32) = (0.1, 0.9);

    /// `key=value` lines as `settings.txt` has them; a line it does not know or cannot read keeps the default.
    pub fn parse(text: &str) -> Settings {
        let mut s = Settings::default();
        for (k, v) in text.lines().filter_map(|l| l.split_once('=')) {
            let (k, v) = (k.trim(), v.trim());
            if k == "fold" || k == "hide" {
                if !v.is_empty() {
                    (if k == "fold" { &mut s.folded } else { &mut s.hidden }).insert(v.to_string());
                }
                continue;
            }
            if k == "font" {
                s.game_font = v == "game";
                continue;
            }
            if k == "page" {
                s.page = v.to_string();
                continue;
            }
            if let Some(page) = k.strip_prefix("open.") {
                if !v.is_empty() {
                    s.open.insert(page.to_string(), v.to_string());
                }
                continue;
            }
            if let Some(page) = k.strip_prefix("order.") {
                let list: Vec<String> = v.split('|').filter(|t| !t.is_empty()).map(str::to_string).collect();
                if !list.is_empty() {
                    s.order.insert(page.to_string(), list);
                }
                continue;
            }
            let Ok(v) = v.trim().parse::<f32>() else { continue };
            if !v.is_finite() {
                continue;
            }
            match k.trim() {
                "files" => s.files_w = v.clamp(Self::FILES.0, Self::FILES.1),
                "inspector" => s.inspector_w = v.clamp(Self::INSPECTOR.0, Self::INSPECTOR.1),
                "scale" => s.scale = v.clamp(Self::SCALE.0, Self::SCALE.1),
                "swap" => s.swap = v != 0.0,
                k => {
                    if let Some(name) = k.strip_prefix("split.") {
                        s.splits.insert(name.to_string(), v.clamp(Self::SPLIT.0, Self::SPLIT.1));
                    } else if let Some(name) = k.strip_prefix("width.") {
                        let (lo, hi) = if name.ends_with("/files") { Self::FILES } else { Self::INSPECTOR };
                        s.widths.insert(name.to_string(), v.clamp(lo, hi));
                    }
                }
            }
        }
        s
    }
    pub fn write(&self) -> String {
        let mut t = format!("files={}\ninspector={}\nscale={}\n", self.files_w.round(), self.inspector_w.round(), (self.scale * 100.0).round() / 100.0);
        if self.swap {
            t += "swap=1\n";
        }
        if self.game_font {
            t += "font=game\n";
        }
        for (k, set) in [("hide", &self.hidden), ("fold", &self.folded)] {
            for f in set {
                t += &format!("{k}={f}\n");
            }
        }
        for (k, v) in &self.widths {
            t += &format!("width.{k}={}\n", v.round());
        }
        for (k, v) in &self.order {
            t += &format!("order.{k}={}\n", v.join("|"));
        }
        for (k, v) in &self.splits {
            t += &format!("split.{k}={v}\n");
        }
        if !self.page.is_empty() {
            t += &format!("page={}\n", self.page);
        }
        for (k, v) in &self.open {
            t += &format!("open.{k}={v}\n");
        }
        t
    }
}

/// A pixel of the game's font in points at scale 1 (`font.rs`: size 14 of an em of 9.33 pixels).
pub const FONT_PIXEL: f32 = 1.5;
/// The interface scale next to `scale` (`dir` 1 up, -1 down, 0 the nearest) among those that draw a pixel of
/// the font on whole screen pixels, so the text is sharp; `native` is the screen's pixels a point. `scale`
/// itself when the range holds none.
pub fn sharp_scale(scale: f32, native: f32, dir: i32) -> f32 {
    let unit = FONT_PIXEL * native.max(0.1);
    let (lo, hi) = ((Settings::SCALE.0 * unit).ceil().max(1.0), (Settings::SCALE.1 * unit).floor());
    if hi < lo {
        return scale;
    }
    ((scale * unit).round() + dir as f32).clamp(lo, hi) / unit
}

/// The scales the clean font offers (it is sharp at any).
pub const CLEAN_SCALES: [f32; 8] = [0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0];
/// The next scale up (`dir` 1), down (-1) or the nearest (0): among the game font's sharp ones
/// ([`sharp_scale`]) or the clean font's list.
pub fn step_scale(scale: f32, native: f32, dir: i32, game: bool) -> f32 {
    if game {
        return sharp_scale(scale, native, dir);
    }
    let near = (0..CLEAN_SCALES.len()).min_by(|a, b| (CLEAN_SCALES[*a] - scale).abs().total_cmp(&(CLEAN_SCALES[*b] - scale).abs())).unwrap();
    CLEAN_SCALES[(near as i32 + dir).clamp(0, CLEAN_SCALES.len() as i32 - 1) as usize]
}

/// What "Try in game" says to a game it started that is still running: the command that stands the open
/// thing ahead of the player. None where the game shows a saved file by itself (models, effects, textures).
pub fn game_line(page: &str, open: Option<&str>) -> Option<String> {
    match (page, open) {
        ("Structures", Some(n)) => Some(format!("/structure {n}")),
        ("Fauna", Some(n)) => Some(format!("/spawn {n}")),
        _ => None,
    }
}

/// "Try in game": the game's arguments for a throwaway creative world (no save is kept) with the page's open
/// thing standing ahead at the spawn; `extra` is what only the page knows (a creature's model).
pub fn try_args(page: &str, open: Option<&str>, extra: &[String]) -> Vec<String> {
    // (lines=stdin: the game runs what the Studio sends it later, `game_line`)
    let mut v: Vec<String> = ["run=play", "onscreen=1", "save=0", "lines=stdin"].map(String::from).into();
    let mut acts = "0.5:say:/playmode creative".to_string();
    match (page, open) {
        ("Models", Some(n)) => v.push(format!("model={n}")),
        ("Effects", Some(n)) => v.push(format!("fxplay={n}")),
        ("Structures", Some(n)) => v.push(format!("structure={n}")),
        ("Fauna", Some(n)) => v.push(format!("creature={n}")),
        ("Textures", Some(n)) => acts += &format!(",1:say:/give {n} 64"),
        _ => {}
    }
    if open.is_some() {
        v.extend(extra.iter().cloned());
    }
    v.push(format!("acts={acts}"));
    v
}

/// How well `query` finds `text` in the command box: every letter in order, ignoring case; a run of the
/// query inside the text beats scattered letters, an earlier one beats a later one. None: not found.
pub fn score(query: &str, text: &str) -> Option<i32> {
    let (q, t) = (query.trim().to_lowercase(), text.to_lowercase());
    if q.is_empty() {
        return Some(0);
    }
    if let Some(at) = t.find(&q) {
        let word = at == 0 || t[..at].ends_with([' ', ':', '_', '.']);
        return Some(1000 - at as i32 + if word { 200 } else { 0 });
    }
    let mut chars = t.char_indices();
    let (mut gaps, mut last) = (0, None);
    for c in q.chars() {
        let (i, _) = chars.by_ref().find(|(_, d)| *d == c)?;
        if let Some(l) = last {
            gaps += (i - l - 1) as i32;
        }
        last = Some(i);
    }
    Some(500 - gaps)
}

/// The filter chips over the files column.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Filter {
    All,
    Made,
    Drafts,
    Unsaved,
    Problems,
}
pub const FILTERS: [(Filter, &str, &str); 5] = [
    (Filter::All, "All", "every file of the page"),
    (Filter::Made, "Made", "files that exist and are not Claude's drafts"),
    (Filter::Drafts, "Drafts", "Claude's drafts, still to be redone by hand"),
    (Filter::Unsaved, "Unsaved", "files with work not saved yet"),
    (Filter::Problems, "Problems", "files the check refuses"),
];

/// What a row of the files column is.
#[derive(Clone, Copy, Default, Debug)]
pub struct Flags {
    pub made: bool,
    pub draft: bool,
    pub dirty: bool,
    pub problem: bool,
}

impl Filter {
    pub fn keeps(self, f: Flags) -> bool {
        match self {
            Filter::All => true,
            Filter::Made => f.made && !f.draft,
            Filter::Drafts => f.draft,
            Filter::Unsaved => f.dirty,
            Filter::Problems => f.problem,
        }
    }
}

/// The file a problem line of the check names ("data/textures/x.png: why").
pub fn problem_file(line: &str) -> Option<&str> {
    line.split_once(": ").map(|(f, _)| f).filter(|f| f.starts_with("data/"))
}

/// The modified times of the files the Studio reads: some named files and every file in some folders.
pub struct Watch {
    files: Vec<PathBuf>,
    dirs: Vec<PathBuf>,
    seen: BTreeMap<PathBuf, SystemTime>,
}

fn stamp(files: &[PathBuf], dirs: &[PathBuf]) -> BTreeMap<PathBuf, SystemTime> {
    let mut m = BTreeMap::new();
    let mut add = |p: PathBuf| {
        if let Ok(t) = std::fs::metadata(&p).and_then(|m| m.modified()) {
            m.insert(p, t);
        }
    };
    files.iter().cloned().for_each(&mut add);
    for d in dirs {
        // a temporary file of a save in progress is not a change
        std::fs::read_dir(d).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.extension().is_none_or(|e| e != "tmp")).for_each(&mut add);
    }
    m
}

impl Watch {
    /// Takes the first look, so what is there now is not a change.
    pub fn new(files: &[&str], dirs: &[&str]) -> Watch {
        let files: Vec<PathBuf> = files.iter().map(PathBuf::from).collect();
        let dirs: Vec<PathBuf> = dirs.iter().map(PathBuf::from).collect();
        let seen = stamp(&files, &dirs);
        Watch { files, dirs, seen }
    }
    /// The paths made, changed or gone since the last look, by modified time only (cheap: no file is read).
    pub fn poll(&mut self) -> Vec<PathBuf> {
        let now = stamp(&self.files, &self.dirs);
        let mut out: Vec<PathBuf> = now.iter().filter(|(p, t)| self.seen.get(*p) != Some(t)).map(|(p, _)| p.clone()).collect();
        out.extend(self.seen.keys().filter(|p| !now.contains_key(*p)).cloned());
        out.sort();
        self.seen = now;
        out
    }
    /// The Studio wrote this file itself: its new time is not a change.
    pub fn note(&mut self, path: &Path) {
        match std::fs::metadata(path).and_then(|m| m.modified()) {
            Ok(t) => self.seen.insert(path.to_path_buf(), t),
            Err(_) => self.seen.remove(path),
        };
    }
}

/// What a file changed on disk by another program does to the Studio's copy.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outside {
    /// What is on disk is what the Studio last saw (its own save, or only touched).
    Nothing,
    /// No unsaved work: take the disk's.
    Reload,
    /// Unsaved work and a different file on disk: the developer picks which to keep.
    Ask,
}

/// `dirty`: the Studio's copy has unsaved work; `disk`: the file now (None: gone or unreadable); `saved`: the
/// file as the Studio last read or wrote it (None: never on disk).
pub fn outside<T: PartialEq>(dirty: bool, disk: Option<&T>, saved: Option<&T>) -> Outside {
    if disk == saved {
        Outside::Nothing
    } else if dirty {
        Outside::Ask
    } else {
        Outside::Reload
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_in_game_starts_a_throwaway_creative_world_with_the_open_thing_ahead() {
        let a = |page, open, extra: &[&str]| try_args(page, open, &extra.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        let plain = a("Palette", None, &[]);
        assert_eq!(plain, ["run=play", "onscreen=1", "save=0", "lines=stdin", "acts=0.5:say:/playmode creative"]);
        assert!(a("Models", Some("furnace"), &[]).contains(&"model=furnace".to_string()));
        assert!(a("Effects", Some("fire"), &[]).contains(&"fxplay=fire".to_string()));
        assert!(a("Structures", Some("oak"), &[]).contains(&"structure=oak".to_string()));
        let boar = a("Fauna", Some("boar"), &["model=boar:walk"]);
        assert!(boar.contains(&"creature=boar".to_string()) && boar.contains(&"model=boar:walk".to_string()), "{boar:?}");
        // a block is given, to place and look at
        assert_eq!(a("Textures", Some("blue lamp"), &[]).last().unwrap(), "acts=0.5:say:/playmode creative,1:say:/give blue lamp 64");
        // nothing open: the plain world
        assert_eq!(a("Models", None, &[]), plain);
        // to a game already running: the command that stands the thing ahead, where there is one
        assert_eq!(game_line("Structures", Some("oak")).as_deref(), Some("/structure oak"));
        assert_eq!(game_line("Fauna", Some("boar")).as_deref(), Some("/spawn boar"));
        assert!(game_line("Models", Some("ape")).is_none() && game_line("Fauna", None).is_none());
    }

    #[test]
    fn the_scale_steps_are_those_that_draw_the_fonts_pixel_on_whole_screen_pixels() {
        let near = |a: f32, b: f32| (a - b).abs() < 1e-4;
        // a plain screen: a font pixel is 1, 2 or 3 screen pixels at 67, 133 and 200 %
        assert!(near(sharp_scale(1.0, 1.0, 0), 4.0 / 3.0) && near(sharp_scale(1.2, 1.0, 0), 4.0 / 3.0));
        assert!(near(sharp_scale(4.0 / 3.0, 1.0, 1), 2.0) && near(sharp_scale(4.0 / 3.0, 1.0, -1), 2.0 / 3.0));
        // the ends hold
        assert!(near(sharp_scale(2.0, 1.0, 1), 2.0) && near(sharp_scale(2.0 / 3.0, 1.0, -1), 2.0 / 3.0));
        // a screen of 1.5 pixels a point: 89, 133 and 178 %
        assert!(near(sharp_scale(1.0, 1.5, 0), 2.0 / 2.25) && near(sharp_scale(1.0, 1.5, 1), 3.0 / 2.25));
        for (s, n) in [(1.0, 1.0), (0.7, 1.25), (1.9, 2.0), (1.3, 1.75)] {
            let px = sharp_scale(s, n, 0) * FONT_PIXEL * n;
            assert!(near(px, px.round()) && px >= 1.0, "{s} on {n}: {px}");
        }
    }

    #[test]
    fn the_clean_font_steps_through_its_list_and_the_font_is_kept() {
        assert_eq!((step_scale(1.0, 1.0, 1, false), step_scale(1.0, 1.0, -1, false), step_scale(1.3, 1.0, 0, false)), (1.1, 0.9, 1.25));
        assert_eq!((step_scale(2.0, 1.0, 1, false), step_scale(0.8, 1.0, -1, false)), (2.0, 0.8));
        assert!((step_scale(1.0, 1.0, 0, true) - 4.0 / 3.0).abs() < 1e-4, "the game font keeps its sharp steps");
        let s = Settings { game_font: true, ..Settings::default() };
        assert_eq!(Settings::parse(&s.write()), s);
        assert!(!Settings::parse("scale=1").game_font, "clean when absent");
    }

    #[test]
    fn studio_txt_round_trips_and_holds_its_ranges() {
        let mut s = Settings { files_w: 250.0, inspector_w: 300.0, scale: 1.25, ..Settings::default() };
        assert_eq!(Settings::parse(&s.write()), s);
        // the layout: hidden panels, the inspector's side, folded sections and dragged dividers
        // each page's own panels: hidden or not, and how wide
        s.swap = true;
        s.hidden.insert("Sounds/inspector".into());
        s.widths.insert("Models/inspector".into(), 640.0);
        s.folded.insert("Fauna/Parts".into());
        s.splits.insert("Structures/view".into(), 0.7);
        s.order.insert("Fauna".into(), vec!["Gait".into(), "Size and speed".into(), "Body".into()]);
        // where the developer was: the page and each page's open file
        s.page = "Sounds".into();
        s.open.insert("Sounds".into(), "furnace.clank".into());
        s.open.insert("Textures".into(), "blue lamp".into());
        assert_eq!(Settings::parse(&s.write()), s);
        assert!(Settings::parse("open.Models=\n").open.is_empty());
        let t = Settings::parse("scale=7\nfiles=10\ninspector=nope\nother=3\nno line\nsplit.Models/view=9\nsplit.x=nope\nfold=\nhide=\norder.Models=\nwidth.Models/files=5\nwidth.Models/inspector=99999\n");
        let widths = [("Models/files".to_string(), Settings::FILES.0), ("Models/inspector".to_string(), Settings::INSPECTOR.1)].into();
        assert_eq!(t, Settings { files_w: 150.0, inspector_w: 320.0, scale: 2.0, splits: [("Models/view".to_string(), Settings::SPLIT.1)].into(), widths, ..Settings::default() });
        assert_eq!(Settings::parse(""), Settings::default());
        assert_eq!(Settings::parse("scale=NaN").scale, 1.0);
    }

    #[test]
    fn the_command_box_finds_runs_before_scattered_letters() {
        assert_eq!(score("", "anything"), Some(0));
        assert!(score("shade", "Tool: Shade").unwrap() > score("shade", "Tool: Scatter, shaded").unwrap_or(-9999) - 1);
        assert!(score("sav", "Save").unwrap() > score("sav", "Swap a variant").unwrap(), "a run beats letters spread out");
        assert!(score("grs", "grass").is_some());
        assert_eq!(score("zz", "grass"), None);
        assert_eq!(score("GRASS", "grass"), score("grass", "Grass"), "case does not matter");
        assert!(score("top", "Open grass_top.png").unwrap() < score("top", "Top face").unwrap(), "a word's start first");
    }

    #[test]
    fn the_filter_chips_keep_their_rows() {
        let draft = Flags { made: true, draft: true, ..Default::default() };
        let mine = Flags { made: true, ..Default::default() };
        let none = Flags::default();
        assert!(Filter::Made.keeps(mine) && !Filter::Made.keeps(draft) && !Filter::Made.keeps(none));
        assert!(Filter::Drafts.keeps(draft) && !Filter::Drafts.keeps(mine));
        assert!(Filter::Unsaved.keeps(Flags { dirty: true, ..none }) && !Filter::Unsaved.keeps(mine));
        assert!(Filter::Problems.keeps(Flags { problem: true, ..none }) && Filter::All.keeps(none));
    }

    #[test]
    fn a_problem_names_its_file() {
        assert_eq!(problem_file("data/textures/rock_top.png: pixel (1, 2) is #010203, not a palette colour"), Some("data/textures/rock_top.png"));
        assert_eq!(problem_file("no colon here"), None);
        assert_eq!(problem_file("blocks: a thing"), None);
    }

    #[test]
    fn the_watch_reports_made_changed_and_gone_files_once() {
        let d = std::env::temp_dir().join(format!("technoape-studio-watch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("tex")).unwrap();
        let (pal, tex) = (d.join("palette.toml"), d.join("tex"));
        std::fs::write(&pal, "a").unwrap();
        std::fs::write(tex.join("a.png"), "a").unwrap();
        let mut w = Watch::new(&[pal.to_str().unwrap()], &[tex.to_str().unwrap()]);
        assert_eq!(w.poll(), Vec::<PathBuf>::new(), "the first look is no change");
        let later = SystemTime::now() + std::time::Duration::from_secs(5);
        std::fs::File::options().write(true).open(&pal).unwrap().set_modified(later).unwrap();
        std::fs::write(tex.join("b.png"), "b").unwrap();
        std::fs::write(tex.join("c.png.tmp"), "half").unwrap();
        assert_eq!(w.poll(), vec![pal.clone(), tex.join("b.png")]);
        assert_eq!(w.poll(), Vec::<PathBuf>::new(), "reported once");
        std::fs::remove_file(tex.join("a.png")).unwrap();
        assert_eq!(w.poll(), vec![tex.join("a.png")]);
        let later = later + std::time::Duration::from_secs(5);
        std::fs::File::options().write(true).open(tex.join("b.png")).unwrap().set_modified(later).unwrap();
        w.note(&tex.join("b.png"));
        assert_eq!(w.poll(), Vec::<PathBuf>::new(), "the Studio's own write is not a change");
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn an_outside_change_reloads_when_clean_and_asks_over_unsaved_work() {
        let (a, b) = (1, 2);
        assert_eq!(outside(false, Some(&a), Some(&a)), Outside::Nothing, "its own save");
        assert_eq!(outside(true, Some(&a), Some(&a)), Outside::Nothing, "touched: the unsaved work stays");
        assert_eq!(outside(false, Some(&b), Some(&a)), Outside::Reload);
        assert_eq!(outside(true, Some(&b), Some(&a)), Outside::Ask);
        assert_eq!(outside(false, None, Some(&a)), Outside::Reload, "deleted, nothing lost");
        assert_eq!(outside(true, None, Some(&a)), Outside::Ask, "deleted under unsaved work");
        assert_eq!(outside(true, Some(&b), None), Outside::Ask, "a new file in the Studio and one made elsewhere");
        assert_eq!(outside::<i32>(true, None, None), Outside::Nothing);
    }
}
