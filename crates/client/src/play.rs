//! run=play: building, digging and the bucket, `acts=` scripted input, chat and commands, F3 and F9, reconciling with the server (ADR 0025 L: moved out of main.rs).
use super::*;

/// acts="2:C,2.5:c,3:R,3.5:r": (seconds after load, action), sorted so the soonest is last.
pub(crate) fn parse_acts(s: &str) -> Vec<(f64, String)> {
    let mut v: Vec<(f64, String)> =
        s.split(',').filter_map(|e| e.trim().split_once(':')).filter_map(|(t, a)| Some((t.parse().ok()?, a.to_string()))).collect();
    v.sort_by(|a, b| b.0.total_cmp(&a.0));
    v
}

impl App {
    /// One input of run=play's building, from the keyboard and mouse or from acts=: L/R press and l/r release
    /// the dig and place buttons, h takes up or puts away the build hammer, b the bucket, q cycles its box, cylinder and
    /// cone, up/down change a cylinder's or cone's height, C and c press and let go of the zoom, 1-9 pick a hotbar slot,
    /// m:dx:dy turns the look.
    pub(crate) fn act(&mut self, a: &str) {
        match a {
            "L" | "R" if self.bucket => {
                if let Some((_, before)) = self.aim {
                    self.link.send(ToServer::Bucket { seq: self.pred.seq(), at: before, fill: a == "L" });
                }
            }
            "L" | "R" => {
                let dig = a == "L";
                if let Some(at) = self.aim.map(|(hit, before)| self.anchor(hit, before, dig)) {
                    self.held = Some((dig, at, at));
                }
            }
            "l" | "r" => {
                if let Some((dig, at, end)) = self.held.take().filter(|h| h.0 == (a == "l")) {
                    if let Some(s) = self.shape(dig, at, end) {
                        self.apply(&s, dig);
                    }
                }
            }
            "b" => {
                self.tool.kind = match self.tool.kind {
                    Kind::Box => Kind::Cylinder,
                    Kind::Cylinder => Kind::Cone,
                    Kind::Cone => Kind::Box,
                }
            }
            // C held zooms (it is read from the held keys); acts press it with C, let go with c
            "C" => self.zoomed = true,
            "c" => self.zoomed = false,
            // the zoom's key: held, or pressed once to stay (the settings' choice)
            "zoom" => self.zoomed = self.settings.at(S::ZoomMode) == 0 || !self.zoomed,
            "unzoom" => self.zoomed &= self.settings.at(S::ZoomMode) == 1,
            // the menu by keys: next and prev move along the rows, go presses one (a slider: types its number),
            // nudge+ and nudge- change a value; scroll:n moves the window of rows; bind:name gives a waiting
            // key row its key (bind:none clears it)
            "next" => self.select(1),
            "prev" => self.select(-1),
            "go" => {
                if let Some(k) = self.sel {
                    if self.menu_sliders().get(k).is_some_and(|s| s.is_some()) {
                        self.edit = Some((k, String::new()));
                    } else {
                        self.menu_press(k, false);
                    }
                }
            }
            "nudge+" | "nudge-" => self.nudge(a == "nudge-"),
            a if a.starts_with("scroll:") => self.scroll_by(a[7..].parse().unwrap_or(0)),
            a if a.starts_with("bind:") => self.bind_to(Bind::parse(&a[5..])),
            // key+:name and key-:name press and let go of a key or button itself (KeyW, Mouse1), through the
            // settings' bindings as the keyboard does
            a if a.starts_with("key+:") || a.starts_with("key-:") => {
                if let Some(b) = Bind::parse(&a[5..]) {
                    self.bind_event(b, a.starts_with("key+"), true);
                }
            }
            // the wheel: the flight speed while flying, unless the hammer holds a shape with a height
            "up" | "down" if self.flying && !(self.hammer && self.tool.kind != Kind::Box) => {
                let f = if a == "up" { 1.25 } else { 1.0 / 1.25 };
                self.fly_speed = (self.fly_speed * f).clamp(1.0, phys::FLY_MAX);
            }
            "up" | "down" => {
                self.tool.height = (self.tool.height + if a == "up" { 1 } else { -1 }).clamp(1, edit::MAX_SIDE);
            }
            // space twice within FLY_TAP seconds takes off or lands (creative, ADR 0023)
            "space" => {
                let now = Instant::now();
                if self.space_at.is_some_and(|t| (now - t).as_secs_f64() < self.settings.get(S::FlyTap)) {
                    self.space_at = None;
                    self.act("fly");
                } else {
                    self.space_at = Some(now);
                }
            }
            "fly" if self.creative => self.flying = !self.flying,
            "fly" => self.note("flying is creative's: /playmode creative".into()),
            "h" => (self.hammer, self.bucket) = (!self.hammer, false),
            "v" => (self.bucket, self.hammer) = (!self.bucket, false),
            // the chat and the bug report (ADR 0023): t opens the chat, / opens it on a command, type:x writes,
            // back takes a character, enter sends or saves, cancel closes; say:x sends a line at once
            "t" | "/" if !self.texting() => {
                self.typing = Some(if a == "/" { "/".into() } else { String::new() });
                // the up arrow first recalls the lines sent before; typing shows the completions
                (self.recall, self.pick) = (None, None);
                (self.held, self.mouse) = (None, (0.0, 0.0));
                self.keys.clear();
                self.zoomed &= self.settings.at(S::ZoomMode) == 1;
            }
            "f3" => self.f3 = !self.f3,
            // . and , step the time warp ten times up or down (the server's /timewarp answers)
            "warp+" | "warp-" => self.link.send(ToServer::Chat(format!("/timewarp {}", if a == "warp+" { "up" } else { "down" }))),
            "f9" if !self.texting() => {
                self.bug_grab = true;
                (self.held, self.mouse) = (None, (0.0, 0.0));
                self.keys.clear();
                self.zoomed &= self.settings.at(S::ZoomMode) == 1;
            }
            "enter" => self.submit(),
            // the menu (ADR 0023): esc opens it, goes back a page or closes it; click:k and rclick:k press button k
            "esc" if self.menu == Some(Menu::Confirm) => self.undo_display(),
            "esc" => self.set_menu(match self.menu {
                None => Some(Menu::Pause),
                Some(Menu::Pause) | Some(Menu::Confirm) => None,
                Some(Menu::Settings) => Some(self.home()),
                Some(Menu::Page(_)) => Some(Menu::Settings),
                Some(Menu::Title | Menu::Worlds) => Some(Menu::Title),
            }),
            // point:x:y puts the pointer there (pixels), to see a button lit
            a if a.starts_with("point:") => {
                if let Some((x, y)) = a[6..].split_once(':').and_then(|(x, y)| Some((x.parse().ok()?, y.parse().ok()?))) {
                    self.pointer = (x, y);
                }
            }
            a if a.starts_with("click:") || a.starts_with("rclick:") => {
                if let Some(k) = a.split(':').nth(1).and_then(|k| k.parse().ok()) {
                    self.menu_press(k, a.starts_with('r'));
                }
            }
            // with the completions shown, the arrows move along them and Tab takes one (ADR 0023)
            "earlier" | "later" if self.pick.is_some() && !self.hints().is_empty() => {
                let (n, k) = (self.hints().len(), self.pick.unwrap_or(0));
                self.pick = Some(if a == "earlier" { (k + n - 1) % n } else { (k + 1) % n });
            }
            "tab" => {
                let Some(t) = self.typing.clone() else { return };
                let (at, hints) = engine::net::complete(&t);
                if let Some(h) = hints.get(self.pick.unwrap_or(0).min(hints.len().saturating_sub(1))) {
                    let mut line = format!("{}{h}", &t[..at]);
                    // on to the next word when there is one to pick
                    if !engine::net::complete(&format!("{line} ")).1.is_empty() {
                        line.push(' ');
                    }
                    self.typing = Some(line);
                }
                self.pick = Some(0);
            }
            // the chat's up and down arrows: the lines sent before, newest first, then back to an empty line
            "earlier" | "later" if self.typing.is_some() => {
                let n = self.sent.len();
                let at = match (a, self.recall) {
                    ("earlier", None) => n.checked_sub(1),
                    ("earlier", Some(k)) => Some(k.saturating_sub(1)),
                    (_, Some(k)) if k + 1 < n => Some(k + 1),
                    _ => None,
                };
                if at.is_some() || self.recall.is_some() {
                    self.typing = Some(at.map_or(String::new(), |k| self.sent[k].clone()));
                }
                (self.recall, self.pick) = (at, None);
            }
            "cancel" => (self.typing, self.bug, self.bug_shot, self.edit) = (None, None, None, None),
            // lclick: a left click at the pointer in the menu, let go at once (a slider jumps there)
            "lclick" => {
                self.menu_click(false);
                if self.drag.take().is_some() {
                    self.save_settings();
                }
            }
            "back" => {
                if let Some(t) = self.typing.as_mut().or(self.bug.as_mut()).or(self.edit.as_mut().map(|e| &mut e.1)) {
                    t.pop();
                }
                self.pick = Some(0);
            }
            // a slider's number takes digits and a point only
            a if a.starts_with("type:") && self.edit.is_some() => {
                let t = &mut self.edit.as_mut().unwrap().1;
                let room = 6usize.saturating_sub(t.len());
                t.extend(a[5..].chars().filter(|c| c.is_ascii_digit() || *c == '.').take(room));
            }
            a if a.starts_with("type:") => {
                if let Some(t) = self.typing.as_mut().or(self.bug.as_mut()) {
                    t.push_str(&a[5..]);
                }
                self.pick = Some(0);
            }
            a if a.starts_with("say:") => self.link.send(ToServer::Chat(a[4..].to_string())),
            _ => {
                if let Ok(d) = a.parse::<usize>() {
                    self.slot = d.clamp(1, 9) - 1;
                } else if let Some((x, y)) = a.strip_prefix("m:").and_then(|r| r.split_once(':')) {
                    self.mouse.0 += x.parse().unwrap_or(0.0);
                    self.mouse.1 += y.parse().unwrap_or(0.0);
                } else {
                    self.note(format!("unknown act {a:?}"));
                }
            }
        }
    }

