class_name Player
extends CharacterBody3D

## First-person player. World units are voxels (0.5 m); all tunables live in
## Settings in metres and are converted here.

signal break_requested(hit: VoxelRaycastResult)
signal place_requested(hit: VoxelRaycastResult)

var world: World
var camera: Camera3D
var flying := true
var _yaw := 0.0
var _pitch := -0.2
var _mouse_captured := false
var autopilot := false
var autopilot_dir := Vector3(0, 0, -1)
var autopilot_speed_m := 6.0


func _ready() -> void:
	var shape := CollisionShape3D.new()
	var cap := CapsuleShape3D.new()
	cap.radius = Settings.m2v(Settings.player_radius_m)
	cap.height = Settings.m2v(Settings.player_height_m)
	shape.shape = cap
	shape.position = Vector3(0, cap.height / 2.0, 0)
	add_child(shape)
	camera = Camera3D.new()
	camera.position = Vector3(0, Settings.m2v(Settings.eye_height_m), 0)
	camera.fov = 75.0
	camera.near = 0.05
	camera.far = Settings.m2v(Settings.view_distance_m) * 1.5
	add_child(camera)
	camera.make_current()
	var viewer := VoxelViewer.new()
	viewer.view_distance = Settings.view_distance_voxels()
	viewer.requires_collisions = true
	camera.add_child(viewer)
	_apply_look()


func capture_mouse(on: bool) -> void:
	_mouse_captured = on
	Input.mouse_mode = Input.MOUSE_MODE_CAPTURED if on else Input.MOUSE_MODE_VISIBLE


func _unhandled_input(event: InputEvent) -> void:
	if autopilot:
		return
	if event is InputEventMouseMotion and _mouse_captured:
		_yaw -= event.relative.x * Settings.mouse_sensitivity
		_pitch = clampf(_pitch - event.relative.y * Settings.mouse_sensitivity, -1.5, 1.5)
		_apply_look()
	elif event is InputEventMouseButton and event.pressed and not _mouse_captured:
		capture_mouse(true)
	elif event.is_action_pressed("release_mouse"):
		capture_mouse(false)
	elif event.is_action_pressed("toggle_fly"):
		flying = not flying
	elif event.is_action_pressed("action_break") and _mouse_captured:
		var hit := _aim()
		if hit:
			break_requested.emit(hit)
	elif event.is_action_pressed("action_place") and _mouse_captured:
		var hit := _aim()
		if hit:
			place_requested.emit(hit)


func _aim() -> VoxelRaycastResult:
	return world.raycast(camera.global_position, -camera.global_transform.basis.z, Settings.m2v(Settings.reach_m))


func _apply_look() -> void:
	rotation = Vector3(0, _yaw, 0)
	camera.rotation = Vector3(_pitch, 0, 0)


func _physics_process(delta: float) -> void:
	var input := Vector2.ZERO
	var up_down := 0.0
	if autopilot:
		input = Vector2(0, -1)
	else:
		input = Input.get_vector("move_left", "move_right", "move_forward", "move_back")
		up_down = (1.0 if Input.is_action_pressed("jump") else 0.0) - (1.0 if Input.is_action_pressed("descend") else 0.0)
	var dir := (transform.basis * Vector3(input.x, 0, input.y)).normalized()
	if flying:
		var speed := Settings.m2v(autopilot_speed_m if autopilot else Settings.fly_speed_m)
		velocity = dir * speed
		velocity.y = up_down * speed
		move_and_slide()
		return
	var speed_m := Settings.sprint_speed_m if Input.is_action_pressed("sprint") else Settings.walk_speed_m
	var speed := Settings.m2v(speed_m)
	velocity.x = dir.x * speed
	velocity.z = dir.z * speed
	if not is_on_floor():
		velocity.y -= Settings.m2v(Settings.gravity_m) * delta
	elif Input.is_action_just_pressed("jump"):
		velocity.y = sqrt(2.0 * Settings.m2v(Settings.gravity_m) * Settings.m2v(Settings.jump_height_m))
	move_and_slide()
