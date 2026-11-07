class_name ShapeBuilder
extends Node3D

## Shape building, box mode and prefabs (ADR 0038, Lay of the Land's building). Hold V for the build
## ring, C for the mine ring (brush shape, the wheel sizes it while held); release on a slice picks it.
## With a material in hand a shape's preview follows the aim, snapped to the snap grid (N: off, 0.5 m,
## 1 m; `snap`); a left click sets it down and frees the mouse (ADR 0143): the cursor drags its handles
## (the wheel over one steps it one snap): the green top arrow sets the height, the red ring at the
## foot grows the side you grab, the yellow ring on top hollows it (drag inward), the white knob moves
## it (up and down with the wheel); the right button held looks around; R turns it, Shift+R tips it,
## X mirrors it; a left click away builds it and takes the voxels from the inventory; a right click
## cancels. Box mode marks a box with two clicks
## (to build, or with a tool to dig); Select marks one, the same handles adjust it, then a click saves
## it as a prefab (/prefab save). GDScript
## orchestrates: cells, meshes and writes are native (ShapeTool).

const SHAPES := ["box", "wall", "cylinder", "cone", "sphere", "stairs", "slope"]
const DEFAULT_SIZE := {"box": Vector3i(8, 8, 8), "wall": Vector3i(32, 24, 4), "cylinder": Vector3i(16, 24, 16),
	"cone": Vector3i(16, 16, 16), "sphere": Vector3i(16, 16, 16), "stairs": Vector3i(8, 16, 16), "slope": Vector3i(8, 8, 16)}
const MAX_SIDE := 256          # voxels (32 m) per side
const AIM_M := 32.0            # how far the builder aims (further than the hand reaches: big shapes)
const HANDLE := 2.4            # handle size, voxels at 4 m (they grow with distance)
const GHOST_OPACITY := 0.8     # the ghost is the real material, see-through (ADR 0144)
const GHOST_TINT := 0.1        # how far it leans to the state's colour: enough to tell it from a placed block
const MENU_TAP_MS := 250       # V or C let go sooner: a tap, the ring stays up until a click
const PREVIEW_HZ := 30.0
const PREVIEW_CELLS := 8000.0   # above this a changing shape previews coarse: ShapeTool.cells costs 0.7 us a cell, a 1 M voxel ball took 700 ms per drag step
const FULL_AFTER_S := 0.15      # how long a big shape must rest before its full detail is built off the main thread
const SNAPS := [1, Settings.BLOCK, 2 * Settings.BLOCK]   # N cycles: off, the 0.5 m block grid, 1 m

enum State { OFF, AIM, EDIT, BOX_START, BOX_END, DIG_BOX, SELECTED }

var main: Node
var player: Player
var bubble: SimBubble
var st := ShapeTool.new()
var prefabs := PrefabStore.new()
var menu: RadialMenu

var state := State.OFF
var mode := ""              # "shape", "box", "dig", "select", "prefab"
var shape := "box"
var lo := Vector3i.ZERO     # world min corner of the placement
var wsize := Vector3i(8, 8, 8)   # its world (oriented) size
var rot := 0
var mirror := false
var hollow := 0
var step := Settings.BLOCK
var snap: int = Settings.BLOCK   # the grid a placement's corner and its handle steps keep to (voxels)
var grid := PackedByteArray()    # a prefab's materials (local)
var grid_size := Vector3i.ZERO
var prefab_label := ""
var refusal := ""           # why the last commit was refused ("" when it built)
var on_select: Callable     # ADR 0161: a blueprint copies the selected box instead of naming a prefab
var on_commit: Callable     # ADR 0161: (lo, rot, mirror) marks a blueprint site instead of building
var stats := {"built": 0, "voxels": 0, "dug": 0, "saved": 0, "last_ms": 0.0, "max_ms": 0.0}

var _box_a := Vector3i.ZERO
var _layer: CanvasLayer
var _hint: Label
var _preview: MeshInstance3D
var _ghost_mat: ShaderMaterial    # the brush ghost's shader: the held material's pattern washed in the state colour
var _handles: Array = []    # [{id, node, pos, axis, color, radius (rings)}]
var _hover := ""
var _hover_pt := Vector3.ZERO   # where the aim ray meets the hovered handle (a ring: which side)
var _rmb := {}              # the right button held in EDIT: {pos, moved}; looking around
var _drag := {}
var _dirty := true
var _since_preview := 1.0
var _short := false
var _need_text := ""
var _dig_progress := 0.0
var _dig_time := 0.0
var _mask_free := PackedByteArray()
var _menu_items: Array = []
var _menu_kind := ""
var _menu_opened_ms := 0
var _depth_mat: ShaderMaterial    # the ghost's first pass: only the nearest surface gets coloured
var _preview_key := ""            # the params the mesh on show was built from (it is relative to lo: a move only moves it)
var _full_task := -1              # WorkerThreadPool task building a big shape's full detail
var _full_result := {}
var _full_wait := 0.0             # seconds left until that task starts: it waits for the shape to stop changing
var _handle_mat: StandardMaterial3D   # the handles' look (each handle gets a copy for its colour); next_pass is the x-ray through terrain


func setup(p_main: Node) -> void:
	main = p_main
	player = main.player
	bubble = main.bubble
	_layer = CanvasLayer.new()
	_layer.layer = 5
	add_child(_layer)
	menu = RadialMenu.new()
	_layer.add_child(menu)
	_hint = Label.new()
	_hint.set_anchors_and_offsets_preset(Control.PRESET_BOTTOM_RIGHT)   # a short key list at the corner (ADR 0144), out of the view of what you build
	_hint.offset_left = -460.0
	_hint.offset_right = -16.0
	_hint.offset_top = -420.0
	_hint.offset_bottom = -72.0
	_hint.horizontal_alignment = HORIZONTAL_ALIGNMENT_RIGHT
	_hint.vertical_alignment = VERTICAL_ALIGNMENT_BOTTOM
	_hint.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_hint.add_theme_font_size_override("font_size", 14)
	_hint.add_theme_color_override("font_outline_color", Color.BLACK)
	_hint.add_theme_constant_override("outline_size", 5)
	_layer.add_child(_hint)
	_preview = MeshInstance3D.new()
	_preview.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	_ghost_mat = ShaderMaterial.new()
	_ghost_mat.shader = preload("res://assets/shaders/brush_ghost.gdshader")
	_ghost_mat.render_priority = 1
	_depth_mat = ShaderMaterial.new()
	_depth_mat.shader = preload("res://assets/shaders/brush_ghost_depth.gdshader")
	_depth_mat.next_pass = _ghost_mat
	_preview.visible = false
	add_child(_preview)
	_handle_mat = StandardMaterial3D.new()   # flat white blocks over everything (ADR 0144): the transparent pipeline draws them after the terrain
	_handle_mat.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	_handle_mat.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	_handle_mat.no_depth_test = true
	_handle_mat.cull_mode = BaseMaterial3D.CULL_DISABLED
	_handle_mat.vertex_color_use_as_albedo = true
	_handle_mat.render_priority = 3
	_mask_free.resize(256)
	for i in GameMaterials.defs().size():
		var d: Dictionary = GameMaterials.defs()[i]
		if d.get("replaceable", false) or d.get("cell_type", "solid") in ["gas", "fire"]:
			_mask_free[i + 1] = 1
	_mask_free[GameMaterials.AIR] = 1


