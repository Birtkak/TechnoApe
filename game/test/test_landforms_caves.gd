extends GdUnitTestSuite

## The 640 m world, big caves and landforms of ADR 0051/0052: every feature is found by find_nearest and
## generates what it promises; cave pools never stand over a lower floor; wild food comes from biomes.json.

const S := preload("res://scripts/settings.gd")
const SEED := 1866545922
const FAR := 64000

var gen: FineGen


func before() -> void:
	gen = FineGen.new()
	gen.configure(SEED, S.SEA_LEVEL_VOXEL, S.WORLD_HEIGHT_VOXELS)


func _block(org: Vector3i, size: Vector3i) -> PackedInt32Array:
	var b: PackedByteArray = gen.generate_block(org, size, 0, 2)
	var out := PackedInt32Array()
	out.resize(size.x * size.y * size.z)
	if b.is_empty():
		return out
	for i in out.size():
		out[i] = b.decode_u16(i * 2)
	return out


func _count(ids: PackedInt32Array, mat: int) -> int:
	var n := 0
	for m in ids:
		if m == mat:
			n += 1
	return n


func test_world_is_640_m() -> void:
	assert_int(S.WORLD_HEIGHT_VOXELS).is_equal(5120)
	var p: Vector3i = gen.find_nearest("mountain", 0, 0, FAR)
	assert_int(p.y).is_greater(0)


func test_crystal_and_reserved_ids() -> void:
	assert_int(GameMaterials.CRYSTAL).is_equal(120)
	assert_str(String(GameMaterials.defs()[GameMaterials.CRYSTAL - 1]["name"])).is_equal("crystal")
	for id in range(116, 120):
		assert_str(String(GameMaterials.defs()[id - 1]["name"])).is_equal("reserved_%d" % id)


func test_geode_is_a_crystal_lined_hollow() -> void:
	var p: Vector3i = gen.find_nearest("geode", 0, 0, FAR)
	assert_int(p.y).is_greater(0)
	var ids := _block(p - Vector3i(32, 32, 32), Vector3i(64, 64, 64))
	assert_int(_count(ids, GameMaterials.CRYSTAL)).is_greater(500)
	assert_int(_count(ids, GameMaterials.BASALT)).is_greater(200)
	assert_int(_count(ids, 0)).is_greater(2000)


func test_cavern_is_a_hall() -> void:
	var p: Vector3i = gen.find_nearest("cavern", 0, 0, FAR)
	assert_int(p.y).is_greater(0)
	# 10 m of air above the floor over a 4 m square
	var ids := _block(p + Vector3i(-16, 0, -16), Vector3i(32, 80, 32))
	assert_float(float(_count(ids, 0)) / ids.size()).is_greater(0.6)


func test_cave_mouths_open_on_the_surface() -> void:
	# over a 1 km square: 10-40 entrances per km^2 (developer: "big" caves)
	var n := 160
	var step := int(S.m2v(1000.0)) / n
	var cm: PackedByteArray = gen.cave_map(-n / 2 * step, -n / 2 * step, n, step)
	var open := 0
	for i in n * n:
		open += cm[i * 2 + 1]
	assert_int(open).is_greater(20)
	var share := 0.0
	for i in n * n:
		share += cm[i * 2] / 255.0
	assert_float(share / (n * n)).is_between(0.04, 0.25)


func test_cave_pools_never_hang_over_a_drop() -> void:
	# every pool voxel (sludge in sulfur caves, water in lush ones) has no cave air beside it at its height
	var p: Vector3i = gen.find_nearest("sulfur_cave", 0, 0, FAR)
	assert_int(p.y).is_greater(0)
	var size := Vector3i(96, 48, 96)
	var org := p - Vector3i(48, 16, 48)
	var ids := _block(org, size)
	var pools := 0
	var bad := 0
	for z in range(1, size.z - 1):
		for x in range(1, size.x - 1):
			for y in range(0, size.y):
				var m := ids[y + size.y * (x + size.x * z)]
				if m != GameMaterials.TOXIC_SLUDGE_STATIC and m != GameMaterials.WATER_STATIC:
					continue
				pools += 1
				for d in [Vector2i(1, 0), Vector2i(-1, 0), Vector2i(0, 1), Vector2i(0, -1)]:
					if ids[y + size.y * (x + d.x + size.x * (z + d.y))] == 0:
						bad += 1
	assert_int(bad).override_failure_message("%d of %d pool voxels open sideways onto air" % [bad, pools]).is_equal(0)


func test_landforms_are_found() -> void:
	for k in ["monolith", "sea_stack", "arch", "delta", "ravine"]:
		var p: Vector3i = gen.find_nearest(k, 0, 0, FAR)
		assert_int(p.y).override_failure_message("no %s within 8 km" % k).is_greater(0)


func test_monolith_stands_out_in_rock() -> void:
	var p: Vector3i = gen.find_nearest("monolith", 0, 0, FAR)
	var around := gen.height_at(p.x + int(S.m2v(400.0)), p.z)
	assert_int(p.y - around).override_failure_message("monolith %d over %d" % [p.y, around]).is_greater(int(S.m2v(25.0)))
	var ids := _block(Vector3i(p.x - 8, p.y - 24, p.z - 8), Vector3i(16, 32, 16))
	assert_int(_count(ids, GameMaterials.TERRACOTTA)).is_greater(100)


func test_wild_food_comes_from_biomes_json() -> void:
	var reg := BiomeRegistry.load_default()
	assert_array(reg.errors).is_empty()
	var forest: Dictionary = reg.biomes[reg.by_name["forest"]]
	assert_float(float(forest["decor"]["berries"])).is_less(0.1)        # a third of 1 in 6
	assert_float(float(forest["decor"]["mushrooms"])).is_less(0.05)     # a third of 3 in 32
	assert_float(float(forest["decor"]["log_mushrooms"])).is_less(0.02) # a third of 1 in 29


func test_ruin_pot_spots_stand_inside_ruins() -> void:
	var r: Vector3i = gen.find_nearest("ruin", 0, 0, FAR)
	assert_int(r.y).is_greater(0)
	var pots: Array = gen.ruin_pots_near(r.x, r.z, int(S.m2v(20.0)))
	assert_int(pots.size()).is_between(2, 4)
	# the same spots whatever radius they are asked with
	var again: Array = gen.ruin_pots_near(r.x + 8, r.z, int(S.m2v(40.0)))
	assert_int(again.size()).is_greater_equal(pots.size())
	for d in pots:
		var under := _block(Vector3i(int(d["x"]), int(d["y"]) - 1, int(d["z"])), Vector3i(1, 2, 1))
		assert_int(under[0]).is_not_equal(0)   # the floor
