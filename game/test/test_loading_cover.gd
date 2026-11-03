extends GdUnitTestSuite

## The loading cover waits only for ground the box can draw (World.near_rings): at the low preset's 24 m the
## 32 m ring is never meshed, and waiting for it held every cover to its 20 s cap (2026-11-03).


func test_near_rings_stay_inside_the_view() -> void:
	var at_24 := World.near_rings(24.0).map(func(r: Array) -> float: return r[0])
	assert_array(at_24).is_equal([0.0, 4.0, 10.0, 16.0])
	assert_int(World.near_rings(72.0).size()).is_equal(World.NEAR_RINGS.size())
	for v in [16.0, 24.0, 40.0, 72.0]:
		for r in World.near_rings(v):
			assert_float(r[0]).is_less_equal(v * Settings.horizon_fade_frac)
