class_name MegalithEntity
extends RefCounted

## A megalith (ADR 0059): a stone of LEN blocks (1.5 m) split loose from the rock with wooden
## wedges. It lies along x or z, or stands (axis y) once raised. Logs under it (rollers) and two
## ropes let lever strokes push it a block at a time; raised, it is part of a stone circle
## (Megaliths). Its footprint is its LEN blocks of solid work voxels (WORK_BLOCK); BlockEntities
## asks footprint_cells(), empty until the axis is set.

signal changed

const LEN := 3
const ROLLER_LOGS := 2
const ROPES := 2

var kind := "megalith"
var pos: Vector3i
var axis := ""           # x, z (lying) or y (standing)
var rollers := 0         # logs put under it
var ropes := 0           # cordage tied round it
var push := 0            # lever strokes toward the next block
var lift := 0            # lever strokes raising it
var seed_ := 0


func _init(p_pos: Vector3i) -> void:
	pos = p_pos
	seed_ = hash(p_pos)


func standing() -> bool:
	return axis == "y"


func dir() -> Vector3i:
	return Vector3i(1, 0, 0) if axis == "x" else (Vector3i(0, 1, 0) if axis == "y" else Vector3i(0, 0, 1))


## The block anchors it covers.
func blocks() -> Array[Vector3i]:
	var out: Array[Vector3i] = []
	if axis == "":
		return out
	for i in LEN:
		out.append(pos + dir() * i * Settings.BLOCK)
	return out


func footprint_cells() -> Array[Vector3i]:
	var out: Array[Vector3i] = []
	for b in blocks():
		out.append_array(BlockEntities.cube(b))
	return out


## The travel picker of a stone circle chose (Megaliths._shrine sets the meta).
func set_recipe(r: Dictionary) -> void:
	if has_meta("travel"):
		get_meta("travel").travel(r)


func movable() -> bool:
	return not standing() and rollers >= ROLLER_LOGS and ropes >= ROPES


func is_lit() -> bool:
	return false


func tick() -> bool:
	return false


func contents() -> Array:
	var out := []
	if rollers > 0:
		out.append({"id": "oak_log", "count": rollers})
	if ropes > 0:
		out.append({"id": "cordage", "count": ropes})
	return out


## A rough boulder over LEN blocks: an 8 x 8 section per block, corners knocked off, the ends
## rounded, bumps from its seed; logs under it while it has rollers, ropes round it.
func view_parts() -> Dictionary:
	var n := 8 * LEN
	var size := Vector3i(n, 8, 8) if axis == "x" else (Vector3i(8, n, 8) if axis == "y" else Vector3i(8, 8, n))
	var c := WorkPalette.new_cells(size)
	var rng := RandomNumberGenerator.new()
	rng.seed = seed_
	for z in size.z:
		for y in size.y:
			for x in size.x:
				var q := Vector3i(x, y, z)
				var along := q.x if axis == "x" else (q.y if axis == "y" else q.z)
				var u := q.y if axis != "y" else q.x
				var v := q.z if axis != "z" else q.x
				if axis == "z":
					u = q.y
				# distance from the section's centre line, the ends tapering
				var du := absf(u + 0.5 - 4.0)
				var dv := absf(v + 0.5 - 4.0)
				var end := minf(along + 0.5, n - along - 0.5)
				var r := 4.0 - maxf(0.0, 2.5 - end) * 0.8 - float(hash(Vector3i(x / 3, y / 3, z / 3) + Vector3i(seed_, 0, 0)) % 3) * 0.35
				if du * du + dv * dv * 0.9 > r * r:
					continue
				var paint := WorkPalette.STONE if (x + y * 3 + z * 5) % 7 != 0 else WorkPalette.ANVIL
				c[WorkPalette.idx(size, x, y, z)] = paint + 1
	if not standing():
		for i in rollers:
			var at := 4 + i * (n - 8) / maxi(ROLLER_LOGS - 1, 1)
			for k in range(1, 7):
				var q := Vector3i(at, 0, k) if axis == "x" else Vector3i(k, 0, at)
				c[WorkPalette.idx(size, q.x, q.y, q.z)] = WorkPalette.WOOD + 1
		for i in ropes:
			var at := 6 + i * (n - 12) / maxi(ROPES - 1, 1)
			for k in 8:
				for h in range(1, 8):
					var q := Vector3i(at, h, k) if axis == "x" else Vector3i(k, h, at)
					var idx := WorkPalette.idx(size, q.x, q.y, q.z)
					if c[idx] != 0 and (k == 0 or k == 7 or h == 7 or c[WorkPalette.idx(size, q.x, mini(q.y + 1, size.y - 1), q.z)] == 0):
						c[idx] = WorkPalette.STRAW + 1
	return {"solid": {"cells": c, "size": size}}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "axis": axis, "rollers": rollers, "ropes": ropes, "seed": seed_}


func load_from(d: Dictionary) -> void:
	axis = String(d.get("axis", "x"))
	rollers = int(d.get("rollers", 0))
	ropes = int(d.get("ropes", 0))
	seed_ = int(d.get("seed", seed_))


func summary() -> Dictionary:
	return {"kind": kind, "pos": pos, "axis": axis, "rollers": rollers, "ropes": ropes, "push": push, "lift": lift}
