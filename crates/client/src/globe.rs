//! The globe map (docs/design/navigation.md part 3, ADR 0077): a body's baked map as a latitude-longitude
//! picture, drawn on an orthographic globe the player turns and zooms; what the player has seen as a bitmap
//! on the same grid. The picture is made once on a worker (`image`); the globe's pixels are the UI shader's
//! (`overlay.wgsl`, glyph GLYPH), which reads the pictures, the masks and this frame's `View` from one
//! storage buffer laid out as `OFF`. Everything here is plain numbers, so the tests and the spike
//! (prototypes/globe-map) run it without a GPU.
use engine::bake::{grid::Grid, Map};
use engine::world::{Block, World};
use glam::DVec3;
use std::f64::consts::PI;

/// The UI rectangle's glyph that the shader draws the globe into.
pub const GLYPH: u32 = 200;
/// u32s of the buffer's head: the view (`View::head`).
pub const HEAD: usize = 32;

/// A body's picture in the buffer: where its texels start (u32s), how many, and where its seen bits start.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slot {
    pub pic: usize,
    pub size: (usize, usize),
    pub mask: usize,
}

/// A body's picture size by its row: one with a bake (a map) has one, a planet's 1024 x 512 (about 24 m a texel
/// on Terra), a smaller body's 512 x 256 (10 m on Luna); a gas body none.
pub fn size_of(row: &engine::bodies::Row) -> Option<(usize, usize)> {
    row.bake.as_ref().map(|_| if row.n >= 3000 { (1024, 512) } else { (512, 256) })
}

/// Where each row's picture and seen bits lie in the buffer: the head, every picture in row order, then every
/// mask in row order (one run, written at once; Terra's and Luna's where they were); and the length, u32s.
pub fn layout(rows: &[engine::bodies::Row]) -> (Vec<Option<Slot>>, usize) {
    let sizes: Vec<Option<(usize, usize)>> = rows.iter().map(size_of).collect();
    let mut at = HEAD;
    let mut take = |n: usize| {
        at += n;
        at - n
    };
    let pics: Vec<usize> = sizes.iter().map(|s| take(s.map_or(0, |(w, h)| w * h))).collect();
    let slots = sizes.iter().zip(pics).map(|(s, pic)| s.map(|size| Slot { pic, size, mask: take(size.0 * size.1 / 32) })).collect();
    (slots, take(0))
}

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
/// The giant's icy moon's kinds (`bake::icy`, PLAIN .. FROST): dark plains, bright fresh ice, pale palimpsests, frost.
const ICY: [[u8; 3]; 4] = [[78, 70, 62], [196, 214, 230], [150, 140, 130], [228, 226, 224]];
/// Mars's kinds (`bake::mars`, HIGHLAND .. FROST): rust highlands, ochre plains, dark lava, pale sediment, white ice.
const MARS: [[u8; 3]; 11] = [
    [150, 84, 52],
    [184, 112, 66],
    [66, 57, 52],
    [74, 62, 55],
    [120, 76, 56],
    [140, 96, 70],
    [164, 120, 86],
    [196, 160, 120],
    [176, 140, 100],
    [236, 232, 226],
    [214, 204, 200],
];