    /// The completions of the command being typed.
    pub(crate) fn hints(&self) -> Vec<String> {
        self.typing.as_deref().map_or(Vec::new(), |t| engine::net::complete(t).1)
    }

    /// The field of view drawn now, in degrees: the setting's, narrowed by the zoom.
    pub(crate) fn fov(&self) -> f64 {
        2.0 * ((self.settings.get(S::Fov).to_radians() / 2.0).tan() / self.zoom).atan().to_degrees()
    }

    /// A text is being written (the chat or the bug report's form), or F9 waits for its frame: keys and the
    /// mouse go there, not to the player.
    pub(crate) fn texting(&self) -> bool {
        self.typing.is_some() || self.bug.is_some() || self.bug_grab || self.edit.is_some()
    }

    /// A line of this client's own for the chat, and the log.
    pub(crate) fn note(&mut self, line: String) {
        eprintln!("[technoape] {line}");
        self.chat_line(line);
    }

    pub(crate) fn chat_line(&mut self, line: String) {
        self.chat.push((Instant::now(), line));
        if self.chat.len() > 200 {
            self.chat.remove(0);
        }
    }

    /// Enter: the chat's line goes to the server (it runs the commands), the bug report to bugs/.
    pub(crate) fn submit(&mut self) {
        if let Some((k, t)) = self.edit.take() {
            if let (Ok(v), Some(Row::Set(s))) = (t.parse::<f64>(), self.page_row(k)) {
                let before = self.settings.clone();
                self.settings.set(s, v);
                self.apply_settings(&before);
            }
            return;
        }
        if let Some(line) = self.typing.take() {
            let line = line.trim().to_string();
            self.recall = None;
            if !line.is_empty() && self.sent.last() != Some(&line) {
                self.sent.push(line.clone());
            }
            if line.starts_with('/') {
                self.chat_line(format!("> {line}"));
            }
            // nothing stands under the sun: a creative player flies there
            if line.eq_ignore_ascii_case("/goto sun") && self.creative {
                self.flying = true;
            }
            if !line.is_empty() {
                self.link.send(ToServer::Chat(line));
            }
        } else if let Some(text) = self.bug.take() {
            self.report_bug(text.trim().to_string());
        }
    }

