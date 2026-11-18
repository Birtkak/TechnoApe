//! Maps (docs/design/studio-maps.md, ADR draft-studio-maps): `data/maps/<name>.toml`, a graph of nodes that
//! makes a small picture which tiles and may loop. Read, check, write, and the working out on the CPU, each
//! node's picture kept until something before it changes. Every generator lies on a lattice that wraps at the
//! picture's edge and, when it moves, along time too, so every map tiles and every animated map loops.
use crate::rows::num;
use engine::palette::Palette;
use std::borrow::Cow;
use std::collections::HashMap;
use std::f32::consts::TAU;
use std::path::Path;

/// What a pin carries. A picture holds three numbers a pixel: a number is (v, v, v), a colour (r, g, b), a
/// direction (x, y, 0) with y down the picture.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Data {
    Num,
    Col,
    Dir,
    /// A pin that takes any kind; the node gives what its first such pin is fed, and its other such pins must
    /// take that.
    Any,
}
use Data::*;

impl Data {
    pub fn name(self) -> &'static str {
        match self {
            Num => "a number",
            Col => "a colour",
            Dir => "a direction",
            Any => "anything",
        }
    }
    /// A number plugs in as grey, a colour as its brightness; nothing becomes a direction by itself.
    fn takes(self, from: Data) -> bool {
        self == Any || self == from || matches!((self, from), (Num, Col) | (Col, Num))
    }
}

#[derive(PartialEq, Debug)]
pub struct Slider {
    pub name: &'static str,
    pub min: f32,
    pub max: f32,
    pub def: f32,
    pub whole: bool,
}
const fn n(name: &'static str, min: f32, max: f32, def: f32) -> Slider {
    Slider { name, min, max, def, whole: false }
}
const fn w(name: &'static str, min: f32, max: f32, def: f32) -> Slider {
    Slider { name, min, max, def, whole: true }
}

