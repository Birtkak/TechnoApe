class_name ChatBox
extends CanvasLayer

## Minecraft-style chat (ADR 0018). T opens it, / opens it with the slash
## already typed. Enter sends, Esc closes, Up/Down walk the sent history, Tab
## cycles completions (Shift+Tab backwards). Lines starting with / run through
## the CommandRegistry; anything else is said as chat. Messages stay at the
## bottom left for 10 s and fade; while the box is open the last 20 show.
## Main pauses player input while it is open (opened / closed signals).

signal opened
signal closed

const WIDTH := 460
const SHOW_S := 10.0
const FADE_S := 1.0
const CLOSED_LINES := 10
const OPEN_LINES := 20
const KEEP := 100
const HISTORY := 50
const CHAT := Color(1, 1, 1)
const FEEDBACK := Color(0.78, 0.78, 0.78)
const ERROR := Color(1.0, 0.36, 0.36)
const INFO := Color(1.0, 1.0, 0.4)

var commands: CommandRegistry
var player_name := "Ape"
var can_open: Callable          # returns false while another screen owns input
var is_open := false
var messages: Array = []        # {text, color, t}
var _history: Array[String] = []
var history_path := "user://chat_history.txt"   # the sent lines survive a restart (ADR 0130); "" keeps them in memory
var _hist_i := 0                # == _history.size() while editing a fresh line
var _draft := ""
var _completions: Array[String] = []
var _comp_i := -1
var _clock := 0.0
var _lines: VBoxContainer
var _labels: Array[Label] = []
var _shown: Array = []          # message per label, or null
var _field: LineEdit
var _hint: Label


func _ready() -> void:
	layer = 8   # above the HUD (5), below inventory screens (10)
	load_history()
	_lines = VBoxContainer.new()
	_lines.set_anchors_preset(Control.PRESET_BOTTOM_LEFT)
	_lines.offset_left = 8
	_lines.offset_right = 8 + WIDTH
	_lines.offset_top = -50
	_lines.offset_bottom = -50
	_lines.grow_vertical = Control.GROW_DIRECTION_BEGIN
	_lines.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_lines.add_theme_constant_override("separation", 0)
	_lines.alignment = BoxContainer.ALIGNMENT_END   # newest line stays at the bottom while older ones fade
	add_child(_lines)
	var line_bg := StyleBoxFlat.new()
	line_bg.bg_color = Color(0, 0, 0, 0.42)
	line_bg.content_margin_left = 6
	line_bg.content_margin_right = 6
	line_bg.content_margin_top = 1
	line_bg.content_margin_bottom = 1
	for i in OPEN_LINES:
		var l := Label.new()
		l.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
		l.custom_minimum_size = Vector2(WIDTH, 0)
		l.mouse_filter = Control.MOUSE_FILTER_IGNORE
		l.add_theme_stylebox_override("normal", line_bg)
		l.add_theme_font_size_override("font_size", 15)
		l.add_theme_color_override("font_shadow_color", Color(0, 0, 0, 0.85))
		l.add_theme_constant_override("shadow_offset_x", 1)
		l.add_theme_constant_override("shadow_offset_y", 1)
		l.visible = false
		_lines.add_child(l)
		_labels.append(l)
		_shown.append(null)

	_field = LineEdit.new()
	_field.set_anchors_preset(Control.PRESET_BOTTOM_WIDE)
	_field.offset_left = 8
	_field.offset_right = -8
	_field.offset_top = -42
	_field.offset_bottom = -10
	_field.max_length = 256
	_field.context_menu_enabled = false
	_field.caret_blink = true
	_field.add_theme_font_size_override("font_size", 16)
	var in_bg := StyleBoxFlat.new()
	in_bg.bg_color = Color(0, 0, 0, 0.6)
	in_bg.border_color = Color(1, 1, 1, 0.18)
	in_bg.set_border_width_all(1)
	in_bg.content_margin_left = 8
	in_bg.content_margin_right = 8
	_field.add_theme_stylebox_override("normal", in_bg)
	_field.add_theme_stylebox_override("focus", in_bg)
	_field.visible = false
	_field.text_submitted.connect(_on_submitted)
	_field.text_changed.connect(_on_text_changed)
	add_child(_field)

	_hint = Label.new()
	_hint.set_anchors_preset(Control.PRESET_BOTTOM_LEFT)
	_hint.offset_left = 8
	_hint.offset_top = -46
	_hint.offset_bottom = -46
	_hint.grow_vertical = Control.GROW_DIRECTION_BEGIN
	_hint.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_hint.add_theme_font_size_override("font_size", 14)
	_hint.add_theme_color_override("font_color", Color(0.85, 0.85, 0.85))
	var hint_bg := StyleBoxFlat.new()
	hint_bg.bg_color = Color(0.02, 0.02, 0.03, 0.9)
	hint_bg.set_content_margin_all(6)
	_hint.add_theme_stylebox_override("normal", hint_bg)
	_hint.visible = false
	add_child(_hint)


func open(prefix: String = "") -> void:
	if is_open:
		return
	is_open = true
	_field.text = prefix
	_field.visible = true
	_field.grab_focus()
	_field.caret_column = prefix.length()
	_hist_i = _history.size()
	_draft = ""
	_comp_i = -1
	_refresh_lines()
	_update_hint()
	opened.emit()


func close() -> void:
	if not is_open:
		return
	is_open = false
	_field.release_focus()
	_field.visible = false
	_field.text = ""
	_hint.visible = false
	_refresh_lines()
	closed.emit()


