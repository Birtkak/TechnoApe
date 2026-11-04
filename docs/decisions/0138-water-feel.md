# ADR 0138: Water feel: dug holes fill to the body, films run off, floods rest flat

Status: accepted (2026-11-04; round 2 the same day). Follows ADR 0104 (world water endless), 0119 (column flow), and the "a lake nobody
touches stays asleep" rule of 337f1f5 (budget hunt 2026-11-03).

## Context
Bug report 2026-11-03_23-05-16: "how fluids react and fill spaces etc is still very junky and laggy". The developer
asked for competing tries; this one is about feel (another session owns frame cost). Filmed first:
`fluid_film=1` (scripts/visuals/fluid_film.gd) builds seven scenes on a flattened patch (pit beside a lake, water
dropped into a pit, a stream into a cave, a dam let go, a block released at the top of a terraced slope, a channel
dug from a lake in four steps, water_test's trench in a shelving bank) and films each from a fixed camera (strip +
burst + a text slice of the simulation every strip frame). `tools/fluid_feel.gd` runs the same scenes
(scripts/visuals/fluid_scenes.gd) on the bare kernel and prints how fast and how full each fills, ticks to quiet,
mass, chunks awake, hanging cells, specks and the surface spread.

## What was junky, and why (all simulation; the mesher drew what the kernel held)
| Scene | Seen | Cause |
|-------|------|-------|
| channel | 97 of 512 cells filled, a 1-5 eighth film, then the lake slept for good | 337f1f5: moving water beside a body counts as an opening only while it moves; the film rested under the two-eighth rest step, so the lake stopped feeding |
| pit_lake | stopped three cells under the lake level, a 45-degree wedge against the bank | same rule, plus the surface row (no head) never feeds anything: the pit could end one cell low at best |
| trench (water_test) | stayed dry (old run: 29 % only because dirt slid in and opened a cell under the river) | a shelving bank: only the river's surface row touches the cut, and ADR 0104 never lets the surface row pour sideways |
| slope | a 2-2-1 eighth film lay on every step for good (ribbed stripes), drying as random specks; 10 % of the water lost | a step's lip cell only runs off a ledge onto air; onto the partial cell of the step below nothing moved, and a lone eighth cannot split |
| dam | the flood rested as a staircase of an eighth per seven cells: 8-10 eighths (more than a cell) of tilt across 6 m, a visible ridge | the 4c reach rule looks 7 cells along one row only; it never crossed into the row below |

## Decision (native, `SimWorld::reservoir` and `update_liquid_column`)
1. **Resting water on ground is still an opening.** A still body's own moving water beside it counts while it moves
   (<= 6 eighths) or, resting, while it holds 2..7 eighths (or sits on its own water) and its column stands on
   ground (`grounded`: down through its own moving water to something solid). It is topped up to full and then stops,
   so the rule cannot loop. Water resting on a still body (a river's lower step, the budget hunt's case) stays no
   opening. "Air over its own moving water" also looks down through air (a stream that paused over a half-filled
   trench starts again).
2. **The surface row feeds an opening deeper than the surface**: a fresh dig beside it with air under it, air over its
   own moving water, and the moving top of a filled pit, so a pit or trench ends level with the body. Air over ground
   at the surface's height (the shore) never opens: a lake does not flood its plain.
3. **A ledge onto a partial cell of its liquid pours what fits** (like the partial-below rule), and **a lone eighth on
   bare ground walks a cell toward the nearest drop** within SURFACE_REACH (nearest first, fixed tie order: each step
   shortens the way, nothing walks back and forth).
4. **The 4c rule is a conveyor.** A surface cell with no neighbour two eighths lower looks FLAT_REACH (14) cells along
   the surface, which may step down into the row below, for a column two eighths lower, and passes one eighth to the
   neighbour one eighth lower on the way. The write is one cell, so 1 + 14 < 16 keeps chunks two apart from touching
   one cell (the parity scheme); the old teleport wrote 7 cells away and could only look 7. A flood rests flat to an
   eighth per 14 cells.

Rejected: a longer teleport (breaks the parity rule), probabilistic one-eighth moves (jitter: every tick remeshes),
interpolating the surface between ticks (not where the junk was; see open issues).

## Measured (bare kernel, 300-450 ticks; before -> after)
- channel cells filled 97 -> 512 of 512; pit_lake 1574 -> 2240 of 2240 (level with the lake); trench (shelving bank)
  0 -> 336 of 336.
- slope: surface spread 147 -> 1 eighth, never quiet -> quiet at tick 49, mass lost 931 -> 9 eighths, basin cells
  704 -> 1408.
