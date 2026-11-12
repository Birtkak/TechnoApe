//! Play's settings (ADR 0023, ADR 0028, docs/design/settings.md): every setting as a row of data, the key
//! bindings, the pages of the menu, and the file they are kept in. Only play reads the file, so benchmarks
//! and views keep the defaults, which draw today's picture.

use winit::keyboard::KeyCode;

/// A slider's unit, for its number.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Unit {
    Deg,
    Times,
    Secs,
    Pct,
}

/// What kind of value a setting holds. All are kept as an f64: a choice is its index.
#[derive(Clone, Copy, Debug)]
pub enum Kind {
    /// One of these names; the file holds the name.
    Choice(&'static [&'static str]),
    /// A number on a track from `lo` to `hi` in `step`s; a typed number may go up to `max`.
    Slider { lo: f64, hi: f64, step: f64, max: f64, unit: Unit },
    /// A number the menu steps through `list`; the file may hold any number from `lo` to `hi`.
    Steps { list: &'static [f64], lo: f64, hi: f64 },
}

pub struct Def {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: Kind,
    pub default: f64,
}

const OFF_ON: &[&str] = &["off", "on"];
const VOLUME: Kind = Kind::Slider { lo: 0.0, hi: 100.0, step: 5.0, max: 100.0, unit: Unit::Pct };

macro_rules! settings {
    ($($id:ident $key:literal $label:literal $kind:expr, $def:expr;)*) => {
        /// Every setting; `as usize` is its place in `DEFS` and in `Settings::v`.
        #[derive(Clone, Copy, PartialEq, Debug)]
        pub enum S { $($id),* }
        pub const DEFS: &[Def] = &[$(Def { key: $key, label: $label, kind: $kind, default: $def }),*];
    };
}

settings! {
    Width "width" "Width" Kind::Steps { list: &[], lo: 640.0, hi: 7680.0 }, 1920.0;
    Height "height" "Height" Kind::Steps { list: &[], lo: 360.0, hi: 4320.0 }, 1080.0;
    Monitor "monitor" "Monitor" Kind::Steps { list: &[], lo: 0.0, hi: 16.0 }, 0.0; // 0: the one the window is on
    Display "display" "Display" Kind::Choice(&["windowed", "windowed fullscreen", "fullscreen"]), 1.0;
    FrameCap "frame_cap" "Frame cap" Kind::Choice(&["refresh rate", "half refresh rate", "30", "60", "120", "144", "240", "none"]), 0.0;
    BackCap "background_cap" "Cap in the background" Kind::Choice(&["30", "same"]), 0.0;
    Vsync "vsync" "Wait for the display" Kind::Choice(OFF_ON), 0.0;
    Fov "fov" "Field of view" Kind::Slider { lo: 30.0, hi: 130.0, step: 1.0, max: 130.0, unit: Unit::Deg }, 70.0;
    Brightness "brightness" "Brightness" Kind::Slider { lo: 0.5, hi: 1.5, step: 0.05, max: 1.5, unit: Unit::Times }, 1.0;
    Detail "detail" "Detail distance" Kind::Steps { list: &[3.0, 4.0, 5.0, 6.0, 8.0], lo: 2.0, hi: 12.0 }, 4.0;
    Shadows "shadows" "Sun shadows" Kind::Choice(&["sharp", "soft"]), 1.0;
    Bounce "bounce" "Bounce light" Kind::Choice(OFF_ON), 1.0;
    Reflections "reflections" "Reflections" Kind::Choice(OFF_ON), 1.0;
    Lamps "lamps" "Lamp light" Kind::Choice(&["nearest 16", "nearest 64", "all in range"]), 2.0;
    Particles "particles" "Particles" Kind::Choice(&["few", "normal", "many"]), 1.0;
    Textures "textures" "Block textures" Kind::Choice(&["flat colours", "16 px"]), 1.0;
    // fancy: seen through, light through them; fast by default: fancy's fly-through misses ADR 0004's p99 (ADR 0069)
    Leaves "leaves" "Leaves" Kind::Choice(&["fast", "fancy"]), 0.0;
    Stars "stars" "Stars and galaxy" Kind::Choice(OFF_ON), 1.0;
    // off: the old flat sheet; simple: ripples and the sky in it, no rays of its own; traced: the bed seen
    // through it, the world in it, foam at the shore (ADR 0071)
    Water "water" "Water" Kind::Choice(&["off", "simple", "traced"]), 2.0;
    Sensitivity "sensitivity" "Mouse sensitivity" Kind::Slider { lo: 0.05, hi: 5.0, step: 0.05, max: 10.0, unit: Unit::Times }, 1.0;
    ZoomSens "zoom_sensitivity" "Zoomed sensitivity" Kind::Slider { lo: 0.25, hi: 3.0, step: 0.05, max: 3.0, unit: Unit::Times }, 1.0;
    InvertY "invert_look" "Invert look up and down" Kind::Choice(OFF_ON), 0.0;
    ZoomMode "zoom_mode" "Zoom" Kind::Choice(&["hold", "press once"]), 0.0;
    AutoJump "auto_jump" "Auto jump" Kind::Choice(OFF_ON), 0.0;
    FlyTap "fly_tap" "Double tap to fly" Kind::Slider { lo: 0.15, hi: 0.6, step: 0.05, max: 0.6, unit: Unit::Secs }, 0.3;
    UiScale "interface_scale" "Interface scale" Kind::Steps { list: &[0.75, 1.0, 1.25, 1.5, 2.0], lo: 0.5, hi: 3.0 }, 1.0;
    Contrast "high_contrast" "High contrast interface" Kind::Choice(OFF_ON), 0.0;
    Marks "waypoints_shown" "Waypoints shown" Kind::Choice(OFF_ON), 1.0;
    Volume "volume" "Master volume" VOLUME, 100.0;
    VolWorld "volume_world" "World" VOLUME, 100.0;
    VolSteps "volume_footsteps" "Footsteps" VOLUME, 100.0;
    VolUi "volume_interface" "Interface" VOLUME, 100.0;
    VolMusic "volume_music" "Music" VOLUME, 60.0;
    MuteBack "mute_in_background" "Mute in the background" Kind::Choice(OFF_ON), 1.0;
    SpaceSound "sound_in_space" "Sound in space" Kind::Choice(&["realistic", "softened", "full"]), 0.0;
}

