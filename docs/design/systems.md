# Systems reference

Per-system notes moved out of CLAUDE.md (2026-10-25) so they are read on demand, not loaded
every session. Keep this current when a system changes; CLAUDE.md only points here.

- Phase 2 (current): **the voxel world (ADR 0033-0036).** One 12.5 cm grid for terrain, simulation,
  collision and rendering. Native `FineGen` draws the voxels over `WorldGen`'s 0.5 m macro layer
  (landforms: ~8 km continents of plains, hill country and massif mountains to ~400 m in a 640 m world,
  sea cliffs and stacks, monoliths, meanders, deltas (ADR 0051); climate, biomes from biomes.json,
  hydrology, big caves: halls carved to LOD 2, ravines, cave mouths, geodes (ADR 0052); GregTech ore veins, ruins; ledges of 3-8 voxels on the ground and 0.5 m in caves, cliff bands on the massifs, a 110 m sea (ADR 0063)); aged trees with bark trunks,
  roots and clumped crowns, young trees, a forest floor of leaf litter, logs, stumps, bushes, ferns and
  mushrooms; far forests as leaf domes. No far horizon past the box (ADR 0105: render distance only). Flat colours: one texel of the material's tile per voxel
  (`voxel_sample`); props use `terrain_prop.gdshader`. `SimBubble` ticks a wrapped 64 x 64 x 64 m window
  of the world's voxels (Noita kernel: sand, water, fire, gas). Brush digging and building
  (`scripts/player/brush.gd`: C holds the mine ring for the shape with the wheel sizing it, Ctrl + wheel size, B / Shift + B shape); voxel items count voxels, recipes blocks of 64
  (`ItemRegistry.units`); the deconstruction hammer (`deconstructs` on the item, `built` on a mining.json material: planks,
  bricks, thatch...) takes built blocks apart at once and gives them back, natural ground it leaves (`MiningTable.deconstructs`);
  block entities cover footprints (`BlockEntities.footprint`). The loading screen
  covers the start until the world is in, and respawns and teleports past a 100 m hop (or onto terrain
  not drawn yet) until the ground is in: voxels under the feet and the ground within 32 m, the rest of the
  40 m box streams in behind it (ADR 0066, 0073); the player and animals are frozen under it. `tp_test=1` times them. Gameplay on top: player loop (ADR 0005), stations (0007), recipe browser (0008), collision and
  swimming (0011), animals, farming, furniture (0022), Vintage Story early game (0023), progression book
  (0028), Flint Age (0030), menus and saves (0032), doors, fences, ladders and decoration as thin furniture
  (`box`, `open`, `wall`, `climb` in `FurnitureEntity.KINDS`, ADR 0094, `building_test=1`). Probes: `tools/terrain_map.gd`, `fine_gen_bench.gd`,
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
  **Shape building (ADR 0038, 0144):** V opens the build ring (tap: it stays up until a click; hold: release picks; brush, box, wall, cylinder, cone, sphere, stairs, slope,
  box mode, box dig, select, prefabs; release on a slice picks it); a placed shape's corner keeps to the snap grid
  (N: off / 0.5 m / 1 m, `ShapeBuilder.snap`, x and z only); a placed shape frees the cursor (ADR 0143): it drags the
  handles (top arrow: height, the pixel ring at the foot grows the side grabbed, the one on top hollows, the cross at the
  foot moves; white, cyan under the cursor) in snap steps, the wheel over one steps it one snap (the move handle up and down), the right button held
  looks around; a selection gets the same handles (no hollow) before `/prefab save`; `ShapeBuilder` (`scripts/building/`) orchestrates, native `ShapeTool` rasterizes,
  meshes and fills; prefabs in `user://prefabs/<category>/<name>.json`. **Blueprints (ADR 0161):** a blank `blueprint` copies a
  selected box (voxels + stations and work pieces with facing) onto a `written_blueprint` stack (`Blueprint`), placed with
  the prefab ghost it marks a site (`Blueprints`, saved per world) that a powered `builder` beside a chest builds
  (`blueprint_test=1`). Scripted checks: `later=/a;wait 1;/b` runs chat
  commands after loading (`/shape ... at x y z`, `/shape drag`, `/shape menu`, `/box`, `/prefab`).
  **The rings replaced the world tools panel (ADR 0144):** the build ring's Brush tile, or C, opens the brush ring (shapes,
  the wheel sizes it); a brush choice leaves any build mode, and a build mode hides the brush's ghost and keys, so
  the two are never on together. The ghost toggle is in Settings; dig only is Shift + middle click and the HUD line. Placed powders go in as their still kind (`GameMaterials.still_of`): sand built from
  dug sand no longer slides apart; set down over nothing it falls (`SimWorld::try_loosen`, the edit stamp).
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
  ADR 0082: that ray is cast from the camera's column and eight 2 m around it; a third of them open is daylight
  (overhangs stay lit); the look fades in and out over 2.5 s (ADR 0092). The mist lies below the camera; Video settings Fog (air haze off) and Brightness (ambient x
  `Atmosphere.brightness_mult`). Steep liquid tops (cascades) do not gloss; lava tops sample the crust on one plane.
  **Progression walk (ADR 0139):** `scripts/crafting/progression_walk.gd` walks the quest book's main roads from
  empty hands over the data with stations, tool tiers, heat, fuel, steam and power as gates; `test_progression_walk.gd`
  fails on dead ends, `tools/progression_report.tscn` writes the balance report `docs/plans/progression-walk.md`.
  **Crafting in the world (ADR 0042):** flint and copper tools are assembled on the ground part by part (`assemble` recipes,
  `AssemblyEntity`), the firepit is laid the same way (four stones, two sticks) and becomes a firepit where it lies
  (ADR 0083) and is worked by hand (no screen), tools wear (`durability` = uses), hot metal carries `hot_until`
  against `Inventory.clock_s`, and a stone anvil smiths hot copper with a hammer (`smith` recipes, height per column,
  `AnvilEntity`). Stacks keep per-stack data (`spoil`, `wear`, `hot_until`, `work`) through every move: build stacks with
  `Inventory.make_stack`. Hot and cold stacks never merge (`Inventory.same_kind`), hot metal reads in °C and
  glows (`temp_c`, `glow_color`, `BlockMeshes.heat_material`), a hand holds it only with tongs in the other, the
  forge holds two items, and sneak + right click loads a fire as full as it goes (ADR 0087). `craft_test=1` checks it all (headless with `craft_shots=0`). Stations load on a plain
  right click, show their contents in the look-at panel (`look_lines()`), lit ones burn, and smithing blows answer
  with sounds (ADR 0078). The look-at panel also draws the block's picture, its hardness and drops and a chest's or
  furnace's contents; R / U look up the aimed block or held item from the world, the recipe view has a tab per
  station, Fill works on the furnace and Mark bookmarks a recipe (ADR 0093, `info_test=1`). A machine shows in the
  browser by recipe data alone (`process` type: `station`, `ingredients`, `seconds`, `info`), and bookmarks are
  saved with the player (ADR 0115). **Hand feedback (bug report 2026-10-27_18-03-10):** `Viewmodel.swing()` plays the craft
  motion of the held item (`MOTIONS`: hammer or axe raise-and-slam, firestarter bow drill, knapping stone or flint and
  steel strike, knife scrape, clay press, saw, pan shake; stroke work and the sieve call `play()`); every lighting
  (`WorkSystem._wear_held` with a firestarter) puffs smoke and embers with `sim/ignite`; anvil blows throw 9 sparks,
  knapping 4-5 chips. `craft_film=1` (run with `--fixed-fps 60`) saves burst sheets `film_firestarter`/`film_smithing`
  (`craft_test`) and `film_knapping` (`stone_age_test`).
  **Bronze and iron ages (ADR 0070):** tin laid into a molten copper crucible on the firepit makes bronze (cast or
  smithed, tier 2); a bloomery (`BloomeryEntity`) turns iron ore and charcoal into a hot bloom, only a forge
  (`ForgeEntity`; both are `FirepitEntity` subclasses) heats `forge_heat` items, and only the bronze anvil (`AnvilEntity`
  kind `bronze_anvil`, tier 1) takes metal with `anvil_tier` 1; iron is smithed, never cast (tier 3). Smelt recipes take
  `count` and `hot`. The furnace smelts no ore. Check: `test_metal_ages.gd`, `metal_age_test=1`.
  **Steam age, stage 1 (ADR 0097):** iron plates and rods and bronze pipes are smithed; the small boiler (`BoilerEntity`,
  a firepit subclass) burns coal under a water tank and builds `pressure` (1.0 = working), screams over 1.0 or dry and
  bursts at `boiler_burst_at` or after `boiler_dry_s` dry (`burst` signal: `WorkSystem._on_burst` throws plates out,
  a steam ball, ash and fire, hurts a near player, halved per `steam_proof` clothing). `BoilerEntity.feed` (every 20
  ticks in `BlockEntities.tick`) walks bronze pipes (`PipeEntity`) on one level up to `steam_pipe_max` and sets `boiler`
  on the machines at the ends: the steam hammer (`SteamHammerEntity`, an anvil that strikes by itself, `strike()`) and
  the steam sieve (`SteamSieveEntity`, `loads` and `found`). All three rust (`wear`) and an iron plate mends them. Check:
  `test_steam_age.gd`, `/kit steam`.
  **Stage 2, the turbine and the helpers (ADR 0099):** `BlastFurnaceEntity` (a bloomery that only cooks under steam:
  iron ingot to steel), the steel boiler (the same `BoilerEntity` with `item = "steel_boiler"`: 2.5x the build rate,
  a safety valve at `STEEL_VALVE` that sticks past half wear), `OreCrusherEntity` (a block of ore to two `crushed`
  items), `TankEntity` (feed() lets boilers drink from tanks on the pipes), `TurbineEntity` (spins, powers nothing),
  `CompressorEntity` (fills `air`, a held jetpack takes it). Helpers: the miner's helmet (`TorchLights` lights from the
  worn head piece), the traveler's backpack (`bag_slots`: `PlayerInventory.bag` is rebuilt to the worn bag's size),
  the steam drill (`speed` 2.5), the boiler suit, the jetpack (clothing slot `back`, `Player._worn_with`, air in the
  stack), the surveyor's stake (`waypoint` field). Tier 4 tools are grid recipes. Check: `test_steam_age_2.gd`.
  **Movement toys (ADR 0151):** items with `move_toy` (`glider`, `rope`, `grapple`) belong to `MovementToys`
  (`scripts/player/movement_toys.gd`, a child of Main). `Player._physics_step` calls `toys.step` on foot after the walk,
  gravity, jetpack and ladder and before `_move`; a toy only sets `Player.velocity` (and `_fall_peak_y`), so the body
  still moves through `VoxelBoxMover`. Ropes are `{x, z, top, bottom}` lines saved in the player file (`ropes`), not
  voxels. A toy's let-go speed is carried by `_carry` in the air and becomes `Player._knock` on landing. Player-local:
  other players see none of it. Check: `toys_test=1` (game/README.md).
  **Machines and networks (ADR 0113):** machines are work pieces (`work` on the item, a kind in
  `BlockEntities.create`); every entity with `net(channel) -> {role, amount}` joins `Networks.solve` once a second
  (20 block ticks): the native `NetworkSolver` unions the boxes that touch and moves min(supply, demand, weakest
  conductor) per component, then each hears `net_in(channel, amount)`. Energy in EU/t: the turbine offers
  `turbine_eu_t`, `CableEntity` carries `cable_eu_t`, `MachineEntity` (one class for every LV machine: the item's `machine: {watts, inputs}` and ADR 0115 process
  recipes at its id; the electric furnace is a `FurnaceEntity` subclass). **Real units (ADR 0123):** the energy
  channel is a 48 V DC circuit solved natively (`NetworkSolver.solve_dc`: EMF + internal ohms sources, cable ohms,
  watt loads as conductances, runs of cables collapsed, conjugate gradient); `PowerLoad` decides when a machine joins
  (95 % of 48 V, one start a second per circuit) and stops (85 %); cables burn past their amps; `CircuitEntity` is the
  knife switch, fuse box and breaker; the battery charges or gives by physics; the multimeter and the look-at panel read
  volts, amps and watts. `tools/eu_to_watts.py` converts EU-era data and is re-runnable. **Electrical age tier 1:**
  rubber (tar + sulfur, cured), insulated wire, coated board, vacuum tube, resistor, primitive circuit, machine hull,
  electric motor, electric piston; machines electric furnace, lathe, bending press, wiremill; book chapter
  `electrical_age`. `/kit electric`; `test_networks.gd`, `test_electrical_age.gd`, `tools/network_bench.gd`.
  **Stage 3 transport (ADR 0100):** `Transport.give/pull` is the one place that knows how every piece loads and
  unloads; `HopperEntity` (hopper and chute, `item`) pulls from above (the chute from behind too) and pushes the
  way it faces or down; `BeltEntity` carries `carried` [{stack, t}] one block in `belt_s` and draws them riding;
  `RailEntity` (`axis`, `rise` for slopes, `cart` "minecart" or "chest_cart" with `cargo`, `next_of` up slopes and round
  corners, `height_on`; a pushed chest cart rolls in its own `tick`) and `Player.ride`/`_ride` roll the player along at
  `minecart_speed_m`. Facings: `Transport.DIRS`, laid with `facing_from_yaw`. Check: `test_transport.gd`.
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
  and draws as a vertical water face. **ADR 0103:** a column's vote is its surface connected to the meshed cell's height
  (no drops in the air above it; a run past the reach gives none), and falling water (its run ends in air below) draws
  as full cubes with their side faces. FineGen rivers interpolated per voxel and their trenches and low banks filled;
  `tools/water_surface_probe.gd` counts holes, glass edges, corner seams (`rule=field|ramps`; 0 for field), out-of-reach
  wall corners and the histogram of surface steps. **World water is endless and never simulated (ADR 0104, replacing the finite water of ADR 0068):**
  generated water (`water_static`: rivers, lakes, the sea) sleeps until a fresh opening (air that changed in the last
  two ticks) touches it beside or below; then it *pours*: the opening takes a full cell of moving `water` and the still cell
  stays (`SimWorld::reservoir`). A hole under a river takes a stream until it is full, a breach in the bank fills to one cell
  under the surface and goes on flooding lower ground past it (only the moving front costs: rested water sleeps). A side
  opening is fed only by a cell with still water above it (head), so the surface and stray still cells on it never become
  springs. Cave pools are the exception (bug report 2026-11-07 10-32-53): water and sludge pools on cave floors are generated as the moving kind (`biomes.json` `surface.pool`), so a dug pool drains and is gone; lava pools stay still. Air over a river's own lower step never wakes it; water that rests on a still body spreads flat over it. Lava or
  sand sinking through still water trades places with it and the lifted cell becomes moving water (finite, never a still cell
  on top of a body). **One fluid system (ADR 0119, 2026-11-03):** moving liquids level as *columns* by head
  (`update_liquid_column`: half the height difference between neighbouring columns in whole eighths, a full cell under
  the surface pours a cell into an opening beside it, the 4c reach rule flattens the rest; deterministic order; a spill
  ends and its chunks sleep, where the eighth kernel sloshed for ever; `column_flow=0` runs the old kernel). Pastes are
  box-sized (`get_dirty_boxes` -> `get_box_bytes` -> `World.write_box`). Heat lives on a 4^3 coarse grid (`cell_info`
  reads it), lava / fire / burning cells heat it, it relaxes toward `Survival.outside_c`; pressure is gas over open cells
  per chunk; materials carry `heat_c`, `boil_c` + `boil_rise_c` x pressure, `boils_to`, `condense_c`, `freeze_c`,
  `melt_c` (water <-> steam <-> one-eighth drops, sludge <-> toxic gas 254, ice <-> water; still liquids never switch).
  Gas looks: `data/gas_looks.json` (`gas_tint`, `gas_alpha`, `rise_speed`, one gas material copy per gas). The minimap's
  last line shows the aimed cell's material, state, temperature and pressure. Bench `tools/fluid_bench.gd`, tests
  `test/test_sim_0119.gd`. **Fluid lag (ADR 0137):** the outside temperature is floored and a loaded chunk's heat starts at
  it (generated ice no longer melts on load; Survival measures again after a 32 m move), a cold window wakes only chunks
  that may hold a liquid that freezes or a gas that condenses, a liquid
  substep scans only chunks with moving fast liquid in or beside them, the pool wakes as many helpers as there are
  chunks, warm ice thaws from its open faces a few cells per 64-tick heat turn (`thaw_`), and column levelling gives
  nothing to a full top under a roof (it woke water filled to a hole's roof for ever); `fluid_lag=lake|river|bomb|pour|fly` times a change to its remesh and rest (`debug fluid_lag` JSON).
  **Water feel (ADR 0138):** resting moving water on ground beside a body (2..7 eighths) is still an opening, so a dug
  pit or channel fills to the body's level (water resting on a lower still step is not); the surface row feeds a dig
  deeper than the surface (a trench in a shelving bank) but never air over the shore; a ledge onto a partial cell pours
  what fits and a lone eighth on bare ground walks to the nearest drop (films run off slopes); the 4c rule is a conveyor
  (looks FLAT_REACH 14 cells, also a row down, writes one cell over); terrace edges whose look found nothing seed a
  serial level pass after the tick (`SimWorld::level_surfaces`: the connected resting surface, tilted 2..8 eighths,
  gives an eighth from its highest columns to its lowest), so a flood rests flat to an eighth. Falling liquid also draws
  as streaks (`scripts/visuals/fall_streaks.gd`, one MultiMesh per liquid in its own material, warmed under the loading
  screen): `SimWorld.get_fall_streaks` is the air the falling cells crossed in the last 3 ticks, merged per column, so
  the mesher's falling cubes (ADR 0103) join into one stream; `streaks=0` turns them off. `fluid_film=1` films the
  scenes (`scripts/visuals/fluid_scenes.gd`; `fluid_full=` full-size shots, frame-time p50/p99 per scene),
  `tools/fluid_feel.gd` runs them on the bare kernel, `tools/fluid_stream_sheet.py` puts consecutive frames of two runs
  side by side, tests `test/test_sim_0138.gd`. **ADR 0081:** a lone still cell
  (none of its kind on its six sides: the specks the generator leaves on cave walls) wakes at any air beside or below it;
  reactions of one pair chain (`Reaction::next`, rolled in data order): lava + water boils the water to steam at 0.6 and
  only then crusts to obsidian at 0.25, so the steam grows with the water the lava touches.
  **Generated liquids (ADR 0069 C):** every pool and sea is a still kind (`water_static`, `toxic_sludge_static`,
  `lava_static`; the deep lava sea too); FineGen drops lone still cells (air above, none of its kind beside, air beside);
  river surfaces interpolate with plain bilinear weights and blend every stream segment in reach by distance, so
  they are continuous; a drop of three or more cells between neighbouring surface cells is a cascade where the bed
  drops as much (the river's surface is clamped to the ground), and `water_surface_probe.gd` counts the rest as
  terraces; `tools/column_probe.gd` prints a column's hydrology. GEN_VERSION 22.
  **Liquid speed (ADR 0069 B):** `liquid_speed` substeps per material (water 8 since 2026-10-27, cap 8, oil, blood, sludge 2, lava 1): after
  the main pass the kernel runs extra passes that only spread and pour moving liquids with substeps left (no falling,
  no downhill run, nothing standing on a falling cell; its own liquid that only evened out underneath does not hold it); `mark_` is the per-pass move marker, `stamp_`
  still counts ticks. A surface evens out by half the difference to a lower neighbour (ends flat), rests at a
  difference under two eighths; a cell with no neighbour two eighths lower sends one eighth to one up to 7 cells along the
  surface (`SURFACE_REACH`), so a surface ends flat instead of tilted an eighth per cell. `test/test_liquid_speed.gd`. `water_test=1 target=river` checks a
  trench and a crater; kernel tests `test/test_sim_v6.gd`. A still body beside a dug hole (2026-10-27) also wakes at its
  own moving water two eighths lower beside it, and at a river step whose lower cell has moving water beside it (a
  drained bank's edge), so it no longer sleeps as a 45-degree wall with the hole a third full (`tools/water_bench.gd`).
  **Gases and flames (2026-10-27):** they move through air (the displace table never let density-0 cells enter air, so
  smoke, steam and flames sat where born); a `lifetime` past 255 ticks counts down every other tick (smoke 330 = 11 s).
  Tall grass and ferns catch at 0.6 and burn 2 s, so a meadow or forest floor fire keeps going while touching tufts
  connect; it dies out at ~20 % cover, on bare ground, at water and at the sim window (`tools/fire_bench.gd` forest_floor). What still moves when the window walks on finishes in a pin (ADR 0050).
  No waterfalls (the sheet, spray, roar and landmark are gone, GEN_VERSION 21): a river just drops over its cliff; the
  waterfall loop hears the water that fell each tick (`SimWorld.get_falls`, ADR 0103).
  **Every liquid looks like water (2026-10-25):** one fluid shader body (`terrain_fluid.gdshaderinc`), no atlas texture;
  each liquid's character (colours by depth, rim, ripples, gloss, glow, oil sheen, sludge bubbles) is
  data in `data/liquid_looks.json`, set per fluid material by `World.apply_liquid_look` (`test_liquid_looks.gd` checks
  every key is a uniform); lava is Minecraft's matte glowing orange with dark blotches in half-voxel pixels (ADR 0092). `liquids_test=1` pours a pool of each and shoots them by day and night.
  **Items burn in lava:** a dropped stack touching a liquid with `temperature_of_fire` burns up with `item_burn` +
  `item_burn_smoke` and a sizzle (`ItemEntity.burns_at`); items tagged `fireproof` (obsidian, lava bucket) survive.
  **Buckets (ADR 0091):** a bucket holds `Settings.bucket_m3` (1 m^3 = 512 voxels, `Bucket.capacity`). An empty one
  scoops the connected cells of the liquid the aim ray crosses (`Bucket.scoop_cells`, whole cells); a filled one keeps
  `fill` in its stack data (missing = full) and pours that many cells into the free space at the aimed face, never
  above it, lowest first (`Bucket.pour_cells`). Written as plain liquid; the simulation is untouched.
  **Setting things down (ADR 0095):** sneak + right click lays a held `tool` flat on a top face (`laid_tool`) or leans it
  on a wall (`leaning_tool`), and an `ingot` starts a pile (`ingot_pile`, up to `Settings.ingot_pile_max`, held ingots add one);
  an empty hand takes it back (`LaidEntity`, `test_laid_items.gd`, `showcase=copper_axe,copper_ingot`).
  **Held right clicks (ADR 0043):** ground pickups are whole pieces (`GroundPickups`): aimable (aim-only box), a dig takes the
  whole stick, a 1 s held right click pops it into an item on the ground; firepit moves are held too (`hold_key`/`hold_done`).
  **Sound (ADR 0044):** the `Sound` autoload (`GameAudio`, `scripts/audio/`) plays `data/sounds.json` (material groups,
  bursts, mobs, sim loops, music) from CC0 assets cut by `tools/make_sounds.py` out of `~/Tools/downloads/sounds/` (recipes
  in `tools/sound_recipes.json`, ffmpeg in `~/Tools/ffmpeg/`). It listens instead of being called: player signals,
  `ParticleFx.burst_played` (names in `fx.sound_bursts`), `FallingPieces.piece_event`, every UI button via `node_added`;
  call `Sound.play(name, pos)` only where nothing else tells the story. Buses Music / World / UI; JSON "sound"; `sound=0`.
  Sim loops (ADR 0081) key on `sample_cells` flag bits: `"where": "moving"` (the cell moved this or the last tick: sand,
  gravel, earth and snow loops, one asset at a `pitch` and `db` per loop) and `"fresh"` (a gas just born: the sizzle of
  steam off lava).
  **Four Stone Age stages (ADR 0059):** the book's Old, Middle, New and Copper Stone Age chapters; `StoneAge`
  (`scripts/work/stone_age/`, hooked first into the player's right click as `item_use`) runs earth ovens (tar, tempered
  flint), racks (dry/smoke), stroke work (polish, quern, sew, loom), water work (retting, traps), canoes, megaliths and
  circles, cave paintings, clothing; items/recipes in `data/stone_age_*.json` (new types bury, rack, stroke, soak, trap;
  slow work in calendar game hours via `StoneAge.now_days`); per-stack `dur_x` for tar/tempered tools. Scripted check:
  `stone_ages_test=1` (with `walk=1 speed=0 mob_spawn=0`; `flint_age_test=1` runs it too).
  **Brush shapes and sizes (ADR 0065, 0073):** cube, sphere, cylinder, `flat` (a square one voxel deep in the aimed
  face's plane), `column` (size/2 wide, 2 x size long along the face normal: a shaft, tunnel, pillar or beam) and
  `tunnel` (ADR 0091: a 1 m x 2 m hallway from the feet, `Brush.tunnel_floor_y`, size/2 deep per dig, on the block
  grid; a shaft on floors; digs only). The brush ghost shows only where the brush works (`Player.brush_ghost_fits`:
  terrain, plants, liquids; not stations, work pieces, furniture). The
  box shapes are flush with the aimed face (into it digging, out of it placing) and centred on the aim across it, on
  the 0.5 m block grid from size 4 up (`Brush._face_box`); the round ones go around the aimed voxel / rest on the face. Survival
  sizes 1-8 voxels, digging capped by `MiningTable.dig_cap` and paid in time per voxel and a tool use per block; creative
  adds 16 and 32 (4 m), reaches 12 m and paints with Shift + right click (the dig shape takes the held block).
  `BrushTool` (`scripts/player/brush_tool.gd`) draws the shell of the exact cells (orange dig, blue placement; box
  shapes as one scaled shell, round ones half resolution above 8) and runs brushes above 8 as `ShapeTool.fill` jobs of cached relative cells, 6000 cells a frame;
  big creative digs drop nothing. HUD line right of the hotbar; `/brush <shape> [size] | hold <s>`; costs per shape and
  size: `tools/brush_bench.gd`.
  **TNT (ADR 0065):** material `tnt` (203), a voxel item placed with the brush; a charge is one 0.5 m block cell holding
  TNT, its blast radius `tnt_radius_m` x cbrt(its TNT share). `Tnt` (`scripts/items/tnt.gd`) lights charges touched by
  fire or lava, aimed at with flint and steel or the `igniter` (sneak: every charge in 16 m), or reached by a blast
  (random 0.5-1.5 s fuse); a primed box blinks, hisses, sparks and falls for `tnt_fuse_s`, then three native spheres by
  hardness (soft full radius, stone 3/4, hard rock 1/2, obsidian and bedrock never), rim seeds to the rigid piece check,
  damage and knockback to 2 x radius, 30 % of the estimated take as drops. One blast per frame; a 20-block chain holds
  max frame 12.4 ms. `/tnt grid [n] [spacing_m] | light | stats`, `/kit tnt`; JSON `tnt`.
  **Fuse cord and debris (ADR 0150):** `fuse_cord` (137) burns along itself at `Settings.fuse_speed_mps` by the kernel's
  fire rule (ignite_chance from the speed), its fire too cold for anything else and its flame the cold `fuse_spark` (138);
  `Tnt._cord_burning_at` lights a charge a burning cord touches. `BlastDebris` throws up to `tnt_debris_max` 2^3 chunks
  per blast that land as loose voxels (rock as gravel). `/tnt fuse [m]`, `/scenario fuse`, `test/test_fuse_cord.gd`.
  **Mine shoring (ADR 0142):** a roof cell over a dug 0.5 m block must lie within its rock's span of a wall
  (`1 m + 1 m x mining hardness`, max 4 m: stone 2.5 m, sandstone 1.8 m; a 3 m tunnel always stands) or within
  a prop's radius (timber prop 1.5 m / 4 m2, stone pillar 2.5 m / 12 m2, iron beam 3.5 m / 30 m2: furniture kinds
  with `"prop"`, stretched floor to roof, `height` in the save); any solid column floor to roof is a wall. Native
  (`RigidPieces.support_*`, `src/rigid_support.cpp`: Dijkstra over the roof surface, on the driver thread), event
  driven: digs mark blocks (`Main._on_dug`, crater rod, TNT, box dig; never the laser) and `MineSupport` scans the
  dug box every 0.25 s. Unheld roofs crack and darken, shed dust and grit, creak louder and shake the camera at the
  end, for 15-120 s (`warn_s`); overloaded props bend; then the roof falls as one rock piece (`support_drop`) and
  hurts what is under it. Survival only unless `/support modes all`; marks (per chunk) and creaking roofs are saved
  in `support_<seed>_g<gen>.bin`. `/support`, F3 "Support:", JSON `support`, `mine_test=`.
  **Next:** the rest of the ADR 0037 work order; still open from before: fire/smoke pacing per cell, fast flight vs
  generation, the water spill / blast / `persist=1` runs; plan in `docs/plans/lotl-overhaul.md`, playtest
  list in `docs/plans/to-test.md`.
- Material ids are definition order, append only (saves and the generator use them): 32..36 stations,
  37..43 detail, 44 reserved (once `CHISELED`), 47 blood, 48..52 ground pickups, 57..75 animals, farming,
  furniture, 76..87 and 90..101 content passes, 88..89 work spots, 103..107 Flint Age, 108 leaf litter,
  109 cobblestone, 110 bricks, 111..115 clutter (plant stem, kelp, seagrass, seashell, mushroom stem; ADR 0047),
  116..119 liquids (part A), 120 crystal (geodes, ADR 0052), 121 lava_static (ADR 0069 C), 122..123 reserved for world generation, 124..126 giant flower stem, petal, heart (meadow, 2026-11-02), 127..129 savanna grass,
  savanna tall grass, red earth (ADR 0064; `GameMaterials.is_grass` / `is_tall_grass` treat them as grass), 130..132 torch and
  wheat parts (ADR 0053), 137..138 fuse cord and spark (ADR 0150), 160..202 rock kinds, clutter, pyrite and ochres (ADR 0057), Stone Age ids after (ADR 0059), 203 tnt (ADR 0065), 204..206 monolith (ADR 0098), 207..210 spruce/acacia/jungle/birch planks and 211..215 saplings (ADR 0112: `Saplings` grows them into trees, `data/saplings.json`), 218..221 fouled water, waste ground, slag, chimney (ADR 0141; 222.. reserved up to the cave ids).
  **Pollution (ADR 0141):** machine waste is cells: recipe `"waste"` and item `"firebox"` fill a saved buffer that
  `Waste.emit_all` (once a second) pours out of the port (back face; gases over the top or a chimney stack; liquids
  into a touching pipe run's intake or far end); a full buffer stalls the machine. Sludge fouls water; fouled water
  dilutes only against still water; sludge erodes soil and settles on gravel through `"passive"` reactions (the ground
  stays inert, only `"react_awake"` sludge and fouled water keep their chunk awake while they touch a partner).
  `SimWorld.set_wind` drifts `wind_drift` gases (smoke), not under a roof (12 m scan up). `/stations pollution`
  and `/stations smoke` show it.
  **Consequences (ADR 0155):** fish in fouled water build `Mob.foul_s` (gasp at the surface, then die belly-up and
  float); crops beside sludge/waste ground, watered foul or under heavy smoke turn sickly (`Farming.pollution`, saved
  `sick`, slow growth) and wilt to dead bushes, clean ticks heal; dense smoke round the head (native count) makes the
  player cough, then hurts, unless a `smoke_mask` piece is worn (cloth mask, diving mask). Numbers in `settings.gd`;
  `/pollute [smoke]` shows it.
  Atlas 8x23 tiles (16..19 ores, patterned since ADR 0064; 104 crystal, 109..111 savanna grass, tall grass, red earth, 112..120 crops and torches, 136..170 rocks, 171..172 tnt side and top, 173..175 monolith, 176..179 per-wood planks (row 23, 180..183 free); 190 waste ground, 191 slag, 215 chimney top (ADR 0141); 84 leaf litter, 85 oak bark, 86 pebbles, 87 flint, 93/97 twigs top/bark, 94..96 ore-bearing
  stones; 98 cobblestone, 99 bricks; 100 blueberry bush; 101 kelp, 102 seagrass, 103 seashell: full, add a row to grow;
  after editing `tools/make_atlas.py` run `--import`, or the game draws the old texture). `"pattern": true` materials lay their tile over the grid, one texel per voxel (ADR 0039). Ground pickups are voxel shapes (stone chunks on a 0.5 m patch, sticks as `FineGen::STICK` stamps);
  clutter (flowers, mushrooms, glow mushrooms (a `mushroom_stem` under a cap of emissive `glow_mushroom` 45, grown by `FineGen::cave_dressing` on a 4x4 voxel lattice), seashells, kelp, seagrass) and ore nuggets are voxel pieces that dig whole (ADR 0047,
  `GroundPickups`, probe `tools/clutter_probe.gd`). Crafting: any layout, amounts in inventory counts (ADR 0046). `MicroMesher` builds the
  models of stations, furniture, work pieces, mobs and the ape at 6.25 cm cells; it winds triangles
  counter-clockwise, so pass its meshes through `MobModel.outward` (which reads an ArrayMesh back and
  waits 4-6 ms for the render thread: never per edit). Stations (table, chest, furnace) are 1 m models
  from `StationEntity.model_mesh(name)` with footprints in `StationEntity.SIZES`; every cell model goes
  through `WorkPalette.shade_cells` (tones per cell, darker convex edges, ADR 0076); check with
  `tools/station_sheet.tscn`.
