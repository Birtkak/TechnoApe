extends GdUnitTestSuite

## The dig and build brush and the voxel units (ADR 0034).

var mining: MiningTable
var items: ItemRegistry


func before() -> void:
	mining = MiningTable.load_default()
	items = ItemRegistry.load_default()


func _per_block() -> int:
	return Settings.BLOCK * Settings.BLOCK * Settings.BLOCK


func test_placed_powders_are_the_still_kind() -> void:
	# sand built on a beach slid apart (bug report 2026-10-26_19-38-35): placed sand is the still kind
	assert_int(GameMaterials.still_of(GameMaterials.id_of("sand"))).is_equal(GameMaterials.id_of("sand_static"))
	assert_int(GameMaterials.still_of(GameMaterials.id_of("gravel"))).is_equal(GameMaterials.id_of("gravel_static"))
	assert_int(GameMaterials.still_of(GameMaterials.id_of("water"))).is_equal(GameMaterials.id_of("water"))
	assert_int(GameMaterials.still_of(GameMaterials.id_of("stone"))).is_equal(GameMaterials.id_of("stone"))
	assert_int(GameMaterials.still_of(0)).is_equal(0)


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


## Blocks go into water and gas as into air (bug report 2026-10-25: nothing could be built in water).
func test_blocks_can_be_placed_in_water() -> void:
	var p = auto_free(preload("res://scripts/player.gd").new())
	assert_bool(p._placeable(GameMaterials.AIR)).is_true()
	assert_bool(p._placeable(GameMaterials.WATER)).is_true()
	assert_bool(p._placeable(GameMaterials.WATER_STATIC)).is_true()
	assert_bool(p._placeable(GameMaterials.FLUID_LEVEL_BASE + 3)).is_true()   # a partly full cell
	assert_bool(p._placeable(GameMaterials.STEAM)).is_true()
	assert_bool(p._placeable(GameMaterials.STONE)).is_false()


# ---------------- shapes and sizes for reshaping (ADR 0065) ----------------

func test_flat_is_one_voxel_deep_in_the_face_plane() -> void:
	# a wall facing +x: the square stands in y/z, one voxel thick at the aimed x
	var cells := Brush.dig_cells(Vector3i(5, 9, -3), 8, "flat", Vector3i(1, 0, 0))
	assert_int(cells.size()).is_equal(64)
	for c in cells:
		assert_int(c.x).is_equal(5)
	# a floor: a level square at the aimed height
	for c in Brush.dig_cells(Vector3i(5, 9, -3), 4, "flat", Vector3i.UP):
		assert_int(c.y).is_equal(9)
	# placing lays it on the face, never into the aimed voxel
	for c in Brush.place_cells(Vector3i(5, 9, -3), Vector3i(5, 10, -3), 4, "flat"):
		assert_int(c.y).is_equal(10)


func test_column_runs_into_the_material_and_out_of_the_face() -> void:
	var d := Brush.column_dims(8)
	assert_object(d).is_equal(Vector2i(4, 16))
	# dug from a top face: a shaft down from the aimed voxel
	var shaft := Brush.dig_cells(Vector3i(1, 40, 1), 8, "column", Vector3i.UP)
	assert_int(shaft.size()).is_equal(4 * 4 * 16)
	var ys := shaft.map(func(c: Vector3i) -> int: return c.y)
	assert_int(ys.max()).is_equal(40)
	assert_int(ys.min()).is_equal(25)
	# dug from a wall facing -z: a tunnel towards +z
	var tunnel := Brush.dig_cells(Vector3i(0, 0, 10), 8, "column", Vector3i(0, 0, -1))
	var zs := tunnel.map(func(c: Vector3i) -> int: return c.z)
	assert_int(zs.min()).is_equal(10)
	assert_int(zs.max()).is_equal(25)
	# placed on a floor: a pillar up from the face
	var pillar := Brush.place_cells(Vector3i(0, 0, 0), Vector3i(0, 1, 0), 4, "column")
	assert_int(pillar.size()).is_equal(2 * 2 * 8)
	for c in pillar:
		assert_int(c.y).is_between(1, 8)


func test_creative_has_bigger_brushes_and_the_region_matches() -> void:
	assert_array(Brush.sizes_for(false)).is_equal([1, 2, 4, 8])
	assert_array(Brush.sizes_for(true)).contains([16, 32])
	var r := Brush.region(Vector3i(3, 3, 3), Vector3i(3, 4, 3), 32, "cube", false)
	assert_object(r["lo"]).is_equal(Vector3i(0, 0, 0))
	assert_object(r["size"]).is_equal(Vector3i(32, 32, 32))
	# native cells of a big sphere: about 4/3 pi 16.5^3, centred on the aimed voxel
	var st := ShapeTool.new()
	var s := Brush.region(Vector3i(100, 50, -7), Vector3i(100, 51, -7), 32, "sphere", false)
	var cells: PackedInt32Array = st.cells(s["params"], s["lo"])["cells"]
	assert_int(cells.size() / 3).is_between(17000, 20500)
	var lo: Vector3i = s["lo"]
	assert_object(lo + s["size"] / 2).is_equal(Vector3i(100, 50, -7))


func test_brush_preview_cells_match_the_dig() -> void:
	var bt := BrushTool.new()
	var hit := Vector3i(7, 7, 7)
	var c := bt.cells(hit, hit + Vector3i.UP, 8, "sphere", false)
	assert_int(c.size() / 3).is_equal(Brush.dig_cells(hit, 8, "sphere").size())
	assert_object(BrushTool.anchor(hit, hit + Vector3i.UP, 8, "cube", false)).is_equal(Vector3i(0, 0, 0))


func test_brush_sizes_described() -> void:
	assert_str(Brush.describe(4, "cube")).is_equal("cube 50 cm")
	assert_str(Brush.describe(32, "sphere")).is_equal("sphere 4.0 m")
	assert_str(Brush.describe(8, "column")).is_equal("column 50 cm wide, 2.0 m long")


## Big brushes keep their cells relative to the box corner: shifted back, they are ShapeTool's
## world cells for that aim (ShapeTool.fill is handed window - lo instead of shifted cells).
func test_big_brush_cells_are_cached_relative_to_the_box() -> void:
	var bt := BrushTool.new()
	var st := ShapeTool.new()
	for shape in ["sphere", "cube", "column"]:
		var hit := Vector3i(37, -12, 5)
		var front := hit + Vector3i(1, 0, 0)
		var r := Brush.region(hit, front, 16, shape, false)
		var world: PackedInt32Array = st.cells(r["params"], r["lo"])["cells"]
		var rel := bt.rel_cells(hit, front, 16, shape, false)
		assert_int(rel.size()).is_equal(world.size())
		var lo := BrushTool.anchor(hit, front, 16, shape, false)
		assert_object(lo).is_equal(r["lo"])
		assert_int(rel[0] + lo.x).is_equal(world[0])
		assert_int(rel[rel.size() - 1] + lo.z).is_equal(world[world.size() - 1])
	# a placement against the player's body is caught
	var body := AABB(Vector3(0, 1, 0), Vector3(4, 14, 4))
	assert_bool(bt.overlaps(Vector3i.ZERO, Vector3i.UP, 16, "cube", true, body)).is_true()
	assert_bool(bt.overlaps(Vector3i.ZERO, Vector3i.UP, 16, "cube", true, AABB(Vector3(100, 0, 0), Vector3.ONE))).is_false()
