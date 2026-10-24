class_name ShapeBuilder
extends Node3D

## Shape building, box mode and prefabs (ADR 0038, Lay of the Land's building). Hold V for the build
## menu. With a material in hand a shape's preview follows the aim; a left click sets it down; its
## handles (aimed with the crosshair, dragged by holding the left button and turning) set height,
## sides, hollow and position; R turns it, Shift+R tips it, X mirrors it; a left click away builds it
## and takes the voxels from the inventory; a right click cancels. Box mode marks a box with two clicks
## (to build, or with a tool to dig); Select marks one to save as a prefab (/prefab save). GDScript
## orchestrates: cells, meshes and writes are native (ShapeTool).

const SHAPES := ["box", "wall", "cylinder", "cone", "sphere", "stairs", "slope"]
const DEFAULT_SIZE := {"box": Vector3i(8, 8, 8), "wall": Vector3i(32, 24, 4), "cylinder": Vector3i(16, 24, 16),
	"cone": Vector3i(16, 16, 16), "sphere": Vector3i(16, 16, 16), "stairs": Vector3i(8, 16, 16), "slope": Vector3i(8, 8, 16)}
const MAX_SIDE := 256          # voxels (32 m) per side
const AIM_M := 32.0            # how far the builder aims (further than the hand reaches: big shapes)
const HANDLE := 1.6            # handle cube size, voxels at 4 m (they grow with distance)
const PREVIEW_HZ := 30.0

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
var grid := PackedByteArray()    # a prefab's materials (local)
var grid_size := Vector3i.ZERO
var prefab_label := ""
var refusal := ""           # why the last commit was refused ("" when it built)
var stats := {"built": 0, "voxels": 0, "dug": 0, "saved": 0, "last_ms": 0.0, "max_ms": 0.0}

var _box_a := Vector3i.ZERO
var _layer: CanvasLayer
var _hint: Label
var _preview: MeshInstance3D
var _preview_mat: StandardMaterial3D
var _handles: Array = []    # [{id, node, pos, axis}]
var _hover := ""
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
var _handle_mat: StandardMaterial3D   # the handles' look (each handle gets a copy for its colour)


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
	_hint.set_anchors_preset(Control.PRESET_CENTER_TOP)
	_hint.position = Vector2(-360, 64)
	_hint.size = Vector2(720, 60)
	_hint.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	_hint.add_theme_font_size_override("font_size", 16)
	_hint.add_theme_color_override("font_outline_color", Color.BLACK)
	_hint.add_theme_constant_override("outline_size", 5)
	_layer.add_child(_hint)
	_preview = MeshInstance3D.new()
	_preview.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	_preview_mat = StandardMaterial3D.new()
	_preview_mat.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	_preview_mat.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	_preview_mat.no_depth_test = false
	_preview.visible = false
	add_child(_preview)
	_handle_mat = StandardMaterial3D.new()
	_handle_mat.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	_handle_mat.no_depth_test = true
	_handle_mat.render_priority = 1
	_mask_free.resize(256)
	for i in GameMaterials.defs().size():
		var d: Dictionary = GameMaterials.defs()[i]
		if d.get("replaceable", false) or d.get("cell_type", "solid") in ["gas", "fire"]:
			_mask_free[i + 1] = 1
	_mask_free[GameMaterials.AIR] = 1


## Materials MaterialWarmup draws under the loading screen (a first draw compiles a pipeline).
func warm_materials() -> Array:
	return [_preview_mat, _handle_mat]


func is_active() -> bool:
	return state != State.OFF


# ---------------- the menu ----------------

func _held_material() -> int:
	var def: Dictionary = player.inventory.held_def()
	if def.is_empty() or not player.inventory.items.is_voxel_item(def["id"]):
		return 0
	return ItemIcon.material_id_for(player.inventory.items, def["id"])


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
	elif id.begins_with("shape:"):
		start_shape(id.substr(6))
	elif id in ["box", "dig", "select"]:
		start_box(id)
	elif id == "prefabs":
		_open_prefab_menu.call_deferred()
	elif id.begins_with("prefab:"):
		var parts := id.substr(7).split("/")
		start_prefab(parts[0], parts[1])


## Starts placing a shape with the held material (also /shape).
func start_shape(p_shape: String, size := Vector3i.ZERO) -> void:
	mode = "shape"
	shape = p_shape
	rot = 0
	mirror = false
	hollow = 0
	wsize = size if size != Vector3i.ZERO else DEFAULT_SIZE.get(shape, Vector3i(8, 8, 8))
	_set_state(State.AIM)