/// A key or a mouse button, by where it is on the board (a KeyCode), so WASD stays under the same fingers on
/// any layout.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Bind {
    Key(KeyCode),
    /// 1 left, 2 right, 3 middle, 4 back, 5 forward.
    Mouse(u8),
}

macro_rules! codes {
    ($($c:ident)*) => { &[$((KeyCode::$c, stringify!($c))),*] };
}

/// The keys an action can be bound to, with the names the file holds.
const CODES: &[(KeyCode, &str)] = codes!(
    KeyA KeyB KeyC KeyD KeyE KeyF KeyG KeyH KeyI KeyJ KeyK KeyL KeyM KeyN KeyO KeyP KeyQ KeyR KeyS KeyT KeyU KeyV
    KeyW KeyX KeyY KeyZ Digit0 Digit1 Digit2 Digit3 Digit4 Digit5 Digit6 Digit7 Digit8 Digit9
    F1 F2 F3 F4 F5 F6 F7 F8 F9 F10 F11 F12 Space Tab Enter Backspace CapsLock
    ShiftLeft ShiftRight ControlLeft ControlRight AltLeft AltRight
    ArrowUp ArrowDown ArrowLeft ArrowRight Insert Delete Home End PageUp PageDown
    Minus Equal BracketLeft BracketRight Backslash Semicolon Quote Backquote Comma Period Slash IntlBackslash
    Numpad0 Numpad1 Numpad2 Numpad3 Numpad4 Numpad5 Numpad6 Numpad7 Numpad8 Numpad9
    NumpadAdd NumpadSubtract NumpadMultiply NumpadDivide NumpadDecimal NumpadEnter
);

