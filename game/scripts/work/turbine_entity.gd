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


var amps := 0.0    # delivered, last solve
var volts := 0.0


## On the LV network (ADR 0123) it is the dynamo: an EMF of dynamo_volts at working speed (less as it slows)
## behind dynamo_ohms, so a loaded dynamo sags and a slow one cannot hold the machines.
func net(channel: String) -> Dictionary:
	if channel != "energy" or rpm <= 0.0:
		return {}
	return {"kind": "source", "volts": Settings.dynamo_volts * clampf(rpm / Settings.turbine_rpm, 0.0, 1.0), "ohms": Settings.dynamo_ohms}


func net_in(channel: String, d) -> bool:
	if channel == "energy":
		amps = float(d.get("amps", 0.0))
		volts = float(d.get("volts", 0.0))
	return false


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
	return {"consume": 0, "give": [], "msg": "The turbine turns at %d rpm: %.1f V, %.1f A (%.0f W) on the cables that touch it." % [roundi(rpm), volts, amps, volts * amps] if rpm > 0.0 else "A bladed wheel waiting for steam: the dynamo; cables carry its power."}


func contents() -> Array:
	return [{"id": "steam_turbine", "count": 1}]


func look_lines() -> Array[String]:
	if boiler == null:
		return ["No steam: lay pipes from a boiler"]
	if boiler.pressure <= 0.0:
		return ["No pressure: the boiler is cold"]
	return ["Turning at %d rpm (%.2f bar): %.1f V, %.1f A, %.0f W on the cables" % [roundi(rpm), boiler.pressure, volts, amps, volts * amps]]


## 1.5 m on a side (SCALE 3), one cell per voxel (grid 12), big and industrial (ADR 0129): a cast-iron bed
## plate with a hazard-striped front edge, two steel saddles under the engine-green casing (a drum along x
## between riveted steel flanges) with a window in its front (+z) where the bladed wheel turns a step every
## five ticks, the copper generator on the +x end under a terminal box, the bronze steam inlet at the back
## with a yellow valve wheel, and a pressure gauge on top.
func view_parts() -> Dictionary:
	var n := 12
	var size := Vector3i(n, n, n)
	var c := WorkPalette.new_cells(size)
	var cy := 6.5
	var cz := 6.0
	WorkPalette.box(c, size, Vector3i(0, 0, 0), Vector3i(n, 1, n), WorkPalette.IRON_DARK)   # the bed plate
	WorkPalette.hazard_box(c, size, Vector3i(0, 0, n - 1), Vector3i(n, 1, n))
	for p in [Vector3i(0, 1, 0), Vector3i(n - 1, 1, 0), Vector3i(0, 1, n - 1), Vector3i(n - 1, 1, n - 1)]:
		c[WorkPalette.idx(size, p.x, p.y, p.z)] = WorkPalette.RIVET + 1   # its bolts
	for x in [2, 5]:   # the saddles
		WorkPalette.box(c, size, Vector3i(x, 1, 2), Vector3i(x + 2, 3, 10), WorkPalette.STEEL)
	WorkPalette.drum(c, size, 0, 1, cy, cz, 1.6, WorkPalette.STEEL)               # the bearing cap
	WorkPalette.drum(c, size, 2, 7, cy, cz, 4.5, WorkPalette.PAINT, 1.2)          # the casing, hollow
	for x in [1, 7]:
		WorkPalette.drum(c, size, x, x + 1, cy, cz, 5.0, WorkPalette.IRON_DARK)   # the flanges
		WorkPalette.drum_rivets(c, size, x, cy, cz, 5.0, 3)
	for z in size.z:   # the window in the casing's front
		for y in size.y:
			if z + 0.5 - cz > 1.5 and absf(y + 0.5 - cy) < 2.0:
				for x in range(3, 6):
					c[WorkPalette.idx(size, x, y, z)] = 0
	WorkPalette.drum(c, size, 1, 9, cy, cz, 1.0, WorkPalette.STEEL)               # the shaft and its hub
	var turn := deg_to_rad(22.5 * ((spin / 5) % 4))
	for x in range(2, 7):   # four blades, turned a step by the phase; every other row leans the other way
		for k in 4:
			var a := turn + k * PI / 2.0 + (0.2 if x % 2 == 0 else -0.2)
			for r in [1.5, 2.3, 3.1]:
				var y := int(floor(cy + r * sin(a)))
				var z := int(floor(cz + r * cos(a)))
				c[WorkPalette.idx(size, x, y, z)] = WorkPalette.TIN + 1
	WorkPalette.drum(c, size, 8, 9, cy, cz, 2.0, WorkPalette.IRON_DARK)           # the coupling
	WorkPalette.drum(c, size, 9, n, cy, cz, 3.6, WorkPalette.COPPER)              # the generator's windings
	WorkPalette.drum(c, size, 11, n, cy, cz, 3.7, WorkPalette.IRON_DARK, 1.0)     # its end band
	WorkPalette.drum(c, size, 11, n, cy, cz, 1.0, WorkPalette.STEEL)
	WorkPalette.box(c, size, Vector3i(9, 1, 3), Vector3i(n, 3, 9), WorkPalette.STEEL)     # its foot
	WorkPalette.box(c, size, Vector3i(9, 10, 4), Vector3i(n, 11, 8), WorkPalette.IRON_DARK)   # the terminal box
	for z in [5, 6]:
		c[WorkPalette.idx(size, 10, 11, z)] = WorkPalette.COPPER + 1             # its terminals
	WorkPalette.box(c, size, Vector3i(3, 6, 0), Vector3i(6, 9, 3), WorkPalette.BRONZE)    # the steam inlet
	WorkPalette.box(c, size, Vector3i(2, 5, 0), Vector3i(7, 10, 1), WorkPalette.STEEL)    # its flange
	for p in [Vector3i(2, 5, 0), Vector3i(6, 5, 0), Vector3i(2, 9, 0), Vector3i(6, 9, 0)]:
		c[WorkPalette.idx(size, p.x, p.y, p.z)] = WorkPalette.RIVET + 1
	WorkPalette.box(c, size, Vector3i(4, 9, 1), Vector3i(5, 10, 2), WorkPalette.STEEL)    # the valve stem
	WorkPalette.box(c, size, Vector3i(3, 10, 0), Vector3i(6, 11, 3), WorkPalette.HAZARD)  # its wheel
	c[WorkPalette.idx(size, 4, 10, 1)] = WorkPalette.STEEL + 1
	WorkPalette.box(c, size, Vector3i(3, 11, 5), Vector3i(6, 12, 8), WorkPalette.LINEN)   # the gauge face
	c[WorkPalette.idx(size, 4 + (spin / 5) % 2 if rpm > 0.0 else 3, 11, 6)] = WorkPalette.SOOT + 1   # its needle
	return {"solid": {"cells": c, "size": size, "grid": n}}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "spin": spin}


func load_from(d: Dictionary) -> void:
	spin = int(d.get("spin", 0))


func summary() -> Dictionary:
	return {"kind": kind, "rpm": roundi(rpm), "powered": powered()}
