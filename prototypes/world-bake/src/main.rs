//! World bake spike (worldgen W1): `world-bake mode=bake seed=1 planet=17376`. See README.md.
mod bake;
mod grid;
mod life;
mod render;

use bake::{Bake, Params};
use glam::DVec3;
use grid::NONE;
use render::{draw, Close, Cross, Globe, Img, Layer, View};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

/// TASTE CALL, decided 2026-11-11: Terra's default seed, picked from the sheet of 12 (worldgen.md, call 3).
pub const DEFAULT_SEED: u64 = 5;

fn main() {
    let args: HashMap<String, String> =
        std::env::args().skip(1).filter_map(|a| a.split_once('=').map(|(k, v)| (k.to_string(), v.to_string()))).collect();
    let num = |k: &str, d: f64| args.get(k).map_or(d, |v| v.parse().unwrap_or_else(|_| panic!("{k}={v} is not a number")));
    let mut p = Params::default();
    p.mountain_m = num("mountain", p.mountain_m as f64) as f32;
    p.sea_depth_m = num("sea", p.sea_depth_m as f64) as f32;
    p.cell_m = num("cell", p.cell_m);
    p.coarse_iters = num("coarse_iters", p.coarse_iters as f64) as usize;
    p.fine_iters = num("fine_iters", p.fine_iters as f64) as usize;
    p.coarse_dt = num("coarse_dt", p.coarse_dt as f64) as f32;
    p.fine_dt = num("fine_dt", p.fine_dt as f64) as f32;
    p.min_lake_m = num("min_lake", p.min_lake_m as f64) as f32;
    p.shield_uplift = num("shield", p.shield_uplift);
    p.old_range_uplift = num("old", p.old_range_uplift);
    let planet = num("planet", 17376.0) as u32;
    let out = PathBuf::from(args.get("out").map_or("out", String::as_str));
    std::fs::create_dir_all(&out).unwrap();
    match args.get("mode").map_or("bake", String::as_str) {
        "bake" => {
            let seed = num("seed", DEFAULT_SEED as f64) as u64;
            let mut b = bake::bake(planet, seed, &p);
            let climate = args.get("climate").is_none_or(|v| v != "0");
            if climate {
                b.life = Some(life::life(&b, seed, &life::LifeParams::default()));
            }
            let dir = out.join(format!("seed-{seed}-planet-{planet}"));
            std::fs::create_dir_all(&dir).unwrap();
            let disk = save(&b, &dir.join("bake.bin.gz"));
            report(&b, seed, planet, &p, disk);
            if args.get("draw").is_none_or(|v| v != "0") {
                sheet(&b).save(&dir.join("sheet.png"));
                globe(&b, DVec3::new(0.35, 1.0, 0.55), 1000).save(&dir.join("orbit.png"));
                if climate {
                    climate_sheet(&b, seed).save(&dir.join("climate.png"));
                    let at = DVec3::new(0.35, 1.0, 0.55);
                    draw(&b, &Globe { at, up: DVec3::Y }, Layer::Biome, 1000, 1000, sun_for(at), 4.0).save(&dir.join("orbit-biome.png"));
                }
            }
        }
        "seeds" => {
            let seeds: Vec<u64> = args.get("seeds").map_or("1,2,3,4,5,6,7,8,9,10,11,12", String::as_str).split(',').map(|s| s.parse().unwrap()).collect();
            let (tw, th) = (400, 700);
            let cols = 4;
            let mut img = Img::new(tw * cols, th * seeds.len().div_ceil(cols), [0, 0, 0]);
            for (k, &seed) in seeds.iter().enumerate() {
                let mut b = bake::bake(planet, seed, &p);
                let layer = if args.get("climate").is_some_and(|v| v == "1") {
                    b.life = Some(life::life(&b, seed, &life::LifeParams::default()));
                    Layer::Biome
                } else {
                    Layer::Height
                };
                report(&b, seed, planet, &p, 0);
                let (x, y) = ((k % cols) * tw, (k / cols) * th);
                let at = DVec3::new(0.35, 1.0, 0.55);
                img.blit(&draw(&b, &Globe { at, up: DVec3::Y }, layer, 400, 400, sun_for(at), 4.0), x, y);
                img.blit(&draw(&b, &Cross, layer, 400, 300, sun_for(DVec3::Y), 3.0), x, y + 400);
                img.text(x + 8, y + 8, &format!("SEED {seed}"), 3, [255, 255, 255]);
            }
            img.save(&out.join(format!("seeds-planet-{planet}.png")));
        }
        "relief" => {
            let seed = num("seed", DEFAULT_SEED as f64) as u64;
            let vars: Vec<(f32, f32)> = args
                .get("variants")
                .map_or("50:25,120:60,200:100", String::as_str)
                .split(',')
                .map(|v| v.split_once(':').map(|(a, b)| (a.parse().unwrap(), b.parse().unwrap())).unwrap())
                .collect();
            let mut b = bake::bake(planet, seed, &p);
            // the highest peak: the close-up and the section go through it
            let top = (0..b.h.len()).max_by(|a, c| b.h[*a].total_cmp(&b.h[*c])).unwrap() as u32;
            let at = b.g.d(top);
            let toward = at.any_orthonormal_vector();
            let rows: Vec<Img> = vars
                .iter()
                .map(|&(m, s)| {
                    rescale(&mut b, m, s);
                    // a section 2.2 km long through the peak, 2 m a pixel, heights 2x
                    let section = render::profile(&b, at, toward, 2200.0, 2.0, 2.0, -110.0, 210.0);
                    let mut row = Img::new(1600, 500 + section.h, [20, 20, 20]);
                    row.blit(&globe(&b, at + DVec3::new(0.2, 0.3, 0.1), 500), 0, 0);
                    let close = Close { at, up: toward, half: 700.0, r: b.g.r };
                    row.blit(&draw(&b, &close, Layer::Height, 500, 500, sun_for(at), 1.0), 520, 0);
                    row.blit(&section, 0, 500);
                    eprintln!("relief {m} m / {s} m: drawn");
                    row
                })
                .collect();
            let mut img = Img::new(1600, rows.iter().map(|r| r.h).sum(), [0, 0, 0]);
            let mut y = 0;
            for r in &rows {
                img.blit(r, 0, y);
                y += r.h;
            }
            img.save(&out.join(format!("relief-seed-{seed}.png")));
        }
        m => panic!("mode={m}: bake, seeds or relief"),
    }
}

