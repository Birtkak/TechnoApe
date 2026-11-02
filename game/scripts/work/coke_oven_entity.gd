class_name CokeOvenEntity
extends MachineEntity

## The coke oven (ADR 0117): the first multiblock. A controller block in front of a hollow 3 x 3 x 3
## of bricks (26 brick blocks, the middle one empty). WorkSystem checks the pattern once a second
## (`formed_at` over the bubble-first voxels: 27 reads, never per tick) and sets `enabled`; formed, the
## oven covers its brick cells too (footprint_cells), so a hopper on the hull feeds it. It burns no
## power (eu_t 0): coal to coal coke, logs to charcoal, by its process recipes.

var parts: Array[Vector3i] = []   # the hull's anchor cells while formed
var hull_dir := -1                # which way the hull lies: index into DIRS, -1 = not formed

const DIRS := [Vector3i(0, 0, -1), Vector3i(1, 0, 0), Vector3i(0, 0, 1), Vector3i(-1, 0, 0)]


func _init(p_items: ItemRegistry, p_recipes: RecipeRegistry, p_pos: Vector3i) -> void:
	super(p_items, p_recipes, p_pos, "coke_oven")
	enabled = false


func formed() -> bool:
	return enabled


## The hull's block anchors when a hollow 3 x 3 x 3 of bricks stands against the controller on side
## `dir` (world voxels, blocks of Settings.BLOCK), or [] when not. get_voxel reads bubble first.
static func hull_at(get_voxel: Callable, p_pos: Vector3i, dir: Vector3i) -> Array[Vector3i]:
	var n := Settings.BLOCK
	var side := Vector3i(dir.z, 0, -dir.x)   # across the hull
	var out: Array[Vector3i] = []
	for k in range(1, 4):
		for j in range(-1, 2):
			for i in range(-1, 2):
				var a := p_pos + dir * (k * n) + side * (i * n) + Vector3i(0, j * n, 0)
				var inner := k == 2 and i == 0 and j == 0
				var m := int(get_voxel.call(a))
				if inner:
					if m != GameMaterials.AIR:
						return []
				elif m != GameMaterials.BRICKS:
					return []
				else:
					out.append(a)
	return out


## The hull on any side, as {dir, parts}, or {} when none stands there.
static func formed_at(get_voxel: Callable, p_pos: Vector3i) -> Dictionary:
	for d in DIRS.size():
		var hull := hull_at(get_voxel, p_pos, DIRS[d])
		if not hull.is_empty():
			return {"dir": d, "parts": hull}
	return {}


## WorkSystem's per-second check: true when the formed state changed.
func set_formed(found: Dictionary) -> bool:
	var was := enabled
	if found.is_empty():
		parts = []
		hull_dir = -1
		enabled = false
	else:
		parts = found["parts"]
		hull_dir = int(found["dir"])
		enabled = true
	if was != enabled:
		changed.emit()
	return was != enabled


## The controller's own cube plus every brick cell while formed.
func footprint_cells() -> Array[Vector3i]:
	var out: Array[Vector3i] = BlockEntities.cube(pos)
	for a in parts:
		out.append_array(BlockEntities.cube(a))
	return out


func look_lines() -> Array[String]:
	var out := super()
	out.append("Formed: a hollow 3 x 3 x 3 of bricks behind it" if enabled else "Not formed: build a hollow 3 x 3 x 3 of bricks against its back")
	return out


## A brick-faced controller block with an iron door, lit while it cokes.
func view_parts() -> Dictionary:
	var size := Vector3i(8, 8, 8)
	var c := WorkPalette.new_cells(size)
	WorkPalette.box(c, size, Vector3i(0, 0, 0), Vector3i(8, 8, 8), WorkPalette.BRICK)
	WorkPalette.box(c, size, Vector3i(2, 1, 7), Vector3i(6, 6, 8), WorkPalette.IRON_DARK)   # the door
	WorkPalette.box(c, size, Vector3i(3, 2, 7), Vector3i(5, 3, 8), WorkPalette.EMBER if is_lit() else WorkPalette.SOOT)
	return {"solid": {"cells": c, "size": size}}


func to_dict() -> Dictionary:
	var d := super()
	d["hull_dir"] = hull_dir
	return d


func summary() -> Dictionary:
	var d := super()
	d["formed"] = enabled
	return d
