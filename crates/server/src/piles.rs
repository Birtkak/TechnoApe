//! Server: digs, strikes, edits, rubble, piles and moving items.
use super::*;

impl Server {
    /// Client i holds its dig on `at` now, or on nothing (ADR 0034; `edit::Hold` keeps a left shape's
    /// seconds), and the others are told.
    pub(super) fn dig(&mut self, i: usize, at: Option<Shape>) {
        let now = self.clock();
        let pos = DVec3::from_array(self.clients[i].player.pos);
        let at = at.filter(|s| s.sane() && !self.clients[i].creative && self.valid_shape(s, AIR, pos) && edit::dig_time(&self.world, s) > 0.0);
        let (world, c) = (&self.world, &mut self.clients[i]);
        if !c.hold.on(at, now) {
            return;
        }
        let tool = c.tool();
        let told = at.map(|s| (s, edit::dig_time_with(world, &s, tool))).filter(|(_, need)| *need > 0.0).map(|(s, need)| (s, (c.hold.held(&s, now) / need).min(1.0), 1.0 / need));
        self.tell_dig(i, told);
    }

    /// Every other player hears of client i's dig.
    pub(super) fn tell_dig(&self, i: usize, at: Option<(Shape, f32, f32)>) {
        let id = self.clients[i].player.id;
        for (k, o) in self.clients.iter().enumerate() {
            if k != i {
                o.link.send(ToClient::Digging { id, at });
            }
        }
    }

    /// Client `i` digs or places `shape`: all of it or none. False when the shape fails `valid_shape`, a
    /// survival dig was not held for its time, or a survival player cannot pay for every block it places.
    pub(super) fn edit(&mut self, i: usize, shape: &Shape, m: u16, changed: &mut Vec<(Block, u16)>) -> bool {
        if self.clients[i].vitals.dead.is_some() || !self.valid_shape(shape, m, DVec3::from_array(self.clients[i].player.pos)) {
            return false;
        }
        // a survival dig is taken once it was held for its time (ADR 0034), with the tool in hand (ADR 0110).
        // shortcut: the time held counts whatever was in hand meanwhile, so a tool taken up at the end of a
        // bare-hand hold digs at once; a hold per tool when that matters
        let tool = self.clients[i].tool();
        if m == AIR && !self.clients[i].creative {
            let now = self.clock();
            let held = self.clients[i].hold.held(shape, now);
            let need = edit::dig_time_with(&self.world, shape, tool);
            if held + DIG_LAG_S + need * DIG_LAG_SHARE < need {
                return false;
            }
        }
        // a solid block never goes where it would trap a player
        let (world, clients) = (&self.world, &self.clients);
        let cells = edit::changes(world, shape, m, |b| clients.iter().any(|c| phys::holds(world, &c.phys, b)));
        // a block that hangs (a torch, ADR 0117) goes only where a floor or a wall holds it, and never into water
        if BLOCKS.get(m).is_some_and(|d| d.hangs) && cells.iter().any(|&(b, _)| engine::fall::held_from(world, b).is_none() || world.water_surface(world.block_centre(b)).is_some()) {
            return false;
        }
        let c = &mut self.clients[i];
        // a survival dig of rock or ore leaves its rubble in the cell (ADR 0093)
        let leave = |old: u16| if m == AIR { edit::dug(old, !c.creative) } else { m };
        let mut drops = Vec::new();
        if !c.creative && !cells.is_empty() {
            if m == AIR {
                // bare hands take only loose blocks; the rest breaks into nothing unless the tool in hand takes it
                // (ADR 0034, 0110; rock leaves its rubble either way). Rubble goes into the slots (mining.md 4: no
                // pile to walk over); with no room for all of it the dig is refused and the rubble stays a block
                let (mut slots, mut taken) = (c.slots.clone(), Vec::new());
                let takes = |d: &engine::blocks::Def| d.loose || tool.is_some_and(|t| t.takes(d));
                for &(b, old) in cells.iter().filter(|(_, old)| leave(*old) == AIR && BLOCKS.get(*old).is_some_and(takes)) {
                    if BLOCKS.get(old).is_none_or(|d| d.rubble_of.is_none()) {
                        drops.push(old);
                        // what a generated cell holds besides its block (ADR 0097: flint in gravel)
                        drops.extend(edit::held(&self.world, b, old));
                    } else if items::give(&mut slots, Stack::new(old, 1)) > 0 {
                        return false;
                    } else {
                        taken.push(old);
                    }
                }
                // each cell the tool dug costs it a use (a hammer's shape as many); its last leaves nothing (ADR 0047,
                // 0110), the ledger's "worn away"
                let fit = tool.map_or(0, |t| cells.iter().filter(|(_, old)| BLOCKS.get(*old).is_some_and(|d| t.fits(d))).count());
                let worn = c.slots[c.hand as usize].map_or(AIR, |s| s.item);
                if (0..fit).any(|_| items::wear(&mut slots[c.hand as usize])) {
                    *self.item_made.entry(worn).or_default() -= 1;
                    // what it leaves (a metal tool's stick) is in its slot now
                    if let Some(l) = slots[c.hand as usize] {
                        *self.item_made.entry(l.item).or_default() += 1;
                    }
                }
                c.slots = slots;
                for old in taken {
                    *self.item_made.entry(old).or_default() += 1;
                }
                c.vitals.dug(cells.len());
            } else {
                if !items::take(&mut c.slots, m, cells.len() as u32) {
                    return false;
                }
                *self.item_made.entry(m).or_default() -= cells.len() as i64;
            }
            c.inv_dirty = true;
        }
        // a survival dig on a crack loosens its neighbours along it into rubble too (ADR 0094), read before the
        // dig changes the world; and the roof over an opening it made too wide for its rock (ADR 0112)
        let loose: Vec<(Block, u16)> = if m == AIR && !c.creative {
            let mut loose: Vec<(Block, u16)> = cells.iter().flat_map(|&(b, old)| engine::crack::drop(&self.world, b, old).into_iter().map(move |n| (n, old))).filter(|(n, _)| !cells.iter().any(|c| c.0 == *n)).collect();
            let (dug, open): (Vec<Block>, Vec<Block>) = (cells.iter().map(|c| c.0).collect(), loose.iter().map(|c| c.0).collect());
            let roof = engine::crack::undercut(&self.world, &dug, &open);
            loose.extend(roof.into_iter().map(|b| (b, self.world.block(b))));
            loose
        } else {
            Vec::new()
        };
        for &(b, old) in cells.iter().chain(&loose) {
            let m = leave(old);
            if self.world.block(b) != m {
                self.world.set_block(b, m);
                changed.push((b, m));
            }
        }
        if !drops.is_empty() {
            self.drop_dug(i, &cells, drops);
        }
        true
    }