/// The relief is the bake's last, linear step: a variant rescales land and sea (the shape stays).
fn rescale(b: &mut Bake, mountain: f32, sea: f32) {
    let top = b.h.iter().fold(0f32, |a, &v| a.max(v)).max(1e-3);
    let deep = b.h.iter().fold(0f32, |a, &v| a.max(-v)).max(1e-3);
    let (l, s) = (mountain / top, sea / deep);
    let f = |v: &mut f32| *v *= if *v >= 0.0 { l } else { s };
    b.h.iter_mut().for_each(f);
    b.w.iter_mut().for_each(f);
    b.lvl.iter_mut().for_each(f);
    b.scale = (b.scale.0 * l, b.scale.1 * s);
}

/// The sun over a view's upper left.
fn sun_for(at: DVec3) -> DVec3 {
    let f = at.normalize();
    let r = DVec3::Y.cross(f).normalize_or(DVec3::X);
    (f * 0.7 - r * 0.5 + f.cross(r) * 0.5).normalize()
}

fn globe(b: &Bake, at: DVec3, px: usize) -> Img {
    draw(b, &Globe { at, up: DVec3::Y }, Layer::Height, px, px, sun_for(at), 4.0)
}

/// The sheet: the cube unfolded three times (height with rivers, plates, rivers), the globe from two
/// sides, and close-ups 600 m across of two corners and two seams (height, then rivers).
fn sheet(b: &Bake) -> Img {
    let mut img = Img::new(2400, 1200, [0, 0, 0]);
    let sun = sun_for(DVec3::new(0.3, 1.0, 0.4));
    for (k, l) in [Layer::Height, Layer::Plates, Layer::Rivers].into_iter().enumerate() {
        img.blit(&draw(b, &Cross, l, 800, 600, sun, 3.0), k * 800, 0);
    }
    img.blit(&globe(b, DVec3::new(0.35, 1.0, 0.55), 600), 0, 600);
    img.blit(&globe(b, DVec3::new(-0.35, -1.0, -0.55), 600), 600, 600);
    // the corners and seam points with the most coast in view: a close-up of open sea shows nothing
    let land_in = |at: DVec3| {
        let v = Close { at, up: at.any_orthonormal_vector(), half: 300.0, r: b.g.r };
        let k = (0..256).filter(|i| v.dir((i % 16) as f64 / 7.5 - 1.0, (i / 16) as f64 / 7.5 - 1.0).is_some_and(|d| b.land(b.g.locate(d) as usize))).count();
        (k as f64 / 256.0 - 0.6).abs()
    };
    let best = |mut v: Vec<DVec3>, k: usize| {
        v.sort_by(|a, c| land_in(*a).total_cmp(&land_in(*c)));
        v.truncate(k);
        v
    };
    let along: Vec<DVec3> = render::seams().iter().flat_map(|s| {
        // 31 points along each seam, between its two corners
        let a = s.abs();
        let free = if a.x == 0.0 { DVec3::X } else if a.y == 0.0 { DVec3::Y } else { DVec3::Z };
        let base = *s * std::f64::consts::SQRT_2;
        (1..32).map(move |t| (base + free * (t as f64 / 16.0 - 1.0)).normalize())
    }).collect();
    let mut spots = best(render::corners(), 2);
    spots.extend(best(along, 2));
    for (k, at) in spots.into_iter().enumerate() {
        let close = Close { at, up: at.any_orthonormal_vector(), half: 300.0, r: b.g.r };
        let (x, y) = (1200 + (k % 2) * 300, 600 + (k / 2) * 300);
        img.blit(&draw(b, &close, Layer::Height, 300, 300, sun_for(at), 2.0), x, y);
        img.blit(&draw(b, &close, Layer::Rivers, 300, 300, sun_for(at), 2.0), x + 600, y);
    }
    img
}

