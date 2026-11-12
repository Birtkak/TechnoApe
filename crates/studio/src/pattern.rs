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
        Pattern::Leaves => {
            let h = hash(x.div_euclid(2) + cz * 8, y.div_euclid(2), 71);
            if h < 0.3 {
                (1, 1, 0.0, 1.0)
            } else if h > 0.9 {
                (2, 2, 0.0, 1.0)
            } else {
                plain
            }
        }
        Pattern::Plain | Pattern::Grass | Pattern::Stone => plain,
    }
}

/// The row whose look texel (x, y) of face `dir` shows: a grass block's side block under its fringe, any other
/// block's `side` on its sides and foot, else its own.
fn look_of<'a>(reg: &'a Registry, b: &'a Def, dir: i32, cell: [i32; 3], x: i32, y: i32) -> &'a Def {
    let n = S as i32;
    // a block that is its own side is drawn from `b` (the pattern panel hands in a changed row)
    let side = if b.side == b.id { b } else { reg.get(b.side).unwrap_or(b) };
    if b.pattern == Pattern::Grass && dir != 4 {
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
    }
}

/// Whether texel (x, y) of face `dir` of the block at `cell` is a hole of the leaves pattern (shader.wgsl
/// `leaf_hole`, ADR 0069): hashed by the texel's place on the face's plane, so the two faces between
/// neighbouring leaf blocks have their holes in the same places. Empty on a cutout block, dark on any other.
pub fn hole(reg: &Registry, b: &Def, dir: i32, cell: [i32; 3], x: i32, y: i32) -> bool {
    let m = look_of(reg, b, dir, cell, x, y);
    if m.pattern != Pattern::Leaves {
        return false;
    }
    // (x, y) back to the shader's grid on the face: `texel` turns a side so x runs rightwards, y down
    let (tx, ty) = if dir < 4 {
        let x0 = if dir == 1 || dir == 2 { 15 - x } else { x };
        if dir / 2 == 1 {
            (15 - y, x0)
        } else {
            (x0, 15 - y)
        }
    } else {
        (x, y)
    };
    let a = (dir / 2) as usize;
    let s = m.size.max(1) as i32;
    let (pu, pv) = (cell[(a + 1) % 3] * 16 + tx, cell[(a + 2) % 3] * 16 + ty);
    let plane = cell[a] + (dir % 2 == 0) as i32;
    hash(pu.div_euclid(s), pv.div_euclid(s), plane * 4 + a as i32 + 61) < m.amount
}

