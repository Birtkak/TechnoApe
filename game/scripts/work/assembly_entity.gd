class_name AssemblyEntity
extends RefCounted

## A tool being put together on the ground (ADR 0042, ADR 0037 decision 3): the head lies there,
## then a stick goes onto it, then the binding. "assemble" recipes list their parts in order
## (`parts`, ids or #tags); the first part starts a piece on a top face, each right click with the
## next part adds it, and the last one finishes the tool. An empty hand takes the last part back
## off; breaking the piece gives every part back. The parts are drawn as their own item sprites
## laid flat, so the tool grows on the ground as it is built.

signal changed

var kind := "assembly"
var pos: Vector3i
var parts: Array[String] = []   # item ids laid so far, in order
var recipes: RecipeRegistry


func _init(p_recipes: RecipeRegistry, p_pos: Vector3i) -> void:
	recipes = p_recipes
	pos = p_pos


## The assemble recipes whose parts begin with what lies here (by spec), sorted by id.
func candidates() -> Array:
	var out := []
	for r in recipes.of_type("assemble"):
		var want: Array = r["parts"]
		if want.size() < parts.size():
			continue
		var ok := true
		for i in parts.size():
			if not recipes.items.matches(parts[i], want[i]):
				ok = false
				break
		if ok:
			out.append(r)
	return out


## True when `id` can start a piece (it is the first part of an assemble recipe).
static func starts(p_recipes: RecipeRegistry, id: String) -> bool:
	for r in p_recipes.of_type("assemble"):
		if p_recipes.items.matches(id, r["parts"][0]):
			return true
	return false


## The specs that may go on next (one per candidate recipe, without repeats).
func next_specs() -> Array[String]:
	var out: Array[String] = []
	for r in candidates():
		var want: Array = r["parts"]
		if want.size() > parts.size() and not String(want[parts.size()]) in out:
			out.append(String(want[parts.size()]))
	return out


## The recipe the laid parts complete, or {}.
func finished_recipe() -> Dictionary:
	for r in candidates():
		if (r["parts"] as Array).size() == parts.size():
			return r
	return {}


## Right click with `held` (an item definition, {} for an empty hand).
## Returns {consume: n, give: [stacks], remove: bool, done: output stack or {}, msg: text}.
func use(held: Dictionary) -> Dictionary:
	var id: String = held.get("id", "")
	var res := {"consume": 0, "give": [], "remove": false, "done": {}, "msg": ""}
	if id == "":
		if not parts.is_empty():
			res["give"] = [{"id": parts.pop_back(), "count": 1}]
			res["remove"] = parts.is_empty()
			changed.emit()
		return res
	for spec in next_specs():
		if recipes.items.matches(id, spec):
			parts.append(id)
			res["consume"] = 1
			var r := finished_recipe()
			if not r.is_empty() and next_specs().is_empty():
				res["done"] = {"id": r["output"]["id"], "count": int(r["output"].get("count", 1))}
				res["remove"] = true
			changed.emit()
			return res
	res["msg"] = "Next: " + " or ".join(next_specs().map(func(s: String) -> String: return _spec_name(s)))
	return res


func _spec_name(spec: String) -> String:
	if spec.begins_with("#"):
		return "any " + spec.substr(1)
	return recipes.items.display_name(spec)


func contents() -> Array:
	var out := []
	for id in parts:
		out.append({"id": id, "count": 1})
	return out


func tick() -> bool:
	return false


func is_lit() -> bool:
	return false


## Where part i lies, in block fractions over the footprint, and how. Tool sprites are layered on one
## 16 px tile (handle, then head at the handle's tip), so the head and the handles share a centre and a
## size and the laid parts draw the tool itself; a second handle lies a little behind the first, the
## binding over the joint (the handle tip, 3.5 px up and right of the tile's centre).
const SCALE := 0.8

func _layout(i: int, id: String) -> Dictionary:
	var items := recipes.items
	if i > 0 and items.has_tag(id, "binding"):
		var tip := 3.5 / 16.0 * SCALE
		return {"id": id, "at": Vector3(0.5 + tip, 0.04, 0.5 - tip), "yaw": 45.0, "scale": 0.3}
	var behind := 0
	for j in range(1, i):
		if not items.has_tag(parts[j], "binding"):
			behind += 1
	var o := 0.1 * behind
	return {"id": id, "at": Vector3(0.5 - o, 0.03 if i == 0 else 0.02 - 0.002 * behind, 0.5 + o), "yaw": 0.0, "scale": SCALE}


func view_parts() -> Dictionary:
	var laid := []
	for i in parts.size():
		laid.append(_layout(i, parts[i]))
	return {"items": laid}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "parts": parts.duplicate()}


func load_from(d: Dictionary) -> void:
	parts.clear()
	for id in d.get("parts", []):
		if recipes.items.has(String(id)):
			parts.append(String(id))


func summary() -> Dictionary:
	return {"kind": kind, "pos": pos, "parts": parts.duplicate(), "next": next_specs()}
