extends GdUnitTestSuite

## Kernel v6 (P10): static ground shaken loose by nearby changes (Noita), finite still water (ADR 0068),
## background ticks, level of detail and the per-tick cell budget.

const N := 3


func _world() -> SimWorld:
	var w := SimWorld.new()
	w.configure(Vector3i(N, N, N), 2, 5)
	w.set_materials(_no_films(GameMaterials.defs()))
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


## Water of either kind (moving, or still: generated water that nothing has disturbed yet).
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


## Still water (ADR 0068): generated water sleeps until a fresh opening touches it; then it is finite moving water.
## A lake whose shore breaks drains onto the floor past it, its level drops, and the world holds exactly as much
## water afterwards as before (no reservoir, no pouring source).
func test_lake_drains_through_a_breach_and_keeps_its_mass() -> void:
	var w := _game_world()
	_load_slab(w, Vector3i(0, 0, 0), 1, 6, GameMaterials.WATER_STATIC)   # a generated lake at the edge
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 1, 48), GameMaterials.STONE)   # its bed
	w.fill_box(Vector3i(16, 1, 0), Vector3i(17, 6, 16), GameMaterials.STONE)   # its shore (edit, then removed)
	_run(w, 3)
	var mass := w.count_liquid_eighths(GameMaterials.WATER_STATIC) + w.count_liquid_eighths(GameMaterials.WATER)
	w.fill_box(Vector3i(16, 1, 0), Vector3i(17, 6, 16), 0)    # break the shore
	assert_int(_settle(w, 3000)).override_failure_message("never settled").is_greater(0)
	# the floor past the shore floods, never above the lake's level
	assert_int(_wet(w, Vector3i(17, 1, 0), Vector3i(48, 6, 48))).is_greater(50)
	assert_int(_wet(w, Vector3i(0, 6, 0), Vector3i(48, 48, 48))).is_equal(0)
	# the lake gave what spilled: fewer still cells, the same water in all
	assert_int(w.count_in_box(Vector3i(0, 1, 0), Vector3i(16, 6, 16), GameMaterials.WATER_STATIC)).is_less(16 * 5 * 16)
	assert_int(w.count_liquid_eighths(GameMaterials.WATER_STATIC) + w.count_liquid_eighths(GameMaterials.WATER)).is_equal(mass)
	assert_int(int(w.get_stats()["still_woken"])).is_greater(0)


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
	assert_int(_settle(w, 1500)).override_failure_message("never settled").is_greater(0)
	# the pocket took water from the pond, no more and no less of it in all; what lies under the pocket's floor sleeps on
	assert_int(w.count_material(GameMaterials.WATER_STATIC)).is_less(16 * 16 * 8)
	assert_int(w.count_liquid_eighths(GameMaterials.WATER) + w.count_liquid_eighths(GameMaterials.WATER_STATIC)).is_equal(mass)
	assert_int(w.count_in_box(Vector3i(24, 6, 12), Vector3i(28, 11, 16), GameMaterials.WATER)).is_greater(12)
	assert_int(int(w.get_stats()["still_woken"])).is_greater(0)


## The developer's 3x3x3 pocket (bug report 2026-10-25 02-17-32): water walled off from a lake is a small body; dug
## free, it spills what it holds and no more (ADR 0068: every still body does).
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


## ADR 0068: water poured on a lake stays what it is (finite moving water at rest on the still body), the lake
## keeps every still cell, and nothing of the poured water is lost or made.
func test_water_poured_on_a_lake_rests_on_it() -> void:
	var w := _world()
	w.set_liquid_fullness(true)
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 6, 48), GameMaterials.STONE)   # the banks
	_load_slab(w, Vector3i(0, 0, 0), 1, 6, GameMaterials.WATER_STATIC)
	_load_slab(w, Vector3i(1, 0, 0), 1, 6, GameMaterials.WATER_STATIC)
	w.fill_box(Vector3i(0, 0, 0), Vector3i(32, 1, 16), GameMaterials.STONE)   # its bed
	w.fill_box(Vector3i(10, 8, 4), Vector3i(20, 12, 10), GameMaterials.WATER)   # a water bomb over it
	_run(w, 600)
	assert_int(w.count_liquid_eighths(GameMaterials.WATER)).is_equal(10 * 4 * 6 * 8)
	assert_int(w.count_material(GameMaterials.WATER_STATIC)).is_equal(2 * 16 * 5 * 16)
	assert_int(_wet(w, Vector3i(0, 7, 0), Vector3i(48, 48, 48))).is_equal(0)   # spread flat, no heap


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


## After a reload, generated water beside air the generator left (stamp 0) must not move: only a fresh opening wakes it.
func test_loaded_water_stays_where_it_is() -> void:
	var w := _world()
	w.set_liquid_fullness(true)
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 1, 48), GameMaterials.STONE)
	_load_slab(w, Vector3i(0, 0, 0), 1, 6, GameMaterials.WATER_STATIC)   # beside open floor, as loaded
	_run(w, 120)
	assert_int(w.count_material(GameMaterials.WATER_STATIC)).is_equal(16 * 5 * 16)


## Part D's minimap feed: the top cell that is not air, gas or fire, packed with the depth of a liquid top.
func test_surface_map_packs_the_top_cell() -> void:
	var w := _world()
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 10, 48), GameMaterials.STONE)
	w.fill_box(Vector3i(4, 10, 4), Vector3i(8, 13, 8), GameMaterials.WATER)   # a pool 3 deep
	w.fill_box(Vector3i(20, 10, 20), Vector3i(21, 14, 21), GameMaterials.STEAM)   # gas is passed over
	var m: PackedInt32Array = w.surface_map(Vector2i(0, 0), Vector2i(12, 12), 4, 100)
	assert_int(m.size()).is_equal(144)
	var pool: int = m[1 + 12 * 1]   # column (4, 4)
	assert_int(pool & 255).is_equal(GameMaterials.WATER)
	assert_int((pool >> 8) & 255).is_equal(3)
	assert_int(pool >> 16).is_equal(12 + 100)
	var rock: int = m[5 + 12 * 5]   # column (20, 20): stone under the steam
	assert_int(rock & 255).is_equal(GameMaterials.STONE)
	assert_int(rock >> 16).is_equal(9 + 100)
	assert_int(w.surface_map(Vector2i(60, 0), Vector2i(1, 1), 1, 0)[0]).is_equal(0)   # outside the window


## These tests count exact mass: films drying up (ADR 0069 B3) loses mass on purpose, so they run without it.
static func _no_films(defs: Array) -> Array:
	var out := []
	for d in defs:
		var c: Dictionary = d.duplicate()
		c.erase("film_lifetime_ticks")
		out.append(c)
	return out
