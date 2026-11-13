//! The globe map in the game (docs/design/navigation.md part 3, ADR 0077): M opens it as a menu page over
//! the world. A drag turns it, the wheel zooms, a click drops a waypoint or opens the one under the pointer,
//! a right click on one takes it away; Terra and Luna are a button apart. What it draws comes from
//! `globe.rs`; what the GPU gets from here is a 128-byte view a frame while it is open, and the seen-masks
//! when they changed.
use crate::app::App;
use crate::menu::Menu;
use crate::{globe, hud, nav};
use glam::DVec3;
use std::time::Instant;

/// The globe's pixel blocks, UI pixels.
const PX: f64 = 3.0;
/// The buttons down the left: each body, then back to the game.
const BUTTONS: [&str; 3] = ["Terra", "Luna", "Back to game"];

impl App {
    /// Every frame of play: marks what the player sees, takes the worker's pictures, and while the globe is
    /// open hands the GPU its view and the seen-masks.
    pub(crate) fn globe_frame(&mut self) {
        let on = self.world.frame_at(self.me.pos);
        // (marking costs 1 us walking, 1 ms from orbit: twice a second at the most, and only after a move)
        let due = self.globe.marked.is_none_or(|(t, at)| t.elapsed().as_secs_f64() > 0.5 && (at - self.me.pos).length() > 1.0);
        if on < 2 && due {
            let b = &self.world.bodies[on];
            let rel = self.me.pos - b.centre;
            let rho = globe::horizon(b.radius, rel.length() - b.height(rel.normalize()));
            if self.globe.seen[on].mark(rel, self.globe.zeros[on], rho) {
                self.globe.dirty = (true, true);
            }
            self.globe.marked = Some((Instant::now(), self.me.pos));
        }
        if self.globe.saved.elapsed().as_secs() >= 60 {
            self.globe_save();
        }
        let (lw, lh) = self.ui_size();
        let w = &self.world;
        let Some(gpu) = self.gpu.as_ref() else { return };
        if let Some(p) = self.globe.pictures.as_ref().and_then(|rx| rx.try_recv().ok()) {
            gpu.queue.write_buffer(&gpu.globe_buf, (globe::off(1) * 4) as u64, bytemuck::cast_slice(&p));
            self.globe.pictures = None;
        }
        if self.menu != Some(Menu::Globe) {
            return;
        }
        let g = &mut self.globe;
        (g.view.cx, g.view.cy, g.view.r) = (lw as f64 * 0.5, lh as f64 * 0.5, globe::fit(lh as f64) * g.zoom);
        let b = g.body;
        let sun = w.from_inertial(b, w.sky.sun(w.time)) - w.bodies[b].centre;
        let head = g.view.head(g.zeros[b], PX, sun, b, self.creative, lw as f64);
        gpu.queue.write_buffer(&gpu.globe_buf, 0, bytemuck::cast_slice(&head));
        if g.dirty.0 {
            let bits = [g.seen[0].bits.as_slice(), g.seen[1].bits.as_slice()].concat();
            gpu.queue.write_buffer(&gpu.globe_buf, (globe::off(3) * 4) as u64, bytemuck::cast_slice(&bits));
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

    /// The globe's inputs (`act`): map opens or closes it; globe:spin:dx:dy turns it as a drag of that many
    /// pixels, globe:zoom:f zooms by f, globe:click and globe:rclick click at the pointer (acts=).
    pub(crate) fn globe_act(&mut self, a: &str) {
        let num = |k: usize| a.split(':').nth(k).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
        match a {
            "map" if self.menu == Some(Menu::Globe) => self.set_menu(None),
            "map" if self.menu.is_none() && !self.texting() => {
                let w = &self.world;
                let on = w.frame_at(self.me.pos);
                let b = if on < 2 { on } else { 0 };
                let (lat, lon) = nav::geo(self.me.pos - w.bodies[b].centre, self.globe.zeros[b]);
                (self.globe.body, self.globe.zoom, self.globe.view.lat, self.globe.view.lon) = (b, 1.0, lat.clamp(-89.0, 89.0), lon);
                self.set_menu(Some(Menu::Globe));
            }
            _ if self.menu != Some(Menu::Globe) => {}
            a if a.starts_with("globe:spin:") => self.globe.view.drag(num(2), num(3)),
            a if a.starts_with("globe:zoom:") => self.globe.zoom = (self.globe.zoom * num(2)).clamp(1.0, globe::ZOOM_MAX),
            "globe:click" | "globe:rclick" => self.globe_click(a == "globe:rclick"),
            _ => {}
        }
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
            .filter_map(|(k, p)| g.view.project(p.rel, g.zeros[b]).map(|(px, py)| (k, (px - x).hypot(py - y))))
            .filter(|(_, d)| *d < 12.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(k, _)| k)
    }

    /// A click at the pointer: on a waypoint it opens its page (a right click takes it away), elsewhere on
    /// the globe it drops a new one on the ground there; survival only on land it has charted (ADR 0077).
    fn globe_click(&mut self, back: bool) {
        match (self.globe_mark_under(), back) {
            (Some(k), true) => self.remove_waypoint(k),
            (Some(k), false) => self.set_menu(Some(Menu::Waypoint(k))),
            (None, true) => {}
            (None, false) => {
                let b = self.globe.body;
                let Some(d) = self.globe.view.pick(self.pointer.0 as f64, self.pointer.1 as f64, self.globe.zeros[b]) else { return };
                let (lat, lon) = nav::geo(d, self.globe.zeros[b]);
                if !self.creative && !self.globe.seen[b].get(crate::globe::texel(lat, lon, crate::globe::SIZE[b])) {
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

    /// Button k of the globe's page.
    pub(crate) fn globe_button(&mut self, k: usize) {
        if k < 2 {
            self.globe.body = k;
            // the new body turned to where the player is when they are on it, else to its meridian
            let w = &self.world;
            let (lat, lon) = if w.frame_at(self.me.pos) == k { nav::geo(self.me.pos - w.bodies[k].centre, self.globe.zeros[k]) } else { (0.0, 0.0) };
            (self.globe.view.lat, self.globe.view.lon) = (lat.clamp(-89.0, 89.0), lon);
        } else {
            self.set_menu(None);
        }
    }

    pub(crate) fn globe_items(&self) -> Vec<String> {
        BUTTONS.iter().map(|s| s.to_string()).collect()
    }

    /// The globe's page (`build`): the globe, the marks on it, the buttons and what is under the pointer.
    /// Returns the buttons' boxes. `fwd` is where the player looks.
    pub(crate) fn globe_draw(&self, p: &mut hud::Paint, lw: f32, lh: f32, fwd: DVec3) -> Vec<[f32; 4]> {
        let (g, w) = (&self.globe, &self.world);
        let (b, zero, v) = (g.body, g.zeros[g.body], g.view);
        let body = &w.bodies[b];
        p.rect(0.0, 0.0, lw, lh, [0.02, 0.02, 0.05, 0.94], 0);
        let r = v.r as f32;
        p.rect(v.cx as f32 - r, v.cy as f32 - r, 2.0 * r, 2.0 * r, [1.0; 4], globe::GLYPH);
        if g.pictures.is_some() {
            p.text(v.cx as f32 - hud::text_width("drawing the map...") / 2.0, v.cy as f32, "drawing the map...", [1.0, 1.0, 1.0, 0.9]);
        }
        let at = |rel: DVec3| v.project(rel, zero).map(|(x, y)| (x as f32, y as f32));
        // the other players on this body, small and white with their names
        for o in &g.others {
            let pos = DVec3::from_array(o.pos);
            if o.id == g.id || w.frame_at(pos) != b {
                continue;
            }
            if let Some((x, y)) = at(pos - body.centre) {
                p.rect(x - 3.0, y - 3.0, 6.0, 6.0, [0.0, 0.0, 0.0, 0.8], 0);
                p.rect(x - 2.0, y - 2.0, 4.0, 4.0, [1.0, 1.0, 1.0, 1.0], 0);
                p.text_px(x + 6.0, y - 6.0, &o.name, [1.0, 1.0, 1.0, 0.85], hud::PX * 0.75);
            }
        }
        // the waypoints (K hides them)
        if self.settings.on(crate::settings::S::Marks) {
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
        if w.frame_at(self.me.pos) == b {
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
        // the buttons, the body's name, and the place under the pointer in the compass's notation
        let boxes: Vec<[f32; 4]> = (0..BUTTONS.len()).map(|k| [24.0, 70.0 + 46.0 * k as f32, 200.0, 38.0]).collect();
        for (k, bx) in boxes.iter().enumerate() {
            let lit = k == b || self.pointer.0 >= bx[0] && self.pointer.0 < bx[0] + bx[2] && self.pointer.1 >= bx[1] && self.pointer.1 < bx[1] + bx[3];
            p.rect(bx[0], bx[1], bx[2], bx[3], if lit { [0.95, 0.85, 0.45, 0.9] } else { [0.15, 0.15, 0.2, 0.85] }, 0);
            let ink = if lit { [0.05, 0.05, 0.08, 1.0] } else { [1.0, 1.0, 1.0, 0.95] };
            p.text_flat(bx[0] + 12.0, bx[1] + 11.0, BUTTONS[k], ink, hud::PX);
        }
        p.text_px(24.0, 20.0, &body.name, [1.0, 1.0, 1.0, 0.95], hud::PX * 1.5);
        // the coordinates under the pointer are creative's, like the compass's read-out (ADR 0077)
        let under = v.pick(self.pointer.0 as f64, self.pointer.1 as f64, zero).filter(|_| self.creative).map(|d| format!("{} {}", body.name, nav::geo_text(nav::geo(d, zero))));
        let hint = "Drag turns the globe, the wheel zooms. A click drops a waypoint or opens one, a right click takes one away. M or Esc closes.";
        p.text(24.0, lh - 34.0, hint, [0.8, 0.8, 0.85, 0.85]);
        if let Some(s) = under {
            p.text_px(24.0, lh - 34.0 - hud::LINE * 1.6, &s, [1.0, 1.0, 1.0, 0.95], hud::PX * 1.25);
        }
        boxes
    }
}
