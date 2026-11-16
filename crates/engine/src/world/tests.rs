/// The developer's rule for Terra and Luna (ADR 0041): Luna is 3.7 times smaller than Terra and 30 Terra
/// diameters away, centre to centre, on every planet size; and from the ground under Luna at a new moon on
/// the ecliptic, Luna covers the sun, a little bigger as the Moon on a total eclipse: totality a few
/// hundredths of Luna's radius wide (some 60 m on the 4 km Terra, about 2 s), and a sun Luna's size would
/// not be total.
#[test]
fn luna_is_a_fixed_part_of_terra_and_just_covers_the_sun() {
    for n in [1536, 3072, 6144, 12288, 24576] {
        let w = World::new(n, crate::bodies::columns(1, n), 1);
        let (big, small) = (w.bodies[0].radius, w.bodies[1].radius);
        assert!((big / small / 3.7 - 1.0).abs() < 0.01, "planet {n}: Terra over Luna is {:.3}", big / small);
        assert!((w.sky.moon_orbit / big - 60.0).abs() < 1e-9, "planet {n}");
        // a new moon on the ecliptic: Luna between Terra and the sun, the ground point under it
        let s = w.sky;
        let t = 0.0;
        let p = s.planet(t).centre;
        let to_sun = (s.sun(t) - p).normalize();
        let (moon, ground) = (p + to_sun * s.moon_orbit, p + to_sun * big);
        let off = to_sun.any_orthonormal_vector() * 0.03 * small;
        assert_eq!(s.covered(t, ground + off, moon, small), 1.0, "planet {n}: not total 3 % of Luna off the middle");
        let bigger = Sky { sun_radius: s.sun_radius / crate::sky::SUN_UNDER_MOON * 1.01, ..s };
        assert!(bigger.covered(t, ground, moon, small) < 1.0, "planet {n}: a sun Luna's size is still covered");
    }
}

#[test]
fn height_tiles_match_the_direct_heights_to_the_bit() {
    // ADR 0025, E: the cache must not move a height by one bit, padding past a seam included
    let b = Body::new(DVec3::ZERO, 1536, 7).with_continents(7);
    let last = b.n / CS - 1;
    for (face, level, x, y) in [(2u8, 0u8, 5, 9), (0, 0, 0, 3), (4, 2, last >> 2, 0), (1, 1, 2, last >> 1)] {
        let s = (1i32 << level) as f64;
        let t = b.tile(face, level, x, y);
        for v in 0..P {
            for u in 0..P {
                let (f, ci, cj) = fold(face as usize, x * CS - 1 + u as i32, y * CS - 1 + v as i32, b.n >> level);
                let want = b.height(b.dir(f, (ci as f64 + 0.5) * s, (cj as f64 + 0.5) * s));
                assert_eq!(t.h[u + v * P].to_bits(), want.to_bits(), "face {face} level {level} at {u},{v}");
            }
        }
    }
    // a level-0 column from a cached tile or computed: the same bits
    let (i, j) = (5 * CS + 7, 9 * CS + 30);
    assert_eq!(b.col_height(2, i, j).to_bits(), b.height(b.dir(2, i as f64 + 0.5, j as f64 + 0.5)).to_bits());
    assert_eq!(b.col_height(3, i, j).to_bits(), b.height(b.dir(3, i as f64 + 0.5, j as f64 + 0.5)).to_bits());
}

/// A full height cache keeps the younger half: the tiles used after the median age, and the new one.
#[test]
fn a_full_height_cache_keeps_the_younger_half() {
    let b = Body::new(DVec3::ZERO, 1536, 7);
    let t = b.tile(0, 0, 0, 0);
    let age = |i: usize| (i as u64 * 7919) % HEIGHT_TILES as u64; // every age once, shuffled
    let keys: Vec<(u8, u8, i32, i32)> = (0..HEIGHT_TILES).map(|i| (0, 0, (i % 64) as i32, (i / 64) as i32)).collect();
    *b.tiles.map.lock().unwrap() = keys.iter().enumerate().map(|(i, k)| (*k, (t.clone(), age(i)))).collect();
    b.tile(1, 0, 0, 0);
    let mut sorted: Vec<u64> = (0..HEIGHT_TILES).map(age).collect();
    sorted.sort();
    let cut = sorted[HEIGHT_TILES / 2];
    let mut want: Vec<(u8, u8, i32, i32)> = keys.iter().enumerate().filter(|(i, _)| age(*i) > cut).map(|(_, k)| *k).collect();
    want.push((1, 0, 0, 0));
    want.sort();
    let mut got: Vec<(u8, u8, i32, i32)> = b.tiles.map.lock().unwrap().keys().copied().collect();
    got.sort();
    assert_eq!(got, want);
}