    /// Client i strikes what is in hand on block `at` (`ToServer::Strike`, ADR 0110): a survival player alive,
    /// within reach, a knapping stone in hand and hard rock there; a strike sooner than `KNAP_WAIT_S` after the
    /// last that counted, or of another item, does not count (another item starts again). At the item's `knap`
    /// count one of it becomes the item it knaps into, in its slot when that empties, else where it fits, else at
    /// the feet.
    pub(super) fn strike(&mut self, i: usize, at: Block) {
        // a prospecting tool in hand prospects; a knapping stone knaps
        if let Some(range) = self.clients[i].tool().and_then(|t| t.prospects()) {
            return self.prospect(i, at, range);
        }
        let tick = self.tick;
        let wait = (items::KNAP_WAIT_S * TICK_HZ).round() as u64;
        let feet = self.clients[i].phys.pos;
        let rock = self.valid_shape(&Shape::one(at), AIR, feet) && BLOCKS.get(self.world.block(at)).is_some_and(items::knaps);
        let c = &mut self.clients[i];
        let k = c.hand as usize;
        let Some(st) = c.slots[k].filter(|_| rock && !c.creative && c.vitals.dead.is_none()) else { return };
        let Some((into, strikes)) = ITEMS.get(st.item).and_then(|d| d.knap) else { return };
        if c.knap.0 != st.item {
            c.knap = (st.item, 0, 0);
        } else if c.knap.1 > 0 && tick < c.knap.2 + wait {
            return;
        }
        c.knap = (st.item, c.knap.1 + 1, tick);
        if c.knap.1 < strikes {
            return;
        }
        c.knap.1 = 0;
        c.slots[k] = (st.n > 1).then_some(Stack { n: st.n - 1, ..st });
        let made = Stack::new(into, 1);
        let over = if c.slots[k].is_none() {
            c.slots[k] = Some(made);
            0
        } else {
            items::give(&mut c.slots, made)
        };
        c.inv_dirty = true;
        *self.item_made.entry(st.item).or_default() -= 1;
        *self.item_made.entry(into).or_default() += 1;
        if over > 0 {
            self.lay(self.world.block_at(feet + phys::up(&self.world, feet) * 0.5), vec![made], (0, 0));
        }
    }

