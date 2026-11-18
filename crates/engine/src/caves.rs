//! Caves by rock (W4, ADR 0063; `docs/design/worldgen.md`, "Caves follow rock"): water-cut systems that
//! wander along the water table through limestone and come out where a valley cuts down to it, and lava tubes
//! a few metres under rift basalt. Made from the map and the seed when a planet loads, on the server and every
//! client alike, as balls of air (`world::Cave`) that the readers ask through `Body::col_caves`.
use crate::rock::{Caves, Rock, ROCKS};
use crate::world::{Body, Cave, SEA_DEPTH};
use glam::DVec3;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::LazyLock;

/// A candidate start every this many columns, in each direction: some three limestone systems a square
/// kilometre of basin.
const SPACING: i32 = 384;
/// Caves keep this far from a face's seam (a ball is cut there, `col_caves`) and from the spawn, columns.
const SEAM: f64 = 12.0;
const SPAWN: f64 = 40.0;
/// A ball whose ground is lower than this over the sea is not made: no cave opens under the sea (the
/// generated sea starts at the generated floor, `Solid::generated`).
const DRY: f64 = 3.0;
/// The kinds of `data/caves.toml` keep this far from the spawn (its ground stays as before them), columns.
const SPAWN_KINDS: f64 = 64.0;
/// A kind's cave shorter than this many balls is not made.
const MIN_RUN: usize = 40;