/// On the 4 km planet a height a hair from a layer's centre, or from the top block's or the band's floor, is
/// one answer for the mesher's cells, the physics and the water: columns placed just each side of them.
#[test]
fn the_mesher_physics_and_water_agree_at_a_layers_centre() {
    let w = World::new(6144, 32, 1);
    let b = &w.bodies[0];
    let (face, x, y) = (2u8, 40, 70);
    let kz = (b.layer_of(b.radius).floor() as i32).div_euclid(CS);
    let zc = kz * CS + CS / 2;
    // heights over cell zc's centre: the air/top, top/band and band/rock boundaries, within f32's rounding
    let h: [f64; P * P] = std::array::from_fn(|i| {
        let edge = [0.0, BLOCK, 4.0][i % 3];
        b.layer_r(zc as f64 + 0.5) + edge + ((i / 3) % 25) as f64 * 2e-5 - 2.4e-4
    });
    let (min, max) = (h.iter().fold(f32::MAX, |a, v| a.min(*v as f32)), h.iter().fold(f32::MIN, |a, v| a.max(*v as f32)));
    b.tiles.map.lock().unwrap().insert((face, 0, x, y), (Arc::new(HeightTile { h, g: [b.ground; P * P], rock: [Rock::NONE; P * P], plants: Vec::new(), biome: [0; P * P], trees: Vec::new(), min, max }), 0));
    let k = Key { body: 0, face, level: 0, pos: IVec3::new(x, y, kz) };
    let Gen::Cells(cells) = generate(b, k, &[]) else { panic!("the chunk is all one") };
    let mut bad = Vec::new();
    for v in 1..=CS as usize {
        for u in 1..=CS as usize {
            let (i, j) = (x * CS - 1 + u as i32, y * CS - 1 + v as i32);
            let gaps = w.ground().gaps(Col::new(0, face, i, j));
            for zl in 1..P - 1 {
                let z = kz * CS - 1 + zl as i32;
                let phys = w.generated_block(Block { body: 0, face, pos: IVec3::new(i, j, z) });
                let mesh = cells[pidx(u, v, zl)];
                let open = gaps.iter().any(|&(lo, hi)| (lo..hi).contains(&z));
                if mesh != phys || open != !BLOCKS.solid(phys) {
                    bad.push(format!("({i}, {j}, {z}) height {:+e} over the centre: mesh {mesh}, physics {phys}, water open {open}", h[u + v * P] - b.layer_r(z as f64 + 0.5)));
                }
            }
        }
    }
    assert!(bad.is_empty(), "{} cells disagree, e.g. {:?}", bad.len(), &bad[..bad.len().min(4)]);
}

/// ADR 0054: a room built 40 m over the ground is still among blocks; only over the build headroom is
/// there surely nothing to hear back.
#[test]
fn only_over_the_build_headroom_is_there_surely_air() {
    let b = Body::new(DVec3::ZERO, 1536, 1337);
    let ground = b.height(DVec3::Y);
    assert!(b.over_top(DVec3::Y * (ground + 40.0)) <= 0.0, "a tower's room");
    assert!(b.over_top(DVec3::Y * (b.radius + AMP + BUILD_HEADROOM - 0.5)) <= 0.0, "the last streamed layer");
    let over = b.over_top(DVec3::Y * (b.radius + AMP + BUILD_HEADROOM + 30.0));
    assert!(over > 28.0 && over <= 30.0, "30 m over the top: {over}");
}

