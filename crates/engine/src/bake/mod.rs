//! The bake of a planet with continents (worldgen.md stages 1 to 8, ADR 0042, 0043): plates, erosion, the
//! sea, rivers and lakes (`land`), the climate from the air grid (`air`), water on land, soil and biomes
//! (`life`). What the game keeps of it is a `Map`: per 8 m cell the height over the sea and the biome,
//! with mips of the heights for far chunks, and where players start. A bake is deterministic from (seed,
//! planet) on one machine; the server bakes or loads it, clients get it from the server (a local one shares
//! it in memory), and it is kept on disk by its hash.
pub mod air;
pub mod climate;
pub mod grid;
pub mod land;
pub mod life;
pub mod rivers;

pub use climate::Climate;
pub use rivers::{Lines, Reach};
use rivers::{V1, V2};

/// A map as version 3 wrote it (W3's climate; rivers without their flow, no lakes: a lake was a flat).
#[derive(Deserialize)]
struct V3 {
    #[allow(dead_code)]
    version: u32,
    seed: i32,
    n: i32,
    m: u32,
    h: Vec<f32>,
    biome: Vec<u8>,
    spawn: [f64; 3],
    rivers: Vec<(u32, u32, f32)>,
    climate: Option<Climate>,
}

use crate::world::fold;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex, Weak};

/// Changes whenever the bake's code or parameters change what it makes: a map of another version is baked
/// anew, and a save names the map it was played on (by hash). 3 keeps the climate (W3) and bakes it over a
/// year with the sky's seasons; 4 adds the lakes and each river cell's flow (W5, ADR 0059).
pub const VERSION: u32 = 4;

/// How far in longitude from the +y meridian the spawn may lie, radians (about 26 degrees: 9 am at world
/// time 0 is then between 7 and 11 am at the spawn). shortcut: until the sky's clock takes the spawn's own
/// longitude (sky.rs), which is the sky's code.
const SPAWN_LON: f64 = 0.45;

/// Where maps are kept, relative to the game's folder (next to the saves).
pub const DIR: &str = "saves/bakes";

#[derive(Serialize, Deserialize, Default)]
pub struct Map {
    pub version: u32,
    pub seed: i32,
    /// The planet's columns per face edge.
    pub n: i32,
    /// Cells per face edge; cell (f, i, j) is index (f * m + j) * m + i, on the engine's face axes.
    pub m: u32,
    /// Metres over the sea level: the ground (a lake's bed under its water, `lakes`).
    pub h: Vec<f32>,
    /// The biome (`life::ICE` .. `life::REEF`, the rows of `data/biomes.toml`).
    pub biome: Vec<u8>,
    /// Where players start: a unit direction from the centre (`spawn`).
    pub spawn: [f64; 3],
    /// The river cells (since version 2; ADR 0056), downstream first: a cell's `to` comes before it when it
    /// is a river cell too.
    pub rivers: Vec<Reach>,
    /// The lakes' cells and each one's level, metres over the sea, sorted (since version 4; ADR 0059). `h`
    /// holds the bed under the water; before version 4 a lake was a flat at its level and held none.
    pub lakes: Vec<(u32, f32)>,
    /// The air columns, the year's climate and the air's state at world time 0 (since version 3, W3); None on
    /// a map of an older version.
    pub climate: Option<Climate>,
    /// The climate's air columns (`Climate::grid`), built on load.
    #[serde(skip)]
    pub air: Option<grid::Grid>,
    /// The rivers' lines and beds, from `rivers` and `h`.
    #[serde(skip)]
    pub lines: Lines,
    /// The heights halved again and again (2 x 2 means), for chunks whose columns are wider than a cell.
    #[serde(skip)]
    pub mips: Vec<Vec<f32>>,
    /// How rough each cell's ground is, metres of detail (`rough`, ADR 0058); empty on a map of version 1,
    /// whose ground keeps the uniform detail of ADR 0043.
    #[serde(skip)]
    pub rough: Vec<f32>,
    /// FNV of the bytes it was read from or would be written as.
    #[serde(skip)]
    pub hash: u64,
}

