//! The globe map (docs/design/navigation.md part 3, ADR 0077): a body's baked map as a latitude-longitude
//! picture, drawn on an orthographic globe the player turns and zooms; what the player has seen as a bitmap
//! on the same grid. The picture is made once on a worker (`image`); the globe's pixels are the UI shader's
//! (`overlay.wgsl`, glyph GLYPH), which reads the pictures, the masks and this frame's `View` from one
//! storage buffer laid out as `OFF`. Everything here is plain numbers, so the tests and the spike
//! (prototypes/globe-map) run it without a GPU.
use engine::bake::{grid::Grid, Map};
use glam::DVec3;
use std::f64::consts::PI;

/// The UI rectangle's glyph that the shader draws the globe into.
pub const GLYPH: u32 = 200;
/// Texels of each body's picture (Terra, Luna): about 24 m at Terra's equator, 10 m at Luna's.
pub const SIZE: [(usize, usize); 2] = [(1024, 512), (512, 256)];
/// u32s of the buffer's head: the view (`View::head`).
pub const HEAD: usize = 32;

/// Where each part starts in the buffer, u32s: the head, Terra's and Luna's pictures, Terra's and Luna's seen
/// bits; and the whole length.
pub const fn off(part: usize) -> usize {
    let t = SIZE[0].0 * SIZE[0].1;
    let l = SIZE[1].0 * SIZE[1].1;
    [0, HEAD, HEAD + t, HEAD + t + l, HEAD + t + l + t / 32, HEAD + t + l + t / 32 + l / 32][part]
}
pub const LEN: usize = off(5);

/// The unit direction of (lat, lon) in degrees on a body whose longitude 0 runs through `zero` (`nav::geo`'s
/// inverse).
pub fn dir(lat: f64, lon: f64, zero: DVec3) -> DVec3 {
    let (la, lo) = (lat.to_radians(), lon.to_radians() + zero.y.atan2(zero.x));
    DVec3::new(la.cos() * lo.cos(), la.cos() * lo.sin(), la.sin())
}

/// The texel of (lat, lon) on a w x h picture: row 0 at the north pole, column 0 at 180 W.
pub fn texel(lat: f64, lon: f64, (w, h): (usize, usize)) -> usize {
    let x = (((lon + 180.0) / 360.0 * w as f64).floor() as i64).rem_euclid(w as i64) as usize;
    let y = (((90.0 - lat) / 180.0 * h as f64).floor().max(0.0) as usize).min(h - 1);
    y * w + x
}

/// The (lat, lon) in degrees of texel (x, y)'s middle.
fn texel_geo(x: usize, y: usize, (w, h): (usize, usize)) -> (f64, f64) {
    (90.0 - (y as f64 + 0.5) / h as f64 * 180.0, (x as f64 + 0.5) / w as f64 * 360.0 - 180.0)
}

/// Luna's kinds (`bake::luna`, HIGHLAND .. SHADOW) in the picture's colours: pale highlands, dark maria.
const LUNA: [[u8; 3]; 7] = [[168, 166, 160], [82, 82, 88], [214, 212, 204], [118, 118, 124], [150, 120, 96], [160, 140, 110], [40, 40, 52]];
/// Mars's kinds (`bake::mars`, HIGHLAND .. FROST): rust highlands, ochre plains, dark lava, pale sediment, white ice.
const MARS: [[u8; 3]; 11] = [
    [150, 84, 52],
    [184, 112, 66],
    [104, 62, 44],
    [132, 70, 42],
    [120, 76, 56],
    [140, 96, 70],
    [164, 120, 86],
    [196, 160, 120],
    [176, 140, 100],
    [236, 232, 226],
    [214, 204, 200],
];

