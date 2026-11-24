//! Ground water in the world (ADR 0190): the map's water table under a column, the caves it floods, and the aquifer a
//! stored gap trades with (`liquid::Ground::aquifer`). Only a map that holds a table (version 10, generation 12 on) has
//! any; every older world is dry as it was.
use super::*;
use crate::bake::ground::DRY;
use crate::liquid::{Aq, K, U};

impl Body {
    /// The water table under column (i, j) of `face`, layers: the map's table between its cells' middles. None without
    /// one (an older map, tight rock, a body without a map).
    pub fn table_at(&self, face: u8, i: i32, j: i32) -> Option<f64> {
        let b = self.baked.as_ref()?;
        let s = b.map.m as f64 / self.n as f64;
        let (mx, my) = self.map_xy(s, i as f64 + 0.5, j as f64 + 0.5);
        let t = b.map.table_at(face as usize, mx, my)?;
        Some(self.layer_of(self.radius - SEA_DEPTH + t as f64))
    }

    /// The water generation puts in column (i, j)'s caves (ADR 0190, "caves below the table are flooded"): each cave
    /// whose floor lies under the table holds water up to it, a closed one full under it with the pressure share that
    /// gives its head the table's (`liquid::K`), a cave open to the sky up to the table in the open gap (never over the
    /// ground). As `lava_col`: (bottom, top, units held under pressure, kind) in units, bottom-up. None in a column of
    /// generated open water (the sea, a river, a lake: the table's fixed heads) or lava.
    pub fn ground_water_col(&self, face: u8, i: i32, j: i32) -> Vec<(i64, i64, i64, u8)> {
        let caves = self.col_caves(face, i, j);
        if caves.is_empty() {
            return Vec::new();
        }
        let Some(t) = self.table_at(face, i, j) else { return Vec::new() };
        if self.water_at(face, i, j).is_some() || !self.lava_col(face, i, j).is_empty() {
            return Vec::new();
        }
        let zs = self.first_air(self.col_height(face, i, j));
        let tu = (t * U as f64) as i64;
        // the caves as the gaps are: overlapping or touching balls are one air run
        let mut runs: Vec<(i32, i32)> = caves.to_vec();
        runs.sort_unstable();
        let mut merged: Vec<(i32, i32)> = Vec::new();
        for (lo, hi) in runs {
            match merged.last_mut() {
                Some(l) if lo <= l.1 => l.1 = l.1.max(hi),
                _ => merged.push((lo, hi)),
            }
        }
        let mut out = Vec::new();
        for (lo, hi) in merged {
            let (lo_u, hi_u) = (lo as i64 * U, hi as i64 * U);
            if lo_u >= tu {
                break;
            }
            if hi >= zs {
                // open to the sky: a pond in it at the table, under the ground
                let top = tu.min(zs as i64 * U);
                if top > lo_u {
                    out.push((lo_u, top, 0, crate::fluids::WATER));
                }
                break;
            }
            out.push((lo_u, tu.min(hi_u), ((tu - hi_u) / K).max(0), crate::fluids::WATER));
        }
        out
    }

    /// The aquifer under column (i, j) of `face` (`liquid::Ground::aquifer`): None where the map has no table, the rock
    /// under the soil is tight, or generation's open water stands (the table's fixed heads).
    pub fn aquifer(&self, face: u8, i: i32, j: i32) -> Option<Aq> {
        let b = self.baked.as_ref()?;
        let s = b.map.m as f64 / self.n as f64;
        let (mx, my) = self.map_xy(s, i as f64 + 0.5, j as f64 + 0.5);
        let cell = b.map.cell(face as usize, mx, my);
        // (never over the generated ground: a gap there is the open air's, not a well)
        let table = self.table_at(face, i, j)?.min(self.first_air(self.col_height(face, i, j)) as f64);
        if self.water_at(face, i, j).is_some() {
            return None;
        }
        Some(Aq { table, ..self.aquifer_cell(cell as u64)? })
    }

    /// The aquifer of map cell `cell` at rest (its table its column's: `table` the cell's own head), with its four
    /// neighbouring cells.
    pub fn aquifer_cell(&self, cell: u64) -> Option<Aq> {
        let map = &self.baked.as_ref()?.map;
        let c = cell as usize;
        let t = *map.table.get(c)?;
        if t == DRY {
            return None;
        }
        let bottom = -(DEEP - SEA_DEPTH) as f32;
        let (k, sy, floor) = crate::rock::ROCKS.aquifer(*map.rock.get(c)?, map.h[c], bottom)?;
        let m = map.m as usize;
        let (f, ci, cj) = (c / (m * m), (c % m) as f64 + 0.5, ((c / m) % m) as f64 + 0.5);
        let nb = [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)].map(|(di, dj)| map.cell(f, ci + di, cj + dj) as u64);
        let lay = |v: f32| self.layer_of(self.radius - SEA_DEPTH + v as f64);
        let rest = lay(t);
        Some(Aq { cell, nb, rest, table: rest, base: lay(floor), k: k as f64, sy: sy as f64, dx: self.n as f64 / map.m as f64 })
    }
}
