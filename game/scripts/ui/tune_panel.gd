class_name TunePanel
extends CanvasLayer

## Live tuning panel (F6): one row per `var` on the Settings autoload, built by
## reflection, so every tunable gets a control without anyone writing one.
## Changes apply at once; Save writes the values that differ from the defaults
## to user://tune.cfg, which normal launches load after the playtest prefs
## (autopilot runs never read it, the way user settings work). Reset deletes
## that file and restores the defaults.

signal opened
signal closed

const TUNE_FILE := "user://tune.cfg"

var can_open: Callable
var is_open := false
var _root: Control
var _panel: PanelContainer
var _filter: LineEdit
var _rows: Dictionary = {}     # name -> HBoxContainer
var _defaults: Dictionary = {} # name -> the script's default (a fresh Settings instance, so Reset undoes a loaded tune too)


## Names of every script var on Settings, in declaration order.
static func tunable_names() -> PackedStringArray:
	var out := PackedStringArray()
	for p in Settings.get_property_list():
		if p.usage & PROPERTY_USAGE_SCRIPT_VARIABLE and not String(p.name).begins_with("_"):
			out.append(p.name)
	return out


## Normal launches call this from Settings.apply_cmdline: saved tunes win over the script defaults.
static func load_tune() -> void:
	var cf := ConfigFile.new()
	if cf.load(TUNE_FILE) != OK:
		return
	for key in cf.get_section_keys("tune"):
		if key in Settings:
			Settings.set(key, cf.get_value("tune", key))


func _ready() -> void:
	layer = 9
	_root = Control.new()
	_root.set_anchors_preset(Control.PRESET_FULL_RECT)
	_root.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_root.theme = UiStyle.theme()
	add_child(_root)
	_build()


func _build() -> void:
	_panel = PanelContainer.new()
	_panel.add_theme_stylebox_override("panel", UiStyle.panel())
	_panel.set_anchors_preset(Control.PRESET_CENTER_RIGHT)
	_panel.grow_horizontal = Control.GROW_DIRECTION_BEGIN
	_panel.grow_vertical = Control.GROW_DIRECTION_BOTH
	_panel.visible = false
	_root.add_child(_panel)
	var box := VBoxContainer.new()
	_panel.add_child(box)
	box.add_child(UiStyle.label("Tune   (F6 / Esc to close; settings read once at load need a restart)", 15, UiStyle.DIM))
	var top := HBoxContainer.new()
	box.add_child(top)
	_filter = LineEdit.new()
	_filter.placeholder_text = "filter"
	_filter.custom_minimum_size.x = 260
	_filter.text_changed.connect(_apply_filter)
	top.add_child(_filter)
	top.add_child(UiStyle.button("Save", _save, 90))
	top.add_child(UiStyle.button("Reset", _reset, 90))
	var scroll := ScrollContainer.new()
	scroll.custom_minimum_size = Vector2(520, 620)
	scroll.horizontal_scroll_mode = ScrollContainer.SCROLL_MODE_DISABLED
	box.add_child(scroll)
	var list := VBoxContainer.new()
	list.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	scroll.add_child(list)
	var fresh: Node = Settings.get_script().new()
	for key in tunable_names():
		var value = Settings.get(key)
		var ctl := _control_for(key, value)
		if ctl == null:
			continue
		_defaults[key] = fresh.get(key)
		var row := HBoxContainer.new()
		var l := UiStyle.label(key, 14)
		l.size_flags_horizontal = Control.SIZE_EXPAND_FILL
		row.add_child(l)
		row.add_child(ctl)
		list.add_child(row)
		_rows[key] = row
	fresh.free()


## A control that edits Settings[key] in place, or null for types the panel does not show.
func _control_for(key: String, value) -> Control:
	match typeof(value):
		TYPE_BOOL:
			var c := CheckBox.new()
			c.button_pressed = value
			c.toggled.connect(func(v): Settings.set(key, v))
			return c
		TYPE_INT, TYPE_FLOAT:
			var s := SpinBox.new()
			s.custom_minimum_size.x = 150
			s.allow_greater = true
			s.allow_lesser = true
			s.min_value = -1e9
			s.max_value = 1e9
			if typeof(value) == TYPE_INT:
				s.step = 1
				s.value_changed.connect(func(v): Settings.set(key, int(v)))
			else:
				s.step = 0.001
				s.value_changed.connect(func(v): Settings.set(key, v))
			s.value = value
			return s
		TYPE_STRING, TYPE_VECTOR3I, TYPE_VECTOR3, TYPE_VECTOR2:
			var e := LineEdit.new()
			e.custom_minimum_size.x = 150
			e.text = _to_text(value)
			e.text_submitted.connect(func(t): Settings.set(key, _from_text(t, Settings.get(key))))
			return e
	return null


static func _to_text(value) -> String:
	if typeof(value) == TYPE_STRING:
		return value
	return ",".join(Array(str(value).trim_prefix("(").trim_suffix(")").split(", ")))


## Parses the row's text back into the type the setting already has; bad text keeps the old value.
static func _from_text(t: String, old):
	match typeof(old):
		TYPE_STRING:
			return t
		TYPE_VECTOR2, TYPE_VECTOR3, TYPE_VECTOR3I:
			var parts := t.split(",")
			var n := 2 if typeof(old) == TYPE_VECTOR2 else 3
			if parts.size() != n:
				return old
			var f: Array[float] = []
			for p in parts:
				if not p.strip_edges().is_valid_float():
					return old
				f.append(float(p))
			match typeof(old):
				TYPE_VECTOR2: return Vector2(f[0], f[1])
				TYPE_VECTOR3: return Vector3(f[0], f[1], f[2])
				_: return Vector3i(int(f[0]), int(f[1]), int(f[2]))
	return old


func _apply_filter(text: String) -> void:
	for key in _rows:
		_rows[key].visible = text.is_empty() or key.containsn(text)


func _save() -> void:
	var cf := ConfigFile.new()
	for key in _rows:
		if Settings.get(key) != _defaults[key]:
			cf.set_value("tune", key, Settings.get(key))
	cf.save(TUNE_FILE)


func _reset() -> void:
	DirAccess.remove_absolute(TUNE_FILE)
	for key in _rows:
		Settings.set(key, _defaults[key])
		var ctl: Control = _rows[key].get_child(1)
		if ctl is CheckBox:
			ctl.button_pressed = _defaults[key]
		elif ctl is SpinBox:
			ctl.value = _defaults[key]
		else:
			ctl.text = _to_text(_defaults[key])


# ---------------- open / close ----------------

func open() -> void:
	if is_open or (can_open.is_valid() and not can_open.call()):
		return
	_panel.visible = true
	is_open = true
	opened.emit()


func close() -> void:
	if not is_open:
		return
	_panel.visible = false
	is_open = false
	closed.emit()


func _unhandled_input(event: InputEvent) -> void:
	if not (event is InputEventKey) or not event.pressed or event.echo:
		return
	if event.keycode == KEY_F6:
		close() if is_open else open()
	elif event.keycode == KEY_ESCAPE and is_open:
		close()
	else:
		return
	get_viewport().set_input_as_handled()
