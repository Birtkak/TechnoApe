//! Sound (docs/design/sound.md, ADR 0032). Every sound is a row of `data/sounds.toml`: made in code here
//! (`render`), or a recording in `data/sounds/` of the row's name. The sound card's own thread mixes them
//! with `Mixer::mix`, the same function the tests call; the game only sends it small messages.
pub mod chain;

use glam::DVec3;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
use std::sync::{mpsc, Arc, Mutex};

/// The rate the rows are rendered at; the mixer plays them at the sound card's.
pub const RATE: f32 = 48000.0;
/// The most voices at once; past it the quietest one-shot goes.
pub const VOICES: usize = 32;

#[derive(Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Wave {
    /// White noise through a band-pass filter at `freq`.
    Noise,
    Sine,
    Triangle,
    Square,
}

/// Whose volume slider it follows (the settings' Sound page).
#[derive(Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Bus {
    World = 0,
    Steps = 1,
    Music = 3,
    Ui = 2,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Layer {
    wave: Wave,
    /// Hz at its start and its end: a tone's pitch, a noise's filter.
    freq: [f32; 2],
    /// How narrow a noise's filter is.
    #[serde(default = "one")]
    q: f32,
    #[serde(default = "attack")]
    attack: f32,
    /// Seconds to fall to a thousandth; a loop has none.
    #[serde(default)]
    decay: f32,
    #[serde(default = "one")]
    gain: f32,
    #[serde(default)]
    delay: f32,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub name: String,
    pub bus: Bus,
    /// Metres at which it is gone.
    #[serde(default = "reach")]
    pub reach: f32,
    /// Its loudest sample.
    #[serde(default = "one")]
    pub gain: f32,
    /// How many different takes are made of it.
    #[serde(default = "one_n")]
    pub variants: u32,
    /// The share each play's pitch varies by.
    #[serde(default)]
    pub pitch: f32,
    /// Seconds of a sound that goes round; 0 for one that plays once.
    #[serde(default, rename = "loop")]
    pub loops: f32,
    #[serde(default)]
    pub layer: Vec<Layer>,
    /// Made from other sounds instead of layers (ADR 0048): rows by name, or recordings in data/sounds/base/.
    #[serde(default)]
    pub from: Vec<Base>,
    #[serde(default, rename = "chain")]
    chain_toml: Vec<toml::Table>,
    #[serde(default, rename = "spread")]
    spread_toml: toml::Table,
    /// The changes made to every take in turn, and how far each take's numbers stray (read by `parse`).
    #[serde(skip)]
    pub chain: Vec<chain::Step>,
    #[serde(skip)]
    pub spread: Vec<chain::Spread>,
}

/// One sound a row is made from: a row's name, or a recording `data/sounds/base/<base>.wav`.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Base {
    pub base: String,
    #[serde(default = "one")]
    pub gain: f32,
    #[serde(default)]
    pub delay: f32,
}
fn one() -> f32 {
    1.0
}
fn one_n() -> u32 {
    1
}
fn attack() -> f32 {
    0.002
}
fn reach() -> f32 {
    24.0
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    sound: Vec<Row>,
}

/// Reads and checks the rows: names once each, every number in its range.
pub fn parse(text: &str) -> Result<Vec<Row>, String> {
    let mut f: File = toml::from_str(text).map_err(|e| e.to_string())?;
    for r in &mut f.sound {
        let name = r.name.clone();
        let bad = |what: &str| format!("sound '{name}': {what}");
        r.chain = r.chain_toml.iter().map(chain::Step::parse).collect::<Result<_, _>>().map_err(|e| bad(&e))?;
        r.spread = chain::parse_spread(&r.spread_toml).map_err(|e| bad(&e))?;
        if r.spread.iter().any(|p| !r.chain.iter().any(|s| s.kind == p.step)) {
            return Err(bad("a spread on a step its chain does not have"));
        }
        // a switched-off step is checked, then left out; its spread draws nothing without it
        let on: Vec<bool> = r.chain_toml.iter().map(chain::Step::is_on).collect();
        let mut on = on.into_iter();
        r.chain.retain(|_| on.next().unwrap());
        if r.loops > 0.0 && r.chain.iter().any(|s| matches!(s.name(), "trim" | "fade")) {
            return Err(bad("a loop's chain has no trim or fade (it goes round)"));
        }
        if r.from.iter().any(|b| !(0.0..=2.0).contains(&b.gain) || !(0.0..=10.0).contains(&b.delay) || (r.loops > 0.0 && b.delay >= r.loops)) {
            return Err(bad("a base's gain 0..2, delay 0..10 (within a loop)"));
        }
    }
    // a row made from itself, directly or through others, is refused: follow every row's bases
    for r in &f.sound {
        let mut path = vec![r.name.as_str()];
        fn walk<'a>(rows: &'a [Row], path: &mut Vec<&'a str>) -> Result<(), String> {
            let Some(r) = rows.iter().find(|r| r.name == *path.last().unwrap()) else { return Ok(()) };
            for b in &r.from {
                if b.base == path[0] {
                    return Err(format!("sound '{}': made from itself (through {})", path[0], path[1..].iter().chain([&b.base.as_str()]).map(|n| format!("'{n}'")).collect::<Vec<_>>().join(", ")));
                }
                if !path.contains(&b.base.as_str()) {
                    path.push(&b.base);
                    walk(rows, path)?;
                    path.pop();
                }
            }
            Ok(())
        }
        walk(&f.sound, &mut path)?;
    }
    for r in &f.sound {
        let bad = |what: &str| Err(format!("sound '{}': {what}", r.name));
        if f.sound.iter().filter(|o| o.name == r.name).count() > 1 {
            return bad("its name is used twice");
        }
        if r.layer.is_empty() == r.from.is_empty() || !(1..=8).contains(&r.variants) {
            return bad("needs layers or bases (`from`), not both, and 1 to 8 variants");
        }
        if !(0.0..=2.0).contains(&r.gain) || !(1.0..=200.0).contains(&r.reach) || !(0.0..=0.5).contains(&r.pitch) || !(r.loops == 0.0 || (0.5..=60.0).contains(&r.loops)) {
            return bad("gain 0..2, reach 1..200, pitch 0..0.5, loop 0.5..60 or none");
        }
        for l in &r.layer {
            if l.freq.iter().any(|f| !(20.0..=16000.0).contains(f)) || !(0.5..=20.0).contains(&l.q) || !(0.0..=2.0).contains(&l.gain) {
                return bad("a layer's freq 20..16000, q 0.5..20, gain 0..2");
            }
            if l.wave == Wave::Noise && (l.q < 0.6 || l.freq.iter().any(|f| *f > 7000.0)) {
                return bad("a noise layer's q from 0.6, freq to 7000 (past that its filter is not stable)");
            }
            if r.loops > 0.0 {
                // (a layer with a decay is a note in the loop; without, it sounds all the way round)
                if !(0.0005..=10.0).contains(&l.attack) || !(0.0..r.loops).contains(&l.delay) || !(0.0..=20.0).contains(&l.decay) || l.attack + l.decay > r.loops || (l.decay == 0.0 && l.delay != 0.0) {
                    return bad("in a loop a layer's attack 0.0005..10, delay within the loop (none without a decay), decay 0..20, and no longer than the loop");
                }
            } else if !(0.0005..=1.0).contains(&l.attack) || !(0.0..=1.0).contains(&l.delay) || !(0.005..=4.0).contains(&l.decay) {
                return bad("a layer's attack 0.0005..1, delay 0..1, decay 0.005..4");
            }
        }
    }
    Ok(f.sound)
}

/// A landing's thump (the row `land`) by the metres fallen: none for a jump's own landing, a small one
/// from two blocks, louder and lower from higher. `(gain, pitch)`.
pub fn thump(drop: f64) -> Option<(f32, f32)> {
    let over = (drop - 1.6) as f32;
    (over >= 0.0).then(|| ((0.3 + over * 0.1).min(1.5), 1.0 - (over * 0.04).min(0.35)))
}

/// Which take plays next: any but the one this row played last, so no sound is heard twice running.
pub fn next_take(seed: &mut u32, variants: u32, last: u8) -> u8 {
    if variants < 2 {
        return 0;
    }
    // one of the others, each as likely
    let t = ((rand(seed) * (variants - 1) as f32) as u8).min(variants as u8 - 2);
    t + (t >= last) as u8
}

fn rand(s: &mut u32) -> f32 {
    *s ^= *s << 13;
    *s ^= *s >> 17;
    *s ^= *s << 5;
    (*s >> 8) as f32 / (1 << 24) as f32
}

/// One take of a row, made in code: the same samples for the same row and variant.
pub fn render(r: &Row, variant: u32) -> Vec<f32> {
    let mut seed = r.name.bytes().fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619)) ^ (variant + 1).wrapping_mul(0x9E3779B9) | 1;
    // a loop is made a little longer and its tail faded into its head, so it goes round without a seam
    let fade = if r.loops > 0.0 { (0.1 * RATE) as usize } else { 0 };
    let lap = (r.loops * RATE) as usize;
    // (in a loop a layer with a decay is a note: it rings on round the loop's end into its start)
    let note = |l: &Layer| r.loops > 0.0 && l.decay > 0.0;
    let n_of = |l: &Layer| if r.loops > 0.0 && !note(l) { lap + fade } else { ((l.attack + l.decay) * RATE) as usize };
    let len = if r.loops > 0.0 { lap + fade } else { r.layer.iter().map(|l| (l.delay * RATE) as usize + (n_of(l) as f32 * 1.16) as usize).max().unwrap_or(0) };
    let mut out = vec![0.0f32; len];
    let mut ring = vec![0.0f32; if r.layer.iter().any(note) { lap } else { 0 }];
    let mut one = Vec::new();
    for l in &r.layer {
        let tune = 1.0 + r.pitch * (rand(&mut seed) - 0.5);
        // each take of a one-shot is a little longer or shorter and weighs its layers its own way, so the
        // ear does not learn it (bug report 2026-11-11_12-56-13)
        let (long, weight) = if r.loops == 0.0 && r.variants > 1 { (1.0 + 0.3 * (rand(&mut seed) - 0.5), 1.0 + 0.3 * (rand(&mut seed) - 0.5)) } else { (1.0, 1.0) };
        let l = &Layer { decay: l.decay * long, gain: l.gain * weight, ..l.clone() };
        let (n, start) = (n_of(l), (l.delay * RATE) as usize);
        let (mut low, mut band, mut phase) = ([0.0f32; 2], [0.0f32; 2], 0.0f32);
        one.clear();
        for i in 0..n {
            let t = i as f32 / RATE;
            let along = if r.loops > 0.0 && !note(l) { 0.0 } else { t / (l.attack + l.decay) };
            let f = l.freq[0] * (l.freq[1] / l.freq[0]).powf(along) * tune;
            let s = match l.wave {
                Wave::Noise => {
                    // a state-variable filter's band-pass (stable while f stays under a sixth of the rate)
                    let fc = 2.0 * (std::f32::consts::PI * f.min(RATE / 6.5) / RATE).sin();
                    // twice over, so what lies far from `freq` is really gone (once leaves a hiss on a thud)
                    let mut x = rand(&mut seed) * 2.0 - 1.0;
                    for k in 0..2 {
                        low[k] += fc * band[k];
                        band[k] += fc * (x - low[k] - band[k] / l.q);
                        x = band[k];
                    }
                    x
                }
                w => {
                    phase = (phase + f / RATE).fract();
                    match w {
                        Wave::Sine => (std::f32::consts::TAU * phase).sin(),
                        Wave::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
                        _ => 0.5f32.copysign(0.5 - phase),
                    }
                }
            };
            let env = if r.loops > 0.0 && !note(l) {
                1.0
            } else if t < l.attack {
                t / l.attack
            } else {
                (-6.9 * (t - l.attack) / l.decay).exp()
            };
            one.push(s * env);
        }
        let peak = one.iter().fold(1e-9f32, |m, s| m.max(s.abs()));
        for (i, s) in one.iter().enumerate() {
            if note(l) {
                ring[(start + i) % lap] += s / peak * l.gain;
            } else {
                out[start + i] += s / peak * l.gain;
            }
        }
    }
    if fade > 0 {
        let keep = len - fade;
        for i in 0..fade {
            let w = i as f32 / fade as f32;
            out[i] = out[i] * w.sqrt() + out[keep + i] * (1.0 - w).sqrt();
        }
        out.truncate(keep);
        out.iter_mut().zip(&ring).for_each(|(o, n)| *o += n);
    }
    let peak = out.iter().fold(1e-9f32, |m, s| m.max(s.abs()));
    out.iter_mut().for_each(|s| *s *= r.gain / peak);
    out
}

