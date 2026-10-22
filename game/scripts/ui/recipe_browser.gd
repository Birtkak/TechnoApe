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
##   on a grid; tag ingredients cycle through their members.

const CATEGORY_ORDER := ["crafting", "smelting", "spellcraft"]
const CATALYSTS := {"crafting": ["crafting_table"], "smelting": ["furnace"]}

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

func category_of(r: Dictionary) -> String:
	match r["type"]:
		"smelt":
			return "smelting"
		"spellcraft":
			return "spellcraft"
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
		var c := category_of(recipes.recipes[rid])
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
	return _group(recipes.makes(item_id))


## Uses: recipes taking the item, plus every recipe of a category the item is
## the station (catalyst) of.
func uses_for(item_id: String) -> Array:
	var rids: Array = []
	for cat in _all_categories():
		if item_id in catalysts(cat):
			for rid in _recipes_in(cat):
				rids.append(rid)
	for rid in recipes.uses_of(item_id):
		if not rid in rids:
			rids.append(rid)
	return _group(rids)


func _all_categories() -> Array:
	var out := []
	for rid in recipes.recipes:
		var c := category_of(recipes.recipes[rid])
		if not c in out:
			out.append(c)
	return out


func _recipes_in(cat: String) -> Array:
	var out := []
	for rid in recipes.recipes:
		if category_of(recipes.recipes[rid]) == cat:
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
	return recipes.recipes[c["recipes"][view["index"]]]


func turn_recipe(direction: int) -> void:
	if is_open():
		view["index"] = posmod(view["index"] + direction, current_category()["recipes"].size())


func turn_category(direction: int) -> void:
	if is_open():
		view["cat"] = posmod(view["cat"] + direction, view["categories"].size())
		view["index"] = 0


# ---------------- layout ----------------

## Where a recipe's inputs sit: kind "grid" (3x3 crafting), "smelt" (one
## input above the flame) or "row" (spellcraft ingredients side by side).
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
		_:
			kind = "row"
			var ing: Array = r.get("ingredients", [])
			for i in ing.size():
				inputs.append({"cell": Vector2i(i, 0), "spec": ing[i]})
	var info := ""
	match r["type"]:
		"shapeless":
			info = "Shapeless"
		"smelt":
			info = "%.1f s" % (float(r.get("ticks", 200)) / 20.0)
	var st: String = r.get("station", "")
	var needs := ""
	if st != "" and not (r["type"] == "smelt" and st == "furnace"):
		needs = items.display_name(st) if items.has(st) else st.capitalize()
	return {"kind": kind, "inputs": inputs, "output": r["output"], "info": info, "needs": needs, "id": r["id"]}


## Items an ingredient spec stands for, in items.json order.
func members(spec: String) -> Array:
	if not spec.begins_with("#"):
		return [spec]
	var m := items.items_with_tag(spec.substr(1))
	m.sort_custom(func(a: String, b: String) -> bool: return _order.get(a, 0) < _order.get(b, 0))
	return m


## The member shown at time step `t` (JEI cycles tag ingredients each second).
func display_item(spec: String, t: int) -> String:
	var m := members(spec)
	return m[posmod(t, m.size())] if not m.is_empty() else ""


func summary() -> Dictionary:
	var s := {"query": query, "page": page, "pages": page_count(), "shown": _filtered.size(), "history": history.size()}
	if is_open():
		var c := current_category()
		s["view"] = {"mode": view["mode"], "item": view["item"], "category": c["id"],
			"recipe": current_recipe()["id"], "index": view["index"], "of": c["recipes"].size(),
			"categories": view["categories"].map(func(x: Dictionary) -> String: return x["id"])}
	return s
