//! The live air grid (W3, `docs/design/air.md`, ADR 0060): the bake's own model (`bake::air`) stepping on
//! with the world's clock on the server, off its thread.
//!
//! - **A step is `DT` seconds of world time.** When the grid is a step or more behind the clock, the server
//!   hands it to a worker thread for up to `MAX_STEPS` steps and takes it back `LAG` ticks later (waiting if
//!   it is not done; on Terra it takes 1.6 ms). What a job does is fixed when it is handed over, and
//!   what the game reads changes only on the tick it is taken back, so the weather is the same on every run
//!   of the same ticks: thread timing never shows.
//! - **Under a time warp it keeps up to `MAX_STEPS` steps a second;** further behind, its clock jumps to the
//!   world's and the weather in between is skipped.
//! - It starts from the map's climate, the grid as the bake's year left it at world time 0, or from a save.
use crate::bake::air::{Air, State, LAYERS};
use crate::bake::grid::Grid;
use crate::bake::Climate;
use crate::world::{fnv, FNV0};
use glam::DVec3;
use std::sync::mpsc::{channel, Receiver};
use std::sync::Mutex;

/// Seconds of world time a live step: the bake's own, so a year of live weather is the bake's year (at 5 s
/// it rained 10 to 20 % less in every biome: W3's rain check). Once every 200 ticks.
pub const DT: f32 = 10.0;
/// Ticks from handing a job over to taking it back.
pub const LAG: u64 = 20;
/// Steps a job takes at most: a second of real time keeps up with a time warp of 80.
pub const MAX_STEPS: usize = 8;
/// How far a biome's mean rain over a year of live weather may lie from the bake's, as a share of the bake's
/// (W3's rain check, `tests::the_bakes_rain_is_a_year_of_live_weather_in_every_biome`).
/// 5 %: a year after the bake's repeats it to 0.1 % in every biome; the live step at 5 s (10 to 20 %) or a bake
/// without seasons (18 % at 60 to 70 degrees, `prototypes/air`) fail it.
pub const RAIN_TOLERANCE: f64 = 0.05;

/// What the game reads of the grid, made on the worker after each job. Per air column:
pub struct View {
    /// the grid's clock, world seconds
    pub t: f64,
    /// the surface air's potential temperature, C at sea level (less `Params::lapse` a metre over the sea)
    pub theta: Vec<f32>,
    /// rain and snow over the job, mm an hour
    pub rain: Vec<f32>,
    /// the share of the sky the column's cloud covers, 0 to 1
    pub cover: Vec<f32>,
    /// the surface wind, east and north, m/s
    pub wind: Vec<[f32; 2]>,
    /// water units (`liquid::U` a block) the column's ground ran off to its rivers over the job: what W5's
    /// reaches take as live rain (air.md, pass 7)
    pub runoff: Vec<u32>,
    /// FNV of all of it: the soak's state hash
    pub hash: u64,
    /// the job's wall time on its worker, ms (not in the hash)
    pub ms: f32,
}

impl View {
    fn of(a: &Air, runoff: Vec<u32>, rain_units: &[u64], steps: usize) -> View {
        let n = a.n;
        let theta: Vec<f32> = (0..n).map(|c| a.theta[c * LAYERS]).collect();
        let secs = (steps.max(1) as f64) * DT as f64;
        let rain: Vec<f32> = (0..n).map(|c| (rain_units[c] as f64 / 4096.0 / a.area[c] as f64 * 1000.0 * 3600.0 / secs) as f32).collect();
        let cover: Vec<f32> = (0..n)
            .map(|c| {
                let per_m2 = a.cloud[c * LAYERS..c * LAYERS + LAYERS].iter().map(|&x| x as u64).sum::<u64>() as f32 / a.area[c];
                per_m2 / (per_m2 + a.p.cover_half)
            })
            .collect();
        let wind: Vec<[f32; 2]> = (0..n).map(|c| a.surface_wind(c).into()).collect();
        let mut h = fnv(FNV0, &a.t.to_bits().to_le_bytes());
        for v in theta.iter().chain(&rain).chain(&cover).chain(wind.iter().flatten()) {
            h = fnv(h, &v.to_bits().to_le_bytes());
        }
        for v in &runoff {
            h = fnv(h, &v.to_le_bytes());
        }
        View { t: a.t, theta, rain, cover, wind, runoff, hash: h, ms: 0.0 }
    }
}

