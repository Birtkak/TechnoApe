extends GdUnitTestSuite

## The micro simulation's kernel pieces (ADR 0029): the toroidal window,
## held chunks in the 0.5 m bubble, expanding bubble chunks into 6.25 cm cells,
## and publishing changed bricks back (store cells + majority voxels).


func _micro(chunks: Vector3i) -> SimWorld:
	var w := SimWorld.new()
	w.configure(chunks, 2, 7)
	assert_bool(w.set_wrapped(true)).is_true()
	w.set_materials(GameMaterials.defs())
	w.set_reactions(GameMaterials.reactions())
	return w


func _coarse() -> SimWorld:
	var w := SimWorld.new()
	w.configure(Vector3i(3, 3, 3), 2, 3)
	w.set_materials(GameMaterials.defs())
	w.set_reactions(GameMaterials.reactions())
	w.set_liquid_fullness(true)
	return w


func test_wrapping_needs_powers_of_two() -> void:
	var w := SimWorld.new()
	w.configure(Vector3i(3, 2, 4), 1, 1)
	assert_bool(w.set_wrapped(true)).is_false()


func test_toroidal_shift_keeps_the_cells_that_stay() -> void:
	var w := _micro(Vector3i(4, 2, 4))
	var before := {}
	for k in 400:
		var p := Vector3i((k * 37) % 64, (k * 11) % 32, (k * 23) % 64)
		var m := GameMaterials.STONE + k % 5
		w.set_voxel(p, m)
		before[p] = m
	var d := Vector3i(1, 0, -2)
	var vacated: Array = w.shift(d)
	assert_int(vacated.size()).is_equal(4 * 2 * 4 - 3 * 2 * 2)   # every chunk whose source left the window
	for p: Vector3i in before:
		var l: Vector3i = p - d * 16
		if l.x < 0 or l.y < 0 or l.z < 0 or l.x >= 64 or l.y >= 32 or l.z >= 64:
			continue
		var got := w.get_voxel(l)
		if got != before[p]:
			fail("cell %s moved to %s holds %d, expected %d" % [p, l, got, before[p]])
			return
	for c: Vector3i in vacated:
		assert_int(int(w.chunk_info(c)["loaded"])).is_equal(0)


func test_sand_falls_across_the_physical_seam() -> void:
	var w := _micro(Vector3i(4, 2, 4))
	w.set_gravity(0.174)
	w.shift(Vector3i(2, 0, 1))   # the window's memory now wraps inside it
	w.mark_all_loaded(true)
	w.fill_box(Vector3i(0, 0, 0), Vector3i(64, 32, 64), 0)
	w.fill_box(Vector3i(0, 0, 0), Vector3i(64, 1, 64), GameMaterials.STONE)
	w.fill_box(Vector3i(24, 20, 10), Vector3i(40, 28, 26), GameMaterials.SAND)
	var sand := w.count_material(GameMaterials.SAND)
	for i in 240:
		w.tick(1)
	assert_int(w.count_material(GameMaterials.SAND)).is_equal(sand)
	assert_int(w.count_unsupported(GameMaterials.SAND)).is_equal(0)
	assert_int(w.count_in_box(Vector3i(0, 1, 0), Vector3i(64, 4, 64), GameMaterials.SAND)).is_greater(sand / 4)
	# nothing leaked to the far side of the window through the memory seam
	assert_int(w.count_in_box(Vector3i(0, 0, 0), Vector3i(8, 32, 64), GameMaterials.SAND)).is_equal(0)
	assert_int(w.count_in_box(Vector3i(56, 0, 0), Vector3i(64, 32, 64), GameMaterials.SAND)).is_equal(0)


func test_held_chunk_is_a_wall_and_does_not_tick() -> void:
	var w := _coarse()
	w.fill_box(Vector3i(16, 20, 16), Vector3i(20, 24, 20), GameMaterials.SAND)   # chunk (1,1,1), floating
	w.set_chunk_held(Vector3i(1, 1, 1), true)
	w.fill_box(Vector3i(10, 20, 18), Vector3i(16, 21, 19), GameMaterials.WATER)  # beside it, in chunk (0,1,1)
	assert_bool(w.is_chunk_held(Vector3i(1, 1, 1))).is_true()
	for i in 60:
		w.tick(1)
	assert_int(w.count_in_box(Vector3i(16, 20, 16), Vector3i(20, 24, 20), GameMaterials.SAND)).is_equal(64)
	assert_int(w.count_in_box(Vector3i(16, 16, 16), Vector3i(32, 32, 32), GameMaterials.WATER)).is_equal(0)
	w.set_chunk_held(Vector3i(1, 1, 1), false)
	for i in 60:
		w.tick(1)
	assert_int(w.count_in_box(Vector3i(16, 20, 16), Vector3i(20, 24, 20), GameMaterials.SAND)).is_less(64)


