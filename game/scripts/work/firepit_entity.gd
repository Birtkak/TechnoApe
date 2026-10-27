class_name FirepitEntity
extends RefCounted

## A firepit (ADR 0023, Vintage Story): sticks and stones on the ground. It keeps
## FurnaceEntity's slots (input, fuel, output, vessel), but fire needs a firestarter:
## ignite() lights it when there is fuel, then it burns fuel by itself until the fuel
## runs out (not only while something cooks, unlike Minecraft's furnace). While lit it
## cooks "smelt" recipes with station "firepit" from input to output, or, with a crucible
## in the vessel slot, runs the "melt" recipe for the input: `count` ores become the
## recipe's output vessel (a molten crucible).
## Since ADR 0042 it is worked by hand in the world, with no screen: use() lays fuel,
## food, ore and vessels on it one right click at a time and takes things off with an
## empty hand, and the view shows what lies on it. A "heatable" item in the input (an
## ingot, a smithing work piece) heats while the fire burns: its hot_until climbs to
## fully hot over Settings.metal_heat_s.
## The forge and the bloomery (ADR 0070) are firepits too, with their own station, fuel and heat:
## a smelt recipe may take `count` units of its input at once, and `hot` hands its output out hot.

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


## Fuel this fire takes (the firepit: anything that burns).
func burns(id: String) -> bool:
	return fuel_ticks(id) > 0


## Heatable metal this fire gets hot (the firepit: not iron, which needs a forge).
func heats(id: String) -> bool:
	return items.has_tag(id, "heatable") and not items.has_tag(id, "forge_heat")


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
	var r := recipes.smelting_for(input["id"], station)
	if r.is_empty() or int(input["count"]) < int(r.get("count", 1)) * items.units(input["id"]):
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
			return burns(id)
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
	if not recipes.first_of("melt", {"input": id}).is_empty() or not recipes.smelting_for(id, station).is_empty():
		return [INPUT]
	if burns(id):
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
		_heat_input()
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


const TICK := 0.05
var _heat_step := -1

## A heatable stack in the input warms toward fully hot (ADR 0042). The stack is changed in place,
## without the slot signal, so the cook progress and the view stay; the view redraws when the
## heat crosses a quarter.
func _heat_input() -> void:
	var s: Dictionary = inv.get_slot(INPUT)
	if s.is_empty() or not heats(s["id"]):
		_heat_step = -1
		return
	var h := minf(1.0, Inventory.heat_of(s) + TICK / Settings.metal_heat_s + TICK / Settings.metal_cool_s)
	s["hot_until"] = Inventory.clock_s + h * Settings.metal_cool_s
	var step := int(h * 4.0)
	if step != _heat_step:
		_heat_step = step
		changed.emit()


func _finish(r: Dictionary) -> void:
	made += 1
	if r["type"] == "melt":
		inv.take(INPUT, int(r.get("count", 1)) * items.units(inv.get_slot(INPUT)["id"]))
		inv.set_slot(VESSEL, {"id": r["output"]["id"], "count": 1})
		return
	inv.take(INPUT, int(r.get("count", 1)) * items.units(inv.get_slot(INPUT)["id"]))
	var o := inv.get_slot(OUTPUT)
	var out: Dictionary = r["output"]
	var made_stack := {"id": out["id"], "count": int(out["count"]) * items.units(out["id"]) + (int(o["count"]) if not o.is_empty() else 0)}
	if r.get("hot", false):
		made_stack["hot_until"] = Inventory.clock_s + Settings.metal_cool_s   # a glowing bloom (ADR 0070)
	inv.set_slot(OUTPUT, made_stack)


# ---------------- by hand (ADR 0042) ----------------

