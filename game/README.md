# TechnoApe – game project (Godot 4.7)

The real project. Prototypes live in `../prototypes/`; decisions in
`../docs/decisions/`; the vision in `../docs/design/vision.md`.

## Run

```
# play (mouse captured; Esc releases; F fly/walk; F3 debug overlay; hold LMB to mine; RMB place/use;
#       1-9 / wheel select; E inventory; Q drop one (Ctrl+Q stack))
Godot_v4.7.2-stable_mono_win64_console.exe --path game

# unattended check for Claude: fly forward 20 s, filmstrip every second, JSON on stdout
Godot_v4.7.2-stable_mono_win64_console.exe --path game -- autopilot=1 duration=20 capture_every=1 vsync=0 out=res://out/run1
# any Settings value can be overridden: render_distance_m=64 view_distance_m=1024 lod_count=8 sim_tick_hz=30 ...
# park at a feature and look down: target=lake|ruin|spring|river|alpine|snow|mountain speed=0 offset=15 alt=20 pitch=-0.7
# walk instead of fly (gravity + 1-voxel auto step-up): walk=1 ; JSON reports travelled_horizontal_m
# pick a world: seed=1337 or seed="any text" (hashed); see docs/design/seeds.md
# the JSON ends with bubble_verify: simulation bubble vs generator (0 mismatches expected)
# interaction checks (ADR 0005): give=... fills the hotbar; mine=1 digs a 2x2 shaft under the feet
# (JSON: picked_up, item_stats, player.hotbar); place=1 places the held block ahead every 0.2 s;
# scatter_items=1 drops one entity per given item in front; open_inventory=1 for a screen shot
Godot_..._console.exe --path game -- autopilot=1 mine=1 duration=20 capture_every=2 give=wooden_pickaxe out=res://out/mine
Godot_..._console.exe --path game -- autopilot=1 walk=1 speed=0 duration=4 open_inventory=1 give=oak_log:12,cobblestone:64,bucket:2 out=res://out/inv


# generator probes (headless): stats + biome map; settle test around a feature
Godot_..._console.exe --headless --path game --script res://tools/gen_probe.gd -- seed=1337 radius=2048
Godot_..._console.exe --headless --path game --script res://tools/settle_probe.gd -- target=lake ticks=60
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
| `scripts/player.gd` | First-person walk/fly, mouse look, hold-to-mine with progress, place/use (buckets), hotbar keys, drop, fall damage, held-item viewmodel (`scripts/player/viewmodel.gd`). |
| `scripts/player/player_inventory.gd` | Player model: 36-slot inventory (hotbar = slots 0..8), cursor stack, 2x2/3x3 crafting grid, health. Pure logic, tested. |
| `scripts/player/mining_table.gd` | `data/mining.json`: hardness, tool class/tier, drops per material (Minecraft break-time formula). |
| `scripts/items/item_entity.gd` | Dropped stacks: textured mini-cube rigid bodies, bob/spin, merge, pickup, despawn; pop out of solid voxels. |
| `scripts/ui/hud.gd` | Crosshair, hotbar with isometric atlas icons (`item_icon.gd`), hearts, break progress. |
| `scripts/ui/inventory_screen.gd` | Vanilla-layout inventory / crafting-table screen with cursor semantics. |
| `scripts/debug_hud.gd` | F3 overlay. |
| `scripts/debug_capture.gd` | Autoload. Autopilot, filmstrips, JSON stats, vsync switch. |
| `addons/technosim/` | Our GDExtension (built from `../prototypes/p5-sim-spike/src`). |
| `addons/zylann.voxel/` | godot_voxel GDExtension (CI build 2026-10-01; pin a tag). |
| `assets/textures/atlas.png` | 4×4 tiles of 16×16 placeholder textures; `tools/make_atlas.py` regenerates. |

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
- Generator: no dungeons, villages, waterfalls or vegetation details yet (ruins, trees, rivers, lakes exist).
- No crafting table / furnace / chest *blocks* yet (they need materials + atlas tiles); the 3x3 screen exists
  (`InventoryScreen.open(3, "crafting_table")`). Cobblestone places stone until a cobblestone material exists.
- Player body is a capsule 1.2 voxels wide: 1-voxel shafts are not enterable (dig 2x2 like the autopilot does).
- Item icons for non-block items are coloured tags with initials; the held item is a cube or a card, no arm.
- Memory at large full-detail radius; see `docs/research` matrix in the commit log.
