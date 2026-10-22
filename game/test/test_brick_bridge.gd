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
