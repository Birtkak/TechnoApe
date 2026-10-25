class_name RecipeBrowser
extends RefCounted

## JEI-style recipe browser model (ADR 0008), pure logic over the same
## registries the crafting system uses, so it is never out of date:
## - item list in items.json order, filtered by a search query (all terms
##   must match; a term matches id or name; "#tag" matches a tag), paged;
## - "recipes" (how an item is made) and "uses" (what it goes into) views,
##   grouped by category (Crafting, Smelting, Spellcraft, other stations);
##   the uses of a station item (furnace, crafting table) include its whole
##   category like JEI catalysts;
## - a history stack for Backspace; layouts that place each recipe's inputs
##   on a grid; tag ingredients cycle through their members;
## - "mining" pseudo-recipes from data/mining.json (block -> drops, which tool),
##   added by set_mining(), so "how do I get flint" has an answer too;
## - bookmarks (JEI's A key), kept in user://bookmarks.json by the screen.

const CATEGORY_ORDER := ["crafting", "knapping", "clayforming", "pit_kiln", "station:firepit", "casting", "scraping", "drying", "charcoal_pit", "station:sieve", "station:pan", "smelting", "spellcraft", "mining"]
const BOOKMARKS_PATH := "user://bookmarks.json"
const CATALYSTS := {"crafting": ["crafting_table"], "smelting": ["furnace"], "knapping": ["flint"],
	"clayforming": ["clay_ball"], "pit_kiln": ["dry_grass", "firestarter"], "scraping": ["flint_knife", "copper_knife"],
	"charcoal_pit": ["firewood", "firestarter"], "smithing": ["stone_anvil", "copper_hammer"]}
## Vintage Story work types (ADR 0023) -> browser category and title.
const WORK_CATEGORIES := {"knap": ["knapping", "Knapping"], "clayform": ["clayforming", "Clay forming"],
	"kiln": ["pit_kiln", "Pit kiln"], "cast": ["casting", "Casting"], "scrape": ["scraping", "Scraping hides"],
	"dry": ["drying", "Drying"], "charcoal_pit": ["charcoal_pit", "Charcoal pit"],
	"assemble": ["assembly", "Assembly on the ground"], "smith": ["smithing", "Smithing"],
	"bury": ["earth_oven", "Earth oven"], "rack": ["rack", "Drying rack"], "stroke": ["stroke", "Grinding, sewing, weaving"],
	"soak": ["soak", "Laid in water"], "trap": ["trap", "Traps"]}

var items: ItemRegistry
var recipes: RecipeRegistry
var query := ""
var page := 0
var page_size := 45
var _filtered: Array = []
var _order := {}          # item id -> index in items.json, for stable member order

# open view: {} when the item list is showing, else
# {"mode": "recipes"|"uses", "item": id, "categories": [{id, title, recipes}], "cat": i, "index": i}
var view := {}
var history: Array = []
var bookmarks: Array = []   # item ids, oldest first
var _mining := {}           # pseudo-recipe id ("mining:<block>:<drop>") -> recipe
var _mining_makes := {}     # drop item id -> [pseudo-recipe id]
var _mining_uses := {}      # block item id -> [pseudo-recipe id]
var have: Callable          # item id -> how many the player carries (the inventory screen sets it): have / need lines


func _init(p_items: ItemRegistry, p_recipes: RecipeRegistry) -> void:
	items = p_items
	recipes = p_recipes
	var i := 0
	for id in items.all_ids():
		_order[id] = i
		i += 1
	set_query("")


# ---------------- item list ----------------

func set_query(q: String) -> void:
	query = q
	page = 0
	_filtered = []
	var terms := q.to_lower().split(" ", false)
	for id in items.all_ids():
		if _matches_all(id, terms):
			_filtered.append(id)


func _matches_all(id: String, terms: PackedStringArray) -> bool:
	var d := items.get_def(id)
	var name := String(d.get("name", id)).to_lower()
	for t in terms:
		if t.begins_with("#"):
			var tag := t.substr(1)
			var hit := false
			for tg in d.get("tags", []):
				if String(tg).begins_with(tag):
					hit = true
					break
			if not hit:
				return false
		elif not (t in id or t in name):
			return false
	return true