#[derive(Clone)]
pub struct Clip {
    pub data: Arc<[f32]>,
    pub rate: f32,
}

/// A recording as one channel of samples. shortcut: WAV only (16 or 24 bit, or float); a compressed format
/// when music arrives.
fn wav(path: &Path) -> Result<Clip, String> {
    let mut r = hound::WavReader::open(path).map_err(|e| e.to_string())?;
    let spec = r.spec();
    let all: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().collect::<Result<_, _>>(),
        hound::SampleFormat::Int => {
            let full = (1i64 << (spec.bits_per_sample - 1)) as f32;
            r.samples::<i32>().map(|s| s.map(|s| s as f32 / full)).collect::<Result<_, _>>()
        }
    }
    .map_err(|e| e.to_string())?;
    let ch = spec.channels.max(1) as usize;
    let data: Vec<f32> = all.chunks(ch).map(|c| c.iter().sum::<f32>() / ch as f32).collect();
    if data.is_empty() || spec.sample_rate == 0 {
        return Err("no samples, or no rate".into());
    }
    Ok(Clip { data: data.into(), rate: spec.sample_rate as f32 })
}

pub const ROWS_TOML: &str = include_str!("../../../data/sounds.toml");

/// `sound_dir=dir`: the folder that holds `sounds.toml` and `sounds/` (`data` when unset), set once at start.
pub static DIR: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

/// The sounds' file in use (`sound_dir=`).
pub fn rows_file() -> std::path::PathBuf {
    files().0
}

/// The sounds' file and folder under `DIR`.
fn files() -> (std::path::PathBuf, std::path::PathBuf) {
    let d = DIR.get().map_or(Path::new("data"), |p| p.as_path());
    (d.join("sounds.toml"), d.join("sounds"))
}

/// The rows a run starts with (ADR 0048, Review): the file on disk when it is there and parses, else the
/// compiled-in rows; also why a file that is there was not used.
pub fn start_rows(toml: &Path) -> (Vec<Row>, Option<String>) {
    let compiled = || parse(ROWS_TOML).expect("the compiled-in data/sounds.toml");
    match std::fs::read_to_string(toml) {
        Err(_) => (compiled(), None),
        Ok(t) => match parse(&t) {
            Ok(rows) => (rows, None),
            Err(e) => (compiled(), Some(format!("{} refused, the built-in sounds play: {e}", toml.display()))),
        },
    }
}

/// Every row with its takes, ready to play.
pub struct Bank {
    pub rows: Vec<Row>,
    pub clips: Vec<Vec<Clip>>,
    /// The recordings under `<dir>/base/` by name: what rows may be made `from` besides other rows.
    pub bases: BTreeMap<String, Clip>,
    /// What each row's takes were made of (the row, its bases' keys, its recordings' file times): a reload
    /// keeps the takes of a row whose key is the same.
    keys: Vec<u64>,
}

/// A file's time and length, so a change to it is seen.
fn stamp(p: &Path) -> Option<(std::time::SystemTime, u64)> {
    let m = std::fs::metadata(p).ok()?;
    Some((m.modified().ok()?, m.len()))
}

impl Bank {
    /// `build` with no old bank, what went wrong printed.
    pub fn load(rows: Vec<Row>, dir: Option<&Path>) -> Bank {
        let (bank, errors) = Bank::build(rows, dir, None);
        for e in errors {
            eprintln!("[technoape] {e}");
        }
        bank
    }

    /// Each row's takes: the recordings `<dir>/<name>.wav` and `<name>.1.wav` to `.8.wav` when there are
    /// any, else made in code (`make`), a row's bases before it. The takes of a row whose key `old` has too
    /// are kept, not made again. Also what went wrong; a row that could not be made gets a silent take.
    pub fn build(rows: Vec<Row>, dir: Option<&Path>, old: Option<&Bank>) -> (Bank, Vec<String>) {
        use std::hash::{Hash, Hasher};
        let (mut errors, mut bases, mut base_keys) = (Vec::new(), BTreeMap::new(), BTreeMap::new());
        for p in dir.and_then(|d| std::fs::read_dir(d.join("base")).ok()).into_iter().flatten().flatten().map(|e| e.path()) {
            let Some(name) = p.file_stem().and_then(|s| s.to_str()).filter(|_| p.extension().is_some_and(|e| e == "wav")) else { continue };
            match wav(&p) {
                Ok(c) => {
                    bases.insert(name.to_string(), c);
                    base_keys.insert(name.to_string(), stamp(&p));
                }
                Err(e) => errors.push(format!("sound base {}: {e}", p.display())),
            }
        }
        let (mut clips, mut keys): (Vec<Option<Vec<Clip>>>, _) = (vec![None; rows.len()], vec![0u64; rows.len()]);
        let silent = || vec![Clip { data: vec![0.0f32].into(), rate: RATE }];
        loop {
            let mut made = false;
            for (k, r) in rows.iter().enumerate() {
                if clips[k].is_some() {
                    continue;
                }
                // its bases first: a row's key, or a recording's file time
                let deps: Option<Vec<u64>> = r
                    .from
                    .iter()
                    .map(|b| match rows.iter().position(|o| o.name == b.base) {
                        Some(j) => clips[j].as_ref().map(|_| keys[j]),
                        None => Some(base_keys.get(&b.base).map_or(1, |t| {
                            let mut h = std::hash::DefaultHasher::new();
                            t.hash(&mut h);
                            h.finish()
                        })),
                    })
                    .collect();
                let Some(deps) = deps else { continue };
                made = true;
                let files: Vec<std::path::PathBuf> = dir.into_iter().flat_map(|d| std::iter::once(format!("{}.wav", r.name)).chain((1..=8).map(|k| format!("{}.{k}.wav", r.name))).map(move |n| d.join(n))).filter(|p| p.exists()).collect();
                let mut h = std::hash::DefaultHasher::new();
                (format!("{r:?}"), &deps, files.iter().map(|f| (f, stamp(f))).collect::<Vec<_>>()).hash(&mut h);
                keys[k] = h.finish();
                if let Some(o) = old.and_then(|o| o.rows.iter().position(|x| x.name == r.name).filter(|&j| o.keys[j] == keys[k]).map(|j| &o.clips[j])) {
                    clips[k] = Some(o.clone());
                    continue;
                }
                let recorded: Vec<Clip> = files
                    .iter()
                    .filter_map(|p| wav(p).map_err(|e| errors.push(format!("sound {}: {e}", p.display()))).ok())
                    .map(|c| Clip { data: c.data.iter().map(|s| s * r.gain).collect(), rate: c.rate })
                    .collect();
                let takes = if !recorded.is_empty() {
                    recorded
                } else {
                    let base = |name: &str| match rows.iter().position(|o| o.name == name) {
                        Some(j) => clips[j].clone().map(|c| (c, rows[j].loops > 0.0)),
                        None => bases.get(name).map(|c| (vec![c.clone()], false)),
                    };
                    match make(r, None, &base) {
                        Ok(t) => t.into_iter().map(|d| Clip { data: d.into(), rate: RATE }).collect(),
                        Err(e) => {
                            errors.push(e);
                            silent()
                        }
                    }
                };
                clips[k] = Some(takes);
            }
            if !made {
                break;
            }
        }
        // (only a row list `parse` never saw can be made from itself)
        let clips = clips.into_iter().zip(&rows).map(|(c, r)| c.unwrap_or_else(|| (errors.push(format!("sound '{}': made from itself", r.name)), silent()).1)).collect();
        (Bank { rows, clips, bases, keys }, errors)
    }
}

/// A row's takes as samples at `RATE`: from its layers (`render`) or its bases, then its chain, each take
/// with its own draw of the spread. `skip` leaves one step of the chain out. `base` gives a base's takes by
/// name, and whether it goes round.
fn make(r: &Row, skip: Option<usize>, base: &dyn Fn(&str) -> Option<(Vec<Clip>, bool)>) -> Result<Vec<Vec<f32>>, String> {
    let lap = (r.loops * RATE) as usize;
    let hash = r.name.bytes().fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619));
    (0..r.variants)
        .map(|v| {
            let mut x = if r.from.is_empty() {
                render(r, v)
            } else {
                let mut x = vec![0.0f32; lap];
                for b in &r.from {
                    let (takes, round) = base(&b.base).ok_or_else(|| format!("sound '{}': no base '{}' (a row, or data/sounds/base/{}.wav)", r.name, b.base, b.base))?;
                    // each take of the row on its own take of the base, where it has several
                    let c = &takes[v as usize % takes.len()];
                    let s = if c.rate == RATE { c.data.to_vec() } else { chain::resample(&c.data, c.rate / RATE) };
                    let start = (b.delay * RATE) as usize;
                    if lap > 0 {
                        // in a loop a base that goes round fills it; one that plays once is a note, rung round its end
                        for i in 0..if round { lap } else { s.len() } {
                            x[(start + i) % lap] += s[i % s.len()] * b.gain;
                        }
                    } else {
                        x.resize(x.len().max(start + s.len()), 0.0);
                        x[start..].iter_mut().zip(&s).for_each(|(o, s)| *o += s * b.gain);
                    }
                }
                let peak = x.iter().fold(1e-9f32, |m, s| m.max(s.abs()));
                x.iter_mut().for_each(|s| *s *= r.gain / peak);
                x
            };
            if r.chain.is_empty() {
                return Ok(x);
            }
            let mut seed = hash ^ (v + 1).wrapping_mul(0x85EBCA6B) | 1;
            // a skipped step draws nothing, as a step written off (left out by `parse`) does
            let steps: Vec<chain::Step> = r.chain.iter().enumerate().filter(|(i, _)| Some(*i) != skip).map(|(_, s)| s.spread(&r.spread, &mut || rand(&mut seed) * 2.0 - 1.0)).collect();
            if lap > 0 {
                // three laps through the chain, so what rings over a lap's end comes round into the next; the
                // middle one is kept, with its tail faded into its head (as `render` does a loop)
                let period = steps.iter().fold(lap as f32, |p, s| match s.name() {
                    "speed" => p / s.n[0],
                    "stretch" => p * s.n[0],
                    _ => p,
                }) as usize;
                let mut y = x.repeat(3);
                for s in &steps {
                    y = chain::apply(y, s);
                }
                let mut seg = y[period.min(y.len())..(2 * period + (0.1 * RATE) as usize).min(y.len())].to_vec();
                let fade = seg.len().saturating_sub(period).min(period / 4);
                for i in 0..fade {
                    let w = i as f32 / fade as f32;
                    seg[i] = seg[i] * w.sqrt() + seg[period + i] * (1.0 - w).sqrt();
                }
                seg.truncate(period);
                x = seg;
            } else {
                for s in &steps {
                    x = chain::apply(x, s);
                }
                // shortcut: a take is cut at a minute, unfaded; a row that long wants a fade step
                x.truncate((60.0 * RATE) as usize);
            }
            // never past the row's gain, however the chain built up
            let peak = x.iter().fold(1e-9f32, |m, s| m.max(s.abs()));
            if peak > r.gain {
                x.iter_mut().for_each(|s| *s *= r.gain / peak);
            }
            Ok(x)
        })
        .collect()
}

