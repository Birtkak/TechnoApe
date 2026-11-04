extends GdUnitTestSuite

## ADR 0138: how water feels. A channel dug from a still body fills to the body's level (resting water on ground is
## still an opening), so does a trench cut into a shelving bank that only the surface row touches, water resting over
## a lower still step is no opening (no endless feeding), a film runs down a terraced slope instead of lying on every
## step, and a flood rests flat across a row boundary and past the conveyor's reach (the level pass).

const N := 3


func _world() -> SimWorld:
	var w := SimWorld.new()
	w.configure(Vector3i(N, N, N), 2, 5)
	w.set_materials(load("res://test/test_sim_0119.gd")._no_films(GameMaterials.defs()))   # exact mass
	w.set_reactions(GameMaterials.reactions())
	w.mark_all_loaded(true)
	w.set_liquid_fullness(true)
	w.set_column_flow(true)
	w.set_cascade(true)
	w.set_gravity(0.26)
	return w


func _run(w: SimWorld, ticks: int) -> void:
	for i in ticks:
		w.tick(1)


func _eighths(w: SimWorld, p: Vector3i) -> int:
	var c: Dictionary = w.cell_info(p)
	return int(c.get("eighths", 0)) if c.get("material", 0) == GameMaterials.WATER else 0


func test_channel_dug_in_steps_fills_to_the_lake() -> void:
	var w := _world()
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 16, 48), GameMaterials.STONE)
	w.fill_box(Vector3i(2, 6, 4), Vector3i(16, 16, 40), GameMaterials.WATER_STATIC)   # surface row 15
	_run(w, 3)
	for step in 4:   # dug from the lake outward, a step every 15 ticks
		w.fill_box(Vector3i(16 + step * 7, 11, 20), Vector3i(23 + step * 7, 17, 24), GameMaterials.AIR)
		_run(w, 15)
	_run(w, 400)
	var full := 0
	var top := 0
	for z in range(20, 24):
		for x in range(16, 44):
			for y in range(11, 15):
				full += 1 if _eighths(w, Vector3i(x, y, z)) == 8 else 0
			top += _eighths(w, Vector3i(x, 15, z))
	assert_int(full).is_equal(28 * 4 * 4)   # every cell under the lake's surface row full
	assert_float(top / (28.0 * 4.0)).is_greater_equal(5.5)   # its surface row topped up close to the lake's (an eighth per 14 cells of tilt)
	assert_int(w.get_active_chunk_count()).is_equal(0)


func test_trench_in_a_shelving_bank_fills_to_the_surface() -> void:
	var w := _world()
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 16, 48), GameMaterials.STONE)
	w.fill_box(Vector3i(2, 6, 4), Vector3i(16, 16, 40), GameMaterials.WATER_STATIC)   # surface row 15
	w.fill_box(Vector3i(10, 6, 4), Vector3i(16, 15, 40), GameMaterials.STONE)         # the bank shelves: only row 15 meets the cut
	_run(w, 3)
	w.fill_box(Vector3i(16, 12, 20), Vector3i(28, 20, 27), GameMaterials.AIR)   # floor three cells under the surface
	_run(w, 300)
	var full := 0
	for z in range(20, 27):
		for x in range(16, 28):
			for y in range(12, 15):
				full += 1 if _eighths(w, Vector3i(x, y, z)) == 8 else 0
	assert_int(full).is_equal(12 * 7 * 3)   # ADR 0104 alone: the surface row never poured, the cut stayed dry
	assert_int(w.get_active_chunk_count()).is_equal(0)


func test_water_resting_on_a_lower_still_step_is_no_opening() -> void:
	var w := _world()
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 8, 48), GameMaterials.STONE)
	w.fill_box(Vector3i(0, 8, 0), Vector3i(16, 16, 48), GameMaterials.WATER_STATIC)    # upper step, surface row 15
	w.fill_box(Vector3i(16, 8, 0), Vector3i(48, 12, 48), GameMaterials.WATER_STATIC)   # lower step, surface row 11
	_run(w, 3)
	w.fill_box(Vector3i(17, 12, 20), Vector3i(19, 13, 22), GameMaterials.WATER)   # a splash on the lower step
	_run(w, 300)
	assert_int(w.count_liquid_eighths(GameMaterials.WATER)).is_equal(4 * 8)   # the upper step fed none of it
	assert_int(w.get_active_chunk_count()).is_equal(0)


func test_film_runs_down_a_terraced_slope() -> void:
	var w := _world()
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 8, 48), GameMaterials.STONE)
	for x in 36:
		w.fill_box(Vector3i(x, 8, 0), Vector3i(x + 1, 30 - x / 3, 48), GameMaterials.STONE)   # a voxel down every three
	w.fill_box(Vector3i(1, 30, 20), Vector3i(6, 34, 28), GameMaterials.WATER)
	var mass := w.count_liquid_eighths(GameMaterials.WATER)
	_run(w, 500)
	var on_steps := 0
	for z in 48:
		for x in 36:
			for y in range(18, 31):
				on_steps += _eighths(w, Vector3i(x, y, z))
	assert_int(on_steps).is_less_equal(mass / 100)   # was a film of two eighths on every step
	assert_int(w.count_liquid_eighths(GameMaterials.WATER)).is_equal(mass)


func test_flood_rests_flat_across_a_row_boundary() -> void:
	var w := _world()
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 8, 48), GameMaterials.STONE)
	w.fill_box(Vector3i(0, 8, 0), Vector3i(10, 22, 48), GameMaterials.WATER)   # a tank's worth let go on one side
	var mass := w.count_liquid_eighths(GameMaterials.WATER)
	_run(w, 600)
	var lo := 1 << 30
	var hi := -1
	for h in load("res://test/test_sim_0119.gd").surface_heights(w, 0, 48, 0, 48, 8, 24, GameMaterials.WATER).values():
		lo = mini(lo, h)
		hi = maxi(hi, h)
	assert_int(hi - lo).is_less_equal(4)   # the old reach rested an eighth per seven cells: 10 eighths, more than a cell, across 6 m
	assert_int(w.count_liquid_eighths(GameMaterials.WATER)).is_equal(mass)
	assert_int(w.get_active_chunk_count()).is_equal(0)


func test_resting_flood_levels_past_the_conveyors_reach() -> void:
	var w := _world()
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 8, 48), GameMaterials.STONE)
	w.fill_box(Vector3i(0, 8, 0), Vector3i(10, 22, 48), GameMaterials.WATER)
	var mass := w.count_liquid_eighths(GameMaterials.WATER)
	_run(w, 600)
	var lo := 1 << 30
	var hi := -1
	for h in load("res://test/test_sim_0119.gd").surface_heights(w, 0, 48, 0, 48, 8, 24, GameMaterials.WATER).values():
		lo = mini(lo, h)
		hi = maxi(hi, h)
	assert_int(hi - lo).is_less_equal(1)   # the conveyor alone rests a terrace of an eighth per 14 cells: 3-4 eighths here
	assert_int(w.count_liquid_eighths(GameMaterials.WATER)).is_equal(mass)
	assert_int(w.get_active_chunk_count()).is_equal(0)
