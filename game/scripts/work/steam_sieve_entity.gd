class_name SteamSieveEntity
extends SieveEntity

## The steam sieve (ADR 0097): a sieve on an iron frame, shaken by a boiler on its pipes. A right
## click with gravel or sand loads a block (up to steam_sieve_loads); every steam_sieve_shake_s at
## working pressure it sifts one load with the sieve's loot table and keeps what it finds, which an
## empty hand takes. Rusts with every shake (`wear`), stops at 1 until an iron plate mends it.

signal shook(p: Vector3i, material: String)   # one steam shake (sound, a spray of the input)

var boiler: BoilerEntity = null
var loads := 0
var found: Array = []
var wear := 0.0
var _cooldown := 0.0
var _rng := RandomNumberGenerator.new()


func _init(p_recipes: RecipeRegistry, p_pos: Vector3i) -> void:
	super(p_recipes, p_pos)
	kind = "steam_sieve"


func powered() -> bool:
	return boiler != null and boiler.pressure > 0.0 and wear < 1.0


func tick() -> bool:
	if not powered() or loads <= 0:
		return false
	_cooldown -= BlockEntities.TICK_S * clampf(boiler.pressure, 0.25, 2.0)
	if _cooldown > 0.0:
		return false
	_cooldown = Settings.steam_sieve_shake_s
	if not boiler.draw(Settings.steam_per_use):
		return false
	wear = minf(1.0, wear + 1.0 / (Settings.boiler_wear_s / Settings.steam_sieve_shake_s))
	shook.emit(pos, input)
	for s in roll(input, _rng):
		_keep(s)
	loads -= 1
	sifted += 1
	if loads == 0:
		input = ""
	changed.emit()
	return false


func _keep(s: Dictionary) -> void:
	for f in found:
		if f["id"] == s["id"]:
			f["count"] += s["count"]
			return
	found.append(s)


func use(held: Dictionary, _rng_unused: RandomNumberGenerator) -> Dictionary:
	var id: String = held.get("id", "")
	var res := {"consume": 0, "drops": [], "msg": ""}
	if id == "":
		if found.is_empty():
			res["msg"] = "Load it with gravel or sand; steam does the shaking."
		else:
			res["drops"] = found
			found = []
			changed.emit()
		return res
	if id == "iron_plate" and wear > 0.0:
		wear = 0.0
		res["consume"] = 1
		res["msg"] = "The sieve is patched up."
		changed.emit()
		return res
	if recipes.sift_table(id, "sieve").is_empty():
		res["msg"] = "Load the sieve with gravel or sand."
		return res
	if input != "" and input != id:
		res["msg"] = "It is sifting %s now." % recipes.items.display_name(input)
		return res
	if loads >= Settings.steam_sieve_loads:
		res["msg"] = "The sieve holds no more."
		return res
	input = id
	loads += 1
	res["consume"] = recipes.items.units(id)
	changed.emit()
	return res


func contents() -> Array:
	var out := [{"id": "steam_sieve", "count": 1}]
	if input != "":
		out.append({"id": input, "count": recipes.items.units(input) * loads})
	out.append_array(found)
	return out


func look_lines() -> Array[String]:
	var out: Array[String] = []
	if wear >= 1.0:
		out.append("Rusted through: fit an iron plate")
	elif boiler == null:
		out.append("No steam: lay bronze pipes from a boiler")
	elif boiler.pressure <= 0.0:
		out.append("No pressure: the boiler is cold")
	else:
		out.append("Steam at %.2f bar" % boiler.pressure)
	if loads > 0:
		out.append("Sifting %d %s" % [loads, recipes.items.display_name(input)])
	for f in found:
		out.append("Found: %d %s (empty hand takes it)" % [f["count"], recipes.items.display_name(f["id"])])
	return out


func view_parts() -> Dictionary:
	var size := Vector3i(8, 8, 8)
	var c := WorkPalette.new_cells(size)
	for q in [Vector3i(0, 0, 0), Vector3i(7, 0, 0), Vector3i(0, 0, 7), Vector3i(7, 0, 7)]:
		WorkPalette.box(c, size, q, q + Vector3i(1, 5, 1), WorkPalette.IRON_DARK)
	WorkPalette.box(c, size, Vector3i(0, 5, 0), Vector3i(8, 7, 8), WorkPalette.WOOD)
	WorkPalette.box(c, size, Vector3i(1, 5, 1), Vector3i(7, 7, 7), 0 - 1)
	WorkPalette.box(c, size, Vector3i(1, 5, 1), Vector3i(7, 6, 7), WorkPalette.MESH)
	WorkPalette.box(c, size, Vector3i(3, 0, 7), Vector3i(5, 2, 8), WorkPalette.BRONZE)   # the steam inlet
	WorkPalette.box(c, size, Vector3i(3, 2, 6), Vector3i(5, 5, 7), WorkPalette.ANVIL if powered() else WorkPalette.IRON_DARK)   # the shaker
	if loads > 0:   # the heap shifts side to side with every shake
		var j := sifted % 2
		WorkPalette.box(c, size, Vector3i(1 + j, 6, 1), Vector3i(6 + j, 7 + mini(loads / 4, 1), 7), WorkPalette.GRAVEL)
	return {"solid": {"cells": c, "size": size}}


func to_dict() -> Dictionary:
	var d := super()
	d["loads"] = loads
	d["found"] = found
	d["wear"] = wear
	return d


func load_from(d: Dictionary) -> void:
	super(d)
	loads = int(d.get("loads", 0))
	found = d.get("found", [])
	wear = float(d.get("wear", 0.0))


func summary() -> Dictionary:
	var s := super()
	s["loads"] = loads
	s["found"] = found.size()
	s["powered"] = powered()
	return s
