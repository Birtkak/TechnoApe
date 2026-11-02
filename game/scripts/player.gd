class_name Player
extends Node3D

## First-person player (ADR 0005). World units are 12.5 cm voxels (ADR 0033); all
## tunables live in Settings in metres and are converted here. Owns the
## PlayerInventory model, the mining progress and the held-item viewmodel;
## world writes are requested through signals and done by Main.
## Moves against the voxel grid (VoxelBody, ADR 0011), not physics colliders.

signal voxel_broken(pos: Vector3i, drops: Array)
signal voxel_placed(pos: Vector3i, material: int)
signal voxels_dug(cells: Array, drops: Array, center: Vector3)   # a brush dig (ADR 0034)
signal voxels_placed(cells: Array, material: int)                # a brush placement of a voxel item
signal brush_filled(box: AABB, material: int)
signal big_dug(box: AABB, material: int)   # a big creative brush dug natively: the box it took from (no drops: world editing)                    # a big brush filled natively: the box it may have touched (ADR 0065)
signal item_dropped(stack: Dictionary, from: Vector3, velocity: Vector3)
signal inventory_toggled(open: bool)
signal block_used(pos: Vector3i, material: int)
signal pause_requested   # Esc with nothing else open: the pause menu (ADR 0032)   # right click on a block with an "interact" key (ADR 0007)
signal died
signal moved_far           # a teleport or respawn that needs a cover (_needs_cover): Main covers it until the ground is in
signal game_mode_changed(mode: String)
signal stain_drip(at: Vector3, colour: Color)   # a stained body drips on the ground (StainMap.drip)
signal landed(feet_voxel: Vector3i, fall_m: float)   # touched ground after a fall (farmland trampling, ADR 0022)
signal ate(item_id: String)
signal map_requested    # M: the full map (plan item D10)
signal lookup_requested(ids: Array, mode: String)   # R / U in the world: the aimed block's item, then the held one (ADR 0093)

var world: World
var camera: Camera3D
var model: ApeModel          # the body (visible in third person)
var perspective := 0         # 0 first person, 1 third person behind, 2 third person front (F5 cycles)
var inventory: PlayerInventory
var mining: MiningTable
var viewmodel: Viewmodel
var hud: GameHud
var flying := true
var ride := {}   # riding a minecart (ADR 0100): {rail: Vector3i, dir: Vector3i (a block step), t: 0..1 along it}
var noclip := false                  # flight passes through voxels (spectator, or /noclip while flying)
var game_mode := GameMode.SURVIVAL   # rules in GameMode (ADR 0018); set with set_game_mode or /gamemode
var ui_open := false
var edits_locked := false   # no breaking or placing while the loading screen is up (ADR 0020)
const FAR_MOVE_M := 24.0   # past the full-detail radius, the ground there may not exist yet
const HOP_M := 100.0       # a jump shorter than this onto terrain already drawn gets no cover (ADR 0066)
var _last_jump_ms := -1000           # creative: double-tap jump toggles flying
var _yaw := 0.0
var _pitch := -0.2
var _jet_thrust := false            # the jetpack fired this step (the body shows its exhaust)
var _eye_drop := 0.0                 # voxels the eyes sit lower while sneaking
var _mouse_captured := false
var autopilot := false
var free_cam := false        # a DebugCamera has the view: no moving, looking or mining from input
var building := false        # the ShapeBuilder (ADR 0038) owns the mouse buttons: no breaking or placing
var input_grab := false      # the build menu (V) has the mouse: no looking either
var survival: Survival = null   # hunger and body temperature (ADR 0040), set by Main
var speed_mult := 1.0        # Survival slows a shivering body
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
var furniture_is: Callable        # (voxel, "climb" | "use") -> bool: a ladder covers it, a click there does something (ADR 0094)
var work_use: Callable           # (hit, first_press, from, dir, sneaking) -> bool: a work piece took the right click (ADR 0023)
var item_use: Callable           # (hit, first_press, from, dir) -> bool: the Stone Age took it, before anything else (ADR 0059)
var work_hit: Callable           # (hit, from, dir, sneaking) -> bool: a knapping piece took the left click
var work_ghost: Callable         # (hit, sneaking) -> {anchor, size, free} or {}: where the held work item's piece goes
var hold_key: Callable           # (hit) -> String: what a held right click works on ("" = an instant use) (ADR 0043)
var hold_done: Callable          # (hit) -> void: right click was held Settings.hold_use_s on it
var autopilot_hold := false      # scripted checks: the right button is held
var autopilot_sneak := false     # scripted checks: Shift is held
var _hold_at := ""
var _hold_t := 0.0
var describe_block: Callable     # (voxel) -> {title?, line?}: a block entity's word for the look-at panel
var fire_at: Callable            # (voxel) -> bool: a lit firepit, forge or bloomery covers it (it burns, ADR 0078)
var playtest_laser: Callable     # (firing, from, dir, delta, brush_size) -> void: a held laser cutter (PlaytestTools)
var playtest_use: Callable       # (def, hit, from, dir, sneaking) -> bool: a right click with a playtest gadget
var ignite_at: Callable          # (hit) -> bool: flint and steel sets the aimed face on fire (Weather, ADR 0056)
var _lasering := false
var _look_key := ""              # what the look-at panel shows now (rebuilt only when it changes)
var _look_refresh := 0
var look_item := ""               # the look-at panel's item ("" for none): what R / U look up (ADR 0093)
var farm: Farming
var seat: Vector3 = Vector3.INF  # sitting on a chair: the seat surface (world voxels)
var _eat_t := 0.0
var _eat_id := ""
var _attack_cooldown := 0.0
const EAT_S := 1.6               # Minecraft: 32 ticks

# mining state
# Brush (ADR 0034, 0065): size in voxels (1, 2, 4, 8, creative also 16, 32; Ctrl + wheel) and shape
# (B / Shift + B: cube, sphere, cylinder, flat, column); the survival dig size is capped by the held
# tool (MiningTable.dig_cap). BrushTool draws the shape's shell and runs the big brushes natively.
var brush_size := 4
var brush_shape := "cube"
var brush_tool := BrushTool.new()
var _dig_normal := Vector3i.UP         # the aimed face's direction when the current dig started
var _brush_info := ""
var spectator_speed_mul := 1.0   # wheel in spectator scales the fly speed (2026-10-26)
const SPECTATOR_SPEED_STEP := 1.25
const SPECTATOR_SPEED_RANGE := Vector2(0.1, 20.0)
var script_break_s := 0.0              # /brush hold: the break button is held this long (scripted checks)
var _dig_cells: Array[Vector3i] = []   # the voxels the current dig removes
var _dig_carry := {}                   # fractional drops carried between digs (MiningTable.voxel_drops)
var _mine_target := Vector3i(1 << 30, 0, 0)
var _mine_progress := 0.0
var _mine_time := -1.0
var _break_cooldown := 0.0
var _place_cooldown := 0.0
var _highlight: MeshInstance3D
var _highlight_mat: StandardMaterial3D   # the outline's lines, depth-tested
var _highlight_hidden_mat: StandardMaterial3D   # a dig's buried lines, faint through the terrain: all at full strength the box read as standing on the ground
var _station_ghost: MeshInstance3D       # the held station's own model where a click puts it
var script_aim := false                  # /use aim: the aim and its ghost show under autopilot too (evidence runs)
var _crack: Node3D
var crack_material: ShaderMaterial   # MaterialWarmup draws it under the loading screen
var _swing_t := 0.0                  # mining: time to the next arm swing
const SWING_EVERY_S := 0.25          # the arm keeps swinging while a block is being broken (Minecraft)

# movement (ADR 0011)
var velocity := Vector3.ZERO
var _knock := Vector3.ZERO        # sideways push from a blast (Tnt), added to walking and dying away
var body: VoxelBody
var _on_floor := false
var in_liquid := false
var air := 1.0                    # breath, 0..1 (Settings.air_s); drowning once it is gone
var _drown_t := 0.0
var _wall_contact := false        # the last move was blocked sideways (climbing out of water)
var stains := PlayerStains.new()   # Noita stains on the body: wet, bloody, oily, toxic, burning
var _drip_timer := 0.0
var _stain_overlay: StandardMaterial3D        # made in _ready: building a material's shader mid-game cost a 20 ms frame
var _stain_overlay_burn: StandardMaterial3D   # the same with emission (burning)
var _stain_overlay_on: Material = null
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
	get_tree().root.call_deferred("add_child", brush_tool.preview)
	_station_ghost = MeshInstance3D.new()
	_station_ghost.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	_station_ghost.visible = false
	get_tree().root.call_deferred("add_child", _station_ghost)
	_stain_overlay = _make_stain_overlay(false)
	_stain_overlay_burn = _make_stain_overlay(true)
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
	if atlas_material is ShaderMaterial:
		brush_tool.set_atlas(atlas_material)
	camera.add_child(viewmodel)
	inventory.selection_changed.connect(func(_i: int) -> void: viewmodel.show_item(inventory.held()))
	inventory.changed.connect(func() -> void:
		viewmodel.show_item(inventory.held())
		viewmodel.show_offhand(inventory.offhand()))


## Survival takes flight away (and lands the player); creative allows F and
## double-tapping jump to fly.
func set_game_mode(mode: String) -> void:
	game_mode = mode
	if not GameMode.rule(mode, "can_fly"):
		flying = false
	noclip = GameMode.rule(mode, "noclip")
	if GameMode.rule(mode, "always_fly"):
		flying = true
	if model:
		model.visible = model.visible and not noclip   # a spectator has no body to see
	if viewmodel:
		viewmodel.visible = perspective == 0 and GameMode.rule(mode, "interacts")   # nor hands; none in third person
	_mine_progress = 0.0
	_mine_target = Vector3i(1 << 30, 0, 0)
	game_mode_changed.emit(mode)