## Materials MaterialWarmup draws under the loading screen (a first draw compiles a pipeline).
func warm_materials() -> Array:
	return [_depth_mat]


## A handle's mesh and material for MaterialWarmup: the pipeline depends on the vertex format too.
func warm_mesh() -> Mesh:
	var m := GizmoMesh.arrow()
	m.surface_set_material(0, _handle_mat)
	return m


func _exit_tree() -> void:
	if _full_task >= 0:
		WorkerThreadPool.wait_for_task_completion(_full_task)
		_full_task = -1


func is_active() -> bool:
	return state != State.OFF


# ---------------- the menu ----------------

func _held_material() -> int:
	var def: Dictionary = player.inventory.held_def()
	if def.is_empty() or not player.inventory.items.is_voxel_item(def["id"]):
		return 0
	return GameMaterials.still_of(ItemIcon.material_id_for(player.inventory.items, def["id"]))


func _open_menu() -> void:
	player.input_grab = true
	_menu_kind = "main"
	_menu_items = [{"id": "brush", "label": "Brush"}]
	if _held_material() > 0:
		for s in SHAPES:
			_menu_items.append({"id": "shape:" + s, "label": s.capitalize()})
		_menu_items.append({"id": "box", "label": "Box mode"})
		_menu_items.append({"id": "prefabs", "label": "Prefabs"})
	else:
		_menu_items.append({"id": "dig", "label": "Box dig"})
		_menu_items.append({"id": "prefabs", "label": "Prefabs"})
	_menu_items.append({"id": "select", "label": "Select"})
	menu.open(_menu_items, "Build")


## The mine ring (C): the brush shapes, the wheel sizes the brush while it is up.
func _open_mine_menu() -> void:
	player.input_grab = true
	_menu_kind = "mine"
	_menu_items = []
	for s in Brush.SHAPES:
		_menu_items.append({"id": "mine:" + s, "label": s.capitalize()})
	menu.open(_menu_items, _mine_title(), -1 if is_active() else Brush.SHAPES.find(player.brush_shape), "(Shift + Middle Click) Dig Only")   # in a build mode a tap on C keeps it


func _mine_title() -> String:
	return "Brush  %s  (Wheel) Size" % Brush._len(player.brush_size)


func _open_prefab_menu() -> void:
	_menu_kind = "prefabs"
	_menu_items = []
	var all := prefabs.list()
	for cat in all:
		for nm in all[cat]:
			_menu_items.append({"id": "prefab:%s/%s" % [cat, nm], "label": nm if cat == PrefabStore.LAST_CATEGORY else "%s / %s" % [cat, nm]})
	if _menu_items.is_empty():
		_toast("No prefabs yet: build something, or Select an area and /prefab save <category> <name>")
		return
	menu.open(_menu_items.slice(0, 12), "Prefabs")
	player.input_grab = true


func _close_menu() -> void:
	var id := menu.close()
	player.input_grab = false
	if id == "":
		return
	if id == "brush":
		_set_state(State.OFF)
		_open_mine_menu.call_deferred()   # its shapes and, with the wheel, its size
	elif id.begins_with("shape:"):
		start_shape(id.substr(6))
	elif id in ["box", "dig", "select"]:
		start_box(id)
	elif id == "prefabs":
		_open_prefab_menu.call_deferred()
	elif id.begins_with("prefab:"):
		var parts := id.substr(7).split("/")
		start_prefab(parts[0], parts[1])
	elif id.begins_with("mine:"):
		player.set_brush(player.brush_size, id.substr(5))
		_set_state(State.OFF)   # a brush shape means the brush: never a build mode beside it


## Starts placing a shape with the held material (also /shape).
func start_shape(p_shape: String, size := Vector3i.ZERO) -> void:
	on_select = Callable()
	on_commit = Callable()
	mode = "shape"
	shape = p_shape
	rot = 0
	mirror = false
	hollow = 0
	wsize = size if size != Vector3i.ZERO else DEFAULT_SIZE.get(shape, Vector3i(8, 8, 8))
	_set_state(State.AIM)


## Box mode ("box" builds, "dig" digs with the held tool, "select" marks a prefab): two clicks.
func start_box(p_mode: String) -> void:
	on_select = Callable()
	on_commit = Callable()
	mode = p_mode
	rot = 0
	mirror = false
	hollow = 0
	_set_state(State.BOX_START)


## Marks the box [a, b] (inclusive) at once, as the two clicks would (also /box).
func mark_box(a: Vector3i, b: Vector3i) -> void:
	_box_a = a
	lo = Vector3i(mini(a.x, b.x), mini(a.y, b.y), mini(a.z, b.z))
	wsize = Vector3i(absi(a.x - b.x) + 1, absi(a.y - b.y) + 1, absi(a.z - b.z) + 1).clamp(Vector3i.ONE, Vector3i.ONE * MAX_SIDE)
	state = State.BOX_END
	player.building = true
	_left_press_box_end()


## Sets the current placement down where the crosshair points (what a click does in AIM), or with
## its bottom centre at `at`.
func place_at_aim(at: Variant = null) -> void:
	var p: Variant = at if at is Vector3i else _aim_cell(true)
	if p == null:
		var r := _ray()
		p = Vector3i((r[0] + r[1] * Settings.m2v(8.0)).floor())
	lo = (p as Vector3i) - Vector3i(wsize.x / 2, 0, wsize.z / 2)
	_set_state(State.EDIT)