func test_held_chunks_are_not_written_into_the_store_or_carved() -> void:
	var w := _coarse()
	var s := BrickStore.new()
	s.configure(Vector3i(64, 64, 64))
	w.set_chunk_held(Vector3i(1, 1, 1), true)
	w.set_voxel(Vector3i(20, 20, 20), GameMaterials.STONE)
	assert_int(w.write_dirty_to_store(s, Vector3i.ZERO, 8)).is_equal(0)
	assert_int(w.fill_sphere(Vector3(24, 24, 24), 3.0, GameMaterials.WATER, PackedInt32Array())).is_equal(0)


func _coarse_with_samples() -> SimWorld:
	var c := _coarse()
	c.mark_all_loaded(false)
	var bytes := PackedByteArray()
	bytes.resize(4096 * 2)
	var put := func(x: int, y: int, z: int, v: int) -> void:
		var i := (y + 16 * (x + 16 * z)) * 2
		bytes[i] = v & 255
		bytes[i + 1] = v >> 8
	for x in 16:
		for z in 16:
			put.call(x, 0, z, GameMaterials.STONE)
	put.call(2, 1, 2, GameMaterials.DIRT)
	put.call(4, 1, 4, GameMaterials.FLUID_LEVEL_BASE + 0 * 8 + (3 - 1))   # 3/8 water (fluid slot 0)
	put.call(6, 1, 6, GameMaterials.TALL_GRASS)
	put.call(8, 1, 8, GameMaterials.SAND)   # loose sand on the floor: wakes its micro chunk
	c.set_chunk_bytes(Vector3i(1, 1, 1), bytes, 2)
	return c


func test_load_from_coarse_expands_bricks() -> void:
	var c := _coarse_with_samples()
	var m := _micro(Vector3i(8, 8, 8))   # one coarse chunk = 128 cells
	m.set_liquid_fullness(true)
	m.mark_all_loaded(false)
	assert_int(m.load_from_coarse(c, Vector3i(1, 1, 1), Vector3i.ZERO)).is_equal(512)
	assert_int(m.get_voxel(Vector3i(3, 7, 100))).is_equal(GameMaterials.STONE)
	assert_int(m.get_voxel(Vector3i(16 + 5, 8 + 7, 16 + 1))).is_equal(GameMaterials.DIRT)
	# 3/8 water: the bottom three cell layers of its brick
	assert_int(m.get_voxel(Vector3i(32 + 4, 8 + 2, 32 + 4))).is_equal(GameMaterials.WATER)
	assert_int(m.get_voxel(Vector3i(32 + 4, 8 + 3, 32 + 4))).is_equal(0)
	assert_int(m.brick_majority(Vector3i(4, 1, 4)).x).is_equal(GameMaterials.WATER)
	assert_int(m.brick_majority(Vector3i(4, 1, 4)).y).is_equal(3)
	assert_int(m.brick_majority(Vector3i(6, 1, 6)).x).is_equal(GameMaterials.TALL_GRASS)
	# only chunks with something that moves are awake: the sand's, not the stone floor's
	assert_int(int(m.chunk_info(Vector3i(4, 0, 4))["active_next"])).is_equal(1)
	assert_int(int(m.chunk_info(Vector3i(0, 0, 0))["active_next"])).is_equal(0)
	assert_int(int(m.chunk_info(Vector3i(7, 7, 7))["loaded"])).is_equal(1)
	assert_int(int(m.chunk_info(Vector3i(0, 0, 0))["loaded"])).is_equal(1)


