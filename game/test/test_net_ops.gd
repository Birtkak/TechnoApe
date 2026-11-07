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
