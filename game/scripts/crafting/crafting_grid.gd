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