impl Map {
    /// Bakes the planet of `n` columns per face edge from `seed`. Seconds: 22 for the 4 km planet on 8 threads.
    pub fn bake(seed: i32, n: i32) -> Map {
        Map::bake_with(seed, n, &land::Params::default())
    }
    /// `bake` with other land parameters (tests: smaller rivers on a small planet).
    pub fn bake_with(seed: i32, n: i32, p: &land::Params) -> Map {
        let t = std::time::Instant::now();
        let s = seed as u32 as u64;
        let mut b = land::bake(n as u32, s, p);
        let l = life::life(&b, s, &life::LifeParams::default());
        let h = b.h.clone();
        let lakes = (0..b.g.n()).filter(|&c| b.lake(c)).map(|c| (c as u32, b.lvl[c])).collect();
        let flow = l.flow.clone();
        let spawn = spawn(&b, &l);
        b.life = Some(l);
        let biome = std::mem::take(&mut b.life.as_mut().unwrap().biome);
        let climate = Some(b.life.as_ref().unwrap().climate.clone());
        let rivers = b.rivers.iter().filter(|r| b.land(r.cell as usize)).map(|r| Reach { cell: r.cell, to: r.to, area: r.area, flow: flow[r.cell as usize] }).collect();
        let mut map = Map { version: VERSION, seed, n, m: b.g.m as u32, h, biome, spawn, rivers, lakes, climate, ..Map::empty() };
        map.finish(&map.to_bytes());
        eprintln!("[bake] seed {seed} planet {n}: {:.1} s", t.elapsed().as_secs_f64());
        map
    }
    /// A map with nothing in it, for `..Map::empty()`.
    pub fn empty() -> Map {
        Map { version: VERSION, spawn: [0.0, 1.0, 0.0], ..Map::default() }
    }
    /// Its bytes in the layout of its version (an older map as it was read, so its hash holds).
    pub fn to_bytes(&self) -> Vec<u8> {
        let rivers = || self.rivers.iter().map(|r| (r.cell, r.to, r.area)).collect::<Vec<_>>();
        match self.version {
            1 => postcard::to_stdvec(&(1u32, self.seed, self.n, self.m, &self.h, &self.biome, self.spawn)),
            2 => postcard::to_stdvec(&(2u32, self.seed, self.n, self.m, &self.h, &self.biome, self.spawn, rivers())),
            3 => postcard::to_stdvec(&(3u32, self.seed, self.n, self.m, &self.h, &self.biome, self.spawn, rivers(), &self.climate)),
            _ => postcard::to_stdvec(self),
        }
        .expect("a map serialises")
    }
    /// A map of this version, or of version 3 (no lakes or flows), 2 (no climate either) or 1 (no rivers
    /// either): old saves carry on on the ground they were played on.
    pub fn from_bytes(b: &[u8]) -> Result<Map, String> {
        let version = postcard::take_from_bytes::<u32>(b).map_err(|e| format!("not a map: {e}"))?.0;
        let flowless = |r: Vec<(u32, u32, f32)>| r.into_iter().map(|(cell, to, area)| Reach { cell, to, area, flow: 0.0 }).collect();
        let mut m: Map = match version {
            VERSION => postcard::from_bytes::<Map>(b).map_err(|e| format!("not a map: {e}"))?,
            3 => {
                let v: V3 = postcard::from_bytes(b).map_err(|e| format!("not a map: {e}"))?;
                Map { version: 3, seed: v.seed, n: v.n, m: v.m, h: v.h, biome: v.biome, spawn: v.spawn, rivers: flowless(v.rivers), climate: v.climate, ..Map::empty() }
            }
            2 => {
                let v: V2 = postcard::from_bytes(b).map_err(|e| format!("not a map: {e}"))?;
                Map { version: 2, seed: v.seed, n: v.n, m: v.m, h: v.h, biome: v.biome, spawn: v.spawn, rivers: flowless(v.rivers), ..Map::empty() }
            }
            1 => {
                let v: V1 = postcard::from_bytes(b).map_err(|e| format!("not a map: {e}"))?;
                Map { version: 1, seed: v.seed, n: v.n, m: v.m, h: v.h, biome: v.biome, spawn: v.spawn, ..Map::empty() }
            }
            v => return Err(format!("a map of version {v} that does not read")),
        };
        let cells = 6 * m.m as usize * m.m as usize;
        if let Some(c) = &m.climate {
            c.check()?;
        }
        let known = m.version == VERSION || m.version == 3 || (m.version == 2 && m.climate.is_none()) || (m.version == 1 && m.rivers.is_empty());
        // a river drains into a neighbouring cell (bytes from a server: a far one would build a huge index)
        let mm = m.m as i32;
        let next_to = |r: &Reach| {
            let (f, i, j) = ((r.cell / (m.m * m.m)) as usize, (r.cell % m.m) as i32, ((r.cell / m.m) % m.m) as i32);
            (-1..=1).any(|dj| (-1..=1).any(|di| {
                let (f, i, j) = fold(f, i + di, j + dj, mm);
                (f as u32 * m.m + j as u32) * m.m + i as u32 == r.to
            }))
        };
        let bad = m.rivers.iter().any(|r| r.cell as usize >= cells || r.to as usize >= cells || !(r.area.is_finite() && r.area > 0.0) || !(r.flow.is_finite() && r.flow >= 0.0) || !next_to(r))
            || m.lakes.iter().any(|&(c, l)| c as usize >= cells || !l.is_finite())
            || m.lakes.windows(2).any(|w| w[0].0 >= w[1].0);
        if !known || bad || m.h.len() != cells || m.biome.len() != cells {
            return Err(format!("a map of version {} with {} cells, not {VERSION} and {cells}", m.version, m.h.len()));
        }
        m.finish(b);
        Ok(m)
    }
    fn finish(&mut self, bytes: &[u8]) {
        self.hash = crate::world::fnv(crate::world::FNV0, bytes);
        self.mips.clear();
        let mut m = self.m as usize;
        while m % 2 == 0 && m > 2 {
            let src = self.mips.last().unwrap_or(&self.h);
            let h = m / 2;
            let next: Vec<f32> = (0..6 * h * h)
                .map(|c| {
                    let (f, i, j) = (c / (h * h), c % h, (c / h) % h);
                    let at = |a: usize, b: usize| src[(f * m + 2 * j + b) * m + 2 * i + a];
                    (at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1)) * 0.25
                })
                .collect();
            self.mips.push(next);
            m = h;
        }
        self.lines = Lines::new(self);
        self.rough = if self.version == 1 { Vec::new() } else { self.roughs() };
        self.air = self.climate.as_ref().map(Climate::grid);
    }
    /// The year's climate at face point (x, y), in cells: the surface air's potential temperature (C at sea
    /// level: less `air::Params::lapse` a metre over the sea is the air there), its day swing (K) and rain
    /// (mm a day); None on a map from before the climate.
    pub fn climate_at(&self, face: usize, x: f64, y: f64) -> Option<(f32, f32, f32)> {
        let (c, g) = (self.climate.as_ref()?, self.air.as_ref()?);
        Some(c.at(g, grid::Grid::bare(self.m as usize, c.r).dir_at(face, x, y)))
    }
    /// The baked year's mean temperature at the ground of the cell that holds face point (x, y), C: what the
    /// bake chose the biome by (`life::Life::temp`, a lake's cell at its level).
    pub fn temp(&self, face: usize, x: f64, y: f64) -> Option<f32> {
        self.climate.as_ref()?;
        let m = self.m as usize;
        let (i, j) = ((x.floor().max(0.0) as usize).min(m - 1), (y.floor().max(0.0) as usize).min(m - 1));
        let theta = self.climate_at(face, i as f64 + 0.5, j as f64 + 0.5)?.0;
        Some(theta - air::Params::default().lapse * self.h[(face * m + j) * m + i].max(0.0))
    }
    /// Every cell's `rough`, its slope from its four neighbours (across the seams too).
    fn roughs(&self) -> Vec<f32> {
        let m = self.m as i32;
        let cell = self.n as f32 / m as f32;
        let at = |f: usize, i: i32, j: i32| {
            let (f, i, j) = fold(f, i, j, m);
            self.h[(f * m as usize + j as usize) * m as usize + i as usize]
        };
        (0..self.h.len())
            .map(|c| {
                let (f, i, j) = (c / (m * m) as usize, c as i32 % m, (c as i32 / m) % m);
                let slope = (at(f, i + 1, j) - at(f, i - 1, j)).hypot(at(f, i, j + 1) - at(f, i, j - 1)) / (2.0 * cell);
                rough(slope, self.biome[c], self.h[c])
            })
            .collect()
    }
    /// The heights of mip `k` (0 the cells themselves) and its cells per face edge.
    fn level(&self, k: usize) -> (&[f32], i32) {
        let k = k.min(self.mips.len());
        (if k == 0 { &self.h } else { &self.mips[k - 1] }, (self.m >> k) as i32)
    }
    /// Metres over the sea at face point (x, y) in cells of mip `k` (x = 0.5 the middle of cell 0), a
    /// Catmull-Rom spline through the 4 x 4 cells around it; cells past the face fold over its seam.
    pub fn height(&self, face: usize, x: f64, y: f64, k: usize) -> f32 {
        self.spline::<false>(face, x, y, k).0
    }
    /// `height` at mip 0 with the ground's roughness there (`rough`), on the same spline: no step at a cell's
    /// border. Only on a map with `rough` (version 2 on).
    pub fn height_rough(&self, face: usize, x: f64, y: f64) -> (f32, f32) {
        self.spline::<true>(face, x, y, 0)
    }
    fn spline<const R: bool>(&self, face: usize, x: f64, y: f64, k: usize) -> (f32, f32) {
        let (h, m) = self.level(k);
        let (x, y) = (x / (1u32 << k.min(self.mips.len())) as f64 - 0.5, y / (1u32 << k.min(self.mips.len())) as f64 - 0.5);
        let (i0, j0) = (x.floor() as i32, y.floor() as i32);
        let (tx, ty) = ((x - i0 as f64) as f32, (y - j0 as f64) as f32);
        let w = |t: f32| [((-0.5 * t + 1.0) * t - 0.5) * t, (1.5 * t - 2.5) * t * t + 1.0, ((-1.5 * t + 2.0) * t + 0.5) * t, (0.5 * t - 0.5) * t * t];
        let (wx, wy) = (w(tx), w(ty));
        let (mut s, mut r) = (0.0, 0.0);
        for (b, wb) in wy.iter().enumerate() {
            let (mut row, mut rr) = (0.0, 0.0);
            for (a, wa) in wx.iter().enumerate() {
                let (f, i, j) = fold(face, i0 - 1 + a as i32, j0 - 1 + b as i32, m);
                let c = (f * m as usize + j as usize) * m as usize + i as usize;
                row += wa * h[c];
                if R {
                    rr += wa * self.rough[c];
                }
            }
            s += wb * row;
            r += wb * rr;
        }
        // the spline overshoots a little next to a calm cell
        (s, r.max(0.0))
    }
    /// The biome of the cell that holds face point (x, y), in cells.
    pub fn biome(&self, face: usize, x: f64, y: f64) -> u8 {
        let m = self.m as i32;
        let (f, i, j) = fold(face, x.floor() as i32, y.floor() as i32, m);
        self.biome[(f * m as usize + j as usize) * m as usize + i as usize]
    }
}

