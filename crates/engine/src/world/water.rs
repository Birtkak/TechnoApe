//! The sea's and rivers' quads of a chunk and the chunks water stands in.
use super::*;

/// Generate the padded cell grid of a chunk: x, y, z = column i, column j, layer.
/// The cell (at chunk `key`'s level) that holds the sea's surface, if it is in this chunk: at level 0 the one at
/// the highest tide (the tidal band's columns are solid in the mean's), coarser the mean's, whose solid test
/// stands for the shallows (a coarse cell is 2 layers or more: the band is inside it).
pub fn sea_cell(body: &Body, key: Key) -> Option<i32> {
    let sea = if key.level == 0 { body.sea_high()? } else { body.sea? };
    let cz = (sea / (1i32 << key.level) as f64).floor() as i32;
    (cz.div_euclid(CS) == key.pos.z).then_some(cz)
}

/// The generated sea's surface in chunk `key` (ADR 0020): quads in the mesher's format, top faces of the cell
/// that holds the sea level, merged into rectangles; the water shader lays them at the exact level. Columns in
/// a `wet` tile (sorted) are left out: the client's water mesh draws all of those. A quad's material is 1
/// where the (coarse) cell at the sea level is solid: far away a cell is up to 128 m tall, and the shader lays
/// that water on the cell's top instead of inside it.
pub fn sea_quads(body: &Body, key: Key, wet: &[Col], gen: &Gen) -> Vec<[u32; 2]> {
    let Some(cz) = sea_cell(body, key) else { return Vec::new() };
    let s = 1i32 << key.level;
    // meshed for the highest tide (ADR 0067): the shader lays each vertex at the tide's level and the ground hides
    // the water where it stands higher, so the tide never meshes the sea again
    let r = body.layer_r(body.sea_high().unwrap());
    let base = key.base();
    let n = CS as usize;
    let zl = (cz - key.pos.z * CS) as usize;
    // 0 no sea, 1 sea over air, 2 sea in a solid cell
    let mut sea = vec![0u8; n * n];
    let heights = body.tile(key.face, key.level, key.pos.x, key.pos.y);
    for y in 0..n {
        for x in 0..n {
            let (ci, cj) = (base.x + x as i32 * s, base.y + y as i32 * s);
            // the same sample as `generate`: the middle of the (coarse) column
            if wet.binary_search(&Col::new(key.body, key.face, ci, cj).tile()).is_err() && heights.h[x + 1 + (y + 1) * P] < r {
                let solid = match gen {
                    Gen::Cells(c) => c[pidx(x + 1, y + 1, zl + 1)] != 0,
                    Gen::Solid => true,
                    Gen::Empty => false,
                };
                // a block that holds the sea level stands out of it; only a coarse cell stands for shallows
                if !(solid && key.level == 0) {
                    sea[y * n + x] = 1 + solid as u8;
                }
            }
        }
    }
    let z = (zl + 1) as u32;
    // a quad is a flat chord under the sphere: one wider than the terrain's quads under it would sag below
    // them (120 m for a whole far chunk). Merged quads stay within about 90 m, a sag of 0.25 m.
    let cap = (((2.0 * body.radius).sqrt() / BLOCK) as usize >> key.level).clamp(1, n);
    let mut out = Vec::new();
    for y in 0..n {
        let mut x = 0;
        while x < n {
            let m = sea[y * n + x];
            if m == 0 {
                x += 1;
                continue;
            }
            let mut w = 1;
            while x + w < n && w < cap && sea[y * n + x + w] == m {
                w += 1;
            }
            let mut h = 1;
            while y + h < n && h < cap && (x..x + w).all(|i| sea[(y + h) * n + i] == m) {
                h += 1;
            }
            for v in y..y + h {
                for u in x..x + w {
                    sea[v * n + u] = 0;
                }
            }
            out.push([x as u32 | (y as u32) << 6 | z << 12 | ((w - 1) as u32) << 18 | ((h - 1) as u32) << 23 | 4 << 28, (m - 1) as u32]);
            x += w;
        }
    }
    out
}

