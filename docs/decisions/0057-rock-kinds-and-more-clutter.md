# ADR 0057: Rock kinds in provinces and strata, more clutter, ruin pots

Status: accepted (2026-10-25)

## Context
Part E of the 2026-10-25 split (`docs/plans/2026-10-25-bug-report-split.md`): the developer asked for
more content alongside the bug fixes: stone types, clutter items, and (added later) breakable pots in
ruins with loot. Until now every rock below the soil was one grey `stone` (plus sandstone and the
badlands terracotta near the surface, basalt in a few biomes), and the ground clutter was flowers,
mushrooms and seashells (ADR 0047). Part F (a deeper Flint Age) needs chalk, pyrite and ochre.

Developer calls (2026-10-25): eight rock kinds; laid out as provinces with layers; rock kind biases
which ores show up, never locks them; clutter for food (forage), crafting bits and rock pieces, all
with a use; pyrite and ochre placed by this generator code for part F; the rock sheet approved as drawn.

## Decision
1. **Eight rock kinds.** `stone` (the common grey rock) and `basalt` stay; six are new: granite,
   diorite, limestone, slate, marble, chalk (ids 160-165, palettes on tiles 136-141). Each kind but
   stone gets a cobble (166-172; stone keeps `cobblestone`) and every kind cut-stone bricks (173-180,
   patterned courses). Loose stones of each kind lie on the ground as clutter pieces (181-187; stone
   keeps `pebbles`) and drop `<kind>_stone` items tagged `rock`: the firepit and the first quest take
   any `#rock` now (they took `loose_stone`). Rocks drop their cobble; a furnace smelts a cobble back
   into its rock; four rocks make four bricks at a crafting table. Chalk mines without a copper pick
   (drop tier 0): part F digs flint out of it.
2. **Provinces and strata** (`rock_types.cpp`, data `data/rock_types.json`, header
   `rock_table.gen.h` from `tools/rock_codegen.gd`). The land is cut into ~420 m cells with jittered,
   warped centres; each picks a province by weight (grey hills, granite shield, limestone basin, chalk
   downs, volcanic, metamorphic). A province is a stack of 2-4 layers: the top one reaches up through
   every mountain, the last goes to bedrock, the ones between have a thickness range. The boundaries
   rise with the land (60 % of its height above the sea), tilt by the province's dip and undulate
   gently, so cliffs, mountain flanks and cave walls show tilted bands; province borders are faults.
   Every `STONE` FineGen produces becomes the local rock (one line at the end of the voxel loop), so
   boulders and cliffs follow too (ruin walls stay stone). Deterministic from the seed (gdUnit).
3. **Ore bias.** `ore_bias[vein][rock]` multiplies a vein's nugget density in that host rock (tin
   doubles in granite, coal in limestone and slate, copper in basalt and diorite ...); never zero.
4. **Pyrite and ochre** (ids 200-202, the first three of part F's range, agreed with F): 2x2x2 pyrite
   nodules in slate and chalk; red and yellow ochre as soft patches in the subsoil where the surface
   rock is limestone, chalk or slate.
5. **More clutter** (ADR 0047 pieces, ids 188-197, data-driven chances per biome in
   `rock_types.json`, all scarce): wild strawberries (food, on plant_stem leaves), hazelnuts (food,
   keeps), acorns (roast them, or feed pigs and boar), bones and feathers (the existing items), pine
   cones (kindling fuel), driftwood (fuel, splits into sticks), antlers (part F's antler picks), ice
   shards (snow and frozen shores; decoration for now), cave crystals (clusters on deep cave floors,
   many more in crystal caves; decoration, magic later). Snail shells were already a seashell shape.
   FineGen asks `RockTypes::clutter_pick` when its own rules leave a clutter cell empty; snowy ground
   now gets a roll too (only ice shards live there).
6. **Ruin pots** (198 clay pot, 199 cracked): amphorae, jars and urns of whole voxels in ruins' inside
   corners and now and then by a wall, never side by side. A material with `"shatter": "<table>"`
   breaks at any hit (dig or held right click) with shards and its own sound, and drops a roll of
   `data/loot.json` instead of itself (`RuinLoot`): weighted entries, count ranges, worn old tools,
   rarely an ingot; a deeper table below 12 m for later dungeons. The roll is seeded by the world seed
   and the piece's lowest voxel.
7. **Hooks, not rewrites.** FineGen (part B's) only gains calls marked "ADR 0057"; `make_atlas.py`
   and `make_item_sprites.py` (part C's) gain one line each, the tiles and sprites live in
   `tools/atlas_rocks.py` and `tools/item_sprites_e.py`. The terrain shader's patterned-tile mask grew
   from 4 to 8 ints (tiles 0-255).

## Consequences
- Generation cost per block grows ~5-15 % (`tools/fine_gen_bench.gd`, spawn and forest, within noise
  at several LODs): a strata lattice per block, one material lookup per stone voxel. Deep uniform
  blocks keep their one-voxel fill unless a boundary crosses them.
- Saves: the generator output changed; part B bumps `GEN_VERSION` once for the merged branches.
- `tools/rock_probe.tscn` draws a vertical cut of the strata; `tools/clutter_probe.gd` counts the new
  pieces. Uses beyond the ones above (lime from limestone, polished marble, crystals as magic) are
  open for later ages.
