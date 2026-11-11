//! Air grid spike (ADR 0010, air.md): `air mode=cost` times a step on three planet sizes;
//! `air mode=climate bake=<world-bake out dir>/bake.bin.gz planet=6144` runs the model on baked heights
//! to a steady day and draws it. See README.md.
use air::{cube, Air, Params, DAY, LAYERS};
use glam::DVec3;
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::time::Instant;

fn main() {
    let args: HashMap<String, String> =
        std::env::args().skip(1).filter_map(|a| a.split_once('=').map(|(k, v)| (k.to_string(), v.to_string()))).collect();
    let num = |k: &str, d: f64| args.get(k).map_or(d, |v| v.parse().unwrap());
    let out = PathBuf::from(args.get("out").map_or("out", String::as_str));
    std::fs::create_dir_all(&out).unwrap();
    match args.get("mode").map_or("climate", String::as_str) {
        "cost" => {
            for planet in [6144u32, 12288, 17376] {
                let m = (planet as f64 / 128.0).round() as usize;
                let r = radius(planet);
                let (g, w) = air::test_world(m, r);
                let mut row = Vec::new();
                for threads in [1usize, 0] {
                    let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
                    let ms = pool.install(|| {
                        let mut a = Air::new(m, r, g.clone(), w.clone(), Params::default());
                        for _ in 0..100 {
                            a.step(1.0);
                        }
                        let t = Instant::now();
                        let steps = 300;
                        for _ in 0..steps {
                            a.step(1.0);
                        }
                        t.elapsed().as_secs_f64() * 1000.0 / steps as f64
                    });
                    row.push(format!("\"{}\":{ms:.3}", if threads == 1 { "step_ms_1_thread" } else { "step_ms_all" }));
                    row.push(format!("\"{}\":{:.4}", if threads == 1 { "tick_ms_1_thread" } else { "tick_ms_all" }, ms / 20.0));
                }
                let a = Air::new(m, r, g.clone(), w.clone(), Params::default());
                let bytes = a.n * LAYERS * 4 * 4 + a.edges() * LAYERS * 4 + a.n * 4 * 6;
                println!("{{\"planet\":{planet},\"m\":{m},\"columns\":{},\"cells\":{},\"edges\":{},\"state_mb\":{:.2},{}}}", a.n, a.n * LAYERS, a.edges(), bytes as f64 / 1048576.0, row.join(","));
            }
        }
        "climate" => {
            let planet = num("planet", 6144.0) as u32;
            let bake = args.get("bake").expect("bake=<path to bake.bin.gz>");
            let days = num("days", 40.0) as usize;
            let dt = num("dt", 10.0) as f32;
            let (m, ground, water) = columns(bake, planet);
            let mut a = Air::new(m, radius(planet), ground, water, Params::default());
            climate(&mut a, days, dt, &out);
        }
        "compare" => {
            // two runs' last-day means: mean |dT|, and rain per column against each other
            let load = |k: &str| -> Vec<f32> {
                std::fs::read(args.get(k).unwrap()).unwrap().chunks(4).map(|b| f32::from_le_bytes(b.try_into().unwrap())).collect()
            };
            let (x, y) = (load("a"), load("b"));
            let n = x.len() / 2;
            let dt = (0..n).map(|c| (x[c] - y[c]).abs() as f64).sum::<f64>() / n as f64;
            let (rx, ry) = (&x[n..], &y[n..]);
            let (mx, my) = (rx.iter().sum::<f32>() as f64 / n as f64, ry.iter().sum::<f32>() as f64 / n as f64);
            let cov: f64 = (0..n).map(|c| (rx[c] as f64 - mx) * (ry[c] as f64 - my)).sum();
            let vx: f64 = (0..n).map(|c| (rx[c] as f64 - mx).powi(2)).sum();
            let vy: f64 = (0..n).map(|c| (ry[c] as f64 - my).powi(2)).sum();
            let l1 = (0..n).map(|c| (rx[c] - ry[c]).abs() as f64).sum::<f64>() / n as f64 / mx.max(1e-9);
            println!("{{\"mean_abs_dT\":{dt:.3},\"rain_mean_a\":{mx:.3},\"rain_mean_b\":{my:.3},\"rain_correlation\":{:.3},\"rain_l1_share\":{l1:.3}}}", cov / (vx * vy).sqrt());
        }
        m => panic!("mode={m}: cost, climate or compare"),
    }
}

