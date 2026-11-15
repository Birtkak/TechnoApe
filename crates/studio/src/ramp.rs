//! Ramps without a window (docs/design/studio-tools.md "Palette: ramps"): making a ramp from a base colour and
//! a few numbers, putting it in the palette or recolouring one there, and the mood of the whole palette.
use engine::palette::{Colour, Palette};

/// Hue, saturation, value, each 0..1 (hue in turns).
pub fn hsv(c: [u8; 3]) -> [f32; 3] {
    let [r, g, b] = c.map(|v| v as f32 / 255.0);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        ((g - b) / d).rem_euclid(6.0) / 6.0
    } else if max == g {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    [h, if max == 0.0 { 0.0 } else { d / max }, max]
}

pub fn rgb([h, s, v]: [f32; 3]) -> [u8; 3] {
    let (s, v) = (s.clamp(0.0, 1.0), v.clamp(0.0, 1.0));
    let h = h.rem_euclid(1.0) * 6.0;
    let (i, f) = (h.floor() as i32 % 6, h.fract());
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    let c = match i {
        0 => [v, t, p],
        1 => [q, v, p],
        2 => [p, v, t],
        3 => [p, q, v],
        4 => [t, p, v],
        _ => [v, p, q],
    };
    c.map(|x| (x * 255.0).round() as u8)
}

/// How light a colour looks, 0..1 (Rec. 601 weights on the sRGB bytes; enough to order a ramp).
pub fn luma(c: [u8; 3]) -> f32 {
    (0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32) / 255.0
}

/// The hues a ramp turns towards: blue in the dark, yellow in the light (in turns).
const COOL: f32 = 230.0 / 360.0;
const WARM: f32 = 50.0 / 360.0;

/// Turns hue `h` towards `to` by at most `by` (all in turns), the short way round, never past it.
fn turn(h: f32, to: f32, by: f32) -> f32 {
    let d = (to - h + 0.5).rem_euclid(1.0) - 0.5;
    (h + d.signum() * by.min(d.abs())).rem_euclid(1.0)
}

/// The sliders of "Make a ramp".
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RampSpec {
    pub base: [u8; 3],
    /// 3 to 9 colours.
    pub steps: usize,
    /// Degrees the hue turns towards cool at the dark end and warm at the light end, 0..60.
    pub turn: f32,
    /// How much the saturation falls at both ends, 0..1.
    pub fade: f32,
    /// How far the ends reach from the base towards black and white, 0..1.
    pub reach: f32,
}

impl RampSpec {
    pub fn new(base: [u8; 3]) -> RampSpec {
        RampSpec { base, steps: 5, turn: 20.0, fade: 0.3, reach: 0.7 }
    }
}

/// A ramp from dark to light with the base colour exactly at its middle (the lighter middle of an even ramp),
/// every colour different.
pub fn make(spec: &RampSpec) -> Vec<[u8; 3]> {
    let n = spec.steps.clamp(3, 9);
    let m = n / 2;
    let [bh, bs, bv] = hsv(spec.base);
    let mut out: Vec<[u8; 3]> = vec![];
    for i in 0..n {
        let t = if i < m { (i as f32 - m as f32) / m as f32 } else if i > m { (i - m) as f32 / (n - 1 - m) as f32 } else { 0.0 };
        let c = if i == m {
            spec.base
        } else {
            let a = t.abs();
            let v = if t < 0.0 { bv * (1.0 - a * spec.reach * 0.8) } else { bv + (1.0 - bv) * a * spec.reach };
            let s = bs * (1.0 - spec.fade * a);
            let h = turn(bh, if t < 0.0 { COOL } else { WARM }, spec.turn / 360.0 * a);
            rgb([h, s, v])
        };
        out.push(c);
    }
    distinct(&mut out, &[]);
    out
}

