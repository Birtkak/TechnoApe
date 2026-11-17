//! Server: one tick of the world, and a player's health and hunger after its physics step.
use super::*;

impl Server {
    pub fn tick(&mut self) {
        if self.live_data && self.tick % TICK_HZ as u64 == 0 {
            self.watch_defs();
        }
        // the sky's clock: the world's age, which a save carries on (ADR 0022)
        self.tick0 = self.tick0.wrapping_add(self.warp - 1);
        self.world.time = self.tick0.wrapping_add(self.tick) as f64 / TICK_HZ;
        if let Some(w) = &mut self.weather {
            // the air's runoff into the rivers (W5, ADR 0059), already out of the air's sea term
            for (reach, units) in w.tick(self.tick, self.world.time) {
                self.world.water.rain(reach, units);
            }
            self.send_sky();
        }
        if self.tick == 0 {
            let [b0, b1] = [&self.world.bodies[0], &self.world.bodies[1]];
            let start = Ev::Start { planet_n: b0.n, moon_n: b1.n, seed: self.seed, dt: self.dt, map: self.map_hash() };
            self.log(start);
            if let Some(save) = self.loaded.take() {
                self.log(Ev::Loaded(save));
            }
            if let Some(b) = self.herds_loaded.take() {
                self.log(Ev::Herds(b));
            }
        }
        // the player list changed this tick (a join, a name, a leave): it is sent only then
        let mut roster = false;
        while let Ok(link) = self.accept.try_recv() {
            roster = true;
            let id = self.next_id;
            self.next_id += 1;
            self.log(Ev::Join { tick: self.tick, id });
            let [p, m] = [self.world.bodies[0].n, self.world.bodies[1].n];
            if let Some((blocks, items)) = engine::live::reloaded() {
                link.send(ToClient::Defs { blocks, items });
            }
            link.send(ToClient::Welcome { id, seed: self.seed, planet_n: p, moon_n: m, blocks: BLOCKS.hash, items: ITEMS.hash, map: self.map_hash(), luna: self.luna_hash() });
            link.send(ToClient::Kinds(self.herds.kinds.iter().map(|k| k.name.clone()).collect()));
            let all: Vec<(Block, u16)> = self
                .world
                .edits
                .iter()
                .flat_map(|(k, cells)| cells.iter().map(move |(pos, m)| (Block { body: k.body, face: k.face, pos: *pos }, *m)))
                .collect();
            for part in all.chunks(EDITS_PART) {
                link.send(ToClient::Edits(part.to_vec()));
            }
            if !self.sky.is_empty() {
                link.send(ToClient::Sky { t: self.world.time, all: self.sky.clone(), changed: Vec::new() });
            }
            let piles: Vec<(Block, Vec<Slot>)> = self.piles.iter().map(|(k, p)| (unkey(*k), p.slots.clone())).collect();
            for part in piles.chunks(EDITS_PART) {
                link.send(ToClient::Piles(part.to_vec()));
            }
            let taken: Vec<(Block, u32)> = self.taken.iter().map(|(k, y)| (unkey(*k), *y)).collect();
            for part in taken.chunks(EDITS_PART) {
                link.send(ToClient::Taken(part.to_vec()));
            }
            let fires = self.fires();
            if !fires.is_empty() {
                link.send(ToClient::Fires(fires));
            }
            let spawn = self.spawn();
            self.clients.push(Client {
                link,
                player: Player { id, name: String::new(), pos: spawn.to_array(), fwd: [1.0, 0.0, 0.0], pitch: 0.0, hand: 0 },
                phys: phys::Player::at(spawn, DVec3::X),
                input: None,
                queue: VecDeque::new(),
                acked: 0,
                credit: 0,
                creative: false,
                slots: items::ape(),
                water: 0,
                tiles: FxMap::default(),
                done: Vec::new(),
                inv_dirty: true,
                vitals: Vitals::default(),
                breath: vitals::Breath::default(),
                marks: save::Marks::default(),
                sent_marks: (None, None),
                was: DVec3::ZERO,
                sent: None,
                drink: None,
                hold: edit::Hold::default(),
                animals: FxMap::default(),
                hand: 0,
                sent_hand: AIR,
                knap: (0, 0, 0),
                prospected: None,
                journal: engine::journal::JOURNAL.fresh(),
                told: None,
                kept: Vec::new(),
                bench: None,
                furnace: None,
                sent_furnace: None,
            });
        }
        let mut changed: Vec<(Block, u16)> = Vec::new();
        let mut deaths = Vec::new();
        let mut i = 0;
        while i < self.clients.len() {
            let mut open = true;
            // the recorder logs what is handled, at the tick it is handled, so a replay meets the same cap
            for _ in 0..MSGS_PER_TICK {
                let r = self.clients[i].link.try_recv();
                let (tick, id) = (self.tick, self.clients[i].player.id);
                match &r {
                    Recv::Msg(m) => self.log(Ev::Msg { tick, id, m: m.clone() }),
                    Recv::Closed => self.log(Ev::Leave { tick, id }),
                    Recv::Empty => {}
                }
                // the sprint bit's messages (ADR 0118) as the steps and inputs they carry
                let r = match r {
                    Recv::Msg(m) => Recv::Msg(m.unpacked()),
                    r => r,
                };
                match r {
                    // a client names itself once: a second Hello would take another name's saved state
                    Recv::Msg(ToServer::Hello { .. }) if !self.clients[i].player.name.is_empty() => {}
                    Recv::Msg(ToServer::Hello { name }) => {
                        let base: String = name.chars().filter(|c| !c.is_control()).take(32).collect();
                        // a name already online gets a suffix, so two of one name are two saved players
                        let online = |n: &str| self.clients.iter().any(|c| c.player.name == n);
                        let mut name = base.clone();
                        for k in 2.. {
                            if name.is_empty() || !online(&name) {
                                break;
                            }
                            name = format!("{base}-{k}");
                        }
                        let c = &mut self.clients[i];
                        c.player.name = name;
                        roster = true;
                        // a player the save remembers carries on where it left, with what it held
                        let mut before = Vec::new();
                        if let Some(sv) = self.absent.remove(&c.player.name) {
                            // what it got before its name (a /give, a pick-up) is laid where it comes back
                            before = c.slots.iter_mut().filter_map(Option::take).collect();
                            let p = DVec3::from_array(sv.pos);
                            c.phys = phys::Player::at(p, phys::tangent(DVec3::from_array(sv.fwd.map(f64::from)), phys::up(&self.world, p), DVec3::X));
                            (c.player.pos, c.player.fwd, c.creative, c.inv_dirty, c.water, c.vitals, c.slots, c.marks) = (sv.pos, sv.fwd, sv.creative, true, sv.water, sv.vitals, sv.slots, sv.marks);
                            (c.journal, c.kept) = engine::journal::JOURNAL.load(&sv.journal);
                        }
                        let feet = self.clients[i].phys.pos;
                        self.lay(self.world.block_at(feet + phys::up(&self.world, feet) * 0.5), before, (0, 0));
                        // a grid a save kept full (it was saved while the screen was open) goes into the pack
                        self.uncraft(i);
                        let line = format!("{} joined", self.clients[i].player.name);
                        self.say(line);
                    }
                    // a dead player stays where it fell (ADR 0040)
                    Recv::Msg(ToServer::Move { .. }) if self.clients[i].vitals.dead.is_some() => {}
                    Recv::Msg(ToServer::Move { pos, fwd, pitch }) => {
                        if pos.iter().all(|c| c.is_finite()) && fwd.iter().all(|c| c.is_finite()) {
                            self.put(i, DVec3::from_array(pos), DVec3::from_array(fwd.map(f64::from)));
                            self.clients[i].player.pitch = pitch;
                        }
                    }
                    Recv::Msg(ToServer::Input(inp)) => {
                        if phys::sane(&inp) {
                            self.clients[i].input = Some(inp);
                        }
                    }
                    Recv::Msg(ToServer::Steps { first, inputs }) => {
                        let c = &mut self.clients[i];
                        c.input = None;
                        for (k, inp) in inputs.into_iter().enumerate() {
                            // in order and new only; a client more than a second ahead loses the rest
                            let seq = first.wrapping_add(k as u32);
                            if seq > c.queue.back().map_or(c.acked, |q| q.0) && c.queue.len() < QUEUE_MAX && phys::sane(&inp) {
                                c.queue.push_back((seq, inp));
                            }
                        }
                    }
                    Recv::Msg(ToServer::Edit { seq, shape, material }) => {
                        // the steps the client took before this edit came before it: they are taken first, so the
                        // edit falls between the same two steps here as in its prediction (ADR 0015, 0019). Taken
                        // ahead of the credit, which goes below 0 and holds the next steps back as long, down to a
                        // floor of a full queue: a client in step with real time stays far above it, a flood of steps and edits stops
                        // there (at most QUEUE_MAX + CREDIT_MAX steps a tick) and its edits fall ahead of the rest.
                        let (c, warp) = (&mut self.clients[i], self.warp);
                        if c.input.is_none() && !c.queue.is_empty() {
                            let dead = c.vitals.dead.is_some();
                            while c.credit > -(QUEUE_MAX as i32) && queued_step(c, &mut self.world, warp, dead, &mut deaths) {
                                c.credit -= 1;
                            }
                            c.player.pos = c.phys.pos.to_array();
                            c.player.fwd = c.phys.fwd.as_vec3().to_array();
                        }
                        let ok = self.edit(i, &shape, material, &mut changed);
                        self.clients[i].done.push((seq, ok));
                        // the dig is done: its hold ends, and the others' cracks with it
                        if ok && self.clients[i].hold.shape() == Some(shape) {
                            self.clients[i].hold.done();
                            self.tell_dig(i, None);
                        }
                    }
                    Recv::Msg(ToServer::Items { seq, from, to, n }) => {
                        let ok = self.items(i, from, to, n);
                        // the slots go back either way: they put right a prediction the server refused
                        self.clients[i].done.push((seq, ok));
                        self.clients[i].inv_dirty = true;
                    }
                    Recv::Msg(ToServer::Take { seq, at }) => {
                        let ok = self.take(i, at);
                        self.clients[i].done.push((seq, ok));
                    }
                    Recv::Msg(ToServer::Bucket { seq, at, fill }) => {
                        let ok = self.bucket(i, at, fill);
                        self.clients[i].done.push((seq, ok));
                    }
                    Recv::Msg(ToServer::Creative(on)) => {
                        let c = &mut self.clients[i];
                        (c.creative, c.inv_dirty) = (on, true);
                    }
                    Recv::Msg(ToServer::Chat(text)) => self.chat(i, &text, &mut changed),
                    Recv::Msg(ToServer::Respawn) => {
                        if self.clients[i].vitals.dead.is_some() {
                            // what it carried stays where it died, marked until the pile is empty (ADR 0047, 0074,
                            // needs.md); creative loses nothing because it cannot die
                            let feet = self.clients[i].phys.pos;
                            let left: Vec<Stack> = self.clients[i].slots.iter_mut().filter_map(Option::take).collect();
                            // (with nothing carried the last death's mark stays)
                            let laid = self.lay(self.world.block_at(feet + phys::up(&self.world, feet) * 0.5), left, (0, 0));
                            if let Some(&at) = laid.first() {
                                self.events.push(Event::Laid { at, by: self.clients[i].player.id });
                            }
                            if !laid.is_empty() {
                                self.clients[i].marks.death = laid;
                            }
                            self.clients[i].inv_dirty = true;
                            // it wakes at its sleeping place while it is one and the ape fits on it, else at the
                            // spawn, and forgets it
                            let home = self.clients[i].marks.home.filter(|b| vitals::sleeping_place(&self.world, *b));
                            self.clients[i].marks.home = home;
                            let p = home.map_or_else(|| self.spawn(), |b| vitals::wake_at(&self.world, b));
                            self.put(i, p, DVec3::X);
                            self.clients[i].vitals = Vitals::default();
                            (self.clients[i].breath, self.clients[i].drink) = (vitals::Breath::default(), None);
                        }
                    }
                    Recv::Msg(ToServer::Dig(at)) => self.dig(i, at),
                    Recv::Msg(ToServer::Drink(at)) => self.hold_drink(i, at),
                    Recv::Msg(ToServer::Hand(k)) => self.clients[i].hand = k.min(items::HOTBAR as u8 - 1),
                    Recv::Msg(ToServer::Strike(at)) => self.strike(i, at),
                    Recv::Msg(ToServer::Craft { recipe, n }) => self.craft(i, &recipe, n),
                    Recv::Msg(ToServer::Use(at)) => self.use_block(i, at),
                    Recv::Msg(ToServer::Close) => self.uncraft(i),
                    Recv::Msg(ToServer::Choose { at, cast }) => self.choose(i, at, cast),
                    Recv::Msg(ToServer::Sprints { .. } | ToServer::Sprint(..)) => unreachable!("unpacked above"),
                    Recv::Msg(ToServer::Map(h)) => {
                        if let Some(m) = self.world.bodies.iter().filter_map(|b| b.map()).find(|m| m.hash == h) {
                            // shortcut: any client may ask again; a limit per client when servers go public
                            for p in self.map_pieces.entry(h).or_insert_with(|| engine::net::map_pieces(m)) {
                                self.clients[i].link.send(p.clone());
                            }
                        }
                    }
                    Recv::Empty => break,
                    Recv::Closed => {
                        open = false;
                        break;
                    }
                }
            }
            if open {
                i += 1;
            } else {
                self.tell_dig(i, None);
                roster = true;
                self.uncraft(i);
                let c = self.clients.swap_remove(i);
                if c.player.name.is_empty() {
                    // no name, no save to keep its things in: they stay where it stood
                    let feet = c.phys.pos;
                    self.lay(self.world.block_at(feet + phys::up(&self.world, feet) * 0.5), c.slots.into_iter().flatten().collect(), (0, 0));
                } else {
                    self.absent.insert(c.player.name.clone(), saved(&c));
                    self.say(format!("{} left", c.player.name));
                }
            }
        }
        self.benches();
        self.debt += self.dt;
        let steps = ((self.debt + 1e-9) / phys::DT) as u32;
        self.debt -= steps as f64 * phys::DT;
        for c in &mut self.clients {
            // flight is creative's (ADR 0023); a dead player's input moves nothing (ADR 0040)
            let (creative, dead) = (c.creative, c.vitals.dead.is_some());
            let walk = move |inp: Input| {
                let inp = if creative { inp } else { Input { fly: 0.0, ..inp } };
                if dead { vitals::still(inp) } else { inp }
            };
            if let Some(inp) = c.input.map(walk) {
                for _ in 0..steps {
                    // (a player that died in an earlier step of this tick moves no more in it)
                    let inp = if c.vitals.dead.is_some() { vitals::still(inp) } else { inp };
                    // a sprint takes hunger over the floor, at the hunger of this step (ADR 0118)
                    let inp = vitals::gate(inp, c.vitals.hunger, c.creative);
                    let before = c.phys;
                    let sprinted = phys::step(&self.world, &mut c.phys, &inp, phys::DT);
                    live(c, &self.world, &before, &inp, sprinted, &mut deaths);
                }
                c.player.pos = c.phys.pos.to_array();
                c.player.fwd = c.phys.fwd.as_vec3().to_array();
                c.player.pitch = inp.pitch;
            } else {
                // each step once, at most as fast as real time; a late batch catches up within CREDIT_MAX
                c.credit = (c.credit + steps as i32).min(CREDIT_MAX);
                while c.credit > 0 && queued_step(c, &mut self.world, self.warp, dead, &mut deaths) {
                    c.credit -= 1;
                }
                c.player.pos = c.phys.pos.to_array();
                c.player.fwd = c.phys.fwd.as_vec3().to_array();
            }
            c.link.send(ToClient::You { step: c.acked, me: c.phys, time: self.world.time, warp: self.warp as f64, hunger: c.vitals.hunger });
        }
        // a death is an event (`Died`), and so is the pile its respawn lays (`Laid`, the Respawn arm)
        // shortcut: neither has an effect or a sound yet; one each when the developer asks for them
        for line in deaths {
            self.died(&line);
            self.say(line);
        }
        // loose blocks wake where this tick's edits changed the world, then fall and slide (ADR 0079), through
        // a player too: `live` takes the breath of one buried to the eyes
        for (b, _) in &changed {
            self.falls.wake(&self.world, *b);
        }
        let mine = changed.len();
        if self.falls.awake() > 0 {
            self.falls.step(&mut self.world, self.tick, &mut changed);
            self.rock_hits(&changed[mine..]);
        }
        // each move is two edits, the cell left then the cell come to (`Falls::step`). shortcut: a move across a
        // face's edge is the cell's dig and place, as the client drew it before the list; a fall when noticed
        for p in changed[mine..].chunks_exact(2) {
            let ((from, _), (to, block)) = (p[0], p[1]);
            self.events.extend(if (from.body, from.face) == (to.body, to.face) {
                vec![Event::Fall { from, to, block }]
            } else {
                vec![Event::Edit { at: from, was: block, now: AIR, by: None }, Event::Edit { at: to, was: AIR, now: block, by: None }]
            });
        }
        // a torch a falling block took the cell of was knocked off first (ADR 0117): it drops as itself where the
        // block fell from
        for (from, m) in std::mem::take(&mut self.falls.knocked) {
            self.knock_off(from, m);
        }
        // a hanging block (a torch, ADR 0117) that this tick's edits left with no floor or wall drops as itself
        for b in engine::fall::unheld(&self.world, &changed) {
            let m = self.world.block(b);
            self.world.set_block(b, AIR);
            changed.push((b, AIR));
            self.events.push(Event::Edit { at: b, was: m, now: AIR, by: None });
            self.knock_off(b, m);
        }
        // the animals (ADR 0083): woken by the players on Terra, stepped as far as the players were
        let players: Vec<(DVec3, DVec3)> = self.clients.iter().filter(|c| self.world.frame_at(c.phys.pos) == 0).map(|c| (c.phys.pos, c.phys.vel)).collect();
        let t = self.herds.clock;
        self.herds.tick(&self.world, &players, t, steps as f64 * phys::DT);
        self.send_animals(t);
        // water finds the new gaps of every column an edit changed, then flows: as many steps as the physics
        // took, fewer when that would spend more than WATER_BUDGET
        let mut cols: Vec<Block> = changed.iter().map(|(b, _)| Block { pos: glam::IVec3::new(b.pos.x, b.pos.y, 0), ..*b }).collect();
        cols.sort_unstable_by_key(|b| (b.body, b.face, b.pos.x, b.pos.y));
        cols.dedup();
        for b in cols {
            self.world.regap(b);
        }
        // piles over a dug cell fall, a pile in a placed one rises (ADR 0074); then every player picks up
        for (b, m) in &changed {
            self.resettle(Block { pos: b.pos + glam::IVec3::Z * !BLOCKS.solid(*m) as i32, ..*b });
        }
        // the furnaces burn and work; one dug lays what it held (ADR 0141)
        self.furnaces();
        self.pick_up();
        let can = (WATER_BUDGET / self.world.water.awake().max(1)) as u32;
        let wsteps = if can == 0 && self.tick % 4 == 0 { 1 } else { steps.min(can) };
        self.water_skipped += (steps - wsteps) as u64;
        for _ in 0..wsteps {
            self.world.step_water();
        }
        // what happened, before the edits that show it
        let events = std::mem::take(&mut self.events);
        self.send_events(&events);
        // the falls' moves in messages of their own, so no move's two edits are split between two
        for part in changed[..mine].chunks(EDITS_PART).chain(changed[mine..].chunks(EDITS_PART)) {
            for c in &self.clients {
                c.link.send(ToClient::Edits(part.to_vec()));
            }
        }
        let cols: Vec<Col> = self.world.water.take_changes().into_iter().map(|w| w.0).collect();
        // a flame the water reached (a torch, ADR 0117) pops off as itself, in a message of its own
        // shortcut: every placed model is looked at when any water moved, and once a second; by the changed
        // columns if torches are many
        if !cols.is_empty() || self.tick % TICK_HZ as u64 == 0 {
            let wet: Vec<(Block, u16)> = self.world.models.iter().filter(|(b, id)| BLOCKS.get(**id).is_some_and(|d| d.flame) && self.world.water_surface(self.world.block_centre(**b)).is_some()).map(|(b, id)| (*b, *id)).collect();
            for &(b, m) in &wet {
                self.world.set_block(b, AIR);
                self.knock_off(b, m);
            }
            if !wet.is_empty() {
                self.send_events(&wet.iter().map(|&(at, was)| Event::Edit { at, was, now: AIR, by: None }).collect::<Vec<_>>());
                let gone: Vec<(Block, u16)> = wet.iter().map(|w| (w.0, AIR)).collect();
                for c in &self.clients {
                    c.link.send(ToClient::Edits(gone.clone()));
                }
            }
        }
        self.water_changed(&cols);
        self.send_water();
        if !self.taken_changed.is_empty() {
            let taken = std::mem::take(&mut self.taken_changed);
            for c in &self.clients {
                c.link.send(ToClient::Taken(taken.clone()));
            }
        }
        if !self.piles_changed.is_empty() {
            let mut cells = std::mem::take(&mut self.piles_changed);
            cells.sort_unstable_by_key(|b| key(*b));
            cells.dedup();
            let piles: Vec<(Block, Vec<Slot>)> = cells.into_iter().map(|b| (b, self.piles.get(&key(b)).map_or(Vec::new(), |p| p.slots.clone()))).collect();
            for c in &self.clients {
                c.link.send(ToClient::Piles(piles.clone()));
            }
        }
        self.learn();
        for c in &mut self.clients {
            for (seq, ok) in c.done.drain(..) {
                c.link.send(ToClient::Done { seq, ok });
            }
            if std::mem::take(&mut c.inv_dirty) {
                c.link.send(ToClient::Inventory { creative: c.creative, slots: c.slots.clone(), water: c.water });
            }
            if let Some(w) = self.weather.as_ref().filter(|_| self.tick % engine::net::AIR_EVERY == 0) {
                let b = &self.world.bodies[0];
                let rel = c.phys.pos - b.centre;
                if self.world.frame_at(c.phys.pos) == 0 && rel.length() > 0.0 {
                    let d = rel.normalize();
                    let h = w.here(d, b.over_sea(c.phys.pos).unwrap_or(0.0));
                    c.link.send(ToClient::Air(self.force.map_or(h, |f| f.here(h, w.view().theta[w.column(d)]))));
                }
            }
            let shown = (c.vitals.shown(), c.breath.shown(), c.vitals.dead.is_some());
            if c.sent != Some(shown) {
                c.sent = Some(shown);
                c.link.send(ToClient::Vitals { health: shown.0 .0, hunger: shown.0 .1, thirst: shown.0 .2, breath: shown.1, dead: c.vitals.dead.clone() });
            }
            // the death mark goes with its piles (needs.md): a cell emptied leaves it at once, so a pile laid there
            // later does not keep it; `resettle` moves the cells with the piles
            c.marks.death.retain(|d| self.piles.contains_key(&key(*d)));
            let now = (c.marks.death.first().copied(), c.marks.home);
            if c.sent_marks != now {
                c.sent_marks = now;
                c.link.send(ToClient::Marks { death: now.0, home: now.1 });
            }
            // the journal once it has its name (a save's comes with it): all of it first, then what changed
            if !c.player.name.is_empty() && c.told.as_ref() != Some(&c.journal) {
                let was = c.told.replace(c.journal.clone());
                let changed = engine::journal::JOURNAL.entries.iter().zip(&c.journal).enumerate().filter(|(k, (_, s))| was.as_ref().map_or(**s != engine::journal::State::Unknown, |w| w[*k] != **s));
                c.link.send(ToClient::Journal(changed.map(|(_, (e, s))| (e.name.clone(), *s)).collect()));
            }
        }
        // clients count the list; each one's own position comes in its `You` every tick. The others' come
        // with it once a second too, for the globe map (ADR 0077)
        // (with what each holds, ADR 0117: another's held torch lights the world for everyone; a change of it is
        // sent at once)
        let holds = |c: &Client| c.slots.get(c.hand as usize).copied().flatten().map_or(AIR, |s| s.item);
        if roster || self.tick % TICK_HZ as u64 == 0 || self.clients.iter().any(|c| c.sent_hand != holds(c)) {
            for c in &mut self.clients {
                c.sent_hand = holds(c);
            }
            let players: Vec<Player> = self.clients.iter().map(|c| Player { hand: c.sent_hand, ..c.player.clone() }).collect();
            for c in &self.clients {
                c.link.send(ToClient::Players(players.clone()));
            }
        }
        self.tick += 1;
        if let Some((path, every)) = &self.autosave {
            if self.tick % every == 0 {
                // the snapshot is taken here, between ticks; serialising and writing are a thread's
                let (s, path, herds) = (self.snapshot(), path.clone(), self.herds.to_bytes());
                if let Some(h) = self.saving.take() {
                    let _ = h.join();
                }
                self.saving = Some(std::thread::spawn(move || {
                    if let Err(e) = s.write(&path).and_then(|_| Server::write_herds(&herds, &path)) {
                        eprintln!("[technoape-server] save failed: {e}");
                    }
                }));
            }
        }
        if self.rec.is_some() {
            if self.tick % CHECK_EVERY == 0 {
                let ev = Ev::Check { tick: self.tick, hash: self.hash() };
                self.log(ev);
            }
            // every tick reaches the file, so a crash (panic = abort) loses at most the tick it happened in
            if let Err(e) = self.rec.as_mut().unwrap().flush() {
                eprintln!("[technoape-server] recording stopped: {e}");
                self.rec = None;
            }
        }
    }

