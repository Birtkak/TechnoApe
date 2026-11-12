//! Voxel models (docs/design/studio-tools.md "Models", ADR 0047): `data/models/<name>.toml` read and written,
//! each part's transform at a time in a clip, and each part meshed. No GPU, so the Studio calls the same code.
use crate::palette::Palette;
use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};

/// One voxel's edge in metres: the fine cell's size (ADR 0033), 8 to a block's edge.
pub const VOXEL: f32 = 0.125;
/// A part's box is 1..MAX_BOX voxels a side.
pub const MAX_BOX: u32 = 32;
/// The letters of the voxel rows, in the order of `colours`; '.' is empty.
pub const LETTERS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Model {
    /// Palette names; a voxel's letter picks one.
    pub colours: Vec<String>,
    #[serde(default, rename = "part", skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<Part>,
    #[serde(default, rename = "point", skip_serializing_if = "Vec::is_empty")]
    pub points: Vec<Point>,
    #[serde(default, rename = "clip", skip_serializing_if = "Vec::is_empty")]
    pub clips: Vec<Clip>,
    /// `colours`' bytes from the palette (filled by `parse`).
    #[serde(skip)]
    pub rgb: Vec<[u8; 3]>,
    /// Each part's parent's place in `parts` (filled by `parse`).
    #[serde(skip)]
    pub up: Vec<Option<usize>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Part {
    pub name: String,
    /// "" for a root, else another part's name.
    #[serde(default)]
    pub parent: String,
    /// Where the box's corner sits in the parent's voxels (a root: in the cell).
    #[serde(default)]
    pub at: [i32; 3],
    /// The point it turns about, in its own voxels.
    #[serde(default)]
    pub pivot: [f32; 3],
    /// x, y (up), z in voxels.
    #[serde(rename = "box")]
    pub size: [u32; 3],
    pub voxels: String,
    /// 0 empty, else a colour's place + 1, at x + size.x * (z + size.z * y) (filled by `parse`).
    #[serde(skip)]
    pub cells: Vec<u8>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Point {
    pub name: String,
    pub part: String,
    pub at: [f32; 3],
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Clip {
    pub name: String,
    /// Seconds.
    pub length: f32,
    #[serde(default, rename = "loop")]
    pub looping: bool,
    #[serde(default)]
    pub key: Vec<Key>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Key {
    pub part: String,
    pub t: f32,
    /// In voxels.
    #[serde(default, rename = "move")]
    pub shift: [f32; 3],
    /// Degrees about x, y, z at the pivot (x first).
    #[serde(default)]
    pub turn: [f32; 3],
    /// How it leaves this key.
    #[serde(default)]
    pub ease: Ease,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Ease {
    #[default]
    Linear,
    Smooth,
    Step,
}

/// A model's life (docs/design/studio-tools.md "A model's life: states", ADR 0050): its `[[state]]` tables and
/// its clips' timed `event` lists, read from the same file (`parse_life`) and kept beside the model, so `Model`
/// and `Clip` keep the fields their callers build them with. `write_life` puts them back into the file's text.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Life {
    pub states: Vec<State>,
    /// Each clip's events, by the clip's name.
    pub events: Vec<(String, Vec<Event>)>,
}

/// What a model does in a state: its clip, its lasting effects at its points, a looping sound, a light.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<StateEffect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light: Option<Light>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct StateEffect {
    pub effect: String,
    pub point: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct Light {
    pub point: String,
    /// A palette name.
    pub colour: String,
    #[serde(default = "one")]
    pub strength: f32,
    /// `colour` from the palette, 0..1 (filled by `parse_life`).
    #[serde(skip)]
    pub rgb: [f32; 3],
}

fn one() -> f32 {
    1.0
}

/// A moment of a clip: a sound, an effect, or both, at a point (none: the model's foot).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub t: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub point: Option<String>,
}

/// One merged face of a part: its min corner in the part's voxels, its direction (0..6 = +x,-x,+y,-y,+z,-z),
/// its extent along the axes (a+1)%3 and (a+2)%3 of its axis a, and its colour's place in `colours`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Quad {
    pub at: [u8; 3],
    pub dir: u8,
    pub w: u8,
    pub h: u8,
    pub colour: u8,
}

/// The voxel rows of a part: layers from the bottom up, a blank line between, one row per z, one letter per x.
pub fn decode(size: [u32; 3], text: &str, colours: usize) -> Result<Vec<u8>, String> {
    let [sx, sy, sz] = size.map(|v| v as usize);
    let mut layers: Vec<Vec<&str>> = vec![Vec::new()];
    for line in text.lines().map(str::trim_end) {
        if line.is_empty() {
            if !layers.last().unwrap().is_empty() {
                layers.push(Vec::new());
            }
        } else {
            layers.last_mut().unwrap().push(line);
        }
    }
    if layers.last().is_some_and(|l| l.is_empty()) {
        layers.pop();
    }
    if layers.len() != sy {
        return Err(format!("{} layers, the box is {sy} high", layers.len()));
    }
    let mut cells = vec![0u8; sx * sy * sz];
    for (y, rows) in layers.iter().enumerate() {
        if rows.len() != sz {
            return Err(format!("layer {} has {} rows, the box is {sz} deep", y + 1, rows.len()));
        }
        for (z, row) in rows.iter().enumerate() {
            if row.len() != sx {
                return Err(format!("layer {} row {} is {} long, the box is {sx} wide", y + 1, z + 1, row.len()));
            }
            for (x, ch) in row.bytes().enumerate() {
                if ch == b'.' {
                    continue;
                }
                let k = LETTERS.iter().position(|l| *l == ch).ok_or(format!("layer {} row {}: '{}' is no voxel letter", y + 1, z + 1, ch as char))?;
                if k >= colours {
                    return Err(format!("layer {} row {}: '{}' names no colour ({colours} listed)", y + 1, z + 1, ch as char));
                }
                cells[x + sx * (z + sz * y)] = k as u8 + 1;
            }
        }
    }
    Ok(cells)
}

/// `decode`'s inverse.
pub fn encode(size: [u32; 3], cells: &[u8]) -> String {
    let [sx, sy, sz] = size.map(|v| v as usize);
    let layers: Vec<String> = (0..sy)
        .map(|y| {
            (0..sz)
                .map(|z| (0..sx).map(|x| match cells[x + sx * (z + sz * y)] { 0 => '.', c => LETTERS[c as usize - 1] as char }).collect::<String>() + "\n")
                .collect()
        })
        .collect();
    layers.join("\n")
}

impl Model {
    /// Reads and checks a model; every refusal names `file` and why.
    pub fn parse(file: &str, text: &str, palette: &Palette) -> Result<Model, String> {
        // (a file with a life reads too: its states and events checked against the model's clips and points)
        parse_life(file, text, palette, &|_| true, &|_| true).map(|(m, _)| m)
    }

    fn check(&mut self, palette: &Palette) -> Result<(), String> {
        if self.colours.len() > LETTERS.len() {
            return Err(format!("{} colours, at most {}", self.colours.len(), LETTERS.len()));
        }
        self.rgb = self
            .colours
            .iter()
            .map(|n| palette.colours.iter().find(|c| &c.name == n).map(|c| c.rgb).ok_or(format!("colour '{n}' is not in the palette")))
            .collect::<Result<_, _>>()?;
        let find = |n: &str| self.parts.iter().position(|p| p.name == n);
        let mut up = Vec::new();
        for (i, p) in self.parts.iter().enumerate() {
            if find(&p.name) != Some(i) {
                return Err(format!("part '{}' is named twice", p.name));
            }
            if p.size.iter().any(|v| !(1..=MAX_BOX).contains(v)) {
                return Err(format!("part '{}': box {:?} outside 1..{MAX_BOX}", p.name, p.size));
            }
            up.push(match p.parent.as_str() {
                "" => None,
                n => Some(find(n).ok_or(format!("part '{}': no part '{n}' for its parent", p.name))?),
            });
        }
        for (i, p) in self.parts.iter().enumerate() {
            let (mut at, mut steps) = (up[i], 0);
            while let Some(k) = at {
                steps += 1;
                if steps > self.parts.len() {
                    return Err(format!("part '{}': its parents run in a loop", p.name));
                }
                at = up[k];
            }
        }
        self.up = up;
        let n = self.colours.len();
        for p in &mut self.parts {
            p.cells = decode(p.size, &p.voxels, n).map_err(|e| format!("part '{}': {e}", p.name))?;
        }
        let find = |n: &str| self.parts.iter().position(|p| p.name == n);
        for pt in &self.points {
            find(&pt.part).ok_or(format!("point '{}': no part '{}'", pt.name, pt.part))?;
        }
        for c in &self.clips {
            if !(c.length > 0.0) {
                return Err(format!("clip '{}': length must be over 0", c.name));
            }
            for k in &c.key {
                find(&k.part).ok_or(format!("clip '{}': a key names no part '{}'", c.name, k.part))?;
                if !(0.0..=c.length).contains(&k.t) {
                    return Err(format!("clip '{}': key of '{}' at t = {} is outside 0..{}", c.name, k.part, k.t, c.length));
                }
            }
        }
        Ok(())
    }

    /// The file's text (the voxels as their rows; parse it again to check it).
    pub fn write(&self) -> String {
        let mut v = toml::Value::try_from(self).expect("a model always serialises");
        as_typed(&mut v);
        toml::to_string(&v).expect("a model always serialises")
    }

    /// Each part's transform from its voxels to the cell's metres (x, y up, z; the cell's corner at 0) at `t`
    /// seconds into `clip` (a looping clip wraps, another holds its end); no clip, or one it lacks, rests.
    pub fn pose(&self, clip: Option<&str>, t: f32) -> Vec<Mat4> {
        let clip = clip.and_then(|n| self.clips.iter().find(|c| c.name == n));
        let t = clip.map_or(0.0, |c| if c.looping { t.rem_euclid(c.length) } else { t.clamp(0.0, c.length) });
        let mut out: Vec<Option<Mat4>> = vec![None; self.parts.len()];
        for i in 0..self.parts.len() {
            self.place(i, clip, t, &mut out);
        }
        let s = Mat4::from_scale(Vec3::splat(VOXEL));
        out.into_iter().map(|m| s * m.unwrap()).collect()
    }

    /// Part i in the model's voxels, its parents first.
    fn place(&self, i: usize, clip: Option<&Clip>, t: f32, out: &mut Vec<Option<Mat4>>) -> Mat4 {
        if let Some(m) = out[i] {
            return m;
        }
        let p = &self.parts[i];
        let (shift, turn) = clip.map_or((Vec3::ZERO, Vec3::ZERO), |c| sample(c, &p.name, t));
        let pivot = Vec3::from(p.pivot);
        let turn = turn * (std::f32::consts::PI / 180.0);
        let local = Mat4::from_translation(Vec3::from(p.at.map(|v| v as f32)) + pivot + shift)
            * Mat4::from_rotation_z(turn.z)
            * Mat4::from_rotation_y(turn.y)
            * Mat4::from_rotation_x(turn.x)
            * Mat4::from_translation(-pivot);
        let m = match self.up[i] {
            Some(k) => self.place(k, clip, t, out) * local,
            None => local,
        };
        out[i] = Some(m);
        m
    }
}

/// A model and its life from a file's text; every refusal names `file` and why: a state or event naming no clip,
/// point, effect (`effect` says which exist) or sound (`sound`), a light's colour not in the palette, an event's
/// time outside its clip.
pub fn parse_life(file: &str, text: &str, palette: &Palette, effect: &dyn Fn(&str) -> bool, sound: &dyn Fn(&str) -> bool) -> Result<(Model, Life), String> {
    let at = |e: String| format!("{file}: {e}");
    let mut t: toml::Table = toml::from_str(text).map_err(|e| at(e.to_string()))?;
    // the life comes out of the table first, so the model's own fields stay as strict as before
    let states = t.remove("state");
    let mut events = Vec::new();
    if let Some(toml::Value::Array(clips)) = t.get_mut("clip") {
        for c in clips.iter_mut().filter_map(|c| c.as_table_mut()) {
            if let Some(ev) = c.remove("event") {
                let name = c.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
                let ev: Vec<Event> = ev.try_into().map_err(|e: toml::de::Error| at(format!("clip '{name}': {e}")))?;
                events.push((name, ev));
            }
        }
    }
    let mut m: Model = toml::Value::Table(t).try_into().map_err(|e: toml::de::Error| at(e.to_string()))?;
    m.check(palette).map_err(at)?;
    let states: Vec<State> = match states {
        Some(s) => s.try_into().map_err(|e: toml::de::Error| at(format!("state: {e}")))?,
        None => Vec::new(),
    };
    let mut life = Life { states, events };
    check_life(&m, &mut life, palette, effect, sound).map_err(at)?;
    Ok((m, life))
}

fn check_life(m: &Model, life: &mut Life, palette: &Palette, effect: &dyn Fn(&str) -> bool, sound: &dyn Fn(&str) -> bool) -> Result<(), String> {
    let point = |n: &str| m.points.iter().any(|p| p.name == n);
    let names: Vec<String> = life.states.iter().map(|s| s.name.clone()).collect();
    for s in life.states.iter_mut() {
        let n = s.name.clone();
        let no = |what: &str, x: &str| Err(format!("state '{n}': no {what} '{x}'"));
        if names.iter().filter(|k| **k == n).count() > 1 {
            return Err(format!("state '{n}' is named twice"));
        }
        if let Some(c) = &s.clip {
            if !m.clips.iter().any(|k| &k.name == c) {
                return no("clip", c);
            }
        }
        for e in &s.effects {
            if !point(&e.point) {
                return no("point", &e.point);
            }
            if !effect(&e.effect) {
                return no("effect", &e.effect);
            }
        }
        if let Some(x) = &s.sound {
            if !sound(x) {
                return no("sound", x);
            }
        }
        if let Some(l) = &mut s.light {
            if !point(&l.point) {
                return no("point", &l.point);
            }
            l.rgb = palette.colours.iter().find(|c| c.name == l.colour).map(|c| c.rgb.map(|v| v as f32 / 255.0)).ok_or(format!("state '{n}': colour '{}' is not in the palette", l.colour))?;
            if !(l.strength > 0.0) {
                return Err(format!("state '{n}': a light's strength is over 0"));
            }
        }
    }
    for (c, evs) in &life.events {
        let clip = m.clips.iter().find(|k| &k.name == c).ok_or(format!("clip '{c}': events of no clip"))?;
        for e in evs {
            let no = |what: &str, x: &str| Err(format!("clip '{c}': no {what} '{x}'"));
            if !(0.0..=clip.length).contains(&e.t) {
                return Err(format!("clip '{c}': an event at t = {} is outside 0..{}", e.t, clip.length));
            }
            if e.sound.is_none() && e.effect.is_none() {
                return Err(format!("clip '{c}': an event plays a sound, an effect or both"));
            }
            if let Some(p) = e.point.as_deref().filter(|p| !point(p)) {
                return no("point", p);
            }
            if let Some(x) = e.effect.as_deref().filter(|x| !effect(x)) {
                return no("effect", x);
            }
            if let Some(x) = e.sound.as_deref().filter(|x| !sound(x)) {
                return no("sound", x);
            }
        }
    }
    Ok(())
}

/// The file's text of a model with its life (`Model::write` and the states and events put back).
pub fn write_life(m: &Model, life: &Life) -> String {
    let mut t: toml::Table = toml::from_str(&m.write()).expect("a written model reads");
    if let Some(toml::Value::Array(clips)) = t.get_mut("clip") {
        for c in clips.iter_mut().filter_map(|c| c.as_table_mut()) {
            let name = c.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
            if let Some((_, ev)) = life.events.iter().find(|(n, ev)| *n == name && !ev.is_empty()) {
                c.insert("event".into(), toml::Value::try_from(ev).expect("events serialise"));
            }
        }
    }
    if !life.states.is_empty() {
        t.insert("state".into(), toml::Value::try_from(&life.states).expect("states serialise"));
    }
    let mut t = toml::Value::Table(t);
    as_typed(&mut t);
    toml::to_string(&t).expect("a model always serialises")
}

/// Every number as its f32 prints (0.8, not 0.800000011920929), as `creatures::write` does, so a saved
/// file reads as it was typed.
fn as_typed(v: &mut toml::Value) {
    match v {
        toml::Value::Float(f) => *f = format!("{:?}", *f as f32).parse().unwrap_or(*f),
        toml::Value::Array(a) => a.iter_mut().for_each(as_typed),
        toml::Value::Table(t) => t.iter_mut().for_each(|(_, v)| as_typed(v)),
        _ => {}
    }
}

/// A file's leading comment lines (and the blank lines among them), which a written model puts back on top.
pub fn head(text: &str) -> String {
    text.lines().take_while(|l| l.trim().is_empty() || l.trim_start().starts_with('#')).map(|l| format!("{l}
")).collect()
}

/// The file's text of a model with its life, written over `old` (the file as it was): its leading comments on
/// top, every other comment line above the section (`[[part]]`, `[[state]]`, ...) it stood above, found again by
/// the section's `name` (else by its place among its kind), and CRLF if `old` had it.
pub fn write_keeping(old: &str, m: &Model, life: &Life) -> String {
    let top = head(old);
    let (notes, tail) = notes(&old.lines().skip(top.lines().count()).collect::<Vec<_>>());
    let new = write_life(m, life);
    let lines: Vec<&str> = new.lines().collect();
    let mut out = top;
    let mut kept = 0;
    for (i, id) in sections(&lines) {
        if let Some((_, c)) = notes.iter().find(|(n, _)| *n == id) {
            out.extend(lines[kept..i].iter().map(|l| format!("{l}\n")));
            out.extend(c.iter().map(|l| format!("{l}\n")));
            kept = i;
        }
    }
    out.extend(lines[kept..].iter().chain(&tail).map(|l| format!("{l}\n")));
    // (a section gone from the model takes its comments with it)
    if old.contains("\r\n") {
        out = out.replace('\n', "\r\n");
    }
    out
}

/// Each section's first line and who it is: its header (without a trailing comment) and its `name` (however
/// quoted or spaced), else its place among its kind; a sub-section (`[[clip.key]]`) its place within its parent.
fn sections(lines: &[&str]) -> Vec<(usize, (String, String))> {
    let mut out: Vec<(usize, (String, String))> = Vec::new();
    let mut in_string = false;
    // the last top-level section: its place in `out` and who it is
    let mut parent: Option<(usize, String)> = None;
    for (i, l) in lines.iter().enumerate() {
        let t = l.split('#').next().unwrap_or("").trim();
        if !in_string && t.starts_with('[') {
            let sub = t.contains('.');
            let since = if sub { parent.as_ref().map_or(0, |p| p.0 + 1) } else { 0 };
            let n = out[since..].iter().filter(|(_, (h, _))| h == t).count();
            let name = lines[i + 1..].iter().take_while(|l| !l.trim_start().starts_with('[')).find_map(|l| toml::from_str::<toml::Table>(l).ok()?.get("name")?.as_str().map(String::from));
            let mut id = name.unwrap_or_else(|| format!("#{n}"));
            if sub {
                id = format!("{}/{id}", parent.as_ref().map_or("", |p| p.1.as_str()));
            } else {
                parent = Some((out.len(), id.clone()));
            }
            out.push((i, (t.to_string(), id)));
        }
        in_string ^= l.matches("\"\"\"").count() % 2 == 1;
    }
    out
}

/// The comment lines of a file's body by the section they stand above (blank lines among them kept), and those
/// after the last section.
#[allow(clippy::type_complexity)]
fn notes<'a>(lines: &[&'a str]) -> (Vec<((String, String), Vec<&'a str>)>, Vec<&'a str>) {
    let heads = sections(lines);
    let mut out = Vec::new();
    let (mut c, mut in_string) = (Vec::new(), false);
    for (i, l) in lines.iter().enumerate() {
        if let Some((_, id)) = heads.iter().find(|(k, _)| *k == i) {
            if c.iter().any(|l: &&str| !l.trim().is_empty()) {
                // a blank line between the comments and the section stays out, as the written file has its own
                while c.last().is_some_and(|l: &&str| l.trim().is_empty()) {
                    c.pop();
                }
                out.push((id.clone(), std::mem::take(&mut c)));
            }
            c.clear();
        } else if !in_string && l.trim_start().starts_with('#') {
            c.push(*l);
        } else if !in_string && l.trim().is_empty() && !c.is_empty() {
            c.push(*l);
        }
        in_string ^= l.matches("\"\"\"").count() % 2 == 1;
    }
    (out, c.into_iter().skip_while(|l| l.trim().is_empty()).collect())
}

/// The events of a clip that fire as its time runs from `t0` (not included) to `t1` (seconds since it started):
/// each once a pass of a looping clip (at its t, then a length later, and so on), once in all of another.
/// Each at most once a call, however many passes `t0..t1` spans (a scrub in the Studio).
pub fn events_between<'a>(c: &Clip, events: &'a [Event], t0: f64, t1: f64) -> Vec<&'a Event> {
    let len = c.length as f64;
    events
        .iter()
        .filter(|e| {
            let t = e.t as f64;
            if c.looping {
                // the passes k >= 0 whose moment k * len + t lies in (t0, t1]
                ((t1 - t) / len).floor().max(-1.0) > ((t0 - t) / len).floor().max(-1.0)
            } else {
                t0 < t && t <= t1
            }
        })
        .collect()
}

impl Model {
    /// Where point `name` is under `poses` (`pose`'s), in the cell's metres.
    pub fn point_at(&self, poses: &[Mat4], name: &str) -> Option<Vec3> {
        let p = self.points.iter().find(|p| p.name == name)?;
        let k = self.parts.iter().position(|q| q.name == p.part)?;
        Some(poses.get(k)?.transform_point3(Vec3::from(p.at)))
    }
}

/// A part's move and turn at t in a clip: held before its first key and after its last, eased between.
pub fn sample(c: &Clip, part: &str, t: f32) -> (Vec3, Vec3) {
    let mut keys: Vec<&Key> = c.key.iter().filter(|k| k.part == part).collect();
    keys.sort_by(|a, b| a.t.total_cmp(&b.t));
    let val = |k: &Key| (Vec3::from(k.shift), Vec3::from(k.turn));
    let Some(first) = keys.first() else { return (Vec3::ZERO, Vec3::ZERO) };
    let Some(n) = keys.iter().position(|k| k.t > t) else { return val(keys.last().unwrap()) };
    if n == 0 {
        return val(first);
    }
    let (a, b) = (keys[n - 1], keys[n]);
    let f = (t - a.t) / (b.t - a.t);
    let f = match a.ease {
        Ease::Linear => f,
        Ease::Smooth => f * f * (3.0 - 2.0 * f),
        Ease::Step => 0.0,
    };
    let ((s0, r0), (s1, r1)) = (val(a), val(b));
    (s0.lerp(s1, f), r0.lerp(r1, f))
}

/// A part's faces: hidden ones (against a voxel of the part) dropped, flat runs of one colour merged.
pub fn mesh(part: &Part) -> Vec<Quad> {
    let s = part.size.map(|v| v as i32);
    let get = |p: [i32; 3]| -> u8 {
        if (0..3).all(|a| (0..s[a]).contains(&p[a])) {
            part.cells[(p[0] + s[0] * (p[2] + s[2] * p[1])) as usize]
        } else {
            0
        }
    };
    let mut out = Vec::new();
    for a in 0..3 {
        let (ua, va) = ((a + 1) % 3, (a + 2) % 3);
        for sgn in 0..2 {
            let step = if sgn == 0 { 1 } else { -1 };
            for d in 0..s[a] {
                // this slice's visible faces: their colour + 1 at [v][u]
                let (nu, nv) = (s[ua] as usize, s[va] as usize);
                let mut m = vec![0u8; nu * nv];
                for v in 0..nv {
                    for u in 0..nu {
                        let mut p = [0; 3];
                        p[a] = d;
                        p[ua] = u as i32;
                        p[va] = v as i32;
                        let c = get(p);
                        p[a] += step;
                        if c != 0 && get(p) == 0 {
                            m[v * nu + u] = c;
                        }
                    }
                }
                for v in 0..nv {
                    let mut u = 0;
                    while u < nu {
                        let c = m[v * nu + u];
                        if c == 0 {
                            u += 1;
                            continue;
                        }
                        let mut w = 1;
                        while u + w < nu && m[v * nu + u + w] == c {
                            w += 1;
                        }
                        let mut h = 1;
                        while v + h < nv && (u..u + w).all(|x| m[(v + h) * nu + x] == c) {
                            h += 1;
                        }
                        for y in v..v + h {
                            m[y * nu + u..y * nu + u + w].fill(0);
                        }
                        let mut at = [0u8; 3];
                        at[a] = d as u8;
                        at[ua] = u as u8;
                        at[va] = v as u8;
                        out.push(Quad { at, dir: (a * 2 + sgn) as u8, w: w as u8, h: h as u8, colour: c - 1 });
                        u += w;
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette() -> Palette {
        Palette::parse("[[colour]]\nname = \"stone\"\nrgb = \"#808080\"\n\n[[colour]]\nname = \"copper\"\nrgb = \"#b87333\"\n").unwrap()
    }

    const BOX: &str = r#"colours = ["stone", "copper"]

[[part]]
name = "body"
box = [2, 2, 1]
voxels = """
ab

aa
"""

[[part]]
name = "door"
parent = "body"
at = [2, 0, 0]
pivot = [0, 0, 0]
box = [1, 1, 1]
voxels = """
b
"""

[[point]]
name = "flame"
part = "body"
at = [1, 1, 0]

[[clip]]
name = "work"
length = 1.0
loop = true
key = [
  { part = "door", t = 0.0, turn = [0, 0, 0], ease = "linear" },
  { part = "door", t = 0.5, turn = [0, 90, 0], ease = "step" },
  { part = "door", t = 0.75, move = [4, 0, 0] },
]
"#;

    #[test]
    fn a_model_reads_and_round_trips() {
        let m = Model::parse("box.toml", BOX, &palette()).unwrap();
        assert_eq!(m.rgb, vec![[0x80; 3], [0xb8, 0x73, 0x33]]);
        assert_eq!(m.up, vec![None, Some(0)]);
        // layer 0 (bottom) is "ab": x = 1, y = 0 is copper; layer 1 "aa"
        assert_eq!(m.parts[0].cells, vec![1, 2, 1, 1]);
        let again = Model::parse("again", &m.write(), &palette()).unwrap();
        assert_eq!(again, m);
        assert_eq!(decode([2, 2, 1], &encode([2, 2, 1], &m.parts[0].cells), 2).unwrap(), m.parts[0].cells);
    }

    #[test]
    fn bad_models_are_refused_by_file_and_reason() {
        let p = palette();
        let many: Vec<String> = (0..63).map(|_| "\"stone\"".to_string()).collect();
        for (bad, why) in [
            (BOX.replace("\"copper\"]", "\"gold\"]"), "not in the palette"),
            (format!("colours = [{}]\n", many.join(", ")), "63 colours"),
            (BOX.replace("box = [2, 2, 1]", "box = [33, 2, 1]"), "outside 1..32"),
            (BOX.replace("box = [2, 2, 1]", "box = [0, 2, 1]"), "outside 1..32"),
            (BOX.replace("ab\n\naa", "ab\n\naaa"), "row 1 is 3 long"),
            (BOX.replace("ab\n\naa", "ab"), "1 layers"),
            (BOX.replace("ab\n\naa", "ac\n\naa"), "'c' names no colour"),
            (BOX.replace("parent = \"body\"", "parent = \"lid\""), "no part 'lid'"),
            (BOX.replace("parent = \"body\"", "parent = \"door\""), "loop"),
            (BOX.replace("{ part = \"door\", t = 0.75", "{ part = \"knob\", t = 0.75"), "names no part 'knob'"),
            (BOX.replace("t = 0.75", "t = 1.5"), "outside 0..1"),
            (BOX.replace("name = \"flame\"", "name = \"flame\"\nsize = 2"), "unknown field"),
        ] {
            let e = Model::parse("data/models/box.toml", &bad, &p).expect_err(why);
            assert!(e.starts_with("data/models/box.toml: ") && e.contains(why), "{why}: {e}");
        }
    }

    #[test]
    fn a_clip_eases_turns_about_the_pivot_and_children_follow() {
        let mut m = Model::parse("box.toml", BOX, &palette()).unwrap();
        let at = |m: &Model, clip, t, part: usize, p: Vec3| m.pose(clip, t)[part].transform_point3(p) / VOXEL;
        let near = |a: Vec3, b: Vec3| assert!(a.distance(b) < 1e-4, "{a} vs {b}");
        // at rest the door sits at its `at`
        near(at(&m, None, 0.0, 1, Vec3::ZERO), Vec3::new(2.0, 0.0, 0.0));
        // linear half way to 90 degrees about y at the pivot: the far corner x = 1 swings towards -z
        let h = 45f32.to_radians();
        near(at(&m, Some("work"), 0.25, 1, Vec3::X), Vec3::new(2.0 + h.cos(), 0.0, -h.sin()));
        // step holds 90 until the next key, then the move; the turn left zero there (no turn given)
        near(at(&m, Some("work"), 0.6, 1, Vec3::X), Vec3::new(2.0, 0.0, -1.0));
        near(at(&m, Some("work"), 0.75, 1, Vec3::ZERO), Vec3::new(6.0, 0.0, 0.0));
        // looping wraps: 1.25 is 0.25
        near(at(&m, Some("work"), 1.25, 1, Vec3::X), at(&m, Some("work"), 0.25, 1, Vec3::X));
        // smooth: f = 0.5 eases to 0.5, f = 0.25 to 0.15625
        m.clips[0].key[0].ease = Ease::Smooth;
        let a = (90.0f32 * 0.15625).to_radians();
        near(at(&m, Some("work"), 0.125, 1, Vec3::X), Vec3::new(2.0 + a.cos(), 0.0, -a.sin()));
        // a child follows its parent: the body moved, the door moves with it
        m.clips[0].key.push(Key { part: "body".into(), t: 0.0, shift: [0.0, 3.0, 0.0], turn: [0.0; 3], ease: Ease::Linear });
        near(at(&m, Some("work"), 0.0, 1, Vec3::ZERO), Vec3::new(2.0, 3.0, 0.0));
    }

    #[test]
    fn the_mesher_drops_hidden_faces_and_merges_flat_runs() {
        let part = |size: [u32; 3], cells: Vec<u8>| Part { name: "p".into(), parent: String::new(), at: [0; 3], pivot: [0.0; 3], size, voxels: String::new(), cells };
        assert_eq!(mesh(&part([1, 1, 1], vec![1])).len(), 6);
        // a bar of one colour: six faces; of two colours: the four long sides split in two
        assert_eq!(mesh(&part([2, 1, 1], vec![1, 1])).len(), 6);
        assert_eq!(mesh(&part([2, 1, 1], vec![1, 2])).len(), 10);
        // a full 4 x 4 x 4 cube: six faces, each 4 x 4, nothing inside
        let q = mesh(&part([4, 4, 4], vec![1; 64]));
        assert_eq!(q.len(), 6);
        assert!(q.iter().all(|q| q.w == 4 && q.h == 4));
        let top = q.iter().find(|q| q.dir == 2).unwrap();
        assert_eq!(top.at, [0, 3, 0], "a + face's corner is in its voxel; the face is on its far side");
    }

    #[test]
    fn the_draft_furnace_loads() {
        let p = Palette::parse(include_str!("../../../data/palette.toml")).unwrap();
        let m = Model::parse("data/models/furnace.toml", include_str!("../../../data/models/furnace.toml"), &p).unwrap();
        assert!(m.clips.iter().any(|c| c.name == "work") && m.points.iter().any(|p| p.name == "flame"));
        // and its life: working burns, smokes at the chimney, hums, glows, and clanks as the door shuts
        let (_, life) = parse_life("data/models/furnace.toml", include_str!("../../../data/models/furnace.toml"), &p, &|_| true, &|_| true).unwrap();
        let s = life.states.iter().find(|s| s.name == "working").expect("a working state");
        assert_eq!(s.clip.as_deref(), Some("work"));
        assert!(s.effects.iter().any(|e| e.effect == "fire" && e.point == "flame") && s.effects.iter().any(|e| e.point == "chimney"));
        assert_eq!(s.sound.as_deref(), Some("furnace.burn"));
        assert!(s.light.as_ref().is_some_and(|l| l.rgb[0] > 0.5));
        assert!(life.events.iter().any(|(c, e)| c == "work" && e.iter().any(|e| e.sound.as_deref() == Some("furnace.clank"))));
    }

    #[test]
    fn the_furnace_writes_back_as_typed() {
        let p = Palette::parse(include_str!("../../../data/palette.toml")).unwrap();
        let text = include_str!("../../../data/models/furnace.toml");
        let (m, life) = parse_life("furnace.toml", text, &p, &|_| true, &|_| true).unwrap();
        let out = head(text) + &write_life(&m, &life);
        // the leading comment kept, every number as typed (no f32 noise such as 0.800000011920929)
        assert!(out.starts_with("# A draft furnace"), "{out}");
        assert!(!out.contains("0000") && !out.contains("9999"), "{out}");
        assert!(out.contains("t = 0.8
") && out.contains("strength = 0.7
"), "{out}");
        assert_eq!(parse_life("again", &out, &p, &|_| true, &|_| true).unwrap(), (m, life));
    }

    #[test]
    fn a_save_keeps_every_comment_above_its_section_and_crlf() {
        let p = Palette::parse(include_str!("../../../data/palette.toml")).unwrap();
        let text = include_str!("../../../data/models/furnace.toml").replace("\r\n", "\n");
        let (m, life) = parse_life("furnace.toml", &text, &p, &|_| true, &|_| true).unwrap();
        // a note above the door part, one inside the body part (it stands above the next section), one above a state
        let door = m.parts.iter().position(|q| q.name == "door").expect("the furnace has a door");
        let mut noted = text.replacen("[[state]]", "# when it burns\n[[state]]", 1);
        let at = noted.match_indices("[[part]]").nth(door).unwrap().0;
        noted.insert_str(at, "# the door: copper\n#   swings in `work`\n");
        noted = noted.replacen("pivot = ", "# about its middle\npivot = ", 1);
        // as a hand writes them: other quotes and spacing, a comment after a header
        noted = noted.replacen("name = \"door\"", "name='door'", 1).replacen("[[state]]\nname = \"working\"", "[[state]] # the one state\nname=\"working\"", 1);
        assert!(noted.contains("name='door'") && noted.contains("# the one state"));
        for crlf in [false, true] {
            let old = if crlf { noted.replace('\n', "\r\n") } else { noted.clone() };
            let out = write_keeping(&old, &m, &life);
            assert_eq!(parse_life("again", &out, &p, &|_| true, &|_| true).unwrap(), (m.clone(), life.clone()));
            assert!(out.starts_with("# A draft furnace"), "the head kept");
            assert_eq!(out.contains("\r\n"), crlf);
            assert!(!crlf || !out.replace("\r\n", "").contains('\n'), "every line CRLF");
            let out = out.replace("\r\n", "\n");
            let after = |c: &str| out[out.find(c).unwrap_or_else(|| panic!("{c} lost: {out}")) + c.len()..].to_string();
            // the section below a comment: its header and its name (the writer orders the keys)
            let below = |c: &str, header: &str, name: &str| {
                let rest = after(c);
                let section = &rest[..rest[2..].find("\n[[").map_or(rest.len(), |k| k + 2)];
                rest.starts_with(header) && section.contains(&format!("name = \"{name}\""))
            };
            assert!(below("# the door: copper\n#   swings in `work`\n", "[[part]]", "door"), "{out}");
            assert!(below("# when it burns\n", "[[state]]", "working"), "{out}");
            assert!(after("# about its middle\n").lines().find(|l| !l.trim().is_empty() && !l.starts_with('#')).is_some_and(|l| l.starts_with("[[")),"a comment inside a section stands above the next: {out}");
        }
    }

    /// A comment above a clip's key stays with that key of that clip when another clip gains a key.
    #[test]
    fn a_keys_comment_stays_with_its_clip() {
        let p = Palette::parse(include_str!("../../../data/palette.toml")).unwrap();
        let text = include_str!("../../../data/models/furnace.toml").replace("\r\n", "\n");
        let (mut m, life) = parse_life("furnace.toml", &text, &p, &|_| true, &|_| true).unwrap();
        let mut two = m.clips[0].clone();
        two.name = "shut".into();
        m.clips.push(two);
        let old = write_life(&m, &life);
        // a note above the first key of the second clip
        let at = old.find("name = \"shut\"").unwrap();
        let k = at + old[at..].find("[[clip.key]]").unwrap();
        let old = format!("{}# shut's first key\n{}", &old[..k], &old[k..]);
        // the first clip gains a key at its start
        let first = m.clips[0].key[0].clone();
        m.clips[0].key.insert(0, first);
        let out = write_keeping(&old, &m, &life);
        let at = out.find("# shut's first key\n").unwrap_or_else(|| panic!("lost: {out}"));
        let clip = out[..at].rfind("[[clip]]").unwrap();
        assert!(out[clip..at].contains("name = \"shut\"") && !out[clip..at].contains("[[clip.key]]"), "the note moved off shut's first key: {out}");
    }

    const LIFE: &str = r#"
[[state]]
name = "working"
clip = "work"
effects = [{ effect = "fire", point = "flame" }]
sound = "hum"
light = { point = "flame", colour = "copper", strength = 2.0 }

[[state]]
name = "still"
"#;

    fn lively() -> String {
        BOX.replace("  { part = \"door\", t = 0.75, move = [4, 0, 0] },\n]", "  { part = \"door\", t = 0.75, move = [4, 0, 0] },\n]\nevent = [{ t = 0.4, sound = \"clank\" }, { t = 0.0, effect = \"sparks\", point = \"flame\" }]") + LIFE
    }

    #[test]
    fn a_life_reads_refuses_by_file_and_reason_and_round_trips() {
        let p = palette();
        let effects = |n: &str| ["fire", "sparks"].contains(&n);
        let sounds = |n: &str| ["hum", "clank"].contains(&n);
        let text = lively();
        assert!(text.contains("event = ["));
        let (m, life) = parse_life("box.toml", &text, &p, &effects, &sounds).unwrap();
        assert_eq!(life.states.len(), 2);
        assert_eq!(life.states[0].light.as_ref().unwrap().rgb, [0xb8 as f32 / 255.0, 0x73 as f32 / 255.0, 0x33 as f32 / 255.0]);
        assert_eq!(life.events, vec![("work".to_string(), vec![Event { t: 0.4, sound: Some("clank".into()), ..Event::default() }, Event { t: 0.0, effect: Some("sparks".into()), point: Some("flame".into()), ..Event::default() }])]);
        // the model alone reads the same file, its life left aside, and is the same model
        assert_eq!(Model::parse("box.toml", &text, &p).unwrap(), m);
        // written back with its life, read again: the same
        let again = parse_life("again", &write_life(&m, &life), &p, &effects, &sounds).unwrap();
        assert_eq!(again, (m.clone(), life.clone()));
        for (bad, why) in [
            (text.replace("clip = \"work\"", "clip = \"dance\""), "state 'working': no clip 'dance'"),
            (text.replace("{ effect = \"fire\", point = \"flame\" }", "{ effect = \"fire\", point = \"spout\" }"), "state 'working': no point 'spout'"),
            (text.replace("effect = \"fire\"", "effect = \"rain\""), "state 'working': no effect 'rain'"),
            (text.replace("sound = \"hum\"", "sound = \"whir\""), "state 'working': no sound 'whir'"),
            (text.replace("colour = \"copper\"", "colour = \"mauve\""), "state 'working': colour 'mauve' is not in the palette"),
            (text.replace("point = \"flame\", colour", "point = \"top\", colour"), "state 'working': no point 'top'"),
            (text.replace("t = 0.4, sound = \"clank\"", "t = 1.5, sound = \"clank\""), "clip 'work': an event at t = 1.5 is outside 0..1"),
            (text.replace("sound = \"clank\"", "sound = \"bong\""), "clip 'work': no sound 'bong'"),
            (text.replace("effect = \"sparks\"", "effect = \"smoke\""), "clip 'work': no effect 'smoke'"),
            (text.replace("{ t = 0.4, sound = \"clank\" }", "{ t = 0.4 }"), "clip 'work': an event plays a sound, an effect or both"),
            (text.replace("name = \"still\"", "name = \"working\""), "state 'working' is named twice"),
            (text.replace("name = \"still\"", "name = \"still\"\nmood = 3"), "unknown field"),
        ] {
            let e = parse_life("data/models/box.toml", &bad, &p, &effects, &sounds).expect_err(why);
            assert!(e.starts_with("data/models/box.toml: ") && e.contains(why), "{why}: {e}");
        }
    }

    #[test]
    fn events_fire_exactly_once_a_pass() {
        let m = Model::parse("box.toml", BOX, &palette()).unwrap();
        let ev = |t: f32| Event { t, sound: Some("x".into()), ..Event::default() };
        let events = vec![ev(0.0), ev(0.4), ev(1.0)];
        let mut clip = m.clips[0].clone();
        // ten seconds of a one-second loop, in frames of uneven length: each event ten times
        for dt in [1.0 / 60.0, 1.0 / 7.0, 0.3] {
            let mut counts = [0; 3];
            let (mut t0, mut t) = (-1e-9, 0.0f64);
            while t < 10.0 - 1e-5 {
                t = (t + dt).min(10.0 - 1e-6);
                for e in events_between(&clip, &events, t0, t) {
                    counts[events.iter().position(|x| std::ptr::eq(x, e)).unwrap()] += 1;
                }
                t0 = t;
            }
            // the first frame starts it: t = 0 fires at once; 1.0 is the end of a pass (the next one's start)
            assert_eq!(counts, [10, 10, 9], "frames of {dt} s");
        }
        // a jump of many passes in one frame (the Studio scrubbing): each event at most once
        assert_eq!(events_between(&clip, &events, 0.1, 25.7).len(), 3);
        assert_eq!(events_between(&clip, &events, 25.7, 0.1).len(), 0, "and none scrubbing back");
        // one that does not loop: once each, then never
        clip.looping = false;
        let mut n = 0;
        let mut t0 = -1e-9;
        for k in 1..=300 {
            let t = k as f64 / 60.0;
            n += events_between(&clip, &events, t0, t).len();
            t0 = t;
        }
        assert_eq!(n, 3);
    }

    #[test]
    fn a_point_moves_with_its_part() {
        let mut m = Model::parse("box.toml", BOX, &palette()).unwrap();
        m.points.push(Point { name: "knob".into(), part: "door".into(), at: [1.0, 0.0, 0.0] });
        // at rest the door's voxel x = 1 is at the body's x = 3; at 0.6 s the door has turned 90 degrees about y
        let near = |a: Vec3, b: Vec3| assert!(a.distance(b) < 1e-4, "{a} vs {b}");
        near(m.point_at(&m.pose(None, 0.0), "knob").unwrap(), Vec3::new(3.0, 0.0, 0.0) * VOXEL);
        near(m.point_at(&m.pose(Some("work"), 0.6), "knob").unwrap(), Vec3::new(2.0, 0.0, -1.0) * VOXEL);
        near(m.point_at(&m.pose(Some("work"), 0.6), "flame").unwrap(), Vec3::new(1.0, 1.0, 0.0) * VOXEL);
        assert!(m.point_at(&m.pose(None, 0.0), "nowhere").is_none());
    }
}