/// The air where someone stands (`Weather::here`), what a client is sent.
#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Default)]
pub struct Here {
    /// the air at that height, C
    pub temp: f32,
    /// rain and snow, mm an hour
    pub rain: f32,
    /// cloud cover, 0 to 1
    pub cover: f32,
    /// the surface wind, east and north, m/s
    pub wind: [f32; 2],
}

enum Job {
    Idle(Box<Air>),
    /// handed over on a tick, back at `due`
    Busy { back: Receiver<(Box<Air>, View)>, due: u64 },
    /// back early (a save asked for the state), not yet shown: shown at `due`
    Back { air: Box<Air>, view: View, due: u64 },
}

pub struct Weather {
    grid: Grid,
    lapse: f32,
    job: Mutex<Option<Job>>,
    view: View,
}

impl Weather {
    /// The live grid at world time 0 from the map's climate, or at the clock of a saved `state`.
    pub fn new(c: &Climate, state: Option<&State>) -> Result<Weather, String> {
        let mut a = c.air()?;
        match state {
            Some(s) => a.restore(s)?,
            // the bake's year ended at its turn: world time 0 to the sun
            None => a.t = 0.0,
        }
        let view = View::of(&a, vec![0; a.n], &vec![0; a.n], 0);
        Ok(Weather { grid: c.grid(), lapse: a.p.lapse, job: Mutex::new(Some(Job::Idle(Box::new(a)))), view })
    }

    /// Once a tick, with the tick's number and the world's time: takes a job back when it is due, hands the
    /// next one over when the grid is a step behind.
    pub fn tick(&mut self, tick: u64, time: f64) {
        let job = self.job.get_mut().unwrap();
        let ready = match job.take() {
            Some(Job::Busy { back, due }) if tick >= due => {
                let (air, view) = back.recv().expect("the air's worker ends with its job");
                Some((air, view))
            }
            Some(Job::Back { air, view, due }) if tick >= due => Some((air, view)),
            Some(Job::Idle(air)) => {
                *job = Some(Job::Idle(air));
                None
            }
            other => {
                *job = other;
                return;
            }
        };
        if let Some((air, view)) = ready {
            self.view = view;
            *job = Some(Job::Idle(air));
        }
        let Some(Job::Idle(mut air)) = job.take() else { unreachable!("idle by now") };
        // a clock set back (/time set): the weather keeps its state and follows the clock from there
        if time < air.t {
            air.t = time;
        }
        let behind = time - air.t;
        if behind < DT as f64 {
            *job = Some(Job::Idle(air));
            return;
        }
        let steps = ((behind / DT as f64) as usize).min(MAX_STEPS);
        if behind > 2.0 * MAX_STEPS as f64 * DT as f64 {
            air.t = time - (MAX_STEPS as f64 * DT as f64);
        }
        let (done, back) = channel();
        std::thread::Builder::new()
            .name("air".into())
            .spawn(move || {
                let t0 = std::time::Instant::now();
                let n = air.n;
                let (mut runoff, mut rain) = (vec![0u32; n], vec![0u64; n]);
                for _ in 0..steps {
                    air.step(DT);
                    for c in 0..n {
                        runoff[c] = runoff[c].saturating_add(air.runoff[c]);
                        rain[c] += air.fell[c] as u64;
                    }
                }
                let view = View { ms: t0.elapsed().as_secs_f32() * 1000.0, ..View::of(&air, runoff, &rain, steps) };
                let _ = done.send((air, view));
            })
            .expect("a thread for the air");
        *job = Some(Job::Busy { back, due: tick + LAG });
    }

    /// The grid's state to save: a job in flight is waited for, and its result still shows only when due.
    pub fn state(&self) -> State {
        let mut job = self.job.lock().unwrap();
        if let Some(Job::Busy { .. }) = job.as_ref() {
            let Some(Job::Busy { back, due }) = job.take() else { unreachable!() };
            let (air, view) = back.recv().expect("the air's worker ends with its job");
            *job = Some(Job::Back { air, view, due });
        }
        match job.as_ref() {
            Some(Job::Idle(a)) | Some(Job::Back { air: a, .. }) => a.state(),
            _ => unreachable!("not busy by now"),
        }
    }

