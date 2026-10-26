# ADR 0069: The fluid baseline (generated, simulated, drawn)

Status: part A accepted and done (2026-10-26); B and C proposed, the marked taste calls are the developer's

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

### B. Simulated: per-liquid speed, faster levelling, films that go away
1. **`liquid_speed` substeps per material** (data, like `liquid_flow_speed`): water 3, oil and blood 2, sludge 2,
   lava 1. The kernel keeps 30 Hz (powders feel right); after the main pass it runs `liquid_speed - 1` extra
   passes over the active chunks' dirty boxes that update only liquid cells whose material still has substeps
   left. Cost: two more sweeps of liquid cells only (sim tick today 2 ms; measure). The parity scheme holds per
   pass. Why not 60 or 90 Hz for everything: powders would need per-tick probabilities to feel the same, and
   fire, gas and reactions would all pay.
2. **Level to the mean.** A surface cell beside a lower neighbour of its kind hands over **half the difference**
   (at least one eighth) instead of one eighth: a flood evens out in a few ticks and ends flat. The rest rule
   (neighbours settle when they differ by less than two eighths) stays, so pools come to rest and the sim goes
   quiet. This also removes ADR 0049's "mosaic of specks" cause without any joining rule.
3. **Thin films dry up** (taste call 1). A one-eighth water cell resting on a floor that is not its liquid, with no
   liquid of its kind beside it, evaporates after ~10 s (`film_lifetime_ticks`, data; 0 = never). Lava's film
   becomes a basalt crust instead. Why: today every spill leaves a permanent wet speckle on the floor. Mass is
   lost on purpose: a film is a puddle drying, not the river.
4. Kept, not reopened: finite water, still-until-disturbed, river steps are walls, cascades, density swaps, the
   window's moving water baked as `water` when the player walks on. Flowing rivers (spring to sea, real
   waterfalls) remain Phase 3.
5. Checks: `test_sim_v6.gd` gains substep tests (a water column spreads three times as far per tick as lava's,
   mass conserved by `count_liquid_eighths`), the half-difference level test (a 2 m pour ends within one eighth
   of flat in under 2 s), the film test. Filmstrip (`frame_sheet.gd`) of a 1 m^3 water bomb and a lava bomb on
   flat ground: time to rest and radius, before and after.

### C. Generated: every liquid placed settled and still
1. **A still kind for every generated liquid**: add `lava_static` (and any other pool liquid that lacks one);
   `biomes.json` pools use the still kinds. A cave lava pool then stays until someone opens it, like a lake.
2. **Pools lie flat in hollows** (FineGen): a pool's surface is one level over its whole hollow, filled to the
   rim and never above the lowest rim cell (the to-test item for sulfur and lush caves); no single liquid cells on
   wall steps (a cell with no liquid neighbour and open air beside it is dropped at generation).
3. Rivers keep the per-voxel interpolation, the filled trenches and low banks and no waterfall sheet, but **no step
   of more than two cells between adjacent surface cells** except at a declared fall: today the channel's edge
   line stands 3-4 cells above the water beside it (one macro step) and the old waterfall sites descend as 3-4 cell
   stairs (measured in A). With that, A draws every river slope as a sheet.
4. Check: `water_surface_probe.gd` over a sulfur cave and a lava cave: zero lone cells, zero cells above the
   rim; `/goto lava_cave` screenshot.

### Order
A is done; then B1 and B2 together (one kernel change, one test file), then C (with the river terraces found in
A), then B3 if taste call 1 is yes. Each step: its check, a screenshot or filmstrip, then the developer
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
