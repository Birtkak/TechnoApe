//! Sounds from outside files (docs/design/studio.md, usability round step 5; ADR 0066): a WAV, OGG, MP3 or FLAC
//! read by symphonia, made one channel at the game's 48 kHz, cut, trimmed of silence, faded and levelled, and
//! written as 48 kHz mono 16-bit WAV, the one format the game reads. Where each recording came from is kept in
//! `data/sounds/credits.toml`; `check=1` refuses a recording without its row there or with a licence other than
//! own, CC0 or CC BY.
use std::path::Path;

pub const RATE: u32 = 48000;
/// The fades at both ends, against clicks.
pub const FADE_MS: f32 = 5.0;
/// A one-shot longer than this is warned of (a sound that long is music or a loop).
pub const LONG_S: f32 = 10.0;
pub const CREDITS: &str = "data/sounds/credits.toml";
/// The licences a recording may have (the developer, 2026-11-12); anything else is refused.
pub const LICENCES: [&str; 3] = ["own", "CC0", "CC BY"];
/// The takes a row may have beside `<row>.wav` (the game's `Bank::build` reads `.1` to `.8`).
pub const TAKES: u32 = 8;

/// A file as read: one channel at its own rate, and what it was.
pub struct Decoded {
    pub samples: Vec<f32>,
    pub rate: u32,
    pub channels: usize,
}

/// Reads a sound file's bytes; `ext` (wav, ogg, mp3, flac) is a hint, the bytes decide.
pub fn decode(bytes: Vec<u8>, ext: &str) -> Result<Decoded, String> {
    use symphonia::core::{audio::SampleBuffer, codecs::DecoderOptions, errors::Error, formats::FormatOptions, io::MediaSourceStream, meta::MetadataOptions, probe::Hint};
    let mss = MediaSourceStream::new(Box::new(std::io::Cursor::new(bytes)), Default::default());
    let mut hint = Hint::new();
    hint.with_extension(ext);
    // gapless: an MP3's encoder delay and padding are cut, so its length is the source's
    let fmt = FormatOptions { enable_gapless: true, ..Default::default() };
    let mut format = symphonia::default::get_probe().format(&hint, mss, &fmt, &MetadataOptions::default()).map_err(|e| format!("not a sound file this reads (WAV, OGG, MP3, FLAC): {e}"))?.format;
    let track = format.default_track().ok_or("no sound in the file")?;
    let (id, rate) = (track.id, track.codec_params.sample_rate.ok_or("the file names no sample rate")?);
    let mut dec = symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default()).map_err(|e| format!("its codec: {e}"))?;
    let (mut samples, mut channels) = (vec![], 1);
    loop {
        let p = match format.next_packet() {
            Ok(p) => p,
            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(Error::ResetRequired) => break,
            Err(e) => return Err(e.to_string()),
        };
        if p.track_id() != id {
            continue;
        }
        let buf = match dec.decode(&p) {
            Ok(b) => b,
            // a damaged packet is skipped, as players do
            Err(Error::DecodeError(_)) => continue,
            Err(e) => return Err(e.to_string()),
        };
        let spec = *buf.spec();
        channels = spec.channels.count().max(1);
        let mut sb = SampleBuffer::<f32>::new(buf.capacity() as u64, spec);
        sb.copy_interleaved_ref(buf);
        samples.extend(sb.samples().chunks(channels).map(|c| c.iter().sum::<f32>() / channels as f32));
    }
    if samples.is_empty() {
        return Err("no samples in the file".into());
    }
    Ok(Decoded { samples, rate, channels })
}

/// `x` at `from` Hz made `RATE` Hz: a windowed sinc (Blackman, 16 zero crossings each side), low-passed under
/// the lower of the two rates' Nyquist so a 96 kHz file does not fold its highs down.
pub fn resample(x: &[f32], from: u32) -> Vec<f32> {
    if from == RATE {
        return x.to_vec();
    }
    let step = from as f64 / RATE as f64;
    let fc = (1.0 / step).min(1.0);
    let half = (16.0 / fc).ceil() as i64;
    let n = (x.len() as f64 / step).round() as usize;
    let sinc = |t: f64| if t.abs() < 1e-9 { 1.0 } else { (std::f64::consts::PI * t).sin() / (std::f64::consts::PI * t) };
    (0..n)
        .map(|i| {
            let t = i as f64 * step;
            let c = t.floor() as i64;
            let mut s = 0.0;
            for k in (c - half + 1)..=(c + half) {
                let Some(v) = usize::try_from(k).ok().and_then(|k| x.get(k)) else { continue };
                let d = t - k as f64;
                let u = std::f64::consts::PI * d / half as f64;
                let w = 0.42 + 0.5 * u.cos() + 0.08 * (2.0 * u).cos();
                s += *v as f64 * fc * sinc(fc * d) * w;
            }
            s as f32
        })
        .collect()
}