/// A kind of node: its group on the page, what it gives, its input pins (name, kind, the number an unplugged
/// one holds), its numbers and its words (an empty list: a palette ramp's name).
#[derive(PartialEq, Debug)]
pub struct Spec {
    pub kind: &'static str,
    pub group: &'static str,
    pub out: Data,
    pub pins: &'static [(&'static str, Data, f32)],
    pub nums: &'static [Slider],
    pub words: &'static [(&'static str, &'static [&'static str])],
}
const fn spec(kind: &'static str, group: &'static str, out: Data, pins: &'static [(&'static str, Data, f32)], nums: &'static [Slider], words: &'static [(&'static str, &'static [&'static str])]) -> Spec {
    Spec { kind, group, out, pins, nums, words }
}

const SEED: Slider = w("seed", 0.0, 65535.0, 1.0);
const SCALE: Slider = w("scale", 1.0, 64.0, 4.0);
const NOISE: &[Slider] = &[SEED, SCALE, w("octaves", 1.0, 8.0, 4.0), n("roughness", 0.0, 1.0, 0.5), w("speed", 0.0, 16.0, 0.0)];
const IN_NUM: &[(&str, Data, f32)] = &[("in", Num, 0.0)];
const IN_ANY: &[(&str, Data, f32)] = &[("in", Any, 0.0)];
pub const SIZES: [usize; 6] = [16, 32, 64, 128, 256, 512];

pub static SPECS: &[Spec] = &[
    spec("value noise", "Make", Num, &[], NOISE, &[]),
    spec("smooth noise", "Make", Num, &[], NOISE, &[]),
    spec("cells", "Make", Num, &[], &[SEED, SCALE, w("speed", -16.0, 16.0, 0.0)], &[("mode", &["distance", "edges", "cell"])]),
    spec("speckle", "Make", Num, &[], &[SEED, w("scale", 1.0, 512.0, 64.0), w("speed", 0.0, 64.0, 0.0)], &[]),
    spec("gradient", "Make", Num, &[], &[w("repeat", 1.0, 16.0, 1.0)], &[("shape", &["across", "down", "round"])]),
    spec("shape", "Make", Num, &[], &[w("repeat", 1.0, 32.0, 4.0), n("size", 0.0, 1.0, 0.5), n("soft", 0.0, 1.0, 0.1)], &[("shape", &["disc", "square", "bricks", "checker", "stripes"])]),
    spec("constant", "Make", Num, &[], &[n("value", 0.0, 1.0, 0.5)], &[]),
    spec("levels", "Change", Num, IN_NUM, &[n("low", 0.0, 1.0, 0.0), n("high", 0.0, 1.0, 1.0), n("contrast", 0.0, 4.0, 1.0)], &[]),
    spec("steps", "Change", Num, IN_NUM, &[w("count", 2.0, 32.0, 4.0)], &[]),
    spec("invert", "Change", Num, IN_NUM, &[], &[]),
    spec("blur", "Change", Any, IN_ANY, &[n("radius", 0.0, 32.0, 2.0)], &[]),
    spec("move", "Change", Any, IN_ANY, &[n("shift_x", -1.0, 1.0, 0.0), n("shift_y", -1.0, 1.0, 0.0), w("repeat", 1.0, 8.0, 1.0), w("mirror", 0.0, 1.0, 0.0), w("speed_x", -16.0, 16.0, 0.0), w("speed_y", -16.0, 16.0, 0.0)], &[]),
    spec("warp", "Change", Any, &[("in", Any, 0.0), ("by", Dir, 0.0)], &[n("amount", 0.0, 1.0, 0.1)], &[]),
    spec("blend", "Combine", Any, &[("a", Any, 0.0), ("b", Any, 0.0), ("mask", Num, 1.0)], &[n("amount", 0.0, 1.0, 1.0)], &[("mode", &["mix", "add", "multiply", "screen", "lighter", "darker", "difference"])]),
    spec("colours", "Turn into", Col, IN_NUM, &[w("dither", 0.0, 1.0, 0.0)], &[("ramp", &[])]),
    spec("slope", "Turn into", Dir, IN_NUM, &[n("strength", 0.0, 10.0, 1.0)], &[]),
    spec("curl", "Turn into", Dir, IN_NUM, &[n("strength", 0.0, 10.0, 1.0)], &[]),
    spec("length", "Turn into", Num, &[("in", Dir, 0.0)], &[], &[]),
    spec("out", "Out", Any, IN_ANY, &[], &[]),
];

#[derive(Clone, PartialEq, Debug)]
pub enum Pin {
    Wire(usize),
    /// Nothing plugged in: its slider's number (a direction pin's is no push).
    Value(f32),
}

#[derive(Clone, PartialEq, Debug)]
pub struct Node {
    pub id: String,
    kind: usize,
    pub at: [f32; 2],
    /// By the spec's `nums`, `words` and `pins`.
    pub nums: Vec<f32>,
    pub words: Vec<String>,
    pub pins: Vec<Pin>,
    /// Colours only: the palette colours chosen by name, when no ramp is.
    pub colours: Vec<String>,
}

impl Node {
    fn new(kind: usize, id: String, at: [f32; 2]) -> Node {
        let s = &SPECS[kind];
        Node {
            id,
            kind,
            at,
            nums: s.nums.iter().map(|x| x.def).collect(),
            words: s.words.iter().map(|x| x.1.first().unwrap_or(&"").to_string()).collect(),
            pins: s.pins.iter().map(|p| Pin::Value(p.2)).collect(),
            colours: vec![],
        }
    }
    pub fn spec(&self) -> &'static Spec {
        &SPECS[self.kind]
    }
    fn num(&self, name: &str) -> f32 {
        self.nums[self.spec().nums.iter().position(|s| s.name == name).expect("a number of its kind")]
    }
    fn word(&self, name: &str) -> &str {
        &self.words[self.spec().words.iter().position(|s| s.0 == name).expect("a word of its kind")]
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Map {
    pub size: usize,
    pub frames: usize,
    pub seconds: f32,
    pub nodes: Vec<Node>,
    /// Colours nodes' colours as 0..1, by node (resolved from the palette when read).
    rgb: Vec<Vec<[f32; 3]>>,
}

/// The colours a Colours node turns a number into: a palette ramp's, else the colours it names.
fn paints(ramp: &str, colours: &[String], pal: &Palette) -> Result<Vec<[f32; 3]>, String> {
    let names: Vec<String> = if ramp.is_empty() { colours.to_vec() } else { pal.ramp(ramp).iter().map(|&i| pal.colours[i].name.clone()).collect() };
    if !ramp.is_empty() && names.is_empty() {
        return Err(format!("ramp '{ramp}' is not in the palette"));
    }
    if names.is_empty() {
        return Err("names neither a ramp nor colours".into());
    }
    names.iter().map(|c| pal.colours.iter().find(|p| &p.name == c).map(|p| p.rgb.map(|b| b as f32 / 255.0)).ok_or(format!("colour '{c}' is not in the palette"))).collect()
}

fn f(v: &toml::Value) -> Option<f32> {
    v.as_float().map(|f| f as f32).or(v.as_integer().map(|i| i as f32))
}

/// Reads and checks a map; every refusal names `file` and, where there is one, the node.
pub fn parse(file: &str, text: &str, pal: &Palette) -> Result<Map, String> {
    let bad = |m: String| format!("{file}: {m}");
    let t: toml::Table = text.parse().map_err(|e| bad(format!("{e}")))?;
    if let Some(k) = t.keys().find(|k| !["size", "frames", "seconds", "node"].contains(&k.as_str())) {
        return Err(bad(format!("unknown key '{k}'")));
    }
    let top = |k: &str, def: f32| t.get(k).map_or(Some(def), f).ok_or(bad(format!("{k} is not a number")));
    let (size, frames, seconds) = (top("size", 128.0)?, top("frames", 1.0)?, top("seconds", 2.0)?);
    if !SIZES.contains(&(size as usize)) || size.fract() != 0.0 {
        return Err(bad(format!("size {size} is not one of {SIZES:?}")));
    }
    if !(1.0..=64.0).contains(&frames) || frames.fract() != 0.0 {
        return Err(bad(format!("frames {frames} is not a whole number in 1..64")));
    }
    if !(0.1..=60.0).contains(&seconds) {
        return Err(bad(format!("seconds {seconds} is out of 0.1..60")));
    }
    let rows: Vec<&toml::Table> = match t.get("node") {
        None => vec![],
        Some(v) => v.as_array().and_then(|a| a.iter().map(|r| r.as_table()).collect()).ok_or(bad("node is not a list of [[node]] rows".into()))?,
    };
    let mut m = Map { size: size as usize, frames: frames as usize, seconds, nodes: vec![], rgb: vec![] };
    let mut wires = vec![];
    for (row, r) in rows.iter().enumerate() {
        let id = r.get("id").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).ok_or(bad(format!("node {} has no id", row + 1)))?.to_string();
        let bad = |e: String| bad(format!("node '{id}': {e}"));
        if m.find(&id).is_some() {
            return Err(bad("its id is used twice".into()));
        }
        let kind = r.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        let k = SPECS.iter().position(|s| s.kind == kind).ok_or(bad(format!("unknown kind '{kind}'")))?;
        let s = &SPECS[k];
        let mut node = Node::new(k, id.clone(), [0.0; 2]);
        let mut wired = vec![None; s.pins.len()];
        for (key, v) in r.iter().filter(|(k, _)| !["id", "kind"].contains(&k.as_str())) {
            if key == "at" {
                node.at = v.as_array().filter(|a| a.len() == 2).and_then(|a| Some([f(&a[0])?, f(&a[1])?])).filter(|a| a.iter().all(|x| x.is_finite())).ok_or(bad("at is not [x, y] of two finite numbers".into()))?;
            } else if let Some(i) = s.nums.iter().position(|x| x.name == key) {
                let (x, v) = (&s.nums[i], f(v).ok_or(bad(format!("{key} is not a number")))?);
                if !(x.min..=x.max).contains(&v) {
                    return Err(bad(format!("{key} {v} is out of {}..{}", x.min, x.max)));
                }
                if x.whole && v.fract() != 0.0 {
                    return Err(bad(format!("{key} {v} is not a whole number")));
                }
                node.nums[i] = v;
            } else if let Some(i) = s.words.iter().position(|x| x.0 == key) {
                let word = v.as_str().ok_or(bad(format!("{key} is not a word")))?;
                let list = s.words[i].1;
                if !list.is_empty() && !list.contains(&word) {
                    return Err(bad(format!("{key} '{word}' is not one of {list:?}")));
                }
                node.words[i] = word.to_string();
            } else if let Some(i) = s.pins.iter().position(|p| p.0 == key) {
                match (v.as_str(), f(v)) {
                    (Some(src), _) => wired[i] = Some(src.to_string()),
                    (_, Some(_)) if s.pins[i].1 == Dir => return Err(bad(format!("pin '{key}' takes a direction: plug one in or leave it out"))),
                    (_, Some(x)) if (0.0..=1.0).contains(&x) => node.pins[i] = Pin::Value(x),
                    _ => return Err(bad(format!("pin '{key}' is neither a node's id nor a number in 0..1"))),
                }
            } else if key == "colours" && kind == "colours" {
                node.colours = v.as_array().and_then(|a| a.iter().map(|c| c.as_str().map(String::from)).collect()).ok_or(bad("colours is not a list of palette names".into()))?;
            } else {
                return Err(bad(format!("unknown key '{key}'")));
            }
        }
        m.rgb.push(if kind == "colours" { paints(&node.words[0], &node.colours, pal).map_err(bad)? } else { vec![] });
        m.nodes.push(node);
        wires.push(wired);
    }
    for (i, wired) in wires.into_iter().enumerate() {
        for (p, src) in wired.into_iter().enumerate() {
            if let Some(src) = src {
                let j = m.find(&src).ok_or(bad(format!("node '{}': pin '{}' names '{src}', which is no node", m.nodes[i].id, m.nodes[i].spec().pins[p].0)))?;
                m.nodes[i].pins[p] = Pin::Wire(j);
            }
        }
    }
    m.order().map_err(bad)?;
    Ok(m)
}

impl Map {
    pub fn find(&self, id: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.id == id)
    }

    fn order(&self) -> Result<(Vec<usize>, Vec<Data>), String> {
        self.check().map_err(|e| e.1)
    }

    /// The order to work the nodes out in (each after what feeds it) and the kind each gives; a circle of
    /// wires, a pin fed a kind it cannot take (that node and pin come with the refusal), and no out or two
    /// are refused.
    #[allow(clippy::type_complexity)]
    fn check(&self) -> Result<(Vec<usize>, Vec<Data>), (Option<(usize, usize)>, String)> {
        fn visit(m: &Map, i: usize, state: &mut [u8], order: &mut Vec<usize>) -> Result<(), String> {
            match state[i] {
                2 => return Ok(()),
                1 => return Err(format!("node '{}': its wires make a circle", m.nodes[i].id)),
                _ => {}
            }
            state[i] = 1;
            for p in &m.nodes[i].pins {
                if let Pin::Wire(j) = p {
                    visit(m, *j, state, order)?;
                }
            }
            state[i] = 2;
            order.push(i);
            Ok(())
        }
        let (mut state, mut order) = (vec![0; self.nodes.len()], vec![]);
        for i in 0..self.nodes.len() {
            visit(self, i, &mut state, &mut order).map_err(|e| (None, e))?;
        }
        let mut kinds = vec![Num; self.nodes.len()];
        for &i in &order {
            let n = &self.nodes[i];
            let s = n.spec();
            kinds[i] = match s.out {
                Any => match s.pins.iter().position(|p| p.1 == Any).map(|p| &n.pins[p]) {
                    Some(Pin::Wire(j)) => kinds[*j],
                    _ => Num,
                },
                k => k,
            };
            for (p, pin) in n.pins.iter().enumerate() {
                let want = if s.pins[p].1 == Any { kinds[i] } else { s.pins[p].1 };
                if let Pin::Wire(j) = pin {
                    if !want.takes(kinds[*j]) {
                        return Err((Some((i, p)), format!("node '{}': pin '{}' takes {}, '{}' gives {}", n.id, s.pins[p].0, want.name(), self.nodes[*j].id, kinds[*j].name())));
                    }
                }
            }
        }
        let outs: Vec<&str> = self.nodes.iter().filter(|n| n.spec().kind == "out").map(|n| n.id.as_str()).collect();
        match outs[..] {
            [_] => Ok((order, kinds)),
            [] => Err((None, "no node is the out".into())),
            [a, b, ..] => Err((None, format!("nodes '{a}' and '{b}' are both an out"))),
        }
    }

    /// An id not taken: `base` without a number after it, then "base 2", "base 3"...
    fn free_id(&self, base: &str) -> String {
        let base = base.rsplit_once(' ').filter(|(_, n)| n.parse::<u32>().is_ok()).map_or(base, |(b, _)| b);
        (1..).map(|k| if k == 1 { base.to_string() } else { format!("{base} {k}") }).find(|n| self.find(n).is_none()).unwrap()
    }

    /// A new node of `kind` with its numbers as new, named after its kind; its index.
    pub fn add(&mut self, kind: &str, at: [f32; 2]) -> usize {
        let k = SPECS.iter().position(|s| s.kind == kind).expect("a kind of SPECS");
        let id = self.free_id(kind);
        self.nodes.push(Node::new(k, id, at));
        self.rgb.push(vec![]);
        self.nodes.len() - 1
    }

    /// Plugs `from`'s output into pin `pin` of `to`; a circle or a kind the pin cannot take is refused, and
    /// the map stays as it was.
    pub fn wire(&mut self, from: usize, to: usize, pin: usize) -> Result<(), String> {
        let was = std::mem::replace(&mut self.nodes[to].pins[pin], Pin::Wire(from));
        self.check().map(|_| ()).map_err(|e| {
            self.nodes[to].pins[pin] = was;
            e.1
        })
    }

    /// Pulls the wire out of a pin: it holds its number as new.
    pub fn unplug(&mut self, to: usize, pin: usize) {
        self.nodes[to].pins[pin] = Pin::Value(self.nodes[to].spec().pins[pin].2);
        self.repair();
    }

    /// Unplugs pins until the map checks again: a node unplugged or taken away can change what a node after
    /// it gives.
    fn repair(&mut self) {
        while let Err((Some((i, p)), _)) = self.check() {
            self.nodes[i].pins[p] = Pin::Value(self.nodes[i].spec().pins[p].2);
        }
    }

    /// Takes nodes away (never the out); what they fed is unplugged.
    pub fn remove(&mut self, set: &[usize]) {
        let gone: Vec<bool> = (0..self.nodes.len()).map(|i| set.contains(&i) && self.nodes[i].spec().kind != "out").collect();
        let to: Vec<usize> = gone.iter().scan(0, |k, g| Some(std::mem::replace(k, *k + !g as usize))).collect();
        for i in 0..self.nodes.len() {
            for p in 0..self.nodes[i].pins.len() {
                if let Pin::Wire(j) = self.nodes[i].pins[p] {
                    self.nodes[i].pins[p] = if gone[j] { Pin::Value(self.nodes[i].spec().pins[p].2) } else { Pin::Wire(to[j]) };
                }
            }
        }
        let mut k = 0..;
        self.nodes.retain(|_| !gone[k.next().unwrap()]);
        let mut k = 0..;
        self.rgb.retain(|_| !gone[k.next().unwrap()]);
        self.repair();
    }

    /// Copies of nodes (never the out) a little aside, wired among themselves as the originals are and to
    /// what fed them from outside; their indexes.
    pub fn copy(&mut self, set: &[usize]) -> Vec<usize> {
        let set: Vec<usize> = set.iter().copied().filter(|&i| self.nodes[i].spec().kind != "out").collect();
        let start = self.nodes.len();
        for &i in &set {
            let mut n = self.nodes[i].clone();
            n.id = self.free_id(&n.id);
            n.at = [n.at[0] + 40.0, n.at[1] + 40.0];
            for p in &mut n.pins {
                if let Pin::Wire(j) = p {
                    if let Some(c) = set.iter().position(|s| s == j) {
                        *j = start + c;
                    }
                }
            }
            self.nodes.push(n);
            self.rgb.push(self.rgb[i].clone());
        }
        (start..self.nodes.len()).collect()
    }

    /// Sets a node's number, or an unplugged pin's, held to its range; false: it has no such number.
    pub fn set_num(&mut self, node: usize, name: &str, v: f32) -> bool {
        let n = &mut self.nodes[node];
        let s = n.spec();
        if let Some(i) = s.nums.iter().position(|x| x.name == name) {
            let v = v.clamp(s.nums[i].min, s.nums[i].max);
            n.nums[i] = if s.nums[i].whole { v.round() } else { v };
        } else if let Some(i) = s.pins.iter().position(|p| p.0 == name && p.1 != Dir).filter(|&i| matches!(n.pins[i], Pin::Value(_))) {
            n.pins[i] = Pin::Value(v.clamp(0.0, 1.0));
        } else {
            return false;
        }
        true
    }

    /// Sets a node's word (a mode, a shape, a Colours node's palette ramp).
    pub fn set_word(&mut self, node: usize, name: &str, word: &str, pal: &Palette) -> Result<(), String> {
        let s = self.nodes[node].spec();
        let i = s.words.iter().position(|w| w.0 == name).ok_or(format!("node '{}' has no {name}", self.nodes[node].id))?;
        let list = s.words[i].1;
        if !list.is_empty() && !list.contains(&word) {
            return Err(format!("{name} '{word}' is not one of {list:?}"));
        }
        if list.is_empty() {
            self.rgb[node] = paints(word, &[], pal)?;
            self.nodes[node].colours.clear();
        }
        self.nodes[node].words[i] = word.into();
        Ok(())
    }

    /// The file's text: every number, word and pin written, in the kind's order.
    pub fn write(&self) -> String {
        let q = crate::js;
        let mut t = format!("size = {}\nframes = {}\nseconds = {}\n", self.size, self.frames, num(self.seconds));
        for n in &self.nodes {
            let s = n.spec();
            t += &format!("\n[[node]]\nid = {}\nkind = {}\nat = [{}, {}]\n", q(&n.id), q(s.kind), num(n.at[0]), num(n.at[1]));
            for (x, v) in s.nums.iter().zip(&n.nums) {
                t += &format!("{} = {}\n", x.name, num(*v));
            }
            for (x, v) in s.words.iter().zip(&n.words).filter(|(_, v)| !v.is_empty()) {
                t += &format!("{} = {}\n", x.0, q(v));
            }
            if !n.colours.is_empty() {
                t += &format!("colours = [{}]\n", n.colours.iter().map(|c| q(c)).collect::<Vec<_>>().join(", "));
            }
            for (p, pin) in s.pins.iter().zip(&n.pins) {
                match pin {
                    Pin::Wire(j) => t += &format!("{} = {}\n", p.0, q(&self.nodes[*j].id)),
                    Pin::Value(v) if p.1 != Dir => t += &format!("{} = {}\n", p.0, num(*v)),
                    Pin::Value(_) => {}
                }
            }
        }
        t
    }
}

/// `check=1` over `data/maps/*.toml`: the maps that passed and every refusal.
pub fn check_dir(dir: &Path, pal: &Palette) -> (usize, Vec<String>) {
    let mut files: Vec<String> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.ends_with(".toml")).collect();
    files.sort();
    let mut bad = vec![];
    for n in &files {
        let shown = format!("data/maps/{n}");
        if let Err(e) = std::fs::read_to_string(dir.join(n)).map_err(|e| format!("{shown}: {e}")).and_then(|t| parse(&shown, &t, pal)) {
            bad.push(e);
        }
    }
    (files.len() - bad.len(), bad)
}

