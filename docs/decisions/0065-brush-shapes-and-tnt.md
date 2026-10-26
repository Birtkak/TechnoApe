# ADR 0065: Brush shapes and sizes for reshaping the world; TNT

Status: accepted (2026-10-26)

## Context
Bug reports of 2026-10-25:
- 21-03-36: "change what shape and how big of an area you will mine, to make manipulating the world
  with tools easy; also make it easier to manipulate the world in creative". The brush (ADR 0034) had
  cube, sphere and cylinder at 1-8 voxels (12.5 cm-1 m), the dig size capped by the tool; its
  highlight was the bounding box only, every cell went through GDScript.
- 21-10-33: "add tnt". The crater rod and `/blast` (native `SimBubble.blast`) already carved 8 m
  craters; nothing in survival did.

## Decision
1. **Two more brush shapes** (`Brush`, B / Shift + B cycles): `flat`, a size x size square one voxel
   deep in the aimed face's plane (shave a wall, level a floor, lay a skin), and `column`, a size/2
   wide bar 2 x size long along the face normal (a shaft down or tunnel in when digging, a pillar up
   or beam out when placing). Flat and column snap on the size grid like the cube.
2. **Sizes by mode and tier.** Survival keeps 1-8 voxels; digging stays capped by `MiningTable.dig_cap`
   (hands 2, tools 4, tier 3+ 8) and costs what it did: the sum of the voxels' block times / 64 and a
   tool use per block dug (ADR 0034). Creative adds 16 and 32 (2 m, 4 m), digs whatever the brush
   covers (instant, as before), reaches 12 m (`Settings.creative_reach_m`, survival 5 m), and
   **Shift + right click with a block paints** the brush's dig shape: every plain voxel in it takes
   the held material.
3. **Preview.** Besides the box outline, `BrushTool` draws a see-through shell of the exact cells
   (orange: the dig; blue: a placement, shown while a voxel item is held and the break button is not),
   outer faces only so it vanishes when the camera is inside it. It is rebuilt only when the shape,
   size, face or mode changes; big brushes show a half-resolution shell drawn twice as large (a full
   4 m flat shell took 28 ms to mesh). The HUD shows the brush and its keys right of the hotbar; the
   Settings key list names B and the paint click; `/brush <shape> [size]` sets it from chat.
4. **Big brushes are native and spread over frames.** Above 8 voxels a dig, placement or paint is a
   `ShapeTool.fill` job of the brush's cached relative cells (passing `window - lo` as the window
   origin places them without a GDScript pass), written 6000 cells per frame (0.3-0.4 ms per slice
   measured). The round big brushes are rasterised under the loading screen (a 4 m ball is 16 ms in
   `ShapeTool.cells`). Big creative digs drop nothing (world editing); a sphere of seeds around the
   box goes to the rigid piece check. Measured (seed 1337 plains, creative, 4 m sphere, cube, cylinder
   and flat, 48k voxels, other Godot runs on the machine): max frame 14.3 ms, none over 16.7 ms.
5. **TNT** (material 203 `tnt`, atlas tiles 171 side / 172 top, patterned with a 4-voxel period so a
   0.5 m block reads as one stick bundle; item `tnt`, a voxel item placed with the brush; recipe
   sulfur x2 + charcoal + sand; item `igniter`). A **charge** is one 0.5 m block cell of the block grid
   holding TNT voxels; it blasts with radius `Settings.tnt_radius_m` (2.5 m) x cbrt(share of the
   block that is TNT). `Tnt` (scripts/items/tnt.gd) lights a charge when
   - fire or lava touches a face of its block (checked for 48 charges every 0.2 s, round robin),
   - flint and steel or the igniter is used on it (the igniter only lights TNT; sneak + igniter lights
     every charge within 16 m, the nearest first),
   - a blast reaches it (found natively with `ShapeTool.select`, so TNT placed by the shape builder
     chains too; a random 0.5-1.5 s fuse, Minecraft's chain).
   Lit, its voxels become a primed box (`material_override` copy of one warmed material) that blinks
   white faster as the 3 s fuse burns, hisses (`tnt/fuse`), sparks (`tnt_fuse`) and falls while
   nothing solid is under it. The blast is three native spheres by hardness (`hp`): soft ground
   (hp < 400, powders, wood, plants) the full radius, stone and bricks (<= 1200) 3/4, harder rock 1/2,
   obsidian and bedrock (>= 5000) never; liquids, gases and block entities stay. Then the rim seeds go
   to the rigid piece check (trees fall), the player and animals within twice the radius take up to
   16 damage and a knockback falling linearly with distance (walls do not shield yet), and 30 % of
   what it took drops as the materials' mining drops, estimated from 64 samples in the sphere (never
   by reading every voxel), at most 4 stacks per blast. Fuses that run out together wait their turn:
   one blast per frame (`Settings.tnt_blasts_per_frame`). Flash, smoke and spark particles
   (particles.json `tnt_*`), sounds `tnt/light`, `tnt/fuse`, `tnt/explode` from the existing CC0 cuts.
   Charges are saved with the player's file (`tnt`), like the stone age state.
6. **Benchmark** (`/tnt grid 20 1.5`: 20 blocks 1.5 m apart, the nearest lit; seed 1337 plains, 2-3
   other Godot runs on the machine): 20-23 blasts over ~8 s, 130k voxels, worst blast 8.8 ms
   (chain search 5.9, spheres 0.5, drops and entities 1.4, particles and sound 1.7), max frame
   12.4 ms, none over 16.7 ms. The first primed box compiled 4 pipelines in a 43 ms frame until the
   box mesh itself was added to `MaterialWarmup.add_meshes` and its shadow switched off.

## Consequences
- Material 203 and atlas tiles 171-172 are taken (append only).
- Creative world editing has no undo yet; a 4 m paint or dig is one click.
- Blast damage ignores cover; armour does not exist yet.
- TNT placed before this ADR does not exist; charges placed by the shape builder are only known once a
  blast or an igniter finds them (the fire check uses the registry).
