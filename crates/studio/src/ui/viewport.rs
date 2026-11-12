//! The game's renderer in a page (ADR 0049): the game itself runs as `technoape run=view`, off-screen on its own
//! device, and draws the scene the Studio describes on its stdin (camera, time of day, model, blocks); each frame
//! comes back on its stdout and is shown as one egui texture. One renderer for every page (`App::view`); a page
//! sets what it shows, then calls [`Viewport::ui`]. Until the first frame (a few seconds) the page's own 2D
//! preview stands in, and the Studio never waits for it, except `shot=` ([`Viewport::settle`]).
use super::look::{self, GAP};
use crate::voxel::Orbit;
use eframe::egui::{self, Color32, Pos2, Rect, RichText, Sense, Ui, Vec2};
use glam::Vec3;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// `renderer=` on the Studio's command line: the game's exe to draw with.
pub static RENDERER: OnceLock<String> = OnceLock::new();
/// Where the Studio puts what the renderer reads: its settings, the open model, the textures in memory.
pub const DIR: &str = "out/studio-view";

struct Frame {
    id: u64,
    w: u32,
    h: u32,
    fov: f32,
    px: Vec<u8>,
}

struct Proc {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<Frame>,
    log: Arc<Mutex<Vec<String>>>,
}

/// Where the thing stands.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Scene {
    Grass,
    Cave,
}
pub const SCENES: [(Scene, &str, &str); 2] = [
    (Scene::Grass, "Grass", "On the grass at the spawn, in the open under the sky"),
    (Scene::Cave, "Cave", "In a dark stone room beside a lamp: block light only"),
];
impl Scene {
    /// The blocks that make it, around the stand cell.
    fn blocks(self) -> String {
        match self {
            Scene::Grass => String::new(),
            Scene::Cave => {
                let mut v = vec![];
                for di in -3..=3i32 {
                    for dj in -3..=3i32 {
                        for dl in -1..=4 {
                            let wall = di.abs() == 3 || dj.abs() == 3 || dl == -1 || dl == 4;
                            v.push(format!("{di}:{dj}:{dl}:{}", if wall { "stone" } else { "_" }));
                        }
                    }
                }
                v.push("2:-2:3:lamp".into());
                v.join(",")
            }
        }
    }
}

enum State {
    Off,
    Starting,
    Up,
    Failed(String),
}

pub struct Viewport {
    pub orbit: Orbit,
    /// The hour of the day, 0..24; the game's clock starts at 9 (ADR 0022).
    pub hour: f32,
    pub scene: Scene,
    /// A drag with the left button turns too (the right one always does).
    pub left_turns: bool,
    proc: Option<Proc>,
    state: State,
    tex: Option<egui::TextureHandle>,
    /// The last frame's size and vertical field of view.
    img: [u32; 2],
    fov: f32,
    /// What the renderer was last told, by kind, so only changes go.
    told: HashMap<&'static str, String>,
    /// The page's own blocks around the stand cell.
    pub place: String,
    next_id: u64,
    pending: Option<(u64, Instant)>,
    /// Frames asked for since the last change: the renderer's light settles over a few dozen.
    fresh: u32,
    /// Keep frames coming (a clip playing).
    pub live: bool,
    /// The effect to play this frame and a count of its restarts: set by the page before each `ui`; a page that
    /// does not set it shows none.
    pub effect: Option<(String, u64)>,
    shown_effect: Option<(String, u64)>,
    started: Option<Instant>,
    startup_s: Option<f32>,
    delays: Vec<f32>,
    /// The image's rectangle on screen and the part of the frame it shows (cropped to fill).
    shown: Option<(Rect, Rect)>,
}

impl Viewport {
    pub fn new() -> Viewport {
        Viewport {
            orbit: Orbit::default(),
            hour: 11.0,
            scene: Scene::Grass,
            left_turns: true,
            proc: None,
            state: State::Off,
            tex: None,
            img: [0, 0],
            fov: 70.0,
            told: HashMap::new(),
            place: String::new(),
            next_id: 1,
            pending: None,
            fresh: 0,
            live: false,
            effect: None,
            shown_effect: None,
            started: None,
            startup_s: None,
            delays: vec![],
            shown: None,
        }
    }

    pub fn is_up(&self) -> bool {
        matches!(self.state, State::Up)
    }