struct Rng(u64);
impl Rng {
    fn f(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// How a kind runs (`data/caves.toml`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Shape {
    Tube,
    Fissure,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawKind {
    name: String,
    shape: Shape,
    hosts: Vec<String>,
    provinces: Vec<String>,
    depth: [f64; 2],
    spacing: i32,
    chance: f64,
    width: [f64; 2],
    height: [f64; 2],
    length: [f64; 2],
    skylight: Option<f64>,
    trench: Option<f64>,
    branch: Option<f64>,
    slope: Option<f64>,
    open: Option<f64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct KindsFile {
    kind: Vec<RawKind>,
}

/// A cave kind (a row of `data/caves.toml`): the rocks (block ids) and provinces it is cut in, its depth window
/// under the ground, how many, and its shape's numbers, metres. A tube's fields are 0 on a fissure, `open` on a tube.
#[derive(Clone, Debug, PartialEq)]
pub struct Kind {
    pub name: String,
    pub shape: Shape,
    pub hosts: Vec<u16>,
    pub provinces: Vec<u8>,
    pub depth: [f64; 2],
    pub spacing: i32,
    pub chance: f64,
    pub width: [f64; 2],
    pub height: [f64; 2],
    pub length: [f64; 2],
    pub skylight: f64,
    pub trench: f64,
    pub branch: f64,
    pub slope: f64,
    pub open: f64,
}

pub struct Kinds {
    pub kinds: Vec<Kind>,
}

impl Kinds {
    /// Reads and checks the file.
    pub fn parse(text: &str) -> Result<Kinds, String> {
        let f: KindsFile = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut kinds = Vec::new();
        for k in f.kind {
            let name = &k.name;
            // a host is a rock some province of data/rocks.toml is made of
            let mut hosts = Vec::new();
            for h in &k.hosts {
                let id = ROCKS.provinces.iter().flat_map(|p| &p.layers).map(|l| l.0).find(|&b| crate::blocks::BLOCKS.get(b).is_some_and(|d| &d.name == h)).ok_or(format!("kind '{name}': no rock '{h}' in data/rocks.toml"))?;
                hosts.push(id);
            }
            let mut provinces = Vec::new();
            for p in &k.provinces {
                provinces.push(crate::bake::rock::NAMES.iter().position(|n| n == p).ok_or(format!("kind '{name}': no province '{p}' (data/rocks.toml)"))? as u8);
            }
            if hosts.is_empty() || provinces.is_empty() {
                return Err(format!("kind '{name}': at least one host and one province"));
            }
            let range = |what: &str, v: [f64; 2], lo: f64, hi: f64| if lo <= v[0] && v[0] <= v[1] && v[1] <= hi { Ok(()) } else { Err(format!("kind '{name}': {what} {v:?}: least <= most, within {lo}..{hi}")) };
            range("depth", k.depth, 0.0, crate::world::DEEP)?;
            range("width", k.width, 0.5, 20.0)?;
            range("height", k.height, 1.0, k.depth[1] - k.depth[0])?;
            range("length", k.length, 10.0, 5000.0)?;
            if !(32..=4096).contains(&k.spacing) || !(0.0..=1.0).contains(&k.chance) {
                return Err(format!("kind '{name}': spacing 32..4096 columns, chance 0..1"));
            }
            let tube = [k.skylight, k.trench, k.branch, k.slope];
            let (skylight, trench, branch, slope, open) = match k.shape {
                Shape::Tube => {
                    let [Some(s), Some(t), Some(b), Some(l)] = tube else { return Err(format!("kind '{name}': a tube has skylight, trench, branch and slope")) };
                    if k.open.is_some() || !(20.0..=2000.0).contains(&s) || !(0.0..=1.0).contains(&t) || !(0.0..=5.0).contains(&b) || !(0.0..=0.5).contains(&l) {
                        return Err(format!("kind '{name}': no open on a tube; skylight 20..2000 m, trench 0..1, branch 0..5 a 100 m, slope 0..0.5"));
                    }
                    (s, t, b, l, 0.0)
                }
                Shape::Fissure => {
                    if tube.iter().any(|v| v.is_some()) {
                        return Err(format!("kind '{name}': skylight, trench, branch and slope are only a tube's"));
                    }
                    let o = k.open.ok_or(format!("kind '{name}': a fissure has open"))?;
                    if !(0.0..=1.0).contains(&o) || k.width[1] > 2.0 {
                        return Err(format!("kind '{name}': open 0..1, a fissure at most 2 m wide"));
                    }
                    (0.0, 0.0, 0.0, 0.0, o)
                }
            };
            kinds.push(Kind { name: k.name, shape: k.shape, hosts, provinces, depth: k.depth, spacing: k.spacing, chance: k.chance, width: k.width, height: k.height, length: k.length, skylight, trench, branch, slope, open });
        }
        if kinds.len() >= Cave::BALL as usize {
            return Err("too many kinds".into());
        }
        Ok(Kinds { kinds })
    }
}

pub static CAVES: LazyLock<Kinds> = LazyLock::new(|| Kinds::parse(include_str!("../../../data/caves.toml")).unwrap_or_else(|e| panic!("data/caves.toml: {e}")));

/// One cave of a kind as made: its axis (column i, column j, its floor's radius for a tube, its middle's for a
/// fissure), the stretches of the axis open to the sky (index ranges, a skylight or a trench, an open slot), and
/// its balls, which `make` puts in the ground.
#[derive(Clone, Debug, PartialEq)]
pub struct System {
    pub kind: u8,
    pub face: u8,
    pub axis: Vec<DVec3>,
    pub openings: Vec<(usize, usize)>,
    pub balls: Vec<Cave>,
}

/// The caves of `data/caves.toml`'s kinds on a planet with the mantle, in a fixed order: kind, face, start.
pub fn systems(b: &Body, seed: u64) -> Vec<System> {
    systems_of(b, seed, &CAVES.kinds)
}

/// `systems` of these kinds.
pub fn systems_of(b: &Body, seed: u64, kinds: &[Kind]) -> Vec<System> {
    let Some(map) = b.map() else { return Vec::new() };
    if map.rock.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (k, kind) in kinds.iter().enumerate() {
        let s = kind.spacing;
        for face in 0..6u8 {
            let mut probe = Probe { b, face, cols: HashMap::new() };
            for cy in 0..b.n / s {
                for cx in 0..b.n / s {
                    let mut rng = Rng(seed ^ 0xca5e_4b1d ^ (k as u64) << 60 ^ (face as u64) << 52 ^ (cx as u64) << 26 ^ cy as u64);
                    let (i, j) = ((cx * s) as f64 + rng.f() * s as f64, (cy * s) as f64 + rng.f() * s as f64);
                    if rng.f() >= kind.chance || probe.off(i, j, 0.0) {
                        continue;
                    }
                    // (a tube starts in its provinces; a fissure looks for a border first)
                    if kind.shape == Shape::Tube && !kind.provinces.contains(&probe.col(i as i32, j as i32).0.province) {
                        continue;
                    }
                    match kind.shape {
                        Shape::Tube => {
                            let a = rng.f() * std::f64::consts::TAU;
                            tube(&mut probe, kind, k as u8, &mut rng, i, j, a, None, 2, &mut out);
                        }
                        Shape::Fissure => fissure(&mut probe, kind, k as u8, &mut rng, i, j, &mut out),
                    }
                    // a column's rock and ground are asked again only near the next start
                    if probe.cols.len() > 1 << 16 {
                        probe.cols.clear();
                    }
                }
            }
        }
    }
    out
}

/// A face's columns as a kind's walk asks them: rock and ground, kept while it walks.
struct Probe<'a> {
    b: &'a Body,
    face: u8,
    cols: HashMap<(i32, i32), (Rock, f64)>,
}

impl Probe<'_> {
    fn col(&mut self, i: i32, j: i32) -> (Rock, f64) {
        let (b, f) = (self.b, self.face);
        *self.cols.entry((i, j)).or_insert_with(|| (b.col_rock(f, i, j), b.col_height(f, i, j)))
    }
    /// Too near a face's seam (a ball is cut there, `col_caves`; the slope is read 4 columns out) or the spawn.
    fn off(&self, i: f64, j: f64, r: f64) -> bool {
        let (n, m) = (self.b.n as f64, SEAM + r + 4.0);
        let (sf, si, sj) = self.b.locate(self.b.spawn);
        i < m || j < m || i > n - m || j > n - m || (self.face as usize == sf && (i - si).hypot(j - sj) < SPAWN_KINDS + r)
    }
    /// Water stands over the ground of column (i, j): the sea, a lake or a river would flood an opening there.
    fn wet(&mut self, i: f64, j: f64) -> bool {
        let (ci, cj) = (i.floor() as i32, j.floor() as i32);
        let ground = self.col(ci, cj).1;
        self.b.water_at(self.face, ci, cj).is_some_and(|w| self.b.layer_r(w.0) > ground)
    }
    /// Whether every cell ball c hollows under the ground is one of the kind's hosts by the rock rule
    /// (`Body::rock`), in a column of one of its provinces.
    fn fits(&mut self, c: &Cave, kind: &Kind) -> bool {
        let b = self.b;
        for (i, j, z) in cells_with(b, c, |i, j| self.col(i, j).1) {
            let rock = self.col(i, j).0;
            if !kind.provinces.contains(&rock.province) || !kind.hosts.contains(&b.rock(rock, b.layer_r(z as f64 + 0.5) as f32, 0)) {
                return false;
            }
        }
        true
    }
    /// The ground's slope at (i, j), metres a column, uphill.
    fn grad(&self, i: f64, j: f64) -> (f64, f64) {
        let g = |a: f64, c: f64| self.b.ground_r(self.face as usize, i + a, j + c);
        ((g(4.0, 0.0) - g(-4.0, 0.0)) / 8.0, (g(0.0, 4.0) - g(0.0, -4.0)) / 8.0)
    }
}

fn lerp(rng: &mut Rng, v: [f64; 2]) -> f64 {
    v[0] + rng.f() * (v[1] - v[0])
}

fn wrap(a: f64) -> f64 {
    (a + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
}

/// A lava tube from (i, j), heading a (deep-world.md 4): it follows the ground downhill a metre a step, a few
/// metres under it, its floor never rising (`floor`: a branch's starts at its parent's), and ends where the slope
/// flattens under the kind's `slope`, the host rock or its province ends, the ground rose so far over it that it
/// left the depth window, or near the sea, water, a seam or the spawn. Then its skylights, each over a heap of
/// rock on the floor (the roof that fell), and its branches.
#[allow(clippy::too_many_arguments)]
fn tube(p: &mut Probe, kind: &Kind, k: u8, rng: &mut Rng, mut i: f64, mut j: f64, mut a: f64, mut floor: Option<f64>, branches: u32, out: &mut Vec<System>) {
    let (b, face, sea) = (p.b, p.face, p.b.radius - SEA_DEPTH);
    let (w, hgt) = (lerp(rng, kind.width), lerp(rng, kind.height));
    let roof = kind.depth[0] + rng.f() * (kind.depth[1] - kind.depth[0] - hgt).clamp(0.0, 2.0);
    let len = lerp(rng, kind.length) as usize;
    // an arched roof over a flat floor: the ball stretched to the height, cut flat a third of the way down
    let (r, hz) = (w / 2.0, hgt / 1.35);
    let fl = 0.35 * hz;
    let phase = rng.f() * std::f64::consts::TAU;
    let (mut axis, mut balls, mut forks) = (Vec::new(), Vec::new(), Vec::new());
    for step in 0..len {
        if p.off(i, j, r) {
            break;
        }
        let ground = b.ground_r(face as usize, i, j);
        if ground - sea < DRY + hgt || p.wet(i, j) {
            break;
        }
        let (gx, gy) = p.grad(i, j);
        if gx.hypot(gy) < kind.slope {
            break;
        }
        let f = floor.map_or(ground - roof - hgt, |f: f64| f.min(ground - roof - hgt));
        if ground - f > kind.depth[1] {
            break;
        }
        let c = Cave { face, at: DVec3::new(i, j, b.layer_of(f) + fl), r, h: hz, floor: fl, kind: k };
        if !p.fits(&c, kind) {
            break;
        }
        floor = Some(f);
        axis.push(DVec3::new(i, j, f));
        balls.push(c);
        if branches > 0 && rng.f() < kind.branch / 100.0 {
            let side = if rng.f() < 0.5 { 1.0 } else { -1.0 };
            forks.push((axis.len() - 1, a + side * (0.5 + rng.f() * 0.5)));
        }
        // downhill, winding a little as lava does, never more than a tenth of a radian a metre
        let wind = 0.25 * (step as f64 / 23.0 + phase).sin();
        a += wrap((-gy).atan2(-gx) + wind - a).clamp(-0.1, 0.1);
        (i, j) = (i + a.cos(), j + a.sin());
    }
    if axis.len() < MIN_RUN {
        return;
    }
    // the skylights: the first within 0.8 of the spacing, then every 0.7 to 1.3 of it; a stretch that has fallen
    // in is a trench
    let n = axis.len();
    let mut at = kind.skylight * (0.3 + 0.5 * rng.f());
    let mut tries = Vec::new();
    while at < n as f64 - 4.0 {
        tries.push(at as usize);
        at += kind.skylight * (0.7 + 0.6 * rng.f());
    }
    if tries.is_empty() {
        tries.push(n / 2);
    }
    let mut openings = Vec::new();
    for s in tries {
        let trench = rng.f() < kind.trench;
        let e = (s + if trench { 20.0 + rng.f() * 30.0 } else { 3.0 + rng.f() * 2.0 } as usize).min(n - 1);
        let mut shaft = Vec::new();
        for q in &axis[s..=e] {
            let zg = b.layer_of(b.ground_r(face as usize, q.x, q.y)) + 1.0;
            let zc = b.layer_of(q.z) + fl;
            shaft.push(Cave { face, at: DVec3::new(q.x, q.y, zg), r: r * if trench { 0.9 } else { 0.8 }, h: 1.6 * (zg - zc), floor: zg - zc, kind: k });
        }
        if axis[s..=e].iter().any(|q| p.wet(q.x, q.y)) || !shaft.iter().all(|c| p.fits(c, kind)) {
            continue;
        }
        // the roof that fell: a heap under the opening, its foot the tube's radius and 3 m out
        let m = (r + 3.0).ceil() as usize;
        for (q, c) in balls.iter_mut().enumerate().take((e + m).min(n - 1) + 1).skip(s.saturating_sub(m)) {
            let out_by = s.saturating_sub(q).max(q.saturating_sub(e)) as f64;
            c.floor -= 0.45 * hgt * (1.0 - out_by / m as f64).max(0.0);
        }
        balls.extend(shaft);
        openings.push((s, e));
    }
    if openings.is_empty() {
        return;
    }
    out.push(System { kind: k, face, axis: axis.clone(), openings, balls });
    for (q, heading) in forks {
        tube(p, kind, k, rng, axis[q].x, axis[q].y, heading, Some(axis[q].z), branches - 1, out);
    }
}

/// A fissure near (i, j) (deep-world.md 4): it starts on the nearest border between provinces of the map within
/// 24 columns and follows it half a metre a step, a narrow tall slot whose height waves along it; over an open
/// stretch its top breaks the ground. A ball in a rock that is no host is left out (the crack does not open
/// there), so a fissure is in pieces where soft rock meets it; it ends where the border is lost, near the sea, a
/// seam or the spawn.
fn fissure(p: &mut Probe, kind: &Kind, k: u8, rng: &mut Rng, mut i: f64, mut j: f64, out: &mut Vec<System>) {
    let (b, face, sea) = (p.b, p.face, p.b.radius - SEA_DEPTH);
    let map = b.map().unwrap();
    let s = map.m as f64 / b.n as f64;
    let province = |i: f64, j: f64| {
        let (x, y) = b.map_xy(s, i, j);
        map.rock(face as usize, x, y)
    };
    let p0 = province(i, j);
    let Some((t, a0)) = (0..8).filter_map(|d| {
        let a = d as f64 * std::f64::consts::FRAC_PI_4;
        (1..=24).find(|&t| province(i + t as f64 * a.cos(), j + t as f64 * a.sin()) != p0).map(|t| (t, a))
    }).min_by_key(|x| x.0) else { return };
    (i, j) = (i + (t as f64 - 0.5) * a0.cos(), j + (t as f64 - 0.5) * a0.sin());
    let mut a = a0 + if rng.f() < 0.5 { 1.0 } else { -1.0 } * std::f64::consts::FRAC_PI_2;
    let r = (lerp(rng, kind.width) / 2.0).max(0.6);
    let (tall, top0) = (lerp(rng, kind.height), kind.depth[0] + rng.f() * 3.0);
    let len = lerp(rng, kind.length);
    let open = (rng.f() < kind.open).then(|| {
        let s0 = rng.f() * len * 0.6;
        (s0, s0 + 10.0 + rng.f() * 30.0)
    });
    let (ph, ph2) = (rng.f() * std::f64::consts::TAU, rng.f() * std::f64::consts::TAU);
    let (mut axis, mut balls, mut openings) = (Vec::new(), Vec::new(), Vec::<(usize, usize)>::new());
    for step in 0..(len * 2.0) as usize {
        let d = step as f64 * 0.5;
        if p.off(i, j, r) {
            break;
        }
        let ground = b.ground_r(face as usize, i, j);
        if ground - sea < DRY {
            break;
        }
        if step % 2 == 0 {
            // the border across the heading, nearest first: step towards it, turn a little with it
            let (nx, ny) = ((a + std::f64::consts::FRAC_PI_2).cos(), (a + std::f64::consts::FRAC_PI_2).sin());
            let at = |t: i32| province(i + t as f64 * nx, j + t as f64 * ny);
            let Some(c) = (0..=10).flat_map(|t| [t, -t - 1]).find(|&t| at(t) != at(t + 1)).map(|t| t as f64 + 0.5) else { break };
            (i, j) = (i + nx * c.clamp(-0.5, 0.5), j + ny * c.clamp(-0.5, 0.5));
            a += (c * 0.03).clamp(-0.06, 0.06) + 0.02 * (d / 17.0 + ph).sin();
        }
        // open over its open stretch (ramping up and down over 5 m), where no water stands
        let up = open.map_or(0.0, |(s0, s1)| ((d - s0).min(s1 - d) / 5.0).clamp(0.0, 1.0));
        let up = if up > 0.0 && p.wet(i, j) { 0.0 } else { up };
        let top = top0 + (-2.0 - top0) * up;
        // (a metre to spare over the least height: a slot's cells are whole layers)
        let foot = (top.max(0.0) + (tall * (0.8 + 0.2 * (d / 41.0 + ph2).sin())).max(kind.height[0] + 1.0)).min(kind.depth[1]);
        let mid = ground - (top + foot) / 2.0;
        let c = Cave { face, at: DVec3::new(i, j, b.layer_of(mid)), r, h: (foot - top) / 2.0, floor: (foot - top) / 2.0, kind: k };
        if p.fits(&c, kind) {
            axis.push(DVec3::new(i, j, mid));
            balls.push(c);
            if top < 0.0 {
                let q = axis.len() - 1;
                match openings.last_mut() {
                    Some(o) if o.1 + 1 == q => o.1 = q,
                    _ => openings.push((q, q)),
                }
            }
        }
        (i, j) = (i + 0.5 * a.cos(), j + 0.5 * a.sin());
    }
    if balls.len() >= MIN_RUN {
        out.push(System { kind: k, face, axis, openings, balls });
    }
}

/// The cells (i, j, layer) under the generated ground that ball c hollows: `col_caves`'s rule.
pub fn cells(b: &Body, c: &Cave) -> Vec<(i32, i32, i32)> {
    cells_with(b, c, |i, j| b.col_height(c.face, i, j))
}

/// `cells`, the ground's radius of a column from `ground`.
fn cells_with(b: &Body, c: &Cave, mut ground: impl FnMut(i32, i32) -> f64) -> Vec<(i32, i32, i32)> {
    let mut out = Vec::new();
    let r = c.r.ceil() as i32 + 1;
    let (ci, cj) = (c.at.x.floor() as i32, c.at.y.floor() as i32);
    for (i, j) in (ci - r..=ci + r).flat_map(|i| (cj - r..=cj + r).map(move |j| (i, j))) {
        if let Some((lo, hi)) = c.span(i, j) {
            let g = ground(i, j);
            out.extend((lo..hi).take_while(|&z| b.layer_r(z as f64 + 0.5) < g).map(|z| (i, j, z)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_caves_file_loads_and_a_broken_one_is_refused_and_says_why() {
        let k = &CAVES.kinds;
        assert_eq!(k.iter().map(|k| (k.name.as_str(), k.shape)).collect::<Vec<_>>(), [("lava tube", Shape::Tube), ("fissure", Shape::Fissure)]);
        assert_eq!(k[0].hosts, [crate::blocks::BLOCKS.id("basalt")]);
        assert_eq!(k[0].provinces, [crate::bake::rock::RIFT, crate::bake::rock::ARC]);
        let good = include_str!("../../../data/caves.toml").replace("\r\n", "\n");
        for (from, to, says) in [
            ("hosts = [\"basalt\"]", "hosts = [\"basalts\"]", "no rock 'basalts'"),
            ("hosts = [\"basalt\"]", "hosts = [\"dirt\"]", "no rock 'dirt'"),
            ("provinces = [\"rift\"", "provinces = [\"rifts\"", "no province 'rifts'"),
            ("depth = [1.5, 14.0]", "depth = [14.0, 1.5]", "depth"),
            ("depth = [1.5, 14.0]", "depth = [1.5, 400.0]", "depth"),
            ("width = [3.0, 8.0]", "width = [8.0, 3.0]", "width"),
            ("skylight = 200.0\n", "", "skylight"),
            ("open = 0.35\n", "open = 0.35\nskylight = 9.0\n", "only a tube"),
        ] {
            let e = Kinds::parse(&good.replacen(from, to, 1)).err().unwrap_or_else(|| panic!("{from} -> {to} loads"));
            assert!(e.contains(says), "{from} -> {to}: {e}");
        }
    }
}

/// Every cave of a baked planet, in a fixed order.
pub fn make(b: &Body, seed: u64) -> Vec<Cave> {
    let Some(map) = b.map() else { return Vec::new() };
    if map.rock.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let (sf, si, sj) = b.locate(b.spawn);
    let n = b.n;
    for face in 0..6 {
        for cy in 0..n / SPACING {
            for cx in 0..n / SPACING {
                let mut rng = Rng(seed ^ 0xca5e ^ (face as u64) << 52 ^ (cx as u64) << 26 ^ cy as u64);
                let (i, j) = ((cx * SPACING) as f64 + rng.f() * SPACING as f64, (cy * SPACING) as f64 + rng.f() * SPACING as f64);
                if face == sf && (i - si).hypot(j - sj) < SPAWN + 60.0 {
                    continue;
                }
                let d = b.dir(face, i, j);
                let rock = b.rock_at(face, i, j, d);
                let Some(p) = ROCKS.provinces.get(rock.province as usize) else { continue };
                let worm = match p.caves {
                    Caves::Limestone if rng.f() < 0.5 => Worm::Limestone,
                    // a planet with the mantle has the lava tubes of data/caves.toml instead
                    Caves::Tubes if b.mantle == 0 && rng.f() < 0.7 => Worm::Tube,
                    _ => continue,
                };
                let a = rng.f() * std::f64::consts::TAU;
                let len = match worm {
                    Worm::Limestone => 50 + (rng.f() * 110.0) as usize,
                    Worm::Tube => 30 + (rng.f() * 80.0) as usize,
                };
                walk(b, &mut rng, worm, face, i, j, a, len, 2, &mut out);
            }
        }
    }
    // shortcut: new generation rides on the mantle flag, give worlds a generation number when the next rule needs it
    if b.mantle > 0 {
        out.extend(systems(b, seed).into_iter().flat_map(|s| s.balls));
    }
    out
}

#[derive(Clone, Copy, PartialEq)]
enum Worm {
    Limestone,
    Tube,
}

/// The water table under face point (i, j), metres over the sea: the ground's height over the sea, smoothed
/// over 40 m and subdued to six tenths of it, and at least 3 m under the ground there.
pub fn water_table(b: &Body, face: usize, i: f64, j: f64) -> f64 {
    let over = |i: f64, j: f64| b.ground_r(face, i, j) - (b.radius - SEA_DEPTH);
    let smooth = [(0.0, 0.0), (40.0, 0.0), (-40.0, 0.0), (0.0, 40.0), (0.0, -40.0)].iter().map(|(a, c)| over(i + a, j + c)).sum::<f64>() / 5.0;
    (0.6 * smooth.max(0.0)).min(over(i, j) - 3.0)
}

/// One cave's balls from (i, j): a limestone system holds to the water table and ends where the limestone
/// does, a tube runs a few metres under the ground downhill; either ends near a seam, the spawn or the sea.
#[allow(clippy::too_many_arguments)]
fn walk(b: &Body, rng: &mut Rng, worm: Worm, face: usize, mut i: f64, mut j: f64, mut a: f64, len: usize, branches: u32, out: &mut Vec<Cave>) {
    let (n, sea) = (b.n as f64, b.radius - SEA_DEPTH);
    let (sf, si, sj) = b.locate(b.spawn);
    let tube_depth = 3.0 + rng.f() * 3.0;
    let mut z = None::<f64>;
    for _ in 0..len {
        let r = match worm {
            Worm::Limestone if rng.f() < 0.06 => 3.5 + rng.f() * 2.0,
            Worm::Limestone => 1.6 + rng.f() * 1.8,
            Worm::Tube => 1.8 + rng.f() * 1.0,
        };
        if i < SEAM + r || j < SEAM + r || i > n - SEAM - r || j > n - SEAM - r || (face == sf && (i - si).hypot(j - sj) < SPAWN) {
            return;
        }
        let ground = b.ground_r(face, i, j);
        if ground - sea < DRY + r {
            return;
        }
        let want = match worm {
            Worm::Limestone => b.layer_of(sea + water_table(b, face, i, j)),
            // a skylight now and then: the tube's roof breaks the ground
            Worm::Tube if rng.f() < 0.03 => b.layer_of(ground - r * 0.5),
            Worm::Tube => b.layer_of(ground - tube_depth - r),
        };
        let zz = z.map_or(want, |z| z + (want - z).clamp(-0.5, 0.5));
        z = Some(zz);
        // still in the rock this cave is cut in
        let rock = b.rock_at(face, i, j, b.dir(face, i, j));
        let here = ROCKS.at(rock, b.layer_r(zz) as f32, 0);
        let fits = match worm {
            Worm::Limestone => here == crate::blocks::BLOCKS.id("limestone"),
            Worm::Tube => here == crate::blocks::BLOCKS.id("basalt"),
        };
        if !fits || zz - r < 1.0 {
            return;
        }
        out.push(Cave::ball(face as u8, DVec3::new(i, j, zz), r));
        if branches > 0 && rng.f() < 0.025 {
            let side = if rng.f() < 0.5 { 1.0 } else { -1.0 };
            let (turn, len) = (side * (0.8 + rng.f() * 0.8), 20 + (rng.f() * 40.0) as usize);
            walk(b, rng, worm, face, i, j, a + turn, len, branches - 1, out);
        }
        a += match worm {
            Worm::Limestone => (rng.f() - 0.5) * 0.7,
            // downhill, as lava ran
            Worm::Tube => {
                let slope = |da: f64| b.ground_r(face, i + 6.0 * (a + da).cos(), j + 6.0 * (a + da).sin());
                if slope(0.4) < slope(-0.4) { 0.15 } else { -0.15 }
            }
        };
        let step = 0.8 * r;
        (i, j) = (i + a.cos() * step, j + a.sin() * step);
    }
}
