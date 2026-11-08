class_name PonderScreen
extends CanvasLayer

## The guide screen (ADR 0132, Create's "ponder" in our own shape): a scene from Ponder plays on a small
## 3D stage drawn in a SubViewport with the real pieces (a throwaway BlockEntities store, views built as
## WorkSystem builds them), step by step with a caption, highlight boxes and moving dots for what flows.
## Opened from the inventory (P over an item that has a guide) or `/ponder <id>`; Esc closes, Space or
## the right arrow steps, the left arrow steps back, R replays; steps also advance by themselves.

signal closed

const B := Settings.BLOCK
const FLOW_COLORS := {"steam": Color(0.95, 0.95, 1.0), "water": Color(0.3, 0.55, 1.0), "power": Color(1.0, 0.85, 0.2), "items": Color(1.0, 0.6, 0.25)}
const BLOCK_COLORS := {"bricks": Color(0.62, 0.32, 0.24), "water_static": Color(0.2, 0.45, 0.85, 0.7), "water": Color(0.2, 0.45, 0.85, 0.7),
	"dirt": Color(0.4, 0.3, 0.2), "stone": Color(0.5, 0.5, 0.52), "cobblestone": Color(0.45, 0.45, 0.47)}
const GROUND := Color(0.33, 0.42, 0.26)
const DOTS := 3                   # moving dots per flow
const FLOW_PERIOD := 1.6          # seconds a dot takes from one end to the other
const DEFAULT_SECONDS := 6.0      # a step shows this long before the next comes by itself

var is_open := false
var scene_id := ""
var step := -1
var items: ItemRegistry
var recipes: RecipeRegistry
var player: Node                  # the mouse's owner when opened outside the inventory
var took_mouse := false

var _scene := {}
var _store: BlockEntities
var _root: Control
var _title: Label
var _caption: Label
var _strip: VBoxContainer         # the scene's recipes: input icons -> output icon
var _counter: Label
var _vp: SubViewport
var _vpc: SubViewportContainer
var _world: Node3D
var _cam: Camera3D
var _views := {}                  # entity pos -> Node3D
var _blocks := {}                 # "x,y,z" -> MeshInstance3D
var _marks: Array[MeshInstance3D] = []
var _flows: Array = []            # {dots: [MeshInstance3D], from: Vector3, to: Vector3}
var _ground: MeshInstance3D
var _centre := Vector3.ZERO
var _distance := 40.0
var _yaw := 0.0
var _t := 0.0                     # seconds into the step
var _seconds := DEFAULT_SECONDS
var _frame := 0


func setup(p_items: ItemRegistry, p_recipes: RecipeRegistry, p_player: Node) -> void:
	items = p_items
	recipes = p_recipes
	player = p_player
	Ponder.items = p_items
	Ponder.recipes = p_recipes


func _ready() -> void:
	layer = 30
	visible = false
	_root = Control.new()
	_root.set_anchors_preset(Control.PRESET_FULL_RECT)
	_root.mouse_filter = Control.MOUSE_FILTER_STOP
	_root.draw.connect(func() -> void: _root.draw_rect(Rect2(Vector2.ZERO, _root.size), Color(0, 0, 0, 0.6)))
	_root.theme = UiStyle.theme()
	add_child(_root)
	var pb: Array = UiStyle.centered_panel(_root, 520.0)
	var box: VBoxContainer = pb[1]
	_title = UiStyle.label("", 24, UiStyle.ACCENT)
	box.add_child(_title)
	_vpc = SubViewportContainer.new()
	_vpc.stretch = true
	_vpc.custom_minimum_size = Vector2(640, 320)
	box.add_child(_vpc)
	_vp = SubViewport.new()
	_vp.own_world_3d = true
	_vp.render_target_update_mode = SubViewport.UPDATE_DISABLED
	_vpc.add_child(_vp)
	_world = Node3D.new()
	_vp.add_child(_world)
	_cam = Camera3D.new()
	_cam.fov = 40.0
	var env := Environment.new()
	env.background_mode = Environment.BG_COLOR
	env.background_color = Color(0.11, 0.12, 0.15)
	env.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	env.ambient_light_color = Color(0.8, 0.85, 1.0)
	env.ambient_light_energy = 0.45
	_cam.environment = env
	_vp.add_child(_cam)
	var sun := DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-50, 35, 0)
	sun.light_energy = 1.3
	sun.shadow_enabled = true
	_vp.add_child(sun)
	_caption = UiStyle.label("", 20)
	_caption.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_caption.custom_minimum_size = Vector2(0, 84)
	box.add_child(_caption)
	_strip = VBoxContainer.new()
	box.add_child(_strip)
	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", 10)
	box.add_child(row)
	row.add_child(UiStyle.button("< Back", prev, 120))
	row.add_child(UiStyle.button("Next >", next, 120))
	row.add_child(UiStyle.button("Replay (R)", replay, 140))
	_counter = UiStyle.label("", 17, UiStyle.DIM)
	_counter.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_counter.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	row.add_child(_counter)
	row.add_child(UiStyle.button("Close (Esc)", close, 140))
	set_process(false)


