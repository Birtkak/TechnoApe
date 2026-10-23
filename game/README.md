# TechnoApe – game project (Godot 4.7)

The real project. Prototypes live in `../prototypes/`; decisions in
`../docs/decisions/`; the vision in `../docs/design/vision.md`.

## Run

```
# play (mouse captured; Esc releases; F fly/walk; F3 debug overlay; hold LMB to mine; RMB place/use;
#       1-9 / wheel select; E inventory; Q drop one (Ctrl+Q stack); RMB on a crafting table / furnace /
#       chest opens it, Shift+RMB places against it; in any inventory screen the recipe browser sits on the
#       right: LMB recipes, RMB uses, R/U over any item, Backspace back, Esc closes the recipe view)
# chat and commands (ADR 0018): T chat, / command (Tab completes, Up/Down history), /help lists them;
#       /gamemode creative|survival (creative: F or double-tap Space flies, instant break, endless blocks,
#       middle click picks a block, LMB on the recipe list takes a stack); F3 Minecraft-style debug screen;
#       F4 (or the Debug button top right) playtest menu with saved start options; F9 bug report; F2 screenshot.
#       Playing from the editor starts minimized (turn off in F4 start options or with minimized=0).
#       Bug reports land in game/out/bug_reports/<time>/ (report.md, state.json, screenshot.png, log.txt).
Godot_v4.7.2-stable_mono_win64_console.exe --path game

# unattended check for Claude: fly forward 20 s, filmstrip every second, JSON on stdout
Godot_v4.7.2-stable_mono_win64_console.exe --path game -- autopilot=1 duration=20 capture_every=1 vsync=0 out=res://out/run1
# any Settings value can be overridden: render_distance_m=64 view_distance_m=1024 lod_count=8 sim_tick_hz=30 ...
# park at a feature and look down: target=lake|ruin|spring|river|waterfall|marsh|alpine|snow|mountain|<biome> speed=0 offset=15 alt=20 pitch=-0.7
# walk instead of fly (gravity + 1-voxel auto step-up): walk=1 ; JSON reports travelled_horizontal_m
# pick a world: seed=1337 or seed="any text" (hashed); see docs/design/seeds.md
# the JSON ends with bubble_verify: simulation bubble vs generator (0 mismatches expected)
# interaction checks (ADR 0005): give=... fills the hotbar; mine=1 digs a 2x2 shaft under the feet
# (JSON: picked_up, item_stats, player.hotbar); place=1 places the held block ahead every 0.2 s;
# scatter_items=1 drops one entity per given item in front; open_inventory=1 for a screen shot
Godot_..._console.exe --path game -- autopilot=1 mine=1 duration=20 capture_every=2 give=wooden_pickaxe out=res://out/mine
Godot_..._console.exe --path game -- autopilot=1 walk=1 speed=0 duration=4 open_inventory=1 give=oak_log:12,cobblestone:64,bucket:2 out=res://out/inv
# station blocks (ADR 0007): stations=1 places a crafting table, chest and furnace ahead (furnace smelts 8 iron
# ore on 1 coal, JSON "blocks"); open=crafting_table|chest|furnace right-clicks that one at 60 % of the run
Godot_..._console.exe --path game -- autopilot=1 walk=1 speed=0 stations=1 open=furnace duration=24 capture_every=2 out=res://out/stations
# recipe browser (ADR 0008), with open_inventory=1: browse=iron_pickaxe | browse=oak_planks:uses, search=#metal,
# hover_test=1 (mouse over a cell + R + RMB through real input; prints the result); JSON "browser"
Godot_..._console.exe --path game -- autopilot=1 walk=1 speed=0 open_inventory=1 browse=oak_planks:uses duration=4 out=res://out/browser

# bubble / fire (ADR 0010): JSON sim.worst_frames (bubble cost per frame) and sim.slow_frames (frames > 6 ms with
# Godot monitors); fire_test=N ignites burnables within N voxels of the player once loaded
Godot_..._console.exe --path game -- autopilot=1 walk=1 speed=0 target=forest fire_test=14 duration=40 out=res://out/fire
Godot_..._console.exe --headless --path game --script res://tools/fire_bench.gd   # fire pacing + cost; set=tall_grass.fire_hp:40 only=grass_field

# budget check (ADR 0009, 0014): every autopilot JSON has budget_ok + budget{p99,max,ram,vram,loaded}, judged on
# `duration` s of wall-clock frames after the world settled (load_s: bubble loaded, voxel tasks idle 30 frames;
# load_settled=false if the 10 s cap hit first); frames after a filmstrip readback are excluded (capture_stalls);
# rss_mb is the process working set; spikes[] splits frames > 8.3 ms into process/physics/render cpu+gpu/other;
# other_godot_processes > 0 means another Godot run shared the machine: rerun before trusting the numbers
# frame cap: play runs at most 120 fps (project.godot run/max_fps); vsync=0 runs lift it (max_fps=N sets another cap)
# render thread (ADR 0016): the project renders on a separate thread (Godot prints an "experimental" warning
# and a "finalize ... render thread" error on exit: both expected); for one run in the old mode add --render-thread safe
Godot_..._console.exe --path game --render-thread safe -- autopilot=1 duration=20 out=res://out/safe
# commands=/gamemode creative;/give oak_log 64 runs chat commands at start (JSON: game_mode, chat); chat_text=/give oak
# opens the chat with that typed; debug_menu=1 opens the F4 menu; f3=1 the debug screen. In Git Bash set
# MSYS_NO_PATHCONV=1 or it rewrites /arguments into Windows paths.
Godot_..._console.exe --path game -- autopilot=1 walk=1 speed=0 duration=3 f3=1 "commands=/gamemode creative;/kit tools" out=res://out/chat
# liquid spill stress: blast=1.5 blast_material=water blast_every=2 (passes for ~20 s; minutes of flow do not yet)
# far view + streaming (ADR 0017): JSON voxel_backlog = pending voxel tasks while moving (max/mean/busy_share/
# max_busy_s/threads); voxel_threads=N overrides the worker count; persist=1 seed=<unused seed> measures the
# save-file load path (never use the seed you play); lod_fade_s=0 turns the LOD cross-fade off (default 0.35 s, ADR 0020);
# lod_ring_m=24 narrows the LOD rings back to godot_voxel's default (LOD n ends at render_distance_m + lod_ring_m * 2^n, ADR 0020);
# terrain_threaded_update=1 moves VoxelLodTerrain's LOD bookkeeping to a worker thread;
# load_cap_s=120 waits longer than 10 s for the first load to settle (slow horizons); the JSON's
# voxel_backlog.peak_by_type tells a generator backlog from a meshing one; every spike lists phys_cost (ms per
# physics callback: player and its sub-steps, items, item_bodies, blocks)
Godot_..._console.exe --path game -- autopilot=1 speed=20 alt=30 duration=16 vsync=0 persist=1 seed=424242 out=res://out/stream
# ray-cast near field (ADR 0012 step 3, ADR 0021): near_field=1 draws the cube around the player per pixel from the
# brick world at 6.25 cm (near_field_m=32 -> 512^3 cells, 128 MB VRAM; 16 -> 256^3), terrain meshes inside it are hidden;
# near_upload_mb=8 caps the volume bytes uploaded per frame; near_debug=1 tints the ray-cast pixels magenta (where does it
# draw?); near_verify=1 reads the GPU volume back once the window is uploaded and prints a JSON line comparing it with
# the store (bad must be 0). JSON "near_field": uploads, mb, pending slabs/chunks, valid box, store stats
Godot_..._console.exe --path game -- autopilot=1 walk=1 speed=0 duration=6 near_field=1 near_debug=1 capture_every=0.5 out=res://out/near
# micro simulation (ADR 0029): micro_sim=1 (implies near_field=1) simulates 32 x 16 x 32 m around the player at 6.25 cm
# cells (the window sits low: 6-14 m of it below the feet); JSON sim.micro: tick ms p50/p99/max, scanned cells, publish
# ms and queue, loads, shifts, edit_cells. micro_cell_budget, micro_lod_m, micro_publish_budget_ms, micro_load_budget_ms
Godot_..._console.exe --path game -- autopilot=1 speed=0 alt=6 pitch=-0.45 micro_sim=1 blast=1.5 blast_material=water blast_every=4 capture_every=0.33 full_frames=1 out=res://out/micro_water
# mass edits (ADR 0014): blast=R craters of R m on the ground around the player every blast_every s (default 2),
# blast_count=N per volley; JSON sim.blasts, blast_voxels, blast_ms_max (native SimWorld.fill_sphere)
Godot_..._console.exe --path game -- autopilot=1 speed=0 alt=20 pitch=-0.6 blast=4 blast_every=1 duration=20 capture_every=1 out=res://out/blast
Godot_..._console.exe --path game -- autopilot=1 speed=0 alt=20 pitch=-0.6 blast=1 blast_count=50 duration=20 out=res://out/tnt

# generator probes (headless): stats + biome map; settle test around a feature
Godot_..._console.exe --headless --path game --script res://tools/gen_probe.gd -- seed=1337 radius=2048
Godot_..._console.exe --headless --path game --script res://tools/settle_probe.gd -- target=lake ticks=60
Godot_..._console.exe --headless --path game --script res://tools/seam_probe.gd -- seed=1337   # rivers across hydrology tile borders
Godot_..._console.exe --headless --path game --script res://tools/detail_probe.gd -- seed=1337 radius=96  # micro-voxel detail placement checks
Godot_..._console.exe --headless --path game --script res://tools/chisel_bench.gd                 # MicroMesher sanity + timings (P6)
Godot_..._console.exe --path game --script res://tools/model_sheet.gd -- out=res://out/models/sheet.png  # renders every micro model
Godot_..._console.exe --path game --script res://tools/anim_probe.gd                       # terrain shader: animated tiles change, others do not
Godot_..._console.exe --headless --path game --script res://tools/shore_probe.gd -- seed=1337   # hydrology flags along a lake waterline
Godot_..._console.exe --path game -- autopilot=1 duration=8 capture_every=2 target=cave inside=1 speed=0.6 out=res://out/cave1  # inside a cave, with a torch (deep_cave: 50-170 voxels down)
Godot_..._console.exe --path game -- autopilot=1 duration=10 capture_every=1 target=waterfall alt=4 offset=10 speed=0.4 out=res://out/fall_spray  # waterfall with spray; read strip.png for the motion
Godot_..._console.exe --path game --script res://tools/ape_sheet.gd -- out=res://out/ape/sheet.png     # player body from front/left/back + head close-up
Godot_..._console.exe --path game -- autopilot=1 duration=6 capture_every=0.5 walk=1 third_person=1 out=res://out/ape_walk  # third-person walk cycle (2 = camera in front)
# look / atmosphere (scripts/visuals/atmosphere.gd): time=0..1 (0.25 sunrise, 0.5 noon, 0.75 sunset; default 0.40),
# day_length=<seconds per day, 0 = frozen>, clouds=0..1, ssao=0|1, glow=0|1, fog=0|1, shadows=soft|hard|off, tonemap=aces|agx|filmic|linear, saturation=1.08, fxaa=1
# cost on the dev box (forest, 1280x720): defaults ~+1.2 ms/frame over bare (p99 3.4 ms walking); soft shadows ~0.5 ms, SSAO ~0.1 ms, glow ~0.05 ms
Godot_..._console.exe --path game -- autopilot=1 target=forest walk=1 speed=0 duration=5 capture_every=3 time=0.76 out=res://out/sunset
# animals, farming, furniture (ADR 0022): animals=N spawns N sheep and pigs ahead once loaded (animal=sheep|pig only that one,
#       animal_babies=1 every third a baby, animal_love=1 adults start in love and breed); hold their food (give=wheat:8) and
#       they follow; mob_spawn=0 stops natural herds; JSON "mobs" (active, goals, avg/worst frame ms, stats: herds, bred, died)
# farm_test=1 sows a 9x9 field around a water voxel ahead, farm_speed=N runs farm random ticks N x faster; JSON "farm"
# hands_test=1 tills, sows, places a chair and a bed, sits, sleeps, feeds, shears, kills a sheep and eats through the
#       player's own input code; prints "debug hands {...}" (every field should read true / placed)
# chat: /summon <sheep|pig> [count] [baby], /kit farm (hoe, seeds, carrots, wheat, shears, bed, chairs, wool)
# Vintage Story early game (ADR 0023): stone_age_test=1 knaps a flint axe head, forms a crucible and a pickaxe mold,
#       fires them in pit kilns, melts copper-bearing stones in a firepit, casts a copper pickaxe head and sifts gravel
#       through the player's own clicks; prints "debug stone_age {...}" and saves stone_age_*.png per stage
#       (stone_age_shots=0 skips the screenshots, which stall the render thread ~100 ms, for a budget run);
#       chat: /kit stone (flint, sticks, clay, dry grass, stones, copper-bearing stones, firestarter, logs)
#       piece timings: JSON work.rebuild_ms_max / use_ms_max / hit_ms_max; headless: res://tools/work_probe.tscn
# Deeper Flint Age (ADR 0030): flint_age_test=1 scrapes and dries a hide, digs, stacks, lights, seals and burns a
#       charcoal pit (and lets an open one burst into flames), pans gravel in a pool, throws a spear at a sheep,
#       places torches (lights at night), thatch, wattle and daub, a straw bed and a storage pot, wears a bag and
#       cooks stew; prints "debug flint_age {...}" and saves flint_age_*.png (flint_age_shots=0 skips them);
#       chat: /kit flint; progression book: open_inventory=1 quest_book=1 [quest_select=id] [quests_done=a,b|all]
Godot_..._console.exe --path game -- autopilot=1 walk=1 speed=0 duration=40 stone_age_test=1 mob_spawn=0 vsync=0 out=res://out/stone_age
Godot_..._console.exe --path game -- autopilot=1 walk=1 speed=0 duration=90 flint_age_test=1 mob_spawn=0 vsync=0 out=res://out/flint_age
# Menus (ADR 0032): the title screen is the main scene; autopilot=1 or play=1 skip it (play=1 seed=... starts that
#       world directly). menu_test=1 drives title -> new world -> settings -> play -> Esc pause -> preset applied live ->
#       save and quit -> reload (inventory and position kept) -> delete; prints "debug menu_test {...}".
#       graphics=low|medium|high|ultra picks a graphics preset for a run; frame_parts in the JSON says where a frame goes
Godot_..._console.exe --path game -- menu_test=1 minimized=0
# Windows build for playtests (export_presets.cfg): a debug-template export, because the sim and
#       voxel extensions are built and measured as template_debug (ADR 0020). Output: build/windows/TechnoApe.exe
#       (+ .pck and the two DLLs; TechnoApe.console.exe shows the log). The first launch on a machine compiles
#       shaders once (one ~60 ms frame); later launches hold the budget. The exe keeps worlds, settings and
#       F9 bug reports in %APPDATA%/Godot/app_userdata/TechnoApe/ (bug_reports/), the editor in game/out/
Godot_..._console.exe --headless --path game --export-debug "Windows Desktop" ../build/windows/TechnoApe.exe
Godot_..._console.exe --path game -- autopilot=1 walk=1 speed=0 duration=15 animals=40 mob_spawn=0 give=wheat:8 vsync=0 out=res://out/animals
Godot_..._console.exe --path game -- autopilot=1 walk=1 speed=0 duration=18 capture_every=1 hands_test=1 mob_spawn=0 out=res://out/hands
Godot_..._console.exe --path game --script res://tools/mob_sheet.gd -- out=res://out/mobs/sheet.png   # sheep, shorn sheep, lamb, pig, piglet, bed, chair
Godot_..._console.exe --headless --path game --script res://tools/wild_crop_probe.gd -- seed=1337 radius=1024   # wild wheat / carrot patches per km^2
# stains (ADR 0015, scripts/visuals/stain_map.gd): stain_test=1 bleeds and drops oil + blood next to the player once loaded;
# stains=0 turns stains off (same spill, for A/B), stain_uploads=N layers per frame, stain_scan=0 ignores the simulation
Godot_..._console.exe --path game -- autopilot=1 walk=1 speed=0 third_person=1 pitch=-0.75 stain_test=1 duration=14 out=res://out/stains
```

