//! "Start from pattern": the game's block pattern recipe on the CPU, ported from `tools/review/workbench.html`
//! (`hash`, `PATTERNS`, `darker`, `lighter`, `part`, `texel`), the same recipe as `texel` in
//! `crates/client/src/shader.wgsl`. Whole numbers and `hash` only, so it gives what the game draws.
use crate::art::{Img, S};
use crate::tools::snap;
use engine::blocks::{Def, Pattern, Registry};
use engine::palette::Palette;

/// The shader's `hash`: 0..1 in 1024 steps.
pub fn hash(x: i32, y: i32, z: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(73856093) ^ (y as u32).wrapping_mul(19349663) ^ (z as u32).wrapping_mul(83492791);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    (h & 1023) as f32 / 1023.0
}

/// Which part texel (x, y) of a 16 x 16 face is (0 the colour, 1 `colour2`, 2 `colour3`), the part it blends
/// towards, how far, and a shade. `y` counts down from the face's top edge.
pub fn part(d: &Def, dir: i32, cell: [i32; 3], x: i32, y: i32) -> (usize, usize, f32, f32) {
    let (s, amt) = (d.size.max(1) as i32, d.amount);
    let cz = cell[0] * 7 + cell[1] * 13 + cell[2] * 5 + dir;
    let e = x.min(y).min(15 - x).min(15 - y);
    let plain = (0, 0, 0.0, 1.0);
    match d.pattern {
        Pattern::Pebbles => {
            let h = hash(x + cz * 16, y, 31);
            if h < amt * 0.25 {
                (1, 1, 0.0, 1.0)
            } else if h > 1.0 - amt * 0.1 {
                (2, 2, 0.0, 1.0)
            } else {
                plain
            }
        }
        Pattern::Speckle => {
            let (cx, cy) = (x.div_euclid(s), y.div_euclid(s));
            if hash(cx + cz, cy, 91) >= amt {
                return plain;
            }
            let ox = cx * s + (hash(cx + cz, cy, 17) * s as f32).floor() as i32;
            let oy = cy * s + (hash(cx + cz, cy, 23) * s as f32).floor() as i32;
            let (ax, ay) = ((x - ox).abs() as f32, (y - oy).abs() as f32);
            let (dist, r) = (ax.max(ay) + ax.min(ay) / 2.0, s as f32 * 0.42);
            if dist <= r * 0.4 {
                (2, 2, 0.0, 1.0)
            } else if dist <= r {
                (1, 1, 0.0, 1.0)
            } else {
                plain
            }
        }
        Pattern::Frame => {
            if e < s {
                return (1, 1, 0.0, 1.0);
            }
            let t = ((e - s) as f32 / (7 - s).max(1) as f32).min(1.0);
            (0, 2, (t * amt * 4.0 + 0.5).floor() / 4.0, 1.0)
        }
        Pattern::Weave => {
            let off = (hash(cz, y, 5) * s as f32).floor() as i32;
            if hash((x + off).div_euclid(s) + cz * 3, y, 9) < amt {
                (1, 1, 0.0, 1.0)
            } else {
                plain
            }
        }
        Pattern::Planks => {
            let board = x.div_euclid(s) + cz;
            if x % s == 0 || y == (hash(board, 0, 3) * 16.0).floor() as i32 {
                return (1, 1, 0.0, 1.0);
            }
            (0, 1, if hash(board, y, 4) < amt * 0.3 { 0.35 } else { 0.0 }, 0.92 + 0.16 * hash(board, 1, 4))
        }
        Pattern::Tiles => {
            if x % s == 0 || y % s == 0 {
                (1, 1, 0.0, 1.0)
            } else if hash(x + cz * 16, y, 41) < amt * 0.3 {
                (2, 2, 0.0, 1.0)
            } else {
                plain
            }
        }
        Pattern::Bricks => {
            let h = s.max(2);
            let row = y / h;
            let xo = x + (row % 2) * h;
            if y % h == 0 || xo % (2 * h) == 0 {
                return (1, 1, 0.0, 1.0);
            }
            (0, 2, if y % h == 1 { amt } else { 0.0 }, 0.94 + 0.12 * hash(xo / (2 * h) + cz, row, 6))
        }
        Pattern::Rings => {
            if (e / s) % 2 == 1 {
                (1, 1, 0.0, 1.0)
            } else {
                plain
            }
        }
        Pattern::Plain | Pattern::Grass | Pattern::Stone => plain,
    }
}

