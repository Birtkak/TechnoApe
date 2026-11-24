//! The globe map in the game (docs/design/navigation.md part 3, ADR 0077; the system view, ADR 0201, "The map
//! zooms out to the system"): M opens it as a menu page over the world. A drag turns the globe, the wheel zooms,
//! out past the whole globe into the system round the focused body (every body where the sky has it, its orbit,
//! its name); a click on the globe drops a waypoint or opens the one under the pointer, a right click on one takes
//! it away, a click on a body in the system focuses it; a button a body goes to its globe from any zoom. What it
//! draws comes from `globe.rs`; what the GPU gets from here is a 128-byte view a frame while it is open, the
//! pictures as they are made and the seen-masks when they changed.
use crate::app::App;
use crate::menu::Menu;
use crate::{globe, hud, nav};
use engine::world::INERTIAL;
use glam::DVec3;
use std::time::Instant;

/// The globe's pixel blocks, UI pixels.
const PX: f64 = 3.0;
/// The UI rectangle's glyph of a plainly shaded ball (overlay.wgsl): a body with no picture, any body in the system.
const GLYPH_DISC: u32 = 201;
/// Under this radius, pixels, the focused body is a ball, not the globe's picture; its marks show from `MARKS_PX`.
const GLOBE_PX: f64 = 24.0;
const MARKS_PX: f64 = 40.0;
/// Columns worked out again a frame after edits (`globe::State::refresh`).
const TOPS_A_FRAME: usize = 64;
/// The most edited columns drawn a frame (`globe::State::visible`): the UI's rectangles are `hud::RECTS_MAX`.
const EDITS_MAX: usize = 8000;
/// The sun's colour on the map.
const SUN: [f32; 3] = [1.0, 0.86, 0.45];

impl App {
    /// Every frame of play: marks what the player sees, starts and takes the pictures of the bodies whose maps are
    /// in, works out edited columns under a budget, and while the map is open hands the GPU its view and masks.
    pub(crate) fn globe_frame(&mut self) {
        let on = self.world.frame_at(self.me.pos);
        // (marking costs 1 us walking, 1 ms from orbit: twice a second at the most, and only after a move)
        let due = self.globe.marked.is_none_or(|(t, at)| t.elapsed().as_secs_f64() > 0.5 && (at - self.me.pos).length() > 1.0);
        if due && self.globe.seen.get(on).is_some_and(Option::is_some) {
            let b = &self.world.bodies[on];
            let rel = self.me.pos - b.centre;
            let rho = globe::horizon(b.radius, rel.length() - b.height(rel.normalize()));
            if self.globe.seen[on].as_mut().unwrap().mark(rel, self.globe.zeros[on], rho) {
                self.globe.dirty = (true, true);
            }
            self.globe.marked = Some((Instant::now(), self.me.pos));
        }
        if self.globe.saved.elapsed().as_secs() >= 60 {
            self.globe_save();
        }
        // a body's picture once its map is in (a body loaded on approach, ADR 0175, when it comes)
        for b in 0..self.world.bodies.len().min(self.globe.asked.len()) {
            if !self.globe.asked[b] && self.world.loaded(b) {
                if let Some(m) = self.world.bodies[b].map().cloned() {
                    self.globe.ask(b, m);
                }
            }
        }
        // a body loaded on approach that is not in yet: its map alone, on the worker, once it is looked at
        let b = self.globe.body;
        if self.menu == Some(Menu::Globe) && !self.globe.asked.get(b).is_none_or(|a| *a) {
            if let Some(l) = self.world.lazy.get(b).and_then(Option::as_ref).filter(|l| !l.loaded) {
                let body = &self.world.bodies[b];
                let (row, seed, n, hash) = (engine::bodies::of(body.gen)[b].clone(), l.seed, body.n, l.hash);
                self.globe.ask_with(b, move || row.bake.as_ref()?.for_hash(seed, n, hash));
            }
        }
        self.globe.refresh(&self.world, TOPS_A_FRAME);
        let (lw, lh) = self.ui_size();
        let w = &self.world;
        let Some(gpu) = self.gpu.as_ref() else { return };
        while let Some((b, p)) = self.globe.take() {
            let at = self.globe.slots[b].expect("a picture is asked only of a body with a slot").pic;
            gpu.queue.write_buffer(&gpu.globe_buf, (at * 4) as u64, bytemuck::cast_slice(&p));
        }
        if self.menu != Some(Menu::Globe) {
            return;
        }
        let g = &mut self.globe;
        (g.view.cx, g.view.cy, g.view.r) = (lw as f64 * 0.5, lh as f64 * 0.5, globe::fit(lh as f64) * g.zoom);
        let b = g.body;
        if let Some(slot) = g.slots.get(b).copied().flatten() {
            let sun = w.from_inertial(b, w.sky.sun(w.time)) - w.bodies[b].centre;
            let head = g.view.head(g.zeros[b], PX, sun, slot, self.creative, lw as f64, w.bodies[b].air, g.relief[b]);
            gpu.queue.write_buffer(&gpu.globe_buf, 0, bytemuck::cast_slice(&head));
        }
        if g.dirty.0 {
            if let Some(first) = g.slots.iter().flatten().next() {
                gpu.queue.write_buffer(&gpu.globe_buf, (first.mask * 4) as u64, bytemuck::cast_slice(&g.masks()));
            }
            g.dirty.0 = false;
        }
    }

