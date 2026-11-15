//! The Models page's editing (docs/design/studio-tools.md "The Models page", ADR 0049): pure functions over a model,
//! tested apart from the window; the window only maps the pointer and keys to them. Cells are a part's voxels
//! (x, y up, z); the camera and rays are in the model's metres (the cell's edge is 1, its corner 0), as the game
//! draws it.
use crate::art::Img;
use engine::models::{encode, Clip, Ease, Event, Key, Life, Model, Part, Point, LETTERS, MAX_BOX, VOXEL};
use engine::palette::Palette;
use glam::{IVec3, Mat4, Vec3};

/// The orbit camera: it looks at `target` from `dist` away, turned `yaw` about up and raised `pitch` (radians).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Orbit {
    pub target: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub dist: f32,
}
impl Default for Orbit {
    fn default() -> Orbit {
        Orbit { target: Vec3::new(0.5, 0.4, 0.5), yaw: 0.6, pitch: 0.45, dist: 2.6 }
    }
}
impl Orbit {
    pub fn eye(&self) -> Vec3 {
        self.target + self.dist * Vec3::new(self.pitch.cos() * self.yaw.sin(), self.pitch.sin(), self.pitch.cos() * self.yaw.cos())
    }
    /// Forward, right and up of the view, as the game's camera makes them (its look with the cell's up).
    pub fn basis(&self) -> (Vec3, Vec3, Vec3) {
        let f = (self.target - self.eye()).normalize();
        let r = f.cross(Vec3::Y).normalize();
        (f, r, r.cross(f))
    }
    /// The ray through a point of the view (`ndc` -1..1 across and up); `fov` the vertical degrees.
    pub fn ray(&self, ndc: [f32; 2], fov: f32, aspect: f32) -> (Vec3, Vec3) {
        let (f, r, u) = self.basis();
        let th = (fov.to_radians() / 2.0).tan();
        (self.eye(), (f + r * ndc[0] * th * aspect + u * ndc[1] * th).normalize())
    }
    /// Where a point shows in the view (-1..1 across and up); none behind the eye.
    pub fn project(&self, p: Vec3, fov: f32, aspect: f32) -> Option<[f32; 2]> {
        let (f, r, u) = self.basis();
        let v = p - self.eye();
        let z = v.dot(f);
        if z < 0.02 {
            return None;
        }
        let th = (fov.to_radians() / 2.0).tan();
        Some([v.dot(r) / (z * th * aspect), v.dot(u) / (z * th)])
    }
    pub fn turn(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx;
        // (a Top or Bottom view stands past the drag's limit: a drag from there keeps it, never jumps)
        let lim = 1.45f32.max(self.pitch.abs());
        self.pitch = (self.pitch + dy).clamp(-lim, lim);
    }
    /// `turn`, about the point `p` instead of the target: `p` stays where it shows in the view.
    pub fn turn_about(&mut self, p: Vec3, dx: f32, dy: f32) {
        let (f, r, u) = self.basis();
        let v = self.target - p;
        let c = [v.dot(r), v.dot(u), v.dot(f)];
        self.turn(dx, dy);
        let (f, r, u) = self.basis();
        self.target = p + r * c[0] + u * c[1] + f * c[2];
    }
    /// `zoom` toward the point `p` (one in the plane of the target, under the pointer): `p` stays where it shows.
    pub fn zoom_about(&mut self, k: f32, p: Vec3) {
        let was = self.dist;
        self.zoom(k);
        self.target = p + (self.target - p) * (self.dist / was);
    }
    /// Looks from one of `VIEWS` at the same target.
    pub fn look_from(&mut self, view: usize) {
        let (_, _, yaw, pitch) = VIEWS[view.min(5)];
        (self.yaw, self.pitch) = (yaw, pitch);
    }
    /// Looks at the box `lo`..`hi` (metres) so it fills the view; `th` is the tangent of half the view's
    /// vertical field of view at the target's distance.
    pub fn frame(&mut self, lo: Vec3, hi: Vec3, th: f32) {
        self.target = (lo + hi) / 2.0;
        self.dist = ((hi - lo).length() / 2.0 * 1.6 / th.max(0.05)).clamp(0.3, 12.0);
    }
    pub fn zoom(&mut self, k: f32) {
        self.dist = (self.dist * k).clamp(0.3, 12.0);
    }
    /// Moves the target across the view by a share of the distance.
    pub fn pan(&mut self, dx: f32, dy: f32) {
        let (_, r, u) = self.basis();
        self.target += (-r * dx + u * dy) * self.dist;
    }
}

/// The six views: the view cube's face, the button's word, the yaw and pitch it looks from. Front looks from +z,
/// where a model faces in the hand and in the world; Top has the front at the bottom of the view.
pub const VIEWS: [(&str, &str, f32, f32); 6] = [
    ("front", "Front", 0.0, 0.0),
    ("back", "Back", std::f32::consts::PI, 0.0),
    ("right", "Right", std::f32::consts::FRAC_PI_2, 0.0),
    ("left", "Left", -std::f32::consts::FRAC_PI_2, 0.0),
    ("top", "Top", 0.0, 1.56),
    ("bottom", "Bottom", 0.0, -1.56),
];

/// What a ray meets: a voxel's face (`solid`), or the cell where it enters a part's box; `n` the face's outward
/// normal (zero from inside the box), `t` how far along the ray.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub part: usize,
    pub cell: IVec3,
    pub n: IVec3,
    pub solid: bool,
    pub t: f32,
}
impl Hit {
    /// The cell an add fills: beside the voxel met, or the empty cell entered.
    pub fn target(&self) -> IVec3 {
        if self.solid {
            self.cell + self.n
        } else {
            self.cell
        }
    }
}

fn size(p: &Part) -> IVec3 {
    IVec3::from(p.size.map(|v| v as i32))
}
fn idx(s: IVec3, c: IVec3) -> usize {
    (c.x + s.x * (c.z + s.z * c.y)) as usize
}
pub fn inside(p: &Part, c: IVec3) -> bool {
    c.cmpge(IVec3::ZERO).all() && c.cmplt(size(p)).all()
}
/// A part's voxel: 0 empty (also outside its box), else its colour's place + 1.
pub fn get(p: &Part, c: IVec3) -> u8 {
    if inside(p, c) {
        p.cells[idx(size(p), c)]
    } else {
        0
    }
}
fn set(p: &mut Part, c: IVec3, v: u8) -> bool {
    if !inside(p, c) || get(p, c) == v {
        return false;
    }
    let s = size(p);
    p.cells[idx(s, c)] = v;
    true
}

/// A ray into one part posed by `pose` (its voxels to the model's metres), stepped voxel by voxel.
fn pick_part(p: &Part, pose: Mat4, o: Vec3, d: Vec3) -> Option<Hit> {
    let inv = pose.inverse();
    let (o, d) = (inv.transform_point3(o), inv.transform_vector3(d));
    let s = size(p).as_vec3();
    let (mut t0, mut t1, mut axis) = (f32::NEG_INFINITY, f32::INFINITY, None);
    for a in 0..3 {
        if d[a].abs() < 1e-9 {
            if o[a] < 0.0 || o[a] > s[a] {
                return None;
            }
            continue;
        }
        let (mut u, mut v) = (-o[a] / d[a], (s[a] - o[a]) / d[a]);
        if u > v {
            std::mem::swap(&mut u, &mut v);
        }
        if u > t0 {
            (t0, axis) = (u, Some(a));
        }
        t1 = t1.min(v);
    }
    if t1 < t0.max(0.0) {
        return None;
    }
    let t = t0.max(0.0);
    let mut n = IVec3::ZERO;
    if let (true, Some(a)) = (t0 > 0.0, axis) {
        n[a] = if d[a] > 0.0 { -1 } else { 1 };
    }
    let mut c = (o + d * t + d.normalize() * 1e-4).floor().as_ivec3().clamp(IVec3::ZERO, size(p) - 1);
    let first = (c, n);
    let step = IVec3::new(d.x.signum() as i32, d.y.signum() as i32, d.z.signum() as i32);
    let (mut tmax, mut delta) = (Vec3::INFINITY, Vec3::INFINITY);
    for a in 0..3 {
        if d[a].abs() >= 1e-9 {
            let edge = c[a] as f32 + if step[a] > 0 { 1.0 } else { 0.0 };
            tmax[a] = (edge - o[a]) / d[a];
            delta[a] = 1.0 / d[a].abs();
        }
    }
    let mut tt = t;
    loop {
        if get(p, c) != 0 {
            return Some(Hit { part: 0, cell: c, n, solid: true, t: tt });
        }
        let a = if tmax.x <= tmax.y && tmax.x <= tmax.z { 0 } else if tmax.y <= tmax.z { 1 } else { 2 };
        tt = tmax[a];
        c[a] += step[a];
        n = IVec3::ZERO;
        n[a] = -step[a];
        tmax[a] += delta[a];
        if !inside(p, c) {
            break;
        }
    }
    Some(Hit { part: 0, cell: first.0, n: first.1, solid: false, t })
}

/// The voxel face under a ray (in the model's metres, `poses` from `Model::pose`): the nearest voxel of any part,
/// or of `only`; missing every voxel, where it enters the box of `only` (or the nearest box).
pub fn pick(m: &Model, poses: &[Mat4], only: Option<usize>, o: Vec3, d: Vec3) -> Option<Hit> {
    let hits: Vec<Hit> = (0..m.parts.len())
        .filter(|&i| only.is_none_or(|k| k == i))
        .filter_map(|i| pick_part(&m.parts[i], poses[i], o, d).map(|h| Hit { part: i, ..h }))
        .collect();
    let best = |solid: bool| hits.iter().filter(|h| h.solid == solid).min_by(|a, b| a.t.total_cmp(&b.t)).copied();
    best(true).or_else(|| best(false))
}

