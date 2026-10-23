class_name Player
extends Node3D

## First-person player (ADR 0005). World units are voxels (0.5 m); all
## tunables live in Settings in metres and are converted here. Owns the
## PlayerInventory model, the mining progress and the held-item viewmodel;
## world writes are requested through signals and done by Main.
## Moves against the voxel grid (VoxelBody, ADR 0011), not physics colliders.

signal voxel_broken(pos: Vector3i, drops: Array)
signal voxel_placed(pos: Vector3i, material: int)
signal item_dropped(stack: Dictionary, from: Vector3, velocity: Vector3)
signal inventory_toggled(open: bool)
signal block_used(pos: Vector3i, material: int)
signal pause_requested   # Esc with nothing else open: the pause menu (ADR 0032)   # right click on a block with an "interact" key (ADR 0007)
signal died
signal game_mode_changed(mode: String)
signal stain_drip(at: Vector3, colour: Color)   # a stained body drips on the ground (StainMap.drip)
signal landed(feet_voxel: Vector3i, fall_m: float)   # touched ground after a fall (farmland trampling, ADR 0022)
signal ate(item_id: String)

var world: World
var camera: Camera3D
var model: ApeModel          # the body (visible in third person)
var perspective := 0         # 0 first person, 1 third person behind, 2 third person front (F5 cycles)
var inventory: PlayerInventory
var mining: MiningTable
var viewmodel: Viewmodel
var hud: GameHud
var flying := true
var game_mode := GameMode.SURVIVAL   # rules in GameMode (ADR 0018); set with set_game_mode or /gamemode
var ui_open := false
var edits_locked := false   # no breaking or placing while the loading screen is up (ADR 0020)
var _last_jump_ms := -1000           # creative: double-tap jump toggles flying
var _yaw := 0.0
var _pitch := -0.2
var _mouse_captured := false
var autopilot := false
var autopilot_dir := Vector3(0, 0, -1)
var autopilot_speed_m := 6.0
var mined_log: Array = []   # autopilot mine=1: every break (voxel, what it reads now, feet height), first 64
var autopilot_action := ""     # "" = fly/walk forward, "mine" = stand still and hold break

# animals, farming, furniture (ADR 0022): hooks Main sets
var entity_pick: Callable        # (from, dir, max_dist) -> {mob, dist} or {}
var entity_attack: Callable      # (mob, damage) -> void
var throw_item: Callable         # (stack, from, dir) -> void: a thrown spear flies (ADR 0030)
var entity_use: Callable         # (mob, held def) -> bool (true: one held item used up)
var place_furniture: Callable    # (kind, anchor, facing) -> bool
var work_use: Callable           # (hit, first_press, from, dir) -> bool: a work piece took the right click (ADR 0023)
var work_hit: Callable           # (hit, from, dir, sneaking) -> bool: a knapping piece took the left click
var farm: Farming
var seat: Vector3 = Vector3.INF  # sitting on a chair: the seat surface (world voxels)
var _eat_t := 0.0
var _eat_id := ""
var _attack_cooldown := 0.0
const EAT_S := 1.6               # Minecraft: 32 ticks

# mining state
var _mine_target := Vector3i(1 << 30, 0, 0)
var _mine_progress := 0.0
var _mine_time := -1.0
var _break_cooldown := 0.0
var _place_cooldown := 0.0
var _highlight: MeshInstance3D
var _crack: MeshInstance3D

# movement (ADR 0011)
var velocity := Vector3.ZERO
var body: VoxelBody
var _on_floor := false
var in_liquid := false
var stains := PlayerStains.new()   # Noita stains on the body: wet, bloody, oily, toxic, burning
var _drip_timer := 0.0
var _stain_overlay: StandardMaterial3D
var _stain_overlay_on := false
const DRIP_EVERY_S := 0.35

# fall damage
var _fall_peak_y := 0.0
var _was_on_floor := false
var _drop_rng := RandomNumberGenerator.new()


