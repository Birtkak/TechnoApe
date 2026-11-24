//! Server: animals sent to players, chat and the commands.
use super::*;

impl Server {
    /// Each client's animal deltas (ADR 0083): those within `SEE` new to it, moved over 2 cm or doing
    /// something else, and the ids it no longer sees.
    pub(super) fn send_animals(&mut self, age: u64) {
        let herds = &self.herds;
        let world = &self.world;
        for c in &mut self.clients {
            let me = c.phys.pos;
            let on = world.frame_at(me) == 0;
            let mut seen = Vec::new();
            let mut here: std::collections::HashSet<u32> = Default::default();
            for a in herds.awake_animals(age).filter(|a| on && (a.p.pos - me).length() < engine::net::SEE) {
                here.insert(a.id);
                let v = engine::net::Animal { id: a.id, kind: a.kind, pos: a.p.pos.to_array(), fwd: a.p.fwd.as_vec3().to_array(), act: a.mind as u8, call: a.call };
                let old = c.animals.get(&a.id);
                if old.is_none_or(|o| o.act != v.act || o.call != v.call || (DVec3::from_array(o.pos) - a.p.pos).length() > 0.02) {
                    c.animals.insert(a.id, v);
                    seen.push(v);
                }
            }
            let mut gone: Vec<u32> = c.animals.keys().copied().filter(|id| !here.contains(id)).collect();
            gone.sort_unstable();
            for id in &gone {
                c.animals.remove(id);
            }
            if !seen.is_empty() || !gone.is_empty() {
                c.link.send(ToClient::Animals { seen, gone });
            }
        }
    }

    /// Puts player `i` at `p` looking along `look` (in the tangent plane there), at rest.
    pub(super) fn put(&mut self, i: usize, p: DVec3, look: DVec3) {
        let c = &mut self.clients[i];
        c.phys = phys::Player::at(p, phys::tangent(look, phys::up(&self.world, p), DVec3::X));
        (c.input, c.credit, c.was) = (None, 0, DVec3::ZERO);
        c.queue.clear();
        (c.player.pos, c.player.fwd) = (p.to_array(), c.phys.fwd.as_vec3().to_array());
    }

    /// Where /goto takes a player (ADR 0023): Terra's spawn, another body (Luna) where the sun is 45 degrees up,
    /// or a spot in space SUN_VIEW sun radii from the sun's centre with the sun on its horizon; and the way to look.
    /// A body is named by its row in `data/bodies.toml` (Terra or planet, Luna or moon).
    pub(super) fn goto(&self, to: &str) -> Option<(DVec3, DVec3)> {
        let w = &self.world;
        match to {
            "sun" => {
                let (sun, planet) = (w.sky.sun(w.time), w.sky.planet(w.time).centre);
                // beside the sun as seen from the planet: up there is away from the planet, the sun level
                let side = (planet - sun).normalize().cross(DVec3::Z).normalize();
                let x = sun + side * self.world.sky.sun_radius * SUN_VIEW;
                let (pos, f) = w.locate(x);
                Some((pos, w.pose(f, w.time).rot.inverse() * (sun - x)))
            }
            _ => match engine::bodies::find(to).filter(|&i| i < w.bodies.len())? {
                0 => Some((w.bodies[0].spawn_point(), w.bodies[0].spawn.any_orthonormal_vector())),
                // the day side, the sun 45 degrees up: an airless night is black (on a gas body, its cloud tops:
                // `Body::height`, step 6a)
                i => {
                    let p = w.pose(i, w.time);
                    let s = (p.rot.inverse() * (w.sky.sun(w.time) - p.centre)).normalize();
                    let d = (s + s.any_orthonormal_vector()).normalize();
                    Some((w.bodies[i].stand_point(d), s))
                }
            },
        }
    }

    /// One line to everyone's chat.
    pub(super) fn say(&self, line: String) {
        for c in &self.clients {
            c.link.send(ToClient::Chat(line.clone()));
        }
    }

    /// A chat line from client `i` (ADR 0023): a '/' command runs here and answers it alone; any other line
    /// goes to everyone with its sender's name.
    pub(super) fn chat(&mut self, i: usize, text: &str, changed: &mut Vec<(Block, u16)>) {
        let text: String = text.chars().filter(|c| !c.is_control()).take(CHAT_MAX).collect();
        let text = text.trim();
        if let Some(cmd) = text.strip_prefix('/') {
            let answer = self.command(i, cmd, changed);
            self.clients[i].link.send(ToClient::Chat(answer));
        } else if !text.is_empty() {
            let name = &self.clients[i].player.name;
            self.say(format!("<{}> {text}", if name.is_empty() { "?" } else { name }));
        }
    }

