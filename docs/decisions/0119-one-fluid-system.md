# ADR 0119: One fluid system: column flow, native pushes, gases with heat and pressure

Status: accepted (2026-11-03); A-D built, column flow and the heat pass on by default from 2026-11-03. Follows ADR 0069 (fluid baseline), 0104 (world water endless and
never simulated), 0113 (machines: the boiler's `pressure`).

## Context
Bug reports 2026-10-28 14-38-16 "liquids need to be completely different again" and 14-39-27 "slow laggy ugly and
unclear, find us the liquid fixes" (seed 1867691517). Both screenshots are a woken river: 371-580 sim chunks awake,
tick 31-36 ms, a surface of per-cell cubes. ADR 0104 closed the wake (rivers never simulate; only what they pour does),
so this ADR measures what is left on main today and redesigns the rest. The developer added (2026-11-02): liquids and
gases are one fluid system; every gas has its own readable colour as data; fluids change state by temperature and
pressure (water boils, steam condenses, a sealed hot space builds pressure, venting drops it), per cell, native,
deterministic, in budget; and a readout under the minimap shows temperature, pressure, material and state of the
cell the player looks at or stands in.

### Measured on main (`6d243e4`, headless, muted, `view_distance_m=24`; the machine shared by eight agents, so the
numbers are upper bounds, back-to-back medians come with the build)
1. **Kernel.** `tools/water_bench.gd` (a 4 x 1 x 4 m pond drains into a 2 m pit): pit 52 % full after 1 s, 85 % after
   4 s, settled after 5.9 s; **tick 20.5 ms avg, 229 ms max** for one small spill. Nineteen 1.5 m water bombs over 40 s
   (`blast=1.5 blast_material=water blast_every=2`): **156 chunks awake, tick 19 ms**, 878 still cells woken.
2. **Pushes.** The same stress pasted **34 842 chunks in 40 s** (870/s) back into the terrain, push queue up to 410, and
   the worst frame spent **187 ms pasting 35 chunks** (5.3 ms a chunk): `World.write_chunk` builds a `VoxelBuffer` in
   GDScript and pastes a whole 16^3 chunk for any changed cell, and every paste remeshes a 32 block with the fluid
   mesher's 44^3 buffer (`PADDING` 6, ADR 0069 A3: 2.2x the plain blocky volume). p99 frame 37 ms, 129 frames over 16.7.
3. **River edit** (`water_test=1 target=river`): tick 126 ms at the end of the run with only 9 chunks awake (the
   machine), p99 67 ms.
4. **Why it is ugly and unclear** (read from the kernel, `update_liquid_fullness`): every liquid cell holds 1..8
   eighths and moves by random local rules (split half into air, hand half the difference to a lower neighbour, send one
   eighth up to seven cells along the surface, slide a pressed cell sideways with momentum). A surface therefore
   undulates for seconds, rests tilted "flat to an eighth", stops as a 45-degree heap in a pit, leaves specks and films
   that dry away (mass vanishes), and communicating vessels never level: there is no head, only neighbours. Each wobble
   re-dirties its chunk, so the push and remesh cost above is the kernel's cost in another coat. Gases are a random
   walk with a lifetime and three fixed atlas tiles; nothing has a temperature or a pressure, so the boiler (ADR 0113)
   keeps its own `pressure` number.

## Decision
One native fluid system in `sim_world.cpp`, in five parts, each behind a flag until its bench and tests pass, so
`test_sim_v6`, `test_sim_0104`, `test_liquid_*` and `test_fluid_levels` keep running against the old kernel until the
switch.

### A. Column flow for moving liquids (replaces the eighth exchange)
A vertical run of the same moving liquid in a column is **one column** with a level: its floor cell, its full cells and
one surface cell with 1..8 eighths. Per tick the kernel visits **surface cells** (and nothing below them unless they
have an opening), in a fixed parity order per chunk (deterministic, no RNG for levelling):
1. **Fall** stays per cell as today (a stream is cells falling; gravity once a tick, `fall`).
2. **Level to neighbours by head.** For each of the four side neighbours that is the same liquid or air over a floor at
   most one cell lower: move `(h_a - h_b) / 2` eighths of height, where h is the surface height above the lower of the
   two floors, at most what the giving column holds. Whole eighths, the remainder alternating by parity so mass is
   exact. A column pair rests at a difference under one eighth; a flat surface needs no further tick.
