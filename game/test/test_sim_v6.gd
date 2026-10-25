extends GdUnitTestSuite

## Kernel v6 (P10): static ground shaken loose by nearby changes (Noita), reservoirs (ADR 0045),
## background ticks, level of detail and the per-tick cell budget.

const N := 3


func _world() -> SimWorld:
	var w := SimWorld.new()
	w.configure(Vector3i(N, N, N), 2, 5)
	w.set_materials(GameMaterials.defs())
	w.set_reactions(GameMaterials.reactions())
	w.mark_all_loaded(true)
	return w


## Loads a chunk "from the world" (no edit stamps): cells in [y0, y1) are mat.
func _load_slab(w: SimWorld, chunk: Vector3i, y0: int, y1: int, mat: int) -> void:
	var b := PackedByteArray()
	b.resize(4096 * 2)
	for z in 16:
		for x in 16:
			for y in range(y0, y1):
				b[(y + 16 * (x + 16 * z)) * 2] = mat
	w.set_chunk_bytes(chunk, b, 2)


## As SimBubble runs it: liquids in eighths, columns falling together, gravity.
func _game_world() -> SimWorld:
	var w := _world()
	w.set_liquid_fullness(true)
	w.set_cascade(true)
	w.set_gravity(0.17)
	return w


## Water of either kind (moving, or still: filled water at rest beside a big body joins it, ADR 0049).
func _wet(w: SimWorld, lo: Vector3i, hi: Vector3i) -> int:
	return w.count_in_box(lo, hi, GameMaterials.WATER) + w.count_in_box(lo, hi, GameMaterials.WATER_STATIC)


func _run(w: SimWorld, ticks: int) -> void:
	for i in ticks:
		w.tick(1)


func test_loaded_overhang_stays() -> void:
	var w := _world()
	_load_slab(w, Vector3i(1, 1, 1), 8, 16, GameMaterials.DIRT)   # dirt ceiling over air, as generated
	_run(w, 60)
	assert_int(w.count_in_box(Vector3i(16, 24, 16), Vector3i(32, 32, 32), GameMaterials.DIRT)).is_equal(16 * 16 * 8)


func test_undermined_dirt_collapses() -> void:
	var w := _world()
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 16, 48), GameMaterials.STONE)
	_load_slab(w, Vector3i(1, 1, 1), 8, 16, GameMaterials.DIRT)
	# dig a hole into the ceiling from below: the dirt above it has nothing under it now
	for x in range(20, 28):
		for z in range(20, 28):
			w.set_voxel(Vector3i(x, 24, z), 0)
	_run(w, 120)
	# the column above the hole came down onto the stone floor
	assert_int(w.count_in_box(Vector3i(20, 25, 20), Vector3i(28, 32, 28), GameMaterials.DIRT)).is_less(8 * 8 * 7 / 2)
	assert_int(w.count_in_box(Vector3i(16, 16, 16), Vector3i(32, 20, 32), GameMaterials.DIRT)).is_greater(0)


func test_dirt_wall_holds_sand_slumps() -> void:
	var w := _world()
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 1, 48), GameMaterials.STONE)
	# two static blocks with a trench dug next to each: dirt keeps its wall, natural sand erodes into a slope
	w.fill_box(Vector3i(4, 1, 4), Vector3i(16, 12, 16), GameMaterials.DIRT)
	w.fill_box(Vector3i(28, 1, 4), Vector3i(40, 12, 16), GameMaterials.SAND_STATIC)
	_run(w, 3)
	w.fill_box(Vector3i(4, 1, 16), Vector3i(16, 12, 20), 0)    # fresh trench along the dirt
	w.fill_box(Vector3i(28, 1, 16), Vector3i(40, 12, 20), 0)   # and along the sand
	_run(w, 300)
	assert_int(w.count_in_box(Vector3i(4, 1, 16), Vector3i(16, 12, 20), GameMaterials.DIRT)).is_equal(0)
	assert_int(w.count_in_box(Vector3i(28, 1, 16), Vector3i(40, 12, 24), GameMaterials.SAND_STATIC)).is_greater(20)


