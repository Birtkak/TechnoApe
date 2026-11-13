# 0086 – Beds under water: sand, clay, gravel, stone and dirt by what the bake knows

- **Status:** accepted (built)
- **Date:** 2026-11-13

## Context
The developer: "make river and sea/lake bottoms be clay and sand or stone. whatever would be geologic correct",
then "dirt too but not grass either way". With the water now clear (ADR 0071) its beds show. Whatever
block a column's biome gave its top was drawn under the water. The biome map is coarser than the water, so lakes, rivers and the
sea's edge lay on grass. On the 1536-column test planet, about 18,000 of 19,206 lake columns had grass, most river
columns did, and the sea's edge had grass and dirt. The shelf biome's sand also ran down to 20 m and more.

## Options considered
1. **Bake the beds into the map** (a bed per cell): exact, but a map version, a rebake of every kept bake and a
   map cell is wider than a river.
2. **Choose the bed where the ground is chosen** (chosen): `Body::ground_biome_at`, the one place every reader
   (the mesher's tiles, `generated_under`, physics, the water's gaps) takes a column's blocks from, asks
   whether the column's top lies under the water (`water_at`, the map's lakes and reaches, the sea) and if so
   replaces the biome's top and band by a bed. Nothing new is baked.

## Decision
`Body::bed` (engine `world.rs`). A column is under water when the water's level (`water_at`, the mean sea or the map's
lake or reach) stands over its ground. The bed then depends on:
- **Slope** of the map's cell (`Map::slope`, the bake's own four-neighbour rise over run, which the roughness
  uses too), over `BED_STEEP` = 0.7 (about 35 degrees, sediment's angle of repose): **bare rock**, the
  province's (`ROCKS.at`, so basalt, granite, limestone ... as the ground below), under any water. A lake in
  the alpine biome is rock too (a mountain tarn).
- **Rivers**, by the reach's fall (m a metre) and flow: falling 1 % or more, **gravel**; under 0.2 % or 2 m
  deep or more, **clay** (slow and deep reaches and the flat mouth); a small stream (under 0.05 m2/s a metre of
  width), under 0.5 m deep, over land whose band is dirt, **dirt**; else **sand**.
- **Lakes**: under 0.5 m over land whose band is dirt, **dirt** (a pond's flooded edge); to 1.5 m **sand**;
  deeper **clay**.
- **The sea**: to 8 m **sand** (beach and shelf); deeper **mud** on a band of **clay**. A warm reef keeps its row
  (sand and coral).
- Grass never, nor snow or ice: no rule returns them. The band under a bed is the bed's block (clay under mud),
  the rock below as before. Banks above the waterline keep the biome's ground.

Blocks used: `sand`, `clay`, `mud`, `gravel`, `dirt`, the provinces' rocks. All exist already, so no data row
was added and no texture is new. `gravel` has no painted texture on main: the developer's untracked
`data/textures/gravel.png` in the main checkout, left alone, will draw it once committed. Until then, gravel is
drawn with the code-made pebbles.

Test (red, then green): `world::baked_tests::the_beds_under_water_are_sand_clay_gravel_stone_or_dirt` bakes the
1536 planet (the 512 one has no rivers) and walks every second column of every face. It checks that no
column under sea, lake or river water has grass, snow or ice on top or in its band, that the sea deeper than
12 m is not sand, that a lake shallower than 0.75 m is not clay, and that the sea is never dirt. On main it
failed with lake grass 13,462, meadow grass 2,308, sea grass 1,022, river grass 159, sea sand 211,423 deep,
and more. Now it passes. Its printout, columns by kind and top (seed 5, planet 1536, every 2nd column):
sea mud 1,718,392, sand 119,593, rock 156,384; lake clay 9,391, rock 6,806, sand 1,980, dirt 1,029; river
gravel 336, sand 113, clay 84, rock 18.

## Measured
- `col_ground` over all 2.36 M columns of a face of the 1536 planet (release build, best of 3): main 105 ns a
  column, this branch 172 ns. The first try sampled the slope off the map's spline four times (339 ns); one
  cell's four neighbours do the same job. That is about 0.08 ms more worker time per level-0 tile of about
  1,150 columns, on the workers, never the frame thread. Dry columns pay only `water_at`'s quick refusal.
- The bake is untouched: no time added, the map and its version unchanged. `Map::roughs` takes its slope from
  the same `cell_slope` with the same arithmetic, so a baked map's roughness is unchanged to the bit.
  `made_hash` (caves and ores) and the ore report do not depend on the ground's top blocks, so they are
  unchanged.

## Consequences
- **Kept bakes** (`saves/bakes`) stay valid: no rebake. **Existing saves** keep their edits; every unedited bed
  under water now generates as the new rule says the next time it is loaded (generation is from the map, not
  the save). A block a player already dug or placed stays as it was.
- Sand and gravel fall (ADR 0079). A bed of them over a cave falls when an edit wakes it, as beach sand already did.
- The `river`, `lake` and `coast` views change. They are re-taken on main.
- The bed is chosen per column; a stream narrower than the map's cell takes the cell's slope.

## Calls for the developer to confirm
- The numbers: 35 degrees for bare rock, gravel from a 1 % fall, sand to 8 m in the sea and 1.5 m in lakes, dirt
  under 0.5 m of fresh water on soil.
- Deep sea is mud over clay (the shelf biome's sand only to 8 m); clay alone would be calmer to look at.
- Alpine lakes are bare rock throughout.