## /tp: move without fall damage from the jump.
func teleport(pos: Vector3) -> void:
	var far := _needs_cover(pos)
	position = pos
	velocity = Vector3.ZERO
	_fall_peak_y = pos.y
	if far:
		moved_far.emit()


## A jump to `pos` needs the loading cover: it leaves the full-detail radius and either goes further
## than a hop or lands where nothing is drawn yet. A hop onto drawn terrain is instant: the coarse
## meshes there sharpen in place and _ground_ready holds the body until its voxels are in (ADR 0066).
func _needs_cover(pos: Vector3) -> bool:
	var d := position.distance_to(pos)
	if d <= Settings.m2v(FAR_MOVE_M):
		return false
	return d > Settings.m2v(HOP_M) or world == null or not world.is_view_ready(pos)


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
	if autopilot or free_cam or input_grab:
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
	elif event.is_action_pressed("map"):
		map_requested.emit()
	elif event.is_action_pressed("swap_hands"):
		inventory.swap_hands()   # F: main hand <-> offhand (Minecraft; flight toggles with a double jump)
	elif event.is_action_pressed("jump") and GameMode.rule(game_mode, "can_fly") and not GameMode.rule(game_mode, "always_fly"):
		var now := Time.get_ticks_msec()
		if now - _last_jump_ms < 300:
			flying = not flying
			_last_jump_ms = -1000
		else:
			_last_jump_ms = now
	elif event is InputEventMouseButton and event.pressed and game_mode == GameMode.SPECTATOR and event.button_index in [MOUSE_BUTTON_WHEEL_UP, MOUSE_BUTTON_WHEEL_DOWN]:
		step_spectator_speed(1 if event.button_index == MOUSE_BUTTON_WHEEL_UP else -1)
	elif event is InputEventMouseButton and event.pressed and event.ctrl_pressed and event.button_index in [MOUSE_BUTTON_WHEEL_UP, MOUSE_BUTTON_WHEEL_DOWN]:
		step_brush_size(1 if event.button_index == MOUSE_BUTTON_WHEEL_UP else -1)
	elif event is InputEventKey and event.pressed and not event.echo and event.keycode in [KEY_R, KEY_U] and not building:
		lookup_requested.emit([look_item, String(inventory.held().get("id", ""))], "recipes" if event.keycode == KEY_R else "uses")
	elif event is InputEventKey and event.pressed and not event.echo and event.keycode == KEY_B:
		var n := Brush.SHAPES.size()
		set_brush(brush_size, Brush.SHAPES[(Brush.SHAPES.find(brush_shape) + (n - 1 if event.shift_pressed else 1)) % n])
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
	elif event.is_action_pressed("action_place") and _mouse_captured and not building:
		_use(true)
	elif event.is_action_pressed("action_break") and _mouse_captured and not building:
		_attack()
	else:
		for i in 9:
			if event.is_action_pressed("hotbar_%d" % (i + 1)):
				inventory.select(i)
				break


func set_brush(size: int, shape: String) -> void:
	brush_size = size
	brush_shape = shape
	_reset_mining()
	if hud:
		var capped := dig_size() < brush_size
		hud.show_toast("Brush: %s%s" % [Brush.describe(brush_size, shape), " (this tool digs %s)" % Brush.describe(dig_size(), shape) if capped else ""])
	_update_brush_info()


## Wheel in spectator: fly faster (up) or slower (down).
func step_spectator_speed(dir: int) -> void:
	spectator_speed_mul = clampf(spectator_speed_mul * pow(SPECTATOR_SPEED_STEP, dir), SPECTATOR_SPEED_RANGE.x, SPECTATOR_SPEED_RANGE.y)
	if hud:
		hud.show_toast("Fly speed: %.0f m/s (x%.2f)" % [Settings.fly_speed_m * spectator_speed_mul, spectator_speed_mul])


## Ctrl + wheel: the next brush size up or down (creative has 16 and 32 too).
func step_brush_size(dir: int) -> void:
	var sizes := Brush.sizes_for(GameMode.rule(game_mode, "instant_break"))
	var i := sizes.find(brush_size)
	if i < 0:
		i = sizes.size() - 1
		while i > 0 and sizes[i] > brush_size:
			i -= 1
	set_brush(sizes[clampi(i + dir, 0, sizes.size() - 1)], brush_shape)


## The dig brush's size with the held item: survival tools cap it by tier, creative digs the whole brush.
func dig_size() -> int:
	if GameMode.rule(game_mode, "instant_break"):
		return brush_size
	return mini(mini(brush_size, Brush.NATIVE_ABOVE), MiningTable.dig_cap(inventory.held_def() if inventory else {}))


## The placement brush's size: survival places up to the biggest survival brush (and what the stack holds).
func place_size() -> int:
	return brush_size if not GameMode.rule(game_mode, "consumes_items") else mini(brush_size, Brush.NATIVE_ABOVE)


## The HUD line right of the hotbar: the brush and its keys.
func _update_brush_info() -> void:
	if hud == null:
		return
	var text := ""
	if GameMode.rule(game_mode, "interacts"):
		text = "Brush %s  [C: mine ring, V: build ring, G: tools]" % Brush.describe(dig_size(), brush_shape)
		if dig_size() < brush_size:
			text += "\n(tool caps it; set %s)" % Brush.describe(brush_size, brush_shape)
		elif GameMode.rule(game_mode, "instant_break"):
			text += "\nShift+right click paints it"
	if text != _brush_info:
		_brush_info = text
		hud.set_brush_info(text)


func _aim() -> VoxelRaycastResult:
	return world.raycast(camera.global_position, -camera.global_transform.basis.z, Settings.m2v(reach()))


## How far the hand reaches: creative world editing reaches further (big brushes, ADR 0065).
func reach() -> float:
	return Settings.creative_reach_m if GameMode.rule(game_mode, "instant_break") else Settings.reach_m


## F5 view: 0 = first person, 1 = behind, 2 = in front (facing the player).
func set_perspective(p: int) -> void:
	perspective = p
	if model:
		model.visible = p != 0
	if viewmodel:
		viewmodel.visible = p == 0 and GameMode.rule(game_mode, "interacts")
	_apply_look()


func _apply_look() -> void:
	rotation = Vector3(0, _yaw, 0)
	var eye := Vector3(0, Settings.m2v(Settings.eye_height_m) - _eye_drop, 0)
	if perspective == 0:
		camera.position = eye
		camera.rotation = Vector3(_pitch, 0, 0)
		return
	# orbit the head at a fixed distance, pulled in when terrain is in the way
	var back := 1.0 if perspective == 1 else -1.0
	var dist := Settings.m2v(3.5)
	var offset := Basis(Vector3.RIGHT, _pitch * back) * Vector3(0, 0, dist * back)
	if world:
		var hit := world.raycast(global_position + eye, global_transform.basis * offset.normalized(), dist + 0.5, VoxelBody.MASK_SOLID)
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


## Feet or chest in a liquid (water, lava, oil), or in kelp standing in water (ADR 0047).
func _check_liquid() -> bool:
	# every voxel from knee to waist, not two samples: finite water churns with air gaps, and a
	# sample in a gap dropped the swimmer to the bottom (bug report 2026-10-28_08-22-07)
	var h := Settings.m2v(Settings.player_height_m)
	var feet := Vector3i(position.floor())
	for dy in range(int(h * 0.2), int(h * 0.5) + 1):
		var m := world.get_voxel(feet + Vector3i(0, dy, 0))
		if GameMaterials.is_liquid(m) or GameMaterials.is_waterlogged(m):
			return true
	return false


## A ladder's voxel touches the body's sides (one voxel out) at the feet or the waist.
func _on_ladder() -> bool:
	if not furniture_is.is_valid():
		return false
	var r := Settings.m2v(Settings.player_radius_m) + 0.5
	for h in [0.5, Settings.m2v(Settings.player_height_m) * 0.5]:
		for d in [Vector3(r, h, 0), Vector3(-r, h, 0), Vector3(0, h, r), Vector3(0, h, -r)]:
			if furniture_is.call(Vector3i((position + d).floor()), "climb"):
				return true
	return false


