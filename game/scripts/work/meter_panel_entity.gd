class_name MeterPanelEntity
extends RefCounted

## A meter panel (ADR 0123): a block of conductor with a voltmeter and an ammeter on its face. Every solve it
## records the volts at it and the amps through it and redraws: the left bar is 0..60 V, the right 0..60 A.

signal changed

var kind := "meter_panel"
var pos: Vector3i
var volts := 0.0
var amps := 0.0


func _init(p_pos: Vector3i) -> void:
	pos = p_pos


func net(channel: String) -> Dictionary:
	return {"kind": "conductor", "ohms": Settings.circuit_device_ohms} if channel == "energy" else {}


func net_in(channel: String, d) -> bool:
	if channel != "energy":
		return false
	var v := float(d.get("volts", 0.0))
	var a := float(d.get("amps", 0.0))
	if absf(v - volts) > 0.5 or absf(a - amps) > 0.5:
		changed.emit()
	volts = v
	amps = a
	return false


func tick() -> bool:
	return false


func is_lit() -> bool:
	return false


func use(_stack: Dictionary) -> Dictionary:
	return {"consume": 0, "give": [], "msg": look_lines()[0]}


func contents() -> Array:
	return [{"id": kind, "count": 1}]


func look_lines() -> Array[String]:
	return ["%.1f V, %.1f A (%.0f W through it)" % [volts, amps, volts * amps]]


## An iron box; its front face holds two bars under glass: volts in glow, amps in copper.
func view_parts() -> Dictionary:
	var size := Vector3i(8, 8, 8)
	var c := WorkPalette.new_cells(size)
	WorkPalette.box(c, size, Vector3i(0, 0, 0), Vector3i(8, 8, 8), WorkPalette.IRON_DARK)
	WorkPalette.box(c, size, Vector3i(1, 1, 7), Vector3i(7, 7, 8), WorkPalette.SOOT)   # the dial face
	var hv := clampi(int(round(volts / 60.0 * 6.0)), 0, 6)
	var ha := clampi(int(round(absf(amps) / 60.0 * 6.0)), 0, 6)
	if hv > 0:
		WorkPalette.box(c, size, Vector3i(2, 1, 7), Vector3i(3, 1 + hv, 8), WorkPalette.GLOW)
	if ha > 0:
		WorkPalette.box(c, size, Vector3i(5, 1, 7), Vector3i(6, 1 + ha, 8), WorkPalette.COPPER)
	return {"solid": {"cells": c, "size": size}}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z]}


func load_from(_d: Dictionary) -> void:
	pass


func summary() -> Dictionary:
	return {"kind": kind, "volts": volts, "amps": amps}
