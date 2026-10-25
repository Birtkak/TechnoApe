# ADR 0061: The survival loop: diet, seasons, prospecting, discoveries, HUD trackers

Status: accepted (2026-10-25)

## Context
Part G of the 2026-10-25 split (`docs/plans/2026-10-25-bug-report-split.md`): the developer picked five
items from a quality-of-life and gameplay-loop brainstorm to give the day-to-day loop reasons: eat well,
prepare for winter, hunt for ore, explore, keep track of goals. ADR 0040 left nutrition groups out ("no
nutrition groups"); this ADR adds them, at the developer's request. The developer's direction calls
(2026-10-25):
- **Diet:** a bonus and a mild penalty (not bonus only, not a strong penalty).
- **Winter:** frost kills tender crops; hardy ones survive (not "only stop", not "everything dies").
- **Prospecting pick:** short range, exact (about 12 m, the ore shares and the vein centre).
- **Discovery reward:** the journal entry only (no items, no recipe unlocks).

## Decision
1. **Food groups** (`Nutrition`, `data/nutrition.json`, Vintage Story's model). Five bars of 0..10:
   meat, grain, fruit, vegetable, dairy (eggs count as dairy, so it is reachable before cheese).
   A food's items.json `"nutrition": {group: points}` fills them, else its id's entry in nutrition.json
   `"items"` (other parts' foods: part F's cured meat, flatbread, einkorn, emmer; part E's fish, nuts and
   berries, so their items.json lines stay theirs); a food with neither falls back to the first tag rule (`berry` -> fruit, `mushroom` -> vegetable, `fish` -> meat, ...) worth 0.8 of its hunger
   points, so other parts' new foods count as soon as they carry a tag. Stale food gives half. Every bar
   drains one point per 150 s at rest (a full bar lasts about a game day), with hunger's sprint, swim and
   cold multipliers. **Bonus:** each group at 5 or more is "well fed"; two or more well-fed groups give
   +1 heart each (all five: +5 hearts); one food alone gives nothing. **Penalty:** each group under 0.5
   costs a heart, but max health never drops below 6 hearts. A new player starts every bar at 4 (no
   bonus, no penalty). Survival drains the bars in survival mode, sets `PlayerInventory.max_health` once
   a second and saves the bars and the health (above 20) with the player. The bars show in the inventory
   screen (`NutritionPanel`, under the panel); the hearts row wraps into a second row past ten.
2. **Seasons that matter** (`Seasons`). The calendar already rests crops in winter (ADR 0025). Frost:
   every 10 s each tracked crop's column temperature (the ADR 0040 environment formula, the hour
   included, so a cold night counts) is compared with its crops.json `frost_c` (default 0 C; wheat -1,
   carrots -8: hardy); colder kills it to a `dead_bush` voxel and it drops nothing. A crop in a closed
   room (the ClimateProbe fill: a greenhouse) is spared, probed at most 4 times per check and cached half
   a day per 0.5 m block. Wild food: a drop entry with `"seasons": [...]` only drops then (blueberry bushes
   in summer and autumn). Leaner animals: a drop entry with `"winter_mult"` gives that share in winter,
   never less than one (raw meat 0.5). `Seasons.season` is a static the drop tables read, set by
   SurvivalLoop from the calendar.
3. **Root cellar.** A container in a closed room at least 1.5 m under the surface spoils at the cellar's
   8 C and 0.3 of that again (raw meat: 2 days on the surface at 16 C, about 13 in a root cellar); 3 m and
   deeper without a room stays ADR 0040's 8 C cellar; the storage pot still halves it. The room test is
   the ClimateProbe fill from the air beside the container, cached 30 s. Part F gives its cured foods
   longer `spoil_days` (dried about 4x, smoked 8x, salted 15x raw), which stacks with the cellar; a food
   tagged `preserved` also spoils at 0.25, for cured food that keeps the raw item's spoil_days.
4. **Prospecting pick** (`Prospecting`, `scripts/items/prospecting.gd`). Two tools: the flint pick
   (knapped head, assembled with a stick and binding) only says "something" or "nothing" within 12 m, a
   Flint Age teaser; the copper pick (smithed head) lists every vein within 12 m, nearest first: its share
   of each ore and its centre ("Copper vein 5 m away, centre x y z (14 m down): Copper Ore 27%, ..."),
   puts a waypoint on part D's map (`main.waypoints.add(name, pos, "prospect", colour)`, skipped until it
   exists) and notes the vein in the journal. A tap is a right click on rock (any material whose mining
   entry wants a pickaxe) and wears the pick once. The reading comes from the generator, not the voxels:
   `FineGen.vein_at` for the few vein cells that can reach the tap, ore_veins.json for what a vein holds
   (the thirds and the sporadic share at the grid density), distance from the tap to the lens
   (size x thickness). No native query was needed.