/// Where a ray meets the plane at the foot of part `p`'s row `layer` (its cells, posed by `pose`): the cell of that
/// row it lands in (`n` up, `solid` whether a voxel is there), whatever stands above. Layer mode's clicks, and the
/// floor's (row 0) where no voxel is under the pointer. None: the ray runs along the plane, away from it, or lands
/// farther from the box than a part may grow.
pub fn pick_layer(p: &Part, pose: Mat4, layer: i32, o: Vec3, d: Vec3) -> Option<Hit> {
    let inv = pose.inverse();
    let (o, d) = (inv.transform_point3(o), inv.transform_vector3(d));
    if d.y.abs() < 1e-6 {
        return None;
    }
    let t = (layer as f32 - o.y) / d.y;
    if t < 0.0 {
        return None;
    }
    let at = o + d * t;
    let cell = IVec3::new(at.x.floor() as i32, layer, at.z.floor() as i32);
    let reach = MAX_BOX as i32;
    if cell.cmplt(IVec3::splat(-reach)).any() || cell.cmpge(size(p) + reach).any() {
        return None;
    }
    // (t in the posed ray's units: the pose is a scale and a turn, so the order of hits along it is kept)
    Some(Hit { part: 0, cell, n: IVec3::Y, solid: get(p, cell) != 0, t })
}

/// The six face normals, in the order `exposed` numbers them.
pub const NORMALS: [IVec3; 6] = [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z];

/// The faces of part `p`'s voxels with no voxel beside them: the cell and its `NORMALS` index.
pub fn exposed(p: &Part) -> Vec<(IVec3, usize)> {
    let s = size(p);
    let mut out = vec![];
    for y in 0..s.y {
        for z in 0..s.z {
            for x in 0..s.x {
                let c = IVec3::new(x, y, z);
                if get(p, c) != 0 {
                    out.extend((0..6).filter(|&k| get(p, c + NORMALS[k]) == 0).map(|k| (c, k)));
                }
            }
        }
    }
    out
}

/// A cell's face toward `n`, its four corners in the cell's units, round its edge.
pub fn face_quad(cell: IVec3, n: IVec3) -> [Vec3; 4] {
    let a = (0..3).find(|&k| n[k] != 0).unwrap_or(1);
    let (u, v) = ((a + 1) % 3, (a + 2) % 3);
    let mut base = cell.as_vec3();
    if n[a] > 0 {
        base[a] += 1.0;
    }
    let (mut du, mut dv) = (Vec3::ZERO, Vec3::ZERO);
    du[u] = 1.0;
    dv[v] = 1.0;
    [base, base + du, base + du + dv, base + dv]
}

/// Box, line and fill work on the plane of the face first clicked: where a ray meets that layer of cells.
pub fn on_plane(pose: Mat4, first: &Hit, o: Vec3, d: Vec3) -> Option<IVec3> {
    let inv = pose.inverse();
    let (o, d) = (inv.transform_point3(o), inv.transform_vector3(d));
    let at = first.target();
    let a = (0..3).find(|&a| first.n[a] != 0).unwrap_or(1);
    if d[a].abs() < 1e-6 {
        return None;
    }
    let t = (at[a] as f32 + 0.5 - o[a]) / d[a];
    if t < 0.0 {
        return None;
    }
    let mut c = (o + d * t).floor().as_ivec3();
    c[a] = at[a];
    Some(c)
}

/// Grows part `i`'s box to hold `c` (at most `MAX_BOX` a side). A box grown below 0 moves its corner, pivot,
/// children and points with it, so nothing moves in the cell. The shift added to its cells; none: too big.
pub fn grow(m: &mut Model, i: usize, c: IVec3) -> Option<IVec3> {
    let s = size(&m.parts[i]);
    let (lo, hi) = (c.min(IVec3::ZERO), (c + 1).max(s));
    let new = hi - lo;
    if new.cmpgt(IVec3::splat(MAX_BOX as i32)).any() {
        return None;
    }
    if new == s {
        return Some(IVec3::ZERO);
    }
    let shift = -lo;
    let p = &mut m.parts[i];
    let mut cells = vec![0u8; (new.x * new.y * new.z) as usize];
    for y in 0..s.y {
        for z in 0..s.z {
            for x in 0..s.x {
                let at = IVec3::new(x, y, z);
                cells[idx(new, at + shift)] = p.cells[idx(s, at)];
            }
        }
    }
    p.cells = cells;
    p.size = new.as_uvec3().to_array();
    p.at = (IVec3::from(p.at) - shift).to_array();
    let f = shift.as_vec3();
    p.pivot = (Vec3::from(p.pivot) + f).to_array();
    let name = p.name.clone();
    for q in m.parts.iter_mut().filter(|q| q.parent == name) {
        q.at = (IVec3::from(q.at) + shift).to_array();
    }
    for pt in m.points.iter_mut().filter(|pt| pt.part == name) {
        pt.at = (Vec3::from(pt.at) + f).to_array();
    }
    Some(shift)
}

/// What a tool does to its cells.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Op {
    /// Fills with a colour's place + 1, growing the box.
    Add(u8),
    Remove,
    /// Recolours voxels that are there.
    Paint(u8),
}

/// `op` on cells of part `i` (given before any growing); whether anything changed.
pub fn apply(m: &mut Model, i: usize, cells: &[IVec3], op: Op) -> bool {
    let (mut shift, mut changed) = (IVec3::ZERO, false);
    for &c in cells {
        let c = c + shift;
        changed |= match op {
            Op::Add(k) => match grow(m, i, c) {
                Some(s) => {
                    shift += s;
                    set(&mut m.parts[i], c + s, k)
                }
                None => false,
            },
            Op::Remove => set(&mut m.parts[i], c, 0),
            Op::Paint(k) => get(&m.parts[i], c) != 0 && set(&mut m.parts[i], c, k),
        };
    }
    changed
}

/// The cells with their twins across the box's middle in x and in z.
pub fn mirrored(size: [u32; 3], cells: &[IVec3], mx: bool, mz: bool) -> Vec<IVec3> {
    let (sx, sz) = (size[0] as i32, size[2] as i32);
    let mut out: Vec<IVec3> = vec![];
    for &c in cells {
        let fx = IVec3::new(sx - 1 - c.x, c.y, c.z);
        let fz = IVec3::new(c.x, c.y, sz - 1 - c.z);
        let both = IVec3::new(sx - 1 - c.x, c.y, sz - 1 - c.z);
        for (on, v) in [(true, c), (mx, fx), (mz, fz), (mx && mz, both)] {
            if on && !out.contains(&v) {
                out.push(v);
            }
        }
    }
    out
}

/// Every cell of the box between two corners.
pub fn rect(a: IVec3, b: IVec3) -> Vec<IVec3> {
    let (lo, hi) = (a.min(b), a.max(b));
    let mut v = vec![];
    for y in lo.y..=hi.y {
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                v.push(IVec3::new(x, y, z));
            }
        }
    }
    v
}

/// The cells of a straight line, each touching the last.
pub fn line(a: IVec3, b: IVec3) -> Vec<IVec3> {
    let d = b - a;
    let n = d.abs().max_element().max(1);
    let mut v: Vec<IVec3> = (0..=n).map(|k| a + (d.as_vec3() * k as f32 / n as f32).round().as_ivec3()).collect();
    v.dedup();
    v
}

/// The cells of `start`'s layer across `axis` joined to it with its value (a voxel's colour, or empty).
pub fn fill_plane(p: &Part, start: IVec3, axis: usize) -> Vec<IVec3> {
    if !inside(p, start) {
        return vec![];
    }
    let want = get(p, start);
    let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
    let mut seen = vec![start];
    let mut todo = vec![start];
    while let Some(c) = todo.pop() {
        for (a, s) in [(u, 1), (u, -1), (v, 1), (v, -1)] {
            let mut n = c;
            n[a] += s;
            if inside(p, n) && get(p, n) == want && !seen.contains(&n) {
                seen.push(n);
                todo.push(n);
            }
        }
    }
    seen
}

/// The model with every voxel whose middle stands above `top` (the model's voxels, at rest) hidden: the slice.
pub fn sliced(m: &Model, poses: &[Mat4], top: f32) -> Model {
    let mut out = m.clone();
    for (i, p) in out.parts.iter_mut().enumerate() {
        let s = size(p);
        for y in 0..s.y {
            for z in 0..s.z {
                for x in 0..s.x {
                    let c = IVec3::new(x, y, z);
                    if poses[i].transform_point3(c.as_vec3() + 0.5).y / VOXEL > top {
                        p.cells[idx(s, c)] = 0;
                    }
                }
            }
        }
    }
    out
}

/// Moves the voxels of a selection (corners `a`, `b`) by `by`; what would leave the box is dropped.
/// (shortcut: a move does not grow the box; grow it with an add first when that is wanted)
pub fn move_sel(m: &mut Model, i: usize, a: IVec3, b: IVec3, by: IVec3) -> bool {
    let p = &mut m.parts[i];
    let held: Vec<(IVec3, u8)> = rect(a, b).into_iter().map(|c| (c, get(p, c))).filter(|(_, v)| *v != 0).collect();
    let mut changed = false;
    for (c, _) in &held {
        changed |= set(p, *c, 0);
    }
    for (c, v) in held {
        changed |= set(p, c + by, v);
    }
    changed
}