    /// The journal's triggers (draft ADR journal-data), once a tick after the events: `have:` from the slots
    /// when they changed, `look:` from the block the player's look meets every `LOOK_EVERY` ticks. Not for a
    /// creative player (its slots are a builder's), a dead one, or one without its name yet.
    pub(super) fn learn(&mut self) {
        let j = &*engine::journal::JOURNAL;
        let look = self.tick % LOOK_EVERY == 0;
        let world = &self.world;
        for c in self.clients.iter_mut().filter(|c| !c.creative && c.vitals.dead.is_none() && !c.player.name.is_empty()) {
            // and on the tick its name came (`told` none yet): what it held before is no change of this tick
            if c.inv_dirty || c.told.is_none() {
                for s in c.slots.iter().flatten() {
                    j.had(&mut c.journal, s.item);
                }
            }
            if look {
                let p = &c.phys;
                let up = p.body_up(phys::up(world, p.pos));
                let pitch = c.player.pitch as f64;
                let dir = (p.fwd * pitch.cos() + up * pitch.sin()).normalize_or_zero();
                if let Some((hit, _)) = edit::aim(world, p.pos + up * phys::EYE, dir, LOOK_REACH) {
                    j.looked(&mut c.journal, world.block(hit));
                }
            }
        }
    }

