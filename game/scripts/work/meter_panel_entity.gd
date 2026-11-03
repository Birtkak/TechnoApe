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


## ADR 0129: a riveted iron case with a steel bezel round a cream face and two gauges read from across a room:
## the voltmeter on the left, the ammeter on the right, each a dark channel with ticks beside it that fills from
## the bottom, every cell of the fill in its zone's colour (green, amber, red at the top), under a coloured cap
## (glow = volts, copper = amps). Full scale is 60 V and 60 A.
func view_parts() -> Dictionary:
	var n := 16
	var size := Vector3i(n, n, n)
	var c := WorkPalette.new_cells(size)
	WorkPalette.box(c, size, Vector3i(0, 0, 0), Vector3i(n, n, n), WorkPalette.IRON_DARK)
	WorkPalette.box(c, size, Vector3i(1, 1, n - 1), Vector3i(n - 1, n - 1, n), WorkPalette.STEEL)   # the bezel
	WorkPalette.box(c, size, Vector3i(2, 2, n - 1), Vector3i(n - 2, n - 2, n), WorkPalette.LINEN)   # the face
	for p in [Vector2i(1, 1), Vector2i(n - 2, 1), Vector2i(1, n - 2), Vector2i(n - 2, n - 2)]:
		c[WorkPalette.idx(size, p.x, p.y, n - 1)] = WorkPalette.RIVET + 1
	_gauge(c, size, 3, absf(volts) / 60.0, WorkPalette.GLOW)
	_gauge(c, size, 9, absf(amps) / 60.0, WorkPalette.COPPER)
	return {"solid": {"cells": c, "size": size, "grid": n}}


## One gauge on the face, its channel at x0..x0+3: filled to `f` of its ten cells, ticks every second cell.
static func _gauge(c: PackedInt32Array, size: Vector3i, x0: int, f: float, cap: int) -> void:
	var z := size.z - 1
	var h := clampi(int(round(f * 10.0)), 0, 10)
	for y in range(3, 13):
		var k := y - 3
		var paint := WorkPalette.SOOT
		if k < h:
			paint = WorkPalette.PAINT if k < 6 else (WorkPalette.HAZARD if k < 8 else WorkPalette.EMBER)
		WorkPalette.box(c, size, Vector3i(x0, y, z), Vector3i(x0 + 3, y + 1, z + 1), paint)
		if k % 2 == 0:
			c[WorkPalette.idx(size, x0 + 3, y, z)] = WorkPalette.SOOT + 1   # a tick
	WorkPalette.box(c, size, Vector3i(x0, 13, z), Vector3i(x0 + 3, 14, z + 1), cap)


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z]}


func load_from(_d: Dictionary) -> void:
	pass


func summary() -> Dictionary:
	return {"kind": kind, "volts": volts, "amps": amps}
