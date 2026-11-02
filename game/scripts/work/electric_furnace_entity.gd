class_name ElectricFurnaceEntity
extends FurnaceEntity

## The electric furnace (ADR 0113): a furnace with no fuel slot that runs every furnace smelt recipe while
## the energy network gives it its `machine.eu_t` (a second of fire per solve). Worked by hand like a work
## piece: a right click with a smeltable loads the input, an empty hand takes the output; hoppers load and
## unload it through Transport.

var eu := 0.0        # what the last solve delivered, for the look-at panel
var eu_t := 16.0


func _init(p_items: ItemRegistry, p_recipes: RecipeRegistry, p_pos: Vector3i) -> void:
	super(p_items, p_recipes, p_pos)
	kind = "electric_furnace"
	title = "Electric Furnace"
	eu_t = float(p_items.get_def(kind).get("machine", {}).get("eu_t", 16.0))


func net(channel: String) -> Dictionary:
	if channel != "energy":
		return {}
	var r := recipes.smelting_for(inv.get_slot(INPUT).get("id", ""), station) if not inv.is_empty_slot(INPUT) else {}
	return {"role": NetworkSolver.SINK, "amount": eu_t if _can_burn(r) else 0.0}


## Enough power for the coming second lights it; anything less and it goes dark.
func net_in(channel: String, amount: float) -> void:
	if channel != "energy":
		return
	eu = amount
	var was := is_lit()
	lit_time = 21 if amount >= eu_t - 1e-3 else 0   # one solve interval plus a tick
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
	out.append("Powered: %d EU/t" % roundi(eu) if is_lit() else "No power: cable it to a generator (%d EU/t)" % roundi(eu_t))
	return out


func view_parts() -> Dictionary:
	return MachineEntity.box_model(WorkPalette.MOLTEN, is_lit())


func summary() -> Dictionary:
	var d := super()
	d["eu"] = eu
	return d