## Debug (/shape drag): drags a handle `amount` voxels along its axis (move: along +x) the way a
## player does: aim at it, grab it, turn the view to the new spot, let go. Returns the hover it saw.
func drag_by(id: String, amount: float) -> String:
	var h := _handle(id)
	if h.is_empty():
		return ""
	player.capture_mouse(true)   # aim with the view, as the cursor would
	_look_at(h["pos"])
	var seen := _pick_handle()
	_hover_pt = h["pos"]
	_start_drag(h["id"])
	var axis: Vector3 = _drag["axis"] if h["id"] != "move" else Vector3.RIGHT
	_look_at((_drag["pos"] as Vector3) + axis * amount)
	_update_drag()
	_drag = {}
	_make_handles()
	return seen


func _look_at(p: Vector3) -> void:
	var eye := player.camera.global_position
	var d := p - eye
	player._yaw = atan2(-d.x, -d.z)
	player._pitch = atan2(d.y, Vector2(d.x, d.z).length())
	player._apply_look()


## Digs a marked box at once (/box go; the hold of the left button otherwise).
func dig_now() -> bool:
	if state != State.DIG_BOX:
		return false
	_dig()
	return true


func start_prefab(category: String, name: String) -> bool:
	var p := prefabs.load_prefab(category, name)
	if p.is_empty():
		_toast("Prefab %s / %s not found" % [category, name])
		return false
	start_grid(p["grid"], p["size"], name)
	return true


## Places a material grid like a prefab (the prefab menu, a blueprint's ghost).
func start_grid(p_grid: PackedByteArray, size: Vector3i, label: String) -> void:
	on_select = Callable()
	on_commit = Callable()
	mode = "prefab"
	shape = "grid"
	grid = p_grid
	grid_size = size
	prefab_label = label
	rot = 0
	mirror = false
	wsize = grid_size
	_set_state(State.AIM)


func _set_state(s: State) -> void:
	state = s
	player.building = s != State.OFF
	_drag = {}
	_rmb = {}
	if not s in [State.EDIT, State.SELECTED] and not player._mouse_captured and not player.ui_open and not menu.visible:
		player.capture_mouse(true)   # a placed shape or a selection frees the cursor (see _process); every other state looks
	_dig_progress = 0.0
	_dirty = true
	_preview_key = ""
	_full_wait = 0.0
	_preview.visible = s != State.OFF and s != State.BOX_START
	if s == State.OFF:
		_clear_handles()
		_hint.text = ""
	elif s in [State.EDIT, State.SELECTED]:
		_make_handles()
	else:
		_clear_handles()


# ---------------- input ----------------

func _unhandled_input(event: InputEvent) -> void:
	if player == null or player.autopilot or player.ui_open or player.free_cam or not GameMode.rule(player.game_mode, "interacts"):
		return
	if event is InputEventKey and event.keycode in [KEY_V, KEY_C] and not event.echo:
		if event.pressed and not menu.visible:
			_menu_opened_ms = Time.get_ticks_msec()
			if event.keycode == KEY_C:
				_open_mine_menu()
			else:
				_open_menu()
		elif not event.pressed and menu.visible and Time.get_ticks_msec() - _menu_opened_ms >= MENU_TAP_MS:
			_close_menu()
		get_viewport().set_input_as_handled()
		return
	if menu.visible:
		if event is InputEventMouseMotion:
			menu.feed(event.relative)
			get_viewport().set_input_as_handled()
		elif event is InputEventMouseButton and event.pressed and event.button_index == MOUSE_BUTTON_LEFT:
			player.click_swallowed = true
			_close_menu()
			get_viewport().set_input_as_handled()
		elif (event is InputEventMouseButton and event.pressed and event.button_index == MOUSE_BUTTON_RIGHT) \
				or (event is InputEventKey and event.pressed and event.keycode == KEY_ESCAPE):
			player.click_swallowed = event is InputEventMouseButton
			menu.selected = -1
			_close_menu()
			get_viewport().set_input_as_handled()
		elif event is InputEventMouseButton and event.pressed and _menu_kind == "mine" and event.button_index in [MOUSE_BUTTON_WHEEL_UP, MOUSE_BUTTON_WHEEL_DOWN]:
			player.step_brush_size(1 if event.button_index == MOUSE_BUTTON_WHEEL_UP else -1)
			menu.title = _mine_title()
			menu.queue_redraw()
			get_viewport().set_input_as_handled()
		return
	if state == State.OFF:
		return
	if event is InputEventMouseMotion and not _rmb.is_empty():
		_rmb["moved"] += event.relative.length()   # not handled: the player turns the view
		return
	if event is InputEventMouseButton and event.pressed and not event.ctrl_pressed and state in [State.EDIT, State.SELECTED] and _hover != "" \
			and event.button_index in [MOUSE_BUTTON_WHEEL_UP, MOUSE_BUTTON_WHEEL_DOWN]:
		if _apply(_grab(_hover)["side"], lo, wsize, hollow, snap if event.button_index == MOUSE_BUTTON_WHEEL_UP else -snap, false):
			_move_handles()
		get_viewport().set_input_as_handled()
	elif event is InputEventMouseButton:
		if event.button_index == MOUSE_BUTTON_LEFT:
			if event.pressed:
				_left_press()
			else:
				_drag = {}
			get_viewport().set_input_as_handled()
		elif event.button_index == MOUSE_BUTTON_RIGHT and state in [State.EDIT, State.SELECTED]:
			_rmb_look(event.pressed)
			get_viewport().set_input_as_handled()
		elif event.button_index == MOUSE_BUTTON_RIGHT and event.pressed:
			_cancel()
			get_viewport().set_input_as_handled()
	elif event is InputEventKey and event.pressed and not event.echo and state in [State.AIM, State.EDIT]:
		if event.keycode == KEY_R:
			turn(2 if event.shift_pressed and not on_commit.is_valid() else 1)   # a blueprint only turns about up
			get_viewport().set_input_as_handled()
		elif event.keycode == KEY_X:
			mirror = not mirror
			_changed()
			get_viewport().set_input_as_handled()
		elif event.keycode == KEY_N:
			snap = SNAPS[(SNAPS.find(snap) + 1) % SNAPS.size()]
			_toast("Snap: " + _snap_text())
			get_viewport().set_input_as_handled()


## Turns the placement: axis 1 = a quarter about up (R), else tips it toward the camera (Shift+R).
func turn(axis: int) -> void:
	var before := wsize
	var local := grid_size if mode == "prefab" else st.local_size(wsize, rot)
	if axis == 1:
		rot = st.turn(rot, 1)
	else:
		var f := -player.camera.global_transform.basis.z
		rot = st.turn(rot, 0 if absf(f.z) >= absf(f.x) else 2)
	wsize = st.oriented_size(local, rot)
	# keep it standing where it stood, centred on the same spot
	lo += Vector3i((before.x - wsize.x) / 2, 0, (before.z - wsize.z) / 2)
	_changed()