#[test]
fn border_walls_follow_the_surface() {
    let b = Body::new(DVec3::ZERO, 1536, 1337);
    let mid = (b.n / 2 / CS) as i32;
    // the chunk whose layers hold the surface in the middle of face 2
    let hz = b.layer_of(b.height(b.dir(2, (mid * CS + CS / 2) as f64, (mid * CS + CS / 2) as f64))) as i32;
    let k = Key { body: 0, face: 2, level: 0, pos: IVec3::new(mid, mid, hz.div_euclid(CS)) };
    let Gen::Cells(c) = generate(&b, k, &[]) else { panic!("surface chunk has cells") };
    let m = crate::mesh::mesh(&c, 1);
    // every -x face sits at cell x = 0 of a column whose surface is inside the chunk; with +-30 m
    // terrain most of the 32 border columns qualify, and without skirts there would be none
    let west = m.quads.iter().filter(|q| (q[0] >> 28) & 7 == 1 && q[0] & 63 == 0).count();
    assert!(west >= 24, "{west} border faces on the -x side");
}

use super::*;

/// A one-block-wide tower of 60 stays in the far chunks' coarse cells, though no cell's middle lies on it:
/// every cell that holds s of its blocks (s the cell's edge) shows it, at levels 1 to 4; dug away, it is gone.
#[test]
fn a_thin_tower_shows_in_coarse_cells_and_goes_when_dug() {
    let mut w = World::new(1536, 64, 1337);
    w.open_far();
    let lamp = BLOCKS.id("lamp");
    // a column off every coarse cell's middle (i, j even and not both on a middle): ground + 40 up, 60 blocks
    let (i, j) = (100, 98);
    let z = air_at(&w, i, j);
    let tower: Vec<Block> = (z..z + 60).map(|z| Block { body: 0, face: 0, pos: IVec3::new(i, j, z) }).collect();
    // per level, each cell the tower passes and how many of its blocks it holds
    let cells = |level: u8| {
        let s = 1i32 << level;
        let mut c: Vec<(IVec3, i32)> = Vec::new();
        for b in &tower {
            let cell = b.pos.div_euclid(IVec3::splat(s));
            match c.last_mut() {
                Some(l) if l.0 == cell => l.1 += 1,
                _ => c.push((cell, 1)),
            }
        }
        c
    };
    for level in 1..=4u8 {
        assert!(cells(level).iter().all(|&(c, _)| far_cell(&w, 0, c, level) == AIR), "level {level}: the air the tower goes in");
    }
    for &b in &tower {
        w.set_block(b, lamp);
    }
    for level in 1..=4u8 {
        let full = cells(level).iter().filter(|&&(_, n)| n >= 1 << level).count();
        assert!(full >= 2, "level {level}: {full} cells hold a whole edge of the tower");
        for (c, n) in cells(level) {
            assert_eq!(far_cell(&w, 0, c, level), if n >= 1 << level { lamp } else { AIR }, "level {level}: a cell with {n} of the tower");
        }
    }
    for &b in &tower {
        w.set_block(b, AIR);
    }
    for level in 1..=4u8 {
        assert!(cells(level).iter().all(|&(c, _)| far_cell(&w, 0, c, level) == AIR), "level {level}: the dug tower's cells");
    }
}

/// The block coarse cell `cell` of `level` shows, edits and generation together.
fn far_cell(w: &World, face: u8, cell: IVec3, level: u8) -> u16 {
    let k = Key { body: 0, face, level, pos: cell.div_euclid(IVec3::splat(CS)) };
    let q = cell - (k.pos * CS - 1);
    match generate(&w.bodies[0], k, &w.edits_for(k)) {
        Gen::Cells(c) => c[pidx(q.x as usize, q.y as usize, q.z as usize)],
        Gen::Empty => AIR,
        Gen::Solid => u16::MAX,
    }
}

/// A layer of air 40 blocks over the ground at column (i, j) of face 0.
fn air_at(w: &World, i: i32, j: i32) -> i32 {
    let mut z = w.bodies[0].layer_of(w.bodies[0].radius - 100.0) as i32;
    while w.block(Block { body: 0, face: 0, pos: IVec3::new(i, j, z) }) != AIR {
        z += 1;
    }
    z + 40
}

