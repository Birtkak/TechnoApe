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


func _init(p_items: ItemRegistry, p_recipes: RecipeRegistry, p_width: int = 3, p_station: String = "crafting_table") -> void:
	items = p_items
	recipes = p_recipes
	width = p_width
	station = p_station
	grid = Inventory.new(p_items, p_width * p_width)


## The grid as a 3x3 id array (2x2 grids are placed top-left).
func as_3x3() -> Array:
	var out := []
	out.resize(9)
	out.fill("")
	for y in width:
		for x in width:
			var s: Dictionary = grid.get_slot(y * width + x)
			out[y * 3 + x] = s["id"] if not s.is_empty() else ""
	return out


func current_recipe() -> Dictionary:
	return recipes.match_grid(as_3x3(), station)


func preview_output() -> Dictionary:
	var r := current_recipe()
	if r.is_empty():
		return {}
	return {"id": r["output"]["id"], "count": int(r["output"]["count"])}


## Crafts once: consumes one item from every non-empty grid slot and returns
## the output stack, or {} if nothing matches.
func craft_once() -> Dictionary:
	var out := preview_output()
	if out.is_empty():
		return {}
	for i in grid.size():
		if not grid.is_empty_slot(i):
			grid.take(i, 1)
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
