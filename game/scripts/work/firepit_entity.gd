class_name FirepitEntity
extends RefCounted

## A firepit (ADR 0023, Vintage Story): sticks and stones on the ground. Same
## container interface as FurnaceEntity (the furnace screen plus a vessel
## slot), but fire needs a firestarter: ignite() lights it when there is fuel,
## then it burns fuel by itself until the fuel runs out (not only while
## something cooks, unlike Minecraft's furnace). While lit it cooks "smelt"
## recipes with station "firepit" from input to output, or, with a crucible
## in the vessel slot, runs the "melt" recipe for the input: `count` ores
## become the recipe's output vessel (a molten crucible).

signal changed

const INPUT := 0
const FUEL := 1
const OUTPUT := 2
const VESSEL := 3
const STATION := "firepit"

var kind := "firepit"
var title := "Firepit"
var station := STATION
var pos: Vector3i
var inv: Inventory
var items: ItemRegistry
var recipes: RecipeRegistry
var lit_time := 0
var lit_duration := 0
var cook := 0
var cook_total := 200
var made := 0
var _was_lit := false
var _last_key := "|"


func _init(p_items: ItemRegistry, p_recipes: RecipeRegistry, p_pos: Vector3i) -> void:
	items = p_items
	recipes = p_recipes
	pos = p_pos
	inv = Inventory.new(items, 4)
	inv.changed.connect(_on_slot_changed)


## Like the furnace: the progress resets only when a different item goes in.
func _on_slot_changed(i: int) -> void:
	if i == INPUT or i == VESSEL:
		var key: String = inv.get_slot(INPUT).get("id", "") + "|" + inv.get_slot(VESSEL).get("id", "")
		if key != _last_key:
			_last_key = key
			cook = 0
	changed.emit()


func fuel_ticks(id: String) -> int:
	if id == "" or not items.has(id):
		return 0
	return int(items.get_def(id).get("fuel_ticks", 0))


func is_lit() -> bool:
	return lit_time > 0


## Firestarter: lights the fuel. False when there is none.
func ignite() -> bool:
	if is_lit():
		return true
	if not _burn_next():
		return false
	changed.emit()
	return true


func _burn_next() -> bool:
	var fuel := inv.get_slot(FUEL)
	var t := fuel_ticks(fuel.get("id", ""))
	if t <= 0 or int(fuel.get("count", 0)) < items.units(fuel["id"]):
		return false
	lit_time = t
	lit_duration = t
	inv.take(FUEL, items.units(fuel["id"]))   # logs burn a block's worth of voxels at a time (ADR 0034)
	return true


## The recipe the current slots run: a melt (crucible in the vessel slot) or a cook.
func current_recipe() -> Dictionary:
	var input := inv.get_slot(INPUT)
	if input.is_empty():
		return {}
	var vessel := inv.get_slot(VESSEL)
	if not vessel.is_empty():
		var m := recipes.first_of("melt", {"input": input["id"], "vessel": vessel["id"]})
		if not m.is_empty() and int(input["count"]) >= int(m.get("count", 1)) * items.units(input["id"]):
			return m
		return {}
	var r := recipes.smelting_for(input["id"], STATION)
	if r.is_empty() or int(input["count"]) < items.units(input["id"]):
		return {}
	var o := inv.get_slot(OUTPUT)
	if not o.is_empty() and (o["id"] != r["output"]["id"] or int(o["count"]) + int(r["output"]["count"]) > items.max_stack(o["id"])):
		return {}
	return r


func may_place(slot: int, id: String) -> bool:
	match slot:
		INPUT:
			return true
		FUEL:
			return fuel_ticks(id) > 0
		VESSEL:
			return not recipes.first_of("melt", {"vessel": id}).is_empty() or _is_melt_output(id)
	return false


func _is_melt_output(id: String) -> bool:
	return not recipes.first_of("melt", {"output": id}).is_empty()


func is_output(slot: int) -> bool:
	return slot == OUTPUT


func shift_targets(id: String) -> Array:
	if may_place(VESSEL, id):
		return [VESSEL]
	if not recipes.first_of("melt", {"input": id}).is_empty() or not recipes.smelting_for(id, STATION).is_empty():
		return [INPUT]
	if fuel_ticks(id) > 0:
		return [FUEL]
	return []