/// The climate and biomes of one bake: the cube unfolded (mean temperature, rain with the rivers, biome),
/// the globe from its six sides (biome), the legend with each biome's share, and the soil.
fn climate_sheet(b: &Bake, seed: u64) -> Img {
    let l = b.life.as_ref().unwrap();
    let mut img = Img::new(2400, 1650, [12, 12, 16]);
    let sun = sun_for(DVec3::new(0.3, 1.0, 0.4));
    for (k, (layer, name)) in [(Layer::Temp, "MEAN TEMPERATURE"), (Layer::Rain, "RAIN (LOG), RIVERS"), (Layer::Biome, "BIOME")].into_iter().enumerate() {
        img.blit(&draw(b, &Cross, layer, 800, 600, sun, 3.0), k * 800, 0);
        img.text(k * 800 + 10, 10, name, 3, [255, 255, 255]);
    }
    let sides = [(DVec3::X, "+X"), (-DVec3::X, "-X"), (DVec3::Y, "+Y"), (-DVec3::Y, "-Y"), (DVec3::Z, "+Z NORTH"), (-DVec3::Z, "-Z SOUTH")];
    for (k, (at, name)) in sides.into_iter().enumerate() {
        let up = if at.z.abs() > 0.5 { DVec3::Y } else { DVec3::Z };
        img.blit(&draw(b, &Globe { at, up }, Layer::Biome, 400, 400, sun_for(at), 4.0), k * 400, 600);
        img.text(k * 400 + 8, 608, name, 3, [255, 255, 255]);
    }
    let sh = life::shares(b, l);
    img.text(20, 1020, &format!("SEED {seed}   SHARE OF LAND (LAKES OUT) OR OF SEA"), 4, [255, 255, 255]);
    for (k, (name, c)) in life::BIOMES.iter().enumerate() {
        let (x, y) = (20 + (k / 8) * 780, 1070 + (k % 8) * 64);
        img.rect(x, y, 48, 48, *c);
        let tag = if k < life::LAND_BIOMES as usize { "LAND" } else { "SEA" };
        img.text(x + 64, y + 12, &format!("{name} {:.1}% OF {tag}", sh[k] * 100.0), 4, if sh[k] > 0.0 { [230, 230, 230] } else { [255, 80, 80] });
    }
    img.blit(&draw(b, &Cross, Layer::Soil, 800, 600, sun, 3.0), 1600, 1050);
    img.text(1610, 1060, "SOIL: SAND LOAM CLAY GRAVEL PEAT ROCK", 3, [255, 255, 255]);
    img
}

