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


func may_place(slot: int, id: String) -> bool:
	return false if slot == VESSEL else super(slot, id)


func view_parts() -> Dictionary:
	var size := Vector3i(8, 6, 8)
	var c := WorkPalette.new_cells(size)
	# a ring of stones two high round a bed of coal, glowing while it burns
	for z in 8:
		for x in 8:
			var dx := x - 3.5
			var dz := z - 3.5
			var r2 := dx * dx + dz * dz
			if r2 > 7.0 and r2 < 16.5:
				c[WorkPalette.idx(size, x, 0, z)] = WorkPalette.STONE + 1
				if (x + 2 * z) % 3 != 0:
					c[WorkPalette.idx(size, x, 1, z)] = WorkPalette.STONE + 1
			elif r2 <= 7.0:
				c[WorkPalette.idx(size, x, 0, z)] = WorkPalette.ASH + 1
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
		laid.append({"id": input["id"], "at": Vector3(0.5, 0.3, 0.5), "yaw": 20.0, "scale": 0.5})
	return {"solid": {"cells": c, "size": size}, "items": laid}
