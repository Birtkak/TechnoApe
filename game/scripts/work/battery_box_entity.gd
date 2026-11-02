class_name BatteryBoxEntity
extends RefCounted

## A battery box (ADR 0117): stores Settings.battery_eu. On the energy network it is a low sink while it
## charges (it takes only what the machines leave, NetworkSolver.SINK_LOW) and a source while it gives.
## It flips on its own: charging and given nothing while it holds charge, it offers next solve; giving and
## asked for nothing, it goes back to charging. One solve (a second) of lag either way: default, developer
## to confirm.

signal changed

var kind := "battery_box"
var pos: Vector3i
var charge := 0.0
var giving := false
var last := 0.0   # what the last solve moved, EU/t


func _init(p_pos: Vector3i) -> void:
	pos = p_pos


func net(channel: String) -> Dictionary:
	if channel != "energy":
		return {}
	if giving and charge > 0.0:
		return {"role": NetworkSolver.SOURCE, "amount": minf(Settings.battery_eu_t, charge / 20.0)}
	return {"role": NetworkSolver.SINK_LOW, "amount": minf(Settings.battery_eu_t, (Settings.battery_eu - charge) / 20.0)}


## `amount` is per tick; a solve covers the next 20 ticks.
func net_in(channel: String, amount: float) -> void:
	if channel != "energy":
		return
	last = amount
	if giving:
		charge = maxf(0.0, charge - amount * 20.0)
		if amount <= 0.0 or charge <= 0.0:
			giving = false
	else:
		charge = minf(Settings.battery_eu, charge + amount * 20.0)
		if amount <= 0.0 and charge > 0.0:
			giving = true
	changed.emit()


func frac() -> float:
	return charge / maxf(Settings.battery_eu, 1.0)


func tick() -> bool:
	return false


func is_lit() -> bool:
	return false


func use(_stack: Dictionary) -> Dictionary:
	return {"consume": 0, "give": [], "msg": "Charge %d%% (%d / %d EU), %s." % [roundi(frac() * 100.0), roundi(charge), roundi(Settings.battery_eu),
		"giving %d EU/t" % roundi(last) if giving else "taking %d EU/t" % roundi(last)]}


func contents() -> Array:
	return [{"id": kind, "count": 1}]


func look_lines() -> Array[String]:
	return ["Charge %d%%, %s" % [roundi(frac() * 100.0), ("giving %d EU/t" % roundi(last)) if giving else ("charging at %d EU/t" % roundi(last))]]


## A tin box with copper terminals on top and a charge gauge up its front.
func view_parts() -> Dictionary:
	var size := Vector3i(8, 8, 8)
	var c := WorkPalette.new_cells(size)
	WorkPalette.box(c, size, Vector3i(0, 0, 0), Vector3i(8, 7, 8), WorkPalette.STEEL)   # a steel case in an iron frame (ADR 0129)
	for q in [Vector2i(0, 0), Vector2i(7, 0), Vector2i(0, 7), Vector2i(7, 7)]:
		WorkPalette.box(c, size, Vector3i(q.x, 0, q.y), Vector3i(q.x + 1, 7, q.y + 1), WorkPalette.IRON_DARK)
	WorkPalette.box(c, size, Vector3i(0, 6, 0), Vector3i(8, 7, 8), WorkPalette.IRON_DARK)
	WorkPalette.box(c, size, Vector3i(1, 6, 1), Vector3i(7, 7, 7), WorkPalette.STEEL)
	WorkPalette.hazard_box(c, size, Vector3i(1, 5, 7), Vector3i(7, 6, 8))
	WorkPalette.box(c, size, Vector3i(1, 7, 1), Vector3i(3, 8, 3), WorkPalette.COPPER)
	WorkPalette.box(c, size, Vector3i(5, 7, 5), Vector3i(7, 8, 7), WorkPalette.COPPER)
	var h := clampi(int(round(frac() * 6.0)), 0, 6)
	WorkPalette.box(c, size, Vector3i(3, 0, 7), Vector3i(5, 7, 8), WorkPalette.SOOT)
	if h > 0:
		WorkPalette.box(c, size, Vector3i(3, 0, 7), Vector3i(5, h, 8), WorkPalette.GLOW)
	return {"solid": {"cells": c, "size": size}}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "charge": charge, "giving": giving}


func load_from(d: Dictionary) -> void:
	charge = float(d.get("charge", 0.0))
	giving = bool(d.get("giving", false))


func summary() -> Dictionary:
	return {"kind": kind, "charge": roundi(charge), "giving": giving, "last": last}
