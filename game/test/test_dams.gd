extends GdUnitTestSuite

## ADR 0154: the water wheel's flow count and the dam scan on the bare kernel. Moving water counts while it runs
## and not once it rests; a still pool never counts. A thin dirt wall holding a tall pool over dry ground is
## overloaded, a thick one or stone is not, a low pool barely loads it.

const N := 3


func _world() -> SimWorld:
	return load("res://test/test_sim_0138.gd").new()._world()


func _hold() -> PackedFloat32Array:
	var h := PackedFloat32Array()
	h.resize(256)
	h[GameMaterials.id_of("dirt")] = 3.0
	return h


func _worst(w: SimWorld, lo: Vector3i, hi: Vector3i) -> float:
	var r := w.dam_scan(PackedInt32Array([lo.x, lo.y, lo.z, hi.x, hi.y, hi.z]), _hold(), 0.1)
	var worst := 0.0
	for i in range(0, r.size(), 7):
		worst = maxf(worst, r[i + 3] / 1000.0)
	return worst


func _pool(w: SimWorld, depth: int, dam_x0: int, dam_x1: int, mat: int) -> void:
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 4, 48), GameMaterials.STONE)
	w.fill_box(Vector3i(0, 4, 10), Vector3i(48, 30, 12), GameMaterials.STONE)   # side walls
	w.fill_box(Vector3i(0, 4, 20), Vector3i(48, 30, 22), GameMaterials.STONE)
	w.fill_box(Vector3i(2, 4, 12), Vector3i(dam_x0, 4 + depth, 20), GameMaterials.WATER_STATIC)
	w.fill_box(Vector3i(dam_x0, 4, 12), Vector3i(dam_x1, 28, 20), mat)


func test_thin_dirt_dam_over_dry_ground_is_overloaded() -> void:
	var w := _world()
	_pool(w, 16, 20, 24, GameMaterials.id_of("dirt"))   # 2 m of water on 4 cells of dirt (holds 12)
	assert_float(_worst(w, Vector3i(20, 4, 12), Vector3i(24, 28, 20))).is_equal_approx(16.0 / 12.0, 0.01)


func test_thick_or_stone_or_low_holds() -> void:
	var w := _world()
	_pool(w, 16, 20, 32, GameMaterials.id_of("dirt"))   # 12 cells thick: holds 36
	assert_float(_worst(w, Vector3i(20, 4, 12), Vector3i(32, 28, 20))).is_less(0.5)
	w = _world()
	_pool(w, 16, 20, 24, GameMaterials.STONE)
	assert_float(_worst(w, Vector3i(20, 4, 12), Vector3i(24, 28, 20))).is_equal(0.0)
	w = _world()
	_pool(w, 4, 20, 24, GameMaterials.id_of("dirt"))   # a spillway keeps the head at 0.5 m
	assert_float(_worst(w, Vector3i(20, 4, 12), Vector3i(24, 28, 20))).is_less(0.5)


func test_moving_water_counts_until_it_rests() -> void:
	var w := _world()
	w.fill_box(Vector3i(0, 0, 0), Vector3i(48, 4, 48), GameMaterials.STONE)
	w.fill_box(Vector3i(2, 4, 12), Vector3i(10, 12, 20), GameMaterials.WATER_STATIC)
	var race := [Vector3i(14, 4, 12), Vector3i(22, 8, 20)]
	assert_int(w.count_moving_in_box(race[0], race[1], GameMaterials.WATER)).is_equal(0)
	w.fill_box(Vector3i(10, 4, 12), Vector3i(40, 12, 20), GameMaterials.AIR)   # an opening: the pool pours out
	var seen := 0
	for i in 60:
		w.tick(1)
		seen = maxi(seen, w.count_moving_in_box(race[0], race[1], GameMaterials.WATER))
	assert_int(seen).is_greater(20)
	assert_int(w.count_moving_in_box(race[0], race[1], GameMaterials.WATER_STATIC)).is_equal(0)
