# World generation

- **Date:** 2026-10-22
- **Code:** `prototypes/p5-sim-spike/src/world_gen.{h,cpp}` (native, in the
  technosim extension), bridged by `game/scripts/generator.gd`
  (`VoxelGeneratorScript`: one native call per block, one buffer copy).
- **Debug:** `game/tools/gen_probe.gd` prints timings, biome mix, height
  range, cave share and writes a biome map PNG; `WorldGen.column_info(x, z)`
  explains any column.

## Model (Minecraft 1.18 style, voxels are 0.5 m)
| Noise | Period | Drives |
|---|---|---|
| continentalness | ~3 km | base height via spline: deep ocean −80 … plains +4 … mountains +240 voxels over sea level; biome ocean/land split |
| erosion | ~1.2 km | relief scale 1.25 (rugged) … 0.15 (flat) |
| peaks & valleys (ridged) | ~400 m | mountain relief, up to 290 voxels where continentalness is high |
| detail | ~50 m | ±4 voxels |
| temperature, humidity | ~2.5 km | desert / tundra / forest / plains |
| cheese + 2× spaghetti (3D) | ~50 m | caves below the surface, more rooms deeper; lava pools below sea −180 |
| ore (3D) | ~11 m | coal (shallow), iron, copper, tin (deeper) inside stone |

Sea level 256, world height 768, bedrock below y=3. Beaches where land is
within 3 voxels of sea level; snow caps above sea +300; snowy tundra when
cold. Caves and ores only at LOD 0 (they alias at coarser LODs); caves are
not carved under the sea floor. Max height ≈ sea + 606, under the ceiling.

## Measured (seed 1337)
- LOD 0 block 16³ with caves + ores: 375 µs; with trees, lakes, ruins: 425 µs;
  LOD 2 block: 67 → 118 µs (one thread; godot_voxel runs them on its pool).
- 4 km × 4 km sample: ocean 43 %, plains 15 %, forest 13 %, highlands 13 %,
  beach 6 %, tundra 4 %, mountains 4 %, desert 1 %, snowy peaks 0.5 %.
- Height range 191 … 703. Cave air under the spawn area: 4.3 % of volume.
- Fly-through at 96 m full detail, 1 km horizon: 1.0 ms/frame, 218 MB.

## Lessons
- godot_voxel asks generators for **padded, non-cubic regions** (e.g.
  34×33×34 when mesh blocks are 32): never assume a cube.
- Match the buffer's channel depth (8/16/32-bit) when writing bytes.

## Hydrology (2026-10-22, replaces the noise-band rivers)
Water is routed on a coarse grid: cells of 32 voxels (16 m), tiles of
128×128 cells (2 km) computed with a 64-cell (1 km) margin, cached per tile.
1. **Priority-flood with epsilon** fills every depression to its spill
   level; outlets are the window border and all ocean cells.
2. **D8 flow routing** on the filled surface, then **flow accumulation**
   from high to low.
3. Cells with accumulation ≥ 18 become streams; the first stream cell of a
   branch is a **spring** (small pool). Width and depth grow with
   accumulation (half-width 2.5 → 18 voxels, depth 1.6 → 7), so brooks merge
   into collector rivers that reach the sea. Centre lines are jittered by
   noise so they are not grid-aligned; the carve blends over wide banks.
4. **Lakes** are the filled depressions deeper than 3 voxels: natural,
   irregular basins; their surface is the spill level. Caves are suppressed
   under rivers and lakes.