/// A file read and made one channel at 48 kHz.
pub fn load(path: &Path) -> Result<(Vec<f32>, Decoded), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let d = decode(bytes, &ext).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok((resample(&d.samples, d.rate), d))
}

/// How the import panel makes the file: the cut as shares of the whole (0..1), trim and level.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Make {
    pub start: f32,
    pub end: f32,
    pub trim: bool,
    /// The peak to level to (the row's), or none to keep the file's own (held under 1).
    pub level: Option<f32>,
}
impl Default for Make {
    fn default() -> Make {
        Make { start: 0.0, end: 1.0, trim: true, level: Some(1.0) }
    }
}

/// Where the sound lies in `x`: the first and last sample above -40 dB of its peak, with the fades' length of
/// quiet kept before and after so the fades fall in the quiet and the attack stays whole.
pub fn sound_bounds(x: &[f32]) -> (usize, usize) {
    let peak = x.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    let floor = peak * 0.01;
    let (Some(a), Some(b)) = (x.iter().position(|s| s.abs() > floor), x.iter().rposition(|s| s.abs() > floor)) else { return (0, x.len()) };
    let pad = (FADE_MS / 1000.0 * RATE as f32) as usize;
    (a.saturating_sub(pad), (b + 1 + pad).min(x.len()))
}

/// A handle's share of `len` samples as an index (f32 rounds a long file's last index past its end).
pub fn share(len: usize, k: f32) -> usize {
    ((k.clamp(0.0, 1.0) as f64 * len as f64) as usize).min(len)
}

/// The samples `m` keeps of `x` (48 kHz), faded at both ends and levelled.
pub fn make(x: &[f32], m: &Make) -> Vec<f32> {
    let (a, b) = (share(x.len(), m.start), share(x.len(), m.end));
    let mut y = x[a.min(b)..b.max(a)].to_vec();
    if m.trim {
        let (a, b) = sound_bounds(&y);
        y = y[a..b].to_vec();
    }
    let fade = ((FADE_MS / 1000.0 * RATE as f32) as usize).min(y.len() / 2);
    let n = y.len();
    for i in 0..fade {
        let g = i as f32 / fade as f32;
        y[i] *= g;
        y[n - 1 - i] *= g;
    }
    let peak = y.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    let to = match m.level {
        Some(p) => p.min(1.0),
        None => peak.min(1.0),
    };
    if peak > 1e-6 {
        y.iter_mut().for_each(|s| *s *= to / peak);
    }
    y
}

/// The bytes of `x` as 48 kHz mono 16-bit WAV.
pub fn wav_bytes(x: &[f32]) -> Vec<u8> {
    let spec = hound::WavSpec { channels: 1, sample_rate: RATE, bits_per_sample: 16, sample_format: hound::SampleFormat::Int };
    let mut out = std::io::Cursor::new(vec![]);
    {
        let mut w = hound::WavWriter::new(&mut out, spec).expect("a WAV header in memory");
        for s in x {
            w.write_sample((s.clamp(-1.0, 1.0) * 32767.0).round() as i16).expect("a sample in memory");
        }
        w.finalize().expect("a WAV in memory");
    }
    out.into_inner()
}

/// Writes a file through a temporary one renamed over it (a game reading the folder never sees half a file),
/// or removes it for None.
pub fn put_file(path: &str, bytes: Option<&[u8]>) -> Result<(), String> {
    let why = |e: std::io::Error| format!("{path}: {e}");
    match bytes {
        None => match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(why(e)),
            _ => Ok(()),
        },
        Some(b) => {
            if let Some(d) = Path::new(path).parent() {
                std::fs::create_dir_all(d).map_err(why)?;
            }
            let tmp = format!("{path}.tmp");
            std::fs::write(&tmp, b).and_then(|_| std::fs::rename(&tmp, path)).map_err(|e| {
                let _ = std::fs::remove_file(&tmp);
                why(e)
            })
        }
    }
}

/// "Use as this sound": `<row>.wav` in `dir`, or the next free take `<row>.N.wav` when the row has one; none
/// when its eight takes are there.
pub fn take_name(dir: &Path, row: &str) -> Option<String> {
    std::iter::once(format!("{row}.wav")).chain((1..=TAKES).map(|k| format!("{row}.{k}.wav"))).find(|n| !dir.join(n).exists())
}

