class_name SettingsPanel
extends Control

## The Settings screen (ADR 0032), shared by the title screen and the pause
## menu. Video: a graphics preset (low / medium / high, from ADR 0031's
## measurements) and the values it sets, which can be changed one by one
## (the preset then reads "Custom"), plus field of view, vsync, frame cap and
## fullscreen. Audio: master, music, effects and interface volume (ADR 0044).
## Controls: mouse sensitivity, invert Y and the key list. Every
## change applies at once and is saved to user://settings.cfg.

signal done

const PRESETS := ["low", "medium", "high"]
const SHADOWS := ["off", "low", "medium", "high"]
const VIEW_M := Settings.VIEW_CHOICES_M   # ADR 0073: the one box of full-detail voxels; 40 m is the budget's edge
## Keys that cannot be rebound (the rest are rows above them, plan item D11).
const KEYS := [["Space twice", "fly (creative)"], ["G", "world tools panel: brush, build modes, keys"], ["Ctrl + wheel", "brush size, laser width"], ["B / Shift + B", "brush shape: next / previous"],
	["Shift + right click", "paint the brush with the held block (creative)"], ["Ctrl + Q", "drop the stack"],
	["1-9, wheel", "hotbar"], ["Alt + wheel", "turn the inventory rows into the hotbar"], ["Double click", "gather a stack (Shift: move all of it)"], ["R / U / A", "recipes / uses / bookmark (in the inventory)"], ["Alt + click", "favourite slot (in the inventory)"],
	["/", "commands"], ["F3 / F5", "debug screen / camera view"], ["F1 / F2", "hide the HUD / screenshot"], ["F9", "bug report"], ["F4 / F6", "playtest menu / tuning panel"], ["Esc", "pause menu"]]
var _key_buttons := {}     # action -> Button
var _rebinding := ""       # the action waiting for a key

var _preset: OptionButton
var _shadows: OptionButton
var _view: OptionButton
var _ssao: CheckBox
var _bloom: CheckBox
var _fog: CheckBox
var _bright: HSlider
var _bright_label: Label
var _fov: HSlider
var _fov_label: Label
var _vsync: CheckBox
var _fps: OptionButton
var _fullscreen: CheckBox
var _mini_corner: OptionButton
var _mini_size: OptionButton
var _gui: OptionButton
var _jei_shown: CheckBox
var _jei_cols: OptionButton
const MINIMAP_CORNERS := ["top_right", "top_left", "bottom_right", "bottom_left"]
const MINIMAP_SIZES := [0, 120, 160, 220, 300]
var _sens: HSlider
var _sens_label: Label
var _invert: CheckBox
var _toggle_sprint: CheckBox
var _toggle_sneak: CheckBox
var _auto_step: CheckBox
var _coords: CheckBox
var _effects: CheckBox
var _volumes := {}   # Settings key -> HSlider
var _syncing := false


func _ready() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	theme = UiStyle.theme()
	var cp := UiStyle.centered_panel(self, 640.0)
	var box: VBoxContainer = cp[1]
	box.add_child(UiStyle.label("Settings", 28))
	var tabs := TabContainer.new()
	tabs.custom_minimum_size = Vector2(600, 380)
	box.add_child(tabs)
	tabs.add_child(_video_tab())
	tabs.add_child(_audio_tab())
	tabs.add_child(_controls_tab())
	tabs.add_child(_interface_tab())
	var row := HBoxContainer.new()
	row.alignment = BoxContainer.ALIGNMENT_END
	box.add_child(row)
	row.add_child(UiStyle.button("Done", func() -> void: done.emit(), 160))
	sync_from_settings()


func _grid(tab_name: String) -> GridContainer:
	var g := GridContainer.new()
	g.name = tab_name
	g.columns = 2
	g.add_theme_constant_override("h_separation", 24)
	g.add_theme_constant_override("v_separation", 6)
	return g


func _row(g: GridContainer, text: String, c: Control) -> void:
	var l := UiStyle.label(text)
	l.custom_minimum_size.x = 210
	g.add_child(l)
	c.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	g.add_child(c)


func _options(items: Array, cb: Callable) -> OptionButton:
	var o := OptionButton.new()
	for it in items:
		o.add_item(it)
	o.item_selected.connect(func(_i: int) -> void:
		if not _syncing:
			cb.call())
	return o


func _check(cb: Callable) -> CheckBox:
	var c := CheckBox.new()
	c.text = "On"
	c.toggled.connect(func(_v: bool) -> void:
		if not _syncing:
			cb.call())
	return c


