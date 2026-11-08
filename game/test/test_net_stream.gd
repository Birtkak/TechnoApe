extends GdUnitTestSuite
## NetStream (ADR 0169): the client's mirror of the server-owned chunks.

const POS := Vector3i(2, -1, 3)


func _buffer_with(m: int) -> VoxelBuffer:
	var vb := VoxelBuffer.new()
	vb.create(Settings.CHUNK, Settings.CHUNK, Settings.CHUNK)
	vb.set_voxel(m, 1, 2, 3, VoxelBuffer.CHANNEL_TYPE)
	return vb


func _bytes_with(m: int) -> PackedByteArray:
	return VoxelBlockSerializer.serialize_to_byte_array(_buffer_with(m), VoxelBlockSerializer.COMPRESSION_LZ4)


func test_put_keeps_the_newer_snapshot() -> void:
	var s := NetStream.new()
	assert_object(s.put(POS, _bytes_with(GameMaterials.STONE), 5)).is_not_null()
	assert_object(s.put(POS, _bytes_with(GameMaterials.DIRT), 3)).is_null()   # older journal position: ignored
	assert_int(s.baked(POS)).is_equal(5)
	assert_int(s.blocks[POS].get_voxel(1, 2, 3, VoxelBuffer.CHANNEL_TYPE)).is_equal(GameMaterials.STONE)
	assert_object(s.put(POS, _bytes_with(GameMaterials.DIRT), 7)).is_not_null()
	assert_int(s.baked(POS)).is_equal(7)
	assert_int(s.blocks[POS].get_voxel(1, 2, 3, VoxelBuffer.CHANNEL_TYPE)).is_equal(GameMaterials.DIRT)
	assert_int(s.size()).is_equal(1)


func test_an_own_snapshot_goes_to_the_outbox() -> void:
	var s := NetStream.new()
	s.known_n = 9
	s.put_own(POS, _buffer_with(GameMaterials.STONE))
	var out := s.take_outbox()
	assert_int(out.size()).is_equal(1)
	assert_that(out[0][0]).is_equal(POS)
	assert_int(NetStream.to_buffer(out[0][1]).get_voxel(1, 2, 3, VoxelBuffer.CHANNEL_TYPE)).is_equal(GameMaterials.STONE)
	assert_int(out[0][2]).is_equal(9)
	assert_array(s.take_outbox()).is_empty()   # taking empties it
	assert_int(s.baked(POS)).is_equal(9)
	assert_object(s.put(POS, _bytes_with(GameMaterials.DIRT), 8)).is_null()   # the server's older copy does not replace it


func test_overlay_lands_the_chunk_in_a_padded_block() -> void:
	var s := NetStream.new()
	s.put(POS, _bytes_with(GameMaterials.STONE), 1)
	var out := VoxelBuffer.new()
	out.create(18, 18, 18)   # a generated block one voxel wider than the chunk on every side
	out.fill(GameMaterials.DIRT, VoxelBuffer.CHANNEL_TYPE)
	var origin := POS * Settings.CHUNK - Vector3i.ONE
	assert_bool(s.overlay(out, origin, out.get_size())).is_true()
	assert_int(out.get_voxel(2, 3, 4, VoxelBuffer.CHANNEL_TYPE)).is_equal(GameMaterials.STONE)   # the chunk's 1,2,3
	assert_int(out.get_voxel(1, 1, 1, VoxelBuffer.CHANNEL_TYPE)).is_equal(0)                     # the chunk's 0,0,0: air
	assert_int(out.get_voxel(0, 0, 0, VoxelBuffer.CHANNEL_TYPE)).is_equal(GameMaterials.DIRT)    # the padding: generated
	assert_int(out.get_voxel(17, 17, 17, VoxelBuffer.CHANNEL_TYPE)).is_equal(GameMaterials.DIRT)
	var other := VoxelBuffer.new()
	other.create(16, 16, 16)
	assert_bool(s.overlay(other, (POS + Vector3i(5, 0, 0)) * Settings.CHUNK, other.get_size())).is_false()


func test_to_buffer_round_trips_a_voxel() -> void:
	var vb := NetStream.to_buffer(_bytes_with(GameMaterials.STONE))
	assert_object(vb).is_not_null()
	assert_int(vb.get_voxel(1, 2, 3, VoxelBuffer.CHANNEL_TYPE)).is_equal(GameMaterials.STONE)
	assert_int(vb.get_voxel(0, 0, 0, VoxelBuffer.CHANNEL_TYPE)).is_equal(0)
	assert_object(NetStream.to_buffer(PackedByteArray())).is_null()


func test_ground_range_reaches_the_mirror() -> void:
	var s := NetStream.new()
	s.native = func(_x: int, _z: int, _size: int) -> Vector2i: return Vector2i(1000, 1100)
	assert_that(s.ground_range(POS.x * Settings.CHUNK, POS.z * Settings.CHUNK, Settings.CHUNK)).is_equal(Vector2i(1000, 1100))
	s.put(Vector3i(POS.x, 80, POS.z), _bytes_with(GameMaterials.STONE), 1)   # a chunk at 1280..1295, above the ground
	assert_that(s.ground_range(POS.x * Settings.CHUNK, POS.z * Settings.CHUNK, Settings.CHUNK)).is_equal(Vector2i(1000, 81 * Settings.CHUNK))
	assert_that(s.ground_range((POS.x + 1) * Settings.CHUNK, POS.z * Settings.CHUNK, Settings.CHUNK)).is_equal(Vector2i(1000, 1100))   # another column
	assert_that(s.ground_range(POS.x * Settings.CHUNK, POS.z * Settings.CHUNK, 32)).is_equal(Vector2i(1000, 1100))   # not our block size: untouched