/// Pulls the selection's face on side `n` out by `k` layers (its voxels copied outwards, the box grown to hold
/// them); the selection grown over them.
pub fn extrude(m: &mut Model, i: usize, a: IVec3, b: IVec3, n: IVec3, k: i32) -> (IVec3, IVec3) {
    let (mut lo, mut hi) = (a.min(b), a.max(b));
    let Some(ax) = (0..3).find(|&x| n[x] != 0) else { return (lo, hi) };
    let face = if n[ax] > 0 { hi[ax] } else { lo[ax] };
    let layer: Vec<(IVec3, u8)> = rect(lo, hi).into_iter().filter(|c| c[ax] == face).map(|c| (c, get(&m.parts[i], c))).filter(|(_, v)| *v != 0).collect();
    let mut shift = IVec3::ZERO;
    for step in 1..=k.max(0) {
        for &(c, v) in &layer {
            let at = c + n * step + shift;
            if let Some(s) = grow(m, i, at) {
                shift += s;
                set(&mut m.parts[i], at + s, v);
            }
        }
    }
    let mut end = if n[ax] > 0 { hi } else { lo };
    end[ax] += n[ax] * k.max(0);
    (lo, hi) = (lo.min(end) + shift, hi.max(end) + shift);
    (lo, hi)
}

/// The place + 1 of a palette colour in the model's colours, added when missing; none: not a palette colour, or
/// the model has all 62.
pub fn colour_index(m: &mut Model, pal: &Palette, rgb: [u8; 3]) -> Option<u8> {
    if let Some(k) = m.rgb.iter().position(|&c| c == rgb) {
        return Some(k as u8 + 1);
    }
    let c = &pal.colours[pal.find(rgb)?];
    if m.colours.len() >= LETTERS.len() {
        return None;
    }
    m.colours.push(c.name.clone());
    m.rgb.push(rgb);
    Some(m.colours.len() as u8)
}

/// Shade by light: every voxel stepped along its ramp by how open it is (of its 26 neighbours, how many are
/// empty, against the 9 a flat face has), lighter with the sky above it, darker over an empty underside; `k` the
/// strength, at most two steps. Voxels on no ramp keep their colour. How many changed.
pub fn shade_by_light(m: &mut Model, pal: &Palette, only: Option<usize>, k: f32) -> usize {
    let mut changes: Vec<(usize, IVec3, [u8; 3])> = vec![];
    for (i, p) in m.parts.iter().enumerate().filter(|(i, _)| only.is_none_or(|o| o == *i)) {
        let s = size(p);
        for c in rect(IVec3::ZERO, s - 1) {
            let v = get(p, c);
            if v == 0 {
                continue;
            }
            let open = rect(IVec3::splat(-1), IVec3::ONE).into_iter().filter(|d| *d != IVec3::ZERO && get(p, c + *d) == 0).count();
            if open == 0 {
                continue;
            }
            let (up, down) = (get(p, c + IVec3::Y) == 0, get(p, c - IVec3::Y) == 0);
            let score = (open as f32 - 9.0) / 26.0 + if up { 0.35 } else { 0.0 } - if down && !up { 0.35 } else { 0.0 };
            let steps = (score * k).round().clamp(-2.0, 2.0) as i32;
            let rgb = m.rgb[v as usize - 1];
            if let Some(to) = pal.step(rgb, steps).filter(|t| *t != rgb) {
                changes.push((i, c, to));
            }
        }
    }
    let mut n = 0;
    for (i, c, to) in changes {
        if let Some(v) = colour_index(m, pal, to) {
            n += set(&mut m.parts[i], c, v) as usize;
        }
    }
    n
}

/// The palette colour nearest a colour.
fn nearest(pal: &Palette, c: [u8; 3]) -> [u8; 3] {
    let d = |a: [u8; 3]| (0..3).map(|k| (a[k] as i32 - c[k] as i32).pow(2)).sum::<i32>();
    pal.colours.iter().map(|p| p.rgb).min_by_key(|&p| d(p)).unwrap_or(c)
}

fn unique<'a>(names: impl Iterator<Item = &'a str> + Clone, base: &str) -> String {
    (1..).map(|k| if k == 1 { base.to_string() } else { format!("{base} {k}") }).find(|n| !names.clone().any(|m| m == n)).unwrap()
}

/// From a texture: a block's painted faces (top, side, bottom; a missing one falls back to the side, then the
/// top) become a new 8-voxel cube to carve; each voxel takes the face it is nearest, the commoner colour of its
/// 2 x 2 pixels there. The new part's place; none with nothing painted.
/// (shortcut: which way a side's pixels run is this function's own choice, not checked against the game's texel
/// turn of ADR 0047; check it when a carved block is set beside the real one)
pub fn from_texture(m: &mut Model, pal: &Palette, faces: [Option<&Img>; 3]) -> Option<usize> {
    let top = faces[0].or(faces[1])?;
    let side = faces[1].unwrap_or(top);
    let bottom = faces[2].unwrap_or(side);
    let mut cells = vec![0u8; 512];
    let s = IVec3::splat(8);
    for c in rect(IVec3::ZERO, s - 1) {
        let (x, y, z) = (c.x, c.y, c.z);
        // nearest face first, top first on a tie: (distance, image, u, v)
        let opts = [(7 - y, top, x, z), (x, side, z, 7 - y), (7 - x, side, 7 - z, 7 - y), (z, side, 7 - x, 7 - y), (7 - z, side, x, 7 - y), (y, bottom, x, 7 - z)];
        let (_, img, u, v) = *opts.iter().min_by_key(|o| o.0).unwrap();
        let px: Vec<[u8; 3]> = [(0, 0), (1, 0), (0, 1), (1, 1)].iter().map(|(a, b)| img.get(0, (u * 2 + a) as usize, (v * 2 + b) as usize)).collect();
        let common = *px.iter().max_by_key(|p| (px.iter().filter(|q| q == p).count(), std::cmp::Reverse(px.iter().position(|q| q == *p)))).unwrap();
        cells[idx(s, c)] = colour_index(m, pal, nearest(pal, common)).unwrap_or(1);
    }
    let name = unique(m.parts.iter().map(|p| p.name.as_str()), "block");
    m.parts.push(Part { name, parent: String::new(), at: [0; 3], pivot: [4.0, 0.0, 4.0], size: [8, 8, 8], voxels: String::new(), turn: [0.0; 3], boxes: vec![], cells });
    m.up.push(None);
    Some(m.parts.len() - 1)
}

/// Sets a part's key at `t` in a clip: the key there changed, or a new one (eased smoothly) put in order.
pub fn set_key(c: &mut Clip, part: &str, t: f32, shift: [f32; 3], turn: [f32; 3]) {
    let t = t.clamp(0.0, c.length);
    match c.key.iter_mut().find(|k| k.part == part && (k.t - t).abs() < 1e-3) {
        Some(k) => (k.shift, k.turn) = (shift, turn),
        None => c.key.push(Key { part: part.into(), t, shift, turn, ease: Ease::Smooth }),
    }
    c.key.sort_by(|a, b| a.part.cmp(&b.part).then(a.t.total_cmp(&b.t)));
}

/// The ready-made motions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Spin,
    Swing,
    Bob,
    Shake,
    OpenShut,
}
/// Each motion, its name, what it does, and its amount's default and unit.
pub const MOTIONS: [(Motion, &str, &str, f32, &str); 5] = [
    (Motion::Spin, "Spin", "turns about the pivot once a clip", 360.0, "°"),
    (Motion::Swing, "Swing", "swings to either side and back", 30.0, "°"),
    (Motion::Bob, "Bob", "rises and falls", 1.0, "vox"),
    (Motion::Shake, "Shake", "shakes to and fro", 0.5, "vox"),
    (Motion::OpenShut, "Open and shut", "opens, holds, shuts", 90.0, "°"),
];

/// Fills a part's keys in a clip with a ready-made motion about or along `axis` (0 x, 1 y, 2 z) by `amount`
/// (degrees, or voxels for bob and shake); the part's other keys there go, the clip loops; then they are ordinary
/// keys.
pub fn motion(c: &mut Clip, part: &str, kind: Motion, axis: usize, amount: f32) {
    c.key.retain(|k| k.part != part);
    c.looping = true;
    let l = c.length;
    let v = |x: f32| {
        let mut a = [0.0; 3];
        a[axis] = x;
        a
    };
    let z = [0.0; 3];
    use Ease::*;
    // (t, move, turn, ease)
    let keys: Vec<(f32, [f32; 3], [f32; 3], Ease)> = match kind {
        Motion::Spin => vec![(0.0, z, z, Linear), (l, z, v(amount), Linear)],
        Motion::Swing => vec![(0.0, z, v(-amount), Smooth), (l / 2.0, z, v(amount), Smooth), (l, z, v(-amount), Smooth)],
        Motion::Bob => vec![(0.0, z, z, Smooth), (l / 2.0, v(amount), z, Smooth), (l, z, z, Smooth)],
        Motion::Shake => (0..=8).map(|i| (l * i as f32 / 8.0, if i == 0 || i == 8 { z } else { v(if i % 2 == 1 { amount } else { -amount }) }, z, Linear)).collect(),
        Motion::OpenShut => vec![(0.0, z, z, Smooth), (0.4 * l, z, v(amount), Linear), (0.6 * l, z, v(amount), Smooth), (l, z, z, Linear)],
    };
    c.key.extend(keys.into_iter().map(|(t, shift, turn, ease)| Key { part: part.into(), t, shift, turn, ease }));
    c.key.sort_by(|a, b| a.part.cmp(&b.part).then(a.t.total_cmp(&b.t)));
}

/// A new empty part (4 voxels a side) under `parent`, or a root; its place.
pub fn add_part(m: &mut Model, parent: Option<usize>) -> usize {
    let name = unique(m.parts.iter().map(|p| p.name.as_str()), "part");
    let parent_name = parent.map_or(String::new(), |k| m.parts[k].name.clone());
    m.parts.push(Part { name, parent: parent_name, at: [0; 3], pivot: [0.0; 3], size: [4, 4, 4], voxels: String::new(), turn: [0.0; 3], boxes: vec![], cells: vec![0; 64] });
    m.up.push(parent);
    m.parts.len() - 1
}

