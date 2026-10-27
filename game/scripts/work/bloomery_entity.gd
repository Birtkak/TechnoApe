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
	# a stepped brick stack: a wide base, two narrower tiers and a cap slab, courses of brick with dark mortar;
	# a small arched mouth at the front (+z) with the fire glowing in it, a hollow in the base, a chimney hole
	WorkPalette.wall_box(c, size, Vector3i(0, 0, 0), Vector3i(8, 5, 8), WorkPalette.WALL_BRICK, WorkPalette.BRICK_DARK)
	WorkPalette.wall_box(c, size, Vector3i(1, 5, 1), Vector3i(7, 8, 7), WorkPalette.WALL_BRICK, WorkPalette.BRICK_DARK)
	WorkPalette.wall_box(c, size, Vector3i(2, 8, 2), Vector3i(6, 11, 6), WorkPalette.WALL_BRICK, WorkPalette.BRICK_DARK)
	WorkPalette.box(c, size, Vector3i(1, 11, 1), Vector3i(7, 12, 7), WorkPalette.BRICK_LIGHT)   # cap slab
	WorkPalette.box(c, size, Vector3i(3, 11, 3), Vector3i(5, 12, 5), WorkPalette.SOOT)          # chimney hole
	for z in range(2, 6):   # the hollow the fire burns in
		for y in range(0, 4):
			for x in range(2, 6):
				c[WorkPalette.idx(size, x, y, z)] = 0
	WorkPalette.box(c, size, Vector3i(2, 0, 2), Vector3i(6, 1, 6), WorkPalette.ASH if not is_lit() else WorkPalette.EMBER)
	if is_lit():
		WorkPalette.box(c, size, Vector3i(3, 1, 3), Vector3i(5, 3, 5), WorkPalette.GLOW)
	for y in range(1, 4):   # the mouth, through the front wall into the hollow
		for x in range(3, 5):
			for z in range(5, 8):
				c[WorkPalette.idx(size, x, y, z)] = 0
	WorkPalette.box(c, size, Vector3i(3, 1, 5), Vector3i(5, 2, 8), WorkPalette.EMBER if is_lit() else WorkPalette.ASH)
	WorkPalette.box(c, size, Vector3i(3, 4, 7), Vector3i(5, 5, 8), WorkPalette.SOOT)            # soot above the mouth
	var laid := []
	var out := inv.get_slot(OUTPUT)
	if not out.is_empty():
		laid.append({"id": out["id"], "at": Vector3(0.5, 0.06, 0.95), "yaw": 0.0, "scale": 0.4})
	return {"solid": {"cells": c, "size": size}, "items": laid}
