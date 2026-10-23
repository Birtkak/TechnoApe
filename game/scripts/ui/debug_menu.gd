class_name DebugMenu
extends CanvasLayer

## Playtest menu (ADR 0018). F4 (or the "Debug" button top right while the
## mouse is free) opens a panel of buttons that run slash commands through
## the chat, so every button also works typed and shows its result in chat.
## F9 starts a bug report (screenshot of that moment + a description box),
## F2 saves a screenshot. "Start options" are saved to user://playtest.cfg
## and applied on the next normal launch (Settings.load_playtest_prefs).

signal opened
signal closed

var main: Node
var can_open: Callable
var is_open := false
var _root: Control
var _panel: PanelContainer
var _bug_panel: PanelContainer
var _bug_text: TextEdit
var _bug_shot: Image
var _corner: Button
var _opt_creative: CheckBox
var _opt_minimized: CheckBox
var _opt_noon: CheckBox
var _opt_kit: OptionButton
var _opt_extra: LineEdit
var _loading_prefs := false

const KIT_CHOICES := ["none", "tools", "builder", "stations", "magic"]


func _ready() -> void:
	layer = 9   # above chat (8), below inventory screens (10)
	_root = Control.new()
	_root.set_anchors_preset(Control.PRESET_FULL_RECT)
	_root.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_root.theme = _make_theme()
	add_child(_root)
	_corner = Button.new()
	_corner.text = "Debug (F4)"
	_corner.focus_mode = Control.FOCUS_NONE
	_corner.set_anchors_preset(Control.PRESET_TOP_RIGHT)
	_corner.offset_left = -112
	_corner.offset_right = -8
	_corner.offset_top = 8
	_corner.offset_bottom = 38
	_corner.visible = false
	_corner.pressed.connect(open.bind(false))
	_root.add_child(_corner)
	_build_panel()
	_build_bug_panel()
	_load_prefs()


# ---------------- layout ----------------

## Raised buttons and toggles that read on the dark panel.
func _make_theme() -> Theme:
	var t := Theme.new()
	var states := {"normal": Color(0.17, 0.17, 0.21), "hover": Color(0.27, 0.27, 0.33),
		"pressed": Color(0.42, 0.36, 0.2), "hover_pressed": Color(0.48, 0.42, 0.24), "focus": Color(0, 0, 0, 0)}
	for type in ["Button", "CheckBox", "OptionButton"]:
		for state in states:
			var sb := StyleBoxFlat.new()
			sb.bg_color = states[state]
			sb.set_corner_radius_all(5)
			sb.set_content_margin_all(6)
			sb.content_margin_left = 10
			sb.content_margin_right = 10
			if state == "focus":
				sb.draw_center = false
			else:
				sb.border_color = Color(1, 1, 1, 0.12)
				sb.set_border_width_all(1)
			t.set_stylebox(state, type, sb)
	return t


func _panel_style() -> StyleBoxFlat:
	var sb := StyleBoxFlat.new()
	sb.bg_color = Color(0.06, 0.06, 0.08, 0.94)
	sb.border_color = Color(1, 1, 1, 0.18)
	sb.set_border_width_all(1)
	sb.set_corner_radius_all(8)
	sb.set_content_margin_all(14)
	return sb