/// A picture, `w` x `w`, by rows.
pub type Pic = Vec<[f32; 3]>;

/// A map being worked out: each node's pictures kept by (frame, size) until a number before it changes.
pub struct Graph {
    pub map: Map,
    order: Vec<usize>,
    /// Per node, a number new each time its pictures are thrown away (the page keeps its textures by it).
    pub stamp: Vec<u64>,
    pub kinds: Vec<Data>,
    kept: Vec<HashMap<(usize, usize), Pic>>,
    /// Pictures worked out so far (the tests count the rework).
    pub worked: usize,
    /// Where the picture starts, in sides (0 always but in the tiling test, which works it out half a side on).
    pub shift: f32,
}

impl Graph {
    /// A map that `parse` took.
    pub fn new(map: Map) -> Graph {
        let (order, kinds) = map.order().expect("a map that was read is checked");
        Graph { kept: vec![HashMap::new(); map.nodes.len()], stamp: map.nodes.iter().map(|_| next_stamp()).collect(), map, order, kinds, worked: 0, shift: 0.0 }
    }

    pub fn out(&self) -> usize {
        self.map.nodes.iter().position(|n| n.spec().kind == "out").expect("one out")
    }

    /// Takes the map as the page has it now: pictures a changed number does not reach are kept; anything
    /// else changed is worked out anew.
    pub fn update(&mut self, map: &Map) {
        if self.map == *map {
            return;
        }
        let shape = |m: &Map| (m.size, m.frames, m.rgb.clone(), m.nodes.iter().map(|n| (n.kind, n.words.clone(), n.pins.iter().map(|p| if let Pin::Wire(j) = p { Some(*j) } else { None }).collect::<Vec<_>>())).collect::<Vec<_>>());
        if shape(&self.map) != shape(map) {
            let worked = self.worked;
            *self = Graph::new(map.clone());
            self.worked = worked;
            return;
        }
        let changed = self.map.nodes.iter().zip(&map.nodes).map(|(a, b)| a.nums != b.nums || a.pins != b.pins).collect();
        self.map = map.clone();
        self.dirty(changed);
    }