/// Renames a part and every mention of it (children, points, keys).
pub fn rename_part(m: &mut Model, i: usize, new: &str) -> Result<(), String> {
    let new = new.trim();
    if new.is_empty() || m.parts.iter().enumerate().any(|(k, p)| k != i && p.name == new) {
        return Err(format!("'{new}' is empty or another part's name"));
    }
    let old = std::mem::replace(&mut m.parts[i].name, new.to_string());
    for p in m.parts.iter_mut().filter(|p| p.parent == old) {
        p.parent = new.into();
    }
    for p in m.points.iter_mut().filter(|p| p.part == old) {
        p.part = new.into();
    }
    for k in m.clips.iter_mut().flat_map(|c| c.key.iter_mut()).filter(|k| k.part == old) {
        k.part = new.into();
    }
    Ok(())
}

/// Puts a part under another (or makes it a root); refused when the new parent is the part or one of its own.
pub fn reparent(m: &mut Model, i: usize, parent: Option<usize>) -> Result<(), String> {
    let mut at = parent;
    while let Some(k) = at {
        if k == i {
            return Err(format!("'{}' cannot hang under itself or its own parts", m.parts[i].name));
        }
        at = m.up[k];
    }
    m.parts[i].parent = parent.map_or(String::new(), |k| m.parts[k].name.clone());
    m.up[i] = parent;
    Ok(())
}

/// A new point on a part at `at` (its voxels); its place.
pub fn add_point(m: &mut Model, part: usize, at: [f32; 3]) -> usize {
    let name = unique(m.points.iter().map(|p| p.name.as_str()), "point");
    m.points.push(Point { name, part: m.parts[part].name.clone(), at });
    m.points.len() - 1
}

/// The file's text, the voxels written from the cells.
pub fn text(m: &Model) -> String {
    let mut m = m.clone();
    for p in &mut m.parts {
        p.voxels = encode(p.size, &p.cells);
    }
    m.write()
}

/// The file's text with the model's life (its states and clip events, ADR 0050).
pub fn text_life(m: &Model, life: &Life) -> String {
    text_keeping("", m, life)
}

/// `text_life` written over `old` (the file as opened): its comments and line ends kept.
pub fn text_keeping(old: &str, m: &Model, life: &Life) -> String {
    let mut m = m.clone();
    for p in &mut m.parts {
        p.voxels = encode(p.size, &p.cells);
    }
    engine::models::write_keeping(old, &m, life)
}

/// Renames a point and every state and event that names it.
pub fn rename_point(m: &mut Model, life: &mut Life, i: usize, new: &str) -> Result<(), String> {
    let new = new.trim();
    if new.is_empty() || m.points.iter().enumerate().any(|(k, p)| k != i && p.name == new) {
        return Err(format!("'{new}' is empty or another point's name"));
    }
    let old = std::mem::replace(&mut m.points[i].name, new.to_string());
    let fix = |p: &mut String| {
        if *p == old {
            *p = new.into();
        }
    };
    for s in &mut life.states {
        s.effects.iter_mut().for_each(|e| fix(&mut e.point));
        s.light.iter_mut().for_each(|l| fix(&mut l.point));
    }
    life.events.iter_mut().flat_map(|(_, ev)| ev.iter_mut()).filter_map(|e| e.point.as_mut()).for_each(fix);
    Ok(())
}

/// Renames a clip and every state and event list that names it.
pub fn rename_clip(m: &mut Model, life: &mut Life, i: usize, new: &str) -> Result<(), String> {
    let new = new.trim();
    if new.is_empty() || m.clips.iter().enumerate().any(|(k, c)| k != i && c.name == new) {
        return Err(format!("'{new}' is empty or another clip's name"));
    }
    let old = std::mem::replace(&mut m.clips[i].name, new.to_string());
    life.states.iter_mut().filter_map(|s| s.clip.as_mut()).filter(|c| **c == old).for_each(|c| *c = new.into());
    life.events.iter_mut().filter(|(c, _)| *c == old).for_each(|(c, _)| *c = new.into());
    Ok(())
}

/// The life kept true to the model after any edit: an event stays inside its clip (a shortened clip pulls it in),
/// and what names a removed clip or point is cleared (a state's clip, effect or light; an event's point; the
/// events of a gone clip; an event left with nothing to play).
pub fn fit_life(m: &Model, life: &mut Life) {
    let point = |n: &str| m.points.iter().any(|p| p.name == n);
    let clip = |n: &str| m.clips.iter().find(|c| c.name == n);
    for s in &mut life.states {
        if s.clip.as_deref().is_some_and(|c| clip(c).is_none()) {
            s.clip = None;
        }
        s.effects.retain(|e| point(&e.point));
        if s.light.as_ref().is_some_and(|l| !point(&l.point)) {
            s.light = None;
        }
    }
    life.events.retain(|(c, _)| clip(c).is_some());
    for (c, ev) in &mut life.events {
        let len = clip(c).unwrap().length;
        for e in ev.iter_mut() {
            e.t = e.t.clamp(0.0, len);
            if e.point.as_deref().is_some_and(|p| !point(p)) {
                e.point = None;
            }
        }
        ev.retain(|e| e.sound.is_some() || e.effect.is_some());
    }
    life.events.retain(|(_, ev)| !ev.is_empty());
}

/// A clip's events, made when it has none.
pub fn events_mut<'a>(life: &'a mut Life, clip: &str) -> &'a mut Vec<Event> {
    match life.events.iter().position(|(c, _)| c == clip) {
        Some(k) => &mut life.events[k].1,
        None => {
            life.events.push((clip.to_string(), vec![]));
            &mut life.events.last_mut().unwrap().1
        }
    }
}

/// The starter shapes a new model begins from (docs/design/studio.md "The bar"): id, the button's word, its tip.
pub const STARTERS: [(&str, &str, &str); 5] = [
    ("block", "Block", "a whole 1 m block, 8 x 8 x 8 voxels, to carve and recolour"),
    ("slab", "Slab", "half a block: 8 x 4 x 8 voxels"),
    ("post", "Post", "a post 2 voxels thick and a block high, in the block's middle"),
    ("tool", "Tool", "a handle with a head on top, as a pick or a hammer: two parts, the head hanging on the handle"),
    ("creature", "Creature", "a body, a head and four legs, each a part with its pivot where it turns, facing front (+z)"),
];

/// A part of `size` whose cells are `fill`'s colours (a colour's place + 1, 0 for none).
fn filled(name: &str, parent: &str, at: [i32; 3], size: [u32; 3], pivot: [f32; 3], fill: impl Fn(IVec3) -> u8) -> Part {
    let s = IVec3::from(size.map(|v| v as i32));
    let mut cells = vec![0u8; (s.x * s.y * s.z) as usize];
    for y in 0..s.y {
        for z in 0..s.z {
            for x in 0..s.x {
                let c = IVec3::new(x, y, z);
                cells[idx(s, c)] = fill(c);
            }
        }
    }
    Part { name: name.into(), parent: parent.into(), at, pivot, size, voxels: String::new(), turn: [0.0; 3], boxes: vec![], cells }
}

/// A new model of a starter shape: its parts and pivots set, its colours the palette's by name (one the palette
/// lacks falls to its first); it stands in the block at the origin. None: no such shape.
pub fn starter(pal: &Palette, kind: &str) -> Option<Model> {
    let mut colours: Vec<String> = vec![];
    let mut col = |name: &str| -> u8 {
        let name = pal.colours.iter().find(|c| c.name == name).or(pal.colours.first()).map_or(name.to_string(), |c| c.name.clone());
        (colours.iter().position(|c| *c == name).unwrap_or_else(|| {
            colours.push(name);
            colours.len() - 1
        }) + 1) as u8
    };
    let parts = match kind {
        "block" | "slab" => {
            let (side, top) = (col("stone"), col("stone light"));
            let h = if kind == "block" { 8 } else { 4 };
            vec![filled(kind, "", [0; 3], [8, h, 8], [4.0, 0.0, 4.0], |c| if c.y == h as i32 - 1 { top } else { side })]
        }
        "post" => {
            let wood = col("dirt");
            vec![filled("post", "", [3, 0, 3], [2, 8, 2], [1.0, 0.0, 1.0], |_| wood)]
        }
        "tool" => {
            let (wood, head) = (col("dirt dark"), col("stone"));
            vec![filled("handle", "", [4, 0, 4], [1, 7, 1], [0.5, 0.0, 0.5], |_| wood), filled("head", "handle", [-2, 6, 0], [5, 2, 1], [2.5, 1.0, 0.5], |_| head)]
        }
        "creature" => {
            let (body, head, leg, eye) = (col("dirt"), col("dirt light"), col("dirt dark"), col("stone dark"));
            let mut v = vec![
                filled("body", "", [2, 2, 0], [4, 3, 5], [2.0, 1.5, 2.5], |_| body),
                filled("head", "body", [0, 0, 5], [4, 3, 3], [2.0, 1.5, 0.0], |c| if c.z == 2 && c.y == 2 && (c.x == 0 || c.x == 3) { eye } else { head }),
            ];
            // its left is +x: it faces +z
            for (name, at) in [("leg_fl", [3, -2, 3]), ("leg_fr", [0, -2, 3]), ("leg_bl", [3, -2, 0]), ("leg_br", [0, -2, 0])] {
                v.push(filled(name, "body", at, [1, 2, 1], [0.5, 2.0, 0.5], |_| leg));
            }
            v
        }
        _ => return None,
    };
    let m = Model { colours, item: None, parts, points: vec![], clips: vec![], rgb: vec![], up: vec![] };
    Model::parse(kind, &text(&m), pal).ok()
}

/// The longest side of a painted box the game draws, model pixels (a quad's 6-bit size, ADR 0116).
pub const BOX_SIDE: u32 = 63;

