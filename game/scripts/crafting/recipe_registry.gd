class_name RecipeRegistry
extends RefCounted

## Recipes loaded from data/recipes.json. Also the query API for the
## Too-Many-Items-style browser: what makes X, what X is used for, search.
## Shaped matching follows Minecraft: the pattern may sit anywhere in the
## grid and may be mirrored horizontally; empty cells must be empty.

var items: ItemRegistry
var recipes := {}            # id -> Dictionary
var _by_output := {}         # item id -> Array[recipe id]
var _by_type := {}           # type -> recipes sorted by id (of_type cache, cleared on load)
var errors: Array[String] = []


func _init(p_items: ItemRegistry) -> void:
	items = p_items


static func load_default(p_items: ItemRegistry) -> RecipeRegistry:
	var r := RecipeRegistry.new(p_items)
	r.load_file("res://data/recipes.json")
	return r


func load_file(path: String) -> bool:
	var f := FileAccess.open(path, FileAccess.READ)
	if f == null:
		errors.append("cannot open %s" % path)
		return false
	return load_json(f.get_as_text())


func load_json(text: String) -> bool:
	var data = JSON.parse_string(text)
	if typeof(data) != TYPE_DICTIONARY or not data.has("recipes"):
		errors.append("recipes.json: expected {recipes: [...]}")
		return false
	_by_type.clear()
	for r in data["recipes"]:
		if not _validate(r):
			continue
		recipes[r["id"]] = r
		var out_id: String = r["output"]["id"]
		if not _by_output.has(out_id):
			_by_output[out_id] = []
		_by_output[out_id].append(r["id"])
	return errors.is_empty()


func _validate(r) -> bool:
	if typeof(r) != TYPE_DICTIONARY or not r.has("id") or not r.has("type") or not r.has("output"):
		errors.append("recipe missing id/type/output: %s" % str(r))
		return false
	if recipes.has(r["id"]):
		errors.append("duplicate recipe id %s" % r["id"])
		return false
	if not items.has(r["output"]["id"]):
		errors.append("recipe %s outputs unknown item %s" % [r["id"], r["output"]["id"]])
		return false
	for spec in ingredient_specs(r):
		if not _spec_exists(spec):
			errors.append("recipe %s: ingredient %s matches no item" % [r["id"], spec])
			return false
	match r["type"]:
		"shaped":
			if not r.has("pattern") or not r.has("key"):
				errors.append("shaped recipe %s needs pattern+key" % r["id"])
				return false
			if r["pattern"].size() > 3 or r["pattern"].size() < 1:
				errors.append("recipe %s: pattern must be 1..3 rows" % r["id"])
				return false
			for row in r["pattern"]:
				if String(row).length() > 3 or String(row).length() < 1:
					errors.append("recipe %s: rows must be 1..3 wide" % r["id"])
					return false
				for ch in String(row):
					if ch != " " and not r["key"].has(ch):
						errors.append("recipe %s: pattern char '%s' not in key" % [r["id"], ch])
						return false
		"shapeless", "spellcraft":
			if not r.has("ingredients") or r["ingredients"].is_empty() or r["ingredients"].size() > 9:
				errors.append("recipe %s: needs 1..9 ingredients" % r["id"])
				return false
			for id in r.get("remainder", {}):
				if not items.has(r["remainder"][id]) or not id in r["ingredients"]:
					errors.append("recipe %s: bad remainder %s" % [r["id"], id])
					return false
			for spec in r.get("keep", []):
				if not spec in r["ingredients"]:
					errors.append("recipe %s: keeps %s, which is no ingredient" % [r["id"], spec])
					return false
		"smelt", "kiln", "sift", "dry", "charcoal_pit":
			if not r.has("input"):
				errors.append("%s recipe %s needs input" % [r["type"], r["id"]])
				return false
		"knap":
			if not r.has("input") or not _is_grid(r.get("pattern")):
				errors.append("knap recipe %s needs input and an 8x8 pattern" % r["id"])
				return false
		"clayform":
			var layers = r.get("layers")
			if not r.has("input") or not (layers is Array) or layers.is_empty():
				errors.append("clayform recipe %s needs input and layers" % r["id"])
				return false
			for l in layers:
				if not _is_grid(l):
					errors.append("clayform recipe %s: every layer is 8 rows of 8" % r["id"])
					return false
		"melt":
			if not r.has("input") or not r.has("vessel"):
				errors.append("melt recipe %s needs input and vessel" % r["id"])
				return false
		"scrape":
			if not r.has("input") or not r.has("tool") or int(r.get("strokes", 0)) <= 0:
				errors.append("scrape recipe %s needs input, tool and strokes" % r["id"])
				return false
		"assemble":
			var parts = r.get("parts")
			if not (parts is Array) or parts.size() < 2 or parts.size() > 6:
				errors.append("assemble recipe %s needs 2..6 parts" % r["id"])
				return false
		"smith":
			var layers = r.get("layers")
			if not r.has("input") or not (layers is Array) or layers.is_empty() or layers.size() > AnvilEntity.MAX_H:
				errors.append("smith recipe %s needs input and 1..%d layers" % [r["id"], AnvilEntity.MAX_H])
				return false
			for i in layers.size():
				if not _is_grid(layers[i]):
					errors.append("smith recipe %s: every layer is 8 rows of 8" % r["id"])
					return false
				for z in 8:
					for x in 8:
						if i > 0 and String(layers[i][z])[x] == "#" and String(layers[i - 1][z])[x] != "#":
							errors.append("smith recipe %s: metal over nothing at layer %d (%d, %d)" % [r["id"], i, x, z])
							return false
		"cast":
			if not r.has("input") or not r.has("mold"):
				errors.append("cast recipe %s needs input and mold" % r["id"])
				return false
			if r.has("returns") and not items.has(r["returns"]):
				errors.append("cast recipe %s returns unknown item %s" % [r["id"], r["returns"]])
				return false
		_:
			errors.append("recipe %s: unknown type %s" % [r["id"], r["type"]])
			return false
	return true