## Runs a line as if the player had typed it and pressed Enter (also used by
## the autopilot's commands= argument). Returns the command result.
func send(text: String) -> Dictionary:
	text = text.strip_edges()
	if text == "":
		return CommandRegistry.ok()
	if not text.begins_with("/"):
		add_message("<%s> %s" % [player_name, text], CHAT)
		return CommandRegistry.ok()
	var r := commands.execute(text) if commands != null else CommandRegistry.fail("Commands are not available")
	if String(r["text"]) != "":
		for line in String(r["text"]).split("\n"):
			add_message(line, FEEDBACK if r["ok"] else ERROR)
	return r


func add_message(text: String, color: Color = INFO) -> void:
	messages.append({"text": text, "color": color, "t": _clock})
	if messages.size() > KEEP:
		messages = messages.slice(messages.size() - KEEP)
	_refresh_lines()


## The last n message texts, oldest first (debug JSON).
func recent(n: int = 10) -> Array:
	var out := []
	for m in messages.slice(maxi(0, messages.size() - n)):
		out.append(m["text"])
	return out


func _unhandled_input(event: InputEvent) -> void:
	if is_open or not (event is InputEventKey) or not event.pressed or event.echo:
		return
	if can_open.is_valid() and not can_open.call():
		return
	if event.is_action_pressed("chat"):
		open("")
		get_viewport().set_input_as_handled()
	elif event.unicode == 47 or event.keycode == KEY_SLASH or event.keycode == KEY_KP_DIVIDE:
		# a slash on any keyboard layout opens the box with "/" typed
		open("/")
		get_viewport().set_input_as_handled()


## Before the LineEdit sees keys: Esc, history and completion belong to the chat.
func _input(event: InputEvent) -> void:
	if not is_open or not (event is InputEventKey) or not event.pressed:
		return
	match event.keycode:
		KEY_ESCAPE:
			close()
		KEY_UP:
			_history_step(-1)
		KEY_DOWN:
			_history_step(1)
		KEY_TAB:
			_tab(event.shift_pressed)
		_:
			return
	get_viewport().set_input_as_handled()


func _process(delta: float) -> void:
	_clock += delta
	if is_open:
		return
	for i in _labels.size():
		var m: Variant = _shown[i]
		if m == null:
			continue
		var a := clampf((SHOW_S - (_clock - float(m["t"]))) / FADE_S, 0.0, 1.0)
		_labels[i].modulate.a = a
		_labels[i].visible = a > 0.0


func _refresh_lines() -> void:
	if _labels.is_empty():
		return
	var n := OPEN_LINES if is_open else CLOSED_LINES
	var shown := messages.slice(maxi(0, messages.size() - n))
	for i in _labels.size():
		var l := _labels[i]
		if i < shown.size():
			l.text = shown[i]["text"]
			l.add_theme_color_override("font_color", shown[i]["color"])
			l.modulate.a = 1.0
			l.visible = true
			_shown[i] = shown[i]
		else:
			l.visible = false
			_shown[i] = null


func _on_submitted(text: String) -> void:
	var t := text.strip_edges()
	close()
	if t == "":
		return
	if _history.is_empty() or _history[-1] != t:
		_history.append(t)
		if _history.size() > HISTORY:
			_history.remove_at(0)
		_save_history()
	send(t)


func _on_text_changed(_t: String) -> void:
	_comp_i = -1
	_update_hint()


func load_history() -> void:
	var f := FileAccess.open(history_path, FileAccess.READ) if history_path != "" else null
	if f == null:
		return
	_history.clear()
	for line in f.get_as_text().split("\n", false):
		_history.append(line)
	_history = _history.slice(maxi(0, _history.size() - HISTORY))
	_hist_i = _history.size()


func _save_history() -> void:
	var f := FileAccess.open(history_path, FileAccess.WRITE) if history_path != "" else null
	if f != null:
		f.store_string("\n".join(_history))


func _history_step(d: int) -> void:
	if _history.is_empty():
		return
	if _hist_i == _history.size():
		_draft = _field.text
	_hist_i = clampi(_hist_i + d, 0, _history.size())
	_set_text(_draft if _hist_i == _history.size() else _history[_hist_i])


func _tab(backwards: bool) -> void:
	if commands == null:
		return
	if _comp_i < 0:
		_completions = commands.complete(_field.text)
		if _completions.is_empty():
			return
		_comp_i = _completions.size() - 1 if backwards else 0
	else:
		_comp_i = posmod(_comp_i + (-1 if backwards else 1), _completions.size())
	_set_text(_completions[_comp_i])


## Programmatic text (history, completion): no text_changed, so a Tab cycle survives.
func _set_text(t: String) -> void:
	_field.text = t
	_field.caret_column = t.length()
	_update_hint()


## Usage of the command being typed, plus what Tab would offer next.
func _update_hint() -> void:
	var text := _field.text
	if not is_open or commands == null or not text.begins_with("/"):
		_hint.visible = false
		return
	var tokens := CommandRegistry.tokenize(text.substr(1))
	var name := tokens[0] if tokens.size() > 0 else ""
	var lines: Array[String] = []
	var cmd := commands.find(name)
	if not cmd.is_empty() and (tokens.size() > 1 or text.ends_with(" ")):
		lines.append(cmd["usage"])
		var opts: Array[String] = commands.complete(text) if _comp_i < 0 else _completions
		if not opts.is_empty():
			var last: Array[String] = []
			for o in opts.slice(0, 8):
				last.append(o.substr(o.rfind(" ") + 1))
			lines.append("Tab: " + ", ".join(last) + (" ..." if opts.size() > 8 else ""))
	else:
		for n in commands.names():
			if String(n).begins_with(name.to_lower()):
				lines.append(commands.find(n)["usage"])
	_hint.text = "\n".join(lines)
	_hint.visible = not lines.is_empty()
	# collapse to the bottom-left corner; the minimum size grows it up and right again
	_hint.offset_top = -46
	_hint.offset_bottom = -46
	_hint.offset_right = 8