First run after adding scripts/addons: `--headless --path game --import` (twice if the
voxel extension segfaults once).

```
# crafting / inventory / recipe-browser tests (gdUnit4, headless, exit code = result)
Godot_v4.7.2-stable_mono_win64_console.exe --headless --path game -s res://addons/gdUnit4/bin/GdUnitCmdTool.gd --add res://test --ignoreHeadlessMode
```

Crafting core (`scripts/crafting/`, data in `data/items.json` + `data/recipes.json`):
`ItemRegistry` (tags, search), `RecipeRegistry` (shaped with translation +
mirroring, shapeless, smelt, spellcraft; `makes`, `uses_of`, `search`,
`by_station`, `reachable_from` for the Too-Many-Items browser), `Inventory`
(Minecraft cursor semantics: left/right/shift click, stack limits),
`CraftingGrid` (preview, craft once/all, return contents). Wands have no
recipe by design (found only); spells do.

## Layout

| Path | What |
|---|---|
| `scripts/settings.gd` | Autoload. Every budget is a setting, in metres. One world unit = one voxel = 0.5 m (ADR 0004). |
| `scripts/materials.gd` | Material table (Noita attribute names) + atlas tiles; builds the blocky library. |
| `scripts/generator.gd` | Bridge to the native `WorldGen` (1.18-style splines, caves, ores, biomes); see `docs/design/world-generation.md`. `tools/gen_probe.gd` prints stats and a biome map. |
| `scripts/world.gd` | `VoxelLodTerrain` (clipbox streaming, 7 LODs) + copy/paste helpers. |
| `scripts/sim_bubble.gd` | Sliding window of the native `SimWorld` around the player; load/tick/push. |
| `scripts/player.gd` | First-person walk/fly/swim against the voxel grid (`scripts/physics/voxel_body.gd`, ADR 0011), mouse look, hold-to-mine with progress, place/use (buckets), hotbar keys, drop, fall damage, held-item viewmodel (`scripts/player/viewmodel.gd`). |
| `scripts/player/player_inventory.gd` | Player model: 36-slot inventory (hotbar = slots 0..8), cursor stack, 2x2/3x3 crafting grid, health. Pure logic, tested. |
| `scripts/player/mining_table.gd` | `data/mining.json`: hardness, tool class/tier, drops per material (Minecraft break-time formula). |
| `scripts/items/item_entity.gd` | Dropped stacks: textured mini-cubes moved by VoxelBody (gravity, vanilla drag, sleep), bob/spin, merge, pickup, despawn; pop out of solid voxels. |
| `scripts/ui/hud.gd` | Crosshair, hotbar with isometric atlas icons (`item_icon.gd`), hearts, break progress. |
| `scripts/ui/inventory_screen.gd` | Vanilla-layout inventory, crafting table, chest and furnace screens with cursor semantics. |
| `scripts/ui/recipe_browser.gd`, `recipe_browser_view.gd` | JEI-style browser (ADR 0008): search, recipes/uses by category, history; the view beside every inventory screen. |
| `scripts/blocks/` | Block entities by voxel position (ADR 0007): `ChestEntity`, `FurnaceEntity` (vanilla tick rules), `BlockEntities` store, saved to `user://worlds/blocks_<seed>_g<gen>.json`. |
| `scripts/debug_hud.gd` | F3 debug screen after vanilla's (ADR 0018): facts left, system + targeted block right, axis crosshair, frame graph. |
| `scripts/ui/chat_box.gd`, `scripts/commands/` | Chat (T, /) and slash commands (ADR 0018): `CommandRegistry` (pure: parse, dispatch, complete), `GameCommands` (the commands). |
| `scripts/player/game_mode.gd` | Survival / creative rule sets the player, HUD and inventory consult. |
| `scripts/ui/debug_menu.gd`, `scripts/debug/bug_report.gd` | F4 playtest menu (saved start options in `user://playtest.cfg`), F9 bug reports, F2 screenshots. |
| `scripts/debug_capture.gd` | Autoload. Autopilot, filmstrips, JSON stats, vsync switch. |
| `addons/technosim/` | Our GDExtension (built from `../prototypes/p5-sim-spike/src`). |
| `addons/zylann.voxel/` | godot_voxel GDExtension (CI build 2026-10-01; pin a tag). |
| `assets/textures/atlas.png` | 8×11 tiles of 16×16 placeholder textures (82..87 free: grow ROWS for more, ADR 0024); `tools/make_atlas.py` regenerates, then `--headless --import` before tests. |
| `assets/shaders/terrain.gdshader` | Opaque material for terrain, micro models, item cubes, viewmodel: nearest atlas, baked AO, per-face tint, scrolling water/lava/fire tiles, emissive glow/lava/fire tiles (bloom). Body shared via `terrain_common.gdshaderinc`. |
| `assets/shaders/terrain_liquid.gdshader` | Same sampling with alpha + depth write for passable transparent voxels (water, oil, gases, fire); rippling sun glint on water tops. `GameMaterials.build_library` assigns it. |
| `assets/shaders/sky.gdshader` | Procedural sky: gradient, sun disc/halo, noise clouds, stars, moon; driven by the true sun direction. |
| `scripts/visuals/stain_map.gd` | Noita-style stains (ADR 0015): blood, oil, soot and scorch coat the faces around the cells they touched; water washes them. Fed by `SimBubble.chunks_changed` and `bleed()`; the shader half is `surface_stain` in `terrain_common.gdshaderinc`. |
| `scripts/visuals/near_field.gd` | Ray-cast near field (ADR 0012 step 3): the bubble writes a wrapped `BrickStore` (brick = voxel, micro vegetation expanded to cells), the native `NearFieldFeed` picks and packs the window chunks to upload per frame, `near_scatter.glsl` scatters them into a toroidal volume texture + brick occupancy on the render thread, `near_field.gdshader` marches it with Godot lighting (`LIGHT_VERTEX`), `terrain_common` hides the meshes inside the valid box. Off by default (`Settings.near_field`). |
| `scripts/visuals/atmosphere.gd` | Sky, sun (soft 4-split shadows, `shadows=hard|off` to trade them for ~0.5 ms, optional day/night cycle), AgX tonemap, bloom, SSAO, fog with aerial perspective; caves darken with depth below the surface, liquids tint the view when the camera is inside one. Pure presentation. No MSAA: it extrapolates UVs over tile borders on silhouettes and neighbour tiles bleed in. |
| `scripts/world/waterfall_spray.gd` | Mist emitters at waterfall feet near the camera (reads `WorldGen.waterfalls_near`). |
| `scripts/mobs/` | Sheep and pigs (ADR 0022, `data/mobs.json`): `MobManager` (herd spawning per 16 m cell, dormancy beyond 64 m, picking, feeding, shearing, breeding, hearts, saves), `Mob` (VoxelBody + Minecraft goal brain at 10 Hz), `MobModel` (micro-voxel bodies, shared meshes). |
| `scripts/farming/farming.gd` | Minecraft farming (ADR 0022, `data/crops.json`): tilling, planting, random ticks, hydration, growth formula, trampling, crop drops; tracks only player-made farmland and crops. |
| `scripts/blocks/furniture_entity.gd` | Beds and chairs (ADR 0022): a block entity over several invisible `furniture` voxels with one rotated micro-voxel mesh. |
| `scripts/player/ape_model.gd` | The player body: a monkey in goggles and a lab coat built from micro-voxel cells (body 3.1 cm, head 1.8 cm) by `MicroMesher`, limbs pivoted for the walk cycle; also the first-person arm. `tools/ape_sheet.gd` renders it from four sides. |

