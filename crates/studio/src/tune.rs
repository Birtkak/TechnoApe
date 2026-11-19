//! `tune=`: a row's numbers tried and judged by numbers, with no window and no renderer, so a try costs
//! milliseconds. `tune=-` takes commands from stdin and answers each with one JSON line, until `quit` or the end
//! of the input; `tune="creature boar; set swing 35; save"` runs them from the argument, `;` apart, and stops at
//! the first refusal. Commands:
//! `creature <name>` (a row of `data/creatures.toml`, read again from the file), `show`,
//! `set <key> <value>` (within the key's range; answers with the gait it makes),
//! `gait [walk|run|<m/s>]` (measures at that speed, which `set`, `sweep` and `film` keep using),
//! `sweep <key> <from> <to> <n>` (n values measured; the row stays as it was),
//! `film <png>` (eight side views of one cycle over a ground that moves at the speed),
//! `save` (rewrites the row in the file: a running game and the Studio's window show it within a second).
//! `hand` chooses the first-person swing of `data/hand.toml` (ADR 0172) in a creature's place: `show`, `set`,
//! `sweep` and `save` work on its numbers and answer with the swing they make (a running game shows a save
//! within a second; there is no film of it).
use crate::js;
use engine::creatures::{self, Creature, RANGES};
use engine::hand::{self, Hand};
use engine::models::Model;
use engine::palette::Palette;
use glam::{DVec3, Mat4, Vec3};
use std::path::{Path, PathBuf};

/// Samples of one walk cycle.
const N: usize = 96;

/// A walk by numbers. A foot is on the ground while it moves back under the body (half the cycle).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gait {
    /// Seconds of one cycle (two steps).
    pub cycle_s: f32,
    /// Metres a foot moves back under the body while on the ground, the legs' mean; the row's `step` when the
    /// feet keep pace with the ground.
    pub sweep_m: f32,
    /// Centimetres a step's ground is longer than the sweep: over 0 the feet skate forward, under 0 they push
    /// back faster than the ground goes; 0 is a foot that ends its step where it began it.
    pub drift_cm: f32,
    /// Centimetres a foot slips over the ground in one step, forward and back added up, the worst leg's; 0 is a
    /// foot that stays put (a leg that swings as a pendulum never quite does).
    pub slide_cm: f32,
    /// Centimetres a foot goes under the ground at its deepest.
    pub sink_cm: f32,
    /// Centimetres a foot is over the ground at its highest while it should be on it.
    pub hover_cm: f32,
}

/// Each leg's foot: the part of the leg (itself or one hung from it) lowest at rest, and the middle of its
/// underside in its own voxels.
fn feet(m: &Model, c: &Creature) -> Vec<(usize, Vec3)> {
    let rest = m.pose(None, 0.0);
    let under = |q: usize| Vec3::new(m.parts[q].size[0] as f32 / 2.0, 0.0, m.parts[q].size[2] as f32 / 2.0);
    let low = |q: usize| rest[q].transform_point3(under(q)).y;
    let of = |q: usize, leg: &str| {
        let mut p = &m.parts[q];
        for _ in 0..m.parts.len() {
            if p.name == leg {
                return true;
            }
            match m.parts.iter().find(|r| r.name == p.parent) {
                Some(r) => p = r,
                None => break,
            }
        }
        false
    };
    c.legs.iter().filter_map(|leg| (0..m.parts.len()).filter(|&q| of(q, leg)).min_by(|&a, &b| low(a).total_cmp(&low(b))).map(|q| (q, under(q)))).collect()
}

