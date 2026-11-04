extends GdUnitTestSuite
## ADR 0137 (fluid lag): loaded chunks take the outside temperature, read down to whole degrees (3.6 C is under ice's
## melt_c of 4), a cold window wakes no chunk that holds nothing to freeze, and warm ice melts a few exposed cells
## per heat turn. Speed and the unchanged kernel results:
## tools/fluid_bench.gd (reload, cold) and tools/sim_equiv_probe.gd.


func _world() -> SimWorld:
	var w := SimWorld.new()
	w.configure(Vector3i(4, 2, 4), 2, 7)
	w.set_materials(GameMaterials.defs())
	w.set_reactions(GameMaterials.reactions())
	w.mark_all_loaded(true)
	w.set_liquid_fullness(true)
	w.set_states(true)
	return w


func _bytes(material: int) -> PackedByteArray:
	var b := PackedByteArray()
	b.resize(16 * 16 * 16 * 2)
	for i in range(0, b.size(), 2):
		b.encode_u16(i, material)
	return b


func test_ice_loaded_under_its_melting_point_stays_ice() -> void:
	var w := _world()   # the field starts at 18 C, as a recycled chunk slot did
	w.set_ambient_c(3.6)
	w.set_chunk_bytes(Vector3i(1, 0, 1), _bytes(GameMaterials.ICE), 2)
	for t in 120:
		w.tick(1)
	assert_int(w.count_material(GameMaterials.WATER)).is_equal(0)
	assert_int(w.count_material(GameMaterials.ICE)).is_equal(4096)


func test_a_cold_window_leaves_chunks_with_nothing_to_freeze_asleep() -> void:
	var w := _world()
	w.set_ambient_c(-5.0, true)
	w.fill_box(Vector3i(0, 0, 0), Vector3i(64, 16, 64), GameMaterials.STONE)
	w.fill_box(Vector3i(8, 16, 8), Vector3i(12, 18, 12), GameMaterials.WATER)   # a puddle that does freeze
	var scanned := 0
	for t in 200:
		var s: Dictionary = w.tick(1)
		if t >= 100:
			scanned += int(s.get("scanned_cells_per_tick", 0))
	assert_int(w.count_material(GameMaterials.WATER)).is_equal(0)   # froze
	assert_int(scanned).is_equal(0)   # then nothing wakes: stone and ice have nothing left to do at -5 C


func test_warm_ice_thins_from_its_top_over_minutes() -> void:
	var w := _world()
	w.set_ambient_c(10.0, true)
	w.fill_box(Vector3i(0, 0, 0), Vector3i(64, 8, 64), GameMaterials.STONE)
	w.fill_box(Vector3i(8, 8, 8), Vector3i(56, 11, 56), GameMaterials.ICE)   # 6 x 6 m, 3 cells thick, on rock
	var ice := 48 * 48 * 3
	var top := Vector3i(8, 10, 8)
	var bottom := Vector3i(8, 8, 8)
	for t in 30:
		w.tick(1)
	assert_int(w.count_material(GameMaterials.ICE)).override_failure_message("melted at once").is_greater(ice * 9 / 10)
	for t in 870:
		w.tick(1)
	var left := w.count_material(GameMaterials.ICE)   # 30 s at 10 C (old: none after a tick)
	assert_int(left).is_less(ice * 9 / 10)
	assert_int(left).is_greater(ice / 3)
	assert_int(w.count_in_box(top, top + Vector3i(48, 1, 48), GameMaterials.ICE)).is_less(w.count_in_box(bottom, bottom + Vector3i(48, 1, 48), GameMaterials.ICE))


func test_water_against_a_roof_beside_a_deeper_column_sleeps() -> void:
	var w := _world()   # the lake case: a hole dug into a bank under the surface fills to its roof
	w.set_column_flow(true)
	w.fill_box(Vector3i(0, 0, 0), Vector3i(64, 24, 64), GameMaterials.STONE)
	w.fill_box(Vector3i(20, 4, 20), Vector3i(28, 24, 28), 0)   # a shaft open to the sky
	w.fill_box(Vector3i(28, 4, 20), Vector3i(36, 10, 28), 0)   # a tunnel off it with a roof at 10
	w.fill_box(Vector3i(20, 4, 20), Vector3i(28, 18, 28), GameMaterials.WATER)
	w.fill_box(Vector3i(28, 4, 20), Vector3i(36, 10, 28), GameMaterials.WATER)
	var moved := 0
	for t in 300:
		var s: Dictionary = w.tick(1)
		if t >= 280:
			moved += int(s.get("moved_cells_per_tick", 0))
	assert_int(moved).override_failure_message("water at rest kept moving %d cells" % moved).is_equal(0)
	assert_int(w.count_liquid_eighths(GameMaterials.WATER)).is_equal((8 * 8 * 14 + 8 * 8 * 6) * 8)