    /// Sets a node's number, or an unplugged pin's, held to its range; what is downstream is worked out anew.
    #[allow(dead_code)]
    pub fn set(&mut self, node: usize, name: &str, v: f32) -> Result<(), String> {
        if !self.map.set_num(node, name, v) {
            return Err(format!("node '{}' has no number '{name}'", self.map.nodes[node].id));
        }
        let mut changed = vec![false; self.map.nodes.len()];
        changed[node] = true;
        self.dirty(changed);
        Ok(())
    }

    /// Throws away the pictures of the nodes changed and of every node after them.
    fn dirty(&mut self, mut d: Vec<bool>) {
        for &i in &self.order {
            if self.map.nodes[i].pins.iter().any(|p| matches!(p, Pin::Wire(j) if d[*j])) {
                d[i] = true;
            }
        }
        for (i, _) in d.into_iter().enumerate().filter(|(_, d)| *d) {
            self.kept[i].clear();
            self.stamp[i] = next_stamp();
        }
    }

    /// Node `node`'s picture at `frame` (it loops: `frames` is frame 0 again) and `w` px a side, any size: a
    /// preview may ask for less than the map's.
    pub fn pic(&mut self, node: usize, frame: usize, w: usize) -> &Pic {
        let key = (frame, w);
        if !self.kept[node].contains_key(&key) {
            for p in self.map.nodes[node].pins.clone() {
                if let Pin::Wire(j) = p {
                    self.pic(j, frame, w);
                }
            }
            let px = self.work(node, frame, w);
            // shortcut: a node keeps at most 8 pictures (a strip of frames would hold them all); a picture
            // per frame, once Play wants it
            if self.kept[node].len() >= 8 {
                self.kept[node].clear();
            }
            self.kept[node].insert(key, px);
            self.worked += 1;
        }
        &self.kept[node][&key]
    }

