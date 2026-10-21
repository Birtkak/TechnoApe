extends SceneTree

## P2 part A, GDScript: throughput of generate + mesh + ArrayMesh upload for
## N chunks, headless. Prints one JSON line with the numbers.
##   godot_console.exe --headless --path . --script res://bench_gd.gd

const CHUNKS_PER_AXIS := 8  # 8x4x8 = 256 chunks
const CHUNKS_Y := 4


func _init() -> void:
	var noise2d := FastNoiseLite.new()
	noise2d.seed = 1337
	noise2d.frequency = 0.01
	var noise3d := FastNoiseLite.new()
	noise3d.seed = 7331
	noise3d.frequency = 0.05

	# Warm-up so JIT-less GDScript and caches are in a steady state.
	var warm := ChunkGd.generate(0, 1, 0, noise2d, noise3d)
	ChunkGd.build_array_mesh(ChunkGd.mesh(warm))

	var gen_us := 0
	var mesh_us := 0
	var upload_us := 0
	var faces := 0
	var count := 0
	for cz in CHUNKS_PER_AXIS:
		for cy in CHUNKS_Y:
			for cx in CHUNKS_PER_AXIS:
				var t0 := Time.get_ticks_usec()
				var vox := ChunkGd.generate(cx, cy, cz, noise2d, noise3d)
				var t1 := Time.get_ticks_usec()
				var arrays := ChunkGd.mesh(vox)
				var t2 := Time.get_ticks_usec()
				ChunkGd.build_array_mesh(arrays)
				var t3 := Time.get_ticks_usec()
				gen_us += t1 - t0
				mesh_us += t2 - t1
				upload_us += t3 - t2
				faces += (arrays[Mesh.ARRAY_INDEX] as PackedInt32Array).size() / 6
				count += 1
	var total_ms := (gen_us + mesh_us + upload_us) / 1000.0
	print(JSON.stringify({
		"lang": "gdscript",
		"chunks": count,
		"chunk_size": ChunkGd.SIZE,
		"faces": faces,
		"gen_ms_per_chunk": snappedf(gen_us / 1000.0 / count, 0.001),
		"mesh_ms_per_chunk": snappedf(mesh_us / 1000.0 / count, 0.001),
		"upload_ms_per_chunk": snappedf(upload_us / 1000.0 / count, 0.001),
		"total_ms": snappedf(total_ms, 0.1),
		"chunks_per_sec": snappedf(count / (total_ms / 1000.0), 0.1),
	}))
	quit(0)
