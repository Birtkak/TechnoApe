class_name PumpEntity
extends RefCounted

## An electric pump (ADR 0117, 0123): on its `load.watts` it draws a bucket (1 m^3, 512 voxels) a second from the water
## under it into its tank, the way a bucket scoops (Bucket.scoop_cells over the bubble: the only edit API
## it uses), and pushes buckets into a water tank beside it. A bucket in hand takes one. While it is
## powered with room in its tank it needs the material simulation (needs_sim), so MachinePins keeps a
## pin alive around it when the player is away.

signal changed

var kind := "pump"
var pos: Vector3i
var buckets := 0
var load: PowerLoad
var power_ticks := 0
var drawn := 0        # buckets ever drawn, for debug JSON


func _init(p_items: ItemRegistry, p_pos: Vector3i) -> void:
	pos = p_pos
	load = PowerLoad.new(float(p_items.get_def(kind).get("load", {}).get("watts", 200.0)))


func powered() -> bool:
	return power_ticks > 0


func wants_water() -> bool:
	return powered() and buckets < Settings.pump_buckets


func needs_sim() -> bool:
	return wants_water()


var _acc := 0.0   # fractional buckets the water channel took


func net(channel: String) -> Dictionary:
	if channel == "energy":
		return load.net(buckets < Settings.pump_buckets)
	if channel == "water":   # ADR 0122: tanks and boilers on the pipes that touch it drink from it
		return {"role": NetworkSolver.SOURCE, "amount": minf(Settings.pipe_water_cap, float(buckets))}
	return {}


func net_in(channel: String, amount) -> bool:
	if channel == "energy":
		var started := load.net_in(amount)
		power_ticks = 21 if load.on else 0
		return started
	if channel == "water":
		_acc += amount
		while _acc >= 1.0 - 1e-6 and buckets > 0:
			_acc -= 1.0
			buckets -= 1
			changed.emit()
	return false


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
		"pumping" if wants_water() else ("full" if buckets >= Settings.pump_buckets else "no power: " + load.status())]
	return res


func contents() -> Array:
	return [{"id": kind, "count": 1}]


func look_lines() -> Array[String]:
	return ["%d / %d buckets, %s" % [buckets, Settings.pump_buckets, "pumping" if wants_water() else ("full" if buckets >= Settings.pump_buckets else "no power")]]


## A squat iron drum with a bronze pipe down into the ground and a copper terminal on top.
## The industrial pump (ADR 0129): a steel casing on a hazard-edged bed raised on feet over the bronze
## intake, an engine-green motor drum on its back, the copper outlet up front and a level gauge.
func view_parts() -> Dictionary:
	var n := 16
	var size := Vector3i(n, n, n)
	var c := WorkPalette.new_cells(size)
	for q in [Vector2i(1, 1), Vector2i(13, 1), Vector2i(1, 13), Vector2i(13, 13)]:
		WorkPalette.box(c, size, Vector3i(q.x, 0, q.y), Vector3i(q.x + 2, 2, q.y + 2), WorkPalette.IRON_DARK)   # the feet
	WorkPalette.box(c, size, Vector3i(6, 0, 6), Vector3i(10, 4, 10), WorkPalette.BRONZE)   # the intake pipe
	WorkPalette.box(c, size, Vector3i(5, 1, 5), Vector3i(11, 2, 11), WorkPalette.IRON_DARK)   # its flange
	WorkPalette.box(c, size, Vector3i(0, 2, 0), Vector3i(n, 4, n), WorkPalette.IRON_DARK)   # the bed
	WorkPalette.hazard_box(c, size, Vector3i(0, 3, 0), Vector3i(n, 4, n))
	WorkPalette.box(c, size, Vector3i(2, 4, 2), Vector3i(14, 11, 14), WorkPalette.STEEL)   # the casing
	for y in [4, 10]:
		WorkPalette.box(c, size, Vector3i(2, y, 2), Vector3i(14, y + 1, 14), WorkPalette.IRON_DARK)
	for x in [2, 13]:
		for z in [2, 13]:
			WorkPalette.box(c, size, Vector3i(x, 4, z), Vector3i(x + 1, 11, z + 1), WorkPalette.IRON_DARK)
	for x in range(3, 13, 3):
		c[WorkPalette.idx(size, x, 10, 13)] = WorkPalette.RIVET + 1
	WorkPalette.drum(c, size, 3, 13, 13.5, 5.5, 2.6, WorkPalette.PAINT)   # the motor
	for x in [3, 12]:
		WorkPalette.drum(c, size, x, x + 1, 13.5, 5.5, 2.6, WorkPalette.IRON_DARK)
	WorkPalette.box(c, size, Vector3i(6, 11, 3), Vector3i(10, 12, 8), WorkPalette.IRON_DARK)   # its saddle
	WorkPalette.box(c, size, Vector3i(7, 11, 9), Vector3i(10, n, 12), WorkPalette.COPPER)   # the outlet
	WorkPalette.box(c, size, Vector3i(6, 13, 8), Vector3i(11, 14, 13), WorkPalette.IRON_DARK)
	WorkPalette.box(c, size, Vector3i(5, 5, 13), Vector3i(11, 10, 14), WorkPalette.IRON_DARK)   # the gauge frame
	WorkPalette.box(c, size, Vector3i(6, 6, 13), Vector3i(10, 9, 14), WorkPalette.SOOT)
	var h := clampi(int(round(float(buckets) / maxf(Settings.pump_buckets, 1.0) * 3.0)), 0, 3)
	if h > 0:
		WorkPalette.box(c, size, Vector3i(6, 6, 13), Vector3i(10, 6 + h, 14), WorkPalette.GLOW)
	return {"solid": {"cells": c, "size": size, "grid": n}}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "buckets": buckets}


func load_from(d: Dictionary) -> void:
	buckets = int(d.get("buckets", 0))


func summary() -> Dictionary:
	return {"kind": kind, "buckets": buckets, "drawn": drawn, "watts": load.drawn}