func _video_tab() -> Control:
	var g := _grid("Video")
	_preset = _options(["Low", "Medium", "High", "Custom"], func() -> void:
		if _preset.selected < PRESETS.size():
			Settings.apply_graphics_preset(PRESETS[_preset.selected])
		_edited())
	_preset.tooltip_text = "Low and Medium trade shadows and detail for speed; High is the default look."
	_row(g, "Graphics", _preset)
	_shadows = _options(["Off", "Low", "Medium", "High"], func() -> void:
		Settings.shadow_quality = SHADOWS[_shadows.selected]
		_edited())
	_row(g, "Shadows", _shadows)
	var names: Array = []
	for v in VIEW_M:
		names.append("%d m" % int(v))
	_view = _options(names, func() -> void:
		Settings.view_distance_m = VIEW_M[_view.selected]
		_edited())
	_view.tooltip_text = "How far the world is drawn, at full detail all the way; it ends in the sky. Further costs frame time and memory."
	_row(g, "Render distance", _view)
	_ssao = _check(func() -> void:
		Settings.ssao = _ssao.button_pressed
		_edited())
	_row(g, "Ambient occlusion", _ssao)
	_bloom = _check(func() -> void:
		Settings.bloom = _bloom.button_pressed
		_edited())
	_row(g, "Bloom", _bloom)
	_fog = _check(func() -> void:
		Settings.fog = _fog.button_pressed
		_edited())
	_fog.tooltip_text = "The haze and lowland mist above ground. Caves and water keep their own."
	_row(g, "Fog", _fog)
	var bright_row := HBoxContainer.new()
	_bright = HSlider.new()
	_bright.min_value = 0.0
	_bright.max_value = 1.0
	_bright.step = 0.05
	_bright.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_bright.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	_bright.value_changed.connect(func(v: float) -> void:
		_bright_label.text = "%d%%" % roundi(v * 100.0)
		if not _syncing:
			Settings.brightness = v
			_edited())
	bright_row.add_child(_bright)
	_bright_label = UiStyle.label("50%")
	_bright_label.custom_minimum_size.x = 48
	bright_row.add_child(_bright_label)
	_bright.tooltip_text = "Lifts shadows, caves and night (50% is the graded look)."
	_row(g, "Brightness", bright_row)
	var fov_row := HBoxContainer.new()
	_fov = HSlider.new()
	_fov.min_value = 60
	_fov.max_value = 100
	_fov.step = 1
	_fov.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_fov.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	_fov.value_changed.connect(func(v: float) -> void:
		_fov_label.text = "%d°" % int(v)
		if not _syncing:
			Settings.fov = v
			_edited())
	fov_row.add_child(_fov)
	_fov_label = UiStyle.label("75°")
	_fov_label.custom_minimum_size.x = 48
	fov_row.add_child(_fov_label)
	_row(g, "Field of view", fov_row)
	_vsync = _check(func() -> void:
		Settings.vsync = _vsync.button_pressed
		_edited())
	_row(g, "VSync", _vsync)
	var fps_items := []
	for f in Settings.MAX_FPS_CHOICES:
		fps_items.append("Unlimited" if f == 0 else "%d fps" % f)
	_fps = _options(fps_items, func() -> void:
		Settings.max_fps = Settings.MAX_FPS_CHOICES[_fps.selected]
		_edited())
	_row(g, "Frame rate cap", _fps)
	_fullscreen = _check(func() -> void:
		Settings.fullscreen = _fullscreen.button_pressed
		_edited())
	_row(g, "Fullscreen", _fullscreen)
	_mini_corner = _options(["Top right", "Top left", "Bottom right", "Bottom left"], func() -> void:
		Settings.minimap_corner = MINIMAP_CORNERS[_mini_corner.selected]
		_edited())
	_row(g, "Minimap corner", _mini_corner)
	_mini_size = _options(["Off", "Small", "Medium", "Large", "Huge"], func() -> void:
		Settings.minimap_size_px = MINIMAP_SIZES[_mini_size.selected]
		_edited())
	_row(g, "Minimap size", _mini_size)
	var gui_items := []
	for x in Settings.GUI_SCALES:
		gui_items.append("%d%%" % roundi(x * 100.0))
	_gui = _options(gui_items, func() -> void:
		Settings.gui_scale = Settings.GUI_SCALES[_gui.selected]
		_edited())
	_row(g, "GUI scale", _gui)
	_jei_shown = _check(func() -> void:
		Settings.jei_shown = _jei_shown.button_pressed
		_edited())
	_row(g, "Recipe browser (JEI)", _jei_shown)
	var jei_items := []
	for c in Settings.JEI_COLS:
		jei_items.append("Auto" if c == 0 else "%d columns" % c)
	_jei_cols = _options(jei_items, func() -> void:
		Settings.jei_cols = Settings.JEI_COLS[_jei_cols.selected]
		_edited())
	_row(g, "Recipe browser width", _jei_cols)
	return g