/// One texel of a block's face as the game draws it unpainted, 0..1 per channel. `dir` is the shader's: 0 to
/// 3 the sides, 4 the top, 5 the bottom; (x, y) with y down from the face's top edge; `cell` the block's place.
pub fn texel(reg: &Registry, b: &Def, dir: i32, cell: [i32; 3], x: i32, y: i32) -> [f32; 3] {
    let n = S as i32;
    let h = hash(cell[0] * n + x, cell[1] * n + y, cell[2] * n + dir);
    let tone = 0.82 + 0.12 * (h * 3.0).floor();
    let m = look_of(reg, b, dir, cell, x, y);
    if hole(reg, b, dir, cell, x, y) {
        // the shader's `pattern_texel` for a hole: what an opaque block (or fast leaves) shows there
        return m.colour.map(|v| (v * 0.6).clamp(0.0, 1.0));
    }
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
#[cfg(test)]
pub fn face(reg: &Registry, b: &Def, face: usize, pal: &Palette) -> Img {
    face_at(reg, b, face, pal, [0, 0, 0])
}

/// `face` as drawn on the block at `cell`: where a recipe has chance in it, another cell is another draw.
pub fn face_at(reg: &Registry, b: &Def, face: usize, pal: &Palette, cell: [i32; 3]) -> Img {
    let dir = [0, 4, 0, 5][face.min(3)];
    let mut img = Img::filled(1, [0; 3]);
    for y in 0..S {
        for x in 0..S {
            let (xi, yi) = (x as i32, y as i32);
            // a cutout block's hole is an empty pixel; elsewhere `texel`'s dark stand-in
            let c = if b.see == engine::blocks::See::Cutout && hole(reg, b, dir, cell, xi, yi) { crate::art::EMPTY } else { snap(pal, texel(reg, b, dir, cell, xi, yi)) };
            crate::art::pencil(&mut img, 0, x, y, c, false);
        }
    }
    img
}

/// The twelve patterns by the names `blocks.toml` gives them, in the engine's order.
pub const NAMES: [(Pattern, &str); 12] = [
    (Pattern::Plain, "plain"),
    (Pattern::Grass, "grass"),
    (Pattern::Stone, "stone"),
    (Pattern::Pebbles, "pebbles"),
    (Pattern::Speckle, "speckle"),
    (Pattern::Frame, "frame"),
    (Pattern::Weave, "weave"),
    (Pattern::Planks, "planks"),
    (Pattern::Tiles, "tiles"),
    (Pattern::Bricks, "bricks"),
    (Pattern::Rings, "rings"),
    (Pattern::Leaves, "leaves"),
];

/// What the pattern panel chose (docs/design/studio.md, usability round step 2): a pattern, its three
/// colours (the second and third None: "auto", one step darker and lighter on the base's ramp), its values
/// and a seed (the cell it is drawn at, so another seed is another draw of the same values).
#[derive(Clone, PartialEq, Debug)]
pub struct Recipe {
    pub pattern: Pattern,
    pub colours: [Option<[u8; 3]>; 3],
    pub size: u8,
    pub amount: f32,
    pub contrast: f32,
    pub seed: i32,
}

impl Recipe {
    /// The block's own row as a start: its pattern and values, its colour snapped to the palette, the others auto.
    pub fn of(b: &Def, pal: &Palette) -> Recipe {
        let base = snap(pal, b.colour);
        Recipe { pattern: b.pattern, colours: [Some(base), None, None], size: b.size, amount: b.amount, contrast: b.contrast, seed: 0 }
    }
    /// Which values `p` reads: (size, amount). Plain, grass and stone neither, rings no amount.
    pub fn reads(p: Pattern) -> (bool, bool) {
        (p.defaults().is_some(), p.defaults().is_some() && p != Pattern::Rings)
    }
    /// Picks another pattern: its own default size and amount, the colours and contrast kept.
    pub fn set_pattern(&mut self, p: Pattern) {
        self.pattern = p;
        if let Some((size, amount)) = p.defaults() {
            (self.size, self.amount) = (size, amount);
        }
    }
    /// The three colours, the auto ones made from the base.
    pub fn colours(&self, pal: &Palette) -> [[u8; 3]; 3] {
        let base = self.colours[0].unwrap_or([128; 3]);
        let auto = |k: usize| pal.step(base, if k == 1 { -1 } else { 1 }).unwrap_or(base);
        [base, self.colours[1].unwrap_or_else(|| auto(1)), self.colours[2].unwrap_or_else(|| auto(2))]
    }
    /// The block's row with this recipe in place of its look.
    fn def(&self, b: &Def, pal: &Palette) -> Def {
        let f = |c: [u8; 3]| c.map(|v| v as f32 / 255.0);
        let [c1, c2, c3] = self.colours(pal).map(f);
        Def { pattern: self.pattern, colour: c1, colour2: c2, colour3: c3, size: self.size.max(1), amount: self.amount, contrast: self.contrast, ..b.clone() }
    }
    /// One face drawn from the recipe at `seed`, snapped to the palette.
    pub fn draw(&self, reg: &Registry, b: &Def, face: usize, pal: &Palette, seed: i32) -> Img {
        face_at(reg, &self.def(b, pal), face, pal, [seed, 0, 0])
    }
    /// The file with the recipe filled in: variant `v` alone, or every variant from its own seed (a file of one
    /// variant then gets four: one pattern, four seeds, a varied field).
    pub fn fill(&self, img: &Img, v: usize, every: bool, reg: &Registry, b: &Def, face: usize, pal: &Palette) -> Img {
        let mut out = if every && img.variants() == 1 { Img::filled(4, [0; 3]) } else { img.clone() };
        for k in 0..out.variants() {
            if k == v || every {
                let one = self.draw(reg, b, face, pal, self.seed + if every { k as i32 * 101 } else { 0 });
                crate::tools::paste(&mut out, k, &crate::tools::copy(&one, 0, crate::tools::Sel { x: 0, y: 0, w: S, h: S }), 0, 0);
            }
        }
        out
    }
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

    fn ramp_pal() -> Palette {
        // two ramps far apart, so a texel with no tone (contrast 0) snaps to its own colour
        let mut p = Palette::default();
        for (i, c) in [[20, 20, 200], [60, 60, 230], [120, 120, 250], [200, 30, 30], [230, 90, 90], [250, 160, 160]].into_iter().enumerate() {
            let mut c = Colour::new(format!("c{i}"), c);
            c.ramp = Some(if i < 3 { "blue" } else { "red" }.into());
            p.colours.push(c);
        }
        p
    }

    #[test]
    fn a_recipe_reads_only_its_values_and_its_auto_colours_are_the_ramp_neighbours() {
        assert_eq!(Recipe::reads(Pattern::Plain), (false, false));
        assert_eq!(Recipe::reads(Pattern::Grass), (false, false));
        assert_eq!(Recipe::reads(Pattern::Stone), (false, false));
        assert_eq!(Recipe::reads(Pattern::Rings), (true, false));
        assert_eq!(Recipe::reads(Pattern::Bricks), (true, true));
        let p = ramp_pal();
        let r = reg();
        let mut k = Recipe::of(r.get(3).unwrap(), &p);
        k.colours[0] = Some([60, 60, 230]);
        assert_eq!(k.colours(&p), [[60, 60, 230], [20, 20, 200], [120, 120, 250]], "auto: one darker, one lighter");
        k.colours[2] = Some([250, 160, 160]);
        assert_eq!(k.colours(&p)[2], [250, 160, 160], "a set colour wins");
        k.set_pattern(Pattern::Tiles);
        assert_eq!((k.size, k.amount), (8, 0.3), "a new pattern starts at its defaults");
    }

    #[test]
    fn a_bricks_recipe_draws_its_colours_and_size_and_fills_one_or_every_variant() {
        let (p, r) = (ramp_pal(), reg());
        let b = r.get(3).unwrap();
        let k = Recipe { pattern: Pattern::Bricks, colours: [Some([200, 30, 30]), Some([20, 20, 200]), Some([250, 160, 160])], size: 3, amount: 0.0, contrast: 0.0, seed: 0 };
        let img = k.draw(&r, b, 1, &p, 0);
        for y in 0..S {
            let mortar = y % 3 == 0;
            assert_eq!(img.get(0, 5, y) == [20, 20, 200], mortar, "row {y}: mortar every 3 rows in the second colour");
        }
        assert!(img.px.iter().all(|c| p.find(*c).is_some()), "snapped to the palette");
        // every face and the sides draw the recipe too, not the registry's look of the block
        for f in [0, 2, 3] {
            let side = k.draw(&r, b, f, &p, 0);
            assert!((0..S).all(|y| (side.get(0, 5, y) == [20, 20, 200]) == (y % 3 == 0)), "face {f}: the recipe's mortar");
        }
        assert!(img.px.contains(&[200, 30, 30]), "the base colour shows");
        assert_eq!(k.draw(&r, b, 1, &p, 0), img, "the same recipe draws the same face");
        // a chance pattern: another seed, another draw
        let s = Recipe { pattern: Pattern::Speckle, size: 3, amount: 0.6, ..k.clone() };
        assert_ne!(s.draw(&r, b, 1, &p, 0), s.draw(&r, b, 1, &p, 1));
        let file = Img::filled(3, [60, 60, 230]);
        let one = s.fill(&file, 1, false, &r, b, 1, &p);
        assert_eq!((one.variants(), one.get(0, 0, 0), one.get(2, 0, 0)), (3, [60, 60, 230], [60, 60, 230]), "only variant 2 is filled");
        assert_ne!(one.get(1, 0, 0), [60, 60, 230]);
        let all = s.fill(&file, 1, true, &r, b, 1, &p);
        let v = |i: usize| (0..S * S).map(|n| all.get(i, n % S, n / S)).collect::<Vec<_>>();
        assert!(v(0) != v(1) && v(1) != v(2) && v(0) != v(2), "every variant from its own seed");
        assert!(v(0).iter().all(|c| *c != [60, 60, 230]));
        assert_eq!(s.fill(&Img::filled(1, [60, 60, 230]), 0, true, &r, b, 1, &p).variants(), 4, "one variant: every makes four");
        assert_eq!(s.fill(&Img::filled(1, [60, 60, 230]), 0, false, &r, b, 1, &p).variants(), 1);
    }

    #[test]
    fn leaves_leave_their_holes_empty_on_a_cutout_block_only() {
        let (p, r) = (ramp_pal(), reg());
        let mut b = r.get(1).unwrap().clone();
        b.pattern = Pattern::Leaves;
        (b.size, b.amount) = (2, 0.3);
        let holes = |img: &Img| (0..S * S).filter(|&n| img.empty_at(0, n % S, n / S)).count();
        assert_eq!(holes(&face(&r, &b, 2, &p)), 0, "an opaque block shows the dark stand-in");
        b.see = engine::blocks::See::Cutout;
        let img = face(&r, &b, 2, &p);
        let want = (0..S * S).filter(|&n| hole(&r, &b, 0, [0; 3], (n % S) as i32, (n / S) as i32)).count();
        assert!(want > 0 && holes(&img) == want, "every hole empty: {} of {want}", holes(&img));
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
