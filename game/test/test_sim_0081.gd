extends GdUnitTestSuite

## ADR 0081: lava boils the water it touches before it crusts, lone still specks wake on their own,
## and sample_cells tells which cells move or were just born (the material sounds).

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


## Loads chunk (0, 0, 0) "from the world" (no edit stamps) with the given cells: Dictionary Vector3i -> material.
func _load_cells(w: SimWorld, cells: Dictionary) -> void:
	var b := PackedByteArray()
	b.resize(4096 * 2)
	for p in cells:
		b[(p.y + 16 * (p.x + 16 * p.z)) * 2] = cells[p]
	w.set_chunk_bytes(Vector3i(0, 0, 0), b, 2)


func _run(w: SimWorld, ticks: int) -> void:
	for i in ticks:
		w.tick(1)


func test_lava_in_water_boils_more_than_it_crusts() -> void:
	var w := _world()
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 8, 48), GameMaterials.STONE)
	w.fill_box(Vector3i(19, 8, 19), Vector3i(28, 12, 28), GameMaterials.STONE)    # a basin
	w.fill_box(Vector3i(20, 8, 20), Vector3i(27, 12, 27), GameMaterials.WATER)    # a 7 x 4 x 7 pool in it
	var water := w.count_liquid_eighths(GameMaterials.WATER)
	for p in [Vector3i(21, 10, 21), Vector3i(25, 10, 21), Vector3i(23, 10, 23), Vector3i(21, 10, 25), Vector3i(25, 10, 25)]:
		w.set_voxel(p, GameMaterials.LAVA)   # five lava cells in it (each replaces one water cell)
	_run(w, 120)
	var obsidian := w.count_in_box(Vector3i(0, 0, 0), Vector3i(48, 48, 48), GameMaterials.OBSIDIAN)
	var boiled := (water - 5 * 8 - w.count_liquid_eighths(GameMaterials.WATER)) / 8
	assert_int(obsidian).override_failure_message("each lava cell crusts once at most").is_less_equal(5)
	# about six boils per crust: a crust at 0.4 x 0.25 against a boil at 0.6; five cells make a run of one unlucky roll rare
	assert_int(boiled).override_failure_message("it boiled little water: %d cells, %d obsidian" % [boiled, obsidian]).is_greater_equal(2 * obsidian + 5)
	assert_int(w.count_in_box(Vector3i(0, 0, 0), Vector3i(48, 48, 48), GameMaterials.LAVA)).is_equal(0)


func test_lone_still_speck_wakes_and_goes_while_a_body_sleeps() -> void:
	var w := _world()
	# a still body of 4 x 2 x 4 on a stone floor (no air under it, no fresh opening) and one speck on a ledge
	var cells := {}
	for x in range(0, 16):
		for z in range(0, 16):
			cells[Vector3i(x, 0, z)] = GameMaterials.STONE
	for x in range(2, 6):
		for z in range(2, 6):
			cells[Vector3i(x, 1, z)] = GameMaterials.WATER_STATIC
			cells[Vector3i(x, 2, z)] = GameMaterials.WATER_STATIC
	cells[Vector3i(12, 1, 12)] = GameMaterials.WATER_STATIC   # the speck
	_load_cells(w, cells)
	_run(w, 400)
	assert_int(w.count_in_box(Vector3i(2, 1, 2), Vector3i(6, 3, 6), GameMaterials.WATER_STATIC)).override_failure_message("the body woke").is_equal(32)
	assert_int(w.count_in_box(Vector3i(6, 1, 6), Vector3i(16, 16, 16), GameMaterials.WATER_STATIC)).override_failure_message("the speck is still there").is_equal(0)
	assert_int(int(w.get_stats()["still_woken"])).is_equal(1)


func test_sample_cells_flags_moving_cells_and_fresh_gas() -> void:
	var w := _world()
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 4, 48), GameMaterials.STONE)
	w.fill_box(Vector3i(20, 24, 20), Vector3i(24, 28, 24), GameMaterials.SAND)   # falls
	w.fill_box(Vector3i(8, 4, 8), Vector3i(12, 5, 12), GameMaterials.SAND)       # rests (one flat layer cannot slump)
	_run(w, 3)   # past the edit stamps: only what still moves counts
	w.fill_box(Vector3i(30, 10, 30), Vector3i(34, 14, 34), GameMaterials.STEAM)   # just born
	var mask := PackedByteArray()
	mask.resize(256)
	mask[GameMaterials.SAND] = 1
	mask[GameMaterials.STEAM] = 1
	var r := w.sample_cells(Vector3i(24, 12, 24), 24, 40000, 3, mask)
	var moving := 0
	var resting := 0
	var fresh := 0
	for i in range(0, r.size(), 7):
		if r[i + 3] == GameMaterials.STEAM:
			fresh += 1 if r[i + 6] & 16 else 0
		elif r[i + 6] & 8:
			moving += 1
			assert_int(r[i + 1]).override_failure_message("a resting cell flagged moving").is_greater(6)
		else:
			resting += 1
	assert_int(moving).is_greater(0)
	assert_int(resting).is_greater(0)
	assert_int(fresh).is_greater(0)


## Smoke lives 11 s and rises fast (bug report 2026-10-27 18-11-32): its 330-tick lifetime is past a byte, so the
## kernel counts it down every other tick; a puff in a closed box is still there at 300 ticks and gone by 340.
func test_smoke_outlives_a_byte_of_ticks() -> void:
	var w := _world()
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 48, 48), GameMaterials.STONE)
	w.fill_box(Vector3i(8, 8, 8), Vector3i(40, 40, 40), 0)
	w.fill_box(Vector3i(20, 10, 20), Vector3i(22, 12, 22), GameMaterials.SMOKE)
	_run(w, 300)
	assert_int(w.count_material(GameMaterials.SMOKE)).is_greater(0)
	_run(w, 40)
	assert_int(w.count_material(GameMaterials.SMOKE)).is_equal(0)


## A puff in open air is near the top of a 6 m box within 2 s (at the old pace it reached about half as high).
func test_smoke_rises_fast() -> void:
	var w := _world()
	w.fill_box(Vector3i(20, 2, 20), Vector3i(22, 4, 22), GameMaterials.SMOKE)
	_run(w, 60)
	assert_int(w.count_in_box(Vector3i(0, 40, 0), Vector3i(48, 48, 48), GameMaterials.SMOKE)).is_greater(4)
