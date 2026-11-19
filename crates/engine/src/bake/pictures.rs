//! Pictures of a baked map without the game (a dev tool, `technoape-server bakemap=`): the whole planet in an
//! equirectangular projection (+z up, longitude across), each pixel the nearest cell's, with a legend under it.
//! The image and its 3 x 5 font are `prototypes/world-bake`'s.
use super::grid::Grid;
use super::mars::{self, Stats};
use super::Map;
use glam::DVec3;
use rayon::prelude::*;
use std::f64::consts::{FRAC_PI_2, PI, TAU};

pub struct Img {
    pub w: usize,
    pub h: usize,
    pub px: Vec<[u8; 3]>,
}

impl Img {
    pub fn put(&mut self, x: i64, y: i64, c: [u8; 3]) {
        if (0..self.w as i64).contains(&x) && (0..self.h as i64).contains(&y) {
            self.px[y as usize * self.w + x as usize] = c;
        }
    }
    pub fn rect(&mut self, x: usize, y: usize, w: usize, h: usize, c: [u8; 3]) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.put(xx as i64, yy as i64, c);
            }
        }
    }
    /// Text at (x, y), each font pixel `s` pixels.
    pub fn text(&mut self, x: usize, y: usize, t: &str, s: usize, c: [u8; 3]) {
        for (k, ch) in t.chars().enumerate() {
            for (row, bits) in glyph(ch).iter().enumerate() {
                for col in 0..3 {
                    if bits >> (2 - col) & 1 == 1 {
                        self.rect(x + (k * 4 + col) * s, y + row * s, s, s, c);
                    }
                }
            }
        }
    }
    pub fn save(&self, path: &str) -> std::io::Result<()> {
        if let Some(d) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(d)?;
        }
        let mut e = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(path)?), self.w as u32, self.h as u32);
        e.set_color(png::ColorType::Rgb);
        let flat: Vec<u8> = self.px.iter().flatten().copied().collect();
        e.write_header()?.write_image_data(&flat)?;
        Ok(())
    }
}

/// A 3 x 5 pixel font: rows of 3 bits.
fn glyph(ch: char) -> [u8; 5] {
    match ch.to_ascii_uppercase() {
        'A' => [2, 5, 7, 5, 5], 'B' => [6, 5, 6, 5, 6], 'C' => [3, 4, 4, 4, 3], 'D' => [6, 5, 5, 5, 6],
        'E' => [7, 4, 6, 4, 7], 'F' => [7, 4, 6, 4, 4], 'G' => [3, 4, 5, 5, 3], 'H' => [5, 5, 7, 5, 5],
        'I' => [7, 2, 2, 2, 7], 'J' => [1, 1, 1, 5, 2], 'K' => [5, 5, 6, 5, 5], 'L' => [4, 4, 4, 4, 7],
        'M' => [5, 7, 7, 5, 5], 'N' => [6, 5, 5, 5, 5], 'O' => [2, 5, 5, 5, 2], 'P' => [6, 5, 6, 4, 4],
        'Q' => [2, 5, 5, 6, 3], 'R' => [6, 5, 6, 5, 5], 'S' => [3, 4, 2, 1, 6], 'T' => [7, 2, 2, 2, 2],
        'U' => [5, 5, 5, 5, 7], 'V' => [5, 5, 5, 5, 2], 'W' => [5, 5, 7, 7, 5], 'X' => [5, 5, 2, 5, 5],
        'Y' => [5, 5, 2, 2, 2], 'Z' => [7, 1, 2, 4, 7], '0' => [7, 5, 5, 5, 7], '1' => [2, 6, 2, 2, 7],
        '2' => [6, 1, 2, 4, 7], '3' => [6, 1, 2, 1, 6], '4' => [5, 5, 7, 1, 1], '5' => [7, 4, 6, 1, 6],
        '6' => [3, 4, 7, 5, 7], '7' => [7, 1, 2, 2, 2], '8' => [7, 5, 7, 5, 7], '9' => [7, 5, 7, 1, 6],
        '.' => [0, 0, 0, 0, 2], '%' => [5, 1, 2, 4, 5], '-' => [0, 0, 7, 0, 0], ':' => [0, 2, 0, 2, 0],
        ',' => [0, 0, 0, 2, 4], '(' => [1, 2, 2, 2, 1], ')' => [4, 2, 2, 2, 4], '/' => [1, 1, 2, 4, 4],
        '+' => [0, 2, 7, 2, 0], '=' => [0, 7, 0, 7, 0],
        _ => [0; 5],
    }
}