## Box mode ("box" builds, "dig" digs with the held tool, "select" marks a prefab): two clicks.
func start_box(p_mode: String) -> void:
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
	_look_at(h["pos"])
	var seen := _pick_handle()
	_start_drag(id)
	var axis: Vector3 = h["axis"] if h["id"] != "move" else Vector3.RIGHT
	_look_at((h["pos"] as Vector3) + axis * amount)
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
	mode = "prefab"
	shape = "grid"
	grid = p["grid"]
	grid_size = p["size"]
	prefab_label = name
	rot = 0
	mirror = false
	wsize = grid_size
	_set_state(State.AIM)
	return true


func _set_state(s: State) -> void:
	state = s
	player.building = s != State.OFF
	_drag = {}
	_dig_progress = 0.0
	_dirty = true
	_preview.visible = s != State.OFF and s != State.BOX_START
	if s == State.OFF:
		_clear_handles()
		_hint.text = ""
	elif s == State.EDIT:
		_make_handles()
	else:
		_clear_handles()


# ---------------- input ----------------

func _unhandled_input(event: InputEvent) -> void:
	if player == null or player.autopilot or player.ui_open or player.free_cam or not GameMode.rule(player.game_mode, "interacts"):
		return
	if event is InputEventKey and event.keycode == KEY_V and not event.echo:
		if event.pressed and not menu.visible:
			_open_menu()
		elif not event.pressed and menu.visible:
			_close_menu()
		get_viewport().set_input_as_handled()
		return
	if menu.visible:
		if event is InputEventMouseMotion:
			menu.feed(event.relative)
			get_viewport().set_input_as_handled()
		elif event is InputEventMouseButton and event.pressed and event.button_index == MOUSE_BUTTON_LEFT and _menu_kind == "prefabs":
			_close_menu()
		return
	if state == State.OFF:
		return
	if event is InputEventMouseButton:
		if event.button_index == MOUSE_BUTTON_LEFT:
			if event.pressed:
				_left_press()
			else:
				_drag = {}
			get_viewport().set_input_as_handled()
		elif event.button_index == MOUSE_BUTTON_RIGHT and event.pressed:
			_cancel()
			get_viewport().set_input_as_handled()
	elif event is InputEventKey and event.pressed and not event.echo and state in [State.AIM, State.EDIT]:
		if event.keycode == KEY_R:
			turn(2 if event.shift_pressed else 1)
			get_viewport().set_input_as_handled()
		elif event.keycode == KEY_X:
			mirror = not mirror
			_changed()
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
	if state == State.EDIT:
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
			if main.chat:
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


func _cancel() -> void:
	match state:
		State.EDIT:
			_set_state(State.AIM if mode in ["shape", "prefab"] else State.BOX_START)
		State.BOX_END, State.DIG_BOX, State.SELECTED:
			_set_state(State.BOX_START)
		_:
			_set_state(State.OFF)


# ---------------- aiming ----------------

func _ray() -> Array:
	var cam: Camera3D = player.camera
	return [cam.global_position, -cam.global_transform.basis.z]


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
			var nlo: Vector3i = p - Vector3i(wsize.x / 2, 0, wsize.z / 2)
			if nlo != lo:
				lo = nlo
				_dirty = true
		State.EDIT:
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
	var r := st.cells(p, lo)
	if r.is_empty():
		_preview.visible = false
		return
	var arr: Array = st.mesh(r["cells"], lo, r["size"])
	var m := ArrayMesh.new()
	if not (arr[Mesh.ARRAY_INDEX] as PackedInt32Array).is_empty():
		m.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arr)
		m.surface_set_material(0, _preview_mat)
	_preview.mesh = m
	_preview.position = Vector3(lo)
	_preview.visible = true
	_short = false
	_need_text = ""
	if mode in ["shape", "prefab"] and not outline:
		_update_need(r)
	var col := Color(0.35, 0.85, 1.0, 0.38)
	if outline:
		col = Color(1.0, 0.45, 0.3, 0.4) if mode == "dig" else Color(1.0, 0.95, 0.4, 0.35)
	elif _short:
		col = Color(1.0, 0.25, 0.2, 0.42)
	_preview_mat.albedo_color = col


