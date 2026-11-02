# World generation

> **2026-10-23:** the world is drawn at 12.5 cm by `FineGen` (ADR 0033) over the 0.5 m macro layer described here; landforms and forests changed in ADR 0035. Voxel counts below are macro voxels (0.5 m); the 0.5 m block generator and its probes were removed (ADR 0036).

- **Date:** 2026-10-22
- **Code:** `prototypes/p5-sim-spike/src/world_gen.{h,cpp}` (native, in the
  technosim extension), bridged by `game/scripts/generator.gd`
  (`VoxelGeneratorScript`: one native call per block, one buffer copy).
- **Debug:** `game/tools/terrain_map.gd` draws the land (hillshade, landform mix); `fine_gen_bench.gd` times blocks; `WorldGen.column_info(x, z)`
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

2026-10-26 (ADR 0062): seven kinds from `game/data/ruins.json` (house as above, watchtower, stone
circle, crypt, mud house, shrine, wreck), picked by the biome at the centre; shapes in
`prototypes/p5-sim-spike/src/ruins.cpp`, data header `ruin_table.gen.h` (`tools/ruin_codegen.gd`).
Level ground only (centre and eight points a cell outside within 1.5 m); the footprint is flattened
and the ground ramps to the land over 4 cells round it. 1-4 pots per ruin, each kind its own loot
table. `find_nearest("ruin_<kind>")`, `find_nearest("crypt_room")`, `ruin_at(x, z)`.
Footprints are at most 7 m from the centre (`RUIN_MAX_HALF`); stone circles 5-7 m with stones up to 6 m (ADR 0086).

## Biome blending (2026-10-22)
Temperature, humidity and the height thresholds are jittered by a 50 m noise
(±0.09, ±6 voxels), so biome borders dither over ~20 m instead of a line.

