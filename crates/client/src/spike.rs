//! The see-through spike (prototypes/see-through, ADR 0068): three hard-coded still box models, painted at 32
//! pixels per metre, for the chunk mesh's model quads (engine `mesh::clutter`). Their face table and paintings
//! sit in the block textures' buffer past the layers the blocks take (shader.wgsl `MBASE`).
//! shortcut: hard-coded models and paintings, throwaway with the spike.
use glam::{EulerRot, Mat3, Vec3};

/// Where the face table starts, in u32s of the block textures' buffer: layer 1792 of 2048.
pub const BASE: usize = 1792 * 256;
/// Room for the face table (12 u32s a face); the paintings follow.
const TABLE: usize = 1024;
const W: usize = 64;

#[derive(Clone)]
struct Face {
    o: Vec3,
    u: Vec3,
    v: Vec3,
    /// what part it is, for its paint: 0 stone, 1 leaf, 2 stem, 3 cap side, 4 cap top
    part: u8,
}

/// Six faces of a box, corner and size in model pixels (1/32 m, x and y across, z up, from the middle of the
/// cell's floor), turned about its own middle by `rot` degrees; each face's u x v points out.
fn boxed(c: [f32; 3], s: [f32; 3], rot: [f32; 3], part: u8, top: u8) -> Vec<Face> {
    let (c, s) = (Vec3::from(c) / 32.0, Vec3::from(s) / 32.0);
    let r = Mat3::from_euler(EulerRot::XYZ, rot[0].to_radians(), rot[1].to_radians(), rot[2].to_radians());
    let mid = c + s * 0.5;
    let (x, y, z) = (Vec3::X * s.x, Vec3::Y * s.y, Vec3::Z * s.z);
    let faces = [(c + x, y, z, part), (c, z, y, part), (c + y, z, x, part), (c, x, z, part), (c + z, x, y, top), (c, y, x, part)];
    faces.into_iter().map(|(o, u, v, part)| Face { o: mid + r * (o - mid), u: r * u, v: r * v, part }).collect()
}

/// A plane w x h pixels standing up across the cell's middle, turned `deg` about the up axis: both its sides.
fn plane(w: f32, h: f32, deg: f32) -> Vec<Face> {
    let r = Mat3::from_rotation_z(deg.to_radians());
    let (u, v) = (Vec3::X * w / 32.0, Vec3::Z * h / 32.0);
    let o = -u * 0.5;
    vec![Face { o: r * o, u: r * u, v: r * v, part: 1 }, Face { o: r * (o + u), u: r * -u, v: r * v, part: 1 }]
}

fn models() -> [Vec<Face>; 3] {
    let pebble = [
        boxed([-4.0, -3.0, -0.5], [8.0, 6.0, 3.0], [0.0, 8.0, 15.0], 0, 0),
        boxed([1.0, -1.0, -0.5], [5.0, 4.0, 3.0], [-12.0, 0.0, 40.0], 0, 0),
        boxed([-6.0, 1.0, -0.5], [4.0, 3.0, 2.0], [10.0, -10.0, 70.0], 0, 0),
    ]
    .concat();
    let plant = [plane(16.0, 20.0, 45.0), plane(16.0, 20.0, -45.0)].concat();
    let mushroom = [
        boxed([-1.5, -1.5, 0.0], [3.0, 3.0, 7.0], [0.0; 3], 2, 2),
        boxed([-5.0, -5.0, 7.0], [10.0, 10.0, 2.0], [0.0; 3], 3, 4),
        boxed([-4.0, -4.0, 9.0], [8.0, 8.0, 1.0], [0.0; 3], 3, 4),
        boxed([-2.5, -2.5, 10.0], [5.0, 5.0, 1.0], [0.0; 3], 3, 4),
        boxed([3.0, 2.0, 0.0], [2.0, 2.0, 3.0], [0.0, 0.0, 20.0], 2, 2),
        boxed([2.0, 1.0, 3.0], [4.0, 4.0, 1.5], [0.0, 0.0, 20.0], 3, 4),
    ]
    .concat();
    [pebble, plant, mushroom]
}

