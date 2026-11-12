//! The bake of a planet with continents (worldgen.md stages 1 to 8, ADR 0042, 0043): plates, erosion, the
//! sea, rivers and lakes (`land`), the climate from the air grid (`air`), water on land, soil and biomes
//! (`life`). What the game keeps of it is a `Map`: per 8 m cell the height over the sea and the biome,
//! with mips of the heights for far chunks, and where players start. A bake is deterministic from (seed,
//! planet) on one machine; the server bakes or loads it, clients get it from the server (a local one shares
//! it in memory), and it is kept on disk by its hash.
pub mod air;
pub mod grid;
pub mod land;
pub mod life;

use crate::world::fold;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex, Weak};

/// Changes whenever the bake's code or parameters change what it makes: a map of another version is baked
/// anew, and a save names the map it was played on (by hash).
pub const VERSION: u32 = 1;

/// How far in longitude from the +y meridian the spawn may lie, radians (about 26 degrees: 9 am at world
/// time 0 is then between 7 and 11 am at the spawn). shortcut: until the sky's clock takes the spawn's own
/// longitude (sky.rs), which is the sky's code.
const SPAWN_LON: f64 = 0.45;

/// Where maps are kept, relative to the game's folder (next to the saves).
pub const DIR: &str = "saves/bakes";

#[derive(Serialize, Deserialize)]
pub struct Map {
    pub version: u32,
    pub seed: i32,
    /// The planet's columns per face edge.
    pub n: i32,
    /// Cells per face edge; cell (f, i, j) is index (f * m + j) * m + i, on the engine's face axes.
    pub m: u32,
    /// Metres over the sea level: the ground, or a lake's level over its bed (lakes are flats until the
    /// lakes hold water, W5).
    pub h: Vec<f32>,
    /// The biome (`life::ICE` .. `life::REEF`, the rows of `data/biomes.toml`).
    pub biome: Vec<u8>,
    /// Where players start: a unit direction from the centre (`spawn`).
    pub spawn: [f64; 3],
    /// The heights halved again and again (2 x 2 means), for chunks whose columns are wider than a cell.
    #[serde(skip)]
    pub mips: Vec<Vec<f32>>,
    /// FNV of the bytes it was read from or would be written as.
    #[serde(skip)]
    pub hash: u64,
}