/// Nudges colours (never the one at the middle of a ramp: callers keep a base exactly by putting it first in
/// `taken` order) until none equals another or any of `taken`.
pub fn distinct(cs: &mut [[u8; 3]], taken: &[[u8; 3]]) {
    for i in 0..cs.len() {
        let mut k = 0u8;
        while taken.contains(&cs[i]) || cs[..i].contains(&cs[i]) {
            // one step of blue, away from the edge it would hit, then green
            let c = &mut cs[i];
            k = k.wrapping_add(1);
            let ch = if k % 2 == 0 { 1 } else { 2 };
            c[ch] = if c[ch] < 128 { c[ch] + 1 } else { c[ch] - 1 };
        }
    }
}

/// The names of a new ramp's colours, dark to light: "x dark", "x", "x light" for three, else "x 1" to "x n".
pub fn names(ramp: &str, n: usize) -> Vec<String> {
    match n {
        3 => vec![format!("{ramp} dark"), ramp.to_string(), format!("{ramp} light")],
        _ => (1..=n).map(|i| format!("{ramp} {i}")).collect(),
    }
}

/// Adds a ramp to the end of the palette; its colours are first made different from every colour there.
pub fn add(pal: &mut Palette, ramp: &str, colours: &[[u8; 3]]) -> Result<(), String> {
    let ramp = ramp.trim();
    if ramp.is_empty() || ramp != ramp.to_lowercase() {
        return Err(format!("Ramp name '{ramp}' must be lower case and not empty"));
    }
    if pal.colours.iter().any(|c| c.ramp.as_deref() == Some(ramp)) {
        return Err(format!("A ramp named '{ramp}' already exists. Choose another name."));
    }
    let taken: Vec<[u8; 3]> = pal.colours.iter().map(|c| c.rgb).collect();
    let mut cs = colours.to_vec();
    distinct(&mut cs, &taken);
    let mut next = pal.clone();
    for (name, rgb) in names(ramp, cs.len()).into_iter().zip(cs) {
        next.check(&name, rgb, None)?;
        next.colours.push(Colour { name, rgb, ramp: Some(ramp.into()) });
    }
    *pal = next;
    Ok(())
}

/// Recolours a ramp in place with as many new colours as it has; the (old, new) pairs to repaint textures by.
pub fn set(pal: &mut Palette, ramp: &str, colours: &[[u8; 3]]) -> Result<Vec<([u8; 3], [u8; 3])>, String> {
    let run = pal.ramp(ramp);
    if run.len() != colours.len() {
        return Err(format!("Ramp '{ramp}' has {} colours, not {}. Make the new ramp the same length.", run.len(), colours.len()));
    }
    let taken: Vec<[u8; 3]> = pal.colours.iter().enumerate().filter(|(i, _)| !run.contains(i)).map(|(_, c)| c.rgb).collect();
    let mut cs = colours.to_vec();
    distinct(&mut cs, &taken);
    let pairs: Vec<([u8; 3], [u8; 3])> = run.iter().zip(&cs).map(|(&i, &c)| (pal.colours[i].rgb, c)).filter(|(a, b)| a != b).collect();
    for (&i, c) in run.iter().zip(cs) {
        pal.colours[i].rgb = c;
    }
    Ok(pairs)
}