fn hash(x: usize, y: usize, k: usize) -> u32 {
    let mut h = (x as u32).wrapping_mul(73856093) ^ (y as u32).wrapping_mul(19349663) ^ (k as u32).wrapping_mul(83492791);
    h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    h ^ (h >> 16)
}

fn rgba(r: u32, g: u32, b: u32) -> u32 {
    r | g << 8 | b << 16 | 255 << 24
}

/// Pixel (x, y) of a face w x h (y from the top) of part `part`, face `k`: 0 is empty.
fn paint(part: u8, k: usize, x: usize, y: usize, h: usize) -> u32 {
    let n = hash(x, y, k) % 32;
    match part {
        0 => rgba(118 + n, 116 + n, 110 + n),
        1 => {
            // five blades leaning out from the foot, two pixels wide, of different heights
            let up = (h - 1 - y) as f32 / h as f32;
            for b in 0..5 {
                let foot = 2.0 + b as f32 * 3.0;
                let lean = (b as f32 - 2.0) * 2.5;
                let tall = 0.55 + 0.45 * ((b * 7 + 3) % 5) as f32 / 4.0;
                let cx = foot + lean * up * up;
                if up <= tall && (x as f32 - cx).abs() < 1.0 + 0.6 * (1.0 - up) {
                    return rgba(60 + n, 120 + n * 2, 40 + n);
                }
            }
            0
        }
        2 => rgba(220 + n / 2, 212 + n / 2, 186 + n / 2),
        3 => rgba(170 + n, 40, 30),
        _ => {
            if hash(x / 2, y / 2, k + 99) % 8 == 0 {
                rgba(240, 236, 228)
            } else {
                rgba(180 + n, 44, 32)
            }
        }
    }
}

/// The face table and the paintings, to be written at `BASE`.
pub fn data() -> Vec<u32> {
    let mut out = vec![0u32; TABLE];
    let mut fi = 0;
    for (m, faces) in models().iter().enumerate() {
        let (_, first, n) = engine::mesh::MODEL_FACES[m];
        assert_eq!((first as usize, n as usize), (fi, faces.len()), "engine MODEL_FACES out of step with spike.rs");
        // shelf-packed in rows W wide
        let px = |v: Vec3| ((v.length() * 32.0).round() as usize).max(1);
        let (mut x, mut y, mut row) = (0, 0, 0);
        let mut at = Vec::new();
        for f in faces {
            let (w, h) = (px(f.u), px(f.v));
            if x + w > W {
                (x, y, row) = (0, y + row, 0);
            }
            at.push((x, y, w, h));
            x += w;
            row = row.max(h);
        }
        let base = BASE + out.len();
        let mut img = vec![0u32; W * (y + row)];
        for (k, (f, &(x0, y0, w, h))) in faces.iter().zip(&at).enumerate() {
            for yy in 0..h {
                for xx in 0..w {
                    img[(y0 + yy) * W + x0 + xx] = paint(f.part, k, xx, yy, h);
                }
            }
            let b = fi * 12;
            let fl = |v: Vec3| [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()];
            out[b..b + 3].copy_from_slice(&fl(f.o));
            out[b + 3] = base as u32;
            out[b + 4..b + 7].copy_from_slice(&fl(f.u));
            out[b + 7] = W as u32;
            out[b + 8..b + 11].copy_from_slice(&fl(f.v));
            out[b + 11] = x0 as u32 | (y0 as u32) << 16;
            fi += 1;
        }
        out.extend(img);
    }
    assert!(fi * 12 <= TABLE && BASE + out.len() <= 2048 * 256);
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_spike_models_fit_and_the_plant_has_holes() {
        let d = super::data();
        assert!(d.len() > super::TABLE);
        assert!(d[super::TABLE..].iter().any(|t| t >> 24 == 0), "the plant's painting has empty pixels");
    }
}
