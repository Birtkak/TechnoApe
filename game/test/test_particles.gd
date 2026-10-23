extends GdUnitTestSuite

## Particles (ADR 0026): the data in data/particles.json, the sprite sheet
## built from it, the chip tiles per material, and the native ambient sampler.

var fx: ParticleFx


func before() -> void:
	fx = auto_free(ParticleFx.load_default())


func test_data_loads() -> void:
	assert_array(fx.errors).is_empty()
	assert_int(fx.effects.size()).is_greater(20)
	for name in ["block_break", "block_hit", "landing", "sprint", "splash", "bubbles", "poof", "heart", "damage",
			"smoke", "flame", "lava_pop", "drip_water", "spores", "falling_leaf", "graze",
			"waterfall_spray", "waterfall_burst", "waterfall_mist", "waterfall_foam"]:
		assert_bool(fx.effects.has(name)).override_failure_message("effect %s missing" % name).is_true()


func test_errors_are_reported() -> void:
	var bad: ParticleFx = auto_free(ParticleFx.new())
	bad.load_dict({
		"sprites": {"dot": {"frames": [["....", "...."]]}},
		"motions": {"m": {"floor": "bounce"}},
		"effects": {"x": {"channel": "sprites", "sprite": "nope", "motion": "m", "count": [1, 1], "life": [1, 1], "size": [1, 1], "speed": [0, 0]}},
		"ambient": {"unobtainium": [{"effect": "x", "chance": 1.0}]},
	})
	var text := "\n".join(bad.errors)
	assert_str(text).contains("want 8")
	assert_str(text).contains("floor must be")
	assert_str(text).contains("unknown sprite")
	assert_str(text).contains("unknown material unobtainium")


func test_sprite_codes_name_rows_and_frames() -> void:
	# code = row * 8 + frames - 1: the draw shader plays smoke's 8 frames, a heart has 1
	var smoke: Dictionary = fx.sprite_rows["smoke"]
	assert_int(smoke["frames"]).is_equal(8)
	assert_float(fx.effects["poof"]["code"]).is_equal(float(smoke["row"] * 8 + 7))
	assert_float(fx.effects["heart"]["code"]).is_equal(float(fx.sprite_rows["heart"]["row"] * 8))


func test_effects_in_voxels() -> void:
	var e: Dictionary = fx.effects["block_break"]
	assert_float(e["size_v"][1]).is_equal_approx(Settings.m2v(0.11), 1e-5)
	assert_float(e["speed_v"][0]).is_equal_approx(Settings.m2v(0.6), 1e-5)


func test_chip_tiles() -> void:
	# grass breaks into dirt like Minecraft's, logs into bark, a lit furnace into its top
	var defs := GameMaterials.defs()
	assert_int(fx._tile[GameMaterials.GRASS]).is_equal(int(defs[GameMaterials.GRASS - 1]["tiles"][2]))
	assert_int(fx._tile[GameMaterials.WOOD]).is_equal(int(defs[GameMaterials.WOOD - 1]["tiles"][1]))
	assert_int(fx._tile[GameMaterials.FURNACE_LIT]).is_equal(int(defs[GameMaterials.FURNACE_LIT - 1]["tiles"][0]))
	assert_int(fx._solid[GameMaterials.STONE]).is_equal(1)
	assert_int(fx._solid[GameMaterials.TALL_GRASS]).is_equal(0)


func test_ambient_mask() -> void:
	assert_int(fx._mask[GameMaterials.LAVA]).is_equal(1)
	assert_int(fx._mask[GameMaterials.FURNACE_LIT]).is_equal(1)
	assert_int(fx._mask[GameMaterials.DIRT]).is_equal(0)
	assert_bool(fx.ambient.has(-1)).is_true()   # burning cells


func test_sample_cells_finds_only_masked_cells() -> void:
	var sim := SimWorld.new()
	sim.configure(Vector3i(2, 2, 2), 2, 7)
	sim.set_materials(GameMaterials.defs())
	sim.mark_all_loaded(true)
	sim.fill_box(Vector3i(0, 0, 0), Vector3i(32, 8, 32), GameMaterials.STONE)
	sim.fill_box(Vector3i(8, 8, 8), Vector3i(24, 9, 24), GameMaterials.LAVA)
	var mask := PackedByteArray()
	mask.resize(256)
	mask[GameMaterials.LAVA] = 1
	var r := sim.sample_cells(Vector3i(16, 8, 16), 16, 20000, 3, mask)
	assert_int(r.size() % 7).is_equal(0)
	assert_int(r.size()).is_greater(7 * 50)
	for i in range(0, r.size(), 7):
		assert_int(r[i + 3]).is_equal(GameMaterials.LAVA)
		assert_int(r[i + 1]).is_equal(8)
		assert_int(r[i + 4]).is_equal(GameMaterials.AIR)     # above
		assert_int(r[i + 5]).is_equal(GameMaterials.STONE)   # below
	# a short mask reads nothing rather than past its end
	assert_int(sim.sample_cells(Vector3i(16, 8, 16), 16, 100, 3, PackedByteArray()).size()).is_equal(0)


func test_burst_without_gpu_pools_is_harmless() -> void:
	# headless tests never call setup(): bursts are no-ops, not errors
	assert_int(fx.burst("block_break", Vector3.ZERO, {"material": GameMaterials.STONE})).is_equal(0)
	assert_int(fx.burst("no_such_effect", Vector3.ZERO)).is_equal(0)