## Right click with `stack` (the held stack, {} for an empty hand) and its definition.
## Returns {consume: n, give: [stacks], ignited: bool, act: bool, msg: text}; act says something
## changed. With `dry` nothing changes: act says whether it would (ADR 0043: only acts are held).
## A consumed item keeps its per-stack data in the slot (a hot ingot stays hot, food its freshness).
func use(stack: Dictionary, def: Dictionary, dry := false) -> Dictionary:
	var res := {"consume": 0, "give": [], "ignited": false, "act": false, "msg": ""}
	var id: String = stack.get("id", "")
	if id == "":
		for slot in [OUTPUT, VESSEL, INPUT, FUEL]:
			if slot == FUEL and is_lit():
				continue
			if not inv.is_empty_slot(slot):
				res["act"] = true
				if not dry:
					res["give"] = [inv.take(slot)]
				return res
		res["msg"] = "Lay fuel on the %s, then light it with a firestarter." % title.to_lower()
		return res
	if "firestarter" in def.get("tags", []):
		if is_lit():
			res["msg"] = "The %s is burning." % title.to_lower()
		elif dry:
			res["act"] = fuel_ticks(inv.get_slot(FUEL).get("id", "")) > 0
		elif ignite():
			res["ignited"] = true
			res["act"] = true
			res["msg"] = "The %s is lit." % title.to_lower()
		else:
			res["msg"] = "Lay fuel on the %s first." % title.to_lower()
		return res
	var n := items.units(id)
	if may_place(VESSEL, id):
		if not inv.is_empty_slot(VESSEL):
			res["msg"] = "There is a vessel on the fire already."
			return res
		res["act"] = true
		if not dry:
			inv.set_slot(VESSEL, Inventory.make_stack(items, id, 1, stack))
			res["consume"] = 1
		return res
	if _cooks_here(id):
		var s := inv.get_slot(INPUT)
		if not s.is_empty() and (s["id"] != id or int(s["count"]) + n > items.max_stack(id)):
			res["msg"] = "Take the %s off first." % items.display_name(s["id"])
			return res
		res["act"] = true
		if dry:
			return res
		if s.is_empty():
			inv.set_slot(INPUT, Inventory.make_stack(items, id, n, stack))
		else:
			var merged := s.duplicate()
			Inventory.merge_into(merged, n, stack)
			inv.set_slot(INPUT, merged)
		res["consume"] = n
		return res
	if burns(id):
		var f := inv.get_slot(FUEL)
		if not f.is_empty() and f["id"] != id:
			res["msg"] = "The %s burns %s now." % [title.to_lower(), items.display_name(f["id"])]
			return res
		if int(f.get("count", 0)) / maxi(n, 1) >= Settings.firepit_fuel_max:
			res["msg"] = "The %s holds no more fuel." % title.to_lower()
			return res
		res["act"] = true
		if not dry:
			inv.set_slot(FUEL, {"id": id, "count": int(f.get("count", 0)) + n})
			res["consume"] = n
		return res
	var melt := recipes.first_of("melt", {"input": id})
	if items.has_tag(id, "heatable"):
		res["msg"] = "%s needs a forge's heat." % items.display_name(id)
	elif fuel_ticks(id) > 0:
		res["msg"] = "The %s burns only charcoal or coal." % title.to_lower()
	elif not melt.is_empty():
		res["msg"] = "Set a %s on the fire first." % items.display_name(melt["vessel"]).to_lower()
	else:
		res["msg"] = "%s does not go on a %s." % [items.display_name(id), title.to_lower()]
	return res


## Food and ore go on the fire (a cook recipe here, a melt recipe for the vessel on it), and so does
## anything heatable.
func _cooks_here(id: String) -> bool:
	if heats(id) or not recipes.smelting_for(id, station).is_empty():
		return true
	var vessel := inv.get_slot(VESSEL)
	return not vessel.is_empty() and not recipes.first_of("melt", {"input": id, "vessel": vessel["id"]}).is_empty()


## The look-at panel's lines (ADR 0078): fire, fuel, what is on it and what is ready.
func look_lines() -> Array[String]:
	var out: Array[String] = []
	var fuel := inv.get_slot(FUEL)
	if is_lit():
		out.append("Burning (%d%% of this fuel left)" % roundi(flame() * 100.0))
	elif fuel.is_empty():
		out.append("Not lit: lay fuel on, then use a firestarter")
	else:
		out.append("Not lit: light it with a firestarter")
	if not fuel.is_empty():
		out.append("Fuel: %s" % _count_name(fuel))
	var vessel := inv.get_slot(VESSEL)
	if not vessel.is_empty():
		out.append("Vessel: %s" % items.display_name(vessel["id"]))
	var input := inv.get_slot(INPUT)
	if not input.is_empty():
		var line := "On the fire: %s" % _count_name(input)
		if heats(input["id"]):
			line += " (%d%% hot)" % roundi(Inventory.heat_of(input) * 100.0)
		elif cook > 0:
			line += " (%d%% done)" % roundi(progress() * 100.0)
		out.append(line)
	var o := inv.get_slot(OUTPUT)
	if not o.is_empty():
		out.append("Ready: %s (empty hand takes it)" % _count_name(o))
	return out


