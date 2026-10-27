# 0094 – Doors, base building and decoration pieces

- **Status:** accepted (built 2026-10-27)
- **Date:** 2026-10-27

## Context
Requested 2026-10-27: "add decoration and base building items, for sure doors at least". The shape
builder already makes walls, stairs, slopes and boxes from any material, planks and carpet are
placeable materials, and beds and chairs are furniture (ADR 0022): block entities that fill their
footprint with invisible `FURNITURE` voxels (full collision, the aim ray hits them) and draw one
micro-voxel model. Glass is an item but not a placeable material.

## Decision
Every new piece is a furniture kind (`FurnitureEntity.KINDS`) with an item (`"furniture": kind`), a
crafting-table recipe and a sprite; no new material ids or atlas tiles.

- **Thin footprints.** A kind may give a `box` of local voxels (x across, y up, z along the facing):
  only those voxels become `FURNITURE`. A door is a one-voxel slab, a fence two.
- **Doors open.** A kind with an `open` box (door, trapdoor, fence gate) swaps its voxels to that box
  on a right click (`Main.toggle_door`, refused when a wall or the player is in the way) and its model
  swings about `hinge` by `swing` (a tween, the chest's sounds). The open state is saved with the entity.
- **Sizes in metres, not Minecraft blocks.** The player is 0.6 m wide and 1.8 m tall and a block is
  0.5 m, so a door is 2 x 4 blocks (1 x 2 m) and a trapdoor 2 x 2 (a 1 m hatch); a "1 x 2 block" door
  would not let the player through. Fences are 1 m high but collide to 1.5 m, above the 1.25 m jump.
- **Wall pieces** (`wall`: door, fence, gate, ladder, window, painting) snap across to the block grid
  but sit in the clicked voxel's depth layer, so they line up with walls the shape builder put
  anywhere on the voxel grid; the rest snap to the block grid like beds.
- **Ladders** (`climb`): the player climbs while a ladder voxel touches the body's side (walk into it
  or hold jump; sneak hangs; else a slow slide; no fall damage).
- **Lanterns** use the item `"light"` key: TorchLights also lights placed furniture whose item has one.
- **Windows** are a wooden frame plus one transparent box (the micro model is opaque), warmed at load.
- Furniture with nothing to do on a click (table, shelf, window...) no longer swallows the right
  click: the held item is placed on it (a flower pot on a table).

Items: door, trapdoor, fence, fence gate, ladder, window, table, bookshelf, lantern, flower pot,
painting. Not added: glass blocks or panes as a material (the window covers it), stairs and slabs
(the shape builder makes them), carpet (exists).

## Consequences
- Checks: `test/test_building_items.gd` (footprints, door collision open and shut, save and load,
  recipes); `building_test=1` places every piece through the player's input code in a room and saves
  `out/<run>/building.png` (door shut | open) with a JSON line `debug building`.
- Known limits: ladders are placed one item at a time against the wall (clicking a ladder's face puts
  the next one in front of it); fences do not join up; mobs path around doors like any wall and never
  open them.
