//! The Sounds page's logic without a window (docs/design/studio-tools.md "Sounds", ADR 0048): a row of
//! `data/sounds.toml` as its keys plus its bases and chain to edit, the chain's edits, mutate, the starts, the
//! families of the files column and the wave's picture. The game's own `audio` reads and renders what this writes.
use crate::rows::{num, Keys};
use crate::wall::Rng;
use client::audio::chain::{parse_spread, Spread, Step, STEPS};

/// One base the row is made from.
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub base: String,
    pub gain: f32,
    pub delay: f32,
}

/// One card of the chain: a step, its numbers, and whether it is on (an off card is saved `on = false`).
#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    pub kind: usize,
    pub n: [f32; 3],
    pub on: bool,
}

/// A row as the page edits it: its other keys kept as written.
#[derive(Clone, Debug, PartialEq)]
pub struct Sound {
    pub keys: Keys,
    pub from: Vec<Layer>,
    pub chain: Vec<Card>,
    pub spread: Vec<Spread>,
}

pub fn kind(name: &str) -> Option<usize> {
    STEPS.iter().position(|s| s.0 == name)
}

/// What each step does, in plain words, for the add menu and the cards.
pub fn step_help(name: &str) -> &'static str {
    match name {
        "pitch" => "higher or lower, the length kept",
        "speed" => "faster or slower, the pitch kept",
        "stretch" => "slower and lower together, like a tape",
        "lowpass" => "cuts the highs: duller, further, behind a wall",
        "highpass" => "cuts the lows: thinner, smaller",
        "bandpass" => "keeps one band: a phone, a tin, a pipe",
        "reverb" => "a room's ring after it",
        "echo" => "repeats it, fading",
        "drive" => "overdrives it: rougher, angrier",
        "tremolo" => "the loudness wobbles",
        "vibrato" => "the pitch wobbles",
        "fade" => "fades in at the start, out at the end",
        "trim" => "cuts the start or the end off",
        "reverse" => "plays it backwards",
        "gain" => "louder or softer",
        "crush" => "fewer bits: grittier, older",
        _ => "",
    }
}

/// A number's unit for its slider, and whether it reads better as a share in percent.
pub fn unit(step: &str, number: &str) -> &'static str {
    match (step, number) {
        (_, "semitones") => "st",
        ("vibrato", "depth") => "st",
        (_, "times") => "×",
        (_, "hz") | (_, "rate") => "Hz",
        (_, "seconds") | (_, "in") | (_, "out") | (_, "start") | (_, "end") => "s",
        (_, "size") | (_, "damping") | (_, "mix") | (_, "feedback") | (_, "depth") => "%",
        (_, "bits") => "bits",
        _ => "",
    }
}

/// How far a number's spread slider goes: what reads as "a little different" for that number.
pub fn spread_max(step: usize, number: usize) -> f32 {
    let d = &STEPS[step].1[number];
    match d.name {
        "semitones" => 6.0,
        "hz" => 2000.0,
        _ => (d.max - d.min) * 0.25,
    }
}

fn f(v: &toml::Value) -> Option<f32> {
    v.as_float().map(|f| f as f32).or(v.as_integer().map(|i| i as f32))
}

impl Sound {
    /// A row's text (with its `[[sound]]` line).
    pub fn parse(row: &str) -> Result<Sound, String> {
        let keys = Keys::parse(row);
        let t = keys.table()?;
        let from = t.get("from").and_then(|v| v.as_array()).into_iter().flatten().filter_map(|b| b.as_table()).map(|b| Layer { base: b.get("base").and_then(|v| v.as_str()).unwrap_or_default().to_string(), gain: b.get("gain").and_then(f).unwrap_or(1.0), delay: b.get("delay").and_then(f).unwrap_or(0.0) }).collect();
        let chain = t.get("chain").and_then(|v| v.as_array()).into_iter().flatten().filter_map(|s| s.as_table()).map(|t| Step::parse(t).map(|s| Card { kind: s.kind, n: s.n, on: Step::is_on(t) })).collect::<Result<_, _>>()?;
        let spread = parse_spread(t.get("spread").and_then(|v| v.as_table()).unwrap_or(&toml::Table::new()))?;
        Ok(Sound { keys, from, chain, spread })
    }
    pub fn name(&self) -> String {
        self.keys.get("name").unwrap_or("").trim_matches('"').to_string()
    }
    pub fn is_loop(&self) -> bool {
        self.keys.get("loop").and_then(|v| v.parse::<f32>().ok()).is_some_and(|v| v > 0.0)
    }
    pub fn variants(&self) -> u32 {
        self.keys.get("variants").and_then(|v| v.parse().ok()).unwrap_or(1)
    }