## Plays scene `id` from its first step. False when there is no such scene.
func open(id: String) -> bool:
	var sc := Ponder.get_scene(id)
	if sc.is_empty():
		return false
	if is_open:
		_clear()
	scene_id = id
	_scene = sc
	_store = BlockEntities.new(items, recipes)
	var win: Vector2 = _root.get_viewport_rect().size
	var w := clampf(win.x - 160.0, 480.0, 880.0)
	_vpc.custom_minimum_size = Vector2(w, minf(w * 0.5, win.y - 300.0))
	_title.text = String(sc.get("title", id))
	_fill_strip(sc.get("recipes", []))
	_fit_camera()
	_ground = MeshInstance3D.new()
	var gm := BoxMesh.new()
	var lo := _bounds_lo - Vector3(2, 1, 2) * B
	var hi := Vector3(_bounds_hi.x + 2 * B, 0, _bounds_hi.z + 2 * B)
	gm.size = hi - lo
	_ground.mesh = gm
	_ground.position = (lo + hi) * 0.5
	_ground.material_override = _flat(GROUND)
	_world.add_child(_ground)
	visible = true
	is_open = true
	_vp.render_target_update_mode = SubViewport.UPDATE_ALWAYS
	set_process(true)
	if player != null and not player.ui_open:
		took_mouse = true
		player.set_ui_open(true, false)
	step_to(0)
	return true


func close() -> void:
	if not is_open:
		return
	_clear()
	is_open = false
	visible = false
	_vp.render_target_update_mode = SubViewport.UPDATE_DISABLED
	set_process(false)
	if took_mouse and player != null:
		player.set_ui_open(false, false)
	took_mouse = false
	closed.emit()


func steps() -> int:
	return _scene.get("steps", []).size()


func next() -> void:
	if step + 1 < steps():
		step_to(step + 1)


func prev() -> void:
	if step > 0:
		step_to(step - 1)


func replay() -> void:
	step_to(0)


## Shows step `i`: the stage as the steps up to it built it, with its own marks and flows.
func step_to(i: int) -> void:
	if i < 0 or i >= steps():
		return
	_clear_stage()
	_store = BlockEntities.new(items, recipes)
	var all: Array = _scene["steps"]
	for k in i + 1:
		_apply(all[k], k == i)
	_rebuild_views()
	step = i
	_t = 0.0
	_seconds = float(all[i].get("seconds", DEFAULT_SECONDS))
	_caption.text = String(all[i].get("text", ""))
	_counter.text = "Step %d of %d" % [i + 1, steps()]


