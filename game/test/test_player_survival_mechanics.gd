extends GdUnitTestSuite

## Player survival rules from the 2026-10-27 bug reports: water breaks a fall, buried heads
## suffocate, the bow draws, seasonal drops (blueberries) and their look-at line.


func after_test() -> void:
	Seasons.season = -1


func test_water_breaks_a_fall() -> void:
	assert_int(Player.fall_damage(20.0, true)).is_equal(0)
	assert_int(Player.fall_damage(20.0, false)).is_equal(int(20.0 - Settings.fall_damage_safe_m))
	assert_int(Player.fall_damage(1.0, false)).is_equal(0)


func test_buried_head_suffocates_fast() -> void:
	assert_float(Player.breath_time(GameMaterials.id_of("sand"))).is_equal(Settings.suffocate_s)
	assert_float(Player.breath_time(GameMaterials.id_of("gravel"))).is_equal(Settings.suffocate_s)
	assert_float(Player.breath_time(GameMaterials.WATER)).is_equal(Settings.air_s)
	assert_float(Player.breath_time(GameMaterials.AIR)).is_equal(0.0)
	assert_float(Player.breath_time(GameMaterials.STONE)).is_equal(0.0)
	assert_float(Settings.suffocate_s).is_less(Settings.air_s)


func test_bow_power_grows_with_the_draw() -> void:
	assert_float(StoneAge.draw_power(0.0, 1.0)).is_equal(0.0)
	assert_float(StoneAge.draw_power(0.5, 1.0)).is_equal_approx(0.4167, 0.001)
	assert_float(StoneAge.draw_power(1.0, 1.0)).is_equal(1.0)
	assert_float(StoneAge.draw_power(3.0, 1.0)).is_equal(1.0)
	var bow: Dictionary = ItemRegistry.load_default().get_def("bow")
	assert_bool(bow["shoots"].has("draw_s")).is_true()


func test_blueberry_bushes_drop_in_every_season_most_when_ripe() -> void:
	var mining := MiningTable.load_default()
	var rng := RandomNumberGenerator.new()
	var yields := []
	for s in 4:
		Seasons.season = s
		var carry := {}
		var out := mining.voxel_drops({"blueberry_bush": 64 * 10}, {}, func(_id: String) -> int: return 1,
			func(_n: String) -> bool: return true, carry, rng, 64)
		var n := 0
		for d in out:
			n += int(d["count"])
		yields.append(n)
	# 0 winter, 1 spring, 2 summer, 3 autumn
	assert_int(yields[0]).is_greater(0)
	assert_int(yields[1]).is_greater(0)
	assert_int(yields[2]).is_greater(yields[1] * 3)
	assert_int(yields[3]).is_equal(yields[2])


func test_look_at_says_when_berries_are_ripe() -> void:
	var mining := MiningTable.load_default()
	assert_str(mining.season_note("blueberry_bush", 2)).is_equal("Blueberry: ripe now")
	assert_str(mining.season_note("blueberry_bush", 1)).is_equal("Blueberry: ripe in summer and autumn")
	assert_str(mining.season_note("blueberry_bush", -1)).is_equal("")
	assert_str(mining.season_note("stone", 2)).is_equal("")