/// A body's picture, `size` texels, as u32s (`pack`: red the lowest byte, the height the top one): the ground by
/// biome (Luna's by kind), the sea by its depth, lakes and rivers blue; and how far its relief stands out, a share
/// of the radius (`lift`) up to `cap` (`relief_max`), the sea a level surface. `zero` is where longitude 0 runs.
/// Some tens of ms: a worker's.
pub fn image(map: &Map, zero: DVec3, size: (usize, usize), cap: f64) -> (Vec<u32>, f64) {
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
    let (luna, mars, icy) = (map.crust(), map.version == engine::bake::mars::VERSION, map.version == engine::bake::icy::VERSION);
    // the ground's height a texel, the sea's level over a sea: its span over the body's radius (the columns, a metre
    // each, 4 n round) is how far the globe's surface stands out (`RELIEF`)
    let ground: Vec<f32> = height.iter().map(|&z| if luna { z } else { z.max(0.0) }).collect();
    let (lo, hi) = ground.iter().fold((f32::MAX, f32::MIN), |(a, b), &z| (a.min(z), b.max(z)));
    let span = (hi - lo).max(1e-3);
    let r = map.n as f64 * 2.0 / PI;
    let relief = (RELIEF * span as f64 / r).min(cap);
    // a texel's metres north, for the slope
    let dy = PI * r / h as f64;
    let mut out: Vec<u32> = (0..w * h)
        .map(|k| {
            let (x, y) = (k % w, k / w);
            let (c, z) = (cells[k], height[k]);
            let mut rgb = if icy {
                ICY[(map.biome[c].saturating_sub(engine::bake::icy::PLAIN) as usize).min(3)].map(f32::from)
            } else if mars {
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
            // every slope shaded to a light in the north-west, so the small relief reads under the real sun's (the
            // shader's, on the coarser height it stands out by)
            if luna || z >= 0.0 {
                let lat = texel_geo(x, y, size).0.to_radians();
                let dx = (2.0 * PI * r * lat.cos() / w as f64).max(1.0);
                let at = |x: usize, y: usize| ground[y.min(h - 1) * w + x % w] as f64;
                let east = (at(x + 1, y) - at(x + w - 1, y)) / (2.0 * dx);
                let north = (at(x, y.saturating_sub(1)) - at(x, y + 1)) / (2.0 * dy);
                let shade = (1.0 + if luna { 1.0 } else { 2.5 } * (north - east)).clamp(0.55, 1.35) as f32;
                rgb = rgb.map(|v| v * shade);
            }
            pack(rgb, 1 + ((ground[k] - lo) / span * 254.0).round() as u32)
        })
        .collect();
    // rivers are a cell or two wide, narrower than a texel: each river cell paints the texel it lies in
    if !luna {
        for reach in &map.rivers {
            let (f, i, j) = grid.fij(reach.cell);
            let d = grid.dir_at(f, i as f64 + 0.5, j as f64 + 0.5);
            let (lat, lon) = crate::nav::geo(d, zero);
            let k = texel(lat, lon, size);
            out[k] = pack([50.0, 110.0, 200.0], out[k] >> 24);
        }
    }
    (out, relief)
}

/// How much the globe's relief is stretched over the body's own (heights over radius): the map's taste knob (ADR 0201,
/// "The map zooms out to the system"), with the most each body's may stand out (`relief_max`).
pub const RELIEF: f64 = 1.0;
/// The most a body's globe relief stands out, a share of its radius, by name: the one table of the taste knob
/// (Terra and Luna higher so they read as small worlds, not painted balls; Mars's dichotomy reads as a potato over 2.5 %).
pub fn relief_max(name: &str) -> f64 {
    match name {
        "Terra" => 0.06,
        "Luna" => 0.05,
        _ => 0.025,
    }
}

/// A texel: its colour in the low bytes, its height in the top one (1 the lowest of the body, 255 the highest; 0 no
/// picture).
fn pack(rgb: [f32; 3], height: u32) -> u32 {
    let [r, g, b] = rgb.map(|v| v.clamp(0.0, 255.0) as u32);
    r | g << 8 | b << 16 | height.clamp(1, 255) << 24
}

/// How far out a texel's ground stands, a share of the globe's radius: 1 at the body's highest, 1 - relief at its
/// lowest (so the globe never leaves its disc).
pub fn lift(texel: u32, relief: f64) -> f64 {
    1.0 - relief + relief * ((texel >> 24) as f64 - 1.0) / 254.0
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
    pub fn axes(&self, zero: DVec3) -> [DVec3; 3] {
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
    /// Where direction `d` shows on screen with its ground `lift` out (`State::lift`), None on the far side.
    pub fn project_at(&self, d: DVec3, zero: DVec3, lift: f64) -> Option<(f64, f64)> {
        let (x, y) = self.project(d, zero)?;
        Some((self.cx + (x - self.cx) * lift, self.cy + (y - self.cy) * lift))
    }
    /// The buffer's head for the shader (overlay.wgsl `globe_px`), floats as bits, in UI pixels: the middle,
    /// radius and pixel size; east, north and towards the viewer; the sun's direction; whether the body has air
    /// (a rim of sky); whether the unseen shows (creative); the meridian's angle; the UI's width (the shader scales
    /// by the frame's over it); then as plain u32s the body's picture: where it starts, its width and where its
    /// seen bits start (`Slot`); then its relief (`lift`), a float.
    #[allow(clippy::too_many_arguments)]
    pub fn head(&self, zero: DVec3, px: f64, sun: DVec3, slot: Slot, all: bool, ui_w: f64, air: bool, relief: f64) -> [u32; HEAD] {
        let [e, n, d] = self.axes(zero);
        let mut f = [0f32; HEAD];
        f[..4].copy_from_slice(&[self.cx as f32, self.cy as f32, self.r as f32, px as f32]);
        for (k, v) in [e, n, d, sun.normalize_or_zero()].iter().enumerate() {
            f[4 + 4 * k..7 + 4 * k].copy_from_slice(&v.as_vec3().to_array());
        }
        f[20..24].copy_from_slice(&[air as u8 as f32, all as u8 as f32, zero.y.atan2(zero.x) as f32, ui_w as f32]);
        f[27] = relief as f32;
        let mut h = f.map(f32::to_bits);
        h[24..27].copy_from_slice(&[slot.pic as u32, slot.size.0 as u32, slot.mask as u32]);
        h
    }
}

/// Steps the march takes through a chord of `chord` radii on a picture `width` texels round: two a texel, so a
/// one-texel ridge is never stepped over (between 8 and 192). The shader's rule (overlay.wgsl `globe_px`) too.
pub fn march_steps(chord: f64, width: usize) -> usize {
    (chord * width as f64 / PI).ceil().clamp(8.0, 192.0) as usize
}

/// A column's key on the map: its body, face and column.
pub type Column = (u8, u8, i32, i32);
/// The edited columns are kept by tiles of `TILE` x `TILE` columns (its body, face and tile), so the screen's
/// share of them is found without walking the rest (`State::visible`).
pub type Tile = (u8, u8, i32, i32);
pub const TILE: i32 = 64;
fn tile_of(k: Column) -> Tile {
    (k.0, k.1, k.2.div_euclid(TILE), k.3.div_euclid(TILE))
}

/// The globe in the game (`globe_ui.rs`): each body's picture as the workers make it, what was seen and its
/// file, the view and the body focused, a drag under way, the orbits, and what building and digging made of
/// the columns the client was told of.
pub struct State {
    /// Each row's place in the buffer (`layout`).
    pub slots: Vec<Option<Slot>>,
    /// The workers' finished pictures, by body; and per body whether one was asked for and whether it came.
    pictures: (std::sync::mpsc::Sender<(usize, (Vec<u32>, f64))>, std::sync::mpsc::Receiver<(usize, (Vec<u32>, f64))>),
    pub asked: Vec<bool>,
    pub got: Vec<bool>,
    /// Each body's heights as its picture has them (the top byte, `lift`) and its relief, for the marks to sit on
    /// the ground the shader draws.
    pub heights: Vec<Vec<u8>>,
    pub relief: Vec<f64>,
    /// What was seen of each body with a picture.
    pub seen: Vec<Option<Seen>>,
    /// The masks changed since the GPU had them, and since the file had them.
    pub dirty: (bool, bool),
    /// When and where the last marking was.
    pub marked: Option<(std::time::Instant, DVec3)>,
    pub view: View,
    /// How close: 1 is the whole globe of the focused body, up to `ZOOM_MAX` closer; under 1 the system around
    /// it (`system`), down to `least`.
    pub zoom: f64,
    /// The focused body.
    pub body: usize,
    /// Where a press on the globe began, and whether it has moved since (a drag, not a click).
    pub drag: Option<((f32, f32), bool)>,
    pub file: Option<String>,
    /// Each body's meridian (`App::meridian`).
    pub zeros: Vec<DVec3>,
    /// When the masks were last written to their file.
    pub saved: std::time::Instant,
    /// Every player the server lists, with where they are (`ToClient::Players`), and this player's id in it.
    pub others: Vec<engine::net::Player>,
    pub id: u32,
    /// The columns an edit changed (`top`) by tile, and those touched since, with the highest layer touched; and
    /// by body those touched while the body was not loaded, worked out when it is.
    pub tops: std::collections::BTreeMap<Tile, std::collections::BTreeMap<(i32, i32), Top>>,
    pub touched: std::collections::BTreeMap<Column, i32>,
    pub waiting: Vec<std::collections::BTreeMap<Column, i32>>,
    /// Each body's most relief (`relief_max`).
    pub caps: Vec<f64>,
}

impl State {
    /// A map of `rows`' bodies with these meridians, what was seen read from `file` (one mask after another in
    /// row order, so a file of Terra's and Luna's from before reads as it did). Pictures start in `ask`.
    pub fn new(rows: &[engine::bodies::Row], zeros: Vec<DVec3>, file: Option<String>) -> State {
        let slots = layout(rows).0;
        let bytes = file.as_deref().and_then(|f| std::fs::read(f).ok()).unwrap_or_default();
        let mut at = 0;
        let seen = slots
            .iter()
            .map(|s| {
                s.map(|s| {
                    let n = s.size.0 * s.size.1 / 8;
                    at += n;
                    Seen::from_bytes(s.size, bytes.get(at - n..at).unwrap_or(&[]))
                })
            })
            .collect();
        let view = View { cx: 0.0, cy: 0.0, r: 1.0, lat: 0.0, lon: 0.0 };
        let n = rows.len();
        State {
            slots,
            pictures: std::sync::mpsc::channel(),
            asked: vec![false; n],
            got: vec![false; n],
            heights: vec![Vec::new(); n],
            relief: vec![0.0; n],
            seen,
            dirty: (true, false),
            marked: None,
            view,
            zoom: 1.0,
            body: 0,
            drag: None,
            file,
            zeros,
            saved: std::time::Instant::now(),
            others: Vec::new(),
            id: 0,
            tops: Default::default(),
            touched: Default::default(),
            waiting: vec![Default::default(); n],
            caps: rows.iter().map(|r| relief_max(&r.name)).collect(),
        }
    }
    /// Starts body `b`'s picture of `map` on a worker of its own (tens of ms), once.
    pub fn ask(&mut self, b: usize, map: std::sync::Arc<Map>) {
        self.ask_with(b, move || Some(map));
    }
    /// `ask` with the map still to get on the worker (a body not loaded: its bake from memory or disk, or baked).
    pub fn ask_with(&mut self, b: usize, map: impl FnOnce() -> Option<std::sync::Arc<Map>> + Send + 'static) {
        let (Some(Some(slot)), false) = (self.slots.get(b), self.asked[b]) else { return };
        let (size, zero, cap, tx) = (slot.size, self.zeros[b], self.caps[b], self.pictures.0.clone());
        self.asked[b] = true;
        let _ = std::thread::Builder::new().name("globe picture".into()).spawn(move || {
            if let Some(map) = map() {
                let _ = tx.send((b, image(&map, zero, size, cap)));
            }
        });
    }
    /// The direction on body b's globe under pixel (x, y) of view v: where the ray meets its ground as the shader
    /// marches it (`lift`, `march_steps`), so a click is where the mark under it is drawn; None off the globe.
    pub fn pick(&self, b: usize, v: &View, x: f64, y: f64) -> Option<DVec3> {
        let zero = self.zeros[b];
        let rel = match (self.slots.get(b).copied().flatten(), self.heights.get(b).is_some_and(|h| !h.is_empty())) {
            (Some(s), true) if self.relief[b] > 0.0 => (self.relief[b], s.size.0),
            _ => return v.pick(x, y, zero),
        };
        let (p, q) = ((x - v.cx) / v.r, (v.cy - y) / v.r);
        let s = p * p + q * q;
        if s > 1.0 {
            return None;
        }
        let [e, n, d] = v.axes(zero);
        let at = |t: f64| e * p + n * q + d * t;
        let under = |t: f64| at(t).length() <= self.lift(b, at(t));
        let t0 = (1.0 - s).sqrt();
        let inner = (1.0 - rel.0) * (1.0 - rel.0);
        let t1 = if s < inner { (inner - s).sqrt() } else { -t0 };
        let k = march_steps(t0 - t1, rel.1);
        let step = (t0 - t1) / k as f64;
        let (mut above, mut below) = match (0..=k).map(|i| t0 - step * i as f64).find(|&t| under(t)) {
            Some(t) => ((t + step).min(t0), t),
            // a ray past the lowest ground's sphere misses; one that reaches it meets it there (the sea's level)
            None if s >= inner => return None,
            None => (t1, t1),
        };
        for _ in 0..6 {
            let t = 0.5 * (above + below);
            if under(t) {
                below = t;
            } else {
                above = t;
            }
        }
        Some(at(below).normalize())
    }
    /// A finished picture, if one came: its body and texels.
    pub fn take(&mut self) -> Option<(usize, Vec<u32>)> {
        let (b, (p, relief)) = self.pictures.1.try_recv().ok()?;
        (self.got[b], self.relief[b], self.heights[b]) = (true, relief, p.iter().map(|t| (t >> 24) as u8).collect());
        Some((b, p))
    }
    /// How far out the ground in direction `d` stands on body b's globe, a share of its radius (`lift`; 1 before
    /// its picture came): where a mark sits.
    pub fn lift(&self, b: usize, d: DVec3) -> f64 {
        match (self.slots.get(b).copied().flatten(), self.heights.get(b).filter(|h| !h.is_empty())) {
            (Some(s), Some(h)) => {
                let (lat, lon) = crate::nav::geo(d.normalize(), self.zeros[b]);
                lift((h[texel(lat, lon, s.size)] as u32) << 24, self.relief[b])
            }
            _ => 1.0,
        }
    }
    /// Where a mark at `rel` from body b's centre is drawn in view v: on its ground (`lift`); what a click on it is
    /// tested against, so the two never part.
    pub fn at(&self, b: usize, v: &View, rel: DVec3) -> Option<(f64, f64)> {
        v.project_at(rel, self.zeros[b], self.lift(b, rel))
    }
    /// Every mask, one after another as the buffer and the file hold them.
    pub fn masks(&self) -> Vec<u32> {
        self.seen.iter().flatten().flat_map(|s| s.bits.iter().copied()).collect()
    }
    /// Writes what was seen to its file when it changed; an error is the caller's to tell.
    pub fn save(&mut self) -> std::io::Result<()> {
        if let (Some(f), true) = (&self.file, self.dirty.1) {
            // a new file renamed over the old: a crash while writing never leaves a short one (read as nothing seen)
            let tmp = format!("{f}.tmp");
            std::fs::write(&tmp, self.seen.iter().flatten().flat_map(Seen::to_bytes).collect::<Vec<u8>>())?;
            std::fs::rename(&tmp, f)?;
            self.dirty.1 = false;
        }
        Ok(())
    }
    /// Cell `b` was edited (or its edit undone): its column is worked out again (`refresh`).
    pub fn touch(&mut self, b: Block) {
        let z = self.touched.entry((b.body, b.face, b.pos.x, b.pos.y)).or_insert(b.pos.z);
        *z = (*z).max(b.pos.z);
    }
    /// Works out up to `n` touched columns' tops against `w` (each a walk down a few cells): the frame's budget,
    /// so an edit never costs more than that whatever the server sends at once.
    pub fn refresh(&mut self, w: &World, n: usize) {
        // a body loaded since: what was touched on it while it was not
        for (b, waiting) in self.waiting.iter_mut().enumerate() {
            if !waiting.is_empty() && w.loaded(b) {
                self.touched.append(waiting);
            }
        }
        for _ in 0..n {
            let Some((k, z)) = self.touched.pop_first() else { return };
            let (body, face, i, j) = k;
            if !w.loaded(body as usize) {
                let at = self.waiting[body as usize].entry(k).or_insert(z);
                *at = (*at).max(z);
                continue;
            }
            let from = self.top_of(k).map_or(z, |t| t.z.max(z));
            match top(w, Block { body, face, pos: glam::IVec3::new(i, j, from) }) {
                Some(t) => {
                    self.tops.entry(tile_of(k)).or_default().insert((i, j), t);
                }
                None => {
                    if let Some(tile) = self.tops.get_mut(&tile_of(k)) {
                        tile.remove(&(i, j));
                        if tile.is_empty() {
                            self.tops.remove(&tile_of(k));
                        }
                    }
                }
            }
        }
    }
    /// Column k's top as edits left it, if they changed it.
    pub fn top_of(&self, k: Column) -> Option<&Top> {
        self.tops.get(&tile_of(k))?.get(&(k.2, k.3))
    }
    /// How many columns edits changed.
    #[cfg(test)]
    pub fn edited(&self) -> usize {
        self.tops.values().map(|t| t.len()).sum()
    }
    /// The edited columns of body b that may be in the screen (w x h) of view v, `cap` at the most: those of the
    /// tiles its ground falls in, found from a grid of points picked on it and its rim (a tile's margin round),
    /// so a frame's work is the screen's, never everything ever built.
    pub fn visible(&self, b: usize, body: &engine::world::Body, v: &View, (w, h): (f64, f64), cap: usize) -> Vec<Top> {
        const S: usize = 16;
        let zero = self.zeros[b];
        let mut boxes: [Option<[i32; 4]>; 6] = [None; 6];
        let grid = (0..=S * S + 2 * S).map(|k| (w * (k % (S + 1)) as f64 / S as f64, h * (k / (S + 1)) as f64 / S as f64));
        let rim = (0..64).map(|k| {
            let a = k as f64 / 64.0 * 2.0 * PI;
            (v.cx + v.r * 0.999 * a.cos(), v.cy + v.r * 0.999 * a.sin())
        });
        for (x, y) in grid.chain(rim.filter(|&(x, y)| x >= 0.0 && y >= 0.0 && x <= w && y <= h)) {
            let Some(d) = v.pick(x, y, zero) else { continue };
            let (f, i, j) = body.locate(d);
            let (ti, tj) = ((i / TILE as f64).floor() as i32, (j / TILE as f64).floor() as i32);
            let bx = boxes[f].get_or_insert([ti, tj, ti, tj]);
            *bx = [bx[0].min(ti), bx[1].min(tj), bx[2].max(ti), bx[3].max(tj)];
        }
        let mut out = Vec::new();
        // shortcut: a face whose corner pokes in between the points (under a tile's margin) is missed; more points if one is
        for (f, bx) in boxes.iter().enumerate() {
            let Some([i0, j0, i1, j1]) = *bx else { continue };
            for ti in i0 - 1..=i1 + 1 {
                for tile in self.tops.range((b as u8, f as u8, ti, j0 - 1)..=(b as u8, f as u8, ti, j1 + 1)).map(|e| e.1) {
                    for t in tile.values() {
                        if out.len() >= cap {
                            return out;
                        }
                        out.push(*t);
                    }
                }
            }
        }
        out
    }
}

/// What building and digging made of a column, as the map draws it: its top block, the layer it is in, how many
/// layers over (or, a pit, under) the ground generation gave it, and the column's middle as a direction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Top {
    pub block: u16,
    pub z: i32,
    pub dz: i32,
    pub dir: DVec3,
}

/// Column `b`'s top as edits left it, walking down from layer `b.pos.z` (or from over the ground) to the first
/// solid block; None when it is generation's own (an edit undone) or its body is not in on this client.
pub fn top(w: &World, b: Block) -> Option<Top> {
    let body = &w.bodies[b.body as usize];
    if body.gas || !w.loaded(b.body as usize) {
        return None;
    }
    let h = w.column_height(b);
    // the generated top: the highest layer whose middle is under the surface (`generated_under`)
    let ground = (body.layer_of(h) - 0.5).floor() as i32;
    let mut c = Block { pos: glam::IVec3::new(b.pos.x, b.pos.y, b.pos.z.max(ground + 1)), ..b };
    for _ in 0..512 {
        let m = w.edit(c).unwrap_or_else(|| w.generated_under(c, h));
        // over the ground only an edit is a build: generation's own there is a plant (a trunk left of a felled tree)
        let built = c.pos.z <= ground || w.edit(c).is_some();
        if built && engine::blocks::BLOCKS.solid(m) || c.pos.z <= 0 {
            let dir = body.dir(b.face as usize, b.pos.x as f64 + 0.5, b.pos.y as f64 + 0.5);
            return (c.pos.z != ground || w.edit(c).is_some_and(|e| e != w.generated_under(c, h))).then_some(Top { block: m, z: c.pos.z, dz: c.pos.z - ground, dir });
        }
        c.pos.z -= 1;
    }
    None
}

/// The colour a column's top shows in: its block's, brighter the higher it was built, darker the deeper dug.
pub fn top_colour(t: &Top) -> [f32; 3] {
    let c = engine::blocks::BLOCKS.get(t.block).map_or([0.5; 3], |d| d.colour);
    let k = if t.dz > 0 { (1.0 + 0.06 * t.dz as f32).min(1.6) } else { (1.0 + 0.1 * t.dz as f32).max(0.3) };
    c.map(|v| (v * k).min(1.0))
}

/// From how many pixels a metre the edits show on the globe: a block a UI pixel at the least (zoom 9 on Terra at
/// a 1080 high screen), drawn `PX` wide at the least.
pub const EDITS_FROM: f64 = 1.0;

/// The least on-screen radius of a body in the system view, pixels.
pub const MIN_PX: f64 = 4.0;
/// Under this zoom the map is the system around the focused body; from it on that body's globe, as before.
pub fn system(zoom: f64) -> bool {
    zoom < 1.0
}
/// Pixels a metre at `zoom` on a body of radius r on a screen h high: at zoom 1 its globe fills `fit(h)` (the
/// game takes it as the view's radius over the body's, `globe_ui`).
#[cfg(test)]
pub fn scale(h: f64, zoom: f64, r: f64) -> f64 {
    fit(h) * zoom / r
}
/// The least zoom on a body of radius r: the farthest thing, `extent` metres from its centre, at 45 % of the
/// screen's height from the middle.
pub fn least(r: f64, extent: f64) -> f64 {
    (0.45 / 0.4 * r / extent.max(r)).min(1.0)
}

/// A body (or the sun) on the system view: its middle and radius, pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dot {
    pub x: f64,
    pub y: f64,
    pub r: f64,
}

