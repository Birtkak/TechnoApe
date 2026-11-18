//! Generating a chunk's padded cell grid and its skirts.
use super::*;

pub fn generate(body: &Body, key: Key, edits: &[(u32, u16)]) -> Gen {
    match generate_raw(body, key, edits) {
        Gen::Cells(mut c) => {
            skirt(&mut c);
            Gen::Cells(c)
        }
        g => g,
    }
}

/// `generate` without the skirts: the padding as the neighbours hold it (the slopes read it, ADR 0101).
pub fn generate_raw(body: &Body, key: Key, edits: &[(u32, u16)]) -> Gen {
    let s =(1i32 << key.level) as f64;
    let c0 = key.pos * CS - 1; // padded cell (0, 0, 0) in cells of this level
    let r_lo = body.layer_r(c0.z as f64 * s);
    // the top padding's skirt looks SKIRT cells higher, past the chunk: generated terrain is air there when
    // the column is lower than this
    let z_air = c0.z + (P - 1 + SKIRT) as i32;
    let r_air = body.layer_r((z_air as f64 + 0.5) * s);
    // caves are carved at level 0 only: a coarser cell is too big to show one (world-data.md)
    let caved = key.level == 0 && body.tile_caves(key.face, key.pos.x, key.pos.y).any(|c| {
        let d = (c.at - (c0.as_dvec3() + P as f64 * 0.5)).abs();
        d.x.max(d.y) < c.r + P as f64 * 0.5 && d.z < c.h.max(c.r) + P as f64 * 0.5
    });
    let ored = key.level == 0 && body.tile_ores(key.face, key.pos.x, key.pos.y).iter().any(|&k| body.ore_z[k as usize].0 < (c0.z + P as i32) as f64 && body.ore_z[k as usize].1 > c0.z as f64);
    let plain = edits.is_empty() && !caved && !ored;
    if plain {
        // whole chunk outside the terrain's height range: no noise needed
        if r_lo > body.radius + AMP {
            return Gen::Empty;
        }
        if r_air < body.radius - AMP {
            return Gen::Solid;
        }
    }
    // one height per column, from the footprint's tile (columns past a seam are the neighbouring face's)
    let tile = body.tile(key.face, key.level, key.pos.x, key.pos.y);
    let h = &tile.h;
    let (hmin, hmax) = (tile.min, tile.max);
    if plain {
        // rounding to f32 keeps order, so these agree with the cells' f64 test below
        if r_lo as f32 > hmax {
            return Gen::Empty;
        }
        if (r_air as f32) < hmin {
            return Gen::Solid;
        }
    }
    let mut cells = Box::new([0u16; P * P * P]);
    // coarse cells keep the surface band one cell thick, so the far planet stays green. In f64, as
    // `material` (physics, water) compares: f32 split cells within a quarter millimetre of a threshold
    let cf = s * BLOCK;
    let grass = cf.max(0.5);
    let dirt = (2.0 * cf).max(4.0);
    let mut any_air = false;
    let mut any_solid = false;
    for z in 0..P {
        let l = (c0.z as f64 + z as f64 + 0.5) * s;
        let r = body.layer_r(l);
        let core = l < 0.0;
        for (i, hv) in h.iter().enumerate() {
            let d = hv - r;
            let g = &tile.g[i];
            let mut m = if core { g[2] } else if d >= dirt { body.rock(tile.rock[i], r as f32, g[2]) } else if d >= grass { g[1] } else if d >= 0.0 { g[0] } else { 0 };
            if z == P - 1 && z_air >= 0 && *hv < r_air {
                // the skirt rule for the top padding, which has no cells above it here (ADR 0018). shortcut:
                // from the generated height alone, edits over the chunk do not count; a dug hole there could
                // still open a pinhole against a coarser chunk above
                m = 0;
            }
            any_air |= m == 0;
            any_solid |= m != 0;
            cells[z * P * P + i] = m;
        }
    }
    if caved {
        for (y, x) in (0..P).flat_map(|y| (0..P).map(move |x| (y, x))) {
            for (lo, hi) in body.col_caves(key.face, c0.x + x as i32, c0.y + y as i32) {
                for z in (lo - c0.z).max(0)..(hi - c0.z).min(P as i32) {
                    cells[pidx(x, y, z as usize)] = 0;
                    any_air = true;
                }
            }
        }
    }
    if ored {
        // the halo first (ADR 0095): rock under the soil band, no cave (the rule of `generated_under`); ore after
        // takes its own cells
        let halos: Vec<&crate::ore::OreBody> = body.tile_ores(key.face, key.pos.x, key.pos.y).iter().map(|&k| &body.ores[k as usize]).filter(|o| !crate::ore::MINERALS.kinds[o.kind as usize].halo.is_empty()).collect();
        for (y, x) in (0..P).flat_map(|y| (0..P).map(move |x| (y, x))) {
            let (i, j) = (c0.x + x as i32, c0.y + y as i32);
            let hv = tile.h[y * P + x];
            // the layers a halo can reach in this column: each body's box, its reach to spare
            let (mut lo, mut hi) = (i32::MAX, i32::MIN);
            for o in &halos {
                if let Some((l, h)) = o.halo_span(i, j) {
                    (lo, hi) = (lo.min(l), hi.max(h));
                }
            }
            // a padding column past the face's seam is the next face's, where this face's bodies are not
            if lo >= hi || !(0..body.n).contains(&i) || !(0..body.n).contains(&j) {
                continue;
            }
            for z in (lo - c0.z).max(0)..(hi - c0.z).min(P as i32) {
                let (zl, z) = (c0.z + z, z as usize);
                let c = &mut cells[pidx(x, y, z)];
                if *c != 0 && zl >= 0 && hv - body.layer_r(zl as f64 + 0.5) >= 4.0 {
                    if let Some((k, g)) = body.halo_at(key.face, i, j, zl) {
                        *c = body.halo_block(k, g);
                    }
                }
            }
        }
        // ore where the ground is (the rule of `Body::ore_at`): a cell still solid here is under the ground and
        // no cave; the bodies last to first, so the first body's ore is what stays
        for (y, x) in (0..P).flat_map(|y| (0..P).map(move |x| (y, x))) {
            for (lo, hi, k) in body.col_ores(key.face, c0.x + x as i32, c0.y + y as i32, tile.h[y * P + x]).into_iter().rev() {
                for z in (lo - c0.z).max(0)..(hi - c0.z).min(P as i32) {
                    let c = &mut cells[pidx(x, y, z as usize)];
                    if *c != 0 {
                        *c = body.ore_block(k);
                    }
                }
            }
        }
    }
    // plants stand where the ground left air (the same rule as `generated_under`, `Solid::gaps`)
    for &(col, z, m) in &tile.plants {
        let zl = z - c0.z;
        if (0..P as i32).contains(&zl) && cells[zl as usize * P * P + col as usize] == 0 {
            cells[zl as usize * P * P + col as usize] = m;
            any_solid = true;
        }
    }
    for (i, m) in edits {
        cells[*i as usize] = *m;
        any_air |= *m == 0;
        any_solid |= *m != 0;
    }
    if !any_solid {
        return Gen::Empty;
    }
    if !any_air {
        return Gen::Solid;
    }
    Gen::Cells(cells)
}

/// The skirts of `generate`.
pub fn skirt(cells: &mut [u16; P * P * P]) {
    // skirts: a neighbour at another detail level draws a different surface, so faces culled against the
    // padding would leave holes and slivers at the chunk border. Padding cells within SKIRT cells under
    // air become air, and the chunk draws its own border walls that deep (hidden inside a same-level
    // neighbour, a tidy step against a coarser or finer one). Bottom-up, so the lookahead sees originals.
    for z in 0..P - 1 {
        for y in 0..P {
            for x in 0..P {
                let pad = x == 0 || y == 0 || z == 0 || x == P - 1 || y == P - 1;
                if pad && cells[pidx(x, y, z)] != 0 && (z + 1..=(z + SKIRT).min(P - 1)).any(|a| cells[pidx(x, y, a)] == 0) {
                    cells[pidx(x, y, z)] = 0;
                }
            }
        }
    }
}