    /// Client i strikes the prospecting tool in hand on block `at` (ADR 0138): alive,
    /// within reach, hard rock there (`items::knaps`: rock, ore, the altered rocks), at most one every
    /// `items::PROSPECT_WAIT_S`. It costs the tool a use (its last leaves nothing) and only that player is
    /// answered, with what `ore::prospect` hears within the tool's `range`.
    fn prospect(&mut self, i: usize, at: Block, range: f64) {
        let wait = (items::PROSPECT_WAIT_S * TICK_HZ).round() as u64;
        let feet = self.clients[i].phys.pos;
        let rock = self.valid_shape(&Shape::one(at), AIR, feet) && BLOCKS.get(self.world.block(at)).is_some_and(items::knaps);
        let (tick, c) = (self.tick, &self.clients[i]);
        if !rock || c.vitals.dead.is_some() || c.prospected.is_some_and(|t| tick < t + wait) {
            return;
        }
        let heard = engine::ore::prospect(&self.world, at, range, &self.ore_open);
        let c = &mut self.clients[i];
        let k = c.hand as usize;
        let worn = c.slots[k].map_or(AIR, |s| s.item);
        if items::wear(&mut c.slots[k]) {
            *self.item_made.entry(worn).or_default() -= 1;
            if let Some(l) = c.slots[k] {
                *self.item_made.entry(l.item).or_default() += 1;
            }
        }
        (c.prospected, c.inv_dirty) = (Some(tick), true);
        c.link.send(ToClient::Prospect(heard));
    }

    /// Client i takes its crafting grid's output up to `n` times (`ToServer::Craft`, ADR 0140):
    /// alive, in either mode, the cells holding the recipe named `recipe` (the 3 x 3 only at a workbench); as many
    /// as the cells hold and the slots have room for (`recipes::make`), the ledger moving with them.
    pub(super) fn craft(&mut self, i: usize, recipe: &str, n: u16) {
        let three = self.at_bench(i);
        let c = &mut self.clients[i];
        let Some(r) = engine::recipes::RECIPES.matched(&c.slots, three).filter(|r| r.name == recipe && c.vitals.dead.is_none()) else { return };
        let (made, used) = engine::recipes::make(&mut c.slots, r, n as u32);
        for m in used {
            *self.item_made.entry(m).or_default() -= 1;
        }
        *self.item_made.entry(r.out.0).or_default() += made as i64 * r.out.1 as i64;
        c.inv_dirty = true;
    }

    /// Client i uses block `at` (`ToServer::Use`): alive, within `BENCH_REACH`, a block with a window. A workbench
    /// opens its 3 x 3; a furnace its window (ADR 0141), sent as it is at once and while it changes.
    pub(super) fn use_block(&mut self, i: usize, at: Block) {
        if !self.exists(at) {
            return;
        }
        let near = self.world.block_centre(at).distance(self.clients[i].phys.pos) <= items::BENCH_REACH;
        let window = BLOCKS.get(self.world.block(at)).and_then(|d| d.window.as_deref());
        let furnace = self.reaches_furnace(at, self.clients[i].phys.pos);
        let c = &mut self.clients[i];
        if near && c.vitals.dead.is_none() && window == Some("workbench") {
            c.bench = Some(at);
        } else if furnace && c.vitals.dead.is_none() {
            (c.furnace, c.sent_furnace) = (Some(at), None);
        }
    }