impl Bind {
    /// The name the file holds.
    pub fn name(self) -> String {
        match self {
            Bind::Key(c) => CODES.iter().find(|x| x.0 == c).map_or("?".into(), |x| x.1.into()),
            Bind::Mouse(n) => format!("Mouse{n}"),
        }
    }
    pub fn parse(s: &str) -> Option<Bind> {
        if let Some(n) = s.strip_prefix("Mouse") {
            return n.parse().ok().filter(|n| (1..=5).contains(n)).map(Bind::Mouse);
        }
        CODES.iter().find(|x| x.1 == s).map(|x| Bind::Key(x.0))
    }
    /// The name the menu shows.
    pub fn label(self) -> String {
        match self {
            Bind::Mouse(1) => "Left click".into(),
            Bind::Mouse(2) => "Right click".into(),
            Bind::Mouse(3) => "Middle click".into(),
            Bind::Mouse(n) => format!("Mouse {n}"),
            Bind::Key(_) => {
                let n = self.name();
                n.strip_prefix("Key").or(n.strip_prefix("Digit")).map_or(n.clone(), |s| s.to_string())
            }
        }
    }
}

pub struct ActDef {
    pub key: &'static str,
    pub label: &'static str,
    pub binds: [Option<Bind>; 2],
    /// The inputs (`App::act`) its press and its release make; empty for one that is only held.
    pub press: &'static str,
    pub release: &'static str,
}

const fn k(c: KeyCode) -> [Option<Bind>; 2] {
    [Some(Bind::Key(c)), None]
}

macro_rules! acts {
    ($($id:ident $key:literal $label:literal $binds:expr, $press:literal $release:literal;)*) => {
        /// Everything the player can do with a key or a button; `as usize` is its place in `ACTS`.
        #[derive(Clone, Copy, PartialEq, Debug)]
        pub enum Act { $($id),* }
        pub const ACTS: &[ActDef] = &[$(ActDef { key: $key, label: $label, binds: $binds, press: $press, release: $release }),*];
        pub const ALL_ACTS: &[Act] = &[$(Act::$id),*];
    };
}

acts! {
    Forward "forward" "Forward" k(KeyCode::KeyW), "" "";
    Back "back" "Back" k(KeyCode::KeyS), "" "";
    Left "left" "Left" k(KeyCode::KeyA), "" "";
    Right "right" "Right" k(KeyCode::KeyD), "" "";
    Jump "jump" "Jump, swim up, fly up" k(KeyCode::Space), "space" "";
    Down "down" "Fly down" [Some(Bind::Key(KeyCode::ShiftLeft)), Some(Bind::Key(KeyCode::ShiftRight))], "" "";
    RollLeft "roll_left" "Roll left (space)" k(KeyCode::KeyQ), "" "";
    RollRight "roll_right" "Roll right (space)" k(KeyCode::KeyE), "" "";
    Zoom "zoom" "Zoom" k(KeyCode::KeyC), "zoom" "unzoom";
    Dig "dig" "Dig, scoop" [Some(Bind::Mouse(1)), None], "L" "l";
    Place "place" "Place, pour" [Some(Bind::Mouse(2)), None], "R" "r";
    Hammer "hammer" "Build hammer" k(KeyCode::KeyH), "h" "";
    Shape "shape" "Hammer shape" k(KeyCode::KeyB), "b" "";
    Bucket "bucket" "Bucket" k(KeyCode::KeyV), "v" "";
    Chat "chat" "Chat" k(KeyCode::KeyT), "t" "";
    Command "command" "Command" k(KeyCode::Slash), "/" "";
    Overlay "overlay" "Numbers overlay" k(KeyCode::F3), "f3" "";
    Bug "bug" "Bug report" k(KeyCode::F9), "f9" "";
    WarpUp "warp_up" "Time warp faster" k(KeyCode::Period), "warp+" "";
    WarpDown "warp_down" "Time warp slower" k(KeyCode::Comma), "warp-" "";
    Waypoint "waypoint" "Drop a waypoint" k(KeyCode::KeyN), "waypoint" "";
    Marks "waypoint_overlay" "Show or hide waypoints" k(KeyCode::KeyK), "waypoints" "";
    Playtest "playtest" "Playtest menu" k(KeyCode::F10), "f10" "";
    Slot1 "slot1" "Hotbar 1" k(KeyCode::Digit1), "1" "";
    Slot2 "slot2" "Hotbar 2" k(KeyCode::Digit2), "2" "";
    Slot3 "slot3" "Hotbar 3" k(KeyCode::Digit3), "3" "";
    Slot4 "slot4" "Hotbar 4" k(KeyCode::Digit4), "4" "";
    Slot5 "slot5" "Hotbar 5" k(KeyCode::Digit5), "5" "";
    Slot6 "slot6" "Hotbar 6" k(KeyCode::Digit6), "6" "";
    Slot7 "slot7" "Hotbar 7" k(KeyCode::Digit7), "7" "";
    Slot8 "slot8" "Hotbar 8" k(KeyCode::Digit8), "8" "";
    Slot9 "slot9" "Hotbar 9" k(KeyCode::Digit9), "9" "";
}