/// The generated rivers' and lakes' surface in chunk `key` (W5, ADR 0059), after the sea's quads: kind 5,
/// one quad per wet column whose level lies in this chunk, as a plane: q.y is its level at the column's middle
/// in 1/64 layer over the chunk's base (18 bits), and its fall along i and j in 1/256 layer a column (7 bits
/// each, signed: a steeper one is drawn in steps). Level columns (a lake) are merged as the sea's are.
/// Columns in a `wet` tile are the client's water mesh's; a far coarse cell that is solid at the level has its
/// water on its top. None past level 6, where a river is narrower than a cell.
pub fn river_quads(body: &Body, key: Key, wet: &[Col], gen: &Gen) -> Vec<[u32; 2]> {
    // water stands at most this far over the ground under it (m): a lake over its bed
    const DEEPEST: f64 = 60.0;
    let Some(b) = body.baked.as_ref().filter(|_| key.level <= 6) else { return Vec::new() };
    let s = 1i32 << key.level;
    let (base, n) = (key.base(), CS as usize);
    let heights = body.tile(key.face, key.level, key.pos.x, key.pos.y);
    if body.layer_r(base.z as f64) > heights.max as f64 + DEEPEST || body.layer_r((base.z + key.span()) as f64) < heights.min as f64 {
        return Vec::new();
    }
    // no river line or lake under the chunk and its ring: nothing to look up column by column
    let cell = |c: f64| (c * b.map.m as f64 / body.n as f64).floor() as i32;
    let (lo, hi) = ((base.x - s) as f64, (base.x + key.span() + s) as f64);
    let (lo_j, hi_j) = ((base.y - s) as f64, (base.y + key.span() + s) as f64);
    let (x0, y0, x1, y1) = if body.warp == 0.0 {
        (cell(lo) - 1, cell(lo_j) - 1, cell(hi) + 1, cell(hi_j) + 1)
    } else {
        // the warp bends the footprint's edges in the map: their points' bounds, 9 an edge
        let (mut a, mut c) = ((f64::MAX, f64::MAX), (f64::MIN, f64::MIN));
        for k in 0..=8 {
            let t = k as f64 / 8.0;
            for (i, j) in [(lo + (hi - lo) * t, lo_j), (lo + (hi - lo) * t, hi_j), (lo, lo_j + (hi_j - lo_j) * t), (hi, lo_j + (hi_j - lo_j) * t)] {
                let (x, y) = body.warp_col(i, j);
                (a, c) = ((a.0.min(x), a.1.min(y)), (c.0.max(x), c.1.max(y)));
            }
        }
        (cell(a.0) - 1, cell(a.1) - 1, cell(c.0) + 1, cell(c.1) + 1)
    };
    // nor a level in the chunk's layers: a chunk above or under the water's surface draws none of it
    let Some((lo, hi)) = b.map.wet_near(key.face as usize, x0, y0, x1, y1) else { return Vec::new() };
    let layer = |v: f32| body.layer_of(body.radius - SEA_DEPTH + v as f64);
    if layer(hi) < (base.z - 1) as f64 || layer(lo) > (base.z + key.span() + 1) as f64 {
        return Vec::new();
    }
    // the level of every (coarse) column and the ring around it, in layers: rivers' and lakes', not the sea's
    let m = n + 2;
    let ck = (key.face, key.level, key.pos.x, key.pos.y);
    let cached = b.levels.lock().unwrap().get(&ck).cloned();
    let lv = cached.unwrap_or_else(|| {
        let lv: Arc<Vec<Option<f64>>> = Arc::new(
            (0..m * m)
                .map(|k| {
                    let (ci, cj) = (base.x + (k % m) as i32 * s - s + s / 2, base.y + (k / m) as i32 * s - s + s / 2);
                    if !(0..body.n).contains(&ci) || !(0..body.n).contains(&cj) {
                        return None;
                    }
                    body.water_at(key.face, ci, cj).filter(|w| w.1 != crate::liquid::SEA).map(|w| w.0)
                })
                .collect(),
        );
        let mut c = b.levels.lock().unwrap();
        if c.len() >= LEVEL_GRIDS {
            c.clear();
        }
        c.insert(ck, lv.clone());
        lv
    });
    let mut code: Vec<Option<(u32, u32)>> = vec![None; n * n];
    for y in 0..n {
        for x in 0..n {
            let at = |dx: i32, dy: i32| lv[(x as i32 + 1 + dx) as usize + (y as i32 + 1 + dy) as usize * m];
            let Some(mut l) = at(0, 0) else { continue };
            let cz = (l / s as f64).floor() as i32;
            if cz.div_euclid(CS) != key.pos.z || wet.binary_search(&Col::new(key.body, key.face, base.x + x as i32 * s, base.y + y as i32 * s).tile()).is_ok() {
                continue;
            }
            let zl = (cz - key.pos.z * CS) as usize;
            let solid = match gen {
                Gen::Cells(c) => c[pidx(x + 1, y + 1, zl + 1)] != 0,
                Gen::Solid => true,
                Gen::Empty => false,
            };
            // the fall a column, from the neighbours that hold water too
            let fall = |a: Option<f64>, b: Option<f64>| match (a, b) {
                (Some(a), Some(b)) => (b - a) / 2.0,
                (Some(a), None) => l - a,
                (None, Some(b)) => b - l,
                _ => 0.0,
            } / s as f64;
            let (mut gx, mut gy) = (fall(at(-1, 0), at(1, 0)), fall(at(0, -1), at(0, 1)));
            if solid {
                // a block that holds the level stands out of it; a coarse cell's water lies on its top
                if key.level == 0 {
                    continue;
                }
                (l, gx, gy) = (((cz + 1) * s) as f64, 0.0, 0.0);
            }
            let q = |g: f64| ((g * 256.0).round().clamp(-63.0, 63.0) as i32 & 127) as u32;
            let lvl = ((l - base.z as f64) * 64.0).round().clamp(0.0, (1 << 18) as f64 - 1.0) as u32;
            code[y * n + x] = Some((lvl | q(gx) << 18 | q(gy) << 25, (zl + 1) as u32));
        }
    }
    let cap = (((2.0 * body.radius).sqrt() / BLOCK) as usize >> key.level).clamp(1, n);
    let mut out = Vec::new();
    for y in 0..n {
        let mut x = 0;
        while x < n {
            let Some((c, z)) = code[y * n + x] else {
                x += 1;
                continue;
            };
            // only a level surface merges: a sloped one is a plane about its own column's middle
            let (mut w, mut h) = (1, 1);
            if c >> 18 == 0 {
                while x + w < n && w < cap && code[y * n + x + w] == Some((c, z)) {
                    w += 1;
                }
                while y + h < n && h < cap && (x..x + w).all(|i| code[(y + h) * n + i] == Some((c, z))) {
                    h += 1;
                }
            }
            for v in y..y + h {
                for u in x..x + w {
                    code[v * n + u] = None;
                }
            }
            out.push([x as u32 | (y as u32) << 6 | z << 12 | ((w - 1) as u32) << 18 | ((h - 1) as u32) << 23 | 5 << 28, c]);
            x += w;
        }
    }
    out
}

