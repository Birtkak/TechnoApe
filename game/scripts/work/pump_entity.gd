class_name PumpEntity
extends RefCounted

## An electric pump (ADR 0117): on 4 EU/t it draws a bucket (1 m^3, 512 voxels) a second from the water
## under it into its tank, the way a bucket scoops (Bucket.scoop_cells over the bubble: the only edit API
## it uses), and pushes buckets into a water tank beside it. A bucket in hand takes one. While it is
## powered with room in its tank it needs the material simulation (needs_sim), so MachinePins keeps a
## pin alive around it when the player is away.

signal changed

var kind := "pump"
var pos: Vector3i
var buckets := 0
var eu := 0.0
var power_ticks := 0
var drawn := 0        # buckets ever drawn, for debug JSON


func _init(p_pos: Vector3i) -> void:
	pos = p_pos


func powered() -> bool:
	return power_ticks > 0


func wants_water() -> bool:
	return powered() and buckets < Settings.pump_buckets


func needs_sim() -> bool:
	return wants_water()


var _acc := 0.0   # fractional buckets the water channel took


func net(channel: String) -> Dictionary:
	if channel == "energy":
		return {"role": NetworkSolver.SINK, "amount": Settings.pump_eu_t if buckets < Settings.pump_buckets else 0.0}
	if channel == "water":   # ADR 0122: tanks and boilers on the pipes that touch it drink from it
		return {"role": NetworkSolver.SOURCE, "amount": minf(Settings.pipe_water_cap, float(buckets))}
	return {}


func net_in(channel: String, amount: float) -> void:
	if channel == "energy":
		eu = amount
		power_ticks = 21 if amount >= Settings.pump_eu_t - 1e-3 else 0
	elif channel == "water":
		_acc += amount
		while _acc >= 1.0 - 1e-6 and buckets > 0:
			_acc -= 1.0
			buckets -= 1
			changed.emit()


func tick() -> bool:
	if power_ticks > 0:
		power_ticks -= 1
	return false


func is_lit() -> bool:
	return false


## Takes one bucket of connected water starting at `start` out of the world (get_voxel / set_voxel over
## the bubble, pins or terrain) into the tank. False when there is less than a bucket there or no room.
func draw_from(get_voxel: Callable, set_voxel: Callable, start: Vector3i) -> bool:
	if buckets >= Settings.pump_buckets:
		return false
	var water := [GameMaterials.WATER, GameMaterials.id_of("water_static")]
	var cells := Bucket.scoop_cells(get_voxel, start, func(m: int) -> bool: return m in water, Bucket.capacity())
	if cells.size() < Bucket.capacity():
		return false
	for c in cells:
		set_voxel.call(c, GameMaterials.AIR)
	buckets += 1
	drawn += 1
	changed.emit()
	return true


func use(stack: Dictionary) -> Dictionary:
	var res := {"consume": 0, "give": [], "msg": ""}
	if stack.get("id", "") == "bucket":
		if buckets <= 0:
			res["msg"] = "The pump's tank is empty."
			return res
		buckets -= 1
		res["consume"] = 1
		res["give"] = [{"id": "water_bucket", "count": 1}]
		changed.emit()
		return res
	res["msg"] = "%d of %d buckets; %s. A bucket in hand takes one; tanks and boilers on its pipes drink from it." % [buckets, Settings.pump_buckets,
		"pumping" if wants_water() else ("full" if buckets >= Settings.pump_buckets else "no power (%d EU/t)" % roundi(Settings.pump_eu_t))]
	return res


func contents() -> Array:
	return [{"id": kind, "count": 1}]


func look_lines() -> Array[String]:
	return ["%d / %d buckets, %s" % [buckets, Settings.pump_buckets, "pumping" if wants_water() else ("full" if buckets >= Settings.pump_buckets else "no power")]]


## A squat iron drum with a bronze pipe down into the ground and a copper terminal on top.
func view_parts() -> Dictionary:
	var size := Vector3i(8, 8, 8)
	var c := WorkPalette.new_cells(size)
	WorkPalette.box(c, size, Vector3i(1, 2, 1), Vector3i(7, 7, 7), WorkPalette.IRON_DARK)
	WorkPalette.box(c, size, Vector3i(3, 0, 3), Vector3i(5, 2, 5), WorkPalette.BRONZE)   # the intake pipe
	WorkPalette.box(c, size, Vector3i(3, 7, 3), Vector3i(5, 8, 5), WorkPalette.COPPER)
	var h := clampi(int(round(float(buckets) / maxf(Settings.pump_buckets, 1.0) * 5.0)), 0, 5)
	if h > 0:
		WorkPalette.box(c, size, Vector3i(3, 2, 7), Vector3i(5, 2 + h, 8), WorkPalette.GLOW)   # the gauge
	return {"solid": {"cells": c, "size": size}}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "buckets": buckets}


func load_from(d: Dictionary) -> void:
	buckets = int(d.get("buckets", 0))


func summary() -> Dictionary:
	return {"kind": kind, "buckets": buckets, "drawn": drawn, "eu": eu}