## Debug tools
- `terrain_map.gd`: hillshaded land map with the landform mix; `fine_gen_bench.gd`: ms per block per
  LOD; `fine_probe.gd`: material mix of one block; `face_probe.gd`: exposed faces per material;
  `seam_probe.gd`, `shore_probe.gd`, `water_surface_probe.gd`: rivers across tile borders, lake
  waterlines, water holes and glass edges (all in `game/tools/`, run headless; `gen_probe.gd` and
  `settle_probe.gd` went with the 0.5 m block generator, ADR 0036).
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
  steps down. The hanging still-water sheet, spray and roar were removed with
  ADR 0068 (`waterfalls_near` lists nothing). Mountain streams become
  stair cascades. `find_nearest("waterfall")` and the map (white) find the drops.
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
  the alpha cutout; the same mesher builds the station, furniture and mob models. They burn
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
- Checks: the retired `tools/detail_probe.gd` generates 192x192 columns around plains,
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
  Since ADR 0064 the ore lies in bunched lumps: Voronoi lumps around points
  5 voxels apart, parted by a seam of stone (each digs as one piece), ore
  or not as a whole by a pocket field (~3 m pockets, richest at the lens
  centre); the layer of a lump follows its height, the pocket field wobbles
  the layer borders. About 30-40 % of the lens is ore. Ore tiles are
  patterned (one picture across a lump).
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
  so does the autopilot `target=copper_vein`. `tools/fine_probe.gd -- vein=1` finds a vein (the retired `vein_probe.gd` listed the
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

## Boat-ready rivers, sloped shelf, deep ocean (2026-10-23, GEN_VERSION 11)
The developer asked for rivers "wide/deep enough for potential future boats" and
"a shoreline with gradually decreasing ocean bottom until you hit deep ocean where it
can get quite deep and dark".
- **Rivers**: `river_half_width` / `river_depth` in `world_gen.cpp` (shared with
  `waterfalls_near`'s sheet width), from strength sqrt(acc / STREAM_ACC):
  half width 2 + 2.5 s (3.5 .. 28 voxels), depth 2 + s (3 .. 12 voxels). The channel
  is **flat-bottomed**: full depth over the inner 45 % of the half width, then rising
  to the bank (it was a V, too shallow for a hull off the centre line).
- **Sea floor**: the continentalness spline now has a shelf, a slope and an abyss.
  Beach -1.5 voxels (c -0.05), shelf -7 (c -0.15), shelf edge -18 (c -0.3),
  continental slope to -55 (c -0.45) and -120 (c -0.6), abyssal plain -160 (c -0.75)
  down to -200 voxels (100 m) at c = -1. Deep ocean is still c < -0.55.
- **Dark water**: `Atmosphere` reads how much water is above the camera (up to
  24 voxels; deeper in the sea by sea level). Light falls to 1/e every
  `water_light_m` = 18 m: the sun's energy, the ambient, the fog colour and the
  screen tint go dark with depth, and also at night. The fog thickens up to 1.6x.
- Measured with `tools/water_probe.gd` (seed 1337):
  - Rivers (338 river-centre samples over 8 km): 4 .. 40.5 m wide, p10 5.5 m,
    median 6.5 m. 0.5 .. 6.5 m deep, p10 1.5 m, median 2 m. One sample was under
    1 m deep. Before: 2.5 .. 36 voxels wide and 1.6 .. 7 voxels deep (V-shaped).
  - Ocean (16 km square, 6820 sea columns): 16 % deeper than 40 m, deepest 91 m.
    Before, the floor stopped at 40 m.
  - Shore transect from the nearest beach: -0.5 m at 32 m out, -6.5 m at 64 m,
    -11 m at 96 m, -22 m at 128 m.
- Frame cost: a 20 m/s flight over the deep ocean costs less than over land
  (p50 2.0 vs 6.8 ms, p99 8.2 vs 12.2 ms). Other Godot processes were running in
  both runs, so neither is a budget check.

## Massifs, big caves and landforms (2026-10-25, GEN_VERSION 16, ADR 0051-0052)
The developer's playtest asked for more massive mountains ("a huge foot that becomes more and more
steep"), "epic" caves with entrances, and real-world landforms; sheets showed the old ranges already hit
the 384 m ceiling.
- **640 m world**, sea still at 128 m. **Massif** mountains: a widened 3 km mask, height
  `400 m x (mask^2 x (0.45 + 0.55 ridge^1.3))^1.3`: a long gentle apron, steepening to ~400 m peaks; heights
  over 200 m bend softly toward the top. Old ranges and "horns" stay as sheet variants (`set_variant`).
- **Caves**: tunnels 2-4 m, halls 40-120 m (squashed 3D noise, carved up to LOD 2), ravines ~10-20 m wide
  and 30-60 m deep, cave-mouth ramps (~24 per km^2) that leave the ground, crystal geodes (material 120).
  Cave pools need rock on all four sides (no more draped sludge).
- **Landforms**: sea cliffs 7-27 m on rocky stretches of coast, sea stacks and arches in front of them,
  Uluru-like monoliths on open lowland, meandering lowland rivers (domain warp), braided deltas at big
  river mouths. Lakes reach half a cell past their lake cells and get a rim beyond (no glass wall along
  a cell border).
- Wild food densities are per biome in `biomes.json` decor (`berries`, `mushrooms`, `log_mushrooms`).
- Ruin pot spots: `ruin_pots_near(x, z, radius)` (the generated pots' spots, with the ruin kind, ADR 0062).
- `find_nearest`: `cavern`, `ravine`, `geode`, `monolith`, `sea_stack`, `arch`, `delta`.
- Sheets without a window: `tools/landscape_sheet.gd` (map, height-field view, profile; rows of
  variant:world height:relief) and `tools/cave_sheet.gd` (cave share, mouths, vertical slice).
- Far trees: wood widens by 0.2 x step at coarse LODs (was 0.45, the "chunky" trunks), leaf lumps fade.

## Ledges, cliff bands, deeper sea (2026-10-26, GEN_VERSION 20, ADR 0063)
Four bug reports of 2026-10-25: shallow oceans, mountains too easy to walk up, trees floating over steep
slot ravines, and "lego" single-voxel steps everywhere with tiny cave pools.
- **Sea**: the spline now reaches 12 m at c -0.15, 32 m at -0.3, 60 m at -0.45 and 110 m at c -1.
- **Mountains**: arêtes on the ridge crests and cliff bands: the massif relief is cut into tiers 8-18 m tall
  with near-vertical risers (6 % of a tier's span), their edges wobbling with the detail noise. The
  auto step-up climbs 4 voxels; around the report spot 6.7 % of neighbour rises are now more than that.
- **Ledges**: `FineGen::ground_f` rounds the fine ground to levels 3-8 voxels apart (patches of ~150 m),
  keeping 15 % of the slope, with hummocks added after so treads are textured, not pillared. Water, rivers,
  marsh and ruins fade it out through the ground weight.
- **Caves**: the cave field is read per 4-voxel band (`cave_band_y`), so cave floors are flat treads with
  0.5 m ledges; pools come in ~25 m blobs and fill whole treads.
- **Ravines**: 10-40 m deep, both depth and width fading in with the mask, walls flaring toward the rim
  ((1 - depth)^2.2). `WorldGen::ravine_near` keeps trees, stamps, sticks and clutter off the rim.
- Probe: `tools/relief_probe.gd` (rise histograms on land, mountain and cave floor, sea depths, ravine
  cross-sections and trunks over ravines; `bench=1` times surface blocks wherever the ground went).

## Cave storeys, entrances, pillars and ore stringers (2026-10-27, GEN_VERSION 23, ADR 0077)
The developer asked for a cave overhaul with the choices left open. Nothing new in GDScript; all in
`cave_value`, `mouths_near`, `cavern_value` and the FineGen column pass.
- **Storeys**: the cheese y-squash eases from 1.6 to 0.5 past ~100-170 m down (`ysq`), so the deep is tall
  rifts under the halls; tunnels at the top, the lava sea at the bottom.
- **Entrances / connectors** on the 64 m mouth cell (`CaveMouth.kind`): ramps (10 %, now winding ±1.2 r),
  **sinkholes** (6 %: a funnel twice the shaft radius at the rim, 2.5-4.5 m shaft 30-68 m deep), **chimneys**
  (16 %: 1.5-3 m shaft from 15-47 m down, 80-180 m long, leaning, never open to the sky). `cave_roof` opens the
  ground over sinkholes, `ravine_near` keeps trees off their rims. `find_nearest("sinkhole" | "chimney")`.
- **Pillars**: `n_pillar_` (~11 m period); where it peaks a hall keeps a 2-3 m rock column floor to ceiling.
- **Ore stringers** (`STRINGER_*` in fine_gen.cpp): below 20 m, in ~1-voxel wavy sheets (value noise near its
  middle) within 1.5 m of cave air, 30-60 % of the sheet is ore in the column's 96 m cell's type
  (`WorldGen::stringer_type`: the lens's type, else a weighted pick). Cave walls hint at the lens.
- Check: `tools/cave_sheet.gd` (sinkholes show as red blobs on the map; the slice shows shafts and pillars),
  `autopilot ... commands=/fullbright target=sinkhole` / `target=cavern inside=1` (fullbright: flat white
  ambient, no fog; without it cave captures are black). No plant stands over a surface voxel a cave carved.

## Gallery storeys, open cave tops, level sticks (2026-10-27, GEN_VERSION 24, ADR 0080)
- **Galleries** (`cave_value`): a storey every 14 m from 6 to 110 m down; passages on the zero lines of
  `n_gallery_` (offset per storey), ~3 m wide, 2.5 m tall, flat floor, tilted by `n_galoff_` (under 10 degrees).
  The followable layer; noodles, shafts and halls join the storeys.
- **Open tops**: where `cave_roof == 0` and the field is within 3 bands of the ground, the top 2 bands open too
  (no grass slabs over sinkholes and ravines).
- **Sticks** only where both ends lie within 2 voxels of the middle's ground; sticks ~70 %, loose stones ~45 %
  of before.
- Check: `tools/cave_sheet.gd` shows the storeys as flat slots in the slice.

## Next
- Fjords where cold ranges meet the sea, oxbow lakes beside meanders.
- Small ores between veins (single blocks dropping nuggets) and the copper
  prospecting pick (reads `vein_at` within ~16 m): `progression.md` "Ores".
- The Noita-style dungeons (underground stamps from a per-region RNG) - parked.
- Chisel every voxel: ADR 0006 (block-entity `Chisel` behavior, overlay
  meshes from `MicroMesher`, `CHISELED` id reserved); P6 measured the mesher.
- Waterfall sound and a wet-rock darkening around the spray.
- A per-thread cache for lake/ruin cells (they re-evaluate columns per block).
- Glow mushrooms as real light sources (today they only glow themselves).

## Shorelines and sea floor (2026-10-28, GEN_VERSION 26, ADR 0101)

- Near coasts the base height follows an estimated distance to the coastline: beach and berm, foreshore,
  sandbar, shelf; beaches only within 15-26 m of the sea; sea floor sand / mud / gravel by depth and patches
  (`WorldGen::seabed`). Supersedes the flat shelf of GEN_VERSION 11 near the shore.

## Giant flowers, lush jungle, crooked sticks, plants over open tops (2026-11-02, next GEN_VERSION)
Asked for in bug reports 2026-11-02 18-55-21, 18-57-25, 18-54-25 and 18-49-07.
- **Plants over open cave tops**: a block over the ground had no cave field (it was skipped above the
  footprint's macro height), so it never saw the open top the block below carved and kept its grass in mid
  air. The field is now built for blocks up to `VEG_REACH` (2 m) over the highest ground and reaches down to
  open_top's depth under the lowest column whose plants can reach the block; it stops at the block's highest
  ground (nothing is carved above it), which pays for the extra blocks.
- **Giant flowers** (`FineGen::GIANT_FLOWER` stamps): ~55 % of the 8 m cells of the flower meadow, a 3.5-5.5 m
  stalk that leans into a curve, two arching leaves, a head tipped toward the lean: a domed yellow heart and
  6-8 red petals that cup upward, 2.5-3.5 m across. Materials 124 giant_stem (rigid wood: fells like a trunk),
  125 giant_petal and 126 giant_heart (rigid leaf), on the plant stem, poppy and dandelion tiles.
- **Jungle**: waist-high ferns over half the floor and grass between, leaf litter only in patches (the floor
  stays green); leafy bushes 0.45-1 m in nearly every featured 2 m cell; young trees at the full tree density
  (a middle storey); vines of jungle leaves hanging 0.5-2.3 m from one column in ~28 under each clump.
- **Sticks**: the shaft bends once off its middle and thins to a voxel at both ends; the forked twig carries a
  sprig, and three in four sticks have a second, shorter twig on the other side.
- Check: `test_fine_gen_lod.gd` (no plant in mid air at the report's spot; the meadow grows all three
  giant flower parts).

## Cave dressing (2026-11-02 overnight)
- `FineGen::cave_dressing` dresses cave floors and ceilings up to DRESS_MAX (1 m) by cave biome: dripstone cones
  (dripstone floors and ceilings), icicles and ice spikes, crystal and sulfur stalactites, basalt columns and
  glowing magma crust, lush vines with glow berries over tufts and ferns (moss ceiling), toadstools of fungus
  stem and glowing caps with glow mushrooms. Materials 228..233, tiles 192..194 and 204..206 (204..207 glow).
- Check: `tools/cave_dress_sheet.gd` (a slice per cave biome, plus a camera line for a look check);
  `test_fine_gen_lod.gd` (dressed, nothing hangs alone).
