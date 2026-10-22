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
- LOD 0 block 16³ with caves + ores: 375 µs; LOD 2 block: 67 µs (one thread;
  godot_voxel runs them on its pool).
- 4 km × 4 km sample: ocean 43 %, plains 15 %, forest 13 %, highlands 13 %,
  beach 6 %, tundra 4 %, mountains 4 %, desert 1 %, snowy peaks 0.5 %.
- Height range 191 … 703. Cave air under the spawn area: 4.3 % of volume.
- Fly-through at 96 m full detail, 1 km horizon: 1.0 ms/frame, 218 MB.

## Lessons
- godot_voxel asks generators for **padded, non-cubic regions** (e.g.
  34×33×34 when mesh blocks are 32): never assume a cube.
- Match the buffer's channel depth (8/16/32-bit) when writing bytes.

## Rivers (2026-10-22)
A river noise (~1.1 km period) defines channels where it crosses zero:
mask = 1 − smoothstep(0, width, |n|), width 0.035–0.055 (wider inland).
Inside the mask the height blends toward sea − 3 and the normal water fill
puts the surface at sea level, so rivers connect to the ocean like
Minecraft's. Banks within the mask get sand, the bed gravel. Rivers cut
valleys through hills and canyons through mountains (same carve). About
1.2 % of land area. No lakes above sea level yet.

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

## Next
- Lakes above sea level; waterfalls where rivers meet cliffs.
- Biome vegetation (grass tufts, flowers, cacti), sandstone under deserts, clay.
- Structures: villages, and the Noita-style dungeons (stamps from a
  deterministic per-region RNG; the same cross-border trick as trees).
- Biome blending at borders (currently hard edges).
