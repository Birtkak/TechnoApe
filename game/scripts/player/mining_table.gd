class_name MiningTable
extends RefCounted

## Mining through the material model (ADR 0005), loaded from data/mining.json.
## Pure logic: given a material name and the held item definition it answers
## "how long does breaking take" and "what drops". Engine-agnostic, headless
## testable. Materials missing from the table are unbreakable (safe default).

var materials := {}          # material name -> entry
var hand_speed := 1.0
var tier_speed: Array = [2.0, 4.0, 6.0, 8.0, 10.0]
# the right tool breaks in hardness * factor / tier speed, kept within [min_s, max_s];
# hands or the wrong tool on a material that wants a tool take hardness * s_per_hardness
var right_tool := {"factor": 3.0, "min_s": 0.5, "max_s": 10.0}
var wrong_tool := {"s_per_hardness": 80.0, "min_s": 20.0, "max_s": 120.0}
var errors: Array[String] = []


static func load_default() -> MiningTable:
	var t := MiningTable.new()
	t.load_file("res://data/mining.json")
	return t


func load_file(path: String) -> bool:
	var f := FileAccess.open(path, FileAccess.READ)
	if f == null:
		errors.append("cannot open %s" % path)
		return false
	return load_json(f.get_as_text())


func load_json(text: String) -> bool:
	var data = JSON.parse_string(text)
	if typeof(data) != TYPE_DICTIONARY or not data.has("materials"):
		errors.append("mining.json: expected {materials: {...}}")
		return false
	var speeds: Dictionary = data.get("tool_speed", {})
	hand_speed = float(speeds.get("hand", 1.0))
	if speeds.has("tier"):
		tier_speed = speeds["tier"]
	right_tool.merge(data.get("right_tool", {}), true)
	wrong_tool.merge(data.get("wrong_tool", {}), true)
	for name in data["materials"]:
		var e = data["materials"][name]
		if typeof(e) != TYPE_DICTIONARY or not e.has("hardness"):
			errors.append("mining.json: %s needs hardness" % name)
			continue
		materials[name] = e
	return errors.is_empty()


func is_minable(material_name: String) -> bool:
	return materials.has(material_name)


func entry(material_name: String) -> Dictionary:
	return materials.get(material_name, {})


## Tool class of an item definition ("pickaxe", "shovel", "axe") or "".
static func tool_class(held: Dictionary) -> String:
	for t in held.get("tags", []):
		if t in ["pickaxe", "shovel", "axe", "hoe", "shears", "knife"]:
			return t
	return ""


## True when `held` is the right tool class and tier for the material.
func tool_fits(material_name: String, held: Dictionary) -> bool:
	var e := entry(material_name)
	if e.is_empty():
		return false
	var needed: String = e.get("tool", "")
	if needed == "":
		return true
	if tool_class(held) != needed:
		return false
	return int(held.get("tier", 0)) >= int(e.get("min_tier", 0))


## Seconds to break, or -1 when unbreakable. A material that names a tool
## class breaks in 0.5..10 s with that tool at or above its min_tier, and very
## slowly (20..120 s) by hand, with another tool or a too-low tier; materials
## without a tool class break by hand at hardness * 1.5 (leaves, plants).
func break_time(material_name: String, held: Dictionary = {}) -> float:
	var e := entry(material_name)
	if e.is_empty():
		return -1.0
	var hardness := float(e["hardness"])
	if hardness <= 0.0:
		return 0.0
	var needed: String = e.get("tool", "")
	if needed == "":
		return hardness * 1.5 / hand_speed
	if tool_class(held) == needed and int(held.get("tier", 0)) >= int(e.get("min_tier", 0)):
		var tier := clampi(int(held.get("tier", 0)), 0, tier_speed.size() - 1)
		return clampf(hardness * float(right_tool["factor"]) / float(tier_speed[tier]), float(right_tool["min_s"]), float(right_tool["max_s"]))
	return clampf(hardness * float(wrong_tool["s_per_hardness"]), float(wrong_tool["min_s"]), float(wrong_tool["max_s"]))


## False when the material drops nothing for this tool: the wrong class or tier
## for a "requires" material, or below its "drop_tier" (rock before copper, ADR 0023).
func yields(material_name: String, held: Dictionary) -> bool:
	var e := entry(material_name)
	if e.get("requires", false) and not tool_fits(material_name, held):
		return false
	if e.has("drop_tier"):
		if tool_class(held) != e.get("tool", "") or int(held.get("tier", 0)) < int(e["drop_tier"]):
			return false
	return true


## What breaking this material with `held` yields: {id, count} or {}. For a
## drop list ({id, count, chance} entries) this is the first entry's item at
## its smallest count; roll_drops gives the real, random result.
func drops(material_name: String, held: Dictionary = {}) -> Dictionary:
	var e := entry(material_name)
	if e.is_empty() or not e.has("drops"):
		return {}
	if not yields(material_name, held):
		return {}
	if e["drops"] is Array:
		var list: Array = e["drops"].filter(func(d: Dictionary) -> bool: return d.get("tool", "") in ["", tool_class(held)])
		if list.is_empty():
			return {}
		var first: Dictionary = list[0]
		return {"id": first["id"], "count": _count_range(first)[0]}
	return {"id": e["drops"], "count": int(e.get("count", 1))}


## Every stack breaking this material yields (each list entry rolled on its own).
func roll_drops(material_name: String, held: Dictionary, rng: RandomNumberGenerator) -> Array:
	var e := entry(material_name)
	if e.is_empty() or not e.has("drops"):
		return []
	if not yields(material_name, held):
		return []
	if not (e["drops"] is Array):
		return [{"id": e["drops"], "count": int(e.get("count", 1))}]
	return roll_list(e["drops"], rng, tool_class(held))


## Rolls a drop list: {id, count: n | [min, max], chance: 0..1, binomial: [n, p],
## tool: class}. binomial adds Minecraft's "n tries at p" extra count (crop seeds)
## to count; an entry with a tool class only drops when `held_class` is it.
static func roll_list(list: Array, rng: RandomNumberGenerator, held_class: String = "") -> Array:
	var out := []
	for d in list:
		if d.get("tool", "") != "" and d["tool"] != held_class:
			continue
		if rng.randf() >= float(d.get("chance", 1.0)):
			continue
		var r := _count_range(d)
		var n := rng.randi_range(r[0], r[1])
		if d.has("binomial"):
			for _i in int(d["binomial"][0]):
				if rng.randf() < float(d["binomial"][1]):
					n += 1
		if n > 0:
			out.append({"id": d["id"], "count": n})
	return out


static func _count_range(d: Dictionary) -> Array:
	var c = d.get("count", 1)
	if c is Array:
		return [int(c[0]), int(c[1])]
	return [int(c), int(c)]