## 8 strings of 8 characters (knapping patterns, clay layers; ADR 0023).
static func _is_grid(g) -> bool:
	if not (g is Array) or g.size() != 8:
		return false
	for row in g:
		if String(row).length() != 8:
			return false
	return true


func _spec_exists(spec: String) -> bool:
	if spec.begins_with("#"):
		return not items.items_with_tag(spec.substr(1)).is_empty()
	return items.has(spec)


## All ingredient specs of a recipe (ids or #tags), one per required unit.
func ingredient_specs(r: Dictionary) -> Array:
	match r["type"]:
		"shaped":
			var out := []
			for row in r["pattern"]:
				for ch in String(row):
					if ch != " ":
						out.append(r["key"][ch])
			return out
		"shapeless", "spellcraft":
			return r["ingredients"].duplicate()
		"smelt", "knap", "clayform", "kiln", "sift", "dry":
			return [r["input"]]
		"scrape":
			return [r["input"], r["tool"]]
		"charcoal_pit":
			var out := []
			for i in int(r.get("per", 1)):
				out.append(r["input"])
			return out
		"melt":
			var out := []
			for i in int(r.get("count", 1)):
				out.append(r["input"])
			out.append(r["vessel"])
			return out
		"cast":
			return [r["input"], r["mold"]]
		"assemble":
			return r["parts"].duplicate()
		"smith":
			var out := []
			for i in AnvilEntity.ingots_for(r):
				out.append(r["input"])
			return out
	return []


## Recipes of one type, sorted by id.
func of_type(t: String) -> Array:
	if _by_type.has(t):
		return _by_type[t]
	var out := []
	for rid in recipes:
		if recipes[rid]["type"] == t:
			out.append(recipes[rid])
	out.sort_custom(func(a: Dictionary, b: Dictionary) -> bool: return a["id"] < b["id"])
	_by_type[t] = out
	return out


## The first recipe of type `t` (by id) whose fields match `filter`: "input",
## "vessel" and "mold" match an item id against the recipe's spec, "output"
## the output id. {} when none.
func first_of(t: String, filter: Dictionary) -> Dictionary:
	for r in of_type(t):
		var ok := true
		for k in filter:
			if k == "output":
				ok = r["output"]["id"] == filter[k]
			else:
				ok = r.has(k) and items.matches(String(filter[k]), String(r[k]))
			if not ok:
				break
		if ok:
			return r
	return {}