func _apply(s: Dictionary, last: bool) -> void:
	for a in s.get("remove", []):
		var at := _v(a) * B
		_store.remove(at)
		_blocks.erase("%d,%d,%d" % [at.x, at.y, at.z])
	for p in s.get("place", []):
		var at := _v(p.get("at", [0, 0, 0])) * B
		var e: RefCounted = _store.create(at, String(p.get("kind", "")))
		if e == null:
			continue
		if p.has("item") and "item" in e:
			e.item = String(p["item"])
		if p.has("facing") and "facing" in e:
			e.facing = int(p["facing"])
		if p.has("axis") and "axis" in e:
			e.axis = int(p["axis"])
		_set_fields(e, p.get("set", {}))
	for st in s.get("set", []):   # a piece placed earlier changes state (a switch closes, a turbine stops)
		_set_fields(_store.at(_v(st.get("at", [0, 0, 0])) * B), st.get("set", {}))
	for b in s.get("blocks", []):
		var at := _v(b.get("at", [0, 0, 0])) * B
		var size := _v(b.get("size", [1, 1, 1])) * B
		var mi := MeshInstance3D.new()
		var bm := BoxMesh.new()
		bm.size = Vector3(size)
		mi.mesh = bm
		mi.position = Vector3(at) + Vector3(size) * 0.5
		var mat := String(b.get("material", "stone"))
		mi.material_override = _flat(BLOCK_COLORS.get(mat, ItemIcon.color_for(mat)))
		_blocks["%d,%d,%d" % [at.x, at.y, at.z]] = mi
	if not last:
		return
	for m in s.get("mark", []):
		var at := _v(m.get("at", [0, 0, 0])) * B
		var size := _v(m.get("size", [1, 1, 1])) * B
		var mi := MeshInstance3D.new()
		var bm := BoxMesh.new()
		bm.size = Vector3(size) + Vector3.ONE * 0.6
		mi.mesh = bm
		mi.position = Vector3(at) + Vector3(size) * 0.5
		var c := Color(0.3, 1.0, 0.45, 0.2) if bool(m.get("ok", true)) else Color(1.0, 0.25, 0.2, 0.3)
		mi.material_override = _flat(c, true)
		_marks.append(mi)
		_world.add_child(mi)
	for f in s.get("flow", []):
		var kind := String(f.get("kind", "steam"))
		var fl := {"from": _centre_of(f.get("from", [0, 0, 0])), "to": _centre_of(f.get("to", [0, 0, 0])), "dots": []}
		for d in DOTS:
			var mi := MeshInstance3D.new()
			var sm := SphereMesh.new()
			sm.radius = 1.3 if kind != "power" else 0.9
			sm.height = sm.radius * 2.0
			mi.mesh = sm
			mi.material_override = _flat(FLOW_COLORS.get(kind, Color.WHITE), true)
			fl["dots"].append(mi)
			_world.add_child(mi)
		if f.has("label"):   # IN / OUT / POWER over the middle of the flow
			var lb := Label3D.new()
			lb.text = String(f["label"])
			lb.billboard = BaseMaterial3D.BILLBOARD_ENABLED
			lb.no_depth_test = true
			lb.font_size = 64
			lb.pixel_size = 0.028
			lb.outline_size = 12
			lb.modulate = FLOW_COLORS.get(kind, Color.WHITE)
			lb.position = (fl["from"] + fl["to"]) * 0.5 + Vector3.UP * B * 1.2
			fl["dots"].append(lb)   # freed with the dots
			_world.add_child(lb)
		_flows.append(fl)


static func _set_fields(e: RefCounted, fields: Dictionary) -> void:
	if e == null:
		return
	for k in fields:
		var cur = e.get(k)
		var v = fields[k]
		e.set(k, int(v) if cur is int else (bool(v) if cur is bool else v))


## The middle of a block at `a` (blocks), lifted half a block.
func _centre_of(a: Array) -> Vector3:
	return Vector3(_v(a) * B) + Vector3(0.5, 0.5, 0.5) * B


func _rebuild_views() -> void:
	for v in _views.values():
		v.queue_free()
	_views.clear()
	for e in _store.all_entities():
		_view(e)
	for k in _blocks:
		if _blocks[k].get_parent() == null:
			_world.add_child(_blocks[k])