## "3 Oak Log": counts in items, not voxels (a log is a block's worth of them, ADR 0034).
func _count_name(s: Dictionary) -> String:
	return "%d %s" % [int(s["count"]) / maxi(items.units(s["id"]), 1), items.display_name(s["id"])]


func contents() -> Array:
	var out := [{"id": kind, "count": 1}]   # the firepit, forge or bloomery item
	for s in inv.to_array():
		if not s.is_empty():
			out.append(s)
	return out


func view_parts() -> Dictionary:
	var size := Vector3i(8, 6, 8)
	var c := WorkPalette.new_cells(size)
	# a ring of big boulders (2 x 2 cells, one to three high, three stone tones) round a 4 x 4 bed of ash or
	# embers; charred logs when there is no fuel
	for z in range(0, 8, 2):
		for x in range(0, 8, 2):
			if x in [2, 4] and z in [2, 4]:
				continue
			var h := 1 + hash(Vector3i(x, 0, z)) % 3
			var tone: int = WorkPalette.WALL_STONE[hash(Vector3i(x, 1, z)) % 3]
			WorkPalette.box(c, size, Vector3i(x, 0, z), Vector3i(x + 2, h, z + 2), tone)
			WorkPalette.box(c, size, Vector3i(x, h - 1, z), Vector3i(x + 2, h, z + 2), WorkPalette.STONE_LIGHT)
	for z in range(2, 6):
		for x in range(2, 6):
			c[WorkPalette.idx(size, x, 0, z)] = (WorkPalette.EMBER if is_lit() else WorkPalette.ASH) + 1
	if inv.get_slot(FUEL).is_empty() and inv.get_slot(VESSEL).is_empty() and inv.get_slot(INPUT).is_empty():
		WorkPalette.box(c, size, Vector3i(2, 1, 3), Vector3i(6, 2, 4), WorkPalette.WOOD_DARK)
		WorkPalette.box(c, size, Vector3i(3, 1, 2), Vector3i(4, 2, 6), WorkPalette.CHARCOAL)
	# the fuel laid on: one more crossed stick per fuel item, up to firepit_fuel_max
	var fuel := inv.get_slot(FUEL)
	var logs := 0 if fuel.is_empty() else int(fuel["count"]) / maxi(items.units(fuel["id"]), 1)
	for k in mini(logs, Settings.firepit_fuel_max):
		var y := mini(1 + k / 2, 4)
		var o := k / 2 - 1
		for i in range(1, 7):
			if k % 2 == 0:
				c[WorkPalette.idx(size, i, y, clampi(i + o, 0, 7))] = WorkPalette.WOOD + 1
			else:
				c[WorkPalette.idx(size, 7 - i, y, clampi(i + o, 0, 7))] = WorkPalette.WOOD_DARK + 1
	var vessel := inv.get_slot(VESSEL)
	if not vessel.is_empty():
		WorkPalette.box(c, size, Vector3i(2, 2, 2), Vector3i(6, 6, 6), WorkPalette.FIRED)
		var fill := WorkPalette.MOLTEN if _is_melt_output(vessel["id"]) else WorkPalette.ASH
		WorkPalette.box(c, size, Vector3i(3, 5, 3), Vector3i(5, 6, 5), fill)
	elif is_lit():
		for p in [Vector3i(3, 2, 3), Vector3i(4, 2, 4), Vector3i(4, 3, 3), Vector3i(3, 2, 4)]:
			c[WorkPalette.idx(size, p.x, p.y, p.z)] = WorkPalette.EMBER + 1
	# what lies on it, drawn as the items themselves (ADR 0042): food or metal over the fire, the
	# cooked things at its edge, ore beside the vessel it goes into
	var laid := []
	var input := inv.get_slot(INPUT)
	if not input.is_empty() and vessel.is_empty():
		laid.append({"id": input["id"], "at": Vector3(0.5, 0.42, 0.5), "yaw": 20.0, "scale": 0.5})
	var out := inv.get_slot(OUTPUT)
	if not out.is_empty():
		laid.append({"id": out["id"], "at": Vector3(0.86, 0.03, 0.5), "yaw": 70.0, "scale": 0.4})
	if not vessel.is_empty() and not input.is_empty():
		laid.append({"id": input["id"], "at": Vector3(0.14, 0.03, 0.5), "yaw": -40.0, "scale": 0.32})
	return {"solid": {"cells": c, "size": size}, "items": laid}


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
