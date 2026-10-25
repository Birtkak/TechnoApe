# ADR 0052: Big caves: halls, ravines, cave mouths and crystal geodes

Status: accepted (2026-10-25, the developer's picks; numbers are routine calls)

## Context
Playtest reports of 2026-10-25 (part B of `docs/plans/2026-10-25-bug-report-split.md`):
- 02-03-27: "all caves are super small and not useable and also there are no cave entrances on the surface".
- 02-28-03: "make caves epic".
- 02-20-46: "sludge generates weird": a sulfur cave's sludge pool stood over its floor as a stepped skin
  with green walls.

The caves of ADR 0033/0035 were cheese rooms and spaghetti tunnels of ~1-2 m, 4 % of the rock, stopping
2.5 m under the ground everywhere (no mouths). They were carved at LOD 0 only (and within 12 m of the
ground at LOD 1), so anything wider than the 24 m full-detail radius would show a hole into nothing at
its edge. Cave pools replaced the top floor voxel of any floor column under a noise patch.

The developer chose (sheets `tools/cave_sheet.gd`): "Big" (halls 40-120 m across, tunnels 2-4 m,
~15-30 mouths per km^2, ~12 % cave), ravines + mouths, and crystal geodes.

## Decision
1. **One cave field in WorldGen** (`cave_value`), FineGen interpolates it per voxel as before:
   - tunnels 1.6x as wide (spaghetti width 0.03, 2-4 m) and cheese rooms a little more often;
   - **halls**: a slow 3D noise (period ~150 m, squashed 2.4x in y so floors are wide and flat), from 15 m
     under the ground down to bedrock; the lava line (90 m under the sea) floods their bottoms;
   - **ravines**: along the zero line of a 450 m 2D noise where a 700 m mask opens it, a crack ~10-20 m
     wide at the top (measured: open runs 2 m down p50 10 m, p90 19 m) narrowing to nothing 30-60 m down;
   - **cave mouths**: ramps, one chance in ten per 64 m cell (~24 per km^2), a 1.5-2.5 m tunnel leaving
     the ground and sloping down at 25-40 degrees for 20-45 m, following the local surface; caves may
     break through the ground only there and in ravines (`cave_roof` 0, elsewhere 2.5 m as before).
2. **Halls and ravines are carved up to LOD 2** (`BIG_CAVE_LOD`, ~120 m from the camera), tunnels and
   mouths at LOD 0 (and LOD 1 near the ground). The cavern noise is sampled on a 2 m x 1 m x 2 m grid and
   interpolated, the ravine noise once per column, so coarse blocks stay cheap. The deep-rock shortcut of
   ADR 0048 asks `big_cave_possible` (cavern noise on a 16-voxel grid with a margin, ravines only within
   60 m of the ground) before it calls a block solid.
3. **Crystal geodes**: 1 in 9 cells of 96 m, 25-90 m under the ground and above the lava line: a hollow
   3-6 m across lined with crystal (spikes pointing in) in a basalt shell. New material 120 `crystal`
   (atlas tile 104, a new atlas row 104-111; emissive through the shader's second glow tile), item
   `crystal`, pickaxe, drops itself. Ids 116-119 are placeholders for part A; 121-129 stay reserved.
4. **Cave pools are enclosed** (bug 02-20-46): a pool voxel needs rock on all four sides at its height, so
   its surface never stands over a lower floor; pools are two voxels deep (25 cm) so they read as pools.
5. `find_nearest`: `cavern` (a hall 10 m tall), `ravine`, `geode`; with `sea_stack`, `arch`, `monolith`,
   `delta` of ADR 0051. Debug sheets: `tools/cave_sheet.gd` (cave share map, mouths, a vertical slice),
   `tools/landscape_sheet.gd`; variants through `WorldGen.set_variant(1, 0)` (old caves).
6. Wild food (developer, a third of before), now per biome in `biomes.json` `decor`: `berries` (share
   of bushes with blueberries, 0.056 = 1 in 18, was 1 in 6), `mushrooms` (per forest-floor clutter cell,
   0.031, was 3/32; old growth and swamp 0.047), `log_mushrooms` (per mossy log voxel, 0.012, was 1/29;
   0.017). The blueberry drop stays 1-3.

`GEN_VERSION` 16 (with ADR 0051).

## Measured (seed 1866545922 unless noted)
- `tools/cave_sheet.gd` 2 km squares: cave share of the column under the ground 6-14 % (was 3-4 %); 16-25
  mouths per km^2 (mouth ramps and ravine openings).
- Generation (`tools/fine_gen_bench.gd`, seed 1337, one thread, two interleaved runs each, ms per block;
  other sessions' Godot processes were running, so +-20 %): surface blocks lod 0 0.19 -> 0.25, lod 1
  0.28 -> 0.47, lod 2 0.39 -> 0.63; deep blocks (`deep=1`) lod 1 0.19 -> 0.24, lod 2 0.28 -> 0.53; mountain
  deep lod 2 0.64 -> 1.07. The forest line moved (another place after the landform change), so its
  numbers do not compare.
- Budget runs (vsync off, `load_cap_s=30`): forest p50 4.26, p99 4.95, max 5.61 ms, load 15.9 s,
  `budget_ok`. Mountains: see the plan's Part B notes.

## Consequences
- Caves are places now: halls to find, mouths to walk into, ravines as landmarks; more rock is open, so
  digging down meets air more often (the ore lens test now measures ore against rock, not the box).
- Coarse LOD blocks underground cost more; still inside the load budget measured above.
- Fjords and oxbow lakes, lava tubes and underground lakes stay open (the developer did not pick the last
  two for now).
