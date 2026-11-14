# 0118 – Sprint

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
3. **Who enforces the hunger floor.** The server refuses the bit (survival, hunger at 6 or less) before the step;
   the client predicts the same rule from the whole points the server shows it. `ceil(h) > 6` is exactly `h > 6`,
   so the two only part for the few steps between the server crossing the floor and its `Vitals` arriving (one
   small correction, once, when a sprint runs out).

## Decision (sprint)
| what | number |
|---|---|
| speed | the forward share of the walk times **1.5** (`phys::SPRINT`): 6.45 m/s straight ahead; strafe and back stay 4.3; a diagonal's forward share only |
| where | walking (survival or creative walking), out of the water (wading under 0.3 of the body still sprints), in the air too: a jump keeps the speed |
| not | creative flight (its wheel is the speed), swimming, dead (`vitals::still` clears it), a menu (the keys are not held there) |
| drain | the walk's drain times **3** (`vitals::SPRINT_DRAIN`): 6x standing, a point every 15 s; full to the floor in 3.5 min of sprinting |
| floor | no sprint at hunger **6** of 20 or less (`vitals::SPRINT_FLOOR`) |
| key | "Sprint (hold)", default **Left Ctrl**, in Keys > Moving; rebindable, kept as `key.sprint=` in `settings.txt`, red when shared |
| view | +**8** degrees of field of view, eased linearly in and out over **0.15 s**, while the predicted body moves faster than 1.2 walks over the ground (a refused or blocked sprint does not widen it) |

- `phys::step`: the move's direction (clamped to 1) gains `fwd * max(0, dir . fwd) * 0.5` when `sprint` is set and
  under 0.3 of the body is under water. The fly branch never reads it.
- `phys::sprints(inp)` (the bit, forward, no flight) is what the server's `live` drains at
  `WALK_DRAIN * SPRINT_DRAIN`; `Vitals::drain(rate)` is the old `step` with the rate named.
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
- Bot (`a_sprinting_bot_takes_two_thirds_of_the_walk_s_time`, real time, 24 m there and back over the test
  planet's hills): walked in 5.79 to 5.91 s, sprinted in 4.08 s (0.69 to 0.70 of the walk; seen red at 1.02 with
  the bit not sent).
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
- shortcut: the floor is checked once per server tick on the hunger at its start, not per step: at most three
  steps late, once.
- The bot's sprint test times real seconds (two thirds within 0.08); a loaded PC can make it flaky.

## Review
