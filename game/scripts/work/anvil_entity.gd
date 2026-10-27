class_name AnvilEntity
extends RefCounted

## A stone anvil and the metal on it (ADR 0042, Vintage Story smithing). A hot ingot laid on the
## anvil becomes a work piece: a lump of 6.25 cm cells on an 8 x 8 grid, one column height per
## cell (`heights`). The player picks a "smith" recipe: its layers (bottom first, 8 rows of 8, '#'
## = metal) give the target height of every column, drawn as a ghost. Hammer blows move metal:
## - hit (left click with a hammer): the top cell of the aimed column moves one cell further away
##   from the player (the camera's facing, snapped to an axis) — metal is driven, not conjured;
## - split (right click with a hammer): the top cell of the aimed column is cut away (lost).
## The piece is done when every column has its target height. Metal works only while hot
## (Inventory.heat_of over hot_until); an empty hand takes the piece off as a work piece item that
## keeps its cells, so it can go back on the fire and back onto the anvil. Another hot ingot of
## the same metal adds a lump on top when the piece is short of metal.
## Since ADR 0070 there are two anvils: the stone anvil (tier 0) and the bronze anvil (tier 1, kind
## "bronze_anvil"); a metal whose item has "anvil_tier" above the anvil's (iron) does not go on.

signal changed

const N := 8
const MAX_H := 4                  # cells of metal a column holds on the anvil (25 cm)
const BASE_H := 4                 # the stone under the work face, in cells
const CELLS_PER_INGOT := 32       # one ingot: a 4 x 4 lump two cells high

var kind := "anvil"
var item := "stone_anvil"         # what breaking it gives
var tier := 0                     # the highest "anvil_tier" of metal it takes
var pos: Vector3i
var recipes: RecipeRegistry
var metal := ""                   # the ingot id the piece was started from ("" = an empty anvil)
var recipe: Dictionary = {}
var heights := PackedByteArray()
var hot_until := 0.0
var hits := 0
var splits := 0


func _init(p_recipes: RecipeRegistry, p_pos: Vector3i, p_kind := "anvil") -> void:
	recipes = p_recipes
	pos = p_pos
	kind = p_kind
	if kind == "bronze_anvil":
		item = "bronze_anvil"
		tier = 1
	heights.resize(N * N)
	heights.fill(0)


func has_piece() -> bool:
	return metal != ""


func has_recipe() -> bool:
	return not recipe.is_empty()


func heat() -> float:
	return Inventory.heat_of({"hot_until": hot_until})


func height(x: int, z: int) -> int:
	return heights[x + N * z] if x >= 0 and z >= 0 and x < N and z < N else 0


## The recipe's target height of column (x, z): its layers with '#' there.
func target(x: int, z: int) -> int:
	return target_of(recipe, x, z)


static func target_of(r: Dictionary, x: int, z: int) -> int:
	var t := 0
	for layer in r.get("layers", []):
		if String(layer[z])[x] == "#":
			t += 1
	return t


static func cells_of(r: Dictionary) -> int:
	var n := 0
	for z in N:
		for x in N:
			n += target_of(r, x, z)
	return n


## Ingots a smith recipe needs: enough cells for its shape.
static func ingots_for(r: Dictionary) -> int:
	return maxi(1, ceili(cells_of(r) / float(CELLS_PER_INGOT)))


func metal_cells() -> int:
	var n := 0
	for h in heights:
		n += h
	return n


## Lays a lump of one ingot's metal in the middle (on top of what is there). False without room.
func _add_lump() -> bool:
	for z in range(2, 6):
		for x in range(2, 6):
			if height(x, z) + 2 > MAX_H:
				return false
	for z in range(2, 6):
		for x in range(2, 6):
			heights[x + N * z] += 2
	return true