/// The result as it would be kept: heights in decimetres, plates, the rivers. Returns bytes on disk.
fn save(b: &Bake, path: &Path) -> u64 {
    let f = std::fs::File::create(path).unwrap();
    let mut z = flate2::write::GzEncoder::new(std::io::BufWriter::new(f), flate2::Compression::new(6));
    z.write_all(b"BAKE1").unwrap();
    z.write_all(&(b.g.m as u32).to_le_bytes()).unwrap();
    for v in &b.h {
        z.write_all(&((v * 10.0).round().clamp(-32768.0, 32767.0) as i16).to_le_bytes()).unwrap();
    }
    z.write_all(&b.plate).unwrap();
    z.write_all(&(b.rivers.len() as u32).to_le_bytes()).unwrap();
    for r in &b.rivers {
        z.write_all(&r.cell.to_le_bytes()).unwrap();
        z.write_all(&r.to.to_le_bytes()).unwrap();
        z.write_all(&r.area.to_le_bytes()).unwrap();
    }
    z.finish().unwrap().flush().unwrap();
    std::fs::metadata(path).unwrap().len()
}

/// (peak working set, peak private commit) in MB
fn peak_mb() -> (f64, f64) {
    #[repr(C)]
    #[derive(Default)]
    struct Counters {
        cb: u32,
        faults: u32,
        peak_ws: usize,
        ws: usize,
        a: usize,
        b: usize,
        c: usize,
        d: usize,
        commit: usize,
        peak_commit: usize,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentProcess() -> isize;
        fn K32GetProcessMemoryInfo(p: isize, c: *mut Counters, cb: u32) -> i32;
    }
    let mut c = Counters { cb: std::mem::size_of::<Counters>() as u32, ..Default::default() };
    unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };
    (c.peak_ws as f64 / 1048576.0, c.peak_commit as f64 / 1048576.0)
}

fn report(b: &Bake, seed: u64, planet: u32, p: &Params, disk: u64) {
    let land: f64 = (0..b.h.len()).filter(|&c| b.land(c)).map(|c| b.g.area(c as u32) as f64).sum();
    let all: f64 = (0..b.h.len()).map(|c| b.g.area(c as u32) as f64).sum();
    let total: f64 = b.times.iter().map(|t| t.1).sum();
    let (ws, commit) = peak_mb();
    let mut times: Vec<String> = b.times.iter().map(|(k, v)| format!("\"{k}\":{v:.2}")).collect();
    let mut total = total;
    let mut extra = String::new();
    if let Some(l) = &b.life {
        times.extend(l.times.iter().map(|(k, v)| format!("\"{k}\":{v:.2}")));
        let lt: f64 = l.times.iter().map(|t| t.1).sum();
        total += lt;
        let sh = life::shares(b, l);
        let s: Vec<String> = life::BIOMES.iter().zip(sh).map(|((n, _), v)| format!("\"{n}\":{:.4}", v)).collect();
        extra = format!(
            ",\"climate_s\":{lt:.2},\"air_columns\":{},\"air_days\":{},\"air_last_change\":[{:.4},{:.4}],\"shares\":{{{}}},\"all_15\":{}",
            l.air_columns,
            l.air_days,
            l.air_change.0,
            l.air_change.1,
            s.join(","),
            sh.iter().all(|&v| v > 0.0)
        );
    }
    let lake_cells: u32 = b.lakes.iter().map(|l| l.cells).sum();
    let mut lh: Vec<f32> = (0..b.h.len()).filter(|&c| b.land(c)).map(|c| b.h[c].max(0.0)).collect();
    lh.sort_by(f32::total_cmp);
    let q = |f: f64| lh.get(((lh.len().max(1) - 1) as f64 * f) as usize).copied().unwrap_or(0.0);
    println!(
        "{{\"seed\":{seed},\"planet\":{planet},\"radius_m\":{:.0},\"cells\":{},\"cell_m\":{},\"seconds\":{{{}}},\"total_s\":{total:.2},\"peak_ws_mb\":{ws:.0},\"peak_commit_mb\":{commit:.0},\"disk_bytes\":{disk},\"hash\":\"{:016x}\",\"land_share\":{:.3},\"top_m\":{:.1},\"deep_m\":{:.1},\"plates\":{},\"rivers\":{},\"lakes\":{},\"lake_cells\":{lake_cells},\"land_p50_p90_p99_m\":[{:.1},{:.1},{:.1}],\"last_change_m\":[{:.4},{:.4}],\"drains\":{}{extra}}}",
        b.g.r,
        b.h.len(),
        p.cell_m,
        times.join(","),
        b.hash(),
        land / all,
        b.h.iter().fold(0f32, |a, &v| a.max(v)),
        b.h.iter().fold(0f32, |a, &v| a.min(v)),
        b.plate.iter().max().map_or(0, |m| m + 1),
        b.rivers.len(),
        b.lakes.len(),
        q(0.5),
        q(0.9),
        q(0.99),
        b.converged.0 * b.scale.0,
        b.converged.1 * b.scale.0,
        check_rivers(b).is_ok(),
    );
}