    /// The commands (ADR 0023), and the answer for the player who gave one. shortcut: anyone may run every
    /// command; an operator role when servers go public.
    fn command(&mut self, i: usize, cmd: &str, changed: &mut Vec<(Block, u16)>) -> String {
        let w: Vec<&str> = cmd.split_whitespace().collect();
        let mode = |creative: bool| if creative { "creative" } else { "survival" };
        let block_named = |name: &str| BLOCKS.placeable().find(|d| d.name.eq_ignore_ascii_case(name));
        let blocks = || BLOCKS.placeable().map(|d| d.name.as_str()).collect::<Vec<&str>>().join(", ");
        match w.first().map(|s| s.to_lowercase()).as_deref() {
            Some("help") | None => [
                "Commands:",
                "  /time                the time of day at the spawn",
                "  /time set <when>     sunrise day noon sunset night",
                "                       midnight, 14:30, or world seconds",
                "                       or spring summer autumn winter",
                "  /time add <seconds>  move the clock on",
                "  /playmode <mode>     creative or survival",
                "  /give <block> [n]    n blocks, 64 if left out",
                "  /place <block>       one under your feet; or at",
                "    [face i j layer]   that block of the body you are on (F3)",
                "  /structure <name>    a file of data/structures stood five",
                "    [seed]             blocks ahead (a grown one: that seed)",
                "  /goto <where>        terra (home: the spawn), luna, mars,",
                "                       jupiter (a new world's: its cloud",
                "                       tops), or sun",
                "  /timewarp <n>        the clock n times faster (1 to 10000,",
                "                       up, down); . and , step it too",
                "  /weather <what>      clear cloudy rain storm snow, or auto",
                "  /fluid <name>        what creative's bucket pours (water,",
                "                       lava, oil: data/fluids.toml)",
                "  /erupt               fills the nearest live volcano's",
                "                       chamber to bursting: it erupts",
                "  /heal, /feed [n]     full health, full hunger (or n)",
                "  /thirst [n]          full thirst (or n)",
                "  /hurt <points>       takes health: 2 points a segment",
                "  /journal [entry      your journal's entries; sets one",
                "    unknown|noticed|known]  (known notices what it points at)",
                "  /help                this list",
                "Creative: double space flies, the wheel sets the speed.",
            ]
            .join("\n"),
            Some("time") => {
                let now = self.tick0.wrapping_add(self.tick) as f64 / TICK_HZ;
                let named = |v: &str| match v {
                    "sunrise" => Some(6.0),
                    "day" => Some(9.0),
                    "noon" => Some(12.0),
                    "sunset" => Some(18.0),
                    "night" => Some(21.0),
                    "midnight" => Some(0.0),
                    _ => {
                        let (h, m) = v.split_once(':')?;
                        let (h, m) = (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?);
                        (h < 24 && m < 60).then(|| h as f64 + m as f64 / 60.0)
                    }
                };
                let secs = |v: &str| v.parse::<f64>().ok().filter(|s| s.is_finite());
                let to = match (w.get(1).copied(), w.get(2).copied()) {
                    (None | Some("query"), _) => None,
                    (Some("set"), Some(v)) if sky::season(v).is_some() => Some(sky::next_season(now, sky::season(v).unwrap(), self.world.sky.year)),
                    (Some("set"), Some(v)) => match (named(v), secs(v)) {
                        (Some(h), _) => Some(sky::next_hour(now, h)),
                        (None, Some(s)) if s >= 0.0 => Some(s),
                        _ => return format!("not a time: {v}\n  try noon, sunset, 14:30 or a number of seconds"),
                    },
                    (Some("add"), Some(v)) => match secs(v).map(|s| now + s) {
                        Some(t) if t >= 0.0 => Some(t),
                        _ => return format!("not a number of seconds: {v}"),
                    },
                    _ => return "usage:\n  /time\n  /time set <sunrise|day|noon|sunset|night|midnight|hh:mm|seconds>\n  /time set <spring|summer|autumn|winter>\n  /time add <seconds>".into(),
                };
                let t = match to {
                    Some(t) => {
                        // the next tick shows it; a time before this session's start wraps the tick counter
                        let ticks = (t.min(1e12) * TICK_HZ).round() as u64; // past 1e12 s the clock steps coarser than 0.1 ms
                        self.tick0 = ticks.wrapping_sub(self.tick);
                        self.world.time = ticks as f64 / TICK_HZ;
                        self.world.time
                    }
                    None => now,
                };
                let (day, h) = sky::clock(t);
                let what = if to.is_some() { "time set to" } else { "it is" };
                let m = (h * 60.0).round() as u32 % (24 * 60);
                format!("{what} day {day}, {:02}:{:02} at the spawn (world time {t:.0} s)", m / 60, m % 60)
            }
            // for trying the journal (draft ADR journal-data) until the book shows it
            Some("journal") => {
                let j = &*engine::journal::JOURNAL;
                let c = &mut self.clients[i];
                match (w.get(1), w.get(2).and_then(|s| engine::journal::State::named(&s.to_lowercase()))) {
                    (None, _) => j.entries.iter().zip(&c.journal).map(|(e, s)| format!("{}: {}", e.name, s.name())).collect::<Vec<_>>().join("
"),
                    (Some(name), Some(to)) => match j.index(name) {
                        Some(k) => {
                            c.journal[k] = engine::journal::State::Unknown;
                            j.raise(&mut c.journal, k, to);
                            format!("{name}: {}", to.name())
                        }
                        None => format!("no journal entry '{name}': {}", j.entries.iter().map(|e| e.name.as_str()).collect::<Vec<_>>().join(", ")),
                    },
                    _ => "usage: /journal [<entry> unknown|noticed|known]".into(),
                }
            }
            // shortcut: until there is food and harm of more kinds, these let the overlay and the death screen be
            // tried (ADR 0040); they go to the operator role with the others
            Some(cmd @ ("heal" | "feed" | "thirst" | "hurt")) => {
                let c = &mut self.clients[i];
                if c.vitals.dead.is_some() {
                    return "you are dead: respawn first".into();
                }
                if c.creative {
                    return "creative has no health or hunger: /playmode survival".into();
                }
                match cmd {
                    "heal" => {
                        c.vitals.health = vitals::MAX;
                        "health full".into()
                    }
                    // a number sets it there: /feed 0 shows empty hunger
                    "feed" => match w.get(1).map(|s| s.parse::<u8>()) {
                        None => {
                            c.vitals.hunger = vitals::MAX as f64;
                            "hunger full".into()
                        }
                        Some(Ok(n)) if n <= vitals::MAX => {
                            c.vitals.hunger = n as f64;
                            format!("hunger {n} of {}", vitals::MAX)
                        }
                        _ => format!("usage: /feed [0 to {}]", vitals::MAX),
                    },
                    // the same for thirst: /thirst 0 shows it empty
                    "thirst" => match w.get(1).map(|s| s.parse::<u8>()) {
                        None => {
                            c.vitals.thirst = vitals::MAX as f64;
                            "thirst full".into()
                        }
                        Some(Ok(n)) if n <= vitals::MAX => {
                            c.vitals.thirst = n as f64;
                            format!("thirst {n} of {}", vitals::MAX)
                        }
                        _ => format!("usage: /thirst [0 to {}]", vitals::MAX),
                    },
                    _ => {
                        let Some(n) = w.get(1).and_then(|s| s.parse::<u32>().ok()).filter(|n| *n > 0) else {
                            return "usage: /hurt <points>\n  20 is full health, 2 points a segment".into();
                        };
                        let name = if c.player.name.is_empty() { "?".to_string() } else { c.player.name.clone() };
                        if c.vitals.hurt(n.min(255) as u8, || format!("{name} was hurt by a command")) {
                            let line = c.vitals.dead.clone().unwrap_or_default();
                            self.say(line);
                            return "you died".into();
                        }
                        format!("took {n}: health {} of {}", c.vitals.health, vitals::MAX)
                    }
                }
            }
            Some("goto") if self.clients[i].vitals.dead.is_some() => "you are dead: respawn first".into(),
            Some("goto") => {
                let to = w.get(1).map(|s| s.to_lowercase()).unwrap_or_default();
                if to == "sun" && !self.clients[i].creative {
                    return "the sun is creative's: there is nothing to stand on\n  /playmode creative, then /goto sun".into();
                }
                if let Some(b) = engine::bodies::find(&to).filter(|b| self.loads.failed.contains(b)) {
                    return format!("{}'s ground cannot be loaded on this server (its map is not this build's)", self.world.bodies[b].name);
                }
                if engine::bodies::find(&to).is_some_and(|b| b < self.world.bodies.len() && !self.world.loaded(b)) {
                    // its ground comes in first (step 6b, `lazy.rs`): the player goes when it is in
                    self.loads.gotos.retain(|g| g.0 != self.clients[i].player.id);
                    self.loads.gotos.push((self.clients[i].player.id, to.clone()));
                    return format!("going to {}: its ground is loading", self.world.bodies[engine::bodies::find(&to).unwrap()].name);
                }
                match self.goto(&to) {
                    Some((p, look)) => {
                        self.put(i, p, look);
                        format!("gone to {}", engine::bodies::find(&to).map_or("the sun", |b| self.world.bodies[b].name.as_str()))
                    }
                    None => {
                        let names: Vec<String> = self.world.bodies.iter().map(|b| b.name.to_lowercase()).collect();
                        format!("usage: /goto {}|sun\n  {} (or home) is the spawn", names.join("|"), names[0])
                    }
                }
            }
            Some("weather") => match (w.get(1).map(|s| s.to_lowercase()), &self.weather) {
                (_, None) => "this world has no air grid".into(),
                (None, Some(_)) => format!("the weather is {}
  /weather <{}>", if self.force.is_some() { "forced" } else { "the air's own" }, engine::weather::Force::NAMES),
                (Some(n), Some(_)) => match engine::weather::Force::named(&n) {
                    Some(f) => {
                        self.force = f;
                        self.sky_at = None;
                        if f.is_some() { format!("weather forced: {n}") } else { "the air's own weather again".into() }
                    }
                    None => format!("not a weather: {n}
  /weather <{}>", engine::weather::Force::NAMES),
                },
            },
            Some("timewarp") | Some("warp") => {
                let now = self.warp;
                let to = match w.get(1).map(|s| s.to_lowercase()).as_deref() {
                    None => return format!("the time warp is {now}x
usage: /timewarp <1 to {MAX_WARP}|up|down>"),
                    Some("up") => (now * 10).min(MAX_WARP),
                    Some("down") => (now / 10).max(1),
                    Some(v) => match v.trim_end_matches('x').parse::<u64>() {
                        Ok(n) if (1..=MAX_WARP).contains(&n) => n,
                        _ => return format!("not a time warp: {v}
  1 (real time) to {MAX_WARP}"),
                    },
                };
                self.warp = to;
                let (day, year) = (sky::DAY / to as f64, self.world.sky.year / to as f64);
                format!("time warp {to}x: a day in {day:.0} s, a year in {year:.0} s")
            }
            Some("erupt") => match self.world.force_eruption(self.clients[i].phys.pos) {
                Some(k) => {
                    let ch = self.world.chamber(k).expect("a live chamber");
                    format!("the nearest live volcano's chamber is full: {:.0} m3 of lava will pour from its crater", ch.holds / engine::liquid::U as f64)
                }
                None => "no live volcano on this world".into(),
            },
            Some("fluid") => {
                let f = engine::fluids::FLUIDS.now();
                let c = &mut self.clients[i];
                match w.get(1).map(|s| s.to_lowercase()) {
                    None => format!("creative's bucket pours {}\nusage: /fluid <{}>", f.get(c.fluid).name, f.names()),
                    Some(n) => match f.kind(&n) {
                        Some(k) => {
                            c.fluid = k;
                            format!("creative's bucket pours {n}")
                        }
                        None => format!("no fluid '{n}': {}", f.names()),
                    },
                }
            }
            Some("playmode") | Some("gamemode") => {
                let c = &mut self.clients[i];
                let on = match w.get(1).map(|s| s.to_lowercase()).as_deref() {
                    None => return format!("you play in {} mode\nusage: /playmode creative|survival", mode(c.creative)),
                    Some("creative" | "c" | "1") => true,
                    Some("survival" | "s" | "0") => false,
                    Some(m) => return format!("no play mode '{m}': creative or survival"),
                };
                (c.creative, c.inv_dirty) = (on, true);
                format!("play mode: {}", mode(on))
            }
            Some("give") => {
                // the count is a last word that is a number; the item's name is the words before it
                let (words, n) = match w.last().and_then(|s| s.parse::<u32>().ok()) {
                    Some(n) if w.len() > 2 => (&w[1..w.len() - 1], Some(n)),
                    _ => (&w[1..], None),
                };
                let name = words.join(" ");
                let Some(id) = ITEMS.named(&name) else {
                    let what = if name.is_empty() { String::new() } else { format!("no block or item '{name}'\n") };
                    let others: Vec<&str> = ITEMS.defs.iter().map(|d| d.name.as_str()).collect();
                    return format!("{what}usage: /give <block or item> [count]\nblocks: {}\nitems: {}", blocks(), others.join(", "));
                };
                // a stack if no count is given; into the slots, what does not fit at the feet (ADR 0074)
                let n = n.unwrap_or(ITEMS.stack(id) as u32).min(GIVE_MAX);
                let c = &mut self.clients[i];
                let over: Vec<Stack> = items::stacks(id, n).filter_map(|st| Some(Stack { n: items::give(&mut c.slots, st), ..st }).filter(|o| o.n > 0)).collect();
                c.inv_dirty = true;
                let held = items::count(&c.slots, id);
                *self.item_made.entry(id).or_default() += n as i64;
                let more = if over.is_empty() { String::new() } else { format!(", {} at your feet", over.iter().map(|s| s.n as u32).sum::<u32>()) };
                let feet = self.clients[i].phys.pos;
                self.lay(self.world.block_at(feet + phys::up(&self.world, feet) * 0.5), over, (0, 0));
                format!("gave {n} {}: {held} held{more}", ITEMS.name(id))
            }
            Some("place") => {
                // bugs/2026-11-11_13-03-12: the last four words, when numbers, are F3's "Feet in" (face, i, j,
                // layer) on the body the player is on; without them the block under the feet. Free, like /give.
                // shortcut: anyone may place any block anywhere on their body, as anyone may run every command;
                // it becomes the owner's and creative's when the server has roles
                let nums: Vec<i32> = w.iter().skip(1).rev().take(4).map_while(|s| s.parse().ok()).collect();
                let (words, at) = if nums.len() == 4 { (&w[1..w.len() - 4], Some(nums)) } else { (&w[1..], None) };
                let name = words.join(" ");
                let Some(d) = block_named(&name) else {
                    let what = if name.is_empty() { String::new() } else { format!("no block '{name}'\n") };
                    return format!("{what}usage: /place <block> [face i j layer]\nblocks: {}", blocks());
                };
                let world = &self.world;
                let feet = self.clients[i].phys.pos;
                let body = world.frame_at(feet);
                if body == engine::world::INERTIAL {
                    return "there is no body under you out here: /place works on Terra and Luna".into();
                }
                let b = match at {
                    // (a face that is no face, a negative one too, fails `fits` below)
                    Some(n) => Block { body: body as u8, face: u8::try_from(n[3]).unwrap_or(u8::MAX), pos: glam::IVec3::new(n[2], n[1], n[0]) },
                    None => world.block_at(feet - phys::up(world, feet) * 0.5),
                };
                if !Shape::one(b).fits(world) {
                    return format!("no such block on {}: face 0 to 5, then i j layer as F3 shows them", world.bodies[body].name);
                }
                if BLOCKS.solid(d.id) && self.clients.iter().any(|c| phys::holds(world, &c.phys, b)) {
                    return "a player stands there: a solid block would trap them".into();
                }
                let was = self.world.block(b);
                self.world.set_block(b, d.id);
                changed.push((b, d.id));
                self.events.push(Event::Edit { at: b, was, now: d.id, by: Some(self.clients[i].player.id) });
                format!("placed {} at face {} {} {}, layer {}", d.name, b.face, b.pos.x, b.pos.y, b.pos.z)
            }
            Some("spawn") => {
                // a real animal three metres ahead (ADR 0083); the client keeps `/spawn <creature>` as its preview
                let names = self.herds.kinds.iter().map(|k| k.name.as_str()).collect::<Vec<_>>().join(", ");
                let (Some(name), Some("live"), 3) = (w.get(1), w.get(2).copied(), w.len()) else {
                    return format!("usage: /spawn <creature> live   a real one three metres ahead. There are: {names}");
                };
                let c = &self.clients[i];
                if self.world.frame_at(c.phys.pos) != 0 {
                    return "animals live on Terra: /spawn works there".into();
                }
                let at = c.phys.pos + c.phys.fwd * 3.0 - self.world.bodies[0].centre;
                let (fwd, age) = (-c.phys.fwd, self.herds.clock);
                match self.herds.add(&self.world, name, at.normalize(), fwd, age) {
                    Some(id) => format!("a {name} (animal {id}) stands three metres ahead"),
                    None if self.herds.kinds.iter().any(|k| k.name == *name) => "no ground to stand on there (water)".into(),
                    None => format!("no creature '{name}'. There are: {names}"),
                }
            }
            Some("structure") => {
                // A structure file (the Studio's, ADR 0052) stood near the player, to see it in the world: its
                // anchor in the first air over the ground five metres ahead of the player (the way the body faces). Free
                // and anyone's, like /place.
                // shortcut: the file is read on the tick thread from the server's own data/ (small, and a
                // developer's command), and a replay of a session that used it needs the same file; it goes
                // to the operator role with the others.
                const USAGE: &str = "usage: /structure <name> [seed]   a file of data/structures, five blocks ahead of you";
                let name = w.get(1).copied().unwrap_or("");
                let seed = w.get(2).map(|s| s.parse::<u64>());
                // (letters, digits, _ and - only: no path out of the folder; and none of Windows' device names,
                // which a read would wait on)
                let device = { let n = name.to_ascii_lowercase(); ["con", "prn", "aux", "nul"].contains(&n.as_str()) || (n.len() == 4 && (n.starts_with("com") || n.starts_with("lpt"))) };
                if name.is_empty() || device || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') || matches!(seed, Some(Err(_))) || w.len() > 3 {
                    return USAGE.into();
                }
                let file = format!("{}/structures/{name}.toml", self.data);
                let mut s = match std::fs::read_to_string(&file).map_err(|e| e.to_string()).and_then(|t| engine::structures::Structure::parse(&file, &t, &BLOCKS)) {
                    Ok(s) => s,
                    Err(e) => return format!("no structure '{name}': {e}"),
                };
                if let (Some(Ok(seed)), Some(r)) = (seed, s.grow.clone()) {
                    s = engine::structures::grow(&engine::structures::Recipe { seed, ..r });
                }
                let world = &self.world;
                let feet = self.clients[i].phys.pos;
                if world.frame_at(feet) == engine::world::INERTIAL {
                    return "there is no body under you out here: /structure works on Terra and Luna".into();
                }
                let mut foot = world.block_at(feet + self.clients[i].phys.fwd * 5.0);
                let solid = |b: Block| Shape::one(b).fits(world) && BLOCKS.solid(world.block(b));
                let mut found = false;
                for _ in 0..24 {
                    let below = Block { pos: foot.pos - glam::IVec3::Z, ..foot };
                    if solid(foot) {
                        foot.pos.z += 1;
                    } else if !solid(below) && Shape::one(below).fits(world) {
                        foot.pos.z -= 1;
                    } else {
                        found = true;
                        break;
                    }
                }
                if !found {
                    return "no ground within 24 blocks of the point five ahead of you: stand nearer the ground".into();
                }
                // every edit is an effect and a remesh on each player's frame thread: a structure of more than
                // this many blocks is the world generator's to place, not a command's
                const MOST: usize = 4096;
                let cells = s.blocks(&BLOCKS);
                if cells.len() > MOST {
                    return format!("{name} is {} blocks: /structure places {MOST} at most", cells.len());
                }
                // where the players are, gathered once (not once a block)
                let held: std::collections::HashSet<Block> = self.clients.iter().flat_map(|c| phys::cells(world, c.phys.pos)).collect();
                // x along j, z along i, y up the layers: as `structure=` stands one and the Studio shows it
                let put: Vec<(Block, u16)> = cells
                    .into_iter()
                    .map(|(d, id)| (Block { pos: foot.pos + glam::IVec3::new(d[2], d[0], d[1]), ..foot }, id))
                    .filter(|(b, id)| Shape::one(*b).fits(world) && !(BLOCKS.solid(*id) && held.contains(b)))
                    .collect();
                let by = Some(self.clients[i].player.id);
                for &(b, id) in &put {
                    let was = self.world.block(b);
                    self.world.set_block(b, id);
                    changed.push((b, id));
                    self.events.push(Event::Edit { at: b, was, now: id, by });
                }
                format!("placed {name}: {} blocks, five ahead of you at face {} {} {}, layer {}", put.len(), foot.face, foot.pos.x, foot.pos.y, foot.pos.z)
            }
            Some(other) => format!("unknown command /{other}\n  /help lists the commands"),
        }
    }

    /// Where a new player's feet go: on the surface at the planet's +y pole, like the client's benchmark.
    pub(super) fn spawn(&self) -> DVec3 {
        let b = &self.world.bodies[0];
        b.spawn_point()
    }

    /// The server's clock in seconds: ticks, so a replay decides as the run did.
    pub(super) fn clock(&self) -> f64 {
        self.tick as f64 * self.dt
    }
}