/// The walk `engine::creatures::walk` makes of the row at `speed` m/s, measured; none for a creature without
/// legs, legs the model lacks, or a speed that stands.
pub fn gait(m: &Model, c: &Creature, speed: f32) -> Option<Gait> {
    let feet = feet(m, c);
    let clip = creatures::walk(m, c, speed);
    if feet.is_empty() || feet.len() != c.plan.legs() || clip.name != "walk" {
        return None;
    }
    let (length, speed) = (clip.length, speed.min(25.0));
    let rest = m.pose(None, 0.0);
    let mut m = m.clone();
    m.clips = vec![clip];
    let poses: Vec<Vec<Mat4>> = (0..=N).map(|k| m.pose(Some("walk"), length * k as f32 / N as f32)).collect();
    let dt = length / N as f32;
    let mut g = Gait { cycle_s: length, sweep_m: 0.0, drift_cm: 0.0, slide_cm: 0.0, sink_cm: 0.0, hover_cm: 0.0 };
    for &(q, p) in &feet {
        // metres from the body, as drawn: the front is +z, so the ground passes under it towards -z
        let at = |pose: &Vec<Mat4>| pose[q].transform_point3(p) * c.scale;
        let ground = at(&rest).y;
        let pts: Vec<Vec3> = poses.iter().map(at).collect();
        // how far the foot slips over the ground in each sample; it is on the ground for the half cycle in
        // which it slips least (a leg that hangs still is dragged a whole step in any half)
        let slip: Vec<f32> = (0..N).map(|k| (speed * dt + pts[k + 1].z - pts[k].z).abs()).collect();
        let on = |start: usize| (0..N / 2).map(move |i| (start + i) % N);
        let slid = |start: usize| on(start).map(|k| slip[k]).sum::<f32>();
        let start = (0..N).min_by(|&a, &b| slid(a).total_cmp(&slid(b))).unwrap();
        g.sink_cm = pts.iter().fold(g.sink_cm, |s, p| s.max((ground - p.y) * 100.0));
        g.hover_cm = on(start).fold(g.hover_cm, |h, k| h.max((pts[k].y - ground) * 100.0));
        g.sweep_m += on(start).map(|k| pts[k].z - pts[k + 1].z).sum::<f32>() / feet.len() as f32;
        g.slide_cm = g.slide_cm.max(slid(start) * 100.0);
    }
    g.drift_cm = (speed * length / 2.0 - g.sweep_m) * 100.0;
    Some(g)
}

fn gait_json(m: &Model, c: &Creature, speed: f32) -> String {
    let head = format!("\"creature\":{},\"speed\":{speed},\"step_m\":{}", js(&c.name), c.step);
    match gait(m, c, speed) {
        Some(g) => format!("{{{head},\"cycle_s\":{:.3},\"sweep_m\":{:.3},\"drift_cm\":{:.2},\"slide_cm\":{:.2},\"sink_cm\":{:.2},\"hover_cm\":{:.2}}}", g.cycle_s, g.sweep_m, g.drift_cm, g.slide_cm, g.sink_cm, g.hover_cm),
        None => format!("{{{head},\"gait\":null}}"),
    }
}

/// A swing of the hand by numbers.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Swing {
    /// Seconds from its start to its peak (the strike); the rest of `seconds` is the way back.
    pub peak_s: f64,
    /// Centimetres the hand's end is from its rest at the peak.
    pub travel_cm: f64,
    /// Metres a second the hand's end moves at its fastest on the way in.
    pub strike_m_s: f64,
}

pub fn swing(h: &Hand) -> Swing {
    const K: usize = 400;
    // any eye will do: lengths are the same in every frame
    let (fwd, up) = (DVec3::Z, DVec3::Y);
    let right = fwd.cross(up);
    let t = |k: usize| h.seconds * k as f64 / K as f64;
    let at = |k: usize| h.turn(h.curve(t(k)) as f64, fwd, right, up).transform_point3(h.hand_at(fwd, right, up));
    let peak = (0..=K).max_by(|&a, &b| h.curve(t(a)).total_cmp(&h.curve(t(b)))).unwrap();
    let fastest = (0..peak).map(|k| (at(k + 1) - at(k)).length() / t(1)).fold(0.0, f64::max);
    Swing { peak_s: t(peak), travel_cm: (at(peak) - at(0)).length() * 100.0, strike_m_s: fastest }
}

fn swing_json(h: &Hand) -> String {
    let s = swing(h);
    format!("{{\"seconds\":{},\"peak_s\":{:.3},\"return_s\":{:.3},\"travel_cm\":{:.1},\"strike_m_s\":{:.2},\"chop_deg\":{:.1}}}", h.get("seconds"), s.peak_s, h.seconds - s.peak_s, s.travel_cm, s.strike_m_s, h.chop.to_degrees())
}