/// A body's picture, `size` texels, as RGBA8 in u32s (red the lowest byte): the ground by biome (Luna's by
/// kind), the sea by its depth, lakes and rivers blue, every land texel shaded by its slope to a light in
/// the north-west, so the relief reads. `zero` is where longitude 0 runs. Some tens of ms: a worker's.
pub fn image(map: &Map, zero: DVec3, size: (usize, usize)) -> Vec<u32> {
    let (w, h) = size;
    let m = map.m as usize;
    let grid = Grid::bare(m, 1.0);
    let cells: Vec<usize> = (0..w * h)
        .map(|k| {
            let (lat, lon) = texel_geo(k % w, k / w, size);
            grid.locate(dir(lat, lon, zero)) as usize
        })
        .collect();
    let height: Vec<f32> = cells.iter().map(|&c| map.h[c]).collect();
    let lake: std::collections::HashSet<u32> = map.lakes.iter().map(|l| l.0).collect();
    // an airless crust's map (Luna's, Mars's): its kinds' colours and relief below the datum too
    let (luna, mars) = (map.crust(), map.version == engine::bake::mars::VERSION);
    // a texel's metres east and north, for the slope: the radius from the columns (a metre each, 4 n round)
    let r = map.n as f64 * 2.0 / PI;
    let dy = PI * r / h as f64;
    let mut out: Vec<u32> = (0..w * h)
        .map(|k| {
            let (x, y) = (k % w, k / w);
            let (c, z) = (cells[k], height[k]);
            let mut rgb = if mars {
                MARS[(map.biome[c].saturating_sub(engine::bake::mars::HIGHLAND) as usize).min(10)].map(f32::from)
            } else if luna {
                LUNA[(map.biome[c].saturating_sub(engine::bake::luna::HIGHLAND) as usize).min(6)].map(f32::from)
            } else if map.biome[c] == engine::bake::life::ICE {
                // the polar caps over land, lake and sea alike (ADR 0107)
                engine::bake::life::BIOMES[0].1.map(f32::from)
            } else if lake.contains(&(c as u32)) {
                [60.0, 120.0, 190.0]
            } else if z < 0.0 {
                let t = (-z / 160.0).clamp(0.0, 1.0);
                [70.0 + (16.0 - 70.0) * t, 140.0 + (40.0 - 140.0) * t, 190.0 + (100.0 - 190.0) * t]
            } else {
                engine::bake::life::BIOMES[(map.biome[c] as usize).min(14)].1.map(f32::from)
            };
            if luna || z >= 0.0 {
                let lat = texel_geo(x, y, size).0.to_radians();
                let dx = (2.0 * PI * r * lat.cos() / w as f64).max(1.0);
                let at = |x: usize, y: usize| height[y.min(h - 1) * w + x % w].max(if luna { f32::MIN } else { 0.0 }) as f64;
                let east = (at(x + 1, y) - at(x + w - 1, y)) / (2.0 * dx);
                let north = (at(x, y.saturating_sub(1)) - at(x, y + 1)) / (2.0 * dy);
                // lit from the north-west and above: a slope facing it brightens, one facing away darkens
                let shade = (1.0 + if luna { 1.0 } else { 2.5 } * (north - east)).clamp(0.55, 1.35) as f32;
                rgb = rgb.map(|v| v * shade);
            }
            pack(rgb)
        })
        .collect();
    // rivers are a cell or two wide, narrower than a texel: each river cell paints the texel it lies in
    if !luna {
        for reach in &map.rivers {
            let (f, i, j) = grid.fij(reach.cell);
            let d = grid.dir_at(f, i as f64 + 0.5, j as f64 + 0.5);
            let (lat, lon) = crate::nav::geo(d, zero);
            out[texel(lat, lon, size)] = pack([50.0, 110.0, 200.0]);
        }
    }
    out
}

fn pack(rgb: [f32; 3]) -> u32 {
    let [r, g, b] = rgb.map(|v| v.clamp(0.0, 255.0) as u32);
    r | g << 8 | b << 16 | 255 << 24
}

/// What a player has seen of a body: a bit per texel of its picture, kept beside the save.
#[derive(Clone, PartialEq, Debug)]
pub struct Seen {
    pub size: (usize, usize),
    pub bits: Vec<u32>,
}

