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
            // waypoint: marks where the player stands, on the body it stands on (N)
            "waypoint" => {
                let (w, at) = (&self.world, self.me.pos);
                let body = w.frame_at(at);
                if !self.creative {
                    self.note("waypoints come with the compass, which is still to be made: /playmode creative has them".into());
                } else if body == engine::world::INERTIAL {
                    self.note("no waypoints out in space yet: stand on a body".into());
                } else {
                    let rel = at - w.bodies[body].centre;
                    let name = nav::next_name(&self.waypoints);
                    let line = format!("{name} at {} on {}", nav::geo_text(nav::geo(rel, self.meridian(body))), w.bodies[body].name);
                    self.waypoints.push(nav::Waypoint { name, body, rel });
                    self.note(line);
                    self.save_waypoints();
                }
            }
            // remove: the world under the keyboard's row, asked about first (the Delete key)
            "remove" => {
                let (x, y) = self.pointer;
                let under = self.menu_boxes.iter().position(|b| x >= b[0] && x < b[0] + b[2] && y >= b[1] && y < b[1] + b[3]).map(|v| v + self.scroll);
                match (self.menu, self.sel.or(under)) {
                    (Some(Menu::Worlds), Some(k)) if k < self.worlds.len() => self.set_menu(Some(Menu::Remove(k))),
                    (Some(Menu::Waypoints), Some(k)) => self.remove_waypoint(k),
                    _ => {}
                }
            }
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
                Some(Menu::Remove(_)) => Some(Menu::Worlds),
                Some(Menu::Waypoints) => Some(Menu::Pause),
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
        // F3's groups as plain lines, for the bug report
        let text = |v: Vec<hud::Section>| -> Vec<String> {
            v.into_iter().flat_map(|s| std::iter::once(format!("[{}]", s.title)).chain(s.rows.into_iter().map(|r| format!("{}: {}", r.0, r.1)))).collect()
        };
        let (l, r) = self.debug_sections();
        (text(l), text(r))
    }

    /// F3's numbers in groups (ADR 0023; regrouped 2026-11-11): what the game costs, how the simulation
    /// keeps up, where the player is, the world, the player; then what is drawn, the machine, the session.
    /// A value is coloured by how it stands against its limit (ADR 0004).
    pub(crate) fn debug_sections(&self) -> (Vec<hud::Section>, Vec<hud::Section>) {
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
                "{} (face {}, {} {}, layer {})",
                BLOCKS.get(w.block(hit)).map_or("?", |d| d.name.as_str()),
                hit.face,
                hit.pos.x,
                hit.pos.y,
                hit.pos.z
            ),
            None => "nothing in reach".into(),
        };
        let (yaw, pitch) = stand_angles(w.up_at(pos), fwd);
        let tool = if self.bucket { "bucket" } else if self.hammer { "build hammer" } else { "hands" };
        let tool = if self.flying { format!("{tool}, flying {:.0} m/s", self.fly_speed) } else { tool.to_string() };
        let block = self.slot_block(self.slot).and_then(|m| BLOCKS.get(m)).map_or("-", |d| d.name.as_str());
        let d = &self.drawn;
        let gpu = self.gpu.as_ref().unwrap();
        // good, near the limit (three quarters of it), over it
        let tone = |v: f64, limit: f64| if v > limit { 3 } else if v > limit * 0.75 { 2 } else { 1 };
        let section = |title, rows| hud::Section { title, rows, graph: None };
        // the server's last ticks (a local server's; a remote one's times stay there)
        let tick_ms = 1000.0 / TICK_HZ;
        let mut ticks: Vec<f32> = self.ticks.iter().map(|t| t.1).collect();
        let bars: Vec<f32> = ticks.iter().map(|ms| ms / tick_ms as f32).collect();
        let since = Instant::now() - Duration::from_secs(2);
        let rate = self.ticks.iter().filter(|t| t.0 >= since).count() as f64 / 2.0;
        let simulation = if self.tick_log.is_none() {
            vec![("Tick", "the server is remote: its times stay there".into(), 0)]
        } else if ticks.is_empty() {
            vec![("Tick", "no tick yet".into(), 0)]
        } else {
            let worst = ticks.iter().copied().fold(0.0, f32::max) as f64;
            let p50 = pct(&mut ticks, 0.5);
            vec![
                // ADR 0004: a tick's p99 within half its time; the line across the graph is that half
                ("Tick", format!("{p50:.1} ms typical, {worst:.1} ms worst, of {tick_ms:.0}"), if worst > tick_ms { 3 } else if worst > tick_ms / 2.0 { 2 } else { 1 }),
                ("Speed", format!("{rate:.1} of {TICK_HZ:.0} ticks a second"), if (ticks.len() as f64) < 2.0 * TICK_HZ { 0 } else if rate < TICK_HZ * 0.75 { 3 } else if rate < TICK_HZ - 1.0 { 2 } else { 1 }),
            ]
        };
        let left = vec![
            section(
                "Performance",
                vec![
                    ("Frame", format!("{:.0} fps, {avg:.2} ms average", 1000.0 / avg.max(0.01)), 0),
                    ("Worst 1%", format!("{p99:.2} ms of 8.3"), tone(p99 as f64, 8.3)),
                    ("GPU", format!("{:.2} ms", self.gpu_last), 0),
                    ("Memory", format!("RAM {:.0} MB, VRAM {:.0} MB", self.mem.0, self.mem.1), tone(self.mem.0 as f64, 8000.0).max(tone(self.mem.1 as f64, 6000.0))),
                ],
            ),
            hud::Section {
                title: "Simulation",
                rows: simulation
                    .into_iter()
                    .chain([
                        ("Players", format!("{}", self.players), 0),
                        ("Corrections", format!("{}", self.snaps), 0),
                        ("Warp", self.clock.map_or("1x".into(), |c| format!("{}x", c.1)), 0),
                    ])
                    .collect(),
                graph: (!bars.is_empty()).then_some((bars, 0.5)),
            },
            section(
                "Position",
                vec![
                    ("On", if space { "nothing: in space".into() } else { body.to_string() }, 0),
                    if space {
                        ("Away", format!("{:.1} km from Sol's centre, {:.1} km from {}'s", to_sun / 1000.0, to_planet / 1000.0, w.bodies[0].name), 0)
                    } else {
                        ("XYZ", format!("{:.2} / {:.2} / {:.2} from the centre", rel.x, rel.y, rel.z), 0)
                    },
                    ("Height", if space { "no ground under you".into() } else { format!("{:.1} m over the ground{sea}", r - b.top(up)) }, 0),
                    ("Feet in", if space { "no block".into() } else { format!("face {} at {} {}, layer {}", blk.face, blk.pos.x, blk.pos.y, blk.pos.z) }, 0),
                    ("Facing", format!("yaw {yaw:.1}, pitch {pitch:.1}"), 0),
                    ("Looking at", look, 0),
                    ("Where", if space { "no latitude out here".into() } else { format!("{}, {:.0} m up", nav::geo_text(nav::geo(rel, self.meridian(bi))), r - b.radius) }, 0),
                ],
            ),
            section(
                "World",
                vec![
                    ("Time", format!("day {day}, {:02}:{:02} at the spawn, {:.0} s", m / 60, m % 60, w.time), 0),
                    ("Sol", format!("{:.1} degrees over the horizon", d.sun_deg), 0),
                    ("Moon", format!("{} {:.0}% lit", w.bodies.get(1).map_or("none", |b| b.name.as_str()), d.moon_lit * 100.0), 0),
                    ("Seed", format!("{}", self.seed), 0),
                    ("Size", w.bodies.iter().map(|b| format!("{} {}", b.name, b.n)).collect::<Vec<_>>().join(", ") + " columns a face", 0),
                ],
            ),
            section(
                "Player",
                vec![("Mode", if self.creative { "creative" } else { "survival" }.into(), 0), ("Tool", tool, 0), ("Slot", format!("{}: {block}", self.slot + 1), 0)],
            ),
        ];
        let right = vec![
            section(
                "Drawn",
                vec![
                    ("Chunks", format!("{} of {} kept", d.chunks, self.streamer.slot_count()), 0),
                    ("Quads", format!("{}", d.quads), 0),
                    ("Traced", format!("{} builds waiting", d.rt_pending), 0),
                    ("Missing", format!("{} chunks, {} holes", d.streaming.0, d.streaming.1), if d.streaming.1 > 0 { 2 } else { 0 }),
                    ("Loading", format!("{} requests, {} jobs running", d.streaming.2, d.streaming.3), 0),
                    ("Lamps", format!("{} near", self.lamps.len()), 0),
                    ("Water", format!("{} quads", self.wquad_n), 0),
                    ("Edits", format!("{} applied", self.edits_applied), 0),
                ],
            ),
            section(
                "Machine",
                vec![
                    ("GPU", gpu.adapter_name.clone(), 0),
                    ("Backend", format!("{} with ray queries", gpu.backend), 0),
                    ("Screen", format!("{}x{}", gpu.config.width, gpu.config.height), 0),
                    ("CPU", format!("{} workers", rayon::current_num_threads()), 0),
                    ("Build", format!("{BUILD} ({})", if cfg!(debug_assertions) { "debug" } else { "release" }), 0),
                ],
            ),
            section(
                "Session",
                vec![
                    ("Server", self.server_at.clone(), 0),
                    ("Save", self.save_path.clone().unwrap_or("none".into()), 0),
                    ("Recording", self.rec_path.clone().unwrap_or("none".into()), 0),
                ],
            ),
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
        // what the player sees of it at once: the hand swings and sparks of the block fly (ADR 0030)
        let seen = if dig { self.aim.map(|a| self.world.block(a.0)) } else { Some(m) };
        if let Some(d) = seen.and_then(|b| BLOCKS.get(b)) {
            self.acted = (Instant::now(), d.colour, self.acted.2.wrapping_add(17));
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
            .map(|m| {
                let d = BLOCKS.get(m).unwrap();
                // a grass block's sides are another block's (blocks.toml `side`)
                let side = BLOCKS.get(d.side).map_or(d.colour, |s| s.colour);
                hud::Slot { colour: d.colour, side, pattern: d.pattern as u8, count: (!self.creative).then(|| self.held_count(m)) }
            })
            .collect();
        let height = (self.hammer && self.tool.kind != Kind::Box).then_some(self.tool.height);
        // the title menu shows the orbit view and itself only
        // the title page and any open menu stand alone: no hotbar, chat or labels under them
        let title = self.home() == Menu::Title || self.menu.is_some();
        let (lw, lh) = self.ui_size();
        // the block in hand by name over the hotbar: full for two seconds after a change of slot, then gone in half
        if self.held_name.0 != self.slot {
            self.held_name = (self.slot, Instant::now());
        }
        let shown = (2.5 - self.held_name.1.elapsed().as_secs_f32()).clamp(0.0, 0.5) * 2.0;
        let held = self.slot_block(self.slot).and_then(|m| BLOCKS.get(m)).map(|d| (d.name.clone(), shown));
        let mut rects = if title { Vec::new() } else { hud::layer(lw, lh, &slots, self.slot, self.creative, height, held.as_ref().map(|h| (h.0.as_str(), h.1))) };
        if self.world.water_surface(p.pos).is_some() {
            // under water: a blue veil first, under the HUD. shortcut: a veil, not fog, until the sky has fog
            rects.insert(0, hud::Rect { xy: [-1.0, -1.0, 1.0, 1.0], colour: [0.05, 0.25, 0.4, 0.55], glyph: 0 });
        }
        // the text over it (ADR 0023): F3's numbers, the chat, the bug report's form
        let mut paint = hud::Paint::new(lw, lh);
        if self.f3 {
            let (l, r) = self.debug_sections();
            hud::panels(&mut paint, &l, &r);
        }
        let lines: Vec<(f32, &str)> = self.chat.iter().map(|(at, l)| (at.elapsed().as_secs_f32(), l.as_str())).collect();
        let caret = self.start.elapsed().as_millis() / 400 % 2 == 0;
        let (at, hints) = engine::net::complete(self.typing.as_deref().unwrap_or(""));
        if !title {
            hud::chat(&mut paint, &lines, self.typing.as_deref(), caret, at, &hints, self.pick);
        }
        // the compass and the waypoints (docs/design/navigation.md): creative's for now; in survival they are
        // earned with the first tool that gives them (the developer, 2026-11-11), which does not exist yet
        let mut label_y = 10.0;
        let (eye, fwd) = (p.pos, p.fwd);
        let on = self.world.frame_at(eye);
        if self.creative && !title && self.menu.is_none() && !self.f3 && on != engine::world::INERTIAL {
            let b = &self.world.bodies[on];
            let rel = eye - b.centre;
            let up = rel.normalize();
            let here: Vec<(usize, &nav::Waypoint)> = self.waypoints.iter().enumerate().filter(|w| w.1.body == on).collect();
            let marks: Vec<(f64, [f32; 3])> = here.iter().map(|(k, w)| (nav::bearing(up, w.rel - rel), nav::COLOURS[k % nav::COLOURS.len()])).collect();
            let heading = nav::bearing(up, fwd);
            let text = format!("{}  {}  {:.0} m up", b.name, nav::geo_text(nav::geo(rel, self.meridian(on))), rel.length() - b.radius);
            label_y = hud::compass(&mut paint, heading, &marks, &text) + 10.0;
            // this frame's view, as `frame` makes it after this: from the camera, the up kept off the look
            let cam_up = if fwd.cross(p.up).length() < 1e-4 { DVec3::Z } else { p.up };
            let proj = glam::camera::rh::proj::directx::perspective_infinite_reverse((self.fov() as f32).to_radians(), w() as f32 / h() as f32, 0.1);
            let vp = proj * glam::camera::rh::view::look_to_mat4(Vec3::ZERO, fwd.as_vec3(), cam_up.as_vec3());
            // 4. the nearest sixteen are drawn: more would crowd the view and the HUD's room for rectangles
            let mut here = here;
            here.sort_by(|a, b| (a.1.rel - rel).length_squared().total_cmp(&(b.1.rel - rel).length_squared()));
            here.truncate(16);
            for (k, w) in here {
                // a beam from where it stands to a height that grows with how far it is, so the tag stays
                // over the ground between: 3 m up close by, 60 m from 400 m on
                let far = (w.rel - rel).length();
                let rise = (far * 0.15).clamp(3.0, 60.0);
                let screen = |d: DVec3| {
                    let clip = vp * d.as_vec3().extend(1.0);
                    (clip.w > 0.01).then(|| ((clip.x / clip.w * 0.5 + 0.5) * lw, (0.5 - clip.y / clip.w * 0.5) * lh))
                };
                if let (Some(base), Some(top)) = (screen(w.rel - rel), screen(w.rel + w.rel.normalize() * rise - rel)) {
                    // a beam that would run up past the compass ends under it, its tag there
                    let roof = label_y + 96.0; // under the looked-at label too
                    let (base, top) = if top.1 >= roof {
                        (base, top)
                    } else if base.1 > roof {
                        let t = (base.1 - roof) / (base.1 - top.1);
                        (base, (base.0 + (top.0 - base.0) * t, roof))
                    } else {
                        // its foot is up there too: no beam, the tag where the foot points
                        ((base.0, roof), (base.0, roof))
                    };
                    if top.0 > -200.0 && top.0 < lw + 200.0 && top.1 >= roof - 0.5 && top.1 < lh + 100.0 {
                        let far = if far >= 1000.0 { format!("{:.1} km", far / 1000.0) } else { format!("{far:.0} m") };
                        hud::marker(&mut paint, base, top, nav::COLOURS[k % nav::COLOURS.len()], &w.name, &far);
                    }
                }
            }
        }
        // what the crosshair is on (the developer, 2026-11-11), when no menu or overlay is over the top
        if let (Some((hit, _)), None, false) = (self.aim, self.menu, self.f3 || title) {
            if let Some(d) = BLOCKS.get(self.world.block(hit)) {
                let lit = d.light.iter().any(|c| *c > 0.0);
                let note = match (lit, d.metal > 0.0) {
                    (true, true) => "gives light, metal",
                    (true, false) => "gives light",
                    (false, true) => "metal",
                    _ => "",
                };
                let side = BLOCKS.get(d.side).map_or(d.colour, |s| s.colour);
                hud::looking_at(&mut paint, &d.name, d.colour, side, d.pattern as u8, note, label_y);
            }
        }
        // the block in hand, large at the lower right, dipping when it digs or places; and the sparks
        let since = self.acted.0.elapsed().as_secs_f32();
        if !title {
            if let (Some(d), false) = (self.slot_block(self.slot).and_then(|m| BLOCKS.get(m)), self.hammer || self.bucket || self.f3 || lw < 1100.0) {
                let swing = if since < 0.18 { (since / 0.18 * std::f32::consts::PI).sin() } else { 0.0 };
                let side = BLOCKS.get(d.side).map_or(d.colour, |s| s.colour);
                let (x, y) = (lw - 250.0 - 46.0 * swing, lh - 236.0 + 60.0 * swing);
                hud::cube(&mut |x, y, w, h, c, g| paint.rect(x, y, w, h, c, g), x, y, 176.0, d.colour, side, d.pattern as u8, 1.0);
            }
            hud::sparks(&mut paint, since, self.acted.1, self.acted.2);
        }
        if self.flying && !title {
            let s = format!("Flying {:.0} m/s", self.fly_speed);
            // over the hotbar's right end (hud::layer: its pane ends 268 px right of the middle)
            paint.text_px(lw / 2.0 + 268.0 - s.len() as f32 * hud::ADV * hud::PX * 0.75, lh - 110.0, &s, [1.0, 1.0, 1.0, 0.9], hud::PX * 0.75);
        }
        if let Some(m) = self.menu {
            let (title, hint) = match m {
                Menu::Title => ("TechnoApe", "Pillars of Simulation Engine"),
                Menu::Worlds => ("Singleplayer", "A world carries on from its save. Right click or Delete removes one."),
                Menu::Remove(_) => ("Remove world", "It moves to saves/removed/ and can be put back by hand."),
                Menu::Waypoints => ("Waypoints", "N drops one where you stand. Right click or Delete takes one away."),
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
            // the page slides in over a tenth of a second, the lit row out as long (ADR 0030)
            let now = Instant::now();
            if self.lit.0 != hover {
                self.lit = (hover, now);
            }
            // (off screen the rows stand still, so a scripted run's pictures repeat; motion=1 shows it there)
            let moving = self.onscreen || self.args.get("motion", 0) == 1;
            let ease = if moving { ((now - self.menu_since).as_secs_f32() / 0.14, (now - self.lit.1).as_secs_f32() / 0.1) } else { (1.0, 1.0) };
            hud::menu(&mut paint, title, items, sliders, hint, hover, caret, self.scroll, ease);
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
