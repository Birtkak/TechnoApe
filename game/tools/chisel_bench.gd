extends SceneTree

## P6 chisel spike (ADR 0006): is one native micro mesher cheap enough to build
## both library micro models and chisel overlay meshes for whole mesh blocks?
## Sanity checks the mesher on known shapes, then times it on realistic and
## worst-case 16^3 blocks of chiseled voxels (8^3 cells each). One JSON line.
##   godot_console.exe --headless --path game --script res://tools/chisel_bench.gd

const G := 8  # cells per voxel (6.25 cm)


func _init() -> void:
	var report := {"test": "chisel_bench", "grid": G}
	var tiles := PackedInt32Array([-1, 0, 1, 2])  # material 1 = stone tile 0, 2 = dirt, 3 = grass top
	var atlas := Vector2i(8, 6)
	var ok := true
	# --- sanity: a full voxel is 6 quads (12 triangles)
	var cells := MicroMesher.fill_box(PackedInt32Array(), Vector3i(G, G, G), Vector3i.ZERO, Vector3i(G, G, G), 1)
	var arrays: Array = MicroMesher.build_arrays(cells, Vector3i(G, G, G), G, tiles, atlas, Vector3.ZERO)
	var q_full: int = arrays[Mesh.ARRAY_INDEX].size() / 6
	report["full_voxel_quads"] = q_full
	ok = ok and q_full == 6
	# --- a voxel with one corner cell removed: 6 faces -> each of the 3 faces touching the corner splits, +3 inner faces
	cells[0] = 0
	arrays = MicroMesher.build_arrays(cells, Vector3i(G, G, G), G, tiles, atlas, Vector3.ZERO)
	report["corner_cut_quads"] = arrays[Mesh.ARRAY_INDEX].size() / 6
	# --- two voxels side by side, one material: shared face culled, quads merge per voxel (not across)
	cells = MicroMesher.fill_box(PackedInt32Array(), Vector3i(2 * G, G, G), Vector3i.ZERO, Vector3i(2 * G, G, G), 1)
	arrays = MicroMesher.build_arrays(cells, Vector3i(2 * G, G, G), G, tiles, atlas, Vector3.ZERO)
	var q_pair: int = arrays[Mesh.ARRAY_INDEX].size() / 6
	report["two_voxels_quads"] = q_pair  # 10 = 2 voxels x 6 - 2 shared, no merging across the border
	ok = ok and q_pair == 10
	# --- UV check: the full voxel's top face spans exactly one tile
	cells = MicroMesher.fill_box(PackedInt32Array(), Vector3i(G, G, G), Vector3i.ZERO, Vector3i(G, G, G), 1)
	arrays = MicroMesher.build_arrays(cells, Vector3i(G, G, G), G, tiles, atlas, Vector3.ZERO)
	var uv: PackedVector2Array = arrays[Mesh.ARRAY_TEX_UV]
	var umin := Vector2(9, 9)
	var umax := Vector2(-9, -9)
	for p in uv:
		umin = umin.min(p)
		umax = umax.max(p)
	report["uv_span"] = [snappedf((umax - umin).x * atlas.x, 0.001), snappedf((umax - umin).y * atlas.y, 0.001)]
	ok = ok and is_equal_approx((umax - umin).x * atlas.x, 1.0) and is_equal_approx((umax - umin).y * atlas.y, 1.0)
	# --- normals face outward (dot of centroid direction and normal > 0 on a cube)
	var verts: PackedVector3Array = arrays[Mesh.ARRAY_VERTEX]
	var norms: PackedVector3Array = arrays[Mesh.ARRAY_NORMAL]
	var idx: PackedInt32Array = arrays[Mesh.ARRAY_INDEX]
	var bad_wind := 0
	for t in range(0, idx.size(), 3):
		var a := verts[idx[t]]
		var b := verts[idx[t + 1]]
		var c := verts[idx[t + 2]]
		if (b - a).cross(c - a).dot(norms[idx[t]]) <= 0.0:
			bad_wind += 1
	report["bad_winding"] = bad_wind
	ok = ok and bad_wind == 0
	report["sanity_ok"] = ok

	# --- timing on 16^3 blocks of chiseled voxels (128^3 cells)
	var B := 16
	var S := B * G
	var rng := RandomNumberGenerator.new()
	rng.seed = 7
	# typical build: every voxel a full cube of stone with a random 1..3 chisel bites (stairs, bevels)
	var typical := PackedInt32Array()
	typical.resize(S * S * S)
	typical.fill(0)
	for vz in B:
		for vy in B:
			for vx in B:
				if (vx + vy + vz) % 3 == 0:
					continue  # some air so faces exist inside the block
				var o := Vector3i(vx, vy, vz) * G
				typical = MicroMesher.fill_box(typical, Vector3i(S, S, S), o, o + Vector3i(G, G, G), 1 + (vx + vz) % 2)
				for k in rng.randi_range(1, 3):
					var bite := Vector3i(rng.randi_range(0, 4), rng.randi_range(0, 4), rng.randi_range(0, 4))
					typical = MicroMesher.fill_box(typical, Vector3i(S, S, S), o + bite, o + bite + Vector3i(4, 4, 4), 0)
	report["typical_block"] = _time_build(typical, S, tiles, atlas)
	# worst case: 3D checkerboard of cells (every cell face visible, nothing merges)
	var worst := PackedInt32Array()
	worst.resize(S * S * S)
	for z in S:
		for y in S:
			for x in S:
				worst[x + S * (y + S * z)] = 1 if (x + y + z) % 2 == 0 else 0
	report["checkerboard_block"] = _time_build(worst, S, tiles, atlas)
	# one chiseled voxel alone (what one edit costs if we re-mesh per voxel)
	var one := MicroMesher.fill_box(PackedInt32Array(), Vector3i(G, G, G), Vector3i.ZERO, Vector3i(G, G, G), 1)
	one = MicroMesher.fill_box(one, Vector3i(G, G, G), Vector3i(2, 2, 2), Vector3i(6, 8, 6), 0)
	report["one_voxel"] = _time_build(one, G, tiles, atlas)
	print(JSON.stringify(report))
	quit(0 if ok else 1)


func _time_build(cells: PackedInt32Array, s: int, tiles: PackedInt32Array, atlas: Vector2i) -> Dictionary:
	var best := 1e18
	var quads := 0
	var mesh: ArrayMesh
	for i in 5:
		var t0 := Time.get_ticks_usec()
		mesh = MicroMesher.build(cells, Vector3i(s, s, s), G, tiles, atlas, Vector3.ZERO)
		var dt := Time.get_ticks_usec() - t0
		best = minf(best, dt)
		quads = mesh.surface_get_arrays(0)[Mesh.ARRAY_INDEX].size() / 6 if mesh else 0
	var t1 := Time.get_ticks_usec()
	var shape := mesh.create_trimesh_shape() if mesh else null
	var shape_us := Time.get_ticks_usec() - t1
	return {"cells": s * s * s, "quads": quads, "mesh_ms": snappedf(best / 1000.0, 0.01), "trimesh_ms": snappedf(shape_us / 1000.0, 0.01), "has_shape": shape != null}
