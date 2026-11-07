class_name ToolsMenu
extends CanvasLayer

## The world tools panel (ADR 0071, ADR 0073). G (or /tools) opens a panel over the game with the
## mouse free. It says in words what the clicks do right now (the "Now" lines), then the brush's shape
## and size, the shape builder's modes and saved prefabs as buttons, each with its description beside
## it, and the keys that do the same. Buttons call what the keys call; nothing here touches a voxel.

signal opened
signal closed

const SHAPE_TIPS := {
	"cube": "A cube flush with the aimed face: into it when digging, standing on it when placing. Centred on the aim, on the 0.5 m block grid.",
	"sphere": "A ball around the aimed voxel when digging, resting on the aimed face when placing.",
	"cylinder": "An upright cylinder as high as it is wide, placed like the ball.",
	"flat": "A square one voxel thick in the aimed face: shave a wall, level a floor, lay a floor or wall skin.",
	"column": "A bar half as wide as it is long along the aimed face: a shaft or tunnel when digging, a pillar or beam when placing.",
	"tunnel": "A hallway you walk in (1 m wide, 2 m tall, floor at your feet), half the size deep per dig, on the block grid: keep digging straight ahead. Digs only; placing places the cube.",
}
const MODES := [
	["brush", "Brush", "Left click digs the brush, right click places the held block as the brush (shape and size above)."],
	["shape:box", "Box", "A box of the held material: a click sets it down where you aim and frees the cursor, drag the top arrow, the foot ring (size) and the top ring (hollow), a click away builds it."],
	["shape:wall", "Wall", "A wall of the held material, the same handles as the box."],
	["shape:cylinder", "Cylinder", "A cylinder of the held material."],
	["shape:cone", "Cone", "A cone of the held material."],
	["shape:sphere", "Sphere", "A sphere of the held material."],
	["shape:stairs", "Stairs", "Stairs of the held material, rising away from you."],
	["shape:slope", "Slope", "A slope of the held material, rising away from you."],
	["box", "Box fill", "Two clicks mark a box; it fills with the held material."],
	["dig", "Box dig", "Two clicks mark a box; holding the left button digs it with the held tool."],
	["select", "Select", "Two clicks mark a box to save as a prefab: /prefab save <category> <name>."],
]
const KEYS := [
	["Left click", "dig the brush (hold in survival)"],
	["Right click", "place the held block as the brush"],
	["Shift + right click", "paint the dig shape (creative)"],
	["B / Shift + B", "next / previous brush shape"],
	["Ctrl + wheel", "brush size"],
	["V (hold)", "the build ring: these modes"],
	["C (hold)", "the mine ring: brush shape, wheel for size"],
	["N", "snap a placed shape: off / 0.5 m / 1 m"],
	["Wheel over a handle", "step it one snap (move handle: up / down)"],
	["R / Shift + R", "turn / tip a placed shape"],
	["X", "mirror a placed shape"],
	["Right click", "cancel a shape or box (held on a placed shape: look around)"],
]

var main: Node
var can_open: Callable
var is_open := false
var _center: Control
var _now: Label          # what the clicks do right now, in words
var _shape_tip: Label    # the selected brush shape, described
var _size_tip: Label     # the selected size, and what the held tool caps it to
var _prefab_box: VBoxContainer
var _ghost: CheckBox
var _buttons := {}   # "shape:cube", "size:4", "mode:box", "prefab:cat/name" -> Button


