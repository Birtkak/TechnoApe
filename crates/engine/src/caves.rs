//! Caves by rock (W4, ADR 0063; `docs/design/worldgen.md`, "Caves follow rock"): water-cut systems that
//! wander along the water table through limestone and come out where a valley cuts down to it, and lava tubes
//! a few metres under rift basalt; from generation 3 the karst of `data/caves.toml` in their place
//! (draft-karst-caves). Made from the map and the seed when a planet loads, on the server and every
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
    Karst,
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
    generations: Option<[u8; 2]>,
    skylight: Option<f64>,
    trench: Option<f64>,
    branch: Option<f64>,
    slope: Option<f64>,
    open: Option<f64>,
    hall: Option<[f64; 2]>,
    hall_height: Option<[f64; 2]>,
    sinkhole: Option<[f64; 2]>,
    shaft: Option<[f64; 2]>,
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
    /// The worlds' generations it is made in (`Body::gen`), [least, most].
    pub generations: [u8; 2],
    pub skylight: f64,
    pub trench: f64,
    pub branch: f64,
    pub slope: f64,
    pub open: f64,
    /// A karst's: its halls' width and height, its sinkholes' bowl width and its shafts' width, [least, most]
    /// metres (0 on the other shapes).
    pub hall: [f64; 2],
    pub hall_height: [f64; 2],
    pub sinkhole: [f64; 2],
    pub shaft: [f64; 2],
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
            // a cave of fewer than MIN_RUN balls is not made: a tube's are a metre apart, a fissure's half a metre
            let step = if k.shape == Shape::Tube { 1.0 } else { 0.5 };
            range(&format!("length (at least {} m)", MIN_RUN as f64 * step), k.length, MIN_RUN as f64 * step, 5000.0)?;
            if !(32..=4096).contains(&k.spacing) || !(0.0..=1.0).contains(&k.chance) {
                return Err(format!("kind '{name}': spacing 32..4096 columns, chance 0..1"));
            }
            let tube = [k.skylight, k.trench, k.branch, k.slope];
            let karst = [k.hall, k.hall_height, k.sinkhole, k.shaft];
            if k.shape != Shape::Karst && karst.iter().any(|v| v.is_some()) {
                return Err(format!("kind '{name}': hall, hall_height, sinkhole and shaft are only a karst's"));
            }
            let (mut hall, mut hall_height, mut sinkhole, mut shaft) = ([0.0; 2], [0.0; 2], [0.0; 2], [0.0; 2]);
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
                Shape::Karst => {
                    let ([Some(s), None, Some(b), None], [Some(h), Some(hh), Some(sk), Some(sh)], None) = (tube, karst, k.open) else {
                        return Err(format!("kind '{name}': a karst has skylight (metres between sinkholes), branch, hall, hall_height, sinkhole and shaft, and no trench, slope or open"));
                    };
                    if !(20.0..=2000.0).contains(&s) || !(0.0..=5.0).contains(&b) {
                        return Err(format!("kind '{name}': skylight 20..2000 m, branch 0..5 a 100 m"));
                    }
                    range("hall", h, 6.0, 40.0)?;
                    range("hall_height", hh, 3.0, 20.0)?;
                    range("sinkhole", sk, 8.0, 40.0)?;
                    range("shaft", sh, 2.0, 8.0)?;
                    (hall, hall_height, sinkhole, shaft) = (h, hh, sk, sh);
                    (s, 0.0, b, 0.0, 0.0)
                }
            };
            let generations = k.generations.unwrap_or([1, u8::MAX]);
            if generations[0] < 1 || generations[0] > generations[1] {
                return Err(format!("kind '{name}': generations {generations:?}: least 1 or more, least <= most"));
            }
            kinds.push(Kind { name: k.name, shape: k.shape, hosts, provinces, depth: k.depth, spacing: k.spacing, chance: k.chance, width: k.width, height: k.height, length: k.length, generations, skylight, trench, branch, slope, open, hall, hall_height, sinkhole, shaft });
        }
        // a kind's starts are seeded with 4 bits of its index (`systems_of`)
        if kinds.len() > 16 {
            return Err(format!("{} kinds: at most 16", kinds.len()));
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
    /// A karst's halls: indices into `balls`.
    pub halls: Vec<usize>,
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
        // (a kind's index seeds it: a row made in other generations keeps its place in the file)
        if !(kind.generations[0]..=kind.generations[1]).contains(&b.gen) {
            continue;
        }
        let s = kind.spacing;
        for face in 0..6u8 {
            let mut probe = Probe { b, face, cols: HashMap::new(), axes: HashMap::new(), widest: 0.0, tables: HashMap::new(), next: 0 };
            for cy in 0..b.n / s {
                for cx in 0..b.n / s {
                    let mut rng = Rng(seed ^ 0xca5e_4b1d ^ (k as u64) << 60 ^ (face as u64) << 52 ^ (cx as u64) << 26 ^ cy as u64);
                    let (i, j) = ((cx * s) as f64 + rng.f() * s as f64, (cy * s) as f64 + rng.f() * s as f64);
                    if rng.f() >= kind.chance || probe.off(i, j, 0.0) {
                        continue;
                    }
                    // (a tube and a karst start in their provinces; a fissure looks for a border first)
                    if kind.shape != Shape::Fissure && !kind.provinces.contains(&probe.col(i as i32, j as i32).0.province) {
                        continue;
                    }
                    match kind.shape {
                        Shape::Tube if b.gen >= 2 => round_tube(&mut probe, kind, k as u8, &mut rng, i, j, None, 2, &mut out),
                        Shape::Tube => {
                            let a = rng.f() * std::f64::consts::TAU;
                            tube(&mut probe, kind, k as u8, &mut rng, i, j, a, None, 2, &mut out);
                        }
                        Shape::Fissure => fissure(&mut probe, kind, k as u8, &mut rng, i, j, &mut out),
                        Shape::Karst => karst(&mut probe, kind, k as u8, &mut rng, i, j, &mut out),
                    }
                    // a column's rock and ground are asked again only near the next start
                    if probe.cols.len() > 1 << 16 {
                        probe.cols.clear();
                        probe.tables.clear();
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
    cols: HashMap<(i32, i32), (Rock, f64, bool)>,
    /// The round tubes made on the face so far (generation 2): their axes' points (i, j, radius, the system's
    /// index in the output) by `AXES` x `AXES` columns, which a later tube keeps off (`crowds`).
    axes: HashMap<(i32, i32), Vec<(f64, f64, f64, usize)>>,
    /// The largest radius in `axes`: how far `crowds` must look.
    widest: f64,
    /// The water table's radius under a column's middle (`water_table`), as a karst asks it.
    tables: HashMap<(i32, i32), f64>,
    /// The next karst passage's number in `axes`.
    next: usize,
}

/// The cells of `Probe::axes`, columns.
const AXES: f64 = 8.0;
/// Rock left between two round tubes' walls at least, metres: a tube that comes nearer another ends.
const WALL: f64 = 3.0;
/// A branch is let near the tube it leaves for its first this many metres.
const FORK: usize = 25;
/// A round tube follows the flank: the ground's slope read this many columns either side (a gully's is not).
const FLANK: f64 = 16.0;
/// A round tube looks for water this far ahead, metres.
const AHEAD: f64 = 10.0;

/// The soil band's thickness at level 0, metres: `generated_under` and `generate` lay the biome's ground there,
/// not rock.
const BAND: f64 = 4.0;

impl Probe<'_> {
    /// Column (i, j)'s rock, ground radius, and whether generated water stands over its ground.
    fn col(&mut self, i: i32, j: i32) -> (Rock, f64, bool) {
        let (b, f) = (self.b, self.face);
        *self.cols.entry((i, j)).or_insert_with(|| {
            let ground = b.col_height(f, i, j);
            (b.col_rock(f, i, j), ground, b.water_at(f, i, j).is_some_and(|w| b.layer_r(w.0) > ground))
        })
    }
    /// Too near a face's seam (a ball is cut there, `col_caves`; the slope is read 4 columns out) or the spawn.
    fn off(&self, i: f64, j: f64, r: f64) -> bool {
        let (n, m) = (self.b.n as f64, SEAM + r + 4.0);
        let (sf, si, sj) = self.b.locate(self.b.spawn);
        i < m || j < m || i > n - m || j > n - m || (self.face as usize == sf && (i - si).hypot(j - sj) < SPAWN_KINDS + r)
    }
    /// Water stands over the ground of column (i, j): the sea, a lake or a river would flood an opening there.
    fn wet(&mut self, i: f64, j: f64) -> bool {
        self.col(i.floor() as i32, j.floor() as i32).2
    }
    /// Whether ball c may be cut: every cell it hollows under the ground is, as generated without it, one of the
    /// kind's hosts (rock by `Body::rock` under the soil band), in a column of one of its provinces, with no
    /// generated water over that column or the eight round it (a cave there would drain it). An `opening` (a
    /// skylight, a trench, an open slot) may also cut the soil band, in the columns where it breaks the ground.
    fn fits(&mut self, c: &Cave, kind: &Kind, opening: bool) -> bool {
        let b = self.b;
        for (i, j, z) in cells_with(b, c, |i, j| self.col(i, j).1) {
            let (rock, ground, _) = self.col(i, j);
            if !kind.provinces.contains(&rock.province) || (-1..=1).any(|a| (-1..=1).any(|d| self.col(i + a, j + d).2)) {
                return false;
            }
            let r = b.layer_r(z as f64 + 0.5);
            // a karst is dry: nothing of it under the water table (draft-karst-caves)
            if kind.shape == Shape::Karst && r < self.table(i, j) {
                return false;
            }
            let ok = if ground - r < BAND { opening && c.span(i, j).is_some_and(|(_, hi)| hi as f64 > b.layer_of(ground)) } else { kind.hosts.contains(&b.rock(rock, r as f32, 0)) };
            if !ok {
                return false;
            }
        }
        true
    }
    /// The ground's slope at (i, j), metres a column, uphill.
    fn grad(&self, i: f64, j: f64) -> (f64, f64) {
        self.grad_over(i, j, 4.0)
    }
    /// `grad` read `d` columns either side.
    fn grad_over(&self, i: f64, j: f64, d: f64) -> (f64, f64) {
        let g = |a: f64, c: f64| self.b.ground_r(self.face as usize, i + a, j + c);
        ((g(d, 0.0) - g(-d, 0.0)) / (2.0 * d), (g(0.0, d) - g(0.0, -d)) / (2.0 * d))
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
        if !p.fits(&c, kind, false) {
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
        if axis[s..=e].iter().any(|q| p.wet(q.x, q.y)) || !shaft.iter().all(|c| p.fits(c, kind, true)) {
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
    out.push(System { kind: k, face, axis: axis.clone(), openings, balls, halls: Vec::new() });
    for (q, heading) in forks {
        tube(p, kind, k, rng, axis[q].x, axis[q].y, heading, Some(axis[q].z), branches - 1, out);
    }
}

impl Probe<'_> {
    /// Whether a round tube of radius r at (i, j) comes within `WALL` of another's wall (but the tube `except`).
    fn crowds(&self, i: f64, j: f64, r: f64, except: Option<usize>) -> bool {
        let (x, y) = ((i / AXES).floor() as i32, (j / AXES).floor() as i32);
        let n = ((r + self.widest + WALL) / AXES).ceil() as i32;
        (-n..=n).flat_map(|a| (-n..=n).map(move |c| (x + a, y + c))).filter_map(|c| self.axes.get(&c)).flatten().any(|&(pi, pj, pr, s)| Some(s) != except && (pi - i).hypot(pj - j) < r + pr + WALL)
    }
    /// The water table's radius under the middle of column (i, j).
    fn table(&mut self, i: i32, j: i32) -> f64 {
        let (b, f) = (self.b, self.face as usize);
        *self.tables.entry((i, j)).or_insert_with(|| b.radius - SEA_DEPTH + water_table(b, f, i as f64 + 0.5, j as f64 + 0.5))
    }
    /// Lets go of the karst passages numbered `first` and on (a system that was not made).
    fn forget(&mut self, first: usize) {
        self.axes.values_mut().for_each(|v| v.retain(|x| x.3 < first));
    }
    /// Keeps system `s`'s axis (its points' radii `rs`) for the tubes after it.
    fn keep(&mut self, s: usize, axis: &[DVec3], rs: &[f64]) {
        for (q, &r) in axis.iter().zip(rs) {
            self.widest = self.widest.max(r);
            self.axes.entry(((q.x / AXES).floor() as i32, (q.y / AXES).floor() as i32)).or_default().push((q.x, q.y, r, s));
        }
    }
    /// Whether column (i, j) is a host of the kind at m metres under its ground, in one of its provinces.
    fn host(&mut self, i: f64, j: f64, m: f64, kind: &Kind) -> bool {
        let (rock, ground, wet) = self.col(i.floor() as i32, j.floor() as i32);
        !wet && kind.provinces.contains(&rock.province) && kind.hosts.contains(&self.b.rock(rock, (ground - m) as f32, 0))
    }
}

/// A lava tube of a world of generation 2 (ADR 0160-generation-number-and-arc-basalt): one round passage, its
/// width drawn per tube and waving slowly along it, its lower third filled flat (a ball of the radius, cut a third
/// of the radius under its middle), a few metres under the ground. A start first climbs the ground's slope up the
/// basalt (a tube runs from high on its flow), then it runs downhill as `tube` does, gently winding, and also ends
/// where it would come within `WALL` of another round tube; a branch (`fork`: its parent and the floor there) bends
/// off to its side and is let near its parent for `FORK` metres. Each skylight is at least 2.3 m across its
/// radius, so it opens a 3 x 3 of columns at the least.
#[allow(clippy::too_many_arguments)]
fn round_tube(p: &mut Probe, kind: &Kind, k: u8, rng: &mut Rng, mut i: f64, mut j: f64, fork: Option<(usize, f64, f64, f64)>, branches: u32, out: &mut Vec<System>) {
    let (b, face, sea) = (p.b, p.face, p.b.radius - SEA_DEPTH);
    let w0 = lerp(rng, kind.width);
    let roof = kind.depth[0] + rng.f() * (kind.depth[1] - kind.depth[0] - kind.height[1]).clamp(0.0, 1.5);
    let len = lerp(rng, kind.length) as usize;
    let (phase, wave) = (rng.f() * std::f64::consts::TAU, rng.f() * std::f64::consts::TAU);
    let width = |s: usize| (w0 * (1.0 + 0.12 * (s as f64 / 41.0 + wave).sin())).clamp(kind.width[0], kind.width[1]);
    let mid = roof + w0 / 3.0;
    let (mut a, mut floor) = match fork {
        Some((_, a, f, _)) => (a, Some(f)),
        None => {
            // up the slope while the ground there and round it (a width out each way, the tube's walls) is the
            // host at the tube's depth: the flow's high end, short of its edge
            let inside = |p: &mut Probe, i: f64, j: f64| [(0.0, 0.0), (w0, 0.0), (-w0, 0.0), (0.0, w0), (0.0, -w0)].iter().all(|&(a, c)| p.host(i + a, j + c, mid, kind));
            if !inside(p, i, j) {
                return;
            }
            for _ in 0..400 {
                let (gx, gy) = p.grad_over(i, j, FLANK);
                let g = gx.hypot(gy);
                if g < 2.0 * kind.slope || p.off(i, j, w0) {
                    break;
                }
                let (ni, nj) = (i + gx / g, j + gy / g);
                if !inside(p, ni, nj) {
                    break;
                }
                (i, j) = (ni, nj);
            }
            let (gx, gy) = p.grad_over(i, j, FLANK);
            ((-gy).atan2(-gx), None)
        }
    };
    let except = fork.map(|f| f.0);
    let (mut axis, mut balls, mut rs, mut forks) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for step in 0..len {
        let w = width(step);
        let r = w / 2.0;
        let hgt = w * 2.0 / 3.0;
        if p.off(i, j, r) || p.crowds(i, j, r, except.filter(|_| step < FORK)) {
            break;
        }
        let ground = b.ground_r(face as usize, i, j);
        if ground - sea < DRY + r || p.wet(i, j) {
            break;
        }
        let (gx, gy) = p.grad_over(i, j, FLANK);
        if gx.hypot(gy) < kind.slope {
            break;
        }
        let f = floor.map_or(ground - roof - hgt, |f: f64| f.min(ground - roof - hgt));
        if ground - f > kind.depth[1] {
            break;
        }
        // round: the ball's middle a third of its radius over the floor, so its lower third is filled
        let c = Cave { face, at: DVec3::new(i, j, b.layer_of(f) + r / 3.0), r, h: r, floor: r / 3.0, kind: k };
        if !p.fits(&c, kind, false) {
            break;
        }
        floor = Some(f);
        axis.push(DVec3::new(i, j, f));
        balls.push(c);
        rs.push(r);
        if branches > 0 && step > FORK && rng.f() < kind.branch / 100.0 {
            let side = if rng.f() < 0.5 { 1.0 } else { -1.0 };
            forks.push((axis.len() - 1, a + side * (0.6 + rng.f() * 0.4), side));
        }
        // downhill, winding a little; a branch bends on off to its side over its first 80 m
        let mut wind = 0.25 * (step as f64 / 23.0 + phase).sin() + fork.map_or(0.0, |f| f.3 * 0.5 * (1.0 - step as f64 / 80.0).max(0.0));
        // and round water ahead (a river in the valley, a lake): towards the drier side, as a flow is held off
        // by the ground a valley's stream later cut
        let wet_at = |p: &mut Probe, t: f64| p.wet(i + AHEAD * (a + t).cos(), j + AHEAD * (a + t).sin());
        if wet_at(p, 0.0) {
            wind += match (wet_at(p, -0.6), wet_at(p, 0.6)) {
                (false, true) => -0.6,
                (true, false) => 0.6,
                _ => 0.0,
            };
        }
        a += wrap((-gy).atan2(-gx) + wind - a).clamp(-0.1, 0.1);
        (i, j) = (i + a.cos(), j + a.sin());
    }
    if axis.len() < MIN_RUN {
        return;
    }
    // the skylights, as `tube`'s
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
        for (q, c) in axis[s..=e].iter().zip(&balls[s..=e]) {
            let zg = b.layer_of(b.ground_r(face as usize, q.x, q.y)) + 1.0;
            let zc = c.at.z;
            shaft.push(Cave { face, at: DVec3::new(q.x, q.y, zg), r: (c.r * if trench { 0.9 } else { 0.8 }).max(2.3), h: 1.6 * (zg - zc), floor: zg - zc, kind: k });
        }
        if axis[s..=e].iter().any(|q| p.wet(q.x, q.y)) || !shaft.iter().all(|c| p.fits(c, kind, true)) {
            continue;
        }
        // the roof that fell: a heap under the opening, its foot the tube's radius and 3 m out
        let m = (rs[s] + 3.0).ceil() as usize;
        for (q, c) in balls.iter_mut().enumerate().take((e + m).min(n - 1) + 1).skip(s.saturating_sub(m)) {
            let out_by = s.saturating_sub(q).max(q.saturating_sub(e)) as f64;
            c.floor -= 0.45 * (c.r * 4.0 / 3.0) * (1.0 - out_by / m as f64).max(0.0);
        }
        balls.extend(shaft);
        openings.push((s, e));
    }
    if openings.is_empty() {
        return;
    }
    let me = out.len();
    p.keep(me, &axis, &rs);
    out.push(System { kind: k, face, axis: axis.clone(), openings, balls, halls: Vec::new() });
    for (q, heading, side) in forks {
        round_tube(p, kind, k, rng, axis[q].x, axis[q].y, Some((me, heading, axis[q].z, side)), branches - 1, out);
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
        if p.fits(&c, kind, top < 0.0) {
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
        out.push(System { kind: k, face, axis, openings, balls, halls: Vec::new() });
    }
}

/// The most a karst passage's floor moves a metre along it, metres: near-level, as a gallery cut at the water
/// table is.
const KARST_STEP: f64 = 0.08;
/// A karst passage's floor lies this far over the water table where it can (it follows the table down at
/// `KARST_STEP`, and is never under it).
const LIFT: f64 = 0.3;
/// Halls along a karst passage lie at least this many metres apart.
const HALLS: usize = 80;

/// A dry karst system near (i, j) (generation 3, draft-karst-caves): a main passage from there down the slope of
/// the water table (toward the valley or the coast it drains to) at or just over the table, near-level, with
/// branches up the slope (tributaries) and a hall where one leaves it; a passage ends where its limestone, its
/// depth window under the ground or the dry ground ends, where its floor would fall under the table, near water, a
/// seam or the spawn, and where it would come within `WALL` of another passage (a branch is let near its parent
/// for its first `FORK` metres). Its sinkholes, where the limestone reaches up to the soil: a bowl in the ground
/// and a shaft down to the passage. A system with no sinkhole is not made (nothing would lead into it).
fn karst(p: &mut Probe, kind: &Kind, k: u8, rng: &mut Rng, i: f64, j: f64, out: &mut Vec<System>) {
    let first = p.next;
    let mut sys = System { kind: k, face: p.face, axis: Vec::new(), openings: Vec::new(), balls: Vec::new(), halls: Vec::new() };
    let (w, hgt) = (lerp(rng, kind.width), lerp(rng, kind.height));
    let len = lerp(rng, kind.length) as usize;
    if !passage(p, kind, k, rng, i, j, None, len, 2, w, hgt, &mut sys) || sys.openings.is_empty() {
        p.forget(first);
        return;
    }
    out.push(sys);
}

/// One passage of a karst system, put in `sys` if it is at least `MIN_RUN` metres long (and its sinkholes, its
/// halls and its branches after it); `fork` a branch's parent passage, heading, floor and side.
#[allow(clippy::too_many_arguments)]
fn passage(p: &mut Probe, kind: &Kind, k: u8, rng: &mut Rng, mut i: f64, mut j: f64, fork: Option<(usize, f64, f64, f64)>, len: usize, branches: u32, w: f64, hgt: f64, sys: &mut System) -> bool {
    let (b, face, sea) = (p.b, p.face, p.b.radius - SEA_DEPTH);
    let id = p.next;
    p.next += 1;
    // a round arch over a flat floor: the ball's lower quarter of the height cut flat
    let (r, hz, fl) = (w / 2.0, 0.75 * hgt, 0.25 * hgt);
    let phase = rng.f() * std::f64::consts::TAU;
    // along the slope's contour, one way or the other: the main passage leaning down it (toward where it drains),
    // a tributary up it
    let along = fork.map_or(if rng.f() < 0.5 { 1.0 } else { -1.0 }, |f| f.3);
    let lean = along * (std::f64::consts::FRAC_PI_2 + if fork.is_some() { 0.6 } else { -0.35 });
    let (mut a, mut floor) = match fork {
        Some((_, a, f, _)) => (a, Some(f)),
        None => {
            let (gx, gy) = p.grad_over(i, j, FLANK);
            ((-gy).atan2(-gx) + lean, None)
        }
    };
    // across the passage at (i, j) (its middle and a radius out four ways): the lowest ground and the highest
    // water table, the floor's height to keep over it
    let across = |p: &mut Probe, i: f64, j: f64| {
        [(0.0, 0.0), (r, 0.0), (-r, 0.0), (0.0, r), (0.0, -r)].iter().fold((f64::MAX, f64::MIN), |(g, t), &(x, y)| {
            let (ci, cj) = ((i + x).floor() as i32, (j + y).floor() as i32);
            (g.min(p.col(ci, cj).1), t.max(p.table(ci, cj)))
        })
    };
    // where a passage can run: dry, of the kind's province, its host from its floor over the water table to over
    // its roof, the roof under the soil band with a metre and a half to spare
    let runs = |p: &mut Probe, i: f64, j: f64| {
        let (rock, _, wet) = p.col(i.floor() as i32, j.floor() as i32);
        let (ground, t) = across(p, i, j);
        let t = t + LIFT;
        !wet && kind.provinces.contains(&rock.province) && (hgt + BAND + 1.5..=kind.depth[1]).contains(&(ground - t)) && [t + 0.5, t + hgt + 0.5].iter().all(|&m| kind.hosts.contains(&b.rock(rock, m as f32, 0)))
    };
    // a start seeks the band of the slope where the passage can run: up it where the table lies too near the
    // ground, down it where the ground stands so high that the table lies over the limestone
    if fork.is_none() {
        for _ in 0..160 {
            if runs(p, i, j) {
                break;
            }
            let (rock, _, wet) = p.col(i.floor() as i32, j.floor() as i32);
            if wet || !kind.provinces.contains(&rock.province) {
                return false;
            }
            let (ground, t) = across(p, i, j);
            let (gx, gy) = p.grad_over(i, j, FLANK);
            let g = gx.hypot(gy).max(1e-9);
            let up = if ground - t - LIFT < hgt + BAND + 1.5 { 1.0 } else { -1.0 };
            (i, j) = (i + up * gx / g, j + up * gy / g);
        }
    }
    let (mut axis, mut balls, mut forks) = (Vec::new(), Vec::new(), Vec::new());
    for step in 0..len {
        if p.off(i, j, r) || p.crowds(i, j, r, fork.map(|f| f.0).filter(|_| step < FORK)) {
            break;
        }
        let ground = b.ground_r(face as usize, i, j);
        if ground - sea < DRY + hgt || p.wet(i, j) {
            break;
        }
        let (_, table) = across(p, i, j);
        let f = floor.map_or(table + LIFT, |f: f64| f + (table + LIFT - f).clamp(-KARST_STEP, KARST_STEP));
        if f < table || !(kind.depth[0]..=kind.depth[1]).contains(&(ground - f)) {
            break;
        }
        let c = Cave { face, at: DVec3::new(i, j, b.layer_of(f) + fl), r, h: hz, floor: fl, kind: k };
        if !p.fits(&c, kind, false) {
            break;
        }
        floor = Some(f);
        axis.push(DVec3::new(i, j, f));
        balls.push(c);
        if branches > 0 && step > FORK && rng.f() < kind.branch / 100.0 {
            forks.push((axis.len() - 1, if rng.f() < 0.5 { 1.0 } else { -1.0 }));
        }
        // along the contour with its lean, winding; where the limestone or the depth ends 8 m ahead, toward the
        // nearest heading where it goes on (the gallery keeps to the rock the water table lies in)
        let (gx, gy) = p.grad_over(i, j, FLANK);
        let want = (-gy).atan2(-gx) + lean + 0.35 * (step as f64 / 19.0 + phase).sin();
        let turn = [0.0, 0.5, -0.5, 1.0, -1.0, 1.5, -1.5].into_iter().find(|t| runs(p, i + 8.0 * (want + t).cos(), j + 8.0 * (want + t).sin())).unwrap_or(0.0);
        a += wrap(want + turn - a).clamp(-0.15, 0.15);
        (i, j) = (i + a.cos(), j + a.sin());
    }
    let n = axis.len();
    if n < MIN_RUN {
        return false;
    }
    p.keep(id, &axis, &vec![r; n]);
    let base = sys.axis.len();
    sys.axis.extend(&axis);
    sys.balls.extend(&balls);
    // the sinkholes: the first within a third of the spacing, then at least the spacing apart, where one fits
    let mut s = (kind.skylight * 0.3 * rng.f()) as usize;
    while s < n {
        if let Some(cs) = sinkhole(p, kind, k, rng, &balls[s]) {
            sys.openings.push((base + s, base + s));
            sys.balls.extend(cs);
            s += (kind.skylight * (1.0 + 0.5 * rng.f())) as usize;
        } else {
            s += 3;
        }
    }
    // halls where the roof broke down: along the passage, where there is room under the ground and limestone
    // over the floor for the least hall (checked first in the middle column), `HALLS` metres apart at least
    let room = |p: &mut Probe, q: DVec3| {
        let (rock, ground, _) = p.col(q.x.floor() as i32, q.y.floor() as i32);
        ground - q.z >= kind.hall_height[0] + BAND + 0.5 && (0..kind.hall_height[0].ceil() as i32).all(|m| kind.hosts.contains(&b.rock(rock, (q.z + m as f64 + 0.5) as f32, 0)))
    };
    let mut s = HALLS / 3;
    while s + 10 < n {
        match room(p, axis[s]).then(|| hall(p, kind, k, rng, axis[s], id)).flatten() {
            Some(h) => {
                p.keep(id, &[axis[s]], &[h.r]);
                sys.halls.push(sys.balls.len());
                sys.balls.push(h);
                s += HALLS;
            }
            None => s += 4,
        }
    }
    for (q, side) in forks {
        let back = axis[q] - axis[q.saturating_sub(2)];
        let heading = back.y.atan2(back.x) + std::f64::consts::PI + side * (0.6 + 0.4 * rng.f());
        if let Some(h) = room(p, axis[q]).then(|| hall(p, kind, k, rng, axis[q], id)).flatten() {
            p.keep(id, &[axis[q]], &[h.r]);
            sys.halls.push(sys.balls.len());
            sys.balls.push(h);
        }
        let (w2, h2) = ((w * 0.85).max(kind.width[0]), (hgt * 0.9).max(kind.height[0]));
        passage(p, kind, k, rng, axis[q].x, axis[q].y, Some((id, heading, axis[q].z, side)), len / 2, branches - 1, w2, h2, sys);
    }
    true
}

/// A sinkhole over passage ball c, if one fits there: a bowl `sinkhole` wide at the ground (deeper where the ground
/// slopes, so that all of it is open) and a shaft `shaft` wide from it down to the passage.
fn sinkhole(p: &mut Probe, kind: &Kind, k: u8, rng: &mut Rng, c: &Cave) -> Option<[Cave; 2]> {
    let (b, face) = (p.b, p.face);
    // (first, cheaply: the limestone reaches up to the soil band in the middle column, and it is dry)
    let (rock, ground, wet) = p.col(c.at.x.floor() as i32, c.at.y.floor() as i32);
    if wet || !kind.hosts.contains(&b.rock(rock, (ground - BAND - 0.5) as f32, 0)) {
        return None;
    }
    let wb = lerp(rng, kind.sinkhole);
    let rb = wb / 2.0 + 0.5;
    let ring = (0..16).map(|q| q as f64 * std::f64::consts::TAU / 16.0).flat_map(|t| [0.5, 1.0].map(|s| (s * rb * t.cos(), s * rb * t.sin())));
    let gs: Vec<f64> = std::iter::once((0.0, 0.0)).chain(ring).map(|(x, y)| b.ground_r(face as usize, c.at.x + x, c.at.y + y)).collect();
    let (lo, hi) = gs.iter().fold((f64::MAX, f64::MIN), |(l, h), &g| (l.min(g), h.max(g)));
    let depth = (wb / 4.0).max((hi - lo + 1.5) / 0.5);
    let bowl = Cave { face, at: DVec3::new(c.at.x, c.at.y, b.layer_of(hi) + 0.5), r: rb, h: depth, floor: depth, kind: k };
    let zg = b.layer_of(ground) + 1.0;
    let shaft = Cave { face, at: DVec3::new(c.at.x, c.at.y, zg), r: lerp(rng, kind.shaft) / 2.0, h: 1.6 * (zg - c.at.z), floor: zg - c.at.z, kind: k };
    (p.fits(&bowl, kind, true) && p.fits(&shaft, kind, true)).then_some([bowl, shaft])
}

/// A hall at passage point q (where a branch leaves passage `id`): a flat-floored room `hall` wide and
/// `hall_height` high, made smaller (2 m narrower and 1 m lower a try, down to the least) where it does not fit
/// the limestone, or keeps no wall from other passages; none if even the least does not.
fn hall(p: &mut Probe, kind: &Kind, k: u8, rng: &mut Rng, q: DVec3, id: usize) -> Option<Cave> {
    let (b, face) = (p.b, p.face);
    let (mut wd, mut ht) = (lerp(rng, kind.hall), lerp(rng, kind.hall_height));
    loop {
        let r = wd / 2.0;
        // its floor flat at the passage's, or over the highest water table under it
        let f = [(0.0, 0.0), (r, 0.0), (-r, 0.0), (0.0, r), (0.0, -r)].iter().map(|&(x, y)| p.table((q.x + x).floor() as i32, (q.y + y).floor() as i32) + LIFT).fold(q.z, f64::max);
        let c = Cave { face, at: DVec3::new(q.x, q.y, b.layer_of(f) + 1.0), r, h: ht - 1.0, floor: 1.0, kind: k };
        if !p.off(q.x, q.y, r) && !p.crowds(q.x, q.y, r, Some(id)) && p.fits(&c, kind, false) {
            return Some(c);
        }
        if wd <= kind.hall[0] && ht <= kind.hall_height[0] {
            return None;
        }
        (wd, ht) = ((wd - 2.0).max(kind.hall[0]), (ht - 1.0).max(kind.hall_height[0]));
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
    fn two_widest_round_tubes_converging_keep_the_full_wall() {
        let b = Body::new(DVec3::ZERO, 256, 1);
        let mut p = Probe { b: &b, face: 0, cols: HashMap::new(), axes: HashMap::new(), widest: 0.0, tables: HashMap::new(), next: 0 };
        let r = CAVES.kinds.iter().filter(|k| k.shape == Shape::Tube && k.generations[0] >= 2).map(|k| k.width[1]).fold(0.0, f64::max) / 2.0;
        assert!(r >= 3.5, "the widest round tube {r} m in radius");
        // tube 0 just under j = 0 (the bottom of a grid cell, the far side from tube 1); tube 1 comes in at 30
        // degrees, a tenth of a metre a step
        let axis: Vec<DVec3> = (0..200).map(|i| DVec3::new(i as f64 * 0.5, -0.05, 0.0)).collect();
        p.keep(0, &axis, &vec![r; axis.len()]);
        let mut t = 0.0;
        loop {
            let (i, j) = (60.0 - t * 0.866, 40.0 - t * 0.5);
            if p.crowds(i, j, r, Some(1)) {
                break;
            }
            let d = j + 0.05;
            assert!(d >= r + r + WALL - 0.11, "tube 1 not stopped at {d:.2} m from tube 0's axis (wall {:.2} m)", d - 2.0 * r);
            t += 0.1;
        }
    }

    #[test]
    fn the_caves_file_loads_and_a_broken_one_is_refused_and_says_why() {
        let k = &CAVES.kinds;
        assert_eq!(k.iter().map(|k| (k.name.as_str(), k.shape, k.generations)).collect::<Vec<_>>(), [("lava tube", Shape::Tube, [1, 1]), ("fissure", Shape::Fissure, [1, 255]), ("lava tube", Shape::Tube, [2, 255]), ("karst", Shape::Karst, [3, 255])]);
        assert_eq!((k[3].hosts.clone(), k[3].provinces.clone()), (vec![crate::blocks::BLOCKS.id("limestone")], vec![crate::bake::rock::NAMES.iter().position(|n| *n == "basin").unwrap() as u8]));
        for t in [&k[0], &k[2]] {
            assert_eq!(t.hosts, [crate::blocks::BLOCKS.id("basalt")]);
            assert_eq!(t.provinces, [crate::bake::rock::RIFT, crate::bake::rock::ARC]);
        }
        let good = include_str!("../../../data/caves.toml").replace("\r\n", "\n");
        for (from, to, says) in [
            ("hosts = [\"basalt\"]", "hosts = [\"basalts\"]", "no rock 'basalts'"),
            ("hosts = [\"basalt\"]", "hosts = [\"dirt\"]", "no rock 'dirt'"),
            ("provinces = [\"rift\"", "provinces = [\"rifts\"", "no province 'rifts'"),
            ("depth = [4.5, 16.0]", "depth = [16.0, 4.5]", "depth"),
            ("depth = [4.5, 16.0]", "depth = [4.5, 400.0]", "depth"),
            ("width = [3.0, 8.0]", "width = [8.0, 3.0]", "width"),
            ("skylight = 200.0\n", "", "skylight"),
            ("open = 0.35\n", "open = 0.35\nskylight = 9.0\n", "only a tube"),
            ("length = [150.0, 700.0]", "length = [20.0, 700.0]", "at least 40"),
            ("generations = [1, 1]", "generations = [2, 1]", "generations"),
            ("generations = [1, 1]", "generations = [0, 1]", "generations"),
            ("hall = [10.0, 30.0]", "hall = [30.0, 10.0]", "hall"),
            ("hall = [10.0, 30.0]", "hall = [10.0, 90.0]", "hall"),
            ("shaft = [3.0, 5.0]\n", "", "a karst has"),
            ("shaft = [3.0, 5.0]\n", "shaft = [3.0, 5.0]\ntrench = 0.2\n", "no trench, slope or open"),
            ("sinkhole = [9.0, 18.0]", "sinkhole = [4.0, 18.0]", "sinkhole"),
            ("open = 0.35\n", "open = 0.35\nhall = [10.0, 20.0]\n", "only a karst's"),
        ] {
            let e = Kinds::parse(&good.replacen(from, to, 1)).err().unwrap_or_else(|| panic!("{from} -> {to} loads"));
            assert!(e.contains(says), "{from} -> {to}: {e}");
        }
        // a kind's seed keeps 4 bits for it: 16 kinds at most
        // (the fissure's row, up to the comment over the next)
        let fissure = &good[good.find("[[kind]]\nname = \"fissure\"").unwrap()..];
        let fissure = &fissure[..fissure.find("\n#").unwrap() + 1];
        let many = good.clone() + &fissure.repeat(13);
        let e = Kinds::parse(&many).err().expect("17 kinds load");
        assert!(e.contains("at most 16"), "{e}");
        assert!(Kinds::parse(&(good.clone() + &fissure.repeat(12))).is_ok(), "16 kinds are refused");
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
    // from generation 3 the limestone has the karst of data/caves.toml instead (draft-karst-caves); the tubes
    // here are only a planet's without the mantle
    for face in (0..6).filter(|_| b.gen < 3) {
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
    // (generation 1 came in on the mantle flag, and these gates stay on it: the same worlds. A rule new since asks
    // `b.gen`, ADR 0160-generation-number-and-arc-basalt; the kinds of a generation are chosen in `systems_of`)
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