fn lerp(stops: &[(f32, [f32; 3])], v: f32) -> [f32; 3] {
    if v <= stops[0].0 {
        return stops[0].1;
    }
    for w in stops.windows(2) {
        if v <= w[1].0 {
            let t = (v - w[0].0) / (w[1].0 - w[0].0);
            return std::array::from_fn(|k| w[0].1[k] + (w[1].1[k] - w[0].1[k]) * t);
        }
    }
    stops[stops.len() - 1].1
}

/// Height colours by absolute metres (as the real planet's altimetry maps: low blue to high white).
const HEIGHT: [(f32, [f32; 3]); 9] = [
    (-50.0, [60., 20., 110.]),
    (-25.0, [40., 70., 190.]),
    (-10.0, [50., 160., 210.]),
    (5.0, [70., 180., 110.]),
    (40.0, [225., 205., 85.]),
    (100.0, [220., 125., 55.]),
    (200.0, [175., 60., 45.]),
    (300.0, [125., 85., 75.]),
    (360.0, [255., 255., 255.]),
];

/// The colours of a planet's crust's kinds, in `mars::KINDS` order.
const KIND: [[u8; 3]; 11] = [
    [170, 105, 70],
    [228, 178, 120],
    [110, 62, 55],
    [60, 38, 40],
    [215, 55, 35],
    [245, 140, 25],
    [55, 170, 210],
    [105, 165, 85],
    [205, 215, 60],
    [250, 250, 255],
    [140, 185, 255],
];

/// The projection: centre longitude `lon0`; pixel to direction and back.
struct View {
    w: usize,
    h: usize,
    lon0: f64,
}
impl View {
    fn dir(&self, x: f64, y: f64) -> DVec3 {
        let (lon, lat) = (self.lon0 + (x / self.w as f64 - 0.5) * TAU, FRAC_PI_2 - y / self.h as f64 * PI);
        DVec3::new(lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin())
    }
    fn xy(&self, d: DVec3) -> (f64, f64) {
        let lon = (d.y.atan2(d.x) - self.lon0 + PI).rem_euclid(TAU) - PI;
        ((lon / TAU + 0.5) * self.w as f64, (FRAC_PI_2 - d.z.clamp(-1.0, 1.0).asin()) / PI * self.h as f64)
    }
}