## One move through the voxel grid; zeroes velocity on blocked axes.
func _move(delta: float, allow_step: bool) -> void:
	var got := body.move(position, velocity, delta, world.terrain, allow_step)
	position += got
	_on_floor = body.on_floor
	_wall_contact = body.blocked.x > 0.0 or body.blocked.z > 0.0
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
		model.animate(delta, Vector2(velocity.x, velocity.z).length(), is_on_floor(), flying, _pitch, viewmodel.swing_amount() if viewmodel else 0.0, sneaking())
		model.jet_flame(_jet_thrust, Time.get_ticks_msec() / 1000.0)
	_jet_thrust = false
	_cost("player_model", t0)
	_break_cooldown = maxf(0.0, _break_cooldown - delta)
	_place_cooldown = maxf(0.0, _place_cooldown - delta)
	if not ride.is_empty():
		_ride(delta)
		return
	t0 = Time.get_ticks_usec()
	_update_mining(delta)
	_brush_jobs_done()
	_update_hold(delta)
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
	elif not ui_open and not free_cam:
		input = Input.get_vector("move_left", "move_right", "move_forward", "move_back")
		up_down = (1.0 if Input.is_action_pressed("jump") else 0.0) - (1.0 if Input.is_action_pressed("descend") else 0.0)
	var dir := (transform.basis * Vector3(input.x, 0, input.y)).normalized()
	if flying:
		if autopilot and autopilot_speed_m <= 0.0:
			velocity = Vector3.ZERO
			return
		var speed_mul := spectator_speed_mul if game_mode == GameMode.SPECTATOR else 1.0
		var speed := Settings.m2v(autopilot_speed_m if autopilot else Settings.fly_speed_m * speed_mul)
		velocity = dir * speed + _knock
		velocity.y = up_down * speed + _knock.y
		_knock = _knock.move_toward(Vector3.ZERO, Settings.m2v(30.0) * delta)
		if noclip:
			position += velocity * delta   # through rock and trees: no collision at all
			_on_floor = false
		else:
			_move(delta, false)
		_fall_peak_y = position.y
		return
	var sneak := sneaking()
	var drop := move_toward(_eye_drop, Settings.m2v(Settings.sneak_eye_drop_m) if sneak else 0.0, Settings.m2v(2.0) * delta)
	if drop != _eye_drop:
		_eye_drop = drop
		_apply_look()
	var speed_m := Settings.walk_speed_m
	if sneak:
		speed_m = Settings.sneak_speed_m
	elif Input.is_action_pressed("sprint") and not ui_open:
		speed_m = Settings.sprint_speed_m
	if autopilot:
		speed_m = minf(autopilot_speed_m, Settings.sprint_speed_m)   # speed=0 stands still
	in_liquid = _check_liquid()
	var speed := Settings.m2v(speed_m) * (0.5 if in_liquid else 1.0) * speed_mult
	velocity.x = dir.x * speed + _knock.x
	velocity.z = dir.z * speed + _knock.z
	_knock = _knock.move_toward(Vector3.ZERO, Settings.m2v(30.0 if _on_floor else 6.0) * delta)
	var jump := Input.is_action_pressed("jump") and not ui_open and not autopilot and not free_cam
	if in_liquid:
		# Minecraft-style swimming: drift down slowly, hold jump to rise
		var target := Settings.m2v(Settings.swim_up_speed_m) if jump else -Settings.m2v(Settings.sink_speed_m)
		velocity.y = move_toward(velocity.y, target, Settings.m2v(Settings.gravity_m) * 0.5 * delta)
		# swimming against a bank with jump held: hop out onto it (Minecraft's water jump)
		var hop := sqrt(2.0 * Settings.m2v(Settings.gravity_m) * Settings.m2v(Settings.jump_height_m))
		if jump and _wall_contact and dir != Vector3.ZERO and velocity.y < hop * 0.5:
			velocity.y = hop
	else:
		# always pull down a little, so standing still still reports floor contact
		velocity.y -= Settings.m2v(Settings.gravity_m) * delta
		if _on_floor and Input.is_action_just_pressed("jump") and not ui_open:
			velocity.y = sqrt(2.0 * Settings.m2v(Settings.gravity_m) * Settings.m2v(Settings.jump_height_m))
		var pack := _worn_with("jetpack")   # the steam jetpack (ADR 0099): on compressed air, hold jump to rise slowly, let go to hover down
		if not pack.is_empty() and float(pack.get("air", 0.0)) > 0.0:
			pack["air"] = maxf(0.0, float(pack["air"]) - delta / Settings.jetpack_air_s * (1.0 if jump else 0.25))
			_jet_thrust = jump
			if jump:
				velocity.y = move_toward(velocity.y, Settings.m2v(Settings.jetpack_rise_m), Settings.m2v(Settings.gravity_m) * 2.0 * delta)
			else:
				velocity.y = maxf(velocity.y, -Settings.m2v(Settings.jetpack_sink_m))
			_fall_peak_y = position.y
		if _on_ladder():
			# Minecraft ladders: walk into one or hold jump to climb, sneak to hang, else slide down slowly
			var climb := Settings.m2v(Settings.climb_speed_m)
			if jump or (dir != Vector3.ZERO and _wall_contact):
				velocity.y = climb
			elif sneak:
				velocity.y = 0.0
			else:
				velocity.y = maxf(velocity.y, -climb)
			_fall_peak_y = position.y
	if sneak and _on_floor and not in_liquid:
		_hold_edge(delta)   # sneaking never walks off a ledge (Minecraft)
	t0 = Time.get_ticks_usec()
	_move(delta, _on_floor or in_liquid)   # in water the body also steps up out of the shallows
	_cost("player_move", t0)
	_update_air(delta)
	t0 = Time.get_ticks_usec()
	_unstick_from_voxels()
	_update_fall_damage()
	_cost("player_unstick", t0)
	if autopilot and autopilot_action == "place" and _place_cooldown <= 0.0:
		t0 = Time.get_ticks_usec()
		_use(true)
		_cost("player_place", t0)


## A blast's push (voxels per second): up at once, sideways through _knock so walking keeps it.
func knock(v: Vector3) -> void:
	if noclip:
		return
	_knock += Vector3(v.x, v.y if flying else 0.0, v.z)
	if not flying:
		velocity.y = maxf(velocity.y, v.y)
		_on_floor = false


## Shift (Minecraft's sneak; Ctrl sprints since bug report 2026-10-25): slower, lower eyes, no
## falling off edges, and a right click places against stations instead of opening them. Flying
## Shift descends instead.
func sneaking() -> bool:
	if autopilot:
		return autopilot_sneak
	return not flying and not ui_open and not free_cam and Input.is_action_pressed("sneak")


## Sneaking on the ground: an axis whose move would leave no ground under any corner of the body
## (within a step's depth) is stopped, so the body stays on the ledge.
func _hold_edge(delta: float) -> void:
	for axis in [0, 2]:
		if velocity[axis] == 0.0:
			continue
		var next := position
		next[axis] += velocity[axis] * delta
		if not _ground_under(next):
			velocity[axis] = 0.0


func _ground_under(at: Vector3) -> bool:
	var r := Settings.m2v(Settings.player_radius_m) * 0.9
	var feet := int(floor(at.y + 0.05))
	var depth := int(Settings.m2v(Settings.step_height_m)) + 1
	for c in [Vector2(-r, -r), Vector2(r, -r), Vector2(-r, r), Vector2(r, r)]:
		var x := int(floor(at.x + c.x))
		var z := int(floor(at.z + c.y))
		for d in range(1, depth + 1):
			if _is_solid_voxel(Vector3i(x, feet - d, z)):
				return true
	return false


## A voxel placed or simulated into the body (falling sand) would trap it:
## if the feet are inside a solid voxel, lift to its top and stop falling.
func _unstick_from_voxels() -> void:
	if noclip:
		return   # inside rock on purpose
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
	return GameMaterials.blocks_bodies(world.get_voxel(v))


# ---------------- health ----------------

## One step of a minecart ride (ADR 0100): forward and back roll it along the rails, straight on or
## round corners, the cart flag moving from rail to rail; the line's end stops it, sneaking gets off.
func _ride(delta: float) -> void:
	var m := get_parent()
	var store: BlockEntities = m.blocks
	var rail: RefCounted = store.at(ride["rail"])
	if not (rail is RailEntity) or rail.cart != "minecart" or sneaking():
		ride = {}
		return
	var n := Settings.BLOCK
	var fwd := 0.0 if ui_open or autopilot else Input.get_axis("move_back", "move_forward")
	var d: Vector3i = ride["dir"]
	var t := float(ride["t"])
	if d == Vector3i.ZERO and fwd != 0.0:
		var look := -transform.basis.z   # start the way the player looks along the rail
		d = Vector3i(0, 0, n * (1 if look.z >= 0.0 else -1)) if rail.axis == 0 else Vector3i(n * (1 if look.x >= 0.0 else -1), 0, 0)
	if fwd < 0.0 and d != Vector3i.ZERO:
		d = -d
		t = 1.0 - t
	if fwd != 0.0:
		t += Settings.m2v(Settings.minecart_speed_m) * delta / n
		while t >= 1.0:
			var nxt := RailEntity.next_of(store, rail, d)
			if nxt.is_empty():
				t = 1.0   # the end of the line
				break
			rail.cart = ""
			rail.changed.emit()
			rail = nxt["rail"]
			rail.cart = "minecart"
			rail.changed.emit()
			d = nxt["dir"]
			t -= 1.0
	ride = {"rail": rail.pos, "dir": d, "t": t}
	position = Vector3(rail.pos) + Vector3(n, 0, n) * 0.5 + Vector3(d) * (t - 0.5) + Vector3(0, 1.0 + RailEntity.height_on(rail, d, t), 0)
	velocity = Vector3.ZERO
	_fall_peak_y = position.y


## The worn stack whose "clothing" has `key` (the jetpack, ADR 0099), or {}.
func _worn_with(key: String) -> Dictionary:
	var m := get_parent()
	if m == null or m.get("stone_age") == null:
		return {}
	for s in m.stone_age.clothing.worn.values():
		if m.items.get_def(s["id"]).get("clothing", {}).has(key):
			return s
	return {}


## Vanilla fall damage: 1 per block fallen beyond the safe distance.
func _update_fall_damage() -> void:
	# "landed" = floor contact, or any contact while no longer falling (standing
	# on a voxel edge in a shaft reports a wall normal, not a floor)
	# water breaks a fall, as in vanilla: the body in it, or a shallow layer at the feet
	var wet := in_liquid or GameMaterials.is_liquid(world.get_voxel(Vector3i(position.floor())))
	var on_floor := is_on_floor() or wet
	if on_floor and not _was_on_floor:
		var fall_m := Settings.v2m(_fall_peak_y - position.y)
		if not wet and fall_m > 0.5:
			landed.emit(Vector3i((position + Vector3(0, -0.1, 0)).floor()), fall_m)
		var dmg := fall_damage(fall_m, wet)
		if dmg > 0 and inventory and GameMode.rule(game_mode, "takes_damage"):
			print("fall damage %d after %.1f m" % [dmg, fall_m])
			inventory.damage(dmg, "impact (fall %.0f m)" % fall_m)
			if inventory.is_dead():
				died.emit()
	if on_floor or position.y > _fall_peak_y:
		_fall_peak_y = position.y
	_was_on_floor = on_floor


## Fall damage for a fall of `fall_m` metres: none into water (bug report 2026-10-27_15-18-51).
static func fall_damage(fall_m: float, into_water: bool) -> int:
	return 0 if into_water else maxi(0, int(floor(fall_m - Settings.fall_damage_safe_m)))


