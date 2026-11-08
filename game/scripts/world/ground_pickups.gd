class_name GroundPickups
extends RefCounted

## Pieces: things made of a handful of voxels that go as one (ADR 0043, ADR 0047). A fallen stick, a
## nodule of copper-bearing stones, a flower (petals and stem), a mushroom, a seashell, an ore nugget
## in the rock: digging any of its voxels, or holding right click on it (by hand: not ores), takes all
## of it. Plants that grow in stalks (grass, ferns, reeds, kelp, seagrass) go a stalk at a time: the
## column of touching voxels above and below.
## Kinds of piece:
##   blob      the touching voxels (26 neighbours) of one group: a pickup material, a clutter group
##             ("flower" = petals + stem), or one ore. Drops once, by its first non-stem material,
##             except ores, which give their voxels (a nugget of 20 voxels is 20 ore, as dug before).
##   column    one stalk; drops per voxel (a tall tuft gave more dry grass than a short one).

const MAX_CELLS := 96        # a piece is never bigger (a stick is 8-12 voxels, a flower ~30, kelp up to 80 tall)
const MAX_REACH_SQ := 196    # blobs: nor further than 14 voxels from where it was touched
const COLUMN_PLANTS := ["tall_grass", "savanna_tall_grass", "fern", "reed", "dead_bush", "kelp", "seagrass", "dune_grass", "tundra_sedge", "sagebrush", "desert_marigold", "moss_campion", "fire_lily", "lingonberry", "bromeliad", "cattail", "aloe", "bluebell", "frost_lichen", "sea_aster"]

static var _groups := PackedStringArray()   # material id -> group ("" = not a piece)
static var _column := PackedByteArray()
static var _per_voxel := PackedByteArray()
static var _hand := PackedByteArray()        # popped by a held right click
static var _stem := PackedByteArray()


static func _build() -> void:
	var d := GameMaterials.defs()
	_groups.resize(d.size() + 1)
	_column.resize(d.size() + 1)
	_per_voxel.resize(d.size() + 1)
	_hand.resize(d.size() + 1)
	_stem.resize(d.size() + 1)
	for i in d.size():
		var m := i + 1
		var e: Dictionary = d[i]
		var name: String = e["name"]
		var g := ""
		if m in GameMaterials.GROUND_PICKUPS:
			g = name
			_hand[m] = 1
		elif e.has("clutter"):
			g = String(e["clutter"])
			_hand[m] = 1
		elif name.ends_with("_ore") and GameMaterials.is_plain_voxel(m):
			g = name
			_per_voxel[m] = 1
		if name in COLUMN_PLANTS:
			g = name
			_column[m] = 1
			_per_voxel[m] = 1
		_groups[m] = g
		_stem[m] = 1 if e.get("stem", false) else 0


static func group_of(m: int) -> String:
	if _groups.is_empty():
		_build()
	return _groups[m] if m > 0 and m < _groups.size() else ""


## Any piece voxel: digs as part of its piece.
static func is_piece(m: int) -> bool:
	return group_of(m) != ""


## A piece a held right click pops out by hand: ground pickups and clutter (not ores, not stalks).
static func is_pickup(m: int) -> bool:
	return is_piece(m) and _hand[m] == 1 and _column[m] == 0


static func drops_per_voxel(m: int) -> bool:
	return is_piece(m) and _per_voxel[m] == 1


## The piece the voxel at `p` belongs to, `p` first. Empty when `p` holds no piece.
## `get_voxel` is (Vector3i) -> int.
static func piece(get_voxel: Callable, p: Vector3i) -> Array[Vector3i]:
	var out: Array[Vector3i] = []
	var m: int = get_voxel.call(p)
	if not is_piece(m):
		return out
	out.append(p)
	if _column[m] == 1:
		for dir in [Vector3i.DOWN, Vector3i.UP]:
			var q: Vector3i = p + dir
			while out.size() < MAX_CELLS and get_voxel.call(q) == m:
				out.append(q)
				q += dir
		return out
	var g := group_of(m)
	var seen := {p: true}
	var todo: Array[Vector3i] = [p]
	while not todo.is_empty() and out.size() < MAX_CELLS:
		var c: Vector3i = todo.pop_back()
		for dz in range(-1, 2):
			for dy in range(-1, 2):
				for dx in range(-1, 2):
					var q := c + Vector3i(dx, dy, dz)
					if seen.has(q) or q.distance_squared_to(p) > MAX_REACH_SQ:
						continue
					seen[q] = true
					if out.size() < MAX_CELLS and group_of(get_voxel.call(q)) == g:
						out.append(q)
						todo.append(q)
	return out


## Splits a dig selection: every piece voxel in `cells` brings its whole piece along.
## Returns {cells: the selection with the pieces completed (no repeats), pieces: [{material, cells,
## center, per_voxel}]}; material is what the piece drops as (a flower's petals, not its stem), the
## centre the middle of the piece's voxels.
static func complete(get_voxel: Callable, cells: Array[Vector3i]) -> Dictionary:
	var all: Array[Vector3i] = []
	var taken := {}
	var pieces := []
	for c in cells:
		if taken.has(c):
			continue
		var m: int = get_voxel.call(c)
		if not is_piece(m):
			taken[c] = true
			all.append(c)
			continue
		var pc := piece(get_voxel, c)
		var sum := Vector3.ZERO
		var cs: Array[Vector3i] = []
		var main := m
		for q in pc:
			if not taken.has(q):
				taken[q] = true
				all.append(q)
				cs.append(q)
				sum += Vector3(q)
				var qm: int = get_voxel.call(q)
				if _stem[main] == 1 and _stem[qm] == 0:
					main = qm
		if not cs.is_empty():
			pieces.append({"material": main, "cells": cs, "center": sum / cs.size() + Vector3(0.5, 0.5, 0.5),
				"per_voxel": drops_per_voxel(main)})
	return {"cells": all, "pieces": pieces}
