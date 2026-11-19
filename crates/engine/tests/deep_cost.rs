//! What the deep world costs the chunk jobs and a world's load, on one thread, with no GPU in the way (the
//! optimisation batch of 2026-11-19): the height tiles a stand at four places asks, levels 0 to 6 (the spawn, a
//! round lava tube, a karst on Terra, a lava tube on Luna), each built once in a fresh world; and the world
//! build with its cave walk. Seed 1, the 4 km planet, generation 4. Run from a folder that holds `saves/bakes`
//! (`BAKES_ROOT=<checkout>`), on a play build:
//! `cargo test -p engine --profile play --test deep_cost -- --ignored --nocapture`.
use engine::world::{World, CS};
use std::time::Instant;

#[test]
#[ignore]
fn deep_world_tiles_and_load() {
    if let Ok(d) = std::env::var("BAKES_ROOT") {
        std::env::set_current_dir(d).unwrap();
    }
    let moon = engine::bodies::moon_columns(6144);
    let mut best = [f64::MAX; 7];
    for _ in 0..3 {
        let t = Instant::now();
        let w = World::baked(6144, moon, 1);
        let load = t.elapsed().as_secs_f64();
        let (b, l) = (&w.bodies[0], &w.bodies[w.moon]);
        let t = Instant::now();
        let caves = engine::caves::make(b, b.baked_seed().unwrap());
        let walk = t.elapsed().as_secs_f64();
        let t = Instant::now();
        let lcaves = engine::caves::make(l, l.baked_seed().unwrap());
        let lwalk = t.elapsed().as_secs_f64();
        assert_eq!((caves.len(), lcaves.len()), (b.caves.len(), l.caves.len()));
        let (sf, si, sj) = b.locate(b.spawn);
        let first = |body: &engine::world::Body, kind: u8| body.caves.iter().find(|c| c.kind == kind).map(|c| (c.face as usize, c.at.x, c.at.y)).unwrap();
        let sites = [(b, (sf, si, sj)), (b, first(b, 2)), (b, first(b, 3)), (l, first(l, 4))];
        let mut ms = [0.0; 4];
        let mut tiles = 0;
        for (k, (body, (f, i, j))) in sites.into_iter().enumerate() {
            let t = Instant::now();
            for level in 0..=6u8 {
                let (x, y) = ((i as i32 >> level).div_euclid(CS), (j as i32 >> level).div_euclid(CS));
                for (dx, dy) in (-3..=3).flat_map(|a| (-3..=3).map(move |c| (a, c))) {
                    let (x, y) = (x + dx, y + dy);
                    if x >= 0 && y >= 0 && x < (body.n >> level) / CS && y < (body.n >> level) / CS {
                        std::hint::black_box(body.tile(f as u8, level, x, y));
                        tiles += 1;
                    }
                }
            }
            ms[k] = t.elapsed().as_secs_f64() * 1e3;
        }
        for (k, v) in [load, walk, lwalk].into_iter().map(|s| s * 1e3).chain(ms).enumerate() {
            best[k] = best[k].min(v);
        }
        eprintln!("pass: {tiles} tiles, sites {ms:?} ms");
    }
    println!("{{\"load_ms\": {:.0}, \"terra_walk_ms\": {:.0}, \"luna_walk_ms\": {:.0}, \"tiles_spawn_ms\": {:.0}, \"tiles_tube_ms\": {:.0}, \"tiles_karst_ms\": {:.0}, \"tiles_luna_ms\": {:.0}}}", best[0], best[1], best[2], best[3], best[4], best[5], best[6]);
}
