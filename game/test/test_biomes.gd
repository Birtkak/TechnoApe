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
	assert_str(_lowland(-0.4, 0.3)).is_equal("taiga")
	assert_str(_lowland(0.4, -0.3)).is_equal("savanna")


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
	assert_str(_lowland(0.5, 0.0, 300)).is_equal("alpine")


func test_coast_and_ocean() -> void:
	assert_str(_lowland(0.0, 0.0, 2)).is_equal("beach")
	assert_str(_lowland(-0.9, 0.0, 2)).is_equal("snowy_beach")
	assert_str(_lowland(0.0, 0.0, 2, {"slope": 1.0})).is_equal("stony_shore")
	assert_str(_lowland(0.0, 0.0, -10)).is_equal("ocean")
	assert_str(_lowland(-0.9, 0.0, -10)).is_equal("frozen_ocean")
	assert_str(_lowland(0.0, 0.0, -60, {"continentalness": -0.8})).is_equal("deep_ocean")


func test_water_overrides() -> void:
	assert_str(_lowland(0.0, 0.0, 30, {"river": 0.9})).is_equal("river")
	assert_str(_lowland(0.0, 0.0, 30, {"lake": true})).is_equal("lake")
	assert_str(_lowland(0.0, 0.0, 30, {"river": 0.2})).is_equal("plains")


# ---------- adjacency ----------

func test_declared_borders_match_derived() -> void:
	# The borders listed in biomes.json (for humans) must agree with the derived graph:
	# every listed border is possible, and every common border is listed.
	for name in reg.names():
		var declared: Array = reg.get_def(name)["borders"]
		var derived := rules.neighbors(name)
		for b in declared:
			assert_bool(rules.can_border(name, b)).override_failure_message("%s lists %s as a border but they cannot touch" % [name, b]).is_true()
		for c in derived["common"]:
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
	assert_array(rules.neighbors("snowy_peaks")["common"]).is_equal(["alpine"])
	assert_bool(rules.can_border("river", "mountains")).is_true()
	assert_bool(rules.can_border("lake", "mountains")).is_true()   # basin lakes in mountain valleys
	assert_bool(rules.can_border("lake", "alpine")).is_false()
	assert_bool(rules.can_border("river", "snowy_peaks")).is_false()
	assert_bool(rules.can_border("beach", "ocean")).is_true()
	assert_bool(rules.can_border("beach", "highlands")).is_false()


func test_colder_neighbours_going_north() -> void:
	assert_array(rules.colder_than("plains")).contains(["tundra", "taiga"])
	assert_array(rules.colder_than("plains")).not_contains(["desert", "savanna", "jungle"])
	assert_array(rules.colder_than("ice_fields")).is_empty()


# ---------- generated header ----------

func test_generated_header_is_up_to_date() -> void:
	var path := ProjectSettings.globalize_path("res://").path_join("..").path_join(BiomeCodegen.HEADER_PATH)
	var f := FileAccess.open(path, FileAccess.READ)
	assert_object(f).override_failure_message("missing %s; run tools/biome_codegen.gd" % path).is_not_null()
	if f == null:
		return
	assert_str(f.get_as_text().replace("\r\n", "\n")).override_failure_message("header stale; run tools/biome_codegen.gd").is_equal(BiomeCodegen.generate(reg))
