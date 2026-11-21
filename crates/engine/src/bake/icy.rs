//! An icy moon's crust (planet-model.md step 6c, the giant's icy moon): the bake of a body of kind icy, Luna's
//! stages (`bake::luna`) on a crust of ice and rock, as the real Callisto is:
//! - `crust`: Luna's old crust, rough from the start;
//! - `impacts`: her basins and her power law of craters (the most cratered kind of surface there is), then the two
//!   widest basins made **multi-ring**: their middles flowed flat (a pale palimpsest: ice flows where rock would
//!   have kept a bowl) and rings of troughs round them out to an angle of 1 radian, the crust broken in circles;
//! - `ice`: her polar ground no sun reaches, frost there.
//!
//! Its kinds (`KINDS`, rows of `data/biomes.toml` after Mars's): the **dark plain** (a lag of dust and grit over
//! dirty ice: the ice under the top has gone off into space, what was in it left behind), **fresh ice** (young
//! craters' floors, walls and rays: clean ice dug out from under the dark), the **palimpsest** and the **polar
//! frost**. No volcanism, tubes, rock provinces, ore or caves (those are later runs').
//!
//! What the game keeps is a `bake::Map` of `VERSION` (an icy crust's from 301), kept under its row's name and hash
//! as a planet's crust's (`mars::file`).
use super::luna::{self, Work};
use super::{Map, Stage};
use glam::DVec3;

/// Changes whenever this bake makes other ground or bytes: 300 + its own version.
pub const VERSION: u32 = 301;

/// The kinds, rows of `data/biomes.toml` after Mars's 22..=32.
pub const PLAIN: u8 = 33;
pub const FRESH: u8 = 34;
pub const PALIMPSEST: u8 = 35;
pub const FROST: u8 = 36;
pub const KINDS: [&str; 4] = ["dark plain", "fresh ice", "palimpsest", "polar frost"];

/// A multi-ring basin's rings: the first at `RING0` of the basin's radius, one each `RING_STEP` out, as far as
/// `RING_MAX` radians from its middle; each a trough `TROUGH_DEP` deep and `TROUGH_W` metres to each side.
const RING0: f64 = 1.3;
const RING_STEP: f64 = 0.3;
const RING_MAX: f64 = 1.0;
const TROUGH_DEP: f64 = 2.5;
const TROUGH_W: f64 = 3.0;

/// What the bake measured, for its report and tests.
#[derive(Default)]
pub struct Stats {
    /// Every crater (not the basins): its middle, diameter, and when it fell (0 the oldest, 1 now).
    pub craters: Vec<(DVec3, f64, f64)>,
    /// The multi-ring basins: middle, angular radius and how many rings.
    pub rings: Vec<(DVec3, f64, usize)>,
    /// Every kind's area, m^2.
    pub area: [f64; 4],
    pub seconds: f64,
}

/// The bake of a list (`super::check`ed for a moon; an icy crust lists no volcanism, `bodies::parse`), the stages in
/// its order, and what it measured; `high` is `luna::sun_high` of the body baked (its row's, `bodies::Bake`).
pub fn run(stages: &[Stage], seed: i32, n: i32, high: f64) -> (Map, Stats) {
    use Stage::*;
    super::check(stages, true).unwrap_or_else(|e| panic!("a bake of {stages:?}: {e}"));
    assert!(stages[0] == Crust && !stages.contains(&Volcanism), "an icy crust: {stages:?}");
    let mut w = luna::crust(seed, n);
    let mut stats = Stats::default();
    for &st in &stages[1..] {
        match st {
            Impacts => {
                luna::impacts(&mut w, false);
                stats.rings = rings(&mut w);
            }
            Ice => luna::ice(&mut w, high),
            Plates | Crust | Erosion | Climate | Life | Volcanism => unreachable!("checked: one crust, no plates, air or lava on it"),
        }
    }
    finish(w, stats)
}