    /// The F3 overlay's lines (ADR 0023), left and right column; a bug report carries them too.
    pub(crate) fn debug_lines(&self) -> (Vec<String>, Vec<String>) {
        let (pos, fwd, _) = self.cam;
        let w = &self.world;
        let space = w.frame_at(pos) == engine::world::INERTIAL;
        let bi = if space { 0 } else { w.frame_at(pos) };
        let b = &w.bodies[bi];
        let rel = pos - b.centre;
        let (r, up) = (rel.length(), rel.normalize());
        let feet = if self.run == "play" { self.me.pos } else { pos - up * 1.7 };
        let blk = w.block_at(feet);
        let x = w.to_inertial(pos);
        let (to_sun, to_planet) = ((x - w.sky.sun(w.time)).length(), (x - w.sky.planet(w.time).centre).length());
        let mut ms: Vec<f32> = self.recent.iter().copied().collect();
        let avg = ms.iter().sum::<f32>() / ms.len().max(1) as f32;
        let p99 = pct(&mut ms, 0.99);
        let (day, h) = engine::sky::clock(w.time);
        let m = (h * 60.0).round() as u32 % (24 * 60);
        let body = b.name.as_str();
        let sea = b.sea.map_or(String::new(), |s| format!(", {:.1} m over the sea", r - s));
        let look = match self.aim {
            Some((hit, _)) => format!(
                "Looking at: {} (face {}, {} {}, layer {})",
                BLOCKS.get(w.block(hit)).map_or("?", |d| d.name.as_str()),
                hit.face,
                hit.pos.x,
                hit.pos.y,
                hit.pos.z
            ),
            None => "Looking at: nothing in reach".into(),
        };
        let (yaw, pitch) = stand_angles(w.up_at(pos), fwd);
        let tool = if self.bucket { "bucket" } else if self.hammer { "build hammer" } else { "hands" };
        let tool = if self.flying { format!("{tool}, flying {:.0} m/s", self.fly_speed) } else { tool.to_string() };
        let block = self.slot_block(self.slot).and_then(|m| BLOCKS.get(m)).map_or("-", |d| d.name.as_str());
        let d = &self.drawn;
        let gpu = self.gpu.as_ref().unwrap();
        let left = vec![
            format!("TechnoApe {BUILD} ({}), Pillars of Simulation Engine", if cfg!(debug_assertions) { "debug" } else { "release" }),
            format!("{:.0} fps, frame {avg:.2} ms average, {p99:.2} ms p99, GPU {:.2} ms", 1000.0 / avg.max(0.01), self.gpu_last),
            if space {
                format!("In space: {:.1} km from the sun's centre, {:.1} km from {}'s", to_sun / 1000.0, to_planet / 1000.0, w.bodies[0].name)
            } else {
                format!("XYZ: {:.2} / {:.2} / {:.2} from {body}'s centre", rel.x, rel.y, rel.z)
            },
            if space { "Height: no ground under you".into() } else { format!("Height: {r:.1} m from the centre, {:.1} m over the ground{sea}", r - b.top(up)) },
            if space {
                "Feet in block: none".into()
            } else {
                format!("Feet in block: {body} face {} at {} {}, layer {}", blk.face, blk.pos.x, blk.pos.y, blk.pos.z)
            },
            format!("Facing: yaw {yaw:.1}, pitch {pitch:.1}"),
            look,
            format!(
                "Time: day {day}, {:02}:{:02} at the spawn, world time {:.0} s{}",
                m / 60,
                m % 60,
                w.time,
                self.clock.filter(|c| c.1 > 1.0).map_or(String::new(), |c| format!(", warp {}x", c.1))
            ),
            format!("Sun {:.1} degrees over the horizon, {} {:.0}% lit", d.sun_deg, w.bodies.get(1).map_or("the moon", |b| b.name.as_str()), d.moon_lit * 100.0),
            format!("Mode: {}, {tool}, slot {}: {block}", if self.creative { "creative" } else { "survival" }, self.slot + 1),
            format!("Chunks: {} drawn of {} kept, {} quads, {} traced builds waiting", d.chunks, self.streamer.slot_count(), d.quads, d.rt_pending),
            format!("Streaming: {} missing, {} holes, {} requests, {} jobs running", d.streaming.0, d.streaming.1, d.streaming.2, d.streaming.3),
            format!("Lamps near: {}, water quads: {}, edits applied: {}", self.lamps.len(), self.wquad_n, self.edits_applied),
            format!("Players: {}, corrections: {}", self.players, self.snaps),
            String::new(),
            "F3 hides this. T chats, / runs a command (/help), F9 reports a bug".into(),
        ];
        let right = vec![
            format!("GPU: {}", gpu.adapter_name),
            format!("{} with ray queries, {}x{}", gpu.backend, gpu.config.width, gpu.config.height),
            format!("RAM {:.0} MB, VRAM {:.0} MB", self.mem.0, self.mem.1),
            format!("CPU: {} workers", rayon::current_num_threads()),
            format!("World: seed {}, {}", self.seed, w.bodies.iter().map(|b| format!("{} {} columns a face", b.name, b.n)).collect::<Vec<_>>().join(", ")),
            format!("Server: {}", self.server_at),
            format!("Save: {}", self.save_path.as_deref().unwrap_or("none")),
            format!("Recording: {}", self.rec_path.as_deref().unwrap_or("none")),
        ];
        (left, right)
    }

