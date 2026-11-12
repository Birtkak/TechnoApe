//! Unsaved work kept on disk every few seconds and offered back after a crash (docs/design/studio.md, usability
//! round, "It never loses work"). A page hands [`Recovery::keep`] its unsaved files as (name, bytes) each frame;
//! a clean close clears them; at the next start [`Recovery::left`] is what a crash left and [`offer`] asks.
use eframe::egui::{self, RichText};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Where each page keeps its set: `studio-recovery/<page>/`, beside `studio.txt`.
pub const DIR: &str = "studio-recovery";
/// How often unsaved work is written at most.
pub const EVERY: Duration = Duration::from_secs(3);

pub struct Recovery {
    dir: PathBuf,
    every: Duration,
    last: Option<Instant>,
    /// A hash of the set last written (0: none on disk).
    kept: u64,
}

impl Recovery {
    /// The set of `page` under `root` (the working folder).
    pub fn new(root: impl Into<PathBuf>, page: &str, every: Duration) -> Recovery {
        Recovery { dir: root.into().join(DIR).join(page), every, last: None, kept: 1 }
    }

    /// What a session that did not close cleanly left: (name, bytes) by name. A set half written (its
    /// temporary folder) does not count.
    pub fn left(&self) -> Vec<(String, Vec<u8>)> {
        let files = std::fs::read_dir(&self.dir).into_iter().flatten().flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_file()));
        let mut v: Vec<(String, Vec<u8>)> = files.filter_map(|e| Some((e.file_name().into_string().ok()?, std::fs::read(e.path()).ok()?))).collect();
        v.sort();
        v
    }

    /// At most every `every`: `files()` written as the set when it changed since the last write; no files
    /// clears it. True when the disk was touched.
    /// Not to be called while [`left`](Self::left) is still being offered: an empty set would clear it.
    pub fn keep(&mut self, now: Instant, files: impl FnOnce() -> Vec<(String, Vec<u8>)>) -> bool {
        if self.last.is_some_and(|t| now.duration_since(t) < self.every) {
            return false;
        }
        self.last = Some(now);
        let files = files();
        let h = if files.is_empty() {
            0
        } else {
            use std::hash::{Hash, Hasher};
            let mut h = std::hash::DefaultHasher::new();
            files.hash(&mut h);
            h.finish() | 1
        };
        if h == self.kept {
            return false;
        }
        let done = if files.is_empty() { self.clear_dir() } else { self.write(&files) };
        // a failed write is tried again at the next turn
        if done.is_ok() {
            self.kept = h;
        }
        true
    }

    /// The set written whole into a temporary folder, then put in place of the old one.
    fn write(&self, files: &[(String, Vec<u8>)]) -> std::io::Result<()> {
        let tmp = self.dir.with_extension("tmp");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp)?;
        for (name, bytes) in files.iter().filter(|(n, _)| !n.contains(['/', '\\']) && !n.starts_with('.')) {
            std::fs::write(tmp.join(name), bytes)?;
        }
        self.clear_dir()?;
        std::fs::rename(&tmp, &self.dir)
    }

    fn clear_dir(&self) -> std::io::Result<()> {
        match std::fs::remove_dir_all(&self.dir) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    /// Nothing kept (a clean close, or the developer said no).
    pub fn clear(&mut self) {
        if self.clear_dir().is_ok() {
            self.kept = 0;
        }
    }
}

/// The question after a crash: Some(true) restore, Some(false) discard, None while unanswered.
pub fn offer(ctx: &egui::Context, what: &str, names: &[String]) -> Option<bool> {
    let mut answer = None;
    egui::Modal::new(egui::Id::unique(("recover", what))).show(ctx, |ui| {
        ui.set_max_width(520.0);
        ui.label(RichText::new("Unsaved work was kept").strong());
        ui.label(format!("The Studio did not close cleanly last time. It had kept {what} not yet saved: {}.", names.join(", ")));
        ui.label(RichText::new("Restore puts them back as unsaved work: look, then Save (or Undo). Discard drops them.").small().weak());
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let r = ui.button("Restore");
            crate::drive::mark(ui.ctx(), "btn:recover.restore", r.rect);
            if r.on_hover_text("Put the kept work back, unsaved").clicked() {
                answer = Some(true);
            }
            let r = ui.button("Discard");
            crate::drive::mark(ui.ctx(), "btn:recover.discard", r.rect);
            if r.on_hover_text("Drop the kept work; the files stay as saved").clicked() {
                answer = Some(false);
            }
        });
    });
    answer
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("technoape-recover-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn unsaved_work_is_kept_every_few_seconds_and_found_again() {
        let root = temp("keep");
        let t0 = Instant::now();
        let mut r = Recovery::new(&root, "textures", Duration::from_secs(3));
        assert!(r.left().is_empty(), "nothing kept at first");
        let set = || vec![("grass.png".to_string(), vec![1, 2, 3]), ("dirt_top.png".to_string(), vec![4])];
        assert!(r.keep(t0, set), "the first call writes");
        assert!(!r.keep(t0 + Duration::from_secs(1), || panic!("not due: the files are not even made")), "not before 3 s");
        assert!(!r.keep(t0 + Duration::from_secs(4), set), "due, but nothing changed");
        // a crash: a new session finds the set
        let again = Recovery::new(&root, "textures", Duration::from_secs(3));
        assert_eq!(again.left(), vec![("dirt_top.png".to_string(), vec![4]), ("grass.png".to_string(), vec![1, 2, 3])]);
        assert!(Recovery::new(&root, "models", Duration::from_secs(3)).left().is_empty(), "each page its own set");
        // a changed set replaces the old one whole
        assert!(r.keep(t0 + Duration::from_secs(8), || vec![("grass.png".to_string(), vec![9])]));
        assert_eq!(again.left(), vec![("grass.png".to_string(), vec![9])]);
        // a half-written set is not offered
        std::fs::create_dir_all(root.join(DIR).join("sounds.tmp")).unwrap();
        std::fs::write(root.join(DIR).join("sounds.tmp/a.wav"), [1]).unwrap();
        assert!(Recovery::new(&root, "sounds", Duration::from_secs(3)).left().is_empty());
        // all saved: the set goes
        assert!(r.keep(t0 + Duration::from_secs(12), Vec::new));
        assert!(again.left().is_empty());
        assert!(!r.keep(t0 + Duration::from_secs(16), Vec::new), "nothing to clear twice");
        r.keep(t0 + Duration::from_secs(20), set);
        r.clear();
        assert!(again.left().is_empty(), "a clean close clears it");
        let _ = std::fs::remove_dir_all(&root);
    }
}