    /// What the game reads now.
    pub fn view(&self) -> &View {
        &self.view
    }

    /// The air column that holds unit direction `d` (an index into the view's vectors).
    pub fn column(&self, d: DVec3) -> usize {
        self.grid.locate(d) as usize
    }

    /// The air at unit direction `d`, `over_sea` metres over the sea level: the surface layer smoothed over
    /// the columns as the bake smooths its climate, less the lapse by the height.
    pub fn here(&self, d: DVec3, over_sea: f64) -> Here {
        let d32 = d.as_vec3().to_array();
        let k = self.grid.locate(DVec3::from_array(d32.map(f64::from)));
        let at = |v: &[f32]| crate::bake::land::upsample_at(&self.grid, d32, k, v);
        let v = &self.view;
        Here { temp: at(&v.theta) - self.lapse * over_sea.max(0.0) as f32, rain: at(&v.rain), cover: at(&v.cover), wind: v.wind[k as usize] }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn climate() -> Climate {
        crate::bake::Map::bake(5, 256).climate.clone().expect("a baked map keeps its climate")
    }

    /// The weather follows the clock a step at a time, shows a job only when it is due, and two runs of the
    /// same ticks give the same weather, whether or not one of them saved in between.
    #[test]
    fn the_live_air_steps_with_the_clock_and_saving_changes_nothing() {
        let c = climate();
        let run = |save_at: Option<u64>| {
            let mut w = Weather::new(&c, None).unwrap();
            let mut hashes = Vec::new();
            for tick in 0..400u64 {
                w.tick(tick, tick as f64 / 20.0);
                if Some(tick) == save_at {
                    let s = w.state();
                    assert!(s.t > 0.0, "the job in flight is in the state");
                }
                hashes.push(w.view().hash);
            }
            (hashes, w.view().t, w.state())
        };
        let (a, t, s) = run(None);
        let (b, _, s2) = run(Some(205));
        assert_eq!(a, b, "a save changes nothing the game reads");
        assert_eq!(s, s2);
        // a step every 10 s (200 ticks), shown 20 ticks after it was handed over
        assert_eq!(a[219], a[0]);
        assert_ne!(a[220], a[219]);
        assert!((t - 10.0).abs() < 1e-9, "the grid's clock after 20 s: {t}");
        // a save carries it on
        let mut w = Weather::new(&c, Some(&s)).unwrap();
        assert_eq!(w.state(), s);
        w.tick(400, 20.0);
        w.tick(420, 20.0);
        assert!((w.view().t - 20.0).abs() < 1e-9);
    }

    /// Far behind the clock (a time warp), the grid jumps to it rather than running days of steps.
    #[test]
    fn far_behind_the_clock_the_air_jumps() {
        let mut w = Weather::new(&climate(), None).unwrap();
        w.tick(0, 10_000.0);
        w.tick(20, 10_000.0);
        assert!((w.view().t - 10_000.0).abs() < 1e-9, "{}", w.view().t);
    }

    /// W3's rain check: per biome, the bake's mean rain (its year at 10 s steps) against the live grid's next
    /// year at `DT` from where the bake left it, both brought to the map's cells the same way. Rows of (biome,
    /// cells, bake mm a day, live mm a day), biomes of at least `min_cells` cells.
    /// Also the live air at the spawn's ground at noon of the four seasons (the views' times), C.
    pub(crate) fn rain_check(seed: i32, n: i32, min_cells: usize) -> (Vec<(u8, usize, f64, f64)>, Vec<f32>) {
        let map = crate::bake::Map::bake(seed, n);
        let c = map.climate.clone().unwrap();
        let mut a = c.air().unwrap();
        a.t = 0.0;
        let steps = (crate::sky::YEAR / DT as f64).round() as usize;
        let mut fell = vec![0f64; a.n];
        let (g, m) = (c.grid(), map.m as usize);
        let spawn = DVec3::from_array(map.spawn);
        let (sf, sx, sy) = crate::bake::grid::Grid::bare(m, c.r).locate_xy(spawn);
        let spawn_h = map.h[(sf * m + (sy as usize).min(m - 1)) * m + (sx as usize).min(m - 1)].max(0.0);
        let mut noons = Vec::new();
        for k in 0..steps {
            let t = k as f64 * DT as f64;
            if [225.0, 23625.0, 48825.0, 72225.0].iter().any(|&noon| t <= noon && noon < t + DT as f64) {
                let now = Climate { theta: (0..a.n).map(|i| a.theta[i * LAYERS]).collect(), ..c.clone() };
                noons.push(now.at(&g, spawn).0 - a.p.lapse * spawn_h);
            }
            a.step(DT);
            for (f, &x) in fell.iter_mut().zip(&a.fell) {
                *f += x as f64;
            }
        }
        let days = crate::sky::YEAR / crate::sky::DAY;
        let live = Climate { rain: (0..a.n).map(|k| (fell[k] / days / 4096.0 / a.area[k] as f64 * 1000.0) as f32).collect(), ..c.clone() };
        let mut by = [(0usize, 0f64, 0f64); 15];
        for (cell, &bio) in map.biome.iter().enumerate() {
            let d = crate::bake::grid::Grid::bare(m, c.r).dir_at(cell / (m * m), (cell % m) as f64 + 0.5, ((cell / m) % m) as f64 + 0.5);
            let row = &mut by[bio as usize];
            row.0 += 1;
            row.1 += c.at(&g, d).2 as f64;
            row.2 += live.at(&g, d).2 as f64;
        }
        let rows = (0..15u8)
            .filter(|&b| by[b as usize].0 >= min_cells)
            .map(|b| {
                let (k, x, y) = by[b as usize];
                (b, k, x / k as f64, y / k as f64)
            })
            .collect();
        (rows, noons)
    }

    /// The bake's mean rain against a year of live weather, per biome (W3): within `RAIN_TOLERANCE` of the
    /// bake's in every biome of 100 cells or more, on the tests' small planet.
    #[test]
    fn the_bakes_rain_is_a_year_of_live_weather_in_every_biome() {
        let (rows, _) = rain_check(5, 256, 100);
        let worst = rows.iter().map(|r| ((r.3 - r.2) / r.2).abs()).fold(0.0, f64::max);
        eprintln!("{rows:?}");
        assert!(rows.len() >= 4, "biomes seen: {rows:?}");
        assert!(worst <= RAIN_TOLERANCE, "worst biome off by {:.1} %: {rows:?}", worst * 100.0);
    }

    /// The same on Terra (seed 1337): `cargo test --release -p engine -- --ignored terra_rain` (a minute).
    #[test]
    #[ignore]
    fn terra_rain_check() {
        let (rows, noons) = rain_check(1337, 6144, 2000);
        eprintln!("the spawn's air at noon, spring summer autumn winter: {noons:?} C");
        for r in &rows {
            eprintln!("{:<18} cells {:>8}  bake {:.3}  live {:.3} mm/day  {:+.1} %", crate::bake::life::BIOMES[r.0 as usize].0, r.1, r.2, r.3, (r.3 / r.2 - 1.0) * 100.0);
        }
        let worst = rows.iter().map(|r| ((r.3 - r.2) / r.2).abs()).fold(0.0, f64::max);
        assert!(worst <= RAIN_TOLERANCE, "worst biome off by {:.1} %", worst * 100.0);
    }

    /// A clock set back (/time set to an earlier hour) does not freeze the weather: it steps on from there.
    #[test]
    fn a_clock_set_back_does_not_freeze_the_air() {
        let mut w = Weather::new(&climate(), None).unwrap();
        w.tick(0, 1000.0);
        w.tick(20, 1000.0);
        assert!((w.view().t - 1000.0).abs() < 1e-9);
        w.tick(40, 20.0);
        w.tick(60, 30.0);
        w.tick(80, 30.0);
        assert!((w.view().t - 30.0).abs() < 1e-9, "the air's clock: {}", w.view().t);
    }

    /// The air someone stands in: warmer at the sea than on a hill, by the lapse.
    #[test]
    fn the_air_here_cools_with_height() {
        let w = Weather::new(&climate(), None).unwrap();
        let d = DVec3::new(0.3, 0.8, 0.2).normalize();
        let (low, high) = (w.here(d, 0.0), w.here(d, 100.0));
        assert!((low.temp - high.temp - 100.0 * w.lapse).abs() < 1e-3, "{low:?} {high:?}");
        assert!((-40.0..50.0).contains(&low.temp));
    }
}