/// The three pictures of a planet's crust's map, `<prefix>-height.png`, `-kinds.png` and `-features.png`; the paths.
pub fn mars(map: &Map, s: &Stats, prefix: &str) -> Vec<String> {
    let (w, h, legend) = (2400usize, 1200usize, 150usize);
    let g = Grid::bare(map.m as usize, 1.0);
    // the canyon's middle in the middle of the picture
    let mid = s.canyon.get(s.canyon.len() / 2).copied().unwrap_or(s.bulge);
    let v = View { w, h, lon0: mid.y.atan2(mid.x) };
    let height = |d: DVec3| {
        let (f, x, y) = g.locate_xy(d.normalize());
        map.height(f, x, y, 0)
    };
    // per pixel: height, kind, light (from the north-west, the slope over 1.5 cells)
    let e = 1.5 / map.m as f64 * FRAC_PI_2;
    let px: Vec<(f32, u8, f32)> = (0..w * h)
        .into_par_iter()
        .map(|k| {
            let d = v.dir((k % w) as f64 + 0.5, (k / w) as f64 + 0.5);
            let (ea, no) = (DVec3::Z.cross(d).normalize_or(DVec3::X), d.cross(DVec3::Z.cross(d).normalize_or(DVec3::X)));
            let (dx, dy) = (height(d + ea * e) - height(d - ea * e), height(d + no * e) - height(d - no * e));
            let cell_m = (2.0 * e * map.radius()) as f32;
            let light = (0.55 + 1.6 * (-dx + dy) / cell_m).clamp(0.0, 1.0);
            let (f, x, y) = g.locate_xy(d);
            (height(d), map.biome(f, x, y), light)
        })
        .collect();
    let canvas = || Img { w, h: h + legend, px: vec![[24, 22, 26]; w * (h + legend)] };
    let mut paths = Vec::new();
    let mut save = |img: Img, name: &str| {
        let p = format!("{prefix}-{name}.png");
        img.save(&p).unwrap_or_else(|e| panic!("{p}: {e}"));
        paths.push(p);
    };
    let title = |img: &mut Img, t: &str| img.text(12, h + 10, &format!("SEED {} N {} {t}", map.seed, map.n), 3, [235, 235, 235]);

    // 1. height with relief shading, a colour bar
    let mut a = canvas();
    for (k, &(hh, _, l)) in px.iter().enumerate() {
        let c = lerp(&HEIGHT, hh);
        a.px[k] = c.map(|c| (c * (0.45 + 0.75 * l)).min(255.0) as u8);
    }
    title(&mut a, "HEIGHT OVER THE DATUM, M (SHADED FROM THE NORTH-WEST)");
    for i in 0..900 {
        let hh = -50.0 + 410.0 * i as f32 / 900.0;
        a.rect(12 + i, h + 40, 1, 30, lerp(&HEIGHT, hh).map(|c| c as u8));
    }
    for hh in [-50, -25, 0, 40, 100, 200, 300, 360] {
        a.text(12 + ((hh + 50) as f32 / 410.0 * 900.0) as usize, h + 78, &hh.to_string(), 3, [235, 235, 235]);
    }
    save(a, "height");

    // 2. kinds in flat colours, a legend
    let mut b = canvas();
    for (k, &(_, kind, l)) in px.iter().enumerate() {
        b.px[k] = KIND[(kind - mars::HIGHLAND) as usize].map(|c| (c as f32 * (0.7 + 0.4 * l)).min(255.0) as u8);
    }
    title(&mut b, "KINDS OF GROUND");
    for (i, name) in mars::KINDS.iter().enumerate() {
        let (x, y) = (12 + (i % 6) * 395, h + 40 + (i / 6) * 40);
        b.rect(x, y, 28, 28, KIND[i]);
        b.text(x + 38, y + 6, name, 3, [235, 235, 235]);
    }
    save(b, "kinds");

    // 3. the features drawn over a grey relief
    let mut c = canvas();
    for (k, &(hh, _, l)) in px.iter().enumerate() {
        let t = ((hh + 50.0) / 410.0).clamp(0.0, 1.0);
        let g = (40.0 + 150.0 * l + 50.0 * t).min(255.0) as u8;
        c.px[k] = [g, g, g];
    }
    let mut line = |img: &mut Img, a: DVec3, b: DVec3, col: [u8; 3], thick: i64| {
        let ((x0, y0), (x1, y1)) = (v.xy(a), v.xy(b));
        if (x1 - x0).abs() > w as f64 / 2.0 {
            return;
        }
        let steps = ((x1 - x0).abs().max((y1 - y0).abs()).ceil() as usize).max(1);
        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            let (x, y) = ((x0 + (x1 - x0) * t) as i64, (y0 + (y1 - y0) * t) as i64);
            for dy in 0..thick {
                for dx in 0..thick {
                    img.put(x + dx - thick / 2, y + dy - thick / 2, col);
                }
            }
        }
    };
    let circle = |img: &mut Img, line: &mut dyn FnMut(&mut Img, DVec3, DVec3, [u8; 3], i64), c0: DVec3, a: f64, col: [u8; 3], thick: i64| {
        let (e1, e2) = (c0.any_orthonormal_vector(), c0.cross(c0.any_orthonormal_vector()));
        let pt = |k: usize| (c0 * a.cos() + (e1 * (k as f64 / 48.0 * TAU).cos() + e2 * (k as f64 / 48.0 * TAU).sin()) * a.sin()).normalize();
        for k in 0..48 {
            line(img, pt(k), pt(k + 1), col, thick);
        }
    };
    let r = map.radius();
    // the caps, the delta fans, the lake beds and the chaos as tints
    for (k, &(_, kind, _)) in px.iter().enumerate() {
        let tint = match kind {
            mars::CAP => Some([255, 255, 255]),
            mars::DELTA => Some([205, 215, 60]),
            mars::SEDIMENT => Some([105, 165, 85]),
            mars::CHAOS => Some([245, 140, 25]),
            _ => None,
        };
        if let Some(t) = tint {
            let p = c.px[k];
            c.px[k] = std::array::from_fn(|i| ((p[i] as u16 + t[i] as u16) / 2) as u8);
        }
    }
    for cr in s.craters.iter().filter(|cr| cr.2 && cr.1 >= 40.0) {
        circle(&mut c, &mut line, cr.0, 0.5 * cr.1 / r, [235, 150, 60], 1);
    }
    for b in &s.basins {
        circle(&mut c, &mut line, b.0, 0.5 * b.1 / r, [255, 220, 40], 4);
    }
    let gm = Grid::bare(map.m as usize, 1.0);
    let dir = |cell: u32| {
        let m = map.m as usize;
        let (f, i, j) = (cell as usize / (m * m), cell as usize % m, (cell as usize / m) % m);
        gm.dir_at(f, i as f64 + 0.5, j as f64 + 0.5)
    };
    for rv in &map.rivers {
        let t = ((rv.area / 6e4).log10() / 2.0).clamp(0.0, 1.0);
        line(&mut c, dir(rv.cell), dir(rv.to), [(70.0 - 40.0 * t) as u8, (140.0 + 60.0 * t) as u8, 255], 1 + (t > 0.5) as i64);
    }
    for ch in &s.channels {
        for p in ch.windows(2) {
            line(&mut c, p[0], p[1], [40, 230, 230], 4);
        }
    }
    for p in s.canyon.windows(2) {
        line(&mut c, p[0], p[1], [235, 40, 40], 4);
    }
    for t in &map.tubes {
        let (x, y) = v.xy(dir(t.0));
        let col = if t.1 < 0.0 { [255, 255, 255] } else { [255, 60, 230] };
        c.rect((x as usize).saturating_sub(1), (y as usize).saturating_sub(1), 3, 3, col);
    }
    for sh in &s.shields {
        circle(&mut c, &mut line, sh.c, sh.radius / r, [255, 90, 40], 2);
        circle(&mut c, &mut line, sh.c, (if sh.giant { 0.13 } else { 0.1 }) * sh.radius / r, [255, 255, 255], 2);
    }
    title(&mut c, "FEATURES");
    let keys: [(&str, [u8; 3]); 10] = [
        ("CRATERS SEEN OVER 40 M", [235, 150, 60]),
        ("GIANT BASINS", [255, 220, 40]),
        ("SHIELDS AND CALDERAS", [255, 90, 40]),
        ("LAVA TUBES (WHITE: PITS)", [255, 60, 230]),
        ("CANYON AXIS", [235, 40, 40]),
        ("OUTFLOW CHANNELS", [40, 230, 230]),
        ("DRY VALLEYS", [60, 170, 255]),
        ("LAKE BEDS", [105, 165, 85]),
        ("DELTAS", [205, 215, 60]),
        ("CHAOS, CAPS", [245, 140, 25]),
    ];
    for (i, (name, col)) in keys.iter().enumerate() {
        let (x, y) = (12 + (i % 5) * 475, h + 40 + (i / 5) * 40);
        c.rect(x, y, 28, 28, *col);
        c.text(x + 38, y + 6, name, 3, [235, 235, 235]);
    }
    save(c, "features");
    paths
}