/// The surface of generation's water in caves under the table (ADR 0190) in level-0 chunk `key`, after the rivers'
/// quads: kind 5, as a lake's (flat, merged), one a column whose cave water's top lies in the chunk in an open cell;
/// a cave full to its roof shows none. Columns in a `wet` tile are the client's water mesh's.
/// shortcut: level 0 only (cave water is seen from inside the ground, near); each column flat at its own level (the
/// table's slope, a few cm a column at most, in steps).
pub fn cave_quads(body: &Body, key: Key, wet: &[Col], gen: &Gen) -> Vec<[u32; 2]> {
    let (0, Gen::Cells(c)) = (key.level, gen) else { return Vec::new() };
    if !body.cave_cols.contains_key(&(key.face, key.pos.x, key.pos.y)) || body.baked.as_ref().is_none_or(|b| b.map.table.is_empty()) {
        return Vec::new();
    }
    let (base, n) = (key.base(), CS as usize);
    let mut code: Vec<Option<(u32, u32)>> = vec![None; n * n];
    for y in 0..n {
        for x in 0..n {
            let (ci, cj) = (base.x + x as i32, base.y + y as i32);
            for (lo, top, _, _) in body.ground_water_col(key.face, ci, cj) {
                let cz = (top - 1).div_euclid(U) as i32;
                // a surface: the water's top short of its gap's roof, in this chunk, its cell open
                if top <= lo || cz.div_euclid(CS) != key.pos.z || wet.binary_search(&Col::new(key.body, key.face, ci, cj).tile()).is_ok() {
                    continue;
                }
                let zl = (cz - key.pos.z * CS) as usize;
                let roofed = top % U == 0 && c[pidx(x + 1, y + 1, zl + 2)] != 0;
                if c[pidx(x + 1, y + 1, zl + 1)] != 0 || roofed {
                    continue;
                }
                let lvl = ((top as f64 / U as f64 - base.z as f64) * 64.0).round().clamp(0.0, (1 << 18) as f64 - 1.0) as u32;
                code[y * n + x] = Some((lvl, (zl + 1) as u32));
            }
        }
    }
    let cap = (((2.0 * body.radius).sqrt() / BLOCK) as usize).clamp(1, n);
    let mut out = Vec::new();
    for y in 0..n {
        let mut x = 0;
        while x < n {
            let Some((cd, z)) = code[y * n + x] else {
                x += 1;
                continue;
            };
            let (mut w, mut h) = (1, 1);
            while x + w < n && w < cap && code[y * n + x + w] == Some((cd, z)) {
                w += 1;
            }
            while y + h < n && h < cap && (x..x + w).all(|i| code[(y + h) * n + i] == Some((cd, z))) {
                h += 1;
            }
            for v in y..y + h {
                for u in x..x + w {
                    code[v * n + u] = None;
                }
            }
            out.push([x as u32 | (y as u32) << 6 | z << 12 | ((w - 1) as u32) << 18 | ((h - 1) as u32) << 23 | 5 << 28, cd]);
            x += w;
        }
    }
    out
}