## Architecture in one paragraph

godot_voxel owns the *static* world: generation, streaming, LOD rings, meshes,
collisions. The native `SimWorld` owns a *bubble* of chunks around the player
(Settings.simulation_distance_m): chunks are copied in from the terrain when
they enter the bubble, ticked at `sim_tick_hz` with the Falling Everything
rules, and chunks the simulation changed are pasted back through
`VoxelTool.paste` (never `try_set_block_data`, see P5 §C). Player edits inside
the bubble go to the sim so materials react immediately. Outside the bubble
nothing moves. Server and client will share this split (ADR 0002).

## Known gaps (skeleton stage)
- Persistence is godot_voxel's SQLite stream (`user://worlds/world_<seed>.sqlite`),
  a placeholder for the versioned region format; sim-bubble state (burning,
  lifetimes) is not saved, only materials.
- Generator: no dungeons or villages yet (ruins, trees, rivers, lakes, marshes, waterfalls, vegetation exist).
- Station blocks (ADR 0007) have no facing; no double chests. Their entities save as JSON beside the
  world database (every 30 s and on exit), not yet in the region format. Cobblestone
  places stone until a cobblestone material exists.
- Chiseling (ADR 0006) is designed and the mesher measured (P6), but there is no chisel tool, no `Chisel`
  block entity and no overlay meshes yet; `CHISELED` (44) is reserved and renders as nothing.
