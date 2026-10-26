extends GdUnitTestSuite

## ADR 0069 part B: liquids with liquid_speed > 1 spread in extra substeps per tick (water 3, oil and blood and
## sludge 2, lava 1), falling stays with the main pass, a surface evens out by half the difference (ends flat),
## and mass is kept through all of it.

const N := 3


func _world(defs: Array) -> SimWorld:
	var w := SimWorld.new()
	w.configure(Vector3i(N, N, N), 2, 5)
	w.set_materials(defs)
	w.set_reactions(GameMaterials.reactions())
	w.mark_all_loaded(true)
	w.set_liquid_fullness(true)
	w.set_cascade(true)
	w.set_gravity(0.17)
	return w


## The game's materials with water's liquid_speed overridden.
func _defs_with_water_speed(speed: int) -> Array:
	var out := []
	for d in GameMaterials.defs():
		var c: Dictionary = d.duplicate()
		if c["name"] == "water":
			c["liquid_speed"] = speed
		out.append(c)
	return out


## Horizontal extent (cells along x) of a material on a floor level.
func _extent_x(w: SimWorld, mat: int, y: int) -> int:
	var lo := 48
	var hi := -1
	for x in 48:
		if w.count_in_box(Vector3i(x, y, 0), Vector3i(x + 1, y + 1, 48), mat) > 0:
			lo = mini(lo, x)
			hi = maxi(hi, x)
	return hi - lo + 1 if hi >= 0 else 0


func _run(w: SimWorld, ticks: int) -> void:
	for t in ticks:
		w.tick(1)


## Ticks until nothing moved for 20 ticks in a row, -1 if never.
func _settle(w: SimWorld, limit: int) -> int:
	var quiet := 0
	for t in limit:
		var r: Dictionary = w.tick(1)
		quiet = quiet + 1 if int(r["moved_cells_per_tick"]) == 0 else 0
		if quiet >= 20:
			return t
	return -1


## A 4x4x4 block of water let go on a floor: with three substeps it has spread much wider after two ticks than
## with one, and holds exactly the same water.
func test_water_with_three_substeps_spreads_faster_and_keeps_its_mass() -> void:
	var extents := {}
	for speed in [1, 3]:
		var w := _world(_defs_with_water_speed(speed))
		w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 8, 48), GameMaterials.STONE)
		w.fill_box(Vector3i(22, 8, 22), Vector3i(26, 12, 26), GameMaterials.WATER)
		var mass := w.count_liquid_eighths(GameMaterials.WATER)
		_run(w, 2)
		extents[speed] = _extent_x(w, GameMaterials.WATER, 8)
		assert_int(w.count_liquid_eighths(GameMaterials.WATER)).override_failure_message("mass at speed %d" % speed).is_equal(mass)
	assert_int(extents[3]).override_failure_message("extents %s" % str(extents)).is_greater_equal(extents[1] + 6)


## Lava keeps one substep: the same block of lava spreads far less than water in the same ticks.
func test_lava_stays_slow() -> void:
	var w := _world(GameMaterials.defs())
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 8, 48), GameMaterials.STONE)
	w.fill_box(Vector3i(10, 8, 22), Vector3i(14, 12, 26), GameMaterials.LAVA)
	w.fill_box(Vector3i(34, 8, 22), Vector3i(38, 12, 26), GameMaterials.WATER)
	_run(w, 6)
	var lava_w := 0
	var water_w := 0
	for x in 24:
		if w.count_in_box(Vector3i(x, 8, 0), Vector3i(x + 1, 9, 48), GameMaterials.LAVA) > 0:
			lava_w += 1
		if w.count_in_box(Vector3i(x + 24, 8, 0), Vector3i(x + 25, 9, 48), GameMaterials.WATER) > 0:
			water_w += 1
	assert_int(water_w).override_failure_message("water %d lava %d" % [water_w, lava_w]).is_greater(lava_w + 4)


## Substeps never move a falling cell sideways: a column of water let go in the air lands exactly as it does with
## one substep (the main pass alone decides where falling water goes).
func test_substeps_do_not_drift_falling_water() -> void:
	var columns := {}
	for speed in [1, 3]:
		var w := _world(_defs_with_water_speed(speed))
		w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 4, 48), GameMaterials.STONE)
		w.fill_box(Vector3i(24, 30, 24), Vector3i(25, 34, 25), GameMaterials.WATER)   # 4 cells, 26 cells up
		_run(w, 4)   # still in the air (gravity 0.17 cells/tick^2)
		columns[speed] = w.count_in_box(Vector3i(24, 4, 24), Vector3i(25, 34, 25), GameMaterials.WATER)
	assert_int(columns[3]).override_failure_message("in the column: %s" % str(columns)).is_equal(columns[1])


## A 2 m pour on a flat floor ends within one eighth of flat, in under two seconds (60 ticks) at 30 Hz.
func test_pour_levels_to_the_mean_and_rests() -> void:
	var w := _world(GameMaterials.defs())
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 8, 48), GameMaterials.STONE)
	w.fill_box(Vector3i(16, 8, 16), Vector3i(32, 9, 32), GameMaterials.WATER)   # one layer, 16 x 16 cells
	w.fill_box(Vector3i(22, 9, 22), Vector3i(26, 11, 26), GameMaterials.WATER)   # a heap on it: 4 x 4 x 2
	var mass := w.count_liquid_eighths(GameMaterials.WATER)
	var t := _settle(w, 60)
	assert_int(t).override_failure_message("not at rest within 60 ticks").is_greater(0)
	assert_int(w.count_liquid_eighths(GameMaterials.WATER)).is_equal(mass)
	# flat: nothing left on the second layer, and surface cells differ by at most one eighth from their neighbours
	assert_int(w.count_in_box(Vector3i(0, 10, 0), Vector3i(48, 48, 48), GameMaterials.WATER)).is_equal(0)
	var levels := {}
	for cz in N:
		for cx in N:
			var b: PackedByteArray = w.get_chunk_bytes(Vector3i(cx, 0, cz), 2)   # terrain values: partial cells carry their eighths
			for z in 16:
				for x in 16:
					var v := b.decode_u16((9 + 16 * (x + 16 * z)) * 2)
					if v != 0:
						levels[Vector2i(cx * 16 + x, cz * 16 + z)] = GameMaterials.eighths_of_voxel(v)
	var worst := 0
	for k in levels:
		for d in [Vector2i(1, 0), Vector2i(0, 1)]:
			if levels.has(k + d):
				worst = maxi(worst, absi(levels[k] - levels[k + d]))
	assert_int(worst).override_failure_message("surface steps of %d eighths" % worst).is_less_equal(1)