    /// Pin `p` of node `i` as the pin takes it (its inputs are worked out).
    fn input(&self, i: usize, p: usize, key: (usize, usize)) -> Cow<'_, Pic> {
        let want = match self.map.nodes[i].spec().pins[p].1 {
            Any => self.kinds[i],
            k => k,
        };
        match self.map.nodes[i].pins[p] {
            Pin::Wire(j) if want == Num && self.kinds[j] == Col => Cow::Owned(self.kept[j][&key].iter().map(|c| [0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]; 3]).collect()),
            Pin::Wire(j) => Cow::Borrowed(&self.kept[j][&key]),
            Pin::Value(v) => Cow::Owned(vec![if want == Dir { [0.0; 3] } else { [v; 3] }; key.1 * key.1]),
        }
    }

    fn work(&self, i: usize, frame: usize, w: usize) -> Pic {
        let n = &self.map.nodes[i];
        let v = |name: &str| n.num(name);
        let t = frame as f32 / self.map.frames as f32;
        let key = (frame, w);
        let inp = |p: usize| self.input(i, p, key);
        let uv = |k: usize| (((k % w) as f32 + 0.5) / w as f32, ((k / w) as f32 + 0.5) / w as f32);
        // generators only: what changes a picture works on its input, which is already shifted
        let make = |g: &dyn Fn(f32, f32) -> f32| -> Pic {
            (0..w * w).map(|k| {
                let (x, y) = uv(k);
                [g(x + self.shift, y + self.shift); 3]
            }).collect()
        };
        let each = |p: &Pic, g: &dyn Fn([f32; 3]) -> [f32; 3]| -> Pic { p.iter().map(|c| g(*c)).collect() };
        let seed = || v("seed") as u32;
        let px = match n.spec().kind {
            k @ ("value noise" | "smooth noise") => make(&|x, y| fbm(n, x, y, t, k == "smooth noise")),
            "cells" => {
                let (s, speed, mode) = (v("scale") as i32, v("speed"), n.word("mode"));
                make(&|x, y| cells(seed(), s, speed, mode, x, y, t))
            }
            "speckle" => {
                let (s, speed) = (v("scale"), v("speed") as i32);
                let step = if speed == 0 { 0 } else { ((t * speed as f32).floor() as i32).rem_euclid(speed) };
                let cell = |a: f32| ((a * s).floor() as i32).rem_euclid(s as i32);
                make(&|x, y| unit(hash(seed(), cell(x), cell(y), step)))
            }
            "gradient" => {
                let r = v("repeat");
                let tri = |a: f32| 1.0 - (2.0 * (a * r).fract() - 1.0).abs();
                match n.word("shape") {
                    "across" => make(&|x, _| tri(x)),
                    "down" => make(&|_, y| tri(y)),
                    _ => make(&|x, y| 1.0 - ((x * r).fract() - 0.5).hypot((y * r).fract() - 0.5) / 0.5f32.hypot(0.5)),
                }
            }
            "shape" => {
                let (r, size, soft, shape) = (v("repeat"), v("size"), v("soft"), n.word("shape"));
                let fill = |d: f32| if soft < 1e-3 { (d < 0.0) as u8 as f32 } else { 0.5 - d / soft };
                make(&|x, y| {
                    let (fu, fv) = ((x * r).fract() - 0.5, (y * r).fract() - 0.5);
                    match shape {
                        "disc" => fill(fu.hypot(fv) - size * 0.5),
                        "square" => fill(fu.abs().max(fv.abs()) - size * 0.5),
                        "stripes" => fill(fu.abs() - size * 0.5),
                        "checker" => (((x * r * 2.0) as i32 + (y * r * 2.0) as i32) & 1) as f32,
                        // two rows a brick across, every other row half a brick along; size is how much is brick
                        _ => {
                            let row = (y * r * 2.0) as i32;
                            let (bu, bv) = ((x * r + 0.5 * (row & 1) as f32).fract() - 0.5, (y * r * 2.0).fract() - 0.5);
                            let gap = (1.0 - size) * 0.25;
                            fill((bu.abs() - 0.5 + gap * 0.5).max(bv.abs() - 0.5 + gap))
                        }
                    }
                })
            }
            "constant" => make(&|_, _| v("value")),
            "levels" => {
                let (lo, hi, c) = (v("low"), v("high"), v("contrast"));
                each(&inp(0), &|p| {
                    let x = ((p[0] - lo) / (hi - lo).max(1e-4)).clamp(0.0, 1.0);
                    [(x - 0.5) * c + 0.5; 3]
                })
            }
            "steps" => {
                let c = v("count");
                each(&inp(0), &|p| [(p[0] * c).floor().min(c - 1.0) / (c - 1.0); 3])
            }
            "invert" => each(&inp(0), &|p| [1.0 - p[0]; 3]),
            "blur" => {
                let r = (v("radius") * w as f32 / self.map.size as f32).round() as i32;
                let mut p = inp(0).into_owned();
                for _ in 0..2 * (r > 0) as usize {
                    p = blur(&blur(&p, w, r, 1, w), w, r, w, 1);
                }
                p
            }
            "move" => {
                let (r, mirror) = (v("repeat"), v("mirror") == 1.0);
                let (sx, sy) = (v("shift_x") + t * v("speed_x"), v("shift_y") + t * v("speed_y"));
                let tile = |a: f32| {
                    let x = a * r;
                    if mirror && (x as i32) & 1 == 1 { 1.0 - x.fract() } else { x.fract() }
                };
                let src = inp(0);
                (0..w * w).map(|k| {
                    let (x, y) = uv(k);
                    sample(&src, w, tile(x) - sx, tile(y) - sy)
                }).collect()
            }
            "warp" => {
                let (src, by, a) = (inp(0), inp(1), v("amount"));
                (0..w * w).map(|k| {
                    let (x, y) = uv(k);
                    sample(&src, w, x + by[k][0] * a, y + by[k][1] * a)
                }).collect()
            }
            "blend" => {
                let (a, b, mask, amt, mode) = (inp(0), inp(1), inp(2), v("amount"), n.word("mode"));
                (0..w * w).map(|k| {
                    let s = amt * mask[k][0];
                    std::array::from_fn(|c| {
                        let (a, b) = (a[k][c], b[k][c]);
                        let r = match mode {
                            "add" => a + b,
                            "multiply" => a * b,
                            "screen" => 1.0 - (1.0 - a) * (1.0 - b),
                            "lighter" => a.max(b),
                            "darker" => a.min(b),
                            "difference" => (a - b).abs(),
                            _ => b,
                        };
                        a + (r - a) * s
                    })
                }).collect()
            }
            "colours" => {
                let (rgb, dither) = (&self.map.rgb[i], v("dither") == 1.0);
                if rgb.is_empty() {
                    return inp(0).into_owned();
                }
                let last = rgb.len() - 1;
                const BAYER: [f32; 16] = [0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0, 3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0];
                let src = inp(0);
                (0..w * w).map(|k| {
                    let x = src[k][0];
                    let c = if dither { (x * last as f32 + (BAYER[(k / w % 4) * 4 + k % 4] + 0.5) / 16.0 - 0.5).round() } else { (x * rgb.len() as f32).floor() };
                    rgb[(c.max(0.0) as usize).min(last)]
                }).collect()
            }
            k @ ("slope" | "curl") => {
                let (src, s) = (inp(0), v("strength") * 0.1 * w as f32 / 2.0);
                let at = |x: usize, y: usize| src[(y % w) * w + x % w][0];
                (0..w * w).map(|k2| {
                    let (x, y) = (k2 % w, k2 / w);
                    let (gx, gy) = ((at(x + 1, y) - at(x + w - 1, y)) * s, (at(x, y + 1) - at(x, y + w - 1)) * s);
                    if k == "slope" { [-gx, -gy, 0.0] } else { [gy, -gx, 0.0] }
                }).collect()
            }
            "length" => each(&inp(0), &|p| [p[0].hypot(p[1]); 3]),
            _ => inp(0).into_owned(),
        };
        if self.kinds[i] == Dir {
            // the whole field scaled so its strongest push is 1: a clamp of x and y apart would bend the
            // pushes and give curl sinks
            let top = px.iter().map(|c| c[0].hypot(c[1])).fold(1.0, f32::max);
            return px.into_iter().map(|c| [c[0] / top, c[1] / top, 0.0]).collect();
        }
        px.into_iter().map(|c| c.map(|x| x.clamp(0.0, 1.0))).collect()
    }
}