func filtered() -> Array:
	return _filtered.duplicate()


func page_count() -> int:
	return maxi(1, ceili(float(_filtered.size()) / float(maxi(page_size, 1))))


func page_items() -> Array:
	var from := page * page_size
	return _filtered.slice(from, from + page_size)


## Mouse wheel / arrows: wraps around like JEI.
func turn_page(direction: int) -> void:
	page = posmod(page + direction, page_count())


# ---------------- recipe views ----------------

## A recipe by id: recipes.json first, then the mining pseudo-recipes.
func recipe(rid: String) -> Dictionary:
	return recipes.recipes[rid] if recipes.recipes.has(rid) else _mining.get(rid, {})


func _all_recipe_ids() -> Array:
	return recipes.recipes.keys() + _mining.keys()


func category_of(r: Dictionary) -> String:
	match r["type"]:
		"smelt":
			return "smelting"
		"spellcraft":
			return "spellcraft"
		"mining":
			return "mining"
	if WORK_CATEGORIES.has(r["type"]):
		return WORK_CATEGORIES[r["type"]][0]
	var st: String = r.get("station", "")
	return "crafting" if st == "" or st == "crafting_table" else "station:" + st


func category_title(cat: String) -> String:
	match cat:
		"crafting":
			return "Crafting"
		"smelting":
			return "Smelting"
		"spellcraft":
			return "Spellcraft"
		"mining":
			return "Mining"
	for t in WORK_CATEGORIES:
		if WORK_CATEGORIES[t][0] == cat:
			return WORK_CATEGORIES[t][1]
	var st := cat.trim_prefix("station:")
	return items.display_name(st) if items.has(st) else st.capitalize()


func catalysts(cat: String) -> Array:
	if CATALYSTS.has(cat):
		return CATALYSTS[cat].duplicate()
	var st := cat.trim_prefix("station:")
	return [st] if items.has(st) else []


func _group(rids: Array) -> Array:
	var by_cat := {}
	for rid in rids:
		var c := category_of(recipe(rid))
		if not by_cat.has(c):
			by_cat[c] = []
		if not rid in by_cat[c]:
			by_cat[c].append(rid)
	var cats: Array = by_cat.keys()
	cats.sort_custom(func(a: String, b: String) -> bool:
		var ia := CATEGORY_ORDER.find(a)
		var ib := CATEGORY_ORDER.find(b)
		ia = 99 if ia < 0 else ia
		ib = 99 if ib < 0 else ib
		return ia < ib if ia != ib else a < b)
	var out := []
	for c in cats:
		out.append({"id": c, "title": category_title(c), "recipes": by_cat[c]})
	return out


func recipes_for(item_id: String) -> Array:
	return _group(recipes.makes(item_id) + _mining_makes.get(item_id, []))


## Uses: recipes taking the item, plus every recipe of a category the item is
## the station (catalyst) of.
func uses_for(item_id: String) -> Array:
	var rids: Array = []
	for cat in _all_categories():
		if item_id in catalysts(cat):
			for rid in _recipes_in(cat):
				rids.append(rid)
	for rid in recipes.uses_of(item_id) + _mining_uses.get(item_id, []):
		if not rid in rids:
			rids.append(rid)
	return _group(rids)


func _all_categories() -> Array:
	var out := []
	for rid in _all_recipe_ids():
		var c := category_of(recipe(rid))
		if not c in out:
			out.append(c)
	return out


func _recipes_in(cat: String) -> Array:
	var out := []
	for rid in _all_recipe_ids():
		if category_of(recipe(rid)) == cat:
			out.append(rid)
	out.sort()
	return out


## Opens recipes ("recipes") or uses ("uses") of an item. Returns false and
## changes nothing when there are none (JEI ignores the click).
func show(item_id: String, mode: String) -> bool:
	var cats := recipes_for(item_id) if mode == "recipes" else uses_for(item_id)
	if cats.is_empty():
		return false
	if not view.is_empty():
		history.append(view.duplicate(true))
	view = {"mode": mode, "item": item_id, "categories": cats, "cat": 0, "index": 0}
	return true


