class_name RecipeRegistry
extends RefCounted

## Recipes loaded from data/recipes.json. Also the query API for the
## Too-Many-Items-style browser: what makes X, what X is used for, search.
## Shaped matching follows Minecraft: the pattern may sit anywhere in the
## grid and may be mirrored horizontally; empty cells must be empty.

var items: ItemRegistry
var recipes := {}            # id -> Dictionary
var _by_output := {}         # item id -> Array[recipe id]
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
		"smelt":
			if not r.has("input"):
				errors.append("smelt recipe %s needs input" % r["id"])
				return false
		_:
			errors.append("recipe %s: unknown type %s" % [r["id"], r["type"]])
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
		"smelt":
			return [r["input"]]
	return []


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
