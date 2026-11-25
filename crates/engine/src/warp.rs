//! The conformal grid's warp (a trial, ADR 0131): a face's coordinates (a, b), -1..1 across it, are moved to
//! (a, b) + w * table(a, b) before the equi-angular map of ADR 0003 lays them on the sphere. At w = 1 every block
//! top is square; blocks shrink towards the cube's 8 corners. The table is the same for every face and planet:
//! 193 x 193 points of (da, db), bilinear between them, made and checked in `prototypes/square-blocks`
//! (`warp_table.py`, from `conformal_face.npy`). It is symmetric under the square's 8 symmetries and has da = 0 on
//! a = +-1 (db = 0 on b = +-1), so two faces lay a shared edge on the same points.

use std::sync::LazyLock;

/// Points a table edge: point k is at coordinate -1 + 2k / (N - 1).
pub const N: usize = 193;
/// The table as stored: [ia][ib] pairs of little-endian f32 (da, db), ia along a, ib along b. The GPU's copy
/// (an rg32float texture, x = ib, y = ia) is made from these bytes.
pub static BYTES: &[u8] = include_bytes!("../data/warp_table.bin");

static TABLE: LazyLock<Vec<[f64; 2]>> = LazyLock::new(|| {
    assert_eq!(BYTES.len(), N * N * 8, "warp_table.bin is {N} x {N} pairs of f32");
    let f = |k: usize| f32::from_le_bytes(BYTES[k * 4..k * 4 + 4].try_into().unwrap()) as f64;
    (0..N * N).map(|p| [f(2 * p), f(2 * p + 1)]).collect()
});

/// The table at face point (a, b): (da, db), and its derivatives [d/da, d/db] of (da, db). Bilinear, worked on
/// |a| and |b| and the signs put back, so mirrored points get mirrored values to the bit (the shader's
/// `warp_at` does the same in f32); constant past the face's edge.
pub fn at(a: f64, b: f64) -> ([f64; 2], [[f64; 2]; 2]) {
    let t = &*TABLE;
    let h = (N - 1) as f64 * 0.5;
    let (sa, sb) = (if a < 0.0 { -1.0 } else { 1.0 }, if b < 0.0 { -1.0 } else { 1.0 });
    let (x, y) = ((a.abs().min(1.0) + 1.0) * h, (b.abs().min(1.0) + 1.0) * h);
    let (i0, j0) = ((x.floor() as usize).min(N - 2), (y.floor() as usize).min(N - 2));
    let (fx, fy) = (x - i0 as f64, y - j0 as f64);
    let p = |i: usize, j: usize| t[i * N + j];
    let (t00, t10, t01, t11) = (p(i0, j0), p(i0 + 1, j0), p(i0, j0 + 1), p(i0 + 1, j0 + 1));
    let mix = |u: f64, v: f64, f: f64| u * (1.0 - f) + v * f;
    let v = |k: usize| mix(mix(t00[k], t10[k], fx), mix(t01[k], t11[k], fx), fy);
    // past the edge the table is constant along that axis
    let (ka, kb) = (if a.abs() < 1.0 { h } else { 0.0 }, if b.abs() < 1.0 { h } else { 0.0 });
    let da = |k: usize| mix(t10[k] - t00[k], t11[k] - t01[k], fy) * ka;
    let db = |k: usize| mix(t01[k] - t00[k], t11[k] - t10[k], fx) * kb;
    let s = sa * sb;
    ([sa * v(0), sb * v(1)], [[da(0), s * da(1)], [s * db(0), db(1)]])
}

/// `at`'s (da, db) alone, to the same bits: what every map read of a column asks (some twenty a column of a
/// height tile), without the four derivatives only `unwarp`'s Newton steps use.
pub fn value(a: f64, b: f64) -> [f64; 2] {
    let t = &*TABLE;
    let h = (N - 1) as f64 * 0.5;
    let (sa, sb) = (if a < 0.0 { -1.0 } else { 1.0 }, if b < 0.0 { -1.0 } else { 1.0 });
    let (x, y) = ((a.abs().min(1.0) + 1.0) * h, (b.abs().min(1.0) + 1.0) * h);
    let (i0, j0) = ((x.floor() as usize).min(N - 2), (y.floor() as usize).min(N - 2));
    let (fx, fy) = (x - i0 as f64, y - j0 as f64);
    let (r0, r1) = (&t[i0 * N + j0..i0 * N + j0 + 2], &t[(i0 + 1) * N + j0..(i0 + 1) * N + j0 + 2]);
    let mix = |u: f64, v: f64, f: f64| u * (1.0 - f) + v * f;
    let v = |k: usize| mix(mix(r0[0][k], r1[0][k], fx), mix(r0[1][k], r1[1][k], fx), fy);
    [sa * v(0), sb * v(1)]
}

