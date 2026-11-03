class_name MachineEntity
extends RefCounted

## A low-voltage machine (ADR 0113, 0117): its `kind` is its item id and the item carries
## `"machine": {"eu_t": <want>, "inputs": <slots, default 1>}`. It runs the `process` recipes (ADR 0115)
## whose station is its id: ingredients (ids or #tags, a spec per unit) in its input slots, one output stack,
## `seconds` of work while the energy network gives it eu_t (a second of power per solve). Worked by hand
## like a work piece: a right click with something a recipe takes loads a block's worth, an empty hand takes
## the output; hoppers and belts load and unload it through Transport. One class for the lathe, bending
## press, wiremill, macerator, alloy smelter, assembler and the ADR 0131 machines: a new machine is an item,
## its recipes and a DOOR entry (BlockEntities builds every DOOR kind as a MachineEntity, 1 m like the lathe).

signal changed

const DOOR := {"lathe": WorkPalette.BRONZE, "bending_press": WorkPalette.ANVIL, "wiremill": WorkPalette.COPPER,
	"macerator": WorkPalette.GRAVEL, "alloy_smelter": WorkPalette.MOLTEN, "assembler": WorkPalette.GLOW,
	"extractor": WorkPalette.GLOW, "cutter": WorkPalette.STEEL, "extruder": WorkPalette.MOLTEN, "canning_machine": WorkPalette.COPPER,
	"electric_compressor": WorkPalette.STEEL, "electrolyzer": WorkPalette.GLOW}

var kind: String
var title: String
var pos: Vector3i
var items: ItemRegistry
var recipes: RecipeRegistry
var inv: Inventory          # the input slots
var output := {}            # the one output stack
var load: PowerLoad         # its draw on the LV network (ADR 0123); null for an unpowered machine (the coke oven)
var power_ticks := 0        # ticks of power left from the last solve
var progress := 0           # ticks worked on the current recipe
var total := 20
var made := 0
var enabled := true         # a machine with eu_t 0 (the coke oven) runs while this is set (formed)
var facing := 0             # quarter turns; a wrench turns it (ADR 0097's rule, ADR 0117)
var _list: Array = []       # this station's process recipes


func _init(p_items: ItemRegistry, p_recipes: RecipeRegistry, p_pos: Vector3i, p_kind: String) -> void:
	items = p_items
	recipes = p_recipes
	pos = p_pos
	kind = p_kind
	var def := p_items.get_def(p_kind)
	title = String(def.get("name", "Machine"))
	var m: Dictionary = def.get("machine", {})
	if float(m.get("watts", 0.0)) > 0.0:
		load = PowerLoad.new(float(m["watts"]))
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
	return powered() and not current().is_empty()


func powered() -> bool:
	return power_ticks > 0 or (load == null and enabled)


func net(channel: String) -> Dictionary:
	if channel != "energy" or load == null:
		return {}
	return load.net(not current().is_empty())


func net_in(channel: String, d) -> bool:
	if channel != "energy" or load == null:
		return false
	var was := is_lit()
	var started := load.net_in(d)
	power_ticks = 21 if load.on else 0   # one solve interval plus a tick
	if was != is_lit():
		changed.emit()
	return started


## One game tick: work while powered and fed. Returns true when the lit state flipped.
func tick() -> bool:
	var was := is_lit()
	if power_ticks > 0:
		power_ticks -= 1
	var r := current()
	if r.is_empty() or not powered():
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
	if load != null:
		out.append(("Running, " if powered() else "No power: ") + load.status())
	return out


