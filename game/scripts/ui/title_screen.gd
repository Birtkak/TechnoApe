class_name TitleScreen
extends Control

## The title screen (ADR 0032), the project's main scene. Singleplayer lists
## the worlds (WorldList): play one, create one (name, seed, game mode) or
## delete one. Multiplayer waits for the dedicated server (phase 4).
## Settings is the shared Settings screen. Autopilot runs and play=1 skip
## straight into the game with the command line's world, so every benchmark
## and debug run works as before. The backdrop is drawn here: a sky, a sun
## and three layers of blocky hills drifting past.

const GAME_SCENE := "res://scenes/main.tscn"

var worlds: WorldList
var _pages := {}          # name -> Control
var _rows: VBoxContainer
var _group := ButtonGroup.new()
var _shown: Array = []    # world entries in list order
var _sel := -1
var _play_btn: Button
var _delete_btn: Button
var _name_edit: LineEdit
var _seed_edit: LineEdit
var _mode: OptionButton
var _create_error: Label
var _confirm: ConfirmationDialog
var _old_gen: ConfirmationDialog   # the world's saves are from another generator version
var _settings: SettingsPanel
var _addr_edit: LineEdit        # the Multiplayer page (ADR 0145)
var _net_name_edit: LineEdit
var _join_status: Label
var _servers_label: Label
var _server_rows: VBoxContainer   # one button per server found (ServerFinder)
var _finder: ServerFinder
var _probe_t := 0.0
var _listed: Array = []
var _join_label := ""   # the server's name for the chat's greeting
var _t := 0.0


func _ready() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	theme = UiStyle.theme()
	if OS.get_cmdline_user_args().has("server=1"):
		return   # a headless server (ADR 0145): Net frees this scene, no pages to build
	if Net.connect_arg != "":   # connect=host[:port] name=<n>: join before anything else (ADR 0145)
		_join(Net.connect_arg, Net.player_name)
		return
	if skip_menu():
		_enter_game.call_deferred()
		return
	Settings.apply_display()
	Input.mouse_mode = Input.MOUSE_MODE_VISIBLE
	var embedded: bool = Engine.has_method("is_embedded_in_editor") and bool(Engine.call("is_embedded_in_editor"))
	if Settings.start_minimized and EngineDebugger.is_active() and not embedded and not OS.get_cmdline_user_args().has("menu_test=1") \
			and Engine.get_process_frames() < 5:
		DisplayServer.window_set_mode(DisplayServer.WINDOW_MODE_MINIMIZED)   # playtests from the editor start minimized
	worlds = WorldList.load_default()
	_build_main()
	_build_worlds()
	_build_create()
	_build_multiplayer()
	_settings = SettingsPanel.new()
	_settings.done.connect(show_page.bind("main"))
	add_child(_settings)
	_pages["settings"] = _settings
	_confirm = ConfirmationDialog.new()
	_confirm.title = "Delete world"
	_confirm.ok_button_text = "Delete"
	_confirm.confirmed.connect(_delete_selected)
	add_child(_confirm)
	_old_gen = ConfirmationDialog.new()
	_old_gen.title = "Older world"
	_old_gen.ok_button_text = "Play anyway"
	_old_gen.confirmed.connect(func() -> void: play(_selected()))
	add_child(_old_gen)
	show_page("main")
	if ClientUpdate.can_update() and Settings.net_address != "":   # the last server has a newer build: fetch it now (ADR 0166)
		ClientUpdate.run(self, Settings.net_address, Settings.net_name, true)
	if DebugCapture.has_method("menu_test_ready"):
		DebugCapture.menu_test_ready(self)


## Benchmarks and debug runs go straight into the game (their world comes from the command line).
static func skip_menu() -> bool:
	var args := OS.get_cmdline_user_args()
	return args.has("autopilot=1") or args.has("play=1")


func _enter_game() -> void:
	get_tree().change_scene_to_file(GAME_SCENE)


# ---------------- pages ----------------

func _page(page_name: String, width: float) -> VBoxContainer:
	var cp := UiStyle.centered_panel(self, width)
	_pages[page_name] = cp[0]
	return cp[1]


func show_page(page_name: String) -> void:
	for p in _pages:
		_pages[p].visible = p == page_name
	if page_name == "worlds":
		_refresh_worlds()
	elif page_name == "settings":
		_settings.sync_from_settings()
	elif page_name == "create":
		_name_edit.text = "New World"
		_seed_edit.text = ""
		_create_error.text = ""
		_name_edit.grab_focus()
		_name_edit.select_all()


