//! LOD octree selection, job scheduling on worker threads, GPU sub-allocation of chunk meshes.
use crate::mesh::{mesh_cracked, Mesh};
use crate::liquid::{Col, TILE};
use crate::world::{fold, generate_raw, pidx, skirt, river_quads, sea_quads, Block, Bounds, FxMap, Gen, Key, World, CS, P};
use glam::{DQuat, DVec3, IVec3, Vec4};
use crate::clutter::Still;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Instant;


#[derive(Clone, Copy)]
pub struct Opts {
    /// A chunk splits when the camera is nearer than k chunk edges.
    pub k: f64,
    pub greedy: bool,
    /// Fancy leaves (ADR 0069): level 0 keeps the faces behind and between cutout blocks; the client remeshes
    /// level 0 when it changes.
    pub cutout: bool,
    pub horizon: bool,
    pub prefetch_s: f64,
    pub max_inflight: usize,
}

/// First-fit free list over the quad buffer, in quad units.
pub struct Alloc {
    free: Vec<(u32, u32)>, // sorted (offset, len)
    pub used: u64,
    pub cap: u32,
    /// An allocation failed and nothing was released since: a new mesh would fail too.
    pub full: bool,
}
impl Alloc {
    pub fn new(cap: u32) -> Self {
        Alloc { free: vec![(0, cap)], used: 0, cap, full: false }
    }
    pub fn alloc(&mut self, n: u32) -> Option<u32> {
        let Some(i) = self.free.iter().position(|f| f.1 >= n) else {
            self.full = true;
            return None;
        };
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
        self.full = false;
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
    Ready { off: u32, len: u32, water: u32, cut: u32, ray_cut: u32, fluff: u32, glass: u32, still: u32 },
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
    /// The climate at its footprint's corners (`clutter::chunk_climate`, ADR 0091), from its last mesh job.
    clim: u32,
    key: Key,
    /// Its children's places in the slab by child number: `UNKNOWN` until looked up, `NONE` when it has no slot.
    kids: [u32; 8],
    /// False once evicted: its place in the slab waits for another chunk.
    live: bool,
    /// `Streamer::complete_at`'s answer for the walk of that number (`Streamer::walk_no`).
    complete: (u64, bool),
}

/// No slot (`Slot::kids`, a walk's stack: not made yet).
const NONE: u32 = u32::MAX;
/// A child not looked up yet (`Slot::kids`).
const UNKNOWN: u32 = u32::MAX - 1;

/// The slots by key, and in a slab where each knows its children's places: the walk follows those instead of
/// hashing a key per look (a walk at detail 8 was some 55,000 hashed look-ups; `walk_cost`).
#[derive(Default)]
struct Slots {
    index: FxMap<Key, u32>,
    slab: Vec<Slot>,
    free: Vec<u32>,
}

/// `k`'s parent and which of its children `k` is (`Key::child`).
fn parent(k: &Key) -> (Key, usize) {
    (Key { level: k.level + 1, pos: k.pos >> 1, ..*k }, ((k.pos.x & 1) | (k.pos.y & 1) << 1 | (k.pos.z & 1) << 2) as usize)
}

impl Slots {
    fn id(&self, k: &Key) -> Option<u32> {
        self.index.get(k).copied()
    }
    fn get(&self, k: &Key) -> Option<&Slot> {
        self.id(k).map(|i| &self.slab[i as usize])
    }
    fn get_mut(&mut self, k: &Key) -> Option<&mut Slot> {
        self.id(k).map(|i| &mut self.slab[i as usize])
    }
    fn at(&self, i: u32) -> &Slot {
        &self.slab[i as usize]
    }
    fn at_mut(&mut self, i: u32) -> &mut Slot {
        &mut self.slab[i as usize]
    }
    fn len(&self) -> usize {
        self.index.len()
    }
    #[cfg(test)]
    fn iter(&self) -> impl Iterator<Item = (&Key, &Slot)> {
        self.slab.iter().filter(|s| s.live).map(|s| (&s.key, s))
    }
    fn iter_mut(&mut self) -> impl Iterator<Item = (Key, &mut Slot)> {
        self.slab.iter_mut().filter(|s| s.live).map(|s| (s.key, s))
    }
    /// `k`'s place, its slot made pending (with `bounds`) if it had none.
    fn make(&mut self, k: Key, frame: u64, bounds: impl FnOnce() -> Bounds) -> u32 {
        if let Some(i) = self.id(&k) {
            return i;
        }
        let slot = Slot { state: State::Pending, last_seen: frame, want_v: 0, shown_v: 0, inflight: false, bounds: bounds(), leaf_at: 0, clim: 0, drawn_at: (0, 0), key: k, kids: [UNKNOWN; 8], live: true, complete: (0, false) };
        let i = match self.free.pop() {
            Some(i) => {
                self.slab[i as usize] = slot;
                i
            }
            None => {
                self.slab.push(slot);
                self.slab.len() as u32 - 1
            }
        };
        self.index.insert(k, i);
        let (p, ci) = parent(&k);
        if let Some(p) = self.id(&p) {
            self.slab[p as usize].kids[ci] = i;
        }
        i
    }
    /// The place of child `ci` (key `c`) of the slot at `p`, or `NONE`.
    fn kid(&self, p: u32, ci: usize, c: &Key) -> u32 {
        match self.slab[p as usize].kids[ci] {
            UNKNOWN => self.id(c).unwrap_or(NONE),
            i => i,
        }
    }
    /// Drops the slots `keep` turns down; their parents forget them.
    fn retain(&mut self, mut keep: impl FnMut(&mut Slot) -> bool) {
        for i in 0..self.slab.len() {
            if self.slab[i].live && !keep(&mut self.slab[i]) {
                let k = self.slab[i].key;
                self.slab[i].live = false;
                self.index.remove(&k);
                self.free.push(i as u32);
                let (p, ci) = parent(&k);
                if let Some(p) = self.id(&p) {
                    self.slab[p as usize].kids[ci] = NONE;
                }
            }
        }
    }
}

impl std::ops::Index<&Key> for Slots {
    type Output = Slot;
    fn index(&self, k: &Key) -> &Slot {
        &self.slab[self.index[k] as usize]
    }
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
    /// The look it was walked for (`View::planes`).
    planes: [Vec4; 6],
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
    clim: u32,
}

/// One drawable range of the quad buffer: `len` quads, the last `water` of them the sea's surface and the `cut`
/// before those the still models' (clutter, ADR 0082), the `glass` before those the glass blocks' (ADR 0081) and
/// the `cut` before those the cutout blocks' (ADR 0069).
#[derive(Clone, Copy)]
pub struct DrawChunk {
    pub key: Key,
    pub off: u32,
    pub len: u32,
    pub water: u32,
    pub cut: u32,
    /// How many of `cut` are not opaque to a ray: the mesh's own (`Mesh::ray_cut`), not the setting's.
    pub ray_cut: u32,
    /// How many of `cut`, first among them, open on air: the fluff's (`Mesh::fluff`, ADR 0103).
    pub fluff: u32,
    pub glass: u32,
    pub still: u32,
    /// Outside the view or behind a horizon: not drawn, only traced.
    pub hidden: bool,
    /// From the camera to its bounding sphere, metres: the draws go near to far (ADR 0025, D).
    pub dist: f32,
    /// From the camera to its centre, metres: glass blends far to near by it (ADR 0081; `dist` is 0 for every chunk
    /// round the camera).
    pub centre: f32,
    /// The climate at its footprint's corners (`clutter::chunk_climate`): the grass's and the plants' tint (ADR 0091).
    pub clim: u32,
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
pub fn hide_models(edits: &mut [(u32, u16)], reg: &crate::blocks::Registry) {
    for e in edits.iter_mut() {
        if reg.get(e.1).is_some_and(|d| d.model.is_some()) {
            e.1 = crate::blocks::AIR;
        }
    }
}

/// Added to a request's distance when something is already drawn there: every hole goes first.
const REFINE: f64 = 1e9;

/// The chunks, at the levels whose bits are set in `levels`, whose cells or padding sample block `b`.
fn sampling(world: &World, b: Block, levels: u32) -> Vec<Key> {
    let body = &world.bodies[b.body as usize];
    let mut keys = Vec::new();
    for l in 0..=body.root_level {
        if levels >> l & 1 == 0 {
            continue;
        }
        // a placed block anywhere in a coarse cell can change it, not only its middle (ADR 0109)
        let cell = b.pos.div_euclid(IVec3::splat(1i32 << l));
        let own = Key { body: b.body, face: b.face, level: l, pos: cell.div_euclid(IVec3::splat(CS)) };
        let start = keys.len(); // keys of other levels never match
        keys.push(own);
        // a cell off its chunk's sides is padding of no other chunk
        let local = cell.rem_euclid(IVec3::splat(CS));
        if local.cmpgt(IVec3::ZERO).all() && local.cmplt(IVec3::splat(CS - 1)).all() {
            continue;
        }
        // all 26 neighbours: the mesher's corner shading reads the diagonal padding too
        for i in 0..27 {
            let c = cell + IVec3::new(i % 3 - 1, i / 3 % 3 - 1, i / 9 - 1);
            let (f, ci, cj) = fold(b.face as usize, c.x, c.y, body.n >> l);
            let k = Key { face: f as u8, pos: IVec3::new(ci, cj, c.z).div_euclid(IVec3::splat(CS)), ..own };
            if !keys[start..].contains(&k) {
                keys.push(k);
            }
        }
    }
    keys
}

pub struct Streamer {
    slots: Slots,
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
    /// The block rows these meshes were asked from (`live::rev`, ADR 0134).
    defs_rev: u64,
    /// The last walk left a cover out for a child out of view: the next frame that looks another way walks.
    look_dirty: bool,
    /// Slots touched since the last frame.
    touched: Vec<Key>,
    /// Walks done, for the benchmark line.
    pub walks: u64,
    /// Quad-buffer offsets released since the client last took them: their BLAS go too (ADR 0025, G).
    pub released: Vec<u32>,
    /// What level 0 lays its clutter with (ADR 0082): the still models, the sky's time it was last set at (its
    /// season picks the rows, its year what has come back), that time's (nearest season, year), and the Clutter
    /// setting's density and the Vegetation setting's (ADR 0091).
    pub clutter: (Arc<Still>, f64, (i64, u32), [f32; 2]),
    /// The pick-ups taken (ADR 0082): the cell each lay in and the year it was taken, as the server sends them.
    pub taken: FxMap<Block, u32>,
    /// Cells level 0 meshes as air while the client draws their block gliding in (ADR 0101): few, a slump's.
    pub gliding: Vec<Block>,
    /// Chunks the walks took off their stack, for the test of the empty leaves' shortcut.
    visits: u64,
    /// Walk every empty leaf one by one, as before the shortcut: the test's reference.
    slow_walk: bool,
    /// Counts the walks of both kinds (drawn and look-ahead): what a slot's remembered `complete` is for.
    walk_no: u64,
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
            slots: Slots::default(),
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
            defs_rev: crate::live::rev(),
            look_dirty: false,
            touched: Vec::new(),
            walks: 0,
            released: Vec::new(),
            clutter: (Arc::new(Still::default()), 0.0, (i64::MIN, 0), [1.0; 2]),
            taken: FxMap::default(),
            gliding: Vec::new(),
            visits: 0,
            slow_walk: false,
            walk_no: 0,
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

