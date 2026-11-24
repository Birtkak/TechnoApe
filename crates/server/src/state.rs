//! Server: making one, from a save or not, snapshots, recording, ledgers, hashes, the run loop and live data.
use super::*;

impl Server {
    /// A server on the noise land (tests, saves from before the bake).
    pub fn new(planet_n: i32, moon_n: i32, seed: i32, accept: Receiver<ServerLink>) -> Server {
        Server::with_world(World::new(planet_n, moon_n, seed), seed, accept)
    }
    /// A new game's server: its planet baked (ADR 0043), from memory, the disk or baked now.
    pub fn baked(planet_n: i32, moon_n: i32, seed: i32, accept: Receiver<ServerLink>) -> Server {
        Server::with_world(World::baked(planet_n, moon_n, seed), seed, accept)
    }
    /// `baked` on the block grid warped by `warp` (the conformal grid's trial, ADR 0131; `warp=`). The warp is in
    /// no save and no message: a client must be given the same `warp=`. `gen`: the world's generation (`Body::gen`),
    /// the newest unless `gen=` or `mantle=0` (`gen_arg`); the save and the welcome say it.
    pub fn baked_warp(planet_n: i32, moon_n: i32, seed: i32, warp: f64, gen: u8, accept: Receiver<ServerLink>) -> Server {
        let lazy = World::with_grid_lazy(planet_n, moon_n, seed, Some(engine::bake::get_at(seed, planet_n, gen)), Some(engine::bake::luna::get(seed, moon_n)), warp, gen, true);
        Server::with_world(lazy, seed, accept)
    }
    /// A server on `bodies` (columns and baked map's hash of every row, as a save has them; 0163-n-bodies),
    /// each body on the map of its hash (None: the seed's noise); a bake is redone if it is not kept. A map
    /// that comes out other than the hash stops the game, as a save this build cannot read does: its edits
    /// belong to other ground, and the next autosave would write over the old map's name.
    /// `gen`: the world's generation (`Body::gen`), as the save says; a world whose home is on the noise land
    /// (from before the bake) is of generation 0, as it always was.
    pub fn on_map(bodies: &[(i32, Option<u64>)], seed: i32, gen: u8, accept: Receiver<ServerLink>) -> Server {
        // a world has the rows of its generation (a world from before generation 5 has no Mars)
        let baked = bodies.first().is_some_and(|b| b.1.is_some());
        let rows = engine::bodies::of(if baked { gen } else { 0 });
        assert_eq!(bodies.len(), rows.len(), "this world has {} bodies and data/bodies.toml {} of its generation {gen}: it cannot be carried on", bodies.len(), rows.len());
        if !baked {
            return Server::with_world(World::of(rows, bodies.iter().map(|b| (b.0, None)).collect(), seed, 0.0, 0), seed, accept);
        }
        // (a body loaded on approach is checked against its hash when it comes in, `World::loader`)
        let list = bodies
            .iter()
            .zip(rows.iter())
            .enumerate()
            .map(|(i, (&(n, h), row))| match h {
                Some(h) if engine::world::lazy_row(rows, i) => (n, engine::world::Source::Later(h)),
                h => {
                    let m = h.map(|h| {
                        let bake = row.bake.as_ref().unwrap_or_else(|| panic!("{} has no bake in data/bodies.toml, but this world has a map of it", row.name));
                        bake.for_hash(seed, n, h).unwrap_or_else(|| panic!("this world was played on another map of {} of seed {seed} (map version {}): it cannot be carried on", row.name, bake.version()))
                    });
                    (n, engine::world::Source::Now(m))
                }
            })
            .collect();
        Server::with_world(World::of_sources(rows, list, seed, 0.0, gen), seed, accept)
    }
    pub(super) fn with_world(mut world: World, seed: i32, accept: Receiver<ServerLink>) -> Server {
        world.open_ledger();
        let ore_open = world.ore_left.clone().unwrap_or_default();
        let weather = world.bodies[0].map().and_then(|m| Some(engine::weather::Weather::new(m.climate.as_ref()?, None, world.sky.year).expect("a map's climate was checked on load").feeding(m)));
        Server {
            air0: weather.as_ref().map_or(0, |w| w.ledger()),
            weather,
            world,
            seed,
            accept,
            clients: Vec::new(),
            next_id: 1,
            tick: 0,
            data: "data".into(),
            tick0: 0,
            warp: 1,
            dt: 1.0 / TICK_HZ,
            debt: 0.0,
            rec: None,
            absent: BTreeMap::new(),
            loaded: None,
            autosave: None,
            saving: None,
            made: 0,
            removed: 0,
            water_skipped: 0,
            water_bytes: 0,
            tiles: FxMap::default(),
            near: FxMap::default(),
            tick_ms: None,
            map_pieces: HashMap::new(),
            ore_open,
            herds: engine::herds::compiled(seed),
            defs_seen: None,
            live_data: false,
            herds_loaded: None,
            falls: Default::default(),
            sky: Vec::new(),
            sky_at: None,
            force: None,
            piles: BTreeMap::new(),
            piles_changed: Vec::new(),
            item_made: BTreeMap::new(),
            taken: BTreeMap::new(),
            taken_changed: Vec::new(),
            furnaces: BTreeMap::new(),
            fires_changed: Vec::new(),
            events: Vec::new(),
            loads: Default::default(),
        }
    }

