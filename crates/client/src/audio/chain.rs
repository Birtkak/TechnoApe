//! A row's chain (ADR 0048): each step one change to a take's samples, a pure function of a buffer at
//! `RATE`. Steps are read from a row's tables (`{ lowpass = 1200, q = 0.7 }`) and checked against `STEPS`.
use super::RATE;
use std::f32::consts::{PI, TAU};

/// One number of a step: its name in a row, the value it has when a row leaves it out, its range.
pub struct Num {
    pub name: &'static str,
    pub default: f32,
    pub min: f32,
    pub max: f32,
}
const fn n(name: &'static str, default: f32, min: f32, max: f32) -> Num {
    Num { name, default, min, max }
}

/// Every step: the key that names it in a row (and sets its first number), and its numbers.
pub const STEPS: &[(&str, &[Num])] = &[
    ("pitch", &[n("semitones", 0.0, -24.0, 24.0)]),
    ("speed", &[n("times", 1.0, 0.25, 4.0)]),
    ("stretch", &[n("times", 1.0, 0.25, 4.0)]),
    ("lowpass", &[n("hz", 1000.0, 20.0, 20000.0), n("q", 0.707, 0.1, 20.0)]),
    ("highpass", &[n("hz", 1000.0, 20.0, 20000.0), n("q", 0.707, 0.1, 20.0)]),
    ("bandpass", &[n("hz", 1000.0, 20.0, 20000.0), n("q", 1.0, 0.1, 20.0)]),
    ("reverb", &[n("size", 0.5, 0.0, 1.0), n("damping", 0.5, 0.0, 1.0), n("mix", 0.3, 0.0, 1.0)]),
    ("echo", &[n("seconds", 0.25, 0.01, 2.0), n("feedback", 0.3, 0.0, 0.9), n("mix", 0.5, 0.0, 1.0)]),
    ("drive", &[n("amount", 1.0, 0.0, 20.0)]),
    ("tremolo", &[n("rate", 5.0, 0.1, 40.0), n("depth", 0.5, 0.0, 1.0)]),
    ("vibrato", &[n("rate", 5.0, 0.1, 20.0), n("depth", 0.3, 0.0, 2.0)]),
    ("fade", &[n("in", 0.0, 0.0, 10.0), n("out", 0.0, 0.0, 10.0)]),
    ("trim", &[n("start", 0.0, 0.0, 60.0), n("end", 0.0, 0.0, 60.0)]),
    ("reverse", &[]),
    ("gain", &[n("times", 1.0, 0.0, 4.0)]),
    ("crush", &[n("bits", 8.0, 1.0, 16.0)]),
];

/// One step of a chain: which (an index into `STEPS`) and its numbers in `STEPS`' order.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub kind: usize,
    pub n: [f32; 3],
}

/// A take's random change to one number of every step of a kind: up to `amount` either way.
#[derive(Clone, Debug, PartialEq)]
pub struct Spread {
    pub step: usize,
    pub number: usize,
    pub amount: f32,
}

fn kind(name: &str) -> Option<usize> {
    STEPS.iter().position(|s| s.0 == name)
}
fn num(v: &toml::Value) -> Option<f32> {
    v.as_float().map(|f| f as f32).or(v.as_integer().map(|i| i as f32))
}

