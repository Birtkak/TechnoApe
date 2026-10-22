extends SceneTree

## P9: the brick world store (BrickStore in the technosim extension) under
## realistic load: a 64 m cube of generated world (128^3 bricks = 1 G cells),
## surfaces expanded as chisel/sim would, random cell writes, a 10 m TNT
## sphere, collapse, and the 16^3 chunk feed for the 32 m near field.
##   godot_console.exe --headless --path prototypes/p7-raycast-near --script res://brick_bench.gd -- bricks=128

var bricks := 128


func _init() -> void:
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2 and kv[0] == "bricks":
			bricks = int(kv[1])
	var r := {"test": "brick_bench", "bricks_per_edge": bricks, "extent_m": bricks * 0.5}
	var store := BrickStore.new()
	store.configure(Vector3i(bricks, bricks, bricks))
	var gen := WorldGen.new()
	gen.configure(1337, 256, 768)
	gen.column_info(0, 0)
	# fill from the generator: 16^3 voxel blocks around the spawn, surface band centred on sea level + 40
	var t0 := Time.get_ticks_usec()
	var y0 := 256 + 40 - bricks / 2
	var blocks := 0
	for bz in range(0, bricks, 16):
		for by in range(0, bricks, 16):
			for bx in range(0, bricks, 16):
				var bytes: PackedByteArray = gen.generate_block(Vector3i(bx - bricks / 2, y0 + by, bz - bricks / 2), Vector3i(16, 16, 16), 0, 2)
				blocks += 1
				if bytes.is_empty():
					continue
				store.load_voxels(Vector3i(bx, by, bz), Vector3i(16, 16, 16), bytes)
	r["fill_ms"] = snappedf((Time.get_ticks_usec() - t0) / 1000.0, 0.1)
	r["fill_blocks"] = blocks
	store.clear_dirty()
	r["after_fill"] = store.stats()
	# expand every surface brick (worst case for chisel/sim detail everywhere)
	t0 = Time.get_ticks_usec()
	var expanded := store.expand_surfaces()
	r["expand_surfaces_ms"] = snappedf((Time.get_ticks_usec() - t0) / 1000.0, 0.1)
	r["expanded_surface_bricks"] = expanded
	r["after_expand"] = store.stats()
	store.clear_dirty()
	# random cell writes (chisel bits, sim cells) into the surface band
	var rng := RandomNumberGenerator.new()
	rng.seed = 5
	var n_writes := 200000
	t0 = Time.get_ticks_usec()
	for i in n_writes:
		var c := Vector3i(rng.randi_range(0, bricks * 8 - 1), rng.randi_range(bricks * 4 - 64, bricks * 4 + 64), rng.randi_range(0, bricks * 8 - 1))
		store.set_cell(c, 1 + (i % 3))
	var w_ms := (Time.get_ticks_usec() - t0) / 1000.0
	r["cell_writes"] = n_writes
	r["cell_writes_ms"] = snappedf(w_ms, 0.1)
	r["cell_writes_per_ms"] = int(n_writes / maxf(w_ms, 0.001))
	r["dirty_after_writes"] = store.get_dirty_bricks().size()
	store.clear_dirty()
	# 1000 TNT: a 10 m sphere (radius 80 cells) carved to air in the surface band
	t0 = Time.get_ticks_usec()
	var touched := store.carve_sphere(Vector3i(bricks * 4, bricks * 4, bricks * 4), 80, 0)
	r["tnt_sphere_ms"] = snappedf((Time.get_ticks_usec() - t0) / 1000.0, 0.01)
	r["tnt_bricks_touched"] = touched
	r["tnt_dirty_bricks"] = store.get_dirty_bricks().size()
	r["after_tnt"] = store.stats()
	store.clear_dirty()
	# black hole: a 5 m sphere per tick for 30 ticks moving 2 m/s at 30 Hz (6.7 cm per tick = 1 cell)
	t0 = Time.get_ticks_usec()
	var bh_touched := 0
	for k in 30:
		bh_touched += store.carve_sphere(Vector3i(bricks * 4 - 200 + k, bricks * 4 + 20, bricks * 4), 40, 0)
	r["black_hole_30_ticks_ms"] = snappedf((Time.get_ticks_usec() - t0) / 1000.0, 0.01)
	r["black_hole_bricks_touched"] = bh_touched
	# incremental collapse of only the bricks touched since the last clear (the TNT/black-hole bricks)
	t0 = Time.get_ticks_usec()
	var collapsed_dirty := store.collapse_dirty()
	r["collapse_dirty_ms"] = snappedf((Time.get_ticks_usec() - t0) / 1000.0, 0.01)
	r["collapsed_dirty"] = collapsed_dirty
	# collapse sweep over the whole 64 m cube
	t0 = Time.get_ticks_usec()
	var collapsed := store.collapse_all()
	r["collapse_all_ms"] = snappedf((Time.get_ticks_usec() - t0) / 1000.0, 0.1)
	r["collapsed"] = collapsed
	r["after_collapse"] = store.stats()
	# near-field feed: all 16^3 chunks of the central 32 m (64 bricks = 4096 chunks)
	t0 = Time.get_ticks_usec()
	var c0 := (bricks / 2 - 32) / 2
	var total := 0
	for cz in range(c0, c0 + 32):
		for cy in range(c0, c0 + 32):
			for cx in range(c0, c0 + 32):
				total += store.get_chunk_bytes(Vector3i(cx, cy, cz)).size()
	r["feed_32m_chunks_ms"] = snappedf((Time.get_ticks_usec() - t0) / 1000.0, 0.1)
	r["feed_32m_bytes_mb"] = snappedf(total / 1048576.0, 0.1)
	r["process_mem_mb"] = snappedf(OS.get_static_memory_usage() / 1048576.0, 0.1)
	print(JSON.stringify(r))
	quit(0)
