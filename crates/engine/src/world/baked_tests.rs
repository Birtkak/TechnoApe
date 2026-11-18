use super::*;
use std::sync::LazyLock;

/// One small baked planet for these tests (a few seconds).
static MAP: LazyLock<Arc<Map>> = LazyLock::new(|| Arc::new(Map::bake(5, 512)));

fn world() -> World {
    World::with_map(512, 32, 5, Some(MAP.clone()))
}

/// The warp (ADR 0131) moves the block grid over the land, not the land: the bake is read by direction, so a
/// column's ground on the warped grid is the unwarped world's ground along that column's direction.
#[test]
fn the_warp_moves_the_blocks_not_the_land() {
    let (w0, w1) = (world(), World::with_maps_warp(512, 32, 5, Some(MAP.clone()), None, 1.0));
    let (b0, b1) = (&w0.bodies[0], &w1.bodies[0]);
    let n = b1.n;
    let mut worst: f64 = 0.0;
    for f in 0..6u8 {
        for (i, j) in [(3, 3), (40, 7), (n / 2, n / 2), (n - 5, n / 3), (n - 2, n - 2), (n / 4, n - 30), (120, 300)] {
            let d = b1.dir(f as usize, i as f64 + 0.5, j as f64 + 0.5);
            worst = worst.max((b1.col_height(f, i, j) - b0.height(d)).abs());
            worst = worst.max((b1.height(d) - b0.height(d)).abs());
            // and the tile builder's, which works out the warped directions itself
            let t = b1.tile(f, 0, i / CS, j / CS);
            worst = worst.max((t.h[(i % CS + 1) as usize + (j % CS + 1) as usize * P] - b0.height(d)).abs());
        }
    }
    assert!(worst < 1e-6, "the ground moved by {worst} m");
}

/// A scouting tool, not a check: on the 4 km planet (seed 1337), the large tree of each species nearest the
/// spawn at each size and a `run=stand` that looks at it from 10 to 15 m, for ADR 0092's shots. The bake is read from
/// `TECHNOAPE_BAKE` (a path), else `saves/bakes` under the repository.
/// `cargo test --profile play -p engine tree_spots -- --ignored --nocapture`.
#[test]
#[ignore]
fn tree_spots() {
    let path = std::env::var("TECHNOAPE_BAKE").map(std::path::PathBuf::from).unwrap_or(std::path::Path::new("../..").join(crate::bake::file(1337, 6144)));
    let map = Arc::new(Map::from_bytes(&std::fs::read(&path).unwrap()).unwrap());
    let w = World::with_map(map.n, 32, map.seed, Some(map.clone()));
    let b = &w.bodies[0];
    // every spot by species and size, nearest the spawn first
    let mut all: Vec<Vec<(f64, usize, Foot)>> = vec![Vec::new(); ROWS.plants.len() * 3];
    // the forests: a spot nearest the spawn of each whose ground 30 m round is all that biome
    use crate::bake::life::{COLD_FOREST, RAIN_FOREST, TEMPERATE_FOREST};
    let kinds = [(TEMPERATE_FOREST, "temperate"), (COLD_FOREST, "cold"), (RAIN_FOREST, "rain")];
    let mut forest: [Option<(f64, usize, Foot)>; 3] = [None; 3];
    for f in 0..6 {
        for cx in 0..b.n / SPACING {
            for cy in 0..b.n / SPACING {
                let Some(t) = b.candidate(f, cx, cy) else { continue };
                let p = &ROWS.plants[t.plant];
                if p.radius == 0 || p.shape == crate::biomes::Shape::Bush {
                    continue;
                }
                let d = b.dir(f, t.i as f64 + 0.5, t.j as f64 + 0.5);
                let near = d.dot(b.spawn);
                all[t.plant * 3 + p.size(t.h) as usize].push((near, f, t));
                let here = b.biome_at(f, t.i as f64 + 0.5, t.j as f64 + 0.5, d);
                if let Some(q) = kinds.iter().position(|k| Some(k.0) == here) {
                    let inside = || {
                        (0..8).all(|a| {
                            let (si, ci) = (a as f64 * std::f64::consts::FRAC_PI_4).sin_cos();
                            let (x, y) = (t.i as f64 + 0.5 + 30.0 * ci, t.j as f64 + 0.5 + 30.0 * si);
                            b.biome_at(f, x, y, b.dir(f, x, y)) == here
                        })
                    };
                    if forest[q].is_none_or(|x| near > x.0) && inside() {
                        forest[q] = Some((near, f, t));
                    }
                }
            }
        }
    }
    let sun = w.sky.sun_dir(225.0);
    // the view from `dc` (a direction from the centre) at `eye` over the ground to `target`
    let aim = |dc: DVec3, eye: f64, target: DVec3| {
        let pos = b.centre + dc * (b.top(dc) + eye);
        let up = dc;
        let rr = if up.y.abs() > 0.9 { DVec3::Z } else { DVec3::Y };
        let ahead = (rr - up * rr.dot(up)).normalize();
        let fwd = (target - pos).normalize();
        let hz = fwd - up * fwd.dot(up);
        (pos, hz.dot(up.cross(ahead)).atan2(hz.dot(ahead)).to_degrees(), fwd.dot(up).asin().to_degrees())
    };
    for (k, list) in all.iter_mut().enumerate() {
        list.sort_by(|x, y| y.0.total_cmp(&x.0));
        let p = &ROWS.plants[k / 3];
        // the nearest of the first forty with a view from the sun's half that nothing stands in: no block
        // but air on the line from the eye to the tree's middle until its crown
        let found = list.iter().take(40).find_map(|&(near, f, t)| {
            let dt = b.dir(f, t.i as f64 + 0.5, t.j as f64 + 0.5);
            let r = b.layer_r(t.z as f64);
            let target = b.centre + dt * (r + t.h as f64 * 0.55);
            let (e1, e2) = (dt.any_orthonormal_vector(), dt.cross(dt.any_orthonormal_vector()));
            let mut ways: Vec<DVec3> = (0..16).map(|a| (a as f64 * std::f64::consts::PI / 8.0).sin_cos()).map(|(s, c)| (dt * r + (e1 * c + e2 * s) * (12.0 + t.h as f64 * 0.5)).normalize()).filter(|dc| (*dc - dt).dot(sun) > 0.0).collect();
            ways.sort_by(|x, y| y.dot(sun).total_cmp(&x.dot(sun)));
            ways.into_iter().find_map(|dc| {
                let (pos, yaw, pitch) = aim(dc, 1.7, target);
                let len = (target - pos).length() - p.crown(t.h) as f64 - 1.0;
                let clear = (0..(len * 2.0) as i32).all(|s| w.block(w.block_at(pos + (target - pos).normalize() * s as f64 * 0.5)) == 0);
                (clear && w.water_surface(pos).is_none()).then_some((dc, yaw, pitch, near, t))
            })
        });
        let Some((dc, yaw, pitch, near, t)) = found else { continue };
        let name = p.name.replace(' ', "-");
        println!("SHOT {name}-{} at={:.6},{:.6},{:.6} eye=1.7 yaw={yaw:.1} pitch={pitch:.1}  # h {} {:.0} m from the spawn", ["small", "medium", "large"][k % 3], dc.x, dc.y, dc.z, t.h, near.clamp(-1.0, 1.0).acos() * b.radius);
    }
    // each forest from among its trees looking ahead; the temperate one also from 40 m over them
    for (q, (_, kind)) in kinds.iter().enumerate() {
        let Some((_, f, t)) = forest[q] else { continue };
        let dt = b.dir(f, t.i as f64 + 0.5, t.j as f64 + 0.5);
        let r = b.layer_r(t.z as f64);
        let (e1, e2) = (dt.any_orthonormal_vector(), dt.cross(dt.any_orthonormal_vector()));
        // a spot near the tree whose eye is in the open and sees 10 m ahead between the trunks
        let clear = |from: DVec3, to: DVec3, n: i32| (0..n).all(|s| w.block(w.block_at(from + (to - from) * (s as f64 / n as f64))) == 0);
        let found = (2..20).flat_map(|m| (0..8).map(move |a| (m, a))).find_map(|(m, a)| {
            let (sa, ca) = (a as f64 * std::f64::consts::FRAC_PI_4).sin_cos();
            let dc = (dt * r + (e1 * ca + e2 * sa) * m as f64).normalize();
            let (pos, ..) = aim(dc, 1.7, dt);
            let ahead = (e1 * ca + e2 * sa) * 10.0;
            (clear(pos - dc * 0.6, pos + dc * 0.6, 3) && clear(pos, pos + ahead, 20) && w.water_surface(pos).is_none()).then_some((dc, b.centre + dt * r + (e1 * ca + e2 * sa) * (m as f64 + 30.0)))
        });
        let Some((dc, far)) = found else { continue };
        let (_, yaw, _) = aim(dc, 1.7, far);
        println!("SHOT forest-{kind} at={:.6},{:.6},{:.6} eye=1.7 yaw={yaw:.1} pitch=8", dc.x, dc.y, dc.z);
        if q == 0 {
            println!("SHOT forest-above at={:.6},{:.6},{:.6} eye=40 yaw={yaw:.1} pitch=-40", dc.x, dc.y, dc.z);
        }
    }
}

/// ADR 0092: what trees cost a forest chunk on one worker thread: the chunk's generation (cold, then with
/// its tiles cached) and its quads (fast and fancy leaves). The chunks are those over every candidate spot
/// in a forest biome, so the set is the same whatever grows there.
/// `cargo test --profile play -p engine tree_chunk_cost -- --ignored --nocapture`.
#[test]
#[ignore]
fn tree_chunk_cost() {
    let w = world();
    let b = &w.bodies[0];
    let forest = [crate::bake::life::COLD_FOREST, crate::bake::life::TEMPERATE_FOREST, crate::bake::life::RAIN_FOREST];
    let mut keys = std::collections::BTreeSet::new();
    for f in 0..6usize {
        for cx in 0..b.n / SPACING {
            for cy in 0..b.n / SPACING {
                let (i, j) = (cx * SPACING + SPACING / 2, cy * SPACING + SPACING / 2);
                let d = b.dir(f, i as f64 + 0.5, j as f64 + 0.5);
                if b.biome_at(f, i as f64 + 0.5, j as f64 + 0.5, d).is_some_and(|bi| forest.contains(&bi)) {
                    let z = b.first_air(b.col_height(f as u8, i, j)) + 4;
                    keys.insert((f as u8, i.div_euclid(CS), j.div_euclid(CS), z.div_euclid(CS)));
                }
            }
        }
    }
    let keys: Vec<Key> = keys.into_iter().map(|(face, x, y, z)| Key { body: 0, face, level: 0, pos: IVec3::new(x, y, z) }).collect();
    let run = || {
        let t0 = std::time::Instant::now();
        let cells: Vec<_> = keys.iter().filter_map(|k| match generate(b, *k, &[]) {
            Gen::Cells(c) => Some(c),
            _ => None,
        }).collect();
        (t0.elapsed().as_secs_f64() * 1e6 / keys.len() as f64, cells)
    };
    let (cold, _) = run();
    let (warm, cells) = run();
    let t0 = std::time::Instant::now();
    let fast: usize = cells.iter().map(|c| crate::mesh::mesh(c, 1).quads.len()).sum();
    let mesh_us = t0.elapsed().as_secs_f64() * 1e6 / cells.len() as f64;
    let fancy: usize = cells.iter().map(|c| crate::mesh::mesh_cut(c, 1, crate::mesh::Cut::Full, true).quads.len()).sum();
    let n = cells.len() as f64;
    println!("{{\"chunks\":{},\"gen_us_cold\":{cold:.1},\"gen_us_warm\":{warm:.1},\"mesh_us_fast\":{mesh_us:.1},\"quads_fast\":{:.0},\"quads_fancy\":{:.0}}}", cells.len(), fast as f64 / n, fancy as f64 / n);
}