func _ready() -> void:
	var r := Settings.m2v(Settings.player_radius_m)
	body = VoxelBody.new(AABB(Vector3(-r, 0.0, -r), Vector3(2.0 * r, Settings.m2v(Settings.player_height_m), 2.0 * r)),
		Settings.m2v(Settings.step_height_m))
	camera = Camera3D.new()
	camera.position = Vector3(0, Settings.m2v(Settings.eye_height_m), 0)
	camera.fov = Settings.fov
	camera.near = 0.05
	camera.far = Settings.m2v(Settings.view_distance_m) * 1.5
	add_child(camera)
	camera.make_current()
	model = ApeModel.new()
	model.name = "Model"
	model.scale = Vector3.ONE * (Settings.m2v(Settings.player_height_m) / (57.0 / ApeModel.GRID))
	model.visible = false
	add_child(model)
	var viewer := VoxelViewer.new()
	viewer.view_distance = Settings.view_distance_voxels()
	viewer.requires_collisions = Settings.terrain_mesh_collisions
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


## Survival takes flight away (and lands the player); creative allows F and
## double-tapping jump to fly.
func set_game_mode(mode: String) -> void:
	game_mode = mode
	if not GameMode.rule(mode, "can_fly"):
		flying = false
	_mine_progress = 0.0
	_mine_target = Vector3i(1 << 30, 0, 0)
	game_mode_changed.emit(mode)


## /tp: move without fall damage from the jump.
func teleport(pos: Vector3) -> void:
	position = pos
	velocity = Vector3.ZERO
	_fall_peak_y = pos.y


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
		_pitch = clampf(_pitch - event.relative.y * Settings.mouse_sensitivity * (-1.0 if Settings.invert_y else 1.0), -1.5, 1.5)
		_apply_look()
	elif event is InputEventMouseButton and event.pressed and not _mouse_captured:
		capture_mouse(true)
	elif event.is_action_pressed("release_mouse"):
		capture_mouse(false)
		pause_requested.emit()
	elif event.is_action_pressed("toggle_fly"):
		if GameMode.rule(game_mode, "can_fly"):
			flying = not flying
	elif event.is_action_pressed("jump") and GameMode.rule(game_mode, "can_fly"):
		var now := Time.get_ticks_msec()
		if now - _last_jump_ms < 300:
			flying = not flying
			_last_jump_ms = -1000
		else:
			_last_jump_ms = now
	elif event is InputEventMouseButton and event.pressed and event.button_index == MOUSE_BUTTON_MIDDLE:
		_pick_block()
	elif event is InputEventKey and event.pressed and not event.echo and event.keycode == KEY_F5:
		set_perspective((perspective + 1) % 3)
	elif event.is_action_pressed("hotbar_next"):
		inventory.scroll(1)
	elif event.is_action_pressed("hotbar_prev"):
		inventory.scroll(-1)
	elif event.is_action_pressed("drop"):
		_drop(Input.is_key_pressed(KEY_CTRL))
	elif event.is_action_pressed("action_place") and _mouse_captured:
		_use(true)
	elif event.is_action_pressed("action_break") and _mouse_captured:
		_attack()
	else:
		for i in 9:
			if event.is_action_pressed("hotbar_%d" % (i + 1)):
				inventory.select(i)
				break


func _aim() -> VoxelRaycastResult:
	return world.raycast(camera.global_position, -camera.global_transform.basis.z, Settings.m2v(Settings.reach_m))


## F5 view: 0 = first person, 1 = behind, 2 = in front (facing the player).
func set_perspective(p: int) -> void:
	perspective = p
	if model:
		model.visible = p != 0
	if viewmodel:
		viewmodel.visible = p == 0
	_apply_look()


func _apply_look() -> void:
	rotation = Vector3(0, _yaw, 0)
	var eye := Vector3(0, Settings.m2v(Settings.eye_height_m), 0)
	if perspective == 0:
		camera.position = eye
		camera.rotation = Vector3(_pitch, 0, 0)
		return
	# orbit the head at a fixed distance, pulled in when terrain is in the way
	var back := 1.0 if perspective == 1 else -1.0
	var dist := Settings.m2v(3.5)
	var offset := Basis(Vector3.RIGHT, _pitch * back) * Vector3(0, 0, dist * back)
	if world:
		var hit := world.raycast(global_position + eye, global_transform.basis * offset.normalized(), dist + 0.5)
		if hit:
			dist = maxf(0.6, hit.distance - 0.6)
			offset = offset.normalized() * dist
	camera.position = eye + offset
	camera.look_at(global_position + eye + Vector3(0, 0.1, 0))


