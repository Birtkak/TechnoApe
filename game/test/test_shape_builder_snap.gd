extends GdUnitTestSuite

## The shape builder's snap grid: a placement's corner lands on it across the ground, and a handle
## step (a drag or the wheel over it) moves sides and position by it.


func _builder() -> ShapeBuilder:
	var b: ShapeBuilder = auto_free(ShapeBuilder.new())   # off the tree: no player, camera or handles
	b.snap = Settings.BLOCK
	return b


func test_a_brush_choice_leaves_build_mode() -> void:
	var b := _builder()
	var p: Player = auto_free(Player.new())
	p._crack = auto_free(Node3D.new())
	b.player = p
	b.menu = auto_free(RadialMenu.new())
	b._preview = auto_free(MeshInstance3D.new())
	b._hint = auto_free(Label.new())
	b.state = ShapeBuilder.State.AIM
	p.building = true
	b.menu.open([{"id": "mine:flat", "label": "Flat"}], "", 0)
	b._close_menu()
	assert_str(p.brush_shape).is_equal("flat")
	assert_bool(b.is_active()).is_false()
	assert_bool(p.building).is_false()


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