/// Beds (ADR 0086): no column under the sea, a lake or a river has grass, snow or ice on top; the deep
/// sea is not sand and a shallow lake not clay; each kind of water is found.
#[test]
fn the_beds_under_water_are_sand_clay_gravel_stone_or_dirt() {
    // a planet big enough for rivers (the 512 one has none), baked in seconds
    let w = World::with_map(1536, 32, 5, Some(Arc::new(Map::bake(5, 1536))));
    let b = &w.bodies[0];
    let soil = |m: u16| {
        let n = &BLOCKS.get(m).unwrap().name;
        n.contains("grass") || ["snow", "ice"].contains(&n.as_str())
    };
    let (mut seen, mut bad) = ([0usize; 3], Vec::new());
    let mut tally = std::collections::BTreeMap::new();
    let mut all = std::collections::BTreeMap::new();
    for f in 0..6u8 {
        for (i, j) in (0..b.n).step_by(2).flat_map(|i| (0..b.n).step_by(2).map(move |j| (i, j))) {
            let Some((top, reach, river)) = b.water_at(f, i, j) else { continue };
            let (ci, cj) = (i as f64 + 0.5, j as f64 + 0.5);
            let h = b.height_at(f as usize, ci, cj, b.dir(f as usize, ci, cj), 0);
            let depth = b.layer_r(top) - h;
            if depth <= 0.0 {
                continue;
            }
            let kind = if river.is_some() { 2 } else if reach == crate::liquid::SEA { 0 } else { 1 };
            seen[kind] += 1;
            let g = b.col_ground(f, i, j, Some(h));
            let name = &BLOCKS.get(g[0]).unwrap().name;
            *all.entry(format!("{} {name}", ["sea", "lake", "river"][kind])).or_insert(0usize) += 1;
            let wrong = soil(g[0]) || soil(g[1])
                || (kind == 0 && depth > 12.0 && name == "sand")
                || (kind == 1 && depth < 0.75 && name == "clay")
                || (kind == 0 && name == "dirt");
            if wrong {
                *tally.entry(format!("{} {name}", ["sea", "lake", "river"][kind])).or_insert(0usize) += 1;
                if bad.len() < 6 {
                    bad.push(format!("{} at {f} {i} {j}, {depth:.1} m under, top {name}", ["sea", "lake", "river"][kind]));
                }
            }
        }
    }
    assert!(seen.iter().all(|&n| n > 20), "sea, lake, river columns seen: {seen:?}");
    eprintln!("beds (kind, top: columns): {all:?}");
    assert!(tally.is_empty(), "wrong beds (kind, top: columns) {tally:?} of {seen:?}, e.g. {bad:?}");
}

/// `air_over_ground` is air with ground (not air) under it, on the baked map and the old noise land, over
/// land: the terrain's height falls inside its top block, or over a lower neighbour's on a slope.
#[test]
fn the_air_over_the_ground_stands_on_it() {
    for (name, w) in [("baked", world()), ("noise", World::new(512, 32, 5))] {
        let pl = &w.bodies[0];
        let land: Vec<DVec3> = lattice(2000).filter(|d| pl.sea.is_none_or(|s| pl.height(*d) > pl.layer_r(s) + 1.0)).take(300).collect();
        assert!(land.len() >= 100, "{name}: {} land spots", land.len());
        let bad: Vec<String> = land.iter().filter_map(|d| {
            let c = w.air_over_ground(0, *d);
            let under = w.block(Block { pos: c.pos - IVec3::Z, ..c });
            (w.block(c) != AIR || under == AIR).then(|| format!("cell layer {} is {}, under it {}, first air of the column {}", c.pos.z, w.block(c), under, pl.first_air(w.column_height(c))))
        }).collect();
        assert!(bad.is_empty(), "{name}: {} of {} spots not on the ground, e.g. {:?}", bad.len(), land.len(), &bad[..bad.len().min(3)]);
    }
}

/// The 192 m crust (ADR 0063): a big body's layers keep their radii and move up 128 in index; a small
/// one keeps its old floor.
#[test]
fn the_deep_ground_lies_under_the_old_layers() {
    for (n, big) in [(1536, true), (6144, true), (12288, true), (512, false), (32, false)] {
        let (b, deep) = (Body::new(DVec3::ZERO, n, 1), deep_layers(n));
        assert_eq!(deep > 0, big, "planet {n}: {deep} deep layers");
        let old_r0 = b.radius - CRUST.min(b.radius * 0.5);
        for l in [0.0, 10.5, 64.0, 200.0] {
            let (now, was) = (b.layer_r(l + deep as f64), old_r0 * (b.lnq * l).exp());
            assert!((now - was).abs() < 1e-6, "planet {n}, layer {l}: {now} against {was}");
        }
        if big {
            let crust = b.radius - b.r0;
            assert!((DEEP..DEEP + 1.0).contains(&crust), "planet {n}: {crust} m of crust");
        }
    }
}

/// A cave is one answer (ADR 0062): the block physics sees (`generated_block`, `phys::blocked`), the chunk
/// the mesher draws (`generate`) and the water's gaps (`Solid::gaps`) agree cell by cell around a closed
/// cave and one open to the sky, and a player fits in the closed one.
#[test]
fn the_mesher_physics_and_water_see_the_same_cave() {
    let mut w = world();
    let top = w.air_over_ground(0, w.bodies[0].spawn);
    let (f, i0, j0, z0) = (top.face, top.pos.x, top.pos.y, top.pos.z);
    // the closed one on a chunk corner, so it reaches into four chunks and their padding
    let corner = |v: i32| ((v + CS / 2).div_euclid(CS) * CS) as f64;
    let shut = Cave::ball(f, DVec3::new(corner(i0), corner(j0), ((z0 - 8).div_euclid(CS) * CS) as f64), 3.5);
    let open = Cave::ball(f, DVec3::new(i0 as f64 + 20.5, j0 as f64 + 0.5, w.air_over_ground(0, w.bodies[0].spawn).pos.z as f64 - 1.0), 4.0);
    std::sync::Arc::get_mut(&mut w.bodies).unwrap()[0].set_caves(vec![shut, open]);
    let b = &w.bodies[0];
    let mut hollow = 0;
    for c in [shut, open] {
        for (i, j) in (-6..=6).flat_map(|a| (-6..=6).map(move |d| (c.at.x as i32 + a, c.at.y as i32 + d))) {
            let gaps = w.ground().gaps(Col::new(0, f, i, j));
            let floor = b.first_air(b.col_height(f, i, j));
            for z in c.at.z as i32 - 6..=c.at.z as i32 + 6 {
                let phys = w.generated_block(Block { body: 0, face: f, pos: IVec3::new(i, j, z) });
                let k = Key { body: 0, face: f, level: 0, pos: IVec3::new(i, j, z).div_euclid(IVec3::splat(CS)) };
                let mesh = match generate(b, k, &[]) {
                    Gen::Cells(cells) => cells[pidx((i - k.pos.x * CS + 1) as usize, (j - k.pos.y * CS + 1) as usize, (z - k.pos.z * CS + 1) as usize)],
                    Gen::Solid => u16::MAX,
                    Gen::Empty => AIR,
                };
                assert!(mesh == phys || (mesh == u16::MAX && phys != AIR), "({i}, {j}, {z}): mesh {mesh}, physics {phys}");
                let wet = gaps.iter().any(|&(lo, hi)| (lo..hi).contains(&z));
                assert_eq!(wet, !BLOCKS.solid(phys), "({i}, {j}, {z}): water {gaps:?}, physics {phys}");
                hollow += (phys == AIR && z < floor) as usize;
            }
        }
    }
    assert!(hollow > 150, "{hollow} cave cells under the ground");
    let at = |l: f64| b.centre + b.dir(f as usize, shut.at.x, shut.at.y) * b.layer_r(l);
    assert!(!crate::phys::blocked(&w, at(shut.at.z - 1.0)), "a player fits in the closed cave");
    assert!(crate::phys::blocked(&w, at(shut.at.z - 6.0)), "and not in the rock under it");
}

/// Ore is one answer too (ADR 0063): physics, the mesher's chunk and the ledger's count agree cell by cell
/// on a deep lump across a chunk corner with a cave through it (its halo too, ADR 0095), and on a stream-bed gravel at the ground,
/// which shows at the top.
#[test]
fn the_mesher_physics_and_the_count_see_the_same_ore() {
    use crate::ore::{OreBody, MINERALS};
    let mut w = world();
    let top = w.air_over_ground(0, w.bodies[0].spawn);
    let (f, i0, j0, z0) = (top.face, top.pos.x, top.pos.y, top.pos.z);
    let kind = |n: &str| MINERALS.kinds.iter().position(|k| k.name == n).unwrap() as u16;
    let corner = |v: i32| ((v + CS / 2).div_euclid(CS) * CS) as f64;
    let lump = OreBody { kind: kind("deep copper"), face: f, at: DVec3::new(corner(i0), corner(j0), ((z0 - 10).div_euclid(CS) * CS) as f64), a: 4.5, b: 4.5, h: 3.5, angle: 0.3 };
    let gravel = OreBody { kind: kind("copper gravel"), face: f, at: DVec3::new(i0 as f64 + 20.5, j0 as f64 + 0.5, 0.75), a: 6.0, b: 2.0, h: 0.75, angle: 1.0 };
    let hole = Cave::ball(f, lump.at + DVec3::new(2.0, 0.0, 0.0), 2.5);
    let m = std::sync::Arc::get_mut(&mut w.bodies).unwrap();
    m[0].set_caves(vec![hole]);
    m[0].set_ores(vec![lump, gravel]);
    let b = &w.bodies[0];
    let held = crate::ore::count(b);
    let mut halos = 0;
    for (k, o) in [lump, gravel].iter().enumerate() {
        let ore = MINERALS.kinds[o.kind as usize].block;
        let (mut seen, mut ingots, mut at_top) = (0, 0, 0);
        for (i, j) in (-8..=8).flat_map(|a| (-8..=8).map(move |d| (o.at.x as i32 + a, o.at.y as i32 + d))) {
            let floor = b.first_air(b.col_height(f, i, j));
            let zs = if k == 0 { o.at.z as i32 - 6..=o.at.z as i32 + 6 } else { floor - 4..=floor + 1 };
            for z in zs {
                let phys = w.generated_block(Block { body: 0, face: f, pos: IVec3::new(i, j, z) });
                let key = Key { body: 0, face: f, level: 0, pos: IVec3::new(i, j, z).div_euclid(IVec3::splat(CS)) };
                let mesh = match generate(b, key, &[]) {
                    Gen::Cells(cells) => cells[pidx((i - key.pos.x * CS + 1) as usize, (j - key.pos.y * CS + 1) as usize, (z - key.pos.z * CS + 1) as usize)],
                    Gen::Solid => u16::MAX,
                    Gen::Empty => AIR,
                };
                let halo = MINERALS.kinds[o.kind as usize].halo.contains(&phys);
                assert!(mesh == phys || (mesh == u16::MAX && phys != AIR && phys != ore && !halo), "({i}, {j}, {z}): mesh {mesh}, physics {phys}");
                halos += halo as u32;
                if phys == ore {
                    seen += 1;
                    ingots += o.grade(i, j);
                    at_top += (z == floor - 1) as u32;
                }
            }
        }
        assert!(seen > 20, "body {k}: {seen} ore cells");
        assert_eq!((held[k].blocks, held[k].ingots), (seen, ingots), "body {k}: the count against what physics sees");
        assert!(k == 0 || at_top > 10, "the gravel shows at the ground: {at_top} cells");
    }
    assert_eq!(w.generated_block(Block { body: 0, face: f, pos: IVec3::new(hole.at.x as i32, hole.at.y as i32, hole.at.z as i32) }), AIR, "the cave goes through the ore");
    assert!(halos > 100, "{halos} halo cells, mesher and physics alike (ADR 0095)");
}

