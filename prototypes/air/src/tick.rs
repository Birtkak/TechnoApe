//! W3's tick spike (README, "The live grid's tick"): `mode=tick` and `mode=year`.
use crate::radius;
use air::{Air, Params, LAYERS};
use std::sync::mpsc::channel;
use std::time::Instant;

fn sky_air(m: usize, r: f64) -> Air {
    let (g, w) = air::test_world(m, r);
    let mut a = Air::new(m, r, g, w, Params::default());
    a.tilt = 20f64.to_radians();
    a.day = 1800.0;
    a
}

/// A live step of `dt` seconds on Terra and the huge planet on 1, 4 and all threads, and what the server's own
/// thread pays when the step runs on a worker: handing the state over, taking it back with a view (surface
/// temperature, rain and runoff per column) built on the worker.
pub fn tick(dt: f32) {
    for planet in [6144u32, 12288] {
        let m = (planet as f64 / 128.0).round() as usize;
        let r = radius(planet);
        let mut row = vec![format!("\"planet\":{planet}"), format!("\"columns\":{}", 6 * m * m)];
        for threads in [1usize, 4, 0] {
            let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
            let ms = pool.install(|| {
                let mut a = sky_air(m, r);
                for _ in 0..50 {
                    a.step(dt);
                }
                let t = Instant::now();
                for _ in 0..200 {
                    a.step(dt);
                }
                t.elapsed().as_secs_f64() * 1000.0 / 200.0
            });
            let name = if threads == 0 { "all".to_string() } else { threads.to_string() };
            row.push(format!("\"step_ms_{name}\":{ms:.3}"));
            // one step every dt seconds of 20 ticks each: the share of a tick, averaged
            row.push(format!("\"per_tick_ms_{name}\":{:.4}", ms / (dt as f64 * 20.0)));
        }
        let (to_worker, work) = channel::<Box<Air>>();
        let (done, back) = channel::<(Box<Air>, Vec<f32>, Vec<u32>, Vec<u32>, f64)>();
        let worker = std::thread::spawn(move || {
            let mut runoff_sum: Vec<u32> = Vec::new();
            while let Ok(mut a) = work.recv() {
                let t = Instant::now();
                a.step(dt);
                runoff_sum.resize(a.n, 0);
                for (s, &x) in runoff_sum.iter_mut().zip(&a.runoff) {
                    *s = s.saturating_add(x);
                }
                let temp: Vec<f32> = (0..a.n).map(|c| a.surface_t(c)).collect();
                let fell = a.fell.clone();
                let ms = t.elapsed().as_secs_f64() * 1000.0;
                let _ = done.send((a, temp, fell, runoff_sum.clone(), ms));
            }
        });
        let mut a = Box::new(sky_air(m, r));
        let (mut send_us, mut take_us, mut job_ms) = (Vec::new(), Vec::new(), Vec::new());
        for _ in 0..60 {
            let t = Instant::now();
            to_worker.send(a).unwrap();
            send_us.push(t.elapsed().as_secs_f64() * 1e6);
            // the ticks between hand-over and collection
            std::thread::sleep(std::time::Duration::from_millis(40));
            let t = Instant::now();
            let (b, temp, _fell, _runoff, ms) = back.recv().unwrap();
            take_us.push(t.elapsed().as_secs_f64() * 1e6);
            job_ms.push(ms);
            std::hint::black_box(&temp);
            a = b;
        }
        drop(to_worker);
        worker.join().unwrap();
        let med = |v: &mut Vec<f64>| {
            v.sort_by(f64::total_cmp);
            v[v.len() / 2]
        };
        let worst = |v: &Vec<f64>| v.iter().cloned().fold(0.0, f64::max);
        row.push(format!(
            "\"handoff_send_us\":{:.1},\"handoff_take_us\":{:.1},\"handoff_worst_us\":{:.1},\"job_ms\":{:.3}",
            med(&mut send_us),
            med(&mut take_us),
            worst(&send_us).max(worst(&take_us)),
            med(&mut job_ms)
        ));
        let bytes = a.n * LAYERS * 4 * 4 + a.edges() * LAYERS * 4 + a.n * 4 * 6;
        row.push(format!("\"state_mb\":{:.2}", bytes as f64 / 1048576.0));
        println!("{{{}}}", row.join(","));
    }
}

