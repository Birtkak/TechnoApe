class_name StrokeEntity
extends RefCounted

## Work done in strokes on a surface (ADR 0059, "stroke" recipes): grinding an axe roughout on
## sandstone with sand and water, grain into flour and ochre into pigment on a flat rock (a saddle
## quern), sewing pelts and cloth on the ground, weaving on a loom. The first ingredient starts the
## piece where it is laid (the recipe picked by StoneAge, with a picker when several start with it),
## right clicks add the rest, then `strokes` right clicks with the recipe's tool finish it (held
## clicks repeat). `grit` must be added every `grit_every` strokes; `water` needs a water voxel within
## a block (StoneAge checks it when the piece is laid and when grit goes on). An empty hand takes the
## last ingredient back; the last one taken removes the piece (a loom stays).

signal changed

var kind := "stroke"          # "loom" for a placed loom
var pos: Vector3i
var recipes: RecipeRegistry
var station := "ground"       # what the piece lies on: ground, stone, sandstone, loom
var recipe: Dictionary = {}
var loaded: Array[String] = []
var strokes := 0
var grit := 0                 # strokes left before more grit is needed
var water_ok := true


func _init(p_recipes: RecipeRegistry, p_pos: Vector3i, p_kind: String = "stroke") -> void:
	recipes = p_recipes
	pos = p_pos
	kind = p_kind
	if kind == "loom":
		station = "loom"


func is_lit() -> bool:
	return false


func tick() -> bool:
	return false


## The stroke recipes for `station` whose first ingredient `id` is, sorted by id.
static func options(p_recipes: RecipeRegistry, p_station: String, id: String) -> Array:
	var out := []
	for r in p_recipes.of_type("stroke"):
		if String(r.get("on", "")) == p_station and p_recipes.items.matches(id, String(r["ingredients"][0])):
			out.append(r)
	return out


func set_recipe(r: Dictionary) -> void:
	recipe = r
	strokes = 0
	grit = 0
	changed.emit()


func has_recipe() -> bool:
	return not recipe.is_empty()


func ready() -> bool:
	return has_recipe() and loaded.size() >= (recipe["ingredients"] as Array).size()


func needs_grit() -> bool:
	return String(recipe.get("grit", "")) != "" and grit <= 0


func _spec_name(spec: String) -> String:
	return spec.substr(1) if spec.begins_with("#") else recipes.items.display_name(spec)


## Right click with `held`. Returns {consume: units, give: [stacks], stroke: bool, done: stack or {},
## remove: bool, pick: bool (open the picker), msg}.
func use(held: Dictionary) -> Dictionary:
	var id: String = held.get("id", "")
	var items := recipes.items
	var res := {"consume": 0, "give": [], "stroke": false, "done": {}, "remove": false, "pick": false, "msg": ""}
	if id == "":
		if loaded.is_empty():
			res["msg"] = "Hang linen thread on the loom." if kind == "loom" else ""
			res["remove"] = kind != "loom"
			return res
		res["give"] = [{"id": loaded.pop_back(), "count": 1}]
		strokes = 0
		if loaded.is_empty() and kind != "loom":
			res["remove"] = true
		elif loaded.is_empty():
			recipe = {}
		changed.emit()
		return res
	if not has_recipe():
		var opts := options(recipes, station, id)
		if opts.is_empty():
			res["msg"] = "Nothing to work with that here."
		elif opts.size() == 1:
			loaded.append(id)
			res["consume"] = 1
			set_recipe(opts[0])
		else:
			loaded.append(id)
			res["consume"] = 1
			res["pick"] = true
			changed.emit()
		return res
	var ings: Array = recipe["ingredients"]
	if loaded.size() < ings.size():
		if items.matches(id, String(ings[loaded.size()])):
			loaded.append(id)
			res["consume"] = 1
			changed.emit()
			if ready():
				res["msg"] = "Now %d strokes with %s." % [int(recipe["strokes"]), _tool_name()]
		else:
			res["msg"] = "Add %s (%d/%d)." % [_spec_name(String(ings[loaded.size()])), loaded.size(), ings.size()]
		return res
	var g := String(recipe.get("grit", ""))
	if g != "" and items.matches(id, g):
		grit = int(recipe.get("grit_every", 16))
		res["consume"] = maxi(1, items.units(id) / 8)   # a handful: an eighth of a block of sand
		res["msg"] = "Grit on the stone."
		changed.emit()
		return res
	var tool := String(recipe.get("tool", ""))
	if tool != "" and not items.matches(id, tool):
		res["msg"] = "Work it with %s (%d/%d strokes)." % [_tool_name(), strokes, int(recipe["strokes"])]
		return res
	if recipe.get("water", false) and not water_ok:
		res["msg"] = "This needs water within a block of the stone."
		return res
	if needs_grit():
		res["msg"] = "Add %s to the stone first." % _spec_name(g)
		return res
	strokes += 1
	grit -= 1
	res["stroke"] = true
	if strokes >= int(recipe["strokes"]):
		res["done"] = {"id": recipe["output"]["id"], "count": int(recipe["output"].get("count", 1))}
		loaded.clear()
		strokes = 0
		grit = 0
		if kind == "loom":
			recipe = {}
		else:
			res["remove"] = true
	changed.emit()
	return res