/// A painted box of `size` px added to part `part` at its corner, its net laid below the painting (which grows to
/// hold it, at most `MAX_PAINT` a side) and filled with `c`, so it shows at once.
pub fn add_box(m: &mut Model, paint: &mut Img, part: usize, size: [u32; 3], c: [u8; 3]) -> Result<(), String> {
    if let Some(v) = size.iter().find(|&&v| v > BOX_SIDE) {
        return Err(format!("a box side of {v} px: the game draws at most {BOX_SIDE}"));
    }
    let [sx, sy, sz] = size;
    let (w, h) = (paint.w.max((2 * (sx + sz)) as usize), paint.h + (sz + sy) as usize);
    let max = engine::models::MAX_PAINT as usize;
    if w > max || h > max {
        return Err(format!("the painting would be {w} x {h} px, at most {max} a side"));
    }
    let b = engine::models::Bx { at: [0.0; 3], size, turn: [0.0; 3], uv: [0, paint.h as u32] };
    let p = m.parts.get_mut(part).ok_or("no such part")?;
    let mut grown = Img { w, h, px: vec![crate::art::EMPTY; w * h] };
    for y in 0..paint.h {
        grown.px[y * w..y * w + paint.w].copy_from_slice(&paint.px[y * paint.w..(y + 1) * paint.w]);
    }
    for [x, y, rw, rh] in b.rects() {
        for yy in y..y + rh {
            for xx in x..x + rw {
                grown.px[yy as usize * w + xx as usize] = c;
            }
        }
    }
    p.boxes.push(b);
    *paint = grown;
    Ok(())
}

/// Every part's voxels made one painted box of the part's size (4 px a voxel), its faces in the part's most
/// used colour, and the voxels emptied: a voxel starter becomes a figure to paint (ADR 0116). How many parts.
pub fn voxels_to_boxes(m: &mut Model, paint: &mut Img) -> Result<usize, String> {
    let mut n = 0;
    for i in 0..m.parts.len() {
        let mut count = [0usize; 256];
        m.parts[i].cells.iter().filter(|v| **v != 0).for_each(|v| count[*v as usize] += 1);
        let Some(k) = (1..256).filter(|k| count[*k] > 0).max_by_key(|k| count[*k]) else { continue };
        let c = m.rgb.get(k - 1).copied().unwrap_or([128; 3]);
        add_box(m, paint, i, m.parts[i].size.map(|v| v * 4), c).map_err(|e| format!("{}: {e}", m.parts[i].name))?;
        m.parts[i].cells.iter_mut().for_each(|v| *v = 0);
        n += 1;
    }
    Ok(n)
}

/// The files column's picture: the model at rest seen from the front (+z), 16 x 16.
pub fn thumb(m: &Model, paint: Option<&Img>) -> Img {
    let bg = [26, 28, 32];
    let mut img = Img::filled(1, bg);
    let poses = m.pose(None, 0.0);
    let mut vox = vec![];
    for (i, p) in m.parts.iter().enumerate() {
        for c in rect(IVec3::ZERO, size(p) - 1) {
            let v = get(p, c);
            if v != 0 {
                vox.push((poses[i].transform_point3(c.as_vec3() + 0.5) / VOXEL, m.rgb.get(v as usize - 1).copied().unwrap_or([200; 3])));
            }
        }
    }
    // a painted box's faces, a point a pixel (ADR 0116)
    for ([o, eu, ev], r) in paint.map_or(vec![], |_| m.rest_faces()) {
        let p = paint.unwrap();
        for (u, v) in (0..r[3]).flat_map(|v| (0..r[2]).map(move |u| (u, v))) {
            let (x, y) = ((r[0] + u) as usize, (r[1] + v) as usize);
            let Some(&c) = p.px.get(y * p.w + x).filter(|c| x < p.w && **c != crate::art::EMPTY) else { continue };
            vox.push(((o + eu * ((u as f32 + 0.5) / r[2] as f32) + ev * ((v as f32 + 0.5) / r[3] as f32)) / VOXEL, c));
        }
    }
    if vox.is_empty() {
        return img;
    }
    let (lo, hi) = vox.iter().fold((Vec3::INFINITY, Vec3::NEG_INFINITY), |(a, b), (p, _)| (a.min(*p - 0.5), b.max(*p + 0.5)));
    let span = (hi - lo).x.max((hi - lo).y);
    let k = 14.0 / span;
    let mut depth = vec![f32::NEG_INFINITY; 256];
    let off = Vec3::new(1.0 + (14.0 - (hi - lo).x * k) / 2.0, 1.0 + (14.0 - (hi - lo).y * k) / 2.0, 0.0);
    for (p, c) in vox {
        let x0 = ((p.x - 0.5 - lo.x) * k + off.x).floor().max(0.0) as usize;
        let x1 = ((p.x + 0.5 - lo.x) * k + off.x).ceil().min(16.0) as usize;
        let y0 = ((hi.y - p.y - 0.5) * k + off.y).floor().max(0.0) as usize;
        let y1 = ((hi.y - p.y + 0.5) * k + off.y).ceil().min(16.0) as usize;
        for y in y0..y1 {
            for x in x0..x1 {
                if p.z > depth[y * 16 + x] {
                    depth[y * 16 + x] = p.z;
                    img.px[y * 16 + x] = c;
                }
            }
        }
    }
    img
}