## Still water (ADR 0045, 0049): a big body (64 m^3 or more, or one that runs past the window) keeps its level and
## pours moving water into what is opened; a small one turns into moving water and drains. A lake touching the
## world's edge counts as big (it goes on out there).
func test_big_lake_fills_a_breach_and_stays_full() -> void:
	var w := _game_world()
	_load_slab(w, Vector3i(0, 0, 0), 1, 6, GameMaterials.WATER_STATIC)   # a generated lake at the edge
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 1, 48), GameMaterials.STONE)   # its bed
	w.fill_box(Vector3i(16, 1, 0), Vector3i(17, 6, 16), GameMaterials.STONE)   # its shore (edit, then removed)
	_run(w, 3)
	w.fill_box(Vector3i(16, 1, 0), Vector3i(17, 6, 16), 0)    # break the shore
	_run(w, 240)
	assert_int(w.count_in_box(Vector3i(0, 1, 0), Vector3i(16, 6, 16), GameMaterials.WATER_STATIC)).is_equal(16 * 5 * 16)
	# the floor past the shore floods, never above the lake's level
	assert_int(_wet(w, Vector3i(17, 1, 0), Vector3i(48, 6, 48))).is_greater(50)
	assert_int(_wet(w, Vector3i(0, 6, 0), Vector3i(48, 48, 48))).is_equal(0)
	assert_int(int(w.get_stats()["bodies_measured"])).is_greater(0)


## Ticks until nothing moved for 20 ticks in a row (the pour rolls each tick), -1 if never.
func _settle(w: SimWorld, limit: int) -> int:
	var quiet := 0
	for t in limit:
		var r: Dictionary = w.tick(1)
		quiet = quiet + 1 if int(r["moved_cells_per_tick"]) == 0 else 0
		if quiet >= 20:
			return t
	return -1


func test_small_pond_drains_into_a_pocket() -> void:
	var w := _game_world()
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 12, 48), GameMaterials.STONE)
	w.fill_box(Vector3i(8, 4, 8), Vector3i(24, 12, 24), GameMaterials.WATER_STATIC)   # a pond 8 deep: 2048 cells, small
	_run(w, 3)
	var mass := w.count_liquid_eighths(GameMaterials.WATER_STATIC)
	w.fill_box(Vector3i(24, 6, 12), Vector3i(28, 12, 16), 0)   # a pocket dug into the bank, below the surface
	assert_int(_settle(w, 900)).override_failure_message("never settled").is_greater(0)
	# the whole pond is moving water now, no more and no less of it, and the pocket filled from it
	assert_int(w.count_material(GameMaterials.WATER_STATIC)).is_equal(0)
	assert_int(w.count_liquid_eighths(GameMaterials.WATER)).is_equal(mass)
	assert_int(w.count_in_box(Vector3i(24, 6, 12), Vector3i(28, 11, 16), GameMaterials.WATER)).is_equal(4 * 5 * 4)
	assert_int(int(w.get_stats()["bodies_drained"])).is_equal(1)


func test_big_pond_refills_a_pocket_with_moving_water() -> void:
	var w := _game_world()
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 12, 48), GameMaterials.STONE)
	w.fill_box(Vector3i(0, 4, 0), Vector3i(24, 12, 24), GameMaterials.WATER_STATIC)   # runs to the world's edge: big
	_run(w, 3)
	w.fill_box(Vector3i(24, 6, 12), Vector3i(28, 12, 16), 0)
	assert_int(_settle(w, 900)).override_failure_message("never settled").is_greater(0)
	assert_int(w.count_in_box(Vector3i(0, 4, 0), Vector3i(24, 12, 24), GameMaterials.WATER_STATIC)).is_equal(24 * 8 * 24)
	assert_int(_wet(w, Vector3i(24, 6, 12), Vector3i(28, 12, 16))).is_equal(4 * 6 * 4)


## The developer's 3x3x3 pocket (bug report 2026-10-25 02-17-32): water walled off from a lake is a small body; dug
## free, it spills what it holds and no more.
func test_walled_off_pocket_is_finite() -> void:
	var w := _game_world()
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 20, 48), GameMaterials.STONE)
	w.fill_box(Vector3i(20, 8, 20), Vector3i(32, 20, 32), GameMaterials.WATER_STATIC)   # 12^3, open on top
	_run(w, 3)
	w.fill_box(Vector3i(16, 1, 16), Vector3i(36, 8, 36), 0)   # a big room dug out under it
	assert_int(_settle(w, 1500)).override_failure_message("never settled").is_greater(0)
	assert_int(w.count_material(GameMaterials.WATER_STATIC)).is_equal(0)
	assert_int(w.count_liquid_eighths(GameMaterials.WATER)).is_equal(12 * 12 * 12 * 8)