## HUD extras (ADR 0130), each its own switch.
func _interface_tab() -> Control:
	var g := _grid("Interface")
	_coords = _check(func() -> void:
		Settings.hud_coords = _coords.button_pressed
		_edited())
	_row(g, "Coordinates and compass", _coords)
	_effects = _check(func() -> void:
		Settings.screen_effects = _effects.button_pressed
		_edited())
	_row(g, "Hurt, cold and heat effects", _effects)
	return g


## Master, music, effects and interface volume (ADR 0044), 0-100 %.
func _audio_tab() -> Control:
	var g := _grid("Audio")
	for v in [["Master volume", "volume_master"], ["Music", "volume_music"], ["Effects", "volume_effects"], ["Interface", "volume_ui"]]:
		var row := HBoxContainer.new()
		var s := HSlider.new()
		s.min_value = 0.0
		s.max_value = 1.0
		s.step = 0.05
		s.size_flags_horizontal = Control.SIZE_EXPAND_FILL
		s.size_flags_vertical = Control.SIZE_SHRINK_CENTER
		var l := UiStyle.label("100%")
		l.custom_minimum_size.x = 56
		var key: String = v[1]
		s.value_changed.connect(func(x: float) -> void:
			l.text = "%d%%" % roundi(x * 100.0)
			if not _syncing:
				Settings.set(key, x)
				_edited())
		row.add_child(s)
		row.add_child(l)
		_row(g, v[0], row)
		_volumes[key] = s
	return g


func _controls_tab() -> Control:
	var scroll := ScrollContainer.new()   # the key rows make it taller than the tab
	scroll.name = "Controls"
	scroll.horizontal_scroll_mode = ScrollContainer.SCROLL_MODE_DISABLED
	var v := VBoxContainer.new()
	v.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	scroll.add_child(v)
	v.add_theme_constant_override("separation", 14)
	var g := _grid("Options")
	v.add_child(g)
	var sens_row := HBoxContainer.new()
	_sens = HSlider.new()
	_sens.min_value = 0.2
	_sens.max_value = 3.0
	_sens.step = 0.05
	_sens.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_sens.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	_sens.value_changed.connect(func(x: float) -> void:
		_sens_label.text = "%d%%" % roundi(x * 100.0)
		if not _syncing:
			Settings.mouse_sensitivity = 0.0025 * x
			_edited())
	sens_row.add_child(_sens)
	_sens_label = UiStyle.label("100%")
	_sens_label.custom_minimum_size.x = 56
	sens_row.add_child(_sens_label)
	_row(g, "Mouse sensitivity", sens_row)
	_invert = _check(func() -> void:
		Settings.invert_y = _invert.button_pressed
		_edited())
	_row(g, "Invert mouse Y", _invert)
	_toggle_sprint = _check(func() -> void:
		Settings.toggle_sprint = _toggle_sprint.button_pressed
		_edited())
	_row(g, "Toggle sprint", _toggle_sprint)
	_toggle_sneak = _check(func() -> void:
		Settings.toggle_sneak = _toggle_sneak.button_pressed
		_edited())
	_row(g, "Toggle sneak", _toggle_sneak)
	_auto_step = _check(func() -> void:
		Settings.auto_step = _auto_step.button_pressed
		_edited())
	_row(g, "Auto step up blocks", _auto_step)
	v.add_child(UiStyle.label("Keys (click one, then press the new key or mouse button; Esc cancels)", 17, UiStyle.ACCENT))
	var binds := GridContainer.new()
	binds.columns = 4
	binds.add_theme_constant_override("h_separation", 16)
	for a in Settings.REMAPPABLE:
		binds.add_child(UiStyle.label(a[1], 15, UiStyle.DIM))
		var b := Button.new()
		b.custom_minimum_size = Vector2(130, 0)
		b.focus_mode = Control.FOCUS_NONE
		var action: String = a[0]
		b.pressed.connect(func() -> void:
			_rebinding = action
			_refresh_keys())
		binds.add_child(b)
		_key_buttons[action] = b
	v.add_child(binds)
	var reset := Button.new()
	reset.text = "Reset keys"
	reset.size_flags_horizontal = Control.SIZE_SHRINK_BEGIN
	reset.pressed.connect(func() -> void:
		Settings.reset_keys()
		_rebinding = ""
		_edited())
	v.add_child(reset)
	_refresh_keys()
	var keys := GridContainer.new()
	keys.columns = 2
	keys.add_theme_constant_override("h_separation", 24)
	for k in KEYS:
		keys.add_child(UiStyle.label(k[0], 15, UiStyle.ACCENT))
		keys.add_child(UiStyle.label(k[1], 15, UiStyle.DIM))
	v.add_child(keys)
	return scroll