    /// F9's report (ADR 0023): bugs/<UTC time>/ gets report.md (the text, a line that stands at the same
    /// spot, F3's lines, the chat, the arguments), screen.png (the frame when F9 was pressed) and world.sav (a
    /// copy of the save). The files are written on a thread.
    pub(crate) fn report_bug(&mut self, text: String) {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let dir = format!("bugs/{}", sys::utc_stamp(now));
        let (left, right) = self.debug_lines();
        let w = &self.world;
        let (pos, fwd, _) = self.cam;
        let bi = w.frame_at(pos);
        let b = &w.bodies[bi.min(w.bodies.len() - 1)];
        let up = (pos - b.centre).normalize();
        let (yaw, pitch) = stand_angles(up, fwd);
        let stand = if bi == 0 {
            format!(
                "technoape.exe run=stand at={:.6},{:.6},{:.6} yaw={yaw:.1} pitch={pitch:.1} eye={:.2} time={:.1} seed={} planet={} capture_ms=2500 sheet=out/bug.png",
                up.x,
                up.y,
                up.z,
                (pos - b.centre).length() - b.top(up),
                w.time,
                self.seed,
                b.n
            )
        } else {
            "(run=stand stands on Terra only)".into()
        };
        let mut args: Vec<String> = self.args.0.iter().map(|(k, v)| format!("{k}={v}")).collect();
        args.sort();
        let chat: Vec<String> =
            self.chat.iter().rev().take(50).rev().map(|(at, l)| format!("- {:.0} s ago: {l}", at.elapsed().as_secs_f64())).collect();
        let inv: Vec<String> = self.inv.iter().map(|(m, n)| format!("{} {n}", BLOCKS.get(*m).map_or("?", |d| d.name.as_str()))).collect();
        let report = format!(
            "# Bug report {}\n\n{}\n\n## To look at it again\n\n- The view, without the session's edits: `{stand}`\n- The world: {}\n- The session: {}\n\n## State (F3)\n\n{}\n\n{}\n\nInventory: {}; bucket {} units\n\n## Chat and messages\n\n{}\n\n## Arguments\n\n`{}`\n",
            sys::utc_stamp(now),
            if text.is_empty() { "(no text)" } else { &text },
            if self.save_path.as_ref().is_some_and(|s| std::path::Path::new(s).exists()) {
                "`world.sav` here is the save as it was (up to 30 s old); play a copy with `technoape.exe run=play onscreen=1 save=<copy>`"
            } else {
                "no save yet (play saves every 30 s), only what the view line shows"
            },
            self.rec_path.as_ref().map_or("not recorded".to_string(), |r| format!("recorded to `{r}`; `technoape-server replay={r}` runs it again")),
            left.iter().filter(|l| !l.is_empty() && !l.starts_with("F3 ")).map(|l| format!("- {l}")).collect::<Vec<_>>().join("\n"),
            right.iter().map(|l| format!("- {l}")).collect::<Vec<_>>().join("\n"),
            if inv.is_empty() { "nothing".into() } else { inv.join(", ") },
            self.bucket_water,
            if chat.is_empty() { "(none)".into() } else { chat.join("\n") },
            args.join(" ")
        );
        let (shot, save, said) = (self.bug_shot.take(), self.save_path.clone(), self.bug_said.0.clone());
        std::thread::spawn(move || {
            let out = (|| -> Result<(), String> {
                let d = std::path::Path::new(&dir);
                std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
                std::fs::write(d.join("report.md"), report).map_err(|e| e.to_string())?;
                if let Some(img) = shot {
                    img.save(d.join("screen.png")).map_err(|e| e.to_string())?;
                }
                if let Some(s) = save.filter(|s| std::path::Path::new(s).exists()) {
                    std::fs::copy(&s, d.join("world.sav")).map_err(|e| e.to_string())?;
                }
                Ok(())
            })();
            let _ = said.send(match out {
                Ok(()) => format!("bug report saved to {dir}/"),
                Err(e) => format!("the bug report could not be saved to {dir}/: {e}"),
            });
        });
    }

