extends GdUnitTestSuite

## Headless tests for the biome data and placement rules (data/biomes.json,
## scripts/world/biome_*.gd). Run:
##   godot_console.exe --headless --path game -s res://addons/gdUnit4/bin/GdUnitCmdTool.gd --add res://test --ignoreHeadlessMode

var reg: BiomeRegistry
var rules: BiomeRules


func before_test() -> void:
	reg = BiomeRegistry.load_default()
	rules = BiomeRules.new(reg)


func _lowland(t: float, h: float, above: int = 30, extra: Dictionary = {}) -> String:
	var col := {"temperature": t, "humidity": h, "above_sea": above, "continentalness": 0.2, "erosion": 0.0, "slope": 0.0}
	col.merge(extra, true)
	return rules.classify_name(col)


# ---------- data ----------

func test_data_file_is_valid() -> void:
	assert_array(reg.errors).is_empty()
	assert_int(reg.count()).is_greater_equal(25)


func test_original_ids_are_stable() -> void:
	# The native WorldGen::Biome enum (0..12) must keep its meaning.
	var original := ["deep_ocean", "ocean", "beach", "plains", "forest", "desert", "tundra", "highlands", "mountains", "snowy_peaks", "river", "lake", "alpine"]
	for i in original.size():
		assert_str(reg.name_of(i)).is_equal(original[i])
	if ClassDB.class_exists("WorldGen"):
		for i in original.size():
			assert_str(WorldGen.biome_name(i)).is_equal(reg.name_of(i))


func test_every_land_biome_is_placeable() -> void:
	for name in reg.names():
		if reg.get_def(name)["band"] == "water":
			continue
		assert_array(rules.cells_of(name)).override_failure_message("%s has no climate cell" % name).is_not_empty()


func test_validation_catches_bad_data() -> void:
	var r := BiomeRegistry.new()
	r.load_json('{"climate": {}, "tables": {}, "biomes": [{"id": 1, "name": "x"}]}')
	assert_array(r.errors).is_not_empty()


# ---------- climate: latitude ----------

func test_north_is_colder() -> void:
	var prev := 2.0
	for z in range(60000, -60001, -4000):  # south -> north
		var t := rules.temperature(0.0, float(z))
		assert_float(t).override_failure_message("temperature rose going north at z=%d" % z).is_less_equal(prev)
		prev = t
	assert_int(rules.temp_band(rules.temperature(0.0, 0.0))).is_equal(2)        # spawn: temperate
	assert_int(rules.temp_band(rules.temperature(0.0, -40000.0))).is_equal(0)   # 20 km north: frozen
	assert_int(rules.temp_band(rules.temperature(0.0, 40000.0))).is_equal(4)    # 20 km south: hot


func test_noise_cannot_undo_far_latitude() -> void:
	# Even the warmest noise 16 km north stays cold; coldest noise 16 km south stays warm.
	assert_int(rules.temp_band(rules.temperature(1.0, -32000.0))).is_less_equal(1)
	assert_int(rules.temp_band(rules.temperature(-1.0, 32000.0))).is_greater_equal(3)


func test_every_band_reachable_near_spawn() -> void:
	# Within ±8 km of spawn all five temperature bands occur somewhere.
	var seen := {}
	for z in range(-16000, 16001, 1000):
		for n in [-1.0, -0.5, 0.0, 0.5, 1.0]:
			seen[rules.temp_band(rules.temperature(n, float(z)))] = true
	assert_int(seen.size()).is_equal(5)


func test_altitude_lapse_cools_highlands() -> void:
	assert_float(rules.temperature(0.0, 0.0, 170)).is_less(rules.temperature(0.0, 0.0, 70))
	assert_float(rules.temperature(0.0, 0.0, 70)).is_equal(rules.temperature(0.0, 0.0, 0))


# ---------- classification ----------