func test_publish_writes_majority_and_cells() -> void:
	var c := _coarse_with_samples()
	c.set_chunk_held(Vector3i(1, 1, 1), true)
	var m := _micro(Vector3i(8, 8, 8))
	m.set_liquid_fullness(true)
	m.mark_all_loaded(false)
	m.load_from_coarse(c, Vector3i(1, 1, 1), Vector3i.ZERO)
	var s := BrickStore.new()
	s.configure(Vector3i(64, 64, 64))
	var patterns := {GameMaterials.TALL_GRASS: PackedByteArray()}
	patterns[GameMaterials.TALL_GRASS].resize(512)
	patterns[GameMaterials.TALL_GRASS][7] = 200
	s.set_micro_patterns(patterns)
	var world_cell := Vector3i(16, 16, 16) * 8   # micro cell (0,0,0) = world voxel (16,16,16)
	# floor brick (10,0,10): carve its top 5 layers -> 3/8 stone -> the voxel becomes air
	m.fill_box(Vector3i(80, 3, 80), Vector3i(88, 8, 88), 0)
	# floor brick (12,0,12): carve 2 layers -> still stone
	m.fill_box(Vector3i(96, 6, 96), Vector3i(104, 8, 104), 0)
	# the grass brick, untouched cells but rewritten: shows its pattern
	m.fill_box(Vector3i(48, 8, 48), Vector3i(56, 16, 56), GameMaterials.TALL_GRASS)
	c.clear_dirty()
	var n := m.publish(s, Vector3i.ZERO, c, world_cell, Vector3i.ZERO, 0)
	assert_int(n).is_equal(3)
	assert_int(m.get_publish_pending()).is_equal(0)
	assert_int(c.get_voxel(Vector3i(26, 16, 26))).is_equal(0)
	assert_int(c.get_voxel(Vector3i(28, 16, 28))).is_equal(GameMaterials.STONE)
	assert_array(c.get_dirty_chunks()).contains([Vector3i(1, 1, 1)])
	assert_bool(s.is_expanded(Vector3i(26, 16, 26))).is_true()
	assert_int(s.get_cell(Vector3i(26, 16, 26) * 8 + Vector3i(1, 2, 1))).is_equal(GameMaterials.STONE)
	assert_int(s.get_cell(Vector3i(26, 16, 26) * 8 + Vector3i(1, 3, 1))).is_equal(0)
	assert_int(s.get_cell(Vector3i(22, 17, 22) * 8 + Vector3i(7, 0, 0))).is_equal(200)
	assert_int(s.get_cell(Vector3i(22, 17, 22) * 8 + Vector3i(6, 0, 0))).is_equal(0)
	# a homogeneous brick collapses to one word again
	m.fill_box(Vector3i(80, 0, 80), Vector3i(88, 8, 88), GameMaterials.STONE)
	m.publish(s, Vector3i.ZERO, c, world_cell, Vector3i.ZERO, 0)
	assert_bool(s.is_expanded(Vector3i(26, 16, 26))).is_false()
	assert_int(c.get_voxel(Vector3i(26, 16, 26))).is_equal(GameMaterials.STONE)


func test_publish_skips_bricks_that_left_the_window() -> void:
	var m := _micro(Vector3i(4, 2, 4))
	m.mark_all_loaded(true)
	m.fill_box(Vector3i(0, 0, 0), Vector3i(64, 2, 64), GameMaterials.STONE)
	m.shift(Vector3i(4, 0, 0))   # everything leaves before it was published
	assert_int(m.publish(null, Vector3i.ZERO, null, Vector3i.ZERO, Vector3i.ZERO, 0)).is_equal(0)
	assert_int(m.get_publish_pending()).is_equal(0)


func test_liquid_mass_is_kept_at_micro_scale() -> void:
	var m := _micro(Vector3i(4, 2, 4))
	m.set_liquid_fullness(true)
	m.set_gravity(0.174)
	m.mark_all_loaded(true)
	m.fill_box(Vector3i(0, 0, 0), Vector3i(64, 1, 64), GameMaterials.STONE)
	m.shift(Vector3i(1, 0, 1))
	m.mark_all_loaded(true)
	m.fill_box(Vector3i(0, 0, 0), Vector3i(64, 32, 64), 0)
	m.fill_box(Vector3i(0, 0, 0), Vector3i(64, 1, 64), GameMaterials.STONE)
	m.fill_box(Vector3i(26, 10, 26), Vector3i(38, 18, 38), GameMaterials.WATER)
	var mass := m.count_liquid_eighths(GameMaterials.WATER)
	for i in 200:
		m.tick(1)
	assert_int(m.count_liquid_eighths(GameMaterials.WATER)).is_equal(mass)
	assert_int(m.count_in_box(Vector3i(0, 10, 0), Vector3i(64, 32, 64), GameMaterials.WATER)).is_equal(0)