func _changed() -> void:
	_dirty = true
	if state in [State.EDIT, State.SELECTED]:
		_make_handles()


func _left_press() -> void:
	match state:
		State.AIM:
			_set_state(State.EDIT)
		State.EDIT:
			if _hover != "":
				_start_drag(_hover)
			elif not _aim_in_box():
				commit()
		State.BOX_START:
			var p: Variant = _aim_cell(mode == "box")
			if p != null:
				_box_a = p
				_set_state(State.BOX_END)
		State.BOX_END:
			_left_press_box_end()
		State.SELECTED:
			if _hover != "":
				_start_drag(_hover)
			elif on_select.is_valid():
				on_select.call()
			elif main.chat:
				main.chat.open("/prefab save builds ")   # type a name, Enter saves (category first)


func _left_press_box_end() -> void:
	if mode == "box":
		shape = "box"
		mode = "shape"
		_set_state(State.EDIT)
	elif mode == "dig":
		_prepare_dig()
	else:
		_set_state(State.SELECTED)


## The right button on a placed shape: held, it looks around (the mouse is captured meanwhile and the
## cursor comes back where it was); a plain click cancels.
func _rmb_look(pressed: bool) -> void:
	if pressed:
		_rmb = {"pos": player.camera.get_viewport().get_mouse_position(), "moved": 0.0}
		player.capture_mouse(true)
	elif not _rmb.is_empty():
		var r := _rmb
		_rmb = {}
		player.capture_mouse(false)
		player.camera.get_viewport().warp_mouse(r["pos"])
		if r["moved"] < 6.0:
			_cancel()


func _cancel() -> void:
	match state:
		State.EDIT:
			_set_state(State.AIM if mode in ["shape", "prefab"] else State.BOX_START)
		State.BOX_END, State.DIG_BOX, State.SELECTED:
			_set_state(State.BOX_START)
		_:
			_set_state(State.OFF)


# ---------------- aiming ----------------

## The aim ray: through the cursor while it is free (a placed shape), else the view centre.
func _ray() -> Array:
	var cam: Camera3D = player.camera
	if player._mouse_captured:
		return [cam.global_position, -cam.global_transform.basis.z]
	var mp := cam.get_viewport().get_mouse_position()
	return [cam.project_ray_origin(mp), cam.project_ray_normal(mp)]


func _aim_hit() -> VoxelRaycastResult:
	var r := _ray()
	return main.world.raycast(r[0], r[1], Settings.m2v(AIM_M))


## The aimed voxel (dig, select) or the voxel in front of the aimed face (build); null at the sky.
func _aim_cell(front: bool) -> Variant:
	var hit := _aim_hit()
	if hit == null:
		return null
	return hit.previous_position if front else hit.position


func _aim_in_box() -> bool:
	var r := _ray()
	return AABB(Vector3(lo), Vector3(wsize)).grow(0.5).intersects_ray(r[0], r[1]) != null


## The placement's corner on the snap grid across the ground; y stays on the aimed face so a shape
## set down on a floor stands on it. shortcut: y never snaps, add it if stacked floors drift.
func _snapped(p: Vector3i) -> Vector3i:
	return Vector3i(floori(float(p.x) / snap) * snap, p.y, floori(float(p.z) / snap) * snap)


func _snap_text() -> String:
	return "off" if snap <= 1 else Brush._len(snap)


# ---------------- frame ----------------

func _process(delta: float) -> void:
	if player == null:
		return
	if state == State.OFF:
		return
	match state:
		State.AIM:
			var p: Variant = _aim_cell(true)
			if p == null:
				var r := _ray()
				p = Vector3i((r[0] + r[1] * Settings.m2v(8.0)).floor())
			var nlo := _snapped(p - Vector3i(wsize.x / 2, 0, wsize.z / 2))
			if nlo != lo:
				lo = nlo
				_dirty = true
		State.EDIT, State.SELECTED:
			if player._mouse_captured and _rmb.is_empty() and not player.ui_open and not menu.visible:
				player.capture_mouse(false)   # the cursor drags the handles (LotL); the right button held looks
			if _drag.is_empty():
				_hover = _pick_handle()
			else:
				_update_drag()
			_style_handles()
		State.BOX_START:
			pass
		State.BOX_END:
			var b: Variant = _aim_cell(mode == "box")
			if b != null:
				var a := _box_a
				var bb: Vector3i = b
				var nlo := Vector3i(mini(a.x, bb.x), mini(a.y, bb.y), mini(a.z, bb.z))
				var ns := Vector3i(absi(a.x - bb.x) + 1, absi(a.y - bb.y) + 1, absi(a.z - bb.z) + 1).clamp(Vector3i.ONE, Vector3i.ONE * MAX_SIDE)
				if nlo != lo or ns != wsize:
					lo = nlo
					wsize = ns
					_dirty = true
		State.DIG_BOX:
			if Input.is_mouse_button_pressed(MOUSE_BUTTON_LEFT) and player._mouse_captured:
				_dig_progress += delta / maxf(_dig_time, 0.001) if _dig_time > 0.0 else 1.0
				if player.hud:
					player.hud.break_progress = _dig_progress
				if _dig_progress >= 1.0:
					_dig()
	_since_preview += delta
	if _dirty and _since_preview >= 1.0 / PREVIEW_HZ:
		_rebuild_preview()
	_tick_full_preview(delta)
	_update_hint()


func _params() -> Dictionary:
	var local := grid_size if mode == "prefab" else st.local_size(wsize, rot)
	var p := {"shape": "box" if state in [State.BOX_END, State.DIG_BOX, State.SELECTED] else shape,
		"size": local, "hollow": hollow, "step": step, "rot": rot, "mirror": mirror}
	if mode == "prefab":
		p["grid"] = grid
	return p


func _window() -> Vector3i:
	return bubble.origin * Settings.CHUNK


