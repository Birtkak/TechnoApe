class_name ProspectMenu
extends CanvasLayer

## The prospecting pick's panel (ADR 0091). Shift + right click with a pick (or /prospect menu) opens
## it over the game with the mouse free: the search distance, the ore to look for and what a tap shows
## (Prospecting.MODES), then the last reading and, in density map mode, its map: the square around
## the tap seen from above, north up, brighter where more of the ore lies under it.

signal opened
signal closed

var main: Node
var can_open: Callable
var prospecting: Prospecting
var is_open := false
var pick := {}           # the held pick's def
var _center: Control
var _pick_line: Label
var _reading: Label
var _map: Control
var _buttons := {}       # "range:24", "ore:copper_ore", "mode:map" -> Button


func _ready() -> void:
	layer = 9   # with the tools panel
	var root := Control.new()
	root.set_anchors_preset(Control.PRESET_FULL_RECT)
	root.mouse_filter = Control.MOUSE_FILTER_IGNORE
	root.theme = UiStyle.theme()
	add_child(root)
	var made: Array = UiStyle.centered_panel(root, 640.0)
	_center = made[0]
	_center.visible = false
	var box: VBoxContainer = made[1]
	box.add_theme_constant_override("separation", 4)
	box.add_child(UiStyle.label("Prospecting   (Shift + right click or Esc closes)", 18, UiStyle.ACCENT))
	_pick_line = _wrap(UiStyle.label("", 13, UiStyle.DIM))
	box.add_child(_pick_line)
	box.add_child(UiStyle.label("Search distance", 15, UiStyle.DIM))
	var ranges := HBoxContainer.new()
	box.add_child(ranges)
	for r in Prospecting.RANGES_M:
		_add(ranges, "range:%d" % r, "%d m" % r, func() -> void: prospecting.range_m = r)
	box.add_child(UiStyle.label("Look for", 15, UiStyle.DIM))
	var ores := HFlowContainer.new()
	box.add_child(ores)
	_add(ores, "ore:", "Any ore", func() -> void: prospecting.ore = "")
	for m in prospecting.ore_names():
		_add(ores, "ore:" + m, String(m).capitalize(), func() -> void: prospecting.ore = m)
	box.add_child(UiStyle.label("A tap shows", 15, UiStyle.DIM))
	var modes := HBoxContainer.new()
	box.add_child(modes)
	for k in Prospecting.MODES:
		_add(modes, "mode:" + k, Prospecting.MODES[k], func() -> void: prospecting.mode = k)
	_map = Control.new()
	_map.custom_minimum_size = Vector2(216, 216)
	_map.size_flags_horizontal = Control.SIZE_SHRINK_CENTER
	_map.draw.connect(_draw_map)
	box.add_child(_map)
	_reading = _wrap(UiStyle.label("", 13))
	box.add_child(_reading)


func _wrap(l: Label) -> Label:
	l.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	return l


func _add(parent: Control, id: String, text: String, cb: Callable) -> void:
	var b := Button.new()
	b.text = text
	b.toggle_mode = true
	b.focus_mode = Control.FOCUS_NONE
	b.pressed.connect(func() -> void:
		cb.call()
		_refresh())
	parent.add_child(b)
	_buttons[id] = b


## Marks the current settings, greys what the held pick cannot do, shows the last reading.
func _refresh() -> void:
	var exact := bool(pick.get("prospects", {}).get("exact", false))
	var top := float(pick.get("prospects", {}).get("range_m", 12.0))
	var reach := prospecting.reach_m(pick.get("prospects", {}))
	_pick_line.text = "%s: %s. A right click on rock reads it; each find goes on the map." % [pick.get("name", "Prospecting pick"),
		("names every vein and its ores up to %d m" % roundi(top)) if exact else ("only says whether ore lies within %d m" % roundi(top))]
	for r in Prospecting.RANGES_M:
		var b: Button = _buttons["range:%d" % r]
		b.disabled = r > top
		b.button_pressed = is_equal_approx(r, reach)
	for id in _buttons:
		if id.begins_with("ore:") or id.begins_with("mode:"):
			var b: Button = _buttons[id]
			b.disabled = not exact   # the flint pick cannot tell ores apart
			b.button_pressed = id == "ore:" + prospecting.ore or id == "mode:" + prospecting.mode
	_map.visible = exact and prospecting.mode == "map"
	_map.queue_redraw()
	var lines: Array = prospecting.last.get("lines", [])
	_reading.text = "Last reading: " + ("\n".join(lines) if not lines.is_empty() else "none yet. Tap rock with the pick.")


func _draw_map() -> void:
	var n := Prospecting.MAP_N
	var px := _map.size.x / n
	var m := prospecting.map
	for row in n:
		for col in n:
			var v := m[row * n + col] if m.size() == n * n else 0.0
			var c := Color(0.12, 0.12, 0.14).lerp(UiStyle.ACCENT, clampf(v / maxf(prospecting.density, 0.01), 0.0, 1.0))
			_map.draw_rect(Rect2(col * px + 1, row * px + 1, px - 2, px - 2), c)
	var mid := _map.size / 2.0
	_map.draw_circle(mid, 4.0, Color.WHITE)   # where the pick tapped
	_map.draw_string(ThemeDB.fallback_font, Vector2(mid.x - 4, 12), "N", HORIZONTAL_ALIGNMENT_LEFT, -1, 12, Color.WHITE)
	if m.size() == n * n:
		_map.draw_string(ThemeDB.fallback_font, Vector2(2, _map.size.y - 4), "%d m across" % roundi(2.0 * prospecting.map_range_m), HORIZONTAL_ALIGNMENT_LEFT, -1, 11, UiStyle.DIM)


func _process(_delta: float) -> void:
	if is_open:
		_refresh()   # a tap or a command may change the reading under the panel


# ---------------- open / close ----------------

## Opens for the pick `def`. force: open even over the autopilot (/prospect menu in captures).
func open(def: Dictionary, force := false) -> void:
	if is_open or (not force and can_open.is_valid() and not can_open.call()):
		return
	pick = def
	is_open = true
	_refresh()
	_center.visible = true
	opened.emit()


func close() -> void:
	if not is_open:
		return
	is_open = false
	_center.visible = false
	closed.emit()


## Esc (or Shift + right click again) closes before the game sees it; other keys stay out of the game.
func _input(event: InputEvent) -> void:
	if not is_open:
		return
	if event is InputEventMouseButton and event.pressed and event.button_index == MOUSE_BUTTON_RIGHT and event.shift_pressed:
		close()
		get_viewport().set_input_as_handled()
	elif event is InputEventKey and event.pressed:
		if event.keycode == KEY_ESCAPE:
			close()
			get_viewport().set_input_as_handled()
		elif not event.keycode in [KEY_F2, KEY_F3, KEY_F4, KEY_F9]:
			get_viewport().set_input_as_handled()
