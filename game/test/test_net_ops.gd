extends GdUnitTestSuite

## The multiplayer edit ops (ADR 0145): every kind survives the wire unchanged, carries the box the
## chunk index needs, and garbage decodes to nothing.


func _roundtrip(op: Dictionary) -> Dictionary:
	var back := NetOps.decode(NetOps.encode(op))
	assert_str(back["k"]).is_equal(op["k"])
	assert_that(back["box"]).is_equal(op["box"])
	return back


func test_fill_roundtrip_keeps_cells_material_and_mask() -> void:
	var mask := PackedByteArray()
	mask.resize(256)
	mask[GameMaterials.id_of("stone")] = 1
	var op := NetOps.fill(Vector3i(100, 40, -20), PackedInt32Array([0, 0, 0, 3, 1, 2, -1, 0, 0]), GameMaterials.AIR, mask)
	var back := _roundtrip(op)
	assert_that(back["lo"]).is_equal(Vector3i(100, 40, -20))
	assert_that(back["cells"]).is_equal(PackedInt32Array([0, 0, 0, 3, 1, 2, -1, 0, 0]))
	assert_int(back["m"]).is_equal(GameMaterials.AIR)
	assert_int(back["mask"][GameMaterials.id_of("stone")]).is_equal(1)
	assert_that(back["box"]).is_equal([Vector3i(99, 40, -20), Vector3i(103, 41, -18)])


func test_fill_with_per_cell_materials() -> void:
	var op := NetOps.fill(Vector3i.ZERO, PackedInt32Array([5, 5, 5, 6, 5, 5]), 0, NetOps.all_mask(), PackedByteArray([3, 4]))
	var back := _roundtrip(op)
	assert_that(back["mats"]).is_equal(PackedByteArray([3, 4]))


func test_voxels_box_is_the_bounds_of_the_cells() -> void:
	var op := NetOps.voxels(PackedInt32Array([10, 20, 30, 8, 25, 30, 12, 20, 29]), PackedByteArray([1, 2, 3]))
	var back := _roundtrip(op)
	assert_that(back["box"]).is_equal([Vector3i(8, 20, 29), Vector3i(12, 25, 30)])
	assert_that(back["mats"]).is_equal(PackedByteArray([1, 2, 3]))


func test_box_and_sphere_roundtrip() -> void:
	var b := _roundtrip(NetOps.box(Vector3i(0, 0, 0), Vector3i(16, 2, 16), 7))
	assert_that(b["hi"]).is_equal(Vector3i(16, 2, 16))
	assert_that(b["box"]).is_equal([Vector3i(0, 0, 0), Vector3i(15, 1, 15)])   # hi is exclusive, the box inclusive
	var s := _roundtrip(NetOps.sphere(Vector3(10.5, 20.5, 30.5), 4.0, 0, PackedInt32Array([5])))
	assert_float(s["r"]).is_equal(4.0)
	assert_that(s["only"]).is_equal(PackedInt32Array([5]))


func test_chunks_of_covers_every_chunk_the_box_touches() -> void:
	var op := NetOps.box(Vector3i(14, 0, 0), Vector3i(18, 1, 1), 1)   # x 14..17 crosses the chunk edge at 16
	var chunks := NetOps.chunks_of(op)
	assert_int(chunks.size()).is_equal(2)
	assert_that(chunks).contains([Vector3i(0, 0, 0), Vector3i(1, 0, 0)])
	var neg := NetOps.chunks_of(NetOps.voxels(PackedInt32Array([-1, 0, -17]), PackedByteArray([1])))
	assert_that(neg).is_equal([Vector3i(-1, 0, -2)] as Array[Vector3i])


func test_garbage_decodes_to_nothing() -> void:
	assert_that(NetOps.decode(PackedByteArray([1, 2, 3]))).is_equal({})
	assert_that(NetOps.decode(var_to_bytes({"k": "fill"}))).is_equal({})
	assert_that(NetOps.decode(var_to_bytes("hello"))).is_equal({})


func test_gen_version_read_as_text_matches_the_constant() -> void:
	assert_int(Net.gen_version()).is_equal(TerrainGenerator.GEN_VERSION)


## ADR 0165: the server keeps the newest state per block, drops it on {}, and refuses what is not one.
func test_server_block_registry() -> void:
	var net: Node = auto_free(load("res://scripts/net/net.gd").new())
	assert_bool(net._keep_block({"k": "block", "pos": Vector3i(4, 5, 6), "d": {"kind": "chest", "pos": [0, 0, 0], "slots": []}})).is_true()
	assert_that(net._blocks["4,5,6"]["pos"]).is_equal([4, 5, 6])   # the key wins over what the sender wrote
	assert_bool(net._keep_block({"k": "block", "pos": "4,5,6", "d": {}})).is_false()
	assert_bool(net._keep_block({"k": "block", "pos": Vector3i(4, 5, 6), "d": "x"})).is_false()
	assert_bool(net._keep_block({"k": "block", "pos": Vector3i(4, 5, 6), "d": {}})).is_true()
	assert_bool(net._blocks.has("4,5,6")).is_false()


## A block entity's saved dict made again on another client is the same entity.
func test_block_load_one_roundtrip() -> void:
	var items := ItemRegistry.load_default()
	var recipes := RecipeRegistry.load_default(items)
	var b := BlockEntities.new(items, recipes)
	var c: RefCounted = b.create(Vector3i(8, 0, 8), "chest")
	c.inv.set_slot(0, {"id": "stick", "count": 3})
	var other := BlockEntities.new(items, recipes)
	var e: RefCounted = other.load_one(JSON.parse_string(JSON.stringify(c.to_dict())))
	assert_object(e).is_not_null()
	assert_that(e.pos).is_equal(Vector3i(8, 0, 8))
	assert_int(int(e.inv.get_slot(0)["count"])).is_equal(3)
	assert_object(other.load_one({"kind": "chest", "pos": "bad"})).is_null()
