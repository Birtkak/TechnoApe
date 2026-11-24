//! run=play: building, digging and the bucket, `acts=` scripted input, chat and commands, F3 and F9, reconciling with the server (ADR 0025 L: moved out of main.rs).
use super::*;

/// The playtest menu's rows (F10, bugs/2026-11-11_13-12-42): a name, its steps and what to look for. A step
/// is a command as typed in the chat, or `eclipse:sun`, `eclipse:luna` (the clock and the player go to the
/// next one), `fly:<m/s>` (flying, at that speed). A new thing worth a look
/// gets a row here.
pub(crate) const PLAYTEST: &[(&str, &[&str], &str)] = &[
    ("Eclipse of the sun: Luna in front of Sol", &["eclipse:sun"], ""),
    ("Eclipse of Luna: in Terra's shadow", &["eclipse:luna"], ""),
    ("On Luna: 3.7 times less gravity, Terra overhead", &["/playmode creative", "/goto luna"], "A jump is about 4.4 m here. Terra's night side should fade in, with no hard line."),
    ("Fast flight: 1000 m/s over Terra", &["/playmode creative", "/goto terra", "fly:1000"], "Fly low and turn hard: the ground should never vanish."),
    ("Midnight: the moon grey, not red", &["/goto terra", "/time set midnight"], "Find Luna: grey-white, unless an eclipse is on."),
    ("Sunset: the light fading out", &["/goto terra", "/time set sunset"], "The sunlight on the ground should dim smoothly as Sol sets."),
    ("Lamps at night: a kit for a lit room", &["/goto terra", "/time set night", "/give lamp 32", "/give blue lamp 32"], "Dig a room and set lamps into its floor and walls: even light, no hard cones."),
    ("Every block: creative's inventory", &["/playmode creative"], "E opens every block and item in tabs: a click takes a stack, the middle button picks a block in the world."),
    ("Health and hunger: the bars at 13 and 6", &["/playmode survival", "/heal", "/hurt 7", "/feed 6"], "Two bars over the hotbar: a green cross, an orange stomach. /heal and /feed fill them."),
    ("Thirst: the third bar at 6", &["/playmode survival", "/heal", "/feed 20", "/thirst 6"], "A blue drop over the hunger bar. The place button held on fresh water with nothing in hand fills it in two seconds; the sea is salt."),
    ("Low health and an empty stomach", &["/playmode survival", "/heal", "/hurt 17", "/feed 0"], "Health pulses with red corners; hunger is outlined in red and health sinks to 1."),
    ("The death screen", &["/playmode survival", "/heal", "/hurt 20"], "You died: Respawn puts you at the spawn with full bars."),
    ("Impact: fly up in creative, then fall in survival", &["/playmode creative", "/goto terra", "fly:20"], "Fly up 10 m or more, type /playmode survival, and fall: the faster you hit, the more it costs. 21 m/s kills."),
    ("Home at noon, in survival", &["/goto terra", "/time set noon", "/playmode survival"], "Back at the spawn, at noon."),
];

/// acts="2:C,2.5:c,3:R,3.5:r": (seconds after load, action), sorted so the soonest is last.
/// What the look-at panel shows (draft 2026-11-24, "Felt temperature"): its target, the name and the note under it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Looked {
    pub(crate) what: edit::Look,
    pub(crate) name: String,
    pub(crate) note: String,
}

/// Seconds between two workings-out of the felt temperature and the look-at panel's lines.
pub(crate) const SENSE_S: f64 = 0.25;

/// The look-at panel's lines for target `look`, `live` the weather's air at the eye: a block's name and its
/// temperature, hardness, the tool it wants and what else it is; a fluid's name (sea water by the sea's salt), its
/// temperature and depth.
pub(crate) fn looked(w: &World, look: edit::Look, live: Option<f64>) -> Looked {
    match look {
        edit::Look::Block(b) => {
            let m = w.block(b);
            let Some(d) = BLOCKS.get(m) else { return Looked { what: look, name: "?".into(), note: String::new() } };
            let mut note = format!("{:.1} C, hardness {}", w.block_temp(b, live), d.hardness);
            if let Some(t) = &d.tool {
                note += &format!(", {t}");
            }
            if d.light.iter().any(|c| *c > 0.0) {
                note += ", gives light";
            }
            if d.metal > 0.0 {
                note += ", metal";
            }
            Looked { what: look, name: d.name.clone(), note }
        }
        edit::Look::Fluid(b, k) => {
            let row = engine::fluids::FLUIDS.now().get(k).clone();
            let name = if k == engine::fluids::WATER && w.salt(b) { "Sea water".to_string() } else { capital(&row.name) };
            Looked { what: look, name, note: format!("{:.1} C, {:.1} m deep", w.fluid_temp(b, k, live), w.fluid_depth(b, k)) }
        }
    }
}

/// `s` with its first letter a capital.
fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or(String::new(), |f| f.to_uppercase().chain(c).collect())
}

impl App {
    /// The felt temperature at the eye and the look-at target's lines, again when `SENSE_S` has passed: the HUD and
    /// F3 read them, the run's JSON prints them.
    pub(crate) fn sense(&mut self, p: &Pose) {
        if self.sensed.as_ref().is_some_and(|s| s.0.elapsed().as_secs_f64() < SENSE_S) || self.world.bodies.is_empty() {
            return;
        }
        // the live air is the home's, where the player stands on it
        let live = self.air.filter(|_| self.world.frame_at(p.pos) == 0).map(|a| a.temp as f64);
        let felt = self.world.felt(p.pos, live);
        let look = edit::look(&self.world, p.pos, p.fwd, AIM_REACH).map(|l| looked(&self.world, l, live));
        self.sensed = Some((Instant::now(), felt, look));
    }
}

pub(crate) fn parse_acts(s: &str) -> Vec<(f64, String)> {
    let mut v: Vec<(f64, String)> =
        s.split(',').filter_map(|e| e.trim().split_once(':')).filter_map(|(t, a)| Some((t.parse().ok()?, a.to_string()))).collect();
    v.sort_by(|a, b| b.0.total_cmp(&a.0));
    v
}

/// One predicted step, as the server takes it (ADR 0118): the sprint refused at the floor by the `hunger`
/// carried, the step, then that step's drain (`drains`: survival and alive). True when it sprinted.
pub(crate) fn predict(world: &mut World, me: &mut phys::Player, hunger: &mut f64, creative: bool, drains: bool, inp: &Input) -> bool {
    let g = engine::vitals::gate(*inp, *hunger, creative);
    let sprinted = phys::step_at(world, me, &g, phys::DT);
    if drains {
        *hunger = engine::vitals::drained(*hunger, engine::vitals::rate(&g, sprinted));
    }
    sprinted
}

/// Where the steps the server has not answered take a player from the server's last word on it (ADR 0019),
/// and the hunger they leave (from the server's, `(hunger, creative, drains)` as `predict` takes them).
pub(crate) fn replay(world: &mut World, server: phys::Player, (mut hunger, creative, drains): (f64, bool, bool), steps: &VecDeque<(u32, Input)>) -> (phys::Player, f64) {
    let mut me = server;
    for (_, inp) in steps {
        predict(world, &mut me, &mut hunger, creative, drains, inp);
    }
    (me, hunger)
}

/// Answers in a row from before this player's last edit that are left out (half a second at TICK_HZ): edits
/// faster than the round trip would else leave every one out.
pub(crate) const SKIPS_MAX: u32 = 10;

/// Whether the server's answer for `step` moves the player (ADR 0019): `off` the move, `skipped` the answers
/// in a row not taken.
pub(crate) fn takes_answer(step: u32, edit_after: u32, off: f64, skipped: u32) -> bool {
    // (one from before the last edit replays pre-edit steps in the edited world, so it is left out, unless it is
    // a teleport or a respawn, or too many in a row were)
    (step.wrapping_sub(edit_after) as i32) >= 0 || off >= 2.0 || skipped >= SKIPS_MAX
}

#[test]
fn an_answer_from_before_an_edit_moves_the_player_only_when_it_must() {
    // past the edit, or exactly at it: taken
    assert!(takes_answer(100, 100, 0.3, 0) && takes_answer(101, 100, 0.3, 0));
    // from before it: its replay would run pre-edit steps in the edited world
    assert!(!takes_answer(97, 100, 0.3, 0));
    // a teleport or a respawn is never left out
    assert!(takes_answer(97, 100, 50.0, 0));
    // edits faster than the round trip: every answer is from before the last one, so after a while one goes through
    let mut skipped = 0;
    let mut taken = false;
    for k in 0..100u32 {
        if takes_answer(100 + k * 5, 103 + k * 5, 0.3, skipped) {
            taken = true;
            break;
        }
        skipped += 1;
    }
    assert!(taken && skipped <= SKIPS_MAX, "{skipped} answers in a row left out");
}

/// Sprint widens the view this much, degrees, eased in and out over SPRINT_EASE seconds (ADR 0118).
pub(crate) const SPRINT_FOV: f64 = 8.0;
const SPRINT_EASE: f64 = 0.15;

/// Sprint's widening `dt` seconds on from `cur`, towards SPRINT_FOV while `on`, else none.
pub(crate) fn sprint_fov(cur: f64, on: bool, dt: f64) -> f64 {
    let (want, most) = (if on { SPRINT_FOV } else { 0.0 }, SPRINT_FOV * dt / SPRINT_EASE);
    cur + (want - cur).clamp(-most, most)
}

#[test]
fn the_sprint_view_eases_to_eight_degrees_and_back() {
    let run = |mut v: f64, on: bool, n: usize| {
        for _ in 0..n {
            v = sprint_fov(v, on, 1.0 / 120.0);
        }
        v
    };
    assert!((run(0.0, true, 9) - 4.0).abs() < 1e-9, "half way at 0.075 s: {}", run(0.0, true, 9));
    assert!((run(0.0, true, 18) - 8.0).abs() < 1e-9, "all of it at 0.15 s: {}", run(0.0, true, 18));
    assert_eq!(run(8.0, true, 100), 8.0, "and no further");
    assert!((run(8.0, false, 18)).abs() < 1e-9, "back in 0.15 s: {}", run(8.0, false, 18));
}