## Seconds of breath with the eyes in material `m`; 0 when it can be breathed (bug report 2026-10-27_15-55-49).
static func breath_time(m: int) -> float:
	if GameMaterials.is_liquid(m) or GameMaterials.is_waterlogged(m):
		return Settings.air_s
	if GameMaterials.is_powder(m):
		return Settings.suffocate_s
	return 0.0


## Breath: the eyes under a liquid use it up in Settings.air_s, buried in a powder (sand, gravel
## fallen on the head) in Settings.suffocate_s; then the body takes Settings.drown_damage per
## second; out of it breath refills in 2 s.
func _update_air(delta: float) -> void:
	var eye := Vector3i((position + Vector3(0, Settings.m2v(Settings.eye_height_m), 0)).floor())
	var eye_m := world.get_voxel(eye)
	var breath_s := breath_time(eye_m)
	if breath_s <= 0.0 or not GameMode.rule(game_mode, "takes_damage"):
		air = minf(1.0, air + delta / 2.0)
		_drown_t = 0.0
	else:
		air = maxf(0.0, air - delta / breath_s)
		if air <= 0.0:
			_drown_t -= delta
			if _drown_t <= 0.0:
				_drown_t = 1.0
				if inventory:
					inventory.damage(Settings.drown_damage, "drowning")
					if inventory.is_dead():
						died.emit()
	if hud:
		hud.air = air


func respawn_at(pos: Vector3) -> void:
	var far := _needs_cover(pos)
	position = pos
	velocity = Vector3.ZERO
	_fall_peak_y = pos.y
	air = 1.0
	stains.clear()
	if inventory:
		inventory.respawn()
	if far:
		moved_far.emit()


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
	# a lit fire piece burns like fire: standing in a campfire or on a bloomery's top (bug report 2026-10-27_15-34-22)
	if fire_at.is_valid():
		var feet := Vector3i(position.floor())
		for v in [feet, feet + Vector3i.DOWN]:
			var wm := world.get_voxel(v)
			if (wm == GameMaterials.WORK_SPOT or wm == GameMaterials.WORK_BLOCK) and fire_at.call(v):
				touching.append(GameMaterials.FIRE)
				break
	var dmg := stains.update(delta, touching)
	if dmg > 0 and inventory and GameMode.rule(game_mode, "takes_damage"):
		inventory.damage(dmg, "exposure (burning or toxic coat)")
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
	var burn := float(stains.amount["burning"])
	var m: StandardMaterial3D = null
	if c.a > 0.01:
		m = _stain_overlay_burn if burn > 0.0 else _stain_overlay
		m.albedo_color = c
		if burn > 0.0:
			m.emission = PlayerStains.COLORS["burning"] * (1.2 + 0.4 * sin(Time.get_ticks_msec() * 0.02))
	if m != _stain_overlay_on:
		_stain_overlay_on = m
		if model:
			for part in model.get_children():
				if part is GeometryInstance3D:
					part.material_overlay = m
		if viewmodel:
			viewmodel.set_overlay(m)


## Materials MaterialWarmup draws under the loading screen (their first draw compiles a pipeline).
func warm_materials() -> Array:
	return [crack_material, _stain_overlay, _stain_overlay_burn, brush_tool.preview_mat]


static func _make_stain_overlay(burning: bool) -> StandardMaterial3D:
	var m := StandardMaterial3D.new()
	m.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	m.roughness = 0.4   # a wet, glossy coat
	m.emission_enabled = burning
	m.albedo_color = Color(1, 1, 1, 0)
	return m


# ---------------- mining ----------------

func _update_mining(delta: float) -> void:
	var holding := false
	if is_on_floor() or flying:
		Brush.tunnel_floor_y = floori(position.y + 0.05)   # the tunnel brush's floor: where the feet stand
	script_break_s = maxf(0.0, script_break_s - delta)
	if autopilot:
		holding = autopilot_action == "mine" or script_break_s > 0.0
	if edits_locked or not GameMode.rule(game_mode, "interacts"):
		holding = false
	elif not autopilot:
		holding = script_break_s > 0.0 or (_mouse_captured and not ui_open and not free_cam and not building and Input.is_action_pressed("action_break"))
	var hit: VoxelRaycastResult = _aim() if (holding or (_mouse_captured and not ui_open and not autopilot) or (script_aim and not ui_open)) else null
	var mob = aimed_mob(hit) if (_mouse_captured and not ui_open and not autopilot) else null
	_update_look(hit, mob)
	if hit and mob != null:
		hit = null   # an animal is in front of the block: LMB hits it instead (_attack)
	_update_brush_info()
	var station_mat := _held_station()
	var held_def := inventory.held_def() if inventory != null and GameMode.rule(game_mode, "interacts") else {}
	var work_spot: Dictionary = work_ghost.call(hit, sneaking()) if hit and work_ghost.is_valid() and not holding and not held_def.is_empty() else {}
	_station_ghost.visible = false
	if hit and Settings.brush_ghost and (station_mat > 0 or held_def.has("furniture") or not work_spot.is_empty()) and not holding:
		var tm := world.get_voxel(hit.position)
		_highlight.visible = false
		brush_tool.hide()
		if not work_spot.is_empty():   # a firepit, sieve, anvil, mould...: its footprint's outline
			_highlight.visible = true
			_set_outline(Vector3(work_spot["anchor"]), Vector3(work_spot["size"]), Color(0.35, 0.7, 1.0, 0.85) if work_spot["free"] else Color(0.9, 0.1, 0.1, 0.9), false)
		elif sneaking() or tm <= 0 or GameMaterials.defs()[tm - 1].get("interact", "work") == "work":   # a click there opens it, places nothing
			if station_mat > 0:
				_show_station_ghost(hit, station_mat)
			else:
				_show_furniture_ghost(hit, held_def["furniture"])
	elif hit and Settings.brush_ghost and brush_ghost_fits(world.get_voxel(hit.position)) and (holding or _places_voxels() or _mines_with_held()):
		# a block shows its textured ghost, a bare hand or a tool the thick outline of what it digs;
		# anything else (mushrooms, sticks, food) shows nothing until the button is held
		var placing := _places_voxels() and not holding
		var size := place_size() if placing else dig_size()
		_highlight.visible = not placing
		if not placing:
			var hb := Brush.dig_bounds(hit.position, size, brush_shape, hit.previous_position - hit.position)
			_set_outline(hb.position, hb.size, Color(0.05, 0.05, 0.05, 0.85), true)   # a dig lies inside the terrain: its outline shows through
		brush_tool.show_at(hit, size, brush_shape, placing, ItemIcon.material_id_for(inventory.items, inventory.held_def()["id"]) if placing else 0)
	else:
		_highlight.visible = false
		brush_tool.hide()
	# keep placing while the button is held (vanilla repeats every 4 ticks)
	if not autopilot and not ui_open and not building and _mouse_captured and Input.is_action_pressed("action_place") and _place_cooldown <= 0.0:
		_use(false)
	# the laser cutter (PlaytestTools): its beam replaces breaking
	var laser_held := playtest_laser.is_valid() and inventory != null and PlaytestTools.is_beam(inventory.held_def())
	if laser_held or _lasering:
		_lasering = laser_held and holding
		playtest_laser.call(_lasering, camera.global_position, -camera.global_transform.basis.z, delta, brush_size)
		if laser_held:
			_highlight.visible = false
			brush_tool.hide()
			_reset_mining()
			return
	# knapping (ADR 0023): a left click chips a cell instead of mining; holding repeats
	if holding and hit and _break_cooldown <= 0.0 and work_hit.is_valid():
		if work_hit.call(hit, camera.global_position, -camera.global_transform.basis.z, sneaking()):
			_break_cooldown = Settings.work_repeat_s
			_reset_mining()
			if viewmodel:
				viewmodel.swing()
			return
	if not holding or hit == null or _break_cooldown > 0.0:
		_reset_mining()
		return
	var p: Vector3i = hit.position
	var size := dig_size()
	var normal: Vector3i = hit.previous_position - hit.position
	if size > Brush.NATIVE_ABOVE:
		_dig_big(hit, size)
		return
	var key: Vector3i = Vector3i(Brush.dig_bounds(p, size, brush_shape, normal).position.floor()) if brush_shape in Brush.BOX_SHAPES else p
	if key != _mine_target or normal != _dig_normal:
		_mine_target = key
		_dig_normal = normal
		_mine_progress = 0.0
		_dig_cells = _dig_selection(p, size)
		_mine_time = _dig_time(p, _dig_cells)
		_swing_t = 0.0
	if _mine_time < 0.0:
		_crack.visible = false
		if hud:
			hud.break_progress = 0.0
		return  # unbreakable
	_mine_progress += delta / maxf(_mine_time, 0.001) if _mine_time > 0.0 else 1.0
	if hud:
		hud.break_progress = _mine_progress
	_crack.visible = false
	_swing_t -= delta
	if _swing_t <= 0.0 and _mine_progress < 1.0:
		_swing_t = SWING_EVERY_S
		if viewmodel:
			viewmodel.swing()
	if _mine_progress >= 1.0:
		var mat := world.get_voxel(p)
		if mat > 0 and not GameMaterials.is_plain_voxel(mat) and not _is_plant(mat):
			# a station, furniture piece or work spot: the single-voxel break
			var mat_name := GameMaterials.name_of(mat)
			var drops := mining.roll_drops(mat_name, inventory.held_def(), _drop_rng) if mining and GameMode.rule(game_mode, "drops") else []
			voxel_broken.emit(p, drops)
			_wear(1)
		else:
			_dig_cells = _dig_selection(p, size)   # the world may have changed while digging
			var split := GroundPickups.complete(world.get_voxel, _dig_cells)
			var in_piece := {}   # pieces that drop once; stalks and ore nuggets drop per voxel, as before (ADR 0047)
			for pc in split["pieces"]:
				if not pc["per_voxel"]:
					for c in pc["cells"]:
						in_piece[c] = true
			var counts := {}
			for c in _dig_cells:
				if not in_piece.has(c):
					var n := GameMaterials.name_of(world.get_voxel(c))
					counts[n] = int(counts.get(n, 0)) + 1
			var drops := []
			if mining and GameMode.rule(game_mode, "drops"):
				drops = mining.voxel_drops(counts, inventory.held_def(), inventory.items.units,
					func(n: String) -> bool: return GameMaterials.is_plain_voxel(GameMaterials.id_of(n)),
					_dig_carry, _drop_rng, Settings.BLOCK * Settings.BLOCK * Settings.BLOCK)
				# each stick or stone piece drops its item once, where it lay (ADR 0043)
				for pc in split["pieces"]:
					if pc["per_voxel"]:
						continue
					for d in mining.roll_drops(GameMaterials.name_of(pc["material"]), inventory.held_def(), _drop_rng):
						d["at"] = pc["center"]
						drops.append(d)
			voxels_dug.emit(_dig_cells.duplicate(), drops, Brush.dig_bounds(p, size, brush_shape, _dig_normal).get_center())
			brush_tool.note("dig", p, _dig_normal, Brush.dig_bounds(p, size, brush_shape, _dig_normal), _dig_cells.size())
			if not _dig_cells.is_empty():
				_wear(ceili(_dig_cells.size() / float(Settings.BLOCK * Settings.BLOCK * Settings.BLOCK)))   # a use per block dug
		if viewmodel:
			viewmodel.swing()
		_break_cooldown = Settings.break_cooldown_s
		if autopilot and autopilot_action == "mine":
			# into the run's JSON, not stdout: one console print costs 3-15 ms on Windows and
			# showed up as the worst frames of every mining benchmark
			if mined_log.size() < 64:
				mined_log.append({"voxel": p, "now": GameMaterials.name_of(world.get_voxel(p)), "player_y": snappedf(position.y, 0.01), "on_floor": is_on_floor()})
			# dig a 2x2 shaft of brush cubes: the player stands where four meet and
			# turns a quarter after every dig (the body is narrower than two cubes)
			_yaw += PI / 2.0
			_apply_look()
		_reset_mining()


