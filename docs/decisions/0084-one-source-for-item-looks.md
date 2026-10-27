# ADR 0084: One source for a thing's looks; real fish only

Status: accepted (2026-10-27)

## Context

Four bug reports of 2026-10-27 about how items and creatures look:

- 15-28-41: the crafting table's world model (ADR 0053, 0076) and its held, dropped and inventory
  looks no longer matched. The model is a cell grid in `StationEntity`; the other three were an
  atlas cube from tiles 32/33/39, which nobody redraws when the model changes.
- 15-23-17: the axe was held the wrong way round. The tool pose (Viewmodel `TOOL_ROTATION`, yaw
  125 degrees) shows the sprite's back, which turns the honed edge (the sprite's left) back toward
  the camera.
- 15-23-56: on the tool assembly the stick ran through the blade. Since the Minecraft-style
  sprites, a loose head (`*_loose` shapes in make_item_sprites.py) is moved to the middle of its
  icon, but `AssemblyEntity` still laid it as if it sat at the handle's tip; and a stick sprite is
  2 px longer than a tool's handle.
- 15-13-32: the fish were "tiny fake things". `OceanLife` (ADR 0027) drew decorative schools as one
  MultiMesh of small shader fish; the real fish (ADR 0058: 0.4 m mobs that swim, flee, flop on land
  and drop raw fish) were rarer than the fakes.

## Decision

1. **Stations draw everything from their cells.** `StationEntity.cells(name)` is the one grid; the
   placed model (`model_mesh`), the held and dropped mesh (`item_mesh`, 1 unit across, centred,
   handed out by `BlockMeshes.cube`) and the inventory icon (`icon`, the cells ray-cast from the
   corner ItemIcon draws block cubes from, shaded top 1.0 / left 0.8 / right 0.6) all come from it,
   for the crafting table, chest and furnace. A change to a model changes all four looks.
2. **An axe is held edge out**: its sprite is turned half a turn about the handle (the sprite's
   diagonal, so the grip stays put), and the edge faces the chop. Other tools keep their pose.
3. **A loose head is laid where it sits on its tool** (`ItemSprites.loose_offset`, read from the
   shapes, not stored), and a laid stick is pulled back 2 px so it ends in the head's eye.
4. **Only real fish.** `OceanLife` is gone; fish groups are 4-8 instead of 3-6.

## Consequences

- The station icons cost about 10 ms each, once, on first draw.
- Rivers hold fewer fish than the decorative schools showed; spawn chance for water groups
  (WildMoves.WATER_CHANCE) is unchanged and may need tuning after a playtest.
- Look checks: `tools/station_sheet.tscn` now shows a half-built flint axe; `tools/held_sheet.tscn`
  shows the held stations.
