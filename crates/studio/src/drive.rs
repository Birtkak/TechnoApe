//! Drives the window headless with raw input (`shot=... input="..."`): every step becomes egui events fed
//! through `RawInput.events`, a frame run between them, so hit-testing, focus and drags are egui's own.
//! Steps, `;` apart (positions in logical points, or `@name[:fx,fy]` a rect the pages [`mark`]ed last frame,
//! at that fraction of it, the centre by default; `rects=1` lists them):
//! `move T`, `down [left|right|middle]`, `up [..]`, `click [..] [T]`, `drag T T [..]`, `wheel dy`,
//! `key [Ctrl+][Shift+][Alt+]Name`, `mods ctrl+alt|none` (held for the clicks after), `text words`,
//! `wait [n]`, `close` (the window's close button).
use eframe::egui::{self, Event, Key, Modifiers, PointerButton, Pos2, Rect};
use std::collections::BTreeMap;

#[derive(Clone, Default)]
pub struct Rects(pub BTreeMap<String, Rect>);

fn id() -> egui::Id {
    egui::Id::unique("drive rects")
}

/// A page's main area, by name, for `@name` targets and `rects=1`.
pub fn mark(ctx: &egui::Context, name: impl Into<String>, r: Rect) {
    // a script splits on spaces: `file:acacia_leaves`
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Rects>(id()).0.insert(name.into().replace(' ', "_"), r));
}

/// At the start of a frame: only what this frame marks is there.
pub fn clear(ctx: &egui::Context) {
    ctx.data_mut(|d| d.remove::<Rects>(id()));
}

pub fn rects(ctx: &egui::Context) -> Rects {
    ctx.data(|d| d.get_temp::<Rects>(id())).unwrap_or_default()
}

pub fn rects_json(r: &Rects) -> String {
    let v: Vec<String> = r.0.iter().map(|(k, r)| format!("{}:[{:.0},{:.0},{:.0},{:.0}]", crate::js(k), r.min.x, r.min.y, r.max.x, r.max.y)).collect();
    format!("{{{}}}", v.join(","))
}

/// One input frame: its events and whether the window's close button was pressed.
#[derive(Default)]
pub struct Frame {
    pub events: Vec<Event>,
    pub close: bool,
}

#[derive(Default)]
pub struct Driver {
    pub pos: Pos2,
    pub mods: Modifiers,
}

fn button(t: &str) -> Option<PointerButton> {
    match t {
        "left" => Some(PointerButton::Primary),
        "right" => Some(PointerButton::Secondary),
        "middle" => Some(PointerButton::Middle),
        _ => None,
    }
}

fn mods(s: &str) -> Result<(Modifiers, &str), String> {
    let mut m = Modifiers::NONE;
    let mut rest = s;
    while let Some((head, tail)) = rest.split_once('+').filter(|(h, t)| !t.is_empty() && !h.is_empty()) {
        match head.to_ascii_lowercase().as_str() {
            "ctrl" => {
                m.ctrl = true;
                m.command = true;
            }
            "shift" => m.shift = true,
            "alt" => m.alt = true,
            _ => break,
        }
        rest = tail;
    }
    Ok((m, rest))
}

