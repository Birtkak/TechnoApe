extends GdUnitTestSuite

## ADR 0069 B3: a one-eighth film on a foreign floor with nothing deeper of its kind beside it dries up after
## about film_lifetime_ticks (water), or crusts to basalt (lava). A pool deeper than a film keeps its mass.


func _world() -> SimWorld:
	var w := SimWorld.new()
	w.configure(Vector3i(4, 2, 4), 2, 5)
	w.set_materials(GameMaterials.defs())
	w.set_reactions(GameMaterials.reactions())
	w.mark_all_loaded(true)
	w.set_liquid_fullness(true)
	w.set_cascade(true)
	w.set_gravity(0.17)
	w.fill_box(Vector3i(0, 0, 0), Vector3i(64, 1, 64), GameMaterials.STONE)
	return w


## A little water let go on a wide floor thins to films, which then dry up: nothing is left after a minute.
func test_spill_dries_up() -> void:
	var w := _world()
	w.fill_box(Vector3i(31, 1, 31), Vector3i(33, 3, 33), GameMaterials.WATER)   # 8 cells: thins to a film sheet
	for i in 60 * 30:
		w.tick(1)
	assert_int(w.count_liquid_eighths(GameMaterials.WATER)).is_equal(0)


## A walled pool two cells deep is no film: it keeps all its mass.
func test_pool_keeps_its_water() -> void:
	var w := _world()
	w.fill_box(Vector3i(20, 1, 20), Vector3i(30, 4, 30), GameMaterials.STONE)
	w.fill_box(Vector3i(21, 1, 21), Vector3i(29, 3, 29), GameMaterials.WATER)
	var mass := w.count_liquid_eighths(GameMaterials.WATER)
	for i in 30 * 30:
		w.tick(1)
	assert_int(w.count_liquid_eighths(GameMaterials.WATER)).is_equal(mass)


## A lava spill thins and its films crust over as basalt (the rim beside two-eighth cells stays lava).
func test_lava_film_crusts() -> void:
	var w := _world()
	w.fill_box(Vector3i(31, 1, 31), Vector3i(33, 2, 33), GameMaterials.LAVA)
	var mass := w.count_liquid_eighths(GameMaterials.LAVA)
	for i in 60 * 30:
		w.tick(1)
	assert_int(w.count_liquid_eighths(GameMaterials.LAVA)).is_less(mass)
	assert_int(w.count_in_box(Vector3i(0, 1, 0), Vector3i(64, 2, 64), GameMaterials.BASALT)).is_greater(0)
