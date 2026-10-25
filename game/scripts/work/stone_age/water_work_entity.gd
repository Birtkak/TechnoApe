class_name WaterWorkEntity
extends RefCounted

## Something set in water (ADR 0059): flax laid in to ret ("soak" recipes: done after `hours`) or
## a fish trap ("trap" recipes: every `every_hours` it catches its output with `chance`, twice as
## often in a river when the recipe says `river`; at most MAX_CAUGHT wait in it). It lies on the
## bed, one voxel high, under the water. An empty hand takes what is ready (a catch, the retted
## flax); with nothing ready it takes the last thing laid in, so the trap comes back out.

signal changed
signal caught(item: String)   # a trap caught something or a soak finished: the book counts it

const MAX_CAUGHT := 4

var kind := "water_work"
var pos: Vector3i
var recipes: RecipeRegistry
var recipe: Dictionary = {}
var loaded: Array[String] = []
var started := -1.0           # game day the soak was complete / the trap was set
var tries := 0                # trap tries made since it was set
var ready: Array = []         # stacks waiting to be taken
var river := false            # set by StoneAge when placed: the water is a river
var rng_seed := 0


func _init(p_recipes: RecipeRegistry, p_pos: Vector3i) -> void:
	recipes = p_recipes
	pos = p_pos
	rng_seed = hash(p_pos)


func is_lit() -> bool:
	return false


## The soak or trap recipe `id` starts, or {}.
static func recipe_for(p_recipes: RecipeRegistry, id: String) -> Dictionary:
	for t in ["soak", "trap"]:
		for r in p_recipes.of_type(t):
			if p_recipes.items.matches(id, String(r["ingredients"][0])):
				return r
	return {}


func complete() -> bool:
	return not recipe.is_empty() and loaded.size() >= (recipe["ingredients"] as Array).size()


func use(held: Dictionary) -> Dictionary:
	var id: String = held.get("id", "")
	var res := {"consume": 0, "give": [], "remove": false, "msg": ""}
	if id == "":
		if not ready.is_empty():
			res["give"] = ready.duplicate(true)
			ready.clear()
			if recipe.get("type", "") == "soak":
				res["remove"] = true
			changed.emit()
			return res
		if not loaded.is_empty():
			res["give"] = [{"id": loaded.pop_back(), "count": 1}]
			started = -1.0
			res["remove"] = loaded.is_empty()
			changed.emit()
			return res
		res["remove"] = true
		return res
	if recipe.is_empty():
		recipe = recipe_for(recipes, id)
		if recipe.is_empty():
			res["msg"] = "Lay flax in the water to ret it, or set a fish trap."
			return res
	if complete():
		res["msg"] = _status()
		return res
	var ings: Array = recipe["ingredients"]
	if recipes.items.matches(id, String(ings[loaded.size()])):
		loaded.append(id)
		res["consume"] = 1
		if complete():
			started = StoneAge.now_days
			tries = 0
		changed.emit()
		res["msg"] = _status() if complete() else "Lay in %d (%d/%d)." % [ings.size(), loaded.size(), ings.size()]
	else:
		res["msg"] = _status()
	return res


func _status() -> String:
	if not complete():
		return "Lay in more (%d/%d)." % [loaded.size(), (recipe["ingredients"] as Array).size()]
	if recipe["type"] == "soak":
		return "Retting: %.0f game hours left." % maxf(0.0, float(recipe["hours"]) - (StoneAge.now_days - started) * 24.0)
	return "The trap is set%s: %d caught." % [" in a river" if river and recipe.get("river", false) else "", ready.size()]


func tick() -> bool:
	if not complete() or started < 0.0:
		return false
	var now := StoneAge.now_days
	if now < started:
		started = now   # the date went back (/time)
	var hours := (now - started) * 24.0
	if recipe["type"] == "soak":
		if ready.is_empty() and hours >= float(recipe["hours"]):
			ready = [{"id": recipe["output"]["id"], "count": int(recipe["output"].get("count", 1))}]
			loaded.clear()
			started = -1.0
			caught.emit(ready[0]["id"])
			changed.emit()
		return false
	var every := float(recipe.get("every_hours", 4.0)) / (2.0 if river and recipe.get("river", false) else 1.0)
	var due := int(hours / every)
	var any := false
	while tries < due:
		tries += 1
		var r := RandomNumberGenerator.new()
		r.seed = rng_seed + tries * 7919
		if r.randf() < float(recipe.get("chance", 1.0)) and ready.size() < MAX_CAUGHT:
			ready.append({"id": recipe["output"]["id"], "count": int(recipe["output"].get("count", 1))})
			caught.emit(recipe["output"]["id"])
			any = true
	if any:
		changed.emit()
	return false


func contents() -> Array:
	var out := []
	for id in loaded:
		out.append({"id": id, "count": 1})
	out.append_array(ready.duplicate(true))
	return out


## A wicker basket (trap), a wattle fence (weir) or a bundle of stalks (soaking) on the bed.
func view_parts() -> Dictionary:
	var size := Vector3i(8, 8, 8)
	var c := WorkPalette.new_cells(size)
	var laid := []
	match String(recipe.get("id", "")):
		"trap_weir":
			for x in 8:
				WorkPalette.box(c, size, Vector3i(x, 0, 3), Vector3i(x + 1, 6 + x % 2, 4 + x % 2), WorkPalette.WOOD if x % 2 == 0 else WorkPalette.WOOD_DARK)
		"trap_basket":
			for z in range(1, 7):
				for x in range(2, 6):
					for y in range(0, 3):
						if (x == 2 or x == 5 or z == 1 or z == 6 or y == 0) and (x + y + z) % 2 == 0:
							c[WorkPalette.idx(size, x, y, z)] = WorkPalette.STRAW + 1
		_:
			for i in loaded.size():
				laid.append({"id": loaded[i], "at": Vector3(0.35 + 0.1 * i, 0.03, 0.5), "yaw": 80.0 + 10.0 * i, "scale": 0.45})
	for i in ready.size():
		laid.append({"id": ready[i]["id"], "at": Vector3(0.3 + 0.13 * i, 0.06, 0.45), "yaw": 30.0 * i, "scale": 0.35})
	return {"solid": {"cells": c, "size": size}, "items": laid}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "recipe": recipe.get("id", ""), "loaded": loaded.duplicate(),
		"started": started, "tries": tries, "ready": ready.duplicate(true), "river": river}


func load_from(d: Dictionary) -> void:
	recipe = recipes.recipes.get(String(d.get("recipe", "")), {})
	loaded.clear()
	for id in d.get("loaded", []):
		loaded.append(String(id))
	started = float(d.get("started", -1.0))
	tries = int(d.get("tries", 0))
	ready.clear()
	for s in d.get("ready", []):
		if s is Dictionary and s.has("id"):
			ready.append({"id": String(s["id"]), "count": int(s.get("count", 1))})
	river = bool(d.get("river", false))


func summary() -> Dictionary:
	return {"kind": kind, "pos": pos, "recipe": recipe.get("id", ""), "loaded": loaded.size(), "ready": ready.size(), "river": river}
