class_name TextScreen
extends CanvasLayer

## One small page for the words on a sign or a note (ADR 0164): a parchment panel with a multi-line box
## capped at `max_lines` x `max_cols`, Done and Cancel. Read-only for a note you only look at. The text
## that leaves is SignText.clean()ed again by whoever stores it.

signal done(text: String)
signal closed

const PANEL := Vector2(460, 330)

var is_open := false
var _edit: TextEdit
var _title: Label
var _count: Label
var _done: Button
var _lines := 4
var _cols := 15
var _panel: Control


func _ready() -> void:
	layer = 11
	visible = false
	var root := Control.new()
	root.set_anchors_preset(Control.PRESET_FULL_RECT)
	root.mouse_filter = Control.MOUSE_FILTER_STOP
	root.draw.connect(func() -> void: root.draw_rect(Rect2(Vector2.ZERO, root.size), Color(0, 0, 0, 0.45)))
	add_child(root)
	_panel = Control.new()
	_panel.set_anchors_preset(Control.PRESET_CENTER)
	_panel.size = PANEL
	_panel.position = -PANEL / 2.0
	_panel.draw.connect(func() -> void:
		var sb := StyleBoxFlat.new()
		sb.bg_color = Color(0.93, 0.89, 0.76)
		sb.border_color = Color(0.36, 0.25, 0.14)
		sb.set_border_width_all(4)
		sb.set_corner_radius_all(4)
		_panel.draw_style_box(sb, Rect2(Vector2.ZERO, _panel.size)))
	root.add_child(_panel)
	_title = Label.new()
	_title.position = Vector2(18, 12)
	_title.add_theme_color_override("font_color", Color(0.25, 0.17, 0.1))
	_panel.add_child(_title)
	_edit = TextEdit.new()
	_edit.position = Vector2(18, 44)
	_edit.size = PANEL - Vector2(36, 120)
	_edit.add_theme_font_override("font", FurnitureEntity.label_font())
	_edit.add_theme_font_size_override("font_size", 20)
	_edit.add_theme_color_override("font_color", Color(0.13, 0.08, 0.05))
	_edit.add_theme_color_override("background_color", Color(0.98, 0.95, 0.85))
	_edit.add_theme_color_override("font_readonly_color", Color(0.13, 0.08, 0.05))
	_edit.text_changed.connect(_limit)
	_panel.add_child(_edit)
	_count = Label.new()
	_count.position = Vector2(18, PANEL.y - 66)
	_count.add_theme_color_override("font_color", Color(0.4, 0.3, 0.2))
	_count.add_theme_font_size_override("font_size", 13)
	_panel.add_child(_count)
	_done = Button.new()
	_done.text = "Done"
	_done.position = Vector2(PANEL.x - 200, PANEL.y - 52)
	_done.custom_minimum_size = Vector2(84, 36)
	_done.pressed.connect(_finish)
	_panel.add_child(_done)
	var cancel := Button.new()
	cancel.text = "Cancel"
	cancel.position = Vector2(PANEL.x - 106, PANEL.y - 52)
	cancel.custom_minimum_size = Vector2(88, 36)
	cancel.pressed.connect(close)
	_panel.add_child(cancel)


## Opens the page. `editable` false: reading only (no Done).
func open(title: String, text: String, max_lines: int, max_cols: int, editable := true) -> void:
	_lines = max_lines
	_cols = max_cols
	_title.text = title
	_edit.text = text
	_edit.editable = editable
	_done.visible = editable
	_edit.size = PANEL - Vector2(36, 120)
	_update_count()
	is_open = true
	visible = true
	if editable:
		_edit.grab_focus()
		_edit.set_caret_line(_edit.get_line_count() - 1)
		_edit.set_caret_column(_edit.get_line(_edit.get_line_count() - 1).length())


func close() -> void:
	if not is_open:
		return
	is_open = false
	visible = false
	closed.emit()


func _finish() -> void:
	var t := SignText.clean(_edit.text, _lines, _cols)
	done.emit(t)
	close()


## Keeps the box inside lines x columns while typing (the final clean is SignText's).
func _limit() -> void:
	var line := _edit.get_caret_line()
	var col := _edit.get_caret_column()
	var changed := false
	while _edit.get_line_count() > _lines:
		_edit.remove_line_at(_edit.get_line_count() - 1)
		changed = true
	for i in _edit.get_line_count():
		var l := _edit.get_line(i)
		if l.length() > _cols:
			_edit.set_line(i, l.left(_cols))
			changed = true
	if changed:
		_edit.set_caret_line(mini(line, _edit.get_line_count() - 1))
		_edit.set_caret_column(mini(col, _edit.get_line(_edit.get_caret_line()).length()))
	_update_count()


func _update_count() -> void:
	_count.text = "%d lines of %d characters, %d / %d lines" % [_lines, _cols, _edit.get_line_count(), _lines]
