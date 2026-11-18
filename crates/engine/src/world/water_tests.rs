use super::*;
use crate::liquid::Ground;

/// The tide's benchmark (ADR 0067; `cargo test --release -p engine tide_cost -- --ignored --nocapture`): what
/// one `Tide::at`, one `Solid::at` (the poses) and one `generated` of a sea column cost with and without the
/// tide, and how far the ocean's shape moves the mean (`Tide::base`) over a month, on the baked 4 km Terra.
#[test]
#[ignore]
fn tide_cost() {
    use std::hint::black_box;
    use std::time::Instant;
    let w = World::with_map(6144, crate::bodies::columns(1, 6144), 1337, Some(crate::bake::get(1337, 6144)));
    let b = &w.bodies[0];
    let tide = w.tide();
    let n = 2_000_000;
    let t = Instant::now();
    let mut acc = 0.0;
    for k in 0..n {
        acc += tide.at(DVec3::new(1.0, k as f64 * 1e-7, 0.5).normalize());
    }
    let at_ns = t.elapsed().as_secs_f64() * 1e9 / n as f64;
    black_box(acc);
    let t = Instant::now();
    for k in 0..100_000 {
        black_box(Solid::at(&w.bodies, &w.edits, &w.sky, k as f64));
    }
    let solid_ns = t.elapsed().as_secs_f64() * 1e9 / 100_000.0;
    // sea columns: the face's columns under the sea, every 37th
    let cols: Vec<Col> = (0..b.n).step_by(37).flat_map(|i| (0..b.n).step_by(37).map(move |j| Col::new(0, 2, i, j))).filter(|c| b.col_height(2, c.i(), c.j()) < b.layer_r(b.sea.unwrap()) - 3.0).collect();
    let (mean, tidal) = (Solid { bodies: &w.bodies, edits: &w.edits, tide: None }, w.ground());
    let time = |g: &Solid| {
        let t = Instant::now();
        for _ in 0..20 {
            for &c in &cols {
                black_box(g.generated(c));
            }
        }
        t.elapsed().as_secs_f64() * 1e9 / (20 * cols.len()) as f64
    };
    let (gm, gt) = (time(&mean), time(&tidal));
    let bases: Vec<f64> = (0..2000).map(|k| w.sky.tide(k as f64 * crate::sky::MONTH / 2000.0, &b.ocean).base).collect();
    let round = -0.5 * (crate::sky::TIDE_LUNA + crate::sky::TIDE_SUN);
    println!(
        "{{\"tide_at_ns\":{at_ns:.1},\"solid_at_ns\":{solid_ns:.0},\"generated_ns\":{gm:.0},\"generated_tide_ns\":{gt:.0},\"sea_cols\":{},\"ocean_trace\":{:.4},\"base_m\":[{:.4},{:.4}],\"round_base_m\":{round:.4}}}",
        cols.len(),
        b.ocean.x_axis.x + b.ocean.y_axis.y + b.ocean.z_axis.z,
        bases.iter().copied().fold(f64::MAX, f64::min),
        bases.iter().copied().fold(f64::MIN, f64::max)
    );
    // a beach near the spawn for the filmstrips: a run of 4 band columns between the sea and the land
    let band = b.sea.unwrap().floor() as i32 + 1;
    let (f, x, y) = b.locate(b.spawn);
    let top = |i: i32, j: i32| b.first_air(b.col_height(f as u8, i, j));
    let (i0, j0) = (x as i32, y as i32);
    let found = (1..600).find_map(|r| {
        (-r..=r).flat_map(|a| (-r..=r).map(move |c| (i0 + a, j0 + c))).filter(|&(i, j)| (i - i0).abs().max((j - j0).abs()) == r).find_map(|(i, j)| {
            [(1, 0), (-1, 0), (0, 1), (0, -1)].into_iter().find(|&(di, dj)| top(i - di, j - dj) < band && (0..4).all(|k| top(i + k * di, j + k * dj) == band) && top(i + 6 * di, j + 6 * dj) > band).map(|d| (i, j, d))
        })
    });
    if let Some((i, j, (di, dj))) = found {
        let at = |i: i32, j: i32, up: f64| b.centre + b.dir(f, i as f64 + 0.5, j as f64 + 0.5) * (b.layer_r(band as f64) + up);
        let (eye, look) = (at(i + 9 * di, j + 9 * dj, 3.0), at(i - 4 * di, j - 4 * dj, 0.0));
        // run=stand's at=, yaw and pitch for that view (client bench.rs `stand_angles`)
        let up = (eye - b.centre).normalize();
        let fwd = (look - eye).normalize();
        let r = if up.y.abs() > 0.9 { DVec3::Z } else { DVec3::Y };
        let ahead = (r - up * r.dot(up)).normalize();
        let (yaw, pitch) = (fwd.dot(up.cross(ahead)).atan2(fwd.dot(ahead)).to_degrees(), fwd.dot(up).asin().to_degrees());
        println!("{{\"stand\":\"at={:.6},{:.6},{:.6} yaw={yaw:.1} pitch={pitch:.1}\",\"face\":{f},\"i\":{i},\"j\":{j}}}", up.x, up.y, up.z);
        // the tide there over a month, a minute apart: each day's low and high and when the highest came
        let d = b.dir(f, i as f64 + 0.5, j as f64 + 0.5);
        let h = |t: f64| w.sky.tide(t, &b.ocean).at(d);
        let days: Vec<String> = (0..16)
            .map(|k| {
                let ts: Vec<f64> = (0..30).map(|m| (k * 30 + m) as f64 * 60.0).collect();
                let hi = ts.iter().copied().max_by(|&a, &c| h(a).total_cmp(&h(c))).unwrap();
                let lo = ts.iter().copied().fold(f64::MAX, |m, t| m.min(h(t)));
                format!("[{:.3},{:.3},{hi}]", lo, h(hi))
            })
            .collect();
        println!("{{\"days\":[{}]}}", days.join(","));
    }
}