func _rebuild_preview() -> void:
	_dirty = false
	_since_preview = 0.0
	var outline := state in [State.BOX_END, State.DIG_BOX, State.SELECTED] and mode != "box"
	var p := _params()
	if outline:
		p = {"shape": "box", "size": wsize, "hollow": 1}   # a dig or a selection shows as a frame
	_preview.position = Vector3(lo)
	_preview.visible = true
	var key := str(p)
	var f := 1 if mode == "prefab" else maxi(1, ceili(pow(float(wsize.x) * wsize.y * wsize.z / PREVIEW_CELLS, 1.0 / 3.0)))
	if f > 1:
		_full_wait = FULL_AFTER_S
		if key != _preview_key:
			var q := p.duplicate()
			q["size"] = (Vector3i(p["size"]) / f).maxi(1)
			q["hollow"] = 0 if int(p.get("hollow", 0)) == 0 else maxi(1, int(p["hollow"]) / f)
			if p.has("step"):
				q["step"] = maxi(1, int(p["step"]) / f)
			var coarse := st.cells(q, Vector3i.ZERO)
			if coarse.is_empty():
				_preview.visible = false
				return
			_set_preview_mesh(_mesh_of(st, coarse, Vector3i.ZERO))
			_preview.scale = Vector3(wsize) / Vector3(coarse["size"])
			_preview_key = key
		_style_ghost(outline)
		return
	var r := st.cells(p, lo)
	if r.is_empty():
		_preview.visible = false
		return
	_set_preview_mesh(_mesh_of(st, r, lo))
	_preview.scale = Vector3.ONE
	_preview_key = key
	_short = false
	_need_text = ""
	if mode in ["shape", "prefab"] and not outline:
		_update_need(r)
	_style_ghost(outline)


func _mesh_of(tool: ShapeTool, r: Dictionary, at: Vector3i) -> ArrayMesh:
	var arr: Array = tool.mesh(r["cells"], at, r["size"])
	var m := ArrayMesh.new()
	if not (arr[Mesh.ARRAY_INDEX] as PackedInt32Array).is_empty():
		m.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arr)
		m.surface_set_material(0, _depth_mat)
	return m


func _set_preview_mesh(m: ArrayMesh) -> void:
	_preview.mesh = m


## A big shape that came to rest gets its full detail and its bill from a worker; a shape that changed
## meanwhile drops the result.
func _tick_full_preview(delta: float) -> void:
	if _full_task >= 0:
		if not WorkerThreadPool.is_task_completed(_full_task):
			return
		WorkerThreadPool.wait_for_task_completion(_full_task)
		_full_task = -1
		if _full_result["key"] == _preview_key and state != State.OFF:
			_set_preview_mesh(_full_result["mesh"])
			_preview.scale = Vector3.ONE
			if _full_result["lo"] == lo and mode == "shape" and state in [State.AIM, State.EDIT]:
				_short = false
				_need_text = ""
				_update_need(_full_result["r"])
				_style_ghost(false)
		_full_result = {}
	if _full_wait <= 0.0:
		return
	_full_wait -= delta
	if _full_wait <= 0.0 and not _dirty:
		var outline := state in [State.BOX_END, State.DIG_BOX, State.SELECTED] and mode != "box"
		var p := {"shape": "box", "size": wsize, "hollow": 1} if outline else _params()
		_full_task = WorkerThreadPool.add_task(_full_preview_job.bind(p, lo, _preview_key))


func _full_preview_job(p: Dictionary, at: Vector3i, key: String) -> void:
	var tool := ShapeTool.new()
	var r := tool.cells(p, at)
	_full_result = {"key": key, "lo": at, "r": r, "mesh": _mesh_of(tool, r, at) if not r.is_empty() else ArrayMesh.new()}


func _style_ghost(outline: bool) -> void:
	var col := Color(0.55, 0.75, 1.0, GHOST_OPACITY)
	if outline:
		col = Color(1.0, 0.45, 0.3, 0.4) if mode == "dig" else Color(0.95, 0.6, 0.0, 0.5)
	elif _short:
		col = Color(1.0, 0.25, 0.2, GHOST_OPACITY)
	# a shape of one material looks like that material (ADR 0144, red when short); frames and prefabs show the colour alone
	var held := _held_material() if mode == "shape" and not outline else 0
	var src: ShaderMaterial = player.brush_tool.ghost_mat
	for pn in BrushTool.ATLAS_PARAMS:
		_ghost_mat.set_shader_parameter(pn, src.get_shader_parameter(pn))
	if held > 0:
		var t: Array = GameMaterials.defs()[held - 1]["tiles"]
		_ghost_mat.set_shader_parameter("tiles", Vector3(float(t[0]), float(t[1]), float(t[2])))
	_ghost_mat.set_shader_parameter("tint", Color(col, 1.0))
	_ghost_mat.set_shader_parameter("tint_mix", 1.0 if held <= 0 else (0.5 if _short else GHOST_TINT))
	_ghost_mat.set_shader_parameter("rim", 0.2 if held <= 0 else 0.0)
	_ghost_mat.set_shader_parameter("opacity", col.a)


## What the placement needs and whether the inventory has it.
func _update_need(r: Dictionary) -> void:
	if on_commit.is_valid():
		return   # a blueprint marks a site: nothing is paid now
	var need := _need_of(r)
	if not GameMode.rule(player.game_mode, "consumes_items"):
		_need_text = "%d voxels" % _total(need)
		return
	var parts := []
	for m in need:
		var item := player.inventory.items.item_for_material(GameMaterials.name_of(m))
		var have := _have(item)
		if have < int(need[m]):
			_short = true
		parts.append("%s %d/%d" % [player.inventory.items.display_name(item) if item != "" else GameMaterials.name_of(m), have, need[m]])
	_need_text = ", ".join(parts)


func _need_of(r: Dictionary) -> Dictionary:
	if mode == "prefab":
		return st.count_free_by_material(bubble.sim, r["cells"], r["mats"], _window(), _mask_free)
	var mat := _held_material()
	if mat <= 0:
		return {}
	return {mat: st.count_free(bubble.sim, r["cells"], _window(), _mask_free)}


static func _total(need: Dictionary) -> int:
	var n := 0
	for m in need:
		n += int(need[m])
	return n


func _have(item: String) -> int:
	if item == "":
		return 0
	var inv: PlayerInventory = player.inventory
	return inv.inv.count_of(item) + (inv.bag.count_of(item) if inv.has_bag() else 0)


func _pay(item: String, n: int) -> void:
	var inv: PlayerInventory = player.inventory
	var left := n - inv.inv.remove(item, n)
	if left > 0 and inv.has_bag():
		inv.bag.remove(item, left)


