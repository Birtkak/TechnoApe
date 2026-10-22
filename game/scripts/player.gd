class_name Player
extends CharacterBody3D

## First-person player (ADR 0005). World units are voxels (0.5 m); all
## tunables live in Settings in metres and are converted here. Owns the
## PlayerInventory model, the mining progress and the held-item viewmodel;
## world writes are requested through signals and done by Main.

signal voxel_broken(pos: Vector3i, drop: Dictionary)
signal voxel_placed(pos: Vector3i, material: int)
signal item_dropped(stack: Dictionary, from: Vector3, velocity: Vector3)
signal inventory_toggled(open: bool)
signal block_used(pos: Vector3i, material: int)   # right click on a block with an "interact" key (ADR 0007)
signal died

var world: World
var camera: Camera3D
var inventory: PlayerInventory
var mining: MiningTable
var viewmodel: Viewmodel
var hud: GameHud
var flying := true
var ui_open := false
var _yaw := 0.0
var _pitch := -0.2
var _mouse_captured := false
var autopilot := false
var autopilot_dir := Vector3(0, 0, -1)
var autopilot_speed_m := 6.0
var autopilot_action := ""     # "" = fly/walk forward, "mine" = stand still and hold break

# mining state
var _mine_target := Vector3i(1 << 30, 0, 0)
var _mine_progress := 0.0
var _mine_time := -1.0
var _break_cooldown := 0.0
var _place_cooldown := 0.0
var _highlight: MeshInstance3D
var _crack: MeshInstance3D

# fall damage
var _fall_peak_y := 0.0
var _was_on_floor := false


func _ready() -> void:
	collision_layer = 2          # player body; terrain is layer 1, items layer 4 (items pass through the player)
	collision_mask = 1
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
	_highlight = _make_box_outline()
	_highlight.visible = false
	get_tree().root.call_deferred("add_child", _highlight)
	_crack = _make_crack_box()
	_crack.visible = false
	get_tree().root.call_deferred("add_child", _crack)
	_apply_look()


## Called by Main once the registries exist.
func setup_inventory(p_inv: PlayerInventory, p_mining: MiningTable, atlas_material: Material) -> void:
	inventory = p_inv
	mining = p_mining
	viewmodel = Viewmodel.new()
	viewmodel.items = inventory.items
	viewmodel.atlas_material = atlas_material
	camera.add_child(viewmodel)
	inventory.selection_changed.connect(func(_i: int) -> void: viewmodel.show_item(inventory.held()))
	inventory.changed.connect(func() -> void: viewmodel.show_item(inventory.held()))


func set_look(yaw: float, pitch: float) -> void:
	_yaw = yaw
	_pitch = clampf(pitch, -1.5, 1.5)
	if camera:
		_apply_look()


func capture_mouse(on: bool) -> void:
	_mouse_captured = on
	Input.mouse_mode = Input.MOUSE_MODE_CAPTURED if on else Input.MOUSE_MODE_VISIBLE


## notify = false when Main opens a station screen itself (block_used).
func set_ui_open(open: bool, notify: bool = true) -> void:
	ui_open = open
	_mine_progress = 0.0
	capture_mouse(not open)
	if notify or not open:
		inventory_toggled.emit(open)


func _unhandled_input(event: InputEvent) -> void:
	if autopilot:
		return
	if event.is_action_pressed("inventory"):
		set_ui_open(not ui_open)
		return
	if ui_open:
		if event.is_action_pressed("release_mouse"):
			set_ui_open(false)
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
	elif event.is_action_pressed("hotbar_next"):
		inventory.scroll(1)
	elif event.is_action_pressed("hotbar_prev"):
		inventory.scroll(-1)
	elif event.is_action_pressed("drop"):
		_drop(Input.is_key_pressed(KEY_CTRL))
	elif event.is_action_pressed("action_place") and _mouse_captured:
		_use(true)
	else:
		for i in 9:
			if event.is_action_pressed("hotbar_%d" % (i + 1)):
				inventory.select(i)
				break


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
	_break_cooldown = maxf(0.0, _break_cooldown - delta)
	_place_cooldown = maxf(0.0, _place_cooldown - delta)
	_update_mining(delta)
	if not _ground_ready():
		velocity = Vector3.ZERO
		return
	var input := Vector2.ZERO
	var up_down := 0.0
	if autopilot:
		input = Vector2.ZERO if autopilot_action != "" else Vector2(0, -1)
	elif not ui_open:
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
		_fall_peak_y = position.y
		return
	var speed_m := Settings.sprint_speed_m if (Input.is_action_pressed("sprint") and not ui_open) else Settings.walk_speed_m
	if autopilot:
		speed_m = minf(autopilot_speed_m, Settings.sprint_speed_m)   # speed=0 stands still
	var speed := Settings.m2v(speed_m)
	velocity.x = dir.x * speed
	velocity.z = dir.z * speed
	if not is_on_floor():
		velocity.y -= Settings.m2v(Settings.gravity_m) * delta
	elif Input.is_action_just_pressed("jump") and not ui_open:
		velocity.y = sqrt(2.0 * Settings.m2v(Settings.gravity_m) * Settings.m2v(Settings.jump_height_m))
	_try_step_up(delta)
	move_and_slide()
	_unstick_from_voxels()
	_update_fall_damage()
	if autopilot and autopilot_action == "place" and _place_cooldown <= 0.0:
		_use(true)


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


