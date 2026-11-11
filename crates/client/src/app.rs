//! `App`, the state every part of the client touches, and the constants of play (ADR 0025 L: moved out of main.rs).
use super::*;

/// How far run=play aims a dig or place: half the server's reach, the distance a hand-held tool feels right at.
/// Seconds a correction takes to ease out (to 1/e).
pub(crate) const CORRECT_S: f64 = 0.1;
/// How much C held magnifies.
pub(crate) const ZOOM: f64 = 3.0;
pub(crate) const AIM_REACH: f64 = 8.0;
/// Metres between two footsteps: one every 0.42 s at a walk.
pub(crate) const STEP_M: f64 = 1.8;
/// Two presses of space this close (seconds) take off or land.

pub(crate) struct App {
    pub(crate) args: Args,
    pub(crate) gpu: Option<Gpu>,
    /// The traced terrain (ADR 0022), beside the GPU it lives on.
    pub(crate) rt: Option<rt::Rt>,
    pub(crate) slots: ChunkSlots,
    /// The bounce light (ADR 0022).
    pub(crate) gi: Option<gi::Gi>,
    pub(crate) world: World,
    pub(crate) link: Link<ToServer, ToClient>,
    /// run=play: the player this client predicts, the keys held, mouse motion since the last frame, the look
    pub(crate) me: phys::Player,
    /// Where `me` was one physics step ago, and the time not yet stepped: the camera is drawn between them.
    pub(crate) prev: DVec3,
    pub(crate) debt: f64,
    pub(crate) drift_max: f64,
    pub(crate) keys: std::collections::HashSet<Bind>,
    pub(crate) mouse: (f64, f64),
    pub(crate) pitch: f64,
    /// The mouse's turn (radians to the left) not stepped yet: the next step takes it, the camera shows it now.
    /// Out in space (`phys::free`) the mouse's pitch tilts the body and Q/E roll it, the same way (ADR 0024).
    pub(crate) turn: f64,
    pub(crate) tilt: f64,
    pub(crate) roll: f64,
    pub(crate) input: Input,
    /// Times the server's state, replayed to now, differed from the prediction by over a millimetre.
    pub(crate) snaps: u64,
    /// run=play (ADR 0019): the steps the server has not answered yet, by number, the next number, and the
    /// first one not sent; `None` until the server's first `You` says where this player is.
    pub(crate) steps: std::collections::VecDeque<(u32, Input)>,
    pub(crate) next_step: u32,
    pub(crate) unsent: u32,
    pub(crate) spawned: bool,
    /// What a correction moved the drawn position by; it eases out over CORRECT_S.
    pub(crate) smooth: DVec3,
    /// Seconds the last frame spent after present (stats, loading checks), and when it ended: both land in the
    /// next frame's time, outside its timed phases, so a spike reports them.
    pub(crate) tail_prev: f64,
    pub(crate) frame_end: Option<Instant>,
    /// When the last steps went to the server: by the wall clock, since the player moves (from its spawn) before
    /// the world has loaded, and a second's steps held back past the server's queue were lost (a correction).
    pub(crate) last_move: Instant,
    pub(crate) edits_applied: u64,
    pub(crate) players: usize,
    pub(crate) streamer: Streamer,
    pub(crate) run: String,
    /// The world time at the run's start: time= seconds, by default 0 (9 am at the spawn), for the orbit run
    /// `orbit_start` (ADR 0022).
    pub(crate) time0: f64,
    pub(crate) start: Instant,
    pub(crate) loaded_at: Option<Instant>,
    pub(crate) last_frame: Instant,
    pub(crate) ts_slot: usize,
    pub(crate) samples: Samples,
    pub(crate) rng: u64,
    pub(crate) edits_done: u64,
    /// This client's edits in flight (ADR 0015).
    pub(crate) pred: Predictor,
    /// run=play building (ADR 0017): the hammer's tool, the aimed block and the cell before it, the button
    /// held (dig or not) with where it was pressed and where the drag is now, the hotbar slot.
    pub(crate) tool: Tool,
    /// The build hammer is in hand. shortcut: a key takes it up until items exist; the server does not yet
    /// check that a shape comes from a hammer.
    pub(crate) hammer: bool,
    /// The bucket is in hand (ADR 0020): the dig button scoops, the place button pours, at the cell before
    /// the aimed block (the ray passes through water to what is under it).
    pub(crate) bucket: bool,
    pub(crate) aim: Option<(Block, Block)>,
    pub(crate) held: Option<(bool, Block, Block)>,
    pub(crate) slot: usize,
    /// The last dig or place: when, the colour of what was dug or placed, and a number that shapes its sparks.
    pub(crate) acted: (Instant, [f32; 3], u32),
    /// What each world of the Singleplayer list is (seed, size, last played), beside `worlds`.
    pub(crate) world_notes: Vec<String>,
    /// The slot whose block's name shows over the hotbar, and since when.
    pub(crate) held_name: (usize, Instant),
    pub(crate) creative: bool,
    pub(crate) inv: Vec<(u8, u32)>,
    /// Units of water in the bucket (ADR 0020).
    pub(crate) bucket_water: u32,
    /// The mesh of the tiles that hold stored water, and how many of its quads the GPU has.
    pub(crate) water: water::WaterMesh,
    pub(crate) wquad_n: u32,
    /// The lamps near the camera (`gather_lamps`), and the frame and edit count they were gathered at.
    pub(crate) lamps: Vec<(u8, DVec3, [f32; 3])>,
    pub(crate) lamps_at: (u64, u64),
    /// The local server's tick lengths in ms, for every run but play (ADR 0025, S).
    pub(crate) tick_log: Option<Arc<std::sync::Mutex<Vec<f32>>>>,
    /// What the overlay buffers hold: the ghost's shape and colour, its vertex count, the rectangle count.
    pub(crate) ghost_of: Option<(Shape, [u8; 4])>,
    pub(crate) ghost_n: u32,
    pub(crate) ui_n: u32,
    /// acts=: scripted input for checks without hands, as (seconds after load, action), soonest last.
    pub(crate) acts: Vec<(f64, String)>,
    pub(crate) title: String,
    /// shot=: where one settled frame of run=stand goes, and what it measured (ADR 0018).
    pub(crate) shot: Option<String>,
    pub(crate) shot_json: String,
    /// The last frame's camera (position, forward, up): the crack detector casts its rays from it.
    pub(crate) cam: (DVec3, DVec3, DVec3),
    /// The feel report (ADR 0018): edits not on screen yet (frame and time of the click, their cells), then
    /// each one's frames and milliseconds until it was; the last physics positions and the jerk between them.
    pub(crate) clicks: Vec<(u64, Instant, Vec<Block>)>,
    pub(crate) shown: Vec<(u64, f64)>,
    pub(crate) path: Vec<DVec3>,
    pub(crate) jerks: Vec<(f64, f64)>,
    /// When each correction came and how far it moved the player.
    pub(crate) snap_t: Vec<(f64, f64)>,
    pub(crate) last_pos: DVec3,
    pub(crate) sheet: Vec<image::RgbaImage>,
    pub(crate) next_cap: f64,
    pub(crate) done: bool,
    pub(crate) cap_prev: bool,
    /// The chat (ADR 0023): its lines with when they came, and the line being typed (T, or / for a command).
    pub(crate) chat: Vec<(Instant, String)>,
    pub(crate) typing: Option<String>,
    /// The lines this player sent, oldest first, and the one the up arrow shows now.
    pub(crate) sent: Vec<String>,
    pub(crate) recall: Option<usize>,
    /// The command line's completions (Tab, ADR 0023): the one picked, None while the list is hidden.
    pub(crate) pick: Option<usize>,
    /// C held zooms (3x); this eases towards it, 1 is none.
    pub(crate) zoom: f64,
    /// The server's clock (/timewarp): its time and warp at its last `You`, and when that came. The sky's
    /// clock runs on at the warp between them and eases onto it, so a fast warp turns the sky smoothly.
    pub(crate) clock: Option<(f64, f64, Instant)>,
    /// F3: the overlay of numbers; the last frames' times, the last GPU time, RAM and VRAM now and then, and
    /// what the last frame drew.
    pub(crate) f3: bool,
    pub(crate) recent: VecDeque<f32>,
    /// The places the player marked and the file they are kept in (beside the save; none without one).
    pub(crate) waypoints: Vec<nav::Waypoint>,
    pub(crate) waypoint_file: Option<String>,
    /// What the keys page's search holds (bugs/2026-11-11_13-13-42); a new page starts it empty.
    pub(crate) key_query: String,
    /// When the open menu page came up, and the row lit and since when: the menu's short motion.
    pub(crate) menu_since: Instant,
    pub(crate) lit: (Option<usize>, Instant),
    /// Play: the local server's last ticks, when each was seen and how long it took (ms), for F3's graph.
    pub(crate) ticks: VecDeque<(Instant, f32)>,
    pub(crate) gpu_last: f64,
    /// ADR 0025: the frame number each timestamp slot was written in, the GPU's end of the last frame read
    /// back (ticks), and the last frame's CPU stretches, which the next frame's interval is made of.
    pub(crate) ts_frames: [u64; TS_RING],
    pub(crate) gpu_end: Option<(u64, u64)>,
    pub(crate) seg_prev: [f64; CPU_STAGES.len()],
    /// The streamer's job time and count when loading finished: the jobs after it are remeshes and new chunks
    /// as the camera moves (ADR 0025, E).
    pub(crate) jobs_at_load: (f64, u64),
    pub(crate) mem: (f64, f64),
    pub(crate) drawn: Drawn,
    /// F9: the frame is grabbed first, then the form holds what is written, and Enter writes it all to bugs/
    /// on a thread, which says how that went.
    pub(crate) bug_grab: bool,
    pub(crate) bug: Option<String>,
    pub(crate) bug_shot: Option<image::RgbaImage>,
    pub(crate) bug_said: (std::sync::mpsc::Sender<String>, std::sync::mpsc::Receiver<String>),
    /// Play's settings (ADR 0023) and where they are kept; the menu (Esc) when open, the mouse pointer, and
    /// the menu's buttons as last drawn (x, y, width, height).
    pub(crate) settings: Settings,
    pub(crate) settings_file: String,
    pub(crate) menu: Option<Menu>,
    pub(crate) pointer: (f32, f32),
    pub(crate) menu_boxes: Vec<[f32; 4]>,
    /// The settings' slider being dragged, and the one whose number is being typed with the text so far.
    pub(crate) drag: Option<usize>,
    pub(crate) edit: Option<(usize, String)>,
    /// The first row the menu shows, the row the keyboard is on, the key row waiting for its key (and which
    /// of its two), and the display settings to go back to unless the change is kept by then.
    pub(crate) scroll: usize,
    pub(crate) sel: Option<usize>,
    pub(crate) rebind: Option<(Act, usize)>,
    pub(crate) revert: Option<(Settings, Instant)>,
    /// The zoom is on; the window has the keyboard; when the frame cap lets the next frame start.
    pub(crate) zoomed: bool,
    pub(crate) focused: bool,
    /// Sound (docs/design/sound.md), and the metres walked since the last footstep.
    pub(crate) audio: audio::Audio,
    pub(crate) walked: f64,
    pub(crate) next_frame: Instant,
    pub(crate) onscreen: bool,
    /// Creative flight (ADR 0023): on or not, its speed (the wheel), and when space was last pressed.
    pub(crate) flying: bool,
    pub(crate) fly_speed: f64,
    pub(crate) space_at: Option<Instant>,
    /// Where this world comes from, for F3 and bug reports: the seed, the server, the save and the recording.
    pub(crate) seed: i32,
    pub(crate) server_at: String,
    pub(crate) save_path: Option<String>,
    pub(crate) rec_path: Option<String>,
    /// The title menu's saves as last listed, and the arguments the game starts again with when it closes
    /// (a world picked, or back to the title).
    pub(crate) worlds: Vec<String>,
    pub(crate) next: Option<Vec<String>>,
}

/// What the last frame drew, for F3 and bug reports.
#[derive(Default)]
pub(crate) struct Drawn {
    pub(crate) chunks: usize,
    pub(crate) quads: u64,
    pub(crate) rt_pending: usize,
    pub(crate) sun_deg: f64,
    pub(crate) moon_lit: f64,
    /// The streamer this frame: chunks missing, holes, requests, jobs in flight.
    pub(crate) streaming: (u32, u32, u32, usize),
}