## Floor contact from the last move (VoxelBody), like CharacterBody3D's.
func is_on_floor() -> bool:
	return _on_floor


## Spawn / streaming safety: unloaded voxels read as air, so the body does not
## move while the chunk it stands in or the one below has not streamed in
## (Minecraft freezes players in unloaded chunks the same way).
func _ground_ready() -> bool:
	if flying:
		return true
	var c := Vector3i((position / Settings.CHUNK).floor())
	return world.is_area_loaded(c) and world.is_area_loaded(c - Vector3i(0, 1, 0))


## Feet or chest in a liquid (water, lava, oil).
func _check_liquid() -> bool:
	for h in [0.2, 0.5]:
		var m := world.get_voxel(Vector3i((position + Vector3(0, Settings.m2v(Settings.player_height_m) * h, 0)).floor()))
		if m > 0:
			var d: Dictionary = GameMaterials.defs()[m - 1]
			if d.get("cell_type", "solid") == "liquid":
				return true
	return false


## One move through the voxel grid; zeroes velocity on blocked axes.
func _move(delta: float, allow_step: bool) -> void:
	var got := body.move(position, velocity, delta, world.terrain, allow_step)
	position += got
	_on_floor = body.on_floor
	if body.blocked.x > 0.0:
		velocity.x = 0.0
	if body.blocked.z > 0.0:
		velocity.z = 0.0
	if body.on_floor or body.hit_ceiling:
		velocity.y = 0.0


func _physics_process(delta: float) -> void:
	var t0 := Time.get_ticks_usec()
	_physics_step(delta)
	var m := get_parent()
	if m != null and "phys_cost" in m:
		m.phys_cost["player"] = maxf(float(m.phys_cost.get("player", 0.0)), (Time.get_ticks_usec() - t0) / 1000.0)


## Records a sub-step's cost in Main.phys_cost (DebugCapture lists it on a spike).
func _cost(name: String, t0: int) -> void:
	var m := get_parent()
	if m != null and "phys_cost" in m:
		m.phys_cost[name] = maxf(float(m.phys_cost.get(name, 0.0)), (Time.get_ticks_usec() - t0) / 1000.0)


func _physics_step(delta: float) -> void:
	var t0 := Time.get_ticks_usec()
	if model and model.visible:
		model.animate(delta, Vector2(velocity.x, velocity.z).length(), is_on_floor(), flying, _pitch, viewmodel.swing_amount() if viewmodel else 0.0)
	_cost("player_model", t0)
	_break_cooldown = maxf(0.0, _break_cooldown - delta)
	_place_cooldown = maxf(0.0, _place_cooldown - delta)
	t0 = Time.get_ticks_usec()
	_update_mining(delta)
	_cost("player_mining", t0)
	if seat != Vector3.INF:
		_sit_step()
		return
	_update_eating(delta)
	_attack_cooldown = maxf(0.0, _attack_cooldown - delta)
	t0 = Time.get_ticks_usec()
	var ground := _ground_ready()
	_cost("player_ground", t0)
	if not ground:
		velocity = Vector3.ZERO
		return
	t0 = Time.get_ticks_usec()
	_update_stains(delta)
	_cost("player_stains", t0)
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
		_move(delta, false)
		_fall_peak_y = position.y
		return
	var speed_m := Settings.sprint_speed_m if (Input.is_action_pressed("sprint") and not ui_open) else Settings.walk_speed_m
	if autopilot:
		speed_m = minf(autopilot_speed_m, Settings.sprint_speed_m)   # speed=0 stands still
	in_liquid = _check_liquid()
	var speed := Settings.m2v(speed_m) * (0.5 if in_liquid else 1.0)
	velocity.x = dir.x * speed
	velocity.z = dir.z * speed
	var jump := Input.is_action_pressed("jump") and not ui_open and not autopilot
	if in_liquid:
		# Minecraft-style swimming: drift down slowly, hold jump to rise
		var target := Settings.m2v(Settings.swim_up_speed_m) if jump else -Settings.m2v(Settings.sink_speed_m)
		velocity.y = move_toward(velocity.y, target, Settings.m2v(Settings.gravity_m) * 0.5 * delta)
	else:
		# always pull down a little, so standing still still reports floor contact
		velocity.y -= Settings.m2v(Settings.gravity_m) * delta
		if _on_floor and Input.is_action_just_pressed("jump") and not ui_open:
			velocity.y = sqrt(2.0 * Settings.m2v(Settings.gravity_m) * Settings.m2v(Settings.jump_height_m))
	t0 = Time.get_ticks_usec()
	_move(delta, _on_floor)
	_cost("player_move", t0)
	t0 = Time.get_ticks_usec()
	_unstick_from_voxels()
	_update_fall_damage()
	_cost("player_unstick", t0)
	if autopilot and autopilot_action == "place" and _place_cooldown <= 0.0:
		t0 = Time.get_ticks_usec()
		_use(true)
		_cost("player_place", t0)