/// ADR 0109: a dig at a coarse cell's middle wins over placed blocks elsewhere in it (a pit with rubble or a
/// lamp in it still reads as a hole).
#[test]
fn a_dig_at_the_middle_beats_placed_blocks_in_the_cell() {
    let mut w = World::new(1536, 64, 1337);
    w.open_far();
    let lamp = BLOCKS.id("lamp");
    let c = IVec3::new(25, 24, (air_at(&w, 100, 96) - 60) / 4); // a level 2 cell under the ground
    assert_ne!(far_cell(&w, 0, c, 2), AIR, "the ground");
    w.set_block(Block { body: 0, face: 0, pos: c * 4 + 2 }, AIR);
    for d in [IVec3::ZERO, IVec3::X, IVec3::Y, IVec3::Z, IVec3::splat(3)] {
        w.set_block(Block { body: 0, face: 0, pos: c * 4 + d }, lamp);
    }
    assert_eq!(far_cell(&w, 0, c, 2), AIR, "the dug middle shows");
}

/// ADR 0109: a coarse cell of edge s shows placed blocks once it holds s of them: a 1-wide line does, one
/// stray block (or s - 1) does not; glass never counts.
#[test]
fn a_coarse_cell_shows_placed_blocks_from_its_edge_in_number() {
    let (lamp, glass) = (BLOCKS.id("lamp"), BLOCKS.id("glass"));
    for level in 1..=4u8 {
        let s = 1i32 << level;
        for (n, m, want) in [(s, lamp, lamp), (s - 1, lamp, AIR), (1, lamp, AIR), (s, glass, AIR)] {
            let mut w = World::new(1536, 64, 1337);
    w.open_far();
            let c = IVec3::new(100, 96, air_at(&w, 100, 96)).div_euclid(IVec3::splat(s));
            // a line up the cell's corner column, off its middle
            for z in 0..n {
                w.set_block(Block { body: 0, face: 0, pos: c * s + IVec3::new(0, 0, z) }, m);
            }
            assert_eq!(far_cell(&w, 0, c, level), want, "level {level}: {n} of block {m}");
        }
    }
}

/// ADR 0109: what a coarse cell shows does not depend on the order its edits came in (the server's and a
/// client's may differ): a tie goes to the lower id.
#[test]
fn a_coarse_cell_does_not_depend_on_the_order_of_its_edits() {
    let (lamp, granite) = (BLOCKS.id("lamp"), BLOCKS.id("granite"));
    let mut cells: Vec<(IVec3, u16)> = (0..8).map(|i| (IVec3::new(i % 8, i / 2 % 2, 0), if i % 2 == 0 { lamp } else { granite })).collect();
    let mut shown = Vec::new();
    for turn in 0..4 {
        let mut w = World::new(1536, 64, 1337);
    w.open_far();
        let c = IVec3::new(96, 96, air_at(&w, 96, 96)).div_euclid(IVec3::splat(8));
        for &(d, m) in &cells {
            w.set_block(Block { body: 0, face: 0, pos: c * 8 + d }, m);
        }
        shown.push(far_cell(&w, 0, c, 3));
        cells.reverse();
        if turn == 1 {
            cells.rotate_left(3);
        }
    }
    assert_eq!(shown, vec![lamp.min(granite); 4]);
}

/// ADR 0109: a model block (drawn as a model) never counts towards a far cell nor is shown at a middle, so
/// hiding models never turns a far ground cell to air; glass and cutouts never count.
#[test]
fn model_and_see_through_blocks_are_never_picked_for_a_far_cell() {
    let reg = crate::blocks::Registry::parse("[[block]]\nid = 0\nname = \"air\"\nsolid = false\n[[block]]\nid = 1\nname = \"rock\"\nhardness = 1\n[[block]]\nid = 2\nname = \"furnace\"\nhardness = 3\nmodel = \"furnace\"\n").unwrap();
    assert!(far_counts(&reg, 1) && !far_counts(&reg, 2) && !far_counts(&reg, AIR));
    assert!(far_mid(&reg, 1) && far_mid(&reg, AIR) && !far_mid(&reg, 2));
    for name in ["glass", "jungle leaves"] {
        assert!(!far_counts(&BLOCKS, BLOCKS.id(name)), "{name}");
    }
}