func _refresh_keys() -> void:
	for action in _key_buttons:
		(_key_buttons[action] as Button).text = "press a key..." if action == _rebinding else Settings.key_text(Settings.current_key(action))


## While a key button waits: the next key or mouse button press becomes that action's binding.
func _input(event: InputEvent) -> void:
	if _rebinding == "" or not is_visible_in_tree():
		return
	var ev: InputEvent = null
	if event is InputEventKey and event.pressed and not event.echo:
		if event.keycode == KEY_ESCAPE:
			_rebinding = ""
			_refresh_keys()
			get_viewport().set_input_as_handled()
			return
		ev = InputEventKey.new()
		(ev as InputEventKey).physical_keycode = event.physical_keycode
	elif event is InputEventMouseButton and event.pressed and event.button_index in [MOUSE_BUTTON_LEFT, MOUSE_BUTTON_RIGHT, MOUSE_BUTTON_MIDDLE, MOUSE_BUTTON_XBUTTON1, MOUSE_BUTTON_XBUTTON2]:
		if event.button_index == MOUSE_BUTTON_LEFT and _key_buttons[_rebinding].get_global_rect().has_point(event.global_position):
			return   # the click that started it
		ev = InputEventMouseButton.new()
		(ev as InputEventMouseButton).button_index = event.button_index
		(ev as InputEventMouseButton).pressed = true
	if ev == null:
		return
	Settings.bind_key(_rebinding, ev)
	_rebinding = ""
	get_viewport().set_input_as_handled()
	_edited()


## Every control shows the current Settings values (after a preset changed several).
func sync_from_settings() -> void:
	_syncing = true
	_preset.select(PRESETS.find(Settings.graphics_preset) if Settings.graphics_preset in PRESETS else PRESETS.size())
	_shadows.select(maxi(0, SHADOWS.find(Settings.shadow_quality)))
	_view.select(_nearest(VIEW_M, Settings.view_distance_m))
	_ssao.button_pressed = Settings.ssao
	_bloom.button_pressed = Settings.bloom
	_fog.button_pressed = Settings.fog
	_bright.value = Settings.brightness
	_bright_label.text = "%d%%" % roundi(Settings.brightness * 100.0)
	_fov.value = Settings.fov
	_fov_label.text = "%d°" % int(Settings.fov)
	_vsync.button_pressed = Settings.vsync
	_fps.select(maxi(0, Settings.MAX_FPS_CHOICES.find(Settings.max_fps)))
	_fullscreen.button_pressed = Settings.fullscreen
	_mini_corner.select(maxi(0, MINIMAP_CORNERS.find(Settings.minimap_corner)))
	_mini_size.select(_nearest(MINIMAP_SIZES, Settings.minimap_size_px))
	_gui.select(_nearest(Settings.GUI_SCALES, Settings.gui_scale))
	_jei_shown.button_pressed = Settings.jei_shown
	_jei_cols.select(_nearest(Settings.JEI_COLS, Settings.jei_cols))
	_sens.value = Settings.mouse_sensitivity / 0.0025
	_sens_label.text = "%d%%" % roundi(_sens.value * 100.0)
	_invert.button_pressed = Settings.invert_y
	_toggle_sprint.button_pressed = Settings.toggle_sprint
	_toggle_sneak.button_pressed = Settings.toggle_sneak
	_auto_step.button_pressed = Settings.auto_step
	_coords.button_pressed = Settings.hud_coords
	_effects.button_pressed = Settings.screen_effects
	_refresh_keys()
	for k in _volumes:
		_volumes[k].value = Settings.get(k)
	_syncing = false


static func _nearest(choices: Array, v: float) -> int:
	var best := 0
	for i in choices.size():
		if absf(choices[i] - v) < absf(choices[best] - v):
			best = i
	return best


func _edited() -> void:
	Settings.user_settings_edited()
	sync_from_settings()