/// The halo (ADR 0095): for every kind with one, a line walked from the host rock toward a body's core, along
/// each of its axes both ways, meets the halo's grades outermost first and then ore; the mesher's chunk sees
/// every cell as physics does; the halo eats no ore (the count is the ore physics sees) and is never ore to
/// the ledger.
#[test]
fn a_line_toward_a_body_passes_rising_halo_grades_then_ore() {
    use crate::ore::{OreBody, MINERALS};
    let mut w = world();
    let top = w.air_over_ground(0, w.bodies[0].spawn);
    let f = top.face;
    let kinds: Vec<usize> = (0..MINERALS.kinds.len()).filter(|&k| !MINERALS.kinds[k].halo.is_empty()).collect();
    assert!(kinds.len() >= 6, "{} kinds with a halo", kinds.len());
    for &k in &kinds {
        let kind = &MINERALS.kinds[k];
        let (a, bw, h) = kind.shape.axes(kind.blocks as f64);
        // the big bodies cut down to fit the small planet; the halo is the kind's own
        let (a, bw, h) = (a.min(16.0), bw.min(16.0), h.min(10.0));
        let t = kind.halo_m;
        let o = OreBody { kind: k as u16, face: f, at: DVec3::new(top.pos.x as f64 + 0.5, top.pos.y as f64 + 0.5, top.pos.z as f64 - h - t - 12.0), a, b: bw, h, angle: 0.4 };
        let m = Arc::get_mut(&mut w.bodies).unwrap();
        m[0].set_caves(Vec::new());
        m[0].set_ores(vec![o]);
        let b = &w.bodies[0];
        let top_level = kind.halo.len() + 1;
        let level = |m: u16| if m == kind.block { top_level } else { kind.halo.iter().position(|&x| x == m).map_or(0, |g| g + 1) };
        let mut chunk: Option<(Key, Gen)> = None;
        let mut check = |p: IVec3| -> u16 {
            let blk = Block { body: 0, face: f, pos: p };
            let phys = w.generated_block(blk);
            let key = Key { body: 0, face: f, level: 0, pos: p.div_euclid(IVec3::splat(CS)) };
            if chunk.as_ref().is_none_or(|c| c.0 != key) {
                chunk = Some((key, generate(b, key, &[])));
            }
            let mesh = match &chunk.as_ref().unwrap().1 {
                Gen::Cells(cells) => cells[pidx((p.x - key.pos.x * CS + 1) as usize, (p.y - key.pos.y * CS + 1) as usize, (p.z - key.pos.z * CS + 1) as usize)],
                Gen::Solid => u16::MAX,
                Gen::Empty => AIR,
            };
            // a chunk wholly solid draws nothing, whatever its cells are; a dig makes it cells again
            assert!(mesh == phys || (mesh == u16::MAX && phys != AIR), "{}: {p}: mesh {mesh}, physics {phys}", kind.name);
            if level(phys) > 0 && level(phys) < top_level {
                assert_eq!(w.ore_cell(blk), None, "{}: halo is no ore", kind.name);
            }
            phys
        };
        let (c, s) = (o.angle.cos(), o.angle.sin());
        for (du, dv, dw, ext) in [(1.0, 0.0, 0.0, a), (-1.0, 0.0, 0.0, a), (0.0, 1.0, 0.0, bw), (0.0, -1.0, 0.0, bw), (0.0, 0.0, 1.0, h), (0.0, 0.0, -1.0, h)] {
            // from 1.5 halos out to the middle, a quarter metre a step; the levels met, each once in a row
            let mut seen: Vec<usize> = Vec::new();
            for n in (0..=((ext + 1.5 * t) * 4.0) as i32).rev() {
                let r = n as f64 * 0.25;
                let (u, v) = (du * r, dv * r);
                let p = IVec3::new((o.at.x + u * c - v * s).floor() as i32, (o.at.y + u * s + v * c).floor() as i32, (o.at.z + dw * r).floor() as i32);
                let l = level(check(p));
                if seen.last() != Some(&l) {
                    seen.push(l);
                }
            }
            let mut first: Vec<usize> = Vec::new();
            for &l in &seen {
                if !first.contains(&l) {
                    first.push(l);
                }
            }
            assert_eq!(first, (0..=top_level).collect::<Vec<_>>(), "{} along ({du}, {dv}, {dw}): levels met {seen:?}", kind.name);
            assert_eq!(seen.last(), Some(&top_level), "{}: the line ends in ore", kind.name);
        }
        // the whole box: ore as the count has it, halo only within its reach
        let (mut ore, mut halo) = (0u32, 0u32);
        let rr = a.max(bw) + 1.5 * t;
        for (i, j) in (-rr as i32..=rr as i32).flat_map(|x| (-rr as i32..=rr as i32).map(move |y| (x, y))) {
            for z in (o.at.z - h - 1.5 * t) as i32..=(o.at.z + h + 1.5 * t) as i32 {
                let p = IVec3::new(o.at.x as i32 + i, o.at.y as i32 + j, z);
                let l = level(w.generated_block(Block { body: 0, face: f, pos: p }));
                ore += (l == top_level) as u32;
                if l > 0 && l < top_level {
                    halo += 1;
                    assert!(o.within(p.x, p.y, p.z, t * (1.0 + crate::ore::FRAY)) && !o.within(p.x, p.y, p.z, 0.0), "{}: halo at {p} past its reach", kind.name);
                }
            }
        }
        assert_eq!(ore, crate::ore::count(b)[0].blocks, "{}: the halo took no ore", kind.name);
        assert!(halo > ore / 2, "{}: {halo} halo cells round {ore} ore", kind.name);
        assert_eq!(crate::ore::count_halo(b)[0].iter().sum::<u32>(), halo, "{}: the report's halo count", kind.name);
    }
}

/// The ledger (ADR 0063) opens with the count, goes to zero when a body's cells are dug out, and up again by
/// one for an ore block put back in its own cell or an edit undone; ore put elsewhere counts nothing; the
/// recount from the edits alone agrees all along.
#[test]
fn digging_a_body_out_empties_its_ledger() {
    use crate::ore::{OreBody, MINERALS};
    let mut w = world();
    let top = w.air_over_ground(0, w.bodies[0].spawn);
    let kind = MINERALS.kinds.iter().position(|k| k.name == "lead pocket").unwrap() as u16;
    let lump = OreBody { kind, face: top.face, at: DVec3::new(top.pos.x as f64 + 0.5, top.pos.y as f64 + 0.5, top.pos.z as f64 - 12.0), a: 3.5, b: 3.5, h: 2.5, angle: 0.0 };
    std::sync::Arc::get_mut(&mut w.bodies).unwrap()[0].set_ores(vec![lump]);
    w.open_ledger();
    let open = w.ore_left.clone().unwrap();
    let cells = crate::ore::cells(&w.bodies[0], 0);
    assert!(open[0] > 50 && open[0] == cells.len() as i64, "{open:?} against {} cells", cells.len());
    let ore = w.bodies[0].ore_block(0);
    let at = |p: IVec3| Block { body: 0, face: top.face, pos: p };
    for &p in &cells {
        w.set_block(at(p), AIR);
    }
    assert_eq!((w.ore_left.as_ref().unwrap()[0], w.ore_recount(&open)[0]), (0, 0), "dug out");
    w.set_block(at(cells[0]), ore);
    w.put_edit(at(cells[1]), None);
    w.set_block(at(top.pos + IVec3::Z), ore);
    assert_eq!((w.ore_left.as_ref().unwrap()[0], w.ore_recount(&open)[0]), (2, 2), "one put back, one undone, one elsewhere");
}

/// ADR 0092 (the overseer: a closed canopy left the forest floor black at noon): a forest has clearings,
/// patches of four by four spots where no tree grows, about one patch in eight.
#[test]
fn forests_have_clearings() {
    let w = world();
    let b = &w.bodies[0];
    let forest = crate::bake::life::TEMPERATE_FOREST;
    let (mut patches, mut clear) = (0, 0);
    for f in 0..6 {
        for px in 0..b.n / SPACING / 4 {
            for py in 0..b.n / SPACING / 4 {
                let spots: Vec<(i32, i32)> = (0..4).flat_map(|a| (0..4).map(move |c| (px * 4 + a, py * 4 + c))).collect();
                let all = spots.iter().all(|&(cx, cy)| {
                    let (i, j) = (cx * SPACING + SPACING / 2, cy * SPACING + SPACING / 2);
                    b.biome_at(f, i as f64, j as f64, b.dir(f, i as f64, j as f64)) == Some(forest)
                });
                if all {
                    patches += 1;
                    clear += spots.iter().all(|&(cx, cy)| b.candidate(f, cx, cy).is_none_or(|t| ROWS.plants[t.plant].trunk == 0)) as usize;
                }
            }
        }
    }
    assert!(patches >= 40 && clear * 12 >= patches && clear * 4 <= patches, "{clear} clearings in {patches} patches of temperate forest");
}

/// ADR 0092: on a baked planet every tree of a biome's row grows in it where that biome is common, palms
/// only where the year is warm, and two looks at a spot give the same tree.
#[test]
fn every_species_grows_in_its_biomes() {
    let w = world();
    let b = &w.bodies[0];
    let mut spots = vec![0usize; ROWS.biomes.len()];
    let mut found = std::collections::BTreeSet::new();
    for f in 0..6 {
        for cx in 0..b.n / SPACING {
            for cy in 0..b.n / SPACING {
                let (i, j) = (cx * SPACING + SPACING / 2, cy * SPACING + SPACING / 2);
                if let Some(bi) = b.biome_at(f, i as f64, j as f64, b.dir(f, i as f64, j as f64)) {
                    spots[bi as usize] += 1;
                }
                let Some(foot) = b.candidate(f, cx, cy) else { continue };
                assert_eq!(Some(foot), b.candidate(f, cx, cy));
                let bi = b.biome_at(f, foot.i as f64 + 0.5, foot.j as f64 + 0.5, b.dir(f, foot.i as f64 + 0.5, foot.j as f64 + 0.5)).unwrap();
                let p = &ROWS.plants[foot.plant];
                if let Some(warm) = p.warm {
                    let t = b.year_air(f, foot.i, foot.j).unwrap();
                    assert!(t >= warm, "a {} at {t:.1} C", p.name);
                }
                found.insert((bi, foot.plant));
            }
        }
    }
    let mut checked = 0;
    for (bi, row) in ROWS.biomes.iter().enumerate() {
        for &(p, share) in &row.plants {
            // enough spots that the share would find it ten times over; palms need warm ones too
            if (spots[bi] as f32) * share >= 10.0 && ROWS.plants[p].warm.is_none() {
                assert!(found.contains(&(bi as u8, p)), "no {} in {} ({} spots)", ROWS.plants[p].name, row.name, spots[bi]);
                checked += 1;
            }
        }
    }
    assert!(checked >= 10, "{checked} pairs checked");
    let palm = ROWS.plants.iter().position(|p| p.name == "palm").unwrap();
    assert!(found.iter().any(|&(_, p)| p == palm), "palms somewhere warm: rain forest {} spots, beach {}", spots[crate::bake::life::RAIN_FOREST as usize], spots[crate::bake::life::BEACH as usize]);
}

