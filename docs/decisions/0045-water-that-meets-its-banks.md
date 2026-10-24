# ADR 0045: Water that meets its banks (Lay of the Land look)

Status: accepted (2026-10-24, reservoirs 2026-10-25)

## Context
The developer, after playing (bug report 2026-10-24): "water still is blocky and ugly, not flowing and
a complete piece"; "I want our water to become the same look as Lay of the Land, now you can see voxel
shaped holes on the bank". Asked to choose, they kept the Noita simulation (option 1): the water
model stays, the look changes.

What made the holes, measured with `tools/water_surface_probe.gd` (seed 2116854772, the river of the report,
128 m square):
- **Glass walls.** godot_voxel's `VoxelBlockyFluid` raises every top corner to the highest of the four
  cells around it, so full water next to air stays flat up to a vertical see-through face. Every pool
  edge over a lower bank and every step of a river surface was a wall.
- **0.5 m terraces.** FineGen took each fine column's water level from the nearest macro column
  (0.5 m) while the ground under it is interpolated: rivers came down in 4-voxel steps.
- **Dry trenches.** Hydrology marks a river's whole trench (`river` ~1) but gives only its channel a
  `water_level`; the rest stayed dry, up to 6 voxels below the water beside it. 258 glass edges at
  floor level in that square, most of them 2-6 voxels tall.
- Generated water never settles: it is `water_static`, which the kernel only wakes next to a change
  (ADR 0013), so it stays exactly where the generator put it, gaps included.

## Decision
1. **Ramps, not walls** (`prototypes/p11-lod-horizon/voxel_patch_fluid.py`, our godot_voxel build).
   A top corner drops to the very bottom of its cell when, in the 2x2 cells around it, open air (no
   model or an empty one) sits side by side with the fluid. A step in a surface becomes a ramp that
   meets the lower surface (whose edge rises through the stock covered-neighbour rule); a pool over a
   lower bank slopes to its rim. Air touching only diagonally is ignored (it is hidden, and counting
   it left dimples), and the rule reads the same from every cell sharing the corner, so surfaces stay
   joined. Plants standing in water are not holes. The flow state follows the ramps.
2. **Rivers per voxel** (FineGen). A river's surface is interpolated between the macro corners that
   hold water, so it descends 12.5 cm at a time. Corners without a level are left out.
3. **Water fills the sides** (FineGen). A column beside the water whose ground lies below the
   interpolated level fills up to it as static water over a lake bed (river trenches, low banks):
   generated water is placed already settled, near and at the horizon alike.

4. **Reservoirs** (`SimWorld::reservoir`, decided by the developer 2026-10-25 over drainable water).
   Generated water (rivers, lakes and now the sea: all `water_static`) never drains; it fills what is
   opened beside or under it, and moving water that comes to rest on it joins it. Everything else
   (spills, bombs, what breaks loose) stays mass-conserving Noita water. Why not keep the sim's
   mass-conserving water for world water too, with the bubble's edges open (the first plan): the
   bubble follows the player and writes its state back to the terrain, so a river that drains or
   refills while awake is baked in half-empty when the player walks on, and the woken stretch meets
   frozen water at the bubble's edge.
   - The developer's rule: only liquid that touches air beside or below it is simulated, until only
     its top touches air. A reservoir cell is awake exactly then; it fills the opening instead of
     moving (the cell it left would be the next opening, and the river would drain cell by cell).
   - Filled cells are reservoir water too (`FLAG_FILLED`, `aux` = distance from the generated water):
     still, so a trench or pocket is as quiet as the lake once full. They fill straight down through
     air and sideways over anything but the reservoir's own generated cells, up to
     `RESERVOIR_REACH` = 24 cells (3 m), like Minecraft's 7 blocks: rivers slope, so without a reach an
     upstream stretch floods every valley below it (the first try never let the loading screen lift).
   - Air over a river's own lower stretch (each 12.5 cm step of its surface) is never an opening, or
     every river would spread forever. Generated water only answers a fresh opening (changed this or
     the last two ticks; stamp 0 means never), then stays awake (`FLAG_SHAKEN`) until it is full:
     after a reload, filled water reads as generated and must not creep further.
   - A reservoir that touches something it reacts with stays awake (water on a sulfur cave's sludge
     pool still cleans it).

`GEN_VERSION` 14 (with the round blueberry bushes of the same day; the sea is `water_static` too).

## Measured
- Glass edges in the report's 128 m square: 258 -> 3 (all one voxel, which the ramps cover); water
  surface cells 4456 -> 5298 (the trench filled). Four other stretches of the same river, 192 m
  squares: 0-28 edges among 20-15346 surface cells, mostly 1-2 voxels.
- No holes (air ringed by water) in any probe. The dark "pinwheels" on the water are underwater
  cross-model plants reaching the surface; they predate this.
- FineGen bench: lod 0-7 averages 0.13-0.36 ms a block (ADR 0035: 0.2-0.4 on plains).
- Budget run at a river (`target=river`, vsync off, no other Godot): p99 4.49 ms, max 6.52 ms,
  `budget_ok`.
- Screenshots: `game/out/water_p13d/shots/` (before: the bug report's screenshot).
- Reservoirs, `water_test=1 target=river` (the game finds the bank, digs a 12-cell trench 7 wide into
  it from 0.5 m under the surface): the trench is 99-100 % full to the river's level after 12 s,
  nothing above it, the river's column unchanged, nothing past the trench, 0 sim chunks awake after.
  A 2 m blast into the river: 151 chunk pushes, sim quiet (the first, unlimited rule: 238 310 pushes
  and a loading screen that never lifted). Budget runs at a river: p99 4.5-5.1 ms, max 10-14 ms (one
  run had a single 18 ms render-thread stall while streaming, not the sim). Unit tests: breach,
  pocket, river steps, pouring onto a lake, the reach, a reload (`test_sim_v6.gd`).

## Not done (next)
- A flow animation down the ramps that reads at a distance; waterfalls (`fall_top`) still hang as
  static sheets.
- Filled water stays when its source is cut off (Minecraft removes flowing water then): a dam does not
  dry the trench behind it.

## Alternatives
- Our own water mesher (a heightfield surface per chunk drawn by its own nodes): full control over
  the shape and the look, but it would have to follow godot_voxel's streaming and LODs out to 2 km.
  The patch gets the shape inside the existing meshing, LODs included.
- Simulating every surface cell: too many cells at 12.5 cm, only near the player, and with mass
  conserved it drains rivers. Only cells with an opening beside or under them are awake (point 4).
- Drainable world water with open bubble edges: see point 4 (baked-in levels, seams at the edge).
