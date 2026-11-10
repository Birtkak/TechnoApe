//! LOD octree selection, job scheduling on worker threads, GPU sub-allocation of chunk meshes.
use crate::mesh::{mesh, Mesh};
use crate::liquid::{Col, TILE};
use crate::world::{fold, generate, sea_cell, sea_quads, Block, Bounds, FxMap, Gen, Key, World, CS};
use glam::{DQuat, DVec3, IVec3, Vec4};
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
    Ready { off: u32, len: u32, water: u32 },
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

/// One drawable range of the quad buffer: `len` quads, the last `water` of them the sea's surface.
pub struct DrawChunk {
    pub key: Key,
    pub off: u32,
    pub len: u32,
    pub water: u32,
    /// Outside the view or behind a horizon: not drawn, only traced.
    pub hidden: bool,
}

#[derive(Default, Clone, Copy)]
pub struct FrameStats {
    pub missing: u32,
    pub holes: u32,
    pub applied: u32,
    pub applied_nonempty: u32,
    pub requests: u32,
}

/// Added to a request's distance when something is already drawn there: every hole goes first.
const REFINE: f64 = 1e9;

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
    /// Per body (ADR 0022): the camera in the body's frame, and the body's frame turned into the camera's.
    /// Distances are measured in the body's frame; the frustum and the horizons in the camera's.
    at: Vec<(DVec3, DQuat)>,
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
        let cf = world.frame_at(cam);
        let at: Vec<(DVec3, DQuat)> = (0..world.bodies.len())
            .map(|b| {
                let (r, off) = world.place(cf, b);
                (r * (cam - world.nominal(cf)) + off, world.place(b, cf).0)
            })
            .collect();
        let hz = world.bodies.iter().enumerate().filter_map(|(i, b)| {
            let cv = at[i].1 * (b.centre - at[i].0);
            let dc = cv.length();
            if dc <= b.r0 {
                return None;
            }
            let t2 = dc * dc - b.r0 * b.r0;
            Some(Horizon { u: cv / dc, plane: t2 / dc, sin: b.r0 / dc, cos: t2.sqrt() / dc })
        }).collect();
        View { cam, planes, hz, at }
    }
    /// A point of body `body` from the camera, in the camera's frame.
    fn rel(&self, body: u8, p: DVec3) -> DVec3 {
        let (c, r) = self.at[body as usize];
        r * (p - c)
    }
    /// The camera in body `body`'s frame.
    pub fn cam_in(&self, body: u8) -> DVec3 {
        self.at[body as usize].0
    }
    fn in_frustum(&self, body: u8, b: &Bounds) -> bool {
        let c = self.rel(body, b.c).as_vec3();
        let r = -b.rad as f32;
        self.planes.iter().all(|p| p.truncate().dot(c) + p.w >= r)
    }
}

