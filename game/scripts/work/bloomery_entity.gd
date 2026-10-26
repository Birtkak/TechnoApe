class_name BloomeryEntity
extends FirepitEntity

## A bloomery (ADR 0070): a clay-brick shaft where iron ore becomes an iron bloom. Lay ore in (a
## "smelt" recipe with station "bloomery": 8 iron nuggets, 4 iron-bearing stones or a block of iron
## ore) and charcoal or coal, light it with a firestarter; while the fuel lasts it works the ore for
## the recipe's ticks (two minutes) and leaves a glowing bloom, which an empty hand takes out. It
## holds one bloom: take it out to start the next. No vessel, and it heats nothing.


func _init(p_items: ItemRegistry, p_recipes: RecipeRegistry, p_pos: Vector3i) -> void:
	super(p_items, p_recipes, p_pos)
	kind = "bloomery"
	title = "Bloomery"
	station = "bloomery"
	cook_total = 2400


func burns(id: String) -> bool:
	return items.has_tag(id, "coal") and fuel_ticks(id) > 0


func heats(_id: String) -> bool:
	return false


func may_place(slot: int, id: String) -> bool:
	return false if slot == VESSEL else super(slot, id)


func view_parts() -> Dictionary:
	var size := Vector3i(8, 12, 8)
	var c := WorkPalette.new_cells(size)
	# a tapering shaft of fired clay with a mouth at the front (+z) and a glow inside while it burns
	for y in 12:
		var r := 3.9 if y < 5 else (3.4 if y < 9 else 2.9)
		for z in 8:
			for x in 8:
				var dx := x - 3.5
				var dz := z - 3.5
				var d2 := dx * dx + dz * dz
				if d2 > r * r:
					continue
				var mouth := y >= 1 and y <= 3 and z >= 6 and x >= 3 and x <= 4
				if d2 > (r - 1.2) * (r - 1.2) and not mouth:
					c[WorkPalette.idx(size, x, y, z)] = (WorkPalette.FIRED if (x + y + z) % 5 != 0 else WorkPalette.CLAY) + 1
				elif y == 0 or (mouth and y == 1):
					c[WorkPalette.idx(size, x, y, z)] = (WorkPalette.EMBER if is_lit() else WorkPalette.ASH) + 1
				elif is_lit() and y <= 3:
					c[WorkPalette.idx(size, x, y, z)] = WorkPalette.GLOW + 1
	var laid := []
	var out := inv.get_slot(OUTPUT)
	if not out.is_empty():
		laid.append({"id": out["id"], "at": Vector3(0.5, 0.06, 0.95), "yaw": 0.0, "scale": 0.4})
	return {"solid": {"cells": c, "size": size}, "items": laid}
