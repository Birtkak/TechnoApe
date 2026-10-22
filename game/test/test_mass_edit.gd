extends GdUnitTestSuite

## Mass edits in the native kernel (ADR 0012: explosions, black holes and big digs
## never iterate voxels in GDScript): SimWorld.fill_sphere and ignite_box.

const N := 3


func _world(loaded := true) -> SimWorld:
	var w := SimWorld.new()
	w.configure(Vector3i(N, N, N), 2, 5)
	w.set_materials(GameMaterials.defs())
	w.set_reactions(GameMaterials.reactions())
	w.mark_all_loaded(loaded)
	return w


func _ball_count(c: Vector3, r: float) -> int:
	var n := 0
	for z in 48:
		for y in 48:
			for x in 48:
				if (Vector3(x, y, z) + Vector3.ONE * 0.5 - c).length_squared() <= r * r:
					n += 1
	return n


func test_sphere_carves_the_exact_ball() -> void:
	var w := _world()
	w.fill_box(Vector3i.ZERO, Vector3i(48, 48, 48), GameMaterials.STONE)
	var c := Vector3(24.3, 23.8, 24.0)   # off-centre, straddles chunk borders at 16 / 32
	var expect := _ball_count(c, 9.5)
	assert_int(w.fill_sphere(c, 9.5, GameMaterials.AIR)).is_equal(expect)
	assert_int(w.count_material(GameMaterials.AIR)).is_equal(expect)
	# a second blast over the same hole changes nothing
	assert_int(w.fill_sphere(c, 9.5, GameMaterials.AIR)).is_equal(0)


func test_kept_materials_survive() -> void:
	var w := _world()
	w.fill_box(Vector3i.ZERO, Vector3i(48, 48, 48), GameMaterials.STONE)
	w.set_voxel(Vector3i(24, 24, 24), GameMaterials.FURNACE)
	w.set_voxel(Vector3i(25, 24, 24), GameMaterials.BEDROCK)
	var keep := PackedInt32Array([GameMaterials.FURNACE, GameMaterials.BEDROCK])
	w.fill_sphere(Vector3(24.5, 24.5, 24.5), 4.0, GameMaterials.AIR, keep)
	assert_int(w.get_voxel(Vector3i(24, 24, 24))).is_equal(GameMaterials.FURNACE)
	assert_int(w.get_voxel(Vector3i(25, 24, 24))).is_equal(GameMaterials.BEDROCK)
	assert_int(w.get_voxel(Vector3i(23, 24, 24))).is_equal(GameMaterials.AIR)


func test_unloaded_chunks_are_not_edited() -> void:
	var w := _world(false)
	var b := PackedByteArray()
	b.resize(8192)
	for i in 4096:
		b[i * 2] = GameMaterials.STONE
	w.set_chunk_bytes(Vector3i(1, 1, 1), b, 2)   # only the centre chunk holds data
	var n := w.fill_sphere(Vector3(16, 24, 24), 6.0, GameMaterials.WATER)
	# the ball's half in chunk (0,1,1) is skipped: its data would be overwritten when it loads
	assert_int(w.count_in_box(Vector3i(0, 0, 0), Vector3i(16, 48, 48), GameMaterials.WATER)).is_equal(0)
	assert_int(w.count_in_box(Vector3i(16, 16, 16), Vector3i(32, 32, 32), GameMaterials.WATER)).is_equal(n)
	assert_int(n).is_greater(0)


func test_dirty_boxes_cover_the_blast() -> void:
	var w := _world()
	w.fill_box(Vector3i.ZERO, Vector3i(48, 48, 48), GameMaterials.DIRT)
	w.clear_dirty()
	var before := {}
	for cz in N:
		for cy in N:
			for cx in N:
				before[Vector3i(cx, cy, cz)] = w.get_chunk_bytes(Vector3i(cx, cy, cz), 1)
	w.fill_sphere(Vector3(30, 18, 20), 7.0, GameMaterials.AIR)
	var boxes := {}
	var bx: PackedInt32Array = w.get_dirty_boxes()
	for k in range(0, bx.size(), 9):
		boxes[Vector3i(bx[k], bx[k + 1], bx[k + 2])] = [Vector3i(bx[k + 3], bx[k + 4], bx[k + 5]), Vector3i(bx[k + 6], bx[k + 7], bx[k + 8])]
	var changed := 0
	for c in before:
		var now: PackedByteArray = w.get_chunk_bytes(c, 1)
		var old: PackedByteArray = before[c]
		for i in 4096:
			if now[i] == old[i]:
				continue
			changed += 1
			var cell := Vector3i((i / 16) % 16, i % 16, i / 256)   # y fastest, then x, then z
			assert_bool(boxes.has(c)).override_failure_message("chunk %s changed but not listed" % c).is_true()
			var b: Array = boxes[c]
			var inside: bool = cell.x >= b[0].x and cell.y >= b[0].y and cell.z >= b[0].z and cell.x <= b[1].x and cell.y <= b[1].y and cell.z <= b[1].z
			assert_bool(inside).override_failure_message("%s cell %s outside %s" % [c, cell, b]).is_true()
	assert_int(changed).is_greater(1000)


func test_crater_in_static_sand_slumps() -> void:
	var w := _world()
	w.fill_box(Vector3i.ZERO, Vector3i(48, 1, 48), GameMaterials.STONE)
	w.fill_box(Vector3i(0, 1, 0), Vector3i(48, 20, 48), GameMaterials.SAND_STATIC)
	var hole := w.fill_sphere(Vector3(24, 14, 24), 6.0, GameMaterials.AIR)
	for i in 90:
		w.tick(1)
	# the crater's fresh walls are loosened (ADR 0013): sand runs back into the hole
	var still_air := w.count_in_box(Vector3i(18, 8, 18), Vector3i(30, 14, 30), GameMaterials.AIR)
	assert_int(still_air).is_less(hole / 4)


func test_ignite_box_lights_only_burnables() -> void:
	var w := _world()
	w.fill_box(Vector3i(10, 10, 10), Vector3i(14, 14, 14), GameMaterials.WOOD)
	w.fill_box(Vector3i(14, 10, 10), Vector3i(18, 14, 14), GameMaterials.STONE)
	assert_int(w.ignite_box(Vector3i(8, 8, 8), Vector3i(20, 20, 20))).is_equal(64)
	assert_int(w.count_burning()).is_equal(64)
	assert_int(w.ignite_box(Vector3i(8, 8, 8), Vector3i(20, 20, 20))).is_equal(0)   # already burning
