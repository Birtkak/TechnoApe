//! The TechnoApe client: window, renderer and the benchmark runs (docs/plans/roadmap.md, "The shared benchmark").
mod app;
mod audio;
mod bench;
mod frame;
mod gi;
mod gpu;
mod hud;
mod menu;
mod play;
mod nav;
mod rt;
mod settings;
mod sim;
mod sys;
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

struct Args(HashMap<String, String>);
impl Args {
    fn get<T: std::str::FromStr>(&self, k: &str, d: T) -> T {
        self.0.get(k).and_then(|v| v.parse().ok()).unwrap_or(d)
    }
    fn s(&self, k: &str, d: &str) -> String {
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
            WindowEvent::Resized(size) if self.run == "play" && self.onscreen => self.resize(size.width, size.height),
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

/// mimalloc: the Windows heap is slow under many threads (speed-lever spike, ADR 0012).
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
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
    let link: Link<ToServer, ToClient> = match args.0.get("connect") {
        Some(addr) => {
            let addr = if addr.contains(':') { addr.clone() } else { format!("{addr}:{}", net::DEFAULT_PORT) };
            net::tcp(std::net::TcpStream::connect(&addr).expect("connect to the server")).expect("link")
        }
        None => {
            let (accept_tx, accept_rx) = std::sync::mpsc::channel();
            let (planet, moon, seed) = (args.get("planet", BODIES[0].n), args.get("moon", BODIES[1].n), args.get("seed", 1337));
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
                None => Server::new(planet, moon, seed, accept_rx),
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
    if run != "play" {
        link.send(ToServer::Creative(true)); // the edit benchmark places stone it never dug
    }
    let (_id, mut world, seed) = match link.rx.recv_timeout(Duration::from_secs(10)) {
        Ok(ToClient::Welcome { id, seed, planet_n, moon_n, blocks }) => {
            assert_eq!(blocks, engine::blocks::BLOCKS.hash, "the server's build has other blocks (data/blocks.toml)");
            (id, World::new(planet_n, moon_n, seed), seed)
        }
        other => panic!("no welcome from the server: {other:?}"),
    };
    // the waypoints live beside the save (shortcut: none without one, and none per remote server yet)
    let waypoint_file = save_path.as_ref().map(|s| format!("{s}.waypoints"));
    // the server spawns players on the +y pole; the client starts its prediction there too
    let spawn = world.bodies[0].spawn_point();
    assert!(world.bodies.len() <= MAX_BODIES);
    // run=sim stands at the spawn, among its players (sim.rs)
    if run == "sim" && !args.0.contains_key("at") {
        let s = world.bodies[0].spawn;
        args.0.insert("at".into(), format!("{},{},{}", s.x, s.y, s.z));
    }
    // place=di:dj:dl:block,... (a name, _ for a space) sets blocks around the stand point, the air block at its
    // feet the origin: lamps or metal in a shot without a session. Only the client's world has them.
    if run != "play" {
        let at: Vec<f64> = args.s("at", "0,1,0").split(',').filter_map(|v| v.parse().ok()).collect();
        let up = DVec3::new(at[0], at[1], at[2]).normalize();
        let feet = world.block_at(up * (world.bodies[0].top(up) + 0.1));
        for spec in args.s("place", "").split(',').filter(|s| !s.is_empty()) {
            let f: Vec<&str> = spec.split(':').collect();
            let d = IVec3::new(f[0].parse().unwrap(), f[1].parse().unwrap(), f[2].parse().unwrap());
            world.set_block(Block { pos: feet.pos + d, ..feet }, BLOCKS.id(&f[3].replace('_', " ")));
        }
    }
    // play's settings (ADR 0023); every other run keeps the defaults
    let settings_file = args.s("settings", "settings.txt");
    let settings = if run == "play" { Settings::parse(&std::fs::read_to_string(&settings_file).unwrap_or_default()) } else { Settings::default() };
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
    let (acts, creative, shot) = (parse_acts(&args.s("acts", "")), run != "play", args.0.get("shot").cloned());
    let time0 = args.get("time", if run == "orbit" { orbit_start(&world) } else { 0.0 });
    let server_at = args.0.get("connect").map_or("on this machine".to_string(), |a| a.clone());
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
        acted: (Instant::now() - Duration::from_secs(10), [0.0; 3], 0),
        world_notes: Vec::new(),
        creative,
        inv: Vec::new(),
        bucket_water: 0,
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
    };
    el.run_app(&mut app).unwrap();
    // the local server saves as it stops
    if let Some((stop, thread)) = local_server {
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = thread.join();
    }
    // a world picked in the menu, or back to the title: the game again, once the old save is written
    if let Some(next) = app.next.take() {
        drop(app);
        let exe = std::env::current_exe().expect("the game's own path");
        if let Err(e) = std::process::Command::new(exe).args(&next).spawn() {
            eprintln!("[technoape] could not start again with {next:?}: {e}");
        }
    }
}
