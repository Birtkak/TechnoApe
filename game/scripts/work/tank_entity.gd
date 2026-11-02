class_name TankEntity
extends RefCounted

## The water tank (ADR 0099): an upright riveted drum that holds tank_buckets of water. Water
## buckets fill it, an empty bucket draws one back. On the water channel (ADR 0122) it is storage like the
## battery box on the energy one: a low sink while it fills (pumps feed it from what the boilers leave), a
## source while it gives (boilers on the pipes drink a bucket a second); it flips by itself with a second's lag.

signal changed

var kind := "tank"
var pos: Vector3i
var buckets := 0
var giving := false
var _acc := 0.0   # fractional buckets owed either way by the water channel


func _init(p_pos: Vector3i) -> void:
	pos = p_pos


## Right click with `held`. Returns {consume: n, give: [stacks], msg}.
func use(held: Dictionary) -> Dictionary:
	var id: String = held.get("id", "")
	var res := {"consume": 0, "give": [], "msg": ""}
	if id == "water_bucket":
		if buckets >= Settings.tank_buckets:
			res["msg"] = "The tank is full."
			return res
		buckets += 1
		res["consume"] = 1
		res["give"] = [{"id": String(held.get("empties_to", "bucket")), "count": 1}]
		changed.emit()
		return res
	if id == "bucket":
		if buckets <= 0:
			res["msg"] = "The tank is empty."
			return res
		buckets -= 1
		res["consume"] = 1
		res["give"] = [{"id": "water_bucket", "count": 1}]
		changed.emit()
		return res
	res["msg"] = "Water buckets fill it; a boiler on its pipes drinks from it."
	return res


func net(channel: String) -> Dictionary:
	if channel != "water":
		return {}
	if giving and buckets > 0:
		return {"role": NetworkSolver.SOURCE, "amount": minf(Settings.pipe_water_cap, float(buckets))}
	return {"role": NetworkSolver.SINK_LOW, "amount": minf(Settings.pipe_water_cap, float(Settings.tank_buckets - buckets))}


func net_in(channel: String, amount: float) -> void:
	if channel != "water":
		return
	if giving:
		_acc += amount
		while _acc >= 1.0 - 1e-6 and buckets > 0:
			_acc -= 1.0
			buckets -= 1
			changed.emit()
		if amount <= 0.0 or buckets <= 0:
			giving = false
			_acc = 0.0
	else:
		_acc += amount
		while _acc >= 1.0 - 1e-6 and buckets < Settings.tank_buckets:
			_acc -= 1.0
			buckets += 1
			changed.emit()
		if amount <= 0.0 and buckets > 0:
			giving = true
			_acc = 0.0


## A pump puts a bucket in (ADR 0117); false when full.
func add_bucket() -> bool:
	if buckets >= Settings.tank_buckets:
		return false
	buckets += 1
	changed.emit()
	return true


## A boiler takes a bucket out; false when there is none.
func take_bucket() -> bool:
	if buckets <= 0:
		return false
	buckets -= 1
	changed.emit()
	return true


func contents() -> Array:
	return [{"id": "water_tank", "count": 1}]


func look_lines() -> Array[String]:
	return ["Water: %d of %d buckets" % [buckets, Settings.tank_buckets]]


## An upright riveted drum on four feet with a bronze outlet at the foot, a filler cap on top and a
## sight glass down the front showing the level.
func view_parts() -> Dictionary:
	var size := Vector3i(8, 12, 8)
	var c := WorkPalette.new_cells(size)
	for q in [Vector3i(1, 0, 1), Vector3i(6, 0, 1), Vector3i(1, 0, 6), Vector3i(6, 0, 6)]:
		WorkPalette.box(c, size, q, q + Vector3i(1, 1, 1), WorkPalette.IRON_DARK)
	for y in range(1, 11):   # riveted steel plate in courses, dark iron bands with bright rivet heads (ADR 0129)
		for z in 8:
			for x in 8:
				var d2 := (x + 0.5 - 4.0) * (x + 0.5 - 4.0) + (z + 0.5 - 4.0) * (z + 0.5 - 4.0)
				if d2 <= 16.0:
					var band := y % 4 == 2 or y == 10
					c[WorkPalette.idx(size, x, y, z)] = ((WorkPalette.RIVET if (x + z) % 3 == 0 else WorkPalette.IRON_DARK) if band else WorkPalette.STEEL) + 1
	WorkPalette.box(c, size, Vector3i(3, 11, 3), Vector3i(5, 12, 5), WorkPalette.BRONZE)   # the filler cap
	WorkPalette.box(c, size, Vector3i(3, 0, 7), Vector3i(5, 2, 8), WorkPalette.BRONZE)     # the outlet to the pipes
	var level := int(ceil(9.0 * buckets / float(Settings.tank_buckets)))
	WorkPalette.box(c, size, Vector3i(3, 1, 0), Vector3i(5, 10, 1), WorkPalette.STONE_LIGHT)   # the sight glass
	if level > 0:
		WorkPalette.box(c, size, Vector3i(3, 1, 0), Vector3i(5, 1 + level, 1), WorkPalette.TIN)
	return {"solid": {"cells": c, "size": size}}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "buckets": buckets, "giving": giving}


func load_from(d: Dictionary) -> void:
	buckets = int(d.get("buckets", 0))
	giving = bool(d.get("giving", false))


func summary() -> Dictionary:
	return {"kind": kind, "buckets": buckets}