/// The engine's radius for N columns of 1 m blocks (ADR 0003, 0033).
pub fn radius(planet: u32) -> f64 {
    planet as f64 / (2.0 * std::f64::consts::PI / 3.0).sqrt()
}

/// The bake's 8 m heights gathered into air columns of about 128 m: mean land height and water share.
fn columns(path: &str, planet: u32) -> (usize, Vec<f32>, Vec<f32>) {
    let mut z = flate2::read::GzDecoder::new(std::fs::File::open(path).unwrap());
    let mut buf = Vec::new();
    z.read_to_end(&mut buf).unwrap();
    assert_eq!(&buf[..5], b"BAKE1");
    let mf = u32::from_le_bytes(buf[5..9].try_into().unwrap()) as usize;
    let r = radius(planet);
    let gf = cube::Grid::new(mf, r);
    let m = (planet as f64 / 128.0).round() as usize;
    let ga = cube::Grid::new(m, r);
    let (mut sum, mut land, mut all) = (vec![0f64; ga.n()], vec![0u32; ga.n()], vec![0u32; ga.n()]);
    for c in 0..gf.n() {
        let h = i16::from_le_bytes([buf[9 + 2 * c], buf[10 + 2 * c]]) as f32 / 10.0;
        let a = ga.locate(gf.d(c as u32)) as usize;
        all[a] += 1;
        if h > 0.0 {
            land[a] += 1;
            sum[a] += h as f64;
        }
    }
    let ground = (0..ga.n()).map(|a| if land[a] > 0 { (sum[a] / land[a] as f64) as f32 } else { 0.0 }).collect();
    let water = (0..ga.n()).map(|a| 1.0 - land[a] as f32 / all[a].max(1) as f32).collect();
    (m, ground, water)
}