/// The tidal band (ADR 0067): a column whose ground's top is the layer over the mean sea's is dry at the mean
/// and low tide and wet at high, and the sea's quads, meshed once for the highest tide, cover it.
#[test]
fn the_tidal_band_is_wet_at_high_water_and_meshed_for_it() {
    let w = World::new(1536, 512, 1337);
    let b = &w.bodies[0];
    let band = b.sea.unwrap().floor() as i32 + 1;
    let (i, j) = (0..b.n).flat_map(|i| (0..b.n).map(move |j| (i, j))).find(|&(i, j)| b.first_air(b.col_height(2, i, j)) == band).expect("a beach");
    let c = Col::new(0, 2, i, j);
    let d = b.dir(2, i as f64 + 0.5, j as f64 + 0.5);
    // the tide there over a month, a minute apart: dry at its lows, wet at its highs
    let tide = |t: f64| b.tide_layers(&w.sky.tide(t, &b.ocean), d);
    let ts: Vec<f64> = (0..crate::sky::MONTH as usize / 60).map(|k| k as f64 * 60.0).collect();
    let high = *ts.iter().max_by(|&&a, &&x| tide(a).total_cmp(&tide(x))).unwrap();
    let low = *ts.iter().min_by(|&&a, &&x| tide(a).total_cmp(&tide(x))).unwrap();
    let wet = |t: f64| {
        let (lo, hi) = Solid::at(&w.bodies, &w.edits, &w.sky, t).generated(c);
        lo < hi
    };
    assert!(wet(high) && !wet(low), "the tide at the band from {:.2} to {:.2} layers", tide(low), tide(high));
    // its chunk's sea quads cover it, at level 0
    let cz = b.sea_high().unwrap().floor() as i32;
    let key = Key { body: 0, face: 2, level: 0, pos: IVec3::new(i.div_euclid(CS), j.div_euclid(CS), cz.div_euclid(CS)) };
    let quads = sea_quads(b, key, &[], &generate(b, key, &[]));
    let (x, y) = ((i - key.base().x) as u32, (j - key.base().y) as u32);
    let covers = |q: &[u32; 2]| {
        let (qx, qy, qw, qh) = (q[0] & 63, (q[0] >> 6) & 63, ((q[0] >> 18) & 31) + 1, ((q[0] >> 23) & 31) + 1);
        (qx..qx + qw).contains(&x) && (qy..qy + qh).contains(&y)
    };
    assert!(quads.iter().any(covers), "{} quads, none over the band's column", quads.len());
}

