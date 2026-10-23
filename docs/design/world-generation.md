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
| ore veins (GEN_VERSION 9) | 96 m grid | one mixed-ore lens per cell at most, from `ore_veins.json` (see below) |

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
Limits: the 1.5 km margin truncates basins larger than that; seam blending
(see the polish round below) hides the accumulation difference; flow does not
animate. Debug: the biome map draws
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

## Polish round (2026-10-22, GEN_VERSION 5)
- **Lakes** need a depression deeper than 5 voxels (was 3): the puddle lakes
  are gone, the large highland basins stay (seed 1337 map: 5.2 % lake of the
  4 km square, almost all in two basins).
- **Marshes**: hollows 1.5-5 voxels deep in warm humid lowland (< 24 m above
  sea) are wet ground (dirt, clay, grass mix, dense tufts) with static-water
  puddles where the ground dips below the fill level minus one; cold marshes
  and lakes get an ice voxel on top.
- **Waterfalls**: where a stream segment drops >= 10 voxels between cells the
  channel keeps the upstream level to a lip at 55 % of the segment and then
  steps down; a 2-voxel band past the lip is filled with static water from the
  pool up to the upper surface (a hanging sheet). Mountain streams become
  stair cascades. `find_nearest("waterfall")` and the map (white) find them.
- **Tile seams**: margin 1 km -> 1.5 km, and within 12 cells of a tile's core
  border the accumulation is blended with the neighbour tile (50/50 on the
  border). `tools/seam_probe.gd` walks the x = 4096 and z = 4096 seams:
  max height jump 1 voxel, max water-level jump 1, mean 0.03-0.18 (before:
  rivers changed width by several voxels at seams).
- **Materials**: sandstone (16 voxels under desert sand, 8 under beaches, no
  ores in it), clay patches in river, lake and marsh beds, ice. Vegetation at
  LOD 0 only: tall grass (plains 29 %, forest 17 %, highlands 11 %, marsh
  55 %), flowers in patches (20 m noise), cacti 2-4 high and dead bushes in
  deserts. Vegetation voxels are **micro-voxel models** (`materials.gd`
  `micro: tuft|flower|bush|...`): a pattern on the 8x8x8 cell grid (6.25 cm,
  ADR 0006) meshed by the native `MicroMesher` into a `VoxelBlockyModelMesh`
  with only exposed faces, textured from opaque atlas tiles, no collision,
  never culling neighbours. Direction call (2026-10-22) over Minecraft
  cross-quads, which showed hairlines edge-on and atlas-bleed specks through
  the alpha cutout; the same mesher builds chisel overlays later. They burn
  (grass fires) and are solid for the simulation.
- Cost with warmed tiles: LOD 0 block 573 us, LOD 2 block 215 us; a hydrology
  tile (320x320 cells) builds in ~65 ms.

## Biomes from data (2026-10-22, GEN_VERSION 6)
The if-chain is gone: `biome_table.gen.h` (generated from
`game/data/biomes.json`) provides the climate model (latitude: north is
colder, 16 km to saturate; lapse above 70 voxels), 36 biomes with surface
materials and tree densities, and the 3D cave layer. See `biomes.md`.
Seed 1337, 4 km square: plains 10 %, forest 5.6 %, old growth 2.2 %,
shrubland 1.8 %, taiga 1.3 %, jungle 0.8 %, savanna 0.7 %, swamp 0.5 %;
the nearest desert moved from 0.5 km to 6 km south (deserts are a hot-band
biome now). LOD 0 block 595 us.

## Micro-voxel detail (2026-10-22, GEN_VERSION 7)
- Materials 37..43 (`PEBBLES`, `MUSHROOM_BROWN/RED`, `REED`, `LILY_PAD`,
  `STALACTITE`, `STALAGMITE`) are micro models like the vegetation; 44 is the
  reserved `CHISELED` id (ADR 0006). Tufts are now 1-cell (6 cm) blades, 3-7
  cells tall; flowers a 1-cell stem with a 3x3 plus of petals at ~35 cm.
- Surface (`vegetation_at`, per column hash, LOD 0): **boulders** (2x2 columns
  of stone, 1-2 high, from a coarser hash, 0.3 % of highland/mountain/alpine/
  shrubland cells), **pebbles** on bare rock and gravel (5 %) and sparsely on
  grass (plains 0.4 %, highlands 2 %, mountains 3 %), **mushrooms** in forests
  (1-2 %), taiga, jungle and swamp (red caps only in the wet dark biomes),
  **reeds** 1 m tall (two stacked voxels) at marsh puddle edges (the puddle
  noise just under its threshold), **lily pads** on 25 % of puddle water
  voxels where the puddle is not frozen and the ground is really under water.