    /// Where a press starts: digging takes the aimed block, placing the cell in front of it.
    pub(crate) fn anchor(&self, hit: Block, before: Block, dig: bool) -> Block {
        if dig { hit } else { before }
    }

    /// What a press at `at` released at `end` does (ADR 0017): bare hands dig and place one block where the
    /// release is; the build hammer places the tool's shape over the drag. shortcut: digging stays one block
    /// until mining tools come.
    pub(crate) fn shape(&self, dig: bool, at: Block, end: Block) -> Option<Shape> {
        if dig || !self.hammer {
            return Some(Shape::one(end));
        }
        self.tool.shape(&self.world, at, end, false)
    }

    /// The hotbar: every placeable block in id order.
    pub(crate) fn slot_block(&self, k: usize) -> Option<u8> {
        BLOCKS.placeable().nth(k).map(|d| d.id)
    }

    pub(crate) fn held_count(&self, m: u8) -> u32 {
        self.inv.iter().find(|(id, _)| *id == m).map_or(0, |(_, n)| *n)
    }

    /// Digs or places a shape: predicted here at once (ADR 0015), sent to the server. A survival place this
    /// player cannot pay for is not sent, so it never flickers in and out.
    pub(crate) fn apply(&mut self, s: &Shape, dig: bool) {
        let m = if dig {
            AIR
        } else {
            match self.slot_block(self.slot) {
                Some(m) => m,
                None => return,
            }
        };
        if !s.fits(&self.world) {
            return;
        }
        if !dig && !self.creative {
            let me = &self.me;
            let need = edit::changes(&self.world, s, m, |c| phys::holds(&self.world, me, c)).len() as u32;
            if need > self.held_count(m) {
                return;
            }
        }
        let me = &self.me;
        let (seq, cells) = self.pred.edit(&mut self.world, s, m, |w, c| phys::holds(w, me, c));
        for c in &cells {
            self.streamer.touch_block(&self.world, *c);
        }
        if !cells.is_empty() {
            self.clicks.push((self.streamer.frame, Instant::now(), cells));
        }
        self.link.send(ToServer::Edit { seq, shape: *s, material: m });
    }