impl Step {
    pub fn name(&self) -> &'static str {
        STEPS[self.kind].0
    }

    /// From a row's table: exactly one key names the step and holds its first number (`reverse = true`
    /// has none); the others are its other numbers by name.
    pub fn parse(t: &toml::Table) -> Result<Step, String> {
        if t.get("on").is_some_and(|v| v.as_bool().is_none()) {
            return Err("a chain step's switch is `on = false` (left out it is on)".into());
        }
        let named: Vec<&String> = t.keys().filter(|k| kind(k).is_some()).collect();
        let [k] = named[..] else { return Err(format!("a chain step names one of {} (this one: {:?})", STEPS.iter().map(|s| s.0).collect::<Vec<_>>().join(", "), t.keys().collect::<Vec<_>>())) };
        let kind = kind(k).unwrap();
        let nums = STEPS[kind].1;
        let mut s = Step { kind, n: [0.0; 3] };
        for (i, d) in nums.iter().enumerate() {
            s.n[i] = d.default;
        }
        for (key, v) in t.iter().filter(|(k, _)| *k != "on") {
            let at = if key == k { 0 } else { nums.iter().position(|d| d.name == key).filter(|i| *i > 0).ok_or(format!("step '{k}' has no number '{key}'"))? };
            if nums.is_empty() {
                if v.as_bool() != Some(true) {
                    return Err(format!("step '{k}' is written `{k} = true`"));
                }
                continue;
            }
            let d = &nums[at];
            match num(v) {
                Some(x) if (d.min..=d.max).contains(&x) => s.n[at] = x,
                _ => return Err(format!("step '{k}': {} {}..{}", d.name, d.min, d.max)),
            }
        }
        if k == "trim" && s.n[1] != 0.0 && s.n[1] <= s.n[0] {
            return Err("step 'trim': end after start, or 0 for the end".into());
        }
        Ok(s)
    }

    /// Whether a step's table is switched on (`on = false` keeps it in the row, not in the sound).
    pub fn is_on(t: &toml::Table) -> bool {
        t.get("on").and_then(|v| v.as_bool()) != Some(false)
    }

    /// The step with its numbers moved by a take's spread (`draw` gives -1..1 each call), kept in range.
    pub fn spread(&self, spread: &[Spread], draw: &mut impl FnMut() -> f32) -> Step {
        let mut s = self.clone();
        for p in spread.iter().filter(|p| p.step == self.kind) {
            let d = &STEPS[self.kind].1[p.number];
            s.n[p.number] = (s.n[p.number] + p.amount * draw()).clamp(d.min, d.max);
        }
        s
    }
}

/// `spread = { pitch = 1.5, lowpass = 400, reverb = { mix = 0.1 } }`: a step's name sets its first
/// number's spread, a table under it any of its numbers by name.
pub fn parse_spread(t: &toml::Table) -> Result<Vec<Spread>, String> {
    let mut out = Vec::new();
    for (k, v) in t {
        let step = kind(k).ok_or(format!("spread names no step '{k}'"))?;
        let nums = STEPS[step].1;
        let pairs: Vec<(usize, &toml::Value)> = match v.as_table() {
            Some(sub) => sub.iter().map(|(name, v)| nums.iter().position(|d| d.name == name).map(|i| (i, v)).ok_or(format!("spread: step '{k}' has no number '{name}'"))).collect::<Result<_, _>>()?,
            None if !nums.is_empty() => vec![(0, v)],
            None => return Err(format!("spread: step '{k}' has no numbers")),
        };
        for (number, v) in pairs {
            let d = &nums[number];
            match num(v) {
                Some(amount) if (0.0..=d.max - d.min).contains(&amount) => out.push(Spread { step, number, amount }),
                _ => return Err(format!("spread: {k}.{} from 0 to {}", d.name, d.max - d.min)),
            }
        }
    }
    Ok(out)
}

/// The step done to a buffer.
pub fn apply(mut x: Vec<f32>, s: &Step) -> Vec<f32> {
    let [a, b, c] = s.n;
    match s.name() {
        "pitch" => pitch(&x, a),
        "speed" => speed(&x, a),
        "stretch" => resample(&x, 1.0 / a),
        "lowpass" | "highpass" | "bandpass" => biquad(&x, s.name(), a, b),
        "reverb" => reverb(&x, a, b, c),
        "echo" => echo(&x, a, b, c),
        "drive" => {
            let k = 1.0 + a;
            x.iter_mut().for_each(|s| *s = (k * *s).tanh() / k.tanh());
            x
        }
        "tremolo" => {
            x.iter_mut().enumerate().for_each(|(i, s)| *s *= 1.0 - b * (0.5 - 0.5 * (TAU * a * i as f32 / RATE).cos()));
            x
        }
        "vibrato" => vibrato(&x, a, b),
        "fade" => {
            let n = x.len();
            let (fi, fo) = ((a * RATE) as usize, (b * RATE) as usize);
            for i in 0..fi.min(n) {
                x[i] *= i as f32 / fi as f32;
            }
            for i in 0..fo.min(n) {
                x[n - 1 - i] *= i as f32 / fo as f32;
            }
            x
        }
        "trim" => trim(x, a, b),
        "reverse" => {
            x.reverse();
            x
        }
        "gain" => {
            x.iter_mut().for_each(|s| *s *= a);
            x
        }
        "crush" => {
            let q = 2f32.powf(a - 1.0);
            x.iter_mut().for_each(|s| *s = (*s * q).round() / q);
            x
        }
        _ => unreachable!(),
    }
}