/// The file check's part for models: every `<dir>/*.toml` read by the game's own `Model::parse`.
pub fn check_models(dir: &std::path::Path, pal: &Palette) -> (usize, Vec<String>) {
    let (mut ok, mut bad) = (0, vec![]);
    let mut files: Vec<String> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| n.ends_with(".toml")).collect();
    files.sort();
    for n in files {
        let shown = format!("data/models/{n}");
        match std::fs::read_to_string(dir.join(&n)).map_err(|e| format!("{shown}: {e}")).and_then(|t| Model::parse(&shown, &t, pal)) {
            Ok(_) => ok += 1,
            Err(e) => bad.push(e),
        }
    }
    (ok, bad)
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::models::{sample, VOXEL};

    fn pal() -> Palette {
        Palette::parse(
            "[[colour]]\nname = \"stone dark\"\nrgb = \"#404040\"\nramp = \"stone\"\n\n[[colour]]\nname = \"stone\"\nrgb = \"#808080\"\nramp = \"stone\"\n\n[[colour]]\nname = \"stone light\"\nrgb = \"#c0c0c0\"\nramp = \"stone\"\n\n[[colour]]\nname = \"copper\"\nrgb = \"#b87333\"\n",
        )
        .unwrap()
    }
    const BOX: &str = "colours = [\"stone\", \"copper\"]\n\n[[part]]\nname = \"body\"\nbox = [2, 2, 2]\nvoxels = \"\"\"\na.\n..\n\n..\n..\n\"\"\"\n\n[[part]]\nname = \"door\"\nparent = \"body\"\nat = [2, 0, 0]\nbox = [1, 1, 1]\nvoxels = \"\"\"\nb\n\"\"\"\n\n[[point]]\nname = \"flame\"\npart = \"body\"\nat = [1, 1, 0]\n\n[[clip]]\nname = \"work\"\nlength = 2.0\n";
    fn model() -> Model {
        Model::parse("box.toml", BOX, &pal()).unwrap()
    }
    fn near(a: Vec3, b: Vec3) {
        assert!(a.distance(b) < 1e-3, "{a} vs {b}");
    }
    fn solid(p: &Part) -> Vec<IVec3> {
        let s = IVec3::from(p.size.map(|v| v as i32));
        let mut v = vec![];
        for y in 0..s.y {
            for z in 0..s.z {
                for x in 0..s.x {
                    if get(p, IVec3::new(x, y, z)) != 0 {
                        v.push(IVec3::new(x, y, z));
                    }
                }
            }
        }
        v
    }

    #[test]
    fn the_orbit_looks_at_its_target_and_its_rays_meet_what_it_projects() {
        let o = Orbit { target: Vec3::new(0.5, 0.2, 0.5), yaw: 0.7, pitch: 0.4, dist: 3.0 };
        assert!((o.eye().distance(o.target) - 3.0).abs() < 1e-4);
        assert!(o.eye().y > o.target.y, "raised");
        let c = o.project(o.target, 70.0, 1.6).unwrap();
        assert!(c[0].abs() < 1e-4 && c[1].abs() < 1e-4);
        let p = Vec3::new(1.0, 0.7, -0.2);
        let s = o.project(p, 70.0, 1.6).unwrap();
        let (e, d) = o.ray(s, 70.0, 1.6);
        near(e + d * (p - e).dot(d), p);
        // up in the world is up on the screen; behind the eye is nowhere
        assert!(o.project(o.target + Vec3::Y * 0.1, 70.0, 1.6).unwrap()[1] > 0.0);
        assert_eq!(o.project(o.eye() * 2.0 - o.target, 70.0, 1.6), None);
    }

    #[test]
    fn the_views_look_from_their_side_and_turning_and_zooming_keep_their_point_in_place() {
        let mut o = Orbit::default();
        for (k, (name, ..)) in VIEWS.iter().enumerate() {
            o.look_from(k);
            let from = (o.eye() - o.target).normalize();
            let want = match *name {
                "front" => Vec3::Z,
                "back" => -Vec3::Z,
                "right" => Vec3::X,
                "left" => -Vec3::X,
                "top" => Vec3::Y,
                _ => -Vec3::Y,
            };
            assert!(from.dot(want) > 0.999, "{name}: from {from}");
            // the front view has +x on the right and +y up; the top view has the front (+z) at the bottom
            let s = o.project(o.target + Vec3::X * 0.1, 70.0, 1.6).unwrap();
            assert!(s[0] > 0.0 || !matches!(*name, "front" | "top"), "{name}: +x at {s:?}");
            if *name == "top" {
                assert!(o.project(o.target + Vec3::Z * 0.1, 70.0, 1.6).unwrap()[1] < 0.0);
            }
        }
        // turning about a point keeps it where it shows, a drag from the Top view does not jump
        let mut o = Orbit::default();
        let p = Vec3::new(0.2, 0.1, 0.9);
        let before = o.project(p, 70.0, 1.6).unwrap();
        o.turn_about(p, 0.4, -0.3);
        let after = o.project(p, 70.0, 1.6).unwrap();
        assert!((before[0] - after[0]).abs() < 1e-3 && (before[1] - after[1]).abs() < 1e-3, "{before:?} {after:?}");
        o.look_from(4);
        o.turn(0.0, 0.01);
        assert!((o.pitch - 1.56).abs() < 1e-4, "Top kept: {}", o.pitch);
        // zooming toward a point keeps it in place too
        let mut o = Orbit::default();
        let (f, r, _) = o.basis();
        let p = o.target + r * 0.3;
        let before = o.project(p, 70.0, 1.6).unwrap();
        o.zoom_about(0.5, p);
        let after = o.project(p, 70.0, 1.6).unwrap();
        assert!((before[0] - after[0]).abs() < 1e-3 && (before[1] - after[1]).abs() < 1e-3, "{before:?} {after:?}");
        assert!((o.dist - 1.3).abs() < 1e-4 && (o.target - p).dot(f).abs() < 1e-4);
        // framing a box looks at its middle, near enough that it fills the view and every corner shows
        o.frame(Vec3::new(0.0, 0.5, 0.0), Vec3::new(0.25, 1.0, 0.25), (35f32).to_radians().tan());
        near(o.target, Vec3::new(0.125, 0.75, 0.125));
        assert!(o.dist < 1.0, "{}", o.dist);
        for k in 0..8 {
            let c = Vec3::new((k & 1) as f32 * 0.25, 0.5 + (k >> 1 & 1) as f32 * 0.5, (k >> 2 & 1) as f32 * 0.25);
            let s = o.project(c, 70.0, 1.6).unwrap();
            assert!(s[0].abs() < 1.0 && s[1].abs() < 1.0, "corner {c} at {s:?}");
        }
    }

    #[test]
    fn picking_finds_the_face_under_the_ray_or_where_it_enters_the_box() {
        let m = model();
        let poses = m.pose(None, 0.0);
        let v = VOXEL;
        // straight down onto the body's voxel (0, 0, 0): its top face
        let h = pick(&m, &poses, None, Vec3::new(0.5 * v, 1.0, 0.5 * v), -Vec3::Y).unwrap();
        assert_eq!((h.part, h.cell, h.n, h.solid), (0, IVec3::ZERO, IVec3::Y, true));
        assert_eq!(h.target(), IVec3::Y);
        // down beside it: no voxel, so the cell it enters the body's box by, on top
        let h = pick(&m, &poses, Some(0), Vec3::new(1.5 * v, 1.0, 0.5 * v), -Vec3::Y).unwrap();
        assert_eq!((h.cell, h.n, h.solid), (IVec3::new(1, 1, 0), IVec3::Y, false));
        assert_eq!(h.target(), IVec3::new(1, 1, 0));
        // the door (a child at x = 2) from the +x side, and only the door when it is the one chosen
        let h = pick(&m, &poses, None, Vec3::new(1.0, 0.5 * v, 0.5 * v), -Vec3::X).unwrap();
        assert_eq!((h.part, h.cell, h.n), (1, IVec3::ZERO, IVec3::X));
        // a miss
        assert_eq!(pick(&m, &poses, None, Vec3::new(5.0, 5.0, 5.0), Vec3::Y), None);
        // a turned part is picked where it is drawn: the door turned 90 degrees about y at its corner
        let mut m2 = m.clone();
        m2.clips[0].key.push(Key { part: "door".into(), t: 0.0, shift: [0.0; 3], turn: [0.0, 90.0, 0.0], ease: Ease::Linear });
        let poses = m2.pose(Some("work"), 0.0);
        // the door's voxel now lies at x 2..3, z -1..0: from above at its middle
        let h = pick(&m2, &poses, Some(1), Vec3::new(2.5 * v, 1.0, -0.5 * v), -Vec3::Y).unwrap();
        assert!(h.solid && h.part == 1, "{h:?}");
        // the plane of the first face: a ray from elsewhere lands on the same layer
        let first = pick(&m, &m.pose(None, 0.0), Some(0), Vec3::new(0.5 * v, 1.0, 0.5 * v), -Vec3::Y).unwrap();
        let c = on_plane(m.pose(None, 0.0)[0], &first, Vec3::new(1.5 * v, 1.0, 1.5 * v), Vec3::new(0.0, -1.0, 0.0)).unwrap();
        assert_eq!(c, IVec3::new(1, 1, 1));
    }

    #[test]
    fn picking_on_a_layer_lands_on_its_plane_whatever_stands_above() {
        let m = model();
        let poses = m.pose(None, 0.0);
        let (body, v) = (&m.parts[0], VOXEL);
        // from above beside the voxel (0, 0, 0): row 0's empty cell (1, 0, 0), though the box's top is nearer
        let h = pick_layer(body, poses[0], 0, Vec3::new(1.5 * v, 1.0, 0.5 * v), -Vec3::Y).unwrap();
        assert_eq!((h.cell, h.n, h.solid), (IVec3::new(1, 0, 0), IVec3::Y, false));
        // straight down onto the voxel itself: that cell, solid
        let h = pick_layer(body, poses[0], 0, Vec3::new(0.5 * v, 1.0, 0.5 * v), -Vec3::Y).unwrap();
        assert_eq!((h.cell, h.solid), (IVec3::ZERO, true));
        // row 1 over it: the empty cell above, the voxel under the plane does not stop the click
        let h = pick_layer(body, poses[0], 1, Vec3::new(0.5 * v, 1.0, 0.5 * v), -Vec3::Y).unwrap();
        assert_eq!((h.cell, h.solid), (IVec3::new(0, 1, 0), false));
        // a slanted ray lands where it crosses the plane: from (0.5, 1 + 3, 0.5) voxels down and along +x by 1 a voxel
        let o = Vec3::new(0.5 * v, 3.0 * v, 0.5 * v);
        let h = pick_layer(body, poses[0], 0, o, Vec3::new(1.0, -1.0, 0.0).normalize()).unwrap();
        assert_eq!(h.cell, IVec3::new(3, 0, 0), "outside the box: a cell the add grows the box to");
        // the same ray on row 1 crosses its plane a voxel sooner
        assert_eq!(pick_layer(body, poses[0], 1, o, Vec3::new(1.0, -1.0, 0.0).normalize()).unwrap().cell, IVec3::new(2, 1, 0));
        // from below, up through the floor
        let h = pick_layer(body, poses[0], 0, Vec3::new(1.5 * v, -1.0, 1.5 * v), Vec3::Y).unwrap();
        assert_eq!(h.cell, IVec3::new(1, 0, 1));
        // along the plane, away from it, or far off: nothing
        assert_eq!(pick_layer(body, poses[0], 0, Vec3::new(0.0, 0.5 * v, 0.0), Vec3::X), None);
        assert_eq!(pick_layer(body, poses[0], 0, Vec3::new(0.0, 1.0, 0.0), Vec3::Y), None);
        assert_eq!(pick_layer(body, poses[0], 0, Vec3::new(0.0, 1.0, 0.0), Vec3::new(1.0, -0.01, 0.0).normalize()), None);
        // the door, a child at x = 2: its own row 0, in its own cells
        let h = pick_layer(&m.parts[1], poses[1], 0, Vec3::new(2.5 * v, 1.0, 0.5 * v), -Vec3::Y).unwrap();
        assert_eq!((h.cell, h.solid), (IVec3::ZERO, true));
    }

    #[test]
    fn exposed_faces_are_the_ones_with_no_voxel_beside_them() {
        let mut m = model();
        assert_eq!(exposed(&m.parts[0]).len(), 6, "one voxel: six faces");
        apply(&mut m, 0, &[IVec3::new(1, 0, 0)], Op::Add(1));
        let e = exposed(&m.parts[0]);
        assert_eq!(e.len(), 10, "two side by side: the faces between them hidden");
        assert!(!e.contains(&(IVec3::ZERO, 0)) && e.contains(&(IVec3::ZERO, 1)));
        let q = face_quad(IVec3::new(1, 0, 0), IVec3::X);
        assert!(q.iter().all(|c| c.x == 2.0) && q.contains(&Vec3::new(2.0, 1.0, 1.0)));
        assert!(face_quad(IVec3::ZERO, IVec3::NEG_Y).iter().all(|c| c.y == 0.0));
    }

    #[test]
    fn adding_outside_the_box_grows_it_and_nothing_moves_in_the_cell() {
        let mut m = model();
        let before = m.pose(None, 0.0)[1].transform_point3(Vec3::ZERO);
        let flame = m.points[0].at;
        // one voxel to the -x of the body: the box grows to 3 wide, its corner one voxel lower in x
        assert!(apply(&mut m, 0, &[IVec3::new(-1, 0, 0)], Op::Add(2)));
        let b = &m.parts[0];
        assert_eq!((b.size, b.at, b.pivot), ([3, 2, 2], [-1, 0, 0], [1.0, 0.0, 0.0]));
        assert_eq!(solid(b), vec![IVec3::new(0, 0, 0), IVec3::new(1, 0, 0)]);
        assert_eq!(get(b, IVec3::ZERO), 2);
        // the door (its child) and the flame (its point) stay where they were in the cell
        assert_eq!(m.parts[1].at, [3, 0, 0]);
        near(m.pose(None, 0.0)[1].transform_point3(Vec3::ZERO), before);
        assert_eq!(m.points[0].at, [flame[0] + 1.0, flame[1], flame[2]]);
        // never past 32
        assert!(!apply(&mut m, 0, &[IVec3::new(40, 0, 0)], Op::Add(1)));
        // remove and paint
        assert!(apply(&mut m, 0, &[IVec3::ZERO], Op::Remove));
        assert!(!apply(&mut m, 0, &[IVec3::new(2, 1, 1)], Op::Paint(2)), "paint only colours voxels that are there");
        assert!(apply(&mut m, 0, &[IVec3::new(1, 0, 0)], Op::Paint(2)));
        assert_eq!(solid(&m.parts[0]), vec![IVec3::new(1, 0, 0)]);
        assert_eq!(get(&m.parts[0], IVec3::new(1, 0, 0)), 2);
    }

    #[test]
    fn mirror_box_line_and_fill_choose_their_cells() {
        let m = mirrored([4, 1, 4], &[IVec3::new(0, 0, 1)], true, true);
        assert_eq!(m, vec![IVec3::new(0, 0, 1), IVec3::new(3, 0, 1), IVec3::new(0, 0, 2), IVec3::new(3, 0, 2)]);
        assert_eq!(mirrored([4, 1, 4], &[IVec3::new(0, 0, 0)], true, false).len(), 2);
        assert_eq!(rect(IVec3::new(2, 0, 1), IVec3::new(0, 0, 0)).len(), 6);
        let l = line(IVec3::ZERO, IVec3::new(3, 0, 1));
        assert_eq!(l.len(), 4);
        assert_eq!((l[0], l[3]), (IVec3::ZERO, IVec3::new(3, 0, 1)));
        assert!(l.windows(2).all(|w| (w[1] - w[0]).abs().max_element() == 1), "no gaps: {l:?}");
        // fill the empty cells of the bottom layer joined to (1, 0, 0): three of four, the layer above untouched
        let p = model().parts[0].clone();
        let f = fill_plane(&p, IVec3::new(1, 0, 0), 1);
        assert_eq!(f.len(), 3);
        assert!(f.iter().all(|c| c.y == 0));
    }

    #[test]
    fn a_selection_moves_and_a_face_of_it_extrudes() {
        let mut m = model();
        assert!(move_sel(&mut m, 0, IVec3::ZERO, IVec3::ZERO, IVec3::new(1, 1, 1)));
        assert_eq!(solid(&m.parts[0]), vec![IVec3::new(1, 1, 1)]);
        // pull the voxel's +y face up by 2: the box grows to hold it
        let (a, b) = extrude(&mut m, 0, IVec3::new(1, 1, 1), IVec3::new(1, 1, 1), IVec3::Y, 2);
        assert_eq!(solid(&m.parts[0]), vec![IVec3::new(1, 1, 1), IVec3::new(1, 2, 1), IVec3::new(1, 3, 1)]);
        assert_eq!((a, b), (IVec3::new(1, 1, 1), IVec3::new(1, 3, 1)));
        assert_eq!(m.parts[0].size, [2, 4, 2]);
    }

    #[test]
    fn shade_by_light_lightens_open_tops_and_darkens_undersides_along_the_ramp() {
        let p = pal();
        // a 3 x 3 x 3 cube of stone with one voxel standing on top in the middle
        let mut text = String::from("colours = [\"stone\"]\n\n[[part]]\nname = \"c\"\nbox = [3, 4, 3]\nvoxels = \"\"\"\n");
        text += "aaa\naaa\naaa\n\naaa\naaa\naaa\n\naaa\naaa\naaa\n\n...\n.a.\n...\n\"\"\"\n";
        let mut m = Model::parse("c.toml", &text, &p).unwrap();
        let n = shade_by_light(&mut m, &p, None, 3.0);
        assert!(n > 0);
        let name = |m: &Model, c: IVec3| m.colours[get(&m.parts[0], c) as usize - 1].clone();
        assert_eq!(name(&m, IVec3::new(1, 3, 1)), "stone light", "the top voxel, open on five sides");
        assert_eq!(name(&m, IVec3::new(0, 2, 0)), "stone light", "a top corner");
        assert_eq!(name(&m, IVec3::new(1, 0, 1)), "stone dark", "the underside");
        assert_eq!(name(&m, IVec3::new(1, 1, 0)), "stone", "the middle of a side");
        assert_eq!(name(&m, IVec3::new(1, 1, 1)), "stone", "inside: never seen");
        // every colour is a palette name: the file still reads
        assert!(Model::parse("c.toml", &text_of(&m), &p).is_ok());
    }
    fn text_of(m: &Model) -> String {
        text(m)
    }

    #[test]
    fn a_painted_block_becomes_an_eight_voxel_cube_of_its_commoner_colours() {
        let p = pal();
        let mut m = model();
        let (g, c) = ([0x80; 3], [0xb8, 0x73, 0x33]);
        let mut top = Img::filled(1, g);
        // the top's texels (0, 0), (1, 0), (0, 1) copper: its corner 2 x 2 is copper by three to one
        for (x, y) in [(0, 0), (1, 0), (0, 1)] {
            crate::art::pencil(&mut top, 0, x, y, c, false);
        }
        let side = Img::filled(1, [0x40; 3]);
        let i = from_texture(&mut m, &p, [Some(&top), Some(&side), None]).unwrap();
        let part = &m.parts[i];
        assert_eq!(part.size, [8, 8, 8]);
        assert_eq!(part.cells.iter().filter(|&&v| v == 0).count(), 0, "a whole cube");
        let col = |c: IVec3| m.colours[get(part, c) as usize - 1].clone();
        assert_eq!(col(IVec3::new(0, 7, 0)), "copper");
        assert_eq!(col(IVec3::new(4, 7, 4)), "stone");
        assert_eq!(col(IVec3::new(0, 3, 4)), "stone dark", "a side");
        assert_eq!(col(IVec3::new(4, 0, 4)), "stone dark", "no bottom painted: the side's");
        assert!(Model::parse("b.toml", &text(&m), &p).is_ok());
    }

    #[test]
    fn a_key_is_set_at_a_time_and_replaced_there() {
        let mut m = model();
        let c = &mut m.clips[0];
        set_key(c, "door", 1.0, [0.0; 3], [0.0, 50.0, 0.0]);
        set_key(c, "door", 0.0, [0.0; 3], [0.0; 3]);
        set_key(c, "door", 1.0, [0.0; 3], [0.0, 80.0, 0.0]);
        assert_eq!(c.key.len(), 2);
        assert_eq!(c.key.iter().map(|k| k.t).collect::<Vec<_>>(), vec![0.0, 1.0]);
        near(sample(c, "door", 1.0).1, Vec3::new(0.0, 80.0, 0.0));
        near(sample(c, "door", 0.5).1, Vec3::new(0.0, 40.0, 0.0));
    }

    #[test]
    fn ready_made_motions_fill_the_clip_with_ordinary_keys() {
        let mut m = model();
        let c = &mut m.clips[0];
        motion(c, "door", Motion::Swing, 1, 30.0);
        assert!(c.looping);
        near(sample(c, "door", 0.0).1, Vec3::new(0.0, -30.0, 0.0));
        near(sample(c, "door", 1.0).1, Vec3::new(0.0, 30.0, 0.0));
        motion(c, "door", Motion::Spin, 1, 360.0);
        assert_eq!(c.key.iter().filter(|k| k.part == "door").count(), 2, "a motion replaces the part's keys");
        near(sample(c, "door", 1.0).1, Vec3::new(0.0, 180.0, 0.0));
        motion(c, "body", Motion::Bob, 1, 2.0);
        near(sample(c, "body", 1.0).0, Vec3::new(0.0, 2.0, 0.0));
        near(sample(c, "body", 2.0).0, Vec3::ZERO);
        motion(c, "body", Motion::OpenShut, 1, 90.0);
        near(sample(c, "body", 1.0).1, Vec3::new(0.0, 90.0, 0.0));
        near(sample(c, "body", 2.0).1, Vec3::ZERO);
        motion(c, "body", Motion::Shake, 0, 0.5);
        assert!(sample(c, "body", 0.25).0.x.abs() > 0.4);
        near(sample(c, "body", 2.0).0, Vec3::ZERO);
    }

    #[test]
    fn parts_and_points_are_added_renamed_and_reparented() {
        let mut m = model();
        let i = add_part(&mut m, Some(1));
        assert_eq!(m.parts[i].parent, "door");
        assert_eq!(m.up[i], Some(1));
        m.clips[0].key.push(Key { part: "door".into(), t: 0.0, shift: [0.0; 3], turn: [0.0; 3], ease: Ease::Linear });
        rename_part(&mut m, 1, "lid").unwrap();
        assert_eq!((m.parts[i].parent.as_str(), m.clips[0].key[0].part.as_str()), ("lid", "lid"));
        assert!(rename_part(&mut m, 1, "body").is_err(), "a name used twice");
        assert!(reparent(&mut m, 0, Some(i)).is_err(), "a loop");
        reparent(&mut m, i, None).unwrap();
        assert_eq!((m.parts[i].parent.as_str(), m.up[i]), ("", None));
        let k = add_point(&mut m, 1, [0.5, 0.5, 0.0]);
        assert_eq!((m.points[k].part.as_str(), m.points[k].at), ("lid", [0.5, 0.5, 0.0]));
        assert_ne!(m.points[k].name, m.points[0].name);
        let again = Model::parse("box.toml", &text(&m), &pal()).unwrap();
        assert_eq!(again.parts.len(), 3);
    }

    #[test]
    fn the_slice_hides_what_is_above_its_height() {
        let mut m = model();
        apply(&mut m, 0, &[IVec3::new(0, 1, 0)], Op::Add(1));
        let s = sliced(&m, &m.pose(None, 0.0), 1.0);
        assert_eq!(solid(&s.parts[0]), vec![IVec3::ZERO]);
        assert_eq!(solid(&s.parts[1]), vec![IVec3::ZERO]);
        assert_eq!(solid(&sliced(&m, &m.pose(None, 0.0), 2.0).parts[0]).len(), 2);
    }

    /// A click lands on the voxel drawn under the pointer: the snake's tail (a child of a child, `at` and pivot
    /// not 0) bent by a clip, picked through the middle of each of its voxels as the renderer draws them (the
    /// same `Model::pose`, every voxel's own corners).
    #[test]
    fn a_click_lands_on_the_posed_voxel_of_a_child_part() {
        let p = Palette::parse(include_str!("../../../data/palette.toml")).unwrap();
        let bend = r#"
[[clip]]
name = "bend"
length = 1.0
key = [
  { part = "neck", t = 0.0, turn = [0, 40, 0] },
  { part = "middle", t = 0.0, turn = [0, -30, 10] },
  { part = "tail", t = 0.0, turn = [20, 50, 0] },
]
"#;
        let m = Model::parse("snake", &(include_str!("../testdata/snake_voxels.toml").replace("\r\n", "\n") + bend), &p).unwrap();
        let tail = m.parts.iter().position(|q| q.name == "tail").unwrap();
        assert!(m.parts[tail].at != [0; 3] && m.parts[tail].pivot != [0.0; 3]);
        let poses = m.pose(Some("bend"), 0.0);
        assert!(!poses[tail].abs_diff_eq(m.pose(None, 0.0)[tail], 1e-3), "the clip moves the tail");
        let eye = Vec3::new(0.4, 1.6, -0.9);
        let q = &m.parts[tail];
        let mut seen = 0;
        for c in (0..q.cells.len() as i32).map(|k| IVec3::new(k % q.size[0] as i32, k / q.size[0] as i32 % q.size[1] as i32, k / (q.size[0] * q.size[1]) as i32)).filter(|c| get(q, *c) != 0) {
            let mid = poses[tail].transform_point3(c.as_vec3() + 0.5);
            let h = pick(&m, &poses, None, eye, (mid - eye).normalize()).expect("a hit");
            // the voxel under the pointer, or one drawn in front of it on the same line
            if h.part == tail && h.cell == c {
                seen += 1;
            } else {
                let front = poses[h.part].transform_point3(h.cell.as_vec3() + 0.5);
                assert!(front.distance(eye) < mid.distance(eye), "{c} picked as {} {} behind it", m.parts[h.part].name, h.cell);
            }
        }
        assert!(seen > 0, "no voxel of the tail seen from the eye");
    }

    #[test]
    fn every_starter_shape_is_a_valid_model_standing_in_its_block() {
        let p = Palette::parse(include_str!("../../../data/palette.toml")).unwrap();
        for (kind, ..) in STARTERS {
            let m = starter(&p, kind).unwrap_or_else(|| panic!("{kind}: not made"));
            let again = Model::parse(kind, &text(&m), &p).unwrap();
            assert_eq!(again.parts.iter().map(|q| &q.cells).collect::<Vec<_>>(), m.parts.iter().map(|q| &q.cells).collect::<Vec<_>>(), "{kind}");
            assert!(m.colours.iter().all(|c| p.colours.iter().any(|k| &k.name == c)), "{kind}: a colour the palette lacks");
            let poses = m.pose(None, 0.0);
            for (i, q) in m.parts.iter().enumerate() {
                assert!(q.cells.iter().any(|c| *c != 0), "{kind}: {} is empty", q.name);
                assert!((0..3).all(|k| (0.0..=q.size[k] as f32).contains(&q.pivot[k])), "{kind}: {}'s pivot outside its box", q.name);
                assert!(q.parent.is_empty() || m.parts.iter().any(|o| o.name == q.parent), "{kind}: {}'s parent", q.name);
                // every voxel at rest inside the block at the origin
                let s = IVec3::from(q.size.map(|v| v as i32));
                for c in (0..s.x * s.y * s.z).map(|n| IVec3::new(n % s.x, n / (s.x * s.z), n / s.x % s.z)).filter(|c| get(q, *c) != 0) {
                    let at = poses[i].transform_point3(c.as_vec3() + 0.5);
                    assert!(at.cmpgt(Vec3::ZERO).all() && at.cmplt(Vec3::ONE).all(), "{kind}: {} {c} at {at}", q.name);
                }
            }
            assert!(thumb(&m, None).px.iter().any(|c| *c != [26, 28, 32]), "{kind}: an empty thumbnail");
        }
        let c = starter(&p, "creature").unwrap();
        assert_eq!(c.parts.iter().map(|q| q.name.as_str()).collect::<Vec<_>>(), ["body", "head", "leg_fl", "leg_fr", "leg_bl", "leg_br"]);
        assert!(c.parts[1..].iter().all(|q| q.parent == "body") && c.parts[2..].iter().all(|q| q.pivot[1] == q.size[1] as f32), "legs turn at the hip");
        let t = starter(&p, "tool").unwrap();
        assert_eq!((t.parts.len(), t.parts[1].parent.as_str()), (2, "handle"));
        assert!(starter(&p, "nothing").is_none());
    }

    /// A creature from nothing: the starter's voxels become painted boxes the game's reader takes with their
    /// painting, and the files column shows the painting.
    #[test]
    fn a_voxel_starter_becomes_a_painted_figure() {
        let p = pal();
        let mut m = starter(&p, "creature").unwrap();
        let mut paint = Img { w: 0, h: 0, px: vec![] };
        assert_eq!(voxels_to_boxes(&mut m, &mut paint).unwrap(), 6);
        let again = Model::parse("new.toml", &text(&m), &p).unwrap();
        assert!(again.parts.iter().all(|q| q.cells.iter().all(|c| *c == 0) && q.boxes.len() == 1), "voxels emptied, a box each");
        assert_eq!(again.parts[0].boxes[0].size, [16, 12, 20], "4 px a voxel");
        let bytes = crate::art::encode(&paint);
        let painting = engine::models::painting(&bytes, &p).unwrap();
        again.check_painting(&painting).unwrap();
        assert!(!again.rest_faces().is_empty());
        assert!(thumb(&again, Some(&paint)).px.iter().any(|c| *c != [26, 28, 32]), "the thumbnail shows the painting");
        // another box on the head grows the painting under the others
        let h = paint.h;
        add_box(&mut m, &mut paint, 1, [4, 4, 2], [0x40; 3]).unwrap();
        assert_eq!((m.parts[1].boxes.len(), m.parts[1].boxes[1].uv), (2, [0, h as u32]));
        assert!(paint.h == h + 6 && paint.px[(h + 2) * paint.w + 3] == [0x40; 3]);
        assert!(add_box(&mut m, &mut paint, 0, [64, 4, 4], [0x40; 3]).unwrap_err().contains("63"));
        assert!(add_box(&mut m, &mut paint, 0, [60, 60, 60], [0x40; 3]).is_err(), "the painting would pass 128");
    }

    #[test]
    fn a_new_model_reads_its_text_back_and_has_a_thumbnail() {
        let p = pal();
        let m = starter(&p, "block").unwrap();
        let again = Model::parse("new.toml", &text(&m), &p).unwrap();
        assert_eq!(again.parts[0].cells, m.parts[0].cells);
        let t = thumb(&m, None);
        assert_eq!((t.w, t.px.len()), (16, 256));
        assert!(t.px.iter().any(|&c| c == [0x40, 0x40, 0x40] || c == [0x80; 3]), "the model's colour shows");
    }

    #[test]
    fn the_file_check_reads_every_model() {
        let d = std::env::temp_dir().join(format!("technoape-voxel-check-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("good.toml"), BOX).unwrap();
        std::fs::write(d.join("bad.toml"), BOX.replace("\"copper\"]", "\"gold\"]")).unwrap();
        let (ok, bad) = check_models(&d, &pal());
        assert_eq!(ok, 1);
        assert_eq!(bad.len(), 1);
        assert!(bad[0].starts_with("data/models/bad.toml: ") && bad[0].contains("gold"), "{bad:?}");
        let _ = std::fs::remove_dir_all(d);
    }

    const LIFE: &str = "
[[clip.event]]
t = 1.5
sound = \"clank\"
point = \"flame\"

[[state]]
name = \"working\"
clip = \"work\"
effects = [{ effect = \"fire\", point = \"flame\" }]
light = { point = \"flame\", colour = \"copper\", strength = 0.5 }
";
    fn lived() -> (Model, Life) {
        engine::models::parse_life("box.toml", &format!("{BOX}{LIFE}"), &pal(), &|_| true, &|_| true).unwrap()
    }

    #[test]
    fn a_rename_follows_through_the_life() {
        let (mut m, mut life) = lived();
        rename_point(&mut m, &mut life, 0, "fire").unwrap();
        rename_clip(&mut m, &mut life, 0, "burn").unwrap();
        let s = &life.states[0];
        assert_eq!((s.clip.as_deref(), s.effects[0].point.as_str(), s.light.as_ref().unwrap().point.as_str()), (Some("burn"), "fire", "fire"));
        assert_eq!((life.events[0].0.as_str(), life.events[0].1[0].point.as_deref()), ("burn", Some("fire")));
        assert!(rename_point(&mut m, &mut life, 0, " ").is_err());
        // the renamed file reads back with its life whole
        let (_, back) = engine::models::parse_life("box.toml", &text_life(&m, &life), &pal(), &|_| true, &|_| true).unwrap();
        assert_eq!(back, life);
    }

    #[test]
    fn an_event_keeps_inside_a_shortened_clip_and_a_removed_point_clears() {
        let (mut m, mut life) = lived();
        m.clips[0].length = 1.0;
        m.points.clear();
        fit_life(&m, &mut life);
        let e = &life.events[0].1[0];
        assert_eq!((e.t, e.point.as_deref()), (1.0, None));
        assert!(life.states[0].effects.is_empty() && life.states[0].light.is_none());
        m.clips.clear();
        fit_life(&m, &mut life);
        assert!(life.events.is_empty() && life.states[0].clip.is_none());
    }
}
