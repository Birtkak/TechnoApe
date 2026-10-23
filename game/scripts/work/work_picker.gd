class_name WorkPicker
extends CanvasLayer

## Recipe picker for a fresh knapping or clay piece (ADR 0023, Vintage
## Story's recipe selection): one button per recipe with the output's icon
## and name. A click emits `chosen`; Esc closes without choosing (the piece
## stays on the ground, a right click opens the picker again).

signal chosen(recipe: Dictionary)
signal closed

var is_open := false
var _root: Control
var _grid: GridContainer
var _title: Label
var _options: Array = []


func _ready() -> void:
	layer = 11
	visible = false
	_root = Control.new()
	_root.set_anchors_preset(Control.PRESET_FULL_RECT)
	_root.mouse_filter = Control.MOUSE_FILTER_STOP
	_root.draw.connect(func() -> void: _root.draw_rect(Rect2(Vector2.ZERO, _root.size), Color(0, 0, 0, 0.45)))
	add_child(_root)
	var panel := PanelContainer.new()
	panel.set_anchors_preset(Control.PRESET_CENTER)
	panel.grow_horizontal = Control.GROW_DIRECTION_BOTH
	panel.grow_vertical = Control.GROW_DIRECTION_BOTH
	_root.add_child(panel)
	var box := VBoxContainer.new()
	box.add_theme_constant_override("separation", 10)
	panel.add_child(box)
	_title = Label.new()
	_title.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	_title.add_theme_font_size_override("font_size", 18)
	box.add_child(_title)
	_grid = GridContainer.new()
	_grid.columns = 3
	_grid.add_theme_constant_override("h_separation", 8)
	_grid.add_theme_constant_override("v_separation", 8)
	box.add_child(_grid)
	var hint := Label.new()
	hint.text = "Pick what to make  ·  Esc: later"
	hint.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	hint.add_theme_font_size_override("font_size", 12)
	box.add_child(hint)


## options: recipe dictionaries; extra(r) gives a line under the name ("4 clay").
func open(title: String, options: Array, items: ItemRegistry, extra: Callable = Callable()) -> void:
	_options = options
	_title.text = title
	for c in _grid.get_children():
		c.queue_free()
	for r in options:
		var b := Button.new()
		b.custom_minimum_size = Vector2(150, 56)
		b.alignment = HORIZONTAL_ALIGNMENT_LEFT
		var out_id: String = r["output"]["id"]
		b.text = "        " + items.display_name(out_id) + (("\n        " + str(extra.call(r))) if extra.is_valid() else "")
		var icon := ItemIcon.new()
		icon.items = items
		icon.stack = {"id": out_id, "count": 1}
		icon.size = Vector2(32, 32)
		icon.position = Vector2(8, 12)
		icon.mouse_filter = Control.MOUSE_FILTER_IGNORE
		b.add_child(icon)
		b.pressed.connect(_choose.bind(r))
		_grid.add_child(b)
	is_open = true
	visible = true


func _choose(r: Dictionary) -> void:
	close()
	chosen.emit(r)


func close() -> void:
	if not is_open:
		return
	is_open = false
	visible = false
	closed.emit()


## Automation (autopilot / tests): choose by recipe id.
func choose_id(rid: String) -> bool:
	for r in _options:
		if r["id"] == rid:
			_choose(r)
			return true
	return false


func _unhandled_input(event: InputEvent) -> void:
	if is_open and event.is_action_pressed("ui_cancel"):
		get_viewport().set_input_as_handled()
		close()