- dam: spread 9-10 -> 3 eighths, quiet at tick 203, chunks awake at the end 4 -> 0.
- Every scene quiet with 0 chunks awake at the end (`fluid_feel.gd`).
- In game, `water_test=1 target=river`: trench 0.29 -> 0.75 of its box (the rest is dirt that slid in), filled to the
  river's surface row; crater 0.83 never done -> 0.96 in 12 s; nothing above the level, nothing past the trench.
- `fluid_bench.gd` lake_load unchanged (0 chunks awake after 300 ticks, 15 specks woken); basin 0.99-1.09 ->
  1.07-1.11 ms a tick, bombs settle cheaper (3.9-4.1 -> 2.3-2.6 ms after), basin mass now kept exactly (films walk
  off instead of drying). Back to back on the shared machine.
- Tests: `test/test_sim_0138.gd` (5 cases, one per rule; the old kernel fails the channel and flood cases), the 10
  existing fluid test files green.
- Before/after sheets (pit_lake, channel, slope, trench): `fluid_film=1` then `python tools/fluid_compare.py`.

## Round 2 (2026-11-04): a flat flood, a continuous stream
Filmed again after the merge with ADR 0137, with a waterfall scene (a cliff-top lake whose lip is cut, side on) and a
top view of the surface heights at the end of each scene.

5. **A resting flood levels past the conveyor's reach.** The dam case rested as a staircase of 0..5 eighths, a tread per
   14-16 cells (8 cm across 7.5 m, a hump against the wall in game): the conveyor cannot see further and the parity rule
   keeps it there. A cell whose conveyor look found nothing at an edge one eighth down records itself as a seed (256 a
   tick); after the passes `SimWorld::level_surfaces` gathers each seed's connected resting surface (tops of the liquid
   with air over them and something under, neighbours a row up or down at most a cell apart, up to 8192 columns) and,
   if it tilts 2..8 eighths, moves one eighth from each highest column to a lowest one while they differ by two or more.
   More than 8 is still flowing and left to the column rules. Moves are sorted by height and position, so the result
   does not depend on which thread recorded a seed first. Every fluid_feel scene now rests flat to an eighth (dam 3 ->
   1) and goes quiet as soon or sooner (dam tick 203 -> 188); in game the dam's heights are all 0 or 1 (were 0..5).
6. **The shipped voxel build had lost ADR 0103's mesher half.** `~/Tools/godot_voxel` (the 2026-10-29 DLL) was patched
   with voxel_patch_fluid.py before it grew ADR 0103, and the script cannot run twice: no connected column tops, no
   falling boxes, so a waterfall drew as glass shards and slivers pulled toward any water in reach.
   `voxel_patch_fluid_resync.py` puts the fluid mesher file back to stock and applies the script's current part
   (idempotent); built in `~/Tools/godot_voxel-wt/fluid-feel`. A stream is now ADR 0103's cubes.
7. **Falling liquid draws as streaks.** A column gets a falling cell every two or three ticks and each jumps up to 7
   cells, so the cubes are a sparse curtain with gaps that steps at 30 Hz. `fall()` records the cells each falling
   liquid cell crossed (per chunk update, then into one buffer of 8192 a tick); after the tick they are merged per
   column over the last 3 ticks (100 ms) and `get_fall_streaks(slot, offset, width)` returns them as a MultiMesh
   buffer. `FallStreaks` (under SimBubble, main bubble only) draws them as open boxes 0.94 cells wide in the liquid's own
   fluid material, whose side faces run foam lanes down at frame rate, so the stream reads as one curtain of water.
   The instanced pipeline is drawn under the loading screen (`MaterialWarmup.add_nodes`). Frame cost within noise
   (fall/cave scenes, back to back: p50 1.45-1.83 ms either way, p99 +0.1-0.25 ms).

Rejected: interpolating the cubes between ticks (they are the mesher's, remeshed per paste, and a stream's gaps are
real cells missing, not late ones); a wider single-tick streak (one tick covers a third of a column).

## Open
- A stream seen from straight above shows the cubes' tops only (the streaks have no caps: a cap would draw as a glossy
  surface in mid-air). The streaks lead the cubes by the paste and remesh delay (a frame or two).
- A window shift drops the streak history (100 ms without streaks; the cubes stay).
- The fill rate through a 1 m breach is ~350 cells/s (a 1.75 m pit in ~6 s): one pour per still cell per tick.
- `water_test=1 target=river` flaked once with water_above_level 26-84 under a heavily loaded machine, with and without
  the level pass (ok on a quieter run): timing-dependent, not chased.