## A voxel placed or simulated into the body (falling sand) would trap it:
## if the feet are inside a solid voxel, lift to its top and stop falling.
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
	return d.get("cell_type", "solid") != "liquid"


# ---------------- health ----------------

## Vanilla fall damage: 1 per block fallen beyond the safe distance.
func _update_fall_damage() -> void:
	# "landed" = floor contact, or any contact while no longer falling (standing
	# on a voxel edge in a shaft reports a wall normal, not a floor)
	var on_floor := is_on_floor() or in_liquid   # water breaks a fall, as in vanilla
	if on_floor and not _was_on_floor:
		var fall_m := Settings.v2m(_fall_peak_y - position.y)
		if not in_liquid and fall_m > 0.5:
			landed.emit(Vector3i((position + Vector3(0, -0.1, 0)).floor()), fall_m)
		var dmg := int(floor(fall_m - Settings.fall_damage_safe_m))
		if dmg > 0 and inventory and GameMode.rule(game_mode, "takes_damage"):
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
	stains.clear()
	if inventory:
		inventory.respawn()


# ---------------- stains ----------------

## Noita stains (PlayerStains): what the feet, waist and head touch coats the
## body; burning and toxic coats hurt; bloody, oily and toxic coats drip.
func _update_stains(delta: float) -> void:
	var h := Settings.m2v(Settings.player_height_m)
	var touching := []
	for f in [0.1, 0.5, 0.9]:
		var m := world.get_voxel(Vector3i((position + Vector3(0, h * f, 0)).floor()))
		if m > 0:
			touching.append(m)
	var dmg := stains.update(delta, touching)
	if dmg > 0 and inventory and GameMode.rule(game_mode, "takes_damage"):
		inventory.damage(dmg)
	_drip_timer -= delta
	if _drip_timer <= 0.0:
		_drip_timer = DRIP_EVERY_S
		var best := ""
		for k in ["bloody", "oily", "toxic"]:
			if float(stains.amount[k]) > 0.2 and (best == "" or float(stains.amount[k]) > float(stains.amount[best])):
				best = k
		if best != "" and _on_floor:
			stain_drip.emit(position, PlayerStains.COLORS[best])
	_apply_stain_tint()


## The body (third person) and the bare arm (first person) take the stains' colour.
func _apply_stain_tint() -> void:
	var c := stains.tint()
	var on := c.a > 0.01
	if on:
		if _stain_overlay == null:
			_stain_overlay = StandardMaterial3D.new()
			_stain_overlay.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
			_stain_overlay.roughness = 0.4   # a wet, glossy coat
		_stain_overlay.albedo_color = c
		var burn := float(stains.amount["burning"])
		_stain_overlay.emission_enabled = burn > 0.0
		if burn > 0.0:
			_stain_overlay.emission = PlayerStains.COLORS["burning"] * (1.2 + 0.4 * sin(Time.get_ticks_msec() * 0.02))
	if on != _stain_overlay_on:
		_stain_overlay_on = on
		var m: Material = _stain_overlay if on else null
		if model:
			for part in model.get_children():
				if part is GeometryInstance3D:
					part.material_overlay = m
		if viewmodel:
			viewmodel.set_overlay(m)


# ---------------- mining ----------------