func _update_hint() -> void:
	var m := Settings.VOXEL_M
	var size_text := "%s x %s x %s m" % [snappedf(wsize.x * m, 0.01), snappedf(wsize.y * m, 0.01), snappedf(wsize.z * m, 0.01)]
	var round_shape: bool = mode == "shape" and shape in ["cylinder", "cone", "sphere"]
	var drag_name := {"top": "Adjust Height", "ring": "Adjust Diameter" if round_shape else "Adjust Size", "hollow": "Adjust Hollow", "move": "Move"}
	var on_handle: String = _drag.get("id", _hover)
	var build_name: String = prefab_label if mode == "prefab" else shape.capitalize()
	var lines := PackedStringArray()
	match state:
		State.AIM:
			lines = ["Build Mode: %s" % build_name, "Size: %s" % size_text, _need_text,
				"(V) Change Build Mode", "(R) Turn   (Shift+R) Tip   (X) Mirror", "(N) Snap: %s" % _snap_text(), "(LMB) Set It Down", "(RMB) Leave"]
		State.EDIT:
			lines = ["Build Mode: %s" % build_name, "Height: %s m" % snappedf(wsize.y * m, 0.01),
				"Diameter: %s m" % snappedf(wsize.x * m, 0.01) if round_shape and wsize.x == wsize.z else "Width: %s x %s m" % [snappedf(wsize.x * m, 0.01), snappedf(wsize.z * m, 0.01)],
				"Hollow: %s m" % snappedf(hollow * m, 0.01) if hollow > 0 else "", _need_text,
				"(V) Change Build Mode", "(R) Turn   (X) Mirror",
				"(LMB) %s   (Wheel) Step: %s" % [drag_name[on_handle], _snap_text()] if on_handle != "" else ("(LMB) Mark the Site" if on_commit.is_valid() else "(LMB) Build It"),
				"(RMB) Cancel, Hold: Look Around"]
		State.BOX_START:
			lines = [{"box": "Build mode: Box fill", "dig": "Dig mode: Box", "select": "Select"}.get(mode, ""), "(LMB) Set the first corner", "(RMB) Leave"]
		State.BOX_END:
			lines = ["Size: %s" % size_text, "(LMB) Set the second corner", "(RMB) Cancel"]
		State.DIG_BOX:
			lines = ["Dig mode: Box", "Size: %s" % size_text, "(LMB) Hold to dig", "(RMB) Another box"]
		State.SELECTED:
			lines = ["Selected: %s" % size_text, "(LMB) %s" % drag_name[on_handle] if on_handle != "" else ("(LMB) Copy It" if on_select.is_valid() else "(LMB) Name and Save It"), "(RMB) Another Box"]
	if _short and state in [State.AIM, State.EDIT]:
		lines.append("Not enough material")
	var shown := PackedStringArray()
	for l in lines:
		if l != "":
			shown.append(l)
	_hint.text = "\n".join(shown)


# ---------------- handles ----------------

func _clear_handles() -> void:
	for h in _handles:
		(h["node"] as Node).queue_free()
	_handles = []
	_hover = ""


func _make_handles() -> void:
	_clear_handles()
	var c := Vector3(lo) + Vector3(wsize) * Vector3(0.5, 0.0, 0.5)
	var top := float(lo.y + wsize.y)
	var list := [["move", Vector3(c.x, float(lo.y) + 0.5, c.z), Vector3.ZERO]]   # at the foot's centre, as the reference
	if mode != "prefab":
		# LotL's rings (a 5th entry is the ring radius): the outer one at the foot grows the side you
		# grab, the inner one on top hollows (its radius is what stays open; shapes only, not a selection)
		list.append_array([
			["top", Vector3(c.x, top + 3.0, c.z), Vector3.UP],
			["ring", Vector3(c.x, float(lo.y) + 0.5, c.z), Vector3.ZERO, maxf(wsize.x, wsize.z) * 0.5 + 1.5],
		])
		if state == State.EDIT:
			list.append(["hollow", Vector3(c.x, top + 0.5, c.z), Vector3.ZERO, maxf(minf(wsize.x, wsize.z) * 0.5 - hollow, 1.0)])
	for e in list:
		var mi := MeshInstance3D.new()
		if e[0] == "top":
			mi.mesh = GizmoMesh.arrow()
		elif e[0] == "move":
			mi.mesh = GizmoMesh.cross()
		mi.material_override = _handle_mat.duplicate()
		mi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
		add_child(mi)
		mi.position = e[1]
		var h := {"id": e[0], "node": mi, "pos": e[1], "axis": e[2]}
		if e.size() > 3:
			h["radius"] = e[3]
		_handles.append(h)
	_style_handles()


func _style_handles() -> void:
	var eye := player.camera.global_position
	for h in _handles:
		var mi: MeshInstance3D = h["node"]
		var on: bool = h["id"] == _hover or h["id"] == _drag.get("id", "")
		var s := HANDLE * maxf(1.0, eye.distance_to(h["pos"]) / Settings.m2v(4.0))
		if h.has("radius"):
			var band := Vector2(h["radius"], snappedf(minf(s * 0.3, float(h["radius"]) * 0.5), 0.25))
			if h.get("band", Vector2.ZERO) != band:   # the same on screen at any distance: rebuilt as you walk
				h["band"] = band
				mi.mesh = GizmoMesh.ring(band.x, band.y)
		else:
			mi.scale = Vector3.ONE * s
		(mi.material_override as StandardMaterial3D).albedo_color = GizmoMesh.HOVER if on else Color.WHITE


func _pick_handle() -> String:
	var r := _ray()
	var o: Vector3 = r[0]
	var d: Vector3 = r[1]
	var best := ""
	var best_d := INF
	for h in _handles:
		var p: Vector3 = h["pos"]
		var t: float
		var hit: Vector3
		var dist: float
		if h.has("radius"):   # a ring: where the ray crosses its plane, how far from the ring line
			if absf(d.y) < 1e-4:
				continue
			t = (p.y - o.y) / d.y
			hit = o + d * t
			dist = absf(Vector2(hit.x - p.x, hit.z - p.z).length() - float(h["radius"]))
		else:
			t = (p - o).dot(d)
			hit = p
			dist = (o + d * t - p).length()
		if t <= 0.0:
			continue
		var reach := HANDLE * maxf(1.0, t / Settings.m2v(4.0)) * 0.9
		if dist < reach and t < best_d:
			best_d = t
			best = h["id"]
			_hover_pt = hit
	return best


## The handle `id`; a side (+x, -x, +z, -z) or hollow names the point on its ring at that side
## (/shape drag), with the axis a drag there works along.
func _handle(id: String) -> Dictionary:
	const SIDES := {"+x": Vector3.RIGHT, "-x": Vector3.LEFT, "+z": Vector3.BACK, "-z": Vector3.FORWARD, "hollow": Vector3.RIGHT}
	var want := "ring" if SIDES.has(id) and id != "hollow" else id
	for h in _handles:
		if h["id"] != want:
			continue
		if SIDES.has(id):
			var dir: Vector3 = SIDES[id]
			return {"id": want, "pos": (h["pos"] as Vector3) + dir * float(h["radius"]), "axis": -dir if id == "hollow" else dir}
		return h
	return {}


