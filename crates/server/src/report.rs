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
        "{{\"seed\": {}, \"planet\": {}, \"gen\": {}, \"spawn\": [{:.5}, {:.5}, {:.5}], \"made_hash\": \"{:016x}\", \"made_hash_crust\": \"{:016x}\", \"provinces\": {{{}}}, \"flows\": {{{}}}, \"caves\": {{\"balls\": {}, \"in_limestone\": {}, \"in_basalt\": {}, \"depth_m\": [{:.1}, {:.1}], \"open_to_the_sky\": {}, \"kinds\": {{{}}}}}, \"heat_c_under_the_spawn\": [{:.1}, {:.1}], \"pressure_mpa_under_the_spawn\": [{:.3}, {:.3}], \"deep\": {{{}}}, \"minerals\": {{{}}}}}",
        map.seed, map.n, b.gen, b.spawn.x, b.spawn.y, b.spawn.z, made_hash(&b.caves, &b.ores), made_hash(&b.caves, &crust), provinces.join(", "), flows(b).join(", "), b.caves.len(), in_lime, b.caves.len() - in_lime, dmin, dmax, open, kinds(b).join(", "), heat(ground), heat(0.0), pressure(ground), pressure(0.0), deep.join(", "), minerals(w).join(", ")
    )
}

/// Each kind of `data/caves.toml` as made (only a planet with the mantle has them): how many, their open cells under
/// the ground, their length and openings to the sky, and the nearest opening to the spawn: its distance over the
/// ground, its direction (`run=stand at=`) and how deep the cave's floor (a fissure's middle) lies under it there.
fn kinds(b: &engine::world::Body) -> Vec<String> {
    use engine::caves::{cells, systems, CAVES};
    let t = std::time::Instant::now();
    let sys = b.baked_seed().filter(|_| b.mantle > 0).map_or(Vec::new(), |s| systems(b, s));
    // what making them costs a planet's load (`make` does the same once)
    let made = format!("\"made_s\": {:.3}", t.elapsed().as_secs_f64());
    std::iter::once(made).chain(CAVES
        .kinds
        .iter()
        .enumerate()
        .filter(|(_, kind)| (kind.generations[0]..=kind.generations[1]).contains(&b.gen))
        .map(|(k, kind)| {
            let of: Vec<&engine::caves::System> = sys.iter().filter(|s| s.kind as usize == k).collect();
            let open: std::collections::HashSet<(u8, i32, i32, i32)> = of.iter().flat_map(|s| s.balls.iter().flat_map(|c| cells(b, c).into_iter().map(move |(i, j, z)| (c.face, i, j, z)))).collect();
            let mut lengths: Vec<f64> = of.iter().map(|s| s.axis.windows(2).map(|p| (p[1] - p[0]).truncate().length()).sum::<f64>()).collect();
            lengths.sort_by(f64::total_cmp);
            let length: f64 = lengths.iter().sum();
            let median = lengths.get(lengths.len() / 2).copied().unwrap_or(0.0);
            let away = |s: &engine::caves::System, o: &(usize, usize)| {
                let p = s.axis[(o.0 + o.1) / 2];
                b.radius * b.dir(s.face as usize, p.x, p.y).angle_between(b.spawn)
            };
            let near = of.iter().flat_map(|s| s.openings.iter().map(move |o| (s, o))).min_by(|x, y| away(x.0, x.1).total_cmp(&away(y.0, y.1)));
            let near = near.map_or("null".to_string(), |(s, o)| {
                let k = (o.0 + o.1) / 2;
                let (f, p) = (s.face as usize, s.axis[k]);
                let d = b.dir(f, p.x, p.y);
                // two ready stand lines: from the ground a few metres before the opening, looking at it; and inside,
                // a tube 15 m on from it looking back up the tube to its light, a fissure at its bottom looking up
                let look = |q: glam::DVec3, at: glam::DVec3, depth: f64, pitch: f64| {
                    let up = b.dir(f, q.x, q.y);
                    let r = if up.y.abs() > 0.9 { glam::DVec3::Z } else { glam::DVec3::Y };
                    let ahead = (r - up * r.dot(up)).normalize();
                    let to = b.dir(f, at.x, at.y) - up;
                    let yaw = to.dot(up.cross(ahead)).atan2(to.dot(ahead)).to_degrees();
                    format!("at={:.6},{:.6},{:.6} depth={depth:.1} yaw={yaw:.0} pitch={pitch:.0}", up.x, up.y, up.z)
                };
                let tube = engine::caves::CAVES.kinds[s.kind as usize].shape == engine::caves::Shape::Tube;
                let before = s.axis[o.0.saturating_sub(if tube { 5 } else { 10 })];
                let outside = look(before, p, 0.0, -45.0);
                // inside: the middle of the open column nearest q, feet on its lowest open cell
                let inside = |q: glam::DVec3, toward: glam::DVec3, pitch: f64| {
                    let (i0, j0) = (q.x.floor() as i32, q.y.floor() as i32);
                    let best = (-1..=1).flat_map(|a| (-1..=1).map(move |c| (i0 + a, j0 + c))).filter_map(|(i, j)| s.balls.iter().filter_map(|c| c.span(i, j)).map(|x| x.0).min().map(|lo| (i, j, lo))).min_by_key(|&(i, j, _)| (i - i0).abs() + (j - j0).abs());
                    best.map_or("none".to_string(), |(i, j, lo)| {
                        let c = glam::DVec3::new(i as f64 + 0.5, j as f64 + 0.5, 0.0);
                        look(c, toward, b.ground_r(f, c.x, c.y) - b.layer_r(lo as f64), pitch)
                    })
                };
                // a tube's station farthest along its axis from every opening, looking on along it
                let gap = |k: usize| s.openings.iter().map(|o| if k < o.0 { o.0 - k } else { k.saturating_sub(o.1) }).min().unwrap_or(usize::MAX);
                let far = (0..s.axis.len().saturating_sub(6)).max_by_key(|&k| gap(k)).filter(|_| tube);
                let closed = far.map_or("none".to_string(), |k| format!("{} (axis point {k} of {}, {} from an opening)", inside(s.axis[k], s.axis[k + 5], 0.0), s.axis.len(), gap(k)));
                let inside = if tube { inside(s.axis[(o.1 + 15).min(s.axis.len() - 1)], p, 10.0) } else { inside(p, s.axis[(k + 4).min(s.axis.len() - 1)], 75.0) };
                format!("{{\"m\": {:.0}, \"at\": [{:.6}, {:.6}, {:.6}], \"depth_m\": {:.1}, \"outside\": \"{outside}\", \"inside\": \"{inside}\", \"closed\": \"{closed}\"}}", away(s, o), d.x, d.y, d.z, b.ground_r(f, p.x, p.y) - p.z)
            });
            format!("\"{}\": {{\"caves\": {}, \"open_cells\": {}, \"length_m\": {length:.0}, \"median_m\": {median:.0}, \"openings\": {}, \"nearest_opening\": {near}}}", kind.name, of.len(), open.len(), of.iter().map(|s| s.openings.len()).sum::<usize>())
        }))
        .collect()
}

