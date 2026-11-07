class_name VentFanEntity
extends RefCounted

## A vent fan (ADR 0148): a 0.5 m LV machine (`load.watts` on the cables like the lamp) that pushes every gas
## cell in the duct in front of it (its own cross-section, Settings.vent_fan_reach_m long) one cell along its
## facing every frame (SimBubble.blow, native), so a mine's firedamp can be driven out of a gallery. A wrench
## turns it; it needs the simulation where it stands (machine pins) while it runs.

signal changed

const DIRS := [Vector3i(0, 0, 1), Vector3i(1, 0, 0), Vector3i(0, 0, -1), Vector3i(-1, 0, 0)]

var kind := "vent_fan"
var pos: Vector3i
var on := false
var facing := 0
var load: PowerLoad
var moved := 0   # gas cells pushed so far (stats)


func _init(p_items: ItemRegistry, p_pos: Vector3i) -> void:
	pos = p_pos
	load = PowerLoad.new(float(p_items.get_def(kind).get("load", {}).get("watts", 100.0)))


func net(channel: String) -> Dictionary:
	return load.net(true) if channel == "energy" else {}


func net_in(channel: String, d) -> bool:
	if channel != "energy":
		return false
	var started := load.net_in(d)
	if load.on != on:
		on = load.on
		changed.emit()
	return started


func tick() -> bool:
	return false


func is_lit() -> bool:
	return false


func needs_sim() -> bool:
	return on


func dir() -> Vector3i:
	return DIRS[facing % 4]


## The duct it blows through: [min, max) in world voxels and the push direction.
func blow_box() -> Array:
	var b := Settings.BLOCK
	var reach := int(Settings.m2v(Settings.vent_fan_reach_m))
	var d := dir()
	var lo := pos
	var hi := pos + Vector3i(b, b, b)
	for a in 3:
		if d[a] > 0:
			lo[a] = pos[a] + b
			hi[a] = pos[a] + b + reach
		elif d[a] < 0:
			lo[a] = pos[a] - reach
			hi[a] = pos[a]
	return [lo, hi, d]


func use(_stack: Dictionary) -> Dictionary:
	return {"consume": 0, "give": [], "msg": "The fan is %s (%s)." % ["running" if on else "still", load.status()]}


func contents() -> Array:
	return [{"id": kind, "count": 1}]


func look_lines() -> Array[String]:
	return [("Running, " if on else "Still, ") + load.status()]


## A steel box with a dark ring (the blades) on the face it blows out of.
func view_parts() -> Dictionary:
	var n := 8
	var size := Vector3i(n, n, n)
	var c := WorkPalette.new_cells(size)
	WorkPalette.box(c, size, Vector3i(0, 0, 0), Vector3i(n, n, n), WorkPalette.STEEL)
	WorkPalette.box(c, size, Vector3i(1, 1, n - 1), Vector3i(n - 1, n - 1, n), WorkPalette.IRON_DARK)   # the outlet (+z before turning)
	WorkPalette.box(c, size, Vector3i(3, 3, n - 1), Vector3i(5, 5, n), WorkPalette.GLOW if on else WorkPalette.TIN)   # the hub
	return MachineEntity.turned({"solid": {"cells": c, "size": size, "grid": n}}, facing)


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "facing": facing}


func load_from(d: Dictionary) -> void:
	facing = int(d.get("facing", 0))


func summary() -> Dictionary:
	return {"kind": kind, "on": on, "facing": facing, "moved": moved}
