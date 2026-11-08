extends GdUnitTestSuite
## NetStream (ADR 0169): the client's mirror of the server-owned chunks.

const POS := Vector3i(2, -1, 3)


func _buffer_with_stone() -> VoxelBuffer:
	var vb := VoxelBuffer.new()
	vb.create(Settings.CHUNK, Settings.CHUNK, Settings.CHUNK)
	vb.set_voxel(GameMaterials.STONE, 1, 2, 3, VoxelBuffer.CHANNEL_TYPE)
	return vb


func test_put_keeps_the_newer_snapshot() -> void:
	var s := NetStream.new()
	assert_bool(s.put(POS, PackedByteArray([1]), 5)).is_true()
	assert_bool(s.put(POS, PackedByteArray([2]), 3)).is_false()   # older journal position: ignored
	assert_int(s.baked(POS)).is_equal(5)
	assert_array(Array(s.blocks[POS])).is_equal([1])
	assert_bool(s.put(POS, PackedByteArray([3]), 7)).is_true()
	assert_int(s.baked(POS)).is_equal(7)
	assert_array(Array(s.blocks[POS])).is_equal([3])


func test_the_terrain_save_is_ignored() -> void:
	var s := NetStream.new()
	s._save_voxel_block(_buffer_with_stone(), POS, 0)
	assert_array(s.take_outbox()).is_empty()
	assert_int(s.size()).is_equal(0)
	assert_int(s.baked(POS)).is_equal(-1)


func test_an_own_snapshot_goes_to_the_outbox() -> void:
	var s := NetStream.new()
	s.known_n = 9
	s.put_own(POS, PackedByteArray([4]))
	var out := s.take_outbox()
	assert_int(out.size()).is_equal(1)
	assert_that(out[0][0]).is_equal(POS)
	assert_array(Array(out[0][1])).is_equal([4])
	assert_int(out[0][2]).is_equal(9)
	assert_array(s.take_outbox()).is_empty()   # taking empties it
	assert_int(s.baked(POS)).is_equal(9)
	assert_bool(s.put(POS, PackedByteArray([5]), 8)).is_false()   # the server's older copy does not replace it


func test_a_stored_chunk_is_served() -> void:
	var s := NetStream.new()
	var vb := VoxelBuffer.new()
	vb.create(Settings.CHUNK, Settings.CHUNK, Settings.CHUNK)
	assert_int(s._load_voxel_block(vb, POS, 0)).is_equal(VoxelStream.RESULT_BLOCK_NOT_FOUND)
	s.put(POS, VoxelBlockSerializer.serialize_to_byte_array(_buffer_with_stone(), VoxelBlockSerializer.COMPRESSION_LZ4), 1)
	assert_int(s._load_voxel_block(vb, POS, 0)).is_equal(VoxelStream.RESULT_BLOCK_FOUND)
	assert_int(vb.get_voxel(1, 2, 3, VoxelBuffer.CHANNEL_TYPE)).is_equal(GameMaterials.STONE)


func test_to_buffer_round_trips_a_voxel() -> void:
	var bytes := VoxelBlockSerializer.serialize_to_byte_array(_buffer_with_stone(), VoxelBlockSerializer.COMPRESSION_LZ4)
	var vb := NetStream.to_buffer(bytes)
	assert_object(vb).is_not_null()
	assert_int(vb.get_voxel(1, 2, 3, VoxelBuffer.CHANNEL_TYPE)).is_equal(GameMaterials.STONE)
	assert_int(vb.get_voxel(0, 0, 0, VoxelBuffer.CHANNEL_TYPE)).is_equal(0)
	assert_object(NetStream.to_buffer(PackedByteArray())).is_null()