    /// Whether client i's workbench is open: it used one that is still there and within `BENCH_REACH`.
    pub(super) fn at_bench(&self, i: usize) -> bool {
        let c = &self.clients[i];
        c.bench.is_some_and(|b| BLOCKS.get(self.world.block(b)).is_some_and(|d| d.window.as_deref() == Some("workbench")) && self.world.block_centre(b).distance(c.phys.pos) <= items::BENCH_REACH)
    }

    /// Client i's crafting grid is put away (`ToServer::Close`, a walk away from the bench, a death, a join, a
    /// leave): its window closes and every cell goes into the usable slots, what does not fit at its feet.
    pub(super) fn uncraft(&mut self, i: usize) {
        let c = &mut self.clients[i];
        (c.bench, c.furnace) = (None, None);
        let cells: Vec<Stack> = c.slots[items::CRAFT_AT..].iter_mut().filter_map(Option::take).collect();
        if cells.is_empty() {
            return;
        }
        c.inv_dirty = true;
        let over: Vec<Stack> = cells.into_iter().filter_map(|st| Some(Stack { n: items::give(&mut c.slots, st), ..st }).filter(|o| o.n > 0)).collect();
        let feet = c.phys.pos;
        self.lay(self.world.block_at(feet + phys::up(&self.world, feet) * 0.5), over, (0, 0));
    }

    /// Every player's grid as the rules want it each tick: put away from a dead player, and from one whose
    /// workbench went or was left out of reach.
    pub(super) fn benches(&mut self) {
        for i in 0..self.clients.len() {
            let c = &self.clients[i];
            let full = c.slots[items::CRAFT_AT..].iter().any(Option::is_some);
            if (c.vitals.dead.is_some() && (full || c.bench.is_some())) || (c.bench.is_some() && !self.at_bench(i)) {
                self.uncraft(i);
            }
        }
    }

    /// A falling rock that moved into a survival player's head this tick hits it (ADR 0094: a drop hurts who is
    /// under it); sand only buries. A block falls through a head once, so it hits once.
    pub(super) fn rock_hits(&mut self, moves: &[(Block, u16)]) {
        let mut deaths = Vec::new();
        let rocks: Vec<(Block, u16)> = moves.iter().copied().filter(|(_, m)| BLOCKS.get(*m).is_some_and(|d| d.rubble_of.is_some())).collect();
        if rocks.is_empty() {
            return;
        }
        let world = &self.world;
        let mut heads: Vec<(Block, &mut Client)> = self.clients.iter_mut().filter(|c| !c.creative && c.vitals.dead.is_none())
            .map(|c| (world.block_at(c.phys.pos + phys::up(world, c.phys.pos) * phys::EYE), c)).collect();
        for &(b, m) in &rocks {
            for (eye, c) in heads.iter_mut() {
                let eye = *eye;
                let name = if c.player.name.is_empty() { "?" } else { c.player.name.as_str() };
                if eye == b && c.vitals.hurt(ROCK_HIT, || format!("{name} was hit by falling {}", BLOCKS.get(m).map_or("rock", |d| d.name.as_str()))) {
                    deaths.extend(c.vitals.dead.clone());
                }
            }
        }
        for line in deaths {
            self.say(line);
        }
    }

    /// What a survival dig took falls to the ground (ADR 0047, 0074): as few piles as hold it, in the dug cells
    /// nearest the digger, each down to the first floor under it.
    fn drop_dug(&mut self, i: usize, cells: &[(Block, u16)], dug: Vec<u16>) {
        let mut counts: BTreeMap<u16, u32> = BTreeMap::new();
        for m in dug {
            *counts.entry(m).or_default() += 1;
            *self.item_made.entry(m).or_default() += 1;
        }
        let stacks: Vec<Stack> = counts.into_iter().flat_map(|(m, n)| items::stacks(m, n)).collect();
        let feet = self.clients[i].phys.pos;
        let mut near: Vec<(f64, Block)> = cells.iter().map(|(b, _)| (self.world.block_centre(*b).distance_squared(feet), *b)).collect();
        near.sort_by(|a, b| a.0.total_cmp(&b.0).then(key(a.1).cmp(&key(b.1))));
        for (part, (_, b)) in stacks.chunks(items::PILE_SLOTS).zip(near) {
            self.lay(b, part.to_vec(), (0, 0));
        }
    }

