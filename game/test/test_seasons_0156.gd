extends GdUnitTestSuite

## ADR 0156: seasons that matter: SimWorld.season_pass lays snow on open ground, freezes still water to lake ice and
## thaws both back; the mode from the temperatures; crops' growing windows; drought dries fire's spread.


func _world() -> SimWorld:
	var w := SimWorld.new()
	w.configure(Vector3i(2, 2, 2), 2, 5)
	w.set_materials(GameMaterials.defs())
	w.mark_all_loaded(true)
	return w


func _ids() -> PackedInt32Array:
	return PackedInt32Array([GameMaterials.SNOW_LAYER, GameMaterials.LAKE_ICE, GameMaterials.WATER_STATIC])


func test_winter_snows_freezes_and_thaws() -> void:
	var w := _world()
	w.fill_box(Vector3i(0, 0, 0), Vector3i(32, 4, 32), GameMaterials.DIRT)            # ground under the whole world
	w.fill_box(Vector3i(0, 4, 0), Vector3i(8, 6, 8), GameMaterials.WATER_STATIC)      # a pond in one corner
	w.set_voxel(Vector3i(20, 4, 20), GameMaterials.TALL_GRASS)                          # a plant: no snow on it
	for i in 4:
		w.tick(1)   # the fills settle and the chunks sleep
	w.clear_dirty()
	var mask := SeasonWorld.lies_on_mask(GameMaterials.defs())
	assert_int(mask[GameMaterials.DIRT]).is_equal(1)
	assert_int(mask[GameMaterials.TALL_GRASS]).is_equal(0)
	assert_int(mask[GameMaterials.WATER_STATIC]).is_equal(0)
	var winter := 3 | (255 << 8) | 0x10000
	var n := 0
	for cz in 2:
		for cx in 2:
			n += w.season_pass(Vector2i(cx, cz), winter, _ids(), mask, 1)
	assert_int(n).is_equal(32 * 32 - 1)   # every column but the grass plant: 64 ice, the rest snow
	assert_int(w.get_voxel(Vector3i(3, 5, 3))).is_equal(GameMaterials.LAKE_ICE)
	assert_int(w.get_voxel(Vector3i(3, 4, 3))).is_equal(GameMaterials.WATER_STATIC)   # only the top freezes
	assert_int(w.get_voxel(Vector3i(15, 4, 15))).is_equal(GameMaterials.SNOW_LAYER)
	assert_int(w.get_voxel(Vector3i(20, 5, 20))).is_equal(GameMaterials.AIR)
	assert_bool(w.get_active_chunks().is_empty()).is_true()   # nothing woken ...
	assert_bool(w.get_dirty_boxes().is_empty()).is_false()    # ... but the edits reach the terrain
	# a second winter pass changes nothing (one layer, no ice on ice)
	assert_int(w.season_pass(Vector2i(0, 0), winter, _ids(), mask, 2)).is_equal(0)
	# the thaw puts everything back
	n = 0
	for cz in 2:
		for cx in 2:
			n += w.season_pass(Vector2i(cx, cz), 4 | 0x10000, _ids(), mask, 3)
	assert_int(n).is_equal(32 * 32 - 1)
	assert_int(w.get_voxel(Vector3i(3, 5, 3))).is_equal(GameMaterials.WATER_STATIC)
	assert_int(w.get_voxel(Vector3i(15, 4, 15))).is_equal(GameMaterials.AIR)
	assert_int(w.count_material(GameMaterials.SNOW_LAYER) + w.count_material(GameMaterials.LAKE_ICE)).is_equal(0)


func test_mode_follows_temperatures() -> void:
	assert_int(SeasonWorld.flags_for(10.0, 10.0, true)).is_equal(4)        # warm: thaw
	assert_int(SeasonWorld.flags_for(0.0, -1.0, false)).is_equal(0)        # cool but not freezing, no snow
	assert_int(SeasonWorld.flags_for(-5.0, -5.0, false)).is_equal(2)       # freezing: ice only
	assert_int(SeasonWorld.flags_for(-5.0, -5.0, true) & 3).is_equal(3)    # and snow while it snows
	assert_int(SeasonWorld.flags_for(0.5, 3.0, true) & 1).is_equal(0)      # rain, not snow, above 0 C


func test_crop_windows() -> void:
	var beans := {"grows_in": ["spring", "summer"]}
	assert_float(Seasons.growth_share(beans, 1, 1.0)).is_equal(1.0)
	assert_float(Seasons.growth_share(beans, 3, 0.5)).is_equal(0.0)        # autumn: past its window
	assert_float(Seasons.growth_share({}, 3, 0.5)).is_equal(0.5)           # no window: the calendar's share
	assert_float(Seasons.growth_share({"grows_in": ["autumn"]}, 3, 0.5)).is_equal(0.5)
	assert_str(Seasons.window_note("bean_plants", beans, 3)).contains("will not grow in autumn")
	assert_str(Seasons.window_note("bean_plants", beans, 2)).is_empty()


func test_short_year_and_bigger_swing() -> void:
	var c := Calendar.with_month_days(Settings.month_days)
	assert_int(c.month()).is_equal(3)   # still begins on 1 April
	assert_int(c.year_days()).is_equal(12 * Settings.month_days)
	var jan := Survival.environment_c(0.0, int(0.5 * c.month_days), c.year_days(), c.month_days, 0.33, 0.0, false, false, 0.0)
	assert_float(jan).is_less(Settings.ice_form_c)   # a temperate winter freezes the ponds