func _update_mining(delta: float) -> void:
	var holding := false
	if autopilot:
		holding = autopilot_action == "mine"
	if edits_locked:
		holding = false
	elif not autopilot:
		holding = _mouse_captured and not ui_open and Input.is_action_pressed("action_break")
	var hit: VoxelRaycastResult = _aim() if (holding or (_mouse_captured and not ui_open and not autopilot)) else null
	if hit and aimed_mob(hit) != null:
		hit = null   # an animal is in front of the block: LMB hits it instead (_attack)
	if hit:
		_highlight.visible = true
		_highlight.global_position = Vector3(hit.position)
	else:
		_highlight.visible = false
	# keep placing while the button is held (vanilla repeats every 4 ticks)
	if not autopilot and not ui_open and _mouse_captured and Input.is_action_pressed("action_place") and _place_cooldown <= 0.0:
		_use(false)
	# knapping (ADR 0023): a left click chips a cell instead of mining; holding repeats
	if holding and hit and _break_cooldown <= 0.0 and work_hit.is_valid():
		if work_hit.call(hit, camera.global_position, -camera.global_transform.basis.z, Input.is_action_pressed("sprint")):
			_break_cooldown = Settings.work_repeat_s
			_reset_mining()
			if viewmodel:
				viewmodel.swing()
			return
	if not holding or hit == null or _break_cooldown > 0.0:
		_reset_mining()
		return
	var p: Vector3i = hit.position
	if p != _mine_target:
		_mine_target = p
		_mine_progress = 0.0
		var mat_name := GameMaterials.name_of(world.get_voxel(p))
		if GameMode.rule(game_mode, "instant_break"):
			_mine_time = 0.0 if world.get_voxel(p) > 0 else -1.0   # creative breaks anything, at once
		else:
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
		var drops := mining.roll_drops(mat_name, inventory.held_def(), _drop_rng) if mining and GameMode.rule(game_mode, "drops") else []
		voxel_broken.emit(p, drops)
		if viewmodel:
			viewmodel.swing()
		_break_cooldown = Settings.break_cooldown_s
		if autopilot and autopilot_action == "mine":
			# into the run's JSON, not stdout: one console print costs 3-15 ms on Windows and
			# showed up as the worst frames of every mining benchmark
			if mined_log.size() < 64:
				mined_log.append({"voxel": p, "now": GameMaterials.name_of(world.get_voxel(p)), "player_y": snappedf(position.y, 0.01), "on_floor": is_on_floor()})
			# dig a 2x2 shaft: the player stands on the corner where four voxels
			# meet and turns a quarter after every break (the body is 1.2 voxels wide)
			_yaw += PI / 2.0
			_apply_look()
		_reset_mining()
	elif viewmodel and fmod(_mine_progress * 4.0, 1.0) < delta * 4.0:
		viewmodel.swing()


## The voxel being mined (the crack is showing), else x = 1 << 30 (particles, ADR 0026).
func mining_target() -> Vector3i:
	return _mine_target if _mine_progress > 0.0 and _mine_time > 0.0 else Vector3i(1 << 30, 0, 0)


## The item being eaten, "" when not eating.
func eating_item() -> String:
	return _eat_id


func _reset_mining() -> void:
	_mine_target = Vector3i(1 << 30, 0, 0)
	_mine_progress = 0.0
	_crack.visible = false
	if hud:
		hud.break_progress = 0.0


# ---------------- using the held item (right click) ----------------