    /// Lays `stacks` in cell `at` (ADR 0074): out of a solid block upwards, down to the first floor, into that
    /// cell's pile and, while it has no room, the piles over it. `wait` is the thrower and the tick it may take
    /// them back from.
    /// A hanging block (a torch, ADR 0117) knocked off its cell, already emptied or taken: one of it lies at `at`,
    /// counted as made so the ledger stays exact.
    pub(super) fn knock_off(&mut self, at: Block, m: u16) {
        *self.item_made.entry(m).or_default() += 1;
        self.lay(at, vec![Stack::new(m, 1)], (0, 0));
    }

    pub(super) fn lay(&mut self, mut at: Block, stacks: Vec<Stack>, wait: (u32, u64)) {
        if stacks.is_empty() {
            return;
        }
        let layers = self.world.bodies[at.body as usize].layers;
        let solid = |w: &World, b: Block| BLOCKS.solid(w.block(b));
        while at.pos.z + 1 < layers && solid(&self.world, at) {
            at.pos.z += 1;
        }
        // a fall of at most FALL layers: a throw or a death in space leaves its pile floating there, not a walk
        // down tens of thousands of layers on the tick thread
        let lowest = (at.pos.z - FALL).max(0);
        while at.pos.z > lowest && !solid(&self.world, Block { pos: at.pos - glam::IVec3::Z, ..at }) {
            at.pos.z -= 1;
        }
        for st in stacks {
            let (mut c, mut left) = (at, st);
            // shortcut: a column full of piles to the top of the world would lose the rest; nothing nears that
            while left.n > 0 && c.pos.z < layers {
                if !solid(&self.world, c) {
                    let p = self.piles.entry(key(c)).or_insert_with(|| Pile { slots: vec![None; items::PILE_SLOTS], wait: (0, 0) });
                    let n = items::add(&mut p.slots, 0..items::PILE_SLOTS, left);
                    if n < left.n && wait.0 != 0 {
                        p.wait = wait;
                    }
                    if p.slots.iter().all(Option::is_none) {
                        self.piles.remove(&key(c));
                    }
                    left.n = n;
                    self.piles_changed.push(c);
                }
                c.pos.z += 1;
            }
        }
    }

    /// The piles from cell `at` up its column, laid again: a dig under them lets them fall, a block placed in
    /// one lifts it.
    pub(super) fn resettle(&mut self, mut at: Block) {
        let mut column = Vec::new();
        while let Some(p) = self.piles.remove(&key(at)) {
            column.push((at, p));
            at.pos.z += 1;
        }
        for (b, p) in column {
            self.piles_changed.push(b);
            self.lay(b, p.slots.into_iter().flatten().collect(), p.wait);
        }
    }

    /// Every living player takes what its slots have room for from the piles within `PICK` of its feet,
    /// players in their order, piles in theirs (ADR 0074). shortcut: the cells around the feet are on the
    /// feet's face, so a pile just over a cube face's edge waits until the player steps across.
    pub(super) fn pick_up(&mut self) {
        let Server { clients, piles, piles_changed, world, tick, .. } = self;
        for c in clients.iter_mut().filter(|c| c.vitals.dead.is_none()) {
            let feet = c.phys.pos;
            let mid = world.block_at(feet + phys::up(world, feet) * 0.5);
            for (dz, dy, dx) in (-1..=1).flat_map(|z| (-1..=1).flat_map(move |y| (-1..=1).map(move |x| (z, y, x)))) {
                let b = Block { pos: mid.pos + glam::IVec3::new(dx, dy, dz), ..mid };
                let Some(p) = piles.get_mut(&key(b)) else { continue };
                if (p.wait.0 == c.player.id && *tick < p.wait.1) || world.block_centre(b).distance(feet) > PICK {
                    continue;
                }
                let mut took = false;
                for s in p.slots.iter_mut() {
                    if let Some(st) = *s {
                        let left = items::give(&mut c.slots, st);
                        took |= left < st.n;
                        *s = Some(Stack { n: left, ..st }).filter(|o| o.n > 0);
                    }
                }
                if took {
                    c.inv_dirty = true;
                    piles_changed.push(b);
                    if p.slots.iter().all(Option::is_none) {
                        piles.remove(&key(b));
                    }
                }
            }
        }
    }

