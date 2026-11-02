class_name MachineEntity
extends RefCounted

## A low-voltage machine (ADR 0113, 0117): its `kind` is its item id and the item carries
## `"machine": {"eu_t": <want>, "inputs": <slots, default 1>}`. It runs the `process` recipes (ADR 0115)
## whose station is its id: ingredients (ids or #tags, a spec per unit) in its input slots, one output stack,
## `seconds` of work while the energy network gives it eu_t (a second of power per solve). Worked by hand
## like a work piece: a right click with something a recipe takes loads a block's worth, an empty hand takes
## the output; hoppers and belts load and unload it through Transport. One class for the lathe, bending
## press, wiremill, macerator, alloy smelter and assembler: a new machine is an item plus recipes.

signal changed

const DOOR := {"lathe": WorkPalette.BRONZE, "bending_press": WorkPalette.ANVIL, "wiremill": WorkPalette.COPPER,
	"macerator": WorkPalette.GRAVEL, "alloy_smelter": WorkPalette.MOLTEN, "assembler": WorkPalette.GLOW}

var kind: String
var title: String
var pos: Vector3i
var items: ItemRegistry
var recipes: RecipeRegistry
var inv: Inventory          # the input slots
var output := {}            # the one output stack
var eu_t := 8.0
var eu := 0.0               # what the last solve delivered
var power_ticks := 0        # ticks of power left from the last solve
var progress := 0           # ticks worked on the current recipe
var total := 20
var made := 0
var _list: Array = []       # this station's process recipes


func _init(p_items: ItemRegistry, p_recipes: RecipeRegistry, p_pos: Vector3i, p_kind: String) -> void:
	items = p_items
	recipes = p_recipes
	pos = p_pos
	kind = p_kind
	var def := p_items.get_def(p_kind)
	title = String(def.get("name", "Machine"))
	var m: Dictionary = def.get("machine", {})
	eu_t = float(m.get("eu_t", 8.0))
	inv = Inventory.new(items, int(m.get("inputs", 1)))
	for rid in recipes.by_station(kind):
		if recipes.recipes[rid].get("type", "") == "process":
			_list.append(recipes.recipes[rid])


## A recipe's ingredient specs grouped: spec -> units needed (a spec per unit, like the crusher's ore block).
static func needs_of(r: Dictionary, p_items: ItemRegistry) -> Dictionary:
	var out := {}
	for spec in r["ingredients"]:
		var unit := p_items.units(String(spec).trim_prefix("#")) if not String(spec).begins_with("#") else 1
		out[spec] = int(out.get(spec, 0)) + unit
	return out


## The slot holding something that matches `spec` with at least `n`, or -1.
func _slot_for(spec: String, n: int) -> int:
	for i in inv.size():
		var s := inv.get_slot(i)
		if not s.is_empty() and items.matches(String(s["id"]), spec) and int(s["count"]) >= n:
			return i
	return -1


## The first recipe the inputs satisfy with room in the output, or {}.
func current() -> Dictionary:
	for r in _list:
		var ok := true
		for spec in needs_of(r, items):
			if _slot_for(spec, needs_of(r, items)[spec]) < 0:
				ok = false
				break
		if ok and _room_for(r["output"]):
			return r
	return {}


func _room_for(out: Dictionary) -> bool:
	if output.is_empty():
		return true
	if output["id"] != out["id"]:
		return false
	return int(output["count"]) + int(out["count"]) * items.units(out["id"]) <= items.max_stack(out["id"])


func is_lit() -> bool:
	return power_ticks > 0 and not current().is_empty()


func powered() -> bool:
	return power_ticks > 0


func net(channel: String) -> Dictionary:
	if channel != "energy":
		return {}
	return {"role": NetworkSolver.SINK, "amount": eu_t if not current().is_empty() else 0.0}


func net_in(channel: String, amount: float) -> void:
	if channel != "energy":
		return
	eu = amount
	var was := is_lit()
	power_ticks = 21 if amount >= eu_t - 1e-3 else 0   # one solve interval plus a tick
	if was != is_lit():
		changed.emit()


## One game tick: work while powered and fed. Returns true when the lit state flipped.
func tick() -> bool:
	var was := is_lit()
	if power_ticks > 0:
		power_ticks -= 1
	var r := current()
	if r.is_empty() or power_ticks <= 0:
		if r.is_empty():
			progress = 0
		return was != is_lit()
	total = maxi(1, int(round(float(r.get("seconds", 1.0)) * 20.0)))
	progress += 1
	if progress >= total:
		progress = 0
		_finish(r)
	return was != is_lit()