3. **Head through openings.** A full cell below a surface whose side neighbour is air, or the same liquid with a
   lower *surface* within reach, has head `h_self - h_there`; it moves a cell (or the head's worth of eighths) into
   the opening. This feeds a breach from the body's depth and replaces 4a's momentum slide. Found on the way
   (2026-11-03): head carries only between *adjacent columns*, so a U-tube joined by a one-cell passage does not level
   (the passage cells have no column to carry the head); communicating vessels need a head value per cell propagated
   through full cells, which the 4-byte cell has no room for. Not delivered here; pipes and pumps stay Phase 3.
   Bounded by `liquid_speed` cells a tick per column, so lava stays slow.
4. **Rest and sleep.** A column with no neighbour differing by an eighth or more and no opening marks nothing, so its
   chunk sleeps the next tick. No film rule: a one-eighth sheet on a floor rests where it is (a puddle is a puddle),
   unless it is alone with no liquid beside it, then it still dries after `film_lifetime_ticks` (taste call kept).
5. Powders, reactions, burning, lifetimes, the substep loop and the still-liquid `reservoir` wake are untouched;
   `reservoir` feeds the opening a column instead of a cell when the opening is deeper than one cell.
Bench target (`tools/fluid_bench.gd`, a new one that prints JSON): the water_bench pond settles in **under 1.5 s with a
tick under 4 ms** on a quiet machine; the 19-bomb stress keeps **under 40 chunks awake** after the last bomb and pastes
under a fifth of today's chunks. Tests `test_sim_0119.gd`: a block in a basin ends flat to half a cell and quiet; the same spill lands
the same twice; mass exact; lava spreads slower than water.
**Measured (2026-11-03, `fluid_bench.gd`, three rounds back to back):** basin: old quiet at tick 11, column flow at 15,
both 1.0-1.3 ms a tick; ten bombs over steps and pits: the old kernel never goes quiet (5000+ cells move every tick,
42-48 chunks awake for ever), column flow is quiet 24 ticks after the last bomb at 0-5 moves a tick. The per-tick cost
is the same; the win is that a spill ends, so its chunks stop pasting and remeshing.

### B. Pushes are native and box-sized
`SimWorld::write_dirty(VoxelTool)`-style path: the sim already tracks the changed-cells box per dirty chunk (`dbox_`);
the bridge copies **that box**, not the chunk, into a `VoxelBuffer` built in C++ and pastes it, from GDScript's
`_drain_push` as today (budget, interval and the remesh token bucket stay). Expected: a wobble of a few cells pastes a
few cells. The fluid mesher's reach stays 6 (ADR 0069 A3) in this ADR; if the bench still shows the mesher, a
per-block surface pre-pass replaces the padding (noted as the next step, not done here).

### C. Gases, heat and pressure (one system with the liquids)
1. **Temperature** lives on a coarse grid, one `uint8_t` per 4^3 cells (0.5 m; 2 MB for the window) in units of 2 °C
   from -40: diffuses to its six neighbours every 4th tick and relaxes toward the climate's outside temperature
   (`Survival.outside_c`, set per tick from GDScript) in open air; sources are per material data `heat_c` (lava 1200,
   fire 600, a burning cell 400, magma heart 900) and sinks `cool_c` (water, ice). Cost: 2 M coarse cells / 4 ticks,
   only inside active chunks' coarse cells plus a one-cell halo: well under 1 ms.
2. **Pressure** is per 16^3 chunk: gas cells / open (air or gas) cells, 0..1, written each tick by the chunk update from
   its own counts (deterministic, no flood fill). A sealed hot box fills with steam cells, its ratio rises; venting lets
   the gas random-walk out and it drops. (The player's room pressure for the readout reads the chunk's value; the
   `ClimateProbe` flood fill is not needed for the sim.)
3. **State switching** as material data: `boil_c`, `boils_to`, `condense_c`, `condenses_to`, `freeze_c`, `freezes_to`,
   `melt_c`, `melts_to`, and `boil_rise_c` (how much one unit of pressure raises the boiling point; water 60, so a
   sealed chamber at 0.5 pressure boils at 130 °C). Per cell, in the main pass, from the coarse temperature of its cell:
   a liquid cell at or over its (pressure-shifted) boiling point becomes one cell of its gas; a gas cell under its
   condensing point becomes one cell of its liquid at one eighth (1 : 1 cells, mass in cells exact; the expansion is
   the gas's own spread). Water <-> steam, sludge <-> toxic gas (new material, id 254: heavy, sinks, pools in low
   caves), water <-> ice at `freeze_c` / `melt_c` (ice exists). Lava keeps the film crust rule instead of a `freeze_c`:
   a stream's front enters cold coarse cells before it has heated them and would crust at every step. Steam near lava
   keeps its reaction table entry; the two agree (both make steam). Still liquids never switch (ADR 0104).
   Built (2026-11-03): `heat_` (int16 degrees per 4^3 cells, 4 MB for the window), `pressure_` per chunk recounted
   every 8th awake tick, sources pull their coarse cell an eighth of the way per visit, `relax_heat` every 4th tick on
   the chunks that ran, a loaded chunk starts at the ambient, `set_ambient_c` from `Survival.outside_c` each frame.
4. **Gas looks as data** (built): `data/gas_looks.json` gives every gas `gas_tint` (multiplies its atlas tile's
   colour in `terrain_gas.gdshader`), `gas_alpha` and `rise_speed`; `World` makes one copy of the gas material per
   gas and `build_library` assigns it. No new atlas tiles: toxic gas is the smoke tile tinted green and heavier
   (`gas_downwards_speed` 120). `test_liquid_looks.gd` checks every gas has a look and every key is a uniform.
5. The boiler (ADR 0113) keeps its own `pressure` for now; the hook: its water tank's chunk pressure can feed it later
   (`SimWorld.chunk_pressure(p)`), noted in 0113, not rewritten.
Bench: `fluid_bench.gd` boils a 4 m sealed box of water over lava: the steam count rises, pressure reaches the shifted
point and boiling stops; opening one wall drains the pressure and boiling resumes; tick cost under 2 ms over the base.
Tests in `test_sim_0119.gd`.

### D. Readout under the minimap
`SimWorld.cell_info(local)` returns {material, state (solid / powder / liquid / gas / fire, still / moving, eighths),
temp_c, pressure}; `game/scripts/ui/minimap.gd` (the label that already shows coordinates, biome, season, date) adds one
line: `water (moving, 5/8)  24 C  p 0.12`, for the aimed cell, else the cell the player stands in. One file, one line.

### E. Ids and saves
Materials: toxic gas 254; 255 reserved for a second new gas. `FLUID_LEVEL_BASE` 256 and `MAX_MATERIALS` 256 stay: this
ADR needs one id, so the save format does not change. Raising both (the voxel channel is 16 bits, so `FLUID_LEVEL_BASE`
could move to 1024 and `MAX_MATERIALS` to 1024) is the right move when the next ten ids are asked for; it changes every
saved voxel over 255, so it needs a save version and a remap on load, recorded then.

## Alternatives
- Keep the eighth automaton and tune (every ADR since 0045 did): the cost is the rule's, not the constants'.
- Full shallow-water height field per column across the window: flat surfaces for free, but caves, overhangs and
  streams through air are 3D; column flow keeps cells and gains the same convergence where it matters.
- Per-cell temperature (one byte per cell, +134 MB for the window): the heat field is smooth; the coarse grid is
  physically fine and 64x cheaper to diffuse.
- Pressure per sealed room by flood fill: exact but not local; a chunk ratio is deterministic, O(1) and good enough
  for boiling and the readout.

## Built and switched on (2026-11-03)
A (column flow, `Settings.sim_column_flow`, `column_flow=0` for the old kernel), B (box pastes), C (heat, pressure,
states, gas looks, `states=0` to switch off), D (the minimap line). In-game A/B on one scene (a 2 m lava ball and two
1.5 m water balls, 45 s, headless): old kernel 285 chunks awake, tick 335 ms, 74 698 whole-chunk pastes; new 85
chunks, tick 4-5 ms, box pastes at a third of the cost each. Not delivered: communicating vessels (A.3), gas tiles of
their own (tints instead), a per-block surface pre-pass for the mesher (B, left for a measured need).

## Order and milestones
A (spike behind `set_column_flow`, bench + tests) -> B -> C (heat, pressure, states, looks) -> D -> polish. A report at
each; the switch to column flow as the default is the first working build.