func _build_main() -> void:
	var box := _page("main", 380.0)
	var pc: PanelContainer = box.get_parent()
	pc.add_theme_stylebox_override("panel", StyleBoxEmpty.new())   # buttons float on the backdrop
	var title := UiStyle.label("TechnoApe", 64)
	title.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	title.add_theme_constant_override("outline_size", 10)
	title.add_theme_color_override("font_outline_color", Color(0.1, 0.07, 0.04))
	box.add_child(title)
	var sub := UiStyle.label("Flint, fire, and then the machines", 18, UiStyle.ACCENT)
	sub.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	sub.add_theme_constant_override("outline_size", 6)
	sub.add_theme_color_override("font_outline_color", Color(0.1, 0.07, 0.04))
	box.add_child(sub)
	var gap := Control.new()
	gap.custom_minimum_size.y = 28
	box.add_child(gap)
	box.add_child(UiStyle.button("Singleplayer", show_page.bind("worlds"), 360))
	box.add_child(UiStyle.button("Multiplayer", show_page.bind("multiplayer"), 360))
	box.add_child(UiStyle.button("Settings", show_page.bind("settings"), 360))
	box.add_child(UiStyle.button("Quit", func() -> void: get_tree().quit(), 360))


func _build_worlds() -> void:
	var box := _page("worlds", 640.0)
	box.add_child(UiStyle.label("Select world", 28))
	var scroll := ScrollContainer.new()
	scroll.custom_minimum_size = Vector2(600, 320)
	scroll.horizontal_scroll_mode = ScrollContainer.SCROLL_MODE_DISABLED
	box.add_child(scroll)
	_rows = VBoxContainer.new()
	_rows.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_rows.add_theme_constant_override("separation", 6)
	scroll.add_child(_rows)
	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", 10)
	box.add_child(row)
	_play_btn = UiStyle.button("Play", _play_selected, 140)
	row.add_child(_play_btn)
	row.add_child(UiStyle.button("New world", show_page.bind("create"), 160))
	_delete_btn = UiStyle.button("Delete", _ask_delete, 120)
	row.add_child(_delete_btn)
	var spacer := Control.new()
	spacer.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	row.add_child(spacer)
	row.add_child(UiStyle.button("Back", show_page.bind("main"), 110))


func _build_create() -> void:
	var box := _page("create", 520.0)
	box.add_child(UiStyle.label("New world", 28))
	box.add_child(UiStyle.label("Name", 15, UiStyle.DIM))
	_name_edit = LineEdit.new()
	_name_edit.text_submitted.connect(func(_t: String) -> void: _create_and_play())
	box.add_child(_name_edit)
	box.add_child(UiStyle.label("Seed (empty: random; any text works)", 15, UiStyle.DIM))
	_seed_edit = LineEdit.new()
	_seed_edit.placeholder_text = "random"
	_seed_edit.text_submitted.connect(func(_t: String) -> void: _create_and_play())
	box.add_child(_seed_edit)
	box.add_child(UiStyle.label("Game mode", 15, UiStyle.DIM))
	_mode = OptionButton.new()
	_mode.add_item("Survival")
	_mode.add_item("Creative")
	box.add_child(_mode)
	_create_error = UiStyle.label("", 15, Color(1.0, 0.45, 0.4))
	box.add_child(_create_error)
	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", 10)
	box.add_child(row)
	row.add_child(UiStyle.button("Create world", _create_and_play, 200))
	row.add_child(UiStyle.button("Cancel", show_page.bind("worlds"), 120))


## Join a server (ADR 0145): address (a Tailscale name or IP, host:port), player name.
func _build_multiplayer() -> void:
	var box := _page("multiplayer", 560.0)
	box.add_child(UiStyle.label("Join a server", 28))
	box.add_child(UiStyle.label("Your name (the server keeps your player under it)", 15, UiStyle.DIM))
	_net_name_edit = LineEdit.new()
	_net_name_edit.text = Settings.net_name if Settings.net_name != "player" else OS.get_environment("USERNAME").to_lower()
	_net_name_edit.text_submitted.connect(func(_t: String) -> void: _join_from_page())
	box.add_child(_net_name_edit)
	_servers_label = UiStyle.label("Servers on this machine, the LAN and your tailnet", 15, UiStyle.DIM)
	box.add_child(_servers_label)
	_server_rows = VBoxContainer.new()
	_server_rows.add_theme_constant_override("separation", 6)
	box.add_child(_server_rows)
	box.add_child(UiStyle.label("Or an address (host or host:port)", 15, UiStyle.DIM))
	_addr_edit = LineEdit.new()
	_addr_edit.text = Settings.net_address
	_addr_edit.text_submitted.connect(func(_t: String) -> void: _join_from_page())
	box.add_child(_addr_edit)
	_join_status = UiStyle.label("", 15, UiStyle.ACCENT)
	box.add_child(_join_status)
	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", 10)
	box.add_child(row)
	row.add_child(UiStyle.button("Join", _join_from_page, 200))
	row.add_child(UiStyle.button("Back", show_page.bind("main"), 120))


