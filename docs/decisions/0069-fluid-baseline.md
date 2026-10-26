# ADR 0069: The fluid baseline (generated, simulated, drawn)

Status: parts A, B1-B2 and C accepted and done (2026-10-26); B3 proposed, the marked taste calls are the developer's

## Context
Fluids have been reworked four times in three days (ADR 0045 ramps and reservoirs, 0049 bodies that pour, 0068 all
water finite). Each pass fixed the last report and opened the next. The developer (2026-10-26): "this has been a big
struggle, I want the best fluid baseline we can have with the Noita-style simulation". Bug reports of 2026-10-26
(exported build, seed 1128002456):
- 15-53-53 "all these triangles in fluids look ugly": the river's surface is a field of wedges wherever it slopes or
  is uneven (screenshot: every cell shows one or two dropped corners).
- 15-53-22 "the speed that powders move feels right but fluids should update 2 or 3 times as quick, lava can stay
  slow".
- 2026-10-25 02-23-24 (still open): lava pours out of a cave wall as scattered cells.

What the game does today:
- **Generated:** rivers, lakes, sea and pools are `water_static` (and other `*_static` liquids), placed settled, a
  river's surface interpolated per voxel so it descends 12.5 cm at a time. There is no `lava_static`: cave lava
  pools are placed as *moving* lava, so they run the moment the window loads them.
- **Simulated:** the v6 kernel at 30 Hz (`Settings.sim_tick_hz`); a liquid cell holds 1..8 eighths; one move per
  cell per tick: fall, pour into the cell below, run down a slope, a pressed full cell slides up to
  `liquid_flow_speed` x 7 cells sideways, a surface cell splits half into empty air or hands one eighth to a
  neighbour that holds two fewer. Still liquid wakes only on a fresh opening and is finite from then on (ADR 0068).
- **Drawn:** godot_voxel's blocky fluid mesher with our patch (`voxel_patch_fluid.py`): per cell, four corner
  levels from the max of the 2x2 same-height neighbours; a corner next to open air drops to the cell's bottom.
  One shader body (`terrain_fluid.gdshaderinc`) with per-liquid looks in `data/liquid_looks.json`.

Why the triangles: the drop rule is right for a single step (a pool's rim meets the lower surface), but a river
slope is a step in every cell, and moving water with mixed eighths is a step between most cells. Each cell then
drops a different subset of its corners, neighbouring cells do not agree on the height of the corner they share,
and every disagreement is a wedge. No tuning of that rule fixes it: the surface has to be one height field.

## Decision (proposed)

### A. Drawn: one height field, not per-cell ramps (done 2026-10-26)
The drop rule in the mesher patch is replaced by a shared **surface height per corner** (`voxel_patch_fluid.py`):
1. A column's *vote* is its highest cell of the fluid within **two cells up or down** of the cell being meshed,
   whatever sits at that height (water over a shelf of ground counts); its height is that cell's `y` + eighths / 8.
   The blocky mesher's `PADDING` goes from 1 to 2 for the reach (every blocky loop reads the constant; the meshing
   buffer grows from 34^3 to 36^3 cells for a 32 block).
2. A corner's height is the **mean of the votes of the up-to-four columns around it**. An open column with no fluid
   in reach votes the bottom of the lowest fluid cell at that corner (so the surface slopes to a rim or a lip the
   same way from every viewer); a solid, plant or foreign-fluid column with none votes nothing (a lake meets its
   bank flat). Every cell sharing the corner computes the same number, so the surface is continuous: a river
   slope is a sheet, a ripple of mixed eighths an undulation, a filled crater flat.
3. The top quad is emitted by the column's top cell only; corner heights may leave the cell's own 0..1 range
   (clamped to -2..3): vertices are only positions.
4. The capped-cell and lone-film rules of ADR 0049 stay. No side face toward a column whose fluid lies one or two
   cells lower (the lower surface rises to meet this one; the face would stand inside the water); a drop of
   three or more cells is out of reach and keeps its side faces: a vertical water face, drawn honestly.