/// The chunks (levels up to 4) that draw the generated water of tile `t`: the sea's and every river's and
/// lake's level in it. When the tile turns wet or dry they are meshed again (`Streamer::touch_tile`).
pub fn water_keys(body: &Body, t: Col) -> Vec<Key> {
    // the sea's chunks: level 0's at the highest tide, the coarser ones' at the mean (`sea_cell`)
    let mut tops: Vec<f64> = body.sea.into_iter().chain(body.sea_high()).collect();
    for j in t.j() * TILE..(t.j() + 1) * TILE {
        for i in t.i() * TILE..(t.i() + 1) * TILE {
            if let Some((top, acct, _)) = body.water_at(t.face(), i, j) {
                if acct != crate::liquid::SEA {
                    tops.push(top);
                }
            }
            // and the caves' under the table (ADR 0190)
            tops.extend(body.ground_water_col(t.face(), i, j).iter().map(|w| (w.1 - 1) as f64 / U as f64));
        }
    }
    let mut keys: Vec<Key> = (0..=body.root_level.min(4))
        .flat_map(|l| {
            let span = CS << l;
            tops.iter().map(move |&top| Key { body: t.body(), face: t.face(), level: l, pos: IVec3::new(t.i() * TILE / span, t.j() * TILE / span, ((top / (1i32 << l) as f64).floor() as i32).div_euclid(CS)) })
        })
        .collect();
    keys.sort_unstable_by_key(|k| (k.level, k.pos.z));
    keys.dedup();
    keys
}

/// The liquid metal sea's surface in chunk `key` (draft the metal sea): kind 6, one quad per column of a level-0
/// chunk whose layer-0 cell is open, laid 1/8 of the cell over its bottom (q.y as a river's level, in 1/64 layer
/// over the chunk's base, flat). Seen only down a hole in the mantle's floor.
/// shortcut: level 0 only (a hole is seen from near); one quad a column, unmerged, while holes are small
pub fn metal_quads(body: &Body, key: Key, gen: &Gen) -> Vec<[u32; 2]> {
    let (Some(_), 0, 0, Gen::Cells(c)) = (body.core, key.level, key.pos.z, gen) else { return Vec::new() };
    let n = CS as usize;
    (0..n * n)
        .filter(|k| !BLOCKS.solid(c[pidx(k % n + 1, k / n + 1, 1)]))
        .map(|k| [(k % n) as u32 | ((k / n) as u32) << 6 | 1 << 12 | 6 << 28, 8])
        .collect()
}
