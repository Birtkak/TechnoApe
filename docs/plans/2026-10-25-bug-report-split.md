# Bug reports of 2026-10-25 02:01-02:59: four sessions

31 reports came in after the last fix commits (db6d353, 02:13), plus one older report that is still
open (far LOD trees, 2026-10-24 22:15). Everything older was fixed in 55f3027, df9f59f and the ADR
0046-0048 commits. The reports are in `%APPDATA%/Godot/app_userdata/TechnoApe/bug_reports/<folder>/`
(report.md, state.json, screenshot.png, log.txt): read the screenshot of every report you take on.

The split follows the code, so no two sessions edit the same native sources:

| Part | Area | Main files | Builds technosim? |
|------|------|------------|-------------------|
| A | Liquids, simulation, falling pieces | `p5/src/sim_world.*`, `rigid_pieces.*`, `sim_bubble.gd`, `voxel_patch_fluid.py`, fluid shaders | yes |
| B | World generation | `p5/src/world_gen.*`, `fine_gen.*`, `biomes.json`, `GEN_VERSION` | yes |
| C | How things look: crops, items, models, animals, light | `tools/make_item_sprites.py`, `tools/make_atlas.py`, `scripts/farming/`, `scripts/mobs/`, `scripts/blocks/*_entity.gd`, `scripts/visuals/` | no |
| D | Player, inventory, interaction, fire and weather | `scripts/player/`, `scripts/ui/inventory_screen.gd`, `scripts/crafting/inventory.gd`, `scripts/items/`, `scripts/work/`, `scripts/commands/`, new `scripts/world/weather.gd` | no |
| E | New content: more animals, stone types, clutter items (added after A-D started; not from a report) | `data/mobs.json` (new species), new stone and clutter entries in `materials.gd`/`items.json`/`mining.json`, new FineGen content in its own source file with small hooks | yes |
| F | A deeper Flint Age: four Stone Age stages (added later; not from a report) | `data/quests.json`, `recipes.json`, new Flint Age items/materials, new mechanics in new files under `scripts/work/` and `scripts/survival/` | no |
| G | The survival loop: diet, seasons, prospecting, discoveries, HUD trackers (added later; not from a report) | `scripts/survival/`, `scripts/ui/quest_book_view.gd`, `recipe_browser*.gd`, `ui/hud.gd` trackers, new `scripts/items/prospecting.gd` | no |

## Rules for all four

- **Own worktree each.** A and B both rebuild the technosim DLL, and a running Godot locks it; four
  sessions in one folder would keep tripping over that. The DLLs are not in git, so after the worktree
  is made: copy `game/addons/technosim/bin/` and `game/addons/zylann.voxel/bin/` from the main checkout,
  then run `--headless --path game --import` once.
- **Headless runs only** unless the developer says they are free (they may be playing).
- **GPU lock for budget runs.** Budget runs fail when another Godot runs (`other_godot_processes`).
  Before a budget run, create `%TEMP%\technoape-gpu.lock` (`$TEMP/technoape-gpu.lock` in Git Bash)
  holding your part's name and the start time; delete it when the run ends. Before starting *any*
  Godot process (tests, imports, captures), check for the lock: if it exists and is under 10 minutes
  old, do other work and check again later; an older lock is stale, delete it. Keep a budget run under
  5 minutes, and take the lock only for the run itself. Windowed test runs (looks tests, captures)
  count too. Before measuring, also check that no other Godot process is running.
  "Old" means the file's last-write time: while you hold the lock, refresh it every 2 minutes
  (PowerShell: `(Get-Item $env:TEMP\technoape-gpu.lock).LastWriteTime = Get-Date`; Git Bash:
  `touch "$TEMP/technoape-gpu.lock"`), so a long run is never taken for stale. Delete only a lock
  that names your part, or one that is stale; a wait that times out means waiting longer, never
  starting Godot anyway.
- **Own godot-cpp per worktree.** A part that builds native code builds against its own copy of
  godot-cpp (copy `~/Tools/godot-cpp` into the worktree's scratch area and point `GODOT_CPP_PATH` at
  it). The shared copy's library gets rebuilt by each worktree, and the builds race.