/// Far chunks show the forest (ADR 0100): a coarse column whose footprint holds a tree's trunk mostly stands
/// at a canopy over its ground, leaves on top; one with no tree within reach is bare ground.
#[test]
fn far_tiles_carry_the_canopy() {
    let w = world();
    let b = &w.bodies[0];
    for level in 1..=4u8 {
        let (mut forest, mut raised, mut bare) = (0, 0, 0);
        // the footprints of a dozen trees' tiles, over every face
        let mut tiles: Vec<(usize, i32, i32)> = (0..6).flat_map(|f| (0..b.n / SPACING).step_by(7).flat_map(move |cx| (0..b.n / SPACING).step_by(5).map(move |cy| (f, cx, cy)))).filter_map(|(f, cx, cy)| b.candidate(f, cx, cy).filter(|t| ROWS.plants[t.plant].trunk != 0).map(|t| (f, (t.i >> level) / CS, (t.j >> level) / CS))).collect();
        tiles.dedup();
        for &(face, x, y) in tiles.iter().step_by((tiles.len() / 12).max(1)) {
            let t = b.tile(face as u8, level, x, y);
            for (u, v) in (1..P - 1).flat_map(|u| (1..P - 1).map(move |v| (u, v))) {
                let (fi, fj) = (x * CS + u as i32 - 1, y * CS + v as i32 - 1);
                let (pi, pj) = ((fi as f64 + 0.5) * (1 << level) as f64, (fj as f64 + 0.5) * (1 << level) as f64);
                let ground = b.height_at(face, pi, pj, b.dir(face, pi, pj), level);
                let k = u + v * P;
                // the trunks whose crown can reach the footprint, and those standing in it
                let (lo, hi) = (fi << level, (fi + 1) << level);
                let (lo2, hi2) = (fj << level, (fj + 1) << level);
                let trunks: Vec<Foot> = ((lo - REACH).div_euclid(SPACING)..=(hi + REACH).div_euclid(SPACING)).flat_map(|cx| ((lo2 - REACH).div_euclid(SPACING)..=(hi2 + REACH).div_euclid(SPACING)).map(move |cy| (cx, cy))).filter_map(|(cx, cy)| b.candidate(face, cx, cy)).filter(|f| ROWS.plants[f.plant].trunk != 0).collect();
                // (by a seam the next face's trees reach in too)
                let inner = lo >= REACH && lo2 >= REACH && hi + REACH <= b.n && hi2 + REACH <= b.n;
                if trunks.is_empty() && inner {
                    assert_eq!(t.h[k], ground, "level {level} ({fi}, {fj}): no tree, yet raised");
                    bare += 1;
                } else if trunks.iter().any(|f| (lo..hi).contains(&f.i) && (lo2..hi2).contains(&f.j)) {
                    forest += 1;
                    if t.h[k] >= ground + 3.0 && BLOCKS.get(t.g[k][0]).is_some_and(|d| d.name.contains("leaves")) {
                        raised += 1;
                    }
                }
            }
        }
        assert!(bare > 100 && forest > 50 && raised * 2 >= forest, "level {level}: {raised} of {forest} columns over a trunk stand at a leafy canopy ({bare} bare)");
    }
}

/// The far tiles' fast canopy (ADR 0102) against its slow reference: `tree_candidate` is `candidate` of the
/// trees, `plant_top_with` the last of `plants_with`, at every spot and over trees and seams.
#[test]
fn the_fast_canopy_matches_the_plants() {
    let w = world();
    let b = &w.bodies[0];
    let (n, s) = (b.n, b.n / SPACING);
    let (mut trees, mut tops) = (0, 0);
    for f in 0..6 {
        for (cx, cy) in (-2..s + 2).flat_map(|cx| (-2..s + 2).map(move |cy| (cx, cy))) {
            let slow = b.candidate(f, cx, cy).filter(|t| ROWS.plants[t.plant].trunk != 0);
            assert_eq!(b.tree_candidate(f, cx, cy), slow, "spot {f} {cx} {cy}");
            trees += slow.is_some() as usize;
        }
        // every column of a strip over each seam and a band through the middle
        let cols = (0..n).step_by(3).flat_map(|i| (0..REACH + 2).chain(n / 2 - 8..n / 2 + 8).chain(n - REACH - 2..n).map(move |j| (i, j)));
        for (i, j) in cols.flat_map(|(i, j)| [(i, j), (j, i)]) {
            let slow = b.plants_with(f, i, j, &mut |g, cx, cy| b.candidate(g, cx, cy)).last().copied();
            assert_eq!(b.plant_top_with(f, i, j, &mut |g, cx, cy| b.candidate(g, cx, cy)), slow, "column {f} {i} {j}");
            tops += slow.is_some() as usize;
        }
    }
    assert!(trees > 100 && tops > 1000, "{trees} trees, {tops} columns under a plant");
    // the bound before the biome: a roll that picks a tree in any biome lies in `tree_rolls`
    for k in 0..100_000 {
        let roll = k as f32 / 100_000.0;
        for (bi, row) in ROWS.biomes.iter().enumerate() {
            if row.plant(roll).is_some_and(|p| ROWS.plants[p].trunk != 0) {
                assert!(ROWS.may_tree(roll), "roll {roll} picks a tree in biome {bi}");
            }
        }
    }
    // and at each interval's ends, and the floats beside them
    for &(lo, hi) in &ROWS.tree_rolls {
        // (a roll is in 0..1: `unit`)
        for roll in [lo, lo.next_up(), hi.next_down(), hi, lo.next_down()].into_iter().filter(|r| (0.0..1.0).contains(r)) {
            assert_eq!(ROWS.may_tree(roll), ROWS.biomes.iter().any(|row| row.plant(roll).is_some_and(|p| ROWS.plants[p].trunk != 0)), "roll {roll}");
        }
    }
    // two plants whose tops meet in one layer: wood wins over leaves, else the first stays
    let (spruce, cactus) = (ROWS.plants.iter().position(|p| p.name == "spruce").unwrap(), ROWS.plants.iter().position(|p| p.name == "cactus").unwrap());
    let (i, j) = (n / 2, n / 2);
    let leafy = Foot { plant: spruce, h: 8, v: 7, i, j, z: 40 };
    let wood = Foot { plant: cactus, h: 4, v: 9, i, j, z: 45 };
    let reed = Foot { plant: ROWS.plants.iter().position(|p| p.name == "reed").unwrap(), ..wood };
    assert_ne!(ROWS.plants[reed.plant].trunk, ROWS.plants[cactus].trunk);
    for pair in [[leafy, wood], [wood, leafy], [leafy, Foot { z: 41, ..leafy }], [wood, reed], [reed, wood]] {
        let feed = || {
            let mut k = 0;
            move |_: usize, _: i32, _: i32| {
                k += 1;
                pair.get(k - 1).copied()
            }
        };
        let slow = b.plants_with(0, i, j, &mut feed()).last().copied();
        assert_eq!(b.plant_top_with(0, i, j, &mut feed()), slow, "{pair:?}");
    }
    // whole tiles from their spot grid against the slow path: a level-0 tile's plants and trunks, a far tile's
    // canopy, over forest and over a seam
    let forest: Vec<(usize, i32, i32)> = (0..6).flat_map(|f| (0..s).step_by(9).flat_map(move |cx| (0..s).step_by(9).map(move |cy| (f, cx, cy)))).filter_map(|(f, cx, cy)| b.candidate(f, cx, cy).filter(|t| ROWS.plants[t.plant].trunk != 0).map(|t| (f, t.i, t.j))).step_by(7).chain([(0, 3, n / 2), (2, n / 2, n - 2)]).collect();
    for level in 0..=4u8 {
        for &(f, i, j) in &forest {
            let (x, y) = ((i >> level) / CS, (j >> level) / CS);
            let t = b.build_tile(f, level, x, y);
            let mut seen = Vec::new();
            for (u, v) in (0..P).flat_map(|u| (0..P).map(move |v| (u, v))) {
                let (g, fi, fj) = fold(f, x * CS - 1 + u as i32, y * CS - 1 + v as i32, n >> level);
                let k = u + v * P;
                if level == 0 {
                    // the slow path, and the trunks of the tile's own face's spots it looked at
                    let slow = b.plants_with(g, fi, fj, &mut |g, cx, cy| {
                        let c = b.candidate(g, cx, cy);
                        if let Some(t) = c.filter(|t| g == f && ROWS.plants[t.plant].trunk != 0) {
                            seen.push((t.i, t.j));
                        }
                        c
                    });
                    let fast: Vec<(i32, u16)> = t.plants.iter().filter(|p| p.0 as usize == k).map(|p| (p.1, p.2)).collect();
                    assert_eq!(fast, slow, "level 0 tile {f} {x} {y} column {u} {v}");
                } else {
                    let q = 1 << level;
                    let at: &[(i32, i32)] = if level == 1 { &[(1, 1)] } else { &[(q / 4, q / 4), (3 * q / 4, q / 4), (q / 4, 3 * q / 4), (3 * q / 4, 3 * q / 4)] };
                    let tops = at.iter().filter_map(|&(a, c)| b.plants_with(g, (fi << level) + a, (fj << level) + c, &mut |g, cx, cy| b.candidate(g, cx, cy).filter(|t| ROWS.plants[t.plant].trunk != 0)).last().copied());
                    let (pi, pj) = ((fi as f64 + 0.5) * q as f64, (fj as f64 + 0.5) * q as f64);
                    let hv = b.height_at(g, pi, pj, b.dir(g, pi, pj), level);
                    let mut want = hv;
                    if let Some((z, _)) = tops.max_by_key(|o| o.0) {
                        let wet = b.water_at(g as u8, (fi << level) + q / 2, (fj << level) + q / 2).is_some_and(|w| w.0 > b.layer_of(hv));
                        want = if b.layer_r(z as f64 + 1.0) > hv && !wet { b.layer_r(z as f64 + 1.0) } else { hv };
                    }
                    assert_eq!(t.h[k], want, "level {level} tile {f} {x} {y} column {u} {v}");
                }
            }
            if level == 0 {
                seen.sort_unstable();
                seen.dedup();
                let mut fast = t.trees.clone();
                fast.dedup();
                assert_eq!(fast, seen, "tile {f} {x} {y} trunks");
            }
        }
    }
}

/// What a far tile costs to build over forest (ADR 0100, perf-debt): microseconds a tile by level, each built
/// fresh. `cargo test --profile play -p engine far_tile_cost -- --ignored --nocapture`.
#[test]
#[ignore]
fn far_tile_cost() {
    let w = world();
    let b = &w.bodies[0];
    for level in 1..=4u8 {
        let tiles: Vec<(usize, i32, i32)> = (0..6).flat_map(|f| (0..b.n / SPACING).step_by(3).flat_map(move |cx| (0..b.n / SPACING).step_by(3).map(move |cy| (f, cx, cy)))).filter_map(|(f, cx, cy)| b.candidate(f, cx, cy).filter(|t| ROWS.plants[t.plant].trunk != 0).map(|t| (f, (t.i >> level) / CS, (t.j >> level) / CS))).collect::<std::collections::BTreeSet<_>>().into_iter().take(40).collect();
        let mut us: Vec<f64> = tiles
            .iter()
            .map(|&(f, x, y)| {
                let s = std::time::Instant::now();
                std::hint::black_box(b.build_tile(f, level, x, y));
                s.elapsed().as_secs_f64() * 1e6
            })
            .collect();
        us.sort_by(f64::total_cmp);
        println!("{{\"far_tile\":{{\"level\":{level},\"tiles\":{},\"median_us\":{:.0},\"max_us\":{:.0}}}}}", us.len(), us[us.len() / 2], us[us.len() - 1]);
    }
}