    /// Client `i` throws `stacks` ahead of it (`THROW`), onto the ground there, or at its feet when a wall is
    /// in the way; it cannot take them back for `THROW_WAIT` ticks.
    fn throw(&mut self, i: usize, stacks: Vec<Stack>) {
        let c = &self.clients[i];
        let (feet, id) = (c.phys.pos, c.player.id);
        let up = phys::up(&self.world, feet);
        let ahead = (c.phys.fwd - up * c.phys.fwd.dot(up)).normalize_or_zero();
        let mid = feet + up * 0.5;
        let mut at = self.world.block_at(mid + ahead * THROW);
        if BLOCKS.solid(self.world.block(at)) {
            at = self.world.block_at(mid);
        }
        self.lay(at, stacks, (id, self.tick + THROW_WAIT));
    }

    /// Client `i` takes the pick-up lying in air cell `at` (`ToServer::Take`, ADR 0082): within `REACH`, one lies
    /// there and was not taken (or has come back), and all it gives fits in the slots; else nothing changes. With
    /// no pick-up there in any season it cuts the plant growing there (draft-plant-fibre, `clutter::thing_at`), at
    /// the most Vegetation any client draws: what it gives must fit, and the cell is written as air, an edit that
    /// hides the column's clutter for everyone and in the save. It is sent at once, not through the tick's
    /// `changed`: air stays air, so no water is regapped or woken and nothing falls.
    pub(super) fn take(&mut self, i: usize, at: Block) -> bool {
        use engine::clutter::{Kind, CLUTTER, VEGETATION};
        let c = &self.clients[i];
        let t = self.world.time;
        if c.vitals.dead.is_some() || !self.exists(at) || self.world.block_centre(at).distance(c.phys.pos) > REACH {
            return false;
        }
        let year = engine::clutter::year(t);
        let lies = |b: Block, th: &engine::clutter::Thing| engine::clutter::lies(&CLUTTER[th.row], self.taken.get(&key(b)).copied(), year);
        let Some(th) = engine::clutter::thing_at(&self.world, at, t, VEGETATION[VEGETATION.len() - 1], false, lies) else { return false };
        let row = &CLUTTER[th.row];
        // what it gives, an item that exists, or nothing is taken or cut (a plant is never cut for nothing)
        let Some((item, n)) = row.item() else { return false };
        let mut slots = c.slots.clone();
        if items::stacks(item, n as u32).any(|st| items::give(&mut slots, st) > 0) {
            return false;
        }
        let c = &mut self.clients[i];
        (c.slots, c.inv_dirty) = (slots, true);
        *self.item_made.entry(item).or_default() += n as i64;
        if row.kind == Kind::Plant {
            self.world.set_block(at, AIR);
            for c in &self.clients {
                c.link.send(ToClient::Edits(vec![(at, AIR)]));
            }
        } else {
            self.taken.insert(key(at), year);
            self.taken_changed.push((at, year));
        }
        true
    }