## A creative brush above Brush.NATIVE_ABOVE (2 m, 4 m): instant and native (BrushTool.dig), one dig
## per break cooldown; what it frees is checked for rigid pieces, drops follow the histogram.
func _dig_big(hit: VoxelRaycastResult, size: int) -> void:
	if brush_tool.busy():
		return   # the last big brush is still being written
	var mask := BrushTool.dig_mask(true, {}, mining, _is_plant)
	for m in 256:
		if mask[m] != 0 and farm != null and farm.is_crop_material(m):
			mask[m] = 0   # crops go through Farming: the small brush takes them
	var n := brush_tool.dig(hit.position, hit.previous_position, size, brush_shape, mask, world.get_voxel(hit.position))
	brush_tool.note("dig", hit.position, hit.previous_position - hit.position, Brush.dig_bounds(hit.position, size, brush_shape, hit.previous_position - hit.position), n)
	if viewmodel:
		viewmodel.swing()
	_break_cooldown = Settings.break_cooldown_s


## Big brushes finished writing this frame (BrushTool.tick): Main gets the chips, the rigid piece
## check and TNT charges.
func _brush_jobs_done() -> void:
	for j in brush_tool.tick():
		if j["kind"] == "dig":
			if int(j["n"]) > 0:
				big_dug.emit(j["box"], int(j["hint"]))
		elif int(j["n"]) > 0:
			brush_filled.emit(j["box"], int(j["material"]))


## The dig / build ghost belongs on what the brush digs or builds against: a station, work piece,
## furniture or anything a click works on gets none (bug report 2026-10-27_18-06-58: a brush cube
## outline sat on the anvil's work piece). Holding a station item shows its own ghost instead.
static func brush_ghost_fits(m: int) -> bool:
	return m <= 0 or GameMaterials.is_plain_voxel(m) or _is_plant(m) or GameMaterials.is_liquid(m)


## A plant or ground pickup voxel (grass, flowers, reeds, pebbles, twigs): dug
## with the brush, walked through.
static func _is_plant(m: int) -> bool:
	if m <= 0 or GameMaterials.is_plain_voxel(m) or GameMaterials.blocks_bodies(m) or GameMaterials.is_liquid(m):
		return false
	var d: Dictionary = GameMaterials.defs()[m - 1]
	return not d.has("interact") and not d.has("entity") and d.get("model", "") != "empty"


## The voxels a dig at `p` removes: the brush's cells the held tool can break
## at a pace comparable to the aimed voxel (a shovel in dirt leaves the rock
## around it standing). Stations, furniture, work spots, liquids and the
## unbreakable never go with a brush; such an aimed voxel digs alone.
func _dig_selection(p: Vector3i, size: int) -> Array[Vector3i]:
	var out: Array[Vector3i] = []
	var hit_mat := world.get_voxel(p)
	if hit_mat > 0 and not GameMaterials.is_plain_voxel(hit_mat) and not _is_plant(hit_mat):
		out.append(p)
		return out
	var creative := GameMode.rule(game_mode, "instant_break")
	var held := inventory.held_def() if inventory else {}
	var t_hit := mining.break_time(GameMaterials.name_of(hit_mat), held) if mining else 0.0
	var times := {}
	for c in Brush.dig_cells(p, size, brush_shape, _dig_normal):
		var m := world.get_voxel(c)
		if m <= 0 or GameMaterials.is_monolith(m) or not (GameMaterials.is_plain_voxel(m) or _is_plant(m)):
			continue
		if creative:
			out.append(c)
			continue
		if not times.has(m):
			times[m] = mining.break_time(GameMaterials.name_of(m), held) if mining else 0.0
		var t: float = times[m]
		if t < 0.0 or t > maxf(t_hit * 4.0, 0.5):
			continue
		out.append(c)
	return GroundPickups.complete(world.get_voxel, out)["cells"]   # pieces go whole (ADR 0043, 0047)


## Seconds a dig takes: each voxel costs its material's block time / BLOCK^3,
## so a full 0.5 m cube of dirt takes what a Minecraft block did (ADR 0034).
func _dig_time(p: Vector3i, cells: Array[Vector3i]) -> float:
	var hit_mat := world.get_voxel(p)
	if hit_mat <= 0 or GameMaterials.is_monolith(hit_mat):
		return -1.0
	if GameMode.rule(game_mode, "instant_break"):
		return 0.0
	var held := inventory.held_def() if inventory else {}
	if not GameMaterials.is_plain_voxel(hit_mat) and not _is_plant(hit_mat):
		return mining.break_time(GameMaterials.name_of(hit_mat), held) if mining else 0.0
	if cells.is_empty():
		return -1.0
	if MiningTable.deconstructs(held):
		return 0.0   # built blocks come apart at once
	var per := float(Settings.BLOCK * Settings.BLOCK * Settings.BLOCK)
	var total := 0.0
	var times := {}
	for c in cells:
		var m := world.get_voxel(c)
		if not times.has(m):
			times[m] = maxf(mining.break_time(GameMaterials.name_of(m), held), 0.0) if mining else 0.0
		total += times[m] / per
	return maxf(total, 0.12)


## The look-at panel (like the Jade mod): what is under the crosshair, the tool
## it wants and whether the held item is that tool, or an animal and its health.
func _update_look(hit: VoxelRaycastResult, mob) -> void:
	if hud == null:
		return
	# the panel is rebuilt when its target, the held item or the mode changes, and
	# a few times a second for live lines (health, a clay form's layer)
	var held_id: String = inventory.held().get("id", "") if inventory else ""
	var key := ""
	var liquid := _aimed_liquid(hit) if mob == null else Vector3i(1 << 30, 0, 0)
	if liquid.x != 1 << 30:
		key = "liquid %s %d" % [liquid, world.get_voxel(liquid)]
	elif mob != null:
		key = "mob %d %d" % [mob.uid, mob.health]
	elif hit != null:
		key = "%s %d %s %s" % [hit.position, world.get_voxel(hit.position), held_id, game_mode]
	_look_refresh -= 1
	if key == _look_key and _look_refresh > 0:
		return
	_look_key = key
	_look_refresh = 15
	var info := {}
	if liquid.x != 1 << 30:
		var lm := GameMaterials.material_of_voxel(world.get_voxel(liquid))
		var lname := _loose_name(lm)
		var lid := inventory.items.item_for_material(lname) if inventory else ""
		info = {"title": inventory.items.display_name(lid) if lid != "" else lname.capitalize(), "icon": lid}
	elif mob != null:
		info = {"title": String(mob.species).capitalize(), "line": "Health %d / %d" % [mob.health, int(mob.def.get("health", 8))]}
	elif hit != null:
		var mat := world.get_voxel(hit.position)
		if mat > 0:
			info = _block_info(hit.position, mat)
	look_item = String(info.get("icon", ""))
	hud.set_look(info)


## The first liquid voxel the aim ray crosses before its hit (the aim ray passes through liquids),
## else x = 1 << 30; scripted: also with the mouse free (a bucket used by /use). Under a liquid the panel keeps showing blocks (bug report 2026-10-27_15-50-30).
func _aimed_liquid(hit: VoxelRaycastResult, scripted := false) -> Vector3i:
	var none := Vector3i(1 << 30, 0, 0)
	if (not _mouse_captured and not scripted) or ui_open:
		return none
	var from := camera.global_position
	if GameMaterials.is_liquid(GameMaterials.material_of_voxel(world.get_voxel(Vector3i(from.floor())))):
		return none
	var dir := -camera.global_transform.basis.z
	var dist := Settings.m2v(reach()) if hit == null else from.distance_to(Vector3(hit.position) + Vector3(0.5, 0.5, 0.5))
	var last := Vector3i(from.floor())
	for i in int(dist * 2.0):
		var v := Vector3i((from + dir * (i * 0.5)).floor())
		if v == last:
			continue
		last = v
		if hit != null and v == hit.position:
			break
		if GameMaterials.is_liquid(GameMaterials.material_of_voxel(world.get_voxel(v))):
			return v
	return none


