# 0118 – Sprint, and swimming

- **Status:** accepted (built; waits for the developer's play-test and merge)
- **Date:** 2026-11-14

## Context
The developer: "add a sprint mode". Walking is one speed (`phys::WALK`, 4.3 m/s, ADR 0007); the client and the
server step the same numbered inputs (ADR 0019), so anything that changes the walk must be a bit of that input
and run in the shared `phys::step`. Recordings (ADR 0011) are postcard bytes of `ToServer` messages, and postcard
keeps no field names: one more field in `Input` would make every older recording unreadable. Hunger (ADR 0040)
drains per physics step, twice as fast walking.

## Options considered (sprint)
1. **Where the bit rides.** `Input` has no spare bit: its bools are whole bytes, and a new field shifts every
   later byte. (a) A new field with `#[serde(default)]`: postcard does not read defaults for a missing field, so
   old recordings break. (b) A negative `fly` meaning "walking, sprinting": no format change, but a trick every
   reader must know. (c) **Chosen:** `Input::sprint` is `#[serde(skip)]` (in memory only, so `Input`'s bytes are
   exactly what they were), and two variants added at the end of `ToServer`, as ADR 0110 did: `Sprints { first,
   inputs: Vec<(Input, bool)> }` beside `Steps`, `Sprint(Input, bool)` beside `Input`. `ToServer::steps` and
   `ToServer::input` pick the new one only when an input sprints; the server turns them back into `Steps` and
   `Input` with the bit set (`ToServer::unpacked`) right after it logs the message, so the recorder writes the
   bit and a replay reads it. An old recording has only the old variants: every input reads as not sprinting.
2. **Toggle, double-tap W, hold.** Hold only (the brief's call). Toggle and double-tap were left out; either is a
   client-only change later (the bit stays the same).
3. **Who enforces the hunger floor.** (a) The server refuses the bit, and the client guesses from the whole points
   it is shown: a round trip late, a correction at every crossing (what was first built; the review). (b) A
   "tired" flag in `You` the client obeys: still late by the steps in flight. (c) **Chosen, exact:** `You` carries
   the server's hunger to the bit after its last answered step; `vitals::gate` refuses the bit at the hunger of
   each step, on the server at the hunger it has then and on the client at the hunger it predicts by draining each
   predicted step as the server does (`vitals::drained`, `vitals::rate`), so both refuse the same step.

## Decision (sprint)
| what | number |
|---|---|
| speed | the forward share of the walk times **1.5** (`phys::SPRINT`): 6.45 m/s straight ahead; strafe and back stay 4.3; a diagonal's forward share only |
| where | walking (survival or creative walking), out of the water (wading under 0.3 of the body still sprints), in the air too: a jump keeps the speed |
| not | creative flight (its wheel is the speed), swimming, dead (`vitals::still` clears it), a menu (the keys are not held there) |
| drain | the walk's drain times **3** (`vitals::SPRINT_DRAIN`): 6x standing, a point every 15 s; full to the floor in 3.5 min of sprinting |
| floor | no sprint at hunger **6** of 20 or less (`vitals::SPRINT_FLOOR`) |
| key | "Sprint (hold)", default **Left Ctrl**, in Keys > Moving; rebindable, kept as `key.sprint=` in `settings.txt`, red when shared |
| view | +**8** degrees of field of view, eased linearly in and out over **0.15 s**, while the last predicted step sprinted |

- `phys::step`: the move's direction (clamped to 1) gains `fwd * max(0, dir . fwd) * 0.5` when `sprint` is set and
  under 0.3 of the body is under water. The fly branch never reads it.
- `phys::step` returns whether the step **sprinted**: the sprint was applied and the body went further forward
  than the walk's share would have (by a twentieth), so a sprint into a wall, up a ledge or in the water is none.
  The drain (`vitals::rate`: a sprint's, a walk's or standing's) and the wider view both go by it.
- Bots: `sprint on` / `sprint off`; every `walk` after `on` sprints. The soak's random walkers set the bit on 30 %
  of their inputs (its own random draw, last, so the rest of each input is drawn as before).
- `acts=`: `key+:ControlLeft` holds it through the bindings, as any key.
- There is no reduce-motion setting on the Accessibility page, so the view widens for everyone (none added).

## Evidence (sprint)
- Engine, a level floor, 60 steps from standing (`sprint_is_half_as_fast_again_forward_only`): 4.30 m walking,
  6.45 m sprinting; strafe and back the same with the bit or without; a diagonal's forward share 1.5 times, its
  strafe within 1e-4 m; flight the same to the bit.
- Server (`a_sprint_lands_where_predicted_costs_hunger_and_stops_at_the_floor`): 100 ticks (300 steps, 5 s) of
  survival sprinting steps with turns and jumps, sent as `Sprints`, land on the client's prediction to the bit;
  hunger lost 0.333 points (seen red at 0.111, the walk's rate, before the drain); at hunger 6 the bit is refused
  and the player lands on a walking prediction; creative sprints at the floor and loses nothing.
- Bot, first by real time (24 m there and back over the test planet's hills): walked in 5.79 to 5.91 s, sprinted
  in 4.08 s (seen red at 1.02 with the bit not sent); after the review by server ticks
  (`a_sprinting_bot_covers_half_as_much_again_in_the_same_ticks`): 11.08 m walked, 16.76 m sprinted in 60 ticks (1.51).
- Wire (`the_sprint_bit_rides_the_wire_and_old_inputs_walk`): the bit survives postcard through `Sprints` and
  `Sprint`; an `Input` in the old field order reads as not sprinting, and a sprinting `Input`'s own bytes equal
  the old ones.
- Soak `technoape-server soak=1 bots=50 minutes=2 seed=1 record=...` (play build, the walkers sprinting on 30 % of
  inputs): `"soak":"ok"`, hash `9ced0ff2a98f49cd`; its replay: 120 checks, `"diverged":null`, the same hash.
- Old recordings: `out/rec/play-1794621101.rec` and `play-1794579779.rec` (made before this branch) replay with
  this build to the same hashes as with a main build: both part from their recording at the same tick with the
  same replayed hash (940 and 20), so that parting is older than this change and this change adds none.
- View: `the_sprint_view_eases_to_eight_degrees_and_back`: 4 degrees at 0.075 s, 8 at 0.15 s, back to 0 in 0.15 s.
- Not measured: a `run=play` scripted sprint (distance and the feel report's corrections), and a filmstrip of the
  view's widening.

## Calls for the developer (sprint)
- 1.5 times, forward share only; the drain 3 times the walk's; the floor at 6; +8 degrees over 0.15 s.
- Left Ctrl, hold only (no toggle, no double-tap W).
- A sprint pressed in the air speeds the air move too (the step keeps no "began on the ground" state, so it needs
  no new field in `Player`, which the wire and the hash carry).

## Shortcuts
- shortcut: a dig's hunger (`Vitals::dug`) is not predicted: a dig within a few steps of the floor can still cost
  one correction there.

## Swimming (added scope: "a real swimming movement like Minecraft")

### Context
ADR 0020's water: `sub`, the share of the body under the water's surface (feet to head, 0..1), slows the walk to
`WALK * (1 - sub / 2)`; buoyancy (`G / FLOAT * sub`) and a drag hold the body with 0.8 of it under, the head out;
jump with more than 0.3 in swims up at `SWIM` (2 m/s); a ledge a block high under shallow water is jumped (W5).
Under water there was no way to dive, nothing moved along the look, and a bank over the surface could not be
climbed (the ledge rule wants the bank's top within 1.5 m of the feet, which a floating body never has).

### Decision
All in `phys::step`, so the server and the prediction agree as before (no new field, no new message):
| mode | when | what |
|---|---|---|
| wading | on the bottom, the head out | a walk at `WALK * (1 - sub / 2)` (3.38 m/s 0.8 and 1 m deep), jumps as on land; a sprint still sprints with under 0.3 of the body in |
| floating | in, the head out, no stroke | as ADR 0020: buoyancy holds 0.8 of the body under; jump swims up at 2 m/s and bobs at the surface; the down key dives at 2 m/s |
| under | the feet more than `HEIGHT + UNDER` (2.0 m) under, no stroke | no gravity or buoyancy; the velocity eases (rate 4/s) to the ground's way at `SWIM_SLOW` (2.15 m/s) plus up 2 m/s on jump, down 2 m/s on the down key (Shift, now "Fly down, swim down"), else a slow sink of `SINK` 0.6 m/s |
| stroke | sprint and forward with more than 0.6 of the body in | the velocity eases (4/s) to the look, pitch included, at `SWIM_FAST` 4 m/s (strafe at 2.15): look down to dive, up to surface; let go, it eases back to the slow move and the sink |
| climbing out | in the water, jump held (or a stroke) and a wall ahead at the feet | up at least 2 m/s along the bank; at a ledge (its top within reach) the push is `CLIMB` 6 m/s, which lifts the feet over a bank a block high |

The numbers are calls (taste, the developer may change them; each is one constant in `phys.rs`):
| constant | value | what it sets |
|---|---|---|
| `SINK` | 0.6 m/s | the slow sink under water with no key held |
| `SWIM` | 2 m/s | up on jump, down on the down key, in the water and under it |
| `SWIM_FAST` | 4 m/s | the sprint's stroke along the look |
| `SWIM_SLOW` | 2.15 m/s (`WALK / 2`) | the plain move under water, and the stroke's strafe |
| `WATER_EASE` | 4 /s | how fast the velocity eases to the wanted one under water (about 0.25 s) |
| `UNDER` | 0.2 m | how far the head must be under before the body leaves floating (no flicker at the surface) |
| `STROKE_SUB` | 0.6 | the share of the body in the water before a sprint strokes |
| `CLIMB` | 6 m/s | the push up at a ledge out of the water |
- Hunger: a stroke that moved along the look is a sprint (`phys::step` says so), so it drains the sprint's rate,
  and a stroke against a wall does not; at the floor the server refuses the bit, and the body swims slowly.
- Creative walking swims exactly as survival (no hunger); creative flight ignores the water as before.
- The down key: `Input.down` is read only in flight, under water and (now) floating with 0.3 of the body in, so
  Shift does nothing new on land or in shallow wading, and creative flight is unchanged.
- The camera stays upright: no prone view, no roll or bob. A prone swimming pose of the body for other players
  is a later step of the model work.
- Bots: a `walk` looks at its target (pitch from the height difference), so with `sprint on` in water it swims to
  it; the soak's walkers already sprint on 30 % of inputs.

### Evidence (swimming)
- Sink with the head under: 0.6 m/s (seen red at +0.65, the old buoyancy). Feet out from 5 m under, jump held:
  1.93 s; let go it floats with the head out; jump held at the surface it bobs within the bounds. The down key held
  2 s from floating: the feet 5.45 m under (seen red: 1.45 m, the buoyancy held it; the filmstrip found it).
- Stroke at pitches -0.6, 0 and 0.5: 4.0 m/s along the look within 0.1 (seen red: the old move went level at
  1.9 m/s); let go it slows smoothly. Without sprint under water: 2.15 m/s.
- Wading 0.8 m and 1 m deep: 3.38 m/s, on the bottom, neither sinking nor floating off (a guard; this held before).
- A bank 1.22 m over the surface: climbed out in 1.77 s with jump, 1.5 s with a stroke (seen red: never).
- Server (`a_dive_turn_and_surfacing_land_where_predicted`, a 1536 planet; the test planet's sea is 4 m deep): a
  2 s sprint dive at -0.9, a turn, 6 s up at +0.7 with strafe: lands on the prediction to the bit, dived to 6.72 m,
  came up with the feet 0.18 m over the surface; 335 of 480 steps stroked, the hunger lost exactly that.
  `a_sprint_held_in_the_sea_drains_only_where_it_strokes`: the drain counts the stroking steps only.
- Engine: `a_sprint_into_a_wall_is_no_sprint_and_a_stroke_is_one` (the stroke half seen red without it).
- Bot (`a_sprinting_bot_swims_across_the_sea`): 14.56 m across open sea in 4.73 s, arrived.
- Baked planet (6144, seed 1337, a dedicated server): a bot with `sprint on` crossed a 67 m inlet of the sea
  (6.9 m deep) 700 m from the spawn, shore to shore, 68.6 m in 18.8 s, arriving 0.4 m from the far shore point.
- Filmstrip `out/swim-dive.png` (`run=play` joined to that server, `acts=`, 11 frames 1.2 s apart): floating at the
  surface, Shift dives to the sea floor, a level sprint stroke along it to the slope of the bank, Space rises and
  the head comes out at the surface. Not filmed: a pitched dive (`acts=` has no look); the server test covers it.
- Soak, 50 bots, 2 minutes, seed 1, recorded: `"soak":"ok"`, and its replay 120 checks, `diverged: null`.

### Old recordings
Recordings read unchanged (no format change). A recording whose players never went under water,
never swam with jump at a wall in water and never held the down key while floating replays to the same hashes. One
whose players went under water, or swam at a wall with jump, or held down while floating **replays differently from
the first such step on**: a behaviour change, its replay diverges there.

## Review
A fresh reviewer read `727df052..566ca39b`. The wire, the recordings, speed cheating, creative and the key binding
were found right. Fixed, each with a test seen red first where it could be:
- *The drain charged a sprint that did nothing* (in water, into a wall): `phys::step` now says whether it sprinted
  and the drain goes by that (`a_sprint_into_a_wall_or_in_the_sea_is_no_sprint`, red on the wall; with swimming its sea half became a stroke that sprints;
  `a_sprint_held_in_the_sea_drains_as_a_walk`, red at 0.0667 against a walk's 0.0222).
- *The floor rubber-banded at every crossing*: checked once a tick on the tick's starting hunger by the server
  (a catch-up tick ran up to CREDIT_MAX steps on it), and by the client a round trip late. Now per step on both
  sides, the client predicting the server's exact hunger (option 3 c above);
  `a_sprint_through_the_hunger_floor_needs_no_correction` (40 ticks through 6.0, every `You` equal to the
  prediction in place, hunger and step; red at the crossing tick 15 before).
- *Ctrl held through an alt-tab sprinted on*: losing the focus clears the held keys, as opening a menu does. No
  test: the window event path has none, and the change is the one line.
- *The view widened on any speed over 1.2 walks* (a slide, a refused sprint downhill): it goes by the step's own
  "sprinted".
- *The bot's sprint test timed real seconds on a live server thread*: it now steps a server and the bot in turn
  and compares the ground covered in 60 ticks (not seen red: a test made sturdier, not a fix).
