class_name LightningRodEntity
extends RefCounted

## A placed lightning rod (ADR 0149): the foot voxel of a 2 m pole (PlantStamps.for_placed). Being a block entity
## puts it in the saved list, so Weather finds every rod by position without scanning the world; a strike aimed
## within Settings.lightning_rod_m of one hits its tip and grounds harmlessly. Breaking any voxel takes the pole.

signal changed

var kind := "lightning_rod"
var pos: Vector3i


func _init(p_pos: Vector3i) -> void:
	pos = p_pos


func tip() -> Vector3i:
	return pos + Vector3i(0, PlantStamps.ROD_H - 1, 0)


## The pole's voxels: breaking any of them takes the whole rod (Main._break_torch). The item drops through mining.json.
func footprint_cells() -> Array[Vector3i]:
	var out: Array[Vector3i] = []
	for y in PlantStamps.ROD_H:
		out.append(pos + Vector3i(0, y, 0))
	return out


func contents() -> Array:
	return []


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z]}


func load_from(_d: Dictionary) -> void:
	pass


func summary() -> Dictionary:
	return {"kind": kind}
