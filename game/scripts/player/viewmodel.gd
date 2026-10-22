class_name Viewmodel
extends Node3D

## First-person arm and held item, Minecraft style: the ape's right arm
## (ApeModel.build_arm_mesh, hand at this node's origin) pokes into the view
## from the lower right; the selected block (a small cube) or item (a card)
## sits in the hand. The whole node swings on use. Hidden in third person.

var items: ItemRegistry
var atlas_material: Material
var _mesh: MeshInstance3D
var _arm: MeshInstance3D
var _swing := 0.0        # 0 = rest, 1 = fully swung
var _swinging := false
var _rest_pos := Vector3(0.5, -0.54, -0.82)
var _current_id := ""


func _ready() -> void:
	_arm = ApeModel.build_arm_mesh()
	_arm.rotation_degrees = Vector3(62, 0, -28)   # hand at the origin, arm back towards the camera's lower right
	_arm.rotate_object_local(Vector3.UP, PI)      # back of the hand and the outer sleeve towards the camera
	_arm.scale = Vector3.ONE * 0.8
	add_child(_arm)
	_mesh = MeshInstance3D.new()
	_mesh.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	_mesh.position = Vector3(0.0, 0.02, -0.16)
	add_child(_mesh)
	position = _rest_pos
	rotation_degrees = Vector3(0, 20, 0)
	show_item({})


func show_item(stack: Dictionary) -> void:
	var id: String = stack.get("id", "")
	if id == _current_id:
		return
	_current_id = id
	if id == "":
		_mesh.visible = false
		return
	_mesh.visible = true
	var mat_id := ItemIcon.material_id_for(items, id)
	if mat_id > 0:
		_mesh.mesh = BlockMeshes.cube(mat_id, atlas_material)
		_mesh.scale = Vector3.ONE * 0.24
		_mesh.rotation_degrees = Vector3.ZERO
	else:
		_mesh.mesh = BlockMeshes.card(id)
		_mesh.scale = Vector3.ONE * 0.3
		_mesh.rotation_degrees = Vector3(0, 0, -30)


func swing() -> void:
	_swinging = true


func swing_amount() -> float:
	return _swing


func _process(delta: float) -> void:
	if _swinging:
		_swing = minf(1.0, _swing + delta * 7.0)
		if _swing >= 1.0:
			_swinging = false
	else:
		_swing = maxf(0.0, _swing - delta * 5.0)
	var s := sin(_swing * PI)
	position = _rest_pos + Vector3(-0.18 * s, -0.08 * s, -0.28 * s)
	rotation_degrees = Vector3(-40 * s, 20 - 25 * s, 0)