/// Runs to a steady day, then a day of measurements and the pictures.
fn climate(a: &mut Air, days: usize, dt: f32, out: &std::path::Path) {
    let per_day = (DAY / dt as f64).round() as usize;
    let n = a.n;
    let w0 = a.ledger();
    let t0 = Instant::now();
    let mut prev_t: Option<Vec<f64>> = None;
    let mut prev_r: Option<Vec<f64>> = None;
    let mut log = Vec::new();
    let (mut mean_t, mut rain, mut tmin, mut tmax) = (vec![0f64; n], vec![0f64; n], vec![0f64; n], vec![0f64; n]);
    let mut hour_rain_land = [0f64; 24];
    let mut breeze = [0f64; 24];
    let coast: Vec<(usize, usize)> = coast_pairs(a);
    let mut clouds = Vec::new();
    for day in 0..days {
        mean_t.iter_mut().for_each(|v| *v = 0.0);
        rain.iter_mut().for_each(|v| *v = 0.0);
        tmin.iter_mut().for_each(|v| *v = 1e9);
        tmax.iter_mut().for_each(|v| *v = -1e9);
        let last = day + 1 == days;
        for s in 0..per_day {
            a.step(dt);
            for c in 0..n {
                let t = a.surface_t(c) as f64;
                mean_t[c] += t / per_day as f64;
                tmin[c] = tmin[c].min(t);
                tmax[c] = tmax[c].max(t);
                rain[c] += a.fell[c] as f64;
            }
            if last {
                // by local solar time: noon where the sun is overhead
                let sun = Air::sun(a.t);
                let hour = |c: usize| {
                    let d = a.dir[c];
                    let east = DVec3::Z.cross(d).try_normalize().unwrap_or(DVec3::X);
                    let ang = (-sun.dot(east)).atan2(sun.dot(d));
                    (((ang / std::f64::consts::TAU * 24.0 + 12.0).rem_euclid(24.0)) as usize).min(23)
                };
                for c in (0..n).filter(|&c| a.water[c] < 0.5) {
                    hour_rain_land[hour(c)] += a.fell[c] as f64;
                }
                // the surface wind from sea to land across the coast
                for &(l, s) in &coast {
                    breeze[hour(l)] += toward(a, s, l) / coast.len() as f64 * 24.0 / per_day as f64;
                }
                if s % (per_day / 6) == 0 {
                    clouds.push((s, a.cloud.chunks(LAYERS).map(|c| c.iter().sum::<u32>() as f32).collect::<Vec<f32>>()));
                }
            }
        }
        assert_eq!(a.ledger(), w0, "the water ledger moved on day {day}");
        let dt_mean = prev_t.as_ref().map_or(f64::NAN, |p| (0..n).map(|c| (mean_t[c] - p[c]).abs()).sum::<f64>() / n as f64);
        let tot: f64 = rain.iter().sum();
        let dr = prev_r.as_ref().map_or(f64::NAN, |p| (0..n).map(|c| (rain[c] - p[c]).abs()).sum::<f64>() / tot.max(1.0));
        log.push(format!("[{day},{dt_mean:.4},{dr:.4},{:.2}]", mm_day(a, tot / n as f64, 0)));
        prev_t = Some(mean_t.clone());
        prev_r = Some(rain.clone());
    }
    let secs = t0.elapsed().as_secs_f64();
    if std::env::var("AIRDBG").is_ok() {
        a.debug_zonal().iter().for_each(|l| eprintln!("{l}"));
    }
    // rain in mm a day per column
    let mm: Vec<f64> = (0..n).map(|c| mm_day(a, rain[c], c)).collect();
    let land: Vec<usize> = (0..n).filter(|&c| a.water[c] < 0.5).collect();
    let zonal: Vec<String> = (0..9)
        .map(|b| {
            let lo = b as f32 * 10.0;
            let cs: Vec<usize> = (0..n).filter(|&c| (lo..lo + 10.0).contains(&a.lat[c].to_degrees().abs())).collect();
            let l: Vec<&usize> = cs.iter().filter(|&&c| a.water[c] < 0.5).collect();
            let avg = |v: &[f64], s: &[usize]| s.iter().map(|&c| v[c]).sum::<f64>() / s.len().max(1) as f64;
            let lc: Vec<usize> = l.iter().map(|&&c| c).collect();
            format!("[{lo},{:.1},{:.2},{:.2},{:.1}]", avg(&mean_t, &cs), avg(&mm, &cs), avg(&mm, &lc), avg(&(0..n).map(|c| tmax[c] - tmin[c]).collect::<Vec<_>>(), &lc))
        })
        .collect();
    // windward and lee: the surface wind's component up the ground's slope at each land column
    let (mut up, mut down, mut nu, mut nd) = (0f64, 0f64, 0, 0);
    for &c in &land {
        let s = upslope(a, c);
        if s > 0.02 {
            up += mm[c];
            nu += 1;
        } else if s < -0.02 {
            down += mm[c];
            nd += 1;
        }
    }
    let coast_d = coast_distance(a);
    let by_dist: Vec<String> = [0, 1, 2, 4, 8, 16]
        .windows(2)
        .map(|w| {
            let cs: Vec<usize> = land.iter().copied().filter(|&c| (w[0]..w[1]).contains(&coast_d[c])).collect();
            format!("[{},{:.2}]", w[0], cs.iter().map(|&c| mm[c]).sum::<f64>() / cs.len().max(1) as f64)
        })
        .collect();
    let hours: Vec<String> = hour_rain_land.iter().map(|v| format!("{:.0}", v / 1e6)).collect();
    let br: Vec<String> = breeze.iter().map(|v| format!("{v:.2}")).collect();
    let mut sorted = mm.clone();
    sorted.sort_by(|x, y| y.total_cmp(x));
    let top1: f64 = sorted[..n / 100].iter().sum::<f64>() / sorted.iter().sum::<f64>().max(1e-9);
    if std::env::var("AIRDBG").is_ok() {
        let g = cube::Grid::new(a.m, a.r);
        let mut idx: Vec<usize> = (0..n).collect();
        idx.sort_by(|&x, &y| mm[y].total_cmp(&mm[x]));
        for &c in &idx[..12] {
            let (f, i, j) = g.fij(c as u32);
            eprintln!("wet {c}: face {f} i {i} j {j} lat {:.1} lon {:.1} mm {:.1} water {:.2} ground {:.0}", a.lat[c].to_degrees(), a.dir[c].y.atan2(a.dir[c].x).to_degrees(), mm[c], a.water[c], a.ground[c]);
        }
    }
    eprintln!("rain in the wettest 1 % of columns: {:.1} %; wettest {:.1} mm/day", top1 * 100.0, sorted[0]);
    let (sp, cells) = puff(a);
    println!(
        "{{\"columns\":{n},\"days\":{days},\"dt\":{dt},\"seconds\":{secs:.2},\"ms_per_step\":{:.3},\"ledger_exact\":true,\"days_log_[day,dT_mean_K,rain_change_share,mean_mm_day]\":[{}],\"zonal_[lat,T,mm_all,mm_land,day_swing_land]\":[{}],\"windward_mm\":{:.2},\"lee_mm\":{:.2},\"land_mm_by_coast_distance_cells\":[{}],\"land_rain_by_hour_e6\":[{}],\"sea_breeze_by_hour_ms\":[{}],\"puff_spread_cells_after_60s\":{sp:.2},\"puff_cells_over_1pct\":{cells}}}",
        secs * 1000.0 / (days * per_day) as f64,
        log.join(","),
        zonal.join(","),
        up / nu.max(1) as f64,
        down / nd.max(1) as f64,
        by_dist.join(","),
        hours.join(","),
        br.join(",")
    );
    sheet(a, &mean_t, &mm, &tmin, &tmax, &clouds, per_day).save(&out.join("air-sheet.png"));
    let mut b = Vec::new();
    for v in mean_t.iter().chain(&mm) {
        b.extend_from_slice(&(*v as f32).to_le_bytes());
    }
    std::fs::write(out.join("means.bin"), b).unwrap();
}