impl Map {
    /// Bakes the planet of `n` columns per face edge from `seed`. Seconds: 22 for the 4 km planet on 8 threads.
    pub fn bake(seed: i32, n: i32) -> Map {
        let t = std::time::Instant::now();
        let s = seed as u32 as u64;
        let mut b = land::bake(n as u32, s, &land::Params::default());
        let l = life::life(&b, s, &life::LifeParams::default());
        let h: Vec<f32> = (0..b.g.n()).map(|c| if b.lake(c) { b.lvl[c] } else { b.h[c] }).collect();
        let spawn = spawn(&b, &l);
        b.life = Some(l);
        let biome = std::mem::take(&mut b.life.as_mut().unwrap().biome);
        let mut map = Map { version: VERSION, seed, n, m: b.g.m as u32, h, biome, spawn, mips: Vec::new(), hash: 0 };
        map.finish(&map.to_bytes());
        eprintln!("[bake] seed {seed} planet {n}: {:.1} s", t.elapsed().as_secs_f64());
        map
    }
    pub fn to_bytes(&self) -> Vec<u8> {
        postcard::to_stdvec(self).expect("a map serialises")
    }
    pub fn from_bytes(b: &[u8]) -> Result<Map, String> {
        let mut m: Map = postcard::from_bytes(b).map_err(|e| format!("not a map: {e}"))?;
        let cells = 6 * m.m as usize * m.m as usize;
        if m.version != VERSION || m.h.len() != cells || m.biome.len() != cells {
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
    }
    /// The heights of mip `k` (0 the cells themselves) and its cells per face edge.
    fn level(&self, k: usize) -> (&[f32], i32) {
        let k = k.min(self.mips.len());
        (if k == 0 { &self.h } else { &self.mips[k - 1] }, (self.m >> k) as i32)
    }
    /// Metres over the sea at face point (x, y) in cells of mip `k` (x = 0.5 the middle of cell 0), a
    /// Catmull-Rom spline through the 4 x 4 cells around it; cells past the face fold over its seam.
    pub fn height(&self, face: usize, x: f64, y: f64, k: usize) -> f32 {
        let (h, m) = self.level(k);
        let (x, y) = (x / (1u32 << k.min(self.mips.len())) as f64 - 0.5, y / (1u32 << k.min(self.mips.len())) as f64 - 0.5);
        let (i0, j0) = (x.floor() as i32, y.floor() as i32);
        let (tx, ty) = ((x - i0 as f64) as f32, (y - j0 as f64) as f32);
        let w = |t: f32| [((-0.5 * t + 1.0) * t - 0.5) * t, (1.5 * t - 2.5) * t * t + 1.0, ((-1.5 * t + 2.0) * t + 0.5) * t, (0.5 * t - 0.5) * t * t];
        let (wx, wy) = (w(tx), w(ty));
        let mut s = 0.0;
        for (b, wb) in wy.iter().enumerate() {
            let mut row = 0.0;
            for (a, wa) in wx.iter().enumerate() {
                let (f, i, j) = fold(face, i0 - 1 + a as i32, j0 - 1 + b as i32, m);
                row += wa * h[(f * m as usize + j as usize) * m as usize + i as usize];
            }
            s += wb * row;
        }
        s
    }
    /// The biome of the cell that holds face point (x, y), in cells.
    pub fn biome(&self, face: usize, x: f64, y: f64) -> u8 {
        let m = self.m as i32;
        let (f, i, j) = fold(face, x.floor() as i32, y.floor() as i32, m);
        self.biome[(f * m as usize + j as usize) * m as usize + i as usize]
    }
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
    ("climate (air model)", 5.12),
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
static LOADED: LazyLock<Mutex<HashMap<(i32, i32), Weak<Map>>>> = LazyLock::new(Mutex::default);

/// Where the map of (seed, n) is kept.
pub fn file(seed: i32, n: i32) -> PathBuf {
    PathBuf::from(DIR).join(format!("seed-{seed}-planet-{n}-v{VERSION}.map"))
}

/// The map of (seed, n) if it is in memory or on disk, and its hash is `hash` (any, when None).
pub fn find(seed: i32, n: i32, hash: Option<u64>) -> Option<Arc<Map>> {
    let ok = |m: &Map| hash.map_or(true, |h| h == m.hash);
    if let Some(m) = LOADED.lock().unwrap().get(&(seed, n)).and_then(Weak::upgrade).filter(|m| ok(m)) {
        return Some(m);
    }
    let m = Arc::new(Map::from_bytes(&std::fs::read(file(seed, n)).ok()?).ok().filter(ok)?);
    LOADED.lock().unwrap().insert((seed, n), Arc::downgrade(&m));
    Some(m)
}

/// Keeps a map in memory and on disk (a failed write only costs the next start a bake).
pub fn keep(map: Map) -> Arc<Map> {
    let path = file(map.seed, map.n);
    let tmp = path.with_extension("tmp");
    let wrote = std::fs::create_dir_all(DIR).and_then(|_| std::fs::write(&tmp, map.to_bytes())).and_then(|_| std::fs::rename(&tmp, &path));
    if let Err(e) = wrote {
        eprintln!("[bake] could not keep {}: {e}", path.display());
    }
    let m = Arc::new(map);
    LOADED.lock().unwrap().insert((m.seed, m.n), Arc::downgrade(&m));
    m
}

/// The map of (seed, n): from memory, from disk, else baked now and kept.
pub fn get(seed: i32, n: i32) -> Arc<Map> {
    find(seed, n, None).unwrap_or_else(|| keep(Map::bake(seed, n)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small() -> Map {
        let mut m = Map { version: VERSION, seed: 1, n: 64, m: 8, h: (0..384).map(|c| (c % 8) as f32).collect(), biome: vec![3; 384], spawn: [0.0, 1.0, 0.0], mips: Vec::new(), hash: 0 };
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

    /// The whole bake twice on a small planet: the same map to the bit.
    #[test]
    fn a_small_planet_bakes_the_same_twice() {
        let (a, b) = (Map::bake(3, 256), Map::bake(3, 256));
        assert_eq!(a.hash, b.hash);
        assert!(a.biome.iter().any(|&x| x < life::LAND_BIOMES) && a.h.iter().any(|&h| h < 0.0), "land and sea");
    }
}