func _block_info(p: Vector3i, mat: int) -> Dictionary:
	var loose := _loose_name(mat)
	var id := inventory.items.item_for_material(loose) if inventory else ""
	var title := inventory.items.display_name(id) if id != "" else loose.capitalize()
	var info := {"title": title, "icon": id, "mat": mat}
	if describe_block.is_valid():
		info.merge(describe_block.call(p), true)
	if mining != null and not info.has("line"):
		var ripe := mining.season_note(GameMaterials.name_of(mat), Seasons.season)
		if ripe != "":
			info["line"] = ripe
	if mining == null:
		return info
	var name := GameMaterials.name_of(mat)
	var e := mining.entry(name)
	if not e.is_empty():
		info["mine"] = _mine_text(e)
	if GameMode.rule(game_mode, "instant_break"):
		return info
	if e.is_empty():
		info["tool"] = "Unbreakable"
		info["ok"] = false
		return info
	var needed: String = e.get("tool", "")
	if needed == "":
		return info
	var tier := int(e.get("min_tier", 0))
	info["tool"] = needed.capitalize() + (" (%s or better)" % ["flint", "copper", "bronze", "iron", "steel"][mini(tier, 4)] if tier > 0 else "")
	var held := inventory.held_def() if inventory else {}
	info["ok"] = MiningTable.tool_class(held) == needed and int(held.get("tier", 0)) >= tier
	if not mining.yields(name, held):
		info["drops"] = false
	return info


## "Hardness 3 · drops Iron Ore": the look-at panel's mining line from data/mining.json (ADR 0093).
func _mine_text(e: Dictionary) -> String:
	var names := PackedStringArray()
	var raw = e.get("drops", [])
	for d in (raw if raw is Array else [{"id": raw}]):
		var n := inventory.items.display_name(String(d.get("id", "")))
		if not n in names:
			names.append(n)
	var hard := "Hardness %s" % str(e.get("hardness", 0))
	return hard if names.is_empty() else hard + " · drops " + ", ".join(names)   # no "drops": crops and stations drop their own way


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
	if inventory == null or edits_locked or not GameMode.rule(game_mode, "interacts"):
		return
	var hit := _aim()
	if _offhand_builds(hit):
		# Minecraft: an empty main hand places the offhand's block (torches in the left hand, ADR 0055)
		_offhand_turn = true
		inventory.swap_hands()
		_use(first_press)
		inventory.swap_hands()
		_offhand_turn = false
		return
	if item_use.is_valid() and item_use.call(hit, first_press, camera.global_position, -camera.global_transform.basis.z):
		_after_use()
		return
	if first_press and inventory.held_def().has("throw") and throw_item.is_valid():
		_throw()
		return
	if first_press and inventory.held_def().has("fills") and _scoop(_aimed_liquid(hit, true)):
		return   # the aim ray passes through liquids: a bucket takes the first one it crosses
	if first_press and inventory.held_def().has("waypoint"):   # a surveyor's stake (ADR 0099): marks the spot on the map
		var m := get_parent()
		m.waypoints.add("%s %d" % [inventory.held_def()["waypoint"], m.waypoints.list.size() + 1], Vector3i(position.floor()), "stake")
		if GameMode.rule(game_mode, "consumes_items"):
			inventory.consume_held(1)
		_after_use()
		return
	if first_press and inventory.held_def().has("playtest") and playtest_use.is_valid():
		var pdef := inventory.held_def()
		if playtest_use.call(pdef, hit, camera.global_position, -camera.global_transform.basis.z, sneaking()):
			if GameMode.rule(game_mode, "consumes_items") and not ("tool" in pdef.get("tags", [])):
				inventory.consume_held(1)
			_after_use()
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
		_start_eating()
		return
	if not first_press and _place_cooldown > 0.0:
		return
	var target: Vector3i = hit.position
	# picking up sticks and stones: hold for a second (ADR 0043; firepits take a plain click, ADR 0078)
	if hold_key.is_valid() and hold_key.call(hit) != "":
		if first_press:
			_hold_at = hold_key.call(hit)
			_hold_t = 0.0
		return
	# Vintage Story work pieces (ADR 0023): knapping, clay forming, pit kilns, firepits, sieves
	if work_use.is_valid() and work_use.call(hit, first_press, camera.global_position, -camera.global_transform.basis.z, sneaking()):
		_after_use()
		return
	# stations open on right click; sneaking (Shift) places against them instead (vanilla)
	if first_press and not sneaking():
		var tm := world.get_voxel(target)
		if tm > 0 and GameMaterials.defs()[tm - 1].get("interact", "work") != "work":   # work pieces: the work system above
			if not (tm == GameMaterials.FURNITURE and furniture_is.is_valid() and not furniture_is.call(target, "use")):
				block_used.emit(target, tm)
				return
	var def := inventory.held_def()
	if def.is_empty():
		return
	if def.get("ignites", false):
		if first_press and ignite_at.is_valid() and ignite_at.call(hit):
			_wear(1)
			_after_use()
		return
	# farming (ADR 0022): hoes till, seeds and carrots plant on a farmland top face
	# farming (ADR 0022, 0034): the hoe tills, seeds plant over the brush's patch of ground
	if "hoe" in def.get("tags", []) and farm != null:
		if hit.previous_position.y >= target.y:
			var tilled := 0
			for c in _farm_patch(target):
				if farm.till(c):
					tilled += 1
			if tilled > 0:
				_wear(1)
				_after_use()
		return
	if def.has("plants") and farm != null and farm.is_farmland_material(farm.get_voxel.call(target)):   # Main's view: the bubble is ahead of the terrain
		if hit.previous_position == target + Vector3i.UP:
			var sown := 0
			var consumes := GameMode.rule(game_mode, "consumes_items")
			for c in _farm_patch(target):
				if consumes and int(inventory.held().get("count", 0)) <= 0:
					break
				if farm.plant(c, def["plants"]):
					sown += 1
					if consumes:
						inventory.consume_held(1)
			if sown > 0:
				_after_use()
		return
	if def.has("furniture"):
		_place_furniture(hit, def)
		return
	if def.has("food"):
		_start_eating()
		return
	if not def.has("material"):
		return
	var mat := GameMaterials.still_of(ItemIcon.material_id_for(inventory.items, def["id"]))
	if mat <= 0:
		return
	if def.has("empties_to") and GameMaterials.is_liquid(mat):
		if first_press:   # held, the button would pour the next bucket straight after
			_pour(hit, mat)
		return
	if inventory.items.is_voxel_item(def["id"]):
		if Input.is_key_pressed(KEY_SHIFT) and GameMode.rule(game_mode, "instant_break") and _paint(hit, mat):
			return
		_place_voxels(hit, mat)
		return
	if GameMaterials.is_station(mat):
		_place_station(hit, mat)
		return
	var p: Vector3i = hit.previous_position
	if _replaceable(world.get_voxel(target)):
		p = target   # Minecraft places into tall grass instead of in front of it
	if _overlaps_player(p) or p.y < 0 or p.y >= Settings.WORLD_HEIGHT_VOXELS:
		return
	if not _placeable(world.get_voxel(p)):
		return
	if GameMaterials.defs()[mat - 1].get("floor_only", false):
		var top_face := p == target or hit.previous_position == target + Vector3i.UP
		if not top_face or not MobManager.is_solid(world.get_voxel(p - Vector3i.UP)):
			return   # torches stand on a floor, not on a wall (ADR 0030)
		if GameMaterials.defs()[mat - 1].has("sapling") and not GameMaterials.is_soil(world.get_voxel(p - Vector3i.UP)):
			return   # saplings root in soil only (ADR 0112)
	voxel_placed.emit(p, mat)
	if not GameMode.rule(game_mode, "consumes_items"):
		pass   # creative: blocks and buckets never run out
	elif def.has("empties_to"):
		inventory.replace_held_one(def["empties_to"])
	else:
		inventory.consume_held(1)
	_after_use()


var _offhand_turn := false

## The main hand is empty, the offhand holds something that builds, and the aimed voxel is plain
## (no station, work piece or pickup that an empty hand works on).
func _offhand_builds(hit: VoxelRaycastResult) -> bool:
	if _offhand_turn or hit == null or not inventory.held().is_empty():
		return false
	var off := inventory.offhand_def()
	if not (off.has("material") or off.has("plants") or off.has("furniture")):
		return false
	var m := world.get_voxel(hit.position)
	if m <= 0 or m > GameMaterials.defs().size() or m == GameMaterials.WORK_SPOT or m == GameMaterials.WORK_BLOCK or GroundPickups.is_pickup(m):
		return false
	return not GameMaterials.defs()[m - 1].has("interact")


## An empty bucket scoops the connected body of the liquid at `at`, up to a bucket's worth (ADR 0091).
## False when there is no liquid it takes (at.x = 1 << 30: none crossed).
func _scoop(at: Vector3i) -> bool:
	if at.x == 1 << 30:
		return false
	var def := inventory.held_def()
	var liquid := _loose_name(world.get_voxel(at))
	if not def["fills"].has(liquid):
		return false
	var cells := Bucket.scoop_cells(world.get_voxel, at, func(m: int) -> bool: return _loose_name(m) == liquid, Bucket.capacity())
	voxels_placed.emit(cells, GameMaterials.AIR)
	if GameMode.rule(game_mode, "consumes_items"):
		inventory.replace_held_one(def["fills"][liquid], {"fill": cells.size()})
	if hud:
		hud.show_toast("Scooped %d voxels of %s" % [cells.size(), liquid.capitalize()])
	_after_use()
	return true