/// How rough the ground of a cell is, metres of detail over the map's spline (ADR 0058): by its slope (m/m),
/// its biome and its height over the sea (`h`, m). Plains and beaches are smooth, steep ground rough; the
/// sea floor by the coast is calm, and a lake's flat (no slope at all) has none. Until rock provinces exist,
/// height and biome stand in for rock. W4 hands this function the cell's rock as one more argument: hard rock
/// raises it (and so the crags of `World::height_at`), soft rock lowers it, in place of the `high` factor.
pub fn rough(slope: f32, biome: u8, h: f32) -> f32 {
    if slope == 0.0 {
        return 0.0;
    }
    let s = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    };
    let by_slope = 0.1 + 0.4 * s(slope / 0.2) + 2.5 * s((slope - 0.25) / 0.6);
    let high = 1.0 + 0.5 * s((h - 50.0) / 50.0);
    let calm = if h < 0.0 { 0.1 + 0.9 * s((-h - 4.0) / 12.0) } else { 1.0 };
    let kind = match biome {
        life::BEACH => 0.3,
        life::WETLAND => 0.4,
        life::DESERT => 0.6,
        life::GRASSLAND | life::SAVANNA => 0.8,
        life::ICE | life::TUNDRA => 1.1,
        life::ALPINE => 1.3,
        _ => 1.0,
    };
    (by_slope * high * calm * kind).min(3.0)
}

