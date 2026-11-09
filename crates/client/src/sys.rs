//! Process RAM / VRAM probes (Windows) and the headless meshing benchmark.
use engine::mesh::mesh;
use engine::world::{generate, Gen, Key, World, CS};
use glam::IVec3;
use rayon::prelude::*;
use std::collections::HashMap;
use std::time::Instant;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows::Win32::System::Threading::GetCurrentProcess;

pub fn ram_mb() -> f64 {
    let mut c = PROCESS_MEMORY_COUNTERS::default();
    unsafe {
        let _ = GetProcessMemoryInfo(GetCurrentProcess(), &mut c, std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32);
    }
    c.WorkingSetSize as f64 / 1048576.0
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
    let world = World::new(num("planet", 3072), 512);
    let greedy = num("greedy", 0) == 1;
    let radius = num("chunks", 6);
    let b = &world.bodies[0];
    // the middle of the +y face, through the terrain's height range
    let mid = b.n / CS / 2;
    let top = b.layer_of(b.radius) as i32 / CS;
    let mut keys = Vec::new();
    for z in top - 3..=top + 2 {
        for y in -radius..=radius {
            for x in -radius..=radius {
                keys.push(Key { body: 0, face: 2, level: 0, pos: IVec3::new(mid + x, mid + y, z) });
            }
        }
    }
    let job = |k: &Key| -> (f64, f64, usize, u8) {
        let t0 = Instant::now();
        let g = generate(b, *k, &[]);
        let t1 = Instant::now();
        let (q, kind) = match g {
            Gen::Cells(c) => (mesh(&c, greedy).quads.len(), 2),
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
        "{{\"run\":\"meshbench\",\"greedy\":{},\"chunks\":{},\"surface_chunks\":{},\"gen_us\":{:.1},\"mesh_us\":{:.1},\"shortcut_us\":{:.1},\"quads_per_chunk\":{:.0},\"bytes_per_chunk\":{:.0},\"single_chunks_per_s\":{:.0},\"parallel_chunks_per_s\":{:.0},\"threads\":{}}}",
        greedy as u8, keys.len(), surf.len(), gen_us, mesh_us, shortcut_us, quads, quads * 8.0,
        keys.len() as f64 / st, keys.len() as f64 / pt, rayon::current_num_threads()
    );
}