/// A face's seam is no bare strip (ADR 0100): trees stand as close to an edge as further in, and a crown
/// over the edge goes on in the next face's columns, block for block.
#[test]
fn trees_grow_over_face_seams() {
    let w = world();
    let b = &w.bodies[0];
    let n = b.n;
    let (mut edge, mut inner) = (0, 0);
    // blocks of edge trees found, of those sought: [this face's columns, over the seam]
    let (mut sought, mut found) = ([0; 2], [0; 2]);
    for f in 0..6 {
        for (cx, cy) in (0..n / SPACING).flat_map(|cx| (0..n / SPACING).map(move |cy| (cx, cy))) {
            let Some(t) = b.candidate(f, cx, cy) else { continue };
            let p = &ROWS.plants[t.plant];
            if p.trunk == 0 {
                continue;
            }
            let m = t.i.min(t.j).min(n - 1 - t.i).min(n - 1 - t.j);
            if m < REACH {
                edge += 1;
            } else if m < 2 * REACH {
                inner += 1;
            }
            for (dx, dz) in (-p.radius..=p.radius).flat_map(|a| (-p.radius..=p.radius).map(move |c| (a, c))) {
                let (i, j) = (t.i + dx, t.j + dz);
                let over = !(0..n).contains(&i) as usize + !(0..n).contains(&j) as usize;
                // one edge over at most, not round a corner; trees by an edge only
                if over == 2 || m >= REACH {
                    continue;
                }
                let (g, gi, gj) = fold(f, i, j, n);
                for z in 0..=t.h {
                    if let Some((m, _)) = p.block(t.h, t.v, dx, dz, z) {
                        sought[over] += 1;
                        found[over] += (w.generated_block(Block { body: 0, face: g as u8, pos: IVec3::new(gi, gj, t.z + z) }) == m) as usize;
                    }
                }
            }
        }
    }
    assert!(edge * 2 >= inner && inner > 10, "trees within {REACH} columns of an edge: {edge}, the next {REACH}: {inner}");
    // a crown's block can be lost to the ground or a neighbour's wood: as often over the seam as on its own face
    let share = |k: usize| found[k] as f64 / sought[k].max(1) as f64;
    assert!(sought[1] > 50 && share(1) > share(0) - 0.03, "found {:?} of {:?} crown blocks (own face, over the seam)", found, sought);
}

/// Plants of every reader agree (ADR 0043): the block physics sees (`World::generated_block`), the chunk
/// the mesher draws (`generate`) and what the water flows around (`Solid::gaps`), from a cached tile and
/// computed without one.
#[test]
fn the_mesher_physics_and_water_see_the_same_plants() {
    let w = world();
    let b = &w.bodies[0];
    let feet: Vec<Foot> = (0..6).flat_map(|f| (0..b.n / SPACING).flat_map(move |cx| (0..b.n / SPACING).map(move |cy| (f, cx, cy)))).filter_map(|(f, cx, cy)| b.candidate(f, cx, cy).map(|p| (f, p))).map(|(_, p)| p).collect();
    let kinds: std::collections::BTreeSet<usize> = feet.iter().map(|f| f.plant).collect();
    assert!(feet.len() > 20 && kinds.len() >= 3, "{} plants of {} kinds", feet.len(), kinds.len());
    // the spots of face 0 with a plant, and every column a crown reaches
    // the 40 spots of face 0 whose plant's top stands highest over the ground under its crown: tall trees,
    // leaves over a slope (the reviewer's case)
    let over = |fc: usize, f: &Foot| {
        let r = ROWS.plants[f.plant].radius;
        (-r..=r).flat_map(|a| (-r..=r).map(move |c| (a, c))).map(|(a, c)| fold(fc, f.i + a, f.j + c, b.n)).map(|(g, i, j)| f.z + f.h - b.first_air(b.col_height(g as u8, i, j))).max().unwrap()
    };
    let mut spots: Vec<(i32, (usize, i32, i32))> = (0..6).flat_map(|fc| (0..b.n / SPACING).flat_map(move |cx| (0..b.n / SPACING).map(move |cy| (fc, cx, cy)))).filter_map(|(fc, cx, cy)| b.candidate(fc, cx, cy).map(|f| (over(fc, &f), (fc, cx, cy)))).collect();
    spots.sort_by_key(|s| std::cmp::Reverse(s.0));
    assert!(spots.len() > 40 && spots[39].0 >= 10, "tall crowns: {:?}", &spots[..3]);
    assert!((spots[0].0 as f64) < PLANT_TOP - 8.0, "the highest crown {:?} near `PLANT_TOP`", spots[0]);
    let spots: Vec<(usize, i32, i32)> = spots.into_iter().take(40).map(|s| s.1).collect();
    let mut checked = 0;
    for pass in 0..2 {
        for &(fc, cx, cy) in &spots {
            let f = b.candidate(fc, cx, cy).unwrap();
            let fu = fc as u8;
            for (di, dj) in (-REACH..=REACH).flat_map(|a| (-REACH..=REACH).map(move |c| (a, c))) {
                let (i, j) = (f.i + di, f.j + dj);
                // a crown over a seam: `trees_grow_over_face_seams`
                if !(0..b.n).contains(&i) || !(0..b.n).contains(&j) {
                    continue;
                }
                let c = Col::new(0, fu, i, j);
                let gaps = w.ground().gaps(c);
                let key = |z: i32| Key { body: 0, face: fu, level: 0, pos: IVec3::new(i.div_euclid(CS), j.div_euclid(CS), z.div_euclid(CS)) };
                for z in f.z - 3..=f.z + f.h + 2 {
                    let blk = Block { body: 0, face: fu, pos: IVec3::new(i, j, z) };
                    let phys = w.generated_block(blk);
                    let k = key(z);
                    let mesh = match generate(b, k, &[]) {
                        Gen::Cells(cells) => cells[pidx((i - k.pos.x * CS + 1) as usize, (j - k.pos.y * CS + 1) as usize, (z - k.pos.z * CS + 1) as usize)],
                        Gen::Solid => u16::MAX,
                        Gen::Empty => AIR,
                    };
                    assert!(mesh == phys || (mesh == u16::MAX && phys != AIR), "pass {pass} ({i}, {j}, {z}): mesh {mesh}, physics {phys}");
                    let open = gaps.iter().any(|&(lo, hi)| (lo..hi).contains(&z));
                    assert_eq!(open, !BLOCKS.solid(phys), "pass {pass} ({i}, {j}, {z}): water {gaps:?}, physics {phys}");
                    checked += (phys != AIR && z >= f.z) as usize;
                }
            }
        }
        // the second pass reads the tiles the first one's chunks cached
    }
    assert!(checked > 100, "{checked} plant blocks checked");
}

/// A baked planet's cached heights and ground are the computed ones to the bit (ADR 0025, E), and every
/// biome's ground is generated somewhere.
#[test]
fn baked_tiles_match_the_direct_columns() {
    let w = world();
    let b = &w.bodies[0];
    let last = b.n / CS - 1;
    for (face, x, y) in [(2u8, 5, 9), (0, 0, 3), (4, last, 0), (1, 2, last)] {
        let t = b.tile(face, 0, x, y);
        for v in 0..P {
            for u in 0..P {
                let (f, ci, cj) = fold(face as usize, x * CS - 1 + u as i32, y * CS - 1 + v as i32, b.n);
                let (pi, pj) = (ci as f64 + 0.5, cj as f64 + 0.5);
                let d = b.dir(f, pi, pj);
                assert_eq!(t.h[u + v * P].to_bits(), b.height_at(f, pi, pj, d, 0).to_bits(), "face {face} at {u},{v}");
                assert_eq!(t.g[u + v * P], b.ground_at(f, pi, pj, d, t.h[u + v * P]));
            }
        }
    }
    let sea = b.layer_r(b.sea.unwrap());
    assert!(b.height(b.spawn) > sea + 1.0, "the spawn is on land");
    assert!(b.col_height(2, 3, 4) - b.radius < AMP && b.radius - b.col_height(2, 3, 4) < AMP);
}

/// ADR 0107: on the developer's world the polar sea carries a sheet of ice one block thick at its surface, over
/// open water, and the far tiles show it; nowhere else is there ice over air (no sheet over land or in a cave).
#[test]
fn the_polar_sea_carries_a_sheet_of_ice_over_open_water() {
    let w = World::with_map(6144, 32, 933105400, Some(crate::bake::get(933105400, 6144)));
    let b = &w.bodies[0];
    // the sheet is the ice biome's top: snow, or a patch of bare ice
    let (snow, ice) = (BLOCKS.id("snow"), BLOCKS.id("ice"));
    let sheet = |m: u16| m == snow || m == ice;
    let sea = b.sea.unwrap().floor() as i32;
    let (mut sheets, mut looked) = (0, 0);
    // face 4 is the +z pole's
    for i in (0..b.n).step_by(61) {
        for j in (0..b.n).step_by(61) {
            let at = |z: i32| w.generated_block(Block { body: 0, face: 4, pos: IVec3::new(i, j, z) });
            looked += 1;
            if let Some((z, m)) = b.ice_sheet(4, i, j) {
                assert!(sheet(m), "column {i},{j}: a sheet of {m}");
                assert_eq!((at(z - 1), at(z), at(z + 1)), (AIR, m, AIR), "column {i},{j}: water, the sheet, air");
                assert!(b.water_at(4, i, j).unwrap().0 > z as f64, "column {i},{j}: the water's surface lies in the sheet");
                sheets += 1;
            } else {
                assert!((sea - 3..sea + 3).all(|z| !(sheet(at(z)) && at(z - 1) == AIR)), "column {i},{j}: a sheet over air");
            }
        }
    }
    assert!(sheets * 20 > looked, "{sheets} sheets of {looked} columns of the pole's face");
    // the far tiles: a level-3 tile over the frozen sea stands at the sheet's top, in ice
    let (i, j) = (0..b.n).step_by(61).flat_map(|i| (0..b.n).step_by(61).map(move |j| (i, j))).find(|&(i, j)| b.ice_sheet(4, i, j).is_some()).unwrap();
    let q = 8;
    let t = b.tile(4, 3, (i / q).div_euclid(CS), (j / q).div_euclid(CS));
    assert!(t.g.iter().any(|g| sheet(g[0]) && g[2] == g[0]), "a far tile over the frozen sea shows the sheet");
    // a map from before the caps grows no sheet, even where its biome is ice (its icy lakes): its ground holds
    let mut old = Map::from_bytes(&crate::bake::get(933105400, 6144).to_bytes()).unwrap();
    old.version = 6;
    let w6 = World::with_map(6144, 32, 933105400, Some(Arc::new(old)));
    assert_eq!(w6.bodies[0].ice_sheet(4, i, j), None, "a version 6 map");
}