    /// The herds kept beside save `save` (`<save>.herds`, ADR 0083): none there is a world where no herd was
    /// met. A file this build cannot read is refused on stderr and the herds start from the seed.
    pub fn read_herds(&mut self, save: &Path) {
        let Ok(b) = std::fs::read(save.with_extension("herds")) else { return };
        match self.herds.load(&b) {
            Ok(()) => self.herds_loaded = Some(b),
            Err(e) => eprintln!("[technoape-server] {}: {e}", save.with_extension("herds").display()),
        }
    }

    /// Writes the herds beside the save at `path`, through a temporary file synced as the save's is.
    pub(super) fn write_herds(bytes: &[u8], path: &Path) -> std::io::Result<()> {
        crate::save::synced(bytes, &path.with_extension("herds.tmp"), &path.with_extension("herds"))
    }

    /// A server that carries on from `save` (its seed and planet win over any given elsewhere).
    pub fn from_save(save: Save, accept: Receiver<ServerLink>) -> Server {
        engine::world::known_generation(save.gen).unwrap_or_else(|e| panic!("{e}"));
        let mut s = Server::on_map(&save.bodies, save.seed, save.gen, accept);
        for (b, m) in &save.edits {
            s.world.set_block(*b, *m);
        }
        // a block saved mid-fall, or over a hole it had not fallen into yet, carries on (ADR 0079): each falling
        // block of the edits, and the one over each dug cell when it falls, is looked at once
        for (b, m) in &save.edits {
            let over = Block { pos: b.pos + glam::IVec3::Z, ..*b };
            if BLOCKS.get(*m).is_some_and(|d| d.falls) {
                s.falls.poke(*b);
            } else if *m == AIR && BLOCKS.get(s.world.block(over)).is_some_and(|d| d.falls) {
                s.falls.poke(over);
            }
        }
        // every ice block an edit set or laid open is followed again (water's phases; their melt so far is not saved)
        for (b, _) in &save.edits {
            s.world.watch_ice_near(*b);
        }
        // the clock of the save's last step first (a save's tick is one past it): its water is restored at that
        // step's tide, and the next step retides exactly when the running world's did (ADR 0067)
        s.world.time = save.tick.saturating_sub(1) as f64 / TICK_HZ;
        let World { bodies, edits, water, sky, time, .. } = &mut s.world;
        water.restore(&engine::world::Solid::at(bodies, edits, sky, *time), &save.water);
        water.restore_heat(&engine::world::Solid::at(bodies, edits, sky, *time), &save.heat.0, &save.kinds);
        water.froze = save.heat.1.iter().copied().collect();
        (water.vapour, water.spilt, water.waiting) = (save.booked.0.iter().copied().collect(), save.booked.1.iter().copied().collect(), save.booked.2.clone());
        (water.chambers, water.erupted) = save.chambers.clone();
        water.crust = save.crust.iter().map(|&(c, k, f, u)| ((c, k), (f, u))).collect();
        (water.ground_extra, water.rim) = (save.ground.0, save.ground.1);
        water.aquifer = save.ground.2.iter().cloned().collect();
        (s.made, s.removed, water.sea_extra) = (save.made, save.removed, save.sea_extra);
        water.rained = save.reaches.0;
        water.reach_extra = save.reaches.1.iter().copied().collect();
        water.rain_held = save.rain.0.iter().copied().collect();
        water.steps = save.rain.2;
        s.tick0 = save.tick;
        // the air carries on from the save; an older save on a map with a climate starts from the map's
        if let (Some(state), Some(m)) = (&save.air, s.world.bodies[0].map().filter(|m| m.climate.is_some())) {
            match engine::weather::Weather::new(m.climate.as_ref().unwrap(), Some(state), s.world.sky.year) {
                Ok(w) => s.weather = Some(w.feeding(m).with_pending(save.rain.1.clone(), s.tick)),
                Err(e) => eprintln!("[technoape-server] the save's air does not fit its map ({e}): it starts from the map's"),
            }
        }
        s.world.snow.restore(&save.snow);
        s.air0 = s.weather.as_ref().map_or(0, |w| w.ledger()) + s.world.water.rained as i128 + (s.world.snow.lent - s.world.snow.owing()) as i128;
        let cols: Vec<Col> = save.water.iter().map(|w| w.0).collect();
        s.water_changed(&cols);
        // a save from before Luna's grid and the inertial frame moved in the shared coordinates (ADR 0041,
        // version 5 and older): a player on Luna is moved with it; one in space (past the old Terra frame, at
        // most 43 km out, and not on Luna), whose place around the old sun means nothing now, goes to the spawn
        let (old, spawn) = (save.version < 6, s.spawn());
        let moved = |p: &save::Saved| {
            let mut p = p.clone();
            let at = DVec3::from(p.pos);
            if old && (at - engine::world::OLD_MOON_AT).length() < 2.0e5 {
                p.pos = (at - engine::world::OLD_MOON_AT + engine::world::MOON_AT).to_array();
            } else if old && at.length() > 2.0e5 {
                p.pos = spawn.to_array();
            }
            p
        };
        s.absent = save.players.iter().map(|p| (p.name.clone(), moved(p))).collect();
        // the piles, and the players of a save from before slots (ADR 0074): their stacks into slots, what does
        // not fit laid at their feet
        // (any whose slots do not fit: a save from before slots, a pack or a stack made smaller in data/items.toml)
        s.piles = save.held.piles.iter().filter(|p| items::stacks_fit(&p.1)).map(|(b, slots)| (key(*b), Pile { slots: slots.clone(), wait: (0, 0) })).collect();
        for (b, slots) in save.held.piles.iter().filter(|p| !items::stacks_fit(&p.1)) {
            let over: Vec<Stack> = slots.iter().flatten().filter(|s| s.n > 0).flat_map(|s| items::stacks(s.item, s.n as u32).into_iter().map(|t| Stack { payload: s.payload, ..t })).collect();
            s.lay(*b, over, (0, 0));
        }
        // a save from before the crafting grid (ADR 0140) has no cells: empty ones
        for p in s.absent.values_mut().filter(|p| p.slots.len() == items::APE_SLOTS) {
            p.slots.resize(items::SLOTS, None);
        }
        let old: Vec<String> = s.absent.iter().filter(|(_, p)| !items::ape_fits(&p.slots)).map(|(n, _)| n.clone()).collect();
        for n in old {
            let p = s.absent.get_mut(&n).unwrap();
            let stacks = std::mem::replace(&mut p.slots, items::ape());
            let over: Vec<Stack> = stacks.into_iter().flatten().filter_map(|st| Some(Stack { n: items::give(&mut p.slots, st), ..st }).filter(|o| o.n > 0)).collect();
            let feet = DVec3::from_array(p.pos);
            let at = s.world.block_at(feet + phys::up(&s.world, feet) * 0.5);
            s.lay(at, over, (0, 0));
        }
        // the furnaces (ADR 0141): one whose stacks no longer fit lays them where it stands, as a pile's
        for (b, f) in &save.held.furnaces {
            if items::stacks_fit(&f.slots) && f.slots.len() == engine::furnace::SLOTS {
                s.furnaces.insert(key(*b), f.clone());
            } else {
                let over: Vec<Stack> = f.slots.iter().flatten().filter(|s| s.n > 0).flat_map(|s| items::stacks(s.item, s.n as u32).into_iter().map(|t| Stack { payload: s.payload, ..t })).collect();
                s.lay(*b, over, (0, 0));
            }
        }
        s.piles_changed.clear();
        s.taken =save.held.taken.iter().map(|(b, y)| (key(*b), *y)).collect();
        s.item_made = s.held_now();
        s.loaded = Some(save);
        s
    }

