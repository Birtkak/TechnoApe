class_name GroundEntity
extends RefCounted

## One piece of clay ware lying on the ground (ADR 0023): Vintage Story's
## ground storage, pit kiln and casting in one block entity on a WORK_SPOT.
## - Pit kiln: raw ware with a "kiln" recipe takes KILN_GRASS dry grass, then
##   KILN_STICKS sticks; a firestarter lights it; after Settings.pit_kiln_burn_s
##   the ware becomes the recipe's output and the fuel is gone.
## - Casting: a fired mold takes a molten crucible whose "cast" recipe names
##   it; the crucible comes back (`returns`), the metal cools for
##   Settings.cast_cool_s, then a right click takes the casting out.
## - Anything else: a right click picks the ware back up.
## use() answers with what Main must do to the player's hand.

signal changed

const KILN_GRASS := 4
const KILN_STICKS := 8

var kind := "ground"
var pos: Vector3i
var item := ""
var grass := 0
var sticks := 0
var burn := 0          # ticks of kiln fire left
var cast_output := ""  # casting in the mold
var cool := 0          # ticks until the casting can be taken
var recipes: RecipeRegistry


func _init(p_recipes: RecipeRegistry, p_pos: Vector3i, p_item: String = "") -> void:
	recipes = p_recipes
	pos = p_pos
	item = p_item


func is_lit() -> bool:
	return burn > 0


func kiln_recipe() -> Dictionary:
	return recipes.first_of("kiln", {"input": item})


## Right click with `held` (an item definition, {} for an empty hand).
## Returns {consume: n, replace_held: id, give: [stacks], remove: bool, msg: text}.
func use(held: Dictionary) -> Dictionary:
	var id: String = held.get("id", "")
	var res := {"consume": 0, "replace_held": "", "give": [], "remove": false, "msg": ""}
	if burn > 0:
		res["msg"] = "The pit kiln is burning."
		return res
	if cast_output != "":
		if cool > 0:
			res["msg"] = "The metal is still hot."
		else:
			res["give"] = [{"id": cast_output, "count": 1}]
			cast_output = ""
			changed.emit()
		return res
	var cast := recipes.first_of("cast", {"input": id, "mold": item}) if id != "" else {}
	if not cast.is_empty():
		cast_output = cast["output"]["id"]
		cool = int(Settings.cast_cool_s * 20.0)
		res["replace_held"] = cast.get("returns", "")
		res["consume"] = 0 if res["replace_held"] != "" else 1
		changed.emit()
		return res
	var kiln := kiln_recipe()
	if not kiln.is_empty():
		if grass < KILN_GRASS:
			if id == "dry_grass":
				grass += 1
				res["consume"] = 1
				changed.emit()
				return res
			if grass > 0 or id != "":
				res["msg"] = "A pit kiln needs %d dry grass (%d/%d)." % [KILN_GRASS, grass, KILN_GRASS]
				return res
		elif sticks < KILN_STICKS:
			if id != "" and recipes.items.matches(id, "#stick"):
				sticks += 1
				res["consume"] = 1
				changed.emit()
				return res
			res["msg"] = "Now add %d sticks (%d/%d)." % [KILN_STICKS, sticks, KILN_STICKS]
			return res
		else:
			if "firestarter" in held.get("tags", []):
				burn = int(Settings.pit_kiln_burn_s * 20.0)
				changed.emit()
				res["msg"] = "The pit kiln is lit."
			else:
				res["msg"] = "Light the pit kiln with a firestarter."
			return res
	if id == "":
		res["give"] = [{"id": item, "count": 1}]
		res["remove"] = true
	return res


## One tick (1/20 s). Emits changed when the look changes (fired, cooled).
func tick() -> bool:
	if burn > 0:
		burn -= 1
		if burn == 0:
			var r := kiln_recipe()
			if not r.is_empty():
				item = r["output"]["id"]
			grass = 0
			sticks = 0
			changed.emit()
	if cool > 0:
		cool -= 1
		if cool == 0:
			changed.emit()
	return false


func contents() -> Array:
	var out := [{"id": item, "count": 1}]
	if burn == 0:
		if grass > 0:
			out.append({"id": "dry_grass", "count": grass})
		if sticks > 0:
			out.append({"id": "stick", "count": sticks})
	if cast_output != "":
		out.append({"id": cast_output, "count": 1})
	return out


## The ware's shape: the clayform recipe that makes it, or (fired ware) the
## one that makes its raw form.
func _shape_recipe() -> Dictionary:
	var r := recipes.first_of("clayform", {"output": item})
	if r.is_empty():
		var k := recipes.first_of("kiln", {"output": item})
		if not k.is_empty():
			r = recipes.first_of("clayform", {"output": k["input"]})
	return r


func view_parts() -> Dictionary:
	var shape := _shape_recipe()
	var raw := not kiln_recipe().is_empty()
	var base := ClayFormEntity.shape_cells(shape, WorkPalette.CLAY if raw else WorkPalette.FIRED) if not shape.is_empty() else {}
	var h: int = base["size"].y if not base.is_empty() else 2
	var size := Vector3i(8, mini(h + (3 if grass > 0 else 0), 16), 8)
	var c := WorkPalette.new_cells(size)
	if base.is_empty():
		WorkPalette.box(c, size, Vector3i(2, 0, 2), Vector3i(6, 2, 6), WorkPalette.FIRED)
	else:
		var bs: Vector3i = base["size"]
		for y in bs.y:
			for z in bs.z:
				for x in bs.x:
					var v: int = base["cells"][WorkPalette.idx(bs, x, y, z)]
					if v != 0:
						c[WorkPalette.idx(size, x, y, z)] = v
	# metal in the mold's cavity: top-layer holes over filled cells
	if cast_output != "" and not base.is_empty():
		var bs: Vector3i = base["size"]
		var top := bs.y - 1
		for z in bs.z:
			for x in bs.x:
				if top > 0 and base["cells"][WorkPalette.idx(bs, x, top, z)] == 0 and base["cells"][WorkPalette.idx(bs, x, top - 1, z)] != 0:
					c[WorkPalette.idx(size, x, top, z)] = (WorkPalette.MOLTEN if cool > 0 else WorkPalette.COPPER) + 1
	# the kiln: a straw mound, then a stick shell over it, glowing while it burns
	if grass > 0:
		var top := mini(h + 1, size.y)
		for z in 8:
			for x in 8:
				var edge := x == 0 or z == 0 or x == 7 or z == 7
				var paint := WorkPalette.STRAW
				if sticks > 0 and ((x + z) % 2 == 0 or edge) and (x * 8 + z) % KILN_STICKS < sticks + 1:
					paint = WorkPalette.WOOD
				if burn > 0 and (x * 3 + z * 5) % 4 == 0:
					paint = WorkPalette.EMBER
				for y in top:
					var idx := WorkPalette.idx(size, x, y, z)
					if c[idx] == 0 and (edge or y == top - 1):
						c[idx] = paint + 1
	return {"solid": {"cells": c, "size": size}}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "item": item, "grass": grass, "sticks": sticks,
		"burn": burn, "cast": cast_output, "cool": cool}


func load_from(d: Dictionary) -> void:
	item = String(d.get("item", item))
	grass = int(d.get("grass", 0))
	sticks = int(d.get("sticks", 0))
	burn = int(d.get("burn", 0))
	cast_output = String(d.get("cast", ""))
	cool = int(d.get("cool", 0))


func summary() -> Dictionary:
	return {"kind": kind, "pos": pos, "item": item, "grass": grass, "sticks": sticks, "burn": burn,
		"cast": cast_output, "cool": cool}