func shift_out_reverse(slot: int) -> bool:
	return slot == OUTPUT


func flame() -> float:
	return float(lit_time) / float(maxi(lit_duration, 1)) if is_lit() else 0.0


func progress() -> float:
	return float(cook) / float(maxi(cook_total, 1))


func tick() -> bool:
	if is_lit():
		lit_time -= 1
		if lit_time == 0:
			_burn_next()   # keeps burning while there is fuel
	if is_lit():
		var r := current_recipe()
		if r.is_empty():
			cook = 0
		else:
			cook_total = int(r.get("ticks", 200))
			cook += 1
			if cook >= cook_total:
				cook = 0
				_finish(r)
	elif cook > 0:
		cook = maxi(cook - 2, 0)
	if _was_lit != is_lit():
		_was_lit = is_lit()
		changed.emit()   # the view swaps embers in or out
	return false


func _finish(r: Dictionary) -> void:
	made += 1
	if r["type"] == "melt":
		inv.take(INPUT, int(r.get("count", 1)) * items.units(inv.get_slot(INPUT)["id"]))
		inv.set_slot(VESSEL, {"id": r["output"]["id"], "count": 1})
		return
	inv.take(INPUT, items.units(inv.get_slot(INPUT)["id"]))
	var o := inv.get_slot(OUTPUT)
	var out: Dictionary = r["output"]
	inv.set_slot(OUTPUT, {"id": out["id"], "count": int(out["count"]) * items.units(out["id"]) + (int(o["count"]) if not o.is_empty() else 0)})


func contents() -> Array:
	var out := [{"id": "firepit", "count": 1}]
	for s in inv.to_array():
		if not s.is_empty():
			out.append(s)
	return out


func view_parts() -> Dictionary:
	var size := Vector3i(8, 6, 8)
	var c := WorkPalette.new_cells(size)
	# a ring of stones, crossed sticks, ash or embers in the middle
	for z in 8:
		for x in 8:
			var dx := x - 3.5
			var dz := z - 3.5
			var r2 := dx * dx + dz * dz
			if r2 > 9.0 and r2 < 16.5 and (x + z) % 3 != 0:
				c[WorkPalette.idx(size, x, 0, z)] = WorkPalette.STONE + 1
			elif r2 <= 6.5:
				c[WorkPalette.idx(size, x, 0, z)] = (WorkPalette.EMBER if is_lit() else WorkPalette.ASH) + 1
	for i in range(1, 7):
		c[WorkPalette.idx(size, i, 1, i)] = WorkPalette.WOOD + 1
		c[WorkPalette.idx(size, 7 - i, 1, i)] = WorkPalette.WOOD_DARK + 1
	var vessel := inv.get_slot(VESSEL)
	if not vessel.is_empty():
		WorkPalette.box(c, size, Vector3i(2, 2, 2), Vector3i(6, 6, 6), WorkPalette.FIRED)
		var fill := WorkPalette.MOLTEN if _is_melt_output(vessel["id"]) else WorkPalette.ASH
		WorkPalette.box(c, size, Vector3i(3, 5, 3), Vector3i(5, 6, 5), fill)
	elif is_lit():
		for p in [Vector3i(3, 2, 3), Vector3i(4, 2, 4), Vector3i(4, 3, 3), Vector3i(3, 2, 4)]:
			c[WorkPalette.idx(size, p.x, p.y, p.z)] = WorkPalette.EMBER + 1
	return {"solid": {"cells": c, "size": size}}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "slots": inv.to_array(), "lit_time": lit_time,
		"lit_duration": lit_duration, "cook": cook}


func load_from(d: Dictionary) -> void:
	var slots: Array = d.get("slots", [])
	for i in mini(slots.size(), 4):
		inv.set_slot(i, slots[i])
	lit_time = int(d.get("lit_time", 0))
	lit_duration = int(d.get("lit_duration", 0))
	cook = int(d.get("cook", 0))
	_was_lit = is_lit()


func summary() -> Dictionary:
	return {"kind": kind, "pos": pos, "input": inv.get_slot(INPUT), "fuel": inv.get_slot(FUEL),
		"output": inv.get_slot(OUTPUT), "vessel": inv.get_slot(VESSEL), "lit": is_lit(), "cook": cook, "made": made}
