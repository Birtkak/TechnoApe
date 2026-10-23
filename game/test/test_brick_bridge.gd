extends GdUnitTestSuite

## SimWorld.write_dirty_to_store: the simulation's changed cells reach the
## BrickStore (ADR 0012 step 3), at brick scale (the 0.5 m game) and at cell
## scale (micro simulation), without destroying chiseled bricks.

const N := 3   # sim chunks per axis (48 cells)


func _sim() -> SimWorld:
	var w := SimWorld.new()
	w.configure(Vector3i(N, N, N), 2, 3)
	w.set_materials(GameMaterials.defs())
	w.set_reactions(GameMaterials.reactions())
	w.mark_all_loaded(true)
	return w


func _store(bricks: int) -> BrickStore:
	var s := BrickStore.new()
	s.configure(Vector3i(bricks, bricks, bricks))
	return s


func test_brick_scale_mirrors_the_simulation() -> void:
	var w := _sim()
	var s := _store(64)
	var origin := Vector3i(5, 2, 7)      # store brick of sim cell (0,0,0)
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 1, 48), GameMaterials.STONE)
	w.fill_box(Vector3i(10, 20, 10), Vector3i(18, 30, 18), GameMaterials.SAND)
	for i in 40:
		w.tick(1)
		w.write_dirty_to_store(s, origin, 8)
		w.clear_dirty()
	for z in 48:
		for y in 48:
			for x in 48:
				var m := w.get_voxel(Vector3i(x, y, z))
				var b := s.brick_material(origin + Vector3i(x, y, z))
				if m != b:
					fail("sim %s = %d but store brick = %d" % [Vector3i(x, y, z), m, b])
					return


func test_chiseled_brick_survives_but_a_changed_one_is_rewritten() -> void:
	var w := _sim()
	var s := _store(64)
	# a stone brick with its top quarter chiseled away (3/4 stone, so its majority is stone)
	var br := Vector3i(4, 4, 4)
	s.fill_uniform(br, br + Vector3i.ONE, GameMaterials.STONE)
	s.fill_cells(br * 8 + Vector3i(0, 6, 0), br * 8 + Vector3i(8, 8, 8), 0)
	var before := s.get_cell(br * 8 + Vector3i(1, 7, 1))
	assert_int(before).is_equal(0)
	w.set_voxel(Vector3i(4, 4, 4), GameMaterials.STONE)   # the sim says stone there (unchanged material)
	w.set_voxel(Vector3i(6, 4, 4), GameMaterials.WATER)   # and water next to it
	assert_int(w.write_dirty_to_store(s, Vector3i.ZERO, 8)).is_equal(1)
	assert_bool(s.is_expanded(br)).is_true()                              # chisel kept
	assert_int(s.get_cell(br * 8 + Vector3i(1, 7, 1))).is_equal(0)
	assert_int(s.brick_material(Vector3i(6, 4, 4))).is_equal(GameMaterials.WATER)
	w.clear_dirty()
	w.set_voxel(Vector3i(4, 4, 4), 0)                                      # the sim removed it: rewritten
	w.write_dirty_to_store(s, Vector3i.ZERO, 8)
	assert_bool(s.is_expanded(br)).is_false()
	assert_int(s.brick_material(br)).is_equal(0)


func test_partial_liquid_loads_as_layers() -> void:
	var w := _sim()
	var s := _store(64)
	# chunk (0,0,0): stone floor, a half-full (4 eighths) water voxel and a full one, encoded as the terrain stores them
	var bytes := PackedByteArray()
	bytes.resize(4096 * 2)
	for x in 16:
		for z in 16:
			var i := (0 + 16 * (x + 16 * z)) * 2
			bytes[i] = GameMaterials.STONE
	var half := GameMaterials.FLUID_LEVEL_BASE + 0 * 8 + (4 - 1)   # water is fluid slot 0
	var ih := (1 + 16 * (5 + 16 * 5)) * 2
	bytes[ih] = half & 255
	bytes[ih + 1] = half >> 8
	var ifull := (1 + 16 * (6 + 16 * 5)) * 2
	bytes[ifull] = GameMaterials.WATER
	w.set_chunk_bytes(Vector3i.ZERO, bytes, 2)
	assert_int(w.get_voxel(Vector3i(5, 1, 5))).is_equal(GameMaterials.WATER)
	var origin := Vector3i(8, 8, 8)
	assert_int(w.write_chunk_to_store(s, Vector3i.ZERO, origin)).is_equal(4096)
	assert_int(s.brick_material(origin + Vector3i(3, 0, 3))).is_equal(GameMaterials.STONE)
	assert_int(s.brick_material(origin + Vector3i(6, 1, 5))).is_equal(GameMaterials.WATER)   # full: uniform
	var b := origin + Vector3i(5, 1, 5)
	assert_bool(s.is_expanded(b)).is_true()                                                  # half: four layers
	assert_int(s.get_cell(b * 8 + Vector3i(3, 3, 3))).is_equal(GameMaterials.WATER)
	assert_int(s.get_cell(b * 8 + Vector3i(3, 4, 3))).is_equal(0)
	assert_int(s.get_cell(b * 8 + Vector3i(7, 0, 7))).is_equal(GameMaterials.WATER)
	# nothing above 255 ever reaches the store
	for z in 16:
		for y in 2:
			for x in 16:
				var m := s.brick_material(origin + Vector3i(x, y, z))
				assert_bool(m == -1 or m <= 255).is_true()


func test_cell_scale_writes_cells() -> void:
	var w := _sim()
	var s := _store(16)
	var origin := Vector3i(13, 9, 21)    # store cell of sim cell (0,0,0), not brick-aligned on purpose
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 1, 48), GameMaterials.STONE)
	w.fill_box(Vector3i(20, 10, 20), Vector3i(26, 16, 26), GameMaterials.WATER)
	for i in 20:
		w.tick(1)
		w.write_dirty_to_store(s, origin, 1)
		w.clear_dirty()
	for z in 48:
		for y in 30:
			for x in 48:
				var m := w.get_voxel(Vector3i(x, y, z))
				var c := s.get_cell(origin + Vector3i(x, y, z))
				if m != c:
					fail("sim %s = %d but store cell = %d" % [Vector3i(x, y, z), m, c])
					return
