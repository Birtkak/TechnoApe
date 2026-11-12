//! Process RAM / VRAM probes (Windows) and the headless meshing benchmark.
use engine::world::{generate, Gen, Key, World, CS};
use glam::IVec3;
use rayon::prelude::*;
use std::collections::HashMap;
use std::time::Instant;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows::Win32::System::Threading::{GetCurrentProcess, GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL};

/// A UTC date and time for file names, from seconds since 1970: 2026-11-10_14-03-22.
pub fn utc_stamp(secs: u64) -> String {
    // days to a civil date (Howard Hinnant's algorithm)
    let (z, s) = ((secs / 86400) as i64 + 719468, secs % 86400);
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let (d, m) = (doy - (153 * mp + 2) / 5 + 1, if mp < 10 { mp + 3 } else { mp - 9 });
    let y = yoe + era * 400 + (m <= 2) as i64;
    format!("{y:04}-{m:02}-{d:02}_{:02}-{:02}-{:02}", s / 3600, s / 60 % 60, s % 60)
}

#[test]
fn utc_stamps() {
    assert_eq!(utc_stamp(0), "1970-01-01_00-00-00");
    assert_eq!(utc_stamp(1_700_000_000), "2023-11-14_22-13-20");
    assert_eq!(utc_stamp(951_782_400), "2000-02-29_00-00-00");
}

/// Waits up to `limit` for the file the next game process writes once its window shows its first frame, and
/// removes it: the old window stays over the screen till then, so going between a world and the title never
/// shows the desktop (ADR 0038). False when it never came: the limit passed, or `gone` says the new process
/// has ended without showing anything (the wait stops at once then).
pub fn wait_shown(file: &std::path::Path, limit: std::time::Duration, mut gone: impl FnMut() -> bool) -> bool {

    let end = Instant::now() + limit;
    loop {
        if std::fs::remove_file(file).is_ok() {
            return true;
        }
        if Instant::now() >= end || gone() {
            // (it may have shown and ended between the two looks)
            return std::fs::remove_file(file).is_ok();
        }
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
}

#[test]
fn waits_for_the_next_window() {
    use std::time::Duration;
    let f = std::env::temp_dir().join(format!("technoape-shown-test-{}", std::process::id()));
    let _ = std::fs::remove_file(&f);
    // nobody writes it: the wait gives up at its limit
    let t = Instant::now();
    assert!(!wait_shown(&f, Duration::from_millis(100), || false));
    assert!(t.elapsed() >= Duration::from_millis(100));
    // the next process ended without showing anything (it failed to load): no waiting for it
    let t = Instant::now();
    assert!(!wait_shown(&f, Duration::from_secs(30), || true));
    assert!(t.elapsed() < Duration::from_secs(1), "waited {:?} for a process that is gone", t.elapsed());
    // written a little later: the wait sees it and clears it
    let g = f.clone();
    let w = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        std::fs::write(g, "").unwrap();
    });
    assert!(wait_shown(&f, Duration::from_secs(5), || false));
    w.join().unwrap();
    assert!(!f.exists());
}

pub fn ram_mb() -> f64 {
    let mut c = PROCESS_MEMORY_COUNTERS::default();
    unsafe {
        let _ = GetProcessMemoryInfo(GetCurrentProcess(), &mut c, std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32);
    }
    c.WorkingSetSize as f64 / 1048576.0
}

/// Puts the calling thread below normal priority.
pub fn below_normal() {
    unsafe {
        if let Err(e) = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL) {
            eprintln!("[technoape] worker priority unchanged: {e}");
        }
    }
}

/// This process's dedicated VRAM use on the adapter with the most dedicated memory (WDDM counts all APIs).
pub fn vram_mb() -> f64 {
    unsafe {
        let Ok(f) = CreateDXGIFactory1::<IDXGIFactory4>() else { return -1.0 };
        let mut best: Option<(usize, IDXGIAdapter1)> = None;
        let mut i = 0;
        while let Ok(a) = f.EnumAdapters1(i) {
            if let Ok(d) = a.GetDesc1() {
                if best.as_ref().map_or(true, |b| d.DedicatedVideoMemory > b.0) {
                    best = Some((d.DedicatedVideoMemory, a));
                }
            }
            i += 1;
        }
        let Some((_, a)) = best else { return -1.0 };
        let Ok(a3) = windows::core::Interface::cast::<IDXGIAdapter3>(&a) else { return -1.0 };
        let mut info = DXGI_QUERY_VIDEO_MEMORY_INFO::default();
        if a3.QueryVideoMemoryInfo(0, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, &mut info).is_err() {
            return -1.0;
        }
        info.CurrentUsage as f64 / 1048576.0
    }
}