    /// `complete` for the slot at `id` (`NONE`: by its key), remembered per slot for this walk: every level asked
    /// it of the whole subtree under each child again (most of a walk at detail 8, `walk_cost`). Within one walk
    /// the answer cannot change: no job is applied, the camera and `kk` are one, and a slot the walk makes is
    /// pending, as a missing one counts.
    fn complete_at(&mut self, world: &World, view: &View, k: &Key, id: u32, kk: f64) -> bool {
        if id == NONE {
            return self.complete(world, view, k, kk);
        }
        let s = self.slots.at(id);
        if s.state != State::Pending {
            return true;
        }
        if s.complete.0 == self.walk_no {
            return s.complete.1;
        }
        let bd = s.bounds;
        let splits = k.level > 0 && ((bd.c - view.cam_in(k.body)).length() - bd.rad).max(0.0) < kk * k.edge();
        let done = splits
            && (0..8).all(|i| {
                let c = k.child(i);
                !Self::in_band(world, c) || {
                    let ci = self.slots.kid(id, i as usize, &c);
                    self.slots.at_mut(id).kids[i as usize] = ci;
                    self.complete_at(world, view, &c, ci, kk)
                }
            });
        self.slots.at_mut(id).complete = (self.walk_no, done);
        done
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
            // 4 m near a body, more the higher over it: the moon's orbit alone does not force a walk
            let within = |slack: f64| {
                self.walk.as_ref().is_some_and(|w| {
                    w.cams.iter().zip(&cams).zip(world.bodies.iter()).all(|((a, b), body)| {
                        (*a - *b).length() < slack + 0.01 * (b.length() - body.radius).max(0.0)
                    })
                })
            };
            let still = within(4.0);
            // (a hidden child's left-out cover: only a new look or place can show its hole; a walk for the same one
            // picks the same, and at detail 8 one that never came, its quad buffer full, walked 6 ms every frame,
            // ADR 0111)
            let turned = self.look_dirty && (!within(0.0) || self.walk.as_ref().is_some_and(|w| w.planes != view.planes));
            if still && !self.dirty && !turned {
                self.cull(view, prio_mul, out, reqs, st);
                return;
            }
            self.dirty = false;
            self.look_dirty = false;
            self.touched.clear();
            self.walk_frame = self.frame;
            self.walks += 1;
        }
        self.walk_no += 1;
        let (first, missing0, holes0) = (out.len(), st.missing, st.holes);
        // (chunk, covered, its slot's place when its parent knew it)
        let mut stack: Vec<(Key, bool, u32)> = self.roots.iter().map(|k| (*k, false, NONE)).collect();
        // The look-ahead splits 10 % earlier: its camera is a straight-line guess, and a chunk the real camera
        // only grazes (a few frames inside the split distance) would otherwise never be requested ahead.
        let kk = if draw { self.opts.k } else { self.opts.k * 1.1 };
        let frame = self.frame;
        while let Some((k, covered, id)) = stack.pop() {
            self.visits += 1;
            let id = if id == NONE { self.slots.make(k, frame, || world.bodies[k.body as usize].bounds(k)) } else { id };
            let slot = self.slots.at_mut(id);
            slot.last_seen = frame;
            let (bd, needs_job, pending) = (slot.bounds, slot.needs_job(), slot.state == State::Pending);
            let d = ((bd.c - view.cam_in(k.body)).length() - bd.rad).max(0.0);
            if k.level > 0 && d < kk * k.edge() {
                if !draw && d >= self.opts.k * k.edge() && needs_job {
                    // split only because of the look-ahead margin: the real camera may still want this chunk itself
                    reqs.push((d * prio_mul + REFINE, k));
                }
                // the children in the band, and each one's slot (the slow walk looks every one up by its key)
                let mut kids = [(k, NONE); 8];
                let mut nk = 0;
                for i in 0..8 {
                    let c = k.child(i);
                    if Self::in_band(world, c) {
                        let ci = if self.slow_walk { NONE } else { self.slots.kid(id, i as usize, &c) };
                        if !self.slow_walk {
                            self.slots.at_mut(id).kids[i as usize] = ci;
                        }
                        kids[nk] = (c, ci);
                        nk += 1;
                    }
                }
                let kids = &kids[..nk];
                // a child that splits too never gets a mesh of its own (only leaves do): it counts as done once its
                // own children are, so this chunk covers until then and never forever (a chunk meshed as a leaf
                // from far away would otherwise cover forever once the camera is back), and a camera that nears
                // the children before they are meshed leaves no hole (ADR 0038 B)
                // a child wholly out of view does not hold it either: it would cover the visible children that are in,
                // over their finer ground, until the hidden ones came last in the queue (bug 2026-11-12_21-00-30)
                // one look-up of each child answers both: does it draw without this chunk's cover, and is it a leaf
                // for this camera that is empty and wants no job (the air over the ground, the rock under it)
                let (mut by_view, mut all, mut empty_leaves) = (false, true, !self.slow_walk);
                if self.slow_walk && !covered {
                    // the walk before the shortcut, the test's reference
                    all = kids.iter().all(|(c, _)| {
                        self.complete(world, view, c, kk) || (draw && {
                            let h = self.hidden(view, c.body, &self.slots.get(c).map_or_else(|| world.bodies[c.body as usize].bounds(*c), |s| s.bounds));
                            by_view |= h;
                            h
                        })
                    });
                }
                let fast = !self.slow_walk;
                for (c, ci) in kids.iter().filter(|_| fast) {
                    let s = (*ci != NONE).then(|| self.slots.at(*ci));
                    empty_leaves &= s.is_some_and(|s| {
                        s.state == State::Empty
                            && !s.needs_job()
                            && !(c.level > 0 && ((s.bounds.c - view.cam_in(c.body)).length() - s.bounds.rad).max(0.0) < kk * c.edge())
                    });
                    // as `all` over the children, stopping at the first that neither draws nor is out of view
                    let bounds = s.map(|s| s.bounds);
                    if all && !covered {
                        all = self.complete_at(world, view, c, *ci, kk)
                            || (draw && {
                                let h = self.hidden(view, c.body, &bounds.unwrap_or_else(|| world.bodies[c.body as usize].bounds(*c)));
                                by_view |= h;
                                h
                            });
                    }
                }
                let all = covered || all;
                // that choice depends on the look, which a frame that only culls does not check: walk again until
                // the hidden child is in, or turning round shows its hole (the review of ADR 0073)
                self.look_dirty |= by_view && all && !covered;
                let mut cov = covered;
                if !all && !covered && draw && self.emit_at(view, id, None, out) {
                    cov = true;
                }
                // such children would draw and ask for nothing: marked as their visits would, not visited
                if empty_leaves {
                    let at = out.len() as u32;
                    for (_, ci) in kids {
                        let s = self.slots.at_mut(*ci);
                        s.last_seen = frame;
                        if draw && !cov {
                            (s.leaf_at, s.drawn_at) = (frame, (frame, at));
                        }
                    }
                    continue;
                }
                for &(c, ci) in kids {
                    stack.push((c, cov, ci));
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
                // a pending chunk is not meshed while the quad buffer is full (`dispatch` passes over it): it is not
                // handed over, thousands a frame at detail 8 in a forest. For the drawn walk that changes nothing
                // (`dispatch` runs this same frame); the look-ahead's requests are kept up to `prefetch_every`
                // frames, so when room comes its pending chunks are asked for that many frames later at most
                if !(pending && self.alloc.full) {
                    reqs.push((p, k));
                }
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
                self.emit_at(view, id, Some(hidden), out);
            }
        }
        if draw {
            let draws = out[first..].iter().map(|d| (*d, self.slots[&d.key].bounds)).collect();
            self.walk = Some(Walk { draws, first: first as u32, missing: st.missing - missing0, holes: st.holes - holes0, cams, planes: view.planes });
        }
    }