## A hot ingot of `id` (a stack carrying hot_until) laid on the anvil: starts a piece, or adds metal
## to one of the same metal. Returns "" when it took it, else why not.
func add_ingot(stack: Dictionary) -> String:
	if not takes(stack["id"]):
		return "%s needs a bronze anvil." % recipes.items.display_name(stack["id"])
	if Inventory.heat_of(stack) < Settings.smith_min_heat:
		return "Heat it in a %s first." % ("forge" if recipes.items.has_tag(stack["id"], "forge_heat") else "firepit")
	if has_piece() and stack["id"] != metal:
		return "There is %s on the anvil already." % recipes.items.display_name(metal)
	if not _add_lump():
		return "There is no room for more metal."
	metal = stack["id"]
	hot_until = maxf(hot_until, float(stack["hot_until"]))
	changed.emit()
	return ""


## Whether metal `id` (an ingot, bloom or work piece) goes on this anvil.
func takes(id: String) -> bool:
	return int(recipes.items.get_def(id).get("anvil_tier", 0)) <= tier


func set_recipe(r: Dictionary) -> void:
	recipe = r
	changed.emit()


## The smith recipes this piece's metal can make.
func options() -> Array:
	var out := []
	for r in recipes.of_type("smith"):
		if recipes.items.matches(metal, r["input"]):
			out.append(r)
	return out


## Hammer blow on column (x, z), pushing along `dir` (a unit axis on x/z). Returns "" (no metal
## there), "cold", "stuck" (the edge of the anvil or a full column in the way), "hit" or "done".
func hit(x: int, z: int, dir: Vector2i) -> String:
	if not has_recipe() or height(x, z) == 0:
		return ""
	if heat() < Settings.smith_min_heat:
		return "cold"
	var nx := x + dir.x
	var nz := z + dir.y
	if nx < 0 or nz < 0 or nx >= N or nz >= N or height(nx, nz) >= MAX_H:
		return "stuck"
	heights[x + N * z] -= 1
	heights[nx + N * nz] += 1
	hits += 1
	changed.emit()
	return "done" if is_done() else "hit"


## Split: cuts the top cell of column (x, z) away. Same answers as hit().
func split(x: int, z: int) -> String:
	if not has_recipe() or height(x, z) == 0:
		return ""
	if heat() < Settings.smith_min_heat:
		return "cold"
	heights[x + N * z] -= 1
	splits += 1
	changed.emit()
	if metal_cells() == 0:
		_clear()
		return "hit"
	return "done" if is_done() else "hit"


func is_done() -> bool:
	if not has_recipe():
		return false
	for z in N:
		for x in N:
			if height(x, z) != target(x, z):
				return false
	return true


## Metal still missing from the shape (more than the piece holds: add an ingot).
func short_of() -> int:
	return maxi(0, cells_of(recipe) - metal_cells()) if has_recipe() else 0


func output() -> Dictionary:
	var o: Dictionary = recipe.get("output", {})
	return {"id": o["id"], "count": int(o.get("count", 1))} if not o.is_empty() else {}


## The finished piece leaves the anvil.
func finish() -> Dictionary:
	var out := output()
	_clear()
	return out


func _clear() -> void:
	metal = ""
	recipe = {}
	heights.fill(0)
	hot_until = 0.0
	changed.emit()


## The piece as an item stack (empty hand): it keeps its cells, recipe and heat.
func take_piece() -> Dictionary:
	var s := _piece_stack()
	if not s.is_empty():
		_clear()
	return s


func _piece_stack() -> Dictionary:
	if not has_piece():
		return {}
	var work := {"metal": metal, "recipe": recipe.get("id", ""), "heights": Array(heights), "output": recipe.get("output", {}).get("id", "")}
	var id: String = recipes.items.get_def(metal).get("work_piece", "")
	if id == "":
		return {}
	var s := {"id": id, "count": 1, "work": work}
	if hot_until > Inventory.clock_s:
		s["hot_until"] = hot_until
	return s


