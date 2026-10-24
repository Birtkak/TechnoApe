# ADR 0045: Water that meets its banks (Lay of the Land look)

Status: accepted (2026-10-24)

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

`GEN_VERSION` 14 (with the round blueberry bushes of the same day).

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

## Not done (next)
- **Settle in the simulation.** Still water whose side is open could wake once when it enters the
  sim bubble, run into the pocket and go static again, which would clean up whatever the generator
  gets wrong near the player. Rivers cannot do that yet: a sloped surface in a mass-conserving
  kernel drains downhill and piles up at the bubble's edge (the "huge effect" when a river woke on
  2026-10-24). They need open edges: where a river crosses the bubble boundary, upstream refills
  from the generator's level and downstream drains.
- A flow animation down the ramps that reads at a distance; waterfalls (`fall_top`) still hang as
  static sheets.

## Alternatives
- Our own water mesher (a heightfield surface per chunk drawn by its own nodes): full control over
  the shape and the look, but it would have to follow godot_voxel's streaming and LODs out to 2 km.
  The patch gets the shape inside the existing meshing, LODs included.
- Simulating every surface cell (the developer's question): too many cells at 12.5 cm and only near
  the player, and without open edges it drains rivers. The generator places settled water instead.