5. Generated river and lake water is `water_static` (Noita's static liquid)
   so the material simulation does not drain slopes; disturbing it makes it
   flow. Sea water stays live `water`.
Limits: the 1 km margin truncates basins larger than that, so a river's width
can jump at a tile border; flow does not animate. Debug: the biome map draws
streams (light blue), lakes (blue) and springs (yellow); `find_nearest`
knows "spring", "river", "lake", "alpine", "snow", "mountain".

## Mountains (2026-10-22)
Altitude bands in voxels above sea, jittered and shifted by temperature:
tree line ≈ 220 (cold: −60, warm: +60), permanent snow line ≈ 330 (cold:
−90). Above the tree line: alpine rock with patches of thin grass and
gravel; trees thin out from 160 up and become stunted. Slope from two extra
samples (only above 120): > 48° is bare rock, 37–48° in the alpine zone is
gravel scree. Lowland detail noise is ±1.2 voxels so plains roll gently and
walk with a 1-voxel step-up; mountains keep ±4.7 of fine roughness plus the
ridged peaks, so they stay hard to traverse without a path.

## Trees (2026-10-22)
Trees sit on a jittered 8-voxel grid: one hash per cell (seed, cell) decides
existence (biome density: forest 55 %, highlands 18 %, tundra 8 %, plains
6 %, mountains 5 %), position, trunk height (8–13 voxels = 4–6.5 m) and
canopy radius (2–3). Any block asks for the trees whose canopy can touch it
(`trees_near`, ±3 voxels), so trees cross block borders deterministically
and need no neighbour data. Stamped at LOD 0–2 so forests stay visible on
the horizon as blobs. Trunk = wood, canopy = leaves (burnable, fire_hp 40:
forest fires work). Cost: +25 µs per LOD 0 block, +28 µs per LOD 2 block.

## Static ground
Natural sand and gravel are generated as `sand_static` / `gravel_static`
(Noita's `liquid_static` ground): without this, beaches on terraces slid
diagonally into the sea under the powder rules (3,400 chunk pushes in a
25 s fly-through). Mining them yields the loose material (`loose` field).

## Lakes (2026-10-22)
Superseded by hydrology: lakes are now filled depressions (see Hydrology).
The earlier circular "pond" generator (jittered grid, rim-sampled level,
berm) is gone.

## Ruins (2026-10-22, "keep structures minimal")
One candidate per 160-voxel cell (12 %), footprint 11–21 voxels, only where
the four footprint corners are within 4 voxels of each other. The ground is
flattened to the ruin base; a 10-voxel-high volume above it is opened;
stone floor with gravel patches; perimeter walls in 4-voxel segments of
random height 0–6 with some segments collapsed and random missing stones
(more missing near the top); corner pillars 6–9 high; rubble stones
scattered just outside. Trees keep off ruins.

## Biome blending (2026-10-22)
Temperature, humidity and the height thresholds are jittered by a 50 m noise
(±0.09, ±6 voxels), so biome borders dither over ~20 m instead of a line.

## Debug tools
- `gen_probe.gd`: timings, biome mix, cave share, biome map PNG with lakes
  (light blue) and ruins (dark) overlaid.
- `settle_probe.gd`: generate a region around a point or the nearest
  lake/ruin, run the material simulation on it, report every cell that moved
  with its neighbours. Generated terrain must be still (it is: 0 moves).
- Autopilot `target=lake|ruin offset= alt= pitch= speed=0` parks the camera
  at a feature; `bubble_verify` in the JSON compares the simulation bubble
  with the generator at the end of a run (must be 0 mismatches in an
  unedited world).

## Lesson: streaming and the simulation
Chunks enter the simulation bubble over several frames. Until a chunk is
loaded it must act as a *wall*, not air: before this rule, lake water next
to a not-yet-loaded chunk flowed "into nothing", the load then overwrote it,
and the lost water was pasted back as holes (3,566 pushes in 12 s parked at
a lake). `SimWorld` now carries a per-chunk loaded flag (`mark_all_loaded`,
set by `set_chunk_bytes`, carried through `shift`). Cost ≈ 5–10 % per tick.

## Next
- Waterfalls where rivers meet cliffs; swamps.
- Biome vegetation (grass tufts, flowers, cacti), sandstone under deserts, clay.
- The Noita-style dungeons (underground stamps from a per-region RNG).
- A per-thread cache for lake/ruin cells (they re-evaluate columns per block).