fn mm_day(a: &Air, units_per_day: f64, c: usize) -> f64 {
    units_per_day / 4096.0 / a.area[c] as f64 * 1000.0
}

/// m/s of surface wind from column s toward column l.
fn toward(a: &Air, s: usize, l: usize) -> f64 {
    let (e, nn) = a.surface_wind(s);
    let d = a.dir[s];
    let east = DVec3::Z.cross(d).try_normalize().unwrap_or(DVec3::X);
    let north = d.cross(east);
    let v = east * e as f64 + north * nn as f64;
    v.dot((a.dir[l] - a.dir[s]).normalize())
}

/// Land columns next to a sea column, as (land, sea).
fn coast_pairs(a: &Air) -> Vec<(usize, usize)> {
    let g = cube::Grid::new(a.m, a.r);
    let mut v = Vec::new();
    for c in 0..a.n {
        if a.water[c] < 0.2 {
            if let Some(s) = g.around(c as u32).find(|&o| a.water[o as usize] > 0.8) {
                v.push((c, s as usize));
            }
        }
    }
    v
}

/// Rise of the ground per metre along the surface wind at a land column.
fn upslope(a: &Air, c: usize) -> f64 {
    let (e, nn) = a.surface_wind(c);
    let sp = ((e * e + nn * nn) as f64).sqrt();
    if sp < 0.3 {
        return 0.0;
    }
    let d = a.dir[c];
    let east = DVec3::Z.cross(d).try_normalize().unwrap_or(DVec3::X);
    let north = d.cross(east);
    let wdir = (east * e as f64 + north * nn as f64) / sp;
    let g = cube::Grid::new(a.m, a.r);
    let mut s = 0.0;
    let mut k = 0.0;
    for o in g.around(c as u32) {
        let step = a.dir[o as usize] - d;
        let dist = step.length() * a.r;
        let along = step.normalize().dot(wdir);
        s += along * (a.ground[o as usize] - a.ground[c]) as f64 / dist;
        k += along.abs();
    }
    s / k.max(1e-6)
}