func back() -> void:
	view = history.pop_back() if not history.is_empty() else {}


func close() -> void:
	view = {}
	history.clear()


func is_open() -> bool:
	return not view.is_empty()


func current_category() -> Dictionary:
	return view["categories"][view["cat"]] if is_open() else {}


func current_recipe() -> Dictionary:
	if not is_open():
		return {}
	var c := current_category()
	return recipe(c["recipes"][view["index"]])


func turn_recipe(direction: int) -> void:
	if is_open():
		view["index"] = posmod(view["index"] + direction, current_category()["recipes"].size())


func turn_category(direction: int) -> void:
	if is_open():
		view["cat"] = posmod(view["cat"] + direction, view["categories"].size())
		view["index"] = 0


# ---------------- mining ----------------

## Adds one "mining" pseudo-recipe per (block, drop) from a MiningTable. Blocks
## without an item to show are skipped, and so are plain self-drops (dirt
## gives dirt) unless a tool is required. Calling it again replaces them.
func set_mining(table: MiningTable) -> void:
	_mining.clear()
	_mining_makes.clear()
	_mining_uses.clear()
	var names: Array = table.materials.keys()
	names.sort()
	for mat in names:
		var e: Dictionary = table.materials[mat]
		var block := items.item_for_material(mat)
		if block == "" and mat.ends_with("_static"):
			block = items.item_for_material(mat.trim_suffix("_static"))
		if block == "" or not e.has("drops"):
			continue
		var drops: Array = e["drops"] if e["drops"] is Array else [{"id": e["drops"], "count": int(e.get("count", 1))}]
		for d in drops:
			var id: String = d.get("id", "")
			var c = d.get("count", 1)
			var lo := int(c[0]) if c is Array else int(c)
			var hi := int(c[1]) if c is Array else int(c)
			var chance := float(d.get("chance", 1.0))
			if not items.has(id):
				continue
			if id == block and not e.get("requires", false) and chance >= 1.0 and hi == 1:
				continue
			var rid := "mining:%s:%s" % [block, id]
			if _mining.has(rid):
				continue   # sand and sand_static: one entry
			var info := PackedStringArray()
			if lo != hi:
				info.append("%d-%d" % [lo, hi])
			if chance < 1.0:
				info.append("%s %% chance" % str(snappedf(chance * 100.0, 0.1)))
			_mining[rid] = {"id": rid, "type": "mining", "input": block,
				"output": {"id": id, "count": lo},
				"info": ", ".join(info), "needs": _tool_text(e, d)}
			if not _mining_makes.has(id):
				_mining_makes[id] = []
			_mining_makes[id].append(rid)
			if not _mining_uses.has(block):
				_mining_uses[block] = []
			_mining_uses[block].append(rid)


## "Stone Pickaxe or better", "Any pickaxe", "Hand (faster with a shovel)" ...
func _tool_text(e: Dictionary, drop: Dictionary = {}) -> String:
	if drop.get("tool", "") != "":
		return "Any %s" % drop["tool"]
	var tool: String = e.get("tool", "")
	if tool == "":
		return "Hand"
	var tier := maxi(int(e.get("min_tier", 0)), int(e.get("drop_tier", 0)))
	if tier > 0:
		for id in items.items_with_tag(tool):
			if int(items.get_def(id).get("tier", -1)) == tier:
				return "%s or better" % items.display_name(id)
	return ("Any %s" % tool) if e.get("requires", false) or e.has("drop_tier") else ("Hand (faster with a %s)" % tool)


# ---------------- bookmarks ----------------

func is_bookmarked(item_id: String) -> bool:
	return item_id in bookmarks


## Adds or removes a bookmark; returns true when the item is now bookmarked.
func toggle_bookmark(item_id: String) -> bool:
	if item_id == "" or not items.has(item_id):
		return false
	if item_id in bookmarks:
		bookmarks.erase(item_id)
		return false
	bookmarks.append(item_id)
	return true


## Keeps the known ids of `ids` in order, without duplicates (items can vanish between versions).
func set_bookmarks(ids: Array) -> void:
	bookmarks = []
	for id in ids:
		if id is String and items.has(id) and not id in bookmarks:
			bookmarks.append(id)