func _use(first_press: bool) -> void:
	if inventory == null or edits_locked:
		return
	var hit := _aim()
	if first_press and inventory.held_def().has("throw") and throw_item.is_valid():
		_throw()
		return
	if first_press:
		var mob = aimed_mob(hit)
		if mob != null:
			if entity_use.is_valid() and entity_use.call(mob, inventory.held_def()):
				if GameMode.rule(game_mode, "consumes_items") and not ("tool" in inventory.held_def().get("tags", [])):
					inventory.consume_held(1)
				_after_use()
			return
	if hit == null:
		_start_eating(first_press)
		return
	if not first_press and _place_cooldown > 0.0:
		return
	var target: Vector3i = hit.position
	# Vintage Story work pieces (ADR 0023): knapping, clay forming, pit kilns, firepits, sieves
	if work_use.is_valid() and work_use.call(hit, first_press, camera.global_position, -camera.global_transform.basis.z):
		_after_use()
		return
	# stations open on right click; sneaking (Shift) places against them instead (vanilla)
	if first_press and not Input.is_action_pressed("sprint"):
		var tm := world.get_voxel(target)
		if tm > 0 and GameMaterials.defs()[tm - 1].has("interact"):
			block_used.emit(target, tm)
			return
	var def := inventory.held_def()
	if def.is_empty():
		return
	# farming (ADR 0022): hoes till, seeds and carrots plant on a farmland top face
	if "hoe" in def.get("tags", []) and farm != null:
		if hit.previous_position.y >= target.y and farm.till(target):
			_after_use()
		return
	if def.has("plants") and farm != null and farm.is_farmland_material(farm.get_voxel.call(target)):   # Main's view: the bubble is ahead of the terrain
		if hit.previous_position == target + Vector3i.UP and farm.plant(target, def["plants"]):
			if GameMode.rule(game_mode, "consumes_items"):
				inventory.consume_held(1)
			_after_use()
		return
	if def.has("furniture"):
		_place_furniture(hit, def)
		return
	if def.has("food"):
		_start_eating(first_press)
		return
	var target_name := _loose_name(world.get_voxel(target))
	# container pickup (bucket on water / lava)
	if def.has("fills") and def["fills"].has(target_name):
		voxel_placed.emit(target, GameMaterials.AIR)
		if GameMode.rule(game_mode, "consumes_items"):
			inventory.replace_held_one(def["fills"][target_name])
		_after_use()
		return
	if not def.has("material"):
		return
	var mat := ItemIcon.material_id_for(inventory.items, def["id"])
	if mat <= 0:
		return
	var p: Vector3i = hit.previous_position
	if _replaceable(world.get_voxel(target)):
		p = target   # Minecraft places into tall grass instead of in front of it
	if _overlaps_player(p) or p.y < 0 or p.y >= Settings.WORLD_HEIGHT_VOXELS:
		return
	if world.get_voxel(p) != GameMaterials.AIR and not _replaceable(world.get_voxel(p)):
		return
	if GameMaterials.defs()[mat - 1].get("floor_only", false):
		var top_face := p == target or hit.previous_position == target + Vector3i.UP
		if not top_face or not MobManager.is_solid(world.get_voxel(p - Vector3i.UP)):
			return   # torches stand on a floor, not on a wall (ADR 0030)
	voxel_placed.emit(p, mat)
	if not GameMode.rule(game_mode, "consumes_items"):
		pass   # creative: blocks and buckets never run out
	elif def.has("empties_to"):
		inventory.replace_held_one(def["empties_to"])
	else:
		inventory.consume_held(1)
	_after_use()


func _replaceable(m: int) -> bool:
	return m > 0 and GameMaterials.defs()[m - 1].get("replaceable", false)


## An animal under the crosshair closer than the aimed voxel (hit may be null), or null.
func aimed_mob(hit: VoxelRaycastResult = null):
	if not entity_pick.is_valid():
		return null
	var r: Dictionary = entity_pick.call(camera.global_position, -camera.global_transform.basis.z, Settings.m2v(Settings.reach_m))
	if r.is_empty():
		return null
	if hit != null and float(hit.distance) < float(r["dist"]):
		return null
	return r["mob"]


## Left click: hit the animal in front (Minecraft: hand 1, tools 2 + tier).
func _attack() -> void:
	if _attack_cooldown > 0.0 or edits_locked or ui_open or not entity_attack.is_valid():
		return
	var mob = aimed_mob(_aim())
	if mob == null:
		return
	var def := inventory.held_def() if inventory else {}
	var dmg := 1
	if def.has("attack_damage"):
		dmg = int(def["attack_damage"])   # weapons (the flint spear, ADR 0030)
	elif "tool" in def.get("tags", []):
		dmg = 2 + int(def.get("tier", 0))
	entity_attack.call(mob, dmg)
	_attack_cooldown = 0.25
	if viewmodel:
		viewmodel.swing()


## Right click with a throwable (the spear): it leaves the hand and flies
## along the look; Main moves it and hits animals (creative keeps it).
func _throw() -> void:
	var def := inventory.held_def()
	var stack := {"id": def["id"], "count": 1}
	if GameMode.rule(game_mode, "consumes_items"):
		stack = inventory.inv.take(inventory.selected, 1)
	var dir := -camera.global_transform.basis.z
	throw_item.call(stack, camera.global_position + dir * 0.6, dir)
	_attack_cooldown = 0.5
	if viewmodel:
		viewmodel.swing()


