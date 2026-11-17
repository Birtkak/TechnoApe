//! Server: furnaces in the world (ADR 0141): their slots, fires and work, stepped every tick.
use super::*;
use engine::furnace::Furnace;
use engine::recipes::RECIPES;

impl Server {
    /// Whether a living client with its feet at `feet` reaches the furnace at `at`: a block that exists, a furnace
    /// (`RECIPES.furnace`), its middle within `items::BENCH_REACH`.
    pub(super) fn reaches_furnace(&self, at: Block, feet: DVec3) -> bool {
        self.exists(at) && RECIPES.furnace(self.world.block(at)).is_some() && self.world.block_centre(at).distance(feet) <= items::BENCH_REACH
    }

    /// Client i chooses the cast of the furnace at `at` (`ToServer::Choose`): alive, reaching it, and a cast
    /// that furnace reaches (or none).
    pub(super) fn choose(&mut self, i: usize, at: Block, cast: Option<String>) {
        let c = &self.clients[i];
        if c.vitals.dead.is_some() || !self.reaches_furnace(at, c.phys.pos) {
            return;
        }
        let top = RECIPES.furnace(self.world.block(at)).map_or(0.0, |f| f.top);
        if cast.as_ref().is_some_and(|n| !RECIPES.casts(top).any(|w| &w.name == n)) {
            return;
        }
        self.furnaces.entry(key(at)).or_default().choice = cast;
    }

    /// Every furnace one tick on, in the order of their cells: one whose block is no furnace any more (dug,
    /// replaced) lays what it held there, nothing lost; the rest step (`Furnace::step`), the ledger moving with
    /// what they use and make, and drop out of the list when idle. A fire lit or gone out is told to everyone;
    /// an open window, to its player each tick it changed; one walked away from (or gone) closes.
    pub(super) fn furnaces(&mut self) {
        let dt = self.dt as f32;
        let keys: Vec<PileKey> = self.furnaces.keys().copied().collect();
        for k in keys {
            let b = unkey(k);
            let Some(def) = RECIPES.furnace(self.world.block(b)) else {
                let f = self.furnaces.remove(&k).unwrap();
                if f.burning() {
                    self.fires_changed.push((b, false));
                }
                self.lay(b, f.slots.into_iter().flatten().collect(), (0, 0));
                continue;
            };
            let f = self.furnaces.get_mut(&k).unwrap();
            let was = f.burning();
            for (item, d) in f.step(def, &RECIPES, dt) {
                *self.item_made.entry(item).or_default() += d;
            }
            if f.burning() != was {
                self.fires_changed.push((b, f.burning()));
            }
            if f.idle() {
                self.furnaces.remove(&k);
            }
        }
        for i in 0..self.clients.len() {
            let c = &self.clients[i];
            let Some(b) = c.furnace else { continue };
            if c.vitals.dead.is_some() || !self.reaches_furnace(b, c.phys.pos) {
                (self.clients[i].furnace, self.clients[i].sent_furnace) = (None, None);
                continue;
            }
            let f = self.furnaces.get(&key(b)).cloned().unwrap_or_default();
            let c = &mut self.clients[i];
            if c.sent_furnace.as_ref() != Some(&f) {
                c.link.send(ToClient::Furnace { at: b, f: f.clone() });
                c.sent_furnace = Some(f);
            }
        }
        if !self.fires_changed.is_empty() {
            let fires = std::mem::take(&mut self.fires_changed);
            self.events.extend(fires.iter().map(|&(at, on)| Event::Fire { at, on }));
            for c in &self.clients {
                c.link.send(ToClient::Fires(fires.clone()));
            }
        }
    }

    /// The furnaces burning now: what a joining client is told.
    pub(super) fn fires(&self) -> Vec<(Block, bool)> {
        self.furnaces.iter().filter(|(_, f)| f.burning()).map(|(k, _)| (unkey(*k), true)).collect()
    }

    /// The furnace at `at`'s slots (a fresh one's when none is kept).
    pub(super) fn furnace_slots(&self, at: Block) -> Vec<Slot> {
        self.furnaces.get(&key(at)).map_or_else(|| Furnace::default().slots, |f| f.slots.clone())
    }
}
