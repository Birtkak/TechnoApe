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
