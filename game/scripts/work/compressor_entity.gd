class_name CompressorEntity
extends RefCounted

## The air compressor (ADR 0099): a steam cylinder pumping a riveted air tank. On a boiler's pipes it
## draws steam_per_use a second and fills its tank (`air`, 0..1) in compressor_fill_s at working
## pressure, faster above. A right click with a jetpack in hand moves the air into the jetpack's
## `air` (the charge it flies on); anything else gets told what it is for.

signal changed

var kind := "compressor"
var pos: Vector3i
var boiler: BoilerEntity = null
var air := 0.0
var _draw := 0
var _gauge := 0


func _init(p_pos: Vector3i) -> void:
	pos = p_pos


func powered() -> bool:
	return boiler != null and boiler.pressure > 0.0


func is_lit() -> bool:
	return powered() and air < 1.0   # pumping: BlockEntities.tick keeps the view fresh


func tick() -> bool:
	if not powered() or air >= 1.0:
		return false
	_draw = (_draw + 1) % 20
	if _draw == 0 and not boiler.draw(Settings.steam_per_use):
		return false
	air = minf(1.0, air + BlockEntities.TICK_S / Settings.compressor_fill_s * clampf(boiler.pressure, 0.25, 2.0))
	_gauge = (_gauge + 1) % 20
	if _gauge == 0:
		changed.emit()   # the gauge and the piston move
	return false


## Right click with the held `stack` (changed in place when it is a jetpack). Returns {consume, give, msg}.
func use(stack: Dictionary) -> Dictionary:
	var res := {"consume": 0, "give": [], "msg": ""}
	if stack.get("id", "") != "jetpack":
		res["msg"] = "Steam in, compressed air out: right click it with a jetpack to fill the pack."
		return res
	if air <= 0.0:
		res["msg"] = "The air tank is empty: it needs steam to pump."
		return res
	var have := float(stack.get("air", 0.0))
	var take := minf(air, 1.0 - have)
	if take <= 0.0:
		res["msg"] = "The jetpack is full."
		return res
	stack["air"] = have + take
	air -= take
	changed.emit()
	res["msg"] = "The jetpack is charged to %d%%." % roundi((have + take) * 100.0)
	return res


func contents() -> Array:
	return [{"id": "compressor", "count": 1}]


func look_lines() -> Array[String]:
	var out: Array[String] = ["Air: %d%% (right click with a jetpack)" % roundi(air * 100.0)]
	if boiler == null:
		out.append("No steam: lay pipes from a boiler")
	elif boiler.pressure <= 0.0:
		out.append("No pressure: the boiler is cold")
	else:
		out.append("Pumping at %.2f bar" % boiler.pressure)
	return out


## A riveted air tank lying on an iron base, the steam cylinder standing at its back with the piston
## rod pumping, a gauge on the front that fills with the air, the hose coupling on the side.
func view_parts() -> Dictionary:
	var size := Vector3i(8, 8, 8)
	var c := WorkPalette.new_cells(size)
	WorkPalette.box(c, size, Vector3i(0, 0, 0), Vector3i(8, 1, 8), WorkPalette.IRON_DARK)   # the base
	for y in range(1, 6):   # the tank along x
		for z in range(2, 8):
			var d2 := (y - 3.0) * (y - 3.0) + (z - 4.5) * (z - 4.5)
			if d2 <= 6.5:
				for x in range(0, 8):
					c[WorkPalette.idx(size, x, y, z)] = (WorkPalette.ANVIL if x % 3 == 1 else WorkPalette.IRON_DARK) + 1
	var up := 1 if _gauge < 10 and powered() and air < 1.0 else 0
	WorkPalette.box(c, size, Vector3i(5, 1, 0), Vector3i(7, 5, 2), WorkPalette.BRONZE)              # the steam cylinder
	WorkPalette.box(c, size, Vector3i(6, 5, 1), Vector3i(7, 7 + up, 2), WorkPalette.ANVIL)           # the piston rod
	WorkPalette.box(c, size, Vector3i(3, 0, 0), Vector3i(5, 1, 2), WorkPalette.BRONZE)              # the steam inlet
	WorkPalette.box(c, size, Vector3i(0, 3, 1), Vector3i(1, 5, 3), WorkPalette.COPPER)              # the hose coupling
	var fill := int(round(air * 3.0))
	WorkPalette.box(c, size, Vector3i(2, 5, 3), Vector3i(5, 6, 6), WorkPalette.STONE_DARK)          # the gauge
	if fill > 0:
		WorkPalette.box(c, size, Vector3i(2, 5, 3), Vector3i(2 + fill, 6, 6), WorkPalette.GLOW if air >= 1.0 else WorkPalette.COPPER)
	return {"solid": {"cells": c, "size": size}}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "air": air}


func load_from(d: Dictionary) -> void:
	air = float(d.get("air", 0.0))


func summary() -> Dictionary:
	return {"kind": kind, "air": snappedf(air, 0.01), "powered": powered()}