    /// Keeps what was seen beside the save; says so when it cannot.
    pub(crate) fn globe_save(&mut self) {
        self.globe.saved = Instant::now();
        if let Err(e) = self.globe.save() {
            self.note(format!("the map of what you have seen could not be saved: {e}"));
        }
    }

    /// The least zoom on the focused body: the whole system in view (`globe::least`).
    fn globe_least(&self) -> f64 {
        let b = self.globe.body;
        globe::least(self.world.bodies[b].radius, globe::extent(&self.world, b))
    }

    /// The globe's inputs (`act`): map opens or closes it; globe:spin:dx:dy turns it as a drag of that many
    /// pixels, globe:zoom:f zooms by f, globe:zoomto:z sets the zoom (1 the whole globe, under 1 the system, 0 the
    /// whole system), globe:focus:<body> goes to that body's globe as its button does, globe:click and
    /// globe:rclick click at the pointer (acts=).
    pub(crate) fn globe_act(&mut self, a: &str) {
        let num = |k: usize| a.split(':').nth(k).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
        match a {
            "map" if self.menu == Some(Menu::Globe) => self.set_menu(None),
            "map" if self.menu.is_none() && !self.texting() => {
                let on = self.world.frame_at(self.me.pos);
                self.globe_focus(if on < self.world.bodies.len() { on } else { 0 }, true);
                self.set_menu(Some(Menu::Globe));
            }
            _ if self.menu != Some(Menu::Globe) => {}
            a if a.starts_with("globe:spin:") => self.globe.view.drag(num(2), num(3)),
            a if a.starts_with("globe:zoom:") => self.globe.zoom = (self.globe.zoom * num(2)).clamp(self.globe_least(), globe::ZOOM_MAX),
            a if a.starts_with("globe:zoomto:") => self.globe.zoom = num(2).clamp(self.globe_least(), globe::ZOOM_MAX),
            a if a.starts_with("globe:focus:") => {
                let name = &a["globe:focus:".len()..];
                match self.world.bodies.iter().position(|b| b.name.eq_ignore_ascii_case(name)) {
                    Some(k) => self.globe_focus(k, false),
                    None => self.note(format!("no body '{name}' on the map")),
                }
            }
            "globe:click" | "globe:rclick" => self.globe_click(a == "globe:rclick"),
            _ => {}
        }
    }

