class_name ToolsMenu
extends CanvasLayer

## The world tools panel (ADR 0071). G (or /tools) opens a panel over the game with the mouse free:
## the brush's shape and size, the shape builder's modes and saved prefabs as buttons, the keys that
## do the same, each with a hover tooltip. Buttons call what the keys call; nothing here touches a voxel.

signal opened
signal closed

const SHAPE_TIPS := {
	"cube": "A cube snapped to its size grid (size 4 = one 0.5 m block).",
	"sphere": "A ball centred on the aimed voxel.",
	"cylinder": "An upright cylinder, as high as it is wide.",
	"flat": "A square one voxel deep in the aimed face's plane: shave a wall, level a floor, lay a skin.",
	"column": "A bar half as wide as it is long along the aimed face: a shaft or tunnel when digging, a pillar or beam when placing.",
}
const MODES := [
	["brush", "Brush", "Plain digging and placing with the brush: left click digs, right click places the held block."],
	["shape:box", "Box", "Place a box of the held material: a left click sets it down, handles resize it, a click away builds it."],
	["shape:wall", "Wall", "A wall of the held material (same handles as the box)."],
	["shape:cylinder", "Cylinder", "A cylinder of the held material."],
	["shape:cone", "Cone", "A cone of the held material."],
	["shape:sphere", "Sphere", "A sphere of the held material."],
	["shape:stairs", "Stairs", "Stairs of the held material, rising away from you."],
	["shape:slope", "Slope", "A slope of the held material, rising away from you."],
	["box", "Box fill", "Two clicks mark a box; it fills with the held material."],
	["dig", "Box dig", "Two clicks mark a box; the held tool digs it out."],
	["select", "Select", "Two clicks mark a box to save as a prefab: /prefab save <category> <name>."],
]
const KEYS := [
	["Left / right click", "dig / place the brush"],
	["Shift + right click", "paint the brush shape with the held block (creative)"],
	["B / Shift + B", "next / previous brush shape"],
	["Ctrl + wheel", "brush size"],
	["V (hold)", "the build ring: the same modes as above"],
	["R / Shift + R", "turn / tip a placed shape"],
	["X", "mirror a placed shape"],
	["Right click", "cancel a shape or box"],
]

var main: Node
var can_open: Callable
var is_open := false
var _center: Control
var _prefab_box: HFlowContainer
var _ghost: CheckBox
var _buttons := {}   # "shape:cube", "size:4", "mode:box", "prefab:cat/name" -> Button


func _ready() -> void:
	layer = 9   # with the playtest menu: above chat (8), below inventory screens (10)
	var root := Control.new()
	root.set_anchors_preset(Control.PRESET_FULL_RECT)
	root.mouse_filter = Control.MOUSE_FILTER_IGNORE
	root.theme = UiStyle.theme()
	add_child(root)
	var made: Array = UiStyle.centered_panel(root, 560.0)
	_center = made[0]
	_center.visible = false
	var box: VBoxContainer = made[1]
	box.add_child(UiStyle.label("World tools   (G or Esc closes)", 20, UiStyle.ACCENT))
	box.add_child(UiStyle.label("Brush shape", 15, UiStyle.DIM))
	var shapes := HBoxContainer.new()
	box.add_child(shapes)
	for s in Brush.SHAPES:
		_add(shapes, "shape:" + s, s.capitalize(), SHAPE_TIPS[s], _pick_shape.bind(s))
	box.add_child(UiStyle.label("Brush size", 15, UiStyle.DIM))
	var sizes := HBoxContainer.new()
	box.add_child(sizes)
	for n in Brush.CREATIVE_SIZES:
		_add(sizes, "size:%d" % n, Brush._len(n), "", _pick_size.bind(n))
	box.add_child(UiStyle.label("Build mode", 15, UiStyle.DIM))
	var modes := HFlowContainer.new()
	box.add_child(modes)
	for m in MODES:
		_add(modes, "mode:" + m[0], m[1], m[2], _pick_mode.bind(m[0]))
	_prefab_box = HFlowContainer.new()
	box.add_child(_prefab_box)
	_ghost = CheckBox.new()
	_ghost.text = "Ghost of what you will place or break"
	_ghost.tooltip_text = "The outline and see-through shell on the aimed spot: blue for a placement, orange for a dig. Saved with your settings."
	_ghost.focus_mode = Control.FOCUS_NONE
	_ghost.toggled.connect(func(on: bool) -> void:
		Settings.brush_ghost = on
		Settings.save_user_settings())
	box.add_child(_ghost)
	box.add_child(UiStyle.label("Keys", 15, UiStyle.DIM))
	var keys := GridContainer.new()
	keys.columns = 2
	keys.add_theme_constant_override("h_separation", 18)
	box.add_child(keys)
	for k in KEYS:
		keys.add_child(UiStyle.label(k[0], 14))
		keys.add_child(UiStyle.label(k[1], 14, UiStyle.DIM))


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

## Marks the current brush and mode, greys what the mode or the held item rules out.
func _refresh() -> void:
	var player: Player = main.player
	var builder: ShapeBuilder = main.builder
	var creative: bool = GameMode.rule(player.game_mode, "instant_break")
	var dig := player.dig_size()
	_ghost.set_pressed_no_signal(Settings.brush_ghost)
	for s in Brush.SHAPES:
		_buttons["shape:" + s].button_pressed = s == player.brush_shape
	for n in Brush.CREATIVE_SIZES:
		var b: Button = _buttons["size:%d" % n]
		b.button_pressed = n == player.brush_size
		b.disabled = not Brush.sizes_for(creative).has(n)
		b.tooltip_text = "%s across" % Brush._len(n)
		if b.disabled:
			b.tooltip_text += " (creative only)"
		elif n > dig:
			b.tooltip_text += " (this tool digs %s; placing is not capped)" % Brush._len(dig)
	var held: bool = player.inventory != null and builder._held_material() > 0
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


## The saved prefabs as buttons, read from disk once per opening.
func _list_prefabs() -> void:
	for id in _buttons.keys():
		if id.begins_with("prefab:"):
			_buttons[id].queue_free()
			_buttons.erase(id)
	var all: Dictionary = main.builder.prefabs.list()
	for cat in all:
		for nm in all[cat]:
			_add(_prefab_box, "prefab:%s/%s" % [cat, nm], "Prefab: " + nm, "Place the saved prefab %s / %s (R turns it, X mirrors it)." % [cat, nm], _pick_prefab.bind(cat, nm))


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
