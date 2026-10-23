class_name PauseMenu
extends CanvasLayer

## The Esc menu in a world (ADR 0032). In singleplayer it pauses the game
## tree: the simulation, animals, the calendar and the player stop until
## "Back to game" (or Esc again). Settings opens the shared Settings screen;
## "Save and quit to title" and "Save and quit" write every save file first.
## It also opens when the window loses focus, like Minecraft.

signal resumed
signal quit_to_title
signal quit_game

var is_open := false
var _root: Control
var _menu: Control
var _settings: SettingsPanel
var _world_label: Label
var _saved_label: Label


func _ready() -> void:
	layer = 15   # above the HUD, chat and playtest menu; under nothing
	process_mode = Node.PROCESS_MODE_ALWAYS
	_root = Control.new()
	_root.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_root.theme = UiStyle.theme()
	_root.mouse_filter = Control.MOUSE_FILTER_STOP
	_root.draw.connect(func() -> void: _root.draw_rect(Rect2(Vector2.ZERO, _root.size), Color(0, 0, 0, 0.55)))
	_root.visible = false
	add_child(_root)
	var cp := UiStyle.centered_panel(_root, 380.0)
	_menu = cp[0]
	var box: VBoxContainer = cp[1]
	var title := UiStyle.label("Game paused", 30)
	title.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	box.add_child(title)
	_world_label = UiStyle.label("", 15, UiStyle.DIM)
	_world_label.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	box.add_child(_world_label)
	box.add_child(HSeparator.new())
	box.add_child(UiStyle.button("Back to game", resume))
	box.add_child(UiStyle.button("Settings", _open_settings))
	box.add_child(UiStyle.button("Save and quit to title", func() -> void: quit_to_title.emit()))
	box.add_child(UiStyle.button("Save and quit game", func() -> void: quit_game.emit()))
	_saved_label = UiStyle.label("The world saves itself every 30 seconds.", 13, UiStyle.DIM)
	_saved_label.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	box.add_child(_saved_label)
	_settings = SettingsPanel.new()
	_settings.visible = false
	_settings.done.connect(_close_settings)
	_root.add_child(_settings)


func open() -> void:
	if is_open:
		return
	is_open = true
	_world_label.text = "%s  ·  seed %s" % [Settings.world_name if Settings.world_name != "" else "Unnamed world",
		Settings.world_seed_text if Settings.world_seed_text != "" else str(Settings.world_seed)]
	_root.visible = true
	_menu.visible = true
	_settings.visible = false
	get_tree().paused = true   # singleplayer; a server keeps running when multiplayer comes (phase 4)
	Input.mouse_mode = Input.MOUSE_MODE_VISIBLE


func resume() -> void:
	if not is_open:
		return
	is_open = false
	_root.visible = false
	get_tree().paused = false
	resumed.emit()


func _open_settings() -> void:
	_settings.sync_from_settings()
	_menu.visible = false
	_settings.visible = true


func _close_settings() -> void:
	_settings.visible = false
	_menu.visible = true


func is_settings_open() -> bool:
	return is_open and _settings.visible


func _unhandled_input(event: InputEvent) -> void:
	if not is_open or not (event is InputEventKey) or not event.pressed or event.echo:
		return
	if event.keycode == KEY_ESCAPE:
		if _settings.visible:
			_close_settings()
		else:
			resume()
		get_viewport().set_input_as_handled()