    /// The row's text with the cards `keep` keeps on; the others are written `on = false`.
    pub fn write_with(&self, keep: impl Fn(usize, &Card) -> bool) -> String {
        let mut k = self.keys.clone();
        let from: Vec<String> = self
            .from
            .iter()
            .map(|b| {
                let mut s = format!("{{ base = \"{}\"", b.base);
                if b.gain != 1.0 {
                    s += &format!(", gain = {}", num(b.gain));
                }
                if b.delay != 0.0 {
                    s += &format!(", delay = {}", num(b.delay));
                }
                s + " }"
            })
            .collect();
        if !self.from.is_empty() {
            k.set("from", Some(format!("[{}]", from.join(", "))));
            k.set("layer", None);
        } else if k.get("layer").is_some() {
            k.set("from", None);
        }
        let cards = &self.chain;
        let steps: Vec<String> = cards
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let (name, nums) = STEPS[c.kind];
                let off = if keep(i, c) { "" } else { "on = false, " };
                if nums.is_empty() {
                    return format!("  {{ {off}{name} = true }},");
                }
                let mut s = format!("  {{ {off}{name} = {}", num(c.n[0]));
                for (i, d) in nums.iter().enumerate().skip(1) {
                    if (c.n[i] - d.default).abs() > 1e-6 {
                        s += &format!(", {} = {}", d.name, num(c.n[i]));
                    }
                }
                s + " },"
            })
            .collect();
        k.set("chain", (!steps.is_empty()).then(|| format!("[\n{}\n]", steps.join("\n"))));
        // a spread only on a step the written chain has
        let mut groups: Vec<(usize, Vec<&Spread>)> = vec![];
        for p in self.spread.iter().filter(|p| p.amount > 0.0 && cards.iter().any(|c| c.kind == p.step)) {
            match groups.iter_mut().find(|g| g.0 == p.step) {
                Some(g) => g.1.push(p),
                None => groups.push((p.step, vec![p])),
            }
        }
        let parts: Vec<String> = groups
            .iter()
            .map(|(step, ps)| {
                let (name, nums) = STEPS[*step];
                if ps.len() == 1 && ps[0].number == 0 {
                    format!("{name} = {}", num(ps[0].amount))
                } else {
                    format!("{name} = {{ {} }}", ps.iter().map(|p| format!("{} = {}", nums[p.number].name, num(p.amount))).collect::<Vec<_>>().join(", "))
                }
            })
            .collect();
        k.set("spread", (!parts.is_empty()).then(|| format!("{{ {} }}", parts.join(", "))));
        k.text()
    }
    /// The row's text as saved: the off cards written off.
    pub fn write(&self) -> String {
        self.write_with(|_, c| c.on)
    }

    // the chain's edits; each says whether it changed anything
    pub fn add(&mut self, kind: usize) {
        let mut n = [0.0; 3];
        for (i, d) in STEPS[kind].1.iter().enumerate() {
            n[i] = d.default;
        }
        self.chain.push(Card { kind, n, on: true });
    }
    pub fn move_card(&mut self, from: usize, to: usize) -> bool {
        if from >= self.chain.len() || from == to {
            return false;
        }
        let c = self.chain.remove(from);
        self.chain.insert(to.min(self.chain.len()), c);
        true
    }
    pub fn toggle(&mut self, i: usize) -> bool {
        self.chain.get_mut(i).map(|c| c.on = !c.on).is_some()
    }
    pub fn remove(&mut self, i: usize) -> bool {
        (i < self.chain.len()).then(|| self.chain.remove(i)).is_some()
    }
    /// A card's number, kept in its range.
    pub fn set(&mut self, i: usize, number: usize, v: f32) -> bool {
        let Some(c) = self.chain.get_mut(i) else { return false };
        let Some(d) = STEPS[c.kind].1.get(number) else { return false };
        c.n[number] = v.clamp(d.min, d.max);
        true
    }
    pub fn spread_of(&self, step: usize, number: usize) -> f32 {
        self.spread.iter().find(|p| p.step == step && p.number == number).map_or(0.0, |p| p.amount)
    }
    /// How far each take strays on one number of every step of a kind; 0 takes it off.
    pub fn set_spread(&mut self, step: usize, number: usize, amount: f32) -> bool {
        let Some(d) = STEPS.get(step).and_then(|s| s.1.get(number)) else { return false };
        let a = amount.clamp(0.0, d.max - d.min);
        self.spread.retain(|p| !(p.step == step && p.number == number));
        if a > 0.0 {
            self.spread.push(Spread { step, number, amount: a });
        }
        true
    }
}

