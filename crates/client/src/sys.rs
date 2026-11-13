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
    // at=x,y,z (ADR 0068, 0069): the baked planet's chunks around that place, else the
    // noise land's around the middle of the +y face
    let at = args.get("at").map(|s| s.split(',').map(|v| v.parse::<f64>().unwrap()).collect::<Vec<_>>());
    let world = if at.is_some() { World::baked(planet, engine::bodies::columns(1, planet), 1337) } else { World::new(planet, engine::bodies::columns(1, planet), 1337) };
    let greedy = num("greedy", 1) == 1;
    // leaves=fancy: level 0 as fancy leaves mesh it (ADR 0069), else as today
    let cutout = args.get("leaves").is_some_and(|v| v == "fancy");
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
    // clutter=off|few|normal|many (ADR 0082): its quads laid after each mesh, timed apart, in summer (time=)
    let density = args.get("clutter").and_then(|v| ["off", "few", "normal", "many"].iter().position(|n| n == v)).map_or(1.0, |l| [0.0, 0.25, 1.0, 4.0][l]);
    let pal = engine::palette::Palette::parse(include_str!("../../../data/palette.toml")).unwrap();
    let still = engine::clutter::Still::load(std::path::Path::new("data/models"), &pal).0;
    let t_sky = args.get("time").and_then(|v| v.parse::<f64>().ok()).unwrap_or(23625.0);
    let clutter_us = std::sync::Mutex::new((0.0f64, 0usize, 0usize));
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
                    n = engine::mesh::mesh_cut(&c, if greedy { b.lattice(0) as usize } else { 1 }, cutout, false).quads.len();
                    best = best.min(t.elapsed().as_secs_f64() * 1e6);
                }
                let t = Instant::now();
                let cq = engine::clutter::quads(b, *k, &c, &[], &still, t_sky, density, &[]).len();
                let mut cu = clutter_us.lock().unwrap();
                (cu.0, cu.1, cu.2) = (cu.0 + t.elapsed().as_secs_f64() * 1e6, cu.1 + cq, cu.2 + 1);
                drop(cu);
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
        "{{\"run\":\"meshbench\",\"greedy\":{},\"chunks\":{},\"surface_chunks\":{},\"gen_us\":{:.1},\"mesh_us\":{:.1},\"shortcut_us\":{:.1},\"quads_per_chunk\":{:.0},\"bytes_per_chunk\":{:.0},\"single_chunks_per_s\":{:.0},\"parallel_chunks_per_s\":{:.0},\"threads\":{},\"clutter_us\":{:.1},\"clutter_quads_per_chunk\":{:.0}}}",
        greedy as u8, keys.len(), surf.len(), gen_us, mesh_us, shortcut_us, quads, quads * 8.0,
        keys.len() as f64 / st, keys.len() as f64 / pt, rayon::current_num_threads(),
        { let c = clutter_us.lock().unwrap(); c.0 / c.2.max(1) as f64 }, { let c = clutter_us.lock().unwrap(); c.1 as f64 / c.2.max(1) as f64 }
    );
}