/// The hotbar slot (0 to 8) the wheel turns to from `slot`: on to the next when it turns down, round the end.
/// How much of a tool's life is left, 0..1, for the bar on its slot (ADR 0110); None for any other stack.
pub(crate) fn wear(s: engine::items::Stack) -> Option<f32> {
    let uses = engine::items::tool(Some(s))?.uses;
    Some(engine::items::left(Some(s))? as f32 / uses as f32)
}

pub(crate) fn wheel_slot(slot: usize, down: bool) -> usize {
    (slot + if down { 1 } else { 8 }) % 9
}

#[test]
fn every_playtest_row_is_made_of_steps_the_game_knows() {
        let mut names = std::collections::HashSet::new();
        for (name, steps, _) in PLAYTEST {
            assert!(names.insert(name) && !steps.is_empty(), "{name}");
            for s in *steps {
                let known = match s.split_once(':') {
                    Some(("eclipse", w)) => w == "sun" || w == "luna",
                    Some(("fly", v)) => v.parse::<f64>().is_ok_and(|v| (1.0..=phys::FLY_MAX).contains(&v)),
                    Some(("give", w)) => w == "all",
                    _ => ["/playmode creative", "/playmode survival", "/goto terra", "/goto luna", "/heal"].contains(s)
                        || ["/hurt ", "/feed ", "/thirst "].iter().any(|c| s.strip_prefix(c).is_some_and(|n| n.parse::<u8>().is_ok_and(|n| n <= 20)))
                        || s.strip_prefix("/time set ").is_some_and(|w| ["sunrise", "day", "noon", "sunset", "night", "midnight"].contains(&w))
                        || s.strip_prefix("/give ").and_then(|r| r.rsplit_once(' ')).is_some_and(|(b, n)| BLOCKS.placeable().any(|d| d.name == b) && n.parse::<u32>().is_ok()),
                };
                assert!(known, "{name}: '{s}'");
            }
        }
    }

/// Whether press `a` acts on the aimed thing (ADR 0144, ADR 0147): the dig button whatever is in hand
/// (a plant is cut), the place button only on a pick-up and with the hand empty, so a place, a strike, a drink or
/// a window keeps its button.
pub(crate) fn on_thing(a: &str, aimed: bool, plant: bool, hand_empty: bool) -> bool {
    aimed && (a == "L" || a == "R" && hand_empty && !plant)
}

/// The place button held on a pick-up (ADR 0144): `picking` (its cell, the clock at the press) at clock
/// `now`, the aim on `aim`: let go when the aim left it or the hand took something, the cell to take once held
/// `PICK_HOLD`, else kept.
pub(crate) fn pick_step(picking: Option<(Block, f64)>, aim: Option<Block>, hand_empty: bool, now: f64) -> (Option<(Block, f64)>, Option<Block>) {
    match picking {
        Some((b, _)) if aim != Some(b) || !hand_empty => (None, None),
        Some((b, t0)) if now - t0 >= PICK_HOLD => (None, Some(b)),
        p => (p, None),
    }
}

/// What a menu opening (`m`, the book's J or the inventory's Tab among them) does to the place button's holds:
/// a pick-up held and a drink held let go, since their button's release is not seen while a menu is open (the
/// review of draft journal-book: a thing was taken with the book open, a drink ran on after it closed). The
/// message that tells the server the drink ended, if one was sent.
pub(crate) fn let_go(m: Option<Menu>, picking: &mut Option<(Block, f64)>, drinking: &mut bool, drink_sent: &mut Option<Block>) -> Option<ToServer> {
    m?;
    (*picking, *drinking) = (None, false);
    drink_sent.take().map(|_| ToServer::Drink(None))
}

#[test]
fn a_menu_opening_lets_go_of_a_held_pick_up_and_drink() {
    let b = Block { body: 0, face: 2, pos: glam::IVec3::new(1, 2, 3) };
    for m in [Menu::Book, Menu::Inventory] {
        let (mut picking, mut drinking, mut sent) = (Some((b, 1.0)), true, Some(b));
        assert_eq!(let_go(Some(m), &mut picking, &mut drinking, &mut sent), Some(ToServer::Drink(None)), "{m:?}: the server told");
        assert_eq!((picking, drinking, sent), (None, false, None), "{m:?}: both let go");
        // nothing held, nothing said; a held pick-up after its menu closed (none opening) is kept
        assert_eq!(let_go(Some(m), &mut picking, &mut drinking, &mut sent), None);
        let mut picking = Some((b, 1.0));
        assert_eq!((let_go(None, &mut picking, &mut drinking, &mut sent), picking), (None, Some((b, 1.0))));
    }
}

/// Whether the place button drinks (ADR 0145): on water in reach with nothing in hand (`drink_target`), unless a
/// pick-up is aimed at, which wins (`on_thing`): a thing lying in or by the water is picked up, not drunk past.
pub(crate) fn drinks(thing_aimed: bool, water: bool) -> bool {
    water && !thing_aimed
}

/// Whether the place button claims the aimed bed (draft death's cost): only when no window (checked before),
/// no pick-up and no drink takes the press, so a thing lying on a bed or water before leaves keeps its button.
pub(crate) fn claims(thing_aimed: bool, water: bool, bed: bool) -> bool {
    bed && !thing_aimed && !water
}

#[test]
fn a_bed_is_claimed_only_when_no_pick_up_or_drink_takes_the_button() {
    assert!(!claims(true, false, true), "a thing lying on the bed is picked up");
    assert!(!claims(false, true, true), "water before the leaves is drunk");
    assert!(claims(false, false, true) && !claims(false, false, false));
    // and the pick-up and the drink still have it
    assert!(on_thing("R", true, false, true) && drinks(false, true));
}

#[test]
fn a_pick_up_in_the_water_wins_over_drinking() {
    // aimed at a thing lying in the water with the hand empty: the pick-up, not the drink
    assert!(on_thing("R", true, false, true) && !drinks(true, true));
    assert!(drinks(false, true) && !on_thing("R", false, false, true), "no thing: the drink");
    assert!(!drinks(false, false));
}

#[test]
fn a_held_place_button_picks_a_thing_up_once_with_the_hand_empty() {
    let b = Block { body: 0, face: 2, pos: IVec3::new(5, 6, 7) };
    let other = Block { pos: IVec3::new(6, 6, 7), ..b };
    assert!(on_thing("L", true, false, false) && on_thing("L", true, false, true), "the dig button takes it whatever is in hand");
    assert!(on_thing("R", true, false, true) && !on_thing("R", true, false, false), "the place button only with the hand empty");
    assert!(!on_thing("L", false, false, true) && !on_thing("R", false, false, true), "nothing aimed at");
    assert!(on_thing("L", true, true, false) && on_thing("L", true, true, true), "a plant is cut by the dig button");
    assert!(!on_thing("R", true, true, true), "the place button does not pick a plant");
    // held: nothing before PICK_HOLD, the take once at it, then nothing more
    let p = Some((b, 10.0));
    assert_eq!(pick_step(p, Some(b), true, 10.0 + PICK_HOLD * 0.5), (p, None));
    let (p2, took) = pick_step(p, Some(b), true, 10.0 + PICK_HOLD);
    assert_eq!((p2, took), (None, Some(b)));
    assert_eq!(pick_step(p2, Some(b), true, 11.0), (None, None), "once");
    // the aim left it, or the hand took something: let go, even past the time
    assert_eq!(pick_step(p, Some(other), true, 12.0), (None, None));
    assert_eq!(pick_step(p, None, true, 12.0), (None, None));
    assert_eq!(pick_step(p, Some(b), false, 12.0), (None, None));
}

#[test]
fn the_wheel_steps_through_the_hotbar_round_its_ends() {
    // bugs/2026-11-11_13-14-13
    assert_eq!((wheel_slot(0, true), wheel_slot(4, true), wheel_slot(8, true)), (1, 5, 0));
    assert_eq!((wheel_slot(0, false), wheel_slot(5, false)), (8, 4));
}