/// (a, b) warped with strength w.
pub fn warp(a: f64, b: f64, w: f64) -> (f64, f64) {
    let v = value(a, b);
    (a + w * v[0], b + w * v[1])
}

/// The (a, b) that `warp` moves to (ta, tb): Newton on the bilinear cell from one fixed-point step, to 1e-14.
/// (Plain fixed-point steps crawl near a corner, where the table's slope is about -0.75.)
pub fn unwarp(ta: f64, tb: f64, w: f64) -> (f64, f64) {
    let (v, _) = at(ta, tb);
    let (mut a, mut b) = (ta - w * v[0], tb - w * v[1]);
    for _ in 0..40 {
        let (v, d) = at(a, b);
        let (ra, rb) = (a + w * v[0] - ta, b + w * v[1] - tb);
        if ra.abs().max(rb.abs()) < 1e-14 {
            break;
        }
        // J = I + w dT: columns d/da and d/db
        let (j00, j10, j01, j11) = (1.0 + w * d[0][0], w * d[0][1], w * d[1][0], 1.0 + w * d[1][1]);
        let det = j00 * j11 - j01 * j10;
        a -= (j11 * ra - j01 * rb) / det;
        b -= (j00 * rb - j10 * ra) / det;
    }
    (a, b)
}

#[cfg(test)]
mod tests {
    use crate::world::{fold, Body, AX};
    use glam::DVec3;
    use std::f64::consts::FRAC_PI_2;

    /// The standard planet (6144 columns a face edge, 4245 m) on the conformal grid.
    fn conformal() -> Body {
        let mut b = Body::new(DVec3::ZERO, 6144, 1);
        b.warp = 1.0;
        b
    }
    /// A block's two edges from grid point (i, j), metres: along i and along j, and the angle between them.
    fn block(b: &Body, f: usize, i: f64, j: f64) -> (f64, f64, f64) {
        let o = b.dir(f, i, j);
        let (u, v) = ((b.dir(f, i + 1.0, j) - o) * b.radius, (b.dir(f, i, j + 1.0) - o) * b.radius);
        (u.length(), v.length(), u.angle_between(v).to_degrees())
    }

    #[test]
    fn a_block_at_a_face_edge_is_square() {
        let b = conformal();
        let n = b.n as f64;
        for f in 0..6 {
            let (u, v, _) = block(&b, f, n - 1.0, n / 2.0);
            assert!((u / 1.05 - 1.0).abs() < 0.03 && (v / 1.05 - 1.0).abs() < 0.03, "face {f}: {u:.3} x {v:.3} m");
        }
    }

    #[test]
    fn a_block_near_a_corner_is_small_and_square() {
        let b = conformal();
        let n = b.n as f64;
        for (f, i, j) in [(0, 100.0, 100.0), (3, n - 101.0, 100.0), (5, n - 101.0, n - 101.0)] {
            let (u, v, ang) = block(&b, f, i, j);
            assert!((u - 0.43).abs() < 0.02 && (v - 0.43).abs() < 0.02, "face {f} ({i}, {j}): {u:.3} x {v:.3} m");
            assert!((ang - 90.0).abs() < 4.0, "face {f} ({i}, {j}): a corner of {ang:.2} degrees");
        }
    }

    #[test]
    fn locate_undoes_dir_on_the_warped_grid() {
        let b = conformal();
        let n = b.n as f64;
        let at = [0.3, 1.0, 1.7, 2.0, 37.25, 500.5, n / 2.0 + 0.1, n - 700.0, n - 2.0, n - 1.5, n - 0.2];
        for f in 0..6 {
            for &i in &at {
                for &j in &at {
                    let (g, x, y) = b.locate(b.dir(f, i, j));
                    assert!(g == f && (x - i).abs() < 1e-6 && (y - j).abs() < 1e-6, "face {f} ({i}, {j}) came back as face {g} ({x}, {y})");
                }
            }
        }
    }