/// Six variations of a chain's numbers: each number of each card moved by up to `spread` (0..1) of a fair
/// share of its range (frequencies by octaves), kept in range.
pub fn mutate(chain: &Vec<Card>, r: &mut Rng, spread: f32) -> Vec<Card> {
    chain
        .iter()
        .map(|c| {
            let mut c = c.clone();
            for (i, d) in STEPS[c.kind].1.iter().enumerate() {
                let x = r.signed() * spread;
                c.n[i] = if d.name == "hz" { c.n[i] * 2f32.powf(x * 1.5) } else { c.n[i] + x * (d.max - d.min) * 0.15 }.clamp(d.min, d.max);
            }
            c
        })
        .collect()
}

/// Ready chains by name: what each does in plain words, and its steps as a row writes them.
pub const STARTS: [(&str, &str, &str); 10] = [
    ("bigger", "a larger thing: lower, slower, a little room", "[{ pitch = -5 }, { lowpass = 6000 }, { reverb = 0.3, mix = 0.15 }]"),
    ("smaller", "a smaller thing: higher, thinner, quicker", "[{ pitch = 5 }, { highpass = 250 }]"),
    ("further away", "heard from a distance: the highs gone, a room around it", "[{ lowpass = 1800 }, { gain = 0.6 }, { reverb = 0.5, damping = 0.7, mix = 0.4 }]"),
    ("in a cave", "a big stone room's long ring and slap", "[{ echo = 0.12, feedback = 0.35, mix = 0.25 }, { reverb = 0.85, damping = 0.3, mix = 0.45 }]"),
    ("underwater", "muffled and wobbling", "[{ lowpass = 600, q = 1.2 }, { vibrato = 3, depth = 0.4 }, { reverb = 0.4, damping = 0.8, mix = 0.3 }]"),
    ("metallic", "a ring of metal in it", "[{ bandpass = 2500, q = 6 }, { echo = 0.015, feedback = 0.6, mix = 0.4 }, { reverb = 0.25, mix = 0.15 }]"),
    ("soft", "gentler: no sharp start, the highs eased", "[{ lowpass = 3000 }, { fade = 0.02 }, { gain = 0.7 }]"),
    ("heavy", "weightier: lower, pushed, with a thud", "[{ pitch = -7 }, { drive = 2.5 }, { lowpass = 4000 }]"),
    ("old machine", "worn and rattling: gritty, wobbling, a hum of a room", "[{ crush = 6 }, { tremolo = 9, depth = 0.35 }, { bandpass = 1200, q = 0.8 }, { reverb = 0.3, mix = 0.2 }]"),
    ("tiny", "very small and quick", "[{ pitch = 12 }, { speed = 1.5 }, { highpass = 600 }]"),
];