/// Read `x` `step` samples at a time (a tape played faster or slower): the length over `step`, every
/// frequency times `step`. Above 1 a low-pass first, so what would fold past the top is gone.
pub fn resample(x: &[f32], step: f32) -> Vec<f32> {
    let lp;
    let x = if step > 1.0 {
        lp = biquad(x, "lowpass", 0.45 * RATE / step, 0.707);
        &lp[..]
    } else {
        x
    };
    let len = if x.is_empty() { 0 } else { ((x.len() - 1) as f32 / step) as usize + 1 };
    (0..len)
        .map(|i| {
            let p = i as f32 * step;
            let k = p as usize;
            let next = x.get(k + 1).copied().unwrap_or(0.0);
            x[k] + (next - x[k]) * (p - k as f32)
        })
        .collect()
}

/// Times as fast with the pitch kept: WSOLA, grains of 30 ms at half overlap, each placed where it best
/// continues the last one within 5 ms of where it should be.
pub fn speed(x: &[f32], times: f32) -> Vec<f32> {
    if (times - 1.0).abs() < 1e-4 || x.is_empty() {
        return x.to_vec();
    }
    let (w, hop, tol) = (1440usize, 720usize, 240isize);
    let len = (x.len() as f32 / times) as usize;
    let win: Vec<f32> = (0..w).map(|j| 0.5 - 0.5 * (TAU * (j as f32 + 0.5) / w as f32).cos()).collect();
    let (mut y, mut norm) = (vec![0.0f32; len + w], vec![0.0f32; len + w]);
    let at = |i: isize| if i >= 0 { x.get(i as usize).copied().unwrap_or(0.0) } else { 0.0 };
    let mut prev = 0isize;
    for o in (0..len).step_by(hop) {
        let nominal = (o as f32 * times) as isize;
        let pos = if o == 0 {
            0
        } else {
            // the natural continuation of the last grain, matched over the first half of a grain
            let want = prev + hop as isize;
            let score = |d: isize| (0..w as isize / 2).step_by(2).map(|j| at(want + j) * at(nominal + d + j)).sum::<f32>();
            nominal + (-tol..=tol).max_by(|a, b| score(*a).total_cmp(&score(*b))).unwrap()
        };
        for j in 0..w {
            y[o + j] += win[j] * at(pos + j as isize);
            norm[o + j] += win[j];
        }
        prev = pos;
    }
    y.truncate(len);
    y.iter_mut().zip(&norm).for_each(|(s, n)| *s = if *n > 1e-6 { *s / n } else { 0.0 });
    y
}

/// Up or down by semitones, the length kept: read faster or slower, then back to the length at that pitch.
pub fn pitch(x: &[f32], semitones: f32) -> Vec<f32> {
    let r = 2f32.powf(semitones / 12.0);
    let mut y = speed(&resample(x, r), 1.0 / r);
    y.resize(x.len(), 0.0);
    y
}

/// The cookbook's biquads (R. Bristow-Johnson): a low-pass, a high-pass or a band-pass (0 dB at its peak).
pub fn biquad(x: &[f32], kind: &str, hz: f32, q: f32) -> Vec<f32> {
    let w0 = TAU * hz.min(0.45 * RATE) / RATE;
    let (cs, alpha) = (w0.cos(), w0.sin() / (2.0 * q));
    let b = match kind {
        "lowpass" => [(1.0 - cs) / 2.0, 1.0 - cs, (1.0 - cs) / 2.0],
        "highpass" => [(1.0 + cs) / 2.0, -(1.0 + cs), (1.0 + cs) / 2.0],
        _ => [alpha, 0.0, -alpha],
    };
    let a0 = 1.0 + alpha;
    let (a1, a2) = (-2.0 * cs / a0, (1.0 - alpha) / a0);
    let b = b.map(|v| v / a0);
    let (mut x1, mut x2, mut y1, mut y2) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    x.iter()
        .map(|&s| {
            let y = b[0] * s + b[1] * x1 + b[2] * x2 - a1 * y1 - a2 * y2;
            (x2, x1, y2, y1) = (x1, s, y1, y);
            y
        })
        .collect()
}

