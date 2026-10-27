extends GdUnitTestSuite

## The shape builder's snap grid: a placement's corner lands on it across the ground, and a handle
## step (a drag or the wheel over it) moves sides and position by it.


func _builder() -> ShapeBuilder:
	var b: ShapeBuilder = auto_free(ShapeBuilder.new())   # off the tree: no player, camera or handles
	b.snap = Settings.BLOCK
	return b


func test_the_corner_snaps_across_the_ground_but_keeps_its_floor() -> void:
	var b := _builder()
	assert_that(b._snapped(Vector3i(5, 3, -1))).is_equal(Vector3i(4, 3, -4))
	b.snap = 1
	assert_that(b._snapped(Vector3i(5, 3, -1))).is_equal(Vector3i(5, 3, -1))
	assert_int(b._snap_step(5.4)).is_equal(5)
	b.snap = 8
	assert_int(b._snap_step(5.4)).is_equal(8)
	assert_int(b._snap_step(3.0)).is_equal(0)


func test_handle_steps_size_lift_and_hollow() -> void:
	var b := _builder()
	b.lo = Vector3i(4, 0, 4)
	b.wsize = Vector3i(8, 8, 8)
	assert_bool(b._apply("+x", b.lo, b.wsize, 0, 4, false)).is_true()
	assert_that(b.wsize).is_equal(Vector3i(12, 8, 8))
	assert_bool(b._apply("-z", b.lo, b.wsize, 0, 4, false)).is_true()
	assert_that(b.lo).is_equal(Vector3i(4, 0, 0))
	assert_that(b.wsize).is_equal(Vector3i(12, 8, 12))
	assert_bool(b._apply("move", b.lo, b.wsize, 0, -4, false)).is_true()
	assert_that(b.lo).is_equal(Vector3i(4, -4, 0))
	assert_bool(b._apply("top", b.lo, b.wsize, 0, 4, true)).is_true()
	assert_that(b.lo.y).is_equal(0)
	assert_bool(b._apply("hollow", b.lo, b.wsize, 0, 4, false)).is_true()
	assert_int(b.hollow).is_equal(1)
	assert_bool(b._apply("+x", b.lo, b.wsize, b.hollow, -40, false)).is_true()
	assert_that(b.wsize.x).is_equal(1)   # clamped, never zero
	assert_bool(b._apply("+x", b.lo, b.wsize, b.hollow, 0, false)).is_false()
