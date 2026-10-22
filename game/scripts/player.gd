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


func set_look(yaw: float, pitch: float) -> void:
	_yaw = yaw
	_pitch = clampf(pitch, -1.5, 1.5)
	if camera:
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


var _ground_seen_at := -1.0   # seconds; terrain under the player was first seen at this time


## Spawn / teleport safety: while the terrain below has not streamed in
## (and for a short grace period after, until its collision exists), the body
## does not move at all, so nobody falls through an unloaded world.
func _ground_ready() -> bool:
	if flying:
		return true
	var now := Time.get_ticks_msec() / 1000.0
	if _ground_seen_at < 0.0:
		# Ask the physics world, not the voxel data: data streams in long before
		# the collision shapes exist, and only colliders stop a fall.
		var space := get_world_3d().direct_space_state
		var q := PhysicsRayQueryParameters3D.create(global_position + Vector3(0, 1, 0), global_position + Vector3(0, -64, 0))
		q.exclude = [get_rid()]
		var hit := space.intersect_ray(q)
		if hit.is_empty():
			return false
		_ground_seen_at = now
		return false
	return now - _ground_seen_at > 0.3


func _physics_process(delta: float) -> void:
	if not _ground_ready():
		velocity = Vector3.ZERO
		return
	var input := Vector2.ZERO
	var up_down := 0.0
	if autopilot:
		input = Vector2(0, -1)
	else:
		input = Input.get_vector("move_left", "move_right", "move_forward", "move_back")
		up_down = (1.0 if Input.is_action_pressed("jump") else 0.0) - (1.0 if Input.is_action_pressed("descend") else 0.0)
	var dir := (transform.basis * Vector3(input.x, 0, input.y)).normalized()
	if flying:
		if autopilot and autopilot_speed_m <= 0.0:
			velocity = Vector3.ZERO
			return
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
	_try_step_up(delta)
	move_and_slide()


## Minecraft-style auto step: when walking into a ledge no taller than
## Settings.step_height_m, lift the body onto it instead of stopping. Taller
## ledges (steep rock, cliffs) still block, so mountains stay hard to climb.
func _try_step_up(delta: float) -> void:
	if not is_on_floor():
		return
	var horizontal := Vector3(velocity.x, 0, velocity.z)
	if horizontal.length_squared() < 0.0001:
		return
	var motion := horizontal * delta
	if not test_move(global_transform, motion):
		return  # nothing in the way
	var step := Vector3(0, Settings.m2v(Settings.step_height_m), 0)
	if test_move(global_transform, step):
		return  # ceiling right above
	var raised := global_transform.translated(step)
	if test_move(raised, motion):
		return  # still blocked: the ledge is taller than one step
	global_position += step
	floor_snap_length = step.y + 0.1
