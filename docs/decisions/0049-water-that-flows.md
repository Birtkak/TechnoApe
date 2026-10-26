# ADR 0049: Water that flows (big bodies keep their level, small ones drain)

Status: superseded by ADR 0068 (2026-10-26): no bodies are measured, no body pours; all water is finite

## Context
Playtest reports of 2026-10-25 (part A of `docs/plans/2026-10-25-bug-report-split.md`):
- 02-19-55 "noita simulation still not working on fluids. this crater should get filled and part of the river should
  lose elevation, gravity and all that"; 02-54-16 a crater in a river left a dent the river never refilled.
- 02-17-32 a 3x3x3 pocket of river water, walled off and dug free, gave far more water than it held: the reservoir
  rule of ADR 0045 (generated water fills openings, never drains) applied to every scrap of generated water.
- 02-17-49 sand should sink through water and push it up; 02-23-24 lava pours out of a cave wall as scattered cells.

ADR 0045 made generated water a still reservoir: an opening filled with *still* water (`FLAG_FILLED`), cell by cell,
up to 3 m (`RESERVOIR_REACH`), with nothing flowing. A bigger crater than the reach stayed half dry, nothing moved
with gravity, and a cut-off pocket was as endless as the sea.

Asked (2026-10-25), the developer chose: a size check (big bodies endless, small ones finite), rivers drainable,
and on the follow-up "flow, big ones level fast": all disturbed water is real Noita water; big bodies keep their
level quickly; small bodies visibly drain.

## Decision
1. **Still until disturbed.** Generated liquid stays `*_static` and asleep; it wakes only on a fresh opening (changed
   in the last two ticks; stamp 0 = never), as in ADR 0045, so loading or reloading the world never sets it moving.
2. **Measure the body** (`SimWorld::measure_bodies`, driver thread between ticks). A woken still cell asks; a capped
   flood fill through connected liquid of the same fluid slot (still or moving) answers: **big** at
   `BIG_BODY_CELLS` = 32 768 cells (64 m^3 at 12.5 cm), or when it reaches the window's edge or a chunk still loading
   (a river or lake goes on out there); otherwise **small**. Answers are kept 150 ticks (5 s) per asking cell and
   cleared when the window moves.
3. **Small bodies drain.** Every still cell of a small body turns into its moving kind at once (fullness kept) and
   is plain mass-conserving Noita water from then on: the walled-off pocket spills exactly what it held.
4. **Big bodies pour.** A still cell of a big body with an opening (air below it, air beside it, or a partly full
   cell of its moving kind) pours *moving* water into it, one cell a tick: a crater or trench
   fills by water falling and spreading, and the body keeps its level. Each source cell pours at most `POUR_BUDGET`
   = 255 cells until it has nothing left to fill, so a breach into a valley floods it and then stops (the window would
   otherwise pool water against its edge). Moving water that comes to rest on a big body joins it (vanishes into
   it); on a body not measured yet it asks, and a small pond under it drains instead. A full moving cell at rest beside
   a big body's still water at the same height, with no air beside it, becomes still water too (`FLAG_JOINED`) and
   pours like the body: it tops up the partly full cells beside it, they join in turn, and a filled crater ends flat
   at the river's level and part of it. Without this the eighths rule (neighbours even out only when they differ by
   2/8) left a slope of 1/8 a cell, 4 cells over a 30-cell crater, drawn as a mosaic of dark specks (first runs).
   Moving water landing on joined water stays (the crater fills layer by layer), and air over joined water is no
   river step.
5. **River steps are walls, both ways.** Air beside still water over that body's own still cells (each 12.5 cm step
   of a river surface) is no opening for the source, and moving water never flows into air over still water of its
   kind, or every river would pour over its steps for ever.
6. **Sand sinks** through still liquid of lower density: inside the body (still liquid beside it) the still cell
   trades places, so the body stays whole; from above the surface the body keeps its level and the cell is gone.
   Moving liquid already swapped by density.
7. **Streams stay streams.** Gravity no longer waits for the viscosity roll (`liquid_gravity`): a thick liquid falls
   every tick and only spreads slowly. A cell following one that fell this tick (cascade) takes its speed from the
   air it left, so a pouring column does not thin into scattered cells.

`FLAG_FILLED` and `RESERVOIR_REACH` are gone; `SimWorld.BIG_BODY_CELLS` and `POUR_BUDGET` are bound instead.
`get_stats` reports `bodies_measured`, `bodies_drained`, `body_cells_max`; the bubble's JSON `bodies`, `bodies_drained`.

## Measured
- `water_test=1 target=river` (headless): a trench 12 x 7 dug into the bank is 94-98 % full to the river's level
  after 12-20 s (runs vary; the last few cells stay dry, not traced yet; ADR 0045's still fill reached 99-100 %),
  nothing above it, the river's column unchanged, nothing past the trench. Then a 2.5 m crater where the river
  meets its bank: 26 / 49 / 69 / 83 / 93 / 98 % full after 1 / 2 / 3 / 4 / 5 / 6 s, flat at the river's level, the
  river beside it unchanged, the sim quiet afterwards (0 active chunks). Before the joining rule (point 4) the crater
  stalled at 48-56 % with a speckled surface. Filmstrip and shots: `game/out/a_water8/`.
- Kernel tests (`test_sim_v6.gd`): big lake breach, small pond drains into a pocket, big pond refills a pocket, the
  walled-off pocket spills exactly what it held, sand sinks through a lake, lava falls every tick, a breach into a
  valley stops at the budget, river steps stay quiet, water poured on a lake joins it, loaded water stays.

## Also in this pass (part A of the 2026-10-25 bug reports)
- **Cave spikes** (02-02-16, `voxel_patch_fluid.py`): a fluid cell capped by a solid block keeps its corners; a lone
  cell (no fluid beside it on any side) lies flat as a film. The generator should not leave single water cells on a
  cave wall's steps at all (asked of part B); draining them when they load was tried and dropped: the world holds
  thousands (3440 bodies drained in one run, and the loading screen never lifted).
- **Waterfalls** (02-09-43): falling water faces carry foam streaks running down (`terrain_fluid.gdshaderinc`,
  `waterfall_speed`), and every fall's foot plays its own looping roar (`WaterfallSpray._roar`): the sim's
  `waterfall` loop only hears moving water over air, and a generated sheet is still water.
- **Pieces in flight take cuts** (02-51-52, `RigidPieces.queue_carve` / `raycast`): `SimBubble.blast` carves pieces
  too, the laser stops on a piece nearer than the ground; a piece cut through flies on as two (smaller parts drop as
  crumbs). Headless check at the report's spot: `/fell` + `/laser`: 1152 piece cells cut, 4 pieces. Not done: a hand
  dig (the brush, part D's file) does not aim at pieces yet. The laser's piece raycast waits for a running sim tick: a pulse took up
  to 36 ms while a crown flew (a playtest gadget; reading the last collected transforms instead would avoid it).

## Consequences
- Filled water is moving water now, saved as `water`: a reload shows it as it was, at rest.
- A big body never drops its level (the developer's "level fast"): "part of the river loses elevation" shows only as
  the water rushing in, not as a lasting dip. A small pond does drop.
- Cost: a flood fill is at most 32 768 cells (about a millisecond) on the driver thread, once per body per 5 s.

## Alternatives
- Drainable world water everywhere (the first answer): a river in the window would drain downhill and pile up
  against the window's edge, and its half-drained state would be baked into the terrain when the player walks on.
- Generator-tagged bodies (sea, rivers, named lakes): needs part B, and a pocket cut off from a river stays endless.
