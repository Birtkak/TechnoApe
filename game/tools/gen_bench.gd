extends SceneTree

## Headless generator benchmark (ADR 0020): time WorldGen.generate_block per
## LOD on a strip of blocks around the spawn, JSON on stdout. Run:
##   godot_console.exe --headless --path game --script res://tools/gen_bench.gd -- blocks=64 seed=1337

const S := preload("res://scripts/settings.gd")   # autoloads are absent under --script; constants and statics only


func _init() -> void:
	var args := {}
	for a in OS.get_cmdline_user_args():
		var kv := a.split("=")
		if kv.size() == 2:
			args[kv[0]] = kv[1]
	var blocks := int(args.get("blocks", "64"))
	var sd := int(args.get("seed", "1337"))
	var g := WorldGen.new()
	g.configure(sd, S.SEA_LEVEL_VOXEL, S.WORLD_HEIGHT_VOXELS)
	var size := Vector3i(16, 16, 16)   # data block + no padding (what the terrain asks for)
	var out := {"seed": sd, "blocks_per_lod": blocks, "lods": {}}
	for lod in 6:
		var step := 1 << lod
		var t0 := Time.get_ticks_usec()
		var non_empty := 0
		var bytes_total := 0
		var side := int(ceil(sqrt(blocks)))
		for i in blocks:
			# a square of blocks at surface height, each 16 cells wide at this lod (a
			# row would cross a new 2 km hydrology tile every few blocks and time the
			# tile build instead: ~35 ms per tile, cached globally in the game)
			var wx := (i % side) * 16 * step
			var wz := (i / side) * 16 * step
			var h: int = g.height_at(wx + 8 * step, wz + 8 * step)
			var y0 := ((h - 8 * step) / (16 * step)) * (16 * step)
			var b: PackedByteArray = g.generate_block(Vector3i(wx, y0, wz), size, lod, 2)
			if not b.is_empty():
				non_empty += 1
				bytes_total += b.size()
		var dt := (Time.get_ticks_usec() - t0) / 1000.0
		out["lods"][str(lod)] = {"ms_total": snappedf(dt, 0.01), "ms_per_block": snappedf(dt / blocks, 0.001), "non_empty": non_empty, "voxels_per_block": 16 * 16 * 16, "world_width_m": S.v2m(16 * step)}
	# column queries at LOD spacings: the cost grows with the spacing because each call that
	# enters a new 2 km hydrology tile pays for that tile (~35 ms); the noise itself is ~17 us
	out["height_at_us_per_call_by_spacing"] = {}
	for sp in [1, 2, 4, 8, 16, 32, 64]:
		var t0 := Time.get_ticks_usec()
		var acc := 0
		for i in 4096:
			acc += g.height_at(100000 + i * sp, 777)
		out["height_at_us_per_call_by_spacing"][str(sp)] = snappedf((Time.get_ticks_usec() - t0) / 4096.0, 0.01)
	print(JSON.stringify(out))
	quit(0)