    /// Client `i` moves `n` of the stack in `from` to `to` (`ToServer::Items`, ADR 0074): all of it or nothing.
    /// A pile must lie within `PILE_REACH`; creative's grid is creative's; every holder keeps its rules after it.
    pub(super) fn items(&mut self, i: usize, from: (Holder, u16), to: (Holder, u16), n: u16) -> bool {
        let c = &self.clients[i];
        let (creative, feet) = (c.creative, c.phys.pos);
        let grid = from.0 == Holder::Grid || to.0 == Holder::Grid;
        let reach = |h: Holder| match h {
            Holder::Pile(b) => self.piles.contains_key(&key(b)) && self.world.block_centre(b).distance(feet) <= PILE_REACH,
            Holder::Furnace(b) => self.reaches_furnace(b, feet),
            _ => true,
        };
        if c.vitals.dead.is_some() || (grid && !creative) || from.0 == Holder::Ground || !reach(from.0) || !reach(to.0) {
            return false;
        }
        // the grid's cells outside the 2 x 2 take something only at a workbench (ADR 0140)
        let bench_cell = |(h, k): (Holder, u16)| h == Holder::Ape && (k as usize) >= items::CRAFT_AT && !items::in_two(k as usize - items::CRAFT_AT);
        if (bench_cell(to) || bench_cell(from)) && !self.at_bench(i) {
            return false;
        }
        let slots_of = |s: &Server, h: Holder| match h {
            Holder::Ape => Some(s.clients[i].slots.clone()),
            Holder::Pile(b) => s.piles.get(&key(b)).map(|p| p.slots.clone()),
            Holder::Furnace(b) => Some(s.furnace_slots(b)),
            _ => None,
        };
        let (fi, ti) = (from.1 as usize, to.1 as usize);
        // the holders as they will be (`b` only when it is another holder than `a`), what creative made and
        // what is thrown
        let (mut a, mut b): (Option<Vec<Slot>>, Option<Vec<Slot>>) = (slots_of(self, from.0), None);
        let mut made: Vec<(u16, i64)> = Vec::new();
        let mut thrown = None;
        match (from.0, to.0) {
            (Holder::Grid, _) if !ITEMS.known(from.1) || n == 0 || n > ITEMS.stack(from.1) => return false,
            (Holder::Grid, Holder::Grid) => return false,
            (Holder::Grid, Holder::Ground) => {
                thrown = Some(Stack::new(from.1, n));
                made.push((from.1, n as i64));
            }
            (Holder::Grid, _) => {
                let Some(mut t) = slots_of(self, to.0) else { return false };
                let Some(slot) = t.get_mut(ti) else { return false };
                // creative's endless stack; whatever else lay in that slot is gone
                if let Some(old) = slot.filter(|o| o.item != from.1) {
                    made.push((old.item, -(old.n as i64)));
                    *slot = None;
                }
                let before = slot.map_or(0, |o| o.n);
                if !items::rule(&mut Some(Stack::new(from.1, ITEMS.stack(from.1))), slot, n) {
                    return false;
                }
                made.push((from.1, (slot.unwrap().n - before) as i64));
                b = Some(t);
            }
            (_, Holder::Grid | Holder::Ground) => {
                let Some(s) = a.as_mut().and_then(|a| a.get_mut(fi)) else { return false };
                let Some(st) = s.filter(|st| n > 0 && n <= st.n) else { return false };
                *s = Some(Stack { n: st.n - n, ..st }).filter(|o| o.n > 0);
                if to.0 == Holder::Grid {
                    made.push((st.item, -(n as i64)));
                } else {
                    thrown = Some(Stack { n, ..st });
                }
            }
            (f, t) if f == t => {
                if !items::shift(a.as_mut().unwrap(), fi, None, ti, n) {
                    return false;
                }
            }
            (_, t) => {
                let Some(mut tb) = slots_of(self, t) else { return false };
                if !items::shift(a.as_mut().unwrap(), fi, Some(&mut tb), ti, n) {
                    return false;
                }
                b = Some(tb);
            }
        }
        // a furnace keeps its slots' rules too: fuel, mould, nothing put into what comes out
        let fits = |h: Holder, s: &Option<Vec<Slot>>| match h {
            Holder::Ape => s.as_ref().is_none_or(|s| items::ape_fits(s)),
            Holder::Furnace(fb) => s.as_ref().is_none_or(|s| engine::furnace::fits(s, &self.furnace_slots(fb), &engine::recipes::RECIPES)),
            _ => true,
        };
        if !fits(from.0, &a) || !fits(to.0, &b) {
            return false;
        }
        for (k, d) in made {
            *self.item_made.entry(k).or_default() += d;
        }
        for (h, s) in [(from.0, a), (to.0, b)] {
            let Some(s) = s else { continue };
            match h {
                Holder::Ape => self.clients[i].slots = s,
                Holder::Pile(c) => {
                    if s.iter().all(Option::is_none) {
                        self.piles.remove(&key(c));
                    } else {
                        self.piles.get_mut(&key(c)).unwrap().slots = s;
                    }
                    self.piles_changed.push(c);
                }
                Holder::Furnace(fb) => self.furnaces.entry(key(fb)).or_default().slots = s,
                _ => {}
            }
        }
        if let Some(t) = thrown {
            self.throw(i, vec![t]);
        }
        true
    }
}