## A filled bucket pours what it holds into the free space at the aimed face (ADR 0091); creative
## buckets never run dry.
func _pour(hit: VoxelRaycastResult, mat: int) -> void:
	var front: Vector3i = hit.previous_position
	if _replaceable(world.get_voxel(hit.position)):
		front = hit.position
	var held := inventory.held()
	var have := Bucket.fill_of(held)
	var free := func(m: int) -> bool: return _placeable(m) and not GameMaterials.is_liquid(GameMaterials.material_of_voxel(m))
	var cells := Bucket.pour_cells(world.get_voxel, front, free, have)
	if cells.is_empty():
		return
	voxels_placed.emit(cells, mat)
	if GameMode.rule(game_mode, "consumes_items"):
		if cells.size() >= have:
			inventory.replace_held_one(inventory.held_def()["empties_to"])
		else:
			held["fill"] = have - cells.size()
			inventory.inv.set_slot(inventory.selected, held)
	_after_use()


## Places the held voxel item as the brush shape against the aimed face (ADR
## 0034): only empty or replaceable cells outside the player's body are filled,
## at most as many as the stack holds.
func _place_voxels(hit: VoxelRaycastResult, mat: int) -> void:
	var cells: Array = []
	var have := int(inventory.held().get("count", 0))
	var creative := not GameMode.rule(game_mode, "consumes_items")
	var size := place_size()
	if size > Brush.NATIVE_ABOVE:
		if brush_tool.overlaps(hit.position, hit.previous_position, size, brush_shape, true, _body_box()):
			if hud:
				hud.show_toast("You are standing in it")
			return
		if not brush_tool.busy():
			var n := brush_tool.place(hit.position, hit.previous_position, size, brush_shape, mat, BrushTool.place_mask(_placeable))
			brush_tool.note("place", hit.position, hit.previous_position - hit.position, Brush.place_bounds(hit.position, hit.previous_position, size, brush_shape), n)
			if n > 0:
				_after_use()
		return
	for c in Brush.place_cells(hit.position, hit.previous_position, size, brush_shape):
		if not creative and cells.size() >= have:
			break
		if c.y < 0 or c.y >= Settings.WORLD_HEIGHT_VOXELS or _overlaps_player(c):
			continue
		var m := world.get_voxel(c)
		if not _placeable(m):
			continue
		cells.append(c)
	if cells.is_empty():
		return
	brush_tool.note("place", hit.position, hit.previous_position - hit.position, Brush.place_bounds(hit.position, hit.previous_position, size, brush_shape), cells.size())
	voxels_placed.emit(cells, mat)
	if not creative:
		inventory.consume_held(cells.size())
	_after_use()


## Creative Shift + right click with a block: the dig shape around the aimed voxel takes the held
## material wherever it is a plain voxel (recolouring a hillside, a wall, a floor). Any size, native.
func _paint(hit: VoxelRaycastResult, mat: int) -> bool:
	var mask := BrushTool.paint_mask()
	var aimed := world.get_voxel(hit.position)
	if aimed <= 0 or aimed >= mask.size() or mask[aimed] == 0:
		return false   # a station, plant or work piece: the right click goes on as usual
	if not brush_tool.busy():
		brush_tool.paint(hit.position, hit.previous_position, dig_size(), brush_shape, mat, mask)
	_after_use()
	return true


## The player's body box in voxels (placements never fill it).
func _body_box() -> AABB:
	var r := Settings.m2v(Settings.player_radius_m)
	return AABB(position - Vector3(r, 0, r), Vector3(2 * r, Settings.m2v(Settings.player_height_m), 2 * r))


## The held item places voxels with the brush (a voxel item): the preview shows the placement.
func _places_voxels() -> bool:
	if inventory == null:
		return false
	var def := inventory.held_def()
	return def.has("material") and inventory.items.is_voxel_item(def["id"])


## A station item (crafting table, chest, furnace, pot) fills its footprint standing on the aimed
## face (ADR 0034), when all of it is free.
func _place_station(hit: VoxelRaycastResult, mat: int) -> void:
	var spot := station_spot(hit, mat)
	if not spot["free"]:
		return
	var anchor: Vector3i = spot["anchor"]
	voxel_placed.emit(anchor, mat)
	if GameMode.rule(game_mode, "consumes_items"):
		inventory.consume_held(1)
	_after_use()


## The material of the held station item (chest, crafting table, furnace, pot), 0 for anything else.
func _held_station() -> int:
	if inventory == null or not GameMode.rule(game_mode, "interacts"):
		return 0
	var def := inventory.held_def()
	if not def.has("material") or inventory.items.is_voxel_item(def["id"]):
		return 0
	var mat := ItemIcon.material_id_for(inventory.items, def["id"])
	return mat if mat > 0 and GameMaterials.is_station(mat) else 0


## Where a station placed at this aim goes: its footprint centred on the aimed 0.5 m block (a 1 m
## chest used to grow off one corner), standing on the aimed face, lifted up to a voxel over bumpy
## ground. {anchor, size, free}: free is false when no lift clears it (the ghost turns red).
func station_spot(hit: VoxelRaycastResult, mat: int) -> Dictionary:
	var kind: String = GameMaterials.defs()[mat - 1]["entity"]
	var size: Vector3i = StationEntity.SIZES.get(kind, Vector3i(Settings.BLOCK, BlockEntities.height_of(kind), Settings.BLOCK))
	var front: Vector3i = hit.position if _replaceable(world.get_voxel(hit.position)) else hit.previous_position
	var anchor := BlockEntities.anchor_for(front) - Vector3i((size.x - Settings.BLOCK) / 2, 0, (size.z - Settings.BLOCK) / 2)
	for lift in 2:
		var a := anchor + Vector3i(0, lift, 0)
		var ok := true
		for c in BlockEntities.box(a, size):
			var v := world.get_voxel(c)
			if not (_placeable(v) or PlantStamps.is_clearable_cell(v)) or _overlaps_player(c):
				ok = false
				break
		if ok:
			return {"anchor": a, "size": size, "free": true}
	return {"anchor": anchor, "size": size, "free": false}


func _show_station_ghost(hit: VoxelRaycastResult, mat: int) -> void:
	var spot := station_spot(hit, mat)
	var has_model := StationEntity.SIZES.has(GameMaterials.defs()[mat - 1]["entity"])
	if has_model:
		_station_ghost.mesh = StationEntity.ghost_mesh(GameMaterials.name_of(mat))
		_station_ghost.global_transform = Transform3D(Basis(), Vector3(spot["anchor"]))   # the furniture ghost turns and scales it
		_station_ghost.visible = true
		StationEntity.ghost_tint(not spot["free"])
	_highlight.visible = not has_model   # the pot and anything without a model show their footprint's outline
	if not has_model:
		_set_outline(Vector3(spot["anchor"]), Vector3(spot["size"]), Color(0.35, 0.7, 1.0, 0.85) if spot["free"] else Color(0.9, 0.1, 0.1, 0.9), false)


## A bare hand or a tool: what mining is done with (anything else held shows no dig outline).
func _mines_with_held() -> bool:
	if inventory == null:
		return false
	var def := inventory.held_def()
	return def.is_empty() or "tool" in def.get("tags", [])


## The ground voxels a hoe or seeds work on: the brush's square (brush_size
## across, snapped like a cube brush) around the aimed voxel, each column's
## surface within a voxel of the aimed height (open air above it).
func _farm_patch(target: Vector3i) -> Array[Vector3i]:
	var out: Array[Vector3i] = []
	var n := clampi(brush_size, 1, Brush.NATIVE_ABOVE)
	var o := Vector3i(floori(float(target.x) / n) * n, target.y, floori(float(target.z) / n) * n)
	for dz in n:
		for dx in n:
			for dy in [0, 1, -1]:
				var c := o + Vector3i(dx, dy, dz)
				if farm.get_voxel.call(c + Vector3i.UP) == GameMaterials.AIR and farm.get_voxel.call(c) != GameMaterials.AIR:
					out.append(c)
					break
	return out


func _replaceable(m: int) -> bool:
	return m > 0 and m <= GameMaterials.defs().size() and GameMaterials.defs()[m - 1].get("replaceable", false)


## A cell a placed block may take: air, a replaceable plant, or liquid and gas (building in water
## pushes it out, like Minecraft; bug report 2026-10-25).
func _placeable(v: int) -> bool:
	if v == GameMaterials.AIR or _replaceable(v):
		return true
	var m := GameMaterials.material_of_voxel(v)
	return GameMaterials.is_liquid(m) or (m > 0 and GameMaterials.defs()[m - 1].get("cell_type", "") == "gas")


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
	if _attack_cooldown > 0.0 or edits_locked or ui_open or not entity_attack.is_valid() or not GameMode.rule(game_mode, "interacts"):
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
	_wear(1 if "weapon" in def.get("tags", []) else 2)   # Minecraft: a tool used as a weapon wears twice
	_attack_cooldown = 0.25
	if viewmodel:
		viewmodel.swing()


## Right click with a throwable (the spear): it leaves the hand and flies
## along the look; Main moves it and hits animals (creative keeps it).
func _throw() -> void:
	var def := inventory.held_def()
	var stack := {"id": def["id"], "count": 1}
	if GameMode.rule(game_mode, "consumes_items"):
		stack = Inventory.wear_stack(inventory.items, inventory.inv.take(inventory.selected, 1))   # broken: it shatters where it lands
	var dir := -camera.global_transform.basis.z
	throw_item.call(stack, camera.global_position + dir * 0.6, dir)
	_attack_cooldown = 0.5
	if viewmodel:
		viewmodel.swing()


## Wears the held tool by `uses` in survival (ADR 0042); creative tools never wear.
func _wear(uses: int) -> void:
	if inventory != null and GameMode.rule(game_mode, "consumes_items") and uses > 0:
		inventory.wear_held(uses)


