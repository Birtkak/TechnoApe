//! The menus and the window: pages, rows, settings applied, display modes, resizing (ADR 0025 L: moved out of main.rs).
use super::*;

/// What a start without arguments runs: the title menu over a throwaway world, seen from orbit.
pub(crate) const TITLE_ARGS: [&str; 5] = ["run=play", "onscreen=1", "title=1", "save=0", "record=0"];
/// shortcut: the title lists the newest six saves; a scrolling list when there are more.
pub(crate) const WORLDS_SHOWN: usize = 6;

/// The saves under saves/, newest first.
pub(crate) fn list_worlds() -> Vec<String> {
    let mut v: Vec<(std::time::SystemTime, String)> = std::fs::read_dir("saves")
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "sav"))
        .map(|e| (e.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH), e.path().display().to_string().replace('\\', "/")))
        .collect();
    v.sort_by(|a, b| b.0.cmp(&a.0));
    v.into_iter().take(WORLDS_SHOWN).map(|(_, p)| p).collect()
}

/// Takes a world out of the list: its save moves to removed/ beside it, under its own name or the first free
/// one after it, so nothing is ever deleted or written over. Says where it went.
pub(crate) fn remove_world(save: &str) -> std::io::Result<std::path::PathBuf> {
    let from = std::path::Path::new(save);
    let dir = from.parent().unwrap_or(std::path::Path::new(".")).join("removed");
    std::fs::create_dir_all(&dir)?;
    let stem = from.file_stem().and_then(|s| s.to_str()).unwrap_or("world");
    let to = (1..).map(|n| dir.join(if n == 1 { format!("{stem}.sav") } else { format!("{stem}-{n}.sav") })).find(|p| !p.exists()).unwrap();
    std::fs::rename(from, &to)?;
    // its waypoints go with it, so the next world of that name starts with none and this one keeps its own
    let marks = format!("{save}.waypoints");
    if std::path::Path::new(&marks).exists() {
        std::fs::rename(&marks, format!("{}.waypoints", to.display()))?;
    }
    Ok(to)
}

