class_name EarthOvenEntity
extends RefCounted

## An earth oven (ADR 0059): a small pit dug with a shovel (a right click on soil or sand). Lay a
## "bury" recipe's ingredients in, cover them with its cover (soil for birch tar, sand for tempered
## flint), lay its fuel on top and light it with a firestarter. After the recipe's `hours` of game
## time (StoneAge.now_days, so sleeping counts) the fire is out and an empty hand digs the output
## out; the pit stays for the next batch. An empty hand takes the last thing laid in back out;
## breaking the pit gives back the ingredients and the unburnt fuel.

signal changed
signal baked(item: String)   # the oven finished: the progression book counts it

var kind := "earth_oven"
var pos: Vector3i
var recipes: RecipeRegistry
var recipe: Dictionary = {}
var loaded: Array[String] = []   # ingredients laid in, in order
var cover := 0                   # cover units on them
var fuel := 0                    # fuel items on top
var lit_at := -1.0               # game day it was lit, -1 while unlit
var output: Dictionary = {}      # the baked result, waiting to be dug out


func _init(p_recipes: RecipeRegistry, p_pos: Vector3i) -> void:
	recipes = p_recipes
	pos = p_pos


func is_lit() -> bool:
	return lit_at >= 0.0


func hours_left() -> float:
	if not is_lit():
		return 0.0
	return maxf(0.0, float(recipe.get("hours", 1.0)) - (StoneAge.now_days - lit_at) * 24.0)


func _ings() -> Array:
	return recipe.get("ingredients", [])


## The bury recipe a pit starts with when `id` goes in first, or {}.
func recipe_for(id: String) -> Dictionary:
	for r in recipes.of_type("bury"):
		if recipes.items.matches(id, String(r["ingredients"][0])):
			return r
	return {}


## Right click with `held` (an item definition, {} for an empty hand).
## Returns {consume: units, give: [stacks], lit: bool, msg: text}.
func use(held: Dictionary) -> Dictionary:
	var id: String = held.get("id", "")
	var items := recipes.items
	var res := {"consume": 0, "give": [], "lit": false, "msg": ""}
	if not output.is_empty():
		if id == "":
			res["give"] = [output.duplicate()]
			output = {}
			changed.emit()
		else:
			res["msg"] = "Dig it out with an empty hand."
		return res
	if is_lit():
		res["msg"] = "The earth oven is baking: %.1f h left." % hours_left()
		return res
	if id == "":
		if fuel > 0 and cover >= int(recipe.get("cover_count", 1)):
			fuel -= 1
			res["give"] = [{"id": items.items_with_tag(String(recipe["fuel"]).substr(1))[0] if String(recipe["fuel"]).begins_with("#") else String(recipe["fuel"]), "count": 1}]
		elif cover > 0:
			res["msg"] = "The cover is on: lay %d %s on it and light it." % [int(recipe.get("fuel_count", 1)), _spec_name(String(recipe["fuel"]))]
			return res
		elif not loaded.is_empty():
			res["give"] = [{"id": loaded.pop_back(), "count": 1}]
			if loaded.is_empty():
				recipe = {}
		else:
			res["msg"] = "An empty earth oven: lay birch bark (for tar) or flint (to temper it) in it."
			return res
		changed.emit()
		return res
	if recipe.is_empty():
		recipe = recipe_for(id)
		if recipe.is_empty():
			res["msg"] = "Bake birch bark (for tar) or flint (to temper it) in an earth oven."
			return res
	var ings := _ings()
	if loaded.size() < ings.size():
		if items.matches(id, String(ings[loaded.size()])):
			loaded.append(id)
			res["consume"] = 1
			changed.emit()
			if loaded.size() == ings.size():
				res["msg"] = "Now cover it with %d %s." % [int(recipe.get("cover_count", 1)), _spec_name(String(recipe["cover"]))]
		else:
			res["msg"] = "Lay in %d %s (%d/%d)." % [ings.size(), _spec_name(String(ings[loaded.size()])), loaded.size(), ings.size()]
		return res
	if cover < int(recipe.get("cover_count", 1)):
		if items.matches(id, String(recipe["cover"])):
			cover += 1
			res["consume"] = items.units(id)
			changed.emit()
		else:
			res["msg"] = "Cover it with %s (%d/%d)." % [_spec_name(String(recipe["cover"])), cover, int(recipe.get("cover_count", 1))]
		return res
	if fuel < int(recipe.get("fuel_count", 1)):
		if items.matches(id, String(recipe["fuel"])):
			fuel += 1
			res["consume"] = 1
			changed.emit()
		else:
			res["msg"] = "Lay %s on the cover (%d/%d)." % [_spec_name(String(recipe["fuel"])), fuel, int(recipe.get("fuel_count", 1))]
		return res
	if "firestarter" in held.get("tags", []):
		lit_at = StoneAge.now_days
		res["lit"] = true
		res["msg"] = "The earth oven is lit: %d game hours." % int(recipe.get("hours", 1))
		changed.emit()
	else:
		res["msg"] = "Light the fire on the oven with a firestarter."
	return res


