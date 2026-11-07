extends Node

## Stand-in for Main in test_companion.gd: flat ground at y <= 0 and a player node.

var player: Node3D
var wall_x := -1   # a solid wall at this x (all y, z), -1 = none


func get_world_voxel(p: Vector3i) -> int:
	if p.x == wall_x:
		return GameMaterials.GRASS
	return GameMaterials.GRASS if p.y <= 0 else GameMaterials.AIR
