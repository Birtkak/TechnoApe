class_name DebugCamera
extends Camera3D

## Free camera for playtests and filmstrips (/cam, /fell cam): it flies anywhere while the player stays
## where it is, so terrain detail and the simulation window (both follow the player) stay put. Mouse
## looks, the move keys fly it (sprint for 3x), the player ignores input while it is on. Made for
## watching what the player's own view cannot see, like a tree falling inside a forest (P12).

var player: Node   # Player
var _yaw := 0.0
var _pitch := 0.0


func _init(p_player: Node) -> void:
	player = p_player
	name = "DebugCamera"


## Take over the view at `at` (world voxels), looking at `target`; from the player's view when at is null.
func enable(at: Variant = null, target: Variant = null) -> void:
	var cam: Camera3D = player.camera
	fov = cam.fov
	near = cam.near
	far = cam.far
	global_transform = cam.global_transform
	if at is Vector3:
		global_position = at
	if target is Vector3:
		look_at(target, Vector3.UP)
	_yaw = rotation.y
	_pitch = rotation.x
	make_current()
	player.free_cam = true


func disable() -> void:
	player.free_cam = false
	(player.camera as Camera3D).make_current()


func is_on() -> bool:
	return current


func _unhandled_input(event: InputEvent) -> void:
	if not current or player.autopilot:
		return
	if event is InputEventMouseMotion and Input.mouse_mode == Input.MOUSE_MODE_CAPTURED:
		_yaw -= event.relative.x * Settings.mouse_sensitivity
		_pitch = clampf(_pitch - event.relative.y * Settings.mouse_sensitivity * (-1.0 if Settings.invert_y else 1.0), -1.5, 1.5)
		rotation = Vector3(_pitch, _yaw, 0.0)


func _process(delta: float) -> void:
	if not current or player.autopilot or player.ui_open:
		return
	var input := Input.get_vector("move_left", "move_right", "move_forward", "move_back")
	var up_down := (1.0 if Input.is_action_pressed("jump") else 0.0) - (1.0 if Input.is_action_pressed("descend") else 0.0)
	var dir := global_transform.basis * Vector3(input.x, 0.0, input.y) + Vector3(0.0, up_down, 0.0)
	if dir.length_squared() > 0.0:
		var speed := Settings.m2v(Settings.fly_speed_m) * (3.0 if Input.is_action_pressed("sprint") else 1.0)
		global_position += dir.normalized() * speed * delta
