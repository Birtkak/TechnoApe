//! Server: the item ledger, the water and sky sent to players, the bucket and shape checks.
use super::*;

impl Server {
    /// Of each item, how many every player (online or not) and every pile holds.
    pub(super) fn held_now(&self) -> BTreeMap<u16, i64> {
        let mut h: BTreeMap<u16, i64> = BTreeMap::new();
        let slots = self.clients.iter().map(|c| &c.slots).chain(self.absent.values().map(|p| &p.slots)).chain(self.piles.values().map(|p| &p.slots)).chain(self.furnaces.values().map(|f| &f.slots));
        for st in slots.flatten().flatten() {
            *h.entry(st.item).or_default() += st.n as i64;
        }
        h
    }

    /// The items' conservation law (ADR 0074, fundamentals 4.8): each item whose count over every player and
    /// pile is not what digs, places, creative's grid and /give made of it, as (item, held, made).
    /// shortcut: a ledger of items, not of matter; the planet's material ledger (ore-budget.md) counts the
    /// ground and does not see what is carried or lying in piles yet.
    pub fn item_ledger(&self) -> Vec<(u16, i64, i64)> {
        let held = self.held_now();
        let ids: std::collections::BTreeSet<u16> = held.keys().chain(self.item_made.keys()).copied().collect();
        ids.into_iter().map(|k| (k, held.get(&k).copied().unwrap_or(0), self.item_made.get(&k).copied().unwrap_or(0))).filter(|(_, a, b)| a != b).collect()
    }

    /// The water tiles client `i` has the latest version of.
    pub fn water_seen(&self, i: usize) -> Vec<Col> {
        let mut v: Vec<Col> = self.clients[i].tiles.iter().filter(|(t, v)| self.tiles[*t].0 == **v).map(|(t, _)| *t).collect();
        v.sort_unstable();
        v
    }

    /// Columns whose water changed: their tiles get this tick as version.
    pub(super) fn water_changed(&mut self, cols: &[Col]) {
        for c in cols {
            let t = c.tile();
            let b = &self.world.bodies[t.body() as usize];
            let mid = (TILE as f64) * 0.5;
            let at = b.centre + b.dir(t.face() as usize, (t.i() * TILE) as f64 + mid, (t.j() * TILE) as f64 + mid) * b.radius;
            let near = &mut self.near;
            self.tiles
                .entry(t)
                .or_insert_with(|| {
                    near.entry(cell(at)).or_default().push(t);
                    (0, at)
                })
                .0 = self.tick + 1;
        }
    }

    /// Sends every client the water tiles it lacks: within WATER_RANGE, nearest first, until WATER_BYTES.
    /// The rest waits for the next tick, so a big flood reaches a client over a few ticks, near water first.
    /// The air's columns to every client when the grid landed a step or the forced weather changed (ADR 0076):
    /// those that moved more than a byte's step since they were sent, so a slow drift does not cost a message
    /// a column every step.
    pub(super) fn send_sky(&mut self) {
        let Some(w) = &self.weather else { return };
        if self.sky_at == Some(w.view().hash) {
            return;
        }
        self.sky_at = Some(w.view().hash);
        let cells = w.cells(self.force.as_ref());
        if self.sky.len() != cells.len() {
            self.sky = cells;
            for c in &self.clients {
                c.link.send(ToClient::Sky { t: self.world.time, all: self.sky.clone(), changed: Vec::new() });
            }
            return;
        }
        let mut changed = Vec::new();
        for (k, (old, new)) in self.sky.iter_mut().zip(&cells).enumerate() {
            if old.iter().zip(new).any(|(a, b)| a.abs_diff(*b) > 1) {
                *old = *new;
                changed.push((k as u32, *new));
            }
        }
        if !changed.is_empty() {
            for c in &self.clients {
                c.link.send(ToClient::Sky { t: self.world.time, all: Vec::new(), changed: changed.clone() });
            }
        }
    }

