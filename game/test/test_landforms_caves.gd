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


func test_halls_have_a_talus_and_a_roof() -> void:
	# ADR 0134: a giant chamber is at most 22 m tall over its floor, and rubble (gravel, cobble) slopes up its walls
	var p: Vector3i = gen.find_nearest("cavern", 0, 0, FAR)
	var top := 0
	var b: PackedByteArray = gen.generate_block(p, Vector3i(1, int(S.m2v(40.0)), 1), 0, 2)
	while top < b.size() / 2 and b.decode_u16(top * 2) == 0:
		top += 1
	assert_float(top * S.VOXEL_M).override_failure_message("chamber %.1f m tall" % (top * S.VOXEL_M)).is_less_equal(22.5)
	var r := int(S.m2v(40.0))
	var ids := _block(p + Vector3i(-r, -8, -r), Vector3i(2 * r, int(S.m2v(10.0)), 2 * r))
	var rubble := _count(ids, GameMaterials.GRAVEL_STATIC) + _count(ids, GameMaterials.COBBLESTONE)
	assert_int(rubble).override_failure_message("%d rubble voxels round the chamber floor" % rubble).is_greater(2000)


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
				if m != GameMaterials.TOXIC_SLUDGE and m != GameMaterials.WATER:   # finite cave pools (bug report 2026-11-07 10-32-53)
					continue
				pools += 1
				for d in [Vector2i(1, 0), Vector2i(-1, 0), Vector2i(0, 1), Vector2i(0, -1)]:
					if ids[y + size.y * (x + d.x + size.x * (z + d.y))] == 0:
						bad += 1
	assert_int(pools).override_failure_message("no moving-liquid cave pool near the sulfur cave").is_greater(0)
	assert_int(_count(ids, GameMaterials.TOXIC_SLUDGE_STATIC) + _count(ids, GameMaterials.WATER_STATIC)).is_equal(0)
	assert_int(bad).override_failure_message("%d of %d pool voxels open sideways onto air" % [bad, pools]).is_equal(0)


func test_landforms_are_found() -> void:
	for k in ["monolith", "volcano", "sea_stack", "arch", "delta", "ravine"]:
		var p: Vector3i = gen.find_nearest(k, 0, 0, FAR)
		assert_int(p.y).override_failure_message("no %s within 8 km" % k).is_greater(0)


func test_monolith_stands_out_in_rock() -> void:
	var p: Vector3i = gen.find_nearest("monolith", 0, 0, FAR)
	var around := gen.height_at(p.x + int(S.m2v(400.0)), p.z)
	assert_int(p.y - around).override_failure_message("monolith %d over %d" % [p.y, around]).is_greater(int(S.m2v(25.0)))
	var ids := _block(Vector3i(p.x - 8, p.y - 24, p.z - 8), Vector3i(16, 32, 16))
	assert_int(_count(ids, GameMaterials.TERRACOTTA)).is_greater(100)


func test_volcano_holds_a_lava_crater() -> void:
	var p: Vector3i = gen.find_nearest("volcano", 0, 0, FAR)   # the crater's centre
	var lava := int(gen.column_info(p.x, p.z)["lava"])
	assert_int(lava).override_failure_message("no lava in the crater at %s" % p).is_greater(p.y)
	var rim := 0   # the crater's rim: the highest ground within 40 m east of the centre
	for r in range(0, int(S.m2v(40.0)), 8):
		rim = maxi(rim, gen.height_at(p.x + r, p.z))
	assert_int(rim - lava).override_failure_message("rim %d over lava %d" % [rim, lava]).is_greater(int(S.m2v(3.0)))
	var ids := _block(Vector3i(p.x - 8, lava - 12, p.z - 8), Vector3i(16, 16, 16))
	assert_int(_count(ids, GameMaterials.LAVA_STATIC)).is_greater(100)


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
	assert_int(pots.size()).is_between(1, 4)   # ADR 0062: 1..4 per ruin, by kind
	# the same spots whatever radius they are asked with
	var again: Array = gen.ruin_pots_near(r.x + 8, r.z, int(S.m2v(40.0)))
	assert_int(again.size()).is_greater_equal(pots.size())
	for d in pots:
		var under := _block(Vector3i(int(d["x"]), int(d["y"]) - 1, int(d["z"])), Vector3i(1, 2, 1))
		assert_int(under[0]).is_not_equal(0)   # the floor


func test_glow_mushrooms_are_pieces_with_a_stem() -> void:
	# bug report 2026-11-07 10-27-47: a hall's glow mushrooms stand as stem + glowing cap pieces (dug whole, like
	# flowers), never as lone glowing voxels on the floor
	var g := FineGen.new()
	g.configure(1337, S.SEA_LEVEL_VOXEL, S.WORLD_HEIGHT_VOXELS)
	var size := Vector3i(64, 48, 64)
	var b: PackedByteArray = g.generate_block(Vector3i(868, 600, -380), size, 0, 2)
	var at := func(p: Vector3i) -> int:
		if p.x < 0 or p.y < 0 or p.z < 0 or p.x >= size.x or p.y >= size.y or p.z >= size.z:
			return 0
		return b.decode_u16(2 * (p.y + size.y * (p.x + size.x * p.z)))
	var seen := {}
	var pieces := 0
	var lone := 0
	for z in size.z:
		for x in size.x:
			for y in size.y:
				var p := Vector3i(x, y, z)
				if at.call(p) != GameMaterials.GLOW_MUSHROOM or seen.has(p):
					continue
				var stack: Array[Vector3i] = [p]
				var cells := 0
				var stem := false
				var edge := false   # a piece cut by the block's side may have its stem outside
				seen[p] = true
				while not stack.is_empty():
					var q: Vector3i = stack.pop_back()
					cells += 1
					stem = stem or at.call(q) == GameMaterials.MUSHROOM_STEM
					edge = edge or q.x <= 0 or q.z <= 0 or q.y <= 0 or q.x >= size.x - 1 or q.z >= size.z - 1 or q.y >= size.y - 1
					for dz in [-1, 0, 1]:
						for dy in [-1, 0, 1]:
							for dx in [-1, 0, 1]:
								var r: Vector3i = q + Vector3i(dx, dy, dz)
								var m: int = at.call(r)
								if not seen.has(r) and (m == GameMaterials.GLOW_MUSHROOM or m == GameMaterials.MUSHROOM_STEM):
									seen[r] = true
									stack.append(r)
				pieces += 1
				if not edge and (not stem or cells < 2):
					lone += 1
	assert_int(pieces).override_failure_message("no glow mushroom in the hall of the bug report").is_greater(0)
	assert_int(lone).override_failure_message("%d of %d glow mushrooms are not stem-and-cap pieces" % [lone, pieces]).is_equal(0)
