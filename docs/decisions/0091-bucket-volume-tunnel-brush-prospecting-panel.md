# ADR 0091: Bucket volume, tunnel brush, prospecting panel, brush ghost only where it works

Status: accepted (2026-10-27)

## Context

Bug reports of 2026-10-27:

- 17-53-22: "a bucket should hold about 500 to 1000 voxels, not 10". A bucket took and gave one
  12.5 cm voxel (2 litres). Worse, since the aim ray passes through liquids (bug report 15-50-30)
  an empty bucket never saw the water it was pointed at.
- 17-51-34: "make it so you can dig a perfect player hallway", and "make the prospecting pick
  better with a menu to change distance and search modes so you can plan". The copper pick read
  12 m, a vein cell is 96 m: most taps said "Nothing in the rock within 12 m".
- 18-06-58 (first half): the dig/build ghost should follow the voxel brush, and not show where it
  makes no sense (a brush cube outline sat on the anvil's work piece).

## Decision

1. **A bucket holds 1 m^3 = 512 voxels** (`Settings.bucket_m3`, `Bucket.capacity`). 512 is in the
   asked range, is 8^3 (a 2 x 2 x 2 block cube, a pit one metre each way) and is enough to move a
   pond in a few trips without emptying a lake in one. An empty bucket scoops the connected cells
   of the first liquid the aim ray crosses (`Bucket.scoop_cells`, breadth first from that cell, up
   to the capacity). A filled bucket keeps `fill` in its stack data (missing = full, so kits and
   creative buckets stay full) and pours that many cells into the free cells at the aimed face,
   never above it, lowest first (`Bucket.pour_cells`): a pit fills from the bottom. The cells are
   written as plain liquid through the usual `voxels_placed` path; the liquid simulation is not
   changed. Scoop and pour happen on the first press only (held, the next bucket would pour at
   once). The inventory tooltip shows "Holds n / 512 voxels". A partly full cell counts as a whole
   voxel (World.get_voxel drops the eighths): a known, small leak, noted in code.
2. **Tunnel brush shape** (`Brush` shape `tunnel`, B cycles to it, also in the G panel): on a wall
   face a `Settings.tunnel_width_m` x `tunnel_height_m` (1 m x 2 m, the 0.6 m x 1.8 m body with
   room) hallway whose floor is the row the feet stand in (`Brush.tunnel_floor_y`, set by the
   player while on the ground or flying), centred on the aim across the face on the block grid,
   and size / 2 voxels deep per dig with that slice on the block grid too, so dig after dig lines
   up into one straight passage. Half the size, because a full 1 m slice is 1024 cells and the
   GDScript dig took 15-24 ms on completion; 512 cells is what the 1 m cube already costs (measured
   after: no mining spike over 0.4 ms). On a floor or ceiling it digs a hallway-wide shaft.
   Placing with it places the cube.
3. **Prospecting panel** (`ProspectMenu`, Shift + right click with a pick, `/prospect menu [mode]`):
   the search distance (12 / 24 / 48 / 96 m, capped by the pick's `range_m`: flint 24 m, copper
   96 m, one vein cell), the ore to look for (any, or one ore from `ore_veins.json`), and what a
   tap shows: the nearest vein, every vein in range, or a density map (9 x 9 cells over the square
   around the tap, north up, each the biggest share of the ore among the lenses over it at any
   depth). The flint pick only takes the distance. Settings are saved with the world
   (`Prospecting.to_dict`).
4. **The brush ghost shows only where the brush works** (`Player.brush_ghost_fits`): terrain,
   plants and pickups, liquids. Aimed at a station, work piece, furniture or anything a click
   works on, the outline and shell hide; holding a station item still shows that station's ghost.

## Consequences

- Buckets move real amounts of liquid; pouring one in the open makes a low heap at the face that
  the simulation spreads. Pouring into standing liquid adds nothing (only free cells take it).
- Tunnels are one key away; survival tools still cap the brush size, so a weak pick digs thinner
  slices of the same hallway.
- Prospecting reaches far enough to plan; the map makes the vein layout readable at a glance.
- Tests: `test_bucket.gd`, `test_brush.gd` (tunnel, ghost), `test_survival_loop.gd` (prospecting
  settings and panel).
