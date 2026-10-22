class_name VoxelBody
extends RefCounted

## Minecraft-style movement of an axis-aligned box through the voxel grid
## (ADR 0011): godot_voxel's VoxelBoxMover against the blocky models'
## collision boxes, no physics colliders. Models whose collision_mask lacks
## MASK_SOLID (liquids, gases, fire) and models without collision boxes
## (micro plants) do not block. Reads voxel data, so an edit blocks the same
## frame it is written, with no collider rebuild.
##
## The box is in local coordinates relative to the node position (player:
## feet centre). After move(): on_floor / hit_ceiling / blocked per axis.

const MASK_SOLID := 1
const EPS := 0.0001

var aabb: AABB
var on_floor := false
var hit_ceiling := false
var blocked := Vector3.ZERO      # 1 on an axis whose motion was cut short
var stepped := false
var _mover := VoxelBoxMover.new()


func _init(p_aabb: AABB, step_height: float = 0.0) -> void:
	aabb = p_aabb
	_mover.set_collision_mask(MASK_SOLID)
	_mover.set_max_step_height(step_height)


## Moves by velocity * delta as far as the voxels allow. Returns the motion
## actually possible (add it to the position). Step climbing only when
## allow_step (Minecraft only steps up while on the ground).
func move(pos: Vector3, velocity: Vector3, delta: float, terrain: Node, allow_step: bool = false) -> Vector3:
	var motion := velocity * delta
	_mover.set_step_climbing_enabled(allow_step and _mover.get_max_step_height() > 0.0)
	var got: Vector3 = _mover.get_motion(pos, motion, aabb, terrain)
	stepped = allow_step and _mover.has_stepped_up()
	blocked = Vector3(
		1.0 if absf(got.x - motion.x) > EPS else 0.0,
		1.0 if absf(got.y - motion.y) > EPS and not stepped else 0.0,
		1.0 if absf(got.z - motion.z) > EPS else 0.0)
	on_floor = stepped or (motion.y < 0.0 and got.y > motion.y + EPS)
	hit_ceiling = not stepped and motion.y > 0.0 and got.y < motion.y - EPS
	return got


## True when the box at pos overlaps a blocking voxel (stuck inside terrain).
func intersects(pos: Vector3, terrain: Node) -> bool:
	return _mover.intersects(AABB(aabb.position + pos, aabb.size), terrain)
