# 0007 – Station blocks: crafting table, furnace, chest

Status: accepted (2026-10-22)

## Decision
- New materials 32..36: crafting_table, furnace, furnace_lit, chest, planks; atlas grows to 8x5 (tiles 32..39). Material keys `interact` (what right click opens) and `entity` (block entity created on placement).
- Block entities (`scripts/blocks/`) keyed by voxel position: ChestEntity (27 slots), FurnaceEntity (input/fuel/output). Pure logic, ticked by Main at Minecraft's 20 ticks/s.
- Furnace copies vanilla AbstractFurnaceBlockEntity.serverTick: fuel only lit when the input can smelt, 200 ticks per item, progress decays 2/tick without fire, lava bucket leaves its bucket, voxel swaps furnace <-> furnace_lit.
- Shift-click follows vanilla quickMoveStack order (chest and furnace output fill the player inventory back to front; furnace takes smeltables to input, fuel to fuel slot, refuses the rest).
- Right click opens a station; Shift (our sneak) places against it instead. Breaking spills contents.
- Fuel times from items.json `fuel_ticks` (log/planks/table/chest 300, stick 100, coal 1600, lava bucket 20000).

## Consequences / revisit
- No block facing yet: the furnace front shows on all four sides, chests are full cubes, no double chests.
- Block entities are not saved, like other world edits.
- The bronze_alloy recipe (station furnace, shapeless) has no screen; it belongs in a later alloy machine.
- Early edits can log godot_voxel "Destination block not found when cascading edits on LOD 1"; harmless so far, not investigated.