- Caves (`generate_block`, column cave mask, depth > 6 so soil overhangs stay
  clean): cave floors get stalagmites 2.5 %, pebbles 6 %, mushrooms 2 %
  (fungal cave biome: 12.5 %, half red); ceilings get stalactites 4 %.
  Stalactites/stalagmites have a core collision box (0.5 voxel wide).
- Checks: `tools/detail_probe.gd` generates 192x192 columns around plains,
  forest, marsh and highlands and asserts nothing floats, every stalactite
  has a ceiling, lily pads sit on water, reeds come in pairs (seed 1337: 0 bad
  of ~40 000 detail voxels). `tools/model_sheet.gd` renders every micro model
  to one PNG (`out/models/`). Captures: `out/detail_highlands`,
  `out/detail2_marsh`, `out/detail2_forest`.

## Shores, caves by eye, moving water (2026-10-22, same GEN_VERSION 7)
- `Hydro.shore`: dry ground within ~1 m above a lake surface (3x3 cells) or
  just outside a river channel (0.9-1.8 half widths, within 1 voxel of the
  water). `vegetation_at` puts **reeds** (1 m, two voxels) on 32 % of shore
  columns with grass/dirt/clay/sand tops where the climate is above the ice
  line (`t > -0.42`). `tools/shore_probe.gd` walks the nearest lake's
  waterline and prints the flags.
- `find_nearest("cave")`: a cheese room at least 5 voxels tall with a solid
  floor, 10-120 voxels under dry land; `autopilot ... target=cave inside=1`
  starts the player on that floor with a debug torch (OmniLight on the
  player), so cave detail can be captured (`out/cave1`).
- The terrain material is now `assets/shaders/terrain.gdshader` (nearest
  atlas, vertex AO, specular off): fragments in the water, lava or fire tile
  scroll inside their tile over time (UVs wrap within the tile, derivatives
  from the unshifted UV so mips do not seam). Rivers, lakes, waterfall sheets
  and lava pools move; `tools/anim_probe.gd` renders one quad per tile and
  reports pixels changed in 0.5 s (water 50 %, lava 41 %, fire 64 %, stone 0 %).
  Still missing: waterfall mist particles.

## Spray, glow and moss (2026-10-22, GEN_VERSION 8)
- **Waterfall spray**: `WorldGen.waterfalls_near(x, z, radius)` lists the
  falls around a point (foot voxel half a step past the cliff, sheet top,
  flow direction). `scripts/world/waterfall_spray.gd` (a child of `World`)
  rescans every 2 s around the camera (120 m) and keeps one `GPUParticles3D`
  per fall at its foot: 36 soft white billboards, 1.8 s, rising with the
  flow and sinking back. Presentation only, nothing saved.
- **Glowing fungi** (`GLOW_MUSHROOM`, 45): the mushroom micro model with a
  cyan cap tile (45) that the terrain shader renders emissive (`glow_tile`,
  strength 2.2; lava and fire tiles glow mildly too). Placed on cave floors
  deeper than 40 voxels: 2.5 % of floor cells, 4.5 % in fungal caves.
- **Moss carpets** (`MOSS`, 46): a 1-cell layer with bumps over 33 % of the
  floor cells of lush cave biomes (the ones whose cave top material is grass).
- `find_nearest("deep_cave")`: like `cave` but 50-170 voxels down, for
  captures of the glow (`out/cave_deep`).

## Ore veins and ground pickups (2026-10-23, GEN_VERSION 9)
GregTech-style veins replace the 3D ore noise (developer, ADR 0019; plan in
`progression.md` "Ores"). Data: `game/data/ore_veins.json`, validated by
`OreVeinRegistry` and compiled into `ore_veins.gen.h` by
`tools/ore_vein_codegen.gd` (test fails when the header is stale).
- The world is cut into 192-voxel (96 m) cells; a cell holds a vein with
  chance 0.55. Type by weight (coal 30, copper 25, iron 20, cassiterite 15,
  copper-iron 10), centre y uniform in the type's band, centre jittered so
  the lens never leaves its cell: a column looks up exactly one cell.