func _build_panel() -> void:
	_panel = PanelContainer.new()
	_panel.add_theme_stylebox_override("panel", _panel_style())
	_panel.set_anchors_preset(Control.PRESET_CENTER)
	_panel.custom_minimum_size = Vector2(760, 0)
	_panel.grow_horizontal = Control.GROW_DIRECTION_BOTH
	_panel.grow_vertical = Control.GROW_DIRECTION_BOTH
	_panel.visible = false
	_root.add_child(_panel)
	var scroll := ScrollContainer.new()
	scroll.custom_minimum_size = Vector2(740, 560)
	scroll.horizontal_scroll_mode = ScrollContainer.SCROLL_MODE_DISABLED
	_panel.add_child(scroll)
	var box := VBoxContainer.new()
	box.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	box.add_theme_constant_override("separation", 6)
	scroll.add_child(box)

	var title := Label.new()
	title.text = "Playtest menu   (F4 / Esc to close, results show in chat)"
	title.add_theme_font_size_override("font_size", 18)
	box.add_child(title)

	_section(box, "Game mode", [
		["Survival", "/gamemode survival"], ["Creative", "/gamemode creative"], ["Toggle fly", _toggle_fly],
		["Heal", "/heal"], ["Kill", "/kill"], ["Clear inventory", "/clear"]])
	_section(box, "Kits", [
		["Tools", "/kit tools"], ["Building blocks", "/kit builder"], ["Stations + ores", "/kit stations"], ["Wand + spells", "/kit magic"]])
	_section(box, "Time", [
		["Sunrise", "/time set sunrise"], ["Noon", "/time set noon"], ["Sunset", "/time set sunset"], ["Midnight", "/time set midnight"],
		["Day cycle on", "/daycycle on"], ["Freeze sun", "/daycycle off"]])
	_section(box, "Teleport (closes the menu)", [
		["Spawn", "!/spawn"], ["Up 50 m", "!/tp ~ ~100 ~"], ["Cave", "!/goto cave"], ["Deep cave", "!/goto deep_cave"], ["Ruin", "!/goto ruin"],
		["Waterfall", "!/goto waterfall"], ["Lake", "!/goto lake"], ["River", "!/goto river"], ["Mountain", "!/goto mountain"], ["Snow", "!/goto snow"],
		["Desert", "!/goto desert"], ["Forest", "!/goto forest"]])
	_section(box, "World tests (closes the menu)", [
		["Place stations", "!/stations"], ["Crater 4 m", "!/blast 4"], ["Water ball", "!/blast 2 water"], ["Sand ball", "!/blast 2 sand"],
		["Lava ball", "!/blast 1 lava"], ["Oil ball", "!/blast 2 oil"]])
	_section(box, "Debug", [
		["F3 overlay", _toggle_f3], ["Camera view (F5)", _cycle_view], ["Screenshot (F2)", _menu_screenshot], ["Report a bug (F9)", _menu_bug]])

	var opts_title := Label.new()
	opts_title.text = "Start options (saved, used every time you press Play)"
	opts_title.add_theme_color_override("font_color", Color(1, 0.9, 0.55))
	box.add_child(opts_title)
	var opts := HFlowContainer.new()
	opts.add_theme_constant_override("h_separation", 14)
	box.add_child(opts)
	_opt_creative = _check(opts, "Start in creative")
	_opt_minimized = _check(opts, "Start minimized (editor Play)")
	_opt_noon = _check(opts, "Start at noon, sun frozen")
	var kit_row := HBoxContainer.new()
	var kit_label := Label.new()
	kit_label.text = "Start kit:"
	kit_row.add_child(kit_label)
	_opt_kit = OptionButton.new()
	for k in KIT_CHOICES:
		_opt_kit.add_item(k)
	_opt_kit.item_selected.connect(func(_i: int) -> void: _save_prefs())
	kit_row.add_child(_opt_kit)
	opts.add_child(kit_row)
	_opt_extra = LineEdit.new()
	_opt_extra.placeholder_text = "More start commands, separated by ;  e.g.  /goto lake; /give glass 64"
	_opt_extra.text_changed.connect(func(_t: String) -> void: _save_prefs())
	box.add_child(_opt_extra)
	var hint := Label.new()
	hint.text = "Keys: T chat, / command, Tab completes, F3 debug screen, F9 bug report, F2 screenshot. Creative: middle click picks a block,\nclick items in the recipe list (E) to take stacks, double-tap Space or F to fly."
	hint.add_theme_font_size_override("font_size", 13)
	hint.add_theme_color_override("font_color", Color(0.7, 0.7, 0.7))
	box.add_child(hint)


## buttons: [label, "/command" | "!/command" (closes the menu first) | Callable]
func _section(box: VBoxContainer, title: String, buttons: Array) -> void:
	var l := Label.new()
	l.text = title
	l.add_theme_color_override("font_color", Color(1, 0.9, 0.55))
	box.add_child(l)
	var flow := HFlowContainer.new()
	flow.add_theme_constant_override("h_separation", 6)
	flow.add_theme_constant_override("v_separation", 6)
	box.add_child(flow)
	for b in buttons:
		var btn := Button.new()
		btn.text = b[0]
		btn.focus_mode = Control.FOCUS_NONE
		if b[1] is Callable:
			btn.pressed.connect(b[1])
		else:
			btn.pressed.connect(_run.bind(String(b[1])))
		flow.add_child(btn)