/// For the Studio: every take of `r` as samples at `RATE`, made as the game makes them from the bank's
/// rows and recordings (`r` itself need not be in the bank: an edited row), `skip` with one chain step off.
/// Its own recordings are not used: this is the row as made in code.
pub fn render_row(bank: &Bank, r: &Row, skip: Option<usize>) -> Result<Vec<Vec<f32>>, String> {
    let base = |name: &str| match bank.rows.iter().position(|o| o.name == name) {
        Some(j) => Some((bank.clips[j].clone(), bank.rows[j].loops > 0.0)),
        None => bank.bases.get(name).map(|c| (vec![c.clone()], false)),
    };
    make(r, skip, &base)
}

#[cfg(test)]
fn rows_for_tests() -> Vec<Row> {
    parse(ROWS_TOML).unwrap()
}

pub enum Msg {
    /// Play a row's take once: how loud, at what pitch, where from the head (metres right, up, ahead; none
    /// for a sound that is nowhere), and how dull (0..1).
    Play { row: u16, take: u8, gain: f32, pitch: f32, rel: Option<[f32; 3]>, dull: f32 },
    /// A row that goes round: how loud and how dull it is now. It starts with the first of these.
    Hold { row: u16, gain: f32, dull: f32 },
    Levels { master: f32, bus: [f32; 4] },
    /// Samples that are no row (the Studio's audition): played once as a row of this reach and bus would be.
    Clip { clip: Clip, reach: f32, bus: Bus, gain: f32, rel: Option<[f32; 3]>, dull: f32 },
    /// The sounds made again (a reload): what plays on plays out, a held row's new take fades in over its old.
    Bank(Arc<Bank>),
    /// The place the head is in (`room`): how much of the cave's reverb (0..1) and how big it is (metres).
    Room { wet: f32, size: f32 },
}

/// How far the rays of `room` look, in metres.
pub const ROOM_RAY: f64 = 24.0;

/// How closed in the head is (ADR 0054): 14 rays from it, the six ways and eight corners of a cube stood on
/// the body's up (`ear[2]`; not the look, so pitching the head changes nothing), each out to the cube's face
/// `ROOM_RAY` away (a corner ray goes √3 times as far) in steps of a metre along each axis. Gives the reverb's
/// share, 0 where fewer than half the rays meet something (a field: only the ground's), 1 where all do, and
/// the room's size, the mean distance of the sideways and upward hits (the floor under the feet says
/// nothing of how big it is).
pub fn room(solid: &dyn Fn(DVec3) -> bool, ear: &[DVec3; 4]) -> (f32, f32) {
    let up = ear[2].normalize();
    let (a, b) = up.any_orthonormal_pair();
    let (mut hits, mut over, mut far) = (0u32, 0u32, 0.0f64);
    for x in -1..=1i32 {
        for y in -1..=1i32 {
            for z in -1..=1i32 {
                // the six ways (one axis) and the eight corners (all three); not the twelve edges
                if (x * x + y * y + z * z) % 2 == 0 {
                    continue;
                }
                let d = a * x as f64 + up * y as f64 + b * z as f64;
                if let Some(t) = (1..=ROOM_RAY as i32).map(|t| t as f64).find(|t| solid(ear[0] + d * *t)) {
                    hits += 1;
                    if y >= 0 {
                        (over, far) = (over + 1, far + t * d.length());
                    }
                }
            }
        }
    }
    let wet = ((hits as f32 / 14.0 - 0.5) * 2.0).clamp(0.0, 1.0);
    (wet, if over == 0 { 0.0 } else { (far / over as f64) as f32 })
}

/// The cave's reverb (ADR 0054): one for every sound in the world, after Freeverb as `chain::reverb` is, each
/// ear its own combs a little apart. It runs on the card's thread, sample by sample.
struct Reverb {
    /// Four combs an ear (its samples and its low-pass's state), then two all-passes an ear.
    combs: Vec<(Vec<f32>, f32)>,
    passes: Vec<Vec<f32>>,
    /// Each comb's feedback, eased toward the room's (a new size while it rings never steps).
    fb: [f32; 8],
    fb_want: [f32; 8],
    n: usize,
    /// How much of it is heard, eased (so a change never clicks), and how much is wanted.
    wet: f32,
    want: f32,
}

impl Reverb {
    fn new(rate: f32) -> Reverb {
        let s = rate / 44100.0;
        let len = |d: f32| vec![0.0f32; (d * s) as usize];
        // the right ear's 23 samples longer, as Freeverb's
        let combs = [0.0, 23.0].iter().flat_map(|o| [1557.0, 1617.0, 1491.0, 1422.0].map(|d| (len(d + o), 0.0))).collect();
        let passes = [0.0, 23.0].iter().flat_map(|o| [556.0, 441.0].map(|d| len(d + o))).collect();
        let mut r = Reverb { combs, passes, fb: [0.0; 8], fb_want: [0.0; 8], n: 0, wet: 0.0, want: 0.0 };
        r.size(4.0, rate);
        r.fb = r.fb_want;
        r
    }

    /// A room `size` metres across rings from 0.4 s (a burrow) to 3 s (a hall of `ROOM_RAY`) to a thousandth.
    fn size(&mut self, size: f32, rate: f32) {
        let t60 = (0.4 + 2.6 * (size / ROOM_RAY as f32).clamp(0.0, 1.0)) * rate;
        for (k, (c, _)) in self.combs.iter().enumerate() {
            self.fb_want[k] = 0.001f32.powf(c.len() as f32 / t60);
        }
    }

    /// `x` the sounds sent to it, mono; gives each ear's echo of the room.
    fn tick(&mut self, x: f32) -> [f32; 2] {
        self.wet += (self.want - self.wet) * 0.001;
        // a floor far under hearing, so silence rings down to it and never into denormals (slow on x86)
        let x = x + 1e-20;
        let mut out = [0.0f32; 2];
        for (e, o) in out.iter_mut().enumerate() {
            for k in 4 * e..4 * e + 4 {
                self.fb[k] += (self.fb_want[k] - self.fb[k]) * 0.001;
                let (buf, low) = &mut self.combs[k];
                let i = self.n % buf.len();
                let y = buf[i];
                *low = y * 0.8 + *low * 0.2;
                buf[i] = x * (1.0 - self.fb[k]) + *low * self.fb[k];
                *o += y * 0.25;
            }
            for buf in &mut self.passes[2 * e..2 * e + 2] {
                let i = self.n % buf.len();
                let y = buf[i];
                buf[i] = *o + y * 0.5;
                *o = y - *o;
            }
            *o *= self.wet * ROOM_MIX;
        }
        self.n += 1;
        out
    }

    /// Out of every cave and eased there: silent, so it is not run (what rings in it is let go).
    fn idle(&mut self) -> bool {
        if self.want > 0.0 || self.wet > 1e-4 {
            return false;
        }
        if self.wet > 0.0 {
            self.wet = 0.0;
            self.combs.iter_mut().for_each(|(b, l)| (b.fill(0.0), *l = 0.0).1);
            self.passes.iter_mut().for_each(|b| b.fill(0.0));
        }
        true
    }
}

/// How loud a cave's reverb is under its sounds, at its fullest.
const ROOM_MIX: f32 = 0.5;

/// What the mixer reports, for F3 and the run's JSON.
#[derive(Default)]
pub struct Stats {
    pub rate: AtomicU32,
    pub voices_peak: AtomicU32,
    pub mix_us_max: AtomicU32,
    /// The longest the sound card said a buffer waits before it is heard, in microseconds.
    pub delay_us: AtomicU32,
    pub device: Mutex<String>,
    /// The game's side is gone, or the card is (unplugged): the card's thread closes.
    gone: AtomicBool,
}

struct Voice {
    clip: Clip,
    /// Where it is in the clip, in samples, and how far a frame moves it.
    at: f64,
    step: f64,
    /// Each ear: its share and how many samples later it hears.
    pan: [f32; 2],
    lag: [f64; 2],
    /// How loud it is and is going to be (a held row is eased, so a change never clicks).
    level: f32,
    want: f32,
    /// The low-pass's weight and each ear's state.
    lp: f32,
    z: [f32; 2],
    bus: usize,
    held: Option<u16>,
    /// It goes round (held, or a held row's old take fading out after a reload).
    wrap: bool,
}

pub struct Mixer {
    rx: mpsc::Receiver<Msg>,
    bank: Arc<Bank>,
    rate: f32,
    voices: Vec<Voice>,
    master: (f32, f32),
    bus: [f32; 4],
    stats: Arc<Stats>,
    room: Reverb,
    /// What the world's sounds send the reverb this buffer, mono.
    send: Vec<f32>,
}

impl Mixer {
    pub fn new(rx: mpsc::Receiver<Msg>, bank: Arc<Bank>, rate: f32, stats: Arc<Stats>) -> Mixer {
        stats.rate.store(rate as u32, Relaxed);
        Mixer { rx, bank, rate, voices: Vec::with_capacity(VOICES), master: (1.0, 1.0), bus: [1.0; 4], stats, room: Reverb::new(rate), send: Vec::with_capacity(16384) }
    }

    /// The low-pass's weight for a dullness of 0 (open) to 1 (800 Hz).
    fn lp(&self, dull: f32) -> f32 {
        let fc = 18000.0 * (800.0f32 / 18000.0).powf(dull.clamp(0.0, 1.0));
        1.0 - (-std::f32::consts::TAU * fc / self.rate).exp()
    }

    fn take(&mut self, m: Msg) {
        match m {
            Msg::Levels { master, bus } => (self.master.1, self.bus) = (master, bus),
            Msg::Hold { row, gain, dull } => {
                let lp = self.lp(dull);
                if let Some(k) = self.voices.iter().position(|v| v.held == Some(row)) {
                    (self.voices[k].want, self.voices[k].lp) = (gain, lp);
                } else if let (Some(c), true) = (self.bank.clips.get(row as usize).and_then(|c| c.first()), gain > 0.0 && self.voices.len() < VOICES) {
                    let bus = self.bank.rows[row as usize].bus as usize;
                    self.voices.push(Voice { clip: c.clone(), at: 0.0, step: (c.rate / self.rate) as f64, pan: [0.707; 2], lag: [0.0; 2], level: 0.0, want: gain, lp, z: [0.0; 2], bus, held: Some(row), wrap: true });
                }
            }
            Msg::Play { row, take, gain, pitch, rel, dull } => {
                let Some(r) = self.bank.rows.get(row as usize) else { return };
                let c = &self.bank.clips[row as usize];
                self.start(c[take as usize % c.len()].clone(), r.reach, r.bus, gain, pitch, rel, dull);
            }
            Msg::Clip { clip, reach, bus, gain, rel, dull } => self.start(clip, reach, bus, gain, 1.0, rel, dull),
            Msg::Room { wet, size } => {
                self.room.want = wet.clamp(0.0, 1.0);
                self.room.size(size, self.rate);
            }
            Msg::Bank(b) => {
                for k in 0..self.voices.len() {
                    let Some(row) = self.voices[k].held else { continue };
                    let name = &self.bank.rows[row as usize].name;
                    let (now, room) = (b.rows.iter().position(|r| r.name == *name), self.voices.len() < VOICES);
                    let v = &mut self.voices[k];
                    match now.map(|j| (j, &b.clips[j][0])) {
                        Some((j, c)) if Arc::ptr_eq(&c.data, &v.clip.data) => v.held = Some(j as u16),
                        Some((j, c)) if room => {
                            let new = Voice { clip: c.clone(), at: 0.0, step: (c.rate / self.rate) as f64, level: 0.0, held: Some(j as u16), ..*v };
                            (v.held, v.want) = (None, 0.0);
                            self.voices.push(new);
                        }
                        // shortcut: with every voice taken the new take starts where the old one was (it may click)
                        Some((j, c)) => (v.held, v.clip, v.at) = (Some(j as u16), c.clone(), v.at % c.data.len() as f64),
                        None => (v.held, v.want) = (None, 0.0),
                    }
                }
                // (the old bank's takes are let go on the card's thread, which keeps it a while: not freed here)
                self.bank = b;
            }
        }
    }