func _ready() -> void:
	layer = 9   # with the playtest menu: above chat (8), below inventory screens (10)
	var root := Control.new()
	root.set_anchors_preset(Control.PRESET_FULL_RECT)
	root.mouse_filter = Control.MOUSE_FILTER_IGNORE
	root.theme = UiStyle.theme()
	add_child(root)
	var made: Array = UiStyle.centered_panel(root, 900.0)
	_center = made[0]
	_center.visible = false
	var box: VBoxContainer = made[1]
	box.add_theme_constant_override("separation", 4)   # the panel fits a 720 px screen
	box.add_child(UiStyle.label("World tools   (G or Esc closes)", 18, UiStyle.ACCENT))
	_now = UiStyle.label("", 15)
	_now.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	box.add_child(_now)
	box.add_child(UiStyle.label("Brush shape", 15, UiStyle.DIM))
	var shapes := HBoxContainer.new()
	box.add_child(shapes)
	for s in Brush.SHAPES:
		_add(shapes, "shape:" + s, s.capitalize(), SHAPE_TIPS[s], _pick_shape.bind(s))
	_shape_tip = _tip_label()
	box.add_child(_shape_tip)
	box.add_child(UiStyle.label("Brush size", 15, UiStyle.DIM))
	var sizes := HBoxContainer.new()
	box.add_child(sizes)
	for n in Brush.CREATIVE_SIZES:
		_add(sizes, "size:%d" % n, Brush._len(n), "", _pick_size.bind(n))
	_size_tip = _tip_label()
	box.add_child(_size_tip)
	box.add_child(UiStyle.label("Build mode", 15, UiStyle.DIM))
	var modes := GridContainer.new()   # two columns of button + description
	modes.columns = 4
	modes.add_theme_constant_override("h_separation", 10)
	modes.add_theme_constant_override("v_separation", 2)
	box.add_child(modes)
	for m in MODES:
		var b := _add(modes, "mode:" + m[0], m[1], m[2], _pick_mode.bind(m[0]))
		b.custom_minimum_size.x = 84
		b.size_flags_horizontal = Control.SIZE_SHRINK_BEGIN
		var l := UiStyle.label(m[2], 12, UiStyle.DIM)
		l.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
		l.custom_minimum_size.x = 320
		l.size_flags_horizontal = Control.SIZE_EXPAND_FILL
		modes.add_child(l)
	if MODES.size() % 2 == 1:
		modes.add_child(Control.new())
		modes.add_child(Control.new())
	_prefab_box = VBoxContainer.new()
	box.add_child(_prefab_box)
	_ghost = CheckBox.new()
	_ghost.text = "Ghost of what you will place or break"
	_ghost.tooltip_text = "The outline and see-through shell on the aimed spot: blue for a placement, orange for a dig. Saved with your settings."
	_ghost.focus_mode = Control.FOCUS_NONE
	_ghost.toggled.connect(func(on: bool) -> void:
		Settings.brush_ghost = on
		Settings.save_user_settings())
	box.add_child(_ghost)
	var keys := GridContainer.new()   # three columns of key + what it does
	keys.columns = 6
	keys.add_theme_constant_override("h_separation", 10)
	keys.add_theme_constant_override("v_separation", 0)
	box.add_child(keys)
	for k in KEYS:
		keys.add_child(UiStyle.label(k[0], 12))
		var l := UiStyle.label(k[1], 12, UiStyle.DIM)
		l.size_flags_horizontal = Control.SIZE_EXPAND_FILL
		keys.add_child(l)


func _tip_label() -> Label:
	var l := UiStyle.label("", 12, UiStyle.DIM)
	l.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	return l


func _add(parent: Control, id: String, text: String, tip: String, cb: Callable) -> Button:
	var b := Button.new()
	b.text = text
	b.tooltip_text = tip
	b.toggle_mode = true
	b.focus_mode = Control.FOCUS_NONE
	b.pressed.connect(cb)
	parent.add_child(b)
	_buttons[id] = b
	return b


# ---------------- state ----------------

## Marks the current brush and mode, greys what the mode or the held item rules out, and says in
## words what a click does now.
func _refresh() -> void:
	var player: Player = main.player
	var builder: ShapeBuilder = main.builder
	var creative: bool = GameMode.rule(player.game_mode, "instant_break")
	var dig := player.dig_size()
	_ghost.set_pressed_no_signal(Settings.brush_ghost)
	for s in Brush.SHAPES:
		_buttons["shape:" + s].button_pressed = s == player.brush_shape
	_shape_tip.text = SHAPE_TIPS[player.brush_shape]
	for n in Brush.CREATIVE_SIZES:
		var b: Button = _buttons["size:%d" % n]
		b.button_pressed = n == player.brush_size
		b.disabled = not Brush.sizes_for(creative).has(n)
		b.tooltip_text = "%s across" % Brush._len(n)
		if b.disabled:
			b.tooltip_text += " (creative only)"
		elif n > dig:
			b.tooltip_text += " (this tool digs %s; placing is not capped)" % Brush._len(dig)
	_size_tip.text = "%s across%s" % [Brush._len(player.brush_size),
		"" if dig >= player.brush_size else "; the held tool digs only %s of it (placing is not capped)" % Brush._len(dig)]
	if not creative:
		_size_tip.text += ". 2 m and 4 m are creative."
	var held_mat: int = builder._held_material() if player.inventory != null else 0
	var held := held_mat > 0
	var current: String = "brush" if builder.state == ShapeBuilder.State.OFF else ("shape:" + builder.shape if builder.mode == "shape" else builder.mode)
	for m in MODES:
		var b: Button = _buttons["mode:" + m[0]]
		var id: String = m[0]
		b.button_pressed = id == current
		b.disabled = (id.begins_with("shape:") or id == "box") and not held
		b.tooltip_text = m[2] + ("\n(hold a block to use it)" if b.disabled else "")
	for id in _buttons:
		if id.begins_with("prefab:"):
			var b: Button = _buttons[id]
			b.disabled = not held
			b.button_pressed = builder.mode == "prefab" and builder.prefab_label == id.get_file()
	_now.text = _now_text(player, builder, creative, dig, held_mat, current)