    /// A frame without a walk: the last walk's chunks culled for this view, and the touched ones it drew as
    /// leaves requested.
    fn cull(&mut self, view: &View, prio_mul: f64, out: &mut Vec<DrawChunk>, reqs: &mut Vec<(f64, Key)>, st: &mut FrameStats) {
        let w = self.walk.as_ref().unwrap();
        st.missing += w.missing;
        st.holes += w.holes;
        for (d, bd) in &w.draws {
            let centre = (bd.c - view.cam_in(d.key.body)).length();
            out.push(DrawChunk { hidden: self.hidden(view, d.key.body, bd), dist: (centre - bd.rad).max(0.0) as f32, centre: centre as f32, ..*d });
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

    /// The drawn, visible chunks this view would split: drawn coarser than their level (ADR 0073, the check
    /// for F9 report 2026-11-12_21-00-30). Fine while chunks stream in; it must not last once the camera slows.
    pub fn too_coarse(&self, world: &World, view: &View, draws: &[DrawChunk]) -> u32 {
        draws.iter().filter(|d| !d.hidden && self.splits(world, view, &d.key, self.opts.k)).count() as u32
    }

    /// Append a ready chunk to the draw list, marked hidden if culled; true if it is ready.
    fn emit(&mut self, view: &View, k: Key, hidden: Option<bool>, out: &mut Vec<DrawChunk>) -> bool {
        self.slots.id(&k).is_some_and(|id| self.emit_at(view, id, hidden, out))
    }

    /// `emit` for the slot at `id`.
    fn emit_at(&mut self, view: &View, id: u32, hidden: Option<bool>, out: &mut Vec<DrawChunk>) -> bool {
        let s = self.slots.at_mut(id);
        let k = s.key;
        s.last_seen = self.frame;
        if hidden.is_some() {
            s.leaf_at = self.frame;
        }
        s.drawn_at = (self.frame, out.len() as u32);
        let (bd, clim) = (s.bounds, s.clim);
        let (off, len, water, cut, ray_cut, fluff, glass, still) = match s.state {
            State::Pending => return false,
            State::Empty => return true,
            State::Ready { off, len, water, cut, ray_cut, fluff, glass, still } => (off, len, water, cut, ray_cut, fluff, glass, still),
        };
        // hidden chunks are listed too: they are not drawn, but they cast shadows (ADR 0022)
        let hidden = hidden.unwrap_or_else(|| self.hidden(view, k.body, &bd));
        let centre = (bd.c - view.cam_in(k.body)).length();
        out.push(DrawChunk { key: k, off, len, water, cut, ray_cut, fluff, glass, still, hidden, dist: (centre - bd.rad).max(0.0) as f32, centre: centre as f32, clim });
        true
    }

    pub fn dispatch(&mut self, world: &World, reqs: &mut Vec<(f64, Key)>) {
        // no worker free: nothing goes out, whatever the order
        if self.inflight >= self.opts.max_inflight {
            reqs.clear();
            return;
        }
        // nearest first, as a stable sort hands them out (ties in the order asked), but sorted only as far as the
        // workers take: a flight at detail 8 asks for some 20,000 a frame and sends a handful, and sorting them
        // all was most of this (`walk_cost`, ADR 0120). The nearest `FIRST` are picked out and sorted; the rest
        // only when those did not fill the workers
        const FIRST: usize = 256;
        let mut order: Vec<(f64, u32)> = reqs.iter().enumerate().map(|(i, r)| (r.0, i as u32)).collect();
        let by = |a: &(f64, u32), b: &(f64, u32)| a.0.partial_cmp(&b.0).unwrap().then(a.1.cmp(&b.1));
        let first = FIRST.min(order.len());
        if first < order.len() {
            order.select_nth_unstable_by(first, by);
        }
        let mut done = 0;
        for end in [first, order.len()] {
            if self.inflight >= self.opts.max_inflight {
                break;
            }
            order[done..end].sort_unstable_by(by);
            let part = done..end;
            done = end;
            for &(_, i) in &order[part] {
                if self.inflight >= self.opts.max_inflight {
                    break;
                }
                let k = reqs[i as usize].1;
                let Some(s) = self.slots.get_mut(&k) else { continue };
                // a full buffer would refuse a new mesh again: wait until something is released
                if !s.needs_job() || (self.alloc.full && s.state == State::Pending) {
                    continue; // requested by both traversals, or a kept look-ahead request that is done
                }
                s.inflight = true;
                self.inflight += 1;
                let version = s.want_v;
                // the edits are gathered on the worker from shared maps: the frame thread only looks them up (ADR 0109)
                let snap = world.far_snap(k);
                let models = !world.models.is_empty();
                // a gliding cell is air to the mesher, a loose block to its neighbours' slopes (ADR 0101)
                let mut held = Vec::new();
                if k.level == 0 {
                    let c0 = k.pos * CS - 1; // padded cell (0, 0, 0)
                    for b in self.gliding.iter().filter(|b| (b.body, b.face) == (k.body, k.face)) {
                        let q = b.pos - c0;
                        if q.cmpge(IVec3::ZERO).all() && q.cmplt(IVec3::splat(P as i32)).all() {
                            held.push(pidx(q.x as usize, q.y as usize, q.z as usize));
                        }
                    }
                }
                let tx = self.tx.clone();
                let bodies = world.bodies.clone();
                // generation makes no glass: a chunk without edits holds none (`mesh_cut`)
                let may_glass = !snap.is_empty();
                let greedy = self.opts.greedy;
                let cutout = match (k.level, self.opts.cutout) {
                    (0, true) => crate::mesh::Cut::Full,
                    (0, false) => crate::mesh::Cut::Shell,
                    _ => crate::mesh::Cut::Solid,
                };
                // the sky's time the clutter was set at: both hemispheres change season at once (`sky::season_at`)
                let (still, sky_t, _, density) = self.clutter.clone();
                // shortcut: every taken mark is looked through for every level-0 job; a map by chunk once a world
                // holds thousands
                let near = |b: &&Block| b.body == k.body && b.face == k.face && b.pos.x.div_euclid(CS) == k.pos.x && b.pos.y.div_euclid(CS) == k.pos.y;
                let taken: Vec<(IVec3, u32)> = if k.level == 0 { self.taken.iter().filter(|(b, _)| near(b)).map(|(b, y)| (b.pos, *y)).collect() } else { Vec::new() };
                // tiles of stored water are the water mesh's; a coarse column lies within one tile up to level 4, and past
                // that a far chunk's sea covers them (shortcut: a flood far away shows as sea level)
                let wet = if 1 << k.level <= TILE { world.water.wet_tiles(k.body, k.face, k.base().x, k.base().y, k.span()) } else { Vec::new() };
                let queued = Instant::now();
                rayon::spawn(move || {
                    let t = Instant::now();
                    let wait_us = (t - queued).as_secs_f32() * 1e6;
                    let mut edits = snap.edits();
                    // a cell whose block is a model is drawn by the client's models, not meshed (ADR 0047)
                    if models {
                        hide_models(&mut edits, &crate::blocks::BLOCKS);
                    }
                    edits.iter_mut().filter(|e| held.contains(&(e.0 as usize))).for_each(|e| e.1 = crate::blocks::AIR);
                    let body = &bodies[k.body as usize];
                    // the slopes read the padding before its skirts (ADR 0101)
                    let mut gen = generate_raw(body, k, &edits);
                    let raw = match &mut gen {
                        Gen::Cells(c) => {
                            let raw = c.clone();
                            skirt(c);
                            Some(raw)
                        }
                        _ => None,
                    };
                    let mut mesh = match &gen {
                        Gen::Cells(c) => {
                            let cracks = if k.level == 0 { cracks(body, k, c) } else { None };
                            Some(mesh_cracked(c, if greedy { body.lattice(k.level) as usize } else { 1 }, cutout, may_glass, cracks.as_deref(), &held, raw.as_deref())).filter(|m| !m.quads.is_empty())
                        }
                        _ => None,
                    };
                    if let (Gen::Cells(c), Some(m)) = (&gen, mesh.as_mut()) {
                        let q = crate::clutter::quads(body, k, c, &edits, &still, sky_t, density, &taken);
                        m.still = q.len() as u32;
                        m.quads.extend(q);
                    }
                    let mut sea = sea_quads(body, k, &wet, &gen);
                    sea.extend(river_quads(body, k, &wet, &gen));
                    if !sea.is_empty() {
                        let m = mesh.get_or_insert_with(|| crate::mesh::Mesh { quads: Vec::new(), counts: [0; 6], cut: 0, ray_cut: 0, fluff: 0, glass: 0, still: 0, water: 0 });
                        m.water = sea.len() as u32;
                        m.quads.extend(sea);
                    }
                    let clim = if mesh.is_some() { crate::clutter::chunk_climate(body, k) } else { 0 };
                    let _ = tx.send(JobResult { key: k, version, mesh, us: t.elapsed().as_secs_f64() * 1e6, wait_us, clim });
                });
            }
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
                    State::Ready { off, len: n, water: m.water, cut: m.cut, ray_cut: m.ray_cut, fluff: m.fluff, glass: m.glass, still: m.still }
                }
            };
            let s = self.slots.get_mut(&r.key).unwrap();
            s.clim = r.clim;
            if let State::Ready { off, len, .. } = s.state {
                self.alloc.release(off, len);
                self.released.push(off);
            }
            // a remesh that stays drawn changes only its range: the last walk's list takes it in place; any other
            // change of state can change what the walk picks
            match (s.state, new, &mut self.walk) {
                (State::Ready { .. }, State::Ready { off, len, water, cut, ray_cut, fluff, glass, still }, Some(w)) if s.drawn_at.0 == self.walk_frame => {
                    let d = &mut w.draws[(s.drawn_at.1 - w.first) as usize].0;
                    (d.off, d.len, d.water, d.cut, d.ray_cut, d.fluff, d.glass, d.still) = (off, len, water, cut, ray_cut, fluff, glass, still);
                }
                _ => self.dirty = true,
            }
            s.state = new;
            s.shown_v = r.version;
            st.applied += 1;
        }
    }