/// A start's steps as cards.
pub fn start_cards(steps: &str) -> Result<Vec<Card>, String> {
    let t: toml::Table = toml::from_str(&format!("s = {steps}")).map_err(|e| e.to_string())?;
    t["s"].as_array().ok_or("not a list")?.iter().map(|s| s.as_table().ok_or("not a table".to_string()).and_then(Step::parse).map(|s| Card { kind: s.kind, n: s.n, on: true })).collect()
}

/// A row's family in the files column: (the family, the row's part in it, the family's rank). A material's
/// dig, place and step sit together; the bases; the rest by their first word.
pub fn family(name: &str) -> (String, String, u8) {
    match name.split_once('.') {
        Some(("base", rest)) => ("bases".into(), rest.into(), 1),
        Some((kind @ ("dig" | "place" | "step"), material)) => (material.into(), kind.into(), 0),
        Some((first, rest)) => (first.into(), rest.into(), 2),
        None => (String::new(), name.into(), 3),
    }
}

/// The rows in the files column's order: materials first (each dig, place, step), then the bases, then the
/// rest; with the label each shows.
pub fn ordered(names: &[String]) -> Vec<(String, String)> {
    let part = |p: &str| ["dig", "place", "step"].iter().position(|k| *k == p).unwrap_or(3);
    let mut v: Vec<(u8, String, usize, String, String)> = names
        .iter()
        .map(|n| {
            let (fam, p, rank) = family(n);
            let label = if fam.is_empty() { p.clone() } else { format!("{fam}: {p}") };
            (rank, fam, part(&p), n.clone(), label)
        })
        .collect();
    v.sort_by(|a, b| (a.0, &a.1, a.2, &a.3).cmp(&(b.0, &b.1, b.2, &b.3)));
    v.into_iter().map(|x| (x.3, x.4)).collect()
}

