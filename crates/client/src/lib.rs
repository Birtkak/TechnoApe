//! The TechnoApe client as a library: window, renderer and the benchmark runs (docs/plans/roadmap.md, "The shared
//! benchmark"). `technoape` (main.rs) runs it; the Studio draws with `gpu` and plays with `audio`.
mod app;
pub mod audio;
mod bench;
pub mod frame;
pub mod fx;
mod gi;
pub mod gpu;
mod hud;
/// The game's font as cells, for the Studio's interface (its `font.rs`).
pub use hud::font_cells;
mod menu;
pub mod models;
mod play;
mod nav;
mod rt;
mod settings;
mod sim;
mod sys;
mod view;
mod water;

use app::*;
use bench::*;
use gpu::*;
use menu::*;
use play::*;

use engine::bodies::BODIES;
use engine::blocks::{AIR, BLOCKS};
use engine::edit::{self, Kind, Predictor, Shape, Tool};
use engine::net::{self, Link, Recv, ToClient, ToServer, TICK_HZ};
use engine::phys::{self, Input};
use engine::stream::{DrawChunk, FrameStats, Opts, Streamer, View};
use engine::world::{Block, World};
use server::Server;
use settings::{Act, Bind, Row, Settings, S};
use glam::{DQuat, DVec3, IVec3, Mat4, Vec3, Vec4};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{CursorGrabMode, Window, WindowId};

