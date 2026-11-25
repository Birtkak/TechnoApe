//! The bake of a planet with continents (worldgen.md stages 1 to 8, ADR 0042, 0043): plates, erosion, the
//! sea, rivers and lakes (`land`), the climate from the air grid (`air`), water on land, soil and biomes
//! (`life`). What the game keeps of it is a `Map`: per 8 m cell the height over the sea and the biome,
//! with mips of the heights for far chunks, and where players start. A bake is deterministic from (seed,
//! planet) on one machine; the server bakes or loads it, clients get it from the server (a local one shares
//! it in memory), and it is kept on disk by its hash.
pub mod air;
pub mod climate;
pub mod grid;
pub mod icy;
pub mod land;
pub mod life;
pub mod luna;
pub mod mars;
pub mod pictures;
pub mod rivers;
pub mod rock;
pub mod ground;
pub mod volcano;

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

/// A map as version 4 wrote it (W5's lakes and flows; no rock).
#[derive(Deserialize)]
struct V4 {
    #[allow(dead_code)]
    version: u32,
    seed: i32,
    n: i32,
    m: u32,
    h: Vec<f32>,
    biome: Vec<u8>,
    spawn: [f64; 3],
    rivers: Vec<Reach>,
    lakes: Vec<(u32, f32)>,
    climate: Option<Climate>,
}

use crate::world::fold;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex, Weak};

/// Changes whenever the bake's code or parameters change what it makes: a map of another version is baked
/// anew, and a save names the map it was played on (by hash). 3 keeps the climate (W3) and bakes it over a
/// year with the sky's seasons; 4 adds the lakes and each river cell's flow (W5, ADR 0059); 5 keeps each cell's
/// rock province (W4, ADR 0063); 6 stretches the peaks (ADR 0105: the same layout as 5, other heights over
/// `land::STRETCH_FROM`; a save on a 5 is baked back by `Map::bake_v5`); 7 lays the polar caps (ADR 0107: the same
/// layout and ground as 6, ICE on the cold land and sea; a save on a 6 is baked back by `Map::bake_v6`); 8 runs the
/// climate over the year of real gravity (ADR 0180, `world::REAL`: Kepler's from the masses, `world::year_of`; the
/// layout of 7): a world of an earlier generation plays on a 7 (`get_at`, `Map::bake_v7`); 9 raises the volcanoes
/// on the arcs and keeps them after the map (`volcano`, draft 2026-11-23): a world before generation 10
/// (`world::VOLCANOES`) plays on an 8 (`Map::bake_v8`, the same bytes as before); 10 solves the water table and lays
/// the springs' streams (`ground`, ADR 0190) and keeps the table after the volcanoes: a world before generation 12
/// (`world::GROUND_WATER`) plays on a 9 (`Map::bake_v9`, the same bytes as before).
pub const VERSION: u32 = 11;

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
    /// Each cell's rock province (`rock::SHIELD` .. `rock::OCEAN`, the rows of `data/rocks.toml`; since
    /// version 5, ADR 0063); empty on an older map, whose rock is the biomes' plain stone.
    pub rock: Vec<u8>,
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
    /// Luna's lava tubes (`luna::VERSION` maps only): balls of air as (cell, depth of the middle under the
    /// cell's ground, radius), metres. A Terra map's bytes do not hold the field.
    #[serde(skip)]
    pub tubes: Vec<(u32, f32, f32)>,
    /// Terra's volcanoes (version 9 on; written after the map, as Luna's tubes are).
    #[serde(skip)]
    pub volcanoes: Vec<volcano::Volcano>,
    /// The water table, metres over the sea a cell (`ground::DRY` on tight rock; version 10 on, written after the
    /// volcanoes): the rest of the ground's Darcy flow (ADR 0190). Empty on an older map: its world is dry.
    #[serde(skip)]
    pub table: Vec<f32>,
    /// How the table's solve ended (version 10 on, written after the table): its sweeps and its residual, metres
    /// (`ground::Aquifer::residual`). Settled when the sweeps are under `ground::SWEEPS`; a field not settled is kept.
    #[serde(skip)]
    pub solve: (u32, f32),
    /// FNV of the bytes it was read from or would be written as.
    #[serde(skip)]
    pub hash: u64,
}

