extends GdUnitTestSuite

## The F6 tuning panel: every Settings var gets a row, a row edits the setting live,
## Save keeps only what differs from the defaults, load_tune reads it back, Reset undoes it.


func test_rows_edit_save_reload_and_reset() -> void:
	var panel: TunePanel = auto_free(TunePanel.new())
	add_child(panel)
	assert_bool(panel._rows.has("sim_tick_hz")).is_true()
	assert_bool(panel._rows.has("sim_window_chunks")).is_true()
	assert_int(panel._rows.size()).is_greater(100)
	var old_hz: float = Settings.sim_tick_hz
	var old_win: Vector3i = Settings.sim_window_chunks
	var spin: SpinBox = panel._rows["sim_tick_hz"].get_child(1)
	spin.value = old_hz + 5
	assert_float(Settings.sim_tick_hz).is_equal(old_hz + 5)
	var edit: LineEdit = panel._rows["sim_window_chunks"].get_child(1)
	edit.text_submitted.emit("8,16,8")
	assert_that(Settings.sim_window_chunks).is_equal(Vector3i(8, 16, 8))
	edit.text_submitted.emit("garbage")
	assert_that(Settings.sim_window_chunks).is_equal(Vector3i(8, 16, 8))
	panel._save()
	var cf := ConfigFile.new()
	assert_int(cf.load(TunePanel.TUNE_FILE)).is_equal(OK)
	assert_that(cf.get_value("tune", "sim_tick_hz")).is_equal(old_hz + 5)
	assert_bool(cf.has_section_key("tune", "walk_speed_m")).is_false()
	Settings.sim_tick_hz = old_hz
	TunePanel.load_tune()
	assert_float(Settings.sim_tick_hz).is_equal(old_hz + 5)
	panel._reset()
	assert_float(Settings.sim_tick_hz).is_equal(old_hz)
	assert_that(Settings.sim_window_chunks).is_equal(old_win)
	assert_bool(FileAccess.file_exists(TunePanel.TUNE_FILE)).is_false()
