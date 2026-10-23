class_name TorchEntity
extends RefCounted

## A placed torch (ADR 0030). It holds no state; being a block entity puts
## every torch in the saved block-entity list, so TorchLights finds them
## after a reload without scanning the world. The torch item drops through
## mining.json, so contents() is empty.

signal changed

var kind := "torch"
var pos: Vector3i


func _init(p_pos: Vector3i) -> void:
	pos = p_pos


func tick() -> bool:
	return false


func is_lit() -> bool:
	return false   # never ticks, never makes the save dirty


func contents() -> Array:
	return []


func to_dict() -> Dictionary:
	return {"kind": kind, "pos": [pos.x, pos.y, pos.z]}


func load_from(_d: Dictionary) -> void:
	pass


func summary() -> Dictionary:
	return {"kind": kind, "pos": pos}
