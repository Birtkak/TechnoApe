//! LOD octree selection, job scheduling on worker threads, GPU sub-allocation of chunk meshes.
use crate::mesh::{mesh, Mesh};
use crate::world::{fold, generate, Block, Bounds, FxMap, Gen, Key, World, CS};
use glam::{DVec3, IVec3, Vec4};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Instant;


#[derive(Clone, Copy)]
pub struct Opts {
    /// A chunk splits when the camera is nearer than k chunk edges.
    pub k: f64,
    pub greedy: bool,
    pub horizon: bool,
    pub prefetch_s: f64,
    pub max_inflight: usize,
}

/// First-fit free list over the quad buffer, in quad units.
pub struct Alloc {
    free: Vec<(u32, u32)>, // sorted (offset, len)
    pub used: u64,
    pub cap: u32,
}
impl Alloc {
    pub fn new(cap: u32) -> Self {
        Alloc { free: vec![(0, cap)], used: 0, cap }
    }
    pub fn alloc(&mut self, n: u32) -> Option<u32> {
        let i = self.free.iter().position(|f| f.1 >= n)?;
        let (o, l) = self.free[i];
        if l == n {
            self.free.remove(i);
        } else {
            self.free[i] = (o + n, l - n);
        }
        self.used += n as u64;
        Some(o)
    }
    pub fn release(&mut self, o: u32, n: u32) {
        self.used -= n as u64;
        let i = self.free.partition_point(|f| f.0 < o);
        self.free.insert(i, (o, n));
        if i + 1 < self.free.len() && self.free[i].0 + self.free[i].1 == self.free[i + 1].0 {
            self.free[i].1 += self.free[i + 1].1;
            self.free.remove(i + 1);
        }
        if i > 0 && self.free[i - 1].0 + self.free[i - 1].1 == self.free[i].0 {
            self.free[i - 1].1 += self.free[i].1;
            self.free.remove(i);
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    Pending,
    Empty,
    Ready { off: u32, len: u32 },
}

/// Every chunk the selection visits has a slot (it caches the bounds); only wanted leaves get a mesh.
struct Slot {
    state: State,
    last_seen: u64,
    want_v: u32,
    shown_v: u32,
    inflight: bool,
    bounds: Bounds,
}

impl Slot {
    fn needs_job(&self) -> bool {
        (self.state == State::Pending || self.shown_v < self.want_v) && !self.inflight
    }
}

pub struct JobResult {
    key: Key,
    version: u32,
    mesh: Option<Mesh>,
    pub us: f64,
}

/// One drawable range of the quad buffer.
pub struct DrawChunk {
    pub key: Key,
    pub off: u32,
    pub len: u32,
}

#[derive(Default, Clone, Copy)]
pub struct FrameStats {
    pub missing: u32,
    pub holes: u32,
    pub applied: u32,
    pub applied_nonempty: u32,
    pub requests: u32,
}

/// The chunks, at every level (or only `level`), whose cells or padding sample block `b`.
fn sampling(world: &World, b: Block, level: Option<u8>) -> Vec<Key> {
    let body = &world.bodies[b.body as usize];
    let mut keys = Vec::new();
    for l in 0..=body.root_level {
        if level.is_some_and(|only| only != l) {
            continue;
        }
        let s = 1i32 << l;
        let q = b.pos - s / 2; // coarse cells sample the block in their middle
        if q.x.rem_euclid(s) | q.y.rem_euclid(s) | q.z.rem_euclid(s) != 0 {
            continue;
        }
        let cell = q.div_euclid(IVec3::splat(s));
        let own = Key { body: b.body, face: b.face, level: l, pos: cell.div_euclid(IVec3::splat(CS)) };
        keys.push(own);
        for d in [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z] {
            let c = cell + d;
            let (f, ci, cj) = fold(b.face as usize, c.x, c.y, body.n >> l);
            let k = Key { face: f as u8, pos: IVec3::new(ci, cj, c.z).div_euclid(IVec3::splat(CS)), ..own };
            if k != own && !keys.contains(&k) {
                keys.push(k);
            }
        }
    }
    keys
}

pub struct Streamer {
    slots: FxMap<Key, Slot>,
    tx: Sender<JobResult>,
    rx: Receiver<JobResult>,
    pub inflight: usize,
    pub alloc: Alloc,
    pub frame: u64,
    roots: Vec<Key>,
    pub opts: Opts,
    pub job_us_total: f64,
    pub jobs_done: u64,
    pub alloc_fail: u64,
    pub ahead_reqs: Vec<(f64, Key)>,
}

pub struct View {
    pub cam: DVec3,
    pub planes: [Vec4; 6], // camera-relative frustum planes, inside when dot >= 0
    hz: Vec<Horizon>,
}

/// The cone from the camera that touches a body's core, so the per-chunk test has no divide.
struct Horizon {
    u: DVec3,   // unit vector from the camera to the core's centre
    plane: f64, // distance along u to the circle where the cone touches the core
    sin: f64,   // of the cone's half angle
    cos: f64,
}

impl View {
    pub fn new(cam: DVec3, planes: [Vec4; 6], world: &World) -> Self {
        let hz = world.bodies.iter().filter_map(|b| {
            let cv = b.centre - cam;
            let dc = cv.length();
            if dc <= b.r0 {
                return None;
            }
            let t2 = dc * dc - b.r0 * b.r0;
            Some(Horizon { u: cv / dc, plane: t2 / dc, sin: b.r0 / dc, cos: t2.sqrt() / dc })
        }).collect();
        View { cam, planes, hz }
    }
    fn in_frustum(&self, b: &Bounds) -> bool {
        let c = (b.c - self.cam).as_vec3();
        let r = -b.rad as f32;
        self.planes.iter().all(|p| p.truncate().dot(c) + p.w >= r)
    }
}

/// Is the whole sphere hidden behind a body's core (never dug, so a safe occluder)?
/// Hidden = past the touching circle and inside the cone, by its own radius.
fn horizon_hidden(cam: DVec3, h: &Horizon, b: &Bounds) -> bool {
    let v = b.c - cam;
    let along = v.dot(h.u);
    if along - b.rad < h.plane {
        return false;
    }
    let across = (v.length_squared() - along * along).max(0.0).sqrt();
    h.sin * along - h.cos * across >= b.rad
}

impl Streamer {
    pub fn new(world: &World, cap: u32, opts: Opts) -> Self {
        let (tx, rx) = channel();
        let mut roots = Vec::new();
        for (bi, b) in world.bodies.iter().enumerate() {
            let top = (b.layers - 1) >> (5 + b.root_level);
            for face in 0..6 {
                for z in 0..=top {
                    for y in 0..b.roots() {
                        for x in 0..b.roots() {
                            roots.push(Key { body: bi as u8, face, level: b.root_level, pos: IVec3::new(x, y, z) });
                        }
                    }
                }
            }
        }
        Streamer {
            slots: FxMap::default(),
            tx,
            rx,
            inflight: 0,
            alloc: Alloc::new(cap),
            frame: 0,
            roots,
            opts,
            job_us_total: 0.0,
            jobs_done: 0,
            alloc_fail: 0,
            ahead_reqs: Vec::new(),
        }
    }

    /// Does the chunk reach into the streamed layers? (Sideways every child of a root is on its face.)
    fn in_band(world: &World, k: Key) -> bool {
        let s = k.span();
        k.pos.z * s < world.bodies[k.body as usize].layers && (k.pos.z + 1) * s > 0
    }

    fn resolved(&self, k: &Key) -> bool {
        matches!(self.slots.get(k), Some(s) if s.state != State::Pending)
    }

    fn hidden(&self, view: &View, b: &Bounds) -> bool {
        !view.in_frustum(b) || (self.opts.horizon && view.hz.iter().any(|h| horizon_hidden(view.cam, h, b)))
    }

    /// Walk the octree for one camera; draw = false only requests (prefetch).
    pub fn select(
        &mut self,
        world: &World,
        view: &View,
        draw: bool,
        prio_mul: f64,
        out: &mut Vec<DrawChunk>,
        reqs: &mut Vec<(f64, Key)>,
        st: &mut FrameStats,
    ) {
        let mut stack: Vec<(Key, bool)> = self.roots.iter().map(|k| (*k, false)).collect();
        // The look-ahead splits 10 % earlier: its camera is a straight-line guess, and a chunk the real camera
        // only grazes (a few frames inside the split distance) would otherwise never be requested ahead.
        let kk = if draw { self.opts.k } else { self.opts.k * 1.1 };
        let frame = self.frame;
        while let Some((k, covered)) = stack.pop() {
            let slot = self.slots.entry(k).or_insert_with(|| Slot {
                state: State::Pending,
                last_seen: frame,
                want_v: 0,
                shown_v: 0,
                inflight: false,
                bounds: world.bodies[k.body as usize].bounds(k),
            });
            slot.last_seen = frame;
            let (bd, needs_job, pending) = (slot.bounds, slot.needs_job(), slot.state == State::Pending);
            let d = ((bd.c - view.cam).length() - bd.rad).max(0.0);
            if k.level > 0 && d < kk * k.edge() {
                if !draw && d >= self.opts.k * k.edge() && needs_job {
                    // split only because of the look-ahead margin: the real camera may still want this chunk itself
                    reqs.push((d / k.edge() * prio_mul, k));
                }
                let mut kids = [k; 8];
                let mut nk = 0;
                for i in 0..8 {
                    let c = k.child(i);
                    if Self::in_band(world, c) {
                        kids[nk] = c;
                        nk += 1;
                    }
                }
                let kids = &kids[..nk];
                let all = kids.iter().all(|c| self.resolved(c));
                let mut cov = covered;
                if !all && !covered && draw && self.emit(view, k, None, out) {
                    cov = true;
                }
                for &c in kids {
                    stack.push((c, cov));
                }
                continue;
            }
            // wanted leaf
            let hidden = self.hidden(view, &bd);
            if needs_job {
                let mut p = d / k.edge() * prio_mul;
                if hidden {
                    p *= 3.0;
                }
                if pending && !covered && !hidden {
                    p *= 0.25;
                }
                reqs.push((p, k));
                st.requests += 1;
            }
            if pending {
                if draw && !hidden {
                    st.missing += 1;
                    if !covered {
                        // stand-in from finer chunks left over from before
                        if k.level > 0 && (0..8).all(|i| { let c = k.child(i); !Self::in_band(world, c) || self.resolved(&c) }) {
                            for i in 0..8 {
                                self.emit(view, k.child(i), None, out);
                            }
                        } else {
                            st.holes += 1;
                        }
                    }
                }
            } else if draw && !covered {
                self.emit(view, k, Some(hidden), out);
            }
        }
    }

    /// Append a ready chunk to the draw list if visible; true if it is ready (drawn or culled).
    fn emit(&mut self, view: &View, k: Key, hidden: Option<bool>, out: &mut Vec<DrawChunk>) -> bool {
        let Some(s) = self.slots.get_mut(&k) else { return false };
        s.last_seen = self.frame;
        let bd = s.bounds;
        let (off, len) = match s.state {
            State::Pending => return false,
            State::Empty => return true,
            State::Ready { off, len } => (off, len),
        };
        if !hidden.unwrap_or_else(|| self.hidden(view, &bd)) {
            out.push(DrawChunk { key: k, off, len });
        }
        true
    }

    pub fn dispatch(&mut self, world: &World, reqs: &mut Vec<(f64, Key)>) {
        reqs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        for &(_, k) in reqs.iter() {
            if self.inflight >= self.opts.max_inflight {
                break;
            }
            let Some(s) = self.slots.get_mut(&k) else { continue };
            if !s.needs_job() {
                continue; // requested by both traversals, or a kept look-ahead request that is done
            }
            s.inflight = true;
            self.inflight += 1;
            let version = s.want_v;
            let edits = world.edits_for(k);
            let tx = self.tx.clone();
            let bodies = world.bodies.clone();
            let greedy = self.opts.greedy;
            rayon::spawn(move || {
                let t = Instant::now();
                let mesh = match generate(&bodies[k.body as usize], k, &edits) {
                    Gen::Cells(c) => Some(mesh(&c, greedy)).filter(|m| !m.quads.is_empty()),
                    _ => None,
                };
                let _ = tx.send(JobResult { key: k, version, mesh, us: t.elapsed().as_secs_f64() * 1e6 });
            });
        }
        reqs.clear();
    }

    /// Apply finished jobs within an upload budget; `upload` writes quads at an offset.
    pub fn apply(&mut self, budget_bytes: usize, st: &mut FrameStats, mut upload: impl FnMut(u32, &[[u32; 2]])) {
        let mut bytes = 0;
        while bytes < budget_bytes {
            let Ok(r) = self.rx.try_recv() else { break };
            self.inflight -= 1;
            self.jobs_done += 1;
            self.job_us_total += r.us;
            let Some(s) = self.slots.get_mut(&r.key) else { continue };
            s.inflight = false;
            if r.version < s.shown_v {
                continue;
            }
            let new = match r.mesh {
                None => State::Empty,
                Some(m) => {
                    let n = m.quads.len() as u32;
                    let Some(off) = self.alloc.alloc(n) else {
                        self.alloc_fail += 1;
                        continue;
                    };
                    upload(off, &m.quads);
                    bytes += m.quads.len() * 8;
                    st.applied_nonempty += 1;
                    State::Ready { off, len: n }
                }
            };
            let s = self.slots.get_mut(&r.key).unwrap();
            if let State::Ready { off, len } = s.state {
                self.alloc.release(off, len);
            }
            s.state = new;
            s.shown_v = r.version;
            st.applied += 1;
        }
    }

    /// An edit changed a block: every chunk that samples it, in its cells or its padding, gets a new version.
    pub fn touch_block(&mut self, world: &World, b: Block) {
        for k in sampling(world, b, None) {
            if let Some(sl) = self.slots.get_mut(&k) {
                sl.want_v += 1;
            }
        }
    }

    /// True when every full-detail chunk that samples block `b` draws its latest version: an edit there is
    /// on screen (the feel report's click-to-change, fundamentals section 1).
    pub fn shows(&self, world: &World, b: Block) -> bool {
        sampling(world, b, Some(0)).iter().all(|k| self.slots.get(k).is_none_or(|s| s.shown_v >= s.want_v))
    }

    pub fn evict(&mut self, age: u64) {
        let f = self.frame;
        let alloc = &mut self.alloc;
        self.slots.retain(|_, s| {
            let keep = s.inflight || f.saturating_sub(s.last_seen) < age;
            if !keep {
                if let State::Ready { off, len } = s.state {
                    alloc.release(off, len);
                }
            }
            keep
        });
    }

    pub fn idle(&self) -> bool {
        self.inflight == 0
    }
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn horizon_hides_only_what_is_behind_the_core() {
        let w = World::new(1536, 512, 1337);
        let r0 = w.bodies[0].r0;
        let v = View::new(DVec3::new(0.0, r0 + 100.0, 0.0), [Vec4::ZERO; 6], &w);
        let h = &v.hz[0];
        let ball = |x: f64, y: f64| Bounds { c: DVec3::new(x, y, 0.0), rad: 14.0 };
        assert!(horizon_hidden(v.cam, h, &ball(0.0, -r0 - 20.0))); // far side of the planet
        assert!(!horizon_hidden(v.cam, h, &ball(0.0, r0 + 20.0))); // between camera and planet
        assert!(!horizon_hidden(v.cam, h, &ball((r0 + 20.0) * h.cos, (r0 + 20.0) * h.sin))); // above the horizon line
        assert!(horizon_hidden(v.cam, h, &ball(r0 + 20.0, 0.0))); // on the equator, seen from above the pole
        assert!(horizon_hidden(v.cam, h, &ball(r0 * 0.9, -r0 * 0.6))); // past the horizon, well inside the cone
    }
}
