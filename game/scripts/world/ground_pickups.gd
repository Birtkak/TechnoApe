class_name GroundPickups
extends RefCounted

## Ground pickups as whole pieces (ADR 0043): a fallen stick, a nodule of copper-bearing stones, a
## few pebbles are a handful of touching voxels of one pickup material (GameMaterials.GROUND_PICKUPS).
## A piece always goes as one: digging any of its voxels, or holding right click on it, takes all of
## it, and it pops out as one item where it lay.

const MAX_CELLS := 64        # a piece is never bigger (a stick is 8-12 voxels long, 1-2 wide)
const MAX_REACH_SQ := 196    # nor further than 14 voxels from where it was touched


static func is_pickup(m: int) -> bool:
	return m in GameMaterials.GROUND_PICKUPS


## The piece the pickup voxel at `p` belongs to: the touching voxels (26 neighbours) of the same
## material, `p` first. Empty when `p` holds no pickup. `get_voxel` is (Vector3i) -> int.
static func piece(get_voxel: Callable, p: Vector3i) -> Array[Vector3i]:
	var out: Array[Vector3i] = []
	var m: int = get_voxel.call(p)
	if not is_pickup(m):
		return out
	var seen := {p: true}
	var todo: Array[Vector3i] = [p]
	out.append(p)
	while not todo.is_empty() and out.size() < MAX_CELLS:
		var c: Vector3i = todo.pop_back()
		for dz in range(-1, 2):
			for dy in range(-1, 2):
				for dx in range(-1, 2):
					var q := c + Vector3i(dx, dy, dz)
					if seen.has(q) or q.distance_squared_to(p) > MAX_REACH_SQ:
						continue
					seen[q] = true
					if get_voxel.call(q) == m and out.size() < MAX_CELLS:
						out.append(q)
						todo.append(q)
	return out


## Splits a dig selection: every pickup voxel in `cells` brings its whole piece along.
## Returns {cells: the selection with the pieces completed (no repeats), pieces: [{material, cells,
## center}]}; the centre is the middle of the piece's voxels, a little above the ground.
static func complete(get_voxel: Callable, cells: Array[Vector3i]) -> Dictionary:
	var all: Array[Vector3i] = []
	var taken := {}
	var pieces := []
	for c in cells:
		if taken.has(c):
			continue
		var m: int = get_voxel.call(c)
		if not is_pickup(m):
			taken[c] = true
			all.append(c)
			continue
		var pc := piece(get_voxel, c)
		var sum := Vector3.ZERO
		var cs: Array[Vector3i] = []
		for q in pc:
			if not taken.has(q):
				taken[q] = true
				all.append(q)
				cs.append(q)
				sum += Vector3(q)
		if not cs.is_empty():
			pieces.append({"material": m, "cells": cs, "center": sum / cs.size() + Vector3(0.5, 0.5, 0.5)})
	return {"cells": all, "pieces": pieces}