/// Every body of `w` in row order, then the sun, on the system view: looked down on from the ecliptic's north
/// (inertial +z towards the viewer, +y up the screen), the focused body's centre at (cx, cy), `s` pixels a metre, each at least
/// `MIN_PX` across. Where they are is the sky's own (`World::pose`), now.
pub fn dots(w: &World, focus: usize, (cx, cy): (f64, f64), s: f64) -> Vec<Dot> {
    let c = w.pose(focus, w.time).centre;
    let at = |p: DVec3, r: f64| Dot { x: cx + (p - c).x * s, y: cy - (p - c).y * s, r: (r * s).max(MIN_PX) };
    (0..w.bodies.len()).map(|i| at(w.pose(i, w.time).centre, w.bodies[i].radius)).chain([at(w.sky.sun(w.time), w.sky.sun_radius)]).collect()
}

/// The farthest any body or the sun is from body `focus`'s centre, metres: what `least` fits.
pub fn extent(w: &World, focus: usize) -> f64 {
    let c = w.pose(focus, w.time).centre;
    (0..w.bodies.len()).map(|i| (w.pose(i, w.time).centre - c).length() + w.bodies[i].radius).fold((w.sky.sun(w.time) - c).length() + w.sky.sun_radius, f64::max)
}

/// The dot under pixel (x, y): of those whose disc, 6 pixels larger, holds it, the one whose middle is nearest
/// (a small moon drawn over its planet is picked on itself).
pub fn pick(dots: &[Dot], x: f64, y: f64) -> Option<usize> {
    dots.iter().enumerate().map(|(k, d)| (k, (d.x - x).hypot(d.y - y), d.r)).filter(|&(_, e, r)| e <= r + 6.0).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(k, ..)| k)
}

