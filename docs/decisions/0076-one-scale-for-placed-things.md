# ADR 0076: One scale and one shading for placed things

Status: accepted (2026-10-27)

## Context

The crafting table became a 1 m voxel model in ADR 0053, but the chest and the furnace stayed
0.5 m cubes textured with flat atlas tiles: next to the table (and a 1.8 m player) a chest looked
like a toy. The work pieces (firepit, anvil, knapping slab, molds, ...) are 6.25 cm cell models
drawn from `WorkPalette`, one flat colour per paint, so a piece of one material read as a single
blob with no edges. The item sprites had the same complaint at 16 px: outlines no darker than the
body's dark tone, thin tool heads.

## Decision

- **Stations are 1 m voxel models at one scale.** `StationEntity.SIZES` holds the footprint per
  entity kind (table 8x6x8 voxels, chest 8x6x8, furnace 8x8x8); `StationEntity.model_mesh(name)`
  builds the model by material name (`crafting_table`, `chest`, `furnace`, `furnace_lit`) from one
  shared palette and material. `BlockEntities.footprint_of` reads `SIZES`; kinds not in it keep the
  BLOCK^3 cube (the storage pot, the work pieces).
- **The big work pieces are 1 m too** (developer, 2026-10-27: at 0.5 m the anvils, forge, bloomery
  and firepit read as toys next to the furnace). `BlockEntities.WIDE` names them; `width_of(kind)`
  is two blocks for them, their footprint height doubles with it, `WorkPalette.make_view` draws
  their 8-cell grid at one voxel per cell, and the anvil's hammering grid, the drop and light
  positions and `WorkSystem._at` follow `width_of`. Knapping slabs, clay forms, molds on the ground,
  the sieve and the Stone Age pieces stay one block. Held items in the viewmodel grew with them
  (sprites 0.6, cubes 0.32).
- **Every cell model is shaded, with darker edges.** `WorkPalette.shade_cells` gives each paint
  `MobModel.SHADES` tones: a hash per cell with a grain along x and a lift towards the top (the
  workbench's and the animals' rule), and cells with empty neighbours on two axes (a convex edge)
  go 2.5 tones darker, so every shape carries its own outline. Signal colours (`WorkPalette.FLAT`:
  molten, ember, glow, the clay form's guide) keep one tone. The station models use the same pass.
- **Sprite outlines are darker than the ramp.** `make_item_sprites.py`'s `ramp` tints the outline
  tone to 62 %; the hammer head is 5 px deep and 9 long.
- Check: `tools/station_sheet.tscn` renders the four station models and three work pieces in a row
  (`game/out/stations.png`); `tools/item_sheet.tscn` the sprites.

## Consequences

- Worlds saved before this hold chests and furnaces with a 4^3 footprint of station voxels; the
  entity now claims 8x6x8 / 8x8x8 cells, so the view is 1 m while only the old quarter blocks
  bodies. Breaking and replacing the chest fixes it; nothing is lost.
- A chest or furnace needs a free 1 m footprint to be placed, like the table ("No room" otherwise).
- The furnace has no facing, so its mouth is on all four sides.