## A work piece item laid back on an empty anvil. Returns "" when it took it, else why not.
func put_piece(stack: Dictionary) -> String:
	if has_piece():
		return "There is metal on the anvil already."
	var w: Dictionary = stack.get("work", {})
	var hs: Array = w.get("heights", [])
	if hs.size() != N * N or not recipes.items.has(String(w.get("metal", ""))):
		return "That work piece is spoilt."
	if not takes(String(w["metal"])):
		return "%s needs a bronze anvil." % recipes.items.display_name(stack["id"])
	metal = String(w["metal"])
	for i in N * N:
		heights[i] = clampi(int(hs[i]), 0, MAX_H)
	recipe = recipes.recipes.get(String(w.get("recipe", "")), {})
	hot_until = float(stack.get("hot_until", 0.0))
	changed.emit()
	return ""


## What breaking the anvil gives: the anvil, and the piece as an item.
func contents() -> Array:
	var out := [{"id": item, "count": 1}]
	var piece := _piece_stack()
	if not piece.is_empty():
		out.append(piece)
	return out


## A tick (1/20 s): only the glow changes, a few times as the metal cools.
var _glow_step := -1

func tick() -> bool:
	if has_piece():
		var step := int(heat() * 4.0)
		if step != _glow_step:
			_glow_step = step
			changed.emit()
	return false


func is_lit() -> bool:
	return false


func view_parts() -> Dictionary:
	var size := Vector3i(N, N, N)
	var c := WorkPalette.new_cells(size)
	if tier > 0:
		# the bronze anvil is cast metal: a wide foot, a waist, the block with its face on top
		WorkPalette.box(c, size, Vector3i(0, 0, 1), Vector3i(8, 1, 7), WorkPalette.COPPER)
		WorkPalette.box(c, size, Vector3i(2, 1, 2), Vector3i(6, 2, 6), WorkPalette.COPPER)
		WorkPalette.box(c, size, Vector3i(0, 2, 0), Vector3i(8, BASE_H, 8), WorkPalette.COPPER)
	else:
		# the stone anvil: a flat boulder with a worked face, set on a log stump
		WorkPalette.box(c, size, Vector3i(1, 0, 1), Vector3i(7, 2, 7), WorkPalette.WOOD_DARK)
		WorkPalette.box(c, size, Vector3i(0, 2, 0), Vector3i(8, BASE_H - 1, 8), WorkPalette.STONE)
		WorkPalette.box(c, size, Vector3i(0, BASE_H - 1, 0), Vector3i(8, BASE_H, 8), WorkPalette.ANVIL)
	var cold := WorkPalette.ANVIL if metal.begins_with("iron") else WorkPalette.COPPER
	var paint := WorkPalette.GLOW if heat() >= 0.5 else (WorkPalette.MOLTEN if heat() >= Settings.smith_min_heat else cold)
	var g := WorkPalette.new_cells(size)
	var ghost_any := false
	for z in N:
		for x in N:
			var h := height(x, z)
			var t := target(x, z) if has_recipe() else h
			for y in maxi(h, t):
				var i := WorkPalette.idx(size, x, BASE_H + y, z)
				if y < h:
					c[i] = (WorkPalette.AVOID if y >= t else paint) + 1   # metal above the shape: red, split or drive it away
				else:
					g[i] = WorkPalette.PLACE + 1
					ghost_any = true
	var out := {"solid": {"cells": c, "size": size}}
	if ghost_any:
		out["ghost"] = {"cells": g, "size": size}
	return out


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "metal": metal, "recipe": recipe.get("id", ""),
		"heights": Array(heights), "hot_until": hot_until, "hits": hits, "splits": splits}


func load_from(d: Dictionary) -> void:
	metal = String(d.get("metal", ""))
	recipe = recipes.recipes.get(String(d.get("recipe", "")), {})
	var hs: Array = d.get("heights", [])
	for i in mini(hs.size(), N * N):
		heights[i] = int(hs[i])
	hot_until = float(d.get("hot_until", 0.0))
	hits = int(d.get("hits", 0))
	splits = int(d.get("splits", 0))


func summary() -> Dictionary:
	return {"kind": kind, "pos": pos, "metal": metal, "recipe": recipe.get("id", ""), "cells": metal_cells(),
		"short": short_of(), "heat": snappedf(heat(), 0.01), "hits": hits, "splits": splits}