func test_lowland_table_corners() -> void:
	assert_str(_lowland(0.9, -0.9)).is_equal("desert")
	assert_str(_lowland(0.9, 0.9)).is_equal("jungle")
	assert_str(_lowland(-0.9, -0.9)).is_equal("ice_fields")
	assert_str(_lowland(-0.9, 0.9)).is_equal("snowy_taiga")
	assert_str(_lowland(0.0, 0.0)).is_equal("plains")
	assert_str(_lowland(0.0, 0.3)).is_equal("forest")
	assert_str(_lowland(-0.4, 0.0)).is_equal("taiga")
	assert_str(_lowland(-0.4, 0.3)).is_equal("birch_forest")
	assert_str(_lowland(0.4, -0.3)).is_equal("savanna")
	assert_str(_lowland(0.4, 0.0)).is_equal("flower_meadow")


func test_swamp_needs_low_flat_wet_ground() -> void:
	assert_str(_lowland(0.0, 0.8, 10, {"erosion": 0.3})).is_equal("swamp")
	assert_str(_lowland(0.0, 0.8, 40, {"erosion": 0.3})).is_equal("old_growth_forest")
	assert_str(_lowland(0.0, 0.8, 10, {"erosion": -0.5})).is_equal("old_growth_forest")
	assert_str(_lowland(0.9, 0.8, 10, {"erosion": 0.3})).is_equal("jungle")


func test_altitude_bands() -> void:
	assert_str(_lowland(0.0, 0.0, 100)).is_equal("highlands")
	assert_str(_lowland(-0.5, 0.0, 100)).is_equal("highlands")       # cold highlands stay green
	assert_str(_lowland(-0.9, 0.0, 100)).is_equal("frozen_highlands")
	assert_str(_lowland(0.9, -0.9, 100)).is_equal("badlands")
	assert_str(_lowland(0.0, 0.0, 200)).is_equal("mountains")
	assert_str(_lowland(0.0, 0.0, 250)).is_equal("alpine")
	assert_str(_lowland(0.0, 0.0, 400)).is_equal("snowy_peaks")
	# Cold lowers the snow line: 300 above sea is snow when frozen, alpine when warm.
	assert_str(_lowland(-1.0, 0.0, 300)).is_equal("snowy_peaks")
	assert_str(_lowland(-1.0, 0.0, 175)).is_equal("mountains")   # coldest tree line stays above highland_max
	assert_str(_lowland(0.5, 0.6, 300)).is_equal("alpine")


func test_coast_and_ocean() -> void:
	assert_str(_lowland(0.0, 0.0, 2)).is_equal("beach")
	assert_str(_lowland(-0.9, 0.0, 2)).is_equal("snowy_beach")
	assert_str(_lowland(0.0, 0.0, 2, {"slope": 1.0})).is_equal("stony_shore")
	assert_str(_lowland(0.0, 0.0, -10)).is_equal("ocean")
	assert_str(_lowland(-0.9, 0.0, -10)).is_equal("frozen_ocean")
	assert_str(_lowland(0.0, 0.0, -60, {"continentalness": -0.8})).is_equal("deep_ocean")
	# a beach only where the sea is near: low ground inland is plain land
	assert_str(_lowland(0.0, 0.0, 2, {"shore": false})).is_equal("plains")
	assert_str(_lowland(0.0, 0.0, 6)).is_equal("beach")
	assert_str(_lowland(0.0, 0.0, 7)).is_equal("plains")


func test_band_overrides() -> void:
	assert_str(_lowland(0.4, 0.6, 2)).is_equal("mangrove")                     # warm wet coast
	assert_str(_lowland(0.4, 0.6, 2, {"slope": 1.0})).is_equal("stony_shore")  # steep shores stay rock
	assert_str(_lowland(0.4, -0.3, 2)).is_equal("beach")
	assert_str(_lowland(0.8, -0.3, 200)).is_equal("volcanic")                  # hot dry mountain
	assert_str(_lowland(0.4, 0.0, 250)).is_equal("volcanic")                   # warm, above the tree line
	assert_str(_lowland(0.4, 0.6, 200)).is_equal("mountains")                  # wet mountains stay green
	assert_str(_lowland(0.8, -0.3, 500)).is_equal("snowy_peaks")               # snow still caps them
	assert_str(_lowland(0.8, -0.3, 100)).is_equal("badlands")