/// The far summary in a form to compare: per chunk, its picks and its cells' sums, sorted.
fn far_digest(w: &World) -> Vec<String> {
    let mut out: Vec<String> = w
        .far
        .as_ref()
        .unwrap()
        .iter()
        .map(|(k, c)| {
            let mut picks: Vec<_> = c.picks.iter().map(|(p, m)| (p.to_array(), *m)).collect();
            picks.sort_unstable();
            let mut cells: Vec<_> = c
                .cells
                .iter()
                .map(|(p, s)| {
                    let mut n = s.counts.clone();
                    n.sort_unstable();
                    (p.to_array(), s.mid, n)
                })
                .collect();
            cells.sort_unstable();
            format!("{:?} {:?} {picks:?} {cells:?}", (k.body, k.face, k.level), k.pos.to_array())
        })
        .collect();
    out.sort_unstable();
    out
}

/// ADR 0109: a counted block overwritten in place moves its count; taken back (`put_edit(b, None)`, a
/// prediction's rollback) its count and entry go; with every edit taken back the summary is empty.
#[test]
fn far_sums_follow_overwrites_and_take_backs() {
    let (lamp, granite) = (BLOCKS.id("lamp"), BLOCKS.id("granite"));
    let mut w = World::new(1536, 64, 1337);
    w.open_far();
    let c = IVec3::new(100, 96, air_at(&w, 100, 96)).div_euclid(IVec3::splat(4));
    let line: Vec<Block> = (0..4).map(|z| Block { body: 0, face: 0, pos: c * 4 + IVec3::new(0, 0, z) }).collect();
    line.iter().for_each(|&b| w.set_block(b, lamp));
    assert_eq!(far_cell(&w, 0, c, 2), lamp);
    w.set_block(line[3], granite);
    let k = Key { body: 0, face: 0, level: 2, pos: c.div_euclid(IVec3::splat(CS)) };
    let mut n = w.far.as_ref().unwrap()[&k].cells[&c].counts.clone();
    n.sort_unstable();
    assert_eq!(n, vec![(lamp, 3), (granite, 1)], "the overwritten block's count moved");
    assert_eq!(far_cell(&w, 0, c, 2), lamp);
    w.put_edit(line[3], None);
    assert_eq!(w.far.as_ref().unwrap()[&k].cells[&c].counts, vec![(lamp, 3)], "taken back");
    assert_eq!(far_cell(&w, 0, c, 2), AIR, "three of four");
    let mut three = World::new(1536, 64, 1337);
    three.open_far();
    line[..3].iter().for_each(|&b| three.set_block(b, lamp));
    assert_eq!(far_digest(&w), far_digest(&three));
    line.iter().for_each(|&b| w.put_edit(b, None));
    assert!(w.far.as_ref().unwrap().is_empty(), "every edit taken back: {:?}", far_digest(&w));
}

/// ADR 0109: the summary is the same whatever order the edits came in, and built at once from the edits
/// (`open_far` on a loaded world) as edit by edit.
#[test]
fn the_far_summary_does_not_depend_on_edit_order_or_on_loading() {
    let ids = [BLOCKS.id("lamp"), BLOCKS.id("granite"), BLOCKS.id("glass"), AIR];
    let base = IVec3::new(96, 96, air_at(&World::new(1536, 64, 1337), 96, 96) - 44);
    let edits: Vec<(Block, u16)> = (0..600)
        .map(|i: i32| {
            let d = IVec3::new(i % 10, i / 10 % 6, i / 60);
            (Block { body: 0, face: 0, pos: base + d }, ids[(i.wrapping_mul(2654435761u32 as i32) >> 7).rem_euclid(4) as usize])
        })
        .collect();
    let world = |order: &mut dyn Iterator<Item = &(Block, u16)>, open_first: bool| {
        let mut w = World::new(1536, 64, 1337);
        if open_first {
            w.open_far();
        }
        order.for_each(|&(b, m)| w.set_block(b, m));
        if !open_first {
            w.open_far();
        }
        far_digest(&w)
    };
    let forward = world(&mut edits.iter(), true);
    assert!(forward.len() > 4);
    assert_eq!(world(&mut edits.iter().rev(), true), forward, "in reverse");
    assert_eq!(world(&mut edits.iter().step_by(2).chain(edits.iter().skip(1).step_by(2)), true), forward, "evens, then odds");
    assert_eq!(world(&mut edits.iter(), false), forward, "on load");
}