    /// Body k's globe, whole (zoom 1); a body newly focused (or `fresh`, as the map opens) turned to where the player
    /// is when they are on it, else to its meridian, the focused one left as it was turned.
    fn globe_focus(&mut self, k: usize, fresh: bool) {
        let w = &self.world;
        if fresh || k != self.globe.body {
            let (lat, lon) = if w.frame_at(self.me.pos) == k { nav::geo(self.me.pos - w.bodies[k].centre, self.globe.zeros[k]) } else { (0.0, 0.0) };
            (self.globe.view.lat, self.globe.view.lon) = (lat.clamp(-89.0, 89.0), lon);
        }
        (self.globe.body, self.globe.zoom) = (k, 1.0);
    }

    /// A press on the globe (not on a button): a drag begins; it is a click if the pointer has not moved
    /// when it is let go (`globe_release`).
    pub(crate) fn globe_press(&mut self, back: bool) {
        if back {
            self.globe_click(true);
        } else {
            self.globe.drag = Some((self.pointer, false));
        }
    }

    /// The pointer moved to `to` (UI pixels): a drag turns the globe with it.
    pub(crate) fn globe_move(&mut self, to: (f32, f32)) {
        if let Some(((x, y), moved)) = self.globe.drag {
            let (dx, dy) = ((to.0 - self.pointer.0) as f64, (to.1 - self.pointer.1) as f64);
            let moved = moved || (to.0 - x).hypot(to.1 - y) > 4.0;
            self.globe.drag = Some(((x, y), moved));
            if moved {
                self.globe.view.drag(dx, dy);
            }
        }
    }

    pub(crate) fn globe_release(&mut self) {
        if let Some((_, false)) = self.globe.drag.take() {
            self.globe_click(false);
        }
    }

    /// The waypoint drawn nearest the pointer on the shown body, within 12 pixels.
    fn globe_mark_under(&self) -> Option<usize> {
        let (b, g) = (self.globe.body, &self.globe);
        let (x, y) = (self.pointer.0 as f64, self.pointer.1 as f64);
        self.waypoints
            .iter()
            .enumerate()
            .filter(|(_, p)| p.body == b)
            .filter_map(|(k, p)| g.at(b, &g.view, p.rel).map(|(px, py)| (k, (px - x).hypot(py - y))))
            .filter(|(_, d)| *d < 12.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(k, _)| k)
    }

    /// The system view's dots now (`globe::dots`): every body, then the sun.
    fn globe_dots(&self) -> Vec<globe::Dot> {
        let (g, w) = (&self.globe, &self.world);
        globe::dots(w, g.body, (g.view.cx, g.view.cy), g.view.r / w.bodies[g.body].radius)
    }

    /// A click at the pointer. In the system: on a body it focuses it, as near as it was (the same pixels a
    /// metre); on the focused body's disc once its marks show, it is the globe's. On the globe: on a waypoint it opens its page (a right click takes it away), elsewhere it drops a
    /// new one on the ground there; survival only on land it has charted (ADR 0077).
    fn globe_click(&mut self, back: bool) {
        if globe::system(self.globe.zoom) {
            let d = self.globe_dots();
            match globe::system_click(&d[..self.world.bodies.len()], self.globe.body, self.pointer.0 as f64, self.pointer.1 as f64, MARKS_PX) {
                globe::SystemClick::Globe => {}
                globe::SystemClick::Focus(k) if !back => {
                    let (was, zoom) = (self.world.bodies[self.globe.body].radius, self.globe.zoom);
                    self.globe_focus(k, false);
                    self.globe.zoom = (zoom * self.world.bodies[k].radius / was).clamp(self.globe_least(), globe::ZOOM_MAX);
                    return;
                }
                _ => return,
            }
        }
        match (self.globe_mark_under(), back) {
            (Some(k), true) => self.remove_waypoint(k),
            (Some(k), false) => self.set_menu(Some(Menu::Waypoint(k))),
            (None, true) => {}
            (None, false) => {
                let b = self.globe.body;
                let Some(Some(seen)) = self.globe.seen.get(b) else { return };
                let Some(d) = self.globe.pick(b, &self.globe.view, self.pointer.0 as f64, self.pointer.1 as f64) else { return };
                let (lat, lon) = nav::geo(d, self.globe.zeros[b]);
                if !self.creative && !seen.get(globe::texel(lat, lon, seen.size)) {
                    self.note("that land is not charted yet: go and see it first".into());
                    return;
                }
                let body = &self.world.bodies[b];
                let rel = body.stand_point(d) - body.centre;
                let name = nav::next_name(&self.waypoints);
                let line = self.waypoint_line(&name, rel, b);
                self.waypoints.push(nav::Waypoint::new(name, b, rel, self.waypoints.len()));
                self.note(line);
                self.save_waypoints();
            }
        }
    }