## Beds and chairs: the clicked face's neighbour is the anchor; the footprint must be free.
func _place_furniture(hit: VoxelRaycastResult, def: Dictionary) -> void:
	if not place_furniture.is_valid():
		return
	var spot := furniture_spot(hit, def["furniture"])
	if spot["free"] and place_furniture.call(def["furniture"], spot["anchor"], spot["facing"]):
		if GameMode.rule(game_mode, "consumes_items"):
			inventory.consume_held(1)
		_after_use()


## Where a piece of furniture placed at this aim goes: {anchor, facing, free}; free is false when a
## footprint voxel is taken (the ghost turns red).
func furniture_spot(hit: VoxelRaycastResult, kind: String) -> Dictionary:
	var front := hit.position if _replaceable(world.get_voxel(hit.position)) else hit.previous_position
	var anchor := BlockEntities.anchor_for(front)
	var facing := FurnitureEntity.facing_for(kind, -camera.global_transform.basis.z)
	if FurnitureEntity.KINDS[kind].get("wall", false):
		# a door, fence or painting goes in the clicked voxel's depth layer, wherever the wall is (ADR 0094)
		var fwd := FurnitureEntity.DIRS[facing]
		var d: Vector3i = front - FurnitureEntity.footprint(kind, anchor, facing)[0]
		anchor += fwd * (d.x * fwd.x + d.z * fwd.z)
	var free := true
	for c in FurnitureEntity.footprint(kind, anchor, facing):
		if not _placeable(world.get_voxel(c)) or _overlaps_player(c):
			free = false
			break
	return {"anchor": anchor, "facing": facing, "free": free}


## The held furniture's see-through model where a click puts it (the station ghost's material and tint).
func _show_furniture_ghost(hit: VoxelRaycastResult, kind: String) -> void:
	var spot := furniture_spot(hit, kind)
	var f := FurnitureEntity.new(spot["anchor"], kind, spot["facing"])
	_station_ghost.mesh = FurnitureEntity.ghost_mesh(kind, not spot["free"])
	_station_ghost.global_transform = Transform3D(Basis(Vector3.UP, f.yaw()).scaled(Vector3.ONE * Settings.BLOCK), f._block_centre())
	_station_ghost.visible = true


# ---------------- eating ----------------

## Holding right click with food eats it after EAT_S, when hungry or hurt (Survival, ADR 0040);
## kept held, the next bite starts on its own (bug report 2026-10-27_15-36-07).
func _start_eating() -> void:
	var def := inventory.held_def()
	if not def.has("food") or _eat_id != "":
		return
	if survival != null:
		if not survival.wants_food():
			return
	elif inventory.health >= inventory.max_health:
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
		if survival != null:
			survival.eat(def, float(inventory.held().get("spoil", 0.0)))
			if def.has("buff") and hud:
				hud.show_toast(survival.buff_text())   # ADR 0121: what the meal does, and for how long
		else:
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
		model.leg_r.rotation = Vector3(PI * 0.5, 0, -0.05)
		model.leg_l.rotation = Vector3(PI * 0.5, 0, 0.05)
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


## A right click held on a pickup or a firepit (ADR 0043): the bar fills over Settings.hold_use_s, then
## hold_done acts; still holding, it goes again (the next thing onto the fire). Letting go or looking
## away cancels it.
func _update_hold(delta: float) -> void:
	if _hold_at == "":
		return
	var pressed := autopilot_hold if autopilot else (_mouse_captured and not ui_open and not building and Input.is_action_pressed("action_place"))
	var hit: VoxelRaycastResult = _aim() if pressed and not edits_locked else null
	if hit == null or hold_key.call(hit) != _hold_at:
		if not pressed and hud and _hold_t < Settings.hold_use_s * 0.8:
			# let go early: a short click on a firepit or a pickup did nothing (bug report 2026-10-25)
			hud.show_toast("Hold right click for a second to pick it up")
		_hold_at = ""
		if hud:
			hud.break_progress = 0.0
		return
	_hold_t += delta
	if hud:
		hud.break_progress = _hold_t / Settings.hold_use_s
	if _hold_t >= Settings.hold_use_s:
		_hold_t = 0.0
		hold_done.call(hit)
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
	throw_stack(inventory.drop_held(whole))


## Throws a stack out in front of the eyes (Q, and drops from the inventory screen, ADR 0055).
func throw_stack(stack: Dictionary) -> void:
	if stack.is_empty():
		return
	var from := camera.global_position - camera.global_transform.basis.z * Settings.m2v(0.3) - Vector3(0, Settings.m2v(0.25), 0)
	# in blocks per second, the unit spawn_item takes (voxels per second here threw it 4x too fast)
	var vel := (-camera.global_transform.basis.z * Settings.drop_throw_speed_m + Vector3(0, 1.0, 0)) / (Settings.BLOCK * Settings.VOXEL_M)
	item_dropped.emit(stack, from, vel)
	if viewmodel:
		viewmodel.swing()


# ---------------- helpers: target outline and crack box ----------------

func _make_box_outline() -> MeshInstance3D:
	var m := StandardMaterial3D.new()
	_highlight_mat = m
	m.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	m.albedo_color = Color(0, 0, 0, 0.8)
	m.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	m.no_depth_test = false
	m.cull_mode = BaseMaterial3D.CULL_DISABLED
	_highlight_hidden_mat = m.duplicate()
	_highlight_hidden_mat.no_depth_test = true
	var mi := MeshInstance3D.new()
	mi.mesh = ImmediateMesh.new()
	mi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	return mi


var _outline_size := Vector3.ZERO

## Puts the outline round the box at `pos` of `size` voxels: twelve bars as thick as a fraction of the
## box (lines are one pixel wide in Godot), rebuilt only when the size changes.
func _set_outline(pos: Vector3, size: Vector3, colour: Color, through: bool) -> void:
	_highlight.global_position = pos
	_highlight.scale = Vector3.ONE
	_highlight_mat.albedo_color = colour
	_highlight_hidden_mat.albedo_color = Color(colour, colour.a * 0.3)
	_highlight_mat.next_pass = _highlight_hidden_mat if through else null
	if size == _outline_size:
		return
	_outline_size = size
	var im := _highlight.mesh as ImmediateMesh
	im.clear_surfaces()
	im.surface_begin(Mesh.PRIMITIVE_TRIANGLES, _highlight_mat)
	var t := clampf(maxf(size.x, maxf(size.y, size.z)) * 0.07, 0.3, 1.0) * 0.5
	for axis in 3:
		for i in 4:
			var lo := Vector3(-t, -t, -t)
			var hi := Vector3(t, t, t)
			hi[axis] = size[axis] + t
			var u := (axis + 1) % 3
			var w := (axis + 2) % 3
			var off := Vector3.ZERO
			off[u] = size[u] if i & 1 else 0.0
			off[w] = size[w] if i & 2 else 0.0
			_add_box(im, lo + off, hi + off)
	im.surface_end()


static func _add_box(im: ImmediateMesh, lo: Vector3, hi: Vector3) -> void:
	var c := [Vector3(lo.x, lo.y, lo.z), Vector3(hi.x, lo.y, lo.z), Vector3(hi.x, lo.y, hi.z), Vector3(lo.x, lo.y, hi.z),
		Vector3(lo.x, hi.y, lo.z), Vector3(hi.x, hi.y, lo.z), Vector3(hi.x, hi.y, hi.z), Vector3(lo.x, hi.y, hi.z)]
	for f in [[0, 1, 2, 3], [7, 6, 5, 4], [0, 4, 5, 1], [1, 5, 6, 2], [2, 6, 7, 3], [3, 7, 4, 0]]:
		for k in [0, 1, 2, 0, 2, 3]:
			im.surface_add_vertex(c[f[k]])


func _make_crack_box() -> Node3D:
	var box := BoxMesh.new()
	box.size = Vector3.ONE * 1.01
	crack_material = ShaderMaterial.new()
	crack_material.shader = preload("res://assets/shaders/crack.gdshader")
	crack_material.set_shader_parameter("crack_map", ImageTexture.create_from_image(crack_image()))
	box.material = crack_material
	var mi := MeshInstance3D.new()
	mi.mesh = box
	mi.position = Vector3(0.5, 0.5, 0.5)
	mi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	# wrap so global_position addresses the voxel corner
	var holder := Node3D.new()
	holder.add_child(mi)
	return holder


## 16x16 crack map: each pixel holds the break progress at which it cracks
## (1 = never). Random walks from the middle with branches: pixels further
## along a crack open later, so the cracks grow outwards as the block breaks.
static func crack_image() -> Image:
	var img := Image.create(16, 16, false, Image.FORMAT_R8)
	img.fill(Color(1, 1, 1))
	var rng := RandomNumberGenerator.new()
	rng.seed = 7
	var walks := []   # [x, y, dir x, dir y, start progress]
	for i in 5:
		var a := TAU * (i + rng.randf() * 0.6) / 5.0
		walks.append([7.5, 7.5, cos(a), sin(a), 0.05])
	while not walks.is_empty():
		var w: Array = walks.pop_back()
		var x: float = w[0]
		var y: float = w[1]
		var at: float = w[4]
		for _step in 12:
			x += w[2] + rng.randf_range(-0.5, 0.5)
			y += w[3] + rng.randf_range(-0.5, 0.5)
			at += 0.075
			if x < 0.0 or y < 0.0 or x >= 16.0 or y >= 16.0 or at >= 0.95:
				break
			var px := Vector2i(int(x), int(y))
			if img.get_pixelv(px).r > at:
				img.set_pixelv(px, Color(at, at, at))
			if rng.randf() < 0.12 and walks.size() < 12:
				walks.append([x, y, w[3] * (1 if rng.randf() < 0.5 else -1), w[2] * (1 if rng.randf() < 0.5 else -1), at])
	return img
