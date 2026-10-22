class_name Viewmodel
extends Node3D

## First-person held item: a small cube of the selected block (or a card for
## other items) hanging bottom-right of the camera, with a swing on use.
## Placeholder for an arm/body model; see ADR 0005.

var items: ItemRegistry
var atlas_material: Material
var _mesh: MeshInstance3D
var _swing := 0.0        # 0 = rest, 1 = fully swung
var _swinging := false
var _rest_pos := Vector3(0.5, -0.42, -0.85)
var _current_id := ""


func _ready() -> void:
	_mesh = MeshInstance3D.new()
	_mesh.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	add_child(_mesh)
	position = _rest_pos
	rotation_degrees = Vector3(0, 35, 0)
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


func _process(delta: float) -> void:
	if _swinging:
		_swing = minf(1.0, _swing + delta * 7.0)
		if _swing >= 1.0:
			_swinging = false
	else:
		_swing = maxf(0.0, _swing - delta * 5.0)
	var s := sin(_swing * PI)
	position = _rest_pos + Vector3(-0.15 * s, -0.1 * s, -0.25 * s)
	rotation_degrees = Vector3(-35 * s, 35 - 20 * s, 0)
