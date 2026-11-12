//! The texture tools without a window (docs/design/studio-tools.md "Textures: the tools"): tiled painting and
//! the seam view, shade, scatter, spray, dither fill, gradient, the selection, the wrap shift, start from a
//! picture and make variants. Each works on one variant of a texture and is a pure function of its inputs.
use crate::art::{pencil, Img, S};
use crate::wall::Rng;
use engine::palette::Palette;

fn wrap(v: i32) -> usize {
    v.rem_euclid(S as i32) as usize
}

/// The pixels of a straight line, both ends included, in any coordinates (Bresenham).
pub fn line_points(a: (i32, i32), b: (i32, i32)) -> Vec<(i32, i32)> {
    let (mut x, mut y) = a;
    let (dx, dy) = ((b.0 - x).abs(), -(b.1 - y).abs());
    let (sx, sy) = ((b.0 - x).signum(), (b.1 - y).signum());
    let mut err = dx + dy;
    let mut out = vec![];
    loop {
        out.push((x, y));
        if (x, y) == b {
            return out;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

/// Tiled painting: a line anywhere on the canvas and its ghosted neighbours lands wrapped on the face, so what
/// leaves one edge comes in at the other.
pub fn line_wrap(img: &mut Img, v: usize, a: (i32, i32), b: (i32, i32), c: [u8; 3], mirror: bool) {
    for (x, y) in line_points(a, b) {
        pencil(img, v, wrap(x), wrap(y), c, mirror);
    }
}

/// The 4-connected area of one colour holding (x, y); with `tiled` it runs over the edges.
pub fn region(img: &Img, v: usize, x: usize, y: usize, tiled: bool) -> Vec<(usize, usize)> {
    let from = img.get(v, x, y);
    let mut seen = [[false; S]; S];
    let (mut todo, mut out) = (vec![(x as i32, y as i32)], vec![]);
    while let Some((x, y)) = todo.pop() {
        if !tiled && !(0..S as i32).contains(&x) || !tiled && !(0..S as i32).contains(&y) {
            continue;
        }
        let (ux, uy) = (wrap(x), wrap(y));
        if seen[uy][ux] || img.get(v, ux, uy) != from {
            continue;
        }
        seen[uy][ux] = true;
        out.push((ux, uy));
        let (x, y) = (ux as i32, uy as i32);
        todo.extend([(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)]);
    }
    out
}

/// Fill, tiled: the area runs over the edges.
pub fn fill_wrap(img: &mut Img, v: usize, x: usize, y: usize, c: [u8; 3]) {
    for (x, y) in region(img, v, x, y, true) {
        pencil(img, v, x, y, c, false);
    }
}

/// Whether two colours side by side clash: different and not neighbours on one ramp.
pub fn clash(pal: &Palette, a: [u8; 3], b: [u8; 3]) -> bool {
    if a == b {
        return false;
    }
    let (Some(i), Some(j)) = (pal.find(a), pal.find(b)) else { return true };
    match (&pal.colours[i].ramp, &pal.colours[j].ramp) {
        (Some(r), Some(q)) if r == q => {
            let run = pal.ramp(r);
            let p = |k| run.iter().position(|&x| x == k).unwrap_or(0) as i32;
            (p(i) - p(j)).abs() > 1
        }
        _ => true,
    }
}

/// The seam view: the edge pixels that clash with the pixel across the far edge they meet when tiled.
pub fn seams(img: &Img, v: usize, pal: &Palette) -> Vec<(usize, usize)> {
    let mut out = vec![];
    for k in 0..S {
        for (p, q) in [((0, k), (S - 1, k)), ((k, 0), (k, S - 1))] {
            if clash(pal, img.get(v, p.0, p.1), img.get(v, q.0, q.1)) {
                out.extend([p, q]);
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Shade: the pixel `steps` along its ramp (negative darker); whether it changed.
pub fn shade(img: &mut Img, v: usize, x: usize, y: usize, pal: &Palette, steps: i32, mirror: bool) -> bool {
    let c = img.get(v, x, y);
    match pal.step(c, steps).filter(|n| *n != c) {
        Some(n) => {
            pencil(img, v, x, y, n, mirror);
            true
        }
        None => false,
    }
}

/// Scatter: the colour or, by `amount` (0..1), one of its ramp neighbours at random.
pub fn scatter_colour(pal: &Palette, c: [u8; 3], amount: f32, rng: &mut Rng) -> [u8; 3] {
    let r = rng.f32();
    let steps = if r < amount / 2.0 { -1 } else if r < amount { 1 } else { 0 };
    pal.step(c, steps).unwrap_or(c)
}

/// Spray: each pixel within `radius` of (x, y) painted by chance `density` (0..1); wraps over the edges.
pub fn spray(img: &mut Img, v: usize, (x, y): (i32, i32), radius: i32, density: f32, c: [u8; 3], mirror: bool, rng: &mut Rng) {
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx * dx + dy * dy <= radius * radius + radius && rng.f32() < density {
                pencil(img, v, wrap(x + dx), wrap(y + dy), c, mirror);
            }
        }
    }
}

/// Dither fill: the area that holds (x, y) becomes a checker of `a` and `b`.
pub fn dither(img: &mut Img, v: usize, x: usize, y: usize, a: [u8; 3], b: [u8; 3], tiled: bool) {
    for (x, y) in region(img, v, x, y, tiled) {
        pencil(img, v, x, y, if (x + y) % 2 == 0 { a } else { b }, false);
    }
}

const BAYER: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

/// Gradient: the whole variant runs through `ramp` (first at `a`, last at `b`), ordered-dithered between steps.
pub fn gradient(img: &mut Img, v: usize, a: (f32, f32), b: (f32, f32), ramp: &[[u8; 3]]) {
    let Some(&last) = ramp.last() else { return };
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    for y in 0..S {
        for x in 0..S {
            let t = if len2 == 0.0 { 0.0 } else { (((x as f32 + 0.5 - a.0) * dx + (y as f32 + 0.5 - a.1) * dy) / len2).clamp(0.0, 1.0) };
            let f = t * (ramp.len() - 1) as f32;
            let threshold = (BAYER[y % 4][x % 4] as f32 + 0.5) / 16.0;
            let i = f.floor() as usize + (f.fract() > threshold) as usize;
            pencil(img, v, x, y, *ramp.get(i).unwrap_or(&last), false);
        }
    }
}

/// A rectangle of pixels on a face.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Sel {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
}

impl Sel {
    pub fn corners(a: (usize, usize), b: (usize, usize)) -> Sel {
        let (x0, x1, y0, y1) = (a.0.min(b.0).min(S - 1), a.0.max(b.0).min(S - 1), a.1.min(b.1).min(S - 1), a.1.max(b.1).min(S - 1));
        Sel { x: x0, y: y0, w: x1 - x0 + 1, h: y1 - y0 + 1 }
    }
    pub fn contains(&self, (x, y): (usize, usize)) -> bool {
        (self.x..self.x + self.w).contains(&x) && (self.y..self.y + self.h).contains(&y)
    }
}

/// Copied pixels.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Clip {
    pub w: usize,
    pub h: usize,
    pub px: Vec<[u8; 3]>,
}

impl Clip {
    fn at(&self, x: usize, y: usize) -> [u8; 3] {
        self.px[y * self.w + x]
    }
    pub fn flip_x(&self) -> Clip {
        Clip { px: (0..self.h).flat_map(|y| (0..self.w).rev().map(move |x| (x, y))).map(|(x, y)| self.at(x, y)).collect(), ..*self }
    }
    pub fn flip_y(&self) -> Clip {
        Clip { px: (0..self.h).rev().flat_map(|y| (0..self.w).map(move |x| (x, y))).map(|(x, y)| self.at(x, y)).collect(), ..*self }
    }
    /// A quarter turn clockwise.
    pub fn turn(&self) -> Clip {
        let (w, h) = (self.h, self.w);
        Clip { w, h, px: (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).map(|(x, y)| self.at(y, self.h - 1 - x)).collect() }
    }
}

pub fn copy(img: &Img, v: usize, s: Sel) -> Clip {
    Clip { w: s.w, h: s.h, px: (s.y..s.y + s.h).flat_map(|y| (s.x..s.x + s.w).map(move |x| (x, y))).map(|(x, y)| img.get(v, x, y)).collect() }
}

/// Pastes with its top left at (x, y), wrapping over the edges.
pub fn paste(img: &mut Img, v: usize, clip: &Clip, x: i32, y: i32) {
    for cy in 0..clip.h {
        for cx in 0..clip.w {
            pencil(img, v, wrap(x + cx as i32), wrap(y + cy as i32), clip.at(cx, cy), false);
        }
    }
}

/// Fills a rectangle with one colour: the hole a moved selection leaves (a texture has no empty pixels).
pub fn cut(img: &mut Img, v: usize, s: Sel, c: [u8; 3]) {
    paste(img, v, &Clip { w: s.w, h: s.h, px: vec![c; s.w * s.h] }, s.x as i32, s.y as i32);
}

/// The arrow keys: the whole face moved by (dx, dy) with wrap.
pub fn shift(img: &mut Img, v: usize, dx: i32, dy: i32) {
    let all = copy(img, v, Sel { x: 0, y: 0, w: S, h: S });
    paste(img, v, &all, dx, dy);
}

/// The palette colour nearest to `c` (a weighted distance that follows the eye better than plain RGB).
pub fn snap(pal: &Palette, c: [f32; 3]) -> [u8; 3] {
    let d = |p: [u8; 3]| {
        let [r, g, b] = [0, 1, 2].map(|i| p[i] as f32 / 255.0 - c[i]);
        let rm = (p[0] as f32 / 255.0 + c[0]) / 2.0;
        (2.0 + rm) * r * r + 4.0 * g * g + (3.0 - rm) * b * b
    };
    pal.colours.iter().map(|p| p.rgb).min_by(|a, b| d(*a).total_cmp(&d(*b))).unwrap_or([128; 3])
}

/// Any 8-bit PNG (grey, grey and alpha, RGB, RGBA, or palette) as RGBA bytes: (width, height, pixels).
// shortcut: PNG only (the png crate is already here); a JPEG or other format needs the image crate, add it if wanted
pub fn decode_picture(bytes: &[u8]) -> Result<(usize, usize, Vec<[u8; 4]>), String> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut r = dec.read_info().map_err(|e| format!("not a PNG this can read: {e}"))?;
    let mut buf = vec![0; r.output_buffer_size().ok_or("too large")?];
    let info = r.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let (w, h) = (info.width as usize, info.height as usize);
    let n = info.color_type.samples();
    let px = buf[..info.buffer_size()]
        .chunks(n)
        .map(|p| match n {
            1 => [p[0], p[0], p[0], 255],
            2 => [p[0], p[0], p[0], p[1]],
            3 => [p[0], p[1], p[2], 255],
            _ => [p[0], p[1], p[2], p[3]],
        })
        .collect();
    Ok((w, h, px))
}

/// Start from a picture: its middle square cut to 16 x 16 (each pixel the mean of its box, transparent parts
/// left out) and snapped to the palette, as a rough to paint over.
pub fn from_picture(w: usize, h: usize, px: &[[u8; 4]], pal: &Palette) -> Img {
    let side = w.min(h).max(1);
    let (ox, oy) = ((w - side.min(w)) / 2, (h - side.min(h)) / 2);
    let mut img = Img::filled(1, [0; 3]);
    for ty in 0..S {
        for tx in 0..S {
            let (x0, x1) = (ox + tx * side / S, (ox + (tx + 1) * side / S).max(ox + tx * side / S + 1));
            let (y0, y1) = (oy + ty * side / S, (oy + (ty + 1) * side / S).max(oy + ty * side / S + 1));
            let mut sum = [0.0f32; 4];
            for y in y0..y1.min(h) {
                for x in x0..x1.min(w) {
                    let p = px[y * w + x];
                    let a = p[3] as f32 / 255.0;
                    for i in 0..3 {
                        sum[i] += p[i] as f32 / 255.0 * a;
                    }
                    sum[3] += a;
                }
            }
            let c = if sum[3] > 0.0 { [sum[0] / sum[3], sum[1] / sum[3], sum[2] / sum[3]] } else { [0.5; 3] };
            pencil(&mut img, 0, tx, ty, snap(pal, c), false);
        }
    }
    img
}

/// Make variants: variant 1 kept, and up to `n` in all made from it by moving a `share` (0..1) of its pixels
/// to a neighbour's place and re-scattering others one step along their ramp.
pub fn make_variants(img: &Img, n: usize, share: f32, pal: &Palette, rng: &mut Rng) -> Img {
    let n = n.clamp(1, 4);
    let mut out = Img::filled(n, [0; 3]);
    for v in 0..n {
        for y in 0..S {
            for x in 0..S {
                pencil(&mut out, v, x, y, img.get(0, x, y), false);
            }
        }
        if v == 0 {
            continue;
        }
        let k = (share.clamp(0.0, 1.0) * (S * S) as f32 / 2.0).round() as usize;
        for _ in 0..k {
            let (x, y) = (rng.below(S), rng.below(S));
            if rng.f32() < 0.5 {
                let (nx, ny) = [(1, 0), (-1, 0), (0, 1), (0, -1)][rng.below(4)];
                let (qx, qy) = (wrap(x as i32 + nx), wrap(y as i32 + ny));
                let (a, b) = (out.get(v, x, y), out.get(v, qx, qy));
                pencil(&mut out, v, x, y, b, false);
                pencil(&mut out, v, qx, qy, a, false);
            } else {
                let c = out.get(v, x, y);
                let c = pal.step(c, if rng.f32() < 0.5 { -1 } else { 1 }).unwrap_or(c);
                pencil(&mut out, v, x, y, c, false);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::palette::Colour;

    const D: [u8; 3] = [20, 40, 10];
    const M: [u8; 3] = [60, 120, 30];
    const L: [u8; 3] = [140, 220, 90];
    const R: [u8; 3] = [200, 30, 30];

    /// A ramp dark, mid, light and one loose red.
    fn pal() -> Palette {
        let mut p = Palette::default();
        for (n, c, r) in [("d", D, Some("g")), ("m", M, Some("g")), ("l", L, Some("g")), ("r", R, None)] {
            p.colours.push(Colour { name: n.into(), rgb: c, ramp: r.map(String::from) });
        }
        p
    }
    fn count(img: &Img, v: usize, c: [u8; 3]) -> usize {
        (0..S).flat_map(|y| (0..S).map(move |x| (x, y))).filter(|&(x, y)| img.get(v, x, y) == c).count()
    }

    #[test]
    fn a_tiled_stroke_wraps_over_the_edges() {
        let mut img = Img::filled(1, M);
        line_wrap(&mut img, 0, (14, 3), (17, 3), D, false);
        assert_eq!([0, 1, 14, 15].map(|x| img.get(0, x, 3)), [D; 4], "what leaves the right edge comes in at the left");
        assert_eq!(count(&img, 0, D), 4);
        line_wrap(&mut img, 0, (5, -1), (5, -1), L, false);
        assert_eq!(img.get(0, 5, 15), L, "above the top is the bottom row");
        let mut f = Img::filled(1, M);
        for y in 0..S {
            pencil(&mut f, 0, 8, y, R, false); // a wall in the middle
        }
        fill_wrap(&mut f, 0, 0, 0, D);
        assert_eq!(count(&f, 0, D), 15 * S, "round the back of the wall, over the edge");
        assert_eq!(region(&f, 0, 8, 0, false).len(), S);
    }

    #[test]
    fn the_seam_view_marks_edges_that_clash_across_the_tile() {
        let p = pal();
        let mut img = Img::filled(1, M);
        assert!(seams(&img, 0, &p).is_empty());
        pencil(&mut img, 0, 0, 4, L, false);
        assert!(seams(&img, 0, &p).is_empty(), "one ramp step is no clash");
        pencil(&mut img, 0, 0, 4, R, false);
        assert_eq!(seams(&img, 0, &p), vec![(0, 4), (15, 4)]);
        pencil(&mut img, 0, 6, 15, D, false);
        pencil(&mut img, 0, 6, 0, L, false);
        assert!(seams(&img, 0, &p).contains(&(6, 0)) && seams(&img, 0, &p).contains(&(6, 15)), "two steps apart clash");
    }

    #[test]
    fn shade_steps_along_the_ramp() {
        let p = pal();
        let mut img = Img::filled(1, M);
        assert!(shade(&mut img, 0, 1, 1, &p, -1, false));
        assert_eq!(img.get(0, 1, 1), D);
        assert!(!shade(&mut img, 0, 1, 1, &p, -1, false), "the dark end stays");
        assert!(shade(&mut img, 0, 2, 2, &p, 1, true));
        assert_eq!((img.get(0, 2, 2), img.get(0, 13, 2)), (L, L), "the mirror too");
        pencil(&mut img, 0, 3, 3, R, false);
        assert!(!shade(&mut img, 0, 3, 3, &p, 1, false), "a loose colour has no ramp to step on");
    }

    #[test]
    fn scatter_and_spray_stay_in_the_palette_and_follow_the_seed() {
        let p = pal();
        let mut r = Rng::new(3);
        let got: Vec<[u8; 3]> = (0..200).map(|_| scatter_colour(&p, M, 0.6, &mut r)).collect();
        assert!(got.contains(&D) && got.contains(&L) && got.contains(&M));
        let share = got.iter().filter(|c| **c != M).count();
        assert!((80..160).contains(&share), "about the amount: {share}");
        assert!((0..50).map(|_| scatter_colour(&p, M, 0.0, &mut r)).all(|c| c == M), "no amount, no grain");
        let (mut a, mut b) = (Img::filled(1, M), Img::filled(1, M));
        spray(&mut a, 0, (0, 0), 3, 0.5, D, false, &mut Rng::new(9));
        spray(&mut b, 0, (0, 0), 3, 0.5, D, false, &mut Rng::new(9));
        assert_eq!(a, b, "the same seed sprays the same");
        let n = count(&a, 0, D);
        assert!(n > 5 && n < 40, "{n}");
        assert!(a.get(0, 15, 15) == D || a.get(0, 15, 0) == D || a.get(0, 0, 15) == D, "it wraps round the corner");
        let mut full = Img::filled(1, M);
        spray(&mut full, 0, (8, 8), 2, 1.0, D, false, &mut Rng::new(1));
        assert_eq!(full.get(0, 8, 8), D);
        assert_eq!(full.get(0, 8, 11), M, "nothing past the radius");
    }

    #[test]
    fn dither_fills_the_area_with_a_checker() {
        let mut img = Img::filled(1, M);
        for y in 0..S {
            pencil(&mut img, 0, 4, y, R, false);
        }
        dither(&mut img, 0, 0, 0, D, L, false);
        assert_eq!((img.get(0, 0, 0), img.get(0, 1, 0), img.get(0, 0, 1), img.get(0, 3, 15)), (D, L, L, D));
        assert_eq!(img.get(0, 5, 0), M, "not past the wall");
        assert_eq!(count(&img, 0, R), S);
    }

    #[test]
    fn a_gradient_runs_the_ramp_along_the_line_dithered() {
        let mut img = Img::filled(1, R);
        gradient(&mut img, 0, (0.5, 8.0), (15.5, 8.0), &[D, M, L]);
        assert!((0..S).all(|y| img.get(0, 0, y) == D && img.get(0, 15, y) == L), "the ends are the ends");
        assert_eq!(count(&img, 0, R), 0);
        let m = count(&img, 0, M);
        assert!(m > 60 && m < 180, "the middle of the ramp in the middle: {m}");
        let col = |x: usize| (0..S).filter(|&y| img.get(0, x, y) == M).count();
        assert!(col(3) > 0 && col(3) < S, "dithered between two steps, not banded");
        let mut v = Img::filled(1, R);
        gradient(&mut v, 0, (8.0, 0.5), (8.0, 15.5), &[D, L]);
        assert!((0..S).all(|x| v.get(0, x, 0) == D && v.get(0, x, 15) == L), "and top to bottom");
    }

    #[test]
    fn a_selection_copies_flips_turns_pastes_and_shifts_with_wrap() {
        let mut img = Img::filled(1, M);
        pencil(&mut img, 0, 2, 3, D, false); // the top left of a 3 x 2 selection
        let s = Sel::corners((4, 4), (2, 3));
        assert_eq!(s, Sel { x: 2, y: 3, w: 3, h: 2 });
        assert!(s.contains((4, 4)) && !s.contains((5, 4)));
        let c = copy(&img, 0, s);
        assert_eq!((c.w, c.h, c.px[0]), (3, 2, D));
        assert_eq!(c.flip_x().px[2], D, "flipped: top right");
        assert_eq!(c.flip_y().px[3], D, "flipped: bottom left");
        let t = c.turn();
        assert_eq!((t.w, t.h, t.px[1]), (2, 3, D), "a quarter turn: the top left goes to the top right");
        assert_eq!(c.turn().turn().turn().turn(), c);
        paste(&mut img, 0, &c, 15, 15);
        assert_eq!((img.get(0, 15, 15), img.get(0, 1, 0)), (D, M), "pasted over the corner, wrapped");
        cut(&mut img, 0, s, L);
        assert_eq!(count(&img, 0, L), 6);
        let mut sh = Img::filled(1, M);
        pencil(&mut sh, 0, 15, 0, D, false);
        shift(&mut sh, 0, 1, -1);
        assert_eq!(sh.get(0, 0, 15), D, "right off the edge, up off the top");
        assert_eq!(count(&sh, 0, D), 1);
    }

    #[test]
    fn a_picture_is_cut_to_sixteen_and_snapped_to_the_palette() {
        let p = pal();
        assert_eq!(snap(&p, [0.25, 0.5, 0.1]), M);
        assert_eq!(snap(&p, [1.0, 0.0, 0.0]), R);
        // 40 x 32: the middle 32 x 32, left half red, right half near the mid green
        let (w, h) = (40, 32);
        let px: Vec<[u8; 4]> = (0..w * h).map(|i| if i % w < 4 + 16 { [210, 20, 20, 255] } else { [70, 110, 40, 255] }).collect();
        let img = from_picture(w, h, &px, &p);
        assert_eq!((img.get(0, 0, 0), img.get(0, 7, 9), img.get(0, 8, 0), img.get(0, 15, 15)), (R, R, M, M));
        let mut out = Vec::new();
        let mut e = png::Encoder::new(&mut out, 2, 1);
        e.set_color(png::ColorType::Rgb);
        let mut wr = e.write_header().unwrap();
        wr.write_image_data(&[1, 2, 3, 4, 5, 6]).unwrap();
        wr.finish().unwrap();
        assert_eq!(decode_picture(&out).unwrap(), (2, 1, vec![[1, 2, 3, 255], [4, 5, 6, 255]]));
        assert!(decode_picture(b"not a png").is_err());
    }

    #[test]
    fn variants_are_made_from_the_first_and_follow_the_seed() {
        let p = pal();
        let mut img = Img::filled(1, M);
        for i in 0..S {
            pencil(&mut img, 0, i, i, D, false);
            pencil(&mut img, 0, i, 15 - i, L, false);
        }
        let a = make_variants(&img, 3, 0.3, &p, &mut Rng::new(5));
        assert_eq!(a, make_variants(&img, 3, 0.3, &p, &mut Rng::new(5)), "the same seed, the same variants");
        assert_eq!(a.variants(), 3);
        let same = |v: usize| (0..S).flat_map(|y| (0..S).map(move |x| (x, y))).filter(|&(x, y)| a.get(v, x, y) == img.get(0, x, y)).count();
        assert_eq!(same(0), S * S, "variant 1 is kept");
        for v in 1..3 {
            assert!(same(v) < S * S && same(v) > S * S / 2, "changed a little: {}", same(v));
        }
        assert!(a.px.iter().all(|c| p.find(*c).is_some()), "only palette colours");
        let b = make_variants(&img, 2, 0.0, &p, &mut Rng::new(5));
        assert!((0..S).all(|y| (0..S).all(|x| b.get(1, x, y) == img.get(0, x, y))), "no share, a copy");
    }
}