- Micro-voxel plants/detail live in LOD 0 blocks only; after an edit, godot_voxel downsamples them into
  LOD 1/2 where they show as full voxels.
- Player body is a box 1.2 voxels wide: 1-voxel shafts are not enterable (dig 2x2 like the autopilot does).
- No terrain physics colliders (ADR 0011): new movers use VoxelBody or voxel ray-casts. Swimming is minimal.
- Item icons for non-block items are coloured tags with initials; the held item is a cube or a card, no arm.
- Memory at large full-detail radius; see `docs/research` matrix in the commit log.

The F4 playtest menu's "Terrain detail" (normal / high / ultra) sets the LOD
ring width and the full-detail radius for the next start (ADR 0020).

## The voxel extension is our build (ADR 0020)

`addons/zylann.voxel/bin/libvoxel.windows.template_debug.x86_64.dll` (git-ignored)
is godot_voxel 045ff932 plus `prototypes/p11-lod-horizon/voxel_patch.py`. To
rebuild it (MinGW on PATH, godot-cpp 4.5 branch at `~/Tools/godot-cpp`):

```
git clone https://github.com/Zylann/godot_voxel ~/Tools/godot_voxel && cd ~/Tools/godot_voxel && git checkout 045ff932
python <repo>/prototypes/p11-lod-horizon/voxel_patch.py ~/Tools/godot_voxel
python <repo>/prototypes/p11-lod-horizon/voxel_patch_budget.py ~/Tools/godot_voxel
python <repo>/prototypes/p11-lod-horizon/voxel_patch_remesh.py ~/Tools/godot_voxel
GODOT_CPP_PATH=$HOME/Tools/godot-cpp python -m SCons platform=windows target=template_debug use_mingw=yes use_static_cpp=yes -j12
cp project/addons/zylann.voxel/bin/libvoxel.windows.template_debug.x86_64.dll <repo>/game/addons/zylann.voxel/bin/
```

The stock CI DLL still runs the game (`voxel.gdextension` can point `windows.debug`
back at `libvoxel.windows.editor.x86_64.dll`), but then set `lod_fade_s=0`: the
stock fade needs a `terrain.material` and costs 30-60 ms frames.