/// Each province with a lava flow (`data/rocks.toml` `flow`): the share of its land (columns over the sea, every
/// 8th each way) that lies under one (none before generation 2).
fn flows(b: &engine::world::Body) -> Vec<String> {
    let sea = b.radius - engine::world::SEA_DEPTH;
    let (mut land, mut under) = ([0u64; NAMES.len()], [0u64; NAMES.len()]);
    for f in 0..6u8 {
        for i in (4..b.n).step_by(8) {
            for j in (4..b.n).step_by(8) {
                let r = b.col_rock(f, i, j);
                let p = r.province as usize;
                if engine::rock::ROCKS.provinces.get(p).is_some_and(|p| p.flow.is_some()) && b.col_height(f, i, j) > sea {
                    land[p] += 1;
                    under[p] += (r.flow > 0.0) as u64;
                }
            }
        }
    }
    (0..NAMES.len()).filter(|&p| land[p] > 0).map(|p| format!("\"{}\": {:.3}", NAMES[p], under[p] as f64 / land[p] as f64)).collect()
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
        // a kind's shape too (a plain ball's hash is the one from before the kinds)
        if c.kind != engine::world::Cave::BALL {
            [c.kind as u64, c.h.to_bits(), c.floor.to_bits()].into_iter().for_each(&mut put);
        }
    }
    for o in ores {
        [o.kind as u64, o.face as u64, o.at.x.to_bits(), o.at.y.to_bits(), o.at.z.to_bits(), o.a.to_bits(), o.b.to_bits(), o.h.to_bits(), o.angle.to_bits()].into_iter().for_each(&mut put);
    }
    h
}