func _join_from_page() -> void:
	_join_address(_addr_edit.text.strip_edges())


func _join_address(address: String, label := "") -> void:
	Settings.net_address = address
	_join_label = label
	Settings.net_name = _net_name_edit.text.strip_edges()
	Settings.save_user_settings()
	_join_status.text = "Connecting to %s..." % address
	_join(address, Settings.net_name)


## The server list: probes every 2 s while the page is open (the tailnet is read once per visit), one
## button per server that answered.
func _poll_servers(delta: float) -> void:
	if _finder == null:
		_finder = ServerFinder.new()
	if not _finder.tailnet_checked:
		_finder.read_tailnet()
	_probe_t -= delta
	if _probe_t <= 0.0:
		_probe_t = 2.0
		_finder.probe()
	_finder.poll()
	var keys := _finder.servers.keys()
	keys.sort()
	if keys == _listed:
		return
	_listed = keys
	for c in _server_rows.get_children():
		c.queue_free()
	if keys.is_empty():
		_server_rows.add_child(UiStyle.label("None answering yet" + (" (Tailscale: %d peers online)" % _finder.tailnet.size() if _finder.tailnet_checked else ""), 15, UiStyle.DIM))
		return
	for k in keys:
		var d: Dictionary = _finder.servers[k]
		var text := "%s  ·  seed %d  ·  %s  ·  %d playing  ·  %s" % [d.get("name", "?"), int(d.get("seed", 0)), String(d.get("game_mode", "")).capitalize(), int(d.get("players", 0)), k]
		if d.has("ver") and String(d["ver"]) != ClientUpdate.version():
			text += "  ·  updates on join"
		var b := UiStyle.button(text, _join_address.bind(k, String(d.get("name", ""))), 520)
		if int(d.get("proto", 0)) != Net.PROTO:
			b.disabled = true
			b.tooltip_text = "Another game version"
		_server_rows.add_child(b)


func _join(address: String, pname: String) -> void:
	Net.welcomed.connect(func() -> void:
		Settings.world_seed = Net.seed_value
		Settings.set_seed_text(str(Net.seed_value))
		Settings.world_name = _join_label if _join_label != "" else address
		Settings.game_mode = Net.game_mode
		Settings.persist_world = false   # the server holds the save
		ClientUpdate.clean_up()
		_enter_game.call_deferred(), CONNECT_ONE_SHOT)
	Net.failed.connect(func(reason: String) -> void:
		print("net: join failed: " + reason)
		if reason.begins_with(ClientUpdate.UPDATE_PREFIX):   # another build: fetch it, restart, join (ADR 0166)
			multiplayer.multiplayer_peer = null
			if ClientUpdate.can_update():
				ClientUpdate.run(self, address, pname)
				return
			reason = "the server runs build %s, this is %s (only an exported game updates itself)" % [
				reason.trim_prefix(ClientUpdate.UPDATE_PREFIX), ClientUpdate.version()]
		if _join_status:
			_join_status.text = "Could not join: " + reason
		elif Net.connect_arg != "":
			get_tree().quit(2), CONNECT_ONE_SHOT)
	Net.join(address, pname)


# ---------------- worlds ----------------

## One two-line button per world, newest first; a click selects, a double click plays.
func _refresh_worlds() -> void:
	for c in _rows.get_children():
		c.queue_free()
	_shown = worlds.sorted()
	_sel = 0 if not _shown.is_empty() else -1
	for i in _shown.size():
		var e: Dictionary = _shown[i]
		var when := Time.get_datetime_string_from_unix_time(int(e.get("last_played", 0)), true)
		var b := Button.new()
		b.toggle_mode = true
		b.button_group = _group
		b.alignment = HORIZONTAL_ALIGNMENT_LEFT
		b.text = "%s\n%s  ·  seed %s  ·  last played %s" % [e["name"], String(e.get("game_mode", "survival")).capitalize(),
			e.get("seed_text", str(e["seed"])), when.substr(0, 16).replace("T", " ")]
		b.custom_minimum_size = Vector2(0, 62)
		b.button_pressed = i == 0
		b.toggled.connect(func(on: bool) -> void:
			if on:
				_sel = i
				_update_world_buttons())
		b.gui_input.connect(func(ev: InputEvent) -> void:
			if ev is InputEventMouseButton and ev.double_click and ev.button_index == MOUSE_BUTTON_LEFT:
				_sel = i
				_play_selected())
		_rows.add_child(b)
	if _shown.is_empty():
		_rows.add_child(UiStyle.label("No worlds yet: make one with New world.", 16, UiStyle.DIM))
	_update_world_buttons()