func test_water_overrides() -> void:
	assert_str(_lowland(0.0, 0.0, 30, {"river": 0.9})).is_equal("river")
	assert_str(_lowland(0.0, 0.0, 30, {"lake": true})).is_equal("lake")
	assert_str(_lowland(0.0, 0.0, 30, {"river": 0.2})).is_equal("plains")


# ---------- adjacency ----------

func test_declared_borders_match_derived() -> void:
	# The borders listed in biomes.json (for humans) must agree with the derived graph
	# within the same layer (surface or cave): every listed border is possible, and
	# every common border is listed. Vertical links are derived only (see below()).
	for name in reg.names():
		var declared: Array = reg.get_def(name)["borders"]
		var derived := rules.neighbors(name)
		var cave: bool = reg.get_def(name)["band"] == "cave"
		for b in declared:
			assert_bool(rules.can_border(name, b)).override_failure_message("%s lists %s as a border but they cannot touch" % [name, b]).is_true()
			assert_bool((reg.get_def(b)["band"] == "cave") == cave).override_failure_message("%s lists %s: borders are per layer" % [name, b]).is_true()
		for c in derived["common"]:
			if (reg.get_def(c)["band"] == "cave") != cave:
				continue
			assert_bool(declared.has(c)).override_failure_message("%s commonly borders %s but does not list it" % [name, c]).is_true()


func test_adjacency_is_symmetric() -> void:
	for a in reg.names():
		for b in reg.names():
			if a != b:
				assert_bool(rules.can_border(a, b)).override_failure_message("%s/%s asymmetric" % [a, b]).is_equal(rules.can_border(b, a))


func test_hot_and_frozen_never_touch() -> void:
	for hot in reg.with_tag("hot"):
		for frozen in reg.with_tag("frozen"):
			assert_bool(rules.can_border(hot, frozen)).override_failure_message("%s can border %s" % [hot, frozen]).is_false()


func test_specific_borders() -> void:
	assert_bool(rules.can_border("desert", "savanna")).is_true()
	assert_array(rules.neighbors("desert")["common"]).not_contains(["plains"])  # savanna sits between; only a rare corner touch
	assert_bool(rules.can_border("desert", "tundra")).is_false()
	assert_bool(rules.can_border("snowy_peaks", "plains")).is_false()
	var peaks_surface: Array = rules.neighbors("snowy_peaks")["common"].filter(func(n): return reg.get_def(n)["band"] != "cave")
	assert_array(peaks_surface).is_equal(["alpine", "lake", "river", "volcanic"])  # tarns and springs, snow-capped volcanoes; caves run beneath
	assert_bool(rules.can_border("river", "mountains")).is_true()
	assert_bool(rules.can_border("lake", "mountains")).is_true()   # basin lakes in mountain valleys
	assert_bool(rules.can_border("lake", "ocean")).is_false()
	assert_bool(rules.can_border("river", "deep_ocean")).is_false()
	assert_bool(rules.can_border("alpine", "highlands")).is_false()   # mountains always sit between
	assert_bool(rules.can_border("beach", "ocean")).is_true()
	assert_bool(rules.can_border("beach", "highlands")).is_false()


func test_colder_neighbours_going_north() -> void:
	assert_array(rules.colder_than("plains")).contains(["tundra", "taiga"])
	assert_array(rules.colder_than("plains")).not_contains(["desert", "savanna", "jungle"])
	assert_array(rules.colder_than("ice_fields")).is_empty()


# ---------- 3D layer ----------

func _col(t: float, h: float, above: int) -> Dictionary:
	return {"temperature": t, "humidity": h, "above_sea": above, "continentalness": 0.2, "erosion": 0.0, "slope": 0.0}


func test_straight_down_from_a_mountain_foot() -> void:
	# Mountain on top, humid ground at its foot, lush cave straight below.
	var mountain := _col(0.0, 0.3, 200)
	var foot := _col(0.0, 0.3, 30)
	assert_str(rules.classify_name(mountain)).is_equal("mountains")
	assert_str(rules.classify_name(foot)).is_equal("forest")
	assert_str(rules.classify_3d_name(foot, 2)).is_equal("forest")        # soil keeps the surface biome
	assert_str(rules.classify_3d_name(foot, 30)).is_equal("lush_cave")
	assert_str(rules.classify_3d_name(foot, 100)).is_equal("lush_cave")
	assert_str(rules.classify_3d_name(foot, 200)).is_equal("deep_cave")
	assert_str(rules.classify_3d_name(mountain, 30)).is_equal("lush_cave")  # the mountain shares the climate


