class_name ElectricFurnaceEntity
extends FurnaceEntity

## The electric furnace (ADR 0113): the first machine on the energy network. A furnace with no fuel slot:
## while the network gives it electric_furnace_eu_t it burns (a second of fire per solve), so the furnace's
## own smelt rules run unchanged. Worked by hand like a work piece: a right click with a smeltable loads
## the input, an empty hand takes the output.

var eu := 0.0        # what the last solve delivered, for the look-at panel


func _init(p_items: ItemRegistry, p_recipes: RecipeRegistry, p_pos: Vector3i) -> void:
	super(p_items, p_recipes, p_pos)
	kind = "electric_furnace"
	title = "Electric Furnace"


func net(channel: String) -> Dictionary:
	if channel != "energy":
		return {}
	var r := recipes.smelting_for(inv.get_slot(INPUT).get("id", ""), station) if not inv.is_empty_slot(INPUT) else {}
	return {"role": NetworkSolver.SINK, "amount": Settings.electric_furnace_eu_t if _can_burn(r) else 0.0}


## Enough power for the coming second lights it; anything less and it goes dark.
func net_in(channel: String, amount: float) -> void:
	if channel != "energy":
		return
	eu = amount
	var was := is_lit()
	lit_time = 21 if amount >= Settings.electric_furnace_eu_t - 1e-3 else 0   # one solve interval plus a tick
	lit_duration = 21
	if was != is_lit():
		changed.emit()


func may_place(slot: int, _id: String) -> bool:
	return slot == INPUT


## Right click with the held stack: a smeltable goes in (one block's worth), an empty hand takes the output.
func use(stack: Dictionary) -> Dictionary:
	var res := {"consume": 0, "give": [], "msg": ""}
	var id: String = stack.get("id", "")
	if id == "":
		var o: Dictionary = inv.get_slot(OUTPUT)
		if o.is_empty():
			res["msg"] = "Nothing to take: load an ore or raw food, cable it to a generator."
		else:
			res["give"] = [o.duplicate()]
			inv.set_slot(OUTPUT, {})
		return res
	if recipes.smelting_for(id, station).is_empty():
		res["msg"] = "The furnace cannot smelt %s." % items.display_name(id)
		return res
	var cur: Dictionary = inv.get_slot(INPUT)
	if not cur.is_empty() and cur["id"] != id:
		res["msg"] = "It is loaded with %s." % items.display_name(cur["id"])
		return res
	var n := mini(int(stack["count"]), items.units(id))
	inv.set_slot(INPUT, {"id": id, "count": int(cur.get("count", 0)) + n})
	res["consume"] = n
	return res


func look_lines() -> Array[String]:
	var out: Array[String] = []
	var i: Dictionary = inv.get_slot(INPUT)
	if not i.is_empty():
		out.append("%s x%d, %d%% done" % [items.display_name(i["id"]), int(i["count"]), roundi(progress() * 100.0)])
	var o: Dictionary = inv.get_slot(OUTPUT)
	if not o.is_empty():
		out.append("Ready: %s x%d (empty hand takes it)" % [items.display_name(o["id"]), int(o["count"])])
	out.append("Powered: %d EU/t" % roundi(eu) if is_lit() else "No power: cable it to a turning turbine")
	return out


## An iron box with a dark door and a copper terminal on its back, lit orange through the door's slit.
func view_parts() -> Dictionary:
	var size := Vector3i(8, 8, 8)
	var c := WorkPalette.new_cells(size)
	WorkPalette.box(c, size, Vector3i(0, 0, 0), Vector3i(8, 8, 8), WorkPalette.IRON_DARK)
	WorkPalette.box(c, size, Vector3i(2, 2, 7), Vector3i(6, 6, 8), WorkPalette.SOOT)   # the door
	WorkPalette.box(c, size, Vector3i(3, 3, 7), Vector3i(5, 4, 8), WorkPalette.MOLTEN if is_lit() else WorkPalette.SOOT + 1)
	WorkPalette.box(c, size, Vector3i(3, 3, 0), Vector3i(5, 5, 1), WorkPalette.COPPER)   # the terminal
	return {"solid": {"cells": c, "size": size}}


func summary() -> Dictionary:
	var d := super()
	d["eu"] = eu
	return d