/// The recordings a row has in `dir`, as the game finds them.
pub fn recordings(dir: &Path, row: &str) -> Vec<String> {
    std::iter::once(format!("{row}.wav")).chain((1..=TAKES).map(|k| format!("{row}.{k}.wav"))).filter(|n| dir.join(n).exists()).collect()
}

/// A base's name from a file's: lower case, a space or anything odd an underscore.
pub fn base_name(file: &str) -> String {
    let stem = Path::new(file).file_stem().and_then(|s| s.to_str()).unwrap_or("sound");
    stem.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' { c.to_ascii_lowercase() } else { '_' }).collect()
}

/// One row of the credits: the recording (its path under `data/sounds/`), where it came from, its licence, and
/// for CC BY the author and the title a credits screen shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Credit {
    pub file: String,
    pub source: String,
    pub licence: String,
    pub author: String,
    pub title: String,
}
impl Credit {
    pub fn row(&self) -> String {
        let q = |s: &str| crate::js(s);
        format!("[[credit]]\nname = {}\nsource = {}\nlicence = {}\nauthor = {}\ntitle = {}\n", q(&self.file), q(&self.source), q(&self.licence), q(&self.author), q(&self.title))
    }
    /// Why it cannot be kept, if it cannot.
    pub fn refused(&self) -> Option<String> {
        if !LICENCES.contains(&self.licence.as_str()) {
            return Some(if self.licence.is_empty() { "pick its licence".into() } else { format!("licence '{}': only own, CC0 and CC BY are taken", self.licence) });
        }
        if self.licence != "own" && self.source.trim().is_empty() {
            return Some("say where it came from (a URL)".into());
        }
        (self.licence == "CC BY" && (self.author.trim().is_empty() || self.title.trim().is_empty())).then(|| "CC BY needs its author and title (the credits screen shows them)".into())
    }
}

const HEAD: &str = "# Where each recording under data/sounds/ came from (ADR 0066): `name` its path there, `source` a URL,\n# `licence` own, CC0 or CC BY (`check=1` refuses any other, and a recording with no row). Before any public\n# release every CC BY row must be shown on a credits screen (author, title, source, licence).\n";

/// The credits file's text with `c`'s row put in (a new file starts with its header).
pub fn put_credit(text: &str, c: &Credit) -> String {
    put_row(text, &c.file, &c.row())
}
/// The credits file's text with the row of `file` set to `row` as written.
pub fn put_row(text: &str, file: &str, row: &str) -> String {
    let text = if text.trim().is_empty() { HEAD } else { text };
    crate::rows::put(text, "credit", file, row)
}
/// The row of `file` as written, if the credits file has one.
pub fn credit_row(text: &str, file: &str) -> Option<String> {
    crate::rows::spans(text).into_iter().find(|s| s.header == "credit" && s.name == file).map(|s| text[s.at].to_string())
}
/// The credits file's text without the row of `file` (and the blank line before it); every other byte stays.
pub fn drop_credit(text: &str, file: &str) -> String {
    let Some(s) = crate::rows::spans(text).into_iter().find(|s| s.header == "credit" && s.name == file) else { return text.to_string() };
    let e = crate::rows::eol(text);
    let head = &text[..s.at.start];
    let head = head.strip_suffix(e).filter(|h| h.ends_with(e)).unwrap_or(head);
    format!("{head}{}", &text[s.at.end..])
}