#[test]
fn a_removed_world_moves_aside_and_never_over_another() {
    let dir = std::env::temp_dir().join(format!("technoape-remove-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let save = dir.join("world-1.sav");
    let marks = dir.join("world-1.sav.waypoints");
    for text in ["first", "second"] {
        std::fs::write(&save, text).unwrap();
        std::fs::write(&marks, text).unwrap();
        remove_world(save.to_str().unwrap()).unwrap();
        assert!(!save.exists() && !marks.exists(), "the save and its waypoints both go");
    }
    assert_eq!(std::fs::read_to_string(dir.join("removed/world-1-2.sav.waypoints")).unwrap(), "second");
    assert_eq!(std::fs::read_to_string(dir.join("removed/world-1.sav")).unwrap(), "first");
    assert_eq!(std::fs::read_to_string(dir.join("removed/world-1-2.sav")).unwrap(), "second");
    assert!(remove_world(save.to_str().unwrap()).is_err(), "a save that is not there");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// How long ago, in the largest unit that fits: "just now", "5 min ago", "3 h ago", "2 days ago".
pub(crate) fn ago(secs: u64) -> String {
    match secs {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", secs / 60),
        3600..=86_399 => format!("{} h ago", secs / 3600),
        86_400..=172_799 => "yesterday".into(),
        _ => format!("{} days ago", secs / 86_400),
    }
}

/// What a world of the list is: its seed, its planet's size and when it was last played. A save this build
/// cannot read says so, and is still listed.
/// shortcut: reads the whole save for two numbers, on the frame thread, when the page opens; a header of
/// its own in the save format when saves grow past a few megabytes.
/// What the list says of a save this build cannot read (one of 0.5 m blocks, ADR 0033); a click on it
/// starts nothing.
pub(crate) const OLD_WORLD: &str = "an older world, cannot be played";

pub(crate) fn world_note(path: &str) -> String {
    let played = std::fs::metadata(path).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).map_or(String::new(), |d| format!(", played {}", ago(d.as_secs())));
    match server::save::Save::read(std::path::Path::new(path)) {
        Ok(s) => {
            let size = match s.planet_n {
                6144 => "4 km".to_string(),
                12288 => "8 km".to_string(),
                n => format!("{n} columns"),
            };
            format!("seed {}, {size}{played}", s.seed)
        }
        Err(_) => format!("{OLD_WORLD}{played}"),
    }
}

#[test]
fn a_world_s_age_is_told_in_the_largest_unit_that_fits() {
    let told: Vec<String> = [0, 59, 60, 3599, 3600, 86_399, 86_400, 172_800, 30 * 86_400].into_iter().map(ago).collect();
    assert_eq!(told, ["just now", "just now", "1 min ago", "59 min ago", "1 h ago", "23 h ago", "yesterday", "2 days ago", "30 days ago"]);
    assert!(world_note("no/such/file.sav").starts_with(OLD_WORLD));
}

/// The row of the "remove world k?" page that removes: never row k itself, where the pointer still is from
/// the click that asked, so a second click by habit keeps the world.
pub(crate) fn remove_row(k: usize) -> usize {
    (k != 1) as usize
}

#[test]
fn the_removing_row_is_not_under_the_click_that_asked() {
    for k in 0..WORLDS_SHOWN {
        assert!(remove_row(k) != k && remove_row(k) < 2, "world {k}");
    }
}

/// The menu's pages (ADR 0023): the title's, and the pause menu's.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Menu {
    Title,
    Worlds,
    /// The question before world k of the list is removed.
    Remove(usize),
    Pause,
    /// The places the player marked (the pause menu's): a click opens one, a right click or Delete takes one away.
    Waypoints,
    /// Waypoint k's own page: its name, colour, icon and where it shows (bugs/2026-11-11_13-15-26).
    Waypoint(usize),
    /// F10: set-ups to look at without typing commands (bugs/2026-11-11_13-12-42).
    Playtest,
    /// The list of the settings' pages, one of them (`settings::PAGES`), and the question after a display
    /// change: keep it?
    Settings,
    Page(usize),
    Confirm,
    /// The death screen (ADR 0040): how, Respawn, or back to the title. Esc does not close it.
    Dead,
}

/// Window sizes the settings offer, those that fit the monitor.
pub(crate) const RESOLUTIONS: [(u32, u32); 5] = [(1280, 720), (1600, 900), (1920, 1080), (2560, 1440), (3840, 2160)];

/// The window's fullscreen state for `s` on monitor `m`, and the size to render at: exclusive fullscreen
/// takes the monitor's fastest mode of the chosen size, or the whole screen borderless when it has none.
pub(crate) fn fullscreen(s: &Settings, m: Option<winit::monitor::MonitorHandle>) -> (Option<winit::window::Fullscreen>, (u32, u32)) {
    use winit::window::Fullscreen;
    let size = (s.get(S::Width) as u32, s.get(S::Height) as u32);
    let Some(m) = m.filter(|_| s.name(S::Display) != "windowed") else { return (None, size) };
    let screen = (m.size().width, m.size().height);
    let mode = m.video_modes().filter(|v| (v.size().width, v.size().height) == size).max_by_key(|v| v.refresh_rate_millihertz());
    match mode {
        Some(v) if s.name(S::Display) == "fullscreen" => (Some(Fullscreen::Exclusive(v)), size),
        _ => (Some(Fullscreen::Borderless(Some(m))), screen),
    }
}

/// How fast Q and E roll the body out in space (ADR 0024), radians a second.
pub(crate) const ROLL: f64 = 1.5;

/// The value after `cur` in `list` (before it when `back`), around the end; from the nearest when `cur` is
/// not in it.
pub(crate) fn cycle<T: Copy + PartialEq>(list: &[T], cur: T, back: bool) -> T {
    let i = list.iter().position(|v| *v == cur).unwrap_or(0);
    list[if back { (i + list.len() - 1) % list.len() } else { (i + 1) % list.len() }]
}

/// A mouse button as a binding.
pub(crate) fn mouse_bind(b: MouseButton) -> Option<Bind> {
    Some(Bind::Mouse(match b {
        MouseButton::Left => 1,
        MouseButton::Right => 2,
        MouseButton::Middle => 3,
        MouseButton::Back => 4,
        MouseButton::Forward => 5,
        _ => return None,
    }))
}

impl App {
    /// The interface's scale, and the screen's size in the interface's own pixels.
    pub(crate) fn ui(&self) -> f32 {
        self.settings.get(S::UiScale) as f32
    }
    pub(crate) fn ui_size(&self) -> (f32, f32) {
        (w() as f32 / self.ui(), h() as f32 / self.ui())
    }

    pub(crate) fn monitors(&self) -> Vec<winit::monitor::MonitorHandle> {
        self.gpu.as_ref().map_or(Vec::new(), |g| g.window.available_monitors().collect())
    }
    /// The settings' monitor, or the one the window is on when there is no such.
    pub(crate) fn monitor(&self) -> Option<winit::monitor::MonitorHandle> {
        let win = &self.gpu.as_ref()?.window;
        let n = self.settings.at(S::Monitor);
        n.checked_sub(1).and_then(|k| win.available_monitors().nth(k)).or_else(|| win.current_monitor())
    }

    /// Row `k` of the open settings page.
    pub(crate) fn page_row(&self, k: usize) -> Option<Row> {
        match self.menu {
            Some(Menu::Page(p)) => self.page_rows(p).get(k).copied(),
            _ => None,
        }
    }
    /// A settings page's rows: the keys page's as its search leaves them.
    pub(crate) fn page_rows(&self, p: usize) -> Vec<Row> {
        if p == 3 { self.settings.key_rows(&self.key_query) } else { settings::page_rows(p) }
    }
    /// The open page's rows' tones for `hud::menu`: a key another action shares red, a heading plain.
    pub(crate) fn menu_tones(&self) -> Vec<u8> {
        let Some(Menu::Page(p)) = self.menu else { return Vec::new() };
        self.page_rows(p)
            .into_iter()
            .map(|r| match r {
                Row::Key(a) if self.settings.clashes(a) => 1,
                Row::Group(_) => 2,
                _ => 0,
            })
            .collect()
    }

    /// The menu's buttons on its open page, all of them: the menu shows as many as fit, from `scroll` on.
    pub(crate) fn menu_items(&self) -> Vec<String> {
        let s = &self.settings;
        match self.menu {
            None => Vec::new(),
            Some(Menu::Title) => vec!["Singleplayer".into(), "Settings".into(), "Quit game".into()],
            Some(Menu::Worlds) => {
                let names = self.worlds.iter().zip(&self.world_notes).map(|(p, n)| format!("{}   {n}", p.trim_start_matches("saves/").trim_end_matches(".sav")));
                names.chain(["New world: Terra, 4 km, a seed of its own".into(), "Back".into()]).collect()
            }
            Some(Menu::Remove(k)) => {
                let name = self.worlds.get(k).map_or("", |p| p.trim_start_matches("saves/").trim_end_matches(".sav"));
                let mut rows = vec!["Keep it".to_string(), format!("Remove {name}")];
                if remove_row(k) == 0 {
                    rows.swap(0, 1);
                }
                rows
            }
            Some(Menu::Dead) => vec!["Respawn".into(), "To the title".into()],
            Some(Menu::Pause) => vec!["Back to game".into(), "Settings".into(), "Waypoints".into(), "Main menu".into(), "Quit game".into()],
            Some(Menu::Waypoints) => {
                let w = &self.world;
                let me = self.me.pos;
                let rows = self.waypoints.iter().map(|p| {
                    let b = &w.bodies[p.body.min(w.bodies.len() - 1)];
                    let far = if w.frame_at(me) == p.body { format!("{:.0} m away", (b.centre + p.rel - me).length()) } else { "on another body".into() };
                    if self.creative { format!("{}: {} {}, {far}", p.name, b.name, nav::geo_text(nav::geo(p.rel, self.meridian(p.body)))) } else { p.name.clone() }
                });
                rows.chain(["Back".into()]).collect()
            }
            Some(Menu::Waypoint(k)) => {
                let Some(p) = self.waypoints.get(k) else { return vec!["Back".into()] };
                let name = self.edit.as_ref().map_or(p.name.clone(), |e| format!("{}_", e.1));
                vec![
                    format!("Name: {name}"),
                    format!("Colour: {}", nav::COLOUR_NAMES[p.colour % nav::COLOURS.len()]),
                    format!("Icon: {}", nav::ICONS[p.icon % nav::ICONS.len()]),
                    format!("Shown: {}", p.view.text()),
                    "Remove this waypoint".into(),
                    "Back".into(),
                ]
            }
            Some(Menu::Playtest) => play::PLAYTEST.iter().map(|r| r.0.to_string()).chain(["Back to game".to_string()]).collect(),
            Some(Menu::Settings) => settings::PAGES.iter().map(|p| p.0.to_string()).chain(["Done".into()]).collect(),
            Some(Menu::Confirm) => {
                let left = self.revert.as_ref().map_or(0, |r| r.1.saturating_duration_since(Instant::now()).as_secs() + 1);
                vec!["Keep these display settings".into(), format!("Go back ({left} s)")]
            }
            Some(Menu::Page(p)) => self
                .page_rows(p)
                .into_iter()
                .map(|r| match r {
                    Row::Resolution if s.name(S::Display) == "windowed fullscreen" => format!("Resolution: {}x{} (native)", w(), h()),
                    Row::Resolution => format!("Resolution: {}x{}", s.get(S::Width), s.get(S::Height)),
                    Row::Set(S::Monitor) => match s.at(S::Monitor).min(self.monitors().len()) {
                        0 => "Monitor: the one the window is on".into(),
                        k => format!("Monitor: {k} of {}", self.monitors().len()),
                    },
                    Row::Set(x) => s.row_text(x),
                    Row::Preset => format!("Preset: {}", s.preset().map_or("Custom", |i| settings::PRESETS[i])),
                    Row::Key(a) if self.rebind.map(|r| r.0) == Some(a) => format!("{}: press a key or a button", settings::ACTS[a as usize].label),
                    Row::Key(a) => s.key_text(a),
                    Row::KeyDefaults => "Back to the default keys".into(),
                    Row::Search if self.edit.is_some() => format!("Search: {}_", self.key_query),
                    Row::Search if self.key_query.is_empty() => "Search: click, then type an action or a key".into(),
                    Row::Search => format!("Search: {}", self.key_query),
                    Row::Group(name) => name.into(),
                })
                .chain(["Back".into()])
                .collect(),
        }
    }

    /// How many rows fit under the page's title.
    pub(crate) fn menu_fit(&self) -> usize {
        let h = self.ui_size().1;
        (((h - (h * 0.36).max(230.0) - 76.0) / 60.0) as usize).max(3) // clear of the hotbar
    }

    /// The open page's sliders, row by row.
    pub(crate) fn menu_sliders(&self) -> Vec<hud::Slider> {
        (0..self.menu_items().len())
            .map(|k| {
                let Some(Row::Set(s)) = self.page_row(k) else { return None };
                let (at, text) = self.settings.slider(s)?;
                let typing = self.edit.as_ref().filter(|e| e.0 == k);
                Some((at, typing.map_or(text, |e| e.1.clone()), typing.is_some()))
            })
            .collect()
    }

    /// A left or right click at the pointer in the menu: on a slider's track it drags the slider, on its number
    /// it starts typing one; anywhere else on a button it presses it.
    pub(crate) fn menu_click(&mut self, back: bool) {
        let (x, y) = self.pointer;
        let Some(v) = self.menu_boxes.iter().position(|b| x >= b[0] && x < b[0] + b[2] && y >= b[1] && y < b[1] + b[3]) else { return };
        let k = v + self.scroll;
        if self.menu_sliders().get(k).is_some_and(|s| s.is_some()) {
            let (t, n) = hud::slider_parts(self.menu_boxes[v], &self.menu_items()[k]);
            if x >= n[0] - 12.0 {
                self.edit = Some((k, String::new()));
                return;
            }
            if x >= t[0] - 12.0 && !back {
                self.drag = Some(k);
                self.slide();
                return;
            }
        }
        self.menu_press(k, back);
    }

    /// The dragged slider follows the pointer along its track.
    pub(crate) fn slide(&mut self) {
        let Some(k) = self.drag else { return };
        let (Some(Row::Set(s)), Some(b)) = (self.page_row(k), k.checked_sub(self.scroll).and_then(|v| self.menu_boxes.get(v))) else { return };
        let (t, _) = hud::slider_parts(*b, &self.menu_items()[k]);
        self.settings.slide(s, ((self.pointer.0 - t[0]) / t[2]) as f64);
    }

    /// Keeps the settings in their file.
    pub(crate) fn save_settings(&mut self) {
        if self.revert.is_some() {
            return; // a display change nobody has kept yet stays out of the file
        }
        if let Err(e) = std::fs::write(&self.settings_file, self.settings.text()) {
            self.note(format!("the settings could not be saved to {}: {e}", self.settings_file));
        }
    }

    /// The keyboard's place in the menu moves by `d` rows; the window of rows follows it.
    pub(crate) fn select(&mut self, d: i32) {
        let n = self.menu_items().len();
        if n == 0 {
            return;
        }
        let k = self.sel.map_or(0, |k| (k as i32 + d).clamp(0, n as i32 - 1) as usize);
        self.sel = Some(k);
        self.scroll = self.scroll.clamp((k + 1).saturating_sub(self.menu_fit()), k);
    }

    /// The window of rows moves by `d` (the wheel).
    pub(crate) fn scroll_by(&mut self, d: i32) {
        let most = self.menu_items().len().saturating_sub(self.menu_fit());
        self.scroll = (self.scroll as i32 + d).clamp(0, most as i32) as usize;
    }

    /// Left and right on the keyboard's row: a slider a step, any other row its value before or after.
    pub(crate) fn nudge(&mut self, back: bool) {
        let Some(k) = self.sel.filter(|_| matches!(self.menu, Some(Menu::Page(_)))) else { return };
        if let Some(Row::Set(s)) = self.page_row(k) {
            if let settings::Kind::Slider { step, .. } = settings::DEFS[s as usize].kind {
                let before = self.settings.clone();
                self.settings.set(s, self.settings.get(s) + if back { -step } else { step });
                self.apply_settings(&before);
                return;
            }
        }
        self.menu_press(k, back);
    }

    /// Button `k` of the open page pressed (`back`: with the right button, a value goes the other way).
    pub(crate) fn menu_press(&mut self, k: usize, back: bool) {
        self.audio.play("ui.click", None, 1.0, 1.0, false);
        match (self.menu, k) {
            (Some(Menu::Title), 0) => self.set_menu(Some(Menu::Worlds)),
            (Some(Menu::Title) | Some(Menu::Pause), 1) => self.set_menu(Some(Menu::Settings)),
            (Some(Menu::Title), 2) | (Some(Menu::Pause), 4) => self.finish(),
            (Some(Menu::Pause), 2) => self.set_menu(Some(Menu::Waypoints)),
            (Some(Menu::Waypoints), k) if k < self.waypoints.len() => {
                if back {
                    self.remove_waypoint(k);
                } else {
                    self.set_menu(Some(Menu::Waypoint(k)));
                }
            }
            (Some(Menu::Waypoints), _) => self.set_menu(Some(Menu::Pause)),
            // a waypoint's page: the name is typed (Enter keeps it), the other rows step on (back: the one before)
            (Some(Menu::Waypoint(j)), row) if j < self.waypoints.len() && row < 4 => {
                let w = &mut self.waypoints[j];
                match row {
                    0 => self.edit = Some((0, w.name.clone())),
                    1 => w.colour = cycle(&[0, 1, 2, 3, 4, 5], w.colour % nav::COLOURS.len(), back),
                    2 => w.icon = cycle(&[0, 1, 2, 3], w.icon % nav::ICONS.len(), back),
                    _ => w.view = cycle(&nav::VIEWS, w.view, back),
                }
                self.save_waypoints();
            }
            (Some(Menu::Waypoint(j)), 4) => {
                self.remove_waypoint(j);
                self.set_menu(Some(Menu::Waypoints));
            }
            (Some(Menu::Waypoint(_)), _) => self.set_menu(Some(Menu::Waypoints)),
            (Some(Menu::Playtest), k) if k < play::PLAYTEST.len() => {
                self.playtest(k);
                self.set_menu(None);
            }
            (Some(Menu::Playtest), _) => self.set_menu(None),
            // the right button on a world asks whether to remove it
            (Some(Menu::Worlds), k) if k < self.worlds.len() && back => self.set_menu(Some(Menu::Remove(k))),
            (Some(Menu::Remove(k)), row) if row == remove_row(k) && !back => {
                if let Some(save) = self.worlds.get(k).cloned() {
                    match remove_world(&save) {
                        Ok(to) => self.note(format!("{save} moved to {}", to.display().to_string().replace('\\', "/"))),
                        Err(e) => self.note(format!("{save} could not be removed: {e}")),
                    }
                }
                self.set_menu(Some(Menu::Worlds));
            }
            (Some(Menu::Remove(_)), _) => self.set_menu(Some(Menu::Worlds)),
            (Some(Menu::Worlds), k) if self.world_notes.get(k).is_some_and(|n| n.starts_with(OLD_WORLD)) => {}
            (Some(Menu::Worlds), k) if k < self.worlds.len() => self.start_again(vec![format!("save={}", self.worlds[k])]),
            (Some(Menu::Worlds), k) if k == self.worlds.len() => {
                // a new world: the first free saves/world-N.sav, a seed from the clock; the server makes the file
                let n = (1..).find(|n| !std::path::Path::new(&format!("saves/world-{n}.sav")).exists()).unwrap();
                let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.subsec_nanos() as i32 & 0x7fff_ffff);
                self.start_again(vec![format!("save=saves/world-{n}.sav"), format!("seed={seed}")]);
            }
            (Some(Menu::Worlds), _) => self.set_menu(Some(Menu::Title)),
            (Some(Menu::Pause), 0) => self.set_menu(None),
            // the server answers a respawn with full health, which closes this page (`vitals_in`)
            (Some(Menu::Dead), 0) => self.link.send(ToServer::Respawn),
            (Some(Menu::Dead), 1) => self.start_again(TITLE_ARGS[2..].iter().map(|s| s.to_string()).collect()),
            (Some(Menu::Pause), 3) => self.start_again(TITLE_ARGS[2..].iter().map(|s| s.to_string()).collect()),
            (Some(Menu::Settings), k) if k < settings::PAGES.len() => self.set_menu(Some(Menu::Page(k))),
            (Some(Menu::Settings), _) => self.set_menu(Some(self.home())),
            (Some(Menu::Confirm), 0) => {
                self.revert = None;
                self.save_settings();
                self.set_menu(Some(Menu::Page(0)));
            }
            (Some(Menu::Confirm), _) => self.undo_display(),
            (Some(Menu::Page(_)), k) => match self.page_row(k) {
                Some(row) => self.change(row, back),
                None => self.set_menu(Some(Menu::Settings)),
            },
            _ => {}
        }
    }

    /// A settings row pressed: its next value (the one before when `back`), applied at once.
    pub(crate) fn change(&mut self, row: Row, back: bool) {
        let before = self.settings.clone();
        match row {
            Row::Resolution if self.settings.name(S::Display) == "windowed fullscreen" => {}
            Row::Resolution => {
                // a window fits the screen; exclusive fullscreen takes the sizes the screen can switch to
                let mon = self.monitor();
                let exclusive = self.settings.name(S::Display) == "fullscreen";
                let fits: Vec<(u32, u32)> = RESOLUTIONS
                    .iter()
                    .copied()
                    .filter(|r| match &mon {
                        None => true,
                        Some(m) if exclusive => m.video_modes().any(|v| (v.size().width, v.size().height) == *r),
                        Some(m) => r.0 <= m.size().width && r.1 <= m.size().height,
                    })
                    .collect();
                let cur = (self.settings.get(S::Width) as u32, self.settings.get(S::Height) as u32);
                let (nw, nh) = cycle(if fits.is_empty() { &RESOLUTIONS[..1] } else { &fits }, cur, back);
                self.settings.set(S::Width, nw as f64);
                self.settings.set(S::Height, nh as f64);
            }
            Row::Preset => {
                // from Custom to High, the default
                let n = match (self.settings.preset(), back) {
                    (None, _) => 2,
                    (Some(i), false) => (i + 1) % 4,
                    (Some(i), true) => (i + 3) % 4,
                };
                self.settings.set_preset(n);
            }
            Row::Set(S::Monitor) => {
                // 0 (the window's own), then each screen
                let n = self.monitors().len() + 1;
                let i = self.settings.at(S::Monitor).min(n - 1);
                self.settings.set(S::Monitor, ((i + if back { n - 1 } else { 1 }) % n) as f64);
            }
            Row::Set(s) => self.settings.cycle(s, back),
            Row::Key(a) => {
                // the row waits for its key: the first with the left button, the second with the right
                self.rebind = Some((a, back as usize));
                return;
            }
            Row::KeyDefaults => self.settings.default_binds(),
            // the search row (always the first) takes what is typed; the rows under it follow each key
            Row::Search => {
                self.edit = Some((0, self.key_query.clone()));
                return;
            }
            Row::Group(_) => return,
        }
        self.apply_settings(&before);
    }

    /// Does what a change of the settings from `before` needs, and keeps the file. A change that could lose the
    /// picture is kept only once the player says so (docs/design/settings.md): it goes back after 10 s.
    pub(crate) fn apply_settings(&mut self, before: &Settings) {
        self.streamer.opts.k = self.settings.get(S::Detail);
        hud::set_contrast(self.settings.on(S::Contrast));
        if self.settings.on(S::Vsync) != before.on(S::Vsync) {
            self.apply_present();
        }
        if settings::RISKY.iter().any(|s| self.settings.get(*s) != before.get(*s)) {
            self.apply_display();
            // off screen there is no picture to lose; confirm=1 asks all the same, for scripted checks
            if self.onscreen || self.args.get("confirm", 0) == 1 {
                if self.revert.is_none() {
                    self.revert = Some((before.clone(), Instant::now() + Duration::from_secs(10)));
                }
                self.set_menu(Some(Menu::Confirm));
                return;
            }
        }
        if self.revert.is_none() {
            self.save_settings();
        }
    }

    /// The surface waits for the display or does not, as the settings say.
    pub(crate) fn apply_present(&mut self) {
        let vsync = self.settings.on(S::Vsync);
        let Some(gpu) = self.gpu.as_mut() else { return };
        gpu.config.present_mode = if vsync { wgpu::PresentMode::AutoVsync } else { wgpu::PresentMode::AutoNoVsync };
        gpu.surface.configure(&gpu.device, &gpu.config);
    }

    /// The display settings from before the change nobody kept.
    pub(crate) fn undo_display(&mut self) {
        let Some((old, _)) = self.revert.take() else { return };
        self.settings = old;
        self.apply_display();
        self.save_settings();
        self.set_menu(Some(Menu::Page(0)));
    }

    /// The meridian a body's longitudes count from: the spawn's on the first body, on a moon the middle of
    /// the face it keeps turned to its planet (its -x, `engine::sky`).
    pub(crate) fn meridian(&self, body: usize) -> DVec3 {
        if body == 0 { self.world.bodies[0].spawn_point() - self.world.bodies[0].centre } else { -DVec3::X }
    }

    /// Keeps the waypoints in their file, when the world has one.
    pub(crate) fn save_waypoints(&mut self) {
        if let Some(f) = self.waypoint_file.clone() {
            if let Err(e) = std::fs::write(&f, nav::text(&self.waypoints)) {
                self.note(format!("the waypoints could not be saved to {f}: {e}"));
            }
        }
    }

    pub(crate) fn remove_waypoint(&mut self, k: usize) {
        if k < self.waypoints.len() {
            let w = self.waypoints.remove(k);
            self.note(format!("{} removed", w.name));
            self.save_waypoints();
        }
    }

    /// The key or button for the key row that waits for one; None clears it.
    pub(crate) fn bind_to(&mut self, b: Option<Bind>) {
        if let Some((a, slot)) = self.rebind.take() {
            self.settings.binds[a as usize][slot] = b;
            self.save_settings();
        }
    }

    /// A key or button went down or up: the inputs of the actions bound to it. A key held repeats nothing.
    /// `fire` false only holds the key down.
    pub(crate) fn bind_event(&mut self, b: Bind, down: bool, fire: bool) {
        let changed = if down { self.keys.insert(b) } else { self.keys.remove(&b) };
        if !changed || !fire {
            return;
        }
        for a in self.settings.acts_of(b).collect::<Vec<_>>() {
            let d = &settings::ACTS[a as usize];
            let input = if down { d.press } else { d.release };
            if !input.is_empty() {
                self.act(input);
            }
        }
    }

    /// Frames a second play is held to on screen (the settings' cap; in the background its own), None for no
    /// cap. Benchmarks and off-screen runs have none.
    pub(crate) fn cap_hz(&self) -> Option<f64> {
        if self.run != "play" || !self.onscreen {
            return None;
        }
        let refresh = self.gpu.as_ref()?.window.current_monitor().and_then(|m| m.refresh_rate_millihertz()).filter(|m| *m >= 10_000).map_or(60.0, |m| m as f64 / 1000.0);
        let cap = match self.settings.at(S::FrameCap) {
            0 => Some(refresh),
            1 => Some(refresh / 2.0),
            7 => None,
            _ => self.settings.name(S::FrameCap).parse().ok(),
        }
        .filter(|hz: &f64| *hz >= 1.0);
        if !self.focused && self.settings.at(S::BackCap) == 0 {
            return Some(cap.unwrap_or(30.0).min(30.0));
        }
        cap
    }

    /// The graphics rows the shaders read, a bit each (`g.opt.x`): sharp sun shadows, no reflections, flat
    /// block colours, no stars. The defaults are 0, today's picture.
    pub(crate) fn gfx_bits(&self) -> u32 {
        let s = &self.settings;
        (s.at(S::Shadows) == 0) as u32 | (!s.on(S::Reflections) as u32) << 1 | ((s.at(S::Textures) == 0) as u32) << 2 | (!s.on(S::Stars) as u32) << 3
    }

    /// The title menu's view: in orbit over the planet's sunlit limb, the planet filling the left half and the
    /// sun to the right, drifting slowly around the line to the sun while the planet turns under it.
    pub(crate) fn title_view(&self, t: f64) -> Pose {
        let w = &self.world;
        let (c, r) = (w.bodies[0].centre, w.bodies[0].radius);
        let s = (w.from_inertial(0, w.sky.sun(w.time)) - c).normalize();
        // just out of the air (it fades by 1.5 km up), the sun 100 degrees from the planet's centre as seen from
        // here, so more than half the disc in view is lit
        let q = DQuat::from_axis_angle(s, t * 0.01) * s.any_orthonormal_vector();
        let (d, b) = (r + 1700.0, 100f64.to_radians());
        let o = -(s * b.cos() + q * b.sin());
        // the limb lies asin(r / d) off the centre's direction; the look a third of the way from it to the sun
        let l = (r / d).asin();
        let a = l + (b - l) * 0.35;
        let e = (s + o * s.dot(o)).normalize(); // the sun's way, square to the centre's
        let fwd = -o * a.cos() + e * a.sin();
        // rolled 35 degrees: the planet low on the left, the sun high on the right
        let up = DQuat::from_axis_angle(fwd, 35f64.to_radians()) * s.cross(fwd).normalize();
        Pose { pos: c + o * d, fwd, up }
    }

    /// The page the settings go back to: the title's in the title menu, else the pause menu.
    pub(crate) fn home(&self) -> Menu {
        if self.args.get("title", 0) == 1 { Menu::Title } else { Menu::Pause }
    }

    /// Closes this world (the local server saves as it stops) and starts the game again with `more` after
    /// play on screen: a fresh process per world, so nothing of the old one carries over.
    pub(crate) fn start_again(&mut self, more: Vec<String>) {
        let mut a: Vec<String> = TITLE_ARGS[..2].iter().map(|s| s.to_string()).collect();
        a[1] = format!("onscreen={}", self.args.get("onscreen", 0));
        // scripted checks keep their length and settings file in the next world too
        a.extend(["seconds", "settings"].iter().filter_map(|k| self.args.0.get(*k).map(|v| format!("{k}={v}"))));
        a.extend(more);
        self.next = Some(a);
        self.finish();
    }

    /// Opens a menu page or closes the menu: the pointer is free while it is open.
    pub(crate) fn set_menu(&mut self, m: Option<Menu>) {
        if m == Some(Menu::Worlds) {
            self.worlds = list_worlds();
            self.world_notes = self.worlds.iter().map(|p| world_note(p)).collect();
        }
        if m != self.menu {
            self.scroll = 0;
            self.menu_since = Instant::now();
            self.key_query.clear();
            self.edit = None;
        }
        (self.sel, self.rebind) = (None, None);
        self.menu = m;
        (self.held, self.mouse) = (None, (0.0, 0.0));
        self.keys.clear();
        self.zoomed &= self.settings.at(S::ZoomMode) == 1;
        if self.onscreen {
            let win = &self.gpu.as_ref().unwrap().window;
            if m.is_some() {
                let _ = win.set_cursor_grab(CursorGrabMode::None);
            } else {
                let _ = win.set_cursor_grab(CursorGrabMode::Confined).or_else(|_| win.set_cursor_grab(CursorGrabMode::Locked));
            }
            win.set_cursor_visible(m.is_some());
        }
    }

    /// The window as the settings say: fullscreen at the screen's size, or a window of the chosen size.
    pub(crate) fn apply_display(&mut self) {
        if !self.onscreen {
            self.resize(self.settings.get(S::Width) as u32, self.settings.get(S::Height) as u32);
            return;
        }
        let win = self.gpu.as_ref().unwrap().window.clone();
        let mon = self.monitor();
        let (fs, (nw, nh)) = fullscreen(&self.settings, mon.clone());
        let windowed = fs.is_none();
        win.set_fullscreen(fs);
        win.set_decorations(windowed);
        if windowed {
            // a window moves to the settings' monitor when it is on another
            if let Some(m) = mon.filter(|m| self.settings.at(S::Monitor) > 0 && Some(m) != win.current_monitor().as_ref()) {
                win.set_outer_position(m.position());
            }
            let _ = win.request_inner_size(winit::dpi::PhysicalSize::new(nw, nh));
        }
        self.resize(nw, nh);
    }

    /// A new render size: the surface, the depth buffer, the capture buffer and the bounce light's targets
    /// are made again at it.
    pub(crate) fn resize(&mut self, nw: u32, nh: u32) {
        if nw == 0 || nh == 0 || (nw, nh) == (w(), h()) {
            return; // minimised, or no change
        }
        SIZE.store((nw as u64) << 32 | nh as u64, Ordering::Relaxed);
        let gpu = self.gpu.as_mut().unwrap();
        (gpu.config.width, gpu.config.height) = (nw, nh);
        gpu.surface.configure(&gpu.device, &gpu.config);
        gpu.depth = depth_view(&gpu.device);
        if gpu.capture.is_some() {
            gpu.capture = Some(capture_buf(&gpu.device));
        }
        let rt = self.rt.as_ref().unwrap();
        let shared = [
            gpu.globals.as_entire_binding(),
            gpu.quads.as_entire_binding(),
            gpu.chunks.as_entire_binding(),
            gpu.looks.as_entire_binding(),
            rt.tlas.as_binding(),
            gpu.lamps.as_entire_binding(),
            gpu.lamp_cells.as_entire_binding(),
        ];
        self.gi = Some(gi::Gi::new(&gpu.device, &gpu.shader, shared, gpu.config.format, nw, nh));
        eprintln!("[technoape] the render size is {nw}x{nh}");
    }
}
