//! One frame: input, the server's messages, streaming, the passes in order and their timing (ADR 0025 L: moved out of main.rs).
use super::*;

pub(crate) fn frustum_planes(m: Mat4) -> [Vec4; 6] {
    let r = [m.row(0), m.row(1), m.row(2), m.row(3)];
    // reversed-z infinite: near plane is z <= w, no far plane (use a dummy always-true plane)
    let mut p = [r[3] + r[0], r[3] - r[0], r[3] + r[1], r[3] - r[1], r[3] - r[2], Vec4::new(0.0, 0.0, 0.0, 1.0)];
    for q in p.iter_mut() {
        let l = q.truncate().length();
        if l > 0.0 {
            *q /= l;
        }
    }
    p
}

impl App {
    pub(crate) fn frame(&mut self) {
        // a new world's bake or a server's map done: the game starts again with it
        self.poll_loading();
        if self.done {
            return;
        }
        // the Studio's view (ADR 0049): its commands first
        if self.run == "view" {
            self.view_take();
            if self.done {
                return;
            }
        }
        let a = &self.args;
        // length= scales a run (the benchmark uses 0.5); quick=1 is 0.25, for iterating, never a final number
        let q: f64 = a.get("length", if a.get("quick", 0) == 1 { 0.25 } else { 1.0 });
        let warm: f64 = a.get("warmup", (10.0 * q).max(3.0));
        let speed: f64 = a.get("speed", 60.0);
        let alt: f64 = a.get("alt", 20.0);
        let upload_mb: f64 = a.get("upload_mb", 16.0);
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f64();
        let between = self.frame_end.map_or(0.0, |e| (now - e).as_secs_f64());
        self.last_frame = now;
        let t = self.loaded_at.map(|l| (now - l).as_secs_f64()).unwrap_or(0.0);
        let keep = keeps_frames(&self.run, a);
        let measuring = self.loaded_at.is_some() && t >= warm && keep;
        // the tick and job numbers count from the measuring on, and play keeps none
        if !measuring || self.run == "play" {
            self.streamer.job_times.clear();
            if let Some(v) = &self.tick_log {
                let mut v = v.lock().unwrap();
                if self.run == "play" {
                    // F3's graph: the last ten seconds of ticks
                    self.ticks.extend(v.drain(..).map(|ms| (now, ms)));
                    let over = self.ticks.len().saturating_sub(10 * TICK_HZ as usize);
                    self.ticks.drain(..over);
                }
                v.clear();
            }
        }
        let len = if self.run == "play" || self.run == "view" { a.get("seconds", f64::INFINITY) } else { run_len(&self.run, warm, q) };
        if t >= len && self.loaded_at.is_some() {
            self.finish();
            return;
        }
        let cpu0 = Instant::now();
        // ADR 0025: each stretch of the frame thread, back to back from `now`
        let mut seg = [0f64; CPU_STAGES.len()];
        let mut mark = now;
        macro_rules! lap {
            ($name:literal) => {{
                let n = Instant::now();
                seg[CPU_STAGES.iter().position(|s| *s == $name).unwrap()] += (n - mark).as_secs_f64();
                mark = n;
            }};
        }
        // the sky's clock: the server's in play, else time= seconds into the world plus the run's own
        if self.run == "view" {
            self.world.time = self.time0;
        } else if self.run != "play" {
            self.world.time = self.time0 + t;
        } else if let Some((st, warp, at)) = self.clock {
            let target = st + (now - at).as_secs_f64() * warp;
            self.world.time += dt.min(0.1) * warp;
            self.world.time += (target - self.world.time) * (1.0 - (-dt / 0.3).exp());
        }
        let p = if self.run == "play" {
            // the player: mouse turns the look, keys make the input, the shared physics moves the box here in
            // the same fixed steps as on the server, whose position wins when the two drift apart
            let (mx, my) = std::mem::take(&mut self.mouse);
            // C held zooms 3x, eased over about 0.1 s; the mouse turns as much slower
            let want = if self.zoomed { ZOOM } else { 1.0 };
            self.zoom += (want - self.zoom) * (1.0 - (-dt / 0.04).exp());
            // zoomed, the mouse turns as much slower, times the settings' own factor for it
            let zs = 1.0 + (self.settings.get(S::ZoomSens) - 1.0) * (self.zoom - 1.0) / (ZOOM - 1.0);
            let sens = 0.0025 * self.settings.get(S::Sensitivity) * zs / self.zoom;
            let my = if self.settings.on(S::InvertY) { -my } else { my };
            // the look is the player's own and turns in the steps, so no frame change makes it mean another way
            self.turn -= mx * sens;
            // flight is creative's; the server holds to that too
            self.flying &= self.creative;
            if phys::free(&self.world, &self.me, self.flying) {
                // out here the mouse pitches the body itself, the head's tilt folded into it, and Q/E roll it
                self.tilt += std::mem::take(&mut self.pitch) - my * sens;
                let q = |a: Act| self.settings.held(&self.keys, a) as i32 as f64;
                self.roll += (q(Act::RollRight) - q(Act::RollLeft)) * ROLL * dt;
            } else {
                self.pitch = (self.pitch - my * sens).clamp(-1.55, 1.55);
            }
            let k = |a: Act| self.settings.held(&self.keys, a) as i32 as f32;
            let auto = a.get("autowalk", 0) == 1;
            let mut inp = Input {
                mv: [if auto { 1.0 } else { k(Act::Forward) - k(Act::Back) }, k(Act::Right) - k(Act::Left)],
                // (auto jump, the settings: a ledge one block high in the way of the walk is jumped)
                jump: auto || self.settings.held(&self.keys, Act::Jump) || (self.settings.on(S::AutoJump) && !self.flying && phys::ledge_ahead(&self.world, &self.me)),
                pitch: self.pitch as f32,
                fly: if self.flying { self.fly_speed as f32 } else { 0.0 },
                down: self.flying && self.settings.held(&self.keys, Act::Down),
                t: self.world.time,
                ..Default::default()
            };
            // nothing moves before the server says where this player starts
            self.debt = if self.spawned { self.debt + dt.min(0.1) } else { 0.0 };
            while self.debt >= phys::DT {
                self.prev = self.me.pos;
                inp.turn = std::mem::take(&mut self.turn) as f32;
                (inp.tilt, inp.roll) = (std::mem::take(&mut self.tilt) as f32, std::mem::take(&mut self.roll) as f32);
                if self.dead.is_some() {
                    // the server steps a dead player so (ADR 0040): the prediction keeps with it
                    inp = engine::vitals::still(inp);
                }
                let f0 = self.world.frame_at(self.me.pos);
                let in_air = !self.me.on_ground;
                phys::step(&self.world, &mut self.me, &inp, phys::DT);
                // the metres fallen since the ground was left (a flight or a swim is no fall)
                // (nothing is added in the step that changes the frame the place is told in: `prev` is still
                // in the old one)
                let sunk = if self.world.frame_at(self.me.pos) == f0 { -(self.me.pos - self.prev).dot(phys::up(&self.world, self.me.pos)) } else { 0.0 };
                // (a step of more than 3 m is a teleport; the water is asked only in the air)
                self.fell = if self.me.on_ground {
                    self.fell
                } else if self.flying || sunk > 3.0 || self.world.water_surface(self.me.pos).is_some() {
                    0.0
                } else {
                    self.fell + sunk.max(0.0)
                };
                if self.loaded_at.is_some() && self.me.on_ground && !self.flying {
                    // a footstep every STEP_M walked and one on landing, in the sound of the block underfoot;
                    // a fall of two blocks or more thumps as well, louder and lower the further it was
                    let land = in_air && self.fell > 0.3;
                    let thump = if in_air { audio::thump(self.fell) } else { None };
                    self.fell = 0.0;
                    self.walked += (self.me.pos - self.prev).length();
                    if land || self.walked >= STEP_M {
                        self.walked = 0.0;
                        let feet = self.me.pos;
                        let under = self.world.block(self.world.block_at(feet - phys::up(&self.world, feet) * 0.25));
                        if let Some(d) = BLOCKS.get(under).filter(|d| d.solid) {
                            self.audio.play(&format!("step.{}", d.sound), Some(feet), if land { 1.3 } else { 1.0 }, 1.0, true);
                            if let Some((gain, pitch)) = thump {
                                self.audio.play("land", Some(feet), gain, pitch, true);
                                // bits of the ground thrown up round the feet, more from higher (bug report
                                // 2026-11-11_12-57-17: "a bit louder thump and particles")
                                let c = self.world.block_at(feet + phys::up(&self.world, feet) * 0.1);
                                for _ in 0..(1.0 + gain * 3.0) as u32 {
                                    self.fx.play(crate::fx::id("chip"), &crate::fx::At::cell(&self.world, c, under));
                                }
                            }
                        }
                    }
                }
                let f1 = self.world.frame_at(self.me.pos);
                if f0 != f1 {
                    // a step into another frame: the drawn path and what is left of a correction go along, or the
                    // camera is drawn between two places in two frames for a frame
                    let (r, off) = self.world.place(f0, f1);
                    self.prev = r * (self.prev - self.world.nominal(f0)) + off;
                    self.smooth = r * self.smooth;
                    self.path.clear();
                }
                self.steps.push_back((self.next_step, inp));
                self.next_step += 1;
                if self.loaded_at.is_some() && keep {
                    // the third difference of the fixed-step path: jerk in m/s^3
                    self.path.push(self.me.pos);
                    if self.path.len() > 4 {
                        self.path.remove(0);
                    }
                    if let [a, b, c, d] = self.path[..] {
                        self.jerks.push((t, (d - c * 3.0 + b * 3.0 - a).length() / phys::DT.powi(3)));
                    }
                }
                self.debt -= phys::DT;
            }
            self.input = inp;
            // the look turns every frame; the position is drawn between the last two physics steps, plus what
            // is left of the last correction
            self.smooth *= (-dt / CORRECT_S).exp();
            let pos = self.prev.lerp(self.me.pos, self.debt / phys::DT) + self.smooth;
            // the body as last stepped, with the turn, tilt and roll the next step takes shown now; the head's
            // pitch over its horizon
            let up = self.me.body_up(phys::up(&self.world, pos));
            let fwd = DQuat::from_axis_angle(up, self.turn) * phys::tangent(self.me.fwd, up, self.me.fwd);
            let tilt = DQuat::from_axis_angle(fwd.cross(up).normalize(), self.tilt);
            let (up, fwd) = (tilt * up, tilt * fwd);
            let up = DQuat::from_axis_angle(fwd, self.roll) * up;
            let look = DQuat::from_axis_angle(fwd.cross(up), self.pitch) * fwd;
            Pose { pos: pos + up * phys::EYE, fwd: look, up }
        } else if let Some(v) = &self.view {
            v.pose(&self.world)
        } else {
            pose(&self.run, t, &self.world, speed, alt, warm, q, a)
        };
        if self.run == "play" && self.loaded_at.is_some() {
            self.build(&p, t);
        }
        let p = if self.home() == Menu::Title { self.title_view(t) } else { p };
        let vel = if dt > 0.0 && self.loaded_at.is_some() { (p.pos - self.last_pos) / dt } else { DVec3::ZERO };
        self.last_pos = p.pos;
        lap!("pose");

        // the server's deltas: edits land in the world and remesh their chunks here, never directly
        loop {
            match self.link.try_recv() {
                Recv::Msg(ToClient::Edits(v)) => {
                    let mut heard = 0;
                    for (b, m) in v {
                        // the cell's delta: its effect and, for up to three cells of an edit, its sound (another
                        // player's digging and building; this player's own were seen and heard at the click)
                        let hear = heard < 3 && self.loaded_at.is_some();
                        let (shows, was) = fx::server_edit(&mut self.fx, &mut self.pred, &mut self.world, b, m, hear);
                        if shows {
                            self.streamer.touch_block(&self.world, b);
                            if let Some(was) = was.filter(|_| hear) {
                                heard += self.heard(b, was, m, false) as u32;
                            }
                        }
                        self.edits_applied += 1;
                        if measuring {
                            self.samples.edits += 1;
                        }
                    }
                }
                Recv::Msg(ToClient::Done { seq, .. }) => {
                    for b in self.pred.done(&mut self.world, seq) {
                        self.streamer.touch_block(&self.world, b);
                    }
                }
                Recv::Msg(ToClient::Players(ps)) => self.players = ps.len(),
                Recv::Msg(ToClient::Kinds(k)) => {
                    // shortcut: the creatures file is read here, once on join (small); the server's index wins
                    let rows = engine::creatures::rows(&std::fs::read_to_string("data/creatures.toml").unwrap_or_default()).unwrap_or_default();
                    self.kinds = k.iter().map(|n| rows.iter().find(|c| &c.name == n).map(|c| (c.model.clone(), c.voice.clone(), c.scale))).collect();
                }
                Recv::Msg(ToClient::Animals { seen, gone }) => {
                    for id in gone {
                        self.animals.remove(&id);
                    }
                    let now = Instant::now();
                    for a in seen {
                        let was = self.animals.get(&a.id).map(|(o, _, _)| *o);
                        // a new call plays its voice where it stands
                        if was.is_some_and(|o| o.call != a.call) {
                            if let Some(Some((_, voice, _))) = self.kinds.get(a.kind as usize).filter(|_| self.world.frame_at(self.me.pos) == 0) {
                                if !voice.is_empty() {
                                    self.audio.play(voice, Some(DVec3::from_array(a.pos)), 1.0, 1.0, false);
                                }
                            }
                        }
                        let from = self.animal_at(a.id, now).unwrap_or(DVec3::from_array(a.pos));
                        self.animals.insert(a.id, (a, from, now));
                    }
                }
                // another player's dig (ADR 0034): its cracks run on from this
                Recv::Msg(ToClient::Digging { id, at }) => {
                    let now = self.start.elapsed().as_secs_f64();
                    self.digs.retain(|d| d.0 != id);
                    if let Some((s, done, rate)) = at {
                        self.digs.push((id, s, done, rate, now, now - HIT_S));
                    }
                }
                Recv::Msg(ToClient::You { step, me, time, warp }) if self.run == "play" => {
                    // the first, or a jump (/time set): straight there
                    if self.clock.is_none_or(|(_, w, _)| (time - self.world.time).abs() > 2.0 * w.max(warp) + 1.0) {
                        self.world.time = time;
                    }
                    self.clock = Some((time, warp, Instant::now()));
                    self.reconcile(step, me, t)
                }
                Recv::Msg(ToClient::You { .. }) => {}
                Recv::Msg(ToClient::Inventory { creative, counts, water }) => (self.creative, self.inv, self.bucket_water) = (creative, counts, water),
                Recv::Msg(ToClient::Vitals { health, hunger, dead }) => self.vitals_in(health, hunger, dead),
                Recv::Msg(ToClient::Water(v)) => {
                    let mut ts = Vec::with_capacity(v.len());
                    for (t, cols) in &v {
                        let was = self.world.water.wet(*t);
                        self.world.water.set_tile(*t, cols);
                        if was != self.world.water.wet(*t) {
                            // the chunks' sea leaves this tile out, or covers it again
                            self.streamer.touch_tile(&self.world, *t);
                        }
                        ts.push(*t);
                    }
                    self.water.update(&self.world, &ts, self.start.elapsed().as_secs_f32());
                }
                Recv::Msg(ToClient::Air(h)) => self.air = Some(h),
                Recv::Msg(ToClient::Welcome { .. } | ToClient::Map { .. }) => {}
                Recv::Msg(ToClient::Chat(text)) => {
                    for line in text.lines() {
                        eprintln!("[chat] {line}");
                        self.chat_line(line.to_string());
                    }
                }
                Recv::Empty => break,
                Recv::Closed => {
                    eprintln!("[technoape] the server closed the connection");
                    self.finish();
                    return;
                }
            }
        }
        if (now - self.last_move).as_secs_f64() >= 1.0 / TICK_HZ {
            if self.run == "play" {
                let inputs: Vec<Input> = self.steps.iter().filter(|s| s.0 >= self.unsent).map(|s| s.1).collect();
                if !inputs.is_empty() {
                    self.link.send(ToServer::Steps { first: self.unsent, inputs });
                    self.unsent = self.next_step;
                }
            } else {
                // the benchmark runs fly: the camera position is sent as the player's position
                self.link.send(ToServer::Move { pos: p.pos.to_array(), fwd: p.fwd.as_vec3().to_array(), pitch: 0.0 });
            }
            self.last_move = now;
        }
        lap!("msgs");

        // edits: 2000 toggles/s within 10 m of the feet, sent to the server; they count when its delta is back
        if self.run == "edit" && self.loaded_at.is_some() {
            let rate: f64 = self.args.get("edit_rate", 2000.0);
            let target = (t * rate) as u64;
            let feet = p.pos - p.up * 1.7;
            while self.edits_done < target {
                let mut r = || {
                    self.rng ^= self.rng << 13;
                    self.rng ^= self.rng >> 7;
                    self.rng ^= self.rng << 17;
                    (self.rng >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
                };
                let o = loop {
                    let o = DVec3::new(r(), r(), r());
                    if o.length_squared() <= 1.0 {
                        break o * 10.0;
                    }
                };
                let b = self.world.block_at(feet + o);
                if b.pos.z < 0 {
                    continue; // below layer 0 nothing is stored
                }
                let m = if self.world.block(b) == AIR { BLOCKS.id("stone") } else { AIR };
                let me = &self.me;
                let (seq, cells) = self.pred.edit(&mut self.world, &Shape::one(b), m, |w, c| phys::holds(w, me, c));
                for c in cells {
                    self.streamer.touch_block(&self.world, c);
                }
                self.link.send(ToServer::Edit { seq, shape: Shape::one(b), material: m });
                self.edits_done += 1;
            }
        }
        // fxplay=name[:n] (ADR 0050): lasting effects on the ground ahead, once loaded
        if self.fx.on && !self.fx.placed && self.loaded_at.is_some() {
            self.fx.scene(&self.world, (p.pos, p.fwd), &self.args.s("fxplay", ""));
        }
        // run=fx (ADR 0031): bursts of the break on the ground 2-14 m ahead until fx_n particles are alive
        if self.run == "fx" && self.loaded_at.is_some() {
            let want: usize = self.args.get("fx_n", crate::fx::CAP[1]);
            let ahead = (p.fwd - p.up * p.fwd.dot(p.up)).normalize_or(DVec3::X);
            let side = p.up.cross(ahead);
            let brk = crate::fx::id("break");
            for _ in 0..64 {
                if self.fx.live() + 12 > want {
                    break;
                }
                let mut r = || {
                    self.rng ^= self.rng << 13;
                    self.rng ^= self.rng >> 7;
                    self.rng ^= self.rng << 17;
                    (self.rng >> 11) as f64 / (1u64 << 53) as f64
                };
                let d = (p.pos + ahead * (2.0 + 12.0 * r()) + side * (r() * 2.0 - 1.0) * 8.0 - self.world.bodies[0].centre).normalize();
                // the air cell over the ground, which lands them on its top; its look is the ground's
                let c = self.world.air_over_ground(0, d);
                let under = self.world.block(engine::world::Block { pos: c.pos - IVec3::Z, ..c });
                self.fx.play(brk, &crate::fx::At::cell(&self.world, c, under));
            }
        }
        // fxedit=: a cell changed as a click would change it, with its effects
        while self.fx_edits.last().is_some_and(|e| self.loaded_at.is_some() && e.0 <= t) {
            let (_, b, m) = self.fx_edits.pop().unwrap();
            let was = self.world.block(b);
            self.world.set_block(b, m);
            fx::changed(&mut self.fx, &self.world, b, was, m);
            if self.fx.on && BLOCKS.solid(m) && !BLOCKS.solid(was) {
                self.grow(b, m);
            } else {
                self.streamer.touch_block(&self.world, b);
            }
        }
        // a placed block that grew into place: its mesh may show now, and the grow ends once it does
        if !self.growing.is_empty() {
            let now = self.start.elapsed().as_secs_f64();
            let (world, streamer, fxs) = (&self.world, &mut self.streamer, &mut self.fx);
            self.growing.retain_mut(|(due, b, m, tag, touched)| {
                if !*touched && now >= *due {
                    streamer.touch_block(world, *b);
                    *touched = true;
                }
                let over = world.block(*b) != *m || (*touched && streamer.shows(world, *b)) || now > *due - crate::app::GROW_S + 2.0;
                if over {
                    fxs.end(*tag);
                    if !*touched {
                        streamer.touch_block(world, *b);
                    }
                }
                !over
            });
        }
        lap!("edits");

        let gpu = self.gpu.as_ref().unwrap();
        let mut st = FrameStats::default();
        let budget = (upload_mb * 1e6) as usize;
        let mut ups = Vec::new();
        self.streamer.apply(budget, &mut st, |off, q| {
            gpu.upload(off, q);
            ups.push(off);
        });
        let rt = self.rt.as_mut().unwrap();
        // a released mesh's BLAS goes the same frame (ADR 0025, G)
        for off in self.streamer.released.drain(..) {
            rt.forget(off);
        }
        for off in ups {
            rt.forget(off);
            // new quads at an offset: its table entry is for another chunk now
            if let Some((i, _)) = self.slots.at.remove(&off) {
                self.slots.free.push(i);
            }
        }
        if !self.clicks.is_empty() {
            let (world, streamer, frame) = (&self.world, &self.streamer, self.streamer.frame);
            let mut done = Vec::new();
            self.clicks.retain(|(f, at, cells)| {
                let on = cells.iter().all(|b| streamer.shows(world, *b));
                if on {
                    done.push((frame - f, at.elapsed().as_secs_f64() * 1000.0));
                }
                !on
            });
            self.shown.extend(done);
        }
        lap!("apply");

        let aspect = w() as f32 / h() as f32;
        let proj = glam::camera::rh::proj::directx::perspective_infinite_reverse((self.fov() as f32).to_radians(), aspect, 0.1);
        // the local up keeps the horizon level; a fixed axis only when the look is (almost exactly) straight up or
        // down. Swapping to it already at 78 degrees (0.98) rolled the view as the player walked, looking down.
        let up = if p.fwd.cross(p.up).length() < 1e-4 { DVec3::Z } else { p.up };
        self.cam = (p.pos, p.fwd, up);
        let viewm = glam::camera::rh::view::look_to_mat4(Vec3::ZERO, p.fwd.as_vec3(), up.as_vec3());
        let vp = proj * viewm;
        let view = View::new(p.pos, frustum_planes(vp), &self.world);
        let mut draws: Vec<DrawChunk> = Vec::new();
        let mut reqs = Vec::new();
        self.streamer.frame += 1;
        lap!("table");
        let ts0 = Instant::now();
        self.streamer.select(&self.world, &view, true, 1.0, &mut draws, &mut reqs, &mut st);
        let pf = self.streamer.opts.prefetch_s;
        // the look-ahead only requests chunks, so every `prefetch_every`th frame is enough
        let every: u64 = self.args.get("prefetch_every", 4);
        if pf > 0.0 && vel.length() > 1.0 {
            // between refreshes the last look-ahead requests stay in the queue, or the workers fill up with
            // low-priority hidden chunks and the look-ahead arrives late
            let mut ahead_reqs = std::mem::take(&mut self.streamer.ahead_reqs);
            if self.streamer.frame % every == 0 {
                ahead_reqs.clear();
                let ahead = View::new(p.pos + vel * pf, view.planes, &self.world);
                let mut sink = Vec::new();
                let mut st2 = FrameStats::default();
                self.streamer.select(&self.world, &ahead, false, 1.5, &mut sink, &mut ahead_reqs, &mut st2);
            }
            reqs.extend_from_slice(&ahead_reqs);
            self.streamer.ahead_reqs = ahead_reqs;
        }
        lap!("select");
        // the Leaves setting (ADR 0069), or a run's leaves=fancy|fast: level 0 meshed again when it changes
        let fancy = self.args.0.get("leaves").map_or(self.settings.at(S::Leaves) == 1, |v| v == "fancy");
        self.streamer.set_cutout(fancy);
        self.streamer.dispatch(&self.world, &mut reqs);
        let sel = ts0.elapsed().as_secs_f64();
        lap!("dispatch");
        if self.streamer.frame % 30 == 0 {
            self.streamer.evict(120);
            let rt = self.rt.as_mut().unwrap();
            for off in self.streamer.released.drain(..) {
                rt.forget(off);
            }
        }

        // the indirect args per frame; the chunk table's entries only for meshes new to it (ADR 0025, B)
        let mut table: Vec<(u32, ChunkGpu)> = Vec::new();
        let mut args: Vec<(f32, DrawArgs)> = Vec::with_capacity(draws.len());
        let mut sea_args: Vec<DrawArgs> = Vec::new();
        let mut cut_args: Vec<(f32, DrawArgs)> = Vec::new();
        let mut nquads = 0u64;
        let mut insts: Vec<rt::Inst> = Vec::with_capacity(draws.len());
        let frame_no = self.streamer.frame;
        for d in draws.iter().take(MAX_CHUNKS) {
            let k = d.key;
            // the lattice its quads were merged in (ADR 0025 I2), as log2 at bit 16
            let lat = if self.streamer.opts.greedy { self.world.bodies[k.body as usize].lattice(k.level) } else { 1 };
            let info = k.face as u32 | (k.level as u32) << 3 | (k.body as u32) << 8 | lat.trailing_zeros() << 16;
            let (ci, new) = self.slots.get(d.off, frame_no);
            assert!((ci as usize) < MAX_CHUNKS, "chunk table full");
            if new {
                table.push((ci, ChunkGpu { base: k.base().into(), info, off: d.off, opq: d.len - d.water - d.cut, _p: [0; 2] }));
            }
            let land = d.len - d.water;
            if land > 0 {
                insts.push(rt::Inst { off: d.off, land, cut: d.cut, base: k.base().into(), info, body: k.body, ci });
            }
            if d.hidden {
                continue;
            }
            // the opaque quads, then the cutout blocks' through the pipeline that discards (ADR 0069)
            let opq = land - d.cut;
            if opq > 0 {
                args.push((d.dist, DrawArgs { vertex_count: opq * 6, instance_count: 1, first_vertex: d.off * 6, first_instance: ci }));
            }
            if d.cut > 0 {
                cut_args.push((d.dist, DrawArgs { vertex_count: d.cut * 6, instance_count: 1, first_vertex: (d.off + opq) * 6, first_instance: ci }));
            }
            if d.water > 0 {
                // vs_sea draws each sea quad in SEA_SUB^2 pieces, one instance each: instance = chunk * pieces + piece
                let pieces = SEA_SUB * SEA_SUB;
                sea_args.push(DrawArgs { vertex_count: d.water * 6, instance_count: pieces, first_vertex: (d.off + land) * 6, first_instance: ci * pieces });
            }
            nquads += d.len as u64;
        }
        // near to far, so the depth test drops what is hidden before its fragments trace their rays (ADR 0025, D)
        args.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
        let args: Vec<DrawArgs> = args.into_iter().map(|a| a.1).collect();
        cut_args.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
        let cut_args: Vec<DrawArgs> = cut_args.into_iter().map(|a| a.1).collect();
        // the water mesh when it changed, or the tide moved the sea in its tiles
        self.water.retide(&self.world, self.start.elapsed().as_secs_f32());
        let n = self.water.all.len().min(MAX_WQUADS);
        if std::mem::take(&mut self.water.changed) {
            // shortcut: past the cap the list is cut in map order, a far tile can go; when floods outgrow it
            if n < self.water.all.len() && !std::mem::replace(&mut self.water.cut_told, true) {
                eprintln!("[technoape] water: {} quads, the buffer holds {MAX_WQUADS}: the rest are not drawn", self.water.all.len());
            }
            gpu.queue.write_buffer(&gpu.wquads, 0, bytemuck::cast_slice(&self.water.all[..n]));
        }
        for r in self.water.dirty.drain(..) {
            let r = r.start.min(n)..r.end.min(n);
            if !r.is_empty() {
                gpu.queue.write_buffer(&gpu.wquads, (r.start * std::mem::size_of::<water::WQuad>()) as u64, bytemuck::cast_slice(&self.water.all[r]));
            }
        }
        self.wquad_n = n as u32;
        lap!("table");
        // the effects (ADR 0031): the water's spray, every particle moved on, the pool to the GPU
        self.fx_n = 0;
        if self.fx.on {
            self.fx.follow(self.settings.at(S::Particles));
            self.water.spray(&self.world, dt as f32, &mut self.fx);
            self.fx.step(&self.world, (dt as f32).min(0.1));
            self.cracks(self.start.elapsed().as_secs_f64());
            let gpu = self.gpu.as_ref().unwrap();
            let d = self.fx.drawn();
            let k = d.len().min(MAX_FX);
            if k > 0 {
                gpu.queue.write_buffer(&gpu.fx_buf, 0, bytemuck::cast_slice(&d[..k]));
            }
            self.fx_n = k as u32;
            if measuring {
                self.samples.fx_live += k as u64;
            }
        }
        // taken again: the cracks above needed all of self
        let gpu = self.gpu.as_ref().unwrap();
        lap!("fx");
        // the sun where the sky has it, seen from the camera's frame (ADR 0022)
        let cf = self.world.frame_at(p.pos);
        let sun = (self.world.from_inertial(cf, self.world.sky.sun(self.world.time)) - p.pos).normalize().as_vec3();
        let clock = self.start.elapsed().as_secs_f32();
        let (sky_m, wt) = (&self.world.sky, self.world.time);
        let sun_at = self.world.from_inertial(cf, sky_m.sun(wt)) - p.pos;
        // the moon's light: sunlight by its phase seen from the planet, less what the planet hides of the sun
        let (sp, mp, pp) = (sky_m.sun(wt), sky_m.moon(wt).centre, sky_m.planet(wt).centre);
        let phase = 0.5 * (1.0 + (sp - mp).normalize().dot((pp - mp).normalize()));
        let moon = MOON_LIGHT * (phase * (1.0 - sky_m.covered(wt, mp, pp, sky_m.planet_radius))) as f32;
        let sun_deg = self.world.up_at(p.pos).dot(sun_at.normalize()).clamp(-1.0, 1.0).asin().to_degrees();
        self.drawn = Drawn {
            chunks: args.len(),
            quads: nquads,
            rt_pending: self.drawn.rt_pending,
            sun_deg,
            moon_lit: phase,
            streaming: (st.missing, st.holes, st.requests, self.streamer.inflight),
        };
        // the sky here: the sun's height over this horizon and what the moon hides of it, and the air thinning
        let day = {
            let h = self.world.up_at(p.pos).dot(sun_at.normalize());
            let x = ((h + 0.2) / 0.35).clamp(0.0, 1.0);
            (x * x * (3.0 - 2.0 * x) * (1.0 - sky_m.eclipse(wt, self.world.to_inertial(p.pos)))) as f32
        };
        // the bodies with air (data/bodies.toml): the nearest one's sky
        let x = self.world.to_inertial(p.pos);
        let over_air = (0..self.world.bodies.len())
            .filter(|&i| self.world.bodies[i].air)
            .map(|i| (x - self.world.pose(i, wt).centre).length() - self.world.bodies[i].radius)
            .fold(f64::MAX, f64::min);
        let air = (1.0 - over_air.max(0.0) / 1500.0).clamp(0.0, 1.0) as f32;
        // what the head hears (docs/design/sound.md): where it is, how much the air here carries (the
        // settings' "Sound in space"), dull under water; the volumes; and the wind, with height and speed
        self.audio.ear = [p.pos, p.fwd.cross(p.up).normalize_or_zero(), p.up, p.fwd];
        self.audio.carry = [air, air.max(0.4), 1.0][self.settings.at(S::SpaceSound).min(2)];
        self.audio.dull = if self.world.water_surface(p.pos).is_some() { 0.8 } else { 0.0 };
        self.audio.listen(&|q| BLOCKS.get(self.world.block(self.world.block_at(q))).is_some_and(|d| d.solid && !d.soft), &|q| {
            self.world.bodies[self.world.nearest(q)].over_top(q)
        });
        let vol = |s: S| (self.settings.get(s) / 100.0) as f32;
        let listening = self.focused || !self.onscreen || !self.settings.on(S::MuteBack);
        self.audio.levels(if listening { vol(S::Volume) } else { 0.0 }, [vol(S::VolWorld), vol(S::VolSteps), vol(S::VolUi), vol(S::VolMusic)]);
        // (the title has its music and no wind: bug reports 2026-11-11_12-53-31 and 12-55-41)
        let title = self.home() == Menu::Title;
        let wind = if title { 0.0 } else { (0.05 + over_air.clamp(0.0, 400.0) / 400.0 * 0.3 + self.me.vel.length() / 50.0).min(1.0) as f32 * air };
        self.audio.hold("wind", wind);
        self.audio.hold("music.menu", if title { 1.0 } else { 0.0 });
        let sky_c = Vec3::new(0.45, 0.65, 0.95) * day * air + Vec3::new(0.01, 0.012, 0.025);
        // the bounce light on or off (off: a flat fill from the sky), and the screen's brightness
        let fill = sky_c * 0.35 + Vec3::splat(0.01);
        // the air: the horizon's colour (as `sky_colour` in shader.wgsl makes it) for far ground to fade
        // into, a third of the way by 1 km; and the bounce drawn half again as strong in the air, so a
        // shadow under the open sky keeps its colour while a cave stays dark
        // (with the night's glow at the horizon, NIGHT_GLOW in shader.wgsl `sky_colour`)
        let x = (day * air / 0.15).clamp(0.0, 1.0);
        let night = 1.0 - x * x * (3.0 - 2.0 * x);
        let haze = sky_c + Vec3::new(0.55, 0.60, 0.62) * day * air * 0.45 + Vec3::new(0.02, 0.03, 0.07) * air * night * 1.5;
        let tune = (
            self.settings.on(S::Bounce),
            [fill.x, fill.y, fill.z, self.settings.get(S::Brightness) as f32],
            [haze.x, haze.y, haze.z, air / 2500.0],
            [0.5 * air, 0.0, 0.0, cf as f32],
        );
        // the sunlight through the air: the longer its way through it (the lower the sun), the more blue and
        // green it loses; at noon as it was, in space white (shader.wgsl sun_tint is the same)
        let sun_h = self.world.up_at(p.pos).dot(sun_at.normalize()) as f32;
        let tint = {
            let k = Vec3::new(0.02, 0.06, 0.16);
            let mass = (1.0 / (sun_h.max(0.0) + 0.035)).min(28.0);
            Vec3::ONE.lerp(Vec3::new((-k.x * mass).exp(), (-k.y * mass).exp(), (-k.z * mass).exp()) / Vec3::new((-k.x).exp(), (-k.y).exp(), (-k.z).exp()), air)
        };
        let sun_light = Vec3::new(0.65, 0.97 * 0.65, 0.9 * 0.65).lerp(Vec3::splat(0.65), 1.0 - air) * tint;
        let to_stars = glam::DMat3::from_quat(self.world.pose(cf, wt).rot).as_mat3();
        // the planet's tide (ADR 0067) in layers: the sea's shader lays its quads at it
        let tide = self.world.tide();
        let b0 = &self.world.bodies[0];
        let per_m = b0.sea.map_or(0.0, |s| 1.0 / (b0.layer_r(s) * b0.lnq));
        let pull = |d: DVec3, a: f64| [d.x as f32, d.y as f32, d.z as f32, (1.5 * a * per_m) as f32];
        let mut g = Globals {
            tide: [pull(tide.luna, engine::sky::TIDE_LUNA), pull(tide.sun, engine::sky::TIDE_SUN)],
            view_proj: vp.to_cols_array_2d(),
            sun: [sun.x, sun.y, sun.z, clock],
            bodies: bytemuck::Zeroable::zeroed(),
            inv_vp: vp.inverse().to_cols_array_2d(),
            sun_pos: [sun_at.x as f32, sun_at.y as f32, sun_at.z as f32, self.world.sky.sun_radius as f32],
            sun_col: [sun_light.x, sun_light.y, sun_light.z, cf.min(255) as f32],
            moon_col: [moon.x, moon.y, moon.z, 1.0],
            sky: [sky_c.x, sky_c.y, sky_c.z, day * air],
            n_lamps: [0; 4],
            opt: [self.gfx_bits(), (self.settings.get(S::Brightness) * 1000.0).round() as u32, w(), h()],
            stars: [
                [to_stars.x_axis.x, to_stars.x_axis.y, to_stars.x_axis.z, air],
                [to_stars.y_axis.x, to_stars.y_axis.y, to_stars.y_axis.z, sun_h],
                [to_stars.z_axis.x, to_stars.z_axis.y, to_stars.z_axis.z, 0.0],
            ],
        };
        // per body: the camera in its coordinates, and its frame in the camera's
        let mut cams = [(DVec3::ZERO, glam::DMat3::IDENTITY); MAX_BODIES];
        for (bi, (b, o)) in self.world.bodies.iter().zip(g.bodies.iter_mut()).enumerate() {
            // the camera in the body's frame, relative to its centre, in two floats per axis: one float is
            // 0.1 mm coarse at 1 km; and the body's frame turned into the camera's
            let (r, off) = self.world.place(cf, bi);
            let c = r * (p.pos - self.world.nominal(cf)) + off - b.centre;
            let m = glam::DMat3::from_quat(self.world.place(bi, cf).0).as_mat3();
            let col = |v: Vec3| [v.x, v.y, v.z, 0.0];
            let hi = c.as_vec3();
            let lo = (c - hi.as_dvec3()).as_vec3();
            *o = BodyGpu {
                cam_hi: hi.into(),
                r0: b.r0 as f32,
                cam_lo: lo.into(),
                lnq: b.lnq as f32,
                theta: (std::f64::consts::FRAC_PI_2 / b.n as f64) as f32,
                half_n: b.n as f32 * 0.5,
                // shortcut: only the planet (body 0) has seasons; a second planet needs its own year in `Sky`
                _p: [b.sea.map_or(-1.0, |s| (s + if bi == 0 { tide.base * per_m } else { 0.0 }) as f32), if bi == 0 { self.season_year(engine::sky::season_at(DVec3::Z, self.world.time).0) as f32 } else { -1.0 }],
                rot: [col(m.x_axis), col(m.y_axis), col(m.z_axis)],
                ctr: { let v = (m.as_dmat3() * -c).as_vec3(); [v.x, v.y, v.z, b.radius as f32] },
            };
            cams[bi] = (c + b.centre, m.as_dmat3());
        }
        gpu.take_textures();
        // the models (ADR 0047): the watcher's newest, then this frame's pieces (timed under "table")
        self.models.take(&gpu.queue, &gpu.mquads);
        let hand = (self.run == "play" && !self.hammer && !self.bucket && !self.f3 && self.home() != Menu::Title)
            .then(|| self.slot_block(self.slot))
            .flatten()
            .and_then(|m| BLOCKS.get(m))
            .map(|d| d.name.clone());
        let now = Instant::now();
        let herd: Vec<crate::models::Beast> = self
            .animals
            .iter()
            .filter_map(|(id, (a, _, _))| {
                let (model, _, scale) = self.kinds.get(a.kind as usize)?.as_ref()?;
                Some(crate::models::Beast { id: *id, model: model.clone(), act: a.act, pos: self.animal_at(*id, now)?, fwd: DVec3::from_array(a.fwd.map(f64::from)), scale: *scale })
            })
            .collect();
        self.models.frame(&self.world, &cams, cf, (p.pos, p.fwd, up), self.view.as_ref().map_or(self.start.elapsed().as_secs_f64(), |v| v.t), hand.as_deref(), &herd, &mut self.fx, &mut self.audio);
        self.models.upload(&gpu.queue, &gpu.mpieces);
        // the lamps: gathered again when they change or every 10 frames, placed and gridded every frame
        let ec = self.world.lights_rev;
        if self.lamps_at.1 != ec || self.streamer.frame >= self.lamps_at.0 + 10 {
            let c: Vec<DVec3> = cams.iter().map(|c| c.0).collect();
            self.lamps = gather_lamps(&self.world, &c[..self.world.bodies.len()]);
            self.lamps.truncate([16, 64, MAX_LAMPS][self.settings.at(S::Lamps)]);
            self.lamps_at = (self.streamer.frame, ec);
        }
        // the lamps of the world, then the glowing lasting effects' and the models' states' (ADR 0050)
        let near: Vec<DVec3> = cams.iter().map(|c| c.0).collect();
        let cap = [16, 64, MAX_LAMPS][self.settings.at(S::Lamps)];
        let lamps: Vec<LampGpu> = crate::gpu::cap_lamps(self.lamps.iter().chain(self.fx.lamps()).chain(self.models.lamps()), &near, cap)
            .iter()
            .map(|&(bi, x, l)| {
                let (cam, m) = cams[bi as usize];
                let v = (m * (x - cam)).as_vec3();
                LampGpu { pos: [v.x, v.y, v.z, bi as f32], col: [l[0], l[1], l[2], 0.0] }
            })
            .collect();
        g.n_lamps[0] = lamps.len() as u32;
        g.n_lamps[1] = self.gi.as_ref().unwrap().frame() as u32;
        if !lamps.is_empty() {
            gpu.queue.write_buffer(&gpu.lamps, 0, bytemuck::cast_slice(&lamps));
            gpu.queue.write_buffer(&gpu.lamp_cells, 0, bytemuck::cast_slice(&lamp_grid(&lamps)));
        }
        [g.n_lamps[2], g.n_lamps[3]] = eclipse_masks(&g);
        gpu.queue.write_buffer(&gpu.globals, 0, bytemuck::bytes_of(&g));
        self.slots.sweep(frame_no);
        for (ci, c) in &table {
            gpu.queue.write_buffer(&gpu.chunks, (*ci as usize * std::mem::size_of::<ChunkGpu>()) as u64, bytemuck::bytes_of(c));
        }
        if !args.is_empty() {
            gpu.queue.write_buffer(&gpu.indirect, 0, bytemuck::cast_slice(&args));
        }
        if !sea_args.is_empty() {
            gpu.queue.write_buffer(&gpu.indirect, (MAX_CHUNKS * 16) as u64, bytemuck::cast_slice(&sea_args));
        }
        if !cut_args.is_empty() {
            gpu.queue.write_buffer(&gpu.indirect, (MAX_CHUNKS * 32) as u64, bytemuck::cast_slice(&cut_args));
        }

        let mut sky = wgpu::Color { r: sky_c.x as f64, g: sky_c.y as f64, b: sky_c.z as f64, a: 1.0 };
        if self.shot.is_some() {
            // the crack detector's colour: no block or sky has it, so every such pixel is a hole or sky
            sky = wgpu::Color { r: 1.0, g: 0.0, b: 1.0, a: 1.0 };
        }
        lap!("table");

        let frame = match gpu.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            other => {
                eprintln!("[technoape] surface: {:?}", std::mem::discriminant(&other));
                gpu.surface.configure(&gpu.device, &gpu.config);
                return;
            }
        };
        let cpu_a = cpu0.elapsed().as_secs_f64();
        let acq = cpu_a;
        lap!("acquire");
        let cpu1 = Instant::now();
        let fview = frame.texture.create_view(&Default::default());
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        // timestamp slot: read a finished one, then reuse
        let mut ts_use = None;
        if let Some((_, _, ring)) = &gpu.ts {
            let i = self.ts_slot % TS_RING;
            let (rb, flag, pending) = &ring[i];
            if *pending {
                if flag.load(Ordering::Acquire) {
                    let data = rb.slice(..).get_mapped_range().unwrap();
                    let v: Vec<u64> = bytemuck::cast_slice(&data).to_vec();
                    drop(data);
                    rb.unmap();
                    let ms = |a: u64, b: u64| b.wrapping_sub(a) as f64 * gpu.ts_period as f64 / 1e6;
                    // the old gpu_ms (G-buffer start to screen end) stays comparable; the stages are new
                    let old = ms(v[1], v[7]);
                    let f = self.ts_frames[i];
                    let gap = match self.gpu_end {
                        Some((pf, end)) if pf + 1 == f => ms(end, v[0]),
                        _ => f64::NAN,
                    };
                    self.gpu_end = Some((f, v[7]));
                    let st: Vec<f64> = (0..7).map(|k| ms(v[k], v[k + 1])).chain([ms(v[0], v[7]), gap]).collect();
                    if old < 1000.0 {
                        if measuring {
                            self.samples.gpu_ms.push(old as f32);
                            let g = &mut self.samples.gpu_st;
                            g.resize(GPU_STAGES.len(), Vec::new());
                            for (k, x) in st.iter().enumerate() {
                                if x.is_finite() {
                                    g[k].push(*x as f32);
                                }
                            }
                        }
                        if let Some(s) = self.samples.spikes.iter_mut().find(|s| s.0 == f) {
                            s.2 = Some(GPU_STAGES.iter().zip(&st).map(|(n, x)| format!("\"{n}\":{:.2}", if x.is_finite() { *x } else { -1.0 })).collect::<Vec<_>>().join(","));
                        }
                    }
                    self.gpu_last = old;
                    ts_use = Some(i);
                }
            } else {
                ts_use = Some(i);
            }
        }
        let stamp = |enc: &mut wgpu::CommandEncoder, k: usize| {
            if let (Some(i), Some((qs, _, _))) = (ts_use, &gpu.ts) {
                enc.write_timestamp(qs, (i * TS_N + k) as u32);
            }
        };
        stamp(&mut enc, 0);
        let rt = self.rt.as_mut().unwrap();
        rt.record(&gpu.device, &gpu.queue, &mut enc, &insts);
        let rt_pending = rt.pending;
        self.drawn.rt_pending = rt_pending;
        stamp(&mut enc, 1);
        lap!("rt");
        {
            // the G-buffer: the sky and the terrain; the bounce light and the composite come after
            let gi = self.gi.as_mut().unwrap();
            let [tdirect, talbedo, tgbuf] = gi.targets();
            // the Studio's view with no background: a plain grey where the sky and the land would be
            const BARE: wgpu::Color = wgpu::Color { r: 0.3, g: 0.3, b: 0.32, a: 1.0 };
            let bare = self.view.as_ref().is_some_and(|v| v.bare);
            let target = |view, clear| {
                Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(clear), store: wgpu::StoreOp::Store },
                })
            };
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gbuffer"),
                color_attachments: &[target(tdirect, if bare { BARE } else { sky }), target(talbedo, wgpu::Color::TRANSPARENT), target(tgbuf, wgpu::Color::TRANSPARENT)],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &gpu.depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_bind_group(0, &gpu.bind, &[]);
            // the crack detector's shots keep the plain clear colour; so does the Studio's view with no background
            if self.shot.is_none() && !bare {
                rp.set_pipeline(&gpu.sky_pipeline);
                rp.draw(0..3, 0..1);
            }
            rp.set_pipeline(&gpu.pipeline);
            if bare {
                // (the land is still there for the light: only its picture is left out)
            } else if self.args.get("mdi", 1) == 1 {
                rp.multi_draw_indirect(&gpu.indirect, 0, args.len() as u32);
                if !cut_args.is_empty() {
                    rp.set_pipeline(&gpu.cut_pipeline);
                    rp.multi_draw_indirect(&gpu.indirect, (MAX_CHUNKS * 32) as u64, cut_args.len() as u32);
                }
            } else {
                for d in &args {
                    rp.draw(d.first_vertex..d.first_vertex + d.vertex_count, d.first_instance..d.first_instance + 1);
                }
                rp.set_pipeline(&gpu.cut_pipeline);
                for d in &cut_args {
                    rp.draw(d.first_vertex..d.first_vertex + d.vertex_count, d.first_instance..d.first_instance + 1);
                }
            }
            // the models (ADR 0047): one draw per part, one instance per piece
            if !self.models.draws.is_empty() {
                rp.set_pipeline(&gpu.model_pipeline);
                for d in &self.models.draws {
                    rp.draw(0..d.quads * 6, d.first..d.first + d.count);
                }
            }
            drop(rp);
            stamp(&mut enc, 2);
            // the effects (ADR 0031): over the terrain into the same G-buffer, in a pass of their own to time it
            if self.fx_n > 0 {
                let load = |view| {
                    Some(wgpu::RenderPassColorAttachment {
                        view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                    })
                };
                let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("fx"),
                    color_attachments: &[load(tdirect), load(talbedo), load(tgbuf)],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &gpu.depth,
                        depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                rp.set_bind_group(0, &gpu.bind, &[]);
                rp.set_pipeline(&gpu.fx_pipeline);
                rp.draw(0..36, 0..self.fx_n);
            }
            stamp(&mut enc, 3);
            gi.record(&gpu.queue, &mut enc, vp, p.pos, cf, self.shot.is_some(), tune, &mut |e, k| stamp(e, 4 + k));
            stamp(&mut enc, 6);
            // the screen: direct and bounced light, then the water and the overlays over it
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("screen"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &fview,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &gpu.depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Discard }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            gi.composite(&mut rp);
            rp.set_bind_group(0, &gpu.bind, &[]);
            // (with no background the sea goes with the land)
            if !sea_args.is_empty() && !self.view.as_ref().is_some_and(|v| v.bare) {
                rp.set_pipeline(&gpu.sea_pipeline);
                if self.args.get("mdi", 1) == 1 {
                    rp.multi_draw_indirect(&gpu.indirect, (MAX_CHUNKS * 16) as u64, sea_args.len() as u32);
                } else {
                    for d in &sea_args {
                        rp.draw(d.first_vertex..d.first_vertex + d.vertex_count, d.first_instance..d.first_instance + d.instance_count);
                    }
                }
            }
            if self.wquad_n > 0 && !self.view.as_ref().is_some_and(|v| v.bare) {
                rp.set_pipeline(&gpu.water_pipeline);
                rp.draw(0..self.wquad_n * 6, 0..1);
            }
            if self.ghost_n > 0 {
                rp.set_pipeline(&gpu.ghost_pipeline);
                rp.set_vertex_buffer(0, gpu.ghost_buf.slice(..));
                rp.draw(0..self.ghost_n, 0..9); // nine times, a pixel apart: overlay.wgsl ghost_vs
            }
            if self.ui_n > 0 {
                rp.set_pipeline(&gpu.ui_pipeline);
                rp.set_vertex_buffer(0, gpu.ui_buf.slice(..));
                rp.draw(0..6, 0..self.ui_n);
            }
        }
        stamp(&mut enc, 7);
        if let (Some(i), Some((qs, resolve, ring))) = (ts_use, &gpu.ts) {
            enc.resolve_query_set(qs, (i * TS_N) as u32..((i + 1) * TS_N) as u32, resolve, (i * 256) as u64);
            enc.copy_buffer_to_buffer(resolve, (i * 256) as u64, &ring[i].0, 0, (TS_N * 8) as u64);
            self.ts_frames[i] = self.streamer.frame;
        }
        // a shot waits a second after loading, so the last meshes are in, and as many frames as a quiet PC draws
        // in that second: the bounce light settles by frames, not by the clock, and a shot beside a build got a
        // handful of them and came out dark (the gate's viewpoints failing under load, 2026-11-12)
        let cap_at = if self.shot.is_some() { 1.0 } else { self.next_cap };
        let film = self.shot.is_some() || self.args.get("capture_ms", 0.0) > 0.0;
        let settled = self.shot.is_none() || bench::shot_settled(self.streamer.frame - self.loaded_frame);
        let cap_now = film && gpu.capture.is_some() && self.loaded_at.is_some() && t >= cap_at && settled;
        // F9: this frame, without the form, is the report's screenshot
        let bug_now = self.bug_grab && gpu.capture.is_some();
        let view_now = self.view.as_ref().is_some_and(|v| v.wants()) && self.loaded_at.is_some();
        if cap_now || bug_now || view_now {
            enc.copy_texture_to_buffer(
                frame.texture.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: gpu.capture.as_ref().unwrap(),
                    layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row_bytes()), rows_per_image: Some(h()) },
                },
                wgpu::Extent3d { width: w(), height: h(), depth_or_array_layers: 1 },
            );
        }
        lap!("encode");
        let tsub = Instant::now();
        gpu.queue.submit([enc.finish()]);
        let sub = tsub.elapsed().as_secs_f64();
        lap!("submit");
        if let (Some(i), Some((_, _, ring))) = (ts_use, &gpu.ts) {
            let f = ring[i].1.clone();
            f.store(false, Ordering::Release);
            ring[i].0.slice(..).map_async(wgpu::MapMode::Read, move |r| {
                if r.is_ok() {
                    f.store(true, Ordering::Release)
                }
            });
        }
        if let (Some(i), Some((_, _, ring))) = (ts_use, &mut self.gpu.as_mut().unwrap().ts) {
            ring[i].2 = true;
        }
        self.ts_slot += 1;
        let gpu = self.gpu.as_ref().unwrap();
        let _ = gpu.device.poll(wgpu::PollType::Poll);
        let cpu_b = cpu1.elapsed().as_secs_f64();
        gpu.window.pre_present_notify();
        lap!("post");
        let tpres = Instant::now();
        gpu.queue.present(frame);
        let pres = tpres.elapsed().as_secs_f64();
        if let Some(f) = self.args.0.remove("shown") {
            let _ = std::fs::write(f, ""); // the process before this one may close its window now (ADR 0038)
        }
        lap!("present");
        let tail0 = Instant::now();
        if self.bug_grab {
            let shot = if bug_now { Some(self.read_frame()) } else { None };
            self.bug_shot = shot;
            (self.bug_grab, self.bug) = (false, Some(String::new()));
        }
        if view_now {
            self.view_send();
        }
        if cap_now && self.shot.is_some() {
            self.take_shot();
            self.finish();
        } else if cap_now {
            self.capture_frame(t);
        }

        // a filmstrip grab stalls on a readback: keep that frame and the next out of the numbers
        let skip = cap_now || bug_now || view_now || self.cap_prev;
        self.cap_prev = cap_now || bug_now || view_now;
        self.recent.push_back((dt * 1000.0) as f32);
        if self.recent.len() > 240 {
            self.recent.pop_front();
        }
        if self.f3 && self.streamer.frame % 30 == 0 {
            self.mem = (sys::ram_mb(), sys::vram_mb());
        }
        if measuring && skip {
            self.samples.capture_stalls += 1;
        }
        if measuring && !skip {
            let s = &mut self.samples;
            // this frame's interval is the last frame's stretches and the wait before this one (ADR 0025)
            let mut fs = self.seg_prev;
            fs[CPU_STAGES.len() - 1] = between;
            s.cpu_st.resize(CPU_STAGES.len(), Vec::new());
            for (v, x) in s.cpu_st.iter_mut().zip(fs) {
                v.push((x * 1000.0) as f32);
            }
            // every frame over the 120 fps budget names where its time went (ADR 0004); the GPU's stages are
            // the last frame's, filled in when they are read back
            if dt * 1000.0 > 8.3 && s.spikes.len() < 50 {
                let stages: Vec<String> = CPU_STAGES.iter().zip(fs).map(|(n, x)| format!("\"{n}\":{:.2}", x * 1000.0)).collect();
                s.spikes.push((
                    self.streamer.frame - 1,
                    format!(
                        "\"t\":{:.2},\"frame_ms\":{:.2},\"cpu_ms\":{:.2},\"select_ms\":{:.2},\"submit_ms\":{:.2},\"present_ms\":{:.2},\"prev_tail_ms\":{:.2},\"between_ms\":{:.2},\"chunks_applied\":{},\"cpu\":{{{}}}",
                        t,
                        dt * 1000.0,
                        (cpu_a + cpu_b) * 1000.0,
                        sel * 1000.0,
                        sub * 1000.0,
                        pres * 1000.0,
                        self.tail_prev * 1000.0,
                        between * 1000.0,
                        st.applied,
                        stages.join(",")
                    ),
                    None,
                ));
            }
            s.frame_ms.push((dt * 1000.0) as f32);
            s.cpu_ms.push(((cpu_a + cpu_b) * 1000.0) as f32);
            s.sel_ms.push((sel * 1000.0) as f32);
            s.submit_ms.push((sub * 1000.0) as f32);
            s.acq_ms.push((acq * 1000.0) as f32);
            s.draws += args.len() as u64;
            s.quads += nquads;
            s.frames += 1;
            s.missing_max = s.missing_max.max(st.missing);
            s.holes_max = s.holes_max.max(st.holes);
            s.applied += st.applied as u64;
            s.applied_nonempty += st.applied_nonempty as u64;
            if s.frames % 60 == 1 {
                s.ram_max = s.ram_max.max(sys::ram_mb());
                s.vram_max = s.vram_max.max(sys::vram_mb());
            }
        }
        if self.loaded_at.is_none() {
            let el = self.start.elapsed().as_secs_f64();
            if (self.streamer.idle() && st.missing == 0 && rt_pending == 0 && self.streamer.frame > 10) || el > 60.0 {
                eprintln!("[technoape] loaded in {:.2} s, {} chunks, {} quads", el, self.streamer.slot_count(), self.streamer.alloc.used);
                self.loaded_at = Some(Instant::now());
                self.loaded_frame = self.streamer.frame;
                if let (Some(gi), true) = (self.gi.as_mut(), self.shot.is_some()) {
                    gi.drop_history();
                }
                self.jobs_at_load = (self.streamer.job_us_total, self.streamer.jobs_done as u64);
                if self.run == "play" {
                    self.chat_line("Welcome to Terra. T chats, / runs a command, Esc opens the menu.".to_string());
                }
                self.last_pos = p.pos;
            }
        }
        lap!("tail");
        self.seg_prev = seg;
        let end = mark;
        self.tail_prev = (end - tail0).as_secs_f64();
        self.frame_end = Some(end);
    }
}

impl crate::app::App {
    /// Where animal `id` is drawn now (ADR 0083): lerped from where it was to its last view over one tick.
    pub(crate) fn animal_at(&self, id: u32, now: Instant) -> Option<DVec3> {
        let (a, from, at) = self.animals.get(&id)?;
        let k = (now.duration_since(*at).as_secs_f64() * engine::net::TICK_HZ).min(1.0);
        Some(from.lerp(DVec3::from_array(a.pos), k))
    }
}