/// "Add shades" (studio.md step 8): the palette with four shades of colour `i` around it, two darker leaning
/// cooler and two lighter leaning warmer (a ramp of five, `make`'s), named after it; they join its ramp, or a new
/// ramp named after it, ordered dark to light. Also the names added.
pub fn shades(pal: &Palette, i: usize) -> Result<(Palette, Vec<String>), String> {
    let c = pal.colours.get(i).ok_or(format!("there is no colour {}", i + 1))?.clone();
    let five = make(&RampSpec::new(c.rgb));
    let mut new = [five[0], five[1], five[3], five[4]];
    distinct(&mut new, &pal.colours.iter().map(|c| c.rgb).collect::<Vec<_>>());
    let mut added: Vec<String> = vec![];
    for w in ["darker", "dark", "light", "lighter"] {
        let n = free(&format!("{} {w}", c.name), |n| pal.colours.iter().all(|c| c.name != n) && !added.iter().any(|a| a == n));
        added.push(n);
    }
    let ramp = c.ramp.clone().unwrap_or_else(|| free(&c.name, |n| pal.ramp(n).is_empty()));
    let mut next = pal.clone();
    next.colours[i].ramp = Some(ramp.clone());
    let run = next.ramp(&ramp);
    let at = run[0];
    let mut members: Vec<Colour> = run.iter().rev().map(|&k| next.colours.remove(k)).collect();
    members.reverse();
    // each new shade goes before the first colour lighter than it: the ramp's own order is kept
    for (name, rgb) in added.iter().zip(new) {
        next.check(name, rgb, None)?;
        let k = members.iter().position(|m| luma(m.rgb) > luma(rgb)).unwrap_or(members.len());
        members.insert(k, Colour { name: name.clone(), rgb, ramp: Some(ramp.clone()) });
    }
    next.colours.splice(at..at, members);
    Ok((next, added))
}

/// `base`, else "base 2", "base 3", ...: the first that `ok` takes.
fn free(base: &str, ok: impl Fn(&str) -> bool) -> String {
    (1..).map(|k| if k == 1 { base.to_string() } else { format!("{base} {k}") }).find(|n| ok(n)).unwrap()
}

/// A name for a new colour: a plain word for its hue and lightness ("dark green", "pale blue"), free in `pal`.
pub fn suggest(pal: &Palette, c: [u8; 3]) -> String {
    let [h, s, v] = hsv(c);
    let word = if s < 0.15 {
        match v {
            v if v < 0.15 => "black".to_string(),
            v if v > 0.9 => "white".to_string(),
            v if v < 0.35 => "dark grey".to_string(),
            v if v > 0.75 => "light grey".to_string(),
            _ => "grey".to_string(),
        }
    } else {
        let deg = h * 360.0;
        let hue = [(15.0, "red"), (40.0, "orange"), (65.0, "yellow"), (160.0, "green"), (195.0, "teal"), (255.0, "blue"), (290.0, "purple"), (340.0, "pink"), (361.0, "red")].into_iter().find(|(d, _)| deg < *d).map_or("red", |(_, n)| n);
        match () {
            _ if v < 0.4 => format!("dark {hue}"),
            _ if s < 0.35 && v > 0.75 => format!("pale {hue}"),
            _ => hue.to_string(),
        }
    };
    free(&word, |n| pal.colours.iter().all(|c| c.name != n))
}