## Beds and chairs: the clicked face's neighbour is the anchor; the footprint must be free.
func _place_furniture(hit: VoxelRaycastResult, def: Dictionary) -> void:
	if not place_furniture.is_valid():
		return
	var kind: String = def["furniture"]
	var anchor: Vector3i = hit.position if _replaceable(world.get_voxel(hit.position)) else hit.previous_position
	var look := -camera.global_transform.basis.z
	var facing := FurnitureEntity.facing_for(kind, look)
	for c in FurnitureEntity.footprint(kind, anchor, facing):
		var m := world.get_voxel(c)
		if (m != GameMaterials.AIR and not _replaceable(m)) or _overlaps_player(c):
			return
	if place_furniture.call(kind, anchor, facing):
		if GameMode.rule(game_mode, "consumes_items"):
			inventory.consume_held(1)
		_after_use()


# ---------------- eating ----------------

## Holding right click with food eats it after EAT_S (only when hurt: no hunger yet).
func _start_eating(first_press: bool) -> void:
	var def := inventory.held_def()
	if not def.has("food") or not first_press:
		return
	if inventory.health >= inventory.max_health:
		return
	_eat_id = def["id"]
	_eat_t = 0.0


func _update_eating(delta: float) -> void:
	if _eat_id == "":
		return
	var def := inventory.held_def()
	if ui_open or not (autopilot or Input.is_action_pressed("action_place")) or def.get("id", "") != _eat_id:
		_eat_id = ""
		return
	_eat_t += delta
	if viewmodel and fmod(_eat_t, 0.25) < delta:
		viewmodel.swing()
	if _eat_t >= EAT_S:
		inventory.heal(int(def["food"].get("hunger", 1)))
		if GameMode.rule(game_mode, "consumes_items"):
			if def.has("empties_to"):
				inventory.replace_held_one(def["empties_to"])   # stew leaves its bowl
			else:
				inventory.consume_held(1)
		ate.emit(_eat_id)
		_eat_id = ""


# ---------------- sitting ----------------

## Sit on a chair: hips on the seat, looking out of the chair's front.
func sit(seat_surface: Vector3, chair_yaw: float) -> void:
	seat = seat_surface
	velocity = Vector3.ZERO
	flying = false
	set_look(chair_yaw + PI, _pitch)   # the player looks along -Z of its yaw


func stand_up() -> void:
	if seat == Vector3.INF:
		return
	var s := seat
	seat = Vector3.INF
	# step off in front of the seat if there is room, else onto it
	var front := s + Vector3(-sin(_yaw), 0, -cos(_yaw))
	var fp := Vector3i(int(floor(front.x)), int(floor(s.y)) - 1, int(floor(front.z)))
	if world.get_voxel(fp) == GameMaterials.AIR and world.get_voxel(fp + Vector3i.UP) == GameMaterials.AIR and world.get_voxel(fp + Vector3i.DOWN) != GameMaterials.AIR:
		position = Vector3(front.x, fp.y + 0.05, front.z)
	else:
		position = Vector3(s.x, s.y + 0.05, s.z)
	_fall_peak_y = position.y
	if model:
		model.leg_r.rotation = Vector3.ZERO
		model.leg_l.rotation = Vector3.ZERO


func _sit_step() -> void:
	# hips (the ape's are at 24/57 of its height) on the seat
	position = seat - Vector3(0, Settings.m2v(Settings.player_height_m) * 24.0 / 57.0, 0)
	velocity = Vector3.ZERO
	_fall_peak_y = position.y
	if model and model.visible:
		model.leg_r.rotation = Vector3(-PI * 0.5, 0, 0.05)
		model.leg_l.rotation = Vector3(-PI * 0.5, 0, -0.05)
	if not ui_open and not autopilot and (Input.is_action_just_pressed("jump") or Input.is_action_just_pressed("descend")):
		stand_up()


## Middle click (vanilla pick block): the item that places the aimed voxel.
## Creative conjures it; survival only switches to it if it is carried.
func _pick_block() -> void:
	if inventory == null or not _mouse_captured:
		return
	var hit := _aim()
	if hit == null:
		return
	var id := inventory.items.item_for_material(_loose_name(world.get_voxel(hit.position)))
	if id != "":
		inventory.pick_block(id, GameMode.rule(game_mode, "item_picker"))


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
