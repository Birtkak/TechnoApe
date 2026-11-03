class_name CableEntity
extends RefCounted

## A copper cable (ADR 0113, 0123): one block of conductor on the LV network, laid on the ground like a pipe.
## Its item says its resistance per block (copper, 6 mm^2: 1.4 milliohm a block) and its rating in amps;
## carried past the rating it heats, and after cable_burn_s of that it burns out (no conductor until a cable
## in hand replaces it). The solver joins it to whatever touches it face to face.

signal changed

var kind := "cable"
var item := "copper_cable"
var pos: Vector3i
var axis := 0   # 0 along x, 1 along z (the way the player faced when laying it)
var amps := 0.0      # through it, last solve
var volts := 0.0
var hot := 0         # seconds over its rating
var burnt := false
var _items: ItemRegistry


func _init(p_items: ItemRegistry, p_pos: Vector3i) -> void:
	_items = p_items
	pos = p_pos


func spec() -> Dictionary:
	return _items.get_def(item).get("cable", {"ohms_per_block": 0.0014, "amps": 40})


func net(channel: String) -> Dictionary:
	if channel != "energy" or burnt:
		return {}
	return {"kind": "conductor", "ohms": float(spec().get("ohms_per_block", 0.0014)), "amps": float(spec().get("amps", 40))}


func net_in(channel: String, d) -> bool:
	if channel != "energy":
		return false
	amps = float(d.get("amps", 0.0))
	volts = float(d.get("volts", 0.0))
	if amps > float(spec().get("amps", 40)):
		hot += 1
		if hot >= Settings.cable_burn_s:
			burnt = true
			changed.emit()
	elif hot > 0:
		hot -= 1
	return false


## Right click: a cable in hand replaces a burnt one.
func use(stack: Dictionary) -> Dictionary:
	var res := {"consume": 0, "give": [], "msg": ""}
	if burnt and stack.get("id", "") == item:
		burnt = false
		hot = 0
		res["consume"] = 1
		res["msg"] = "A new length of cable."
		changed.emit()
	else:
		res["msg"] = look_lines()[0]
	return res


func contents() -> Array:
	return [{"id": item, "count": 1}]


func tick() -> bool:
	return false


func is_lit() -> bool:
	return false


func look_lines() -> Array[String]:
	if burnt:
		return ["Burnt out: replace it with a cable in hand"]
	var line := "%.1f V, %.1f A of %d A" % [volts, amps, int(spec().get("amps", 40))]
	if hot > 0:
		line += ", overloaded %d s" % hot
	return [line]


## A thin copper line in a dark sleeve along the cable's axis, flat on the ground; black when burnt.
func view_parts() -> Dictionary:
	var size := Vector3i(8, 2, 8)
	var c := WorkPalette.new_cells(size)
	var a := Vector3i(0, 0, 3)
	var b := Vector3i(8, 2, 5)
	if axis == 1:
		a = Vector3i(a.z, a.y, a.x)
		b = Vector3i(b.z, b.y, b.x)
	WorkPalette.box(c, size, a, b, WorkPalette.SOOT)
	if not burnt:
		WorkPalette.box(c, size, a + Vector3i(0, 1, 1) if axis == 0 else a + Vector3i(1, 1, 0), b - (Vector3i(0, 0, 1) if axis == 0 else Vector3i(1, 0, 0)), WorkPalette.EMBER if hot > 0 else WorkPalette.COPPER)
	return {"solid": {"cells": c, "size": size}}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "item": item, "axis": axis, "burnt": burnt}


func load_from(d: Dictionary) -> void:
	item = String(d.get("item", item))
	axis = int(d.get("axis", 0))
	burnt = bool(d.get("burnt", false))


func summary() -> Dictionary:
	return {"kind": kind, "item": item, "amps": amps, "burnt": burnt}
