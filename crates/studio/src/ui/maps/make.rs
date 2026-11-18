//! What the Maps page makes from a map (docs/design/studio-maps.md, step 3): the starters New map begins
//! from, variations under other seeds, and a block's texture.
use crate::art::Img;
use crate::map::{self, Data, Graph, Map};
use crate::wall::Rng;
use engine::palette::Palette;

/// New map's starters: name, what it is for, the file. Kept in code: no file of theirs to keep in step.
pub const STARTERS: [(&str, &str, &str); 7] = [
    ("clouds", "soft grey lumps: skies, dust, a mask for blending", CLOUDS),
    ("stone cells", "cracked stone in the stone ramp: a block's face", STONE),
    ("wood grain", "rings bent by noise in oak: planks and logs", WOOD),
    ("wind gusts", "a direction drifting across: wind for grass and smoke", GUSTS),
    ("smoke swirl", "a curling direction: smoke, steam, sparks", SWIRL),
    ("ripples", "rings warped as they spread, looping: water", RIPPLES),
    ("flicker", "a number that jumps about, fast loop: fire, lamps", FLICKER),
];

/// A starter's map.
pub fn starter(name: &str, pal: &Palette) -> Result<Map, String> {
    let (_, _, text) = STARTERS.iter().find(|s| s.0 == name).ok_or(format!("no starter '{name}'"))?;
    map::parse(&format!("starter {name}"), text, pal)
}

/// The map with every seed moved on by `shift` (wrapping in the seed's range); its wiring and the rest as they
/// were.
pub fn reseed(m: &Map, shift: u32) -> Map {
    let mut out = m.clone();
    for (i, n) in m.nodes.iter().enumerate() {
        if let Some(k) = n.spec().nums.iter().position(|s| s.name == "seed") {
            out.set_num(i, "seed", ((n.nums[k] as u32 + shift) % 65536) as f32);
        }
    }
    out
}

/// A variation for the wall: the same graph under other seeds.
pub fn variation(m: &Map, r: &mut Rng, _spread: f32) -> Map {
    reseed(m, 1 + r.below(65534) as u32)
}

/// The map's out as a picture `side` px a side (its first frame).
pub fn picture(m: &Map, side: usize) -> (Data, map::Pic) {
    let mut g = Graph::new(m.clone());
    let o = g.out();
    (g.kinds[o], g.pic(o, 0, side).clone())
}

/// A small picture of the map for the files column and the starters' list.
pub fn thumb(m: &Map, side: usize) -> Img {
    let (kind, p) = picture(m, side);
    let px = map::rgba(&p, kind);
    Img { w: side, h: side, px: px.chunks(4).map(|c| [c[0], c[1], c[2]]).collect() }
}

/// A block's texture from the map: `variants` squares `side` a side, each the map under seeds moved on. A
/// colour is snapped to the palette; a number runs along `ramp` (dark to light); a direction is refused.
pub fn texture(m: &Map, side: usize, variants: usize, ramp: &[[u8; 3]], pal: &Palette) -> Result<Img, String> {
    let mut img = Img { w: side * variants, h: side, px: vec![[0; 3]; side * side * variants] };
    for v in 0..variants {
        let (kind, p) = picture(&reseed(m, v as u32 * 7919), side);
        for (i, c) in p.iter().enumerate() {
            let rgb = match kind {
                Data::Dir => return Err("a direction is a push, not a picture: it cannot be a block's texture (Export writes it; Length turns it into a number)".into()),
                Data::Col => crate::tools::snap(pal, *c),
                _ if ramp.is_empty() => return Err("a number needs a palette ramp to run along".into()),
                _ => ramp[((c[0].clamp(0.0, 1.0) * (ramp.len() - 1) as f32).round() as usize).min(ramp.len() - 1)],
            };
            img.px[(i / side) * img.w + v * side + i % side] = rgb;
        }
    }
    Ok(img)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::Pin;

    fn pal() -> Palette {
        Palette::parse(&std::fs::read_to_string("../../data/palette.toml").unwrap()).unwrap()
    }

    #[test]
    fn the_seven_starters_read() {
        let pal = pal();
        for (name, _, _) in STARTERS {
            let m = starter(name, &pal).unwrap_or_else(|e| panic!("{e}"));
            assert!(m.nodes.len() >= 3, "{name}");
        }
        assert_eq!(STARTERS.map(|s| s.0), ["clouds", "stone cells", "wood grain", "wind gusts", "smoke swirl", "ripples", "flicker"]);
    }

    #[test]
    fn a_variation_changes_pixels_not_wiring() {
        let pal = pal();
        let m = starter("stone cells", &pal).unwrap();
        let v = variation(&m, &mut Rng::new(3), 0.5);
        let wires = |m: &Map| m.nodes.iter().map(|n| (n.id.clone(), n.at, n.pins.iter().filter(|p| matches!(p, Pin::Wire(_))).cloned().collect::<Vec<_>>(), n.words.clone())).collect::<Vec<_>>();
        assert_eq!(wires(&v), wires(&m), "the same graph");
        assert_ne!(picture(&v, 32).1, picture(&m, 32).1, "other pixels");
        assert_eq!(reseed(&m, 0), m);
    }

    #[test]
    fn a_texture_is_snapped_or_ramped_and_a_direction_is_refused() {
        let pal = pal();
        let stone: Vec<[u8; 3]> = pal.ramp("stone").iter().map(|&i| pal.colours[i].rgb).collect();
        // a colour: every pixel a palette colour; two variants side by side, not alike
        let img = texture(&starter("stone cells", &pal).unwrap(), 16, 2, &[], &pal).unwrap();
        assert_eq!((img.w, img.h), (32, 16));
        assert!(img.px.iter().all(|c| pal.find(*c).is_some()));
        assert_ne!((0..16).map(|y| img.get(0, 0, y)).collect::<Vec<_>>(), (0..16).map(|y| img.get(1, 0, y)).collect::<Vec<_>>());
        // a number: along the ramp
        let img = texture(&starter("clouds", &pal).unwrap(), 16, 1, &stone, &pal).unwrap();
        assert!(img.px.iter().all(|c| stone.contains(c)));
        assert!(texture(&starter("clouds", &pal).unwrap(), 16, 1, &[], &pal).unwrap_err().contains("ramp"));
        let e = texture(&starter("wind gusts", &pal).unwrap(), 16, 1, &stone, &pal).unwrap_err();
        assert!(e.contains("direction"), "{e}");
    }
}