/// The mood sliders over the whole palette, each -1..1: warmer (+) or cooler, more saturated, lighter. The
/// colours in the palette's order, every one still different.
pub fn mood(pal: &Palette, warm: f32, sat: f32, light: f32) -> Vec<[u8; 3]> {
    let mut out: Vec<[u8; 3]> = pal
        .colours
        .iter()
        .map(|c| {
            if warm == 0.0 && sat == 0.0 && light == 0.0 {
                return c.rgb;
            }
            let [h, s, v] = hsv(c.rgb);
            // a grey has no hue to turn
            let h = if s < 0.02 { h } else { turn(h, if warm > 0.0 { WARM } else { COOL }, warm.abs() * 25.0 / 360.0) };
            let s = if sat > 0.0 { s + (1.0 - s) * sat * 0.6 } else { s * (1.0 + sat) };
            let v = if light > 0.0 { v + (1.0 - v) * light * 0.5 } else { v * (1.0 + light * 0.5) };
            rgb([h, s, v])
        })
        .collect();
    distinct(&mut out, &[]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hue_gap(a: f32, b: f32) -> f32 {
        ((a - b + 0.5).rem_euclid(1.0) - 0.5).abs()
    }

    #[test]
    fn hsv_round_trips() {
        for c in [[0, 0, 0], [255, 255, 255], [76, 148, 51], [217, 133, 82], [87, 115, 140], [255, 0, 0]] {
            assert_eq!(rgb(hsv(c)), c, "{c:?}");
        }
    }

    #[test]
    fn a_ramp_runs_dark_to_light_round_its_base() {
        let base = [0x4c, 0x94, 0x33];
        let spec = RampSpec { base, steps: 5, turn: 30.0, fade: 0.4, reach: 0.8 };
        let r = make(&spec);
        assert_eq!(r.len(), 5);
        assert_eq!(r[2], base, "the base sits in the middle");
        assert!(r.windows(2).all(|w| luma(w[0]) < luma(w[1])), "dark to light: {r:?}");
        let bh = hsv(base)[0];
        assert!(hue_gap(hsv(r[0])[0], COOL) < hue_gap(bh, COOL), "the dark end turns cool");
        assert!(hue_gap(hsv(r[4])[0], WARM) < hue_gap(bh, WARM), "the light end turns warm");
        assert!(hsv(r[0])[1] < hsv(base)[1] && hsv(r[4])[1] < hsv(base)[1], "saturation falls at both ends");
        let flat = make(&RampSpec { turn: 0.0, fade: 0.0, ..spec });
        assert!(flat.iter().all(|c| hue_gap(hsv(*c)[0], bh) < 0.01), "no turn keeps the hue: {flat:?}");
        assert_eq!(make(&RampSpec { steps: 4, ..spec })[2], base, "an even ramp has it at the lighter middle");
        assert_eq!(make(&RampSpec { steps: 20, ..spec }).len(), 9, "at most 9");
        let mut all = make(&RampSpec { base: [255, 255, 255], steps: 9, ..spec });
        let n = all.len();
        all.dedup();
        assert_eq!(all.len(), n, "every colour differs even on white");
    }

    #[test]
    fn a_new_ramp_is_named_and_kept_apart_from_the_palette() {
        let mut pal = Palette { colours: vec![Colour::new("moss", [10, 20, 30])] };
        add(&mut pal, "sand", &[[10, 20, 30], [100, 90, 60], [200, 190, 150]]).unwrap();
        let names: Vec<&str> = pal.colours.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["moss", "sand dark", "sand", "sand light"]);
        assert_ne!(pal.colours[1].rgb, [10, 20, 30], "nudged off a colour already there");
        assert_eq!(pal.ramp("sand"), vec![1, 2, 3]);
        assert!(add(&mut pal, "sand", &[[1, 1, 1]; 3]).is_err(), "a ramp name once");
        assert!(add(&mut pal, "Rock", &[[1, 1, 1]; 3]).is_err(), "lower case");
        add(&mut pal, "rock", &[[1, 1, 1], [2, 2, 2], [3, 3, 3], [4, 4, 4]]).unwrap();
        assert_eq!(pal.colours[4].name, "rock 1");
        assert!(Palette::parse(&pal.write()).is_ok());
    }

    #[test]
    fn recolouring_a_ramp_gives_the_pairs_to_repaint() {
        let mut pal = Palette::default();
        add(&mut pal, "sand", &[[10, 10, 10], [20, 20, 20], [30, 30, 30]]).unwrap();
        let pairs = set(&mut pal, "sand", &[[10, 10, 10], [25, 25, 25], [35, 35, 35]]).unwrap();
        assert_eq!(pairs, vec![([20, 20, 20], [25, 25, 25]), ([30, 30, 30], [35, 35, 35])], "unchanged colours are left out");
        assert_eq!(pal.colours[2].rgb, [35, 35, 35]);
        assert!(set(&mut pal, "sand", &[[1, 1, 1]]).is_err(), "as many colours as it has");
    }

    #[test]
    fn the_mood_moves_every_colour_and_keeps_them_apart() {
        let mut pal = Palette::default();
        for (i, c) in [[200, 40, 40], [40, 60, 200], [120, 120, 120], [76, 148, 51]].into_iter().enumerate() {
            pal.colours.push(Colour::new(format!("c{i}"), c));
        }
        let same: Vec<[u8; 3]> = pal.colours.iter().map(|c| c.rgb).collect();
        assert_eq!(mood(&pal, 0.0, 0.0, 0.0), same, "no mood changes nothing");
        let warm = mood(&pal, 1.0, 0.0, 0.0);
        assert!(hue_gap(hsv(warm[1])[0], WARM) < hue_gap(hsv(same[1])[0], WARM), "blue turns warmer");
        assert_eq!(warm[2], same[2], "a grey has no hue to turn");
        let light = mood(&pal, 0.0, 0.0, 0.6);
        assert!(light.iter().zip(&same).all(|(a, b)| luma(*a) > luma(*b)), "lighter, every one");
        let grey = mood(&pal, 0.0, -1.0, 0.0);
        assert!(grey.iter().all(|c| hsv(*c)[1] < 0.02), "no saturation left: {grey:?}");
        let mut d = grey.clone();
        d.sort();
        d.dedup();
        assert_eq!(d.len(), 4, "still four colours");
    }

    #[test]
    fn shades_join_the_colours_ramp_dark_to_light() {
        // a loose colour: a new ramp of five named after it, the colour at its middle
        let mut pal = Palette { colours: vec![Colour::new("rust", [150, 70, 40]), Colour::new("moss", [60, 110, 40])] };
        let (next, added) = shades(&pal, 0).unwrap();
        assert_eq!(added, ["rust darker", "rust dark", "rust light", "rust lighter"]);
        let run = next.ramp("rust");
        assert_eq!(run.len(), 5);
        let cs: Vec<[u8; 3]> = run.iter().map(|&k| next.colours[k].rgb).collect();
        assert_eq!(cs[2], [150, 70, 40], "the colour itself in the middle");
        assert!(cs.windows(2).all(|w| luma(w[0]) < luma(w[1])), "dark to light: {cs:?}");
        assert!(hue_gap(hsv(cs[0])[0], COOL) < hue_gap(hsv(cs[2])[0], COOL), "darker leans cooler");
        assert!(hue_gap(hsv(cs[4])[0], WARM) < hue_gap(hsv(cs[2])[0], WARM), "lighter leans warmer");
        assert_eq!(next.step([150, 70, 40], -1), Some(cs[1]), "the Shade tool and auto colours walk the new ramp");
        assert_eq!(next.colours.last().unwrap().name, "moss", "the other colours stay where they were");
        assert!(Palette::parse(&next.write()).is_ok());
        // a colour on a ramp: the shades join that ramp, names kept free
        pal = next;
        let (again, added) = shades(&pal, run[2]).unwrap();
        assert_eq!(again.ramp("rust").len(), 9);
        assert!(added.iter().all(|n| pal.colours.iter().all(|c| &c.name != n)), "fresh names: {added:?}");
        let run: Vec<[u8; 3]> = again.ramp("rust").iter().map(|&k| again.colours[k].rgb).collect();
        assert!(run.windows(2).all(|w| luma(w[0]) <= luma(w[1])), "still dark to light: {run:?}");
        assert!(Palette::parse(&again.write()).is_ok());
        assert!(shades(&pal, 99).is_err());
    }

    #[test]
    fn a_new_colour_gets_a_plain_free_name() {
        let pal = Palette { colours: vec![Colour::new("green", [40, 160, 40])] };
        assert_eq!(suggest(&pal, [40, 160, 40]), "green 2", "free in the palette");
        assert_eq!(suggest(&pal, [30, 60, 160]), "blue");
        assert_eq!(suggest(&pal, [20, 30, 90]), "dark blue");
        assert_eq!(suggest(&pal, [200, 210, 250]), "pale blue");
        assert_eq!(suggest(&pal, [128, 128, 128]), "grey");
        assert_eq!(suggest(&pal, [10, 10, 10]), "black");
        assert_eq!(suggest(&pal, [200, 60, 40]), "red");
    }
}
