extends GdUnitTestSuite

## Liquid fullness (P10, opt-in for the micro simulation): eighths of a cell
## are conserved, a released block spreads into a flat pool and then settles.

const N := 4   # chunks per axis (64 cells)


func _world() -> SimWorld:
	var w := SimWorld.new()
	w.configure(Vector3i(N, 2, N), 2, 9)
	w.set_materials(GameMaterials.defs())
	w.set_reactions(GameMaterials.reactions())
	w.mark_all_loaded(true)
	w.set_liquid_fullness(true)
	w.set_gravity(0.174)
	w.fill_box(Vector3i(0, 0, 0), Vector3i(64, 1, 64), GameMaterials.STONE)
	return w


func test_mass_is_conserved() -> void:
	var w := _world()
	w.fill_box(Vector3i(24, 1, 24), Vector3i(40, 17, 40), GameMaterials.WATER)
	var mass := w.count_liquid_eighths(GameMaterials.WATER)
	assert_int(mass).is_equal(16 * 16 * 16 * 8)
	for i in 120:
		w.tick(1)
	assert_int(w.count_liquid_eighths(GameMaterials.WATER)).is_equal(mass)


func test_block_spreads_flat_and_settles() -> void:
	var w := _world()
	# 8 x 8 x 8 cells of water = 4096 eighths: enough for a 1/8 film over 64 x 64
	w.fill_box(Vector3i(28, 1, 28), Vector3i(36, 9, 36), GameMaterials.WATER)
	var settled := -1
	for t in 900:
		var r: Dictionary = w.tick(1)
		if int(r["moved_cells_per_tick"]) == 0:
			settled = t
			break
	assert_int(settled).override_failure_message("never settled").is_greater(0)
	# a 0.5 m cube of water ends as a puddle one cell thick: nothing above the first layer,
	# spread well past the block (a shallow mound: neighbours differ by less than 2/8)
	assert_int(w.count_in_box(Vector3i(0, 2, 0), Vector3i(64, 32, 64), GameMaterials.WATER)).is_equal(0)
	assert_int(w.count_in_box(Vector3i(0, 1, 0), Vector3i(22, 2, 64), GameMaterials.WATER)).is_greater(0)


func test_front_moves_about_a_cell_per_tick() -> void:
	var w := _world()
	w.fill_box(Vector3i(0, 1, 0), Vector3i(8, 9, 64), GameMaterials.WATER)   # a wall of water along x = 0..7
	for t in 30:
		w.tick(1)
	# after 30 ticks the front has run at least 20 cells (P5 liquids: a few cells)
	assert_int(w.count_in_box(Vector3i(28, 1, 0), Vector3i(29, 2, 64), GameMaterials.WATER)).is_greater(0)