func save_bookmarks(path := BOOKMARKS_PATH) -> bool:
	var f := FileAccess.open(path, FileAccess.WRITE)
	if f == null:
		return false
	f.store_string(JSON.stringify({"bookmarks": bookmarks}))
	return true


func load_bookmarks(path := BOOKMARKS_PATH) -> void:
	var f := FileAccess.open(path, FileAccess.READ)
	if f == null:
		return
	var data = JSON.parse_string(f.get_as_text())
	if data is Dictionary and data.get("bookmarks") is Array:
		set_bookmarks(data["bookmarks"])


# ---------------- layout ----------------

## Where a recipe's inputs sit: kind "grid" (3x3 crafting), "smelt" (one
## input above the flame), "mine" (the block) or "row" (spellcraft
## ingredients side by side).
## inputs: [{"cell": Vector2i, "spec": id or #tag}].
func layout(r: Dictionary) -> Dictionary:
	var inputs := []
	var kind := "grid"
	match r["type"]:
		"shaped":
			var pattern: Array = r["pattern"]
			for y in pattern.size():
				var row := String(pattern[y])
				for x in row.length():
					if row[x] != " ":
						inputs.append({"cell": Vector2i(x, y), "spec": r["key"][row[x]]})
		"shapeless":
			var ing: Array = r["ingredients"]
			for i in ing.size():
				inputs.append({"cell": Vector2i(i % 3, i / 3), "spec": ing[i]})
		"smelt":
			kind = "smelt"
			inputs.append({"cell": Vector2i(0, 0), "spec": r["input"]})
		"mining":
			kind = "mine"
			inputs.append({"cell": Vector2i(0, 0), "spec": r["input"]})
		"knap", "clayform", "smith":
			kind = "pattern"
			inputs.append({"cell": Vector2i(0, 0), "spec": r["input"]})
		"assemble":
			kind = "row"
			var parts: Array = r["parts"]
			for i in parts.size():
				inputs.append({"cell": Vector2i(i, 0), "spec": parts[i]})
		"kiln", "sift", "dry":
			kind = "smelt"
			inputs.append({"cell": Vector2i(0, 0), "spec": r["input"]})
		"charcoal_pit":
			kind = "row"
			for i in int(r.get("per", 1)):
				inputs.append({"cell": Vector2i(i, 0), "spec": r["input"]})
		"scrape":
			kind = "row"
			inputs.append({"cell": Vector2i(0, 0), "spec": r["input"]})
			inputs.append({"cell": Vector2i(1, 0), "spec": r["tool"]})
		"melt", "cast":
			kind = "row"
			inputs.append({"cell": Vector2i(0, 0), "spec": r["input"]})
			inputs.append({"cell": Vector2i(1, 0), "spec": r["vessel"] if r["type"] == "melt" else r["mold"]})
		_:
			kind = "row"
			var ing: Array = r.get("ingredients", [])
			for i in ing.size():
				inputs.append({"cell": Vector2i(i, 0), "spec": ing[i]})
	var info := ""
	match r["type"]:
		"shaped":
			info = "Any layout" if recipes.any_layout(r) else "Lay it out as shown"
		"shapeless":
			info = "Any layout"
			for k in r.get("keep", []):
				info += ", keeps the " + (k.substr(1) if k.begins_with("#") else items.display_name(k))
		"smelt":
			info = "%.1f s" % (float(r.get("ticks", 200)) / 20.0)
		"mining":
			info = r["info"]
		"knap":
			info = "Chip off the dark cells"
		"clayform":
			info = "%d layers, %d clay" % [r["layers"].size(), ClayFormEntity.balls_for(r)]
		"assemble":
			info = "Lay the first part on the ground, then add the rest in order"
		"smith":
			var n := AnvilEntity.ingots_for(r)
			info = "%d hot ingot%s on an anvil; hammer it to shape" % [n, "" if n == 1 else "s"]
		"kiln":
			info = "4 dry grass, 8 sticks, %d s" % int(Settings.pit_kiln_burn_s)
		"melt":
			info = "%d x %s, %.0f s" % [int(r.get("count", 1)), items.display_name(r["input"]) if items.has(r["input"]) else r["input"], float(r.get("ticks", 200)) / 20.0]
		"cast":
			info = "Pour on the mold; gives the %s back" % items.display_name(r["returns"]) if r.has("returns") else "Pour on the mold"
		"sift":
			info = "%s %% chance" % str(snappedf(float(r.get("chance", 1.0)) * 100.0, 0.1))
			if r.get("station", "") == "pan":
				info += ", sand or gravel under water"
		"scrape":
			info = "On the ground, %d strokes" % int(r["strokes"])
		"dry":
			info = "Lies on the ground %d s" % int(r.get("seconds", 60))
		"charcoal_pit":
			info = "Piles sealed with soil, %d s" % int(Settings.charcoal_pit_burn_s)
		_:
			info = String(r.get("info", ""))   # the Stone Age types say it themselves (ADR 0059)
	var st: String = r.get("station", "")
	var needs := ""
	if r["type"] == "mining":
		needs = r["needs"]
	elif st != "" and not (r["type"] == "smelt" and st == "furnace"):
		needs = items.display_name(st) if items.has(st) else st.capitalize()
	var out := {"kind": kind, "inputs": inputs, "output": r["output"], "info": info, "needs": needs, "id": r["id"]}
	if kind == "pattern":
		out["heights"] = pattern_heights(r)
	return out