impl Seen {
    pub fn new(size: (usize, usize)) -> Seen {
        Seen { size, bits: vec![0; size.0 * size.1 / 32] }
    }
    pub fn get(&self, k: usize) -> bool {
        self.bits[k / 32] >> (k % 32) & 1 == 1
    }
    /// Marks every texel whose middle lies within `rho` radians of direction `d` (the meridian `zero`); true
    /// if any was new. A row's span comes from the spherical law of cosines, so a walk costs a few rows.
    pub fn mark(&mut self, d: DVec3, zero: DVec3, rho: f64) -> bool {
        let (w, h) = self.size;
        let (lat, lon) = crate::nav::geo(d.normalize(), zero);
        let (p1, cos) = (lat.to_radians(), rho.min(PI).cos());
        let row = |l: f64| (((90.0 - l) / 180.0 * h as f64).floor().max(0.0) as usize).min(h - 1);
        let mut new = false;
        for y in row((lat + rho.to_degrees()).min(90.0))..=row((lat - rho.to_degrees()).max(-90.0)) {
            let p2 = texel_geo(0, y, self.size).0.to_radians();
            // cos(distance) = sin p1 sin p2 + cos p1 cos p2 cos(dlon) >= cos(rho)
            let c = (cos - p1.sin() * p2.sin()) / (p1.cos() * p2.cos()).max(1e-12);
            if c > 1.0 {
                continue;
            }
            let half = if c < -1.0 { 180.0 } else { c.acos().to_degrees() };
            // the texels whose middles fall in lon - half ..= lon + half
            let first = ((lon - half + 180.0) / 360.0 * w as f64 - 0.5).ceil() as i64;
            let last = ((lon + half + 180.0) / 360.0 * w as f64 - 0.5).floor() as i64;
            for x in first..=last.min(first + w as i64 - 1) {
                let k = y * w + x.rem_euclid(w as i64) as usize;
                new |= !self.get(k);
                self.bits[k / 32] |= 1 << (k % 32);
            }
        }
        new
    }
    pub fn to_bytes(&self) -> Vec<u8> {
        self.bits.iter().flat_map(|b| b.to_le_bytes()).collect()
    }
    /// The bits from `to_bytes`; a file of another length (another picture size) reads as nothing seen.
    pub fn from_bytes(size: (usize, usize), b: &[u8]) -> Seen {
        let mut s = Seen::new(size);
        if b.len() == s.bits.len() * 4 {
            s.bits = b.chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        }
        s
    }
}

/// How far a player sees over the ground from `over` metres above it on a body of radius `r`: the angle to
/// its horizon (a 2 m eye at the least, about 180 m on Terra).
pub fn horizon(r: f64, over: f64) -> f64 {
    (r / (r + over.max(2.0))).acos()
}

/// The globe on screen: its middle and radius in pixels, and the (lat, lon) turned to face the viewer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub cx: f64,
    pub cy: f64,
    pub r: f64,
    pub lat: f64,
    pub lon: f64,
}

impl View {
    /// East, north and towards the viewer at the globe's middle, in the body's frame.
    fn axes(&self, zero: DVec3) -> [DVec3; 3] {
        let d = dir(self.lat, self.lon, zero);
        let lo = self.lon.to_radians() + zero.y.atan2(zero.x);
        let e = DVec3::new(-lo.sin(), lo.cos(), 0.0);
        [e, d.cross(e), d]
    }
    /// The body's direction under screen pixel (x, y), None off the globe.
    pub fn pick(&self, x: f64, y: f64, zero: DVec3) -> Option<DVec3> {
        let (p, q) = ((x - self.cx) / self.r, (self.cy - y) / self.r);
        let s = p * p + q * q;
        (s <= 1.0).then(|| {
            let [e, n, d] = self.axes(zero);
            e * p + n * q + d * (1.0 - s).sqrt()
        })
    }
    /// Where direction `d` shows on screen, None on the far side.
    pub fn project(&self, d: DVec3, zero: DVec3) -> Option<(f64, f64)> {
        let [e, n, f] = self.axes(zero);
        let d = d.normalize();
        (d.dot(f) >= 0.0).then(|| (self.cx + d.dot(e) * self.r, self.cy - d.dot(n) * self.r))
    }
    /// Dragged by (dx, dy) pixels: the ground under the pointer follows it; north stays up.
    pub fn drag(&mut self, dx: f64, dy: f64) {
        let k = 1.0 / self.r * 180.0 / PI;
        self.lon = (self.lon - dx * k + 540.0).rem_euclid(360.0) - 180.0;
        self.lat = (self.lat + dy * k).clamp(-89.0, 89.0);
    }
    /// The buffer's head for the shader (overlay.wgsl `globe_px`), floats as bits, in UI pixels: the middle,
    /// radius and pixel size; east, north and towards the viewer; the sun's direction; the body (0 Terra, 1
    /// Luna); whether the unseen shows (creative); the meridian's angle; the UI's width (the shader scales by
    /// the frame's over it).
    pub fn head(&self, zero: DVec3, px: f64, sun: DVec3, body: usize, all: bool, ui_w: f64) -> [u32; HEAD] {
        let [e, n, d] = self.axes(zero);
        let mut f = [0f32; HEAD];
        f[..4].copy_from_slice(&[self.cx as f32, self.cy as f32, self.r as f32, px as f32]);
        for (k, v) in [e, n, d, sun.normalize_or_zero()].iter().enumerate() {
            f[4 + 4 * k..7 + 4 * k].copy_from_slice(&v.as_vec3().to_array());
        }
        f[20..24].copy_from_slice(&[body as f32, all as u8 as f32, zero.y.atan2(zero.x) as f32, ui_w as f32]);
        f.map(f32::to_bits)
    }
}