/// Every river runs down to the sea or a lake and never climbs: from each river cell along the
/// receivers, the water surface falls at every step and the bed downstream never stands over it.
pub fn check_rivers(b: &Bake) -> Result<usize, String> {
    let n = b.h.len();
    let mut ok = vec![false; n];
    let mut walked = 0;
    for r in &b.rivers {
        let mut c = r.cell as usize;
        let mut steps = 0;
        let mut path = Vec::new();
        loop {
            if ok[c] || !b.land(c) || (b.lake(c) && c != r.cell as usize) {
                break;
            }
            let next = b.recv[c];
            if next == NONE {
                return Err(format!("land cell {c} has no receiver"));
            }
            let nx = next as usize;
            if b.land(nx) && b.w[nx] >= b.w[c] {
                return Err(format!("cell {c} to {nx}: the water climbs from {} to {} m", b.w[c], b.w[nx]));
            }
            if b.h[nx] > b.w[c] {
                return Err(format!("cell {c} to {nx}: the bed {} m stands over the water {} m", b.h[nx], b.w[c]));
            }
            path.push(c);
            steps += 1;
            if steps > n {
                return Err(format!("the river from {} never ends", r.cell));
            }
            c = nx;
        }
        for c in path {
            ok[c] = true;
            walked += 1;
        }
    }
    Ok(walked)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small(seed: u64) -> Bake {
        // the small planet of ADR 0021 (531 m) at 8 m cells: 96 a face, 24 coarse
        bake::bake(768, seed, &Params::default())
    }

    #[test]
    fn the_same_seed_bakes_the_same_world() {
        let (a, b, c) = (small(7), small(7), small(8));
        assert_eq!(a.hash(), b.hash());
        assert_ne!(a.hash(), c.hash());
    }

    #[test]
    fn every_river_reaches_the_sea_or_a_lake_downhill() {
        // the 1 km planet (Luna's size from W6; 1536 columns of 1 m), with smaller rivers counted, so there are plenty
        let p = Params { river_area_m2: 2e4, ..Params::default() };
        for seed in [1, 2, 3] {
            let b = bake::bake(1536, seed, &p);
            assert!(b.rivers.len() > 20, "seed {seed}: {} rivers", b.rivers.len());
            let walked = check_rivers(&b).unwrap_or_else(|e| panic!("seed {seed}: {e}"));
            assert!(walked >= b.rivers.len());
            // a lake is at least min_lake_m deep somewhere; the shallower ones were filled
            assert!(b.lakes.iter().all(|l| l.depth >= p.min_lake_m), "seed {seed}: a lake under {} m", p.min_lake_m);
        }
    }

    #[test]
    fn the_sea_covers_its_share_and_the_relief_its_taste_call() {
        let b = small(4);
        let land: f64 = (0..b.h.len()).filter(|&c| b.land(c)).map(|c| b.g.area(c as u32) as f64).sum();
        let all: f64 = (0..b.h.len()).map(|c| b.g.area(c as u32) as f64).sum();
        assert!((land / all - 0.45).abs() < 0.03, "land {}", land / all);
        let p = Params::default();
        assert!((b.h.iter().fold(0f32, |a, &v| a.max(v)) - p.mountain_m).abs() < 0.01);
        assert!((b.h.iter().fold(0f32, |a, &v| a.min(v)) + p.sea_depth_m).abs() < 0.01);
    }

    fn with_life(seed: u64) -> Bake {
        let mut b = bake::bake(1536, seed, &Params::default());
        b.life = Some(life::life(&b, seed, &life::LifeParams::default()));
        b
    }

    /// Stages 5 to 8: the same seed gives the same climate, soil and biomes, and they enter the hash.
    #[test]
    fn the_climate_and_biomes_bake_the_same_twice() {
        let (a, b) = (with_life(3), with_life(3));
        assert_eq!(a.hash(), b.hash());
        let mut c = with_life(3);
        c.life.as_mut().unwrap().biome[1234] ^= 1;
        assert_ne!(a.hash(), c.hash(), "the biomes are in the hash");
    }

    /// Water only gathers downstream: every land cell passes at least its own runoff on to its receiver.
    #[test]
    fn rivers_grow_downstream() {
        let b = with_life(2);
        let l = b.life.as_ref().unwrap();
        for c in 0..b.h.len() {
            let r = b.recv[c];
            if b.land(c) && r != NONE && b.land(r as usize) {
                assert!(l.flow[r as usize] >= l.flow[c], "cell {c}: {} m3/s, its receiver {} has {}", l.flow[c], r, l.flow[r as usize]);
            }
        }
        let most = l.flow.iter().fold(0f32, |a, &v| a.max(v));
        assert!(most > 100.0 * l.flow.iter().filter(|&&v| v > 0.0).fold(f32::MAX, |a, &v| a.min(v)), "a river gathers: {most}");
    }

    /// The Whittaker order: by mean temperature ice < tundra < cold forest < temperate forest < rain
    /// forest, and by mean rain desert < dry scrub < savanna < rain forest (each over several seeds).
    #[test]
    fn biomes_follow_temperature_and_rain() {
        let (mut t, mut r, mut k) = ([0f64; 15], [0f64; 15], [0f64; 15]);
        for seed in [1, 2, 5] {
            let b = with_life(seed);
            let l = b.life.as_ref().unwrap();
            for c in 0..b.h.len() {
                let x = l.biome[c] as usize;
                t[x] += l.temp[c] as f64;
                r[x] += l.rain[c] as f64;
                k[x] += 1.0;
            }
        }
        let m = |v: &[f64; 15], x: u8| v[x as usize] / k[x as usize].max(1.0);
        use life::*;
        // only the biomes that occur on these small planets (ice may not)
        let here = |v: &[u8]| -> Vec<u8> { v.iter().copied().filter(|&x| k[x as usize] > 100.0).collect() };
        let by_t = here(&[ICE, TUNDRA, COLD_FOREST, TEMPERATE_FOREST, RAIN_FOREST]);
        let by_r = here(&[DESERT, DRY_SCRUB, SAVANNA, RAIN_FOREST]);
        assert!(by_t.len() >= 4 && by_r.len() == 4, "{by_t:?} {by_r:?}");
        for w in by_t.windows(2) {
            assert!(m(&t, w[0]) < m(&t, w[1]), "{} {:.1} C, {} {:.1} C", BIOMES[w[0] as usize].0, m(&t, w[0]), BIOMES[w[1] as usize].0, m(&t, w[1]));
        }
        for w in by_r.windows(2) {
            assert!(m(&r, w[0]) < m(&r, w[1]), "{} {:.2} mm, {} {:.2} mm", BIOMES[w[0] as usize].0, m(&r, w[0]), BIOMES[w[1] as usize].0, m(&r, w[1]));
        }
    }
}