func test_cave_table_follows_surface_climate() -> void:
	assert_str(rules.classify_3d_name(_col(0.9, -0.9, 30), 30)).is_equal("dripstone_cave")   # under desert
	assert_str(rules.classify_3d_name(_col(0.0, 0.0, 30), 30)).is_equal("cave")              # under plains
	assert_str(rules.classify_3d_name(_col(0.0, 0.8, 30), 100)).is_equal("fungal_cave")      # wettest, mid depth
	assert_str(rules.classify_3d_name(_col(-0.9, 0.0, 30), 30)).is_equal("frozen_cave")      # frozen surface, shallow
	assert_str(rules.classify_3d_name(_col(-0.9, 0.0, 30), 100)).is_equal("cave")            # frozen only shallow


func test_deep_layer_variants() -> void:
	var c := _col(0.0, 0.0, 30)
	assert_str(rules.classify_3d_name(c, 300, 0.0)).is_equal("deep_cave")
	assert_str(rules.classify_3d_name(c, 300, -0.9)).is_equal("deep_dark")
	assert_str(rules.classify_3d_name(c, 300, 0.9)).is_equal("crystal_cave")
	assert_str(rules.classify_3d_name(_col(0.9, 0.0, 30), 300, 0.0)).is_equal("magma_cave")
	assert_str(rules.classify_3d_name(_col(0.9, 0.0, 30), 300, -0.9)).is_equal("magma_cave") # temperature override first
	assert_str(rules.classify_3d_name(c, 100, -0.9)).is_equal("cave")                        # deep dark only deep
	assert_str(rules.classify_3d_name(c, 100, 0.9)).is_equal("sulfur_cave")                 # rare mid-depth pockets


func test_vertical_adjacency() -> void:
	assert_bool(rules.can_border("plains", "cave")).is_true()
	assert_bool(rules.can_border("forest", "lush_cave")).is_true()
	assert_bool(rules.can_border("plains", "deep_cave")).is_false()
	assert_bool(rules.can_border("plains", "deep_dark")).is_false()
	assert_bool(rules.can_border("ocean", "cave")).is_false()
	assert_bool(rules.can_border("lush_cave", "cave")).is_true()
	assert_bool(rules.can_border("lush_cave", "dripstone_cave")).is_false()   # plain cave sits between
	assert_bool(rules.can_border("deep_dark", "crystal_cave")).is_false()     # deep cave sits between
	assert_bool(rules.can_border("deep_dark", "magma_cave")).is_true()
	assert_bool(rules.can_border("river", "cave")).is_true()
	assert_bool(rules.can_border("river", "deep_cave")).is_false()
	# Forest is humid (not wet, not hot): lush caves (sulfur pockets at mid depth), then the deep layer without magma.
	assert_array(rules.below("forest")).is_equal(["crystal_cave", "deep_cave", "deep_dark", "lush_cave", "sulfur_cave"])
	assert_array(rules.below("old_growth_forest")).contains(["fungal_cave"])
	assert_array(rules.below("desert")).contains(["dripstone_cave", "magma_cave"])
	assert_array(rules.below("desert")).not_contains(["lush_cave", "frozen_cave"])


# ---------- generated header ----------

func test_generated_header_is_up_to_date() -> void:
	var path := ProjectSettings.globalize_path("res://").path_join("..").path_join(BiomeCodegen.HEADER_PATH)
	var f := FileAccess.open(path, FileAccess.READ)
	assert_object(f).override_failure_message("missing %s; run tools/biome_codegen.gd" % path).is_not_null()
	if f == null:
		return
	assert_str(f.get_as_text().replace("\r\n", "\n")).override_failure_message("header stale; run tools/biome_codegen.gd").is_equal(BiomeCodegen.generate(reg))
