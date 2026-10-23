extends GdUnitTestSuite

## BrickStore wrapped mode and micro patterns, and NearFieldFeed's window slabs,
## dirty chunks and byte budget (ADR 0012 step 3). Pure data: no GPU.


func _store(bricks: int, origin: Vector3i) -> BrickStore:
	var s := BrickStore.new()
	s.configure(Vector3i.ONE * bricks)
	s.set_wrap_origin(origin)
	return s


func _ids16(count: int, material: int) -> PackedByteArray:
	var b := PackedByteArray()
	b.resize(count * 2)
	for i in count:
		b[i * 2] = material & 255
		b[i * 2 + 1] = material >> 8
	return b


func test_wrapped_store_addresses_world_bricks() -> void:
	var origin := Vector3i(1000, 192, -3008)
	var s := _store(64, origin)
	assert_bool(s.is_wrapped()).is_true()
	s.load_voxels(origin + Vector3i(16, 0, 16), Vector3i(16, 16, 16), _ids16(4096, GameMaterials.STONE))
	assert_int(s.brick_material(origin + Vector3i(20, 5, 20))).is_equal(GameMaterials.STONE)
	assert_int(s.brick_material(origin + Vector3i(40, 5, 20))).is_equal(0)
	assert_int(s.get_cell((origin + Vector3i(20, 5, 20)) * 8 + Vector3i(3, 3, 3))).is_equal(GameMaterials.STONE)
	# dirty bricks come back in world coordinates inside the window
	var dirty: Array = s.get_dirty_bricks()
	assert_int(dirty.size()).is_equal(4096)
	for b in dirty:
		var v: Vector3i = b
		assert_bool(v.x >= origin.x + 16 and v.x < origin.x + 32 and v.y >= origin.y and v.y < origin.y + 16 and v.z >= origin.z + 16 and v.z < origin.z + 32).is_true()
	# a chunk of 16^3 cells (2^3 bricks) read by world chunk coords
	var chunk := (origin + Vector3i(16, 0, 16)) / 2
	var bytes := s.get_chunk_bytes(chunk)
	assert_int(bytes.size()).is_equal(4096)
	assert_int(bytes[0]).is_equal(GameMaterials.STONE)
	assert_int(bytes[4095]).is_equal(GameMaterials.STONE)
	# sliding the origin keeps bricks that stay inside the new window
	s.set_wrap_origin(origin + Vector3i(16, 0, 0))
	assert_int(s.brick_material(origin + Vector3i(20, 5, 20))).is_equal(GameMaterials.STONE)


func test_micro_pattern_expands_on_write() -> void:
	var s := _store(16, Vector3i.ZERO)
	s.set_micro_patterns(NearField.micro_patterns())
	var b := Vector3i(3, 3, 3)
	s.fill_uniform(b, b + Vector3i.ONE, GameMaterials.TALL_GRASS)
	assert_bool(s.is_expanded(b)).is_true()
	var pattern: PackedInt32Array = GameMaterials.micro_pattern("tuft", GameMaterials.defs()[GameMaterials.TALL_GRASS - 1]["tiles"])
	var filled := 0
	for z in 8:
		for y in 8:
			for x in 8:
				var expect := GameMaterials.TALL_GRASS if pattern[x + 8 * (y + 8 * z)] != 0 else 0
				assert_int(s.get_cell(b * 8 + Vector3i(x, y, z))).is_equal(expect)
				if expect != 0:
					filled += 1
	assert_int(filled).is_greater(8)
	# load_voxels takes the same path; stone stays uniform
	s.load_voxels(Vector3i(5, 5, 5), Vector3i(2, 1, 1), _ids16(2, GameMaterials.STONE))
	assert_bool(s.is_expanded(Vector3i(5, 5, 5))).is_false()
	assert_int(s.brick_material(Vector3i(6, 5, 5))).is_equal(GameMaterials.STONE)


