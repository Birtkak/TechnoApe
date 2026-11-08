class_name NetOps
extends RefCounted

## Voxel edit ops on the wire and in the journal (ADR 0145): encode, decode, the chunks an op touches.
## An op is a Dictionary with "k" (fill | voxels | box | sphere) and "box": [lo, hi] world voxels, inclusive.


## `cells` are x, y, z triples relative to `lo` (world cells when lo is zero); `mats` optional per-cell materials (prefabs).
static func fill(lo: Vector3i, cells: PackedInt32Array, m: int, mask: PackedByteArray, mats := PackedByteArray()) -> Dictionary:
	var b0 := lo + Vector3i(cells[0], cells[1], cells[2]) if cells.size() >= 3 else lo
	var b1 := b0
	for i in range(0, cells.size(), 3):
		var c := lo + Vector3i(cells[i], cells[i + 1], cells[i + 2])
		b0 = b0.min(c)
		b1 = b1.max(c)
	return {"k": "fill", "lo": lo, "cells": cells, "m": m, "mask": mask, "mats": mats, "box": [b0, b1]}


static func voxels(cells: PackedInt32Array, mats: PackedByteArray) -> Dictionary:
	var lo := Vector3i(cells[0], cells[1], cells[2])
	var hi := lo
	for i in range(0, cells.size(), 3):
		var c := Vector3i(cells[i], cells[i + 1], cells[i + 2])
		lo = lo.min(c)
		hi = hi.max(c)
	return {"k": "voxels", "cells": cells, "mats": mats, "box": [lo, hi]}


static func box(lo: Vector3i, hi: Vector3i, m: int) -> Dictionary:
	return {"k": "box", "lo": lo, "hi": hi, "m": m, "box": [lo, hi - Vector3i.ONE]}


static func sphere(c: Vector3, r: float, m: int, only: PackedInt32Array) -> Dictionary:
	var e := Vector3.ONE * (r + 1.0)
	return {"k": "sphere", "c": c, "r": r, "m": m, "only": only, "box": [Vector3i((c - e).floor()), Vector3i((c + e).ceil())]}


static func encode(op: Dictionary) -> PackedByteArray:
	return var_to_bytes(op)


static func decode(bytes: PackedByteArray) -> Dictionary:
	var v: Variant = bytes_to_var(bytes)
	if not (v is Dictionary) or not v.has("k") or not (v.get("box") is Array) or (v["box"] as Array).size() != 2:
		return {}
	return v


## The world chunks (16^3) the op's box touches.
static func chunks_of(op: Dictionary) -> Array[Vector3i]:
	var out: Array[Vector3i] = []
	var lo := Vector3i((Vector3(op["box"][0]) / Settings.CHUNK).floor())
	var hi := Vector3i((Vector3(op["box"][1]) / Settings.CHUNK).floor())
	for z in range(lo.z, hi.z + 1):
		for y in range(lo.y, hi.y + 1):
			for x in range(lo.x, hi.x + 1):
				out.append(Vector3i(x, y, z))
	return out


## Applies an op to a client's bubble through the same calls that made it. False when the area was not
## loaded (the replay on chunk load tries again).
static func apply(op: Dictionary, bubble: Node, st: RefCounted) -> bool:   # a SimBubble and a ShapeTool: untyped so the server, without the extension, loads this
	if bubble == null or not bubble._ready:
		return false
	match op["k"]:
		"fill":
			return st.fill(bubble.sim, op["cells"], op["m"], op.get("mats", PackedByteArray()), bubble.origin * Settings.CHUNK - op["lo"], op["mask"]) >= 0
		"voxels":
			if op["cells"].size() == 3:   # one cell (a click, a fire lit): the sim's own write, which wakes and lights it
				return bubble.set_voxel(Vector3i(op["cells"][0], op["cells"][1], op["cells"][2]), op["mats"][0])
			return st.fill(bubble.sim, op["cells"], 0, op["mats"], bubble.origin * Settings.CHUNK, all_mask()) >= 0
		"box":
			return bubble.fill_box(op["lo"], op["hi"], op["m"]) >= 0
		"sphere":
			return bubble.blast(op["c"], op["r"], op["m"], op["only"]) > 0
	return false


static var _all: PackedByteArray


static func all_mask() -> PackedByteArray:
	if _all.is_empty():
		_all.resize(256)
		_all.fill(1)
	return _all