    /// A one-shot voice for a take: where it is and how far, as `Msg::Play` says.
    #[allow(clippy::too_many_arguments)]
    fn start(&mut self, c: Clip, reach: f32, bus: Bus, gain: f32, pitch: f32, rel: Option<[f32; 3]>, dull: f32) {
        let (mut pan, mut lag, mut gain, mut dull) = ([0.707; 2], [0.0; 2], gain, dull);
        if let Some(rel) = rel {
            let d = (rel[0] * rel[0] + rel[1] * rel[1] + rel[2] * rel[2]).sqrt().max(0.01);
            let (x, z) = (rel[0] / d, rel[2] / d);
            // quieter with distance and gone at the row's reach; left and right by where it is
            // around the head, the far ear up to 0.6 ms late; duller behind
            gain *= (1.0 - d / reach).max(0.0) / (1.0 + d * 0.25);
            let a = (x + 1.0) * std::f32::consts::FRAC_PI_4;
            pan = [a.cos(), a.sin()];
            lag = [(x.max(0.0) * 0.0006 * c.rate) as f64, ((-x).max(0.0) * 0.0006 * c.rate) as f64];
            dull = dull.max(-z * 0.5);
        }
        if gain < 0.001 {
            return;
        }
        if self.voices.len() == VOICES {
            // the quietest one-shot makes room, unless this one is quieter still
            let quiet = self.voices.iter().enumerate().filter(|(_, v)| v.held.is_none()).map(|(k, v)| (k, v.level)).min_by(|a, b| a.1.total_cmp(&b.1));
            match quiet {
                Some((k, level)) if level < gain => drop(self.voices.swap_remove(k)),
                _ => return,
            }
        }
        let v = Voice { step: (c.rate / self.rate * pitch) as f64, clip: c, at: 0.0, pan, lag, level: gain, want: gain, lp: self.lp(dull), z: [0.0; 2], bus: bus as usize, held: None, wrap: false };
        self.voices.push(v);
    }

    /// Fills `out` (left and right in turn) with the next stretch of sound.
    pub fn mix(&mut self, out: &mut [f32]) {
        let t0 = std::time::Instant::now();
        loop {
            match self.rx.try_recv() {
                Ok(m) => self.take(m),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.stats.gone.store(true, Relaxed);
                    break;
                }
            }
        }
        out.fill(0.0);
        let frames = out.len() / 2;
        let wet = !self.room.idle();
        self.send.clear();
        self.send.resize(if wet { frames } else { 0 }, 0.0);
        for v in &mut self.voices {
            let (d, n, bus) = (&v.clip.data, v.clip.data.len(), self.bus[v.bus]);
            // the world's sounds and the steps are in the cave; the interface and the music are in the head
            let send = wet && v.bus <= Bus::Steps as usize;
            for f in 0..frames {
                v.level += (v.want - v.level) * 0.001;
                for e in 0..2 {
                    let p = v.at - v.lag[e];
                    let i = p as usize;
                    let s = if p < 0.0 || i >= n {
                        0.0
                    } else {
                        let next = if i + 1 < n { d[i + 1] } else if v.wrap { d[0] } else { 0.0 };
                        d[i] + (next - d[i]) * (p - i as f64) as f32
                    };
                    v.z[e] += v.lp * (s - v.z[e]);
                    let s = v.z[e] * v.pan[e] * v.level * bus;
                    out[2 * f + e] += s;
                    if send {
                        self.send[f] += s * 0.5;
                    }
                }
                v.at += v.step;
                if v.wrap && v.at >= n as f64 {
                    v.at -= n as f64;
                }
            }
        }
        // a one-shot is done when its later ear has heard its end
        // (a held row that has faded to nothing gives its voice back: it starts anew when it is wanted)
        self.voices.retain(|v| if v.wrap { v.want > 0.0 || v.level > 1e-4 } else { v.at - v.lag[0].max(v.lag[1]) < v.clip.data.len() as f64 });
        for (f, x) in self.send.iter().enumerate() {
            let [l, r] = self.room.tick(*x);
            (out[2 * f], out[2 * f + 1]) = (out[2 * f] + l, out[2 * f + 1] + r);
        }
        // the master eases over the buffer (a mute never clicks); tanh keeps a loud moment from clipping
        let (from, to) = self.master;
        for f in 0..frames {
            let g = from + (to - from) * (f + 1) as f32 / frames as f32;
            out[2 * f] = (out[2 * f] * g).tanh();
            out[2 * f + 1] = (out[2 * f + 1] * g).tanh();
        }
        self.master.0 = to;
        self.stats.voices_peak.fetch_max(self.voices.len() as u32, Relaxed);
        self.stats.mix_us_max.fetch_max(t0.elapsed().as_micros() as u32, Relaxed);
    }
}

/// Opens the system's sound card and plays the mixer on it until the game's side is gone. Runs on its own
/// thread: the takes are made here, off the frame thread.
fn card(rx: mpsc::Receiver<Msg>, rows: Vec<Row>, mut seen: Vec<(std::path::PathBuf, Option<(std::time::SystemTime, u64)>)>, stats: Arc<Stats>, fresh: Fresh) -> Result<(), String> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    let dev = cpal::default_host().default_output_device().ok_or("no sound device")?;
    let cfg = dev.default_output_config().map_err(|e| e.to_string())?;
    if cfg.sample_format() != cpal::SampleFormat::F32 {
        // shortcut: the systems' shared mixers all take floats; convert here if a card ever does not
        return Err(format!("the sound card wants {:?} samples", cfg.sample_format()));
    }
    let (ch, rate) = (cfg.channels() as usize, cfg.sample_rate().0);
    // 10 ms buffers where the card lets us choose (fundamentals.md: a sound starts within 20 ms)
    let size = match cfg.buffer_size() {
        cpal::SupportedBufferSize::Range { min, max } => cpal::BufferSize::Fixed((rate / 100).clamp(*min, *max)),
        _ => cpal::BufferSize::Default,
    };
    let mut config: cpal::StreamConfig = cfg.into();
    config.buffer_size = size;
    *stats.device.lock().unwrap() = dev.name().unwrap_or_default();
    let (toml, dir) = files();
    let (toml, dir) = (toml.as_path(), dir.as_path());
    // (`seen`: the files' times when `rows` were read; a change since is a reload at the first look)
    let bank = Arc::new(Bank::load(rows, Some(dir)));
    // what was asked for while the takes were being made is old news; levels and held rows still count
    let mut mixer = Mixer::new(rx, bank.clone(), rate as f32, stats.clone());
    for m in mixer.rx.try_iter().collect::<Vec<_>>() {
        if !matches!(m, Msg::Play { .. } | Msg::Clip { .. }) {
            mixer.take(m);
        }
    }
    let mut pair = vec![0.0f32; 2 * 16384];
    let (st, lost) = (stats.clone(), stats.clone());
    let stream = dev
        .build_output_stream(
            &config,
            move |data: &mut [f32], info: &cpal::OutputCallbackInfo| {
                let frames = data.len() / ch;
                if ch == 2 {
                    mixer.mix(data);
                } else if frames * 2 <= pair.len() {
                    mixer.mix(&mut pair[..frames * 2]);
                    data.fill(0.0);
                    for f in 0..frames {
                        if ch == 1 {
                            data[f] = (pair[2 * f] + pair[2 * f + 1]) * 0.5;
                        } else {
                            data[f * ch..f * ch + 2].copy_from_slice(&pair[2 * f..2 * f + 2]);
                        }
                    }
                } else {
                    data.fill(0.0);
                }
                let t = info.timestamp();
                if let Some(d) = t.playback.duration_since(&t.callback) {
                    st.delay_us.fetch_max(d.as_micros() as u32 + (frames as u64 * 1_000_000 / rate as u64) as u32, Relaxed);
                }
            },
            move |e| {
                eprintln!("[technoape] sound: {e}");
                lost.gone.store(true, Relaxed);
            },
            None,
        )
        .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    // live reload (ADR 0048): twice a second the files are looked at, and a change is made again here, off
    // the frame thread and the card's; the game's side passes the new bank on in order with its plays. The
    // last two banks are kept, so what still plays from one is not freed on the card's thread.
    let (mut now, mut kept, mut tick) = (bank, Vec::new(), 0u32);
    while !stats.gone.load(Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(50));
        tick += 1;
        if tick % 10 != 0 || stamps(toml, dir) == seen {
            continue;
        }
        seen = stamps(toml, dir);
        match reload(&now, toml, dir) {
            Ok(b) => {
                // the rows whose first take's samples changed: a check without ears
                let changed: Vec<&str> = b.rows.iter().zip(&b.clips).filter(|(r, c)| now.rows.iter().position(|o| o.name == r.name).is_none_or(|i| now.clips[i][0].data[..] != c[0].data[..])).map(|(r, _)| r.name.as_str()).collect();
                eprintln!("[technoape] sounds made again from {}, samples changed: [{}]", toml.display(), changed.join(","));
                let b = Arc::new(b);
                kept.push(std::mem::replace(&mut now, b.clone()));
                if kept.len() > 2 {
                    kept.remove(0);
                }
                *fresh.lock().unwrap() = Some(b);
            }
            Err(e) => eprintln!("[technoape] {} refused, the old sounds stay: {e}", toml.display()),
        }
    }
    Ok(())
}

/// A bank made on the card's thread, waiting for the game's side to pass it on.
type Fresh = Arc<Mutex<Option<Arc<Bank>>>>;

/// The sounds' file and every file under their folder, with their times: a change to any is a reload.
fn stamps(toml: &Path, dir: &Path) -> Vec<(std::path::PathBuf, Option<(std::time::SystemTime, u64)>)> {
    let (mut v, mut todo) = (vec![(toml.to_path_buf(), stamp(toml))], vec![dir.to_path_buf()]);
    while let Some(d) = todo.pop() {
        for p in std::fs::read_dir(&d).into_iter().flatten().flatten().map(|e| e.path()) {
            if p.is_dir() {
                todo.push(p);
            } else {
                v.push((p.clone(), stamp(&p)));
            }
        }
    }
    v.sort();
    v
}

/// The sounds made again from the files as they are now, the takes of rows that did not change kept.
/// Refused, with the reasons, when the file does not parse, a row cannot be made, or a row is gone (the
/// game may still play it: rows are renamed or removed with the game closed).
pub fn reload(old: &Bank, toml: &Path, dir: &Path) -> Result<Bank, String> {
    let rows = parse(&std::fs::read_to_string(toml).map_err(|e| e.to_string())?)?;
    if let Some(r) = old.rows.iter().find(|o| !rows.iter().any(|r| r.name == o.name)) {
        return Err(format!("sound '{}': gone, and the game may play it", r.name));
    }
    let (bank, errors) = Bank::build(rows, Some(dir), Some(old));
    if errors.is_empty() {
        Ok(bank)
    } else {
        Err(errors.join("; "))
    }
}