/// `check=1`'s rule over `dir` (`data/sounds`): every recording, `*.wav` and `base/*.wav`, has a row in its
/// `credits.toml` with a licence the game may ship. Each refusal names the file.
pub fn check_credits(dir: &Path) -> Vec<String> {
    let wavs = |sub: &str| -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir.join(sub)).into_iter().flatten().flatten().filter_map(|e| e.file_name().to_str().map(String::from)).filter(|n| n.ends_with(".wav")).map(|n| if sub.is_empty() { n } else { format!("{sub}/{n}") }).collect();
        v.sort();
        v
    };
    let files: Vec<String> = wavs("").into_iter().chain(wavs("base")).collect();
    if files.is_empty() {
        return vec![];
    }
    let rows: Vec<toml::Table> = match std::fs::read_to_string(dir.join("credits.toml")).map(|t| toml::from_str::<toml::Table>(&t)) {
        Err(_) => vec![],
        Ok(Err(e)) => return vec![format!("{CREDITS}: {}", e.message())],
        Ok(Ok(t)) => t.get("credit").and_then(|c| c.as_array()).into_iter().flatten().filter_map(|r| r.as_table().cloned()).collect(),
    };
    let s = |r: &toml::Table, k: &str| r.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    files
        .into_iter()
        .filter_map(|f| {
            let shown = format!("data/sounds/{f}");
            match rows.iter().find(|r| s(r, "name") == f) {
                None => Some(format!("{shown}: no row in {CREDITS} (where it came from and its licence)")),
                Some(r) => {
                    let l = s(r, "licence");
                    if !LICENCES.contains(&l.as_str()) {
                        Some(format!("{shown}: licence '{l}' in {CREDITS}: only own, CC0 and CC BY are taken"))
                    } else {
                        // the release duty: a credits screen shows these for CC BY
                        let missing: Vec<&str> = ["source", "author", "title"].into_iter().filter(|k| l == "CC BY" && s(r, k).trim().is_empty()).collect();
                        (!missing.is_empty()).then(|| format!("{shown}: CC BY in {CREDITS} without its {}", missing.join(", ")))
                    }
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(f: &str) -> (Vec<f32>, Decoded) {
        load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata").join(f)).unwrap()
    }

    #[test]
    fn each_format_decodes_to_the_same_length_within_a_frame() {
        // the four files are one sound (0.35 s at 44.1 kHz, stereo, tone from 50 ms to 300 ms) made by ffmpeg
        let (wav, d) = read("tone.wav");
        assert_eq!((d.rate, d.channels), (44100, 2));
        assert_eq!(wav.len(), (0.35 * RATE as f32).round() as usize);
        // a frame: an MP3's 1152 samples at the file's rate, made 48 kHz
        let frame = (1152.0 * RATE as f32 / 44100.0).ceil() as i64;
        for f in ["tone.ogg", "tone.mp3", "tone.flac"] {
            let (x, d) = read(f);
            assert_eq!(d.rate, 44100, "{f}");
            assert!((x.len() as i64 - wav.len() as i64).abs() <= frame, "{f}: {} samples against the WAV's {}", x.len(), wav.len());
            let peak = x.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            // the two channels made one: (0.5 sin 440 + 0.4 sin 660) / 2 peaks near 0.45
            assert!((0.38..=0.5).contains(&peak), "{f}: peak {peak}");
        }
        // FLAC is lossless: the same samples as the WAV
        let (flac, _) = read("tone.flac");
        assert!(flac.iter().zip(&wav).all(|(a, b)| (a - b).abs() < 1e-4));
        assert!(decode(b"not a sound".to_vec(), "wav").is_err());
    }

    #[test]
    fn resampling_keeps_a_tone_and_its_length() {
        let tone: Vec<f32> = (0..44100).map(|i| (i as f32 / 44100.0 * 1000.0 * std::f32::consts::TAU).sin() * 0.5).collect();
        let y = resample(&tone, 44100);
        assert_eq!(y.len(), 48000);
        // the same tone at the new rate, away from the ends
        let err = (1000..47000).map(|i| (y[i] - (i as f32 / 48000.0 * 1000.0 * std::f32::consts::TAU).sin() * 0.5).abs()).fold(0.0f32, f32::max);
        assert!(err < 0.01, "{err}");
        assert_eq!(resample(&tone, 48000), tone);
        // 96 kHz down: a 30 kHz tone (over 48 kHz's Nyquist) is cut, not folded to 18 kHz
        let high: Vec<f32> = (0..9600).map(|i| (i as f32 / 96000.0 * 30000.0 * std::f32::consts::TAU).sin()).collect();
        let y = resample(&high, 96000);
        assert!(y[500..4300].iter().all(|s| s.abs() < 0.02), "{}", y[500..4300].iter().fold(0.0f32, |m, s| m.max(s.abs())));
    }

    #[test]
    fn the_panel_cuts_trims_fades_and_levels() {
        let (x, _) = read("tone.wav");
        let ms = |n: usize| n as f32 / RATE as f32 * 1000.0;
        // trim: the 50 ms of silence at each end go, the fades' 5 ms kept
        let y = make(&x, &Make::default());
        assert!((ms(y.len()) - 260.0).abs() < 2.0, "{} ms", ms(y.len()));
        assert_eq!((y[0], *y.last().unwrap()), (0.0, 0.0), "faded to nothing at both ends");
        let peak = y.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!((peak - 1.0).abs() < 1e-5, "levelled to the row's peak: {peak}");
        // untrimmed, unlevelled, cut to the second half
        let y = make(&x, &Make { start: 0.5, end: 1.0, trim: false, level: None });
        assert_eq!(y.len(), x.len() - x.len() / 2);
        assert!(y.iter().fold(0.0f32, |m, s| m.max(s.abs())) < 0.5);
        // handles crossed are the same cut
        assert_eq!(make(&x, &Make { start: 0.8, end: 0.2, trim: false, level: None }).len(), make(&x, &Make { start: 0.2, end: 0.8, trim: false, level: None }).len());
        // written as the game reads it: 48 kHz mono 16 bit
        let r = hound::WavReader::new(std::io::Cursor::new(wav_bytes(&y))).unwrap();
        assert_eq!((r.spec().sample_rate, r.spec().channels, r.spec().bits_per_sample, r.len() as usize), (48000, 1, 16, y.len()));
    }

    #[test]
    fn takes_and_credits() {
        let dir = std::env::temp_dir().join(format!("studio-import-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("base")).unwrap();
        assert_eq!(take_name(&dir, "a.b").as_deref(), Some("a.b.wav"));
        std::fs::write(dir.join("a.b.wav"), wav_bytes(&[0.0; 10])).unwrap();
        assert_eq!(take_name(&dir, "a.b").as_deref(), Some("a.b.1.wav"));
        for k in 1..=8 {
            std::fs::write(dir.join(format!("a.b.{k}.wav")), b"").unwrap();
        }
        assert_eq!(take_name(&dir, "a.b"), None, "eight takes: no more");
        assert_eq!(recordings(&dir, "a.b").len(), 9);
        assert_eq!(base_name("C:/x/Door Knock 02.MP3"), "door_knock_02");
        // check: a recording with no row, then with one of each licence
        std::fs::write(dir.join("base/knock.wav"), b"").unwrap();
        let bad = check_credits(&dir);
        assert_eq!(bad.len(), 10, "{bad:?}");
        assert_eq!(bad[0], "data/sounds/a.b.1.wav: no row in data/sounds/credits.toml (where it came from and its licence)");
        let mut text = String::new();
        for k in 1..=8 {
            text = put_credit(&text, &Credit { file: format!("a.b.{k}.wav"), source: "https://x".into(), licence: "CC0".into(), ..Default::default() });
        }
        text = put_credit(&text, &Credit { file: "a.b.wav".into(), licence: "own".into(), ..Default::default() });
        text = put_credit(&text, &Credit { file: "base/knock.wav".into(), source: "https://y".into(), licence: "other".into(), ..Default::default() });
        std::fs::write(dir.join("credits.toml"), &text).unwrap();
        assert_eq!(check_credits(&dir), vec!["data/sounds/base/knock.wav: licence 'other' in data/sounds/credits.toml: only own, CC0 and CC BY are taken".to_string()]);
        // a row put again replaces itself
        text = put_credit(&text, &Credit { file: "base/knock.wav".into(), source: "https://y".into(), licence: "CC BY".into(), ..Default::default() });
        std::fs::write(dir.join("credits.toml"), &text).unwrap();
        assert_eq!(check_credits(&dir), vec!["data/sounds/base/knock.wav: CC BY in data/sounds/credits.toml without its author, title".to_string()]);
        text = put_credit(&text, &Credit { file: "base/knock.wav".into(), source: "https://y".into(), licence: "CC BY".into(), author: "Someone".into(), title: "Knock".into() });
        std::fs::write(dir.join("credits.toml"), &text).unwrap();
        assert!(check_credits(&dir).is_empty());
        assert_eq!(text.matches("[[credit]]").count(), 10);
        assert!(text.starts_with("# Where each recording"));
        // a row taken out and put back: the file as it was, byte for byte
        let row = credit_row(&text, "a.b.3.wav").unwrap();
        let without = drop_credit(&text, "a.b.3.wav");
        assert_eq!(without.matches("[[credit]]").count(), 9);
        assert!(!without.contains("\n\n\n"), "no blank lines left behind");
        assert_eq!(drop_credit(&without, "a.b.3.wav"), without);
        let back = put_row(&without, "a.b.3.wav", &row);
        assert_eq!(credit_row(&back, "a.b.3.wav").as_deref(), Some(row.as_str()));
        assert!(Credit { licence: "CC BY".into(), source: "s".into(), author: "a".into(), ..Default::default() }.refused().is_some(), "no title");
        assert_eq!(share(16777219, 1.0), 16777219, "a long file's end is its length");
        // what the panel refuses before writing
        assert!(Credit { licence: "CC BY".into(), ..Default::default() }.refused().is_some(), "no source");
        assert!(Credit { licence: "other".into(), source: "s".into(), ..Default::default() }.refused().is_some());
        assert!(Credit { licence: "own".into(), ..Default::default() }.refused().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