/// Where players start (worldgen.md, "spawn by score"): gentle ground in grassland or temperate forest,
/// near the sea or a lake, the temperature nearest 14 C; the best-scored cell, ties to the lower index. It
/// keeps within `SPAWN_LON` of the +y meridian, where the sky's clock has 9 am at world time 0 (sky.rs).
fn spawn(b: &land::Bake, l: &life::Life) -> [f64; 3] {
    let g = &b.g;
    let slope = life::slope(b);
    let water = life::distance(g, |c| b.ocean[c] || b.lake(c));
    let lon = |c: usize| {
        let d = g.d(c as u32);
        (d.y.atan2(d.x) - std::f64::consts::FRAC_PI_2).abs()
    };
    let mut best: Option<(f32, usize)> = None;
    for wide in [false, true] {
        for c in 0..g.n() {
            let ok = matches!(l.biome[c], life::GRASSLAND | life::TEMPERATE_FOREST) && !b.lake(c) && b.h[c] > 2.0 && slope[c] < 0.08 && water[c] < 160.0;
            if !ok || (!wide && lon(c) > SPAWN_LON) {
                continue;
            }
            let score = -(l.temp[c] - 14.0).abs() - 40.0 * slope[c] - water[c] / 80.0;
            if best.map_or(true, |(s, _)| score > s) {
                best = Some((score, c));
            }
        }
        if best.is_some() {
            break;
        }
    }
    // no such cell (a tiny test planet): the land nearest the +y pole
    let c = best.map(|b| b.1).unwrap_or_else(|| {
        (0..g.n()).filter(|&c| b.h[c] > 2.0).max_by(|&a, &c| g.d(a as u32).y.total_cmp(&g.d(c as u32).y)).unwrap_or(0)
    });
    g.d(c as u32).to_array()
}