/// A column's gaps follow the terrain and its edits: generated ground is solid up to the surface, a dug
/// hollow under it is a closed gap, a placed block splits the open air.
#[test]
fn gaps_follow_terrain_and_edits() {
    let mut w = World::new(1536, 512, 1337);
    let b0 = Block { body: 0, face: 2, pos: IVec3::new(700, 700, 0) };
    let c = World::col(b0);
    let zs = w.ground().gen_floor(c);
    assert_eq!(w.generated_block(Block { pos: IVec3::new(700, 700, zs - 1), ..b0 }) != AIR, true);
    assert_eq!(w.generated_block(Block { pos: IVec3::new(700, 700, zs), ..b0 }), AIR);
    assert_eq!(w.ground().gaps(c), vec![(zs, OPEN)]);
    for z in [zs - 6, zs - 5] {
        w.set_block(Block { pos: IVec3::new(700, 700, z), ..b0 }, AIR);
    }
    w.set_block(Block { pos: IVec3::new(700, 700, zs + 3), ..b0 }, BLOCKS.id("stone"));
    assert_eq!(w.ground().gaps(c), vec![(zs - 6, zs - 4), (zs, zs + 3), (zs + 4, OPEN)]);
}

/// The sea: a low column is water up to sea level from generation, and a point in it is under water.
#[test]
fn the_sea_is_generated_where_the_ground_is_low() {
    let mut w = World::new(1536, 512, 1337);
    let b = &w.bodies[0];
    // find a low column on face 2
    let (i, j) = (0..b.n).step_by(7).flat_map(|i| (0..b.n).step_by(7).map(move |j| (i, j))).find(|&(i, j)| b.height(b.dir(2, i as f64 + 0.5, j as f64 + 0.5)) < b.radius - 10.0).unwrap();
    let d = b.dir(2, i as f64 + 0.5, j as f64 + 0.5);
    // at its tide (ADR 0067): the mean sea and the tide there at the whole second
    let sea = b.sea.unwrap() + b.tide_layers(&w.sky.tide(0.0, &b.ocean), d);
    let c = Col::new(0, 2, i, j);
    let (lo, hi) = w.ground().generated(c);
    assert!(lo < hi && hi == (sea * U as f64) as i64, "{lo} {hi} {}", sea * U as f64);
    let p = b.dir(2, i as f64 + 0.5, j as f64 + 0.5) * (b.layer_r(lo as f64 / U as f64) + 1.0);
    let r = w.water_surface(p).expect("under the sea");
    assert!((r - b.layer_r(sea)).abs() < 1e-3, "the surface at {r}"); // a unit is 0.1 mm
    assert!((r - (b.radius - SEA_DEPTH)).abs() < BLOCK, "the sea {r} within a block of its depth");
    assert_eq!(w.water_surface(p.normalize() * (b.radius + 5.0)), None);
    // the moon has no sea
    assert!(w.bodies[1].sea.is_none());
    // a quarter of a lunar day on, the surface has moved with the tide, and swimming follows it
    let t = 480.7;
    let tide = |w: &World, t: f64| w.bodies[0].tide_layers(&w.sky.tide(t, &w.bodies[0].ocean), d);
    let moved = tide(&w, 480.0) - tide(&w, 0.0);
    assert!(moved.abs() > 0.05, "{moved}");
    w.time = t;
    let r2 = w.water_surface(p).expect("under the sea");
    let b = &w.bodies[0];
    assert!((r2 - b.layer_r(b.sea.unwrap() + tide(&w, 480.0))).abs() < 1e-3, "the surface at {r2} after the tide moved {moved}");
}