/// ADR 0109: a level 2 cell on a cube-face seam shows the same block in its own face's chunk and in the
/// padding of the chunk across the seam.
#[test]
fn a_far_cell_on_a_seam_shows_the_same_from_both_faces() {
    let lamp = BLOCKS.id("lamp");
    let mut w = World::new(256, 32, 1337);
    w.open_far();
    let nl = 256 >> 2;
    let cz = w.bodies[0].layer_of(w.bodies[0].radius) as i32 / 4;
    // the level 2 cell just past face 0's i = 0 edge, at j = 40 (cells)
    let (f, ci, cj) = fold(0, -1, 40, nl);
    let theirs_cell = IVec3::new(ci, cj, cz);
    for z in 0..4 {
        w.set_block(Block { body: 0, face: f as u8, pos: theirs_cell * 4 + IVec3::new(1, 1, z) }, lamp);
    }
    let shows = |face: u8, at: IVec3| {
        // the chunk whose cells or padding hold `at`: the one of its in-face neighbour
        let k = Key { body: 0, face, level: 2, pos: at.max(IVec3::ZERO).div_euclid(IVec3::splat(CS)) };
        let q = at - (k.pos * CS - 1);
        let i = pidx(q.x as usize, q.y as usize, q.z as usize) as u32;
        w.edits_for(k).iter().find(|e| e.0 == i).map(|e| e.1)
    };
    assert_eq!(shows(f as u8, theirs_cell), Some(lamp), "in its own face's chunk");
    // face 0's chunk at the edge holds it in its padding, at cell (-1, 40)
    assert_eq!(shows(0, IVec3::new(-1, 40, cz)), Some(lamp), "in the padding across the seam");
}

#[test]
fn fold_steps_over_a_seam_and_back() {
    let n = 64;
    for face in 0..6 {
        for k in 0..n {
            for (i, j) in [(-1, k), (n, k), (k, -1), (k, n)] {
                let (f, fi, fj) = fold(face, i, j, n);
                assert_ne!(f, face);
                assert!((0..n).contains(&fi) && (0..n).contains(&fj));
                // the folded cell touches an edge of its face, and one step over that edge is the cell we left
                let back = [(-1, fj), (n, fj), (fi, -1), (fi, n)].into_iter().filter(|(a, b)| (a - fi).abs() + (b - fj).abs() == 1);
                let home = (face, i.clamp(0, n - 1), j.clamp(0, n - 1));
                assert!(back.map(|(a, b)| fold(f, a, b, n)).any(|c| c == home), "face {face} ({i}, {j})");
            }
        }
    }
}

#[test]
fn folded_columns_are_neighbours_in_space() {
    let b = Body::new(DVec3::ZERO, 64, 1);
    let width = b.radius * FRAC_PI_2 / b.n as f64; // the widest column
    for face in 0..6 {
        for k in 0..b.n {
            for (i, j) in [(-1, k), (b.n, k), (k, -1), (k, b.n)] {
                let (f, fi, fj) = fold(face, i, j, b.n);
                let inside = b.dir(face, i.clamp(0, b.n - 1) as f64 + 0.5, j.clamp(0, b.n - 1) as f64 + 0.5);
                let d = (b.dir(f, fi as f64 + 0.5, fj as f64 + 0.5) - inside).length() * b.radius;
                assert!(d < width * 1.01, "face {face} ({i}, {j}) is {d} m from its neighbour");
            }
        }
    }
}

#[test]
fn block_at_finds_the_block_of_its_centre() {
    let w = World::new(96, 32, 1337);
    for body in 0..2u8 {
        let n = w.bodies[body as usize].n;
        for face in 0..6u8 {
            for (i, j, l) in [(0, 0, 0), (n - 1, 0, 40), (n / 2, n / 3, 17), (5, n - 1, 99), (n - 1, n - 1, 3)] {
                let b = Block { body, face, pos: IVec3::new(i, j, l) };
                assert_eq!(w.block_at(w.block_centre(b)), b);
            }
        }
    }
}

/// The top block of a column is the body's top material however deep its middle lies, then the band,
/// then the rock: what `World::block` says is what the mesh draws.
#[test]
fn the_top_block_is_the_top_material() {
    let g = [1, 2, 3];
    assert_eq!([-0.1, 0.05, 0.95, 1.05, 3.9, 4.1].map(|d| material(d, &g)), [AIR, 1, 1, 2, 2, 3]);
}

