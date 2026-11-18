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
    let pressure = |z: f64| b.pressure(f as u8, i as i32, j as i32, z).unwrap_or(f32::NAN);
    // under the spawn and under the nearest column of the hottest and the coldest province (by `heat` a metre)
    let p = &engine::rock::ROCKS.provinces;
    // (the land's provinces: the ocean floor, the last, lies under the sea)
    let land = || (0..p.len() as u8 - 1).map(|k| (p[k as usize].heat, k));
    let (hot, cold) = (land().max_by(|a, c| a.0.total_cmp(&c.0)).unwrap().1, land().min_by(|a, c| a.0.total_cmp(&c.0)).unwrap().1);
    let deep: Vec<String> = [("spawn", None), ("hottest", Some(hot)), ("coldest", Some(cold))].iter().filter_map(|&(name, prov)| deep_column(b, f as u8, i as i32, j as i32, prov).map(|c| format!("\"{name}\": {c}"))).collect();
    // the caves and the crust's ore bodies alone: the hash of the same world before the mantle's minerals
    let crust: Vec<engine::ore::OreBody> = b.ores.iter().copied().filter(|o| !engine::ore::MINERALS.kinds[o.kind as usize].deep()).collect();
    format!(
        "{{\"seed\": {}, \"planet\": {}, \"spawn\": [{:.5}, {:.5}, {:.5}], \"made_hash\": \"{:016x}\", \"made_hash_crust\": \"{:016x}\", \"provinces\": {{{}}}, \"caves\": {{\"balls\": {}, \"in_limestone\": {}, \"in_basalt\": {}, \"depth_m\": [{:.1}, {:.1}], \"open_to_the_sky\": {}}}, \"heat_c_under_the_spawn\": [{:.1}, {:.1}], \"pressure_mpa_under_the_spawn\": [{:.3}, {:.3}], \"deep\": {{{}}}, \"minerals\": {{{}}}}}",
        map.seed, map.n, b.spawn.x, b.spawn.y, b.spawn.z, made_hash(&b.caves, &b.ores), made_hash(&b.caves, &crust), provinces.join(", "), b.caves.len(), in_lime, b.caves.len() - in_lime, dmin, dmax, open, heat(ground), heat(0.0), pressure(ground), pressure(0.0), deep.join(", "), minerals(w).join(", ")
    )
}

/// A column's ground, each mantle boundary (the first layer, going down, of that rock or a deeper one: the first
/// is the crust's floor) and the floor, each as its depth under the ground (m), heat (C) and pressure (MPa):
/// the spawn's column (`province` None) or the nearest of that province on the spawn's face, searched in steps of
/// 32 columns; None where there is none.
fn deep_column(b: &engine::world::Body, face: u8, i0: i32, j0: i32, province: Option<u8>) -> Option<String> {
    use engine::rock::ROCKS;
    let (i, j) = match province {
        None => (i0, j0),
        Some(p) => (0..b.n).step_by(32).flat_map(|i| (0..b.n).step_by(32).map(move |j| (i, j))).filter(|&(i, j)| b.col_rock(face, i, j).province == p).min_by_key(|&(i, j)| (i - i0).pow(2) + (j - j0).pow(2))?,
    };
    let (ground, rock) = (b.col_height(face, i, j), b.col_rock(face, i, j));
    let line = |name: &str, z: f64| format!("{{\"at\": \"{name}\", \"depth_m\": {:.1}, \"heat_c\": {:.1}, \"mpa\": {:.3}}}", ground - b.layer_r(z), b.heat(face, i, j, z).unwrap_or(f32::NAN), b.pressure(face, i, j, z).unwrap_or(f32::NAN));
    let mut out = vec![line("ground", b.layer_of(ground))];
    for (k, d) in ROCKS.mantle.iter().enumerate() {
        let top = (0..b.layer_of(ground) as i32).rev().find(|&z| ROCKS.mantle[k..].iter().any(|m| m.block == b.rock(rock, b.layer_r(z as f64 + 0.5) as f32, 0)));
        if let Some(z) = top {
            out.push(line(&engine::blocks::BLOCKS.get(d.block).unwrap().name, z as f64 + 0.5));
        }
    }
    out.push(line("floor", 0.0));
    Some(format!("{{\"province\": \"{}\", \"lines\": [{}]}}", engine::bake::rock::NAMES.get(rock.province as usize).unwrap_or(&"none"), out.join(", ")))
}

