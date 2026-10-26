# Systems reference

Per-system notes moved out of CLAUDE.md (2026-10-25) so they are read on demand, not loaded
every session. Keep this current when a system changes; CLAUDE.md only points here.

- Phase 2 (current): **the voxel world (ADR 0033-0036).** One 12.5 cm grid for terrain, simulation,
  collision and rendering. Native `FineGen` draws the voxels over `WorldGen`'s 0.5 m macro layer
  (landforms: ~8 km continents of plains, hill country and massif mountains to ~400 m in a 640 m world,
  sea cliffs and stacks, monoliths, meanders, deltas (ADR 0051); climate, biomes from biomes.json,
  hydrology, big caves: halls carved to LOD 2, ravines, cave mouths, geodes (ADR 0052); GregTech ore veins, ruins; ledges of 3-8 voxels on the ground and 0.5 m in caves, cliff bands on the massifs, a 110 m sea (ADR 0063)); aged trees with bark trunks,
  roots and clumped crowns, young trees, a forest floor of leaf litter, logs, stumps, bushes, ferns and
  mushrooms; far forests as leaf domes. Flat colours: one texel of the material's tile per voxel
  (`voxel_sample`); props use `terrain_prop.gdshader`. `SimBubble` ticks a wrapped 64 x 64 x 64 m window
  of the world's voxels (Noita kernel: sand, water, fire, gas). Brush digging and building
  (`scripts/player/brush.gd`: Ctrl + wheel size, B / Shift + B shape); voxel items count voxels, recipes blocks of 64
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
  **World tools panel (ADR 0071):** G (`/tools`) opens the panel: brush shape and size, builder modes, prefabs, the ghost
  toggle (`Settings.brush_ghost`) and the keys, each with a tooltip (`scripts/ui/tools_menu.gd`). Placed powders go in as
  their still kind (`GameMaterials.still_of`): sand built from dug sand no longer slides apart.
  **Survival (ADR 0040):** `Survival` (`scripts/survival/`) runs hunger (Minecraft bar + saturation), body temperature
  (climate, season, hour, caves, water, rooms from the native `ClimateProbe` flood fill, heat of fire cells and lit
  entities) and food spoilage (perishable stacks carry `spoil` through every Inventory move, averaged on merge; rot).
  Tunables in `Settings`; `/survival` reads and sets it. A new `class_name` needs an `--import` before the game sees it.
  **Animal crowds (ADR 0067):** body moves go through godot_voxel's `VoxelBoxMover`, which needs the
  `voxel_patch_boxmover.py` build (per-block reads; a bear's move cost 0.5 ms without it). `Mob` steps every frame
  within 16 m, every 2nd to 40 m, every 4th beyond (running animals full rate, standing ones at most every 2nd);
  `MobManager.neighbours` is the 2 m grid for pushes. Check with `mob_bench=100` and the JSON's `mobs.step_ms`.
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
  **Bronze and iron ages (ADR 0070):** tin laid into a molten copper crucible on the firepit makes bronze (cast or
  smithed, tier 2); a bloomery (`BloomeryEntity`) turns iron ore and charcoal into a hot bloom, only a forge
  (`ForgeEntity`; both are `FirepitEntity` subclasses) heats `forge_heat` items, and only the bronze anvil (`AnvilEntity`
  kind `bronze_anvil`, tier 1) takes metal with `anvil_tier` 1; iron is smithed, never cast (tier 3). Smelt recipes take
  `count` and `hot`. The furnace smelts no ore. Check: `test_metal_ages.gd`, `metal_age_test=1`.
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
  **Water look (ADR 0045, surface ADR 0069 A):** godot_voxel fluid patch (`voxel_patch_fluid.py`, sets the blocky
  mesher's PADDING to 2): the surface is one height field, each top corner the mean of the surface tops of the
  up-to-four columns around it (a column's highest cell of the fluid within two cells up or down; an open column with
  none in reach votes the bottom of the lowest fluid cell at that corner; solid does not vote), so every cell agrees on
  a shared corner and slopes up to two cells per cell are sheets, not wedges; the flow state follows the corner
  heights, no side face toward a column one or two cells lower, capped cells keep the stock corners. A drop of three or
  more cells between neighbouring surface cells (the generator's terraces and the old waterfall sites) is out of reach
  and draws as a vertical water face. FineGen rivers interpolated per voxel and their trenches and low banks filled;
  `tools/water_surface_probe.gd` counts holes, glass edges, corner seams (`rule=field|ramps`; 0 for field), out-of-reach
  wall corners and the histogram of surface steps. **All water is finite (ADR 0068, replacing the reservoirs of ADR 0045 and the big bodies of
  ADR 0049):** generated water (`water_static`) is still until a fresh opening (air that changed in the last two ticks) touches
  it beside or below; then the cell becomes plain mass-conserving `water` and drains into the hole, cell by cell (the cell it
  leaves is the next opening); the river beside a hole drops, nothing refills it. Air over a river's own lower step never wakes
  it, moving water in a hole never climbs out onto still water, but water that rests on a still body spreads flat over it.
  Lava sinking through still water trades places with it (the water rises, finite).
  **Generated liquids (ADR 0069 C):** every pool and sea is a still kind (`water_static`, `toxic_sludge_static`,
  `lava_static`; the deep lava sea too); FineGen drops lone still cells (air above, none of its kind beside, air beside);
  river surfaces interpolate with plain bilinear weights and blend every stream segment in reach by distance, so
  they are continuous; a drop of three or more cells between neighbouring surface cells is a cascade where the bed
  drops as much (the river's surface is clamped to the ground), and `water_surface_probe.gd` counts the rest as
  terraces; `tools/column_probe.gd` prints a column's hydrology. GEN_VERSION 22.
  **Liquid speed (ADR 0069 B):** `liquid_speed` substeps per material (water 3, oil, blood, sludge 2, lava 1): after
  the main pass the kernel runs extra passes that only spread and pour moving liquids with substeps left (no falling,
  no downhill run, nothing standing on a cell that moved this tick); `mark_` is the per-pass move marker, `stamp_`
  still counts ticks. A surface evens out by half the difference to a lower neighbour (ends flat), rests at a
  difference under two eighths. `test/test_liquid_speed.gd`. `water_test=1 target=river` checks a
  trench and a crater; kernel tests `test/test_sim_v6.gd`. What still moves when the window walks on finishes in a pin (ADR 0050).
  No waterfalls (the sheet, spray, roar and landmark are gone, GEN_VERSION 21): a river just drops over its cliff.
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
  **Brush shapes and sizes (ADR 0065):** cube, sphere, cylinder, `flat` (a square one voxel deep in the aimed face's
  plane) and `column` (size/2 wide, 2 x size long along the face normal: a shaft, tunnel, pillar or beam). Survival
  sizes 1-8 voxels, digging capped by `MiningTable.dig_cap` and paid in time per voxel and a tool use per block; creative
  adds 16 and 32 (4 m), reaches 12 m and paints with Shift + right click (the dig shape takes the held block).
  `BrushTool` (`scripts/player/brush_tool.gd`) draws the shell of the exact cells (orange dig, blue placement, half
  resolution above 8) and runs brushes above 8 as `ShapeTool.fill` jobs of cached relative cells, 6000 cells a frame;
  big creative digs drop nothing. HUD line right of the hotbar; `/brush <shape> [size] | hold <s>`; costs per shape and
  size: `tools/brush_bench.gd`.
  **TNT (ADR 0065):** material `tnt` (203), a voxel item placed with the brush; a charge is one 0.5 m block cell holding
  TNT, its blast radius `tnt_radius_m` x cbrt(its TNT share). `Tnt` (`scripts/items/tnt.gd`) lights charges touched by
  fire or lava, aimed at with flint and steel or the `igniter` (sneak: every charge in 16 m), or reached by a blast
  (random 0.5-1.5 s fuse); a primed box blinks, hisses, sparks and falls for `tnt_fuse_s`, then three native spheres by
  hardness (soft full radius, stone 3/4, hard rock 1/2, obsidian and bedrock never), rim seeds to the rigid piece check,
  damage and knockback to 2 x radius, 30 % of the estimated take as drops. One blast per frame; a 20-block chain holds
  max frame 12.4 ms. `/tnt grid [n] [spacing_m] | light | stats`, `/kit tnt`; JSON `tnt`.
  **Next:** the rest of the ADR 0037 work order; still open from before: fire/smoke pacing per cell, fast flight vs
  generation, the water spill / blast / `persist=1` runs; plan in `docs/plans/lotl-overhaul.md`, playtest
  list in `docs/plans/to-test.md`.
- Material ids are definition order, append only (saves and the generator use them): 32..36 stations,
  37..43 detail, 44 reserved (once `CHISELED`), 47 blood, 48..52 ground pickups, 57..75 animals, farming,
  furniture, 76..87 and 90..101 content passes, 88..89 work spots, 103..107 Flint Age, 108 leaf litter,
  109 cobblestone, 110 bricks, 111..115 clutter (plant stem, kelp, seagrass, seashell, mushroom stem; ADR 0047),
  116..119 liquids (part A), 120 crystal (geodes, ADR 0052), 121 lava_static (ADR 0069 C), 122..126 reserved for world generation, 127..129 savanna grass,
  savanna tall grass, red earth (ADR 0064; `GameMaterials.is_grass` / `is_tall_grass` treat them as grass), 130..132 torch and
  wheat parts (ADR 0053), 160..202 rock kinds, clutter, pyrite and ochres (ADR 0057), Stone Age ids after (ADR 0059), 203 tnt (ADR 0065).
  Atlas 8x22 tiles (16..19 ores, patterned since ADR 0064; 104 crystal, 109..111 savanna grass, tall grass, red earth, 112..120 crops and torches, 136..170 rocks, 171..172 tnt side and top; 84 leaf litter, 85 oak bark, 86 pebbles, 87 flint, 93/97 twigs top/bark, 94..96 ore-bearing
  stones; 98 cobblestone, 99 bricks; 100 blueberry bush; 101 kelp, 102 seagrass, 103 seashell: full, add a row to grow;
  after editing `tools/make_atlas.py` run `--import`, or the game draws the old texture). `"pattern": true` materials lay their tile over the grid, one texel per voxel (ADR 0039). Ground pickups are voxel shapes (stone chunks on a 0.5 m patch, sticks as `FineGen::STICK` stamps);
  clutter (flowers, mushrooms, seashells, kelp, seagrass) and ore nuggets are voxel pieces that dig whole (ADR 0047,
  `GroundPickups`, probe `tools/clutter_probe.gd`). Crafting: any layout, amounts in inventory counts (ADR 0046). `MicroMesher` builds the
  models of stations, furniture, work pieces, mobs and the ape at 6.25 cm cells; it winds triangles
  counter-clockwise, so pass its meshes through `MobModel.outward` (which reads an ArrayMesh back and
  waits 4-6 ms for the render thread: never per edit).