impl Driver {
    /// A target: `x y` or `@name[:fx,fy]`.
    fn target<'a>(&self, t: &mut impl Iterator<Item = &'a str>, rects: &Rects) -> Result<Pos2, String> {
        let a = t.next().ok_or("a position is missing")?;
        if let Some(name) = a.strip_prefix('@') {
            // names have colons too (`tab:Models`): only a last part of two numbers is a fraction
            let frac = |f: &str| f.split_once(',').and_then(|(x, y)| Some((x.parse::<f32>().ok()?, y.parse::<f32>().ok()?)));
            let (name, f) = name.rsplit_once(':').and_then(|(n, f)| Some((n, frac(f)?))).unwrap_or((name, (0.5, 0.5)));
            let r = rects.0.get(name).ok_or(format!("no rect '{name}' (rects=1 lists them)"))?;
            return Ok(r.min + r.size() * egui::vec2(f.0, f.1));
        }
        let x: f32 = a.parse().map_err(|_| format!("'{a}' is no position"))?;
        let y: f32 = t.next().and_then(|y| y.parse().ok()).ok_or("a position needs x and y")?;
        Ok(Pos2::new(x, y))
    }

    fn press(&self, b: PointerButton, pressed: bool) -> Frame {
        Frame { events: vec![Event::PointerButton { pos: self.pos, button: b, pressed, modifiers: self.mods }], close: false }
    }
    fn moved(&mut self, p: Pos2) -> Frame {
        self.pos = p;
        Frame { events: vec![Event::PointerMoved(p)], close: false }
    }

    /// The frames of one step.
    pub fn step(&mut self, step: &str, rects: &Rects) -> Result<Vec<Frame>, String> {
        let step = step.trim();
        let (verb, rest) = step.split_once(' ').unwrap_or((step, ""));
        let mut t = rest.split_whitespace().peekable();
        let btn = |t: &mut std::iter::Peekable<std::str::SplitWhitespace>| match t.peek().and_then(|s| button(s)) {
            Some(b) => {
                t.next();
                b
            }
            None => PointerButton::Primary,
        };
        Ok(match verb {
            "" => vec![],
            "move" => {
                let p = self.target(&mut t, rects)?;
                vec![self.moved(p)]
            }
            "down" | "up" => {
                let b = btn(&mut t);
                vec![self.press(b, verb == "down")]
            }
            "click" => {
                let b = btn(&mut t);
                let mut v = vec![];
                if t.peek().is_some() {
                    let p = self.target(&mut t, rects)?;
                    v.push(self.moved(p));
                }
                v.push(self.press(b, true));
                v.push(self.press(b, false));
                v
            }
            "drag" => {
                let a = self.target(&mut t, rects)?;
                let z = self.target(&mut t, rects)?;
                let b = btn(&mut t);
                let mut v = vec![self.moved(a), self.press(b, true)];
                for i in 1..=8 {
                    v.push(self.moved(a + (z - a) * (i as f32 / 8.0)));
                }
                v.push(self.press(b, false));
                v
            }
            "wheel" => {
                let dy: f32 = rest.trim().parse().map_err(|_| format!("wheel '{rest}'"))?;
                vec![Frame {
                    events: vec![Event::MouseWheel { unit: egui::MouseWheelUnit::Line, delta: egui::vec2(0.0, dy), phase: egui::TouchPhase::Move, source: egui::MouseWheelSource::Wheel, modifiers: self.mods }],
                    close: false,
                }]
            }
            "mods" => {
                self.mods = if rest.trim() == "none" { Modifiers::NONE } else { mods(&format!("{}+x", rest.trim()))?.0 };
                vec![Frame { events: vec![Event::ModifiersChanged(self.mods)], close: false }]
            }
            "key" => {
                let (m, name) = mods(rest.trim())?;
                let key = Key::from_name(name).ok_or(format!("no key '{name}'"))?;
                let ev = |pressed| Event::Key { key, physical_key: Some(key), pressed, repeat: false, modifiers: m };
                let mut down = vec![Event::ModifiersChanged(m), ev(true)];
                // a key that types: its text as winit sends it
                if m.is_none() || m == Modifiers::SHIFT {
                    if key == Key::Space {
                        down.push(Event::Text(" ".into()));
                    } else if name.chars().count() == 1 {
                        down.push(Event::Text(if m.shift { name.to_uppercase() } else { name.to_lowercase() }));
                    }
                }
                vec![Frame { events: down, close: false }, Frame { events: vec![ev(false), Event::ModifiersChanged(self.mods)], close: false }]
            }
            "text" => vec![Frame { events: vec![Event::Text(rest.into())], close: false }],
            "wait" => (0..rest.trim().parse::<usize>().unwrap_or(1)).map(|_| Frame::default()).collect(),
            "close" => vec![Frame { events: vec![], close: true }],
            v => return Err(format!("no input step '{v}'")),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_parse() {
        let mut d = Driver::default();
        let r = Rects(BTreeMap::from([("canvas".to_string(), Rect::from_min_max(Pos2::new(100.0, 100.0), Pos2::new(200.0, 300.0)))]));
        assert_eq!(d.step("drag 0 0 @canvas:0.5,0.5 right", &r).unwrap().len(), 11);
        assert_eq!(d.pos, Pos2::new(150.0, 200.0));
        let k = d.step("key Ctrl+S", &r).unwrap();
        assert!(matches!(k[0].events[1], Event::Key { key: Key::S, pressed: true, modifiers, .. } if modifiers.command));
        assert!(d.step("click @nothing", &r).is_err());
        assert!(d.step("fly", &r).is_err());
    }
}