    #[test]
    fn faces_lay_a_shared_edge_on_the_same_points() {
        let b = conformal();
        let n = b.n;
        let mut seen = 0;
        for f in 0..6 {
            for k in (0..=n).step_by(97).chain([1, 2, n - 2, n - 1, n]) {
                let c = k.min(n - 1);
                // a grid point on each of the face's four edges, and the column past the edge beside it, which
                // `fold` steps over onto the neighbouring face
                for (i, j, ci, cj) in [(n, k, n, c), (0, k, -1, c), (k, n, c, n), (k, 0, c, -1)] {
                    let (g, _, _) = fold(f, ci, cj, n);
                    let [nf, s, t] = AX[f];
                    // the grid point on the cube of half-size n, in g's grid
                    let p = nf * n + s * (2 * i - n) + t * (2 * j - n);
                    let [_, gs, gt] = AX[g];
                    let (gi, gj) = ((gs.dot(p) + n) / 2, (gt.dot(p) + n) / 2);
                    let d = (b.dir(f, i as f64, j as f64) - b.dir(g, gi as f64, gj as f64)).length();
                    assert!(d < 1e-12, "face {f} ({i}, {j}) and face {g} ({gi}, {gj}) are {:.3e} apart", d);
                    seen += 1;
                }
            }
        }
        assert!(seen > 6 * 60);
    }

    #[test]
    fn with_no_warp_the_grid_is_todays_to_the_bit() {
        let b = Body::new(DVec3::ZERO, 6144, 1);
        assert_eq!(b.warp, 0.0);
        let n = b.n as f64;
        // today's formulas (world.rs before ADR 0131), written out
        let col_tan = |i: f64| ((i - n * 0.5) * (FRAC_PI_2 / n)).tan();
        for f in 0..6 {
            let [nn, s, t] = AX[f].map(|v| v.as_dvec3());
            for (i, j) in [(0.5, 0.5), (17.3, 4000.9), (3072.0, 3071.5), (6143.5, 6.25), (6144.0, 6144.0), (-0.5, 100.0)] {
                let d = (nn + s * col_tan(i) + t * col_tan(j)).normalize();
                assert_eq!(b.dir(f, i, j), d, "face {f} ({i}, {j})");
                let (g, x, y) = b.locate(d);
                let [gn, gs, gt] = AX[g].map(|v| v.as_dvec3());
                let dn = d.dot(gn);
                let col = |w: f64| ((w / dn).atan() / FRAC_PI_2 * 2.0 + 1.0) * 0.5 * n;
                assert_eq!((x.to_bits(), y.to_bits()), (col(d.dot(gs)).to_bits(), col(d.dot(gt)).to_bits()), "face {f} ({i}, {j}) on face {g}");
            }
        }
    }

    /// The shader's `tx_unwarp` takes one fixed-point step and then WARP_STEPS = 3 Newton steps, no test of its
    /// own: written out here in f64, three leave under 0.005 of a column (a twelfth of a light texel) on the 4 km
    /// planet's face (3072 columns from its middle to an edge), its corners too; two do not (the start is up to
    /// about 110 columns off near a corner, and Newton crosses the table's cells on the way).
    #[test]
    fn the_shaders_three_newton_steps_suffice() {
        let h = 3072.0;
        let mut worst = [0.0f64; 6];
        let m = 400;
        for p in 0..=m {
            for q in 0..=m {
                // denser towards the corner (1, 1)
                let (a, b) = (1.0 - (p as f64 / m as f64).powi(2) * 2.0, 1.0 - (q as f64 / m as f64).powi(2) * 2.0);
                let (ta, tb) = super::warp(a, b, 1.0);
                let (v, _) = super::at(ta, tb);
                let (mut x, mut y) = (ta - v[0], tb - v[1]);
                for (k, w) in worst.iter_mut().enumerate() {
                    *w = w.max((x - a).abs().max((y - b).abs()) * h);
                    if k == 5 {
                        break;
                    }
                    let (v, d) = super::at(x, y);
                    let (ra, rb) = (x + v[0] - ta, y + v[1] - tb);
                    let (j00, j10, j01, j11) = (1.0 + d[0][0], d[0][1], d[1][0], 1.0 + d[1][1]);
                    let det = j00 * j11 - j01 * j10;
                    (x, y) = (x - (j11 * ra - j01 * rb) / det, y - (j00 * rb - j10 * ra) / det);
                }
            }
        }
        assert!(worst[3] < 0.005 && worst[2] > 0.005, "columns off after 0..5 Newton steps: {worst:?}");
    }

