//! The frame's logic without a window (docs/design/studio-tools.md "The interface"): `studio.txt`, the command
//! box's matching, the files column's filters, the watch on `data/` and what an outside change does.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const SETTINGS: &str = "studio.txt";

/// What `studio.txt` keeps: the panel widths (points) and the interface scale.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub files_w: f32,
    pub inspector_w: f32,
    pub scale: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { files_w: 210.0, inspector_w: 280.0, scale: 1.0 }
    }
}

impl Settings {
    pub const FILES: (f32, f32) = (150.0, 420.0);
    pub const INSPECTOR: (f32, f32) = (220.0, 520.0);
    pub const SCALE: (f32, f32) = (0.8, 2.0);

    /// `key=value` lines as `settings.txt` has them; a line it does not know or cannot read keeps the default.
    pub fn parse(text: &str) -> Settings {
        let mut s = Settings::default();
        for (k, v) in text.lines().filter_map(|l| l.split_once('=')) {
            let Ok(v) = v.trim().parse::<f32>() else { continue };
            if !v.is_finite() {
                continue;
            }
            match k.trim() {
                "files" => s.files_w = v.clamp(Self::FILES.0, Self::FILES.1),
                "inspector" => s.inspector_w = v.clamp(Self::INSPECTOR.0, Self::INSPECTOR.1),
                "scale" => s.scale = v.clamp(Self::SCALE.0, Self::SCALE.1),
                _ => {}
            }
        }
        s
    }
    pub fn write(&self) -> String {
        format!("files={}\ninspector={}\nscale={}\n", self.files_w.round(), self.inspector_w.round(), (self.scale * 100.0).round() / 100.0)
    }
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
    fn studio_txt_round_trips_and_holds_its_ranges() {
        let s = Settings { files_w: 250.0, inspector_w: 300.0, scale: 1.25 };
        assert_eq!(Settings::parse(&s.write()), s);
        let t = Settings::parse("scale=7\nfiles=10\ninspector=nope\nother=3\nno line\n");
        assert_eq!(t, Settings { files_w: 150.0, inspector_w: 280.0, scale: 2.0 });
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
