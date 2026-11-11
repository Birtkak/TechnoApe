//! Sound (docs/design/sound.md, ADR 0032). Every sound is a row of `data/sounds.toml`: made in code here
//! (`render`), or a recording in `data/sounds/` of the row's name. The sound card's own thread mixes them
//! with `Mixer::mix`, the same function the tests call; the game only sends it small messages.
use glam::DVec3;
use serde::Deserialize;
use std::collections::BTreeMap;
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
    pub layer: Vec<Layer>,
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
    let f: File = toml::from_str(text).map_err(|e| e.to_string())?;
    for r in &f.sound {
        let bad = |what: &str| Err(format!("sound '{}': {what}", r.name));
        if f.sound.iter().filter(|o| o.name == r.name).count() > 1 {
            return bad("its name is used twice");
        }
        if r.layer.is_empty() || !(1..=8).contains(&r.variants) {
            return bad("needs a layer and 1 to 8 variants");
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
fn wav(path: &std::path::Path) -> Result<Clip, String> {
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

/// Every row with its takes, ready to play.
pub struct Bank {
    pub rows: Vec<Row>,
    pub clips: Vec<Vec<Clip>>,
}

impl Bank {
    /// Each row's takes: the recordings `<dir>/<name>.wav` and `<name>.1.wav` to `.8.wav` when there are
    /// any, else made in code.
    pub fn load(rows: Vec<Row>, dir: Option<&std::path::Path>) -> Bank {
        let clips = rows
            .iter()
            .map(|r| {
                let names = std::iter::once(format!("{}.wav", r.name)).chain((1..=8).map(|k| format!("{}.{k}.wav", r.name)));
                let files: Vec<Clip> = dir
                    .into_iter()
                    .flat_map(|d| names.clone().map(move |n| d.join(n)))
                    .filter(|p| p.exists())
                    .filter_map(|p| wav(&p).map_err(|e| eprintln!("[technoape] sound {}: {e}", p.display())).ok())
                    .map(|c| Clip { data: c.data.iter().map(|s| s * r.gain).collect(), rate: c.rate })
                    .collect();
                if files.is_empty() {
                    (0..r.variants).map(|v| Clip { data: render(r, v).into(), rate: RATE }).collect()
                } else {
                    files
                }
            })
            .collect();
        Bank { rows, clips }
    }
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
}

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
}

pub struct Mixer {
    rx: mpsc::Receiver<Msg>,
    bank: Arc<Bank>,
    rate: f32,
    voices: Vec<Voice>,
    master: (f32, f32),
    bus: [f32; 4],
    stats: Arc<Stats>,
}

impl Mixer {
    pub fn new(rx: mpsc::Receiver<Msg>, bank: Arc<Bank>, rate: f32, stats: Arc<Stats>) -> Mixer {
        stats.rate.store(rate as u32, Relaxed);
        Mixer { rx, bank, rate, voices: Vec::with_capacity(VOICES), master: (1.0, 1.0), bus: [1.0; 4], stats }
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
                    self.voices.push(Voice { clip: c.clone(), at: 0.0, step: (c.rate / self.rate) as f64, pan: [0.707; 2], lag: [0.0; 2], level: 0.0, want: gain, lp, z: [0.0; 2], bus, held: Some(row) });
                }
            }
            Msg::Play { row, take, gain, pitch, rel, dull } => {
                let Some(r) = self.bank.rows.get(row as usize) else { return };
                let c = &self.bank.clips[row as usize];
                let c = &c[take as usize % c.len()];
                let (mut pan, mut lag, mut gain, mut dull) = ([0.707; 2], [0.0; 2], gain, dull);
                if let Some(rel) = rel {
                    let d = (rel[0] * rel[0] + rel[1] * rel[1] + rel[2] * rel[2]).sqrt().max(0.01);
                    let (x, z) = (rel[0] / d, rel[2] / d);
                    // quieter with distance and gone at the row's reach; left and right by where it is
                    // around the head, the far ear up to 0.6 ms late; duller behind
                    gain *= (1.0 - d / r.reach).max(0.0) / (1.0 + d * 0.25);
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
                let v = Voice { clip: c.clone(), at: 0.0, step: (c.rate / self.rate * pitch) as f64, pan, lag, level: gain, want: gain, lp: self.lp(dull), z: [0.0; 2], bus: r.bus as usize, held: None };
                self.voices.push(v);
            }
        }
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
        for v in &mut self.voices {
            let (d, n, bus) = (&v.clip.data, v.clip.data.len(), self.bus[v.bus]);
            for f in 0..frames {
                v.level += (v.want - v.level) * 0.001;
                for e in 0..2 {
                    let p = v.at - v.lag[e];
                    let i = p as usize;
                    let s = if p < 0.0 || i >= n {
                        0.0
                    } else {
                        let next = if i + 1 < n { d[i + 1] } else if v.held.is_some() { d[0] } else { 0.0 };
                        d[i] + (next - d[i]) * (p - i as f64) as f32
                    };
                    v.z[e] += v.lp * (s - v.z[e]);
                    out[2 * f + e] += v.z[e] * v.pan[e] * v.level * bus;
                }
                v.at += v.step;
                if v.held.is_some() && v.at >= n as f64 {
                    v.at -= n as f64;
                }
            }
        }
        // a one-shot is done when its later ear has heard its end
        // (a held row that has faded to nothing gives its voice back: it starts anew when it is wanted)
        self.voices.retain(|v| if v.held.is_some() { v.want > 0.0 || v.level > 1e-4 } else { v.at - v.lag[0].max(v.lag[1]) < v.clip.data.len() as f64 });
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
fn card(rx: mpsc::Receiver<Msg>, rows: Vec<Row>, stats: Arc<Stats>) -> Result<(), String> {
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
    let bank = Arc::new(Bank::load(rows, Some(std::path::Path::new("data/sounds"))));
    // what was asked for while the takes were being made is old news; levels and held rows still count
    let mut mixer = Mixer::new(rx, bank, rate as f32, stats.clone());
    for m in mixer.rx.try_iter().collect::<Vec<_>>() {
        if !matches!(m, Msg::Play { .. }) {
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
    while !stats.gone.load(Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    Ok(())
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
    /// When a lost card may be opened again.
    retry: std::time::Instant,
}

impl Audio {
    pub fn new() -> Audio {
        let rows = parse(ROWS_TOML).expect("data/sounds.toml");
        // the far end is dropped: until a card is opened the messages go nowhere
        Audio { tx: mpsc::channel().0, rows, stats: Arc::default(), played: BTreeMap::new(), seed: 0x2F6E2B1, levels: (-1.0, [0.0; 4]), last: Vec::new(), ear: [DVec3::ZERO, DVec3::X, DVec3::Y, DVec3::Z], carry: 1.0, dull: 0.0, open: false, retry: std::time::Instant::now() }
    }

    /// Starts the sound card's thread; a machine without one plays on in silence.
    pub fn open(&mut self) {
        let (tx, rx) = mpsc::channel();
        // each card has its own stats: the old one's `gone` must not close the new one
        (self.tx, self.open, self.levels.0, self.stats) = (tx, true, -1.0, Arc::default());
        let (rows, stats) = (self.rows.clone(), self.stats.clone());
        std::thread::spawn(move || {
            if let Err(e) = card(rx, rows, stats.clone()) {
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
        let Some(k) = self.rows.iter().position(|r| r.name == name) else {
            debug_assert!(false, "no sound '{name}'");
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
        if let Some(k) = self.rows.iter().position(|r| r.name == name) {
            let _ = self.tx.send(Msg::Hold { row: k as u16, gain, dull: self.dull });
        }
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
            "\"audio\":{{\"device\":\"{}\",\"rate\":{},\"delay_ms\":{:.1},\"voices_peak\":{},\"mix_us_max\":{},\"played\":{{{}}}}},",
            s.device.lock().unwrap().replace(['"', '\\'], ""),
            s.rate.load(Relaxed),
            s.delay_us.load(Relaxed) as f32 / 1000.0,
            s.voices_peak.load(Relaxed),
            s.mix_us_max.load(Relaxed),
            played.join(",")
        )
    }
}

/// `sounds=1`: every row's takes in numbers (seconds, loudest sample, mean loudness, and about where its
/// pitch sits, from how often it crosses zero), one JSON line.
pub fn report() -> String {
    let bank = Bank::load(parse(ROWS_TOML).expect("data/sounds.toml"), Some(std::path::Path::new("data/sounds")));
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
            format!("{{\"name\":\"{}\",\"takes\":{},\"made_in_code\":{},\"seconds\":{:.3},\"peak\":{:.3},\"rms\":{:.3},\"hz\":{:.0}}}", r.name, cs.len(), c.rate == RATE && cs.len() == r.variants as usize, n / c.rate, peak, rms, cross / 2.0 / (n / c.rate))
        })
        .collect();
    format!("{{\"sounds\":[{}]}}", rows.join(","))
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
        for r in rows() {
            let a = render(&r, 0);
            assert_eq!(a, render(&r, 0), "{}: the same take twice", r.name);
            let peak = a.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            let rms = (a.iter().map(|s| s * s).sum::<f32>() / a.len() as f32).sqrt();
            assert!((peak - r.gain).abs() < 1e-3 && peak <= 1.0, "{}: peak {peak}", r.name);
            assert!(rms > 0.01 * r.gain, "{}: all but silent, rms {rms}", r.name);
            assert!(a.len() as f32 / RATE <= 60.0, "{}: {} s", r.name, a.len() as f32 / RATE);
            if r.loops > 0.0 {
                // no seam: the step from its end to its start is no bigger than its steps anywhere else
                let big = a.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0f32, f32::max);
                assert!((a[0] - a[a.len() - 1]).abs() <= big, "{}: a seam where it goes round", r.name);
            } else {
                assert!(a[a.len() - 48..].iter().all(|s| s.abs() < 0.02 * r.gain), "{}: ends with a click", r.name);
                assert!(r.variants == 1 || a != render(&r, 1), "{}: its takes are the same", r.name);
            }
        }
    }

    #[test]
    fn every_block_has_its_sounds() {
        let rows = rows();
        for id in 1..=255u8 {
            let Some(d) = BLOCKS.get(id) else { continue };
            for what in ["dig", "place", "step"] {
                let name = format!("{what}.{}", d.sound);
                assert!(rows.iter().any(|r| r.name == name), "block '{}' needs the sound '{name}'", d.name);
            }
        }
        for name in ["ui.hover", "ui.click", "ui.tick", "wind", "land", "music.menu"] {
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
}
