//! Pictures of a bake. Every view maps a pixel to a direction and samples the nearest cell, so a seam in a
//! picture is a seam in the data, never in the drawing.
use crate::bake::Bake;
use glam::DVec3;
use rayon::prelude::*;

pub struct Img {
    pub w: usize,
    pub h: usize,
    pub px: Vec<[u8; 3]>,
}

impl Img {
    pub fn new(w: usize, h: usize, c: [u8; 3]) -> Img {
        Img { w, h, px: vec![c; w * h] }
    }
    pub fn put(&mut self, x: i64, y: i64, c: [u8; 3]) {
        if (0..self.w as i64).contains(&x) && (0..self.h as i64).contains(&y) {
            self.px[y as usize * self.w + x as usize] = c;
        }
    }
    pub fn blit(&mut self, o: &Img, x0: usize, y0: usize) {
        for y in 0..o.h {
            for x in 0..o.w {
                self.put((x0 + x) as i64, (y0 + y) as i64, o.px[y * o.w + x]);
            }
        }
    }
    pub fn save(&self, path: &std::path::Path) {
        let f = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
        let mut e = png::Encoder::new(f, self.w as u32, self.h as u32);
        e.set_color(png::ColorType::Rgb);
        e.set_depth(png::BitDepth::Eight);
        let flat: Vec<u8> = self.px.iter().flatten().copied().collect();
        e.write_header().unwrap().write_image_data(&flat).unwrap();
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Layer {
    Height,
    Plates,
    Rivers,
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

/// Colours by absolute metres, so two reliefs of one seed look different.
fn height_colour(h: f32) -> [f32; 3] {
    const LAND: [(f32, [f32; 3]); 6] = [
        (0.0, [78., 140., 62.]),
        (15.0, [118., 158., 78.]),
        (40.0, [165., 152., 96.]),
        (75.0, [128., 102., 74.]),
        (110.0, [150., 146., 142.]),
        (140.0, [245., 245., 245.]),
    ];
    const SEA: [(f32, [f32; 3]); 4] = [(-100.0, [8., 22., 64.]), (-40.0, [18., 48., 118.]), (-10.0, [36., 96., 160.]), (0.0, [70., 140., 190.])];
    if h >= 0.0 { lerp(&LAND, h) } else { lerp(&SEA, h) }
}

fn plate_colour(p: u8) -> [f32; 3] {
    let x = (p as u32).wrapping_mul(2654435761) >> 8;
    [(60 + (x & 0x7f)) as f32, (60 + ((x >> 7) & 0x7f)) as f32, (60 + ((x >> 14) & 0x7f)) as f32]
}

/// Light on the ground: the slope from the neighbours, lit from `sun`, exaggerated `ex` times.
fn shade(b: &Bake, c: u32, sun: DVec3, ex: f64) -> f32 {
    let g = &b.g;
    let up = g.d(c);
    let (nb, k) = g.neigh(c);
    let hc = b.w[c as usize].max(0.0) as f64;
    let mut grad = DVec3::ZERO;
    for &o in &nb[..k] {
        let t = g.d(o) - up;
        let l = t.length() * g.r;
        grad += t.normalize() * ((b.w[o as usize].max(0.0) as f64 - hc) / l);
    }
    grad *= 2.0 / k as f64;
    let n = (up - grad * ex).normalize();
    // hillshade: the light comes from high up on the sun's side, wherever the cell is
    let sun = (up + (sun - up * sun.dot(up)).normalize_or_zero() * 0.9).normalize();
    (n.dot(sun).max(0.0) * 0.75 + 0.3) as f32
}

/// The colour of cell c for a layer.
pub fn colour(b: &Bake, c: u32, layer: Layer, sun: DVec3, ex: f64) -> [u8; 3] {
    let i = c as usize;
    let (h, land) = (b.h[i], b.land(i));
    let s = if land { shade(b, c, sun, ex) } else { 1.0 };
    let col = match layer {
        Layer::Height => {
            if b.lake(i) {
                [90., 150., 205.]
            } else {
                height_colour(h).map(|v| v * s)
            }
        }
        Layer::Plates => {
            let k = plate_colour(b.plate[i]);
            let m = if b.continental[i] { 1.3 } else { 0.7 };
            match b.boundary[i] {
                1 => [230., 40., 40.],
                2 => [40., 120., 255.],
                3 => [240., 220., 60.],
                _ => k.map(|v| (v * m).min(255.0) * if land { s } else { 1.0 }),
            }
        }
        Layer::Rivers => {
            if !land {
                [16., 24., 48.]
            } else if b.lake(i) {
                [110., 170., 230.]
            } else {
                [70. * s, 70. * s, 66. * s]
            }
        }
    };
    col.map(|v| v.clamp(0.0, 255.0) as u8)
}

/// A view: maps a pixel (x, y) in [-1, 1] to a direction, or None off the picture.
pub trait View: Sync {
    fn dir(&self, x: f64, y: f64) -> Option<DVec3>;
    /// where a direction lands, in [-1, 1]
    fn back(&self, d: DVec3) -> Option<(f64, f64)>;
}

/// The globe from far away (orthographic), looking at `at` with `up` up.
pub struct Globe {
    pub at: DVec3,
    pub up: DVec3,
}
impl Globe {
    fn basis(&self) -> (DVec3, DVec3, DVec3) {
        let f = self.at.normalize();
        let r = self.up.cross(f).normalize();
        (f, r, f.cross(r))
    }
}
impl View for Globe {
    fn dir(&self, x: f64, y: f64) -> Option<DVec3> {
        let (f, r, u) = self.basis();
        let z2 = 1.0 - x * x - y * y;
        (z2 > 0.0).then(|| f * z2.sqrt() + r * x + u * y)
    }
    fn back(&self, d: DVec3) -> Option<(f64, f64)> {
        let (f, r, u) = self.basis();
        (d.dot(f) > 0.0).then(|| (d.dot(r), d.dot(u)))
    }
}

/// A close-up around `at` (gnomonic), `half` metres from the middle to the edge on a sphere of radius r.
pub struct Close {
    pub at: DVec3,
    pub up: DVec3,
    pub half: f64,
    pub r: f64,
}
impl View for Close {
    fn dir(&self, x: f64, y: f64) -> Option<DVec3> {
        let g = Globe { at: self.at, up: self.up };
        let (f, r, u) = g.basis();
        let k = self.half / self.r;
        Some((f + r * x * k + u * y * k).normalize())
    }
    fn back(&self, d: DVec3) -> Option<(f64, f64)> {
        let g = Globe { at: self.at, up: self.up };
        let (f, r, u) = g.basis();
        let k = self.half / self.r;
        let z = d.dot(f);
        (z > 0.0).then(|| (d.dot(r) / z / k, d.dot(u) / z / k))
    }
}

/// The cube unfolded as a cross: +Y on top of +Z, then -X +Z +X -Z in a row, -Y under +Z. Pixels are
/// spaced evenly in angle, as the cells are.
pub struct Cross;
/// (tile x, tile y, face centre, right, up)
const TILES: [(usize, usize, [f64; 3], [f64; 3], [f64; 3]); 6] = [
    (1, 0, [0., 1., 0.], [1., 0., 0.], [0., 0., -1.]),
    (0, 1, [-1., 0., 0.], [0., 0., 1.], [0., 1., 0.]),
    (1, 1, [0., 0., 1.], [1., 0., 0.], [0., 1., 0.]),
    (2, 1, [1., 0., 0.], [0., 0., -1.], [0., 1., 0.]),
    (3, 1, [0., 0., -1.], [-1., 0., 0.], [0., 1., 0.]),
    (1, 2, [0., -1., 0.], [1., 0., 0.], [0., 0., 1.]),
];
impl View for Cross {
    // x in [-1, 1] spans 4 tiles, y spans 3 (the image is 4:3)
    fn dir(&self, x: f64, y: f64) -> Option<DVec3> {
        let (tx, ty) = ((x + 1.0) * 2.0, (1.0 - y) * 1.5);
        let (ix, iy) = (tx.floor() as usize, ty.floor() as usize);
        let t = TILES.iter().find(|t| t.0 == ix && t.1 == iy)?;
        let a = (tx - ix as f64) * 2.0 - 1.0;
        let b = 1.0 - (ty - iy as f64) * 2.0;
        let q = std::f64::consts::FRAC_PI_4;
        let [c, r, u] = [t.2, t.3, t.4].map(DVec3::from_array);
        Some((c + r * (a * q).tan() + u * (b * q).tan()).normalize())
    }
    fn back(&self, d: DVec3) -> Option<(f64, f64)> {
        let q = std::f64::consts::FRAC_PI_4;
        let t = TILES.iter().max_by(|a, b| d.dot(DVec3::from_array(a.2)).total_cmp(&d.dot(DVec3::from_array(b.2))))?;
        let [c, r, u] = [t.2, t.3, t.4].map(DVec3::from_array);
        let z = d.dot(c);
        let (a, b) = ((d.dot(r) / z).atan() / q, (d.dot(u) / z).atan() / q);
        let (tx, ty) = (t.0 as f64 + (a + 1.0) * 0.5, t.1 as f64 + (1.0 - b) * 0.5);
        Some((tx / 2.0 - 1.0, 1.0 - ty / 1.5))
    }
}

/// Draws a view of a layer, `w` x `h` pixels, rivers over it when the layer is Rivers or Height.
pub fn draw(b: &Bake, view: &dyn View, layer: Layer, w: usize, h: usize, sun: DVec3, ex: f64) -> Img {
    let px: Vec<[u8; 3]> = (0..w * h)
        .into_par_iter()
        .map(|k| {
            let (x, y) = ((k % w) as f64 + 0.5, (k / w) as f64 + 0.5);
            let (x, y) = (x / w as f64 * 2.0 - 1.0, 1.0 - y / h as f64 * 2.0);
            match view.dir(x, y) {
                Some(d) => colour(b, b.g.locate(d), layer, sun, ex),
                None => [0, 0, 0],
            }
        })
        .collect();
    let mut img = Img { w, h, px };
    if layer != Layer::Plates {
        let thr = b.rivers.iter().map(|r| r.area).fold(f32::MAX, f32::min);
        let deep = layer == Layer::Rivers;
        for r in &b.rivers {
            let (a, c) = (b.g.d(r.cell), b.g.d(r.to));
            let (Some(p), Some(q)) = (view.back(a), view.back(c)) else { continue };
            let big = r.area > 8.0 * thr;
            let col = if deep { if big { [120, 200, 255] } else { [50, 120, 230] } } else { [40, 90, 200] };
            // a short line from the cell to its receiver
            let to_px = |(x, y): (f64, f64)| ((x + 1.0) * 0.5 * w as f64, (1.0 - y) * 0.5 * h as f64);
            let ((x0, y0), (x1, y1)) = (to_px(p), to_px(q));
            let span = (x1 - x0).abs().max((y1 - y0).abs());
            // a step that jumps between the cross's tiles is not a river on the picture
            if span > 12.0 {
                continue;
            }
            let steps = (span.ceil() as usize).max(1);
            for s in 0..=steps {
                let t = s as f64 / steps as f64;
                let (x, y) = ((x0 + (x1 - x0) * t) as i64, (y0 + (y1 - y0) * t) as i64);
                img.put(x, y, col);
                if big && deep {
                    img.put(x + 1, y, col);
                }
            }
        }
    }
    img
}

/// The 8 corners and the middles of the 12 seams, as directions.
pub fn corners() -> Vec<DVec3> {
    let mut v = Vec::new();
    for x in [-1.0, 1.0] {
        for y in [-1.0, 1.0] {
            for z in [-1.0, 1.0] {
                v.push(DVec3::new(x, y, z).normalize());
            }
        }
    }
    v
}
pub fn seams() -> Vec<DVec3> {
    let mut v = Vec::new();
    for (a, b) in [(0, 1), (0, 2), (1, 2)] {
        for sa in [-1.0, 1.0] {
            for sb in [-1.0, 1.0] {
                let mut p = [0.0; 3];
                (p[a], p[b]) = (sa, sb);
                v.push(DVec3::from_array(p).normalize());
            }
        }
    }
    v
}

/// Heights along a great circle through `at` towards `toward`, `len` metres long: a section at true scale
/// times `ex` vertically, 1 px per `m_px` metres.
pub fn profile(b: &Bake, at: DVec3, toward: DVec3, len: f64, m_px: f64, ex: f64, hmin: f64, hmax: f64) -> Img {
    let w = (len / m_px) as usize;
    let h = (((hmax - hmin) * ex) / m_px).ceil() as usize + 4;
    let mut img = Img::new(w, h, [235, 235, 240]);
    let a = at.normalize();
    let t = (toward - a * toward.dot(a)).normalize();
    let y_of = |v: f64| (h as f64 - 2.0 - (v - hmin) * ex / m_px) as i64;
    for x in 0..w {
        let ang = (x as f64 * m_px - len / 2.0) / b.g.r;
        let d = a * ang.cos() + t * ang.sin();
        let c = b.g.locate(d) as usize;
        let (g, wl) = (b.h[c] as f64, b.w[c].max(0.0) as f64);
        let (yg, yw) = (y_of(g), y_of(wl.max(g)));
        for y in 0..h as i64 {
            let col = if y >= yg { [90, 75, 60] } else if y >= yw { [70, 130, 200] } else { continue };
            img.put(x as i64, y, col);
        }
        img.put(x as i64, y_of(0.0), [0, 0, 0]);
    }
    img
}