/// `old` (the file's text) with each number's line set to the hand's: comments and every other byte stay.
fn hand_text(old: &str, h: &Hand) -> String {
    let line = |l: &str| match hand::RANGES.iter().find(|r| l.starts_with(&format!("{} = ", r.0))) {
        Some(r) => format!("{} = {:?}{}", r.0, h.get(r.0), &l[l.trim_end_matches(['\r', '\n']).len()..]),
        None => l.to_string(),
    };
    old.split_inclusive('\n').map(line).collect()
}

/// One command on the hand.
fn run_hand(h: &mut Hand, file: &str, line: &str, verb: &str, a: &[&str]) -> Result<String, String> {
    let range = |key: &str, v: f32| match hand::RANGES.iter().find(|r| r.0 == key) {
        Some(r) if (r.1..=r.2).contains(&v) => Ok(()),
        Some(r) => Err(format!("{key} = {v} is outside {}..{} {}", r.1, r.2, r.3)),
        None => Err(format!("no key '{key}' (the keys: {})", hand::RANGES.map(|r| r.0).join(", "))),
    };
    match (verb, a) {
        ("hand" | "show", _) => {
            let values: Vec<String> = hand::RANGES.iter().map(|r| format!("\"{}\":{}", r.0, h.get(r.0))).collect();
            let ranges: Vec<String> = hand::RANGES.iter().map(|r| format!("\"{}\":[{},{}]", r.0, r.1, r.2)).collect();
            Ok(format!("{{\"hand\":{},\"values\":{{{}}},\"ranges\":{{{}}},\"swing\":{}}}", js(file), values.join(","), ranges.join(","), swing_json(h)))
        }
        ("set", [key, v]) => {
            let v = num(v)?;
            range(key, v)?;
            h.set(key, v);
            Ok(swing_json(h))
        }
        ("sweep", [key, from, to, n]) => {
            let (from, to, n) = (num(from)?, num(to)?, n.parse::<usize>().ok().filter(|n| (2..=200).contains(n)).ok_or("sweep: 2 to 200 values")?);
            range(key, from)?;
            range(key, to)?;
            let rows: Vec<String> = (0..n)
                .map(|i| {
                    let (mut t, v) = (h.clone(), from + (to - from) * i as f32 / (n - 1) as f32);
                    t.set(key, v);
                    format!("{{\"v\":{v},\"swing\":{}}}", swing_json(&t))
                })
                .collect();
            Ok(format!("{{\"sweep\":{},\"rows\":[{}]}}", js(key), rows.join(",")))
        }
        ("save", []) => {
            let new = hand_text(&std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?, h);
            let read = Hand::parse(&new)?;
            if let Some(r) = hand::RANGES.iter().find(|r| read.get(r.0) != h.get(r.0)) {
                return Err(format!("{file}: no line '{} = ...' to set", r.0));
            }
            crate::import::put_file(file, Some(new.as_bytes()))?;
            Ok(format!("{{\"saved\":\"hand\",\"file\":{}}}", js(file)))
        }
        ("film" | "gait", _) => Err("the hand has neither: run the game, it shows a saved data/hand.toml within a second".into()),
        _ => Err(format!("no command '{line}' (hand, show, set <key> <value>, sweep <key> <from> <to> <n>, save, quit)")),
    }
}

fn num(s: &str) -> Result<f32, String> {
    s.parse::<f32>().ok().filter(|v| v.is_finite()).ok_or(format!("'{s}' is no number"))
}

/// A key's range, and `v` refused outside it.
fn in_range(key: &str, v: f32) -> Result<(), String> {
    let keys = || RANGES.map(|r| r.0).join(", ");
    let (_, lo, hi, unit, _) = RANGES.into_iter().find(|r| r.0 == key).ok_or_else(|| format!("no key '{key}' (the keys: {})", keys()))?;
    if (lo..=hi).contains(&v) {
        Ok(())
    } else {
        Err(format!("{key} = {v} is outside {lo}..{hi} {unit}"))
    }
}