## What a grab of handle `id` at `_hover_pt` works: {side, pos, axis}. A ring grows the side nearest
## the grab (the hollow ring: inward).
func _grab(id: String) -> Dictionary:
	var h := _handle(id)
	if not h.has("radius"):
		return {"side": id, "pos": h["pos"], "axis": h["axis"]}
	var v: Vector3 = _hover_pt - (h["pos"] as Vector3)
	var dir := Vector3.RIGHT * (1.0 if v.x >= 0.0 else -1.0) if absf(v.x) >= absf(v.z) else Vector3.BACK * (1.0 if v.z >= 0.0 else -1.0)
	var side: String = "hollow" if id == "hollow" else {Vector3.RIGHT: "+x", Vector3.LEFT: "-x", Vector3.BACK: "+z", Vector3.FORWARD: "-z"}[dir]
	return {"side": side, "pos": (h["pos"] as Vector3) + dir * float(h["radius"]), "axis": -dir if id == "hollow" else dir}


func _start_drag(id: String) -> void:
	var g := _grab(id)
	_drag = {"id": id, "side": g["side"], "pos": g["pos"], "axis": g["axis"], "lo": lo, "wsize": wsize, "hollow": hollow,
		"s0": _axis_param(g["pos"], g["axis"]), "plane0": _plane_point(float(g["pos"].y)), "shift": Input.is_key_pressed(KEY_SHIFT)}


## Where the aim ray passes closest to the line through p along axis: the parameter along it.
func _axis_param(p: Vector3, axis: Vector3) -> float:
	var r := _ray()
	var o: Vector3 = r[0]
	var d: Vector3 = r[1]
	var w := o - p
	var b := axis.dot(d)
	var den := 1.0 - b * b
	if den < 1e-4:
		return 0.0
	return (axis.dot(w) - b * d.dot(w)) / den


func _plane_point(y: float) -> Vector3:
	var r := _ray()
	var o: Vector3 = r[0]
	var d: Vector3 = r[1]
	if absf(d.y) < 1e-4:
		return o
	var t := (y - o.y) / d.y
	if t < 0.0 or t > Settings.m2v(AIM_M) * 2.0:
		return o + d * Settings.m2v(AIM_M)
	return o + d * t


func _update_drag() -> void:
	var id: String = _drag["id"]
	var l0: Vector3i = _drag["lo"]
	var changed := false
	if id == "move":
		var dp: Vector3 = _plane_point(float(_drag["pos"].y)) - (_drag["plane0"] as Vector3)
		var nlo := l0 + Vector3i(_snap_step(dp.x), 0, _snap_step(dp.z))
		changed = nlo != lo
		lo = nlo
	else:
		var dv := _snap_step(_axis_param(_drag["pos"], _drag["axis"]) - float(_drag["s0"]))
		changed = _apply(_drag["side"], l0, _drag["wsize"], int(_drag["hollow"]), dv, _drag["shift"])
	if changed:
		_dirty = true
		_move_handles()


## A drag distance in voxels, in whole snap steps.
func _snap_step(v: float) -> int:
	return roundi(v / snap) * snap


## Moves handle `id` by `dv` voxels from the placement (l0, s0, h0): sizes along its axis, the top
## handle with shift (or the move handle) lifts instead; hollow in voxels. True when anything changed.
func _apply(id: String, l0: Vector3i, s0: Vector3i, h0: int, dv: int, shift: bool) -> bool:
	var nlo := l0
	var ns := s0
	match id:
		"move":
			nlo.y = l0.y + dv
		"top":
			if shift:
				nlo.y = l0.y + dv
			else:
				ns.y = s0.y + dv
		"+x":
			ns.x = s0.x + dv
		"-x":
			ns.x = s0.x + dv
			nlo.x = l0.x - (clampi(ns.x, 1, MAX_SIDE) - s0.x)
		"+z":
			ns.z = s0.z + dv
		"-z":
			ns.z = s0.z + dv
			nlo.z = l0.z - (clampi(ns.z, 1, MAX_SIDE) - s0.z)
		"hollow":
			var h := clampi(h0 + signi(dv) * maxi(1, absi(dv) / snap), 0, maxi(0, mini(s0.x, s0.z) / 2 - 1))
			if h != hollow:
				hollow = h
				_dirty = true
				return true
			return false
	ns = ns.clamp(Vector3i.ONE, Vector3i.ONE * MAX_SIDE)
	if nlo == lo and ns == wsize:
		return false
	lo = nlo
	wsize = ns
	_dirty = true
	return true


func _move_handles() -> void:
	var keep := _drag
	_make_handles()
	_drag = keep


# ---------------- commit ----------------

## Builds the placement; returns the voxels written (0 when refused).
func commit() -> int:
	var t0 := Time.get_ticks_usec()
	refusal = ""
	var r := st.cells(_params(), lo)
	if r.is_empty():
		return _refuse("Nothing to build")
	if on_commit.is_valid():
		on_commit.call(lo, rot, mirror)
		_set_state(State.OFF)
		return 0
	if not _inside_window():
		return _refuse("Too far: build within the area around you")
	var body := _player_box()
	if body.intersects(AABB(Vector3(lo), Vector3(wsize))) and _hits_body(r):
		return _refuse("You are standing in it")
	var need := _need_of(r)
	var creative := not GameMode.rule(player.game_mode, "consumes_items")
	var pay := {}
	for m in need:
		var item := player.inventory.items.item_for_material(GameMaterials.name_of(m))
		if not creative:
			var have := _have(item)
			if have < int(need[m]):
				return _refuse("Needs %d more %s" % [int(need[m]) - have, player.inventory.items.display_name(item) if item != "" else GameMaterials.name_of(m)])
		pay[item] = int(need[m])
	var mat := _held_material() if mode != "prefab" else 0
	var n := st.fill(bubble.sim, r["cells"], mat, r["mats"], _window(), _mask_free)
	if Net.active:
		Net.send_op(NetOps.fill(Vector3i.ZERO, r["cells"], mat, _mask_free, r["mats"]))   # ADR 0145
	if not creative:
		for item in pay:
			_pay(item, pay[item])
	# the last build, to place again from the Prefabs menu
	var g := grid if mode == "prefab" else st.grid_of(r["cells"], mat, PackedByteArray(), lo, wsize)
	var gs := grid_size if mode == "prefab" else wsize
	if mode == "prefab" and (rot != 0 or mirror):
		g = st.grid_of(r["cells"], 0, r["mats"], lo, wsize)
		gs = wsize
	prefabs.save_last(gs, g)
	stats["built"] += 1
	stats["voxels"] += n
	stats["last_ms"] = (Time.get_ticks_usec() - t0) / 1000.0
	stats["max_ms"] = maxf(stats["max_ms"], stats["last_ms"])
	_toast("Built %d voxels" % n)
	_set_state(State.AIM if mode in ["shape", "prefab"] else State.BOX_START)
	return n