/// The standard planet is about OCEAN sea, the spawn is on land, and its ground stays within AMP.
#[test]
fn continents_leave_the_chosen_share_to_the_sea() {
    for seed in [1, 1337] {
        let w = World::new(12288, 512, seed);
        let b = &w.bodies[0];
        let sea = b.layer_r(b.sea.unwrap());
        let hs: Vec<f64> = lattice(20000).map(|d| b.height(d)).collect();
        let wet = hs.iter().filter(|h| **h < sea).count() as f64 / hs.len() as f64;
        assert!((wet - OCEAN).abs() < 0.06, "seed {seed}: {wet:.3} of the surface is sea");
        assert!(hs.iter().all(|h| (h - b.radius).abs() < AMP));
        let top = hs.iter().cloned().fold(f64::MIN, f64::max) - sea;
        assert!(top > 25.0, "seed {seed}: the highest ground is {top:.1} m over the sea");
        assert!(b.height(b.spawn) > sea + 2.0);
    }
}

/// Buckets poured into the planet's sea: the wave stays local (the edge of what is stored is open
/// sea), dies down, and nothing stays stored; every unit is counted.
#[test]
fn a_bucket_in_the_sea_stays_local() {
    let mut w = World::new(1536, 512, 1);
    let b = &w.bodies[0];
    let (i, j) = (0..b.n).step_by(2).flat_map(|i| (0..b.n).step_by(2).map(move |j| (i, j))).find(|&(i, j)| b.height(b.dir(2, i as f64 + 0.5, j as f64 + 0.5)) < b.radius - 7.0).unwrap();
    let c = Col::new(0, 2, i, j);
    let (_, hi) = w.ground().generated(c);
    let World { bodies, edits, water, sky, time, .. } = &mut w;
    assert!(water.pour(&Solid::at(bodies, edits, sky, *time), c, (hi / U) as i32, 4 * U));
    let mut peak = 0;
    for _ in 0..60 * 30 {
        w.step_water();
        peak = peak.max(w.water.stored());
    }
    assert!(peak < 400, "{peak} columns stored");
    assert_eq!((w.water.stored(), w.water.counted()), (0, 4 * U));
}