## "Left click: dig a 1 m cube into the aimed face. Right click: place a 1 m cube of Sand standing on it."
func _now_text(player: Player, builder: ShapeBuilder, creative: bool, dig: int, held_mat: int, current: String) -> String:
	if current != "brush":
		for m in MODES:
			if m[0] == current:
				return "Now: %s mode. %s" % [m[1], m[2]]
		return "Now: placing a prefab. A click sets it down, R turns it, X mirrors it, a click away builds it."
	var shape: String = player.brush_shape
	var lines := PackedStringArray()
	lines.append("Now, left click: dig %s%s." % [Brush.explain(dig, shape, false), "" if creative else " (hold; the tool decides how fast)"])
	if held_mat > 0:
		var place_size := player.place_size()
		var def: Dictionary = player.inventory.held_def()
		var what := str(def.get("name", def.get("id", GameMaterials.name_of(held_mat)))).capitalize()
		lines.append("Right click: place %s of %s.%s" % [Brush.explain(place_size, shape, true), what,
			" Shift + right click paints it onto what is there." if creative else ""])
	else:
		lines.append("Right click: nothing to place (hold a block for that).")
	return "\n".join(lines)


## The saved prefabs as buttons, read from disk once per opening.
func _list_prefabs() -> void:
	for id in _buttons.keys():
		if id.begins_with("prefab:"):
			_buttons[id].queue_free()
			_buttons.erase(id)
	for c in _prefab_box.get_children():
		c.queue_free()
	var all: Dictionary = main.builder.prefabs.list()
	if all.is_empty():
		return
	var row := HFlowContainer.new()
	_prefab_box.add_child(row)
	row.add_child(UiStyle.label("Prefabs", 15, UiStyle.DIM))
	for cat in all:
		for nm in all[cat]:
			_add(row, "prefab:%s/%s" % [cat, nm], "Prefab: " + nm, "Place the saved prefab %s / %s (R turns it, X mirrors it)." % [cat, nm], _pick_prefab.bind(cat, nm))


func _pick_shape(s: String) -> void:
	main.player.set_brush(main.player.brush_size, s)


func _pick_size(n: int) -> void:
	main.player.set_brush(n, main.player.brush_shape)


func _pick_mode(id: String) -> void:
	var builder: ShapeBuilder = main.builder
	if id == "brush":
		builder._set_state(ShapeBuilder.State.OFF)
		return
	if id.begins_with("shape:"):
		builder.start_shape(id.substr(6))
	else:
		builder.start_box(id)
	close()   # the shape follows the aim: the mouse goes back to the game


func _pick_prefab(cat: String, nm: String) -> void:
	if main.builder.start_prefab(cat, nm):
		close()


# ---------------- open / close ----------------

## force: open even over the autopilot (/tools in scripted captures).
func open(force := false) -> void:
	if is_open or (not force and can_open.is_valid() and not can_open.call()):
		return
	is_open = true
	_list_prefabs()
	_refresh()
	_center.visible = true
	opened.emit()


func close() -> void:
	if not is_open:
		return
	is_open = false
	_center.visible = false
	closed.emit()


func _process(_delta: float) -> void:
	if is_open:
		_refresh()   # a dozen buttons: the brush and mode may change under the panel (keys, chat)


func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed and not event.echo and event.keycode == KEY_G:
		if is_open:
			close()
		else:
			open()
		get_viewport().set_input_as_handled()


## Esc closes before the game sees it; other keys stay out of the game while the panel is up.
func _input(event: InputEvent) -> void:
	if not is_open or not (event is InputEventKey) or not event.pressed:
		return
	if event.keycode == KEY_ESCAPE:
		close()
		get_viewport().set_input_as_handled()
	elif not event.keycode in [KEY_F2, KEY_F3, KEY_F4, KEY_F9, KEY_G]:
		get_viewport().set_input_as_handled()
