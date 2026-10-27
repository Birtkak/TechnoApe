class_name ForgeEntity
extends FirepitEntity

## A forge (ADR 0070): loose stones round a bed of charcoal or coal. The only fire that gets iron
## ("forge_heat" items: ingots, blooms, work pieces) hot enough for the anvil; it heats any heatable
## metal and does nothing else (no vessel, no cooking). Worked by hand like the firepit: right click
## lays on fuel or metal, a firestarter lights it, an empty hand takes the metal off.


func _init(p_items: ItemRegistry, p_recipes: RecipeRegistry, p_pos: Vector3i) -> void:
	super(p_items, p_recipes, p_pos)
	kind = "forge"
	title = "Forge"
	station = "forge"


func burns(id: String) -> bool:
	return items.has_tag(id, "coal") and fuel_ticks(id) > 0


func heats(id: String) -> bool:
	return items.has_tag(id, "heatable")


## Two ingots (or blooms, or work pieces) lie on the coals at once (bug report 2026-10-27_18-02-35).
func input_max(id: String) -> int:
	return Settings.forge_input_max * items.units(id)


func may_place(slot: int, id: String) -> bool:
	return false if slot == VESSEL else super(slot, id)


func view_parts() -> Dictionary:
	var size := Vector3i(8, 4, 8)
	var c := WorkPalette.new_cells(size)
	# a cobblestone trough like the furnace's masonry: thick walls four courses high round a 4 x 4 bed of
	# coal that fills it nearly to the rim (the fuel sits at y = 1) and glows red when lit
	var tones := [WorkPalette.STONE, WorkPalette.STONE_WARM, WorkPalette.IRON_DARK]
	WorkPalette.wall_box(c, size, Vector3i(0, 0, 0), Vector3i(8, 4, 8), tones, WorkPalette.STONE_DARK)
	for z in range(2, 6):
		for x in range(2, 6):
			c[WorkPalette.idx(size, x, 0, z)] = WorkPalette.ASH + 1
			c[WorkPalette.idx(size, x, 1, z)] = (WorkPalette.EMBER if is_lit() else WorkPalette.CHARCOAL) + 1
			c[WorkPalette.idx(size, x, 2, z)] = (WorkPalette.EMBER if is_lit() else WorkPalette.CHARCOAL) + 1
			c[WorkPalette.idx(size, x, 3, z)] = 0
	var fuel := inv.get_slot(FUEL)
	var lumps := 0 if fuel.is_empty() else int(fuel["count"]) / maxi(items.units(fuel["id"]), 1)
	var bed := WorkPalette.EMBER if is_lit() else WorkPalette.CHARCOAL
	var k := 0
	for z in range(2, 6):
		for x in range(2, 6):
			if k < lumps * 2 and (x - 3.5) * (x - 3.5) + (z - 3.5) * (z - 3.5) <= 7.0:
				c[WorkPalette.idx(size, x, 1, z)] = (bed if (x + z) % 2 == 0 or not is_lit() else WorkPalette.CHARCOAL) + 1
			k += 1
	var laid := []
	var input := inv.get_slot(INPUT)
	if not input.is_empty():
		# each item on the coals side by side, lying on top of the bed, glowing with its heat (ADR 0087)
		var on := mini(int(input["count"]) / maxi(items.units(input["id"]), 1), Settings.forge_input_max)
		for j in on:
			var z := 0.5 + (j - (on - 1) / 2.0) * 0.3
			laid.append({"id": input["id"], "at": Vector3(0.5, 0.5, z), "yaw": 0.0, "scale": 0.32, "heat": Inventory.heat_of(input)})
	return {"solid": {"cells": c, "size": size}, "items": laid}