    /// Everything a save holds, from the current state.
    pub fn snapshot(&self) -> Save {
        let mut edits: Vec<(Block, u16)> =
            self.world.edits.iter().flat_map(|(k, cells)| cells.iter().map(move |(pos, m)| (Block { body: k.body, face: k.face, pos: *pos }, *m))).collect();
        edits.sort_unstable_by_key(|(b, _)| (b.body, b.face, b.pos.x, b.pos.y, b.pos.z));
        let mut players = self.absent.clone();
        for c in self.clients.iter().filter(|c| !c.player.name.is_empty()) {
            players.insert(c.player.name.clone(), saved(c));
        }
        Save {
            version: save::VERSION,
            seed: self.seed,
            bodies: self.bodies(),
            blocks: save::registry(),
            edits,
            players: players.into_values().collect(),
            water: self.world.water.all(),
            made: self.made,
            removed: self.removed,
            sea_extra: self.world.water.sea_extra,
            tick: self.tick0.wrapping_add(self.tick),
            air: self.weather.as_ref().map(|w| w.state()),
            reaches: (self.world.water.rained, self.world.water.reach_extra.iter().map(|(&a, &x)| (a, x)).collect()),
            // after `air`: its `state` takes a job in flight back, which `pending` then names
            rain: (self.world.water.rain_held.iter().map(|(&a, &x)| (a, x)).collect(), self.weather.as_ref().and_then(|w| w.pending(self.tick)), self.world.water.steps),
            held: save::Held { items: ITEMS.table(), piles: self.piles.iter().map(|(k, p)| (unkey(*k), p.slots.clone())).collect(), taken: self.taken.iter().map(|(k, y)| (unkey(*k), *y)).collect(), furnaces: self.furnaces.iter().map(|(k, f)| (unkey(*k), f.clone())).collect() },
            gen: self.world.bodies[0].gen,
            heat: (self.world.water.heats(), self.world.water.froze.iter().map(|(&k, &u)| (k, u)).collect()),
            booked: (self.world.water.vapour.iter().map(|(&k, &u)| (k, u)).collect(), self.world.water.spilt.iter().map(|(&k, &u)| (k, u)).collect(), self.world.water.waiting.clone()),
            kinds: self.world.water.heat_kinds(),
            chambers: (self.world.water.chambers.clone(), self.world.water.erupted),
            crust: self.world.water.crust.iter().map(|(&(c, k), &(f, u))| (c, k, f, u)).collect(),
            snow: if self.world.snow.bare() { Default::default() } else { self.world.snow.state() },
            ground: (self.world.water.ground_extra, self.world.water.rim, self.world.water.aquifer.iter().map(|(&k, c)| (k, c.clone())).collect()),
        }
    }