    fn exe() -> Option<std::path::PathBuf> {
        // `view=0`: no game view, the pages' 2D fallback
        if RENDERER.get().is_some_and(|r| r == "0") {
            return None;
        }
        let own = std::env::current_exe().ok().map(|e| e.with_file_name("technoape.exe"));
        let list = [RENDERER.get().map(std::path::PathBuf::from), Some("target/release/technoape.exe".into()), own, Some("build/TechnoApe.exe".into())];
        list.into_iter().flatten().find(|p| p.is_file())
    }

    fn start(&mut self, w: u32, h: u32) {
        let Some(exe) = Self::exe() else {
            self.state = State::Failed("no game exe found (target/release/technoape.exe, beside the Studio, build/TechnoApe.exe or renderer=)".into());
            return;
        };
        let _ = std::fs::create_dir_all(format!("{DIR}/models"));
        let _ = std::fs::write(format!("{DIR}/settings.txt"), format!("width={w}\nheight={h}\n"));
        let mut cmd = Command::new(&exe);
        cmd.args(["run=view", &format!("settings={DIR}/settings.txt"), "save=0", "record=0", "sound=0", "fx=1", &format!("models={DIR}/models")])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // no console window
        }
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                self.state = State::Failed(format!("{}: {e}", exe.display()));
                return;
            }
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let mut out = BufReader::new(child.stdout.take().unwrap());
        std::thread::spawn(move || {
            let mut line = String::new();
            loop {
                line.clear();
                if out.read_line(&mut line).unwrap_or(0) == 0 {
                    return;
                }
                let f: Vec<&str> = line.split_whitespace().collect();
                if f.len() != 5 || f[0] != "frame" {
                    continue;
                }
                let (Ok(id), Ok(w), Ok(h), Ok(fov)) = (f[1].parse(), f[2].parse::<u32>(), f[3].parse::<u32>(), f[4].parse()) else { continue };
                let mut px = vec![0u8; (w * h * 4) as usize];
                if out.read_exact(&mut px).is_err() || tx.send(Frame { id, w, h, fov, px }).is_err() {
                    return;
                }
            }
        });
        let log = Arc::new(Mutex::new(Vec::new()));
        let l = log.clone();
        let err = BufReader::new(child.stderr.take().unwrap());
        std::thread::spawn(move || {
            for line in err.lines().map_while(Result::ok) {
                let mut l = l.lock().unwrap();
                l.push(line);
                let n = l.len().saturating_sub(30);
                l.drain(..n);
            }
        });
        let stdin = child.stdin.take().unwrap();
        self.proc = Some(Proc { child, stdin, rx, log });
        self.state = State::Starting;
        self.started = Some(Instant::now());
        self.told.clear();
        self.told.insert("size", format!("{w} {h}"));
    }

    fn write(&mut self, line: &str) {
        let Some(p) = &mut self.proc else { return };
        if writeln!(p.stdin, "{line}").and_then(|_| p.stdin.flush()).is_err() {
            self.fail();
        }
    }

    /// Tells the renderer `line` when `value` differs from what it was last told under `kind`.
    fn say(&mut self, kind: &'static str, value: String, line: String) {
        if self.told.get(kind) != Some(&value) {
            self.write(&line);
            self.told.insert(kind, value);
            self.fresh = 0;
        }
    }

    fn fail(&mut self) {
        let why = self.proc.as_ref().map(|p| p.log.lock().unwrap().iter().rev().take(3).rev().cloned().collect::<Vec<_>>().join(" / ")).unwrap_or_default();
        if let Some(mut p) = self.proc.take() {
            let _ = p.child.kill();
        }
        self.state = State::Failed(if why.is_empty() { "the renderer stopped".into() } else { format!("the renderer stopped: {why}") });
    }

    /// The models folder changed (the page wrote its model there): read again. `generation` counts the writes.
    pub fn models(&mut self, generation: u64) {
        self.say("models", generation.to_string(), format!("models {DIR}/models"));
    }
    /// The textures the renderer reads: `dir` written anew (`generation` counts the writes).
    pub fn textures(&mut self, dir: &str, generation: u64) {
        self.say("textures", format!("{dir} {generation}"), format!("textures {dir}"));
    }
    /// The effects file the renderer plays from: `file` written anew (`generation` counts the writes).
    pub fn effects(&mut self, file: &str, generation: u64) {
        self.say("effects", format!("{file} {generation}"), format!("effects {file}"));
    }
    /// The model shown in the stand cell, at `t` seconds into `clip`; none shows nothing.
    pub fn show_model(&mut self, name: Option<&str>, clip: Option<&str>, t: f32) {
        self.show_model_at(name, clip, t, 1.0);
    }
    /// `show_model` drawn at `scale` (a creature's row, ADR 0053).
    pub fn show_model_at(&mut self, name: Option<&str>, clip: Option<&str>, t: f32, scale: f32) {
        let line = match name {
            Some(n) => format!("model {n} {}{t:.4}{}", clip.map_or(String::new(), |c| format!("{c} ")), if scale == 1.0 { String::new() } else { format!(" x{scale}") }),
            None => "model -".into(),
        };
        self.say("model", line.clone(), line);
    }

    /// The shown model's state (ADR 0050: its clip, effects, sound and light); none plays the clip of `show_model`.
    pub fn show_state(&mut self, state: Option<&str>) {
        let line = format!("state {}", state.unwrap_or("-"));
        self.say("state", line.clone(), line);
    }

    /// Everything the page set, sent; frames asked for; the newest frame taken.
    fn pump(&mut self, ctx: &egui::Context, w: u32, h: u32) {
        if matches!(self.state, State::Off) {
            self.start(w, h);
        }
        let Some(p) = &mut self.proc else { return };
        if let Ok(Some(_)) = p.child.try_wait() {
            return self.fail();
        }
        let mut last = None;
        while let Ok(f) = p.rx.try_recv() {
            last = Some(f);
        }
        if let Some(f) = last {
            if let Some((id, at)) = self.pending {
                if f.id == id {
                    self.delays.push(at.elapsed().as_secs_f32() * 1000.0);
                    self.pending = None;
                }
            }
            if self.startup_s.is_none() {
                self.startup_s = self.started.map(|s| s.elapsed().as_secs_f32());
            }
            let img = egui::ColorImage::from_rgba_unmultiplied([f.w as usize, f.h as usize], &f.px);
            match &mut self.tex {
                Some(t) => t.set(img, egui::TextureOptions::LINEAR),
                None => self.tex = Some(ctx.load_texture("game view", img, egui::TextureOptions::LINEAR)),
            }
            (self.img, self.fov, self.state) = ([f.w, f.h], f.fov, State::Up);
        }
        let (e, t) = (self.orbit.eye(), self.orbit.target);
        let cam = format!("cam {} {} {} {} {} {}", e.x, e.y, e.z, t.x, t.y, t.z);
        self.say("cam", cam.clone(), cam);
        let s = format!("{:.2}", game_time(self.hour as f64, STAND));
        self.say("time", s.clone(), format!("time {s}"));
        let fx = self.shown_effect.as_ref().map_or("-".to_string(), |(n, k)| format!("{n} {k}"));
        let name = self.shown_effect.as_ref().map_or("-".to_string(), |(n, _)| n.clone());
        self.say("effect", fx, format!("effect {name}"));
        let blocks = [self.scene.blocks(), self.place.clone()].into_iter().filter(|b| !b.is_empty()).collect::<Vec<_>>().join(",");
        if self.told.get("place") != Some(&blocks) {
            self.write("reset");
            if !blocks.is_empty() {
                self.write(&format!("place {blocks}"));
            }
            self.told.insert("place", blocks);
            self.fresh = 0;
        }
        // the renderer's own size follows the view, once the pointer is let go
        if !ctx.input(|i| i.pointer.any_down()) {
            let s = format!("{w} {h}");
            if self.told.get("size").is_some_and(|o| o != &s) {
                self.write(&format!("size {s}"));
                self.told.insert("size", s);
            }
        }
        // one frame in flight; the light settles over about 40 frames after a change
        let lost = self.pending.is_some_and(|(_, at)| at.elapsed() > Duration::from_secs(3) && self.is_up());
        if (self.pending.is_none() || lost) && (self.fresh < 40 || self.live) {
            let id = self.next_id;
            self.next_id += 1;
            self.write(&format!("frame {id}"));
            self.pending = Some((id, Instant::now()));
            self.fresh += 1;
        }
        if self.pending.is_some() {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }

    /// The view: the game's frame filling `ui`'s room (`height` high), or `fallback` drawing until it comes.
    /// Orbit: the right drag (and the left, if `left_turns`) turns, the wheel zooms, the middle drag pans.
    pub fn ui(&mut self, ui: &mut Ui, height: f32, fallback: impl FnOnce(&egui::Painter, &Viewport, Rect)) -> egui::Response {
        self.shown_effect = self.effect.take();
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height.max(80.0)), Sense::click_and_drag());
        crate::drive::mark(ui.ctx(), "view", rect);
        let ppp = ui.ctx().pixels_per_point();
        let (w, h) = (((rect.width() * ppp) as u32).clamp(640, 1920), ((rect.height() * ppp) as u32).clamp(360, 1200));
        // orbit
        let d = resp.drag_delta();
        let turning = resp.dragged_by(egui::PointerButton::Secondary) || (self.left_turns && resp.dragged_by(egui::PointerButton::Primary));
        if turning {
            self.orbit.turn(d.x * 0.008, d.y * 0.008);
        }
        if resp.dragged_by(egui::PointerButton::Middle) {
            self.orbit.pan(d.x / rect.height(), d.y / rect.height());
        }
        if resp.hovered() {
            let s = ui.input(|i| i.smooth_scroll_delta.y);
            if s != 0.0 {
                self.orbit.zoom((-s * 0.002).exp());
            }
        }
        self.pump(ui.ctx(), w, h);
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 4.0, Color32::from_rgb(20, 21, 24));
        // the frame's part that fills the rectangle (cropped, never stretched)
        let uv = if self.img[0] > 0 {
            let (ia, ra) = (self.img[0] as f32 / self.img[1] as f32, rect.width() / rect.height());
            if ra > ia {
                let k = ia / ra;
                Rect::from_min_max(Pos2::new(0.0, (1.0 - k) / 2.0), Pos2::new(1.0, (1.0 + k) / 2.0))
            } else {
                let k = ra / ia;
                Rect::from_min_max(Pos2::new((1.0 - k) / 2.0, 0.0), Pos2::new((1.0 + k) / 2.0, 1.0))
            }
        } else {
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0))
        };
        self.shown = Some((rect, uv));
        match (&self.state, &self.tex) {
            (State::Up, Some(t)) => {
                p.image(t.id(), rect, uv, Color32::WHITE);
            }
            _ => {
                fallback(&p, self, rect);
                let why = match &self.state {
                    State::Failed(e) => format!("2D preview: {e}"),
                    _ => "2D preview: starting the game's renderer…".into(),
                };
                p.text(rect.left_bottom() + Vec2::new(GAP, -GAP), egui::Align2::LEFT_BOTTOM, why, egui::FontId::proportional(look::SMALL), Color32::from_gray(150));
            }
        }
        resp
    }

    /// The pointer's ray in the model's metres.
    pub fn ray(&self, pos: Pos2) -> Option<(Vec3, Vec3)> {
        let (rect, uv) = self.shown?;
        let rel = (pos - rect.min) / rect.size();
        let (u, v) = (uv.min.x + rel.x * uv.width(), uv.min.y + rel.y * uv.height());
        Some(self.orbit.ray([u * 2.0 - 1.0, 1.0 - v * 2.0], self.fov, self.aspect(rect)))
    }
    /// Where a point of the model's metres shows on screen.
    pub fn to_screen(&self, p: Vec3) -> Option<Pos2> {
        let (rect, uv) = self.shown?;
        let n = self.orbit.project(p, self.fov, self.aspect(rect))?;
        let (u, v) = ((n[0] + 1.0) / 2.0, (1.0 - n[1]) / 2.0);
        Some(rect.min + Vec2::new((u - uv.min.x) / uv.width() * rect.width(), (v - uv.min.y) / uv.height() * rect.height()))
    }
    fn aspect(&self, rect: Rect) -> f32 {
        if self.img[0] > 0 {
            self.img[0] as f32 / self.img[1] as f32
        } else {
            rect.width() / rect.height()
        }
    }

    /// The view's own controls: the scene, the time of day, the renderer's state.
    pub fn controls(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            for (s, name, help) in SCENES {
                if look::tip(look::chip(ui, self.scene == s, name), help, "").clicked() {
                    self.scene = s;
                }
            }
            ui.add_space(GAP);
            // the label, the slider and its value wrap as one, the slider narrower in a narrow column
            let slider = (ui.max_rect().width() - 110.0).clamp(60.0, 220.0);
            // (a wrapping row's available width is the whole row's: what is left is from the cursor on)
            if ui.max_rect().max.x - ui.cursor().min.x < slider + 100.0 {
                ui.end_row();
            }
            ui.horizontal(|ui| {
                ui.label(RichText::new("Time").small());
                ui.spacing_mut().slider_width = slider;
                let r = ui.add(egui::Slider::new(&mut self.hour, 0.0..=24.0).step_by(0.25).custom_formatter(|h, _| format!("{:02}:{:02}", h.floor() as u32 % 24, ((h.fract() * 60.0).round() as u32) % 60)));
                look::tip(r, "The local time of day where the scene stands: 12:00 the sun highest, 00:00 night", "");
            });
            let state = match &self.state {
                State::Up => format!("game renderer, {:.0} ms", self.delays.iter().rev().take(20).copied().fold(0.0, f32::max)),
                State::Failed(_) => "2D preview".into(),
                _ => "starting…".into(),
            };
            look::tip(look::weak(ui, state), "The game's own renderer draws this view; the time is the slowest of the last 20 frames from asking to showing", "");
        });
    }

    /// For `shot=`: waits (up to `max`) until the renderer is up and has drawn the scene as it is now, settled.
    pub fn settle(&mut self, ctx: &egui::Context, max: Duration) {
        let t0 = Instant::now();
        while t0.elapsed() < max {
            if matches!(self.state, State::Failed(_)) || (self.is_up() && self.fresh >= 40 && (self.pending.is_none() || self.live)) {
                return;
            }
            let (w, h) = self.told.get("size").and_then(|s| s.split_once(' ')).and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?))).unwrap_or((1280, 800));
            self.pump(ctx, w, h);
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Numbers for the shot's JSON line: how long the renderer took to start, and a frame from asking to having.
    pub fn report(&self) -> String {
        let mut d = self.delays.clone();
        d.sort_by(f32::total_cmp);
        let p = |q: f32| d.get(((d.len() as f32 - 1.0) * q).round() as usize).copied().unwrap_or(-1.0);
        let o = &self.orbit;
        format!(
            "{{\"up\":{},\"startup_s\":{:.2},\"frames\":{},\"delay_ms_p50\":{:.1},\"delay_ms_max\":{:.1},\"size\":[{},{}],\"orbit\":[{:.4},{:.4},{:.4}]}}",
            self.is_up(),
            self.startup_s.unwrap_or(-1.0),
            d.len(),
            p(0.5),
            p(1.0),
            self.img[0],
            self.img[1],
            o.yaw,
            o.pitch,
            o.dist
        )
    }
}