/// A small Schroeder reverb after Freeverb: four damped combs side by side, then two all-passes. `size`
/// sets how long it rings (0.3 to 4 s to fall to a thousandth), `damping` how much sooner the highs go,
/// `mix` how loud the room is under the sound. The sound itself comes through untouched.
pub fn reverb(x: &[f32], size: f32, damping: f32, mix: f32) -> Vec<f32> {
    let t60 = (0.3 + 3.7 * size) * RATE;
    let scale = RATE / 44100.0;
    let combs = [1557.0, 1617.0, 1491.0, 1422.0].map(|d: f32| (d * scale) as usize);
    let passes = [556.0, 441.0].map(|d: f32| (d * scale) as usize);
    let len = x.len() + t60 as usize;
    let mut wet = vec![0.0f32; len];
    for d in combs {
        let fb = 0.001f32.powf(d as f32 / t60);
        let (mut buf, mut low, damp) = (vec![0.0f32; d], 0.0f32, damping * 0.4);
        for (i, w) in wet.iter_mut().enumerate() {
            let out = buf[i % d];
            low = out * (1.0 - damp) + low * damp;
            buf[i % d] = x.get(i).copied().unwrap_or(0.0) * (1.0 - fb) + low * fb;
            *w += out * 0.25;
        }
    }
    for d in passes {
        let mut buf = vec![0.0f32; d];
        for (i, w) in wet.iter_mut().enumerate() {
            let out = buf[i % d];
            buf[i % d] = *w + out * 0.5;
            *w = out - *w;
        }
    }
    (0..len).map(|i| x.get(i).copied().unwrap_or(0.0) + mix * wet[i]).collect()
}

/// Repeats `seconds` apart, each `feedback` times the last, until they fall to a thousandth; `mix` how
/// loud the first is. The sound itself comes through untouched.
pub fn echo(x: &[f32], seconds: f32, feedback: f32, mix: f32) -> Vec<f32> {
    let d = ((seconds * RATE) as usize).max(1);
    let repeats = if feedback < 1e-3 { 1 } else { (0.001f32.ln() / feedback.ln()).ceil() as usize };
    let len = x.len() + d * repeats.min((10.0 * RATE) as usize / d).max(1);
    let mut e = vec![0.0f32; len];
    for i in d..len {
        e[i] = x.get(i - d).copied().unwrap_or(0.0) + feedback * e[i - d];
    }
    (0..len).map(|i| x.get(i).copied().unwrap_or(0.0) + mix * e[i]).collect()
}

/// The pitch swung `rate` times a second by up to `depth` semitones either way: a delay that swings.
pub fn vibrato(x: &[f32], rate: f32, depth: f32) -> Vec<f32> {
    // the pitch moves by the delay's slope: it peaks at the depth's ratio
    let most = (2f32.powf(depth / 12.0) - 1.0) / (PI * rate) * RATE;
    (0..x.len())
        .map(|i| {
            let p = i as f32 - most * (0.5 - 0.5 * (TAU * rate * i as f32 / RATE).cos());
            if p < 0.0 {
                return 0.0;
            }
            let k = p as usize;
            x[k] + (x.get(k + 1).copied().unwrap_or(0.0) - x[k]) * (p - k as f32)
        })
        .collect()
}