const CLOUDS: &str = r#"size = 128
frames = 1
seconds = 2

[[node]]
id = "noise"
kind = "smooth noise"
at = [0, 0]
seed = 1
scale = 4
octaves = 5
roughness = 0.55
speed = 0

[[node]]
id = "levels"
kind = "levels"
at = [270, 0]
low = 0.3
high = 0.8
contrast = 1
in = "noise"

[[node]]
id = "out"
kind = "out"
at = [540, 0]
in = "levels"
"#;

const STONE: &str = r#"size = 64
frames = 1
seconds = 2

[[node]]
id = "cracks"
kind = "cells"
at = [0, 0]
seed = 5
scale = 5
speed = 0
mode = "edges"

[[node]]
id = "grain"
kind = "value noise"
at = [0, 260]
seed = 2
scale = 16
octaves = 2
roughness = 0.5
speed = 0

[[node]]
id = "mix"
kind = "blend"
at = [270, 0]
amount = 0.6
mode = "multiply"
a = "cracks"
b = "grain"
mask = 1

[[node]]
id = "paint"
kind = "colours"
at = [540, 0]
dither = 0
ramp = "stone"
in = "mix"

[[node]]
id = "out"
kind = "out"
at = [540, 260]
in = "paint"
"#;

const WOOD: &str = r#"size = 64
frames = 1
seconds = 2

[[node]]
id = "rings"
kind = "gradient"
at = [0, 0]
repeat = 6
shape = "across"

[[node]]
id = "wobble"
kind = "smooth noise"
at = [0, 200]
seed = 4
scale = 2
octaves = 3
roughness = 0.5
speed = 0

[[node]]
id = "bend"
kind = "slope"
at = [270, 260]
strength = 1
in = "wobble"

[[node]]
id = "warp"
kind = "warp"
at = [270, 0]
amount = 0.15
in = "rings"
by = "bend"

[[node]]
id = "paint"
kind = "colours"
at = [540, 0]
dither = 0
ramp = "oak wood"
in = "warp"

[[node]]
id = "out"
kind = "out"
at = [540, 200]
in = "paint"
"#;

const GUSTS: &str = r#"size = 128
frames = 32
seconds = 4

[[node]]
id = "gusts"
kind = "smooth noise"
at = [0, 0]
seed = 7
scale = 2
octaves = 3
roughness = 0.5
speed = 1

[[node]]
id = "drift"
kind = "move"
at = [270, 0]
shift_x = 0
shift_y = 0
repeat = 1
mirror = 0
speed_x = 1
speed_y = 0
in = "gusts"

[[node]]
id = "push"
kind = "slope"
at = [0, 280]
strength = 3
in = "drift"

[[node]]
id = "out"
kind = "out"
at = [270, 280]
in = "push"
"#;

const SWIRL: &str = r#"size = 128
frames = 32
seconds = 4

[[node]]
id = "swirls"
kind = "smooth noise"
at = [0, 0]
seed = 3
scale = 3
octaves = 3
roughness = 0.45
speed = 1

[[node]]
id = "soften"
kind = "blur"
at = [270, 0]
radius = 2
in = "swirls"

[[node]]
id = "curl"
kind = "curl"
at = [0, 280]
strength = 3
in = "soften"

[[node]]
id = "out"
kind = "out"
at = [270, 280]
in = "curl"
"#;

const RIPPLES: &str = r#"size = 128
frames = 32
seconds = 3

[[node]]
id = "rings"
kind = "gradient"
at = [0, 0]
repeat = 6
shape = "round"

[[node]]
id = "flow"
kind = "smooth noise"
at = [0, 200]
seed = 8
scale = 2
octaves = 2
roughness = 0.5
speed = 1

[[node]]
id = "swirl"
kind = "curl"
at = [270, 260]
strength = 1
in = "flow"

[[node]]
id = "warp"
kind = "warp"
at = [270, 0]
amount = 0.08
in = "rings"
by = "swirl"

[[node]]
id = "out"
kind = "out"
at = [540, 0]
in = "warp"
"#;

const FLICKER: &str = r#"size = 16
frames = 16
seconds = 1

[[node]]
id = "flick"
kind = "value noise"
at = [0, 0]
seed = 11
scale = 1
octaves = 2
roughness = 0.5
speed = 4

[[node]]
id = "lv"
kind = "levels"
at = [270, 0]
low = 0.2
high = 0.9
contrast = 1.5
in = "flick"

[[node]]
id = "out"
kind = "out"
at = [540, 0]
in = "lv"
"#;