/// A row of a settings page.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Row {
    Set(S),
    /// Width and height together, from the sizes the screen allows.
    Resolution,
    /// Low to Ultra: sets the graphics rows, and reads "Custom" when they match none.
    Preset,
    Key(Act),
    /// All bindings back to the defaults.
    KeyDefaults,
    /// The keys page's search: what is typed keeps the actions with a word or a key that match.
    Search,
    /// A heading over a group of key rows; pressing it does nothing.
    Group(&'static str),
}

/// The keys page's groups, in order (bugs/2026-11-11_13-13-42); every action is in one.
const KEY_GROUPS: &[(&str, &[Act])] = &[
    ("Moving", &[Act::Forward, Act::Back, Act::Left, Act::Right, Act::Jump, Act::Down, Act::RollLeft, Act::RollRight]),
    ("Hands and tools", &[Act::Dig, Act::Place, Act::Hammer, Act::Shape, Act::Bucket]),
    ("Hotbar", &[Act::Slot1, Act::Slot2, Act::Slot3, Act::Slot4, Act::Slot5, Act::Slot6, Act::Slot7, Act::Slot8, Act::Slot9]),
    ("Looking and finding", &[Act::Zoom, Act::Waypoint, Act::Marks, Act::Overlay]),
    ("Chat, time and testing", &[Act::Chat, Act::Command, Act::WarpUp, Act::WarpDown, Act::Bug, Act::Playtest]),
];

/// The settings' pages in the menu's order, each with its hint.
pub const PAGES: &[(&str, &str)] = &[
    ("Display", "A change that could lose the picture goes back unless you keep it."),
    ("Graphics", "A preset sets the rows under it. High is what the game is tuned for."),
    ("Controls", "Drag a slider, or click its number to type one."),
    ("Keys", "Left click: first key. Right click: second. Backspace clears, Esc cancels."),
    ("Sound", "In space, true carries only what you touch, through your suit."),
    ("Accessibility", "Some rows are also on other pages; they are the same setting."),
];

pub fn page_rows(page: usize) -> Vec<Row> {
    use Row::Set;
    match page {
        0 => vec![Row::Resolution, Set(S::Monitor), Set(S::Display), Set(S::FrameCap), Set(S::BackCap), Set(S::Vsync), Set(S::Fov), Set(S::Brightness)],
        1 => vec![Row::Preset, Set(S::Detail), Set(S::Shadows), Set(S::Bounce), Set(S::Reflections), Set(S::Lamps), Set(S::Particles), Set(S::Textures), Set(S::Leaves), Set(S::Water), Set(S::Stars)],
        2 => vec![Set(S::Sensitivity), Set(S::ZoomSens), Set(S::InvertY), Set(S::ZoomMode), Set(S::AutoJump), Set(S::FlyTap)],
        3 => Settings::default().key_rows(""),
        4 => vec![Set(S::Volume), Set(S::VolWorld), Set(S::VolSteps), Set(S::VolUi), Set(S::VolMusic), Set(S::MuteBack), Set(S::SpaceSound)],
        _ => vec![Set(S::UiScale), Set(S::Contrast), Set(S::Marks), Set(S::ZoomMode), Set(S::AutoJump), Set(S::Textures), Set(S::Brightness)],
    }
}

pub const PRESETS: [&str; 4] = ["Low", "Medium", "High", "Ultra"];
/// What a preset sets, and each preset's values (a choice by its index).
const PRESET_ROWS: [S; 5] = [S::Detail, S::Shadows, S::Bounce, S::Reflections, S::Lamps];
const PRESET_VALUES: [[f64; 5]; 4] = [[3.0, 0.0, 0.0, 0.0, 0.0], [4.0, 1.0, 1.0, 0.0, 1.0], [4.0, 1.0, 1.0, 1.0, 2.0], [6.0, 1.0, 1.0, 1.0, 2.0]];