    /// The grid follows from the world's generation: from CONFORMAL every body's is warped, before it none.
    #[test]
    fn the_generation_chooses_the_grid() {
        use crate::world::{World, CONFORMAL, GENERATION};
        assert!(GENERATION >= CONFORMAL);
        for (gen, warp) in [(CONFORMAL - 1, 0.0), (CONFORMAL, 1.0)] {
            let w = World::with_grid(6144, crate::bodies::moon_columns(6144), 1, None, None, 0.0, gen);
            for b in w.bodies.iter() {
                assert_eq!(b.warp, warp, "generation {gen}: body {} has warp {}", b.name, b.warp);
            }
        }
    }

    /// On a generation-13 world every body's block tops are square at a face's middle, the middle of a face edge
    /// and a fifth of the way in from it (where the old grid has 1.09 x 0.77 and 1.09 x 0.96 m), the 4 km and
    /// 8 km worlds (Terra 6144 and 12288, Luna 3328 on the 8 km one, Mars, the icy moon).
    #[test]
    fn generation_13_block_tops_are_square() {
        use crate::world::{World, CONFORMAL};
        let mut sizes = Vec::new();
        for planet in [6144, 12288] {
            let w = World::with_grid(planet, crate::bodies::moon_columns(planet), 1, None, None, 0.0, CONFORMAL);
            for b in w.bodies.iter().filter(|b| !b.gas) {
                let n = b.n as f64;
                sizes.push(b.n);
                for f in 0..6 {
                    for (i, j) in [(n / 2.0, n / 2.0), (n - 1.0, n / 2.0), (0.0, n / 2.0), (n * 0.8, n / 2.0), (n / 2.0, n * 0.2)] {
                        let (u, v, ang) = block(b, f, i, j);
                        assert!((u / v - 1.0).abs() < 0.02 && (ang - 90.0).abs() < 1.0, "{} (n {}) face {f} ({i}, {j}): {u:.3} x {v:.3} m at {ang:.2} degrees", b.name, b.n);
                    }
                }
            }
        }
        assert!(sizes.contains(&3328) && sizes.contains(&12288), "{sizes:?}");
    }

    /// The table's derivatives are the bilinear's own, and `unwarp` inverts `warp` (to 1e-12 of a face).
    /// `value` is `at`'s first half to the bit, over the face, past its edges and at mirrored points.
    #[test]
    fn value_is_ats_value_to_the_bit() {
        for k in 0..20_000u32 {
            let f = |s: u32| (k.wrapping_mul(s) % 240_001) as f64 / 100_000.0 - 1.2;
            let (a, b) = (f(2_654_435_761), f(40_503));
            for (a, b) in [(a, b), (-a, b), (a, -b), (b, a)] {
                let (want, got) = (super::at(a, b).0, super::value(a, b));
                assert_eq!((got[0].to_bits(), got[1].to_bits()), (want[0].to_bits(), want[1].to_bits()), "at {a} {b}");
            }
        }
    }

    #[test]
    fn the_table_inverts() {
        for (a, b) in [(0.0, 0.0), (0.3, -0.7), (-0.99, 0.98), (0.999, 0.999), (1.0, -0.2), (-0.5, -1.0), (0.01, 0.6)] {
            let (x, y) = super::warp(a, b, 1.0);
            let (u, v) = super::unwarp(x, y, 1.0);
            assert!((u - a).abs() < 1e-12 && (v - b).abs() < 1e-12, "({a}, {b}) -> ({x}, {y}) -> ({u}, {v})");
            let (_, d) = super::at(a * 0.999 + 1e-4, b * 0.999 + 1e-4);
            let e = 1e-7;
            let (p, q) = (a * 0.999 + 1e-4, b * 0.999 + 1e-4);
            let fd = |k: usize, da: f64, db: f64| (super::at(p + da, q + db).0[k] - super::at(p - da, q - db).0[k]) / (2.0 * e);
            assert!((fd(0, e, 0.0) - d[0][0]).abs() < 1e-5 && (fd(1, e, 0.0) - d[0][1]).abs() < 1e-5, "d/da at ({p}, {q})");
            assert!((fd(0, 0.0, e) - d[1][0]).abs() < 1e-5 && (fd(1, 0.0, e) - d[1][1]).abs() < 1e-5, "d/db at ({p}, {q})");
        }
    }
}