## Terrain colliders are swapped when a mesh block is rebuilt; during that
## gap the body can sink into voxel data that is solid. Trust the data: if the
## feet are inside a solid voxel, lift to its top and stop falling.
func _unstick_from_voxels() -> void:
	var feet := Vector3i((position + Vector3(0, 0.05, 0)).floor())
	if feet.y < 0 or not _is_solid_voxel(feet):
		return
	for _i in 4:
		feet.y += 1
		if not _is_solid_voxel(feet):
			position.y = float(feet.y) + 0.01
			velocity.y = maxf(velocity.y, 0.0)
			return


func _is_solid_voxel(v: Vector3i) -> bool:
	var m := world.get_voxel(v)
	if m <= 0:
		return false
	var d: Dictionary = GameMaterials.defs()[m - 1]
	if d.get("cross", false) or d.get("cell_type", "solid") == "gas":
		return false
	return d.get("cell_type", "solid") != "liquid" or d.get("liquid_sand", false)


# ---------------- health ----------------

## Vanilla fall damage: 1 per block fallen beyond the safe distance.
func _update_fall_damage() -> void:
	# "landed" = floor contact, or any contact while no longer falling (standing
	# on a voxel edge in a shaft reports a wall normal, not a floor)
	var on_floor := is_on_floor() or (get_slide_collision_count() > 0 and velocity.y >= -Settings.m2v(1.0))
	if on_floor and not _was_on_floor:
		var fall_m := Settings.v2m(_fall_peak_y - position.y)
		var dmg := int(floor(fall_m - Settings.fall_damage_safe_m))
		if dmg > 0 and inventory:
			print("fall damage %d after %.1f m" % [dmg, fall_m])
			inventory.damage(dmg)
			if inventory.is_dead():
				died.emit()
	if on_floor or position.y > _fall_peak_y:
		_fall_peak_y = position.y
	_was_on_floor = on_floor


func respawn_at(pos: Vector3) -> void:
	position = pos
	velocity = Vector3.ZERO
	_fall_peak_y = pos.y
	_ground_seen_at = -1.0
	if inventory:
		inventory.respawn()


# ---------------- mining ----------------

func _update_mining(delta: float) -> void:
	var holding := false
	if autopilot:
		holding = autopilot_action == "mine"
	else:
		holding = _mouse_captured and not ui_open and Input.is_action_pressed("action_break")
	var hit: VoxelRaycastResult = _aim() if (holding or (_mouse_captured and not ui_open and not autopilot)) else null
	if hit:
		_highlight.visible = true
		_highlight.global_position = Vector3(hit.position)
	else:
		_highlight.visible = false
	if not holding or hit == null or _break_cooldown > 0.0:
		_reset_mining()
		return
	var p: Vector3i = hit.position
	if p != _mine_target:
		_mine_target = p
		_mine_progress = 0.0
		var mat_name := GameMaterials.name_of(world.get_voxel(p))
		_mine_time = mining.break_time(mat_name, inventory.held_def()) if mining else 0.0
		if viewmodel:
			viewmodel.swing()
	if _mine_time < 0.0:
		_crack.visible = false
		if hud:
			hud.break_progress = 0.0
		return  # unbreakable
	_mine_progress += delta / maxf(_mine_time, 0.001) if _mine_time > 0.0 else 1.0
	if hud:
		hud.break_progress = _mine_progress
	_crack.visible = _mine_progress > 0.0 and _mine_time > 0.0
	_crack.global_position = Vector3(p)
	_crack.transparency = 1.0 - clampf(_mine_progress, 0.0, 1.0) * 0.4
	if _mine_progress >= 1.0:
		var mat_name := GameMaterials.name_of(world.get_voxel(p))
		var drop := mining.drops(mat_name, inventory.held_def()) if mining else {}
		voxel_broken.emit(p, drop)
		if viewmodel:
			viewmodel.swing()
		_break_cooldown = Settings.break_cooldown_s
		if autopilot and autopilot_action == "mine":
			print("mined %s (%s) player_y=%.2f on_floor=%s" % [p, GameMaterials.name_of(world.get_voxel(p)), position.y, is_on_floor()])
			# dig a 2x2 shaft: the player stands on the corner where four voxels
			# meet and turns a quarter after every break (the body is 1.2 voxels wide)
			_yaw += PI / 2.0
			_apply_look()
		_reset_mining()
	elif viewmodel and fmod(_mine_progress * 4.0, 1.0) < delta * 4.0:
		viewmodel.swing()
	# keep placing while the button is held (vanilla repeats every 4 ticks)
	if not autopilot and Input.is_action_pressed("action_place") and _place_cooldown <= 0.0:
		_use(false)