    /// Each frame of run=play: scripted acts that are due, the aim, the ghost and the 2D layer.
    pub(crate) fn build(&mut self, p: &Pose, t: f64) {
        while self.acts.last().is_some_and(|(at, _)| *at <= t) {
            let (_, a) = self.acts.pop().unwrap();
            self.act(&a);
        }
        while let Ok(l) = self.bug_said.1.try_recv() {
            self.note(l);
        }
        self.aim = edit::aim(&self.world, p.pos, p.fwd, AIM_REACH);
        // the ghost: red what a held dig takes, green what a held place fills; when nothing is held, white
        // around the aimed block, or with the hammer where its shape would go
        let ghost = match (self.held, self.aim) {
            (Some((dig, at, _)), Some((hit, before))) => {
                let end = self.anchor(hit, before, dig);
                self.held = Some((dig, at, end));
                self.shape(dig, at, end).map(|s| (s, dig, true))
            }
            (Some((dig, at, end)), None) => self.shape(dig, at, end).map(|s| (s, dig, true)),
            (None, Some((_, before))) if self.hammer => self.shape(false, before, before).map(|s| (s, false, false)),
            (None, Some((hit, _))) => Some((Shape::one(hit), true, false)),
            (None, None) => None,
        };
        let ghost = ghost.filter(|(s, ..)| s.fits(&self.world)).map(|(s, dig, held)| {
            let colour = match (held, dig) {
                (false, _) => [255, 255, 255, 150],
                (true, true) => [255, 90, 60, 230],
                (true, false) => [110, 255, 120, 230],
            };
            (s, colour)
        });
        let gpu = self.gpu.as_ref().unwrap();
        if ghost != self.ghost_of {
            let verts = ghost.map(|(s, c)| hud::ghost(&self.world, &s, c)).unwrap_or_default();
            gpu.queue.write_buffer(&gpu.ghost_buf, 0, bytemuck::cast_slice(&verts));
            (self.ghost_n, self.ghost_of) = (verts.len() as u32, ghost);
        }
        let slots: Vec<hud::Slot> = (0..9)
            .filter_map(|k| self.slot_block(k))
            .map(|m| hud::Slot { colour: BLOCKS.get(m).unwrap().colour, count: (!self.creative).then(|| self.held_count(m)) })
            .collect();
        let height = (self.hammer && self.tool.kind != Kind::Box).then_some(self.tool.height);
        // the title menu shows the orbit view and itself only
        let title = self.home() == Menu::Title;
        let (lw, lh) = self.ui_size();
        let mut rects = if title { Vec::new() } else { hud::layer(lw, lh, &slots, self.slot, self.creative, height) };
        if self.world.water_surface(p.pos).is_some() {
            // under water: a blue veil first, under the HUD. shortcut: a veil, not fog, until the sky has fog
            rects.insert(0, hud::Rect { xy: [-1.0, -1.0, 1.0, 1.0], colour: [0.05, 0.25, 0.4, 0.55], glyph: 0 });
        }
        // the text over it (ADR 0023): F3's numbers, the chat, the bug report's form
        let mut paint = hud::Paint::new(lw, lh);
        if self.f3 {
            let (l, r) = self.debug_lines();
            hud::columns(&mut paint, &l, &r);
        }
        let lines: Vec<(f32, &str)> = self.chat.iter().map(|(at, l)| (at.elapsed().as_secs_f32(), l.as_str())).collect();
        let caret = self.start.elapsed().as_millis() / 400 % 2 == 0;
        let (at, hints) = engine::net::complete(self.typing.as_deref().unwrap_or(""));
        if !title {
            hud::chat(&mut paint, &lines, self.typing.as_deref(), caret, at, &hints, self.pick);
        }
        if self.flying {
            let s = format!("Flying {:.0} m/s", self.fly_speed);
            // right of the hotbar (hud::layer: nine 44 px slots 6 px apart), clear of the chat
            paint.text(lw / 2.0 + 222.0 + 16.0, lh - 46.0, &s, [1.0, 1.0, 1.0, 0.9]);
        }
        if let Some(m) = self.menu {
            let (title, hint) = match m {
                Menu::Title => ("TechnoApe", "Pillars of Simulation Engine"),
                Menu::Worlds => ("Singleplayer", "A world carries on from its save; a new one gets its own."),
                Menu::Pause => ("Paused", "The world runs on. Esc goes back to the game."),
                Menu::Settings => ("Settings", "Click or Enter: the next value. Right click: the one before. Arrows move."),
                Menu::Page(p) => settings::PAGES[p],
                Menu::Confirm => ("Display", "Can you see this? It goes back by itself if you do nothing."),
            };
            let (items, sliders) = (self.menu_items(), self.menu_sliders());
            // as many rows as fit, from `scroll` on
            self.scroll = self.scroll.min(items.len().saturating_sub(self.menu_fit()));
            let end = (self.scroll + self.menu_fit()).min(items.len());
            let (items, sliders) = (&items[self.scroll..end], &sliders[self.scroll..end]);
            let boxes = hud::menu_boxes(lh, items, sliders);
            let (x, y) = self.pointer;
            let under = boxes.iter().position(|b| x >= b[0] && x < b[0] + b[2] && y >= b[1] && y < b[1] + b[3]);
            // the keyboard's row is the lit one until the pointer moves
            let hover = match self.sel {
                Some(k) => k.checked_sub(self.scroll).filter(|v| *v < boxes.len()),
                None => under,
            };
            hud::menu(&mut paint, title, items, sliders, hint, hover, caret, self.scroll);
            self.menu_boxes = boxes;
        }
        // the bug report's form over everything, the menus too
        if let Some(b) = &self.bug {
            hud::bug_form(&mut paint, b, caret);
        }
        rects.extend(paint.out);
        let n = rects.len().min(hud::RECTS_MAX);
        gpu.queue.write_buffer(&gpu.ui_buf, 0, bytemuck::cast_slice(&rects[..n]));
        self.ui_n = n as u32;
        // shortcut: the tool and mode as words in the window title, until the HUD has text
        let block = self.slot_block(self.slot).and_then(|m| BLOCKS.get(m)).map_or("-".to_string(), |d| d.name.clone());
        let tool = match (self.hammer, self.tool.kind) {
            _ if self.bucket => format!("bucket {}%", self.bucket_water as i64 * 100 / engine::liquid::U),
            (false, _) => "hands".to_string(),
            (true, Kind::Box) => "build hammer: box".to_string(),
            (true, k) => format!("build hammer: {} {} high", format!("{k:?}").to_lowercase(), self.tool.height),
        };
        let title = format!("TechnoApe | {} | {tool} | {block}", if self.creative { "creative" } else { "survival" });
        if title != self.title {
            gpu.window.set_title(&title);
            self.title = title;
        }
    }

