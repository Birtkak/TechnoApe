extends GdUnitTestSuite

## Lighting grades (ADR 0041): golden is the default, every grade key is a real Atmosphere value,
## and switching grades sets and resets them.


func test_every_grade_key_is_an_atmosphere_value() -> void:
	var a := Atmosphere.new()
	for g: String in Atmosphere.GRADES:
		for k: String in Atmosphere.GRADES[g]:
			assert_bool(k in a).override_failure_message("%s.%s is not an Atmosphere value" % [g, k]).is_true()
	assert_str(a.grade).is_equal("golden")
	a.free()


func test_grades_switch_and_reset_to_natural() -> void:
	var a := Atmosphere.new()
	assert_bool(a._load_grade("golden")).is_true()
	assert_str(a.tonemap).is_equal("aces")
	assert_float(a.fog_per_km).is_equal(2.8)
	assert_bool(a._load_grade("clear")).is_true()
	assert_float(a.fog_per_km).is_equal(1.1)
	assert_bool(a._load_grade("natural")).is_true()   # keys a grade leaves out go back to natural's
	assert_str(a.tonemap).is_equal("agx")
	assert_bool(a.curve.is_empty()).is_true()
	assert_bool(a._load_grade("sepia")).is_false()
	assert_str(a.grade).is_equal("natural")
	a.free()


func test_the_curve_is_a_per_channel_lookup() -> void:
	assert_object(Atmosphere._curve_texture([])).is_null()
	var t := Atmosphere._curve_texture(Atmosphere.GRADES["golden"]["curve"])
	assert_int(t.width).is_equal(256)
	var c := t.gradient.sample(1.0)
	assert_float(c.b).is_less(c.r)   # warm highlights