fn inside(q: &[[f32; 2]; 4], p: [f32; 2]) -> bool {
    let s = [0, 1, 2, 3].map(|i| {
        let (a, b) = (q[i], q[(i + 1) % 4]);
        (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
    });
    s.iter().all(|v| *v >= 0.0) || s.iter().all(|v| *v <= 0.0)
}

/// Eight side views of one cycle (the front to the right), each part its middle slice, over a ground line whose
/// marks, 25 cm apart, pass at the speed: a foot that keeps beside its mark does not slide.
fn film(m: &Model, c: &Creature, speed: f32, path: &str) -> Result<String, String> {
    const F: usize = 8;
    const W: usize = 200;
    const H: usize = 150;
    let clip = creatures::walk(m, c, speed);
    let (name, length) = (clip.name.clone(), clip.length);
    let mut m = m.clone();
    m.clips = vec![clip];
    let quads = |poses: &[Mat4]| -> Vec<([[f32; 2]; 4], [u8; 3])> {
        m.parts
            .iter()
            .zip(poses)
            .enumerate()
            .map(|(i, (part, pose))| {
                // (a model of painted boxes has no voxel colours: each part its own grey)
                let [sx, sy, sz] = part.size.map(|v| v as f32);
                let q = [[0.0, 0.0], [0.0, sy], [sz, sy], [sz, 0.0]].map(|[z, y]| {
                    let v = pose.transform_point3(Vec3::new(sx / 2.0, y, z)) * c.scale;
                    [v.z, v.y]
                });
                (q, part.cells.iter().find(|v| **v != 0).map_or([110 + (i * 53 % 120) as u8; 3], |v| m.rgb[*v as usize - 1]))
            })
            .collect()
    };
    let rest = quads(&m.pose(None, 0.0));
    let frames: Vec<_> = (0..F).map(|k| quads(&m.pose(Some(&name), length * k as f32 / F as f32))).collect();
    let all = || frames.iter().chain([&rest]).flatten().flat_map(|q| q.0);
    let (z0, z1) = all().fold((f32::MAX, f32::MIN), |a, p| (a.0.min(p[0]), a.1.max(p[0])));
    let (y0, y1) = all().fold((f32::MAX, f32::MIN), |a, p| (a.0.min(p[1]), a.1.max(p[1])));
    let ground = rest.iter().flat_map(|q| q.0).fold(f32::MAX, |a, p| a.min(p[1]));
    let k = ((W - 16) as f32 / (z1 - z0).max(0.01)).min((H - 24) as f32 / (y1 - y0).max(0.01));
    let to_px = |p: [f32; 2]| [W as f32 / 2.0 + (p[0] - (z0 + z1) / 2.0) * k, (H - 14) as f32 - (p[1] - y0) * k];
    let mut px: Vec<u8> = [34, 34, 38, 255].repeat(F * W * H);
    let mut put = |f: usize, x: i32, y: i32, rgb: [u8; 3]| {
        if (0..W as i32).contains(&x) && (0..H as i32).contains(&y) {
            let i = (y as usize * F * W + f * W + x as usize) * 4;
            px[i..i + 3].copy_from_slice(&rgb);
        }
    };
    for (f, frame) in frames.iter().enumerate() {
        for (q, rgb) in frame {
            let q = q.map(to_px);
            let (x0, x1) = q.iter().fold((f32::MAX, f32::MIN), |a, p| (a.0.min(p[0]), a.1.max(p[0])));
            let (ya, yb) = q.iter().fold((f32::MAX, f32::MIN), |a, p| (a.0.min(p[1]), a.1.max(p[1])));
            for y in ya.floor() as i32..=yb.ceil() as i32 {
                for x in x0.floor() as i32..=x1.ceil() as i32 {
                    if inside(&q, [x as f32 + 0.5, y as f32 + 0.5]) {
                        put(f, x, y, *rgb);
                    }
                }
            }
        }
        let gy = to_px([0.0, ground])[1] as i32 + 1;
        for x in 0..W as i32 {
            put(f, x, gy, [120, 120, 120]);
        }
        // the ground's marks have passed speed x t behind the body
        let back = speed.min(25.0) * length * f as f32 / F as f32;
        for i in -40..=40 {
            let x = to_px([i as f32 * 0.25 - back, 0.0])[0] as i32;
            for y in gy + 1..gy + 6 {
                put(f, x, y, [230, 200, 60]);
            }
        }
        for y in 0..H as i32 {
            put(f, 0, y, [60, 60, 66]);
        }
    }
    crate::write_rgba(path, (F * W) as u32, H as u32, &px)?;
    Ok(format!("{{\"film\":{},\"frames\":{F},\"clip\":{},\"cycle_s\":{length:.3},\"speed\":{speed}}}", js(path), js(&name)))
}

pub struct Session {
    data: PathBuf,
    pal: Palette,
    cur: Option<(Creature, Model)>,
    /// The hand, when it was chosen last.
    hand: Option<Hand>,
    /// "walk", "run" or metres a second.
    speed: String,
}

impl Session {
    /// On the `data/` folder of `root`.
    pub fn new(root: &Path) -> Result<Session, String> {
        let data = root.join("data");
        let pal = data.join("palette.toml");
        let pal = Palette::parse(&std::fs::read_to_string(&pal).map_err(|e| format!("{}: {e}", pal.display()))?)?;
        Ok(Session { data, pal, cur: None, hand: None, speed: "walk".into() })
    }

    fn file(&self) -> String {
        self.data.join("creatures.toml").to_string_lossy().into_owned()
    }

    /// One command; its JSON line, or why it was refused.
    pub fn run(&mut self, line: &str) -> Result<String, String> {
        let line = line.trim();
        let (verb, rest) = line.split_once(' ').map_or((line, ""), |(v, r)| (v, r.trim()));
        let a: Vec<&str> = rest.split_whitespace().collect();
        if verb == "creature" {
            let file = self.file();
            let rows = creatures::rows(&std::fs::read_to_string(&file).map_err(|e| format!("{file}: {e}"))?.replace("\r\n", "\n"))?;
            let names = rows.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(", ");
            let c = rows.iter().find(|c| c.name == rest).ok_or(format!("no creature '{rest}' (there are: {names})"))?.clone();
            let path = self.data.join("models").join(format!("{}.toml", c.model));
            let m = Model::parse(&c.model, &std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?, &self.pal)?;
            (self.cur, self.hand, self.speed) = (Some((c, m)), None, "walk".into());
        }
        let hand_file = self.data.join("hand.toml").to_string_lossy().into_owned();
        if verb == "hand" {
            self.hand = Some(Hand::parse(&std::fs::read_to_string(&hand_file).map_err(|e| format!("{hand_file}: {e}"))?)?);
        }
        if let Some(h) = self.hand.as_mut() {
            return run_hand(h, &hand_file, line, verb, &a);
        }
        let file = self.file();
        let Session { data, pal, cur, speed, .. } = self;
        let (c, m) = cur.as_mut().ok_or("choose first: creature <name>, or hand")?;
        let at = |c: &Creature, speed: &str| match speed {
            "walk" => c.walk_speed,
            "run" => c.run_speed,
            s => s.parse().unwrap_or(c.walk_speed),
        };
        match (verb, a.as_slice()) {
            ("creature" | "show", _) => {
                let values: Vec<String> = RANGES.iter().map(|r| format!("\"{}\":{}", r.0, c.get(r.0))).collect();
                let ranges: Vec<String> = RANGES.iter().map(|r| format!("\"{}\":[{},{}]", r.0, r.1, r.2)).collect();
                Ok(format!("{{\"creature\":{},\"model\":{},\"plan\":\"{}\",\"values\":{{{}}},\"ranges\":{{{}}},\"gait\":{}}}", js(&c.name), js(&c.model), c.plan.name(), values.join(","), ranges.join(","), gait_json(m, c, at(c, speed))))
            }
            ("set", [key, v]) => {
                let v = num(v)?;
                in_range(key, v)?;
                c.set(key, v);
                Ok(gait_json(m, c, at(c, speed)))
            }
            ("gait", []) => Ok(gait_json(m, c, at(c, speed))),
            ("gait", [s]) => {
                if !matches!(*s, "walk" | "run") {
                    num(s)?;
                }
                *speed = s.to_string();
                Ok(gait_json(m, c, at(c, speed)))
            }
            ("sweep", [key, from, to, n]) => {
                let (from, to, n) = (num(from)?, num(to)?, n.parse::<usize>().ok().filter(|n| (2..=200).contains(n)).ok_or("sweep: 2 to 200 values")?);
                in_range(key, from)?;
                in_range(key, to)?;
                let mut rows = vec![];
                let mut best: Option<(f32, f32)> = None;
                for i in 0..n {
                    let mut t = c.clone();
                    let v = from + (to - from) * i as f32 / (n - 1) as f32;
                    t.set(key, v);
                    let speed = at(&t, speed);
                    rows.push(match gait(m, &t, speed) {
                        Some(g) => {
                            if best.is_none_or(|b| g.slide_cm < b.1) {
                                best = Some((v, g.slide_cm));
                            }
                            format!("{{\"v\":{v},\"sweep_m\":{:.3},\"drift_cm\":{:.2},\"slide_cm\":{:.2},\"sink_cm\":{:.2},\"hover_cm\":{:.2}}}", g.sweep_m, g.drift_cm, g.slide_cm, g.sink_cm, g.hover_cm)
                        }
                        None => format!("{{\"v\":{v},\"gait\":null}}"),
                    });
                }
                let least = best.map_or("null".into(), |b| format!("{{\"v\":{},\"slide_cm\":{:.2}}}", b.0, b.1));
                Ok(format!("{{\"sweep\":{},\"creature\":{},\"step_m\":{},\"least_slide\":{least},\"rows\":[{}]}}", js(key), js(&c.name), c.step, rows.join(",")))
            }
            ("film", [path]) => film(m, c, at(c, speed), path),
            ("save", []) => {
                let check = |t: &str| crate::art::creatures_refused(data, pal, t).map_or(Ok(()), Err);
                crate::rows::save(&file, "creature", &c.name, &creatures::write_row(c), check)?;
                Ok(format!("{{\"saved\":{},\"file\":{}}}", js(&c.name), js(&file)))
            }
            _ => Err(format!("no command '{line}' (creature <name>, hand, show, set <key> <value>, gait [walk|run|<m/s>], sweep <key> <from> <to> <n>, film <png>, save, quit)")),
        }
    }
}

/// `tune=`: the session on the working folder's `data/`.
pub fn main(script: &str) -> Result<String, String> {
    let mut s = Session::new(Path::new("."))?;
    let flat = |e: String| e.replace(['\r', '\n'], " ");
    if script == "-" {
        for line in std::io::stdin().lines() {
            let line = line.map_err(|e| e.to_string())?;
            match line.trim() {
                "" => {}
                "quit" => break,
                l => println!("{}", s.run(l).unwrap_or_else(|e| format!("{{\"error\":{}}}", js(&flat(e))))),
            }
        }
        return Ok(String::new());
    }
    for step in script.split(';').filter(|s| !s.trim().is_empty()) {
        println!("{}", s.run(step).map_err(|e| flat(format!("'{}': {e}", step.trim())))?);
    }
    Ok(String::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(name: &str) -> Session {
        let mut s = Session::new(Path::new("../..")).unwrap();
        s.run(&format!("creature {name}")).unwrap();
        s
    }
    fn field(line: &str, key: &str) -> f32 {
        let at = line.find(&format!("\"{key}\":")).unwrap_or_else(|| panic!("no {key} in {line}")) + key.len() + 3;
        line[at..].split([',', '}']).next().unwrap().parse().unwrap()
    }

    /// Legs that hang still under a body that moves are dragged a whole step; legs that swing are dragged less.
    #[test]
    fn a_leg_that_does_not_swing_slides_a_whole_step() {
        let mut s = session("boar");
        let (c, m) = s.cur.clone().unwrap();
        // (the ground passes under a still foot for the half cycle it is counted on: one step)
        let still = gait(&m, &Creature { swing: 0.0, bob: 0.0, ..c.clone() }, c.walk_speed).unwrap();
        assert!((still.slide_cm - c.step * 100.0).abs() < 0.05 && still.sweep_m == 0.0, "still legs slide the step of {} m: {still:?}", c.step);
        let small = gait(&m, &Creature { swing: 1.0, bob: 0.0, ..c.clone() }, c.walk_speed).unwrap();
        assert!((small.slide_cm - c.step * 100.0).abs() < c.step * 15.0, "a 1 degree swing slides nearly the step of {} m: {small:?}", c.step);
        let line = s.run("set swing 30").unwrap();
        assert!(field(&line, "slide_cm") < small.slide_cm * 0.8 && field(&line, "sweep_m") > small.sweep_m, "{line}");
        assert!((field(&line, "cycle_s") - 2.0 * c.step / c.walk_speed).abs() < 1e-3, "{line}");
    }

    /// A sweep tries the values, names the one whose feet slide least, and leaves the row as it was.
    #[test]
    fn a_sweep_finds_the_swing_that_plants_the_feet_and_keeps_the_row() {
        let mut s = session("boar");
        let before = s.cur.clone().unwrap().0;
        let line = s.run("sweep swing 0 90 46").unwrap();
        let best = field(&line[line.find("least_slide").unwrap()..], "v");
        assert!(best > 0.0 && best < 90.0, "the least slide is at neither end: {line}");
        assert_eq!(line.matches("\"v\":").count(), 47, "46 rows and the least: {line}");
        assert_eq!(s.cur.as_ref().unwrap().0, before);
        // the feet at that swing sweep about a step
        let g = gait(&s.cur.as_ref().unwrap().1, &Creature { swing: best, ..before.clone() }, before.walk_speed).unwrap();
        assert!((g.sweep_m - before.step).abs() < before.step * 0.35, "{g:?} for a step of {}", before.step);
    }

    /// The shipped swing by hand: it peaks where its curve does and reaches 45 % of the way to the aim.
    #[test]
    fn the_hand_swing_is_measured_and_a_set_moves_it() {
        let mut s = Session::new(Path::new("../..")).unwrap();
        let line = s.run("hand").unwrap();
        let h = s.hand.clone().unwrap();
        let peak = h.seconds * (1.0 - 0.5f64.powf(1.0 / h.snap));
        let way = (DVec3::new(0.0, 0.0, h.arm_hand[0] * h.aim) - DVec3::new(h.arm_hand[1], h.arm_hand[2], h.arm_hand[0])).length();
        let sw = swing(&h);
        assert!((sw.peak_s - peak).abs() < 0.002 && (sw.travel_cm - way * h.reach * 100.0).abs() < 0.1, "{sw:?}: peak {peak}, way {way}");
        assert!(sw.strike_m_s > sw.travel_cm / 100.0 / sw.peak_s, "the fastest is over the mean: {sw:?}");
        assert!(line.contains("\"peak_s\":") && line.contains("\"snap\":2"), "{line}");
        // a sharper curve strikes sooner and faster; a longer reach goes further; the file's ranges hold
        let sharp = s.run("set snap 4").unwrap();
        assert!(field(&sharp, "peak_s") < sw.peak_s as f32 - 0.01 && field(&sharp, "strike_m_s") > sw.strike_m_s as f32, "{sharp}");
        let far = s.run("set reach 0.9").unwrap();
        assert!((field(&far, "travel_cm") - 2.0 * sw.travel_cm as f32).abs() < 0.2, "{far}");
        assert!(s.run("set snap 9").unwrap_err().contains("snap = 9 is outside 1..6"));
        assert_eq!(s.run("sweep seconds 0.1 0.5 5").unwrap().matches("\"v\":").count(), 5);
        assert_eq!(s.hand.as_ref().unwrap().seconds, h.seconds, "a sweep keeps the numbers");
        // saved text: only the numbers' lines change
        let old = engine::hand::HAND_TOML;
        assert_eq!(hand_text(old, &h), old);
        let new = hand_text(old, s.hand.as_ref().unwrap());
        assert_eq!(Hand::parse(&new).unwrap().get("snap"), 4.0);
        assert_eq!(new.lines().filter(|l| l.starts_with('#')).count(), old.lines().filter(|l| l.starts_with('#')).count());
        // back to a creature: the hand is put down
        assert!(s.run("creature boar").unwrap().contains("\"gait\""));
    }

    #[test]
    fn what_is_out_of_range_or_unknown_is_refused_by_name() {
        let mut s = Session::new(Path::new("../..")).unwrap();
        assert!(s.run("set swing 20").unwrap_err().contains("creature <name>, or hand"));
        assert!(s.run("creature unicorn").unwrap_err().contains("boar"));
        s.run("creature boar").unwrap();
        assert!(s.run("set swing 200").unwrap_err().contains("swing = 200 is outside 0..90"));
        assert!(s.run("set wobble 2").unwrap_err().contains("no key 'wobble'"));
        assert!(s.run("gait fast").unwrap_err().contains("no number"));
        assert!(s.run("dance").unwrap_err().contains("no command 'dance'"));
        // a faster gait is its own cycle
        let (walk, run) = (s.run("gait walk").unwrap(), s.run("gait run").unwrap());
        assert!(field(&run, "cycle_s") < field(&walk, "cycle_s"), "{walk} {run}");
    }
}