/// The globe in the game (`globe_ui.rs`): the worker's pictures until they are taken, what was seen and its
/// file, the view and the body shown, a drag under way.
pub struct State {
    pub pictures: Option<std::sync::mpsc::Receiver<Vec<u32>>>,
    pub seen: [Seen; 2],
    /// The masks changed since the GPU had them, and since the file had them.
    pub dirty: (bool, bool),
    /// When and where the last marking was.
    pub marked: Option<(std::time::Instant, DVec3)>,
    pub view: View,
    /// How much closer than the whole globe (1 to `ZOOM_MAX`).
    pub zoom: f64,
    pub body: usize,
    /// Where a press on the globe began, and whether it has moved since (a drag, not a click).
    pub drag: Option<((f32, f32), bool)>,
    pub file: Option<String>,
    /// Each body's meridian (`App::meridian`).
    pub zeros: [DVec3; 2],
    /// When the masks were last written to their file.
    pub saved: std::time::Instant,
    /// Every player the server lists, with where they are (`ToClient::Players`), and this player's id in it.
    pub others: Vec<engine::net::Player>,
    pub id: u32,
}

impl State {
    /// Starts the pictures of `maps` (Terra's, Luna's; none for a body with no map) on a worker and reads
    /// what was seen from `file`.
    pub fn new(maps: [Option<std::sync::Arc<Map>>; 2], zeros: [DVec3; 2], file: Option<String>) -> State {
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = std::thread::Builder::new().name("globe pictures".into()).spawn(move || {
            let mut all = vec![0u32; off(3) - off(1)];
            for (b, map) in maps.iter().enumerate() {
                if let Some(m) = map {
                    let at = off(1 + b) - off(1);
                    all[at..at + SIZE[b].0 * SIZE[b].1].copy_from_slice(&image(m, zeros[b], SIZE[b]));
                }
            }
            let _ = tx.send(all);
        });
        let bytes = file.as_deref().and_then(|f| std::fs::read(f).ok()).unwrap_or_default();
        let cut = SIZE[0].0 * SIZE[0].1 / 8;
        let seen = [Seen::from_bytes(SIZE[0], bytes.get(..cut).unwrap_or(&[])), Seen::from_bytes(SIZE[1], bytes.get(cut..).unwrap_or(&[]))];
        let view = View { cx: 0.0, cy: 0.0, r: 1.0, lat: 0.0, lon: 0.0 };
        State { pictures: Some(rx), seen, dirty: (true, false), marked: None, view, zoom: 1.0, body: 0, drag: None, file, zeros, saved: std::time::Instant::now(), others: Vec::new(), id: 0 }
    }
    /// Writes what was seen to its file when it changed; an error is the caller's to tell.
    pub fn save(&mut self) -> std::io::Result<()> {
        if let (Some(f), true) = (&self.file, self.dirty.1) {
            // a new file renamed over the old: a crash while writing never leaves a short one (read as nothing seen)
            let tmp = format!("{f}.tmp");
            std::fs::write(&tmp, [self.seen[0].to_bytes(), self.seen[1].to_bytes()].concat())?;
            std::fs::rename(&tmp, f)?;
            self.dirty.1 = false;
        }
        Ok(())
    }
}

