extends GdUnitTestSuite

## Rock kinds, their materials, the new clutter pieces, ruin pots and their loot (ADR 0057): the data,
## the generated header, and what the native generator makes of them (deterministic from the seed).

const S := preload("res://scripts/settings.gd")
const F := FineGen.FS


func _gen(seed_value: int) -> FineGen:
	var g := FineGen.new()
	g.configure(seed_value, S.SEA_LEVEL_VOXEL, S.WORLD_HEIGHT_VOXELS)
	return g


func _ids(g: FineGen, origin: Vector3i, size: Vector3i) -> PackedInt32Array:
	var bytes: PackedByteArray = g.generate_block(origin, size, 0, 2)
	var ids := PackedInt32Array()
	ids.resize(size.x * size.y * size.z)
	if bytes.is_empty():
		return ids
	for i in ids.size():
		ids[i] = bytes.decode_u16(i * 2)
	return ids


func test_data_loads_and_the_header_is_current() -> void:
	var t := RockTable.load_default()
	assert_array(t.errors).is_empty()
	assert_int(t.provinces.size()).is_greater_equal(4)
	var path := ProjectSettings.globalize_path("res://").path_join("..").path_join(RockTable.HEADER_PATH)
	var f := FileAccess.open(path, FileAccess.READ)
	assert_object(f).override_failure_message("missing %s; run tools/rock_codegen.gd" % path).is_not_null()
	if f != null:
		assert_str(f.get_as_text().replace("\r\n", "\n")).override_failure_message("header stale; run tools/rock_codegen.gd").is_equal(t.generate())


func test_bad_data_is_rejected() -> void:
	var t := RockTable.new()
	t.load_json(JSON.stringify({"province_m": 400, "warp_m": 80, "provinces": {"p": {"weight": 1, "layers": [{"rock": "mithril", "thick_m": [9, 3]}, {"rock": "stone"}]}},
		"ore_bias": {"coal": {"granite": 0.0}}, "pyrite": {"per_mille": 1, "rocks": []}, "ochre": {"rocks": [], "share": 0.1},
		"clutter": {"acorns": {"shape": "blob", "where": "sky", "chance": 0.1, "biomes": ["moon"]}}, "crystals": {"per_mille": 1, "min_depth_m": 1, "crystal_cave_per_mille": 1}}))
	assert_int(t.errors.size()).is_greater_equal(5)


func test_ids_stay_in_the_reserved_range() -> void:
	assert_int(GameMaterials.id_of("granite")).is_equal(160)
	assert_int(GameMaterials.id_of("clay_pot_cracked")).is_equal(199)
	assert_int(GameMaterials.id_of("pyrite")).is_equal(GameMaterials.PYRITE)
	assert_int(GameMaterials.id_of("yellow_ochre")).is_equal(202)
	assert_str(GameMaterials.name_of(116)).is_equal("reserved_116")
	assert_str(GameMaterials.name_of(159)).is_equal("reserved_159")


func test_every_new_material_mines_and_every_drop_is_an_item() -> void:
	var mining := MiningTable.load_default()
	var items := ItemRegistry.load_default()
	for id in range(160, 203):
		var n := GameMaterials.name_of(id)
		assert_bool(mining.entry(n).is_empty()).override_failure_message("%s not in mining.json" % n).is_false()
		var e: Dictionary = mining.entry(n)
		if e.has("drops"):
			var list: Array = e["drops"] if e["drops"] is Array else [{"id": e["drops"]}]
			for d in list:
				assert_bool(items.has(String(d["id"]))).override_failure_message("%s drops unknown %s" % [n, d["id"]]).is_true()
	# loose rocks count as rocks: the firepit and the first quest take any (#rock)
	for k in GameMaterials.COBBLE_KINDS:
		assert_array(items.get_def(k + "_stone")["tags"]).contains(["rock"])
	var recipes := RecipeRegistry.load_default(items)
	assert_array(recipes.errors).is_empty()


func test_loot_rolls_are_deterministic_and_real_items() -> void:
	var items := ItemRegistry.load_default()
	for t in RuinLoot.tables():
		for e in RuinLoot.tables()[t]["entries"]:
			assert_bool(items.has(String(e["id"]))).override_failure_message("loot %s: unknown %s" % [t, e["id"]]).is_true()
	assert_str(RuinLoot.table_of(GameMaterials.CLAY_POT)).is_equal("ruin_pot")
	var a := RuinLoot.roll("ruin_pot", 42, Vector3i(10, 1100, -7), 0.0, items)
	var b := RuinLoot.roll("ruin_pot", 42, Vector3i(10, 1100, -7), 0.0, items)
	assert_str(str(a)).is_equal(str(b))
	var n := 0
	var worn := 0
	for i in 200:
		for s in RuinLoot.roll("ruin_pot", 42, Vector3i(i, 1100, 3), 0.0, items):
			n += 1
			if s.has("wear"):
				worn += 1
	assert_int(n).is_greater(150)   # most pots hold something
	assert_int(worn).is_greater(0)   # now and then an old tool


func test_rock_kinds_are_deterministic_from_the_seed() -> void:
	var o := Vector3i(3200, S.SEA_LEVEL_VOXEL - 200, -1600)
	var a := _ids(_gen(777), o, Vector3i(16, 16, 16))
	var b := _ids(_gen(777), o, Vector3i(16, 16, 16))
	assert_bool(a == b).is_true()


func test_the_world_holds_several_rock_kinds() -> void:
	var g := _gen(1337)
	var seen := {}
	var rocks := {}
	for k in ["stone", "basalt"] + GameMaterials.ROCK_KINDS:
		rocks[GameMaterials.id_of(k)] = k
	# a deep 16^3 sample every 600 m over a 6 km square
	for zi in 10:
		for xi in 10:
			var x := (xi - 5) * 4800
			var z := (zi - 5) * 4800
			var h := g.height_at(x, z)
			for id in _ids(g, Vector3i(x, h - 160, z), Vector3i(4, 4, 4)):
				if rocks.has(id):
					seen[rocks[id]] = true
	assert_int(seen.size()).override_failure_message("rocks seen: %s" % [seen.keys()]).is_greater_equal(5)


func test_new_clutter_digs_as_whole_pieces() -> void:
	for m in ["wild_strawberries", "hazelnuts", "acorns", "bones", "feathers", "pine_cone", "driftwood", "antler", "ice_shard", "cave_crystal", "clay_pot", "granite_stones", "chalk_stones"]:
		assert_bool(GroundPickups.is_piece(GameMaterials.id_of(m))).override_failure_message(m).is_true()
	# strawberries share the flower group with their plant_stem leaves: one piece
	assert_str(GroundPickups.group_of(GameMaterials.WILD_STRAWBERRIES)).is_equal(GroundPickups.group_of(GameMaterials.PLANT_STEM))