impl App {
    /// One input of run=play's building, from the keyboard and mouse or from acts=: L/R press and l/r release
    /// the dig and place buttons, h takes up or puts away the build hammer, b the bucket, q cycles its box, cylinder and
    /// cone, up/down change a cylinder's or cone's height, C and c press and let go of the zoom, 1-9 pick a hotbar slot,
    /// m:dx:dy turns the look.
    pub(crate) fn act(&mut self, a: &str) {
        match a {
            // a block with a window (a workbench, ADR 0140): the place button uses it, whatever is in
            // hand, before a pick-up, a drink, a pour, a strike or a place against it
            "R" if self.use_target().is_some() => self.use_block(),
            // a pick-up aimed at by its own faces (ADR 0144): the dig button takes it at once (it has
            // no dig rule), the place button held `PICK_HOLD` with the hand empty picks it up
            "L" | "R" if on_thing(a, self.aim_thing.is_some(), self.aim_plant(), self.hand_empty()) => {
                let b = self.aim_thing.unwrap().0;
                if a == "L" {
                    self.take(b);
                } else {
                    self.picking = Some((b, self.start.elapsed().as_secs_f64()));
                }
            }
            // the place button with nothing in hand on water drinks while held (ADR 0145)
            "R" if drinks(self.aim_thing.is_some() && !self.aim_plant(), self.drink_target().is_some()) => self.drinking = true,
            // a sleeping place claimed (draft death's cost): last of the uses, the server answers in chat
            "R" if claims(self.aim_thing.is_some() && !self.aim_plant(), self.drink_target().is_some(), self.bed_target().is_some()) => {
                self.link.send(ToServer::Use(self.bed_target().unwrap()));
            }
            "r" if self.drinking => self.drinking = false,
            "L" | "R" if self.bucket => {
                if let Some((_, before)) = self.aim {
                    // its own seq, as every message its own `Done` answers (a shared one is cleared by another's)
                    self.link.send(ToServer::Bucket { seq: self.pred.fresh(), at: before, fill: a == "L" });
                }
            }
            // knapping by doing (ADR 0110): the place button with a knapping stone in hand on hard rock strikes it
            "R" if self.knap_target().is_some() || self.prospect_target().is_some() => self.strike(),
            "L" | "R" => {
                let dig = a == "L";
                if let Some(at) = self.aim.map(|(hit, before)| self.anchor(hit, before, dig)) {
                    self.held = Some((dig, at, at));
                }
            }
            "l" | "r" => {
                if a == "r" {
                    self.picking = None;
                }
                if a == "l" {
                    self.dig_again = false;
                }
                // a survival dig goes once held for its time (`dig_step`), not on letting go (ADR 0034)
                if let Some((dig, at, end)) = self.held.take().filter(|h| h.0 == (a == "l") && !(h.0 && !self.creative)) {
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
                if body == engine::world::INERTIAL {
                    self.note("no waypoints out in space yet: stand on a body".into());
                } else {
                    let rel = at - w.bodies[body].centre;
                    let name = nav::next_name(&self.waypoints);
                    let line = self.waypoint_line(&name, rel, body);
                    self.waypoints.push(nav::Waypoint::new(name, body, rel, self.waypoints.len()));
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
            // pitch:deg looks up or down, a scripted shot's aim (the mouse's look)
            a if a.starts_with("pitch:") => self.pitch = a[6..].parse::<f64>().map_or(self.pitch, |d| d.to_radians().clamp(-1.55, 1.55)),
            a if a.starts_with("scroll:") => self.scroll_by(a[7..].parse().unwrap_or(0)),
            a if a.starts_with("bind:") => self.bind_to(Bind::parse(&a[5..])),
            // key+:name and key-:name press and let go of a key or button itself (KeyW, Mouse1), through the
            // settings' bindings as the keyboard does
            a if a.starts_with("key+:") || a.starts_with("key-:") => {
                if let Some(b) = Bind::parse(&a[5..]) {
                    self.bind_event(b, a.starts_with("key+"), true);
                }
            }
            // the wheel (bugs/2026-11-11_13-14-13): a cylinder's or cone's height while the hammer holds one,
            // else the flight speed while flying, else the hotbar's slot as the number keys pick it
            "up" | "down" if self.hammer && self.tool.kind != Kind::Box => {
                self.tool.height = (self.tool.height + if a == "up" { 1 } else { -1 }).clamp(1, edit::MAX_SIDE);
            }
            "up" | "down" if self.flying => {
                let f = if a == "up" { 1.25 } else { 1.0 / 1.25 };
                self.fly_speed = (self.fly_speed * f).clamp(1.0, phys::FLY_MAX);
            }
            "up" | "down" => self.act(&(wheel_slot(self.slot, a == "down") + 1).to_string()),
            // the waypoints' marks on and off (bugs/2026-11-11_13-16-39), kept in the settings
            // the globe map (ADR 0077): M, and its acts= inputs
            a if a == "map" || a.starts_with("globe:") => self.globe_act(a),
            "waypoints" => {
                let on = !self.settings.on(S::Marks);
                self.settings.set(S::Marks, on as u8 as f64);
                self.save_settings();
                self.note(if on { "waypoints shown".into() } else { "waypoints hidden".into() });
            }
            // F10: the playtest menu, opened or closed (bugs/2026-11-11_13-12-42)
            "f10" if !self.texting() => self.set_menu(match self.menu {
                None => Some(Menu::Playtest),
                Some(Menu::Playtest) => None,
                m => m,
            }),
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
            // the inventory (ADR 0075): its key, a pick in the world, the screen's clicks
            "inventory" => self.toggle_inventory(),
            // the journal's book (draft ADR journal-book): its key; entry:<name> chooses an entry in it (acts=)
            "journal" => self.toggle_book(),
            a if a.starts_with("entry:") && self.menu == Some(Menu::Book) => {
                if let Some(k) = engine::journal::JOURNAL.index(&a[6..]) {
                    self.book.select(k);
                }
            }
            "pick" if self.menu.is_none() => self.pick_block(),
            "sclick" if self.menu == Some(Menu::Inventory) => self.inv_click(false, true),
            "rclick" if self.menu == Some(Menu::Inventory) => self.inv_click(true, false),
            // the recipe viewer by script (ADR 0140): view:<item> shows its recipe, fill presses Fill
            a if a.starts_with("view:") && self.menu == Some(Menu::Inventory) => {
                (self.viewing, self.lack) = (engine::items::ITEMS.named(&a[5..].replace('_', " ")), Vec::new());
            }
            "fill" if self.menu == Some(Menu::Inventory) => self.fill(),
            // a furnace by script (ADR 0141): sslot:k shift-clicks the ape's slot k (into the furnace open),
            // cast:<name> chooses its cast as a click on it does
            a if a.starts_with("sslot:") && self.menu == Some(Menu::Inventory) => {
                let l = self.inv_layout();
                if let Some(b) = a[6..].parse::<u16>().ok().and_then(|k| l.spots.iter().find(|s| (s.0, s.1) == (engine::items::Holder::Ape, k))) {
                    self.pointer = ((b.2[0] + 8.0) * l.k, (b.2[1] + 8.0) * l.k);
                    self.inv_click(false, true);
                }
            }
            a if a.starts_with("cast:") => {
                if let Some(b) = self.furnace.as_ref().map(|f| f.0) {
                    self.link.send(ToServer::Choose { at: b, cast: Some(a[5..].replace('_', " ")) });
                }
            }
            "back" if self.menu == Some(Menu::Inventory) => {
                self.inv_query.pop();
                self.inv_scroll = 0;
            }
            a if a.starts_with("type:") && self.menu == Some(Menu::Inventory) => {
                if self.inv_tab == inventory::SEARCH && self.inv_query.chars().count() < 24 {
                    self.inv_query.extend(a[5..].chars().filter(|c| !c.is_control()));
                    self.inv_scroll = 0;
                }
            }
            "esc" => self.set_menu(match self.menu {
                None => Some(Menu::Pause),
                Some(Menu::Inventory) | Some(Menu::Book) => None,
                Some(Menu::Pause) | Some(Menu::Confirm) => None,
                Some(Menu::Settings) => Some(self.home()),
                Some(Menu::Page(_)) => Some(Menu::Settings),
                Some(Menu::Title | Menu::Worlds) => Some(Menu::Title),
                Some(Menu::Remove(_) | Menu::NewWorld) => Some(Menu::Worlds),
                Some(Menu::Waypoints) => Some(Menu::Pause),
                Some(Menu::Waypoint(_)) => Some(Menu::Waypoints),
                Some(Menu::Playtest) | Some(Menu::Globe) => None,
                Some(Menu::Dead) => Some(Menu::Dead),
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
                let (at, hints) = complete(&t);
                if let Some(h) = hints.get(self.pick.unwrap_or(0).min(hints.len().saturating_sub(1))) {
                    let mut line = format!("{}{h}", &t[..at]);
                    // on to the next word when there is one to pick
                    if !complete(&format!("{line} ")).1.is_empty() {
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
                self.follow_search();
            }
            // a slider's number takes digits and a point only; the keys' search and a waypoint's name take words
            a if a.starts_with("type:") && self.edit.is_some() => {
                let words = matches!(self.menu, Some(Menu::Waypoint(_) | Menu::NewWorld)) || self.page_row(0) == Some(Row::Search);
                let t = &mut self.edit.as_mut().unwrap().1;
                let room = (if words { 32usize } else { 6 }).saturating_sub(t.chars().count());
                t.extend(a[5..].chars().filter(|c| if words { !c.is_control() } else { c.is_ascii_digit() || *c == '.' }).take(room));
                self.follow_search();
            }
            a if a.starts_with("type:") => {
                if let Some(t) = self.typing.as_mut().or(self.bug.as_mut()) {
                    t.push_str(&a[5..]);
                }
                self.pick = Some(0);
            }
            a if a.starts_with("say:") => self.link.send(ToServer::Chat(a[4..].to_string())),
            // fly:<m/s> as the playtest menu's step: flying at that speed, for scripted flights (ADR 0073)
            a if a.starts_with("fly:") => (self.flying, self.fly_speed) = (true, a[4..].parse::<f64>().unwrap_or(250.0).clamp(1.0, phys::FLY_MAX)),
            _ => {
                if let Ok(d) = a.parse::<usize>() {
                    if self.slot != d.clamp(1, 9) - 1 {
                        self.audio.play("ui.tick", None, 1.0, 1.0, false);
                    }
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

    /// While the keys' search is typed into, the rows under it follow each key.
    fn follow_search(&mut self) {
        if let (Some((_, t)), Some(Row::Search)) = (&self.edit, self.page_row(0)) {
            self.key_query = t.clone();
            (self.sel, self.scroll) = (None, 0);
        }
    }

    /// F10's eclipses (bugs/2026-11-11_13-12-42): the clock to the next one the sky's functions show, the
    /// player where it is seen best, and a line saying where to look. Luna's shadow on Terra: where its middle
    /// meets the ground, 20 s before, so it comes over; Luna in Terra's shadow: under Luna, at that moment.
    /// A row of the playtest menu: its steps in turn, then the line that says what to look for.
    pub(crate) fn playtest(&mut self, k: usize) {
        let (_, steps, look) = PLAYTEST[k];
        for step in steps {
            match *step {
                "eclipse:sun" => self.eclipse(false),
                "eclipse:luna" => self.eclipse(true),
                s if s.starts_with("fly:") => (self.flying, self.fly_speed) = (true, s[4..].parse().unwrap_or(250.0)),
                s => self.link.send(ToServer::Chat(s.to_string())),
            }
        }
        if !look.is_empty() {
            self.note(look.to_string());
        }
    }

    fn eclipse(&mut self, lunar: bool) {
        let sky = self.world.sky;
        let Some(t) = nav::next_eclipse(&sky, self.world.time, lunar) else {
            self.note("no eclipse in the sky's whole cycle: Luna's orbit never lines up".into());
            return;
        };
        let (bodies, pose) = (self.world.bodies.clone(), sky.planet(t));
        let b = &bodies[0];
        // the shadow's middle on the ground: on the sea-level sphere first, then again at the ground's height there
        let on_ground = |r: f64| nav::shadow_at(&sky, t, r);
        let x = if lunar {
            Some(sky.moon(t).centre - pose.centre)
        } else {
            on_ground(b.radius).and_then(|x| on_ground(b.top(pose.rot.inverse() * x.normalize())))
        };
        let Some(x) = x else {
            self.note("the next eclipse's shadow misses Terra".into());
            return;
        };
        let d = pose.rot.inverse() * x.normalize();
        // over the ground and over the sea; a creative player hovers there
        let feet = b.centre + d * ((b.stand_point(d) - b.centre).length().max(b.top(d) + 0.6));
        let eye = pose.to_inertial(b.centre, feet + d * 1.7);
        let target = if lunar { sky.moon(t).centre } else { sky.sun(t) };
        let look = pose.rot.inverse() * (target - eye).normalize();
        let (up_deg, bearing) = (look.dot(d).clamp(-1.0, 1.0).asin().to_degrees(), nav::bearing(d, look));
        let lead = if lunar { 0.0 } else { 20.0 };
        let what = if lunar {
            format!("Luna in Terra's shadow ({:.0}% of Sol hidden from it)", sky.covered(t, sky.moon(t).centre, pose.centre, sky.planet_radius) * 100.0)
        } else {
            format!("Luna over Sol in {lead:.0} s ({:.0}% hidden here)", sky.eclipse(t, eye) * 100.0)
        };
        let line = format!("{what}: look {up_deg:.0} degrees up, bearing {bearing:.0}, from {}", nav::geo_text(nav::geo(d, self.meridian(0))));
        self.link.send(ToServer::Chat(format!("/time set {:.2}", t - lead)));
        self.link.send(ToServer::Move { pos: feet.to_array(), fwd: look.as_vec3().to_array(), pitch: 0.0 });
        self.flying |= self.creative;
        self.note(line);
    }

    /// The server's health and hunger for this player (ADR 0040): a drop flashes and health's is heard;
    /// death opens the death screen, the respawn closes it.
    pub(crate) fn vitals_in(&mut self, health: u8, hunger: u8, thirst: u8, dead: Option<String>) {
        let now = Instant::now();
        if health < self.vitals.0 {
            self.flash[0] = (now, self.vitals.0);
            self.audio.play("hurt", None, 1.0, 1.0, true);
        }
        if hunger < self.vitals.1 {
            self.flash[1] = (now, self.vitals.1);
        }
        if thirst < self.vitals.2 {
            self.flash[2] = (now, self.vitals.2);
        }
        (self.vitals, self.health_min) = ((health, hunger, thirst), self.health_min.min(health));
        match dead {
            Some(cause) if self.dead.is_none() => {
                eprintln!("[technoape] {cause}");
                (self.dead, self.flying, self.drinking) = (Some(cause), false, false);
                self.set_menu(Some(Menu::Dead));
            }
            None if self.dead.is_some() => {
                self.dead = None;
                if self.menu == Some(Menu::Dead) {
                    self.set_menu(None);
                }
            }
            _ => {}
        }
    }

    /// The completions of the command being typed.
    pub(crate) fn hints(&self) -> Vec<String> {
        self.typing.as_deref().map_or(Vec::new(), |t| complete(t).1)
    }

    /// The field of view drawn now, in degrees: the setting's, widened by a sprint, narrowed by the zoom.
    pub(crate) fn fov(&self) -> f64 {
        2.0 * (((self.settings.get(S::Fov) + self.sprint_fov).to_radians() / 2.0).tan() / self.zoom).atan().to_degrees()
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
        if let (Some((_, t)), Some(Menu::Waypoint(j))) = (&self.edit, self.menu) {
            // a waypoint's new name; an empty one keeps the old
            let name = t.trim().to_string();
            self.edit = None;
            if let Some(w) = self.waypoints.get_mut(j).filter(|_| !name.is_empty()) {
                w.name = name;
                self.save_waypoints();
            }
            return;
        }
        if let (Some((k, t)), Some(Menu::NewWorld)) = (&self.edit, self.menu) {
            // a new world's name (an empty one keeps the old) or its seed (an empty one is a new seed)
            match (*k, menu::safe_name(t)) {
                (menu::NW_NAME, name) if !name.is_empty() => self.new_world.name = name,
                (menu::NW_SEED, _) => self.new_world.seed = t.trim().to_string(),
                _ => {}
            }
            self.edit = None;
            return;
        }
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
            // /sound <name> plays a row of data/sounds.toml here, for the developer's ears
            if let Some(n) = line.strip_prefix("/sound") {
                let n = n.trim();
                if self.audio.has(n) {
                    self.audio.play(n, None, 1.0, 1.0, true);
                } else {
                    self.note(format!("No sound '{n}'. Try dig.stone, step.grass, place.metal, ui.click."));
                }
                return;
            }
            // /spawn <creature>[:speed] stands a row of data/creatures.toml walking in place three metres ahead
            // (the way the body faces), in the first air there: the developer's preview (ADR 0053)
            if let Some(n) = line.strip_prefix("/spawn").filter(|n| n.is_empty() || n.starts_with(' ')) {
                let mut cell = self.world.block_at(self.me.pos + self.me.fwd * 3.0);
                for _ in 0..8 {
                    if !Shape::one(cell).fits(&self.world) || !BLOCKS.solid(self.world.block(cell)) {
                        break;
                    }
                    cell.pos.z += 1;
                }
                let said = self.models.spawn(n.trim(), cell).unwrap_or_else(|e| e);
                self.note(said);
                return;
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
        // the ground's air (ADR 0055, W3): the baked year's mean where the map keeps it (the biome's band on an
        // older map), the season and the hour on top
        let (face, fi, fj) = (blk.face as usize, blk.pos.x as f64 + 0.5, blk.pos.y as f64 + 0.5);
        let ap = engine::bake::air::Params::default();
        let over_sea = b.over_sea(feet).unwrap_or(0.0);
        let air = match if space || bi != 0 { None } else { b.biome_at(face, fi, fj, up) } {
            // over the air's top layer there is no air to read (the lapse would pass absolute zero)
            Some(_) if over_sea > ap.tops[ap.tops.len() - 1] as f64 => format!("none, {:.0} m over the air's top", over_sea - ap.tops[ap.tops.len() - 1] as f64),
            Some(bio) => {
                let (mean, swing) = match b.climate_at(face, fi, fj) {
                    Some((theta, swing, _)) => ((theta - ap.cool(over_sea as f32)) as f64, swing as f64),
                    None => engine::sky::BIOME_CLIMATE[bio as usize],
                };
                // the live air (W3) where the server runs it, else the baked year with the season and hour on top
                let (c, rain) = match self.air {
                    Some(a) => (a.temp as f64, format!(", rain {:.2} mm/h, cloud {:.0} %, wind {:.1} m/s", a.rain, a.cover * 100.0, a.wind[0].hypot(a.wind[1]))),
                    None => (engine::sky::temperature(mean, swing, up.z.clamp(-1.0, 1.0).asin(), w.sky.declination(w.time), w.sky.hour_angle(w.time, up)), String::new()),
                };
                format!("{c:.1} C, the year's mean here {mean:.0} C ({}){rain}", engine::bake::life::BIOMES[bio as usize].0)
            }
            None => "no air reading here".into(),
        };
        let sea = b.over_sea(pos).map_or(String::new(), |h| format!(", {h:.1} m over the sea"));
        let look = match self.sensed.as_ref().and_then(|s| s.2.as_ref()) {
            Some(l) => {
                let (edit::Look::Block(hit) | edit::Look::Fluid(hit, _)) = l.what;
                format!("{}: {} (face {}, {} {}, layer {})", l.name, l.note, hit.face, hit.pos.x, hit.pos.y, hit.pos.z)
            }
            None => "nothing in reach".into(),
        };
        let felt = self.sensed.as_ref().map_or("not yet".into(), |s| format!("{:.1} C: the air {:.1} C, {:.1} C radiant", s.1.temp, s.1.air, s.1.radiant));
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
                    ("Feet in", if space { "no block".into() } else if b.gas { "gas: no blocks, the cloud tops hold you".into() } else { format!("face {} at {} {}, layer {}", blk.face, blk.pos.x, blk.pos.y, blk.pos.z) }, 0),
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
                    ("Air", air, 0),
                    ("Felt", felt, 0),
                    ("Field", {
                        // the home's magnetic field, `field=` scaling it for the run, and its aurora (0164-magnetic-field)
                        let f = w.field(0) * crate::frame::field_scale(&self.args);
                        let a = engine::sky::aurora(f);
                        let aurora = if a.bright > 0.0 { format!("aurora at {:.0} degrees latitude, {:.0} % bright", 90.0 - a.colat.to_degrees(), a.bright * 100.0) } else { "no aurora".into() };
                        format!("{f:.3} of {}'s at the start, {aurora}", w.bodies[0].name)
                    }, 0),
                    ("Moon", format!("{} {:.0}% lit", w.bodies.get(w.moon).map_or("none", |b| b.name.as_str()), d.moon_lit * 100.0), 0),
                    ("Seed", format!("{}", self.seed), 0),
                    // (a gas body has no columns: its radius, step 6a)
                    ("Size", w.bodies.iter().map(|b| if b.gas { format!("{} gas {:.0} km", b.name, b.radius / 1000.0) } else { format!("{} {}", b.name, b.n) }).collect::<Vec<_>>().join(", ") + " columns a face", 0),
                ],
            ),
            section(
                "Player",
                {
                    // ADR 0040: points of 20, two to a segment; creative has neither
                    let (hp, food, drink) = self.vitals;
                    let of = |v: u8, low: bool| if self.creative { ("none in creative".to_string(), 0) } else { (format!("{v} of 20"), if v == 0 { 3 } else if low { 2 } else { 1 }) };
                    let ((h, ht), (f, ft), (d, dt)) = (of(hp, hp <= 4), of(food, food <= 4), of(drink, drink <= 4));
                    let h = match &self.dead {
                        Some(c) => format!("dead: {c}"),
                        None if !self.creative => format!("{h}, lowest {}", self.health_min),
                        None => h,
                    };
                    vec![
                        ("Mode", if self.creative { "creative" } else { "survival" }.into(), 0),
                        ("Health", h, ht),
                        ("Hunger", f, ft),
                        ("Thirst", d, dt),
                        ("Breath", format!("{} of 20", self.breath), if self.breath == 0 { 3 } else if self.breath < 20 { 2 } else { 0 }),
                        ("Tool", tool, 0),
                        ("Slot", format!("{}: {block}", self.slot + 1), 0),
                    ]
                },
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
        let inv: Vec<String> = self.inv.iter().enumerate().filter_map(|(k, s)| s.map(|s| format!("{k}: {} {}", engine::items::ITEMS.name(s.item), s.n))).collect();
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
    /// Takes the pick-up in air cell `at` (ADR 0082): one call, the server answers; until it refuses, or the
    /// thing is gone, the cell is not aimed at again, so one click sends one take.
    pub(crate) fn take(&mut self, at: Block) {
        // its own seq: a refused item move's `Done` of a shared one would clear `taking`
        let seq = self.pred.fresh();
        self.link.send(ToServer::Take { seq, at });
        self.taking = Some((at, seq));
        self.swung(self.start.elapsed().as_secs_f64());
    }

    /// Nothing in the hotbar slot in hand, no hammer and no bucket (ADR 0144).
    pub(crate) fn hand_empty(&self) -> bool {
        !self.hammer && !self.bucket && self.inv.get(self.slot).copied().flatten().is_none()
    }

    /// Whether all that taking or cutting `th` gives fits in the slots (ADR 0082, ADR 0147): else a click
    /// does what it does on the block behind.
    pub(crate) fn fits(&self, th: &engine::clutter::Thing) -> bool {
        engine::clutter::CLUTTER[th.row].item().is_some_and(|(item, n)| {
            let mut slots = self.inv.clone();
            !engine::items::stacks(item, n as u32).any(|st| engine::items::give(&mut slots, st) > 0)
        })
    }

    /// The aimed thing is a plant (ADR 0147).
    pub(crate) fn aim_plant(&self) -> bool {
        self.aim_thing.is_some_and(|(_, th)| engine::clutter::CLUTTER[th.row].kind == engine::clutter::Kind::Plant)
    }

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

    /// The block in hotbar slot `k` (ADR 0074): what its stack holds, when that is a block one can place.
    pub(crate) fn slot_block(&self, k: usize) -> Option<u16> {
        self.inv.get(k).copied().flatten().map(|s| s.item).filter(|&m| m != AIR && BLOCKS.get(m).is_some())
    }

    /// The water cell the place button drinks from (ADR 0145): in survival, alive, no bucket and
    /// nothing at all in hand (a knapping stone or a tool strikes instead), the cell before the aimed block under
    /// water and within `items::BENCH_REACH`, as the server's rule (which also refuses the sea's: salt).
    pub(crate) fn drink_target(&self) -> Option<Block> {
        let (_, before) = self.aim.filter(|_| !self.creative && self.dead.is_none() && self.hand_empty())?;
        let c = self.world.block_centre(before);
        (c.distance(self.me.pos) <= engine::items::BENCH_REACH && self.world.water_surface(c).is_some()).then_some(before)
    }

    /// The tool in hand, in survival (ADR 0110): what a dig's time comes from, as the server's does.
    pub(crate) fn hand_tool(&self) -> Option<&'static engine::items::Tool> {
        engine::items::tool(self.inv.get(self.slot).copied().flatten()).filter(|_| !self.creative)
    }

    /// The hard rock aimed at while a knapping stone is in hand in survival, bare-handed (ADR 0110).
    pub(crate) fn knap_target(&self) -> Option<Block> {
        let st = self.inv.get(self.slot).copied().flatten()?;
        let (hit, _) = self.aim?;
        let rock = BLOCKS.get(self.world.block(hit)).is_some_and(engine::items::knaps);
        (rock && !self.creative && !self.hammer && engine::items::ITEMS.get(st.item)?.knap.is_some()).then_some(hit)
    }

    /// The hard rock aimed at while a prospecting tool is in hand in survival (ADR 0138).
    pub(crate) fn prospect_target(&self) -> Option<Block> {
        let (hit, _) = self.aim?;
        let rock = BLOCKS.get(self.world.block(hit)).is_some_and(engine::items::knaps);
        (rock && !self.hammer && self.hand_tool()?.prospects().is_some()).then_some(hit)
    }

    /// A strike of the knapping stone on the aimed rock (ADR 0110): told to the server, which counts it and swaps
    /// the stone for what it knaps into; heard and seen here at once, a few chips of the rock and the knock. No
    /// more than one every `items::KNAP_WAIT_S` and a tick, so a strike shown is never one the server's own wait
    /// drops for jitter.
    fn strike(&mut self) {
        let now = self.start.elapsed().as_secs_f64();
        // or of a prospecting tool (ADR 0138): the server's answer brings its knock
        let prospect = self.prospect_target();
        let wait = if prospect.is_some() { engine::items::PROSPECT_WAIT_S } else { engine::items::KNAP_WAIT_S };
        let Some(hit) = prospect.or(self.knap_target()).filter(|_| now - self.struck >= wait + 1.0 / engine::net::TICK_HZ) else { return };
        self.struck = now;
        // the slot in hand first: a press in the same frame as a change of slot strikes with the new one
        if self.hand_sent != Some(self.slot) {
            self.link.send(ToServer::Hand(self.slot as u8));
            self.hand_sent = Some(self.slot);
        }
        self.link.send(ToServer::Strike(hit));
        let m = self.world.block(hit);
        self.fx.play(fx::id("knap"), &fx::At::cell(&self.world, hit, m));
        self.struck_at = self.world.block_centre(hit);
        if prospect.is_none() {
            self.audio.play("knap", Some(self.struck_at), 1.0, 1.0, false);
        }
        self.swung(now);
    }

    /// One swing of the hand (ADR 0115): a dig, a place, a strike, a pick-up, each hit of a held dig; the block
    /// drawn in hand dips on it (ADR 0031), a model in hand or the bare arm strikes in toward the aim (models.rs). A new one
    /// starts it over.
    pub(crate) fn swung(&mut self, now: f64) {
        self.dip = fx::Track { tween: fx::Tween::new(now, engine::hand::HAND.seconds, fx::Curve::Pulse), from: [0.0; 2], to: [-46.0, 60.0] };
    }

    pub(crate) fn held_count(&self, m: u16) -> u32 {
        engine::items::count(&self.inv, m)
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
        // what the player sees of it at once (ADR 0031): the hand dips, the cells break or place with their effects
        let now = self.start.elapsed().as_secs_f64();
        self.swung(now);
        // the cells' deltas, as the server's are (frame.rs): each its effect, the first its sound, heard the
        // frame of the click (fundamentals.md)
        let me = &self.me;
        let olds = edit::changes(&self.world, s, m, |c| phys::holds(&self.world, me, c));
        let (seq, cells) = self.pred.edit(&mut self.world, s, m, !self.creative, |w, c| phys::holds(w, me, c));
        for &(b, was) in &olds {
            fx::changed(&mut self.fx, &self.world, b, was, self.world.block(b));
        }
        if let Some(&(b, was)) = olds.first() {
            self.heard(b, was, self.world.block(b), true);
        }
        if self.fx.on && !dig && cells.len() == 1 {
            // the effect draws it this frame, so the click shows at once
            self.grow(cells[0], m);
            self.shown.push((0, 0.0));
        } else {
            for c in &cells {
                self.streamer.touch_block(&self.world, *c);
            }
            if !cells.is_empty() {
                self.clicks.push((self.streamer.frame, Instant::now(), cells));
            }
        }
        // the steps taken so far go first: the server takes every step sent before an edit before it, so the
        // edit falls after step `edit_after` there as here (ADR 0015, 0019)
        self.send_steps();
        self.edit_after = self.next_step.wrapping_sub(1);
        self.link.send(ToServer::Edit { seq, shape: *s, material: m });
    }

    /// The survival dig (ADR 0034): the held dig gathers time on its shape (`edit::Hold`, as the server
    /// counts it), hits every `HIT_S`, and goes once it has its `edit::dig_time`; held on, the block under the
    /// aim is next.
    pub(crate) fn dig_step(&mut self, now: f64) {
        // the slot in hand, for the server's tool (ADR 0110)
        if self.hand_sent != Some(self.slot) {
            self.link.send(ToServer::Hand(self.slot as u8));
            self.hand_sent = Some(self.slot);
        }
        // the drink held, on the cell aimed at now: the server is told when that changes
        let drink = self.drink_target().filter(|_| self.drinking);
        if drink != self.drink_sent {
            self.link.send(ToServer::Drink(drink));
            self.drink_sent = drink;
        }
        if std::mem::take(&mut self.dig_again) {
            if let Some((hit, before)) = self.aim {
                let a = self.anchor(hit, before, true);
                self.held = Some((true, a, a));
            }
        }
        // a block with no dig time (a torch, ADR 0117) goes at the click in survival too, as itself
        if let Some((true, at, end)) = self.held.filter(|_| !self.creative && self.aim.is_some()) {
            if let Some(s) = self.shape(true, at, end).filter(|s| s.fits(&self.world) && edit::dig_time(&self.world, s) == 0.0 && s.cells().any(|c| self.world.block(c) != AIR)) {
                self.apply(&s, true);
                self.swung(now);
                (self.held, self.dig_again) = (None, false);
                return;
            }
        }
        let want = match self.held {
            Some((true, at, end)) if !self.creative && self.aim.is_some() => self.shape(true, at, end).filter(|s| s.fits(&self.world) && edit::dig_time(&self.world, s) > 0.0),
            _ => None,
        };
        if self.hold.on(want, now) {
            self.link.send(ToServer::Dig(want));
            self.dig_hit = now - HIT_S;
        }
        let Some(s) = want else { return };
        if now - self.dig_hit >= HIT_S {
            self.dig_hit = now;
            self.hit(&s);
            self.swung(now);
        }
        if self.hold.held(&s, now) >= edit::dig_time_with(&self.world, &s, self.hand_tool()) {
            self.hold.done();
            self.apply(&s, true);
            (self.held, self.dig_again) = (None, true);
        }
    }

    /// A hit of a dig on shape `s`: chips off its nearest solid cell and the dig's sound, softer and higher.
    fn hit(&mut self, s: &Shape) {
        let eye = self.me.pos;
        let near = |c: &Block| self.world.block_centre(*c).distance_squared(eye);
        let Some(c) = s.cells().filter(|c| BLOCKS.solid(self.world.block(*c))).min_by(|a, b| near(a).total_cmp(&near(b))) else { return };
        let m = self.world.block(c);
        self.fx.play(fx::id("chip"), &fx::At::cell(&self.world, c, m));
        if let Some(d) = BLOCKS.get(m) {
            self.audio.play(&format!("dig.{}", d.sound), Some(self.world.block_centre(c)), HIT_GAIN, HIT_PITCH, false);
        }
    }

    /// Cracks over the cells being dug, this frame (ADR 0034): this player's dig, the shapes it left half dug
    /// while they keep their time, and the other players' digs run on from what the server told, with their
    /// hits.
    pub(crate) fn cracks(&mut self, now: f64) {
        let tool = self.hand_tool();
        let (world, fx) = (&self.world, &mut self.fx);
        let share = |s: &Shape, held: f32| (held / edit::dig_time_with(world, s, tool).max(1e-3)).min(1.0);
        let mut draw = |s: &Shape, k: f32| {
            for c in s.cells().filter(|c| BLOCKS.solid(world.block(*c))).take(fx::CRACKS) {
                fx.crack(world, c, k);
            }
        };
        if let Some(s) = self.hold.shape() {
            draw(&s, share(&s, self.hold.held(&s, now)));
        }
        for (s, held) in self.hold.kept(now) {
            draw(&s, share(&s, held));
        }
        // a dig the server stopped telling of (its player left) ends on its own
        self.digs.retain(|d| now - d.4 < 60.0);
        for &(_, s, done, rate, since, _) in &self.digs {
            draw(&s, (done + rate * (now - since) as f32).min(1.0));
        }
        for k in 0..self.digs.len() {
            if now - self.digs[k].5 >= HIT_S {
                self.digs[k].5 = now;
                let s = self.digs[k].1;
                self.hit(&s);
            }
        }
    }

    /// A cell's delta heard (docs/design/sound.md): a dig in the sound of what was there, a place in the new
    /// block's; whether it played.
    pub(crate) fn heard(&mut self, b: Block, was: u16, now: u16, touch: bool) -> bool {
        let (what, id) = if now == AIR || engine::edit::breaks(was, now) { ("dig", was) } else { ("place", now) };
        match BLOCKS.get(id).filter(|_| was != now) {
            Some(d) => self.audio.play(&format!("{what}.{}", d.sound), Some(self.world.block_centre(b)), 1.0, 1.0, touch),
            None => false,
        }
    }

    /// A loose block the server moved from `from` to `to` (ADR 0101): it glides there, the terrain meshing `to` as
    /// air meanwhile, and sheds grains of itself; a block already gliding into `from` carries on.
    pub(crate) fn glide(&mut self, from: Block, to: Block, m: u16) {
        let now = self.start.elapsed().as_secs_f64();
        if let Some((g, shown)) = self.glides.iter_mut().find(|(g, _)| g.to == from && g.block == m) {
            self.streamer.gliding.retain(|b| *b != from);
            g.chain(to, now);
            *shown = None;
        } else {
            self.glides.push((fx::Glide::new(from, to, m, now), None));
        }
        self.streamer.gliding.push(to);
        self.fx.play(fx::id("grains"), &fx::At::cell(&self.world, from, m));
    }

    /// One placed block grows into place over its cell (ADR 0031); its own mesh waits until the grow is done
    /// (frame.rs).
    pub(crate) fn grow(&mut self, b: Block, m: u16) {
        let tag = self.fx.play(fx::id("grow"), &fx::At::cell(&self.world, b, m));
        self.growing.push((self.start.elapsed().as_secs_f64() + GROW_S, b, m, tag, false));
    }

    /// Until the world is in (ADR 0073): the loading screen over it in the same window, its bar the chunks
    /// still missing against the most there were (the bake's own bar is the title's, ADR 0057).
    pub(crate) fn loading_screen(&mut self) {
        let (lw, lh) = self.ui_size();
        let missing = self.drawn.streaming.0;
        self.load_peak = self.load_peak.max(missing);
        let f = if self.load_peak == 0 { 0.0 } else { 1.0 - missing as f32 / self.load_peak as f32 };
        let mut paint = hud::Paint::new(lw, lh);
        paint.rect(0.0, 0.0, lw, lh, [0.015, 0.02, 0.035, 1.0], 0);
        let (bw, bh, y) = ((lw * 0.5).min(720.0), 14.0, lh * 0.55);
        let x = (lw - bw) / 2.0;
        paint.text(x, y - 2.0 * hud::LINE - 6.0, "TechnoApe", [1.0, 1.0, 1.0, 0.95]);
        paint.text(x, y - hud::LINE - 6.0, &format!("Loading the world: {missing} chunks to go  {:.0}%", f * 100.0), [0.8, 0.85, 0.9, 0.95]);
        paint.rect(x - 2.0, y - 2.0, bw + 4.0, bh + 4.0, [0.0, 0.0, 0.0, 0.6], 0);
        paint.rect(x, y, bw * f, bh, [0.45, 0.8, 0.4, 0.95], 0);
        let n = paint.out.len().min(hud::RECTS_MAX);
        let gpu = self.gpu.as_ref().unwrap();
        gpu.queue.write_buffer(&gpu.ui_buf, 0, bytemuck::cast_slice(&paint.out[..n]));
        self.ui_n = n as u32;
    }

    /// Each frame of run=play: scripted acts that are due, the aim, the ghost and the 2D layer.
    pub(crate) fn build(&mut self, p: &Pose, t: f64) {
        while self.acts.last().is_some_and(|(at, _)| *at <= t) {
            let (_, a) = self.acts.pop().unwrap();
            self.act(&a);
        }
        self.globe_frame();
        // lines=stdin: each line as if typed and sent. What the player has in hand is kept: a line being
        // typed, the place in the lines sent before, and those lines themselves. A field being edited (a
        // waypoint's name) makes the lines wait; four a frame at the most, however many are written.
        for _ in 0..4 {
            if self.edit.is_some() {
                break;
            }
            let Some(l) = self.lines.as_ref().and_then(|r| r.try_recv().ok()) else { break };
            let l: String = l.trim().chars().filter(|c| !c.is_control()).take(200).collect();
            if !l.is_empty() {
                let (was, recall, sent) = (self.typing.replace(l), self.recall, self.sent.len());
                self.submit();
                (self.typing, self.recall) = (was, recall);
                self.sent.truncate(sent);
            }
        }
        while let Ok(l) = self.bug_said.1.try_recv() {
            self.note(l);
        }
        self.aim = edit::aim(&self.world, p.pos, p.fwd, AIM_REACH);
        let t = self.world.time;
        let still = &self.gpu.as_ref().unwrap().still;
        let lies = |b: Block, th: &engine::clutter::Thing| engine::clutter::lies(&engine::clutter::CLUTTER[th.row], self.streamer.taken.get(&b).copied(), engine::clutter::year(t, self.world.sky.year));
        // only one a click would take (not with the bucket, not when it does not fit, not one being taken): else
        // the click, and the outline, are the block's behind it
        let vegetation = engine::clutter::VEGETATION[self.vegetation()];
        let thing = engine::clutter::aim_thing(&self.world, still, p.pos, p.fwd, AIM_REACH, t, vegetation, lies);
        self.aim_thing = thing.filter(|(b, th)| !self.bucket && self.taking.is_none_or(|k| k.0 != *b) && self.fits(th));
        // the place button held on one pick-up picks it up; looking off it lets go
        let (picking, take) = pick_step(self.picking, self.aim_thing.map(|a| a.0), self.hand_empty(), self.start.elapsed().as_secs_f64());
        self.picking = picking;
        if let Some(b) = take {
            self.take(b);
        }
        // a workbench gone or left out of reach closes its screen (ADR 0140)
        if self.menu == Some(Menu::Inventory) && self.bench_left() {
            self.set_menu(None);
        }
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
        self.dig_step(self.start.elapsed().as_secs_f64());
        let ghost = ghost.filter(|(s, ..)| s.fits(&self.world)).map(|(s, dig, held)| {
            let colour = match (held, dig) {
                (false, _) => [255, 255, 255, 150],
                (true, true) => [255, 90, 60, 230],
                (true, false) => [110, 255, 120, 230],
            };
            (s, colour)
        });
        // a pick-up a click would take, nothing held: the white outline is on the thing (ADR 0144)
        let thing = self.aim_thing.filter(|_| self.held.is_none());
        let ghost = ghost.filter(|_| thing.is_none());
        let gpu = self.gpu.as_ref().unwrap();
        if ghost != self.ghost_of || thing != self.ghost_thing {
            let verts = match thing {
                Some((b, th)) => hud::thing_ghost(&self.world, &gpu.still, b, &th, [255, 255, 255, 150]),
                None => ghost.map(|(s, c)| hud::ghost(&self.world, &s, c)).unwrap_or_default(),
            };
            gpu.queue.write_buffer(&gpu.ghost_buf, 0, bytemuck::cast_slice(&verts));
            (self.ghost_n, self.ghost_of, self.ghost_thing) = (verts.len() as u32, ghost, thing);
        }
        let season = crate::gpu::season_here(&self.world, self.me.pos);
        // the hotbar's nine slots as they lie (ADR 0074), each drawn as the inventory draws it (ADR 0114)
        let slots: Vec<Option<hud::Slot>> = (0..engine::items::HOTBAR)
            .map(|k| self.inv.get(k).copied().flatten().map(|s| hud::Slot { count: (!self.creative).then_some(s.n as u32), wear: wear(s) }))
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
        let held = self.inv.get(self.slot).copied().flatten().map(|s| (engine::items::ITEMS.name(s.item).to_string(), shown));
        let mut rects = if title { Vec::new() } else { hud::layer(lw, lh, &slots, self.slot, self.creative, height, held.as_ref().map(|h| (h.0.as_str(), h.1)), &mut |p, k, x, y, size| {
                if let Some(s) = self.inv.get(k).copied().flatten() {
                    self.look_of(p, s.item, x, y, size, season);
                }
            }) };
        if !title && !self.creative {
            // health and hunger over the hotbar (ADR 0040): survival's, and how it is told from creative
            let mut p = hud::Paint::new(lw, lh);
            let ages = self.flash.map(|(at, from)| (at.elapsed().as_secs_f32(), from));
            hud::vitals(&mut p, self.vitals, ages, self.start.elapsed().as_secs_f32());
            hud::breath(&mut p, self.breath);
            rects.extend(p.out);
        }
        // the text over it (ADR 0023): F3's numbers, the chat, the bug report's form
        let mut paint = hud::Paint::new(lw, lh);
        if self.f3 {
            let (l, r) = self.debug_sections();
            hud::panels(&mut paint, &l, &r);
        }
        let lines: Vec<(f32, &str)> = self.chat.iter().map(|(at, l)| (at.elapsed().as_secs_f32(), l.as_str())).collect();
        let caret = self.start.elapsed().as_millis() / 400 % 2 == 0;
        let (at, hints) = complete(self.typing.as_deref().unwrap_or(""));
        if !title {
            hud::chat(&mut paint, &lines, self.typing.as_deref(), caret, at, &hints, self.pick);
        }
        // the compass and the waypoints (docs/design/navigation.md): in both modes; where you are in numbers
        // only in creative (ADR 0077)
        let mut label_y = 10.0;
        let (eye, fwd) = (p.pos, p.fwd);
        let on = self.world.frame_at(eye);
        if !title && self.menu.is_none() && !self.f3 {
            // each waypoint where it is now in the camera's frame (it may be on another body, or this one in
            // space), and its up there; those its view lets show from here, unless the marks are switched off
            let wd = &self.world;
            let here: Vec<(&nav::Waypoint, DVec3, DVec3)> = self
                .waypoints
                .iter()
                .chain(&self.marks)
                .filter(|p| p.body < wd.bodies.len() && self.settings.on(S::Marks))
                .map(|p| {
                    let (rot, off) = wd.place(p.body, on);
                    (p, rot * p.rel + off, rot * p.rel.normalize())
                })
                .filter(|(p, at, _)| p.shown(on, at.distance(eye)))
                .collect();
            let colour = |p: &nav::Waypoint| nav::colour(p.colour);
            if on != engine::world::INERTIAL {
                let b = &wd.bodies[on];
                let rel = eye - b.centre;
                let up = rel.normalize();
                let marks: Vec<(f64, [f32; 3], usize)> = here.iter().map(|(p, at, _)| (nav::bearing(up, *at - eye), colour(p), p.icon)).collect();
                let heading = nav::bearing(up, fwd);
                let text = if self.creative { format!("{}  {}  {:.0} m up", b.name, nav::geo_text(nav::geo(rel, self.meridian(on))), rel.length() - b.radius) } else { b.name.clone() };
                // the felt temperature at the eye beside the place (the font has no degree sign: "C")
                let text = match &self.sensed {
                    Some(s) => format!("{text}  {:.1} C", s.1.temp),
                    None => text,
                };
                label_y = hud::compass(&mut paint, heading, &marks, &text) + 10.0;
            }
            // this frame's view, as `frame` makes it after this: from the camera, the up kept off the look
            let cam_up = if fwd.cross(p.up).length() < 1e-4 { DVec3::Z } else { p.up };
            let proj = glam::camera::rh::proj::directx::perspective_infinite_reverse((self.fov() as f32).to_radians(), w() as f32 / h() as f32, 0.1);
            let vp = proj * glam::camera::rh::view::look_to_mat4(Vec3::ZERO, fwd.as_vec3(), cam_up.as_vec3());
            // 4. the nearest sixteen are drawn: more would crowd the view and the HUD's room for rectangles
            let mut here = here;
            here.sort_by(|a, b| a.1.distance_squared(eye).total_cmp(&b.1.distance_squared(eye)));
            here.truncate(16);
            for (w, at, up) in here {
                // a beam from where it stands to a height that grows with how far it is, so the tag stays
                // over the ground between: 3 m up close by, 60 m from 400 m on
                let far = at.distance(eye);
                let rise = (far * 0.15).clamp(3.0, 60.0);
                let screen = |d: DVec3| {
                    let clip = vp * d.as_vec3().extend(1.0);
                    (clip.w > 0.01).then(|| ((clip.x / clip.w * 0.5 + 0.5) * lw, (0.5 - clip.y / clip.w * 0.5) * lh))
                };
                if let (Some(base), Some(top)) = (screen(at - eye), screen(at + up * rise - eye)) {
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
                        hud::marker(&mut paint, base, top, colour(w), w.icon, &w.name, &far);
                    }
                }
            }
        }
        // what the crosshair is on (the developer, 2026-11-11), when no menu or overlay is over the top
        // (its lines worked out a few times a second, `sense`: a block's temperature and hardness, a fluid's name)
        if let (Some(l), None, false) = (self.sensed.as_ref().and_then(|s| s.2.as_ref()), self.menu, self.f3 || title) {
            match l.what {
                edit::Look::Block(hit) => {
                    if let Some(d) = BLOCKS.get(self.world.block(hit)) {
                        let (colour, side, pattern) = crate::gpu::shown(d, season);
                        let pt = crate::gpu::painting(d);
                        hud::looking_at(&mut paint, &l.name, colour, side, pattern, [pt[0].as_ref(), pt[1].as_ref()], &l.note, label_y);
                    }
                }
                edit::Look::Fluid(_, k) => {
                    let c = engine::fluids::FLUIDS.now().get(k).colour;
                    hud::looking_at(&mut paint, &l.name, c, c.map(|v| v * 0.7), 0, [None; 2], &l.note, label_y);
                }
            }
        }
        // the block in hand, large at the lower right, on its tracks (ADR 0031): the dip when it digs or places,
        // a bob with the steps
        if !title {
            if let (Some(d), false) = (self.slot_block(self.slot).and_then(|m| BLOCKS.get(m)), self.hammer || self.bucket || self.f3 || lw < 1100.0) {
                let [dx, dy] = self.dip.at(self.start.elapsed().as_secs_f64());
                let pace = if self.me.on_ground { (self.me.vel.length() / 4.0).min(1.0) as f32 } else { 0.0 };
                let [bx, by] = BOB.at(self.walked).map(|v| v * pace);
                let (colour, side, pattern) = crate::gpu::shown(d, season);
                let (x, y) = (lw - 250.0 + dx + bx, lh - 236.0 + dy + by);
                // a block with a model of its name is held as the model (frame.rs, ADR 0047)
                let pt = crate::gpu::painting(d);
                if !self.models.has(&d.name) { hud::cube(&mut |x, y, w, h, c, g| paint.rect(x, y, w, h, c, g), x, y, 176.0, colour, side, pattern, 1.0, [pt[0].as_ref(), pt[1].as_ref()]); }
            }
        }
        if self.flying && !title {
            let s = format!("Flying {:.0} m/s", self.fly_speed);
            // over the hotbar's right end (hud::layer: its pane ends 268 px right of the middle)
            paint.text_px(lw / 2.0 + 268.0 - s.len() as f32 * hud::ADV * hud::PX * 0.75, lh - 110.0, &s, [1.0, 1.0, 1.0, 0.9], hud::PX * 0.75);
        }
        if self.menu == Some(Menu::Globe) {
            self.menu_boxes = self.globe_draw(&mut paint, lw, lh, p.fwd);
        } else if self.menu == Some(Menu::Inventory) {
            self.inv_draw(&mut paint, season);
            self.menu_boxes.clear();
        } else if self.menu == Some(Menu::Book) {
            self.book_draw(&mut paint);
            self.menu_boxes.clear();
        } else if let Some(m) = self.menu {
            let cause = self.dead.clone().unwrap_or_default();
            if m == Menu::Dead {
                // the world under it darker and a little red, before the menu's own shade
                paint.rect(0.0, 0.0, lw, lh, [0.22, 0.0, 0.02, 0.4], 0);
            }
            let (title, hint) = match m {
                Menu::Inventory | Menu::Book => unreachable!(),
                Menu::Dead => ("You died", cause.as_str()),
                Menu::Title => ("TechnoApe", "Pillars of Simulation Engine"),
                Menu::Worlds => ("Singleplayer", "A world carries on from its save. Right click or Delete removes one."),
                Menu::NewWorld => ("New universe", "The same seed and size always make the same universe. Click a row to change it."),
                Menu::Remove(_) => ("Remove world", "It moves to saves/removed/ and can be put back by hand."),
                Menu::Waypoints => ("Waypoints", "N drops one where you stand. Click one to change it; right click or Delete takes it away."),
                Menu::Waypoint(_) => ("Waypoint", "Click the name to type a new one. Click a row for its next value, right click the one before."),
                Menu::Globe => ("", ""),
                Menu::Playtest => ("Playtest", "Set-ups to look at: the clock and you move there. F10 or Esc closes it."),
                Menu::Pause => ("Paused", "The world runs on. Esc goes back to the game."),
                Menu::Settings => ("Settings", "Click or Enter: the next value. Right click: the one before. Arrows move."),
                Menu::Page(p) => settings::PAGES[p],
                Menu::Confirm => ("Display", "Can you see this? It goes back by itself if you do nothing."),
            };
            let (items, sliders, tones) = (self.menu_items(), self.menu_sliders(), self.menu_tones());
            // as many rows as fit, from `scroll` on
            self.scroll = self.scroll.min(items.len().saturating_sub(self.menu_fit()));
            let end = (self.scroll + self.menu_fit()).min(items.len());
            let (items, sliders) = (&items[self.scroll..end], &sliders[self.scroll..end]);
            // (the tones end before the page's last row, Back, which has none)
            let tones = tones.get(self.scroll.min(tones.len())..end.min(tones.len())).unwrap_or(&[]);
            let boxes = hud::menu_boxes(lh, items, sliders);
            let (x, y) = self.pointer;
            let under = boxes.iter().position(|b| x >= b[0] && x < b[0] + b[2] && y >= b[1] && y < b[1] + b[3]);
            // the keyboard's row is the lit one until the pointer moves
            // (the row typed into stays lit: its box changes width under a still pointer as the text grows)
            let hover = match self.edit.as_ref().map(|e| e.0).or(self.sel) {
                Some(k) => k.checked_sub(self.scroll).filter(|v| *v < boxes.len()),
                None => under,
            };
            // the page slides in over a tenth of a second, the lit row out as long (ADR 0030)
            let now = Instant::now();
            if self.lit.0 != hover {
                self.lit = (hover, now);
                if hover.is_some() {
                    self.audio.play("ui.hover", None, 1.0, 1.0, false);
                }
            }
            // (off screen the rows stand still, so a scripted run's pictures repeat; motion=1 shows it there)
            let moving = self.onscreen || self.args.get("motion", 0) == 1;
            let ease = if moving { ((now - self.menu_since).as_secs_f32() / 0.14, (now - self.lit.1).as_secs_f32() / 0.1) } else { (1.0, 1.0) };
            hud::menu(&mut paint, title, items, sliders, tones, hint, hover, caret, self.scroll, ease);
            self.menu_boxes = boxes;
        }
        // a new world's bake or a server's map under way: the stage and a bar across the foot of the screen
        if let Some(l) = &self.loading {
            let (stage, f) = l.progress.get();
            let (bw, bh, y) = ((lw * 0.5).min(720.0), 14.0, lh - 120.0);
            let x = (lw - bw) / 2.0;
            let s = format!("{}: {stage}  {:.0}%", l.what, f * 100.0);
            paint.text(x, y - hud::LINE - 6.0, &s, [1.0, 1.0, 1.0, 0.95]);
            paint.rect(x - 2.0, y - 2.0, bw + 4.0, bh + 4.0, [0.0, 0.0, 0.0, 0.6], 0);
            paint.rect(x, y, bw * f, bh, [0.45, 0.8, 0.4, 0.95], 0);
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

    /// Sends the steps not sent yet (ADR 0019).
    pub(crate) fn send_steps(&mut self) {
        let inputs: Vec<Input> = self.steps.iter().filter(|s| s.0 >= self.unsent).map(|s| s.1).collect();
        if !inputs.is_empty() {
            self.link.send(ToServer::steps(self.unsent, inputs));
            self.unsent = self.next_step;
        }
    }

    /// The server's state after step `step` (ADR 0019): the steps it has not taken yet are replayed from it, and
    /// where that lands is where this player is. A difference is a correction, drawn eased out unless it is a
    /// jump of metres (a teleport, a respawn). The first answer places the player.
    pub(crate) fn reconcile(&mut self, step: u32, server: phys::Player, hunger: f64, t: f64) {
        self.server_hunger = hunger;
        if !self.spawned {
            (self.me, self.prev, self.spawned, self.hunger) = (server, server.pos, true, hunger);
            self.steps.retain(|s| s.0 > step);
            return;
        }
        self.steps.retain(|s| s.0 > step);
        // each step again at the time it was first taken (the server takes it at that time too)
        let (me, h) = replay(&mut self.world, server, (hunger, self.creative, !self.creative && self.dead.is_none()), &self.steps);
        self.hunger = h;
        let off = self.me.pos - me.pos;
        // a word from before the server took this player's last edit moves it only when it must (`takes_answer`)
        if !takes_answer(step, self.edit_after, off.length(), self.skipped) {
            self.skipped += 1;
            return;
        }
        self.skipped = 0;
        if off.length() >= 2.0 {
            // a teleport (/goto, a respawn): the look is the server's, in the frame it lands in, level; the
            // steps not answered yet only turn it
            (self.pitch, self.tilt, self.roll) = (0.0, 0.0, 0.0);
        }
        self.drift_max = self.drift_max.max(off.length());
        if off.length() > 0.001 {
            if keeps_frames(&self.run, &self.args) {
                self.snap_t.push((t, off.length()));
            }
            self.snaps += 1;
            self.path.clear(); // a correction is counted as one, not as jerk
            self.smooth = if off.length() < 2.0 { self.smooth + off } else { DVec3::ZERO };
            self.prev -= off;
        }
        self.me = me;
    }
}

/// The command line's completions: the engine's (what the server runs), and what only the files of this
/// machine name: `/structure`'s files and `/spawn`'s creatures.
fn complete(line: &str) -> (usize, Vec<String>) {
    // (asked for every frame of play: the files are looked at only while a command is being typed)
    if !line.starts_with('/') {
        return engine::net::complete(line);
    }
    let (structures, creatures) = data_names();
    complete_with(line, &structures, &creatures)
}
fn complete_with(line: &str, structures: &[String], creatures: &[String]) -> (usize, Vec<String>) {
    let (at, mut v) = engine::net::complete(line);
    let Some(rest) = line.strip_prefix('/') else { return (at, v) };
    // (the place in the line is counted in the line as typed: a small letter may be longer or shorter than
    // its capital)
    match rest.split_once(' ') {
        // the command itself: the two the engine's list has not
        None => {
            let low = rest.to_lowercase();
            v.extend(["structure", "spawn"].iter().filter(|w| w.starts_with(low.as_str()) && **w != low).map(|w| w.to_string()));
            (1, v)
        }
        // its one name (what follows a name is a number: no list)
        Some((cmd, arg)) if ["structure", "spawn"].contains(&cmd.to_lowercase().as_str()) && !arg.trim_start().contains(' ') => {
            let (arg, names) = (arg.trim_start(), if cmd.eq_ignore_ascii_case("structure") { structures } else { creatures });
            let low = arg.to_lowercase();
            (line.len() - arg.len(), names.iter().filter(|n| n.to_lowercase().starts_with(&low) && n.to_lowercase() != low).cloned().collect())
        }
        _ => (at, v),
    }
}
/// The files of data/structures and the rows of data/creatures.toml by name. The list is asked for every
/// frame while it shows, so the files are read again every two seconds at the most.
/// shortcut: that reading is on the frame thread (a folder's names and one small file, only while a command
/// is being typed); a watcher's thread should hand them over if either grows large.
fn data_names() -> (Vec<String>, Vec<String>) {
    static KEPT: std::sync::Mutex<Option<(Instant, (Vec<String>, Vec<String>))>> = std::sync::Mutex::new(None);
    let mut kept = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    if kept.as_ref().is_none_or(|(at, _)| at.elapsed().as_secs() >= 2) {
        let mut structures: Vec<String> = std::fs::read_dir("data/structures").into_iter().flatten().flatten().filter_map(|f| f.file_name().to_str()?.strip_suffix(".toml").map(str::to_string)).collect();
        structures.sort();
        let creatures = creature_names(&std::fs::read_to_string("data/creatures.toml").unwrap_or_default());
        *kept = Some((Instant::now(), (structures, creatures)));
    }
    kept.as_ref().unwrap().1.clone()
}
/// The `name = "..."` of every row of a creatures file, as written (the list is a help: /spawn itself reads
/// the file with the real reader).
fn creature_names(text: &str) -> Vec<String> {
    text.lines().filter_map(|l| l.trim().strip_prefix("name")?.trim_start().strip_prefix('=')?.trim().strip_prefix('"')?.split('"').next().map(str::to_string)).collect()
}

#[cfg(test)]
mod complete_tests {
    use super::*;

    #[test]
    fn structure_and_spawn_complete_to_the_files_names() {
        let (s, c) = (vec!["boulder".to_string(), "bush".into(), "oak".into()], creature_names(include_str!("../../../data/creatures.toml")));
        assert!(c.contains(&"boar".to_string()) && c.contains(&"ape".to_string()), "{c:?}");
        let f = |l: &str| complete_with(l, &s, &c);
        // the commands themselves, beside the engine's
        assert_eq!(f("/s"), (1, vec!["structure".to_string(), "spawn".into()]));
        assert!(f("/").1.contains(&"spawn".to_string()) && f("/").1.contains(&"time".to_string()));
        assert_eq!(f("/structure "), (11, s.clone()));
        assert_eq!(f("/structure b"), (11, vec!["boulder".to_string(), "bush".into()]));
        assert_eq!(f("/SPAWN bo"), (7, vec!["boar".to_string()]));
        // a name typed out in full, and what follows it (a seed), have no list
        assert!(f("/structure oak").1.is_empty() && f("/structure oak ").1.is_empty() && f("/spawn boar:2").1.is_empty());
        // the engine's own are as they were
        assert_eq!(f("/time s"), engine::net::complete("/time s"));
        assert_eq!(f("hello"), engine::net::complete("hello"));
        // a second space, a capital in a name, no names at all
        assert_eq!(f("/structure  b"), (12, vec!["boulder".to_string(), "bush".into()]));
        assert_eq!(complete_with("/spawn bo", &[], &["Boar".to_string()]), (7, vec!["Boar".to_string()]));
        assert!(complete_with("/structure ", &[], &[]).1.is_empty());
        // letters whose small form is shorter (the Kelvin sign) or longer (a dotted capital I): the place
        // stays inside the line and on a character's edge
        for l in ["/spawn \u{212a}", "/spawn \u{130}\u{130}\u{130}\u{130}\u{130}\u{130}\u{130}\u{130}", "/\u{212a}", "/structure \u{130}o"] {
            let (at, _) = complete_with(l, &["k".to_string(), "i\u{307}o".into()], &["k".to_string()]);
            assert!(l.is_char_boundary(at), "{l}: {at}");
        }
    }
}

#[cfg(test)]
mod looked_tests {
    use super::*;

    /// The look-at panel's lines (draft 2026-11-24): a granite block in the open air names itself with the air's
    /// temperature, its row's hardness and tool; lava poured in a pit names the fluid, its pour heat (a client keeps
    /// no node's heat) and its depth.
    #[test]
    fn the_panel_names_a_granite_block_and_lava_with_their_heat() {
        let mut w = World::new(256, 32, 1);
        let b = w.bodies[0].clone();
        let top = w.block_at(b.centre + DVec3::Z * (b.height(DVec3::Z) - 0.5));
        let at = |dz: i32| Block { pos: top.pos + IVec3::Z * dz, ..top };
        w.set_block(at(3), BLOCKS.id("granite"));
        let l = looked(&w, edit::Look::Block(at(3)), Some(21.5));
        assert_eq!((l.name.as_str(), l.note.as_str()), ("granite", "21.5 C, hardness 9, pick"));
        let lava = engine::fluids::FLUIDS.now().kind("lava").unwrap();
        w.set_block(at(-2), BLOCKS.id("stone"));
        w.set_block(at(-1), AIR);
        w.set_block(at(0), AIR);
        {
            let World { bodies, edits, water, sky, time, .. } = &mut w;
            let g = engine::world::Solid::at(bodies, edits, sky, *time);
            let c = World::col(at(0));
            water.regap(&g, c);
            assert!(water.pour_kind(&g, c, at(-1).pos.z, engine::liquid::U, lava));
        }
        let l = looked(&w, edit::Look::Fluid(at(-1), lava), Some(21.5));
        assert_eq!((l.name.as_str(), l.note.as_str()), ("Lava", "1150.0 C, 1.0 m deep"));
    }
}
