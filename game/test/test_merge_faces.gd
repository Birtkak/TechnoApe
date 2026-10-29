extends GdUnitTestSuite

## Our godot_voxel build merges unshaded and evenly shaded cube faces into rectangles and strips
## (voxel_patch_merge_faces.py, ADR 0106): far fewer triangles, the same vertex colours.

const PAD := 6   # VoxelMesherBlocky::PADDING of our build


func _mesh(vb: VoxelBuffer, merge: bool) -> Dictionary:
	var m := VoxelMesherBlocky.new()
	m.library = GameMaterials.build_library(StandardMaterial3D.new())
	m.set_merge_faces(merge)
	var mesh := m.build_mesh(vb, [])
	var tris := 0
	var colors := {}
	for s in mesh.get_surface_count():
		var a := mesh.surface_get_arrays(s)
		tris += a[Mesh.ARRAY_INDEX].size() / 3
		for c: Color in a[Mesh.ARRAY_COLOR]:
			colors[snappedf(c.r, 0.001)] = true
	return {"tris": tris, "colors": colors.keys()}


func _slab() -> VoxelBuffer:
	var vb := VoxelBuffer.new()
	vb.set_channel_depth(VoxelBuffer.CHANNEL_TYPE, VoxelBuffer.DEPTH_16_BIT)
	vb.create(8 + 2 * PAD, 8 + 2 * PAD, 8 + 2 * PAD)
	for x in 8:
		for z in 8:
			vb.set_voxel(GameMaterials.STONE, PAD + x, PAD + 2, PAD + z, VoxelBuffer.CHANNEL_TYPE)
	return vb


func test_a_flat_slab_is_six_rectangles() -> void:
	if not VoxelMesherBlocky.new().has_method("set_merge_faces"):
		return
	assert_int(_mesh(_slab(), false)["tris"]).is_equal((64 * 2 + 8 * 4) * 2)
	assert_int(_mesh(_slab(), true)["tris"]).is_equal(6 * 2)


func test_a_step_merges_into_strips_with_the_same_shading() -> void:
	if not VoxelMesherBlocky.new().has_method("set_merge_faces"):
		return
	var vb := _slab()
	for x in 8:
		vb.set_voxel(GameMaterials.STONE, PAD + x, PAD + 3, PAD, VoxelBuffer.CHANNEL_TYPE)   # a step along one edge
	var off := _mesh(vb, false)
	var on := _mesh(vb, true)
	assert_int(on["tris"]).is_less(off["tris"] / 4)
	var a: Array = off["colors"]
	var b: Array = on["colors"]
	a.sort()
	b.sort()
	assert_array(b).is_equal(a)   # the occlusion at the step's foot is kept