/// The wave's picture: per column of `n` its lowest and highest sample and its loudness (RMS).
pub fn peaks(x: &[f32], n: usize) -> Vec<[f32; 3]> {
    if x.is_empty() || n == 0 {
        return vec![];
    }
    (0..n)
        .map(|i| {
            let s = &x[i * x.len() / n..((i + 1) * x.len() / n).max(i * x.len() / n + 1).min(x.len())];
            let (lo, hi) = s.iter().fold((0.0f32, 0.0f32), |(a, b), v| (a.min(*v), b.max(*v)));
            [lo, hi, (s.iter().map(|v| v * v).sum::<f32>() / s.len() as f32).sqrt()]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLANK: &str = "[[sound]]\nname = \"furnace.clank\"\nbus = \"world\"\nvariants = 4\nfrom = [{ base = \"base.ring\", gain = 0.7 }, { base = \"base.thud\" }]\nchain = [\n  { pitch = -5 },\n  { lowpass = 3500 },\n  { reverb = 0.35, mix = 0.25 },\n]\nspread = { pitch = 1.5, reverb = { mix = 0.1 } }\n";

    fn k(n: &str) -> usize {
        kind(n).unwrap()
    }

    #[test]
    fn a_row_reads_and_writes_back_and_the_game_takes_it() {
        let s = Sound::parse(CLANK).unwrap();
        assert_eq!(s.chain.len(), 3);
        assert_eq!(s.from[1], Layer { base: "base.thud".into(), gain: 1.0, delay: 0.0 });
        assert_eq!(s.spread_of(k("reverb"), 2), 0.1);
        assert_eq!(s.write(), CLANK, "an unchanged row writes back as it was");
        assert_eq!(Sound::parse(&s.write()).unwrap(), s);
        let rows = client::audio::parse(&s.write()).unwrap();
        assert_eq!(rows[0].chain.len(), 3);
    }

    #[test]
    fn chain_edits_add_move_switch_off_remove_set_number_and_spread() {
        let mut s = Sound::parse(CLANK).unwrap();
        s.add(k("echo"));
        assert_eq!((s.chain[3].kind, s.chain[3].n), (k("echo"), [0.25, 0.3, 0.5]), "a new card has the defaults");
        assert!(s.move_card(3, 0));
        assert_eq!(s.chain.iter().map(|c| STEPS[c.kind].0).collect::<Vec<_>>(), ["echo", "pitch", "lowpass", "reverb"]);
        assert!(!s.move_card(9, 0));
        assert!(s.toggle(2));
        assert!(s.write().contains("{ on = false, lowpass = 3500 }"), "an off card is written off: {}", s.write());
        assert!(!s.write_with(|_, _| true).contains("on = false"));
        let back = Sound::parse(&s.write()).unwrap();
        assert_eq!((&back.chain, &back.spread, back.write()), (&s.chain, &s.spread, s.write()), "a round trip keeps the off card");
        assert_eq!(client::audio::parse(&s.write()).unwrap()[0].chain.len(), 3, "the game leaves it out");
        assert!(s.set(1, 0, 99.0));
        assert_eq!(s.chain[1].n[0], 24.0, "kept in range");
        assert!(!s.set(1, 2, 1.0), "pitch has one number");
        assert!(s.set_spread(k("echo"), 1, 0.2));
        assert!(s.write().contains("echo = { feedback = 0.2 }"));
        assert!(s.set_spread(k("pitch"), 0, 0.0));
        assert!(!s.write().contains("pitch = 1.5"));
        assert!(s.remove(3));
        assert!(!s.write().contains("reverb"), "a spread on a removed step goes with it: {}", s.write());
        assert!(!s.remove(7));
        client::audio::parse(&s.write()).unwrap();
    }

    #[test]
    fn mutate_is_the_same_for_a_seed_and_stays_in_range() {
        let s = Sound::parse(CLANK).unwrap();
        let a = mutate(&s.chain, &mut Rng::new(5), 1.0);
        assert_eq!(a, mutate(&s.chain, &mut Rng::new(5), 1.0));
        assert_ne!(a, s.chain);
        assert_ne!(a, mutate(&s.chain, &mut Rng::new(6), 1.0));
        let mut r = Rng::new(1);
        for _ in 0..500 {
            for c in mutate(&s.chain, &mut r, 1.0) {
                for (i, d) in STEPS[c.kind].1.iter().enumerate() {
                    assert!((d.min..=d.max).contains(&c.n[i]), "{} {}", d.name, c.n[i]);
                }
            }
        }
    }

    #[test]
    fn every_start_parses() {
        for (name, help, steps) in STARTS {
            let c = start_cards(steps).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(!c.is_empty() && !help.is_empty());
            let mut s = Sound::parse(CLANK).unwrap();
            s.chain.extend(c);
            client::audio::parse(&s.write()).unwrap_or_else(|e| panic!("{name}: {e}"));
        }
    }

    #[test]
    fn families_sit_together() {
        let names: Vec<String> = ["wind", "step.stone", "base.thud", "dig.stone", "furnace.clank", "place.stone", "dig.dirt", "furnace.burn"].map(String::from).to_vec();
        let o: Vec<String> = ordered(&names).into_iter().map(|(_, l)| l).collect();
        assert_eq!(o, ["dirt: dig", "stone: dig", "stone: place", "stone: step", "bases: thud", "furnace: burn", "furnace: clank", "wind"]);
        assert_eq!(family("dig.stone").0, family("step.stone").0);
    }

    #[test]
    fn the_wave_picture_has_its_peaks() {
        let x: Vec<f32> = (0..1000).map(|i| if i < 500 { 0.5 } else { -0.25 }).collect();
        let p = peaks(&x, 4);
        assert_eq!(p.len(), 4);
        assert_eq!(p[0], [0.0, 0.5, 0.5]);
        assert_eq!(p[3], [-0.25, 0.0, 0.25]);
        assert_eq!(peaks(&x[..3], 8).len(), 8, "more columns than samples");
        assert!(peaks(&[], 8).is_empty());
    }
}