func _refuse(why: String) -> int:
	refusal = why
	_toast(why)
	return 0


func _inside_window() -> bool:
	var hi := lo + wsize - Vector3i.ONE
	for c in [lo, hi, Vector3i(lo.x, hi.y, hi.z), Vector3i(hi.x, lo.y, lo.z)]:
		var wc := Vector3i((Vector3(c) / Settings.CHUNK).floor())
		if not bubble.is_chunk_in_window(wc - bubble.origin) or bubble._is_unloaded(wc):
			return false
	return true


func _player_box() -> AABB:
	var rr := Settings.m2v(Settings.player_radius_m)
	return AABB(player.position - Vector3(rr, 0, rr), Vector3(2 * rr, Settings.m2v(Settings.player_height_m), 2 * rr))


## Does any cell of the shape fall inside the player's body (a few cells, checked natively)?
func _hits_body(r: Dictionary) -> bool:
	var b := _player_box()
	var blo := Vector3i(b.position.floor())
	var bs := Vector3i(b.end.ceil()) - blo
	var mine: PackedByteArray = st.grid_of(r["cells"], 1, PackedByteArray(), blo, bs)
	return mine.count(1) > 0


# ---------------- box dig ----------------

func _prepare_dig() -> void:
	var held: Dictionary = player.inventory.held_def()
	var creative := GameMode.rule(player.game_mode, "instant_break")
	var mask := PackedByteArray()
	mask.resize(256)
	var times := {}
	for i in GameMaterials.defs().size():
		var m := i + 1
		if not (GameMaterials.is_plain_voxel(m) or player._is_plant(m)) or main.farming.is_crop_material(m):
			continue
		if not player._dig_only_mask.is_empty() and player._dig_only_mask[m] == 0:
			continue
		var t := 0.0 if creative else (player.mining.break_time(GameMaterials.name_of(m), held) if player.mining else 0.0)
		if t < 0.0:
			continue
		mask[m] = 1
		times[m] = maxf(t, 0.0)
	var cells := st.select(bubble.sim, lo, wsize, _window(), mask)
	if cells.is_empty():
		_toast("Nothing here the held tool can dig")
		return
	var hist: Dictionary = st.histogram(bubble.sim, cells, _window())
	var total := 0.0
	var per := float(Settings.BLOCK * Settings.BLOCK * Settings.BLOCK)
	for m in hist:
		total += float(times.get(m, 0.0)) * int(hist[m]) / per
	_dig_time = 0.0 if creative else maxf(total, 0.12)
	_drag = {"cells": cells, "mask": mask, "hist": hist}
	state = State.DIG_BOX
	_dig_progress = 0.0
	_dirty = true


func _dig() -> void:
	var cells: PackedInt32Array = _drag.get("cells", PackedInt32Array())
	if cells.is_empty():
		_set_state(State.BOX_START)
		return
	# the world may have changed while digging: take what is still there
	cells = st.select(bubble.sim, lo, wsize, _window(), _drag["mask"])
	var hist: Dictionary = st.histogram(bubble.sim, cells, _window())
	var counts := {}
	for m in hist:
		counts[GameMaterials.name_of(m)] = int(hist[m])
	var mask: PackedByteArray = _drag["mask"]
	var n := st.fill(bubble.sim, cells, GameMaterials.AIR, PackedByteArray(), _window(), mask)
	if Net.active:
		Net.send_op(NetOps.fill(Vector3i.ZERO, cells, GameMaterials.AIR, mask))   # ADR 0145
	var centre := Vector3(lo) + Vector3(wsize) * 0.5
	if player.mining and GameMode.rule(player.game_mode, "drops"):
		var drops: Array = player.mining.voxel_drops(counts, player.inventory.held_def(), player.inventory.items.units,
			func(nm: String) -> bool: return GameMaterials.is_plain_voxel(GameMaterials.id_of(nm)),
			player._dig_carry, player._drop_rng, Settings.BLOCK * Settings.BLOCK * Settings.BLOCK)
		for d in drops:
			main.spawn_item(d, centre, Vector3(randf_range(-1, 1), 2.0, randf_range(-1, 1)))
	if main.fx and not hist.is_empty():
		main.fx.block_break(Vector3i(centre.floor()), int(hist.keys()[0]))
	if main.pieces:
		main.pieces.rp.queue_check_packed(cells, player.position)
	if main.support:
		main.support.dug_packed(cells)
	if player.hud:
		player.hud.break_progress = 0.0
	stats["dug"] += n
	_set_state(State.BOX_START)


# ---------------- prefabs ----------------

## Saves the selected box (trimmed to what it holds) as a prefab; returns the path or "".
func save_selection(category: String, name: String) -> String:
	if state != State.SELECTED:
		return ""
	var g := st.read(bubble.sim, lo, wsize, _window())
	var t: Dictionary = st.trim(wsize, _clean_grid(g))
	if t.is_empty():
		return ""
	stats["saved"] += 1
	return prefabs.save(category, name, t["size"], t["grid"])


## A selection keeps what was built: air, liquids, gases and plants become nothing.
func _clean_grid(g: PackedByteArray) -> PackedByteArray:
	var keep := PackedByteArray()
	keep.resize(256)
	for i in GameMaterials.defs().size():
		var m := i + 1
		keep[m] = 1 if GameMaterials.is_plain_voxel(m) else 0
	var r := st.runs(g)
	for k in range(0, r.size(), 2):
		r[k] = r[k] if keep[r[k]] == 1 else 0
	return st.unruns(r)


func _toast(text: String) -> void:
	if player.hud:
		player.hud.show_toast(text)


func snapshot() -> Dictionary:
	return {"state": State.keys()[state], "mode": mode, "shape": shape, "lo": lo, "size": wsize, "rot": rot, "mirror": mirror,
		"hollow": hollow, "need": _need_text, "short": _short, "stats": stats}
