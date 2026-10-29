extends GdUnitTestSuite

## Headless tests for the menus (ADR 0032): the world list (create, seeds,
## old saves adopted, delete removes every file of a seed), graphics presets
## and the "custom" detection, the Settings screen showing Settings' values,
## and the title screen staying out of the way of autopilot runs.

const DIR := "user://test_worlds"

var _keep := {}


func before_test() -> void:
	DirAccess.make_dir_recursive_absolute(DIR)
	_wipe()
	for k in ["graphics_preset", "shadow_quality", "ssao", "bloom", "view_distance_m", "fov"]:
		_keep[k] = Settings.get(k)


func after_test() -> void:
	_wipe()
	for k in _keep:
		Settings.set(k, _keep[k])


func _wipe() -> void:
	var da := DirAccess.open(DIR)
	if da:
		for f in da.get_files():
			da.remove(f)


func _list() -> WorldList:
	var w := WorldList.new()
	w.dir = DIR
	w.load_index()
	return w


func _touch(file: String) -> void:
	var f := FileAccess.open(DIR + "/" + file, FileAccess.WRITE)
	f.store_string("x")
	f.close()


func test_create_lists_and_refuses_a_taken_seed() -> void:
	var w := _list()
	var a := w.create("Alpha", "epic mountains", "survival")
	assert_str(a.get("name", "")).is_equal("Alpha")
	assert_int(a["seed"]).is_equal(Settings.seed_from_string("epic mountains"))
	assert_str(w.create("Beta", "epic mountains", "creative").get("error", "")).contains("Alpha")
	assert_str(w.create("  ", "x", "survival").get("error", "")).is_not_empty()
	var b := w.create("Beta", "", "creative")
	assert_bool(b.has("error")).is_false()
	assert_int(b["seed"]).is_not_equal(a["seed"])
	var again := _list()
	assert_int(again.entries.size()).is_equal(2)
	assert_str(again.find_seed(b["seed"])["game_mode"]).is_equal("creative")


func test_newest_first() -> void:
	var w := _list()
	var a := w.create("Old", "1", "survival")
	var b := w.create("New", "2", "survival")
	a["last_played"] = 100
	b["last_played"] = 200
	assert_str(w.sorted()[0]["name"]).is_equal("New")


func test_old_saves_are_adopted_and_delete_removes_every_file() -> void:
	_touch("world_4242_g7.sqlite")
	_touch("blocks_4242_g7.json")
	_touch("player_4242_g7.json")
	_touch("quests_4242_g3.json")
	_touch("world_14242_g7.sqlite")   # another seed that merely ends in 4242
	var w := _list()
	var e := w.find_seed(4242)
	assert_str(e.get("name", "")).is_equal("World 4242")
	assert_bool(w.find_seed(14242).is_empty()).is_false()
	assert_int(w.delete(e)).is_equal(4)
	assert_bool(FileAccess.file_exists(DIR + "/world_14242_g7.sqlite")).is_true()
	assert_bool(_list().find_seed(4242).is_empty()).is_true()


func test_saved_gen_is_the_newest_version_on_disk() -> void:
	var w := _list()
	var e := w.create("Delta", "5151", "survival")
	assert_int(w.saved_gen(e)).is_equal(-1)   # a new world has no files yet: no warning
	_touch("world_5151_g7.sqlite")
	_touch("player_5151_g9.json.tmp")
	_touch("world_15151_g30.sqlite")   # another seed that merely ends in 5151
	assert_int(w.saved_gen(e)).is_equal(9)


func test_select_points_settings_at_the_world() -> void:
	var keep := [Settings.world_seed, Settings.world_seed_text, Settings.world_name, Settings.game_mode, Settings.persist_world]
	var w := _list()
	var e := w.create("Gamma", "gamma seed", "creative")
	WorldList.select(e)
	assert_int(Settings.world_seed).is_equal(int(e["seed"]))
	assert_str(Settings.world_name).is_equal("Gamma")
	assert_str(Settings.game_mode).is_equal("creative")
	Settings.world_seed = keep[0]
	Settings.world_seed_text = keep[1]
	Settings.world_name = keep[2]
	Settings.game_mode = keep[3]
	Settings.persist_world = keep[4]


func test_presets_set_their_values_and_custom_is_detected() -> void:
	for p in Settings.GRAPHICS_PRESETS:
		Settings.apply_graphics_preset(p)
		assert_str(Settings.matching_preset()).is_equal(p)
		assert_bool(Settings.SHADOW_QUALITY.has(Settings.shadow_quality)).is_true()
	Settings.apply_graphics_preset("high")
	assert_str(Settings.shadow_quality).is_equal("high")
	assert_float(Settings.view_distance_m).is_equal(72.0)
	Settings.ssao = false
	assert_str(Settings.matching_preset()).is_equal("custom")
	Settings.apply_graphics_preset("low")
	assert_bool(Settings.ssao).is_false()
	assert_float(Settings.view_distance_m).is_equal(24.0)


func test_settings_screen_shows_the_settings() -> void:
	Settings.apply_graphics_preset("medium")
	Settings.fov = 90.0
	var panel: SettingsPanel = auto_free(SettingsPanel.new())
	add_child(panel)
	assert_int(panel._preset.selected).is_equal(1)
	assert_int(panel._shadows.selected).is_equal(2)
	assert_float(panel._fov.value).is_equal(90.0)
	Settings.ssao = false
	panel.sync_from_settings()
	assert_bool(panel._ssao.button_pressed).is_false()


func test_title_skips_for_autopilot_runs() -> void:
	# this run has no autopilot=1 / play=1, so the menu shows
	assert_bool(TitleScreen.skip_menu()).is_false()
	assert_str(ProjectSettings.get_setting("application/run/main_scene")).is_equal("res://scenes/title.tscn")