/// The stretch from `start` to `end` seconds (0: to the end), with a 1 ms fade where it was cut.
pub fn trim(x: Vec<f32>, start: f32, end: f32) -> Vec<f32> {
    let s0 = ((start * RATE) as usize).min(x.len());
    let s1 = if end > 0.0 { ((end * RATE) as usize).clamp(s0, x.len()) } else { x.len() };
    let (cut_head, cut_tail) = (s0 > 0, s1 < x.len());
    let mut y = x[s0..s1].to_vec();
    let (m, n) = ((0.001 * RATE) as usize, y.len());
    for i in 0..m.min(n) {
        let w = i as f32 / m as f32;
        if cut_head {
            y[i] *= w;
        }
        if cut_tail {
            y[n - 1 - i] *= w;
        }
    }
    y
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(hz: f32, secs: f32) -> Vec<f32> {
        (0..(secs * RATE) as usize).map(|i| (TAU * hz * i as f32 / RATE).sin()).collect()
    }
    /// About where the pitch sits, from how often it crosses zero.
    fn hz(x: &[f32]) -> f32 {
        x.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count() as f32 / 2.0 / (x.len() as f32 / RATE)
    }
    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|s| s * s).sum::<f32>() / x.len() as f32).sqrt()
    }
    fn step(t: &str) -> Step {
        Step::parse(&toml::from_str(t).unwrap()).unwrap()
    }
    fn near(a: f32, b: f32, share: f32) -> bool {
        (a - b).abs() <= share * b.abs()
    }

    #[test]
    fn steps_are_read_and_checked() {
        assert_eq!(step("lowpass = 1200\nq = 2"), Step { kind: 3, n: [1200.0, 2.0, 0.0] });
        assert_eq!(step("reverb = 0.5").n, [0.5, 0.5, 0.3], "the numbers left out take their defaults");
        assert_eq!(step("reverse = true").name(), "reverse");
        for bad in ["lowpas = 100", "lowpass = 100\nhighpass = 100", "lowpass = 5", "lowpass = 100\nmix = 1", "trim = 0.5\nend = 0.2", "reverse = 2", "pitch = 'up'"] {
            assert!(Step::parse(&toml::from_str(bad).unwrap()).is_err(), "{bad}");
        }
        let sp = parse_spread(&toml::from_str("pitch = 1.5\nreverb = { mix = 0.1 }").unwrap()).unwrap();
        assert_eq!(sp, vec![Spread { step: 0, number: 0, amount: 1.5 }, Spread { step: 6, number: 2, amount: 0.1 }]);
        for bad in ["pich = 1", "reverse = 1", "reverb = { wet = 1 }", "pitch = -1"] {
            assert!(parse_spread(&toml::from_str(bad).unwrap()).is_err(), "{bad}");
        }
        // a spread keeps a number in its range
        let s = step("crush = 15").spread(&[Spread { step: 15, number: 0, amount: 10.0 }], &mut || 1.0);
        assert_eq!(s.n[0], 16.0);
    }

    #[test]
    fn pitch_moves_the_frequency_and_keeps_the_length() {
        let x = sine(220.0, 1.0);
        let up = apply(x.clone(), &step("pitch = 12"));
        assert_eq!(up.len(), x.len());
        assert!(near(hz(&up[2000..46000]), 440.0, 0.02), "an octave up: {}", hz(&up[2000..46000]));
        let down = apply(x.clone(), &step("pitch = -12"));
        assert!(near(hz(&down[2000..46000]), 110.0, 0.02), "an octave down: {}", hz(&down[2000..46000]));
    }

    #[test]
    fn speed_keeps_the_frequency_and_stretch_does_not() {
        let x = sine(440.0, 1.0);
        let fast = apply(x.clone(), &step("speed = 2"));
        assert_eq!(fast.len(), x.len() / 2);
        assert!(near(hz(&fast[1000..23000]), 440.0, 0.02), "{}", hz(&fast[1000..23000]));
        assert!(rms(&fast[1000..23000]) > 0.6, "no grains that cancel: {}", rms(&fast[1000..23000]));
        let slow = apply(x.clone(), &step("speed = 0.5"));
        assert!(near(slow.len() as f32, 2.0 * x.len() as f32, 0.001) && near(hz(&slow[1000..90000]), 440.0, 0.02));
        let long = apply(x.clone(), &step("stretch = 2"));
        assert!(near(long.len() as f32, 2.0 * x.len() as f32, 0.001) && near(hz(&long), 220.0, 0.02), "a tape played at half speed");
    }

    #[test]
    fn filters_cut_where_they_should() {
        let gain = |t: &str, f: f32| rms(&apply(sine(f, 0.5), &step(t))[4800..]) / rms(&sine(f, 0.5)[4800..]);
        assert!(gain("lowpass = 1000", 200.0) > 0.95 && gain("lowpass = 1000", 8000.0) < 0.03);
        assert!(gain("highpass = 1000", 8000.0) > 0.95 && gain("highpass = 1000", 100.0) < 0.03);
        assert!(gain("bandpass = 1000\nq = 2", 1000.0) > 0.95 && gain("bandpass = 1000\nq = 2", 100.0) < 0.1 && gain("bandpass = 1000\nq = 2", 10000.0) < 0.1);
    }

    #[test]
    fn reverb_and_echo_lengthen_the_tail_and_keep_the_head() {
        let click: Vec<f32> = (0..2400).map(|i| (1.0 - i as f32 / 2400.0) * if i % 40 < 20 { 1.0 } else { -1.0 }).collect();
        for (t, more) in [("reverb = 0.5\nmix = 0.5", 0.9 * RATE), ("echo = 0.2\nfeedback = 0.5\nmix = 0.5", 0.2 * 9.0 * RATE)] {
            let y = apply(click.clone(), &step(t));
            assert!(y.len() as f32 >= click.len() as f32 + more, "{t}: {} samples", y.len());
            let head = 1000;
            assert!(y[..head].iter().zip(&click).all(|(a, b)| (a - b).abs() < 0.05), "{t}: the head as it was");
            assert!(rms(&y[click.len() + 2000..click.len() + 12000]) > 0.002, "{t}: a tail after the sound");
            assert!(y[y.len() - 100..].iter().all(|s| s.abs() < 0.01), "{t}: and the tail ends quiet");
        }
    }

    #[test]
    fn reverse_trim_fade_crush_drive_and_the_swings() {
        let x: Vec<f32> = (0..4800).map(|i| i as f32 / 4800.0).collect();
        let r = apply(x.clone(), &step("reverse = true"));
        assert!(r.iter().rev().eq(x.iter()));
        let t = apply(x.clone(), &step("trim = 0.02\nend = 0.05"));
        assert_eq!(t.len(), 1440);
        assert!((t[700] - x[960 + 700]).abs() < 1e-6 && t[0] == 0.0 && t[1439].abs() < 0.01, "the stretch kept, its cut ends faded");
        let f = apply(vec![1.0; 4800], &step("fade = 0.01\nout = 0.02"));
        assert!(f[0] == 0.0 && f[4799] == 0.0 && f[240] == 0.5 && f[2400] == 1.0 && (f[4799 - 480] - 0.5).abs() < 1e-3);
        let c = apply(sine(100.0, 0.1), &step("crush = 3"));
        let mut levels: Vec<i32> = c.iter().map(|s| (s * 1000.0).round() as i32).collect();
        levels.sort();
        levels.dedup();
        assert!(levels.len() <= 9, "three bits, nine levels at most: {}", levels.len());
        let d = apply(sine(100.0, 0.1), &step("drive = 8"));
        assert!(rms(&d) > 0.85 && d.iter().all(|s| s.abs() <= 1.0), "squarer, never past full: {}", rms(&d));
        assert!(near(rms(&apply(x.clone(), &step("gain = 0.5"))), 0.5 * rms(&x), 1e-5));
        let tr = apply(vec![1.0; 48000], &step("tremolo = 4\ndepth = 0.6"));
        let (lo, hi) = tr.iter().fold((9.0f32, 0.0f32), |(l, h), s| (l.min(*s), h.max(*s)));
        assert!((lo - 0.4).abs() < 1e-3 && (hi - 1.0).abs() < 1e-3);
        let v = apply(sine(1000.0, 1.0), &step("vibrato = 5\ndepth = 1"));
        let f: Vec<f32> = v[4800..].chunks(960).map(hz).collect();
        let (lo, hi) = f.iter().fold((1e9f32, 0.0f32), |(l, h), s| (l.min(*s), h.max(*s)));
        assert!(hi / lo > 1.06 && near(hz(&v), 1000.0, 0.02), "the pitch swings about where it was: {lo} {hi}");
    }
}