## What the placement needs and whether the inventory has it.
func _update_need(r: Dictionary) -> void:
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
	match state:
		State.AIM:
			_hint.text = "%s %s   click to set down, R turn, Shift+R tip, X mirror, right click / V: leave\n%s" % [
				prefab_label if mode == "prefab" else shape.capitalize(), size_text, _need_text]
		State.EDIT:
			_hint.text = "%s %s%s   drag the handles, click away to build, right click to move it again\n%s" % [
				prefab_label if mode == "prefab" else shape.capitalize(), size_text, "  hollow %d" % hollow if hollow > 0 else "", _need_text]
		State.BOX_START:
			_hint.text = {"box": "Box: click the first corner", "dig": "Box dig: click the first corner", "select": "Select: click the first corner"}.get(mode, "")
		State.BOX_END:
			_hint.text = "%s   click the second corner" % size_text
		State.DIG_BOX:
			_hint.text = "Dig %s: hold the left button   right click: another box" % size_text
		State.SELECTED:
			_hint.text = "Selected %s   click to name and save it   right click: another box" % size_text
	if _short and state in [State.AIM, State.EDIT]:
		_hint.text += "   (not enough)"


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
	var mid := float(lo.y) + wsize.y * 0.5
	var list := [["move", Vector3(c.x, top + 3.0, c.z), Vector3.ZERO, Color(1, 1, 1)]]
	if mode != "prefab":
		list.append_array([
			["top", Vector3(c.x, top + 1.0, c.z), Vector3.UP, Color(0.3, 1.0, 0.4)],
			["+x", Vector3(lo.x + wsize.x + 1.0, mid, c.z), Vector3.RIGHT, Color(1.0, 0.35, 0.3)],
			["-x", Vector3(lo.x - 1.0, mid, c.z), Vector3.LEFT, Color(1.0, 0.35, 0.3)],
			["+z", Vector3(c.x, mid, lo.z + wsize.z + 1.0), Vector3.BACK, Color(0.35, 0.55, 1.0)],
			["-z", Vector3(c.x, mid, lo.z - 1.0), Vector3.FORWARD, Color(0.35, 0.55, 1.0)],
			["hollow", Vector3(lo.x + maxf(hollow, 0.5), top + 1.0, c.z), Vector3.RIGHT, Color(1.0, 0.9, 0.3)],
		])
	for e in list:
		var mi := MeshInstance3D.new()
		var bm := BoxMesh.new()
		bm.size = Vector3.ONE
		mi.mesh = bm
		var mat: StandardMaterial3D = _handle_mat.duplicate()
		mat.albedo_color = e[3]
		mi.material_override = mat
		mi.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
		add_child(mi)
		mi.position = e[1]
		_handles.append({"id": e[0], "node": mi, "pos": e[1], "axis": e[2], "color": e[3]})
	_style_handles()


func _style_handles() -> void:
	var eye := player.camera.global_position
	for h in _handles:
		var mi: MeshInstance3D = h["node"]
		var s := HANDLE * maxf(1.0, eye.distance_to(h["pos"]) / Settings.m2v(4.0))
		mi.scale = Vector3.ONE * s * (1.35 if h["id"] == _hover or h["id"] == _drag.get("id", "") else 1.0)
		var c: Color = h["color"]
		(mi.material_override as StandardMaterial3D).albedo_color = c.lightened(0.4) if h["id"] == _hover else c


func _pick_handle() -> String:
	var r := _ray()
	var o: Vector3 = r[0]
	var d: Vector3 = r[1]
	var best := ""
	var best_d := INF
	for h in _handles:
		var p: Vector3 = h["pos"]
		var t := (p - o).dot(d)
		if t <= 0.0:
			continue
		var dist := (o + d * t - p).length()
		var reach := HANDLE * maxf(1.0, t / Settings.m2v(4.0)) * 0.9
		if dist < reach and t < best_d:
			best_d = t
			best = h["id"]
	return best


func _handle(id: String) -> Dictionary:
	for h in _handles:
		if h["id"] == id:
			return h
	return {}


func _start_drag(id: String) -> void:
	var h := _handle(id)
	_drag = {"id": id, "pos": h["pos"], "axis": h["axis"], "lo": lo, "wsize": wsize, "hollow": hollow,
		"s0": _axis_param(h["pos"], h["axis"]), "plane0": _plane_point(float(h["pos"].y)), "shift": Input.is_key_pressed(KEY_SHIFT)}


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
	var s0: Vector3i = _drag["wsize"]
	var nlo := l0
	var ns := s0
	if id == "move":
		var dp: Vector3 = _plane_point(float(_drag["pos"].y)) - (_drag["plane0"] as Vector3)
		nlo = l0 + Vector3i(roundi(dp.x), 0, roundi(dp.z))
	else:
		var dv := roundi(_axis_param(_drag["pos"], _drag["axis"]) - float(_drag["s0"]))
		match id:
			"top":
				if _drag["shift"]:
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
				var h := clampi(int(_drag["hollow"]) + dv, 0, maxi(0, mini(s0.x, s0.z) / 2 - 1))
				if h != hollow:
					hollow = h
					_dirty = true
	ns = ns.clamp(Vector3i.ONE, Vector3i.ONE * MAX_SIDE)
	if nlo != lo or ns != wsize:
		lo = nlo
		wsize = ns
		_dirty = true
		_move_handles()


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