    /// Each client is sent, in their order and in parts of at most `EDITS_PART`, the events it can perceive
    /// (ADR 0149): every `Edit` and `Fall`, as their edits go to every client (the effects of a cave-in
    /// 300 m off played before the list, and play still), and of the rest those within `events::PERCEIVE` of
    /// it (where a player's own happened, where that player is) and every one of its own.
    pub(super) fn send_events(&self, events: &[Event]) {
        if events.is_empty() {
            return;
        }
        for c in &self.clients {
            let near = |p: DVec3| p.distance(c.phys.pos) <= engine::events::PERCEIVE;
            let sees = |e: &Event| match e {
                Event::Edit { .. } | Event::Fall { .. } => true,
                _ if e.by() == Some(c.player.id) => true,
                _ => match e.cell() {
                    Some(b) => near(self.world.block_centre(b)),
                    None => e.by().and_then(|id| self.clients.iter().find(|o| o.player.id == id)).is_some_and(|o| near(o.phys.pos)),
                },
            };
            let v: Vec<Event> = events.iter().filter(|e| sees(e)).cloned().collect();
            for part in v.chunks(EDITS_PART) {
                c.link.send(ToClient::Events(part.to_vec()));
            }
        }
    }
}

/// After one physics step of client `c` from `before` with `inp` (ADR 0040): the impact it took, if it hit
/// something solid hard enough, and a step of hunger. Creative has neither: nothing drains, nothing hurts.
/// A death's line for everyone goes to `deaths`.
/// Takes client `c`'s next queued step, if any (`dead`: as it was when the tick's steps began).
fn queued_step(c: &mut Client, world: &mut World, warp: u64, dead: bool, deaths: &mut Vec<String>) -> bool {
    let Some((seq, inp)) = c.queue.pop_front() else { return false };
    let inp = if c.creative { inp } else { Input { fly: 0.0, ..inp } };
    let inp = if dead { vitals::still(inp) } else { inp };
    let mut inp = vitals::gate(inp, c.vitals.hunger, c.creative);
    // at the client's time when it is near this clock (it predicted the step then), else at this one
    let near = 2.0 * warp as f64; // two seconds of real time
    if (inp.t - world.time).abs() > near {
        inp.t = 0.0;
    }
    let before = c.phys;
    let sprinted = phys::step_at(world, &mut c.phys, &inp, phys::DT);
    live(c, world, &before, &inp, sprinted, deaths);
    (c.acked, c.player.pitch) = (seq, inp.pitch);
    true
}

