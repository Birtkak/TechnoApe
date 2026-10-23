# 0034 – Brush digging, voxel item units and block footprints

- **Status:** accepted 2026-10-23 (the user: "brushes + amounts"; "change everything needed
  ... with whatever is best"). Supersedes the mining and placing parts of ADR 0005 and the
  one-voxel blocks of ADR 0007, 0022, 0023 and 0030 on the 12.5 cm grid of ADR 0033.
- **Date:** 2026-10-23

## Context
ADR 0033 made every voxel 12.5 cm. Everything the player did was built on one
voxel being a 0.5 m block: break one voxel per click with Minecraft times, place
one voxel per item, a crafting table or a knapping stone is one voxel, a stack
of 64 dirt is 64 blocks, recipes take one item per grid slot. At 12.5 cm a
click would move 1/64 of what it did, and a station would be a 12.5 cm cube.
The user chose Lay of the Land's model: tools dig blobs, the inventory holds
material amounts in Minecraft-style slots, building places shapes.

## Decision
**The brush** (`scripts/player/brush.gd`, pure logic). One brush for digging and
building: a size of 1, 2, 4 or 8 voxels (Ctrl + wheel) and a shape (B): a cube
snapped to its own grid, a ball, or an upright cylinder. The default is a 4-voxel
cube, so out of the box a dig or a placement is the old 0.5 m block on the old
block grid; smaller brushes sculpt, the 8-voxel brush moves a cubic metre.
- Digging takes the shape around the aimed voxel. The held tool caps the size
  (`MiningTable.dig_cap`): bare hands 2, tools 4, iron and steel 8. The voxels
  the tool breaks at a pace comparable to the aimed one go (a shovel in dirt
  leaves the rock around it); stations, furniture, work pieces and liquids
  never go with a brush, an aimed one breaks alone as before.
- A dig takes the sum of its voxels' block times / 64: a full 4^3 cube of dirt
  takes what a Minecraft block did.
- Placing sets the shape against the aimed face, filling only empty or
  replaceable cells outside the player, at most as many as the stack holds.
- The hoe tills, and seeds sow, over the brush's square of ground.

**Units** (`ItemRegistry.units`). An item that places a plain terrain voxel
(dirt, sand, gravel, cobblestone, logs, planks, ores, thatch, daub) counts in
12.5 cm voxels; its stack holds `stack` blocks of 64 voxels (4096). Recipes,
smelting, firepit fuel and the sieve still count blocks: one recipe unit of a
voxel item is 64 voxels (the crafting grid needs a block's worth per slot,
right click moves a block's worth). Every other item counts in pieces.
Drops (`MiningTable.voxel_drops`) flow by expected value: a block's worth of
stone gives a block's worth of cobblestone, a chance drop (flint from gravel)
comes whole once its fractions add up, carried between digs; plants and ground
pickups roll their old drops once per dig.

**Footprints** (`BlockEntities.footprint`). Every block entity covers voxels:
a station, chest, furnace, storage pot or work piece a 0.5 m block of 4^3
voxels with its anchor at the min corner (flat pieces are lower: a knapping
stone or a mold one voxel, a firepit two, so they never hide the ground from
the aim ray); furniture lays whole blocks out from its anchor block; a torch is
one voxel. Every covered voxel maps to the entity (`at()` finds it from any of
them). Anchors stand on the real ground: x and z snap to the block grid, y is
the voxel in front of the aimed face. Stations' voxels only collide; the
entity draws the block (pixel-art cube or micro model at block scale) with
`terrain_prop.gdshader`, which keeps the 16x16 textures that terrain voxels no
longer show. Work piece views and their aim math (`WorkSystem.plane_cell`)
scale by a block; charcoal pits find neighbours a block away and are sealed
when every voxel touching a face from outside seals.

## Consequences
- Easier: one brush serves digging, building, sculpting and farming; block
  building stays one click per block; recipes and their data did not change.
- Harder: a stack's count is a voxel count (4096 dirt fills a slot), so UI
  numbers are large; recipe pages still show blocks.
- Measured: `stone_age_test`, `flint_age_test`, `hands_test` and
  `swim_test` pass on the fine grid; 298 unit tests (new `test_brush.gd`).
- Open taste calls: dig loss (today a dig yields everything), whether loose
  dug dirt piles keep their shape, shapes beyond cube/ball/cylinder (slopes,
  paths), crop yields per area (a 0.5 m patch now holds 16 crops).