func _check(parent: Control, text: String) -> CheckBox:
	var c := CheckBox.new()
	c.text = text
	c.focus_mode = Control.FOCUS_NONE
	c.toggled.connect(func(_on: bool) -> void: _save_prefs())
	parent.add_child(c)
	return c


func _build_bug_panel() -> void:
	_bug_panel = PanelContainer.new()
	_bug_panel.add_theme_stylebox_override("panel", _panel_style())
	_bug_panel.set_anchors_preset(Control.PRESET_CENTER)
	_bug_panel.grow_horizontal = Control.GROW_DIRECTION_BOTH
	_bug_panel.grow_vertical = Control.GROW_DIRECTION_BOTH
	_bug_panel.visible = false
	_root.add_child(_bug_panel)
	var box := VBoxContainer.new()
	box.add_theme_constant_override("separation", 8)
	_bug_panel.add_child(box)
	var title := Label.new()
	title.text = "Report a bug   (screenshot, game state and log are saved with it)"
	title.add_theme_font_size_override("font_size", 18)
	box.add_child(title)
	_bug_text = TextEdit.new()
	_bug_text.custom_minimum_size = Vector2(560, 160)
	_bug_text.placeholder_text = "What went wrong? What did you expect instead? How did you get there?"
	_bug_text.wrap_mode = TextEdit.LINE_WRAPPING_BOUNDARY
	box.add_child(_bug_text)
	var row := HBoxContainer.new()
	row.alignment = BoxContainer.ALIGNMENT_END
	var cancel := Button.new()
	cancel.text = "Cancel (Esc)"
	cancel.pressed.connect(_close_bug)
	row.add_child(cancel)
	var save := Button.new()
	save.text = "Save report (Ctrl+Enter)"
	save.pressed.connect(_save_bug)
	row.add_child(save)
	box.add_child(row)


# ---------------- open / close ----------------

## force: open even over the autopilot (debug_menu=1 captures).
func open(force: bool = false) -> void:
	if is_open:
		return
	if not force and can_open.is_valid() and not can_open.call():
		return
	_panel.visible = true
	_set_open(true)


func close() -> void:
	_panel.visible = false
	_bug_panel.visible = false
	_set_open(false)


func _set_open(v: bool) -> void:
	if v == is_open:
		return
	is_open = v
	if v:
		opened.emit()
	else:
		closed.emit()


func _process(_delta: float) -> void:
	_corner.visible = not is_open and Input.mouse_mode == Input.MOUSE_MODE_VISIBLE \
		and can_open.is_valid() and bool(can_open.call())


func _unhandled_input(event: InputEvent) -> void:
	if not (event is InputEventKey) or not event.pressed or event.echo:
		return
	match event.keycode:
		KEY_F4:
			if _panel.visible:
				close()
			elif not is_open:
				open()
		KEY_F9:
			if not _bug_panel.visible:
				start_bug_report()
		KEY_F2:
			screenshot()
		_:
			return
	get_viewport().set_input_as_handled()


## Esc and Ctrl+Enter before the text box sees them.
func _input(event: InputEvent) -> void:
	if not is_open or not (event is InputEventKey) or not event.pressed:
		return
	if event.keycode == KEY_ESCAPE:
		if _bug_panel.visible:
			_close_bug()
		else:
			close()
		get_viewport().set_input_as_handled()
	elif _bug_panel.visible and event.ctrl_pressed and (event.keycode == KEY_ENTER or event.keycode == KEY_KP_ENTER):
		_save_bug()
		get_viewport().set_input_as_handled()
	elif get_viewport().gui_get_focus_owner() == null and not event.keycode in [KEY_F2, KEY_F3, KEY_F4, KEY_F9]:
		# no text field focused: the game under the menu gets no keys (E would toggle the inventory)
		get_viewport().set_input_as_handled()


# ---------------- actions ----------------

func _run(cmd: String) -> void:
	if cmd.begins_with("!"):
		close()
		cmd = cmd.substr(1)
	main.chat.send(cmd)