fn next_stamp() -> u64 {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

fn hash(seed: u32, x: i32, y: i32, z: i32) -> u32 {
    let mut h = seed.wrapping_mul(0x9e37_79b1) ^ (x as u32).wrapping_mul(0x85eb_ca77) ^ (y as u32).wrapping_mul(0xc2b2_ae3d) ^ (z as u32).wrapping_mul(0x27d4_eb2f);
    h = (h ^ (h >> 15)).wrapping_mul(0x2c1b_3c6d);
    h = (h ^ (h >> 12)).wrapping_mul(0x297a_2d39);
    h ^ (h >> 15)
}
fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / 16_777_216.0
}

/// One octave on a lattice that wraps every `p` cells along x, y and time: `corner` gives a corner's part at
/// the point's offset from it, blended by the quintic fade.
fn octave(c: [f32; 3], p: [i32; 3], corner: &dyn Fn([i32; 3], [f32; 3]) -> f32) -> f32 {
    let i = c.map(|x| x.floor());
    let fr = [c[0] - i[0], c[1] - i[1], c[2] - i[2]];
    let fade = fr.map(|x| x * x * x * (x * (x * 6.0 - 15.0) + 10.0));
    (0..8).map(|k| {
        let o = [k & 1, (k >> 1) & 1, (k >> 2) & 1];
        let cell = std::array::from_fn(|a| (i[a] as i32 + o[a]).rem_euclid(p[a]));
        let d = std::array::from_fn(|a| fr[a] - o[a] as f32);
        let wt: f32 = (0..3).map(|a| if o[a] == 1 { fade[a] } else { 1.0 - fade[a] }).product();
        wt * corner(cell, d)
    }).sum()
}