/// The game's side: it names a sound and a place, this works out what the head hears of it and tells the
/// mixer. Without a sound card (off-screen runs) it only counts what would have played.
pub struct Audio {
    tx: mpsc::Sender<Msg>,
    rows: Vec<Row>,
    pub stats: Arc<Stats>,
    /// How often each row was played, for the run's JSON: a check without ears.
    pub played: BTreeMap<String, u32>,
    seed: u32,
    levels: (f32, [f32; 4]),
    /// The take each row played last (`next_take`).
    last: Vec<u8>,
    /// The head: where it is, and its right, up and ahead.
    pub ear: [DVec3; 4],
    /// How much of a sound the air carries here (0 in space, by the settings), and how dull the head hears
    /// everything (under water).
    pub carry: f32,
    pub dull: f32,
    open: bool,
    fresh: Fresh,
    /// When a lost card may be opened again.
    retry: std::time::Instant,
    /// The rows asked for that the bank lacks, each named once on stderr.
    missing: std::collections::BTreeSet<String>,
    /// The sounds' files as they were when `rows` were read: the card reloads from any change after.
    seen: Vec<(std::path::PathBuf, Option<(std::time::SystemTime, u64)>)>,
    /// The place the head is in (`room`: the reverb's share and the size), when it was last measured, and
    /// the longest a measure took, in microseconds.
    pub room: (f32, f32),
    room_at: Option<std::time::Instant>,
    room_us: u32,
}

impl Audio {
    pub fn new() -> Audio {
        // at start, not per frame: the files' times first, so a save while they are read is a reload later
        let (toml, dir) = files();
        let seen = stamps(&toml, &dir);
        let (rows, refused) = start_rows(&toml);
        if let Some(e) = refused {
            eprintln!("[technoape] {e}");
        }
        // the far end is dropped: until a card is opened the messages go nowhere
        Audio { tx: mpsc::channel().0, rows, stats: Arc::default(), played: BTreeMap::new(), seed: 0x2F6E2B1, levels: (-1.0, [0.0; 4]), last: Vec::new(), ear: [DVec3::ZERO, DVec3::X, DVec3::Y, DVec3::Z], carry: 1.0, dull: 0.0, open: false, fresh: Fresh::default(), retry: std::time::Instant::now(), missing: Default::default(), seen, room: (0.0, 0.0), room_at: None, room_us: 0 }
    }

    /// The cave's reverb: four times a second the rays of `room` from the head (`ear`) through the world's
    /// cells (`solid`), and the mixer told when the place changed. Called every frame. No rays without a card
    /// (an off-screen run measures with `sound=1`), nor more than `ROOM_RAY` over the highest place a block can be (`over_top`,
    /// metres): there the place is the open.
    pub fn listen(&mut self, solid: &dyn Fn(DVec3) -> bool, over_top: &dyn Fn(DVec3) -> f64) {
        if !self.open || self.room_at.is_some_and(|t| t.elapsed().as_millis() < 250) {
            return;
        }
        let t = std::time::Instant::now();
        let r = if over_top(self.ear[0]) > ROOM_RAY { (0.0, 0.0) } else { room(solid, &self.ear) };        self.room_us = self.room_us.max(t.elapsed().as_micros() as u32);
        self.room_at = Some(t);
        if (r.0 - self.room.0).abs() > 0.02 || (r.1 - self.room.1).abs() > 1.0 {
            self.room = r;
            let _ = self.tx.send(Msg::Room { wet: r.0, size: r.1 });
        }
    }

    /// A reload made on the card's thread: the rows by their new places from here on, in order with the plays.
    fn take_fresh(&mut self) {
        if let Some(b) = self.fresh.try_lock().ok().and_then(|mut f| f.take()) {
            (self.rows, self.last) = (b.rows.clone(), Vec::new());
            let _ = self.tx.send(Msg::Bank(b));
        }
    }

    /// Starts the sound card's thread; a machine without one plays on in silence.
    pub fn open(&mut self) {
        let (tx, rx) = mpsc::channel();
        // each card has its own stats: the old one's `gone` must not close the new one
        (self.tx, self.open, self.levels.0, self.stats, self.fresh) = (tx, true, -1.0, Arc::default(), Fresh::default());
        // a new card's mixer starts in the open: the place is told it again at the next measure
        (self.room, self.room_at) = ((0.0, 0.0), None);
        let (rows, stats, fresh, seen) = (self.rows.clone(), self.stats.clone(), self.fresh.clone(), self.seen.clone());
        std::thread::spawn(move || {
            if let Err(e) = card(rx, rows, seen, stats.clone(), fresh) {
                eprintln!("[technoape] no sound: {e}");
            }
            stats.gone.store(true, Relaxed);
        });
    }

    pub fn has(&self, name: &str) -> bool {
        self.rows.iter().any(|r| r.name == name)
    }

    /// Plays a row: at a place in the world (`None`: in the head, as the interface's are), `gain` and
    /// `pitch` times its own. `touch`: the player's own body made it, so it is heard through the suit
    /// where no air carries it. False when nothing of it is heard.
    pub fn play(&mut self, name: &str, at: Option<DVec3>, gain: f32, pitch: f32, touch: bool) -> bool {
        self.take_fresh();
        // (a model's event or state may name a row the file lacks: named once, skipped)
        let Some(k) = self.rows.iter().position(|r| r.name == name) else {
            if self.missing.insert(name.to_string()) {
                eprintln!("[technoape] no sound '{name}' in data/sounds.toml: not played");
            }
            return false;
        };
        let r = &self.rows[k];
        let (mut gain, mut dull) = (gain, 0.0f32);
        if r.bus != Bus::Ui {
            dull = self.dull;
            if touch {
                gain *= 0.5 + 0.5 * self.carry;
                dull = dull.max(1.0 - self.carry);
            } else {
                gain *= self.carry;
            }
        }
        let rel = at.map(|p| {
            let d = p - self.ear[0];
            [d.dot(self.ear[1]) as f32, d.dot(self.ear[2]) as f32, d.dot(self.ear[3]) as f32]
        });
        if gain < 0.001 || rel.is_some_and(|d| d[0] * d[0] + d[1] * d[1] + d[2] * d[2] >= r.reach * r.reach) {
            return false;
        }
        // never the same twice running: another take, and each play a little louder or softer, higher or lower
        self.last.resize(self.rows.len(), u8::MAX);
        let take = next_take(&mut self.seed, r.variants, self.last[k]);
        self.last[k] = take;
        let pitch = pitch * (1.0 + r.pitch * (rand(&mut self.seed) * 2.0 - 1.0));
        if r.bus != Bus::Ui {
            gain *= 0.85 + 0.3 * rand(&mut self.seed);
        }
        *self.played.entry(name.to_string()).or_default() += 1;
        let _ = self.tx.send(Msg::Play { row: k as u16, take, gain, pitch, rel, dull });
        true
    }

    /// A row that goes round (the wind): how loud it is now. Called every frame, so it is also where a
    /// lost card (unplugged, or none yet) is tried again, every two seconds.
    pub fn hold(&mut self, name: &str, gain: f32) {
        if !self.open {
            return;
        }
        if self.stats.gone.load(Relaxed) && self.retry.elapsed().as_secs() >= 2 {
            self.retry = std::time::Instant::now();
            self.open();
        }
        self.take_fresh();
        if let Some(k) = self.rows.iter().position(|r| r.name == name) {
            let _ = self.tx.send(Msg::Hold { row: k as u16, gain, dull: self.dull });
        }
    }

    /// For the Studio: plays samples (at `RATE`) once as a sound `distance` metres ahead is heard (0: in the
    /// head), on `bus`, gone at `reach`, `dull` 0..1 as under water. shortcut: a cave is only duller until the
    /// cave reverb (sound.md, second cut); the page may put a `reverb` step on it meanwhile.
    pub fn audition(&mut self, samples: &[f32], bus: Bus, distance: f32, reach: f32, dull: f32) {
        let rel = (distance > 0.0).then_some([0.0, 0.0, distance]);
        let _ = self.tx.send(Msg::Clip { clip: Clip { data: samples.into(), rate: RATE }, reach, bus, gain: 1.0, rel, dull });
    }

    /// The volume sliders, each 0..1; sent on when they change.
    pub fn levels(&mut self, master: f32, bus: [f32; 4]) {
        if self.levels != (master, bus) {
            self.levels = (master, bus);
            let _ = self.tx.send(Msg::Levels { master, bus });
        }
    }

    /// The run's JSON: the card, its delay, the mixer's cost and what was played.
    pub fn json(&self) -> String {
        let s = &self.stats;
        let played: Vec<String> = self.played.iter().map(|(k, n)| format!("\"{k}\":{n}")).collect();
        format!(
            "\"audio\":{{\"device\":\"{}\",\"rate\":{},\"delay_ms\":{:.1},\"voices_peak\":{},\"mix_us_max\":{},\"room\":{{\"wet\":{:.2},\"size\":{:.1},\"us_max\":{}}},\"played\":{{{}}}}},",
            s.device.lock().unwrap().replace(['"', '\\'], ""),
            s.rate.load(Relaxed),
            s.delay_us.load(Relaxed) as f32 / 1000.0,
            s.voices_peak.load(Relaxed),
            s.mix_us_max.load(Relaxed),
            self.room.0,
            self.room.1,
            self.room_us,
            played.join(",")
        )
    }
}