## Top view of a knapping or clay shape: per cell (row-major 8x8) how many
## layers are filled (knapping: 1 kept, 0 chipped).
static func pattern_heights(r: Dictionary) -> PackedInt32Array:
	var h := PackedInt32Array()
	h.resize(64)
	var layers: Array = r["layers"] if r["type"] == "clayform" or r["type"] == "smith" else [r["pattern"]]
	for l in layers:
		for z in 8:
			var row := String(l[z])
			for x in 8:
				if x < row.length() and row[x] == "#":
					h[z * 8 + x] += 1
	return h


## Items an ingredient spec stands for, in items.json order.
func members(spec: String) -> Array:
	if not spec.begins_with("#"):
		return [spec]
	var m := items.items_with_tag(spec.substr(1))
	m.sort_custom(func(a: String, b: String) -> bool: return _order.get(a, 0) < _order.get(b, 0))
	return m


## What one input cell stands for, counted as the inventory counts it: a block's worth
## of voxels for a voxel item (64), one of anything else (ADR 0046).
func cell_amount(spec: String) -> int:
	var m := members(spec)
	return items.units(m[0]) if not m.is_empty() else 1


## The output as the inventory counts it (2 planks by recipe = 128 plank voxels).
func output_amount(l: Dictionary) -> int:
	return int(l["output"].get("count", 1)) * items.units(l["output"]["id"])


## Per distinct ingredient of a layout: {spec, name, need, have}; have is -1 without a player.
func needs_list(l: Dictionary) -> Array:
	var out := []
	var at := {}
	for inp in l["inputs"]:
		var spec: String = inp["spec"]
		if not at.has(spec):
			at[spec] = out.size()
			var nm: String = spec.substr(1).capitalize() if spec.begins_with("#") else items.display_name(spec)
			var h := -1
			if have.is_valid():
				h = 0
				for id in members(spec):
					h += int(have.call(id))
			out.append({"spec": spec, "name": nm, "need": 0, "have": h})
		out[at[spec]]["need"] += cell_amount(spec)
	return out


## The member shown at time step `t` (JEI cycles tag ingredients each second).
func display_item(spec: String, t: int) -> String:
	var m := members(spec)
	return m[posmod(t, m.size())] if not m.is_empty() else ""


func summary() -> Dictionary:
	var s := {"query": query, "page": page, "pages": page_count(), "shown": _filtered.size(), "history": history.size(), "bookmarks": bookmarks.duplicate()}
	if is_open():
		var c := current_category()
		s["view"] = {"mode": view["mode"], "item": view["item"], "category": c["id"],
			"recipe": current_recipe()["id"], "index": view["index"], "of": c["recipes"].size(),
			"categories": view["categories"].map(func(x: Dictionary) -> String: return x["id"])}
	return s
