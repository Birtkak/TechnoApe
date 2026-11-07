class_name LightningRodEntity
extends RefCounted

## A placed lightning rod (ADR 0149): the foot voxel of a 2 m pole (PlantStamps.for_placed). Being a block entity
## puts it in the saved list, so Weather finds every rod by position without scanning the world; a strike aimed
## within Settings.lightning_rod_m of one hits its tip and grounds harmlessly. Breaking the foot ends it.

signal changed

var kind := "lightning_rod"
var pos: Vector3i


func _init(p_pos: Vector3i) -> void:
	pos = p_pos


func tip() -> Vector3i:
	return pos + Vector3i(0, PlantStamps.ROD_H - 1, 0)


## The rod item comes back from the foot; the pole's voxels drop nothing (mining.json).
## shortcut: a foot dug first leaves the pole standing to dig voxel by voxel; give it torch-style cells if that annoys.
func contents() -> Array:
	return [{"id": "lightning_rod", "count": 1}]


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z]}


func load_from(_d: Dictionary) -> void:
	pass


func summary() -> Dictionary:
	return {"kind": kind}