    /// The bodies in the buttons' order, each with how deep a moon it is: every body that circles the sun in row
    /// order, its moons (and theirs) under it.
    fn globe_order(&self) -> Vec<(usize, usize)> {
        globe::order(&self.world.bodies.iter().map(|b| b.parent).collect::<Vec<_>>())
    }

    /// Button k of the globe's page: a body's (its globe, from any zoom), then Back.
    pub(crate) fn globe_button(&mut self, k: usize) {
        match self.globe_order().get(k) {
            Some(&(b, _)) => self.globe_focus(b, false),
            None => self.set_menu(None),
        }
    }

    pub(crate) fn globe_items(&self) -> Vec<String> {
        // each body's name, and how many players are on it when any are
        let on = globe::players_on(&self.world, &self.globe.others, self.globe.id);
        let named = |b: usize| match on[b].len() {
            0 => self.world.bodies[b].name.clone(),
            n => format!("{} ({n})", self.world.bodies[b].name),
        };
        self.globe_order().iter().map(|&(b, _)| named(b)).chain(["Back to game".to_string()]).collect()
    }

    /// The buttons' boxes: a body's a row, a moon's indented under its planet, then Back.
    fn globe_boxes(&self) -> Vec<[f32; 4]> {
        let order = self.globe_order();
        let mut boxes: Vec<[f32; 4]> = order.iter().enumerate().map(|(k, &(_, d))| [24.0 + 18.0 * d as f32, 70.0 + 36.0 * k as f32, 200.0 - 18.0 * d as f32, 30.0]).collect();
        boxes.push([24.0, 78.0 + 36.0 * order.len() as f32, 200.0, 30.0]);
        boxes
    }