    /// The server's state after step `step` (ADR 0019): the steps it has not taken yet are replayed from it, and
    /// where that lands is where this player is. A difference is a correction, drawn eased out unless it is a
    /// jump of metres (a teleport, a respawn). The first answer places the player.
    pub(crate) fn reconcile(&mut self, step: u32, server: phys::Player, t: f64) {
        if !self.spawned {
            (self.me, self.prev, self.spawned) = (server, server.pos, true);
            self.steps.retain(|s| s.0 > step);
            return;
        }
        self.steps.retain(|s| s.0 > step);
        // each step again at the time it was first taken (the server takes it at that time too)
        let replay = |world: &mut World, steps: &VecDeque<(u32, Input)>| {
            let mut me = server;
            for (_, inp) in steps {
                phys::step_at(world, &mut me, inp, phys::DT);
            }
            me
        };
        let me = replay(&mut self.world, &self.steps);
        let off = self.me.pos - me.pos;
        if off.length() >= 2.0 {
            // a teleport (/goto, a respawn): the look is the server's, in the frame it lands in, level; the
            // steps not answered yet only turn it
            (self.pitch, self.tilt, self.roll) = (0.0, 0.0, 0.0);
        }
        self.drift_max = self.drift_max.max(off.length());
        if off.length() > 0.001 {
            self.snap_t.push((t, off.length()));
            self.snaps += 1;
            self.path.clear(); // a correction is counted as one, not as jerk
            self.smooth = if off.length() < 2.0 { self.smooth + off } else { DVec3::ZERO };
            self.prev -= off;
        }
        self.me = me;
    }
}
