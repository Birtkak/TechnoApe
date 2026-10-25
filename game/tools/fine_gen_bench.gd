extends SceneTree

## Benchmark: FineGen.generate_block per LOD (ADR 0033), single thread, the
## blocks a flight over the spawn area asks for (surface, above, below).
##   godot --headless --path game --script res://tools/fine_gen_bench.gd [-- biome=forest] [deep=1] [seed=1337]
## With biome=<name> it runs over the nearest place of that biome (forests cost the most). deep=1 times the
## blocks 3-8 below the ground with the deep-rock shortcut on, as the game streams them (caves, ADR 0052).

const Settings := preload("res://scripts/settings.gd")


func _init() -> void:
	var gen := FineGen.new()
	var deep := false
	var seed := 1337
	for a in OS.get_cmdline_user_args():
		if a == "deep=1":
			deep = true
		elif a.begins_with("seed="):
			seed = int(a.substr(5))
	gen.configure(seed, Settings.SEA_LEVEL_VOXEL, Settings.WORLD_HEIGHT_VOXELS)
	gen.set_uniform_blocks(deep)
	var cx := 1000
	var cz := -700
	for a in OS.get_cmdline_user_args():
		if a.begins_with("biome="):
			var p: Vector3i = gen.find_nearest(a.substr(6), 0, 0, int(Settings.m2v(8000.0)))
			if p.y >= 0:
				cx = p.x
				cz = p.z
	var out := {"at": [cx, cz]}
	for lod in 8:
		var step := 1 << lod
		var size := 16 * step   # world voxels a 16^3 block spans at this LOD
		var times := PackedFloat32Array()
		var empty := 0
		var n := 0
		for bz in range(-3, 3):
			for bx in range(-3, 3):
				var x := bx * size + cx
				var z := bz * size + cz
				var h := gen.height_at(x + size / 2, z + size / 2)
				for by in ([-8, -7, -6, -5, -4, -3] if deep else [-2, -1, 0, 1, 2]):
					var y: int = (floori(float(h) / size) + int(by)) * size
					var t0 := Time.get_ticks_usec()
					var b: PackedByteArray = gen.generate_block(Vector3i(x, y, z), Vector3i(16, 16, 16), lod, 2)
					times.append((Time.get_ticks_usec() - t0) / 1000.0)
					if b.is_empty():
						empty += 1
					n += 1
		var sorted := times.duplicate()
		sorted.sort()
		var sum := 0.0
		for t in times:
			sum += t
		out["lod%d" % lod] = {"avg_ms": snappedf(sum / n, 0.001), "p50": snappedf(sorted[n / 2], 0.001), "max": snappedf(sorted[n - 1], 0.001), "empty": empty, "n": n}
	print(JSON.stringify(out))
	quit(0)