5. The flow state (the shader's scroll direction) comes from the corner heights, so water runs downhill on every
   slope, river or spill, with no change to the shader.
6. Checks: `tools/water_surface_probe.gd` counts **corner seams** (adjacent surface cells, at most two apart in
   height, disagreeing on a shared corner), **wall corners** (a column the two cells see differently: out of
   reach) and a histogram of surface steps, for `rule=field` (this) and `rule=ramps` (ADR 0045).

Why not a one-cell reach (the first try): on a diagonal slope the columns around one corner span three heights,
and the lowest cell could not see the highest column's top; the probe measured seams of exactly half a cell
everywhere the river sloped (1883 in the report's 128 m square), drawn as a lattice of tiles.

**Found on the way, for part C:** the generator leaves terraces *inside* river water: along the channel's edge
line the water stands 3-4 cells (0.5 m, one macro step) above the surface beside it, and where a river used to
have its waterfall it now descends as a staircase of 3-4 cell steps (report seed 1128002456 at voxel 110 1096
-2859, surface steps in a 128 m square: 1247 of one cell, 550 of two, 305 of three, 44 of four or more). Those
are beyond any mesher reach and draw as vertical water faces; the interpolation of ADR 0045 point 2 does not
cover them. Part C gets a point for it: no step of more than two cells between adjacent river surface cells
except at a declared fall.

#### Measured (A)
- Probe, the report's river (seed 1128002456, x 90 z -2870, r 64): ramps rule 2101 corner seams; field rule
  **0 seams**, 1935 wall corners (all the generator's terraces above), 1317 lips, 3882 surface cells, 15 glass
  edges (unchanged: generator).
- Screenshots at the report's spot (`/tp 110 1100 -2859`, looking west): `game/out/fluidA_before/final.png`
  (wedges everywhere the river slopes), `game/out/fluidA_after/final.png` (the gentle slope is a sheet; the
  old waterfall site is a staircase of water faces). `liquids_test=1`: `game/out/fluidA_liquids/shots/`, every
  pool flat and meeting its stone rim flat.
- Budget runs at a river (`target=river`, vsync off, no other Godot): p99 5.5 and 5.2 ms, but one frame of 53 and
  40 ms in the two runs, so **`budget_ok` is false and unconfirmed**. Both spikes are render-thread stalls
  (render CPU 28-37 ms) on a frame that compiles three new pipelines, which points at a material's first draw
  (CLAUDE.md: 20-30 ms a pipeline), not at meshing; a baseline run with the previous patch's DLL produced no
  summary line, so the comparison is still owed. Committed on the developer's call (2026-10-26), to be checked
  with the next budget run: if the spike is the mesher's, PADDING 2 is the suspect (a 36^3 instead of 34^3 meshing
  buffer per 32 block).

### B. Simulated: per-liquid speed, faster levelling, films that go away (B1, B2 done 2026-10-26)
1. **`liquid_speed` substeps per material** (data, like `liquid_flow_speed`): water 3, oil, blood and sludge 2,
   lava 1 (`materials.gd`). The kernel keeps 30 Hz (powders feel right); after the main pass `tick_once` runs
   `liquid_speed - 1` extra passes over the same active chunks that update only moving liquids whose material
   still has substeps left. A substep spreads and pours (steps 2, 4a, 4b of `update_liquid_fullness`); it never
   falls or runs downhill (gravity stays once a tick, like everything else), and a cell that could fall or that
   stands on a cell that moved this tick waits, so a falling column or stream is not scattered sideways. Lifetime,
   burning, reactions, still-liquid waking and powders run in the main pass only.
   The "moved this pass" marker is now `mark_`, distinct from the tick stamp `stamp_`: the stamp still counts
   ticks (fresh openings, edits, cascades read "this or the last two ticks"), a substep marks what it moved with
   a value of its own (top bit set, unique per tick and substep), and `moved_` is cleared every 8192 ticks as the
   wrap did. The first try bumped the stamp per substep and sand stopped slumping (openings went stale in one
   tick): `test_dirt_wall_holds_sand_slumps` caught it.
   Why not 60 or 90 Hz for everything: powders would need per-tick probabilities to feel the same, and fire, gas
   and reactions would all pay.
2. **Level to the mean.** A surface cell beside a lower neighbour of its kind hands over **half the difference**
   instead of one eighth: a flood evens out in a few ticks and ends flat. The rest rule (neighbours settle when
   they differ by less than two eighths) stays, so pools come to rest and the sim goes quiet. This also removes
   ADR 0049's "mosaic of specks" cause without any joining rule.
3. **Thin films dry up** (taste call 1). A one-eighth water cell resting on a floor that is not its liquid, with no
   liquid of its kind beside it, evaporates after ~10 s (`film_lifetime_ticks`, data; 0 = never). Lava's film
   becomes a basalt crust instead. Why: today every spill leaves a permanent wet speckle on the floor. Mass is
   lost on purpose: a film is a puddle drying, not the river.
4. Kept, not reopened: finite water, still-until-disturbed, river steps are walls, cascades, density swaps, the
   window's moving water baked as `water` when the player walks on. Flowing rivers (spring to sea, real
   waterfalls) remain Phase 3.
5. Checks: `test/test_liquid_speed.gd` (a 4^3 block of water spreads at least 6 cells wider in two ticks with three
   substeps than with one, same mass; lava spreads far less than water; a falling column lands the same with one
   or three substeps; a heap on a 2 m pool is flat to one eighth and at rest within 60 ticks, same mass);
   `test_sim_v6.gd` and `test_liquid_fullness.gd` unchanged and green (22 tests). Films of a 1 m water and lava
   bomb on a plains hillside (`/blast 1 water|lava`, 0.5 s a frame, old kernel above, new below):
   `game/out/fluidB_{water,lava}_after/compare_before_above_after_below.png`. The water is a thin wide sheet one
   frame earlier than before and runs down the slope sooner; the lava ball slumps the same in both. B3 (films)
   gets its test when decided.

### C. Generated: every liquid placed settled and still (done 2026-10-26)
1. **A still kind for every generated liquid**: `lava_static` (material 121, one of the ids reserved for world
   generation) with lava's slot, density and heat; the magma cave pool (`biomes.json`) and the deep lava sea in
   caves use it, and water quenches it like moving lava. A cave lava pool stays until someone opens it, like a
   lake, instead of running the moment the window loads it (bug report 2026-10-25 02-23-24).
2. **Pools lie flat in hollows**: cave pools already need rock on all four sides (ADR 0051's `enclosed`) and sit
   in the floor two voxels deep, so nothing to change there. **No lone liquid cells**: after a block is filled,
   a still liquid cell with air above, none of its kind on any side and open air beside it is dropped (a one-cell
   puddle on a cave wall's step, a scrap at a bank: it drew as a spike or film and woke the sim for nothing).
3. **River steps are the ground's, not the generator's.** The "terraces" found in A were traced column by column
   (`tools/column_probe.gd`): across the step line the macro ground itself drops 3 units (1.5 m) between adjacent
   0.5 m columns, and the river's surface is clamped to the ground (`surf = min(surf, base_height)`), so the water
   runs down a 3:1 slope as a cascade. A river cannot lie flatter than its bed; that reach *is* the fall. Two
   generator faults that could also make jumps were closed on the way: the interpolation of ADR 0045 point 2 gave
   every wet corner a weight floor of 0.02 (a possible jump at each lattice cell border), and a column took the
   surface of its single nearest stream segment (a jump where the nearest segment changes); now plain bilinear
   weights, and a distance-weighted blend of every segment in reach. The rule is therefore: a step of three or
   more cells between neighbouring surface cells is allowed where the bed under them drops as much (a cascade),
   and the probe counts the others as terraces.
4. Checks: `water_surface_probe.gd` counts every liquid now (`liquid=water` for the old count) and lone surface
   cells; the report's river before and after, a magma cave screenshot, `GEN_VERSION` 22.

#### Measured (C)
- The report's river (seed 1128002456, x 90 z -2870, r 64, every liquid): 3877 surface cells, **0 lone cells**, 0
  seams; steps of three or more between neighbouring surface cells: 340 cascades (the bed drops as much), 20 counted
  as terraces (bed difference 1-2 cells short of the step: gravel and clay lumps in the bed, left as they are).
  The step histogram barely moved with the two interpolation fixes (3-cell steps 305 -> 320, 2-cell 550 -> 541):
  the stairs were the ground's all along.
- A magma cave (x 17858 z 25410, y 350..410, r 48): 2455 lava surface cells, 0 lone cells beside air, 34 one-cell
  puddles boxed in rock (allowed: a pool cell needs rock on all four sides), 0 terraces, and the cave loads with
  `still_woken` 0: the lava stays put.
- Full suite 526 tests green (`test_liquid_looks` and the material tests take the new id). Screenshot of the
  report's river with the new generator: `game/out/fluidC_river/final.png`.

### Order
A, B1, B2 and C are done; B3 (films dry up) waits for taste call 1. Each step: its check, a screenshot or filmstrip, then the developer
plays it.

## Taste calls for the developer
1. Films dry up (B3): yes / no. Default proposed: yes for water, crust for lava.
2. Water speed 3x, oil/blood/sludge 2x, lava 1x (B1): or 2x water.
3. Surface smoothing beyond the four-column mean (A2): none proposed; a wider 3x3 mean is one line if the
   mean still looks faceted close up.

## Alternatives
- Keep per-cell ramps and tune them: every rule set that drops corners per cell leaves seams wherever cells
  disagree; see Context.
- Our own water surface mesh per chunk: full control, but it must follow godot_voxel's streaming and LODs to
  2 km (ADR 0045); the height field inside the mesher gets the same shape at every LOD for free.
- A faster kernel for everything (60 Hz): see B1.
- Pressure (communicating vessels): not Noita, not needed; pipes and pumps are Phase 3 machines.
