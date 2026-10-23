extends GdUnitTestSuite

## Wildlife (ADR 0027): chickens, bears that hunt animals (and some the
## player), the carcasses kills leave, kelp and fish from the water map.

const S := preload("res://scripts/settings.gd")


func _manager() -> MobManager:
	var mm: MobManager = auto_free(MobManager.new())
	mm.configure(MobManager.load_defs())
	return mm


func _mob(mm: MobManager, species: String, pos: Vector3) -> Mob:
	var m: Mob = auto_free(Mob.create(mm, species, mm.defs[species], false))
	m.position = pos
	mm._active.append(m)
	return m


func test_data() -> void:
	var mm := _manager()
	assert_array(mm.errors).is_empty()
	var chicken: Dictionary = mm.defs["chicken"]
	assert_str(String(chicken["lays"]["item"])).is_equal("egg")
	assert_bool(chicken.get("slow_fall", false)).is_true()
	var bear: Dictionary = mm.defs["bear"]
	assert_array(bear["predator"]["prey"]).contains(["sheep", "pig", "chicken"])
	assert_float(float(bear["rarity"])).is_less(1.0)
	var items := ItemRegistry.load_default()
	for id in ["egg", "fried_egg", "feather", "bone", "bear_hide"]:
		assert_bool(items.has(id)).override_failure_message("no item %s" % id).is_true()
	# every part of the new bodies has cells
	for part in [["chicken", "body"], ["chicken", "head"], ["chicken", "leg"], ["chicken", "wing"], ["bear", "body"], ["bear", "head"], ["bear", "leg"]]:
		var spec: Array = MobModel._build(part[0], part[1])
		var filled := 0
		for c in spec[0]:
			if c != 0:
				filled += 1
		assert_int(filled).override_failure_message("%s/%s empty" % part).is_greater(10)


func test_bears_hunt_the_nearest_prey_and_eat_the_kill() -> void:
	var mm := _manager()
	var bear := _mob(mm, "bear", Vector3(0, 0, 0))
	var far_sheep := _mob(mm, "sheep", Vector3(40, 0, 0))   # 20 m: in sight, but not the nearest
	var pig := _mob(mm, "pig", Vector3(10, 0, 0))
	_mob(mm, "bear", Vector3(4, 0, 0))                        # bears do not hunt bears
	_mob(mm, "sheep", Vector3(200, 0, 0))                     # out of sight
	assert_object(mm.find_prey(bear)).is_same(pig)
	# bite until it dies: the kill is marked, counted, and the bear sits down to eat
	var bites := 0
	while not pig.is_dead() and bites < 10:
		pig._invuln_t = 0.0
		mm.maul(bear, pig, 6)
		bites += 1
	assert_bool(pig.is_dead()).is_true()
	assert_int(bites).is_equal(2)   # 10 health, 6 a bite
	assert_str(pig.killed_by).is_equal("bear")
	assert_int(int(mm.stats["kills"])).is_equal(1)
	assert_float(bear._eat_t).is_greater(0.0)
	assert_float(bear._hunger_t).is_greater(60.0)
	# hurt prey panics; a hurt bear does not
	far_sheep.hurt(1, bear.position)
	assert_float(far_sheep._panic_t).is_greater(0.0)
	bear.hurt(1, far_sheep.position)
	assert_float(bear._panic_t).is_equal(0.0)


func test_some_bears_are_aggressive() -> void:
	var mm := _manager()
	var angry := 0
	for uid in range(1, 401):
		var b: Mob = auto_free(Mob.create(mm, "bear", mm.defs["bear"], false))
		b.uid = uid
		add_child(b)
		if b.aggressive:
			angry += 1
		b.free()
	assert_int(angry).is_between(60, 140)   # aggressive_chance 0.25 of 400


func test_carcass_model_and_drops() -> void:
	var cells := GameMaterials.micro_pattern("carcass", [83, 82, 47])
	var bone := 0
	var meat := 0
	for c in cells:
		if c == 83:
			bone += 1
		elif c == 84:
			meat += 1
	assert_int(bone).is_greater(15)
	assert_int(meat).is_greater(0)
	var mining := MiningTable.load_default()
	assert_bool(mining.is_minable("carcass")).is_true()
	assert_str(GameMaterials.name_of(GameMaterials.CARCASS)).is_equal("carcass")


func _gen() -> FineMacro:
	return FineMacro.new()


func test_old_kills_lie_in_the_wild() -> void:
	var g := _gen()
	var p: Vector3i = g.find_nearest("plains", 0, 0, 12000)
	# the ground of a 128 m square, one 8 m tile at a time
	var n := 0
	for tz in range(-128, 128, 16):
		for tx in range(-128, 128, 16):
			var h := g.height_at(p.x + tx + 8, p.z + tz + 8)
			var bytes: PackedByteArray = g.generate_block(Vector3i(p.x + tx, h - 3, p.z + tz), Vector3i(16, 6, 16), 0, 2)
			for i in range(0, bytes.size(), 2):
				if bytes.decode_u16(i) == GameMaterials.CARCASS:
					n += 1
	assert_int(n).override_failure_message("no carcass in 128 m of plains").is_greater(0)


func test_water_map_sees_the_sea_and_not_the_ice() -> void:
	var g := _gen()
	var o: Vector3i = g.find_nearest("ocean", 0, 0, 12000)
	var m: PackedInt32Array = g.water_floor_map(o.x - 16, o.z - 16, 16, 2)
	var wet := 0
	for v in m:
		if v >= 0:
			wet += 1
			assert_int(v >> 16).is_greater(v & 0xFFFF)   # the water stands above the floor
	assert_int(wet).is_greater(200)
	var f: Vector3i = g.find_nearest("frozen_ocean", 0, 0, 32000)
	# the nearest frozen sea is its dithered edge: step in to a spot whose whole window is frozen sea
	var fb := g.biome_at(f.x, f.z)
	for step_in in [Vector3i.ZERO, Vector3i(0, 0, -64), Vector3i(0, 0, -128), Vector3i(64, 0, 0), Vector3i(-64, 0, 0), Vector3i(0, 0, 64)]:
		var q: Vector3i = f + step_in
		if g.biome_at(q.x - 16, q.z - 16) == fb and g.biome_at(q.x + 16, q.z - 16) == fb and g.biome_at(q.x - 16, q.z + 16) == fb and g.biome_at(q.x + 16, q.z + 16) == fb:
			f = q
			break
	m = g.water_floor_map(f.x - 16, f.z - 16, 16, 2)
	var open := 0
	for v in m:
		if v >= 0:
			open += 1
	# mostly ice: the dithered border with the open ocean leaves the same few holes the sea ice has
	assert_int(open).override_failure_message("kelp under the ice").is_less(64)