const GRAD: [[f32; 3]; 12] = [[1., 1., 0.], [-1., 1., 0.], [1., -1., 0.], [-1., -1., 0.], [1., 0., 1.], [-1., 0., 1.], [1., 0., -1.], [-1., 0., -1.], [0., 1., 1.], [0., -1., 1.], [0., 1., -1.], [0., -1., -1.]];

/// Value or gradient noise in octaves: octave `o` has `scale` x 2^o cells a side and, moving, 2 x `speed` x 2^o
/// cells along the loop (whole, so it loops; two a turn, so a speed of 1 still changes).
fn fbm(n: &Node, x: f32, y: f32, t: f32, smooth: bool) -> f32 {
    let (seed, scale, speed, rough) = (n.num("seed") as u32, n.num("scale") as i32, n.num("speed") as i32, n.num("roughness"));
    let (mut s, mut amp, mut total) = (0.0, 1.0, 0.0);
    for o in 0..n.num("octaves") as i32 {
        let (f, pt) = (scale << o, if speed == 0 { 1 } else { (2 * speed) << o });
        let sd = seed.wrapping_add(o as u32 * 1013);
        let c = [x * f as f32, y * f as f32, t * pt as f32 * (speed != 0) as u8 as f32];
        let v = if smooth {
            0.5 + 0.6 * octave(c, [f, f, pt], &|q, d| {
                let g = GRAD[(hash(sd, q[0], q[1], q[2]) % 12) as usize];
                g[0] * d[0] + g[1] * d[1] + g[2] * d[2]
            })
        } else {
            octave(c, [f, f, pt], &|q, _| unit(hash(sd, q[0], q[1], q[2])))
        };
        s += amp * v;
        total += amp;
        amp *= rough;
    }
    (s / total).clamp(0.0, 1.0)
}