- A vein is a flat lens 64 voxels across (coal 80) and 16 thick (coal 20) at
  the centre, its rim wobbling between 60 and 100 % of the radius. 60 % of
  the lens is ore: primary in the top third, between in the middle,
  secondary at the bottom, 12 % of ore voxels the sporadic ore. Between
  veins there is no ore at all. Ores stay lod 0 only.
- Surface indicators: ore-bearing stones (micro `ore_stones` with the ore's
  tile, ids 48-50) on grass, dirt, gravel, stone, sand, snow or clay within
  44 voxels of a vein centre, 1 % of columns when the vein top reaches the
  ground, fading to 0 at 120 voxels deep. Coal has none. Veins under the sea
  get none (vegetation stops at the waterline).
- Ground pickups (Vintage Story start): flint stones (id 51) on gravel (1.6 %
  of columns) and a few per mille of grass in open and rocky biomes, twigs
  (id 52) on forest floors (12-16 per mille), pebbles now also in forests.
  All are hardness 0 in `mining.json` and drop `loose_stone`, `flint`,
  `stick` or `loose_*_ore` items.
- Debug: `WorldGen.vein_at(x, z)`, `column_info()` gains `vein`, `vein_y`,
  `in_vein_column`; `find_nearest("<type>_vein")` returns the nearest vein
  under dry land, so `/locate copper_vein` and `/goto copper_vein` work, and
  so does the autopilot `target=copper_vein`. `tools/vein_probe.gd` lists the
  veins around a point with their depth and indicator count.
- Measured (seed 1337, 8x8 cells around the origin): 38 veins (coal 15,
  copper 12, iron 6, cassiterite 3, copper-iron 2); land veins whose top is
  within 120 voxels of the ground carry 5-36 indicator stones. `gen_bench`
  (64 blocks per lod): lod 0 4.76 -> 4.70 ms per block, other lods within
  noise; the old per-voxel ore noise was dropped, the vein costs one cell
  hash per column. Captures: `out/vein3` (top-down over the copper vein at
  59, 452), `out/forest1` (twigs).

## Sulfur caves, cave pools and ceilings (2026-10-23, GEN_VERSION 10)
Asked for: "toxic sludge which can spawn in sulfur caves (make those)".
- `sulfur_cave` (biome id 36) is a cave biome in `biomes.json`: rare pockets at
  mid depth (60-160 voxels) where the slow 3D cave noise is high, the
  mid-depth twin of crystal caves. Floors and ceilings are sulfur (id 55).
  Its borders follow the derived graph (test_biomes).
- Cave biomes may now set `surface.ceiling` (the stone voxel right above a
  cave cell) and `surface.pool`: a static liquid one voxel deep on cave
  floors where a 2D noise (~11 m blobs) is above 0.35 and no neighbour at
  that height opens onto a drop, so a pool never hangs over an edge. Sulfur
  caves pool toxic sludge (static kind, id 54), lush caves water and magma
  caves lava; the codegen emits `SURFACE_CEILING` and `SURFACE_POOL`.
- The biome codegen now takes material ids from GameMaterials (it used the
  index in the registry's allowed-names list, which only matched while that
  list was in id order). Old ids are unchanged.
- `find_nearest("<cave biome>")` (and `/locate`, `/goto`, autopilot
  `target=`) finds a 5-voxel room whose floor is in that cave biome:
  seed 1337 has a sulfur cave right under spawn at (16, 175, 16), with 2,960
  sulfur and 475 sludge voxels in a 48 x 24 x 48 block around it.
- Frame budget: unchanged by this (walk run p99 2.9-3.0 ms vs 3.1 ms before).
  The 8 s `target=cave inside=1` capture was already over budget at HEAD
  (p99 45 ms), sulfur caves the same.

## Next
- Small ores between veins (single blocks dropping nuggets) and the copper
  prospecting pick (reads `vein_at` within ~16 m): `progression.md` "Ores".
- The Noita-style dungeons (underground stamps from a per-region RNG) - parked.
- Chisel every voxel: ADR 0006 (block-entity `Chisel` behavior, overlay
  meshes from `MicroMesher`, `CHISELED` id reserved); P6 measured the mesher.
- Waterfall sound and a wet-rock darkening around the spray.
- A per-thread cache for lake/ruin cells (they re-evaluate columns per block).
- Glow mushrooms as real light sources (today they only glow themselves).