/// Where a bake or a map's download stands, for the title screen's bar: the stage's name and the fraction
/// done. The worker writes it a few dozen times; the frame thread reads it each frame.
#[derive(Default)]
pub struct Progress(Mutex<(&'static str, f32)>);

impl Progress {
    pub fn get(&self) -> (&'static str, f32) {
        *self.0.lock().unwrap()
    }
    /// (the fraction never falls, whatever order the reports come in)
    fn set(&self, stage: &'static str, f: f32) {
        let mut s = self.0.lock().unwrap();
        *s = (stage, f.clamp(0.0, 1.0).max(s.1));
    }
}

/// The bake's stages in order (the `lap` names of `land::bake` and `life::life`, then `Map::bake`'s own end)
/// and each one's seconds in a 4 km bake (2026-11-12), so the bar moves at the bake's pace. A lap of another
/// name is passed over.
const STAGES: [(&str, f32); 13] = [
    ("grid", 0.13),
    ("plates and boundaries", 0.62),
    ("erosion, coarse", 5.56),
    ("refine", 0.36),
    ("erosion, fine", 14.89),
    ("sea and coast", 0.87),
    ("rivers and lakes", 0.02),
    ("climate (air model)", 16.0),
    ("climate to cells", 0.10),
    ("water on land", 0.73),
    ("soil", 0.30),
    ("biomes", 0.05),
    ("the spawn and the mips", 1.1),
];

thread_local! {
    /// The progress `reporting` writes on this thread, and the stage under way (an index into `STAGES`).
    static SINK: std::cell::RefCell<Option<(Arc<Progress>, usize)>> = const { std::cell::RefCell::new(None) };
}

/// Runs `f` (a bake, `net::fetch_map`) on this thread with its progress written to `p`, which ends at 1.
pub fn reporting<T>(p: Arc<Progress>, f: impl FnOnce() -> T) -> T {
    SINK.with(|s| *s.borrow_mut() = Some((p.clone(), 0)));
    within(0.0);
    let out = f();
    SINK.with(|s| *s.borrow_mut() = None);
    p.set(p.get().0, 1.0);
    out
}

/// A report outside the bake's stages (the download): `stage` and its fraction.
pub fn report(stage: &'static str, f: f32) {
    SINK.with(|s| s.borrow().as_ref().map(|(p, _)| p.set(stage, f)));
}

/// The bake's stage `name` has ended (its `lap`): the next one is under way.
pub(crate) fn lap(name: &str) {
    SINK.with(|s| {
        if let Some((_, k)) = s.borrow_mut().as_mut() {
            *k = STAGES.iter().position(|s| s.0 == name).map_or(*k, |i| (i + 1).min(STAGES.len() - 1));
        }
    });
    within(0.0);
}

/// The stage under way is `f` done (an erosion's steps).
pub(crate) fn within(f: f32) {
    SINK.with(|s| {
        if let Some((p, k)) = s.borrow().as_ref() {
            let total: f32 = STAGES.iter().map(|s| s.1).sum();
            let before: f32 = STAGES[..*k].iter().map(|s| s.1).sum();
            p.set(STAGES[*k].0, (before + f * STAGES[*k].1) / total);
        }
    });
}

/// Maps in memory, so a local server, its client and in-process bots share one.
static LOADED: LazyLock<Mutex<HashMap<(i32, i32, u32), Weak<Map>>>> = LazyLock::new(Mutex::default);

/// Where the map of (seed, n) is kept.
pub fn file(seed: i32, n: i32) -> PathBuf {
    file_v(seed, n, VERSION)
}
fn file_v(seed: i32, n: i32, v: u32) -> PathBuf {
    PathBuf::from(DIR).join(format!("seed-{seed}-planet-{n}-v{v}.map"))
}

/// The map of (seed, n) if it is in memory or on disk, and its hash is `hash` (any of this version, when
/// None; a save that names one of version 1 gets that too).
pub fn find(seed: i32, n: i32, hash: Option<u64>) -> Option<Arc<Map>> {
    let ok = |m: &Map| hash.map_or(m.version == VERSION, |h| h == m.hash);
    let versions: &[u32] = if hash.is_some() { &[VERSION, 3, 2, 1] } else { &[VERSION] };
    for &v in versions {
        if let Some(m) = LOADED.lock().unwrap().get(&(seed, n, v)).and_then(Weak::upgrade).filter(|m| ok(m)) {
            return Some(m);
        }
        if let Some(m) = Map::from_bytes(&std::fs::read(file_v(seed, n, v)).unwrap_or_default()).ok().filter(|m| m.version == v && ok(m)) {
            let m = Arc::new(m);
            LOADED.lock().unwrap().insert((seed, n, v), Arc::downgrade(&m));
            return Some(m);
        }
    }
    None
}

/// Keeps a map in memory and on disk (a failed write only costs the next start a bake).
pub fn keep(map: Map) -> Arc<Map> {
    let path = file_v(map.seed, map.n, map.version);
    let tmp = path.with_extension("tmp");
    let wrote = std::fs::create_dir_all(DIR).and_then(|_| std::fs::write(&tmp, map.to_bytes())).and_then(|_| std::fs::rename(&tmp, &path));
    if let Err(e) = wrote {
        eprintln!("[bake] could not keep {}: {e}", path.display());
    }
    let m = Arc::new(map);
    LOADED.lock().unwrap().insert((m.seed, m.n, m.version), Arc::downgrade(&m));
    m
}

/// The map a save names by `hash`: from memory or disk, else baked now (a version 2 map is this bake without
/// its climate, a version 1 without its rivers too; neither comes back from a bake of another climate); None if
/// the bake makes another map.
pub fn for_hash(seed: i32, n: i32, hash: u64) -> Option<Arc<Map>> {
    if let Some(m) = find(seed, n, Some(hash)) {
        return Some(m);
    }
    let b = Map::bake(seed, n);
    // the older layouts of this bake: a lake a flat at its level, no lakes or flows (3); no climate (2); no rivers (1)
    let mut flat = b.h.clone();
    for &(c, l) in &b.lakes {
        flat[c as usize] = l;
    }
    let rivers: Vec<Reach> = b.rivers.iter().map(|r| Reach { flow: 0.0, ..*r }).collect();
    let old = |version: u32, rivers: Vec<Reach>, climate: Option<Climate>| {
        Map::from_bytes(&Map { version, seed, n, m: b.m, h: flat.clone(), biome: b.biome.clone(), spawn: b.spawn, rivers, climate, ..Map::empty() }.to_bytes()).ok()
    };
    let older = [old(3, rivers.clone(), b.climate.clone()), old(2, rivers, None), old(1, Vec::new(), None)];
    let b = keep(b);
    if b.hash == hash {
        return Some(b);
    }
    older.into_iter().flatten().find(|m| m.hash == hash).map(keep)
}

/// The map of (seed, n): from memory, from disk, else baked now and kept.
pub fn get(seed: i32, n: i32) -> Arc<Map> {
    find(seed, n, None).unwrap_or_else(|| keep(Map::bake(seed, n)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small() -> Map {
        let mut m = Map { seed: 1, n: 64, m: 8, h: (0..384).map(|c| (c % 8) as f32).collect(), biome: vec![3; 384], ..Map::empty() };
        m.finish(&m.to_bytes());
        m
    }

    #[test]
    fn the_spline_meets_every_cell_and_the_mips_keep_the_mean() {
        let m = small();
        // h rises by 1 a cell in i: the spline is exact on a line, at the centres and between them
        for (x, want) in [(2.5, 2.0), (3.0, 2.5), (4.75, 4.25)] {
            assert!((m.height(0, x, 4.5, 0) - want).abs() < 1e-5, "{x}: {}", m.height(0, x, 4.5, 0));
        }
        assert_eq!(m.mips.len(), 2);
        assert_eq!(m.mips[0][0], 0.5);
        assert!((m.height(0, 4.0, 4.0, 1) - 3.5).abs() < 1e-5, "mip 1: halfway between its cells 1 and 2");
    }

    #[test]
    fn a_map_reads_back_with_its_hash_and_a_short_one_is_refused() {
        let m = small();
        let back = Map::from_bytes(&m.to_bytes()).unwrap();
        assert_eq!((back.hash, back.h, back.mips.len()), (m.hash, m.h.clone(), 2));
        let mut short = small();
        short.h.pop();
        assert!(Map::from_bytes(&short.to_bytes()).is_err());
    }

    /// A bake that reports its progress: the fraction never falls and ends at 1, the stages are named as they
    /// come, and the map is the same to the bit as that of a bake that reports nothing.
    #[test]
    fn a_bake_reports_progress_and_makes_the_same_map() {
        let p = Arc::new(Progress::default());
        let (stop, stopped) = std::sync::mpsc::channel::<()>();
        let watch = {
            let p = p.clone();
            std::thread::spawn(move || {
                let mut seen = Vec::new();
                while stopped.try_recv().is_err() {
                    seen.push(p.get());
                    std::thread::sleep(std::time::Duration::from_micros(200));
                }
                seen.push(p.get());
                seen
            })
        };
        let a = reporting(p.clone(), || Map::bake(3, 256));
        stop.send(()).unwrap();
        let seen = watch.join().unwrap();
        assert!(seen.windows(2).all(|w| w[1].1 >= w[0].1), "the fraction falls: {seen:?}");
        assert_eq!(seen.last().unwrap().1, 1.0);
        let mut stages: Vec<&str> = seen.iter().map(|s| s.0).collect();
        stages.dedup();
        assert!(stages.len() >= 5, "stages seen: {stages:?}");
        assert!(seen.iter().any(|s| s.1 > 0.3 && s.1 < 0.7), "the bar moves within the erosion: {stages:?}");
        assert_eq!(a.hash, Map::bake(3, 256).hash);
    }

    /// A map of version 1 (an old save's, fetched from a server) is kept as the bytes it came in, in its own
    /// file; a new game never gets it; a save that names one whose file is gone gets it back from a bake.
    #[test]
    fn a_version_1_map_is_kept_apart_and_baked_back_for_its_save() {
        let fresh = Map::bake(-77, 256);
        let v1 = postcard::to_stdvec(&(1u32, fresh.seed, fresh.n, fresh.m, &fresh.h, &fresh.biome, fresh.spawn)).unwrap();
        let old = Map::from_bytes(&v1).unwrap();
        assert_eq!(old.to_bytes(), v1, "the bytes it came in");
        let h = old.hash;
        let _ = (std::fs::remove_file(file(-77, 256)), std::fs::remove_file(file_v(-77, 256, 1)));
        let kept = keep(old);
        assert!(file_v(-77, 256, 1).exists() && !file(-77, 256).exists(), "kept under its own version");
        assert!(find(-77, 256, None).is_none(), "a new game never gets it");
        assert_eq!(find(-77, 256, Some(h)).map(|m| m.hash), Some(h));
        drop(kept);
        let _ = std::fs::remove_file(file_v(-77, 256, 1));
        LOADED.lock().unwrap().remove(&(-77, 256, 1));
        assert_eq!(for_hash(-77, 256, h).map(|m| (m.version, m.hash)), Some((1, h)), "baked back for its save");
        assert_eq!(for_hash(-77, 256, h ^ 1).map(|m| m.hash), None, "another map's hash");
        let _ = (std::fs::remove_file(file(-77, 256)), std::fs::remove_file(file_v(-77, 256, 1)));
    }

    /// Bytes from a server are checked before anything is built from them.
    #[test]
    fn a_map_with_a_far_river_or_a_broken_tail_is_refused() {
        let mut m = small();
        m.rivers = vec![Reach { cell: 9, to: 300, area: 2e5, flow: 0.0 }];
        assert!(Map::from_bytes(&m.to_bytes()).is_err(), "a river that jumps across the planet");
        m.rivers = vec![Reach { cell: 9, to: 10, area: f32::NAN, flow: 0.0 }];
        assert!(Map::from_bytes(&m.to_bytes()).is_err(), "an area that is not a number");
        m.rivers = vec![Reach { cell: 9, to: 10, area: 2e5, flow: 0.0 }];
        let b = m.to_bytes();
        assert!(Map::from_bytes(&b).is_ok());
        assert!(Map::from_bytes(&b[..b.len() - 3]).is_err(), "a version 2 map cut short is not a version 1");
    }

    /// Version 3 keeps the lakes (each cell with its level; `h` holds the bed under it) and each river cell's
    /// mean flow; a map of version 2 still reads, with neither, and writes back as it came.
    #[test]
    fn lakes_and_flows_survive_and_a_version_2_map_still_reads() {
        let mut m = small();
        m.lakes = vec![(20, 5.5), (21, 5.5)];
        m.rivers = vec![Reach { cell: 9, to: 10, area: 2e5, flow: 0.3 }];
        let back = Map::from_bytes(&m.to_bytes()).unwrap();
        assert_eq!((back.lakes.clone(), back.rivers[0].flow), (m.lakes.clone(), 0.3));
        let v2 = postcard::to_stdvec(&(2u32, m.seed, m.n, m.m, &m.h, &m.biome, m.spawn, vec![(9u32, 10u32, 2e5f32)])).unwrap();
        let old = Map::from_bytes(&v2).unwrap();
        assert_eq!((old.version, old.lakes.len(), old.rivers[0].flow), (2, 0, 0.0));
        assert_eq!(old.to_bytes(), v2, "the bytes it came in");
        m.lakes = vec![(9999, 1.0)];
        assert!(Map::from_bytes(&m.to_bytes()).is_err(), "a lake cell off the map");
    }

    /// The map keeps each cell's baked temperature (W3): the climate it holds gives back the bake's own
    /// `life::Life::temp` at every cell that is not a lake, after a round trip through its bytes too.
    #[test]
    fn the_map_keeps_each_cells_baked_temperature() {
        let (seed, n) = (3, 256);
        let b = land::bake(n as u32, seed as u32 as u64, &land::Params::default());
        let l = life::life(&b, seed as u32 as u64, &life::LifeParams::default());
        let map = Map::from_bytes(&Map::bake(seed, n).to_bytes()).unwrap();
        assert!(map.climate.is_some(), "a map of version {} keeps its climate", map.version);
        let m = map.m as usize;
        let mut seen = 0;
        for c in 0..map.h.len() {
            if b.lake(c) {
                continue;
            }
            let (f, i, j) = (c / (m * m), c % m, (c / m) % m);
            let t = map.temp(f, i as f64 + 0.5, j as f64 + 0.5).unwrap();
            assert!((t - l.temp[c]).abs() < 1e-4, "cell {c}: the map says {t} C, the bake {}", l.temp[c]);
            seen += 1;
        }
        assert!(seen > map.h.len() / 2);
    }

    /// A map of version 2 (before the climate, a save's) reads as it was, without a climate, and gives its own
    /// bytes back; a climate cut short is refused.
    #[test]
    fn a_version_2_map_reads_without_a_climate_and_a_cut_climate_is_refused() {
        let fresh = Map::bake(-78, 256);
        let rivers: Vec<(u32, u32, f32)> = fresh.rivers.iter().map(|r| (r.cell, r.to, r.area)).collect();
        let v2 = postcard::to_stdvec(&(2u32, fresh.seed, fresh.n, fresh.m, &fresh.h, &fresh.biome, fresh.spawn, &rivers)).unwrap();
        let old = Map::from_bytes(&v2).unwrap();
        assert_eq!((old.version, old.climate.is_none(), old.rivers.len()), (2, true, fresh.rivers.len()));
        assert_eq!(old.to_bytes(), v2, "the bytes it came in");
        assert_eq!(old.temp(0, 1.5, 1.5), None);
        // version 3 (W3's: the climate, but no lakes or flows) reads with its climate and gives its bytes back
        let v3 = postcard::to_stdvec(&(3u32, fresh.seed, fresh.n, fresh.m, &fresh.h, &fresh.biome, fresh.spawn, &rivers, &fresh.climate)).unwrap();
        let old = Map::from_bytes(&v3).unwrap();
        assert_eq!((old.version, old.climate == fresh.climate, old.lakes.len(), old.rivers.iter().all(|r| r.flow == 0.0)), (3, true, 0, true));
        assert_eq!(old.to_bytes(), v3, "the bytes it came in");
        let mut cut = Map::bake(-78, 256);
        cut.climate.as_mut().unwrap().state.vap.pop();
        assert!(Map::from_bytes(&cut.to_bytes()).is_err(), "a state that does not fit its grid");
        let mut nan = Map::bake(-78, 256);
        nan.climate.as_mut().unwrap().theta[3] = f32::NAN;
        assert!(Map::from_bytes(&nan.to_bytes()).is_err(), "a mean that is not a number");
    }

    /// A save on a map of version 3 (W3's) finds that map's file again by its hash, and does not bake anew.
    #[test]
    fn a_version_3_map_file_is_found_by_its_hash() {
        let mut m = Map { version: 3, seed: -12347, n: 64, ..small() };
        m.finish(&m.to_bytes());
        let hash = m.hash;
        drop(keep(m)); // only the file is left: the loaded map is a weak reference
        assert!(find(-12347, 64, Some(hash)).is_some_and(|f| f.version == 3 && f.hash == hash));
    }

    /// The whole bake twice on a small planet: the same map to the bit.
    #[test]
    fn a_small_planet_bakes_the_same_twice() {
        let (a, b) = (Map::bake(3, 256), Map::bake(3, 256));
        assert_eq!(a.hash, b.hash);
        assert!(a.biome.iter().any(|&x| x < life::LAND_BIOMES) && a.h.iter().any(|&h| h < 0.0), "land and sea");
    }
}