/// Ore blocks of each element on the 4 km planet, from `docs/design/ore-budget.md` ("What that is in the
/// ground"). shortcut: here until `data/budget.toml` holds the budget; move it there with the recipes.
const BUDGET: [(&str, u32); 9] = [("copper", 330_000), ("tin", 18_000), ("iron", 800_000), ("coal", 1_100_000), ("aluminium", 190_000), ("lead", 35_000), ("gold", 4_000), ("uranium", 1_000), ("diamond", 480)];

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
        // where the nearest one lies, a direction from the planet's centre (`run=stand at=`)
        let at = ks.iter().min_by(|&&x, &&y| away(&b.ores[x]).total_cmp(&away(&b.ores[y]))).map_or(glam::DVec3::ZERO, |&k| b.dir(b.ores[k].face as usize, b.ores[k].at.x, b.ores[k].at.y));
        let (lo, hi) = ks.iter().map(|&k| depth(&b.ores[k])).fold((f64::MAX, f64::MIN), |(a, z), d| (a.min(d), z.max(d)));
        let budget = budget.map_or(String::new(), |n| format!(", \"budget_blocks\": {:.0}", n as f64 * scale));
        format!("\"{name}\": {{\"bodies\": {}, \"blocks\": {blocks}, \"ingots\": {ingots}{budget}, \"nearest_m\": {:.0}, \"nearest_at\": [{:.6}, {:.6}, {:.6}], \"depth_m\": [{:.1}, {:.1}]}}", ks.len(), if ks.is_empty() { -1.0 } else { near }, at.x, at.y, at.z, lo, hi)
    };
    let mut out: Vec<String> = BUDGET.iter().map(|&(e, n)| line(e, &|o| MINERALS.kinds[o.kind as usize].gives == e, Some(n))).collect();
    // the altered rock round each kind's bodies (ADR 0095), blocks per grade outermost first: no metal
    let halo = engine::ore::count_halo(b);
    for (k, kind) in MINERALS.kinds.iter().enumerate() {
        let l = line(&format!("kind {}", kind.name), &|o| o.kind as usize == k, None);
        // the samples lying over its bodies (ADR 0138)
        let l = if kind.sample.is_some() { format!("{}, \"samples\": {}}}", &l[..l.len() - 1], samples(b, k)) } else { l };
        if kind.halo.is_empty() {
            out.push(l);
            continue;
        }
        let grades: Vec<String> = kind.halo.iter().enumerate().map(|(g, &m)| format!("\"{}\": {}", engine::blocks::BLOCKS.get(m).unwrap().name, (0..b.ores.len()).filter(|&o| b.ores[o].kind as usize == k).map(|o| halo[o][g] as u64).sum::<u64>())).collect();
        out.push(format!("{}, \"halo_blocks\": {{{}}}}}", &l[..l.len() - 1], grades.join(", ")));
    }
    out
}

/// The samples of kind k lying on the generated ground over its bodies' spreads (`clutter::sample`), each column
/// once.
fn samples(b: &engine::world::Body, k: usize) -> usize {
    use engine::world::CS;
    let (Some(seed), Some(row)) = (b.baked_seed(), engine::clutter::SAMPLES[k]) else { return 0 };
    let mut cols = std::collections::BTreeSet::new();
    let mut tiles = std::collections::HashMap::new();
    for o in b.ores.iter().filter(|o| o.kind as usize == k) {
        let r = o.a.max(o.b).max(engine::ore::SAMPLE_SPREAD_M).ceil() as i32 + 1;
        let (ci, cj) = (o.at.x as i32, o.at.y as i32);
        for (i, j) in (ci - r..=ci + r).flat_map(|i| (cj - r..=cj + r).map(move |j| (i, j))) {
            if !(0..b.n).contains(&i) || !(0..b.n).contains(&j) || o.spread(i, j).is_none() {
                continue;
            }
            let tile = tiles.entry((i.div_euclid(CS), j.div_euclid(CS))).or_insert_with(|| b.tile(o.face, 0, i.div_euclid(CS), j.div_euclid(CS)));
            if engine::clutter::ground(b, tile, o.face, i, j).is_some() && engine::clutter::sample(b, seed, o.face, i, j).is_some_and(|t| t.row == row) {
                cols.insert((o.face, i, j));
            }
        }
        tiles.clear();
    }
    cols.len()
}

/// The caves and ore bodies as made, as one number: a Linux server and a Windows client must print the same
/// (ADR 0063: both make them from the map with the platform's float functions).
fn made_hash(caves: &[engine::world::Cave], ores: &[engine::ore::OreBody]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    let mut put = |v: u64| {
        for byte in v.to_le_bytes() {
            h = (h ^ byte as u64).wrapping_mul(0x100_0000_01b3);
        }
    };
    for c in caves {
        [c.face as u64, c.at.x.to_bits(), c.at.y.to_bits(), c.at.z.to_bits(), c.r.to_bits()].into_iter().for_each(&mut put);
    }
    for o in ores {
        [o.kind as u64, o.face as u64, o.at.x.to_bits(), o.at.y.to_bits(), o.at.z.to_bits(), o.a.to_bits(), o.b.to_bits(), o.h.to_bits(), o.angle.to_bits()].into_iter().for_each(&mut put);
    }
    h
}