func test_plants_are_their_pattern_cells() -> void:
	var c := _coarse_with_samples()
	var m := _micro(Vector3i(8, 8, 8))
	m.set_micro_masks(NearField.micro_patterns())
	m.mark_all_loaded(false)
	m.load_from_coarse(c, Vector3i(1, 1, 1), Vector3i.ZERO)
	var pattern: PackedByteArray = NearField.micro_patterns()[GameMaterials.TALL_GRASS]
	var cells := 0
	for k in 512:
		var p := Vector3i(48 + k % 8, 8 + (k / 8) % 8, 48 + k / 64)
		var want := GameMaterials.TALL_GRASS if pattern[k] != 0 else 0
		cells += 1 if pattern[k] != 0 else 0
		if m.get_voxel(p) != want:
			fail("cell %d of the tuft is %d, expected %d" % [k, m.get_voxel(p), want])
			return
	assert_int(cells).is_less(400)
	assert_int(m.brick_majority(Vector3i(6, 1, 6)).x).is_equal(GameMaterials.TALL_GRASS)
	m.fill_box(Vector3i(48, 8, 48), Vector3i(56, 16, 52), 0)   # half of it cut away: no longer grass
	assert_int(m.brick_majority(Vector3i(6, 1, 6)).x).is_equal(0)


func _ticks_to_settle(cascade: bool, material: int) -> int:
	var m := _micro(Vector3i(4, 4, 4))
	m.set_gravity(0.174)
	m.set_liquid_fullness(true)
	m.set_cascade(cascade)
	m.fill_box(Vector3i(0, 0, 0), Vector3i(64, 1, 64), GameMaterials.STONE)
	m.fill_sphere(Vector3(32, 40, 32), 12.0, material, PackedInt32Array())   # a 1.5 m ball, 1.6 m above the floor
	for t in 400:
		m.tick(1)
		if m.count_in_box(Vector3i(0, 28, 0), Vector3i(64, 64, 64), material) == 0:   # all of it below where the ball began
			return t + 1
	return 400


func test_cascade_lets_a_ball_fall_together() -> void:
	var slow := _ticks_to_settle(false, GameMaterials.SAND)
	var fast := _ticks_to_settle(true, GameMaterials.SAND)
	print("sand ball fell clear after %d ticks (cascade) vs %d (one layer per tick)" % [fast, slow])
	assert_int(fast).is_less_equal(slow)   # sand: the pile's one-step slopes, not the fall, set the pace
	var water := _ticks_to_settle(true, GameMaterials.WATER)
	var water_slow := _ticks_to_settle(false, GameMaterials.WATER)
	print("water ball: %d ticks (cascade) vs %d" % [water, water_slow])
	assert_int(water).is_less(water_slow * 2 / 3)


func test_water_washes_through_a_meadow() -> void:
	var m := _micro(Vector3i(4, 2, 4))
	m.set_liquid_fullness(true)
	m.set_gravity(0.174)
	m.set_cascade(true)
	m.set_micro_masks(NearField.micro_patterns())
	m.fill_box(Vector3i(0, 0, 0), Vector3i(64, 1, 64), GameMaterials.STONE)
	m.fill_box(Vector3i(0, 1, 0), Vector3i(64, 9, 64), GameMaterials.TALL_GRASS)   # a meadow: one tuft per brick
	var grass := m.count_material(GameMaterials.TALL_GRASS)
	m.fill_box(Vector3i(24, 1, 24), Vector3i(40, 20, 40), GameMaterials.WATER)       # a 1 m column of water in it
	var mass := m.count_liquid_eighths(GameMaterials.WATER)
	for t in 120:
		m.tick(1)
	assert_int(m.count_liquid_eighths(GameMaterials.WATER)).is_equal(mass)
	assert_int(m.count_material(GameMaterials.TALL_GRASS)).is_less(grass)            # washed away where it flowed
	assert_int(m.count_in_box(Vector3i(0, 1, 0), Vector3i(16, 9, 64), GameMaterials.WATER)).is_greater(0)   # and it got 0.5 m+ out
	assert_int(m.count_in_box(Vector3i(0, 12, 0), Vector3i(64, 32, 64), GameMaterials.WATER)).is_equal(0)  # the column fell