func test_feed_slabs_and_valid_box() -> void:
	var origin_brick := Vector3i(64, 128, 64)
	var s := _store(128, origin_brick)
	s.fill_uniform(origin_brick, origin_brick + Vector3i(128, 20, 128), GameMaterials.DIRT)
	s.clear_dirty()
	var f := NearFieldFeed.new()
	f.configure(s, 256, 4096)   # 16 chunks per edge
	var win := origin_brick * 8 + Vector3i(16, 16, 16)   # cells, chunk aligned
	assert_bool(f.set_window(win)).is_true()
	assert_int(f.pending_planes()).is_equal(16)        # one axis of planes covers the whole window
	assert_that(f.get_valid_max()).is_equal(f.get_valid_min())
	var up: Dictionary = f.pop_upload(1 << 30)
	assert_int(up["count"]).is_equal(4096)
	assert_int(f.pending_planes()).is_equal(0)
	assert_that(f.get_valid_min()).is_equal(win)
	assert_that(f.get_valid_max()).is_equal(win + Vector3i.ONE * 256)
	var bytes: PackedByteArray = up["bytes"]
	var coords: PackedInt32Array = up["coords"]
	assert_int(bytes.size()).is_equal(4096 * 4096)
	assert_int(coords.size()).is_equal(4096 * 4)
	# the first chunk's bytes are the store's cells (dirt below y = origin + 20 bricks, air above)
	var c0 := Vector3i(coords[0], coords[1], coords[2])
	assert_that(bytes.slice(0, 4096)).is_equal(s.get_chunk_bytes(c0))
	# slide one chunk in +x: one plane pending at the high side, the valid box misses it
	assert_bool(f.set_window(win + Vector3i(16, 0, 0))).is_true()
	assert_int(f.pending_planes()).is_equal(1)
	assert_that(f.get_valid_min()).is_equal(win + Vector3i(16, 0, 0))
	assert_that(f.get_valid_max()).is_equal(win + Vector3i(256, 256, 256))
	up = f.pop_upload(1 << 30)
	assert_int(up["count"]).is_equal(256)
	coords = up["coords"]
	for i in 256:
		assert_int(coords[i * 4]).is_equal((win.x + 16) / 16 + 15)
	assert_that(f.get_valid_max()).is_equal(win + Vector3i(272, 256, 256))
	# two planes with a budget of one: the valid box grows by one plane per call
	f.set_window(win + Vector3i(16, 0, 32))
	assert_int(f.pending_planes()).is_equal(2)
	up = f.pop_upload(256 * 4096)
	assert_int(up["count"]).is_equal(256)
	assert_int(f.pending_planes()).is_equal(1)
	up = f.pop_upload(256 * 4096)
	assert_int(up["count"]).is_equal(256)
	assert_int(f.pending_planes()).is_equal(0)
	assert_that(f.get_valid_min()).is_equal(win + Vector3i(16, 0, 32))
	# a jump farther than the window: everything pending again, nothing valid
	f.set_window(win + Vector3i(1024, 0, 0))
	assert_int(f.pending_planes()).is_equal(16)
	assert_that(f.get_valid_max()).is_equal(f.get_valid_min())


func test_feed_dirty_chunks_inside_window_only() -> void:
	var origin_brick := Vector3i(0, 64, 0)
	var s := _store(128, origin_brick)
	var f := NearFieldFeed.new()
	f.configure(s, 256, 4096)
	var win := origin_brick * 8 + Vector3i(32, 32, 32)
	f.set_window(win)
	f.pop_upload(1 << 30)
	s.clear_dirty()
	var inside := origin_brick + Vector3i(10, 10, 10)     # brick inside the window
	var outside := origin_brick + Vector3i(100, 10, 10)   # brick in the store but outside the window
	s.fill_uniform(inside, inside + Vector3i.ONE, GameMaterials.SAND)
	s.fill_uniform(outside, outside + Vector3i.ONE, GameMaterials.SAND)
	assert_int(f.collect_dirty()).is_equal(1)
	assert_int(f.pending_chunks()).is_equal(1)
	var up: Dictionary = f.pop_upload(1 << 30)
	assert_int(up["count"]).is_equal(1)
	var coords: PackedInt32Array = up["coords"]
	assert_that(Vector3i(coords[0], coords[1], coords[2])).is_equal(inside / 2)
	var bytes: PackedByteArray = up["bytes"]
	assert_int(bytes.count(GameMaterials.SAND)).is_equal(512)
	assert_int(f.pending_chunks()).is_equal(0)
	# the store's dirty list was consumed
	assert_int(s.get_dirty_bricks().size()).is_equal(0)
