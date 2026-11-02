class_name OreCrusherEntity
extends SteamSieveEntity

## The ore crusher (ADR 0099): steel jaws in an iron frame, worked by a boiler on its pipes. A right
## click with a block of ore loads it (up to ore_crusher_loads); every ore_crusher_crush_s at working
## pressure it crushes one block into two of its crushed ore (the ore's `crushed` item), which
## smelt one for one: twice the metal per block. An empty hand takes what it crushed. Rusts per
## crush like the sieve, and an iron plate mends it.


func _init(p_recipes: RecipeRegistry, p_pos: Vector3i) -> void:
	super(p_recipes, p_pos)
	kind = "ore_crusher"


func tick() -> bool:
	if not powered() or loads <= 0:
		return false
	_cooldown -= BlockEntities.TICK_S * clampf(boiler.pressure, 0.25, 2.0)
	if _cooldown > 0.0:
		return false
	_cooldown = Settings.ore_crusher_crush_s
	if not boiler.draw(Settings.steam_per_use):
		return false
	wear = minf(1.0, wear + 1.0 / (Settings.boiler_wear_s / Settings.ore_crusher_crush_s))
	shook.emit(pos, input)
	_keep({"id": String(recipes.items.get_def(input)["crushed"]), "count": 2})
	loads -= 1
	sifted += 1
	if loads == 0:
		input = ""
	changed.emit()
	return false


func use(held: Dictionary, _rng_unused: RandomNumberGenerator) -> Dictionary:
	var id: String = held.get("id", "")
	var res := {"consume": 0, "drops": [], "msg": ""}
	if id == "":
		if found.is_empty():
			res["msg"] = "Load it with blocks of ore; steam does the crushing."
		else:
			res["drops"] = found
			found = []
			changed.emit()
		return res
	if id == "iron_plate" and wear > 0.0:
		wear = 0.0
		res["consume"] = 1
		res["msg"] = "The crusher is patched up."
		changed.emit()
		return res
	if not recipes.items.get_def(id).has("crushed"):
		res["msg"] = "The crusher takes blocks of ore."
		return res
	if input != "" and input != id:
		res["msg"] = "It is crushing %s now." % recipes.items.display_name(input)
		return res
	if loads >= Settings.ore_crusher_loads:
		res["msg"] = "The crusher holds no more."
		return res
	input = id
	loads += 1
	res["consume"] = recipes.items.units(id)
	changed.emit()
	return res


func contents() -> Array:
	var out := [{"id": "ore_crusher", "count": 1}]
	if input != "":
		out.append({"id": input, "count": recipes.items.units(input) * loads})
	out.append_array(found)
	return out


func look_lines() -> Array[String]:
	var out: Array[String] = []
	if wear >= 1.0:
		out.append("Rusted through: fit an iron plate")
	elif boiler == null:
		out.append("No steam: lay pipes from a boiler")
	elif boiler.pressure <= 0.0:
		out.append("No pressure: the boiler is cold")
	else:
		out.append("Steam at %.2f bar" % boiler.pressure)
	if loads > 0:
		out.append("Crushing %d %s" % [loads, recipes.items.display_name(input)])
	for f in found:
		out.append("Crushed: %d %s (empty hand takes it)" % [f["count"], recipes.items.display_name(f["id"])])
	return out


## An iron frame with a hopper mouth on top and two steel jaws in it that close on every crush.
func view_parts() -> Dictionary:
	var size := Vector3i(8, 10, 8)
	var c := WorkPalette.new_cells(size)
	for q in [Vector3i(0, 0, 0), Vector3i(7, 0, 0), Vector3i(0, 0, 7), Vector3i(7, 0, 7)]:
		WorkPalette.box(c, size, q, q + Vector3i(1, 6, 1), WorkPalette.IRON_DARK)
	WorkPalette.box(c, size, Vector3i(0, 6, 0), Vector3i(8, 7, 8), WorkPalette.IRON_DARK)          # the top ring
	WorkPalette.box(c, size, Vector3i(1, 6, 1), Vector3i(7, 7, 7), 0 - 1)
	WorkPalette.hazard_box(c, size, Vector3i(0, 6, 7), Vector3i(8, 7, 8))                          # its front edge
	WorkPalette.box(c, size, Vector3i(1, 7, 1), Vector3i(7, 10, 2), WorkPalette.PAINT)             # the hopper walls
	WorkPalette.box(c, size, Vector3i(1, 7, 6), Vector3i(7, 10, 7), WorkPalette.PAINT)
	WorkPalette.box(c, size, Vector3i(1, 7, 1), Vector3i(2, 10, 7), WorkPalette.PAINT)
	WorkPalette.box(c, size, Vector3i(6, 7, 1), Vector3i(7, 10, 7), WorkPalette.PAINT)
	WorkPalette.box(c, size, Vector3i(1, 9, 1), Vector3i(7, 10, 7), WorkPalette.STEEL)             # its rim
	WorkPalette.box(c, size, Vector3i(2, 7, 2), Vector3i(6, 10, 6), 0 - 1)
	var j := sifted % 2   # the jaws close on every crush
	WorkPalette.box(c, size, Vector3i(1, 1, 2), Vector3i(3 + j, 6, 6), WorkPalette.ANVIL)
	WorkPalette.box(c, size, Vector3i(5 - j, 1, 2), Vector3i(7, 6, 6), WorkPalette.ANVIL)
	WorkPalette.box(c, size, Vector3i(3, 0, 7), Vector3i(5, 2, 8), WorkPalette.BRONZE)             # the steam inlet
	WorkPalette.box(c, size, Vector3i(3, 2, 6), Vector3i(5, 5, 7), WorkPalette.ANVIL if powered() else WorkPalette.IRON_DARK)   # the cylinder
	if loads > 0:
		WorkPalette.box(c, size, Vector3i(2, 7, 2), Vector3i(6, 8 + mini(loads / 6, 1), 6), WorkPalette.GRAVEL)   # the ore heap
	if wear > 0.5:
		WorkPalette.box(c, size, Vector3i(0, 0, 0), Vector3i(1, 3, 2), WorkPalette.BRICK_DARK)   # rust
	return {"solid": {"cells": c, "size": size}}