/// Columns from each column to the nearest mostly-water column (breadth first).
fn coast_distance(a: &Air) -> Vec<usize> {
    let g = cube::Grid::new(a.m, a.r);
    let mut d = vec![usize::MAX; a.n];
    let mut q = std::collections::VecDeque::new();
    for c in 0..a.n {
        if a.water[c] >= 0.5 {
            d[c] = 0;
            q.push_back(c);
        }
    }
    while let Some(c) = q.pop_front() {
        for o in g.around(c as u32) {
            let o = o as usize;
            if d[o] == usize::MAX {
                d[o] = d[c] + 1;
                q.push_back(o);
            }
        }
    }
    d
}

/// Question 3, the smear: a puff of dirt over the sea at 45 degrees, carried 60 live steps of 1 s.
/// Returns its spread (standard deviation, in columns) and how many columns hold over 1 % of its peak.
fn puff(a: &mut Air) -> (f64, usize) {
    let c0 = (0..a.n).filter(|&c| a.water[c] > 0.99).min_by(|&x, &y| (a.lat[x].to_degrees() - 45.0).abs().total_cmp(&(a.lat[y].to_degrees() - 45.0).abs())).unwrap();
    a.dirt.iter_mut().for_each(|v| *v = 0);
    a.dirt[c0 * LAYERS] = 1_000_000_000;
    for _ in 0..60 {
        a.step(1.0);
    }
    let tot: f64 = (0..a.n).map(|c| a.dirt[c * LAYERS] as f64).sum();
    let mean = (0..a.n).fold(DVec3::ZERO, |s, c| s + a.dir[c] * a.dirt[c * LAYERS] as f64) / tot;
    let var = (0..a.n).map(|c| (a.dir[c] - mean).length_squared() * a.dirt[c * LAYERS] as f64).sum::<f64>() / tot;
    let cell = (a.area[c0] as f64).sqrt() / a.r;
    let peak = (0..a.n).map(|c| a.dirt[c * LAYERS]).max().unwrap() as f64;
    (var.sqrt() / cell, (0..a.n).filter(|&c| a.dirt[c * LAYERS] as f64 > 0.01 * peak).count())
}

struct Img {
    w: usize,
    h: usize,
    px: Vec<u8>,
}
impl Img {
    fn new(w: usize, h: usize) -> Img {
        Img { w, h, px: vec![16; w * h * 3] }
    }
    fn put(&mut self, x: usize, y: usize, c: [u8; 3]) {
        if x < self.w && y < self.h {
            self.px[(y * self.w + x) * 3..][..3].copy_from_slice(&c);
        }
    }
    fn save(&self, p: &std::path::Path) {
        let f = std::io::BufWriter::new(std::fs::File::create(p).unwrap());
        let mut e = png::Encoder::new(f, self.w as u32, self.h as u32);
        e.set_color(png::ColorType::Rgb);
        e.write_header().unwrap().write_image_data(&self.px).unwrap();
    }
}

fn ramp(stops: &[(f64, [f64; 3])], v: f64) -> [u8; 3] {
    let v = v.clamp(stops[0].0, stops[stops.len() - 1].0);
    let k = stops.windows(2).position(|w| v <= w[1].0).unwrap_or(stops.len() - 2);
    let (a, b) = (stops[k], stops[k + 1]);
    let t = (v - a.0) / (b.0 - a.0).max(1e-9);
    [0, 1, 2].map(|i| ((a.1[i] + (b.1[i] - a.1[i]) * t) * 255.0) as u8)
}

