class_name StationEntity
extends RefCounted

## A crafting table (ADR 0007, 0034). It holds no state; being a block entity
## gives it a footprint (BLOCK^3 voxels from its anchor) and a view, like the
## chests and furnaces that do hold inventories. The table item drops through
## mining.json, so contents() is empty.

signal changed

var kind := "station"
var pos: Vector3i


func _init(p_pos: Vector3i) -> void:
	pos = p_pos


func tick() -> bool:
	return false


func is_lit() -> bool:
	return false


func contents() -> Array:
	return []


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z]}


func load_from(_d: Dictionary) -> void:
	pass


func summary() -> Dictionary:
	return {"kind": kind, "pos": pos}