    /// The globe's page (`build`): the system when zoomed out past the globe, the focused globe, the marks on
    /// it, the buttons and what is under the pointer. Returns the buttons' boxes. `fwd` is where the player looks.
    pub(crate) fn globe_draw(&self, p: &mut hud::Paint, lw: f32, lh: f32, fwd: DVec3) -> Vec<[f32; 4]> {
        let (g, w) = (&self.globe, &self.world);
        let (b, zero, v) = (g.body, g.zeros[g.body], g.view);
        let body = &w.bodies[b];
        let s = v.r / body.radius;
        let sys = globe::system(g.zoom);
        let row = |i: usize| engine::bodies::BODIES.get(i).map_or([0.7; 3], |r| r.colour);
        p.rect(0.0, 0.0, lw, lh, [0.02, 0.02, 0.05, 0.94], 0);
        let n = w.bodies.len();
        let dots = if sys { self.globe_dots() } else { Vec::new() };
        let ball = |p: &mut hud::Paint, d: &globe::Dot, c: [f32; 3]| p.rect((d.x - d.r) as f32, (d.y - d.r) as f32, 2.0 * d.r as f32, 2.0 * d.r as f32, [c[0], c[1], c[2], 1.0], GLYPH_DISC);
        let on_screen = |x: f64, y: f64, m: f64| x > -m && y > -m && x < lw as f64 + m && y < lh as f64 + m;
        if sys {
            // the orbits: each its body's colour, a line of 2 px dots a dot's width apart, clipped to the screen, round
            // its parent where it is now (made each frame, `globe::orbit`, so it runs through the body); a moon's too
            // small to tell from its planet is left out
            let c = w.pose(b, w.time).centre;
            let mut left = 12000;
            for i in 0..n {
                let line = globe::orbit(w, i, globe::ORBIT_POINTS);
                let parent = if w.bodies[i].parent == INERTIAL { w.sky.sun(w.time) } else { w.pose(w.bodies[i].parent, w.time).centre };
                let xy = |o: DVec3| (v.cx + (parent + o - c).x * s, v.cy - (parent + o - c).y * s);
                if line[0].length() * s < 8.0 {
                    continue;
                }
                let col = row(i);
                for k in 0..line.len() - 1 {
                    let Some((a, e)) = globe::clip(xy(line[k]), xy(line[k + 1]), (lw as f64, lh as f64)) else { continue };
                    let steps = (((e.0 - a.0).hypot(e.1 - a.1) / 2.0).ceil().max(1.0) as usize).min(left);
                    for t in 0..steps {
                        let u = t as f64 / steps as f64;
                        let (x, y) = (a.0 + (e.0 - a.0) * u, a.1 + (e.1 - a.1) * u);
                        p.rect(x as f32 - 1.0, y as f32 - 1.0, 2.0, 2.0, [col[0], col[1], col[2], 0.5], 0);
                    }
                    left -= steps;
                }
            }
            // every body but the focused one (drawn next, as its globe or ball), and the sun
            for (i, d) in dots.iter().enumerate().filter(|&(i, d)| i != b && on_screen(d.x, d.y, d.r)) {
                ball(p, d, if i == n { SUN } else { row(i) });
            }
        }
        // the focused body: its picture once made, else a ball in its colour
        let picture = g.slots.get(b).is_some_and(Option::is_some) && g.got[b];
        if picture && v.r >= GLOBE_PX {
            let r = v.r as f32;
            p.rect(v.cx as f32 - r, v.cy as f32 - r, 2.0 * r, 2.0 * r, [1.0; 4], globe::GLYPH);
        } else {
            ball(p, &globe::Dot { x: v.cx, y: v.cy, r: v.r.max(globe::MIN_PX) }, row(b));
        }
        if !sys && g.asked[b] && !g.got[b] {
            p.text(v.cx as f32 - hud::text_width("drawing the map...") / 2.0, v.cy as f32, "drawing the map...", [1.0, 1.0, 1.0, 0.9]);
        }
        // a mark sits on the ground the shader draws, its relief included
        let at = |rel: DVec3| g.at(b, &v, rel).map(|(x, y)| (x as f32, y as f32));
        // what building and digging made of the columns the client was told of, once a block is a pixel (a block at
        // the least `PX` wide, so one shows); survival's only on land it has seen
        if !sys && s >= globe::EDITS_FROM {
            let seen = g.seen.get(b).and_then(Option::as_ref);
            let k = (s.max(PX) as f32).ceil();
            for t in g.visible(b, body, &v, (lw as f64, lh as f64), EDITS_MAX) {
                let (lat, lon) = nav::geo(t.dir, zero);
                if !self.creative && seen.is_none_or(|m| !m.get(globe::texel(lat, lon, m.size))) {
                    continue;
                }
                if let Some((x, y)) = at(t.dir * body.radius).filter(|&(x, y)| on_screen(x as f64, y as f64, k as f64)) {
                    let c = globe::top_colour(&t);
                    p.rect(x - k / 2.0, y - k / 2.0, k, k, [c[0], c[1], c[2], 1.0], 0);
                }
            }
        }
        let marks = v.r >= MARKS_PX;
        // the other players on this body, small and white with their names
        for o in &g.others {
            let pos = DVec3::from_array(o.pos);
            if o.id == g.id || w.frame_at(pos) != b || !marks {
                continue;
            }
            if let Some((x, y)) = at(pos - body.centre) {
                p.rect(x - 3.0, y - 3.0, 6.0, 6.0, [0.0, 0.0, 0.0, 0.8], 0);
                p.rect(x - 2.0, y - 2.0, 4.0, 4.0, [1.0, 1.0, 1.0, 1.0], 0);
                p.text_px(x + 6.0, y - 6.0, &o.name, [1.0, 1.0, 1.0, 0.85], hud::PX * 0.75);
            }
        }
        // the waypoints (K hides them)
        if self.settings.on(crate::settings::S::Marks) && marks {
            for wp in self.waypoints.iter().filter(|wp| wp.body == b) {
                if let Some((x, y)) = at(wp.rel) {
                    let c = nav::COLOURS[wp.colour % nav::COLOURS.len()];
                    hud::icon(p, x + 1.5, y + 1.5, 8.0, wp.icon, [0.0, 0.0, 0.0, 0.8]);
                    hud::icon(p, x, y, 8.0, wp.icon, [c[0], c[1], c[2], 1.0]);
                    p.text_px(x + 11.0, y - 6.0, &wp.name, [c[0], c[1], c[2], 0.95], hud::PX * 0.75);
                }
            }
        }
        // you: a ring with a tail the way you look
        let mine = w.frame_at(self.me.pos);
        if mine == b && marks {
            let rel = self.me.pos - body.centre;
            if let Some((x, y)) = at(rel) {
                let up = rel.normalize();
                let ahead = (fwd - up * fwd.dot(up)).normalize_or_zero();
                if let Some((ax, ay)) = at(rel + ahead * body.radius * 0.02 / g.zoom) {
                    let (dx, dy) = (ax - x, ay - y);
                    let len = dx.hypot(dy).max(1e-3);
                    for k in 1..=4 {
                        let s = 8.0 + 5.0 * k as f32;
                        p.rect(x + dx / len * s - 3.0, y + dy / len * s - 3.0, 6.0, 6.0, [0.0, 0.0, 0.0, 0.8], 0);
                        p.rect(x + dx / len * s - 2.0, y + dy / len * s - 2.0, 4.0, 4.0, [1.0, 0.85, 0.2, 1.0], 0);
                    }
                }
                p.rect(x - 9.0, y - 9.0, 18.0, 18.0, [0.0, 0.0, 0.0, 0.7], 0);
                hud::icon(p, x, y, 8.0, 3, [1.0, 0.85, 0.2, 1.0]);
                p.text_px(x + 12.0, y + 6.0, "you", [1.0, 0.85, 0.2, 0.95], hud::PX * 0.75);
            }
        }
        if sys {
            // every body's name, a moon on its planet named with it (`globe::labels`)
            let names: Vec<String> = w.bodies.iter().map(|b| b.name.clone()).chain(["Sun".to_string()]).collect();
            let width = |s: &str| (hud::text_width(s) * 0.75) as f64;
            for (d, l) in dots.iter().zip(globe::labels(&dots, &names, width)) {
                if let Some(l) = l.filter(|_| on_screen(d.x, d.y, d.r)) {
                    let (x, y) = ((d.x + d.r) as f32 + 5.0, d.y as f32 - 6.0);
                    p.text_px(x + 1.0, y + 1.0, &l, [0.0, 0.0, 0.0, 0.8], hud::PX * 0.75);
                    p.text_px(x, y, &l, [0.92, 0.92, 0.96, 0.95], hud::PX * 0.75);
                }
            }
            // every player: under each body's ball who is on it (the focused globe marks its own when it is large),
            // one out in space a dot where it is (the sky's frame), you a ring there
            for (i, names) in globe::players_on(w, &g.others, g.id).iter().enumerate() {
                let (Some(t), Some(d)) = (globe::crowd(names), dots.get(i)) else { continue };
                if (i == b && marks) || !on_screen(d.x, d.y, d.r) {
                    continue;
                }
                let ink = if names.iter().any(|n| n == "you") { [1.0, 0.85, 0.2, 0.95] } else { [1.0, 1.0, 1.0, 0.95] };
                let (x, y) = (d.x as f32 - 3.0, (d.y + d.r) as f32 + 6.0);
                p.rect(x - 1.0, y - 1.0, 8.0, 8.0, [0.0, 0.0, 0.0, 0.8], 0);
                p.rect(x, y, 6.0, 6.0, ink, 0);
                p.text_px(x + 10.0, y - 1.0, &t, ink, hud::PX * 0.75);
            }
            let c = w.pose(b, w.time).centre;
            let xy = |pos: DVec3| {
                let x = w.to_inertial(pos) - c;
                ((v.cx + x.x * s) as f32, (v.cy - x.y * s) as f32)
            };
            for o in g.others.iter().filter(|o| o.id != g.id && w.frame_at(DVec3::from_array(o.pos)) == INERTIAL) {
                let (x, y) = xy(DVec3::from_array(o.pos));
                p.rect(x - 2.0, y - 2.0, 4.0, 4.0, [1.0, 1.0, 1.0, 1.0], 0);
            }
            if mine == INERTIAL {
                let (x, y) = xy(self.me.pos);
                p.rect(x - 7.0, y - 7.0, 14.0, 14.0, [0.0, 0.0, 0.0, 0.6], 0);
                hud::icon(p, x, y, 6.0, 3, [1.0, 0.85, 0.2, 1.0]);
                p.text_px(x - 8.0, y + 10.0, "you", [1.0, 0.85, 0.2, 0.95], hud::PX * 0.75);
            }
        }
        // the buttons (the focused body's lit), the body's name, and the place under the pointer in the compass's
        // notation
        let boxes = self.globe_boxes();
        let items = self.globe_items();
        let order = self.globe_order();
        for (k, bx) in boxes.iter().enumerate() {
            let lit = order.get(k).is_some_and(|o| o.0 == b) || self.pointer.0 >= bx[0] && self.pointer.0 < bx[0] + bx[2] && self.pointer.1 >= bx[1] && self.pointer.1 < bx[1] + bx[3];
            p.rect(bx[0], bx[1], bx[2], bx[3], if lit { [0.95, 0.85, 0.45, 0.9] } else { [0.15, 0.15, 0.2, 0.85] }, 0);
            let ink = if lit { [0.05, 0.05, 0.08, 1.0] } else { [1.0, 1.0, 1.0, 0.95] };
            p.text_flat(bx[0] + 12.0, bx[1] + 8.0, &items[k], ink, hud::PX);
        }
        p.text_px(24.0, 20.0, &body.name, [1.0, 1.0, 1.0, 0.95], hud::PX * 1.5);
        // the coordinates under the pointer are creative's, like the compass's read-out (ADR 0077)
        let under = g.pick(b, &v, self.pointer.0 as f64, self.pointer.1 as f64).filter(|_| self.creative && !sys).map(|d| format!("{} {}", body.name, nav::geo_text(nav::geo(d, zero))));
        let hint = if sys {
            "The system: a click on a body focuses it, the wheel zooms in to its globe. A button goes to a body's globe. M or Esc closes."
        } else {
            "Drag turns the globe, the wheel zooms (out past the globe: the system). A click drops a waypoint or opens one, a right click takes one away. M or Esc closes."
        };
        p.text(24.0, lh - 34.0, hint, [0.8, 0.8, 0.85, 0.85]);
        if let Some(s) = under {
            p.text_px(24.0, lh - 34.0 - hud::LINE * 1.6, &s, [1.0, 1.0, 1.0, 0.95], hud::PX * 1.25);
        }
        boxes
    }
}