pub struct Args(pub HashMap<String, String>);
impl Args {
    pub fn get<T: std::str::FromStr>(&self, k: &str, d: T) -> T {
        self.0.get(k).and_then(|v| v.parse().ok()).unwrap_or(d)
    }
    pub fn s(&self, k: &str, d: &str) -> String {
        self.0.get(k).cloned().unwrap_or(d.to_string())
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        let onscreen = self.args.get("onscreen", 0) == 1;
        self.onscreen = onscreen;
        // sound where someone listens: play on screen (sound=0 keeps it quiet, sound=1 opens the card anywhere)
        if self.args.get("sound", (onscreen && self.run == "play") as i32) == 1 {
            self.audio.open();
        }
        // play on screen as the settings say (ADR 0023): a window with a title bar, or fullscreen
        let play = onscreen && self.run == "play";
        let mon = self.settings.at(S::Monitor).checked_sub(1).and_then(|k| el.available_monitors().nth(k)).or_else(|| el.primary_monitor());
        let (fs, (fw, fh)) = if play { fullscreen(&self.settings, mon) } else { (None, (w(), h())) };
        SIZE.store((fw as u64) << 32 | fh as u64, Ordering::Relaxed);
        let attrs = Window::default_attributes()
            .with_fullscreen(fs.clone())
            .with_title("TechnoApe")
            .with_inner_size(winit::dpi::PhysicalSize::new(w(), h()))
            .with_resizable(false)
            .with_decorations(play && fs.is_none())
            .with_position({
                // off-screen runs keep the window far outside every monitor
                let at = if onscreen { 0 } else { -30000 };
                winit::dpi::PhysicalPosition::new(at, at)
            });
        let window = Arc::new(el.create_window(attrs).unwrap());
        if self.run == "play" && onscreen {
            let _ = window.set_cursor_grab(CursorGrabMode::Confined).or_else(|_| window.set_cursor_grab(CursorGrabMode::Locked));
            window.set_cursor_visible(false);
        }
        let cap: u32 = self.args.get("quad_cap_m", 16u32) * 1_000_000;
        let (gpu, rt, gi) = Gpu::new(window, &self.args, cap);
        self.gpu = Some(gpu);
        self.rt = Some(rt);
        self.gi = Some(gi);
        self.start = Instant::now();
        self.last_frame = Instant::now();
        hud::set_contrast(self.settings.on(S::Contrast));
        if play && self.settings.on(S::Vsync) {
            self.apply_present();
        }
        if self.home() == Menu::Title {
            self.set_menu(Some(Menu::Title));
        }
    }
    fn window_event(&mut self, el: &ActiveEventLoop, _: WindowId, ev: WindowEvent) {
        match ev {
            WindowEvent::CloseRequested => {
                self.finish();
                el.exit();
            }
            WindowEvent::KeyboardInput { event: KeyEvent { physical_key: PhysicalKey::Code(code), state, text, repeat, .. }, .. } => {
                if self.texting() {
                    // the chat's bar or the bug report's form takes the keys; Esc closes it
                    if state == ElementState::Pressed {
                        match code {
                            KeyCode::Escape => self.act("cancel"),
                            KeyCode::Enter | KeyCode::NumpadEnter => self.act("enter"),
                            KeyCode::Backspace => self.act("back"),
                            KeyCode::ArrowUp => self.act("earlier"),
                            KeyCode::ArrowDown => self.act("later"),
                            KeyCode::Tab => self.act("tab"),
                            _ => {
                                if let Some(t) = text.filter(|t| !t.chars().any(char::is_control)) {
                                    self.act(&format!("type:{t}"));
                                }
                            }
                        }
                    }
                    return;
                }
                if self.menu.is_some() {
                    if state == ElementState::Pressed && self.rebind.is_some() {
                        // a key row waits for its key: Esc leaves it as it was, Backspace clears it
                        // the Enter or arrow still held from choosing the row repeats; that is no answer
                        match code {
                            _ if repeat => {}
                            KeyCode::Escape => self.rebind = None,
                            KeyCode::Backspace => self.bind_to(None),
                            c if Bind::Key(c).name() != "?" => self.bind_to(Some(Bind::Key(c))),
                            _ => {}
                        }
                    } else if state == ElementState::Pressed {
                        // the menus take Esc, the arrows and Enter; F3 and F9 (a bug report) work in them too
                        match code {
                            KeyCode::Escape => self.act("esc"),
                            KeyCode::F3 => self.act("f3"),
                            KeyCode::F9 => self.act("f9"),
                            KeyCode::F10 => self.act("f10"),
                            KeyCode::ArrowDown => self.act("next"),
                            KeyCode::ArrowUp => self.act("prev"),
                            KeyCode::ArrowRight => self.act("nudge+"),
                            KeyCode::ArrowLeft => self.act("nudge-"),
                            KeyCode::Enter | KeyCode::NumpadEnter => self.act("go"),
                            KeyCode::Delete => self.act("remove"),
                            _ => {}
                        }
                    }
                    return;
                }
                if code == KeyCode::Escape && self.run != "play" {
                    self.finish();
                    el.exit();
                } else if code == KeyCode::Escape {
                    if state == ElementState::Pressed {
                        self.act("esc");
                    }
                } else {
                    // the settings' bindings say what a key does (docs/design/settings.md, Keys)
                    // a key that types "/" opens the chat on a command wherever the layout has it, and its own
                    // actions stay quiet for that press (ADR 0023; on a German board it is shift and 7)
                    let down = state == ElementState::Pressed;
                    let slash = down && !repeat && text.as_deref() == Some("/");
                    self.bind_event(Bind::Key(code), down, !slash);
                    if slash {
                        self.act("/");
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer = (position.x as f32 / self.ui(), position.y as f32 / self.ui());
                self.sel = None;
                self.slide();
            }
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. } if self.drag.is_some() => {
                self.drag = None;
                self.save_settings();
            }
            WindowEvent::Resized(size) if (self.run == "play" && self.onscreen) || self.run == "view" => self.resize(size.width, size.height),
            WindowEvent::Focused(f) => self.focused = f,
            WindowEvent::MouseInput { state: ElementState::Pressed, button, .. } if self.menu.is_some() && !self.texting() => {
                if self.rebind.is_some() {
                    if let Some(b) = mouse_bind(button) {
                        self.bind_to(Some(b));
                    }
                } else {
                    self.menu_click(button == MouseButton::Right);
                }
            }
            WindowEvent::MouseWheel { delta, .. } if self.menu.is_some() && !self.texting() => {
                let y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y as f64,
                    MouseScrollDelta::PixelDelta(p) => p.y / 40.0,
                };
                self.scroll_by(if y > 0.0 { -1 } else { 1 });
            }
            WindowEvent::MouseInput { .. } | WindowEvent::MouseWheel { .. } if self.texting() || self.menu.is_some() => {}
            WindowEvent::MouseInput { state, button, .. } => {
                if let Some(b) = mouse_bind(button) {
                    self.bind_event(b, state == ElementState::Pressed, true);
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let y = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y as f64,
                    MouseScrollDelta::PixelDelta(p) => p.y / 40.0,
                };
                if y != 0.0 {
                    self.act(if y > 0.0 { "up" } else { "down" });
                }
            }
            _ => {}
        }
    }
    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, ev: DeviceEvent) {
        if let (DeviceEvent::MouseMotion { delta }, false) = (ev, self.texting() || self.menu.is_some()) {
            self.mouse.0 += delta.0;
            self.mouse.1 += delta.1;
        }
    }
    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        if self.gpu.is_none() {
            return;
        }
        if self.done {
            el.exit();
            return;
        }
        if self.revert.as_ref().is_some_and(|r| Instant::now() >= r.1) {
            self.undo_display();
        }
        // the frame cap (docs/design/settings.md, Display): sleep up to the next frame's time
        if let Some(hz) = self.cap_hz() {
            let now = Instant::now();
            if self.next_frame > now {
                std::thread::sleep(self.next_frame - now);
            }
            self.next_frame = self.next_frame.max(now) + Duration::from_secs_f64(1.0 / hz);
        }
        self.frame();
    }
}