func _selected() -> Dictionary:
	return _shown[_sel] if _sel >= 0 and _sel < _shown.size() else {}


func _update_world_buttons() -> void:
	var none := _selected().is_empty()
	_play_btn.disabled = none
	_delete_btn.disabled = none


func _play_selected() -> void:
	var e := _selected()
	if e.is_empty():
		return
	var g := worlds.saved_gen(e)
	if g >= 0 and g != TerrainGenerator.GEN_VERSION:
		_old_gen.dialog_text = ("\"%s\" was saved with world generator %d; this version has %d. It opens as a new world "
			+ "with the same seed: new terrain, a fresh player. The old save stays on disk.") % [e["name"], g, TerrainGenerator.GEN_VERSION]
		_old_gen.popup_centered()
		return
	play(e)


func play(e: Dictionary) -> void:
	worlds.touch(e)
	WorldList.select(e)
	_enter_game.call_deferred()


func _create_and_play() -> void:
	var e := worlds.create(_name_edit.text, _seed_edit.text, "creative" if _mode.selected == 1 else "survival")
	if e.has("error"):
		_create_error.text = e["error"]
		return
	play(e)


func _ask_delete() -> void:
	var e := _selected()
	if e.is_empty():
		return
	_confirm.dialog_text = "Delete \"%s\" for good? Its save files are removed and cannot be brought back." % e["name"]
	_confirm.popup_centered()


func _delete_selected() -> void:
	var e := _selected()
	if not e.is_empty():
		worlds.delete(e)
		_refresh_worlds()


# ---------------- the backdrop ----------------

func _process(delta: float) -> void:
	_t += delta
	queue_redraw()
	if _server_rows and _pages.has("multiplayer") and _pages["multiplayer"].visible:
		_poll_servers(delta)
	elif _finder:
		_finder = null   # the page closed: probe again fresh next time
		_listed = []


func _draw() -> void:
	var s := size
	# sky: deep blue at the top to a warm haze at the horizon
	var bands := 24
	for i in bands:
		var f := float(i) / bands
		var c := Color(0.16, 0.32, 0.62).lerp(Color(0.98, 0.72, 0.46), pow(f, 1.6))
		draw_rect(Rect2(0, s.y * 0.7 * f, s.x, s.y * 0.7 / bands + 1), c)
	draw_rect(Rect2(0, s.y * 0.7, s.x, s.y * 0.3), Color(0.98, 0.72, 0.46))
	# a blocky sun
	var sun := Vector2(s.x * 0.72, s.y * 0.5)
	for r in [70.0, 52.0, 38.0]:
		var a := 0.12 if r == 70.0 else (0.25 if r == 52.0 else 1.0)
		draw_rect(Rect2(sun - Vector2(r, r), Vector2(r, r) * 2), Color(1.0, 0.9, 0.6, a))
	# three hill layers of blocky columns, nearer ones darker and faster
	var layers := [[0.58, 0.10, 18.0, Color(0.36, 0.42, 0.52)], [0.68, 0.12, 34.0, Color(0.24, 0.36, 0.26)], [0.80, 0.10, 60.0, Color(0.13, 0.22, 0.13)]]
	for li in layers.size():
		var L: Array = layers[li]
		var cell := 16.0 + li * 8.0
		var off := fmod(_t * float(L[2]), cell * 512.0)
		var x := -fmod(off, cell)
		var k := int(off / cell)
		while x < s.x:
			var n := k + int(x / cell) + li * 1000
			var h := sin(n * 0.21) * 0.5 + sin(n * 0.057 + li) * 0.35 + sin(n * 0.013) * 0.6
			var top: float = s.y * (float(L[0]) - h * float(L[1]))
			top = floorf(top / cell) * cell
			draw_rect(Rect2(x, top, cell + 1, s.y - top), L[3])
			if li == 2:
				draw_rect(Rect2(x, top, cell + 1, cell * 0.35), Color(0.25, 0.42, 0.2))   # grass edge on the near hills
			x += cell
	# blocky clouds drifting the other way
	for i in 5:
		var cx := fmod(i * 397.0 + _t * 9.0, s.x + 300.0) - 150.0
		var cy := s.y * (0.12 + 0.07 * (i % 3))
		for j in 4:
			draw_rect(Rect2(cx + j * 28.0, cy + (8.0 if j % 2 else 0.0), 56, 22), Color(1, 1, 1, 0.55))
	var ver := "Dev build · Phase 2"
	draw_string(ThemeDB.fallback_font, Vector2(12, s.y - 12), ver, HORIZONTAL_ALIGNMENT_LEFT, -1, 14, Color(1, 1, 1, 0.7))