#[test]
fn blocks_are_cubic_and_a_metre_at_the_surface() {
    let b = Body::new(DVec3::ZERO, 3072, 1);
    let l = b.layer_of(b.radius);
    let thick = b.layer_r(l + 1.0) - b.layer_r(l);
    assert!((thick - BLOCK).abs() < 1e-3, "layer {thick} m");
    let mid = b.n as f64 * 0.5;
    let wide = (b.dir(2, mid + 1.0, mid) - b.dir(2, mid, mid)).length() * b.radius;
    assert!(wide > BLOCK && wide < 1.12 * BLOCK, "widest column {wide} m");
    assert_eq!((b.root_level, b.roots()), (5, 3));
}

/// The padded strip of a chunk at a seam must equal the border cells of the chunk on the other face,
/// or faces would be missing or doubled along the seam.
#[test]
fn padding_across_a_seam_matches_the_neighbouring_chunk() {
    let mut w = World::new(256, 32, 1337);
    let body = &w.bodies[0];
    let top = body.layer_of(body.radius) as i32 / CS;
    // an edit right at the seam, on the neighbour's side
    let (f, fi, fj) = fold(0, -1, 40, 256);
    let dug = Block { body: 0, face: f as u8, pos: IVec3::new(fi, fj, top * CS + 5) };
    let m = if w.block(dug) == 0 { 3 } else { 0 };
    w.set_block(dug, m);
    let body = &w.bodies[0];
    let cells = |k: Key| match generate(body, k, &w.edits_for(k)) {
        Gen::Cells(c) => c,
        Gen::Empty => Box::new([0; P * P * P]),
        Gen::Solid => Box::new([3; P * P * P]),
    };
    let mut checked = 0;
    for ck in top - 1..=top + 1 {
        let k = Key { body: 0, face: 0, level: 0, pos: IVec3::new(0, 1, ck) };
        let mine = cells(k);
        for y in 1..=CS {
            let (f, fi, fj) = fold(0, -1, CS + y - 1, 256);
            let nk = Key { face: f as u8, pos: IVec3::new(fi.div_euclid(CS), fj.div_euclid(CS), ck), ..k };
            let theirs = cells(nk);
            for z in 0..P {
                let (lx, ly) = (fi.rem_euclid(CS) as usize + 1, fj.rem_euclid(CS) as usize + 1);
                // the padding is the neighbour's border with the skirt rule applied: air within SKIRT
                // cells under air (the neighbour's own border cells are never padding, so they keep it)
                let mut want = theirs[pidx(lx, ly, z)];
                if z < P - 1 && (z + 1..=(z + SKIRT).min(P - 1)).any(|a| theirs[pidx(lx, ly, a)] == 0) {
                    want = 0;
                }
                assert_eq!(mine[pidx(0, y as usize, z)], want, "y {y} z {z}");
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 3 * 32 * 34);
    let k = Key { body: 0, face: 0, level: 0, pos: IVec3::new(0, 1, top) };
    // the edit past the seam shows in the padding (a placed block under air is skirted away, so dig)
    if m == 0 {
        assert_eq!(cells(k)[pidx(0, 40 - CS as usize + 1, 6)], 0, "the edit past the seam shows in the padding");
    }
}

#[test]
fn bounds_hold_the_chunk() {
    let b = Body::new(DVec3::new(10.0, 0.0, 0.0), 96, 1);
    for k in [Key { body: 0, face: 3, level: 0, pos: IVec3::new(2, 0, 3) }, Key { body: 0, face: 5, level: 1, pos: IVec3::new(0, 0, 0) }] {
        let bd = b.bounds(k);
        let (base, s) = (k.base().as_dvec3(), k.span() as f64);
        for i in 0..=4 {
            for j in 0..=4 {
                for l in 0..=4 {
                    let g = base + DVec3::new(i as f64, j as f64, l as f64) * s / 4.0;
                    let p = b.centre + b.dir(k.face as usize, g.x, g.y) * b.layer_r(g.z.min(b.layers as f64));
                    assert!((p - bd.c).length() <= bd.rad + 1e-9);
                }
            }
        }
    }
}
