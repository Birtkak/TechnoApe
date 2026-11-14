//! A voxel model as a small flat picture (ADR 0113): what the inventory draws an item, and the ape, with. Made
//! once per model off the frame thread (models.rs), drawn as rects (`hud::Paint`).
use engine::models::{Model, Painting};
use glam::{Mat3, Vec3};

/// An `n` x `n` picture as runs of one colour along its rows: (x, y, length, rgba).
#[derive(Clone, Debug, PartialEq)]
pub struct Sprite {
    pub n: u16,
    pub runs: Vec<(u16, u16, u16, [u8; 4])>,
}

impl Sprite {
    /// The colour at (x, y), None where it is empty.
    pub fn at(&self, x: u16, y: u16) -> Option<[u8; 4]> {
        self.runs.iter().find(|r| r.1 == y && (r.0..r.0 + r.2).contains(&x)).map(|r| r.3)
    }

    /// Drawn into a square of `size` at (x, y), each run one rect.
    pub fn draw(&self, rect: &mut dyn FnMut(f32, f32, f32, f32, [f32; 4]), x: f32, y: f32, size: f32) {
        let k = size / self.n as f32;
        for &(rx, ry, w, c) in &self.runs {
            rect(x + rx as f32 * k, y + ry as f32 * k, w as f32 * k, k, c.map(|v| v as f32 / 255.0));
        }
    }
}

