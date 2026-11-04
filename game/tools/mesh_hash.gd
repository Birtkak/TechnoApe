extends SceneTree

## Hashes VoxelMesherBlocky's output (merged faces and shader occlusion on, as the terrain meshes) over real 64-voxel
## mesh blocks at the ground of a spread of columns, so a mesher speed-up can be checked to change no triangle
## (ADR 0135): run with the old and the new DLL, the hashes match. Also prints the mesher's ms per block.
##   godot --headless --path . --script res://tools/mesh_hash.gd -- seed=1935341982 n=120

const SEA_LEVEL_VOXEL := 1024       # Settings.SEA_LEVEL_VOXEL (autoloads do not exist in --script runs)
const WORLD_HEIGHT_VOXELS := 5120   # Settings.WORLD_HEIGHT_VOXELS
const PAD := 6                      # VoxelMesherBlocky::PADDING of our build
const EDGE := 64                    # mesh block size (voxel_patch_mesh64.py)


func _init() -> void:
	var args := {}
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2:
			args[kv[0]] = kv[1]
	var g := FineGen.new()
	g.configure(int(args.get("seed", "1935341982")), SEA_LEVEL_VOXEL, WORLD_HEIGHT_VOXELS)
	var m := VoxelMesherBlocky.new()
	m.library = GameMaterials.build_library(StandardMaterial3D.new())
	m.set_merge_faces(true)
	m.set_ao_pages(true)
	var n := int(args.get("n", "120"))
	var ctx := HashingContext.new()
	ctx.start(HashingContext.HASH_SHA256)
	var rng := RandomNumberGenerator.new()
	rng.seed = 91
	var size := EDGE + 2 * PAD
	var us := 0
	var tris := 0
	for i in n:
		var x := rng.randi_range(-400, 400) * EDGE
		var z := rng.randi_range(-400, 400) * EDGE
		# the mesh block holding the ground, or one above / below it (cliffs, canopies, buried edges)
		var y := (floori(g.height_at(x + EDGE / 2, z + EDGE / 2) / float(EDGE)) + (i % 3) - 1) * EDGE
		var vb := VoxelBuffer.new()
		vb.set_channel_depth(VoxelBuffer.CHANNEL_TYPE, VoxelBuffer.DEPTH_16_BIT)
		vb.create(size, size, size)
		var bytes := g.generate_block(Vector3i(x - PAD, y - PAD, z - PAD), Vector3i(size, size, size), 0, 2)
		if bytes.size() == 2:
			vb.fill(bytes.decode_u16(0), VoxelBuffer.CHANNEL_TYPE)
		elif not bytes.is_empty():
			vb.set_channel_from_byte_array(VoxelBuffer.CHANNEL_TYPE, bytes)
		var t := Time.get_ticks_usec()
		var mesh := m.build_mesh(vb, [])
		us += Time.get_ticks_usec() - t
		if mesh == null:
			ctx.update(PackedByteArray([0]))
			continue
		for s in mesh.get_surface_count():
			var a := mesh.surface_get_arrays(s)
			for k in [Mesh.ARRAY_VERTEX, Mesh.ARRAY_TEX_UV, Mesh.ARRAY_TEX_UV2, Mesh.ARRAY_NORMAL, Mesh.ARRAY_COLOR, Mesh.ARRAY_INDEX]:
				if a[k] != null:
					ctx.update(var_to_bytes(a[k]))
			tris += a[Mesh.ARRAY_INDEX].size() / 3
	print(JSON.stringify({"mesh_hash": ctx.finish().hex_encode(), "blocks": n, "tris": tris, "mesh_ms_per_block": snappedf(us / 1000.0 / n, 0.01)}))
	quit(0)