func _spec_name(spec: String) -> String:
	if spec.begins_with("#"):
		return spec.substr(1)
	return recipes.items.display_name(spec)


## One tick (1/20 s): the fire burns down after the recipe's game hours.
func tick() -> bool:
	if is_lit() and hours_left() <= 0.0:
		lit_at = -1.0
		output = {"id": recipe["output"]["id"], "count": int(recipe["output"].get("count", 1))}
		loaded.clear()
		cover = 0
		fuel = 0
		recipe = {}
		baked.emit(output["id"])
		changed.emit()
	return false


func contents() -> Array:
	var out := []
	for id in loaded:
		out.append({"id": id, "count": 1})
	if not is_lit() and fuel > 0 and String(recipe.get("fuel", "")) != "":
		var f := String(recipe["fuel"])
		out.append({"id": recipes.items.items_with_tag(f.substr(1))[0] if f.begins_with("#") else f, "count": fuel})
	if not output.is_empty():
		out.append(output.duplicate())
	return out


## The pit is two voxels deep (four work cells): what lies in it, the cover, the fuel and its fire.
func view_parts() -> Dictionary:
	var size := Vector3i(8, 6, 8)
	var c := WorkPalette.new_cells(size)
	var laid := []
	var ings := _ings()
	for i in loaded.size():
		laid.append({"id": loaded[i], "at": Vector3(0.3 + 0.13 * (i % 4), 0.03 + 0.01 * i, 0.35 + 0.1 * (i % 3)), "yaw": 30.0 * i, "scale": 0.35})
	if not output.is_empty():
		WorkPalette.box(c, size, Vector3i(1, 0, 1), Vector3i(7, 1, 7), WorkPalette.ASH)
		for i in mini(int(output["count"]), 4):
			laid.append({"id": output["id"], "at": Vector3(0.3 + 0.13 * i, 0.15, 0.45), "yaw": 40.0 * i, "scale": 0.35})
	var cover_paint := WorkPalette.SAND if String(recipe.get("cover", "")) == "#sand" else WorkPalette.SOIL
	var cn := int(recipe.get("cover_count", 1))
	if cover > 0:
		var top := 1 + 3 * cover / maxi(cn, 1)
		WorkPalette.box(c, size, Vector3i(1, 1, 1), Vector3i(7, top, 7), cover_paint)
		laid = []   # buried
	var fn := int(recipe.get("fuel_count", 1))
	for i in fuel:
		var z := 1 + i * 6 / maxi(fn, 1)
		WorkPalette.box(c, size, Vector3i(1, 4, z), Vector3i(7, 5, z + 1), WorkPalette.WOOD if not is_lit() else WorkPalette.TAR)
	if is_lit():
		for z in range(1, 7):
			for x in range(1, 7):
				if (x * 5 + z * 3) % 3 == 0:
					c[WorkPalette.idx(size, x, 5, z)] = WorkPalette.EMBER + 1
	if loaded.is_empty() and cover == 0 and output.is_empty() and ings.is_empty():
		WorkPalette.box(c, size, Vector3i(1, 0, 1), Vector3i(7, 1, 7), WorkPalette.SOIL)   # the bare pit floor
	return {"solid": {"cells": c, "size": size}, "items": laid}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "recipe": recipe.get("id", ""), "loaded": loaded.duplicate(),
		"cover": cover, "fuel": fuel, "lit_at": lit_at, "output": output.duplicate()}


func load_from(d: Dictionary) -> void:
	recipe = recipes.recipes.get(String(d.get("recipe", "")), {})
	loaded.clear()
	for id in d.get("loaded", []):
		loaded.append(String(id))
	cover = int(d.get("cover", 0))
	fuel = int(d.get("fuel", 0))
	lit_at = float(d.get("lit_at", -1.0))
	var o = d.get("output", {})
	output = {"id": String(o["id"]), "count": int(o.get("count", 1))} if o is Dictionary and o.has("id") else {}


func summary() -> Dictionary:
	return {"kind": kind, "pos": pos, "recipe": recipe.get("id", ""), "loaded": loaded.size(), "cover": cover, "fuel": fuel,
		"lit": is_lit(), "hours_left": snappedf(hours_left(), 0.01), "output": output}
