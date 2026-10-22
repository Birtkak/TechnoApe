# 0005 – Player, inventory, hotbar/HUD and first-person interaction

- **Status:** accepted (routine defaults recorded for Brent to overrule)
- **Date:** 2026-10-22

## Context
The skeleton had a first-person camera with debug break/place (any voxel to
air, right click places sand). The crafting core (ADR-less, see
`game/README.md`: `ItemRegistry`, `RecipeRegistry`, `Inventory`,
`CraftingGrid`, 17 headless tests) existed as pure logic without a UI or a
world connection. The vision fixes the feel: Minecraft for taking matter out
of the world and putting it back (mining, item entities, hotbar, inventory
screens with exact vanilla slot behaviour), Noita for what matter does while
it is in the world. Survival pressure (hunger, temperature) is still an open
question; combat too.

## Options considered
1. **Minecraft-exact slot model and screens** (9-slot hotbar, 27 slots,
   2×2 personal crafting grid, cursor stack, shift-click, number keys and
   wheel). Zero learning for players; the crafting core already implements
   the semantics. Chosen.
2. Grid-free "bag" inventory with auto-sorting (Valheim style). Simpler UI,
   but contradicts the vision ("must feel exactly like Minecraft").
3. Item entities as pure data (auto-pickup at break time, no world objects).
   Cheaper, but the vision says matter drops as an entity you walk over.

## Decision
- **Player model** (`scripts/player.gd` + `scripts/player/`): one
  `PlayerInventory` (hotbar 0..8, main 9..35, one `Inventory` of 36 slots;
  the hotbar is the first row like vanilla), a cursor stack, a 2×2
  `CraftingGrid`, `selected` hotbar index, health 0..20 in half-hearts.
  Pure logic, headless-testable; the player node only forwards input.
- **Mining through the material model**: `data/mining.json` maps material
  names to hardness (seconds by hand), the tool class required, the tool
  class that speeds it up, and what drops. Break time follows Minecraft:
  `hardness × 1.5 / tool_speed` with the right tool, `× 5` and no drop with
  the wrong one. The table is validated against `GameMaterials` and
  `items.json` in tests; a material missing from the table is unbreakable
  by hand (safe default). Liquids and gases are never mined (vision:
  liquids need a container); buckets pick up water and place it back.
- **Placing**: a block item with a `material` places that material at the
  raycast's previous voxel unless it intersects the player; the voxel goes
  through the sim bubble when inside it (so sand placed above a hole falls
  at once). Placing consumes one item.
- **Item entities** (`scripts/items/item_entity.gd`): `RigidBody3D` with a
  tiny textured cube (atlas tiles of the block's material; a placeholder
  card for non-block items), bob + spin, 0.5 s pickup delay, merge with
  stacks of the same item nearby, despawn after `item_despawn_s`. Pickup
  radius and all timings are `Settings` in metres/seconds.
- **HUD** (`scripts/ui/hud.gd`): crosshair, 9-slot hotbar with isometric
  block icons drawn from the atlas (`ItemIcon` control), counts, selected
  slot highlight and item name fade, hearts row (health), break progress.
  The F3 debug overlay stays separate.
- **Inventory screen** (`scripts/ui/inventory_screen.gd`): vanilla layout
  (2×2 grid + output top, 27 main slots, hotbar row), cursor stack drawn
  under the mouse, left/right/shift click via the crafting core's cursor
  operations, closing returns grid contents. A 3×3 crafting table screen
  reuses the same control with a `CraftingGrid` of width 3 (the crafting
  table *block* needs a material + atlas tile and lands with the next
  material batch).
- **Health**: fall damage like vanilla (1 damage per block beyond 3 m of
  fall at 1 m = 2 voxels), death respawns at spawn with the inventory kept
  (no hunger, no keep-inventory decision yet; see open questions).
- **First-person "character"**: no third-person body yet; a held-item
  viewmodel (cube of the selected block, swing on use) hangs from the
  camera. A real arm/body model is art, later.
- **Debug reach**: `main.snapshot()` reports inventory and health; the
  autopilot gains `mine=1` (walk, look down, hold break for `duration`,
  report what was picked up) so Claude can verify the loop without eyes.

## Consequences
- Items that place a material must carry `material` in `items.json`;
  materials that should drop something need an entry in `mining.json`.
  Tests catch dangling names, so content stays data-driven.
- Item entities are plain physics bodies: hundreds are fine, thousands are
  not. Merge-on-contact and despawn keep counts bounded; a pooled
  MultiMesh renderer is the upgrade path if a quarry fills the ground.
- Revisit when hunger/temperature are decided (HUD rows) and when wands
  arrive (the hotbar gets wand slots like Noita, or wands are hotbar items).