5. **Discovery journal** (`Discoveries`, `DiscoveryJournal`, `data/discoveries.json`). Kinds list their
   entries from other data, so new content appears without code: biomes (biomes.json, underground ones
   left out until a cave-biome query exists), landforms (biomes.json `"landforms"`, part B), landmarks
   (ruin, cave, waterfall, river, lake, spring, marsh), animals (mobs.json species), ores and minerals
   (items tagged `ore`, `loose_ore`, `mineral`), stones (items tagged `stone_type`, part E) and ore veins
   (ore_veins.json). Triggers: the column under the player once a second (biome, landform once column_info
   has it, water features), 6 m and more under the surface out of water (cave), a ruin or waterfall within
   20 m every 5 s, a living animal within 16 m, an item of the entry in the inventory, a copper pick's
   reading. A first find shows a "Discovered!" toast (QuestToast's header) and a chat line, and is saved
   with its date. The book gets a Discoveries entry above its chapters, not a chapter in quests.json
   (part F owns the chapters and that file; agreed with F); its
   page shows each kind with found/total and a card per entry (found: icon, name, line, date; else "???").
   An entry's line is `texts["kind:name"]` in the data, else the source's `"lore"`, else the kind's template.
6. **HUD trackers** (`Trackers`, `TrackerHud`). The recipe view has a Pin button, a quest's detail pane a
   "Track on the HUD" button; up to three trackers (the oldest drops) show at the right edge under part
   D's minimap, each a checklist of ingredients or tasks with have/need counts in the inventory's own
   counts (ADR 0046), ticked as they fill; a finished quest leaves the list. Saved per world.
7. **Glue.** `SurvivalLoop` (one node, set up by Main after Survival) owns all of it, wraps the player's
   `work_use` for the pick's right click, saves `user://worlds/journal_<seed>_g<gen>.json` (discoveries,
   trackers, marked veins) every 30 s and on leaving, and adds `loop` to the stats JSON. Commands:
   `/diet`, `/eat`, `/frost [plant <crop> [dx]]`, `/prospect [flint|copper]`, `/discover`, `/pin`,
   `/track`; debug args `pin=<recipe>`, `track=<quest>`.

## Consequences
- Other parts' foods need `"nutrition"` or a matching tag; `test_survival_loop.gd` fails on a food that
  feeds no group. New stones need the `stone_type` tag, minerals `mineral`, to show in the journal.
- `MiningTable.roll_list` (part D's file) reads `Seasons.season` (two lines); `quest_toast.gd` gained a
  header; Farming is untouched (frost erases from `crops` and writes through its `set_voxel`): if part C
  changes how crops are stored, `Seasons.kill` follows.
- No new materials or atlas tiles (the frozen crop reuses `dead_bush`); ids 240-249 and tiles 200-215
  stay reserved for part G.
- Left for later: underground biomes in the journal (needs a cave-biome query from the generator),
  clothing in the comfort band, cheese and milk (dairy is eggs for now), greenhouses as a built thing.
