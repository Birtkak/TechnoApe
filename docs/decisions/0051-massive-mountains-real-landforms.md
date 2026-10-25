# ADR 0051: Massive mountains in a 640 m world, and real-world landforms

Status: accepted (2026-10-25, the developer's picks from sheets; numbers are routine calls)

## Context
Playtest reports of 2026-10-25 (part B of `docs/plans/2026-10-25-bug-report-split.md`):
- 02-05-38: "mountains already look pretty amazing here but i think it can be even more massive, have a
  slower gradual increase around a huge foot off the mountain that becomes more and more steep where the
  peaks are very steep".
- 02-28-03: "I want worldgen to use even more real world features and looks and landmarks".
- 02-11-15: "more generation glitches": a river's water stood as a glass wall over its gravel bank.

Sheets (`tools/landscape_sheet.gd`: map, a height-field view from 1.6 km, a profile) showed that the
ranges of ADR 0035 already reached the world's ceiling: peaks ~245 m above the sea in a 384 m world with
the sea at 128 m. Reshaping the profile inside that ceiling changed little; the mass needed height.

## Decision
1. **The world is 640 m tall** (`Settings.WORLD_HEIGHT_VOXELS` 3072 -> 5120; `WorldGen` 768 -> 1280 macro
   voxels). The sea stays at 128 m. Heights over 200 m above the sea bend softly toward the top (tanh)
   instead of being cut flat.
2. **Massif mountains** (`WorldGen::column_base`, variant 1; the developer chose it over "horns"): one
   broad mask (the 3 km region noise, widened) carries a mountain mass whose height grows concavely
   toward its core, `400 m x (mask^2 x (0.45 + 0.55 ridge^1.3))^1.3`: a wide apron that rises gently over
   kilometres and steepens toward the top; the ridged noise cuts valleys into the upper half instead of
   standing on a plain. Peaks ~400 m above the sea. The old ranges (0) and the sharper horns (2) stay as
   sheet variants (`WorldGen.set_variant(0, n)`).
3. **Landforms** (the developer picked "rocky coasts" and "rivers and lone rocks"):
   - **Sea cliffs**: where a 1.5 km cliff mask is up, the coast ends in a cliff 7-27 m tall along a wobbly
     line; the cliff top slopes back to the normal land over ~1 km.
   - **Sea stacks and arches**: 30 % of the 12 m cells on the shelf in front of a cliff coast hold a stack
     1.5-3.6 m across, 55-100 % of the cliff's height; a quarter of the larger ones have a tunnel through
     them at the waterline (an arch, carved with the caves, so lod 0-2).
   - **Monoliths** (Uluru-like): 1 in 5 cells of 1.5 km on open lowland, 150-350 m long, 40-80 m tall, a
     superellipse footprint with near-vertical sides, a rounded top and rain grooves, bare terracotta.
   - **Meanders**: on flat land (the cell's drop under 1.5 m) the river centre line is domain-warped by a
     ~45 m noise, up to 6 m, blended between cells so the channel never jumps at a cell border.
   - **Deltas**: where a big river (strength 2.5-5) reaches the sea it widens up to three times, half as
     deep, braided around sandbars 0.5 m above the water.
   - Not done: fjords (cold ranges meeting the sea) and oxbow lakes; follow-ups.
4. **Lake rims** (bug 02-11-15). Lake water reaches the 3x3 hydrology cells around a lake cell; on a
   plain lying just below a shallow lake's level the water ended at the edge of that reach, a glass wall
   along a cell border (all 126 edges of the report sat on x = 32 n). Lake water now reaches half a cell
   past its lake cells; beyond that, ground below the lake level becomes a rim one voxel over the water
   (`Hydro::berm`), except where a river carries its own water through it. A river trench carved below
   the level within that half cell joins the lake.
5. `find_nearest` (so `/goto` and `target=`): `monolith`, `sea_stack`, `arch`, `delta` (and the caves of
   ADR 0052). Debug: `FineGen.surface_map` and `cave_map` feed the sheets; `column_info` adds `lake_near`
   and `base_height`.

`GEN_VERSION` 16 (with ADR 0052).

## Measured
- Water probe (`tools/water_surface_probe.gd`, fixed window y0=1110 hgt=72, r=96, seed 1866545922) at the
  report spot and three stretches around it, glass edges before -> after: 185 -> 55 (none over 4 voxels;
  89 of the old ones were 7 voxels), 90 -> 65, 0 -> 6, 273 -> 5.
- See ADR 0052 for generation cost and the budget runs.

## Consequences
- Old worlds do not load (GEN_VERSION 16). Code that assumed 384 m must read `Settings.WORLD_HEIGHT_VOXELS`
  (every part of the 2026-10-25 split was told).
- More of the land is mountain apron: the hill and plains share drops where the widened mask reaches.
- Snow covers more ground: the snow line is unchanged (165 m above the sea) and peaks are higher.
