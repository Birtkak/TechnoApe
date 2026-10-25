class_name TorchEntity
extends RefCounted

## A placed torch (ADR 0030, ADR 0053). Being a block entity puts every torch in the saved
## block-entity list, so TorchLights finds them after a reload without scanning the world. Since
## ADR 0053 a torch is a piece of voxels (PlantStamps.torch: stick, ember wrap, flame), standing on a
## floor or leaning out from a wall (`wall` = the direction to it); its footprint is those voxels, so
## breaking any of them breaks the torch. Torches from older saves are their one voxel (multi false).
## The torch item drops through mining.json, so contents() is empty.

signal changed

var kind := "torch"
var pos: Vector3i
var wall := Vector3i.ZERO
var multi := false
var cells: Array[Vector3i] = []


func _init(p_pos: Vector3i) -> void:
	pos = p_pos
	cells = [pos]


## Makes this a multi-voxel torch hanging on `p_wall` (ZERO: standing).
func set_mount(p_wall: Vector3i) -> void:
	wall = p_wall
	multi = true
	cells = []
	for o in PlantStamps.torch(wall):
		cells.append(pos + o)


## Where the flame burns, in voxels (the light's place).
func flame() -> Vector3:
	if not multi:
		return Vector3(pos) + Vector3(0.5, 0.85, 0.5)
	return Vector3(pos + PlantStamps.torch_flame(wall)) + Vector3(0.5, 0.5, 0.5)


func tick() -> bool:
	return false


func is_lit() -> bool:
	return false   # never ticks, never makes the save dirty


func contents() -> Array:
	return []


func to_dict() -> Dictionary:
	var d := {"kind": kind, "pos": [pos.x, pos.y, pos.z]}
	if multi:
		d["wall"] = [wall.x, wall.y, wall.z]
	return d


func load_from(d: Dictionary) -> void:
	var w: Array = d.get("wall", [])
	if w.size() == 3:
		set_mount(Vector3i(int(w[0]), int(w[1]), int(w[2])))


func summary() -> Dictionary:
	return {"kind": kind, "pos": pos, "wall": wall}
