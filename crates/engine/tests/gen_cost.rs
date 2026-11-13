//! What making the chunks near ore bodies costs (ADR 0095): every level-0 chunk within a halo's reach of a body
//! at a depth on the 4 km planet (seed 1337), made by `generate` on one thread, the best of three passes. Run
//! from a folder that holds `saves/bakes` (`BAKES_ROOT=<checkout>`), on a play or release build:
//! `cargo test -p engine --profile play --test gen_cost -- --ignored --nocapture`.
use engine::ore::MINERALS;
use engine::world::{generate, Key, World, CS};
use glam::IVec3;
use std::time::Instant;

#[test]
#[ignore]
fn generation_near_ore_bodies() {
    if let Ok(d) = std::env::var("BAKES_ROOT") {
        std::env::set_current_dir(d).unwrap();
    }
    let w = World::baked(6144, engine::bodies::columns(1, 6144), 1337);
    let b = &w.bodies[0];
    let mut keys = Vec::new();
    for o in &b.ores {
        if MINERALS.kinds[o.kind as usize].shape.follows_ground() {
            continue;
        }
        let r = o.a.max(o.b) + 14.0;
        let span = |lo: f64, hi: f64| (lo.floor() as i32).div_euclid(CS)..=(hi.ceil() as i32).div_euclid(CS);
        for x in span(o.at.x - r, o.at.x + r) {
            for y in span(o.at.y - r, o.at.y + r) {
                for z in span(o.at.z - o.h - 14.0, o.at.z + o.h + 14.0) {
                    keys.push(Key { body: 0, face: o.face, level: 0, pos: IVec3::new(x, y, z) });
                }
            }
        }
    }
    keys.sort_by_key(|k| (k.face, k.pos.x, k.pos.y, k.pos.z));
    keys.dedup();
    let mut best = f64::MAX;
    for _ in 0..4 {
        let t = Instant::now();
        for k in &keys {
            std::hint::black_box(generate(b, *k, &[]));
        }
        best = best.min(t.elapsed().as_secs_f64());
    }
    println!("{{\"chunks\": {}, \"ms\": {:.1}, \"us_per_chunk\": {:.1}}}", keys.len(), best * 1e3, best * 1e6 / keys.len() as f64);
}
