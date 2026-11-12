//! LOD octree selection, job scheduling on worker threads, GPU sub-allocation of chunk meshes.
use crate::mesh::{mesh, Mesh};
use crate::liquid::{Col, TILE};
use crate::world::{fold, generate, river_quads, sea_quads, Block, Bounds, FxMap, Gen, Key, World, CS};
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
    /// The walk that last drew it as a wanted leaf: a touch on a frame without a walk requests it (ADR 0025, F).
    leaf_at: u64,
    /// The walk that last drew it, and where in its list.
    drawn_at: (u64, u32),
}

/// The last LOD walk (ADR 0025, F): which chunks it drew, their bounds, its stats, and the camera in each
/// body's frame it walked for. Frames without a walk cull this list instead.
struct Walk {
    draws: Vec<(DrawChunk, Bounds)>,
    /// Where `draws` starts in the walk's output.
    first: u32,
    missing: u32,
    holes: u32,
    cams: Vec<DVec3>,
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
    /// From the request to a worker taking it up.
    wait_us: f32,
}

/// One drawable range of the quad buffer: `len` quads, the last `water` of them the sea's surface.
#[derive(Clone, Copy)]
pub struct DrawChunk {
    pub key: Key,
    pub off: u32,
    pub len: u32,
    pub water: u32,
    /// Outside the view or behind a horizon: not drawn, only traced.
    pub hidden: bool,
    /// From the camera to its bounding sphere, metres: the draws go near to far (ADR 0025, D).
    pub dist: f32,
}

#[derive(Default, Clone, Copy)]
pub struct FrameStats {
    pub missing: u32,
    pub holes: u32,
    pub applied: u32,
    pub applied_nonempty: u32,
    pub requests: u32,
}