/// The bake's year with seasons: one continuous year of `dt` steps against four held days of the year (the
/// sun's declination at 1/16, 3/16, 9/16 and 11/16 of it, each settled), both after `spin` days settled at the
/// equinox. Zonal means of the surface air and rain, the difference per column, and the time each took.
pub fn year(planet: u32, dt: f32, spin: usize, held: usize, years: usize) {
    let m = (planet as f64 / 128.0).round() as usize;
    let r = radius(planet);
    let mut a = sky_air(m, r);
    let n = a.n;
    let t0 = Instant::now();
    a.hold = Some(0.0);
    a.settle(dt, spin, spin, 0.0, 0.0);
    let spin_s = t0.elapsed().as_secs_f64();
    let start_t = a.t;
    let base = (a.theta.clone(), a.vap.clone(), a.cloud.clone(), a.wind.clone(), a.tg.clone(), a.soil.clone(), a.snow.clone(), a.sea);
    let t1 = Instant::now();
    let (mut st, mut sr, mut days) = (vec![0f64; n], vec![0f64; n], Vec::new());
    // AIR_EQ=1: one held day at the equinox (the bake without seasons) instead of four
    let phases: &[f64] = if std::env::var("AIR_EQ").is_ok() { &[0.0] } else { &[1.0, 3.0, 11.0, 9.0] };
    for &ph in phases {
        a.hold = Some(ph / 16.0 * 54.0 * 1800.0);
        let me = a.settle(dt, held, held, 0.0, 0.0);
        days.push(me.days);
        for c in 0..n {
            st[c] += me.theta[c] as f64 / phases.len() as f64;
            sr[c] += me.rain[c] as f64 / phases.len() as f64;
        }
    }
    let held_s = t1.elapsed().as_secs_f64();
    (a.theta, a.vap, a.cloud, a.wind, a.tg, a.soil, a.snow, a.sea) = base;
    a.t = start_t - start_t.rem_euclid(54.0 * 1800.0);
    a.hold = None;
    let per = (54.0 * 1800.0 / dt as f64).round() as usize;
    let t2 = Instant::now();
    let (mut ct, mut cr) = (vec![0f64; n], vec![0f64; n]);
    // years=2: the "held" columns are the first year and the second is compared with it
    for y in 0..years {
        if y > 0 {
            (st, sr) = (ct, cr);
            (ct, cr) = (vec![0f64; n], vec![0f64; n]);
        }
        for _ in 0..per {
            a.step(dt);
            for c in 0..n {
                ct[c] += a.theta[c * LAYERS] as f64 / per as f64;
                cr[c] += a.fell[c] as f64;
            }
        }
        for c in 0..n {
            cr[c] = cr[c] / 54.0 / 4096.0 / a.area[c] as f64 * 1000.0;
        }
    }
    let year_s = t2.elapsed().as_secs_f64() / years as f64;
    let bands: Vec<String> = (0..9)
        .map(|b| {
            let lo = b as f32 * 10.0;
            let cs: Vec<usize> = (0..n).filter(|&c| (lo..lo + 10.0).contains(&a.lat[c].to_degrees().abs())).collect();
            let mean = |v: &Vec<f64>| cs.iter().map(|&c| v[c]).sum::<f64>() / cs.len() as f64;
            format!("[{},{:.2},{:.2},{:.3},{:.3}]", b * 10, mean(&st), mean(&ct), mean(&sr), mean(&cr))
        })
        .collect();
    let dtk = (0..n).map(|c| (st[c] - ct[c]).abs()).sum::<f64>() / n as f64;
    let (mx, my) = (sr.iter().sum::<f64>() / n as f64, cr.iter().sum::<f64>() / n as f64);
    let cov: f64 = (0..n).map(|c| (sr[c] - mx) * (cr[c] - my)).sum();
    let vx: f64 = (0..n).map(|c| (sr[c] - mx).powi(2)).sum();
    let vy: f64 = (0..n).map(|c| (cr[c] - my).powi(2)).sum();
    println!(
        "{{\"planet\":{planet},\"dt\":{dt},\"spin_days\":{spin},\"spin_s\":{spin_s:.1},\"held_days\":{days:?},\"held_s\":{held_s:.1},\"year_s\":{year_s:.1},\"mean_abs_dT\":{dtk:.3},\"rain_held\":{mx:.4},\"rain_year\":{my:.4},\"rain_correlation\":{:.3},\"bands_lat_Theld_Tyear_Rheld_Ryear\":[{}]}}",
        cov / (vx * vy).sqrt(),
        bands.join(",")
    );
}
