# ADR 0138: Water feel: dug holes fill to the body, films run off, floods rest flat

Status: accepted (2026-11-04). Follows ADR 0104 (world water endless), 0119 (column flow), and the "a lake nobody
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

## Open
- Presentation, untouched: falling water draws as whole cubes that step at the 30 Hz tick, so a stream down a shaft
  reads as separate chunks with gaps (a stream accelerates and thins); one frame of jagged vertical faces on a dam
  break's front; meshes update per paste. Interpolating the falling cubes would be the next step.
- A flood still tilts an eighth per 14 cells (4 eighths across 6 m, 2D); a body-wide level would need a per-basin pass.
- The fill rate through a 1 m breach is ~350 cells/s (a 1.75 m pit in ~6 s): one pour per still cell per tick.