func _toggle_fly() -> void:
	var p: Player = main.player
	if GameMode.rule(p.game_mode, "can_fly"):
		p.flying = not p.flying
		main.chat.add_message("Flying %s" % ("on" if p.flying else "off"))
	else:
		main.chat.add_message("Flying needs creative (/gamemode creative)", ChatBox.ERROR)


func _toggle_f3() -> void:
	main.hud.toggle()


func _cycle_view() -> void:
	main.player.set_perspective((main.player.perspective + 1) % 3)


## F2: the current frame to out/screenshots (editor) or user://screenshots.
func screenshot(img: Image = null) -> String:
	var dir := "res://out/screenshots" if OS.has_feature("editor") else "user://screenshots"
	DirAccess.make_dir_recursive_absolute(ProjectSettings.globalize_path(dir))
	if img == null:
		img = get_viewport().get_texture().get_image()
	var path := "%s/%s.png" % [dir, Time.get_datetime_string_from_system().replace(":", "-").replace("T", "_")]
	img.save_png(path)
	main.chat.add_message("Saved screenshot as %s" % ProjectSettings.globalize_path(path))
	return path


func _menu_screenshot() -> void:
	close()
	for _i in 2:   # let a frame without the menu render first
		await get_tree().process_frame
	screenshot()


func _menu_bug() -> void:
	_panel.visible = false
	for _i in 2:
		await get_tree().process_frame
	start_bug_report()


## F9: keeps the frame on screen now, then asks what went wrong.
func start_bug_report() -> void:
	if not is_open and can_open.is_valid() and not can_open.call():
		return
	_bug_shot = get_viewport().get_texture().get_image()
	_panel.visible = false
	_bug_panel.visible = true
	_bug_text.text = ""
	_bug_text.grab_focus()
	_set_open(true)


func _close_bug() -> void:
	_bug_panel.visible = false
	_bug_text.release_focus()
	_bug_shot = null
	_set_open(_panel.visible)


func _save_bug() -> void:
	var path := BugReport.write(main, _bug_text.text.strip_edges(), _bug_shot)
	_close_bug()
	if path != "":
		main.chat.add_message("Bug report saved: %s" % path)
		main.chat.add_message("Tell Claude: \"look at the latest bug report\"", ChatBox.FEEDBACK)
	else:
		main.chat.add_message("Could not save the bug report", ChatBox.ERROR)


# ---------------- start options ----------------

func _load_prefs() -> void:
	var cf := ConfigFile.new()
	var ok := cf.load(Settings.PLAYTEST_PREFS) == OK
	_loading_prefs = true
	_opt_creative.button_pressed = ok and cf.get_value("start", "game_mode", "survival") == "creative"
	_opt_minimized.button_pressed = bool(cf.get_value("start", "minimized", true)) if ok else true
	_opt_noon.button_pressed = ok and bool(cf.get_value("start", "noon", false))
	_opt_kit.select(maxi(0, KIT_CHOICES.find(String(cf.get_value("start", "kit", "none")) if ok else "none")))
	_opt_extra.text = String(cf.get_value("start", "extra", "")) if ok else ""
	_loading_prefs = false


func _save_prefs() -> void:
	if _loading_prefs or _opt_extra == null:
		return
	var cmds := PackedStringArray()
	if _opt_noon.button_pressed:
		cmds.append("/time set noon")
		cmds.append("/daycycle off")
	var kit: String = KIT_CHOICES[maxi(0, _opt_kit.selected)]
	if kit != "none":
		cmds.append("/kit " + kit)
	for c in _opt_extra.text.split(";", false):
		if c.strip_edges() != "":
			cmds.append(c.strip_edges())
	var cf := ConfigFile.new()
	cf.set_value("start", "game_mode", "creative" if _opt_creative.button_pressed else "survival")
	cf.set_value("start", "minimized", _opt_minimized.button_pressed)
	cf.set_value("start", "noon", _opt_noon.button_pressed)
	cf.set_value("start", "kit", kit)
	cf.set_value("start", "extra", _opt_extra.text)
	cf.set_value("start", "commands", cmds)
	cf.save(Settings.PLAYTEST_PREFS)