    pub(super) fn send_water(&mut self) {
        // each tile built and measured once a tick, however many clients get it
        let mut built: FxMap<Col, (Vec<engine::liquid::ColWater>, usize)> = FxMap::default();
        // what each client lacks, nearest first: the tiles of the 27 cells round it. On this thread: a pass side by
        // side waited on rayon's global pool, which in a game's own server is the client's, full of chunk jobs and
        // world builds' cave walks (a sim's 50 bots' held ticks 100 to 600 ms)
        let wants: Vec<Vec<(u64, Col)>> = self.clients.iter().map(|c| wanted(&self.tiles, &self.near, DVec3::from_array(c.player.pos), &c.tiles)).collect();
        for (c, want) in self.clients.iter_mut().zip(wants) {
            if want.is_empty() {
                continue;
            }
            let (mut msg, mut bytes) = (Vec::new(), 0);
            for (_, t) in want {
                let (cols, n) = built.entry(t).or_insert_with(|| {
                    let cols = self.world.water.tile_sent(t);
                    let n = postcard::to_allocvec(&(t, &cols)).map_or(0, |b| b.len());
                    (cols, n)
                });
                bytes += *n;
                c.tiles.insert(t, self.tiles[&t].0);
                msg.push((t, cols.clone()));
                if bytes >= WATER_BYTES {
                    break;
                }
            }
            self.water_bytes += bytes as u64;
            c.link.send(ToClient::Water(msg));
        }
    }

    /// Client `i`'s bucket at block `at`: `fill` scoops up to a full bucket from the water there, else the
    /// bucket is poured out there (into the gap that holds `at`, which must be air). Creative scoops and pours
    /// a full bucket every time, holds nothing, and the ledger counts it.
    pub(super) fn bucket(&mut self, i: usize, at: Block, fill: bool) -> bool {
        let body = self.world.bodies.get(at.body as usize);
        // a gas body holds no water (step 6a): it would sink through its air cells for ever
        let inside = body.is_some_and(|b| !b.gas && at.face < 6 && (0..b.n).contains(&at.pos.x) && (0..b.n).contains(&at.pos.y) && (0..b.layers).contains(&at.pos.z));
        let feet = DVec3::from_array(self.clients[i].player.pos);
        if !inside || self.clients[i].vitals.dead.is_some() || self.world.block_centre(at).distance(feet) > REACH || BLOCKS.solid(self.world.block(at)) {
            return false;
        }
        let col = World::col(at);
        let c = &mut self.clients[i];
        let World { bodies, edits, water, sky, time, .. } = &mut self.world;
        let g = engine::world::Solid::at(bodies, edits, sky, *time);
        let ok = match (fill, c.creative) {
            (true, false) => {
                let took = water.scoop(&g, col, at.pos.z, U - c.water as i64);
                c.water += took as u32;
                took > 0
            }
            (true, true) => {
                self.removed += water.scoop(&g, col, at.pos.z, U);
                true
            }
            (false, false) => c.water > 0 && water.pour(&g, col, at.pos.z, std::mem::take(&mut c.water) as i64),
            (false, true) => {
                let ok = water.pour(&g, col, at.pos.z, U);
                self.made += if ok { U } else { 0 };
                ok
            }
        };
        c.inv_dirty |= ok && !c.creative;
        ok
    }

    /// Client `i`'s use button held on cell `at` to drink, or let go (`ToServer::Drink`): whether the cell holds
    /// fresh water is judged here, once a cell, not every step it is held.
    /// shortcut: water that drains or comes while the button stays on one cell keeps the first verdict
    pub(super) fn hold_drink(&mut self, i: usize, at: Option<Block>) {
        if self.clients[i].drink.map(|d| d.0) == at {
            return;
        }
        let w = &self.world;
        let fresh = |b: Block| {
            let inside = w.bodies.get(b.body as usize).is_some_and(|d| b.face < 6 && (0..d.n).contains(&b.pos.x) && (0..d.n).contains(&b.pos.y) && (0..d.layers).contains(&b.pos.z));
            inside && w.water_surface(w.block_centre(b)).is_some() && !w.salt(b)
        };
        self.clients[i].drink = at.map(|b| (b, fresh(b)));
    }

    /// The trust boundary: a box within the size limits on a face that exists, a known block, every cell
    /// within the player's reach.
    /// A block a client named is one of the world's: its body, its face and its cell exist. Asked before anything
    /// indexes by it (a made-up `Use` or `Take` took the server down; the review of the crafting grid).
    pub(super) fn exists(&self, b: Block) -> bool {
        self.world.bodies.get(b.body as usize).is_some_and(|w| b.face < 6 && (0..w.n).contains(&b.pos.x) && (0..w.n).contains(&b.pos.y) && (0..w.layers).contains(&b.pos.z))
    }

    pub(super) fn valid_shape(&self, s: &Shape, m: u16, at: DVec3) -> bool {
        s.fits(&self.world)
            && BLOCKS.get(m).is_some()
            && s.cells().all(|b| self.world.block_centre(b).distance(at) <= REACH)
    }
}
