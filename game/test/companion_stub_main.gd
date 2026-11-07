extends Node

## Stand-in for Main in test_companion.gd: flat ground at y <= 0 and a player node.

var player: Node3D


func get_world_voxel(p: Vector3i) -> int:
	return GameMaterials.GRASS if p.y <= 0 else GameMaterials.AIR