/// The game: reads the command line's key=value pairs and runs until the window closes.
pub fn main() {
    let mut args = Args(
        std::env::args()
            .skip(1)
            .filter_map(|a| a.split_once('=').map(|(k, v)| (k.to_string(), v.to_string())))
            .collect(),
    );
    if args.0.is_empty() {
        // started without arguments (a double click, playtest/): the title menu on screen, in the repository the
        // exe sits in, so saves/ and bugs/ are the usual ones
        args.0.extend(TITLE_ARGS.iter().filter_map(|a| a.split_once('=')).map(|(k, v)| (k.to_string(), v.to_string())));
        let exe = std::env::current_exe().unwrap_or_default();
        if let Some(r) = exe.ancestors().skip(1).find(|d| d.join("Cargo.toml").exists() && d.join("crates").is_dir()) {
            let _ = std::env::set_current_dir(r);
        }
    }
    let threads: usize = args.get("threads", std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8).saturating_sub(2).max(1));
    // workers below normal priority can never preempt the frame thread (speed-lever spike, ADR 0012)
    let low = args.get("low_workers", 1) == 1;
    LOW_WORKERS.store(low, std::sync::atomic::Ordering::Relaxed);
    rayon::ThreadPoolBuilder::new().num_threads(threads).start_handler(move |_| if low { sys::below_normal() }).build_global().unwrap();
    // join=host:port: the title screen downloading a server's map under its bar (a connect= run that lacked it
    // started this); the join starts first, and the title's own world is a small one with no save
    let loading = args.0.get("join").map(|addr| menu::Loading::join(addr.clone(), args.s("name", "player"), menu::joined_args(&args.0)));
    if loading.is_some() {
        for (k, v) in [("run", "play"), ("title", "1"), ("save", "0"), ("record", "0"), ("planet", "768")] {
            args.0.insert(k.into(), v.into());
        }
        args.0.remove("moon");
    }
    let run = args.s("run", "stand");
    if run == "meshbench" {
        sys::meshbench(&args.0);
        return;
    }
    // the world comes from the server: a remote one (connect=host[:port]) or a local one on its own thread
    // (planet= and moon= are columns per face edge, 1 m each, ADR 0033: 6144 is the 4 km planet, the standard;
    // 12288 the 8 km huge one; 768 the benchmark's 500 m class)
    let mut local_server = None;
    let mut tick_log = None;
    let (mut save_path, mut rec_path) = (None, None);
    let mut link: Link<ToServer, ToClient> = match args.0.get("connect") {
        Some(addr) => {
            let addr = menu::server_addr(addr);
            net::tcp(std::net::TcpStream::connect(&addr).expect("connect to the server")).expect("link")
        }
        None => {
            let (accept_tx, accept_rx) = std::sync::mpsc::channel();
            let planet = args.get("planet", BODIES[0].n);
            // Luna follows Terra's size (ADR 0041); moon= still overrides it, for tests
            let (moon, seed) = (args.get("moon", engine::bodies::columns(1, planet)), args.get("seed", 1337));
            // the flight recorder (ADR 0011): on for play on screen, or wherever record= says
            let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
            let rec = args.0.get("record").cloned().or((args.get("onscreen", 0) == 1).then(|| format!("out/rec/play-{unix}.rec")));
            // saves (ADR 0016): play keeps one per seed and planet under saves/, save= names another, save=0 none
            let save = args.0.get("save").cloned().or((run == "play").then(|| format!("saves/seed-{seed}-planet-{planet}.sav"))).filter(|s| s != "0");
            let mut server = match save.as_deref().map(std::path::Path::new).filter(|p| p.exists()) {
                Some(p) => {
                    // a save this build cannot read stops the game rather than being overwritten
                    let s = server::save::Save::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
                    eprintln!("[technoape] carrying on from {} ({} edited cells)", p.display(), s.edits.len());
                    Server::from_save(s, accept_rx)
                }
                None => Server::baked(planet, moon, seed, accept_rx),
            };
            server.autosave = save.clone().map(|p| (p.into(), 30 * TICK_HZ as u64));
            save_path = save;
            if let Some(r) = rec.filter(|r| r != "0") {
                rec_path = Some(r.clone());
                match server.record(std::path::Path::new(&r)) {
                    Ok(()) => eprintln!("[technoape] recording to {r}"),
                    Err(e) => eprintln!("[technoape] not recording: {e}"),
                }
            }
            let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let st = stop.clone();
            // every tick's time: the benchmark's numbers (ADR 0025, S) and F3's graph in play
            let v = Arc::new(std::sync::Mutex::new(Vec::new()));
            server.tick_ms = Some(v.clone());
            tick_log = Some(v);
            local_server = Some((stop, std::thread::spawn(move || server.run(TICK_HZ, &st))));
            let (c, s) = net::local();
            accept_tx.send(s).unwrap();
            if run == "sim" {
                sim::start(accept_tx.clone(), args.get("bots", 50), seed as u64);
            }
            c
        }
    };
    link.send(ToServer::Hello { name: args.s("name", "player") });
    // (mode=creative: a new world made creative on its page; a world carried on keeps each player's own mode)
    if run != "play" || args.s("mode", "") == "creative" {
        link.send(ToServer::Creative(true)); // the edit benchmark places stone it never dug
    }
    let (_id, mut world, seed) = match link.rx.recv_timeout(Duration::from_secs(10)) {
        Ok(ToClient::Welcome { id, seed, planet_n, moon_n, blocks, map }) => {
            assert_eq!(blocks, engine::blocks::BLOCKS.hash, "the server's build has other blocks (data/blocks.toml)");
            // the baked map (ADR 0043): the local server's in memory, else from the disk or the server
            let kept = map.and_then(|h| engine::bake::find(seed, planet_n, Some(h)));
            if args.0.contains_key("connect") && kept.is_none() && map.is_some() && run == "play" && args.get("joined", 0) != 1 {
                // in play, a map to download comes in on the title screen, under its bar, which joins after it
                // (once: the run it starts has joined=1 and fetches here if the map is still not kept)
                drop(link);
                let exe = std::env::current_exe().expect("the game's own path");
                if let Err(e) = std::process::Command::new(exe).args(menu::join_args(&args.0)).spawn() {
                    eprintln!("[technoape] could not start the title screen: {e}");
                }
                return;
            }
            let map = map.map(|h| kept.map_or_else(|| engine::net::fetch_map(&mut link, seed, planet_n, h), Ok).unwrap_or_else(|e| panic!("{e}")));
            (id, World::with_map(planet_n, moon_n, seed, map), seed)
        }
        other => panic!("no welcome from the server: {other:?}"),
    };
    // the waypoints live beside the save (shortcut: none without one, and none per remote server yet)
    let waypoint_file = save_path.as_ref().map(|s| format!("{s}.waypoints"));
    // the server spawns players on the +y pole; the client starts its prediction there too
    let spawn = world.bodies[0].spawn_point();
    assert!(world.bodies.len() <= MAX_BODIES);
    // run=sim stands at the spawn, among its players (sim.rs)
    if (run == "sim" || run == "view") && !args.0.contains_key("at") {
        let s = world.bodies[0].spawn;
        args.0.insert("at".into(), format!("{},{},{}", s.x, s.y, s.z));
    }
    // creature= (ADR 0053), model=, fxplay= and structure= stand at the spawn too, on land as the Studio's view
    // does (the +y pole of the baked planet can be open sea)
    if ["creature", "model", "fxplay", "structure"].iter().any(|k| args.0.contains_key(*k)) && !args.0.contains_key("at") {
        let s = world.bodies[0].spawn;
        args.0.insert("at".into(), format!("{},{},{}", s.x, s.y, s.z));
    }
    // place=di:dj:dl:block,... (a name, _ for a space) sets blocks around the stand point, the air block at its
    // feet the origin: lamps or metal in a shot without a session. Only the client's world has them.
    // fxedit=t:di:dj:dl:block,... does the same t seconds after loading, as a click would, with its effects
    // (ADR 0031; with fx=1): filmstrips of a break and a place
    let mut fx_edits = Vec::new();
    let up = stand_at(&args).unwrap_or_else(|e| {
        eprintln!("[technoape] {e}");
        std::process::exit(1)
    });
    if run != "play" {
        let (place, fxedit, structure) = stand_args(&args, "data").unwrap_or_else(|e| {
            eprintln!("[technoape] {e}");
            std::process::exit(1)
        });
        let feet = world.block_at(up * (world.bodies[0].top(up) + 0.1));
        for (d, id) in place {
            world.set_block(Block { pos: feet.pos + d, ..feet }, id);
        }
        // structure=name[:seed] stands data/structures/<name>.toml (a grown one from that seed) with its anchor in
        // the first air over the stand point (ADR 0052): x along j, z along i, y up the layers (the camera frame of
        // `models::cell_axes`, so the Studio's view and this agree)
        if let Some(s) = structure {
            let foot = world.air_over_ground(0, up);
            for (d, id) in s.blocks(&BLOCKS) {
                world.set_block(Block { pos: foot.pos + IVec3::new(d[2], d[0], d[1]), ..foot }, id);
            }
        }
        for (t, d, id) in fxedit {
            fx_edits.push((t, Block { pos: feet.pos + d, ..feet }, id));
        }
        // the soonest last, whatever order they came in
        fx_edits.sort_by(|a: &(f64, Block, u8), b| b.0.total_cmp(&a.0));
    }
    // play's settings (ADR 0023); every other run keeps the defaults unless settings= names a file (a shot at
    // another screen size)
    let settings_file = args.s("settings", "settings.txt");
    let settings = if run == "play" || args.0.contains_key("settings") { Settings::parse(&std::fs::read_to_string(&settings_file).unwrap_or_default()) } else { Settings::default() };
    if let Some(d) = args.0.get("sound_dir") {
        let _ = audio::DIR.set(d.into());
    }
    // sounds=1: every sound in numbers, as one JSON line, and nothing else
    if args.get("sounds", 0) == 1 {
        println!("{}", audio::report());
        return;
    }
    // dump=1: what the file means, every row and key as one JSON line, and nothing else
    if args.get("dump", 0) == 1 {
        println!("{}", settings.json());
        return;
    }
    SIZE.store((settings.get(S::Width) as u64) << 32 | settings.get(S::Height) as u64, Ordering::Relaxed);
    let opts = Opts {
        k: args.get("k", settings.get(S::Detail)),
        // merging inside a lattice with interpolated corners (ADR 0025 I2); greedy=0 draws every face
        greedy: args.get("greedy", 1) == 1,
        horizon: args.get("horizon", 1) == 1,
        prefetch_s: args.get("prefetch", 0.5),
        max_inflight: args.get("inflight", threads * 4),
    };
    let cap: u32 = args.get("quad_cap_m", 16u32) * 1_000_000;
    let streamer = Streamer::new(&world, cap, opts);
    let el = EventLoop::new().unwrap();
    el.set_control_flow(ControlFlow::Poll);
    let lines_on = run == "play" && args.s("lines", "") == "stdin";
    let (acts, creative, shot) = (parse_acts(&args.s("acts", "")), run != "play", args.0.get("shot").cloned());
    let time0 = args.get("time", if run == "orbit" { orbit_start(&world) } else { 0.0 });
    let server_at = args.0.get("connect").map_or("on this machine".to_string(), |a| a.clone());
    // effects play in play and their own benchmark; elsewhere only with fx=1, so pictures repeat (ADR 0031)
    let fx = {
        let on = run == "play" || run == "fx" || args.get("fx", 0) == 1;
        let k = settings.at(S::Particles);
        let mut fx = fx::Fx::new(on, args.get("fx_cap", if run == "fx" { fx::CAP[2] } else { fx::CAP[k] }).min(MAX_FX), fx::SCALE[k], seed as u32);
        // play follows the Particles setting as the menu changes it
        if run == "play" && args.get("fx_cap", 0usize) == 0 {
            fx.setting = Some(k);
        }
        // data/effects.toml (effects=file another) read again when it changes (ADR 0050)
        if on {
            fx.watch(std::path::PathBuf::from(args.s("effects", "data/effects.toml")));
        }
        fx
    };
    let models = models::Models::new(&args);
    let mut app = App {
        time0,
        args,
        gpu: None,
        rt: None,
        slots: ChunkSlots::default(),
        gi: None,
        lamps: Vec::new(),
        lamps_at: (0, u64::MAX),
        tick_log,
        world,
        link,
        me: phys::Player::at(spawn, DVec3::X),
        prev: spawn,
        steps: Default::default(),
        next_step: 1,
        unsent: 1,
        spawned: false,
        server_me: None,
        smooth: DVec3::ZERO,
        debt: 0.0,
        drift_max: 0.0,
        keys: Default::default(),
        mouse: (0.0, 0.0),
        pitch: 0.0,
        turn: 0.0,
        tilt: 0.0,
        roll: 0.0,
        input: Input::default(),
        snaps: 0,
        tail_prev: 0.0,
        frame_end: None,
        last_move: Instant::now(),
        edits_applied: 0,
        players: 0,
        streamer,
        run,
        start: Instant::now(),
        loaded_at: None,
        last_frame: Instant::now(),
        ts_slot: 0,
        samples: Samples::default(),
        rng: 0x9E3779B97F4A7C15,
        edits_done: 0,
        pred: Predictor::default(),
        tool: Tool { kind: Kind::Box, single: true, height: 4 },
        hammer: false,
        bucket: false,
        aim: None,
        held: None,
        slot: 0,
        held_name: (usize::MAX, Instant::now()),
        fx,
        models,
        fx_n: 0,
        dip: fx::Track { tween: fx::Tween::new(-10.0, 0.18, fx::Curve::Pulse), from: [0.0; 2], to: [0.0; 2] },
        growing: Vec::new(),
        hold: edit::Hold::default(),
        dig_hit: 0.0,
        dig_again: false,
        digs: Vec::new(),
        fx_edits,
        world_notes: Vec::new(),
        new_world: Default::default(),
        creative,
        inv: Vec::new(),
        bucket_water: 0,
        vitals: (engine::vitals::MAX, engine::vitals::MAX),
        health_min: engine::vitals::MAX,
        flash: [(Instant::now() - Duration::from_secs(10), 0); 2],
        dead: None,
        water: water::WaterMesh::default(),
        wquad_n: 0,
        ghost_of: None,
        ghost_n: 0,
        ui_n: 0,
        acts,
        title: String::new(),
        shot,
        shot_json: String::new(),
        cam: (DVec3::ZERO, DVec3::X, DVec3::Y),
        clicks: Vec::new(),
        shown: Vec::new(),
        path: Vec::new(),
        jerks: Vec::new(),
        snap_t: Vec::new(),
        last_pos: DVec3::ZERO,
        sheet: Vec::new(),
        next_cap: 0.0,
        done: false,
        cap_prev: false,
        chat: Vec::new(),
        typing: None,
        sent: Vec::new(),
        recall: None,
        pick: None,
        zoom: 1.0,
        clock: None,
        f3: false,
        recent: VecDeque::new(),
        waypoints: waypoint_file.as_deref().and_then(|f| std::fs::read_to_string(f).ok()).map_or(Vec::new(), |t| nav::parse(&t)),
        waypoint_file,
        key_query: String::new(),
        menu_since: Instant::now(),
        lit: (None, Instant::now()),
        ticks: VecDeque::new(),
        gpu_last: 0.0,
        ts_frames: [0; TS_RING],
        gpu_end: None,
        seg_prev: [0.0; CPU_STAGES.len()],
        jobs_at_load: (0.0, 0),
        mem: (sys::ram_mb(), sys::vram_mb()),
        drawn: Drawn::default(),
        bug_grab: false,
        bug: None,
        bug_shot: None,
        bug_said: std::sync::mpsc::channel(),
        lines: lines_on.then(|| {
            let (tx, rx) = std::sync::mpsc::channel();
            // (ends with the pipe, or with the game: nothing waits for it)
            let _ = std::thread::Builder::new().name("lines stdin".into()).spawn(move || {
                use std::io::{BufRead, Read};
                // (a line is taken in pieces of 4096 bytes at the most, and what is no text is read past:
                // neither a line without an end nor a wrong byte stops the lines after it)
                let mut stdin = std::io::stdin().lock();
                loop {
                    let mut buf = Vec::new();
                    if !matches!((&mut stdin).take(4096).read_until(b'\n', &mut buf), Ok(n) if n > 0) || tx.send(String::from_utf8_lossy(&buf).into_owned()).is_err() {
                        return;
                    }
                }
            });
            rx
        }),
        settings,
        settings_file,
        menu: None,
        pointer: (0.0, 0.0),
        menu_boxes: Vec::new(),
        drag: None,
        edit: None,
        scroll: 0,
        sel: None,
        rebind: None,
        revert: None,
        zoomed: false,
        focused: true,
        audio: audio::Audio::new(),
        walked: 0.0,
        fell: 0.0,
        next_frame: Instant::now(),
        onscreen: false,
        flying: false,
        fly_speed: 11.0,
        space_at: None,
        seed,
        server_at,
        save_path,
        rec_path,
        worlds: Vec::new(),
        next: None,
        loading,
        view: None,
    };
    el.run_app(&mut app).unwrap();
    // the local server saves as it stops
    if let Some((stop, thread)) = local_server {
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = thread.join();
    }
    // a world picked in the menu, or back to the title: the game again, once the old save is written
    if let Some(next) = app.next.take() {
        // this window stays up until the next one has drawn, so the desktop never shows between them (ADR 0038)
        let shown = std::env::temp_dir().join(format!("technoape-shown-{}", std::process::id()));
        let exe = std::env::current_exe().expect("the game's own path");
        // (one left by an earlier process of this number would end the wait before the window is there)
        let _ = std::fs::remove_file(&shown);
        match std::process::Command::new(exe).args(&next).arg(format!("shown={}", shown.display())).spawn() {
            Ok(mut next) => {
                // (a world may take long to load: the wait ends when it shows, or at once when it has ended
                // without showing, so a world that fails to load never leaves this window frozen)
                sys::wait_shown(&shown, Duration::from_secs(30), || !matches!(next.try_wait(), Ok(None)));
            }
            Err(e) => eprintln!("[technoape] could not start again with {next:?}: {e}"),
        }
        drop(app);
    }
}