    /// The hash of the baked map the planet stands on, None for the noise land.
    pub fn map_hash(&self) -> Option<u64> {
        self.world.map_hash(0)
    }
    /// Every body's columns and baked map's hash (None: the seed's noise), in row order: what a save, a
    /// recording and a welcome carry (0163-n-bodies).
    pub fn bodies(&self) -> Vec<(i32, Option<u64>)> {
        (0..self.world.bodies.len()).map(|i| (self.world.bodies[i].n, self.world.map_hash(i))).collect()
    }

    pub fn players(&self) -> usize {
        self.clients.len()
    }

    /// Every player as JSON: id, feet and whether it walks; a replay's dump.
    pub fn positions(&self) -> String {
        let v: Vec<String> = self
            .clients
            .iter()
            .map(|c| format!("{{\"id\":{},\"pos\":[{:.3},{:.3},{:.3}],\"walking\":{}}}", c.player.id, c.phys.pos.x, c.phys.pos.y, c.phys.pos.z, c.input.is_some()))
            .collect();
        format!("[{}]", v.join(","))
    }

    /// Records this session to `path` from the first tick on (the flight recorder).
    pub fn record(&mut self, path: &Path) -> std::io::Result<()> {
        if self.tick != 0 {
            return Err(std::io::Error::other("record before the first tick"));
        }
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        self.rec = Some(BufWriter::new(std::fs::File::create(path)?));
        Ok(())
    }

    pub(super) fn log(&mut self, ev: Ev) {
        let Some(w) = self.rec.as_mut() else { return };
        let b = postcard::to_stdvec(&ev).expect("events serialize");
        if let Err(e) = w.write_all(&(b.len() as u32).to_le_bytes()).and_then(|_| w.write_all(&b)) {
            eprintln!("[technoape-server] recording stopped: {e}");
            self.rec = None;
        }
    }

