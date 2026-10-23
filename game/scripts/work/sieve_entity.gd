class_name SieveEntity
extends RefCounted

## A sieve (ADR 0019, ADR 0023): a right click with gravel or sand loads one
## block; SHAKES more right clicks (holding repeats) sift it, and every "sift"
## recipe for that input rolls on its own (they form one loot table). The
## loot pops out on top. Solid WORK_BLOCK voxel, drawn by its own micro mesh.

signal changed

const SHAKES := 8

var kind := "sieve"
var pos: Vector3i
var input := ""
var shakes := 0
var sifted := 0
var recipes: RecipeRegistry


func _init(p_recipes: RecipeRegistry, p_pos: Vector3i) -> void:
	recipes = p_recipes
	pos = p_pos


## Right click with `held`. Returns {consume: n, drops: [stacks], msg}.
func use(held: Dictionary, rng: RandomNumberGenerator) -> Dictionary:
	var id: String = held.get("id", "")
	var res := {"consume": 0, "drops": [], "msg": ""}
	if input == "":
		if id != "" and not recipes.sift_table(id, "sieve").is_empty():
			input = id
			shakes = 0
			res["consume"] = recipes.items.units(id)   # a block's worth of gravel or sand voxels (ADR 0034)
			changed.emit()
		else:
			res["msg"] = "Load the sieve with gravel or sand."
		return res
	shakes += 1
	if shakes >= SHAKES:
		res["drops"] = roll(input, rng)
		input = ""
		shakes = 0
		sifted += 1
	changed.emit()
	return res


func roll(id: String, rng: RandomNumberGenerator) -> Array:
	var out := []
	for r in recipes.sift_table(id, "sieve"):
		if rng.randf() < float(r.get("chance", 1.0)):
			out.append({"id": r["output"]["id"], "count": int(r["output"].get("count", 1))})
	return out


func contents() -> Array:
	var out := [{"id": "sieve", "count": 1}]
	if input != "":
		out.append({"id": input, "count": recipes.items.units(input)})
	return out


func tick() -> bool:
	return false


func is_lit() -> bool:
	return false


func view_parts() -> Dictionary:
	var size := Vector3i(8, 8, 8)
	var c := WorkPalette.new_cells(size)
	for q in [Vector3i(0, 0, 0), Vector3i(7, 0, 0), Vector3i(0, 0, 7), Vector3i(7, 0, 7)]:
		WorkPalette.box(c, size, q, q + Vector3i(1, 5, 1), WorkPalette.WOOD_DARK)
	WorkPalette.box(c, size, Vector3i(0, 5, 0), Vector3i(8, 7, 8), WorkPalette.WOOD)
	WorkPalette.box(c, size, Vector3i(1, 5, 1), Vector3i(7, 7, 7), 0 - 1)
	WorkPalette.box(c, size, Vector3i(1, 5, 1), Vector3i(7, 6, 7), WorkPalette.MESH)
	if input != "":
		var h := 7 if shakes < SHAKES / 2 else 6
		WorkPalette.box(c, size, Vector3i(1, 6, 1), Vector3i(7, h + 1, 7), WorkPalette.GRAVEL)
	return {"solid": {"cells": c, "size": size}}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "input": input, "shakes": shakes}


func load_from(d: Dictionary) -> void:
	input = String(d.get("input", ""))
	shakes = int(d.get("shakes", 0))


func summary() -> Dictionary:
	return {"kind": kind, "pos": pos, "input": input, "shakes": shakes, "sifted": sifted}
