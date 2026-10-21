extends GdUnitTestSuite

# P1: proves gdUnit4 runs headless and reports pass/fail through the CLI.


func test_noise_is_deterministic_for_seed() -> void:
	var a := FastNoiseLite.new()
	var b := FastNoiseLite.new()
	a.seed = 1234
	b.seed = 1234
	for i in 50:
		var x := float(i) * 3.7
		assert_float(a.get_noise_2d(x, -x)).is_equal_approx(b.get_noise_2d(x, -x), 0.000001)


func test_noise_differs_for_different_seed() -> void:
	var a := FastNoiseLite.new()
	var b := FastNoiseLite.new()
	a.seed = 1
	b.seed = 2
	assert_float(a.get_noise_2d(10.0, 10.0)).is_not_equal(b.get_noise_2d(10.0, 10.0))


func test_noise_stays_in_range() -> void:
	var n := FastNoiseLite.new()
	n.seed = 7
	for i in 200:
		var v := n.get_noise_3d(i * 1.3, i * 0.7, i * 2.1)
		assert_float(v).is_between(-1.0, 1.0)