    /// The air's ledger and the rain it handed the rivers, against where they started (W5): 0 while no unit of
    /// the air's water appears or vanishes; None without the live air.
    pub fn air_ledger(&self) -> Option<i128> {
        let snow = &self.world.snow;
        Some(self.weather.as_ref()?.ledger() + self.world.water.rained as i128 + (snow.lent - snow.owing()) as i128 - self.air0)
    }

    /// Conservation (ADR 0020): water that differs from generation plus water in buckets, which only creative
    /// changes. Zero when no unit appeared or vanished.
    pub fn water_ledger(&self) -> i64 {
        let held: i64 = self.clients.iter().map(|c| c.water as i64).chain(self.absent.values().map(|p| p.water as i64)).sum();
        // (water a reaction or its heat boiled off is steam, water frozen is ice, both still counted: liquids.md
        // steps 4 and 5)
        let w = &self.world.water;
        let (steam, ice) = (w.vapour.get(&engine::fluids::WATER).copied().unwrap_or(0), w.froze.get(&engine::fluids::WATER).copied().unwrap_or(0));
        // (snow lying is counted water taken from the air's account: what it holds less what it took, ADR 0189)
        let snow = self.world.snow.held() - self.world.snow.lent;
        w.counted() + steam + ice + held + snow - (self.made - self.removed)
    }

    /// Conservation of ore (ADR 0063): the bodies whose ledger differs from a recount from the edits, as
    /// (body, ledger, recount). Empty when no ore block was lost or made unseen.
    pub fn ore_ledger(&self) -> Vec<(usize, i64, i64)> {
        let Some(left) = &self.world.ore_left else { return Vec::new() };
        let again = self.world.ore_recount(&self.ore_open);
        left.iter().zip(&again).enumerate().filter(|(_, (a, b))| a != b).map(|(k, (a, b))| (k, *a, *b)).collect()
    }

    /// The state as one number: the world's edits and water, every player's physics and input, the time
    /// carried over.
    pub fn hash(&self) -> u64 {
        let mut h = fnv(fnv(self.world.hash(), &self.tick.to_le_bytes()), &self.debt.to_bits().to_le_bytes());
        if let Some(w) = &self.weather {
            h = fnv(h, &w.view().hash.to_le_bytes());
        }
        let w = &self.world.water;
        h = fnv(h, &postcard::to_allocvec(&(w.all(), self.made, self.removed, w.sea_extra, &w.reach_extra, &w.rain_held, w.rained)).unwrap());
        for p in self.absent.values() {
            h = fnv(h, &hashed(p));
        }
        h = fnv(h, &self.herds.to_bytes());
        h = fnv(h, &postcard::to_allocvec(&self.piles).unwrap());
        // only when there is one: an empty list's byte would change the hash of every recording from before the crucible
        if !self.furnaces.is_empty() {
            h = fnv(h, &postcard::to_allocvec(&self.furnaces).unwrap());
        }
        // the same for the volcanoes' chambers (a world without a live one keeps the hashes it had)
        if !w.chambers.is_empty() {
            h = fnv(h, &postcard::to_allocvec(&(&w.chambers, w.erupted)).unwrap());
        }
        if !w.crust.is_empty() {
            h = fnv(h, &postcard::to_allocvec(&w.crust).unwrap());
        }
        // and the snow's (ADR 0189): a world without any keeps its hashes
        if !self.world.snow.bare() {
            h = fnv(h, &postcard::to_allocvec(&self.world.snow.state()).unwrap());
        }
        // and the ground water off its rest (ADR 0190; none in a world without a table)
        if !w.aquifer.is_empty() || w.ground_extra != 0 || w.rim != 0 {
            h = fnv(h, &postcard::to_allocvec(&(&w.aquifer, w.ground_extra, w.rim)).unwrap());
        }
        for c in &self.clients {
            let p = &c.phys;
            h = fnv(h, &c.player.id.to_le_bytes());
            for v in [p.pos, p.vel, p.fwd, p.up] {
                for x in v.to_array() {
                    h = fnv(h, &x.to_bits().to_le_bytes());
                }
            }
            h = fnv(h, &[p.on_ground as u8, c.input.is_some() as u8, c.creative as u8]);
            h = fnv(h, &c.water.to_le_bytes());
            h = fnv(h, &postcard::to_allocvec(&(&c.vitals, c.was.to_array())).unwrap());
            h = fnv(h, &postcard::to_allocvec(&c.slots).unwrap());
            // only when it holds something: the same for the recordings from before the journal
            if c.journal.iter().any(|s| *s != engine::journal::State::Unknown) {
                h = fnv(h, &postcard::to_allocvec(&c.journal).unwrap());
            }
            for x in c.player.pos {
                h = fnv(h, &x.to_bits().to_le_bytes());
            }
            if let Some(i) = c.input {
                for x in [i.mv[0], i.mv[1], i.fwd[0], i.fwd[1], i.fwd[2], i.pitch, i.turn, i.tilt, i.roll] {
                    h = fnv(h, &x.to_bits().to_le_bytes());
                }
                h = fnv(h, &[i.jump as u8]);
            }
        }
        h
    }

