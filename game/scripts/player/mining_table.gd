class_name MiningTable
extends RefCounted

## Mining through the material model (ADR 0005), loaded from data/mining.json.
## Pure logic: given a material name and the held item definition it answers
## "how long does breaking take" and "what drops". Engine-agnostic, headless
## testable. Materials missing from the table are unbreakable (safe default).

var materials := {}          # material name -> entry
var hand_speed := 1.0
var tier_speed: Array = [2.0, 4.0, 6.0, 8.0]
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
		if t in ["pickaxe", "shovel", "axe"]:
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


## Seconds to break, or -1 when unbreakable. Minecraft: hardness * 1.5 with
## the right tool (or none needed), * 5 with the wrong one.
func break_time(material_name: String, held: Dictionary = {}) -> float:
	var e := entry(material_name)
	if e.is_empty():
		return -1.0
	var hardness := float(e["hardness"])
	if hardness <= 0.0:
		return 0.0
	var needed: String = e.get("tool", "")
	if needed != "" and tool_class(held) == needed:
		var tier := clampi(int(held.get("tier", 0)), 0, tier_speed.size() - 1)
		var speed := float(tier_speed[tier])
		if int(held.get("tier", 0)) < int(e.get("min_tier", 0)):
			return hardness * 5.0 / speed
		return hardness * 1.5 / speed
	if e.get("requires", false):
		return hardness * 5.0 / hand_speed
	return hardness * 1.5 / hand_speed


## What breaking this material with `held` yields: {id, count} or {}.
func drops(material_name: String, held: Dictionary = {}) -> Dictionary:
	var e := entry(material_name)
	if e.is_empty() or not e.has("drops"):
		return {}
	if e.get("requires", false) and not tool_fits(material_name, held):
		return {}
	return {"id": e["drops"], "count": int(e.get("count", 1))}