    /// The block rows changed (`live::rev`, ADR 0134): every slot gets a new version, so it is meshed again from
    /// the new rows while the old mesh stays drawn. True when they did: the caller's own tables are due too.
    pub fn set_defs(&mut self, rev: u64) -> bool {
        if self.defs_rev == rev {
            return false;
        }
        self.defs_rev = rev;
        for (_, sl) in self.slots.iter_mut() {
            sl.want_v += 1;
        }
        self.dirty = true;
        true
    }

    /// Fancy or fast leaves (ADR 0069): every level-0 slot gets a new version, so it is meshed again the new way
    /// while the old mesh stays drawn.
    pub fn set_cutout(&mut self, on: bool) {
        if self.opts.cutout == on {
            return;
        }
        self.opts.cutout = on;
        for (k, sl) in self.slots.iter_mut() {
            if k.level == 0 {
                sl.want_v += 1;
            }
        }
        self.dirty = true;
    }

    /// The clutter's models, the sky's time and the Clutter and Vegetation settings' densities (ADR 0082, 0091): level 0 is meshed again
    /// when the models are new, the year turns into another part (an eighth, ADR 0091) or begins again, or the density changes.
    /// shortcut: every level-0 chunk, though only those with seasonal rows change; by row when it shows in p99.
    pub fn set_clutter(&mut self, still: &Arc<Still>, t: f64, density: [f32; 2]) {
        let quarter = crate::clutter::stamp(t);
        let c = &self.clutter;
        if Arc::ptr_eq(&c.0, still) && c.2 == quarter && c.3 == density {
            return;
        }
        self.clutter = (still.clone(), t, quarter, density);
        for (k, sl) in self.slots.iter_mut() {
            if k.level == 0 {
                sl.want_v += 1;
            }
        }
        self.dirty = true;
    }

    /// A pick-up in cell `b` was taken in `year` (ADR 0082): the chunks that show it are meshed again.
    pub fn take(&mut self, world: &World, b: Block, year: u32) {
        self.taken.insert(b, year);
        self.touch_block(world, Block { pos: b.pos - IVec3::Z, ..b });
    }

    /// An edit changed a block: every chunk that samples it, in its cells or its padding, gets a new version.
    pub fn touch_block(&mut self, world: &World, b: Block) {
        // coarse chunks only where a far cell changed (ADR 0109)
        for k in sampling(world, b, world.far_levels(b)) {
            if let Some(sl) = self.slots.get_mut(&k) {
                sl.want_v += 1;
                self.touched.push(k);
            }
        }
    }

    /// Tile `t` became wet or dry: the chunks that draw its generated water (levels up to 4) are meshed again.
    pub fn touch_tile(&mut self, world: &World, t: Col) {
        for k in crate::world::water_keys(&world.bodies[t.body() as usize], t) {
            if let Some(sl) = self.slots.get_mut(&k) {
                sl.want_v += 1;
                self.touched.push(k);
            }
        }
    }

    /// True when every full-detail chunk that samples block `b` draws its latest version: an edit there is
    /// on screen (the feel report's click-to-change, fundamentals section 1).
    pub fn shows(&self, world: &World, b: Block) -> bool {
        sampling(world, b, 1).iter().all(|k| self.slots.get(k).is_none_or(|s| s.shown_v >= s.want_v))
    }

