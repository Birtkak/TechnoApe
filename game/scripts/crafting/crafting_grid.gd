class_name CraftingGrid
extends RefCounted

## A 3x3 crafting grid (2x2 in the player inventory: pass size 2) with the
## Minecraft flow: the output slot shows what the current grid would make;
## taking the output consumes one of each ingredient.

var items: ItemRegistry
var recipes: RecipeRegistry
var grid: Inventory
var width: int
var station: String
var tools_broken: Array[String] = []   # kept tools that wore out while crafting (ADR 0042); the owner reads and clears it


func _init(p_items: ItemRegistry, p_recipes: RecipeRegistry, p_width: int = 3, p_station: String = "crafting_table") -> void:
	items = p_items
	recipes = p_recipes
	width = p_width
	station = p_station
	grid = Inventory.new(p_items, p_width * p_width)


## The grid as a 3x3 id array (2x2 grids are placed top-left). A slot holding
## less than one recipe unit of a voxel item (a block's worth, ADR 0034) counts as empty.
func as_3x3() -> Array:
	var out := []
	out.resize(9)
	out.fill("")
	for y in width:
		for x in width:
			var s: Dictionary = grid.get_slot(y * width + x)
			out[y * 3 + x] = s["id"] if not s.is_empty() and int(s["count"]) >= items.units(s["id"]) else ""
	return out


func current_recipe() -> Dictionary:
	return recipes.match_grid(as_3x3(), station)


## Grid slots holding a voxel item short of one recipe unit (a block's worth): they
## count as empty, and the screen marks them with what they need (ADR 0046).
func short_slots() -> Array[int]:
	var out: Array[int] = []
	for i in grid.size():
		var s: Dictionary = grid.get_slot(i)
		if not s.is_empty() and int(s["count"]) < items.units(s["id"]):
			out.append(i)
	return out


## Why the grid makes nothing, in a line for under the output slot ("" when it makes
## something or holds nothing): too little of a voxel item, or the wrong station.
func hint() -> String:
	if not current_recipe().is_empty():
		return ""
	var full := []   # the grid as if every short slot held a whole unit
	full.resize(9)
	full.fill("")
	for y in width:
		for x in width:
			var s: Dictionary = grid.get_slot(y * width + x)
			full[y * 3 + x] = s.get("id", "")
	if full.all(func(id: String) -> bool: return id == ""):
		return ""
	var short := short_slots()
	if not short.is_empty() and not recipes.match_grid(full, station).is_empty():
		var id: String = grid.get_slot(short[0])["id"]
		return "Each slot needs %d %s (one block)" % [items.units(id), items.display_name(id)]
	var any := recipes.match_grid(full, station, true)
	if not any.is_empty():
		var st: String = any.get("station", "")
		return "Needs a %s" % (items.display_name(st) if items.has(st) else st.capitalize())
	return ""


func preview_output() -> Dictionary:
	var r := current_recipe()
	if r.is_empty():
		return {}
	return {"id": r["output"]["id"], "count": int(r["output"]["count"]) * items.units(r["output"]["id"])}


## Crafts once: consumes one recipe unit from every non-empty grid slot and returns
## the output stack, or {} if nothing matches. A recipe's "keep" specs (the
## axe that splits a log) stay in their slot; "remainder" turns an ingredient
## into another item in place (the pot of stew leaves its pot, Minecraft's
## milk bucket) (ADR 0030). A kept tool wears by one use (ADR 0042).
func craft_once() -> Dictionary:
	var r := current_recipe()
	if r.is_empty():
		return {}
	var out := {"id": r["output"]["id"], "count": int(r["output"]["count"]) * items.units(r["output"]["id"])}
	var keep: Array = r.get("keep", []).duplicate()
	var remainder: Dictionary = r.get("remainder", {})
	for i in grid.size():
		if grid.is_empty_slot(i) or int(grid.get_slot(i)["count"]) < items.units(grid.get_slot(i)["id"]):
			continue
		var id: String = grid.get_slot(i)["id"]
		var kept := false
		for k in keep.size():
			if items.matches(id, keep[k]):
				keep.remove_at(k)   # each keep spec spares one slot
				kept = true
				break
		if kept:
			var broke := grid.wear_slot(i, 1)   # the kept axe or knife wears (ADR 0042)
			if broke != "":
				tools_broken.append(broke)
			continue
		grid.take(i, items.units(id))
		if remainder.has(id):
			grid.put(i, {"id": remainder[id], "count": 1})
	return out


## Shift-click craft: repeats until ingredients run out or `max_times` is hit.
## Returns the total stack crafted.
func craft_all(max_times: int = 64) -> Dictionary:
	var total := {}
	for _i in max_times:
		var out := craft_once()
		if out.is_empty():
			break
		if total.is_empty():
			total = out
		else:
			total["count"] += out["count"]
	return total


# ---------------- fill from the recipe browser (bug report 2026-10-25) ----------------

## Whether recipe r is a grid recipe at all (shaped or shapeless): the Fill button shows for these.
static func is_grid_recipe(r: Dictionary) -> bool:
	return r.get("type", "") in ["shaped", "shapeless"]


## Why this grid cannot take recipe r ("" when it can): another station, or too wide for the 2x2.
func cannot_hold(r: Dictionary) -> String:
	if not is_grid_recipe(r):
		return "Not a grid recipe"
	var st: String = r.get("station", "")
	if st != "" and st != station:
		return "Needs a %s" % (items.display_name(st) if items.has(st) else st.capitalize())
	if _cells_for(r).is_empty():
		return "Needs a 3x3 grid (crafting table)"
	return ""


