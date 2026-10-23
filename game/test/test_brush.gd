extends GdUnitTestSuite

## The dig and build brush and the voxel units (ADR 0034).

var mining: MiningTable
var items: ItemRegistry


func before() -> void:
	mining = MiningTable.load_default()
	items = ItemRegistry.load_default()


func _per_block() -> int:
	return Settings.BLOCK * Settings.BLOCK * Settings.BLOCK


func test_cube_snaps_to_its_grid() -> void:
	var cells := Brush.dig_cells(Vector3i(5, 9, -3), 4, "cube")
	assert_int(cells.size()).is_equal(64)
	assert_bool(cells.has(Vector3i(4, 8, -4))).is_true()
	assert_bool(cells.has(Vector3i(7, 11, -1))).is_true()
	assert_bool(cells.has(Vector3i(8, 8, -4))).is_false()
	assert_object(Brush.dig_bounds(Vector3i(5, 9, -3), 4, "cube")).is_equal(AABB(Vector3(4, 8, -4), Vector3(4, 4, 4)))


func test_size_one_is_the_aimed_voxel() -> void:
	assert_array(Brush.dig_cells(Vector3i(1, 2, 3), 1, "sphere")).is_equal([Vector3i(1, 2, 3)])
	assert_array(Brush.place_cells(Vector3i(1, 2, 3), Vector3i(1, 3, 3), 1, "cube")).is_equal([Vector3i(1, 3, 3)])


func test_sphere_and_cylinder_are_round() -> void:
	var s := Brush.dig_cells(Vector3i.ZERO, 8, "sphere")
	var c := Brush.dig_cells(Vector3i.ZERO, 8, "cylinder")
	# a ball of diameter 8 holds about 4/3 pi 4^3 = 268 voxels, the cylinder pi 4^2 8 = 402
	assert_int(s.size()).is_between(230, 310)
	assert_int(c.size()).is_between(360, 460)
	assert_bool(s.has(Vector3i.ZERO)).is_true()
	assert_bool(s.has(Vector3i(4, 4, 4))).is_false()


func test_placing_sits_against_the_face() -> void:
	# aimed at the top of voxel (0,0,0): a sphere grows upwards from the face, never into the hit voxel
	var cells := Brush.place_cells(Vector3i(0, 0, 0), Vector3i(0, 1, 0), 4, "sphere")
	assert_bool(cells.has(Vector3i(0, 0, 0))).is_false()
	for c in cells:
		assert_int(c.y).is_greater_equal(1)


func test_dig_cap_grows_with_the_tool() -> void:
	assert_int(MiningTable.dig_cap({})).is_equal(2)
	assert_int(MiningTable.dig_cap(items.get_def("flint_axe"))).is_equal(4)
	assert_int(MiningTable.dig_cap({"tags": ["pickaxe"], "tier": 3})).is_equal(8)


func test_voxel_items_and_pieces() -> void:
	assert_int(items.units("dirt")).is_equal(_per_block())
	assert_int(items.units("cobblestone")).is_equal(_per_block())
	assert_int(items.units("stick")).is_equal(1)
	assert_int(items.units("crafting_table")).is_equal(1)   # a station, not terrain
	assert_int(items.units("water_bucket")).is_equal(1)
	assert_int(items.max_stack("dirt")).is_equal(64 * _per_block())


func test_a_block_of_stone_gives_a_block_of_cobblestone() -> void:
	var pick := {"tags": ["pickaxe"], "tier": 1}
	var carry := {}
	var rng := RandomNumberGenerator.new()
	var plain := func(n: String) -> bool: return GameMaterials.is_plain_voxel(GameMaterials.id_of(n))
	var drops := mining.voxel_drops({"stone": _per_block()}, pick, items.units, plain, carry, rng, _per_block())
	assert_array(drops).is_equal([{"id": "cobblestone", "count": _per_block()}])


func test_fractional_drops_carry_over() -> void:
	# gravel's flint (a piece, by chance) flows by expected value across digs
	var shovel := {"tags": ["shovel"], "tier": 1}
	var carry := {}
	var rng := RandomNumberGenerator.new()
	var plain := func(n: String) -> bool: return GameMaterials.is_plain_voxel(GameMaterials.id_of(n))
	var total := {}
	for i in 40:
		for d in mining.voxel_drops({"gravel": _per_block() / 4}, shovel, items.units, plain, carry, rng, _per_block()):
			total[d["id"]] = int(total.get(d["id"], 0)) + int(d["count"])
	assert_int(int(total.get("gravel", 0))).is_equal(10 * _per_block())
	# whatever pieces gravel yields per block, ten blocks' worth came out whole
	for id in total:
		if id != "gravel":
			assert_float(float(carry.get(id, 0.0))).is_less(1.0)


func test_right_click_places_a_block_of_a_voxel_item() -> void:
	var inv := Inventory.new(items, 2)
	var cursor := {"id": "dirt", "count": 3 * _per_block()}
	cursor = inv.right_click(0, cursor)
	assert_dict(inv.get_slot(0)).is_equal({"id": "dirt", "count": _per_block()})
	assert_int(int(cursor["count"])).is_equal(2 * _per_block())
	cursor = {"id": "dirt", "count": 10}   # less than a block: all of it
	cursor = inv.right_click(1, cursor)
	assert_dict(cursor).is_empty()
	assert_dict(inv.get_slot(1)).is_equal({"id": "dirt", "count": 10})