- **Reserved numbers**, so branches merge without clashes:
  - ADRs: A 0049-0050, B 0051-0052, C 0053-0054, D 0055-0056, E 0057-0058, F 0059-0060, G 0061-0062.
  - Material ids (definition order, append only): A 116-119, B 120-129, C 130-149, D 150-159,
    E 160-199, F 200-239, G 240-249. A part that appends fewer leaves the gap as reserved
    placeholders, so ids stay put whatever the merge order. Exception: E defines 200-202 (pyrite,
    red_ochre, yellow_ochre) so the generator can place them; F starts at 203 and may extend their
    item definitions.
  - Atlas tiles (atlas is full at 104; add rows): B 104-111, C 112-127, D 128-135, E 136-167,
    F 168-199, G 200-215. Grow the atlas to the rows your tiles need; the merge of `make_atlas.py` is
    by hand.
  - `GEN_VERSION`: only B bumps it.
- **Ask the developer first** about the direction calls listed in your part (working principle 3);
  everything else: sensible default, recorded.
- Before handing back: gdUnit passes, the part's scripted checks pass, a budget run (`autopilot=1`,
  `load_cap_s=30`) shows `budget_ok`, screenshots or filmstrips for anything visual. Add your items to
  `docs/plans/to-test.md` under one heading per part. Commit on your branch; merge order at the end:
  D, C, E, F, G, B, A (the native-heavy ones last, they rebuild the DLL from the merged tree).
- Something in another part's files? Message that session instead of editing it.
- **World height:** part B may raise `WORLD_HEIGHT_VOXELS` (`settings.gd`) for bigger mountains, if the
  developer agrees. That is a change every part may lean on (sim window, climate probe, spawn, map
  relief, saves): if it changes, B tells every running session and notes it under the merge notes.
- **Patterned tiles above 127:** the terrain shader's `pattern_tiles` is `int[4]` (a bit mask), so only
  atlas tiles 0-127 can be `"pattern": true` (`terrain_common.gdshaderinc`, `ti < 128`). Part E widens
  it to `int[8]` (tiles up to 255) as one small hunk and lists it under the merge notes; D and F use
  patterned tiles at 128+ only on top of E's change (until the merge, test them below 128 or plain).