/// Cells: `scale` a side, each with a point that circles `speed` times a loop.
fn cells(seed: u32, scale: i32, speed: f32, mode: &str, x: f32, y: f32, t: f32) -> f32 {
    let (cx, cy) = (x * scale as f32, y * scale as f32);
    let (ix, iy) = (cx.floor() as i32, cy.floor() as i32);
    let (mut d1, mut d2, mut id) = (9f32, 9f32, 0);
    for dy in -1..=1 {
        for dx in -1..=1 {
            let (gx, gy) = (ix + dx, iy + dy);
            let h = hash(seed, gx.rem_euclid(scale), gy.rem_euclid(scale), 0);
            let a = TAU * (t * speed + unit(hash(h, 1, 0, 0))).fract();
            let px = gx as f32 + 0.5 + 0.3 * (2.0 * unit(h) - 1.0) + 0.15 * a.cos();
            let py = gy as f32 + 0.5 + 0.3 * (2.0 * unit(hash(h, 2, 0, 0)) - 1.0) + 0.15 * a.sin();
            let d = (cx - px).hypot(cy - py);
            if d < d1 {
                (d2, d1, id) = (d1, d, h);
            } else if d < d2 {
                d2 = d;
            }
        }
    }
    match mode {
        "edges" => (d2 - d1) * 2.0,
        "cell" => unit(hash(id, 3, 0, 0)),
        _ => d1 * 1.25,
    }
}

/// A box blur of radius `r` along one axis (`step` between neighbours, `line` between lines), wrapping.
fn blur(p: &Pic, w: usize, r: i32, step: usize, line: usize) -> Pic {
    let mut out = vec![[0.0; 3]; w * w];
    let k = 1.0 / (2 * r + 1) as f32;
    for l in 0..w {
        for x in 0..w {
            let mut s = [0.0; 3];
            for d in -r..=r {
                let c = p[l * line + (x as i32 + d).rem_euclid(w as i32) as usize * step];
                (0..3).for_each(|a| s[a] += c[a]);
            }
            out[l * line + x * step] = s.map(|v| v * k);
        }
    }
    out
}

/// The picture at (u, v) in 0..1 of a side, between its pixels, wrapping.
fn sample(p: &Pic, w: usize, u: f32, v: f32) -> [f32; 3] {
    let (x, y) = (u * w as f32 - 0.5, v * w as f32 - 0.5);
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let at = |dx: i64, dy: i64| p[(y0 as i64 + dy).rem_euclid(w as i64) as usize * w + (x0 as i64 + dx).rem_euclid(w as i64) as usize];
    let (a, b, c, d) = (at(0, 0), at(1, 0), at(0, 1), at(1, 1));
    std::array::from_fn(|k| (a[k] * (1.0 - fx) + b[k] * fx) * (1.0 - fy) + (c[k] * (1.0 - fx) + d[k] * fx) * fy)
}

/// A picture as PNG bytes: a number grey, a colour as it is, a direction in red and green (the middle is no push).
pub fn rgba(p: &Pic, kind: Data) -> Vec<u8> {
    let b = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    p.iter().flat_map(|c| match kind {
        Dir => [b(c[0] * 0.5 + 0.5), b(c[1] * 0.5 + 0.5), 0, 255],
        _ => [b(c[0]), b(c[1]), b(c[2]), 255],
    }).collect()
}

/// The out's frames side by side at the map's size, as written to a PNG: its width, height and RGBA bytes.
pub fn strip(g: &mut Graph, frames: &[usize]) -> (usize, usize, Vec<u8>) {
    let (s, o) = (g.map.size, g.out());
    let kind = g.kinds[o];
    let pics: Vec<Vec<u8>> = frames.iter().map(|&k| rgba(g.pic(o, k, s), kind)).collect();
    let row = s * 4;
    let px = (0..s).flat_map(|y| pics.iter().flat_map(move |p| p[y * row..(y + 1) * row].iter().copied())).collect();
    (s * frames.len(), s, px)
}

/// Export: the map's out at its size (an animated one as a strip of its frames) written to `out` as a PNG,
/// free colours; its width and height.
pub fn export(m: &Map, out: &str) -> Result<(usize, usize), String> {
    let mut g = Graph::new(m.clone());
    let (w, h, px) = strip(&mut g, &(0..m.frames).collect::<Vec<_>>());
    crate::write_rgba(out, w as u32, h as u32, &px)?;
    Ok((w, h))
}

/// `map=<name> out=x.png [frame=k]`: the map's out at its size, an animated one without `frame` as a strip of
/// its frames side by side; one JSON line.
pub fn draw(name: &str, out: &str, frame: Option<usize>) -> Result<String, String> {
    let read = |f: &str| std::fs::read_to_string(f).map_err(|e| format!("{f}: {e}"));
    let pal = Palette::parse(&read("data/palette.toml")?).map_err(|e| format!("data/palette.toml: {e}"))?;
    let file = format!("data/maps/{name}.toml");
    let mut g = Graph::new(parse(&file, &read(&file)?, &pal)?);
    let (s, o) = (g.map.size, g.out());
    let kind = g.kinds[o];
    let frames: Vec<usize> = frame.map_or((0..g.map.frames).collect(), |k| vec![k]);
    let start = std::time::Instant::now();
    let (w, h, px) = strip(&mut g, &frames);
    let ms = start.elapsed().as_secs_f64() * 1000.0;
    crate::write_rgba(out, w as u32, h as u32, &px)?;
    let hash = px.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ *b as u64).wrapping_mul(0x100_0000_01b3));
    let kind = ["number", "colour", "direction", "any"][kind as usize];
    Ok(format!("{{\"map\":{},\"out\":{},\"size\":{s},\"frames\":{},\"drawn\":{},\"kind\":\"{kind}\",\"nodes\":{},\"ms\":{ms:.1},\"hash\":\"{hash:016x}\"}}", crate::js(name), crate::js(out), g.map.frames, frames.len(), g.map.nodes.len()))
}

#[cfg(test)]
mod tests;
