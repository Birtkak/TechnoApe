extends GdUnitTestSuite

## ADR 0104: world water is endless. A still body never turns into moving water; it pours a full moving cell into
## a fresh opening beside (under head) or below it and keeps its own cells, so a hole takes water until it is full
## and the river stays where it was. A heavier cell sinking into the body eats the still cell instead of lifting it.

const N := 3


func _world() -> SimWorld:
	var w := SimWorld.new()
	w.configure(Vector3i(N, N, N), 2, 5)
	w.set_materials(GameMaterials.defs())
	w.set_reactions(GameMaterials.reactions())
	w.mark_all_loaded(true)
	w.set_liquid_fullness(true)
	w.set_cascade(true)
	w.set_gravity(0.17)
	return w


func _run(w: SimWorld, ticks: int) -> void:
	for i in ticks:
		w.tick(1)


func test_a_hole_under_a_body_fills_and_the_body_stays() -> void:
	var w := _world()
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 12, 48), GameMaterials.STONE)
	w.fill_box(Vector3i(16, 12, 16), Vector3i(32, 16, 32), GameMaterials.WATER_STATIC)   # a 16 x 4 x 16 pool on the floor
	_run(w, 3)   # past the load: the pool sleeps
	w.fill_box(Vector3i(22, 6, 22), Vector3i(26, 12, 26), 0)   # a 4 x 6 x 4 pit dug under it
	_run(w, 300)
	assert_int(w.count_in_box(Vector3i(16, 12, 16), Vector3i(32, 16, 32), GameMaterials.WATER_STATIC)) \
		.override_failure_message("the body lost still cells").is_equal(16 * 4 * 16)
	var pit := w.count_in_box(Vector3i(22, 6, 22), Vector3i(26, 12, 26), GameMaterials.WATER)
	assert_int(pit).override_failure_message("the pit took %d of 96 cells" % pit).is_greater_equal(90)
	assert_int(w.count_in_box(Vector3i(0, 16, 0), Vector3i(48, 48, 48), GameMaterials.WATER)) \
		.override_failure_message("water above the body").is_equal(0)


func test_a_breach_in_the_bank_fills_to_under_the_surface_and_stops() -> void:
	var w := _world()
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 8, 48), GameMaterials.STONE)
	w.fill_box(Vector3i(8, 8, 8), Vector3i(24, 16, 40), GameMaterials.STONE)     # a basin block
	w.fill_box(Vector3i(9, 8, 9), Vector3i(23, 16, 39), GameMaterials.WATER_STATIC)   # water in it, surface at y 15
	_run(w, 3)
	w.fill_box(Vector3i(23, 8, 20), Vector3i(30, 16, 24), 0)   # the bank cut open: a 7 x 8 x 4 trench to the floor outside
	_run(w, 600)
	assert_int(w.count_in_box(Vector3i(9, 8, 9), Vector3i(23, 16, 39), GameMaterials.WATER_STATIC)) \
		.override_failure_message("the body woke").is_equal(14 * 8 * 30)
	var trench := w.count_in_box(Vector3i(23, 8, 20), Vector3i(30, 16, 24), GameMaterials.WATER)
	assert_int(trench).override_failure_message("the trench took %d cells" % trench).is_greater_equal(120)
	assert_int(w.count_in_box(Vector3i(0, 16, 0), Vector3i(48, 48, 48), GameMaterials.WATER)).override_failure_message("water above the surface").is_equal(0)
	# the river goes on flooding the lower plain past the trench (it is endless), but only its moving front costs:
	# a trickle of a few cells a tick, a handful of chunks awake, never a burst
	var before := w.count_liquid_eighths(GameMaterials.WATER)
	_run(w, 600)
	var per_tick := (w.count_liquid_eighths(GameMaterials.WATER) - before) / 600.0
	assert_float(per_tick).override_failure_message("the breach pours %.1f eighths a tick" % per_tick).is_less(64.0)
	assert_int(int(w.get_stats()["active_chunks"])).override_failure_message("the flood keeps too much awake").is_less_equal(16)


func test_lava_sinking_into_a_body_lifts_no_water() -> void:
	var w := _world()
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 8, 48), GameMaterials.STONE)
	w.fill_box(Vector3i(16, 8, 16), Vector3i(32, 14, 32), GameMaterials.WATER_STATIC)
	_run(w, 3)
	w.set_voxel(Vector3i(24, 20, 24), GameMaterials.LAVA)
	_run(w, 200)
	assert_int(w.count_in_box(Vector3i(0, 14, 0), Vector3i(48, 48, 48), GameMaterials.WATER_STATIC)) \
		.override_failure_message("still water lifted above the body").is_equal(0)
	# what rose above the body is moving water (the cell the lava first traded places with), a film that dries;
	# the body refills what the lava boils at the bed, as an endless body does
	assert_int(w.count_in_box(Vector3i(0, 14, 0), Vector3i(48, 48, 48), GameMaterials.WATER)).is_less_equal(8)


## Bug report 2026-11-07 10-26-45: a multiplayer client replays digs as each column loads, and the river's column
## loaded after the dug one; the dig was stale by then and the river stood as a wall beside the hole for good.
func test_a_body_loaded_after_a_dig_beside_it_still_pours_into_it() -> void:
	var w := _world()
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 16, 48), GameMaterials.STONE)   # ground up to y 15 everywhere
	w.set_chunk_loaded(Vector3i(0, 0, 1), false)   # the river's chunk is not in yet
	_run(w, 3)
	w.fill_box(Vector3i(16, 8, 20), Vector3i(24, 16, 28), 0)   # an 8 x 8 x 8 pit dug against its side
	_run(w, 30)   # the dig goes stale
	var bytes := PackedByteArray()
	bytes.resize(16 * 16 * 16)
	for z in 16:
		for x in 16:
			for y in 16:   # y fastest: stone floor, still water up to y 14, air on top
				bytes[y + 16 * (x + 16 * z)] = GameMaterials.STONE if y < 8 else (GameMaterials.WATER_STATIC if y < 15 else 0)
	w.set_chunk_bytes(Vector3i(0, 0, 1), bytes, 1)
	_run(w, 400)
	var pit := w.count_in_box(Vector3i(16, 8, 20), Vector3i(24, 16, 28), GameMaterials.WATER)
	assert_int(pit).override_failure_message("the pit took %d cells" % pit).is_greater_equal(300)   # 384 under the surface
	assert_int(w.count_in_box(Vector3i(16, 15, 20), Vector3i(24, 16, 28), GameMaterials.WATER)) \
		.override_failure_message("water above the surface").is_equal(0)