- **Merge notes** (hunks a part made in another part's files, to check when merging):
  - B (branch `claude/part-b-worldgen-6d5cbb`, c3449da and later):
    1. World height: `Settings.WORLD_HEIGHT_VOXELS` 3072 -> 5120 (640 m), sea unchanged at 1024;
       WorldGen/FineGen 768 -> 1280 macro; `ore_vein_registry.WORLD_HEIGHT` 1280.
    2. `GEN_VERSION` 16 (`generator.gd`); it covers E's generator hooks too.
    3. `materials.gd`: `_reserved_defs(116, 119)` placeholders, then 120 crystal after
       `_clutter_defs()`. A replaces the placeholders with its 116-119. Parts C-G (ids 130+) merge
       before B: the merger inserts `_reserved_defs(121, 129)` after 120 so their ids stay put.
    4. Atlas: `make_atlas.py` ROWS 13 -> 14, tile 104 crystal; `materials.gd` ATLAS_TILES 8x14. Merge
       by hand with the other parts' rows (C's 112-127 needs ROWS 16, and so on).
    5. `terrain_common.gdshaderinc`: new uniform `glow_tile_b` (crystal); `world.gd` sets it and copies
       it to the liquid/fluid materials.
    6. `biomes.json` decor gets berries/mushrooms/log_mushrooms; `biome_registry` FOOD_KINDS;
       `biome_table.gen.h` regenerated: rerun `tools/biome_codegen.gd` after merging if another part
       touched `biomes.json`.
    7. New `find_nearest` kinds: cavern, ravine, geode, monolith, sea_stack, arch, delta. New
       `FineGen.ruin_pots_near` (for E), `surface_map`, `cave_map`; `WorldGen.set_variant` (debug).
  - D: an optional `only` parameter on `SimBubble.blast` (part A's file, for the world vacuum); two
    sprites in `tools/make_item_sprites.py` (part C's file, just above "validate + write"). Regenerate
    `data/item_sprites.json` after merging.
  - E: `pattern_tiles` widened to `int[8]` (tiles 0-255) in `assets/shaders/terrain_common.gdshaderinc` and
    `GameMaterials.pattern_tiles()`; `tools/make_atlas.py` grows to the rows E needs and appends
    `tools/atlas_rocks.py`'s tiles after empty placeholders for 104-135 (replace them with B/C/D's tiles);
    `materials.gd` has placeholders for ids 116-159 (replace with A-D's) and defines ids 200-202 (pyrite,
    red and yellow ochre: placed by E's generator, used by F); atlas tiles 168-170 hold those three.
    Small hooks in `fine_gen.cpp` (part B's) call `rock_types.cpp`: keep them when B's rework lands.
  - F: `player.gd` `item_use` hook (checked first in `_use`, wired in `main.gd` with `stone_age`, its save
    and summary); `work_system.gd` knapping any `knap` input (+ `e.input`), `dur_x` on knapped heads, the
    held stack into `AssemblyEntity.use`, `tar_hafted`; `assembly_entity.gd` part data; `inventory.gd`
    `durability_of` + `dur_x` merge, read in `item_icon.gd` and `inventory_screen.gd`; `projectiles.gd`
    `throw(..., speed_x, damage_x)`; `mining_table.gd` tool `speed`; `work_palette.gd` five colours (atlas 30);
    `block_entities.gd` creating StoneAge kinds, `footprint_cells`, heights; `survival.gd` `clothing` +
    `felt_c`; `quest_book.gd` events; `recipe_browser.gd` categories + info fallback; `recipe_registry.gd`
    and `item_registry.gd` load `stone_age_*.json`, `stub` items; `settings.gd` a Stone Age block;
    `make_item_sprites.py` imports `stone_age_sprites.py` above "validate + write" (regenerate the JSON);
    `game_commands.gd` the `flint` kit; `mining.json` seeds from tall grass/reeds; `crops.json` einkorn,
    emmer, flax; `items.json` tags (flint `knappable`, cordage/strips `bowstring`, loose copper ore
    `ground_place`); knap recipes take `#knappable`. Part E's items are `"stub"` entries in
    `stone_age_items.json`: delete the stubs once E is merged. Ids 200-202 are E's (pyrite, ochres).
    At the merge with D: put the `item_use` check after D's `_offhand_builds(hit)` line in `player._use`; pass D's
    `sneaking` through `_try_place`; write the `flint` kit in D's block counts (sand 1, dirt 2, oak_log 2: D's kits
    count blocks of voxel items); the clothing panel docks itself once `InventoryScreen.add_side_panel` is there.

  - G: `scripts/player/mining_table.gd` (part D's) `roll_list` reads `Seasons.season` (two lines: "seasons"
    filter, "winter_mult"); `scripts/ui/hud.gd` hearts wrap past ten (max health above 20); three sprites in
    `tools/make_item_sprites.py` (part C's, above "validate + write"; regenerate `data/item_sprites.json`);
    `scripts/main.gd` sets up `SurvivalLoop` after Survival and adds `loop` to the stats JSON; SurvivalLoop
    docks its `NutritionPanel` through part D's `inv_screen.add_side_panel(ctrl, "below")` (falls back to a child of
    `InventoryScreen._panel` before D's merge), places the trackers under `main.minimap.bottom_y()`, and wraps
    `player.work_use`; Seasons
    frost erases from `Farming.crops` and writes through its `set_voxel` (part C: if crops stop being one
    material per voxel, `Seasons.kill` follows); `"nutrition"` on 10 foods in items.json, `"winter_mult"` on
    raw meat in mobs.json, `"seasons"` on the blueberry bush in mining.json, `"frost_c"` in crops.json.

---

## Part A: liquids, simulation and falling pieces

The developer's main complaint: the Noita water does not behave like water. Reports, in priority order:

1. **02-19-55** "noita simulation still not working on fluids. this crater should get filled and part of
   the river should lose elevation, gravity and all that." Plus **02-54-16** "water crater": a dug
   crater in a river leaves a dent in the surface with stray triangles; the river does not refill it.
2. **02-17-32** a 3x3x3 pocket of water, dug out, gave far more water than it held: the reservoir rule
   (ADR 0045: generated water fills openings, never drains) should only apply to big bodies (sea, big
   lakes, main rivers), not to small pockets and puddles.
3. **02-22-37** water high up is frozen, not simulated: "a smaller area around the player that is always
   simulated, and then everything in vision". Today `SimBubble` is a 64 x 32 x 64 m window.
4. **02-17-49** sand should sink through liquids and push them up (density swap in the kernel).
5. **02-23-24** lava pours out of a cave wall as scattered loose cells, not a stream.
6. **02-02-16** under water in a cave, the fluid corner patch draws spikes/triangles inside the stone
   (`voxel_patch_fluid.py`: corners next to open air drop; here they drop against solid voxels).
7. **02-09-43** waterfalls do not work and have no sound (`WorldGen.waterfalls_near`, `waterfall_spray.gd`,
   the `waterfall` sim loop in `data/sounds.json` needs moving water with air below).
9. **For part D's map** (small): a native `SimWorld.surface_map()` returning, per column of the live
   window, the top non-air material and its height (PackedByteArray/PackedInt32Array), so the minimap
   shows real edits. Agree the exact signature with part D.
8. **02-51-52** "meshes should still be destructible": a felled tree in flight (a `RigidPieces` mesh) cannot
   be cut with the laser or dug. Pieces in flight should take digs/cuts like voxels.

Direction calls to ask first: what counts as a "big body" for the reservoir (size threshold, or only
sea + named rivers/lakes?); should a breached river really lose level downstream (drainable rivers
were turned down for reservoirs on 2026-10-25, this report asks for it back); how big the always-live
zone may be against the 8.3 ms budget (measure the cost of a bigger or taller window first, offer
2-3 sizes with numbers).

Checks: `water_test=1 target=river`, `tools/water_surface_probe.gd`, a laser crater beside a river,
filmstrips of the refill (never judge motion from one frame), `/fell` + laser through the falling crown.

## Part B: world generation

1. **02-03-27** caves are small and unusable, and there are no cave entrances on the surface.
2. **02-28-03** (an instruction, not a bug) "worldgen should use even more real-world features, looks and
   landmarks, and make caves epic": research real landforms worth adding (canyons, mesas, arches, sea
   stacks, karst towers, fjords, volcanic cones, river deltas, waterfalls over cliffs ...) and cave types
   (huge caverns, ravines, lava tubes, flooded caves, cave rivers, crystal geodes), pick with the developer.
3. **02-05-38** mountains look good but should be more massive: a wide foot that rises gently and gets
   steeper and steeper, with very steep peaks.
4. **02-11-15** "more generation glitches": the river surface stands above its gravel bank as a glass wall
   (water level higher than the bank it should rest against: FineGen river levels / bank fill, ADR 0045).
5. **02-20-46** sludge "generates weird": a toxic pool whose surface stands above its rim with vertical
   walls, same fault as 4 for generated pools.
6. **Older, still open (2026-10-24 22:15):** far LOD trees look chunky and ugly (LOD 1-2 beyond the 24 m
   full-detail cap; `far forests as leaf domes`).
7. **Trails: moved to part H** (it needs B's new terrain first; see below).
8. **Scarcer wild food** (developer request): blueberry bushes and mushrooms are too common. Today the
   densities are constants in `fine_gen.cpp` (bushes `(s.seed >> 26) % 6 == 0`, mushrooms
   `(hs & 31) < 3` and `hh % 29 == 1`); move them into data (per biome in `biomes.json`, principle 6)
   and lower them, and check the blueberry drop (`mining.json`: 1-3 per bush). Agree the numbers with the
   developer (e.g. half or a third of today). Part G's food variety leans on this staying meaningful.
9. **Placement hook for part E's ruin pots:** ruins get breakable pots (part E makes the pot and its
   loot); give E a hook where ruins are stamped (a few pots per ruin, in corners and by walls).

Direction calls to ask first: the list of landforms and landmarks to add and how rare each is; cave
scale (how big is "epic", how many entrances per km^2); the mountain profile (show 2-3 variants with
`tools/terrain_map.gd` and screenshots before committing to one). New cave or landform materials use
ids 120-129, tiles 104-111. Bump `GEN_VERSION` once at the end.

Checks: `tools/terrain_map.gd`, `fine_gen_bench.gd` (generation cost per block must not grow much: loads
are already 12-17 s), `/goto cave`, `/goto mountain`, `/goto river`, screenshots from `/cam`, a budget
run in a forest and in the mountains. Keep `Biome` ids 0..12 stable.

## Part C: how things look

Everything here is a look, so show sheets or screenshots to the developer before finishing.
Their taste (see memory "voxel objects big and readable"): big, thick, irregular, varied, 8-colour
patterns, one piece, visuals over accurate sizes, treat all of a kind alike.

1. **02-16-36** farming looks weird: a carrot field is a flat orange slab with green holes. Crops should be
   multi-voxel plants (stems, leaves, the crop showing) through every growth stage, like the clutter pieces.
2. **02-35-52** a placed flower is still one weird pixel: placed flowers should be the same multi-voxel
   piece the generator scatters (ADR 0047).
3. **02-28-54** a mushroom in the hand looks like a block: held and dropped clutter (mushrooms, flowers,
   shells, kelp ...) should show its sprite or its voxel shape, not a cube.
4. **02-53-29** torches should be multi-voxel pieces (on the ground and on walls).
5. **02-44-58** the crafting table is too small and ugly: a full block-sized, readable model.
6. **02-21-05** ores in the inventory should look like modern Minecraft ore sprites (`make_item_sprites.py`).
7. **02-12-28** animals do not fit the game's look any more, and their animations play too fast.
8. **02-01-49** a held or dropped light source (torch, lava bucket ...) should light its surroundings, like
   Minecraft shader packs' dynamic lights. Mind the budget: a few `OmniLight3D`s with shadows off, capped.

New materials use ids 130-149, tiles 112-127. Every item still needs a sprite or block material
(`test_item_sprites.gd`); `tools/item_sheet.gd` renders them all. New models drawn mid-game need the
pipeline warm-up (`material_warmup.gd`).

Checks: item sheet, `tools/ape_sheet.gd`-style sheets for mobs and models, a filmstrip of a walking
animal (motion is never judged from one frame), screenshots of a farm at each stage, a budget run with
10 torches in view.

## Part D: player, inventory, interaction, fire and weather

1. **02-47-46** clay knapping places the clay instead. Controls like Minecraft: Ctrl = sprint, Shift =
   crouch/sneak, and crouch + right click on clay starts clay forming. Ctrl + wheel is brush size and
   laser width today: keep that working while Ctrl also sprints.
2. **02-18-35** cannot build onto a seashell (or other clutter): right click tries to pick it up. Picking
   up should only happen with an empty hand or a held click (ADR 0043); a click with a block in hand places.
3. **02-27-14** cannot put fuel (coal) into the firepit, and a broken firepit leaves its voxels as ghosts.
4. **02-36-31** pick up items on the ground from a bit further away.
5. **02-56-13** inventory: click outside the window to drop the held stack, Q over a slot drops it, and a
   trash slot (as in Minecraft mods).
6. **02-57-10** an offhand slot, as in Minecraft and Vintage Story, so in-world crafting (sieving, smithing:
   hammer in one hand, work piece in the other) is easier.
7. **02-59-22** creative world vacuum: pick which voxels it takes; hold left click to suck up only those
   where you aim. Belongs with the playtest gadgets (`scripts/items/playtest_tools.gd`).
8. **02-23-50** update the kits (`/kit`): bring every kit up to date with today's items and gadgets.
9. **02-15-01** fire and flint and steel: make fire a scary, destructive force (forest fires that spread
   fast), and add weather: when a forest fire starts, a random timer brings rain that puts it out, so a
   region burns, not the whole map. Fire spread is the sim's fire (data in `materials.gd` first); if the
   kernel needs changing, ask part A. Rain is new (`scripts/world/weather.gd`): sky, particles, sound,
   wet ground, and it puts fires out.

10. **Minimap and world map** (developer request, added later): a minimap on the HUD whose corner
   and size are set in Settings (`user://settings.cfg`, the ADR 0032 Settings screen), with the
   coordinates and the biome in small text under it; M opens a full map. Both show only what you have
   explored, and they update as you go, so your base, digs and builds show up. You can place, name
   and delete waypoints on the full map (shown on both maps and as a marker in the world); they are
   saved with the player, and the explored area is saved with the world.
   Suggested build (keep GDScript out of per-voxel loops, principle 8): far terrain from
   `WorldGen.map_image` (exists, deterministic, part B's: read only), shown only where the explored mask
   is set; near terrain from the real voxels, so edits show: part A adds a small native
   `SimWorld.surface_map()` (top material and height per column of the live window) that you colour
   per material (average colour of its atlas tile, computed once per material). Cache explored map
   tiles as images per region; redraw the minimap a few times a second, not every frame.

11. **Quality of life** (developer picks, added later):
    - Auto-refill: when the held tool breaks or the held stack runs out, the same item moves into the
      hand from the inventory.
    - A durability bar on tool icons (hotbar and inventory) and a warning sound when a tool is nearly
      worn out (`wear` per-stack data).
    - A pickup feed ("+3 Flint" at the side of the screen) and an "inventory full" note.
    - Key remapping in Settings (every action in `project.godot`'s input map, saved in
      `user://settings.cfg`), including the new sprint/crouch and the map key.
    - Chests: a sort button, quick-stack into chests near you (Terraria), labels on chests (shown when
      aimed at), and favourite slots that sorting, quick-stack and shift-click leave alone.
12. **Death with stakes:** dying drops the inventory into a grave at the death spot (a block entity
    that keeps per-stack data and never despawns), marks it as a waypoint on the map, and says the
    coordinates in chat. Today `Main._on_died` just respawns with everything. Ask the developer whether
    the hotbar or armour stays and whether creative keeps everything (default: creative keeps all).

Direction calls to ask first: the map style (flat colours or shaded relief, how waypoints look); the
offhand key and what goes in it; how destructive fire may be (spread
speed, how big a burn before rain, can it burn placed wood buildings?); rain everywhere sometimes, or
only as the answer to fires. New materials or items use ids 150-159, tiles 128-135.

Checks: gdUnit for inventory moves (offhand, trash, drops keep per-stack data: `Inventory.make_stack`),
`craft_test=1`, the menu and hands tests, a scripted forest fire with `later=` and a filmstrip of it
burning and the rain putting it out, a budget run during the fire.

## Part E: more animals, stone types and clutter items

Not from a report: the developer asked for more content alongside A-D. Three kinds, each data first
(principle 6: data + schema + tests), each fitting the voxel look (memory "voxel objects big and
readable": big, thick, irregular, varied, 8-colour patterns, one piece, all of a kind alike).

1. **Animals.** New species in `data/mobs.json` with models built like the existing ones (`MobModel`,
   MicroMesher at 6.25 cm), behaviour from the existing kinds (grazer, prey, hunter like the bear), spawn
   biomes, drops, sounds (`data/sounds.json` mobs, CC0 sources as ADR 0044). Ideas to pick from with the
   developer: deer, boar, rabbit, fox, wolf, goat in the mountains, duck/frog by water, fish in rivers,
   crab on beaches, bats and cave spiders underground. Part C restyles the existing animals (report
   02-12-28): message part C for its style decision before modelling, so new and old animals match.
2. **Stone types.** Real rock kinds with their own palette and pattern (granite, diorite, andesite,
   limestone, sandstone variants, slate, marble, chalk, tuff ...), placed as strata and regions in the
   world, each with hardness, drops, a cobble and a brick form where it makes sense, and a place in the
   ages (Vintage Story-style rock types decide which ores show up). Placement lives in the generator,
   which part B is reworking: put the rock-type selection in its own function/file
   (e.g. `src/rock_types.*`) called from FineGen with a small hook, and message part B about the hook.
3. **Clutter items.** More ADR 0047 pieces that dig whole: e.g. pine cones, acorns, feathers, bones,
   antlers, snail shells, driftwood, berries, nuts, small rock kinds matching the new stone types, ice
   shards, cave crystals. Each is a voxel piece on the ground, an item with a sprite, and is placed by
   biome. Generator placement again through a separate function with a small hook in `floor_features`.

4. **Pots in ruins with loot** (developer request, added later): breakable clay pots (a clutter piece
   with a few looks: amphora, jar, urn, some cracked) that shatter when hit (any tool, a short burst of
   shards and a sound) and drop random loot from a data table (`data/loot.json`: weighted entries with
   count ranges, per ruin kind or depth, so later dungeons reuse it): seeds, flint, cordage, copper
   nuggets, an old tool with wear, rarely something good. Part B gives you the placement hook where
   ruins are stamped. Loot is rolled when the pot breaks, deterministic from seed and position.

Direction calls to ask first: which animals (and which bite), which stone types and how many, which
clutter items and what each is for (a use in crafting, food, or only decoration). Show model and
material sheets before committing to a look. New materials use ids 160-199, tiles 136-167, ADR
0057-0058. Do not bump `GEN_VERSION` (part B does it once); tell part B your generator change needs it.

Checks: gdUnit (every item has a sprite, mobs.json schema, rock types deterministic from seed),
`tools/item_sheet.gd`, model sheets, `tools/clutter_probe.gd`, a filmstrip of each new animal moving,
`fine_gen_bench.gd` (no big cost growth), a budget run in a forest with the new clutter and herds.

## Part F: a deeper Flint Age

Not from a report: the developer asked for more progress steps in the Flint Age, historically
grounded, and approved the whole list below (2026-10-25). Today the Flint Age is one chapter of 35
quests (ADR 0023, 0028, 0030, 0042) running from knapping to copper casting. Part F splits it into the
four real stages of the Stone Age, each its own chapter in the progression book, and adds the steps
below as data plus in-world mechanics in the style of ADR 0030/0042 (work on the ground, held clicks,
no new screens where the world can do it). Build the starred ones first; the rest follow in order.

**1. Old Stone Age (hunter-gatherers)**
- ★ Birch tar: roll birch bark, bury it under embers (a small sealed pit like the charcoal pit), get
  tar. Tools hafted with tar wear half as fast as cordage-lashed ones (`durability`).
- ★ Heat-treated flint: flint buried in sand under a burning firepit becomes tempered flint, which
  knaps into sharper, longer-lasting heads.
- ★ Bone and antler: an eyed bone needle sews fur clothing (cloak, hood, boots) that counts in body
  temperature (`Survival`); bone fish hooks, harpoon, antler pick.
- ★ Spear-thrower (atlatl): throws the existing spear about twice as far and hard (`Projectiles`).
- Keeping fire: tinder fungus holds an ember to carry fire; the Flint Age striker is flint + pyrite
  under its own id (e.g. `pyrite_striker`). Part D keeps `flint_and_steel` as the Iron Age item (flint +
  iron ingot); giving the striker `"ignites": true` and the `firestarter` tag makes it work with D's
  fire code, no hook needed.
- Ochre and cave painting: dig red/yellow ochre, grind it on a stone, mix with fat, paint hand stencils
  and animals onto walls as a patterned overlay; a quest.
- Fat lamps: a hollowed stone with fat and a moss wick, a carried light for caves. Part C's
  `scripts/visuals/torch_lights.gd` lights any item with `"light": {}` in `data/items.json` (optional
  `{"range_m": 7.0, "energy": 1.5, "color": "#ffb86b"}`, torch values are the defaults) while held and
  while dropped; nothing else to register.

**2. Middle Stone Age (water and the bow)**
- ★ Microliths and the bow: tiny flint bladelets set in tar make arrowheads; a bow from a stave with a
  sinew or cordage string.
- ★ Dugout canoe: fell a big tree (rigid pieces), hollow the log with fire and an adze; the first boat.
- Fishing: wattle fish weirs and basket traps in rivers, bone hooks, cordage nets (fish come from part E).
- ★ Drying and smoking racks: meat hung over smoke keeps much longer (`spoil`).
- The dog: tame a wolf with meat scraps; it follows and helps hunt (wolves come from part E).

**3. New Stone Age (settling down)**
- ★ Polished stone axe: grind a roughout on a sandstone slab with sand and water, many strokes; slow to
  make, tough, fells big trees much faster.
- ★ First farming: gather wild einkorn, emmer and flax, sow them; a saddle quern grinds grain to flour,
  baked into flatbread on a hot stone.
- Flax to linen: ret in water, break, spin with a drop spindle (clay whorl), weave on a warp-weighted
  loom; cloth for clothes and bigger bags.
- Flint mines: surface nodules are poor; good flint lies deep in chalk, mined with antler picks
  (flint quality tiers). Needs chalk from part E's stone types.
- ★ Megaliths: move a huge stone on log rollers with levers and ropes (rigid pieces), raise a dolmen or
  stone circle lined up with the solstice sunrise (calendar); reward: a home shrine / respawn point
  that marks the seasons.
- Salt: boil brine in clay pots; salted food barely spoils.
- Longhouses: prefab blueprints for shape building, plus storage pits.

**4. Copper Stone Age (the bridge)**
- ★ Native copper, cold-hammered with a stone into awls and beads before smelting; green malachite
  (paint) turning into copper in a hot kiln is the discovery quest that opens the Copper Age.
- Ötzi's kit as the finale: copper axe, flint dagger, birch-bark boxes, tinder fungus, grass cape,
  bearskin cap; one capstone quest.

Direction calls to ask first: how much of today's 35 quests moves into which stage (show the new book
layout before rewriting it); the timing scale (minutes like the charcoal pit, or calendar days); what
the megalith reward is exactly; how a boat handles. Everything else: sensible default, recorded in
the ADR.

Coordination: part D owns `scripts/work/` (the firepit fuel fix) and `scripts/player/`/inventory:
put new mechanics in new files and keep hooks into existing ones small, and message part D about
them. Part E brings fish, wolves, chalk and other stones: build against their names, message part E
for them, and stub what is not there yet. Part C owns the light from held light sources and the
model looks. New materials use ids 200-239, tiles 168-199, ADR 0059-0060.

Checks: gdUnit for every new recipe type and the quest graph (every quest reachable, no orphan item),
`flint_age_test=1` and `craft_test=1` extended so each new mechanic is driven through the player's own
clicks, `/kit flint` updated, item sheet, a budget run, items in `docs/plans/to-test.md` under "Part F".

## Part G: the survival loop

Not from a report: the developer picked these from a quality-of-life and gameplay-loop brainstorm
(2026-10-25). They give the day-to-day loop reasons: eat well, prepare for winter, hunt for ore,
explore for discoveries, and keep track of goals.

1. **Food variety (Vintage Story nutrition).** Every food belongs to one or more groups (meat, grain,
   fruit, vegetable, dairy); eating fills that group's bar, the bars slowly drain, and a balanced diet
   raises max health (a single-food diet gives nothing extra). Groups are data on the items; the bars
   live in `Survival` beside hunger and saturation, saved with the player, shown in the inventory.
2. **Seasons that matter.** The calendar exists: crops stop growing and frost kills them in winter (per
   crop data: hardy ones survive), wild berries and plants follow the season, animals are leaner in
   winter. A **root cellar**: a closed room below ground (the room flood fill of `ClimateProbe`) is
   cool, and food stored in chests there spoils much slower. Part F adds salting, drying and smoking:
   use its items when they land, stub until then.
3. **Prospecting pick.** A Copper Age tool (a flint version that only says "something" vs "nothing" is
   a nice Flint Age teaser): tap rock and it reads which ores lie nearby and how rich they are, and marks
   the find on part D's map as a waypoint. Ore data comes from the generator's ore veins (part B's
   native code: if a query like "ore densities around x, y, z" is missing, ask part B to add it).
4. **Discovery journal.** The first time you find each biome, ore, landmark, ruin or animal, it lands in a
   Discoveries tab of the progression book with a short line of text and a small reward. Part B adds
   landforms and landmarks, part E new animals and stones: read their names from the data, so new
   entries appear without code changes.
5. **HUD trackers.** Pin a recipe from the recipe browser: a small checklist on the HUD shows its
   ingredients and ticks them off as the inventory fills. Track a quest from the book the same way.

Direction calls to ask first: how hard the diet bites (bonus only, or also a penalty); how hard
winter is (crops die vs only stop); the prospecting pick's range and how exact it is; what a discovery
reward is.

Coordination: part D owns the HUD layout and the inventory screen (minimap, pickup feed, durability
bars land there too): agree with D where the trackers and nutrition bars sit, and keep your HUD code in
your own nodes/files. Part F owns `data/quests.json` and the book's chapters: the Discoveries tab is a
separate tab, agree its place with F. New materials use ids 240-249, tiles 200-215, ADR 0061-0062.

Checks: gdUnit for nutrition (fill, drain, balanced bonus), seasonal crop rules, cellar spoilage rate
and discovery triggers; a scripted run that eats, winters a crop, prospects and pins a recipe; a
budget run; items in `docs/plans/to-test.md` under "Part G".

## Part H: trails (starts after part B is merged)

Moved out of part B, which asked for it: trails are a subsystem of their own and need B's new
landforms, caves and mountains to route over. Start it once B's branch is on main.

Worn paths through the terrain in their own colour (packed dirt, gravel, trodden grass), linking
ruins, river crossings, passes and lakes; they follow the land (route search over the macro terrain,
no straight lines up cliffs), sit slightly sunk into the ground and are narrower in forests. Native,
in the generator (deterministic from seed, cheap per block: precompute routes per region like the
hydrology tiles), and they colour `WorldGen.map_image` so part D's map shows them. Bump `GEN_VERSION`.
Reserved: ADR 0063, material ids 250-254, tiles 216-219. Checks: `tools/terrain_map.gd` with trails,
`fine_gen_bench.gd`, screenshots along a trail, a budget run.

---

## Briefs to paste into each session

Each session reads `CLAUDE.md`, this file and its own part, then starts with the direction calls.

- **A:** "Take part A of docs/plans/2026-10-25-bug-report-split.md (liquids, simulation, falling pieces).
  Work in your own worktree as the file says. Read every report screenshot of your part first, ask me the
  direction calls, then fix in the priority order given."
- **B:** "Take part B of docs/plans/2026-10-25-bug-report-split.md (world generation). Own worktree. Read the
  screenshots, research landforms and cave types, ask me the direction calls with sheets, then build."
- **C:** "Take part C of docs/plans/2026-10-25-bug-report-split.md (how things look). Own worktree. Read the
  screenshots, show me sheets before you finish each look."
- **D:** "Take part D of docs/plans/2026-10-25-bug-report-split.md (player, inventory, fire, weather). Own
  worktree. Read the screenshots, ask me the direction calls, then build."
- **E:** "Take part E of docs/plans/2026-10-25-bug-report-split.md (more animals, stone types, clutter
  items). Own worktree. Ask me which ones, show me sheets, coordinate with parts B and C as it says."
- **F:** "Take part F of docs/plans/2026-10-25-bug-report-split.md (a deeper Flint Age). Own worktree.
  Ask me the direction calls, build the starred steps first, coordinate with parts C, D and E."
- **G:** "Take part G of docs/plans/2026-10-25-bug-report-split.md (the survival loop: diet, seasons,
  prospecting, discoveries, HUD trackers). Own worktree. Ask me the direction calls, coordinate with
  parts B, D, E and F as it says."