## Sand sinks through still water to the bed (bug report 02-17-49); the lake keeps its level.
func test_sand_sinks_through_a_lake() -> void:
	var w := _game_world()
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 1, 48), GameMaterials.STONE)
	w.fill_box(Vector3i(0, 1, 0), Vector3i(48, 12, 48), GameMaterials.WATER_STATIC)
	_run(w, 3)
	w.fill_box(Vector3i(20, 16, 20), Vector3i(24, 20, 24), GameMaterials.SAND)   # a sand bomb over the lake
	_run(w, 200)
	assert_int(w.count_in_box(Vector3i(0, 12, 0), Vector3i(48, 48, 48), GameMaterials.SAND)).is_equal(0)
	assert_int(w.count_in_box(Vector3i(0, 1, 0), Vector3i(48, 4, 48), GameMaterials.SAND)).is_greater(32)
	assert_int(w.count_in_box(Vector3i(0, 11, 0), Vector3i(48, 12, 48), GameMaterials.WATER_STATIC)).is_equal(48 * 48)


## Lava pours as a stream: gravity never waits for its viscosity roll (bug report 02-23-24).
func test_lava_falls_every_tick() -> void:
	var w := _game_world()
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 1, 48), GameMaterials.STONE)
	w.fill_box(Vector3i(20, 40, 20), Vector3i(21, 46, 21), GameMaterials.LAVA)   # a column of 6 cells
	_run(w, 14)
	# under gravity a cell falls 39 cells in about 20 ticks; with the old roll some cells lagged behind
	var hist: PackedInt32Array = w.layer_histogram(GameMaterials.LAVA)
	var top := -1
	for y in hist.size():
		if hist[y] > 0:
			top = y
	assert_int(top).is_less(36)


func test_river_steps_stay_quiet() -> void:
	var w := _world()
	w.set_liquid_fullness(true)
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 8, 48), GameMaterials.STONE)   # the banks
	# a river surface coming down one cell: air over the lower stretch beside the upper one
	_load_slab(w, Vector3i(0, 0, 0), 1, 6, GameMaterials.WATER_STATIC)
	_load_slab(w, Vector3i(1, 0, 0), 1, 5, GameMaterials.WATER_STATIC)
	w.fill_box(Vector3i(0, 0, 0), Vector3i(32, 1, 16), GameMaterials.STONE)   # its bed (the slabs left y 0 empty)
	var r: Dictionary = {}
	for t in 60:
		r = w.tick(1)
	assert_int(w.count_material(GameMaterials.WATER)).is_equal(0)
	assert_int(w.count_in_box(Vector3i(16, 5, 0), Vector3i(32, 6, 16), GameMaterials.WATER_STATIC)).is_equal(0)
	assert_int(int(r["moved_cells_per_tick"])).is_equal(0)


func test_water_poured_on_a_lake_joins_it() -> void:
	var w := _world()
	w.set_liquid_fullness(true)
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 6, 48), GameMaterials.STONE)   # the banks
	_load_slab(w, Vector3i(0, 0, 0), 1, 6, GameMaterials.WATER_STATIC)
	_load_slab(w, Vector3i(1, 0, 0), 1, 6, GameMaterials.WATER_STATIC)
	w.fill_box(Vector3i(0, 0, 0), Vector3i(32, 1, 16), GameMaterials.STONE)   # its bed
	w.fill_box(Vector3i(10, 8, 4), Vector3i(20, 12, 10), GameMaterials.WATER)   # a water bomb over it
	_run(w, 120)
	assert_int(w.count_material(GameMaterials.WATER)).is_equal(0)


func test_background_tick_matches_inline_tick() -> void:
	var a := _world()
	var b := _world()
	for w in [a, b]:
		w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 1, 48), GameMaterials.STONE)
		w.fill_box(Vector3i(10, 20, 10), Vector3i(30, 30, 30), GameMaterials.SAND)
	for i in 40:
		a.tick(1)
		assert_bool(b.start_tick()).is_true()
		var st: Dictionary = b.finish_tick()
		assert_bool(b.is_tick_done()).is_true()
		assert_int(int(st["ticks"])).is_equal(1)
	assert_object(a.layer_histogram(GameMaterials.SAND)).is_equal(b.layer_histogram(GameMaterials.SAND))


func test_budget_and_lod_delay_but_never_lose_work() -> void:
	var a := _world()
	var b := _world()
	for w in [a, b]:
		w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 1, 48), GameMaterials.STONE)
		w.fill_box(Vector3i(2, 30, 2), Vector3i(46, 40, 46), GameMaterials.SAND)
	b.set_focus(Vector3i(0, 0, 0))
	b.set_lod(1, 3)
	b.set_cell_budget(4096 * 3)
	var deferred := 0.0
	for i in 600:
		a.tick(1)
		deferred += float(b.tick(1)["deferred_chunks_per_tick"])
	assert_float(deferred).is_greater(0.0)
	# same amount of sand, all of it settled on the floor in both worlds
	assert_int(b.count_material(GameMaterials.SAND)).is_equal(a.count_material(GameMaterials.SAND))
	assert_int(b.count_in_box(Vector3i(0, 20, 0), Vector3i(48, 48, 48), GameMaterials.SAND)).is_equal(0)