/// A map of version 1 keeps the uniform detail of ADR 0043 to the bit, at every level: a save on it holds
/// only edits, so its ground must not move (ADR 00XX). The slow path is that detail written out again.
#[test]
fn a_version_1_map_keeps_its_ground_to_the_bit() {
    let m = &*MAP;
    let v1 = Map::from_bytes(&postcard::to_stdvec(&(1u32, m.seed, m.n, m.m, &m.h, &m.biome, m.spawn)).unwrap()).unwrap();
    let w = World::with_map(512, 32, 5, Some(Arc::new(v1)));
    let b = &w.bodies[0];
    let bk = b.baked.as_ref().unwrap();
    let s = bk.map.m as f64 / b.n as f64;
    // `Map::height` as it was before ADR 00XX, written out here so a change to its arithmetic shows
    let old_spline = |face: usize, x: f64, y: f64, k: usize| -> f32 {
        let map = &bk.map;
        let k = k.min(map.mips.len());
        let (h, m) = (if k == 0 { &map.h } else { &map.mips[k - 1] }, (map.m >> k) as i32);
        let (x, y) = (x / (1u32 << k) as f64 - 0.5, y / (1u32 << k) as f64 - 0.5);
        let (i0, j0) = (x.floor() as i32, y.floor() as i32);
        let (tx, ty) = ((x - i0 as f64) as f32, (y - j0 as f64) as f32);
        let w = |t: f32| [((-0.5 * t + 1.0) * t - 0.5) * t, (1.5 * t - 2.5) * t * t + 1.0, ((-1.5 * t + 2.0) * t + 0.5) * t, (0.5 * t - 0.5) * t * t];
        let (wx, wy) = (w(tx), w(ty));
        let mut s = 0.0;
        for (b, wb) in wy.iter().enumerate() {
            let mut row = 0.0;
            for (a, wa) in wx.iter().enumerate() {
                let (f, i, j) = fold(face, i0 - 1 + a as i32, j0 - 1 + b as i32, m);
                row += wa * h[(f * m as usize + j as usize) * m as usize + i as usize];
            }
            s += wb * row;
        }
        s
    };
    for face in 0..6 {
        for q in 0..2000 {
            let (i, j) = ((q * 37 % 512) as f64 + 0.5, (q * 91 % 512) as f64 + 0.25);
            let d = b.dir(face, i, j);
            for level in [0u8, 1, 3, 5] {
                let k = (level as i32 - (1.0 / s).log2().round() as i32).max(0) as usize;
                let p = d * b.radius;
                let old = old_spline(face, i * s, j * s, k) as f64 + DETAIL_M * bk.detail.get_noise_3d(p.x as f32, p.y as f32, p.z as f32) as f64;
                let want = b.radius - SEA_DEPTH + old.clamp(-FLOOR, 140.0 - SEA_DEPTH - 2.0 - TALLEST as f64);
                assert_eq!(b.height_at(face, i, j, d, level).to_bits(), want.to_bits(), "face {face} ({i}, {j}) level {level}");
            }
        }
    }
}

/// A scouting tool, not a check (no asserts): the 4 km planet's slopes per map cell and a spot that sees a
/// plain and a steep slope at once, for ADR 00XX's filmstrip (release,
/// after `rivers_cost` kept the map: `cargo test --release -p engine detail_spot -- --ignored --nocapture`).
#[test]
#[ignore]
fn detail_spot() {
    let map = Arc::new(Map::from_bytes(&std::fs::read(std::path::Path::new("../..").join(crate::bake::file(1337, 6144))).unwrap()).unwrap());
    let m = map.m as i32;
    let cell = map.n as f32 / m as f32;
    let at = |f: usize, i: i32, j: i32| {
        let (f, i, j) = fold(f, i, j, m);
        map.h[(f * m as usize + j as usize) * m as usize + i as usize]
    };
    let slope = |f: usize, i: i32, j: i32| (at(f, i + 1, j) - at(f, i - 1, j)).hypot(at(f, i, j + 1) - at(f, i, j - 1)) / (2.0 * cell);
    let mut land: Vec<f32> = (0..map.h.len()).filter(|&c| map.h[c] > 2.0).map(|c| slope(c / (m * m) as usize, c as i32 % m, (c as i32 / m) % m)).collect();
    land.sort_by(f32::total_cmp);
    let q = |p: f64| land[((land.len() - 1) as f64 * p) as usize];
    println!("land slopes: p10 {:.3} p50 {:.3} p90 {:.3} p99 {:.3} max {:.3}", q(0.1), q(0.5), q(0.9), q(0.99), q(1.0));
    let w = World::with_map(map.n, 32, map.seed, Some(map.clone()));
    let b = &w.bodies[0];
    let d = |f: usize, i: i32, j: i32| b.dir(f, (i as f64 + 0.5) * cell as f64, (j as f64 + 0.5) * cell as f64);
    let near_spawn = |f: usize, i: i32, j: i32| d(f, i, j).dot(b.spawn);
    let mut best: Option<(f64, usize, i32, i32, i32, i32)> = None;
    for f in 0..6 {
        for j in 4..m - 4 {
            for i in 4..m - 4 {
                if at(f, i, j) < 4.0 || slope(f, i, j) > 0.05 || (-1..=1).any(|a| (-1..=1).any(|c| slope(f, i + a, j + c) > 0.08)) {
                    continue;
                }
                for (a, c) in [(4, 0), (-4, 0), (0, 4), (0, -4), (3, 3), (-3, 3), (3, -3), (-3, -3)] {
                    if slope(f, i + a, j + c) > 0.5 && at(f, i + a, j + c) > at(f, i, j) && best.is_none_or(|bb| near_spawn(f, i, j) > bb.0) {
                        best = Some((near_spawn(f, i, j), f, i, j, a, c));
                    }
                }
            }
        }
    }
    let (_, f, i, j, a, c) = best.expect("a plain under a steep slope");
    let up = d(f, i, j);
    let to = d(f, i + a, j + c) - up;
    let ahead_ref = if up.y.abs() > 0.9 { DVec3::Z } else { DVec3::Y };
    let ahead = (ahead_ref - up * ahead_ref.dot(up)).normalize();
    let yaw = to.dot(up.cross(ahead)).atan2(to.dot(ahead)).to_degrees();
    println!("spot: at={:.6},{:.6},{:.6} yaw={yaw:.1}: plain {:.1} m, slope {:.2} at {:.0} m", up.x, up.y, up.z, at(f, i, j), slope(f, i + a, j + c), (a as f32).hypot(c as f32) * cell);
}

/// The mantle (ADR 0152, step 1): a new 4 km planet's grid goes on down to within a layer of R/sqrt 2, its
/// lowest blocks 0.70 to 0.72 m; every layer of the old grid keeps its radius at its index plus the mantle's.
/// Luna, a small planet and a body without a mantle keep today's grid, layer for layer.
#[test]
fn the_mantle_carries_the_grid_down_to_r_over_root_2() {
    let (old, new) = (Body::new(DVec3::ZERO, 6144, 1), Body::new(DVec3::ZERO, 6144, 1).with_mantle());
    let k = new.mantle;
    let lowest = new.layer_r(1.0) - new.layer_r(0.0);
    let floor = new.radius * std::f64::consts::FRAC_1_SQRT_2;
    assert!(k % MANTLE_STEP == 0 && (new.r0 - floor).abs() < lowest * MANTLE_STEP as f64 / 2.0, "floor at {} m, R/sqrt 2 {floor}", new.r0);
    assert!((0.70..0.72).contains(&lowest), "the lowest block is {lowest} m");
    assert_eq!((k, new.layers), (mantle_layers(6144), old.layers + k));
    for z in [0, 1, 70, old.layers - 1] {
        let (now, was) = (new.layer_r((z + k) as f64 + 0.5), old.layer_r(z as f64 + 0.5));
        assert!((now - was).abs() < 1e-6, "layer {z}: {now} against {was}");
    }
    assert_eq!(old.mantle, 0);
    assert_eq!((mantle_layers(512), Body::new(DVec3::ZERO, 512, 1).with_mantle().layers), (0, Body::new(DVec3::ZERO, 512, 1).layers), "a small planet keeps its floor");
    let w = World::with_grid(1536, 512, 1, None, None, 0.0, true);
    let luna = Body::new(DVec3::ZERO, 512, 2);
    assert_eq!((w.bodies[1].mantle, w.bodies[1].r0.to_bits(), w.bodies[1].layers), (0, luna.r0.to_bits(), luna.layers), "Luna keeps hers");
    let flat = World::new(1536, 512, 1);
    assert_eq!((flat.bodies[0].mantle, flat.bodies[0].r0.to_bits()), (0, Body::new(DVec3::ZERO, 1536, 1).r0.to_bits()));
}

/// A small baked planet big enough for a mantle (radius 1061 m, 256 layers of mantle under the crust).
static MAP_1536: LazyLock<Arc<Map>> = LazyLock::new(|| Arc::new(Map::bake(5, 1536)));

/// One generated block for every reader (ADR 0062) at any depth: in a column from over the ground down to the
/// floor of a planet on the deep grid, the mesher's fill, `generated_under`, the physics' solid test and the
/// water's gaps agree cell by cell; under the crust's floor (about 192 m) the rock is the mantle's in the mesher and in physics.
#[test]
fn the_readers_agree_from_the_surface_to_the_mantles_floor() {
    let w = World::with_grid(1536, 32, 5, Some(MAP_1536.clone()), None, 0.0, true);
    let b = &w.bodies[0];
    let mantle: Vec<u16> = ROCKS.mantle.iter().map(|d| d.block).collect();
    let (f, i, j) = b.locate(b.spawn);
    let (face, x, y) = (f as u8, i as i32 / CS, j as i32 / CS);
    let top = (b.layer_of(b.radius + AMP) as i32).div_euclid(CS);
    let (mut bad, mut deep, mut shallow) = (Vec::new(), 0, 0);
    for kz in 0..=top {
        let k = Key { body: 0, face, level: 0, pos: IVec3::new(x, y, kz) };
        // an edit in the padding's corner, which no cell below reads: every chunk is generated cell by cell (one
        // all over the ground is air)
        let cells = match generate(b, k, &[(pidx(0, 0, 0) as u32, AIR)]) {
            Gen::Cells(c) => c,
            Gen::Empty => Box::new([AIR; P * P * P]),
            Gen::Solid => panic!("chunk {kz} is solid with an edit of air"),
        };
        for v in 1..=CS as usize {
            for u in 1..=CS as usize {
                let (i, j) = (x * CS - 1 + u as i32, y * CS - 1 + v as i32);
                let h = b.col_height(face, i, j);
                let gaps = w.ground().gaps(Col::new(0, face, i, j));
                for zl in 1..P - 1 {
                    let z = kz * CS - 1 + zl as i32;
                    if z < 0 {
                        continue;
                    }
                    let at = Block { body: 0, face, pos: IVec3::new(i, j, z) };
                    let (mesh, phys) = (cells[pidx(u, v, zl)], w.generated_under(at, h));
                    let open = gaps.iter().any(|&(lo, hi)| (lo..hi).contains(&z));
                    if mesh != phys || open == BLOCKS.solid(phys) || w.generated_solid_under(at, h) != BLOCKS.solid(phys) {
                        bad.push(format!("({i}, {j}, {z}): mesh {mesh}, physics {phys}, water open {open}"));
                    }
                    let r = b.layer_r(z as f64 + 0.5);
                    // (the crust's floor waves round 192 m: a few tens of metres, never past these)
                    if r < b.radius - DEEP - 30.0 {
                        deep += 1;
                        if !mantle.contains(&phys) {
                            bad.push(format!("({i}, {j}, {z}) {} m down: {phys}, not the mantle's", b.radius - r));
                        }
                    } else if r > b.radius - DEEP + MANTLE_LIFT && mantle.contains(&phys) {
                        shallow += 1;
                    }
                }
            }
        }
    }
    assert!(bad.is_empty(), "{} cells disagree, e.g. {:?}", bad.len(), &bad[..bad.len().min(4)]);
    assert!(deep > 100_000 && shallow == 0, "{deep} cells under the crust, {shallow} of the mantle's rock in it");
}

/// The mantle changes nothing above the crust's floor (ADR 0152): around the spawn every generated block from 40
/// layers under the ground to 40 over it is the one a world without the mantle has, its layer counted from there.
#[test]
fn the_surface_is_the_same_with_and_without_the_mantle() {
    let [old, new] = [false, true].map(|m| World::with_grid(1536, 32, 5, Some(MAP_1536.clone()), None, 0.0, m));
    let k = new.bodies[0].mantle;
    let b = &old.bodies[0];
    let (f, i0, j0) = b.locate(b.spawn);
    let (mut bad, mut n) = (Vec::new(), 0);
    for i in i0 as i32 - 48..i0 as i32 + 48 {
        for j in j0 as i32 - 48..j0 as i32 + 48 {
            let g = b.layer_of(b.col_height(f as u8, i, j)) as i32;
            for z in g - 40..g + 40 {
                let at = Block { body: 0, face: f as u8, pos: IVec3::new(i, j, z) };
                let (a, c) = (old.generated_block(at), new.generated_block(Block { pos: at.pos + IVec3::Z * k, ..at }));
                n += 1;
                if a != c {
                    bad.push(format!("({i}, {j}, {z}): {a} against {c}"));
                }
            }
        }
    }
    assert!(k > 0 && bad.is_empty(), "{} of {n} cells differ, e.g. {:?}", bad.len(), &bad[..bad.len().min(6)]);
}

