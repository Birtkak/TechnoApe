class_name DugoutEntity
extends RefCounted

## A dugout canoe being made (ADR 0059): an adze right clicked on a lying trunk (a felled tree, or
## logs laid in a row) marks a WIDE x HIGH x length box of its wood as the blank (StoneAge.start_dugout).
## Lay GRASS dry grass on it and light it: the hollow burns out over Settings.dugout_burn_h game hours;
## then ADZE_STROKES adze strokes (held clicks repeat) scrape out the char, and the canoe pops out as an
## item where the trunk lay. Its footprint is the box (solid work voxels), saved as anchor, axis, length.

signal changed
signal baked(item: String)   # the hollow has burnt out

const WIDE := 4      # voxels across
const HIGH := 3      # voxels up
const GRASS := 4
const ADZE_STROKES := 24

var kind := "dugout"
var pos: Vector3i
var axis := ""       # x or z; "" until set (no footprint yet)
var length := 0      # voxels along the axis
var grass := 0
var lit_at := -1.0
var burnt := false
var strokes := 0


func _init(p_pos: Vector3i) -> void:
	pos = p_pos


func is_lit() -> bool:
	return lit_at >= 0.0


func size() -> Vector3i:
	return Vector3i(length, HIGH, WIDE) if axis == "x" else Vector3i(WIDE, HIGH, length)


func footprint_cells() -> Array[Vector3i]:
	var out: Array[Vector3i] = []
	if axis == "":
		return out
	var s := size()
	for z in s.z:
		for y in s.y:
			for x in s.x:
				out.append(pos + Vector3i(x, y, z))
	return out


func hours_left() -> float:
	return maxf(0.0, Settings.dugout_burn_h - (StoneAge.now_days - lit_at) * 24.0) if is_lit() else 0.0


## Right click with `held`. Returns {consume, give, stroke, done: stack or {}, remove, lit, msg}.
func use(held: Dictionary) -> Dictionary:
	var id: String = held.get("id", "")
	var tags: Array = held.get("tags", [])
	var res := {"consume": 0, "give": [], "stroke": false, "done": {}, "remove": false, "lit": false, "msg": ""}
	if is_lit():
		res["msg"] = "The hollow is burning out: %.1f game hours left." % hours_left()
		return res
	if not burnt:
		if grass < GRASS:
			if id == "dry_grass":
				grass += 1
				res["consume"] = 1
				changed.emit()
				if grass == GRASS:
					res["msg"] = "Light the grass to burn out the hollow."
			else:
				res["msg"] = "Lay %d dry grass along the trunk (%d/%d), then light it." % [GRASS, grass, GRASS]
			return res
		if "firestarter" in tags:
			lit_at = StoneAge.now_days
			res["lit"] = true
			res["msg"] = "The fire eats into the trunk: %d game hours." % int(Settings.dugout_burn_h)
			changed.emit()
		else:
			res["msg"] = "Light the grass with a firestarter."
		return res
	if not "adze" in tags:
		res["msg"] = "Scrape the char out with an adze (%d/%d)." % [strokes, ADZE_STROKES]
		return res
	strokes += 1
	res["stroke"] = true
	if strokes >= ADZE_STROKES:
		res["done"] = {"id": "dugout_canoe", "count": 1}
		res["remove"] = true
	elif strokes % 4 == 0:
		changed.emit()   # the hollow is redrawn every fourth stroke (a 2304-cell piece)
	return res


func tick() -> bool:
	if is_lit() and hours_left() <= 0.0:
		lit_at = -1.0
		burnt = true
		grass = 0
		baked.emit("dugout_canoe")
		changed.emit()
	return false


func contents() -> Array:
	return [{"id": "dry_grass", "count": grass}] if grass > 0 and not is_lit() else []


## The trunk in work cells (two per voxel): bark outside, pale wood, the hollow charring black and
## widening as the fire and the adze work.
func view_parts() -> Dictionary:
	var vs := size()
	var cs := vs * 2
	var c := WorkPalette.new_cells(cs)
	var along_n := cs.x if axis == "x" else cs.z
	var hollow := 0 if not burnt else 1 + (WIDE * 2 - 3) * strokes / ADZE_STROKES
	for z in cs.z:
		for y in cs.y:
			for x in cs.x:
				var along := x if axis == "x" else z
				var across := z if axis == "x" else x
				var edge := across == 0 or across == WIDE * 2 - 1 or y == 0
				var inner := not edge and along > 1 and along < along_n - 2 and y >= cs.y - 1 - hollow and absi(across * 2 + 1 - WIDE * 2) < 2 + hollow * 2
				var paint := WorkPalette.WOOD_DARK if edge else WorkPalette.WOOD
				if inner and (burnt or is_lit()):
					paint = WorkPalette.TAR if strokes < ADZE_STROKES / 2 else WorkPalette.WOOD
					if hollow > 1 and y > 1:
						continue
				c[WorkPalette.idx(cs, x, y, z)] = paint + 1
	if grass > 0:
		for i in grass:
			var a := 2 + i * (along_n - 4) / GRASS
			for k in range(2, WIDE * 2 - 2):
				var q := Vector3i(a, cs.y - 1, k) if axis == "x" else Vector3i(k, cs.y - 1, a)
				c[WorkPalette.idx(cs, q.x, q.y, q.z)] = (WorkPalette.EMBER if is_lit() else WorkPalette.STRAW) + 1
	# make_view scales cells as 8 per block; ours are 2 per voxel = 8 per block
	return {"solid": {"cells": c, "size": cs}}


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z], "axis": axis, "length": length, "grass": grass, "lit_at": lit_at,
		"burnt": burnt, "strokes": strokes}


func load_from(d: Dictionary) -> void:
	axis = String(d.get("axis", "x"))
	length = int(d.get("length", 20))
	grass = int(d.get("grass", 0))
	lit_at = float(d.get("lit_at", -1.0))
	burnt = bool(d.get("burnt", false))
	strokes = int(d.get("strokes", 0))


func summary() -> Dictionary:
	return {"kind": kind, "pos": pos, "axis": axis, "length": length, "grass": grass, "lit": is_lit(), "burnt": burnt, "strokes": strokes}