func _tool_name() -> String:
	var t := String(recipe.get("tool", ""))
	return "an empty hand" if t == "" else ("a " + _spec_name(t) if t.begins_with("#") else _spec_name(t))


func contents() -> Array:
	var out := []
	for id in loaded:
		out.append({"id": id, "count": 1})
	return out


## The work lies flat on the face as its sprites; grit is a sand film, the loom a frame of sticks
## with the thread hanging down and the cloth growing from the top as the strokes go on.
func view_parts() -> Dictionary:
	var size := Vector3i(8, 8, 8)
	var c := WorkPalette.new_cells(size)
	var laid := []
	if kind == "loom":
		WorkPalette.box(c, size, Vector3i(0, 0, 3), Vector3i(1, 8, 4), WorkPalette.WOOD)
		WorkPalette.box(c, size, Vector3i(7, 0, 3), Vector3i(8, 8, 4), WorkPalette.WOOD)
		WorkPalette.box(c, size, Vector3i(0, 7, 3), Vector3i(8, 8, 4), WorkPalette.WOOD_DARK)
		if not loaded.is_empty():
			var n := (recipe.get("strokes", 16) as int) if has_recipe() else 16
			var woven := 1 + 6 * strokes / maxi(n, 1)
			for x in range(1, 7):
				WorkPalette.box(c, size, Vector3i(x, 1, 3), Vector3i(x + 1, 7, 4), WorkPalette.LINEN if x % 2 == 0 else WorkPalette.STRAW)
			WorkPalette.box(c, size, Vector3i(1, 7 - woven, 3), Vector3i(7, 7, 4), WorkPalette.LINEN)
			WorkPalette.box(c, size, Vector3i(1, 0, 3), Vector3i(7, 1, 4), WorkPalette.STONE)   # the loom weights
		return {"solid": {"cells": c, "size": size}}
	for i in loaded.size():
		laid.append({"id": loaded[i], "at": Vector3(0.5 + 0.08 * (i % 3) - 0.08, 0.02 + 0.004 * i, 0.5 + 0.06 * (i / 3)), "yaw": 25.0 * i, "scale": 0.5 if station == "ground" else 0.4})
	if grit > 0:
		WorkPalette.box(c, size, Vector3i(1, 0, 1), Vector3i(7, 1, 7), WorkPalette.SAND)
		for l in laid:
			l["at"].y += 0.06
	return {"solid": {"cells": c, "size": size}, "items": laid}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "station": station, "recipe": recipe.get("id", ""),
		"loaded": loaded.duplicate(), "strokes": strokes, "grit": grit, "water_ok": water_ok}


func load_from(d: Dictionary) -> void:
	station = String(d.get("station", station))
	recipe = recipes.recipes.get(String(d.get("recipe", "")), {})
	loaded.clear()
	for id in d.get("loaded", []):
		loaded.append(String(id))
	strokes = int(d.get("strokes", 0))
	grit = int(d.get("grit", 0))
	water_ok = bool(d.get("water_ok", true))


func summary() -> Dictionary:
	return {"kind": kind, "pos": pos, "station": station, "recipe": recipe.get("id", ""), "loaded": loaded.duplicate(),
		"strokes": strokes, "grit": grit, "water_ok": water_ok}