/// Each dot's label, written at its right (`width` pixels a text): its name; or None when its disc and name would
/// lie on a dot labelled before it and that label, which then names it too ("Terra, Luna"), so no two labels lie
/// on each other and every name shows.
pub fn labels(dots: &[Dot], names: &[String], width: impl Fn(&str) -> f64) -> Vec<Option<String>> {
    let bx = |d: &Dot, s: &str| [d.x - d.r - 4.0, d.y - d.r.max(10.0), d.x + d.r + 9.0 + width(s), d.y + d.r.max(10.0)];
    let over = |a: [f64; 4], b: [f64; 4]| a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3];
    let mut out: Vec<Option<String>> = vec![None; dots.len()];
    for k in 0..dots.len() {
        match (0..k).find(|&j| out[j].as_ref().is_some_and(|l| over(bx(&dots[j], l), bx(&dots[k], &names[k])))) {
            Some(j) => {
                let s = out[j].as_mut().unwrap();
                s.push_str(", ");
                s.push_str(&names[k]);
            }
            None => out[k] = Some(names[k].clone()),
        }
    }
    out
}

/// Every player the server lists (`ToClient::Players`: all of them, wherever they are, once a second and on each
/// join and leave), on the body whose frame they are in, by name in the list's order, `me` as "you"; one out in
/// space is on none.
pub fn players_on(w: &World, players: &[engine::net::Player], me: u32) -> Vec<Vec<String>> {
    let mut on = vec![Vec::new(); w.bodies.len()];
    for p in players {
        if let Some(names) = on.get_mut(w.frame_at(DVec3::from_array(p.pos))) {
            names.push(if p.id == me { "you".into() } else if p.name.is_empty() { format!("player {}", p.id) } else { p.name.clone() });
        }
    }
    on
}

/// A body's players as its mark says them: up to three names, else how many.
pub fn crowd(names: &[String]) -> Option<String> {
    match names.len() {
        0 => None,
        1..=3 => Some(names.join(", ")),
        n => Some(format!("{n} players")),
    }
}

