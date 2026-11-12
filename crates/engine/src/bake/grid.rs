//! The cube-sphere's coarse grid (ADR 0003): 6 faces of m x m cells, the engine's face axes and its
//! equi-angular warp, so a map cell lines up with the engine's columns. Neighbours fold over the 12 seams
//! and 8 corners through 3D directions, and every distance and area is the true one on the sphere, so
//! nothing downstream knows where a face ends.
use glam::DVec3;
use std::collections::HashMap;
use std::f64::consts::FRAC_PI_2;

/// The engine's face axes (`world.rs`, AX): the face normal and the directions of i and j.
pub const AX: [[[f64; 3]; 3]; 6] = [
    [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
    [[-1., 0., 0.], [0., 0., 1.], [0., 1., 0.]],
    [[0., 1., 0.], [0., 0., 1.], [1., 0., 0.]],
    [[0., -1., 0.], [1., 0., 0.], [0., 0., 1.]],
    [[0., 0., 1.], [1., 0., 0.], [0., 1., 0.]],
    [[0., 0., -1.], [0., 1., 0.], [1., 0., 0.]],
];
pub const NONE: u32 = u32::MAX;

pub struct Grid {
    /// cells per face edge
    pub m: usize,
    /// radius in metres
    pub r: f64,
    /// unit direction of every cell's centre
    pub dir: Vec<[f32; 3]>,
    /// area of a cell in m^2 by its place on a face (the same on all six)
    area: Vec<f32>,
    /// neighbours of the cells on a face's border, folded over the seam (NONE pads)
    edge: HashMap<u32, [u32; 8]>,
}

fn ax(f: usize) -> [DVec3; 3] {
    AX[f].map(DVec3::from_array)
}

impl Grid {
    pub fn new(m: usize, r: f64) -> Grid {
        let mut g = Grid { m, r, dir: Vec::new(), area: Vec::new(), edge: HashMap::new() };
        let n = 6 * m * m;
        g.dir = (0..n)
            .map(|c| {
                let (f, i, j) = g.fij(c as u32);
                g.dir_at(f, i as f64 + 0.5, j as f64 + 0.5).as_vec3().to_array()
            })
            .collect();
        g.area = (0..m * m)
            .map(|k| {
                let (i, j) = ((k % m) as f64, (k / m) as f64);
                let (a, b, c) = (g.dir_at(0, i, j), g.dir_at(0, i + 1.0, j), g.dir_at(0, i, j + 1.0));
                let d = g.dir_at(0, i + 1.0, j + 1.0);
                // two triangles of the cell's corners: a cell is metres across, so flat is exact enough
                (((b - a).cross(c - a).length() + (b - d).cross(c - d).length()) * 0.5 * r * r) as f32
            })
            .collect();
        for c in 0..n as u32 {
            let (f, i, j) = g.fij(c);
            if i > 0 && j > 0 && i + 1 < m && j + 1 < m {
                continue;
            }
            let mut out = [NONE; 8];
            let mut k = 0;
            for (di, dj) in OFFS {
                let (ii, jj) = (i as i64 + di, j as i64 + dj);
                let o = if (0..m as i64).contains(&ii) && (0..m as i64).contains(&jj) {
                    g.idx(f, ii as usize, jj as usize)
                } else {
                    // past the seam: the cell the extended face's point falls in
                    g.locate(g.dir_at(f, ii as f64 + 0.5, jj as f64 + 0.5))
                };
                if o != c && !out[..k].contains(&o) {
                    out[k] = o;
                    k += 1;
                }
            }
            g.edge.insert(c, out);
        }
        g
    }
    pub fn n(&self) -> usize {
        6 * self.m * self.m
    }
    /// A grid without its tables, for `dir_at` and `locate_xy` alone (a map's cells).
    pub fn bare(m: usize, r: f64) -> Grid {
        Grid { m, r, dir: Vec::new(), area: Vec::new(), edge: HashMap::new() }
    }
    pub fn idx(&self, f: usize, i: usize, j: usize) -> u32 {
        ((f * self.m + j) * self.m + i) as u32
    }
    pub fn fij(&self, c: u32) -> (usize, usize, usize) {
        let c = c as usize;
        let m = self.m;
        (c / (m * m), c % m, (c / m) % m)
    }
    /// Direction of the face point (x, y) in cell units (x = 3.5 is the middle of cell 3), past the face too.
    pub fn dir_at(&self, f: usize, x: f64, y: f64) -> DVec3 {
        let h = self.m as f64 * 0.5;
        let th = FRAC_PI_2 / self.m as f64;
        let [n, s, t] = ax(f);
        (n + s * ((x - h) * th).tan() + t * ((y - h) * th).tan()).normalize()
    }
    /// The face of a direction and its continuous cell coordinates there.
    pub fn locate_xy(&self, d: DVec3) -> (usize, f64, f64) {
        let a = d.abs();
        let f = if a.x >= a.y && a.x >= a.z {
            (d.x < 0.0) as usize
        } else if a.y >= a.z {
            2 + (d.y < 0.0) as usize
        } else {
            4 + (d.z < 0.0) as usize
        };
        let [n, s, t] = ax(f);
        let dn = d.dot(n);
        let col = |w: f64| ((w / dn).atan() / FRAC_PI_2 * 2.0 + 1.0) * 0.5 * self.m as f64;
        (f, col(d.dot(s)), col(d.dot(t)))
    }
    pub fn locate(&self, d: DVec3) -> u32 {
        let (f, x, y) = self.locate_xy(d);
        let k = |v: f64| (v.floor().max(0.0) as usize).min(self.m - 1);
        self.idx(f, k(x), k(y))
    }
    pub fn d(&self, c: u32) -> DVec3 {
        DVec3::from_array(self.dir[c as usize].map(f64::from))
    }
    pub fn area(&self, c: u32) -> f32 {
        let (_, i, j) = self.fij(c);
        self.area[j * self.m + i]
    }
    /// Metres between two cells' centres (the chord: a cell is metres wide on a km sphere).
    pub fn dist(&self, a: u32, b: u32) -> f32 {
        let (p, q) = (self.dir[a as usize], self.dir[b as usize]);
        let s = (p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2);
        s.sqrt() * self.r as f32
    }
    /// The 8 neighbours (7 at a corner cell), and how many.
    #[inline]
    pub fn neigh(&self, c: u32) -> ([u32; 8], usize) {
        let (_, i, j) = self.fij(c);
        let m = self.m;
        if i > 0 && j > 0 && i + 1 < m && j + 1 < m {
            let (c, m) = (c as i64, m as i64);
            (OFFS.map(|(di, dj)| (c + di + dj * m) as u32), 8)
        } else {
            let e = self.edge[&c];
            (e, e.iter().take_while(|&&o| o != NONE).count())
        }
    }
    pub fn around(&self, c: u32) -> impl Iterator<Item = u32> {
        let (a, k) = self.neigh(c);
        a.into_iter().take(k)
    }
}

const OFFS: [(i64, i64); 8] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];

#[cfg(test)]
mod tests {
    use super::*;

    /// Every cell has 8 neighbours, 7 at the 24 corner cells, each about a cell away, and the areas sum
    /// to the sphere's: the seams fold.
    #[test]
    fn neighbours_fold_over_the_seams() {
        let g = Grid::new(24, 100.0);
        let mut seven = 0;
        for c in 0..g.n() as u32 {
            let (nb, k) = g.neigh(c);
            assert!(k == 8 || k == 7, "cell {c}: {k}");
            seven += (k == 7) as usize;
            let cell = (g.area(c) as f64).sqrt();
            for &o in &nb[..k] {
                let d = g.dist(c, o) as f64;
                assert!(d > 0.5 * cell && d < 2.0 * cell, "cell {c} to {o}: {d} m, a cell is {cell}");
            }
        }
        assert_eq!(seven, 24, "3 cells at each of the 8 corners");
        let a: f64 = (0..g.n() as u32).map(|c| g.area(c) as f64).sum();
        let want = 4.0 * std::f64::consts::PI * 100.0 * 100.0;
        assert!((a / want - 1.0).abs() < 1e-3, "{a} of {want}");
        for c in [0, 77, 3000] {
            assert_eq!(g.locate(g.d(c)), c);
        }
    }
}