/// The two widest basins (far enough apart) made multi-ring: the middle flattened toward its mean and made
/// palimpsest where it was old plain, the rings' troughs cut round it.
fn rings(w: &mut Work) -> Vec<(DVec3, f64, usize)> {
    let r = w.r;
    let mut basins: Vec<(DVec3, f64)> = w.craters.iter().filter(|c| c.basin).map(|c| (c.c, c.d)).collect();
    basins.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut chosen: Vec<(DVec3, f64)> = Vec::new();
    for b in basins {
        if chosen.len() < 2 && chosen.iter().all(|c| c.0.dot(b.0) < 0.7) {
            chosen.push(b);
        }
    }
    let wobble = luna::noise(w.seed, 0x91, 1.0 / 120.0, 2);
    let land = &mut w.land;
    let mut out = Vec::new();
    for (c, d) in chosen {
        let rb = 0.5 * d / r;
        let ang = |dist: f64| 2.0 * (dist / (2.0 * r)).min(1.0).asin();
        let chord = |a: f64| 2.0 * r * (0.5 * a.min(std::f64::consts::PI)).sin() * 1.0001;
        // the middle: the ice flowed flat, toward the mean of its ground
        let (mut sum, mut cells) = (0.0f64, 0.0f64);
        land.cap(c, chord(0.7 * rb), |cell, _| {
            sum += land.h[cell] as f64;
            cells += 1.0;
        });
        let level = sum / cells.max(1.0);
        let (mut h, mut kind) = (std::mem::take(&mut land.h), std::mem::take(&mut land.kind));
        land.cap(c, chord(0.9 * rb), |cell, dist| {
            let x = ang(dist) / rb;
            let k = 0.7 * (1.0 - smooth(0.6, 0.9, x));
            h[cell] = (h[cell] as f64 + (level - h[cell] as f64) * k) as f32;
            if x < 0.75 && kind[cell] == luna::HIGHLAND {
                kind[cell] = PALIMPSEST;
            }
        });
        // the rings: troughs in circles, wandering a little, with low lips
        let count = (0..).take_while(|&k| (RING0 + RING_STEP * k as f64) * rb < RING_MAX).count();
        let outer = (RING0 + RING_STEP * count as f64) * rb;
        land.cap(c, chord(outer), |cell, dist| {
            let a = ang(dist);
            let p = land.g.d(cell as u32) * r;
            let wob = 1.0 + 0.04 * wobble.get_noise_3d(p.x as f32, p.y as f32, p.z as f32) as f64;
            let x = a / (rb * wob);
            let k = ((x - RING0) / RING_STEP).round();
            if !(0.0..count as f64).contains(&k) {
                return;
            }
            let u = (x - (RING0 + RING_STEP * k)).abs() * rb * r;
            let cut = if u < TROUGH_W { -TROUGH_DEP * (1.0 - (u / TROUGH_W).powi(2)) } else if u < 2.0 * TROUGH_W { 0.6 * (1.0 - (u / TROUGH_W - 1.5).abs() * 2.0).max(0.0) } else { 0.0 };
            h[cell] += cut as f32;
        });
        (land.h, land.kind) = (h, kind);
        out.push((c, rb, count));
    }
    out
}

