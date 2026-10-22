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

## Next
- Trees and structures (needs a placement pass that can write across block
  borders: generate "decorations" from a deterministic per-chunk RNG and
  stamp them when neighbouring blocks exist).
- Rivers (gradient descent on the height field) and lakes above sea level.
- Biome-specific vegetation materials; sandstone under deserts; clay.
- Village/dungeon stamps for the Noita-style dungeons.
