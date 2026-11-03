class_name BatteryBoxEntity
extends RefCounted

## A battery box (ADR 0117, 0123): lead-acid cells holding `battery.wh` watt-hours. On the LV network it is a
## source like any other: an EMF between battery_volts_empty and battery_volts_full by its state of charge,
## behind battery_ohms. Physics does the rest: with the dynamo above it the current runs in and it charges;
## with the dynamo gone or sagging it supplies. Empty it leaves the circuit, full it stops taking.

signal changed

var kind := "battery_box"
var pos: Vector3i
var wh := 2000.0
var charge := 0.0       # watt-hours
var amps := 0.0         # last solve: positive giving, negative taking
var volts := 0.0


func _init(p_items: ItemRegistry, p_pos: Vector3i) -> void:
	pos = p_pos
	wh = float(p_items.get_def(kind).get("battery", {}).get("wh", 2000.0))


func frac() -> float:
	return charge / maxf(wh, 1.0)


func emf() -> float:
	return lerpf(Settings.battery_volts_empty, Settings.battery_volts_full, frac())


func net(channel: String) -> Dictionary:
	if channel != "energy":
		return {}
	if charge <= 0.0 and volts <= emf():
		return {"kind": "load", "watts": 0.01, "volts": Settings.lv_volts}   # drained: it only reads the line until something pushes it above its EMF
	return {"kind": "source", "volts": emf(), "ohms": Settings.battery_ohms}


## A second of the solved current: out of or into the cells.
func net_in(channel: String, d) -> bool:
	if channel != "energy":
		return false
	amps = float(d.get("amps", 0.0))
	volts = float(d.get("volts", 0.0))
	var p := volts * amps   # positive: giving
	if p > 0.0:
		charge = maxf(0.0, charge - p / 3600.0)
	elif charge < wh:
		charge = minf(wh, charge - p / 3600.0)
	else:
		amps = 0.0   # full: it takes nothing (the solver still drives its node, the loss is a rounding error)
	changed.emit()
	return false


func tick() -> bool:
	return false


func is_lit() -> bool:
	return false


func use(_stack: Dictionary) -> Dictionary:
	return {"consume": 0, "give": [], "msg": "Charge %d%% (%d of %d Wh), %.1f V, %s." % [roundi(frac() * 100.0), roundi(charge), roundi(wh), volts,
		("giving %.1f A" % amps) if amps > 0.0 else ("taking %.1f A" % -amps if amps < 0.0 else "resting")]}


func contents() -> Array:
	return [{"id": kind, "count": 1}]


func look_lines() -> Array[String]:
	return ["Charge %d%% (%d Wh), %s" % [roundi(frac() * 100.0), roundi(charge), ("giving %.1f A" % amps) if amps > 0.0 else ("charging at %.1f A" % -amps if amps < 0.0 else "resting")]]


## A steel case in an iron frame (ADR 0129) with copper terminals on top and a charge gauge up its front.
func view_parts() -> Dictionary:
	var size := Vector3i(8, 8, 8)
	var c := WorkPalette.new_cells(size)
	WorkPalette.box(c, size, Vector3i(0, 0, 0), Vector3i(8, 7, 8), WorkPalette.STEEL)
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
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "charge": charge}


func load_from(d: Dictionary) -> void:
	charge = float(d.get("charge", 0.0))


func summary() -> Dictionary:
	return {"kind": kind, "charge": roundi(charge), "amps": amps, "volts": volts}