fn smooth(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Luna's kinds as an icy crust's: her fresh ground is clean ice, her shadow frost, every other kind dark plain.
fn kind_of(k: u8) -> u8 {
    match k {
        luna::HIGHLAND_FRESH | luna::MARE_FRESH => FRESH,
        luna::SHADOW => FROST,
        PALIMPSEST => PALIMPSEST,
        _ => PLAIN,
    }
}

/// The map of what the stages made, and what they measured.
fn finish(w: Work, mut stats: Stats) -> (Map, Stats) {
    let Work { seed, n, mut land, craters, t0, .. } = w;
    for k in land.kind.iter_mut() {
        *k = kind_of(*k);
    }
    for c in 0..land.g.n() as u32 {
        stats.area[(land.kind[c as usize] - PLAIN) as usize] += land.g.area(c) as f64;
    }
    stats.craters = craters.iter().filter(|c| !c.basin).map(|c| (c.c, c.d, c.t)).collect();
    let spawn = luna::spawn(&land, luna::CELL);
    let mut map = Map { version: VERSION, seed, n, m: land.g.m as u32, h: land.h, biome: land.kind, spawn, ..Map::empty() };
    map.finish(&map.to_bytes());
    stats.seconds = t0.elapsed().as_secs_f64();
    eprintln!("[bake] an icy crust of seed {seed} planet {n}: {:.1} s", stats.seconds);
    (map, stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bodies::{Bake, BODIES};
    use std::collections::HashMap;
    use std::sync::{Arc, LazyLock, Mutex};

    /// The giant's icy moon's row and its bake.
    fn row() -> (usize, &'static Bake) {
        let i = crate::bodies::find("callisto").unwrap();
        (i, BODIES[i].bake.as_ref().unwrap())
    }

    /// One bake of (seed, n) of its row for the tests that only read it.
    fn shared(seed: i32, n: i32) -> Arc<(Map, Stats)> {
        static MAPS: LazyLock<Mutex<HashMap<(i32, i32), Arc<std::sync::OnceLock<Arc<(Map, Stats)>>>>>> = LazyLock::new(Mutex::default);
        let cell = MAPS.lock().unwrap().entry((seed, n)).or_default().clone();
        cell.get_or_init(|| Arc::new(run(&row().1 .0, seed, n, row().1 .1))).clone()
    }

    /// The bake is the same twice, reads back from its bytes, refuses another body's kinds, and is kept under its
    /// row's name and the hash of its list (as a planet's crust is, `mars::file`), not Terra's, Luna's or Mars's.
    #[test]
    fn an_icy_bake_is_the_same_twice_and_kept_under_its_name() {
        let (_, bake) = row();
        let (seed, n) = (-777, 256);
        let (a, b) = (run(&bake.0, seed, n, bake.1).0, run(&bake.0, seed, n, bake.1).0);
        assert_eq!(a.hash, b.hash, "deterministic");
        let back = Map::from_bytes(&a.to_bytes()).unwrap();
        assert_eq!((back.hash, back.version, &back.biome, back.crust(), back.rock.is_empty()), (a.hash, VERSION, &a.biome, true, true));
        assert!(a.biome.iter().all(|k| (PLAIN..=FROST).contains(k)));
        let mut odd = Map::from_bytes(&a.to_bytes()).unwrap();
        odd.biome[0] = crate::bake::mars::FROST;
        assert!(Map::from_bytes(&odd.to_bytes()).is_err(), "a kind not an icy crust's is refused");
        let p = bake.2.as_ref().unwrap();
        let path = bake.file(seed, n);
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        assert!(name.starts_with("seed--777-planet-256-callisto-") && name.ends_with("-v301.map"), "{name}");
        let mars = &BODIES[2].bake.as_ref().unwrap();
        assert!(![super::super::file(seed, n), super::super::luna::file(seed, n), mars.file(seed, n)].contains(&path));
        let _ = std::fs::remove_file(&path);
        let got = bake.get(seed, n);
        assert_eq!((got.hash, got.version), (a.hash, VERSION), "baked by its row and kept");
        assert!(path.exists());
        assert_eq!(bake.for_hash(seed, n, a.hash).map(|m| m.hash), Some(a.hash));
        assert_eq!(crate::bake::find(seed, n, Some(a.hash)).map(|m| m.hash), Some(a.hash), "found by its hash");
        drop(got);
        // one from a server is kept under its row's name too, not Mars's
        let _ = std::fs::remove_file(&path);
        let kept = crate::bake::keep(Map::from_bytes(&a.to_bytes()).unwrap());
        assert!(kept.hash == a.hash && path.exists(), "kept as {}", path.display());
        assert_eq!(crate::bake::mars::version(p), VERSION);
        let _ = std::fs::remove_file(&path);
    }

    /// The map has what the row calls for (step 6c), on two seeds: dark old plains the most of it, bright fresh ice
    /// in young craters and their rays, two multi-ring basins with flowed-flat middles and rings of troughs, frost
    /// only near the poles, a crater count of Luna's power law; nothing of Luna's maria, metal or tubes.
    #[test]
    fn the_map_has_what_the_row_calls_for() {
        for seed in [1, 2] {
            let st = shared(seed, 768);
            let (m, s) = (&st.0, &st.1);
            let why = |w: &str| format!("seed {seed}: {w}");
            let all: f64 = s.area.iter().sum();
            for (k, a) in s.area.iter().enumerate() {
                assert!(*a > 0.0, "{}", why(&format!("no {}", KINDS[k])));
            }
            let share = |k: u8| s.area[(k - PLAIN) as usize] / all;
            assert!(share(PLAIN) > 0.6 && share(FRESH) < 0.2 && share(FROST) < 0.02, "{}", why(&format!("shares {:?}", s.area.map(|a| a / all))));
            assert!(m.tubes.is_empty() && m.biome.iter().all(|k| (PLAIN..=FROST).contains(k)));
            // the young craters are fresh ice at their middles
            let g = super::super::grid::Grid::new(m.m as usize, m.radius());
            let young: Vec<_> = s.craters.iter().filter(|c| c.2 > 0.97 && c.1 >= 12.0).collect();
            let bright = young.iter().filter(|c| m.biome[g.locate(c.0) as usize] == FRESH).count();
            assert!(young.len() > 5 && bright * 10 >= young.len() * 8, "{}", why(&format!("{bright} of {} young craters bright", young.len())));
            // two ringed basins, each with rings, its middle flat against the plains round it
            assert_eq!(s.rings.len(), 2, "{}", why("two multi-ring basins"));
            for &(c, rb, rings) in &s.rings {
                assert!(rings >= 3, "{}", why(&format!("{rings} rings")));
                let near = |lo: f64, hi: f64| -> Vec<f32> { (0..g.n() as u32).filter(|&k| (lo..hi).contains(&g.d(k).dot(c).clamp(-1.0, 1.0).acos())).map(|k| m.h[k as usize]).collect() };
                let spread = |v: &[f32]| {
                    let mean = v.iter().map(|&h| h as f64).sum::<f64>() / v.len() as f64;
                    (v.iter().map(|&h| (h as f64 - mean).powi(2)).sum::<f64>() / v.len() as f64).sqrt()
                };
                let (mid, ring) = (spread(&near(0.0, 0.5 * rb)), spread(&near(1.2 * rb, (1.2 + 0.3 * rings as f64) * rb)));
                assert!(mid < 0.7 * ring, "{}", why(&format!("the middle's relief {mid:.2} m against the rings' {ring:.2} m")));
                let pale = (0..g.n() as u32).filter(|&k| g.d(k).dot(c) > (0.3 * rb).cos() && m.biome[k as usize] == PALIMPSEST).count();
                assert!(pale > 0, "{}", why("a palimpsest in the middle"));
            }
            // frost only where the sun is low: within POLAR of a pole
            assert!((0..g.n() as u32).filter(|&k| m.biome[k as usize] == FROST).all(|k| g.d(k).z.abs() > 0.9));
        }
    }

    /// Its map's bytes, locked: seed 1 at the 4 km planet's 1536 and at 384 (the tests' worlds).
    #[test]
    fn the_map_is_as_it_was_baked() {
        for (n, want) in [(384, 0x32ac890dc737b146u64), (1536, 0x7984d03437d95554)] {
            let m = &shared(1, n).0;
            assert_eq!(m.hash, want, "n {n}: {:016x}", m.hash);
        }
    }

    /// A world of generation 7 with the icy moon on its map at n 384 (Terra on the noise land, the rest small).
    fn world() -> crate::world::World {
        let rows = crate::bodies::of(7);
        let list = vec![(1536, None), (32, None), (832, None), (1536 * 11, None), (384, Some(Arc::new(Map::from_bytes(&shared(1, 384).0.to_bytes()).unwrap())))];
        crate::world::World::of(rows, list, 1, 0.0, 7)
    }

    /// Every one of its kinds has its own ground from `data/biomes.toml`: its top, the band under it and its ice
    /// below, generated so in its columns; no Terra, Luna or Mars kind, no water, plant, cave or ore on it.
    #[test]
    fn each_kind_generates_its_ground_and_nothing_of_terra_luna_or_mars() {
        use crate::biomes::ROWS;
        use crate::world::Block;
        let w = world();
        let b = &w.bodies[4];
        assert_eq!((b.name.as_str(), b.map().map(|m| m.version), b.sea, b.caves.len(), b.ores.len(), b.parent), ("Callisto", Some(VERSION), None, 0, 0, 3));
        let (mut seen, mut tops) = ([0usize; 4], [false; 4]);
        let n = 384;
        for c in (0..6 * n * n).step_by(7) {
            let (f, i, j) = ((c / (n * n)) as u8, (c % n) as i32, ((c / n) % n) as i32);
            let (ci, cj) = (i as f64 + 0.5, j as f64 + 0.5);
            let kind = b.biome_at(f as usize, ci, cj, b.dir(f as usize, ci, cj)).unwrap();
            assert!((PLAIN..=FROST).contains(&kind), "column {c}: kind {kind}");
            seen[(kind - PLAIN) as usize] += 1;
            assert!(b.water_at(f, i, j).is_none() && b.col_plants(f, i, j).is_empty(), "column {c}: water or a plant");
            if c % 49 != 0 {
                continue;
            }
            let ground = ROWS.biomes[kind as usize].ground;
            let top = w.column_height(Block { body: 4, face: f, pos: glam::IVec3::new(i, j, 0) });
            let z0 = b.layer_of(top).floor() as i32;
            for z in b.layer_of(top - 12.0).floor() as i32..=z0 + 1 {
                let want = crate::world::material(top - b.layer_r(z as f64 + 0.5), &ground);
                assert_eq!(w.generated_block(Block { body: 4, face: f, pos: glam::IVec3::new(i, j, z) }), want, "column {c} ({}), layer {z} of {z0}", KINDS[(kind - PLAIN) as usize]);
                tops[(kind - PLAIN) as usize] |= want == ground[0];
            }
        }
        assert!(seen.iter().all(|&k| k > 0), "every kind seen: {seen:?}");
        assert!(tops.iter().all(|&t| t), "every kind's top block generated somewhere: {tops:?}");
    }

    /// Its generated blocks at generation 7, locked (as Mars's and Luna's are).
    #[test]
    fn callisto_generates_as_locked() {
        assert_eq!(format!("{:016x}", blocks_hash(&world())), "4b42aadc955da137");
    }

    /// Loaded on approach (0175's rule: past the home's moon, with a bake): a stub until built, then bit for bit as
    /// one built at the start; out and in again the same. From Terra it is neither streamed nor asked for.
    #[test]
    fn callisto_loaded_late_is_callisto_from_the_start_and_terra_holds_none_of_it() {
        use crate::stream::{FrameStats, Opts, Streamer, View};
        use crate::world::Source;
        let rows = crate::bodies::of(7);
        assert!(crate::world::lazy_row(rows, 4) && !crate::world::lazy_row(rows, 3), "the icy moon is lazy, the giant has no ground");
        let map = Arc::new(Map::from_bytes(&shared(1, 384).0.to_bytes()).unwrap());
        let list = vec![(1536, Source::Now(None)), (32, Source::Now(None)), (832, Source::Now(None)), (1536 * 11, Source::Now(None)), (384, Source::Later(map.hash))];
        let mut w = crate::world::World::of_sources(rows, list, 1, 0.0, 7);
        let eager = world();
        assert!(!w.loaded(4) && w.bodies[4].map().is_none() && w.map_hash(4) == eager.map_hash(4), "a stub that says its map");
        // from Terra: nothing of it streamed, asked for or held
        let opts = Opts { k: 4.0, greedy: false, cutout: false, horizon: true, prefetch_s: 0.0, max_inflight: 64 };
        let mut s = Streamer::new(&w, 1 << 22, opts);
        let planes = [glam::Vec4::new(0.0, 0.0, 0.0, 1.0); 6];
        let terra = w.bodies[0].spawn_point() + w.bodies[0].spawn * 2.0;
        let view = View::new(terra, planes, &w);
        for _ in 0..30 {
            let (mut out, mut reqs, mut st) = (Vec::new(), Vec::new(), FrameStats::default());
            s.apply(usize::MAX, &mut st, |_, _| {});
            s.frame += 1;
            assert!(!s.seen(&w, &view, 0.05).contains(&4), "asked for from Terra");
            s.select(&w, &view, true, 1.0, &mut out, &mut reqs, &mut st);
            s.dispatch(&w, &mut reqs);
        }
        assert!(s.slots_of(0) > 0 && s.slots_of(4) == 0 && !w.loaded(4), "Terra streams, the icy moon is not held");
        for _ in 0..2 {
            let b = w.loader(4).expect("a stub has a loader")().expect("this build makes this map");
            w.put(4, b);
            assert!(w.loaded(4));
            assert_eq!(blocks_hash(&w), blocks_hash(&eager), "generated as at the start");
            let (a, e) = (&w.bodies[4], &eager.bodies[4]);
            assert_eq!((a.spawn, a.sea, a.layers, a.parent), (e.spawn, e.sea, e.layers, e.parent));
            w.unload(4);
            assert!(!w.loaded(4) && w.bodies[4].map().is_none());
        }
    }

    /// Places and times on seed 1's full-size icy moon, for pictures (`run=stand body=4 at= time=`): `cargo test -p
    /// engine --lib callisto_places -- --ignored --nocapture`. Its body -x faces the giant.
    #[test]
    #[ignore]
    fn callisto_places() {
        let st = shared(1, 1536);
        let (m, s) = (&st.0, &st.1);
        let g = super::super::grid::Grid::new(m.m as usize, m.radius());
        let p = |w: &str, d: DVec3| println!("PLACE {w} at={:.5},{:.5},{:.5} kind {}", d.x, d.y, d.z, m.biome[g.locate(d) as usize]);
        for &(c, rb, k) in &s.rings {
            p(&format!("ring basin rb {rb:.3} rings {k}"), c);
        }
        for c in s.craters.iter().filter(|c| c.2 > 0.995 && c.1 >= 30.0).take(6) {
            p(&format!("young crater d {:.0} t {:.4}", c.1, c.2), c.0);
        }
        for c in s.craters.iter().filter(|c| c.2 < 0.5 && c.1 >= 60.0 && c.0.x < -0.5).take(3) {
            p(&format!("old crater d {:.0} near side", c.1), c.0);
        }
        let w = crate::world::World::with_grid(6144, crate::bodies::moon_columns(6144), 1, None, None, 0.0, 7);
        let per = w.bodies[4].period;
        for k in 0..24 {
            let t = per * k as f64 / 24.0;
            let pc = w.pose(4, t);
            let sun = (pc.rot.inverse() * (w.sky.sun(t) - pc.centre)).normalize();
            println!("TIME {t:.0}: sun in its frame {:.3},{:.3},{:.3}", sun.x, sun.y, sun.z);
        }
    }

    fn blocks_hash(w: &crate::world::World) -> u64 {
        use crate::world::Block;
        let b = &w.bodies[4];
        let n = 384;
        let mut h = crate::world::FNV0;
        for c in (0..6 * n * n).step_by(97) {
            let (f, i, j) = ((c / (n * n)) as u8, (c % n) as i32, ((c / n) % n) as i32);
            let top = w.column_height(Block { body: 4, face: f, pos: glam::IVec3::new(i, j, 0) });
            for z in b.layer_of(top - 30.0).floor() as i32..=b.layer_of(top + 1.0).ceil() as i32 {
                h = crate::world::fnv(h, &w.generated_block(Block { body: 4, face: f, pos: glam::IVec3::new(i, j, z) }).to_le_bytes());
            }
        }
        h
    }
}
