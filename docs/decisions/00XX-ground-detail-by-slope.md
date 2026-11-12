# 00XX – Ground detail by slope, biome and height

- **Status:** accepted (routine technical calls; how rough is a taste call open for the developer)
- **Date:** 2026-11-12

## Context
Worldgen plan W2's last open code point: "heights from the map: smooth interpolation plus detail driven by
rock and slope" (`docs/design/worldgen.md`, "From the bake to blocks"). Until now `World::height_at` added
one uniform noise, `DETAIL_M * detail` (1 m, ADR 0043), to the map's spline everywhere: plains as bumpy as
cliffs, and the sea floor at the coast as bumpy as the land. Rock provinces are W4 and do not exist yet.
`height_at` is the hottest function of generation, physics and water, and a save stores only edits, so the
ground under a saved world must not move.

## Options considered
1. **Finite differences per column** (four more spline lookups for the slope). Exact, but it about
   doubles the cost of the hottest function.
2. **A roughness per map cell, derived on load, splined with the heights.** One more multiply-add per
   spline tap reading a second array at the same index; no step at a cell border; the map's bytes and
   hash unchanged.
3. **Store the roughness in the map.** Changes the v2 layout and every v2 file; nothing gained over 2.

## Decision
Option 2.

- **`bake::rough(slope, biome, h)`** is the one function that says how rough a cell's ground is (metres of
  detail). Slope is m/m from the four neighbouring cells (across seams), `h` metres over the sea.
  - by slope: `0.1 + 0.4 s(g / 0.2) + 2.5 s((g - 0.25) / 0.6)` (`s` smoothstep): about 0.15 m on a
    plain, 0.5 m at the 4 km planet's median land slope (0.21), 1.3 m at its 90th percentile (0.47),
    2.8 m at its 99th (0.75);
  - height stands in for hard rock: x1 up to 50 m over the sea, rising to x1.5 at 100 m;
  - the sea floor is calm by the coast: x0.1 from the sea level to 4 m down, back to x1 at 16 m down;
  - biome: beach 0.3, wetland 0.4, desert 0.6, grassland and savanna 0.8, ice and tundra 1.1, alpine 1.3,
    the rest 1;
  - capped at 3 m; a cell with no slope at all (a lake's flat) gets 0, so lake beds stay calm.
- **Where rock plugs in (W4):** `rough` takes the cell's rock as one more argument and its hardness
  replaces the `high` factor (hard rock rougher and craggier, soft rock rounder). `Map::roughs` passes the
  rock of the cell; nothing else changes.
- **`Map::rough`** (serde-skipped) is built in `Map::finish` with the mips and the river lines, for maps of
  version 2 only: 3.5 M cells on the 4 km planet, 14 MB.
- **`Map::height_rough`** is `height` at mip 0 with the roughness on the same Catmull-Rom weights (one
  generic `spline`; `height` itself computes to the same bits as before).
- **Character, in `world::detail(a, n)`:** below 1 m of roughness the detail is the noise scaled
  (rounded swells); from 1 m to 2.5 m it blends into the noise folded at 0 (`0.25 - |n|`, scaled 1.6),
  which gives sharp crests: crags on steep ground. No second noise is evaluated.
- **Far columns** of a v2 map get no detail: a chunk of `level` reads mip `k = level - log2(n / m)`, and
  where `k > 0` (columns wider than a map cell) it is the mip alone, as the design says ("far chunks read
  the mips directly: no detail"; the design's "cells of 8 m and more" means columns wider than a cell).
  On the 4 km and 8 km planets (8 columns a cell) levels 0 to 3 read mip 0 and get the detail.
- **Version 1 maps** (chosen by `version == 1`) take exactly the old line, to the bit, at every level.
- **The river cut** is applied after the detail as before, so a bed's line is the bed to the bit.
- Server, height cache, far chunks, physics and bots all still read `height_at`.

## Consequences
- Measured on the 1 km test planet's own heights (`steep_ground_is_rougher_than_flat`, RMS over the
  spline away from beds): plains 0.034 m, slopes over 0.5 0.608 m (17.7 x), the coast's sea floor
  0.042 m. With the uniform detail all three were 0.32-0.34 m.
- Cost: a level-0 height tile of the 4 km planet (`rivers_cost`, release): 356.1 us before, 353.0 us
  after, the same within the noise (one more multiply-add per spline tap on a second array; far columns
  skip the noise). On load `Map::roughs` walks every cell once (not timed; estimated tens of ms on the
  4 km planet) and keeps 14 MB there, about 56 MB on the 8 km one.
- Near and far chunks differ more where they meet, at the level 3/4 border: up to about 2 m on crags (1 m
  before). The skirts cover cracks; a pop on cliffs is possible and unfilmed.
- For W5: beside a bed on a steep bank, a crag dip can lie lower than the bed's floor. The bed itself is
  exact (the walk test), but water in the beds could spill into such pits.
- A world saved on a **version 2** map before this change (v2 landed the same day, ADR 0056) moves under
  its edits by up to the old and new detail; the map hash is unchanged, so it is not detected. Accepted
  because v2 is a day old; a world started on a `build/` of today should be started again.
- Plains are now nearly flat blocks (the spline's own rise only); if that reads as too flat, the 0.1 base
  and the 0.8 grassland factor are the knobs.

## Review
A fresh reviewer read the diff and the task; nothing serious. Fixed: the v1 test now checks against the old
spline written out in the test (not `Map::height`, which this change routes through a generic `spline`);
`height_at` picks the old line by `version == 1` (with a debug assertion that a v2 map has its roughness)
and drops the detail by the effective mip level (`k.min(mips.len())`). Answered: the far-chunk border, the
bank pits for W5, the load cost and the v2 saves of today are in Consequences; `detail_spot` is marked a
scouting tool. Left: the exact `slope == 0.0` test for lakes (harmless after the spline).