/// The bodies in the map's buttons' order from each one's parent (`INERTIAL` the sun), each with how deep a moon
/// it is: every body that circles the sun in row order, its moons (and theirs) under it. Button k focuses the
/// body `order[k].0`.
pub fn order(parents: &[usize]) -> Vec<(usize, usize)> {
    let root = |mut i: usize| {
        let mut d = 0;
        while parents[i] != engine::world::INERTIAL {
            (i, d) = (parents[i], d + 1);
        }
        (i, d)
    };
    (0..parents.len()).filter(|&r| parents[r] == engine::world::INERTIAL).flat_map(|r| (0..parents.len()).filter(move |&i| root(i).0 == r).map(move |i| (i, root(i).1))).collect()
}

/// What a click in the system view at (x, y) does (`pick` on the dots): focuses a body, or on the focused body's
/// disc once it is `large` pixels in radius (its globe drawn with its marks) is a click on that globe.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SystemClick {
    Nothing,
    Focus(usize),
    Globe,
}
pub fn system_click(dots: &[Dot], focus: usize, x: f64, y: f64, large: f64) -> SystemClick {
    match pick(dots, x, y) {
        Some(k) if k == focus && dots[k].r >= large => SystemClick::Globe,
        Some(k) => SystemClick::Focus(k),
        None => SystemClick::Nothing,
    }
}

/// Points an orbit is drawn through (`orbit`).
pub const ORBIT_POINTS: usize = 256;

/// Body i's orbit as `n + 1` points around its parent's centre (the sun's for one that circles it), the first and
/// the last the same: its pose over one period by the same `World::pose` the sky draws, the middle point where it
/// is now and the points closest there (the phase the cube of an even step), so the line runs through the body
/// and follows its path at any zoom its disc shows at. Made every frame the system shows.
pub fn orbit(w: &World, i: usize, n: usize) -> Vec<DVec3> {
    let b = &w.bodies[i];
    let parent = |t: f64| if b.parent == engine::world::INERTIAL { w.sky.sun(t) } else { w.pose(b.parent, t).centre };
    (0..=n)
        .map(|k| {
            let x = 2.0 * k as f64 / n as f64 - 1.0;
            let t = w.time + b.period * 0.5 * x * x * x;
            w.pose(i, t).centre - parent(t)
        })
        .collect()
}