/// Gravity inside a body is a planet's with a dense core (ADR 0152): it grows going down to about 1.135 g at the
/// core's edge (R/sqrt 2, the mantle's floor), joins on there, and falls evenly to nothing at the centre; outside
/// it is unchanged.
#[test]
fn gravity_grows_down_to_the_core_and_falls_to_nothing_at_the_centre() {
    let w = World::new(6144, crate::bodies::columns(1, 6144), 1);
    let (r, g) = (w.bodies[0].radius, w.surface_g(0));
    let a = std::f64::consts::FRAC_1_SQRT_2;
    let at = |x: f64| {
        let p = DVec3::new(0.3, 1.0, -0.2).normalize() * r * x.max(1e-9);
        let got = w.gravity(p);
        assert!(x == 0.0 || got.normalize().dot(-p.normalize()) > 0.9999, "down is to the centre");
        got.length() / g
    };
    for (x, want) in [(1.0, 1.0), (1.5, 1.0 / 2.25), (a, 1.1352), (0.5, 0.8027), (0.0, 0.0)] {
        assert!((at(x) - want).abs() < 1e-3, "at {x} R: {} g, not {want}", at(x));
    }
    assert!((at(a + 1e-6) - at(a - 1e-6)).abs() < 1e-4 && (at(1.0 - 1e-6) - 1.0).abs() < 1e-4, "it joins on at the core's edge and the surface");
    let old_floor = at(1.0 - 192.0 / r);
    assert!(old_floor > 1.0 && old_floor < 1.003, "192 m down, an old world's floor: {old_floor} g");
}

/// The weight of the rock (ADR 0158) is the crust's 2700 kg/m3 and the mantle's 3300 under
/// the world's own gravity, summed metre by metre by hand: at the mantle's floor of the 4 km planet under a
/// column at sea level within 1 %, and the same in closed form part way down.
#[test]
fn the_rock_weighs_what_the_gravity_inside_says_down_to_the_mantles_floor() {
    let w = World::new(6144, crate::bodies::columns(1, 6144), 1);
    let b = Body::new(DVec3::ZERO, 6144, 1).with_mantle();
    let sea = b.radius - SEA_DEPTH;
    let d = DVec3::new(0.3, 1.0, -0.2).normalize();
    let by_hand = |to: f64| {
        let (mut p, mut r) = (ATM, sea);
        while r > to {
            let dr = (r - to).min(0.01);
            let rho = if r - dr * 0.5 > b.radius - DEEP { 2700.0 } else { 3300.0 };
            p += rho * w.gravity(d * (r - dr * 0.5)).length() * dr * 1e-6;
            r -= dr;
        }
        p
    };
    for to in [b.radius - 100.0, b.radius - DEEP, b.radius - 700.0, b.layer_r(0.0)] {
        let (want, got) = (by_hand(to), ATM + b.rock_weight(sea, to));
        eprintln!("{:.0} m down: {got:.3} MPa, by hand {want:.3}", b.radius - to);
        assert!((got / want - 1.0).abs() < 0.01, "{} m down: {got} MPa, by hand {want}", b.radius - to);
    }
    let floor = ATM + b.rock_weight(sea, b.layer_r(0.0));
    assert!((70.0..90.0).contains(&floor), "{floor} MPa at the floor");
}

/// The pressure field (ADR 0158): 1 atmosphere at a land column's ground, growing with every
/// layer down to the floor, and more at a sea floor than at a land column's ground by the water over it.
#[test]
fn pressure_is_one_atmosphere_at_the_ground_and_grows_going_down() {
    let w = World::with_grid(1536, 32, 5, Some(MAP_1536.clone()), None, 0.0, true);
    let b = &w.bodies[0];
    let (f, i, j) = b.locate(b.spawn);
    let (f, i, j) = (f as u8, i as i32, j as i32);
    let ground = b.layer_of(b.col_height(f, i, j));
    let at = |z: f64| b.pressure(f, i, j, z).unwrap();
    assert!((at(ground) - 0.101_325).abs() < 1e-4, "{} MPa at the ground", at(ground));
    assert_eq!(at(ground + 5.0), at(ground), "the air over it weighs nothing here");
    let mut last = at(ground);
    for z in (0..ground as i32).rev() {
        let p = at(z as f64 + 0.5);
        assert!(p > last, "layer {z}: {p} MPa, not over {last}");
        last = p;
    }
    // a sea floor: the nearest column on the spawn's face under 3 m or more of the sea
    let sea = (0..b.n).step_by(16).flat_map(|i| (0..b.n).step_by(16).map(move |j| (i, j))).find(|&(i, j)| b.water_at(f, i, j).is_some_and(|w| b.layer_r(w.0) - b.col_height(f, i, j) > 3.0)).expect("a sea");
    let (top, floor) = (b.layer_r(b.water_at(f, sea.0, sea.1).unwrap().0), b.col_height(f, sea.0, sea.1));
    let p = b.pressure(f, sea.0, sea.1, b.layer_of(floor)).unwrap();
    let want = 0.101_325 + 1000.0 * G * (top - floor) * 1e-6;
    assert!(p > at(ground) && (p as f64 - want).abs() < 1e-3, "the sea floor {} m down: {p} MPa, not {want}", top - floor);
    assert_eq!(World::with_grid(1536, 32, 5, None, None, 0.0, true).bodies[0].pressure(f, i, j, 10.0), None, "no map: none, as heat");
}

/// A mantle boundary (ADR 0158) lies where the column's pressure reaches the row's at the heat
/// there: under a hot rift deeper than under a cold shield (ringwoodite, slope over 0), shallower for bridgmanite
/// and the crust's floor (under 0), by the slope times the heat between them over how fast the pressure grows.
#[test]
fn a_mantle_boundary_moves_with_the_provinces_heat_by_its_slope() {
    use crate::bake::rock::{RIFT, SHIELD};
    let b = Body::new(DVec3::ZERO, 6144, 1).with_mantle();
    let sea = b.radius - SEA_DEPTH;
    let column = |p: u8| Rock { province: p, top: 0.0, air: 15.0, wobble: 0.0 };
    let heat = |p: u8, r: f64| 15.0 + ROCKS.provinces[p as usize].heat as f64 * (sea - r);
    for (k, deep) in ROCKS.mantle.iter().enumerate() {
        // the first layer, going down, of this rock or a deeper one
        let foot = |p: u8| {
            let z = (0..b.layers).rev().find(|&z| ROCKS.mantle[k..].iter().any(|d| d.block == b.rock(column(p), b.layer_r(z as f64 + 0.5) as f32, 0))).unwrap();
            b.layer_r(z as f64 + 0.5)
        };
        let (hot, cold) = (foot(RIFT), foot(SHIELD));
        // where the pressure meets the row's, a layer to spare
        for (p, r) in [(RIFT, hot), (SHIELD, cold)] {
            let (got, want) = (ATM + b.rock_weight(sea, r), deep.at(heat(p, r) as f32) as f64);
            assert!(got >= want && got - want < 0.1, "{}: {got} MPa at its top, the row says {want}", BLOCKS.get(deep.block).unwrap().name);
        }
        // the shift: the slope's MPa over the pressure's growth a metre less the slope's with heat
        let grow = (b.rock_weight(sea, hot - 1.0) - b.rock_weight(sea, hot)) as f64;
        let dh = ROCKS.provinces[RIFT as usize].heat as f64 * (sea - hot) - ROCKS.provinces[SHIELD as usize].heat as f64 * (sea - cold);
        let want = deep.slope as f64 * dh / grow;
        eprintln!("{}: {:.1} m under the rift, {:.1} under the shield", BLOCKS.get(deep.block).unwrap().name, b.radius - hot, b.radius - cold);
        assert!(((cold - hot) - want).abs() < 2.0 && (cold - hot).abs() > 15.0, "{}: {} m deeper under the rift, the data says {want}", BLOCKS.get(deep.block).unwrap().name, cold - hot);
    }
}

/// A mineral with a window (ADR 0158): diamond on the 4 km planet forms only where the field
/// at a body's middle is in its window, so under the cold provinces, never under a rift or an arc, never in the
/// crust; the crust's kinds come first and are those of a world without it; the ledger counts it.
#[test]
fn diamond_forms_only_in_its_window_and_the_ledger_counts_it() {
    use crate::ore::{cells, MINERALS};
    let mut w = World::with_grid(6144, 32, 1, Some(crate::bake::shared(1, 6144)), None, 0.0, true);
    let b = &w.bodies[0];
    let k = MINERALS.kinds.iter().position(|k| k.name == "diamond").unwrap();
    let kind = &MINERALS.kinds[k];
    let ds: Vec<usize> = (0..b.ores.len()).filter(|&o| b.ores[o].kind as usize == k).collect();
    assert!(!ds.is_empty() && ds.len() <= kind.count as usize, "{} bodies", ds.len());
    assert!(ds.iter().all(|&o| o >= b.ores.len() - ds.len()), "the deep kinds' bodies come after the crust's");
    for &o in &ds {
        let d = &b.ores[o];
        let (f, i, j, z) = (d.face, d.at.x.floor() as i32, d.at.y.floor() as i32, d.at.z);
        let (p, t) = (b.pressure(f, i, j, z).unwrap() as f64, b.heat(f, i, j, z).unwrap() as f64);
        let province = b.col_rock(f, i, j).province;
        assert!(![crate::bake::rock::RIFT, crate::bake::rock::ARC].contains(&province), "a diamond under province {province}");
        assert!((36.0..56.0).contains(&p) && (0.0..110.0).contains(&t), "a diamond at {p} MPa, {t} C");
        assert!(b.layer_r(z) < b.radius - DEEP - 100.0, "in the mantle");
        assert!(!cells(b, o).is_empty(), "it holds its blocks");
    }
    w.open_ledger();
    let left = w.ore_left.as_ref().unwrap();
    for &o in &ds {
        assert_eq!(left[o], cells(&w.bodies[0], o).len() as i64, "the ledger counts diamond body {o}");
    }
}

/// The 1536 planet with the mantle and its caves of `data/caves.toml`'s kinds made dense (a start every 16 columns
/// for tubes, its rift is small, 48 for fissures; every one grown), put in the ground beside ADR 0063's: enough
/// of each kind on a small planet to test.
fn dense_caves() -> (World, Vec<crate::caves::System>) {
    let mut w = World::with_grid(1536, 32, 5, Some(MAP_1536.clone()), None, 0.0, true);
    let kinds: Vec<crate::caves::Kind> = crate::caves::CAVES
        .kinds
        .iter()
        .cloned()
        .map(|mut k| {
            (k.spacing, k.chance) = (if k.shape == crate::caves::Shape::Tube { 16 } else { 48 }, 1.0);
            k
        })
        .collect();
    let b = &w.bodies[0];
    let sys = crate::caves::systems_of(b, b.baked_seed().unwrap(), &kinds);
    let mut caves: Vec<Cave> = b.caves.iter().copied().filter(|c| c.kind == Cave::BALL).collect();
    caves.extend(sys.iter().flat_map(|s| s.balls.iter().copied()));
    Arc::get_mut(&mut w.bodies).unwrap()[0].set_caves(caves);
    (w, sys)
}

/// The column (i, j) a point lies in.
fn col_of(p: DVec3) -> (i32, i32) {
    (p.x.floor() as i32, p.y.floor() as i32)
}

