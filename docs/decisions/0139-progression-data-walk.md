# ADR 0139: A progression walk over the game data, from empty hands to LV

Status: accepted (2026-11-04). Extends the "everything craftable is reachable" checks of ADR 0028 and 0059.

## Context
The survival road (flint, copper, bronze, iron, steel and steam, LV electrical; capped at LV) has not been played
end to end for a while: 221 to-test items are unticked, and balance reports such as "one mine gave 900 copper"
slipped through. The developer asked for a cheap scripted check from the game data, before any in-game bot.
`RecipeRegistry.reachable_from` answers "can it be made at all" but ignores stations, tool tiers, heat, fuel and
power, and the tests that use it start from hand-written lists of world items (`test_quests.gd` GROUND), which
also hide items nothing in the world gives.

## Options considered
1. **Extend `reachable_from`.** Small, but the tests that use it want "anything at all" reachability; gating
   would change what they check.
2. **A separate walk next to the registry** (`scripts/crafting/progression_walk.gd`) that reads the same data and
   adds the gates, the world sources (mining.json, mobs, crops, loot, containers, StoneAge.WORLD_SOURCES), a
   cost model and a chapter-by-chapter play of the quest book. Reported, not asserted, except for dead ends.
3. **An in-game bot** playing the road. Real, but slow and costly to keep working; later, if the walk is not enough.

## Decision
Option 2.
- **World sources** come from the data, not lists: every natural mining.json material (not `built`, not a placed
  item giving itself back) with its tool class and tier (`min_tier` when `requires`, `drop_tier`), mob drops,
  shearing, milking and eggs, ripe crops (seed and a hoe), ruin loot (used only when nothing else gives an item),
  container fills, and the Stone Age world sources with the item each needs.
- **Gates per step:** the recipe's station; tools kept (`keep`, `tool`, molds, the crucible a melt gives back);
  firepit melts and smelts need fuel and a firestarter, the bloomery, forge and blast furnace `#coal`; smithing a
  hammer, the anvil whose tier takes the metal's `anvil_tier` and the forge for `forge_heat` metal; steam
  stations a boiler, a pipe and water; machines a cable and a dynamo that gives their watts alone (V^2 / 4R).
- **The walk** starts with empty hands and plays each chapter's main road (the final quest and its deps in the
  chapter) in order. Tools and stations are made once and kept; the way to make a thing is the cheapest counting
  the set-up it still lacks, so flint tools come before copper ones and copper ore waits for a copper pickaxe.
- **Time** is play time: blocks mined with the best tool owned (mining.json speeds), whole trips to veins (the
  search walk and a stair down to the vein top, sized from ore_veins.json), pickups, hunting and crafting at
  fixed guesses (the constants at the top of the walk), waits (fires, machines, racks) listed apart.
- **Outliers** at 10x: a quest that takes 10x its chapter's median, a chapter 10x its neighbour, a vein holding
  10x what the chapter needs, two recipes for one thing 10x apart; and machines no single dynamo can run.
- `test/test_progression_walk.gd` fails on dead ends only (a quest or a step on the road nothing makes or gives).
  `tools/progression_report.tscn` writes `docs/plans/progression-walk.md`; its "Suggested fixes" section is
  hand-written and kept across runs. Balance data is never changed by the walk: fixes are listed for the developer.

## Consequences
- The first run (2026-11-04) found three dead ends, all on optional Old/New Stone Age quests: nothing gives
  `animal_fat` (the item note says pigs and bears, mobs.json does not), so the fat lamp, candles and every paint
  (the cave painting quest) are out of reach, and nothing gives `floorstone_flint` (the flint mine quest). The
  test is red until the data gives them a source.
- Numbers are estimates under stated guesses: biomes and travel between them, tool durability, hunger and
  machine throughput are not modelled, and waits are not overlapped with work. Compare runs, not absolutes.
- Quest counts of voxel items are voxels (64 to a block), as the game counts them; the walk follows that.
- Run: `godot_console.exe --headless --path game res://tools/progression_report.tscn` (about 20 s), and the
  test with gdUnit's `-c` (no fail fast) to see all three cases.