/// `sounds=1`: every row's takes in numbers (seconds, loudest sample, mean loudness, and about where its
/// pitch sits, from how often it crosses zero), one JSON line.
/// Also, per row, its bases and chain and how long its takes take to make (`ms`, made again for the count),
/// and how long the whole bank took (`load_ms`, as when the card opens).
pub fn report() -> String {
    let t0 = std::time::Instant::now();
    let (toml, dir) = files();
    let (rows, refused) = start_rows(&toml);
    if let Some(e) = refused {
        eprintln!("[technoape] {e}");
    }
    let bank = Bank::load(rows, Some(&dir));
    let load_ms = t0.elapsed().as_secs_f32() * 1000.0;
    let rows: Vec<String> = bank
        .rows
        .iter()
        .zip(&bank.clips)
        .map(|(r, cs)| {
            let c = &cs[0];
            let n = c.data.len() as f32;
            let peak = c.data.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            let rms = (c.data.iter().map(|s| s * s).sum::<f32>() / n).sqrt();
            let cross = c.data.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count() as f32;
            let t = std::time::Instant::now();
            let _ = render_row(&bank, r, None);
            let ms = t.elapsed().as_secs_f32() * 1000.0;
            let list = |v: Vec<String>| v.iter().map(|n| format!("\"{n}\"")).collect::<Vec<_>>().join(",");
            let made = format!("\"from\":[{}],\"chain\":[{}]", list(r.from.iter().map(|b| b.base.clone()).collect()), list(r.chain.iter().map(|s| s.name().to_string()).collect()));
            // the files `Bank::build` plays for the row (a 48 kHz recording looked like a made take by its rate)
            let rec: Vec<String> = std::iter::once(format!("{}.wav", r.name)).chain((1..=8).map(|k| format!("{}.{k}.wav", r.name))).filter(|f| dir.join(f).exists()).collect();
            format!("{{\"name\":\"{}\",\"takes\":{},\"made_in_code\":{},\"recordings\":[{}],\"seconds\":{:.3},\"peak\":{:.3},\"rms\":{:.3},\"hz\":{:.0},{made},\"ms\":{ms:.2}}}", r.name, cs.len(), rec.is_empty(), list(rec), n / c.rate, peak, rms, cross / 2.0 / (n / c.rate))
        })
        .collect();
    format!("{{\"sounds\":[{}],\"load_ms\":{load_ms:.1}}}", rows.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::blocks::BLOCKS;

    fn rows() -> Vec<Row> {
        parse(ROWS_TOML).unwrap()
    }
    fn mixer() -> (mpsc::Sender<Msg>, Mixer, Vec<Row>) {
        let (tx, rx) = mpsc::channel();
        let rows = rows();
        (tx, Mixer::new(rx, Arc::new(Bank::load(rows.clone(), None)), RATE, Arc::default()), rows)
    }
    fn row(rows: &[Row], name: &str) -> u16 {
        rows.iter().position(|r| r.name == name).unwrap() as u16
    }
    fn energy(b: &[f32], ear: usize) -> f32 {
        b.iter().skip(ear).step_by(2).map(|s| s * s).sum()
    }

    #[test]
    fn every_row_makes_a_clean_sound() {
        let bank = Bank::load(rows(), None);
        for (r, cs) in bank.rows.iter().zip(&bank.clips) {
            let a = &cs[0].data[..];
            assert_eq!(a, &render_row(&bank, r, None).unwrap()[0][..], "{}: the same take twice", r.name);
            let peak = a.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            let rms = (a.iter().map(|s| s * s).sum::<f32>() / a.len() as f32).sqrt();
            // (a chain may lower a take's loudest sample, never raise it past the row's gain)
            assert!(((peak - r.gain).abs() < 1e-3 || (!r.chain.is_empty() && peak < r.gain)) && peak <= 1.0, "{}: peak {peak}", r.name);
            assert!(rms > 0.01 * r.gain, "{}: all but silent, rms {rms}", r.name);
            assert!(a.len() as f32 / RATE <= 60.0, "{}: {} s", r.name, a.len() as f32 / RATE);
            if r.loops > 0.0 {
                // no seam: the step from its end to its start is no bigger than its steps anywhere else
                let big = a.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0f32, f32::max);
                assert!((a[0] - a[a.len() - 1]).abs() <= big, "{}: a seam where it goes round", r.name);
            } else {
                assert!(a[a.len() - 48..].iter().all(|s| s.abs() < 0.02 * r.gain), "{}: ends with a click", r.name);
                assert!(r.variants == 1 || a != &cs[1].data[..], "{}: its takes are the same", r.name);
            }
        }
    }

    /// ADR 0048: bases and chains change nothing for the rows made before them, sample for sample.
    #[test]
    fn the_old_rows_sound_as_before() {
        let bank = Bank::load(rows(), None);
        let mut h = 0xcbf29ce484222325u64;
        for (r, cs) in bank.rows.iter().zip(&bank.clips) {
            if r.name.starts_with("base.") || !r.chain.is_empty() || !r.from.is_empty() {
                continue;
            }
            for s in cs.iter().flat_map(|c| c.data.iter()) {
                h = (h ^ s.to_bits() as u64).wrapping_mul(0x100000001b3);
            }
        }
        assert_eq!(h, 12066405525041298250, "the rows' samples changed");
    }

    #[test]
    fn every_block_has_its_sounds() {
        let rows = rows();
        for d in BLOCKS.placeable() {
            for what in ["dig", "place", "step"] {
                let name = format!("{what}.{}", d.sound);
                assert!(rows.iter().any(|r| r.name == name), "block '{}' needs the sound '{name}'", d.name);
            }
        }
        for name in ["ui.hover", "ui.click", "ui.tick", "wind", "land", "hurt", "music.menu"] {
            assert!(rows.iter().any(|r| r.name == name), "{name}");
        }
    }

    #[test]
    fn a_sound_has_a_place() {
        let (tx, mut m, rows) = mixer();
        let k = row(&rows, "dig.stone");
        let mut hear = |rel: [f32; 3]| {
            tx.send(Msg::Play { row: k, take: 0, gain: 1.0, pitch: 1.0, rel: Some(rel), dull: 0.0 }).unwrap();
            let mut b = vec![0.0f32; 2 * 48000];
            m.mix(&mut b);
            assert!(m.voices.is_empty(), "it is over after a second");
            assert!(b.iter().all(|s| s.abs() <= 1.0));
            b
        };
        let right = hear([2.0, 0.0, 2.0]);
        assert!(energy(&right, 1) > 4.0 * energy(&right, 0), "louder in the right ear");
        let first = |ear: usize| right.iter().skip(ear).step_by(2).position(|s| s.abs() > 1e-4).unwrap();
        assert!(first(1) + 10 < first(0), "and the right ear hears it first: {} then {}", first(1), first(0));
        let (near, far) = (hear([0.0, 0.0, 2.0]), hear([0.0, 0.0, 12.0]));
        assert!((energy(&near, 0) - energy(&near, 1)).abs() < 1e-3 * energy(&near, 0), "ahead is the same in both ears");
        assert!(energy(&far, 0) < 0.25 * energy(&near, 0), "quieter from afar");
        let behind = hear([0.0, 0.0, -2.0]);
        assert!(energy(&behind, 0) < 0.95 * energy(&near, 0), "duller behind");
        assert_eq!(energy(&hear([0.0, 0.0, 500.0]), 0), 0.0, "nothing past its reach");
    }

    #[test]
    fn the_voices_are_capped_and_the_levels_hold() {
        let (tx, mut m, rows) = mixer();
        let play = |tx: &mpsc::Sender<Msg>, gain| tx.send(Msg::Play { row: row(&rows, "dig.dirt"), take: 0, gain, pitch: 1.0, rel: None, dull: 0.0 }).unwrap();
        for _ in 0..40 {
            play(&tx, 0.5);
        }
        play(&tx, 0.01);
        let mut b = vec![0.0f32; 512];
        m.mix(&mut b);
        assert_eq!(m.voices.len(), VOICES);
        assert!(m.voices.iter().all(|v| v.want == 0.5), "a quieter one does not push a louder one out");
        assert!(b.iter().all(|s| s.abs() <= 1.0), "many at once do not clip");
        // the master at nothing: silence once it has eased there, and the voices still end
        tx.send(Msg::Levels { master: 0.0, bus: [1.0; 4] }).unwrap();
        m.mix(&mut b);
        let mut b = vec![0.0f32; 2 * 48000];
        m.mix(&mut b);
        assert!(b.iter().all(|s| *s == 0.0) && m.voices.is_empty());
        // a bus at nothing silences its own sounds only
        tx.send(Msg::Levels { master: 1.0, bus: [0.0, 1.0, 1.0, 1.0] }).unwrap();
        play(&tx, 1.0);
        m.mix(&mut b);
        assert!(b[1000..].iter().all(|s| *s == 0.0));
        tx.send(Msg::Play { row: row(&rows, "ui.click"), take: 0, gain: 1.0, pitch: 1.0, rel: None, dull: 0.0 }).unwrap();
        m.mix(&mut b);
        assert!(energy(&b, 0) > 0.0);
    }

    /// ADR 0054: the rays tell an open field from a small cave and a small cave from a hall.
    #[test]
    fn rays_tell_a_cave_from_a_field() {
        let ear = [DVec3::ZERO, DVec3::X, DVec3::Y, DVec3::Z];
        let field = room(&|p: DVec3| p.y < -1.6, &ear);
        let cave = room(&|p: DVec3| p.abs().max_element() > 3.0, &ear);
        let hall = room(&|p: DVec3| p.abs().max_element() > 10.0, &ear);
        assert_eq!(field.0, 0.0, "a field has no reverb: {field:?}");
        assert!(cave.0 == 1.0 && hall.0 == 1.0, "closed all round: {cave:?} {hall:?}");
        assert!(cave.1 < 6.0 && hall.1 > 2.0 * cave.1, "a hall is bigger than a cave: {cave:?} {hall:?}");
        // turned with the head, the same room
        let turned = [DVec3::ZERO, DVec3::Z, DVec3::Y, DVec3::NEG_X];
        assert_eq!(room(&|p: DVec3| p.y < -1.6, &turned).0, 0.0);
        // review: the field stands on another side of the planet (up is x): still open, nothing sideways or above
        let tilted = [DVec3::ZERO, DVec3::Z, DVec3::X, DVec3::NEG_Y];
        assert_eq!(room(&|p: DVec3| p.x < -1.6, &tilted), (0.0, 0.0), "a field with x up");
    }

    /// Review of ADR 0054: a hall 30 m across and 20 m high, the head 1.6 m over its floor, is fully wet and bigger than a 3 m
    /// tunnel (the corner rays reach its walls; the floor does not shrink its size).
    #[test]
    fn a_big_hall_is_as_wet_as_a_tunnel_and_bigger() {
        let ear = [DVec3::ZERO, DVec3::X, DVec3::Y, DVec3::Z];
        let hall = room(&|p: DVec3| p.x.abs() > 15.0 || p.z.abs() > 15.0 || p.y < -1.6 || p.y > 18.4, &ear);
        let tunnel = room(&|p: DVec3| p.x.abs() > 1.5 || p.y < -1.6 || p.y > 1.4, &ear);
        assert_eq!(hall.0, 1.0, "the hall is closed all round: {hall:?}");
        assert!(hall.1 > 3.0 * tunnel.1, "and bigger than the tunnel: {hall:?} {tunnel:?}");
    }

    /// Review of ADR 0054: the rays go by the body's up, not the look: pitching the head changes nothing.
    #[test]
    fn looking_up_or_down_does_not_change_the_room() {
        let tunnel = |p: DVec3| p.x.abs() > 1.5 || p.y < -1.6 || p.y > 1.4;
        let level = room(&tunnel, &[DVec3::ZERO, DVec3::X, DVec3::Y, DVec3::Z]);
        let down = room(&tunnel, &[DVec3::ZERO, DVec3::X, DVec3::Y, DVec3::new(0.0, -0.7, 0.7).normalize()]);
        assert_eq!(level, down);
    }

    /// Review of ADR 0054: nothing to hear, no rays: no card open, or high over where blocks can be.
    #[test]
    fn no_rays_with_nothing_to_hear() {
        let asked = std::cell::Cell::new(0);
        let solid = |_: DVec3| {
            asked.set(asked.get() + 1);
            true
        };
        let mut a = Audio::new();
        a.listen(&solid, &|_| 1.6);
        assert_eq!(asked.get(), 0, "no card");
        a.open = true;
        a.listen(&solid, &|_| 100.0);
        assert_eq!(asked.get(), 0, "high over the build headroom");
        a.room_at = None;
        a.listen(&solid, &|_| 1.6);
        assert!(asked.get() > 0 && a.room.0 == 1.0, "in a cave: {:?}", a.room);
    }

    /// Review of ADR 0054: the reverb itself, before the mixer's tanh: its combs die away at every size, a
    /// bigger room rings longer, a new size eases in, and silence never leaves denormals in it.
    #[test]
    fn the_reverb_dies_away_and_a_bigger_room_rings_longer() {
        let ring = |size: f32| {
            let mut r = Reverb::new(RATE);
            (r.want, r.wet) = (1.0, 1.0);
            r.size(size, RATE);
            for _ in 0..RATE as usize {
                r.tick(0.0);
            }
            assert!(r.fb.iter().all(|f| *f > 0.0 && *f < 1.0), "size {size}: {:?}", r.fb);
            let mut late = 0.0;
            for n in 0..2 * RATE as usize {
                let o = r.tick(if n == 0 { 1.0 } else { 0.0 });
                assert!(o.iter().all(|s| s.is_finite() && s.abs() <= 1.0), "size {size}: {o:?}");
                if n > RATE as usize / 2 {
                    late += o[0] * o[0] + o[1] * o[1];
                }
            }
            late
        };
        let (small, big) = (ring(0.0), ring(ROOM_RAY as f32));
        assert!(big > 10.0 * small, "a hall rings longer: {big} against {small}");
        // the World and Steps sliders at 0 in a cave: the combs ring down to nothing, never to denormals
        let mut r = Reverb::new(RATE);
        (r.want, r.wet) = (1.0, 1.0);
        r.size(0.0, RATE);
        for n in 0..30 * RATE as usize {
            let o = r.tick(if n == 0 { 1.0 } else { 0.0 });
            assert!(o.iter().chain(r.combs.iter().map(|c| &c.1)).all(|s| *s == 0.0 || s.is_normal()), "a denormal at {n}");
        }
        // a new size while it rings: the feedback eases, it does not step
        let mut r = Reverb::new(RATE);
        let before = r.fb;
        r.size(ROOM_RAY as f32, RATE);
        assert_eq!(r.fb, before, "no step");
        for _ in 0..RATE as usize {
            r.tick(0.0);
        }
        assert!(r.fb.iter().zip(before).all(|(f, b)| *f > b), "but it gets there: {:?} {before:?}", r.fb);
    }

    /// ADR 0054: in a cave a sound rings on after its end; in the open, or in the head (the interface), not.
    #[test]
    fn a_cave_rings_on_after_the_sound_and_a_field_does_not() {
        let tail = |wet: f32, name: &str| {
            let (tx, mut m, rows) = mixer();
            let k = row(&rows, name);
            tx.send(Msg::Room { wet, size: 4.0 }).unwrap();
            tx.send(Msg::Play { row: k, take: 0, gain: 1.0, pitch: 1.0, rel: Some([0.0, 0.0, 2.0]), dull: 0.0 }).unwrap();
            let mut b = vec![0.0f32; 2 * 96000];
            m.mix(&mut b);
            let end = m.bank.clips[k as usize][0].data.len() + 100;
            (energy(&b[2 * end..], 0) + energy(&b[2 * end..], 1), energy(&b[..2 * end], 0))
        };
        let (open, head) = tail(0.0, "dig.stone");
        let (cave, cave_head) = tail(1.0, "dig.stone");
        assert_eq!(open, 0.0, "nothing after the sound in the open");
        assert!(cave > 1e-3 * head, "the cave rings on: {cave} against {head}");
        assert!(cave_head < 2.0 * head, "and does not drown the sound: {cave_head} against {head}");
        assert!(tail(1.0, "ui.click").0 < 1e-12, "the interface is in the head, not in the cave");
    }

    /// Bug report 2026-11-11_12-56-46 and 12-57-17: a thump from two blocks, a louder one from higher,
    /// none for a jump's own landing (1.25 m).
    #[test]
    fn a_fall_of_two_blocks_thumps_and_a_higher_one_louder() {
        assert_eq!(thump(1.3), None, "a jump's landing is a step");
        let (small, big) = (thump(2.0).expect("two blocks"), thump(8.0).expect("eight"));
        assert!(small.0 >= 0.2 && small.0 <= 0.6 && big.0 > small.0 * 1.5 && big.1 < small.1, "{small:?} {big:?}");
        assert!(thump(500.0).unwrap().0 <= 1.5);
    }

    /// Bug report 2026-11-11_12-56-13 (fatigue): a row never plays the same take twice running.
    #[test]
    fn no_take_twice_running() {
        let (mut seed, mut last) = (77u32, 0u8);
        for _ in 0..500 {
            let t = next_take(&mut seed, 4, last);
            assert!(t < 4 && t != last);
            last = t;
        }
        assert_eq!(next_take(&mut seed, 1, 0), 0, "one take is all there is");
        // (xorshift's fixed point of all ones gives the largest number `rand` can: still a take there is)
        assert!(next_take(&mut 0xFFFF_FFFFu32.clone(), 4, 0) < 4 && next_take(&mut 0xFFFF_FF00u32.clone(), 2, 0) == 1);
    }

    /// Bug report 2026-11-11_12-55-41: the title plays music, not the wind. In a loop a layer with a decay
    /// is a note: it rings on round the loop's end into its start.
    #[test]
    fn a_note_in_a_loop_rings_round_its_end() {
        let rows = parse("[[sound]]\nname='x'\nbus='music'\nloop=2.0\nlayer=[{wave='sine',freq=[220,220],delay=1.8,attack=0.1,decay=1.0}]").unwrap();
        let a = render(&rows[0], 0);
        assert_eq!(a.len(), 2 * RATE as usize);
        let e = |from: usize, to: usize| a[from..to].iter().map(|s| s * s).sum::<f32>();
        assert!(e(0, 4800) > 10.0, "the note's tail is at the start: {}", e(0, 4800));
        assert!(e(48000, 72000) < 1e-3, "and silence before it starts: {}", e(48000, 72000));
        // (the review: rows that would have broken the making of a loop are refused)
        for bad in ["loop=4.0\nlayer=[{wave='sine',freq=[200,200],delay=1}]", "loop=0.00001\nlayer=[{wave='sine',freq=[200,200],attack=0.000001,decay=0.000001}]"] {
            assert!(parse(&format!("[[sound]]\nname='x'\nbus='music'\n{bad}")).is_err(), "{bad}");
        }
        let all = super::rows_for_tests();
        let m = all.iter().find(|r| r.name == "music.menu").expect("the title's music");
        assert!(m.bus == Bus::Music && m.loops >= 20.0);
    }

    /// A row held at nothing takes no voice, and one that has faded away gives its voice back.
    #[test]
    fn a_silent_held_row_takes_no_voice() {
        let (tx, mut m, rows) = mixer();
        let k = row(&rows, "wind");
        let mut b = vec![0.0f32; 2 * 48000];
        tx.send(Msg::Hold { row: k, gain: 0.0, dull: 0.0 }).unwrap();
        m.mix(&mut b);
        assert!(m.voices.is_empty());
        tx.send(Msg::Hold { row: k, gain: 1.0, dull: 0.0 }).unwrap();
        m.mix(&mut b);
        tx.send(Msg::Hold { row: k, gain: 0.0, dull: 0.0 }).unwrap();
        m.mix(&mut b);
        m.mix(&mut b);
        assert!(m.voices.is_empty(), "faded away and gone");
    }

    #[test]
    fn a_held_row_goes_round_and_fades() {
        let (tx, mut m, rows) = mixer();
        let k = row(&rows, "wind");
        tx.send(Msg::Hold { row: k, gain: 1.0, dull: 0.0 }).unwrap();
        let mut b = vec![0.0f32; 2 * 48000];
        for _ in 0..12 {
            m.mix(&mut b);
            assert!(energy(&b, 0) > 1.0, "still blowing");
        }
        tx.send(Msg::Hold { row: k, gain: 0.7, dull: 0.0 }).unwrap();
        m.mix(&mut b);
        assert_eq!(m.voices.len(), 1, "one voice, however often it is told");
        tx.send(Msg::Hold { row: k, gain: 0.0, dull: 0.0 }).unwrap();
        m.mix(&mut b);
        assert!(b[2 * 47000..].iter().all(|s| s.abs() < 1e-3), "faded out");
    }

    #[test]
    fn a_recording_replaces_the_row_of_its_name() {
        let dir = std::env::temp_dir().join(format!("technoape-sounds-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let spec = hound::WavSpec { channels: 2, sample_rate: 22050, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
        let mut w = hound::WavWriter::create(dir.join("ui.click.wav"), spec).unwrap();
        for i in 0..2204 {
            w.write_sample(if i % 2 == 0 { 16384i16 } else { 0 }).unwrap();
        }
        w.finalize().unwrap();
        let rows = rows();
        let bank = Bank::load(rows.clone(), Some(&dir));
        std::fs::remove_dir_all(&dir).unwrap();
        let k = row(&rows, "ui.click") as usize;
        let c = &bank.clips[k][0];
        // two channels become one (their mean), at the file's own rate, times the row's gain
        assert_eq!((bank.clips[k].len(), c.data.len(), c.rate), (1, 1102, 22050.0));
        assert!((c.data[0] - 0.25 * rows[k].gain).abs() < 1e-4);
        assert_eq!(bank.clips[k + 1].len(), rows[k + 1].variants as usize, "the others are still made in code");
    }

    #[test]
    fn space_carries_only_what_you_touch() {
        let mut a = Audio::new();
        let at = Some(DVec3::new(0.0, 0.0, 3.0));
        assert!(a.play("dig.stone", at, 1.0, 1.0, false));
        assert!(!a.play("dig.stone", Some(DVec3::Z * 900.0), 1.0, 1.0, false), "too far to hear");
        a.carry = 0.0;
        assert!(!a.play("dig.stone", at, 1.0, 1.0, false), "no air, no sound");
        assert!(a.play("dig.stone", at, 1.0, 1.0, true), "but your own digging, through the suit");
        assert!(a.play("ui.click", None, 1.0, 1.0, false), "and the interface");
        // what the mixer is told: half as loud and fully dull, and a place to the head's right is +x
        let (tx, rx) = mpsc::channel();
        a.tx = tx;
        a.ear = [DVec3::ZERO, DVec3::NEG_Z, DVec3::Y, DVec3::X];
        a.play("dig.stone", Some(DVec3::new(1.0, 2.0, -3.0)), 1.0, 1.0, true);
        match rx.try_recv() {
            // (each play is up to 15% louder or softer)
            Ok(Msg::Play { gain, dull, rel, .. }) => assert!((0.42..0.58).contains(&gain) && (dull, rel) == (1.0, Some([3.0, 2.0, 1.0])), "{gain} {dull} {rel:?}"),
            _ => panic!("no message"),
        }
        assert_eq!(a.played["dig.stone"], 3);
        assert!(a.json().contains("\"dig.stone\":3"));
    }

    /// ADR 0048: a small row made from a base, with a chain and a spread.
    const MINI: &str = "
[[sound]]
name = 'b'
bus = 'world'
variants = 3
layer = [{ wave = 'sine', freq = [440, 440], decay = 0.3 }]

[[sound]]
name = 'x'
bus = 'world'
variants = 4
from = [{ base = 'b' }, { base = 'b', gain = 0.5, delay = 0.1 }]
chain = [{ pitch = 2 }, { lowpass = 2000 }, { echo = 0.1, feedback = 0.3, mix = 0.4 }]
spread = { pitch = 1.0, echo = { mix = 0.2 } }
";

    #[test]
    fn a_row_is_made_from_bases_and_a_chain_and_its_takes_differ() {
        let rows = parse(MINI).unwrap();
        let bank = Bank::load(rows.clone(), None);
        let takes: Vec<&[f32]> = bank.clips[1].iter().map(|c| &c.data[..]).collect();
        assert_eq!(takes.len(), 4);
        for i in 0..4 {
            for j in i + 1..4 {
                assert!(takes[i] != takes[j], "takes {i} and {j} are the same");
            }
            assert!(takes[i].iter().all(|s| s.abs() <= 1.0) && takes[i].iter().any(|s| s.abs() > 0.5), "never past its gain, and not quiet");
        }
        // a rebuild makes the same takes, and so does the Studio's render
        assert!(Bank::load(rows.clone(), None).clips[1].iter().zip(&takes).all(|(c, t)| &c.data[..] == *t));
        let studio = render_row(&bank, &rows[1], None).unwrap();
        assert!(studio.iter().zip(&takes).all(|(a, b)| &a[..] == *b));
    }

    /// `MINI` with its pitch in the middle of the chain: a spread step (pitch) before another (echo).
    fn mini_pitch_between() -> String {
        MINI.replace("chain = [{ pitch = 2 }, { lowpass = 2000 },", "chain = [{ lowpass = 2000 }, { pitch = 2 },")
    }

    #[test]
    fn a_step_switched_off_in_the_studio_is_the_row_without_it() {
        // the pitch in the middle off: the echo after it draws the numbers it draws without the pitch
        let rows = parse(&mini_pitch_between()).unwrap();
        let bank = Bank::load(rows.clone(), None);
        let off = render_row(&bank, &rows[1], Some(1)).unwrap();
        let mut without = rows[1].clone();
        without.chain.remove(1);
        assert_eq!(off, render_row(&bank, &without, None).unwrap());
        assert!(off[0] != render_row(&bank, &rows[1], None).unwrap()[0]);
    }

    #[test]
    fn a_step_written_off_is_the_row_without_it() {
        // the pitch in the middle off, its spread kept for when it is on again: the game's takes are the
        // row's without it, and the Studio's render with that step skipped the same
        let text = mini_pitch_between();
        let off = parse(&text.replace("{ pitch = 2 }", "{ on = false, pitch = 2 }")).unwrap();
        let without = parse(&text.replace(", { pitch = 2 }", "").replace("pitch = 1.0, ", "")).unwrap();
        let (a, b) = (Bank::load(off.clone(), None), Bank::load(without, None));
        assert!(a.clips[1].iter().zip(&b.clips[1]).all(|(x, y)| x.data[..] == y.data[..]));
        assert_eq!(off[1].chain.len(), 2);
        let on = parse(&text).unwrap();
        let studio = render_row(&a, &on[1], Some(1)).unwrap();
        assert!(studio.iter().zip(&a.clips[1]).all(|(s, c)| s[..] == c.data[..]), "the Studio's skipped step and the game's off step draw different numbers");
        assert!(parse(&MINI.replace("{ echo = 0.1,", "{ on = 1, echo = 0.1,")).unwrap_err().contains("on = false"));
        // a loop whose one fade is off is a loop without it, not refused
        let lp = "[[sound]]\nname = 'a'\nbus = 'world'\nloop = 2.0\nlayer = [{ wave = 'sine', freq = [440, 440] }]\nchain = [{ on = false, fade = 0.1 }]\n";
        assert_eq!(parse(lp).unwrap()[0].chain.len(), 0);
    }

    #[test]
    fn a_base_loop_and_bad_rows_are_refused_by_name() {
        let row = |name: &str, extra: &str| format!("[[sound]]\nname = '{name}'\nbus = 'world'\n{extra}\n");
        let lay = "layer = [{ wave = 'sine', freq = [440, 440], decay = 0.1 }]";
        let cases = [
            (row("a", "from = [{ base = 'b' }]") + &row("b", "from = [{ base = 'c' }]") + &row("c", "from = [{ base = 'a' }]"), "made from itself (through 'b', 'c', 'a')"),
            (row("a", "from = [{ base = 'a' }]"), "made from itself"),
            (row("a", &format!("{lay}\nfrom = [{{ base = 'x' }}]")), "not both"),
            (row("a", &format!("{lay}\nchain = [{{ lowpas = 100 }}]")), "chain step"),
            (row("a", &format!("{lay}\nchain = [{{ lowpass = 100, mix = 1 }}]")), "no number 'mix'"),
            (row("a", &format!("{lay}\nchain = [{{ lowpass = 100 }}]\nspread = {{ pitch = 1 }}")), "a spread on a step"),
            (row("a", "loop = 2.0\nlayer = [{ wave = 'sine', freq = [440, 440] }]\nchain = [{ fade = 0.1 }]"), "trim or fade"),
        ];
        for (t, why) in cases {
            let e = parse(&t).unwrap_err();
            assert!(e.contains("sound 'a'") && e.contains(why), "{e}");
        }
        assert!(parse(&format!("{}{}", row("a", lay), row("z", "from = [{ base = 'a' }]\nchain = [{ reverse = true }]"))).is_ok());
    }

    #[test]
    fn a_base_may_be_a_recording_and_a_missing_one_is_named() {
        let dir = std::env::temp_dir().join(format!("technoape-bases-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("base")).unwrap();
        let spec = hound::WavSpec { channels: 1, sample_rate: 24000, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
        let mut w = hound::WavWriter::create(dir.join("base/knock.wav"), spec).unwrap();
        for i in 0..2400 {
            w.write_sample(((1.0 - i as f32 / 2400.0) * 16000.0) as i16).unwrap();
        }
        w.finalize().unwrap();
        let rows = parse("[[sound]]\nname = 'k'\nbus = 'world'\ngain = 0.5\nfrom = [{ base = 'knock' }]").unwrap();
        let (bank, errors) = Bank::build(rows.clone(), Some(&dir), None);
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(errors.is_empty(), "{errors:?}");
        let c = &bank.clips[0][0];
        // at the game's rate (twice the file's), set to the row's gain
        assert!((c.data.len() as i32 - 4800).abs() <= 2 && (c.data[0] - 0.5).abs() < 1e-3, "{} {}", c.data.len(), c.data[0]);
        let (_, errors) = Bank::build(rows, None, None);
        assert!(errors.len() == 1 && errors[0].contains("sound 'k': no base 'knock'"), "{errors:?}");
    }

    #[test]
    fn a_reload_makes_only_what_changed_and_refuses_a_bad_file() {
        let dir = std::env::temp_dir().join(format!("technoape-reload-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let toml = dir.join("sounds.toml");
        let bank = Bank::load(parse(MINI).unwrap(), Some(&dir));
        std::fs::write(&toml, MINI.replace("lowpass = 2000", "lowpass = 3000")).unwrap();
        let seen = stamps(&toml, &dir);
        let new = reload(&bank, &toml, &dir).unwrap();
        assert!(Arc::ptr_eq(&new.clips[0][0].data, &bank.clips[0][0].data), "the base did not change: kept");
        assert!(!Arc::ptr_eq(&new.clips[1][0].data, &bank.clips[1][0].data) && new.clips[1][0].data != bank.clips[1][0].data, "the changed row made again");
        std::fs::write(&toml, MINI.replace("lowpass = 2000", "lowpas = 2000")).unwrap();
        assert!(stamps(&toml, &dir) != seen, "a save is seen");
        let e = reload(&new, &toml, &dir).err().unwrap();
        assert!(e.contains("sound 'x'"), "{e}");
        std::fs::write(&toml, &MINI[..MINI.find("[[sound]]\nname = 'x'").unwrap()]).unwrap();
        let e = reload(&new, &toml, &dir).err().unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(e.contains("sound 'x': gone"), "{e}");
    }

    #[test]
    fn a_reload_swaps_a_held_row_without_a_gap() {
        let (tx, mut m, mut rows) = mixer();
        let (wind, dig) = (row(&rows, "wind") as usize, row(&rows, "dig.stone"));
        tx.send(Msg::Hold { row: wind as u16, gain: 1.0, dull: 0.0 }).unwrap();
        let mut out = Vec::new();
        let mut b = vec![0.0f32; 960];
        for _ in 0..200 {
            m.mix(&mut b);
            out.extend_from_slice(&b);
        }
        // (another wind: softer, and a sound of its own, so a jump to it would click)
        rows[wind].gain *= 0.5;
        rows[wind].layer[0].freq[0] *= 1.3;
        let (new, errors) = Bank::build(rows, None, Some(&m.bank));
        assert!(errors.is_empty() && Arc::ptr_eq(&new.clips[dig as usize][0].data, &m.bank.clips[dig as usize][0].data));
        // (a quiet one-shot under it, which plays on through the swap)
        tx.send(Msg::Play { row: dig, take: 0, gain: 0.02, pitch: 1.0, rel: None, dull: 0.0 }).unwrap();
        m.mix(&mut b);
        out.extend_from_slice(&b);
        assert_eq!(m.voices.len(), 2);
        let new = Arc::new(new);
        tx.send(Msg::Bank(new.clone())).unwrap();
        let at = out.len();
        for _ in 0..200 {
            m.mix(&mut b);
            assert!(energy(&b, 0) > 0.01, "no gap");
            out.extend_from_slice(&b);
        }
        let jump = |s: &[f32]| s.chunks(2).collect::<Vec<_>>().windows(2).map(|w| (w[1][0] - w[0][0]).abs()).fold(0.0f32, f32::max);
        assert!(jump(&out[at - 960..at + 9600]) <= 1.5 * jump(&out[..at - 960]), "no click: {} against {}", jump(&out[at - 960..at + 9600]), jump(&out[..at - 960]));
        assert_eq!(m.voices.len(), 1, "the old take faded out, the one-shot played out");
        assert!(m.voices[0].held == Some(wind as u16) && Arc::ptr_eq(&m.voices[0].clip.data, &new.clips[wind][0].data));
        assert!(energy(&b, 0) < 0.6 * energy(&out[at - 1920..at - 960], 0), "and it is the new, softer wind");
    }

    #[test]
    fn the_rows_start_from_the_file_on_disk() {
        let dir = std::env::temp_dir().join(format!("technoape-start-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let toml = dir.join("sounds.toml");
        std::fs::write(&toml, MINI).unwrap();
        let (on_disk, e) = start_rows(&toml);
        assert!(e.is_none() && on_disk.iter().map(|r| &r.name).eq(parse(MINI).unwrap().iter().map(|r| &r.name)), "the file as it is now");
        std::fs::write(&toml, MINI.replace("lowpass = 2000", "lowpas = 2000")).unwrap();
        let (bad, e) = start_rows(&toml);
        assert!(bad.len() == rows().len() && e.is_some_and(|e| e.contains("sounds.toml")), "a bad file: named, the compiled-in rows");
        std::fs::remove_dir_all(&dir).unwrap();
        let (gone, e) = start_rows(&toml);
        assert!(gone.len() == rows().len() && e.is_none(), "no file: the compiled-in rows, quietly");
    }

    #[test]
    fn a_reload_reaches_the_mixer_with_nothing_held() {
        let (tx, rx) = mpsc::channel();
        let mut a = Audio::new();
        a.tx = tx;
        *a.fresh.lock().unwrap() = Some(Arc::new(Bank::load(rows(), None)));
        assert!(a.play("ui.click", None, 1.0, 1.0, false));
        let got: Vec<Msg> = rx.try_iter().collect();
        assert!(matches!(got[..], [Msg::Bank(_), Msg::Play { .. }]), "the new bank first, then the play");
    }

    #[test]
    fn a_missing_row_is_named_once_and_skipped() {
        let mut a = Audio::new();
        assert!(!a.play("no.such.sound", None, 1.0, 1.0, false));
        assert!(!a.play("no.such.sound", None, 1.0, 1.0, false));
        assert_eq!(a.missing.len(), 1);
    }

    #[test]
    fn the_studio_auditions_samples_at_a_distance() {
        let (tx, rx) = mpsc::channel();
        let mut m = Mixer::new(rx, Arc::new(Bank::load(rows(), None)), RATE, Arc::default());
        let mut a = Audio::new();
        a.tx = tx;
        let clip: Vec<f32> = (0..4800).map(|i| (i as f32 * 0.05).sin()).collect();
        let mut hear = |d: f32| {
            a.audition(&clip, Bus::World, d, 24.0, 0.0);
            let mut b = vec![0.0f32; 2 * 9600];
            m.mix(&mut b);
            energy(&b, 0)
        };
        let (near, far, gone) = (hear(0.0), hear(10.0), hear(30.0));
        assert!(near > 4.0 * far && far > 0.0 && gone == 0.0, "{near} {far} {gone}");
    }
}
