class_name CableEntity
extends RefCounted

## A copper cable (ADR 0113): one block of conductor on the energy network, laid on the ground like a pipe.
## It holds nothing; the solver joins it to whatever touches it face to face.

signal changed

var kind := "cable"
var item := "copper_cable"
var pos: Vector3i
var axis := 0   # 0 along x, 1 along z (the way the player faced when laying it)


func _init(p_pos: Vector3i) -> void:
	pos = p_pos


func net(channel: String) -> Dictionary:
	return {"role": NetworkSolver.CONDUCTOR, "amount": Settings.cable_eu_t} if channel == "energy" else {}


func net_in(_channel: String, _amount: float) -> void:
	pass


func contents() -> Array:
	return [{"id": item, "count": 1}]


func tick() -> bool:
	return false


func is_lit() -> bool:
	return false


func look_lines() -> Array[String]:
	return ["Carries %d EU/t between the machines it touches" % Settings.cable_eu_t]


## A thin copper line in a dark sleeve along the cable's axis, flat on the ground.
func view_parts() -> Dictionary:
	var size := Vector3i(8, 2, 8)
	var c := WorkPalette.new_cells(size)
	var a := Vector3i(0, 0, 3)
	var b := Vector3i(8, 2, 5)
	if axis == 1:
		a = Vector3i(a.z, a.y, a.x)
		b = Vector3i(b.z, b.y, b.x)
	WorkPalette.box(c, size, a, b, WorkPalette.SOOT)
	WorkPalette.box(c, size, a + Vector3i(0, 1, 1) if axis == 0 else a + Vector3i(1, 1, 0), b - (Vector3i(0, 0, 1) if axis == 0 else Vector3i(1, 0, 0)), WorkPalette.COPPER)
	return {"solid": {"cells": c, "size": size}}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "item": item, "axis": axis}


func load_from(d: Dictionary) -> void:
	item = String(d.get("item", item))
	axis = int(d.get("axis", 0))


func summary() -> Dictionary:
	return {"kind": kind, "item": item}
