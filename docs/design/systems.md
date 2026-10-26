# Systems reference

Per-system notes moved out of CLAUDE.md (2026-10-25) so they are read on demand, not loaded
every session. Keep this current when a system changes; CLAUDE.md only points here.

- Phase 2 (current): **the voxel world (ADR 0033-0036).** One 12.5 cm grid for terrain, simulation,
  collision and rendering. Native `FineGen` draws the voxels over `WorldGen`'s 0.5 m macro layer
  (landforms: ~8 km continents of plains, hill country and massif mountains to ~400 m in a 640 m world,
  sea cliffs and stacks, monoliths, meanders, deltas (ADR 0051); climate, biomes from biomes.json,
  hydrology, big caves: halls carved to LOD 2, ravines, cave mouths, geodes (ADR 0052); GregTech ore veins, ruins); aged trees with bark trunks,
  roots and clumped crowns, young trees, a forest floor of leaf litter, logs, stumps, bushes, ferns and
  mushrooms; far forests as leaf domes. Flat colours: one texel of the material's tile per voxel
  (`voxel_sample`); props use `terrain_prop.gdshader`. `SimBubble` ticks a wrapped 64 x 64 x 64 m window
  of the world's voxels (Noita kernel: sand, water, fire, gas). Brush digging and building
  (`scripts/player/brush.gd`: Ctrl + wheel size, B shape); voxel items count voxels, recipes blocks of 64
  (`ItemRegistry.units`); block entities cover footprints (`BlockEntities.footprint`). The loading screen
  covers the start until the world is in, and respawns and teleports past a 100 m hop (or onto terrain
  not drawn yet) until the ground is in: voxels under the feet and full detail within 24 m, the horizon
  streams in behind it (ADR 0066); the player and animals are frozen under it. `tp_test=1` times them. Gameplay on top: player loop (ADR 0005), stations (0007), recipe browser (0008), collision and
  swimming (0011), animals, farming, furniture (0022), Vintage Story early game (0023), progression book
  (0028), Flint Age (0030), menus and saves (0032). Probes: `tools/terrain_map.gd`, `fine_gen_bench.gd`,
  `fine_probe.gd`, `face_probe.gd`. Loads take 12-17 s: budget runs use `load_cap_s=30`.
  Full detail is capped at 24 m (`Settings.MAX_DETAIL_M`): 32 m blows the budget in forests.
  **Direction (ADR 0037, 2026-10-24):** rigid voxel pieces (trees, cut pieces, later buildings topple),
  LotL shape building + box mode + prefabs from the start, tools and gear crafted in the world (grid for
  bulk), ages into GregTech, hunger + temperature + spoilage, biting wildlife and dangerous places (calm
  surface, combat a side system), patterned voxels for built materials, structures / dungeons / cave
  biomes / roads / traders, new systems keep state server-side. Magic is decided in Phase 3.
  **Rigid pieces (P12, on by default, `rigid_pieces=0` turns them off):** `RigidPieces` (technosim) cuts loose wood (support fill, max-flow
  strength, hardness breaking), flies it and settles it back into voxels. It runs on the sim's driver thread before each
  tick (`set_async`; `get_voxel` does not wait for that phase); a forest felling holds p99 7.2 ms, the main thread pays
  ~2 ms (`prototypes/p12-rigid-pieces/README.md`). Trees never weld (FineGen gap).
  **Shape building (ADR 0038):** V opens the build menu (brush, box, wall, cylinder, cone, sphere, stairs, slope,
  box mode, box dig, select, prefabs); `ShapeBuilder` (`scripts/building/`) orchestrates, native `ShapeTool` rasterizes,
  meshes and fills; prefabs in `user://prefabs/<category>/<name>.json`. Scripted checks: `later=/a;wait 1;/b` runs chat
  commands after loading (`/shape ... at x y z`, `/shape drag`, `/shape menu`, `/box`, `/prefab`).
  **Survival (ADR 0040):** `Survival` (`scripts/survival/`) runs hunger (Minecraft bar + saturation), body temperature
  (climate, season, hour, caves, water, rooms from the native `ClimateProbe` flood fill, heat of fire cells and lit
  entities) and food spoilage (perishable stacks carry `spoil` through every Inventory move, averaged on merge; rot).
  Tunables in `Settings`; `/survival` reads and sets it. A new `class_name` needs an `--import` before the game sees it.
  **Lighting grade (ADR 0041):** golden (warm sun, warm haze, ACES + S-curve LUT) is the look; `Atmosphere.GRADES` holds it
  and the alternatives natural and clear (`/grade <name>`, `grade=`); fog is per km. `/shot <name>` saves a frame without the HUD.
  Underground (camera 2-10 m under the generator surface, unless `Atmosphere.sky_open`: a ray up through the loaded
  voxels, then the cave map's ravine/mouth flag) the sun goes out, the fog loses its sun haze and covers the sky, and
  the ambient is a dim neutral bounce: caves are lit by torches, lava and glow only (bug reports 2026-10-25_21-21-*).
  **Crafting in the world (ADR 0042):** flint and copper tools are assembled on the ground part by part (`assemble` recipes,
  `AssemblyEntity`), the firepit is worked by hand (no screen), tools wear (`durability` = uses), hot metal carries `hot_until`
  against `Inventory.clock_s`, and a stone anvil smiths hot copper with a hammer (`smith` recipes, height per column,
  `AnvilEntity`). Stacks keep per-stack data (`spoil`, `wear`, `hot_until`, `work`) through every move: build stacks with
  `Inventory.make_stack`. `craft_test=1` checks it all (headless with `craft_shots=0`).
  **Grid Fill and Clear (bug report 2026-10-25):** the recipe view shows **Fill** right of the output for a grid recipe
  (shaped/shapeless) while a crafting grid is on screen: it returns the grid's contents to the inventory, then lays the
  recipe out (shaped as drawn from the top left, shapeless in reading order) from the inventory and the worn bag, and closes
  the recipe view so the grid shows. Shift fills as many crafts as the inventory holds and a slot's stack allows (one
  stack of each ingredient); a kept tool (saw, knife) goes in once with its wear; a tag ingredient takes the member the player has most of
  (members are not mixed: 1 oak and 1 birch stick do not make a two-stick recipe). All or nothing: when not one craft is
  there, Fill is greyed out and its tooltip says what is short (also for another station or a recipe too big for the 2x2).
  With no recipe chosen, the panel's **Clear** button (left of the grid) puts the grid back in the inventory. Logic:
  `CraftingGrid.fill` / `return_to`, `PlayerInventory.fill_craft_grid` / `clear_craft_grid`; tests `test/test_grid_fill.gd`;
  `menu_test=1` presses both (JSON `grid_fill`, shots `craft_fill_button`, `craft_filled`, `inventory_clear`).
  **Water look (ADR 0045):** godot_voxel fluid patch (`voxel_patch_fluid.py`: corners next to open air drop, steps
  become ramps), FineGen rivers interpolated per voxel and their trenches and low banks filled; `tools/water_surface_probe.gd`
  counts holes and glass edges. Generated water is still until disturbed; then the sim measures its body (ADR 0049): a big
  one (64 m^3+, or past the window) pours flowing water into the hole and keeps its level, a small one drains like any spill;
  `water_test=1 target=river` checks a trench and a crater. What still moves when the window walks on finishes in a pin (ADR 0050).
  **Every liquid looks like water (2026-10-25):** one fluid shader body (`terrain_fluid.gdshaderinc`), no atlas texture;
  each liquid's character (colours by depth, rim, ripples, gloss, glow, oil sheen, sludge bubbles, lava crack scale) is
  data in `data/liquid_looks.json`, set per fluid material by `World.apply_liquid_look` (`test_liquid_looks.gd` checks
  every key is a uniform). `liquids_test=1` pours a pool of each and shoots them by day and night.
  **Items burn in lava:** a dropped stack touching a liquid with `temperature_of_fire` burns up with `item_burn` +
  `item_burn_smoke` and a sizzle (`ItemEntity.burns_at`); items tagged `fireproof` (obsidian, lava bucket) survive.
  **Held right clicks (ADR 0043):** ground pickups are whole pieces (`GroundPickups`): aimable (aim-only box), a dig takes the
  whole stick, a 1 s held right click pops it into an item on the ground; firepit moves are held too (`hold_key`/`hold_done`).
  **Sound (ADR 0044):** the `Sound` autoload (`GameAudio`, `scripts/audio/`) plays `data/sounds.json` (material groups,
  bursts, mobs, sim loops, music) from CC0 assets cut by `tools/make_sounds.py` out of `~/Tools/downloads/sounds/` (recipes
  in `tools/sound_recipes.json`, ffmpeg in `~/Tools/ffmpeg/`). It listens instead of being called: player signals,
  `ParticleFx.burst_played` (names in `fx.sound_bursts`), `FallingPieces.piece_event`, every UI button via `node_added`;
  call `Sound.play(name, pos)` only where nothing else tells the story. Buses Music / World / UI; JSON "sound"; `sound=0`.
  **Four Stone Age stages (ADR 0059):** the book's Old, Middle, New and Copper Stone Age chapters; `StoneAge`
  (`scripts/work/stone_age/`, hooked first into the player's right click as `item_use`) runs earth ovens (tar, tempered
  flint), racks (dry/smoke), stroke work (polish, quern, sew, loom), water work (retting, traps), canoes, megaliths and
  circles, cave paintings, clothing; items/recipes in `data/stone_age_*.json` (new types bury, rack, stroke, soak, trap;
  slow work in calendar game hours via `StoneAge.now_days`); per-stack `dur_x` for tar/tempered tools. Scripted check:
  `stone_ages_test=1` (with `walk=1 speed=0 mob_spawn=0`; `flint_age_test=1` runs it too).
  **Next:** the rest of the ADR 0037 work order; still open from before: fire/smoke pacing per cell, fast flight vs
  generation, the water spill / blast / `persist=1` runs; plan in `docs/plans/lotl-overhaul.md`, playtest
  list in `docs/plans/to-test.md`.