func _view(e: RefCounted) -> void:
	if _views.has(e.pos):
		_views[e.pos].queue_free()
	var parts: Dictionary = WorkPalette.art_parts(e, e.view_parts())
	var v := WorkPalette.make_view(e.pos, parts.get("solid", {}), parts.get("ghost", {}), parts.get("guide", {}), parts.get("items", []),
		float(BlockEntities.width_of(e.kind)) / B)
	for ch in v.find_children("*", "MeshInstance3D", true, false):
		ch.visibility_range_end = 0.0   # the play range cutoff; the stage camera is further than that
	_world.add_child(v)
	_views[e.pos] = v


func _process(delta: float) -> void:
	_t += delta
	_frame += 1
	_yaw += delta * 0.18
	var pitch := deg_to_rad(28.0)
	var off := Vector3(sin(_yaw) * cos(pitch), sin(pitch), cos(_yaw) * cos(pitch)) * _distance
	_cam.position = _centre + off
	_cam.look_at(_centre, Vector3.UP)
	var pulse := 0.6 + 0.4 * sin(_t * 4.0)
	for m in _marks:
		(m.material_override as StandardMaterial3D).albedo_color.a = 0.1 + 0.14 * pulse
	for f in _flows:
		for i in DOTS:
			var u := fmod(_t / FLOW_PERIOD + float(i) / DOTS, 1.0)
			f["dots"][i].position = f["from"].lerp(f["to"], u)
	if _frame % 6 == 0:   # the turbine's blades and the small sources turn while they run
		for e in _store.all_entities():
			if "spin" in e and (("rpm" in e and e.rpm > 0.0) or ("drive" in e and e.drive > 0.0)):
				e.spin += 1
				_view(e)
	if _t >= _seconds and step + 1 < steps():
		next()


func _input(event: InputEvent) -> void:
	if not is_open:
		return
	if event is InputEventKey and event.pressed and not event.echo:
		match event.keycode:
			KEY_ESCAPE:
				close()
			KEY_RIGHT, KEY_SPACE, KEY_ENTER:
				next()
			KEY_LEFT:
				prev()
			KEY_R:
				replay()
			_:
				return
		_root.get_viewport().set_input_as_handled()


func _clear_stage() -> void:
	for v in _views.values():
		v.queue_free()
	_views.clear()
	for b in _blocks.values():
		b.queue_free()
	_blocks.clear()
	for m in _marks:
		m.queue_free()
	_marks.clear()
	for f in _flows:
		for d in f["dots"]:
			d.queue_free()
	_flows.clear()


func _clear() -> void:
	_clear_stage()
	_fill_strip([])
	if _ground != null:
		_ground.queue_free()
		_ground = null
	_scene = {}
	step = -1


var _bounds_lo := Vector3.ZERO
var _bounds_hi := Vector3.ZERO


## The camera orbits the middle of everything any step places, far enough to see it all.
func _fit_camera() -> void:
	var lo := Vector3(INF, INF, INF)
	var hi := Vector3(-INF, -INF, -INF)
	for s in _scene.get("steps", []):
		for p in s.get("place", []):
			var kind := String(p.get("kind", ""))
			var at := Vector3(_v(p.get("at", [0, 0, 0])) * B)
			var w := float(BlockEntities.width_of(kind))
			var h := float(BlockEntities.height_of(kind))
			lo = lo.min(at)
			hi = hi.max(at + Vector3(w, h, w))
		for b in s.get("blocks", []):
			var at := Vector3(_v(b.get("at", [0, 0, 0])) * B)
			lo = lo.min(at)
			hi = hi.max(at + Vector3(_v(b.get("size", [1, 1, 1])) * B))
	if lo.x == INF:
		lo = Vector3.ZERO
		hi = Vector3.ONE * B
	_bounds_lo = lo
	_bounds_hi = hi
	_centre = (lo + hi) * 0.5
	_centre.y = lo.y + (hi.y - lo.y) * 0.35
	var ext := hi - lo
	_distance = maxf(ext.x, ext.z) * 0.85 + ext.y * 0.6 + 2.0 * B
	_yaw = -0.6