    pub fn evict(&mut self, age: u64) {
        // a slot is seen by walks: frames that only cull see none (ADR 0025, F)
        let f = self.walk_frame;
        let (alloc, released) = (&mut self.alloc, &mut self.released);
        let n = self.slots.len();
        self.slots.retain(|s| {
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

    /// ADR 0094: a row of exposed granite along j gets its joints (on the i faces' u edges) and no bedding; a row of
    /// limestone gets bedding lines on its open sides and no joints.
    #[test]
    fn exposed_rock_gets_its_joints_and_bedding_lines() {
        use crate::crack::{MJ, PJ};
        let w = World::new(1536, 64, 1337);
        let k = Key { body: 0, face: 2, level: 0, pos: IVec3::new(5, 9, 2) };
        let row = |m: u16| {
            let mut c = Box::new([0u16; P * P * P]);
            for y in 1..P - 1 {
                c[pidx(5, y, 5)] = m;
            }
            let k = cracks(&w.bodies[0], k, &c);
            let mesh = crate::mesh::mesh_cracked(&c, 8, crate::mesh::Cut::Solid, false, k.as_deref(), &[], None);
            (k, mesh.quads.iter().filter(|q| q[0] >> 28 & 7 < 2 && q[1] >> 20 & 15 != 0).count())
        };
        let (g, sides) = row(crate::blocks::BLOCKS.id("granite"));
        let g = g.expect("granite on show");
        assert!(sides >= 2, "{sides} i faces carry a joint edge");
        let joints = (1..P - 1).filter(|&y| g[pidx(5, y, 5)][0] & (PJ | MJ) != 0).count();
        assert!(joints >= 2, "{joints} joint sides along {} columns", P - 2);
        assert!((1..P - 1).all(|y| g[pidx(5, y, 5)][1..] == [0; 4]), "granite has no bedding");
        let _ = row(crate::blocks::BLOCKS.id("limestone"));
    }

    /// ADR 0109's benchmark: what the frame thread pays for far builds in a world of about 200k edits in one
    /// place (a quarry of digs and rubble, buildings, lamp posts). `cargo test -p engine --profile play --lib
    /// far_edit_cost -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn far_edit_cost() {
        use crate::blocks::{AIR, BLOCKS};
        use std::time::Instant;
        let mut w = World::new(6144, 1664, 1337);
        w.open_far();
        let top = w.air_over_ground(0, w.bodies[0].spawn);
        let (granite, rubble, lamp) = (BLOCKS.id("granite"), BLOCKS.id("gravel"), BLOCKS.id("lamp"));
        let at = |d: IVec3| Block { pos: top.pos + d, ..top };
        let t = Instant::now();
        let mut n = 0;
        for x in -80..80i32 {
            for y in -80..80i32 {
                for z in -8..4i32 {
                    let h = (x.wrapping_mul(73856093) ^ y.wrapping_mul(19349663) ^ z.wrapping_mul(83492791)) as u32 % 100;
                    let m = match () {
                        _ if x % 16 == 0 && y % 16 == 0 => lamp,
                        _ if z < 0 => if h < 70 { AIR } else { rubble },
                        _ if h < 30 => granite,
                        _ => continue,
                    };
                    w.set_block(at(IVec3::new(x, y, z)), m);
                    n += 1;
                }
            }
        }
        let load_ms = t.elapsed().as_secs_f64() * 1e3;
        let mut out = format!("{n} edits set in {load_ms:.0} ms");
        for level in [0u8, 4, 6] {
            let k = Key { body: 0, face: top.face, level, pos: top.pos.div_euclid(IVec3::splat(1 << level)).div_euclid(IVec3::splat(CS)) };
            let t = Instant::now();
            let mut len = 0;
            for _ in 0..20 {
                len = w.edits_for(k).len();
            }
            out += &format!("; edits_for level {level}: {:.0} us ({len} cells)", t.elapsed().as_secs_f64() * 1e6 / 20.0);
        }
        let opts = Opts { k: 4.0, greedy: false, cutout: false, horizon: true, prefetch_s: 0.0, max_inflight: 1 << 16 };
        let mut s = Streamer::new(&w, 1 << 22, opts);
        rayon::scope(|_| {}); // the worker pool starts on its first use: not a cost of the first edit
        let make =|s: &mut Streamer, w: &World, b: Block| {
            for k in sampling(w, b, !0) {
                s.slots.make(k, 0, || w.bodies[0].bounds(k));
            }
        };
        // one edit: the server's delta applied, its chunks touched and meshed again
        let b = at(IVec3::new(3, 5, 1));
        make(&mut s, &w, b);
        let t = Instant::now();
        w.set_block(b, granite);
        let t1 = Instant::now();
        s.touch_block(&w, b);
        let t2 = Instant::now();
        let touched = s.touched.len();
        let mut reqs: Vec<(f64, Key)> = s.touched.iter().map(|k| (0.0, *k)).collect();
        s.dispatch(&w, &mut reqs);
        let us = |a: Instant, b: Instant| (b - a).as_secs_f64() * 1e6;
        out += &format!("; one edit: set {:.0} us, touch {:.0} us, dispatch {:.0} us for {touched} chunks", us(t, t1), us(t1, t2), us(t2, Instant::now()));
        while !s.idle() {
            std::thread::sleep(std::time::Duration::from_millis(1));
            s.apply(usize::MAX, &mut FrameStats::default(), |_, _| {});
        }
        // a brush or a server batch of 20k cells: set and touched per cell, then one dispatch
        let cells: Vec<Block> = (0..20_000).map(|i| at(IVec3::new(i % 141 - 70, i / 141 - 70, 6))).collect();
        cells.iter().for_each(|&b| make(&mut s, &w, b));
        s.touched.clear();
        let t = Instant::now();
        for &b in &cells {
            w.set_block(b, granite);
        }
        let set_us = t.elapsed().as_secs_f64() * 1e6;
        let t = Instant::now();
        for &b in &cells {
            s.touch_block(&w, b);
        }
        let touch_us = t.elapsed().as_secs_f64() * 1e6;
        let mut reqs: Vec<(f64, Key)> = s.touched.iter().map(|k| (0.0, *k)).collect();
        reqs.sort_by_key(|r| (r.1.level, r.1.pos.x, r.1.pos.y, r.1.pos.z));
        reqs.dedup_by_key(|r| r.1);
        let t = Instant::now();
        let jobs = reqs.len();
        s.dispatch(&w, &mut reqs);
        out += &format!("; 20k-cell batch: set {:.1} ms, touched {:.1} ms, dispatch {:.1} ms for {jobs} chunks", set_us / 1e3, touch_us / 1e3, t.elapsed().as_secs_f64() * 1e3);
        while !s.idle() {
            std::thread::sleep(std::time::Duration::from_millis(1));
            s.apply(usize::MAX, &mut FrameStats::default(), |_, _| {});
        }
        eprintln!("[far_edit_cost] {out}");
    }

    /// ADR 0109: an edit meshes a coarse chunk again only where a far cell's pick changed: a dig off the middle
    /// of a cell that counts nothing bumps no coarse slot; the block that completes a line of s in a level 2
    /// cell bumps that level's chunk.
    #[test]
    fn an_edit_bumps_only_the_coarse_slots_whose_cells_changed() {
        let lamp = crate::blocks::BLOCKS.id("lamp");
        let mut w = World::new(1536, 64, 1337);
        w.open_far();
        let top = w.air_over_ground(0, w.bodies[0].spawn);
        let opts = Opts { k: 4.0, greedy: false, cutout: false, horizon: true, prefetch_s: 0.0, max_inflight: 64 };
        let mut s = Streamer::new(&w, 1 << 20, opts);
        // a cell of every level over the spot, its slots loaded
        let base = (top.pos - IVec3::new(0, 0, 3)).div_euclid(IVec3::splat(16)) * 16;
        let at = |d: IVec3| Block { pos: base + d, ..top };
        for k in sampling(&w, at(IVec3::ZERO), !0) {
            s.slots.make(k, 0, || w.bodies[0].bounds(k));
        }
        let edit = |w: &mut World, s: &mut Streamer, b: Block, m: u16| {
            s.touched.clear();
            w.set_block(b, m);
            s.touch_block(w, b);
            let mut l: Vec<u8> = s.touched.iter().map(|k| k.level).collect();
            l.dedup();
            l
        };
        assert_eq!(edit(&mut w, &mut s, at(IVec3::new(0, 1, 0)), crate::blocks::AIR), vec![0], "a dig off every middle");
        for z in 0..3 {
            edit(&mut w, &mut s, at(IVec3::new(1, 0, z)), lamp);
        }
        assert!(edit(&mut w, &mut s, at(IVec3::new(1, 0, 3)), lamp).contains(&2), "the fourth lamp of a level 2 cell");
    }

    /// ADR 0109: every chunk whose edits change after an edit is meshed again: at levels 0 to 4, a block that
    /// turns a corner cell of a chunk (the cell already holding s - 1 placed blocks) changes the edits of that
    /// chunk and the seven around the corner, and `sampling` names them all.
    #[test]
    fn every_chunk_an_edit_changes_is_sampled() {
        let lamp = crate::blocks::BLOCKS.id("lamp");
        let mut w = World::new(1536, 64, 1337);
        w.open_far();
        let top = w.air_over_ground(0, w.bodies[0].spawn);
        for level in 0..=4u8 {
            let s = 1i32 << level;
            let k0 = Key { body: 0, face: top.face, level, pos: top.pos.div_euclid(IVec3::splat(s * CS)) };
            for corner in [IVec3::ZERO, IVec3::splat(CS - 1)] {
                let cell = k0.pos * CS + corner;
                let around: Vec<Key> = (0..27).map(|i| Key { pos: k0.pos + IVec3::new(i % 3 - 1, i / 3 % 3 - 1, i / 9 - 1), ..k0 }).collect();
                let sorted = |w: &World, k: Key| {
                    let mut e = w.edits_for(k);
                    e.sort_unstable();
                    e
                };
                // s - 1 blocks up the cell's corner column, then the s-th
                for z in 0..s - 1 {
                    w.set_block(Block { body: 0, face: top.face, pos: cell * s + IVec3::new(0, 0, z) }, lamp);
                }
                let before: Vec<_> = around.iter().map(|&k| sorted(&w, k)).collect();
                let b = Block { body: 0, face: top.face, pos: cell * s + IVec3::new(0, 0, s - 1) };
                w.set_block(b, lamp);
                let keys = sampling(&w, b, !0);
                let mut changed = 0;
                for (k, e) in around.iter().zip(&before) {
                    if sorted(&w, *k) != *e {
                        changed += 1;
                        assert!(keys.contains(k), "level {level}: {k:?} changed but is not sampled");
                    }
                }
                assert_eq!(changed, 8, "level {level} corner {corner}: the cell's chunk and the seven around it");
            }
        }
    }

    /// Any block, not only a cell's middle, is sampled by the coarse chunk that holds it (ADR 0109): an edit
    /// off the middle can change a far cell, so that chunk is meshed again.
    #[test]
    fn a_block_off_the_middle_is_sampled_at_every_level() {
        let w = World::new(256, 32, 1337);
        let b = Block { body: 0, face: 0, pos: IVec3::new(100, 98, 3000) };
        let keys = sampling(&w, b, !0);
        for l in 0..=w.bodies[0].root_level {
            let own = Key { body: 0, face: 0, level: l, pos: b.pos.div_euclid(IVec3::splat(1 << l)).div_euclid(IVec3::splat(CS)) };
            assert!(keys.contains(&own), "level {l}");
        }
    }

    /// A block in a chunk's corner cell is in the padding of all seven chunks around that corner: the
    /// diagonal ones read it for their corner shading.
    #[test]
    fn a_corner_block_is_sampled_by_the_diagonal_chunks() {
        let w = World::new(1536, 64, 1337);
        let b = Block { body: 0, face: 2, pos: IVec3::new(5 * CS, 9 * CS, 2 * CS) };
        let keys = sampling(&w, b, 1);
        for (x, y, z) in (4..=5).flat_map(|x| (8..=9).flat_map(move |y| (1..=2).map(move |z| (x, y, z)))) {
            let k = Key { body: 0, face: 2, level: 0, pos: IVec3::new(x, y, z) };
            assert!(keys.contains(&k), "{k:?} not among {keys:?}");
        }
        assert_eq!(keys.len(), 8, "{keys:?}");
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

    /// A player in a cave deep under the ground (W4): the chunks around it are fine ones, meshed with the
    /// cave's walls, though the surface is far above (ADR 0062's open question).
    #[test]
    fn a_cave_deep_under_the_ground_streams_in_around_the_player() {
        let mut w = World::new(1536, 64, 1337);
        let top = w.air_over_ground(0, w.bodies[0].spawn);
        let at = DVec3::new(top.pos.x as f64 + 0.5, top.pos.y as f64 + 0.5, top.pos.z as f64 - 40.0);
        std::sync::Arc::get_mut(&mut w.bodies).unwrap()[0].set_caves(vec![crate::world::Cave { face: top.face, at, r: 4.0 }]);
        let b = &w.bodies[0];
        let cam = b.centre + b.dir(top.face as usize, at.x, at.y) * b.layer_r(at.z);
        let opts = Opts { k: 4.0, greedy: false, cutout: false, horizon: true, prefetch_s: 0.0, max_inflight: 64 };
        let mut s = Streamer::new(&w, 1 << 22, opts);
        let out = settle(&mut s, &w, cam);
        let cell = crate::world::Block { body: 0, face: top.face, pos: glam::IVec3::new(at.x as i32, at.y as i32, at.z as i32) };
        let k = Key { body: 0, face: top.face, level: 0, pos: cell.pos.div_euclid(glam::IVec3::splat(CS)) };
        let d = out.iter().find(|d| d.key == k).unwrap_or_else(|| panic!("the cave's chunk {k:?} is not drawn; finest drawn level {:?}", out.iter().map(|d| d.key.level).min()));
        assert!(d.len > 0 && !d.hidden, "the cave's chunk is drawn with its walls: {} quads, hidden {}", d.len, d.hidden);
    }

    /// A full quad buffer does not mesh the same chunks again every frame: after a failed allocation no job
    /// goes out for a chunk still waiting for a mesh until the buffer releases space, then they do.
    #[test]
    fn a_full_quad_buffer_waits_for_space_before_meshing_again() {
        let w = World::new(1536, 64, 1337);
        let opts = Opts { k: 4.0, greedy: false, cutout: false, horizon: true, prefetch_s: 0.0, max_inflight: 64 };
        let mut s = Streamer::new(&w, 1 << 20, opts);
        let all = s.alloc.alloc(1 << 20).unwrap(); // the buffer taken up by others
        let view = View::new(w.bodies[0].spawn_point() + w.bodies[0].spawn * 1.7, [Vec4::new(0.0, 0.0, 0.0, 1.0); 6], &w);
        let frame = |s: &mut Streamer| {
            let (mut out, mut reqs, mut st) = (Vec::new(), Vec::new(), FrameStats::default());
            while !s.idle() {
                std::thread::sleep(std::time::Duration::from_micros(200));
                s.apply(usize::MAX, &mut st, |_, _| {});
            }
            s.frame += 1;
            s.select(&w, &view, true, 1.0, &mut out, &mut reqs, &mut st);
            s.dispatch(&w, &mut reqs);
            s.inflight
        };
        for _ in 0..50 {
            if s.alloc_fail > 0 {
                break;
            }
            frame(&mut s);
        }
        assert!(s.alloc_fail > 0, "no allocation failed");
        // every frame walks (as while the camera moves), so the waiting chunks are asked for each time
        for f in 0..5 {
            s.dirty = true;
            assert_eq!(frame(&mut s), 0, "frame {f} after the failure: jobs went out");
        }
        s.alloc.release(all, 1 << 20);
        s.dirty = true;
        assert!(frame(&mut s) > 0, "space released, but no job went out");
    }

    #[test]
    fn coming_back_from_far_away_draws_the_fine_chunks_again() {
        // far away the near chunks' coarse ancestors are the leaves and get meshes; back on the ground those
        // ancestors split again, and their children, which never get meshes, must not keep them covering
        let w = World::new(1536, 64, 1337);
        let opts = Opts { k: 4.0, greedy: false, cutout: false, horizon: true, prefetch_s: 0.0, max_inflight: 64 };
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
        let opts = Opts { k: 4.0, greedy: false, cutout: false, horizon: true, prefetch_s: 0.0, max_inflight: 8 };
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
    fn a_fast_flight_leaves_no_near_chunk_coarse_once_the_camera_slows() {
        // F9 report 2026-11-12_21-00-30: flying 100 to 1000 m/s about 125 m up, a large near area stayed at a
        // coarse level. The client's frame as frame.rs runs it: a walk, the look-ahead every 4th frame, jobs
        // landing a few a frame; then the camera stops, and within a second every near chunk is at its level.
        let w = World::new(1536, 64, 1337);
        let opts = Opts { k: 4.0, greedy: false, cutout: false, horizon: true, prefetch_s: 1.0, max_inflight: 10 };
        let mut s = Streamer::new(&w, 1 << 23, opts);
        let b = &w.bodies[0];
        let (e1, e2) = (b.spawn.any_orthonormal_vector(), b.spawn.cross(b.spawn.any_orthonormal_vector()));
        let at = |x: f64, y: f64| {
            let d = (b.spawn + e1 * (x / b.radius) + e2 * (y / b.radius)).normalize();
            d * (b.top(d) + 125.0)
        };
        settle(&mut s, &w, at(0.0, 0.0));
        let (mut x, mut y, mut last) = (0.0, 0.0, at(0.0, 0.0));
        let mut frame = |s: &mut Streamer, cam: DVec3, vel: DVec3| {
            let view = View::new(cam, [Vec4::new(0.0, 0.0, 0.0, 1.0); 6], &w);
            let (mut out, mut reqs, mut st) = (Vec::new(), Vec::new(), FrameStats::default());
            std::thread::sleep(std::time::Duration::from_micros(300));
            s.apply(usize::MAX, &mut st, |_, _| {});
            s.frame += 1;
            s.select(&w, &view, true, 1.0, &mut out, &mut reqs, &mut st);
            let mut ahead = std::mem::take(&mut s.ahead_reqs);
            if vel.length() > 1.0 && s.frame % 4 == 0 {
                ahead.clear();
                let v = View::new(cam + vel * s.opts.prefetch_s, view.planes, &w);
                s.select(&w, &v, false, 1.5, &mut Vec::new(), &mut ahead, &mut FrameStats::default());
            }
            if vel.length() > 1.0 {
                reqs.extend_from_slice(&ahead);
            }
            s.ahead_reqs = ahead;
            s.dispatch(&w, &mut reqs);
            if s.frame % 30 == 0 {
                s.evict(120);
            }
            (out, st)
        };
        // 8 s at 150 fps: speeds stepping 100 -> 1000 m/s, the heading turning
        for f in 0..1200 {
            let speed = 100.0 + 900.0 * ((f / 150) as f64 / 7.0);
            let heading = f as f64 * 0.004;
            x += heading.cos() * speed / 150.0;
            y += heading.sin() * speed / 150.0;
            let cam = at(x, y);
            frame(&mut s, cam, (cam - last) * 150.0);
            last = cam;
        }
        // stopped: once nothing is missing or asked for, every near chunk is at its level
        let out = settle(&mut s, &w, last);
        let view = View::new(last, [Vec4::new(0.0, 0.0, 0.0, 1.0); 6], &w);
        let bad: Vec<(Key, f32)> = out.iter().filter(|d| !d.hidden && s.splits(&w, &view, &d.key, 4.0)).map(|d| (d.key, d.dist)).collect();
        assert_eq!(s.too_coarse(&w, &view, &out) as usize, bad.len());
        assert!(bad.is_empty(), "{} chunks drawn coarser than their level a second after stopping, e.g. {:?}", bad.len(), &bad[..bad.len().min(5)]);
    }

    #[test]
    fn a_coarse_chunk_does_not_cover_once_its_visible_children_are_in() {
        // F9 report 2026-11-12_21-00-30: a huge coarse chunk drawn over the near ground. A chunk covers while a
        // child is not meshed; children out of view rank last, so once every visible child was in, the coarse
        // chunk still covered (and stood over the fine ground) until the hidden ones came, long after
        let w = World::new(1536, 64, 1337);
        let opts = Opts { k: 4.0, greedy: false, cutout: false, horizon: true, prefetch_s: 0.0, max_inflight: 4 };
        let mut s = Streamer::new(&w, 1 << 23, opts);
        let b = &w.bodies[0];
        settle(&mut s, &w, b.spawn * b.radius * 1.6);
        // on the ground, looking one way: everything behind the camera is out of view
        let cam = b.spawn * (b.top(b.spawn) + 2.0);
        let look = b.spawn.any_orthonormal_vector();
        let mut planes = [Vec4::new(0.0, 0.0, 0.0, 1.0); 6];
        planes[0] = Vec4::new(look.x as f32, look.y as f32, look.z as f32, 0.0);
        let view = View::new(cam, planes, &w);
        for f in 0..20000 {
            let (mut out, mut reqs, mut st) = (Vec::new(), Vec::new(), FrameStats::default());
            std::thread::sleep(std::time::Duration::from_micros(200));
            s.apply(usize::MAX, &mut st, |_, _| {});
            s.frame += 1;
            s.select(&w, &view, true, 1.0, &mut out, &mut reqs, &mut st);
            s.dispatch(&w, &mut reqs);
            if st.missing == 0 && f > 0 {
                assert_eq!(s.too_coarse(&w, &view, &out), 0, "every visible child is in, yet coarse chunks are drawn over them (frame {f}, {} still in flight)", s.inflight);
                // a cover left out for a child out of view depends on the look: a frame that turns walks again, one
                // that looks the same way culls (it walked every frame while the hidden child waited, ADR 0111)
                let walks = s.walks;
                let (mut out, mut reqs, mut st) = (Vec::new(), Vec::new(), FrameStats::default());
                s.frame += 1;
                s.select(&w, &view, true, 1.0, &mut out, &mut reqs, &mut st);
                assert_eq!(s.walks, walks, "the same look walked again");
                // a step aside (under the 4 m a still camera allows) can bring the hidden child into view too
                s.frame += 1;
                s.select(&w, &View::new(cam + look, planes, &w), true, 1.0, &mut out, &mut reqs, &mut st);
                assert_eq!(s.walks, walks + 1, "a frame that only culls would keep the hole of a hidden child when the camera steps aside");
                let walks = s.walks;
                let mut turned = planes;
                turned[0] = -planes[0];
                s.frame += 1;
                s.select(&w, &View::new(cam, turned, &w), true, 1.0, &mut out, &mut reqs, &mut st);
                assert_eq!(s.walks, walks + 1, "a frame that only culls would keep the hole of a hidden child when the camera turns");
                return;
            }
        }
        panic!("the visible chunks never came in");
    }

    /// The empty leaves (the air over the ground, 400 m of it since ADR 0105, and the rock under it) are not
    /// visited one by one: the walk draws, asks for and counts what the slow walk does, in far fewer visits.
    #[test]
    fn the_walk_passes_over_empty_leaves_and_picks_the_same() {
        let w = World::new(1536, 64, 1337);
        let opts = Opts { k: 4.0, greedy: false, cutout: false, horizon: true, prefetch_s: 0.0, max_inflight: 64 };
        let mut s = Streamer::new(&w, 1 << 22, opts);
        let b = &w.bodies[0];
        let ground = b.spawn_point() + b.spawn * 1.7;
        settle(&mut s, &w, ground);
        // settled, then 60 m along and 30 m up: chunks split anew and wait for meshes
        for cam in [ground, ground + b.spawn.any_orthonormal_vector() * 60.0 + b.spawn * 30.0] {
            let view = View::new(cam, [Vec4::new(0.0, 0.0, 0.0, 1.0); 6], &w);
            let walk = |s: &mut Streamer, slow: bool, draw: bool| {
                // a frame each, so the slots each walk marks seen and drawn are its own
                (s.slow_walk, s.dirty, s.frame) = (slow, true, s.frame + 1);
                let v0 = s.visits;
                let (mut out, mut reqs, mut st) = (Vec::new(), Vec::new(), FrameStats::default());
                s.select(&w, &view, draw, 1.0, &mut out, &mut reqs, &mut st);
                let key = |k: &Key| (k.face, k.level, k.pos.x, k.pos.y, k.pos.z);
                let mut o: Vec<_> = out.iter().map(|d| (key(&d.key), d.off, d.len, d.hidden, d.dist.to_bits())).collect();
                let mut r: Vec<_> = reqs.iter().map(|q| (key(&q.1), q.0.to_bits())).collect();
                o.sort_unstable();
                r.sort_unstable();
                // the slots this walk marked seen, and drawn as leaves
                let marked = |f: fn(&Slot) -> u64| {
                    let mut m: Vec<_> = s.slots.iter().filter(|(_, sl)| f(sl) == s.frame).map(|(k, _)| key(k)).collect();
                    m.sort_unstable();
                    m
                };
                let marks = (marked(|sl| sl.last_seen), marked(|sl| sl.leaf_at));
                (o, r, (st.missing, st.holes, st.requests, s.dirty, marks), s.visits - v0)
            };
            for draw in [true, false] {
                let slow = walk(&mut s, true, draw);
                let fast = walk(&mut s, false, draw);
                assert!(!slow.0.is_empty() || !draw, "nothing drawn");
                assert_eq!((&fast.0, &fast.1, &fast.2), (&slow.0, &slow.1, &slow.2), "draw {draw}: the walks differ");
                // the look-ahead splits 10 % further out, so fewer of its children are leaves
                assert!(fast.3 * 3 < slow.3 * 2, "draw {draw}: {} visits against the slow walk's {}", fast.3, slow.3);
            }
        }
    }

    /// `dispatch` drops what cannot go out before it sorts: the jobs that go out are still the nearest that need
    /// one, in order, each chunk once though both walks asked for it, and none while no worker is free.
    #[test]
    fn dispatch_sends_the_nearest_that_need_a_job_once_each() {
        let w = World::new(1536, 64, 1337);
        let opts = Opts { k: 4.0, greedy: false, cutout: false, horizon: true, prefetch_s: 0.0, max_inflight: 8 };
        let mut s = Streamer::new(&w, 1 << 22, opts);
        let view = View::new(w.bodies[0].spawn_point() + w.bodies[0].spawn * 1.7, [Vec4::new(0.0, 0.0, 0.0, 1.0); 6], &w);
        let (mut out, mut reqs, mut st) = (Vec::new(), Vec::new(), FrameStats::default());
        s.frame += 1;
        s.select(&w, &view, true, 1.0, &mut out, &mut reqs, &mut st);
        // asked twice, as by the look-ahead, and for a chunk without a slot
        let twice = reqs.clone();
        reqs.extend(twice);
        reqs.push((0.0, Key { body: 0, face: 0, level: 0, pos: IVec3::new(-9, -9, -9) }));
        // the reference: sorted first (stable), then each taken once while a worker is free
        let mut sorted = reqs.clone();
        sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        // the nearest 300 chunks are at work already, so the eight that go out lie past the first that
        // `dispatch` sorts
        let mut busy: Vec<Key> = Vec::new();
        for (_, k) in &sorted {
            if busy.len() < 300 && !busy.contains(k) && s.slots.get(k).is_some() {
                busy.push(*k);
                s.slots.get_mut(k).unwrap().inflight = true;
            }
        }
        assert_eq!(busy.len(), 300);
        let mut want: Vec<Key> = Vec::new();
        for (_, k) in &sorted {
            if want.len() < 8 && s.slots.get(k).is_some_and(|sl| sl.needs_job()) && !want.contains(k) {
                want.push(*k);
            }
        }
        assert_eq!(want.len(), 8, "{} asked", reqs.len());
        s.dispatch(&w, &mut reqs);
        let mut got: Vec<Key> = s.slots.iter().filter(|(k, sl)| sl.inflight && !busy.contains(k)).map(|(k, _)| *k).collect();
        let key = |k: &Key| (k.face, k.level, k.pos.x, k.pos.y, k.pos.z);
        got.sort_by_key(key);
        want.sort_by_key(key);
        assert_eq!(got, want);
        assert!(reqs.is_empty());
        // no worker free: nothing more goes out, and the list is emptied all the same
        let mut more = sorted.clone();
        s.dispatch(&w, &mut more);
        assert!(more.is_empty() && s.inflight == 8);
    }

    /// The slab's links: a slot made after its parent is linked into it, one made before is found by its key;
    /// an evicted child is forgotten by its parent, and its place in the slab taken by the next slot made.
    #[test]
    fn a_slot_knows_its_children_through_making_and_eviction() {
        let bd = || Bounds { c: DVec3::ZERO, rad: 1.0 };
        let p = Key { body: 0, face: 2, level: 3, pos: IVec3::new(5, 6, 1) };
        let mut s = Slots::default();
        // a child made before its parent: looked up by key until the walk writes the link
        let early = s.make(p.child(6), 0, bd);
        let pi = s.make(p, 0, bd);
        assert_eq!(s.at(pi).kids, [UNKNOWN; 8]);
        assert_eq!(s.kid(pi, 6, &p.child(6)), early);
        assert_eq!(s.kid(pi, 1, &p.child(1)), NONE);
        s.at_mut(pi).kids[1] = NONE; // as the walk remembers a missing child
        // made after: linked at once, over a remembered NONE
        let late = s.make(p.child(1), 0, bd);
        assert_eq!((s.at(pi).kids[1], s.kid(pi, 1, &p.child(1))), (late, late));
        assert_eq!(s.make(p.child(1), 0, bd), late, "made once");
        for i in 0..8 {
            assert_eq!(parent(&p.child(i)), (p, i as usize));
        }
        // evicted: gone by key, its parent forgets it, its place is free for the next
        s.at_mut(pi).kids[6] = early;
        s.retain(|sl| sl.key != p.child(6));
        assert!(s.get(&p.child(6)).is_none() && s.len() == 2);
        assert_eq!(s.kid(pi, 6, &p.child(6)), NONE);
        let other = s.make(p.child(3), 0, bd);
        assert_eq!(other, early, "the freed place is used again");
        assert_eq!((s.kid(pi, 3, &p.child(3)), s.kid(pi, 6, &p.child(6))), (other, NONE), "no link left to the place's old chunk");
        assert_eq!(s.iter().count(), 3);
    }

    /// What a walk costs at detail 4 and 8, settled and after a move that leaves chunks waiting (a flight)
    /// (cargo test -p engine --profile play --lib walk_cost -- --ignored --nocapture).
    #[test]
    #[ignore]
    fn walk_cost() {
        let w = World::new(1536, 64, 1337);
        let b = &w.bodies[0];
        let ground = b.spawn_point() + b.spawn * 1.7;
        for k in [4.0, 8.0] {
            let opts = Opts { k, greedy: true, cutout: false, horizon: true, prefetch_s: 0.0, max_inflight: 64 };
            let mut s = Streamer::new(&w, 1 << 24, opts);
            settle(&mut s, &w, ground);
            for (what, cam) in [("settled", ground), ("moved", ground + b.spawn.any_orthonormal_vector() * 60.0 + b.spawn * 30.0)] {
                let view = View::new(cam, [Vec4::new(0.0, 0.0, 0.0, 1.0); 6], &w);
                let mut cost = |slow: bool| {
                    let (v0, t, n) = (s.visits, Instant::now(), 200);
                    let mut drawn = 0;
                    for _ in 0..n {
                        (s.slow_walk, s.dirty, s.frame) = (slow, true, s.frame + 1);
                        let (mut out, mut reqs, mut st) = (Vec::new(), Vec::new(), FrameStats::default());
                        s.select(&w, &view, true, 1.0, &mut out, &mut reqs, &mut st);
                        drawn = out.len();
                    }
                    format!("{{\"us\":{:.0},\"visits\":{},\"drawn\":{drawn}}}", t.elapsed().as_secs_f64() * 1e6 / n as f64, (s.visits - v0) / n)
                };
                let (fast, slow) = (cost(false), cost(true));
                println!("{{\"walk_cost\":{{\"k\":{k},\"camera\":\"{what}\",\"slots\":{},\"walk\":{fast},\"slow_walk\":{slow}}}}}", s.slots.len());
            }
        }
        // a quad buffer too small for detail 8, as in a forest (ADR 0111): the chunks that never come stay pending
        let opts = Opts { k: 8.0, greedy: true, cutout: false, horizon: true, prefetch_s: 0.0, max_inflight: 64 };
        let mut s = Streamer::new(&w, 1 << 19, opts);
        let view = View::new(ground, [Vec4::new(0.0, 0.0, 0.0, 1.0); 6], &w);
        let (mut missing, mut asked) = (0, 0);
        for f in 0..3000 {
            let (mut out, mut reqs, mut st) = (Vec::new(), Vec::new(), FrameStats::default());
            s.apply(usize::MAX, &mut st, |_, _| {});
            (s.frame, s.dirty) = (s.frame + 1, true);
            s.select(&w, &view, true, 1.0, &mut out, &mut reqs, &mut st);
            (missing, asked) = (st.missing, reqs.len());
            s.dispatch(&w, &mut reqs);
            if s.alloc_fail > 0 && s.idle() && f > 200 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let (t, n, v0) = (Instant::now(), 200, s.visits);
        let mut td = 0.0;
        for _ in 0..n {
            (s.dirty, s.frame) = (true, s.frame + 1);
            let (mut out, mut reqs, mut st) = (Vec::new(), Vec::new(), FrameStats::default());
            s.select(&w, &view, true, 1.0, &mut out, &mut reqs, &mut st);
            let d = Instant::now();
            s.dispatch(&w, &mut reqs);
            td += d.elapsed().as_secs_f64();
        }
        let total = t.elapsed().as_secs_f64();
        println!("{{\"walk_cost\":{{\"k\":8,\"camera\":\"buffer full\",\"slots\":{},\"missing\":{missing},\"asked\":{asked},\"alloc_fail\":{},\"walk_us\":{:.0},\"dispatch_us\":{:.0},\"visits\":{}}}}}", s.slots.len(), s.alloc_fail, (total - td) * 1e6 / n as f64, td * 1e6 / n as f64, (s.visits - v0) / n);
    }

    #[test]
    fn after_a_move_requests_radiate_out_from_the_camera() {
        let w = World::new(1536, 64, 1337);
        let opts = Opts { k: 4.0, greedy: false, cutout: false, horizon: false, prefetch_s: 0.0, max_inflight: 64 };
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

/// The crack faces (ADR 0094) of a level-0 chunk's cells that a face shows: cracking rock beside an open cell; None
/// when it has none (most chunks: no rock in sight). shortcut: a cell beside glass or a cutout block draws no crack.
/// The cracks of a level-0 chunk's exposed cracking rock (ADR 0094), padded as `c`: per cell its joint faces
/// (`crack::faces` less the bedding bits, drawn on the face edges) and the bedding line across each of its four side
/// faces that is open (`crack::bed`, from the folded top at the face's two vertical edges, so a line runs on through
/// the faces beside it). None for a chunk with none. Column and corner tops are looked up once each.
fn cracks(body: &crate::world::Body, k: Key, c: &[u16; P * P * P]) -> Option<Box<[[u8; 5]; P * P * P]>> {
    use crate::crack::{bed, faces, salt, ML, PL};
    let cracking = |m: u16| crate::blocks::BLOCKS.get(m).filter(|d| d.bedding > 0.0 || d.joints > 0);
    let base = k.base() - IVec3::ONE;
    let (seed, face) = (salt(body), k.face);
    let r = |l: i32| body.layer_r(l as f64) as f32;
    let (mut cols, mut corners) = (vec![f32::NAN; P * P], vec![f32::NAN; P * P]);
    let mut col = |x: usize, y: usize| {
        if cols[x + y * P].is_nan() {
            cols[x + y * P] = body.col_rock(face, base.x + x as i32, base.y + y as i32).top;
        }
        cols[x + y * P]
    };
    // corner (x, y) is the grid point at the low i, low j edge of padded column (x, y)
    let mut corner = |x: usize, y: usize| {
        if corners[x + y * P].is_nan() {
            let (ci, cj) = ((base.x + x as i32) as f64, (base.y + y as i32) as f64);
            corners[x + y * P] = body.rock_at(face as usize, ci, cj, body.dir(face as usize, ci, cj)).top;
        }
        corners[x + y * P]
    };
    let mut out: Option<Box<[[u8; 5]; P * P * P]>> = None;
    for z in 1..P - 1 {
        for y in 1..P - 1 {
            for x in 1..P - 1 {
                let m = c[pidx(x, y, z)];
                let Some(d) = cracking(m) else { continue };
                // the side faces +i, -i, +j, -j that are open, then the top and foot
                let open = [c[pidx(x + 1, y, z)], c[pidx(x - 1, y, z)], c[pidx(x, y + 1, z)], c[pidx(x, y - 1, z)], c[pidx(x, y, z + 1)], c[pidx(x, y, z - 1)]].map(|n| n == 0);
                if !open.contains(&true) {
                    continue;
                }
                let g = base + IVec3::new(x as i32, y as i32, z as i32);
                let (lo, hi) = (r(g.z), r(g.z + 1));
                let mut e = [faces(m, seed, face, g.x, g.y, col(x, y), r(g.z - 1), lo, hi) & !(PL | ML), 0, 0, 0, 0];
                if d.bedding > 0.0 {
                    // each side's two vertical edges, low end first along the face (j on an i face, i on a j face)
                    let ends = [((x + 1, y), (x + 1, y + 1)), ((x, y), (x, y + 1)), ((x, y + 1), (x + 1, y + 1)), ((x, y), (x + 1, y))];
                    for (s, &((ax, ay), (bx, by))) in ends.iter().enumerate() {
                        if open[s] {
                            e[1 + s] = bed(m, corner(ax, ay), corner(bx, by), lo, hi);
                        }
                    }
                }
                if e != [0; 5] {
                    out.get_or_insert_with(|| Box::new([[0; 5]; P * P * P]))[pidx(x, y, z)] = e;
                }
            }
        }
    }
    out
}