func _finish(r: Dictionary) -> void:
	var needs := needs_of(r, items)
	for spec in needs:
		inv.take(_slot_for(spec, needs[spec]), needs[spec])
	var out: Dictionary = r["output"]
	var n := int(out["count"]) * items.units(out["id"])
	output = {"id": out["id"], "count": int(output.get("count", 0)) + n} if not output.is_empty() else {"id": out["id"], "count": n}
	made += n
	changed.emit()


## Whether any recipe here takes `id`.
func takes(id: String) -> bool:
	for r in _list:
		for spec in r["ingredients"]:
			if items.matches(id, String(spec)):
				return true
	return false


## Right click with the held stack: the stack goes in as far as it fits, an empty hand takes the output.
## {consume, give, msg}.
func use(stack: Dictionary) -> Dictionary:
	var res := {"consume": 0, "give": [], "msg": ""}
	var id: String = stack.get("id", "")
	if id == "":
		if output.is_empty():
			res["msg"] = "Nothing to take: load it, cable it to a generator."
		else:
			res["give"] = [output.duplicate()]
			output = {}
			changed.emit()
		return res
	if not takes(id):
		res["msg"] = "The %s takes no %s." % [title.to_lower(), items.display_name(id)]
		return res
	var n := int(stack["count"])   # the whole held stack: a recipe may want three ingots
	var left := inv.add(id, n, stack)
	res["consume"] = n - left
	if res["consume"] == 0:
		res["msg"] = "It is full."
	else:
		changed.emit()
	return res


## One finished item for a hopper, or {}.
func take_one() -> Dictionary:
	if output.is_empty():
		return {}
	output["count"] = int(output["count"]) - 1
	var out := {"id": output["id"], "count": 1}
	if int(output["count"]) <= 0:
		output = {}
	changed.emit()
	return out


func contents() -> Array:
	var out := []
	for s in inv.to_array():
		if not s.is_empty():
			out.append(s)
	if not output.is_empty():
		out.append(output)
	return out


func progress_frac() -> float:
	return float(progress) / float(maxi(total, 1))


func look_lines() -> Array[String]:
	var out: Array[String] = []
	for s in inv.to_array():
		if not s.is_empty():
			out.append("%s x%d" % [items.display_name(s["id"]), int(s["count"])])
	if not current().is_empty():
		out.append("%s: %d%% done" % [items.display_name(current()["output"]["id"]), roundi(progress_frac() * 100.0)])
	if not output.is_empty():
		out.append("Ready: %s x%d (empty hand takes it)" % [items.display_name(output["id"]), int(output["count"])])
	out.append("Powered: %d EU/t" % roundi(eu) if powered() else "No power: cable it to a generator (%d EU/t)" % roundi(eu_t))
	return out


## An iron box with a dark door and a copper terminal on its back; the door's slit shows `door` while lit.
static func box_model(door: int, lit: bool) -> Dictionary:
	var size := Vector3i(8, 8, 8)
	var c := WorkPalette.new_cells(size)
	WorkPalette.box(c, size, Vector3i(0, 0, 0), Vector3i(8, 8, 8), WorkPalette.IRON_DARK)
	WorkPalette.box(c, size, Vector3i(2, 2, 7), Vector3i(6, 6, 8), WorkPalette.SOOT)   # the door
	WorkPalette.box(c, size, Vector3i(3, 3, 7), Vector3i(5, 4, 8), door if lit else WorkPalette.SOOT + 1)
	WorkPalette.box(c, size, Vector3i(3, 3, 0), Vector3i(5, 5, 1), WorkPalette.COPPER)   # the terminal
	return {"solid": {"cells": c, "size": size}}


func view_parts() -> Dictionary:
	return box_model(int(DOOR.get(kind, WorkPalette.MOLTEN)), is_lit())


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "slots": inv.to_array(), "output": output, "progress": progress}


func load_from(d: Dictionary) -> void:
	var slots: Array = d.get("slots", [])
	for i in mini(slots.size(), inv.size()):
		inv.set_slot(i, slots[i])
	output = d.get("output", {})
	progress = int(d.get("progress", 0))


func summary() -> Dictionary:
	return {"kind": kind, "inputs": inv.to_array(), "output": output, "progress": progress, "eu": eu, "made": made}