impl Drop for Viewport {
    fn drop(&mut self) {
        if let Some(mut p) = self.proc.take() {
            let _ = p.child.kill();
            let _ = p.child.wait();
        }
    }
}

/// Where `run=view` stands: its default `at`, the body's +y (`view.rs`), the spawn's longitude.
const STAND: glam::DVec3 = glam::DVec3::Y;

/// The game's clock (`time s`) at which a point on the planet (body frame direction `up`) has local solar time
/// `hour`: world time 0 is 9 am at the spawn, the body's +y (ADR 0022, 0041), and the planet spins about its +z.
fn game_time(hour: f64, up: glam::DVec3) -> f64 {
    // a point turned ahead of the spawn about +z sees the sun earlier: its clock runs ahead by the angle
    let ahead = (up.y.atan2(up.x) - std::f64::consts::FRAC_PI_2) / std::f64::consts::TAU * 24.0;
    (hour - ahead - 9.0).rem_euclid(24.0) / 24.0 * engine::sky::DAY
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::sky::Sky;
    use glam::DVec3;

    /// The sun's height in degrees over the point `up` at world time `t`, on the standard planet.
    fn elevation(t: f64, up: DVec3) -> f64 {
        let s = Sky::new(4244.0, 4244.0 / 3.7, 60.0, 23500.0);
        let p = s.planet(t);
        (p.rot * up.normalize()).dot((s.sun(t) - p.centre).normalize()).asin().to_degrees()
    }

    #[test]
    fn the_time_slider_is_local_solar_time_at_the_stand_point() {
        for up in [STAND, DVec3::X, DVec3::new(-1.0, -0.3, 0.2)] {
            assert!(elevation(game_time(12.0, up), up) > 60.0, "noon at {up}: {}", elevation(game_time(12.0, up), up));
            assert!(elevation(game_time(0.0, up), up) < -60.0, "midnight at {up}");
            assert!(elevation(game_time(21.0, up), up) < -30.0, "21:00 is night at {up}");
            let dusk = elevation(game_time(18.0, up), up);
            assert!(dusk.abs() < 5.0, "18:00 is sunset at {up}: {dusk}");
        }
    }
}
