class_name TurbineEntity
extends RefCounted

## The steam turbine (ADR 0099): the end of the steam age. A bladed wheel in an iron housing with a
## coil of copper wire round its shaft; on a boiler's pipes it draws steam_per_use a second and
## spins at turbine_rpm times the pressure. Nothing takes its power yet: it is the first generator,
## the door to the electrical age, and says so.

signal changed

var kind := "turbine"
var pos: Vector3i
var boiler: BoilerEntity = null
var rpm := 0.0
var spin := 0   # ticks turned, for the blades
var _draw := 0


func _init(p_pos: Vector3i) -> void:
	pos = p_pos


func powered() -> bool:
	return boiler != null and boiler.pressure > 0.0


func is_lit() -> bool:
	return rpm > 0.0   # BlockEntities.tick redraws "lit" pieces: the wheel turns


func tick() -> bool:
	if not powered():
		if rpm > 0.0:
			rpm = 0.0
			changed.emit()
		return false
	_draw = (_draw + 1) % 20
	if _draw == 0 and not boiler.draw(Settings.steam_per_use):
		rpm = 0.0
		return false
	rpm = Settings.turbine_rpm * boiler.pressure
	spin += 1
	if spin % 5 == 0:
		changed.emit()   # the wheel turns a step
	return false


func use(_held: Dictionary) -> Dictionary:
	return {"consume": 0, "give": [], "msg": "The turbine turns at %d rpm. A dynamo on its shaft would make this the electrical age." % roundi(rpm) if rpm > 0.0 else "A bladed wheel waiting for steam: the first generator, with nothing yet to power."}


func contents() -> Array:
	return [{"id": "steam_turbine", "count": 1}]


func look_lines() -> Array[String]:
	if boiler == null:
		return ["No steam: lay pipes from a boiler"]
	if boiler.pressure <= 0.0:
		return ["No pressure: the boiler is cold"]
	return ["Turning at %d rpm (%.2f bar): nothing to power yet" % [roundi(rpm), boiler.pressure]]


## An iron housing on a base with the wheel's blades showing through its open front (+z), turning a
## step every five ticks; the bronze inlet at the back, the copper coil on the shaft's side.
func view_parts() -> Dictionary:
	var size := Vector3i(8, 8, 8)
	var c := WorkPalette.new_cells(size)
	WorkPalette.box(c, size, Vector3i(0, 0, 0), Vector3i(8, 1, 8), WorkPalette.IRON_DARK)   # the base
	for y in range(1, 8):   # a drum lying along x, open at the front
		for z in range(1, 8):
			var d2 := (y - 4.0) * (y - 4.0) + (z - 4.0) * (z - 4.0)
			if d2 <= 9.5 and d2 > 4.0:
				for x in range(1, 7):
					c[WorkPalette.idx(size, x, y, z)] = WorkPalette.IRON_DARK + 1
	var phase := (spin / 5) % 4
	for x in range(2, 6):   # four blades through the hub, turned by the phase
		c[WorkPalette.idx(size, x, 4, 4)] = WorkPalette.ANVIL + 1
		for r in [1, 2]:
			var a: Vector2i = [Vector2i(r, 0), Vector2i(r, r), Vector2i(0, r), Vector2i(-r, r)][phase]
			for s in [1, -1]:
				c[WorkPalette.idx(size, x, 4 + a.x * s, 4 + a.y * s)] = WorkPalette.ANVIL + 1
	WorkPalette.box(c, size, Vector3i(0, 3, 3), Vector3i(1, 5, 5), WorkPalette.COPPER)       # the coil on the shaft end
	WorkPalette.box(c, size, Vector3i(7, 3, 3), Vector3i(8, 5, 5), WorkPalette.IRON_DARK)    # the bearing
	WorkPalette.box(c, size, Vector3i(3, 0, 7), Vector3i(5, 3, 8), WorkPalette.BRONZE)       # the steam inlet
	return {"solid": {"cells": c, "size": size}}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "spin": spin}


func load_from(d: Dictionary) -> void:
	spin = int(d.get("spin", 0))


func summary() -> Dictionary:
	return {"kind": kind, "rpm": roundi(rpm), "powered": powered()}
