//! World bake spike (worldgen W1): `world-bake mode=bake seed=1 planet=12288`. See README.md.
mod bake;
mod grid;
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
    p.shield_uplift = num("shield", p.shield_uplift);
    p.old_range_uplift = num("old", p.old_range_uplift);
    let planet = num("planet", 12288.0) as u32;
    let out = PathBuf::from(args.get("out").map_or("out", String::as_str));
    std::fs::create_dir_all(&out).unwrap();
    match args.get("mode").map_or("bake", String::as_str) {
        "bake" => {
            let seed = num("seed", DEFAULT_SEED as f64) as u64;
            let b = bake::bake(planet, seed, &p);
            let dir = out.join(format!("seed-{seed}-planet-{planet}"));
            std::fs::create_dir_all(&dir).unwrap();
            let disk = save(&b, &dir.join("bake.bin.gz"));
            report(&b, seed, planet, &p, disk);
            if args.get("draw").is_none_or(|v| v != "0") {
                sheet(&b).save(&dir.join("sheet.png"));
                globe(&b, DVec3::new(0.35, 1.0, 0.55), 1000).save(&dir.join("orbit.png"));
            }
        }
        "seeds" => {
            let seeds: Vec<u64> = args.get("seeds").map_or("1,2,3,4,5,6,7,8,9,10,11,12", String::as_str).split(',').map(|s| s.parse().unwrap()).collect();
            let (tw, th) = (400, 700);
            let cols = 4;
            let mut img = Img::new(tw * cols, th * seeds.len().div_ceil(cols), [0, 0, 0]);
            for (k, &seed) in seeds.iter().enumerate() {
                let b = bake::bake(planet, seed, &p);
                report(&b, seed, planet, &p, 0);
                let (x, y) = ((k % cols) * tw, (k / cols) * th);
                img.blit(&globe(&b, DVec3::new(0.35, 1.0, 0.55), 400), x, y);
                img.blit(&draw(&b, &Cross, Layer::Height, 400, 300, sun_for(DVec3::Y), 3.0), x, y + 400);
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
    let times: Vec<String> = b.times.iter().map(|(k, v)| format!("\"{k}\":{v:.2}")).collect();
    let lake_cells: u32 = b.lakes.iter().map(|l| l.cells).sum();
    let mut lh: Vec<f32> = (0..b.h.len()).filter(|&c| b.land(c)).map(|c| b.h[c].max(0.0)).collect();
    lh.sort_by(f32::total_cmp);
    let q = |f: f64| lh.get(((lh.len().max(1) - 1) as f64 * f) as usize).copied().unwrap_or(0.0);
    println!(
        "{{\"seed\":{seed},\"planet\":{planet},\"radius_m\":{:.0},\"cells\":{},\"cell_m\":{},\"seconds\":{{{}}},\"total_s\":{total:.2},\"peak_ws_mb\":{ws:.0},\"peak_commit_mb\":{commit:.0},\"disk_bytes\":{disk},\"hash\":\"{:016x}\",\"land_share\":{:.3},\"top_m\":{:.1},\"deep_m\":{:.1},\"plates\":{},\"rivers\":{},\"lakes\":{},\"lake_cells\":{lake_cells},\"land_p50_p90_p99_m\":[{:.1},{:.1},{:.1}],\"last_change_m\":[{:.4},{:.4}],\"drains\":{}}}",
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
        bake::bake(1536, seed, &Params::default())
    }

    #[test]
    fn the_same_seed_bakes_the_same_world() {
        let (a, b, c) = (small(7), small(7), small(8));
        assert_eq!(a.hash(), b.hash());
        assert_ne!(a.hash(), c.hash());
    }

    #[test]
    fn every_river_reaches_the_sea_or_a_lake_downhill() {
        // the 1 km planet (Luna's size from W6), with smaller rivers counted, so there are plenty
        let p = Params { river_area_m2: 2e4, ..Params::default() };
        for seed in [1, 2, 3] {
            let b = bake::bake(3072, seed, &p);
            assert!(b.rivers.len() > 20, "seed {seed}: {} rivers", b.rivers.len());
            let walked = check_rivers(&b).unwrap_or_else(|e| panic!("seed {seed}: {e}"));
            assert!(walked >= b.rivers.len());
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
}