/// Equirectangular maps 480 x 240: land, mean temperature with wind, rain, day swing; and the cloud at
/// six times of the last day.
fn sheet(a: &Air, t: &[f64], mm: &[f64], tmin: &[f64], tmax: &[f64], clouds: &[(usize, Vec<f32>)], per_day: usize) -> Img {
    let (w, h) = (480, 240);
    let g = cube::Grid::new(a.m, a.r);
    let mut img = Img::new(w * 3, h * 3);
    let cell_at = |x: usize, y: usize| {
        let lon = (x as f64 + 0.5) / w as f64 * std::f64::consts::TAU - std::f64::consts::PI;
        let lat = std::f64::consts::FRAC_PI_2 - (y as f64 + 0.5) / h as f64 * std::f64::consts::PI;
        g.locate(DVec3::new(lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin())) as usize
    };
    let temp = [(-30.0, [0.3, 0.2, 0.6]), (-10.0, [0.4, 0.6, 1.0]), (0.0, [0.9, 0.95, 1.0]), (10.0, [0.5, 0.85, 0.4]), (20.0, [1.0, 0.85, 0.3]), (35.0, [0.8, 0.15, 0.1])];
    let wet = [(-1.0, [0.75, 0.55, 0.3]), (0.0, [0.95, 0.9, 0.6]), (0.6, [0.4, 0.8, 0.4]), (1.2, [0.1, 0.4, 0.8]), (1.8, [0.1, 0.1, 0.4])];
    for y in 0..h {
        for x in 0..w {
            let c = cell_at(x, y);
            let land = a.water[c] < 0.5;
            let hc = if land { ramp(&[(0.0, [0.3, 0.55, 0.25]), (40.0, [0.6, 0.5, 0.3]), (90.0, [0.95, 0.95, 0.95])], a.ground[c] as f64) } else { [20, 50, 120] };
            img.put(x, y, hc);
            img.put(w + x, y, ramp(&temp, t[c]));
            let mut r = ramp(&wet, mm[c].max(0.1).log10());
            if !land {
                r = r.map(|v| (v as f32 * 0.6) as u8);
            }
            img.put(2 * w + x, y, r);
            img.put(x, h + y, ramp(&[(0.0, [0.1, 0.1, 0.3]), (8.0, [0.6, 0.6, 0.3]), (20.0, [1.0, 0.3, 0.1])], tmax[c] - tmin[c]));
        }
    }
    // wind arrows over the temperature map, every 24 px
    for y in (12..h).step_by(24) {
        for x in (12..w).step_by(24) {
            let c = cell_at(x, y);
            let (e, nn) = a.surface_wind(c);
            for s in 0..10 {
                let f = s as f32 / 10.0 * 3.0;
                img.put(w + (x as f32 + e * f) as usize, (y as f32 - nn * f) as usize, [0, 0, 0]);
            }
            img.put(w + x, y, [255, 255, 255]);
        }
    }
    for (k, (s, cl)) in clouds.iter().enumerate().take(6) {
        let (ox, oy) = (if k < 2 { w * (k + 1) } else { w * (k - 2) }, if k < 2 { h } else { 2 * h });
        let sun = Air::sun(a.t - ((per_day - s) as f64 * DAY / per_day as f64));
        for y in 0..h {
            for x in 0..w {
                let c = cell_at(x, y);
                let lit = a.dir[c].dot(sun) > 0.0;
                let base = if a.water[c] < 0.5 { [60, 90, 50] } else { [20, 40, 90] };
                let v = (cl[c] / a.area[c] / 3.0).min(1.0);
                let px = base.map(|b| ((b as f32 * if lit { 1.0 } else { 0.4 }) * (1.0 - v) + 240.0 * v) as u8);
                img.put(ox + x, oy + y, px);
            }
        }
    }
    img
}