/// Is the whole sphere hidden behind a body's core (never dug, so a safe occluder)?
/// Hidden = past the touching circle and inside the cone, by its own radius.
fn horizon_hidden(v: DVec3, h: &Horizon, b: &Bounds) -> bool {
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

    /// Chunk `k` splits for this view: the camera is nearer than `kk` of its edges.
    fn splits(&self, world: &World, view: &View, k: &Key, kk: f64) -> bool {
        let bd = self.slots.get(k).map_or_else(|| world.bodies[k.body as usize].bounds(*k), |s| s.bounds);
        k.level > 0 && ((bd.c - view.cam_in(k.body)).length() - bd.rad).max(0.0) < kk * k.edge()
    }

    fn hidden(&self, view: &View, body: u8, b: &Bounds) -> bool {
        let v = view.rel(body, b.c);
        !view.in_frustum(body, b) || (self.opts.horizon && view.hz.iter().any(|h| horizon_hidden(v, h, b)))
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
            let d = ((bd.c - view.cam_in(k.body)).length() - bd.rad).max(0.0);
            if k.level > 0 && d < kk * k.edge() {
                if !draw && d >= self.opts.k * k.edge() && needs_job {
                    // split only because of the look-ahead margin: the real camera may still want this chunk itself
                    reqs.push((d * prio_mul + REFINE, k));
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
                // a child that splits too is no child to wait for: only leaves get meshes, so it never has one,
                // and its own children decide what covers what (a chunk meshed as a leaf from far away would
                // otherwise cover forever once the camera is back)
                let all = kids.iter().all(|c| self.resolved(c) || self.splits(world, view, c, kk));
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
            let hidden = self.hidden(view, k.body, &bd);
            // stand-in from finer chunks left over from before
            let stand_in = pending
                && !covered
                && k.level > 0
                && (0..8).all(|i| { let c = k.child(i); !Self::in_band(world, c) || self.resolved(&c) });
            if needs_job {
                // metres, not edges: in edges every level's leaves rank alike, so far and near work interleaved
                // at random; in metres the work radiates out from the camera, holes (nothing drawn) first. The
                // look-ahead never sees cover, so its pending chunks rank as holes: ready before the camera is there.
                let mut p = d * prio_mul;
                if hidden {
                    p *= 3.0;
                }
                if !pending || covered || hidden || stand_in {
                    p += REFINE;
                }
                reqs.push((p, k));
                st.requests += 1;
            }
            if pending {
                if draw && !hidden {
                    st.missing += 1;
                    if !covered {
                        if stand_in {
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

    /// Append a ready chunk to the draw list, marked hidden if culled; true if it is ready.
    fn emit(&mut self, view: &View, k: Key, hidden: Option<bool>, out: &mut Vec<DrawChunk>) -> bool {
        let Some(s) = self.slots.get_mut(&k) else { return false };
        s.last_seen = self.frame;
        let bd = s.bounds;
        let (off, len, water) = match s.state {
            State::Pending => return false,
            State::Empty => return true,
            State::Ready { off, len, water } => (off, len, water),
        };
        // hidden chunks are listed too: they are not drawn, but they cast shadows (ADR 0022)
        let hidden = hidden.unwrap_or_else(|| self.hidden(view, k.body, &bd));
        out.push(DrawChunk { key: k, off, len, water, hidden });
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
            // tiles of stored water are the water mesh's; a coarse column lies within one tile up to level 4, and past
            // that a far chunk's sea covers them (shortcut: a flood far away shows as sea level)
            let wet = match sea_cell(&bodies[k.body as usize], k) {
                Some(_) if 1 << k.level <= TILE => world.water.wet_tiles(k.body, k.face, k.base().x, k.base().y, k.span()),
                _ => Vec::new(),
            };
            rayon::spawn(move || {
                let t = Instant::now();
                let body = &bodies[k.body as usize];
                let gen = generate(body, k, &edits);
                let mut mesh = match &gen {
                    Gen::Cells(c) => Some(mesh(c, greedy)).filter(|m| !m.quads.is_empty()),
                    _ => None,
                };
                let sea = sea_quads(body, k, &wet, &gen);
                if !sea.is_empty() {
                    let m = mesh.get_or_insert_with(|| crate::mesh::Mesh { quads: Vec::new(), counts: [0; 6], water: 0 });
                    m.water = sea.len() as u32;
                    m.quads.extend(sea);
                }
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
                    State::Ready { off, len: n, water: m.water }
                }
            };
            let s = self.slots.get_mut(&r.key).unwrap();
            if let State::Ready { off, len, .. } = s.state {
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
    /// Tile `t` became wet or dry: the chunks that draw its sea (levels up to 4) are meshed again.
    pub fn touch_tile(&mut self, world: &World, t: Col) {
        let body = &world.bodies[t.body() as usize];
        let Some(sea) = body.sea else { return };
        for l in 0..=body.root_level.min(4) {
            let (span, cz) = (CS << l, (sea / (1i32 << l) as f64).floor() as i32);
            let k = Key { body: t.body(), face: t.face(), level: l, pos: IVec3::new(t.i() * TILE / span, t.j() * TILE / span, cz.div_euclid(CS)) };
            if let Some(sl) = self.slots.get_mut(&k) {
                sl.want_v += 1;
            }
        }
    }

    pub fn shows(&self, world: &World, b: Block) -> bool {
        sampling(world, b, Some(0)).iter().all(|k| self.slots.get(k).is_none_or(|s| s.shown_v >= s.want_v))
    }

    pub fn evict(&mut self, age: u64) {
        let f = self.frame;
        let alloc = &mut self.alloc;
        self.slots.retain(|_, s| {
            let keep = s.inflight || f.saturating_sub(s.last_seen) < age;
            if !keep {
                if let State::Ready { off, len, .. } = s.state {
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
    /// Selects, meshes and applies until nothing is missing or asked for; the chunks it draws.
    fn settle(s: &mut Streamer, w: &World, cam: DVec3) -> Vec<DrawChunk> {
        let view = View::new(cam, [Vec4::new(0.0, 0.0, 0.0, 1.0); 6], w);
        for _ in 0..5000 {
            let (mut out, mut reqs, mut st) = (Vec::new(), Vec::new(), FrameStats::default());
            s.apply(usize::MAX, &mut st, |_, _| {});
            s.frame += 1;
            s.select(w, &view, true, 1.0, &mut out, &mut reqs, &mut st);
            s.dispatch(w, &mut reqs);
            if s.frame % 30 == 0 {
                s.evict(120); // as the client does
            }
            if st.missing == 0 && st.requests == 0 && s.idle() && s.frame % 200 == 0 {
                return out;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        panic!("never settled");
    }

    #[test]
    fn coming_back_from_far_away_draws_the_fine_chunks_again() {
        // far away the near chunks' coarse ancestors are the leaves and get meshes; back on the ground those
        // ancestors split again, and their children, which never get meshes, must not keep them covering
        let w = World::new(1536, 64, 1337);
        let opts = Opts { k: 4.0, greedy: false, horizon: true, prefetch_s: 0.0, max_inflight: 64 };
        let mut s = Streamer::new(&w, 1 << 22, opts);
        let ground = w.bodies[0].spawn_point() + w.bodies[0].spawn * 1.7;
        let fine = |out: &[DrawChunk]| out.iter().filter(|d| !d.hidden).map(|d| d.key.level).min().unwrap();
        assert_eq!(fine(&settle(&mut s, &w, ground)), 0);
        assert!(fine(&settle(&mut s, &w, w.bodies[0].spawn * w.bodies[0].radius * 4.0)) > 0, "far away: coarse");
        assert_eq!(fine(&settle(&mut s, &w, ground)), 0, "the finest level drawn on the ground once back");
    }

    #[test]
    fn after_a_move_requests_radiate_out_from_the_camera() {
        let w = World::new(1536, 64, 1337);
        let opts = Opts { k: 4.0, greedy: false, horizon: false, prefetch_s: 0.0, max_inflight: 64 };
        let mut s = Streamer::new(&w, 1 << 22, opts);
        let b = &w.bodies[0];
        let up = b.spawn;
        let side = up.any_orthonormal_vector();
        settle(&mut s, &w, b.spawn_point() + up * 1.7);
        // a fast flight: 300 m further along, a little higher
        let cam = b.spawn_point() + side * 300.0 + up * 20.0;
        let view = View::new(cam, [Vec4::new(0.0, 0.0, 0.0, 1.0); 6], &w);
        let (mut out, mut reqs, mut st) = (Vec::new(), Vec::new(), FrameStats::default());
        s.frame += 1;
        s.select(&w, &view, true, 1.0, &mut out, &mut reqs, &mut st);
        reqs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap()); // as dispatch hands them out
        let dist = |k: &Key| {
            let bd = w.bodies[k.body as usize].bounds(*k);
            ((bd.c - view.cam_in(k.body)).length() - bd.rad).max(0.0)
        };
        let refine: Vec<(u8, f64)> = reqs.iter().filter(|r| r.0 >= REFINE).map(|r| (r.1.level, dist(&r.1))).collect();
        let levels: std::collections::BTreeSet<u8> = refine.iter().map(|r| r.0).collect();
        assert!(refine.len() > 20 && levels.len() > 2, "{} refinements over levels {levels:?}", refine.len());
        // nearer first, whatever the level (in edges, a coarse chunk far away ranked with a fine one at hand)
        for p in refine.windows(2) {
            assert!(p[0].1 <= p[1].1 + 1e-6, "{:?} before {:?}", p[0], p[1]);
        }
    }

    #[test]
    fn horizon_hides_only_what_is_behind_the_core() {
        let w = World::new(1536, 512, 1337);
        let r0 = w.bodies[0].r0;
        let v = View::new(DVec3::new(0.0, r0 + 100.0, 0.0), [Vec4::ZERO; 6], &w);
        let h = &v.hz[0];
        let ball = |x: f64, y: f64| Bounds { c: DVec3::new(x, y, 0.0), rad: 14.0 };
        let hid = |b: Bounds| horizon_hidden(b.c - v.cam, h, &b);
        assert!(hid(ball(0.0, -r0 - 20.0))); // far side of the planet
        assert!(!hid(ball(0.0, r0 + 20.0))); // between camera and planet
        assert!(!hid(ball((r0 + 20.0) * h.cos, (r0 + 20.0) * h.sin))); // above the horizon line
        assert!(hid(ball(r0 + 20.0, 0.0))); // on the equator, seen from above the pole
        assert!(hid(ball(r0 * 0.9, -r0 * 0.6))); // past the horizon, well inside the cone
    }
}
