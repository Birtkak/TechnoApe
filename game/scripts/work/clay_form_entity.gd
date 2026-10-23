class_name ClayFormEntity
extends RefCounted

## Clay being formed on the ground (ADR 0023, Vintage Story's clay forming):
## the player picks a "clayform" recipe, whose `layers` (bottom first, each
## 8 rows of 8, row = z, column = x, '#' = clay) is the target shape. The
## lowest layer with missing cells is the active one; its missing cells show
## as ghosts. Adding fills a 2x2 brush clipped to the target. Every clay
## ball pays for CELLS_PER_BALL cells; unused credit and placed clay come back
## as balls when the piece is broken before it is done.

signal changed

const N := 8
const CELLS_PER_BALL := 32

var kind := "clayform"
var pos: Vector3i
var input := "clay_ball"
var recipe: Dictionary = {}
var filled := PackedByteArray()
var credit := 0        # cells paid for by consumed balls but not placed yet
var placed := 0


func _init(p_pos: Vector3i, p_input: String = "clay_ball") -> void:
	pos = p_pos
	input = p_input


func set_recipe(r: Dictionary) -> void:
	recipe = r
	filled.resize(N * N * height())
	filled.fill(0)
	changed.emit()


func has_recipe() -> bool:
	return not recipe.is_empty()


func height() -> int:
	return recipe.get("layers", []).size()


static func target_of(r: Dictionary, x: int, y: int, z: int) -> bool:
	var layers: Array = r.get("layers", [])
	if y < 0 or y >= layers.size() or z < 0 or z >= N or x < 0:
		return false
	var row := String(layers[y][z])
	return x < row.length() and row[x] == "#"


func is_target(x: int, y: int, z: int) -> bool:
	return target_of(recipe, x, y, z)


func _i(x: int, y: int, z: int) -> int:
	return x + N * (z + N * y)


func is_filled(x: int, y: int, z: int) -> bool:
	return filled[_i(x, y, z)] != 0


func missing_in(y: int) -> int:
	var n := 0
	for z in N:
		for x in N:
			if is_target(x, y, z) and not is_filled(x, y, z):
				n += 1
	return n


## The layer being worked: the lowest with missing cells, or height() when done.
func active_layer() -> int:
	for y in height():
		if missing_in(y) > 0:
			return y
	return height()


func is_done() -> bool:
	return has_recipe() and active_layer() >= height()


## Fills the 2x2 brush with its low corner at (x, z) on the active layer.
## `balls` = clay balls the player can spend. Returns {added, balls_used, done}.
func add(x: int, z: int, balls: int) -> Dictionary:
	var res := {"added": 0, "balls_used": 0, "done": false}
	if not has_recipe() or is_done():
		return res
	var y := active_layer()
	for dz in 2:
		for dx in 2:
			var cx := x + dx
			var cz := z + dz
			if cx < 0 or cz < 0 or cx >= N or cz >= N or not is_target(cx, y, cz) or is_filled(cx, y, cz):
				continue
			if credit == 0:
				if balls - res["balls_used"] <= 0:
					continue
				res["balls_used"] += 1
				credit += CELLS_PER_BALL
			credit -= 1
			filled[_i(cx, y, cz)] = 1
			placed += 1
			res["added"] += 1
	res["done"] = is_done()
	if res["added"] > 0:
		changed.emit()
	return res


## Balls the whole shape needs (for the picker and the browser).
static func balls_for(r: Dictionary) -> int:
	var cells := 0
	var layers: Array = r.get("layers", [])
	for y in layers.size():
		for z in N:
			for x in N:
				if target_of(r, x, y, z):
					cells += 1
	return ceili(float(cells) / CELLS_PER_BALL)


func output() -> Dictionary:
	var o: Dictionary = recipe.get("output", {})
	return {"id": o["id"], "count": int(o.get("count", 1))} if not o.is_empty() else {}


func contents() -> Array:
	var balls := (placed + credit) / CELLS_PER_BALL
	return [{"id": input, "count": balls}] if balls > 0 else []


func tick() -> bool:
	return false


func is_lit() -> bool:
	return false


## Cells of a finished shape (ground storage draws raw and fired ware with it).
static func shape_cells(r: Dictionary, paint: int) -> Dictionary:
	var h: int = maxi(r.get("layers", []).size(), 1)
	var size := Vector3i(N, h, N)
	var c := WorkPalette.new_cells(size)
	for y in h:
		for z in N:
			for x in N:
				if target_of(r, x, y, z):
					c[WorkPalette.idx(size, x, y, z)] = paint + 1
	return {"cells": c, "size": size}


func view_parts() -> Dictionary:
	if not has_recipe():
		# a lump of clay waiting for its recipe
		var size := Vector3i(N, 2, N)
		var c := WorkPalette.new_cells(size)
		WorkPalette.box(c, size, Vector3i(2, 0, 2), Vector3i(6, 2, 6), WorkPalette.CLAY)
		return {"solid": {"cells": c, "size": size}}
	var size := Vector3i(N, height(), N)
	var solid := WorkPalette.new_cells(size)
	var ghost := WorkPalette.new_cells(size)
	var active := active_layer()
	for y in height():
		for z in N:
			for x in N:
				if is_filled(x, y, z):
					solid[WorkPalette.idx(size, x, y, z)] = WorkPalette.CLAY + 1
				elif y == active and is_target(x, y, z):
					ghost[WorkPalette.idx(size, x, y, z)] = WorkPalette.CLAY + 1
	return {"solid": {"cells": solid, "size": size}, "ghost": {"cells": ghost, "size": size}}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "input": input, "recipe": recipe.get("id", ""),
		"filled": Array(filled), "credit": credit, "placed": placed}


func load_from(d: Dictionary, recipes: RecipeRegistry = null) -> void:
	input = String(d.get("input", input))
	var rid := String(d.get("recipe", ""))
	if recipes != null and recipes.recipes.has(rid):
		set_recipe(recipes.recipes[rid])
		var a: Array = d.get("filled", [])
		for i in mini(a.size(), filled.size()):
			filled[i] = int(a[i])
	credit = int(d.get("credit", 0))
	placed = int(d.get("placed", 0))


func summary() -> Dictionary:
	return {"kind": kind, "pos": pos, "recipe": recipe.get("id", ""), "layer": active_layer() if has_recipe() else -1,
		"placed": placed, "credit": credit}
