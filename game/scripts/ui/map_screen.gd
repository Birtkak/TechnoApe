class_name MapScreen
extends CanvasLayer

## The full map (plan item D10), opened with M: the explored map, waypoints with names. Drag with the
## left button to move, wheel to zoom (around the cursor), Space centres on the player, right click
## places a waypoint (or, on a pin: rename, next colour, delete). M or Esc closes.

signal closed

const ZOOMS: Array[float] = [0.25, 0.5, 1.0, 2.0, 3.0, 4.0, 6.0, 8.0]
var main: Node
var view: MapView
var is_open := false
var _root: Control
var _info: Label
var _menu: PopupMenu
var _dialog: PanelContainer
var _name_edit: LineEdit
var _zoom_i := 3
var _drag := false
var _menu_world := Vector2.ZERO   # where the right click was (world x, z)
var _menu_wp := -1                # the pin it was on, or -1
var _rename_id := -1              # the dialog renames this waypoint (-1: names a new one)


func setup(p_main: Node) -> void:
	main = p_main
	layer = 11
	visible = false
	_root = Control.new()
	_root.set_anchors_preset(Control.PRESET_FULL_RECT)
	_root.mouse_filter = Control.MOUSE_FILTER_STOP
	_root.gui_input.connect(_on_input)
	add_child(_root)
	view = MapView.new()
	view.set_anchors_preset(Control.PRESET_FULL_RECT)
	view.show_names = true
	view.world_map = main.world_map
	view.player = main.player
	view.others = main.remote_players
	_root.add_child(view)
	_info = UiStyle.label("", 15, Color(0.92, 0.92, 0.92))
	_info.position = Vector2(16, 12)
	_info.add_theme_color_override("font_outline_color", Color(0, 0, 0, 0.9))
	_info.add_theme_constant_override("outline_size", 5)
	_root.add_child(_info)
	_menu = PopupMenu.new()
	_menu.id_pressed.connect(_on_menu)
	_root.add_child(_menu)
	_build_dialog()
	main.world_map.waypoints.changed.connect(view.refresh)


func _build_dialog() -> void:
	_dialog = PanelContainer.new()
	_dialog.visible = false
	_dialog.set_anchors_preset(Control.PRESET_CENTER)
	var v := VBoxContainer.new()
	_dialog.add_child(v)
	v.add_child(UiStyle.label("Waypoint name", 16, UiStyle.ACCENT))
	_name_edit = LineEdit.new()
	_name_edit.custom_minimum_size = Vector2(260, 0)
	_name_edit.text_submitted.connect(func(_t: String) -> void: _dialog_ok())
	v.add_child(_name_edit)
	var h := HBoxContainer.new()
	v.add_child(h)
	var ok := Button.new()
	ok.text = "OK"
	ok.pressed.connect(_dialog_ok)
	h.add_child(ok)
	var cancel := Button.new()
	cancel.text = "Cancel"
	cancel.pressed.connect(func() -> void: _dialog.visible = false)
	h.add_child(cancel)
	_root.add_child(_dialog)


func open() -> void:
	is_open = true
	visible = true
	var p: Vector3 = main.player.position
	view.center = Vector2(p.x, p.z)
	view.zoom = ZOOMS[_zoom_i]
	view.refresh()
	_update_info(_root.get_local_mouse_position())


func close() -> void:
	if not is_open:
		return
	is_open = false
	visible = false
	_dialog.visible = false
	_menu.hide()
	closed.emit()


func _process(_delta: float) -> void:
	if is_open:
		view.refresh()   # the player arrow and freshly explored cells


func _unhandled_input(event: InputEvent) -> void:
	if not is_open or not (event is InputEventKey) or not event.pressed or event.echo:
		return
	if _dialog.visible:
		if event.keycode == KEY_ESCAPE:
			_dialog.visible = false
			get_viewport().set_input_as_handled()
		return
	if event.is_action("map") or event.keycode == KEY_ESCAPE:
		close()
		get_viewport().set_input_as_handled()
	elif event.keycode == KEY_SPACE:
		var p: Vector3 = main.player.position
		view.center = Vector2(p.x, p.z)
		get_viewport().set_input_as_handled()


func _on_input(event: InputEvent) -> void:
	if event is InputEventMouseButton and event.pressed:
		match event.button_index:
			MOUSE_BUTTON_LEFT:
				_drag = true
			MOUSE_BUTTON_WHEEL_UP, MOUSE_BUTTON_WHEEL_DOWN:
				var before := view.to_world(event.position)
				_zoom_i = clampi(_zoom_i + (1 if event.button_index == MOUSE_BUTTON_WHEEL_UP else -1), 0, ZOOMS.size() - 1)
				view.zoom = ZOOMS[_zoom_i]
				view.center += before - view.to_world(event.position)   # the spot under the cursor stays put
			MOUSE_BUTTON_RIGHT:
				_open_menu(event.position)
	elif event is InputEventMouseButton and not event.pressed and event.button_index == MOUSE_BUTTON_LEFT:
		_drag = false
	elif event is InputEventMouseMotion:
		if _drag:
			view.center -= event.relative / view.zoom * WorldMap.PX
		_update_info(event.position)


func _update_info(local: Vector2) -> void:
	var w := view.to_world(local)
	_info.text = "Map   x %d  z %d   (drag: move, wheel: zoom, Space: you, right click: waypoint, M: close)" % [int(w.x), int(w.y)]


## The pin under a screen position (within 9 px), or {}.
func _pin_at(local: Vector2) -> Dictionary:
	var best := {}
	var bd := 9.0
	for w in main.world_map.waypoints.list:
		var p: Vector3i = w["pos"]
		var d := view.to_screen(Vector2(p.x, p.z)).distance_to(local)
		if d < bd:
			bd = d
			best = w
	return best


func _open_menu(local: Vector2) -> void:
	_menu.clear()
	_menu_world = view.to_world(local)
	var pin := _pin_at(local)
	_menu_wp = int(pin.get("id", -1))
	if pin.is_empty():
		_menu.add_item("Add a waypoint here", 0)
	else:
		_menu.add_item("Rename %s" % pin["name"], 1)
		_menu.add_item("Next colour", 2)
		_menu.add_item("Delete %s" % pin["name"], 3)
	_menu.position = Vector2i(_root.get_global_mouse_position())
	_menu.reset_size()
	_menu.popup()


func _on_menu(id: int) -> void:
	var wps: Waypoints = main.world_map.waypoints
	match id:
		0:
			_rename_id = -1
			_ask_name("")
		1:
			_rename_id = _menu_wp
			_ask_name(String(wps.get_waypoint(_menu_wp).get("name", "")))
		2:
			wps.cycle_colour(_menu_wp)
		3:
			wps.remove(_menu_wp)


func _ask_name(text: String) -> void:
	_name_edit.text = text
	_dialog.visible = true
	_dialog.reset_size()
	_dialog.position = (_root.size - _dialog.size) / 2.0
	_name_edit.grab_focus()
	_name_edit.select_all()


func _dialog_ok() -> void:
	_dialog.visible = false
	var wps: Waypoints = main.world_map.waypoints
	var n := _name_edit.text.strip_edges()
	if _rename_id >= 0:
		wps.rename(_rename_id, n)
		return
	var x := int(_menu_world.x)
	var z := int(_menu_world.y)
	var y: int = main.world.surface_height(x, z) + 1 if main.world else int(main.player.position.y)
	wps.add(n, Vector3i(x, y, z))
