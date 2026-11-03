# ADR 0115: Recipe browser depth: machine recipes as data, bookmarks saved with the player

Status: accepted (2026-11-02)

## Context

Phase 3 lists "recipe browser depth (machine recipes, bookmarks)". The browser (ADR 0008, 0093) already
shows every recipe type the data has, each under a station tab (crafting, knapping, clay forming, pit kiln,
firepit, casting, smithing, bloomery, forge, blast furnace, sieve, pan, ...) with inputs, output, time or work
note and the station line; U shows uses, a station's uses list its whole tab, and item and recipe bookmarks
existed in `user://bookmarks.json`. Missing: a machine whose work is in code (the ore crusher, ADR 0099) had
no recipe at all, a new machine needed a new recipe type in `RecipeRegistry` and the browser, and the
bookmarks were one global list instead of the player's. Machines and energy (ADR 0113) are designed in
parallel, so the browser must not depend on machine code.

## Decision

1. **`process` recipes.** A new recipe type for any machine: `station` (an item id, required), `ingredients`
   (1..9 specs, ids or #tags), `output`, optional `seconds` and `info` (e.g. "steam from a boiler"). The browser
   puts it under the tab `station:<station>` with the station's icon, lays the ingredients in a row and shows
   "<seconds> s, <info>". A machine plugs into the browser by adding recipe data; its own code decides how and
   whether it reads them. The grid ignores them.
2. **The ore crusher's three recipes** are `process` data in `recipes.json`. The crusher keeps running on
   each ore's `crushed` field; `test_crusher_recipes_match_the_crusher` keeps the two in step (output, two per
   block, `ore_crusher_crush_s`).
3. **Catalysts:** the steam hammer joins the smithing tab's stations, the steam sieve the sieve's.
4. **Bookmarks are saved with the player** (`bookmarks` in the player save, ADR 0032), so each world has its
   own list. A world saved without them starts from the old `user://bookmarks.json`, which is no longer
   written. Autopilot runs still take `bookmarks=a,b`.

## Consequences

- `test_recipe_browser.gd` covers process recipes (tab, layout, uses, a new machine by data alone, a bad
  station refused), the crusher sync, and bookmarks through the player save and the old file.
- `info_test=1` adds the shot `info_machine` (crushed iron ore's crusher recipe) to `info.png`.
- Not built: several outputs or byproducts per recipe, energy per recipe (ADR 0113 decides what a machine
  needs), Fill for machines.

## Addendum (2026-11-02): the book through the electrical age, every station in the browser

Bug report 2026-10-28_08-29-44 ("progression book is not updated"): the book stopped at the Iron Age.
- The Steam Age chapter gains optional quests for what ADR 0097-0100 added without one: steam fitter's kit,
  steam sieve, water tank, steel boiler, miner's helmet, traveler's backpack, surveyor's stake, steam drill,
  boiler suit, jetpack (with the compressor), belts and chutes. The Ore Train stays the chapter's final.
- The Electrical Age chapter is the machines lane's (ADR 0117: rubber to the first machine, its final), with
  the second half of `docs/plans/electric-age-quests.md` as side quests after it: macerator, alloy smelter,
  assembler, battery box, electric lamp.
- Content lane (ADR 0112, 0121, `docs/plans/content-quests.md`): many woods and replanting (Old Stone Age),
  porridge and hearty meals (New Stone Age), bronze spear and armour, iron arms, the steel tool set and steel
  armour (Steam Age), all optional. Walkthrough tests check the main path only.
- Browser: U on a station lists recipes worked at it or on it without being used up (`station`, `on`: the
  loom's weaving, any machine's `process` recipes); the electric furnace is a Smelting station, the forge a
  Smithing one, the drying rack the rack's. `test_every_station_and_machine_has_uses` holds every station
  item to it, except the boilers, the turbine, the compressor and the transport pieces (they make steam or
  power or move items in the world; no recipe runs in them).

## Addendum (2026-11-03): real units, hunting, LV machines
- Tooltips and the browser's item facts give an item's place on the 48 V DC circuit (ADR 0123) from its data:
  a load's watts and amps, a cable's rating and milliohms a block, a switch's or fuse's amps, a battery's Wh,
  the dynamo's volts (`InventoryScreen.power_facts`). Process recipe info already reads watts.
- A **Hunting** tab: one browser-only pseudo-recipe per (animal, drop) from mobs.json, beside Mining
  (`RecipeBrowser.set_hunting`, `PSEUDO`), so fur, feathers, antlers and hides say which animal and biomes.
- Electrical Age: optional quests for the six ADR 0131 machines (extractor, cutter, compressor, electrolyzer,
  extruder, canning) and circuitry (knife switch, fuse box, breaker); the multimeter reads in volts and watts.
- Journal: every mob has its own text (tested), and lookouts and shipwrecks are landmarks found nearby
  (`find_nearest("ruin_lookout" | "ruin_wreck")`).