/// `m` at rest seen from `yaw` degrees round its up axis and `pitch` degrees above, orthographic, fitted into
/// `n` x `n` pixels: per pixel the nearest face's colour, shaded by which way it looks. `paint` is its painting
/// (`data/models/<name>.png`) for its painted boxes. None when it has nothing to see.
pub fn sprite(m: &Model, paint: Option<&Painting>, n: u16, yaw: f32, pitch: f32) -> Option<Sprite> {
    // every face that can be seen, in the model's metres: corner, the two edges, a colour (None: the painting's
    // texels over `rect`)
    type Face = (Vec3, Vec3, Vec3, Option<[u8; 3]>, [u32; 4]);
    let pose = m.pose(None, 0.0);
    let mut faces: Vec<Face> = Vec::new();
    for (i, p) in m.parts.iter().enumerate() {
        let s = p.size.map(|v| v as i32);
        let get = |q: [i32; 3]| if (0..3).all(|a| (0..s[a]).contains(&q[a])) { p.cells[(q[0] + s[0] * (q[2] + s[2] * q[1])) as usize] } else { 0 };
        for (x, y, z) in (0..s[1]).flat_map(|y| (0..s[2]).flat_map(move |z| (0..s[0]).map(move |x| (x, y, z)))) {
            let c = get([x, y, z]);
            if c == 0 {
                continue;
            }
            for a in 0..3 {
                for sgn in [1, -1] {
                    let mut q = [x, y, z];
                    q[a] += sgn;
                    if get(q) != 0 {
                        continue;
                    }
                    // the face's corner and its two edges along the other axes
                    let mut o = Vec3::new(x as f32, y as f32, z as f32);
                    o[a] += (sgn > 0) as i32 as f32;
                    let (eu, ev) = (Vec3::AXES[(a + 1) % 3], Vec3::AXES[(a + 2) % 3]);
                    let t = pose[i];
                    faces.push((t.transform_point3(o), t.transform_vector3(eu), t.transform_vector3(ev), Some(m.rgb[c as usize - 1]), [0; 4]));
                }
            }
        }
    }
    if let Some(pt) = paint {
        for ([o, eu, ev], rect) in m.rest_faces() {
            if rect[0] + rect[2] <= pt.w && rect[1] + rect[3] <= pt.h {
                faces.push((o, eu, ev, None, rect));
            }
        }
    }
    // the view: turned by yaw about up, then tipped towards the eye by pitch; x right, y up, z towards the eye
    let view = Mat3::from_rotation_x(pitch.to_radians()) * Mat3::from_rotation_y(-yaw.to_radians());
    if faces.is_empty() {
        return None;
    }
    let corners = faces.iter().flat_map(|f| [f.0, f.0 + f.1, f.0 + f.2, f.0 + f.1 + f.2]).map(|p| view * p);
    let (lo, hi) = corners.fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), p| (lo.min(p), hi.max(p)));
    // fitted with a pixel's margin, centred
    let nf = n as f32;
    let k = (nf - 2.0) / (hi.x - lo.x).max(hi.y - lo.y).max(1e-6);
    let mid = (lo + hi) * 0.5;
    let to_px = |p: Vec3| (nf / 2.0 + (p.x - mid.x) * k, nf / 2.0 - (p.y - mid.y) * k, p.z);
    let light = Vec3::new(-0.35, 0.8, 0.5).normalize();
    let mut depth = vec![f32::MIN; n as usize * n as usize];
    let mut px = vec![[0u8; 4]; n as usize * n as usize];
    for &(o, eu, ev, colour, rect) in &faces {
        let (o, eu, ev) = (view * o, view * eu, view * ev);
        // a face lit by the light whichever side the eye sees (a painted plane shows both)
        let mut normal = eu.cross(ev).normalize_or_zero();
        if normal.z < 0.0 {
            normal = -normal;
        }
        let shade = 0.62 + 0.38 * normal.dot(light).max(0.0);
        // samples half a pixel apart or closer, so no pixel of the face is missed
        let steps = |e: Vec3| ((e.x * e.x + e.y * e.y).sqrt() * k * 2.0).ceil().max(1.0) as u32;
        let (su, sv) = (steps(eu), steps(ev));
        for j in 0..sv {
            for i in 0..su {
                let (u, v) = ((i as f32 + 0.5) / su as f32, (j as f32 + 0.5) / sv as f32);
                let rgb = match colour {
                    Some(c) => c,
                    None => {
                        let pt = paint.unwrap();
                        let (tx, ty) = (rect[0] + (u * rect[2] as f32) as u32, rect[1] + (v * rect[3] as f32) as u32);
                        let t = pt.px[(ty.min(pt.h - 1) * pt.w + tx.min(pt.w - 1)) as usize];
                        if t[3] == 0 {
                            continue;
                        }
                        [t[0], t[1], t[2]]
                    }
                };
                let (x, y, z) = to_px(o + eu * u + ev * v);
                let (xi, yi) = (x.floor() as i32, y.floor() as i32);
                if !(0..n as i32).contains(&xi) || !(0..n as i32).contains(&yi) {
                    continue;
                }
                let at = yi as usize * n as usize + xi as usize;
                if z > depth[at] {
                    depth[at] = z;
                    let c = rgb.map(|c| (c as f32 * shade).min(255.0) as u8);
                    px[at] = [c[0], c[1], c[2], 255];
                }
            }
        }
    }
    let mut runs = Vec::new();
    for y in 0..n {
        let row = &px[y as usize * n as usize..][..n as usize];
        let mut x = 0;
        while x < n {
            let c = row[x as usize];
            let w = row[x as usize..].iter().take_while(|o| **o == c).count() as u16;
            if c[3] > 0 {
                runs.push((x, y, w, c));
            }
            x += w;
        }
    }
    (!runs.is_empty()).then_some(Sprite { n, runs })
}

#[cfg(test)]
fn model(text: &str) -> Model {
    let palette = engine::palette::Palette::parse(include_str!("../../../data/palette.toml")).unwrap();
    Model::parse("test", text, &palette).unwrap()
}

#[test]
fn a_voxel_seen_from_the_front_fills_the_middle_in_its_colour_and_nothing_is_empty_model() {
    let m = model("colours = [\"copper\"]\n[[part]]\nname = \"a\"\nbox = [1, 1, 1]\nvoxels = \"a\"\n");
    let s = sprite(&m, None, 8, 0.0, 0.0).expect("a voxel is something to see");
    assert_eq!(s.n, 8);
    let c = s.at(4, 4).expect("the middle is the voxel's face");
    // its front face, a little shaded, in copper's hue: no channel brighter than copper's own
    let rgb = m.rgb[0];
    assert!((0..3).all(|k| c[k] <= rgb[k] && c[k] as f32 >= rgb[k] as f32 * 0.5), "{c:?} against {rgb:?}");
    assert_eq!(c[3], 255);
    assert_eq!(s.at(0, 0), None, "a margin round it");
    let empty = model("colours = [\"copper\"]\n[[part]]\nname = \"a\"\nbox = [1, 1, 1]\nvoxels = \".\"\n");
    assert_eq!(sprite(&empty, None, 8, 30.0, 30.0), None);
}
