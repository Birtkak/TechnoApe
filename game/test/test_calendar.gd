extends GdUnitTestSuite

## Calendar (ADR 0025): 9-day lunar months, seasons, the real sun at 45 deg N,
## the moon's phase by the day of the month, sidereal stars, rare eclipses.


func _cal(month: int, dom: int, t: float) -> Calendar:
	var c := Calendar.new()
	c.set_date(month, dom, t)
	return c


func _alt_deg(v: Vector3) -> float:
	return rad_to_deg(asin(clampf(v.y, -1.0, 1.0)))


func test_dates_and_seasons() -> void:
	var c := Calendar.new()
	assert_str(c.date_text()).is_equal("1 April, year 1 (Spring)")
	c.advance(9.0)
	assert_str(c.date_text()).is_equal("1 May, year 1 (Spring)")
	c.set_date(0, 3)
	assert_str(c.date_text()).is_equal("4 January, year 2 (Winter)")   # forward only: next January
	assert_int(_cal(6, 0, 0.5).season()).is_equal(2)
	assert_int(_cal(9, 0, 0.5).season()).is_equal(3)
	assert_int(_cal(11, 0, 0.5).season()).is_equal(0)
	# /time set and sleeping move forward
	c.time = 0.8
	var d := c.day
	c.advance_to(0.26)
	assert_int(c.day).is_equal(d + 1)
	assert_float(c.time).is_equal_approx(0.26, 1e-6)


func test_sun_follows_the_seasons_at_45_north() -> void:
	# noon altitude: 90 - 45 + declination
	var summer := _cal(5, 6, 0.5)   # about the June solstice
	var winter := _cal(11, 6, 0.5)
	var equinox := _cal(2, 6, 0.5)
	assert_float(_alt_deg(summer.sun_dir())).is_between(67.0, 68.6)
	assert_float(_alt_deg(winter.sun_dir())).is_between(21.4, 23.0)
	assert_float(_alt_deg(equinox.sun_dir())).is_between(43.5, 46.5)
	# due south at noon (north is -z)
	assert_float(summer.sun_dir().x).is_between(-0.01, 0.01)
	assert_float(summer.sun_dir().z).is_greater(0.0)
	# long summer days, short winter days
	assert_float(summer.day_length_hours()).is_between(15.2, 16.0)
	assert_float(winter.day_length_hours()).is_between(8.4, 9.2)
	# rises in the east, sets in the west
	var morning := _cal(2, 6, equinox.sunrise() + 0.01)
	assert_float(morning.sun_dir().x).is_greater(0.9)
	var evening := _cal(2, 6, equinox.sunset() - 0.01)
	assert_float(evening.sun_dir().x).is_less(-0.9)


func test_polaris_stays_put_and_the_stars_turn() -> void:
	var c := _cal(3, 2, 0.0)
	var pole := c.celestial_to_world() * Vector3(0, 0, 1)
	assert_float(_alt_deg(pole)).is_equal_approx(45.0, 0.01)
	assert_float(pole.z).is_less(0.0)   # north
	# Orion (RA 84, Dec -1) is high in the south on a winter midnight, gone in summer
	var orion := Calendar.equatorial_vector(Vector2(deg_to_rad(84.0), deg_to_rad(-1.0)))
	assert_float(_alt_deg(_cal(0, 0, 0.0).celestial_to_world() * orion)).is_greater(35.0)
	assert_float(_alt_deg(_cal(6, 4, 0.0).celestial_to_world() * orion)).is_less(-20.0)


func test_moon_phase_follows_the_day_of_the_month() -> void:
	var c := _cal(4, 0, 0.5)
	assert_float(c.moon_illumination()).is_less(0.01)
	assert_str(c.phase_name()).is_equal("New Moon")
	c = _cal(4, 5, 0.0)   # midnight starting the 6th day: full
	assert_float(c.moon_illumination()).is_greater(0.99)
	# a full moon at midnight stands in the south, a new one sits with the sun
	assert_float(_alt_deg(c.moon_dir())).is_greater(15.0)
	assert_float(c.moon_dir().z).is_greater(0.0)
	# a waxing crescent is in the west after sunset, above the horizon
	c = _cal(4, 1, 0.0)
	c.time = c.sunset() + 0.02
	assert_float(c.moon_dir().x).is_less(0.0)
	assert_float(_alt_deg(c.moon_dir())).is_greater(0.0)
	assert_str(c.phase_name()).contains("Waxing")


