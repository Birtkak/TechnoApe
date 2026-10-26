extends Node

## A stand-in for Main in test_tnt.gd: a dictionary world and a bubble that records its blasts.

class StubBubble:
	extends RefCounted
	var sim = null
	var origin := Vector3i.ZERO
	var blasts := []
	var owner_main
	func blast(center: Vector3, radius: float, material := 0, only := PackedInt32Array()) -> int:
		blasts.append([center, radius, material, only])
		return int(4.0 / 3.0 * PI * radius * radius * radius) if blasts.size() == 1 else 0
	func get_voxel(p: Vector3i) -> int:
		return int(owner_main.voxels.get(p, 0))

var voxels := {}
var bubble := StubBubble.new()
var fx = null
var pieces = null
var mobs = null
var player = null
var player_inv = null
var items = null


func _init() -> void:
	bubble.owner_main = self


func get_world_voxel(p: Vector3i) -> int:
	return int(voxels.get(p, 0))


func set_world_voxel(p: Vector3i, m: int) -> void:
	voxels[p] = m