/// The settings that can leave the player without a picture: a change to one is kept only when confirmed.
pub const RISKY: [S; 4] = [S::Width, S::Height, S::Monitor, S::Display];

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    v: Vec<f64>,
    pub binds: Vec<[Option<Bind>; 2]>,
    /// Lines of the file this build does not know: written back as they were, so a newer build's settings
    /// survive an older one.
    extra: Vec<String>,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings { v: DEFS.iter().map(|d| d.default).collect(), binds: ACTS.iter().map(|a| a.binds).collect(), extra: Vec::new() }
    }
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

impl Settings {
    pub fn get(&self, s: S) -> f64 {
        self.v[s as usize]
    }
    /// A choice's index, or an on/off row's "on".
    pub fn at(&self, s: S) -> usize {
        self.v[s as usize] as usize
    }
    pub fn on(&self, s: S) -> bool {
        self.v[s as usize] != 0.0
    }
    /// A choice's name.
    pub fn name(&self, s: S) -> &'static str {
        match DEFS[s as usize].kind {
            Kind::Choice(c) => c[self.at(s).min(c.len() - 1)],
            _ => "",
        }
    }
    /// Sets `s`, kept to what its kind allows.
    pub fn set(&mut self, s: S, v: f64) {
        self.v[s as usize] = match DEFS[s as usize].kind {
            Kind::Choice(c) => v.round().clamp(0.0, c.len() as f64 - 1.0),
            Kind::Slider { lo, step, max, .. } => round2((v.clamp(lo, max) / step).round() * step),
            Kind::Steps { lo, hi, .. } => v.clamp(lo, hi),
        };
    }
    /// The next value of a row that is clicked through (the one before when `back`), around the end.
    pub fn cycle(&mut self, s: S, back: bool) {
        let n = match DEFS[s as usize].kind {
            Kind::Choice(c) => {
                let (i, n) = (self.at(s), c.len());
                let next = if back { i + n - 1 } else { i + 1 };
                (next % n) as f64
            }
            Kind::Steps { list, .. } if !list.is_empty() => {
                let i = list.iter().position(|x| *x == self.get(s)).unwrap_or(0);
                let next = if back { i + list.len() - 1 } else { i + 1 };
                list[next % list.len()]
            }
            _ => return,
        };
        self.set(s, n);
    }
    /// A slider row's place on its track (0 to 1) and its number as text.
    pub fn slider(&self, s: S) -> Option<(f32, String)> {
        let Kind::Slider { lo, hi, unit, .. } = DEFS[s as usize].kind else { return None };
        let v = self.get(s);
        let text = match unit {
            Unit::Deg => format!("{v:.0}"),
            Unit::Times => format!("{v:.2}x"),
            Unit::Secs => format!("{v:.2}s"),
            Unit::Pct => format!("{v:.0}%"),
        };
        Some((((v - lo) / (hi - lo)) as f32, text))
    }
    /// Sets a slider from a place on its track.
    pub fn slide(&mut self, s: S, f: f64) {
        if let Kind::Slider { lo, hi, .. } = DEFS[s as usize].kind {
            self.set(s, lo + f.clamp(0.0, 1.0) * (hi - lo));
        }
    }
    /// A row's text in the menu, for the rows that are not key bindings.
    pub fn row_text(&self, s: S) -> String {
        let d = &DEFS[s as usize];
        match d.kind {
            Kind::Choice(_) => format!("{}: {}", d.label, self.name(s)),
            Kind::Slider { .. } => d.label.to_string(),
            Kind::Steps { .. } if s == S::UiScale => format!("{}: {:.0}%", d.label, self.get(s) * 100.0),
            Kind::Steps { .. } => format!("{}: {:.0}", d.label, self.get(s)),
        }
    }

    /// The preset the graphics rows match: 0 to 3, or None for "Custom".
    pub fn preset(&self) -> Option<usize> {
        PRESET_VALUES.iter().position(|p| PRESET_ROWS.iter().zip(p).all(|(s, v)| self.get(*s) == *v))
    }
    pub fn set_preset(&mut self, p: usize) {
        for (s, v) in PRESET_ROWS.iter().zip(PRESET_VALUES[p % 4]) {
            self.set(*s, v);
        }
    }

    /// The actions bound to `b`.
    pub fn acts_of(&self, b: Bind) -> impl Iterator<Item = Act> + '_ {
        ALL_ACTS.iter().copied().filter(move |a| self.binds[*a as usize].contains(&Some(b)))
    }
    /// A key row's text: the action and its two keys in columns (the font is fixed-width), and the other
    /// action a key of its is also bound to.
    pub fn key_text(&self, a: Act) -> String {
        let b = self.binds[a as usize];
        let name = |x: Option<Bind>| x.map_or("-".to_string(), |x| x.label());
        let clash = b.iter().flatten().find_map(|x| self.acts_of(*x).find(|o| *o != a));
        let also = clash.map_or(String::new(), |o| format!("  (also {})", ACTS[o as usize].label));
        format!("{:<23}{:<13}{}{also}", ACTS[a as usize].label, name(b[0]), name(b[1]))
    }
    /// A key of `a` is also another action's: the menu shows its row in red.
    pub fn clashes(&self, a: Act) -> bool {
        self.binds[a as usize].iter().flatten().any(|x| self.acts_of(*x).any(|o| o != a))
    }
    /// The keys page: the search, each group's heading and the actions under it that match `query` (a word of
    /// the action that starts with it, or a key named by it; all when it is empty), the defaults.
    pub fn key_rows(&self, query: &str) -> Vec<Row> {
        let q = query.trim().to_lowercase();
        let hit = |a: Act| {
            let words = ACTS[a as usize].label.to_lowercase();
            q.is_empty() || words.split(|c: char| !c.is_alphanumeric()).any(|w| w.starts_with(&q)) || self.binds[a as usize].iter().flatten().any(|b| b.label().to_lowercase() == q)
        };
        let mut rows = vec![Row::Search];
        for (name, acts) in KEY_GROUPS {
            let found: Vec<Row> = acts.iter().filter(|a| hit(**a)).map(|a| Row::Key(*a)).collect();
            if !found.is_empty() {
                rows.push(Row::Group(name));
                rows.extend(found);
            }
        }
        rows.push(Row::KeyDefaults);
        rows
    }
    /// A key or button of `a` is down.
    pub fn held(&self, keys: &std::collections::HashSet<Bind>, a: Act) -> bool {
        self.binds[a as usize].iter().flatten().any(|b| keys.contains(b))
    }
    pub fn default_binds(&mut self) {
        self.binds = ACTS.iter().map(|a| a.binds).collect();
    }

    /// Reads the file's text: a bad value keeps that row's default, an unknown line is kept as it is.
    pub fn parse(text: &str) -> Settings {
        let mut s = Settings::default();
        for line in text.lines() {
            let Some((key, v)) = line.split_once('=').map(|(a, b)| (a.trim(), b.trim())) else {
                if !line.trim().is_empty() && !line.starts_with('#') {
                    s.extra.push(line.to_string());
                }
                continue;
            };
            if let Some(i) = DEFS.iter().position(|d| d.key == key) {
                let id = &DEFS[i];
                let n = match id.kind {
                    Kind::Choice(c) => c.iter().position(|x| *x == v).map(|p| p as f64),
                    _ => v.parse().ok().filter(|x: &f64| x.is_finite()),
                };
                if let Some(n) = n {
                    s.v[i] = match id.kind {
                        Kind::Slider { lo, max, .. } => n.clamp(lo, max),
                        Kind::Steps { lo, hi, .. } => n.clamp(lo, hi),
                        Kind::Choice(_) => n,
                    };
                }
            } else if let Some(i) = key.strip_prefix("key.").and_then(|a| ACTS.iter().position(|d| d.key == a)) {
                let mut it = v.split(',').map(|x| Bind::parse(x.trim()));
                s.binds[i] = [it.next().flatten(), it.next().flatten()];
            } else if key != "version" {
                s.extra.push(line.to_string());
            }
        }
        s
    }

    /// The file's text: key=value lines, a group per page.
    pub fn text(&self) -> String {
        let mut out = String::from("version=1\n");
        for (i, d) in DEFS.iter().enumerate() {
            match d.kind {
                Kind::Choice(c) => out += &format!("{}={}\n", d.key, c[(self.v[i] as usize).min(c.len() - 1)]),
                _ => out += &format!("{}={}\n", d.key, self.v[i]),
            }
        }
        out += "# keys: by their place on the board, two per action\n";
        for (a, b) in ACTS.iter().zip(&self.binds) {
            // each key in its own place, so a second key with no first reads back as the second
            let names = b.map(|x| x.map_or(String::new(), |x| x.name())).join(",");
            out += &format!("key.{}={}\n", a.key, names.trim_end_matches(','));
        }
        for l in &self.extra {
            out += &format!("{l}\n");
        }
        out
    }

    /// Every setting and binding as one JSON line (`dump=1`), so a test can read what a file means.
    pub fn json(&self) -> String {
        let mut parts: Vec<String> = DEFS
            .iter()
            .enumerate()
            .map(|(i, d)| match d.kind {
                Kind::Choice(c) => format!("\"{}\":\"{}\"", d.key, c[(self.v[i] as usize).min(c.len() - 1)]),
                _ => format!("\"{}\":{}", d.key, self.v[i]),
            })
            .collect();
        for (a, b) in ACTS.iter().zip(&self.binds) {
            let names: Vec<String> = b.iter().flatten().map(|x| format!("\"{}\"", x.name())).collect();
            parts.push(format!("\"key.{}\":[{}]", a.key, names.join(",")));
        }
        format!("{{{}}}", parts.join(","))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_reads_back_and_keeps_what_it_does_not_know() {
        let mut s = Settings::default();
        s.set(S::Fov, 90.0);
        s.set(S::Display, 2.0);
        s.set(S::Detail, 6.0);
        s.binds[Act::Forward as usize] = [Some(Bind::Key(KeyCode::ArrowUp)), Some(Bind::Mouse(4))];
        s.binds[Act::Zoom as usize] = [None, None];
        // a second key with no first stays the second
        s.binds[Act::Jump as usize] = [None, Some(Bind::Key(KeyCode::Enter))];
        assert_eq!(Settings::parse(&s.text()), s);
        // a newer build's row and a stray line survive this build
        let t = Settings::parse("fov=95\nhdr=on\nsomething odd\nversion=7\n");
        assert_eq!(t.get(S::Fov), 95.0);
        assert!(t.text().contains("hdr=on\n") && t.text().contains("something odd\n"));
        assert_eq!(Settings::parse(&t.text()), t);
    }

    #[test]
    fn nonsense_and_extremes_fall_back_row_by_row() {
        let s = Settings::parse("fov=nonsense\nwidth=99999\ndisplay=sideways\ndetail=-4\nbrightness=NaN\nkey.jump=Banana,Mouse9\nfov\n");
        let d = Settings::default();
        assert_eq!((s.get(S::Fov), s.get(S::Width), s.at(S::Display)), (d.get(S::Fov), 7680.0, d.at(S::Display)));
        assert_eq!((s.get(S::Detail), s.get(S::Brightness)), (2.0, 1.0));
        assert_eq!(s.binds[Act::Jump as usize], [None, None]);
        // every row at its least and its most, by the rows' own table
        for (lowest, i) in [true, false].into_iter().flat_map(|l| (0..DEFS.len()).map(move |i| (l, i))) {
            let mut s = Settings::default();
            s.v[i] = if lowest { -1e9 } else { 1e9 };
            let text: String = DEFS.iter().enumerate().map(|(j, d)| format!("{}={}\n", d.key, s.v[j])).collect();
            let back = Settings::parse(&text);
            match DEFS[i].kind {
                Kind::Choice(_) => assert_eq!(back.v[i], DEFS[i].default, "{}", DEFS[i].key),
                Kind::Slider { lo, max, .. } => assert_eq!(back.v[i], if lowest { lo } else { max }, "{}", DEFS[i].key),
                Kind::Steps { lo, hi, .. } => assert_eq!(back.v[i], if lowest { lo } else { hi }, "{}", DEFS[i].key),
            }
        }
    }

    #[test]
    fn rows_cycle_slide_and_presets_match() {
        let mut s = Settings::default();
        assert_eq!(s.preset(), Some(2), "the defaults are High");
        s.cycle(S::Detail, false);
        assert_eq!((s.get(S::Detail), s.preset()), (5.0, None));
        s.set_preset(0);
        assert_eq!((s.preset(), s.name(S::Bounce), s.name(S::Lamps)), (Some(0), "off", "nearest 16"));
        s.cycle(S::Display, false);
        s.cycle(S::Display, false);
        assert_eq!(s.name(S::Display), "windowed");
        s.cycle(S::Display, true);
        assert_eq!(s.name(S::Display), "fullscreen");
        s.set(S::Fov, 83.4);
        s.set(S::Sensitivity, 0.333);
        assert_eq!((s.get(S::Fov), s.get(S::Sensitivity), s.slider(S::Sensitivity).unwrap().1), (83.0, 0.35, "0.35x".to_string()));
        s.slide(S::Fov, 2.0);
        assert_eq!((s.get(S::Fov), s.slider(S::Display)), (130.0, None));
    }

    #[test]
    fn every_key_has_a_name_and_every_action_a_row() {
        for (c, n) in CODES {
            assert_eq!(Bind::parse(n), Some(Bind::Key(*c)));
        }
        let keys = page_rows(3);
        for a in ALL_ACTS {
            assert!(keys.contains(&Row::Key(*a)), "{a:?} has no row");
            assert!(ACTS[*a as usize].binds.iter().flatten().all(|b| b.name() != "?"), "{a:?}: a default key without a name");
        }
        // a clash is named on the row
        let mut s = Settings::default();
        s.binds[Act::Back as usize][0] = Some(Bind::Key(KeyCode::KeyW));
        assert!(s.key_text(Act::Back).contains("(also Forward)"), "{}", s.key_text(Act::Back));
        assert_eq!(s.acts_of(Bind::Key(KeyCode::KeyW)).count(), 2);
        // the row reads as a table: the action, then each key in its own column
        let s = Settings::default();
        assert_eq!(s.key_text(Act::Zoom).find('C'), s.key_text(Act::Forward).find('W'), "{}|{}", s.key_text(Act::Zoom), s.key_text(Act::Forward));
        // every setting is on a page, but for width and height, which the resolution row holds
        let on_pages: Vec<Row> = (0..PAGES.len()).flat_map(page_rows).collect();
        for (i, d) in DEFS.iter().enumerate() {
            assert!(i < 2 || on_pages.iter().any(|r| matches!(r, Row::Set(s) if *s as usize == i)), "{} is on no page", d.key);
        }
    }

    #[test]
    fn the_keys_page_groups_finds_and_marks_clashes() {
        // bugs/2026-11-11_13-13-42: groups under headings, a search by action or key, clashes marked
        let mut s = Settings::default();
        let rows = s.key_rows("");
        assert_eq!((rows[0], rows.last().copied()), (Row::Search, Some(Row::KeyDefaults)));
        assert!(matches!(rows[1], Row::Group(_)));
        for a in ALL_ACTS {
            assert_eq!(rows.iter().filter(|r| **r == Row::Key(*a)).count(), 1, "{a:?} once, under a heading");
        }
        let keys = |rows: Vec<Row>| rows.into_iter().filter_map(|r| if let Row::Key(a) = r { Some(a) } else { None }).collect::<Vec<_>>();
        assert_eq!(keys(s.key_rows("zoo")), [Act::Zoom], "by a word of the action");
        assert_eq!(keys(s.key_rows("F10")), [Act::Playtest], "by its key");
        assert_eq!(keys(s.key_rows("k")), [Act::Marks]);
        assert!(s.key_rows("nothing like it").len() == 2, "the search and the defaults stay");
        assert!(ALL_ACTS.iter().all(|a| !s.clashes(*a)), "the default keys do not clash");
        s.binds[Act::Back as usize][0] = Some(Bind::Key(KeyCode::KeyQ));
        assert!(s.clashes(Act::Back) && s.clashes(Act::RollLeft) && !s.clashes(Act::Forward));
        assert_eq!(keys(s.key_rows("q")), [Act::Back, Act::RollLeft], "a key's search shows who shares it");
    }
}