/// The radius at which the whole globe fits a screen of height h, pixels.
pub fn fit(h: f64) -> f64 {
    h * 0.4
}
/// How far the wheel zooms: from the whole globe to 64 times closer.
pub const ZOOM_MAX: f64 = 64.0;

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_2;

    #[test]
    fn a_direction_comes_back_through_its_latitude_and_longitude() {
        let zero = DVec3::new(0.3, 0.9, 0.1);
        for (lat, lon) in [(0.0, 0.0), (12.4, 48.1), (-60.0, -170.0), (89.0, 179.0)] {
            let (a, b) = crate::nav::geo(dir(lat, lon, zero), zero);
            assert!((a - lat).abs() < 1e-9 && (b - lon).abs() < 1e-9, "{lat} {lon}: {a} {b}");
        }
    }

    #[test]
    fn the_globe_projects_and_picks_the_same_point() {
        let zero = DVec3::Y;
        let v = View { cx: 500.0, cy: 400.0, r: 300.0, lat: 30.0, lon: -40.0 };
        // the middle is the view's own place, north up the screen
        let (a, b) = crate::nav::geo(v.pick(500.0, 400.0, zero).unwrap(), zero);
        assert!((a - 30.0).abs() < 1e-9 && (b + 40.0).abs() < 1e-9);
        assert!(crate::nav::geo(v.pick(500.0, 300.0, zero).unwrap(), zero).0 > 30.0, "up the screen is north");
        assert!(crate::nav::geo(v.pick(600.0, 400.0, zero).unwrap(), zero).1 > -40.0, "right is east");
        assert!(v.pick(500.0, 99.0, zero).is_none(), "off the disc");
        for (x, y) in [(510.0, 420.0), (700.0, 250.0), (320.0, 600.0)] {
            let (px, py) = v.project(v.pick(x, y, zero).unwrap(), zero).unwrap();
            assert!((px - x).abs() < 1e-6 && (py - y).abs() < 1e-6);
        }
        assert!(v.project(-dir(30.0, -40.0, zero), zero).is_none(), "the far side is hidden");
    }

    #[test]
    fn a_drag_moves_the_ground_with_the_pointer() {
        let zero = DVec3::Y;
        let mut v = View { cx: 0.0, cy: 0.0, r: 100.0, lat: 0.0, lon: 0.0 };
        let under = v.pick(10.0, 0.0, zero).unwrap();
        v.drag(-10.0, 0.0);
        let (x, _) = v.project(under, zero).unwrap();
        assert!(x.abs() < 0.1, "the place that was under the pointer followed it: {x}");
    }

    #[test]
    fn seeing_marks_a_cap_and_no_more() {
        let zero = DVec3::Y;
        let mut s = Seen::new((256, 128));
        let d = dir(20.0, 10.0, zero);
        assert!(s.mark(d, zero, 10f64.to_radians()));
        assert!(!s.mark(d, zero, 10f64.to_radians()), "nothing new the second time");
        assert!(s.get(texel(20.0, 10.0, s.size)) && s.get(texel(28.0, 10.0, s.size)) && s.get(texel(20.0, 19.0, s.size)));
        assert!(!s.get(texel(32.0, 10.0, s.size)) && !s.get(texel(20.0, 25.0, s.size)) && !s.get(texel(-20.0, 10.0, s.size)));
        // across the date line and over a pole too
        s.mark(dir(85.0, 179.0, zero), zero, 10f64.to_radians());
        assert!(s.get(texel(89.9, 0.0, s.size)) && s.get(texel(82.0, -178.0, s.size)));
        assert_eq!(Seen::from_bytes(s.size, &s.to_bytes()), s);
        assert_eq!(Seen::from_bytes(s.size, &[1, 2, 3]), Seen::new(s.size), "a file of another size reads as nothing seen");
    }

    #[test]
    fn a_rows_span_marks_what_the_texel_by_texel_test_marks() {
        // the slow reference: every texel's middle against the cap
        let zero = DVec3::new(0.2, 0.9, 0.0);
        for (lat, lon, rho) in [(0.0, 0.0, 0.05), (40.0, 170.0, 0.3), (-80.0, -20.0, 0.4), (10.0, 60.0, 1.4), (5.0, 5.0, 3.0)] {
            let (d, mut s) = (dir(lat, lon, zero), Seen::new((128, 64)));
            s.mark(d, zero, rho);
            let mut slow = Seen::new(s.size);
            for k in 0..128 * 64 {
                let (a, b) = texel_geo(k % 128, k / 128, s.size);
                if dir(a, b, zero).dot(d) >= rho.cos() {
                    slow.bits[k / 32] |= 1 << (k % 32);
                }
            }
            let diff: u32 = s.bits.iter().zip(&slow.bits).map(|(a, b)| (a ^ b).count_ones()).sum();
            assert!(diff <= 2, "{lat} {lon} {rho}: {diff} texels differ (the edge's rounding)");
        }
    }

    #[test]
    fn the_buffer_is_laid_out_as_the_shader_reads_it() {
        // overlay.wgsl globe_px writes these offsets as numbers
        assert_eq!([off(1), off(2), off(3), off(4)], [32, 32 + 1024 * 512, 32 + 1024 * 512 + 512 * 256, 32 + 1024 * 512 + 512 * 256 + 1024 * 512 / 32]);
        assert_eq!(HEAD, 32);
    }

    #[test]
    fn what_was_seen_comes_back_from_its_file() {
        let f = std::env::temp_dir().join(format!("technoape-seen-{}", std::process::id()));
        let file = Some(f.to_string_lossy().into_owned());
        let mut s = State::new([None, None], [DVec3::Y, -DVec3::X], file.clone());
        s.seen[1].mark(DVec3::X, -DVec3::X, 0.2);
        s.dirty.1 = true;
        s.save().unwrap();
        let back = State::new([None, None], [DVec3::Y, -DVec3::X], file);
        assert_eq!(back.seen, s.seen);
        assert!(back.seen[1].bits.iter().any(|b| *b != 0) && back.seen[0].bits.iter().all(|b| *b == 0));
        let _ = std::fs::remove_file(f);
    }

    #[test]
    fn the_horizon_grows_with_height() {
        let r = 3900.0;
        assert!((horizon(r, 2.0) * r - 125.0).abs() < 5.0, "{}", horizon(r, 2.0) * r);
        assert!(horizon(r, 100.0) > horizon(r, 10.0));
        assert!(horizon(r, 1e7) > 1.5 && horizon(r, 1e7) < FRAC_PI_2);
    }

    #[test]
    fn the_picture_shows_land_and_sea_where_the_map_has_them() {
        // a tiny map: one face's cells over the sea, the rest under it
        let m = 8;
        let mut map = Map { version: engine::bake::VERSION, n: 64, m, h: vec![-100.0; 6 * 64], biome: vec![13; 6 * 64], ..Map::empty() };
        // face 2 is +y, where zero's meridian runs: land there
        for c in 2 * 64..3 * 64 {
            (map.h[c], map.biome[c]) = (50.0, 4);
        }
        let size = (64, 32);
        let img = image(&map, DVec3::Y, size);
        let rgb = |lat, lon| {
            let v = img[texel(lat, lon, size)];
            [v & 255, v >> 8 & 255, v >> 16 & 255]
        };
        let (land, sea) = (rgb(0.0, 0.0), rgb(0.0, 180.0));
        assert!(land[1] > land[2], "grassland is green: {land:?}");
        assert!(sea[2] > sea[0] && sea[2] > sea[1], "the deep sea is blue: {sea:?}");
        assert_eq!(img.len(), size.0 * size.1);
    }

    /// A Mars map (`bake::mars`) is drawn in Mars's colours, not taken for Luna's: its dust plains and highlands
    /// red, its polar ice white, every kind its own colour.
    #[test]
    fn a_mars_map_has_mars_colours() {
        use engine::bake::mars;
        let m = 8;
        let colour = |kind: u8| {
            let map = Map { version: mars::VERSION, n: 64, m, h: vec![0.0; 6 * 64], biome: vec![kind; 6 * 64], ..Map::empty() };
            let v = image(&map, DVec3::Y, (16, 8))[texel(0.0, 0.0, (16, 8))];
            [v & 255, v >> 8 & 255, v >> 16 & 255]
        };
        for k in [mars::HIGHLAND, mars::PLAIN, mars::LAVA, mars::SHIELD] {
            let c = colour(k);
            assert!(c[0] > c[1] && c[1] >= c[2] && c[0] > c[2] + 25, "{}: red-brown, {c:?}", mars::KINDS[(k - mars::HIGHLAND) as usize]);
        }
        let ice = colour(mars::CAP);
        assert!(ice.iter().all(|&v| v > 180), "the cap is white: {ice:?}");
        let all: std::collections::HashSet<[u32; 3]> = (mars::HIGHLAND..=mars::FROST).map(colour).collect();
        assert_eq!(all.len(), mars::KINDS.len(), "every kind its own colour");
    }
}
