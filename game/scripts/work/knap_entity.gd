class_name KnapEntity
extends RefCounted

## A piece of flint being knapped on the ground (ADR 0023, Vintage Story's
## knapping): an 8x8 slab one cell (6.25 cm) thick on the WORK_SPOT voxel.
## The player picks a "knap" recipe; its pattern (8 rows of 8, row = z,
## column = x, '#' = keep) marks the cells to chip off. Only marked cells can
## be chipped, so a wrong click costs nothing; the last chip finishes it.
## Breaking an unfinished piece gives the flint back.

signal changed

const N := 8

var kind := "knap"
var pos: Vector3i
var input := "flint"
var recipe: Dictionary = {}
var chipped := PackedByteArray()   # 1 = removed
var chips := 0                     # for debug JSON


func _init(p_pos: Vector3i, p_input: String = "flint") -> void:
	pos = p_pos
	input = p_input
	chipped.resize(N * N)
	chipped.fill(0)


func set_recipe(r: Dictionary) -> void:
	recipe = r
	changed.emit()


func has_recipe() -> bool:
	return not recipe.is_empty()


func keeps(x: int, z: int) -> bool:
	if recipe.is_empty():
		return true
	var row := String(recipe["pattern"][z])
	return x < row.length() and row[x] == "#"


func is_marked(x: int, z: int) -> bool:
	return not keeps(x, z) and chipped[x + N * z] == 0


func remaining() -> int:
	var n := 0
	for z in N:
		for x in N:
			if is_marked(x, z):
				n += 1
	return n


## Chips cell (x, z). Returns "" (nothing to chip there), "chipped" or "done".
func chip(x: int, z: int) -> String:
	if recipe.is_empty() or x < 0 or z < 0 or x >= N or z >= N or not is_marked(x, z):
		return ""
	chipped[x + N * z] = 1
	chips += 1
	changed.emit()
	return "done" if remaining() == 0 else "chipped"


## The nearest marked cell to (x, z) within `radius` cells, or (-1, -1): forgiving aim.
func nearest_marked(x: int, z: int, radius: int = 1) -> Vector2i:
	var best := Vector2i(-1, -1)
	var best_d := 1 << 30
	for dz in range(-radius, radius + 1):
		for dx in range(-radius, radius + 1):
			var cx := x + dx
			var cz := z + dz
			if cx >= 0 and cz >= 0 and cx < N and cz < N and is_marked(cx, cz):
				var d := dx * dx + dz * dz
				if d < best_d:
					best_d = d
					best = Vector2i(cx, cz)
	return best


func output() -> Dictionary:
	var o: Dictionary = recipe.get("output", {})
	return {"id": o["id"], "count": int(o.get("count", 1))} if not o.is_empty() else {}


## What breaking it gives back: the flint while nothing was chipped off yet
## (or no recipe was picked), else nothing (the flint is spoilt).
func contents() -> Array:
	return [{"id": input, "count": 1}] if chips == 0 else []


func tick() -> bool:
	return false


func is_lit() -> bool:
	return false


func view_parts() -> Dictionary:
	var size := Vector3i(N, 2, N)   # a flat stone to knap on, the piece one cell above it
	var c := WorkPalette.new_cells(size)
	WorkPalette.box(c, size, Vector3i(0, 0, 0), Vector3i(N, 1, N), WorkPalette.STONE)
	for z in N:
		for x in N:
			if chipped[x + N * z] == 0:
				c[WorkPalette.idx(size, x, 1, z)] = (WorkPalette.FLINT if keeps(x, z) else WorkPalette.FLINT_MARK) + 1
	return {"solid": {"cells": c, "size": size}}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "input": input, "recipe": recipe.get("id", ""),
		"chipped": Array(chipped), "chips": chips}


## Recipes come back through `recipes` (BlockEntities passes its registry).
func load_from(d: Dictionary, recipes: RecipeRegistry = null) -> void:
	input = String(d.get("input", input))
	var rid := String(d.get("recipe", ""))
	if recipes != null and recipes.recipes.has(rid):
		recipe = recipes.recipes[rid]
	var a: Array = d.get("chipped", [])
	for i in mini(a.size(), N * N):
		chipped[i] = int(a[i])
	chips = int(d.get("chips", 0))


func summary() -> Dictionary:
	return {"kind": kind, "pos": pos, "recipe": recipe.get("id", ""), "remaining": remaining() if has_recipe() else -1, "chips": chips}
