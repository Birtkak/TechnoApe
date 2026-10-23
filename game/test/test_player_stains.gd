extends GdUnitTestSuite

## Noita stains on the player (PlayerStains): coats, washing, fire and damage.


func test_materials_map_to_stains() -> void:
	assert_str(PlayerStains.stain_of(GameMaterials.WATER)).is_equal("wet")
	assert_str(PlayerStains.stain_of(GameMaterials.WATER_STATIC)).is_equal("wet")
	assert_str(PlayerStains.stain_of(GameMaterials.BLOOD)).is_equal("bloody")
	assert_str(PlayerStains.stain_of(GameMaterials.OIL)).is_equal("oily")
	assert_str(PlayerStains.stain_of(GameMaterials.TOXIC_SLUDGE_STATIC)).is_equal("toxic")
	assert_str(PlayerStains.stain_of(GameMaterials.LAVA)).is_equal("burning")
	assert_str(PlayerStains.stain_of(GameMaterials.STONE)).is_equal("")


func test_one_stain_at_a_time() -> void:
	var s := PlayerStains.new()
	s.update(0.1, [GameMaterials.BLOOD])
	assert_array(s.active()).contains_exactly(["bloody"])
	s.update(0.1, [GameMaterials.OIL])
	assert_array(s.active()).contains_exactly(["oily"])   # oil washed the blood off
	s.update(0.1, [GameMaterials.TOXIC_SLUDGE])
	assert_array(s.active()).contains_exactly(["toxic"])


func test_water_washes_the_rest_off_and_keeps_fire_away() -> void:
	var s := PlayerStains.new()
	s.update(0.1, [GameMaterials.TOXIC_SLUDGE])
	s.update(0.1, [GameMaterials.WATER])
	assert_array(s.active()).contains_exactly(["wet"])
	assert_int(s.update(0.1, [GameMaterials.FIRE])).is_equal(0)   # wet: fire does not catch
	assert_bool(s.has("burning")).is_false()
	s.apply("burning")
	assert_bool(s.has("burning")).is_false()


func test_lava_burns_even_when_wet() -> void:
	var s := PlayerStains.new()
	s.apply("wet")
	assert_int(s.update(0.1, [GameMaterials.LAVA])).is_equal(1)   # catching fire hurts at once
	assert_bool(s.has("burning")).is_true()
	assert_bool(s.has("wet")).is_false()


func test_burning_hurts_each_second_and_oil_makes_it_last() -> void:
	var plain := PlayerStains.new()
	var oily := PlayerStains.new()
	oily.apply("oily")
	var dmg_plain := plain.update(0.05, [GameMaterials.FIRE])
	var dmg_oily := oily.update(0.05, [GameMaterials.FIRE])
	for i in 200:   # 10 s more
		dmg_plain += plain.update(0.05, [])
		dmg_oily += oily.update(0.05, [])
	assert_bool(plain.has("burning")).is_false()
	assert_bool(oily.has("oily")).is_false()   # the fire replaced the oil
	assert_int(dmg_plain).is_between(4, 5)    # 4 s of fire
	assert_int(dmg_oily).is_between(8, 9)     # twice as long
	water_puts_it_out(oily)


func water_puts_it_out(s: PlayerStains) -> void:
	s.apply("burning")
	s.update(0.05, [GameMaterials.WATER])
	assert_bool(s.has("burning")).is_false()


func test_toxic_hurts_every_two_seconds_then_fades() -> void:
	var s := PlayerStains.new()
	var dmg := 0
	dmg += s.update(0.5, [GameMaterials.TOXIC_SLUDGE_STATIC])
	assert_int(dmg).is_equal(0)   # the first half-heart comes after 2 s
	for i in 39:   # 20 s in total
		dmg += s.update(0.5, [])
	assert_bool(s.has("toxic")).is_false()
	assert_int(dmg).is_equal(5)   # every 2 s for the 10 s the coat lasts


func test_tint_mixes_colours() -> void:
	var s := PlayerStains.new()
	assert_float(s.tint().a).is_equal(0.0)
	s.apply("bloody")
	var c := s.tint()
	assert_float(c.a).is_greater(0.3)
	assert_float(c.r).is_greater(c.g)
	assert_dict(s.summary()).contains_keys(["bloody"])
