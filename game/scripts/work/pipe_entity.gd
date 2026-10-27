class_name PipeEntity
extends RefCounted

## A bronze or copper pipe (ADR 0097) laid on the ground: a straight square tube with a flange at each end,
## along x or z (`axis`, the way the player faced when laying it). It holds nothing;
## BoilerEntity.feed walks over pipes on all four sides to find the machines, whatever their axis.
## shortcut: a bend is two pipes meeting at a corner with no elbow drawn; add elbows when pipes get looks.

signal changed

var kind := "pipe"
var item := "bronze_pipe"   # or copper_pipe: the cheap one, half the reach
var pos: Vector3i
var axis := 0   # 0 along x, 1 along z


func _init(p_pos: Vector3i) -> void:
	pos = p_pos


func contents() -> Array:
	return [{"id": item, "count": 1}]


func tick() -> bool:
	return false


func is_lit() -> bool:
	return false


func look_lines() -> Array[String]:
	return ["Carries steam from a boiler to the machine at the end of the run"]


func view_parts() -> Dictionary:
	var size := Vector3i(8, 4, 8)
	var c := WorkPalette.new_cells(size)
	# the body: a square tube 4 cells across, a lit copper line along its top edge; a flange one cell
	# bigger all round at each end, its mouth dark
	var copper := item == "copper_pipe"
	var body := WorkPalette.COPPER if copper else WorkPalette.BRONZE
	var edge := WorkPalette.MOLTEN if copper else WorkPalette.COPPER
	_box(c, size, Vector3i(1, 0, 2), Vector3i(7, 3, 6), body)
	_box(c, size, Vector3i(1, 2, 2), Vector3i(7, 3, 3), edge)
	for x in [0, 7]:
		_box(c, size, Vector3i(x, 0, 1), Vector3i(x + 1, 4, 7), body)
		_box(c, size, Vector3i(x, 3, 1), Vector3i(x + 1, 4, 2), edge)
		_box(c, size, Vector3i(x, 1, 3), Vector3i(x + 1, 3, 5), WorkPalette.SOOT)   # the mouth
	return {"solid": {"cells": c, "size": size}}


## A box in the pipe's own frame (along x), swapped onto z when the pipe runs that way.
func _box(c: PackedInt32Array, size: Vector3i, a: Vector3i, b: Vector3i, paint: int) -> void:
	if axis == 1:
		a = Vector3i(a.z, a.y, a.x)
		b = Vector3i(b.z, b.y, b.x)
	WorkPalette.box(c, size, a, b, paint)


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "axis": axis, "item": item}


func load_from(d: Dictionary) -> void:
	axis = int(d.get("axis", 0))
	item = String(d.get("item", "bronze_pipe"))


func summary() -> Dictionary:
	return {"kind": kind, "pos": pos, "axis": axis, "item": item}