## Where recipe r's ingredients go: [[grid slot, spec], ...] (shaped as drawn from the top left,
## shapeless in reading order), or [] when it does not fit this grid's width.
func _cells_for(r: Dictionary) -> Array:
	var out := []
	if r["type"] == "shaped":
		var pattern: Array = r["pattern"]
		if pattern.size() > width:
			return []
		for y in pattern.size():
			var row := String(pattern[y])
			for x in row.length():
				if row[x] == " ":
					continue
				if x >= width:
					return []
				out.append([y * width + x, r["key"][row[x]]])
	else:
		var ing: Array = r["ingredients"]
		if ing.size() > width * width:
			return []
		for i in ing.size():
			out.append([i, ing[i]])
	return out


## Puts the grid's contents back into `sources` (inventories, first one first). What does not fit
## stays in its slot. Returns true when the grid ended up empty.
func return_to(sources: Array) -> bool:
	var empty := true
	for i in grid.size():
		var s := grid.take(i)
		if s.is_empty():
			continue
		var left: int = s["count"]
		for inv in sources:
			left = (inv as Inventory).add(s["id"], left, Inventory.data_of(s))
			if left <= 0:
				break
		if left > 0:
			grid.set_slot(i, Inventory.make_stack(items, s["id"], left, s))
			empty = false
	return empty


## Lays recipe r out in the grid from `sources` (the player's inventory, then the bag): the grid's
## contents go back first, then every cell gets one recipe unit per craft for as many crafts as
## `times` asks (Shift: all), as the sources hold (grid contents count, they come back first) and as
## a slot's stack holds. A kept tool (the saw, ADR 0042) goes in once, not per craft. A tag ingredient
## takes the member the player has most of. All or nothing: when not even one craft is there, nothing
## moves and "missing" lists [{id, need, have}]; "error" says why nothing happened.
## `dry_run` only answers. Returns {crafts, missing, error}.
func fill(r: Dictionary, sources: Array, times: int = 1, dry_run := false) -> Dictionary:
	var why := cannot_hold(r)
	if why != "":
		return {"crafts": 0, "missing": [], "error": why}
	var cells := _cells_for(r)
	var avail := func(id: String) -> int:
		var n := 0
		for inv in sources:
			n += (inv as Inventory).count_of(id)
		for i in grid.size():
			var s: Dictionary = grid.get_slot(i)
			if not s.is_empty() and s["id"] == id:
				n += int(s["count"])
		return n
	var keep: Array = r.get("keep", []).duplicate()
	var per_craft := {}   # id -> units per craft
	var once := {}        # id -> units for kept tools
	var plan := []        # [slot, id, kept]
	var cap := maxi(times, 1)
	for c in cells:
		var spec: String = c[1]
		var best := spec
		if spec.begins_with("#"):
			best = ""
			var most := -1
			for m in items.items_with_tag(spec.substr(1)):
				var n: int = avail.call(m)
				if n > most:
					most = n
					best = m
		var kept := keep.has(spec)
		if kept:
			keep.erase(spec)
			once[best] = int(once.get(best, 0)) + items.units(best)
		else:
			per_craft[best] = int(per_craft.get(best, 0)) + items.units(best)
			cap = mini(cap, items.max_stack(best) / items.units(best))
		plan.append([c[0], best, kept])
	var missing := []
	for id in per_craft.keys() + once.keys():
		var have: int = avail.call(id)
		var need: int = int(per_craft.get(id, 0)) + int(once.get(id, 0))
		if have < need:
			if not missing.any(func(m: Dictionary) -> bool: return m["id"] == id):
				missing.append({"id": id, "need": need, "have": have})
		elif per_craft.has(id):
			cap = mini(cap, (have - int(once.get(id, 0))) / int(per_craft[id]))
	if not missing.is_empty():
		return {"crafts": 0, "missing": missing, "error": "Missing " + ", ".join(PackedStringArray(missing.map(func(m: Dictionary) -> String:
			return "%s %d/%d" % [items.display_name(m["id"]) if items.has(m["id"]) else m["id"], m["have"], m["need"]])))}
	if dry_run:
		return {"crafts": cap, "missing": [], "error": ""}
	if not return_to(sources):
		return {"crafts": 0, "missing": [], "error": "Inventory full"}
	for p in plan:
		var id: String = p[1]
		grid.set_slot(p[0], _pull(sources, id, items.units(id) * (1 if p[2] else cap)))
	return {"crafts": cap, "missing": [], "error": ""}


## Takes `count` of id out of `sources` as one stack, keeping its data (a tool's wear, food's spoil).
## Main inventory before the hotbar: slots are emptied from the back, as Inventory.remove does.
func _pull(sources: Array, id: String, count: int) -> Dictionary:
	var out := {}
	for inv: Inventory in sources:
		for i in range(inv.size() - 1, -1, -1):
			if count <= 0:
				return out
			var s := inv.get_slot(i)
			if s.is_empty() or s["id"] != id:
				continue
			var t := inv.take(i, count)
			count -= int(t["count"])
			if out.is_empty():
				out = t
			else:
				Inventory.merge_into(out, int(t["count"]), Inventory.data_of(t))
	return out


## Returns all grid contents to `inv` (closing the table). Anything that does
## not fit is returned so the caller can drop it in the world.
func return_contents(inv: Inventory) -> Array:
	var dropped := []
	for i in grid.size():
		var s := grid.take(i)
		if s.is_empty():
			continue
		var left := inv.add(s["id"], s["count"])
		if left > 0:
			dropped.append({"id": s["id"], "count": left})
	return dropped