## Every sift recipe for an input at a station (the sieve, or the pan of
## ADR 0030): one loot table, each entry rolled on its own.
func sift_table(item_id: String, station: String = "sieve") -> Array:
	var out := []
	for r in of_type("sift"):
		if r.get("station", "sieve") == station and items.matches(item_id, r["input"]):
			out.append(r)
	return out


# ---------------- browser queries ----------------

func makes(item_id: String) -> Array:
	return _by_output.get(item_id, []).duplicate()


func uses_of(item_id: String) -> Array:
	var out := []
	for rid in recipes:
		for spec in ingredient_specs(recipes[rid]):
			if items.matches(item_id, spec):
				out.append(rid)
				break
	out.sort()
	return out


func search(text: String) -> Array:
	var q := text.to_lower()
	var out := []
	for rid in recipes:
		var r: Dictionary = recipes[rid]
		if q in rid or q in items.display_name(r["output"]["id"]).to_lower():
			out.append(rid)
	out.sort()
	return out


func by_station(station: String) -> Array:
	var out := []
	for rid in recipes:
		if recipes[rid].get("station", "") == station:
			out.append(rid)
	out.sort()
	return out


## Items reachable from a set of raw items (ignores stations). Used by the
## "is everything craftable?" test.
func reachable_from(raw_ids: Array) -> Dictionary:
	var have := {}
	for id in raw_ids:
		have[id] = true
	var changed := true
	while changed:
		changed = false
		for rid in recipes:
			var r: Dictionary = recipes[rid]
			if have.has(r["output"]["id"]):
				continue
			var ok := true
			for spec in ingredient_specs(r):
				var found := false
				for id in have:
					if items.matches(id, spec):
						found = true
						break
				if not found:
					ok = false
					break
			if ok:
				have[r["output"]["id"]] = true
				changed = true
	return have


## The smelt recipe that takes `item_id` at `station` (furnace), or {}.
func smelting_for(item_id: String, station: String = "furnace") -> Dictionary:
	if item_id == "":
		return {}
	for rid in recipes:
		var r: Dictionary = recipes[rid]
		if r["type"] == "smelt" and r.get("station", "furnace") == station and items.matches(item_id, r["input"]):
			return r
	return {}


# ---------------- matching ----------------

## grid: Array of 9 item ids ("" = empty), row-major 3x3. Returns the recipe
## Dictionary or {} when nothing matches. `station` filters recipes that need one.
func match_grid(grid: Array, station: String = "") -> Dictionary:
	for rid in recipes:
		var r: Dictionary = recipes[rid]
		var needs: String = r.get("station", "")
		if needs != "" and needs != station:
			continue
		match r["type"]:
			"shaped":
				if _match_shaped(r, grid):
					return r
			"shapeless":
				if _match_shapeless(r["ingredients"], grid):
					return r
	return {}


func _match_shapeless(specs: Array, grid: Array) -> bool:
	var present := []
	for id in grid:
		if id != "":
			present.append(id)
	if present.size() != specs.size():
		return false
	var remaining := specs.duplicate()
	for id in present:
		var hit := -1
		for i in remaining.size():
			if items.matches(id, remaining[i]):
				hit = i
				break
		if hit < 0:
			return false
		remaining.remove_at(hit)
	return remaining.is_empty()


func _match_shaped(r: Dictionary, grid: Array) -> bool:
	var pattern: Array = r["pattern"]
	var h := pattern.size()
	var w := 0
	for row in pattern:
		w = maxi(w, String(row).length())
	for mirror in [false, true]:
		for oy in range(0, 4 - h):
			for ox in range(0, 4 - w):
				if _shaped_at(r, grid, ox, oy, w, h, mirror):
					return true
	return false


func _shaped_at(r: Dictionary, grid: Array, ox: int, oy: int, w: int, h: int, mirror: bool) -> bool:
	for gy in 3:
		for gx in 3:
			var id: String = grid[gy * 3 + gx]
			var px := gx - ox
			var py := gy - oy
			var spec := ""
			if px >= 0 and px < w and py >= 0 and py < h:
				var row: String = r["pattern"][py]
				var col := (w - 1 - px) if mirror else px
				var ch := row[col] if col < row.length() else " "
				spec = r["key"][ch] if ch != " " else ""
			if spec == "":
				if id != "":
					return false
			else:
				if id == "" or not items.matches(id, spec):
					return false
	return true
