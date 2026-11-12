//! The per-seed report of what the ground holds (W4): `technoape-server report=1 seed=1337 planet=6144` prints
//! one JSON line: each rock province's share of the surface, the caves, and every mineral's bodies, tonnage,
//! depth and distance from the spawn against `data/minerals.toml`'s budget (ADR 0063).
use engine::bake::rock::NAMES;
use engine::world::World;

/// The report as one JSON line.
pub fn report(w: &World) -> String {
    let b = &w.bodies[0];
    let map = b.map().expect("a baked planet");
    let mut share = [0f64; NAMES.len()];
    for &p in &map.rock {
        share[p as usize] += 1.0 / map.rock.len() as f64;
    }
    let provinces: Vec<String> = NAMES.iter().zip(share).map(|(n, s)| format!("\"{n}\": {:.3}", s)).collect();
    let rock_of = |c: &engine::world::Cave| engine::rock::ROCKS.at(b.rock_at(c.face as usize, c.at.x, c.at.y, b.dir(c.face as usize, c.at.x, c.at.y)), b.layer_r(c.at.z) as f32, 0);
    let lime = engine::blocks::BLOCKS.id("limestone");
    let (in_lime, depth): (usize, Vec<f64>) = (b.caves.iter().filter(|c| rock_of(c) == lime).count(), b.caves.iter().map(|c| b.ground_r(c.face as usize, c.at.x, c.at.y) - b.layer_r(c.at.z)).collect());
    let (dmin, dmax) = depth.iter().fold((f64::MAX, f64::MIN), |(a, z), &d| (a.min(d), z.max(d)));
    let open = depth.iter().zip(&b.caves).filter(|(d, c)| **d < c.r).count();
    // at the ground and at the crust's floor
    let (f, i, j) = b.locate(b.spawn);
    let ground = b.layer_of(b.ground_r(f, i, j));
    let heat = |z: f64| b.heat(f as u8, i as i32, j as i32, z).unwrap_or(f32::NAN);
    format!(
        "{{\"seed\": {}, \"planet\": {}, \"spawn\": [{:.5}, {:.5}, {:.5}], \"made_hash\": \"{:016x}\", \"provinces\": {{{}}}, \"caves\": {{\"balls\": {}, \"in_limestone\": {}, \"in_basalt\": {}, \"depth_m\": [{:.1}, {:.1}], \"open_to_the_sky\": {}}}, \"heat_c_under_the_spawn\": [{:.1}, {:.1}], \"minerals\": {{{}}}}}",
        map.seed, map.n, b.spawn.x, b.spawn.y, b.spawn.z, made_hash(b), provinces.join(", "), b.caves.len(), in_lime, b.caves.len() - in_lime, dmin, dmax, open, heat(ground), heat(0.0), minerals(w).join(", ")
    )
}

/// Ore blocks of each element on the 4 km planet, from `docs/design/ore-budget.md` ("What that is in the
/// ground"). shortcut: here until `data/budget.toml` holds the budget; move it there with the recipes.
const BUDGET: [(&str, u32); 8] = [("copper", 330_000), ("tin", 18_000), ("iron", 800_000), ("coal", 1_100_000), ("aluminium", 190_000), ("lead", 35_000), ("gold", 4_000), ("uranium", 1_000)];

/// Every element's bodies as generated: blocks and ingots in the ground, the budget's blocks (scaled by area),
/// how many bodies, the nearest one's distance from the spawn over the ground and the depth range of their
/// middles, metres; then each kind's.
fn minerals(w: &World) -> Vec<String> {
    use engine::ore::MINERALS;
    let b = &w.bodies[0];
    let held = engine::ore::count(b);
    let scale = (b.n as f64 / 6144.0).powi(2);
    let away = |o: &engine::ore::OreBody| b.radius * b.dir(o.face as usize, o.at.x, o.at.y).angle_between(b.spawn);
    let depth = |o: &engine::ore::OreBody| if MINERALS.kinds[o.kind as usize].shape.follows_ground() { o.at.z } else { b.ground_r(o.face as usize, o.at.x, o.at.y) - b.layer_r(o.at.z) };
    let line = |name: &str, of: &dyn Fn(&engine::ore::OreBody) -> bool, budget: Option<u32>| {
        let ks: Vec<usize> = (0..b.ores.len()).filter(|&k| of(&b.ores[k])).collect();
        let (blocks, ingots) = ks.iter().fold((0u64, 0u64), |(a, i), &k| (a + held[k].blocks as u64, i + held[k].ingots as u64));
        let near = ks.iter().map(|&k| away(&b.ores[k])).fold(f64::MAX, f64::min);
        let (lo, hi) = ks.iter().map(|&k| depth(&b.ores[k])).fold((f64::MAX, f64::MIN), |(a, z), d| (a.min(d), z.max(d)));
        let budget = budget.map_or(String::new(), |n| format!(", \"budget_blocks\": {:.0}", n as f64 * scale));
        format!("\"{name}\": {{\"bodies\": {}, \"blocks\": {blocks}, \"ingots\": {ingots}{budget}, \"nearest_m\": {:.0}, \"depth_m\": [{:.1}, {:.1}]}}", ks.len(), if ks.is_empty() { -1.0 } else { near }, lo, hi)
    };
    let mut out: Vec<String> = BUDGET.iter().map(|&(e, n)| line(e, &|o| MINERALS.kinds[o.kind as usize].gives == e, Some(n))).collect();
    for (k, kind) in MINERALS.kinds.iter().enumerate() {
        out.push(line(&format!("kind {}", kind.name), &|o| o.kind as usize == k, None));
    }
    out
}

/// The caves and ore bodies as made, as one number: a Linux server and a Windows client must print the same
/// (ADR 0063: both make them from the map with the platform's float functions).
fn made_hash(b: &engine::world::Body) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    let mut put = |v: u64| {
        for byte in v.to_le_bytes() {
            h = (h ^ byte as u64).wrapping_mul(0x100_0000_01b3);
        }
    };
    for c in &b.caves {
        [c.face as u64, c.at.x.to_bits(), c.at.y.to_bits(), c.at.z.to_bits(), c.r.to_bits()].into_iter().for_each(&mut put);
    }
    for o in &b.ores {
        [o.kind as u64, o.face as u64, o.at.x.to_bits(), o.at.y.to_bits(), o.at.z.to_bits(), o.a.to_bits(), o.b.to_bits(), o.h.to_bits(), o.angle.to_bits()].into_iter().for_each(&mut put);
    }
    h
}