/// `at=x,y,z`: the stand point's direction from the planet's centre (`0,1,0` when unset); three finite numbers,
/// not all 0.
pub fn stand_at(args: &Args) -> Result<DVec3, String> {
    let spec = args.s("at", "0,1,0");
    let v: Vec<f64> = spec.split(',').map(|v| v.trim().parse::<f64>()).collect::<Result<_, _>>().map_err(|_| format!("at={spec}: three numbers, x,y,z"))?;
    let d = (v.len() == 3).then(|| DVec3::new(v[0], v[1], v[2])).filter(|d| d.is_finite() && d.length() > 0.0).ok_or(format!("at={spec}: three numbers, x,y,z, not all 0"))?;
    Ok(d.normalize())
}

/// How far a `place=` or `fxedit=` cell may be from the stand cell, cells on each axis (a cell far off can lie
/// off the planet's grid, where the bake's lookups panic).
pub(crate) const PLACE_REACH: i32 = 64;

/// A stand's `at=`, `place=`, `fxedit=` and `structure=` read, and `creature=`, `model=` and `fxplay=` checked:
/// what is wrong names its argument, for one line on stderr (no panic).
#[allow(clippy::type_complexity)]
pub fn stand_args(args: &Args, data: &str) -> Result<(Vec<(IVec3, u8)>, Vec<(f64, IVec3, u8)>, Option<engine::structures::Structure>), String> {
    let block = |n: &str| (0..=255u8).find(|i| BLOCKS.get(*i).is_some_and(|d| d.name == n.replace('_', " "))).ok_or(format!("no block '{n}'"));
    stand_at(args)?;
    let int = |v: &str| v.parse::<i32>().ok().filter(|v| (-PLACE_REACH..=PLACE_REACH).contains(v)).ok_or(format!("'{v}' is not a whole number within {PLACE_REACH} cells"));
    let each = |key: &str, shape: &str, f: &dyn Fn(&[&str]) -> Result<(f64, IVec3, u8), String>| -> Result<Vec<(f64, IVec3, u8)>, String> {
        let spec = args.s(key, "");
        let named = |e: String| format!("{key}={spec}: {e}");
        spec.split(',').filter(|s| !s.is_empty()).map(|one| {
            let parts: Vec<&str> = one.split(':').collect();
            if parts.len() != shape.split(':').count() {
                return Err(named(format!("'{one}' is not {shape}")));
            }
            f(&parts).map_err(named)
        }).collect()
    };
    let place = each("place", "di:dj:dl:block", &|f| Ok((0.0, IVec3::new(int(f[0])?, int(f[1])?, int(f[2])?), block(f[3])?)))?.into_iter().map(|(_, d, id)| (d, id)).collect();
    let fxedit = each("fxedit", "t:di:dj:dl:block", &|f| Ok((f[0].parse::<f64>().ok().filter(|t| t.is_finite()).ok_or(format!("'{}' is not seconds", f[0]))?, IVec3::new(int(f[1])?, int(f[2])?, int(f[3])?), block(f[4])?)))?;
    let structure = match args.0.get("structure") {
        None => None,
        Some(spec) => Some((|| {
            let (name, seed) = spec.split_once(':').map_or(Ok((spec.as_str(), None)), |(n, s)| s.parse::<u64>().map(|s| (n, Some(s))).map_err(|_| format!("'{s}' is not a seed")))?;
            let file = format!("{data}/structures/{name}.toml");
            let mut s = std::fs::read_to_string(&file).map_err(|e| format!("{file}: {e}")).and_then(|t| engine::structures::Structure::parse(&file, &t, &BLOCKS))?;
            if let (Some(seed), Some(r)) = (seed, s.grow.clone()) {
                s = engine::structures::grow(&engine::structures::Recipe { seed, ..r });
            }
            Ok(s)
        })().map_err(|e: String| format!("structure={spec}: {e}"))?),
    };
    if let Some(spec) = args.0.get("creature") {
        let (name, speed) = spec.split_once(':').map_or((spec.as_str(), None), |(n, s)| (n, Some(s)));
        let file = format!("{data}/creatures.toml");
        let t: toml::Table = std::fs::read_to_string(&file).map_err(|e| e.to_string()).and_then(|t| toml::from_str(&t).map_err(|e| e.to_string())).map_err(|e| format!("creature={spec}: {file}: {e}"))?;
        if !t.get("creature").and_then(|v| v.as_array()).into_iter().flatten().any(|c| c.get("name").and_then(|n| n.as_str()) == Some(name)) {
            return Err(format!("creature={spec}: no creature '{name}' in {file}"));
        }
        if speed.is_some_and(|s| s.parse::<f32>().is_err()) {
            return Err(format!("creature={spec}: the speed after ':' is metres a second"));
        }
    }
    if let Some(spec) = args.0.get("model") {
        let name = spec.split(['@', ':']).next().unwrap_or("");
        let file = format!("{}/{name}.toml", args.s("models", "data/models"));
        if !std::path::Path::new(&file).is_file() {
            return Err(format!("model={spec}: no model file {file}"));
        }
    }
    if let Some((_, n)) = args.0.get("fxplay").and_then(|s| s.split_once(':')) {
        if n.parse::<usize>().is_err() {
            return Err(format!("fxplay={}: the count after ':' is a whole number", args.s("fxplay", "")));
        }
    }
    Ok((place, fxedit, structure))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data");

    fn args(kv: &[(&str, &str)]) -> Args {
        Args(kv.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect())
    }

    #[test]
    fn bad_stand_arguments_are_named_not_panicked() {
        let ok = stand_args(&args(&[("models", &format!("{DATA}/models")), ("structure", "oak:3"), ("creature", "boar:2"), ("model", "boar:walk"), ("fxplay", "fire:2")]), DATA).unwrap();
        assert!(ok.2.is_some());
        let ok = stand_args(&args(&[("models", &format!("{DATA}/models")), ("place", "0:1:2:stone,1:0:0:blue_lamp"), ("fxedit", "0.5:0:0:1:air")]), DATA).unwrap();
        assert_eq!(ok.0, vec![(IVec3::new(0, 1, 2), BLOCKS.id("stone")), (IVec3::X, BLOCKS.id("blue lamp"))]);
        assert_eq!(ok.1, vec![(0.5, IVec3::Z, AIR)]);
        for (k, v) in [
            ("place", "0:1:stone"),
            ("place", "0:x:1:stone"),
            ("place", "0:0:1:no such block"),
            ("fxedit", "0:0:1:air"),
            ("fxedit", "t:0:0:1:air"),
            ("structure", "no such structure"),
            ("structure", "../Cargo"),
            ("creature", "no such creature"),
            ("creature", "boar:fast"),
            ("model", "no such model"),
            ("fxplay", "fire:x"),
            ("at", "1,2"),
            ("at", "1,x,2"),
            ("at", "0,0,0"),
            ("place", "2000000000:0:0:stone"),
            ("fxedit", "0:0:0:-2147483648:air"),
        ] {
            let e = std::panic::catch_unwind(|| stand_args(&args(&[("models", &format!("{DATA}/models")), (k, v)]), DATA)).unwrap_or_else(|_| panic!("{k}={v} panicked")).err().unwrap_or_else(|| panic!("{k}={v} passed"));
            assert!(e.starts_with(&format!("{k}={v}: ")), "{e}");
        }
    }
}