    /// Ticks forever at `hz`, sleeping between ticks; the thread or process ends it.
    /// Ticks at `hz` until `stop` is set, then saves (if `autosave` says where) and returns.
    pub fn run(mut self, hz: f64, stop: &std::sync::atomic::AtomicBool) {
        self.dt = 1.0 / hz;
        let step = Duration::from_secs_f64(1.0 / hz);
        let mut next = Instant::now();
        while !stop.load(std::sync::atomic::Ordering::Relaxed) {
            let t = Instant::now();
            self.tick();
            if let Some(v) = &self.tick_ms {
                v.lock().unwrap().push(t.elapsed().as_secs_f32() * 1000.0);
            }
            next += step;
            match next.checked_duration_since(Instant::now()) {
                Some(d) => std::thread::sleep(d),
                None => next = Instant::now(), // a late tick does not try to catch up
            }
        }
        self.save_now();
    }

    /// Saves where `autosave` says, on this thread: for shutting down.
    pub fn save_now(&mut self) {
        if let Some(h) = self.saving.take() {
            let _ = h.join();
        }
        if let Some((path, _)) = &self.autosave {
            match self.snapshot().write(path).and_then(|_| Server::write_herds(&self.herds.to_bytes(), path)) {
                Ok(()) => eprintln!("[technoape-server] saved to {}", path.display()),
                Err(e) => eprintln!("[technoape-server] save failed: {e}"),
            }
        }
    }

    /// Live definitions (ADR 0134): once a second the block, item, creature and herd files of `data/` are
    /// looked at (four stats); a changed one is read and put in force here and on every client, a bad one is
    /// named on stderr and the rows before stay. No `data/` folder beside the server: the build's rows.
    /// shortcut: read and parsed on the tick thread (about a millisecond for 25 kB); a recording keeps no
    /// reload, so a session that reloaded replays with the build's rows.
    pub(super) fn watch_defs(&mut self) {
        let dir = std::path::Path::new("data");
        let seen = ["blocks.toml", "items.toml", "creatures.toml", "herds.toml"].map(|f| std::fs::metadata(dir.join(f)).ok().and_then(|m| Some((m.modified().ok()?, m.len()))));
        if seen.iter().any(|s| s.is_none()) || self.defs_seen == Some(seen) {
            return;
        }
        self.defs_seen = Some(seen);
        match engine::live::load(dir) {
            Ok(false) => {}
            Ok(true) => {
                self.world.refresh_defs();
                let (blocks, items) = engine::live::reloaded().expect("just applied");
                for c in &self.clients {
                    c.link.send(ToClient::Defs { blocks: blocks.clone(), items: items.clone() });
                }
                eprintln!("[server] data: blocks and items reloaded");
            }
            Err(e) => eprintln!("[server] data: refused, the rows before stay: {e}"),
        }
        let read = |f: &str| std::fs::read_to_string(dir.join(f)).map_err(|e| format!("data/{f}: {e}"));
        match read("creatures.toml").and_then(|c| self.herds.reload(&c, &read("herds.toml")?)) {
            Ok(false) => {}
            Ok(true) => {
                for c in &self.clients {
                    c.link.send(ToClient::Kinds(self.herds.kinds.iter().map(|k| k.name.clone()).collect()));
                }
                eprintln!("[server] data: creatures and herds reloaded");
            }
            Err(e) => eprintln!("[server] data: refused, the rows before stay: {e}"),
        }
    }
}

/// An absent player's bytes in the state hash: its save's, an empty journal's byte left out, so the hash of
/// every recording from before the journal (draft ADR journal-data) holds.
pub(crate) fn hashed(p: &save::Saved) -> Vec<u8> {
    let mut b = postcard::to_allocvec(p).unwrap();
    if p.journal.is_empty() {
        b.pop();
    }
    b
}