func test_dirty_boxes_cover_every_changed_cell() -> void:
	var w := _world()
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 1, 48), GameMaterials.STONE)
	w.fill_box(Vector3i(12, 18, 12), Vector3i(20, 26, 20), GameMaterials.SAND)   # straddles chunk borders
	w.fill_box(Vector3i(30, 1, 30), Vector3i(34, 5, 34), GameMaterials.WATER)
	w.clear_dirty()
	# a single edit reports exactly its own cell
	w.set_voxel(Vector3i(40, 40, 40), GameMaterials.STONE)
	var one: PackedInt32Array = w.get_dirty_boxes()
	assert_array(Array(one)).is_equal([2, 2, 2, 8, 8, 8, 8, 8, 8])
	w.clear_dirty()
	assert_int(w.get_dirty_boxes().size()).is_equal(0)
	for step in 12:
		var before := {}
		for cz in N:
			for cy in N:
				for cx in N:
					before[Vector3i(cx, cy, cz)] = w.get_chunk_bytes(Vector3i(cx, cy, cz), 1)
		w.clear_dirty()
		w.tick(1)
		var boxes := {}
		var b: PackedInt32Array = w.get_dirty_boxes()
		for k in range(0, b.size(), 9):
			boxes[Vector3i(b[k], b[k + 1], b[k + 2])] = [Vector3i(b[k + 3], b[k + 4], b[k + 5]), Vector3i(b[k + 6], b[k + 7], b[k + 8])]
		for c in before:
			var now: PackedByteArray = w.get_chunk_bytes(c, 1)
			var old: PackedByteArray = before[c]
			for i in 4096:
				if now[i] == old[i]:
					continue
				# chunk byte order: y fastest, then x, then z
				var cell := Vector3i((i / 16) % 16, i % 16, i / 256)
				assert_bool(boxes.has(c)).override_failure_message("step %d: chunk %s changed but not listed" % [step, c]).is_true()
				var bx: Array = boxes[c]
				var inside: bool = cell.x >= bx[0].x and cell.y >= bx[0].y and cell.z >= bx[0].z and cell.x <= bx[1].x and cell.y <= bx[1].y and cell.z <= bx[1].z
				assert_bool(inside).override_failure_message("step %d: %s cell %s outside %s" % [step, c, cell, bx]).is_true()


func test_breach_into_a_valley_stops_pouring() -> void:
	var w := _game_world()
	_load_slab(w, Vector3i(0, 0, 0), 1, 6, GameMaterials.WATER_STATIC)
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 1, 48), GameMaterials.STONE)   # the bed (the slab left y 0 empty)
	w.fill_box(Vector3i(16, 1, 0), Vector3i(17, 6, 16), GameMaterials.STONE)
	_run(w, 3)
	w.fill_box(Vector3i(16, 1, 0), Vector3i(17, 6, 16), 0)   # the shore breaks onto the open floor
	_run(w, 900)
	# the lake keeps its level; what poured out is at most the budget of its breached face (16 x 5 cells)
	assert_int(w.count_in_box(Vector3i(0, 1, 0), Vector3i(16, 6, 16), GameMaterials.WATER_STATIC)).is_equal(16 * 5 * 16)
	var out := w.count_liquid_eighths(GameMaterials.WATER) + w.count_liquid_eighths(GameMaterials.WATER_STATIC) - 16 * 5 * 16 * 8
	assert_int(out).is_less_equal(16 * 5 * SimWorld.POUR_BUDGET * 8)
	assert_int(out).is_greater(16 * 5 * 8)


## After a reload, water that once filled an opening reads as generated: it must not creep further.
func test_loaded_water_stays_where_it_is() -> void:
	var w := _world()
	w.set_liquid_fullness(true)
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 1, 48), GameMaterials.STONE)
	_load_slab(w, Vector3i(0, 0, 0), 1, 6, GameMaterials.WATER_STATIC)   # beside open floor, as loaded
	_run(w, 120)
	assert_int(w.count_material(GameMaterials.WATER_STATIC)).is_equal(16 * 5 * 16)
