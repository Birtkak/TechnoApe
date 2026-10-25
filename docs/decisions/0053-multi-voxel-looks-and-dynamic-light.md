# ADR 0053: Multi-voxel crops, torches and placed clutter; dynamic light from carried and dropped sources

Status: accepted (2026-10-25)

## Context
Playtest of 2026-10-25 (part C of `docs/plans/2026-10-25-bug-report-split.md`): a carrot field was a flat
orange slab with green holes (crops were one cube per voxel since ADR 0033 flattened the micro models);
a placed flower was one pixel while generated ones are ~30-voxel pieces (ADR 0047); a held mushroom was a
cube; torches were single glowing voxels ("super ugly"); the crafting table was a knee-high 0.5 m cube;
ore icons did not look like Minecraft's newer ores; animals no longer fit the patterned world and their
legs raced; a held or dropped light source gave no light ("like Minecraft shader packs"), and later
"light sources need to light up a much bigger area".

## Decision
1. **Stamps** (`scripts/farming/plant_stamps.gd`): placed things become voxel pieces, their variety seeded
   by position so a stamp can be rebuilt from it. Cells go only where they are free (air, replaceable).
2. **Crops** stand one plant per 2 x 2 voxels (four to a 0.5 m block); planting snaps to that grid. Every
   column of the 2 x 2 carries a stalk of its own height (the developer asked for denser fields); wheat
   grows to 6 voxels and turns gold with two ears per stalk, carrots are leaf tufts with orange shoulders
   when ripe. The anchor (the voxel above the farmland) keeps the age material Farming checks; `parts`
   maps every voxel to its anchor, so breaking any of them takes the plant and drops once. It is rebuilt
   from the save. Unit tests on one-voxel blocks keep single-voxel crops.
3. **Placed flowers, mushrooms and seashells** grow into FineGen's clutter pieces (`clutter_voxel`
   ported), so they dig whole like generated ones.
4. **Torches** are a 4-voxel dark stick, a cross of ember cloth and a big flame (3 x 3, a cross, a 2-voxel
   tip): ~20 voxels, 1 m tall (the developer picked "bigger"). On a floor they stand; with no floor and a
   wall beside them they lean out from it. The voxels are the TorchEntity's footprint (saved with its
   wall), so breaking any part breaks the torch. The flame has its own tile (120, 8 colours red to a pale
   tip) that glows and flickers in the terrain shader (`flame_tile`), no remesh. The held and dropped
   torch is the same model (`TorchEntity.item_mesh`, through `BlockMeshes.MODELS`); its flame flickers
   with its light.
5. **Crafting table**: 1 m x 0.75 m x 1 m (8 x 6 x 8 voxels, the developer's pick), drawn in 6.25 cm cells
   with 8-shade patterned oak: legs, apron, shelf, planked top with a 3 x 3 grid, a saw and a hammer. With
   no room for the bigger footprint the item drops back.
6. **Items**: ore items show Minecraft 1.17-style ore blocks (stone with cracks and chunky shaded
   clusters; tin as black cassiterite); mushrooms, kelp and seashells get sprites instead of cubes.
7. **Animals** are drawn as before at 3.1 cm cells and merged to 6.25 cm (the developer picked the
   half-voxel style from three), features (eyes, noses) keep their cells, every colour is an 8-shade ramp.
   The stride runs on metres again (it ran on voxels/s since ADR 0033: ~3 strides a second).
8. **Lights** (`TorchLights`): one capped pool of shadowless OmniLight3Ds for placed torches, lit block
   entities (furnace, firepit, kilns, firewood piles) and dropped stacks of items with a `"light"` key; the
   held item's light follows the hand every frame. Range 16 m with a soft falloff (attenuation 0.7), all
   Settings tunables (`torch_light_*`, `fire_light_*`). An item lights with `"light": {}` (optionally
   `range_m`, `energy`, `color`).
9. Ids: materials 130 torch_stick, 131 torch_wrap, 132 wheat_ear, placeholders 116-129 and 133-149; tiles
   112-120 (104-111 placeholders for part B).

## Consequences
- Measured (dev box, no other Godot running): 10 torches and a held one at night p99 6.06 ms, max 8.49;
  24 torches p99 6.88 ms, max 8.27 (render GPU ~5 ms). The first held sprite compiled a pipeline mid-game
  (19.7 ms): item sprites are now drawn under the loading screen with the table.
- Old saves: crops keep growing (their stamps fill in as they grow), torches from before are one voxel.
- Lava, glow mushrooms and the sim's fire give no real light yet (they only glow); that needs a voxel scan
  near the player and is left for later.
- Checks: `looks_test=1` (everything on a patch, shots), `looks_test=1 looks_budget=1 looks_torches=24`
  (the budget case), `tools/mob_style_sheet.tscn` (styles, `film=1` a walk filmstrip),
  `tools/item_sheet.tscn`, `test_plant_stamps.gd`.