impl Map {
    /// Bakes Terra's planet of `n` columns per face edge from `seed` (her stages, `Stage::TERRA`). Seconds: 22 for
    /// the 4 km planet on 8 threads.
    pub fn bake(seed: i32, n: i32) -> Map {
        Map::bake_with(seed, n, &land::Params::default())
    }
    /// `bake` with other land parameters (tests: smaller rivers on a small planet).
    pub fn bake_with(seed: i32, n: i32, p: &land::Params) -> Map {
        run_with(&Stage::TERRA, seed, n, p, crate::world::year_of(n, crate::world::REAL))
    }
    /// The bake of version 7, its climate over the hand-set YEAR: the map of every world before real gravity.
    pub fn bake_v7(seed: i32, n: i32) -> Map {
        let mut m = run_with(&Stage::TERRA, seed, n, &land::Params { volcanoes: false, ground_water: false, coarse_air: false, ..Default::default() }, crate::sky::YEAR);
        m.version = 7;
        m.finish(&m.to_bytes());
        m
    }
    /// The bake of version 8, before the volcanoes: the map of every world from real gravity to generation 9.
    pub fn bake_v8(seed: i32, n: i32) -> Map {
        let mut m = run_with(&Stage::TERRA, seed, n, &land::Params { volcanoes: false, ground_water: false, coarse_air: false, ..Default::default() }, crate::world::year_of(n, crate::world::REAL));
        m.version = 8;
        m.finish(&m.to_bytes());
        m
    }
    /// The bake of version 9, before the ground water: the map of every world from the volcanoes to generation 11.
    pub fn bake_v9(seed: i32, n: i32) -> Map {
        let mut m = run_with(&Stage::TERRA, seed, n, &land::Params { ground_water: false, coarse_air: false, ..Default::default() }, crate::world::year_of(n, crate::world::REAL));
        m.version = 9;
        m.finish(&m.to_bytes());
        m
    }
    /// The bake of version 10, the year of its climate on the live air grid's own cells: the map of the worlds of
    /// generations 12 and 13.
    pub fn bake_v10(seed: i32, n: i32) -> Map {
        let mut m = run_with(&Stage::TERRA, seed, n, &land::Params { coarse_air: false, ..Default::default() }, crate::world::year_of(n, crate::world::REAL));
        m.version = 10;
        m.finish(&m.to_bytes());
        m
    }
    /// The year its climate was baked over, seconds: real gravity's from version 8, else the hand-set YEAR.
    pub fn year(&self) -> f64 {
        if self.version == VERSION || self.version == 10 || self.version == 9 || self.version == 8 { crate::world::year_of(self.n, crate::world::REAL) } else { crate::sky::YEAR }
    }
    /// The map of a bake on plates once its `life` stage has run (`run_with`).
    fn of_life(seed: i32, n: i32, mut b: land::Bake, l: life::Life, volcanoes: Vec<volcano::Volcano>, (table, solve): (Vec<f32>, (u32, f32)), t: std::time::Instant) -> Map {
        let h = b.h.clone();
        let lakes = (0..b.g.n()).filter(|&c| b.lake(c)).map(|c| (c as u32, b.lvl[c])).collect();
        let flow = l.flow.clone();
        let spawn = spawn(&b, &l);
        b.life = Some(l);
        let biome = std::mem::take(&mut b.life.as_mut().unwrap().biome);
        let climate = Some(b.life.as_ref().unwrap().climate.clone());
        let rivers = b.rivers.iter().filter(|r| b.land(r.cell as usize)).map(|r| Reach { cell: r.cell, to: r.to, area: r.area, flow: flow[r.cell as usize] }).collect();
        let rock = std::mem::take(&mut b.rock);
        let version = if table.is_empty() { 9 } else { VERSION };
        let mut map = Map { version, seed, n, m: b.g.m as u32, h, biome, spawn, rivers, lakes, climate, rock, volcanoes, table, solve, ..Map::empty() };
        map.finish(&map.to_bytes());
        eprintln!("[bake] seed {seed} planet {n}: {:.1} s", t.elapsed().as_secs_f64());
        map
    }
    /// The bake of version 5, before the peaks were stretched (ADR 0105): what a save made on one plays on.
    pub fn bake_v5(seed: i32, n: i32) -> Map {
        let mut m = run_with(&Stage::TERRA_ICELESS, seed, n, &land::Params { peak_extra_m: 0.0, volcanoes: false, ground_water: false, coarse_air: false, ..Default::default() }, crate::sky::YEAR);
        m.version = 5;
        m.finish(&m.to_bytes());
        m
    }
    /// The bake of version 6, before the polar caps (ADR 0107; Terra's stages without `ice`): what a save made on
    /// one plays on.
    pub fn bake_v6(seed: i32, n: i32) -> Map {
        let mut m = run_with(&Stage::TERRA_ICELESS, seed, n, &land::Params { volcanoes: false, ground_water: false, coarse_air: false, ..Default::default() }, crate::sky::YEAR);
        m.version = 6;
        m.finish(&m.to_bytes());
        m
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
            4 => postcard::to_stdvec(&(4u32, self.seed, self.n, self.m, &self.h, &self.biome, self.spawn, &self.rivers, &self.lakes, &self.climate)),
            luna::VERSION | mars::VERSION => postcard::to_stdvec(&(self, &self.tubes)),
            VERSION | 10 => postcard::to_stdvec(&(self, &self.volcanoes, &self.table, self.solve)),
            9 => postcard::to_stdvec(&(self, &self.volcanoes)),
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
            8 | 6 | 5 | icy::VERSION => postcard::from_bytes::<Map>(b).map_err(|e| format!("not a map: {e}"))?,
            9 => {
                let (m, rest) = postcard::take_from_bytes::<Map>(b).map_err(|e| format!("not a map: {e}"))?;
                Map { volcanoes: postcard::from_bytes(rest).map_err(|e| format!("not a map's volcanoes: {e}"))?, ..m }
            }
            VERSION | 10 => {
                let (m, rest) = postcard::take_from_bytes::<Map>(b).map_err(|e| format!("not a map: {e}"))?;
                let (volcanoes, rest) = postcard::take_from_bytes(rest).map_err(|e| format!("not a map's volcanoes: {e}"))?;
                let (table, solve) = postcard::from_bytes(rest).map_err(|e| format!("not a map's water table: {e}"))?;
                Map { volcanoes, table, solve, ..m }
            }
            luna::VERSION | mars::VERSION => {
                let (m, rest) = postcard::take_from_bytes::<Map>(b).map_err(|e| format!("not a map: {e}"))?;
                Map { tubes: postcard::from_bytes(rest).map_err(|e| format!("not a map's tubes: {e}"))?, ..m }
            }
            4 => {
                let v: V4 = postcard::from_bytes(b).map_err(|e| format!("not a map: {e}"))?;
                Map { version: 4, seed: v.seed, n: v.n, m: v.m, h: v.h, biome: v.biome, spawn: v.spawn, rivers: v.rivers, lakes: v.lakes, climate: v.climate, ..Map::empty() }
            }
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
        // a planet the engine can make (`Body::new`), its cells' indices within u32: the largest real planet,
        // 12288 columns, has m = 1536
        if !(1..=4096).contains(&m.m) || !(1..=32768).contains(&m.n) || m.n % crate::world::CS != 0 {
            return Err(format!("a map of {} columns and {} cells per face edge", m.n, m.m));
        }
        let cells = 6 * m.m as usize * m.m as usize;
        if let Some(c) = &m.climate {
            c.check()?;
        }
        let known = m.version == VERSION || m.version == 10 || m.version == 9 || m.version == 8 || m.version == 7 || m.version == luna::VERSION || m.version == mars::VERSION || m.version == icy::VERSION || m.version == 6 || m.version == 5 || m.version == 4 || m.version == 3 || (m.version == 2 && m.climate.is_none()) || (m.version == 1 && m.rivers.is_empty());
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
            || m.lakes.windows(2).any(|w| w[0].0 >= w[1].0)
            || m.tubes.iter().any(|&(c, d, r)| c as usize >= cells || !(d.is_finite() && d.abs() < 64.0) || !(r > 0.0 && r < 16.0))
            || m.volcanoes.iter().any(|v| !v.sane(cells))
            || (m.version == VERSION || m.version == 10) != (m.table.len() == cells)
            || m.table.iter().any(|t| !t.is_finite());
        // Luna's maps (`luna::VERSION`) hold no rock provinces: her rock is her kinds' (`Map::rock`); a Terra map's
        // are Terra's; a planet's crust (`mars::VERSION`) and an icy body's (`icy::VERSION`) have none yet (their
        // provinces are later runs')
        let rocks = if m.version >= 5 && ![luna::VERSION, mars::VERSION, icy::VERSION].contains(&m.version) { m.rock.len() == cells && m.rock.iter().all(|&r| r <= rock::OCEAN) } else { m.rock.is_empty() };
        let kinds = match m.version {
            mars::VERSION => m.biome.iter().all(|k| (mars::HIGHLAND..=mars::FROST).contains(k)),
            icy::VERSION => m.biome.iter().all(|k| (icy::PLAIN..=icy::FROST).contains(k)) && m.tubes.is_empty(),
            _ => true,
        };
        if !known || bad || !rocks || !kinds || m.h.len() != cells || m.biome.len() != cells {
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
        Some(theta - air::Params::default().cool(self.h[(face * m + j) * m + i]))
    }
    /// The slope (rise over run) of cell (f, i, j), from its four neighbours (across the seams too).
    fn cell_slope(&self, f: usize, i: i32, j: i32) -> f32 {
        let m = self.m as i32;
        let at = |i: i32, j: i32| {
            let (f, i, j) = fold(f, i, j, m);
            self.h[(f * m as usize + j as usize) * m as usize + i as usize]
        };
        (at(i + 1, j) - at(i - 1, j)).hypot(at(i, j + 1) - at(i, j - 1)) / (2.0 * self.n as f32 / m as f32)
    }
    /// The slope of the cell that holds face point (x, y) in cells: the ground's lie (a bed under water, ADR 0086).
    pub fn slope(&self, face: usize, x: f64, y: f64) -> f32 {
        let m = self.m as i32;
        self.cell_slope(face, (x.floor() as i32).clamp(0, m - 1), (y.floor() as i32).clamp(0, m - 1))
    }
    /// Every cell's `rough`, from its slope.
    fn roughs(&self) -> Vec<f32> {
        let m = self.m as i32;
        (0..self.h.len())
            .map(|c| {
                let (f, i, j) = (c / (m * m) as usize, c as i32 % m, (c as i32 / m) % m);
                rough(self.cell_slope(f, i, j), self.biome[c], self.h[c])
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
    /// Whether it is an airless crust's map, Luna's, a planet's (Mars's) or an icy body's: no sea on it.
    pub fn crust(&self) -> bool {
        [luna::VERSION, mars::VERSION, icy::VERSION].contains(&self.version)
    }
    /// The rock province of the cell that holds face point (x, y), in cells; None on a map from before the rock.
    /// Luna's from her kinds (`luna::province`; a world asks it from generation 4, `Body::rock_at`).
    pub fn rock(&self, face: usize, x: f64, y: f64) -> Option<u8> {
        if self.version == luna::VERSION {
            return Some(luna::province(self.biome(face, x, y)));
        }
        if self.rock.is_empty() {
            return None;
        }
        let m = self.m as i32;
        let (f, i, j) = fold(face, x.floor() as i32, y.floor() as i32, m);
        Some(self.rock[(f * m as usize + j as usize) * m as usize + i as usize])
    }
    /// The cell index that holds face point (x, y), in cells (folded over the seams).
    pub fn cell(&self, face: usize, x: f64, y: f64) -> usize {
        let m = self.m as i32;
        let (f, i, j) = fold(face, x.floor() as i32, y.floor() as i32, m);
        (f * m as usize + j as usize) * m as usize + i as usize
    }
    /// The water table at face point (x, y) in cells, metres over the sea (ADR 0190): bilinear between the four cells'
    /// middles round it, the nearest cell's where one of them is on tight rock; None there, or on a map without one.
    pub fn table_at(&self, face: usize, x: f64, y: f64) -> Option<f32> {
        if self.table.is_empty() {
            return None;
        }
        let near = self.table[self.cell(face, x, y)];
        if near == ground::DRY {
            return None;
        }
        let (x0, y0) = ((x - 0.5).floor(), (y - 0.5).floor());
        let (fx, fy) = ((x - 0.5 - x0) as f32, (y - 0.5 - y0) as f32);
        let t = |dx: f64, dy: f64| self.table[self.cell(face, x0 + dx + 0.5, y0 + dy + 0.5)];
        let c = [t(0.0, 0.0), t(1.0, 0.0), t(0.0, 1.0), t(1.0, 1.0)];
        if c.contains(&ground::DRY) {
            return Some(near);
        }
        Some((c[0] * (1.0 - fx) + c[1] * fx) * (1.0 - fy) + (c[2] * (1.0 - fx) + c[3] * fx) * fy)
    }
    /// The biome of the cell that holds face point (x, y), in cells.
    pub fn biome(&self, face: usize, x: f64, y: f64) -> u8 {
        let m = self.m as i32;
        let (f, i, j) = fold(face, x.floor() as i32, y.floor() as i32, m);
        self.biome[(f * m as usize + j as usize) * m as usize + i as usize]
    }
}

/// A stage of the one bake (0165-bake-stages; planet-model.md section 3): a body's row lists the stages it
/// runs, in their order (`data/bodies.toml` `stages`), and `run` calls each on what the ones before it made.
/// Two crusts are built: `plates` (Terra's: plates, boundaries, their uplift) and `crust` (Luna's: an old crust
/// without plates); `check` says which stage needs which before it.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    Plates,
    Crust,
    Impacts,
    Volcanism,
    Erosion,
    Ice,
    Climate,
    Life,
}

impl Stage {
    /// Terra's bake (ADR 0043): her ice is the polar caps, which the `life` stage lays (ADR 0107).
    pub const TERRA: [Stage; 5] = [Stage::Plates, Stage::Erosion, Stage::Climate, Stage::Ice, Stage::Life];
    /// Terra's bake before the polar caps (map versions 5 and 6).
    pub const TERRA_ICELESS: [Stage; 4] = [Stage::Plates, Stage::Erosion, Stage::Climate, Stage::Life];
    /// Luna's bake (ADR 0070): her maria flood among the impacts when `volcanism` runs, which then lays the
    /// thorium province and the lava tubes; her ice is the polar shadow.
    pub const LUNA: [Stage; 4] = [Stage::Crust, Stage::Impacts, Stage::Ice, Stage::Volcanism];
    fn name(self) -> String {
        format!("{self:?}").to_lowercase()
    }
}

/// Whether a list of stages can be baked on a `moon` (a body that circles another) or not: it starts with a crust
/// (`plates` or `crust`), names no stage twice, and every stage has what it needs before it. What a stage needs is
/// what today's code reads: on plates, `erosion` then `climate` then `life` (whose biomes are the map's ground), and
/// `ice` is the caps `life` lays, so it comes between `climate` and `life`; on a crust without plates, `impacts`,
/// then `volcanism` and `ice`, and on a planet's (`mars`, a body that circles the sun) `erosion` too: its dry beds
/// cut what the impacts made. The other pairs (craters on plates, air or life on a crust without plates, rivers on a
/// moon's) are not built yet.
pub fn check(stages: &[Stage], moon: bool) -> Result<(), String> {
    use Stage::*;
    let plates = match stages.first() {
        Some(Plates) => true,
        Some(Crust) => false,
        _ => return Err(format!("a bake starts with 'plates' or 'crust', not {:?}", stages.iter().map(|s| s.name()).collect::<Vec<_>>())),
    };
    for (k, &s) in stages.iter().enumerate() {
        let (before, after) = (&stages[..k], &stages[k + 1..]);
        if before.contains(&s) {
            return Err(format!("stage '{}' is listed twice", s.name()));
        }
        let need: &[Stage] = match (s, plates) {
            (Plates | Crust, _) if k > 0 => return Err(format!("stage '{}' is a crust: only the first stage", s.name())),
            (Plates | Crust, _) => &[],
            (Erosion, true) => &[Plates],
            (Climate, true) => &[Erosion],
            (Life, true) => &[Climate],
            (Ice, true) if !after.contains(&Life) => return Err("stage 'ice' on plates is the caps the 'life' stage lays: 'life' after it".into()),
            (Ice, true) => &[Climate],
            (Impacts, false) => &[Crust],
            (Volcanism | Ice, false) => &[Impacts],
            (Erosion, false) if !moon => &[Impacts],
            (s, true) => return Err(format!("stage '{}' is not built yet on plates", s.name())),
            (Erosion, false) => return Err("stage 'erosion' is not built yet on a crust without plates of a moon (a planet's is bake/mars.rs)".into()),
            (s, false) => return Err(format!("stage '{}' is not built yet on a crust without plates", s.name())),
        };
        if let Some(n) = need.iter().find(|n| !before.contains(n)) {
            return Err(format!("stage '{}' needs '{}' before it", s.name(), n.name()));
        }
    }
    if plates && !stages.contains(&Life) {
        return Err("a bake on plates ends with 'life': its map's ground is the biomes".into());
    }
    Ok(())
}

/// The one bake: `stages` (`check`ed) of the body of `n` columns from `seed`; `sun_high` is that body's sun at
/// its highest over its equator, radians, which an airless body's `ice` reads (`luna::sun_high` of its row).
pub fn run(stages: &[Stage], seed: i32, n: i32, sun_high: f64) -> Map {
    match stages.first() {
        Some(Stage::Crust) => luna::run(stages, seed, n, sun_high).0,
        _ => run_with(stages, seed, n, &land::Params::default(), crate::world::year_of(n, crate::world::REAL)),
    }
}

/// `run` of a list on plates (Terra's kind), with other land parameters, its climate over `year` seconds.
pub fn run_with(stages: &[Stage], seed: i32, n: i32, p: &land::Params, year: f64) -> Map {
    use Stage::*;
    check(stages, false).unwrap_or_else(|e| panic!("a bake of {stages:?}: {e}"));
    assert_eq!(stages[0], Plates, "run_with bakes on plates; a crust without them is `luna::run`");
    let (t, s) = (std::time::Instant::now(), seed as u32 as u64);
    let lp = if stages.contains(&Ice) { life::LifeParams::default() } else { life::LifeParams::capless() };
    let lp = life::LifeParams { year, air_coarse: if p.coarse_air { lp.air_coarse } else { 1 }, ..lp };
    let (mut crust, mut plates, mut year, mut life, mut volcanoes) = (None, None, None, None, Vec::new());
    for &st in stages {
        match st {
            Plates => crust = Some(land::crust(n as u32, s, p)),
            Erosion => {
                let mut b = land::erosion(crust.take().expect("checked"), n as u32, s, p);
                // the cones on the land before the air and the life see it (map version 9 on)
                if p.volcanoes {
                    volcanoes = volcano::place(&mut b, n as u32, s);
                }
                plates = Some(b);
            }
            Climate => year = Some(life::climate(plates.as_ref().expect("checked"), &lp)),
            Life => life = Some(life::life_of(plates.as_ref().expect("checked"), s, &lp, year.take().expect("checked"))),
            // on plates, the caps the life stage lays by `lp`
            Ice => {}
            Crust | Impacts | Volcanism => unreachable!("checked: not built on plates"),
        }
    }
    let (mut b, l) = (plates.expect("checked"), life.expect("checked"));
    // the water table on the rivers, lakes, rain and rock, and the streams its springs start (map version 10 on)
    let table = if p.ground_water {
        let (table, springs, solve) = ground::table(&b, &l, n as u32, lp.pet_per_c);
        let streams = ground::streams(&b, &springs);
        b.rivers.extend(streams);
        lap("ground water");
        (table, solve)
    } else {
        (Vec::new(), (0, 0.0))
    };
    Map::of_life(seed, n, b, l, volcanoes, table, t)
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
const STAGES: [(&str, f32); 14] = [
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
    ("ground water", 29.0),
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
    // BAKE_LAPS=1: each stage's seconds on stderr, as it ends (what to speed up: `bake_laps`, ignored)
    thread_local!(static LAST: std::cell::Cell<Option<std::time::Instant>> = const { std::cell::Cell::new(None) });
    if std::env::var_os("BAKE_LAPS").is_some() {
        let now = std::time::Instant::now();
        if let Some(t) = LAST.replace(Some(now)) {
            eprintln!("[bake] lap {name}: {:.2} s", (now - t).as_secs_f64());
        } else {
            eprintln!("[bake] lap {name} (the first: no time before it)");
        }
    }
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
    find_in(seed, n, hash, if hash.is_some() { &[VERSION, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, luna::VERSION] } else { &[VERSION] }).or_else(|| mars::find(seed, n, hash?))
}

/// `find` among maps of these versions.
fn find_in(seed: i32, n: i32, hash: Option<u64>, versions: &[u32]) -> Option<Arc<Map>> {
    let ok = |m: &Map| hash.map_or(true, |h| h == m.hash);
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
    if map.version == mars::VERSION || map.version == icy::VERSION {
        return mars::keep_any(map);
    }
    write(&file_v(map.seed, map.n, map.version), &map);
    let m = Arc::new(map);
    LOADED.lock().unwrap().insert((m.seed, m.n, m.version), Arc::downgrade(&m));
    m
}

/// Writes a map's bytes to `path` through a temporary file (a failed write only costs the next start a bake).
fn write(path: &std::path::Path, map: &Map) {
    // one temporary file a process: two baking the same map never rename each other's half-written file
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    let wrote = std::fs::create_dir_all(DIR).and_then(|_| std::fs::write(&tmp, map.to_bytes())).and_then(|_| std::fs::rename(&tmp, path));
    if let Err(e) = wrote {
        eprintln!("[bake] could not keep {}: {e}", path.display());
    }
}

/// The map a save names by `hash`: from memory or disk, else baked now (a version 6 map is this bake without the
/// polar caps, a version 5 map is this bake without the
/// stretched peaks, a 4 that without its rock, a 3 without its lakes and flows, a 2 without its climate too, a 1
/// without its rivers; none comes back from a bake of another climate); None if the bake makes another map.
pub fn for_hash(seed: i32, n: i32, hash: u64) -> Option<Arc<Map>> {
    if let Some(m) = find(seed, n, Some(hash)) {
        return Some(m);
    }
    let b = keep(Map::bake(seed, n));
    if b.hash == hash {
        return Some(b);
    }
    let b = Map::bake_v10(seed, n);
    if b.hash == hash {
        return Some(keep(b));
    }
    let b = Map::bake_v9(seed, n);
    if b.hash == hash {
        return Some(keep(b));
    }
    let b = Map::bake_v8(seed, n);
    if b.hash == hash {
        return Some(keep(b));
    }
    let b = Map::bake_v7(seed, n);
    if b.hash == hash {
        return Some(keep(b));
    }
    let b = Map::bake_v6(seed, n);
    if b.hash == hash {
        return Some(keep(b));
    }
    let b = Map::bake_v5(seed, n);
    if b.hash == hash {
        return Some(keep(b));
    }
    // the older layouts of this bake, one at a time: no rock provinces (4); a lake a flat at its level, no lakes
    // or flows (3); no climate (2); no rivers (1)
    let mut flat = b.h.clone();
    for &(c, l) in &b.lakes {
        flat[c as usize] = l;
    }
    let flowless = || b.rivers.iter().map(|r| Reach { flow: 0.0, ..*r }).collect::<Vec<Reach>>();
    let layout = |version: u32| {
        let old = Map { version, seed, n, m: b.m, h: flat.clone(), biome: b.biome.clone(), spawn: b.spawn, ..Map::empty() };
        let old = match version {
            4 => Map { h: b.h.clone(), rivers: b.rivers.clone(), lakes: b.lakes.clone(), climate: b.climate.clone(), ..old },
            3 => Map { rivers: flowless(), climate: b.climate.clone(), ..old },
            2 => Map { rivers: flowless(), ..old },
            _ => old,
        };
        Map::from_bytes(&old.to_bytes()).ok().filter(|m| m.hash == hash)
    };
    [4, 3, 2, 1].into_iter().find_map(layout).map(keep)
}

/// The map of (seed, n): from memory, from disk, else baked now and kept.
pub fn get(seed: i32, n: i32) -> Arc<Map> {
    find(seed, n, None).unwrap_or_else(|| keep(Map::bake(seed, n)))
}

/// `get` for a world of generation `gen`: before the ground water (`world::GROUND_WATER`) the map of version 9, before
/// the volcanoes (`world::VOLCANOES`) the map of version 8, before real gravity (`world::REAL`) the map of version 7.
pub fn get_at(seed: i32, n: i32, gen: u8) -> Arc<Map> {
    if gen >= crate::world::COARSE_AIR {
        return get(seed, n);
    }
    if gen >= crate::world::GROUND_WATER {
        return find_in(seed, n, None, &[10]).unwrap_or_else(|| keep(Map::bake_v10(seed, n)));
    }
    if gen >= crate::world::VOLCANOES {
        return find_in(seed, n, None, &[9]).unwrap_or_else(|| keep(Map::bake_v9(seed, n)));
    }
    if gen >= crate::world::REAL {
        return find_in(seed, n, None, &[8]).unwrap_or_else(|| keep(Map::bake_v8(seed, n)));
    }
    find_in(seed, n, None, &[7]).unwrap_or_else(|| keep(Map::bake_v7(seed, n)))
}

/// One bake of (seed, n) for every test that only reads it: baked once per test run, kept in memory only (never
/// in saves/bakes/); two sizes or seeds bake side by side. A test that changes its map copies it first.
#[cfg(test)]
pub(crate) fn shared(seed: i32, n: i32) -> Arc<Map> {
    static MAPS: LazyLock<Mutex<HashMap<(i32, i32), Arc<std::sync::OnceLock<Arc<Map>>>>>> = LazyLock::new(Mutex::default);
    let cell = MAPS.lock().unwrap().entry((seed, n)).or_default().clone();
    cell.get_or_init(|| Arc::new(Map::bake(seed, n))).clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small() -> Map {
        let mut m = Map { seed: 1, n: 64, m: 8, h: (0..384).map(|c| (c % 8) as f32).collect(), biome: vec![3; 384], rock: vec![1; 384], table: (0..384).map(|c| (c % 8) as f32 - 1.0).collect(), ..Map::empty() };
        m.finish(&m.to_bytes());
        m
    }

    /// A map keeps its rock provinces (version 5, ADR 0063); a province past the list is refused, and a map
    /// of version 4 reads with none, so a world played on one keeps its plain stone.
    #[test]
    fn a_map_keeps_its_rock_and_an_older_one_has_none() {
        let mut m = small();
        let back = Map::from_bytes(&m.to_bytes()).unwrap();
        assert_eq!((back.version, back.rock(0, 0.5, 0.5)), (VERSION, Some(1)));
        m.rock[5] = rock::NAMES.len() as u8;
        assert!(Map::from_bytes(&m.to_bytes()).is_err(), "a province past the list");
        m.rock[5] = rock::LUNA_HIGHLAND;
        assert!(Map::from_bytes(&m.to_bytes()).is_err(), "a province of Luna's on a Terra map");
        m.rock.clear();
        m.version = 4;
        let v4 = Map::from_bytes(&m.to_bytes()).unwrap();
        assert_eq!((v4.version, v4.rock(0, 0.5, 0.5)), (4, None));
        assert_eq!(v4.hash, crate::world::fnv(crate::world::FNV0, &m.to_bytes()), "an old map's hash holds");
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
        let a = reporting(p.clone(), || Map::bake(5, 256));
        stop.send(()).unwrap();
        let seen = watch.join().unwrap();
        assert!(seen.windows(2).all(|w| w[1].1 >= w[0].1), "the fraction falls: {seen:?}");
        assert_eq!(seen.last().unwrap().1, 1.0);
        let mut stages: Vec<&str> = seen.iter().map(|s| s.0).collect();
        stages.dedup();
        assert!(stages.len() >= 5, "stages seen: {stages:?}");
        assert!(seen.iter().any(|s| s.1 > 0.3 && s.1 < 0.7), "the bar moves within the erosion: {stages:?}");
        assert_eq!(a.hash, shared(5, 256).hash);
    }

    /// A map of version 1 (an old save's, fetched from a server) is kept as the bytes it came in, in its own
    /// file; a new game never gets it; a save that names one whose file is gone gets it back from a bake.
    #[test]
    fn a_version_1_map_is_kept_apart_and_baked_back_for_its_save() {
        let fresh = Map::bake_v5(-77, 256);
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

    /// Every map a save can still name (6, 5, 4, 3, 2, 1, and Luna's) is baked back by its hash when its file is gone.
    #[test]
    fn every_version_a_save_names_is_baked_back_without_its_file() {
        let (seed, n) = (-79, 256);
        let gone = || {
            for v in [1, 2, 3, 4, 5, 6, VERSION, luna::VERSION] {
                let _ = std::fs::remove_file(file_v(seed, n, v));
                LOADED.lock().unwrap().remove(&(seed, n, v));
            }
        };
        let new = Map::bake(seed, n);
        let b = Map::bake_v5(seed, n);
        assert_ne!(new.h, b.h, "the peaks stretched");
        let mut flat = b.h.clone();
        for &(c, l) in &b.lakes {
            flat[c as usize] = l;
        }
        let rivers = || b.rivers.iter().map(|r| (r.cell, r.to, r.area)).collect::<Vec<_>>();
        let layouts = [
            (VERSION, new.to_bytes()),
            (6, Map::bake_v6(seed, n).to_bytes()),
            (5, b.to_bytes()),
            (4, postcard::to_stdvec(&(4u32, seed, n, b.m, &b.h, &b.biome, b.spawn, &b.rivers, &b.lakes, &b.climate)).unwrap()),
            (3, postcard::to_stdvec(&(3u32, seed, n, b.m, &flat, &b.biome, b.spawn, rivers(), &b.climate)).unwrap()),
            (2, postcard::to_stdvec(&(2u32, seed, n, b.m, &flat, &b.biome, b.spawn, rivers())).unwrap()),
            (1, postcard::to_stdvec(&(1u32, seed, n, b.m, &flat, &b.biome, b.spawn)).unwrap()),
        ];
        assert!(!b.rock.is_empty(), "the bake has what 5 adds");
        for (v, bytes) in layouts {
            let h = Map::from_bytes(&bytes).unwrap().hash;
            gone();
            assert_eq!(for_hash(seed, n, h).map(|m| (m.version, m.hash)), Some((v, h)), "version {v}");
        }
        let l = luna::bake(seed, 64).hash;
        let _ = std::fs::remove_file(file_v(seed, 64, luna::VERSION));
        LOADED.lock().unwrap().remove(&(seed, 64, luna::VERSION));
        assert_eq!(luna::for_hash(seed, 64, l).map(|m| (m.version, m.hash)), Some((luna::VERSION, l)), "Luna's");
        gone();
        let _ = std::fs::remove_file(file_v(seed, 64, luna::VERSION));
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

    /// A map's size from a server is refused, not a panic, when nothing can be built on it: no cells, so many
    /// that a cell's index overflows, a planet the engine cannot make.
    #[test]
    fn a_map_of_an_impossible_size_is_refused() {
        let river = vec![Reach { cell: 9, to: 10, area: 2e5, flow: 0.0 }];
        for (n, mm) in [(64, 65536), (64, 0), (64, 4097), (0, 8), (-64, 8), (65, 8), (1 << 20, 8)] {
            let m = Map { n, m: mm, rivers: river.clone(), ..small() };
            assert!(Map::from_bytes(&m.to_bytes()).is_err(), "n {n}, m {mm}");
        }
        assert!(Map::from_bytes(&Map { rivers: river, ..small() }.to_bytes()).is_ok());
    }

    /// The map keeps each cell's baked temperature (W3): the climate it holds gives back the bake's own
    /// `life::Life::temp` at every cell that is not a lake, after a round trip through its bytes too.
    #[test]
    fn the_map_keeps_each_cells_baked_temperature() {
        let (seed, n) = (5, 256);
        let b = land::bake(n as u32, seed as u32 as u64, &land::Params::default());
        let map = Map::from_bytes(&shared(seed, n).to_bytes()).unwrap();
        let l = life::life(&b, seed as u32 as u64, &life::LifeParams { year: map.year(), ..life::LifeParams::default() });
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

    /// ADR 0107: both poles carry a cap of ice (biome ICE) on land and on the sea. On the standard planet of two
    /// seeds (the developer's world's and the tests' 1337; a smaller planet's coarse air has no cold poles) it
    /// covers 4 to 14 % of the planet, nearly all of it poleward of 55 degrees, and most of each pole's last 10.
    #[test]
    fn both_poles_carry_an_ice_cap_on_land_and_sea() {
        for (seed, n) in [(933105400, 6144), (1337, 6144)] {
            let m = get(seed, n);
            let g = grid::Grid::new(m.m as usize, 1.0);
            let lakes: std::collections::HashSet<u32> = m.lakes.iter().map(|l| l.0).collect();
            let ocean = |c: usize| m.h[c] < 0.0 && !lakes.contains(&(c as u32));
            let lat = |c: usize| g.d(c as u32).z.clamp(-1.0, 1.0).asin().to_degrees();
            let area = |f: &dyn Fn(usize) -> bool| (0..g.n()).filter(|&c| f(c)).map(|c| g.area(c as u32) as f64).sum::<f64>();
            let all = area(&|_| true);
            let ice = |c: usize| m.biome[c] == life::ICE;
            let share = area(&ice) / all;
            assert!((0.04..0.14).contains(&share), "seed {seed} planet {n}: ice on {:.1} % of the planet", share * 100.0);
            let polar = area(&|c| ice(c) && lat(c).abs() > 55.0) / area(&ice);
            assert!(polar > 0.9, "seed {seed} planet {n}: {:.1} % of the ice poleward of 55 degrees", polar * 100.0);
            for pole in [1.0, -1.0] {
                let cap = area(&|c| ice(c) && lat(c) * pole > 80.0) / area(&|c| lat(c) * pole > 80.0);
                assert!(cap > 0.8, "seed {seed} planet {n}: the pole at {pole} is {:.0} % ice", cap * 100.0);
            }
            assert!(area(&|c| ice(c) && ocean(c)) > 0.0 && area(&|c| ice(c) && !ocean(c)) > 0.0, "seed {seed} planet {n}: ice on land and sea");
        }
    }

    /// A map of version 2 (before the climate, a save's) reads as it was, without a climate, and gives its own
    /// bytes back; a climate cut short is refused.
    #[test]
    fn a_version_2_map_reads_without_a_climate_and_a_cut_climate_is_refused() {
        let fresh = shared(5, 256);
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
        let mut cut = Map::from_bytes(&fresh.to_bytes()).unwrap();
        cut.climate.as_mut().unwrap().state.vap.pop();
        assert!(Map::from_bytes(&cut.to_bytes()).is_err(), "a state that does not fit its grid");
        let mut nan = Map::from_bytes(&fresh.to_bytes()).unwrap();
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

    /// ADR 0105: the peaks stretched, the lowland as it was. On every planet size the highest land is the
    /// taste call's 120 m and the planet's share of `peak_extra_m`; land under `land::STRETCH_FROM` is the
    /// version 5 bake's to the bit, and so are the rivers.
    #[test]
    fn the_peaks_rise_and_the_lowland_stays_to_the_bit() {
        for (n, lo, hi) in [(768, 140.0, 160.0), (1536, 160.0, 190.0)] {
            let (new, old) = (Map::bake_v8(3, n), Map::bake_v5(3, n));
            let top = |m: &Map| m.h.iter().fold(f32::MIN, |a, &h| a.max(h));
            assert!((lo..hi).contains(&top(&new)), "planet {n}: highest {} m, the old {} m", top(&new), top(&old));
            let low = (0..old.h.len()).filter(|&c| old.h[c] <= land::STRETCH_FROM).count();
            assert!(low > old.h.len() * 9 / 10, "planet {n}: most of it lowland");
            for c in 0..old.h.len() {
                if old.h[c] <= land::STRETCH_FROM {
                    assert_eq!(new.h[c].to_bits(), old.h[c].to_bits(), "planet {n}, cell {c}");
                } else {
                    assert!(new.h[c] >= old.h[c], "planet {n}, cell {c} sank");
                }
            }
            let reach = |m: &Map| m.rivers.iter().map(|r| (r.cell, r.to)).collect::<Vec<_>>();
            assert_eq!(reach(&new), reach(&old), "planet {n}: the same rivers");
            let cells = |m: &Map| m.lakes.iter().map(|l| l.0).collect::<Vec<_>>();
            assert_eq!(cells(&new), cells(&old), "planet {n}: the same lakes");
        }
    }

    /// The 4 km planet's highest peak reaches the clouds' base (250 m, clouds.wgsl `CL_BASE`) by 100 m or so.
    #[test]
    fn the_standard_planets_highest_peak_reaches_into_the_clouds() {
        let m = get(1337, 6144);
        let top = m.h.iter().fold(f32::MIN, |a, &h| a.max(h));
        assert!((320.0..380.0).contains(&top), "highest {top} m");
    }

    /// A lock taken on the code before the bake became stages (0165-bake-stages): Terra's map of every version
    /// a bake still makes, and Luna's, to the bit.
    #[test]
    fn the_stages_bake_terra_and_luna_to_the_bit_as_before() {
        let got = [Map::bake_v7(5, 256).hash, Map::bake_v6(5, 256).hash, Map::bake_v5(5, 256).hash, luna::bake(7, 416).hash, Map::bake_v8(5, 256).hash, Map::bake_v9(5, 256).hash];
        // (version 8, real gravity's year: taken on its first build, 2026-11-23; `bake_v8` since version 9's volcanoes;
        // version 9 taken on the code before the ground water, 2026-11-24: `bake_v9` since version 10's table)
        assert_eq!(got.map(|h| format!("{h:016x}")), ["16ee32bb94308b73", "065e789cbd17a892", "5bfb7a403005f7d7", "cb20ef2abc8ed3a7", "4875a8c0286c488d", "6f6f417558937668"]);
    }

    /// The whole bake twice on a small planet: the same map to the bit (the second the tests' shared one).
    #[test]
    fn a_small_planet_bakes_the_same_twice() {
        let (a, b) = (Map::bake(5, 256), shared(5, 256));
        assert_eq!(a.hash, b.hash);
        assert!(a.biome.iter().any(|&x| x < life::LAND_BIOMES) && a.h.iter().any(|&h| h < 0.0), "land and sea");
    }

    /// The coarse year and its hand-over to the live air (map version 11; a planet this size is the smallest that
    /// takes them, 12 air cells a face edge on 8): the same map to the bit on the pool and on one thread.
    /// (5 minutes: by hand when the bake's climate changes; passed 2026-11-25.)
    #[ignore]
    #[test]
    fn the_coarse_year_bakes_the_same_on_one_thread() {
        assert_ne!(life::air_cells(1536.0, life::LifeParams::default().air_cell_m, 2), (12, 12), "the coarse path");
        let one = rayon::ThreadPoolBuilder::new().num_threads(1).build().unwrap().install(|| Map::bake(5, 1536));
        assert_eq!(one.hash, shared(5, 1536).hash);
    }

    /// Version 10's map (generations 12 and 13: the year on the live grid) bakes as it did before version 11.
    #[test]
    fn version_10_bakes_as_before() {
        assert_eq!(format!("{:016x}", Map::bake_v10(5, 256).hash), "ac21af4bc7501ea7");
    }
}