/// One texel of a block's face as the game draws it unpainted, 0..1 per channel. `dir` is the shader's: 0 to
/// 3 the sides, 4 the top, 5 the bottom; (x, y) with y down from the face's top edge; `cell` the block's place.
pub fn texel(reg: &Registry, b: &Def, dir: i32, cell: [i32; 3], x: i32, y: i32) -> [f32; 3] {
    let n = S as i32;
    let h = hash(cell[0] * n + x, cell[1] * n + y, cell[2] * n + dir);
    let tone = 0.82 + 0.12 * (h * 3.0).floor();
    let side = reg.get(b.side).unwrap_or(b);
    let m = if b.pattern == Pattern::Grass && dir != 4 {
        // a grass block's side is its side block's look under 2 to 4 rows of grass at the top
        let fringe = 2 + (hash(cell[0] * n + x, cell[1] * n + 7, cell[2] * n + dir) * 3.0).floor() as i32;
        if y >= fringe {
            side
        } else {
            b
        }
    } else if dir != 4 {
        side
    } else {
        b
    };
    let k = if m.contrast == 1.0 { tone } else { 1.0 + m.contrast * (tone - 1.0) };
    let mut c = m.colour.map(|v| v * k);
    if m.pattern == Pattern::Stone && h > 0.94 {
        c = c.map(|v| v * 0.6);
    }
    if m.pattern == Pattern::Grass && h > 0.9 {
        c = c.map(|v| v * 1.15);
    }
    if m.pattern as u8 > Pattern::Stone as u8 {
        let (a, to, t, shade) = part(m, dir, cell, x, y);
        let cs = [m.colour, m.colour2, m.colour3];
        c = [0, 1, 2].map(|i| (cs[a][i] + (cs[to][i] - cs[a][i]) * t) * k * shade);
    }
    c.map(|v| v.clamp(0.0, 1.0))
}

/// The pattern of a block's face drawn into a texture, each texel snapped to the nearest palette colour. `face`
/// as the Studio counts them: 0 every face (drawn as a side), 1 top, 2 side, 3 bottom.
pub fn face(reg: &Registry, b: &Def, face: usize, pal: &Palette) -> Img {
    let dir = [0, 4, 0, 5][face.min(3)];
    let mut img = Img::filled(1, [0; 3]);
    for y in 0..S {
        for x in 0..S {
            crate::art::pencil(&mut img, 0, x, y, snap(pal, texel(reg, b, dir, [0, 0, 0], x as i32, y as i32)), false);
        }
    }
    img
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::palette::Colour;

    #[test]
    fn the_hash_is_the_shaders() {
        // the workbench's JS `hash` (Math.imul, >>> 0) gives these
        assert_eq!(hash(0, 0, 0), 0.0);
        assert_eq!((hash(1, 2, 3) * 1023.0).round(), 554.0);
        assert_eq!((hash(100, -5, 7) * 1023.0).round(), 217.0);
        assert_eq!((hash(37, 15, 4) * 1023.0).round(), 847.0);
    }

    fn reg() -> Registry {
        Registry::parse(
            "[[block]]\nid = 0\nname = \"air\"\nsolid = false\n\
             [[block]]\nid = 1\nname = \"dirt\"\ncolour = [0.47, 0.33, 0.20]\nhardness = 1\n\
             [[block]]\nid = 2\nname = \"grass\"\ncolour = [0.30, 0.58, 0.20]\npattern = \"grass\"\nside = \"dirt\"\nhardness = 1\n\
             [[block]]\nid = 3\nname = \"wall\"\ncolour = [0.6, 0.3, 0.2]\npattern = \"bricks\"\nhardness = 1\n",
        )
        .unwrap()
    }
    fn pal() -> Palette {
        let mut p = Palette::default();
        for (i, c) in [[0x2a, 0x51, 0x1c], [0x4c, 0x94, 0x33], [0x93, 0xec, 0x73], [0x42, 0x2e, 0x1c], [0x78, 0x54, 0x33], [0xc9, 0x9c, 0x73]].into_iter().enumerate() {
            p.colours.push(Colour::new(format!("c{i}"), c));
        }
        p
    }

    #[test]
    fn a_grass_side_is_dirt_under_a_fringe_of_grass() {
        let r = reg();
        let g = r.get(2).unwrap();
        let top = texel(&r, g, 4, [0; 3], 3, 3);
        assert!(top[1] > top[0] && top[1] > top[2], "the top is green: {top:?}");
        let low = texel(&r, g, 0, [0; 3], 3, 15);
        assert!(low[0] > low[2] && low[0] > low[1], "low on the side it is dirt: {low:?}");
        let high = texel(&r, g, 0, [0; 3], 3, 0);
        assert!(high[1] > high[0], "the top row of a side is grass: {high:?}");
        let p = pal();
        let side = face(&r, g, 2, &p);
        assert!(side.px.iter().all(|c| p.find(*c).is_some()), "every texel snapped to the palette");
        assert!((0..S).all(|x| p.find(side.get(0, x, 0)).unwrap() < 3), "the first row is grass colours");
        assert!((0..S).all(|x| p.find(side.get(0, x, 15)).unwrap() >= 3), "the last row is dirt colours");
        assert_eq!(face(&r, g, 2, &p), side, "the same every time");
    }

    #[test]
    fn bricks_draw_mortar_rows_in_the_second_colour() {
        let r = reg();
        let w = r.get(3).unwrap();
        // size 4 by default: rows 0, 4, 8, 12 are mortar (colour2, darker)
        let mortar = texel(&r, w, 4, [0; 3], 5, 4);
        let brick = texel(&r, w, 4, [0; 3], 5, 6);
        assert!(mortar[0] < brick[0] * 0.8, "mortar darker: {mortar:?} {brick:?}");
        assert_eq!(part(w, 4, [0; 3], 5, 8).0, 1);
        assert_eq!(part(w, 4, [0; 3], 5, 6).0, 0);
    }
}