func test_harvest_moon_is_big() -> void:
	var june := _cal(5, 5, 0.0)
	var sept := _cal(8, 5, 0.0)
	assert_float(sept.moon_size()).is_greater(june.moon_size() * 1.15)
	assert_str(sept.moon_name()).contains("Harvest Moon")
	assert_str(_cal(5, 2, 0.0).moon_name()).is_equal("")


func test_eclipses_are_rare_and_real() -> void:
	var c := Calendar.new()
	var solar := 0
	var lunar := 0
	var first_solar := {}
	var years := 60
	var end := c.total() + years * c.year_days()
	while true:
		var e := c.next_eclipse()
		if e.is_empty() or float(e["t"]) > end:
			break
		if e["kind"] == "solar":
			solar += 1
			if first_solar.is_empty():
				first_solar = e
		else:
			lunar += 1
		c.advance(float(e["t"]) - c.total() + 0.2)
	# real rates scaled to the game year: ~0.8 solar and ~1.4 lunar per year
	assert_float(solar / float(years)).is_between(0.4, 1.3)
	assert_float(lunar / float(years)).is_between(0.9, 2.0)
	# a solar eclipse happens at noon with the sun up, and is gone a little later
	var s := Calendar.new()
	s.advance(float(first_solar["t"]) - s.total())
	assert_str(s.eclipse()["kind"]).is_equal("solar")
	assert_float(_alt_deg(s.sun_dir())).is_greater(15.0)
	s.advance(0.2)
	assert_str(s.eclipse()["kind"]).is_equal("")


func test_disc_overlap() -> void:
	assert_float(Atmosphere.disc_overlap(0.0, 1.0, 1.0)).is_equal_approx(1.0, 1e-4)
	assert_float(Atmosphere.disc_overlap(2.5, 1.0, 1.0)).is_equal(0.0)
	assert_float(Atmosphere.disc_overlap(1.0, 1.0, 1.0)).is_between(0.38, 0.40)


func test_save_round_trip() -> void:
	var c := _cal(8, 4, 0.7)
	c.advance(200.0)
	var d := Calendar.new()
	d.load_dict(c.to_dict())
	assert_str(d.date_text()).is_equal(c.date_text())
	assert_float(d.time).is_equal_approx(c.time, 1e-6)


func test_crops_rest_in_winter() -> void:
	assert_float(_cal(0, 3, 0.5).crop_growth()).is_equal(0.0)
	assert_float(_cal(4, 3, 0.5).crop_growth()).is_equal(1.0)
	assert_float(_cal(9, 3, 0.5).crop_growth()).is_equal(0.5)
	# snow lies in January, not in July; leaves turn in October
	assert_float(_cal(0, 4, 0.5).snow_cover()).is_equal(1.0)
	assert_float(_cal(6, 4, 0.5).snow_cover()).is_equal(0.0)
	assert_float(_cal(9, 4, 0.5).autumn_colour()).is_greater(0.9)
	assert_float(_cal(5, 4, 0.5).autumn_colour()).is_equal(0.0)


func test_crops_do_not_grow_in_winter() -> void:
	var vox := {}
	var farm := Farming.load_default()
	farm.get_voxel = func(p: Vector3i) -> int: return int(vox.get(p, GameMaterials.AIR))
	farm.set_voxel = func(p: Vector3i, m: int) -> void:
		if m == GameMaterials.AIR:
			vox.erase(p)
		else:
			vox[p] = m
	farm.rng.seed = 99
	for x in range(-3, 4):
		for z in range(-3, 4):
			vox[Vector3i(x, 0, z)] = GameMaterials.GRASS
	vox[Vector3i(2, 0, 0)] = GameMaterials.WATER
	farm.till(Vector3i(0, 0, 0))
	farm.farmland[Vector3i(0, 0, 0)] = 7
	farm.plant(Vector3i(0, 0, 0), "wheat")
	farm.season_growth = _cal(0, 3, 0.5).crop_growth()
	for s in 2 * 3600:
		farm.tick(20)
	assert_int(int(farm.crops[Vector3i(0, 1, 0)]["age"])).is_equal(0)
	farm.season_growth = _cal(4, 3, 0.5).crop_growth()
	for s in 2 * 3600:
		farm.tick(20)
	assert_int(int(farm.crops[Vector3i(0, 1, 0)]["age"])).is_greater(2)