## The machine hull (ADR 0129, industrial like the turbine), on a 16-cell grid: an iron plinth with a
## hazard-striped front edge, dark iron corner posts and bands framing engine-green panels, vent slits in the
## sides, a riveted steel-framed window in the front (+z) that shows `door` while lit, the copper terminal at
## the back, and on top a fitting three cells tall that tells the kinds apart (`top` = the kind).
static func box_model(door: int, lit: bool, top: String = "") -> Dictionary:
	var n := 16
	var size := Vector3i(n, n, n)
	var c := WorkPalette.new_cells(size)
	var b := func(a: Vector3i, z: Vector3i, paint: int) -> void:
		WorkPalette.box(c, size, a, z, paint)
	b.call(Vector3i(0, 0, 0), Vector3i(n, 2, n), WorkPalette.IRON_DARK)   # the plinth
	WorkPalette.hazard_box(c, size, Vector3i(0, 0, n - 1), Vector3i(n, 2, n))
	b.call(Vector3i(0, 2, 0), Vector3i(n, 12, n), WorkPalette.PAINT)   # the panels
	for q in [Vector2i(0, 0), Vector2i(n - 2, 0), Vector2i(0, n - 2), Vector2i(n - 2, n - 2)]:
		b.call(Vector3i(q.x, 2, q.y), Vector3i(q.x + 2, 12, q.y + 2), WorkPalette.IRON_DARK)   # the posts
	b.call(Vector3i(0, 11, 0), Vector3i(n, 13, n), WorkPalette.IRON_DARK)   # the top band
	b.call(Vector3i(2, 12, 2), Vector3i(n - 2, 13, n - 2), WorkPalette.STEEL)   # the lid
	for y in [5, 8]:   # the vents
		b.call(Vector3i(0, y, 4), Vector3i(n, y + 1, 12), WorkPalette.SOOT)
	b.call(Vector3i(2, 2, 2), Vector3i(n - 2, 11, n - 2), WorkPalette.PAINT)
	b.call(Vector3i(4, 4, n - 1), Vector3i(12, 11, n), WorkPalette.STEEL)   # the window frame
	b.call(Vector3i(5, 5, n - 1), Vector3i(11, 10, n), WorkPalette.SOOT)
	if lit:
		b.call(Vector3i(5, 5, n - 1), Vector3i(11, 7, n), door)
	for p in [Vector2i(4, 4), Vector2i(11, 4), Vector2i(4, 10), Vector2i(11, 10)]:
		c[WorkPalette.idx(size, p.x, p.y, n - 1)] = WorkPalette.RIVET + 1
	b.call(Vector3i(4, 4, 0), Vector3i(12, 10, 1), WorkPalette.IRON_DARK)   # the terminal plate
	b.call(Vector3i(6, 6, 0), Vector3i(10, 8, 1), WorkPalette.COPPER)
	var t := 13   # the fittings stand on the lid, y 13..15
	match top:
		"lathe":   # the headstock, a bronze chuck along x, the tailstock
			b.call(Vector3i(1, t, 5), Vector3i(5, n, 11), WorkPalette.IRON_DARK)
			WorkPalette.drum(c, size, 5, 12, 14.5, 8.0, 1.6, WorkPalette.BRONZE)
			b.call(Vector3i(12, t, 6), Vector3i(15, 15, 10), WorkPalette.IRON_DARK)
		"bending_press":   # the ram on two posts, a hazard edge on its face
			for x in [3, 12]:
				b.call(Vector3i(x, t, 7), Vector3i(x + 1, n, 9), WorkPalette.IRON_DARK)
			b.call(Vector3i(4, 14, 4), Vector3i(12, n, 12), WorkPalette.ANVIL)
			WorkPalette.hazard_box(c, size, Vector3i(4, 14, 11), Vector3i(12, n, 12))
		"wiremill":   # a spool of copper wire between iron cheeks
			WorkPalette.drum(c, size, 5, 11, 14.5, 8.0, 1.6, WorkPalette.COPPER)
			for x in [4, 11]:
				WorkPalette.drum(c, size, x, x + 1, 14.5, 8.0, 2.4, WorkPalette.IRON_DARK)
		"macerator":   # the steel feed hopper, ore in it
			b.call(Vector3i(2, t, 2), Vector3i(14, n, 14), WorkPalette.STEEL)
			b.call(Vector3i(3, t, 3), Vector3i(13, 15, 13), WorkPalette.STONE_DARK)   # ore in it, dark against the steel
		"alloy_smelter", "electric_furnace":   # a brick stack with an iron band, sooty mouth
			WorkPalette.wall_box(c, size, Vector3i(9, t, 2), Vector3i(14, n, 7), WorkPalette.WALL_BRICK, WorkPalette.BRICK_DARK)
			b.call(Vector3i(9, 14, 2), Vector3i(14, 15, 7), WorkPalette.IRON_DARK)
			b.call(Vector3i(10, 15, 3), Vector3i(13, n, 6), WorkPalette.SOOT)
		"assembler":   # an arm on a post with a yellow boom and a dark gripper
			b.call(Vector3i(3, t, 6), Vector3i(7, 14, 10), WorkPalette.IRON_DARK)
			b.call(Vector3i(4, t, 7), Vector3i(6, n, 9), WorkPalette.IRON_DARK)
			b.call(Vector3i(6, 15, 7), Vector3i(12, n, 9), WorkPalette.HAZARD)
			b.call(Vector3i(12, 14, 7), Vector3i(13, n, 9), WorkPalette.IRON_DARK)
		"extractor":   # a screw press: a bronze screw through an iron beam over a copper drip tray
			b.call(Vector3i(3, t, 3), Vector3i(13, 14, 13), WorkPalette.COPPER)
			for x in [3, 12]:
				b.call(Vector3i(x, t, 7), Vector3i(x + 1, n, 9), WorkPalette.IRON_DARK)
			b.call(Vector3i(3, 15, 7), Vector3i(13, n, 9), WorkPalette.IRON_DARK)
			b.call(Vector3i(7, 14, 7), Vector3i(9, n, 9), WorkPalette.BRONZE)
		"cutter":   # a saw blade standing up through the table, a hazard edge
			b.call(Vector3i(2, t, 4), Vector3i(14, 14, 12), WorkPalette.IRON_DARK)
			WorkPalette.hazard_box(c, size, Vector3i(2, t, 11), Vector3i(14, 14, 12))
			for x in range(4, 13):
				for y in range(14, n):
					if Vector2(x + 0.5 - 8.0, y + 0.5 - 13.0).length() <= 3.2:
						c[WorkPalette.idx(size, x, y, 7)] = WorkPalette.STEEL + 1
		"extruder":   # the die block with a hot billet pushing out of it
			b.call(Vector3i(2, t, 4), Vector3i(9, n, 12), WorkPalette.STEEL)
			b.call(Vector3i(8, t, 5), Vector3i(9, n, 11), WorkPalette.IRON_DARK)
			b.call(Vector3i(9, t, 6), Vector3i(14, 15, 10), WorkPalette.MOLTEN)
		"canning_machine":   # tin cans on a copper rail under the sealing head
			b.call(Vector3i(2, t, 5), Vector3i(14, 14, 7), WorkPalette.COPPER)
			for x in [3, 7, 11]:
				b.call(Vector3i(x, t, 8), Vector3i(x + 2, 15, 11), WorkPalette.TIN)
			b.call(Vector3i(6, 15, 7), Vector3i(10, n, 12), WorkPalette.IRON_DARK)
		"electric_compressor":   # the steel ram between two posts under a crossbeam
			for x in [2, 12]:
				b.call(Vector3i(x, t, 6), Vector3i(x + 2, n, 10), WorkPalette.IRON_DARK)
			b.call(Vector3i(2, 15, 6), Vector3i(14, n, 10), WorkPalette.IRON_DARK)
			b.call(Vector3i(5, t, 5), Vector3i(11, 15, 11), WorkPalette.STEEL)
			WorkPalette.hazard_box(c, size, Vector3i(5, 14, 10), Vector3i(11, 15, 11))
		"electrolyzer":   # a glowing cell in an iron rim, two dark electrodes with copper caps
			b.call(Vector3i(2, t, 2), Vector3i(14, 15, 14), WorkPalette.IRON_DARK)
			b.call(Vector3i(3, t, 3), Vector3i(13, 15, 13), WorkPalette.GLOW)
			for x in [5, 10]:
				b.call(Vector3i(x, t, 7), Vector3i(x + 1, n, 9), WorkPalette.SOOT)
				b.call(Vector3i(x, 15, 7), Vector3i(x + 1, n, 9), WorkPalette.COPPER)
	return {"solid": {"cells": c, "size": size, "grid": n}}


func view_parts() -> Dictionary:
	return turned(box_model(int(DOOR.get(kind, WorkPalette.MOLTEN)), is_lit(), kind), facing)


## A view turned `quarters` about up (every part of it).
static func turned(view: Dictionary, quarters: int) -> Dictionary:
	if quarters % 4 == 0:
		return view
	for k in view:
		var part: Dictionary = view[k]
		if part.has("cells") and part.has("size"):
			part["cells"] = WorkPalette.rotate_y(part["cells"], part["size"], quarters)
	return view


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "slots": inv.to_array(), "output": output, "progress": progress, "facing": facing}


func load_from(d: Dictionary) -> void:
	var slots: Array = d.get("slots", [])
	for i in mini(slots.size(), inv.size()):
		inv.set_slot(i, slots[i])
	output = d.get("output", {})
	progress = int(d.get("progress", 0))
	facing = int(d.get("facing", 0))


func summary() -> Dictionary:
	return {"kind": kind, "inputs": inv.to_array(), "output": output, "progress": progress, "watts": load.drawn if load else 0.0, "made": made}