/// Segment a..b clipped to the box (0, 0)-(w, h) (Liang-Barsky), None when it misses it.
pub fn clip(a: (f64, f64), b: (f64, f64), (w, h): (f64, f64)) -> Option<((f64, f64), (f64, f64))> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-dx, a.0), (dx, w - a.0), (-dy, a.1), (dy, h - a.1)] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
        } else if p < 0.0 {
            t0 = t0.max(q / p);
        } else {
            t1 = t1.min(q / p);
        }
    }
    (t0 <= t1).then(|| ((a.0 + dx * t0, a.1 + dy * t0), (a.0 + dx * t1, a.1 + dy * t1)))
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
        // every body with a map has a picture, a gas body none; the pictures in row order, then the masks
        let rows = &engine::bodies::BODIES;
        let (slots, len) = layout(rows);
        assert_eq!(slots.len(), rows.len());
        for (r, s) in rows.iter().zip(&slots) {
            assert_eq!(s.is_some(), r.bake.is_some(), "{}", r.name);
        }
        let (t, l) = (slots[0].unwrap(), slots[1].unwrap());
        assert_eq!((t.pic, t.size, l.pic, l.size), (32, (1024, 512), 32 + 1024 * 512, (512, 256)), "Terra's and Luna's where they were");
        let pics: usize = slots.iter().flatten().map(|s| s.size.0 * s.size.1).sum();
        assert_eq!(t.mask, HEAD + pics, "the masks after every picture");
        assert_eq!(len, HEAD + pics + pics / 32);
        // the head carries the slot the shader reads (overlay.wgsl globe_px: words 24, 25 and 26)
        let v = View { cx: 1.0, cy: 2.0, r: 3.0, lat: 0.0, lon: 0.0 };
        let h = v.head(DVec3::Y, 3.0, DVec3::X, l, true, 800.0, true, 0.05);
        assert_eq!(h[24..27], [l.pic as u32, 512, l.mask as u32]);
        assert_eq!([f32::from_bits(h[20]), f32::from_bits(h[21]), f32::from_bits(h[27])], [1.0, 1.0, 0.05], "air, all, relief");
    }

    #[test]
    fn what_was_seen_comes_back_from_its_file() {
        let f = std::env::temp_dir().join(format!("technoape-seen-{}", std::process::id()));
        let file = Some(f.to_string_lossy().into_owned());
        let rows = &engine::bodies::BODIES;
        let zeros = vec![-DVec3::X; rows.len()];
        let mut s = State::new(rows, zeros.clone(), file.clone());
        s.seen[1].as_mut().unwrap().mark(DVec3::X, -DVec3::X, 0.2);
        s.seen[2].as_mut().unwrap().mark(DVec3::Y, -DVec3::X, 0.2);
        s.dirty.1 = true;
        s.save().unwrap();
        let back = State::new(rows, zeros.clone(), file.clone());
        assert_eq!(back.seen, s.seen);
        assert!(back.seen[1].as_ref().unwrap().bits.iter().any(|b| *b != 0) && back.seen[0].as_ref().unwrap().bits.iter().all(|b| *b == 0));
        // a file from before (Terra's and Luna's masks only) reads as it did, the new bodies unseen
        std::fs::write(&f, [s.seen[0].as_ref().unwrap().to_bytes(), s.seen[1].as_ref().unwrap().to_bytes()].concat()).unwrap();
        let old = State::new(rows, zeros, file);
        assert_eq!(old.seen[..2], s.seen[..2]);
        assert!(old.seen[2].as_ref().unwrap().bits.iter().all(|b| *b == 0));
        let _ = std::fs::remove_file(f);
    }

    /// A small world of the noise land (no bake) with every row's body, at a time `t`.
    fn world_at(t: f64) -> World {
        let rows = &engine::bodies::BODIES;
        let mut w = World::of(rows, engine::bodies::sizes(rows, 768, 256).into_iter().map(|n| (n, None)).collect(), 1, 0.0, engine::world::GENERATION);
        w.time = t;
        w
    }

    /// The zoom picks the view: from 1 on the globe as before (its radius `fit`), under it the system, and the
    /// focused body's disc there the same size the globe would be, so the zoom runs on without a jump.
    #[test]
    fn the_zoom_maps_to_the_globe_or_the_system() {
        let (h, r) = (1080.0, 3900.0);
        assert!(!system(1.0) && !system(ZOOM_MAX) && system(0.99) && system(1e-6));
        assert!((scale(h, 1.0, r) * r - fit(h)).abs() < 1e-9, "zoom 1: the globe fits as before");
        assert!((scale(h, 0.5, r) * r - fit(h) * 0.5).abs() < 1e-9);
        // the least zoom puts the farthest thing at 45 % of the height from the middle
        let far = 4.6e8;
        assert!((scale(h, least(r, far), r) * far - 0.45 * h).abs() < 1e-6);
        assert_eq!(least(r, r * 0.5), 1.0, "never past the globe");
        // the focused body's dot is the globe's disc at every zoom it is drawn at (globe_ui: view.r = fit(h) * zoom,
        // the dots at view.r over the radius), so the zoom runs on without a jump
        let w = world_at(0.0);
        for zoom in [0.05, 0.5, 0.99] {
            let vr = fit(h) * zoom;
            let d = dots(&w, 0, (0.0, 0.0), vr / w.bodies[0].radius);
            assert!((d[0].r - vr.max(MIN_PX)).abs() < 1e-9, "{zoom}: {} against {vr}", d[0].r);
            assert!((scale(h, zoom, w.bodies[0].radius) - vr / w.bodies[0].radius).abs() < 1e-12);
        }
    }

    /// Every body of the world's list shows on the system view, the sun last; a far body at least `MIN_PX`, a near
    /// one its real size; the focused body in the middle.
    #[test]
    fn every_body_shows_at_its_least_size() {
        let w = world_at(4321.0);
        let s = 1e-6;
        let d = dots(&w, 0, (500.0, 400.0), s);
        assert_eq!(d.len(), w.bodies.len() + 1, "every body and the sun");
        assert_eq!(w.bodies.len(), engine::bodies::BODIES.len());
        assert!(d.iter().all(|d| d.x.is_finite() && d.y.is_finite() && d.r >= MIN_PX));
        assert_eq!((d[0].x, d[0].y), (500.0, 400.0));
        // the sun is a far body's size from its real radius, Terra a dot at this scale
        assert_eq!(d[0].r, MIN_PX);
        assert!((d[w.bodies.len()].r - (w.sky.sun_radius * s).max(MIN_PX)).abs() < 1e-9);
        // close in, a body is its real size
        let near = dots(&w, 1, (0.0, 0.0), 1.0);
        assert!((near[1].r - w.bodies[1].radius).abs() < 1e-9);
        // the whole system fits at the least zoom
        let s = scale(1080.0, least(w.bodies[0].radius, extent(&w, 0)), w.bodies[0].radius);
        assert!(dots(&w, 0, (0.0, 0.0), s).iter().all(|d| d.x.abs() <= 0.45 * 1080.0 + 1e-6 && d.y.abs() <= 0.45 * 1080.0 + 1e-6));
    }

    /// A click picks the dot under it, a small moon over its planet on itself, nothing off every dot.
    #[test]
    fn a_click_picks_the_body_under_it() {
        let d = [Dot { x: 100.0, y: 100.0, r: 40.0 }, Dot { x: 130.0, y: 100.0, r: 4.0 }, Dot { x: 400.0, y: 300.0, r: 4.0 }];
        assert_eq!(pick(&d, 131.0, 101.0), Some(1), "the moon on itself");
        assert_eq!(pick(&d, 90.0, 95.0), Some(0));
        assert_eq!(pick(&d, 405.0, 305.0), Some(2), "a far dot a little off its edge");
        assert_eq!(pick(&d, 250.0, 250.0), None);
        let names: Vec<String> = ["Terra", "Luna", "Mars"].map(String::from).to_vec();
        let width = |s: &str| 6.0 * s.len() as f64;
        assert_eq!(labels(&d, &names, width), [Some("Terra, Luna".to_string()), None, Some("Mars".to_string())], "a moon on its planet named with it");
        // a dot clear of another's disc but under its label is named with it; one past the label has its own
        let row = [Dot { x: 100.0, y: 100.0, r: 4.0 }, Dot { x: 130.0, y: 102.0, r: 4.0 }, Dot { x: 200.0, y: 100.0, r: 4.0 }];
        assert_eq!(labels(&row, &names, width), [Some("Terra, Luna".to_string()), None, Some("Mars".to_string())]);
    }

    /// Every body has a button, one click from the page: a moon under its planet, whatever the rows' order.
    #[test]
    fn every_body_has_a_button_under_its_parent() {
        let w = world_at(0.0);
        let o = order(&w.bodies.iter().map(|b| b.parent).collect::<Vec<_>>());
        let mut bodies: Vec<usize> = o.iter().map(|b| b.0).collect();
        assert_eq!(o, [(0, 0), (1, 1), (2, 0), (3, 0), (4, 1)], "Terra, Luna under it, Mars, Jupiter, Callisto under it");
        bodies.sort();
        assert_eq!(bodies, (0..w.bodies.len()).collect::<Vec<_>>(), "each once");
        // a moon of the first planet listed after another planet still goes under its own
        let i = engine::world::INERTIAL;
        assert_eq!(order(&[i, i, 0, 1, 3]), [(0, 0), (2, 1), (1, 0), (3, 1), (4, 2)]);
    }

    /// Every player shows on the body they are on, however far from you (the far side of Terra, Luna); you as
    /// "you"; one who left is gone with the next list; a crowd is counted.
    #[test]
    fn every_player_is_on_the_map() {
        let w = world_at(0.0);
        let player = |id: u32, name: &str, pos: DVec3| engine::net::Player { id, name: name.into(), pos: pos.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0, hand: 0 };
        let (t, l) = (&w.bodies[0], &w.bodies[1]);
        let list = vec![
            player(1, "ape", t.centre + DVec3::X * (t.radius + 2.0)),
            player(2, "far", t.centre - DVec3::X * (t.radius + 2.0)),
            player(3, "moon", l.centre + DVec3::Z * (l.radius + 2.0)),
        ];
        let on = players_on(&w, &list, 1);
        assert_eq!(on[0], ["you", "far"], "the far side of Terra too");
        assert_eq!(on[1], ["moon"], "and Luna");
        assert!(on[2..].iter().all(Vec::is_empty));
        let on = players_on(&w, &list[..2], 1);
        assert!(on[1].is_empty(), "who left is gone");
        assert_eq!(crowd(&on[0]).as_deref(), Some("you, far"));
        assert_eq!(crowd(&["a", "b", "c", "d"].map(String::from)).as_deref(), Some("4 players"));
        assert_eq!(crowd(&[]), None);
    }

    /// Each body's orbit line, as drawn, runs through the body and follows its path within a pixel over a screen's
    /// width round it, at a zoom its disc shows at (20 px in radius, and at 1 px); and it closes.
    #[test]
    fn an_orbit_runs_through_the_body() {
        let w = world_at(1000.0 + 0.37 * 3.1e5);
        for i in 0..w.bodies.len() {
            let b = &w.bodies[i];
            let rel = |t: f64| w.pose(i, t).centre - if b.parent == engine::world::INERTIAL { w.sky.sun(t) } else { w.pose(b.parent, t).centre };
            let line = orbit(&w, i, ORBIT_POINTS);
            let off = |q: DVec3| {
                line.windows(2)
                    .map(|s| {
                        let (a, c) = (s[0], s[1]);
                        let u = ((q - a).dot(c - a) / (c - a).length_squared().max(1e-30)).clamp(0.0, 1.0);
                        (a + (c - a) * u - q).length()
                    })
                    .fold(f64::MAX, f64::min)
            };
            let r = rel(w.time).length();
            for disc in [20.0, 1.0] {
                // pixels a metre with the body's disc `disc` pixels in radius; the path 2000 px either side of it
                let s = disc / b.radius;
                let span = (2000.0 / s / (2.0 * PI * r)).min(0.5);
                for k in -20..=20 {
                    let t = w.time + b.period * span * k as f64 / 20.0;
                    let px = off(rel(t)) * s;
                    assert!(px < 1.0, "{} (disc {disc} px): {px} px off its line at {k}/20 of {span} of a period", b.name);
                }
            }
            assert!((line[0] - line[line.len() - 1]).length() < r * 1e-6, "{}: open", b.name);
        }
    }

    /// Segments are clipped to the screen: one across it with both ends off it is kept, one beside it is not.
    #[test]
    fn a_segment_across_the_screen_is_drawn() {
        let box_ = (800.0, 600.0);
        let (a, b) = clip((-100.0, 300.0), (900.0, 300.0), box_).expect("across the screen");
        assert!((a.0 - 0.0).abs() < 1e-9 && (b.0 - 800.0).abs() < 1e-9 && a.1 == 300.0);
        assert_eq!(clip((-100.0, -10.0), (900.0, -5.0), box_), None, "over the top");
        assert_eq!(clip((10.0, 10.0), (20.0, 20.0), box_), Some(((10.0, 10.0), (20.0, 20.0))), "inside it whole");
        assert_eq!(clip((-50.0, 700.0), (700.0, -50.0), box_).map(|s| s.0 .0.round()), Some(50.0), "a diagonal through a corner's side");
    }

    /// In the system view a click on the focused body's disc, once it is large, is the globe's (a waypoint), not a
    /// focus that would turn it back; a small one, or another body, is focused.
    #[test]
    fn a_click_on_the_large_focused_disc_is_the_globes() {
        let d = [Dot { x: 500.0, y: 400.0, r: 420.0 }, Dot { x: 960.0, y: 400.0, r: 6.0 }];
        assert_eq!(system_click(&d, 0, 300.0, 300.0, 40.0), SystemClick::Globe);
        assert_eq!(system_click(&d, 0, 961.0, 401.0, 40.0), SystemClick::Focus(1), "a moon over it is itself");
        assert_eq!(system_click(&d, 1, 300.0, 300.0, 40.0), SystemClick::Focus(0));
        let small = [Dot { r: 20.0, ..d[0] }, d[1]];
        assert_eq!(system_click(&small, 0, 505.0, 405.0, 40.0), SystemClick::Focus(0));
        assert_eq!(system_click(&d, 0, 10.0, 10.0, 40.0), SystemClick::Nothing);
    }

    /// A State with body b's picture of one height everywhere (`height`, 1..=255) and `relief`.
    fn level(b: usize, height: u8, relief: f64) -> State {
        let rows = &engine::bodies::BODIES;
        let mut g = State::new(rows, vec![-DVec3::X; rows.len()], None);
        let size = g.slots[b].unwrap().size;
        (g.heights[b], g.relief[b], g.got[b]) = (vec![height; size.0 * size.1], relief, true);
        g
    }

    /// A mark is drawn on the raised ground (`State::at`); a click at the pixel it is drawn at picks the place it
    /// marks (`State::pick`, the shader's march), near the limb too, where the unit sphere's pick is far off.
    #[test]
    fn a_click_on_a_mark_at_the_limb_picks_its_place() {
        let g = level(0, 1, 0.06);
        let zero = g.zeros[0];
        let v = View { cx: 960.0, cy: 540.0, r: 432.0 * 4.0, lat: 10.0, lon: 20.0 };
        let [e, _, f] = v.axes(zero);
        for tilt in [0.2f64, 0.5, 1.2, 1.45] {
            // a place `tilt` radians from the middle towards the east limb
            let d = (f * tilt.cos() + e * tilt.sin()).normalize();
            let (x, y) = g.at(0, &v, d).unwrap();
            let picked = g.pick(0, &v, x, y).expect("on the globe");
            let px = (picked - d).length() * v.r;
            assert!(px < 2.0, "tilt {tilt}: the pick is {px} px from the mark");
            // the unit sphere's pick misses it by more than a mark's 12 px hit radius near the limb
            if tilt > 1.0 {
                let flat = v.pick(x, y, zero).unwrap();
                let (fx, fy) = v.project_at(flat, zero, g.lift(0, flat)).unwrap();
                assert!((fx - x).hypot(fy - y) > 12.0, "tilt {tilt}: the old pick would have found it");
            }
        }
        // a ridge one texel wide (east to west) near the limb is met wherever the ray crosses it, not stepped over
        let mut g = level(0, 1, 0.06);
        let size = g.slots[0].unwrap().size;
        let (lat, lon) = crate::nav::geo((f * 1.4f64.cos() + e * 1.4f64.sin()).normalize(), zero);
        let ridge = texel(lat, lon, size);
        for k in [ridge - size.0, ridge, ridge + size.0] {
            g.heights[0][k] = 255;
        }
        let (rlat, rlon) = texel_geo(ridge % size.0, ridge / size.0, size);
        let dl = 360.0 / size.0 as f64;
        for u in -4..=4 {
            // a point inside the ridge, high on it, across its width
            let d = dir(rlat, rlon + dl * 0.1 * u as f64, zero);
            let (x, y) = v.project_at(d, zero, 0.99).unwrap();
            let hit = g.pick(0, &v, x, y).unwrap();
            let (a, b) = crate::nav::geo(hit, zero);
            assert_eq!(texel(a, b, size), ridge, "{u}: the ray stepped over the ridge");
        }
        assert!(march_steps(0.44, 1024) >= 143 && march_steps(0.0, 1024) == 8);
    }

    /// The edited columns a frame draws are those the screen holds, `cap` at the most, whatever was ever built:
    /// 100k columns over the whole body, a close view finds the few under it and every one of them.
    #[test]
    fn the_edits_drawn_are_the_screens_and_bounded() {
        let w = world_at(0.0);
        let body = &w.bodies[0];
        let rows = &engine::bodies::BODIES;
        let mut g = State::new(rows, vec![-DVec3::X; rows.len()], None);
        let n = body.n;
        let step = ((6.0 * (n as f64).powi(2) / 100_000.0).sqrt()).floor() as i32;
        let mut all = 0;
        for f in 0..6u8 {
            for i in (0..n).step_by(step as usize) {
                for j in (0..n).step_by(step as usize) {
                    let dir = body.dir(f as usize, i as f64 + 0.5, j as f64 + 0.5);
                    g.tops.entry(tile_of((0, f, i, j))).or_default().insert((i, j), Top { block: 1, z: 1, dz: 1, dir });
                    all += 1;
                }
            }
        }
        assert!(all >= 100_000, "{all}");
        let (sw, sh) = (1920.0, 1080.0);
        let rel = body.spawn_point() - body.centre;
        let (lat, lon) = crate::nav::geo(rel, g.zeros[0]);
        let v = View { cx: sw / 2.0, cy: sh / 2.0, r: fit(sh) * ZOOM_MAX, lat, lon };
        let t = std::time::Instant::now();
        let got = g.visible(0, body, &v, (sw, sh), 8000);
        let took = t.elapsed();
        let want: Vec<DVec3> = g.tops.values().flat_map(|t| t.values()).map(|t| t.dir).filter(|&d| v.project(d, g.zeros[0]).is_some_and(|(x, y)| x >= 0.0 && y >= 0.0 && x <= sw && y <= sh)).collect();
        assert!(!want.is_empty(), "some are on the screen");
        for d in &want {
            assert!(got.iter().any(|t| t.dir == *d), "a column on the screen is missed");
        }
        assert!(got.len() < all / 20, "{} of {all}: the screen's share", got.len());
        // the whole globe: as many as the cap, never all
        let whole = View { r: fit(sh), ..v };
        assert_eq!(g.visible(0, body, &whole, (sw, sh), 8000).len(), 8000);
        assert!(took.as_secs_f64() < 0.02, "{took:?} a frame (a debug build)");
    }

    /// An edit on a body not in on this client is kept and shown once the body loads.
    #[test]
    fn an_edit_on_a_body_not_loaded_shows_when_it_is() {
        let rows = &engine::bodies::BODIES;
        let sources = engine::bodies::sizes(rows, 768, 256).into_iter().enumerate().map(|(i, n)| (n, if i == 2 { engine::world::Source::Later(7) } else { engine::world::Source::Now(None) })).collect();
        let mut w = World::of_sources(rows, sources, 1, 0.0, engine::world::GENERATION);
        assert!(!w.loaded(2), "Mars is out");
        let mut g = State::new(rows, vec![-DVec3::X; rows.len()], None);
        // the layer over the generated ground as `top` reckons it (the stub's own heights)
        let at = w.air_over_ground(2, w.bodies[2].spawn_point().normalize());
        let z = (w.bodies[2].layer_of(w.column_height(at)) - 0.5).floor() as i32 + 1;
        let ground = Block { pos: glam::IVec3::new(at.pos.x, at.pos.y, z), ..at };
        for k in 0..3 {
            let b = Block { pos: ground.pos + glam::IVec3::Z * k, ..ground };
            w.set_block(b, engine::blocks::BLOCKS.id("stone"));
            g.touch(b);
        }
        g.refresh(&w, 64);
        assert_eq!(g.edited(), 0, "nothing to work it out against yet");
        w.lazy[2].as_mut().unwrap().loaded = true;
        g.refresh(&w, 64);
        assert_eq!(g.top_of((2, ground.face, ground.pos.x, ground.pos.y)).map(|t| t.dz), Some(3), "the tower, once Mars is in");
    }

    /// What building and digging changed shows on the map: a tower and a pit near the spawn are touched, worked
    /// out and read as their top blocks, over and under the ground; undone, they are gone from it.
    #[test]
    fn a_build_and_a_dig_show_on_the_map() {
        let mut w = world_at(0.0);
        let mut g = State::new(&engine::bodies::BODIES, vec![-DVec3::X; w.bodies.len()], None);
        let ground = w.air_over_ground(0, w.bodies[0].spawn_point().normalize());
        let col = |b: Block| (b.body, b.face, b.pos.x, b.pos.y);
        let stone = engine::blocks::BLOCKS.id("stone");
        // a tower five high, and two columns over a pit three deep
        let tower = ground;
        for k in 0..5 {
            let b = Block { pos: tower.pos + glam::IVec3::Z * k, ..tower };
            w.set_block(b, stone);
            g.touch(b);
        }
        let pit = Block { pos: ground.pos + glam::IVec3::X * 2, ..ground };
        let pit = w.air_over_ground(0, w.bodies[0].dir(pit.face as usize, pit.pos.x as f64 + 0.5, pit.pos.y as f64 + 0.5));
        for k in 1..=3 {
            let b = Block { pos: pit.pos - glam::IVec3::Z * k, ..pit };
            w.set_block(b, engine::blocks::AIR);
            g.touch(b);
        }
        g.refresh(&w, 1);
        assert_eq!(g.touched.len(), 1, "one column a call of one: the budget holds");
        g.refresh(&w, 64);
        let t = *g.top_of(col(tower)).unwrap();
        assert_eq!((t.block, t.dz), (stone, 5), "the tower's top, five over the ground");
        let p = *g.top_of(col(pit)).unwrap();
        assert_eq!(p.dz, -3, "the pit's floor, three under");
        assert!(top_colour(&t).iter().sum::<f32>() > top_colour(&Top { dz: 0, ..t }).iter().sum::<f32>(), "a tower brighter");
        // an untouched column is not on it; a column put back is gone from it
        assert_eq!(g.edited(), 2);
        for k in 0..5 {
            let b = Block { pos: tower.pos + glam::IVec3::Z * k, ..tower };
            w.put_edit(b, None);
            g.touch(b);
        }
        g.refresh(&w, 64);
        assert!(g.top_of(col(tower)).is_none(), "{:?}", g.top_of(col(tower)));
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
        let (img, relief) = image(&map, DVec3::Y, size, 10.0);
        let rgb = |lat, lon| {
            let v = img[texel(lat, lon, size)];
            [v & 255, v >> 8 & 255, v >> 16 & 255]
        };
        let (land, sea) = (rgb(0.0, 0.0), rgb(0.0, 180.0));
        assert!(land[1] > land[2], "grassland is green: {land:?}");
        assert!(sea[2] > sea[0] && sea[2] > sea[1], "the deep sea is blue: {sea:?}");
        // the relief: the land stands out of a level sea by its height over the radius (40 m), none deeper for the sea
        let at = |lat, lon| lift(img[texel(lat, lon, size)], relief);
        assert!((relief - RELIEF * 50.0 / (64.0 * 2.0 / PI)).abs() < 1e-9, "{relief}");
        assert_eq!(image(&map, DVec3::Y, size, 0.01).1, 0.01, "the cap holds it");
        assert!((at(0.0, 0.0) - 1.0).abs() < 1e-9 && (at(0.0, 180.0) - (1.0 - relief)).abs() < 1e-9, "land high, sea level");
        assert!(img.iter().all(|t| t >> 24 >= 1), "every texel has its height (0 is no picture)");
        assert_eq!(img.len(), size.0 * size.1);
    }

    /// An icy moon's map (`bake::icy`) is drawn in its own colours: dark plains, bright ice, each kind its own.
    #[test]
    fn an_icy_map_has_its_own_colours() {
        use engine::bake::icy;
        let m = 8;
        let colour = |kind: u8| {
            let map = Map { version: icy::VERSION, n: 64, m, h: vec![0.0; 6 * 64], biome: vec![kind; 6 * 64], ..Map::empty() };
            let v = image(&map, DVec3::Y, (16, 8), 0.025).0[texel(0.0, 0.0, (16, 8))];
            [v & 255, v >> 8 & 255, v >> 16 & 255]
        };
        let sum = |c: [u32; 3]| c.iter().sum::<u32>();
        assert!(sum(colour(icy::PLAIN)) * 2 < sum(colour(icy::FRESH)), "dark plains, bright ice");
        let all: std::collections::HashSet<[u32; 3]> = (icy::PLAIN..=icy::FROST).map(colour).collect();
        assert_eq!(all.len(), icy::KINDS.len(), "every kind its own colour");
    }

    /// A Mars map (`bake::mars`) is drawn in Mars's colours, not taken for Luna's: its dust plains and highlands
    /// red, its polar ice white, every kind its own colour.
    #[test]
    fn a_mars_map_has_mars_colours() {
        use engine::bake::mars;
        let m = 8;
        let colour = |kind: u8| {
            let map = Map { version: mars::VERSION, n: 64, m, h: vec![0.0; 6 * 64], biome: vec![kind; 6 * 64], ..Map::empty() };
            let v = image(&map, DVec3::Y, (16, 8), 0.025).0[texel(0.0, 0.0, (16, 8))];
            [v & 255, v >> 8 & 255, v >> 16 & 255]
        };
        for k in [mars::HIGHLAND, mars::PLAIN] {
            let c = colour(k);
            assert!(c[0] > c[1] && c[1] >= c[2] && c[0] > c[2] + 25, "{}: red-brown, {c:?}", mars::KINDS[(k - mars::HIGHLAND) as usize]);
        }
        // the young lava and the shields dark (their basalt sand), under half the dust's brightness
        let sum = |c: [u32; 3]| c.iter().sum::<u32>();
        for k in [mars::LAVA, mars::SHIELD] {
            assert!(sum(colour(k)) * 2 < sum(colour(mars::PLAIN)) + 40, "{}: dark, {:?}", mars::KINDS[(k - mars::HIGHLAND) as usize], colour(k));
        }
        let ice = colour(mars::CAP);
        assert!(ice.iter().all(|&v| v > 180), "the cap is white: {ice:?}");
        let all: std::collections::HashSet<[u32; 3]> = (mars::HIGHLAND..=mars::FROST).map(colour).collect();
        assert_eq!(all.len(), mars::KINDS.len(), "every kind its own colour");
    }
}