## One row per recipe (at most 6): the input icons with their counts, an arrow, the output, the line in words.
func _fill_strip(ids: Array) -> void:
	for c in _strip.get_children():
		c.queue_free()
	_strip.visible = not ids.is_empty()
	if ids.is_empty():
		return
	_strip.add_child(UiStyle.label("What goes in  ->  what comes out", 17, UiStyle.DIM))
	for rid in ids.slice(0, 6):
		var r: Dictionary = recipes.recipes.get(rid, {})
		var row := HBoxContainer.new()
		row.add_theme_constant_override("separation", 6)
		var counts := {}
		for spec in r.get("ingredients", []):
			counts[spec] = int(counts.get(spec, 0)) + 1
		var first := true
		for spec in counts:
			if not first:
				row.add_child(UiStyle.label("+", 20))
			first = false
			var id := String(spec)
			if id.begins_with("#"):
				var any: Array = items.items_with_tag(id.trim_prefix("#"))
				id = String(any[0]) if not any.is_empty() else ""
			row.add_child(_icon(id, counts[spec]))
		row.add_child(UiStyle.label("  ->  ", 20, UiStyle.ACCENT))
		var o: Dictionary = r.get("output", {})
		row.add_child(_icon(String(o.get("id", "")), int(o.get("count", 1))))
		row.add_child(UiStyle.label("  " + Ponder.recipe_line(r), 16, UiStyle.DIM))
		_strip.add_child(row)


func _icon(id: String, count: int) -> Control:
	var ic := ItemIcon.new()
	ic.items = items
	ic.custom_minimum_size = Vector2(36, 36)
	ic.stack = {"id": id, "count": count} if id != "" else {}
	ic.tooltip_text = items.display_name(id) if id != "" else ""
	return ic


static func _v(a) -> Vector3i:
	if a is Array and a.size() >= 3:
		return Vector3i(int(a[0]), int(a[1]), int(a[2]))
	return Vector3i.ZERO


static func _flat(c: Color, unshaded := false) -> StandardMaterial3D:
	var m := StandardMaterial3D.new()
	m.albedo_color = c
	if c.a < 1.0:
		m.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	if unshaded:
		m.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	return m


## Debug (ponder_test=1): plays every scene, four frames a step, into one filmstrip a scene at
## `DebugCapture.out_dir/ponder/<id>.png` (a row per step), prints one JSON line "debug ponder {...}"
## and quits. Read the sheets: the dots must move and the marks pulse between frames of a row.
func film_all(tree: SceneTree) -> void:
	var r := {"scenes": {}, "bad": {}}
	var dir: String = DebugCapture.out_dir + "/ponder"
	var ids: Array = Ponder.scenes().keys()
	if DebugCapture.ponder_only != "":
		ids = [DebugCapture.ponder_only]   # ponder_only=<id>: just that scene (a machine:<item> one too)
	for id in ids:
		var bad := Ponder.check(Ponder.get_scene(id), items, recipes)
		if not bad.is_empty():
			r["bad"][id] = Array(bad)
		open(id)
		var sheet := FrameSheet.new(320, 180, 4)
		for i in steps():
			step_to(i)
			for k in 4:
				for f in 8:
					await tree.process_frame
				sheet.add(tree.root.get_viewport())
			if i == steps() - 1:   # one frame at full size, to read the pieces
				tree.root.get_viewport().get_texture().get_image().save_png(dir + "/%s_step%d.png" % [id.replace(":", "_"), i + 1])
		var err := sheet.save(dir + "/%s.png" % id.replace(":", "_"))   # "machine:x" is no file name on Windows
		r["scenes"][id] = {"steps": steps(), "pieces": _store.size(), "saved": err == OK}
		close()
	print("debug ponder " + JSON.stringify(r))
	tree.quit()