/// Generate + mesh the level 0 chunks around the stand spot, single-threaded and on all workers.
pub fn meshbench(args: &HashMap<String, String>) {
    let num = |k: &str, d: i32| args.get(k).and_then(|v| v.parse().ok()).unwrap_or(d);
    let planet = num("planet", engine::bodies::BODIES[0].n);
    // at=x,y,z (the see-through spike, ADR 0068): the baked planet's chunks around that place, else the
    // noise land's around the middle of the +y face
    let at = args.get("at").map(|s| s.split(',').map(|v| v.parse::<f64>().unwrap()).collect::<Vec<_>>());
    let world = if at.is_some() { World::baked(planet, engine::bodies::columns(1, planet), 1337) } else { World::new(planet, engine::bodies::columns(1, planet), 1337) };
    let greedy = num("greedy", 1) == 1;
    let radius = num("chunks", 6);
    let b = &world.bodies[0];
    let (face, ci, cj, top) = match &at {
        Some(v) => {
            let d = glam::DVec3::new(v[0], v[1], v[2]).normalize();
            let (f, i, j) = b.locate(d);
            (f as u8, i as i32 / CS, j as i32 / CS, b.layer_of(b.top(d)) as i32 / CS)
        }
        None => (2, b.n / CS / 2, b.n / CS / 2, b.layer_of(b.radius) as i32 / CS),
    };
    let mut keys = Vec::new();
    for z in top - 3..=top + 2 {
        for y in -radius..=radius {
            for x in -radius..=radius {
                keys.push(Key { body: 0, face, level: 0, pos: IVec3::new(ci + x, cj + y, z) });
            }
        }
    }
    let near = at.is_some().then_some(1u32);
    let job = |k: &Key| -> (f64, f64, usize, u8) {
        let t0 = Instant::now();
        let g = generate(b, *k, &[]);
        let t1 = Instant::now();
        let (q, kind) = match g {
            Gen::Cells(c) => {
                // with at=, the fastest of five (another process only ever slows a run); else once, as before
                let mut best = f64::MAX;
                let mut n = 0;
                for _ in 0..if near.is_some() { 5 } else { 1 } {
                    let t = Instant::now();
                    n = engine::mesh::mesh_at(&c, if greedy { b.lattice(0) as usize } else { 1 }, near).quads.len();
                    best = best.min(t.elapsed().as_secs_f64() * 1e6);
                }
                return ((t1 - t0).as_secs_f64() * 1e6, best, n, 2);
            }
            Gen::Empty => (0, 0),
            Gen::Solid => (0, 1),
        };
        ((t1 - t0).as_secs_f64() * 1e6, t1.elapsed().as_secs_f64() * 1e6, q, kind)
    };
    let t = Instant::now();
    let single: Vec<_> = keys.iter().map(job).collect();
    let st = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let _par: Vec<_> = keys.par_iter().map(job).collect();
    let pt = t.elapsed().as_secs_f64();
    let surf: Vec<_> = single.iter().filter(|r| r.3 == 2).collect();
    let n = surf.len().max(1) as f64;
    let gen_us = surf.iter().map(|r| r.0).sum::<f64>() / n;
    let mesh_us = surf.iter().map(|r| r.1).sum::<f64>() / n;
    let quads = surf.iter().map(|r| r.2).sum::<usize>() as f64 / n;
    let shortcut_us = single.iter().filter(|r| r.3 != 2).map(|r| r.0).sum::<f64>() / single.iter().filter(|r| r.3 != 2).count().max(1) as f64;
    println!(
        "{{\"run\":\"meshbench\",\"spike\":{},\"greedy\":{},\"chunks\":{},\"surface_chunks\":{},\"gen_us\":{:.1},\"mesh_us\":{:.1},\"shortcut_us\":{:.1},\"quads_per_chunk\":{:.0},\"bytes_per_chunk\":{:.0},\"single_chunks_per_s\":{:.0},\"parallel_chunks_per_s\":{:.0},\"threads\":{}}}",
        engine::mesh::SPIKE.load(std::sync::atomic::Ordering::Relaxed), greedy as u8, keys.len(), surf.len(), gen_us, mesh_us, shortcut_us, quads, quads * 8.0,
        keys.len() as f64 / st, keys.len() as f64 / pt, rayon::current_num_threads()
    );
}
