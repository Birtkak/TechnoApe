class_name CharacterPreview
extends SubViewportContainer

## The player's body in the inventory's portrait box, like vanilla's: its own small 3D world with
## an ApeModel wearing what the player wears (set_worn), turning to face the mouse.

var model: ApeModel
var _vp: SubViewport


func _ready() -> void:
	stretch = true
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	_vp = SubViewport.new()
	_vp.own_world_3d = true
	_vp.transparent_bg = true
	_vp.render_target_update_mode = SubViewport.UPDATE_WHEN_VISIBLE
	add_child(_vp)
	var env := WorldEnvironment.new()
	env.environment = Environment.new()
	env.environment.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	env.environment.ambient_light_color = Color(0.8, 0.8, 0.85)
	_vp.add_child(env)
	var sun := DirectionalLight3D.new()
	sun.rotation = Vector3(-0.7, 0.5, 0)
	_vp.add_child(sun)
	model = ApeModel.new()
	_vp.add_child(model)
	var cam := Camera3D.new()
	cam.projection = Camera3D.PROJECTION_ORTHOGONAL
	cam.size = 4.2   # the model is 57 cells = 3.6 units tall
	cam.position = Vector3(0, 1.8, -6)   # in front: the body faces -Z
	cam.rotation.y = PI
	_vp.add_child(cam)


func set_worn(worn: Dictionary) -> void:
	if model:
		model.set_worn(worn)


func _process(delta: float) -> void:
	if not is_visible_in_tree():
		return
	# Minecraft's portrait: the body turns toward the mouse, the head also looks up or down at it
	var off := get_local_mouse_position() - Vector2(size.x / 2.0, size.y * 0.25)
	model.rotation.y = clampf(off.x / 120.0, -1.0, 1.0)
	model.animate(delta, 0.0, true, false, clampf(-off.y / 160.0, -0.6, 0.6), 0.0)