/// A block placed in the sea, at the surface and on the bed, then dug again (the developer, 2026-11-12:
/// "water also behaved weird when you placed blocks inside it", ADR 0071): the water it pushes out is
/// counted, none stands on the block, the sea around stays level, and once it is dug the sea closes over
/// the column again: as generation has it, nothing left stored.
#[test]
fn a_block_placed_in_the_sea_and_dug_leaves_the_sea_as_it_was() {
    let mut w = World::new(1536, 512, 1);
    let b = &w.bodies[0];
    let (i, j) = (0..b.n).step_by(2).flat_map(|i| (0..b.n).step_by(2).map(move |j| (i, j))).find(|&(i, j)| b.height(b.dir(2, i as f64 + 0.5, j as f64 + 0.5)) < b.radius - 7.0).unwrap();
    let c = Col::new(0, 2, i, j);
    let (lo, hi) = w.ground().generated(c);
    let stone = BLOCKS.id("stone");
    let settle = |w: &mut World| {
        for _ in 0..60 * 30 {
            w.step_water();
        }
    };
    let level = |w: &World, c: Col| w.water.surface_at(&w.ground(), c, (lo / U) as f64 + 0.5);
    for z in [(hi / U) as i32, (lo / U) as i32] {
        let at = Block { body: 0, face: 2, pos: IVec3::new(i, j, z) };
        w.set_block(at, stone);
        w.regap(at);
        assert_eq!(w.water.counted(), 0, "layer {z}: the water the block pushed out is all there");
        settle(&mut w);
        assert_eq!(w.water.counted(), 0, "layer {z}: settled");
        // the sea under a block set into its surface stays full (it swung and drained, ADR 0071)
        for n in w.water.column(c).into_iter().flatten().filter(|n| n.ceil == z) {
            assert!(n.water >= (n.ceil - n.floor) as i64 * U, "layer {z}: under the block {} of {} units", n.water, (n.ceil - n.floor) as i64 * U);
        }
        let over: i64 = w.water.column(c).map_or(0, |ns| ns.filter(|n| n.floor > z).map(|n| n.water).sum());
        let gen_over = (hi - (z as i64 + 1) * U).max(0);
        assert!((over - gen_over).abs() <= U / 16, "layer {z}: {over} units over the block, the sea's {gen_over}");
        let nb = Col::new(0, 2, i + 1, j);
        let s = level(&w, nb).expect("the sea next to it");
        assert!((s - hi as f64 / U as f64).abs() < 1.0 / 16.0, "layer {z}: the sea beside it at {s}");
        w.put_edit(at, None);
        w.regap(at);
        assert_eq!(w.water.counted(), 0, "layer {z}: dug");
        settle(&mut w);
        assert_eq!(w.water.counted(), 0, "layer {z}: dug and settled");
        let s = level(&w, c).expect("the sea closed over the dug column");
        assert!((s - hi as f64 / U as f64).abs() < 1.0 / 16.0, "layer {z}: the dug column's surface at {s}, the sea's {}", hi as f64 / U as f64);
        assert_eq!(w.water.stored(), 0, "layer {z}: the sea is generation's again");
    }
}

/// The metal sea (draft the metal sea): a planet with a mantle has one level for the whole body, at the bottom of
/// layer 0; Luna, a planet without a mantle and one too small for it have none.
#[test]
fn the_metal_sea_lies_at_the_mantles_floor_and_only_there() {
    let w = World::with_grid(1536, 512, 1, None, None, 0.0, true);
    let b = &w.bodies[0];
    assert_eq!(b.core.map(|c| b.layer_r(c)), Some(b.r0), "at the bottom of layer 0");
    assert_eq!(w.bodies[1].core, None, "Luna has none");
    assert_eq!(World::new(1536, 512, 1).bodies[0].core, None, "nor a world without the mantle");
    assert_eq!(Body::new(DVec3::ZERO, 512, 1).with_mantle().core, None, "nor a planet too small for a mantle");
}

/// The metal sea's quads: a lowest chunk draws one in each column whose layer-0 cell is open, at 1/8 of the cell
/// over its bottom, and none where the floor is whole; a chunk over it and a world without the mantle draw none.
#[test]
fn the_metal_shows_only_down_a_hole_in_the_floor() {
    let open = [(3usize, 4usize), (5, 5), (32, 1)];
    let edits: Vec<(u32, u16)> = open.iter().map(|&(u, v)| (pidx(u, v, 1) as u32, AIR)).collect();
    let quads = |w: &World, z: i32| {
        let k = Key { body: 0, face: 2, level: 0, pos: IVec3::new(3, 3, z) };
        metal_quads(&w.bodies[0], k, &generate(&w.bodies[0], k, &edits))
    };
    let w = World::with_grid(1536, 32, 1, None, None, 0.0, true);
    let mut got: Vec<(u32, u32, u32, u32, u32)> = quads(&w, 0).iter().map(|q| (q[0] & 63, q[0] >> 6 & 63, q[0] >> 12 & 63, q[0] >> 28, q[1])).collect();
    got.sort();
    let mut want: Vec<_> = open.iter().map(|&(u, v)| (u as u32 - 1, v as u32 - 1, 1, 6, 8)).collect();
    want.sort();
    assert_eq!(got, want);
    assert!(quads(&w, 1).is_empty(), "a chunk over the floor");
    assert!(quads(&World::new(1536, 32, 1), 0).is_empty(), "a world without the mantle");
}
