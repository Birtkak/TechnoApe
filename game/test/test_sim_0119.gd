extends GdUnitTestSuite

## ADR 0119 A: column flow. Moving liquid levels as columns by head, whole eighths, deterministic neighbour order:
## a pit fills flat and the sim goes quiet, a U-tube levels through its bottom, mass is exact, lava stays slow.

const N := 3


func _world(column := true) -> SimWorld:
	var w := SimWorld.new()
	w.configure(Vector3i(N, N, N), 2, 5)
	w.set_materials(GameMaterials.defs())
	w.set_reactions(GameMaterials.reactions())
	w.mark_all_loaded(true)
	w.set_liquid_fullness(true)
	w.set_column_flow(column)
	w.set_cascade(true)
	w.set_gravity(0.17)
	return w


func _run(w: SimWorld, ticks: int) -> void:
	for i in ticks:
		w.tick(1)


## Highest and lowest surface (in eighths) over a floor box, reading the liquid's top cell per column.
func _surface_range(w: SimWorld, x0: int, x1: int, z0: int, z1: int, y0: int, y1: int, mat: int) -> Vector2i:
	var lo := 1 << 30
	var hi := -1
	for z in range(z0, z1):
		for x in range(x0, x1):
			var h := 0
			for y in range(y0, y1):
				var v := w.get_voxel(Vector3i(x, y, z))
				if v == mat:
					h = 8 * (y + 1)
				elif v >= GameMaterials.FLUID_LEVEL_BASE and GameMaterials.material_of_voxel(v) == mat:
					h = 8 * y + (v - GameMaterials.FLUID_LEVEL_BASE) % 8 + 1
			lo = mini(lo, h)
			hi = maxi(hi, h)
	return Vector2i(lo, hi)


func _quiet_after(w: SimWorld, max_ticks: int) -> int:
	var still := 0
	for t in max_ticks:
		var s: Dictionary = w.tick(1)
		if int(s.get("moved", 1)) == 0:
			still += 1
			if still >= 10:
				return t - 9
		else:
			still = 0
	return -1


func test_a_block_of_water_in_a_basin_ends_flat_and_quiet() -> void:
	var w := _world()
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 8, 48), GameMaterials.STONE)
	w.fill_box(Vector3i(8, 8, 8), Vector3i(40, 20, 40), GameMaterials.STONE)
	w.fill_box(Vector3i(9, 8, 9), Vector3i(39, 20, 39), 0)   # a 30 x 30 basin, 12 deep
	w.fill_box(Vector3i(20, 8, 20), Vector3i(28, 16, 28), GameMaterials.WATER)   # an 8 x 8 x 8 block in its middle
	var mass := w.count_liquid_eighths(GameMaterials.WATER)
	var quiet := _quiet_after(w, 240)
	assert_int(quiet).override_failure_message("still moving after 240 ticks").is_between(1, 120)
	assert_int(w.count_liquid_eighths(GameMaterials.WATER)).override_failure_message("mass changed").is_equal(mass)
	var r := _surface_range(w, 9, 39, 9, 39, 8, 20, GameMaterials.WATER)
	# 512 cells over 900 columns: 4 eighths and a half each; flat means every column within one eighth of the rest
	assert_int(r.y - r.x).override_failure_message("surface from %d to %d eighths" % [r.x, r.y]).is_less_equal(1)
	assert_int(w.count_in_box(Vector3i(0, 16, 0), Vector3i(48, 48, 48), GameMaterials.WATER)).is_equal(0)


func test_a_u_tube_levels_through_its_bottom() -> void:
	var w := _world()
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 48, 48), GameMaterials.STONE)
	# two 2 x 2 shafts 10 cells apart joined by a 2-high passage along the floor
	w.fill_box(Vector3i(10, 4, 10), Vector3i(12, 30, 12), 0)
	w.fill_box(Vector3i(22, 4, 10), Vector3i(24, 30, 12), 0)
	w.fill_box(Vector3i(10, 4, 10), Vector3i(24, 6, 12), 0)
	w.fill_box(Vector3i(10, 6, 10), Vector3i(12, 26, 12), GameMaterials.WATER)   # the left arm full 20 high, the right empty
	var mass := w.count_liquid_eighths(GameMaterials.WATER)
	var quiet := _quiet_after(w, 600)
	assert_int(quiet).override_failure_message("still moving after 600 ticks").is_greater(0)
	assert_int(w.count_liquid_eighths(GameMaterials.WATER)).is_equal(mass)
	var left := _surface_range(w, 10, 12, 10, 12, 4, 30, GameMaterials.WATER)
	var right := _surface_range(w, 22, 24, 10, 12, 4, 30, GameMaterials.WATER)
	assert_int(absi(left.y - right.y)).override_failure_message("arms at %d and %d eighths" % [left.y, right.y]).is_less_equal(2)
	assert_int(right.y).override_failure_message("the right arm holds %d eighths of height" % right.y).is_greater(8 * 10)


func test_the_same_spill_lands_the_same_twice() -> void:
	var a := _world()
	var b := _world()
	for w in [a, b]:
		w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 8, 48), GameMaterials.STONE)
		w.fill_box(Vector3i(12, 8, 12), Vector3i(20, 16, 20), GameMaterials.STONE)   # a block to run off
		w.fill_box(Vector3i(13, 16, 13), Vector3i(19, 22, 19), GameMaterials.WATER)
		_run(w, 90)
	for z in range(0, 48, 3):
		for x in range(0, 48, 3):
			for y in range(8, 24, 2):
				var p := Vector3i(x, y, z)
				assert_int(a.get_voxel(p)).override_failure_message("differs at %s" % p).is_equal(b.get_voxel(p))


func test_lava_levels_slower_than_water() -> void:
	var spread := {}
	for mat in [GameMaterials.WATER, GameMaterials.LAVA]:
		var w := _world()
		w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 8, 48), GameMaterials.STONE)
		w.fill_box(Vector3i(20, 8, 20), Vector3i(24, 12, 24), mat)
		_run(w, 10)
		var n := 0
		for z in 48:
			for x in 48:
				var v := w.get_voxel(Vector3i(x, 8, z))
				if v == mat or (v >= GameMaterials.FLUID_LEVEL_BASE and GameMaterials.material_of_voxel(v) == mat):
					n += 1
		spread[mat] = n
	assert_int(spread[GameMaterials.LAVA]).override_failure_message("lava %d vs water %d columns" % [spread[GameMaterials.LAVA], spread[GameMaterials.WATER]]) \
		.is_less(spread[GameMaterials.WATER])