/// A cave of a kind (`data/caves.toml`) is cut only in one of its hosts, in a column of one of its provinces: every
/// cell under the ground of every ball of every kind's caves is, in the same world without them, a host block as
/// generated (ore or its halo counts: ore is placed after the caves); the soil band only where that ball breaks
/// through the ground in that column (a skylight, a trench, an open slot).
#[test]
fn a_cave_kind_opens_only_its_host_rock_in_its_provinces() {
    let (w, sys) = dense_caves();
    let mut w0 = World::with_grid(1536, 32, 5, Some(MAP_1536.clone()), None, 0.0, true);
    let old: Vec<Cave> = w0.bodies[0].caves.iter().copied().filter(|c| c.kind == Cave::BALL).collect();
    Arc::get_mut(&mut w0.bodies).unwrap()[0].set_caves(old);
    let b = &w.bodies[0];
    let kinds = &crate::caves::CAVES.kinds;
    for (k, kind) in kinds.iter().enumerate() {
        let n = sys.iter().filter(|s| s.kind as usize == k).count();
        assert!(n >= 3, "{}: {n} made on the 1536 planet", kind.name);
    }
    let (mut bad, mut n, mut soil) = (Vec::new(), 0, 0);
    for s in &sys {
        let kind = &kinds[s.kind as usize];
        for c in &s.balls {
            for (i, j, z) in crate::caves::cells(b, c) {
                n += 1;
                let ground = b.col_height(c.face, i, j);
                let m = w0.generated_block(Block { body: 0, face: c.face, pos: IVec3::new(i, j, z) });
                let breaks = c.span(i, j).is_some_and(|(_, hi)| hi as f64 > b.layer_of(ground));
                let band = ground - b.layer_r(z as f64 + 0.5) < 4.0;
                let ore = w0.bodies[0].ore_at(c.face, i, j, z, ground).is_some() || b.halo_at(c.face, i, j, z).is_some();
                soil += (band && breaks) as usize;
                let ok = if band { breaks } else { kind.hosts.contains(&m) || ore };
                if !ok || !kind.provinces.contains(&b.col_rock(c.face, i, j).province) {
                    bad.push(format!("{} ({i}, {j}, {z}) {:.1} m down: {}", kind.name, ground - b.layer_r(z as f64 + 0.5), BLOCKS.get(m).map_or("?", |d| d.name.as_str())));
                }
            }
        }
    }
    assert!(n > 20_000 && soil > 0 && bad.is_empty(), "{} of {n} cells outside their hosts ({soil} in the soil of openings), e.g. {:?}", bad.len(), &bad[..bad.len().min(4)]);
}

/// No open cell of a kind's cave lies in or beside a column that holds generated water (sea, lake, river): a cave
/// there would drain it through `Solid::gaps`.
#[test]
fn a_cave_kind_keeps_clear_of_water() {
    let (w, sys) = dense_caves();
    let b = &w.bodies[0];
    let wet = |f: u8, i: i32, j: i32| b.water_at(f, i, j).is_some_and(|x| b.layer_r(x.0) > b.col_height(f, i, j));
    let mut bad = Vec::new();
    for s in &sys {
        for c in &s.balls {
            for (i, j, z) in crate::caves::cells(b, c) {
                if (-1..=1).any(|a| (-1..=1).any(|d| wet(c.face, i + a, j + d))) {
                    bad.push((i, j, z));
                }
            }
        }
    }
    assert!(bad.is_empty(), "{} cave cells under or beside water, e.g. {:?}", bad.len(), &bad[..bad.len().min(4)]);
}

/// Lava tubes (deep-world.md 4): 3 to 8 m wide, their floor never rising downhill (the axis's floor radius exactly;
/// the lowest open cell under the axis within a cell, away from the rubble under an opening), open to the sky at
/// a skylight at least every 1.3 times the data's spacing and at least once a tube: there every cell from the
/// tube's floor to the ground is air. (Only in rift and arc basalt: the test above.)
#[test]
fn lava_tubes_run_downhill_and_open_to_the_sky_at_skylights() {
    use crate::caves::{Shape, CAVES};
    let (w, sys) = dense_caves();
    let b = &w.bodies[0];
    let tubes: Vec<_> = sys.iter().filter(|s| CAVES.kinds[s.kind as usize].shape == Shape::Tube).collect();
    assert!(tubes.len() >= 3, "{} tubes", tubes.len());
    for t in &tubes {
        let kind = &CAVES.kinds[t.kind as usize];
        assert!(t.axis.windows(2).all(|p| p[1].z <= p[0].z + 1e-9), "a tube's floor rises: {:?}", t.axis.windows(2).find(|p| p[1].z > p[0].z + 1e-9));
        assert!(t.balls.iter().filter(|c| c.floor <= 0.35 * c.h + 1e-9).all(|c| (1.5..=4.0).contains(&c.r)), "a tube 3 to 8 m wide (its balls under the shafts, cut flat a third down or higher)");
        let near_opening = |k: usize| t.openings.iter().any(|&(a, e)| k + 12 >= a && k <= e + 12);
        let mut last = i32::MAX;
        for (k, p) in t.axis.iter().enumerate() {
            let (i, j) = col_of(*p);
            // (its own balls: another tube may cross it)
            let lo = t.balls.iter().filter_map(|c| c.span(i, j)).map(|s| s.0).min().expect("the axis is open");
            if !near_opening(k) {
                assert!(lo <= last.saturating_add(1), "the floor under the axis rises from {last} to {lo} at {k}");
                last = lo;
            }
        }
        assert!(!t.openings.is_empty(), "a tube without a skylight");
        let starts: Vec<f64> = std::iter::once(0.0).chain(t.openings.iter().map(|o| o.0 as f64)).collect();
        assert!(starts.windows(2).all(|s| s[1] - s[0] <= 1.3 * kind.skylight), "openings {:?} over {} m", t.openings, t.axis.len());
        for &(a, e) in &t.openings {
            let p = t.axis[(a + e) / 2];
            let (i, j) = col_of(p);
            let top = b.layer_of(b.col_height(t.face, i, j)).floor() as i32;
            let floor = b.layer_of(p.z).ceil() as i32 + 2;
            for z in floor..=top {
                let m = w.generated_block(Block { body: 0, face: t.face, pos: IVec3::new(i, j, z) });
                assert_eq!(m, AIR, "the skylight at ({i}, {j}) is shut at layer {z} ({floor}..={top}), open {:?}", b.col_caves(t.face, i, j));
            }
        }
    }
}

/// Fissures (deep-world.md 4) lie within 16 m of a border between provinces, are at most 2 m wide, and each is
/// open at least 10 m tall in some column; some are open at the surface.
#[test]
fn fissures_follow_province_borders_and_are_tall_and_narrow() {
    use crate::caves::{Shape, CAVES};
    let (w, sys) = dense_caves();
    let b = &w.bodies[0];
    let fis: Vec<_> = sys.iter().filter(|s| CAVES.kinds[s.kind as usize].shape == Shape::Fissure).collect();
    assert!(fis.len() >= 3, "{} fissures", fis.len());
    assert!(fis.iter().any(|f| !f.openings.is_empty()), "none open at the surface");
    for f in &fis {
        let mut tallest = 0;
        for c in &f.balls {
            assert!(2.0 * c.r <= 2.0 + 1e-9 && c.h > 2.0 * c.r, "a fissure {} m wide, {} tall", 2.0 * c.r, 2.0 * c.h);
            let (i, j) = col_of(c.at);
            let here = b.col_rock(f.face, i, j).province;
            let border = (-8..=8).any(|a| (-8..=8).any(|d| b.col_rock(f.face, i + 2 * a, j + 2 * d).province != here));
            assert!(border, "a fissure ball at ({i}, {j}) more than 16 m from a province border");
            let ground = b.layer_of(b.col_height(f.face, i, j));
            let open: i32 = b.col_caves(f.face, i, j).iter().map(|&(lo, hi)| (hi as f64).min(ground) as i32 - lo).max().unwrap_or(0);
            tallest = tallest.max(open);
        }
        assert!(tallest >= 10, "a fissure {tallest} m tall at most");
    }
}

/// Caves are made with the planet, from the seed: a second world has the same, and the chunks on either side of a
/// chunk border a tube crosses show the same cells in the padding they share, whichever is generated first.
#[test]
fn a_cave_is_the_same_whichever_chunk_asks_first_and_runs_on_across_a_chunk_border() {
    use crate::caves::{Shape, CAVES};
    let ((w, sys), (w2, sys2)) = (dense_caves(), dense_caves());
    assert!(sys == sys2 && w.bodies[0].caves == w2.bodies[0].caves, "the caves differ between two loads");
    let b = &w.bodies[0];
    let (t, k) = sys
        .iter()
        .filter(|s| CAVES.kinds[s.kind as usize].shape == Shape::Tube)
        .find_map(|s| Some((s, s.axis.windows(2).position(|p| col_of(p[0]).0.div_euclid(CS) != col_of(p[1]).0.div_euclid(CS))?)))
        .expect("a tube crossing a chunk border in i");
    let (i, j) = col_of(t.axis[k]);
    let z = b.layer_of(t.axis[k].z).ceil() as i32 + 1;
    let ka = Key { body: 0, face: t.face, level: 0, pos: IVec3::new(i.div_euclid(CS), j.div_euclid(CS), z.div_euclid(CS)) };
    let kb = Key { pos: ka.pos + IVec3::new(col_of(t.axis[k + 1]).0.div_euclid(CS) - ka.pos.x, 0, 0), ..ka };
    let cells = |b: &Body, k: Key| match generate_raw(b, k, &[]) {
        Gen::Cells(c) => c,
        g => panic!("chunk {k:?} is solid {}", matches!(g, Gen::Solid)),
    };
    let (a1, b1) = (cells(b, ka), cells(b, kb));
    let (b2, a2) = (cells(&w2.bodies[0], kb), cells(&w2.bodies[0], ka));
    assert!(a1 == a2 && b1 == b2, "a chunk differs with the order it was generated in");
    // the padding column of one is the first column of the other
    let (pa, pb) = if kb.pos.x > ka.pos.x { (P - 1, 1) } else { (0, P - 2) };
    let (mut open, mut n) = (0, 0);
    for y in 0..P {
        for zz in 0..P {
            assert_eq!(a1[pidx(pa, y, zz)], b1[pidx(pb, y, zz)], "the padding at y {y} z {zz}");
            n += 1;
            open += (a1[pidx(pa, y, zz)] == AIR) as usize;
        }
    }
    assert!(open > 0 && open < n, "the tube is cut off at the border: {open} of {n} open");
    let zl = (z - ka.pos.z * CS + 1) as usize;
    let at = |c: &[u16], x: i32, y: i32, k: Key| c[pidx((x - k.pos.x * CS + 1) as usize, (y - k.pos.y * CS + 1) as usize, zl)];
    let (i1, j1) = col_of(t.axis[k + 1]);
    assert!(at(&a1[..], i, j, ka) == AIR && at(&b1[..], i1, j1, kb) == AIR, "the tube runs on across the border");
}

/// A planet without the mantle (old saves, `mantle=0`) makes ADR 0063's caves alone, to the bit: the caves of
/// this planet as they were before `data/caves.toml`.
#[test]
fn without_the_mantle_the_caves_are_the_ones_from_before() {
    let w = World::with_grid(1536, 32, 5, Some(MAP_1536.clone()), None, 0.0, false);
    let b = &w.bodies[0];
    assert!(b.caves.iter().all(|c| c.kind == Cave::BALL && c.h == c.r && c.floor == c.r));
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for c in &b.caves {
        for v in [c.face as u64, c.at.x.to_bits(), c.at.y.to_bits(), c.at.z.to_bits(), c.r.to_bits()] {
            h = (h ^ v).wrapping_mul(0x100_0000_01b3);
        }
    }
    assert_eq!((b.caves.len(), h), (104, 15974049385019012728), "the caves of the 1536 planet without the mantle");
}