func _reset_mining() -> void:
	_mine_target = Vector3i(1 << 30, 0, 0)
	_mine_progress = 0.0
	_crack.visible = false
	if hud:
		hud.break_progress = 0.0


# ---------------- using the held item (right click) ----------------

func _use(first_press: bool) -> void:
	if inventory == null:
		return
	var hit := _aim()
	if hit == null:
		return
	if not first_press and _place_cooldown > 0.0:
		return
	var target: Vector3i = hit.position
	# stations open on right click; sneaking (Shift) places against them instead (vanilla)
	if first_press and not Input.is_action_pressed("sprint"):
		var tm := world.get_voxel(target)
		if tm > 0 and GameMaterials.defs()[tm - 1].has("interact"):
			block_used.emit(target, tm)
			return
	var def := inventory.held_def()
	if def.is_empty():
		return
	var target_name := _loose_name(world.get_voxel(target))
	# container pickup (bucket on water / lava)
	if def.has("fills") and def["fills"].has(target_name):
		voxel_placed.emit(target, GameMaterials.AIR)
		inventory.replace_held_one(def["fills"][target_name])
		_after_use()
		return
	if not def.has("material"):
		return
	var mat := ItemIcon.material_id_for(inventory.items, def["id"])
	if mat <= 0:
		return
	var p: Vector3i = hit.previous_position
	if _overlaps_player(p) or p.y < 0 or p.y >= Settings.WORLD_HEIGHT_VOXELS:
		return
	if world.get_voxel(p) != GameMaterials.AIR:
		return
	voxel_placed.emit(p, mat)
	if def.has("empties_to"):
		inventory.replace_held_one(def["empties_to"])
	else:
		inventory.consume_held(1)
	_after_use()


func _after_use() -> void:
	_place_cooldown = Settings.place_repeat_s
	if viewmodel:
		viewmodel.swing()


## Static ground variants (sand_static, water_static) count as their loose material.
func _loose_name(mat: int) -> String:
	if mat <= 0:
		return "air"
	var d: Dictionary = GameMaterials.defs()[mat - 1]
	if d.has("loose"):
		return GameMaterials.name_of(int(d["loose"]))
	return d["name"]


func _overlaps_player(p: Vector3i) -> bool:
	var r := Settings.m2v(Settings.player_radius_m)
	var h := Settings.m2v(Settings.player_height_m)
	var body := AABB(position - Vector3(r, 0, r), Vector3(2 * r, h, 2 * r))
	return body.intersects(AABB(Vector3(p), Vector3.ONE))


func _drop(whole: bool) -> void:
	if inventory == null:
		return
	var stack := inventory.drop_held(whole)
	if stack.is_empty():
		return
	var from := camera.global_position - camera.global_transform.basis.z * 0.5
	var vel := -camera.global_transform.basis.z * Settings.m2v(Settings.drop_throw_speed_m) + Vector3(0, Settings.m2v(1.0), 0)
	item_dropped.emit(stack, from, vel)
	if viewmodel:
		viewmodel.swing()


# ---------------- helpers: target outline and crack box ----------------

func _make_box_outline() -> MeshInstance3D:
	var im := ImmediateMesh.new()
	var m := StandardMaterial3D.new()
	m.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	m.albedo_color = Color(0, 0, 0, 0.8)
	m.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	m.no_depth_test = false
	im.surface_begin(Mesh.PRIMITIVE_LINES, m)
	var e := 0.004
	var a := -e
	var b := 1.0 + e
	var c := [Vector3(a, a, a), Vector3(b, a, a), Vector3(b, a, b), Vector3(a, a, b), Vector3(a, b, a), Vector3(b, b, a), Vector3(b, b, b), Vector3(a, b, b)]
	for pair in [[0, 1], [1, 2], [2, 3], [3, 0], [4, 5], [5, 6], [6, 7], [7, 4], [0, 4], [1, 5], [2, 6], [3, 7]]:
		im.surface_add_vertex(c[pair[0]])
		im.surface_add_vertex(c[pair[1]])
	im.surface_end()
	var mi := MeshInstance3D.new()
	mi.mesh = im
	mi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	return mi


func _make_crack_box() -> MeshInstance3D:
	var box := BoxMesh.new()
	box.size = Vector3.ONE * 1.01
	var m := StandardMaterial3D.new()
	m.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	m.albedo_color = Color(0.05, 0.05, 0.05, 1.0)
	m.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	box.material = m
	var mi := MeshInstance3D.new()
	mi.mesh = box
	mi.position = Vector3(0.5, 0.5, 0.5)
	mi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	# wrap so global_position addresses the voxel corner
	var holder := MeshInstance3D.new()
	holder.add_child(mi)
	return holder
