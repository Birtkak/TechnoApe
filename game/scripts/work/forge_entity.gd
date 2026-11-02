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
	var size := Vector3i(8, 16, 8)
	var c := WorkPalette.new_cells(size)
	# a smith's hearth, big and industrial (ADR 0129): a fire-brick body on an iron plinth at working
	# height, an iron rim round a 4 x 4 bed of coal (the fuel sits at y = 5) that glows when lit, a bronze
	# tuyere in its side, and four steel posts carrying a riveted sheet-iron hood that narrows to a brick chimney
	var bed_y := 5
	WorkPalette.box(c, size, Vector3i(0, 0, 0), Vector3i(8, 1, 8), WorkPalette.IRON_DARK)
	WorkPalette.wall_box(c, size, Vector3i(0, 1, 0), Vector3i(8, 6, 8), WorkPalette.WALL_BRICK, WorkPalette.BRICK_DARK)
	WorkPalette.box(c, size, Vector3i(0, 6, 0), Vector3i(8, 7, 8), WorkPalette.IRON_DARK)   # the rim
	WorkPalette.box(c, size, Vector3i(7, 3, 3), Vector3i(8, 5, 5), WorkPalette.BRONZE)      # the tuyere
	for z in range(2, 6):
		for x in range(2, 6):
			c[WorkPalette.idx(size, x, bed_y - 1, z)] = WorkPalette.ASH + 1
			c[WorkPalette.idx(size, x, bed_y, z)] = (WorkPalette.EMBER if is_lit() else WorkPalette.CHARCOAL) + 1
			c[WorkPalette.idx(size, x, bed_y + 1, z)] = 0
	for p in [Vector2i(0, 0), Vector2i(7, 0), Vector2i(0, 7), Vector2i(7, 7)]:
		WorkPalette.box(c, size, Vector3i(p.x, 7, p.y), Vector3i(p.x + 1, 10, p.y + 1), WorkPalette.IRON_DARK)   # the posts
	for k in 3:   # the hood: a stepped iron pyramid, rivets round its skirt
		WorkPalette.box(c, size, Vector3i(k, 10 + k, k), Vector3i(8 - k, 11 + k, 8 - k), WorkPalette.STEEL)
	for i in 8:
		if i % 2 == 0:
			for p in [Vector3i(i, 10, 0), Vector3i(i, 10, 7), Vector3i(0, 10, i), Vector3i(7, 10, i)]:
				c[WorkPalette.idx(size, p.x, p.y, p.z)] = WorkPalette.RIVET + 1
	WorkPalette.wall_box(c, size, Vector3i(2, 13, 2), Vector3i(6, 16, 6), WorkPalette.WALL_BRICK, WorkPalette.BRICK_DARK)   # the chimney
	WorkPalette.box(c, size, Vector3i(2, 15, 2), Vector3i(6, 16, 6), WorkPalette.IRON_DARK)   # its cap band
	WorkPalette.box(c, size, Vector3i(3, 13, 3), Vector3i(5, 16, 5), WorkPalette.SOOT)
	var fuel := inv.get_slot(FUEL)
	var lumps := 0 if fuel.is_empty() else int(fuel["count"]) / maxi(items.units(fuel["id"]), 1)
	var bed := WorkPalette.EMBER if is_lit() else WorkPalette.CHARCOAL
	var k := 0
	for z in range(2, 6):
		for x in range(2, 6):
			if k < lumps * 2 and (x - 3.5) * (x - 3.5) + (z - 3.5) * (z - 3.5) <= 7.0:
				c[WorkPalette.idx(size, x, bed_y, z)] = (bed if (x + z) % 2 == 0 or not is_lit() else WorkPalette.CHARCOAL) + 1
			k += 1
	var laid := []
	var input := inv.get_slot(INPUT)
	if not input.is_empty():
		# each item on the coals side by side, lying on top of the bed, glowing with its heat (ADR 0087)
		var on := mini(int(input["count"]) / maxi(items.units(input["id"]), 1), Settings.forge_input_max)
		for j in on:
			var z := 0.5 + (j - (on - 1) / 2.0) * 0.3
			laid.append({"id": input["id"], "at": Vector3(0.5, (bed_y + 1) / 8.0, z), "yaw": 0.0, "scale": 0.32, "heat": Inventory.heat_of(input)})
	return {"solid": {"cells": c, "size": size}, "items": laid}
