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
const RENDER_M := [16.0, 24.0]   # Settings.MIN_DETAIL_M .. MAX_DETAIL_M
const VIEW_M := [1024.0, 2048.0]
const KEYS := [["W A S D", "walk"], ["Space", "jump; fly up (twice: fly, creative)"], ["Shift", "sprint; with right click, place against a station"],
	["Ctrl / F", "fly down / toggle flying (creative)"],
	["Left / right click", "break / use, place"], ["E", "inventory, progression book top left"], ["Q", "drop (Ctrl: the stack)"],
	["1-9, wheel", "hotbar"], ["R / U / A", "recipes / uses / bookmark (in the inventory)"], ["T, /", "chat and commands"],
	["F3 / F5", "debug screen / camera view"], ["F2 / F9", "screenshot / bug report"], ["F4", "playtest menu"], ["Esc", "pause menu"]]

var _preset: OptionButton
var _shadows: OptionButton
var _render: OptionButton
var _view: OptionButton
var _ssao: CheckBox
var _bloom: CheckBox
var _fov: HSlider
var _fov_label: Label
var _vsync: CheckBox
var _fps: OptionButton
var _fullscreen: CheckBox
var _sens: HSlider
var _sens_label: Label
var _invert: CheckBox
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
	_render = _options(["16 m", "24 m"], func() -> void:
		Settings.render_distance_m = RENDER_M[_render.selected]
		Settings.lod_ring_m = Settings.render_distance_m
		_edited())
	_row(g, "Full detail to", _render)
	_view = _options(["1 km", "2 km"], func() -> void:
		Settings.view_distance_m = VIEW_M[_view.selected]
		_edited())
	_row(g, "View distance", _view)
	_ssao = _check(func() -> void:
		Settings.ssao = _ssao.button_pressed
		_edited())
	_row(g, "Ambient occlusion", _ssao)
	_bloom = _check(func() -> void:
		Settings.bloom = _bloom.button_pressed
		_edited())
	_row(g, "Bloom", _bloom)
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
	var v := VBoxContainer.new()
	v.name = "Controls"
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
	v.add_child(UiStyle.label("Keys", 19, UiStyle.ACCENT))
	var keys := GridContainer.new()
	keys.columns = 2
	keys.add_theme_constant_override("h_separation", 24)
	for k in KEYS:
		keys.add_child(UiStyle.label(k[0], 15, UiStyle.ACCENT))
		keys.add_child(UiStyle.label(k[1], 15, UiStyle.DIM))
	v.add_child(keys)
	return v


## Every control shows the current Settings values (after a preset changed several).
func sync_from_settings() -> void:
	_syncing = true
	_preset.select(PRESETS.find(Settings.graphics_preset) if Settings.graphics_preset in PRESETS else PRESETS.size())
	_shadows.select(maxi(0, SHADOWS.find(Settings.shadow_quality)))
	_render.select(_nearest(RENDER_M, Settings.render_distance_m))
	_view.select(_nearest(VIEW_M, Settings.view_distance_m))
	_ssao.button_pressed = Settings.ssao
	_bloom.button_pressed = Settings.bloom
	_fov.value = Settings.fov
	_fov_label.text = "%d°" % int(Settings.fov)
	_vsync.button_pressed = Settings.vsync
	_fps.select(maxi(0, Settings.MAX_FPS_CHOICES.find(Settings.max_fps)))
	_fullscreen.button_pressed = Settings.fullscreen
	_sens.value = Settings.mouse_sensitivity / 0.0025
	_sens_label.text = "%d%%" % roundi(_sens.value * 100.0)
	_invert.button_pressed = Settings.invert_y
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