/// Edits of blocks that name a model become air for the mesher (ADR 0047).
pub fn hide_models(edits: &mut [(u32, u8)], reg: &crate::blocks::Registry) {
    for e in edits.iter_mut() {
        if reg.get(e.1).is_some_and(|d| d.model.is_some()) {
            e.1 = crate::blocks::AIR;
        }
    }
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
    /// Every applied job's wait for a worker and its own time, in us, until the owner takes them (ADR 0025, S).
    pub job_times: Vec<(f32, f32)>,
    pub alloc_fail: u64,
    pub ahead_reqs: Vec<(f64, Key)>,
    walk: Option<Walk>,
    /// The frame of the last walk; slots age from it.
    walk_frame: u64,
    /// A job was applied or a slot evicted since the last walk: the next frame walks.
    dirty: bool,
    /// Slots touched since the last frame.
    touched: Vec<Key>,
    /// Walks done, for the benchmark line.
    pub walks: u64,
    /// Quad-buffer offsets released since the client last took them: their BLAS go too (ADR 0025, G).
    pub released: Vec<u32>,
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
            job_times: Vec::new(),
            alloc_fail: 0,
            ahead_reqs: Vec::new(),
            walk: None,
            walk_frame: 0,
            dirty: true,
            touched: Vec::new(),
            walks: 0,
            released: Vec::new(),
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

    /// Chunk `k` draws without its parent's cover: it has a mesh (or is empty), or it splits and all its
    /// children do. Stops at the first chunk with a mesh, so it goes deep only where a fast camera split chunks
    /// that were never meshed (ADR 0038 B).
    fn complete(&self, world: &World, view: &View, k: &Key, kk: f64) -> bool {
        self.resolved(k)
            || (self.splits(world, view, k, kk)
                && (0..8).all(|i| {
                    let c = k.child(i);
                    !Self::in_band(world, c) || self.complete(world, view, &c, kk)
                }))
    }

    /// Every in-band child of `k` has a mesh, or has children that all do, down to the finest level.
    fn kids_below(&self, world: &World, k: &Key) -> bool {
        (0..8).all(|i| {
            let c = k.child(i);
            !Self::in_band(world, c) || self.resolved(&c) || (c.level > 0 && self.kids_below(world, &c))
        })
    }

    /// Draws `k`'s children, or where a child has no mesh, its own children (a stand-in, `kids_below`).
    fn emit_below(&mut self, world: &World, view: &View, k: Key, out: &mut Vec<DrawChunk>) {
        for i in 0..8 {
            let c = k.child(i);
            if Self::in_band(world, c) && !self.emit(view, c, None, out) && c.level > 0 {
                self.emit_below(world, view, c, out);
            }
        }
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
        // the walk depends on the camera's place and the slots only: a frame where neither changed culls the
        // last walk's chunks for this look instead (ADR 0025, F)
        let cams: Vec<DVec3> = (0..world.bodies.len()).map(|b| view.cam_in(b as u8)).collect();
        if draw {
            let still = self.walk.as_ref().is_some_and(|w| {
                w.cams.iter().zip(&cams).zip(world.bodies.iter()).all(|((a, b), body)| {
                    // 4 m near a body, more the higher over it: the moon's orbit alone does not force a walk
                    let over = (b.length() - body.radius).max(0.0);
                    (*a - *b).length() < 4.0 + 0.01 * over
                })
            });
            if still && !self.dirty {
                self.cull(view, prio_mul, out, reqs, st);
                return;
            }
            self.dirty = false;
            self.touched.clear();
            self.walk_frame = self.frame;
            self.walks += 1;
        }
        let (first, missing0, holes0) = (out.len(), st.missing, st.holes);
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
                leaf_at: 0,
                drawn_at: (0, 0),
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
                // a child that splits too never gets a mesh of its own (only leaves do): it counts as done once its
                // own children are, so this chunk covers until then and never forever (a chunk meshed as a leaf
                // from far away would otherwise cover forever once the camera is back), and a camera that nears
                // the children before they are meshed leaves no hole (ADR 0038 B)
                let all = covered || kids.iter().all(|c| self.complete(world, view, c, kk));
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
            // stand-in from finer chunks left over from before, as deep as a fast camera left them (ADR 0038 B)
            let stand_in = pending && !covered && k.level > 0 && self.kids_below(world, &k);
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
                            self.emit_below(world, view, k, out);
                        } else {
                            st.holes += 1;
                        }
                    }
                }
            } else if draw && !covered {
                self.emit(view, k, Some(hidden), out);
            }
        }
        if draw {
            let draws = out[first..].iter().map(|d| (*d, self.slots[&d.key].bounds)).collect();
            self.walk = Some(Walk { draws, first: first as u32, missing: st.missing - missing0, holes: st.holes - holes0, cams });
        }
    }

    /// A frame without a walk: the last walk's chunks culled for this view, and the touched ones it drew as
    /// leaves requested.
    fn cull(&mut self, view: &View, prio_mul: f64, out: &mut Vec<DrawChunk>, reqs: &mut Vec<(f64, Key)>, st: &mut FrameStats) {
        let w = self.walk.as_ref().unwrap();
        st.missing += w.missing;
        st.holes += w.holes;
        for (d, bd) in &w.draws {
            let dist = ((bd.c - view.cam_in(d.key.body)).length() - bd.rad).max(0.0);
            out.push(DrawChunk { hidden: self.hidden(view, d.key.body, bd), dist: dist as f32, ..*d });
        }
        for k in std::mem::take(&mut self.touched) {
            let Some(s) = self.slots.get(&k) else { continue };
            if s.leaf_at == self.walk_frame && s.needs_job() {
                let d = ((s.bounds.c - view.cam_in(k.body)).length() - s.bounds.rad).max(0.0);
                let hidden = self.hidden(view, k.body, &s.bounds);
                reqs.push((d * prio_mul * if hidden { 3.0 } else { 1.0 } + REFINE, k));
                st.requests += 1;
            }
        }
    }

    /// Append a ready chunk to the draw list, marked hidden if culled; true if it is ready.
    fn emit(&mut self, view: &View, k: Key, hidden: Option<bool>, out: &mut Vec<DrawChunk>) -> bool {
        let Some(s) = self.slots.get_mut(&k) else { return false };
        s.last_seen = self.frame;
        if hidden.is_some() {
            s.leaf_at = self.frame;
        }
        s.drawn_at = (self.frame, out.len() as u32);
        let bd = s.bounds;
        let (off, len, water) = match s.state {
            State::Pending => return false,
            State::Empty => return true,
            State::Ready { off, len, water } => (off, len, water),
        };
        // hidden chunks are listed too: they are not drawn, but they cast shadows (ADR 0022)
        let hidden = hidden.unwrap_or_else(|| self.hidden(view, k.body, &bd));
        let dist = ((bd.c - view.cam_in(k.body)).length() - bd.rad).max(0.0) as f32;
        out.push(DrawChunk { key: k, off, len, water, hidden, dist });
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
            // a cell whose block is a model is drawn by the client's models, not meshed (ADR 0047)
            let mut edits = world.edits_for(k);
            if !world.models.is_empty() {
                hide_models(&mut edits, &crate::blocks::BLOCKS);
            }
            let tx = self.tx.clone();
            let bodies = world.bodies.clone();
            let greedy = self.opts.greedy;
            // tiles of stored water are the water mesh's; a coarse column lies within one tile up to level 4, and past
            // that a far chunk's sea covers them (shortcut: a flood far away shows as sea level)
            let wet = if 1 << k.level <= TILE { world.water.wet_tiles(k.body, k.face, k.base().x, k.base().y, k.span()) } else { Vec::new() };
            let queued = Instant::now();
            rayon::spawn(move || {
                let t = Instant::now();
                let wait_us = (t - queued).as_secs_f32() * 1e6;
                let body = &bodies[k.body as usize];
                let gen = generate(body, k, &edits);
                let mut mesh = match &gen {
                    Gen::Cells(c) => Some(mesh(c, if greedy { body.lattice(k.level) as usize } else { 1 })).filter(|m| !m.quads.is_empty()),
                    _ => None,
                };
                let mut sea = sea_quads(body, k, &wet, &gen);
                sea.extend(river_quads(body, k, &wet, &gen));
                if !sea.is_empty() {
                    let m = mesh.get_or_insert_with(|| crate::mesh::Mesh { quads: Vec::new(), counts: [0; 6], water: 0 });
                    m.water = sea.len() as u32;
                    m.quads.extend(sea);
                }
                let _ = tx.send(JobResult { key: k, version, mesh, us: t.elapsed().as_secs_f64() * 1e6, wait_us });
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
            self.job_times.push((r.wait_us, r.us as f32));
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
                self.released.push(off);
            }
            // a remesh that stays drawn changes only its range: the last walk's list takes it in place; any other
            // change of state can change what the walk picks
            match (s.state, new, &mut self.walk) {
                (State::Ready { .. }, State::Ready { off, len, water }, Some(w)) if s.drawn_at.0 == self.walk_frame => {
                    let d = &mut w.draws[(s.drawn_at.1 - w.first) as usize].0;
                    (d.off, d.len, d.water) = (off, len, water);
                }
                _ => self.dirty = true,
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
                self.touched.push(k);
            }
        }
    }

    /// True when every full-detail chunk that samples block `b` draws its latest version: an edit there is
    /// on screen (the feel report's click-to-change, fundamentals section 1).
    /// Tile `t` became wet or dry: the chunks that draw its generated water (levels up to 4) are meshed again.
    pub fn touch_tile(&mut self, world: &World, t: Col) {
        for k in crate::world::water_keys(&world.bodies[t.body() as usize], t) {
            if let Some(sl) = self.slots.get_mut(&k) {
                sl.want_v += 1;
                self.touched.push(k);
            }
        }
    }

    pub fn shows(&self, world: &World, b: Block) -> bool {
        sampling(world, b, Some(0)).iter().all(|k| self.slots.get(k).is_none_or(|s| s.shown_v >= s.want_v))
    }

    pub fn evict(&mut self, age: u64) {
        // a slot is seen by walks: frames that only cull see none (ADR 0025, F)
        let f = self.walk_frame;
        let (alloc, released) = (&mut self.alloc, &mut self.released);
        let n = self.slots.len();
        self.slots.retain(|_, s| {
            let keep = s.inflight || f.saturating_sub(s.last_seen) < age;
            if !keep {
                if let State::Ready { off, len, .. } = s.state {
                    alloc.release(off, len);
                    released.push(off);
                }
            }
            keep
        });
        self.dirty |= self.slots.len() != n;
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
    fn a_model_block_is_not_meshed() {
        let reg = crate::blocks::Registry::parse("[[block]]\nid = 0\nname = \"air\"\nsolid = false\n[[block]]\nid = 1\nname = \"rock\"\nhardness = 1\n[[block]]\nid = 2\nname = \"furnace\"\nhardness = 3\nmodel = \"furnace\"\n").unwrap();
        let mut e = vec![(5, 1), (6, 2)];
        hide_models(&mut e, &reg);
        assert_eq!(e, vec![(5, 1), (6, 0)]);
    }
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
    fn a_fast_flight_never_leaves_a_hole_where_a_coarse_chunk_was_drawn() {
        // F9 report 2026-11-11_13-02-00: at 1000 m/s, 200 m up, the ground ahead vanished for a moment. A chunk
        // drawn from far away splits as the camera nears; if the camera nears its children before they are
        // meshed, they split too, never get meshes of their own, and the coarse chunk stopped covering them
        let w = World::new(1536, 64, 1337);
        let opts = Opts { k: 4.0, greedy: false, horizon: true, prefetch_s: 0.0, max_inflight: 8 };
        let mut s = Streamer::new(&w, 1 << 23, opts);
        let b = &w.bodies[0];
        let side = b.spawn.any_orthonormal_vector();
        let at = |m: f64| {
            let d = (b.spawn + side * (m / b.radius)).normalize();
            d * (b.top(d) + 200.0)
        };
        settle(&mut s, &w, at(0.0));
        let mut worst = 0;
        // 6 s of flight at 1000 m/s, a frame each 1/180 s, 8 jobs done a frame (meshing slower than the flight)
        for f in 1..=1080 {
            let view = View::new(at(f as f64 * 1000.0 / 180.0), [Vec4::new(0.0, 0.0, 0.0, 1.0); 6], &w);
            let (mut out, mut reqs, mut st) = (Vec::new(), Vec::new(), FrameStats::default());
            while !s.idle() {
                std::thread::sleep(std::time::Duration::from_micros(200));
                s.apply(usize::MAX, &mut st, |_, _| {});
            }
            s.frame += 1;
            s.select(&w, &view, true, 1.0, &mut out, &mut reqs, &mut st);
            s.dispatch(&w, &mut reqs);
            if s.frame % 30 == 0 {
                s.evict(120);
            }
            worst = worst.max(st.holes);
        }
        assert_eq!(worst, 0, "holes in the ground during the flight");
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