fn live(c: &mut Client, world: &World, before: &phys::Player, inp: &Input, sprinted: bool, deaths: &mut Vec<String>) {
    let (now, hit) = vitals::impact(c.was, before, &c.phys, phys::DT);
    c.was = now;
    if c.creative {
        c.breath = vitals::Breath::default();
        return;
    }
    let name = if c.player.name.is_empty() { "?" } else { c.player.name.as_str() };
    let up = phys::up(world, c.phys.pos);
    // what the head cannot breathe in (draft breath): sand to the eyes, water over them, or no air above
    let eye = c.phys.pos + up * phys::EYE;
    let under = if BLOCKS.solid(world.block(world.block_at(eye))) {
        Some(vitals::Choke::Buried)
    } else if world.water_surface(eye).is_some() {
        Some(vitals::Choke::Water)
    } else if world.over_air(eye) > engine::world::AIR_TOP_M {
        Some(vitals::Choke::Space)
    } else {
        None
    };
    if c.vitals.smother(&mut c.breath, under, name) {
        deaths.extend(c.vitals.dead.clone());
    }
    if hit > vitals::safe_speed() {
        // landing in water costs nothing (asked only for a hit that would hurt)
        if world.water_surface(c.phys.pos + up * 0.05).is_none() {
            let into = before.vel.dot(up) / before.vel.length().max(1e-9);
            let what = if into < -0.7 { "the ground" } else if into > 0.7 { "a ceiling" } else { "a wall" };
            if c.vitals.hurt(vitals::impact_damage(hit), || format!("{name} hit {what} at {hit:.0} m/s")) {
                deaths.extend(c.vitals.dead.clone());
            }
        }
    }
    // (a sprint's drain only for a step that sprinted: not into a wall, not in the water; ADR 0118)
    c.vitals.drain(vitals::rate(inp, sprinted));
    if c.vitals.parch(vitals::rate(inp, sprinted), || format!("{name} died of thirst")) {
        deaths.extend(c.vitals.dead.clone());
    }
    // the use button held with a truly empty hand on fresh water within reach (ADR 0145)
    // shortcut: a mouthful takes no water from the world; the ledger counts it once the water cycle gives it back
    // (death lets go, whatever killed: a command, an impact, thirst)
    if c.vitals.dead.is_some() {
        c.drink = None;
    }
    let empty = c.slots.get(c.hand as usize).copied().flatten().is_none();
    if c.drink.is_some_and(|(b, fresh)| fresh && empty && world.block_centre(b).distance(c.phys.pos) <= items::BENCH_REACH) {
        c.vitals.drink();
    }
}
