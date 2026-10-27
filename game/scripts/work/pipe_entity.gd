class_name PipeEntity
extends RefCounted

## A bronze pipe (ADR 0097) laid on the ground: a cross of tube that joins whatever lies against it
## on the four sides. It holds nothing; BoilerEntity.feed walks over pipes to find the machines.

signal changed

var kind := "bronze_pipe"
var pos: Vector3i


func _init(p_pos: Vector3i) -> void:
	pos = p_pos


func contents() -> Array:
	return [{"id": "bronze_pipe", "count": 1}]


func tick() -> bool:
	return false


func is_lit() -> bool:
	return false


func look_lines() -> Array[String]:
	return ["Carries steam from a boiler to the machine at the end of the run"]


func view_parts() -> Dictionary:
	var size := Vector3i(8, 4, 8)
	var c := WorkPalette.new_cells(size)
	# two round tubes crossing on the ground (radius 1.5 cells, the corners off so they read as round), a lit
	# copper line along each top, and a one-cell iron collar round each of the four ends
	for i in 8:
		for a in range(-2, 2):
			for b in range(-2, 2):
				var round := (a + 0.5) * (a + 0.5) + (b + 0.5) * (b + 0.5) <= 2.6
				if not round:
					continue
				var tone := WorkPalette.COPPER if b == 1 and a in [-1, 0] else WorkPalette.BRONZE
				c[WorkPalette.idx(size, i, b + 2, a + 4)] = tone + 1   # along x
				c[WorkPalette.idx(size, a + 4, b + 2, i)] = tone + 1   # along z
	for i in [0, 7]:
		for a in range(-3, 3):
			for b in range(-2, 3):
				if (a + 0.5) * (a + 0.5) + (b + 0.5) * (b + 0.5) <= 4.6 and b + 2 < 4 and b + 2 >= 0:
					c[WorkPalette.idx(size, i, b + 2, a + 4)] = WorkPalette.IRON_DARK + 1
					c[WorkPalette.idx(size, a + 4, b + 2, i)] = WorkPalette.IRON_DARK + 1
	return {"solid": {"cells": c, "size": size}}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z]}


func load_from(_d: Dictionary) -> void:
	pass


func summary() -> Dictionary:
	return {"kind": kind, "pos": pos}