- Material ids are definition order, append only (saves and the generator use them): 32..36 stations,
  37..43 detail, 44 reserved (once `CHISELED`), 47 blood, 48..52 ground pickups, 57..75 animals, farming,
  furniture, 76..87 and 90..101 content passes, 88..89 work spots, 103..107 Flint Age, 108 leaf litter,
  109 cobblestone, 110 bricks, 111..115 clutter (plant stem, kelp, seagrass, seashell, mushroom stem; ADR 0047),
  116..119 liquids (part A), 120 crystal (geodes, ADR 0052), 121..126 reserved for world generation, 127..129 savanna grass,
  savanna tall grass, red earth (ADR 0064; `GameMaterials.is_grass` / `is_tall_grass` treat them as grass), 130..132 torch and
  wheat parts (ADR 0053), 160..202 rock kinds, clutter, pyrite and ochres (ADR 0057), Stone Age ids after (ADR 0059).
  Atlas 8x22 tiles (16..19 ores, patterned since ADR 0064; 104 crystal, 109..111 savanna grass, tall grass, red earth, 112..120 crops and torches, 136..170 rocks; 84 leaf litter, 85 oak bark, 86 pebbles, 87 flint, 93/97 twigs top/bark, 94..96 ore-bearing
  stones; 98 cobblestone, 99 bricks; 100 blueberry bush; 101 kelp, 102 seagrass, 103 seashell: full, add a row to grow;
  after editing `tools/make_atlas.py` run `--import`, or the game draws the old texture). `"pattern": true` materials lay their tile over the grid, one texel per voxel (ADR 0039). Ground pickups are voxel shapes (stone chunks on a 0.5 m patch, sticks as `FineGen::STICK` stamps);
  clutter (flowers, mushrooms, seashells, kelp, seagrass) and ore nuggets are voxel pieces that dig whole (ADR 0047,
  `GroundPickups`, probe `tools/clutter_probe.gd`). Crafting: any layout, amounts in inventory counts (ADR 0046). `MicroMesher` builds the
  models of stations, furniture, work pieces, mobs and the ape at 6.25 cm cells; it winds triangles
  counter-clockwise, so pass its meshes through `MobModel.outward` (which reads an ArrayMesh back and
  waits 4-6 ms for the render thread: never per edit).
